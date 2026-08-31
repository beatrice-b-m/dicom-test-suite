use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);

fn workspace() -> PathBuf {
    std::env::temp_dir().join(format!(
        "dicom-test-suite-sdk-consumer-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ))
}

fn dependency_root() -> PathBuf {
    std::env::var_os("DTS_SDK_PACKAGE_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")))
}

fn toml_path(path: &Path) -> String {
    path.display()
        .to_string()
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
}

#[test]
fn external_side_project_uses_only_the_supported_sdk_facade() {
    let root = workspace();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("Cargo.toml"),
        format!(
            r#"[package]
name = "dicom-test-suite-sdk-consumer"
version = "0.0.0"
edition = "2024"

[dependencies]
dicom-test-suite = {{ path = "{}", default-features = false }}
"#,
            toml_path(&dependency_root())
        ),
    )
    .unwrap();
    fs::write(
        root.join("src/main.rs"),
        r##"use dicom_test_suite::sdk::{
    CancellationToken, ComposeRequest, DicomTestSuite, ManifestKind, ReportKind, ReportRequest,
    SdkErrorKind, ValidateRequest,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = std::env::args_os().nth(1).expect("output argument");
    let product = DicomTestSuite::embedded()?;
    let version = product.version()?;
    let capabilities = product.capabilities()?;
    assert_eq!(version.cli_api_version, capabilities.cli_api_version);

    let spec = br#"{
      "composition_spec_schema_version":"0.1.0",
      "instances":[{"instance_id":"primary","template":{
        "id":"classic/secondary-capture/monochrome"
      }}]
    }"#;
    let outcome = product.compose(
        ComposeRequest::from_json_bytes(spec.as_slice(), ".", &output).with_seed(11),
    )?;
    assert!(outcome.published());
    assert_eq!(outcome.manifest().expect("manifest").kind(), ManifestKind::QualifiedComposition);
    let validation = product.validate(ValidateRequest::new(&output))?;
    assert!(validation.is_valid());
    let report = product.report(ReportRequest::new(&output))?;
    assert_eq!(report.kind(), ReportKind::QualifiedComposition);

    let cancelled_output = std::path::PathBuf::from(&output).with_extension("cancelled");
    let cancellation = CancellationToken::new();
    cancellation.cancel();
    let error = product
        .compose_cancellable(
            ComposeRequest::from_json_bytes(spec.as_slice(), ".", &cancelled_output),
            &cancellation,
        )
        .expect_err("pre-cancelled request must fail");
    assert_eq!(error.kind(), SdkErrorKind::Execution);
    assert_eq!(error.code(), "generation.execution.cancelled");
    assert!(!cancelled_output.exists());
    Ok(())
}
"##,
    )
    .unwrap();

    let output_root = root.join("generated");
    let result = Command::new(env!("CARGO"))
        .args(["run", "--offline", "--quiet", "--"])
        .arg(&output_root)
        .current_dir(&root)
        .env("CARGO_TARGET_DIR", root.join("target"))
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(result.stdout.is_empty());
    assert!(result.stderr.is_empty());
    assert!(output_root.join("manifest.json").is_file());

    fs::remove_dir_all(root).unwrap();
}
