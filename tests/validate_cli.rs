use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[test]
fn validate_command_accepts_generated_smoke_root() {
    let out_dir = unique_temp_dir("validate-smoke");
    generate_smoke(&out_dir);

    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args([
            "validate",
            out_dir.to_str().expect("temp path should be valid UTF-8"),
        ])
        .output()
        .expect("validate command must run");

    assert!(
        output.status.success(),
        "validate should accept generated output: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("validate stdout must be UTF-8");
    assert!(stdout.contains("files_checked\t3"));
    assert!(stdout.contains("validation_failures\t0"));
    assert!(stdout.contains(&format!(
        "manifest\t{}",
        out_dir.join("manifest.json").display()
    )));

    fs::remove_dir_all(out_dir).expect("temporary output root should be removable");
}

#[test]
fn validate_command_rejects_missing_manifest() {
    let out_dir = unique_temp_dir("validate-missing-manifest");
    fs::create_dir_all(&out_dir).expect("temporary output root should be created");

    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args([
            "validate",
            out_dir.to_str().expect("temp path should be valid UTF-8"),
        ])
        .output()
        .expect("validate command must run");

    assert!(
        !output.status.success(),
        "validate should fail without a manifest"
    );
    let stderr = String::from_utf8(output.stderr).expect("validate stderr must be UTF-8");
    assert!(stderr.contains("failed to read manifest"));

    fs::remove_dir_all(out_dir).expect("temporary output root should be removable");
}

#[test]
fn validate_command_reports_corrupted_generated_file() {
    let out_dir = unique_temp_dir("validate-corrupt");
    generate_smoke(&out_dir);
    let dcm_path = out_dir.join("classic/sc/mono2_u8_explicit_le/instance.dcm");
    let mut bytes = fs::read(&dcm_path).expect("generated DICOM should be readable");
    bytes.push(0);
    fs::write(&dcm_path, bytes).expect("generated DICOM should be corruptible");

    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args([
            "validate",
            out_dir.to_str().expect("temp path should be valid UTF-8"),
        ])
        .output()
        .expect("validate command must run");

    assert!(
        !output.status.success(),
        "validate should fail when generated files drift from the manifest"
    );
    let stdout = String::from_utf8(output.stdout).expect("validate stdout must be UTF-8");
    assert!(stdout.contains("files_checked\t3"));
    assert!(stdout.contains("failure\tclassic/sc/mono2_u8_explicit_le/instance.dcm: sha256"));
    let stderr = String::from_utf8(output.stderr).expect("validate stderr must be UTF-8");
    assert!(stderr.contains("validation failed"));

    fs::remove_dir_all(out_dir).expect("temporary output root should be removable");
}

#[test]
fn validate_command_reports_nonzero_part10_preamble() {
    let out_dir = unique_temp_dir("validate-nonzero-preamble");
    generate_smoke(&out_dir);
    let dcm_path = out_dir.join("classic/sc/mono2_u8_explicit_le/instance.dcm");
    mutate_dicom(&dcm_path, |bytes| {
        bytes[0] = 1;
    });

    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args([
            "validate",
            out_dir.to_str().expect("temp path should be valid UTF-8"),
        ])
        .output()
        .expect("validate command must run");

    assert!(
        !output.status.success(),
        "validate should fail when the normal Part 10 preamble is non-zero"
    );
    let stdout = String::from_utf8(output.stdout).expect("validate stdout must be UTF-8");
    assert!(stdout.contains("part10_zero_preamble"));

    fs::remove_dir_all(out_dir).expect("temporary output root should be removable");
}

#[test]
fn validate_command_reports_missing_file_meta_information_version() {
    let out_dir = unique_temp_dir("validate-missing-file-meta-version");
    generate_smoke(&out_dir);
    let dcm_path = out_dir.join("classic/sc/mono2_u8_explicit_le/instance.dcm");
    mutate_dicom(&dcm_path, |bytes| {
        let offset = find_tag(bytes, 0x0002, 0x0001)
            .expect("generated DICOM should contain File Meta Information Version");
        bytes[offset] = 0x03;
    });

    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args([
            "validate",
            out_dir.to_str().expect("temp path should be valid UTF-8"),
        ])
        .output()
        .expect("validate command must run");

    assert!(
        !output.status.success(),
        "validate should fail when File Meta Information Version is missing"
    );
    let stdout = String::from_utf8(output.stdout).expect("validate stdout must be UTF-8");
    assert!(stdout.contains("file_meta_information_version"));

    fs::remove_dir_all(out_dir).expect("temporary output root should be removable");
}

#[test]
fn validate_command_reports_dataset_group_0002_after_file_meta() {
    let out_dir = unique_temp_dir("validate-dataset-group-0002");
    generate_smoke(&out_dir);
    let dcm_path = out_dir.join("classic/sc/mono2_u8_explicit_le/instance.dcm");
    mutate_dicom(&dcm_path, |bytes| {
        let offset = find_tag(bytes, 0x0008, 0x0016)
            .expect("generated DICOM should start dataset with SOP Class UID");
        bytes[offset] = 0x02;
        bytes[offset + 1] = 0x00;
    });

    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args([
            "validate",
            out_dir.to_str().expect("temp path should be valid UTF-8"),
        ])
        .output()
        .expect("validate command must run");

    assert!(
        !output.status.success(),
        "validate should fail when a group 0002 element appears in the dataset"
    );
    let stdout = String::from_utf8(output.stdout).expect("validate stdout must be UTF-8");
    assert!(stdout.contains("file_meta_allowed_element"));

    fs::remove_dir_all(out_dir).expect("temporary output root should be removable");
}

#[test]
fn validate_command_reports_inconsistent_high_bit() {
    let out_dir = unique_temp_dir("validate-high-bit");
    generate_smoke(&out_dir);
    let dcm_path = out_dir.join("classic/sc/mono2_u8_explicit_le/instance.dcm");
    mutate_dicom(&dcm_path, |bytes| {
        let offset =
            find_tag(bytes, 0x0028, 0x0102).expect("generated DICOM should contain High Bit");
        let value_offset = offset + 8;
        bytes[value_offset] = 8;
        bytes[value_offset + 1] = 0;
    });

    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args([
            "validate",
            out_dir.to_str().expect("temp path should be valid UTF-8"),
        ])
        .output()
        .expect("validate command must run");

    assert!(
        !output.status.success(),
        "validate should fail when High Bit does not equal Bits Stored - 1"
    );
    let stdout = String::from_utf8(output.stdout).expect("validate stdout must be UTF-8");
    assert!(stdout.contains("high_bit_consistency"));

    fs::remove_dir_all(out_dir).expect("temporary output root should be removable");
}

#[test]
fn validate_command_reports_missing_standard_type2_attribute() {
    let out_dir = unique_temp_dir("validate-missing-type2");
    generate_smoke(&out_dir);
    let dcm_path = out_dir.join("classic/sc/mono2_u8_explicit_le/instance.dcm");
    mutate_dicom(&dcm_path, |bytes| {
        let offset =
            find_tag(bytes, 0x0010, 0x0010).expect("generated DICOM should contain Patient's Name");
        bytes[offset] = 0x11;
        bytes[offset + 1] = 0x00;
    });

    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args([
            "validate",
            out_dir.to_str().expect("temp path should be valid UTF-8"),
        ])
        .output()
        .expect("validate command must run");

    assert!(
        !output.status.success(),
        "validate should fail when a standard Type 2 attribute is absent"
    );
    let stdout = String::from_utf8(output.stdout).expect("validate stdout must be UTF-8");
    assert!(stdout.contains("patient_name_type2"));

    fs::remove_dir_all(out_dir).expect("temporary output root should be removable");
}

#[test]
fn validate_command_reports_missing_sc_conversion_type() {
    let out_dir = unique_temp_dir("validate-missing-sc-conversion-type");
    generate_smoke(&out_dir);
    let dcm_path = out_dir.join("classic/sc/mono2_u8_explicit_le/instance.dcm");
    mutate_dicom(&dcm_path, |bytes| {
        let offset = find_tag(bytes, 0x0008, 0x0064)
            .expect("generated DICOM should contain Conversion Type");
        bytes[offset] = 0x09;
        bytes[offset + 1] = 0x00;
    });

    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args([
            "validate",
            out_dir.to_str().expect("temp path should be valid UTF-8"),
        ])
        .output()
        .expect("validate command must run");

    assert!(
        !output.status.success(),
        "validate should fail when SC Conversion Type is absent"
    );
    let stdout = String::from_utf8(output.stdout).expect("validate stdout must be UTF-8");
    assert!(stdout.contains("sc_conversion_type_type1"));

    fs::remove_dir_all(out_dir).expect("temporary output root should be removable");
}

#[test]
fn validate_command_reports_missing_ct_image_type() {
    let out_dir = unique_temp_dir("validate-missing-ct-image-type");
    generate_profile(&out_dir, "core");
    let dcm_path = out_dir.join("classic/ct/mono2_i16_rescale_12bit_explicit_le/instance.dcm");
    mutate_dicom(&dcm_path, |bytes| {
        let offset =
            find_tag(bytes, 0x0008, 0x0008).expect("generated CT should contain Image Type");
        bytes[offset] = 0x09;
        bytes[offset + 1] = 0x00;
    });

    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args([
            "validate",
            out_dir.to_str().expect("temp path should be valid UTF-8"),
        ])
        .output()
        .expect("validate command must run");

    assert!(
        !output.status.success(),
        "validate should fail when CT Image Type is absent"
    );
    let stdout = String::from_utf8(output.stdout).expect("validate stdout must be UTF-8");
    assert!(stdout.contains("ct_image_type_type1"));

    fs::remove_dir_all(out_dir).expect("temporary output root should be removable");
}

fn generate_smoke(out_dir: &Path) {
    generate_profile(out_dir, "smoke");
}

fn generate_profile(out_dir: &Path, profile: &str) {
    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
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
}

fn mutate_dicom(path: &Path, mutate: impl FnOnce(&mut Vec<u8>)) {
    let mut bytes = fs::read(path).expect("generated DICOM should be readable");
    mutate(&mut bytes);
    fs::write(path, bytes).expect("generated DICOM should be writable");
}

fn find_tag(bytes: &[u8], group: u16, element: u16) -> Option<usize> {
    let group = group.to_le_bytes();
    let element = element.to_le_bytes();
    bytes
        .windows(4)
        .position(|window| window == [group[0], group[1], element[0], element[1]])
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
