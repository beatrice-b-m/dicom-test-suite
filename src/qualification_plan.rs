//! Frontend-neutral construction of payload-free qualification plans.
//!
//! Source DICOM instances are planned by their owning recipe providers. This
//! module only binds already-planned private sources, their execution bindings,
//! and their preflight identities to a qualification artifact.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use serde_json::Value;

use crate::corpus_plan::{
    ArtifactDependency, ArtifactProvenance, ArtifactResourceEstimate, CaseBinding,
    EvidenceIndependence, EvidenceObligation, EvidencePlan, PlannedArtifact, PlannedDicomArtifact,
    PlannedQualification, PlannedQualificationSource, QualificationPayloadPolicy, ValidationPlan,
    ValidationRequirement, ValidationRule,
};
use crate::executor::services::{
    ArtifactExecutionBindings, SlotExecutionBinding, StagedAssetHandle,
};
use crate::recipes::{
    qualification_parameters, CaseRecipe, QualificationParameters, EOT_ARITHMETIC_PLAN_PROVIDER_ID,
    FUZZ_PLAN_PROVIDER_ID,
};

const FUZZ_KIND: &str = "bounded_deterministic_fuzz";
const EOT_KIND: &str = "checked_eot_u64_overflow";

/// A caller-planned source and its exact identity at the preflight boundary.
#[derive(Debug, Clone, PartialEq)]
pub struct PreparedQualificationSource {
    pub artifact: PlannedDicomArtifact,
    pub bindings: ArtifactExecutionBindings,
    /// Stable role from the qualification recipe, not a generated slot name.
    pub dependency_role: String,
    /// Artifact-local logical ID from the referenced source recipe.
    pub recipe_artifact_logical_id: String,
    pub preflight_sha256: String,
    pub preflight_size_bytes: u64,
}

/// Caller-owned run context for one qualification recipe.
#[derive(Debug, Clone, PartialEq)]
pub struct QualificationPlanRequest<'a> {
    pub recipe: &'a CaseRecipe,
    /// The already-parsed value is accepted explicitly so callers cannot hide
    /// a parser/provider mismatch. It must equal the recipe document exactly.
    pub parameters: QualificationParameters,
    pub logical_id: String,
    pub order: u64,
    pub run_seed: Option<u64>,
    pub profile: Option<String>,
    pub sources: Vec<PreparedQualificationSource>,
}

/// Closed artifact/binding fragment suitable for insertion into one CorpusPlan.
#[derive(Debug, Clone, PartialEq)]
pub struct QualificationPlanOutput {
    pub artifacts: Vec<PlannedArtifact>,
    pub dependencies: Vec<ArtifactDependency>,
    pub bindings: BTreeMap<String, ArtifactExecutionBindings>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QualificationPlanError {
    UnsupportedProvider(String),
    MissingQualification,
    ParameterDrift,
    InvalidRunContext(&'static str),
    SourceCount { expected: usize, actual: usize },
    SourceOrder { index: usize, message: String },
    SourceNotPrivate(String),
    SourcePublished(String),
    SourceBindingMismatch(String),
    InvalidPreflight(String),
    DuplicateArtifact(String),
    DuplicateOrder(u64),
    Binding(String),
}

impl fmt::Display for QualificationPlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedProvider(id) => write!(formatter, "unsupported provider {id}"),
            Self::MissingQualification => {
                formatter.write_str("recipe lacks qualification contract")
            }
            Self::ParameterDrift => formatter.write_str("typed parameters differ from recipe"),
            Self::InvalidRunContext(message) => formatter.write_str(message),
            Self::SourceCount { expected, actual } => {
                write!(formatter, "expected {expected} sources, received {actual}")
            }
            Self::SourceOrder { index, message } => {
                write!(formatter, "source {index} does not match recipe: {message}")
            }
            Self::SourceNotPrivate(id) => write!(formatter, "source {id} is not private"),
            Self::SourcePublished(id) => write!(formatter, "source {id} is publicly publishable"),
            Self::SourceBindingMismatch(id) => {
                write!(formatter, "source binding mismatch for {id}")
            }
            Self::InvalidPreflight(id) => write!(formatter, "invalid preflight identity for {id}"),
            Self::DuplicateArtifact(id) => write!(formatter, "duplicate artifact ID {id}"),
            Self::DuplicateOrder(order) => write!(formatter, "duplicate artifact order {order}"),
            Self::Binding(message) => write!(formatter, "invalid staged binding: {message}"),
        }
    }
}

impl Error for QualificationPlanError {}

pub fn plan_qualification(
    request: QualificationPlanRequest<'_>,
) -> Result<QualificationPlanOutput, QualificationPlanError> {
    let parsed = qualification_parameters(request.recipe)
        .map_err(|_| QualificationPlanError::MissingQualification)?;
    if parsed != request.parameters {
        return Err(QualificationPlanError::ParameterDrift);
    }
    match request.parameters.clone() {
        QualificationParameters::BoundedDeterministicFuzz { .. } => plan_fuzz(request),
        QualificationParameters::CheckedEotU64Overflow { .. } => plan_eot(request),
    }
}

fn plan_fuzz(
    request: QualificationPlanRequest<'_>,
) -> Result<QualificationPlanOutput, QualificationPlanError> {
    if request.recipe.plan_provider_id != FUZZ_PLAN_PROVIDER_ID {
        return Err(QualificationPlanError::UnsupportedProvider(
            request.recipe.plan_provider_id.clone(),
        ));
    }
    if request.profile.as_deref() != Some("fuzz") || request.run_seed.is_none() {
        return Err(QualificationPlanError::InvalidRunContext(
            "fuzz qualification requires profile fuzz and a run seed",
        ));
    }
    let QualificationParameters::BoundedDeterministicFuzz {
        sources: expected_sources,
        budget,
        ..
    } = &request.parameters
    else {
        unreachable!("variant selected by plan_qualification")
    };
    if request.sources.len() != expected_sources.len() {
        return Err(QualificationPlanError::SourceCount {
            expected: expected_sources.len(),
            actual: request.sources.len(),
        });
    }

    let qualification_binding = case_binding(request.recipe);
    let qualification_id = request.logical_id.clone();
    let mut artifacts = Vec::with_capacity(request.sources.len() + 1);
    let mut dependencies = Vec::with_capacity(request.sources.len());
    let mut bindings = BTreeMap::new();
    let mut planned_sources = Vec::with_capacity(request.sources.len());
    let mut qualification_slots = BTreeMap::new();
    let mut artifact_ids = BTreeSet::from([qualification_id.clone()]);
    let mut orders = BTreeSet::from([request.order]);

    for (index, (prepared, expected)) in request
        .sources
        .into_iter()
        .zip(expected_sources)
        .enumerate()
    {
        let source_id = prepared.artifact.logical_id.clone();
        if prepared.dependency_role != expected.dependency_role
            || prepared.recipe_artifact_logical_id != expected.artifact_logical_id
        {
            return Err(QualificationPlanError::SourceOrder {
                index,
                message: "role or artifact logical ID differs".into(),
            });
        }
        let expected_binding = CaseBinding {
            case_id: prepared
                .artifact
                .case_binding
                .as_ref()
                .map(|binding| binding.case_id.clone())
                .ok_or_else(|| QualificationPlanError::SourceOrder {
                    index,
                    message: "source has no case binding".into(),
                })?,
            recipe_id: expected.recipe.recipe_id.clone(),
            recipe_version: expected.recipe.recipe_version.clone(),
        };
        if prepared.artifact.case_binding.as_ref() != Some(&expected_binding) {
            return Err(QualificationPlanError::SourceOrder {
                index,
                message: "recipe identity differs".into(),
            });
        }
        match &prepared.artifact.provenance {
            ArtifactProvenance::PrivateSource { consumed_by }
                if consumed_by == &[qualification_id.clone()] => {}
            _ => return Err(QualificationPlanError::SourceNotPrivate(source_id)),
        }
        if prepared.artifact.output.publish {
            return Err(QualificationPlanError::SourcePublished(source_id));
        }
        if prepared.bindings.artifact_id != source_id {
            return Err(QualificationPlanError::SourceBindingMismatch(source_id));
        }
        if prepared.preflight_size_bytes == 0 || !valid_sha256(&prepared.preflight_sha256) {
            return Err(QualificationPlanError::InvalidPreflight(source_id));
        }
        if !artifact_ids.insert(source_id.clone()) {
            return Err(QualificationPlanError::DuplicateArtifact(source_id));
        }
        if !orders.insert(prepared.artifact.order) {
            return Err(QualificationPlanError::DuplicateOrder(
                prepared.artifact.order,
            ));
        }

        let slot = format!("source_{index:02}");
        let asset = StagedAssetHandle::new(format!("output:{source_id}"))
            .map_err(|error| QualificationPlanError::Binding(error.to_string()))?;
        qualification_slots.insert(slot.clone(), SlotExecutionBinding::StagedAsset { asset });
        planned_sources.push(PlannedQualificationSource {
            artifact_id: source_id.clone(),
            case_binding: expected_binding,
            artifact_logical_id: expected.artifact_logical_id.clone(),
            dependency_role: expected.dependency_role.clone(),
            binding_slot: slot,
            expected_sha256: prepared.preflight_sha256,
            expected_size_bytes: prepared.preflight_size_bytes,
            parameters: BTreeMap::from([
                (
                    "seed_description_id".into(),
                    Value::String(expected.seed_description_id.clone()),
                ),
                (
                    "mutation_surfaces".into(),
                    Value::Array(
                        expected
                            .mutation_surfaces
                            .iter()
                            .cloned()
                            .map(Value::String)
                            .collect(),
                    ),
                ),
            ]),
        });
        dependencies.push(ArtifactDependency {
            artifact_id: qualification_id.clone(),
            depends_on: source_id.clone(),
            relationship: expected.dependency_role.clone(),
            frame_numbers: Vec::new(),
        });
        bindings.insert(source_id, prepared.bindings);
        artifacts.push(PlannedArtifact::Dicom(prepared.artifact));
    }

    let qualification = request
        .recipe
        .qualification
        .as_ref()
        .ok_or(QualificationPlanError::MissingQualification)?;
    let peak_working_bytes = budget.max_input_bytes.max(budget.max_output_bytes).max(1);
    artifacts.push(PlannedArtifact::Qualification(PlannedQualification {
        logical_id: qualification_id.clone(),
        order: request.order,
        provenance: ArtifactProvenance::Requested,
        case_binding: Some(qualification_binding),
        profile: request.profile,
        run_seed: request.run_seed,
        qualification_kind: FUZZ_KIND.into(),
        parameters: qualification.parameters.clone().into_iter().collect(),
        sources: planned_sources,
        payload_policy: QualificationPayloadPolicy::NoPayload,
        validation: validation_plan(request.recipe),
        evidence: evidence_plan(request.recipe),
        resources: ArtifactResourceEstimate {
            output_bytes: 0,
            peak_working_bytes,
        },
    }));
    bindings.insert(
        qualification_id.clone(),
        ArtifactExecutionBindings {
            artifact_id: qualification_id,
            slots: qualification_slots,
        },
    );
    Ok(QualificationPlanOutput {
        artifacts,
        dependencies,
        bindings,
    })
}

fn plan_eot(
    request: QualificationPlanRequest<'_>,
) -> Result<QualificationPlanOutput, QualificationPlanError> {
    if request.recipe.plan_provider_id != EOT_ARITHMETIC_PLAN_PROVIDER_ID {
        return Err(QualificationPlanError::UnsupportedProvider(
            request.recipe.plan_provider_id.clone(),
        ));
    }
    if !request.sources.is_empty() {
        return Err(QualificationPlanError::SourceCount {
            expected: 0,
            actual: request.sources.len(),
        });
    }
    if request.profile.is_some() || request.run_seed.is_some() {
        return Err(QualificationPlanError::InvalidRunContext(
            "EOT arithmetic qualification has no public profile or run seed",
        ));
    }
    let qualification = request
        .recipe
        .qualification
        .as_ref()
        .ok_or(QualificationPlanError::MissingQualification)?;
    let logical_id = request.logical_id;
    let artifact = PlannedArtifact::Qualification(PlannedQualification {
        logical_id: logical_id.clone(),
        order: request.order,
        provenance: ArtifactProvenance::Requested,
        case_binding: Some(case_binding(request.recipe)),
        profile: None,
        run_seed: None,
        qualification_kind: EOT_KIND.into(),
        parameters: qualification.parameters.clone().into_iter().collect(),
        sources: Vec::new(),
        payload_policy: QualificationPayloadPolicy::EvidenceOnly,
        validation: validation_plan(request.recipe),
        evidence: evidence_plan(request.recipe),
        resources: ArtifactResourceEstimate {
            output_bytes: 0,
            peak_working_bytes: 1,
        },
    });
    Ok(QualificationPlanOutput {
        artifacts: vec![artifact],
        dependencies: Vec::new(),
        bindings: BTreeMap::from([(
            logical_id.clone(),
            ArtifactExecutionBindings {
                artifact_id: logical_id,
                slots: BTreeMap::new(),
            },
        )]),
    })
}

fn case_binding(recipe: &CaseRecipe) -> CaseBinding {
    CaseBinding {
        case_id: recipe.binding.case_id.clone(),
        recipe_id: recipe.recipe_id.clone(),
        recipe_version: recipe.recipe_version.clone(),
    }
}

fn validation_plan(recipe: &CaseRecipe) -> ValidationPlan {
    ValidationPlan {
        rules: recipe
            .validation_rule_ids
            .iter()
            .cloned()
            .map(|rule_id| ValidationRule {
                rule_id,
                requirement: ValidationRequirement::Required,
                parameters: BTreeMap::new(),
            })
            .collect(),
    }
}

fn evidence_plan(recipe: &CaseRecipe) -> EvidencePlan {
    EvidencePlan {
        obligations: recipe
            .projection_rule_ids
            .iter()
            .cloned()
            .map(|route_id| EvidenceObligation {
                obligation_id: route_id.clone(),
                route_id,
                independence: EvidenceIndependence::SameProject,
                required: true,
                parameters: BTreeMap::new(),
            })
            .collect(),
    }
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}
