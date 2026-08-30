use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use dicom_test_suite::corpus_plan::{FragmentationPolicy, PlannedArtifact};
use dicom_test_suite::curated_execution::CuratedExecutionServiceFactory;
use dicom_test_suite::curated_plan::{
    CuratedCatalogPaths, CuratedScCorpusPlanProvider, CuratedScPlanRequest, CuratedScSelection,
};
use dicom_test_suite::executor::adapters::ManifestProjectionCompatibilityInput;
use dicom_test_suite::executor::cancellation::CancellationToken;
use dicom_test_suite::executor::engine::{
    CorpusExecutor, ManifestProjectionError, ManifestProjector,
};
use dicom_test_suite::executor::services::SlotExecutionBinding;

const CASES: [&str; 4] = [
    "stress/sc/large_bulk_data",
    "stress/sc/deep_nested_sequences",
    "stress/sc/long_value_metadata",
    "stress/sc/large_encapsulated_multifragment",
];

static NEXT: AtomicU64 = AtomicU64::new(0);

struct TempRoot(PathBuf);

impl TempRoot {
    fn new(label: &str) -> Self {
        let path = std::env::temp_dir().canonicalize().unwrap().join(format!(
            "dts-stress-sc-{label}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        Self(path)
    }
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct Projector;

impl ManifestProjector for Projector {
    fn project(
        &self,
        input: &ManifestProjectionCompatibilityInput,
    ) -> Result<Vec<u8>, ManifestProjectionError> {
        serde_json::to_vec(&serde_json::json!({
            "corpus_plan_sha256": input.corpus_plan_sha256,
        }))
        .map_err(|error| ManifestProjectionError(error.to_string()))
    }
}

fn plan(
    case_ids: Vec<String>,
    parallelism: u32,
) -> dicom_test_suite::curated_plan::CuratedScCorpusPlan {
    CuratedScCorpusPlanProvider::load(CuratedCatalogPaths::from_repository_root("."))
        .unwrap()
        .plan(&CuratedScPlanRequest {
            selection: CuratedScSelection::CaseIds(case_ids),
            seed: 1,
            max_parallelism: parallelism,
        })
        .unwrap()
}

#[test]
fn all_stress_sc_cases_plan_lazily_with_explicit_structure_and_resources() {
    let bundle = plan(CASES.map(str::to_owned).to_vec(), 4);
    bundle.plan.validate().unwrap();
    assert_eq!(bundle.plan.artifacts.len(), CASES.len());
    assert!(bundle.plan.dependencies.is_empty());
    assert!(bundle.native_content_requests.is_empty());
    let cases = bundle
        .plan
        .artifacts
        .iter()
        .map(|artifact| {
            let PlannedArtifact::Dicom(artifact) = artifact else {
                panic!("stress SC emitted a non-DICOM artifact")
            };
            assert!(artifact.resources.output_bytes > 0);
            assert!(artifact.resources.peak_working_bytes > 0);
            assert_eq!(
                artifact.evidence.obligations[0].parameters["qualification_scale"],
                "reduced"
            );
            assert_eq!(
                artifact.evidence.obligations[0].parameters["full_scale_available"],
                false
            );
            assert!(
                artifact
                    .instance
                    .content
                    .iter()
                    .all(|content| content.materialization.is_none())
            );
            artifact.case_binding.as_ref().unwrap().case_id.clone()
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(cases, CASES.map(str::to_owned).into_iter().collect());

    let nested = bundle
        .plan
        .artifacts
        .iter()
        .find(|artifact| artifact.logical_id().contains("deep_nested"))
        .unwrap();
    let PlannedArtifact::Dicom(nested) = nested else {
        unreachable!()
    };
    assert_eq!(nested.instance.content.len(), 2);
    assert!(nested.instance.content.iter().any(|content| matches!(
        &content.placement,
        dicom_test_suite::composition::ContentPlacement::Nested { sequence_path }
            if sequence_path.len() == 32
    )));

    let encapsulated = bundle
        .plan
        .artifacts
        .iter()
        .find(|artifact| artifact.logical_id().contains("large_encapsulated"))
        .unwrap();
    let PlannedArtifact::Dicom(encapsulated) = encapsulated else {
        unreachable!()
    };
    assert_eq!(
        encapsulated.encoding.fragmentation,
        FragmentationPolicy::FixedFragmentsPerFrame {
            fragments_per_frame: 64
        }
    );
    assert!(matches!(
        bundle.bindings[&encapsulated.logical_id].slots["pixels"],
        SlotExecutionBinding::ProviderCodecPipeline { .. }
    ));
}

#[test]
fn stress_sc_order_is_independent_of_case_input_and_parallelism() {
    let forward = plan(CASES.map(str::to_owned).to_vec(), 1);
    let mut reversed_ids = CASES.map(str::to_owned).to_vec();
    reversed_ids.reverse();
    let reversed = plan(reversed_ids, 8);
    let signature = |bundle: &dicom_test_suite::curated_plan::CuratedScCorpusPlan| {
        bundle
            .plan
            .artifacts
            .iter()
            .map(|artifact| {
                let PlannedArtifact::Dicom(artifact) = artifact else {
                    unreachable!()
                };
                (
                    artifact.logical_id.clone(),
                    artifact.order,
                    artifact.output.relative_path.clone(),
                    artifact.instance.canonical_sha256(),
                )
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(signature(&forward), signature(&reversed));
    assert_eq!(forward.bindings, reversed.bindings);
}

#[test]
fn all_stress_sc_cases_execute_through_private_streaming_services() {
    let bundle = plan(CASES.map(str::to_owned).to_vec(), 2);
    let destination = TempRoot::new("output");
    let result = CorpusExecutor::new(CuratedExecutionServiceFactory::new(&bundle), Projector)
        .execute(&bundle.plan, &destination.0, 2, &CancellationToken::new())
        .unwrap();
    assert_eq!(result.evidence.artifacts.len(), CASES.len());
    for artifact in &bundle.plan.artifacts {
        let PlannedArtifact::Dicom(artifact) = artifact else {
            unreachable!()
        };
        let path = destination.0.join(artifact.output.relative_path.as_str());
        assert!(path.is_file(), "missing {}", path.display());
    }
    assert!(destination.0.join("manifest.json").is_file());
}

#[test]
fn cancelled_stress_execution_never_publishes_destination() {
    let bundle = plan(vec![CASES[0].to_owned()], 1);
    let destination = TempRoot::new("cancelled-output");
    let cancellation = CancellationToken::new();
    cancellation.cancel_with_reason("focused cancellation gate");
    let result = CorpusExecutor::new(CuratedExecutionServiceFactory::new(&bundle), Projector)
        .execute(&bundle.plan, &destination.0, 1, &cancellation);
    assert!(result.is_err());
    assert!(
        !destination.0.exists(),
        "cancelled executor published its destination"
    );
}
