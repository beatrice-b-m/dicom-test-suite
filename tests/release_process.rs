use std::fs;

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
