use std::sync::atomic::{AtomicUsize, Ordering};

use dicom_test_suite::corpus_plan::PlannedArtifact;
use dicom_test_suite::curated_execution::CuratedExecutionServiceFactory;
use dicom_test_suite::curated_plan::{
    CuratedCatalogPaths, CuratedScCorpusPlanProvider, CuratedScPlanRequest, CuratedScSelection,
};
use dicom_test_suite::executor::adapters::ManifestProjectionInput;
use dicom_test_suite::executor::cancellation::CancellationToken;
use dicom_test_suite::executor::engine::{
    CorpusExecutor, ManifestProjectionError, ManifestProjector,
};
use dicom_test_suite::executor::services::{SlotExecutionBinding, StagedAssetHandle};
use dicom_test_suite::planning_preview::{
    PlanningPreviewError, PlanningPreviewLimits, preview_planned_dicom,
};

fn bundle(case_id: &str) -> dicom_test_suite::curated_plan::CuratedScCorpusPlan {
    CuratedScCorpusPlanProvider::load(CuratedCatalogPaths::from_repository_root("."))
        .unwrap()
        .plan(&CuratedScPlanRequest {
            selection: CuratedScSelection::CaseIds(vec![case_id.into()]),
            seed: 1,
            max_parallelism: 2,
        })
        .unwrap()
}

fn dicom_and_bindings(
    bundle: &dicom_test_suite::curated_plan::CuratedScCorpusPlan,
) -> (
    &dicom_test_suite::corpus_plan::PlannedDicomArtifact,
    &dicom_test_suite::executor::services::ArtifactExecutionBindings,
) {
    let artifact = bundle
        .plan
        .artifacts
        .iter()
        .find_map(|artifact| match artifact {
            PlannedArtifact::Dicom(value) => Some(value),
            _ => None,
        })
        .unwrap();
    (artifact, &bundle.bindings[&artifact.logical_id])
}

#[test]
fn native_preview_matches_the_existing_exact_part10_bytes() {
    let bundle = bundle("classic/sc/mono2_u8_explicit_le");
    let (artifact, bindings) = dicom_and_bindings(&bundle);
    let actual = preview_planned_dicom(
        artifact,
        bindings,
        PlanningPreviewLimits::default(),
        &|| false,
    )
    .unwrap();
    let expected = execute_actual(&bundle, artifact, "native");
    assert_eq!(actual.bytes, expected);
    assert_eq!(actual.size_bytes, expected.len() as u64);
    assert_eq!(actual.sha256, dicom_test_suite::sha256_hex(&expected));
}

struct NoManifest;
impl ManifestProjector for NoManifest {
    fn project(&self, _: &ManifestProjectionInput) -> Result<Vec<u8>, ManifestProjectionError> {
        Err(ManifestProjectionError("staging only".into()))
    }
}

fn execute_actual(
    bundle: &dicom_test_suite::curated_plan::CuratedScCorpusPlan,
    artifact: &dicom_test_suite::corpus_plan::PlannedDicomArtifact,
    suffix: &str,
) -> Vec<u8> {
    let root = std::env::temp_dir().join(format!(
        "dts-planning-preview-{suffix}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir(&root).unwrap();
    CorpusExecutor::new(CuratedExecutionServiceFactory::new(bundle), NoManifest)
        .execute_into_staging(&bundle.plan, &root, 2, &CancellationToken::new())
        .unwrap();
    let actual = std::fs::read(root.join(artifact.output.relative_path.as_str())).unwrap();
    std::fs::remove_dir_all(root).unwrap();
    actual
}

#[test]
fn built_in_rle_preview_matches_shared_executor_materialization() {
    let bundle = bundle("encapsulation/sc/eot_single_fragment_multiframe");
    let (artifact, bindings) = dicom_and_bindings(&bundle);
    let preview = preview_planned_dicom(
        artifact,
        bindings,
        PlanningPreviewLimits::default(),
        &|| false,
    )
    .unwrap();
    let actual = execute_actual(&bundle, artifact, "rle");
    assert_eq!(preview.bytes, actual);
}

#[test]
fn preview_rejects_execution_only_and_external_codec_inputs() {
    let native_bundle = bundle("classic/sc/mono2_u8_explicit_le");
    let (artifact, bindings) = dicom_and_bindings(&native_bundle);
    let mut staged = bindings.clone();
    staged.slots.insert(
        "pixels".into(),
        SlotExecutionBinding::StagedAsset {
            asset: StagedAssetHandle::new("staged:pixels").unwrap(),
        },
    );
    assert!(matches!(
        preview_planned_dicom(artifact, &staged, PlanningPreviewLimits::default(), &|| {
            false
        }),
        Err(PlanningPreviewError::ExecutionOnlyBinding(_))
    ));

    let rle = bundle("encapsulation/sc/eot_single_fragment_multiframe");
    let (artifact, bindings) = dicom_and_bindings(&rle);
    let mut external = bindings.clone();
    let SlotExecutionBinding::CodecRequest { request } = external.slots.get_mut("pixels").unwrap()
    else {
        panic!("expected codec request")
    };
    request.backend_id = "external.example".into();
    assert!(matches!(
        preview_planned_dicom(
            artifact,
            &external,
            PlanningPreviewLimits::default(),
            &|| false
        ),
        Err(PlanningPreviewError::UnsupportedCodec(_))
    ));
}

#[test]
fn preview_enforces_resources_and_cancellation() {
    let bundle = bundle("classic/sc/mono2_u8_explicit_le");
    let (artifact, bindings) = dicom_and_bindings(&bundle);
    assert!(matches!(
        preview_planned_dicom(
            artifact,
            bindings,
            PlanningPreviewLimits {
                max_output_bytes: 1,
                ..PlanningPreviewLimits::default()
            },
            &|| false
        ),
        Err(PlanningPreviewError::Materialize(_))
    ));
    let calls = AtomicUsize::new(0);
    assert_eq!(
        preview_planned_dicom(
            artifact,
            bindings,
            PlanningPreviewLimits::default(),
            &|| calls.fetch_add(1, Ordering::SeqCst) >= 1,
        ),
        Err(PlanningPreviewError::Cancelled)
    );
}

#[test]
fn preview_module_has_no_filesystem_or_concrete_executor_dependency() {
    let source = std::fs::read_to_string("src/planning_preview.rs").unwrap();
    assert!(!source.contains("std::fs"));
    assert!(!source.contains("executor::frame_codec"));
    assert!(!source.contains("executor::materialization"));
    assert!(!source.contains("std::process"));
}
