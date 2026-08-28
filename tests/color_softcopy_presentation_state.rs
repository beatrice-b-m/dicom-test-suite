use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use dicom_core::VR;
use dicom_dictionary_std::{tags, uids};
use dicom_object::{InMemDicomObject, open_file};
use serde_json::{Value, json};

const CASE_ID: &str = "derived/presentation-state/color_softcopy";
const RELATIVE_PATH: &str = "derived/presentation-state/color_softcopy/instance.dcm";
const SOURCE_CASE_ID: &str = "classic/sc/rgb_planar0_explicit_le";
const SOURCE_PATH: &str = "classic/sc/rgb_planar0_explicit_le/instance.dcm";
const SOP_CLASS_UID: &str = "1.2.840.10008.5.1.4.1.1.11.2";
const SOURCE_SOP_CLASS_UID: &str = "1.2.840.10008.5.1.4.1.1.7";
const ICC_PROFILE_SHA256: &str = "8e069a3476b71a0e0ae7272d9278ba70540d1c4a0b19af1c7d52e56f49091fef";

#[test]
fn color_softcopy_presentation_state_vertical_slice_is_byte_deterministic_and_closed() {
    let first_workspace = temporary_workspace("color-softcopy-first");
    let second_workspace = temporary_workspace("color-softcopy-second");
    let first_root = first_workspace.join("generated");
    let second_root = second_workspace.join("generated");
    let first_manifest = generate_extended(&first_workspace, &first_root);
    let second_manifest = generate_extended(&second_workspace, &second_root);
    let first = case_file(&first_manifest);
    let second = case_file(&second_manifest);

    let first_bytes = fs::read(first_root.join(RELATIVE_PATH)).expect("first Color PR instance");
    let second_bytes = fs::read(second_root.join(RELATIVE_PATH)).expect("second Color PR instance");
    let first_source = fs::read(first_root.join(SOURCE_PATH)).expect("first RGB source");
    let second_source = fs::read(second_root.join(SOURCE_PATH)).expect("second RGB source");
    assert_eq!(first, second, "seed-7 Color PR manifests must match");
    assert_eq!(
        first_bytes, second_bytes,
        "seed-7 Color PR bytes must match"
    );
    assert_eq!(
        first_source, second_source,
        "seed-7 RGB source bytes must match"
    );
    assert_eq!(first["sha256"], dicom_test_suite::sha256_hex(&first_bytes));
    assert_eq!(first["determinism"], "byte_stable");

    let schema = read_repo_json("schemas/manifest.schema.json");
    let validator = jsonschema::validator_for(&schema).expect("manifest schema");
    let errors = validator
        .iter_errors(&first_manifest)
        .map(|error| error.to_string())
        .collect::<Vec<_>>();
    assert!(errors.is_empty(), "manifest schema failures: {errors:#?}");

    assert_manifest_contract(&first_root, &first_manifest, first);
    assert_dicom_contract(&first_root, first);
    for root in [&first_root, &second_root] {
        let validation = dicom_test_suite::validate_generated_root(root)
            .expect("generated extended root should validate");
        assert!(validation.failures.is_empty(), "{:?}", validation.failures);
        assert_eq!(
            validation.files_checked,
            first_manifest["files"]
                .as_array()
                .expect("manifest files")
                .len()
        );
    }

    fs::remove_dir_all(first_workspace).expect("remove first workspace");
    fs::remove_dir_all(second_workspace).expect("remove second workspace");
}

fn assert_manifest_contract(root: &Path, manifest: &Value, file: &Value) {
    assert_eq!(file["path"], RELATIVE_PATH);
    assert_eq!(
        file.pointer("/recipe/recipe_id"),
        Some(&json!("derived_presentation_state_color_softcopy"))
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

    let source_file = manifest["files"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["case_id"] == SOURCE_CASE_ID)
        .expect("RGB source manifest entry");
    let references = file["references"].as_array().expect("ordinary references");
    assert_eq!(references.len(), 1);
    assert_eq!(references[0]["relationship"], "source_image");
    assert_eq!(references[0]["source_case_id"], SOURCE_CASE_ID);
    assert_eq!(references[0]["source_path"], SOURCE_PATH);
    assert_eq!(references[0]["frame_numbers"], Value::Null);
    assert_eq!(
        references[0]["sop_instance_uid"],
        source_file["uids"]["sop_instance_uid"]
    );
    assert_eq!(
        references[0]["series_instance_uid"],
        source_file["uids"]["series_instance_uid"]
    );

    let expected = &file["expected_color_softcopy_presentation_state"];
    assert_eq!(
        expected["presentation_state"],
        json!({
            "modality": "PR",
            "body_part_examined": "HAND",
            "laterality": "R",
            "content_label": "DTSCOLORPR",
            "content_description": "Synthetic RGB color presentation state",
            "presentation_creation_date": "20260101",
            "presentation_creation_time": "000000",
            "instance_number": 1,
            "series_number": 62
        })
    );
    assert_eq!(expected["source"]["source_case_id"], SOURCE_CASE_ID);
    assert_eq!(expected["source"]["source_path"], SOURCE_PATH);
    assert_eq!(expected["source"]["source_sha256"], source_file["sha256"]);
    assert_eq!(
        expected["source"]["study_instance_uid"],
        source_file["uids"]["study_instance_uid"]
    );
    assert_eq!(
        expected["source"]["series_instance_uid"],
        source_file["uids"]["series_instance_uid"]
    );
    assert_eq!(
        expected["source"]["sop_instance_uid"],
        source_file["uids"]["sop_instance_uid"]
    );
    assert_eq!(expected["source"]["sop_class_uid"], SOURCE_SOP_CLASS_UID);
    assert_eq!(
        expected["source"],
        json!({
            "source_case_id": SOURCE_CASE_ID,
            "source_path": SOURCE_PATH,
            "source_sha256": source_file["sha256"],
            "study_instance_uid": source_file["uids"]["study_instance_uid"],
            "series_instance_uid": source_file["uids"]["series_instance_uid"],
            "sop_class_uid": SOURCE_SOP_CLASS_UID,
            "sop_instance_uid": source_file["uids"]["sop_instance_uid"],
            "rows": 2,
            "columns": 2,
            "photometric_interpretation": "RGB",
            "samples_per_pixel": 3,
            "planar_configuration": 0,
            "complete_instance": true
        })
    );
    assert_eq!(expected["same_study"], true);
    assert_eq!(expected["different_series"], true);
    assert_eq!(
        expected["relationship"],
        json!({
            "referenced_series_items": 1,
            "referenced_image_items": 1,
            "referenced_frame_numbers": [],
            "applies_to_complete_instance": true
        })
    );
    assert_eq!(
        expected["displayed_area"],
        json!({
            "items": 1,
            "applies_to_all_references": true,
            "top_left": [1, 1],
            "bottom_right": [2, 2],
            "presentation_size_mode": "SCALE TO FIT",
            "presentation_pixel_aspect_ratio": [1, 1],
            "presentation_pixel_spacing": null,
            "presentation_pixel_magnification_ratio": null
        })
    );
    assert_eq!(
        expected["icc_profile"],
        json!({
            "vr": "OB",
            "size_bytes": 736,
            "sha256": ICC_PROFILE_SHA256,
            "device_class": "scnr",
            "data_color_space": "RGB ",
            "profile_connection_space": "XYZ ",
            "signature": "acsp",
            "dicom_color_space": "SRGB"
        })
    );
    for field in [
        "shutter_items",
        "graphic_annotation_items",
        "graphic_layer_items",
        "overlay_items",
    ] {
        assert_eq!(expected[field], 0, "{field}");
    }
    assert_eq!(expected["spatial_transform_present"], false);
    assert_eq!(expected["pixel_data_absent"], true);
    assert_eq!(file.pointer("/validation/status"), Some(&json!("passed")));
    assert_eq!(file.pointer("/validation/external"), Some(&json!([])));
    assert_eq!(
        file.pointer("/validation/standards"),
        Some(&json!([
            {
                "name": "color_softcopy_presentation_state_sop_class",
                "status": "passed",
                "message": "SOP Class UID matches Color Softcopy Presentation State Storage in the 2026b reference."
            },
            {
                "name": "explicit_vr_little_endian_transfer_syntax",
                "status": "passed",
                "message": "Transfer Syntax UID matches Explicit VR Little Endian in the 2026b reference."
            },
            {
                "name": "color_softcopy_presentation_state_modules",
                "status": "passed",
                "message": "Color Softcopy relationship, global displayed area, locked ICC profile, and prohibited-content invariants match the recipe."
            }
        ]))
    );
    let internal = file
        .pointer("/validation/internal")
        .and_then(Value::as_array)
        .unwrap();
    assert!(internal.iter().all(|row| row["status"] == "passed"));
    for required in [
        "color_softcopy_part10_preamble",
        "color_softcopy_referenced_series_items",
        "color_softcopy_displayed_area_global",
        "color_softcopy_icc_profile_sha256",
        "color_softcopy_graphic_annotation_sequence_absent",
        "color_softcopy_pixel_data_absent",
        "color_softcopy_source_precheck",
    ] {
        assert!(
            internal.iter().any(|row| row["name"] == required),
            "missing internal evidence {required}"
        );
    }
    assert!(root.join(SOURCE_PATH).exists());
    assert!(
        manifest["files"]
            .as_array()
            .is_some_and(|files| files.len() >= 113)
    );
}

fn assert_dicom_contract(root: &Path, file: &Value) {
    let object = open_file(root.join(RELATIVE_PATH)).expect("open Color Softcopy PR");
    let source = open_file(root.join(SOURCE_PATH)).expect("open RGB source");
    assert_eq!(text(&object, tags::SOP_CLASS_UID), SOP_CLASS_UID);
    assert_eq!(text(&object, tags::MODALITY), "PR");
    assert_eq!(text(&object, tags::BODY_PART_EXAMINED), "HAND");
    assert_eq!(text(&object, tags::LATERALITY), "R");
    assert_eq!(text(&object, tags::CONTENT_LABEL), "DTSCOLORPR");
    assert_eq!(
        text(&object, tags::STUDY_INSTANCE_UID),
        text(&source, tags::STUDY_INSTANCE_UID)
    );
    assert_ne!(
        text(&object, tags::SERIES_INSTANCE_UID),
        text(&source, tags::SERIES_INSTANCE_UID)
    );
    assert_eq!(text(&source, tags::SOP_CLASS_UID), SOURCE_SOP_CLASS_UID);
    assert_eq!(
        source.element(tags::ROWS).unwrap().to_int::<u16>().unwrap(),
        2
    );
    assert_eq!(
        source
            .element(tags::COLUMNS)
            .unwrap()
            .to_int::<u16>()
            .unwrap(),
        2
    );
    assert_eq!(text(&source, tags::PHOTOMETRIC_INTERPRETATION), "RGB");
    assert_eq!(
        source
            .element(tags::SAMPLES_PER_PIXEL)
            .unwrap()
            .to_int::<u16>()
            .unwrap(),
        3
    );
    assert_eq!(
        source
            .element(tags::PLANAR_CONFIGURATION)
            .unwrap()
            .to_int::<u16>()
            .unwrap(),
        0
    );

    let series = sequence(&object, tags::REFERENCED_SERIES_SEQUENCE);
    assert_eq!(series.len(), 1);
    assert_eq!(
        text(&series[0], tags::SERIES_INSTANCE_UID),
        text(&source, tags::SERIES_INSTANCE_UID)
    );
    let images = sequence(&series[0], tags::REFERENCED_IMAGE_SEQUENCE);
    assert_eq!(images.len(), 1);
    assert_eq!(
        text(&images[0], tags::REFERENCED_SOP_CLASS_UID),
        SOURCE_SOP_CLASS_UID
    );
    assert_eq!(
        text(&images[0], tags::REFERENCED_SOP_INSTANCE_UID),
        text(&source, tags::SOP_INSTANCE_UID)
    );
    assert!(images[0].element(tags::REFERENCED_FRAME_NUMBER).is_err());

    let areas = sequence(&object, tags::DISPLAYED_AREA_SELECTION_SEQUENCE);
    assert_eq!(areas.len(), 1);
    assert!(areas[0].element(tags::REFERENCED_IMAGE_SEQUENCE).is_err());
    let top_left = areas[0]
        .element(tags::DISPLAYED_AREA_TOP_LEFT_HAND_CORNER)
        .unwrap();
    assert_eq!(top_left.vr(), VR::SL);
    assert_eq!(top_left.to_multi_int::<i32>().unwrap(), [1, 1]);
    let bottom_right = areas[0]
        .element(tags::DISPLAYED_AREA_BOTTOM_RIGHT_HAND_CORNER)
        .unwrap();
    assert_eq!(bottom_right.vr(), VR::SL);
    assert_eq!(bottom_right.to_multi_int::<i32>().unwrap(), [2, 2]);
    assert_eq!(
        text(&areas[0], tags::PRESENTATION_SIZE_MODE),
        "SCALE TO FIT"
    );
    assert_eq!(
        areas[0]
            .element(tags::PRESENTATION_PIXEL_ASPECT_RATIO)
            .unwrap()
            .to_multi_int::<i32>()
            .unwrap(),
        [1, 1]
    );
    assert!(areas[0].element(tags::PRESENTATION_PIXEL_SPACING).is_err());
    assert!(
        areas[0]
            .element(tags::PRESENTATION_PIXEL_MAGNIFICATION_RATIO)
            .is_err()
    );

    let icc = object.element(tags::ICC_PROFILE).expect("ICC Profile");
    assert_eq!(icc.vr(), VR::OB);
    let icc_bytes = icc.to_bytes().expect("ICC bytes");
    assert_eq!(icc_bytes.len(), 736);
    assert_eq!(
        dicom_test_suite::sha256_hex(icc_bytes.as_ref()),
        ICC_PROFILE_SHA256
    );
    assert_eq!(&icc_bytes[12..16], b"scnr");
    assert_eq!(&icc_bytes[16..20], b"RGB ");
    assert_eq!(&icc_bytes[20..24], b"XYZ ");
    assert_eq!(&icc_bytes[36..40], b"acsp");
    assert_eq!(text(&object, tags::COLOR_SPACE), "SRGB");

    for tag in [
        tags::SHUTTER_SHAPE,
        tags::GRAPHIC_ANNOTATION_SEQUENCE,
        tags::GRAPHIC_LAYER_SEQUENCE,
        tags::IMAGE_ROTATION,
        tags::IMAGE_HORIZONTAL_FLIP,
        tags::PIXEL_DATA,
    ] {
        assert!(object.element(tag).is_err(), "{tag:?} must be absent");
    }
    assert!(
        object
            .tags()
            .all(|tag| !(0x6000..=0x60fe).contains(&tag.group()) || tag.group() % 2 != 0),
        "overlay groups must be absent"
    );
    assert_eq!(
        file["sha256"],
        dicom_test_suite::sha256_hex(&fs::read(root.join(RELATIVE_PATH)).unwrap())
    );
}

fn case_file(manifest: &Value) -> &Value {
    manifest["files"]
        .as_array()
        .unwrap()
        .iter()
        .find(|file| file["case_id"] == CASE_ID)
        .expect("Color Softcopy PR manifest entry")
}

fn generate_extended(workspace: &Path, root: &Path) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .current_dir(workspace)
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

fn temporary_workspace(label: &str) -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let path = std::env::temp_dir().join(format!(
        "dicom-test-suite-{label}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    if path.exists() {
        fs::remove_dir_all(&path).unwrap();
    }
    fs::create_dir_all(path.join("cases")).expect("create temporary project cases directory");
    for metadata in [
        "Cargo.lock",
        "standards.lock.json",
        "generation-backends.lock.json",
    ] {
        fs::copy(repo_path(metadata), path.join(metadata)).expect("copy locked metadata");
    }
    #[cfg(unix)]
    std::os::unix::fs::symlink(
        repo_path("generation-backends"),
        path.join("generation-backends"),
    )
    .expect("link locked generation backends");

    let mut registry = read_repo_json("cases/registry.json");
    let case = registry["cases"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|case| case["case_id"] == CASE_ID)
        .expect("Color Softcopy PR registry row");
    assert!(
        matches!(case["status"].as_str(), Some("planned" | "implemented")),
        "Color Softcopy row must be promotable or already promoted"
    );
    case["status"] = json!("implemented");
    case["blockers"] = json!([]);
    fs::write(
        path.join("cases/registry.json"),
        serde_json::to_vec_pretty(&registry).unwrap(),
    )
    .expect("write temporary registry");
    path
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

fn read_repo_json(path: &str) -> Value {
    serde_json::from_slice(&fs::read(repo_path(path)).unwrap()).unwrap()
}

fn repo_path(path: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(path)
}
