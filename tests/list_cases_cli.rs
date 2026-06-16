use std::process::Command;

#[test]
fn list_cases_command_shows_smoke_case_status_and_evidence() {
    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args(["list-cases", "--profile", "smoke"])
        .output()
        .expect("list-cases command must run");

    assert!(
        output.status.success(),
        "list-cases should exit successfully: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).expect("list-cases stdout must be utf-8");
    assert!(
        stdout.contains(
            "case_id\tstatus\tprofiles\tsop_class_uid\ttransfer_syntax_uid\tstandards_evidence"
        ),
        "list-cases must print structured columns"
    );
    assert!(
        stdout.contains(
            "classic/sc/mono2_u8_explicit_le\timplemented\tsmoke\t1.2.840.10008.5.1.4.1.1.7\t1.2.840.10008.1.2.1\t2/2 covered"
        ),
        "list-cases must include implemented smoke cases with standards evidence"
    );
}

#[test]
fn list_cases_command_shows_core_case_status_and_evidence() {
    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args(["list-cases", "--profile", "core"])
        .output()
        .expect("list-cases command must run");

    assert!(
        output.status.success(),
        "list-cases should exit successfully: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).expect("list-cases stdout must be utf-8");
    assert!(
        stdout.contains(
            "classic/ct/mono2_i16_rescale_12bit_explicit_le\timplemented\tcore\t1.2.840.10008.5.1.4.1.1.2\t1.2.840.10008.1.2.1\t11/11 covered"
        ),
        "list-cases must include implemented CT core cases with standards evidence"
    );
    assert!(
        stdout.contains(
            "classic/mg/for_presentation_mono1_u16_12bit_explicit_le\timplemented\tcore\t1.2.840.10008.5.1.4.1.1.1.2\t1.2.840.10008.1.2.1\t14/14 covered"
        ),
        "list-cases must include implemented MG core cases with standards evidence"
    );
    assert!(
        stdout.contains(
            "classic/mg/for_processing_mono2_u16_12bit_implicit_le\timplemented\tcore\t1.2.840.10008.5.1.4.1.1.1.2.1\t1.2.840.10008.1.2\t15/15 covered"
        ),
        "list-cases must include implemented MG For Processing core cases with standards evidence"
    );
    assert!(
        stdout.contains(
            "classic/cr/overlay_modality_voi_explicit_le\timplemented\tcore\t1.2.840.10008.5.1.4.1.1.1\t1.2.840.10008.1.2.1\t14/14 covered"
        ),
        "list-cases must include implemented CR overlay/LUT core cases with standards evidence"
    );
    assert!(
        stdout.contains(
            "classic/mr/multislice_oblique_explicit_le\timplemented\tcore\t1.2.840.10008.5.1.4.1.1.4\t1.2.840.10008.1.2.1\t11/11 covered"
        ),
        "list-cases must include implemented MR multi-slice core cases with standards evidence"
    );
    assert!(
        stdout.contains(
            "classic/dx/display_shutter_mono2_u16_explicit_le\timplemented\tcore\t1.2.840.10008.5.1.4.1.1.1.1\t1.2.840.10008.1.2.1\t16/16 covered"
        ),
        "list-cases must include implemented DX display shutter core cases with standards evidence"
    );
    assert!(
        stdout.contains(
            "classic/us/mono2_u8_explicit_le\timplemented\tcore\t1.2.840.10008.5.1.4.1.1.6.1\t1.2.840.10008.1.2.1\t9/9 covered"
        ),
        "list-cases must include implemented US core cases with standards evidence"
    );
    assert!(
        stdout.contains(
            "classic/sc/mono2_u16_explicit_le\timplemented\tcore\t1.2.840.10008.5.1.4.1.1.7\t1.2.840.10008.1.2.1\t6/6 covered"
        ),
        "list-cases must include implemented core native pixel cases"
    );
    assert!(
        stdout.contains(
            "classic/sc/mono2_i16_explicit_le\timplemented\tcore\t1.2.840.10008.5.1.4.1.1.7\t1.2.840.10008.1.2.1\t7/7 covered"
        ),
        "list-cases must include implemented signed core native pixel cases"
    );
    assert!(
        stdout.contains(
            "classic/sc/rgb_planar1_explicit_le\timplemented\tcore\t1.2.840.10008.5.1.4.1.1.7\t1.2.840.10008.1.2.1\t5/5 covered"
        ),
        "list-cases must include implemented RGB planar1 core cases"
    );
    assert!(
        stdout.contains(
            "classic/sc/palette_color_u8_explicit_le\timplemented\tcore\t1.2.840.10008.5.1.4.1.1.7\t1.2.840.10008.1.2.1\t10/10 covered"
        ),
        "list-cases must include implemented PALETTE COLOR core cases"
    );
    assert!(
        stdout.contains(
            "classic/sc/ybr_full_planar0_explicit_le\timplemented\tcore\t1.2.840.10008.5.1.4.1.1.7\t1.2.840.10008.1.2.1\t6/6 covered"
        ),
        "list-cases must include implemented YBR_FULL core cases"
    );
    assert!(
        stdout.contains(
            "classic/sc/ybr_full_422_explicit_le\timplemented\tcore\t1.2.840.10008.5.1.4.1.1.7\t1.2.840.10008.1.2.1\t7/7 covered"
        ),
        "list-cases must include implemented YBR_FULL_422 core cases"
    );
    assert!(
        stdout.contains(
            "classic/sc/mono2_u16_odd_3x3_explicit_le\timplemented\tcore\t1.2.840.10008.5.1.4.1.1.7\t1.2.840.10008.1.2.1\t5/5 covered"
        ),
        "list-cases must include implemented odd-dimension core cases"
    );
    assert!(
        stdout.contains(
            "classic/sc/mono2_u16_rect_2x3_explicit_le\timplemented\tcore\t1.2.840.10008.5.1.4.1.1.7\t1.2.840.10008.1.2.1\t5/5 covered"
        ),
        "list-cases must include implemented rectangular core cases"
    );
    assert!(
        stdout.contains(
            "classic/sc/mono2_u16_tiny_1x1_explicit_le\timplemented\tcore\t1.2.840.10008.5.1.4.1.1.7\t1.2.840.10008.1.2.1\t5/5 covered"
        ),
        "list-cases must include implemented tiny-image core cases"
    );
    assert!(
        stdout.contains(
            "classic/sc/mono2_u16_padding_explicit_le\timplemented\tcore\t1.2.840.10008.5.1.4.1.1.7\t1.2.840.10008.1.2.1\t7/7 covered"
        ),
        "list-cases must include implemented pixel-padding core cases"
    );
}

#[test]
fn list_cases_command_shows_legacy_big_endian_case_status_and_evidence() {
    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args(["list-cases", "--profile", "legacy"])
        .output()
        .expect("list-cases command must run");

    assert!(
        output.status.success(),
        "list-cases should exit successfully: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).expect("list-cases stdout must be utf-8");
    assert!(
        stdout.contains(
            "classic/sc/mono2_u8_explicit_be\timplemented\tlegacy\t1.2.840.10008.5.1.4.1.1.7\t1.2.840.10008.1.2.2\t2/2 covered"
        ),
        "list-cases must include implemented legacy Explicit VR Big Endian cases"
    );
}

#[test]
fn list_cases_command_shows_extended_case_status_and_evidence() {
    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args(["list-cases", "--profile", "extended"])
        .output()
        .expect("list-cases command must run");

    assert!(
        output.status.success(),
        "list-cases should exit successfully: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).expect("list-cases stdout must be utf-8");
    assert!(
        stdout.contains(
            "enhanced/ct/multiframe_shared_perframe_explicit_le\timplemented\textended\t1.2.840.10008.5.1.4.1.1.2.1\t1.2.840.10008.1.2.1\t16/16 covered"
        ),
        "list-cases must include implemented Enhanced CT extended cases with standards evidence"
    );
    assert!(
        stdout.contains(
            "enhanced/ct/concatenation_two_part_explicit_le\timplemented\textended\t1.2.840.10008.5.1.4.1.1.2.1\t1.2.840.10008.1.2.1\t14/14 covered"
        ),
        "list-cases must include implemented Enhanced CT concatenation cases with standards evidence"
    );
    assert!(
        stdout.contains(
            "enhanced/mr/multiframe_echo_perframe_explicit_le\timplemented\textended\t1.2.840.10008.5.1.4.1.1.4.1\t1.2.840.10008.1.2.1\t20/20 covered"
        ),
        "list-cases must include implemented Enhanced MR extended cases with standards evidence"
    );
    assert!(
        stdout.contains(
            "enhanced/mr/multiframe_temporal_position_explicit_le\timplemented\textended\t1.2.840.10008.5.1.4.1.1.4.1\t1.2.840.10008.1.2.1\t23/23 covered"
        ),
        "list-cases must include implemented Enhanced MR temporal extended cases with standards evidence"
    );
    assert!(
        stdout.contains(
            "enhanced/mr/multiframe_phase_velocity_encoding_explicit_le\timplemented\textended\t1.2.840.10008.5.1.4.1.1.4.1\t1.2.840.10008.1.2.1\t27/27 covered"
        ),
        "list-cases must include implemented Enhanced MR phase extended cases with standards evidence"
    );
    assert!(
        stdout.contains(
            "derived/seg/binary_multiframe_explicit_le\timplemented\textended\t1.2.840.10008.5.1.4.1.1.66.4\t1.2.840.10008.1.2.1\t8/8 covered"
        ),
        "list-cases must include the implemented SEG extended case with standards evidence"
    );
    assert!(
        stdout.contains(
            "derived/seg/binary_multiframe_deflated_image_frame\timplemented\textended\t1.2.840.10008.5.1.4.1.1.66.4\t1.2.840.10008.1.2.8.1\t10/10 covered"
        ),
        "list-cases must include the implemented feature-gated Deflated Image Frame SEG case with standards evidence"
    );
    assert!(
        stdout.contains(
            "derived/seg/fractional_probability_multiframe_explicit_le\timplemented\textended\t1.2.840.10008.5.1.4.1.1.66.4\t1.2.840.10008.1.2.1\t8/8 covered"
        ),
        "list-cases must include the implemented fractional SEG extended case with standards evidence"
    );
    assert!(
        stdout.contains(
            "derived/seg/labelmap_multiframe_explicit_le\timplemented\textended\t1.2.840.10008.5.1.4.1.1.66.7\t1.2.840.10008.1.2.1\t7/7 covered"
        ),
        "list-cases must include the implemented LABELMAP SEG extended case with standards evidence"
    );
    assert!(
        stdout.contains(
            "derived/presentation-state/grayscale_softcopy_ct_window_explicit_le\timplemented\textended\t1.2.840.10008.5.1.4.1.1.11.1\t1.2.840.10008.1.2.1\t8/8 covered"
        ),
        "list-cases must include the implemented GSPS extended case with standards evidence"
    );
    assert!(
        stdout.contains(
            "derived/rwvm/linear_ct_mapping_explicit_le\timplemented\textended\t1.2.840.10008.5.1.4.1.1.67\t1.2.840.10008.1.2.1\t5/5 covered"
        ),
        "list-cases must include the implemented RWVM extended case with standards evidence"
    );
    assert!(
        stdout.contains(
            "derived/sr/basic_text_observation_explicit_le\timplemented\textended\t1.2.840.10008.5.1.4.1.1.88.11\t1.2.840.10008.1.2.1\t5/5 covered"
        ),
        "list-cases must include the implemented Basic Text SR extended case with standards evidence"
    );
    assert!(
        stdout.contains(
            "derived/sr/comprehensive_measurement_explicit_le\timplemented\textended\t1.2.840.10008.5.1.4.1.1.88.33\t1.2.840.10008.1.2.1\t5/5 covered"
        ),
        "list-cases must include the implemented Comprehensive SR extended case with standards evidence"
    );
    assert!(
        stdout.contains(
            "derived/sr/key_object_selection_explicit_le\timplemented\textended\t1.2.840.10008.5.1.4.1.1.88.59\t1.2.840.10008.1.2.1\t5/5 covered"
        ),
        "list-cases must include the implemented KOS extended case with standards evidence"
    );
    assert!(
        stdout.contains(
            "non-image/rt/structure_set_single_roi_explicit_le\timplemented\textended\t1.2.840.10008.5.1.4.1.1.481.3\t1.2.840.10008.1.2.1\t4/4 covered"
        ),
        "list-cases must include the implemented RT Structure Set extended case with standards evidence"
    );
    assert!(
        stdout.contains(
            "non-image/rt/dose_grid_u16_explicit_le\timplemented\textended\t1.2.840.10008.5.1.4.1.1.481.2\t1.2.840.10008.1.2.1\t5/5 covered"
        ),
        "list-cases must include the implemented RT Dose extended case with standards evidence"
    );
    assert!(
            stdout.contains(
            "non-image/encapsulated-document/pdf_minimal_explicit_le\timplemented\textended\t1.2.840.10008.5.1.4.1.1.104.1\t1.2.840.10008.1.2.1\t7/7 covered"
        ),
        "list-cases must include the implemented Encapsulated PDF Phase 5 case with standards evidence"
    );
    assert!(
        stdout.contains(
            "classic/sc/mono2_u8_deflated_explicit_le\timplemented\textended\t1.2.840.10008.5.1.4.1.1.7\t1.2.840.10008.1.2.1.99\t2/2 covered"
        ),
        "list-cases must include the implemented feature-gated deflated transfer syntax case"
    );
}

#[test]
fn list_cases_command_filters_by_status_and_profile() {
    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args(["list-cases", "--profile", "extended", "--status", "planned"])
        .output()
        .expect("list-cases command must run");

    assert!(
        output.status.success(),
        "list-cases should exit successfully: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).expect("list-cases stdout must be utf-8");
    assert!(
        !stdout.contains("derived/seg/binary_multiframe_explicit_le"),
        "planned status filter must not include implemented SEG"
    );
    assert!(
        !stdout.contains("derived/seg/binary_multiframe_deflated_image_frame"),
        "planned status filter must not include implemented Deflated Image Frame SEG"
    );
    assert!(
        !stdout.contains("derived/seg/fractional_probability_multiframe_explicit_le"),
        "planned status filter must not include implemented fractional SEG"
    );
    assert!(
        !stdout.contains("derived/seg/labelmap_multiframe_explicit_le"),
        "planned status filter must not include implemented LABELMAP SEG"
    );
    assert!(
        !stdout.contains("derived/presentation-state/grayscale_softcopy_ct_window_explicit_le"),
        "planned status filter must not include implemented GSPS"
    );
    assert!(
        !stdout.contains("derived/rwvm/linear_ct_mapping_explicit_le"),
        "planned status filter must not include implemented RWVM"
    );
    assert!(
        !stdout.contains("derived/sr/basic_text_observation_explicit_le"),
        "planned status filter must not include implemented Basic Text SR"
    );
    assert!(
        !stdout.contains("derived/sr/comprehensive_measurement_explicit_le"),
        "planned status filter must not include implemented Comprehensive SR"
    );
    assert!(
        !stdout.contains("derived/sr/key_object_selection_explicit_le"),
        "planned status filter must not include implemented KOS"
    );
    assert!(
        !stdout.contains("non-image/rt/structure_set_single_roi_explicit_le"),
        "planned status filter must not include implemented RT Structure Set"
    );
    assert!(
        !stdout.contains("non-image/rt/dose_grid_u16_explicit_le"),
        "planned status filter must not include implemented RT Dose"
    );
    assert!(
        !stdout.contains("non-image/encapsulated-document/pdf_minimal_explicit_le"),
        "planned status filter must not include implemented Encapsulated PDF"
    );
    assert!(
        !stdout.contains("enhanced/ct/multiframe_shared_perframe_explicit_le"),
        "status filter must exclude implemented extended cases"
    );
    assert!(
        !stdout.contains("vl/photo/rgb_planar0_explicit_le"),
        "profile filter must exclude planned core cases"
    );
    assert!(
        !stdout.contains("classic/sc/mono2_u8_deflated_explicit_le"),
        "planned status filter must exclude implemented feature-gated deflated cases"
    );
}

#[test]
fn list_cases_command_excludes_promoted_compressed_cases_from_skipped_rows() {
    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args(["list-cases", "--profile", "extended", "--status", "skipped"])
        .output()
        .expect("list-cases command must run");

    assert!(
        output.status.success(),
        "list-cases should exit successfully: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).expect("list-cases stdout must be utf-8");
    assert!(
        !stdout.contains("classic/sc/mono2_u16_htj2k_lossless"),
        "promoted HTJ2K row must not be listed as skipped"
    );
    assert!(
        !stdout.contains("classic/sc/mono2_u16_jpeg_lossless_process_14"),
        "promoted legacy JPEG Process 14 row must not be listed as skipped"
    );
    assert!(
        !stdout.contains("classic/sc/mono2_u16_jpeg_lossless_sv1"),
        "promoted legacy JPEG SV1 row must not be listed as skipped"
    );
    assert!(
        !stdout.contains("derived/seg/binary_multiframe_deflated_image_frame"),
        "promoted Deflated Image Frame SEG row must not be listed as skipped"
    );
}

#[test]
fn list_cases_command_shows_rle_lossless_as_implemented() {
    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args([
            "list-cases",
            "--profile",
            "extended",
            "--status",
            "implemented",
        ])
        .output()
        .expect("list-cases command must run");

    assert!(
        output.status.success(),
        "list-cases should exit successfully: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).expect("list-cases stdout must be utf-8");
    assert!(
        stdout.contains(
            "classic/sc/mono2_u8_rle_lossless\timplemented\textended\t1.2.840.10008.5.1.4.1.1.7\t1.2.840.10008.1.2.5\t3/3 covered"
        ),
        "RLE Lossless row must be listed as implemented"
    );
    assert!(
        stdout.contains(
            "classic/sc/mono2_u16_rle_lossless\timplemented\textended\t1.2.840.10008.5.1.4.1.1.7\t1.2.840.10008.1.2.5\t4/4 covered"
        ),
        "16-bit RLE Lossless row must be listed as implemented"
    );
    assert!(
        stdout.contains(
            "classic/sc/mono2_u16_odd_3x3_rle_lossless\timplemented\textended\t1.2.840.10008.5.1.4.1.1.7\t1.2.840.10008.1.2.5\t6/6 covered"
        ),
        "odd 3x3 RLE Lossless row must be listed as implemented"
    );
    assert!(
        stdout.contains(
            "classic/sc/mono2_u16_rect_2x3_rle_lossless\timplemented\textended\t1.2.840.10008.5.1.4.1.1.7\t1.2.840.10008.1.2.5\t6/6 covered"
        ),
        "rectangular 2x3 RLE Lossless row must be listed as implemented"
    );
    assert!(
        stdout.contains(
            "classic/sc/mono2_u16_tiny_1x1_rle_lossless\timplemented\textended\t1.2.840.10008.5.1.4.1.1.7\t1.2.840.10008.1.2.5\t6/6 covered"
        ),
        "tiny 1x1 RLE Lossless row must be listed as implemented"
    );
    assert!(
        stdout.contains(
            "classic/sc/mono2_u16_padding_rle_lossless\timplemented\textended\t1.2.840.10008.5.1.4.1.1.7\t1.2.840.10008.1.2.5\t8/8 covered"
        ),
        "Pixel Padding RLE Lossless row must be listed as implemented"
    );
    assert!(
        stdout.contains(
            "classic/sc/mono2_i16_rle_lossless\timplemented\textended\t1.2.840.10008.5.1.4.1.1.7\t1.2.840.10008.1.2.5\t5/5 covered"
        ),
        "signed 16-bit RLE Lossless row must be listed as implemented"
    );
    assert!(
        stdout.contains(
            "classic/sc/mono1_u8_rle_lossless\timplemented\textended\t1.2.840.10008.5.1.4.1.1.7\t1.2.840.10008.1.2.5\t4/4 covered"
        ),
        "MONOCHROME1 RLE Lossless row must be listed as implemented"
    );
    assert!(
        stdout.contains(
            "classic/sc/rgb_planar0_rle_lossless\timplemented\textended\t1.2.840.10008.5.1.4.1.1.7\t1.2.840.10008.1.2.5\t5/5 covered"
        ),
        "RGB RLE Lossless row must be listed as implemented"
    );
    assert!(
        stdout.contains(
            "classic/sc/rgb_planar0_multiframe_rle_lossless\timplemented\textended\t1.2.840.10008.5.1.4.1.1.7\t1.2.840.10008.1.2.5\t6/6 covered"
        ),
        "RGB multi-frame RLE Lossless row must be listed as implemented"
    );
    assert!(
        stdout.contains(
            "classic/sc/rgb_planar1_rle_lossless\timplemented\textended\t1.2.840.10008.5.1.4.1.1.7\t1.2.840.10008.1.2.5\t5/5 covered"
        ),
        "RGB planar-1 RLE Lossless row must be listed as implemented"
    );
    assert!(
        stdout.contains(
            "classic/sc/rgb_planar1_multiframe_rle_lossless\timplemented\textended\t1.2.840.10008.5.1.4.1.1.7\t1.2.840.10008.1.2.5\t6/6 covered"
        ),
        "RGB planar-1 multi-frame RLE Lossless row must be listed as implemented"
    );
    assert!(
        stdout.contains(
            "classic/sc/ybr_full_planar0_rle_lossless\timplemented\textended\t1.2.840.10008.5.1.4.1.1.7\t1.2.840.10008.1.2.5\t5/5 covered"
        ),
        "YBR_FULL RLE Lossless row must be listed as implemented"
    );
    assert!(
        stdout.contains(
            "classic/sc/ybr_full_planar0_multiframe_rle_lossless\timplemented\textended\t1.2.840.10008.5.1.4.1.1.7\t1.2.840.10008.1.2.5\t6/6 covered"
        ),
        "YBR_FULL multi-frame RLE Lossless row must be listed as implemented"
    );
    assert!(
        stdout.contains(
            "classic/sc/ybr_full_planar1_rle_lossless\timplemented\textended\t1.2.840.10008.5.1.4.1.1.7\t1.2.840.10008.1.2.5\t5/5 covered"
        ),
        "YBR_FULL planar-1 RLE Lossless row must be listed as implemented"
    );
    assert!(
        stdout.contains(
            "classic/sc/palette_color_u8_rle_lossless\timplemented\textended\t1.2.840.10008.5.1.4.1.1.7\t1.2.840.10008.1.2.5\t11/11 covered"
        ),
        "PALETTE COLOR RLE Lossless row must be listed as implemented"
    );
    assert!(
        stdout.contains(
            "classic/sc/mono2_u8_multiframe_rle_lossless\timplemented\textended\t1.2.840.10008.5.1.4.1.1.7\t1.2.840.10008.1.2.5\t4/4 covered"
        ),
        "multi-frame RLE Lossless row must be listed as implemented"
    );
    assert!(
        stdout.contains(
            "classic/sc/mono2_u16_multiframe_rle_lossless\timplemented\textended\t1.2.840.10008.5.1.4.1.1.7\t1.2.840.10008.1.2.5\t5/5 covered"
        ),
        "16-bit multi-frame RLE Lossless row must be listed as implemented"
    );
    assert!(
        stdout.contains(
            "classic/sc/mono2_u8_odd_fragment_rle_lossless\timplemented\textended\t1.2.840.10008.5.1.4.1.1.7\t1.2.840.10008.1.2.5\t4/4 covered"
        ),
        "odd-fragment RLE Lossless row must be listed as implemented"
    );
    assert!(
        stdout.contains(
            "classic/ct/mono2_i16_rescale_12bit_rle_lossless\timplemented\textended\t1.2.840.10008.5.1.4.1.1.2\t1.2.840.10008.1.2.5\t13/13 covered"
        ),
        "CT RLE Lossless row must be listed as implemented"
    );
    assert!(
        stdout.contains(
            "classic/mr/mono2_u16_rle_lossless\timplemented\textended\t1.2.840.10008.5.1.4.1.1.4\t1.2.840.10008.1.2.5\t13/13 covered"
        ),
        "MR RLE Lossless row must be listed as implemented"
    );
    assert!(
        stdout.contains(
            "classic/cr/overlay_modality_voi_rle_lossless\timplemented\textended\t1.2.840.10008.5.1.4.1.1.1\t1.2.840.10008.1.2.5\t16/16 covered"
        ),
        "CR RLE Lossless row must be listed as implemented"
    );
    assert!(
        stdout.contains(
            "classic/dx/display_shutter_mono2_u16_rle_lossless\timplemented\textended\t1.2.840.10008.5.1.4.1.1.1.1\t1.2.840.10008.1.2.5\t11/11 covered"
        ),
        "DX RLE Lossless row must be listed as implemented"
    );
    assert!(
        stdout.contains(
            "classic/mg/for_presentation_mono1_u16_12bit_rle_lossless\timplemented\textended\t1.2.840.10008.5.1.4.1.1.1.2\t1.2.840.10008.1.2.5\t17/17 covered"
        ),
        "MG RLE Lossless row must be listed as implemented"
    );
    assert!(
        stdout.contains(
            "classic/mg/for_processing_mono2_u16_12bit_rle_lossless\timplemented\textended\t1.2.840.10008.5.1.4.1.1.1.2.1\t1.2.840.10008.1.2.5\t15/15 covered"
        ),
        "MG For Processing RLE Lossless row must be listed as implemented"
    );
    assert!(
        stdout.contains(
            "classic/us/mono2_u8_rle_lossless\timplemented\textended\t1.2.840.10008.5.1.4.1.1.6.1\t1.2.840.10008.1.2.5\t11/11 covered"
        ),
        "US RLE Lossless row must be listed as implemented"
    );
    assert!(
        stdout.contains(
            "vl/photo/rgb_planar0_rle_lossless\timplemented\textended\t1.2.840.10008.5.1.4.1.1.77.1.4\t1.2.840.10008.1.2.5\t7/7 covered"
        ),
        "VL Photographic RGB RLE Lossless row must be listed as implemented"
    );
    assert!(
        stdout.contains(
            "vl/photo/rgb_planar1_rle_lossless\timplemented\textended\t1.2.840.10008.5.1.4.1.1.77.1.4\t1.2.840.10008.1.2.5\t7/7 covered"
        ),
        "VL Photographic RGB planar-1 RLE Lossless row must be listed as implemented"
    );
    assert!(
        stdout.contains(
            "classic/sc/rgb_planar0_jpeg_baseline_8bit\timplemented\textended\t1.2.840.10008.5.1.4.1.1.7\t1.2.840.10008.1.2.4.50\t2/2 covered"
        ),
        "JPEG Baseline row must be listed as implemented and feature-gated"
    );
    assert!(
        stdout.contains(
            "classic/sc/mono2_u8_jpeg_ls_lossless\timplemented\textended\t1.2.840.10008.5.1.4.1.1.7\t1.2.840.10008.1.2.4.80\t2/2 covered"
        ),
        "JPEG-LS Lossless row must be listed as implemented and feature-gated"
    );
    assert!(
        stdout.contains(
            "classic/sc/rgb_planar0_jpegxl_lossless\timplemented\textended\t1.2.840.10008.5.1.4.1.1.7\t1.2.840.10008.1.2.4.110\t2/2 covered"
        ),
        "JPEG XL Lossless row must be listed as implemented and feature-gated"
    );
    assert!(
        stdout.contains(
            "classic/sc/mono2_u16_jpeg2000_lossless\timplemented\textended\t1.2.840.10008.5.1.4.1.1.7\t1.2.840.10008.1.2.4.90\t2/2 covered"
        ),
        "JPEG 2000 Lossless row must be listed as implemented and feature-gated"
    );
    assert!(
        stdout.contains(
            "classic/sc/mono2_u16_htj2k_lossless\timplemented\textended\t1.2.840.10008.5.1.4.1.1.7\t1.2.840.10008.1.2.4.201\t2/2 covered"
        ),
        "HTJ2K Lossless row must be listed as implemented and feature-gated"
    );
    assert!(
        stdout.contains(
            "classic/sc/mono2_u16_jpeg_lossless_process_14\timplemented\textended\t1.2.840.10008.5.1.4.1.1.7\t1.2.840.10008.1.2.4.57\t2/2 covered"
        ),
        "JPEG Lossless Process 14 row must be listed as implemented and feature-gated"
    );
    assert!(
        stdout.contains(
            "classic/sc/mono2_u16_jpeg_lossless_sv1\timplemented\textended\t1.2.840.10008.5.1.4.1.1.7\t1.2.840.10008.1.2.4.70\t2/2 covered"
        ),
        "JPEG Lossless SV1 row must be listed as implemented and feature-gated"
    );
    assert!(
        stdout.contains(
            "derived/seg/binary_multiframe_deflated_image_frame\timplemented\textended\t1.2.840.10008.5.1.4.1.1.66.4\t1.2.840.10008.1.2.8.1\t10/10 covered"
        ),
        "Deflated Image Frame SEG row must be listed as implemented and feature-gated"
    );
}

#[test]
fn list_cases_command_rejects_unknown_status() {
    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args(["list-cases", "--status", "unknown"])
        .output()
        .expect("list-cases command must run");

    assert!(
        !output.status.success(),
        "list-cases should reject unsupported status filters"
    );

    let stderr = String::from_utf8(output.stderr).expect("list-cases stderr must be utf-8");
    assert!(
        stderr.contains("unsupported case status unknown"),
        "error should explain the unsupported status"
    );
}
