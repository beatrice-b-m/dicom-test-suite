use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn rust_files(root: &Path) -> Vec<PathBuf> {
    let mut pending = vec![root.to_path_buf()];
    let mut files = Vec::new();
    while let Some(path) = pending.pop() {
        if !path.exists() {
            continue;
        }
        if path.is_dir() {
            for entry in fs::read_dir(&path).expect("source directory should be readable") {
                pending.push(entry.expect("source entry should be readable").path());
            }
        } else if path.extension().and_then(|value| value.to_str()) == Some("rs") {
            files.push(path);
        }
    }
    files.sort();
    files
}

fn repository_relative(path: &Path) -> String {
    path.strip_prefix(repository_root())
        .expect("audited source should be inside the repository")
        .to_string_lossy()
        .replace('\\', "/")
}

fn production_source(path: &Path) -> String {
    let source = fs::read_to_string(path).expect("Rust source should be readable");
    source
        .split_once("#[cfg(test)]")
        .map_or(source.as_str(), |(production, _)| production)
        .to_string()
}

#[test]
fn neutral_spine_modules_do_not_depend_on_frontends_or_reporting() {
    let root = repository_root();
    let neutral_roots = [
        root.join("src/corpus_plan.rs"),
        root.join("src/corpus_plan"),
        root.join("src/planning.rs"),
        root.join("src/planning"),
        root.join("src/recipes.rs"),
        root.join("src/recipes"),
        root.join("src/executor.rs"),
        root.join("src/executor"),
    ];
    let forbidden = [
        ("crate::generator", "curated generator"),
        ("super::generator", "curated generator"),
        ("crate::main", "CLI entry point"),
        ("clap::", "CLI argument parsing"),
        ("PreparedGenerationRun", "curated CLI/run arguments"),
        ("GenerateOptions", "curated CLI arguments"),
        ("ComposeOptions", "composition CLI arguments"),
        ("CompositionSpec", "composition-spec parsing"),
        ("composition::spec", "composition-spec parsing"),
        ("crate::coverage_gaps", "registry reporting"),
        ("crate::report", "registry reporting"),
    ];

    let files = neutral_roots
        .iter()
        .flat_map(|path| rust_files(path))
        .collect::<BTreeSet<_>>();
    assert!(
        !files.is_empty(),
        "the unified spine must expose at least one neutral plan, planning, recipe, or executor module"
    );

    let mut violations = Vec::new();
    for path in files {
        let source = production_source(&path);
        for (needle, boundary) in forbidden {
            if source.contains(needle) {
                violations.push(format!(
                    "{} imports {boundary} through `{needle}`",
                    repository_relative(&path)
                ));
            }
        }
    }
    assert!(
        violations.is_empty(),
        "neutral unified-spine dependency violations:\n{}",
        violations.join("\n")
    );
}

#[test]
fn composition_has_no_generator_dependency() {
    let composition_root = repository_root().join("src/composition");
    let mut imports = BTreeSet::new();
    for path in rust_files(&composition_root) {
        if path
            .file_name()
            .and_then(|value| value.to_str())
            .is_some_and(|name| name.ends_with("_tests.rs"))
        {
            continue;
        }
        for line in production_source(&path).lines() {
            let line = line.trim();
            if line.contains("crate::generator") || line.contains("super::generator") {
                imports.insert(format!("{}:{line}", repository_relative(&path)));
            }
        }
    }

    assert!(
        imports.is_empty(),
        "composition must not depend on the curated generator: {imports:?}"
    );
}

#[test]
fn production_dicom_writers_are_explicitly_classified() {
    #[derive(Clone, Copy)]
    struct Exception {
        removal_task: &'static str,
        reason: &'static str,
    }

    let allowlist = BTreeMap::from([
        (
            "src/composition/materializer.rs",
            Exception {
                removal_task: "PERMANENT_SHARED_MATERIALIZER",
                reason: "Part10Materializer is the target ordinary valid DICOM writer",
            },
        ),
        (
            "src/generator.rs",
            Exception {
                removal_task: "U3.5-U9.2",
                reason: "legacy curated, negative, stress, and encoding writers migrate by their assigned lanes",
            },
        ),
    ]);
    let dicom_writer_markers = [
        ".write_to_file(",
        ".write_meta(",
        "writer.write_all(b\"DICM\")",
        "fs::write(&path, &output.bytes)",
    ];

    let mut observed = BTreeSet::new();
    let mut violations = Vec::new();
    for path in rust_files(&repository_root().join("src")) {
        if path
            .file_name()
            .and_then(|value| value.to_str())
            .is_some_and(|name| name.ends_with("_tests.rs"))
        {
            continue;
        }
        let source = production_source(&path);
        if !dicom_writer_markers
            .iter()
            .any(|marker| source.contains(marker))
        {
            continue;
        }
        let relative = repository_relative(&path);
        match allowlist.get(relative.as_str()) {
            Some(exception) => {
                assert!(
                    exception.removal_task == "PERMANENT_SHARED_MATERIALIZER"
                        || exception.removal_task.starts_with('U'),
                    "temporary writer exception {relative} lacks a migration task"
                );
                assert!(!exception.reason.is_empty());
                observed.insert(relative);
            }
            None => violations.push(relative),
        }
    }
    assert!(
        violations.is_empty(),
        "unclassified production DICOM writers found:\n{}",
        violations.join("\n")
    );
    assert_eq!(
        observed,
        allowlist.keys().copied().map(String::from).collect(),
        "writer allowlist contains a stale exception; remove it when its migration task lands"
    );
}
