//! Explicit R5 isolated-source proof: generator imports use only the supported facade.
use serde_json::{Value, json};
use std::{fs, path::PathBuf};
use synth_dicom_gen::sdk::{
    CancellationToken, CorpusCaseDisposition, CorpusPublicationState, CorpusSelector,
    CorpusValidationState, DicomTestSuite, GenerateCorpusOutcome, GenerateCorpusRequest,
    InspectCorpusRequest, ManifestKind, ReportKind, ReportRequest, ValidateRequest,
};

fn profile(name: &str) -> CorpusSelector {
    CorpusSelector::Profile {
        profile: name.into(),
        include_stress: false,
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = PathBuf::from(std::env::args_os().nth(1).expect("artifact root"));
    assert!(!root.join("source").exists());
    assert!(!root.join("consumer").exists());
    let bundle = root.join("caller-smoke");
    let descriptor = bundle.join("corpus-definition.json");
    let bytes = fs::read(&descriptor)?;
    let product = DicomTestSuite::embedded()?;
    let inspection =
        product.inspect_corpus(InspectCorpusRequest::from_file(&descriptor, &bundle))?;
    assert!(inspection.assessment().is_none());
    assert_eq!(inspection.cases().len(), 3);
    let ids = inspection
        .cases()
        .iter()
        .map(|c| c.case_id().to_owned())
        .collect::<Vec<_>>();
    let capabilities = product.capabilities_with_corpus(
        InspectCorpusRequest::from_json_bytes(bytes.clone(), &bundle)
            .with_selection(profile("smoke"))
            .with_seed(1)
            .with_parallelism(2),
    )?;
    fs::write(
        root.join("sdk-capabilities.json"),
        serde_json::to_vec_pretty(&capabilities)?,
    )?;
    let selectors = [
        profile("smoke"),
        CorpusSelector::CaseIds {
            profile: "smoke".into(),
            include_stress: false,
            case_ids: ids,
        },
        profile("smoke"),
    ];
    for (index, selector) in selectors.into_iter().enumerate() {
        let destination = root.join(["sdk-profile", "sdk-ids", "sdk-repeat"][index]);
        let request = if index == 0 {
            GenerateCorpusRequest::from_file(&descriptor, &bundle, &destination, selector)
        } else {
            GenerateCorpusRequest::from_json_bytes(bytes.clone(), &bundle, &destination, selector)
        }
        .with_seed(1)
        .with_parallelism(2);
        let GenerateCorpusOutcome::Published(run) = product.generate_corpus(request)? else {
            panic!("mandatory smoke unavailable")
        };
        assert_eq!(run.manifest().kind(), ManifestKind::ExternalCorpus);
        assert_eq!(run.emitted_file_count(), 3);
        assert_eq!(run.output_bytes(), 2790);
        assert!(
            product
                .validate(ValidateRequest::new(&destination))?
                .is_valid()
        );
        let report = product.report(ReportRequest::new(&destination))?;
        assert_eq!(report.kind(), ReportKind::ExternalCorpus);
        fs::write(destination.join("sdk-report.json"), report.json_bytes())?;
    }
    let dry_root = root.join("sdk-dry");
    let GenerateCorpusOutcome::Planned(preview) = product.generate_corpus(
        GenerateCorpusRequest::from_json_bytes(bytes.clone(), &bundle, &dry_root, profile("smoke"))
            .with_seed(1)
            .with_parallelism(2)
            .dry_run(true),
    )?
    else {
        panic!("expected preview")
    };
    assert!(!dry_root.exists());
    assert_eq!(preview.publication_state(), CorpusPublicationState::NotRun);
    assert_eq!(preview.validation_state(), CorpusValidationState::NotRun);
    assert!(
        preview
            .cases()
            .iter()
            .all(|c| c.disposition() == CorpusCaseDisposition::Ready)
    );
    let planned_bundle = root.join("caller-planned");
    let empty_root = root.join("sdk-noexec");
    let GenerateCorpusOutcome::NoExecutableCases(empty) =
        product.generate_corpus(GenerateCorpusRequest::from_file(
            planned_bundle.join("corpus-definition.json"),
            &planned_bundle,
            &empty_root,
            profile("extended"),
        ))?
    else {
        panic!("expected explicit no-execution")
    };
    assert!(!empty_root.exists());
    assert!(empty.artifact_ids().is_empty());
    assert_eq!(empty.cases().len(), 1);
    assert_eq!(empty.publication_state(), CorpusPublicationState::NotRun);
    assert_eq!(empty.validation_state(), CorpusValidationState::NotRun);
    assert_eq!(
        empty.cases()[0].disposition(),
        CorpusCaseDisposition::Planned
    );
    assert!(empty.cases()[0].reason_code().is_some());
    let invalid = product
        .inspect_corpus(InspectCorpusRequest::from_json_bytes(
            b"{".to_vec(),
            &bundle,
        ))
        .unwrap_err();
    assert_eq!(invalid.code(), "request.json.invalid");
    let cancelled = CancellationToken::new();
    cancelled.cancel();
    let error = product
        .generate_corpus_cancellable(
            GenerateCorpusRequest::from_file(
                &descriptor,
                &bundle,
                root.join("sdk-cancelled"),
                profile("smoke"),
            ),
            &cancelled,
        )
        .unwrap_err();
    assert_eq!(error.code(), "generation.execution.cancelled");
    assert!(!root.join("sdk-cancelled").exists());
    let evidence = json!({
        "proof_evidence_version":"1.0.0",
        "metadata_case_count":inspection.cases().len(),
        "metadata_identity":inspection.corpus_definition_identity(),
        "dry_run":{"seed":preview.seed(),"plan_sha256":preview.corpus_plan_sha256(),"identity":preview.identity_projection(),"ledger":preview.cases().iter().map(|c|c.evidence()).collect::<Vec<&Value>>(),"publication":"not_run","validation":"not_run"},
        "no_execution":{"identity":empty.identity_projection(),"ledger":empty.cases().iter().map(|c|c.evidence()).collect::<Vec<&Value>>(),"publication":"not_run","validation":"not_run"},
        "errors":[invalid.code(),error.code()],
        "runtime_source_roots_present":false
    });
    fs::write(
        root.join("sdk-evidence.json"),
        serde_json::to_vec_pretty(&evidence)?,
    )?;
    Ok(())
}
