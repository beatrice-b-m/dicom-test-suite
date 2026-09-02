use std::collections::BTreeSet;

use synth_dicom_gen::corpus_plan::{ArtifactProvenance, PlannedArtifact};
use synth_dicom_gen::curated_plan::{
    CuratedCatalogPaths, CuratedScCorpusPlanProvider, CuratedScPlanRequest, CuratedScSelection,
};
use synth_dicom_gen::qualification_plan::{
    PreparedQualificationSource, QualificationPlanError, QualificationPlanRequest,
    plan_qualification,
};
use synth_dicom_gen::recipes::{RecipeCatalog, qualification_parameters};

const FUZZ_CASE: &str = "fuzz/parser/bounded_seed_corpus";
const EOT_CASE: &str = "qualification/encapsulation/eot_u64_overflow";
const SOURCE_CASES: [&str; 2] = [
    "classic/sc/mono2_u8_explicit_le",
    "classic/sc/mono1_u8_rle_lossless",
];

fn recipe(case_id: &str) -> synth_dicom_gen::recipes::CaseRecipe {
    let catalog = RecipeCatalog::load(
        "cases/recipes",
        "cases/registry.json",
        "templates/catalog.json",
    )
    .unwrap();
    catalog.recipes()[catalog.binding_for_case(case_id).unwrap()].clone()
}

fn fuzz_sources(target_id: &str) -> Vec<PreparedQualificationSource> {
    let mut bundle =
        CuratedScCorpusPlanProvider::load(CuratedCatalogPaths::from_repository_root("."))
            .unwrap()
            .plan(&CuratedScPlanRequest {
                selection: CuratedScSelection::CaseIds(SOURCE_CASES.map(str::to_owned).to_vec()),
                seed: 1,
                max_parallelism: 2,
            })
            .unwrap();
    let fuzz = recipe(FUZZ_CASE);
    let parameters = qualification_parameters(&fuzz).unwrap();
    let synth_dicom_gen::recipes::QualificationParameters::BoundedDeterministicFuzz {
        sources, ..
    } = parameters
    else {
        unreachable!()
    };
    sources
        .into_iter()
        .enumerate()
        .map(|(index, expected)| {
            let position = bundle
                .plan
                .artifacts
                .iter()
                .position(|artifact| match artifact {
                    PlannedArtifact::Dicom(value) => {
                        value.case_binding.as_ref().is_some_and(|binding| {
                            binding.recipe_id == expected.recipe.recipe_id
                                && binding.recipe_version == expected.recipe.recipe_version
                        })
                    }
                    _ => false,
                })
                .unwrap();
            let PlannedArtifact::Dicom(mut artifact) = bundle.plan.artifacts.remove(position)
            else {
                unreachable!()
            };
            artifact.provenance = ArtifactProvenance::PrivateSource {
                consumed_by: vec![target_id.into()],
            };
            artifact.output.publish = false;
            let bindings = bundle.bindings.remove(&artifact.logical_id).unwrap();
            PreparedQualificationSource {
                artifact,
                bindings,
                dependency_role: expected.dependency_role,
                recipe_artifact_logical_id: expected.artifact_logical_id,
                preflight_sha256: format!("{:064x}", index + 1),
                preflight_size_bytes: 1024 + index as u64,
            }
        })
        .collect()
}

fn fuzz_request<'a>(
    recipe: &'a synth_dicom_gen::recipes::CaseRecipe,
    sources: Vec<PreparedQualificationSource>,
) -> QualificationPlanRequest<'a> {
    QualificationPlanRequest {
        recipe,
        parameters: qualification_parameters(recipe).unwrap(),
        logical_id: "qualification:fuzz".into(),
        order: 100,
        run_seed: Some(11),
        profile: Some("fuzz".into()),
        sources,
    }
}

#[test]
fn fuzz_plan_is_closed_ordered_private_and_reproducible() {
    let recipe = recipe(FUZZ_CASE);
    let first =
        plan_qualification(fuzz_request(&recipe, fuzz_sources("qualification:fuzz"))).unwrap();
    let second =
        plan_qualification(fuzz_request(&recipe, fuzz_sources("qualification:fuzz"))).unwrap();
    assert_eq!(first, second);
    assert_eq!(first.artifacts.len(), 3);
    assert_eq!(first.dependencies.len(), 2);

    let mut base =
        CuratedScCorpusPlanProvider::load(CuratedCatalogPaths::from_repository_root("."))
            .unwrap()
            .plan(&CuratedScPlanRequest {
                selection: CuratedScSelection::CaseIds(vec![SOURCE_CASES[0].into()]),
                seed: 11,
                max_parallelism: 2,
            })
            .unwrap()
            .plan;
    base.artifacts = first.artifacts;
    base.dependencies = first.dependencies;
    base.unavailable.clear();
    base.resources.max_artifacts = base.artifacts.len() as u64;
    base.resources.max_total_output_bytes = base
        .artifacts
        .iter()
        .map(|artifact| artifact.resource_estimate().output_bytes)
        .sum::<u64>()
        .max(1);
    base.resources.max_peak_working_bytes = base
        .artifacts
        .iter()
        .map(|artifact| artifact.resource_estimate().peak_working_bytes)
        .max()
        .unwrap();
    base.validate().unwrap();
    let order = base.topological_order().unwrap();
    assert_eq!(order.last().unwrap(), "qualification:fuzz");
    assert_eq!(order.iter().collect::<BTreeSet<_>>().len(), 3);
    assert_eq!(
        base.canonical_sha256().unwrap(),
        base.canonical_sha256().unwrap()
    );
}

#[test]
fn fuzz_rejects_reordered_missing_extra_public_and_drifted_sources() {
    let recipe = recipe(FUZZ_CASE);
    let mut reordered = fuzz_sources("qualification:fuzz");
    reordered.swap(0, 1);
    assert!(matches!(
        plan_qualification(fuzz_request(&recipe, reordered)),
        Err(QualificationPlanError::SourceOrder { index: 0, .. })
    ));

    let mut missing = fuzz_sources("qualification:fuzz");
    missing.pop();
    assert!(matches!(
        plan_qualification(fuzz_request(&recipe, missing)),
        Err(QualificationPlanError::SourceCount {
            expected: 2,
            actual: 1
        })
    ));

    let mut extra = fuzz_sources("qualification:fuzz");
    extra.push(extra[0].clone());
    assert!(matches!(
        plan_qualification(fuzz_request(&recipe, extra)),
        Err(QualificationPlanError::SourceCount {
            expected: 2,
            actual: 3
        })
    ));

    let mut public = fuzz_sources("qualification:fuzz");
    public[0].artifact.output.publish = true;
    assert!(matches!(
        plan_qualification(fuzz_request(&recipe, public)),
        Err(QualificationPlanError::SourcePublished(_))
    ));

    let mut drifted = fuzz_sources("qualification:fuzz");
    drifted[0].preflight_sha256 = "A".repeat(64);
    assert!(matches!(
        plan_qualification(fuzz_request(&recipe, drifted)),
        Err(QualificationPlanError::InvalidPreflight(_))
    ));
}

#[test]
fn eot_plan_is_source_free_evidence_only_and_profile_empty() {
    let recipe = recipe(EOT_CASE);
    let output = plan_qualification(QualificationPlanRequest {
        recipe: &recipe,
        parameters: qualification_parameters(&recipe).unwrap(),
        logical_id: "qualification:eot-overflow".into(),
        order: 7,
        run_seed: None,
        profile: None,
        sources: Vec::new(),
    })
    .unwrap();
    assert!(output.dependencies.is_empty());
    assert_eq!(output.artifacts.len(), 1);
    let PlannedArtifact::Qualification(planned) = &output.artifacts[0] else {
        unreachable!()
    };
    assert!(planned.sources.is_empty());
    assert!(planned.profile.is_none());
    assert!(planned.run_seed.is_none());
    assert_eq!(
        planned.payload_policy,
        synth_dicom_gen::corpus_plan::QualificationPayloadPolicy::EvidenceOnly
    );

    let invalid = plan_qualification(QualificationPlanRequest {
        recipe: &recipe,
        parameters: qualification_parameters(&recipe).unwrap(),
        logical_id: "qualification:eot-overflow".into(),
        order: 7,
        run_seed: Some(1),
        profile: None,
        sources: Vec::new(),
    });
    assert!(matches!(
        invalid,
        Err(QualificationPlanError::InvalidRunContext(_))
    ));
}
