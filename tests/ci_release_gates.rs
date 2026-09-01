use std::fs;

#[test]
fn ci_makes_every_standalone_release_gate_mandatory_and_regression_backed() {
    let workflow = fs::read_to_string(".github/workflows/ci.yml").unwrap();
    let default = workflow.split("  default:").nth(1).unwrap();
    let codecs = workflow
        .split("  in-process-codecs:")
        .nth(1)
        .unwrap()
        .split("  external-codec-compile:")
        .next()
        .unwrap();
    assert!(default.contains("timeout-minutes: 120"));
    assert!(default.contains("RUST_TEST_THREADS: \"1\""));
    assert!(default.contains("cargo test --locked --all-targets --no-default-features"));
    assert!(codecs.contains("--all-targets --no-default-features --features"));
    assert!(codecs.contains("--no-run"));
    assert!(codecs.contains("Test feature-sensitive surfaces"));
    assert!(codecs.contains("--lib codecs::tests::"));
    assert!(!codecs.contains("\"${{ matrix.feature }}\" --lib\n"));
    assert!(codecs.contains("curated_exceptional_execution"));
    assert!(codecs.contains("frame_codec_service"));
    assert!(codecs.contains("validate_cli"));
    assert!(
        codecs
            .contains("validate_command_reports_deflated_image_frame_decoded_frame_hash_mismatch")
    );
    assert!(!codecs.contains("--test report_cli"));
    assert!(!codecs.contains("--test validate_cli --test"));
    assert!(codecs.contains("Exercise feature corpus"));
    assert!(codecs.contains(".counts.generated > 115 and .counts.blocked == 0"));
    assert!(codecs.contains("fragments_per_frame == [1,1]"));
    assert!(
        !codecs
            .contains("cargo test --locked --all-targets --features \"${{ matrix.feature }}\"\n")
    );
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
