use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use dicom_core::ops::{ApplyOp, AttributeAction, AttributeOp};
use dicom_core::{PrimitiveValue, VR};
use dicom_dictionary_std::{tags, uids};
use dicom_object::open_file;
use serde_json::{Value, json};

const CASE_ID: &str = "enhanced/pet/multiframe_explicit_le";
const RELATIVE_PATH: &str = "enhanced/pet/multiframe_explicit_le/instance.dcm";
const INSTANCE_SHA256: &str = "f40d03339b2344d0f415c3be9ed5194b3657dcf68a06680f131f1dfe0607125f";
const FRAME_SHA256: &str = "03ec353fd2407afb09c8d65712ef9aa30f03c8243f6f3f1675dca7ea5f6a4784";
const PIXEL_SHA256: &str = "3a43b45e2f6d4d04fe4fc357dfc0efaa21caa5415ffc5db96fc19428d34a7bb5";

#[test]
fn enhanced_pet_vertical_slice_is_deterministic_schema_valid_and_strictly_validated() {
    let first_root = unique_temp_dir("enhanced-pet-first");
    let second_root = unique_temp_dir("enhanced-pet-second");
    let first_manifest = generate_extended(&first_root);
    let second_manifest = generate_extended(&second_root);
    let first = case_file(&first_manifest);
    let second = case_file(&second_manifest);

    let first_bytes = fs::read(first_root.join(RELATIVE_PATH)).unwrap();
    let second_bytes = fs::read(second_root.join(RELATIVE_PATH)).unwrap();
    assert_eq!(
        first_bytes, second_bytes,
        "seeded PET output must be byte stable"
    );
    assert_eq!(first["sha256"], second["sha256"]);
    assert_eq!(first["sha256"], INSTANCE_SHA256);
    assert_eq!(first["sha256"], synth_dicom_gen::sha256_hex(&first_bytes));
    crate::curated_manifest_contract_support::assert_curated_manifest_schema_valid(&first_manifest);

    assert_eq!(
        first.pointer("/dicom/sop_class_uid"),
        Some(&Value::from(uids::ENHANCED_PET_IMAGE_STORAGE))
    );
    assert_eq!(first.pointer("/dicom/modality"), Some(&Value::from("PT")));
    assert_eq!(first.pointer("/image/frames"), Some(&Value::from(2)));
    assert_eq!(
        first.pointer("/pixel_data/frame_hashes"),
        Some(&json!([FRAME_SHA256, FRAME_SHA256]))
    );
    assert_eq!(
        first.pointer("/expected_enhanced_pet/pixel_data_sha256"),
        Some(&Value::from(PIXEL_SHA256))
    );
    assert_eq!(
        first.pointer("/expected_enhanced_pet/image_type"),
        Some(&json!(["DERIVED", "PRIMARY", "STATIC", "MULTIPLICATION"]))
    );
    assert_eq!(
        first.pointer("/expected_enhanced_pet/view_code"),
        Some(&json!({
            "code_value": "24422004",
            "coding_scheme_designator": "SCT",
            "code_meaning": "Axial"
        }))
    );
    assert_eq!(
        first.pointer(
            "/expected_enhanced_pet/radiopharmaceutical_information/total_dose_present_empty"
        ),
        Some(&Value::Bool(true))
    );
    assert_eq!(
        first.pointer("/expected_enhanced_pet/in_stack_position_numbers"),
        Some(&json!([1, 2]))
    );
    assert_eq!(
        first.pointer("/expected_enhanced_pet/activity_values_bqml_by_frame"),
        Some(&json!([
            [0.0, 250.0, 500.0, 1000.0],
            [0.0, 250.0, 500.0, 1000.0]
        ]))
    );

    let obj = open_file(first_root.join(RELATIVE_PATH)).expect("Enhanced PET must parse");
    assert_eq!(
        obj.meta().transfer_syntax.trim_end_matches('\0'),
        uids::EXPLICIT_VR_LITTLE_ENDIAN
    );
    assert_eq!(
        text(&obj, tags::SOP_CLASS_UID),
        uids::ENHANCED_PET_IMAGE_STORAGE
    );
    assert_eq!(
        text(&obj, tags::IMAGE_TYPE),
        "DERIVED\\PRIMARY\\STATIC\\MULTIPLICATION"
    );
    assert_eq!(text(&obj, tags::NUMBER_OF_FRAMES), "2");
    assert_eq!(text(&obj, tags::DECAY_CORRECTED), "NO");

    let view = sequence(&obj, tags::VIEW_CODE_SEQUENCE);
    assert_eq!(view.len(), 1);
    assert_eq!(item_text(&view[0], tags::CODE_VALUE), "24422004");
    assert_eq!(item_text(&view[0], tags::CODING_SCHEME_DESIGNATOR), "SCT");
    assert_eq!(item_text(&view[0], tags::CODE_MEANING), "Axial");
    assert!(
        view[0]
            .element_opt(tags::VIEW_MODIFIER_CODE_SEQUENCE)
            .unwrap()
            .is_none()
    );
    assert!(
        obj.element_opt(tags::SLICE_PROGRESSION_DIRECTION)
            .unwrap()
            .is_none()
    );

    let isotope = sequence(&obj, tags::RADIOPHARMACEUTICAL_INFORMATION_SEQUENCE);
    assert_eq!(isotope.len(), 1);
    let total_dose = isotope[0].element(tags::RADIONUCLIDE_TOTAL_DOSE).unwrap();
    assert_eq!(total_dose.vr(), VR::DS);
    assert_eq!(total_dose.value().to_bytes().unwrap().len(), 0);
    assert_eq!(
        item_text(&isotope[0], tags::RADIONUCLIDE_HALF_LIFE),
        "6586.2"
    );

    let shared = sequence(&obj, tags::SHARED_FUNCTIONAL_GROUPS_SEQUENCE);
    let frame_type = item_sequence(&shared[0], tags::PET_FRAME_TYPE_SEQUENCE);
    assert_eq!(
        item_text(&frame_type[0], tags::FRAME_TYPE),
        "DERIVED\\PRIMARY\\STATIC\\MULTIPLICATION"
    );
    let rwvm = item_sequence(&shared[0], tags::REAL_WORLD_VALUE_MAPPING_SEQUENCE);
    assert_eq!(item_f64(&rwvm[0], tags::REAL_WORLD_VALUE_SLOPE), 2.5);

    let frames = sequence(&obj, tags::PER_FRAME_FUNCTIONAL_GROUPS_SEQUENCE);
    assert_eq!(frames.len(), 2);
    for (index, frame) in frames.iter().enumerate() {
        let content = item_sequence(frame, tags::FRAME_CONTENT_SEQUENCE);
        assert_eq!(
            item_u32(&content[0], tags::IN_STACK_POSITION_NUMBER),
            (index + 1) as u32
        );
        assert_eq!(
            item_u32(&content[0], tags::DIMENSION_INDEX_VALUES),
            (index + 1) as u32
        );
    }
    let pixels = obj
        .element(tags::PIXEL_DATA)
        .unwrap()
        .value()
        .to_bytes()
        .unwrap();
    assert_eq!(
        pixels
            .chunks_exact(2)
            .map(|sample| u16::from_le_bytes([sample[0], sample[1]]))
            .collect::<Vec<_>>(),
        vec![0, 100, 200, 400, 0, 100, 200, 400]
    );
    assert_eq!(synth_dicom_gen::sha256_hex(pixels.as_ref()), PIXEL_SHA256);

    let summary = synth_dicom_gen::validate_generated_root(&first_root).unwrap();
    assert!(summary.failures.is_empty(), "{:?}", summary.failures);
}

#[test]
fn validator_rejects_tampered_enhanced_pet_manifest_contracts() {
    let root = unique_temp_dir("enhanced-pet-manifest-tamper");
    let manifest = generate_extended(&root);

    for (pointer, replacement, failure_key) in [
        (
            "/expected_enhanced_pet/view_code/code_meaning",
            json!("Coronal"),
            "enhanced_pet_view_code_code_meaning_manifest_contract",
        ),
        (
            "/expected_enhanced_pet/frame_type/3",
            json!("NONE"),
            "enhanced_pet_frame_type_manifest_contract",
        ),
        (
            "/expected_enhanced_pet/radiopharmaceutical_information/total_dose_present_empty",
            json!(false),
            "enhanced_pet_total_dose_manifest_contract",
        ),
        (
            "/expected_enhanced_pet/in_stack_position_numbers/1",
            json!(9),
            "enhanced_pet_in_stack_positions_manifest_contract",
        ),
        (
            "/expected_enhanced_pet/radiopharmaceutical_information/half_life_seconds",
            json!(6000.0),
            "enhanced_pet_half_life_manifest_contract",
        ),
        (
            "/expected_enhanced_pet/corrections/decay",
            json!("YES"),
            "enhanced_pet_decay_corrected_manifest_contract",
        ),
        (
            "/expected_enhanced_pet/real_world_value_mapping/slope",
            json!(3.0),
            "enhanced_pet_rwvm_slope_manifest_contract",
        ),
        (
            "/expected_enhanced_pet/stored_values_by_frame/0/1",
            json!(101),
            "enhanced_pet_stored_values_manifest_contract[0]",
        ),
    ] {
        let mut tampered = manifest.clone();
        *case_file_mut(&mut tampered).pointer_mut(pointer).unwrap() = replacement;
        write_manifest(&root, &tampered);
        if !crate::curated_manifest_contract_support::curated_manifest_schema_is_valid(&tampered) {
            let error = synth_dicom_gen::validate_generated_root(&root)
                .expect_err("schema-invalid tampering must fail before semantic validation");
            assert!(
                error.to_string().contains("manifest schema invalid"),
                "{error}"
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

#[test]
fn validator_rejects_tampered_enhanced_pet_dicom_semantics() {
    let root = unique_temp_dir("enhanced-pet-dicom-tamper");
    let manifest = generate_extended(&root);
    let path = root.join(RELATIVE_PATH);
    let pristine = fs::read(&path).unwrap();

    let mutations = vec![
        (
            AttributeOp::new(
                (tags::VIEW_CODE_SEQUENCE, 0, tags::CODE_MEANING),
                AttributeAction::SetStr("Coronal".into()),
            ),
            "enhanced_pet_view_code_code_meaning",
        ),
        (
            AttributeOp::new(
                (
                    tags::SHARED_FUNCTIONAL_GROUPS_SEQUENCE,
                    0,
                    tags::PET_FRAME_TYPE_SEQUENCE,
                    0,
                    tags::FRAME_TYPE,
                ),
                AttributeAction::SetStr("DERIVED\\PRIMARY\\STATIC\\NONE".into()),
            ),
            "enhanced_pet_frame_type",
        ),
        (
            AttributeOp::new(
                (
                    tags::RADIOPHARMACEUTICAL_INFORMATION_SEQUENCE,
                    0,
                    tags::RADIONUCLIDE_TOTAL_DOSE,
                ),
                AttributeAction::SetStr("1".into()),
            ),
            "enhanced_pet_total_dose_present_empty",
        ),
        (
            AttributeOp::new(
                (
                    tags::PER_FRAME_FUNCTIONAL_GROUPS_SEQUENCE,
                    1,
                    tags::FRAME_CONTENT_SEQUENCE,
                    0,
                    tags::IN_STACK_POSITION_NUMBER,
                ),
                AttributeAction::Set(PrimitiveValue::from(9_u32)),
            ),
            "enhanced_pet_in_stack_position[1]",
        ),
        (
            AttributeOp::new(
                (
                    tags::RADIOPHARMACEUTICAL_INFORMATION_SEQUENCE,
                    0,
                    tags::RADIONUCLIDE_HALF_LIFE,
                ),
                AttributeAction::SetStr("6000".into()),
            ),
            "enhanced_pet_half_life",
        ),
        (
            AttributeOp::new(tags::DECAY_CORRECTED, AttributeAction::SetStr("YES".into())),
            "enhanced_pet_decay_corrected",
        ),
        (
            AttributeOp::new(
                (
                    tags::SHARED_FUNCTIONAL_GROUPS_SEQUENCE,
                    0,
                    tags::REAL_WORLD_VALUE_MAPPING_SEQUENCE,
                    0,
                    tags::REAL_WORLD_VALUE_SLOPE,
                ),
                AttributeAction::Set(PrimitiveValue::from(3.0_f64)),
            ),
            "enhanced_pet_rwvm_slope",
        ),
        (
            AttributeOp::new(
                tags::PIXEL_DATA,
                AttributeAction::Set(PrimitiveValue::U16(
                    vec![0_u16, 101, 200, 400, 0, 100, 200, 400].into(),
                )),
            ),
            "enhanced_pet_pixel_data_sha256",
        ),
    ];

    for (mutation, failure_key) in mutations {
        fs::write(&path, &pristine).unwrap();
        write_manifest(&root, &manifest);
        let mut obj = open_file(&path).unwrap();
        obj.apply(mutation).unwrap();
        obj.write_to_file(&path).unwrap();

        let summary = synth_dicom_gen::validate_generated_root(&root).unwrap();
        assert!(
            summary
                .failures
                .iter()
                .any(|failure| failure.contains(failure_key)),
            "missing {failure_key}: {:?}",
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
        .expect("Enhanced PET manifest entry must exist")
}

fn case_file_mut(manifest: &mut Value) -> &mut Value {
    manifest["files"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|file| file["case_id"] == CASE_ID)
        .expect("Enhanced PET manifest entry must exist")
}

fn generate_extended(out_dir: &Path) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_synth-dicom-gen"))
        .args(["generate", "--profile", "extended", "--out"])
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

fn text(obj: &dicom_object::DefaultDicomObject, tag: dicom_core::Tag) -> String {
    obj.element(tag).unwrap().to_str().unwrap().into_owned()
}

fn sequence<'a>(
    obj: &'a dicom_object::DefaultDicomObject,
    tag: dicom_core::Tag,
) -> &'a [dicom_object::InMemDicomObject] {
    obj.element(tag).unwrap().items().unwrap()
}

fn item_sequence<'a>(
    obj: &'a dicom_object::InMemDicomObject,
    tag: dicom_core::Tag,
) -> &'a [dicom_object::InMemDicomObject] {
    obj.element(tag).unwrap().items().unwrap()
}

fn item_text(obj: &dicom_object::InMemDicomObject, tag: dicom_core::Tag) -> String {
    obj.element(tag).unwrap().to_str().unwrap().into_owned()
}

fn item_u32(obj: &dicom_object::InMemDicomObject, tag: dicom_core::Tag) -> u32 {
    obj.element(tag).unwrap().to_int::<u32>().unwrap()
}

fn item_f64(obj: &dicom_object::InMemDicomObject, tag: dicom_core::Tag) -> f64 {
    obj.element(tag).unwrap().to_float64().unwrap()
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
