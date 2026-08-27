use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use dicom_core::VR;
use dicom_dictionary_std::{tags, uids};
use dicom_object::{InMemDicomObject, open_file};
use serde_json::{Value, json};

const CASE_ID: &str = "derived/presentation-state/advanced_blending";
const RELATIVE_PATH: &str = "derived/presentation-state/advanced_blending/instance.dcm";
const SOURCE_CASE_ID: &str = "geometry/ct/multiseries_shared_frame_of_reference";
const SOP_CLASS_UID: &str = "1.2.840.10008.5.1.4.1.1.11.8";
const CT_SOP_CLASS_UID: &str = "1.2.840.10008.5.1.4.1.1.2";
const ICC_PROFILE_SHA256: &str = "8e069a3476b71a0e0ae7272d9278ba70540d1c4a0b19af1c7d52e56f49091fef";
const SOURCE_PATHS: [&str; 4] = [
    "geometry/ct/multiseries_shared_frame_of_reference/series-001/slice-001.dcm",
    "geometry/ct/multiseries_shared_frame_of_reference/series-001/slice-002.dcm",
    "geometry/ct/multiseries_shared_frame_of_reference/series-002/slice-001.dcm",
    "geometry/ct/multiseries_shared_frame_of_reference/series-002/slice-002.dcm",
];

#[test]
fn advanced_blending_vertical_slice_is_byte_deterministic_and_closed() {
    let first_workspace = temporary_workspace("advanced-blending-first");
    let second_workspace = temporary_workspace("advanced-blending-second");
    let first_root = first_workspace.join("generated");
    let second_root = second_workspace.join("generated");
    let first_manifest = generate_extended(&first_workspace, &first_root);
    let second_manifest = generate_extended(&second_workspace, &second_root);
    let first = case_file(&first_manifest);
    let second = case_file(&second_manifest);

    let first_bytes = fs::read(first_root.join(RELATIVE_PATH)).expect("first Advanced Blending PR");
    let second_bytes =
        fs::read(second_root.join(RELATIVE_PATH)).expect("second Advanced Blending PR");
    assert_eq!(
        first, second,
        "seed-7 Advanced Blending manifests must match"
    );
    assert_eq!(
        first_bytes, second_bytes,
        "seed-7 Advanced Blending bytes must match"
    );
    for source_path in SOURCE_PATHS {
        assert_eq!(
            fs::read(first_root.join(source_path)).expect("first CT source"),
            fs::read(second_root.join(source_path)).expect("second CT source"),
            "seed-7 source bytes must match for {source_path}"
        );
    }
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
        assert_eq!(validation.files_checked, 101);
    }

    fs::remove_dir_all(first_workspace).expect("remove first workspace");
    fs::remove_dir_all(second_workspace).expect("remove second workspace");
}

fn assert_manifest_contract(root: &Path, manifest: &Value, file: &Value) {
    assert_eq!(file["path"], RELATIVE_PATH);
    assert_eq!(
        file.pointer("/recipe/recipe_id"),
        Some(&json!("derived_presentation_state_advanced_blending"))
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
        assert_eq!(reference["relationship"], "blending_input");
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

    let expected = &file["expected_advanced_blending_presentation_state"];
    assert_eq!(expected["same_study"], true);
    assert_eq!(expected["shared_frame_of_reference"], true);
    assert_eq!(expected["different_series"], true);
    assert_eq!(expected["pixel_presentation"], "TRUE_COLOR");
    assert_eq!(expected["pixel_data_absent"], true);
    assert_eq!(
        expected["presentation_state"],
        json!({
            "study_instance_uid": study_uid,
            "series_instance_uid": file["uids"]["series_instance_uid"],
            "sop_instance_uid": file["uids"]["sop_instance_uid"],
            "frame_of_reference_uid": frame_uid,
            "position_reference_indicator": "",
            "modality": "PR",
            "laterality": "R",
            "content_label": "DTSADVBLEND",
            "content_description": "Synthetic DTSADVBLEND presentation state",
            "content_creator_name": "DTS^Generator",
            "presentation_creation_date": "20260101",
            "presentation_creation_time": "000000",
            "instance_number": 1,
            "series_number": 80
        })
    );
    let sources = expected["sources"].as_array().expect("locked sources");
    assert_eq!(sources.len(), 4);
    for (index, source) in sources.iter().enumerate() {
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
        expected["blending_inputs"],
        json!([
            {"input_number": 1, "source_series_order": 1, "study_instance_uid": study_uid, "series_instance_uid": source_files[0]["uids"]["series_instance_uid"], "referenced_source_indices": [1, 2], "time_series_blending": "FALSE", "geometry_for_display": "TRUE", "complete_instances": true},
            {"input_number": 2, "source_series_order": 2, "study_instance_uid": study_uid, "series_instance_uid": source_files[2]["uids"]["series_instance_uid"], "referenced_source_indices": [3, 4], "time_series_blending": "FALSE", "geometry_for_display": "FALSE", "complete_instances": true}
        ])
    );
    assert_eq!(
        expected["display_operation"],
        json!({"items": 1, "input_numbers": [1, 2], "blending_mode": "EQUAL", "relative_opacity": null, "output_blending_input_number": null, "final_output": true})
    );
    assert_eq!(
        expected["icc_profile"],
        json!({"vr": "OB", "size_bytes": 736, "sha256": ICC_PROFILE_SHA256, "device_class": "scnr", "data_color_space": "RGB ", "profile_connection_space": "XYZ ", "signature": "acsp", "dicom_color_space": "SRGB"})
    );
    assert_eq!(
        expected["common_instance_reference"],
        json!({
            "series": [
                {"series_order": 1, "series_instance_uid": source_files[0]["uids"]["series_instance_uid"], "referenced_source_indices": [1, 2]},
                {"series_order": 2, "series_instance_uid": source_files[2]["uids"]["series_instance_uid"], "referenced_source_indices": [3, 4]}
            ],
            "other_study_items": 0,
            "mirrors_blending_inputs": true
        })
    );
    assert_eq!(
        expected["optional_transforms"],
        json!({"referenced_spatial_registration_items": 0, "optical_path_selection_items": 0, "softcopy_voi_lut_items": 0, "palette_color_lut_items": 0, "threshold_items": 0, "displayed_area_items": 0, "graphic_annotation_items": 0, "graphic_group_items": 0, "specimen_items": 0, "spatial_transform_present": false, "graphic_layer_items": 0})
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
        "advanced_blending_part10_preamble",
        "advanced_blending_input_count",
        "advanced_blending_input_1_number",
        "advanced_blending_input_2_number",
        "advanced_blending_single_geometry_source",
        "advanced_blending_display_input_1_order",
        "advanced_blending_display_input_2_order",
        "advanced_blending_mode",
        "advanced_blending_display_relative_opacity_absent",
        "advanced_blending_display_output_input_number_absent",
        "advanced_blending_icc_sha256",
        "advanced_blending_common_series_count",
        "advanced_blending_other_studies_absent",
        "advanced_blending_pixel_data_absent",
        "advanced_blending_source_precheck",
    ] {
        assert!(
            internal.iter().any(|row| row["name"] == required),
            "missing internal evidence {required}"
        );
    }
    let standard_names = standards
        .iter()
        .map(|row| row["name"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        standard_names,
        [
            "sop_class_uid",
            "explicit_vr_little_endian_transfer_syntax",
            "advanced_blending_presentation_state_modules"
        ]
    );
    assert!(SOURCE_PATHS.iter().all(|path| root.join(path).exists()));
    assert_eq!(manifest["files"].as_array().map(Vec::len), Some(101));
}

fn assert_dicom_contract(root: &Path, file: &Value) {
    let object = open_file(root.join(RELATIVE_PATH)).expect("open Advanced Blending PR");
    let sources = SOURCE_PATHS.map(|path| open_file(root.join(path)).expect("open source CT"));
    assert_eq!(text(&object, tags::SOP_CLASS_UID), SOP_CLASS_UID);
    assert_eq!(text(&object, tags::MODALITY), "PR");
    assert_eq!(text(&object, tags::PIXEL_PRESENTATION), "TRUE_COLOR");
    assert_eq!(text(&object, tags::CONTENT_LABEL), "DTSADVBLEND");
    assert_eq!(
        text(&object, tags::FRAME_OF_REFERENCE_UID),
        text(&sources[0], tags::FRAME_OF_REFERENCE_UID)
    );
    assert!(
        sources
            .iter()
            .all(|source| text(source, tags::STUDY_INSTANCE_UID)
                == text(&object, tags::STUDY_INSTANCE_UID))
    );

    let inputs = sequence(&object, tags::ADVANCED_BLENDING_SEQUENCE);
    assert_eq!(inputs.len(), 2);
    for (input_index, input) in inputs.iter().enumerate() {
        assert_eq!(
            number(input, tags::BLENDING_INPUT_NUMBER),
            input_index as u16 + 1
        );
        assert_eq!(text(input, tags::TIME_SERIES_BLENDING), "FALSE");
        assert_eq!(
            text(input, tags::GEOMETRY_FOR_DISPLAY),
            if input_index == 0 { "TRUE" } else { "FALSE" }
        );
        let first_source = input_index * 2;
        assert_eq!(
            text(input, tags::SERIES_INSTANCE_UID),
            text(&sources[first_source], tags::SERIES_INSTANCE_UID)
        );
        let images = sequence(input, tags::REFERENCED_IMAGE_SEQUENCE);
        assert_eq!(images.len(), 2);
        for (image_index, image) in images.iter().enumerate() {
            let source = &sources[first_source + image_index];
            assert_eq!(
                text(image, tags::REFERENCED_SOP_CLASS_UID),
                CT_SOP_CLASS_UID
            );
            assert_eq!(
                text(image, tags::REFERENCED_SOP_INSTANCE_UID),
                text(source, tags::SOP_INSTANCE_UID)
            );
            assert!(image.element(tags::REFERENCED_FRAME_NUMBER).is_err());
        }
    }

    let displays = sequence(&object, tags::BLENDING_DISPLAY_SEQUENCE);
    assert_eq!(displays.len(), 1);
    assert_eq!(text(&displays[0], tags::BLENDING_MODE), "EQUAL");
    assert!(displays[0].element(tags::RELATIVE_OPACITY).is_err());
    assert!(displays[0].element(tags::BLENDING_INPUT_NUMBER).is_err());
    let display_inputs = sequence(&displays[0], tags::BLENDING_DISPLAY_INPUT_SEQUENCE);
    assert_eq!(display_inputs.len(), 2);
    assert_eq!(number(&display_inputs[0], tags::BLENDING_INPUT_NUMBER), 1);
    assert_eq!(number(&display_inputs[1], tags::BLENDING_INPUT_NUMBER), 2);

    let common_series = sequence(&object, tags::REFERENCED_SERIES_SEQUENCE);
    assert_eq!(common_series.len(), 2);
    for (series_index, series) in common_series.iter().enumerate() {
        let first_source = series_index * 2;
        assert_eq!(
            text(series, tags::SERIES_INSTANCE_UID),
            text(&sources[first_source], tags::SERIES_INSTANCE_UID)
        );
        let instances = sequence(series, tags::REFERENCED_INSTANCE_SEQUENCE);
        assert_eq!(instances.len(), 2);
        for (image_index, instance) in instances.iter().enumerate() {
            assert_eq!(
                text(instance, tags::REFERENCED_SOP_INSTANCE_UID),
                text(&sources[first_source + image_index], tags::SOP_INSTANCE_UID)
            );
        }
    }

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
        tags::STUDIES_CONTAINING_OTHER_REFERENCED_INSTANCES_SEQUENCE,
        tags::DISPLAYED_AREA_SELECTION_SEQUENCE,
        tags::GRAPHIC_ANNOTATION_SEQUENCE,
        tags::GRAPHIC_LAYER_SEQUENCE,
        tags::IMAGE_HORIZONTAL_FLIP,
        tags::IMAGE_ROTATION,
        tags::PIXEL_DATA,
    ] {
        assert!(object.element(tag).is_err(), "{tag:?} must be absent");
    }
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
        .expect("Advanced Blending manifest entry")
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
        .expect("Advanced Blending registry row");
    assert!(
        matches!(case["status"].as_str(), Some("planned" | "implemented")),
        "Advanced Blending row must be promotable or already promoted"
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

fn number(object: &InMemDicomObject, tag: dicom_core::Tag) -> u16 {
    object
        .element(tag)
        .expect("number element")
        .to_int::<u16>()
        .expect("u16")
}

fn read_repo_json(path: &str) -> Value {
    serde_json::from_slice(&fs::read(repo_path(path)).unwrap()).unwrap()
}

fn repo_path(path: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(path)
}
