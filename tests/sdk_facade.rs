use dicom_test_suite::sdk::{DicomTestSuite, SdkErrorKind};

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
