use super::*;
use crate::corpus_definition::CorpusDefinitionBundle;
use crate::engine_resources::EngineResources;
use std::process::Command;

fn temporary(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "synth-dicom-gen-captured-plan-{}-{name}",
        std::process::id()
    ))
}

fn full(name: &str) -> (PathBuf, CorpusDefinitionBundle) {
    let root = temporary(name);
    assert!(!root.exists());
    assert!(
        Command::new("python3")
            .arg("scripts/build-current-corpus-definition-bundle.py")
            .arg(&root)
            .status()
            .unwrap()
            .success()
    );
    let bundle = CorpusDefinitionBundle::load(&root).unwrap();
    (root, bundle)
}

fn request(selection: CuratedScSelection) -> CuratedScPlanRequest {
    CuratedScPlanRequest {
        selection,
        seed: 1,
        max_parallelism: 3,
    }
}

fn smoke() -> CuratedScSelection {
    CuratedScSelection::Profile {
        profile: "smoke".into(),
        include_stress: false,
    }
}

fn write(root: &Path, path: &str, bytes: &[u8]) {
    fs::create_dir_all(root.join(path).parent().unwrap()).unwrap();
    fs::write(root.join(path), bytes).unwrap();
}

fn smoke_subset(full: &CorpusDefinitionBundle, root: &Path) {
    let mut manifest = serde_json::to_value(full.manifest()).unwrap();
    let mut registry: Value =
        serde_json::from_slice(full.bytes(&full.manifest().registry.path).unwrap()).unwrap();
    registry["cases"].as_array_mut().unwrap().retain(|case| {
        case["profiles"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p == "smoke")
    });
    let ids = registry["cases"]
        .as_array()
        .unwrap()
        .iter()
        .map(|case| case["case_id"].as_str().unwrap().to_owned())
        .collect::<BTreeSet<_>>();
    manifest["cases"]
        .as_array_mut()
        .unwrap()
        .retain(|case| ids.contains(case["case_id"].as_str().unwrap()));
    for profile in manifest["profiles"].as_array_mut().unwrap() {
        if let Some(members) = profile.get_mut("members").and_then(Value::as_array_mut) {
            members.retain(|id| ids.contains(id.as_str().unwrap()));
        }
    }
    let evidence_ids = manifest["cases"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|case| case["evidence_ids"].as_array().unwrap())
        .map(|id| id.as_str().unwrap().to_owned())
        .collect::<BTreeSet<_>>();
    manifest["evidence"]
        .as_array_mut()
        .unwrap()
        .retain(|entry| evidence_ids.contains(entry["evidence_id"].as_str().unwrap()));
    for case in manifest["cases"].as_array().unwrap() {
        let path = case["recipe"]["path"].as_str().unwrap();
        write(root, path, full.bytes(path).unwrap());
    }
    for entry in manifest["evidence"].as_array().unwrap() {
        let path = entry["path"].as_str().unwrap();
        write(root, path, full.bytes(path).unwrap());
    }
    let registry_bytes = serde_json::to_vec(&registry).unwrap();
    manifest["registry"]["size_bytes"] = registry_bytes.len().into();
    manifest["registry"]["sha256"] = sha256_hex(&registry_bytes).into();
    write(root, "cases/registry.json", &registry_bytes);
    write(
        root,
        "corpus-definition.json",
        &serde_json::to_vec(&manifest).unwrap(),
    );
}

#[test]
fn full_capture_matches_legacy_plans_after_source_removal_and_owns_lease() {
    let (root, bundle) = full("parity");
    let resources = EngineResources::embedded();
    let legacy =
        CuratedScCorpusPlanProvider::load(CuratedCatalogPaths::from_repository_root(".")).unwrap();
    let context =
        CapturedCuratedPlanningContext::from_verified_bundle(&bundle, &resources).unwrap();
    fs::remove_dir_all(&root).unwrap();
    for selection in [
        smoke(),
        CuratedScSelection::CaseIds(vec!["derived/registration/spatial_ct_pair".into()]),
    ] {
        let request = request(selection);
        let old = legacy.plan(&request).unwrap();
        let new = context.plan(request).unwrap();
        assert_eq!(
            serde_json::to_value(&old).unwrap(),
            serde_json::to_value(&new.planned).unwrap()
        );
        assert_eq!(new.corpus_identity, *bundle.identity());
        assert_eq!(
            new.planned.external_provider_repository_root,
            new.engine_lease.root()
        );
        assert!(
            new.planned
                .external_provider_standards_lock_path
                .starts_with(new.engine_lease.root())
        );
    }
    let held = context.plan(request(smoke())).unwrap();
    let engine_root = held.engine_lease.root().to_path_buf();
    drop(context);
    drop(resources);
    assert!(engine_root.join("standards.lock.json").is_file());
    drop(held);
    assert!(!engine_root.exists());
}

#[test]
fn smoke_subset_uses_installed_templates_without_unrelated_default_recipes() {
    let (full_root, full_bundle) = full("subset-source");
    let root = temporary("subset");
    smoke_subset(&full_bundle, &root);
    let bundle = CorpusDefinitionBundle::load(&root).unwrap();
    assert_eq!(bundle.manifest().cases.len(), 3);
    let resources = EngineResources::embedded();
    let full = CapturedCuratedPlanningContext::from_verified_bundle(&full_bundle, &resources)
        .unwrap()
        .plan(request(smoke()))
        .unwrap();
    let context =
        CapturedCuratedPlanningContext::from_verified_bundle(&bundle, &resources).unwrap();
    let subset = context.plan(request(smoke())).unwrap();
    assert_eq!(subset.planned.plan.artifacts, full.planned.plan.artifacts);
    assert_eq!(subset.planned.bindings, full.planned.bindings);
    assert!(
        RecipeCatalog::load(
            root.join("cases/recipes"),
            root.join("cases/registry.json"),
            resources
                .shared_snapshot()
                .unwrap()
                .root()
                .join("templates/catalog.json")
        )
        .is_err(),
        "legacy default closure remains enforced"
    );
    fs::remove_dir_all(root).unwrap();
    fs::remove_dir_all(full_root).unwrap();
}

#[test]
fn captured_metadata_isolated_and_external_provider_remains_unavailable() {
    let (root, bundle) = full("metadata");
    let resources = EngineResources::embedded();
    let original =
        CapturedCuratedPlanningContext::from_verified_bundle(&bundle, &resources).unwrap();
    let mut manifest = serde_json::to_value(bundle.manifest()).unwrap();
    manifest["definition_version"] = "1.0.1".into();
    write(
        &root,
        "corpus-definition.json",
        &serde_json::to_vec(&manifest).unwrap(),
    );
    let changed = CorpusDefinitionBundle::load(&root).unwrap();
    let modified =
        CapturedCuratedPlanningContext::from_verified_bundle(&changed, &resources).unwrap();
    assert_ne!(original.corpus_identity, modified.corpus_identity);
    assert_eq!(
        original.provider.installed_codec_matrix,
        modified.provider.installed_codec_matrix
    );
    assert_eq!(
        serde_json::to_value(original.plan(request(smoke())).unwrap().planned).unwrap(),
        serde_json::to_value(modified.plan(request(smoke())).unwrap().planned).unwrap()
    );
    let selection = CuratedScSelection::CaseIds(vec![
        "derived/parametric-map/float32_ct_derived_explicit_le".into(),
    ]);
    let current = original.plan(request(selection.clone())).unwrap();
    let legacy = CuratedScCorpusPlanProvider::load(CuratedCatalogPaths::from_repository_root("."))
        .unwrap()
        .plan(&request(selection))
        .unwrap();
    assert_eq!(
        serde_json::to_value(&current.planned).unwrap(),
        serde_json::to_value(legacy).unwrap()
    );
    assert!(!current.planned.plan.unavailable.is_empty());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn captured_constructor_rejects_incompatible_templates_and_unclosed_members() {
    let (full_root, full_bundle) = full("invalid-source");
    let root = temporary("invalid-subset");
    smoke_subset(&full_bundle, &root);
    let mut manifest: Value =
        serde_json::from_slice(&fs::read(root.join("corpus-definition.json")).unwrap()).unwrap();
    let path = manifest["cases"][0]["recipe"]["path"]
        .as_str()
        .unwrap()
        .to_owned();
    let mut recipe: Value = serde_json::from_slice(&fs::read(root.join(&path)).unwrap()).unwrap();
    recipe["dicom"]["artifacts"][0]["template"]["template_id"] =
        "missing/installed-template".into();
    let bytes = serde_json::to_vec(&recipe).unwrap();
    write(&root, &path, &bytes);
    manifest["cases"][0]["recipe"]["size_bytes"] = bytes.len().into();
    manifest["cases"][0]["recipe"]["sha256"] = sha256_hex(&bytes).into();
    write(
        &root,
        "corpus-definition.json",
        &serde_json::to_vec(&manifest).unwrap(),
    );
    let captured = CorpusDefinitionBundle::load(&root).unwrap();
    let error = CapturedCuratedPlanningContext::from_verified_bundle(
        &captured,
        &EngineResources::embedded(),
    )
    .err()
    .expect("unknown installed template rejected");
    assert!(error.to_string().contains("template"));
    manifest["cases"][0]["dependencies"] = serde_json::json!(["absent/source/case"]);
    write(
        &root,
        "corpus-definition.json",
        &serde_json::to_vec(&manifest).unwrap(),
    );
    assert!(CorpusDefinitionBundle::load(&root).is_err());
    manifest["cases"][0]["dependencies"] = serde_json::json!([]);
    manifest["registry"]["path"] = "transfer-syntax/capability-matrix.json".into();
    write(
        &root,
        "corpus-definition.json",
        &serde_json::to_vec(&manifest).unwrap(),
    );
    assert_eq!(
        CorpusDefinitionBundle::load(&root).unwrap_err().code(),
        "resource.document.invalid"
    );
    fs::remove_dir_all(root).unwrap();
    fs::remove_dir_all(full_root).unwrap();
}
