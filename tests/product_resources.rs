use std::fs;

use synth_dicom_gen::product_resources::{
    PRODUCT_RESOURCE_SET_VERSION, ProductResourceError, ProductResourceOrigin, ProductResources,
};

#[test]
fn embedded_resources_cover_all_first_party_product_families() {
    let resources = ProductResources::embedded();
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
}

#[test]
fn resource_build_tracks_directory_additions() {
    let build = fs::read_to_string("build.rs").unwrap();
    assert!(build.contains("cargo:rerun-if-changed={}"));
    assert!(build.contains("directory.display()"));
}

#[test]
fn embedded_and_explicit_repository_resources_have_identical_identity() {
    let resources = ProductResources::embedded();
    let root = std::env::temp_dir().join(format!(
        "dicom-test-suite-resource-test-{}-{}",
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
    let explicit = ProductResources::explicit(&root)
        .verify_integrity()
        .unwrap();
    assert_eq!(embedded.resource_set_version, PRODUCT_RESOURCE_SET_VERSION);
    assert_eq!(embedded.origin, ProductResourceOrigin::Embedded);
    assert_eq!(explicit.origin, ProductResourceOrigin::Explicit);
    assert_eq!(embedded.resource_count, explicit.resource_count);
    assert_eq!(embedded.resource_set_sha256, explicit.resource_set_sha256);
    assert_eq!(embedded.resources, explicit.resources);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn tampered_explicit_resources_fail_before_materialization_with_a_stable_code() {
    let embedded = ProductResources::embedded();
    let snapshot = embedded.snapshot().unwrap();
    let registry_path = snapshot.root().join("cases/registry.json");
    let mut registry = fs::read(&registry_path).unwrap();
    registry.push(b'\n');
    fs::write(&registry_path, registry).unwrap();

    let explicit = ProductResources::explicit(snapshot.root());
    let error = explicit.snapshot().unwrap_err();
    assert!(matches!(error, ProductResourceError::Integrity { .. }));
    assert_eq!(error.code(), "evidence.integrity.failed");
}

#[test]
fn embedded_bytes_match_committed_sources() {
    let resources = ProductResources::embedded();
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
    let resources = ProductResources::embedded();
    for path in [
        "",
        "../standards.lock.json",
        "/standards.lock.json",
        "cases\\registry.json",
    ] {
        assert!(matches!(
            resources.bytes(path),
            Err(ProductResourceError::UnsafeLogicalPath(_))
        ));
    }
    assert!(matches!(
        resources.bytes("schemas/not-a-schema.json"),
        Err(ProductResourceError::UnknownResource(_))
    ));
}

#[test]
fn snapshot_materializes_and_cleans_the_complete_resource_tree() {
    let resources = ProductResources::embedded();
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
