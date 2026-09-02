use std::collections::BTreeSet;
use std::fs;

use synth_dicom_gen::corpus_plan::PlannedArtifact;
use synth_dicom_gen::curated_plan::{
    CuratedCatalogPaths, CuratedScCorpusPlan, CuratedScCorpusPlanProvider, CuratedScPlanRequest,
    CuratedScSelection,
};
use synth_dicom_gen::recipes::{RecipeCatalog, plan_stress_ct_recipe};
use synth_dicom_gen::sha256_hex;

const CASE_ID: &str = "stress/study/high_instance_count_ct";

fn provider() -> CuratedScCorpusPlanProvider {
    CuratedScCorpusPlanProvider::load(CuratedCatalogPaths::from_repository_root(".")).unwrap()
}

fn plan(max_parallelism: u32) -> CuratedScCorpusPlan {
    provider()
        .plan(&CuratedScPlanRequest {
            selection: CuratedScSelection::CaseIds(vec![CASE_ID.into()]),
            seed: 1,
            max_parallelism,
        })
        .unwrap()
}

#[test]
fn stress_ct_case_id_resolves_all_independent_slices_with_explicit_resources() {
    let bundle = plan(8);
    bundle.plan.validate().unwrap();
    assert!(bundle.pending.is_empty());
    assert!(bundle.plan.unavailable.is_empty());
    assert!(bundle.plan.dependencies.is_empty());
    assert_eq!(bundle.plan.artifacts.len(), 128);
    assert_eq!(bundle.bindings.len(), 128);
    assert_eq!(bundle.native_content_requests.len(), 128);

    let catalog = RecipeCatalog::load(
        "cases/recipes",
        "cases/registry.json",
        "templates/catalog.json",
    )
    .unwrap();
    let identity = catalog.binding_for_case(CASE_ID).unwrap();
    let recipe = &catalog.recipes()[identity];
    let lock = sha256_hex(&fs::read("standards.lock.json").unwrap());
    let direct = plan_stress_ct_recipe(recipe, &lock, 1).unwrap().unwrap();

    let mut logical_ids = BTreeSet::new();
    for (index, (artifact, resource)) in bundle
        .plan
        .artifacts
        .iter()
        .zip(direct.resources.iter())
        .enumerate()
    {
        let PlannedArtifact::Dicom(artifact) = artifact else {
            panic!("stress CT emitted a non-DICOM artifact")
        };
        assert_eq!(artifact.order as usize, index);
        assert_eq!(artifact.resources, *resource);
        assert_eq!(
            artifact.output.relative_path.as_str(),
            format!(
                "stress/study/high_instance_count_ct/slice-{:03}.dcm",
                index + 1
            )
        );
        assert_eq!(artifact.case_binding.as_ref().unwrap().case_id, CASE_ID);
        assert!(logical_ids.insert(artifact.logical_id.clone()));
        let obligation = &artifact.evidence.obligations[0];
        assert_eq!(obligation.parameters["qualification_scale"], "reduced");
        assert_eq!(obligation.parameters["full_scale_available"], false);
        assert!(
            obligation.parameters["full_scale_reason"]
                .as_str()
                .is_some_and(|reason| !reason.is_empty())
        );
    }
}

#[test]
fn stress_ct_plan_is_parallelism_independent_and_explicitly_ordered() {
    let serial = plan(1);
    let parallel = plan(32);
    let signature = |bundle: &CuratedScCorpusPlan| {
        bundle
            .plan
            .artifacts
            .iter()
            .map(|artifact| {
                let PlannedArtifact::Dicom(artifact) = artifact else {
                    panic!("stress CT emitted a non-DICOM artifact")
                };
                (
                    artifact.logical_id.clone(),
                    artifact.order,
                    artifact.output.relative_path.as_str().to_owned(),
                    artifact.instance.identities.clone(),
                    artifact.resources.clone(),
                )
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(signature(&serial), signature(&parallel));
    assert_eq!(serial.bindings, parallel.bindings);
    assert_eq!(
        serial.native_content_requests,
        parallel.native_content_requests
    );
    assert_eq!(
        signature(&parallel)
            .iter()
            .map(|(_, order, _, _, _)| *order)
            .collect::<Vec<_>>(),
        (0..128).collect::<Vec<_>>()
    );
}

#[test]
fn stress_ct_is_opt_in_and_never_leaks_into_all_feature_free() {
    let all_feature_free = provider()
        .plan(&CuratedScPlanRequest {
            selection: CuratedScSelection::AllFeatureFree,
            seed: 1,
            max_parallelism: 4,
        })
        .unwrap();
    let all_without_stress = provider()
        .plan(&CuratedScPlanRequest {
            selection: CuratedScSelection::Profile {
                profile: "all".into(),
                include_stress: false,
            },
            seed: 1,
            max_parallelism: 4,
        })
        .unwrap();
    for bundle in [&all_feature_free, &all_without_stress] {
        assert!(bundle.plan.artifacts.iter().all(|artifact| {
            let PlannedArtifact::Dicom(artifact) = artifact else {
                return true;
            };
            artifact
                .case_binding
                .as_ref()
                .is_none_or(|binding| binding.case_id != CASE_ID)
        }));
    }
    assert_eq!(plan(4).plan.artifacts.len(), 128);
}
