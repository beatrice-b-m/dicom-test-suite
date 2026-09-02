use std::fs;

fn workflow(path: &str) -> String {
    fs::read_to_string(path).unwrap_or_else(|error| panic!("cannot read {path}: {error}"))
}

#[test]
fn fast_pr_cancels_superseded_runs_without_duplicate_pr_branch_ownership() {
    let fast = workflow(".github/workflows/ci.yml");
    let expected_header = r#"name: Fast PR

on:
  push:
    branches:
      - main
  pull_request:
  workflow_dispatch:

concurrency:
  group: ci-${{ github.event_name == 'pull_request' && format('pr-{0}', github.event.pull_request.number) || format('{0}-{1}', github.event_name, github.ref) }}
  cancel-in-progress: true

"#;

    assert!(
        fast.starts_with(expected_header),
        "Fast PR must retain R1.1 main-push, PR, manual, and superseded-run ownership"
    );
    assert_eq!(fast.matches("\n  push:").count(), 1);
    assert_eq!(fast.matches("\n  pull_request:").count(), 1);
    assert_eq!(fast.matches("\n  workflow_dispatch:").count(), 1);
}

#[test]
fn fast_pr_is_bounded_to_light_contracts_and_tiny_smoke() {
    let fast = workflow(".github/workflows/ci.yml");

    for required in [
        "timeout-minutes: 15",
        "cargo fmt --all -- --check",
        "jq empty cases/registry.json schemas/*.json",
        "RUSTFLAGS: -D warnings",
        "cargo check --locked --no-default-features --lib --bins",
        "--test schema_artifacts --test compatibility_ownership",
        "--test standalone_docs --test ci_release_gates",
        "generate --profile smoke",
        "validate \"$root\"",
        "target_bytes=",
        "output_artifact_count=",
    ] {
        assert!(fast.contains(required), "Fast PR omitted {required}");
    }

    for forbidden in [
        "setup-uv",
        "uv python",
        "highdicom",
        "--all-targets",
        "--all-features",
        "--profile core",
        "--profile extended",
        "--profile all",
        "--include-stress",
        "cargo package",
        "build-release-archive",
        "verify-release-archive",
        "upload-artifact",
        "--release",
        "in-process-codecs",
        "external-codec",
    ] {
        assert!(
            !fast.contains(forbidden),
            "heavy boundary leaked into Fast PR: {forbidden}"
        );
    }
}

#[test]
fn heavy_workflow_retains_nightly_matrix_and_immutable_release_gate() {
    let heavy = workflow(".github/workflows/qualification.yml");
    let provider = heavy
        .split("\n  native-provider-contract:\n")
        .nth(1)
        .unwrap()
        .split("\n  default:\n")
        .next()
        .unwrap();
    let default = heavy
        .split("\n  default:\n")
        .nth(1)
        .unwrap()
        .split("\n  standalone-release:\n")
        .next()
        .unwrap();
    let release = heavy
        .split("\n  standalone-release:\n")
        .nth(1)
        .unwrap()
        .split("\n  in-process-codecs:\n")
        .next()
        .unwrap();
    let codecs = heavy
        .split("\n  in-process-codecs:\n")
        .nth(1)
        .unwrap()
        .split("\n  external-codec-compile:\n")
        .next()
        .unwrap();
    let external = heavy.split("\n  external-codec-compile:\n").nth(1).unwrap();

    for required in [
        "schedule:",
        "workflow_dispatch:",
        "qualification_class:",
        "release-candidate",
        "candidate_revision:",
        "^[0-9a-f]{40}$",
        "immutable 40-hex commit",
        "cancel-in-progress: true",
        "Prove selected revision",
    ] {
        assert!(
            heavy.contains(required),
            "heavy selection omitted {required}"
        );
    }
    assert!(!heavy.contains("\n  push:"));
    assert!(!heavy.contains("\n  pull_request:"));
    assert_eq!(
        heavy.matches("--compile-bytecode").count(),
        4,
        "every retained heavy backend environment must precompile the locked runtime"
    );

    assert!(provider.contains("Native provider contract"));
    assert!(provider.contains("fake_backend_cancellation_"));
    assert!(provider.contains("composition_curated_migration"));
    assert!(provider.contains("composition_quantitative"));
    assert!(provider.contains("RUST_TEST_THREADS: \"1\""));

    assert!(default.contains("timeout-minutes: 120"));
    assert!(default.contains("RUST_TEST_THREADS: \"1\""));
    assert!(default.contains("cargo test --locked --all-targets --no-default-features"));
    assert!(default.contains("--profile core"));
    assert!(default.contains("--profile extended"));
    assert!(default.contains("Verify smoke reproducibility"));

    assert!(codecs.contains("feature: [jpeg, charls, jpegxl, jpeg2000, deflate]"));
    assert!(codecs.contains("--all-targets --no-default-features --features"));
    assert!(codecs.contains("--no-run"));
    assert!(codecs.contains("Prepare locked generation backend"));
    assert!(codecs.contains("DTS_HIGHDICOM_PYTHON="));
    assert!(codecs.contains("Test feature-sensitive surfaces"));
    assert!(codecs.contains("--lib codecs::tests::"));
    assert!(codecs.contains("curated_exceptional_execution"));
    assert!(!codecs.contains("composition_curated_migration"));
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

    assert!(external.contains("feature: [htj2k_openjph, legacy_jpeg_dcmtk]"));
    assert!(external.contains("cargo test --locked --all-targets --features"));

    assert!(release.contains("if: needs.selection.outputs.class == 'release-candidate'"));
    for required in [
        "cargo fmt --all -- --check",
        "git diff --check",
        "RUSTFLAGS='-D warnings' cargo check",
        "product_resource_lookup_audit",
        "schema_artifacts",
        "compatibility_ownership",
        "standalone_docs",
        "non_rust_cli_consumer",
        "caller_content_consumer",
        "qualified_catalog_consumer",
        "structural_catalog_consumer",
        "upgrade_consumer",
        "release_process",
        "ci_release_gates",
        "cargo package --locked",
        "sdk_external_consumer",
        "scripts/build-release-archive.sh",
        "scripts/verify-release-archive.sh",
        "release_archive",
        "actions/upload-artifact@v7",
        "needs.selection.outputs.revision",
    ] {
        assert!(
            release.contains(required),
            "release gate omitted {required}"
        );
    }
    assert!(release.contains("path: ${{ runner.temp }}/dist/*"));
    assert_eq!(heavy.matches("cargo package --locked").count(), 1);
    assert_eq!(heavy.matches("actions/upload-artifact@v7").count(), 1);
    assert!(!release.contains("archive: false"));

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
