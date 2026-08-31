use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use jsonschema::Validator;
use serde_json::Value;

static NEXT: AtomicU64 = AtomicU64::new(0);

struct TempRoot(PathBuf);

impl TempRoot {
    fn new(label: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "dts-release-archive-{label}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        assert!(!path.exists());
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn read_json(path: impl AsRef<Path>) -> Value {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

fn assert_installed_example(
    installed: &Path,
    unrelated: &Path,
    request: &Path,
    command: &str,
    first_root: &Path,
    second_root: &Path,
) {
    let input_flag = if command == "compose" {
        "--spec"
    } else {
        "--request"
    };
    for output_root in [first_root, second_root] {
        let output = Command::new(installed)
            .current_dir(unrelated)
            .arg(command)
            .arg(input_flag)
            .arg(request)
            .arg("--out")
            .arg(output_root)
            .args(["--seed", "1", "--format", "json"])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "installed {command} example {} failed: {}",
            request.display(),
            String::from_utf8_lossy(&output.stderr)
        );
        let result: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(result["status"], "success");

        let validation = Command::new(installed)
            .current_dir(unrelated)
            .arg("validate")
            .arg(output_root)
            .args(["--format", "json"])
            .output()
            .unwrap();
        assert!(
            validation.status.success(),
            "installed example validation failed: {}",
            String::from_utf8_lossy(&validation.stderr)
        );
        let report = Command::new(installed)
            .current_dir(unrelated)
            .arg("report")
            .arg(output_root)
            .args(["--format", "json", "--cli-api", "1.0.0"])
            .output()
            .unwrap();
        assert!(
            report.status.success(),
            "installed example report failed: {}",
            String::from_utf8_lossy(&report.stderr)
        );
    }

    let first = read_json(first_root.join("manifest.json"));
    let second = read_json(second_root.join("manifest.json"));
    assert_eq!(
        first["run"]["corpus_plan_sha256"],
        second["run"]["corpus_plan_sha256"],
        "{} changed its plan identity",
        request.display()
    );
    let entries = if command == "compose" {
        first["composition"]["entries"].as_array().unwrap()
    } else {
        first["instances"].as_array().unwrap()
    };
    for entry in entries {
        let relative = entry[if command == "compose" {
            "path"
        } else {
            "output_path"
        }]
        .as_str()
        .unwrap();
        assert_eq!(
            fs::read(first_root.join(relative)).unwrap(),
            fs::read(second_root.join(relative)).unwrap(),
            "{} changed {relative} bytes",
            request.display()
        );
    }
}

#[test]
fn current_target_archive_is_manifest_bound_and_relocatable() {
    let binary = PathBuf::from(env!("CARGO_BIN_EXE_dicom-test-suite"));
    let version_output = Command::new(&binary)
        .args(["version", "--format", "json"])
        .output()
        .unwrap();
    assert!(version_output.status.success());
    let version: Value = serde_json::from_slice(&version_output.stdout).unwrap();
    let target = version["result"]["target"].as_str().unwrap();
    let product_version = version["result"]["product"]["version"].as_str().unwrap();

    let workspace = TempRoot::new("build");
    let dist = workspace.0.join("dist");
    let build = Command::new("sh")
        .arg("scripts/build-release-archive.sh")
        .arg(target)
        .arg(&dist)
        .env("DTS_RELEASE_BINARY", &binary)
        .env("DTS_RELEASE_ALLOW_DIRTY", "1")
        .output()
        .unwrap();
    assert!(
        build.status.success(),
        "archive build failed: {}",
        String::from_utf8_lossy(&build.stderr)
    );

    let archive_name = format!("dicom-test-suite-{product_version}-{target}");
    let archive = dist.join(format!("{archive_name}.tar.gz"));
    let checksum = dist.join(format!("{archive_name}.tar.gz.sha256"));
    assert!(archive.is_file());
    assert_eq!(
        fs::read_to_string(&checksum)
            .unwrap()
            .split_whitespace()
            .next()
            .unwrap(),
        dicom_test_suite::sha256_hex(&fs::read(&archive).unwrap())
    );
    let verified = Command::new("sh")
        .arg("scripts/verify-release-archive.sh")
        .arg(&archive)
        .output()
        .unwrap();
    assert!(
        verified.status.success(),
        "independent archive verification failed: {}",
        String::from_utf8_lossy(&verified.stderr)
    );
    assert!(String::from_utf8_lossy(&verified.stdout).contains("verification=passed"));

    let extracted = workspace.0.join("extracted");
    fs::create_dir(&extracted).unwrap();
    let unpack = Command::new("tar")
        .args(["-xzf"])
        .arg(&archive)
        .arg("-C")
        .arg(&extracted)
        .status()
        .unwrap();
    assert!(unpack.success());
    let root = extracted.join(&archive_name);
    let manifest = read_json(root.join("release-manifest.json"));
    let schema = read_json("schemas/release-manifest.schema.json");
    Validator::new(&schema)
        .unwrap()
        .validate(&manifest)
        .unwrap();
    assert_eq!(manifest["target"], target);
    let expected_dirty = if Path::new(".git").exists() {
        !Command::new("git")
            .args(["status", "--porcelain"])
            .output()
            .unwrap()
            .stdout
            .is_empty()
    } else {
        false
    };
    assert_eq!(manifest["source"]["dirty"], expected_dirty);
    assert_eq!(manifest["source"]["revision"].as_str().unwrap().len(), 40);
    assert_eq!(
        manifest["version_result"]["product"]["version"],
        product_version
    );
    assert_eq!(
        manifest["version_result"]["product_resources"]["origin"],
        "embedded"
    );

    for file in manifest["files"].as_array().unwrap() {
        let relative = file["path"].as_str().unwrap();
        assert!(!relative.starts_with('/'));
        assert!(!relative.split('/').any(|component| component == ".."));
        let bytes = fs::read(root.join(relative)).unwrap();
        assert_eq!(file["size_bytes"], bytes.len() as u64);
        assert_eq!(file["sha256"], dicom_test_suite::sha256_hex(&bytes));
    }
    let notices = read_json(root.join("THIRD_PARTY_LICENSES.json"));
    assert_eq!(notices["target"], target);
    let packages = notices["packages"].as_array().unwrap();
    assert!(!packages.is_empty());
    assert!(packages.iter().all(|package| {
        package["license"]
            .as_str()
            .is_some_and(|value| !value.is_empty())
            || package["license_file"]
                .as_str()
                .is_some_and(|value| !value.is_empty())
    }));

    let unrelated = workspace.0.join("unrelated");
    fs::create_dir(&unrelated).unwrap();
    let installed = root.join("bin/dicom-test-suite");
    for command in ["version", "capabilities"] {
        let output = Command::new(&installed)
            .current_dir(&unrelated)
            .args([command, "--format", "json"])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{command} failed after relocation: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let document: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(document["status"], "success");
    }
    let inventory = Command::new(&installed)
        .current_dir(&unrelated)
        .args(["list-cases", "--profile", "smoke", "--format", "json"])
        .output()
        .unwrap();
    assert!(inventory.status.success());
    assert_eq!(
        serde_json::from_slice::<Value>(&inventory.stdout).unwrap()["status"],
        "success"
    );
    let generated = workspace.0.join("generated");
    let generation = Command::new(&installed)
        .current_dir(&unrelated)
        .args(["generate", "--profile", "smoke", "--out"])
        .arg(&generated)
        .args(["--seed", "1"])
        .output()
        .unwrap();
    assert!(
        generation.status.success(),
        "relocated generation failed: {}",
        String::from_utf8_lossy(&generation.stderr)
    );
    let validation = Command::new(&installed)
        .current_dir(&unrelated)
        .arg("validate")
        .arg(&generated)
        .output()
        .unwrap();
    assert!(validation.status.success());
    let report = Command::new(&installed)
        .current_dir(&unrelated)
        .arg("report")
        .arg(&generated)
        .args(["--format", "json", "--cli-api", "1.0.0"])
        .output()
        .unwrap();
    assert!(
        report.status.success(),
        "relocated report failed: {}",
        String::from_utf8_lossy(&report.stderr)
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&report.stdout).unwrap()["status"],
        "success"
    );

    for (index, example) in [
        "compose-raw-grayscale.json",
        "compose-raw-rgb.json",
        "compose-metadata-private-sequence.json",
        "compose-multi-instance-reference.json",
    ]
    .into_iter()
    .enumerate()
    {
        assert_installed_example(
            &installed,
            &unrelated,
            &root.join("examples").join(example),
            "compose",
            &workspace.0.join(format!("example-{index}-first")),
            &workspace.0.join(format!("example-{index}-second")),
        );
    }
    assert_installed_example(
        &installed,
        &unrelated,
        &root.join("examples/assemble-structural.json"),
        "assemble",
        &workspace.0.join("assembly-example-first"),
        &workspace.0.join("assembly-example-second"),
    );
}
