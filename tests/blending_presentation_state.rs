use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use dicom_core::VR;
use dicom_dictionary_std::{tags, uids};
use dicom_object::{InMemDicomObject, open_file};
use serde_json::{Value, json};

const CASE_ID: &str = "derived/presentation-state/blending";
const RELATIVE_PATH: &str = "derived/presentation-state/blending/instance.dcm";
const SOURCE_CASE_ID: &str = "geometry/ct/multiseries_shared_frame_of_reference";
const SOP_CLASS_UID: &str = "1.2.840.10008.5.1.4.1.1.11.4";
const CT_SOP_CLASS_UID: &str = "1.2.840.10008.5.1.4.1.1.2";
const PALETTE_SHA256: &str = "f393097e80ec38db493eb054a0886181eb2c0e8cf7b5cdf1de392fbe94b0d1f5";
const ICC_PROFILE_SHA256: &str = "8e069a3476b71a0e0ae7272d9278ba70540d1c4a0b19af1c7d52e56f49091fef";
const SOURCE_PATHS: [&str; 4] = [
    "geometry/ct/multiseries_shared_frame_of_reference/series-001/slice-001.dcm",
    "geometry/ct/multiseries_shared_frame_of_reference/series-001/slice-002.dcm",
    "geometry/ct/multiseries_shared_frame_of_reference/series-002/slice-001.dcm",
    "geometry/ct/multiseries_shared_frame_of_reference/series-002/slice-002.dcm",
];

#[test]
fn blending_presentation_state_vertical_slice_is_byte_deterministic_and_closed() {
    let first_workspace = temporary_workspace("blending-first");
    let second_workspace = temporary_workspace("blending-second");
    let first_root = first_workspace.join("generated");
    let second_root = second_workspace.join("generated");
    let first_manifest = generate_extended(&first_workspace, &first_root);
    let second_manifest = generate_extended(&second_workspace, &second_root);
    let first = case_file(&first_manifest);
    let second = case_file(&second_manifest);

    let first_bytes = fs::read(first_root.join(RELATIVE_PATH)).expect("first Blending PR");
    let second_bytes = fs::read(second_root.join(RELATIVE_PATH)).expect("second Blending PR");
    assert_eq!(first, second, "seed-7 Blending manifests must match");
    assert_eq!(
        first_bytes, second_bytes,
        "seed-7 Blending bytes must match"
    );
    for source_path in SOURCE_PATHS {
        assert_eq!(
            fs::read(first_root.join(source_path)).expect("first CT source"),
            fs::read(second_root.join(source_path)).expect("second CT source"),
            "seed-7 source bytes must match for {source_path}"
        );
    }
    assert_eq!(first["sha256"], synth_dicom_gen::sha256_hex(&first_bytes));
    assert_eq!(first["determinism"], "byte_stable");

    crate::curated_manifest_contract_support::assert_curated_manifest_schema_valid(&first_manifest);

    assert_manifest_contract(&first_root, &first_manifest, first);
    assert_dicom_contract(&first_root, first);
    for root in [&first_root, &second_root] {
        let validation = synth_dicom_gen::validate_generated_root(root)
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
        Some(&json!("derived_presentation_state_blending"))
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

    let source_files = SOURCE_PATHS.map(|path| {
        manifest["files"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["case_id"] == SOURCE_CASE_ID && entry["path"] == path)
            .expect("ordered CT source manifest entry")
    });
    let study_uid = &source_files[0]["uids"]["study_instance_uid"];
    let frame_uid = &source_files[0]["uids"]["frame_of_reference_uid"];
    assert!(source_files.iter().all(|source| {
        source["uids"]["study_instance_uid"] == *study_uid
            && source["uids"]["frame_of_reference_uid"] == *frame_uid
    }));
    assert_eq!(
        source_files[0]["uids"]["series_instance_uid"],
        source_files[1]["uids"]["series_instance_uid"]
    );
    assert_eq!(
        source_files[2]["uids"]["series_instance_uid"],
        source_files[3]["uids"]["series_instance_uid"]
    );
    assert_ne!(
        source_files[0]["uids"]["series_instance_uid"],
        source_files[2]["uids"]["series_instance_uid"]
    );

    let references = file["references"].as_array().expect("ordinary references");
    assert_eq!(references.len(), 4);
    for (index, reference) in references.iter().enumerate() {
        assert_eq!(reference["relationship"], "blending_source");
        assert_eq!(reference["source_case_id"], SOURCE_CASE_ID);
        assert_eq!(reference["source_path"], SOURCE_PATHS[index]);
        assert_eq!(reference["frame_numbers"], Value::Null);
        assert_eq!(
            reference["sop_instance_uid"],
            source_files[index]["uids"]["sop_instance_uid"]
        );
        assert_eq!(
            reference["series_instance_uid"],
            source_files[index]["uids"]["series_instance_uid"]
        );
    }

    let expected = &file["expected_blending_presentation_state"];
    assert_eq!(expected["same_study"], true);
    assert_eq!(expected["shared_frame_of_reference"], true);
    assert_eq!(expected["different_series"], true);
    assert_eq!(expected["pixel_data_absent"], true);
    assert_eq!(
        expected["presentation_state"],
        json!({
            "study_instance_uid": study_uid,
            "series_instance_uid": file["uids"]["series_instance_uid"],
            "sop_instance_uid": file["uids"]["sop_instance_uid"],
            "modality": "PR",
            "laterality": "R",
            "content_label": "DTSBLEND",
            "content_description": "Synthetic DTSBLEND presentation state",
            "content_creator_name": "DTS^Generator",
            "presentation_creation_date": "20260101",
            "presentation_creation_time": "000000",
            "instance_number": 1,
            "series_number": 81
        })
    );
    let sources = expected["sources"].as_array().expect("locked sources");
    assert_eq!(sources.len(), 4);
    for (index, source) in sources.iter().enumerate() {
        assert_eq!(source["source_case_id"], SOURCE_CASE_ID);
        assert_eq!(source["source_path"], SOURCE_PATHS[index]);
        assert_eq!(source["source_sha256"], source_files[index]["sha256"]);
        assert_eq!(source["study_instance_uid"], *study_uid);
        assert_eq!(source["frame_of_reference_uid"], *frame_uid);
        assert_eq!(
            source["series_instance_uid"],
            source_files[index]["uids"]["series_instance_uid"]
        );
        assert_eq!(source["sop_class_uid"], CT_SOP_CLASS_UID);
        assert_eq!(
            source["sop_instance_uid"],
            source_files[index]["uids"]["sop_instance_uid"]
        );
        assert_eq!(source["series_order"], index / 2 + 1);
        assert_eq!(source["image_order"], index % 2 + 1);
        assert_eq!(source["rows"], 2);
        assert_eq!(source["columns"], 2);
        assert_eq!(
            source["image_orientation_patient"],
            json!([1, 0, 0, 0, 1, 0])
        );
        assert_eq!(
            source["image_position_patient_mm"],
            json!([0, 0, if index % 2 == 0 { 0 } else { 5 }])
        );
        assert_eq!(source["referenced_frame_numbers"], json!([]));
        assert_eq!(source["complete_instance"], true);
    }
    assert_eq!(
        expected["blending_items"],
        json!([
            {"blending_position": "UNDERLYING", "source_series_order": 1, "study_instance_uid": study_uid, "series_instance_uid": source_files[0]["uids"]["series_instance_uid"], "referenced_source_indices": [1, 2], "referenced_frame_numbers": [], "rescale_intercept": -1024, "rescale_slope": 1, "rescale_type": "HU", "softcopy_voi_lut_items": 0, "referenced_spatial_registration_items": 0, "complete_instances": true},
            {"blending_position": "SUPERIMPOSED", "source_series_order": 2, "study_instance_uid": study_uid, "series_instance_uid": source_files[2]["uids"]["series_instance_uid"], "referenced_source_indices": [3, 4], "referenced_frame_numbers": [], "rescale_intercept": -1024, "rescale_slope": 1, "rescale_type": "HU", "softcopy_voi_lut_items": 0, "referenced_spatial_registration_items": 0, "complete_instances": true}
        ])
    );
    assert_eq!(expected["relative_opacity"], 0.5);
    assert_eq!(
        expected["displayed_area"],
        json!({"items": 1, "applies_to_all_references": true, "referenced_image_items": 0, "top_left": [1, 1], "bottom_right": [2, 2], "presentation_size_mode": "SCALE TO FIT", "presentation_pixel_aspect_ratio": [1, 1], "presentation_pixel_spacing": null, "presentation_pixel_magnification_ratio": null})
    );
    assert_eq!(
        expected["palette_color_lut"],
        json!({"channels": [
        {"channel": "red", "descriptor": [256, 0, 16], "data_vr": "OW", "data_size_bytes": 512, "data_sha256": PALETTE_SHA256, "storage": "identity_u16_little_endian"},
        {"channel": "green", "descriptor": [256, 0, 16], "data_vr": "OW", "data_size_bytes": 512, "data_sha256": PALETTE_SHA256, "storage": "identity_u16_little_endian"},
        {"channel": "blue", "descriptor": [256, 0, 16], "data_vr": "OW", "data_size_bytes": 512, "data_sha256": PALETTE_SHA256, "storage": "identity_u16_little_endian"}
    ], "segmented_data_present": false, "palette_uid_present": false})
    );
    assert_eq!(
        expected["icc_profile"],
        json!({"vr": "OB", "size_bytes": 736, "sha256": ICC_PROFILE_SHA256, "device_class": "scnr", "data_color_space": "RGB ", "profile_connection_space": "XYZ ", "signature": "acsp", "dicom_color_space": "SRGB"})
    );
    assert_eq!(
        expected["absent_modules"],
        json!({
            "clinical_trial_subject": true, "clinical_trial_study": true, "clinical_trial_series": true,
            "clinical_trial_equipment": true, "patient_study": true, "specimen": true,
            "graphic_annotation": true, "graphic_layer": true, "graphic_group": true,
            "spatial_transformation": true, "frame_of_reference": true, "common_instance_reference": true,
            "softcopy_presentation_lut": true, "voi_lut": true, "softcopy_voi_lut": true,
            "overlay_plane": true, "overlay_activation": true, "display_shutter": true,
            "bitmap_display_shutter": true
        })
    );

    assert_eq!(file.pointer("/validation/status"), Some(&json!("passed")));
    assert_eq!(file.pointer("/validation/external"), Some(&json!([])));
    let internal = file
        .pointer("/validation/internal")
        .and_then(Value::as_array)
        .unwrap();
    let standards = file
        .pointer("/validation/standards")
        .and_then(Value::as_array)
        .unwrap();
    assert!(
        internal
            .iter()
            .chain(standards)
            .all(|row| row["status"] == "passed")
    );
    for required in [
        "blending_part10_preamble",
        "blending_item_count",
        "blending_item_1_position",
        "blending_item_2_position",
        "blending_item_1_image_1_sop_instance",
        "blending_item_2_image_2_complete_instance",
        "blending_item_1_rescale_intercept",
        "blending_item_2_softcopy_voi_absent",
        "blending_opacity_vr",
        "blending_opacity_range",
        "blending_displayed_area_global",
        "blending_palette_red_data_sha256",
        "blending_palette_blue_data_vr",
        "blending_icc_sha256",
        "blending_frame_of_reference_absent",
        "blending_common_reference_absent",
        "blending_pixel_data_absent",
        "blending_source_precheck",
    ] {
        assert!(
            internal.iter().any(|row| row["name"] == required),
            "missing internal evidence {required}"
        );
    }
    assert_eq!(
        standards
            .iter()
            .map(|row| row["name"].as_str().unwrap())
            .collect::<Vec<_>>(),
        [
            "blending_softcopy_presentation_state_sop_class",
            "explicit_vr_little_endian_transfer_syntax",
            "blending_presentation_state_modules"
        ]
    );
    assert!(SOURCE_PATHS.iter().all(|path| root.join(path).exists()));
}

fn assert_dicom_contract(root: &Path, file: &Value) {
    let object = open_file(root.join(RELATIVE_PATH)).expect("open Blending PR");
    let sources = SOURCE_PATHS.map(|path| open_file(root.join(path)).expect("open source CT"));
    assert_eq!(text(&object, tags::SOP_CLASS_UID), SOP_CLASS_UID);
    assert_eq!(text(&object, tags::MODALITY), "PR");
    assert_eq!(text(&object, tags::LATERALITY), "R");
    assert_eq!(text(&object, tags::CONTENT_LABEL), "DTSBLEND");
    assert_eq!(
        text(&object, tags::CONTENT_DESCRIPTION),
        "Synthetic DTSBLEND presentation state"
    );
    assert_eq!(text(&object, tags::CONTENT_CREATOR_NAME), "DTS^Generator");
    assert!(
        sources
            .iter()
            .all(|source| text(source, tags::STUDY_INSTANCE_UID)
                == text(&object, tags::STUDY_INSTANCE_UID))
    );
    assert_ne!(
        text(&object, tags::SERIES_INSTANCE_UID),
        text(&sources[0], tags::SERIES_INSTANCE_UID)
    );
    assert_ne!(
        text(&object, tags::SERIES_INSTANCE_UID),
        text(&sources[2], tags::SERIES_INSTANCE_UID)
    );

    let blending = sequence(&object, tags::BLENDING_SEQUENCE);
    assert_eq!(blending.len(), 2);
    assert_eq!(
        object.element(tags::BLENDING_SEQUENCE).unwrap().vr(),
        VR::SQ
    );
    for (series_index, item) in blending.iter().enumerate() {
        let first_source = series_index * 2;
        assert_eq!(
            text(item, tags::BLENDING_POSITION),
            if series_index == 0 {
                "UNDERLYING"
            } else {
                "SUPERIMPOSED"
            }
        );
        assert_eq!(item.element(tags::BLENDING_POSITION).unwrap().vr(), VR::CS);
        assert_eq!(
            text(item, tags::STUDY_INSTANCE_UID),
            text(&sources[first_source], tags::STUDY_INSTANCE_UID)
        );
        assert_eq!(text(item, tags::RESCALE_INTERCEPT), "-1024");
        assert_eq!(text(item, tags::RESCALE_SLOPE), "1");
        assert_eq!(text(item, tags::RESCALE_TYPE), "HU");
        assert_eq!(item.element(tags::RESCALE_INTERCEPT).unwrap().vr(), VR::DS);
        assert_eq!(item.element(tags::RESCALE_SLOPE).unwrap().vr(), VR::DS);
        assert_eq!(item.element(tags::RESCALE_TYPE).unwrap().vr(), VR::LO);
        assert!(item.element(tags::SOFTCOPY_VOILUT_SEQUENCE).is_err());
        assert!(
            item.element(tags::REFERENCED_SPATIAL_REGISTRATION_SEQUENCE)
                .is_err()
        );
        let series = sequence(item, tags::REFERENCED_SERIES_SEQUENCE);
        assert_eq!(series.len(), 1);
        assert_eq!(
            text(&series[0], tags::SERIES_INSTANCE_UID),
            text(&sources[first_source], tags::SERIES_INSTANCE_UID)
        );
        let images = sequence(&series[0], tags::REFERENCED_IMAGE_SEQUENCE);
        assert_eq!(images.len(), 2);
        for (image_index, image) in images.iter().enumerate() {
            assert_eq!(
                text(image, tags::REFERENCED_SOP_CLASS_UID),
                CT_SOP_CLASS_UID
            );
            assert_eq!(
                text(image, tags::REFERENCED_SOP_INSTANCE_UID),
                text(&sources[first_source + image_index], tags::SOP_INSTANCE_UID)
            );
            assert!(image.element(tags::REFERENCED_FRAME_NUMBER).is_err());
        }
    }
    let opacity = object
        .element(tags::RELATIVE_OPACITY)
        .expect("Relative Opacity");
    assert_eq!(opacity.vr(), VR::FL);
    assert_eq!(opacity.to_float32().unwrap(), 0.5);

    let areas = sequence(&object, tags::DISPLAYED_AREA_SELECTION_SEQUENCE);
    assert_eq!(areas.len(), 1);
    let area = &areas[0];
    assert_eq!(
        area.element(tags::DISPLAYED_AREA_TOP_LEFT_HAND_CORNER)
            .unwrap()
            .vr(),
        VR::SL
    );
    assert_eq!(
        area.element(tags::DISPLAYED_AREA_TOP_LEFT_HAND_CORNER)
            .unwrap()
            .to_multi_int::<i32>()
            .unwrap(),
        [1, 1]
    );
    assert_eq!(
        area.element(tags::DISPLAYED_AREA_BOTTOM_RIGHT_HAND_CORNER)
            .unwrap()
            .to_multi_int::<i32>()
            .unwrap(),
        [2, 2]
    );
    assert_eq!(text(area, tags::PRESENTATION_SIZE_MODE), "SCALE TO FIT");
    assert_eq!(
        area.element(tags::PRESENTATION_PIXEL_ASPECT_RATIO)
            .unwrap()
            .vr(),
        VR::IS
    );
    assert_eq!(
        area.element(tags::PRESENTATION_PIXEL_ASPECT_RATIO)
            .unwrap()
            .to_multi_int::<i32>()
            .unwrap(),
        [1, 1]
    );
    for tag in [
        tags::REFERENCED_IMAGE_SEQUENCE,
        tags::PRESENTATION_PIXEL_SPACING,
        tags::PRESENTATION_PIXEL_MAGNIFICATION_RATIO,
    ] {
        assert!(area.element(tag).is_err(), "{tag:?} must be absent");
    }

    for (descriptor_tag, data_tag) in [
        (
            tags::RED_PALETTE_COLOR_LOOKUP_TABLE_DESCRIPTOR,
            tags::RED_PALETTE_COLOR_LOOKUP_TABLE_DATA,
        ),
        (
            tags::GREEN_PALETTE_COLOR_LOOKUP_TABLE_DESCRIPTOR,
            tags::GREEN_PALETTE_COLOR_LOOKUP_TABLE_DATA,
        ),
        (
            tags::BLUE_PALETTE_COLOR_LOOKUP_TABLE_DESCRIPTOR,
            tags::BLUE_PALETTE_COLOR_LOOKUP_TABLE_DATA,
        ),
    ] {
        let descriptor = object.element(descriptor_tag).unwrap();
        assert_eq!(descriptor.vr(), VR::US);
        assert_eq!(descriptor.to_multi_int::<u16>().unwrap(), [256, 0, 16]);
        let data = object.element(data_tag).unwrap();
        assert_eq!(data.vr(), VR::OW);
        let bytes = data.to_bytes().unwrap();
        assert_eq!(bytes.len(), 512);
        assert_eq!(synth_dicom_gen::sha256_hex(bytes.as_ref()), PALETTE_SHA256);
    }
    let icc = object.element(tags::ICC_PROFILE).expect("ICC Profile");
    assert_eq!(icc.vr(), VR::OB);
    let icc_bytes = icc.to_bytes().unwrap();
    assert_eq!(icc_bytes.len(), 736);
    assert_eq!(
        synth_dicom_gen::sha256_hex(icc_bytes.as_ref()),
        ICC_PROFILE_SHA256
    );
    assert_eq!(&icc_bytes[12..16], b"scnr");
    assert_eq!(&icc_bytes[16..20], b"RGB ");
    assert_eq!(&icc_bytes[20..24], b"XYZ ");
    assert_eq!(&icc_bytes[36..40], b"acsp");
    assert_eq!(text(&object, tags::COLOR_SPACE), "SRGB");

    for tag in [
        tags::FRAME_OF_REFERENCE_UID,
        tags::POSITION_REFERENCE_INDICATOR,
        tags::REFERENCED_SERIES_SEQUENCE,
        tags::STUDIES_CONTAINING_OTHER_REFERENCED_INSTANCES_SEQUENCE,
        tags::PRESENTATION_LUT_SEQUENCE,
        tags::PRESENTATION_LUT_SHAPE,
        tags::VOILUT_SEQUENCE,
        tags::SOFTCOPY_VOILUT_SEQUENCE,
        tags::GRAPHIC_ANNOTATION_SEQUENCE,
        tags::GRAPHIC_LAYER_SEQUENCE,
        tags::IMAGE_HORIZONTAL_FLIP,
        tags::IMAGE_ROTATION,
        tags::SHUTTER_SHAPE,
        tags::PALETTE_COLOR_LOOKUP_TABLE_UID,
        tags::SEGMENTED_RED_PALETTE_COLOR_LOOKUP_TABLE_DATA,
        tags::SEGMENTED_GREEN_PALETTE_COLOR_LOOKUP_TABLE_DATA,
        tags::SEGMENTED_BLUE_PALETTE_COLOR_LOOKUP_TABLE_DATA,
        tags::PIXEL_DATA,
    ] {
        assert!(object.element(tag).is_err(), "{tag:?} must be absent");
    }
    assert_eq!(
        file["sha256"],
        synth_dicom_gen::sha256_hex(&fs::read(root.join(RELATIVE_PATH)).unwrap())
    );
}

fn case_file(manifest: &Value) -> &Value {
    manifest["files"]
        .as_array()
        .unwrap()
        .iter()
        .find(|file| file["case_id"] == CASE_ID)
        .expect("Blending manifest entry")
}

fn generate_extended(workspace: &Path, root: &Path) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_synth-dicom-gen"))
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
        .expect("Blending registry row");
    assert!(
        matches!(case["status"].as_str(), Some("planned" | "implemented")),
        "Blending row must be promotable or already promoted"
    );
    case["status"] = json!("implemented");
    case["blockers"] = json!([]);
    case["determinism"] = json!("byte_stable");
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
