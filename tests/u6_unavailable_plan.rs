use synth_dicom_gen::corpus_plan::CapabilityKind;
use synth_dicom_gen::curated_plan::{
    CuratedCatalogPaths, CuratedScCorpusPlanProvider, CuratedScPlanRequest, CuratedScSelection,
};

fn provider() -> CuratedScCorpusPlanProvider {
    CuratedScCorpusPlanProvider::load(CuratedCatalogPaths::from_repository_root(".")).unwrap()
}

#[test]
fn feature_gated_case_is_an_explicit_plan_capability() {
    let bundle = provider()
        .plan(&CuratedScPlanRequest {
            selection: CuratedScSelection::CaseIds(vec![
                "derived/seg/binary_multiframe_deflated_image_frame".into(),
            ]),
            seed: 1,
            max_parallelism: 1,
        })
        .unwrap();

    assert_eq!(bundle.pending.len(), 1);
    assert_eq!(bundle.plan.unavailable.len(), 2);
    let unavailable = bundle
        .plan
        .unavailable
        .iter()
        .find(|item| item.kind == CapabilityKind::Feature)
        .expect("compile-time feature capability");
    assert_eq!(unavailable.reason_code, "feature_disabled");
    assert_eq!(
        unavailable.requirements["features"],
        vec!["deflate".to_string()]
    );
    assert_eq!(
        bundle.pending[0].artifact_ids,
        unavailable.affected_artifact_ids
    );
    let codec = bundle
        .plan
        .unavailable
        .iter()
        .find(|item| item.kind == CapabilityKind::Codec)
        .expect("runtime codec capability");
    assert_eq!(codec.reason_code, "codec_backend_unavailable");
    assert_eq!(
        codec.affected_artifact_ids,
        unavailable.affected_artifact_ids
    );
}

#[test]
fn external_construction_backend_is_explicitly_unavailable() {
    let case_id = "derived/sr/tid1500_ct_measurement_report";
    let bundle = provider()
        .plan(&CuratedScPlanRequest {
            selection: CuratedScSelection::CaseIds(vec![case_id.into()]),
            seed: 1,
            max_parallelism: 1,
        })
        .unwrap();

    assert_eq!(bundle.pending.len(), 1);
    assert_eq!(bundle.pending[0].case_id, case_id);
    assert_eq!(bundle.plan.unavailable.len(), 1);
    assert_eq!(
        bundle.plan.unavailable[0].kind,
        CapabilityKind::ExternalBackend
    );
    assert_eq!(
        bundle.plan.unavailable[0].reason_code,
        "external_backend_unavailable"
    );
    assert!(
        bundle.plan.unavailable[0]
            .message
            .contains("external.highdicom_sr_import_plan")
    );
}

#[test]
fn profile_selection_retains_unavailable_cases() {
    let bundle = provider()
        .plan(&CuratedScPlanRequest {
            selection: CuratedScSelection::Profile {
                profile: "extended".into(),
                include_stress: false,
            },
            seed: 1,
            max_parallelism: 2,
        })
        .unwrap();

    assert!(bundle.pending.iter().any(|pending| {
        pending.case_id == "derived/seg/binary_multiframe_deflated_image_frame"
    }));
    assert!(
        bundle
            .pending
            .iter()
            .any(|pending| { pending.case_id == "derived/sr/tid1500_ct_measurement_report" })
    );
}
