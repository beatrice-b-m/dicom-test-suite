use std::collections::BTreeSet;
use std::path::Path;

use synth_dicom_gen::corpus_plan::PlannedArtifact;
use synth_dicom_gen::curated_plan::{
    CuratedCatalogPaths, CuratedScCorpusPlanProvider, CuratedScPlanRequest, CuratedScSelection,
};

const CASES: [&str; 6] = [
    "derived/registration/spatial_ct_pair",
    "derived/registration/deformable_ct_pair",
    "derived/presentation-state/color_softcopy",
    "derived/presentation-state/advanced_blending",
    "derived/presentation-state/blending",
    "derived/presentation-state/grayscale_softcopy_ct_window_explicit_le",
];

fn reference_plan() -> synth_dicom_gen::curated_plan::CuratedScCorpusPlan {
    CuratedScCorpusPlanProvider::load(CuratedCatalogPaths::from_repository_root("."))
        .unwrap()
        .plan(&CuratedScPlanRequest {
            selection: CuratedScSelection::CaseIds(CASES.map(str::to_owned).to_vec()),
            seed: 1,
            max_parallelism: 3,
        })
        .unwrap()
}

#[test]
fn reference_recipes_plan_with_closed_source_dags_before_output_exists() {
    let absent = Path::new("target/curated-reference-plan-must-not-exist");
    assert!(!absent.exists());
    let bundle = reference_plan();
    assert!(!absent.exists());

    let planned_cases = bundle
        .plan
        .artifacts
        .iter()
        .filter_map(|artifact| match artifact {
            PlannedArtifact::Dicom(artifact) => artifact
                .case_binding
                .as_ref()
                .map(|binding| binding.case_id.clone()),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    for case_id in CASES {
        assert!(planned_cases.contains(case_id), "missing {case_id}");
    }

    for case_id in CASES {
        let target = bundle
            .plan
            .artifacts
            .iter()
            .find_map(|artifact| match artifact {
                PlannedArtifact::Dicom(artifact)
                    if artifact
                        .case_binding
                        .as_ref()
                        .is_some_and(|binding| binding.case_id == case_id) =>
                {
                    Some(artifact)
                }
                _ => None,
            })
            .unwrap();
        assert!(!target.instance.references.is_empty());
        let dependencies = bundle
            .plan
            .dependencies
            .iter()
            .filter(|edge| edge.artifact_id == target.logical_id)
            .collect::<Vec<_>>();
        assert_eq!(dependencies.len(), target.instance.references.len());
        assert!(dependencies.iter().all(|edge| {
            bundle
                .plan
                .artifacts
                .iter()
                .any(|artifact| artifact.logical_id() == edge.depends_on)
        }));
    }
    bundle.plan.validate().unwrap();
}
