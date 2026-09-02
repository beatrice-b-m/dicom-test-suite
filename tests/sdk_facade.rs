use std::sync::atomic::{AtomicU64, Ordering};

use synth_dicom_gen::sdk::{
    AssembleRequest, CancellationToken, ComposeRequest, DicomTestSuite, ManifestKind, ReportKind,
    ReportRequest, SdkErrorKind, ValidateRequest,
};
use synth_dicom_gen::{GenerateOptions, prepare_generation_run, write_generation_run};

static NEXT: AtomicU64 = AtomicU64::new(0);

fn output(label: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "synth-dicom-gen-sdk-{label}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ))
}

#[test]
fn sdk_embedded_discovery_is_typed_and_conservative() {
    let product = DicomTestSuite::embedded().unwrap();
    let version = product.version().unwrap();
    let capabilities = product.capabilities().unwrap();

    assert_eq!(version.product.name, "synth-dicom-gen");
    assert_eq!(version.cli_api_version, "1.0.0");
    assert!(version.product_resources.is_none());
    assert!(capabilities.product_resources.is_none());
    assert_eq!(capabilities.structural_assembly.availability, "available");
}

#[test]
fn sdk_explicit_resources_fail_closed_with_stable_error() {
    let root = std::env::temp_dir().join(format!(
        "synth-dicom-gen-sdk-resources-{}",
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
    assert_eq!(manifest.schema_version(), "1.0.0");
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
    assert_eq!(report.schema_version(), "1.0.0");
    assert!(!report.json_bytes().is_empty());

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn sdk_composition_validate_and_report_read_exact_supported_manifest_versions() {
    let product = DicomTestSuite::embedded().unwrap();
    let root = output("composition-readers");
    product
        .compose(ComposeRequest::from_json_bytes(
            include_bytes!("fixtures/composition/valid/template-only.json").as_slice(),
            ".",
            &root,
        ))
        .unwrap();
    let manifest_path = root.join("manifest.json");
    let current: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&manifest_path).unwrap()).unwrap();

    for version in ["1.0.0", "0.5.0", "0.4.0"] {
        let mut manifest = current.clone();
        manifest["manifest_schema_version"] = version.into();
        if version != "1.0.0" {
            manifest
                .as_object_mut()
                .unwrap()
                .remove("identity_projection");
        }
        if version == "0.4.0" {
            manifest
                .as_object_mut()
                .unwrap()
                .remove("product_resources");
        }
        std::fs::write(
            &manifest_path,
            serde_json::to_vec_pretty(&manifest).unwrap(),
        )
        .unwrap();
        let validation = product.validate(ValidateRequest::new(&root)).unwrap();
        assert!(validation.is_valid());
        assert_eq!(validation.manifest().schema_version(), version);
        assert_eq!(
            validation.manifest().kind(),
            ManifestKind::QualifiedComposition
        );
        let report = product.report(ReportRequest::new(&root)).unwrap();
        assert_eq!(report.kind(), ReportKind::QualifiedComposition);
        assert_eq!(
            report.schema_version(),
            if version == "1.0.0" { "1.0.0" } else { "0.1.0" }
        );
        let report_json: serde_json::Value = report.deserialize().unwrap();
        let legacy_report: serde_json::Value =
            serde_json::from_slice(include_bytes!("fixtures/cli/composition-report-v0.1.json"))
                .unwrap();
        if version == "1.0.0" {
            assert_eq!(
                report_json["identity_projection"],
                manifest["identity_projection"]
            );
            let mut normalized = report_json;
            normalized
                .as_object_mut()
                .unwrap()
                .remove("identity_projection");
            normalized["composition_report_schema_version"] = "0.1.0".into();
            assert_eq!(normalized, legacy_report);
        } else {
            assert!(report_json.get("identity_projection").is_none());
            assert_eq!(report_json, legacy_report);
        }
    }

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn sdk_rejects_invalid_composition_identity_contracts_before_semantics() {
    let product = DicomTestSuite::embedded().unwrap();
    let root = output("duplicate-runtime-identity");
    product
        .compose(ComposeRequest::from_json_bytes(
            include_bytes!("fixtures/composition/valid/template-only.json").as_slice(),
            ".",
            &root,
        ))
        .unwrap();
    let manifest_path = root.join("manifest.json");
    let current: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&manifest_path).unwrap()).unwrap();
    let runtime = serde_json::json!({
        "runtime_id": "provider/primary/fixture",
        "runtime_kind": "generation_provider",
        "executable_sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "version": "1.0.0",
        "invocation_sha256": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
    });
    let mut changed = runtime.clone();
    changed["invocation_sha256"] =
        serde_json::json!("cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc");
    for (label, manifest, diagnostic) in [
        (
            "unknown-version",
            {
                let mut value = current.clone();
                value["manifest_schema_version"] = "9.0.0".into();
                value
            },
            "unsupported composition manifest schema version",
        ),
        (
            "missing-identity",
            {
                let mut value = current.clone();
                value.as_object_mut().unwrap().remove("identity_projection");
                value
            },
            "identity_projection",
        ),
        (
            "malformed-digest",
            {
                let mut value = current.clone();
                value["identity_projection"]["engine"]["engine_sha256"] = "short".into();
                value
            },
            "short",
        ),
        (
            "duplicate-runtime",
            {
                let mut value = current.clone();
                value["identity_projection"]["external_runtime"] =
                    serde_json::json!([runtime, changed]);
                value
            },
            "duplicate runtime_id",
        ),
    ] {
        std::fs::write(
            &manifest_path,
            serde_json::to_vec_pretty(&manifest).unwrap(),
        )
        .unwrap();
        let validation = product.validate(ValidateRequest::new(&root)).unwrap_err();
        assert!(
            validation.diagnostic().contains(diagnostic),
            "{label} validate diagnostic: {}",
            validation.diagnostic()
        );
        let report = product.report(ReportRequest::new(&root)).unwrap_err();
        assert!(
            report.diagnostic().contains(diagnostic),
            "{label} report diagnostic: {}",
            report.diagnostic()
        );
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn sdk_curated_validate_and_report_read_exact_supported_manifest_versions() {
    let product = DicomTestSuite::embedded().unwrap();
    let root = output("curated-readers");
    let run = prepare_generation_run(GenerateOptions {
        profile: "smoke".into(),
        out_dir: root.clone(),
        seed: 1,
        include_stress: false,
    })
    .unwrap();
    write_generation_run(&run).unwrap();
    let manifest_path = root.join("manifest.json");
    let current: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&manifest_path).unwrap()).unwrap();

    for version in ["1.0.0", "0.3.0", "0.2.0"] {
        let mut manifest = current.clone();
        manifest["manifest_schema_version"] = version.into();
        if version != "1.0.0" {
            manifest
                .as_object_mut()
                .unwrap()
                .remove("identity_projection");
        }
        if version == "0.2.0" {
            manifest
                .as_object_mut()
                .unwrap()
                .remove("product_resources");
        }
        std::fs::write(
            &manifest_path,
            serde_json::to_vec_pretty(&manifest).unwrap(),
        )
        .unwrap();
        let validation = product.validate(ValidateRequest::new(&root)).unwrap();
        assert!(validation.is_valid());
        assert_eq!(
            validation.manifest().kind(),
            ManifestKind::CuratedGeneration
        );
        assert_eq!(validation.manifest().schema_version(), version);
        let report = product.report(ReportRequest::new(&root)).unwrap();
        assert_eq!(report.kind(), ReportKind::CuratedCoverage);
        assert_eq!(
            report.schema_version(),
            if version == "1.0.0" { "1.0.0" } else { "0.1.0" }
        );
        let report_json: serde_json::Value = report.deserialize().unwrap();
        if version == "1.0.0" {
            assert_eq!(
                report_json["identity_projection"],
                manifest["identity_projection"]
            );
        } else {
            assert!(report_json.get("identity_projection").is_none());
        }
    }

    let mut unknown = current.clone();
    unknown["manifest_schema_version"] = "9.0.0".into();
    std::fs::write(&manifest_path, serde_json::to_vec_pretty(&unknown).unwrap()).unwrap();
    assert!(product.validate(ValidateRequest::new(&root)).is_err());
    assert!(product.report(ReportRequest::new(&root)).is_err());

    let mut malformed_identity = current.clone();
    malformed_identity["identity_projection"]["engine"]["engine_sha256"] = "not-a-digest".into();
    std::fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&malformed_identity).unwrap(),
    )
    .unwrap();
    assert!(product.validate(ValidateRequest::new(&root)).is_err());
    assert!(product.report(ReportRequest::new(&root)).is_err());

    let runtime = serde_json::json!({
        "runtime_id": "provider/primary/fixture",
        "runtime_kind": "generation_provider",
        "executable_sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "version": "1.0.0",
        "invocation_sha256": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
    });
    let mut changed_runtime = runtime.clone();
    changed_runtime["invocation_sha256"] =
        "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc".into();
    let mut duplicate_runtime = current.clone();
    duplicate_runtime["identity_projection"]["external_runtime"] =
        serde_json::json!([runtime, changed_runtime]);
    std::fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&duplicate_runtime).unwrap(),
    )
    .unwrap();
    let validation = product.validate(ValidateRequest::new(&root)).unwrap_err();
    assert!(validation.diagnostic().contains("duplicate runtime_id"));
    let report = product.report(ReportRequest::new(&root)).unwrap_err();
    assert!(report.diagnostic().contains("duplicate runtime_id"));

    let mut missing_identity = current;
    missing_identity
        .as_object_mut()
        .unwrap()
        .remove("identity_projection");
    std::fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&missing_identity).unwrap(),
    )
    .unwrap();
    assert!(product.validate(ValidateRequest::new(&root)).is_err());
    assert!(product.report(ReportRequest::new(&root)).is_err());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn sdk_file_and_byte_requests_share_asset_root_and_pipeline() {
    let product = DicomTestSuite::embedded().unwrap();
    let workspace = output("file-bytes");
    let assets = workspace.join("caller-assets");
    std::fs::create_dir_all(&assets).unwrap();
    std::fs::write(assets.join("source.raw"), [0_u8, 1, 2, 3]).unwrap();
    let spec = br#"{
      "composition_spec_schema_version":"0.1.0",
      "instances":[{
        "instance_id":"primary",
        "template":{"id":"classic/secondary-capture/monochrome"},
        "content":[{"slot":"pixels","source":{
          "kind":"local_file","path":"source.raw",
          "sha256":"054edec1d0211f624fed0cbca9d4f9400b0e491c43742af2c5b0abebf0c990d8",
          "pixel":{"rows":2,"columns":2,"frames":1,"samples_per_pixel":1,
            "photometric_interpretation":"MONOCHROME2","sample_type":"uint",
            "bits_allocated":8,"bits_stored":8,"high_bit":7,"byte_order":"little"}
        }}]
      }]
    }"#;
    let spec_path = workspace.join("request.json");
    std::fs::write(&spec_path, spec).unwrap();
    let file_root = output("file-input");
    let bytes_root = output("bytes-input");

    let from_file = product
        .compose(
            ComposeRequest::from_file(&spec_path, &file_root)
                .with_caller_asset_root(&assets)
                .with_seed(7),
        )
        .unwrap();
    let from_bytes = product
        .compose(ComposeRequest::from_json_bytes(spec, &assets, &bytes_root).with_seed(7))
        .unwrap();

    assert_eq!(
        from_file.corpus_plan_sha256(),
        from_bytes.corpus_plan_sha256()
    );
    assert_eq!(
        std::fs::read(file_root.join("instances/primary.dcm")).unwrap(),
        std::fs::read(bytes_root.join("instances/primary.dcm")).unwrap()
    );
    assert_eq!(
        from_file.manifest().unwrap().json_bytes(),
        from_bytes.manifest().unwrap().json_bytes()
    );

    std::fs::remove_dir_all(workspace).unwrap();
    std::fs::remove_dir_all(file_root).unwrap();
    std::fs::remove_dir_all(bytes_root).unwrap();
}

#[test]
fn sdk_precancelled_request_returns_typed_error_and_publishes_nothing() {
    let product = DicomTestSuite::embedded().unwrap();
    let root = output("cancelled");
    let cancellation = CancellationToken::new();
    cancellation.cancel();

    let error = product
        .compose_cancellable(
            ComposeRequest::from_json_bytes(
                include_bytes!("fixtures/composition/valid/template-only.json").as_slice(),
                ".",
                &root,
            ),
            &cancellation,
        )
        .unwrap_err();

    assert!(cancellation.is_cancelled());
    assert_eq!(error.kind(), SdkErrorKind::Execution);
    assert_eq!(error.code(), "generation.execution.cancelled");
    assert!(error.retryable());
    assert!(!root.exists());
}

#[test]
fn sdk_structural_assembly_returns_no_claim_typed_manifest() {
    let product = DicomTestSuite::embedded().unwrap();
    let root = output("structural");
    let request = br#"{"assembly_request_schema_version":"1.0.0","instances":[{"instance_id":"primary","sop_class_uid":"1.2.840.10008.5.1.4.1.1.7","elements":[]}]}"#;
    let outcome = product
        .assemble(AssembleRequest::from_json_bytes(request.as_slice(), ".", &root).with_seed(4))
        .unwrap();
    assert!(outcome.published());
    assert_eq!(outcome.artifacts_written(), 1);
    let manifest = outcome.manifest().unwrap();
    assert_eq!(manifest.kind(), ManifestKind::StructuralAssembly);
    assert_eq!(manifest.schema_version(), "2.0.0");
    let validation = product.validate(ValidateRequest::new(&root)).unwrap();
    assert!(validation.is_valid());
    let report = product.report(ReportRequest::new(&root)).unwrap();
    assert_eq!(report.kind(), ReportKind::StructuralAssembly);
    assert_eq!(report.schema_version(), "2.0.0");
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn sdk_assembly_validate_and_report_read_exact_supported_manifest_versions() {
    let product = DicomTestSuite::embedded().unwrap();
    let root = output("assembly-readers");
    product
        .assemble(
            AssembleRequest::from_json_bytes(
                include_bytes!("fixtures/cli/assembly-request-seed5.json").as_slice(),
                ".",
                &root,
            )
            .with_seed(5),
        )
        .unwrap();
    let manifest_path = root.join("manifest.json");
    let current = std::fs::read(&manifest_path).unwrap();
    for (version, bytes) in [
        ("2.0.0", current.as_slice()),
        (
            "1.0.0",
            include_bytes!("fixtures/cli/assembly-manifest-v1.json").as_slice(),
        ),
    ] {
        std::fs::write(&manifest_path, bytes).unwrap();
        let validation = product.validate(ValidateRequest::new(&root)).unwrap();
        assert!(validation.is_valid());
        assert_eq!(validation.manifest().schema_version(), version);
        assert_eq!(
            validation.manifest().kind(),
            ManifestKind::StructuralAssembly
        );
        let report = product.report(ReportRequest::new(&root)).unwrap();
        assert_eq!(report.kind(), ReportKind::StructuralAssembly);
        assert_eq!(
            report.schema_version(),
            if version == "2.0.0" { "2.0.0" } else { "1.0.0" }
        );
        let report_json: serde_json::Value = report.deserialize().unwrap();
        let legacy_report: serde_json::Value = serde_json::from_slice(include_bytes!(
            "fixtures/cli/structural-assembly-report-v1.json"
        ))
        .unwrap();
        if version == "2.0.0" {
            let manifest: serde_json::Value = serde_json::from_slice(bytes).unwrap();
            assert_eq!(
                report_json["identity_projection"],
                manifest["identity_projection"]
            );
            let mut normalized = report_json;
            normalized
                .as_object_mut()
                .unwrap()
                .remove("identity_projection");
            normalized["structural_assembly_report_schema_version"] = "1.0.0".into();
            assert_eq!(normalized, legacy_report);
        } else {
            assert!(report_json.get("identity_projection").is_none());
            assert_eq!(report_json, legacy_report);
        }
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn sdk_rejects_invalid_assembly_identity_contracts_before_semantics() {
    let product = DicomTestSuite::embedded().unwrap();
    let root = output("assembly-identity-rejections");
    product
        .assemble(
            AssembleRequest::from_json_bytes(
                include_bytes!("fixtures/cli/assembly-request-seed5.json").as_slice(),
                ".",
                &root,
            )
            .with_seed(5),
        )
        .unwrap();
    let manifest_path = root.join("manifest.json");
    let current: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&manifest_path).unwrap()).unwrap();
    let runtime = serde_json::json!({
        "runtime_id": "provider/primary/fixture",
        "runtime_kind": "generation_provider",
        "executable_sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "version": "1.0.0",
        "invocation_sha256": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
    });
    let mut changed_runtime = runtime.clone();
    changed_runtime["invocation_sha256"] =
        serde_json::json!("cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc");
    for (label, manifest, diagnostic) in [
        (
            "unknown-version",
            {
                let mut value = current.clone();
                value["manifest_schema_version"] = "9.0.0".into();
                value
            },
            "unsupported assembly manifest schema version",
        ),
        (
            "missing-identity",
            {
                let mut value = current.clone();
                value.as_object_mut().unwrap().remove("identity_projection");
                value
            },
            "identity_projection",
        ),
        (
            "malformed-digest",
            {
                let mut value = current.clone();
                value["identity_projection"]["engine"]["engine_sha256"] = "short".into();
                value
            },
            "short",
        ),
        (
            "duplicate-runtime",
            {
                let mut value = current.clone();
                value["identity_projection"]["external_runtime"] =
                    serde_json::json!([runtime, changed_runtime]);
                value
            },
            "duplicate runtime_id",
        ),
    ] {
        std::fs::write(
            &manifest_path,
            serde_json::to_vec_pretty(&manifest).unwrap(),
        )
        .unwrap();
        let validation = product.validate(ValidateRequest::new(&root)).unwrap_err();
        assert!(
            validation.diagnostic().contains(diagnostic),
            "{label} validate diagnostic: {}",
            validation.diagnostic()
        );
        let report = product.report(ReportRequest::new(&root)).unwrap_err();
        assert!(
            report.diagnostic().contains(diagnostic),
            "{label} report diagnostic: {}",
            report.diagnostic()
        );
    }
    std::fs::remove_dir_all(root).unwrap();
}
