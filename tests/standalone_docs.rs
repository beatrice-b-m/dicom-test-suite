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
}
