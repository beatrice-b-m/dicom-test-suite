use std::process::Command;

#[test]
fn installed_style_consumer_accepts_supported_versions_and_guides_rejection() {
    let binary = std::env::var_os("SYNTH_DICOM_GEN_UPGRADE_BINARY")
        .unwrap_or_else(|| env!("CARGO_BIN_EXE_synth-dicom-gen").into());
    let output = Command::new("python3")
        .arg("tests/upgrade_consumer.py")
        .arg(binary)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "installed upgrade consumer passed\n"
    );
}
