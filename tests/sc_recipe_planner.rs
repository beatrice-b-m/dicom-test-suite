use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use dicom_core::value::Value as DicomValue;
use dicom_dictionary_std::tags;
use dicom_object::open_file;
use dicom_test_suite::codecs::{FrameEncodeInput, FrameEncoder, NativeRleLosslessEncoder};
use dicom_test_suite::composition::{
    CompositionUidRole, ContentMaterialization, Part10Materializer, TemplateCatalog,
};
use dicom_test_suite::recipes::{
    RecipeCatalog, SecondaryCapturePlanInput, native_pixel_content_from_recipe,
    resolved_secondary_capture_plan,
};
use dicom_test_suite::{GenerateOptions, prepare_generation_run, sha256_hex, write_generation_run};
use serde_json::Value;

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

fn temp_dir(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "dicom-test-suite-sc-planner-{label}-{}-{}",
        std::process::id(),
        TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
    ))
}

fn load() -> (RecipeCatalog, TemplateCatalog, String) {
    let recipes = RecipeCatalog::load(
        "cases/recipes",
        "cases/registry.json",
        "templates/catalog.json",
    )
    .unwrap();
    let templates = TemplateCatalog::load("templates/catalog.json").unwrap();
    let lock_hash = sha256_hex(&fs::read("standards.lock.json").unwrap());
    (recipes, templates, lock_hash)
}

fn plan_for<'a>(
    recipes: &'a RecipeCatalog,
    templates: &'a TemplateCatalog,
    lock_hash: &str,
    case_id: &str,
    role: &str,
) -> dicom_test_suite::composition::ResolvedInstancePlan {
    let identity = recipes.binding_for_case(case_id).unwrap();
    let recipe = recipes.recipes().get(identity).unwrap();
    let artifact = recipe
        .dicom
        .as_ref()
        .unwrap()
        .artifacts
        .iter()
        .find(|artifact| artifact.output.role == role)
        .unwrap();
    let reference = artifact.template.as_ref().unwrap();
    let template = templates
        .resolve_qualified(
            &dicom_test_suite::composition::TemplateId(reference.template_id.clone()),
            Some(reference.template_version.parse().unwrap()),
        )
        .unwrap();
    resolved_secondary_capture_plan(SecondaryCapturePlanInput {
        recipe,
        artifact,
        template,
        instance_id: &recipe.recipe_id,
        standards_lock_sha256: lock_hash,
        seed: 7,
    })
    .unwrap()
}

fn generated_path(manifest: &Value, case_id: &str, role_suffix: &str) -> String {
    manifest["files"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| {
            entry["case_id"] == case_id
                && entry["path"]
                    .as_str()
                    .is_some_and(|path| path.ends_with(role_suffix))
        })
        .unwrap()["path"]
        .as_str()
        .unwrap()
        .to_owned()
}

#[test]
fn ordinary_sc_plans_are_byte_identical_to_current_generator_outputs() {
    let generated_root = temp_dir("generated");
    let planned_root = temp_dir("planned");
    let run = prepare_generation_run(GenerateOptions {
        profile: "all".into(),
        out_dir: generated_root.clone(),
        seed: 7,
        include_stress: false,
    })
    .unwrap();
    write_generation_run(&run).unwrap();
    fs::create_dir(&planned_root).unwrap();
    let manifest: Value =
        serde_json::from_slice(&fs::read(generated_root.join("manifest.json")).unwrap()).unwrap();
    let (recipes, templates, lock_hash) = load();

    let fixtures = [
        (
            "classic/sc/mono2_u8_explicit_le",
            "instance",
            "instance.dcm",
        ),
        (
            "classic/sc/rgb_planar0_explicit_le",
            "instance",
            "instance.dcm",
        ),
        (
            "classic/sc/mono2_u16_padding_explicit_le",
            "instance",
            "instance.dcm",
        ),
        (
            "classic/sc/palette_color_u8_explicit_le",
            "instance",
            "instance.dcm",
        ),
        (
            "classic/sc/nonsquare_pixel_spacing",
            "pixel_spacing",
            "pixel-spacing.dcm",
        ),
        (
            "classic/sc/nonsquare_pixel_spacing",
            "pixel_aspect_ratio",
            "pixel-aspect-ratio.dcm",
        ),
        ("classic/sc/mono2_u1_native", "instance", "instance.dcm"),
    ];
    for (index, (case_id, role, suffix)) in fixtures.iter().enumerate() {
        let plan = plan_for(&recipes, &templates, &lock_hash, case_id, role);
        let path = planned_root.join(format!("{index}.dcm"));
        Part10Materializer.materialize(&plan, &path).unwrap();
        let legacy = generated_root.join(generated_path(&manifest, case_id, suffix));
        assert_eq!(
            fs::read(&path).unwrap(),
            fs::read(&legacy).unwrap(),
            "{case_id}/{role}"
        );
    }

    let eot_case = "encapsulation/sc/eot_single_fragment_multiframe";
    let eot_plan = plan_for(&recipes, &templates, &lock_hash, eot_case, "instance");
    let eot_path = generated_root.join(generated_path(&manifest, eot_case, "instance.dcm"));
    let eot = open_file(&eot_path).unwrap();
    assert_eq!(
        eot.meta().implementation_class_uid().trim_end_matches('\0'),
        eot_plan
            .identities
            .get(&CompositionUidRole::ImplementationClass, 0)
            .unwrap()
    );
    assert_eq!(
        eot.meta()
            .media_storage_sop_instance_uid()
            .trim_end_matches('\0'),
        eot_plan
            .identities
            .get(&CompositionUidRole::SopInstance, 0)
            .unwrap()
    );
    for (tag, expected) in [
        (tags::ACQUISITION_DATE_TIME, "20260101000000"),
        (tags::BODY_PART_EXAMINED, "CHEST"),
        (tags::PAGE_NUMBER_VECTOR, "1\\2\\3"),
        (tags::BURNED_IN_ANNOTATION, "NO"),
        (tags::RESCALE_INTERCEPT, "0"),
        (tags::RESCALE_SLOPE, "1"),
        (tags::RESCALE_TYPE, "US"),
        (tags::PRESENTATION_LUT_SHAPE, "IDENTITY"),
    ] {
        assert_eq!(
            eot.element(tag)
                .unwrap()
                .to_str()
                .unwrap()
                .trim_end_matches([' ', '\0']),
            expected
        );
    }
    let eot_identity = recipes.binding_for_case(eot_case).unwrap();
    let eot_recipe = recipes.recipes().get(eot_identity).unwrap();
    let eot_sc = eot_recipe.dicom.as_ref().unwrap().artifacts[0]
        .secondary_capture
        .as_ref()
        .unwrap();
    let neutral = native_pixel_content_from_recipe(eot_sc).unwrap();
    let encoder = NativeRleLosslessEncoder::new();
    let eot_rows = u16::try_from(eot_sc.rows).unwrap();
    let eot_columns = u16::try_from(eot_sc.columns).unwrap();
    let expected_fragments = neutral
        .frames
        .iter()
        .map(|frame| {
            let mut bytes = encoder
                .encode_frame(FrameEncodeInput {
                    native_frame: &frame.decoded_bytes,
                    rows: eot_rows,
                    columns: eot_columns,
                    samples_per_pixel: eot_sc.samples_per_pixel,
                    bits_allocated: eot_sc.bits_allocated,
                    bits_stored: eot_sc.bits_stored,
                    photometric_interpretation: &eot_sc.photometric_interpretation,
                })
                .unwrap()
                .bytes;
            if bytes.len() % 2 != 0 {
                bytes.push(0);
            }
            bytes
        })
        .collect::<Vec<_>>();
    let DicomValue::PixelSequence(sequence) = eot.element(tags::PIXEL_DATA).unwrap().value() else {
        panic!("EOT Pixel Data is not encapsulated");
    };
    assert!(sequence.offset_table().is_empty());
    assert_eq!(sequence.fragments(), expected_fragments);

    fs::remove_dir_all(generated_root).unwrap();
    fs::remove_dir_all(planned_root).unwrap();
}

#[test]
fn eot_plan_locks_pre_encoding_bytes_uids_and_fixed_multiframe_attributes() {
    let (recipes, templates, lock_hash) = load();
    let case_id = "encapsulation/sc/eot_single_fragment_multiframe";
    let plan = plan_for(&recipes, &templates, &lock_hash, case_id, "instance");
    let identity = recipes.binding_for_case(case_id).unwrap();
    let recipe = recipes.recipes().get(identity).unwrap();
    let sc = recipe.dicom.as_ref().unwrap().artifacts[0]
        .secondary_capture
        .as_ref()
        .unwrap();
    let neutral = native_pixel_content_from_recipe(sc).unwrap();
    assert_eq!(
        neutral.unpadded_bytes,
        [0, 85, 170, 255, 17, 17, 17, 17, 255, 170, 85, 0]
    );
    assert_eq!(
        neutral
            .frames
            .iter()
            .map(|frame| frame.decoded_sha256.as_str())
            .collect::<Vec<_>>(),
        sc.frame_sha256
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>()
    );
    let content = &plan.content[0];
    assert!(matches!(
        content.materialization,
        Some(ContentMaterialization::Inline(_))
    ));
    assert_eq!(content.sha256, sha256_hex(&neutral.unpadded_bytes));
    assert!(
        plan.identities
            .get(&CompositionUidRole::ImplementationClass, 0)
            .is_some()
    );

    let string = |tag: &str| {
        let address =
            dicom_test_suite::composition::AttributeAddress::from_normalized_tag(tag).unwrap();
        plan.attributes
            .iter()
            .find(|attribute| attribute.address == address)
            .and_then(|attribute| attribute.value.as_ref())
    };
    for tag in [
        "0008,002A",
        "0018,0015",
        "0018,2001",
        "0028,0009",
        "0028,0301",
        "0028,1052",
        "0028,1053",
        "0028,1054",
        "0028,2110",
        "2050,0020",
    ] {
        assert!(string(tag).is_some(), "missing EOT fixed attribute {tag}");
    }
}
