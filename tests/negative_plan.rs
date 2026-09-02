use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use synth_dicom_gen::corpus_plan::{
    ArtifactProvenance, CORPUS_PLAN_SCHEMA_VERSION, CaseBinding, CorpusPlan, OutputPlan,
    OutputRelativePath, PlannedArtifact, PublicationPlan, PublicationTransaction, ResourcePlan,
};
use synth_dicom_gen::curated_plan::{
    CuratedCatalogPaths, CuratedScCorpusPlanProvider, CuratedScPlanRequest, CuratedScSelection,
};
use synth_dicom_gen::negative::NEGATIVE_RECIPE_VERSION;
use synth_dicom_gen::negative::NegativeError;
use synth_dicom_gen::negative_plan::{
    NEGATIVE_PARSER_RULE_ID, NegativePlanProvider, NegativePlanProviderError,
    NegativePlanProviderRequest,
};
use synth_dicom_gen::recipes::RecipeCatalog;

static NEXT: AtomicU64 = AtomicU64::new(0);

fn absent_root() -> PathBuf {
    std::env::temp_dir().join(format!(
        "dts-negative-plan-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ))
}

fn source() -> (
    synth_dicom_gen::corpus_plan::PlannedDicomArtifact,
    synth_dicom_gen::executor::services::ArtifactExecutionBindings,
) {
    let bundle = CuratedScCorpusPlanProvider::load(CuratedCatalogPaths::from_repository_root("."))
        .unwrap()
        .plan(&CuratedScPlanRequest {
            selection: CuratedScSelection::CaseIds(vec!["classic/sc/mono2_u8_explicit_le".into()]),
            seed: 1,
            max_parallelism: 1,
        })
        .unwrap();
    let artifact = bundle
        .plan
        .artifacts
        .iter()
        .find_map(|artifact| match artifact {
            PlannedArtifact::Dicom(artifact)
                if artifact.case_binding.as_ref().is_some_and(|binding| {
                    binding.case_id == "classic/sc/mono2_u8_explicit_le"
                }) =>
            {
                Some(artifact.clone())
            }
            _ => None,
        })
        .unwrap();
    let bindings = bundle.bindings[&artifact.logical_id].clone();
    (artifact, bindings)
}

fn request() -> NegativePlanProviderRequest {
    let (source, source_bindings) = source();
    let catalog = RecipeCatalog::load(
        "cases/recipes",
        "cases/registry.json",
        "templates/catalog.json",
    )
    .unwrap();
    let identity = catalog
        .binding_for_case("negative/encoding/illegal_vr_bytes")
        .unwrap();
    let mutation_recipe = catalog.recipes()[identity].mutation.clone().unwrap();
    NegativePlanProviderRequest {
        case_binding: CaseBinding {
            case_id: "negative/encoding/illegal_vr_bytes".into(),
            recipe_id: "negative_encoding_illegal_vr_bytes".into(),
            recipe_version: NEGATIVE_RECIPE_VERSION.into(),
        },
        logical_id: "negative-illegal-vr".into(),
        order: source.order + 1,
        output: OutputPlan {
            relative_path: OutputRelativePath::new(
                "negative/encoding/illegal_vr_bytes/instance.dcm",
            )
            .unwrap(),
            role: "expected_invalid".into(),
            publish: true,
        },
        mutation_recipe,
        source,
        source_logical_role: "instance".into(),
        source_bindings,
        max_source_bytes: 4 * 1024 * 1024,
    }
}

#[test]
fn provider_builds_a_closed_private_source_mutation_dag_without_files() {
    let root = absent_root();
    assert!(!root.exists());
    let output = NegativePlanProvider.plan(request()).unwrap();
    assert!(!root.exists());
    assert_eq!(output.artifacts.len(), 2);
    let source = output
        .artifacts
        .iter()
        .find_map(|artifact| match artifact {
            PlannedArtifact::Dicom(source) => Some(source),
            _ => None,
        })
        .unwrap();
    let mutation = output
        .artifacts
        .iter()
        .find_map(|artifact| match artifact {
            PlannedArtifact::Mutation(mutation) => Some(mutation),
            _ => None,
        })
        .unwrap();
    assert!(!source.output.publish);
    assert!(matches!(
        source.provenance,
        ArtifactProvenance::PrivateSource { .. }
    ));
    assert_eq!(mutation.source_artifact_id, source.logical_id);
    assert_eq!(
        mutation.mutation.source_identity.recipe_version,
        source.case_binding.as_ref().unwrap().recipe_version
    );
    assert_eq!(
        mutation.validation.rules[0].rule_id,
        NEGATIVE_PARSER_RULE_ID
    );
    assert_eq!(
        mutation.validation.rules[0].parameters["ordinary_valid_dicom_validation"],
        false
    );
    assert!(!mutation.mutation.operations.is_empty());
    for (order, operation) in mutation.mutation.operations.iter().enumerate() {
        assert_eq!(operation.order, order as u64);
        assert_eq!(
            operation.source_ranges.len(),
            operation.changed_byte_ranges.len()
        );
        if let Some(next) = mutation.mutation.operations.get(order + 1) {
            assert_eq!(
                operation.expected_output_sha256,
                next.expected_source_sha256
            );
        }
    }
    assert_eq!(
        mutation.mutation.expected_output_sha256,
        output.evidence.output_sha256
    );
    assert!(
        mutation
            .mutation
            .acceptable_outcomes
            .contains(&output.parser_probe.outcome.to_string())
    );

    let total_output = output
        .artifacts
        .iter()
        .map(|artifact| artifact.resource_estimate().output_bytes)
        .sum::<u64>();
    let peak = output
        .artifacts
        .iter()
        .map(|artifact| artifact.resource_estimate().peak_working_bytes)
        .max()
        .unwrap();
    let plan = CorpusPlan {
        schema_version: CORPUS_PLAN_SCHEMA_VERSION.into(),
        seed: 1,
        artifacts: output.artifacts,
        dependencies: output.dependencies,
        unavailable: Vec::new(),
        publication: PublicationPlan {
            manifest_path: OutputRelativePath::new("manifest.json").unwrap(),
            transaction: PublicationTransaction::AtomicNoReplace,
            private_staging: true,
            no_overwrite: true,
        },
        resources: ResourcePlan {
            max_artifacts: 2,
            max_total_output_bytes: total_output,
            max_peak_working_bytes: peak,
            max_parallelism: 1,
        },
    };
    plan.validate().unwrap();
    let _ = fs::remove_dir_all(root);
}

#[test]
fn provider_rejects_unversioned_wrong_source_and_non_public_mutation() {
    let baseline = NegativePlanProvider.plan(request()).unwrap();
    let mut explicit = request();
    explicit.mutation_recipe.edits[0]
        .parameters
        .insert("replacement_ascii".into(), "!!".into());
    let changed = NegativePlanProvider.plan(explicit).unwrap();
    assert_ne!(
        baseline.evidence.output_sha256,
        changed.evidence.output_sha256
    );
    let PlannedArtifact::Mutation(changed_artifact) = &changed.artifacts[1] else {
        panic!("second provider artifact must be the mutation")
    };
    assert_eq!(
        changed_artifact.mutation.operations[0].parameters["replacement"],
        serde_json::json!([33, 33])
    );

    let mut invalid = request();
    invalid.case_binding.recipe_version = "0.0.0".into();
    assert!(matches!(
        NegativePlanProvider.plan(invalid),
        Err(NegativePlanProviderError::RecipeVersion { .. })
    ));

    let mut invalid = request();
    invalid.source.case_binding.as_mut().unwrap().recipe_id = "wrong_source_recipe".into();
    assert!(matches!(
        NegativePlanProvider.plan(invalid),
        Err(NegativePlanProviderError::WrongSourceRecipeIdentity)
    ));

    let mut invalid = request();
    invalid.output.publish = false;
    assert!(matches!(
        NegativePlanProvider.plan(invalid),
        Err(NegativePlanProviderError::InvalidMutationOutput)
    ));

    let mut invalid = request();
    invalid.mutation_recipe.edits[0]
        .parameters
        .insert("tag".into(), "7777,7777".into());
    assert!(matches!(
        NegativePlanProvider.plan(invalid),
        Err(NegativePlanProviderError::Negative(
            NegativeError::InvalidContract { .. }
        ))
    ));
}
