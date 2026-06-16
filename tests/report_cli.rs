use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;
use serde_json::json;

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
#[cfg(feature = "htj2k_openjph")]
fn report_command_counts_generated_htj2k_lossless_row() {
    let out_dir = unique_temp_dir("report-htj2k-json");
    generate_extended(&out_dir);

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
    let row = coverage_row(&report, "classic/sc/mono2_u16_htj2k_lossless");
    assert_eq!(row.get("status").and_then(Value::as_str), Some("generated"));
    assert_eq!(
        row.get("transfer_syntax").and_then(Value::as_str),
        Some("1.2.840.10008.1.2.4.201")
    );
    assert_eq!(
        row.get("validation_status").and_then(Value::as_str),
        Some("passed")
    );

    fs::remove_dir_all(out_dir).expect("temporary output root should be removable");
}

#[test]
fn report_projects_manifest_references_for_non_image_rows() {
    let out_dir = unique_temp_dir("report-non-image-references");
    fs::create_dir_all(&out_dir).expect("temporary output root should be created");
    let manifest = json!({
        "generated_at": "19700101000000.000000+0000",
        "standards": {
            "standards_lock_sha256": "0000000000000000000000000000000000000000000000000000000000000000"
        },
        "run": {
            "profile": "extended"
        },
        "files": [
            {
                "case_id": "derived/rwvm/linear_ct_mapping_explicit_le",
                "dicom": {
                    "iod_name": "Real World Value Mapping",
                    "sop_class_uid": "1.2.840.10008.5.1.4.1.1.67",
                    "transfer_syntax_uid": "1.2.840.10008.1.2.1"
                },
                "image": null,
                "pixel_data": null,
                "references": [
                    {
                        "relationship": "source_image",
                        "source_case_id": "enhanced/ct/multiframe_shared_perframe_explicit_le",
                        "source_path": "enhanced/ct/multiframe_shared_perframe_explicit_le/instance.dcm",
                        "sop_class_uid": "1.2.840.10008.5.1.4.1.1.2.1",
                        "sop_instance_uid": "2.25.1",
                        "series_instance_uid": "2.25.2",
                        "frame_numbers": [1, 2]
                    }
                ],
                "validation": {
                    "status": "passed"
                },
                "determinism": "byte_stable",
                "known_stressors": ["real_world_value_mapping"]
            }
        ],
        "skipped_cases": []
    });
    fs::write(
        out_dir.join("manifest.json"),
        serde_json::to_string_pretty(&manifest).expect("manifest should serialize"),
    )
    .expect("manifest should be writable");

    let report = dicom_test_suite::build_coverage_report(&out_dir)
        .expect("report should accept non-image manifest rows");
    let row = coverage_row(&report, "derived/rwvm/linear_ct_mapping_explicit_le");
    assert_eq!(
        row.get("photometric"),
        Some(&Value::Null),
        "non-image rows should not invent image metadata"
    );
    assert_eq!(
        row.pointer("/geometry/rows"),
        Some(&Value::Null),
        "non-image rows should keep geometry empty"
    );
    assert_eq!(
        row.get("derived_refs")
            .and_then(Value::as_array)
            .and_then(|refs| refs.first())
            .and_then(Value::as_str),
        Some("enhanced/ct/multiframe_shared_perframe_explicit_le")
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/object_types/derived")
            .and_then(Value::as_u64),
        Some(1)
    );

    fs::remove_dir_all(out_dir).expect("temporary output root should be removable");
}

#[test]
fn report_counts_feature_gated_planned_cases_as_planned() {
    let out_dir = unique_temp_dir("report-feature-gated-planned");
    fs::create_dir_all(&out_dir).expect("temporary output root should be created");
    let manifest = json!({
        "generated_at": "19700101000000.000000+0000",
        "standards": {
            "standards_lock_sha256": "0000000000000000000000000000000000000000000000000000000000000000"
        },
        "run": {
            "profile": "extended"
        },
        "files": [],
        "skipped_cases": [
            {
                "case_id": "classic/sc/mono2_u8_deflated_explicit_le",
                "status": "unavailable",
                "reason_code": "feature_gated_case_planned",
                "message": "This planned registry case requires Cargo feature(s) deflate.",
                "recheck_phase": "phase-6",
                "standards_evidence": []
            }
        ]
    });
    fs::write(
        out_dir.join("manifest.json"),
        serde_json::to_string_pretty(&manifest).expect("manifest should serialize"),
    )
    .expect("manifest should be writable");

    let report = dicom_test_suite::build_coverage_report(&out_dir)
        .expect("report should accept feature-gated planned rows");
    assert_eq!(
        report.pointer("/counts/planned").and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        report.pointer("/counts/skipped").and_then(Value::as_u64),
        Some(0)
    );
    let row = coverage_row(&report, "classic/sc/mono2_u8_deflated_explicit_le");
    assert_eq!(row.get("status").and_then(Value::as_str), Some("planned"));
    assert_eq!(
        row.get("transfer_syntax").and_then(Value::as_str),
        Some("1.2.840.10008.1.2.1.99")
    );

    fs::remove_dir_all(out_dir).expect("temporary output root should be removable");
}

#[test]
fn report_counts_feature_gated_implemented_cases_as_unavailable() {
    let out_dir = unique_temp_dir("report-feature-gated-implemented");
    fs::create_dir_all(&out_dir).expect("temporary output root should be created");
    let manifest = json!({
        "generated_at": "19700101000000.000000+0000",
        "standards": {
            "standards_lock_sha256": "0000000000000000000000000000000000000000000000000000000000000000"
        },
        "run": {
            "profile": "extended"
        },
        "files": [],
        "skipped_cases": [
            {
                "case_id": "classic/sc/mono2_u8_deflated_explicit_le",
                "status": "unavailable",
                "reason_code": "feature_gated_case_unavailable",
                "message": "This implemented registry case requires Cargo feature(s) deflate.",
                "recheck_phase": "phase-6",
                "standards_evidence": []
            }
        ]
    });
    fs::write(
        out_dir.join("manifest.json"),
        serde_json::to_string_pretty(&manifest).expect("manifest should serialize"),
    )
    .expect("manifest should be writable");

    let report = dicom_test_suite::build_coverage_report(&out_dir)
        .expect("report should accept feature-gated unavailable rows");
    assert_eq!(
        report.pointer("/counts/planned").and_then(Value::as_u64),
        Some(0)
    );
    assert_eq!(
        report.pointer("/counts/skipped").and_then(Value::as_u64),
        Some(1)
    );
    let row = coverage_row(&report, "classic/sc/mono2_u8_deflated_explicit_le");
    assert_eq!(
        row.get("status").and_then(Value::as_str),
        Some("unavailable")
    );

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

#[cfg(feature = "htj2k_openjph")]
fn generate_extended(out_dir: &Path) {
    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args([
            "generate",
            "--profile",
            "extended",
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
