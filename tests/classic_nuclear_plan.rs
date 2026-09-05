use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::Value;
use synth_dicom_gen::codecs::{FrameEncodeInput, FrameEncoder, NativeRleLosslessEncoder};
use synth_dicom_gen::composition::{
    ContentMaterialization, DicomVr, Part10Materializer, TemplateCatalog,
};
use synth_dicom_gen::encapsulation::{BasicOffsetTablePolicy, EncapsulatedPixelData};
use synth_dicom_gen::recipes::classic_nuclear::{
    ClassicNuclearArtifactParameters, ClassicNuclearPlanError, plan_nuclear_recipe,
};
use synth_dicom_gen::recipes::{
    CLASSIC_PIXEL_SLOT, ClassicResolvedPlanInput, OrderedSeriesProvider, RecipeCatalog,
    resolved_classic_instance_plan,
};
use synth_dicom_gen::{GenerateOptions, prepare_generation_run, sha256_hex, write_generation_run};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

fn temp_path(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "dicom-test-suite-classic-nuclear-{label}-{}-{}",
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

fn owned<'a>(catalog: &'a RecipeCatalog) -> Vec<&'a synth_dicom_gen::recipes::CaseRecipe> {
    let mut recipes = catalog
        .recipes()
        .values()
        .filter(|recipe| {
            recipe.dicom.as_ref().is_some_and(|dicom| {
                dicom.artifacts.iter().any(|artifact| {
                    artifact.algorithm_provider_id.as_deref() == Some("algorithm.classic_nuclear")
                })
            })
        })
        .collect::<Vec<_>>();
    recipes.sort_by_key(|recipe| recipe.planning_order);
    recipes
}

fn make_rle_content(
    plan: &mut synth_dicom_gen::composition::ResolvedInstancePlan,
    native: &[u8],
    rows: u32,
    columns: u32,
    bits_allocated: u16,
) {
    let encoded = NativeRleLosslessEncoder::new()
        .encode_frame(FrameEncodeInput {
            native_frame: native,
            rows: u16::try_from(rows).unwrap(),
            columns: u16::try_from(columns).unwrap(),
            samples_per_pixel: 1,
            bits_allocated,
            bits_stored: bits_allocated,
            photometric_interpretation: "MONOCHROME2",
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
fn nuclear_catalog_owns_exact_historical_slice() {
    let (catalog, _, _) = load();
    let recipes = owned(&catalog);
    assert_eq!(
        recipes
            .iter()
            .map(|recipe| recipe.binding.case_id.as_str())
            .collect::<Vec<_>>(),
        [
            "classic/nm/multiframe_explicit_le",
            "classic/pet/rescaled_activity_explicit_le",
            "classic/us/multiframe_explicit_le",
            "classic/us/mono2_u8_explicit_le",
            "classic/us/mono2_u8_rle_lossless",
        ]
    );
    assert_eq!(
        recipes
            .iter()
            .map(|recipe| recipe.planning_order.unwrap())
            .collect::<Vec<_>>(),
        (400..=404).collect::<Vec<_>>()
    );
    for recipe in recipes {
        assert_eq!(recipe.plan_provider_id, "native.classic_plan");
        let artifacts = &recipe.dicom.as_ref().unwrap().artifacts;
        assert_eq!(artifacts.len(), 1);
        let artifact = &artifacts[0];
        assert_eq!(artifact.order, 0);
        assert_eq!(artifact.content.provider_id, "content.native_pixels");
        assert!(artifact.output.path.is_some());
        assert_eq!(
            artifact.algorithm_provider_id.as_deref(),
            Some("algorithm.classic_nuclear")
        );
        assert!(artifact.attribute_operations.is_empty());
    }
}

#[test]
fn nuclear_planning_is_output_free_strict_and_excludes_enhanced_pet() {
    let (catalog, _, lock_hash) = load();
    let recipe = owned(&catalog)[0];
    let absent = temp_path("no-output");
    assert!(!absent.exists());
    plan_nuclear_recipe(recipe, &lock_hash, 7).unwrap().unwrap();
    assert!(!absent.exists());

    let mut unknown = recipe.clone();
    unknown
        .provider_parameters
        .insert("untyped_escape_hatch".into(), Value::Bool(true));
    assert!(matches!(
        plan_nuclear_recipe(&unknown, &lock_hash, 7),
        Err(ClassicNuclearPlanError::Parameters { .. })
    ));

    let mut corrupt = recipe.clone();
    corrupt.dicom.as_mut().unwrap().artifacts[0]
        .parameters
        .insert("counts_accumulated".into(), Value::from(903));
    assert!(matches!(
        plan_nuclear_recipe(&corrupt, &lock_hash, 7),
        Err(ClassicNuclearPlanError::Contract(_))
    ));

    let enhanced = catalog
        .recipes()
        .values()
        .find(|candidate| candidate.binding.case_id.starts_with("enhanced/pet/"))
        .unwrap();
    assert!(
        plan_nuclear_recipe(enhanced, &lock_hash, 7)
            .unwrap()
            .is_none()
    );
}

#[test]
fn direct_nuclear_plans_match_current_bytes_and_manifest_facts() {
    let generated_root = temp_path("legacy");
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
        let parameters: ClassicNuclearArtifactParameters =
            serde_json::from_value(Value::Object(artifact.parameters.clone())).unwrap();
        let requests = plan_nuclear_recipe(recipe, &lock_hash, 7).unwrap().unwrap();
        assert!(
            requests
                .iter()
                .all(|request| request.pixels.slot == CLASSIC_PIXEL_SLOT)
        );
        let planned = OrderedSeriesProvider.plan(requests).unwrap().remove(0);
        let native = planned.pixels.content.unpadded_bytes.clone();
        let shape = planned.pixels.content.plan.shape.clone();
        let reference = artifact.template.as_ref().unwrap();
        let template = templates
            .resolve_qualified(
                &synth_dicom_gen::composition::TemplateId(reference.template_id.clone()),
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
            == synth_dicom_gen::codecs::RLE_LOSSLESS_TRANSFER_SYNTAX_UID
        {
            make_rle_content(
                &mut resolved,
                &native,
                shape.rows,
                shape.columns,
                shape.bits_allocated,
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
            .find(|entry| entry["case_id"] == recipe.binding.case_id)
            .unwrap();
        assert_eq!(entry["path"].as_str(), artifact.output.path.as_deref());
        assert_eq!(entry["recipe"]["recipe_id"], recipe.recipe_id);
        assert_eq!(
            entry["dicom"]["transfer_syntax_uid"],
            artifact.encoding.transfer_syntax_uid
        );
        assert_eq!(
            entry["known_stressors"],
            serde_json::to_value(&artifact.stressors).unwrap()
        );
        let pixels = match parameters {
            ClassicNuclearArtifactParameters::UltrasoundSingleFrame { pixels, .. }
            | ClassicNuclearArtifactParameters::UltrasoundMultiframe { pixels, .. }
            | ClassicNuclearArtifactParameters::NuclearMedicine { pixels, .. }
            | ClassicNuclearArtifactParameters::Pet { pixels, .. } => pixels,
        };
        assert_eq!(entry["image"]["rows"], pixels.rows);
        assert_eq!(entry["image"]["columns"], pixels.columns);
        assert_eq!(entry["image"]["frames"], pixels.frames);
        assert_eq!(
            entry["pixel_data"]["frame_hashes"],
            serde_json::to_value(pixels.frame_sha256).unwrap()
        );
    }
}
