use std::process::Command;

#[test]
fn installed_style_consumer_exercises_every_structural_content_kind() {
    let binary = std::env::var_os("DTS_STRUCTURAL_CATALOG_BINARY")
        .unwrap_or_else(|| env!("CARGO_BIN_EXE_dicom-test-suite").into());
    let output = Command::new("python3")
        .arg("tests/structural_catalog_consumer.py")
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
        "installed structural catalog consumer passed\n"
    );
}
