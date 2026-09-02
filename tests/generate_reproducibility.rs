use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

#[test]
fn smoke_generation_is_byte_stable_across_two_output_roots() {
    assert_profile_is_reproducible("smoke");
}

#[test]
fn core_generation_is_byte_stable_across_two_output_roots() {
    assert_profile_is_reproducible("core");
}

#[test]
fn extended_generation_honors_declared_determinism_across_two_output_roots() {
    assert_profile_is_reproducible("extended");
}

fn assert_profile_is_reproducible(profile: &str) {
    let first_out = unique_temp_dir(&format!("generate-{profile}-reproducibility-a"));
    let second_out = unique_temp_dir(&format!("generate-{profile}-reproducibility-b"));

    let first_manifest = run_generate(&first_out, profile);
    let second_manifest = run_generate(&second_out, profile);

    for file in first_manifest["files"]
        .as_array()
        .expect("manifest files should be an array")
    {
        let path = file["path"]
            .as_str()
            .expect("file entry should have a path");
        let first_dcm =
            fs::read(first_out.join(path)).expect("first generated DICOM file should be readable");
        let second_dcm = fs::read(second_out.join(path))
            .expect("second generated DICOM file should be readable");
        if file["determinism"] == "byte_stable" {
            assert_eq!(
                first_dcm, second_dcm,
                "byte-stable DICOM bytes should match for {path}"
            );
        }
    }

    assert_eq!(
        deterministic_manifest_projection(&first_manifest)
            .pointer("/files")
            .cloned(),
        deterministic_manifest_projection(&second_manifest)
            .pointer("/files")
            .cloned(),
        "deterministic manifest metadata should be stable across output roots"
    );
    assert_eq!(
        first_manifest.pointer("/skipped_cases"),
        second_manifest.pointer("/skipped_cases"),
        "skipped-case metadata should be stable across output roots"
    );
    for (index, path) in generated_paths(&first_manifest).iter().enumerate() {
        let dcm_bytes =
            fs::read(first_out.join(path)).expect("generated DICOM file should be readable");
        assert_eq!(
            first_manifest.pointer(&format!("/files/{index}/sha256")),
            Some(&Value::String(synth_dicom_gen::sha256_hex(&dcm_bytes))),
            "manifest hash should match generated DICOM bytes"
        );
    }
    assert_eq!(
        first_manifest.pointer("/files/0/uids"),
        second_manifest.pointer("/files/0/uids"),
        "deterministic UID metadata should match across runs"
    );
    assert_eq!(
        deterministic_manifest_projection(&first_manifest),
        deterministic_manifest_projection(&second_manifest),
        "manifest deterministic projections should match across output roots"
    );

    for root in [&first_out, &second_out] {
        let validation =
            synth_dicom_gen::validate_generated_root(root).expect("generated root should validate");
        assert!(validation.failures.is_empty(), "{:?}", validation.failures);
    }

    fs::remove_dir_all(first_out).expect("first temporary output root should be removable");
    fs::remove_dir_all(second_out).expect("second temporary output root should be removable");
}

fn deterministic_manifest_projection(manifest: &Value) -> Value {
    let mut projection = manifest.clone();
    for file in projection["files"]
        .as_array_mut()
        .expect("manifest files should be an array")
    {
        if file["determinism"] == "semantic_stable" {
            let object = file
                .as_object_mut()
                .expect("manifest file should be an object");
            object.remove("sha256");
            object.remove("size_bytes");
            if let Some(backend) = object
                .get_mut("generation_backend")
                .and_then(Value::as_object_mut)
            {
                backend.remove("invocation_elapsed_milliseconds");
            }
        }
    }
    projection
}

fn run_generate(out_dir: &Path, profile: &str) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_synth-dicom-gen"))
        .args([
            "generate",
            "--profile",
            profile,
            "--out",
            out_dir.to_str().expect("temp path should be valid UTF-8"),
            "--seed",
            "7",
        ])
        .output()
        .expect("generate command must run");

    assert!(
        output.status.success(),
        "generate {profile} should exit successfully: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    serde_json::from_str(
        &fs::read_to_string(out_dir.join("manifest.json")).expect("manifest should be readable"),
    )
    .expect("manifest should parse")
}

fn generated_paths(manifest: &Value) -> Vec<&str> {
    manifest
        .pointer("/files")
        .and_then(Value::as_array)
        .expect("manifest files should be an array")
        .iter()
        .map(|file| {
            file.get("path")
                .and_then(Value::as_str)
                .expect("file entry should have a path")
        })
        .collect()
}

fn unique_temp_dir(name: &str) -> PathBuf {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock should be after epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "dicom-test-suite-{name}-{}-{nonce}",
        std::process::id()
    ))
}
