use std::sync::atomic::{AtomicU64, Ordering};

use dicom_test_suite::sdk::{
    ComposeRequest, DicomTestSuite, ManifestKind, ReportKind, ReportRequest, SdkErrorKind,
    ValidateRequest,
};

static NEXT: AtomicU64 = AtomicU64::new(0);

fn output(label: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "dicom-test-suite-sdk-{label}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ))
}

#[test]
fn sdk_embedded_discovery_is_typed_and_conservative() {
    let product = DicomTestSuite::embedded().unwrap();
    let version = product.version().unwrap();
    let capabilities = product.capabilities().unwrap();

    assert_eq!(version.product.name, "dicom-test-suite");
    assert_eq!(version.cli_api_version, "1.0.0");
    assert_eq!(
        version.product_resources.resource_set_sha256,
        capabilities.product_resources.resource_set_sha256
    );
    assert_eq!(capabilities.structural_assembly.availability, "unavailable");
}

#[test]
fn sdk_explicit_resources_fail_closed_with_stable_error() {
    let root = std::env::temp_dir().join(format!(
        "dicom-test-suite-sdk-resources-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).unwrap();

    let error = DicomTestSuite::explicit_resource_root(&root).unwrap_err();
    assert_eq!(error.kind(), SdkErrorKind::Internal);
    assert_eq!(error.code(), "io.read.failed");
    assert!(error.retryable());
    assert!(!error.diagnostic().is_empty());

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn sdk_compose_bytes_returns_typed_publish_and_dry_run_outcomes() {
    let product = DicomTestSuite::embedded().unwrap();
    let spec = include_bytes!("fixtures/composition/valid/template-only.json");
    let published_root = output("published");
    let published = product
        .compose(
            ComposeRequest::from_json_bytes(spec.as_slice(), ".", &published_root).with_seed(9),
        )
        .unwrap();

    assert!(published.published());
    assert_eq!(published.instances_written(), 1);
    assert!(published.output_bytes() > 0);
    assert!(published.plan_preview().is_none());
    let manifest = published.manifest().unwrap();
    assert_eq!(manifest.kind(), ManifestKind::QualifiedComposition);
    assert_eq!(manifest.schema_version(), "0.5.0");
    assert_eq!(manifest.seed(), 9);
    assert_eq!(manifest.path(), published_root.join("manifest.json"));

    let dry_root = output("dry");
    let dry = product
        .compose(
            ComposeRequest::from_json_bytes(spec.as_slice(), ".", &dry_root)
                .with_seed(9)
                .dry_run(true),
        )
        .unwrap();
    assert!(!dry.published());
    assert!(dry.manifest().is_none());
    assert_eq!(dry.plan_preview().unwrap().artifact_count(), 1);
    assert_eq!(published.corpus_plan_sha256(), dry.corpus_plan_sha256());
    assert!(!dry_root.exists());

    std::fs::remove_dir_all(published_root).unwrap();
}

#[test]
fn sdk_validation_and_report_return_typed_schema_bound_results() {
    let product = DicomTestSuite::embedded().unwrap();
    let root = output("validate-report");
    product
        .compose(ComposeRequest::from_json_bytes(
            include_bytes!("fixtures/composition/valid/template-only.json").as_slice(),
            ".",
            &root,
        ))
        .unwrap();

    let validation = product.validate(ValidateRequest::new(&root)).unwrap();
    assert!(validation.is_valid());
    assert_eq!(validation.files_checked(), 1);
    assert_eq!(
        validation.manifest().kind(),
        ManifestKind::QualifiedComposition
    );

    let report = product.report(ReportRequest::new(&root)).unwrap();
    assert_eq!(report.kind(), ReportKind::QualifiedComposition);
    assert_eq!(report.schema_version(), "0.1.0");
    assert!(!report.json_bytes().is_empty());

    std::fs::remove_dir_all(root).unwrap();
}
