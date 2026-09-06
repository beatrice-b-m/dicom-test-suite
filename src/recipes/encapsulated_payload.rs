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
pub const CALLER_PDF_ALGORITHM_PROVIDER_ID: &str = "algorithm.encapsulated_pdf_bytes";
pub const CALLER_STL_ALGORITHM_PROVIDER_ID: &str = "algorithm.binary_stl_bytes";
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub caller_metadata: Option<EncapsulatedCallerMetadata>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EncapsulatedCallerMetadata {
    pub patient_birth_date: String,
    pub patient_sex: String,
    pub study_date: String,
    pub study_time: String,
    pub content_date: String,
    pub content_time: String,
    pub manufacturer: String,
    pub software_versions: String,
    pub instance_number: String,
    pub instance_creation_date: String,
    pub instance_creation_time: String,
    pub referring_physician_name: String,
    pub accession_number: String,
    pub position_reference_indicator: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum EncapsulatedPayload {
    CallerPdf {
        mime_type: String,
        declared_size_bytes: u64,
        declared_sha256: String,
        bytes_hex: String,
    },
    CallerBinaryStl {
        mime_type: String,
        declared_size_bytes: u64,
        declared_sha256: String,
        bytes_hex: String,
        triangle_count: u32,
        unit_code_value: String,
        unit_coding_scheme: String,
        unit_code_meaning: String,
    },
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
        EncapsulatedPayload::CallerPdf { .. } => CALLER_PDF_ALGORITHM_PROVIDER_ID,
        EncapsulatedPayload::CallerBinaryStl { .. } => CALLER_STL_ALGORITHM_PROVIDER_ID,
        EncapsulatedPayload::MinimalPdf { .. } => MINIMAL_PDF_ALGORITHM_PROVIDER_ID,
        EncapsulatedPayload::ClosedTetrahedronBinaryStl { .. } => BINARY_STL_ALGORITHM_PROVIDER_ID,
    };
    if artifact.algorithm_provider_id.as_deref() != Some(expected_algorithm) {
        return Err(error("encapsulated payload algorithm is not registered"));
    }
    let input = EncapsulatedPayloadPlanInput {
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
        caller_metadata: parameters.caller_metadata,
    };
    let caller = matches!(
        input.payload,
        EncapsulatedPayload::CallerPdf { .. } | EncapsulatedPayload::CallerBinaryStl { .. }
    );
    if caller {
        if recipe.case_recipe_schema_version != "0.2.0"
            || recipe.planning_order.is_none()
            || recipe.projection_order.is_none()
            || !recipe.dependencies.is_empty()
            || artifact.output.provider_derived == Some(true)
            || !artifact.attribute_operations.is_empty()
            || !artifact.parameters.is_empty()
            || !artifact.content.parameters.is_empty()
            || artifact.secondary_capture.is_some()
            || artifact.metadata_sc.is_some()
            || artifact.classic_projection.is_some()
            || artifact.public_profile_membership.is_some()
            || artifact.encoding.transfer_syntax_uid != "1.2.840.10008.1.2.1"
            || artifact.encoding.sequence_length_policy != "default"
            || artifact.encoding.item_length_policy != "default"
            || artifact.encoding.offset_table_policy != "none"
            || artifact.encoding.fragmentation_policy != "native"
            || artifact
                .encoding
                .non_template_encoding_provider_id
                .is_some()
            || artifact.encoding.preamble_policy.as_deref() != Some("zero_filled")
            || artifact.encoding.file_meta_policy.as_deref() != Some("standard")
            || artifact
                .template
                .as_ref()
                .is_none_or(|t| t.template_version != "1.0.0")
        {
            return Err(error(
                "caller encapsulated payload requires complete recipe0.2 native declaration",
            ));
        }
        let pdf = matches!(input.payload, EncapsulatedPayload::CallerPdf { .. });
        if pdf
            && recipe
                .validation_rule_ids
                .iter()
                .chain(artifact.validation_rule_ids.iter())
                .any(|v| v == "validation.pdf.structure")
        {
            return Err(error(
                "caller PDF is opaque payload integrity, not PDF conformance",
            ));
        }
        let validation_rule = if pdf {
            "validation.encapsulated_document"
        } else {
            "validation.manufacturing_model"
        };
        let projection_rule = if pdf {
            "projection.encapsulated_document"
        } else {
            "projection.encapsulated_mesh"
        };
        if !recipe
            .projection_rule_ids
            .iter()
            .any(|v| v == projection_rule)
            || !artifact
                .projection_rule_ids
                .iter()
                .any(|v| v == projection_rule)
            || !recipe
                .validation_rule_ids
                .iter()
                .any(|v| v == validation_rule)
            || !artifact
                .validation_rule_ids
                .iter()
                .any(|v| v == validation_rule)
        {
            return Err(error(
                "caller encapsulated validation/projection rules differ",
            ));
        }
        validate_caller_encapsulated_input(&input)?;
    } else if input.caller_metadata.is_some() {
        return Err(error(
            "caller metadata requires explicit caller payload variant",
        ));
    }
    Ok(Some(input))
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
    #[serde(default)]
    caller_metadata: Option<EncapsulatedCallerMetadata>,
}

/// Decode bounded caller bytes and reuse the registered neutral content checks.
pub fn caller_encapsulated_bytes(
    input: &EncapsulatedPayloadPlanInput,
) -> Result<Vec<u8>, EncapsulatedPayloadPlanError> {
    let (hex, mime, size, hash, triangles) = match &input.payload {
        EncapsulatedPayload::CallerPdf {
            bytes_hex,
            mime_type,
            declared_size_bytes,
            declared_sha256,
        } => (
            bytes_hex,
            mime_type,
            *declared_size_bytes,
            declared_sha256,
            None,
        ),
        EncapsulatedPayload::CallerBinaryStl {
            bytes_hex,
            mime_type,
            declared_size_bytes,
            declared_sha256,
            triangle_count,
            ..
        } => (
            bytes_hex,
            mime_type,
            *declared_size_bytes,
            declared_sha256,
            Some(*triangle_count),
        ),
        _ => return Err(error("caller bytes require an explicit caller variant")),
    };
    if size == 0
        || size > ContentProviderLimits::default().max_output_bytes
        || hex.len() as u64
            != size
                .checked_mul(2)
                .ok_or_else(|| error("payload size overflow"))?
        || !hex.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err(error("caller payload hex/size exceeds bounded contract"));
    }
    let bytes = hex
        .as_bytes()
        .chunks_exact(2)
        .map(|b| u8::from_str_radix(std::str::from_utf8(b).unwrap(), 16).unwrap())
        .collect::<Vec<_>>();
    let request = if let Some(triangle_count) = triangles {
        if mime != "model/stl" || triangle_count == 0 {
            return Err(error("caller STL MIME type differs"));
        }
        ContentProviderRequest::Mesh(MeshContract {
            target: content_target()?,
            format: MeshFormat::BinaryStl,
            declared_size_bytes: size,
            declared_sha256: hash.clone(),
            triangle_count: Some(triangle_count),
            bytes: bytes.clone(),
        })
    } else {
        let header = bytes.get(..8).is_some_and(|h| {
            h == b"%PDF-2.0" || (h.starts_with(b"%PDF-1.") && (b'0'..=b'7').contains(&h[7]))
        });
        let end = bytes
            .iter()
            .rposition(|b| !b.is_ascii_whitespace())
            .map_or(0, |i| i + 1);
        if mime != "application/pdf"
            || !header
            || !matches!(bytes.get(8), Some(b'\r' | b'\n'))
            || !bytes[..end].ends_with(b"%%EOF")
        {
            return Err(error(
                "caller PDF requires declared MIME and PDF header/EOF framing; structure remains unassessed",
            ));
        }
        ContentProviderRequest::EncapsulatedDocument(BytePayloadContract {
            target: content_target()?,
            media_type: mime.clone(),
            declared_size_bytes: size,
            declared_sha256: hash.clone(),
            bytes: bytes.clone(),
        })
    };
    NeutralContentProvider
        .expand(&request, ContentProviderLimits::default())
        .map_err(|e| EncapsulatedPayloadPlanError::Content(e.to_string()))?;
    if triangles.is_some() {
        caller_stl_bounds(&bytes)?;
    }
    Ok(bytes)
}

pub fn caller_stl_bounds(
    bytes: &[u8],
) -> Result<([f64; 3], [f64; 3]), EncapsulatedPayloadPlanError> {
    if bytes.len() < 84 {
        return Err(error("truncated binary STL"));
    }
    let count = u32::from_le_bytes(bytes[80..84].try_into().unwrap());
    if count == 0 || 84_u64 + 50 * u64::from(count) != bytes.len() as u64 {
        return Err(error("STL triangle count/extent differs"));
    }
    let mut low = [f64::INFINITY; 3];
    let mut high = [f64::NEG_INFINITY; 3];
    for triangle in bytes[84..].chunks_exact(50) {
        if triangle[..48]
            .chunks_exact(4)
            .any(|w| !f32::from_le_bytes(w.try_into().unwrap()).is_finite())
            || triangle[48..50] != [0, 0]
        {
            return Err(error("STL floats must be finite and attribute counts zero"));
        }
        for vertex in triangle[12..48].chunks_exact(12) {
            for axis in 0..3 {
                let v = f64::from(f32::from_le_bytes(
                    vertex[axis * 4..axis * 4 + 4].try_into().unwrap(),
                ));
                low[axis] = low[axis].min(v);
                high[axis] = high[axis].max(v);
            }
        }
    }
    Ok((low, high))
}

pub fn validate_caller_encapsulated_input(
    input: &EncapsulatedPayloadPlanInput,
) -> Result<(), EncapsulatedPayloadPlanError> {
    let m = input
        .caller_metadata
        .as_ref()
        .ok_or_else(|| error("complete caller metadata is required"))?;
    let stl = matches!(input.payload, EncapsulatedPayload::CallerBinaryStl { .. });
    if !stl && !m.position_reference_indicator.is_empty() {
        return Err(error(
            "caller PDF has no Frame of Reference position indicator",
        ));
    }
    if m.instance_number.trim().is_empty() {
        return Err(error("caller encapsulated InstanceNumber is Type 1"));
    }
    if stl
        && [
            m.manufacturer.as_str(),
            m.software_versions.as_str(),
            input.manufacturer_model_name.as_str(),
            input.device_serial_number.as_str(),
        ]
        .iter()
        .any(|s| s.trim().is_empty())
    {
        return Err(error("caller STL equipment Type 1 values must be nonempty"));
    }

    let (sop, template, modality) = if stl {
        ("1.2.840.10008.5.1.4.1.1.104.3", "non-image/mesh/stl", "M3D")
    } else {
        (
            "1.2.840.10008.5.1.4.1.1.104.1",
            "non-image/encapsulated-document/pdf",
            "DOC",
        )
    };
    if input.sop_class_uid != sop
        || input.template_id != template
        || input.modality != modality
        || input.burned_in_annotation != "NO"
        || input.recognizable_visual_features != "NO"
        || !matches!(m.patient_sex.as_str(), "" | "M" | "F" | "O")
    {
        return Err(error(
            "caller encapsulated SOP/template or nonclaims differ",
        ));
    }
    for (keyword, vr, text) in [
        ("PatientName", DicomVr::PN, input.patient_name.as_str()),
        ("PatientID", DicomVr::LO, input.patient_id.as_str()),
        (
            "PatientBirthDate",
            DicomVr::DA,
            m.patient_birth_date.as_str(),
        ),
        ("PatientSex", DicomVr::CS, m.patient_sex.as_str()),
        ("StudyDate", DicomVr::DA, m.study_date.as_str()),
        ("StudyTime", DicomVr::TM, m.study_time.as_str()),
        ("ContentDate", DicomVr::DA, m.content_date.as_str()),
        ("ContentTime", DicomVr::TM, m.content_time.as_str()),
        (
            "InstanceCreationDate",
            DicomVr::DA,
            m.instance_creation_date.as_str(),
        ),
        (
            "InstanceCreationTime",
            DicomVr::TM,
            m.instance_creation_time.as_str(),
        ),
        ("StudyID", DicomVr::SH, input.study_id.as_str()),
        ("SeriesNumber", DicomVr::IS, input.series_number.as_str()),
        (
            "SeriesDescription",
            DicomVr::LO,
            input.series_description.as_str(),
        ),
        ("Manufacturer", DicomVr::LO, m.manufacturer.as_str()),
        (
            "ManufacturerModelName",
            DicomVr::LO,
            input.manufacturer_model_name.as_str(),
        ),
        (
            "DeviceSerialNumber",
            DicomVr::LO,
            input.device_serial_number.as_str(),
        ),
        (
            "SoftwareVersions",
            DicomVr::LO,
            m.software_versions.as_str(),
        ),
        ("InstanceNumber", DicomVr::IS, m.instance_number.as_str()),
        (
            "AcquisitionDateTime",
            DicomVr::DT,
            input.acquisition_datetime.as_str(),
        ),
        ("DocumentTitle", DicomVr::ST, input.document_title.as_str()),
        (
            "ReferringPhysicianName",
            DicomVr::PN,
            m.referring_physician_name.as_str(),
        ),
        ("AccessionNumber", DicomVr::SH, m.accession_number.as_str()),
        (
            "PositionReferenceIndicator",
            DicomVr::LO,
            m.position_reference_indicator.as_str(),
        ),
    ] {
        if !text.is_ascii() || text.contains('\\') {
            return Err(error("caller metadata requires singleton ASCII text"));
        }
        AttributeOperation::Set {
            address: address(keyword)?,
            vr,
            value: AttributeValue::Primitive(PrimitiveValue::String(text.into())),
        }
        .validate_declared_vr()
        .map_err(|e| error(&e.to_string()))?;
    }
    if let EncapsulatedPayload::CallerBinaryStl {
        unit_code_value,
        unit_coding_scheme,
        unit_code_meaning,
        ..
    } = &input.payload
    {
        let description = input
            .content_description
            .as_deref()
            .ok_or_else(|| error("caller STL content description required"))?;
        for (keyword, vr, text) in [
            ("ContentDescription", DicomVr::LO, description),
            ("CodeValue", DicomVr::SH, unit_code_value.as_str()),
            (
                "CodingSchemeDesignator",
                DicomVr::SH,
                unit_coding_scheme.as_str(),
            ),
            ("CodeMeaning", DicomVr::LO, unit_code_meaning.as_str()),
        ] {
            if text.is_empty() || !text.is_ascii() || text.contains('\\') {
                return Err(error("caller STL code/description must be singleton text"));
            }
            AttributeOperation::Set {
                address: address(keyword)?,
                vr,
                value: AttributeValue::Primitive(PrimitiveValue::String(text.into())),
            }
            .validate_declared_vr()
            .map_err(|e| error(&e.to_string()))?;
        }
    } else if input.content_description.is_some() {
        return Err(error("caller PDF does not encode STL content description"));
    }
    caller_encapsulated_bytes(input)?;
    Ok(())
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
            EncapsulatedPayload::CallerPdf {
                mime_type,
                declared_size_bytes,
                declared_sha256,
                ..
            } => {
                validate_caller_encapsulated_input(input)?;
                let bytes = caller_encapsulated_bytes(input)?;
                (
                    ContentProviderRequest::EncapsulatedDocument(BytePayloadContract {
                        target: content_target()?,
                        media_type: mime_type.clone(),
                        declared_size_bytes: *declared_size_bytes,
                        declared_sha256: declared_sha256.clone(),
                        bytes: bytes.clone(),
                    }),
                    bytes,
                    mime_type.clone(),
                    vec![
                        "validation.encapsulated_document",
                        "validation.content.integrity",
                    ],
                )
            }
            EncapsulatedPayload::CallerBinaryStl {
                mime_type,
                declared_size_bytes,
                declared_sha256,
                triangle_count,
                ..
            } => {
                validate_caller_encapsulated_input(input)?;
                let bytes = caller_encapsulated_bytes(input)?;
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
                    vec![
                        "validation.manufacturing_model",
                        "validation.stl.structure",
                        "validation.content.integrity",
                    ],
                )
            }
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
                vec![
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
                    vec![
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
            | EncapsulatedPayload::CallerBinaryStl { .. }
    );
    let metadata = input.caller_metadata.as_ref();
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
            metadata
                .map(|m| m.patient_birth_date.as_str())
                .unwrap_or(if stl { "" } else { "19700101" }),
            false,
        ),
        (
            "PatientSex",
            DicomVr::CS,
            metadata
                .map(|m| m.patient_sex.as_str())
                .unwrap_or(if stl { "" } else { "O" }),
            false,
        ),
        ("StudyInstanceUID", DicomVr::UI, ids.study.as_str(), true),
        (
            "StudyDate",
            DicomVr::DA,
            metadata
                .map(|m| m.study_date.as_str())
                .unwrap_or("20260101"),
            false,
        ),
        (
            "StudyTime",
            DicomVr::TM,
            metadata.map(|m| m.study_time.as_str()).unwrap_or("000000"),
            false,
        ),
        (
            "ReferringPhysicianName",
            DicomVr::PN,
            metadata
                .map(|m| m.referring_physician_name.as_str())
                .unwrap_or(""),
            false,
        ),
        ("StudyID", DicomVr::SH, input.study_id.as_str(), false),
        (
            "AccessionNumber",
            DicomVr::SH,
            metadata.map(|m| m.accession_number.as_str()).unwrap_or(""),
            false,
        ),
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
        (
            "Manufacturer",
            DicomVr::LO,
            metadata
                .map(|m| m.manufacturer.as_str())
                .unwrap_or("dicom-test-suite"),
            false,
        ),
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
            metadata
                .map(|m| m.software_versions.as_str())
                .unwrap_or(BYTE_STABLE_OUTPUT_VERSION),
            false,
        ),
        (
            "InstanceNumber",
            DicomVr::IS,
            metadata.map(|m| m.instance_number.as_str()).unwrap_or("1"),
            false,
        ),
        (
            "ContentDate",
            DicomVr::DA,
            metadata
                .map(|m| m.content_date.as_str())
                .unwrap_or("20260101"),
            false,
        ),
        (
            "ContentTime",
            DicomVr::TM,
            metadata
                .map(|m| m.content_time.as_str())
                .unwrap_or("000000"),
            false,
        ),
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
    if !stl {
        if let Some(metadata) = metadata {
            attributes.push(resolved_string(
                "InstanceCreationDate",
                DicomVr::DA,
                &metadata.instance_creation_date,
                false,
            )?);
            attributes.push(resolved_string(
                "InstanceCreationTime",
                DicomVr::TM,
                &metadata.instance_creation_time,
                false,
            )?);
        }
    }
    if stl {
        attributes.extend([
            resolved_string(
                "InstanceCreationDate",
                DicomVr::DA,
                metadata
                    .map(|m| m.instance_creation_date.as_str())
                    .unwrap_or("20260101"),
                false,
            )?,
            resolved_string(
                "InstanceCreationTime",
                DicomVr::TM,
                metadata
                    .map(|m| m.instance_creation_time.as_str())
                    .unwrap_or("000000"),
                false,
            )?,
            resolved_string("InstanceCreatorUID", DicomVr::UI, &ids.implementation, true)?,
            resolved_string(
                "FrameOfReferenceUID",
                DicomVr::UI,
                ids.frame_of_reference
                    .as_deref()
                    .ok_or(EncapsulatedPayloadPlanError::MissingFrameOfReference)?,
                true,
            )?,
            resolved_string(
                "PositionReferenceIndicator",
                DicomVr::LO,
                metadata
                    .map(|m| m.position_reference_indicator.as_str())
                    .unwrap_or(""),
                false,
            )?,
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
        }
        | EncapsulatedPayload::CallerBinaryStl {
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
                | EncapsulatedPayload::CallerBinaryStl { .. }
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
