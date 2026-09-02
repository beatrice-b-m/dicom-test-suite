use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use dicom_core::VR;
use dicom_dictionary_std::{tags, uids};
use dicom_object::open_file;
use serde_json::{Value, json};

const CASE_ID: &str = "classic/xa/monoplane_explicit_le";
const RELATIVE_PATH: &str = "classic/xa/monoplane_explicit_le/instance.dcm";
const INSTANCE_SHA256: &str = "7efc114021a4a292e7170055f92948823844192d3f3609509a73b8e2b97dc824";
const PAYLOAD_SHA256: &str = "0b9c742cc3fafec4c1d0240048d27210f2da155b3574458ae26035ffa488c00e";
const PIXELS: [u8; 16] = [
    0, 16, 32, 48, 16, 64, 96, 64, 32, 96, 255, 96, 48, 64, 96, 64,
];

#[test]
fn xa_monoplane_vertical_slice_is_exact_byte_stable_and_reported() {
    let first_root = unique_temp_dir("xa-first");
    let second_root = unique_temp_dir("xa-second");
    let first_manifest = generate_core(&first_root);
    let second_manifest = generate_core(&second_root);
    let first = case_file(&first_manifest);
    let second = case_file(&second_manifest);

    assert_eq!(first["sha256"], INSTANCE_SHA256);
    assert_eq!(second["sha256"], INSTANCE_SHA256);
    assert_eq!(
        synth_dicom_gen::sha256_hex(&fs::read(first_root.join(RELATIVE_PATH)).unwrap()),
        INSTANCE_SHA256
    );
    assert_eq!(
        fs::read(first_root.join(RELATIVE_PATH)).unwrap(),
        fs::read(second_root.join(RELATIVE_PATH)).unwrap()
    );
    crate::curated_manifest_contract_support::assert_curated_manifest_schema_valid(&first_manifest);

    assert_eq!(
        first["dicom"],
        json!({
            "sop_class_uid": uids::X_RAY_ANGIOGRAPHIC_IMAGE_STORAGE,
            "sop_class_name": "X-Ray Angiographic Image Storage",
            "iod_name": "X-Ray Angiographic Image",
            "modality": "XA",
            "transfer_syntax_uid": uids::EXPLICIT_VR_LITTLE_ENDIAN,
            "transfer_syntax_name": "Explicit VR Little Endian"
        })
    );
    assert_eq!(
        first["image"],
        json!({
            "rows": 4, "columns": 4, "frames": 1, "samples_per_pixel": 1,
            "photometric_interpretation": "MONOCHROME2", "bits_allocated": 8,
            "bits_stored": 8, "high_bit": 7, "pixel_representation": 0,
            "planar_configuration": null
        })
    );
    assert_eq!(
        first["pixel_data"],
        json!({
            "vr": "OB", "native_or_encapsulated": "native", "value_length": 16,
            "frame_count": 1, "frame_hashes": [PAYLOAD_SHA256]
        })
    );
    let expected_projection = json!({
        "image_type": ["ORIGINAL", "PRIMARY", "SINGLE PLANE"],
        "frame_count": 1,
        "body_part_examined": "HEART",
        "patient_orientation_empty": true,
        "laterality_present": false,
        "pixel_intensity_relationship": "LIN",
        "radiation_setting": "GR",
        "kvp": 80.0,
        "exposure_mas": 4,
        "imager_pixel_spacing_mm": [0.2, 0.2],
        "positioner_primary_angle_degrees": 15.0,
        "positioner_secondary_angle_degrees": -10.0,
        "distance_source_to_detector_mm": 1200.0,
        "distance_source_to_patient_mm": 800.0,
        "estimated_radiographic_magnification_factor": 1.5,
        "lossy_image_compression": "00",
        "multiframe_cine": false,
        "biplane_data_present": false,
        "contrast_used": false,
        "subtraction_applied": false,
        "table_motion_present": false,
        "patient_space_geometry_present": false,
        "pixel_spacing_calibrated": false
    });
    assert_eq!(first["expected_xa_projection"], expected_projection);
    assert_eq!(
        first.pointer("/recipe/recipe_parameters/xa_projection"),
        Some(&expected_projection)
    );
    assert_eq!(
        first.pointer("/recipe/recipe_parameters/payload_sha256"),
        Some(&Value::from(PAYLOAD_SHA256))
    );

    let internal_names = first["validation"]["internal"]
        .as_array()
        .unwrap()
        .iter()
        .map(|check| check["name"].as_str().unwrap())
        .collect::<Vec<_>>();
    for name in [
        "native_pixel_data_length",
        "native_frame_hash_count",
        "native_frame_hashes",
        "xa_modality",
        "xa_body_part_examined",
        "xa_image_type",
        "xa_patient_orientation_empty",
        "xa_pixel_intensity_relationship",
        "xa_lossy_image_compression",
        "xa_radiation_setting",
        "xa_kvp",
        "xa_exposure",
        "xa_imager_pixel_spacing",
        "xa_positioner_primary_angle",
        "xa_positioner_secondary_angle",
        "xa_distance_source_to_detector",
        "xa_distance_source_to_patient",
        "xa_estimated_magnification",
        "xa_image_type_vr",
        "xa_patient_orientation_vr",
        "xa_kvp_vr",
        "xa_exposure_vr",
        "xa_imager_pixel_spacing_vr",
        "xa_positioner_primary_angle_vr",
        "xa_positioner_secondary_angle_vr",
        "xa_sid_sod_magnification_relation",
        "xa_laterality_absent",
        "xa_number_of_frames_absent",
        "xa_frame_increment_pointer_absent",
        "xa_frame_time_absent",
        "xa_frame_time_vector_absent",
        "xa_positioner_motion_absent",
        "xa_primary_angle_increment_absent",
        "xa_secondary_angle_increment_absent",
        "xa_biplane_reference_absent",
        "xa_contrast_agent_absent",
        "xa_mask_subtraction_absent",
        "xa_frame_of_reference_absent",
        "xa_image_orientation_patient_absent",
        "xa_image_position_patient_absent",
        "xa_pixel_spacing_absent",
        "xa_modality_lut_absent",
        "xa_voi_lut_absent",
        "xa_calibration_image_absent",
    ] {
        assert!(
            internal_names.contains(&name),
            "missing internal validation {name}"
        );
    }

    let object = open_file(first_root.join(RELATIVE_PATH)).expect("XA fixture must parse");
    assert_eq!(
        object.meta().transfer_syntax.trim_end_matches('\0'),
        uids::EXPLICIT_VR_LITTLE_ENDIAN
    );
    assert_eq!(
        object
            .element(tags::SOP_CLASS_UID)
            .unwrap()
            .to_str()
            .unwrap(),
        uids::X_RAY_ANGIOGRAPHIC_IMAGE_STORAGE
    );
    for (tag, vr, expected) in [
        (tags::MODALITY, VR::CS, "XA"),
        (tags::IMAGE_TYPE, VR::CS, "ORIGINAL\\PRIMARY\\SINGLE PLANE"),
        (tags::BODY_PART_EXAMINED, VR::CS, "HEART"),
        (tags::PATIENT_ORIENTATION, VR::CS, ""),
        (tags::PIXEL_INTENSITY_RELATIONSHIP, VR::CS, "LIN"),
        (tags::LOSSY_IMAGE_COMPRESSION, VR::CS, "00"),
        (tags::RADIATION_SETTING, VR::CS, "GR"),
        (tags::KVP, VR::DS, "80"),
        (tags::EXPOSURE, VR::IS, "4"),
        (tags::IMAGER_PIXEL_SPACING, VR::DS, "0.2\\0.2"),
        (tags::POSITIONER_PRIMARY_ANGLE, VR::DS, "15"),
        (tags::POSITIONER_SECONDARY_ANGLE, VR::DS, "-10"),
        (tags::DISTANCE_SOURCE_TO_DETECTOR, VR::DS, "1200"),
        (tags::DISTANCE_SOURCE_TO_PATIENT, VR::DS, "800"),
        (
            tags::ESTIMATED_RADIOGRAPHIC_MAGNIFICATION_FACTOR,
            VR::DS,
            "1.5",
        ),
    ] {
        let element = object.element(tag).unwrap();
        assert_eq!(element.vr(), vr, "unexpected VR for {tag:?}");
        assert_eq!(
            element.to_str().unwrap(),
            expected,
            "unexpected value for {tag:?}"
        );
    }
    assert_eq!(
        object
            .element(tags::PATIENT_ORIENTATION)
            .unwrap()
            .value()
            .to_bytes()
            .unwrap()
            .len(),
        0,
        "Patient Orientation must be present with zero value length"
    );
    for tag in [
        tags::LATERALITY,
        tags::NUMBER_OF_FRAMES,
        tags::FRAME_INCREMENT_POINTER,
        tags::FRAME_TIME,
        tags::FRAME_TIME_VECTOR,
        tags::POSITIONER_MOTION,
        tags::POSITIONER_PRIMARY_ANGLE_INCREMENT,
        tags::POSITIONER_SECONDARY_ANGLE_INCREMENT,
        tags::REFERENCED_IMAGE_SEQUENCE,
        tags::CONTRAST_BOLUS_AGENT,
        tags::MASK_SUBTRACTION_SEQUENCE,
        tags::TABLE_MOTION,
        tags::FRAME_OF_REFERENCE_UID,
        tags::IMAGE_ORIENTATION_PATIENT,
        tags::IMAGE_POSITION_PATIENT,
        tags::PIXEL_SPACING,
        tags::MODALITY_LUT_SEQUENCE,
        tags::VOILUT_SEQUENCE,
        tags::CALIBRATION_IMAGE,
        tags::LOSSY_IMAGE_COMPRESSION_RATIO,
        tags::LOSSY_IMAGE_COMPRESSION_METHOD,
    ] {
        assert!(object.element(tag).is_err(), "unexpected element {tag:?}");
    }
    let pixel = object.element(tags::PIXEL_DATA).unwrap();
    assert_eq!(pixel.vr(), VR::OB);
    assert_eq!(pixel.value().to_bytes().unwrap().as_ref(), PIXELS);
    assert_eq!(synth_dicom_gen::sha256_hex(&PIXELS), PAYLOAD_SHA256);

    let summary = synth_dicom_gen::validate_generated_root(&first_root).unwrap();
    assert_eq!(summary.files_checked, 49);
    assert!(summary.failures.is_empty(), "{:?}", summary.failures);

    let report = report_json(&first_root);
    crate::coverage_report::assert_current_contract(&first_root, &report);
    let row = report["coverage_matrix"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["case_id"] == CASE_ID)
        .unwrap();
    for (field, expected) in [
        ("xa_image_type", json!("ORIGINAL; PRIMARY; SINGLE PLANE")),
        ("xa_frame_count", json!(1)),
        ("xa_body_part_examined", json!("HEART")),
        ("xa_patient_orientation_empty", json!(true)),
        ("xa_laterality_present", json!(false)),
        ("xa_pixel_intensity_relationship", json!("LIN")),
        ("xa_radiation_setting", json!("GR")),
        ("xa_kvp", json!(80.0)),
        ("xa_exposure_mas", json!(4)),
        ("xa_imager_pixel_spacing_mm", json!("0.2; 0.2")),
        ("xa_positioner_primary_angle_degrees", json!(15.0)),
        ("xa_positioner_secondary_angle_degrees", json!(-10.0)),
        ("xa_distance_source_to_detector_mm", json!(1200.0)),
        ("xa_distance_source_to_patient_mm", json!(800.0)),
        ("xa_estimated_radiographic_magnification_factor", json!(1.5)),
        ("xa_lossy_image_compression", json!("00")),
        ("xa_multiframe_cine", json!(false)),
        ("xa_biplane_data_present", json!(false)),
        ("xa_contrast_used", json!(false)),
        ("xa_subtraction_applied", json!(false)),
        ("xa_table_motion_present", json!(false)),
        ("xa_patient_space_geometry_present", json!(false)),
        ("xa_pixel_spacing_calibrated", json!(false)),
    ] {
        assert_eq!(row[field], expected, "unexpected report field {field}");
    }
    for pointer in [
        "/grouped_coverage/xa_image_types/ORIGINAL; PRIMARY; SINGLE PLANE",
        "/grouped_coverage/xa_frame_counts/1",
        "/grouped_coverage/xa_body_parts_examined/HEART",
        "/grouped_coverage/xa_patient_orientation_empty_states/true",
        "/grouped_coverage/xa_laterality_present_states/false",
        "/grouped_coverage/xa_pixel_intensity_relationships/LIN",
        "/grouped_coverage/xa_radiation_settings/GR",
        "/grouped_coverage/xa_kvps/80.0",
        "/grouped_coverage/xa_exposures_mas/4",
        "/grouped_coverage/xa_imager_pixel_spacings_mm/0.2; 0.2",
        "/grouped_coverage/xa_positioner_primary_angles_degrees/15.0",
        "/grouped_coverage/xa_positioner_secondary_angles_degrees/-10.0",
        "/grouped_coverage/xa_distances_source_to_detector_mm/1200.0",
        "/grouped_coverage/xa_distances_source_to_patient_mm/800.0",
        "/grouped_coverage/xa_estimated_radiographic_magnification_factors/1.5",
        "/grouped_coverage/xa_lossy_image_compressions/00",
        "/grouped_coverage/xa_multiframe_cine_states/false",
        "/grouped_coverage/xa_biplane_data_present_states/false",
        "/grouped_coverage/xa_contrast_used_states/false",
        "/grouped_coverage/xa_subtraction_applied_states/false",
        "/grouped_coverage/xa_table_motion_present_states/false",
        "/grouped_coverage/xa_patient_space_geometry_present_states/false",
        "/grouped_coverage/xa_pixel_spacing_calibrated_states/false",
    ] {
        assert_eq!(report.pointer(pointer), Some(&Value::from(1)), "{pointer}");
    }
    let markdown = synth_dicom_gen::render_coverage_report_markdown(&report);
    assert!(markdown.contains("## X-Ray Angiographic Projection Expectations"));
    assert!(markdown.contains(CASE_ID));
    assert!(markdown.contains("ORIGINAL; PRIMARY; SINGLE PLANE"));
    assert!(markdown.contains("Source-to-patient distance (mm)"));
    assert!(markdown.contains("## XA Positioner Primary Angles (degrees)"));
    assert!(markdown.contains("## XA Patient-space Geometry Present States"));
}

#[test]
fn validator_rejects_tampered_xa_projection_contract() {
    let root = unique_temp_dir("xa-tampered");
    let manifest = generate_core(&root);
    let mut schema_invalid = manifest.clone();
    *case_file_mut(&mut schema_invalid)
        .pointer_mut("/expected_xa_projection/image_type/2")
        .unwrap() = json!("BIPLANE A");
    crate::curated_manifest_contract_support::assert_curated_manifest_schema_rejected(
        &schema_invalid,
    );
    write_manifest(&root, &schema_invalid);
    let error = synth_dicom_gen::validate_generated_root(&root).unwrap_err();
    assert!(error.to_string().contains("manifest schema invalid"));

    for (pointer, replacement, failure_key) in [
        (
            "/expected_xa_projection/body_part_examined",
            json!("CHEST"),
            "xa_body_part_examined_manifest_contract",
        ),
        (
            "/expected_xa_projection/patient_orientation_empty",
            json!(false),
            "xa_patient_orientation_empty_manifest_contract",
        ),
        (
            "/expected_xa_projection/laterality_present",
            json!(true),
            "xa_laterality_present_manifest_contract",
        ),
        (
            "/expected_xa_projection/pixel_intensity_relationship",
            json!("LOG"),
            "xa_pixel_intensity_relationship_manifest_contract",
        ),
        (
            "/expected_xa_projection/radiation_setting",
            json!("SC"),
            "xa_radiation_setting_manifest_contract",
        ),
        (
            "/expected_xa_projection/kvp",
            json!(81.0),
            "xa_kvp_manifest_contract",
        ),
        (
            "/expected_xa_projection/exposure_mas",
            json!(5),
            "xa_exposure_manifest_contract",
        ),
        (
            "/expected_xa_projection/imager_pixel_spacing_mm/0",
            json!(0.3),
            "xa_imager_pixel_spacing_manifest_contract",
        ),
        (
            "/expected_xa_projection/positioner_primary_angle_degrees",
            json!(16.0),
            "xa_positioner_primary_angle_manifest_contract",
        ),
        (
            "/expected_xa_projection/positioner_secondary_angle_degrees",
            json!(-11.0),
            "xa_positioner_secondary_angle_manifest_contract",
        ),
        (
            "/expected_xa_projection/distance_source_to_detector_mm",
            json!(1201.0),
            "xa_distance_source_to_detector_manifest_contract",
        ),
        (
            "/expected_xa_projection/distance_source_to_patient_mm",
            json!(801.0),
            "xa_distance_source_to_patient_manifest_contract",
        ),
        (
            "/expected_xa_projection/estimated_radiographic_magnification_factor",
            json!(1.6),
            "xa_estimated_magnification_manifest_contract",
        ),
        (
            "/expected_xa_projection/lossy_image_compression",
            json!("01"),
            "xa_lossy_image_compression_manifest_contract",
        ),
        (
            "/expected_xa_projection/frame_count",
            json!(2),
            "xa_frame_count_manifest_contract",
        ),
        (
            "/expected_xa_projection/multiframe_cine",
            json!(true),
            "xa_multiframe_cine_manifest_contract",
        ),
        (
            "/expected_xa_projection/biplane_data_present",
            json!(true),
            "xa_biplane_data_present_manifest_contract",
        ),
        (
            "/expected_xa_projection/contrast_used",
            json!(true),
            "xa_contrast_used_manifest_contract",
        ),
        (
            "/expected_xa_projection/subtraction_applied",
            json!(true),
            "xa_subtraction_applied_manifest_contract",
        ),
        (
            "/expected_xa_projection/table_motion_present",
            json!(true),
            "xa_table_motion_present_manifest_contract",
        ),
        (
            "/expected_xa_projection/patient_space_geometry_present",
            json!(true),
            "xa_patient_space_geometry_present_manifest_contract",
        ),
        (
            "/expected_xa_projection/pixel_spacing_calibrated",
            json!(true),
            "xa_pixel_spacing_calibrated_manifest_contract",
        ),
        (
            "/recipe/recipe_parameters/payload_sha256",
            json!("ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"),
            "xa_payload_hash_manifest_contract",
        ),
        (
            "/recipe/recipe_parameters/payload_sha256",
            json!("ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"),
            "xa_payload_hash",
        ),
        (
            "/pixel_data/frame_hashes/0",
            json!("ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"),
            "native_pixel_data_frame_hash",
        ),
    ] {
        let mut tampered = manifest.clone();
        *case_file_mut(&mut tampered).pointer_mut(pointer).unwrap() = replacement;
        write_manifest(&root, &tampered);
        if !crate::curated_manifest_contract_support::curated_manifest_schema_is_valid(&tampered) {
            let error = synth_dicom_gen::validate_generated_root(&root).unwrap_err();
            assert!(
                error.to_string().contains("manifest schema invalid"),
                "schema-invalid {pointer} mutation was not rejected first: {error}"
            );
            continue;
        }
        let summary = synth_dicom_gen::validate_generated_root(&root).unwrap();
        assert!(
            summary
                .failures
                .iter()
                .any(|failure| failure.contains(failure_key)),
            "missing {failure_key} for {pointer}: {:?}",
            summary.failures
        );
    }
}

fn case_file(manifest: &Value) -> &Value {
    manifest["files"]
        .as_array()
        .unwrap()
        .iter()
        .find(|file| file["case_id"] == CASE_ID)
        .expect("XA manifest entry must exist")
}

fn case_file_mut(manifest: &mut Value) -> &mut Value {
    manifest["files"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|file| file["case_id"] == CASE_ID)
        .expect("XA manifest entry must exist")
}

fn generate_core(out_dir: &Path) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_synth-dicom-gen"))
        .args(["generate", "--profile", "core", "--out"])
        .arg(out_dir)
        .args(["--seed", "1"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    read_json(out_dir.join("manifest.json"))
}

fn report_json(root: &Path) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_synth-dicom-gen"))
        .arg("report")
        .arg(root)
        .args(["--format", "json"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

fn write_manifest(root: &Path, manifest: &Value) {
    fs::write(
        root.join("manifest.json"),
        serde_json::to_vec_pretty(manifest).unwrap(),
    )
    .unwrap();
}

fn read_json(path: impl AsRef<Path>) -> Value {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

fn unique_temp_dir(label: &str) -> PathBuf {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "dicom-test-suite-{label}-{}-{nonce}",
        std::process::id()
    ))
}
