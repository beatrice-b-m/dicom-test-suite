use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use dicom_core::{Tag, VR};
use dicom_dictionary_std::{tags, uids};
use dicom_object::open_file;
use serde_json::{Value, json};

const CASE_ID: &str = "classic/xrf/monoplane_explicit_le";
const RELATIVE_PATH: &str = "classic/xrf/monoplane_explicit_le/instance.dcm";
const INSTANCE_SHA256: &str = "da7415ddb66c2cce4a3e8c27eb4f5a04a6f03b3bfb9402346fe13a41fadf30ff";
const PAYLOAD_SHA256: &str = "0b9c742cc3fafec4c1d0240048d27210f2da155b3574458ae26035ffa488c00e";
const PIXELS: [u8; 16] = [
    0, 16, 32, 48, 16, 64, 96, 64, 32, 96, 255, 96, 48, 64, 96, 64,
];

#[test]
fn xrf_monoplane_vertical_slice_is_exact_byte_stable_and_reported() {
    let first_root = unique_temp_dir("xrf-first");
    let second_root = unique_temp_dir("xrf-second");
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
    assert!(
        jsonschema::validator_for(&read_json("schemas/manifest.schema.json"))
            .unwrap()
            .is_valid(&first_manifest)
    );

    assert_eq!(
        first["dicom"],
        json!({
            "sop_class_uid": uids::X_RAY_RADIOFLUOROSCOPIC_IMAGE_STORAGE,
            "sop_class_name": "X-Ray Radiofluoroscopic Image Storage",
            "iod_name": "X-Ray Radiofluoroscopic Image",
            "modality": "RF",
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
        "body_part_examined": "ABDOMEN",
        "patient_orientation_empty": true,
        "laterality_present": false,
        "pixel_intensity_relationship": "LIN",
        "radiation_setting": "SC",
        "kvp": 70.0,
        "exposure_mas": 1,
        "imager_pixel_spacing_mm": [0.2, 0.2],
        "distance_source_to_detector_mm": 1200.0,
        "distance_source_to_patient_mm": 800.0,
        "estimated_radiographic_magnification_factor": 1.5,
        "column_angulation_degrees": 10.0,
        "lossy_image_compression": "00",
        "multiframe_cine": false,
        "biplane_data_present": false,
        "contrast_used": false,
        "subtraction_applied": false,
        "table_position_present": false,
        "table_motion_present": false,
        "table_tilt_present": false,
        "tomography_present": false,
        "patient_space_geometry_present": false,
        "pixel_spacing_calibrated": false,
        "xa_positioner_angles_present": false
    });
    assert_eq!(first["expected_xrf_projection"], expected_projection);
    assert_eq!(
        first.pointer("/recipe/recipe_parameters/xrf_projection"),
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
        "xrf_modality",
        "xrf_body_part_examined",
        "xrf_image_type",
        "xrf_patient_orientation_empty",
        "xrf_pixel_intensity_relationship",
        "xrf_lossy_image_compression",
        "xrf_radiation_setting",
        "xrf_kvp",
        "xrf_exposure",
        "xrf_imager_pixel_spacing",
        "xrf_distance_source_to_detector",
        "xrf_distance_source_to_patient",
        "xrf_estimated_magnification",
        "xrf_column_angulation",
        "xrf_sid_sod_magnification_relation",
        "xrf_modality_vr",
        "xrf_body_part_examined_vr",
        "xrf_image_type_vr",
        "xrf_patient_orientation_vr",
        "xrf_pixel_intensity_relationship_vr",
        "xrf_lossy_image_compression_vr",
        "xrf_radiation_setting_vr",
        "xrf_kvp_vr",
        "xrf_exposure_vr",
        "xrf_imager_pixel_spacing_vr",
        "xrf_distance_source_to_detector_vr",
        "xrf_distance_source_to_patient_vr",
        "xrf_estimated_magnification_vr",
        "xrf_column_angulation_vr",
        "xrf_positioner_primary_angle_absent",
        "xrf_positioner_secondary_angle_absent",
        "xrf_number_of_frames_absent",
        "xrf_table_position_absent",
        "xrf_table_motion_absent",
        "xrf_tomo_type_absent",
        "xrf_frame_of_reference_absent",
        "xrf_pixel_spacing_absent",
        "xrf_shutter_shape_absent",
        "xrf_collimator_shape_absent",
        "xrf_detector_type_absent",
    ] {
        assert!(
            internal_names.contains(&name),
            "missing internal validation {name}"
        );
    }

    let object = open_file(first_root.join(RELATIVE_PATH)).expect("XRF fixture must parse");
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
        uids::X_RAY_RADIOFLUOROSCOPIC_IMAGE_STORAGE
    );
    for (tag, vr, expected) in [
        (tags::MODALITY, VR::CS, "RF"),
        (tags::IMAGE_TYPE, VR::CS, "ORIGINAL\\PRIMARY\\SINGLE PLANE"),
        (tags::BODY_PART_EXAMINED, VR::CS, "ABDOMEN"),
        (tags::PATIENT_ORIENTATION, VR::CS, ""),
        (tags::PIXEL_INTENSITY_RELATIONSHIP, VR::CS, "LIN"),
        (tags::LOSSY_IMAGE_COMPRESSION, VR::CS, "00"),
        (tags::RADIATION_SETTING, VR::CS, "SC"),
        (tags::KVP, VR::DS, "70"),
        (tags::EXPOSURE, VR::IS, "1"),
        (tags::IMAGER_PIXEL_SPACING, VR::DS, "0.2\\0.2"),
        (tags::DISTANCE_SOURCE_TO_DETECTOR, VR::DS, "1200"),
        (tags::DISTANCE_SOURCE_TO_PATIENT, VR::DS, "800"),
        (
            tags::ESTIMATED_RADIOGRAPHIC_MAGNIFICATION_FACTOR,
            VR::DS,
            "1.5",
        ),
        (tags::COLUMN_ANGULATION, VR::DS, "10"),
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
        tags::EXPOSURE_TIME,
        tags::EXPOSURE_TIME_INU_S,
        tags::X_RAY_TUBE_CURRENT,
        tags::X_RAY_TUBE_CURRENT_INU_A,
        tags::EXPOSURE_INU_AS,
        tags::RADIATION_MODE,
        tags::AVERAGE_PULSE_WIDTH,
        tags::POSITIONER_PRIMARY_ANGLE,
        tags::POSITIONER_SECONDARY_ANGLE,
        tags::POSITIONER_MOTION,
        tags::POSITIONER_PRIMARY_ANGLE_INCREMENT,
        tags::POSITIONER_SECONDARY_ANGLE_INCREMENT,
        tags::NUMBER_OF_FRAMES,
        tags::FRAME_INCREMENT_POINTER,
        tags::FRAME_TIME,
        tags::FRAME_TIME_VECTOR,
        tags::REFERENCED_IMAGE_SEQUENCE,
        tags::CONTRAST_BOLUS_AGENT,
        tags::MASK_SUBTRACTION_SEQUENCE,
        tags::TABLE_HEIGHT,
        tags::TABLE_TRAVERSE,
        tags::TABLE_POSITION,
        tags::TABLE_MOTION,
        tags::TABLE_VERTICAL_INCREMENT,
        tags::TABLE_LATERAL_INCREMENT,
        tags::TABLE_LONGITUDINAL_INCREMENT,
        tags::TABLE_ANGLE,
        tags::GANTRY_DETECTOR_TILT,
        tags::SCAN_OPTIONS,
        tags::TOMO_LAYER_HEIGHT,
        tags::TOMO_ANGLE,
        tags::TOMO_TIME,
        tags::TOMO_TYPE,
        tags::TOMO_CLASS,
        Tag(0x0018, 0x1495),
        tags::FRAME_OF_REFERENCE_UID,
        tags::IMAGE_ORIENTATION_PATIENT,
        tags::IMAGE_POSITION_PATIENT,
        tags::PIXEL_SPACING,
        tags::CALIBRATION_IMAGE,
        tags::MODALITY_LUT_SEQUENCE,
        tags::VOILUT_SEQUENCE,
        tags::PRESENTATION_LUT_SHAPE,
        tags::WINDOW_CENTER,
        tags::WINDOW_WIDTH,
        tags::SHUTTER_SHAPE,
        tags::SHUTTER_LEFT_VERTICAL_EDGE,
        tags::SHUTTER_RIGHT_VERTICAL_EDGE,
        tags::SHUTTER_UPPER_HORIZONTAL_EDGE,
        tags::SHUTTER_LOWER_HORIZONTAL_EDGE,
        Tag(0x6000, 0x0010),
        Tag(0x6000, 0x3000),
        tags::COLLIMATOR_SHAPE,
        tags::COLLIMATOR_LEFT_VERTICAL_EDGE,
        tags::COLLIMATOR_RIGHT_VERTICAL_EDGE,
        tags::COLLIMATOR_UPPER_HORIZONTAL_EDGE,
        tags::COLLIMATOR_LOWER_HORIZONTAL_EDGE,
        tags::IMAGE_AND_FLUOROSCOPY_AREA_DOSE_PRODUCT,
        tags::LOSSY_IMAGE_COMPRESSION_RATIO,
        tags::LOSSY_IMAGE_COMPRESSION_METHOD,
        tags::DETECTOR_TYPE,
        tags::DETECTOR_CONFIGURATION,
        tags::DETECTOR_ID,
        tags::DETECTOR_DESCRIPTION,
        tags::DETECTOR_ELEMENT_PHYSICAL_SIZE,
        tags::DETECTOR_ELEMENT_SPACING,
        tags::DETECTOR_ACTIVE_SHAPE,
        tags::DETECTOR_ACTIVE_DIMENSIONS,
        tags::DETECTOR_ACTIVE_ORIGIN,
        tags::FIELD_OF_VIEW_ORIGIN,
        tags::FIELD_OF_VIEW_ROTATION,
        tags::FIELD_OF_VIEW_HORIZONTAL_FLIP,
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
    assert!(
        jsonschema::validator_for(&read_json("schemas/coverage-report.schema.json"))
            .unwrap()
            .is_valid(&report)
    );
    let row = report["coverage_matrix"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["case_id"] == CASE_ID)
        .unwrap();
    for (field, expected) in [
        ("xrf_image_type", json!("ORIGINAL; PRIMARY; SINGLE PLANE")),
        ("xrf_frame_count", json!(1)),
        ("xrf_body_part_examined", json!("ABDOMEN")),
        ("xrf_patient_orientation_empty", json!(true)),
        ("xrf_laterality_present", json!(false)),
        ("xrf_pixel_intensity_relationship", json!("LIN")),
        ("xrf_radiation_setting", json!("SC")),
        ("xrf_kvp", json!(70.0)),
        ("xrf_exposure_mas", json!(1)),
        ("xrf_imager_pixel_spacing_mm", json!("0.2; 0.2")),
        ("xrf_distance_source_to_detector_mm", json!(1200.0)),
        ("xrf_distance_source_to_patient_mm", json!(800.0)),
        (
            "xrf_estimated_radiographic_magnification_factor",
            json!(1.5),
        ),
        ("xrf_column_angulation_degrees", json!(10.0)),
        ("xrf_lossy_image_compression", json!("00")),
        ("xrf_multiframe_cine", json!(false)),
        ("xrf_biplane_data_present", json!(false)),
        ("xrf_contrast_used", json!(false)),
        ("xrf_subtraction_applied", json!(false)),
        ("xrf_table_position_present", json!(false)),
        ("xrf_table_motion_present", json!(false)),
        ("xrf_table_tilt_present", json!(false)),
        ("xrf_tomography_present", json!(false)),
        ("xrf_patient_space_geometry_present", json!(false)),
        ("xrf_pixel_spacing_calibrated", json!(false)),
        ("xrf_xa_positioner_angles_present", json!(false)),
    ] {
        assert_eq!(row[field], expected, "unexpected report field {field}");
    }
    for pointer in [
        "/grouped_coverage/xrf_image_types/ORIGINAL; PRIMARY; SINGLE PLANE",
        "/grouped_coverage/xrf_frame_counts/1",
        "/grouped_coverage/xrf_body_parts_examined/ABDOMEN",
        "/grouped_coverage/xrf_patient_orientation_empty_states/true",
        "/grouped_coverage/xrf_laterality_present_states/false",
        "/grouped_coverage/xrf_pixel_intensity_relationships/LIN",
        "/grouped_coverage/xrf_radiation_settings/SC",
        "/grouped_coverage/xrf_kvps/70.0",
        "/grouped_coverage/xrf_exposures_mas/1",
        "/grouped_coverage/xrf_imager_pixel_spacings_mm/0.2; 0.2",
        "/grouped_coverage/xrf_distances_source_to_detector_mm/1200.0",
        "/grouped_coverage/xrf_distances_source_to_patient_mm/800.0",
        "/grouped_coverage/xrf_estimated_radiographic_magnification_factors/1.5",
        "/grouped_coverage/xrf_column_angulations_degrees/10.0",
        "/grouped_coverage/xrf_lossy_image_compressions/00",
        "/grouped_coverage/xrf_multiframe_cine_states/false",
        "/grouped_coverage/xrf_biplane_data_present_states/false",
        "/grouped_coverage/xrf_contrast_used_states/false",
        "/grouped_coverage/xrf_subtraction_applied_states/false",
        "/grouped_coverage/xrf_table_position_present_states/false",
        "/grouped_coverage/xrf_table_motion_present_states/false",
        "/grouped_coverage/xrf_table_tilt_present_states/false",
        "/grouped_coverage/xrf_tomography_present_states/false",
        "/grouped_coverage/xrf_patient_space_geometry_present_states/false",
        "/grouped_coverage/xrf_pixel_spacing_calibrated_states/false",
        "/grouped_coverage/xrf_xa_positioner_angles_present_states/false",
    ] {
        assert_eq!(report.pointer(pointer), Some(&Value::from(1)), "{pointer}");
    }
    let markdown = synth_dicom_gen::render_coverage_report_markdown(&report);
    assert!(markdown.contains("## X-Ray Radiofluoroscopic Projection Expectations"));
    assert!(markdown.contains(CASE_ID));
    assert!(markdown.contains("ORIGINAL; PRIMARY; SINGLE PLANE"));
    assert!(markdown.contains("Column angulation (degrees)"));
    assert!(markdown.contains("## XRF Column Angulations (degrees)"));
    assert!(markdown.contains("## XRF Table Position Present States"));
}

#[test]
fn validator_rejects_tampered_xrf_projection_contract() {
    let root = unique_temp_dir("xrf-tampered");
    let manifest = generate_core(&root);
    for (pointer, replacement, failure_key) in [
        (
            "/expected_xrf_projection/image_type/2",
            json!("BIPLANE A"),
            "xrf_image_type_manifest_contract",
        ),
        (
            "/expected_xrf_projection/body_part_examined",
            json!("CHEST"),
            "xrf_body_part_examined_manifest_contract",
        ),
        (
            "/expected_xrf_projection/patient_orientation_empty",
            json!(false),
            "xrf_patient_orientation_empty_manifest_contract",
        ),
        (
            "/expected_xrf_projection/laterality_present",
            json!(true),
            "xrf_laterality_present_manifest_contract",
        ),
        (
            "/expected_xrf_projection/pixel_intensity_relationship",
            json!("LOG"),
            "xrf_pixel_intensity_relationship_manifest_contract",
        ),
        (
            "/expected_xrf_projection/radiation_setting",
            json!("GR"),
            "xrf_radiation_setting_manifest_contract",
        ),
        (
            "/expected_xrf_projection/kvp",
            json!(71.0),
            "xrf_kvp_manifest_contract",
        ),
        (
            "/expected_xrf_projection/exposure_mas",
            json!(2),
            "xrf_exposure_manifest_contract",
        ),
        (
            "/expected_xrf_projection/imager_pixel_spacing_mm/0",
            json!(0.3),
            "xrf_imager_pixel_spacing_manifest_contract",
        ),
        (
            "/expected_xrf_projection/distance_source_to_detector_mm",
            json!(1201.0),
            "xrf_distance_source_to_detector_manifest_contract",
        ),
        (
            "/expected_xrf_projection/distance_source_to_patient_mm",
            json!(801.0),
            "xrf_distance_source_to_patient_manifest_contract",
        ),
        (
            "/expected_xrf_projection/estimated_radiographic_magnification_factor",
            json!(1.6),
            "xrf_estimated_magnification_manifest_contract",
        ),
        (
            "/expected_xrf_projection/column_angulation_degrees",
            json!(11.0),
            "xrf_column_angulation_manifest_contract",
        ),
        (
            "/expected_xrf_projection/lossy_image_compression",
            json!("01"),
            "xrf_lossy_image_compression_manifest_contract",
        ),
        (
            "/expected_xrf_projection/frame_count",
            json!(2),
            "xrf_frame_count_manifest_contract",
        ),
        (
            "/expected_xrf_projection/multiframe_cine",
            json!(true),
            "xrf_multiframe_cine_manifest_contract",
        ),
        (
            "/expected_xrf_projection/biplane_data_present",
            json!(true),
            "xrf_biplane_data_present_manifest_contract",
        ),
        (
            "/expected_xrf_projection/contrast_used",
            json!(true),
            "xrf_contrast_used_manifest_contract",
        ),
        (
            "/expected_xrf_projection/subtraction_applied",
            json!(true),
            "xrf_subtraction_applied_manifest_contract",
        ),
        (
            "/expected_xrf_projection/table_position_present",
            json!(true),
            "xrf_table_position_present_manifest_contract",
        ),
        (
            "/expected_xrf_projection/table_motion_present",
            json!(true),
            "xrf_table_motion_present_manifest_contract",
        ),
        (
            "/expected_xrf_projection/table_tilt_present",
            json!(true),
            "xrf_table_tilt_present_manifest_contract",
        ),
        (
            "/expected_xrf_projection/tomography_present",
            json!(true),
            "xrf_tomography_present_manifest_contract",
        ),
        (
            "/expected_xrf_projection/patient_space_geometry_present",
            json!(true),
            "xrf_patient_space_geometry_present_manifest_contract",
        ),
        (
            "/expected_xrf_projection/pixel_spacing_calibrated",
            json!(true),
            "xrf_pixel_spacing_calibrated_manifest_contract",
        ),
        (
            "/expected_xrf_projection/xa_positioner_angles_present",
            json!(true),
            "xrf_xa_positioner_angles_present_manifest_contract",
        ),
        (
            "/recipe/recipe_parameters/payload_sha256",
            json!("ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"),
            "xrf_payload_hash_manifest_contract",
        ),
        (
            "/recipe/recipe_parameters/payload_sha256",
            json!("ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"),
            "xrf_payload_hash",
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
        .expect("XRF manifest entry must exist")
}

fn case_file_mut(manifest: &mut Value) -> &mut Value {
    manifest["files"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|file| file["case_id"] == CASE_ID)
        .expect("XRF manifest entry must exist")
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
