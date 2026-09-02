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
        .map(|row| (row["case_id"].as_str().unwrap(), row["profiles"].as_array().unwrap()))
        .collect::<std::collections::BTreeMap<_, _>>();
    let invalid_to_ordinary_dependencies = bundle
        .manifest()
        .cases
        .iter()
        .filter(|case| {
            let owner = profile_by_case[case.case_id.as_str()];
            owner.iter().any(|value| matches!(value.as_str(), Some("negative" | "fuzz")))
                && case.dependencies.iter().any(|dependency| {
                    profile_by_case[dependency.as_str()].iter().any(|value| {
                        matches!(value.as_str(), Some("smoke" | "core" | "extended"))
                    })
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
