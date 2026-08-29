use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use dicom_dictionary_std::tags;
use dicom_object::open_file;
use serde_json::Value;

use dicom_test_suite::recipes::{RecipeCatalog, RecipeCatalogError, RecipeIdentity};

const EOT_SC_CASE_ID: &str = "encapsulation/sc/eot_single_fragment_multiframe";

fn registry() -> Value {
    serde_json::from_slice(&fs::read("cases/registry.json").unwrap()).unwrap()
}

fn is_feature_free_native_sc(case: &&Value) -> bool {
    case["status"] == "implemented"
        && case["provider"]["kind"] == "rust_native"
        && case["provider"]["id"] == "rust_native"
        && case["requirements"]["features"]
            .as_array()
            .unwrap()
            .is_empty()
        && case["requirements"]["external_codecs"]
            .as_array()
            .unwrap()
            .is_empty()
        && (case["case_id"].as_str().unwrap().starts_with("classic/sc/")
            || case["case_id"] == EOT_SC_CASE_ID)
}

struct GeneratedRoots(Vec<PathBuf>);

impl Drop for GeneratedRoots {
    fn drop(&mut self) {
        for root in &self.0 {
            let _ = fs::remove_dir_all(root);
        }
    }
}

fn fresh_generation_root(profile: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "dicom-test-suite-u31-{profile}-{}-{nonce}",
        std::process::id()
    ))
}

fn generate_profile(profile: &str, include_stress: bool) -> PathBuf {
    let root = fresh_generation_root(profile);
    let mut command = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"));
    command.args([
        "generate",
        "--profile",
        profile,
        "--out",
        root.to_str().unwrap(),
        "--seed",
        "3101",
    ]);
    if include_stress {
        command.arg("--include-stress");
    }
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "{profile} generation failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    root
}

fn manifest_files(root: &Path) -> Vec<Value> {
    let manifest: Value =
        serde_json::from_slice(&fs::read(root.join("manifest.json")).unwrap()).unwrap();
    manifest["files"].as_array().unwrap().clone()
}

#[test]
fn catalog_exactly_and_uniquely_binds_every_implemented_registry_recipe() {
    let catalog = RecipeCatalog::load(
        "cases/recipes",
        "cases/registry.json",
        "templates/catalog.json",
    )
    .unwrap();
    let registry: Value =
        serde_json::from_slice(&fs::read("cases/registry.json").unwrap()).unwrap();
    let expected = registry["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|case| case["status"] == "implemented")
        .map(|case| RecipeIdentity {
            recipe_id: case["recipe_id"].as_str().unwrap().to_string(),
            recipe_version: case["recipe_version"].as_str().unwrap().to_string(),
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        catalog.recipes().keys().cloned().collect::<BTreeSet<_>>(),
        expected
    );
    for case in registry["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|case| case["status"] == "implemented")
    {
        let case_id = case["case_id"].as_str().unwrap();
        assert_eq!(
            catalog.binding_for_case(case_id),
            Some(&RecipeIdentity {
                recipe_id: case["recipe_id"].as_str().unwrap().to_string(),
                recipe_version: case["recipe_version"].as_str().unwrap().to_string(),
            })
        );
    }
}

#[test]
fn feature_free_native_sc_set_is_derived_and_fully_data_first() {
    let catalog = RecipeCatalog::load(
        "cases/recipes",
        "cases/registry.json",
        "templates/catalog.json",
    )
    .unwrap();
    let registry = registry();
    let expected = registry["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(is_feature_free_native_sc)
        .map(|case| case["case_id"].as_str().unwrap().to_string())
        .collect::<BTreeSet<_>>();
    let actual = catalog
        .recipes()
        .values()
        .filter(|recipe| recipe.plan_provider_id == "native.sc_plan")
        .map(|recipe| recipe.binding.case_id.clone())
        .collect::<BTreeSet<_>>();
    assert_eq!(actual, expected);

    for case_id in expected {
        let identity = catalog.binding_for_case(&case_id).unwrap();
        let recipe = &catalog.recipes()[identity];
        assert_eq!(recipe.plan_provider_id, "native.sc_plan");
        assert!(recipe.provider_parameters.is_empty());
        assert!(!recipe.validation_rule_ids.is_empty());

        let artifacts = &recipe.dicom.as_ref().unwrap().artifacts;
        let orders = artifacts
            .iter()
            .map(|artifact| artifact.order)
            .collect::<Vec<_>>();
        assert_eq!(orders, (0..artifacts.len() as u32).collect::<Vec<_>>());
        for artifact in artifacts {
            let pixels = artifact.secondary_capture.as_ref().unwrap();
            assert_eq!(artifact.content.provider_id, "content.sc.pixel_pattern");
            assert!(artifact.content.parameters.is_empty());
            assert!(artifact.algorithm_provider_id.is_none());
            assert!(artifact.output.path.as_deref().is_some_and(|path| {
                path.starts_with(&format!("{case_id}/")) && path.ends_with(".dcm")
            }));
            assert_ne!(artifact.output.provider_derived, Some(true));
            assert_eq!(
                artifact.encoding.preamble_policy.as_deref(),
                Some("zero_filled")
            );
            assert_eq!(
                artifact.encoding.file_meta_policy.as_deref(),
                Some("standard")
            );
            assert!(!artifact.validation_rule_ids.is_empty());
            assert!(!artifact.stressors.is_empty());
            assert!(!pixels.stored_values.is_empty());
            assert_eq!(pixels.frame_sha256.len(), pixels.frames as usize);
        }
    }
}

#[test]
fn data_first_sc_encoding_and_multi_output_bindings_are_explicit() {
    let catalog = RecipeCatalog::load(
        "cases/recipes",
        "cases/registry.json",
        "templates/catalog.json",
    )
    .unwrap();
    for recipe in catalog
        .recipes()
        .values()
        .filter(|recipe| recipe.plan_provider_id == "native.sc_plan")
    {
        for artifact in &recipe.dicom.as_ref().unwrap().artifacts {
            let expected_provider = match artifact.encoding.transfer_syntax_uid.as_str() {
                "1.2.840.10008.1.2.5" => Some("encoding.native.rle_lossless"),
                "1.2.840.10008.1.2.2" => Some("encoding.native.explicit_vr_big_endian"),
                _ => None,
            };
            assert_eq!(
                artifact
                    .encoding
                    .non_template_encoding_provider_id
                    .as_deref(),
                expected_provider
            );
        }
    }

    let identity = catalog
        .binding_for_case("classic/sc/nonsquare_pixel_spacing")
        .unwrap();
    let artifacts = &catalog.recipes()[identity]
        .dicom
        .as_ref()
        .unwrap()
        .artifacts;
    assert_eq!(
        artifacts
            .iter()
            .map(|artifact| (
                artifact.order,
                artifact.output.role.as_str(),
                artifact.output.path.as_deref()
            ))
            .collect::<Vec<_>>(),
        vec![
            (
                0,
                "pixel_spacing",
                Some("classic/sc/nonsquare_pixel_spacing/pixel-spacing.dcm"),
            ),
            (
                1,
                "pixel_aspect_ratio",
                Some("classic/sc/nonsquare_pixel_spacing/pixel-aspect-ratio.dcm"),
            ),
        ]
    );
}

#[test]
fn data_first_sc_values_and_hashes_match_current_generator_bytes() {
    let all_root = generate_profile("all", true);
    let legacy_root = generate_profile("legacy", false);
    let _cleanup = GeneratedRoots(vec![all_root.clone(), legacy_root.clone()]);
    let mut files = manifest_files(&all_root);
    files.extend(manifest_files(&legacy_root));

    let catalog = RecipeCatalog::load(
        "cases/recipes",
        "cases/registry.json",
        "templates/catalog.json",
    )
    .unwrap();
    for recipe in catalog
        .recipes()
        .values()
        .filter(|recipe| recipe.plan_provider_id == "native.sc_plan")
    {
        for artifact in &recipe.dicom.as_ref().unwrap().artifacts {
            let relative_path = artifact.output.path.as_deref().unwrap();
            let file = files
                .iter()
                .find(|file| file["path"] == relative_path)
                .unwrap_or_else(|| panic!("generated manifest lacks {relative_path}"));
            let pixels = artifact.secondary_capture.as_ref().unwrap();
            assert_eq!(file["case_id"], recipe.binding.case_id);
            assert_eq!(file["recipe"]["recipe_id"], recipe.recipe_id);
            assert_eq!(file["recipe"]["recipe_version"], recipe.recipe_version);
            assert_eq!(
                file["dicom"]["transfer_syntax_uid"],
                artifact.encoding.transfer_syntax_uid
            );
            assert_eq!(file["image"]["rows"], pixels.rows);
            assert_eq!(file["image"]["columns"], pixels.columns);
            assert_eq!(file["image"]["frames"], pixels.frames);
            assert_eq!(file["image"]["samples_per_pixel"], pixels.samples_per_pixel);
            assert_eq!(
                file["image"]["photometric_interpretation"],
                pixels.photometric_interpretation
            );
            assert_eq!(file["image"]["bits_allocated"], pixels.bits_allocated);
            assert_eq!(file["image"]["bits_stored"], pixels.bits_stored);
            assert_eq!(file["image"]["high_bit"], pixels.high_bit);
            assert_eq!(
                file["image"]["pixel_representation"],
                pixels.pixel_representation
            );
            assert_eq!(file["pixel_data"]["vr"], pixels.pixel_data_vr);
            assert_eq!(
                file["pixel_data"]["frame_hashes"],
                serde_json::to_value(&pixels.frame_sha256).unwrap()
            );
            assert_eq!(
                file["recipe"]["recipe_parameters"]["pixel_values"],
                serde_json::to_value(&pixels.stored_values).unwrap()
            );
            assert_eq!(
                file["recipe"]["recipe_parameters"]["pixel_padding"],
                serde_json::to_value(&pixels.padding).unwrap()
            );
            assert_eq!(file["expected_semantics"]["pixel_min"], pixels.pixel_min);
            assert_eq!(file["expected_semantics"]["pixel_max"], pixels.pixel_max);
            assert_eq!(
                file["known_stressors"],
                serde_json::to_value(&artifact.stressors).unwrap()
            );
            assert_eq!(file["validation"]["status"], "passed");

            let root = if recipe.binding.case_id == "classic/sc/mono2_u8_explicit_be" {
                &legacy_root
            } else {
                &all_root
            };
            let bytes = fs::read(root.join(relative_path)).unwrap();
            assert_eq!(&bytes[..128], &[0; 128]);
            assert_eq!(&bytes[128..132], b"DICM");

            let planar = pixels
                .color
                .as_ref()
                .and_then(|color| color.planar_configuration)
                .map(Value::from)
                .unwrap_or(Value::Null);
            assert_eq!(file["image"]["planar_configuration"], planar);
            if let Some(color) = &pixels.color {
                let expected_subsampling = if pixels.photometric_interpretation == "YBR_FULL_422" {
                    "horizontal_2_to_1"
                } else {
                    "none"
                };
                assert_eq!(color.chroma_subsampling, expected_subsampling);
            }

            if let Some(palette) = &pixels.palette {
                assert_eq!(
                    file["recipe"]["recipe_parameters"]["palette"]["descriptor"],
                    serde_json::to_value(palette.descriptor).unwrap()
                );
                let object = open_file(root.join(relative_path)).unwrap();
                for (tag, expected) in [
                    (tags::RED_PALETTE_COLOR_LOOKUP_TABLE_DATA, &palette.red),
                    (tags::GREEN_PALETTE_COLOR_LOOKUP_TABLE_DATA, &palette.green),
                    (tags::BLUE_PALETTE_COLOR_LOOKUP_TABLE_DATA, &palette.blue),
                ] {
                    let data = object.element(tag).unwrap().value().to_bytes().unwrap();
                    let actual = data
                        .chunks_exact(2)
                        .map(|word| u16::from_le_bytes([word[0], word[1]]))
                        .collect::<Vec<_>>();
                    assert_eq!(&actual, expected);
                }
            }

            let encapsulation = &file["pixel_data"]["encapsulated_pixel_data"];
            match artifact.encoding.offset_table_policy.as_str() {
                "none" => assert!(encapsulation.is_null()),
                "empty_basic" => {
                    assert_eq!(encapsulation["basic_offset_table"]["present"], true);
                    assert_eq!(encapsulation["basic_offset_table"]["populated"], false);
                }
                "populated_basic" => {
                    assert_eq!(encapsulation["basic_offset_table"]["present"], true);
                    assert_eq!(encapsulation["basic_offset_table"]["populated"], true);
                }
                "extended" => {
                    assert_eq!(encapsulation["extended_offset_table"]["present"], true);
                    assert_eq!(
                        encapsulation["extended_offset_table"]["lengths_present"],
                        true
                    );
                }
                policy => panic!("unexpected migrated offset-table policy {policy}"),
            }
            if artifact.encoding.fragmentation_policy == "one_per_frame" {
                assert!(
                    encapsulation["fragments_per_frame"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .all(|count| count == 1)
                );
            }
        }
    }
}

#[test]
fn modular_loading_and_dependency_order_are_deterministic() {
    let first = RecipeCatalog::load(
        "cases/recipes",
        "cases/registry.json",
        "templates/catalog.json",
    )
    .unwrap();
    let second = RecipeCatalog::load(
        "cases/recipes",
        "cases/registry.json",
        "templates/catalog.json",
    )
    .unwrap();
    assert_eq!(first.ordered_identities(), second.ordered_identities());
    let positions = first
        .ordered_identities()
        .iter()
        .enumerate()
        .map(|(index, identity)| (identity, index))
        .collect::<std::collections::BTreeMap<_, _>>();
    for (identity, recipe) in first.recipes() {
        for dependency in &recipe.dependencies {
            assert!(positions[&dependency.recipe.identity()] < positions[identity]);
        }
    }
}

#[test]
fn schema_rejects_unknown_fields_before_completeness_checks() {
    let error = RecipeCatalog::load(
        "tests/fixtures/case-recipes/invalid",
        "cases/registry.json",
        "templates/catalog.json",
    )
    .unwrap_err();
    assert!(matches!(error, RecipeCatalogError::Schema { .. }));
}

#[test]
fn committed_positive_fixture_is_schema_valid() {
    let schema: Value =
        serde_json::from_slice(&fs::read("schemas/case-recipe.schema.json").unwrap()).unwrap();
    let fixture: Value =
        serde_json::from_slice(&fs::read("tests/fixtures/case-recipes/valid/dicom.json").unwrap())
            .unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    assert_eq!(validator.iter_errors(&fixture).count(), 0);
}

#[test]
fn schema_rejects_parent_traversal_output_path() {
    let schema: Value =
        serde_json::from_slice(&fs::read("schemas/case-recipe.schema.json").unwrap()).unwrap();
    let fixture: Value = serde_json::from_slice(
        &fs::read("tests/fixtures/case-recipes/invalid/unsafe-output.json").unwrap(),
    )
    .unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    assert!(validator.iter_errors(&fixture).next().is_some());
}

#[test]
fn schema_rejects_kind_payload_mismatch() {
    let schema: Value =
        serde_json::from_slice(&fs::read("schemas/case-recipe.schema.json").unwrap()).unwrap();
    let fixture: Value = serde_json::from_slice(
        &fs::read("tests/fixtures/case-recipes/invalid/kind-payload-mismatch.json").unwrap(),
    )
    .unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    assert!(validator.iter_errors(&fixture).next().is_some());
}
