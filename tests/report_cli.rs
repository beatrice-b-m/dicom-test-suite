use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

#[test]
fn report_command_writes_json_coverage_for_core_root() {
    let out_dir = unique_temp_dir("report-core-json");
    generate_core(&out_dir);

    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args([
            "report",
            out_dir.to_str().expect("temp path should be valid UTF-8"),
            "--format",
            "json",
        ])
        .output()
        .expect("report command must run");

    assert!(
        output.status.success(),
        "report should accept generated output: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: Value =
        serde_json::from_slice(&output.stdout).expect("report stdout should be JSON");
    assert_eq!(
        report
            .get("coverage_report_schema_version")
            .and_then(Value::as_str),
        Some("0.1.0")
    );
    assert_eq!(
        report.pointer("/counts/generated").and_then(Value::as_u64),
        Some(19)
    );
    assert_eq!(
        report.pointer("/counts/planned").and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        report
            .pointer("/coverage_matrix")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(21)
    );
    assert_eq!(
        coverage_row(&report, "classic/ct/mono2_i16_rescale_12bit_explicit_le")
            .get("status")
            .and_then(Value::as_str),
        Some("generated")
    );
    assert_eq!(
        coverage_row(&report, "vl/photo/rgb_planar0_explicit_le")
            .get("status")
            .and_then(Value::as_str),
        Some("planned")
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/profiles/core")
            .and_then(Value::as_u64),
        Some(21)
    );

    fs::remove_dir_all(out_dir).expect("temporary output root should be removable");
}

#[test]
fn report_command_writes_markdown_coverage_for_core_root() {
    let out_dir = unique_temp_dir("report-core-markdown");
    generate_core(&out_dir);

    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args([
            "report",
            out_dir.to_str().expect("temp path should be valid UTF-8"),
            "--format",
            "markdown",
        ])
        .output()
        .expect("report command must run");

    assert!(
        output.status.success(),
        "report should accept generated output: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("report stdout should be UTF-8");
    assert!(stdout.starts_with("# DICOM Test Suite Coverage Report"));
    assert!(stdout.contains("| generated | 19 |"));
    assert!(stdout.contains("| planned | 2 |"));
    assert!(stdout.contains("## Gaps"));
    assert!(stdout.contains("| case | vl/photo/rgb_planar0_explicit_le |"));
    assert!(
        stdout.contains("| classic/ct/mono2_i16_rescale_12bit_explicit_le | generated | core |")
    );
    assert!(stdout.contains("| vl/photo/palette_color_explicit_le | planned | core |"));

    fs::remove_dir_all(out_dir).expect("temporary output root should be removable");
}

#[test]
fn report_command_rejects_missing_manifest() {
    let out_dir = unique_temp_dir("report-missing-manifest");
    fs::create_dir_all(&out_dir).expect("temporary output root should be created");

    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args([
            "report",
            out_dir.to_str().expect("temp path should be valid UTF-8"),
            "--format",
            "json",
        ])
        .output()
        .expect("report command must run");

    assert!(
        !output.status.success(),
        "report should fail without a manifest"
    );
    let stderr = String::from_utf8(output.stderr).expect("report stderr must be UTF-8");
    assert!(stderr.contains("failed to read report metadata"));

    fs::remove_dir_all(out_dir).expect("temporary output root should be removable");
}

fn generate_core(out_dir: &Path) {
    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args([
            "generate",
            "--profile",
            "core",
            "--out",
            out_dir.to_str().expect("temp path should be valid UTF-8"),
            "--seed",
            "7",
        ])
        .output()
        .expect("generate command must run");

    assert!(
        output.status.success(),
        "generate should exit successfully: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn coverage_row<'a>(report: &'a Value, case_id: &str) -> &'a Value {
    report
        .pointer("/coverage_matrix")
        .and_then(Value::as_array)
        .expect("coverage matrix should be an array")
        .iter()
        .find(|row| row.get("case_id").and_then(Value::as_str) == Some(case_id))
        .unwrap_or_else(|| panic!("coverage matrix should contain {case_id}"))
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
