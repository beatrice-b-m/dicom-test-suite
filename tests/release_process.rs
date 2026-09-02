use std::fs;
use std::process::Command;

#[test]
fn maintainer_procedure_is_clean_clone_complete_and_fail_closed() {
    let procedure = fs::read_to_string("docs/release-process.md").unwrap();
    let normalized = procedure.split_whitespace().collect::<Vec<_>>().join(" ");
    for required in [
        "git clone",
        "git checkout --detach RELEASE_REVISION",
        "test -z \"$(git status --porcelain)\"",
        "cargo test --locked --all-targets --no-default-features",
        "cargo package --locked --offline",
        "scripts/build-release-archive.sh \"$TARGET\" \"$DIST\"",
        "scripts/verify-release-archive.sh \"$ARCHIVE\"",
        "release-manifest.json",
        "archive SHA-256",
        "source revision",
        "feature set",
        "resource-set SHA-256",
        "unavailable optional capabilities",
        "Linux x86_64",
        "macOS arm64",
        "Never represent a missing optional runtime, peer, codec, or target as a pass",
    ] {
        assert!(
            normalized.contains(required),
            "release procedure omits {required}"
        );
    }
    assert!(!procedure.contains("DTS_RELEASE_ALLOW_DIRTY=1"));
    assert!(!procedure.contains("git add -A"));
}

#[test]
fn changelog_has_explicit_standalone_migration_actions() {
    let changelog = fs::read_to_string("CHANGELOG.md").unwrap();
    let normalized = changelog.split_whitespace().collect::<Vec<_>>().join(" ");
    for required in [
        "## [Unreleased]",
        "### Migration notes",
        "checksummed native archive",
        "--format json",
        "--cli-api 1.0.0",
        "generate",
        "compose",
        "assemble",
        "iod_conformance = \"not_assessed\"",
        "Install upgrades side by side",
        "unsupported version",
    ] {
        assert!(normalized.contains(required), "changelog omits {required}");
    }
    assert!(normalized.contains("not a promoted `1.0.0` release"));
}

#[test]
fn release_scripts_default_to_clean_locked_target_bound_artifacts() {
    let builder = fs::read_to_string("scripts/build-release-archive.sh").unwrap();
    let verifier = fs::read_to_string("scripts/verify-release-archive.sh").unwrap();
    for required in [
        "release archives require a clean worktree",
        "cargo build --release --locked --target",
        "DTS_RELEASE_BINARY must be an absolute path",
        "DTS_RELEASE_BINARY_SHA256 is required with DTS_RELEASE_BINARY",
        "source revision $source_revision does not match DTS_RELEASE_REVISION",
        "requested target $release_target does not match DTS_RELEASE_TARGET",
        "release binary SHA-256 does not match DTS_RELEASE_BINARY_SHA256",
        ".cargo_vcs_info.json",
        "release-manifest.json",
        "CHANGELOG.md",
        "docs/release-process.md",
        "cp schemas/*.json",
    ] {
        assert!(builder.contains(required), "builder omits {required}");
    }
    for required in [
        "release checksum is missing",
        "archive checksum does not match",
        "unsafe manifest path",
        "payload checksum differs",
        "verification=passed",
    ] {
        assert!(verifier.contains(required), "verifier omits {required}");
    }
}

#[test]
fn release_binary_override_contract_rejects_unbound_candidates() {
    let binary = std::env::current_exe().unwrap().canonicalize().unwrap();
    let binary_sha256 = dicom_test_suite::sha256_hex(&fs::read(&binary).unwrap());
    let revision = String::from_utf8(
        Command::new("git")
            .args(["rev-parse", "HEAD"])
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap()
    .trim()
    .to_owned();
    let run = |extra: &[(&str, &str)]| {
        let mut command = Command::new("sh");
        command
            .arg("scripts/build-release-archive.sh")
            .args(["candidate-target", "/tmp"])
            .env("DTS_RELEASE_BINARY", &binary)
            .env("DTS_RELEASE_ALLOW_DIRTY", "1")
            .env_remove("DTS_RELEASE_BINARY_SHA256")
            .env_remove("DTS_RELEASE_REVISION")
            .env_remove("DTS_RELEASE_TARGET");
        for (name, value) in extra {
            command.env(name, value);
        }
        command.output().unwrap()
    };

    let missing_hash = run(&[]);
    assert_eq!(missing_hash.status.code(), Some(4));
    assert!(
        String::from_utf8_lossy(&missing_hash.stderr)
            .contains("DTS_RELEASE_BINARY_SHA256 is required")
    );

    let wrong_revision = run(&[
        ("DTS_RELEASE_BINARY_SHA256", &binary_sha256),
        ("DTS_RELEASE_REVISION", &"0".repeat(40)),
        ("DTS_RELEASE_TARGET", "candidate-target"),
    ]);
    assert_eq!(wrong_revision.status.code(), Some(4));
    assert!(String::from_utf8_lossy(&wrong_revision.stderr).contains("source revision"));

    let wrong_target = run(&[
        ("DTS_RELEASE_BINARY_SHA256", &binary_sha256),
        ("DTS_RELEASE_REVISION", &revision),
        ("DTS_RELEASE_TARGET", "different-target"),
    ]);
    assert_eq!(wrong_target.status.code(), Some(4));
    assert!(String::from_utf8_lossy(&wrong_target.stderr).contains("requested target"));

    let wrong_hash = run(&[
        ("DTS_RELEASE_BINARY_SHA256", &"0".repeat(64)),
        ("DTS_RELEASE_REVISION", &revision),
        ("DTS_RELEASE_TARGET", "candidate-target"),
    ]);
    assert_eq!(wrong_hash.status.code(), Some(4));
    assert!(String::from_utf8_lossy(&wrong_hash.stderr).contains("release binary SHA-256"));
}
