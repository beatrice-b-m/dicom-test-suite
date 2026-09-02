use std::path::Path;
use std::process::Command;

#[test]
fn installed_style_consumer_discovers_and_reproduces_every_qualified_template() {
    let backend = std::env::var_os("DTS_HIGHDICOM_PYTHON")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            Path::new("generation-backends/highdicom-pydicom/.venv/bin/python").into()
        });
    assert!(
        backend.is_file(),
        "prepared locked generation backend is required"
    );
    let backend = if backend.is_absolute() {
        backend
    } else {
        std::env::current_dir().unwrap().join(backend)
    };
    let binary = std::env::var_os("DTS_QUALIFIED_CATALOG_BINARY")
        .unwrap_or_else(|| env!("CARGO_BIN_EXE_synth-dicom-gen").into());
    let output = Command::new("python3")
        .arg("tests/qualified_catalog_consumer.py")
        .arg(binary)
        .env("DTS_HIGHDICOM_PYTHON", backend)
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
        "installed qualified catalog consumer passed\n"
    );
}
