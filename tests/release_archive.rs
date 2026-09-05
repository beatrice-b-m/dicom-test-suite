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

fn validate_release_manifest_schema(manifest: &Value) {
    match manifest["release_manifest_schema_version"].as_str() {
        Some("1.0.0") => {
            let schema = read_json("schemas/release-manifest.schema.json");
            Validator::new(&schema).unwrap().validate(manifest).unwrap();
        }
        Some(version @ ("2.0.0" | "3.0.0")) => {
            let schema = read_json(format!(
                "schemas/release-manifest-v{}.schema.json",
                &version[..1]
            ));
            let mut options = jsonschema::options();
            options = options.with_draft(jsonschema::Draft::Draft202012);
            for entry in fs::read_dir("schemas").unwrap() {
                let path = entry.unwrap().path();
                if path.extension().and_then(|s| s.to_str()) == Some("json") {
                    let resource = read_json(path);
                    let id = resource["$id"].as_str().unwrap().to_owned();
                    options = options
                        .with_resource(id, jsonschema::Resource::from_contents(resource).unwrap());
                }
            }
            options.build(&schema).unwrap().validate(manifest).unwrap();
        }
        version => panic!("unsupported release manifest schema version: {version:?}"),
    }
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
    let candidate_variables = [
        "SYNTH_DICOM_GEN_RELEASE_ARCHIVE",
        "SYNTH_DICOM_GEN_RELEASE_ARCHIVE_SHA256",
        "SYNTH_DICOM_GEN_RELEASE_BINARY",
        "SYNTH_DICOM_GEN_RELEASE_BINARY_SHA256",
        "SYNTH_DICOM_GEN_RELEASE_TARGET",
        "SYNTH_DICOM_GEN_RELEASE_REVISION",
        "SYNTH_DICOM_GEN_RELEASE_EXTRACTED_ROOT",
    ];
    let supplied_count = candidate_variables
        .iter()
        .filter(|name| std::env::var_os(name).is_some())
        .count();
    assert!(
        supplied_count == 0 || supplied_count == candidate_variables.len(),
        "release candidate environment must be entirely absent or supply archive, binary, extraction, hashes, target, and revision"
    );
    let supplied_candidate = supplied_count != 0;
    let binary = if supplied_candidate {
        PathBuf::from(std::env::var_os("SYNTH_DICOM_GEN_RELEASE_BINARY").unwrap())
    } else {
        PathBuf::from(env!("CARGO_BIN_EXE_synth-dicom-gen"))
            .canonicalize()
            .unwrap()
    };
    assert!(binary.is_absolute());
    let binary_sha256 = synth_dicom_gen::sha256_hex(&fs::read(&binary).unwrap());
    if supplied_candidate {
        assert_eq!(
            binary_sha256,
            std::env::var("SYNTH_DICOM_GEN_RELEASE_BINARY_SHA256").unwrap(),
            "supplied binary does not match its immutable identity"
        );
    }
    let version_output = Command::new(&binary)
        .args(["version", "--format", "json"])
        .output()
        .unwrap();
    assert!(version_output.status.success());
    let version: Value = serde_json::from_slice(&version_output.stdout).unwrap();
    let target = version["result"]["target"].as_str().unwrap();
    if supplied_candidate {
        assert_eq!(
            target,
            std::env::var("SYNTH_DICOM_GEN_RELEASE_TARGET").unwrap()
        );
    }
    let product_version = version["result"]["product"]["version"].as_str().unwrap();

    let workspace = TempRoot::new("build");
    let archive_name = format!("synth-dicom-gen-{product_version}-{target}");
    let (archive, expected_revision) = if supplied_candidate {
        let archive = PathBuf::from(std::env::var_os("SYNTH_DICOM_GEN_RELEASE_ARCHIVE").unwrap());
        assert!(archive.is_absolute());
        assert_eq!(
            archive.file_name().unwrap().to_str().unwrap(),
            format!("{archive_name}.tar.gz")
        );
        assert_eq!(
            synth_dicom_gen::sha256_hex(&fs::read(&archive).unwrap()),
            std::env::var("SYNTH_DICOM_GEN_RELEASE_ARCHIVE_SHA256").unwrap(),
            "supplied archive does not match its immutable identity"
        );
        (
            archive,
            std::env::var("SYNTH_DICOM_GEN_RELEASE_REVISION").unwrap(),
        )
    } else {
        let dist = workspace.0.join("dist");
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
        let build = Command::new("sh")
            .arg("scripts/build-release-archive.sh")
            .arg(target)
            .arg(&dist)
            .env("SYNTH_DICOM_GEN_RELEASE_BINARY", &binary)
            .env("SYNTH_DICOM_GEN_RELEASE_BINARY_SHA256", &binary_sha256)
            .env("SYNTH_DICOM_GEN_RELEASE_REVISION", &revision)
            .env("SYNTH_DICOM_GEN_RELEASE_TARGET", target)
            .env("SYNTH_DICOM_GEN_RELEASE_ALLOW_DIRTY", "1")
            .output()
            .unwrap();
        assert!(
            build.status.success(),
            "archive build failed: {}",
            String::from_utf8_lossy(&build.stderr)
        );
        (dist.join(format!("{archive_name}.tar.gz")), revision)
    };
    let checksum = PathBuf::from(format!("{}.sha256", archive.display()));
    assert!(archive.is_file());
    assert_eq!(
        fs::read_to_string(&checksum)
            .unwrap()
            .split_whitespace()
            .next()
            .unwrap(),
        synth_dicom_gen::sha256_hex(&fs::read(&archive).unwrap())
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

    let adversarial = workspace.0.join("adversarial");
    fs::create_dir(&adversarial).unwrap();
    let bad_checksum_archive = adversarial.join("bad-checksum.tar.gz");
    fs::copy(&archive, &bad_checksum_archive).unwrap();
    fs::write(
        PathBuf::from(format!("{}.sha256", bad_checksum_archive.display())),
        format!("{}  bad-checksum.tar.gz\n", "0".repeat(64)),
    )
    .unwrap();
    let bad_checksum = Command::new("sh")
        .arg("scripts/verify-release-archive.sh")
        .arg(&bad_checksum_archive)
        .output()
        .unwrap();
    assert!(!bad_checksum.status.success());
    assert!(String::from_utf8_lossy(&bad_checksum.stderr).contains("checksum does not match"));

    let tampered_archive = adversarial.join("tampered.tar.gz");
    let mut tampered_bytes = fs::read(&archive).unwrap();
    tampered_bytes.push(0);
    fs::write(&tampered_archive, tampered_bytes).unwrap();
    fs::write(
        PathBuf::from(format!("{}.sha256", tampered_archive.display())),
        format!(
            "{}  tampered.tar.gz\n",
            synth_dicom_gen::sha256_hex(&fs::read(&archive).unwrap())
        ),
    )
    .unwrap();
    let tampered = Command::new("sh")
        .arg("scripts/verify-release-archive.sh")
        .arg(&tampered_archive)
        .output()
        .unwrap();
    assert!(!tampered.status.success());
    assert!(String::from_utf8_lossy(&tampered.stderr).contains("checksum does not match"));

    let wrong_identity_root = adversarial.join("wrong-identity-root");
    fs::create_dir(&wrong_identity_root).unwrap();
    let unpack_wrong_identity = Command::new("tar")
        .args(["-xzf"])
        .arg(&archive)
        .arg("-C")
        .arg(&wrong_identity_root)
        .status()
        .unwrap();
    assert!(unpack_wrong_identity.success());
    let wrong_identity_payload = wrong_identity_root.join(&archive_name);
    let wrong_identity_manifest = wrong_identity_payload.join("release-manifest.json");
    let mut wrong_identity_document = read_json(&wrong_identity_manifest);
    wrong_identity_document["product"]["name"] = Value::String("dicom-test-suite".into());
    wrong_identity_document["version_result"]["product"]["name"] =
        Value::String("dicom-test-suite".into());
    fs::write(
        &wrong_identity_manifest,
        serde_json::to_vec_pretty(&wrong_identity_document).unwrap(),
    )
    .unwrap();
    let wrong_identity_archive = adversarial.join("wrong-identity.tar.gz");
    let repack_wrong_identity = Command::new("tar")
        .arg("-C")
        .arg(&wrong_identity_root)
        .arg("-czf")
        .arg(&wrong_identity_archive)
        .arg(&archive_name)
        .status()
        .unwrap();
    assert!(repack_wrong_identity.success());
    fs::write(
        PathBuf::from(format!("{}.sha256", wrong_identity_archive.display())),
        format!(
            "{}  wrong-identity.tar.gz\n",
            synth_dicom_gen::sha256_hex(&fs::read(&wrong_identity_archive).unwrap())
        ),
    )
    .unwrap();
    let wrong_identity = Command::new("sh")
        .arg("scripts/verify-release-archive.sh")
        .arg(&wrong_identity_archive)
        .output()
        .unwrap();
    assert_eq!(wrong_identity.status.code(), Some(4));
    assert!(
        String::from_utf8_lossy(&wrong_identity.stderr)
            .contains("release manifest product identity must be synth-dicom-gen")
    );

    let root = if supplied_candidate {
        let root =
            PathBuf::from(std::env::var_os("SYNTH_DICOM_GEN_RELEASE_EXTRACTED_ROOT").unwrap());
        assert!(root.is_absolute());
        assert_eq!(root.file_name().unwrap().to_str().unwrap(), archive_name);
        root
    } else {
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
        extracted.join(&archive_name)
    };
    let manifest = read_json(root.join("release-manifest.json"));
    validate_release_manifest_schema(&manifest);
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
    assert_eq!(manifest["source"]["revision"], expected_revision);
    assert_eq!(
        manifest["version_result"]["product"]["version"],
        product_version
    );
    assert_eq!(
        manifest["version_result"]["identity_domains"]["migration"]["legacy_resource_origin"],
        "embedded"
    );
    assert!(
        manifest["version_result"]
            .get("product_resources")
            .is_none()
    );

    for file in manifest["files"].as_array().unwrap() {
        let relative = file["path"].as_str().unwrap();
        assert!(!relative.starts_with('/'));
        assert!(!relative.split('/').any(|component| component == ".."));
        let bytes = fs::read(root.join(relative)).unwrap();
        assert_eq!(file["size_bytes"], bytes.len() as u64);
        assert_eq!(file["sha256"], synth_dicom_gen::sha256_hex(&bytes));
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
    let installed = root.join("bin/synth-dicom-gen");
    assert_eq!(
        synth_dicom_gen::sha256_hex(&fs::read(&installed).unwrap()),
        binary_sha256,
        "installed archive binary differs from the single qualified candidate binary"
    );
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
