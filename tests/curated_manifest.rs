use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use dicom_test_suite::curated_execution::CuratedExecutionServiceFactory;
use dicom_test_suite::curated_manifest::project_curated_file_entries;
use dicom_test_suite::curated_plan::{
    CuratedCatalogPaths, CuratedScCorpusPlanProvider, CuratedScPlanRequest, CuratedScSelection,
};
use dicom_test_suite::executor::adapters::ManifestProjectionCompatibilityInput;
use dicom_test_suite::executor::cancellation::CancellationToken;
use dicom_test_suite::executor::engine::{
    CorpusExecutor, ManifestProjectionError, ManifestProjector,
};
use serde_json::Value;

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Temp(PathBuf);
impl Temp {
    fn new(label: &str) -> Self {
        Self(std::env::temp_dir().join(format!(
            "dts-u36-{label}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        )))
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct UnusedProjector;
impl ManifestProjector for UnusedProjector {
    fn project(
        &self,
        _: &ManifestProjectionCompatibilityInput,
    ) -> Result<Vec<u8>, ManifestProjectionError> {
        panic!("execute_into_staging must not invoke projector")
    }
}

fn generate(profile: &str) -> Temp {
    let root = Temp::new(profile);
    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args([
            "generate",
            "--profile",
            profile,
            "--out",
            root.0.to_str().unwrap(),
            "--seed",
            "7",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    root
}

#[test]
fn production_projection_matches_every_historical_file_value() {
    let generated;
    let (roots, seed) = if let Ok(root) = std::env::var("DTS_CLASSIC_BASELINE_ROOT") {
        (
            vec![
                PathBuf::from(&root).join("all"),
                PathBuf::from(root).join("legacy"),
            ],
            1,
        )
    } else {
        generated = vec![generate("all"), generate("legacy")];
        (generated.iter().map(|root| root.0.clone()).collect(), 7)
    };
    let provider =
        CuratedScCorpusPlanProvider::load(CuratedCatalogPaths::from_repository_root(".")).unwrap();
    let bundle = provider
        .plan(&CuratedScPlanRequest {
            selection: CuratedScSelection::AllFeatureFree,
            seed,
            max_parallelism: 4,
        })
        .unwrap();
    let staging = Temp::new("staging");
    fs::create_dir_all(&staging.0).unwrap();
    let staged = CorpusExecutor::new(
        CuratedExecutionServiceFactory::new(&bundle),
        UnusedProjector,
    )
    .execute_into_staging(&bundle.plan, &staging.0, 4, &CancellationToken::new())
    .unwrap();
    let actual = project_curated_file_entries(&bundle.projection, &staged.projection).unwrap();
    let selected = bundle
        .projection
        .artifacts
        .iter()
        .map(|context| {
            (
                context.registry_case.case_id.as_str(),
                context.artifact_recipe.output.path.as_deref().unwrap(),
            )
        })
        .collect::<std::collections::BTreeSet<_>>();
    let mut expected = Vec::new();
    for root in &roots {
        let manifest: Value =
            serde_json::from_slice(&fs::read(root.join("manifest.json")).unwrap()).unwrap();
        expected.extend(
            manifest["files"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|file| {
                    selected.contains(&(
                        file["case_id"].as_str().unwrap(),
                        file["path"].as_str().unwrap(),
                    ))
                })
                .cloned(),
        );
    }
    assert!(!expected.is_empty());
    assert_eq!(actual.len(), expected.len());
    let expected = expected
        .into_iter()
        .map(|file| {
            (
                (
                    file["case_id"].as_str().unwrap().to_owned(),
                    file["path"].as_str().unwrap().to_owned(),
                ),
                file,
            )
        })
        .collect::<std::collections::BTreeMap<_, _>>();
    for (index, actual) in actual.iter().enumerate() {
        let expected = &expected[&(
            actual["case_id"].as_str().unwrap().to_owned(),
            actual["path"].as_str().unwrap().to_owned(),
        )];
        if actual != expected {
            let (pointer, left, right) = first_difference(actual, expected, "").unwrap();
            panic!(
                "file mismatch index {index} path {} at {pointer}: actual={left:?} expected={right:?}",
                expected["path"]
            );
        }
    }
}

fn first_difference(left: &Value, right: &Value, path: &str) -> Option<(String, Value, Value)> {
    match (left, right) {
        (Value::Object(l), Value::Object(r)) => {
            let keys = l
                .keys()
                .chain(r.keys())
                .collect::<std::collections::BTreeSet<_>>();
            for key in keys {
                match (l.get(key), r.get(key)) {
                    (Some(a), Some(b)) => {
                        if let Some(diff) = first_difference(a, b, &format!("{path}/{key}")) {
                            return Some(diff);
                        }
                    }
                    (Some(a), None) => {
                        return Some((format!("{path}/{key}"), a.clone(), Value::Null));
                    }
                    (None, Some(b)) => {
                        return Some((format!("{path}/{key}"), Value::Null, b.clone()));
                    }
                    _ => {}
                }
            }
            None
        }
        (Value::Array(l), Value::Array(r)) => {
            for (i, (a, b)) in l.iter().zip(r).enumerate() {
                if let Some(diff) = first_difference(a, b, &format!("{path}/{i}")) {
                    return Some(diff);
                }
            }
            if l.len() != r.len() {
                Some((format!("{path}/length"), left.clone(), right.clone()))
            } else {
                None
            }
        }
        _ if left != right => Some((path.to_owned(), left.clone(), right.clone())),
        _ => None,
    }
}

#[test]
fn projection_rejects_crossed_output_and_materialization_hashes() {
    let provider =
        CuratedScCorpusPlanProvider::load(CuratedCatalogPaths::from_repository_root(".")).unwrap();
    let bundle = provider
        .plan(&CuratedScPlanRequest {
            selection: CuratedScSelection::CaseIds(vec!["classic/sc/mono2_u8_explicit_le".into()]),
            seed: 7,
            max_parallelism: 1,
        })
        .unwrap();
    let staging = Temp::new("mismatch");
    fs::create_dir_all(&staging.0).unwrap();
    let mut staged = CorpusExecutor::new(
        CuratedExecutionServiceFactory::new(&bundle),
        UnusedProjector,
    )
    .execute_into_staging(&bundle.plan, &staging.0, 1, &CancellationToken::new())
    .unwrap();
    staged.projection.artifacts[0]
        .execution
        .output
        .as_mut()
        .unwrap()
        .sha256 = "0".repeat(64);
    assert!(project_curated_file_entries(&bundle.projection, &staged.projection).is_err());
}

#[test]
fn production_projector_is_filesystem_and_generator_free() {
    let source = include_str!("../src/curated_manifest.rs");
    for forbidden in [
        "use std::fs",
        "open_file(",
        "crate::generator",
        "src/generator",
        "read_to_string",
        "fs::read",
    ] {
        assert!(
            !source.contains(forbidden),
            "curated manifest projector contains forbidden dependency {forbidden}"
        );
    }
}
