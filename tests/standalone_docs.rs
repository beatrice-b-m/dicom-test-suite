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
        "DTS=",
        "\"$DTS\" version --format json",
        "\"$DTS\" capabilities --format json",
        "\"$DTS\" generate",
        "\"$DTS\" compose",
        "\"$DTS\" assemble",
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
