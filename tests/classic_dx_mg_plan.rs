use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use dicom_core::value::Value as DicomValue;
use dicom_dictionary_std::tags;
use dicom_object::open_file;
use serde_json::Value;
use synth_dicom_gen::composition::{AttributeOperation, AttributeValue, PrimitiveValue};
use synth_dicom_gen::recipes::classic_dx_mg::{ClassicDxMgPlanError, plan_dx_mg_recipe};
use synth_dicom_gen::recipes::{OrderedSeriesProvider, RecipeCatalog};
use synth_dicom_gen::{GenerateOptions, prepare_generation_run, write_generation_run};

const SEED: u64 = 7;
const STANDARDS_LOCK_SHA256: &str =
    "823230c5932b81b504434330d118fba286d5ff41d4e2f7766372633f4a49e559";
static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

struct TempRoot(PathBuf);

impl TempRoot {
    fn absent(label: &str) -> Self {
        Self(std::env::temp_dir().join(format!(
            "dicom-test-suite-classic-dx-mg-{label}-{}-{}",
            std::process::id(),
            TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
        )))
    }
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn catalog() -> RecipeCatalog {
    RecipeCatalog::load(
        "cases/recipes",
        "cases/registry.json",
        "templates/catalog.json",
    )
    .unwrap()
}

fn owned_recipes() -> Vec<synth_dicom_gen::recipes::CaseRecipe> {
    let catalog = catalog();
    let mut recipes = catalog
        .recipes()
        .values()
        .filter(|recipe| {
            recipe.binding.case_id.starts_with("classic/dx/")
                || recipe.binding.case_id.starts_with("classic/mg/")
        })
        .cloned()
        .collect::<Vec<_>>();
    recipes.sort_by_key(|recipe| recipe.planning_order);
    recipes
}

fn generated_all(root: &Path) -> Value {
    let run = prepare_generation_run(GenerateOptions {
        profile: "all".into(),
        out_dir: root.into(),
        seed: SEED,
        include_stress: false,
    })
    .unwrap();
    write_generation_run(&run).unwrap();
    serde_json::from_slice(&fs::read(root.join("manifest.json")).unwrap()).unwrap()
}

fn manifest_file<'a>(manifest: &'a Value, case_id: &str) -> &'a Value {
    manifest["files"]
        .as_array()
        .unwrap()
        .iter()
        .find(|file| file["case_id"] == case_id)
        .unwrap()
}

fn operation<'a>(operations: &'a [AttributeOperation], tag: &str) -> &'a AttributeOperation {
    operations
        .iter()
        .find(|operation| operation.address().normalized_tag() == tag)
        .unwrap_or_else(|| panic!("missing planned operation {tag}"))
}

fn string(operation: &AttributeOperation) -> &str {
    match operation {
        AttributeOperation::Set {
            value: AttributeValue::Primitive(PrimitiveValue::String(value)),
            ..
        } => value,
        other => panic!("expected string operation, got {other:?}"),
    }
}

fn strings(operation: &AttributeOperation) -> Vec<&str> {
    match operation {
        AttributeOperation::Set {
            value: AttributeValue::Multi(values),
            ..
        } => values
            .iter()
            .map(|value| match value {
                PrimitiveValue::String(value) => value.as_str(),
                other => panic!("expected string component, got {other:?}"),
            })
            .collect(),
        other => panic!("expected multi-string operation, got {other:?}"),
    }
}

#[test]
fn dx_mg_recipes_plan_in_historical_order_with_exact_direct_facts() {
    assert_eq!(
        synth_dicom_gen::sha256_hex(&fs::read("standards.lock.json").unwrap()),
        STANDARDS_LOCK_SHA256
    );
    let recipes = owned_recipes();
    assert_eq!(
        recipes
            .iter()
            .map(|recipe| recipe.planning_order.unwrap())
            .collect::<Vec<_>>(),
        (300..=305).collect::<Vec<_>>()
    );
    let output = TempRoot::absent("parity");
    let manifest = generated_all(&output.0);
    let historical_order = manifest["files"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|file| {
            let case_id = file["case_id"].as_str()?;
            (case_id.starts_with("classic/dx/") || case_id.starts_with("classic/mg/"))
                .then_some(case_id)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        historical_order,
        recipes
            .iter()
            .map(|recipe| recipe.binding.case_id.as_str())
            .collect::<Vec<_>>()
    );

    for recipe in recipes {
        let request = plan_dx_mg_recipe(&recipe, STANDARDS_LOCK_SHA256, SEED)
            .unwrap()
            .unwrap()
            .pop()
            .unwrap();
        let file = manifest_file(&manifest, &recipe.binding.case_id);
        assert_eq!(request.order, u64::from(recipe.planning_order.unwrap()));
        assert_eq!(request.output_relative_path.as_str(), file["path"]);
        assert_eq!(
            request.common.study.study_instance_uid,
            file["uids"]["study_instance_uid"]
        );
        assert_eq!(
            request.common.series.series_instance_uid,
            file["uids"]["series_instance_uid"]
        );
        assert_eq!(request.sop_instance_uid, file["uids"]["sop_instance_uid"]);
        assert_eq!(
            request.implementation_class_uid,
            file["uids"]["implementation_class_uid"]
        );
        assert_eq!(request.sop_class_uid, file["dicom"]["sop_class_uid"]);
        assert_eq!(request.pixels.pixels.shape.rows, file["image"]["rows"]);
        assert_eq!(
            request.pixels.pixels.shape.columns,
            file["image"]["columns"]
        );
        assert_eq!(
            request.pixels.pixels.expected_frame_sha256[0],
            file["pixel_data"]["frame_hashes"][0]
        );

        let planned = OrderedSeriesProvider
            .plan(vec![request])
            .unwrap()
            .pop()
            .unwrap();
        let operations = planned.operations();
        assert_eq!(string(operation(&operations, "0008,001C")), "YES");
        assert_eq!(
            string(operation(&operations, "0008,0068")),
            file["recipe"]["recipe_parameters"]["presentation_intent_type"]
        );
        assert_eq!(
            strings(operation(&operations, "0018,1164")).join("\\"),
            file["recipe"]["recipe_parameters"]["imager_pixel_spacing"]
                .as_str()
                .unwrap_or("")
        );
        assert_eq!(
            string(operation(&operations, "2050,0020")),
            file["recipe"]["recipe_parameters"]["presentation_lut_shape"]
        );
        if recipe.binding.case_id.starts_with("classic/dx/") {
            assert_eq!(string(operation(&operations, "0018,1600")), "RECTANGULAR");
            assert_eq!(string(operation(&operations, "0018,700A")), "DTS-DX-DET");
        } else {
            assert_eq!(string(operation(&operations, "0018,1508")), "MAMMOGRAPHIC");
            assert_eq!(string(operation(&operations, "0018,700A")), "DTS-MG-DET");
            let presentation =
                file["expected_semantics"]["presentation_intent_type"] == "FOR PRESENTATION";
            assert_eq!(
                planned.pixels.content.plan.shape.photometric_interpretation,
                if presentation {
                    synth_dicom_gen::native_pixel::PhotometricInterpretation::Monochrome1
                } else {
                    synth_dicom_gen::native_pixel::PhotometricInterpretation::Monochrome2
                }
            );
            assert_eq!(
                operations
                    .iter()
                    .any(|operation| { operation.address().normalized_tag() == "0028,1050" }),
                presentation
            );
        }

        let transfer_syntax = file["dicom"]["transfer_syntax_uid"].as_str().unwrap();
        if transfer_syntax != "1.2.840.10008.1.2.5" {
            let object = open_file(output.0.join(file["path"].as_str().unwrap())).unwrap();
            let DicomValue::Primitive(value) = object.element(tags::PIXEL_DATA).unwrap().value()
            else {
                panic!("native DX/MG Pixel Data must be primitive")
            };
            assert_eq!(
                value.to_bytes().as_ref(),
                planned.pixels.content.padded_bytes
            );
        }
    }
}

#[test]
fn dx_mg_planning_is_output_free_and_frontend_neutral() {
    let source = include_str!("../src/recipes/classic_dx_mg.rs");
    for forbidden in [
        "std::fs",
        "crate::generator",
        "open_file",
        "Part10Materializer",
        "OutputTransaction",
        "out_dir",
        "output_root",
    ] {
        assert!(!source.contains(forbidden), "planner contains {forbidden}");
    }
    let sentinel = TempRoot::absent("no-output");
    assert!(!sentinel.0.exists());
    for recipe in owned_recipes() {
        assert!(
            plan_dx_mg_recipe(&recipe, STANDARDS_LOCK_SHA256, SEED)
                .unwrap()
                .is_some()
        );
    }
    assert!(!sentinel.0.exists(), "planning created filesystem output");
}

#[test]
fn dx_mg_parameters_are_strict_and_semantic_corruption_is_rejected() {
    let recipes = owned_recipes();
    let dx = recipes
        .iter()
        .find(|recipe| recipe.binding.case_id.contains("dx/display"))
        .unwrap();
    let presentation = recipes
        .iter()
        .find(|recipe| {
            recipe.binding.case_id.contains("mg/for_presentation")
                && recipe.binding.case_id.ends_with("explicit_le")
        })
        .unwrap();

    let mut unknown = dx.clone();
    unknown.dicom.as_mut().unwrap().artifacts[0]
        .parameters
        .insert("unexpected".into(), Value::Bool(true));
    assert!(matches!(
        plan_dx_mg_recipe(&unknown, STANDARDS_LOCK_SHA256, SEED),
        Err(ClassicDxMgPlanError::Parameters(_))
    ));

    let mut missing_shutter = dx.clone();
    missing_shutter.dicom.as_mut().unwrap().artifacts[0]
        .parameters
        .insert("shutter".into(), Value::Null);
    assert!(matches!(
        plan_dx_mg_recipe(&missing_shutter, STANDARDS_LOCK_SHA256, SEED),
        Err(ClassicDxMgPlanError::Contract(_))
    ));

    let mut wrong_inversion = presentation.clone();
    let parameters = &mut wrong_inversion.dicom.as_mut().unwrap().artifacts[0].parameters;
    parameters.insert(
        "photometric_interpretation".into(),
        Value::String("MONOCHROME2".into()),
    );
    assert!(matches!(
        plan_dx_mg_recipe(&wrong_inversion, STANDARDS_LOCK_SHA256, SEED),
        Err(ClassicDxMgPlanError::Contract(_))
    ));

    let mut crossed_family = presentation.clone();
    crossed_family
        .provider_parameters
        .insert("family".into(), Value::String("dx".into()));
    assert!(matches!(
        plan_dx_mg_recipe(&crossed_family, STANDARDS_LOCK_SHA256, SEED),
        Err(ClassicDxMgPlanError::Contract(_))
    ));

    let mut short_pixels = dx.clone();
    short_pixels.dicom.as_mut().unwrap().artifacts[0]
        .parameters
        .insert("stored_values".into(), serde_json::json!([0, 1, 2]));
    assert!(matches!(
        plan_dx_mg_recipe(&short_pixels, STANDARDS_LOCK_SHA256, SEED),
        Err(ClassicDxMgPlanError::Contract(_))
    ));

    let mut wrong_declared_vr = dx.clone();
    wrong_declared_vr.dicom.as_mut().unwrap().artifacts[0]
        .parameters
        .insert(
            "field_of_view_dimensions_vr".into(),
            Value::String("IS".into()),
        );
    assert!(matches!(
        plan_dx_mg_recipe(&wrong_declared_vr, STANDARDS_LOCK_SHA256, SEED),
        Err(ClassicDxMgPlanError::Contract(_))
    ));

    let catalog = catalog();
    let unrelated = catalog
        .binding_for_case("classic/sc/mono2_u8_explicit_le")
        .and_then(|identity| catalog.recipes().get(identity))
        .cloned();
    // A non-owned recipe is ignored rather than partially interpreted.
    if let Some(unrelated) = unrelated {
        assert!(
            plan_dx_mg_recipe(&unrelated, STANDARDS_LOCK_SHA256, SEED)
                .unwrap()
                .is_none()
        );
    }
}

#[test]
fn dx_mg_recipe_documents_are_resolved_and_schema_strict() {
    let recipes = owned_recipes();
    let mut orders = BTreeMap::new();
    for recipe in recipes {
        assert_eq!(recipe.plan_provider_id, "native.classic_plan");
        assert!(
            orders
                .insert(recipe.planning_order.unwrap(), recipe.binding.case_id)
                .is_none()
        );
        let artifact = &recipe.dicom.as_ref().unwrap().artifacts[0];
        assert_eq!(artifact.content.provider_id, "content.native_pixels");
        assert_eq!(artifact.parameters["field_of_view_dimensions_vr"], "DS");
        assert!(artifact.output.path.is_some());
        assert_ne!(artifact.output.provider_derived, Some(true));
        assert!(
            !artifact
                .algorithm_provider_id
                .as_deref()
                .unwrap()
                .contains("case_provider")
        );
        for value in [
            &artifact.encoding.sequence_length_policy,
            &artifact.encoding.item_length_policy,
            &artifact.encoding.offset_table_policy,
            &artifact.encoding.fragmentation_policy,
        ] {
            assert_ne!(value, "provider");
        }
    }
}
