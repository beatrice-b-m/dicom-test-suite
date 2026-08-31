use std::process::Command;

#[test]
fn python_consumer_integrates_through_schemas_and_exit_classes() {
    let output = Command::new("python3")
        .arg("tests/black_box_cli_consumer.py")
        .arg(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .arg(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("python3 must run the black-box consumer");
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "black-box CLI API 1.0.0 consumer passed\n"
    );
}
