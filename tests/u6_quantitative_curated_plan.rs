use std::collections::BTreeSet;

use dicom_test_suite::composition::CompositionUidRole;
use dicom_test_suite::corpus_plan::PlannedArtifact;
use dicom_test_suite::curated_plan::{
    CuratedCatalogPaths, CuratedScCorpusPlanProvider, CuratedScPlanRequest, CuratedScSelection,
};

const NATIVE_CASES: [&str; 4] = [
    "derived/seg/binary_multiframe_explicit_le",
    "derived/seg/fractional_probability_multiframe_explicit_le",
    "derived/seg/labelmap_multiframe_explicit_le",
    "derived/rwvm/linear_ct_mapping_explicit_le",
];

#[test]
fn native_quantitative_cases_form_closed_plans_before_execution() {
    let bundle = CuratedScCorpusPlanProvider::load(CuratedCatalogPaths::from_repository_root("."))
        .unwrap()
        .plan(&CuratedScPlanRequest {
            selection: CuratedScSelection::CaseIds(NATIVE_CASES.map(str::to_owned).to_vec()),
            seed: 1,
            max_parallelism: 3,
        })
        .unwrap();

    let planned = bundle
        .plan
        .artifacts
        .iter()
        .filter_map(|artifact| match artifact {
            PlannedArtifact::Dicom(artifact) => artifact
                .case_binding
                .as_ref()
                .map(|binding| (binding.case_id.as_str(), artifact)),
            _ => None,
        })
        .collect::<Vec<_>>();
    let planned_ids = planned
        .iter()
        .map(|(case_id, _)| *case_id)
        .collect::<BTreeSet<_>>();
    for case_id in NATIVE_CASES {
        assert!(planned_ids.contains(case_id), "missing {case_id}");
        let artifact = planned
            .iter()
            .find_map(|(planned_case, artifact)| (*planned_case == case_id).then_some(*artifact))
            .unwrap();
        assert_eq!(artifact.instance.references.len(), 1);
        assert!(bundle.plan.dependencies.iter().any(|dependency| {
            dependency.artifact_id == artifact.logical_id
                && dependency.depends_on == artifact.instance.references[0].target_instance_id
        }));
        if case_id.contains("/seg/") {
            let dimension = artifact
                .instance
                .identities
                .get(&CompositionUidRole::DimensionOrganization, 0)
                .unwrap();
            let sop = artifact
                .instance
                .identities
                .get(&CompositionUidRole::SopInstance, 0)
                .unwrap();
            assert_ne!(dimension, sop);
        }
    }
    bundle.plan.validate().unwrap();
}
