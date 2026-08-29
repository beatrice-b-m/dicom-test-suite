use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use dicom_test_suite::codecs::{FrameEncodeInput, FrameEncoder, NativeRleLosslessEncoder};
use dicom_test_suite::composition::{
    ContentMaterialization, DicomVr, Part10Materializer, TemplateCatalog,
};
use dicom_test_suite::encapsulation::{BasicOffsetTablePolicy, EncapsulatedPixelData};
use dicom_test_suite::recipes::classic_vl_projection::{
    ClassicVlProjectionPlanError, ProjectionArtifactParameters, VlArtifactParameters,
    plan_vl_projection_recipe,
};
use dicom_test_suite::recipes::{
    CLASSIC_PIXEL_SLOT, ClassicResolvedPlanInput, OrderedSeriesProvider, RecipeCatalog,
    resolved_classic_instance_plan,
};
use dicom_test_suite::{GenerateOptions, prepare_generation_run, sha256_hex, write_generation_run};
use serde_json::Value;

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

fn temp_path(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "dicom-test-suite-classic-vl-projection-{label}-{}-{}",
        std::process::id(),
        TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
    ))
}

fn load() -> (RecipeCatalog, TemplateCatalog, String) {
    (
        RecipeCatalog::load(
            "cases/recipes",
            "cases/registry.json",
            "templates/catalog.json",
        )
        .unwrap(),
        TemplateCatalog::load("templates/catalog.json").unwrap(),
        sha256_hex(&fs::read("standards.lock.json").unwrap()),
    )
}

fn owned(catalog: &RecipeCatalog) -> Vec<&dicom_test_suite::recipes::CaseRecipe> {
    let mut recipes = catalog
        .recipes()
        .values()
        .filter(|recipe| {
            recipe.dicom.as_ref().is_some_and(|dicom| {
                dicom.artifacts.iter().any(|artifact| {
                    artifact.algorithm_provider_id.as_deref()
                        == Some("algorithm.classic_vl_projection")
                })
            })
        })
        .collect::<Vec<_>>();
    recipes.sort_by_key(|recipe| recipe.planning_order);
    recipes
}

#[test]
fn owned_catalog_is_explicit_complete_and_ordered() {
    let (catalog, _, _) = load();
    let recipes = owned(&catalog);
    assert!(!recipes.is_empty());
    assert_eq!(
        recipes
            .iter()
            .map(|recipe| recipe.planning_order.unwrap())
            .collect::<Vec<_>>(),
        (600..=609).collect::<Vec<_>>()
    );
    for recipe in recipes {
        assert_eq!(recipe.plan_provider_id, "native.classic_plan");
        let artifacts = &recipe.dicom.as_ref().unwrap().artifacts;
        assert_eq!(artifacts.len(), 1);
        let artifact = &artifacts[0];
        assert_eq!(artifact.logical_id, "instance");
        assert_eq!(artifact.order, 0);
        assert_eq!(artifact.content.provider_id, "content.native_pixels");
        assert_eq!(
            artifact.algorithm_provider_id.as_deref(),
            Some("algorithm.classic_vl_projection")
        );
        assert_eq!(
            artifact.output.path.as_deref(),
            Some(format!("{}/instance.dcm", recipe.binding.case_id).as_str())
        );
        assert!(artifact.attribute_operations.is_empty());
        assert!(artifact.secondary_capture.is_none());
    }
}

#[test]
fn planning_is_output_free_uses_shared_slot_and_rejects_corruption() {
    let (catalog, _, lock_hash) = load();
    let absent = temp_path("planning-no-output");
    assert!(!absent.exists());
    for recipe in owned(&catalog) {
        let requests = plan_vl_projection_recipe(recipe, &lock_hash, 7)
            .unwrap()
            .unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].pixels.slot, CLASSIC_PIXEL_SLOT);
        assert!(!absent.exists());
    }
    let mut corrupt = owned(&catalog)[0].clone();
    corrupt.dicom.as_mut().unwrap().artifacts[0]
        .parameters
        .insert("untyped_escape_hatch".into(), Value::Bool(true));
    assert!(matches!(
        plan_vl_projection_recipe(&corrupt, &lock_hash, 7),
        Err(ClassicVlProjectionPlanError::Parameters(_))
    ));
    let mut wrong_path = owned(&catalog)[0].clone();
    wrong_path.dicom.as_mut().unwrap().artifacts[0].output.path = Some("wrong.dcm".into());
    assert!(matches!(
        plan_vl_projection_recipe(&wrong_path, &lock_hash, 7),
        Err(ClassicVlProjectionPlanError::Contract(_))
    ));
    let non_owned = catalog
        .recipes()
        .values()
        .find(|recipe| {
            !owned(&catalog)
                .iter()
                .any(|owned| owned.recipe_id == recipe.recipe_id)
        })
        .unwrap();
    assert!(
        plan_vl_projection_recipe(non_owned, &lock_hash, 7)
            .unwrap()
            .is_none()
    );
}

fn make_rle_content(
    plan: &mut dicom_test_suite::composition::ResolvedInstancePlan,
    native: &[u8],
    rows: u32,
    columns: u32,
    samples_per_pixel: u16,
    photometric_interpretation: &str,
) {
    let encoded = NativeRleLosslessEncoder::new()
        .encode_frame(FrameEncodeInput {
            native_frame: native,
            rows: rows.try_into().unwrap(),
            columns: columns.try_into().unwrap(),
            samples_per_pixel,
            bits_allocated: 8,
            bits_stored: 8,
            photometric_interpretation,
        })
        .unwrap()
        .bytes;
    let encapsulated = EncapsulatedPixelData::one_fragment_per_frame(
        std::slice::from_ref(&encoded),
        BasicOffsetTablePolicy::Populated,
    )
    .unwrap();
    let content = &mut plan.content[0];
    content.kind = "encapsulated_pixels".into();
    content.vr = DicomVr::OB;
    content.size_bytes = encoded.len() as u64;
    content.sha256 = sha256_hex(&encoded);
    content.materialization = Some(ContentMaterialization::Encapsulated {
        basic_offset_table: encapsulated.basic_offset_table.offsets,
        fragments: encapsulated.fragment_payloads,
    });
}

#[test]
fn direct_plans_match_current_part10_bytes_and_manifest_facts() {
    let generated_root = temp_path("current");
    let planned_root = temp_path("planned");
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
    let (catalog, templates, lock_hash) = load();
    for recipe in owned(&catalog) {
        let artifact = &recipe.dicom.as_ref().unwrap().artifacts[0];
        let request = plan_vl_projection_recipe(recipe, &lock_hash, 7)
            .unwrap()
            .unwrap()
            .remove(0);
        let planned = OrderedSeriesProvider.plan(vec![request]).unwrap().remove(0);
        assert_eq!(planned.pixels.slot, CLASSIC_PIXEL_SLOT);
        let native = planned.pixels.content.unpadded_bytes.clone();
        let shape = planned.pixels.content.plan.shape.clone();
        let reference = artifact.template.as_ref().unwrap();
        let template = templates
            .resolve_qualified(
                &dicom_test_suite::composition::TemplateId(reference.template_id.clone()),
                Some(reference.template_version.parse().unwrap()),
            )
            .unwrap();
        let mut resolved = resolved_classic_instance_plan(ClassicResolvedPlanInput {
            planned,
            template,
            transfer_syntax_uid: &artifact.encoding.transfer_syntax_uid,
            encoding_backend_id: artifact
                .encoding
                .non_template_encoding_provider_id
                .as_deref()
                .unwrap_or("dicom-rs.part10"),
        })
        .unwrap();
        if artifact.encoding.transfer_syntax_uid
            == dicom_test_suite::codecs::RLE_LOSSLESS_TRANSFER_SYNTAX_UID
        {
            make_rle_content(
                &mut resolved,
                &native,
                shape.rows,
                shape.columns,
                shape.samples_per_pixel,
                match shape.photometric_interpretation {
                    dicom_test_suite::native_pixel::PhotometricInterpretation::Rgb => "RGB",
                    dicom_test_suite::native_pixel::PhotometricInterpretation::PaletteColor => {
                        "PALETTE COLOR"
                    }
                    _ => unreachable!(),
                },
            );
        }
        let direct_path = planned_root.join(artifact.output.path.as_ref().unwrap());
        Part10Materializer
            .materialize(&resolved, &direct_path)
            .unwrap();
        let legacy_path = generated_root.join(artifact.output.path.as_ref().unwrap());
        assert_eq!(
            fs::read(&direct_path).unwrap(),
            fs::read(&legacy_path).unwrap(),
            "{} changed bytes",
            recipe.binding.case_id
        );

        let entry = manifest["files"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["case_id"].as_str() == Some(&recipe.binding.case_id))
            .unwrap();
        assert_eq!(
            entry["path"],
            artifact.output.path.as_ref().unwrap().as_str()
        );
        assert_eq!(entry["recipe"]["recipe_id"], recipe.recipe_id);
        assert_eq!(
            entry["dicom"]["transfer_syntax_uid"],
            artifact.encoding.transfer_syntax_uid
        );
        if recipe.binding.case_id.starts_with("vl/") {
            let parameters: VlArtifactParameters =
                serde_json::from_value(Value::Object(artifact.parameters.clone())).unwrap();
            assert_eq!(entry["image"]["rows"], parameters.rows);
            assert_eq!(
                entry["pixel_data"]["frame_hashes"][0],
                parameters.frame_sha256
            );
        } else {
            let parameters: ProjectionArtifactParameters =
                serde_json::from_value(Value::Object(artifact.parameters.clone())).unwrap();
            assert_eq!(entry["image"]["rows"], parameters.rows);
            assert_eq!(
                entry["pixel_data"]["frame_hashes"][0],
                parameters.frame_sha256
            );
            let projection = if parameters.modality == "XA" {
                &entry["expected_xa_projection"]
            } else {
                &entry["expected_xrf_projection"]
            };
            assert_eq!(
                projection["body_part_examined"],
                parameters.body_part_examined
            );
        }
    }
}

#[test]
fn provider_source_has_no_execution_or_filesystem_dependency() {
    let source = include_str!("../src/recipes/classic_vl_projection.rs");
    for forbidden in [
        "std::fs",
        "PathBuf",
        "crate::generator",
        "Part10Materializer",
        "out_dir",
        "write_all",
    ] {
        assert!(
            !source.contains(forbidden),
            "provider source contains {forbidden}"
        );
    }
}
