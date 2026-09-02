//! Direct plans for declared-hash Encapsulated PDF and STL payloads.

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::composition::{
    AttributeAddress, AttributeItem, AttributeOperation, AttributeValue, CompositionUidRole,
    DicomVr, PrimitiveValue, ResolvedAttribute, ResolvedInstancePlan, TemplateId, TemplateVersion,
    ValueOrigin,
};
use crate::corpus_plan::{
    ArtifactProvenance, ArtifactResourceEstimate, CaseBinding, EncodingPlan, EvidenceIndependence,
    EvidenceObligation, EvidencePlan, FileMetaPolicy, FragmentationPolicy,
    ImplementationIdentityPlan, ItemLengthPolicy, OffsetTablePolicy, PlannedDicomArtifact,
    PreamblePolicy, SequenceLengthPolicy, ValidationPlan, ValidationRequirement, ValidationRule,
};
use crate::executor::services::{
    ArtifactExecutionBindings, ByteBinding, NativeFrameBinding, SlotExecutionBinding,
};
use crate::{BYTE_STABLE_OUTPUT_VERSION, IMPLEMENTATION_VERSION_NAME, sha256_hex};

use super::typed_bulk::{TypedBulkPlanProviderOutput, TypedBulkPlanningContext};
use super::{
    BytePayloadContract, CaseRecipe, ContentProviderLimits, ContentProviderRequest, ContentTarget,
    MeshContract, MeshFormat, NeutralContentProvider, RecipeIdentity,
};

pub const ENCAPSULATED_PAYLOAD_PLAN_PROVIDER_ID: &str = "native.encapsulated_payload_plan";
pub const DECLARED_BYTE_PAYLOAD_CONTENT_PROVIDER_ID: &str = "content.declared_byte_payload";
pub const MINIMAL_PDF_ALGORITHM_PROVIDER_ID: &str = "algorithm.encapsulated_pdf_minimal";
pub const BINARY_STL_ALGORITHM_PROVIDER_ID: &str = "algorithm.binary_stl_tetrahedron";

const MINIMAL_PDF: &[u8] = b"%PDF-1.4\n1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 72 72] >>\nendobj\nxref\n0 4\n0000000000 65535 f \n0000000009 00000 n \n0000000058 00000 n \n0000000115 00000 n \ntrailer\n<< /Size 4 /Root 1 0 R >>\nstartxref\n184\n%%EOF\n";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EncapsulatedPayloadPlanInput {
    pub case_id: String,
    pub recipe: RecipeIdentity,
    pub artifact_logical_id: String,
    pub template_id: String,
    pub sop_class_uid: String,
    pub output_path: String,
    pub modality: String,
    pub study_id: String,
    pub series_number: String,
    pub series_description: String,
    pub manufacturer_model_name: String,
    pub device_serial_number: String,
    pub patient_name: String,
    pub patient_id: String,
    pub document_title: String,
    pub content_description: Option<String>,
    pub acquisition_datetime: String,
    pub burned_in_annotation: String,
    pub recognizable_visual_features: String,
    pub payload: EncapsulatedPayload,
    pub projection: EncapsulatedPayloadProjection,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum EncapsulatedPayload {
    MinimalPdf {
        mime_type: String,
        declared_size_bytes: u64,
        declared_sha256: String,
    },
    ClosedTetrahedronBinaryStl {
        mime_type: String,
        declared_size_bytes: u64,
        declared_sha256: String,
        triangle_count: u32,
        unit_code_value: String,
        unit_coding_scheme: String,
        unit_code_meaning: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EncapsulatedPayloadProjection {
    pub expected_capabilities: Vec<String>,
    pub expected_visual_pattern: String,
    pub known_stressors: Vec<String>,
}

pub fn encapsulated_payload_input_from_recipe(
    recipe: &CaseRecipe,
) -> Result<Option<EncapsulatedPayloadPlanInput>, EncapsulatedPayloadPlanError> {
    if recipe.plan_provider_id != ENCAPSULATED_PAYLOAD_PLAN_PROVIDER_ID {
        return Ok(None);
    }
    let [artifact] = recipe
        .dicom
        .as_ref()
        .ok_or_else(|| error("missing DICOM artifact"))?
        .artifacts
        .as_slice()
    else {
        return Err(error("encapsulated payload recipe requires one artifact"));
    };
    let content = &artifact.content;
    if content.provider_id != DECLARED_BYTE_PAYLOAD_CONTENT_PROVIDER_ID {
        return Err(error("encapsulated content provider is not registered"));
    }
    let parameters: EncapsulatedDocumentParameters = serde_json::from_value(
        serde_json::Value::Object(recipe.provider_parameters.clone()),
    )
    .map_err(|error| EncapsulatedPayloadPlanError::Recipe(error.to_string()))?;
    let expected_algorithm = match &parameters.payload {
        EncapsulatedPayload::MinimalPdf { .. } => MINIMAL_PDF_ALGORITHM_PROVIDER_ID,
        EncapsulatedPayload::ClosedTetrahedronBinaryStl { .. } => BINARY_STL_ALGORITHM_PROVIDER_ID,
    };
    if artifact.algorithm_provider_id.as_deref() != Some(expected_algorithm) {
        return Err(error("encapsulated payload algorithm is not registered"));
    }
    Ok(Some(EncapsulatedPayloadPlanInput {
        case_id: recipe.binding.case_id.clone(),
        recipe: recipe.identity(),
        artifact_logical_id: artifact.logical_id.clone(),
        template_id: artifact
            .template
            .as_ref()
            .ok_or_else(|| error("missing encapsulated template"))?
            .template_id
            .clone(),
        sop_class_uid: parameters.sop_class_uid,
        output_path: artifact
            .output
            .path
            .clone()
            .ok_or_else(|| error("missing encapsulated output path"))?,
        modality: parameters.modality,
        study_id: parameters.study_id,
        series_number: parameters.series_number,
        series_description: parameters.series_description,
        manufacturer_model_name: parameters.manufacturer_model_name,
        device_serial_number: parameters.device_serial_number,
        patient_name: parameters.patient_name,
        patient_id: parameters.patient_id,
        document_title: parameters.document_title,
        content_description: parameters.content_description,
        acquisition_datetime: parameters.acquisition_datetime,
        burned_in_annotation: parameters.burned_in_annotation,
        recognizable_visual_features: parameters.recognizable_visual_features,
        payload: parameters.payload,
        projection: parameters.projection,
    }))
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct EncapsulatedDocumentParameters {
    sop_class_uid: String,
    modality: String,
    study_id: String,
    series_number: String,
    series_description: String,
    manufacturer_model_name: String,
    device_serial_number: String,
    patient_name: String,
    patient_id: String,
    document_title: String,
    content_description: Option<String>,
    acquisition_datetime: String,
    burned_in_annotation: String,
    recognizable_visual_features: String,
    payload: EncapsulatedPayload,
    projection: EncapsulatedPayloadProjection,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct EncapsulatedPayloadPlanProvider;

impl EncapsulatedPayloadPlanProvider {
    pub fn plan(
        &self,
        input: &EncapsulatedPayloadPlanInput,
        context: &TypedBulkPlanningContext,
        limits: ContentProviderLimits,
    ) -> Result<TypedBulkPlanProviderOutput, EncapsulatedPayloadPlanError> {
        context
            .validate(&input.artifact_logical_id)
            .map_err(EncapsulatedPayloadPlanError::Context)?;
        let ids = Identities::from_context(context, input)?;
        let (request, bytes, media_type, validation_ids) = match &input.payload {
            EncapsulatedPayload::MinimalPdf {
                mime_type,
                declared_size_bytes,
                declared_sha256,
            } => (
                ContentProviderRequest::EncapsulatedDocument(BytePayloadContract {
                    target: content_target()?,
                    media_type: mime_type.clone(),
                    declared_size_bytes: *declared_size_bytes,
                    declared_sha256: declared_sha256.clone(),
                    bytes: MINIMAL_PDF.to_vec(),
                }),
                MINIMAL_PDF.to_vec(),
                mime_type.clone(),
                [
                    "validation.encapsulated_document",
                    "validation.pdf.structure",
                    "validation.content.integrity",
                ],
            ),
            EncapsulatedPayload::ClosedTetrahedronBinaryStl {
                mime_type,
                declared_size_bytes,
                declared_sha256,
                triangle_count,
                ..
            } => {
                let bytes = closed_tetrahedron_binary_stl();
                (
                    ContentProviderRequest::Mesh(MeshContract {
                        target: content_target()?,
                        format: MeshFormat::BinaryStl,
                        declared_size_bytes: *declared_size_bytes,
                        declared_sha256: declared_sha256.clone(),
                        triangle_count: Some(*triangle_count),
                        bytes: bytes.clone(),
                    }),
                    bytes,
                    mime_type.clone(),
                    [
                        "validation.manufacturing_model",
                        "validation.stl.structure",
                        "validation.content.integrity",
                    ],
                )
            }
        };
        let output = NeutralContentProvider
            .expand(&request, limits)
            .map_err(|error| EncapsulatedPayloadPlanError::Content(error.to_string()))?;
        let content = output
            .contents
            .into_iter()
            .next()
            .ok_or_else(|| EncapsulatedPayloadPlanError::Content("missing content".into()))?;
        if content.sha256 != sha256_hex(&bytes) || content.size_bytes != bytes.len() as u64 {
            return Err(EncapsulatedPayloadPlanError::DeclaredPayload);
        }
        let attributes = attributes(input, &ids, &media_type, content.size_bytes)?;
        let artifact_id = context.target_instance_id.clone();
        let planned = PlannedDicomArtifact {
            logical_id: artifact_id.clone(),
            order: context.order,
            provenance: ArtifactProvenance::Requested,
            case_binding: Some(CaseBinding {
                case_id: input.case_id.clone(),
                recipe_id: input.recipe.recipe_id.clone(),
                recipe_version: input.recipe.recipe_version.clone(),
            }),
            instance: ResolvedInstancePlan {
                plan_schema_version: "0.1.0".into(),
                instance_id: artifact_id.clone(),
                template_id: TemplateId(input.template_id.clone()),
                template_version: "1.0.0"
                    .parse::<TemplateVersion>()
                    .map_err(|error| EncapsulatedPayloadPlanError::Template(error.to_string()))?,
                sop_class_uid: input.sop_class_uid.clone(),
                transfer_syntax_uid: "1.2.840.10008.1.2.1".into(),
                identities: context.identities.clone(),
                attributes,
                content: vec![content],
                references: vec![],
            },
            output: context.output.clone(),
            encoding: encoding(&ids.implementation),
            validation: ValidationPlan {
                rules: validation_ids
                    .into_iter()
                    .map(|rule_id| ValidationRule {
                        rule_id: rule_id.into(),
                        requirement: ValidationRequirement::Required,
                        parameters: BTreeMap::new(),
                    })
                    .collect(),
            },
            evidence: EvidencePlan {
                obligations: vec![EvidenceObligation {
                    obligation_id: format!("same-project:{artifact_id}"),
                    route_id: "builtin.strict".into(),
                    independence: EvidenceIndependence::SameProject,
                    required: true,
                    parameters: BTreeMap::new(),
                }],
            },
            resources: ArtifactResourceEstimate {
                output_bytes: (bytes.len() as u64)
                    .checked_add(256 * 1024)
                    .ok_or(EncapsulatedPayloadPlanError::ResourceOverflow)?,
                peak_working_bytes: (bytes.len() as u64)
                    .checked_mul(2)
                    .and_then(|value| value.checked_add(512 * 1024))
                    .ok_or(EncapsulatedPayloadPlanError::ResourceOverflow)?,
            },
        };
        Ok(TypedBulkPlanProviderOutput {
            artifact: planned,
            bindings: ArtifactExecutionBindings {
                artifact_id,
                slots: BTreeMap::from([(
                    "document".into(),
                    SlotExecutionBinding::NativeFrames {
                        frames: vec![NativeFrameBinding {
                            frame_number: 1,
                            bytes: ByteBinding::Inline {
                                sha256: sha256_hex(&bytes),
                                bytes,
                            },
                            rows: 1,
                            columns: 1,
                            samples_per_pixel: 1,
                            bits_allocated: 8,
                            photometric_interpretation: "DOCUMENT".into(),
                        }],
                    },
                )]),
            },
        })
    }
}

fn content_target() -> Result<ContentTarget, EncapsulatedPayloadPlanError> {
    Ok(ContentTarget {
        slot: "document".into(),
        content_kind: "encapsulated_document".into(),
        address: address("EncapsulatedDocument")?,
        vr: DicomVr::OB,
    })
}

fn attributes(
    input: &EncapsulatedPayloadPlanInput,
    ids: &Identities,
    media_type: &str,
    length: u64,
) -> Result<Vec<ResolvedAttribute>, EncapsulatedPayloadPlanError> {
    let stl = matches!(
        &input.payload,
        EncapsulatedPayload::ClosedTetrahedronBinaryStl { .. }
    );
    let mut attributes = Vec::new();
    if stl {
        attributes.push(resolved_string(
            "SpecificCharacterSet",
            DicomVr::CS,
            "ISO_IR 192",
            false,
        )?);
    }
    for (keyword, vr, value, structural) in [
        (
            "SOPClassUID",
            DicomVr::UI,
            input.sop_class_uid.as_str(),
            false,
        ),
        ("SOPInstanceUID", DicomVr::UI, ids.sop.as_str(), true),
        ("SyntheticData", DicomVr::CS, "YES", false),
        (
            "PatientName",
            DicomVr::PN,
            input.patient_name.as_str(),
            false,
        ),
        ("PatientID", DicomVr::LO, input.patient_id.as_str(), false),
        (
            "PatientBirthDate",
            DicomVr::DA,
            if stl { "" } else { "19700101" },
            false,
        ),
        ("PatientSex", DicomVr::CS, if stl { "" } else { "O" }, false),
        ("StudyInstanceUID", DicomVr::UI, ids.study.as_str(), true),
        ("StudyDate", DicomVr::DA, "20260101", false),
        ("StudyTime", DicomVr::TM, "000000", false),
        ("ReferringPhysicianName", DicomVr::PN, "", false),
        ("StudyID", DicomVr::SH, input.study_id.as_str(), false),
        ("AccessionNumber", DicomVr::SH, "", false),
        ("Modality", DicomVr::CS, input.modality.as_str(), false),
        ("SeriesInstanceUID", DicomVr::UI, ids.series.as_str(), true),
        (
            "SeriesNumber",
            DicomVr::IS,
            input.series_number.as_str(),
            false,
        ),
        (
            "SeriesDescription",
            DicomVr::LO,
            input.series_description.as_str(),
            false,
        ),
        ("Manufacturer", DicomVr::LO, "dicom-test-suite", false),
        (
            "ManufacturerModelName",
            DicomVr::LO,
            input.manufacturer_model_name.as_str(),
            false,
        ),
        (
            "DeviceSerialNumber",
            DicomVr::LO,
            input.device_serial_number.as_str(),
            false,
        ),
        (
            "SoftwareVersions",
            DicomVr::LO,
            BYTE_STABLE_OUTPUT_VERSION,
            false,
        ),
        ("InstanceNumber", DicomVr::IS, "1", false),
        ("ContentDate", DicomVr::DA, "20260101", false),
        ("ContentTime", DicomVr::TM, "000000", false),
        (
            "AcquisitionDateTime",
            DicomVr::DT,
            input.acquisition_datetime.as_str(),
            false,
        ),
        (
            "BurnedInAnnotation",
            DicomVr::CS,
            input.burned_in_annotation.as_str(),
            false,
        ),
        (
            "RecognizableVisualFeatures",
            DicomVr::CS,
            input.recognizable_visual_features.as_str(),
            false,
        ),
        (
            "DocumentTitle",
            DicomVr::ST,
            input.document_title.as_str(),
            false,
        ),
        (
            "MIMETypeOfEncapsulatedDocument",
            DicomVr::LO,
            media_type,
            false,
        ),
    ] {
        attributes.push(resolved_string(keyword, vr, value, structural)?);
    }
    if stl {
        attributes.extend([
            resolved_string("InstanceCreationDate", DicomVr::DA, "20260101", false)?,
            resolved_string("InstanceCreationTime", DicomVr::TM, "000000", false)?,
            resolved_string("InstanceCreatorUID", DicomVr::UI, &ids.implementation, true)?,
            resolved_string(
                "FrameOfReferenceUID",
                DicomVr::UI,
                ids.frame_of_reference
                    .as_deref()
                    .ok_or(EncapsulatedPayloadPlanError::MissingFrameOfReference)?,
                true,
            )?,
            resolved_string("PositionReferenceIndicator", DicomVr::LO, "", false)?,
            resolved_string("ModelModification", DicomVr::CS, "NO", false)?,
            resolved_string("ModelMirroring", DicomVr::CS, "NO", false)?,
            resolved_string(
                "ContentDescription",
                DicomVr::LO,
                input.content_description.as_deref().unwrap_or(""),
                false,
            )?,
        ]);
        attributes.push(resolved_sequence(
            "ConceptNameCodeSequence",
            vec![code_item("129006", "DCM", "Anatomical Model")?],
        )?);
        if let EncapsulatedPayload::ClosedTetrahedronBinaryStl {
            unit_code_value,
            unit_coding_scheme,
            unit_code_meaning,
            ..
        } = &input.payload
        {
            attributes.push(resolved_sequence(
                "MeasurementUnitsCodeSequence",
                vec![code_item(
                    unit_code_value,
                    unit_coding_scheme,
                    unit_code_meaning,
                )?],
            )?);
        }
    } else {
        attributes.push(resolved_sequence("ConceptNameCodeSequence", vec![])?);
        attributes.push(resolved_string(
            "ConversionType",
            DicomVr::CS,
            "SYN",
            false,
        )?);
    }
    attributes.push(ResolvedAttribute {
        address: address("EncapsulatedDocumentLength")?,
        vr: DicomVr::UL,
        value: Some(AttributeValue::Primitive(PrimitiveValue::Unsigned(length))),
        origin: ValueOrigin::DerivedStructural,
    });
    attributes.sort_by(|left, right| left.address.cmp(&right.address));
    Ok(attributes)
}

fn resolved_string(
    keyword: &str,
    vr: DicomVr,
    value: &str,
    structural: bool,
) -> Result<ResolvedAttribute, EncapsulatedPayloadPlanError> {
    Ok(ResolvedAttribute {
        address: address(keyword)?,
        vr,
        value: (!value.is_empty())
            .then(|| AttributeValue::Primitive(PrimitiveValue::String(value.into()))),
        origin: if structural {
            ValueOrigin::DerivedStructural
        } else {
            ValueOrigin::TemplateDefault
        },
    })
}

fn resolved_sequence(
    keyword: &str,
    items: Vec<AttributeItem>,
) -> Result<ResolvedAttribute, EncapsulatedPayloadPlanError> {
    Ok(ResolvedAttribute {
        address: address(keyword)?,
        vr: DicomVr::SQ,
        value: Some(AttributeValue::Sequence(items)),
        origin: ValueOrigin::TemplateDefault,
    })
}

fn code_item(
    value: &str,
    scheme: &str,
    meaning: &str,
) -> Result<AttributeItem, EncapsulatedPayloadPlanError> {
    Ok(AttributeItem {
        attributes: vec![
            set_string("CodeValue", DicomVr::SH, value)?,
            set_string("CodingSchemeDesignator", DicomVr::SH, scheme)?,
            set_string("CodeMeaning", DicomVr::LO, meaning)?,
        ],
    })
}

fn set_string(
    keyword: &str,
    vr: DicomVr,
    value: &str,
) -> Result<AttributeOperation, EncapsulatedPayloadPlanError> {
    Ok(AttributeOperation::Set {
        address: address(keyword)?,
        vr,
        value: AttributeValue::Primitive(PrimitiveValue::String(value.into())),
    })
}

fn address(keyword: &str) -> Result<AttributeAddress, EncapsulatedPayloadPlanError> {
    AttributeAddress::from_keyword(keyword)
        .map_err(|error| EncapsulatedPayloadPlanError::Attribute(error.to_string()))
}

fn closed_tetrahedron_binary_stl() -> Vec<u8> {
    const POINTS: [[[f32; 3]; 4]; 4] = [
        [
            [0.0, 0.0, -1.0],
            [0.0, 0.0, 0.0],
            [0.0, 10.0, 0.0],
            [10.0, 0.0, 0.0],
        ],
        [
            [0.0, -1.0, 0.0],
            [0.0, 0.0, 0.0],
            [10.0, 0.0, 0.0],
            [0.0, 0.0, 10.0],
        ],
        [
            [-1.0, 0.0, 0.0],
            [0.0, 0.0, 0.0],
            [0.0, 0.0, 10.0],
            [0.0, 10.0, 0.0],
        ],
        [
            [0.577_350_26; 3],
            [10.0, 0.0, 0.0],
            [0.0, 10.0, 0.0],
            [0.0, 0.0, 10.0],
        ],
    ];
    let mut bytes = b"dicom-test-suite synthetic closed tetrahedron".to_vec();
    bytes.resize(80, 0);
    bytes.extend_from_slice(&4_u32.to_le_bytes());
    for triangle in POINTS {
        for point in triangle {
            for coordinate in point {
                bytes.extend_from_slice(&coordinate.to_le_bytes());
            }
        }
        bytes.extend_from_slice(&0_u16.to_le_bytes());
    }
    bytes
}

struct Identities {
    study: String,
    series: String,
    sop: String,
    frame_of_reference: Option<String>,
    implementation: String,
}

impl Identities {
    fn from_context(
        context: &TypedBulkPlanningContext,
        input: &EncapsulatedPayloadPlanInput,
    ) -> Result<Self, EncapsulatedPayloadPlanError> {
        let get = |role| context.identities.get(&role, 0).map(str::to_owned);
        let result = Self {
            study: get(CompositionUidRole::StudyInstance)
                .ok_or(EncapsulatedPayloadPlanError::MissingIdentity)?,
            series: get(CompositionUidRole::SeriesInstance)
                .ok_or(EncapsulatedPayloadPlanError::MissingIdentity)?,
            sop: get(CompositionUidRole::SopInstance)
                .ok_or(EncapsulatedPayloadPlanError::MissingIdentity)?,
            frame_of_reference: get(CompositionUidRole::FrameOfReference),
            implementation: get(CompositionUidRole::ImplementationClass)
                .ok_or(EncapsulatedPayloadPlanError::MissingIdentity)?,
        };
        if matches!(
            &input.payload,
            EncapsulatedPayload::ClosedTetrahedronBinaryStl { .. }
        ) && result.frame_of_reference.is_none()
        {
            return Err(EncapsulatedPayloadPlanError::MissingFrameOfReference);
        }
        Ok(result)
    }
}

fn encoding(implementation: &str) -> EncodingPlan {
    EncodingPlan {
        transfer_syntax_uid: "1.2.840.10008.1.2.1".into(),
        sequence_length: SequenceLengthPolicy::WriterDefault,
        item_length: ItemLengthPolicy::WriterDefault,
        fragmentation: FragmentationPolicy::Native,
        offset_table: OffsetTablePolicy::NotApplicable,
        preamble: PreamblePolicy::ZeroFilled,
        file_meta: FileMetaPolicy::Standard,
        implementation: ImplementationIdentityPlan {
            class_uid: implementation.into(),
            version_name: Some(IMPLEMENTATION_VERSION_NAME.into()),
        },
        backend_id: "dicom-rs.part10".into(),
    }
}

fn error(message: &str) -> EncapsulatedPayloadPlanError {
    EncapsulatedPayloadPlanError::Recipe(message.into())
}

#[derive(Debug)]
pub enum EncapsulatedPayloadPlanError {
    Recipe(String),
    Context(String),
    Content(String),
    Attribute(String),
    Template(String),
    DeclaredPayload,
    MissingIdentity,
    MissingFrameOfReference,
    ResourceOverflow,
}

impl fmt::Display for EncapsulatedPayloadPlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Recipe(message)
            | Self::Context(message)
            | Self::Content(message)
            | Self::Attribute(message)
            | Self::Template(message) => formatter.write_str(message),
            Self::DeclaredPayload => {
                formatter.write_str("encapsulated payload differs from its declared size or hash")
            }
            Self::MissingIdentity => {
                formatter.write_str("encapsulated payload context lacks a required identity")
            }
            Self::MissingFrameOfReference => {
                formatter.write_str("STL context lacks Frame of Reference identity")
            }
            Self::ResourceOverflow => {
                formatter.write_str("encapsulated payload resource arithmetic overflow")
            }
        }
    }
}

impl std::error::Error for EncapsulatedPayloadPlanError {}
