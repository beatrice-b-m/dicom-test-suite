//! Shared plan-only support for semantic DICOM recipe providers.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::composition::{
    AttributeOperation, IdentityPlan, MaterializedReference, ResolvedAttribute,
    ResolvedInstancePlan, TemplateId, TemplateVersion, ValueOrigin,
};
use crate::corpus_plan::{
    ArtifactProvenance, ArtifactResourceEstimate, CaseBinding, EncodingPlan, EvidenceIndependence,
    EvidenceObligation, EvidencePlan, OutputPlan, PlannedDicomArtifact, ValidationPlan,
    ValidationRequirement, ValidationRule,
};
use crate::executor::services::ArtifactExecutionBindings;
use crate::planning::RecipeIdentity;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticSource {
    pub recipe: RecipeIdentity,
    pub artifact_id: String,
    pub role: String,
    pub reference: MaterializedReference,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticPlanContext {
    pub case_id: String,
    pub recipe: RecipeIdentity,
    pub logical_id: String,
    pub order: u64,
    pub output: OutputPlan,
    pub template_id: String,
    pub template_version: String,
    pub identities: IdentityPlan,
    pub encoding: EncodingPlan,
    #[serde(default)]
    pub base_attributes: Vec<ResolvedAttribute>,
    #[serde(default)]
    pub sources: Vec<SemanticSource>,
    pub resources: ArtifactResourceEstimate,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticPlanOutput {
    pub artifact: PlannedDicomArtifact,
    pub bindings: ArtifactExecutionBindings,
}

pub(crate) fn build_semantic_plan(
    context: &SemanticPlanContext,
    sop_class_uid: &str,
    operations: Vec<AttributeOperation>,
    content: Vec<crate::composition::CanonicalContent>,
    validation_rules: &[&str],
    evidence_route: &str,
) -> Result<SemanticPlanOutput, SemanticPlanError> {
    validate_context(context)?;
    let mut attributes = context.base_attributes.clone();
    let mut addresses = attributes
        .iter()
        .map(|attribute| attribute.address.clone())
        .collect::<BTreeSet<_>>();
    for operation in operations {
        let AttributeOperation::Set { address, vr, value } = operation else {
            return Err(SemanticPlanError::NonSetOperation);
        };
        if !addresses.insert(address.clone()) {
            return Err(SemanticPlanError::DuplicateAttribute(
                address.normalized_tag(),
            ));
        }
        attributes.push(ResolvedAttribute {
            address,
            vr,
            value: Some(value),
            origin: ValueOrigin::DerivedStructural,
        });
    }
    let references = context
        .sources
        .iter()
        .map(|source| source.reference.clone())
        .collect::<Vec<_>>();
    let implementation = context
        .identities
        .get(
            &crate::composition::CompositionUidRole::ImplementationClass,
            0,
        )
        .ok_or(SemanticPlanError::MissingImplementationIdentity)?;
    if implementation != context.encoding.implementation.class_uid {
        return Err(SemanticPlanError::ImplementationIdentityMismatch);
    }
    let artifact = PlannedDicomArtifact {
        logical_id: context.logical_id.clone(),
        order: context.order,
        provenance: ArtifactProvenance::Requested,
        case_binding: Some(CaseBinding {
            case_id: context.case_id.clone(),
            recipe_id: context.recipe.recipe_id.clone(),
            recipe_version: context.recipe.recipe_version.clone(),
        }),
        instance: ResolvedInstancePlan {
            plan_schema_version: "0.1.0".into(),
            instance_id: context.logical_id.clone(),
            template_id: TemplateId(context.template_id.clone()),
            template_version: context
                .template_version
                .parse::<TemplateVersion>()
                .map_err(|error| SemanticPlanError::Template(error.to_string()))?,
            sop_class_uid: sop_class_uid.into(),
            transfer_syntax_uid: context.encoding.transfer_syntax_uid.clone(),
            identities: context.identities.clone(),
            attributes,
            content,
            references,
        },
        output: context.output.clone(),
        encoding: context.encoding.clone(),
        validation: ValidationPlan {
            rules: validation_rules
                .iter()
                .map(|rule_id| ValidationRule {
                    rule_id: (*rule_id).into(),
                    requirement: ValidationRequirement::Required,
                    parameters: BTreeMap::new(),
                })
                .collect(),
        },
        evidence: EvidencePlan {
            obligations: vec![EvidenceObligation {
                obligation_id: format!("same-project:{}", context.logical_id),
                route_id: evidence_route.into(),
                independence: EvidenceIndependence::SameProject,
                required: true,
                parameters: BTreeMap::new(),
            }],
        },
        resources: context.resources.clone(),
    };
    Ok(SemanticPlanOutput {
        bindings: ArtifactExecutionBindings {
            artifact_id: context.logical_id.clone(),
            slots: BTreeMap::new(),
        },
        artifact,
    })
}

fn validate_context(context: &SemanticPlanContext) -> Result<(), SemanticPlanError> {
    if context.case_id.is_empty()
        || context.recipe.recipe_id.is_empty()
        || context.recipe.recipe_version.is_empty()
        || context.logical_id.is_empty()
        || context.template_id.is_empty()
        || context.template_version.is_empty()
        || context.identities.logical_instance_id != context.logical_id
        || !context.output.publish
        || context.resources.output_bytes == 0
        || context.resources.peak_working_bytes == 0
    {
        return Err(SemanticPlanError::InvalidContext);
    }
    let mut roles = BTreeSet::new();
    let mut artifacts = BTreeSet::new();
    for source in &context.sources {
        if source.role.is_empty()
            || source.artifact_id.is_empty()
            || source.reference.source_instance_id != context.logical_id
            || source.reference.target_instance_id != source.artifact_id
            || source.reference.role != source.role
            || !roles.insert(source.role.clone())
            || !artifacts.insert(source.artifact_id.clone())
        {
            return Err(SemanticPlanError::InvalidSource);
        }
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SemanticPlanError {
    InvalidContext,
    InvalidSource,
    Attribute(String),
    NonSetOperation,
    DuplicateAttribute(String),
    MissingImplementationIdentity,
    ImplementationIdentityMismatch,
    Template(String),
}

impl fmt::Display for SemanticPlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for SemanticPlanError {}
