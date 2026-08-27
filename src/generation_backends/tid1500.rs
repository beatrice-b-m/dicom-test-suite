//! Rust-side orchestration for the external TID 1500 measurement report case.

use std::fs;
use std::path::PathBuf;

use dicom_dictionary_std::tags;
use dicom_object::open_file;
use serde_json::{Value, json};

use super::{
    BACKEND_LOCK_FILE, BackendContractError, BackendDiscovery, BackendInvocation,
    ControlledMetadata, PROTOCOL_VERSION, ParametricMapSource, PreparedBackend,
    StandardsProvenance, backend_policy, discover_prepared_backend, invoke_backend,
    load_backend_lock, promote_staged_outputs,
};
use crate::sha256_hex;

pub const CASE_ID: &str = "derived/sr/tid1500_ct_measurement_report";
pub const RECIPE_ID: &str = "derived_sr_tid1500_ct_measurement_report";
pub const RECIPE_VERSION: &str = "0.1.0";
pub const OUTPUT_FILE: &str = "measurement-report.dcm";
pub const SOP_CLASS_UID: &str = "1.2.840.10008.5.1.4.1.1.88.34";
pub const TRANSFER_SYNTAX_UID: &str = "1.2.840.10008.1.2.1";
pub const TRACKING_IDENTIFIER: &str = "DTS-TID1500-ROI-1";
pub const MEASUREMENT_VALUE: f64 = 5.625;
const BACKEND_ID: &str = "highdicom_pydicom";

#[derive(Debug, Clone)]
pub struct Tid1500Identities {
    pub study_instance_uid: String,
    pub series_instance_uid: String,
    pub frame_of_reference_uid: String,
    pub sop_instance_uid: String,
    pub tracking_uid: String,
    pub observer_uid: String,
}

#[derive(Debug, Clone)]
pub struct Tid1500GenerationInput {
    pub repository_root: PathBuf,
    pub generated_root: PathBuf,
    pub staging_root: PathBuf,
    pub destination_root: PathBuf,
    pub seed: u64,
    pub standards: StandardsProvenance,
    pub controlled_metadata: ControlledMetadata,
    pub identities: Tid1500Identities,
    /// Ordered Enhanced CT then binary SEG sources.
    pub sources: Vec<ParametricMapSource>,
}

#[derive(Debug)]
pub struct Tid1500Generated {
    pub output_path: PathBuf,
    pub output_bytes: Vec<u8>,
    pub response: Value,
    pub backend: PreparedBackend,
    pub identities: Tid1500Identities,
}

#[derive(Debug)]
pub enum Tid1500Outcome {
    Generated(Tid1500Generated),
    Unavailable { code: String, message: String },
}

pub fn generate_tid1500(
    input: &Tid1500GenerationInput,
) -> Result<Tid1500Outcome, BackendContractError> {
    validate_input(input)?;
    let lock = load_backend_lock(&input.repository_root)?;
    let policy = backend_policy(&lock, BACKEND_ID)
        .ok_or_else(|| invalid(format!("{BACKEND_LOCK_FILE} has no {BACKEND_ID} policy")))?;
    let backend = match discover_prepared_backend(&input.repository_root, policy)? {
        BackendDiscovery::Available(backend) => backend,
        BackendDiscovery::Unavailable { code, message } => {
            return Ok(Tid1500Outcome::Unavailable { code, message });
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
            Ok(Tid1500Outcome::Unavailable {
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
            verify_response(&run.response, input)?;
            let staged_path = run.staging_root.join("outputs").join(OUTPUT_FILE);
            let output_bytes =
                fs::read(&staged_path).map_err(|source| BackendContractError::Read {
                    path: staged_path.clone(),
                    source,
                })?;
            verify_staged_object(&staged_path, input)?;
            promote_staged_outputs(&run.staging_root.join("outputs"), &input.destination_root)?;
            Ok(Tid1500Outcome::Generated(Tid1500Generated {
                output_path: input.destination_root.join(OUTPUT_FILE),
                output_bytes,
                response: run.response,
                backend,
                identities: input.identities.clone(),
            }))
        }
        status => Err(invalid(format!("unexpected backend status {status}"))),
    }
}

fn validate_input(input: &Tid1500GenerationInput) -> Result<(), BackendContractError> {
    if input.sources.len() != 2 {
        return Err(invalid("TID 1500 requires exactly two ordered sources"));
    }
    let ct = &input.sources[0];
    let seg = &input.sources[1];
    if ct.role != "source_image"
        || ct.source_case_id != "enhanced/ct/multiframe_shared_perframe_explicit_le"
        || ct.sop_class_uid != "1.2.840.10008.5.1.4.1.1.2.1"
        || ct.frame_numbers.as_deref() != Some(&[1, 2])
    {
        return Err(invalid(
            "TID 1500 first source must be Enhanced CT frames 1 and 2",
        ));
    }
    if seg.role != "segmentation"
        || seg.source_case_id != "derived/seg/binary_multiframe_explicit_le"
        || seg.sop_class_uid != "1.2.840.10008.5.1.4.1.1.66.4"
        || seg.frame_numbers.is_some()
    {
        return Err(invalid(
            "TID 1500 second source must be all frames of binary SEG segment 1",
        ));
    }
    if ct.series_instance_uid.is_none() || seg.series_instance_uid.is_none() {
        return Err(invalid(
            "TID 1500 sources must declare Series Instance UIDs",
        ));
    }
    Ok(())
}

fn build_request(input: &Tid1500GenerationInput) -> Result<Value, BackendContractError> {
    let sources = input
        .sources
        .iter()
        .map(|source| {
            json!({
                "role": source.role,
                "source_case_id": source.source_case_id,
                "relative_path": source.relative_path,
                "sha256": source.sha256,
                "sop_class_uid": source.sop_class_uid,
                "sop_instance_uid": source.sop_instance_uid,
                "series_instance_uid": source.series_instance_uid,
                "frame_numbers": source.frame_numbers,
            })
        })
        .collect::<Vec<_>>();
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
        "run": { "seed": input.seed },
        "standards": {
            "standards_lock_sha256": input.standards.standards_lock_sha256,
            "dicom_base_edition": input.standards.dicom_base_edition,
            "kb_source_manifest_sha256": input.standards.kb_source_manifest_sha256,
        },
        "staging": { "root": ".", "inputs_directory": "inputs", "output_directory": "outputs" },
        "identities": {
            "study_instance_uid": input.identities.study_instance_uid,
            "series_instance_uid": input.identities.series_instance_uid,
            "frame_of_reference_uid": input.identities.frame_of_reference_uid,
            "sop_instances": [{ "role": "primary", "index": 0, "uid": input.identities.sop_instance_uid }],
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
        "sources": sources,
        "parameters": {
            "segment_number": 1,
            "measurement_value": MEASUREMENT_VALUE,
            "tracking_identifier": TRACKING_IDENTIFIER,
            "tracking_uid": input.identities.tracking_uid,
            "observer_uid": input.identities.observer_uid,
        },
        "requested_determinism": "semantic_stable",
    });
    let identity_material =
        serde_json::to_vec(&request).map_err(|source| BackendContractError::Parse {
            label: "TID 1500 request identity".to_string(),
            source,
        })?;
    request["request_id"] = Value::String(sha256_hex(&identity_material));
    Ok(request)
}

fn verify_response(
    response: &Value,
    input: &Tid1500GenerationInput,
) -> Result<(), BackendContractError> {
    let outputs = response["outputs"]
        .as_array()
        .expect("response schema checked outputs");
    if outputs.len() != 1 {
        return Err(invalid("TID 1500 backend must declare exactly one output"));
    }
    let output = &outputs[0];
    if output["relative_path"] != OUTPUT_FILE
        || output["sop_class_uid"] != SOP_CLASS_UID
        || output["sop_instance_uid"] != input.identities.sop_instance_uid
        || output["transfer_syntax_uid"] != TRANSFER_SYNTAX_UID
        || output.pointer("/expected_semantics/root_template_identifier") != Some(&json!("1500"))
        || output.pointer("/expected_semantics/measurement_group_template_identifier")
            != Some(&json!("1411"))
        || output.pointer("/expected_semantics/tracking_identifier")
            != Some(&json!(TRACKING_IDENTIFIER))
        || output.pointer("/expected_semantics/tracking_uid")
            != Some(&json!(input.identities.tracking_uid))
        || output.pointer("/expected_semantics/observer_uid")
            != Some(&json!(input.identities.observer_uid))
        || output.pointer("/expected_semantics/measurement/value")
            != Some(&json!(MEASUREMENT_VALUE))
        || output.pointer("/payload_expectations/pixel_data") != Some(&json!("absent"))
    {
        return Err(invalid(
            "TID 1500 backend response semantics differ from request",
        ));
    }
    let references = output["references"]
        .as_array()
        .expect("response schema checked references");
    if references.len() != 2
        || references[0]["sop_instance_uid"] != input.sources[0].sop_instance_uid
        || references[0]["frame_numbers"] != json!([1, 2])
        || references[1]["sop_instance_uid"] != input.sources[1].sop_instance_uid
        || !references[1]["frame_numbers"].is_null()
    {
        return Err(invalid(
            "TID 1500 backend response references differ from inputs",
        ));
    }
    Ok(())
}

fn verify_staged_object(
    path: &std::path::Path,
    input: &Tid1500GenerationInput,
) -> Result<(), BackendContractError> {
    let object = open_file(path)
        .map_err(|error| invalid(format!("reopen staged TID 1500 output: {error}")))?;
    if object.meta().media_storage_sop_class_uid() != SOP_CLASS_UID
        || object.meta().media_storage_sop_instance_uid() != input.identities.sop_instance_uid
        || object.meta().transfer_syntax() != TRANSFER_SYNTAX_UID
        || object
            .element(tags::SOP_CLASS_UID)
            .ok()
            .and_then(|element| element.to_str().ok())
            .as_deref()
            .map(str::trim)
            != Some(SOP_CLASS_UID)
        || object
            .element_opt(tags::PIXEL_DATA)
            .ok()
            .flatten()
            .is_some()
        || object
            .element_opt(tags::FLOAT_PIXEL_DATA)
            .ok()
            .flatten()
            .is_some()
        || object
            .element_opt(tags::DOUBLE_FLOAT_PIXEL_DATA)
            .ok()
            .flatten()
            .is_some()
    {
        return Err(invalid(
            "staged TID 1500 Part 10 identity or payload is invalid",
        ));
    }
    Ok(())
}

fn invalid(message: impl Into<String>) -> BackendContractError {
    BackendContractError::Invalid {
        label: "TID 1500 backend contract".to_string(),
        problems: vec![message.into()],
    }
}
