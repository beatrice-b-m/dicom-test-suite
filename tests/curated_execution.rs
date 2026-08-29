use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use dicom_object::open_file;
use dicom_test_suite::curated_execution::CuratedExecutionServiceFactory;
use dicom_test_suite::curated_plan::{
    CuratedCatalogPaths, CuratedScCorpusPlanProvider, CuratedScPlanRequest, CuratedScSelection,
};
use dicom_test_suite::executor::adapters::ManifestProjectionCompatibilityInput;
use dicom_test_suite::executor::cancellation::CancellationToken;
use dicom_test_suite::executor::engine::{
    CorpusExecutor, ManifestProjectionError, ManifestProjector,
};
use dicom_test_suite::executor::evidence::{ExecutionStatus, PublicationState, ResultStatus};
use dicom_test_suite::executor::transaction::OutputTransaction;

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

struct TempOutput(PathBuf);

impl TempOutput {
    fn absent() -> Self {
        let safe_temp = std::env::temp_dir().canonicalize().unwrap();
        Self(safe_temp.join(format!(
            "dicom-test-suite-curated-execution-{}-{}",
            std::process::id(),
            TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
        )))
    }
}

impl Drop for TempOutput {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct EvidenceProjector;

impl ManifestProjector for EvidenceProjector {
    fn project(
        &self,
        input: &ManifestProjectionCompatibilityInput,
    ) -> Result<Vec<u8>, ManifestProjectionError> {
        let mut bytes = serde_json::to_vec_pretty(&serde_json::json!({
            "corpus_plan_sha256": input.corpus_plan_sha256,
            "artifacts": input
                .artifacts
                .iter()
                .map(|artifact| serde_json::json!({
                    "logical_id": artifact.execution.logical_id,
                    "path": artifact.execution.output.as_ref().map(|output| &output.relative_path),
                    "status": artifact.execution.status,
                }))
                .collect::<Vec<_>>(),
        }))
        .map_err(|error| ManifestProjectionError(error.to_string()))?;
        bytes.push(b'\n');
        Ok(bytes)
    }
}

#[test]
fn corpus_executor_runs_a_production_curated_sc_plan_end_to_end() {
    let provider =
        CuratedScCorpusPlanProvider::load(CuratedCatalogPaths::from_repository_root(".")).unwrap();
    let bundle = provider
        .plan(&CuratedScPlanRequest {
            selection: CuratedScSelection::CaseIds(vec![
                "classic/sc/mono2_u8_explicit_le".into(),
                "classic/sc/mono2_u8_rle_lossless".into(),
                "metadata/sc/timezone_boundaries".into(),
            ]),
            seed: 7,
            max_parallelism: 2,
        })
        .unwrap();
    let destination = TempOutput::absent();
    let result = CorpusExecutor::new(
        CuratedExecutionServiceFactory::new(&bundle),
        EvidenceProjector,
    )
    .execute(&bundle.plan, &destination.0, 2, &CancellationToken::new())
    .unwrap();

    assert_eq!(result.destination, destination.0);
    assert_eq!(
        result.evidence.publication.state,
        PublicationState::Promoted
    );
    assert_eq!(result.evidence.artifacts.len(), bundle.plan.artifacts.len());
    assert_eq!(result.evidence.resources.used_parallelism, 2);
    assert!(destination.0.join("manifest.json").is_file());
    for artifact in &result.evidence.artifacts {
        assert_eq!(artifact.status, ExecutionStatus::Succeeded);
        assert!(artifact.resources.actual_output_bytes > 0);
        assert!(artifact.resources.actual_peak_working_bytes.is_some());
        assert!(
            artifact
                .validation
                .iter()
                .all(|validation| validation.status == ResultStatus::Passed)
        );
        assert!(
            artifact
                .obligations
                .iter()
                .all(|obligation| obligation.status == ResultStatus::Passed)
        );
        let output = artifact.output.as_ref().unwrap();
        let path = destination.0.join(&output.relative_path);
        assert_eq!(fs::metadata(&path).unwrap().len(), output.size_bytes);
        open_file(path).unwrap();
    }
    let rle = result
        .evidence
        .artifacts
        .iter()
        .find(|artifact| artifact.logical_id.contains("mono2_u8_rle_lossless"))
        .unwrap();
    assert_eq!(rle.codecs.len(), 1);
    assert_eq!(
        rle.codecs[0].backend_id,
        dicom_test_suite::codecs::NativeRleLosslessEncoder::BACKEND_ID
    );
}

#[test]
fn shared_executor_can_finish_inside_a_caller_owned_transaction() {
    let provider =
        CuratedScCorpusPlanProvider::load(CuratedCatalogPaths::from_repository_root(".")).unwrap();
    let bundle = provider
        .plan(&CuratedScPlanRequest {
            selection: CuratedScSelection::CaseIds(vec!["classic/sc/mono2_u8_explicit_le".into()]),
            seed: 7,
            max_parallelism: 1,
        })
        .unwrap();
    let destination = TempOutput::absent();
    let transaction = OutputTransaction::begin(&destination.0).unwrap();
    let staging_root = transaction.staging_root().to_owned();
    let result = CorpusExecutor::new(
        CuratedExecutionServiceFactory::new(&bundle),
        EvidenceProjector,
    )
    .execute_into_staging(&bundle.plan, &staging_root, 1, &CancellationToken::new())
    .unwrap();

    assert_eq!(result.evidence.publication.state, PublicationState::Staging);
    assert_eq!(result.projection.artifacts.len(), 1);
    assert!(
        staging_root
            .join("classic/sc/mono2_u8_explicit_le/instance.dcm")
            .is_file()
    );
    assert!(!staging_root.join("manifest.json").exists());
    assert!(!destination.0.exists());
    transaction.cleanup().unwrap();
}
