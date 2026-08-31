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

// S1.1 freezes the existing findings. S1.2-S1.3 remove these signatures as
// their lookups move behind ProductResources; this list must only shrink.
const KNOWN_AMBIENT_SIGNATURES: &[&str] = &[
    "src/composition/advanced_defaults.rs|standards_lock_path: repository_root.join(\"standards.lock.json\"),",
    "src/composition/advanced_defaults.rs|standards_lock_path: repository_root.join(\"standards.lock.json\"),",
    "src/composition/advanced_semantic_defaults.rs|standards_lock_path: repository_root.join(\"standards.lock.json\"),",
    "src/conformance.rs|pub const DEFAULT_ACCEPTED_FINDINGS: &str = \"conformance/accepted-findings.json\";",
    "src/conformance.rs|pub const DEFAULT_VALIDATOR_CONFIG: &str = \"conformance/validators.json\";",
    "src/conformance.rs|pub const DEFAULT_VALIDATOR_LOCK: &str = \"conformance/validator-lock.json\";",
    "src/curated_plan.rs|recipes_root: root.join(\"cases/recipes\"),",
    "src/curated_plan.rs|registry_path: root.join(\"cases/registry.json\"),",
    "src/curated_plan.rs|standards_lock_path: root.join(\"standards.lock.json\"),",
    "src/curated_plan.rs|template_catalog_path: root.join(\"templates/catalog.json\"),",
    "src/generation_backends/mod.rs|pub const BACKEND_LOCK_FILE: &str = \"generation-backends.lock.json\";",
    "src/lib.rs|build_coverage_report_with_registry(root_dir, &snapshot.root().join(\"cases/registry.json\"))",
    "src/lib.rs|path: PathBuf::from(\"cases/registry.json\"),",
    "src/lib.rs|path: PathBuf::from(\"cases/registry.json\"),",
    "src/lib.rs|path: PathBuf::from(\"cases/registry.json\"),",
    "src/lib.rs|path: PathBuf::from(\"cases/registry.json\"),",
    "src/lib.rs|path: PathBuf::from(\"cases/registry.json\"),",
    "src/lib.rs|path: PathBuf::from(\"cases/registry.json\"),",
    "src/lib.rs|path: PathBuf::from(\"cases/registry.json\"),",
    "src/lib.rs|path: PathBuf::from(\"cases/registry.json\"),",
    "src/lib.rs|path: PathBuf::from(\"cases/registry.json\"),",
    "src/lib.rs|path: PathBuf::from(\"cases/registry.json\"),",
    "src/lib.rs|path: PathBuf::from(\"cases/registry.json\"),",
    "src/lib.rs|path: PathBuf::from(\"cases/registry.json\"),",
    "src/lib.rs|path: PathBuf::from(\"cases/registry.json\"),",
    "src/lib.rs|path: PathBuf::from(\"cases/registry.json\"),",
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
            let lookup_form = [
                "Path::new(",
                "PathBuf::from(",
                "String::from(",
                "fs::read",
                "read_json(",
                "TemplateCatalog::load(",
                ".join(",
                "DEFAULT_",
                "BACKEND_LOCK_FILE",
            ]
            .iter()
            .any(|form| trimmed.contains(form));

            let resolved_product_resource = trimmed.contains("resource_root.join(");
            let resource_path_comparison = trimmed.contains("== Path::new(");
            if compile_time_root
                || (resource_path
                    && lookup_form
                    && !resolved_product_resource
                    && !resource_path_comparison)
            {
                findings.push(format!("{}|{trimmed}", path.display()));
            }
        }
    });

    findings.sort();
    let mut known = KNOWN_AMBIENT_SIGNATURES
        .iter()
        .copied()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    known.sort();
    assert_eq!(
        findings, known,
        "ambient first-party lookups must be inventoried and the allowlist may only shrink"
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
