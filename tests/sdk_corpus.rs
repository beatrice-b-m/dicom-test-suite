//! Consumer proof: all generator imports come exclusively from the supported SDK.
#[path = "support/generic_ct_bundle.rs"]
mod generic_ct_bundle;

use serde_json::{Value, json};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
use synth_dicom_gen::sdk::{
    CancellationToken, CorpusCaseDisposition, CorpusCaseStatus, CorpusPublicationState,
    CorpusSelector, CorpusValidationState, DicomTestSuite, GenerateCorpusOutcome,
    GenerateCorpusRequest, InspectCorpusRequest, ManifestKind, ReportKind, ReportRequest,
    ValidateRequest,
};

struct Fixture {
    root: PathBuf,
    members: PathBuf,
    descriptor: PathBuf,
    bytes: Vec<u8>,
}
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = fs::canonicalize(std::env::temp_dir())
            .unwrap()
            .join(format!(
                "synth-dicom-gen-sdk-corpus-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
        fs::create_dir(&root).unwrap();
        let members = root.join("members");
        assert!(
            std::process::Command::new("python3")
                .arg(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/scripts/build-current-corpus-definition-bundle.py"
                ))
                .arg(&members)
                .output()
                .unwrap()
                .status
                .success()
        );
        let bytes = fs::read(members.join("corpus-definition.json")).unwrap();
        let descriptor = root.join("selected-definition.json");
        fs::write(&descriptor, &bytes).unwrap();
        // Canonical descriptor is optional with explicit input; selected parent is not member root.
        fs::remove_file(members.join("corpus-definition.json")).unwrap();
        Self {
            root,
            members,
            descriptor,
            bytes,
        }
    }
    fn request(&self, name: &str, selector: CorpusSelector) -> GenerateCorpusRequest {
        GenerateCorpusRequest::from_json_bytes(
            self.bytes.clone(),
            &self.members,
            self.root.join(name),
            selector,
        )
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}
fn profile(name: &str) -> CorpusSelector {
    CorpusSelector::Profile {
        profile: name.into(),
        include_stress: false,
    }
}

#[test]
fn inspection_is_destination_free_and_agrees_with_generation_planning() {
    let fixture = Fixture::new();
    let product = DicomTestSuite::embedded().unwrap();
    let metadata = product
        .inspect_corpus(InspectCorpusRequest::from_file(
            &fixture.descriptor,
            &fixture.members,
        ))
        .unwrap();
    assert!(metadata.assessment().is_none());
    assert_eq!(metadata.profiles().len(), 8);
    assert!(
        metadata
            .profiles()
            .iter()
            .any(|p| p.profile_id() == "negative" && p.scope() == "expected_invalid")
    );
    let native = metadata
        .cases()
        .iter()
        .find(|c| c.case_id() == "classic/sc/mono2_u8_explicit_le")
        .unwrap();
    assert_eq!(native.status(), CorpusCaseStatus::Implemented);
    assert!(native.profiles().any(|p| p == "smoke"));
    for selector in [
        profile("smoke"),
        ids(&["derived/registration/spatial_ct_pair"]),
        ids(&["classic/dx/mono2_u12_jpeg_extended"]),
        ids(&["classic/sc/mono2_u16_jpeg2000_lossless"]),
    ] {
        let inspected = product
            .inspect_corpus(
                InspectCorpusRequest::from_json_bytes(fixture.bytes.clone(), &fixture.members)
                    .with_selection(selector.clone())
                    .with_seed(7)
                    .with_parallelism(2),
            )
            .unwrap();
        let assessment = inspected.assessment().unwrap();
        assert_eq!(assessment.seed(), 7);
        assert_eq!(assessment.parallelism(), 2);
        assert_eq!(assessment.validation_state(), CorpusValidationState::NotRun);
        assert_eq!(
            assessment.publication_state(),
            CorpusPublicationState::NotRun
        );
        let GenerateCorpusOutcome::Planned(preview) = product
            .generate_corpus(
                fixture
                    .request("never-published", selector)
                    .with_seed(7)
                    .with_parallelism(2)
                    .dry_run(true),
            )
            .unwrap()
        else {
            panic!("planning only")
        };
        assert_eq!(
            assessment.corpus_plan_sha256(),
            preview.corpus_plan_sha256()
        );
        assert_eq!(assessment.artifact_ids(), preview.artifact_ids());
        assert_eq!(
            assessment.identity_projection(),
            preview.identity_projection()
        );
        assert_eq!(
            assessment
                .cases()
                .iter()
                .map(|c| c.evidence())
                .collect::<Vec<_>>(),
            preview
                .cases()
                .iter()
                .map(|c| c.evidence())
                .collect::<Vec<_>>()
        );
        assert!(!fixture.root.join("never-published").exists());
    }
    let invalid = product
        .inspect_corpus(
            InspectCorpusRequest::from_json_bytes(b"not-json".to_vec(), &fixture.members)
                .with_parallelism(0),
        )
        .unwrap_err();
    assert_eq!(invalid.code(), "request.schema.invalid");
    let mut unsupported: Value = serde_json::from_slice(&fixture.bytes).unwrap();
    unsupported["corpus_definition_bundle_schema_version"] = json!("99.0.0");
    for (bytes, code) in [
        (b"{".to_vec(), "request.json.invalid"),
        (
            serde_json::to_vec(&unsupported).unwrap(),
            "request.version.unsupported",
        ),
        (vec![b' '; 1024 * 1024 + 1], "resource.limit.exceeded"),
    ] {
        assert_eq!(
            product
                .inspect_corpus(InspectCorpusRequest::from_json_bytes(
                    bytes,
                    &fixture.members
                ))
                .unwrap_err()
                .code(),
            code
        );
    }
    assert_eq!(
        product
            .inspect_corpus(
                InspectCorpusRequest::from_json_bytes(fixture.bytes.clone(), &fixture.members)
                    .with_selection(profile("not-a-profile"))
            )
            .unwrap_err()
            .code(),
        "request.schema.invalid"
    );
    let token = CancellationToken::new();
    token.cancel();
    assert_eq!(
        product
            .inspect_corpus_cancellable(
                InspectCorpusRequest::from_file(&fixture.descriptor, &fixture.members),
                &token
            )
            .unwrap_err()
            .code(),
        "generation.execution.cancelled"
    );
    fs::remove_dir_all(&fixture.members).unwrap();
    assert_eq!(
        metadata.corpus_definition_identity()["corpus_definition_sha256"],
        "571fa23fd392dd557ccdbe2db527698eaedc7078d86543efc68dfffc877411f7"
    );
}
fn ids(values: &[&str]) -> CorpusSelector {
    CorpusSelector::CaseIds {
        profile: "all".into(),
        include_stress: false,
        case_ids: values.iter().map(|v| (*v).into()).collect(),
    }
}
fn published(value: GenerateCorpusOutcome) -> synth_dicom_gen::sdk::PublishedCorpus {
    match value {
        GenerateCorpusOutcome::Published(value) => value,
        other => panic!("expected published: {other:?}"),
    }
}

#[test]
fn caller_named_ct_capability_is_complete_through_the_public_sdk() {
    let fixture = generic_ct_bundle::GenericCtBundle::new("sdk");
    let product = DicomTestSuite::embedded().unwrap();
    let inspected = product
        .inspect_corpus(
            InspectCorpusRequest::from_file(&fixture.descriptor, &fixture.members)
                .with_selection(generic_ct_bundle::selector())
                .with_seed(1)
                .with_parallelism(4),
        )
        .unwrap();
    let assessment = inspected.assessment().unwrap();
    assert_eq!(assessment.seed(), 1);
    assert_eq!(assessment.parallelism(), 4);
    assert_eq!(assessment.artifact_ids().len(), 1);
    assert_eq!(assessment.cases().len(), 1);
    assert_eq!(assessment.cases()[0].case_id(), generic_ct_bundle::CASE_ID);
    assert_eq!(
        assessment.cases()[0].disposition(),
        CorpusCaseDisposition::Ready
    );
    assert!(assessment.cases()[0].is_direct());
    assert_eq!(assessment.validation_state(), CorpusValidationState::NotRun);
    assert_eq!(
        assessment.publication_state(),
        CorpusPublicationState::NotRun
    );

    let run = published(
        product
            .generate_corpus(
                GenerateCorpusRequest::from_json_bytes(
                    fs::read(&fixture.descriptor).unwrap(),
                    &fixture.members,
                    fixture.root.join("sdk-output"),
                    generic_ct_bundle::selector(),
                )
                .with_seed(1)
                .with_parallelism(4),
            )
            .unwrap(),
    );
    assert_eq!(run.emitted_file_count(), 1);
    assert_eq!(run.seed(), 1);
    assert_eq!(run.publication_state(), CorpusPublicationState::Published);
    assert_eq!(run.validation_state(), CorpusValidationState::Passed);
    assert_eq!(run.corpus_plan_sha256(), assessment.corpus_plan_sha256());
    let manifest: Value = run.manifest().deserialize().unwrap();
    generic_ct_bundle::assert_manifest(&manifest, &fixture.identity);
    assert_eq!(
        inspected.corpus_definition_identity(),
        &manifest["identity_projection"]["corpus_definition"]["identity"]
    );
    generic_ct_bundle::assert_output_closure(&fixture, "sdk-output");
    let unexpected = fixture.root.join("sdk-output/unexpected-empty");
    fs::create_dir(&unexpected).unwrap();
    assert!(!generic_ct_bundle::output_closure_is_exact(
        &fixture,
        "sdk-output"
    ));
    fs::remove_dir(unexpected).unwrap();
    let validation = product
        .validate(ValidateRequest::new(run.output_root()))
        .unwrap();
    assert!(validation.is_valid());
    assert_eq!(validation.files_checked(), 1);
    assert!(validation.failures().is_empty());
    assert_eq!(validation.manifest().kind(), ManifestKind::ExternalCorpus);
    let report = product
        .report(ReportRequest::new(run.output_root()))
        .unwrap();
    assert_eq!(report.kind(), ReportKind::ExternalCorpus);
    assert_eq!(report.schema_version(), "2.0.0");
    generic_ct_bundle::assert_report(&report.deserialize::<Value>().unwrap(), &manifest);

    let recipe = fixture.members.join("cases/recipes/caller-signed-ct.json");
    let mut tampered = fs::read(&recipe).unwrap();
    tampered[0] ^= 1;
    fs::write(&recipe, tampered).unwrap();
    assert_eq!(
        product
            .inspect_corpus(InspectCorpusRequest::from_file(
                &fixture.descriptor,
                &fixture.members,
            ))
            .unwrap_err()
            .code(),
        "evidence.integrity.failed"
    );
}

#[test]
fn sdk_corpus_file_bytes_reproduce_and_support_readers() {
    if std::env::var_os("SYNTH_DICOM_GEN_SDK_GUIDE_CHILD").is_some() {
        // Only the isolated subprocess has this CWD and opt-in marker. Execute
        // the documented relative arguments, without process-global chdir.
        let product = DicomTestSuite::embedded().unwrap();
        let bare = GenerateCorpusRequest::from_file(
            "definition.json",
            "corpus-members",
            "generated/rejected",
            profile("smoke"),
        );
        assert_eq!(
            product.generate_corpus(bare).unwrap_err().code(),
            "resource.document.invalid"
        );
        assert!(!std::path::Path::new("generated/rejected").exists());
        let request = GenerateCorpusRequest::from_file(
            "./definition.json",
            "corpus-members",
            "generated/caller-smoke",
            CorpusSelector::Profile {
                profile: "smoke".into(),
                include_stress: false,
            },
        )
        .with_seed(1)
        .with_parallelism(2);
        let run = published(product.generate_corpus(request).unwrap());
        assert!(
            product
                .validate(ValidateRequest::new(run.output_root()))
                .unwrap()
                .is_valid()
        );
        assert_eq!(
            product
                .report(ReportRequest::new(run.output_root()))
                .unwrap()
                .schema_version(),
            "2.0.0"
        );
        assert_eq!(run.emitted_file_count(), 3);
    }
    let fixture = Fixture::new();
    let product = DicomTestSuite::embedded().unwrap();
    let first = published(
        product
            .generate_corpus(
                GenerateCorpusRequest::from_file(
                    &fixture.descriptor,
                    &fixture.members,
                    fixture.root.join("file"),
                    profile("smoke"),
                )
                .with_seed(1)
                .with_parallelism(2),
            )
            .unwrap(),
    );
    let second = published(
        product
            .generate_corpus(
                fixture
                    .request("bytes", profile("smoke"))
                    .with_seed(1)
                    .with_parallelism(2),
            )
            .unwrap(),
    );
    assert_eq!(
        first.manifest().json_bytes(),
        second.manifest().json_bytes()
    );
    assert_eq!(first.manifest().kind(), ManifestKind::ExternalCorpus);
    assert_eq!(first.manifest().schema_version(), "2.0.0");
    assert_eq!(first.seed(), 1);
    assert_eq!(first.emitted_file_count(), 3);
    assert_eq!(first.output_bytes(), 2790);
    assert_eq!(first.corpus_plan_sha256(), second.corpus_plan_sha256());
    assert_eq!(first.publication_state(), CorpusPublicationState::Published);
    assert_eq!(first.validation_state(), CorpusValidationState::Passed);
    let manifest: Value = first.manifest().deserialize().unwrap();
    for file in manifest["files"].as_array().unwrap() {
        let path = file["path"].as_str().unwrap();
        assert_eq!(
            fs::read(first.output_root().join(path)).unwrap(),
            fs::read(second.output_root().join(path)).unwrap()
        );
    }
    fs::remove_dir_all(&fixture.members).unwrap();
    fs::remove_file(&fixture.descriptor).unwrap();
    let moved = fixture.root.join("moved");
    fs::rename(first.output_root(), &moved).unwrap();
    let validated = product.validate(ValidateRequest::new(&moved)).unwrap();
    assert!(validated.is_valid());
    assert_eq!(validated.manifest().kind(), ManifestKind::ExternalCorpus);
    let report = product.report(ReportRequest::new(&moved)).unwrap();
    assert_eq!(report.kind(), ReportKind::ExternalCorpus);
    assert_eq!(report.schema_version(), "2.0.0");
    assert_eq!(
        report.deserialize::<Value>().unwrap()["source_manifest"],
        manifest
    );
    let runtime = json!({"runtime_id":"provider/fixture", "runtime_kind":"generation_provider", "executable_sha256":"a".repeat(64), "version":"1.0", "invocation_sha256":"b".repeat(64)});
    let mut second_runtime = runtime.clone();
    second_runtime["invocation_sha256"] = json!("c".repeat(64));
    for mutation in 0..4 {
        let mut invalid = manifest.clone();
        match mutation {
            0 => invalid["manifest_schema_version"] = json!("99.0.0"),
            1 => {
                invalid
                    .as_object_mut()
                    .unwrap()
                    .remove("identity_projection");
            }
            2 => {
                *invalid
                    .pointer_mut("/identity_projection/engine/engine_sha256")
                    .expect("existing engine digest, not an unknown-field mutation") =
                    json!("malformed")
            }
            _ => {
                invalid["identity_projection"]["external_runtime"] =
                    json!([runtime, second_runtime])
            }
        }
        fs::write(
            moved.join("manifest.json"),
            serde_json::to_vec(&invalid).unwrap(),
        )
        .unwrap();
        assert!(
            product.validate(ValidateRequest::new(&moved)).is_err(),
            "mutation {mutation}"
        );
        assert!(
            product.report(ReportRequest::new(&moved)).is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn sdk_corpus_preview_and_empty_execution_are_distinct() {
    let fixture = Fixture::new();
    let product = DicomTestSuite::embedded().unwrap();
    let GenerateCorpusOutcome::Planned(preview) = product
        .generate_corpus(
            fixture
                .request("dry", profile("smoke"))
                .with_seed(7)
                .dry_run(true),
        )
        .unwrap()
    else {
        panic!("expected plan")
    };
    assert_eq!(preview.evidence_version(), "1.0.0");
    assert_eq!(preview.seed(), 7);
    assert_eq!(preview.artifact_ids().len(), 3);
    assert!(
        preview
            .cases()
            .iter()
            .all(|c| c.disposition() == CorpusCaseDisposition::Ready && c.is_direct())
    );
    assert_eq!(preview.publication_state(), CorpusPublicationState::NotRun);
    assert_eq!(preview.validation_state(), CorpusValidationState::NotRun);
    assert!(!preview.identity_projection()["corpus_definition"]["identity"].is_null());
    for (name, id, disposition) in [
        (
            "planned",
            "classic/dx/mono2_u12_jpeg_extended",
            CorpusCaseDisposition::Planned,
        ),
        (
            "unavailable",
            "classic/sc/mono2_u16_jpeg2000_lossless",
            CorpusCaseDisposition::Unavailable,
        ),
    ] {
        let GenerateCorpusOutcome::NoExecutableCases(no) = product
            .generate_corpus(fixture.request(name, ids(&[id])))
            .unwrap()
        else {
            panic!("expected no execution")
        };
        assert!(no.artifact_ids().is_empty());
        assert_eq!(no.cases().len(), 1);
        assert_eq!(no.cases()[0].case_id(), id);
        assert_eq!(no.cases()[0].disposition(), disposition);
        assert!(no.cases()[0].evidence()["reason_code"].is_string());
        assert_eq!(no.publication_state(), CorpusPublicationState::NotRun);
        assert_eq!(no.validation_state(), CorpusValidationState::NotRun);
        assert!(!no.requested_output_root().exists());
        assert!(matches!(
            product
                .generate_corpus(fixture.request(name, ids(&[id])).dry_run(true))
                .unwrap(),
            GenerateCorpusOutcome::Planned(_)
        ));
    }
    assert!(!fixture.root.join("dry").exists());
}

#[test]
fn sdk_corpus_case_selection_retains_dependency_and_multifile_evidence() {
    let fixture = Fixture::new();
    let product = DicomTestSuite::embedded().unwrap();
    let run = published(
        product
            .generate_corpus(
                fixture.request("dependency", ids(&["derived/registration/spatial_ct_pair"])),
            )
            .unwrap(),
    );
    let manifest: Value = run.manifest().deserialize().unwrap();
    assert!(
        manifest["selection_ledger"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["selection"] == "dependency")
    );
    assert_eq!(
        run.emitted_file_count(),
        manifest["files"].as_array().unwrap().len()
    );
    assert!(
        product
            .validate(ValidateRequest::new(run.output_root()))
            .unwrap()
            .is_valid()
    );
    let mixed = published(
        product
            .generate_corpus(fixture.request(
                "mixed",
                ids(&[
                    "classic/sc/mono1_u8_explicit_le",
                    "classic/dx/mono2_u12_jpeg_extended",
                ]),
            ))
            .unwrap(),
    );
    let manifest: Value = mixed.manifest().deserialize().unwrap();
    assert_eq!(manifest["selection_ledger"].as_array().unwrap().len(), 2);
    assert_eq!(mixed.emitted_file_count(), 1);
    let multi = published(
        product
            .generate_corpus(
                fixture.request("multi", ids(&["geometry/ct/nonuniform_slice_spacing"])),
            )
            .unwrap(),
    );
    let manifest: Value = multi.manifest().deserialize().unwrap();
    assert_eq!(manifest["selection_ledger"].as_array().unwrap().len(), 1);
    assert!(multi.emitted_file_count() > 1);
}

#[test]
fn sdk_corpus_errors_preserve_codes_and_do_not_publish() {
    let fixture = Fixture::new();
    let product = DicomTestSuite::embedded().unwrap();
    for selector in [
        ids(&[]),
        ids(&["unknown"]),
        ids(&["classic/sc/mono1_u8_explicit_le"; 2]),
        profile("unknown"),
        CorpusSelector::CaseIds {
            profile: "smoke".into(),
            include_stress: false,
            case_ids: vec!["derived/registration/spatial_ct_pair".into()],
        },
    ] {
        assert_eq!(
            product
                .generate_corpus(fixture.request("invalid", selector))
                .unwrap_err()
                .code(),
            "request.schema.invalid"
        );
    }
    assert_eq!(
        product
            .generate_corpus(
                fixture
                    .request("invalid", profile("smoke"))
                    .with_parallelism(0)
            )
            .unwrap_err()
            .code(),
        "request.schema.invalid"
    );
    let token = CancellationToken::new();
    token.cancel();
    assert_eq!(
        product
            .generate_corpus_cancellable(fixture.request("invalid", profile("smoke")), &token)
            .unwrap_err()
            .code(),
        "generation.execution.cancelled"
    );
    fs::create_dir(fixture.root.join("exists")).unwrap();
    fs::write(fixture.root.join("exists/sentinel"), b"keep").unwrap();
    assert_eq!(
        product
            .generate_corpus(fixture.request("exists", profile("smoke")))
            .unwrap_err()
            .code(),
        "output.destination.exists"
    );
    assert_eq!(
        fs::read(fixture.root.join("exists/sentinel")).unwrap(),
        b"keep"
    );
    for (bytes, code) in [
        (b"{".to_vec(), "request.json.invalid"),
        (
            {
                let mut value: Value = serde_json::from_slice(&fixture.bytes).unwrap();
                value["corpus_definition_bundle_schema_version"] = json!("99.0.0");
                serde_json::to_vec(&value).unwrap()
            },
            "request.version.unsupported",
        ),
    ] {
        assert_eq!(
            product
                .generate_corpus(GenerateCorpusRequest::from_json_bytes(
                    bytes,
                    &fixture.members,
                    fixture.root.join("invalid"),
                    profile("smoke")
                ))
                .unwrap_err()
                .code(),
            code
        );
    }
    for (request, code) in [
        (
            GenerateCorpusRequest::from_file(
                fixture.root.join("missing.json"),
                &fixture.members,
                fixture.root.join("invalid"),
                profile("smoke"),
            ),
            "io.read.failed",
        ),
        (
            GenerateCorpusRequest::from_json_bytes(
                fixture.bytes.clone(),
                fixture.root.join("missing-root"),
                fixture.root.join("invalid"),
                profile("smoke"),
            ),
            "io.read.failed",
        ),
        (
            GenerateCorpusRequest::from_json_bytes(
                fixture.bytes.clone(),
                "",
                fixture.root.join("invalid"),
                profile("smoke"),
            ),
            "resource.document.invalid",
        ),
        (
            GenerateCorpusRequest::from_json_bytes(
                vec![b' '; 1024 * 1024 + 1],
                &fixture.members,
                fixture.root.join("invalid"),
                profile("smoke"),
            ),
            "resource.limit.exceeded",
        ),
    ] {
        assert_eq!(product.generate_corpus(request).unwrap_err().code(), code);
    }
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&fixture.members, fixture.root.join("symlink-members")).unwrap();
        assert_eq!(
            product
                .generate_corpus(GenerateCorpusRequest::from_json_bytes(
                    fixture.bytes.clone(),
                    fixture.root.join("symlink-members"),
                    fixture.root.join("invalid"),
                    profile("smoke")
                ))
                .unwrap_err()
                .code(),
            "resource.document.invalid"
        );
    }
    let request = fixture.request("changed", profile("smoke"));
    fs::write(
        fixture.members.join("cases/registry.json"),
        b"changed after request construction",
    )
    .unwrap();
    assert_eq!(
        product.generate_corpus(request).unwrap_err().code(),
        "evidence.integrity.failed"
    );
    assert!(!fixture.root.join("invalid").exists());
    assert!(!fixture.root.join("changed").exists());
}

#[test]
fn sdk_corpus_nonimplemented_metadata_is_lossless() {
    let fixture = Fixture::new();
    let product = DicomTestSuite::embedded().unwrap();
    let original: Value =
        serde_json::from_slice(&fs::read(fixture.members.join("cases/registry.json")).unwrap())
            .unwrap();
    for (status, disposition) in [
        ("skipped", CorpusCaseDisposition::Skipped),
        ("blocked", CorpusCaseDisposition::Blocked),
        ("deprecated", CorpusCaseDisposition::Deprecated),
    ] {
        let mut registry = original.clone();
        let row = registry["cases"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|r| r["case_id"] == "classic/dx/mono2_u12_jpeg_extended")
            .unwrap();
        row["status"] = json!(status);
        row["skip"] = json!({"reason_code":"test.captured.reason","message":"Synthetic bounded metadata only","recheck_phase":"r5"});
        let expected = row.clone();
        let bytes = serde_json::to_vec(&registry).unwrap();
        fs::write(fixture.members.join("cases/registry.json"), &bytes).unwrap();
        let mut descriptor: Value = serde_json::from_slice(&fixture.bytes).unwrap();
        descriptor["registry"]["size_bytes"] = json!(bytes.len());
        let digest = std::process::Command::new("shasum")
            .args(["-a", "256"])
            .arg(fixture.members.join("cases/registry.json"))
            .output()
            .unwrap();
        assert!(digest.status.success());
        descriptor["registry"]["sha256"] = json!(
            String::from_utf8(digest.stdout)
                .unwrap()
                .split_whitespace()
                .next()
                .unwrap()
        );
        let request = GenerateCorpusRequest::from_json_bytes(
            serde_json::to_vec(&descriptor).unwrap(),
            &fixture.members,
            fixture.root.join(status),
            ids(&["classic/dx/mono2_u12_jpeg_extended"]),
        );
        let GenerateCorpusOutcome::NoExecutableCases(preview) =
            product.generate_corpus(request).unwrap()
        else {
            panic!("not executable")
        };
        assert_eq!(preview.cases()[0].disposition(), disposition);
        assert_eq!(preview.cases()[0].evidence()["case_definition"], expected);
        assert_eq!(
            preview.cases()[0].evidence()["reason_code"],
            "test.captured.reason"
        );
        assert!(!fixture.root.join(status).exists());
    }
}

#[test]
fn sdk_corpus_works_from_unrelated_cwd() {
    let fixture = Fixture::new();
    fs::rename(&fixture.members, fixture.root.join("corpus-members")).unwrap();
    fs::copy(&fixture.descriptor, fixture.root.join("definition.json")).unwrap();
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "sdk_corpus::sdk_corpus_file_bytes_reproduce_and_support_readers",
        ])
        .current_dir(&fixture.root)
        .env("SYNTH_DICOM_GEN_SDK_GUIDE_CHILD", "1")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
