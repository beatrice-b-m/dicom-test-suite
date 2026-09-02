use std::fs;

use synth_dicom_gen::engine_resources::{
    ENGINE_RESOURCE_SET_MEMBERSHIP, ENGINE_RESOURCE_SET_VERSION, EngineResourceError,
    EngineResourceOrigin, EngineResourceSetMembership, EngineResources,
};

#[test]
fn embedded_resources_cover_engine_families_and_transitional_membership() {
    let resources = EngineResources::embedded();
    for path in [
        "cases/registry.json",
        "cases/recipes/classic/sc/sc_mono2_u8.json",
        "cases/recipes/classic/sc/classic_sc_mono2_u16_htj2k_lossy.json",
        "templates/catalog.json",
        "templates/inventory.json",
        "templates/qualification-evidence.json",
        "schemas/manifest.schema.json",
        "standards.lock.json",
        "generation-backends.lock.json",
        "transfer-syntax/capability-matrix.json",
        "conformance/validators.json",
        "security/fixtures/fixtures.lock.json",
        "assets/dcmtk_srgb_input_profile.hex",
        "product/cli-error-codes.json",
    ] {
        assert!(resources.contains(path), "missing embedded resource {path}");
        assert!(
            !resources.bytes(path).unwrap().is_empty(),
            "empty resource {path}"
        );
    }
    assert_eq!(
        ENGINE_RESOURCE_SET_MEMBERSHIP,
        EngineResourceSetMembership::TransitionalMonolithic
    );
    assert!(resources.contains("cases/registry.json"));
    assert!(resources.contains("Cargo.lock"));
}

#[test]
fn resource_build_tracks_directory_additions() {
    let build = fs::read_to_string("build.rs").unwrap();
    assert!(build.contains("cargo:rerun-if-changed={}"));
    assert!(build.contains("directory.display()"));
    assert!(build.contains("embedded_engine_resources.rs"));
    assert!(build.contains("symlink_metadata"));
    assert!(build.contains("require_regular_engine_resource"));
}

#[test]
fn embedded_and_relocated_explicit_resources_have_identical_identity() {
    let resources = EngineResources::embedded();
    let root = std::env::temp_dir().join(format!(
        "synth-dicom-gen-engine-resource-test-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&root).unwrap();
    for logical_path in resources.logical_paths() {
        let path = root.join(logical_path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, resources.bytes(logical_path).unwrap()).unwrap();
    }

    let embedded = resources.identity().unwrap();
    let explicit = EngineResources::explicit(&root)
        .unwrap()
        .verify_integrity()
        .unwrap();
    assert_eq!(embedded.resource_set_version, ENGINE_RESOURCE_SET_VERSION);
    assert_eq!(embedded.origin, EngineResourceOrigin::Embedded);
    assert_eq!(explicit.origin, EngineResourceOrigin::Explicit);
    assert_eq!(embedded.resource_count, explicit.resource_count);
    assert_eq!(embedded.resource_set_sha256, explicit.resource_set_sha256);
    assert_eq!(embedded.resources, explicit.resources);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn tampered_or_symlinked_explicit_resources_fail_before_materialization() {
    let embedded = EngineResources::embedded();
    let snapshot = embedded.snapshot().unwrap();
    let registry_path = snapshot.root().join("cases/registry.json");
    let mut registry = fs::read(&registry_path).unwrap();
    registry.push(b'\n');
    fs::write(&registry_path, registry).unwrap();

    let error = EngineResources::explicit(snapshot.root()).unwrap_err();
    assert!(matches!(error, EngineResourceError::Integrity { .. }));
    assert_eq!(error.code(), "evidence.integrity.failed");

    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;

        let file_snapshot = embedded.snapshot().unwrap();
        let catalog = file_snapshot.root().join("templates/catalog.json");
        let catalog_target = file_snapshot.root().join("catalog-target.json");
        fs::rename(&catalog, &catalog_target).unwrap();
        symlink(&catalog_target, &catalog).unwrap();
        let error = EngineResources::explicit(file_snapshot.root()).unwrap_err();
        assert!(matches!(error, EngineResourceError::Symlink { .. }));
        assert_eq!(error.code(), "resource.document.invalid");

        let directory_snapshot = embedded.snapshot().unwrap();
        let templates = directory_snapshot.root().join("templates");
        let templates_target = directory_snapshot.root().join("templates-target");
        fs::rename(&templates, &templates_target).unwrap();
        symlink(&templates_target, &templates).unwrap();
        let error = EngineResources::explicit(directory_snapshot.root()).unwrap_err();
        assert!(matches!(error, EngineResourceError::Symlink { .. }));

        let root_snapshot = embedded.snapshot().unwrap();
        let root_link = root_snapshot.root().with_extension("symlink");
        symlink(root_snapshot.root(), &root_link).unwrap();
        let error = EngineResources::explicit(&root_link).unwrap_err();
        assert!(matches!(error, EngineResourceError::Symlink { .. }));
        fs::remove_file(root_link).unwrap();
    }
}

#[test]
fn embedded_bytes_match_committed_sources() {
    let resources = EngineResources::embedded();
    for path in resources.logical_paths() {
        let source_path = if path == "assets/dcmtk_srgb_input_profile.hex" {
            "src/assets/dcmtk_srgb_input_profile.hex"
        } else {
            path
        };
        assert_eq!(
            resources.bytes(path).unwrap().as_ref(),
            fs::read(source_path).unwrap(),
            "embedded bytes drifted for {path}"
        );
    }
}

#[test]
fn resource_lookup_rejects_unsafe_and_unknown_paths() {
    let resources = EngineResources::embedded();
    for path in [
        "",
        "../standards.lock.json",
        "/standards.lock.json",
        "cases\\registry.json",
    ] {
        assert!(matches!(
            resources.bytes(path),
            Err(EngineResourceError::UnsafeLogicalPath(_))
        ));
    }
    assert!(matches!(
        resources.bytes("schemas/not-a-schema.json"),
        Err(EngineResourceError::UnknownResource(_))
    ));
}

#[test]
fn snapshot_materializes_and_cleans_the_complete_resource_tree() {
    let resources = EngineResources::embedded();
    let root = {
        let snapshot = resources.snapshot().unwrap();
        let root = snapshot.root().to_path_buf();
        assert_eq!(
            fs::read(snapshot.path("cases/registry.json").unwrap()).unwrap(),
            resources.bytes("cases/registry.json").unwrap().as_ref()
        );
        assert!(snapshot.path("templates/catalog.json").unwrap().is_file());
        root
    };
    assert!(!root.exists(), "snapshot must clean up on drop");
}
