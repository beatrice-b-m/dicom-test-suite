use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use dicom_core::ops::{
    ApplyOp, AttributeAction, AttributeOp, AttributeSelector, AttributeSelectorStep,
};
use dicom_core::{PrimitiveValue, Tag};
use dicom_dictionary_std::{tags, uids};
use dicom_object::{InMemDicomObject, open_file};
use serde_json::{Value, json};

const CASE_ID: &str = "derived/registration/spatial_ct_pair";
const RELATIVE_PATH: &str = "derived/registration/spatial_ct_pair/instance.dcm";
const TARGET_CASE_ID: &str = "enhanced/ct/multiframe_shared_perframe_explicit_le";
const TARGET_PATH: &str = "enhanced/ct/multiframe_shared_perframe_explicit_le/instance.dcm";
const SOURCE_CASE_ID: &str = "classic/ct/mono2_i16_rescale_12bit_explicit_le";
const SOURCE_PATH: &str = "classic/ct/mono2_i16_rescale_12bit_explicit_le/instance.dcm";
const SPATIAL_REGISTRATION_STORAGE_UID: &str = "1.2.840.10008.5.1.4.1.1.66.1";
const TARGET_MATRIX: [f64; 16] = [
    1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
];
const SOURCE_MATRIX: [f64; 16] = [
    1.0, 0.0, 0.0, 0.625, 0.0, 1.0, 0.0, 0.625, 0.0, 0.0, 1.0, 2.5, 0.0, 0.0, 0.0, 1.0,
];

#[test]
fn spatial_registration_vertical_slice_is_byte_deterministic_and_strictly_validated() {
    let first_root = unique_temp_dir("spatial-registration-first");
    let second_root = unique_temp_dir("spatial-registration-second");
    let first_manifest = generate_extended(&first_root);
    let second_manifest = generate_extended(&second_root);
    let first = case_file(&first_manifest);
    let second = case_file(&second_manifest);

    let first_bytes = fs::read(first_root.join(RELATIVE_PATH)).expect("first REG instance");
    let second_bytes = fs::read(second_root.join(RELATIVE_PATH)).expect("second REG instance");
    assert_eq!(
        first, second,
        "seed-7 Spatial Registration manifests must match"
    );
    assert_eq!(first_bytes, second_bytes, "seed-7 REG bytes must match");
    assert_eq!(first["sha256"], synth_dicom_gen::sha256_hex(&first_bytes));
    assert_eq!(first["determinism"], "byte_stable");
    crate::curated_manifest_contract_support::assert_curated_manifest_schema_valid(&first_manifest);

    assert_manifest_contract(&first_root, &first_manifest, first);
    assert_dicom_contract(&first_root, first);
    for root in [&first_root, &second_root] {
        let validation = synth_dicom_gen::validate_generated_root(root)
            .expect("generated extended root should validate");
        assert!(
            validation.failures.is_empty(),
            "generated root validation failed: {:?}",
            validation.failures
        );
    }
    assert_manifest_closure_mutation_is_rejected(&first_root, &first_manifest);
    assert_matrix_mutation_is_rejected(&first_root, &first_manifest);

    fs::remove_dir_all(first_root).expect("remove first root");
    fs::remove_dir_all(second_root).expect("remove second root");
}

fn assert_manifest_closure_mutation_is_rejected(root: &Path, pristine_manifest: &Value) {
    let mut manifest = pristine_manifest.clone();
    let file = manifest["files"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|file| file["case_id"] == CASE_ID)
        .expect("manifest entry");
    file["expected_spatial_registration"]["registration_items"][1]["source"]["source_sha256"] =
        Value::String("0".repeat(64));
    fs::write(
        root.join("manifest.json"),
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();

    let validation = synth_dicom_gen::validate_generated_root(root)
        .expect("mutated manifest should remain structurally readable");
    assert!(
        validation
            .failures
            .iter()
            .any(|failure| { failure.contains("spatial_registration_1_source_sha256") })
    );

    fs::write(
        root.join("manifest.json"),
        serde_json::to_vec_pretty(pristine_manifest).unwrap(),
    )
    .unwrap();
}

fn assert_manifest_contract(root: &Path, manifest: &Value, file: &Value) {
    assert_eq!(file["path"], RELATIVE_PATH);
    assert_eq!(
        file.pointer("/recipe/recipe_id"),
        Some(&json!("derived_registration_spatial_ct_pair"))
    );
    assert_eq!(
        file.pointer("/recipe/recipe_parameters/matrix_direction"),
        Some(&json!("source_to_registered"))
    );
    assert_eq!(
        file.pointer("/recipe/recipe_parameters/target_identity_matrix"),
        Some(&json!(TARGET_MATRIX))
    );
    assert_eq!(
        file.pointer("/recipe/recipe_parameters/source_to_registered_matrix"),
        Some(&json!(SOURCE_MATRIX))
    );
    assert_eq!(
        file.pointer("/dicom/sop_class_uid"),
        Some(&json!(SPATIAL_REGISTRATION_STORAGE_UID))
    );
    assert_eq!(file.pointer("/dicom/modality"), Some(&json!("REG")));
    assert_eq!(
        file.pointer("/dicom/transfer_syntax_uid"),
        Some(&json!(uids::EXPLICIT_VR_LITTLE_ENDIAN))
    );
    assert!(file["image"].is_null() && file["pixel_data"].is_null());

    let references = file["references"].as_array().expect("ordered references");
    assert_eq!(references.len(), 2);
    assert_reference(
        manifest,
        &references[0],
        "registered_target",
        TARGET_CASE_ID,
        TARGET_PATH,
    );
    assert_reference(
        manifest,
        &references[1],
        "moving_source",
        SOURCE_CASE_ID,
        SOURCE_PATH,
    );

    let expected = &file["expected_spatial_registration"];
    assert_eq!(expected["matrix_direction"], "source_to_registered");
    assert_eq!(
        expected["registered_frame_of_reference_uid"],
        file["uids"]["frame_of_reference_uid"]
    );
    let items = expected["registration_items"]
        .as_array()
        .expect("registration items");
    assert_eq!(items.len(), 2);
    assert_registration_item(
        manifest,
        &items[0],
        "registered_target",
        TARGET_CASE_ID,
        TARGET_PATH,
        &TARGET_MATRIX,
    );
    assert_registration_item(
        manifest,
        &items[1],
        "moving_source",
        SOURCE_CASE_ID,
        SOURCE_PATH,
        &SOURCE_MATRIX,
    );
    assert_eq!(
        expected["rigid_tolerances"],
        json!({
            "orthonormal_abs": 0.000001,
            "determinant_abs": 0.000001,
            "homogeneous_abs": 0.000001
        })
    );
    assert_eq!(
        expected["landmark"],
        json!({
            "source_point_mm": [-0.625, -0.625, 0.0],
            "registered_point_mm": [0.0, 0.0, 2.5],
            "tolerance_mm": 0.000001
        })
    );
    assert_eq!(expected["pixel_data_absent"], true);
    assert_eq!(
        expected.pointer("/common_instance_reference/same_study"),
        Some(&items[0]["source"])
    );
    assert_eq!(
        expected.pointer("/common_instance_reference/other_studies/0"),
        Some(&items[1]["source"])
    );
    assert_eq!(
        file["expected_capabilities"],
        json!([
            "open_file",
            "read_metadata",
            "resolve_references",
            "read_spatial_registration",
            "apply_rigid_transform",
            "fuse_registered_images"
        ])
    );
    assert!(
        file.pointer("/validation/internal")
            .and_then(Value::as_array)
            .is_some_and(|results| results.iter().all(|result| result["status"] == "passed"))
    );
    for path in [TARGET_PATH, SOURCE_PATH] {
        assert!(
            root.join(path).exists(),
            "referenced source {path} must exist"
        );
    }
}

fn assert_reference(
    manifest: &Value,
    reference: &Value,
    relationship: &str,
    source_case_id: &str,
    source_path: &str,
) {
    assert_eq!(reference["relationship"], relationship);
    assert_eq!(reference["source_case_id"], source_case_id);
    assert_eq!(reference["source_path"], source_path);
    assert!(reference.get("frame_numbers").is_none());
    let source = file_for_path(manifest, source_path);
    assert_eq!(reference["sop_class_uid"], source["dicom"]["sop_class_uid"]);
    assert_eq!(
        reference["sop_instance_uid"],
        source["uids"]["sop_instance_uid"]
    );
    assert_eq!(
        reference["series_instance_uid"],
        source["uids"]["series_instance_uid"]
    );
}

fn assert_registration_item(
    manifest: &Value,
    item: &Value,
    role: &str,
    source_case_id: &str,
    source_path: &str,
    matrix: &[f64; 16],
) {
    assert_eq!(item["role"], role);
    assert_eq!(item["complete_instance"], true);
    assert_eq!(item["matrix_registration_items"], 1);
    assert_eq!(item["registration_type_code_items"], 0);
    assert_eq!(item["matrix_items"], 1);
    assert_eq!(item.pointer("/matrix/type"), Some(&json!("RIGID")));
    assert_eq!(item.pointer("/matrix/values"), Some(&json!(matrix)));

    let source = file_for_path(manifest, source_path);
    let identity = &item["source"];
    assert_eq!(identity["source_case_id"], source_case_id);
    assert_eq!(identity["source_path"], source_path);
    assert_eq!(identity["source_sha256"], source["sha256"]);
    assert_eq!(
        identity["study_instance_uid"],
        source["uids"]["study_instance_uid"]
    );
    assert_eq!(
        identity["series_instance_uid"],
        source["uids"]["series_instance_uid"]
    );
    assert_eq!(identity["sop_class_uid"], source["dicom"]["sop_class_uid"]);
    assert_eq!(
        identity["sop_instance_uid"],
        source["uids"]["sop_instance_uid"]
    );
    assert_eq!(
        identity["frame_of_reference_uid"],
        source["uids"]["frame_of_reference_uid"]
    );
}

fn assert_dicom_contract(root: &Path, file: &Value) {
    let object = open_file(root.join(RELATIVE_PATH)).expect("Spatial Registration should reopen");
    assert_eq!(
        object.meta().media_storage_sop_class_uid(),
        SPATIAL_REGISTRATION_STORAGE_UID
    );
    assert_eq!(text(&object, tags::MODALITY), "REG");
    assert_eq!(
        text(&object, tags::FRAME_OF_REFERENCE_UID),
        file["uids"]["frame_of_reference_uid"].as_str().unwrap()
    );
    let registrations = sequence(&object, tags::REGISTRATION_SEQUENCE);
    assert_eq!(registrations.len(), 2);
    assert_dicom_registration_item(&registrations[0], &TARGET_MATRIX);
    assert_dicom_registration_item(&registrations[1], &SOURCE_MATRIX);
    assert_eq!(sequence(&object, tags::REFERENCED_SERIES_SEQUENCE).len(), 1);
    assert_eq!(
        sequence(
            &object,
            tags::STUDIES_CONTAINING_OTHER_REFERENCED_INSTANCES_SEQUENCE
        )
        .len(),
        1
    );
    for forbidden in [
        tags::PIXEL_DATA,
        tags::FLOAT_PIXEL_DATA,
        tags::DOUBLE_FLOAT_PIXEL_DATA,
    ] {
        assert!(
            object
                .element_opt(forbidden)
                .expect("pixel lookup")
                .is_none()
        );
    }
}

fn assert_dicom_registration_item(item: &InMemDicomObject, expected_matrix: &[f64; 16]) {
    let images = item_sequence(item, tags::REFERENCED_IMAGE_SEQUENCE);
    assert_eq!(images.len(), 1);
    assert!(
        images[0]
            .element_opt(Tag(0x0008, 0x1160))
            .expect("Referenced Frame Number lookup")
            .is_none()
    );
    let registrations = item_sequence(item, tags::MATRIX_REGISTRATION_SEQUENCE);
    assert_eq!(registrations.len(), 1);
    assert_eq!(
        item_sequence(&registrations[0], tags::REGISTRATION_TYPE_CODE_SEQUENCE).len(),
        0
    );
    let matrices = item_sequence(&registrations[0], tags::MATRIX_SEQUENCE);
    assert_eq!(matrices.len(), 1);
    assert_eq!(
        item_text(
            &matrices[0],
            tags::FRAME_OF_REFERENCE_TRANSFORMATION_MATRIX_TYPE
        ),
        "RIGID"
    );
    assert_eq!(
        matrices[0]
            .element(tags::FRAME_OF_REFERENCE_TRANSFORMATION_MATRIX)
            .expect("matrix")
            .to_multi_float64()
            .expect("DS matrix"),
        expected_matrix
    );
}

fn assert_matrix_mutation_is_rejected(root: &Path, pristine_manifest: &Value) {
    let path = root.join(RELATIVE_PATH);
    let mut object = open_file(&path).expect("REG for mutation");
    let selector = AttributeSelector::new([
        AttributeSelectorStep::Nested {
            tag: tags::REGISTRATION_SEQUENCE,
            item: 1,
        },
        AttributeSelectorStep::Nested {
            tag: tags::MATRIX_REGISTRATION_SEQUENCE,
            item: 0,
        },
        AttributeSelectorStep::Nested {
            tag: tags::MATRIX_SEQUENCE,
            item: 0,
        },
        AttributeSelectorStep::Tag(tags::FRAME_OF_REFERENCE_TRANSFORMATION_MATRIX),
    ])
    .expect("valid nested matrix selector");
    object
        .apply(AttributeOp::new(
            selector,
            AttributeAction::Set(PrimitiveValue::Strs(
                [
                    "2", "0", "0", "0.625", "0", "1", "0", "0.625", "0", "0", "1", "2.5", "0", "0",
                    "0", "1",
                ]
                .into_iter()
                .map(str::to_string)
                .collect::<Vec<_>>()
                .into(),
            )),
        ))
        .expect("matrix mutation");
    object.write_to_file(&path).expect("write mutated REG");

    let bytes = fs::read(&path).expect("mutated bytes");
    let mut manifest = pristine_manifest.clone();
    let file = manifest["files"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|file| file["case_id"] == CASE_ID)
        .expect("manifest entry");
    file["sha256"] = Value::String(synth_dicom_gen::sha256_hex(&bytes));
    file["size_bytes"] = Value::from(bytes.len() as u64);
    fs::write(
        root.join("manifest.json"),
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();

    let validation = synth_dicom_gen::validate_generated_root(root).expect("mutated validation");
    assert!(
        validation.failures.iter().any(|failure| {
            failure.contains("spatial_registration")
                && (failure.contains("matrix") || failure.contains("content_contract"))
        }),
        "non-rigid matrix drift must fail independently of checksum: {:?}",
        validation.failures
    );
}

fn sequence(object: &InMemDicomObject, tag: Tag) -> &[InMemDicomObject] {
    object
        .element(tag)
        .expect("required sequence")
        .items()
        .expect("sequence value")
}

fn item_sequence(object: &InMemDicomObject, tag: Tag) -> &[InMemDicomObject] {
    object
        .element(tag)
        .expect("required item sequence")
        .items()
        .expect("item sequence value")
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

fn item_text(object: &InMemDicomObject, tag: Tag) -> String {
    text(object, tag)
}

fn generate_extended(root: &Path) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_synth-dicom-gen"))
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
        "generation failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&fs::read(root.join("manifest.json")).unwrap()).unwrap()
}

fn case_file(manifest: &Value) -> &Value {
    manifest["files"]
        .as_array()
        .unwrap()
        .iter()
        .find(|file| file["case_id"] == CASE_ID)
        .expect("Spatial Registration manifest entry")
}

fn file_for_path<'a>(manifest: &'a Value, path: &str) -> &'a Value {
    manifest["files"]
        .as_array()
        .unwrap()
        .iter()
        .find(|file| file["path"] == path)
        .unwrap_or_else(|| panic!("missing source manifest entry {path}"))
}

fn unique_temp_dir(label: &str) -> PathBuf {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "synth-dicom-gen-{label}-{}-{nonce}",
        std::process::id()
    ))
}
