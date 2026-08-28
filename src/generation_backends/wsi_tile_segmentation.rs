//! Rust-side orchestration for the external WSI tile segmentation case.

use std::fs;
use std::path::PathBuf;

use dicom_dictionary_std::tags;
use dicom_object::open_file;
use serde_json::{Value, json};

use crate::sha256_hex;

use super::{
    BackendContractError, BackendDiscovery, BackendInvocation, ControlledMetadata,
    PROTOCOL_VERSION, ParametricMapSource, PreparedBackend, StandardsProvenance, backend_policy,
    discover_prepared_backend, invoke_backend, load_backend_lock, promote_staged_outputs,
};

pub const BACKEND_ID: &str = "highdicom_pydicom";
pub const CASE_ID: &str = "derived/seg/wsi_tile_reference";
pub const SOURCE_CASE_ID: &str = "vl/wsi/tiled_full_small";
pub const RECIPE_ID: &str = "derived_seg_wsi_tile_reference";
pub const RECIPE_VERSION: &str = "0.1.0";
pub const OUTPUT_FILE: &str = "wsi-tile-segmentation.dcm";
pub const SOP_CLASS_UID: &str = "1.2.840.10008.5.1.4.1.1.66.4";
pub const SOURCE_SOP_CLASS_UID: &str = "1.2.840.10008.5.1.4.1.1.77.1.6";
pub const TRANSFER_SYNTAX_UID: &str = "1.2.840.10008.1.2.1";
pub const SOURCE_FRAME_NUMBERS: [u64; 2] = [1, 4];
pub const FRAME_VALUES: [[u8; 4]; 2] = [[255, 0, 0, 255], [0, 255, 255, 0]];
pub const FRAME_SHA256: [&str; 2] = [
    "34aaa746c25a0f105c4316bbb1f009aa359f49582656ee97d73c58132d563423",
    "10db5223d19bd1d58c2b8eb3c723b0ba104cf17564f9434e53e1b9e642fb3b37",
];
pub const PAYLOAD_SHA256: &str = "74fa7cbb10160e0eb1f16f35fa9ad0e7f2712af56019996e88cf1034be92635e";
pub const MATRIX_SHA256: &str = "a8ec6f910c0fb02685163a3251bed92517d1016c9173f1e4f021e6b4194f2467";

#[derive(Debug, Clone)]
pub struct WsiTileSegmentationIdentities {
    pub study_instance_uid: String,
    pub series_instance_uid: String,
    pub frame_of_reference_uid: String,
    pub sop_instance_uid: String,
    pub dimension_organization_uid: String,
}

#[derive(Debug, Clone)]
pub struct WsiTileSegmentationGenerationInput {
    pub repository_root: PathBuf,
    pub generated_root: PathBuf,
    pub staging_root: PathBuf,
    pub destination_root: PathBuf,
    pub seed: u64,
    pub standards: StandardsProvenance,
    pub controlled_metadata: ControlledMetadata,
    pub identities: WsiTileSegmentationIdentities,
    pub source: ParametricMapSource,
}

#[derive(Debug)]
pub struct WsiTileSegmentationGenerated {
    pub output_path: PathBuf,
    pub output_bytes: Vec<u8>,
    pub response: Value,
    pub backend: PreparedBackend,
    pub identities: WsiTileSegmentationIdentities,
}

#[derive(Debug)]
pub enum WsiTileSegmentationOutcome {
    Generated(WsiTileSegmentationGenerated),
    Unavailable { code: String, message: String },
}

pub fn generate_wsi_tile_segmentation(
    input: &WsiTileSegmentationGenerationInput,
) -> Result<WsiTileSegmentationOutcome, BackendContractError> {
    validate_input(input)?;
    let lock = load_backend_lock(&input.repository_root)?;
    let policy = backend_policy(&lock, BACKEND_ID)
        .ok_or_else(|| invalid(format!("backend lock has no {BACKEND_ID} policy")))?;
    let backend = match discover_prepared_backend(&input.repository_root, policy)? {
        BackendDiscovery::Available(backend) => backend,
        BackendDiscovery::Unavailable { code, message } => {
            return Ok(WsiTileSegmentationOutcome::Unavailable { code, message });
        }
    };
    let request = build_request(input)?;
    let invocation = BackendInvocation {
        executable: backend.executable.clone(),
        fixed_arguments: backend.fixed_arguments.clone(),
        timeout: backend.timeout,
        max_response_bytes: backend.max_response_bytes,
        max_stdout_bytes: backend.max_stdout_bytes,
        max_stderr_bytes: backend.max_stderr_bytes,
        output_limits: backend.output_limits,
        dependency_lock_sha256: backend.dependency_lock_sha256.clone(),
        environment_fingerprint: backend.environment_fingerprint.clone(),
    };
    let run = invoke_backend(
        &invocation,
        &request,
        &input.generated_root,
        &input.staging_root,
    )?;
    match run.response["status"]
        .as_str()
        .expect("response schema checked status")
    {
        "unavailable" => {
            let failure = &run.response["failure"];
            Ok(WsiTileSegmentationOutcome::Unavailable {
                code: failure["code"]
                    .as_str()
                    .expect("schema checked code")
                    .to_string(),
                message: failure["message"]
                    .as_str()
                    .expect("schema checked message")
                    .to_string(),
            })
        }
        "failed" => Err(invalid(format!(
            "backend generation failed: {}",
            run.response["failure"]["message"]
                .as_str()
                .expect("schema checked failure message")
        ))),
        "generated" => {
            verify_backend_response(&run.response, input)?;
            let staged_path = run.staging_root.join("outputs").join(OUTPUT_FILE);
            let output_bytes =
                fs::read(&staged_path).map_err(|source| BackendContractError::Read {
                    path: staged_path.clone(),
                    source,
                })?;
            verify_dicom_payload(&staged_path, input)?;
            promote_staged_outputs(&run.staging_root.join("outputs"), &input.destination_root)?;
            Ok(WsiTileSegmentationOutcome::Generated(
                WsiTileSegmentationGenerated {
                    output_path: input.destination_root.join(OUTPUT_FILE),
                    output_bytes,
                    response: run.response,
                    backend,
                    identities: input.identities.clone(),
                },
            ))
        }
        status => Err(invalid(format!("unexpected backend status {status}"))),
    }
}

fn validate_input(input: &WsiTileSegmentationGenerationInput) -> Result<(), BackendContractError> {
    if input.source.source_case_id != SOURCE_CASE_ID
        || input.source.role != "source_image"
        || input.source.sop_class_uid != SOURCE_SOP_CLASS_UID
        || input.source.frame_numbers.as_deref() != Some(SOURCE_FRAME_NUMBERS.as_slice())
    {
        return Err(invalid(
            "WSI tile segmentation requires the exact tiled-full source and Frames 1 and 4",
        ));
    }
    if input.source.series_instance_uid.is_none() {
        return Err(invalid(
            "WSI tile segmentation source series UID is required",
        ));
    }
    Ok(())
}

fn build_request(
    input: &WsiTileSegmentationGenerationInput,
) -> Result<Value, BackendContractError> {
    let mut request = json!({
        "request_schema_version": PROTOCOL_VERSION,
        "protocol_version": PROTOCOL_VERSION,
        "request_id": "0".repeat(64),
        "backend_id": BACKEND_ID,
        "case": {
            "case_id": CASE_ID,
            "recipe_id": RECIPE_ID,
            "recipe_version": RECIPE_VERSION,
            "profile": "extended",
            "expected_sop_class_uid": SOP_CLASS_UID,
            "expected_transfer_syntax_uid": TRANSFER_SYNTAX_UID,
        },
        "run": {"seed": input.seed},
        "standards": {
            "standards_lock_sha256": input.standards.standards_lock_sha256,
            "dicom_base_edition": input.standards.dicom_base_edition,
            "kb_source_manifest_sha256": input.standards.kb_source_manifest_sha256,
        },
        "staging": {"root": ".", "inputs_directory": "inputs", "output_directory": "outputs"},
        "identities": {
            "study_instance_uid": input.identities.study_instance_uid,
            "series_instance_uid": input.identities.series_instance_uid,
            "frame_of_reference_uid": input.identities.frame_of_reference_uid,
            "sop_instances": [{"role": "primary", "index": 0, "uid": input.identities.sop_instance_uid}],
        },
        "controlled_metadata": {
            "patient_name": input.controlled_metadata.patient_name,
            "patient_id": input.controlled_metadata.patient_id,
            "manufacturer": input.controlled_metadata.manufacturer,
            "model_name": input.controlled_metadata.model_name,
            "software_versions": input.controlled_metadata.software_versions,
            "study_date": input.controlled_metadata.study_date,
            "study_time": input.controlled_metadata.study_time,
            "content_date": input.controlled_metadata.content_date,
            "content_time": input.controlled_metadata.content_time,
            "timezone_offset_from_utc": input.controlled_metadata.timezone_offset_from_utc,
        },
        "sources": [{
            "role": input.source.role,
            "source_case_id": input.source.source_case_id,
            "relative_path": input.source.relative_path,
            "sha256": input.source.sha256,
            "sop_class_uid": input.source.sop_class_uid,
            "sop_instance_uid": input.source.sop_instance_uid,
            "series_instance_uid": input.source.series_instance_uid,
            "frame_numbers": input.source.frame_numbers,
        }],
        "parameters": {
            "dimension_organization_uid": input.identities.dimension_organization_uid,
            "segmentation_type": "FRACTIONAL",
            "fractional_type": "OCCUPANCY",
            "maximum_fractional_value": 255,
        },
        "requested_determinism": "semantic_stable",
    });
    let identity_material =
        serde_json::to_vec(&request).map_err(|source| BackendContractError::Parse {
            label: "WSI tile segmentation request identity".to_string(),
            source,
        })?;
    request["request_id"] = Value::String(sha256_hex(&identity_material));
    Ok(request)
}

fn verify_backend_response(
    response: &Value,
    input: &WsiTileSegmentationGenerationInput,
) -> Result<(), BackendContractError> {
    let outputs = response["outputs"]
        .as_array()
        .expect("response schema checked outputs");
    if outputs.len() != 1 {
        return Err(invalid(
            "WSI tile segmentation backend must produce exactly one output",
        ));
    }
    let output = &outputs[0];
    let source_series = input
        .source
        .series_instance_uid
        .as_deref()
        .expect("input validated source series");
    let expected = json!({
        "relative_path": OUTPUT_FILE,
        "sop_class_uid": SOP_CLASS_UID,
        "sop_instance_uid": input.identities.sop_instance_uid,
        "transfer_syntax_uid": TRANSFER_SYNTAX_UID,
        "references": [{
            "role": "source_image",
            "relationship": "derivation",
            "sop_class_uid": input.source.sop_class_uid,
            "sop_instance_uid": input.source.sop_instance_uid,
            "series_instance_uid": source_series,
            "frame_numbers": SOURCE_FRAME_NUMBERS,
        }],
        "expected_semantics": {
            "rows": 2,
            "columns": 2,
            "frames": 2,
            "total_pixel_matrix_rows": 4,
            "total_pixel_matrix_columns": 4,
            "dimension_organization_type": "TILED_SPARSE",
            "segmentation_type": "FRACTIONAL",
            "fractional_type": "OCCUPANCY",
            "maximum_fractional_value": 255,
            "segment_number": 1,
            "dimension_organization_uid": input.identities.dimension_organization_uid,
            "dimension_indices": [
                "ReferencedSegmentNumber",
                "RowPositionInTotalImagePixelMatrix",
                "ColumnPositionInTotalImagePixelMatrix"
            ],
            "dimension_index_values": [[1, 1, 1], [1, 2, 2]],
            "positions": [
                {"source_frame_number": 1, "row_position": 1, "column_position": 1, "x_offset": "0", "y_offset": "0", "z_offset": "0"},
                {"source_frame_number": 4, "row_position": 3, "column_position": 3, "x_offset": "1", "y_offset": "1", "z_offset": "0"}
            ]
        },
        "payload_expectations": {
            "vr": "OB",
            "frame_values": FRAME_VALUES,
            "frame_sha256": FRAME_SHA256,
            "payload_sha256": PAYLOAD_SHA256,
            "value_length": 8,
            "reconstructed_total_pixel_matrix_sha256": MATRIX_SHA256,
            "reconstructed_shape": [4, 4]
        }
    });
    if output != &expected {
        return Err(invalid(format!(
            "WSI tile segmentation response expectations differ: expected {expected}, got {output}"
        )));
    }
    Ok(())
}

fn verify_dicom_payload(
    path: &std::path::Path,
    input: &WsiTileSegmentationGenerationInput,
) -> Result<(), BackendContractError> {
    let object = open_file(path)
        .map_err(|error| invalid(format!("reopen staged WSI tile segmentation: {error}")))?;
    if object.meta().transfer_syntax() != TRANSFER_SYNTAX_UID {
        return Err(invalid("WSI tile segmentation transfer syntax differs"));
    }
    for (tag, label, expected) in [
        (tags::SOP_CLASS_UID, "SOP Class UID", SOP_CLASS_UID),
        (
            tags::SOP_INSTANCE_UID,
            "SOP Instance UID",
            input.identities.sop_instance_uid.as_str(),
        ),
        (tags::MODALITY, "Modality", "SEG"),
    ] {
        let actual = object
            .element(tag)
            .map_err(|error| invalid(format!("read {label}: {error}")))?
            .to_str()
            .map_err(|error| invalid(format!("decode {label}: {error}")))?;
        if actual.trim_end_matches([' ', '\0']) != expected {
            return Err(invalid(format!("{label} differs from request")));
        }
    }
    let pixel_bytes = object
        .element(tags::PIXEL_DATA)
        .map_err(|error| invalid(format!("read Pixel Data: {error}")))?
        .value()
        .to_bytes()
        .map_err(|error| invalid(format!("decode Pixel Data: {error}")))?;
    let expected = FRAME_VALUES.into_iter().flatten().collect::<Vec<_>>();
    if pixel_bytes.as_ref() != expected || sha256_hex(&pixel_bytes) != PAYLOAD_SHA256 {
        return Err(invalid(
            "WSI tile segmentation Pixel Data differs from the Rust contract",
        ));
    }
    Ok(())
}

fn invalid(message: impl Into<String>) -> BackendContractError {
    BackendContractError::Invalid {
        label: "WSI tile segmentation backend contract".to_string(),
        problems: vec![message.into()],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input() -> WsiTileSegmentationGenerationInput {
        WsiTileSegmentationGenerationInput {
            repository_root: PathBuf::from("."),
            generated_root: PathBuf::from("generated"),
            staging_root: PathBuf::from("staging"),
            destination_root: PathBuf::from(CASE_ID),
            seed: 7,
            standards: StandardsProvenance {
                standards_lock_sha256: "a".repeat(64),
                dicom_base_edition: "2026b".to_string(),
                kb_source_manifest_sha256: Some("b".repeat(64)),
            },
            controlled_metadata: ControlledMetadata {
                patient_name: "DTS^Synthetic".to_string(),
                patient_id: "DTS-PATIENT".to_string(),
                manufacturer: "dicom-test-suite".to_string(),
                model_name: RECIPE_ID.to_string(),
                software_versions: "0.1.0".to_string(),
                study_date: "20260101".to_string(),
                study_time: "000000".to_string(),
                content_date: "20260101".to_string(),
                content_time: "000000".to_string(),
                timezone_offset_from_utc: "+0000".to_string(),
            },
            identities: WsiTileSegmentationIdentities {
                study_instance_uid: "2.25.1".to_string(),
                series_instance_uid: "2.25.2".to_string(),
                frame_of_reference_uid: "2.25.3".to_string(),
                sop_instance_uid: "2.25.4".to_string(),
                dimension_organization_uid: "2.25.5".to_string(),
            },
            source: ParametricMapSource {
                role: "source_image".to_string(),
                source_case_id: SOURCE_CASE_ID.to_string(),
                relative_path: format!("{SOURCE_CASE_ID}/instance.dcm"),
                sha256: "c".repeat(64),
                sop_class_uid: SOURCE_SOP_CLASS_UID.to_string(),
                sop_instance_uid: "2.25.6".to_string(),
                series_instance_uid: Some("2.25.7".to_string()),
                frame_numbers: Some(SOURCE_FRAME_NUMBERS.to_vec()),
            },
        }
    }

    #[test]
    fn request_locks_sparse_fractional_recipe_and_source_frames() {
        let input = input();
        validate_input(&input).expect("locked input");
        let request = build_request(&input).expect("request");
        assert_eq!(request["case"]["case_id"], CASE_ID);
        assert_eq!(request["parameters"]["segmentation_type"], "FRACTIONAL");
        assert_eq!(request["parameters"]["fractional_type"], "OCCUPANCY");
        assert_eq!(request["sources"][0]["frame_numbers"], json!([1, 4]));
        assert_ne!(request["request_id"], "0".repeat(64));
        assert_eq!(build_request(&input).unwrap(), request);
    }

    #[test]
    fn input_rejects_relinked_source_frames() {
        let mut input = input();
        input.source.frame_numbers = Some(vec![1, 3]);
        let error = validate_input(&input).expect_err("wrong source frames must fail");
        assert!(error.to_string().contains("exact tiled-full source"));
    }
}
