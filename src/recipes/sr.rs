//! Typed, plan-first Structured Report recipes.

use std::collections::BTreeSet;
use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::composition::{
    AttributeAddress, AttributeItem, AttributeOperation, AttributeValue, DicomVr, PrimitiveValue,
};

use super::semantic::{
    SemanticPlanContext, SemanticPlanError, SemanticPlanOutput, build_semantic_plan,
};
use super::{
    CaseRecipe, CodedConcept, CompletionFlag, ContentProviderLimits, ContentProviderRequest,
    NeutralContentProvider, RecipeReference, SemanticReference, SemanticReferenceRole,
    StructuredReportContract, VerificationFlag,
};

pub const SR_PLAN_PROVIDER_ID: &str = "native.sr_plan";
pub const HIGH_DICOM_SR_IMPORT_PROVIDER_ID: &str = "external.highdicom_sr_import_plan";
pub const SR_CONTENT_PROVIDER_ID: &str = "content.sr_semantics";
pub const SR_ALGORITHM_PROVIDER_ID: &str = "algorithm.sr_content_tree";
pub const HIGH_DICOM_SR_CONTENT_PROVIDER_ID: &str = "content.external_import";
pub const HIGH_DICOM_SR_ALGORITHM_PROVIDER_ID: &str = "algorithm.highdicom_sr";

const BASIC_TEXT_SR_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.88.11";
const COMPREHENSIVE_SR_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.88.33";
const KEY_OBJECT_SELECTION_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.88.59";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SrSourceDeclaration {
    pub recipe: RecipeReference,
    pub artifact_logical_id: String,
    pub role: String,
    #[serde(default)]
    pub referenced_frames: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum SrDocumentKind {
    BasicText {
        observation: CodedConcept,
        observation_text: String,
    },
    Comprehensive {
        measurement: CodedConcept,
        numeric_value: String,
        units: CodedConcept,
        image_concept: CodedConcept,
    },
    KeyObjectSelection {
        mapping_resource: String,
        template_identifier: String,
    },
    Comprehensive3d {
        graphic_type: String,
        graphic_data: Vec<String>,
        fiducial_uid: String,
        import: HighDicomSrBoundary,
    },
    Tid1500 {
        procedure_reported: CodedConcept,
        finding: CodedConcept,
        measurement: CodedConcept,
        numeric_value: String,
        units: CodedConcept,
        tracking_identifier: String,
        tracking_uid: String,
        import: HighDicomSrBoundary,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HighDicomSrBoundary {
    pub provider_id: String,
    pub dependency: String,
    pub required_version: String,
    pub dependency_sha256: String,
    pub tool_fingerprint_policy: String,
    pub request_schema_version: String,
    pub response_schema_version: String,
    pub output_media_type: String,
    pub maximum_output_bytes: u64,
    pub determinism: String,
    pub semantic_evidence: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SrDocumentParameters {
    pub series_number: String,
    pub instance_number: u32,
    pub content_date: String,
    pub content_time: String,
    pub completion_flag: CompletionFlag,
    pub verification_flag: VerificationFlag,
    pub continuity_of_content: String,
    pub title: CodedConcept,
    pub document: SrDocumentKind,
    pub sources: Vec<SrSourceDeclaration>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SrPlanInput {
    pub parameters: SrDocumentParameters,
    pub context: SemanticPlanContext,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExternalSrImportRequest {
    pub request_id: String,
    pub artifact_id: String,
    pub provider_id: String,
    pub dependency: String,
    pub required_version: String,
    pub dependency_sha256: String,
    pub tool_fingerprint_policy: String,
    pub request_schema_version: String,
    pub response_schema_version: String,
    pub output_media_type: String,
    pub maximum_output_bytes: u64,
    pub determinism: String,
    pub semantic_evidence: Vec<String>,
    pub source_artifact_ids: Vec<String>,
    pub parameters_sha256: String,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct SrPlanProvider;

impl SrPlanProvider {
    pub fn plan_native(&self, input: &SrPlanInput) -> Result<SemanticPlanOutput, SrPlanError> {
        validate_input(input, false)?;
        let sop_class = match input.parameters.document {
            SrDocumentKind::BasicText { .. } => BASIC_TEXT_SR_STORAGE,
            SrDocumentKind::Comprehensive { .. } => COMPREHENSIVE_SR_STORAGE,
            SrDocumentKind::KeyObjectSelection { .. } => KEY_OBJECT_SELECTION_STORAGE,
            SrDocumentKind::Comprehensive3d { .. } | SrDocumentKind::Tid1500 { .. } => {
                return Err(SrPlanError::ExternalDocumentRequiresImport);
            }
        };
        let references = input
            .context
            .sources
            .iter()
            .map(|source| SemanticReference {
                role: SemanticReferenceRole::SourceImage,
                sop_class_uid: source.reference.referenced_sop_class_uid.clone(),
                sop_instance_uid: source.reference.referenced_sop_instance_uid.clone(),
                frames: source.reference.referenced_frames.clone(),
            })
            .collect();
        let mut operations = NeutralContentProvider
            .expand(
                &ContentProviderRequest::StructuredReport(StructuredReportContract {
                    content_date: input.parameters.content_date.clone(),
                    content_time: input.parameters.content_time.clone(),
                    completion_flag: input.parameters.completion_flag,
                    verification_flag: input.parameters.verification_flag,
                    concept_name: input.parameters.title.clone(),
                    references,
                }),
                ContentProviderLimits::default(),
            )
            .map_err(|error| SrPlanError::Content(error.to_string()))?
            .attribute_operations;
        operations.extend(common_operations(&input.parameters)?);
        operations.push(content_tree(&input.parameters, &input.context)?);
        build_semantic_plan(
            &input.context,
            sop_class,
            operations,
            vec![],
            &["sr.semantic_tree", "sr.reference_graph"],
            "builtin.strict_sr",
        )
        .map_err(SrPlanError::Plan)
    }

    pub fn external_import(
        &self,
        input: &SrPlanInput,
    ) -> Result<ExternalSrImportRequest, SrPlanError> {
        validate_input(input, true)?;
        let boundary = match &input.parameters.document {
            SrDocumentKind::Comprehensive3d { import, .. }
            | SrDocumentKind::Tid1500 { import, .. } => import,
            _ => return Err(SrPlanError::NativeDocumentCannotImport),
        };
        validate_boundary(boundary)?;
        let parameter_bytes = serde_json::to_vec(&input.parameters)
            .map_err(|error| SrPlanError::Serialize(error.to_string()))?;
        Ok(ExternalSrImportRequest {
            request_id: format!("{}-highdicom", input.context.logical_id),
            artifact_id: input.context.logical_id.clone(),
            provider_id: boundary.provider_id.clone(),
            dependency: boundary.dependency.clone(),
            required_version: boundary.required_version.clone(),
            dependency_sha256: boundary.dependency_sha256.clone(),
            tool_fingerprint_policy: boundary.tool_fingerprint_policy.clone(),
            request_schema_version: boundary.request_schema_version.clone(),
            response_schema_version: boundary.response_schema_version.clone(),
            output_media_type: boundary.output_media_type.clone(),
            maximum_output_bytes: boundary.maximum_output_bytes,
            determinism: boundary.determinism.clone(),
            semantic_evidence: boundary.semantic_evidence.clone(),
            source_artifact_ids: input
                .context
                .sources
                .iter()
                .map(|source| source.artifact_id.clone())
                .collect(),
            parameters_sha256: crate::sha256_hex(&parameter_bytes),
        })
    }
}

pub fn sr_input_from_recipe(
    recipe: &CaseRecipe,
    context: SemanticPlanContext,
) -> Result<Option<SrPlanInput>, SrPlanError> {
    if !matches!(
        recipe.plan_provider_id.as_str(),
        SR_PLAN_PROVIDER_ID | HIGH_DICOM_SR_IMPORT_PROVIDER_ID
    ) {
        return Ok(None);
    }
    let parameters = serde_json::from_value::<SrDocumentParameters>(Value::Object(
        recipe.provider_parameters.clone(),
    ))
    .map_err(|error| SrPlanError::Recipe(error.to_string()))?;
    let dicom = recipe.dicom.as_ref().ok_or(SrPlanError::RecipeShape)?;
    let [artifact] = dicom.artifacts.as_slice() else {
        return Err(SrPlanError::RecipeShape);
    };
    if artifact.logical_id != context.logical_id
        || artifact.order != 0
        || artifact
            .output
            .path
            .as_deref()
            .is_none_or(|path| path != context.output.relative_path.to_string())
        || artifact
            .template
            .as_ref()
            .is_none_or(|template| template.template_id != context.template_id)
        || artifact.content.provider_id
            != if recipe.plan_provider_id == SR_PLAN_PROVIDER_ID {
                SR_CONTENT_PROVIDER_ID
            } else {
                HIGH_DICOM_SR_CONTENT_PROVIDER_ID
            }
        || artifact.algorithm_provider_id.as_deref()
            != Some(if recipe.plan_provider_id == SR_PLAN_PROVIDER_ID {
                SR_ALGORITHM_PROVIDER_ID
            } else {
                HIGH_DICOM_SR_ALGORITHM_PROVIDER_ID
            })
    {
        return Err(SrPlanError::RecipeShape);
    }
    Ok(Some(SrPlanInput {
        parameters,
        context,
    }))
}

fn validate_input(input: &SrPlanInput, external: bool) -> Result<(), SrPlanError> {
    if input.parameters.series_number.is_empty()
        || input.parameters.instance_number == 0
        || input.parameters.continuity_of_content != "SEPARATE"
        || input.parameters.sources.is_empty()
    {
        return Err(SrPlanError::InvalidParameters);
    }
    let declared = input
        .parameters
        .sources
        .iter()
        .map(|source| {
            (
                &source.recipe,
                source.artifact_logical_id.as_str(),
                source.role.as_str(),
            )
        })
        .collect::<BTreeSet<_>>();
    let actual = input
        .context
        .sources
        .iter()
        .map(|source| {
            (
                RecipeReference {
                    recipe_id: source.recipe.recipe_id.clone(),
                    recipe_version: source.recipe.recipe_version.clone(),
                },
                source.recipe_artifact_logical_id.as_str(),
                source.role.as_str(),
            )
        })
        .collect::<BTreeSet<_>>();
    let actual = actual
        .iter()
        .map(|(recipe, artifact, role)| (recipe, *artifact, *role))
        .collect::<BTreeSet<_>>();
    if declared != actual {
        return Err(SrPlanError::SourceGraph);
    }
    for declaration in &input.parameters.sources {
        let source = input
            .context
            .sources
            .iter()
            .find(|source| source.recipe_artifact_logical_id == declaration.artifact_logical_id)
            .ok_or(SrPlanError::SourceGraph)?;
        if source.reference.referenced_frames != declaration.referenced_frames {
            return Err(SrPlanError::SourceGraph);
        }
    }
    let is_external = matches!(
        input.parameters.document,
        SrDocumentKind::Comprehensive3d { .. } | SrDocumentKind::Tid1500 { .. }
    );
    if external != is_external {
        return Err(SrPlanError::WrongRoute);
    }
    Ok(())
}

fn validate_boundary(boundary: &HighDicomSrBoundary) -> Result<(), SrPlanError> {
    if boundary.provider_id != "highdicom_pydicom"
        || boundary.dependency != "highdicom"
        || boundary.required_version.is_empty()
        || !is_sha256(&boundary.dependency_sha256)
        || boundary.tool_fingerprint_policy != "runtime_composite_sha256_required"
        || boundary.request_schema_version.is_empty()
        || boundary.response_schema_version.is_empty()
        || boundary.output_media_type != "application/dicom"
        || boundary.maximum_output_bytes == 0
        || boundary.maximum_output_bytes > 64 * 1024 * 1024
        || boundary.determinism != "semantic_stable"
        || boundary.semantic_evidence.is_empty()
        || boundary.semantic_evidence.iter().any(String::is_empty)
    {
        return Err(SrPlanError::ExternalBoundary);
    }
    Ok(())
}

fn common_operations(
    parameters: &SrDocumentParameters,
) -> Result<Vec<AttributeOperation>, SrPlanError> {
    Ok(vec![
        string_op("0020,0011", DicomVr::IS, &parameters.series_number)?,
        string_op(
            "0020,0013",
            DicomVr::IS,
            &parameters.instance_number.to_string(),
        )?,
        string_op("0040,A040", DicomVr::CS, "CONTAINER")?,
        string_op("0040,A050", DicomVr::CS, &parameters.continuity_of_content)?,
    ])
}

fn content_tree(
    parameters: &SrDocumentParameters,
    context: &SemanticPlanContext,
) -> Result<AttributeOperation, SrPlanError> {
    let items = match &parameters.document {
        SrDocumentKind::BasicText {
            observation,
            observation_text,
        } => vec![AttributeItem {
            attributes: vec![
                string_op("0040,A010", DicomVr::CS, "CONTAINS")?,
                string_op("0040,A040", DicomVr::CS, "TEXT")?,
                code_op("0040,A043", observation)?,
                string_op("0040,A160", DicomVr::UT, observation_text)?,
            ],
        }],
        SrDocumentKind::Comprehensive {
            measurement,
            numeric_value,
            units,
            image_concept,
        } => {
            let source = context.sources.first().ok_or(SrPlanError::SourceGraph)?;
            vec![
                AttributeItem {
                    attributes: vec![
                        string_op("0040,A010", DicomVr::CS, "CONTAINS")?,
                        string_op("0040,A040", DicomVr::CS, "NUM")?,
                        code_op("0040,A043", measurement)?,
                        measured_value_op(numeric_value, units)?,
                    ],
                },
                image_item(image_concept, source)?,
            ]
        }
        SrDocumentKind::KeyObjectSelection { .. } => context
            .sources
            .iter()
            .map(|source| image_item(&parameters.title, source))
            .collect::<Result<Vec<_>, _>>()?,
        SrDocumentKind::Comprehensive3d { .. } | SrDocumentKind::Tid1500 { .. } => {
            return Err(SrPlanError::ExternalDocumentRequiresImport);
        }
    };
    Ok(AttributeOperation::Set {
        address: address("0040,A730")?,
        vr: DicomVr::SQ,
        value: AttributeValue::Sequence(items),
    })
}

fn measured_value_op(value: &str, units: &CodedConcept) -> Result<AttributeOperation, SrPlanError> {
    Ok(AttributeOperation::Set {
        address: address("0040,A300")?,
        vr: DicomVr::SQ,
        value: AttributeValue::Sequence(vec![AttributeItem {
            attributes: vec![
                string_op("0040,A30A", DicomVr::DS, value)?,
                code_op("0040,08EA", units)?,
            ],
        }]),
    })
}

fn image_item(
    concept: &CodedConcept,
    source: &super::semantic::SemanticSource,
) -> Result<AttributeItem, SrPlanError> {
    let mut referenced = vec![
        string_op(
            "0008,1150",
            DicomVr::UI,
            &source.reference.referenced_sop_class_uid,
        )?,
        string_op(
            "0008,1155",
            DicomVr::UI,
            &source.reference.referenced_sop_instance_uid,
        )?,
    ];
    if !source.reference.referenced_frames.is_empty() {
        referenced.push(string_op(
            "0008,1160",
            DicomVr::IS,
            &source
                .reference
                .referenced_frames
                .iter()
                .map(u32::to_string)
                .collect::<Vec<_>>()
                .join("\\"),
        )?);
    }
    Ok(AttributeItem {
        attributes: vec![
            string_op("0040,A010", DicomVr::CS, "CONTAINS")?,
            string_op("0040,A040", DicomVr::CS, "IMAGE")?,
            code_op("0040,A043", concept)?,
            AttributeOperation::Set {
                address: address("0008,1199")?,
                vr: DicomVr::SQ,
                value: AttributeValue::Sequence(vec![AttributeItem {
                    attributes: referenced,
                }]),
            },
        ],
    })
}

fn code_op(tag: &str, code: &CodedConcept) -> Result<AttributeOperation, SrPlanError> {
    Ok(AttributeOperation::Set {
        address: address(tag)?,
        vr: DicomVr::SQ,
        value: AttributeValue::Sequence(vec![AttributeItem {
            attributes: vec![
                string_op("0008,0100", DicomVr::SH, &code.code_value)?,
                string_op("0008,0102", DicomVr::SH, &code.coding_scheme_designator)?,
                string_op("0008,0104", DicomVr::LO, &code.code_meaning)?,
            ],
        }]),
    })
}

fn string_op(tag: &str, vr: DicomVr, value: &str) -> Result<AttributeOperation, SrPlanError> {
    if value.is_empty() || value.len() > 1024 * 1024 || value.chars().any(char::is_control) {
        return Err(SrPlanError::InvalidParameters);
    }
    Ok(AttributeOperation::Set {
        address: address(tag)?,
        vr,
        value: AttributeValue::Primitive(PrimitiveValue::String(value.into())),
    })
}

fn address(tag: &str) -> Result<AttributeAddress, SrPlanError> {
    AttributeAddress::from_normalized_tag(tag)
        .map_err(|error| SrPlanError::Attribute(error.to_string()))
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SrPlanError {
    Recipe(String),
    RecipeShape,
    InvalidParameters,
    SourceGraph,
    WrongRoute,
    ExternalBoundary,
    ExternalDocumentRequiresImport,
    NativeDocumentCannotImport,
    Content(String),
    Attribute(String),
    Serialize(String),
    Plan(SemanticPlanError),
}

impl fmt::Display for SrPlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for SrPlanError {}
