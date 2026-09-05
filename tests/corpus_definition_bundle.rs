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
            "evidence_ids": [],
            "asset_ids": []
        }],
        "evidence": [],
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
        DX_CASE,
        "caller/arbitrary/not-ct",
        |_| {},
    );
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
