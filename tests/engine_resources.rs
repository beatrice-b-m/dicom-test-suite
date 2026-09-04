use std::fs::{self, OpenOptions};
use std::io::Write;

use synth_dicom_gen::engine_resources::{
    ENGINE_RESOURCE_COUNT_V2, ENGINE_RESOURCE_SET_MEMBERSHIP, ENGINE_RESOURCE_SET_VERSION,
    ENGINE_RESOURCE_SHA256_V2, ENGINE_RESOURCE_TOTAL_BYTES_V2, EngineResourceError,
    EngineResourceOrigin, EngineResourceSetMembership, EngineResources,
};

#[allow(dead_code)]
#[path = "../build.rs"]
mod engine_resource_build_script;

#[test]
fn embedded_resources_separate_current_identity_from_legacy_physical_closure() {
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
        EngineResourceSetMembership::SeparatedWithLegacyPhysicalClosure
    );
    assert!(resources.contains("cases/registry.json"));
    assert!(resources.contains("Cargo.lock"));
    let identity = resources.verify_integrity().unwrap();
    assert_eq!(identity.resource_set_version, ENGINE_RESOURCE_SET_VERSION);
    assert_eq!(identity.resource_count, ENGINE_RESOURCE_COUNT_V2);
    assert_eq!(identity.resource_count, 80);
    assert_eq!(identity.resource_set_sha256, ENGINE_RESOURCE_SHA256_V2);
    assert_eq!(
        identity
            .resources
            .iter()
            .map(|item| item.size_bytes)
            .sum::<u64>(),
        ENGINE_RESOURCE_TOTAL_BYTES_V2
    );
    assert!(
        !identity.resources.iter().any(
            |item| item.logical_path == "Cargo.lock" || item.logical_path.starts_with("cases/")
        )
    );
    for schema in [
        "schemas/corpus-definition-bundle.schema.json",
        "schemas/version-result-v2.schema.json",
        "schemas/capabilities-result-v2.schema.json",
        "schemas/manifest-v1.schema.json",
        "schemas/manifest-v2.schema.json",
        "schemas/coverage-report-v1.1.schema.json",
        "schemas/release-manifest-v2.schema.json",
    ] {
        assert!(
            identity
                .resources
                .iter()
                .any(|item| item.logical_path == schema),
            "current engine identity omits direct schema {schema}"
        );
    }
}

#[test]
fn resource_build_tracks_directory_additions() {
    let build = fs::read_to_string("build.rs").unwrap();
    assert!(build.contains("cargo:rerun-if-changed={}"));
    assert!(build.contains("directory.display()"));
    assert!(build.contains("embedded_engine_resources.rs"));
    assert!(build.contains("symlink_metadata"));
    assert!(build.contains("require_regular_engine_resource"));
    assert!(build.contains("validate_engine_resource_path"));
    assert!(
        !engine_resource_build_script::is_transitional_engine_resource(
            "schemas/corpus-definition-bundle.schema.json"
        )
    );
    for schema in [
        "schemas/version-result-v2.schema.json",
        "schemas/capabilities-result-v2.schema.json",
        "schemas/composition-manifest-v1.schema.json",
        "schemas/composition-result-v2.schema.json",
        "schemas/generation-result-v2.schema.json",
        "schemas/manifest-v1.schema.json",
        "schemas/manifest-v2.schema.json",
    ] {
        assert!(
            !engine_resource_build_script::is_transitional_engine_resource(schema),
            "new identity-domain schema must remain outside the v1 oracle: {schema}"
        );
    }
    assert!(
        engine_resource_build_script::is_transitional_engine_resource(
            "schemas/case-recipe.schema.json"
        )
    );
    let reader = fs::read_to_string("src/engine_resources.rs").unwrap();
    for required in [
        "libc::openat",
        "libc::O_NOFOLLOW",
        "libc::O_NONBLOCK",
        "take(expected.len() as u64 + 1)",
        "root_after.dev() != root_open.dev()",
        "TRANSITIONAL_ENGINE_RESOURCE_COUNT_V1: usize = 240",
        "dc61cc012f983297fef864f68e6cd172a9d33ac9ad4faab4cc66d3526b688410",
    ] {
        assert!(
            reader.contains(required),
            "missing hardening oracle {required}"
        );
    }

    use engine_resource_build_script::{EngineResourcePathKind, validate_engine_resource_path};
    assert!(
        validate_engine_resource_path(
            std::path::Path::new("schemas"),
            EngineResourcePathKind::Directory,
        )
        .is_ok()
    );
    assert!(
        validate_engine_resource_path(
            std::path::Path::new("standards.lock.json"),
            EngineResourcePathKind::File,
        )
        .is_ok()
    );

    let fixture = std::env::temp_dir().canonicalize().unwrap().join(format!(
        "synth-dicom-gen-build-resource-test-{}",
        std::process::id()
    ));
    fs::create_dir(&fixture).unwrap();
    let nondirectory = fixture.join("not-a-directory");
    fs::write(&nondirectory, b"file").unwrap();
    assert!(
        validate_engine_resource_path(&nondirectory, EngineResourcePathKind::Directory).is_err()
    );

    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;

        let real = fixture.join("real");
        fs::create_dir(&real).unwrap();
        let scan_link = fixture.join("scan-link");
        symlink(&real, &scan_link).unwrap();
        assert!(
            validate_engine_resource_path(&scan_link, EngineResourcePathKind::Directory).is_err()
        );

        let fixed_target = real.join("fixed-target.json");
        fs::write(&fixed_target, b"{}").unwrap();
        let fixed_link = real.join("fixed-link.json");
        symlink(&fixed_target, &fixed_link).unwrap();
        assert!(validate_engine_resource_path(&fixed_link, EngineResourcePathKind::File).is_err());

        let ancestor_link = fixture.join("ancestor-link");
        symlink(&real, &ancestor_link).unwrap();
        assert!(
            validate_engine_resource_path(
                &ancestor_link.join("fixed-target.json"),
                EngineResourcePathKind::File,
            )
            .is_err()
        );
    }
    fs::remove_dir_all(fixture).unwrap();
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
    let explicit = EngineResources::explicit(&root).unwrap();
    let explicit_identity = explicit.verify_integrity().unwrap();
    assert_eq!(embedded.resource_set_version, ENGINE_RESOURCE_SET_VERSION);
    assert_eq!(embedded.origin, EngineResourceOrigin::Embedded);
    assert_eq!(explicit_identity.origin, EngineResourceOrigin::Explicit);
    assert_eq!(embedded.resource_count, explicit_identity.resource_count);
    assert_eq!(
        embedded.resource_set_sha256,
        explicit_identity.resource_set_sha256
    );
    assert_eq!(embedded.resources, explicit_identity.resources);

    fs::write(
        root.join("cases/registry.json"),
        b"post-construction tamper",
    )
    .unwrap();
    assert_eq!(
        resources.bytes("cases/registry.json").unwrap(),
        explicit.bytes("cases/registry.json").unwrap(),
        "an explicit handle must retain its verified immutable capture"
    );
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
    assert!(matches!(error, EngineResourceError::SizeMismatch { .. }));
    assert_eq!(error.code(), "evidence.integrity.failed");

    let oversized = embedded.snapshot().unwrap();
    OpenOptions::new()
        .append(true)
        .open(oversized.root().join("Cargo.lock"))
        .unwrap()
        .write_all(b"x")
        .unwrap();
    let error = EngineResources::explicit(oversized.root()).unwrap_err();
    assert!(matches!(error, EngineResourceError::SizeMismatch { .. }));
    assert_eq!(error.code(), "evidence.integrity.failed");

    let undersized = embedded.snapshot().unwrap();
    OpenOptions::new()
        .write(true)
        .open(undersized.root().join("Cargo.lock"))
        .unwrap()
        .set_len(1)
        .unwrap();
    let error = EngineResources::explicit(undersized.root()).unwrap_err();
    assert!(matches!(error, EngineResourceError::SizeMismatch { .. }));
    assert_eq!(error.code(), "evidence.integrity.failed");

    let sparse = embedded.snapshot().unwrap();
    let cargo_lock = sparse.root().join("Cargo.lock");
    let expected_size = fs::metadata(&cargo_lock).unwrap().len();
    fs::remove_file(&cargo_lock).unwrap();
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&cargo_lock)
        .unwrap()
        .set_len(expected_size)
        .unwrap();
    let error = EngineResources::explicit(sparse.root()).unwrap_err();
    assert!(matches!(error, EngineResourceError::Integrity { .. }));
    assert_eq!(error.code(), "evidence.integrity.failed");

    let root_file = embedded.snapshot().unwrap();
    let error = EngineResources::explicit(root_file.root().join("Cargo.lock")).unwrap_err();
    assert!(matches!(error, EngineResourceError::NotRegular { .. }));
    assert_eq!(error.code(), "resource.document.invalid");

    let intermediate_file = embedded.snapshot().unwrap();
    let templates = intermediate_file.root().join("templates");
    fs::remove_dir_all(&templates).unwrap();
    fs::write(&templates, b"not a directory").unwrap();
    let error = EngineResources::explicit(intermediate_file.root()).unwrap_err();
    assert!(matches!(error, EngineResourceError::NotRegular { .. }));
    assert_eq!(error.code(), "resource.document.invalid");

    let final_directory = embedded.snapshot().unwrap();
    let cargo_lock = final_directory.root().join("Cargo.lock");
    fs::remove_file(&cargo_lock).unwrap();
    fs::create_dir(&cargo_lock).unwrap();
    let error = EngineResources::explicit(final_directory.root()).unwrap_err();
    assert!(matches!(error, EngineResourceError::NotRegular { .. }));
    assert_eq!(error.code(), "resource.document.invalid");

    #[cfg(unix)]
    {
        use std::ffi::CString;
        use std::os::unix::ffi::OsStrExt;
        use std::os::unix::fs::symlink;

        let fifo = embedded.snapshot().unwrap();
        let cargo_lock = fifo.root().join("Cargo.lock");
        fs::remove_file(&cargo_lock).unwrap();
        let fifo_path = CString::new(cargo_lock.as_os_str().as_bytes()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(fifo_path.as_ptr(), 0o600) }, 0);
        let error = EngineResources::explicit(fifo.root()).unwrap_err();
        assert!(matches!(error, EngineResourceError::NotRegular { .. }));
        assert_eq!(error.code(), "resource.document.invalid");

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
        assert!(matches!(
            error,
            EngineResourceError::Symlink { .. } | EngineResourceError::NotRegular { .. }
        ));
        assert_eq!(error.code(), "resource.document.invalid");

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
