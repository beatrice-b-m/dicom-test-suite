use std::collections::BTreeMap;
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
use serde_json::Value;

const CASES: [&str; 5] = [
    "stress/study/high_instance_count_ct",
    "stress/sc/large_bulk_data",
    "stress/sc/deep_nested_sequences",
    "stress/sc/long_value_metadata",
    "stress/sc/large_encapsulated_multifragment",
];
const BASELINE: &str = "/tmp/dts-unified-baseline-20260829-52e1d20/stress/manifest.json";
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
fn typed_stress_projection_matches_frozen_file_values_and_resources() {
    let selected = &CASES;
    let (bundle, execution) = execute(selected, 4);
    let projected = project_curated_file_entries(&bundle.projection, &execution).unwrap();
    let baseline: Value = serde_json::from_slice(&fs::read(BASELINE).unwrap()).unwrap();
    let expected = baseline["files"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|file| selected.contains(&file["case_id"].as_str().unwrap()))
        .map(|file| (file["path"].as_str().unwrap().to_owned(), file.clone()))
        .collect::<BTreeMap<_, _>>();
    assert_eq!(projected.len(), expected.len());
    let plan_sha256 = bundle.plan.canonical_sha256().unwrap();
    for actual in &projected {
        let path = actual["path"].as_str().unwrap();
        assert_eq!(actual["corpus_plan_sha256"], plan_sha256);
        let mut actual_without_provenance = actual.clone();
        actual_without_provenance
            .as_object_mut()
            .unwrap()
            .remove("corpus_plan_sha256");
        if actual_without_provenance != expected[path] {
            let expected_names = expected[path]["validation"]["internal"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|check| check["name"].as_str())
                .collect::<std::collections::BTreeSet<_>>();
            let actual_names = actual["validation"]["internal"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|check| check["name"].as_str())
                .collect::<std::collections::BTreeSet<_>>();
            panic!(
                "stress file mismatch at {path}: {}; added={:?}; missing={:?}",
                first_difference(&expected[path], &actual_without_provenance, "$").unwrap(),
                actual_names.difference(&expected_names).collect::<Vec<_>>(),
                expected_names.difference(&actual_names).collect::<Vec<_>>()
            );
        }
    }

    let qualifications = project_curated_stress_qualifications(&bundle.projection, &execution)
        .expect("typed stress qualifications");
    assert_eq!(qualifications.len(), selected.len());
    let expected_qualifications = baseline["qualifications"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|qualification| selected.contains(&qualification["case_id"].as_str().unwrap()))
        .map(|qualification| {
            (
                qualification["case_id"].as_str().unwrap().to_owned(),
                qualification.clone(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    for qualification in qualifications {
        assert_eq!(qualification["scale"], "reduced");
        assert_eq!(qualification["status"], "passed");
        assert_eq!(qualification["outcome"], "completed");
        assert_eq!(qualification["unavailable_scales"][0]["scale"], "full");
        let case_id = qualification["case_id"].as_str().unwrap();
        let mut expected = expected_qualifications[case_id].clone();
        // Runtime duration is observational rather than byte-stable. Every
        // other public qualification field is frozen compatibility surface.
        expected["observation"]["elapsed_milliseconds"] =
            qualification["observation"]["elapsed_milliseconds"].clone();
        assert_eq!(
            qualification, expected,
            "qualification mismatch for {case_id}"
        );
    }
}

fn first_difference(expected: &Value, actual: &Value, path: &str) -> Option<String> {
    match (expected, actual) {
        (Value::Object(expected), Value::Object(actual)) => {
            for key in expected.keys().chain(actual.keys()) {
                if expected.get(key) != actual.get(key) {
                    return match (expected.get(key), actual.get(key)) {
                        (Some(expected), Some(actual)) => {
                            first_difference(expected, actual, &format!("{path}.{key}"))
                        }
                        _ => Some(format!(
                            "{path}.{key}: expected {:?}, actual {:?}",
                            expected.get(key),
                            actual.get(key)
                        )),
                    };
                }
            }
            None
        }
        (Value::Array(expected), Value::Array(actual)) => {
            if expected.len() != actual.len() {
                return Some(format!(
                    "{path}: expected array length {}, actual {}",
                    expected.len(),
                    actual.len()
                ));
            }
            expected
                .iter()
                .zip(actual)
                .enumerate()
                .find_map(|(index, (expected, actual))| {
                    (expected != actual).then(|| {
                        first_difference(expected, actual, &format!("{path}[{index}]")).unwrap()
                    })
                })
        }
        _ if expected != actual => {
            Some(format!("{path}: expected {expected:?}, actual {actual:?}"))
        }
        _ => None,
    }
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
