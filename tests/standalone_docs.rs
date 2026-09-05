use std::fs;

#[test]
fn installed_product_guides_cover_the_complete_external_consumer_contract() {
    let installation = fs::read_to_string("docs/installation-guide.md").unwrap();
    let automation = fs::read_to_string("docs/automation-guide.md").unwrap();
    let combined = format!("{installation}\n{automation}");

    assert!(!combined.contains("cargo run"));
    for required in [
        "shasum -a 256 -c",
        "version --format json",
        "capabilities --format json",
        "generate",
        "compose",
        "assemble",
        "--cli-api 1.0.0",
        "manifest.json",
        "byte_stable",
        "semantic_stable",
        "Same-project generation plus validation is not independent",
        "Unsupported input receives a",
        "migration action",
        "cargo install` is not a claimed release channel yet",
    ] {
        assert!(
            combined.contains(required),
            "missing guide contract: {required}"
        );
    }
    assert_current_guides_use_the_renamed_product_and_history_stays_exact();
    assert_public_guides_define_the_bounded_caller_classic_ct_contract();
}

#[test]
fn documentation_map_routes_installed_consumers_to_both_guides() {
    let map = fs::read_to_string("docs/README.md").unwrap();
    assert!(map.contains("installation-guide.md"));
    assert!(map.contains("automation-guide.md"));
    assert!(map.contains("examples-guide.md"));
}

#[test]
fn installed_examples_are_neutral_self_contained_and_documented() {
    let guide = fs::read_to_string("docs/examples-guide.md").unwrap();
    let normalized_guide = guide.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(!guide.contains("cargo run"));
    for example in [
        "compose-raw-grayscale.json",
        "compose-raw-rgb.json",
        "compose-metadata-private-sequence.json",
        "compose-multi-instance-reference.json",
        "assemble-structural.json",
    ] {
        assert!(guide.contains(example), "guide omits {example}");
        let document = fs::read_to_string(format!("examples/{example}")).unwrap();
        assert!(
            !document.contains("local_file"),
            "{example} is not relocatable"
        );
        assert!(
            !document.contains("provider"),
            "{example} needs an external provider"
        );
        assert!(
            document.contains("SYNTHETIC") || !document.contains("Patient"),
            "{example} has a patient field without an explicit synthetic value"
        );
    }
    for boundary in [
        "qualified-template outputs",
        "does not assess IOD conformance",
        "same-project structural validator",
    ] {
        assert!(
            normalized_guide.contains(boundary),
            "guide omits {boundary}"
        );
    }
}

#[test]
fn readme_leads_with_installed_product_and_isolates_contributor_commands() {
    let readme = fs::read_to_string("README.md").unwrap();
    let development = readme.find("## Development").unwrap();
    let consumer = &readme[..development];
    let contributor = &readme[development..];
    for required in [
        "shasum -a 256 -c",
        "GENERATOR=",
        "\"$GENERATOR\" version --format json",
        "\"$GENERATOR\" capabilities --format json",
        "\"$GENERATOR\" generate",
        "\"$GENERATOR\" compose",
        "\"$GENERATOR\" assemble",
    ] {
        assert!(
            consumer.contains(required),
            "consumer quick start omits {required}"
        );
    }
    assert!(!consumer.contains("cargo run"));
    assert!(contributor.contains("cargo run --locked"));
    assert!(contributor.contains("cargo test --locked"));
}

fn assert_public_guides_define_the_bounded_caller_classic_ct_contract() {
    for path in [
        "README.md",
        "SYSTEM_SPEC.md",
        "docs/generation-guide.md",
        "docs/sdk-guide.md",
        "docs/corpus-consumption.md",
        "docs/compatibility-policy.md",
    ] {
        let guide = fs::read_to_string(path).unwrap();
        for required in [
            "native.classic_plan",
            "classic/ct@1.0.0",
            "content.native_pixels",
            "algorithm.classic_ct",
        ] {
            assert!(guide.contains(required), "{path} omits {required}");
        }
    }

    let changelog = fs::read_to_string("CHANGELOG.md").unwrap();
    assert!(changelog.contains("Caller-defined classic CT corpus recipes"));
    assert!(changelog.contains("fail-closed capability"));

    let detailed = fs::read_to_string("docs/generation-guide.md").unwrap();
    for required in [
        "classic_projection.family = \"ct\"",
        "planning_order",
        "globally unique",
        "partial or mixed tuple",
        "native.stress_ct_plan",
        "implementation-module imports",
        "sibling-path discovery",
        "independent DICOM conformance",
        "viewer interoperability",
        "release qualification",
    ] {
        assert!(detailed.contains(required), "CT contract omits {required}");
    }
    assert!(!detailed.contains("During corpus separation"));
}

fn assert_current_guides_use_the_renamed_product_and_history_stays_exact() {
    let current_guides = [
        "README.md",
        "AGENTS.md",
        "CHANGELOG.md",
        "SYSTEM_SPEC.md",
        "docs/assembly-guide.md",
        "docs/automation-guide.md",
        "docs/compatibility-policy.md",
        "docs/composition-integration-guide.md",
        "docs/composition-security-policy.md",
        "docs/corpus-consumption.md",
        "docs/deterministic-build-policy.md",
        "docs/examples-guide.md",
        "docs/external-codec-verification.md",
        "docs/generation-guide.md",
        "docs/installation-guide.md",
        "docs/release-process.md",
        "docs/sdk-guide.md",
        "conformance/README.md",
        "conformance-backends/dicom-validator/README.md",
        "conformance-backends/wsi-reconstruction/README.md",
        "generation-backends/highdicom-pydicom/README.md",
        "security/fixtures/README.md",
        "standards/kb-integration.md",
    ];
    let old_product = ["dicom-test", "suite"].join("-");
    let old_crate = ["dicom", "test", "suite"].join("_");

    for path in current_guides {
        let contents = fs::read_to_string(path).unwrap();
        assert!(
            !contents.contains(&old_crate),
            "current guide {path} retains the old Rust crate spelling"
        );
        for suffix in [
            " generate",
            " validate",
            " report",
            " compose",
            " assemble",
            "/compare/HEAD...HEAD",
        ] {
            assert!(
                !contents.contains(&format!("{old_product}{suffix}")),
                "current guide {path} retains an old product command or URL"
            );
        }
        let old_mentions = contents.matches(&old_product).count();
        if path == "docs/installation-guide.md" {
            assert_eq!(old_mentions, 1, "historical candidate label drifted");
            assert!(contents.contains(&format!("{old_product} 0.1.0")));
        } else {
            assert_eq!(old_mentions, 0, "current guide {path} retains old branding");
        }
    }

    let generation = fs::read_to_string("docs/generation-guide.md").unwrap();
    let old_m6 = ["DTS", "M6", "SEGMENTATION", "FIXTURE"].join("_");
    assert!(!generation.contains(&old_m6));
    assert!(generation.contains("SYNTH_DICOM_GEN_M6_SEGMENTATION_FIXTURE"));

    let historical_status =
        fs::read_to_string("docs/standalone-product-status-2026-08-31.md").unwrap();
    assert!(
        historical_status.contains(&format!("{old_product}-0.1.0-aarch64-apple-darwin.tar.gz"))
    );
    let adr =
        fs::read_to_string("docs/adr/0003-synth-dicom-gen-dcmview-corpus-separation.md").unwrap();
    assert!(adr.contains(&old_product));
    assert!(adr.contains("synth-dicom-gen"));
}
