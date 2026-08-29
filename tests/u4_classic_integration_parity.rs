//! Cross-family U4 contract scaffold.
//!
//! The inventory is deliberately discovered from the registry-backed recipe
//! catalog.  Adding another `algorithm.classic_*` recipe therefore adds it to
//! these checks without changing a count or a case list in this test.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use dicom_test_suite::codecs::{FrameEncodeInput, FrameEncoder, NativeRleLosslessEncoder};
use dicom_test_suite::composition::{
    ContentMaterialization, DicomVr, Part10Materializer, TemplateCatalog,
};
use dicom_test_suite::encapsulation::{BasicOffsetTablePolicy, EncapsulatedPixelData};
use dicom_test_suite::native_pixel::PhotometricInterpretation;
use dicom_test_suite::recipes::classic_ct::plan_ct_recipe;
use dicom_test_suite::recipes::classic_dx_mg::plan_dx_mg_recipe;
use dicom_test_suite::recipes::classic_mr_cr::plan_mr_cr_recipe;
use dicom_test_suite::recipes::classic_nuclear::plan_nuclear_recipe;
use dicom_test_suite::recipes::classic_vl_projection::plan_vl_projection_recipe;
use dicom_test_suite::recipes::{
    CaseRecipe, ClassicInstanceRequest, ClassicResolvedPlanInput, OrderedSeriesProvider,
    RecipeCatalog, resolved_classic_instance_plan,
};
use dicom_test_suite::{GenerateOptions, prepare_generation_run, sha256_hex, write_generation_run};
use serde_json::{Value, json};

const SEED: u64 = 7;
const RLE: &str = "1.2.840.10008.1.2.5";
static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

struct TempRoot(PathBuf);

impl TempRoot {
    fn absent(label: &str) -> Self {
        Self(std::env::temp_dir().join(format!(
            "dicom-test-suite-u4-combined-{label}-{}-{}",
            std::process::id(),
            TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
        )))
    }
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        if self.0.exists() {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
}

fn load() -> (RecipeCatalog, TemplateCatalog, String, Value) {
    (
        RecipeCatalog::load(
            "cases/recipes",
            "cases/registry.json",
            "templates/catalog.json",
        )
        .unwrap(),
        TemplateCatalog::load("templates/catalog.json").unwrap(),
        sha256_hex(&fs::read("standards.lock.json").unwrap()),
        serde_json::from_slice(&fs::read("cases/registry.json").unwrap()).unwrap(),
    )
}

fn classic_recipes<'a>(catalog: &'a RecipeCatalog, registry: &Value) -> Vec<&'a CaseRecipe> {
    let implemented = registry["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|case| case["status"] == "implemented")
        .map(|case| case["case_id"].as_str().unwrap())
        .collect::<BTreeSet<_>>();
    let mut recipes = catalog
        .recipes()
        .values()
        .filter(|recipe| {
            recipe.plan_provider_id == "native.classic_plan"
                && implemented.contains(recipe.binding.case_id.as_str())
                && recipe.dicom.as_ref().is_some_and(|dicom| {
                    !dicom.artifacts.is_empty()
                        && dicom.artifacts.iter().all(|artifact| {
                            artifact
                                .algorithm_provider_id
                                .as_deref()
                                .is_some_and(|id| id.starts_with("algorithm.classic_"))
                        })
                })
        })
        .collect::<Vec<_>>();
    recipes.sort_by_key(|recipe| recipe.planning_order);
    recipes
}

fn dispatch(recipe: &CaseRecipe, lock_hash: &str) -> Result<Vec<ClassicInstanceRequest>, String> {
    let candidates = [
        plan_ct_recipe(recipe, lock_hash, SEED).map_err(|error| error.to_string())?,
        plan_dx_mg_recipe(recipe, lock_hash, SEED).map_err(|error| error.to_string())?,
        plan_mr_cr_recipe(recipe, lock_hash, SEED).map_err(|error| error.to_string())?,
        plan_nuclear_recipe(recipe, lock_hash, SEED).map_err(|error| error.to_string())?,
        plan_vl_projection_recipe(recipe, lock_hash, SEED).map_err(|error| error.to_string())?,
    ];
    let mut owned = candidates.into_iter().flatten();
    let requests = owned
        .next()
        .ok_or_else(|| format!("no U4 planner owns {}", recipe.binding.case_id))?;
    if owned.next().is_some() {
        return Err(format!(
            "more than one U4 planner owns {}",
            recipe.binding.case_id
        ));
    }
    Ok(requests)
}

fn photometric_name(value: PhotometricInterpretation) -> &'static str {
    match value {
        PhotometricInterpretation::Monochrome1 => "MONOCHROME1",
        PhotometricInterpretation::Monochrome2 => "MONOCHROME2",
        PhotometricInterpretation::PaletteColor => "PALETTE COLOR",
        PhotometricInterpretation::Rgb => "RGB",
        PhotometricInterpretation::YbrFull => "YBR_FULL",
        PhotometricInterpretation::YbrFull422 => "YBR_FULL_422",
    }
}

fn install_rle_content(
    plan: &mut dicom_test_suite::composition::ResolvedInstancePlan,
    planned: &dicom_test_suite::recipes::ClassicPlannedInstance,
) {
    let shape = &planned.pixels.content.plan.shape;
    let encoder = NativeRleLosslessEncoder::new();
    let encoded = planned
        .pixels
        .content
        .frames
        .iter()
        .map(|frame| {
            encoder
                .encode_frame(FrameEncodeInput {
                    native_frame: &frame.decoded_bytes,
                    rows: shape.rows.try_into().unwrap(),
                    columns: shape.columns.try_into().unwrap(),
                    samples_per_pixel: shape.samples_per_pixel,
                    bits_allocated: shape.bits_allocated,
                    bits_stored: shape.bits_stored,
                    photometric_interpretation: photometric_name(shape.photometric_interpretation),
                })
                .unwrap()
                .bytes
        })
        .collect::<Vec<_>>();
    let encapsulated =
        EncapsulatedPixelData::one_fragment_per_frame(&encoded, BasicOffsetTablePolicy::Populated)
            .unwrap();
    let payload = encoded.concat();
    let content = &mut plan.content[0];
    content.kind = "encapsulated_pixels".into();
    content.vr = DicomVr::OB;
    content.size_bytes = payload.len().try_into().unwrap();
    content.sha256 = sha256_hex(&payload);
    content.materialization = Some(ContentMaterialization::Encapsulated {
        basic_offset_table: encapsulated.basic_offset_table.offsets,
        fragments: encapsulated.fragment_payloads,
    });
}

fn manifest_files_by_path(manifest: &Value) -> BTreeMap<&str, &Value> {
    manifest["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|file| (file["path"].as_str().unwrap(), file))
        .collect()
}

fn expected_uids(plan: &dicom_test_suite::composition::ResolvedInstancePlan) -> Value {
    let mut object = serde_json::Map::from_iter([
        ("study_instance_uid".into(), Value::Null),
        ("series_instance_uid".into(), Value::Null),
        ("sop_instance_uid".into(), Value::Null),
        ("frame_of_reference_uid".into(), Value::Null),
        ("implementation_class_uid".into(), Value::Null),
    ]);
    for (key, uid) in &plan.identities.identities {
        let role = key
            .strip_suffix("#0")
            .unwrap_or_else(|| panic!("manifest cannot project indexed classic UID {key}"));
        object.insert(role.into(), json!(uid));
    }
    Value::Object(object)
}

#[test]
fn catalog_derived_u4_inventory_has_one_owner_and_planning_is_output_free() {
    let (catalog, _, lock_hash, registry) = load();
    let recipes = classic_recipes(&catalog, &registry);
    assert!(!recipes.is_empty(), "registry-backed U4 inventory is empty");

    let absent = TempRoot::absent("planning-only");
    assert!(!absent.0.exists());
    let mut planning_orders = BTreeSet::new();
    let mut output_paths = BTreeSet::new();
    for recipe in recipes {
        assert!(planning_orders.insert(recipe.planning_order.unwrap()));
        let artifacts = &recipe.dicom.as_ref().unwrap().artifacts;
        let requests = dispatch(recipe, &lock_hash).unwrap();
        assert_eq!(requests.len(), artifacts.len());
        assert!(
            requests
                .windows(2)
                .all(|pair| pair[0].order < pair[1].order)
        );
        for (request, artifact) in requests.iter().zip(artifacts) {
            assert_eq!(request.logical_id, artifact.logical_id);
            assert_eq!(
                request.output_relative_path.as_str(),
                artifact.output.path.as_deref().unwrap()
            );
            assert!(output_paths.insert(request.output_relative_path.as_str().to_owned()));
        }
    }
    assert!(!absent.0.exists(), "planning created an output path");
}

#[test]
fn every_u4_plan_matches_fresh_legacy_part10_and_identity_facts() {
    let legacy = TempRoot::absent("legacy");
    let direct = TempRoot::absent("direct");
    let run = prepare_generation_run(GenerateOptions {
        profile: "all".into(),
        out_dir: legacy.0.clone(),
        seed: SEED,
        include_stress: false,
    })
    .unwrap();
    write_generation_run(&run).unwrap();
    fs::create_dir(&direct.0).unwrap();
    let manifest: Value =
        serde_json::from_slice(&fs::read(legacy.0.join("manifest.json")).unwrap()).unwrap();
    let files = manifest_files_by_path(&manifest);
    let (catalog, templates, lock_hash, registry) = load();

    for recipe in classic_recipes(&catalog, &registry) {
        let artifacts = &recipe.dicom.as_ref().unwrap().artifacts;
        let requests = dispatch(recipe, &lock_hash).unwrap();
        let planned = OrderedSeriesProvider.plan(requests).unwrap();
        for (artifact, planned) in artifacts.iter().zip(planned) {
            let path = artifact.output.path.as_deref().unwrap();
            let template_ref = artifact.template.as_ref().unwrap();
            let template = templates
                .resolve_qualified(
                    &dicom_test_suite::composition::TemplateId(template_ref.template_id.clone()),
                    Some(template_ref.template_version.parse().unwrap()),
                )
                .unwrap();
            let mut resolved = resolved_classic_instance_plan(ClassicResolvedPlanInput {
                planned: planned.clone(),
                template,
                transfer_syntax_uid: &artifact.encoding.transfer_syntax_uid,
                encoding_backend_id: artifact
                    .encoding
                    .non_template_encoding_provider_id
                    .as_deref()
                    .unwrap_or("dicom-rs.part10"),
            })
            .unwrap();
            if artifact.encoding.transfer_syntax_uid == RLE {
                install_rle_content(&mut resolved, &planned);
            }
            let direct_path = direct.0.join(path);
            Part10Materializer
                .materialize(&resolved, &direct_path)
                .unwrap();
            assert_eq!(
                fs::read(&direct_path).unwrap(),
                fs::read(legacy.0.join(path)).unwrap(),
                "Part 10 bytes differ for {path}"
            );
            let entry = files.get(path).unwrap();
            assert_eq!(entry["case_id"], recipe.binding.case_id);
            assert_eq!(entry["recipe"]["recipe_id"], recipe.recipe_id);
            assert_eq!(entry["recipe"]["recipe_version"], recipe.recipe_version);
            assert_eq!(
                entry["uids"],
                expected_uids(&resolved),
                "UIDs differ for {path}"
            );
            assert_eq!(
                entry["references"],
                json!([]),
                "references differ for {path}"
            );
        }
    }
}

#[test]
fn global_classic_output_order_is_derived_from_recipe_and_artifact_ordinals() {
    let generated = TempRoot::absent("global-order");
    let run = prepare_generation_run(GenerateOptions {
        profile: "all".into(),
        out_dir: generated.0.clone(),
        seed: SEED,
        include_stress: false,
    })
    .unwrap();
    write_generation_run(&run).unwrap();
    let manifest: Value =
        serde_json::from_slice(&fs::read(generated.0.join("manifest.json")).unwrap()).unwrap();
    let (catalog, _, _, registry) = load();
    let expected_paths = classic_recipes(&catalog, &registry)
        .into_iter()
        .flat_map(|recipe| {
            let mut artifacts = recipe
                .dicom
                .as_ref()
                .unwrap()
                .artifacts
                .iter()
                .collect::<Vec<_>>();
            artifacts.sort_by_key(|artifact| artifact.order);
            artifacts
                .into_iter()
                .map(|artifact| artifact.output.path.as_deref().unwrap().to_owned())
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let selected = expected_paths
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let actual = manifest["files"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|file| file["path"].as_str())
        .filter(|path| selected.contains(path))
        .map(str::to_owned)
        .collect::<Vec<_>>();
    assert_eq!(
        actual.iter().map(String::as_str).collect::<BTreeSet<_>>(),
        selected,
        "global order omitted or duplicated a catalog-derived classic output"
    );
    // Captured from the private pre-cutover seed-1 `all` oracle. The selected
    // path set remains catalog-derived; this digest locks only its exact global
    // manifest order without introducing a second maintained case list.
    assert_eq!(
        sha256_hex(&serde_json::to_vec(&actual).unwrap()),
        "c627f4cef78b394e9d7909bea616efea4be5d2041c5016b6fd679dfabf223124"
    );
}
