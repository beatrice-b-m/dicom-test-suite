use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use dicom_core::Tag;
use dicom_core::ops::{ApplyOp, AttributeAction, AttributeOp};
use dicom_dictionary_std::{tags, uids};
use dicom_object::{InMemDicomObject, open_file};
use serde_json::Value;

const CASE_ID: &str = "derived/sr/tid1500_ct_measurement_report";
const RELATIVE_PATH: &str = "derived/sr/tid1500_ct_measurement_report/measurement-report.dcm";
const SOP_CLASS_UID: &str = "1.2.840.10008.5.1.4.1.1.88.34";
const CT_CASE_ID: &str = "enhanced/ct/multiframe_shared_perframe_explicit_le";
const SEG_CASE_ID: &str = "derived/seg/binary_multiframe_explicit_le";
const CT_SOP_CLASS_UID: &str = "1.2.840.10008.5.1.4.1.1.2.1";
const SEG_SOP_CLASS_UID: &str = "1.2.840.10008.5.1.4.1.1.66.4";
const TAG_REFERENCED_FRAME_NUMBER: Tag = Tag(0x0008, 0x1160);
const TAG_REFERENCED_SEGMENT_NUMBER: Tag = Tag(0x0062, 0x000b);

#[test]
fn tid1500_vertical_slice_is_byte_deterministic_and_strictly_validated() {
    let first_root = unique_temp_dir("tid1500-first");
    let second_root = unique_temp_dir("tid1500-second");
    let first_manifest = generate_extended(&first_root);
    let second_manifest = generate_extended(&second_root);

    for root in [&first_root, &second_root] {
        let validation = dicom_test_suite::validate_generated_root(root)
            .expect("generated extended root should validate");
        assert!(
            validation.failures.is_empty(),
            "generated root validation failed: {:?}",
            validation.failures
        );
    }

    match (
        file_for_case(&first_manifest),
        file_for_case(&second_manifest),
    ) {
        (Some(first), Some(second)) => {
            assert_eq!(
                first, second,
                "seed-7 TID 1500 manifest entries must be identical"
            );
            let first_bytes = fs::read(first_root.join(RELATIVE_PATH))
                .expect("first TID 1500 output should read");
            let second_bytes = fs::read(second_root.join(RELATIVE_PATH))
                .expect("second TID 1500 output should read");
            assert_eq!(
                first_bytes, second_bytes,
                "the controlled highdicom output must be byte deterministic"
            );
            assert_eq!(
                first["sha256"].as_str(),
                Some(dicom_test_suite::sha256_hex(&first_bytes).as_str())
            );
            assert_generated_manifest_contract(&first_root, first);
            assert_generated_dicom_contract(&first_root, first);
            assert_semantic_mutation_is_rejected(&first_root, &first_manifest);
        }
        (None, None) => {
            let first = assert_explicitly_unavailable(&first_manifest, &first_root);
            let second = assert_explicitly_unavailable(&second_manifest, &second_root);
            assert_eq!(
                first, second,
                "backend unavailability must be deterministic and explicit"
            );
        }
        _ => panic!("the same locked backend must not change availability between runs"),
    }

    fs::remove_dir_all(first_root).expect("first temporary root should be removable");
    fs::remove_dir_all(second_root).expect("second temporary root should be removable");
}

fn assert_generated_manifest_contract(root: &Path, file: &Value) {
    assert_eq!(file["case_id"].as_str(), Some(CASE_ID));
    assert_eq!(file["profile_membership"], serde_json::json!(["extended"]));
    assert_eq!(file["path"].as_str(), Some(RELATIVE_PATH));
    assert_eq!(file["determinism"].as_str(), Some("semantic_stable"));
    assert_eq!(
        file.pointer("/recipe/recipe_id").and_then(Value::as_str),
        Some("derived_sr_tid1500_ct_measurement_report")
    );
    assert_eq!(
        file.pointer("/recipe/recipe_version")
            .and_then(Value::as_str),
        Some("0.1.0")
    );
    assert_eq!(
        file.pointer("/recipe/recipe_parameters/segment_number")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        file.pointer("/recipe/recipe_parameters/measurement_value_mm3")
            .and_then(Value::as_f64),
        Some(5.625)
    );
    assert_eq!(
        file.pointer("/recipe/recipe_parameters/tracking_identifier")
            .and_then(Value::as_str),
        Some("DTS-TID1500-ROI-1")
    );
    assert_eq!(
        file.pointer("/recipe/recipe_parameters/source_frame_numbers"),
        Some(&serde_json::json!([1, 2]))
    );
    assert_eq!(
        file.pointer("/dicom/sop_class_uid").and_then(Value::as_str),
        Some(SOP_CLASS_UID)
    );
    assert_eq!(
        file.pointer("/dicom/sop_class_name")
            .and_then(Value::as_str),
        Some("Comprehensive 3D SR Storage")
    );
    assert_eq!(
        file.pointer("/dicom/iod_name").and_then(Value::as_str),
        Some("Comprehensive 3D SR")
    );
    assert_eq!(
        file.pointer("/dicom/modality").and_then(Value::as_str),
        Some("SR")
    );
    assert_eq!(
        file.pointer("/dicom/transfer_syntax_uid")
            .and_then(Value::as_str),
        Some(uids::EXPLICIT_VR_LITTLE_ENDIAN)
    );
    assert!(file["image"].is_null());
    assert!(file["pixel_data"].is_null());
    assert_uid(file, "/uids/study_instance_uid");
    assert_uid(file, "/uids/series_instance_uid");
    assert_uid(file, "/uids/sop_instance_uid");
    assert_uid(file, "/uids/frame_of_reference_uid");
    assert_uid(file, "/uids/implementation_class_uid");
    assert_eq!(
        file.pointer("/uids/implementation_version_name")
            .and_then(Value::as_str),
        Some("highdicom0.28.1")
    );

    let backend = file["generation_backend"]
        .as_object()
        .expect("external backend provenance must be typed");
    assert_eq!(backend["backend_id"].as_str(), Some("highdicom_pydicom"));
    assert_eq!(backend["protocol_version"].as_str(), Some("0.1.0"));
    assert_eq!(backend["determinism"].as_str(), Some("semantic_stable"));
    assert!(
        backend["version"]
            .as_str()
            .is_some_and(|value| value.contains("highdicom.0.28.1"))
    );
    for field in [
        "dependency_lock_sha256",
        "executable_fingerprint",
        "entrypoint_fingerprint",
        "environment_fingerprint",
    ] {
        assert_sha256(&backend[field], field);
    }
    assert_eq!(
        backend["runtime_identity"]["backend_id"].as_str(),
        Some("highdicom_pydicom")
    );
    assert_eq!(
        backend["runtime_identity"]["protocol_version"].as_str(),
        Some("0.1.0")
    );
    assert!(backend["warnings"].as_array().is_some_and(Vec::is_empty));

    let references = file["references"]
        .as_array()
        .expect("TID 1500 references must be an array");
    assert_eq!(references.len(), 2);
    assert_reference(
        root,
        &references[0],
        CT_CASE_ID,
        CT_SOP_CLASS_UID,
        "source_image_for_segmentation",
        Some(&[1, 2]),
    );
    assert_reference(
        root,
        &references[1],
        SEG_CASE_ID,
        SEG_SOP_CLASS_UID,
        "referenced_segment",
        None,
    );

    assert_eq!(
        file["expected_capabilities"],
        serde_json::json!([
            "open_file",
            "read_metadata",
            "parse_structured_report",
            "resolve_references",
            "interpret_tid1500_measurements"
        ])
    );
    assert_eq!(
        file.pointer("/expected_semantics/synthetic_data")
            .and_then(Value::as_str),
        Some("YES")
    );
    for (field, expected) in [
        ("completion_flag", "COMPLETE"),
        ("preliminary_flag", "FINAL"),
        ("verification_flag", "UNVERIFIED"),
        ("root_value_type", "CONTAINER"),
        ("root_continuity_of_content", "CONTINUOUS"),
    ] {
        assert_eq!(
            file.pointer(&format!("/expected_semantics/structured_report/{field}"))
                .and_then(Value::as_str),
            Some(expected)
        );
    }
    assert_eq!(
        file.pointer("/expected_semantics/structured_report/content_sequence_items")
            .and_then(Value::as_u64),
        Some(8)
    );

    let expected = &file["expected_tid1500"];
    assert_eq!(expected["completion_flag"], "COMPLETE");
    assert_eq!(expected["preliminary_flag"], "FINAL");
    assert_eq!(expected["verification_flag"], "UNVERIFIED");
    assert_eq!(
        expected.pointer("/root_template"),
        Some(&serde_json::json!({
            "mapping_resource": "DCMR",
            "template_identifier": "1500"
        }))
    );
    assert_manifest_code(
        &expected["document_title"],
        "126000",
        "DCM",
        "Imaging Measurement Report",
    );
    assert_eq!(
        expected
            .pointer("/observation_context/observer_type")
            .and_then(Value::as_str),
        Some("DEVICE")
    );
    assert_uid(expected, "/observation_context/device_observer_uid");
    assert_manifest_code(
        &expected["procedure_reported"],
        "25045-6",
        "LN",
        "CT unspecified body region",
    );
    assert_manifest_code(
        &expected["imaging_measurements"],
        "126010",
        "DCM",
        "Imaging Measurements",
    );
    let group = &expected["measurement_group"];
    assert_manifest_code(&group["container"], "125007", "DCM", "Measurement Group");
    assert_eq!(
        group["tracking_identifier"].as_str(),
        Some("DTS-TID1500-ROI-1")
    );
    assert_uid(group, "/tracking_uid");
    assert_manifest_code(&group["finding"], "123037004", "SCT", "Body structure");
    assert_manifest_code(&group["measurement"]["name"], "118565006", "SCT", "Volume");
    assert_eq!(
        group
            .pointer("/measurement/numeric_value")
            .and_then(Value::as_str),
        Some("5.625")
    );
    assert_manifest_code(
        &group["measurement"]["units"],
        "mm3",
        "UCUM",
        "cubic millimeter",
    );
    let segment = &group["referenced_segment"];
    assert_eq!(segment["source_case_id"], SEG_CASE_ID);
    assert_eq!(segment["sop_class_uid"], SEG_SOP_CLASS_UID);
    assert_eq!(segment["segment_number"], 1);
    assert!(segment["referenced_frame_numbers"].is_null());
    assert_eq!(segment["source_image"]["source_case_id"], CT_CASE_ID);
    assert_eq!(segment["source_image"]["sop_class_uid"], CT_SOP_CLASS_UID);
    assert_eq!(
        segment["source_image"]["referenced_frame_numbers"],
        serde_json::json!([1, 2])
    );
    assert_eq!(expected["evidence"].as_array().map(Vec::len), Some(2));

    assert_eq!(
        file.pointer("/expected_visual_checks/pattern")
            .and_then(Value::as_str),
        Some("tid1500_volume_measurement_from_binary_segmentation")
    );
    assert!(file["known_stressors"].as_array().is_some_and(|items| {
        items
            .iter()
            .any(|item| item == "tid1500_measurement_report")
    }));
    assert!(
        file["standards_evidence"]
            .as_array()
            .is_some_and(|items| !items.is_empty())
    );
    assert_eq!(
        file.pointer("/validation/status").and_then(Value::as_str),
        Some("passed")
    );
    assert!(
        file.pointer("/validation/internal")
            .and_then(Value::as_array)
            .is_some_and(|items| {
                !items.is_empty()
                    && items
                        .iter()
                        .all(|item| item["status"].as_str() == Some("passed"))
            })
    );
}

fn assert_generated_dicom_contract(root: &Path, file: &Value) {
    let object = open_file(root.join(RELATIVE_PATH)).expect("TID 1500 report should reopen");
    assert_eq!(object.meta().media_storage_sop_class_uid(), SOP_CLASS_UID);
    assert_eq!(
        object.meta().transfer_syntax(),
        uids::EXPLICIT_VR_LITTLE_ENDIAN
    );
    assert_eq!(text(&object, tags::VALUE_TYPE), "CONTAINER");
    assert_eq!(text(&object, tags::CONTINUITY_OF_CONTENT), "CONTINUOUS");
    assert_code(
        &object,
        tags::CONCEPT_NAME_CODE_SEQUENCE,
        "126000",
        "DCM",
        "Imaging Measurement Report",
    );
    let root_template = sequence(&object, tags::CONTENT_TEMPLATE_SEQUENCE);
    assert_eq!(root_template.len(), 1);
    assert_eq!(text(&root_template[0], tags::MAPPING_RESOURCE), "DCMR");
    assert_eq!(text(&root_template[0], tags::TEMPLATE_IDENTIFIER), "1500");

    let root_content = sequence(&object, tags::CONTENT_SEQUENCE);
    assert_eq!(root_content.len(), 8);
    let procedure = find_named_item(root_content, "121058");
    assert_code(
        procedure,
        tags::CONCEPT_CODE_SEQUENCE,
        "25045-6",
        "LN",
        "CT unspecified body region",
    );
    let imaging = find_named_item(root_content, "126010");
    let groups = sequence(imaging, tags::CONTENT_SEQUENCE);
    assert_eq!(groups.len(), 1);
    let group = &groups[0];
    assert_code(
        group,
        tags::CONCEPT_NAME_CODE_SEQUENCE,
        "125007",
        "DCM",
        "Measurement Group",
    );
    let group_template = sequence(group, tags::CONTENT_TEMPLATE_SEQUENCE);
    assert_eq!(text(&group_template[0], tags::TEMPLATE_IDENTIFIER), "1411");
    let group_content = sequence(group, tags::CONTENT_SEQUENCE);
    assert_eq!(group_content.len(), 6);
    assert_eq!(
        text(find_named_item(group_content, "112039"), tags::TEXT_VALUE),
        "DTS-TID1500-ROI-1"
    );
    assert_eq!(
        text(find_named_item(group_content, "112040"), tags::UID),
        file.pointer("/expected_tid1500/measurement_group/tracking_uid")
            .and_then(Value::as_str)
            .expect("tracking UID")
    );
    assert_code(
        find_named_item(group_content, "121071"),
        tags::CONCEPT_CODE_SEQUENCE,
        "123037004",
        "SCT",
        "Body structure",
    );

    let measurement = find_named_item(group_content, "118565006");
    let measured = sequence(measurement, tags::MEASURED_VALUE_SEQUENCE);
    assert_eq!(text(&measured[0], tags::NUMERIC_VALUE), "5.625");
    assert_eq!(
        measured[0]
            .element(tags::FLOATING_POINT_VALUE)
            .expect("floating point value")
            .to_float64()
            .expect("FD value"),
        5.625
    );
    assert_code(
        &measured[0],
        tags::MEASUREMENT_UNITS_CODE_SEQUENCE,
        "mm3",
        "UCUM",
        "cubic millimeter",
    );

    let segment = find_named_item(group_content, "121191");
    let segment_reference = sequence(segment, tags::REFERENCED_SOP_SEQUENCE);
    assert_eq!(
        text(&segment_reference[0], tags::REFERENCED_SOP_CLASS_UID),
        SEG_SOP_CLASS_UID
    );
    assert_eq!(
        segment_reference[0]
            .element(TAG_REFERENCED_SEGMENT_NUMBER)
            .expect("Referenced Segment Number")
            .to_int::<u16>()
            .expect("US value"),
        1
    );
    assert!(
        segment_reference[0]
            .element_opt(TAG_REFERENCED_FRAME_NUMBER)
            .expect("Referenced Frame Number lookup")
            .is_none()
    );
    let source = find_named_item(group_content, "121233");
    assert_code(
        source,
        tags::CONCEPT_NAME_CODE_SEQUENCE,
        "121233",
        "DCM",
        "Source image for segmentation",
    );
    let source_reference = sequence(source, tags::REFERENCED_SOP_SEQUENCE);
    assert_eq!(
        text(&source_reference[0], tags::REFERENCED_SOP_CLASS_UID),
        CT_SOP_CLASS_UID
    );
    assert_eq!(
        source_reference[0]
            .element(TAG_REFERENCED_FRAME_NUMBER)
            .expect("source frames")
            .to_multi_int::<u16>()
            .expect("IS source frames"),
        vec![1, 2]
    );

    let evidence = sequence(&object, tags::CURRENT_REQUESTED_PROCEDURE_EVIDENCE_SEQUENCE);
    assert_eq!(evidence.len(), 1);
    let evidence_series = sequence(&evidence[0], tags::REFERENCED_SERIES_SEQUENCE);
    assert_eq!(evidence_series.len(), 2);
    let evidence_classes = evidence_series
        .iter()
        .flat_map(|series| sequence(series, tags::REFERENCED_SOP_SEQUENCE))
        .map(|instance| text(instance, tags::REFERENCED_SOP_CLASS_UID))
        .collect::<BTreeSet<_>>();
    assert_eq!(
        evidence_classes,
        BTreeSet::from([CT_SOP_CLASS_UID.to_string(), SEG_SOP_CLASS_UID.to_string()])
    );
    for forbidden in [
        tags::PIXEL_DATA,
        tags::FLOAT_PIXEL_DATA,
        tags::DOUBLE_FLOAT_PIXEL_DATA,
    ] {
        assert!(
            object
                .element_opt(forbidden)
                .expect("pixel tag lookup")
                .is_none()
        );
    }
}

fn assert_semantic_mutation_is_rejected(root: &Path, pristine_manifest: &Value) {
    let path = root.join(RELATIVE_PATH);
    let mut object = open_file(&path).expect("TID 1500 report should reopen for mutation");
    object
        .apply(AttributeOp::new(
            (tags::CONCEPT_NAME_CODE_SEQUENCE, 0, tags::CODE_MEANING),
            AttributeAction::SetStr("Broken Measurement Report".into()),
        ))
        .expect("document title mutation should apply");
    object
        .write_to_file(&path)
        .expect("mutated report should write");
    let bytes = fs::read(&path).expect("mutated report should read");
    let mut manifest = pristine_manifest.clone();
    let file = manifest["files"]
        .as_array_mut()
        .expect("manifest files")
        .iter_mut()
        .find(|file| file["case_id"].as_str() == Some(CASE_ID))
        .expect("TID 1500 manifest entry");
    file["sha256"] = Value::String(dicom_test_suite::sha256_hex(&bytes));
    file["size_bytes"] = Value::from(bytes.len() as u64);
    fs::write(
        root.join("manifest.json"),
        serde_json::to_vec_pretty(&manifest).expect("manifest serialization"),
    )
    .expect("mutated manifest should write");

    let validation = dicom_test_suite::validate_generated_root(root)
        .expect("strict validator should complete over semantic mutation");
    assert!(
        validation
            .failures
            .iter()
            .any(|failure| failure.contains("tid1500_document_title")),
        "strict validation must detect the changed SR document title: {:?}",
        validation.failures
    );
}

fn assert_explicitly_unavailable<'a>(manifest: &'a Value, root: &Path) -> &'a Value {
    let row = manifest["skipped_cases"]
        .as_array()
        .expect("skipped cases array")
        .iter()
        .find(|row| row["case_id"].as_str() == Some(CASE_ID))
        .expect("implemented TID 1500 must have explicit unavailable state");
    assert_eq!(row["status"].as_str(), Some("unavailable"));
    assert_eq!(
        row["reason_code"].as_str(),
        Some("external_backend_unavailable")
    );
    assert_eq!(row["recheck_phase"].as_str(), Some("phase-3"));
    assert!(
        row["message"]
            .as_str()
            .is_some_and(|message| !message.is_empty())
    );
    assert!(!root.join(RELATIVE_PATH).exists());
    row
}

fn assert_reference(
    root: &Path,
    reference: &Value,
    source_case_id: &str,
    sop_class_uid: &str,
    relationship: &str,
    frame_numbers: Option<&[u64]>,
) {
    assert_eq!(reference["source_case_id"].as_str(), Some(source_case_id));
    assert_eq!(reference["sop_class_uid"].as_str(), Some(sop_class_uid));
    assert_eq!(reference["relationship"].as_str(), Some(relationship));
    assert_uid(reference, "/sop_instance_uid");
    assert_uid(reference, "/series_instance_uid");
    match frame_numbers {
        Some(expected) => assert_eq!(
            reference["frame_numbers"],
            Value::Array(expected.iter().copied().map(Value::from).collect())
        ),
        None => assert!(reference["frame_numbers"].is_null()),
    }
    let source_path = reference["source_path"]
        .as_str()
        .expect("reference source path");
    let source = open_file(root.join(source_path)).expect("manifest reference should resolve");
    assert_eq!(
        text(&source, tags::SOP_INSTANCE_UID),
        reference["sop_instance_uid"]
            .as_str()
            .expect("manifest source SOP UID")
    );
}

fn assert_manifest_code(value: &Value, code: &str, scheme: &str, meaning: &str) {
    assert_eq!(value["code_value"].as_str(), Some(code));
    assert_eq!(value["coding_scheme_designator"].as_str(), Some(scheme));
    assert_eq!(value["code_meaning"].as_str(), Some(meaning));
}

fn assert_code(object: &InMemDicomObject, tag: Tag, code: &str, scheme: &str, meaning: &str) {
    let items = sequence(object, tag);
    assert_eq!(items.len(), 1);
    assert_eq!(text(&items[0], tags::CODE_VALUE), code);
    assert_eq!(text(&items[0], tags::CODING_SCHEME_DESIGNATOR), scheme);
    assert_eq!(text(&items[0], tags::CODE_MEANING), meaning);
}

fn find_named_item<'a>(items: &'a [InMemDicomObject], code: &str) -> &'a InMemDicomObject {
    items
        .iter()
        .find(|item| {
            sequence(item, tags::CONCEPT_NAME_CODE_SEQUENCE)
                .first()
                .is_some_and(|concept| text(concept, tags::CODE_VALUE) == code)
        })
        .unwrap_or_else(|| panic!("missing content item with concept {code}"))
}

fn sequence(object: &InMemDicomObject, tag: Tag) -> &[InMemDicomObject] {
    object
        .element(tag)
        .expect("required sequence")
        .items()
        .expect("sequence value")
}

fn text(object: &InMemDicomObject, tag: Tag) -> String {
    object
        .element(tag)
        .expect("required text")
        .to_str()
        .expect("text value")
        .trim_end_matches(['\0', ' '])
        .to_string()
}

fn assert_uid(object: &Value, pointer: &str) {
    let uid = object
        .pointer(pointer)
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("{pointer} must be a UID string"));
    assert!(uid.len() <= 64);
    assert!(
        uid.split('.')
            .all(|component| !component.is_empty() && component.bytes().all(|b| b.is_ascii_digit()))
    );
}

fn assert_sha256(value: &Value, label: &str) {
    let value = value
        .as_str()
        .unwrap_or_else(|| panic!("{label} must be a string"));
    assert_eq!(value.len(), 64);
    assert!(value.bytes().all(|byte| byte.is_ascii_hexdigit()));
}

fn generate_extended(root: &Path) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args([
            "generate",
            "--profile",
            "extended",
            "--out",
            root.to_str().expect("UTF-8 path"),
            "--seed",
            "7",
        ])
        .output()
        .expect("extended generation should run");
    assert!(
        output.status.success(),
        "extended generation failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&fs::read(root.join("manifest.json")).expect("manifest should read"))
        .expect("manifest should parse")
}

fn file_for_case(manifest: &Value) -> Option<&Value> {
    manifest["files"]
        .as_array()?
        .iter()
        .find(|file| file["case_id"].as_str() == Some(CASE_ID))
}

fn unique_temp_dir(label: &str) -> PathBuf {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    std::env::temp_dir().join(format!("dts-{label}-{}-{nonce}", std::process::id()))
}
