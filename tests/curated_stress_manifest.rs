use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use dicom_test_suite::curated_execution::CuratedExecutionServiceFactory;
use dicom_test_suite::curated_manifest::{
    project_curated_file_entries, project_curated_stress_qualifications,
};
use dicom_test_suite::curated_plan::{
    CuratedCatalogPaths, CuratedScCorpusPlanProvider, CuratedScPlanRequest, CuratedScSelection,
};
use dicom_test_suite::executor::adapters::ManifestProjectionInput;
use dicom_test_suite::executor::cancellation::CancellationToken;
use dicom_test_suite::executor::engine::{
    CorpusExecutor, ManifestProjectionError, ManifestProjector,
};
use dicom_test_suite::sha256_hex;

const CASES: [&str; 5] = [
    "stress/study/high_instance_count_ct",
    "stress/sc/large_bulk_data",
    "stress/sc/deep_nested_sequences",
    "stress/sc/long_value_metadata",
    "stress/sc/large_encapsulated_multifragment",
];
const FROZEN_FILES_SHA256: &str =
    "9df2977fb44d3f6f7d66ce0d4de90a1cdcf32a7b8f24d340fa201ff90b1f3a67";
const FROZEN_QUALIFICATIONS_SHA256: &str =
    "4089d14a723b2fe329a37849b90a5263d5bc043f5dcba69e512bb7f0dc107c1a";
static NEXT: AtomicU64 = AtomicU64::new(0);

struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let path = std::env::temp_dir().canonicalize().unwrap().join(format!(
            "dts-curated-stress-manifest-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct Unused;
impl ManifestProjector for Unused {
    fn project(&self, _: &ManifestProjectionInput) -> Result<Vec<u8>, ManifestProjectionError> {
        panic!("staging-only execution invoked a manifest projector")
    }
}

fn execute(
    case_ids: &[&str],
    parallelism: u32,
) -> (
    dicom_test_suite::curated_plan::CuratedScCorpusPlan,
    ManifestProjectionInput,
) {
    let provider =
        CuratedScCorpusPlanProvider::load(CuratedCatalogPaths::from_repository_root(".")).unwrap();
    let bundle = provider
        .plan(&CuratedScPlanRequest {
            selection: CuratedScSelection::CaseIds(
                case_ids
                    .iter()
                    .map(|case_id| (*case_id).to_owned())
                    .collect(),
            ),
            seed: 1,
            max_parallelism: parallelism,
        })
        .unwrap();
    let staging = Temp::new();
    let outcome = CorpusExecutor::new(CuratedExecutionServiceFactory::new(&bundle), Unused)
        .execute_into_staging(
            &bundle.plan,
            &staging.0,
            parallelism,
            &CancellationToken::new(),
        )
        .unwrap();
    (bundle, outcome.projection)
}

#[test]
#[ignore = "R2.3 explicit heavy qualification; run through scripts/run-heavy-qualification.sh"]
fn typed_stress_projection_matches_frozen_file_values_and_resources() {
    let selected = &CASES;
    let (bundle, execution) = execute(selected, 4);
    let mut projected = project_curated_file_entries(&bundle.projection, &execution).unwrap();
    let plan_sha256 = bundle.plan.canonical_sha256().unwrap();
    for actual in &mut projected {
        let path = actual["path"].as_str().unwrap();
        assert_eq!(actual["corpus_plan_sha256"], plan_sha256);
        let instance_plan_sha256 = execution
            .artifacts
            .iter()
            .find(|artifact| {
                artifact
                    .planned
                    .output()
                    .is_some_and(|output| output.relative_path.as_str() == path)
            })
            .and_then(|artifact| artifact.execution.instance_plan_sha256.as_deref())
            .expect("published stress DICOM has canonical instance plan evidence");
        assert_eq!(actual["resolved_plan_sha256"], instance_plan_sha256);
        let actual_object = actual.as_object_mut().unwrap();
        actual_object.remove("corpus_plan_sha256");
        actual_object.remove("resolved_plan_sha256");
    }
    assert_eq!(
        sha256_hex(&serde_json::to_vec(&projected).unwrap()),
        FROZEN_FILES_SHA256,
        "frozen stress file projection changed"
    );

    let mut qualifications = project_curated_stress_qualifications(&bundle.projection, &execution)
        .expect("typed stress qualifications");
    assert_eq!(qualifications.len(), selected.len());
    for qualification in &mut qualifications {
        assert_eq!(qualification["scale"], "reduced");
        assert_eq!(qualification["status"], "passed");
        assert_eq!(qualification["outcome"], "completed");
        assert_eq!(qualification["unavailable_scales"][0]["scale"], "full");
        // Runtime duration is observational rather than byte-stable. Every
        // other public qualification field is frozen compatibility surface.
        qualification["observation"]
            .as_object_mut()
            .unwrap()
            .remove("elapsed_milliseconds");
    }
    assert_eq!(
        sha256_hex(&serde_json::to_vec(&qualifications).unwrap()),
        FROZEN_QUALIFICATIONS_SHA256,
        "frozen stress qualification projection changed"
    );
}

#[test]
fn all_feature_free_stays_stress_free() {
    let provider =
        CuratedScCorpusPlanProvider::load(CuratedCatalogPaths::from_repository_root(".")).unwrap();
    let bundle = provider
        .plan(&CuratedScPlanRequest {
            selection: CuratedScSelection::AllFeatureFree,
            seed: 1,
            max_parallelism: 4,
        })
        .unwrap();
    assert!(
        bundle
            .projection
            .artifacts
            .iter()
            .all(|artifact| { !CASES.contains(&artifact.registry_case.case_id.as_str()) })
    );
}

#[test]
fn stress_projection_order_is_parallelism_independent() {
    // Use two inexpensive, structurally different stress plans. The CT case has
    // 128 independent artifacts while long metadata is a single streamed SC
    // artifact, so this exercises scheduler completion-order independence
    // without duplicating the expensive 256-frame qualification run.
    let selected = [CASES[0], CASES[3]];
    let (serial_bundle, serial_execution) = execute(&selected, 1);
    let (parallel_bundle, parallel_execution) = execute(&selected, 8);

    let mut serial =
        project_curated_file_entries(&serial_bundle.projection, &serial_execution).unwrap();
    let mut parallel =
        project_curated_file_entries(&parallel_bundle.projection, &parallel_execution).unwrap();
    let serial_sha256 = serial_bundle.plan.canonical_sha256().unwrap();
    let parallel_sha256 = parallel_bundle.plan.canonical_sha256().unwrap();
    assert_ne!(serial_sha256, parallel_sha256);
    for entry in &mut serial {
        assert_eq!(entry["corpus_plan_sha256"], serial_sha256);
        entry.as_object_mut().unwrap().remove("corpus_plan_sha256");
    }
    for entry in &mut parallel {
        assert_eq!(entry["corpus_plan_sha256"], parallel_sha256);
        entry.as_object_mut().unwrap().remove("corpus_plan_sha256");
    }
    assert_eq!(serial, parallel);

    let serial_paths = serial
        .iter()
        .map(|entry| entry["path"].as_str().unwrap())
        .collect::<Vec<_>>();
    let planned_paths = serial_bundle
        .plan
        .artifacts
        .iter()
        .map(|artifact| artifact.output().unwrap().relative_path.as_str())
        .collect::<Vec<_>>();
    assert_eq!(serial_paths, planned_paths);
}

#[test]
fn migrated_wsi_and_enhanced_ct_emit_typed_stress_qualifications() {
    let selected = ["stress/wsi/large_pyramid", "stress/enhanced-ct/many_frames"];
    let (bundle, execution) = execute(&selected, 4);
    let qualifications =
        project_curated_stress_qualifications(&bundle.projection, &execution).unwrap();
    assert_eq!(qualifications.len(), 2);
    assert_eq!(qualifications[0]["case_id"], selected[0]);
    assert_eq!(qualifications[0]["recipe"], "wsi_pyramid");
    assert_eq!(qualifications[0]["requested"]["instances"], 3);
    assert_eq!(qualifications[0]["requested"]["pyramid_levels"], 3);
    assert_eq!(qualifications[0]["requested"]["tile_rows"], 256);
    assert_eq!(qualifications[1]["case_id"], selected[1]);
    assert_eq!(qualifications[1]["recipe"], "enhanced_ct");
    assert_eq!(qualifications[1]["requested"]["frames"], 256);
    assert!(qualifications.iter().all(|qualification| {
        qualification["status"] == "passed"
            && qualification["scale"] == "reduced"
            && qualification["unavailable_scales"][0]["scale"] == "full"
    }));
}

#[test]
fn stress_projection_source_has_no_filesystem_or_sc_parameter_bridge() {
    let source = include_str!("../src/curated_manifest/stress.rs");
    for forbidden in [
        "SecondaryCaptureParameters",
        "std::fs",
        "open_file(",
        "crate::generator",
        "read_to_string",
    ] {
        assert!(!source.contains(forbidden), "forbidden bridge {forbidden}");
    }
}
