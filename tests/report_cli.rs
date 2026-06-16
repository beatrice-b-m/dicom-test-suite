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
fn report_command_counts_generated_rgb_rle_lossless_row() {
    let out_dir = unique_temp_dir("report-rgb-rle-json");
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
    let row = coverage_row(&report, "classic/sc/rgb_planar0_rle_lossless");
    assert_eq!(row.get("status").and_then(Value::as_str), Some("generated"));
    assert_eq!(
        row.get("transfer_syntax").and_then(Value::as_str),
        Some("1.2.840.10008.1.2.5")
    );
    assert_eq!(
        row.get("codec_family").and_then(Value::as_str),
        Some("RLE Lossless")
    );
    assert_eq!(
        row.get("codec_backend_id").and_then(Value::as_str),
        Some("native_project_rle_encoder")
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/codec_families/RLE Lossless")
            .and_then(Value::as_u64),
        Some(39)
    );
    let mono1_row = coverage_row(&report, "classic/sc/mono1_u8_rle_lossless");
    assert_eq!(
        mono1_row.get("status").and_then(Value::as_str),
        Some("generated")
    );
    assert_eq!(
        mono1_row.get("codec_backend_id").and_then(Value::as_str),
        Some("native_project_rle_encoder")
    );
    assert_eq!(
        mono1_row.get("photometric").and_then(Value::as_str),
        Some("MONOCHROME1")
    );
    let mono1_u16_row = coverage_row(&report, "classic/sc/mono1_u16_rle_lossless");
    assert_eq!(
        mono1_u16_row.get("status").and_then(Value::as_str),
        Some("generated")
    );
    assert_eq!(
        mono1_u16_row
            .get("codec_backend_id")
            .and_then(Value::as_str),
        Some("native_project_rle_encoder")
    );
    assert_eq!(
        mono1_u16_row.get("photometric").and_then(Value::as_str),
        Some("MONOCHROME1")
    );
    assert_eq!(mono1_u16_row.get("bits").and_then(Value::as_u64), Some(16));
    let signed_row = coverage_row(&report, "classic/sc/mono2_i16_rle_lossless");
    assert_eq!(
        signed_row.get("status").and_then(Value::as_str),
        Some("generated")
    );
    assert_eq!(
        signed_row.get("codec_backend_id").and_then(Value::as_str),
        Some("native_project_rle_encoder")
    );
    assert_eq!(
        signed_row.get("photometric").and_then(Value::as_str),
        Some("MONOCHROME2")
    );
    let mono1_signed_row = coverage_row(&report, "classic/sc/mono1_i16_rle_lossless");
    assert_eq!(
        mono1_signed_row.get("status").and_then(Value::as_str),
        Some("generated")
    );
    assert_eq!(
        mono1_signed_row
            .get("codec_backend_id")
            .and_then(Value::as_str),
        Some("native_project_rle_encoder")
    );
    assert_eq!(
        mono1_signed_row.get("photometric").and_then(Value::as_str),
        Some("MONOCHROME1")
    );
    assert_eq!(
        mono1_signed_row.get("bits").and_then(Value::as_u64),
        Some(16)
    );
    let tiny_row = coverage_row(&report, "classic/sc/mono2_u16_tiny_1x1_rle_lossless");
    assert_eq!(
        tiny_row.get("status").and_then(Value::as_str),
        Some("generated")
    );
    assert_eq!(
        tiny_row.get("codec_backend_id").and_then(Value::as_str),
        Some("native_project_rle_encoder")
    );
    assert_eq!(
        tiny_row.pointer("/geometry/rows").and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        tiny_row
            .pointer("/geometry/columns")
            .and_then(Value::as_u64),
        Some(1)
    );
    let padding_row = coverage_row(&report, "classic/sc/mono2_u16_padding_rle_lossless");
    assert_eq!(
        padding_row.get("status").and_then(Value::as_str),
        Some("generated")
    );
    assert_eq!(
        padding_row.get("codec_backend_id").and_then(Value::as_str),
        Some("native_project_rle_encoder")
    );
    assert_eq!(
        padding_row.get("photometric").and_then(Value::as_str),
        Some("MONOCHROME2")
    );
    let padding_multiframe_row = coverage_row(
        &report,
        "classic/sc/mono2_u16_padding_multiframe_rle_lossless",
    );
    assert_eq!(
        padding_multiframe_row.get("status").and_then(Value::as_str),
        Some("generated")
    );
    assert_eq!(
        padding_multiframe_row
            .get("codec_backend_id")
            .and_then(Value::as_str),
        Some("native_project_rle_encoder")
    );
    assert_eq!(
        padding_multiframe_row
            .get("photometric")
            .and_then(Value::as_str),
        Some("MONOCHROME2")
    );
    assert_eq!(
        padding_multiframe_row.get("bits").and_then(Value::as_u64),
        Some(16)
    );
    assert_eq!(
        padding_multiframe_row.get("frames").and_then(Value::as_u64),
        Some(2)
    );
    let signed_padding_row = coverage_row(&report, "classic/sc/mono2_i16_padding_rle_lossless");
    assert_eq!(
        signed_padding_row.get("status").and_then(Value::as_str),
        Some("generated")
    );
    assert_eq!(
        signed_padding_row
            .get("codec_backend_id")
            .and_then(Value::as_str),
        Some("native_project_rle_encoder")
    );
    assert_eq!(
        signed_padding_row
            .get("photometric")
            .and_then(Value::as_str),
        Some("MONOCHROME2")
    );
    assert_eq!(
        signed_padding_row.get("bits").and_then(Value::as_u64),
        Some(16)
    );
    let odd_3x3_row = coverage_row(&report, "classic/sc/mono2_u16_odd_3x3_rle_lossless");
    assert_eq!(
        odd_3x3_row.get("status").and_then(Value::as_str),
        Some("generated")
    );
    assert_eq!(
        odd_3x3_row.get("codec_backend_id").and_then(Value::as_str),
        Some("native_project_rle_encoder")
    );
    assert_eq!(
        odd_3x3_row
            .pointer("/geometry/rows")
            .and_then(Value::as_u64),
        Some(3)
    );
    assert_eq!(
        odd_3x3_row
            .pointer("/geometry/columns")
            .and_then(Value::as_u64),
        Some(3)
    );
    let rect_row = coverage_row(&report, "classic/sc/mono2_u16_rect_2x3_rle_lossless");
    assert_eq!(
        rect_row.get("status").and_then(Value::as_str),
        Some("generated")
    );
    assert_eq!(
        rect_row.get("codec_backend_id").and_then(Value::as_str),
        Some("native_project_rle_encoder")
    );
    assert_eq!(
        rect_row.pointer("/geometry/rows").and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        rect_row
            .pointer("/geometry/columns")
            .and_then(Value::as_u64),
        Some(3)
    );
    let planar1_row = coverage_row(&report, "classic/sc/rgb_planar1_rle_lossless");
    assert_eq!(
        planar1_row.get("status").and_then(Value::as_str),
        Some("generated")
    );
    assert_eq!(
        planar1_row.get("codec_backend_id").and_then(Value::as_str),
        Some("native_project_rle_encoder")
    );
    let rgb_multiframe_row =
        coverage_row(&report, "classic/sc/rgb_planar0_multiframe_rle_lossless");
    assert_eq!(
        rgb_multiframe_row.get("status").and_then(Value::as_str),
        Some("generated")
    );
    assert_eq!(
        rgb_multiframe_row
            .get("codec_backend_id")
            .and_then(Value::as_str),
        Some("native_project_rle_encoder")
    );
    assert_eq!(
        rgb_multiframe_row
            .get("photometric")
            .and_then(Value::as_str),
        Some("RGB")
    );
    assert_eq!(
        rgb_multiframe_row.get("frames").and_then(Value::as_u64),
        Some(2)
    );
    let rgb_planar1_multiframe_row =
        coverage_row(&report, "classic/sc/rgb_planar1_multiframe_rle_lossless");
    assert_eq!(
        rgb_planar1_multiframe_row
            .get("status")
            .and_then(Value::as_str),
        Some("generated")
    );
    assert_eq!(
        rgb_planar1_multiframe_row
            .get("codec_backend_id")
            .and_then(Value::as_str),
        Some("native_project_rle_encoder")
    );
    assert_eq!(
        rgb_planar1_multiframe_row
            .get("photometric")
            .and_then(Value::as_str),
        Some("RGB")
    );
    assert_eq!(
        rgb_planar1_multiframe_row
            .get("frames")
            .and_then(Value::as_u64),
        Some(2)
    );
    let ybr_row = coverage_row(&report, "classic/sc/ybr_full_planar0_rle_lossless");
    assert_eq!(
        ybr_row.get("status").and_then(Value::as_str),
        Some("generated")
    );
    assert_eq!(
        ybr_row.get("codec_backend_id").and_then(Value::as_str),
        Some("native_project_rle_encoder")
    );
    assert!(
        ybr_row
            .get("known_stressors")
            .and_then(Value::as_array)
            .expect("YBR_FULL RLE row should include known stressors")
            .iter()
            .any(|stressor| stressor.as_str() == Some("ybr_full_pixels")),
        "YBR_FULL RLE report row should retain YBR_FULL pixel stressor"
    );
    let ybr_planar1_row = coverage_row(&report, "classic/sc/ybr_full_planar1_rle_lossless");
    assert_eq!(
        ybr_planar1_row.get("status").and_then(Value::as_str),
        Some("generated")
    );
    assert_eq!(
        ybr_planar1_row
            .get("codec_backend_id")
            .and_then(Value::as_str),
        Some("native_project_rle_encoder")
    );
    assert!(
        ybr_planar1_row
            .get("known_stressors")
            .and_then(Value::as_array)
            .expect("YBR_FULL planar-1 RLE row should include known stressors")
            .iter()
            .any(|stressor| stressor.as_str() == Some("ybr_full_pixels")),
        "YBR_FULL planar-1 RLE report row should retain YBR_FULL pixel stressor"
    );
    let ybr_multiframe_row = coverage_row(
        &report,
        "classic/sc/ybr_full_planar0_multiframe_rle_lossless",
    );
    assert_eq!(
        ybr_multiframe_row.get("status").and_then(Value::as_str),
        Some("generated")
    );
    assert_eq!(
        ybr_multiframe_row
            .get("codec_backend_id")
            .and_then(Value::as_str),
        Some("native_project_rle_encoder")
    );
    assert_eq!(
        ybr_multiframe_row
            .get("photometric")
            .and_then(Value::as_str),
        Some("YBR_FULL")
    );
    assert_eq!(
        ybr_multiframe_row.get("frames").and_then(Value::as_u64),
        Some(2)
    );
    assert!(
        ybr_multiframe_row
            .get("known_stressors")
            .and_then(Value::as_array)
            .expect("YBR_FULL multi-frame RLE row should include known stressors")
            .iter()
            .any(|stressor| stressor.as_str() == Some("ybr_full_pixels")),
        "YBR_FULL multi-frame RLE report row should retain YBR_FULL pixel stressor"
    );
    let ybr_planar1_multiframe_row = coverage_row(
        &report,
        "classic/sc/ybr_full_planar1_multiframe_rle_lossless",
    );
    assert_eq!(
        ybr_planar1_multiframe_row
            .get("status")
            .and_then(Value::as_str),
        Some("generated")
    );
    assert_eq!(
        ybr_planar1_multiframe_row
            .get("codec_backend_id")
            .and_then(Value::as_str),
        Some("native_project_rle_encoder")
    );
    assert_eq!(
        ybr_planar1_multiframe_row
            .get("photometric")
            .and_then(Value::as_str),
        Some("YBR_FULL")
    );
    assert_eq!(
        ybr_planar1_multiframe_row
            .get("frames")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert!(
        ybr_planar1_multiframe_row
            .get("known_stressors")
            .and_then(Value::as_array)
            .expect("YBR_FULL planar-1 multi-frame RLE row should include known stressors")
            .iter()
            .any(|stressor| stressor.as_str() == Some("ybr_full_pixels")),
        "YBR_FULL planar-1 multi-frame RLE report row should retain YBR_FULL pixel stressor"
    );
    let palette_row = coverage_row(&report, "classic/sc/palette_color_u8_rle_lossless");
    assert_eq!(
        palette_row.get("status").and_then(Value::as_str),
        Some("generated")
    );
    assert_eq!(
        palette_row.get("codec_backend_id").and_then(Value::as_str),
        Some("native_project_rle_encoder")
    );
    assert!(
        palette_row
            .get("known_stressors")
            .and_then(Value::as_array)
            .expect("PALETTE COLOR RLE row should include known stressors")
            .iter()
            .any(|stressor| stressor.as_str() == Some("palette_color_pixels")),
        "PALETTE COLOR RLE report row should retain palette color stressor"
    );
    let palette_multiframe_row = coverage_row(
        &report,
        "classic/sc/palette_color_u8_multiframe_rle_lossless",
    );
    assert_eq!(
        palette_multiframe_row.get("status").and_then(Value::as_str),
        Some("generated")
    );
    assert_eq!(
        palette_multiframe_row
            .get("codec_backend_id")
            .and_then(Value::as_str),
        Some("native_project_rle_encoder")
    );
    assert_eq!(
        palette_multiframe_row
            .get("photometric")
            .and_then(Value::as_str),
        Some("PALETTE COLOR")
    );
    assert_eq!(
        palette_multiframe_row.get("frames").and_then(Value::as_u64),
        Some(2)
    );
    assert!(
        palette_multiframe_row
            .get("known_stressors")
            .and_then(Value::as_array)
            .expect("PALETTE COLOR multi-frame RLE row should include known stressors")
            .iter()
            .any(|stressor| stressor.as_str() == Some("palette_color_pixels")),
        "PALETTE COLOR multi-frame RLE report row should retain palette color stressor"
    );
    let multiframe_row = coverage_row(&report, "classic/sc/mono2_u8_multiframe_rle_lossless");
    assert_eq!(
        multiframe_row.get("status").and_then(Value::as_str),
        Some("generated")
    );
    assert_eq!(
        multiframe_row
            .get("codec_backend_id")
            .and_then(Value::as_str),
        Some("native_project_rle_encoder")
    );
    let mono1_multiframe_row = coverage_row(&report, "classic/sc/mono1_u8_multiframe_rle_lossless");
    assert_eq!(
        mono1_multiframe_row.get("status").and_then(Value::as_str),
        Some("generated")
    );
    assert_eq!(
        mono1_multiframe_row
            .get("codec_backend_id")
            .and_then(Value::as_str),
        Some("native_project_rle_encoder")
    );
    assert_eq!(
        mono1_multiframe_row
            .get("photometric")
            .and_then(Value::as_str),
        Some("MONOCHROME1")
    );
    assert_eq!(
        mono1_multiframe_row.get("frames").and_then(Value::as_u64),
        Some(2)
    );
    let u16_multiframe_row = coverage_row(&report, "classic/sc/mono2_u16_multiframe_rle_lossless");
    assert_eq!(
        u16_multiframe_row.get("status").and_then(Value::as_str),
        Some("generated")
    );
    assert_eq!(
        u16_multiframe_row
            .get("codec_backend_id")
            .and_then(Value::as_str),
        Some("native_project_rle_encoder")
    );
    assert_eq!(
        u16_multiframe_row.get("bits").and_then(Value::as_u64),
        Some(16)
    );
    let mono1_u16_multiframe_row =
        coverage_row(&report, "classic/sc/mono1_u16_multiframe_rle_lossless");
    assert_eq!(
        mono1_u16_multiframe_row
            .get("status")
            .and_then(Value::as_str),
        Some("generated")
    );
    assert_eq!(
        mono1_u16_multiframe_row
            .get("codec_backend_id")
            .and_then(Value::as_str),
        Some("native_project_rle_encoder")
    );
    assert_eq!(
        mono1_u16_multiframe_row
            .get("photometric")
            .and_then(Value::as_str),
        Some("MONOCHROME1")
    );
    assert_eq!(
        mono1_u16_multiframe_row.get("bits").and_then(Value::as_u64),
        Some(16)
    );
    assert_eq!(
        mono1_u16_multiframe_row
            .get("frames")
            .and_then(Value::as_u64),
        Some(2)
    );
    let i16_multiframe_row = coverage_row(&report, "classic/sc/mono2_i16_multiframe_rle_lossless");
    assert_eq!(
        i16_multiframe_row.get("status").and_then(Value::as_str),
        Some("generated")
    );
    assert_eq!(
        i16_multiframe_row
            .get("codec_backend_id")
            .and_then(Value::as_str),
        Some("native_project_rle_encoder")
    );
    assert_eq!(
        i16_multiframe_row
            .get("photometric")
            .and_then(Value::as_str),
        Some("MONOCHROME2")
    );
    assert_eq!(
        i16_multiframe_row.get("bits").and_then(Value::as_u64),
        Some(16)
    );
    assert_eq!(
        i16_multiframe_row.get("frames").and_then(Value::as_u64),
        Some(2)
    );
    let mono1_i16_multiframe_row =
        coverage_row(&report, "classic/sc/mono1_i16_multiframe_rle_lossless");
    assert_eq!(
        mono1_i16_multiframe_row
            .get("status")
            .and_then(Value::as_str),
        Some("generated")
    );
    assert_eq!(
        mono1_i16_multiframe_row
            .get("codec_backend_id")
            .and_then(Value::as_str),
        Some("native_project_rle_encoder")
    );
    assert_eq!(
        mono1_i16_multiframe_row
            .get("photometric")
            .and_then(Value::as_str),
        Some("MONOCHROME1")
    );
    assert_eq!(
        mono1_i16_multiframe_row.get("bits").and_then(Value::as_u64),
        Some(16)
    );
    assert_eq!(
        mono1_i16_multiframe_row
            .get("frames")
            .and_then(Value::as_u64),
        Some(2)
    );
    let odd_fragment_row = coverage_row(&report, "classic/sc/mono2_u8_odd_fragment_rle_lossless");
    assert_eq!(
        odd_fragment_row.get("status").and_then(Value::as_str),
        Some("generated")
    );
    assert_eq!(
        odd_fragment_row
            .get("codec_backend_id")
            .and_then(Value::as_str),
        Some("native_project_rle_encoder")
    );
    let ct_row = coverage_row(&report, "classic/ct/mono2_i16_rescale_12bit_rle_lossless");
    assert_eq!(
        ct_row.get("status").and_then(Value::as_str),
        Some("generated")
    );
    assert_eq!(
        ct_row.get("codec_backend_id").and_then(Value::as_str),
        Some("native_project_rle_encoder")
    );
    let mr_row = coverage_row(&report, "classic/mr/mono2_u16_rle_lossless");
    assert_eq!(
        mr_row.get("status").and_then(Value::as_str),
        Some("generated")
    );
    assert_eq!(
        mr_row.get("codec_backend_id").and_then(Value::as_str),
        Some("native_project_rle_encoder")
    );
    let cr_row = coverage_row(&report, "classic/cr/overlay_modality_voi_rle_lossless");
    assert_eq!(
        cr_row.get("status").and_then(Value::as_str),
        Some("generated")
    );
    assert_eq!(
        cr_row.get("codec_backend_id").and_then(Value::as_str),
        Some("native_project_rle_encoder")
    );
    let dx_row = coverage_row(&report, "classic/dx/display_shutter_mono2_u16_rle_lossless");
    assert_eq!(
        dx_row.get("status").and_then(Value::as_str),
        Some("generated")
    );
    assert_eq!(
        dx_row.get("codec_backend_id").and_then(Value::as_str),
        Some("native_project_rle_encoder")
    );
    let mg_row = coverage_row(
        &report,
        "classic/mg/for_presentation_mono1_u16_12bit_rle_lossless",
    );
    assert_eq!(
        mg_row.get("status").and_then(Value::as_str),
        Some("generated")
    );
    assert_eq!(
        mg_row.get("codec_backend_id").and_then(Value::as_str),
        Some("native_project_rle_encoder")
    );
    let mg_processing_row = coverage_row(
        &report,
        "classic/mg/for_processing_mono2_u16_12bit_rle_lossless",
    );
    assert_eq!(
        mg_processing_row.get("status").and_then(Value::as_str),
        Some("generated")
    );
    assert_eq!(
        mg_processing_row
            .get("codec_backend_id")
            .and_then(Value::as_str),
        Some("native_project_rle_encoder")
    );
    let us_row = coverage_row(&report, "classic/us/mono2_u8_rle_lossless");
    assert_eq!(
        us_row.get("status").and_then(Value::as_str),
        Some("generated")
    );
    assert_eq!(
        us_row.get("codec_backend_id").and_then(Value::as_str),
        Some("native_project_rle_encoder")
    );
    assert!(
        us_row
            .get("known_stressors")
            .and_then(Value::as_array)
            .expect("US RLE row should include known stressors")
            .iter()
            .any(|stressor| stressor.as_str() == Some("ultrasound_image_storage")),
        "US RLE report row should retain Ultrasound Image Storage stressor"
    );
    let vl_photo_row = coverage_row(&report, "vl/photo/rgb_planar0_rle_lossless");
    assert_eq!(
        vl_photo_row.get("status").and_then(Value::as_str),
        Some("generated")
    );
    assert_eq!(
        vl_photo_row.get("codec_backend_id").and_then(Value::as_str),
        Some("native_project_rle_encoder")
    );
    assert!(
        vl_photo_row
            .get("known_stressors")
            .and_then(Value::as_array)
            .expect("VL Photographic RLE row should include known stressors")
            .iter()
            .any(|stressor| stressor.as_str() == Some("vl_photographic_image_storage")),
        "VL Photographic RLE report row should retain VL Photographic Image Storage stressor"
    );
    let vl_photo_planar1_row = coverage_row(&report, "vl/photo/rgb_planar1_rle_lossless");
    assert_eq!(
        vl_photo_planar1_row.get("status").and_then(Value::as_str),
        Some("generated")
    );
    assert_eq!(
        vl_photo_planar1_row
            .get("codec_backend_id")
            .and_then(Value::as_str),
        Some("native_project_rle_encoder")
    );
    assert!(
        vl_photo_planar1_row
            .get("known_stressors")
            .and_then(Value::as_array)
            .expect("VL Photographic planar-1 RLE row should include known stressors")
            .iter()
            .any(|stressor| stressor.as_str() == Some("vl_rgb_pixels")),
        "VL Photographic planar-1 RLE report row should retain VL RGB pixel stressor"
    );
    let vl_photo_palette_row = coverage_row(&report, "vl/photo/palette_color_rle_lossless");
    assert_eq!(
        vl_photo_palette_row.get("status").and_then(Value::as_str),
        Some("generated")
    );
    assert_eq!(
        vl_photo_palette_row
            .get("codec_backend_id")
            .and_then(Value::as_str),
        Some("native_project_rle_encoder")
    );
    assert_eq!(
        vl_photo_palette_row
            .get("photometric")
            .and_then(Value::as_str),
        Some("PALETTE COLOR")
    );
    assert!(
        vl_photo_palette_row
            .get("known_stressors")
            .and_then(Value::as_array)
            .expect("VL Photographic palette RLE row should include known stressors")
            .iter()
            .any(|stressor| stressor.as_str() == Some("vl_palette_color_pixels")),
        "VL Photographic palette RLE report row should retain VL palette stressor"
    );

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
#[cfg(feature = "deflate")]
fn report_command_counts_generated_deflated_image_frame_seg_row() {
    let out_dir = unique_temp_dir("report-deflated-image-frame-json");
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
    let row = coverage_row(
        &report,
        "derived/seg/binary_multiframe_deflated_image_frame",
    );
    assert_eq!(row.get("status").and_then(Value::as_str), Some("generated"));
    assert_eq!(
        row.get("transfer_syntax").and_then(Value::as_str),
        Some("1.2.840.10008.1.2.8.1")
    );
    assert_eq!(
        row.get("validation_status").and_then(Value::as_str),
        Some("passed")
    );

    fs::remove_dir_all(out_dir).expect("temporary output root should be removable");
}

#[test]
#[cfg(feature = "legacy_jpeg_dcmtk")]
fn report_command_counts_generated_legacy_jpeg_lossless_rows() {
    let out_dir = unique_temp_dir("report-legacy-jpeg-lossless-json");
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
    let process_14_row = coverage_row(&report, "classic/sc/mono2_u16_jpeg_lossless_process_14");
    assert_eq!(
        process_14_row.get("status").and_then(Value::as_str),
        Some("generated")
    );
    assert_eq!(
        process_14_row
            .get("transfer_syntax")
            .and_then(Value::as_str),
        Some("1.2.840.10008.1.2.4.57")
    );
    assert_eq!(
        process_14_row
            .get("validation_status")
            .and_then(Value::as_str),
        Some("passed")
    );

    let row = coverage_row(&report, "classic/sc/mono2_u16_jpeg_lossless_sv1");
    assert_eq!(row.get("status").and_then(Value::as_str), Some("generated"));
    assert_eq!(
        row.get("transfer_syntax").and_then(Value::as_str),
        Some("1.2.840.10008.1.2.4.70")
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
fn report_summarizes_compressed_codec_coverage() {
    let out_dir = unique_temp_dir("report-compressed-codec-summary");
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
                "case_id": "classic/sc/mono2_u8_rle_lossless",
                "dicom": {
                    "iod_name": "Secondary Capture Image",
                    "sop_class_uid": "1.2.840.10008.5.1.4.1.1.7",
                    "transfer_syntax_uid": "1.2.840.10008.1.2.5"
                },
                "image": {
                    "photometric_interpretation": "MONOCHROME2",
                    "bits_stored": 8,
                    "frames": 1,
                    "rows": 2,
                    "columns": 2
                },
                "pixel_data": {
                    "codec": {
                        "backend_id": "native_rle_lossless",
                        "backend_kind": "native",
                        "feature_gate": null
                    }
                },
                "validation": {
                    "status": "passed"
                },
                "determinism": "byte_stable",
                "known_stressors": ["compressed_pixel_data"]
            }
        ],
        "skipped_cases": [
            {
                "case_id": "classic/sc/rgb_planar0_jpeg_baseline_8bit",
                "status": "unavailable",
                "reason_code": "feature_gated_case_unavailable",
                "message": "This implemented registry case requires Cargo feature(s) jpeg.",
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
        .expect("report should summarize compressed coverage");
    let generated = coverage_row(&report, "classic/sc/mono2_u8_rle_lossless");
    assert_eq!(
        generated.get("codec_family").and_then(Value::as_str),
        Some("RLE Lossless")
    );
    assert_eq!(
        generated.get("codec_backend_id").and_then(Value::as_str),
        Some("native_rle_lossless")
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/codec_families/RLE Lossless")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/codec_families/JPEG Baseline")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/codec_backends/native_rle_lossless")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/determinism/byte_stable")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        report
            .pointer("/grouped_coverage/unavailable_reasons/feature_gated_case_unavailable")
            .and_then(Value::as_u64),
        Some(1)
    );
    let unavailable = coverage_row(&report, "classic/sc/rgb_planar0_jpeg_baseline_8bit");
    assert_eq!(
        unavailable
            .get("codec_feature_gate")
            .and_then(Value::as_str),
        Some("jpeg")
    );

    let markdown = dicom_test_suite::render_coverage_report_markdown(&report);
    assert!(markdown.contains("### Codec Families"));
    assert!(markdown.contains("| RLE Lossless | 1 |"));
    assert!(markdown.contains("### Codec Backends"));
    assert!(markdown.contains("| native_rle_lossless | 1 |"));
    assert!(markdown.contains("### Unavailable Reasons"));

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
