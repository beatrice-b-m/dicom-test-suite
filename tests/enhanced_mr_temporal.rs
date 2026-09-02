use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use dicom_core::Tag;
use dicom_dictionary_std::{tags, uids};
use dicom_object::open_file;
use serde_json::{Value, json};
use synth_dicom_gen::sha256_hex;

const CASE_ID: &str = "enhanced/mr/multiframe_temporal_position_explicit_le";
const RELATIVE_PATH: &str = "enhanced/mr/multiframe_temporal_position_explicit_le/instance.dcm";
const TEMPORAL_POSITION_TIME_OFFSET: Tag = Tag(0x0020, 0x930D);
const TEMPORAL_POSITION_SEQUENCE: Tag = Tag(0x0020, 0x9310);

#[test]
fn enhanced_mr_temporal_position_vertical_slice_is_self_consistent() {
    let out_dir = unique_temp_dir("enhanced-mr-temporal");
    generate_extended(&out_dir);

    let manifest: Value = serde_json::from_slice(
        &fs::read(out_dir.join("manifest.json")).expect("manifest must be readable"),
    )
    .expect("manifest must contain JSON");
    crate::curated_manifest_contract_support::assert_curated_manifest_schema_valid(&manifest);

    let file = manifest["files"]
        .as_array()
        .expect("manifest files must be an array")
        .iter()
        .find(|file| file["case_id"].as_str() == Some(CASE_ID))
        .expect("temporal Enhanced MR case must be in the manifest");
    assert_eq!(file["path"], RELATIVE_PATH);
    assert_eq!(
        file["dicom"]["sop_class_uid"],
        uids::ENHANCED_MR_IMAGE_STORAGE
    );
    assert_eq!(file["image"]["frames"], 2);
    assert_eq!(file["pixel_data"]["frame_count"], 2);
    assert_eq!(file["pixel_data"]["value_length"], 16);
    assert_eq!(
        temporal_manifest_contract_errors(file),
        Vec::<String>::new()
    );

    let dcm_path = out_dir.join(RELATIVE_PATH);
    let obj = open_file(&dcm_path).expect("temporal Enhanced MR DICOM must parse");
    assert_eq!(
        text(&obj, tags::SOP_CLASS_UID),
        uids::ENHANCED_MR_IMAGE_STORAGE
    );
    for (tag, pointer) in [
        (tags::STUDY_INSTANCE_UID, "/uids/study_instance_uid"),
        (tags::SERIES_INSTANCE_UID, "/uids/series_instance_uid"),
        (tags::SOP_INSTANCE_UID, "/uids/sop_instance_uid"),
        (tags::FRAME_OF_REFERENCE_UID, "/uids/frame_of_reference_uid"),
    ] {
        assert_eq!(
            text(&obj, tag),
            file.pointer(pointer)
                .and_then(Value::as_str)
                .expect("manifest UID must be a string")
        );
    }

    let dimension_uid = file["uids"]["dimension_organization_uid"]
        .as_str()
        .expect("dimension UID must be a string");
    let organizations = sequence(&obj, tags::DIMENSION_ORGANIZATION_SEQUENCE);
    assert_eq!(organizations.len(), 1);
    assert_eq!(
        item_text(&organizations[0], tags::DIMENSION_ORGANIZATION_UID),
        dimension_uid
    );
    let dimensions = sequence(&obj, tags::DIMENSION_INDEX_SEQUENCE);
    assert_eq!(dimensions.len(), 1);
    assert_eq!(
        item_text(&dimensions[0], tags::DIMENSION_ORGANIZATION_UID),
        dimension_uid
    );
    assert_eq!(
        item_tag(&dimensions[0], tags::DIMENSION_INDEX_POINTER),
        TEMPORAL_POSITION_TIME_OFFSET
    );
    assert_eq!(
        item_tag(&dimensions[0], tags::FUNCTIONAL_GROUP_POINTER),
        TEMPORAL_POSITION_SEQUENCE
    );

    let shared = sequence(&obj, tags::SHARED_FUNCTIONAL_GROUPS_SEQUENCE);
    assert_eq!(shared.len(), 1);
    let frame_type = item_sequence(&shared[0], tags::MR_IMAGE_FRAME_TYPE_SEQUENCE);
    assert_eq!(frame_type.len(), 1);
    assert_eq!(
        item_text(&frame_type[0], tags::COMPLEX_IMAGE_COMPONENT),
        "MAGNITUDE"
    );
    assert_eq!(
        item_text(&frame_type[0], tags::ACQUISITION_CONTRAST),
        "UNKNOWN"
    );
    let anatomy = item_sequence(&shared[0], tags::FRAME_ANATOMY_SEQUENCE);
    let region = item_sequence(&anatomy[0], tags::ANATOMIC_REGION_SEQUENCE);
    assert_eq!(item_text(&region[0], tags::CODING_SCHEME_DESIGNATOR), "SCT");

    for (tag, expected) in [
        (tags::PATIENT_POSITION, ""),
        (tags::CONTENT_QUALIFICATION, "RESEARCH"),
        (tags::APPLICABLE_SAFETY_STANDARD_AGENCY, "IEC"),
        (tags::COMPLEX_IMAGE_COMPONENT, "MAGNITUDE"),
        (tags::ACQUISITION_CONTRAST, "UNKNOWN"),
        (tags::BURNED_IN_ANNOTATION, "NO"),
        (tags::LOSSY_IMAGE_COMPRESSION, "00"),
        (tags::PRESENTATION_LUT_SHAPE, "IDENTITY"),
    ] {
        assert_eq!(text(&obj, tag), expected);
    }

    let timing = item_sequence(&shared[0], tags::MR_TIMING_AND_RELATED_PARAMETERS_SEQUENCE);
    assert_eq!(timing.len(), 1);
    let sar = item_sequence(&timing[0], tags::SPECIFIC_ABSORPTION_RATE_SEQUENCE);
    assert_eq!(sar.len(), 1);
    assert_eq!(
        item_text(&sar[0], tags::SPECIFIC_ABSORPTION_RATE_DEFINITION),
        "IEC_HEAD"
    );
    assert_eq!(item_f64(&sar[0], tags::SPECIFIC_ABSORPTION_RATE_VALUE), 0.1);
    let modes = item_sequence(&timing[0], tags::OPERATING_MODE_SEQUENCE);
    assert_eq!(modes.len(), 3);
    for (mode, (expected_type, expected_mode)) in modes.iter().zip([
        ("STATIC FIELD", "IEC_NORMAL"),
        ("RF", "IEC_NORMAL"),
        ("GRADIENT", "IEC_NORMAL"),
    ]) {
        assert_eq!(item_text(mode, tags::OPERATING_MODE_TYPE), expected_type);
        assert_eq!(item_text(mode, tags::OPERATING_MODE), expected_mode);
    }

    let frames = sequence(&obj, tags::PER_FRAME_FUNCTIONAL_GROUPS_SEQUENCE);
    assert_eq!(frames.len(), 2);
    for (index, frame) in frames.iter().enumerate() {
        let content = item_sequence(frame, tags::FRAME_CONTENT_SEQUENCE);
        assert_eq!(content.len(), 1);
        assert_eq!(
            item_u32(&content[0], tags::DIMENSION_INDEX_VALUES),
            (index + 1) as u32
        );
        assert_eq!(
            item_u32(&content[0], tags::TEMPORAL_POSITION_INDEX),
            (index + 1) as u32
        );
        assert_eq!(
            item_u16(&content[0], tags::FRAME_ACQUISITION_NUMBER),
            (index + 1) as u16
        );

        let temporal = item_sequence(frame, TEMPORAL_POSITION_SEQUENCE);
        assert_eq!(temporal.len(), 1);
        assert_eq!(
            item_f64(&temporal[0], TEMPORAL_POSITION_TIME_OFFSET),
            [0.0, 1.5][index]
        );
        let position = item_sequence(frame, tags::PLANE_POSITION_SEQUENCE);
        assert_eq!(position.len(), 1);
        assert_eq!(
            item_text(&position[0], tags::IMAGE_POSITION_PATIENT),
            "0\\0\\0"
        );
    }

    let pixels = obj
        .element(tags::PIXEL_DATA)
        .expect("Pixel Data must be present")
        .value()
        .to_bytes()
        .expect("native Pixel Data must be bytes");
    let expected_pixels = [
        0, 0, 0x19, 0, 0x32, 0, 0x4b, 0, 0x96, 0, 0xaf, 0, 0xc8, 0, 0xe1, 0,
    ];
    assert_eq!(pixels.as_ref(), expected_pixels);
    assert_eq!(
        pixels
            .chunks_exact(2)
            .map(|value| u16::from_le_bytes([value[0], value[1]]))
            .collect::<Vec<_>>(),
        vec![0, 25, 50, 75, 150, 175, 200, 225]
    );
    for (index, frame_bytes) in pixels.chunks_exact(8).enumerate() {
        assert_eq!(
            sha256_hex(frame_bytes),
            file["pixel_data"]["frame_hashes"][index]
                .as_str()
                .expect("frame hash must be a string")
        );
    }

    let internal = file["validation"]["internal"]
        .as_array()
        .expect("internal validation results must be an array");
    for name in [
        "enhanced_mr_patient_position",
        "enhanced_mr_content_qualification",
        "enhanced_mr_applicable_safety_standard_agency",
        "enhanced_mr_complex_image_component",
        "enhanced_mr_acquisition_contrast",
        "enhanced_mr_burned_in_annotation",
        "enhanced_mr_lossy_image_compression",
        "enhanced_mr_presentation_lut_shape",
        "enhanced_mr_frame_complex_image_component",
        "enhanced_mr_frame_acquisition_contrast",
        "enhanced_mr_specific_absorption_rate_sequence_items",
        "enhanced_mr_specific_absorption_rate_definition",
        "enhanced_mr_specific_absorption_rate_value",
        "enhanced_mr_operating_mode_sequence_items",
        "enhanced_mr_operating_mode_type",
        "enhanced_mr_operating_mode",
        "enhanced_mr_temporal_position_index",
        "enhanced_mr_temporal_position_time_offset",
        "enhanced_mr_dimension_index_values",
    ] {
        assert!(
            internal.iter().any(|result| {
                result["name"].as_str() == Some(name) && result["status"].as_str() == Some("passed")
            }),
            "internal validation result {name} must be recorded as passed"
        );
    }

    let mut wrong_pointer = file.clone();
    wrong_pointer["recipe"]["recipe_parameters"]["dimension_index"]["dimension_index_pointer"] =
        json!("EffectiveEchoTime");
    assert!(
        temporal_manifest_contract_errors(&wrong_pointer)
            .contains(&"dimension_index_pointer".to_string())
    );
    let mut wrong_offset = file.clone();
    wrong_offset["recipe"]["recipe_parameters"]["per_frame_functional_groups"]["temporal_position_time_offset"]
        [1] = json!(2.0);
    assert!(
        temporal_manifest_contract_errors(&wrong_offset)
            .contains(&"temporal_position_time_offset".to_string())
    );

    fs::remove_dir_all(out_dir).expect("temporary output root should be removable");
}

fn temporal_manifest_contract_errors(file: &Value) -> Vec<String> {
    let checks = [
        (
            "/recipe/recipe_parameters/dimension_index/dimension_index_pointer",
            json!("TemporalPositionTimeOffset"),
            "dimension_index_pointer",
        ),
        (
            "/recipe/recipe_parameters/dimension_index/functional_group_pointer",
            json!("TemporalPositionSequence"),
            "functional_group_pointer",
        ),
        (
            "/recipe/recipe_parameters/per_frame_functional_groups/temporal_position_time_offset",
            json!([0.0, 1.5]),
            "temporal_position_time_offset",
        ),
        (
            "/recipe/recipe_parameters/per_frame_functional_groups/image_position_patient",
            json!(["0\\0\\0", "0\\0\\0"]),
            "image_position_patient",
        ),
        (
            "/expected_semantics/dimension_index_values",
            json!([1, 2]),
            "dimension_index_values",
        ),
        (
            "/expected_semantics/temporal_position_time_offset",
            json!([0.0, 1.5]),
            "expected_temporal_position_time_offset",
        ),
    ];
    checks
        .into_iter()
        .filter_map(|(pointer, expected, name)| {
            (file.pointer(pointer) != Some(&expected)).then(|| name.to_string())
        })
        .collect()
}

fn text(obj: &dicom_object::DefaultDicomObject, tag: Tag) -> String {
    obj.element(tag)
        .expect("top-level element must be present")
        .value()
        .to_str()
        .expect("element must be textual")
        .trim_matches('\0')
        .trim()
        .to_string()
}

fn sequence(obj: &dicom_object::DefaultDicomObject, tag: Tag) -> &[dicom_object::InMemDicomObject] {
    obj.element(tag)
        .expect("top-level sequence must be present")
        .items()
        .expect("element must be a sequence")
}

fn item_sequence(
    obj: &dicom_object::InMemDicomObject,
    tag: Tag,
) -> &[dicom_object::InMemDicomObject] {
    obj.element(tag)
        .expect("nested sequence must be present")
        .items()
        .expect("element must be a sequence")
}

fn item_text(obj: &dicom_object::InMemDicomObject, tag: Tag) -> String {
    obj.element(tag)
        .expect("nested element must be present")
        .value()
        .to_str()
        .expect("nested element must be textual")
        .trim_matches('\0')
        .trim()
        .to_string()
}

fn item_tag(obj: &dicom_object::InMemDicomObject, tag: Tag) -> Tag {
    obj.element(tag)
        .expect("nested AT element must be present")
        .value()
        .tags()
        .expect("nested element must be AT")
        .first()
        .copied()
        .expect("AT element must contain one tag")
}

fn item_u16(obj: &dicom_object::InMemDicomObject, tag: Tag) -> u16 {
    obj.element(tag)
        .expect("nested US element must be present")
        .value()
        .to_int::<u16>()
        .expect("nested element must be US")
}

fn item_u32(obj: &dicom_object::InMemDicomObject, tag: Tag) -> u32 {
    obj.element(tag)
        .expect("nested UL element must be present")
        .value()
        .to_int::<u32>()
        .expect("nested element must be UL")
}

fn item_f64(obj: &dicom_object::InMemDicomObject, tag: Tag) -> f64 {
    obj.element(tag)
        .expect("nested FD element must be present")
        .value()
        .to_float64()
        .expect("nested element must be FD")
}

fn generate_extended(out_dir: &Path) {
    let output = Command::new(env!("CARGO_BIN_EXE_synth-dicom-gen"))
        .args([
            "generate",
            "--profile",
            "extended",
            "--out",
            out_dir.to_str().expect("temporary path must be UTF-8"),
            "--seed",
            "7",
        ])
        .output()
        .expect("generate command must run");
    assert!(
        output.status.success(),
        "generate extended must succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn unique_temp_dir(name: &str) -> PathBuf {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock must be after epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "dicom-test-suite-{name}-{}-{nonce}",
        std::process::id()
    ))
}
