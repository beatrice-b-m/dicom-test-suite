use std::fs;

#[test]
fn ci_makes_every_standalone_release_gate_mandatory_and_regression_backed() {
    let workflow = fs::read_to_string(".github/workflows/ci.yml").unwrap();
    let release = workflow.split("  standalone-release:").nth(1).unwrap();
    for required in [
        "needs: default",
        "cargo fmt --all -- --check",
        "git diff --check",
        "RUSTFLAGS='-D warnings' cargo check",
        "product_resource_lookup_audit",
        "schema_artifacts",
        "compatibility_ownership",
        "non_rust_cli_consumer",
        "caller_content_consumer",
        "qualified_catalog_consumer",
        "structural_catalog_consumer",
        "upgrade_consumer",
        "cargo package --locked",
        "sdk_external_consumer",
        "scripts/build-release-archive.sh",
        "scripts/verify-release-archive.sh",
        "release_archive",
        "actions/upload-artifact@v7",
    ] {
        assert!(
            release.contains(required),
            "mandatory CI gate omitted: {required}"
        );
    }

    let regression_sources = [
        (
            "tests/product_resource_lookup_audit.rs",
            "findings.is_empty",
        ),
        ("tests/schema_artifacts.rs", "reject"),
        ("tests/black_box_cli_consumer.py", "expected=0"),
        ("tests/release_archive.rs", "parallelism"),
        ("tests/release_archive.rs", "checksum"),
        ("tests/upgrade_consumer.py", "9.0.0"),
    ];
    for (path, marker) in regression_sources {
        assert!(
            fs::read_to_string(path).unwrap().contains(marker),
            "{path} lacks deliberate regression marker {marker}"
        );
    }
}
