use std::fs;
use std::path::Path;

const RESOURCE_MARKERS: &[&str] = &[
    "cases/",
    "templates/",
    "schemas/",
    "conformance/",
    "security/fixtures/",
    "standards.lock.json",
    "generation-backends.lock.json",
    "transfer-syntax/",
];

#[test]
fn production_has_no_uninventoried_ambient_resource_lookups() {
    let mut findings = Vec::new();
    visit_rust_sources(Path::new("src"), &mut |path, source| {
        let mut previous_was_test_cfg = false;
        for line in source.lines() {
            let trimmed = line.trim();
            if previous_was_test_cfg && trimmed == "mod tests {" {
                break;
            }
            previous_was_test_cfg = trimmed == "#[cfg(test)]";

            let compile_time_root = trimmed.contains("CARGO_MANIFEST_DIR");
            let resource_path = RESOURCE_MARKERS
                .iter()
                .any(|marker| trimmed.contains(marker));
            let direct_ambient_read = [
                "fs::read(\"",
                "fs::read_to_string(\"",
                "File::open(\"",
                "read_json(\"",
                "read_json(Path::new(\"",
                "TemplateCatalog::load(\"",
            ]
            .iter()
            .any(|form| trimmed.contains(form));
            let ambient_default = ["Path::new(\"", "PathBuf::from(\"", "String::from(\""]
                .iter()
                .any(|form| trimmed.contains(form))
                && !trimmed.starts_with("path: PathBuf::from(")
                && !trimmed.contains("== Path::new(");
            if compile_time_root || (resource_path && (direct_ambient_read || ambient_default)) {
                findings.push(format!("{}|{trimmed}", path.display()));
            }
        }
    });

    findings.sort();
    assert!(
        findings.is_empty(),
        "production first-party resources must resolve through ProductResources: {findings:#?}"
    );
}

fn visit_rust_sources(root: &Path, visitor: &mut impl FnMut(&Path, &str)) {
    for entry in fs::read_dir(root).expect("read source directory") {
        let entry = entry.expect("source entry");
        let path = entry.path();
        if path.is_dir() {
            visit_rust_sources(&path, visitor);
        } else if path.extension().and_then(|value| value.to_str()) == Some("rs") {
            let source = fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
            visitor(&path, &source);
        }
    }
}
