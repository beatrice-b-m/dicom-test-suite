use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::Value;
use synth_dicom_gen::codecs::{FrameEncodeInput, FrameEncoder, NativeRleLosslessEncoder};
use synth_dicom_gen::composition::{
    CompositionUidRole, ContentMaterialization, DicomVr, Part10Materializer, TemplateCatalog,
};
use synth_dicom_gen::encapsulation::{BasicOffsetTablePolicy, EncapsulatedPixelData};
use synth_dicom_gen::recipes::classic_ct::{
    ClassicCtArtifactParameters, ClassicCtPlanError, plan_ct_recipe,
};
use synth_dicom_gen::recipes::{
    ClassicResolvedPlanInput, OrderedSeriesProvider, RecipeCatalog, resolved_classic_instance_plan,
};
use synth_dicom_gen::{GenerateOptions, prepare_generation_run, sha256_hex, write_generation_run};

const LOCK_HASH_PATH: &str = "standards.lock.json";
const CT_PREFIXES: [&str; 2] = ["classic/ct/", "geometry/ct/"];
static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

fn temp_path(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "dicom-test-suite-classic-ct-{label}-{}-{}",
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
    let lock_hash = sha256_hex(&fs::read(LOCK_HASH_PATH).unwrap());
    (recipes, templates, lock_hash)
}

fn owned<'a>(catalog: &'a RecipeCatalog) -> Vec<&'a synth_dicom_gen::recipes::CaseRecipe> {
    let mut recipes = catalog
        .recipes()
        .values()
        .filter(|recipe| {
            CT_PREFIXES
                .iter()
                .any(|prefix| recipe.binding.case_id.starts_with(prefix))
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
) {
    let encoded = NativeRleLosslessEncoder::new()
        .encode_frame(FrameEncodeInput {
            native_frame: native,
            rows: u16::try_from(rows).unwrap(),
            columns: u16::try_from(columns).unwrap(),
            samples_per_pixel: 1,
            bits_allocated: 16,
            bits_stored: 12,
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

fn manifest_paths(manifest: &Value, case_id: &str) -> Vec<String> {
    manifest["files"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|entry| entry["case_id"].as_str() == Some(case_id))
        .map(|entry| entry["path"].as_str().unwrap().to_owned())
        .collect()
}

fn manifest_entries<'a>(manifest: &'a Value, case_id: &str) -> Vec<&'a Value> {
    manifest["files"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|entry| entry["case_id"].as_str() == Some(case_id))
        .collect()
}

#[test]
fn migrated_ct_catalog_is_complete_explicit_and_historically_ordered() {
    let (catalog, _, _) = load();
    let recipes = owned(&catalog);
    assert_eq!(recipes.len(), 7);
    assert_eq!(
        recipes
            .iter()
            .map(|recipe| recipe.planning_order.unwrap())
            .collect::<Vec<_>>(),
        (200..=206).collect::<Vec<_>>()
    );
    let cardinalities = recipes
        .iter()
        .map(|recipe| recipe.dicom.as_ref().unwrap().artifacts.len())
        .collect::<Vec<_>>();
    assert_eq!(cardinalities, [1, 1, 3, 3, 3, 3, 4]);
    for recipe in recipes {
        assert_eq!(recipe.plan_provider_id, "native.classic_plan");
        for (order, artifact) in recipe.dicom.as_ref().unwrap().artifacts.iter().enumerate() {
            assert_eq!(artifact.order as usize, order);
            assert!(artifact.output.path.is_some());
            assert_ne!(artifact.output.provider_derived, Some(true));
            assert_eq!(artifact.content.provider_id, "content.native_pixels");
            assert_eq!(
                artifact.algorithm_provider_id.as_deref(),
                Some("algorithm.classic_ct")
            );
            assert!(artifact.secondary_capture.is_none());
            assert!(artifact.attribute_operations.is_empty());
        }
    }
}

#[test]
fn planning_is_filesystem_free_and_rejects_corrupt_contracts() {
    let (catalog, _, lock_hash) = load();
    let recipe = owned(&catalog)[2];
    let absent = temp_path("planning-does-not-write");
    assert!(!absent.exists());
    let requests = plan_ct_recipe(recipe, &lock_hash, 7).unwrap().unwrap();
    assert_eq!(requests.len(), 3);
    assert!(!absent.exists());

    let mut unknown = recipe.clone();
    unknown
        .provider_parameters
        .insert("untyped_escape_hatch".into(), Value::Bool(true));
    assert!(matches!(
        plan_ct_recipe(&unknown, &lock_hash, 7),
        Err(ClassicCtPlanError::Parameters { .. })
    ));

    let mut reordered = recipe.clone();
    reordered.dicom.as_mut().unwrap().artifacts.swap(0, 1);
    assert_eq!(
        plan_ct_recipe(&reordered, &lock_hash, 7).unwrap().unwrap(),
        requests,
        "declaration-array order must not change caller-owned artifact order"
    );

    let non_ct = catalog.recipes().values().find(|candidate| {
        !CT_PREFIXES
            .iter()
            .any(|prefix| candidate.binding.case_id.starts_with(prefix))
    });
    assert!(
        plan_ct_recipe(non_ct.unwrap(), &lock_hash, 7)
            .unwrap()
            .is_none()
    );
}

#[test]
fn caller_ct_series_geometry_is_derived_and_fails_closed() {
    let (catalog, _, lock_hash) = load();
    let case = |case_id: &str| {
        catalog
            .recipes()
            .values()
            .find(|recipe| recipe.binding.case_id == case_id)
            .unwrap()
    };
    let conflict = case("geometry/ct/spatial_sort_conflicts_instance_number");

    let rejects = |recipe: &synth_dicom_gen::recipes::CaseRecipe, expected: &str| {
        let error = plan_ct_recipe(recipe, &lock_hash, 7).unwrap_err();
        assert!(error.to_string().contains(expected), "{error}");
    };

    let mut arbitrary_identity = conflict.clone();
    arbitrary_identity.binding.case_id = "caller/geometry/renamed-stack".into();
    arbitrary_identity.recipe_id = "caller_geometry_renamed_stack".into();
    arbitrary_identity.provider_parameters["image_type"] =
        serde_json::json!(["DERIVED", "SECONDARY", "REFORMATTED"]);
    for (index, artifact) in arbitrary_identity
        .dicom
        .as_mut()
        .unwrap()
        .artifacts
        .iter_mut()
        .enumerate()
    {
        artifact.parameters["uid_file_index"] = Value::from(40 + index as u64);
        artifact.parameters["series_index"] = Value::from(17);
        artifact.parameters["series_number"] = Value::from("83");
        artifact.parameters["acquisition_number"] = Value::from("9");
    }
    arbitrary_identity
        .dicom
        .as_mut()
        .unwrap()
        .artifacts
        .swap(0, 2);
    assert_eq!(
        plan_ct_recipe(&arbitrary_identity, &lock_hash, 7)
            .unwrap()
            .unwrap()
            .len(),
        3
    );

    let mut duplicate_uid = conflict.clone();
    duplicate_uid.dicom.as_mut().unwrap().artifacts[1].parameters["uid_file_index"] =
        Value::from(0);
    rejects(&duplicate_uid, "uid_file_index values must be unique");

    let mut invalid_orientation = conflict.clone();
    invalid_orientation.provider_parameters["image_orientation_patient"] =
        serde_json::json!(["1", "0", "0", "1", "0", "0"]);
    rejects(&invalid_orientation, "orthonormal unit vectors");

    let mut invalid_position = conflict.clone();
    invalid_position.dicom.as_mut().unwrap().artifacts[1].parameters["position_along_normal"] =
        Value::from(6.0);
    rejects(
        &invalid_position,
        "contradicts projected Image Position Patient",
    );

    let mut invalid_spacing = conflict.clone();
    invalid_spacing.provider_parameters["spacing_between_slices"] = Value::from("4");
    rejects(&invalid_spacing, "contradicts projected slice intervals");

    let mut invalid_conflict = conflict.clone();
    invalid_conflict.provider_parameters["sorting_conflict_expected"] = Value::from(false);
    rejects(&invalid_conflict, "contradicts derived spatial");

    let mut duplicate_position = conflict.clone();
    duplicate_position.dicom.as_mut().unwrap().artifacts[1].parameters["image_position_patient"] =
        serde_json::json!(["0", "0", "0"]);
    duplicate_position.dicom.as_mut().unwrap().artifacts[1].parameters["position_along_normal"] =
        Value::from(0.0);
    rejects(&duplicate_position, "positions must be unique");

    let mut invalid_pixels = conflict.clone();
    invalid_pixels.dicom.as_mut().unwrap().artifacts[0].parameters["pixels"]["stored_values"] =
        serde_json::json!([-1024, 0, 1024]);
    rejects(&invalid_pixels, "invalid CT native pixel declaration");

    let mut invalid_hash = conflict.clone();
    invalid_hash.dicom.as_mut().unwrap().artifacts[0].parameters["pixels"]["frame_sha256"] =
        Value::from("0".repeat(64));
    rejects(&invalid_hash, "frame_sha256 does not match");

    let mut overlong_ds = conflict.clone();
    overlong_ds.provider_parameters["slice_thickness"] = Value::from("12345678901234567");
    rejects(&overlong_ds, "invalid CT DS slice_thickness");

    let mut invalid_date = conflict.clone();
    invalid_date.provider_parameters["acquisition_date"] = Value::from("20260230");
    rejects(&invalid_date, "invalid CT acquisition_date");

    let mut invalid_time = conflict.clone();
    invalid_time.provider_parameters["acquisition_time"] = Value::from("12.1");
    rejects(&invalid_time, "invalid CT acquisition_time");

    let tilt = case("geometry/ct/gantry_tilt_series");
    let mut invalid_tilt = tilt.clone();
    invalid_tilt.provider_parameters["gantry_detector_tilt"] = Value::from("15");
    rejects(&invalid_tilt, "contradicts the declared slice-origin shear");

    let multi = case("geometry/ct/multiseries_shared_frame_of_reference");
    let mut sparse_series_indices = multi.clone();
    for artifact in &mut sparse_series_indices.dicom.as_mut().unwrap().artifacts {
        let original = artifact.parameters["series_index"].as_u64().unwrap();
        artifact.parameters["series_index"] = Value::from(if original == 0 { 7 } else { 42 });
        artifact.parameters["series_number"] = Value::from(if original == 0 { "81" } else { "19" });
    }
    assert_eq!(
        plan_ct_recipe(&sparse_series_indices, &lock_hash, 7)
            .unwrap()
            .unwrap()
            .len(),
        4
    );

    let mut missing_organization = multi.clone();
    missing_organization
        .provider_parameters
        .remove("series_organization");
    rejects(
        &missing_organization,
        "require an explicit series organization",
    );
}

#[test]
fn direct_ct_plans_are_byte_identical_to_current_generator() {
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
        let artifacts = &recipe.dicom.as_ref().unwrap().artifacts;
        let requests = plan_ct_recipe(recipe, &lock_hash, 7).unwrap().unwrap();
        let planned = OrderedSeriesProvider.plan(requests).unwrap();
        let expected_paths = artifacts
            .iter()
            .map(|artifact| artifact.output.path.clone().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(
            manifest_paths(&manifest, &recipe.binding.case_id),
            expected_paths
        );
        let legacy_entries = manifest_entries(&manifest, &recipe.binding.case_id);

        for ((artifact, planned), entry) in artifacts.iter().zip(planned).zip(legacy_entries) {
            let parameters: ClassicCtArtifactParameters =
                serde_json::from_value(Value::Object(artifact.parameters.clone())).unwrap();
            assert_eq!(entry["recipe"]["recipe_id"], recipe.recipe_id);
            assert_eq!(entry["recipe"]["recipe_version"], recipe.recipe_version);
            assert_eq!(
                entry["dicom"]["transfer_syntax_uid"],
                artifact.encoding.transfer_syntax_uid
            );
            assert_eq!(entry["dicom"]["sop_class_uid"], "1.2.840.10008.5.1.4.1.1.2");
            assert_eq!(
                entry["known_stressors"],
                serde_json::to_value(&artifact.stressors).unwrap()
            );
            assert_eq!(entry["image"]["rows"], parameters.pixels.rows);
            assert_eq!(entry["image"]["columns"], parameters.pixels.columns);
            assert_eq!(
                entry["pixel_data"]["frame_hashes"][0],
                parameters.pixels.frame_sha256
            );
            assert_eq!(
                entry["recipe"]["recipe_parameters"]["acquisition_number"],
                parameters.acquisition_number
            );
            assert_eq!(
                entry["recipe"]["recipe_parameters"]["geometry"]["image_position_patient"],
                parameters.image_position_patient.join("\\")
            );
            let reference = artifact.template.as_ref().unwrap();
            let template = templates
                .resolve_qualified(
                    &synth_dicom_gen::composition::TemplateId(reference.template_id.clone()),
                    Some(reference.template_version.parse().unwrap()),
                )
                .unwrap();
            let native = planned.pixels.content.unpadded_bytes.clone();
            let rows = planned.pixels.content.plan.shape.rows;
            let columns = planned.pixels.content.plan.shape.columns;
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
                make_rle_content(&mut resolved, &native, rows, columns);
            }
            let direct_path = planned_root.join(artifact.output.path.as_ref().unwrap());
            Part10Materializer
                .materialize(&resolved, &direct_path)
                .unwrap();
            let legacy_path = generated_root.join(artifact.output.path.as_ref().unwrap());
            assert_eq!(
                fs::read(&direct_path).unwrap(),
                fs::read(&legacy_path).unwrap(),
                "{} / {} changed bytes",
                recipe.binding.case_id,
                artifact.logical_id
            );
            let legacy = dicom_object::open_file(&legacy_path).unwrap();
            assert_eq!(
                legacy.meta().media_storage_sop_instance_uid(),
                resolved
                    .identities
                    .get(&CompositionUidRole::SopInstance, 0)
                    .unwrap()
            );
        }
    }
}

#[allow(dead_code)]
fn assert_path_is_beneath(root: &Path, path: &Path) {
    assert!(path.starts_with(root));
}
