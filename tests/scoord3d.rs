use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use dicom_core::Tag;
use dicom_core::ops::{
    ApplyOp, AttributeAction, AttributeOp, AttributeSelector, AttributeSelectorStep,
};
use dicom_core::value::PrimitiveValue;
use dicom_dictionary_std::{tags, uids};
use dicom_object::{InMemDicomObject, open_file};
use serde_json::Value;

const CASE_ID: &str = "derived/sr/comprehensive3d_scoord3d";
const RELATIVE_PATH: &str = "derived/sr/comprehensive3d_scoord3d/scoord3d-report.dcm";
const CT_CASE_ID: &str = "enhanced/ct/multiframe_shared_perframe_explicit_le";
const SOP_CLASS_UID: &str = "1.2.840.10008.5.1.4.1.1.88.34";
const CT_SOP_CLASS_UID: &str = "1.2.840.10008.5.1.4.1.1.2.1";
const TAG_REFERENCED_FRAME_NUMBER: Tag = Tag(0x0008, 0x1160);

#[test]
fn scoord3d_vertical_slice_is_byte_deterministic_and_strictly_validated() {
    let first_root = unique_temp_dir("scoord3d-first");
    let second_root = unique_temp_dir("scoord3d-second");
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

    match (file_for_case(&first_manifest), file_for_case(&second_manifest)) {
        (Some(first), Some(second)) => {
            assert_eq!(first, second, "seed-7 SCOORD3D manifests must match");
            let first_bytes = fs::read(first_root.join(RELATIVE_PATH)).expect("first report");
            let second_bytes = fs::read(second_root.join(RELATIVE_PATH)).expect("second report");
            assert_eq!(first_bytes, second_bytes, "SCOORD3D bytes must match");
            assert_manifest_contract(&first_root, first);
            assert_dicom_contract(&first_root, first);
            assert_coordinate_mutation_is_rejected(&first_root, &first_manifest);
        }
        (None, None) => {
            assert_explicitly_unavailable(&first_manifest, &first_root);
            assert_explicitly_unavailable(&second_manifest, &second_root);
        }
        _ => panic!("locked backend availability changed between runs"),
    }

    fs::remove_dir_all(first_root).expect("remove first root");
    fs::remove_dir_all(second_root).expect("remove second root");
}

fn assert_manifest_contract(root: &Path, file: &Value) {
    assert_eq!(file["path"], RELATIVE_PATH);
    assert_eq!(file["determinism"], "semantic_stable");
    assert_eq!(file.pointer("/recipe/recipe_id").and_then(Value::as_str),
        Some("derived_sr_comprehensive3d_scoord3d"));
    assert_eq!(file.pointer("/recipe/recipe_parameters/graphic_type"), Some(&Value::from("POLYLINE")));
    assert_eq!(file.pointer("/recipe/recipe_parameters/graphic_data_patient_mm"),
        Some(&serde_json::json!([[0.0, 0.0, 0.0], [0.0, 0.0, 2.5]])));
    assert_eq!(file.pointer("/dicom/sop_class_uid").and_then(Value::as_str), Some(SOP_CLASS_UID));
    assert_eq!(file.pointer("/dicom/transfer_syntax_uid").and_then(Value::as_str),
        Some(uids::EXPLICIT_VR_LITTLE_ENDIAN));
    assert!(file["image"].is_null() && file["pixel_data"].is_null());
    assert_eq!(file.pointer("/generation_backend/backend_id").and_then(Value::as_str),
        Some("highdicom_pydicom"));
    assert!(file.pointer("/generation_backend/version").and_then(Value::as_str)
        .is_some_and(|version| version.starts_with("0.4.0+highdicom.0.28.1")));
    assert_eq!(file["references"].as_array().map(Vec::len), Some(1));
    let reference = &file["references"][0];
    assert_eq!(reference["relationship"], "source_of_measurement");
    assert_eq!(reference["source_case_id"], CT_CASE_ID);
    assert_eq!(reference["frame_numbers"], serde_json::json!([1, 2]));
    assert!(root.join(reference["source_path"].as_str().expect("source path")).exists());

    let expected = &file["expected_scoord3d"];
    assert_eq!(expected["root_template"], serde_json::json!({
        "mapping_resource": "DCMR", "template_identifier": "1500"
    }));
    assert_eq!(expected.pointer("/measurement_group/template"), Some(&serde_json::json!({
        "mapping_resource": "DCMR", "template_identifier": "1501"
    })));
    assert_eq!(expected.pointer("/measurement_group/tracking_identifier"),
        Some(&Value::from("DTS-SCOORD3D-ROI-1")));
    assert_eq!(expected.pointer("/measurement_group/measurement/numeric_value"),
        Some(&Value::from("2.5")));
    let coordinates = expected.pointer("/measurement_group/measurement/spatial_coordinates")
        .expect("spatial coordinates");
    assert_eq!(coordinates["relationship"], "INFERRED FROM");
    assert_eq!(coordinates["value_type"], "SCOORD3D");
    assert_eq!(coordinates["graphic_type"], "POLYLINE");
    assert_eq!(coordinates["graphic_data_mm"], serde_json::json!([0.0, 0.0, 0.0, 0.0, 0.0, 2.5]));
    assert_eq!(expected.pointer("/measurement_group/source_image/referenced_frame_numbers"),
        Some(&serde_json::json!([1, 2])));
    assert_eq!(expected["evidence"].as_array().map(Vec::len), Some(1));
    assert_eq!(file["expected_capabilities"], serde_json::json!([
        "parse_structured_report", "parse_scoord3d", "resolve_references",
        "render_spatial_annotation"
    ]));
    assert!(file.pointer("/validation/internal").and_then(Value::as_array)
        .is_some_and(|results| results.iter().all(|result| result["status"] == "passed")));
}

fn assert_dicom_contract(root: &Path, file: &Value) {
    let object = open_file(root.join(RELATIVE_PATH)).expect("SCOORD3D report should reopen");
    assert_eq!(object.meta().media_storage_sop_class_uid(), SOP_CLASS_UID);
    assert_eq!(text(&object, tags::VALUE_TYPE), "CONTAINER");
    assert_eq!(text(&sequence(&object, tags::CONTENT_TEMPLATE_SEQUENCE)[0], tags::TEMPLATE_IDENTIFIER), "1500");
    let root_content = sequence(&object, tags::CONTENT_SEQUENCE);
    assert_eq!(root_content.len(), 8);
    let imaging = find_named_item(root_content, "126010");
    let group = &sequence(imaging, tags::CONTENT_SEQUENCE)[0];
    assert_eq!(text(&sequence(group, tags::CONTENT_TEMPLATE_SEQUENCE)[0], tags::TEMPLATE_IDENTIFIER), "1501");
    let group_content = sequence(group, tags::CONTENT_SEQUENCE);
    assert_eq!(group_content.len(), 5);
    let distance = find_named_item(group_content, "121206");
    assert_eq!(text(&sequence(distance, tags::MEASURED_VALUE_SEQUENCE)[0], tags::NUMERIC_VALUE), "2.5");
    let coordinates = &sequence(distance, tags::CONTENT_SEQUENCE)[0];
    assert_eq!(text(coordinates, tags::RELATIONSHIP_TYPE), "INFERRED FROM");
    assert_eq!(text(coordinates, tags::VALUE_TYPE), "SCOORD3D");
    assert_eq!(text(coordinates, tags::GRAPHIC_TYPE), "POLYLINE");
    assert_eq!(coordinates.element(tags::GRAPHIC_DATA).expect("Graphic Data")
        .to_multi_float32().expect("FL coordinates"), vec![0.0, 0.0, 0.0, 0.0, 0.0, 2.5]);
    assert_eq!(text(coordinates, tags::REFERENCED_FRAME_OF_REFERENCE_UID),
        file.pointer("/uids/frame_of_reference_uid").and_then(Value::as_str).unwrap());
    let source = find_named_item(group_content, "121112");
    let source_sop = &sequence(source, tags::REFERENCED_SOP_SEQUENCE)[0];
    assert_eq!(text(source_sop, tags::REFERENCED_SOP_CLASS_UID), CT_SOP_CLASS_UID);
    assert_eq!(source_sop.element(TAG_REFERENCED_FRAME_NUMBER).expect("frames")
        .to_multi_int::<u16>().expect("IS frames"), vec![1, 2]);
    assert_eq!(sequence(&object, tags::CURRENT_REQUESTED_PROCEDURE_EVIDENCE_SEQUENCE).len(), 1);
    for forbidden in [tags::PIXEL_DATA, tags::FLOAT_PIXEL_DATA, tags::DOUBLE_FLOAT_PIXEL_DATA] {
        assert!(object.element_opt(forbidden).expect("pixel lookup").is_none());
    }
}

fn assert_coordinate_mutation_is_rejected(root: &Path, pristine_manifest: &Value) {
    let path = root.join(RELATIVE_PATH);
    let mut object = open_file(&path).expect("report for mutation");
    let selector = AttributeSelector::new([
        AttributeSelectorStep::Nested { tag: tags::CONTENT_SEQUENCE, item: 7 },
        AttributeSelectorStep::Nested { tag: tags::CONTENT_SEQUENCE, item: 0 },
        AttributeSelectorStep::Nested { tag: tags::CONTENT_SEQUENCE, item: 3 },
        AttributeSelectorStep::Nested { tag: tags::CONTENT_SEQUENCE, item: 0 },
        AttributeSelectorStep::Tag(tags::GRAPHIC_DATA),
    ]).expect("valid nested Graphic Data selector");
    object.apply(AttributeOp::new(
        selector,
        AttributeAction::Set(PrimitiveValue::F32(
            vec![0.0, 0.0, 0.0, 0.0, 0.0, 3.0].into(),
        )),
    )).expect("Graphic Data mutation");
    object.write_to_file(&path).expect("write mutated report");
    let bytes = fs::read(&path).expect("mutated bytes");
    let mut manifest = pristine_manifest.clone();
    let file = manifest["files"].as_array_mut().unwrap().iter_mut()
        .find(|file| file["case_id"] == CASE_ID).expect("manifest entry");
    file["sha256"] = Value::String(dicom_test_suite::sha256_hex(&bytes));
    file["size_bytes"] = Value::from(bytes.len() as u64);
    fs::write(root.join("manifest.json"), serde_json::to_vec_pretty(&manifest).unwrap()).unwrap();
    let validation = dicom_test_suite::validate_generated_root(root).expect("mutated validation");
    assert!(validation.failures.iter().any(|failure| failure.contains("scoord3d_graphic_data")),
        "coordinate drift must fail independently of checksum: {:?}", validation.failures);
}

fn assert_explicitly_unavailable(manifest: &Value, root: &Path) {
    let row = manifest["skipped_cases"].as_array().unwrap().iter()
        .find(|row| row["case_id"] == CASE_ID).expect("explicit unavailable row");
    assert_eq!(row["status"], "unavailable");
    assert_eq!(row["reason_code"], "external_backend_unavailable");
    assert!(!root.join(RELATIVE_PATH).exists());
}

fn sequence(object: &InMemDicomObject, tag: Tag) -> &[InMemDicomObject] {
    object.element(tag).expect("required sequence").items().expect("sequence value")
}

fn find_named_item<'a>(items: &'a [InMemDicomObject], code: &str) -> &'a InMemDicomObject {
    items.iter().find(|item| sequence(item, tags::CONCEPT_NAME_CODE_SEQUENCE)
        .first().is_some_and(|concept| text(concept, tags::CODE_VALUE) == code))
        .unwrap_or_else(|| panic!("missing content item {code}"))
}

fn text(object: &InMemDicomObject, tag: Tag) -> String {
    object.element(tag).expect("required text").to_str().expect("text value")
        .trim_end_matches(['\0', ' ']).to_string()
}

fn generate_extended(root: &Path) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args(["generate", "--profile", "extended", "--out", root.to_str().unwrap(), "--seed", "7"])
        .output().expect("extended generation");
    assert!(output.status.success(), "generation failed: {}", String::from_utf8_lossy(&output.stderr));
    serde_json::from_slice(&fs::read(root.join("manifest.json")).unwrap()).unwrap()
}

fn file_for_case(manifest: &Value) -> Option<&Value> {
    manifest["files"].as_array()?.iter().find(|file| file["case_id"] == CASE_ID)
}

fn unique_temp_dir(label: &str) -> PathBuf {
    let nonce = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    std::env::temp_dir().join(format!("dts-{label}-{}-{nonce}", std::process::id()))
}
