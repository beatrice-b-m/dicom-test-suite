use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use synth_dicom_gen::{CorpusDefinitionBundle, CorpusDefinitionError, CorpusDefinitionLimits};

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
    fs::remove_dir_all(root).unwrap();
}
