use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use synth_dicom_gen::curated_execution::CuratedExecutionServiceFactory;
use synth_dicom_gen::curated_plan::{
    CuratedCatalogPaths, CuratedScCorpusPlanProvider, CuratedScPlanRequest, CuratedScSelection,
};
use synth_dicom_gen::executor::adapters::ManifestProjectionInput;
use synth_dicom_gen::executor::cancellation::CancellationToken;
use synth_dicom_gen::executor::engine::{
    CorpusExecutor, ManifestProjectionError, ManifestProjector,
};
use synth_dicom_gen::executor::evidence::{ExecutionStatus, ResultStatus};

const CASE_ID: &str = "stress/study/high_instance_count_ct";
static NEXT: AtomicU64 = AtomicU64::new(0);

struct TempRoot(PathBuf);

impl TempRoot {
    fn absent() -> Self {
        Self(std::env::temp_dir().canonicalize().unwrap().join(format!(
            "dts-stress-ct-validation-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        )))
    }
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct Projector;

impl ManifestProjector for Projector {
    fn project(&self, _: &ManifestProjectionInput) -> Result<Vec<u8>, ManifestProjectionError> {
        Ok(b"{}\n".to_vec())
    }
}

#[test]
fn stress_ct_executes_with_classic_ct_and_reduced_scale_validation() {
    let bundle = CuratedScCorpusPlanProvider::load(CuratedCatalogPaths::from_repository_root("."))
        .unwrap()
        .plan(&CuratedScPlanRequest {
            selection: CuratedScSelection::CaseIds(vec![CASE_ID.into()]),
            seed: 1,
            max_parallelism: 8,
        })
        .unwrap();
    let destination = TempRoot::absent();
    let result = CorpusExecutor::new(CuratedExecutionServiceFactory::new(&bundle), Projector)
        .execute(&bundle.plan, &destination.0, 8, &CancellationToken::new())
        .unwrap();

    assert_eq!(result.evidence.artifacts.len(), bundle.plan.artifacts.len());
    for artifact in &result.evidence.artifacts {
        assert_eq!(artifact.status, ExecutionStatus::Succeeded);
        let validation = artifact.validation.first().unwrap();
        assert_eq!(validation.status, ResultStatus::Passed);
        let names = validation.details["checks"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|check| check["name"].as_str())
            .collect::<std::collections::BTreeSet<_>>();
        assert!(names.contains("part10_preamble"));
        assert!(names.iter().any(|name| name.starts_with("ct_")));
        assert!(names.contains("curated_composition_plan"));
        assert!(names.contains("stress_ct_reduced_qualification"));
    }
}
