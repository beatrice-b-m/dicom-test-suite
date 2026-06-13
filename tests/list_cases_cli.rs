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
