use std::process::Command;

use serde_json::Value;

#[test]
fn report_gaps_counts_logical_cases_and_dimensions() {
    let output = run_gap_report("json");
    assert!(
        output.status.success(),
        "gap report should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: Value =
        serde_json::from_slice(&output.stdout).expect("gap report should be valid JSON");

    assert_eq!(
        report
            .pointer("/counts/logical_cases")
            .and_then(Value::as_u64),
        Some(181)
    );
    assert_eq!(
        report
            .pointer("/counts/statuses/implemented")
            .and_then(Value::as_u64),
        Some(131)
    );
    assert_eq!(
        report
            .pointer("/counts/statuses/planned")
            .and_then(Value::as_u64),
        Some(50)
    );
    assert!(
        report.pointer("/counts/priorities/now").is_none(),
        "all phase-1 now-priority gaps should be promoted"
    );
    assert!(
        report
            .get("gaps")
            .and_then(Value::as_array)
            .is_some_and(|gaps| !gaps.iter().any(|gap| {
                gap.get("case_id").and_then(Value::as_str)
                    == Some("classic/us/multiframe_explicit_le")
            })),
        "promoted ultrasound multi-frame coverage must not remain a gap"
    );
    assert!(
        report
            .get("gaps")
            .and_then(Value::as_array)
            .is_some_and(|gaps| !gaps.iter().any(|gap| {
                gap.get("case_id").and_then(Value::as_str)
                    == Some("classic/xa/monoplane_explicit_le")
            })),
        "promoted XA monoplane coverage must not remain a gap"
    );
    assert!(
        report
            .get("gaps")
            .and_then(Value::as_array)
            .is_some_and(|gaps| !gaps.iter().any(|gap| {
                gap.get("case_id").and_then(Value::as_str)
                    == Some("classic/xrf/monoplane_explicit_le")
            })),
        "promoted XRF monoplane coverage must not remain a gap"
    );
    assert!(
        report
            .get("gaps")
            .and_then(Value::as_array)
            .is_some_and(|gaps| !gaps.iter().any(|gap| {
                gap.get("case_id").and_then(Value::as_str)
                    == Some("classic/sc/nonsquare_pixel_spacing")
            })),
        "promoted non-square spatial coverage must not remain a gap"
    );
    assert!(
        report
            .get("gaps")
            .and_then(Value::as_array)
            .is_some_and(|gaps| !gaps.iter().any(|gap| {
                gap.get("case_id").and_then(Value::as_str)
                    == Some("classic/sc/mono2_u32_explicit_le")
            })),
        "independently conformant u32 coverage must not remain a gap"
    );
    assert!(
        report
            .get("gaps")
            .and_then(Value::as_array)
            .is_some_and(|gaps| !gaps.iter().any(|gap| {
                gap.get("case_id").and_then(Value::as_str) == Some("classic/sc/mono2_u1_native")
            })),
        "independently conformant u1 coverage must not remain a gap"
    );
    assert!(
        report
            .get("gaps")
            .and_then(Value::as_array)
            .is_some_and(|gaps| !gaps.iter().any(|gap| {
                gap.get("case_id").and_then(Value::as_str)
                    == Some("enhanced/pet/multiframe_explicit_le")
            })),
        "promoted Enhanced PET coverage must not remain a gap"
    );
    assert!(
        report
            .get("gaps")
            .and_then(Value::as_array)
            .is_some_and(|gaps| !gaps.iter().any(|gap| {
                gap.get("case_id").and_then(Value::as_str)
                    == Some("derived/sr/tid1500_ct_measurement_report")
            })),
        "promoted TID 1500 measurement coverage must not remain a gap"
    );
    for pointer in [
        "/dimensions/sop_classes",
        "/dimensions/modalities",
        "/dimensions/object_families",
        "/dimensions/compatibility_axes",
    ] {
        assert!(
            report
                .pointer(pointer)
                .and_then(Value::as_array)
                .is_some_and(|rows| !rows.is_empty()),
            "gap report must populate {pointer}"
        );
    }
    assert!(
        report
            .get("gaps")
            .and_then(Value::as_array)
            .is_some_and(|gaps| gaps.iter().any(|gap| {
                gap.get("case_id").and_then(Value::as_str)
                    == Some("protocol/dicomweb/stow_qido_wado")
                    && gap.get("artifact_kind").and_then(Value::as_str)
                        == Some("transaction_scenario")
            })),
        "protocol gaps must be visible but distinct from file cases"
    );
}

#[test]
fn report_gaps_is_byte_stable_for_unchanged_inputs() {
    let first = run_gap_report("json");
    let second = run_gap_report("json");
    assert!(first.status.success() && second.status.success());
    assert_eq!(first.stdout, second.stdout);
}

#[test]
fn report_gaps_renders_markdown_from_the_same_model() {
    let output = run_gap_report("markdown");
    assert!(output.status.success());
    let markdown = String::from_utf8(output.stdout).expect("markdown should be utf-8");
    assert!(markdown.contains("# DICOM Registry Coverage Gap Report"));
    assert!(!markdown.contains("| now |"));
    assert!(!markdown.contains("derived/parametric-map/float32_ct_derived_explicit_le"));
    assert!(!markdown.contains("derived/parametric-map/float64_ct_derived_explicit_le"));
    assert!(!markdown.contains("derived/sr/tid1500_ct_measurement_report"));
    assert!(markdown.contains("protocol/dicomweb/stow_qido_wado"));
}

#[test]
fn report_gaps_validates_against_the_committed_schema() {
    let output = run_gap_report("json");
    assert!(output.status.success());
    let report: Value = serde_json::from_slice(&output.stdout).expect("report should parse");
    let schema: Value = serde_json::from_str(
        &std::fs::read_to_string("schemas/coverage-gap-report.schema.json")
            .expect("coverage gap schema should be readable"),
    )
    .expect("coverage gap schema should parse");
    let validator = jsonschema::validator_for(&schema).expect("coverage gap schema should compile");
    let errors = validator
        .iter_errors(&report)
        .map(|error| error.to_string())
        .collect::<Vec<_>>();
    assert!(errors.is_empty(), "gap report schema errors: {errors:?}");
}

fn run_gap_report(format: &str) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args(["report", "gaps", "--format", format])
        .output()
        .expect("report gaps command must run")
}
