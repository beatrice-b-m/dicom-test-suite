use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::Value;
use synth_dicom_gen::codecs::{FrameEncodeInput, FrameEncoder, NativeRleLosslessEncoder};
use synth_dicom_gen::composition::{
    ContentMaterialization, DicomVr, Part10Materializer, TemplateCatalog,
};
use synth_dicom_gen::encapsulation::{BasicOffsetTablePolicy, EncapsulatedPixelData};
use synth_dicom_gen::recipes::classic_vl_projection::{
    ClassicVlProjectionPlanError, ProjectionArtifactParameters, VlArtifactParameters,
    plan_vl_projection_recipe,
};
use synth_dicom_gen::recipes::{
    CLASSIC_PIXEL_SLOT, ClassicResolvedPlanInput, OrderedSeriesProvider, RecipeCatalog,
    resolved_classic_instance_plan,
};
use synth_dicom_gen::{GenerateOptions, prepare_generation_run, sha256_hex, write_generation_run};

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

fn owned(catalog: &RecipeCatalog) -> Vec<&synth_dicom_gen::recipes::CaseRecipe> {
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
    for source in owned(&catalog).into_iter().filter(|r| {
        r.binding.case_id.starts_with("classic/xa/")
            || r.binding.case_id.starts_with("classic/xrf/")
    }) {
        for name in ["caller/projection", "vl/photo/caller", "classic/mr/caller"] {
            let mut recipe = source.clone();
            recipe.binding.case_id = name.into();
            recipe.recipe_id = "caller_projection".into();
            recipe.planning_order = Some(900);
            recipe.projection_order = Some(901);
            recipe.dicom.as_mut().unwrap().artifacts[0].output.path =
                Some("independent/projection.dcm".into());
            let plan = plan_vl_projection_recipe(&recipe, &lock_hash, 7)
                .unwrap()
                .unwrap();
            assert_eq!(plan.len(), 1);
            assert_eq!(
                plan[0].output_relative_path.as_str(),
                "independent/projection.dcm"
            );
        }
        let mut crossed = source.clone();
        let template = crossed.dicom.as_mut().unwrap().artifacts[0]
            .template
            .as_mut()
            .unwrap();
        template.template_id = if template.template_id == "classic/xa" {
            "classic/xrf"
        } else {
            "classic/xa"
        }
        .into();
        assert!(plan_vl_projection_recipe(&crossed, &lock_hash, 7).is_err());
        let mut unknown = source.clone();
        unknown
            .provider_parameters
            .insert("unsupported".into(), serde_json::json!(true));
        assert!(plan_vl_projection_recipe(&unknown, &lock_hash, 7).is_err());
        let value = serde_json::to_value(source).unwrap();
        for section in ["provider_parameters", "parameters"] {
            let pointer = if section == "parameters" {
                "/dicom/artifacts/0/parameters"
            } else {
                "/provider_parameters"
            };
            for key in value.pointer(pointer).unwrap().as_object().unwrap().keys() {
                let mut changed = value.clone();
                changed
                    .pointer_mut(pointer)
                    .unwrap()
                    .as_object_mut()
                    .unwrap()
                    .remove(key);
                if let Ok(recipe) = serde_json::from_value(changed) {
                    assert!(
                        plan_vl_projection_recipe(&recipe, &lock_hash, 7).is_err(),
                        "missing {key}"
                    );
                }
            }
        }
        for (pointer, replacement) in [
            (
                "/dicom/artifacts/0/parameters/rows",
                serde_json::json!(4294967295u32),
            ),
            (
                "/dicom/artifacts/0/parameters/stored_values",
                serde_json::json!([256]),
            ),
            (
                "/dicom/artifacts/0/parameters/frame_sha256",
                serde_json::json!("bad"),
            ),
            (
                "/dicom/artifacts/0/parameters/kvp",
                serde_json::json!("NaN"),
            ),
            (
                "/dicom/artifacts/0/parameters/distance_source_to_detector",
                serde_json::json!("1200.0"),
            ),
            (
                "/dicom/artifacts/0/template/template_id",
                serde_json::json!("classic/mr"),
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
            let mut changed = value.clone();
            *changed.pointer_mut(pointer).unwrap() = replacement;
            let recipe = serde_json::from_value(changed).unwrap();
            assert!(
                plan_vl_projection_recipe(&recipe, &lock_hash, 7).is_err(),
                "{pointer}"
            );
        }
        let mut recipe = source.clone();
        recipe.dicom.as_mut().unwrap().artifacts[0]
            .encoding
            .fragments_per_frame = Some(1);
        assert!(plan_vl_projection_recipe(&recipe, &lock_hash, 7).is_err());
    }
    let mut corrupt = owned(&catalog)[0].clone();
    corrupt.dicom.as_mut().unwrap().artifacts[0]
        .parameters
        .insert("untyped_escape_hatch".into(), Value::Bool(true));
    assert!(matches!(
        plan_vl_projection_recipe(&corrupt, &lock_hash, 7),
        Err(ClassicVlProjectionPlanError::Parameters(_))
    ));
    for source in owned(&catalog).into_iter().filter(|r| {
        matches!(
            r.binding.case_id.as_str(),
            "vl/photo/rgb_planar0_explicit_le" | "vl/photo/palette_color_explicit_le"
        )
    }) {
        for name in [
            "caller/photo",
            "classic/xa/monoplane_explicit_le",
            "vl/photo/rgb_icc_profile_explicit_le",
            "vl/photo/rgb_planar0_rle_lossless",
        ] {
            let mut caller = source.clone();
            caller.binding.case_id = name.into();
            caller.recipe_id = "caller_photo".into();
            caller.planning_order = Some(900);
            caller.projection_order = Some(901);
            caller.dicom.as_mut().unwrap().artifacts[0].output.path =
                Some("independent/photo.dcm".into());
            let plan = plan_vl_projection_recipe(&caller, &lock_hash, 7)
                .unwrap()
                .unwrap();
            assert_eq!(
                plan[0].output_relative_path.as_str(),
                "independent/photo.dcm"
            );
            assert_eq!(
                plan[0].pixels,
                plan_vl_projection_recipe(source, &lock_hash, 7)
                    .unwrap()
                    .unwrap()[0]
                    .pixels
            );
        }
        let value = serde_json::to_value(source).unwrap();
        for pointer in ["/provider_parameters", "/dicom/artifacts/0/parameters"] {
            for key in value.pointer(pointer).unwrap().as_object().unwrap().keys() {
                let mut changed = value.clone();
                changed
                    .pointer_mut(pointer)
                    .unwrap()
                    .as_object_mut()
                    .unwrap()
                    .remove(key);
                if let Ok(recipe) = serde_json::from_value(changed) {
                    assert!(
                        plan_vl_projection_recipe(&recipe, &lock_hash, 7).is_err(),
                        "missing {key}"
                    );
                }
            }
        }
        for (pointer, replacement) in [
            (
                "/dicom/artifacts/0/parameters/rows",
                serde_json::json!(4294967295u32),
            ),
            (
                "/dicom/artifacts/0/parameters/stored_values",
                serde_json::json!([256]),
            ),
            (
                "/dicom/artifacts/0/parameters/frame_sha256",
                serde_json::json!("bad"),
            ),
            (
                "/dicom/artifacts/0/parameters/modality",
                serde_json::json!("XA"),
            ),
            (
                "/dicom/artifacts/0/template/template_id",
                serde_json::json!("classic/xa"),
            ),
            (
                "/dicom/artifacts/0/output/path",
                serde_json::json!("../escape.dcm"),
            ),
            (
                "/dicom/artifacts/0/classic_projection/include_implementation_version_name",
                serde_json::json!(false),
            ),
            (
                "/dicom/artifacts/0/encoding/sequence_length_policy",
                serde_json::json!("undefined"),
            ),
            (
                "/dicom/artifacts/0/content/provider_id",
                serde_json::json!("content.sc.pixel_pattern"),
            ),
            (
                "/dicom/artifacts/0/determinism",
                serde_json::json!("semantic_stable"),
            ),
            ("/dicom/artifacts/0/stressors", serde_json::json!([])),
        ] {
            let mut changed = value.clone();
            *changed.pointer_mut(pointer).unwrap() = replacement;
            let recipe = serde_json::from_value(changed).unwrap();
            assert!(
                plan_vl_projection_recipe(&recipe, &lock_hash, 7).is_err(),
                "{pointer}"
            );
        }
        let mut changed = source.clone();
        changed.dicom.as_mut().unwrap().artifacts[0]
            .encoding
            .fragments_per_frame = Some(1);
        assert!(plan_vl_projection_recipe(&changed, &lock_hash, 7).is_err());
        let mut changed = source.clone();
        changed
            .provider_parameters
            .insert("unexpected".into(), serde_json::json!(true));
        assert!(plan_vl_projection_recipe(&changed, &lock_hash, 7).is_err());
        let mut changed = source.clone();
        changed
            .dicom
            .as_mut()
            .unwrap()
            .artifacts
            .push(source.dicom.as_ref().unwrap().artifacts[0].clone());
        assert!(plan_vl_projection_recipe(&changed, &lock_hash, 7).is_err());
    }
    for source in owned(&catalog).into_iter().filter(|r| {
        r.binding.case_id.contains("icc_profile")
            || r.binding.case_id.starts_with("vl/photo/")
                && r.binding.case_id.ends_with("rle_lossless")
    }) {
        let mut renamed = source.clone();
        renamed.binding.case_id = "caller/unsupported-photo".into();
        assert!(plan_vl_projection_recipe(&renamed, &lock_hash, 7).is_err());
        let mut changed = source.clone();
        changed.dicom.as_mut().unwrap().artifacts[0]
            .encoding
            .fragments_per_frame = Some(2);
        assert!(plan_vl_projection_recipe(&changed, &lock_hash, 7).is_err());
        let mut changed = source.clone();
        changed
            .provider_parameters
            .insert("patient_id".into(), serde_json::json!("changed"));
        assert!(plan_vl_projection_recipe(&changed, &lock_hash, 7).is_err());
    }
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
    plan: &mut synth_dicom_gen::composition::ResolvedInstancePlan,
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
                shape.samples_per_pixel,
                match shape.photometric_interpretation {
                    synth_dicom_gen::native_pixel::PhotometricInterpretation::Rgb => "RGB",
                    synth_dicom_gen::native_pixel::PhotometricInterpretation::PaletteColor => {
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
