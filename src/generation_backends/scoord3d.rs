//! Rust-side orchestration for the external Comprehensive 3D SCOORD3D case.

use std::fs;
use std::path::{Component, Path, PathBuf};

use dicom_core::Tag;
use dicom_dictionary_std::StandardDataDictionary;
use dicom_dictionary_std::tags;
use dicom_object::{FileDicomObject, InMemDicomObject, open_file};
use serde_json::{Value, json};

use super::{
    BACKEND_LOCK_FILE, BackendContractError, BackendDiscovery, BackendInvocation,
    ControlledMetadata, PROTOCOL_VERSION, ParametricMapSource, PreparedBackend,
    StandardsProvenance, backend_policy, discover_prepared_backend, invoke_backend,
    load_backend_lock, promote_staged_outputs,
};
use crate::sha256_hex;

pub const CASE_ID: &str = "derived/sr/comprehensive3d_scoord3d";
pub const RECIPE_ID: &str = "derived_sr_comprehensive3d_scoord3d";
pub const RECIPE_VERSION: &str = "0.1.0";
pub const OUTPUT_FILE: &str = "scoord3d-report.dcm";
pub const SOP_CLASS_UID: &str = "1.2.840.10008.5.1.4.1.1.88.34";
pub const TRANSFER_SYNTAX_UID: &str = "1.2.840.10008.1.2.1";
pub const SOURCE_CASE_ID: &str = "enhanced/ct/multiframe_shared_perframe_explicit_le";
pub const SOURCE_SOP_CLASS_UID: &str = "1.2.840.10008.5.1.4.1.1.2.1";
pub const TRACKING_IDENTIFIER: &str = "DTS-SCOORD3D-ROI-1";
pub const GRAPHIC_TYPE: &str = "POLYLINE";
pub const GRAPHIC_DATA_PATIENT_MM: [[f64; 3]; 2] = [[0.0, 0.0, 0.0], [0.0, 0.0, 2.5]];
pub const MEASUREMENT_VALUE_MM: f64 = 2.5;
const BACKEND_ID: &str = "highdicom_pydicom";

#[derive(Debug, Clone)]
pub struct Scoord3dIdentities {
    pub study_instance_uid: String,
    pub series_instance_uid: String,
    pub frame_of_reference_uid: String,
    pub sop_instance_uid: String,
    pub tracking_uid: String,
    pub observer_uid: String,
    pub fiducial_uid: String,
}

#[derive(Debug, Clone)]
pub struct Scoord3dGenerationInput {
    pub repository_root: PathBuf,
    pub generated_root: PathBuf,
    pub staging_root: PathBuf,
    pub destination_root: PathBuf,
    pub seed: u64,
    pub standards: StandardsProvenance,
    pub controlled_metadata: ControlledMetadata,
    pub identities: Scoord3dIdentities,
    /// The Enhanced CT source, with frames 1 and 2 selected in that order.
    pub sources: Vec<ParametricMapSource>,
}

#[derive(Debug)]
pub struct Scoord3dGenerated {
    pub output_path: PathBuf,
    pub output_bytes: Vec<u8>,
    pub response: Value,
    pub backend: PreparedBackend,
    pub identities: Scoord3dIdentities,
}

#[derive(Debug)]
pub enum Scoord3dOutcome {
    Generated(Scoord3dGenerated),
    Unavailable { code: String, message: String },
}

pub fn generate_scoord3d(
    input: &Scoord3dGenerationInput,
) -> Result<Scoord3dOutcome, BackendContractError> {
    validate_input(input)?;
    validate_source_geometry(input)?;
    let lock = load_backend_lock(&input.repository_root)?;
    let policy = backend_policy(&lock, BACKEND_ID)
        .ok_or_else(|| invalid(format!("{BACKEND_LOCK_FILE} has no {BACKEND_ID} policy")))?;
    let backend = match discover_prepared_backend(&input.repository_root, policy)? {
        BackendDiscovery::Available(backend) => backend,
        BackendDiscovery::Unavailable { code, message } => {
            return Ok(Scoord3dOutcome::Unavailable { code, message });
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
            Ok(Scoord3dOutcome::Unavailable {
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
            Ok(Scoord3dOutcome::Generated(Scoord3dGenerated {
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

fn validate_input(input: &Scoord3dGenerationInput) -> Result<(), BackendContractError> {
    if input.sources.len() != 1 {
        return Err(invalid(
            "Comprehensive 3D SCOORD3D requires exactly one source",
        ));
    }
    let source = &input.sources[0];
    if source.role != "source_image"
        || source.source_case_id != SOURCE_CASE_ID
        || source.sop_class_uid != SOURCE_SOP_CLASS_UID
        || source.frame_numbers.as_deref() != Some(&[1, 2])
    {
        return Err(invalid(
            "Comprehensive 3D SCOORD3D source must be Enhanced CT frames 1 and 2",
        ));
    }
    if source.series_instance_uid.is_none() {
        return Err(invalid(
            "Comprehensive 3D SCOORD3D source must declare a Series Instance UID",
        ));
    }
    Ok(())
}

type DatasetObject = InMemDicomObject<StandardDataDictionary>;
type OpenedObject = FileDicomObject<DatasetObject>;

fn validate_source_geometry(input: &Scoord3dGenerationInput) -> Result<(), BackendContractError> {
    let source = &input.sources[0];
    let relative_path = Path::new(&source.relative_path);
    if relative_path.is_absolute()
        || relative_path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(invalid(
            "Enhanced CT source path must be a safe relative path",
        ));
    }
    let path = input.generated_root.join(relative_path);
    let object = open_file(&path).map_err(|error| {
        invalid(format!(
            "reopen Enhanced CT source {}: {error}",
            path.display()
        ))
    })?;

    if object.meta().media_storage_sop_class_uid() != SOURCE_SOP_CLASS_UID
        || object.meta().media_storage_sop_instance_uid() != source.sop_instance_uid
        || string_element(&object, tags::SOP_CLASS_UID)? != SOURCE_SOP_CLASS_UID
        || string_element(&object, tags::SOP_INSTANCE_UID)? != source.sop_instance_uid
        || string_element(&object, tags::STUDY_INSTANCE_UID)? != input.identities.study_instance_uid
        || string_element(&object, tags::FRAME_OF_REFERENCE_UID)?
            != input.identities.frame_of_reference_uid
        || string_element(&object, tags::SERIES_INSTANCE_UID)?
            != source
                .series_instance_uid
                .as_deref()
                .expect("input validation requires source Series Instance UID")
        || integer_element(&object, tags::NUMBER_OF_FRAMES)? != 2
    {
        return Err(invalid(
            "Enhanced CT source identity or frame count differs from SCOORD3D input",
        ));
    }

    let shared_items = top_level_sequence_items(&object, tags::SHARED_FUNCTIONAL_GROUPS_SEQUENCE)?;
    if shared_items.len() != 1 {
        return Err(invalid(
            "Enhanced CT source must have exactly one Shared Functional Groups item",
        ));
    }
    let shared = &shared_items[0];
    let pixel_measures = one_nested_item(shared, tags::PIXEL_MEASURES_SEQUENCE)?;
    if float_values(pixel_measures, tags::PIXEL_SPACING)? != [0.75, 0.75]
        || float_values(pixel_measures, tags::SLICE_THICKNESS)? != [2.5]
        || float_values(pixel_measures, tags::SPACING_BETWEEN_SLICES)? != [2.5]
    {
        return Err(invalid(
            "Enhanced CT source Pixel Measures differ from the SCOORD3D recipe",
        ));
    }
    let plane_orientation = one_nested_item(shared, tags::PLANE_ORIENTATION_SEQUENCE)?;
    if float_values(plane_orientation, tags::IMAGE_ORIENTATION_PATIENT)?
        != [1.0, 0.0, 0.0, 0.0, 1.0, 0.0]
    {
        return Err(invalid(
            "Enhanced CT source must use canonical axial orientation",
        ));
    }

    let per_frame = top_level_sequence_items(&object, tags::PER_FRAME_FUNCTIONAL_GROUPS_SEQUENCE)?;
    if per_frame.len() != 2 {
        return Err(invalid(
            "Enhanced CT source must have exactly two Per-frame Functional Groups items",
        ));
    }
    for (index, expected) in GRAPHIC_DATA_PATIENT_MM.iter().enumerate() {
        let plane_position = one_nested_item(&per_frame[index], tags::PLANE_POSITION_SEQUENCE)?;
        if float_values(plane_position, tags::IMAGE_POSITION_PATIENT)? != expected.as_slice() {
            return Err(invalid(format!(
                "Enhanced CT source frame {} position differs from the SCOORD3D recipe",
                index + 1
            )));
        }
    }
    Ok(())
}

fn string_element(object: &OpenedObject, tag: Tag) -> Result<String, BackendContractError> {
    object
        .element(tag)
        .map_err(|error| invalid(format!("read source attribute {tag}: {error}")))?
        .to_str()
        .map(|value| value.trim_matches('\0').trim().to_string())
        .map_err(|error| invalid(format!("decode source attribute {tag}: {error}")))
}

fn integer_element(object: &OpenedObject, tag: Tag) -> Result<u32, BackendContractError> {
    object
        .element(tag)
        .map_err(|error| invalid(format!("read source attribute {tag}: {error}")))?
        .to_int::<u32>()
        .map_err(|error| invalid(format!("decode source attribute {tag}: {error}")))
}

fn top_level_sequence_items(
    object: &OpenedObject,
    tag: Tag,
) -> Result<&[DatasetObject], BackendContractError> {
    object
        .element(tag)
        .map_err(|error| invalid(format!("read source sequence {tag}: {error}")))?
        .items()
        .ok_or_else(|| invalid(format!("source attribute {tag} is not a sequence")))
}

fn one_nested_item(
    object: &DatasetObject,
    tag: Tag,
) -> Result<&DatasetObject, BackendContractError> {
    let items = object
        .element(tag)
        .map_err(|error| invalid(format!("read source sequence {tag}: {error}")))?
        .items()
        .ok_or_else(|| invalid(format!("source attribute {tag} is not a sequence")))?;
    if items.len() != 1 {
        return Err(invalid(format!(
            "source sequence {tag} must contain exactly one item"
        )));
    }
    Ok(&items[0])
}

fn float_values(object: &DatasetObject, tag: Tag) -> Result<Vec<f64>, BackendContractError> {
    object
        .element(tag)
        .map_err(|error| invalid(format!("read source attribute {tag}: {error}")))?
        .value()
        .to_multi_float64()
        .map_err(|error| invalid(format!("decode source attribute {tag}: {error}")))
}

fn build_request(input: &Scoord3dGenerationInput) -> Result<Value, BackendContractError> {
    let source = &input.sources[0];
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
        "sources": [{
            "role": source.role,
            "source_case_id": source.source_case_id,
            "relative_path": source.relative_path,
            "sha256": source.sha256,
            "sop_class_uid": source.sop_class_uid,
            "sop_instance_uid": source.sop_instance_uid,
            "series_instance_uid": source.series_instance_uid,
            "frame_numbers": source.frame_numbers,
        }],
        "parameters": {
            "tracking_identifier": TRACKING_IDENTIFIER,
            "tracking_uid": input.identities.tracking_uid,
            "observer_uid": input.identities.observer_uid,
            "fiducial_uid": input.identities.fiducial_uid,
            "graphic_type": GRAPHIC_TYPE,
            "graphic_data_patient_mm": GRAPHIC_DATA_PATIENT_MM,
            "measurement_value_mm": MEASUREMENT_VALUE_MM,
        },
        "requested_determinism": "semantic_stable",
    });
    let identity_material =
        serde_json::to_vec(&request).map_err(|source| BackendContractError::Parse {
            label: "Comprehensive 3D SCOORD3D request identity".to_string(),
            source,
        })?;
    request["request_id"] = Value::String(sha256_hex(&identity_material));
    Ok(request)
}

fn verify_response(
    response: &Value,
    input: &Scoord3dGenerationInput,
) -> Result<(), BackendContractError> {
    let outputs = response["outputs"]
        .as_array()
        .expect("response schema checked outputs");
    if outputs.len() != 1 {
        return Err(invalid(
            "Comprehensive 3D SCOORD3D backend must declare exactly one output",
        ));
    }
    let output = &outputs[0];
    if output["relative_path"] != OUTPUT_FILE
        || output["sop_class_uid"] != SOP_CLASS_UID
        || output["sop_instance_uid"] != input.identities.sop_instance_uid
        || output["transfer_syntax_uid"] != TRANSFER_SYNTAX_UID
        || output.pointer("/expected_semantics/root_template_identifier") != Some(&json!("1500"))
        || output.pointer("/expected_semantics/measurement_group_template_identifier")
            != Some(&json!("1501"))
        || output.pointer("/expected_semantics/tracking_identifier")
            != Some(&json!(TRACKING_IDENTIFIER))
        || output.pointer("/expected_semantics/tracking_uid")
            != Some(&json!(input.identities.tracking_uid))
        || output.pointer("/expected_semantics/observer_uid")
            != Some(&json!(input.identities.observer_uid))
        || output.pointer("/expected_semantics/fiducial_uid")
            != Some(&json!(input.identities.fiducial_uid))
        || output.pointer("/expected_semantics/graphic_type") != Some(&json!(GRAPHIC_TYPE))
        || output.pointer("/expected_semantics/graphic_data_patient_mm")
            != Some(&json!(GRAPHIC_DATA_PATIENT_MM))
        || output.pointer("/expected_semantics/frame_of_reference_uid")
            != Some(&json!(input.identities.frame_of_reference_uid))
        || output.pointer("/expected_semantics/source_frame_numbers") != Some(&json!([1, 2]))
        || output.pointer("/expected_semantics/measurement/value")
            != Some(&json!(MEASUREMENT_VALUE_MM))
        || output.pointer("/payload_expectations/pixel_data") != Some(&json!("absent"))
    {
        return Err(invalid(
            "Comprehensive 3D SCOORD3D backend response semantics differ from request",
        ));
    }
    let references = output["references"]
        .as_array()
        .expect("response schema checked references");
    if references.len() != 1
        || references[0]["role"] != "source_image"
        || references[0]["relationship"] != "source_of_measurement"
        || references[0]["sop_class_uid"] != input.sources[0].sop_class_uid
        || references[0]["sop_instance_uid"] != input.sources[0].sop_instance_uid
        || references[0]["series_instance_uid"] != json!(input.sources[0].series_instance_uid)
        || references[0]["frame_numbers"] != json!([1, 2])
    {
        return Err(invalid(
            "Comprehensive 3D SCOORD3D backend response reference differs from input",
        ));
    }
    Ok(())
}

fn verify_staged_object(
    path: &std::path::Path,
    input: &Scoord3dGenerationInput,
) -> Result<(), BackendContractError> {
    let object = open_file(path).map_err(|error| {
        invalid(format!(
            "reopen staged Comprehensive 3D SCOORD3D output: {error}"
        ))
    })?;
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
            "staged Comprehensive 3D SCOORD3D Part 10 identity or payload is invalid",
        ));
    }
    Ok(())
}

fn invalid(message: impl Into<String>) -> BackendContractError {
    BackendContractError::Invalid {
        label: "Comprehensive 3D SCOORD3D backend contract".to_string(),
        problems: vec![message.into()],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input() -> Scoord3dGenerationInput {
        Scoord3dGenerationInput {
            repository_root: PathBuf::from("."),
            generated_root: PathBuf::from("generated"),
            staging_root: PathBuf::from("staging"),
            destination_root: PathBuf::from("destination"),
            seed: 7,
            standards: StandardsProvenance {
                standards_lock_sha256: "a".repeat(64),
                dicom_base_edition: "2026a".to_string(),
                kb_source_manifest_sha256: None,
            },
            controlled_metadata: ControlledMetadata {
                patient_name: "DTS^Synthetic^Patient001".to_string(),
                patient_id: "DTS-PATIENT-001".to_string(),
                manufacturer: "dicom-test-suite".to_string(),
                model_name: RECIPE_ID.to_string(),
                software_versions: RECIPE_VERSION.to_string(),
                study_date: "20260101".to_string(),
                study_time: "000000".to_string(),
                content_date: "20260101".to_string(),
                content_time: "000000".to_string(),
                timezone_offset_from_utc: "+0000".to_string(),
            },
            identities: Scoord3dIdentities {
                study_instance_uid: "2.25.1".to_string(),
                series_instance_uid: "2.25.2".to_string(),
                frame_of_reference_uid: "2.25.3".to_string(),
                sop_instance_uid: "2.25.4".to_string(),
                tracking_uid: "2.25.5".to_string(),
                observer_uid: "2.25.6".to_string(),
                fiducial_uid: "2.25.7".to_string(),
            },
            sources: vec![ParametricMapSource {
                role: "source_image".to_string(),
                source_case_id: SOURCE_CASE_ID.to_string(),
                relative_path: "enhanced-ct.dcm".to_string(),
                sha256: "b".repeat(64),
                sop_class_uid: SOURCE_SOP_CLASS_UID.to_string(),
                sop_instance_uid: "2.25.8".to_string(),
                series_instance_uid: Some("2.25.9".to_string()),
                frame_numbers: Some(vec![1, 2]),
            }],
        }
    }

    #[test]
    fn request_locks_source_frames_and_scoord3d_parameters() {
        let input = input();
        validate_input(&input).expect("valid source");
        let request = build_request(&input).expect("request");
        assert_eq!(
            request.pointer("/sources/0/frame_numbers"),
            Some(&json!([1, 2]))
        );
        assert_eq!(
            request.pointer("/parameters/graphic_data_patient_mm"),
            Some(&json!(GRAPHIC_DATA_PATIENT_MM))
        );
        assert_eq!(
            request.pointer("/parameters/fiducial_uid"),
            Some(&json!(input.identities.fiducial_uid))
        );
        assert_eq!(
            request.pointer("/parameters/measurement_value_mm"),
            Some(&json!(MEASUREMENT_VALUE_MM))
        );
    }

    #[test]
    fn input_rejects_source_frame_drift() {
        let mut input = input();
        input.sources[0].frame_numbers = Some(vec![2, 1]);
        let error = validate_input(&input).expect_err("reordered frames must fail");
        assert!(error.to_string().contains("frames 1 and 2"));
    }

    #[test]
    fn response_rejects_semantic_drift() {
        let input = input();
        let mut response = json!({
            "outputs": [{
                "relative_path": OUTPUT_FILE,
                "sop_class_uid": SOP_CLASS_UID,
                "sop_instance_uid": input.identities.sop_instance_uid,
                "transfer_syntax_uid": TRANSFER_SYNTAX_UID,
                "references": [{
                    "role": "source_image",
                    "relationship": "source_of_measurement",
                    "sop_class_uid": input.sources[0].sop_class_uid,
                    "sop_instance_uid": input.sources[0].sop_instance_uid,
                    "series_instance_uid": input.sources[0].series_instance_uid,
                    "frame_numbers": [1, 2]
                }],
                "expected_semantics": {
                    "root_template_identifier": "1500",
                    "measurement_group_template_identifier": "1501",
                    "tracking_identifier": TRACKING_IDENTIFIER,
                    "tracking_uid": input.identities.tracking_uid,
                    "observer_uid": input.identities.observer_uid,
                    "fiducial_uid": input.identities.fiducial_uid,
                    "graphic_type": GRAPHIC_TYPE,
                    "graphic_data_patient_mm": GRAPHIC_DATA_PATIENT_MM,
                    "frame_of_reference_uid": input.identities.frame_of_reference_uid,
                    "source_frame_numbers": [1, 2],
                    "measurement": { "value": MEASUREMENT_VALUE_MM }
                },
                "payload_expectations": { "pixel_data": "absent" }
            }]
        });
        verify_response(&response, &input).expect("matching response");
        response["outputs"][0]["expected_semantics"]["fiducial_uid"] = json!("2.25.99");
        let error = verify_response(&response, &input).expect_err("drift must fail");
        assert!(error.to_string().contains("semantics differ"));
    }
}
