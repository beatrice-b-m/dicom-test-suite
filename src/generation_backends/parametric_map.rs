//! Rust-side orchestration for the external float32 Parametric Map proof case.

use std::fs;
use std::path::{Path, PathBuf};

use dicom_dictionary_std::tags;
use dicom_object::open_file;
use serde_json::{Value, json};

use crate::sha256_hex;

use super::{
    BackendContractError, BackendDiscovery, BackendInvocation, PROTOCOL_VERSION, PreparedBackend,
    backend_policy, discover_prepared_backend, invoke_backend, load_backend_lock,
    promote_staged_outputs,
};

pub const BACKEND_ID: &str = "highdicom_pydicom";
pub const CASE_ID: &str = "derived/parametric-map/float32_ct_derived_explicit_le";
pub const RECIPE_ID: &str = "derived_parametric_map_float32_ct_derived_explicit_le";
pub const RECIPE_VERSION: &str = "0.1.0";
pub const SOP_CLASS_UID: &str = "1.2.840.10008.5.1.4.1.1.30";
pub const TRANSFER_SYNTAX_UID: &str = "1.2.840.10008.1.2.1";
pub const OUTPUT_FILE: &str = "parametric-map.dcm";

#[derive(Debug, Clone)]
pub struct StandardsProvenance {
    pub standards_lock_sha256: String,
    pub dicom_base_edition: String,
    pub kb_source_manifest_sha256: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ControlledMetadata {
    pub patient_name: String,
    pub patient_id: String,
    pub manufacturer: String,
    pub model_name: String,
    pub software_versions: String,
    pub study_date: String,
    pub study_time: String,
    pub content_date: String,
    pub content_time: String,
    pub timezone_offset_from_utc: String,
}

#[derive(Debug, Clone)]
pub struct ParametricMapIdentities {
    pub study_instance_uid: String,
    pub series_instance_uid: String,
    pub frame_of_reference_uid: String,
    pub sop_instance_uid: String,
    pub dimension_organization_uid: String,
}

#[derive(Debug, Clone)]
pub struct ParametricMapSource {
    pub role: String,
    pub source_case_id: String,
    pub relative_path: String,
    pub sha256: String,
    pub sop_class_uid: String,
    pub sop_instance_uid: String,
    pub series_instance_uid: Option<String>,
    pub frame_numbers: Option<Vec<u64>>,
}

#[derive(Debug, Clone)]
pub struct ParametricMapGenerationInput {
    pub repository_root: PathBuf,
    pub generated_root: PathBuf,
    pub staging_root: PathBuf,
    /// Final case directory. It must not exist before promotion.
    pub destination_root: PathBuf,
    pub seed: u64,
    pub standards: StandardsProvenance,
    pub controlled_metadata: ControlledMetadata,
    pub identities: ParametricMapIdentities,
    /// Spatially ordered single-frame CT source objects.
    pub sources: Vec<ParametricMapSource>,
    pub stored_value_scale: f32,
    pub spatial_rank_increment: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParametricMapFloatPayload {
    pub rows: u16,
    pub columns: u16,
    pub frames: usize,
    pub little_endian_bytes: Vec<u8>,
    pub little_endian_float32_bits: Vec<Vec<u32>>,
    pub frame_sha256: Vec<String>,
    pub minimum: f32,
    pub maximum: f32,
}

#[derive(Debug)]
pub struct ParametricMapGenerated {
    pub output_path: PathBuf,
    pub output_bytes: Vec<u8>,
    pub payload: ParametricMapFloatPayload,
    pub response: Value,
    pub backend: PreparedBackend,
    pub identities: ParametricMapIdentities,
}

#[derive(Debug)]
pub enum ParametricMapOutcome {
    Generated(ParametricMapGenerated),
    Unavailable { code: String, message: String },
}

pub fn generate_parametric_map(
    input: &ParametricMapGenerationInput,
) -> Result<ParametricMapOutcome, BackendContractError> {
    validate_input(input)?;
    let lock = load_backend_lock(&input.repository_root)?;
    let policy = backend_policy(&lock, BACKEND_ID)
        .ok_or_else(|| invalid(format!("backend lock has no {BACKEND_ID} policy")))?;
    let backend = match discover_prepared_backend(&input.repository_root, policy)? {
        BackendDiscovery::Available(backend) => backend,
        BackendDiscovery::Unavailable { code, message } => {
            return Ok(ParametricMapOutcome::Unavailable { code, message });
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
            Ok(ParametricMapOutcome::Unavailable {
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
            let payload = recompute_float_payload(
                &run.staging_root.join("inputs"),
                &input.sources,
                input.stored_value_scale,
                input.spatial_rank_increment,
            )?;
            let output = single_output(&run.response)?;
            verify_backend_expectations(output, &payload, &input.identities)?;
            let staged_path = run.staging_root.join("outputs").join(OUTPUT_FILE);
            let output_bytes =
                fs::read(&staged_path).map_err(|source| BackendContractError::Read {
                    path: staged_path,
                    source,
                })?;
            promote_staged_outputs(&run.staging_root.join("outputs"), &input.destination_root)?;
            Ok(ParametricMapOutcome::Generated(ParametricMapGenerated {
                output_path: input.destination_root.join(OUTPUT_FILE),
                output_bytes,
                payload,
                response: run.response,
                backend,
                identities: input.identities.clone(),
            }))
        }
        status => Err(invalid(format!("unexpected backend status {status}"))),
    }
}

fn validate_input(input: &ParametricMapGenerationInput) -> Result<(), BackendContractError> {
    if input.sources.len() != 3 {
        return Err(invalid(
            "Parametric Map proof requires exactly three CT sources",
        ));
    }
    if !input.stored_value_scale.is_finite() || !input.spatial_rank_increment.is_finite() {
        return Err(invalid("Parametric Map float parameters must be finite"));
    }
    Ok(())
}

fn build_request(input: &ParametricMapGenerationInput) -> Result<Value, BackendContractError> {
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
        // Logical locations keep the request identity independent of temporary paths.
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
            "stored_value_scale": input.stored_value_scale,
            "spatial_rank_increment": input.spatial_rank_increment,
            "dimension_organization_uid": input.identities.dimension_organization_uid,
        },
        "requested_determinism": "semantic_stable",
    });
    let identity_material =
        serde_json::to_vec(&request).map_err(|source| BackendContractError::Parse {
            label: "Parametric Map request identity".to_string(),
            source,
        })?;
    request["request_id"] = Value::String(sha256_hex(&identity_material));
    Ok(request)
}

fn recompute_float_payload(
    staged_inputs: &Path,
    sources: &[ParametricMapSource],
    scale: f32,
    increment: f32,
) -> Result<ParametricMapFloatPayload, BackendContractError> {
    let mut rows = None;
    let mut columns = None;
    let mut bytes = Vec::new();
    let mut all_bits = Vec::new();
    let mut hashes = Vec::new();
    let mut minimum = f32::INFINITY;
    let mut maximum = f32::NEG_INFINITY;
    for (rank, source) in sources.iter().enumerate() {
        let path = staged_inputs.join(&source.relative_path);
        let object = open_file(&path).map_err(|error| {
            invalid(format!(
                "reopen staged Parametric Map source {}: {error}",
                path.display()
            ))
        })?;
        if object.meta().transfer_syntax() != TRANSFER_SYNTAX_UID {
            return Err(invalid(format!(
                "source {} must use Explicit VR Little Endian",
                path.display()
            )));
        }
        let current_rows = element_u16(&object, tags::ROWS, "Rows")?;
        let current_columns = element_u16(&object, tags::COLUMNS, "Columns")?;
        if rows
            .replace(current_rows)
            .is_some_and(|value| value != current_rows)
            || columns
                .replace(current_columns)
                .is_some_and(|value| value != current_columns)
        {
            return Err(invalid("Parametric Map source dimensions differ"));
        }
        if element_u16(&object, tags::SAMPLES_PER_PIXEL, "Samples per Pixel")? != 1
            || element_u16(&object, tags::BITS_ALLOCATED, "Bits Allocated")? != 16
        {
            return Err(invalid(
                "Parametric Map sources must be monochrome 16-bit pixels",
            ));
        }
        let bits_stored = element_u16(&object, tags::BITS_STORED, "Bits Stored")?;
        let high_bit = element_u16(&object, tags::HIGH_BIT, "High Bit")?;
        let signed = element_u16(&object, tags::PIXEL_REPRESENTATION, "Pixel Representation")?;
        if !(1..=16).contains(&bits_stored) || high_bit + 1 != bits_stored || signed > 1 {
            return Err(invalid(
                "Parametric Map source stored-pixel encoding is unsupported",
            ));
        }
        let pixel_bytes = object
            .element(tags::PIXEL_DATA)
            .map_err(|error| invalid(format!("read source Pixel Data: {error}")))?
            .value()
            .to_bytes()
            .map_err(|error| invalid(format!("decode source Pixel Data: {error}")))?;
        let expected = usize::from(current_rows) * usize::from(current_columns) * 2;
        if pixel_bytes.len() != expected {
            return Err(invalid(format!(
                "source {} Pixel Data length is {}, expected {expected}",
                path.display(),
                pixel_bytes.len()
            )));
        }
        let mask = if bits_stored == 16 {
            u16::MAX
        } else {
            (1_u16 << bits_stored) - 1
        };
        let sign_bit = 1_u16 << (bits_stored - 1);
        let mut frame_bytes = Vec::with_capacity(expected * 2);
        let mut frame_bits = Vec::with_capacity(expected / 2);
        for pair in pixel_bytes.chunks_exact(2) {
            let raw = u16::from_le_bytes([pair[0], pair[1]]) & mask;
            let stored = if signed == 1 && raw & sign_bit != 0 {
                (i32::from(raw) - (1_i32 << bits_stored)) as f32
            } else {
                f32::from(raw)
            };
            let scaled = stored * scale;
            let offset = (rank as f32) * increment;
            let value = scaled + offset;
            if !value.is_finite() {
                return Err(invalid("derived Parametric Map value is not finite"));
            }
            minimum = minimum.min(value);
            maximum = maximum.max(value);
            let bits = value.to_bits();
            frame_bits.push(bits);
            frame_bytes.extend_from_slice(&bits.to_le_bytes());
        }
        hashes.push(sha256_hex(&frame_bytes));
        bytes.extend_from_slice(&frame_bytes);
        all_bits.push(frame_bits);
    }
    // highdicom 0.28.1 emits this axial source stack in DICOM spatial
    // dimension order, which is the reverse of the ascending source-path
    // order used to assign the deterministic spatial rank.
    let frame_length = usize::from(rows.expect("three sources"))
        * usize::from(columns.expect("three sources"))
        * 4;
    bytes = bytes
        .chunks_exact(frame_length)
        .rev()
        .flatten()
        .copied()
        .collect();
    all_bits.reverse();
    hashes.reverse();
    Ok(ParametricMapFloatPayload {
        rows: rows.expect("three sources"),
        columns: columns.expect("three sources"),
        frames: sources.len(),
        little_endian_bytes: bytes,
        little_endian_float32_bits: all_bits,
        frame_sha256: hashes,
        minimum,
        maximum,
    })
}

fn single_output(response: &Value) -> Result<&Value, BackendContractError> {
    let outputs = response["outputs"]
        .as_array()
        .expect("response schema checked outputs");
    if outputs.len() != 1 {
        return Err(invalid(
            "Parametric Map backend must produce exactly one output",
        ));
    }
    if outputs[0]["relative_path"].as_str() != Some(OUTPUT_FILE) {
        return Err(invalid(format!(
            "Parametric Map output must be named {OUTPUT_FILE}"
        )));
    }
    Ok(&outputs[0])
}

fn verify_backend_expectations(
    output: &Value,
    payload: &ParametricMapFloatPayload,
    identities: &ParametricMapIdentities,
) -> Result<(), BackendContractError> {
    let expected_bits =
        serde_json::to_value(&payload.little_endian_float32_bits).expect("u32 arrays serialize");
    let expected_hashes = serde_json::to_value(&payload.frame_sha256).expect("strings serialize");
    let checks = [
        (
            "payload VR",
            output.pointer("/payload_expectations/vr") == Some(&json!("OF")),
        ),
        (
            "payload float bits",
            output.pointer("/payload_expectations/little_endian_float32_bits")
                == Some(&expected_bits),
        ),
        (
            "payload frame hashes",
            output.pointer("/payload_expectations/frame_sha256") == Some(&expected_hashes),
        ),
        (
            "payload value length",
            output
                .pointer("/payload_expectations/value_length")
                .and_then(Value::as_u64)
                == Some(payload.little_endian_bytes.len() as u64),
        ),
        (
            "semantic rows",
            output
                .pointer("/expected_semantics/rows")
                .and_then(Value::as_u64)
                == Some(u64::from(payload.rows)),
        ),
        (
            "semantic columns",
            output
                .pointer("/expected_semantics/columns")
                .and_then(Value::as_u64)
                == Some(u64::from(payload.columns)),
        ),
        (
            "semantic frames",
            output
                .pointer("/expected_semantics/frames")
                .and_then(Value::as_u64)
                == Some(payload.frames as u64),
        ),
        (
            "semantic minimum",
            output
                .pointer("/expected_semantics/minimum")
                .and_then(Value::as_f64)
                == Some(f64::from(payload.minimum)),
        ),
        (
            "semantic maximum",
            output
                .pointer("/expected_semantics/maximum")
                .and_then(Value::as_f64)
                == Some(f64::from(payload.maximum)),
        ),
        (
            "dimension organization UID",
            output
                .pointer("/expected_semantics/dimension_organization_uid")
                .and_then(Value::as_str)
                == Some(identities.dimension_organization_uid.as_str()),
        ),
    ];
    let failures = checks
        .into_iter()
        .filter_map(|(label, passed)| (!passed).then_some(label))
        .collect::<Vec<_>>();
    if failures.is_empty() {
        Ok(())
    } else {
        Err(invalid(format!(
            "backend-authored expectations differ from independently recomputed values: {}",
            failures.join(", ")
        )))
    }
}

fn element_u16(
    object: &dicom_object::DefaultDicomObject,
    tag: dicom_core::Tag,
    label: &str,
) -> Result<u16, BackendContractError> {
    object
        .element(tag)
        .map_err(|error| invalid(format!("read source {label}: {error}")))?
        .to_int::<u16>()
        .map_err(|error| invalid(format!("decode source {label}: {error}")))
}

fn invalid(message: impl Into<String>) -> BackendContractError {
    BackendContractError::Invalid {
        label: "Parametric Map backend orchestration".to_string(),
        problems: vec![message.into()],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input() -> ParametricMapGenerationInput {
        ParametricMapGenerationInput {
            repository_root: PathBuf::from("repo-a"),
            generated_root: PathBuf::from("generated-a"),
            staging_root: PathBuf::from("staging-a"),
            destination_root: PathBuf::from("destination-a"),
            seed: 42,
            standards: StandardsProvenance {
                standards_lock_sha256: "a".repeat(64),
                dicom_base_edition: "2025e".to_string(),
                kb_source_manifest_sha256: None,
            },
            controlled_metadata: ControlledMetadata {
                patient_name: "DTS^Synthetic".to_string(),
                patient_id: "DTS-PM".to_string(),
                manufacturer: "DTS".to_string(),
                model_name: "Generator".to_string(),
                software_versions: "0.1.0".to_string(),
                study_date: "20260101".to_string(),
                study_time: "120000".to_string(),
                content_date: "20260101".to_string(),
                content_time: "120000".to_string(),
                timezone_offset_from_utc: "+0000".to_string(),
            },
            identities: ParametricMapIdentities {
                study_instance_uid: "1.2.3.1".to_string(),
                series_instance_uid: "1.2.3.2".to_string(),
                frame_of_reference_uid: "1.2.3.3".to_string(),
                sop_instance_uid: "1.2.3.4".to_string(),
                dimension_organization_uid: "1.2.3.5".to_string(),
            },
            sources: (0..3)
                .map(|index| ParametricMapSource {
                    role: "source_image".to_string(),
                    source_case_id: "geometry/ct/source".to_string(),
                    relative_path: format!("geometry/ct/source/slice-{index}.dcm"),
                    sha256: format!("{index:064x}"),
                    sop_class_uid: "1.2.840.10008.5.1.4.1.1.2".to_string(),
                    sop_instance_uid: format!("1.2.3.10{index}"),
                    series_instance_uid: Some("1.2.3.10".to_string()),
                    frame_numbers: None,
                })
                .collect(),
            stored_value_scale: 0.25,
            spatial_rank_increment: 0.5,
        }
    }

    #[test]
    fn request_identity_ignores_machine_specific_paths() {
        let first = build_request(&input()).expect("request should build");
        super::super::validate_request(&first).expect("request should satisfy protocol");

        let mut moved = input();
        moved.repository_root = PathBuf::from("other-repository");
        moved.generated_root = PathBuf::from("other-generated");
        moved.staging_root = PathBuf::from("other-staging");
        moved.destination_root = PathBuf::from("other-destination");
        let second = build_request(&moved).expect("moved request should build");

        assert_eq!(first["request_id"], second["request_id"]);
        assert_eq!(first["staging"], second["staging"]);
        assert_eq!(first["case"]["recipe_version"], RECIPE_VERSION);
    }

    #[test]
    fn rejects_backend_payload_claim_that_differs_from_recomputed_bits() {
        let payload = ParametricMapFloatPayload {
            rows: 1,
            columns: 1,
            frames: 1,
            little_endian_bytes: 1.5_f32.to_le_bytes().to_vec(),
            little_endian_float32_bits: vec![vec![1.5_f32.to_bits()]],
            frame_sha256: vec![sha256_hex(&1.5_f32.to_le_bytes())],
            minimum: 1.5,
            maximum: 1.5,
        };
        let identities = input().identities;
        let mut output = json!({
            "payload_expectations": {
                "vr": "OF",
                "little_endian_float32_bits": payload.little_endian_float32_bits,
                "frame_sha256": payload.frame_sha256,
                "value_length": 4,
            },
            "expected_semantics": {
                "rows": 1,
                "columns": 1,
                "frames": 1,
                "minimum": 1.5,
                "maximum": 1.5,
                "dimension_organization_uid": identities.dimension_organization_uid,
            }
        });
        output["payload_expectations"]["little_endian_float32_bits"][0][0] = json!(0);

        let error = verify_backend_expectations(&output, &payload, &identities)
            .expect_err("forged backend expectation must fail");
        assert!(error.to_string().contains("payload float bits"));
    }
}
