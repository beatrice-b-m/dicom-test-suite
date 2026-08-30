use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use dicom_test_suite::corpus_plan::{
    ArtifactProvenance, PlannedArtifact, QualificationPayloadPolicy,
};
use dicom_test_suite::curated_execution::CuratedExecutionServiceFactory;
use dicom_test_suite::curated_manifest::project_curated_qualifications;
use dicom_test_suite::curated_plan::{
    CuratedCatalogPaths, CuratedScCorpusPlan, CuratedScCorpusPlanProvider, CuratedScPlanRequest,
    CuratedScSelection,
};
use dicom_test_suite::executor::adapters::ManifestProjectionCompatibilityInput;
use dicom_test_suite::executor::cancellation::CancellationToken;
use dicom_test_suite::executor::engine::{
    CorpusExecutor, ManifestProjectionError, ManifestProjector, StagedCorpusExecution,
};
use dicom_test_suite::executor::services::SlotExecutionBinding;
use dicom_test_suite::sha256_hex;

const FUZZ_CASE: &str = "fuzz/parser/bounded_seed_corpus";
const EOT_CASE: &str = "qualification/encapsulation/eot_u64_overflow";
const NATIVE_SOURCE: &str = "classic/sc/mono2_u8_explicit_le";
const SOURCE_SHA256: [&str; 2] = [
    "eeb0427bacb12d5f8a608757b593d9710c0dcb38669167a740bc162053b3d364",
    "9cef18fdbe59b90f0e79ce87b454f7f8660fae202f18edb5bb17edd95f393126",
];
const SOURCE_SIZE_BYTES: [u64; 2] = [926, 1032];
const QUALIFICATION_SHA256: &str =
    "7d4897cdbefa79a1ab96c64c8c6505d7b95945c4b269b7de933f4a2b93f76df7";

static NEXT: AtomicU64 = AtomicU64::new(0);

struct TempRoot(PathBuf);

impl TempRoot {
    fn new(label: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "dts-curated-robustness-{label}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct NoManifest;

impl ManifestProjector for NoManifest {
    fn project(
        &self,
        _: &ManifestProjectionCompatibilityInput,
    ) -> Result<Vec<u8>, ManifestProjectionError> {
        panic!("qualification staging test must not project a manifest")
    }
}

fn provider() -> CuratedScCorpusPlanProvider {
    CuratedScCorpusPlanProvider::load(CuratedCatalogPaths::from_repository_root(".")).unwrap()
}

fn plan(selection: CuratedScSelection, seed: u64, parallelism: u32) -> CuratedScCorpusPlan {
    provider()
        .plan(&CuratedScPlanRequest {
            selection,
            seed,
            max_parallelism: parallelism,
        })
        .unwrap()
}

fn fuzz(seed: u64, parallelism: u32) -> CuratedScCorpusPlan {
    plan(
        CuratedScSelection::Profile {
            profile: "fuzz".into(),
            include_stress: false,
        },
        seed,
        parallelism,
    )
}

fn qualification(
    bundle: &CuratedScCorpusPlan,
) -> &dicom_test_suite::corpus_plan::PlannedQualification {
    bundle
        .plan
        .artifacts
        .iter()
        .find_map(|artifact| match artifact {
            PlannedArtifact::Qualification(value) => Some(value),
            _ => None,
        })
        .unwrap()
}

fn execute(bundle: &CuratedScCorpusPlan, workers: u32, label: &str) -> StagedCorpusExecution {
    let root = TempRoot::new(label);
    let staged = CorpusExecutor::new(CuratedExecutionServiceFactory::new(bundle), NoManifest)
        .execute_into_staging(&bundle.plan, &root.0, workers, &CancellationToken::new())
        .unwrap();
    let paths = files(&root.0);
    assert!(
        paths.is_empty(),
        "consumed private sources and generated candidates must be removed"
    );
    staged
}

fn files(root: &Path) -> Vec<String> {
    fn walk(root: &Path, path: &Path, files: &mut Vec<String>) {
        for entry in fs::read_dir(path).unwrap() {
            let entry = entry.unwrap();
            if entry.file_type().unwrap().is_dir() {
                walk(root, &entry.path(), files);
            } else {
                files.push(
                    entry
                        .path()
                        .strip_prefix(root)
                        .unwrap()
                        .to_string_lossy()
                        .replace('\\', "/"),
                );
            }
        }
    }
    let mut result = Vec::new();
    walk(root, root, &mut result);
    result.sort();
    result
}

#[test]
fn fuzz_profile_is_one_closed_private_source_qualification_dag() {
    let bundle = fuzz(1, 4);
    bundle.plan.validate().unwrap();
    assert_eq!(bundle.plan.artifacts.len(), 3);
    assert_eq!(bundle.projection.artifacts.len(), 2);
    assert_eq!(bundle.plan.dependencies.len(), 2);

    let PlannedArtifact::Qualification(qualification) = &bundle.plan.artifacts[2] else {
        panic!("qualification must follow both source artifacts")
    };
    assert_eq!(
        qualification.case_binding.as_ref().unwrap().case_id,
        FUZZ_CASE
    );
    assert_eq!(qualification.profile.as_deref(), Some("fuzz"));
    assert_eq!(qualification.run_seed, Some(1));
    assert_eq!(
        qualification.payload_policy,
        QualificationPayloadPolicy::NoPayload
    );
    assert_eq!(qualification.resources.output_bytes, 0);
    assert_eq!(qualification.sources.len(), 2);
    assert_eq!(
        qualification
            .sources
            .iter()
            .map(|source| source.expected_sha256.as_str())
            .collect::<Vec<_>>(),
        SOURCE_SHA256
    );
    assert_eq!(
        qualification
            .sources
            .iter()
            .map(|source| source.expected_size_bytes)
            .collect::<Vec<_>>(),
        SOURCE_SIZE_BYTES
    );
    for (index, artifact) in bundle.plan.artifacts[..2].iter().enumerate() {
        let PlannedArtifact::Dicom(source) = artifact else {
            panic!("fuzz source must be DICOM")
        };
        assert!(!source.output.publish);
        assert!(matches!(
            &source.provenance,
            ArtifactProvenance::PrivateSource { consumed_by }
                if consumed_by == &vec![qualification.logical_id.clone()]
        ));
        assert_eq!(source.logical_id, qualification.sources[index].artifact_id);
        assert!(matches!(
            bundle.bindings[&source.logical_id].slots.values().next(),
            Some(SlotExecutionBinding::NativeFrames { .. })
                | Some(SlotExecutionBinding::CodecRequest { .. })
        ));
    }
    assert!(qualification.sources.iter().enumerate().all(|(index, _)| {
        matches!(
            bundle.bindings[&qualification.logical_id]
                .slots
                .get(&format!("source_{index:02}")),
            Some(SlotExecutionBinding::StagedAsset { .. })
        )
    }));
}

#[test]
fn source_generation_identity_is_seed_seven_and_independent_of_run_seed() {
    let first = fuzz(1, 2);
    let second = fuzz(999, 2);
    assert_eq!(
        qualification(&first).sources,
        qualification(&second).sources
    );
    assert_eq!(qualification(&first).run_seed, Some(1));
    assert_eq!(qualification(&second).run_seed, Some(999));
    assert_eq!(
        qualification(&first).parameters["source_generation_seed"],
        7
    );
    assert_eq!(
        first.plan.canonical_sha256().unwrap(),
        fuzz(1, 2).plan.canonical_sha256().unwrap()
    );
}

#[test]
fn explicitly_requested_valid_source_remains_public_beside_private_fuzz_clone() {
    let bundle = plan(
        CuratedScSelection::CaseIds(vec![NATIVE_SOURCE.into(), FUZZ_CASE.into()]),
        1,
        2,
    );
    let sources = bundle
        .plan
        .artifacts
        .iter()
        .filter_map(|artifact| match artifact {
            PlannedArtifact::Dicom(source)
                if source
                    .case_binding
                    .as_ref()
                    .is_some_and(|binding| binding.case_id == NATIVE_SOURCE) =>
            {
                Some(source)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(sources.len(), 2);
    assert!(sources.iter().any(|source| source.output.publish));
    assert!(sources.iter().any(|source| {
        !source.output.publish
            && source
                .logical_id
                .contains("fuzz_parser_bounded_seed_corpus")
            && matches!(source.provenance, ArtifactProvenance::PrivateSource { .. })
    }));
}

#[test]
fn eot_is_case_id_reachable_but_all_feature_free_excludes_robustness() {
    let eot = plan(CuratedScSelection::CaseIds(vec![EOT_CASE.into()]), 1, 1);
    assert!(eot.projection.artifacts.is_empty());
    assert_eq!(eot.plan.artifacts.len(), 1);
    let PlannedArtifact::Qualification(qualification) = &eot.plan.artifacts[0] else {
        panic!("EOT must be a qualification artifact")
    };
    assert_eq!(
        qualification.case_binding.as_ref().unwrap().case_id,
        EOT_CASE
    );
    assert!(qualification.sources.is_empty());
    assert!(qualification.profile.is_none());
    assert!(qualification.run_seed.is_none());
    assert_eq!(
        qualification.payload_policy,
        QualificationPayloadPolicy::EvidenceOnly
    );

    let all = plan(CuratedScSelection::AllFeatureFree, 1, 1);
    assert!(
        all.plan
            .artifacts
            .iter()
            .all(|artifact| !matches!(artifact, PlannedArtifact::Qualification(_)))
    );
}

#[test]
fn one_fuzz_plan_executes_serially_and_in_parallel_with_exact_frozen_evidence() {
    let bundle = fuzz(1, 4);
    let serial = execute(&bundle, 1, "serial");
    let parallel = execute(&bundle, 4, "parallel");
    let serial_qualification = project_curated_qualifications(&serial.projection).unwrap();
    let parallel_qualification = project_curated_qualifications(&parallel.projection).unwrap();
    assert_eq!(serial_qualification, parallel_qualification);
    assert_eq!(serial_qualification.len(), 1);
    assert_eq!(
        sha256_hex(&serde_json::to_vec(&serial_qualification[0]).unwrap()),
        QUALIFICATION_SHA256
    );
    for staged in [&serial, &parallel] {
        let qualification = staged
            .projection
            .artifacts
            .iter()
            .find(|artifact| matches!(artifact.planned, PlannedArtifact::Qualification(_)))
            .unwrap();
        assert!(qualification.execution.output.is_none());
        assert_eq!(qualification.execution.resources.actual_output_bytes, 0);
        assert_eq!(
            qualification
                .execution
                .materialization
                .as_ref()
                .unwrap()
                .service_evidence
                .len(),
            1
        );
    }
}

#[test]
fn eot_executes_as_internal_evidence_without_public_record_or_payload() {
    let bundle = plan(CuratedScSelection::CaseIds(vec![EOT_CASE.into()]), 1, 1);
    let root = TempRoot::new("eot-execution");
    let staged = CorpusExecutor::new(CuratedExecutionServiceFactory::new(&bundle), NoManifest)
        .execute_into_staging(&bundle.plan, &root.0, 1, &CancellationToken::new())
        .unwrap();
    assert!(files(&root.0).is_empty());
    assert!(
        project_curated_qualifications(&staged.projection)
            .unwrap()
            .is_empty()
    );
    let execution = &staged.projection.artifacts[0].execution;
    assert!(execution.output.is_none());
    assert_eq!(execution.resources.actual_output_bytes, 0);
    assert_eq!(
        execution.materialization.as_ref().unwrap().service_evidence[0].evidence_kind,
        "checked_eot_u64_overflow"
    );
}

#[test]
fn source_identity_failure_cleans_transactional_private_staging() {
    let mut bundle = fuzz(1, 2);
    let PlannedArtifact::Qualification(qualification) = &mut bundle.plan.artifacts[2] else {
        unreachable!()
    };
    qualification.sources[0].expected_sha256 = "0".repeat(64);
    bundle.plan.validate().unwrap();

    let root = TempRoot::new("identity-failure");
    let destination = root.0.join("failed-output");
    let result = CorpusExecutor::new(CuratedExecutionServiceFactory::new(&bundle), NoManifest)
        .execute(&bundle.plan, &destination, 2, &CancellationToken::new());
    assert!(result.is_err());
    assert!(!destination.exists());
    assert!(files(&root.0).is_empty());
}

#[test]
fn pre_cancelled_qualification_execution_leaves_no_private_payload() {
    let bundle = fuzz(1, 2);
    let root = TempRoot::new("cancelled");
    let cancellation = CancellationToken::new();
    cancellation.cancel();
    let result = CorpusExecutor::new(CuratedExecutionServiceFactory::new(&bundle), NoManifest)
        .execute_into_staging(&bundle.plan, &root.0, 2, &cancellation);
    assert!(result.is_err());
    assert!(files(&root.0).is_empty());
}
