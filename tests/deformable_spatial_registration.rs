use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use dicom_core::VR;
use dicom_dictionary_std::{tags, uids};
use dicom_object::{InMemDicomObject, open_file};
use serde_json::{Value, json};

const CASE_ID: &str = "derived/registration/deformable_ct_pair";
const RELATIVE_PATH: &str = "derived/registration/deformable_ct_pair/instance.dcm";
const TARGET_PATH: &str = "enhanced/ct/multiframe_shared_perframe_explicit_le/instance.dcm";
const SOURCE_PATH: &str = "classic/ct/mono2_i16_rescale_12bit_explicit_le/instance.dcm";
const SOP_CLASS_UID: &str = "1.2.840.10008.5.1.4.1.1.66.3";
const PAYLOAD_SHA256: &str = "d0673d2da1b415db6465047e607b7f16f1a886dfae4ede91764c71bf7df72f47";
const IDENTITY: [f64; 16] = [
    1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
];
const VECTORS: [f32; 12] = [
    -0.625, -0.625, -2.5, -0.75, -0.625, -2.5, -0.625, -0.75, -2.5, -0.75, -0.75, -2.5,
];

#[test]
fn deformable_registration_vertical_slice_is_byte_deterministic_and_closed() {
    let first_root = unique_temp_dir("deformable-registration-first");
    let second_root = unique_temp_dir("deformable-registration-second");
    let first_manifest = generate_extended(&first_root);
    let second_manifest = generate_extended(&second_root);
    let first = case_file(&first_manifest);
    let second = case_file(&second_manifest);

    let first_bytes = fs::read(first_root.join(RELATIVE_PATH)).expect("first REG instance");
    let second_bytes = fs::read(second_root.join(RELATIVE_PATH)).expect("second REG instance");
    assert_eq!(first, second, "seed-7 manifests must match");
    assert_eq!(first_bytes, second_bytes, "seed-7 REG bytes must match");
    assert_eq!(first["sha256"], dicom_test_suite::sha256_hex(&first_bytes));
    assert_eq!(first["determinism"], "byte_stable");
    assert!(
        jsonschema::validator_for(&read_json("schemas/manifest.schema.json"))
            .expect("manifest schema")
            .is_valid(&first_manifest)
    );

    assert_manifest_contract(&first_root, &first_manifest, first);
    assert_dicom_contract(&first_root);
    for root in [&first_root, &second_root] {
        let validation = dicom_test_suite::validate_generated_root(root)
            .expect("generated extended root should validate");
        assert!(validation.failures.is_empty(), "{:?}", validation.failures);
    }
    assert_manifest_source_mutation_is_rejected(&first_root, &first_manifest);

    fs::remove_dir_all(first_root).expect("remove first root");
    fs::remove_dir_all(second_root).expect("remove second root");
}

fn assert_manifest_contract(root: &Path, manifest: &Value, file: &Value) {
    assert_eq!(file["path"], RELATIVE_PATH);
    assert_eq!(
        file.pointer("/recipe/recipe_id"),
        Some(&json!("derived_registration_deformable_ct_pair"))
    );
    assert_eq!(
        file.pointer("/dicom/sop_class_uid"),
        Some(&json!(SOP_CLASS_UID))
    );
    assert_eq!(
        file.pointer("/dicom/transfer_syntax_uid"),
        Some(&json!(uids::EXPLICIT_VR_LITTLE_ENDIAN))
    );
    assert!(file["image"].is_null() && file["pixel_data"].is_null());

    let references = file["references"].as_array().expect("ordinary references");
    assert_eq!(references.len(), 2);
    assert_eq!(references[0]["relationship"], "registered_target");
    assert_eq!(references[0]["source_path"], TARGET_PATH);
    assert_eq!(references[1]["relationship"], "deformation_source");
    assert_eq!(references[1]["source_path"], SOURCE_PATH);

    let expected = &file["expected_deformable_spatial_registration"];
    assert_eq!(expected["sampling_direction"], "registered_to_source");
    assert_eq!(expected["deformable_registration_items"], 1);
    assert_eq!(expected["registration_type_code_items"], 0);
    assert_eq!(
        expected["pre_deformation_matrix"],
        json!({"items": 1, "type": "RIGID", "values": IDENTITY})
    );
    assert_eq!(
        expected["post_deformation_matrix"],
        json!({"items": 1, "type": "RIGID", "values": IDENTITY})
    );
    assert_eq!(
        expected.pointer("/grid/dimensions"),
        Some(&json!([2, 2, 1]))
    );
    assert_eq!(
        expected.pointer("/grid/resolution_mm"),
        Some(&json!([0.75, 0.75, 2.5]))
    );
    assert_eq!(
        expected.pointer("/grid/payload_sha256"),
        Some(&json!(PAYLOAD_SHA256))
    );
    assert_eq!(
        expected.pointer("/grid/vectors_mm"),
        Some(&json!([
            [-0.625, -0.625, -2.5],
            [-0.75, -0.625, -2.5],
            [-0.625, -0.75, -2.5],
            [-0.75, -0.75, -2.5]
        ]))
    );
    assert_eq!(expected["point_mappings"].as_array().map(Vec::len), Some(4));
    assert_eq!(
        expected.pointer("/common_instance_reference/same_study/source_path"),
        Some(&json!(TARGET_PATH))
    );
    assert_eq!(
        expected.pointer("/common_instance_reference/other_studies/0/source_path"),
        Some(&json!(SOURCE_PATH))
    );
    assert_eq!(expected["pixel_data_absent"], true);
    assert!(
        file.pointer("/validation/internal")
            .and_then(Value::as_array)
            .is_some_and(|rows| rows.iter().all(|row| row["status"] == "passed"))
    );
    assert!(root.join(TARGET_PATH).exists());
    assert!(root.join(SOURCE_PATH).exists());
    assert!(
        manifest["files"]
            .as_array()
            .is_some_and(|files| files.len() == 113)
    );
}

fn assert_dicom_contract(root: &Path) {
    let object = open_file(root.join(RELATIVE_PATH)).expect("open Deformable REG");
    assert_eq!(text(&object, tags::SOP_CLASS_UID), SOP_CLASS_UID);
    assert_eq!(text(&object, tags::MODALITY), "REG");
    assert!(object.element(tags::PIXEL_DATA).is_err());
    let registrations = sequence(&object, tags::DEFORMABLE_REGISTRATION_SEQUENCE);
    assert_eq!(registrations.len(), 1);
    let registration = &registrations[0];
    assert_eq!(
        sequence(registration, tags::REFERENCED_IMAGE_SEQUENCE).len(),
        1
    );
    assert_eq!(
        sequence(registration, tags::REGISTRATION_TYPE_CODE_SEQUENCE).len(),
        0
    );
    assert_identity_matrix(
        registration,
        tags::PRE_DEFORMATION_MATRIX_REGISTRATION_SEQUENCE,
    );
    assert_identity_matrix(
        registration,
        tags::POST_DEFORMATION_MATRIX_REGISTRATION_SEQUENCE,
    );
    let grids = sequence(registration, tags::DEFORMABLE_REGISTRATION_GRID_SEQUENCE);
    assert_eq!(grids.len(), 1);
    let grid = &grids[0];
    let dimensions = grid.element(tags::GRID_DIMENSIONS).expect("dimensions");
    assert_eq!(dimensions.vr(), VR::UL);
    assert_eq!(dimensions.to_multi_int::<u32>().unwrap(), [2, 2, 1]);
    let resolution = grid.element(tags::GRID_RESOLUTION).expect("resolution");
    assert_eq!(resolution.vr(), VR::FD);
    assert_eq!(resolution.to_multi_float64().unwrap(), [0.75, 0.75, 2.5]);
    let vectors = grid.element(tags::VECTOR_GRID_DATA).expect("vectors");
    assert_eq!(vectors.vr(), VR::OF);
    assert_eq!(
        vectors.value().to_multi_float32().unwrap().as_slice(),
        VECTORS
    );
    assert_eq!(
        dicom_test_suite::sha256_hex(vectors.value().to_bytes().unwrap().as_ref()),
        PAYLOAD_SHA256
    );
}

fn assert_identity_matrix(registration: &InMemDicomObject, tag: dicom_core::Tag) {
    let items = sequence(registration, tag);
    assert_eq!(items.len(), 1);
    assert_eq!(
        text(
            &items[0],
            tags::FRAME_OF_REFERENCE_TRANSFORMATION_MATRIX_TYPE
        ),
        "RIGID"
    );
    assert_eq!(
        items[0]
            .element(tags::FRAME_OF_REFERENCE_TRANSFORMATION_MATRIX)
            .unwrap()
            .to_multi_float64()
            .unwrap(),
        IDENTITY
    );
}

fn assert_manifest_source_mutation_is_rejected(root: &Path, pristine: &Value) {
    let mut manifest = pristine.clone();
    let file = manifest["files"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|file| file["case_id"] == CASE_ID)
        .unwrap();
    file["expected_deformable_spatial_registration"]["source"]["source_sha256"] =
        json!("0".repeat(64));
    fs::write(
        root.join("manifest.json"),
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();
    let validation = dicom_test_suite::validate_generated_root(root).expect("validate mutation");
    assert!(
        validation
            .failures
            .iter()
            .any(|failure| failure.contains("deformable_registration_source_source_sha256"))
    );
    fs::write(
        root.join("manifest.json"),
        serde_json::to_vec_pretty(pristine).unwrap(),
    )
    .unwrap();
}

fn case_file(manifest: &Value) -> &Value {
    manifest["files"]
        .as_array()
        .unwrap()
        .iter()
        .find(|file| file["case_id"] == CASE_ID)
        .expect("case manifest entry")
}

fn generate_extended(root: &Path) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args([
            "generate",
            "--profile",
            "extended",
            "--out",
            root.to_str().unwrap(),
            "--seed",
            "7",
        ])
        .output()
        .expect("extended generation");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&fs::read(root.join("manifest.json")).unwrap()).unwrap()
}

fn sequence(object: &InMemDicomObject, tag: dicom_core::Tag) -> &[InMemDicomObject] {
    object
        .element(tag)
        .expect("sequence")
        .items()
        .expect("sequence items")
}

fn text(object: &InMemDicomObject, tag: dicom_core::Tag) -> String {
    object
        .element(tag)
        .expect("text element")
        .to_str()
        .expect("text")
        .trim_end_matches(['\0', ' '])
        .to_string()
}

fn read_json(path: &str) -> Value {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

fn unique_temp_dir(label: &str) -> PathBuf {
    let path =
        std::env::temp_dir().join(format!("dicom-test-suite-{label}-{}", std::process::id()));
    if path.exists() {
        fs::remove_dir_all(&path).unwrap();
    }
    path
}
