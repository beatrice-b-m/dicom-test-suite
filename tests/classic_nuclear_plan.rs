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
    // Every historical nuclear family retains its existing plan route.
    for historical in owned(&catalog) {
        assert!(
            plan_nuclear_recipe(historical, &lock_hash, 7)
                .unwrap()
                .is_some()
        );
    }
    let pet = catalog
        .recipes()
        .values()
        .find(|r| r.binding.case_id == "classic/pet/rescaled_activity_explicit_le")
        .unwrap();
    for name in [
        "caller/activity",
        "classic/us/mono2_u8_explicit_le",
        "classic/mr/caller",
    ] {
        let mut input = pet.clone();
        input.binding.case_id = name.into();
        input.recipe_id = "caller_activity".into();
        input.planning_order = Some(900);
        input.projection_order = Some(901);
        input.dicom.as_mut().unwrap().artifacts[0].output.path = Some("independent/pet.dcm".into());
        let plans = plan_nuclear_recipe(&input, &lock_hash, 7).unwrap().unwrap();
        assert_eq!(plans.len(), 1);
        assert_eq!(
            plans[0].output_relative_path.as_str(),
            "independent/pet.dcm"
        );
    }
    let pet_source = serde_json::to_value(pet).unwrap();
    // Every scalar/array source parameter and synthetic provider field is bound.
    for section in ["/provider_parameters", "/dicom/artifacts/0/parameters"] {
        for key in pet_source
            .pointer(section)
            .unwrap()
            .as_object()
            .unwrap()
            .keys()
        {
            let mut changed = pet_source.clone();
            changed
                .pointer_mut(section)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .remove(key);
            if let Ok(recipe) = serde_json::from_value(changed) {
                assert!(
                    plan_nuclear_recipe(&recipe, &lock_hash, 7).is_err(),
                    "missing {section}/{key}"
                );
            }
        }
    }
    for (pointer, value) in [
        (
            "/dicom/artifacts/0/parameters/rescale_slope",
            serde_json::json!("NaN"),
        ),
        (
            "/dicom/artifacts/0/parameters/frame_reference_time_ms",
            serde_json::json!("inf"),
        ),
        (
            "/dicom/artifacts/0/parameters/pixels/stored_values",
            serde_json::json!([0, 100, 200, 65536]),
        ),
        (
            "/dicom/artifacts/0/parameters/pixels/rows",
            serde_json::json!(65535),
        ),
        (
            "/dicom/artifacts/0/parameters/pixels/frame_sha256",
            serde_json::json!(["bad"]),
        ),
        (
            "/dicom/artifacts/0/parameters/pixels/pixel_max",
            serde_json::json!(399),
        ),
        (
            "/dicom/artifacts/0/template/template_id",
            serde_json::json!("classic/mr"),
        ),
        (
            "/dicom/artifacts/0/content/provider_id",
            serde_json::json!("different"),
        ),
        (
            "/dicom/artifacts/0/algorithm_provider_id",
            serde_json::json!("different"),
        ),
        (
            "/dicom/artifacts/0/output/path",
            serde_json::json!("../escape.dcm"),
        ),
        (
            "/dicom/artifacts/0/classic_projection/include_implementation_version_name",
            serde_json::json!(true),
        ),
    ] {
        let mut changed = pet_source.clone();
        *changed.pointer_mut(pointer).unwrap() = value;
        let recipe = serde_json::from_value(changed).unwrap();
        assert!(
            plan_nuclear_recipe(&recipe, &lock_hash, 7).is_err(),
            "{pointer}"
        );
    }
    let mut lexical = pet.clone();
    lexical.dicom.as_mut().unwrap().artifacts[0]
        .parameters
        .insert("rescale_slope".into(), serde_json::json!("2.50"));
    assert!(plan_nuclear_recipe(&lexical, &lock_hash, 7).is_err());
    let mut fragmented = pet.clone();
    fragmented.dicom.as_mut().unwrap().artifacts[0]
        .encoding
        .fragments_per_frame = Some(1);
    assert!(plan_nuclear_recipe(&fragmented, &lock_hash, 7).is_err());
    let us = catalog
        .recipes()
        .values()
        .find(|r| r.binding.case_id == "classic/us/mono2_u8_explicit_le")
        .unwrap();
    for name in [
        "caller/ultrasound",
        "classic/pet/rescaled_activity_explicit_le",
        "classic/mr/caller",
    ] {
        let mut input = us.clone();
        input.binding.case_id = name.into();
        input.recipe_id = "mr_multislice_oblique".into();
        input.planning_order = Some(900);
        input.projection_order = Some(901);
        input.dicom.as_mut().unwrap().artifacts[0].output.path = Some("independent/us.dcm".into());
        let plans = plan_nuclear_recipe(&input, &lock_hash, 7).unwrap().unwrap();
        assert_eq!(plans.len(), 1);
        assert_eq!(plans[0].output_relative_path.as_str(), "independent/us.dcm");
    }
    let source = serde_json::to_value(us).unwrap();
    for (pointer, value) in [
        (
            "/dicom/artifacts/0/parameters/pixels/rows",
            serde_json::json!(0),
        ),
        (
            "/dicom/artifacts/0/parameters/pixels/columns",
            serde_json::json!(65535),
        ),
        (
            "/dicom/artifacts/0/parameters/pixels/frames",
            serde_json::json!(u32::MAX),
        ),
        (
            "/dicom/artifacts/0/parameters/pixels/stored_value_type",
            serde_json::json!("u16"),
        ),
        (
            "/dicom/artifacts/0/parameters/pixels/stored_values",
            serde_json::json!([0, 85, 170, 256]),
        ),
        (
            "/dicom/artifacts/0/parameters/pixels/pixel_min",
            serde_json::json!(1),
        ),
        (
            "/dicom/artifacts/0/parameters/pixels/frame_sha256",
            serde_json::json!(["00".repeat(32)]),
        ),
        (
            "/dicom/artifacts/0/parameters/image_type",
            serde_json::json!(["DERIVED", "PRIMARY"]),
        ),
        (
            "/dicom/artifacts/0/parameters/lossy_image_compression",
            serde_json::json!("01"),
        ),
        (
            "/dicom/artifacts/0/parameters/ultrasound_color_data_present",
            serde_json::json!(1),
        ),
        ("/provider_parameters/modality", serde_json::json!("NM")),
        (
            "/dicom/artifacts/0/template/template_id",
            serde_json::json!("classic/mr"),
        ),
        (
            "/dicom/artifacts/0/algorithm_provider_id",
            serde_json::json!("algorithm.classic_mr_cr"),
        ),
        (
            "/dicom/artifacts/0/classic_projection/include_implementation_version_name",
            serde_json::json!(true),
        ),
        (
            "/dicom/artifacts/0/output/path",
            serde_json::json!("../escape"),
        ),
    ] {
        let mut bad = source.clone();
        *bad.pointer_mut(pointer).unwrap() = value;
        let bad = serde_json::from_value(bad).unwrap();
        assert!(
            plan_nuclear_recipe(&bad, &lock_hash, 7).is_err(),
            "{pointer}"
        );
    }
    for field in ["template", "classic_projection", "algorithm_provider_id"] {
        let mut bad = source.clone();
        bad["dicom"]["artifacts"][0]
            .as_object_mut()
            .unwrap()
            .remove(field);
        let bad = serde_json::from_value(bad).unwrap();
        assert!(
            plan_nuclear_recipe(&bad, &lock_hash, 7).is_err(),
            "missing {field}"
        );
    }
    for field in us.provider_parameters.keys() {
        let mut bad = us.clone();
        bad.provider_parameters
            .insert(field.clone(), serde_json::json!("different"));
        assert!(
            plan_nuclear_recipe(&bad, &lock_hash, 7).is_err(),
            "provider {field}"
        );
    }
    for field in ["series_date", "series_time", "body_part_examined"] {
        let mut bad = source.clone();
        bad["provider_parameters"][field] = serde_json::json!("override");
        assert!(plan_nuclear_recipe(&serde_json::from_value(bad).unwrap(), &lock_hash, 7).is_err());
    }
    for field in ["semantic_labels", "mr", "icc"] {
        let mut bad = us.clone();
        let projection = bad.dicom.as_mut().unwrap().artifacts[0]
            .classic_projection
            .as_mut()
            .unwrap();
        match field {
            "semantic_labels" => {
                projection.semantic_labels =
                    Some(serde_json::from_value(serde_json::json!({})).unwrap())
            }
            "mr" => {
                projection.mr = catalog.recipes().values().find_map(|r| {
                    r.dicom
                        .as_ref()?
                        .artifacts
                        .first()?
                        .classic_projection
                        .as_ref()?
                        .mr
                        .clone()
                })
            }
            _ => {
                projection.icc = catalog.recipes().values().find_map(|r| {
                    r.dicom
                        .as_ref()?
                        .artifacts
                        .first()?
                        .classic_projection
                        .as_ref()?
                        .icc
                        .clone()
                })
            }
        }
        assert!(plan_nuclear_recipe(&bad, &lock_hash, 7).is_err(), "{field}");
    }
    let mut fragment = us.clone();
    fragment.dicom.as_mut().unwrap().artifacts[0]
        .encoding
        .fragments_per_frame = Some(1);
    assert!(plan_nuclear_recipe(&fragment, &lock_hash, 7).is_err());
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
