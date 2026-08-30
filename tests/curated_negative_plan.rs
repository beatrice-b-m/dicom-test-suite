use std::collections::BTreeSet;
use std::path::PathBuf;

use dicom_test_suite::corpus_plan::{ArtifactProvenance, PlannedArtifact};
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
use dicom_test_suite::negative_plan::NEGATIVE_PARSER_RULE_ID;

fn provider() -> CuratedScCorpusPlanProvider {
    CuratedScCorpusPlanProvider::load(CuratedCatalogPaths::from_repository_root(".")).unwrap()
}

fn negative_inventory() -> BTreeSet<String> {
    let registry: serde_json::Value =
        serde_json::from_slice(&std::fs::read("cases/registry.json").unwrap()).unwrap();
    registry["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|case| {
            case["status"] == "implemented"
                && case["profiles"]
                    .as_array()
                    .is_some_and(|profiles| profiles.iter().any(|profile| profile == "negative"))
        })
        .map(|case| case["case_id"].as_str().unwrap().to_owned())
        .collect()
}

fn plan(parallelism: u32) -> dicom_test_suite::curated_plan::CuratedScCorpusPlan {
    provider()
        .plan(&CuratedScPlanRequest {
            selection: CuratedScSelection::Profile {
                profile: "negative".into(),
                include_stress: false,
            },
            seed: 1,
            max_parallelism: parallelism,
        })
        .unwrap()
}

#[test]
fn negative_profile_is_a_closed_private_source_dag_without_planning_output() {
    let sentinel = PathBuf::from("target/curated-negative-planning-must-not-exist");
    assert!(!sentinel.exists());
    let bundle = plan(1);
    assert!(!sentinel.exists());

    let expected = negative_inventory();
    assert!(!expected.is_empty());
    let actual = bundle
        .plan
        .artifacts
        .iter()
        .filter_map(|artifact| match artifact {
            PlannedArtifact::Mutation(artifact) => Some(artifact.case_binding.case_id.clone()),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(actual, expected);
    bundle.plan.validate().unwrap();

    for artifact in &bundle.plan.artifacts {
        match artifact {
            PlannedArtifact::Dicom(source) => {
                assert!(!source.output.publish, "{}", source.logical_id);
                assert!(matches!(
                    source.provenance,
                    ArtifactProvenance::PrivateSource { .. }
                ));
            }
            PlannedArtifact::Mutation(mutation) => {
                assert!(mutation.output.publish);
                assert_eq!(mutation.output.role, "expected_invalid");
                assert_eq!(mutation.validation.rules.len(), 1);
                assert_eq!(
                    mutation.validation.rules[0].rule_id,
                    NEGATIVE_PARSER_RULE_ID
                );
                assert_eq!(
                    mutation.validation.rules[0].parameters["ordinary_valid_dicom_validation"],
                    false
                );
                let source = bundle
                    .plan
                    .artifacts
                    .iter()
                    .find(|candidate| candidate.logical_id() == mutation.source_artifact_id)
                    .unwrap();
                assert!(matches!(source, PlannedArtifact::Dicom(_)));
            }
            other => panic!("negative profile planned unexpected artifact {other:?}"),
        }
    }
}

#[test]
fn negative_plan_hash_and_topology_are_deterministic_for_one_resource_policy() {
    let first = plan(8);
    let second = plan(8);
    assert_eq!(first.plan.artifacts, second.plan.artifacts);
    assert_eq!(first.plan.dependencies, second.plan.dependencies);
    assert_eq!(
        first.plan.canonical_sha256().unwrap(),
        second.plan.canonical_sha256().unwrap()
    );
    assert_eq!(
        first.plan.topological_order().unwrap(),
        second.plan.topological_order().unwrap()
    );
}

#[test]
fn explicitly_requested_valid_source_stays_public_and_mutation_uses_private_clone() {
    let bundle = provider()
        .plan(&CuratedScPlanRequest {
            selection: CuratedScSelection::CaseIds(vec![
                "classic/sc/mono2_u8_explicit_le".into(),
                "negative/encoding/illegal_vr_bytes".into(),
            ]),
            seed: 1,
            max_parallelism: 2,
        })
        .unwrap();
    bundle.plan.validate().unwrap();
    let mutation = bundle
        .plan
        .artifacts
        .iter()
        .find_map(|artifact| match artifact {
            PlannedArtifact::Mutation(value) => Some(value),
            _ => None,
        })
        .unwrap();
    let sources = bundle
        .plan
        .artifacts
        .iter()
        .filter_map(|artifact| match artifact {
            PlannedArtifact::Dicom(value)
                if value.case_binding.as_ref().is_some_and(|binding| {
                    binding.case_id == "classic/sc/mono2_u8_explicit_le"
                }) =>
            {
                Some(value)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(sources.len(), 2);
    assert!(sources.iter().any(|source| source.output.publish));
    let private = sources
        .iter()
        .find(|source| !source.output.publish)
        .unwrap();
    assert_eq!(mutation.source_artifact_id, private.logical_id);
    assert!(matches!(
        private.provenance,
        ArtifactProvenance::PrivateSource { .. }
    ));
}

#[test]
fn all_feature_free_intentionally_excludes_robustness_profiles() {
    let bundle = provider()
        .plan(&CuratedScPlanRequest {
            selection: CuratedScSelection::AllFeatureFree,
            seed: 1,
            max_parallelism: 1,
        })
        .unwrap();
    assert!(
        bundle
            .plan
            .artifacts
            .iter()
            .all(|artifact| { !matches!(artifact, PlannedArtifact::Mutation(_)) })
    );
}

#[test]
fn negative_planning_preview_has_no_concrete_executor_or_filesystem_boundary() {
    let source = std::fs::read_to_string("src/negative_plan.rs").unwrap();
    assert!(!source.contains("executor::frame_codec"));
    assert!(!source.contains("executor::materialization"));
    assert!(!source.contains("open_file"));
    assert!(!source.contains("std::fs"));
}

struct NoManifest;

impl ManifestProjector for NoManifest {
    fn project(
        &self,
        _: &ManifestProjectionCompatibilityInput,
    ) -> Result<Vec<u8>, ManifestProjectionError> {
        Err(ManifestProjectionError("staging-only test".into()))
    }
}

#[test]
fn all_negative_cases_execute_without_valid_dicom_validation_and_project_exactly() {
    let bundle = plan(4);
    let root = std::env::temp_dir().join(format!(
        "dts-curated-negative-execution-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir(&root).unwrap();
    let staged = CorpusExecutor::new(CuratedExecutionServiceFactory::new(&bundle), NoManifest)
        .execute_into_staging(&bundle.plan, &root, 4, &CancellationToken::new())
        .unwrap();
    let actual = project_curated_file_entries(&bundle.projection, &staged.projection).unwrap();
    let mut baseline: serde_json::Value = serde_json::from_slice(
        &std::fs::read("/tmp/dts-unified-baseline-20260829-52e1d20/negative/manifest.json")
            .unwrap(),
    )
    .unwrap();
    for entry in baseline["files"].as_array_mut().unwrap() {
        // Expected-invalid entries must not carry the valid-corpus reference
        // field. The frozen migration oracle predates that schema correction.
        entry.as_object_mut().unwrap().remove("references");
    }
    assert_eq!(&actual, baseline["files"].as_array().unwrap());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn one_canonical_plan_projects_identically_with_serial_and_parallel_workers() {
    let bundle = provider()
        .plan(&CuratedScPlanRequest {
            selection: CuratedScSelection::CaseIds(vec![
                "negative/encoding/illegal_vr_bytes".into(),
            ]),
            seed: 1,
            max_parallelism: 4,
        })
        .unwrap();
    let execute = |workers, suffix: &str| {
        let root = std::env::temp_dir().join(format!(
            "dts-curated-negative-{suffix}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir(&root).unwrap();
        let staged = CorpusExecutor::new(CuratedExecutionServiceFactory::new(&bundle), NoManifest)
            .execute_into_staging(&bundle.plan, &root, workers, &CancellationToken::new())
            .unwrap();
        let projected =
            project_curated_file_entries(&bundle.projection, &staged.projection).unwrap();
        std::fs::remove_dir_all(root).unwrap();
        projected
    };
    assert_eq!(execute(1, "serial"), execute(4, "parallel"));
}
