use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;
use synth_dicom_gen::composition::{
    AttributeValue, CompositionUidRole, PrimitiveValue, TemplateCatalog, TemplateStatus,
};
use synth_dicom_gen::corpus_plan::PlannedArtifact;
use synth_dicom_gen::curated_plan::{
    CuratedCatalogPaths, CuratedScCorpusPlanProvider, CuratedScPlanRequest, CuratedScSelection,
};
use synth_dicom_gen::recipes::RecipeCatalog;

fn json(path: &str) -> Value {
    serde_json::from_slice(&fs::read(path).unwrap_or_else(|error| panic!("{path}: {error}")))
        .unwrap_or_else(|error| panic!("{path}: {error}"))
}

fn rust_sources(root: &Path, paths: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(root).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            rust_sources(&path, paths);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            paths.push(path);
        }
    }
}

#[test]
fn implemented_registry_recipe_identities_are_complete_and_unique() {
    let registry = json("cases/registry.json");
    let mut identities = BTreeMap::new();
    for case in registry["cases"].as_array().expect("registry cases") {
        if case["status"] != "implemented" {
            continue;
        }
        let case_id = case["case_id"].as_str().expect("implemented case ID");
        let recipe_id = case["recipe_id"].as_str().expect("implemented recipe ID");
        let recipe_version = case["recipe_version"]
            .as_str()
            .expect("implemented recipe version");
        assert!(!recipe_id.is_empty(), "{case_id} has an empty recipe ID");
        assert!(
            !recipe_version.is_empty(),
            "{case_id} has an empty recipe version"
        );
        assert!(
            identities
                .insert((recipe_id, recipe_version), case_id)
                .is_none(),
            "duplicate implemented recipe binding {recipe_id}@{recipe_version}"
        );
        assert!(
            case["standards_evidence"]
                .as_array()
                .is_some_and(|evidence| !evidence.is_empty()),
            "{case_id} has no standards evidence"
        );
    }
    assert!(
        !identities.is_empty(),
        "implemented registry must not be empty"
    );
}

#[test]
fn valid_registry_sop_classes_resolve_to_qualified_template_families() {
    let registry = json("cases/registry.json");
    let inventory = json("templates/inventory.json");
    let catalog = TemplateCatalog::load("templates/catalog.json").unwrap();

    let expected = registry["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|case| {
            case["status"] == "implemented"
                && case["artifact_kind"] == "dicom_instance"
                && !case["profiles"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|profile| profile == "negative" || profile == "fuzz")
        })
        .map(|case| case["sop_class_uid"].as_str().unwrap())
        .collect::<BTreeSet<_>>();
    let mappings = inventory["mappings"].as_array().unwrap();
    let actual = mappings
        .iter()
        .map(|mapping| mapping["sop_class_uid"].as_str().unwrap())
        .collect::<BTreeSet<_>>();
    assert_eq!(actual, expected);

    for mapping in mappings {
        let uid = mapping["sop_class_uid"].as_str().unwrap();
        let family = mapping["template_family"].as_str().unwrap();
        assert!(
            catalog.templates.iter().any(|template| {
                template.status == TemplateStatus::Qualified
                    && template.sop_class_uid == uid
                    && (template.template_id.0 == family
                        || template.template_id.0.starts_with(&format!("{family}/")))
            }),
            "{uid} does not resolve through qualified family {family}"
        );
    }
}

#[test]
fn every_qualified_template_retains_validation_and_evidence_routes() {
    let catalog = TemplateCatalog::load("templates/catalog.json").unwrap();
    for template in catalog
        .templates
        .iter()
        .filter(|template| template.status == TemplateStatus::Qualified)
    {
        assert!(
            template.validation["generic_rule_ids"]
                .as_array()
                .is_some_and(|rules| !rules.is_empty()),
            "{} has no generic validation rule",
            template.template_id
        );
        assert!(
            template.validation["template_rule_ids"]
                .as_array()
                .is_some_and(|rules| !rules.is_empty()),
            "{} has no template validation rule",
            template.template_id
        );
        assert!(
            template.validation["independent_routes"]
                .as_array()
                .is_some_and(|routes| !routes.is_empty()),
            "{} has no independent evidence route",
            template.template_id
        );
        assert!(
            !template.standards_evidence.is_empty(),
            "{} has no standards evidence",
            template.template_id
        );
    }
}

#[test]
fn every_current_production_direct_writer_is_classified_for_removal() {
    let audit = fs::read_to_string("docs/unified-generation-spine-audit.md").unwrap();
    let allowed = BTreeSet::from([
        PathBuf::from("src/composition/materializer.rs"),
        PathBuf::from("src/executor/materialization.rs"),
    ]);
    let mut sources = Vec::new();
    rust_sources(Path::new("src"), &mut sources);
    for path in sources {
        if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.ends_with("_tests.rs"))
        {
            continue;
        }
        let source = fs::read_to_string(&path).unwrap();
        let Some(writer_offset) = source.find(".write_to_file(") else {
            continue;
        };
        if source
            .find("#[cfg(test)]")
            .is_some_and(|test_offset| test_offset < writer_offset)
        {
            continue;
        }
        assert!(
            allowed.contains(&path),
            "unclassified production direct writer in {}",
            path.display()
        );
        assert!(
            audit.contains(path.to_str().unwrap()),
            "{} is not classified in the U0 audit",
            path.display()
        );
    }

    for (path, marker, removal) in [
        ("src/codecs.rs", "dcmcjpeg", "U7.2"),
        ("src/generation_backends/", "external", "U6.7"),
        ("src/negative.rs", "mutation", "U8"),
    ] {
        assert!(audit.contains(path), "audit does not classify {path}");
        assert!(audit.contains(marker), "audit does not name {marker}");
        assert!(
            audit.contains(removal),
            "audit does not assign {path} to {removal}"
        );
    }
}

#[test]
fn every_temporary_bridge_is_named_and_assigned_to_a_removal_task() {
    let audit = fs::read_to_string("docs/unified-generation-spine-audit.md").unwrap();
    let advanced = fs::read_to_string("src/composition/advanced_family.rs").unwrap();
    assert!(
        !Path::new("src/generator.rs").exists(),
        "the retired curated generator module must be absent"
    );
    assert!(
        !advanced.contains("write_composition_default_artifacts"),
        "composition defaults must not invoke or retain a curated generator bridge"
    );
    assert!(!Path::new("src/composition/curated.rs").exists());
    assert!(audit.contains("resolved_plan_from_curated_dataset"));
    let mut sources = Vec::new();
    rust_sources(Path::new("src"), &mut sources);
    assert!(sources.into_iter().all(|path| {
        !fs::read_to_string(path)
            .unwrap()
            .contains("resolved_plan_from_curated_dataset")
    }));
}

#[test]
fn byte_stable_provider_inventory_is_complete_and_version_decoupled() {
    let registry = json("cases/registry.json");
    let catalog = RecipeCatalog::load(
        "cases/recipes",
        "cases/registry.json",
        "templates/catalog.json",
    )
    .unwrap();
    let mut actual = BTreeMap::<String, usize>::new();
    for case in registry["cases"].as_array().unwrap() {
        if case["status"] != "implemented" || case["determinism"] != "byte_stable" {
            continue;
        }
        let case_id = case["case_id"].as_str().unwrap();
        let identity = catalog.binding_for_case(case_id).unwrap();
        *actual
            .entry(catalog.recipes()[identity].plan_provider_id.clone())
            .or_default() += 1;
    }
    let expected = BTreeMap::from([
        ("mutation.named_plan".to_string(), 15),
        ("native.classic_plan".to_string(), 32),
        ("native.encapsulated_payload_plan".to_string(), 2),
        ("native.enhanced_plan".to_string(), 7),
        ("native.exceptional_sc_plan".to_string(), 1),
        ("native.metadata_sc_plan".to_string(), 7),
        ("native.presentation_state_plan".to_string(), 4),
        ("native.quantitative_plan".to_string(), 5),
        ("native.registration_plan".to_string(), 2),
        ("native.rt_plan".to_string(), 6),
        ("native.sc_plan".to_string(), 66),
        ("native.sr_plan".to_string(), 3),
        ("native.stress_ct_plan".to_string(), 1),
        ("native.stress_sc_plan".to_string(), 4),
        ("native.waveform_plan".to_string(), 2),
        ("native.wsi_plan".to_string(), 5),
    ]);
    assert_eq!(actual, expected);
    assert_eq!(actual.values().sum::<usize>(), 162);

    let mut output_sources = Vec::new();
    rust_sources(Path::new("src/recipes"), &mut output_sources);
    output_sources.extend([
        PathBuf::from("src/composition/modules.rs"),
        PathBuf::from("src/curated_manifest.rs"),
        PathBuf::from("src/validation.rs"),
        PathBuf::from("tests/waveform_document_mesh_plan.rs"),
    ]);
    output_sources.extend(fs::read_dir("src").unwrap().filter_map(|entry| {
        let path = entry.unwrap().path();
        let name = path.file_name()?.to_str()?;
        (name.starts_with("validation_") && name.ends_with("_tests.rs")).then_some(path)
    }));
    for path in output_sources {
        let source = fs::read_to_string(&path).unwrap();
        assert!(
            !source.contains("PACKAGE_VERSION") && !source.contains("CARGO_PKG_VERSION"),
            "{} couples a byte-stable producer to the product release",
            path.display()
        );
    }

    let curated = fs::read_to_string("src/curated_plan.rs").unwrap();
    assert_eq!(curated.matches("crate::PACKAGE_VERSION").count(), 2);
    assert!(curated.contains(
        "recipe.plan_provider_id == HIGH_DICOM_SR_IMPORT_PROVIDER_ID {\n        crate::PACKAGE_VERSION\n    } else {\n        crate::BYTE_STABLE_OUTPUT_VERSION"
    ));
    assert!(curated.contains(
        "recipe.plan_provider_id == crate::recipes::QUANTITATIVE_EXTERNAL_PROVIDER_ID {\n            crate::PACKAGE_VERSION\n        } else {\n            crate::BYTE_STABLE_OUTPUT_VERSION"
    ));
    let semantic = registry["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|case| case["status"] == "implemented" && case["determinism"] == "semantic_stable")
        .filter_map(|case| {
            let identity = catalog.binding_for_case(case["case_id"].as_str().unwrap())?;
            let provider = catalog.recipes()[identity].plan_provider_id.as_str();
            matches!(
                provider,
                "external.highdicom_sr_import_plan" | "external.quantitative_import_plan"
            )
            .then_some(provider)
        })
        .fold(BTreeMap::<&str, usize>::new(), |mut counts, provider| {
            *counts.entry(provider).or_default() += 1;
            counts
        });
    assert_eq!(
        semantic,
        BTreeMap::from([
            ("external.highdicom_sr_import_plan", 2),
            ("external.quantitative_import_plan", 3),
        ])
    );

    let allowed_product_version_lines = BTreeMap::from([
        (
            "src/assembly/run.rs",
            vec![
                "\"generator\": { \"name\": crate::PACKAGE_NAME, \"version\": crate::PACKAGE_VERSION, \"target\": crate::TARGET_TRIPLE, \"rustc\": crate::RUSTC_VERSION },",
            ],
        ),
        (
            "src/codecs.rs",
            vec!["use crate::PACKAGE_VERSION;", "version: PACKAGE_VERSION,"],
        ),
        (
            "src/composition/executor_adapter.rs",
            vec![
                "use crate::{PACKAGE_VERSION, sha256_hex};",
                "version: PACKAGE_VERSION.into(),",
            ],
        ),
        (
            "src/composition/external_quantitative.rs",
            vec!["software_versions: env!(\"CARGO_PKG_VERSION\").into(),"],
        ),
        (
            "src/composition/run.rs",
            vec![
                "use crate::{PACKAGE_NAME, PACKAGE_VERSION, RUSTC_VERSION, TARGET_TRIPLE, sha256_hex};",
                "\"version\": PACKAGE_VERSION,",
            ],
        ),
        (
            "src/curated_execution.rs",
            vec![
                "PACKAGE_VERSION, WsiPyramidLockedInputs, WsiPyramidMemberIdentity, WsiPyramidRole, sha256_hex,",
                "version: PACKAGE_VERSION.into(),",
            ],
        ),
        (
            "src/curated_execution/external_import.rs",
            vec!["software_versions: env!(\"CARGO_PKG_VERSION\").into(),"],
        ),
        (
            "src/curated_plan.rs",
            vec!["crate::PACKAGE_VERSION", "crate::PACKAGE_VERSION"],
        ),
        (
            "src/executor/materialization.rs",
            vec![
                "use crate::{PACKAGE_VERSION, sha256_hex};",
                "version: PACKAGE_VERSION.into(),",
            ],
        ),
        (
            "src/lib.rs",
            vec![
                "pub const PACKAGE_VERSION: &str = env!(\"CARGO_PKG_VERSION\");",
                "/// Product releases report [`PACKAGE_VERSION`] separately. Changing the",
                "format!(\"{PACKAGE_NAME} {PACKAGE_VERSION}\")",
                "\"version\": PACKAGE_VERSION,",
            ],
        ),
    ]);
    for (path, expected) in allowed_product_version_lines {
        let source = fs::read_to_string(path).unwrap();
        let actual = source
            .lines()
            .filter(|line| line.contains("PACKAGE_VERSION") || line.contains("CARGO_PKG_VERSION"))
            .map(str::trim)
            .collect::<Vec<_>>();
        assert_eq!(
            actual, expected,
            "{path} has an unclassified product-version coupling in the output pipeline"
        );
    }
}

#[test]
fn every_direct_output_version_provider_family_has_an_exact_payload_plan() {
    const IMPLEMENTATION_UID: &str = "2.25.93442075376351194778596039619060852790";
    const OUTPUT_VERSION: &str = "0.1.0";
    let representatives = [
        (
            "native.sc_plan",
            "classic/sc/mono1_u8_explicit_le",
            "curated_sc_mono1_u8_instance",
            "71792e2a52c1bb1b0ef483324922a4c7c7613d0ae9535f088f84601c33eec32a",
        ),
        (
            "native.metadata_sc_plan",
            "metadata/sc/defined_undefined_sequence_lengths",
            "curated_metadata_sc_defined_undefined_sequence_lengths_defined",
            "ba18d5029e477f943d6765a9d706cef3c5f437a24c5346fa4f64c8adafaf2040",
        ),
        (
            "native.classic_plan",
            "classic/cr/overlay_modality_voi_explicit_le",
            "curated_cr_overlay_modality_voi_instance",
            "9656dc07538da6542157492b28bbd1c5bb9f27a7b86d73e522440885aa8c6430",
        ),
        (
            "native.enhanced_plan",
            "enhanced/pet/multiframe_explicit_le",
            "advanced_enhanced_pet_multiframe_explicit_le_artifact_1",
            "8d970294e2143f6f55d6c134d5fea21366428faaf33c8ee368f48a2ab9e3dca7",
        ),
        (
            "native.wsi_plan",
            "vl/wsi/tiled_full_small",
            "wsi_tiled_full",
            "be80772ebab7462896244117b6581caf50a541e0eb6aa9f030c97e3bd217b1ab",
        ),
        (
            "native.registration_plan",
            "derived/registration/spatial_ct_pair",
            "curated_derived_registration_spatial_ct_pair_artifact_1",
            "7e3f825b6923734077281143600098003312c6acecf73a69828853a0ddb6c80c",
        ),
        (
            "native.presentation_state_plan",
            "derived/presentation-state/grayscale_softcopy_ct_window_explicit_le",
            "curated_gsps_grayscale_softcopy_ct_window_artifact_1",
            "c032ecbd1179acd98e356a5d4a036fb6fd33ca313c841d91dd8f381efaec647c",
        ),
        (
            "native.waveform_plan",
            "non-image/waveform/general_ecg",
            "curated_non_image_waveform_general_ecg_artifact_1",
            "1848a8c90fee9191d66270822e9fb8fd4f950169182c79688f7ae6a56e8e717a",
        ),
        (
            "native.encapsulated_payload_plan",
            "non-image/encapsulated-document/pdf_minimal_explicit_le",
            "curated_encapsulated_pdf_minimal_artifact_1",
            "f46f37b1f643c4a9b53814bde2926aeb372f7ebad1e822ef174ec2426f624967",
        ),
        (
            "native.quantitative_plan",
            "derived/rwvm/linear_ct_mapping_explicit_le",
            "curated_rwvm_linear_ct_mapping_mapping",
            "002629859f739a4cbc4699a9f1550b095f29bc90e869cde201f3481ec00c0915",
        ),
        (
            "native.sr_plan",
            "derived/sr/basic_text_observation_explicit_le",
            "curated_sr_basic_text_observation_artifact_1",
            "5438c54932da7b4de3e733f385854e2538a350507fb7e9195097f1d6e32d6fc6",
        ),
        (
            "native.rt_plan",
            "non-image/rt/plan_linked",
            "curated_non_image_rt_plan_linked_artifact_1",
            "e8210ca6e1c6864d259d1817c18c4102ec3db65e2b52cdc7c756a1426594c17b",
        ),
        (
            "native.stress_ct_plan",
            "stress/study/high_instance_count_ct",
            "curated_stress_study_high_instance_count_ct_slice_001",
            "5b36d3cf3f930d803a07d118fe9023f3aa80eecd032fc75e6fe7d1c501be00b9",
        ),
        (
            "native.stress_sc_plan",
            "stress/sc/deep_nested_sequences",
            "curated_stress_sc_deep_nested_sequences_instance",
            "6ad82a4b0c076e95037a30a69f85914c40792b6e2c536498f7d07bd6d8068f8f",
        ),
    ];
    let provider =
        CuratedScCorpusPlanProvider::load(CuratedCatalogPaths::from_repository_root(".")).unwrap();
    let catalog = RecipeCatalog::load(
        "cases/recipes",
        "cases/registry.json",
        "templates/catalog.json",
    )
    .unwrap();

    for (provider_id, case_id, expected_artifact_id, expected_plan_sha256) in representatives {
        let identity = catalog.binding_for_case(case_id).unwrap();
        assert_eq!(catalog.recipes()[identity].plan_provider_id, provider_id);
        let bundle = provider
            .plan(&CuratedScPlanRequest {
                selection: CuratedScSelection::CaseIds(vec![case_id.into()]),
                seed: 1,
                max_parallelism: 1,
            })
            .unwrap();
        let mut matching = bundle
            .plan
            .artifacts
            .iter()
            .filter_map(|artifact| match artifact {
                PlannedArtifact::Dicom(dicom)
                    if dicom
                        .case_binding
                        .as_ref()
                        .is_some_and(|binding| binding.case_id == case_id) =>
                {
                    Some(dicom)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        matching.sort_by_key(|artifact| artifact.logical_id.as_str());
        let artifact = matching
            .first()
            .unwrap_or_else(|| panic!("no plan for {case_id}"));
        assert_eq!(
            artifact.encoding.implementation.class_uid, IMPLEMENTATION_UID,
            "{provider_id} implementation UID drifted"
        );
        assert_eq!(
            artifact
                .instance
                .identities
                .get(&CompositionUidRole::ImplementationClass, 0),
            Some(IMPLEMENTATION_UID),
            "{provider_id} plan identity drifted"
        );
        let software_version = artifact.instance.attributes.iter().find_map(|attribute| {
            if attribute.address.normalized_tag() != "0018,1020" {
                return None;
            }
            match attribute.value.as_ref() {
                Some(AttributeValue::Primitive(PrimitiveValue::String(value))) => {
                    Some(value.as_str())
                }
                _ => panic!("{provider_id} has a non-string Software Versions value"),
            }
        });
        assert_eq!(
            software_version,
            Some(OUTPUT_VERSION),
            "{provider_id} output version drifted"
        );
        assert_eq!(
            (
                artifact.logical_id.as_str(),
                artifact.instance.canonical_sha256().as_str()
            ),
            (expected_artifact_id, expected_plan_sha256),
            "{provider_id} representative plan drifted"
        );
    }
}
