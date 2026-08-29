//! Direct, frontend-neutral plans for spatial registration objects.
//!
//! Registration sources are supplied as complete plans and typed references.
//! This module never opens those sources: their identities are checked before
//! the registration plan (and therefore before staging) is returned.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::composition::{
    AttributeAddress, AttributeItem, AttributeOperation, AttributeValue, CompositionUidRole,
    DicomVr, IdentityPlan, MaterializedReference, PrimitiveValue, ResolvedAttribute,
    ResolvedInstancePlan, TemplateId, TemplateVersion, ValueOrigin,
};
use crate::corpus_plan::{
    ArtifactDependency, ArtifactProvenance, ArtifactResourceEstimate, CaseBinding, EncodingPlan,
    EvidenceIndependence, EvidenceObligation, EvidencePlan, FileMetaPolicy, FragmentationPolicy,
    ImplementationIdentityPlan, ItemLengthPolicy, OffsetTablePolicy, OutputPlan,
    OutputRelativePath, PlannedDicomArtifact, PreamblePolicy, SequenceLengthPolicy, ValidationPlan,
    ValidationRequirement, ValidationRule,
};
use crate::executor::services::ArtifactExecutionBindings;
use crate::uid::{DeterministicUidInput, UidRole, deterministic_uid};
use crate::{IMPLEMENTATION_VERSION_NAME, PACKAGE_VERSION};

use super::{
    AdvancedArtifactPlanningContext, AdvancedArtifactProvenance, AdvancedArtifactRole,
    AdvancedPlanProvider, AdvancedPlanProviderOutput, AdvancedPlanProviderRequest,
    AdvancedPlannedArtifact, AdvancedProviderContractError, AdvancedProviderFamily,
    AdvancedSourceConsumer, AdvancedSourceReference, AdvancedSourceRole, CaseRecipe,
    RecipeReference,
};

const EXPLICIT_VR_LE: &str = "1.2.840.10008.1.2.1";
const ENHANCED_CT_SOP: &str = "1.2.840.10008.5.1.4.1.1.2.1";
const CLASSIC_CT_SOP: &str = "1.2.840.10008.5.1.4.1.1.2";
const SPATIAL_REGISTRATION_SOP: &str = "1.2.840.10008.5.1.4.1.1.66.1";
const DEFORMABLE_REGISTRATION_SOP: &str = "1.2.840.10008.5.1.4.1.1.66.3";

pub const REGISTRATION_PLAN_PROVIDER_ID: &str = "native.registration_plan";
pub const REGISTRATION_ALGORITHM_PROVIDER_ID: &str = "algorithm.registration";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegistrationCommonInput {
    pub logical_id: String,
    pub order: u64,
    pub output_path: OutputRelativePath,
    pub template_id: String,
    pub series_number: String,
    pub study_id: String,
    pub laterality: String,
    pub manufacturer_model_name: String,
    pub device_serial_number: String,
    pub content_label: String,
    pub content_description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegistrationSourceInput {
    pub role: AdvancedSourceRole,
    pub artifact: PlannedDicomArtifact,
    pub bindings: ArtifactExecutionBindings,
    pub reference: MaterializedReference,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpatialRegistrationParameters {
    pub fixed_matrix: [String; 16],
    pub fixed_comment: String,
    pub moving_matrix: [String; 16],
    pub moving_comment: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeformableRegistrationParameters {
    pub image_position_patient: [String; 3],
    pub image_orientation_patient: [String; 6],
    pub grid_dimensions: [u32; 3],
    pub grid_resolution: [f64; 3],
    pub vector_grid_data: Vec<f32>,
    pub pre_deformation_matrix: [String; 16],
    pub post_deformation_matrix: [String; 16],
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum RegistrationKindInput {
    Spatial(SpatialRegistrationParameters),
    Deformable(DeformableRegistrationParameters),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegistrationProviderInput {
    pub common: RegistrationCommonInput,
    /// Exact order is fixed, then moving. This makes source semantics stable
    /// independently of map iteration or scheduler order.
    pub sources: Vec<RegistrationSourceInput>,
    pub registration: RegistrationKindInput,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RegistrationDocumentParameters {
    series_number: String,
    study_id: String,
    laterality: String,
    manufacturer_model_name: String,
    device_serial_number: String,
    content_label: String,
    content_description: String,
    registration: RegistrationKindInput,
    sources: Vec<RegistrationSourceDeclaration>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RegistrationSourceDeclaration {
    recipe: RecipeReference,
    artifact_logical_id: String,
    role: AdvancedSourceRole,
}

pub(crate) fn registration_input_from_recipe(
    recipe: &CaseRecipe,
    sources: Vec<RegistrationSourceInput>,
) -> Result<Option<RegistrationProviderInput>, String> {
    if recipe.plan_provider_id != REGISTRATION_PLAN_PROVIDER_ID {
        return Ok(None);
    }
    validate_registration_recipe(recipe)?;
    let parameters: RegistrationDocumentParameters =
        serde_json::from_value(Value::Object(recipe.provider_parameters.clone()))
            .map_err(|error| format!("registration provider_parameters: {error}"))?;
    let dicom = recipe
        .dicom
        .as_ref()
        .ok_or_else(|| "registration provider requires DICOM artifacts".to_string())?;
    let [target] = dicom.artifacts.as_slice() else {
        return Err("registration provider requires exactly one public artifact".into());
    };
    if !target.parameters.is_empty() {
        return Err("registration target stores static facts in provider_parameters".into());
    }
    let template_id = target
        .template
        .as_ref()
        .ok_or_else(|| "registration target requires a template".to_string())?
        .template_id
        .clone();
    let output_path = OutputRelativePath::new(
        target
            .output
            .path
            .as_ref()
            .ok_or_else(|| "registration target requires an exact output path".to_string())?
            .clone(),
    )
    .map_err(|error| error.to_string())?;
    let expected_template = match &parameters.registration {
        RegistrationKindInput::Spatial(_) => "derived/registration/spatial",
        RegistrationKindInput::Deformable(_) => "derived/registration/deformable",
    };
    if template_id != expected_template {
        return Err("registration template does not match registration kind".into());
    }
    if parameters.sources.len() != 2 || sources.len() != 2 {
        return Err("registration requires exactly two declared sources".into());
    }
    let declared_dependencies = recipe
        .dependencies
        .iter()
        .map(|dependency| dependency.recipe.identity())
        .collect::<BTreeSet<_>>();
    let mut remaining = sources;
    let mut ordered = Vec::with_capacity(2);
    for (index, declaration) in parameters.sources.iter().enumerate() {
        if !declared_dependencies.contains(&declaration.recipe.identity()) {
            return Err("registration source lacks an outer recipe dependency".into());
        }
        let position = remaining
            .iter()
            .position(|source| {
                source.artifact.logical_id == declaration.artifact_logical_id
                    && source
                        .artifact
                        .case_binding
                        .as_ref()
                        .is_some_and(|binding| {
                            binding.recipe_id == declaration.recipe.recipe_id
                                && binding.recipe_version == declaration.recipe.recipe_version
                        })
            })
            .ok_or_else(|| {
                format!(
                    "missing registration source {}",
                    declaration.artifact_logical_id
                )
            })?;
        let mut source = remaining.remove(position);
        if source.role != declaration.role {
            return Err(format!(
                "wrong role for registration source {}",
                declaration.artifact_logical_id
            ));
        }
        source.artifact.order = index as u64;
        ordered.push(source);
    }
    if !remaining.is_empty()
        || ordered[0].role != AdvancedSourceRole::RegistrationFixed
        || ordered[1].role != AdvancedSourceRole::RegistrationMoving
    {
        return Err("registration roles must be exactly fixed then moving".into());
    }
    Ok(Some(RegistrationProviderInput {
        common: RegistrationCommonInput {
            logical_id: target.logical_id.clone(),
            order: 2,
            output_path,
            template_id,
            series_number: parameters.series_number,
            study_id: parameters.study_id,
            laterality: parameters.laterality,
            manufacturer_model_name: parameters.manufacturer_model_name,
            device_serial_number: parameters.device_serial_number,
            content_label: parameters.content_label,
            content_description: parameters.content_description,
        },
        sources: ordered,
        registration: parameters.registration,
    }))
}

pub(crate) fn validate_registration_recipe(recipe: &CaseRecipe) -> Result<(), String> {
    let parameters: RegistrationDocumentParameters =
        serde_json::from_value(Value::Object(recipe.provider_parameters.clone()))
            .map_err(|error| format!("registration provider_parameters: {error}"))?;
    let dicom = recipe
        .dicom
        .as_ref()
        .ok_or_else(|| "registration provider requires DICOM artifacts".to_string())?;
    let [target] = dicom.artifacts.as_slice() else {
        return Err("registration provider requires exactly one public artifact".into());
    };
    if !target.parameters.is_empty() || parameters.sources.len() != 2 {
        return Err("registration requires one empty target and two sources".into());
    }
    let expected_roles = [
        AdvancedSourceRole::RegistrationFixed,
        AdvancedSourceRole::RegistrationMoving,
    ];
    let declared_dependencies = recipe
        .dependencies
        .iter()
        .map(|dependency| (dependency.recipe.identity(), dependency.role.as_str()))
        .collect::<BTreeMap<_, _>>();
    if declared_dependencies.len() != 2 {
        return Err("registration requires two unique outer dependencies".into());
    }
    let mut artifacts = BTreeSet::new();
    for (index, source) in parameters.sources.iter().enumerate() {
        if source.role != expected_roles[index]
            || !artifacts.insert((
                source.recipe.identity(),
                source.artifact_logical_id.as_str(),
            ))
        {
            return Err("registration roles must be unique fixed then moving".into());
        }
        let expected = match source.role {
            AdvancedSourceRole::RegistrationFixed => "registration_fixed",
            AdvancedSourceRole::RegistrationMoving => "registration_moving",
            _ => return Err("registration declares an incompatible source role".into()),
        };
        if declared_dependencies
            .get(&source.recipe.identity())
            .copied()
            != Some(expected)
        {
            return Err(
                "registration dependency role does not match its source declaration".into(),
            );
        }
    }
    Ok(())
}

#[derive(Debug, Clone)]
pub struct RegistrationPlanProvider {
    standards_lock_sha256: String,
}

impl RegistrationPlanProvider {
    pub fn new(standards_lock_sha256: impl Into<String>) -> Result<Self, RegistrationPlanError> {
        let standards_lock_sha256 = standards_lock_sha256.into();
        if standards_lock_sha256.len() != 64
            || !standards_lock_sha256
                .bytes()
                .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
        {
            return Err(RegistrationPlanError::InvalidStandardsLockHash);
        }
        Ok(Self {
            standards_lock_sha256,
        })
    }

    pub fn recipe_default_contexts(
        &self,
        input: &RegistrationProviderInput,
        case_id: &str,
        recipe: &crate::planning::RecipeIdentity,
        seed: u64,
    ) -> Result<Vec<AdvancedArtifactPlanningContext>, RegistrationPlanError> {
        let fixed = input
            .sources
            .first()
            .ok_or(RegistrationPlanError::SourceCardinality)?;
        let fixed_ids = source_identities(fixed)?;
        let uid = |role| {
            deterministic_uid(&DeterministicUidInput {
                standards_lock_sha256: &self.standards_lock_sha256,
                case_id,
                recipe_version: &recipe.recipe_version,
                run_seed: seed,
                file_index: 0,
                frame_index: None,
                referenced_object_index: None,
                role,
            })
        };
        let implementation = deterministic_uid(&DeterministicUidInput {
            standards_lock_sha256: &self.standards_lock_sha256,
            case_id: "dicom-test-suite/implementation",
            recipe_version: PACKAGE_VERSION,
            run_seed: 0,
            file_index: 0,
            frame_index: None,
            referenced_object_index: None,
            role: UidRole::ImplementationClass,
        });
        let target = input.common.logical_id.clone();
        Ok(vec![AdvancedArtifactPlanningContext {
            recipe_artifact_logical_id: input.common.logical_id.clone(),
            target_instance_id: target.clone(),
            order: input.common.order,
            output: OutputPlan {
                relative_path: input.common.output_path.clone(),
                role: "dicom_instance".into(),
                publish: true,
            },
            identities: IdentityPlan::from_exact_values(
                target,
                [
                    (CompositionUidRole::StudyInstance, 0, fixed_ids.study),
                    (
                        CompositionUidRole::SeriesInstance,
                        0,
                        uid(UidRole::SeriesInstance),
                    ),
                    (
                        CompositionUidRole::SopInstance,
                        0,
                        uid(UidRole::SopInstance),
                    ),
                    (
                        CompositionUidRole::FrameOfReference,
                        0,
                        fixed_ids.frame_of_reference,
                    ),
                    (CompositionUidRole::ImplementationClass, 0, implementation),
                ],
            )
            .map_err(|error| RegistrationPlanError::Identity(error.to_string()))?,
        }])
    }

    pub fn plan_typed(
        &self,
        request: &AdvancedPlanProviderRequest,
        input: &RegistrationProviderInput,
    ) -> Result<AdvancedPlanProviderOutput, RegistrationPlanError> {
        request
            .validate()
            .map_err(RegistrationPlanError::Contract)?;
        if request.family != AdvancedProviderFamily::Registration
            || request.provider_id != REGISTRATION_PLAN_PROVIDER_ID
        {
            return Err(RegistrationPlanError::WrongProvider);
        }
        let [fixed, moving] = input.sources.as_slice() else {
            return Err(RegistrationPlanError::SourceCardinality);
        };
        validate_source(
            fixed,
            AdvancedSourceRole::RegistrationFixed,
            ENHANCED_CT_SOP,
        )?;
        validate_source(
            moving,
            AdvancedSourceRole::RegistrationMoving,
            CLASSIC_CT_SOP,
        )?;
        if fixed.artifact.order >= moving.artifact.order
            || moving.artifact.order >= input.common.order
        {
            return Err(RegistrationPlanError::SourceOrder);
        }
        if fixed.artifact.logical_id == moving.artifact.logical_id
            || fixed.reference.referenced_sop_instance_uid
                == moving.reference.referenced_sop_instance_uid
        {
            return Err(RegistrationPlanError::DuplicateSource);
        }

        let fixed_ids = source_identities(fixed)?;
        let moving_ids = source_identities(moving)?;
        if fixed_ids.study == moving_ids.study
            || fixed_ids.frame_of_reference == moving_ids.frame_of_reference
        {
            return Err(RegistrationPlanError::SourceIdentityCollision);
        }
        let context = request
            .artifact_context(&input.common.logical_id)
            .map_err(RegistrationPlanError::Contract)?;
        for source in [fixed, moving] {
            if source.reference.source_instance_id != context.target_instance_id
                || source.reference.target_instance_id != source.artifact.instance.instance_id
                || !source.reference.referenced_frames.is_empty()
            {
                return Err(RegistrationPlanError::InvalidReferenceOwnership);
            }
        }

        let ids = RegistrationUids::from_context(context)?;
        if ids.study != fixed_ids.study || ids.frame_of_reference != fixed_ids.frame_of_reference {
            return Err(RegistrationPlanError::SourceIdentityCollision);
        }
        let target = planned_registration(request, input, context, &ids, &fixed_ids, &moving_ids)?;
        let target_id = target.logical_id.clone();

        let mut artifacts = Vec::with_capacity(3);
        let mut bindings = Vec::with_capacity(3);
        let mut references = Vec::with_capacity(2);
        let mut dependencies = Vec::with_capacity(2);
        for source in [fixed, moving] {
            let mut planned = source.artifact.clone();
            planned.provenance = ArtifactProvenance::PrivateSource {
                consumed_by: vec![target_id.clone()],
            };
            planned.output.publish = false;
            let consumer = AdvancedSourceConsumer {
                artifact_id: target_id.clone(),
                role: source.role.clone(),
            };
            artifacts.push(AdvancedPlannedArtifact {
                role: AdvancedArtifactRole::Registration {
                    ordinal: artifacts.len() as u32 + 1,
                },
                planned,
                provenance: AdvancedArtifactProvenance::PrivateSource {
                    consumed_by: vec![consumer],
                },
            });
            bindings.push(source.bindings.clone());
            dependencies.push(ArtifactDependency {
                artifact_id: target_id.clone(),
                depends_on: source.artifact.logical_id.clone(),
                relationship: source.role.dependency_relationship().into(),
                frame_numbers: vec![],
            });
            references.push(AdvancedSourceReference {
                owner_artifact_id: target_id.clone(),
                source_artifact_id: source.artifact.logical_id.clone(),
                source_role: source.role.clone(),
                reference: source.reference.clone(),
            });
        }
        artifacts.push(AdvancedPlannedArtifact {
            role: AdvancedArtifactRole::Registration { ordinal: 3 },
            planned: target,
            provenance: AdvancedArtifactProvenance::Requested,
        });
        bindings.push(ArtifactExecutionBindings {
            artifact_id: target_id,
            slots: BTreeMap::new(),
        });
        let output = AdvancedPlanProviderOutput {
            artifacts,
            dependencies,
            references,
            bindings,
        };
        output
            .validate(request)
            .map_err(RegistrationPlanError::Contract)?;
        Ok(output)
    }
}

impl AdvancedPlanProvider for RegistrationPlanProvider {
    type ProviderInput = RegistrationProviderInput;

    fn provider_id(&self) -> &str {
        REGISTRATION_PLAN_PROVIDER_ID
    }

    fn plan(
        &self,
        request: &AdvancedPlanProviderRequest,
        input: &Self::ProviderInput,
    ) -> Result<AdvancedPlanProviderOutput, AdvancedProviderContractError> {
        self.plan_typed(request, input).map_err(|error| {
            AdvancedProviderContractError::InvalidProviderOutput(error.to_string())
        })
    }
}

struct SourceIdentities {
    study: String,
    series: String,
    sop_class: String,
    sop: String,
    frame_of_reference: String,
}

fn validate_source(
    source: &RegistrationSourceInput,
    expected_role: AdvancedSourceRole,
    expected_sop: &str,
) -> Result<(), RegistrationPlanError> {
    if source.role != expected_role {
        return Err(RegistrationPlanError::SourceRole);
    }
    if source.artifact.instance.sop_class_uid != expected_sop
        || source.reference.referenced_sop_class_uid != expected_sop
    {
        return Err(RegistrationPlanError::SourceSopClass);
    }
    if source.bindings.artifact_id != source.artifact.logical_id {
        return Err(RegistrationPlanError::SourceBinding);
    }
    let expected_slots = source
        .artifact
        .instance
        .content
        .iter()
        .map(|value| value.slot.as_str())
        .collect::<BTreeSet<_>>();
    let bound_slots = source.bindings.slots.keys().map(String::as_str).collect();
    if expected_slots != bound_slots {
        return Err(RegistrationPlanError::SourceBinding);
    }
    let ids = source_identities(source)?;
    if ids.sop
        != source
            .artifact
            .instance
            .identities
            .get(&CompositionUidRole::SopInstance, 0)
            .unwrap_or_default()
        || ids.sop != source.reference.referenced_sop_instance_uid
    {
        return Err(RegistrationPlanError::SourceIdentityMismatch);
    }
    Ok(())
}

fn source_identities(
    source: &RegistrationSourceInput,
) -> Result<SourceIdentities, RegistrationPlanError> {
    let identity = &source.artifact.instance.identities;
    let get = |role| {
        identity
            .get(&role, 0)
            .map(str::to_owned)
            .ok_or(RegistrationPlanError::MissingSourceIdentity)
    };
    Ok(SourceIdentities {
        study: get(CompositionUidRole::StudyInstance)?,
        series: get(CompositionUidRole::SeriesInstance)?,
        sop_class: source.artifact.instance.sop_class_uid.clone(),
        sop: get(CompositionUidRole::SopInstance)?,
        frame_of_reference: get(CompositionUidRole::FrameOfReference)?,
    })
}

struct RegistrationUids {
    study: String,
    series: String,
    sop: String,
    frame_of_reference: String,
    implementation: String,
}

impl RegistrationUids {
    fn from_context(
        context: &AdvancedArtifactPlanningContext,
    ) -> Result<Self, RegistrationPlanError> {
        let get = |role| {
            context
                .identities
                .get(&role, 0)
                .map(str::to_owned)
                .ok_or_else(|| {
                    RegistrationPlanError::Identity(format!("missing {}", role.as_str()))
                })
        };
        Ok(Self {
            study: get(CompositionUidRole::StudyInstance)?,
            series: get(CompositionUidRole::SeriesInstance)?,
            sop: get(CompositionUidRole::SopInstance)?,
            frame_of_reference: get(CompositionUidRole::FrameOfReference)?,
            implementation: get(CompositionUidRole::ImplementationClass)?,
        })
    }
}

fn planned_registration(
    request: &AdvancedPlanProviderRequest,
    input: &RegistrationProviderInput,
    context: &AdvancedArtifactPlanningContext,
    ids: &RegistrationUids,
    fixed: &SourceIdentities,
    moving: &SourceIdentities,
) -> Result<PlannedDicomArtifact, RegistrationPlanError> {
    let (sop_class_uid, attributes) = match &input.registration {
        RegistrationKindInput::Spatial(parameters) => (
            SPATIAL_REGISTRATION_SOP,
            spatial_attributes(&input.common, ids, fixed, moving, parameters)?,
        ),
        RegistrationKindInput::Deformable(parameters) => (
            DEFORMABLE_REGISTRATION_SOP,
            deformable_attributes(&input.common, ids, fixed, moving, parameters)?,
        ),
    };
    Ok(PlannedDicomArtifact {
        logical_id: context.target_instance_id.clone(),
        order: context.order,
        provenance: ArtifactProvenance::Requested,
        case_binding: Some(CaseBinding {
            case_id: request.case_id.clone(),
            recipe_id: request.recipe.recipe_id.clone(),
            recipe_version: request.recipe.recipe_version.clone(),
        }),
        instance: ResolvedInstancePlan {
            plan_schema_version: "0.1.0".into(),
            instance_id: context.target_instance_id.clone(),
            template_id: TemplateId(input.common.template_id.clone()),
            template_version: "1.0.0"
                .parse::<TemplateVersion>()
                .map_err(|error| RegistrationPlanError::Template(error.to_string()))?,
            sop_class_uid: sop_class_uid.into(),
            transfer_syntax_uid: EXPLICIT_VR_LE.into(),
            identities: context.identities.clone(),
            attributes,
            content: vec![],
            references: input
                .sources
                .iter()
                .map(|source| source.reference.clone())
                .collect(),
        },
        output: context.output.clone(),
        encoding: EncodingPlan {
            transfer_syntax_uid: EXPLICIT_VR_LE.into(),
            sequence_length: SequenceLengthPolicy::WriterDefault,
            item_length: ItemLengthPolicy::WriterDefault,
            fragmentation: FragmentationPolicy::Native,
            offset_table: OffsetTablePolicy::NotApplicable,
            preamble: PreamblePolicy::ZeroFilled,
            file_meta: FileMetaPolicy::Standard,
            implementation: ImplementationIdentityPlan {
                class_uid: ids.implementation.clone(),
                version_name: Some(IMPLEMENTATION_VERSION_NAME.into()),
            },
            backend_id: "dicom-rs.part10".into(),
        },
        validation: ValidationPlan {
            rules: vec![ValidationRule {
                rule_id: "registration.reference_graph".into(),
                requirement: ValidationRequirement::Required,
                parameters: BTreeMap::new(),
            }],
        },
        evidence: EvidencePlan {
            obligations: vec![EvidenceObligation {
                obligation_id: format!("same-project:{}", input.common.logical_id),
                route_id: "builtin.strict".into(),
                independence: EvidenceIndependence::SameProject,
                required: true,
                parameters: BTreeMap::new(),
            }],
        },
        resources: ArtifactResourceEstimate {
            output_bytes: 256 * 1024,
            peak_working_bytes: 512 * 1024,
        },
    })
}

fn common_attributes(
    common: &RegistrationCommonInput,
    ids: &RegistrationUids,
    sop_class_uid: &str,
) -> Result<Vec<ResolvedAttribute>, RegistrationPlanError> {
    [
        ("SOPClassUID", DicomVr::UI, sop_class_uid),
        ("SOPInstanceUID", DicomVr::UI, &ids.sop),
        ("SyntheticData", DicomVr::CS, "YES"),
        ("PatientName", DicomVr::PN, "DTS^Synthetic^Patient001"),
        ("PatientID", DicomVr::LO, "DTS-PATIENT-001"),
        ("PatientBirthDate", DicomVr::DA, "19700101"),
        ("PatientSex", DicomVr::CS, "O"),
        ("StudyInstanceUID", DicomVr::UI, &ids.study),
        ("StudyDate", DicomVr::DA, "20260101"),
        ("StudyTime", DicomVr::TM, "000000"),
        ("ReferringPhysicianName", DicomVr::PN, ""),
        ("StudyID", DicomVr::SH, &common.study_id),
        ("AccessionNumber", DicomVr::SH, ""),
        ("Modality", DicomVr::CS, "REG"),
        ("SeriesInstanceUID", DicomVr::UI, &ids.series),
        ("SeriesNumber", DicomVr::IS, &common.series_number),
        ("Laterality", DicomVr::CS, &common.laterality),
        ("FrameOfReferenceUID", DicomVr::UI, &ids.frame_of_reference),
        ("PositionReferenceIndicator", DicomVr::LO, ""),
        ("Manufacturer", DicomVr::LO, "dicom-test-suite"),
        ("InstitutionName", DicomVr::LO, ""),
        ("InstitutionAddress", DicomVr::ST, ""),
        (
            "ManufacturerModelName",
            DicomVr::LO,
            &common.manufacturer_model_name,
        ),
        (
            "DeviceSerialNumber",
            DicomVr::LO,
            &common.device_serial_number,
        ),
        ("SoftwareVersions", DicomVr::LO, PACKAGE_VERSION),
        ("InstanceNumber", DicomVr::IS, "1"),
        ("ContentDate", DicomVr::DA, "20260101"),
        ("ContentTime", DicomVr::TM, "000000"),
        ("ContentLabel", DicomVr::CS, &common.content_label),
        (
            "ContentDescription",
            DicomVr::LO,
            &common.content_description,
        ),
        ("ContentCreatorName", DicomVr::PN, "DTS^Generator"),
    ]
    .into_iter()
    .map(|(keyword, vr, value)| resolved_text(keyword, vr, value))
    .collect()
}

fn spatial_attributes(
    common: &RegistrationCommonInput,
    ids: &RegistrationUids,
    fixed: &SourceIdentities,
    moving: &SourceIdentities,
    parameters: &SpatialRegistrationParameters,
) -> Result<Vec<ResolvedAttribute>, RegistrationPlanError> {
    let mut attributes = common_attributes(common, ids, SPATIAL_REGISTRATION_SOP)?;
    attributes.push(resolved(sequence(
        "RegistrationSequence",
        vec![
            registration_item(fixed, &parameters.fixed_matrix, &parameters.fixed_comment)?,
            registration_item(
                moving,
                &parameters.moving_matrix,
                &parameters.moving_comment,
            )?,
        ],
    )?));
    push_reference_partitions(&mut attributes, fixed, moving)?;
    Ok(attributes)
}

fn deformable_attributes(
    common: &RegistrationCommonInput,
    ids: &RegistrationUids,
    fixed: &SourceIdentities,
    moving: &SourceIdentities,
    parameters: &DeformableRegistrationParameters,
) -> Result<Vec<ResolvedAttribute>, RegistrationPlanError> {
    let expected_vectors = parameters
        .grid_dimensions
        .iter()
        .try_fold(3_u64, |count, value| {
            count
                .checked_mul(u64::from(*value))
                .ok_or(RegistrationPlanError::ResourceOverflow)
        })?;
    if expected_vectors != parameters.vector_grid_data.len() as u64
        || parameters.grid_dimensions.contains(&0)
        || parameters
            .grid_resolution
            .iter()
            .any(|value| !value.is_finite() || *value <= 0.0)
        || parameters
            .vector_grid_data
            .iter()
            .any(|value| !value.is_finite())
    {
        return Err(RegistrationPlanError::InvalidDeformationGrid);
    }
    let mut attributes = common_attributes(common, ids, DEFORMABLE_REGISTRATION_SOP)?;
    let grid = vec![
        text(
            "ImagePositionPatient",
            DicomVr::DS,
            &parameters.image_position_patient.join("\\"),
        )?,
        text(
            "ImageOrientationPatient",
            DicomVr::DS,
            &parameters.image_orientation_patient.join("\\"),
        )?,
        unsigned_multi("GridDimensions", DicomVr::UL, &parameters.grid_dimensions)?,
        float64_multi("GridResolution", &parameters.grid_resolution)?,
        float32_binary("VectorGridData", &parameters.vector_grid_data)?,
    ];
    let source_item = vec![
        sequence(
            "ReferencedImageSequence",
            vec![referenced_sop_item(moving)?],
        )?,
        text(
            "SourceFrameOfReferenceUID",
            DicomVr::UI,
            &moving.frame_of_reference,
        )?,
        sequence("DeformableRegistrationGridSequence", vec![grid])?,
        sequence(
            "PreDeformationMatrixRegistrationSequence",
            vec![identity_matrix_item(&parameters.pre_deformation_matrix)?],
        )?,
        sequence(
            "PostDeformationMatrixRegistrationSequence",
            vec![identity_matrix_item(&parameters.post_deformation_matrix)?],
        )?,
        sequence("RegistrationTypeCodeSequence", vec![])?,
    ];
    attributes.push(resolved(sequence(
        "DeformableRegistrationSequence",
        vec![source_item],
    )?));
    push_reference_partitions(&mut attributes, fixed, moving)?;
    Ok(attributes)
}

fn registration_item(
    source: &SourceIdentities,
    matrix: &[String; 16],
    comment: &str,
) -> Result<Vec<AttributeOperation>, RegistrationPlanError> {
    Ok(vec![
        sequence(
            "ReferencedImageSequence",
            vec![referenced_sop_item(source)?],
        )?,
        text(
            "FrameOfReferenceUID",
            DicomVr::UI,
            &source.frame_of_reference,
        )?,
        sequence(
            "MatrixRegistrationSequence",
            vec![vec![
                sequence("RegistrationTypeCodeSequence", vec![])?,
                sequence("MatrixSequence", vec![identity_matrix_item(matrix)?])?,
                text(
                    "FrameOfReferenceTransformationComment",
                    DicomVr::LO,
                    comment,
                )?,
            ]],
        )?,
    ])
}

fn identity_matrix_item(
    matrix: &[String; 16],
) -> Result<Vec<AttributeOperation>, RegistrationPlanError> {
    if matrix.iter().any(String::is_empty) {
        return Err(RegistrationPlanError::InvalidMatrix);
    }
    Ok(vec![
        text(
            "FrameOfReferenceTransformationMatrixType",
            DicomVr::CS,
            "RIGID",
        )?,
        text(
            "FrameOfReferenceTransformationMatrix",
            DicomVr::DS,
            &matrix.join("\\"),
        )?,
    ])
}

fn push_reference_partitions(
    attributes: &mut Vec<ResolvedAttribute>,
    fixed: &SourceIdentities,
    moving: &SourceIdentities,
) -> Result<(), RegistrationPlanError> {
    attributes.push(resolved(sequence(
        "ReferencedSeriesSequence",
        vec![referenced_series_item(fixed)?],
    )?));
    attributes.push(resolved(sequence(
        "StudiesContainingOtherReferencedInstancesSequence",
        vec![vec![
            text("StudyInstanceUID", DicomVr::UI, &moving.study)?,
            sequence(
                "ReferencedSeriesSequence",
                vec![referenced_series_item(moving)?],
            )?,
        ]],
    )?));
    Ok(())
}

fn referenced_series_item(
    source: &SourceIdentities,
) -> Result<Vec<AttributeOperation>, RegistrationPlanError> {
    Ok(vec![
        sequence(
            "ReferencedInstanceSequence",
            vec![referenced_sop_item(source)?],
        )?,
        text("SeriesInstanceUID", DicomVr::UI, &source.series)?,
    ])
}

fn referenced_sop_item(
    source: &SourceIdentities,
) -> Result<Vec<AttributeOperation>, RegistrationPlanError> {
    Ok(vec![
        text("ReferencedSOPClassUID", DicomVr::UI, &source.sop_class)?,
        text("ReferencedSOPInstanceUID", DicomVr::UI, &source.sop)?,
    ])
}

fn resolved_text(
    keyword: &str,
    vr: DicomVr,
    value: &str,
) -> Result<ResolvedAttribute, RegistrationPlanError> {
    Ok(resolved(text(keyword, vr, value)?))
}

fn resolved(operation: AttributeOperation) -> ResolvedAttribute {
    let AttributeOperation::Set { address, vr, value } = operation else {
        unreachable!()
    };
    ResolvedAttribute {
        address,
        vr,
        value: Some(value),
        origin: ValueOrigin::InstanceOverride,
    }
}

fn text(
    keyword: &str,
    vr: DicomVr,
    value: &str,
) -> Result<AttributeOperation, RegistrationPlanError> {
    let values = value
        .split('\\')
        .map(|value| PrimitiveValue::String(value.into()))
        .collect::<Vec<_>>();
    operation(
        keyword,
        vr,
        if values.len() == 1 {
            AttributeValue::Primitive(values.into_iter().next().unwrap())
        } else {
            AttributeValue::Multi(values)
        },
    )
}

fn unsigned_multi(
    keyword: &str,
    vr: DicomVr,
    values: &[u32],
) -> Result<AttributeOperation, RegistrationPlanError> {
    operation(
        keyword,
        vr,
        AttributeValue::Multi(
            values
                .iter()
                .map(|value| PrimitiveValue::Unsigned(u64::from(*value)))
                .collect(),
        ),
    )
}

fn float64_multi(
    keyword: &str,
    values: &[f64],
) -> Result<AttributeOperation, RegistrationPlanError> {
    operation(
        keyword,
        DicomVr::FD,
        AttributeValue::Multi(
            values
                .iter()
                .map(|value| PrimitiveValue::Float64Bits(value.to_bits()))
                .collect(),
        ),
    )
}

fn float32_binary(
    keyword: &str,
    values: &[f32],
) -> Result<AttributeOperation, RegistrationPlanError> {
    let bytes = values
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect();
    operation(keyword, DicomVr::OF, AttributeValue::Binary(bytes))
}

fn sequence(
    keyword: &str,
    items: Vec<Vec<AttributeOperation>>,
) -> Result<AttributeOperation, RegistrationPlanError> {
    operation(
        keyword,
        DicomVr::SQ,
        AttributeValue::Sequence(
            items
                .into_iter()
                .map(|attributes| AttributeItem { attributes })
                .collect(),
        ),
    )
}

fn operation(
    keyword: &str,
    vr: DicomVr,
    value: AttributeValue,
) -> Result<AttributeOperation, RegistrationPlanError> {
    Ok(AttributeOperation::Set {
        address: AttributeAddress::from_keyword(keyword)
            .map_err(|error| RegistrationPlanError::Attribute(error.to_string()))?,
        vr,
        value,
    })
}

#[derive(Debug)]
pub enum RegistrationPlanError {
    InvalidStandardsLockHash,
    WrongProvider,
    SourceCardinality,
    SourceRole,
    SourceOrder,
    DuplicateSource,
    SourceSopClass,
    SourceBinding,
    MissingSourceIdentity,
    SourceIdentityMismatch,
    SourceIdentityCollision,
    InvalidReferenceOwnership,
    InvalidMatrix,
    InvalidDeformationGrid,
    ResourceOverflow,
    Attribute(String),
    Identity(String),
    Template(String),
    Contract(AdvancedProviderContractError),
}

impl fmt::Display for RegistrationPlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidStandardsLockHash => {
                formatter.write_str("standards lock hash is not lowercase SHA-256")
            }
            Self::WrongProvider => {
                formatter.write_str("request is not for the registration provider")
            }
            Self::SourceCardinality => {
                formatter.write_str("registration requires exactly fixed and moving sources")
            }
            Self::SourceRole => {
                formatter.write_str("registration sources are missing, duplicated, or reordered")
            }
            Self::SourceOrder => {
                formatter.write_str("source and registration artifact order is invalid")
            }
            Self::DuplicateSource => formatter.write_str("registration sources are not distinct"),
            Self::SourceSopClass => {
                formatter.write_str("registration source has the wrong SOP Class")
            }
            Self::SourceBinding => {
                formatter.write_str("registration source execution binding is inconsistent")
            }
            Self::MissingSourceIdentity => {
                formatter.write_str("registration source identity plan is incomplete")
            }
            Self::SourceIdentityMismatch => {
                formatter.write_str("registration source identity and reference differ")
            }
            Self::SourceIdentityCollision => formatter.write_str(
                "fixed and moving sources must have distinct study and frame identities",
            ),
            Self::InvalidReferenceOwnership => {
                formatter.write_str("registration reference ownership or frame scope is invalid")
            }
            Self::InvalidMatrix => formatter.write_str("registration matrix is invalid"),
            Self::InvalidDeformationGrid => {
                formatter.write_str("deformation grid cardinality or values are invalid")
            }
            Self::ResourceOverflow => {
                formatter.write_str("registration resource arithmetic overflow")
            }
            Self::Attribute(error) => {
                write!(formatter, "registration attribute is invalid: {error}")
            }
            Self::Identity(error) => write!(formatter, "registration identity is invalid: {error}"),
            Self::Template(error) => write!(formatter, "registration template is invalid: {error}"),
            Self::Contract(error) => {
                write!(formatter, "registration provider contract failed: {error}")
            }
        }
    }
}

impl Error for RegistrationPlanError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Contract(error) => Some(error),
            _ => None,
        }
    }
}
