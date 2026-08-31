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

fn normalized_outcome(bytes: &[u8]) -> Value {
    let mut value: Value = serde_json::from_slice(bytes).unwrap();
    let result = value["result"].as_object_mut().unwrap();
    result.remove("requested_output_root");
    result.remove("manifest_path");
    value
}

fn assert_instance_bytes_match(
    first_root: &Path,
    second_root: &Path,
    entries: &[Value],
    path_key: &str,
) {
    for entry in entries {
        let relative = entry[path_key].as_str().unwrap();
        assert_eq!(
            fs::read(first_root.join(relative)).unwrap(),
            fs::read(second_root.join(relative)).unwrap(),
            "parallelism or working directory changed {relative} bytes"
        );
    }
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

    let black_box = Command::new("python3")
        .arg("tests/black_box_cli_consumer.py")
        .arg(&installed)
        .arg(&root)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .unwrap();
    assert!(
        black_box.status.success(),
        "installed black-box consumer failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&black_box.stdout),
        String::from_utf8_lossy(&black_box.stderr)
    );
    assert_eq!(
        String::from_utf8(black_box.stdout).unwrap(),
        "black-box CLI API 1.0.0 consumer passed\n"
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

    let cwd_a = workspace.0.join("determinism-cwd-a/nested");
    let cwd_b = workspace.0.join("determinism-cwd-b/other");
    fs::create_dir_all(&cwd_a).unwrap();
    fs::create_dir_all(&cwd_b).unwrap();
    let instances = (0..12)
        .map(|index| {
            serde_json::json!({
                "instance_id": format!("instance-{index:02}"),
                "template": {"id":"classic/secondary-capture/monochrome"}
            })
        })
        .collect::<Vec<_>>();
    let sequential_spec = cwd_a.join("composition.json");
    let parallel_spec = cwd_b.join("composition.json");
    fs::write(
        &sequential_spec,
        serde_json::to_vec_pretty(&serde_json::json!({
            "composition_spec_schema_version":"0.1.0",
            "parallelism":1,
            "instances":instances
        }))
        .unwrap(),
    )
    .unwrap();
    fs::write(
        &parallel_spec,
        serde_json::to_vec_pretty(&serde_json::json!({
            "composition_spec_schema_version":"0.1.0",
            "parallelism":8,
            "instances":instances
        }))
        .unwrap(),
    )
    .unwrap();
    let composition_a = workspace.0.join("determinism-composition-a");
    let composition_b = workspace.0.join("determinism-composition-b");
    let run_composition = |cwd: &Path, spec: &Path, output_root: &Path| {
        Command::new(&installed)
            .current_dir(cwd)
            .args(["compose", "--spec"])
            .arg(spec)
            .arg("--out")
            .arg(output_root)
            .args(["--seed", "91", "--format", "json"])
            .output()
            .unwrap()
    };
    let composition_outcome_a = run_composition(&cwd_a, &sequential_spec, &composition_a);
    let composition_outcome_b = run_composition(&cwd_b, &parallel_spec, &composition_b);
    assert!(composition_outcome_a.status.success());
    assert!(composition_outcome_b.status.success());
    assert_eq!(
        normalized_outcome(&composition_outcome_a.stdout),
        normalized_outcome(&composition_outcome_b.stdout)
    );
    let composition_manifest_a = read_json(composition_a.join("manifest.json"));
    let composition_manifest_b = read_json(composition_b.join("manifest.json"));
    assert_eq!(
        composition_manifest_a["composition"],
        composition_manifest_b["composition"]
    );
    assert_eq!(
        composition_manifest_a["run"]["corpus_plan_sha256"],
        composition_manifest_b["run"]["corpus_plan_sha256"]
    );
    assert_instance_bytes_match(
        &composition_a,
        &composition_b,
        composition_manifest_a["composition"]["entries"]
            .as_array()
            .unwrap(),
        "path",
    );

    let assembly_a = workspace.0.join("determinism-assembly-a");
    let assembly_b = workspace.0.join("determinism-assembly-b");
    let assembly_request = root.join("examples/assemble-structural.json");
    let run_assembly = |cwd: &Path, output_root: &Path, parallelism: &str| {
        Command::new(&installed)
            .current_dir(cwd)
            .args(["assemble", "--request"])
            .arg(&assembly_request)
            .arg("--out")
            .arg(output_root)
            .args([
                "--seed",
                "92",
                "--parallelism",
                parallelism,
                "--format",
                "json",
            ])
            .output()
            .unwrap()
    };
    let assembly_outcome_a = run_assembly(&cwd_a, &assembly_a, "1");
    let assembly_outcome_b = run_assembly(&cwd_b, &assembly_b, "8");
    assert!(assembly_outcome_a.status.success());
    assert!(assembly_outcome_b.status.success());
    assert_eq!(
        normalized_outcome(&assembly_outcome_a.stdout),
        normalized_outcome(&assembly_outcome_b.stdout)
    );
    let assembly_manifest_a = read_json(assembly_a.join("manifest.json"));
    let assembly_manifest_b = read_json(assembly_b.join("manifest.json"));
    assert_eq!(
        assembly_manifest_a["instances"],
        assembly_manifest_b["instances"]
    );
    assert_eq!(
        assembly_manifest_a["run"]["corpus_plan_sha256"],
        assembly_manifest_b["run"]["corpus_plan_sha256"]
    );
    assert_instance_bytes_match(
        &assembly_a,
        &assembly_b,
        assembly_manifest_a["instances"].as_array().unwrap(),
        "output_path",
    );
}
