use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use dicom_core::VR;
use dicom_dictionary_std::{tags, uids};
use dicom_object::open_file;
use serde_json::{Value, json};

const CASE_ID: &str = "classic/us/multiframe_explicit_le";
const RELATIVE_PATH: &str = "classic/us/multiframe_explicit_le/instance.dcm";
const INSTANCE_SHA256: &str = "6f97371d5746d00e10ddadbdf436a29717a7bc241f53993a5fa9bc21ea41206d";
const PAYLOAD_SHA256: &str = "060e2c56c9728f787339515ef16bc8c1adfbfb4fb85b2d2c18f115c17b439bc9";
const FRAME_HASHES: [&str; 4] = [
    "be422fa58b70ec0d940f28a4dba3dadac62d4583b9ecba1e73d65b37ee9733e7",
    "303d53edfa9bf6eeeb81dba8a6a4c1a9c2e1cb0ea773f90afb583d1132d88eee",
    "7f8a6e2fa2665b2465075b9e0cf86dfb0646f6f21a2a647525476e5bb6e489bb",
    "8c213da26d1c57661b68238ac5c1f1d9417f661e0ab578846bf84040e753f650",
];
const FRAMES: [[u8; 16]; 4] = [
    [
        0, 16, 32, 48, 16, 64, 80, 64, 32, 80, 255, 80, 48, 64, 80, 64,
    ],
    [
        0, 16, 32, 48, 16, 64, 80, 64, 32, 80, 80, 255, 48, 64, 80, 80,
    ],
    [
        0, 16, 32, 48, 16, 64, 80, 64, 32, 80, 80, 80, 48, 64, 255, 80,
    ],
    [
        0, 16, 32, 48, 16, 64, 80, 64, 32, 80, 80, 80, 48, 255, 80, 64,
    ],
];

#[test]
fn us_multiframe_vertical_slice_is_exact_byte_stable_and_reported() {
    let first_root = unique_temp_dir("us-first");
    let second_root = unique_temp_dir("us-second");
    let first_manifest = generate_core(&first_root);
    let second_manifest = generate_core(&second_root);
    let first = case_file(&first_manifest);
    let second = case_file(&second_manifest);

    assert_eq!(first["sha256"], INSTANCE_SHA256);
    assert_eq!(second["sha256"], INSTANCE_SHA256);
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
        first["dicom"]["sop_class_uid"],
        uids::ULTRASOUND_MULTI_FRAME_IMAGE_STORAGE
    );
    assert_eq!(
        first["dicom"]["sop_class_name"],
        "Ultrasound Multi-frame Image Storage"
    );
    assert_eq!(first["dicom"]["iod_name"], "Ultrasound Multi-frame Image");
    assert_ne!(first["dicom"]["sop_class_name"], first["dicom"]["iod_name"]);
    assert_eq!(
        first["dicom"]["transfer_syntax_uid"],
        uids::EXPLICIT_VR_LITTLE_ENDIAN
    );
    assert_eq!(first["dicom"]["modality"], "US");
    assert_eq!(
        first["image"],
        json!({
            "rows": 4, "columns": 4, "frames": 4, "samples_per_pixel": 1,
            "photometric_interpretation": "MONOCHROME2", "bits_allocated": 8,
            "bits_stored": 8, "high_bit": 7, "pixel_representation": 0,
            "planar_configuration": null
        })
    );
    assert_eq!(
        first["pixel_data"],
        json!({
            "vr": "OB", "native_or_encapsulated": "native", "value_length": 64,
            "frame_count": 4, "frame_hashes": FRAME_HASHES
        })
    );
    assert_eq!(
        first["expected_us_multiframe"],
        json!({
            "image_type": ["ORIGINAL", "PRIMARY", "ABDOMINAL", "0001"],
            "frame_increment_pointer": "0018,1063",
            "frame_time_ms": 100.0,
            "frame_relative_times_ms": [0.0, 100.0, 200.0, 300.0],
            "frame_count": 4,
            "frames": [
                {"frame_number": 1, "frame_sha256": FRAME_HASHES[0], "pixel_values": FRAMES[0]},
                {"frame_number": 2, "frame_sha256": FRAME_HASHES[1], "pixel_values": FRAMES[1]},
                {"frame_number": 3, "frame_sha256": FRAME_HASHES[2], "pixel_values": FRAMES[2]},
                {"frame_number": 4, "frame_sha256": FRAME_HASHES[3], "pixel_values": FRAMES[3]}
            ],
            "spatially_related_frames": false,
            "color_data_present": false,
            "region_calibrated": false,
            "lossy_image_compression": "00"
        })
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
        "us_multiframe_modality",
        "us_multiframe_body_part_examined",
        "us_multiframe_laterality_absent",
        "us_multiframe_image_type",
        "us_multiframe_lossy_image_compression",
        "us_multiframe_frame_time",
        "us_multiframe_color_data_present",
        "us_multiframe_number_of_frames",
        "us_multiframe_frame_increment_pointer",
        "us_multiframe_frame_time_vector_absent",
        "us_multiframe_frame_of_reference_absent",
        "us_multiframe_region_calibration_absent",
    ] {
        assert!(
            internal_names.contains(&name),
            "missing internal validation {name}"
        );
    }

    let object = open_file(first_root.join(RELATIVE_PATH)).expect("US fixture must parse");
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
        uids::ULTRASOUND_MULTI_FRAME_IMAGE_STORAGE
    );
    assert_eq!(
        object.element(tags::MODALITY).unwrap().to_str().unwrap(),
        "US"
    );
    let body_part_examined = object.element(tags::BODY_PART_EXAMINED).unwrap();
    assert_eq!(body_part_examined.vr(), VR::CS);
    assert_eq!(body_part_examined.to_str().unwrap(), "ABDOMEN");
    assert!(object.element(tags::LATERALITY).is_err());
    assert_eq!(
        object.element(tags::IMAGE_TYPE).unwrap().to_str().unwrap(),
        "ORIGINAL\\PRIMARY\\ABDOMINAL\\0001"
    );
    assert_eq!(object.element(tags::NUMBER_OF_FRAMES).unwrap().vr(), VR::IS);
    assert_eq!(
        object
            .element(tags::NUMBER_OF_FRAMES)
            .unwrap()
            .to_int::<u16>()
            .unwrap(),
        4
    );
    assert_eq!(
        object.element(tags::FRAME_INCREMENT_POINTER).unwrap().vr(),
        VR::AT
    );
    assert_eq!(
        object
            .element(tags::FRAME_INCREMENT_POINTER)
            .unwrap()
            .value()
            .tags()
            .unwrap(),
        &[tags::FRAME_TIME]
    );
    assert_eq!(object.element(tags::FRAME_TIME).unwrap().vr(), VR::DS);
    assert_eq!(
        object
            .element(tags::FRAME_TIME)
            .unwrap()
            .to_float64()
            .unwrap(),
        100.0
    );
    assert_eq!(
        object
            .element(tags::LOSSY_IMAGE_COMPRESSION)
            .unwrap()
            .to_str()
            .unwrap(),
        "00"
    );
    assert_eq!(
        object
            .element(tags::ULTRASOUND_COLOR_DATA_PRESENT)
            .unwrap()
            .to_int::<u16>()
            .unwrap(),
        0
    );
    for tag in [
        tags::FRAME_TIME_VECTOR,
        tags::FRAME_OF_REFERENCE_UID,
        tags::SEQUENCE_OF_ULTRASOUND_REGIONS,
        tags::LOSSY_IMAGE_COMPRESSION_RATIO,
        tags::LOSSY_IMAGE_COMPRESSION_METHOD,
    ] {
        assert!(object.element(tag).is_err(), "unexpected element {tag:?}");
    }
    let pixel = object.element(tags::PIXEL_DATA).unwrap();
    assert_eq!(pixel.vr(), VR::OB);
    let pixel_bytes = pixel.value().to_bytes().unwrap();
    assert_eq!(pixel_bytes.len(), 64);
    assert_eq!(pixel_bytes.as_ref(), FRAMES.concat());
    for (index, expected) in FRAMES.iter().enumerate() {
        assert_eq!(&pixel_bytes[index * 16..(index + 1) * 16], expected);
    }

    let summary = dicom_test_suite::validate_generated_root(&first_root).unwrap();
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
    let joined_hashes = FRAME_HASHES.join("; ");
    for (field, expected) in [
        ("us_image_type", json!("ORIGINAL; PRIMARY; ABDOMINAL; 0001")),
        ("us_frame_increment_pointer", json!("0018,1063")),
        ("us_frame_time_ms", json!(100.0)),
        (
            "us_frame_relative_times_ms",
            json!("0.0; 100.0; 200.0; 300.0"),
        ),
        ("us_frame_count", json!(4)),
        ("us_ordered_frame_hashes", json!(joined_hashes)),
        ("us_spatially_related_frames", json!(false)),
        ("us_color_data_present", json!(false)),
        ("us_region_calibrated", json!(false)),
        ("us_lossy_image_compression", json!("00")),
    ] {
        assert_eq!(row[field], expected, "unexpected report field {field}");
    }
    for pointer in [
        "/grouped_coverage/us_image_types/ORIGINAL; PRIMARY; ABDOMINAL; 0001",
        "/grouped_coverage/us_frame_increment_pointers/0018,1063",
        "/grouped_coverage/us_frame_times_ms/100.0",
        "/grouped_coverage/us_frame_counts/4",
        "/grouped_coverage/us_spatially_related_frames/false",
        "/grouped_coverage/us_color_data_present/false",
        "/grouped_coverage/us_region_calibrated/false",
        "/grouped_coverage/us_lossy_image_compressions/00",
    ] {
        assert_eq!(report.pointer(pointer), Some(&Value::from(1)), "{pointer}");
    }
    let markdown = dicom_test_suite::render_coverage_report_markdown(&report);
    assert!(markdown.contains("## Ultrasound Multi-frame Expectations"));
    assert!(markdown.contains(CASE_ID));
    assert!(markdown.contains("0.0; 100.0; 200.0; 300.0"));
    assert!(markdown.contains(&FRAME_HASHES.join("; ")));
    assert!(markdown.contains("## US Frame Increment Pointers"));
    assert!(markdown.contains("## US Lossy Image Compression History"));
}

#[test]
fn validator_rejects_tampered_us_multiframe_contract() {
    let root = unique_temp_dir("us-tampered");
    let manifest = generate_core(&root);
    for (pointer, replacement, failure_key) in [
        (
            "/expected_us_multiframe/image_type/2",
            json!("CARDIAC"),
            "us_multiframe_image_type_manifest_contract",
        ),
        (
            "/expected_semantics/body_part_examined",
            json!("CHEST"),
            "us_multiframe_body_part_examined_manifest_contract",
        ),
        (
            "/expected_us_multiframe/frame_count",
            json!(3),
            "us_multiframe_frame_count_manifest_contract",
        ),
        (
            "/expected_us_multiframe/frame_increment_pointer",
            json!("0018,1065"),
            "us_multiframe_frame_increment_pointer_manifest_contract",
        ),
        (
            "/expected_us_multiframe/frame_time_ms",
            json!(50.0),
            "us_multiframe_frame_time_manifest_contract",
        ),
        (
            "/expected_us_multiframe/frame_relative_times_ms/2",
            json!(250.0),
            "us_multiframe_relative_times_manifest_contract",
        ),
        (
            "/expected_us_multiframe/lossy_image_compression",
            json!("01"),
            "us_multiframe_lossy_image_compression_manifest_contract",
        ),
        (
            "/expected_us_multiframe/color_data_present",
            json!(true),
            "us_multiframe_color_data_present_manifest_contract",
        ),
        (
            "/expected_us_multiframe/spatially_related_frames",
            json!(true),
            "us_multiframe_spatially_related_frames_manifest_contract",
        ),
        (
            "/expected_us_multiframe/region_calibrated",
            json!(true),
            "us_multiframe_region_calibrated_manifest_contract",
        ),
        (
            "/expected_us_multiframe/frames/0/frame_number",
            json!(2),
            "us_multiframe_frame_number_manifest_contract",
        ),
        (
            "/expected_us_multiframe/frames/0/pixel_values/0",
            json!(1),
            "us_multiframe_pixel_values_manifest_contract",
        ),
        (
            "/expected_us_multiframe/frames/0/frame_sha256",
            json!("ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"),
            "us_multiframe_frame_hash_manifest_contract",
        ),
        (
            "/recipe/recipe_parameters/payload_sha256",
            json!("ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"),
            "us_multiframe_payload_hash_manifest_contract",
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
        let summary = dicom_test_suite::validate_generated_root(&root).unwrap();
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
        .expect("US manifest entry must exist")
}

fn case_file_mut(manifest: &mut Value) -> &mut Value {
    manifest["files"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|file| file["case_id"] == CASE_ID)
        .expect("US manifest entry must exist")
}

fn generate_core(out_dir: &Path) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
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
    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
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
