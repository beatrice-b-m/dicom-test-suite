use std::process::Command;

#[test]
fn installed_style_consumer_uses_external_pixels_and_typed_attributes() {
    let binary = std::env::var_os("DTS_CALLER_CONTENT_BINARY")
        .unwrap_or_else(|| env!("CARGO_BIN_EXE_dicom-test-suite").into());
    let output = Command::new("python3")
        .arg("tests/caller_content_consumer.py")
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
        "installed caller content consumer passed\n"
    );
}
