use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::{BundleRoot, CorpusDefinitionBundle, CorpusDefinitionError, CorpusDefinitionLimits};

fn fixture() -> PathBuf {
    PathBuf::from("tests/fixtures/corpus-definition/minimal")
}

fn copy_bundle(source: &Path, target: &Path) {
    for entry in walk(source) {
        let relative = entry.strip_prefix(source).unwrap();
        let destination = target.join(relative);
        if entry.is_dir() {
            fs::create_dir_all(&destination).unwrap();
        } else {
            fs::create_dir_all(destination.parent().unwrap()).unwrap();
            fs::copy(entry, destination).unwrap();
        }
    }
}

fn walk(root: &Path) -> Vec<PathBuf> {
    fn recurse(path: &Path, out: &mut Vec<PathBuf>) {
        let mut entries = fs::read_dir(path)
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect::<Vec<_>>();
        entries.sort();
        for entry in entries {
            out.push(entry.clone());
            if entry.is_dir() {
                recurse(&entry, out);
            }
        }
    }
    let mut out = Vec::new();
    recurse(root, &mut out);
    out
}

fn temp(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "synth-dicom-gen-corpus-definition-{}-{name}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir(&path).unwrap();
    path
}

fn rewrite_registry(root: &Path, registry: &serde_json::Value, manifest: &mut serde_json::Value) {
    let bytes = serde_json::to_vec(registry).unwrap();
    fs::write(root.join("cases/registry.json"), &bytes).unwrap();
    manifest["registry"]["size_bytes"] = bytes.len().into();
    manifest["registry"]["sha256"] = crate::sha256_hex(&bytes).into();
    fs::write(
        root.join("corpus-definition.json"),
        serde_json::to_vec(manifest).unwrap(),
    )
    .unwrap();
}

const CT_CASE: &str = "classic/ct/mono2_i16_rescale_12bit_explicit_le";
const DX_CASE: &str = "classic/dx/display_shutter_mono2_u16_explicit_le";

fn one_case_bundle(
    name: &str,
    source_case: &str,
    target_case: &str,
    target_recipe_id: &str,
    mutate: impl FnOnce(&mut serde_json::Value),
) -> PathBuf {
    let root = temp(name);
    let registry: serde_json::Value =
        serde_json::from_slice(&fs::read("cases/registry.json").unwrap()).unwrap();
    let mut row = registry["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["case_id"] == source_case)
        .unwrap()
        .clone();
    let source_recipe_id = row["recipe_id"].as_str().unwrap();
    let source_recipe_path = walk(Path::new("cases/recipes"))
        .into_iter()
        .find(|path| {
            path.is_file()
                && serde_json::from_slice::<serde_json::Value>(&fs::read(path).unwrap())
                    .ok()
                    .is_some_and(|value| value["recipe_id"] == source_recipe_id)
        })
        .unwrap();
    let mut recipe: serde_json::Value =
        serde_json::from_slice(&fs::read(source_recipe_path).unwrap()).unwrap();
    recipe["binding"]["case_id"] = target_case.into();
    recipe["recipe_id"] = target_recipe_id.into();
    mutate(&mut recipe);

    row["case_id"] = target_case.into();
    row["recipe_id"] = target_recipe_id.into();
    row["profiles"] = serde_json::json!(["core"]);
    let registry = serde_json::json!({
        "case_registry_schema_version": registry["case_registry_schema_version"],
        "cases": [row]
    });
    let recipe_path = "cases/recipes/caller.json";
    fs::create_dir_all(root.join("cases/recipes")).unwrap();
    let recipe_bytes = serde_json::to_vec(&recipe).unwrap();
    fs::write(root.join(recipe_path), &recipe_bytes).unwrap();
    let registry_bytes = serde_json::to_vec(&registry).unwrap();
    fs::write(root.join("cases/registry.json"), &registry_bytes).unwrap();
    let profiles = [
        ("smoke", "valid"),
        ("core", "valid"),
        ("extended", "valid"),
        ("legacy", "legacy"),
        ("stress", "stress"),
        ("negative", "expected_invalid"),
        ("fuzz", "fuzz"),
    ]
    .into_iter()
    .map(|(profile_id, scope)| {
        serde_json::json!({
            "profile_id": profile_id,
            "scope": scope,
            "members": if profile_id == "core" { vec![target_case] } else { vec![] }
        })
    })
    .chain(std::iter::once(serde_json::json!({
        "profile_id": "all",
        "scope": "valid",
        "union_of": ["smoke", "core", "extended"],
        "optional_profile": "stress"
    })))
    .collect::<Vec<_>>();
    let mut evidence = Vec::new();
    for note in registry["cases"][0]["standards_evidence"]
        .as_array()
        .unwrap()
    {
        if note["source"] != "local-source-note" {
            continue;
        }
        let source_path = note["query"].as_str().unwrap();
        let basename = Path::new(source_path)
            .file_name()
            .unwrap()
            .to_str()
            .unwrap();
        let bytes = fs::read(source_path).unwrap();
        let path = format!("evidence/{basename}");
        fs::create_dir_all(root.join("evidence")).unwrap();
        fs::write(root.join(&path), &bytes).unwrap();
        evidence.push(serde_json::json!({"evidence_id":format!("source-note.{}", basename.trim_end_matches(".md")), "media_type":"text/markdown", "path":path, "size_bytes":bytes.len(), "sha256":crate::sha256_hex(&bytes)}));
    }
    let descriptor = serde_json::json!({
        "corpus_definition_bundle_schema_version": "1.0.0",
        "definition_id": "fixture.ct-capability",
        "definition_version": "1.0.0",
        "profiles": profiles,
        "registry": {
            "path": "cases/registry.json",
            "size_bytes": registry_bytes.len(),
            "sha256": crate::sha256_hex(&registry_bytes)
        },
        "cases": [{
            "case_id": target_case,
            "recipe_id": target_recipe_id,
            "recipe_version": recipe["recipe_version"],
            "recipe": {
                "path": recipe_path,
                "size_bytes": recipe_bytes.len(),
                "sha256": crate::sha256_hex(&recipe_bytes)
            },
            "dependencies": [],
            "evidence_ids": evidence.iter().map(|note| note["evidence_id"].clone()).collect::<Vec<_>>(),
            "asset_ids": []
        }],
        "evidence": evidence,
        "assets": []
    });
    fs::write(
        root.join("corpus-definition.json"),
        serde_json::to_vec(&descriptor).unwrap(),
    )
    .unwrap();
    root
}

fn assert_one_case_rejected(
    name: &str,
    source_case: &str,
    target_case: &str,
    mutate: impl FnOnce(&mut serde_json::Value),
) {
    let root = one_case_bundle(name, source_case, target_case, "caller_recipe", mutate);
    let error = CorpusDefinitionBundle::load(&root).unwrap_err();
    assert!(
        matches!(&error, CorpusDefinitionError::Closure(_)),
        "{error}"
    );
    assert_eq!(error.code(), "resource.document.invalid", "{error}");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn external_ct_capability_is_name_independent_and_integrity_bound() {
    for (name, case_id) in [
        ("renamed-ct", "caller/arbitrary/signed-ct"),
        ("misleading-ct", "classic/dx/caller-named-ct"),
    ] {
        let root = one_case_bundle(name, CT_CASE, case_id, "caller_signed_ct", |recipe| {
            recipe["planning_order"] = 900.into();
        });
        let bundle = CorpusDefinitionBundle::load(&root).unwrap();
        let catalog =
            crate::recipes::RecipeCatalog::from_verified_bundle(&bundle, Path::new(".")).unwrap();
        assert_eq!(bundle.manifest().cases[0].case_id, case_id);
        assert_eq!(bundle.manifest().cases[0].recipe_id, "caller_signed_ct");
        assert!(catalog.binding_for_case(case_id).is_some());
        fs::remove_dir_all(root).unwrap();
    }

    let legacy = one_case_bundle("legacy-dx", DX_CASE, DX_CASE, "caller_dx", |_| {});
    let bundle = CorpusDefinitionBundle::load(&legacy).unwrap();
    crate::recipes::RecipeCatalog::from_verified_bundle(&bundle, Path::new(".")).unwrap();
    fs::remove_dir_all(legacy).unwrap();
}

#[test]
fn external_ct_capability_is_fail_closed_without_broadening_classic_names() {
    assert_one_case_rejected("ct-algorithm", CT_CASE, "caller/ct/algorithm", |recipe| {
        recipe["dicom"]["artifacts"][0]["algorithm_provider_id"] = "algorithm.classic_dx_mg".into();
    });
    assert_one_case_rejected(
        "ct-missing-algorithm",
        CT_CASE,
        "caller/ct/missing-algorithm",
        |recipe| {
            recipe["dicom"]["artifacts"][0]
                .as_object_mut()
                .unwrap()
                .remove("algorithm_provider_id");
        },
    );
    assert_one_case_rejected("ct-template", CT_CASE, "caller/ct/template", |recipe| {
        recipe["dicom"]["artifacts"][0]["template"]["template_id"] =
            "classic/dx/for-presentation".into();
    });
    assert_one_case_rejected("ct-version", CT_CASE, "caller/ct/version", |recipe| {
        recipe["dicom"]["artifacts"][0]["template"]["template_version"] = "2.0.0".into();
    });
    assert_one_case_rejected("ct-content", CT_CASE, "caller/ct/content", |recipe| {
        recipe["dicom"]["artifacts"][0]["content"]["provider_id"] = "content.case_default".into();
    });
    assert_one_case_rejected("ct-projection", CT_CASE, "caller/ct/projection", |recipe| {
        recipe["dicom"]["artifacts"][0]["classic_projection"]["family"] = "dx_mg".into();
    });
    assert_one_case_rejected(
        "ct-provider-params",
        CT_CASE,
        "caller/ct/provider-params",
        |recipe| {
            recipe["provider_parameters"]["unexpected"] = true.into();
        },
    );
    assert_one_case_rejected(
        "ct-artifact-params",
        CT_CASE,
        "caller/ct/artifact-params",
        |recipe| {
            recipe["dicom"]["artifacts"][0]["parameters"]["unexpected"] = true.into();
        },
    );
    assert_one_case_rejected(
        "ct-missing-order",
        CT_CASE,
        "caller/ct/missing-order",
        |recipe| {
            recipe.as_object_mut().unwrap().remove("planning_order");
        },
    );
    assert_one_case_rejected(
        "ct-plan-provider",
        CT_CASE,
        "caller/ct/plan-provider",
        |recipe| {
            recipe["plan_provider_id"] = "native.sc_plan".into();
        },
    );
    assert_one_case_rejected("ct-mixed", CT_CASE, "caller/ct/mixed", |recipe| {
        let mut second = recipe["dicom"]["artifacts"][0].clone();
        second["order"] = 1.into();
        second["logical_id"] = "mixed".into();
        second["output"]["path"] = "caller/mixed.dcm".into();
        second["algorithm_provider_id"] = "algorithm.classic_dx_mg".into();
        recipe["dicom"]["artifacts"]
            .as_array_mut()
            .unwrap()
            .push(second);
    });

    assert_one_case_rejected(
        "classic-name-only",
        "classic/nm/multiframe_explicit_le",
        "caller/arbitrary/not-ct",
        |_| {},
    );
}

#[test]
fn external_dx_mg_capability_is_name_independent_and_fail_closed() {
    for (index, source) in [
        DX_CASE,
        "classic/mg/for_presentation_mono1_u16_12bit_explicit_le",
        "classic/mg/for_processing_mono2_u16_12bit_implicit_le",
    ]
    .into_iter()
    .enumerate()
    {
        let case_id = format!("caller/projection/acquisition-{index}");
        let root = one_case_bundle(
            &format!("generic-dx-mg-{index}"),
            source,
            &case_id,
            "caller_projection",
            |recipe| {
                recipe["planning_order"] = 900.into();
                recipe["dicom"]["artifacts"][0]["output"]["path"] = "images/projection.dcm".into();
            },
        );
        let bundle = CorpusDefinitionBundle::load(&root).unwrap();
        let catalog =
            crate::recipes::RecipeCatalog::from_verified_bundle(&bundle, Path::new(".")).unwrap();
        assert!(catalog.binding_for_case(&case_id).is_some());
        fs::remove_dir_all(root).unwrap();
        for field in ["template", "algorithm_provider_id", "classic_projection"] {
            assert_one_case_rejected(
                &format!("partial-dx-mg-{index}-{field}"),
                source,
                &case_id,
                |recipe| {
                    recipe["dicom"]["artifacts"][0]
                        .as_object_mut()
                        .unwrap()
                        .remove(field);
                },
            );
        }
    }
}

#[test]
fn external_cr_capability_is_name_independent_and_fail_closed() {
    const CR: &str = "classic/cr/overlay_modality_voi_explicit_le";
    for (label, case_id) in [
        ("arbitrary-cr", "caller/radiograph/native"),
        ("misleading-mr-cr", "classic/mr/caller-radiograph"),
    ] {
        let root = one_case_bundle(label, CR, case_id, "caller_radiograph", |recipe| {
            recipe["planning_order"] = 900.into();
            recipe["projection_order"] = 901.into();
            recipe["dicom"]["artifacts"][0]["output"]["path"] = "images/radiograph.dcm".into();
        });
        let bundle = CorpusDefinitionBundle::load(&root).unwrap();
        let catalog =
            crate::recipes::RecipeCatalog::from_verified_bundle(&bundle, Path::new(".")).unwrap();
        assert!(catalog.binding_for_case(case_id).is_some());
        let root = root.canonicalize().unwrap();
        let output = root.with_extension("output");
        let sdk = crate::sdk::DicomTestSuite::embedded().unwrap();
        sdk.generate_corpus(crate::sdk::GenerateCorpusRequest::from_file(
            root.join("corpus-definition.json"),
            &root,
            &output,
            crate::sdk::CorpusSelector::CaseIds {
                profile: "core".into(),
                include_stress: false,
                case_ids: vec![case_id.into()],
            },
        ))
        .unwrap();
        let validation = sdk
            .validate(crate::sdk::ValidateRequest::new(&output))
            .unwrap();
        assert!(validation.is_valid(), "{validation:?}");
        assert_eq!(validation.files_checked(), 1);
        assert!(output.join("images/radiograph.dcm").is_file());
        fs::remove_dir_all(output).unwrap();
        fs::remove_dir_all(root).unwrap();
        for field in ["template", "algorithm_provider_id", "classic_projection"] {
            assert_one_case_rejected(&format!("{label}-missing-{field}"), CR, case_id, |recipe| {
                recipe["dicom"]["artifacts"][0]
                    .as_object_mut()
                    .unwrap()
                    .remove(field);
            });
        }
        assert_one_case_rejected(&format!("{label}-crossed"), CR, case_id, |recipe| {
            recipe["dicom"]["artifacts"][0]["algorithm_provider_id"] =
                "algorithm.classic_ct".into();
        });
    }
}

#[test]
fn external_native_projection_capabilities_are_name_independent_and_fail_closed() {
    for (family, source, modality) in [
        ("xa", "classic/xa/monoplane_explicit_le", "XA"),
        ("xrf", "classic/xrf/monoplane_explicit_le", "RF"),
        ("photo-rgb", "vl/photo/rgb_planar0_explicit_le", "XC"),
        ("photo-palette", "vl/photo/palette_color_explicit_le", "XC"),
    ] {
        let names: &[(&str, &str)] = if modality == "XC" {
            &[
                ("caller", "caller/photo/native"),
                ("xa", "classic/xa/monoplane_explicit_le"),
                ("wsi", "vl/wsi/pyramid_multiresolution"),
                ("icc", "vl/photo/rgb_icc_profile_explicit_le"),
            ]
        } else {
            &[
                ("caller", "caller/projection/native"),
                ("vl", "vl/wsi/pyramid_multiresolution"),
                ("pet", "classic/pet/rescaled_activity_explicit_le"),
            ]
        };
        for &(name, case_id) in names {
            let label = format!("{family}-{name}");
            let root = one_case_bundle(&label, source, case_id, "caller_activity", |recipe| {
                recipe["planning_order"] = 900.into();
                recipe["projection_order"] = 901.into();
                recipe["dicom"]["artifacts"][0]["output"]["path"] = "images/activity.dcm".into();
            });
            let descriptor_path = root.join("corpus-definition.json");
            let registry_path = root.join("cases/registry.json");
            let original_descriptor: serde_json::Value =
                serde_json::from_slice(&fs::read(&descriptor_path).unwrap()).unwrap();
            let original_registry: serde_json::Value =
                serde_json::from_slice(&fs::read(&registry_path).unwrap()).unwrap();
            for wrong in [Some("US"), None] {
                let mut registry = original_registry.clone();
                if let Some(value) = wrong {
                    registry["cases"][0]["modality"] = value.into();
                } else {
                    registry["cases"][0]
                        .as_object_mut()
                        .unwrap()
                        .remove("modality");
                }
                let mut descriptor = original_descriptor.clone();
                rewrite_registry(&root, &registry, &mut descriptor);
                match CorpusDefinitionBundle::load(&root) {
                    Ok(bundle) => assert!(
                        crate::recipes::RecipeCatalog::from_verified_bundle(
                            &bundle,
                            Path::new(".")
                        )
                        .is_err()
                    ),
                    Err(_) => {}
                }
            }
            let mut descriptor = original_descriptor;
            rewrite_registry(&root, &original_registry, &mut descriptor);
            let bundle = CorpusDefinitionBundle::load(&root).unwrap();
            let catalog =
                crate::recipes::RecipeCatalog::from_verified_bundle(&bundle, Path::new("."))
                    .unwrap();
            assert!(catalog.binding_for_case(case_id).is_some());
            let root = root.canonicalize().unwrap();
            let output = root.with_extension("output");
            let sdk = crate::sdk::DicomTestSuite::embedded().unwrap();
            sdk.generate_corpus(crate::sdk::GenerateCorpusRequest::from_file(
                root.join("corpus-definition.json"),
                &root,
                &output,
                crate::sdk::CorpusSelector::CaseIds {
                    profile: "core".into(),
                    include_stress: false,
                    case_ids: vec![case_id.into()],
                },
            ))
            .unwrap();
            let validation = sdk
                .validate(crate::sdk::ValidateRequest::new(&output))
                .unwrap();
            assert!(validation.is_valid(), "{validation:?}");
            assert_eq!(validation.files_checked(), 1);
            let report: serde_json::Value = sdk
                .report(crate::sdk::ReportRequest::new(&output))
                .unwrap()
                .deserialize()
                .unwrap();
            let file = &report["source_manifest"]["files"][0];
            assert_eq!(file["case_id"], case_id);
            assert_eq!(file["dicom"]["modality"], modality);
            assert_eq!(report["coverage_report_schema_version"], "2.0.0");
            if modality == "XC" {
                assert_eq!(file["image"]["rows"], 2);
                assert_eq!(file["image"]["columns"], 2);
                assert_eq!(
                    file["image"]["photometric_interpretation"],
                    if family == "photo-rgb" {
                        "RGB"
                    } else {
                        "PALETTE COLOR"
                    }
                );
                assert!(file.get("expected_xa_projection").is_none());
                assert!(file.get("expected_xrf_projection").is_none());
                assert!(file.get("expected_icc_profile").is_none());
            } else {
                let geometry = &file[if modality == "XA" {
                    "expected_xa_projection"
                } else {
                    "expected_xrf_projection"
                }];
                assert_eq!(
                    geometry["body_part_examined"],
                    if modality == "XA" { "HEART" } else { "ABDOMEN" }
                );
                assert_eq!(
                    geometry["imager_pixel_spacing_mm"],
                    serde_json::json!([0.2, 0.2])
                );
                assert_eq!(geometry["patient_space_geometry_present"], false);
            }
            assert!(output.join("images/activity.dcm").is_file());
            fs::remove_dir_all(output).unwrap();
            fs::remove_dir_all(root).unwrap();
            for field in ["template", "algorithm_provider_id", "classic_projection"] {
                assert_one_case_rejected(
                    &format!("{label}-missing-{field}"),
                    source,
                    case_id,
                    |recipe| {
                        recipe["dicom"]["artifacts"][0]
                            .as_object_mut()
                            .unwrap()
                            .remove(field);
                    },
                );
            }
            assert_one_case_rejected(&format!("{label}-crossed"), source, case_id, |recipe| {
                recipe["dicom"]["artifacts"][0]["algorithm_provider_id"] =
                    "algorithm.classic_ct".into();
            });
        }
    }
    const PET: &str = "classic/pet/rescaled_activity_explicit_le";
    for (label, case_id) in [
        ("pet-caller", "caller/activity/native"),
        ("pet-us-name", "classic/us/mono2_u8_explicit_le"),
        ("pet-mr-name", "classic/mr/multislice_oblique"),
        ("pet-vl-name", "vl/wsi/pyramid_multiresolution"),
    ] {
        let root = one_case_bundle(label, PET, case_id, "caller_activity", |recipe| {
            recipe["planning_order"] = 900.into();
            recipe["projection_order"] = 901.into();
            recipe["dicom"]["artifacts"][0]["output"]["path"] = "images/activity.dcm".into();
        });
        let bundle = CorpusDefinitionBundle::load(&root).unwrap();
        let catalog =
            crate::recipes::RecipeCatalog::from_verified_bundle(&bundle, Path::new(".")).unwrap();
        assert!(catalog.binding_for_case(case_id).is_some());
        let root = root.canonicalize().unwrap();
        let output = root.with_extension("output");
        let sdk = crate::sdk::DicomTestSuite::embedded().unwrap();
        sdk.generate_corpus(crate::sdk::GenerateCorpusRequest::from_file(
            root.join("corpus-definition.json"),
            &root,
            &output,
            crate::sdk::CorpusSelector::CaseIds {
                profile: "core".into(),
                include_stress: false,
                case_ids: vec![case_id.into()],
            },
        ))
        .unwrap();
        let validation = sdk
            .validate(crate::sdk::ValidateRequest::new(&output))
            .unwrap();
        assert!(validation.is_valid(), "{validation:?}");
        assert_eq!(validation.files_checked(), 1);
        let report: serde_json::Value = sdk
            .report(crate::sdk::ReportRequest::new(&output))
            .unwrap()
            .deserialize()
            .unwrap();
        let file = &report["source_manifest"]["files"][0];
        assert_eq!(file["case_id"], case_id);
        assert_eq!(file["dicom"]["modality"], "PT");
        assert_eq!(file["expected_pet_activity"]["rescale_slope"], 2.5);
        assert!(output.join("images/activity.dcm").is_file());
        fs::remove_dir_all(output).unwrap();
        fs::remove_dir_all(root).unwrap();
        for field in ["template", "algorithm_provider_id", "classic_projection"] {
            assert_one_case_rejected(
                &format!("{label}-missing-{field}"),
                PET,
                case_id,
                |recipe| {
                    recipe["dicom"]["artifacts"][0]
                        .as_object_mut()
                        .unwrap()
                        .remove(field);
                },
            );
        }
        assert_one_case_rejected(&format!("{label}-crossed"), PET, case_id, |recipe| {
            recipe["dicom"]["artifacts"][0]["algorithm_provider_id"] =
                "algorithm.classic_ct".into();
        });
    }
    const US: &str = "classic/us/mono2_u8_explicit_le";
    for (label, case_id, recipe_id) in [
        ("stress-prefix-us", "stress/caller", "caller_ultrasound"),
        (
            "stress-exact-us",
            "stress/enhanced-ct/many_frames",
            "caller_ultrasound",
        ),
        (
            "family-collision-0",
            "vl/endoscopic/rgb_explicit_le",
            "caller_ultrasound",
        ),
        (
            "family-collision-1",
            "vl/microscopic/rgb_explicit_le",
            "caller_ultrasound",
        ),
        (
            "family-collision-2",
            "vl/wsi/tiled_full_small",
            "caller_ultrasound",
        ),
        (
            "family-collision-3",
            "vl/wsi/tiled_sparse_small",
            "caller_ultrasound",
        ),
        (
            "family-collision-4",
            "vl/wsi/multiple_optical_paths",
            "caller_ultrasound",
        ),
        (
            "family-collision-5",
            "vl/wsi/pyramid_multiresolution",
            "caller_ultrasound",
        ),
        (
            "family-collision-6",
            "derived/seg/wsi_tile_reference",
            "caller_ultrasound",
        ),
        (
            "family-collision-7",
            "derived/mesh/encapsulated_stl",
            "caller_ultrasound",
        ),
        (
            "family-collision-8",
            "derived/registration/spatial_ct_pair",
            "caller_ultrasound",
        ),
        (
            "family-collision-9",
            "derived/registration/deformable_ct_pair",
            "caller_ultrasound",
        ),
        (
            "family-collision-10",
            "derived/presentation-state/color_softcopy",
            "caller_ultrasound",
        ),
        (
            "family-collision-11",
            "non-image/rt/image_linked",
            "caller_ultrasound",
        ),
        (
            "family-collision-12",
            "derived/sr/comprehensive3d_scoord3d",
            "caller_ultrasound",
        ),
        (
            "arbitrary-us",
            "caller/ultrasound/native",
            "caller_ultrasound",
        ),
        (
            "pet-named-us",
            "classic/pet/rescaled_activity_explicit_le",
            "caller_ultrasound",
        ),
        (
            "vl-named-us",
            "vl/photo/caller-ultrasound",
            "caller_ultrasound",
        ),
        (
            "mr-named-us",
            "classic/mr/caller-ultrasound",
            "mr_multislice_oblique",
        ),
        (
            "runtime-collision-0",
            "classic/sc/mono2_u32_explicit_le",
            "caller_ultrasound",
        ),
        (
            "runtime-collision-1",
            "classic/sc/mono2_u1_native",
            "caller_ultrasound",
        ),
        (
            "runtime-collision-2",
            "vl/photo/rgb_icc_profile_explicit_le",
            "caller_ultrasound",
        ),
        (
            "runtime-collision-3",
            "classic/sc/nonsquare_pixel_spacing",
            "caller_ultrasound",
        ),
        (
            "runtime-collision-4",
            "metadata/sc/private_creator_blocks",
            "caller_ultrasound",
        ),
        (
            "runtime-collision-5",
            "metadata/sc/defined_undefined_sequence_lengths",
            "caller_ultrasound",
        ),
        (
            "runtime-collision-6",
            "metadata/sc/timezone_boundaries",
            "caller_ultrasound",
        ),
        (
            "runtime-collision-7",
            "metadata/sc/utf8_person_name",
            "caller_ultrasound",
        ),
    ] {
        let root = one_case_bundle(label, US, case_id, recipe_id, |recipe| {
            recipe["planning_order"] = 900.into();
            recipe["projection_order"] = 901.into();
            recipe["dicom"]["artifacts"][0]["output"]["path"] = "images/ultrasound.dcm".into();
        });
        let bundle = CorpusDefinitionBundle::load(&root).unwrap();
        let catalog =
            crate::recipes::RecipeCatalog::from_verified_bundle(&bundle, Path::new(".")).unwrap();
        assert!(catalog.binding_for_case(case_id).is_some());
        let root = root.canonicalize().unwrap();
        let output = root.with_extension("output");
        let sdk = crate::sdk::DicomTestSuite::embedded().unwrap();
        sdk.generate_corpus(crate::sdk::GenerateCorpusRequest::from_file(
            root.join("corpus-definition.json"),
            &root,
            &output,
            crate::sdk::CorpusSelector::CaseIds {
                profile: "core".into(),
                include_stress: false,
                case_ids: vec![case_id.into()],
            },
        ))
        .unwrap();
        let validation = sdk
            .validate(crate::sdk::ValidateRequest::new(&output))
            .unwrap();
        assert!(validation.is_valid(), "{validation:?}");
        assert_eq!(validation.files_checked(), 1);
        assert!(output.join("images/ultrasound.dcm").is_file());
        let report = sdk.report(crate::sdk::ReportRequest::new(&output)).unwrap();
        let report: serde_json::Value = report.deserialize().unwrap();
        assert_eq!(report["coverage_report_schema_version"], "2.0.0");
        assert_eq!(report["summary"]["emitted_files"], 1);
        let reported_file = &report["source_manifest"]["files"][0];
        assert_eq!(reported_file["case_id"], case_id);
        assert_eq!(reported_file["dicom"]["modality"], "US");
        for field in [
            "expected_u32_pixels",
            "expected_u1_pixels",
            "expected_icc_profile",
            "expected_nonsquare_spacing",
            "expected_metadata",
        ] {
            assert!(
                reported_file.get(field).is_none(),
                "caller identity must not introduce {field}"
            );
        }

        fs::remove_dir_all(output).unwrap();
        fs::remove_dir_all(root).unwrap();
        for field in ["template", "algorithm_provider_id", "classic_projection"] {
            assert_one_case_rejected(&format!("{label}-missing-{field}"), US, case_id, |recipe| {
                recipe["dicom"]["artifacts"][0]
                    .as_object_mut()
                    .unwrap()
                    .remove(field);
            });
        }
        assert_one_case_rejected(&format!("{label}-crossed"), US, case_id, |recipe| {
            recipe["dicom"]["artifacts"][0]["algorithm_provider_id"] =
                "algorithm.classic_ct".into();
        });
    }
}

#[test]
fn external_metadata3_capability_is_name_independent_and_fail_closed() {
    let sources = [
        "metadata/sc/utf8_person_name",
        "metadata/sc/empty_type2_attributes",
        "metadata/sc/private_creator_blocks",
    ];
    for (index, source) in sources.into_iter().enumerate() {
        for (variant, target) in [
            "caller/metadata/independent",
            "metadata/sc/timezone_boundaries",
        ]
        .into_iter()
        .enumerate()
        {
            let root = one_case_bundle(
                &format!("metadata3-positive-{index}-{variant}"),
                source,
                target,
                "independent_metadata",
                |recipe| {
                    recipe["planning_order"] = 950.into();
                    recipe["projection_order"] = 950.into();
                    recipe["dicom"]["artifacts"][0]["output"]["path"] = "caller/data.dcm".into();
                    if index == 1 {
                        recipe["dicom"]["artifacts"][0]["metadata_sc"]["attributes"]
                            .as_array_mut()
                            .unwrap()
                            .truncate(1);
                    }
                },
            );
            let bundle = CorpusDefinitionBundle::load(&root).unwrap();
            let catalog =
                crate::recipes::RecipeCatalog::from_verified_bundle(&bundle, Path::new("."))
                    .unwrap();
            assert!(catalog.binding_for_case(target).is_some());
            fs::remove_dir_all(root).unwrap();
        }
        for mutation in 0..15 {
            assert_one_case_rejected(
                &format!("metadata3-partial-{index}-{mutation}"),
                source,
                "caller/metadata/invalid",
                |recipe| {
                    let artifact = &mut recipe["dicom"]["artifacts"][0];
                    match mutation {
                        0 => artifact["template"]["template_id"] = "classic/ct".into(),
                        1 => artifact["template"]["template_version"] = "2.0.0".into(),
                        2 => artifact["content"]["provider_id"] = "content.native_pixels".into(),
                        3 => artifact["algorithm_provider_id"] = "algorithm.classic_ct".into(),
                        4 => artifact["encoding"]["sequence_length_policy"] = "defined".into(),
                        5 => artifact["encoding"]["item_length_policy"] = "undefined".into(),
                        6 => artifact["output"]["path"] = "../escape.dcm".into(),
                        7 => {
                            artifact["secondary_capture"]["stored_values"] = serde_json::json!([0])
                        }
                        8 => {
                            artifact["validation_rule_ids"] =
                                serde_json::json!(["validation.sc.pixel"])
                        }
                        9 => artifact
                            .as_object_mut()
                            .unwrap()
                            .remove("metadata_sc")
                            .map(|_| ())
                            .unwrap(),
                        10 => {
                            recipe["validation_rule_ids"] =
                                serde_json::json!(["validation.sc.pixel"])
                        }
                        11 => recipe["plan_provider_id"] = "native.sc_plan".into(),
                        12 => {
                            artifact["classic_projection"] = serde_json::json!({"family":"ct", "expected_capabilities":[], "visual_pattern":"crossed", "include_implementation_version_name":false})
                        }
                        13 => artifact["secondary_capture"]["high_bit"] = 65535.into(),
                        _ => {
                            let extra = artifact.clone();
                            recipe["dicom"]["artifacts"]
                                .as_array_mut()
                                .unwrap()
                                .push(extra);
                        }
                    }
                },
            );
        }
    }
    for mutation in 0..4 {
        assert_one_case_rejected(
            &format!("metadata3-utf8-{mutation}"),
            sources[0],
            "caller/metadata/invalid",
            |recipe| {
                let pn = &mut recipe["dicom"]["artifacts"][0]["metadata_sc"];
                match mutation {
                    0 => pn["native_unicode_round_trip"] = false.into(),
                    1 => pn["specific_character_sets"] = serde_json::json!(["ISO_IR 100"]),
                    2 => {
                        pn["patient_name_raw_hex"] = "41".into();
                        pn["patient_name_raw_sha256"] = crate::sha256_hex(b"A").into();
                    }
                    _ => {
                        pn["patient_name_raw_hex"] =
                            format!("{}0", pn["patient_name_raw_hex"].as_str().unwrap()).into()
                    }
                }
            },
        );
    }
    for mutation in 0..5 {
        assert_one_case_rejected(
            &format!("metadata3-type2-{mutation}"),
            sources[1],
            "caller/metadata/invalid",
            |recipe| {
                let attributes = recipe["dicom"]["artifacts"][0]["metadata_sc"]["attributes"]
                    .as_array_mut()
                    .unwrap();
                match mutation {
                    0 => attributes.clear(),
                    1 => attributes.push(attributes[0].clone()),
                    2 => {
                        attributes[0] = serde_json::json!({"tag":"0008,0018", "keyword":"SOPInstanceUID", "vr":"UI"})
                    }
                    3 => attributes[0]["keyword"] = "PatientID".into(),
                    _ => attributes[0]["vr"] = "LO".into(),
                }
            },
        );
    }
    for (index, source) in [
        "metadata/sc/iso2022_person_name_component_groups",
        "metadata/sc/timezone_boundaries",
        "metadata/sc/long_multivalue_text_numeric_strings",
        "metadata/sc/defined_undefined_sequence_lengths",
    ]
    .into_iter()
    .enumerate()
    {
        let root = one_case_bundle(
            &format!("metadata-legacy-{index}"),
            source,
            source,
            "legacy_metadata",
            |_| {},
        );
        let bundle = CorpusDefinitionBundle::load(&root).unwrap();
        crate::recipes::RecipeCatalog::from_verified_bundle(&bundle, Path::new(".")).unwrap();
        fs::remove_dir_all(root).unwrap();
        assert_one_case_rejected(
            &format!("metadata-not-migrated-{index}"),
            source,
            "caller/metadata/not-qualified",
            |_| {},
        );
    }
}

#[test]
fn external_sc_capability_is_bounded_name_independent_and_fail_closed() {
    let sources = [
        "mono1_u8_explicit_le",
        "mono2_u8_explicit_le",
        "mono2_u16_explicit_le",
        "mono2_i16_explicit_le",
        "mono2_u16_padding_explicit_le",
        "mono2_u16_tiny_1x1_explicit_le",
        "mono2_u16_rect_2x3_explicit_le",
        "mono2_u16_odd_3x3_explicit_le",
        "palette_color_u8_explicit_le",
        "rgb_planar0_explicit_le",
        "rgb_planar1_explicit_le",
        "ybr_full_planar0_explicit_le",
        "ybr_full_422_explicit_le",
    ];
    for (index, suffix) in sources.into_iter().enumerate() {
        let source = format!("classic/sc/{suffix}");
        for (variant, target) in ["caller/sc/independent", "metadata/sc/timezone_boundaries"]
            .into_iter()
            .enumerate()
        {
            let root = one_case_bundle(
                &format!("sc-positive-{index}-{variant}"),
                &source,
                target,
                "caller_sc",
                |recipe| {
                    recipe["planning_order"] = 900.into();
                    recipe["projection_order"] = 900.into();
                    recipe["dicom"]["artifacts"][0]["output"]["path"] =
                        "independent/image.dcm".into();
                },
            );
            let bundle = CorpusDefinitionBundle::load(&root).unwrap();
            let catalog =
                crate::recipes::RecipeCatalog::from_verified_bundle(&bundle, Path::new("."))
                    .unwrap();
            assert!(catalog.binding_for_case(target).is_some());
            fs::remove_dir_all(root).unwrap();
        }
    }
    let source = "classic/sc/mono2_u8_explicit_le";
    for mutation in 0..21 {
        assert_one_case_rejected(
            &format!("sc-crossed-{mutation}"),
            source,
            "caller/sc/invalid",
            |recipe| {
                let artifact = &mut recipe["dicom"]["artifacts"][0];
                match mutation {
                    0 => {
                        artifact["template"]["template_id"] = "classic/secondary-capture/rgb".into()
                    }
                    1 => artifact
                        .as_object_mut()
                        .unwrap()
                        .remove("template")
                        .map(|_| ())
                        .unwrap(),
                    2 => artifact["template"]["template_version"] = "2.0.0".into(),
                    3 => artifact["content"]["provider_id"] = "content.native_pixels".into(),
                    4 => artifact["validation_rule_ids"] = serde_json::json!([]),
                    5 => artifact["secondary_capture"]["high_bit"] = 65535.into(),
                    6 => artifact["encoding"]["transfer_syntax_uid"] = "1.2.840.10008.1.2".into(),
                    7 => artifact["output"]["path"] = "../escape.dcm".into(),
                    8 => artifact["encoding"]["item_length_policy"] = "undefined".into(),
                    9 => artifact["secondary_capture"]["frames"] = 2.into(),
                    10 => artifact["secondary_capture"]["samples_per_pixel"] = 3.into(),
                    11 => {
                        artifact["classic_projection"] = serde_json::json!({"family":"ct", "expected_capabilities":[], "visual_pattern":"crossed", "include_implementation_version_name":false})
                    }
                    12 => {
                        let mut extra = artifact.clone();
                        extra["order"] = 1.into();
                        extra["logical_id"] = "second".into();
                        extra["output"]["path"] = "other/second.dcm".into();
                        recipe["dicom"]["artifacts"]
                            .as_array_mut()
                            .unwrap()
                            .push(extra);
                    }
                    13 => recipe["validation_rule_ids"] = serde_json::json!([]),
                    14 => artifact["projection_rule_ids"] = serde_json::json!([]),
                    15 => {
                        artifact["metadata_sc"] = serde_json::json!({"kind":"empty_type2", "attributes":[{"tag":"0010,0010", "keyword":"PatientName", "vr":"PN"}]})
                    }
                    16 => {
                        artifact["secondary_capture"]["integer_word"] = serde_json::json!({"byte_order":"little", "covers_full_unsigned_range":true})
                    }
                    17 => {
                        artifact["secondary_capture"]["encapsulation_projection"] = serde_json::json!({"offset_origin":"first_fragment_item", "item_header_bytes":8})
                    }
                    18 => {
                        artifact["secondary_capture"]["bit_packing"] = serde_json::json!({"bit_order":"least_significant_bit_first", "frame_boundary_policy":"continuous", "significant_bits":32, "significant_packed_bytes":4, "unused_high_bits":0, "value_field_padding_bytes":0, "frame_start_bit_offsets":[0]})
                    }
                    19 => {
                        artifact["nonsquare_geometry"] = serde_json::json!({"variant_id":"pixel_spacing", "pixel_spacing":["0.6","0.3"], "row_to_column_ratio":2.0, "calibrated":false, "patient_space_geometry_present":false})
                    }
                    _ => {
                        artifact["attribute_operations"] = serde_json::json!([{"operation":"set", "tag":"0010,0020", "vr":"LO", "value":"caller"}])
                    }
                }
            },
        );
    }
    assert_one_case_rejected(
        "sc-malformed-historical-name",
        source,
        "classic/sc/misleading-name",
        |recipe| {
            recipe["dicom"]["artifacts"][0]["content"]["provider_id"] =
                "content.native_pixels".into();
        },
    );
    for (index, (source, rule)) in [
        (
            "classic/sc/palette_color_u8_explicit_le",
            "validation.sc.palette",
        ),
        ("classic/sc/rgb_planar1_explicit_le", "validation.sc.color"),
        (
            "classic/sc/mono2_u16_padding_explicit_le",
            "validation.sc.padding",
        ),
    ]
    .into_iter()
    .enumerate()
    {
        for level in ["recipe", "artifact"] {
            assert_one_case_rejected(
                &format!("sc-rule-{index}-{level}"),
                source,
                "caller/sc/invalid",
                |recipe| {
                    let value = if level == "recipe" {
                        &mut recipe["validation_rule_ids"]
                    } else {
                        &mut recipe["dicom"]["artifacts"][0]["validation_rule_ids"]
                    };
                    value.as_array_mut().unwrap().retain(|id| id != rule);
                },
            );
        }
    }
    for (index, source) in [
        "classic/sc/mono2_u8_rle_lossless",
        "classic/sc/mono2_u8_multiframe_rle_lossless",
        "classic/sc/mono2_u1_native",
        "classic/sc/mono2_u32_explicit_le",
        "classic/sc/nonsquare_pixel_spacing",
        "encapsulation/sc/eot_single_fragment_multiframe",
    ]
    .into_iter()
    .enumerate()
    {
        let root = one_case_bundle(
            &format!("sc-legacy-{index}"),
            source,
            source,
            "historical_sc",
            |_| {},
        );
        let bundle = CorpusDefinitionBundle::load(&root).unwrap();
        crate::recipes::RecipeCatalog::from_verified_bundle(&bundle, Path::new(".")).unwrap();
        fs::remove_dir_all(root).unwrap();
        assert_one_case_rejected(
            &format!("sc-unsupported-{index}"),
            source,
            "caller/sc/not-qualified",
            |_| {},
        );
    }
}

#[test]
fn minimal_bundle_loads_with_stable_exact_byte_identity() {
    let first = CorpusDefinitionBundle::load(fixture()).unwrap();
    let relocated = temp("relocated");
    copy_bundle(&fixture(), &relocated);
    let second = CorpusDefinitionBundle::load(&relocated).unwrap();
    assert_eq!(first.identity(), second.identity());
    assert_eq!(first.identity().file_count, 4);
    assert_eq!(first.manifest().cases.len(), 1);
    assert_eq!(
        first.bytes("cases/recipes/minimal.json").unwrap().len(),
        633
    );
    fs::remove_dir_all(relocated).unwrap();
}

#[test]
fn manifest_bytes_are_identity_bearing() {
    let changed = temp("whitespace");
    copy_bundle(&fixture(), &changed);
    let path = changed.join("corpus-definition.json");
    let mut bytes = fs::read(&path).unwrap();
    bytes.push(b'\n');
    fs::write(&path, bytes).unwrap();
    let original = CorpusDefinitionBundle::load(fixture()).unwrap();
    let modified = CorpusDefinitionBundle::load(&changed).unwrap();
    assert_ne!(
        original.identity().manifest_sha256,
        modified.identity().manifest_sha256
    );
    assert_ne!(
        original.identity().corpus_definition_sha256,
        modified.identity().corpus_definition_sha256
    );
    fs::remove_dir_all(changed).unwrap();
}

#[test]
fn explicit_descriptor_inputs_are_equivalent_relocatable_and_output_free() {
    let parent = temp("explicit-inputs").canonicalize().unwrap();
    let root = parent.join("members");
    copy_bundle(&fixture(), &root);
    let bytes = fs::read(root.join("corpus-definition.json")).unwrap();
    let descriptor = parent.join("selected.json");
    fs::write(&descriptor, &bytes).unwrap();
    let before = walk(&parent);
    let original = CorpusDefinitionBundle::load(&root).unwrap();
    let file = CorpusDefinitionBundle::load_descriptor_file(&descriptor, &root).unwrap();
    let memory = CorpusDefinitionBundle::load_descriptor_bytes(&bytes, &root).unwrap();
    assert_eq!(original.identity(), file.identity());
    assert_eq!(file.identity(), memory.identity());
    assert_eq!(walk(&parent), before, "loading creates no output");
    fs::remove_file(root.join("corpus-definition.json")).unwrap();
    let moved = parent.join("relocated");
    fs::rename(&root, &moved).unwrap();
    assert_eq!(
        original.identity(),
        CorpusDefinitionBundle::load_descriptor_file(&descriptor, &moved)
            .unwrap()
            .identity()
    );
    assert_eq!(
        original.identity(),
        CorpusDefinitionBundle::load_descriptor_bytes(&bytes, &moved)
            .unwrap()
            .identity()
    );
    let retained = memory.bytes("cases/recipes/minimal.json").unwrap().to_vec();
    fs::write(moved.join("cases/recipes/minimal.json"), vec![b'x'; 633]).unwrap();
    for error in [
        CorpusDefinitionBundle::load_descriptor_file(&descriptor, &moved).unwrap_err(),
        CorpusDefinitionBundle::load_descriptor_bytes(&bytes, &moved).unwrap_err(),
    ] {
        assert_eq!(error.code(), "evidence.integrity.failed");
    }
    assert_eq!(
        memory.bytes("cases/recipes/minimal.json").unwrap(),
        retained
    );
    fs::remove_dir_all(parent).unwrap();
}

#[test]
fn explicit_inputs_reject_missing_ambiguous_and_conflicting_locations() {
    let parent = temp("explicit-locations").canonicalize().unwrap();
    let root = parent.join("members");
    copy_bundle(&fixture(), &root);
    let bytes = fs::read(root.join("corpus-definition.json")).unwrap();
    let descriptor = parent.join("selected.json");
    fs::write(&descriptor, &bytes).unwrap();
    assert_eq!(
        CorpusDefinitionBundle::load_descriptor_file(parent.join("missing.json"), &root)
            .unwrap_err()
            .code(),
        "io.read.failed"
    );
    for error in [
        CorpusDefinitionBundle::load_descriptor_bytes(&bytes, parent.join("missing-root"))
            .unwrap_err(),
        CorpusDefinitionBundle::load_descriptor_file(&descriptor, parent.join("missing-root"))
            .unwrap_err(),
    ] {
        assert_eq!(error.code(), "io.read.failed");
    }
    for error in [
        CorpusDefinitionBundle::load_descriptor_bytes(&bytes, "").unwrap_err(),
        CorpusDefinitionBundle::load_descriptor_file("selected.json", &root).unwrap_err(),
        CorpusDefinitionBundle::load_descriptor_bytes(&bytes, parent.join("members/../members"))
            .unwrap_err(),
    ] {
        assert_eq!(error.code(), "resource.document.invalid");
    }
    let empty = parent.join("empty");
    fs::create_dir(&empty).unwrap();
    assert_eq!(
        CorpusDefinitionBundle::load_descriptor_file(&descriptor, &empty)
            .unwrap_err()
            .code(),
        "io.read.failed",
        "never use descriptor siblings as members"
    );
    fs::write(root.join("corpus-definition.json"), b"{}").unwrap();
    for error in [
        CorpusDefinitionBundle::load_descriptor_bytes(&bytes, &root).unwrap_err(),
        CorpusDefinitionBundle::load_descriptor_file(&descriptor, &root).unwrap_err(),
    ] {
        assert!(matches!(error, CorpusDefinitionError::Closure(_)));
    }
    assert_eq!(
        CorpusDefinitionBundle::load(&root).unwrap_err().code(),
        "request.json.invalid"
    );
    fs::remove_dir_all(parent).unwrap();
}

#[test]
fn explicit_descriptor_limits_and_invalid_bytes_fail_closed() {
    let root = temp("explicit-invalid").canonicalize().unwrap();
    copy_bundle(&fixture(), &root);
    let bytes = fs::read(root.join("corpus-definition.json")).unwrap();
    let descriptor = root.join("corpus-definition.json");
    for maximum in [16, u64::MAX] {
        let limits = CorpusDefinitionLimits {
            manifest_bytes: maximum,
            ..CorpusDefinitionLimits::default()
        };
        for error in [
            CorpusDefinitionBundle::load_descriptor_file_with_limits(&descriptor, &root, limits)
                .unwrap_err(),
            CorpusDefinitionBundle::load_descriptor_bytes_with_limits(&bytes, &root, limits)
                .unwrap_err(),
        ] {
            assert_eq!(error.code(), "resource.limit.exceeded");
        }
    }
    fs::remove_file(&descriptor).unwrap();
    let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    value["corpus_definition_bundle_schema_version"] = "99.0.0".into();
    assert_eq!(
        CorpusDefinitionBundle::load_descriptor_bytes(&serde_json::to_vec(&value).unwrap(), &root)
            .unwrap_err()
            .code(),
        "request.version.unsupported"
    );
    value["corpus_definition_bundle_schema_version"] = "1.0.0".into();
    value["registry"]["path"] = "../registry.json".into();
    assert_eq!(
        CorpusDefinitionBundle::load_descriptor_bytes(&serde_json::to_vec(&value).unwrap(), &root)
            .unwrap_err()
            .code(),
        "resource.document.invalid"
    );
    assert_eq!(
        CorpusDefinitionBundle::load_descriptor_bytes(b"{", &root)
            .unwrap_err()
            .code(),
        "request.json.invalid"
    );
    let oversized = vec![b' '; CorpusDefinitionLimits::default().manifest_bytes as usize + 1];
    assert_eq!(
        CorpusDefinitionBundle::load_descriptor_bytes(&oversized, &root)
            .unwrap_err()
            .code(),
        "resource.limit.exceeded"
    );
    fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
#[test]
fn explicit_input_ancestors_and_members_cannot_be_symlinks() {
    use std::os::unix::fs::symlink;
    let parent = temp("explicit-symlinks").canonicalize().unwrap();
    let actual = parent.join("actual");
    let root = actual.join("members");
    copy_bundle(&fixture(), &root);
    let bytes = fs::read(root.join("corpus-definition.json")).unwrap();
    fs::write(actual.join("selected.json"), &bytes).unwrap();
    let anchor = BundleRoot::open(&actual).unwrap();
    let relative = BundleRoot::open_explicit_at(Path::new("members"), &anchor).unwrap();
    assert_eq!(
        relative
            .capture("corpus-definition.json", 1024 * 1024)
            .unwrap(),
        bytes
    );
    symlink(&actual, parent.join("alias")).unwrap();
    for error in [
        CorpusDefinitionBundle::load_descriptor_bytes(&bytes, parent.join("alias/members"))
            .unwrap_err(),
        CorpusDefinitionBundle::load_descriptor_file(parent.join("alias/selected.json"), &root)
            .unwrap_err(),
    ] {
        assert_eq!(error.code(), "resource.document.invalid");
    }
    symlink(actual.join("selected.json"), parent.join("linked.json")).unwrap();
    assert_eq!(
        CorpusDefinitionBundle::load_descriptor_file(parent.join("linked.json"), &root)
            .unwrap_err()
            .code(),
        "resource.document.invalid"
    );
    let recipe = root.join("cases/recipes/minimal.json");
    fs::rename(&recipe, actual.join("recipe.json")).unwrap();
    symlink(actual.join("recipe.json"), &recipe).unwrap();
    assert_eq!(
        CorpusDefinitionBundle::load_descriptor_bytes(&bytes, &root)
            .unwrap_err()
            .code(),
        "resource.document.invalid"
    );
    fs::remove_dir_all(parent).unwrap();
}

#[test]
fn malformed_duplicate_unknown_bom_and_utf8_are_rejected() {
    for (name, bytes) in [
        ("duplicate", br#"{"corpus_definition_bundle_schema_version":"1.0.0","corpus_definition_bundle_schema_version":"1.0.0"}"#.to_vec()),
        ("escaped-duplicate", br#"{"a":1,"\u0061":2}"#.to_vec()),
        ("unknown", br#"{"unknown":true}"#.to_vec()),
        ("bom", [vec![0xef,0xbb,0xbf], b"{}".to_vec()].concat()),
        ("utf8", vec![0xff]),
    ] {
        let root = temp(name); fs::write(root.join("corpus-definition.json"), bytes).unwrap();
        let error = CorpusDefinitionBundle::load(&root).unwrap_err();
        assert_eq!(error.code(), "request.json.invalid", "{name}: {error}");
        fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn undeclared_hash_size_version_and_limits_fail_closed() {
    let extra = temp("extra");
    copy_bundle(&fixture(), &extra);
    fs::write(extra.join("extra.txt"), b"x").unwrap();
    assert!(matches!(
        CorpusDefinitionBundle::load(&extra),
        Err(CorpusDefinitionError::Closure(_))
    ));
    fs::remove_dir_all(extra).unwrap();

    let limited = CorpusDefinitionLimits {
        manifest_bytes: 16,
        ..CorpusDefinitionLimits::default()
    };
    assert_eq!(
        CorpusDefinitionBundle::load_with_limits(fixture(), limited)
            .unwrap_err()
            .code(),
        "resource.limit.exceeded"
    );

    let bad = temp("hash");
    copy_bundle(&fixture(), &bad);
    let recipe = bad.join("cases/recipes/minimal.json");
    fs::write(&recipe, vec![b'x'; 633]).unwrap();
    assert_eq!(
        CorpusDefinitionBundle::load(&bad).unwrap_err().code(),
        "evidence.integrity.failed"
    );
    fs::remove_dir_all(bad).unwrap();
}

#[cfg(unix)]
#[test]
fn root_intermediate_and_file_symlinks_are_rejected() {
    use std::os::unix::fs::symlink;
    let parent = temp("links");
    let root_link = parent.join("root-link");
    symlink(fixture().canonicalize().unwrap(), &root_link).unwrap();
    assert!(matches!(
        CorpusDefinitionBundle::load(&root_link),
        Err(CorpusDefinitionError::Symlink(_))
    ));
    fs::remove_file(&root_link).unwrap();

    let bundle = parent.join("bundle");
    copy_bundle(&fixture(), &bundle);
    fs::remove_file(bundle.join("cases/recipes/minimal.json")).unwrap();
    symlink(
        fixture()
            .canonicalize()
            .unwrap()
            .join("cases/recipes/minimal.json"),
        bundle.join("cases/recipes/minimal.json"),
    )
    .unwrap();
    assert!(matches!(
        CorpusDefinitionBundle::load(&bundle),
        Err(CorpusDefinitionError::Symlink(_) | CorpusDefinitionError::Read { .. })
    ));
    fs::remove_dir_all(&bundle).unwrap();
    copy_bundle(&fixture(), &bundle);
    fs::remove_dir_all(bundle.join("cases/recipes")).unwrap();
    symlink(
        fixture().canonicalize().unwrap().join("cases/recipes"),
        bundle.join("cases/recipes"),
    )
    .unwrap();
    assert!(matches!(
        CorpusDefinitionBundle::load(&bundle),
        Err(CorpusDefinitionError::Symlink(_) | CorpusDefinitionError::Read { .. })
    ));
    fs::remove_dir_all(parent).unwrap();
}

#[test]
fn unsupported_version_has_stable_classification() {
    let root = temp("version");
    copy_bundle(&fixture(), &root);
    let path = root.join("corpus-definition.json");
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    manifest["corpus_definition_bundle_schema_version"] = "2.0.0".into();
    fs::write(&path, serde_json::to_vec(&manifest).unwrap()).unwrap();
    assert_eq!(
        CorpusDefinitionBundle::load(&root).unwrap_err().code(),
        "request.version.unsupported"
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn traversal_and_casefold_collisions_fail_before_file_access() {
    let traversal = temp("traversal");
    copy_bundle(&fixture(), &traversal);
    let path = traversal.join("corpus-definition.json");
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    manifest["registry"]["path"] = "../registry.json".into();
    fs::write(&path, serde_json::to_vec(&manifest).unwrap()).unwrap();
    assert_eq!(
        CorpusDefinitionBundle::load(&traversal).unwrap_err().code(),
        "resource.document.invalid"
    );
    fs::remove_dir_all(traversal).unwrap();

    let collision = temp("casefold");
    copy_bundle(&fixture(), &collision);
    let path = collision.join("corpus-definition.json");
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    manifest["assets"] = serde_json::json!([{"asset_id":"collision","media_type":"application/octet-stream","path":"EVIDENCE/minimal.md","size_bytes":27,"sha256":"6b83b8a0b422cc293b5ba2ff63042a09b60ad715a4c01799db97c0ef09efcf9f"}]);
    fs::write(&path, serde_json::to_vec(&manifest).unwrap()).unwrap();
    assert_eq!(
        CorpusDefinitionBundle::load(&collision).unwrap_err().code(),
        "resource.document.invalid"
    );
    fs::remove_dir_all(collision).unwrap();
}

#[test]
fn declared_size_mismatch_is_integrity_failure() {
    let root = temp("size");
    copy_bundle(&fixture(), &root);
    let path = root.join("corpus-definition.json");
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    manifest["cases"][0]["recipe"]["size_bytes"] = 632.into();
    fs::write(&path, serde_json::to_vec(&manifest).unwrap()).unwrap();
    assert_eq!(
        CorpusDefinitionBundle::load(&root).unwrap_err().code(),
        "evidence.integrity.failed"
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn reserved_engine_namespaces_and_unexpected_directories_are_rejected() {
    let reserved = temp("reserved");
    copy_bundle(&fixture(), &reserved);
    let path = reserved.join("corpus-definition.json");
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    manifest["assets"] = serde_json::json!([{"asset_id":"override","media_type":"application/json","path":"templates/override.json","size_bytes":1,"sha256":"2d711642b726b04401627ca9fbac32f5c8530fb1903cc4db02258717921a4881"}]);
    fs::write(&path, serde_json::to_vec(&manifest).unwrap()).unwrap();
    assert_eq!(
        CorpusDefinitionBundle::load(&reserved).unwrap_err().code(),
        "resource.document.invalid"
    );
    fs::remove_dir_all(reserved).unwrap();

    let directory = temp("extra-directory");
    copy_bundle(&fixture(), &directory);
    fs::create_dir(directory.join("unexpected-empty")).unwrap();
    assert!(matches!(
        CorpusDefinitionBundle::load(&directory),
        Err(CorpusDefinitionError::Closure(_))
    ));
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn closure_rejects_binding_dependency_and_profile_inconsistency() {
    let root = temp("closure");
    copy_bundle(&fixture(), &root);
    let manifest_path = root.join("corpus-definition.json");
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
    manifest["cases"][0]["case_id"] = "classic/sc/other".into();
    fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();
    assert!(matches!(
        CorpusDefinitionBundle::load(&root),
        Err(CorpusDefinitionError::Closure(_))
    ));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn deterministic_current_source_assembly_loads_all_registry_cases() {
    let root = temp("current");
    fs::remove_dir(&root).unwrap();
    let status = Command::new("python3")
        .arg("scripts/build-current-corpus-definition-bundle.py")
        .arg(&root)
        .status()
        .unwrap();
    assert!(status.success());
    let bundle = CorpusDefinitionBundle::load(&root).unwrap();
    let explicit_root = root.canonicalize().unwrap();
    let descriptor = explicit_root.join("corpus-definition.json");
    assert_eq!(
        bundle.identity(),
        CorpusDefinitionBundle::load_descriptor_file(&descriptor, &explicit_root)
            .unwrap()
            .identity()
    );
    assert_eq!(
        bundle.identity(),
        CorpusDefinitionBundle::load_descriptor_bytes(
            &fs::read(&descriptor).unwrap(),
            &explicit_root
        )
        .unwrap()
        .identity()
    );
    assert_eq!(bundle.identity().file_count, 214);
    assert_eq!(bundle.identity().total_size_bytes, 1_754_298);
    eprintln!(
        "current corpus definition identity: {:?}",
        bundle.identity()
    );
    assert_eq!(
        bundle.identity().manifest_sha256,
        "905d36bc93c7ae10ae5011304b25a647c4b792852e143bd2017e2aacd1574de8"
    );
    assert_eq!(
        bundle.identity().corpus_definition_sha256,
        "571fa23fd392dd557ccdbe2db527698eaedc7078d86543efc68dfffc877411f7"
    );
    let registry: serde_json::Value =
        serde_json::from_slice(bundle.bytes("cases/registry.json").unwrap()).unwrap();
    assert_eq!(registry["cases"].as_array().unwrap().len(), 191);
    let local_note_paths = registry["cases"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|case| case["standards_evidence"].as_array().unwrap())
        .filter(|record| record["source"] == "local-source-note")
        .map(|record| record["query"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(local_note_paths.len(), 45);
    assert_eq!(
        local_note_paths
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        34
    );
    assert_eq!(bundle.manifest().cases.len(), 178);
    assert_eq!(bundle.manifest().evidence.len(), 34);
    assert_eq!(bundle.manifest().assets.len(), 0);
    assert_eq!(bundle.manifest().profiles.len(), 8);
    assert_eq!(bundle.identity().file_count, 214);
    let profile_by_case = registry["cases"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| {
            (
                row["case_id"].as_str().unwrap(),
                row["profiles"].as_array().unwrap(),
            )
        })
        .collect::<std::collections::BTreeMap<_, _>>();
    let invalid_to_ordinary_dependencies = bundle
        .manifest()
        .cases
        .iter()
        .filter(|case| {
            let owner = profile_by_case[case.case_id.as_str()];
            owner
                .iter()
                .any(|value| matches!(value.as_str(), Some("negative" | "fuzz")))
                && case.dependencies.iter().any(|dependency| {
                    profile_by_case[dependency.as_str()]
                        .iter()
                        .any(|value| matches!(value.as_str(), Some("smoke" | "core" | "extended")))
                })
        })
        .count();
    assert_eq!(invalid_to_ordinary_dependencies, 16);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn json_depth_array_and_string_limits_have_limit_classification() {
    for (name, limits) in [
        (
            "depth",
            CorpusDefinitionLimits {
                json_depth: 1,
                ..CorpusDefinitionLimits::default()
            },
        ),
        (
            "array",
            CorpusDefinitionLimits {
                json_array_entries: 1,
                ..CorpusDefinitionLimits::default()
            },
        ),
        (
            "string",
            CorpusDefinitionLimits {
                json_string_bytes: 4,
                ..CorpusDefinitionLimits::default()
            },
        ),
    ] {
        let error = CorpusDefinitionBundle::load_with_limits(fixture(), limits).unwrap_err();
        assert_eq!(error.code(), "resource.limit.exceeded", "{name}: {error}");
    }
}

#[test]
fn document_and_aggregate_limits_are_enforced() {
    let document = CorpusDefinitionLimits {
        document_bytes: 100,
        ..CorpusDefinitionLimits::default()
    };
    assert_eq!(
        CorpusDefinitionBundle::load_with_limits(fixture(), document)
            .unwrap_err()
            .code(),
        "resource.limit.exceeded"
    );
    let aggregate = CorpusDefinitionLimits {
        total_document_bytes: 1200,
        ..CorpusDefinitionLimits::default()
    };
    assert_eq!(
        CorpusDefinitionBundle::load_with_limits(fixture(), aggregate)
            .unwrap_err()
            .code(),
        "resource.limit.exceeded"
    );
}

#[cfg(unix)]
#[test]
fn held_root_descriptor_cannot_switch_to_a_replacement_tree() {
    let parent = temp("root-replacement");
    let root = parent.join("bundle");
    copy_bundle(&fixture(), &root);
    let held = BundleRoot::open(&root).unwrap();
    let moved = parent.join("moved");
    fs::rename(&root, &moved).unwrap();
    fs::create_dir(&root).unwrap();
    fs::write(root.join("corpus-definition.json"), b"replacement").unwrap();
    let bytes = held.capture("corpus-definition.json", 1024 * 1024).unwrap();
    assert!(bytes.starts_with(b"{"));
    assert_ne!(bytes, b"replacement");
    fs::remove_dir_all(parent).unwrap();
}

#[cfg(unix)]
#[test]
fn hardlinks_fifo_and_nonregular_roots_are_rejected() {
    let parent = temp("nonregular");
    let file_root = parent.join("file-root");
    fs::write(&file_root, b"x").unwrap();
    assert!(matches!(
        CorpusDefinitionBundle::load(&file_root),
        Err(CorpusDefinitionError::NotRegular(_))
    ));

    let hardlink = parent.join("hardlink");
    copy_bundle(&fixture(), &hardlink);
    let recipe = hardlink.join("cases/recipes/minimal.json");
    fs::hard_link(&recipe, hardlink.join("cases/recipes/alias.json")).unwrap();
    assert!(matches!(
        CorpusDefinitionBundle::load(&hardlink),
        Err(CorpusDefinitionError::NotRegular(_))
    ));

    let fifo = parent.join("fifo");
    copy_bundle(&fixture(), &fifo);
    let recipe = fifo.join("cases/recipes/minimal.json");
    fs::remove_file(&recipe).unwrap();
    assert!(
        Command::new("mkfifo")
            .arg(&recipe)
            .status()
            .unwrap()
            .success()
    );
    assert!(matches!(
        CorpusDefinitionBundle::load(&fifo),
        Err(CorpusDefinitionError::NotRegular(_))
    ));
    fs::remove_dir_all(parent).unwrap();
}

fn assert_dependency_scope_rejected(scope: &str) {
    let root = temp(&format!("scope-leakage-{scope}"));
    fs::remove_dir(&root).unwrap();
    assert!(
        Command::new("python3")
            .arg("scripts/build-current-corpus-definition-bundle.py")
            .arg(&root)
            .status()
            .unwrap()
            .success()
    );
    let manifest_path = root.join("corpus-definition.json");
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
    let cases = manifest["cases"].as_array().unwrap();
    let owner = cases
        .iter()
        .find(|case| !case["dependencies"].as_array().unwrap().is_empty())
        .unwrap();
    let dependency = owner["dependencies"][0].as_str().unwrap().to_string();
    let mut registry: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("cases/registry.json")).unwrap()).unwrap();
    let row = registry["cases"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|row| row["case_id"] == dependency)
        .unwrap();
    row["profiles"] = serde_json::json!([scope]);
    for profile in manifest["profiles"].as_array_mut().unwrap() {
        if profile["profile_id"] == "all" {
            continue;
        }
        let is_target = profile["profile_id"] == scope;
        let members = profile["members"].as_array_mut().unwrap();
        members.retain(|member| member.as_str() != Some(&dependency));
        if is_target {
            members.push(dependency.clone().into());
            members.sort_by_key(|value| value.as_str().unwrap().to_string());
        }
    }
    rewrite_registry(&root, &registry, &mut manifest);
    let error = CorpusDefinitionBundle::load(&root).unwrap_err();
    assert!(
        matches!(&error, CorpusDefinitionError::Closure(message) if message.contains("dependency scope leakage")),
        "{error}"
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn ordinary_dependency_cannot_cross_into_negative_scope() {
    assert_dependency_scope_rejected("negative");
}

#[test]
fn ordinary_dependency_cannot_cross_into_legacy_or_stress_scope() {
    assert_dependency_scope_rejected("legacy");
    assert_dependency_scope_rejected("stress");
}

fn assert_invalid_dependency_scope_rejected(owner_scope: &str, dependency_scope: &str) {
    let root = temp(&format!(
        "{owner_scope}-dependency-cannot-enter-{dependency_scope}"
    ));
    fs::remove_dir(&root).unwrap();
    assert!(
        Command::new("python3")
            .arg("scripts/build-current-corpus-definition-bundle.py")
            .arg(&root)
            .status()
            .unwrap()
            .success()
    );
    let manifest_path = root.join("corpus-definition.json");
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
    let mut registry: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("cases/registry.json")).unwrap()).unwrap();
    let owner_ids = registry["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| {
            row["profiles"]
                .as_array()
                .unwrap()
                .iter()
                .any(|profile| profile == owner_scope)
        })
        .map(|row| row["case_id"].as_str().unwrap().to_string())
        .collect::<std::collections::BTreeSet<_>>();
    let owner = manifest["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| {
            owner_ids.contains(case["case_id"].as_str().unwrap())
                && !case["dependencies"].as_array().unwrap().is_empty()
        })
        .unwrap();
    let dependency = owner["dependencies"][0].as_str().unwrap().to_string();
    let dependency_row = registry["cases"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|row| row["case_id"] == dependency)
        .unwrap();
    dependency_row["profiles"] = serde_json::json!([dependency_scope]);
    for profile in manifest["profiles"].as_array_mut().unwrap() {
        let is_dependency_scope = profile["profile_id"] == dependency_scope;
        let Some(members) = profile
            .get_mut("members")
            .and_then(serde_json::Value::as_array_mut)
        else {
            continue;
        };
        members.retain(|member| member.as_str() != Some(&dependency));
        if is_dependency_scope {
            members.push(dependency.clone().into());
            members.sort_by_key(|value| value.as_str().unwrap().to_string());
        }
    }
    rewrite_registry(&root, &registry, &mut manifest);
    let error = CorpusDefinitionBundle::load(&root).unwrap_err();
    assert!(
        matches!(&error, CorpusDefinitionError::Closure(message) if message.contains("dependency scope leakage")),
        "{error}"
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn negative_and_fuzz_dependencies_cannot_cross_legacy_or_stress_boundaries() {
    for owner_scope in ["negative", "fuzz"] {
        for dependency_scope in ["legacy", "stress"] {
            assert_invalid_dependency_scope_rejected(owner_scope, dependency_scope);
        }
    }
}

#[test]
fn excessive_undeclared_inventory_fails_without_buffering_all_names() {
    let root = temp("excessive-inventory");
    copy_bundle(&fixture(), &root);
    let extras = root.join("excessive");
    fs::create_dir(&extras).unwrap();
    for index in 0..500 {
        fs::write(extras.join(format!("entry-{index:04}")), b"x").unwrap();
    }
    assert!(matches!(
        CorpusDefinitionBundle::load(&root),
        Err(CorpusDefinitionError::Closure(_))
    ));
    let source = fs::read_to_string("src/corpus_definition/mod.rs").unwrap();
    assert!(source.contains("let mut entry_count = 0_usize"));
    assert!(source.contains("std::io::Error::from_raw_os_error(errno)"));
    assert!(!source.contains("let mut names = Vec::new()"));
    fs::remove_dir_all(root).unwrap();
}
