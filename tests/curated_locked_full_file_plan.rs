use std::collections::{BTreeMap, BTreeSet};

use synth_dicom_gen::corpus_plan::{ArtifactProvenance, PlannedArtifact};
use synth_dicom_gen::curated_plan::{
    CuratedCatalogPaths, CuratedScCorpusPlanProvider, CuratedScPlanRequest, CuratedScSelection,
};
use synth_dicom_gen::executor::services::SlotExecutionBinding;
use synth_dicom_gen::runtime_capabilities::{CapabilityInventory, QualifiedExecutableIdentity};

fn set(values: &[&str]) -> BTreeSet<String> {
    values.iter().map(|value| (*value).into()).collect()
}

fn qualified_inventory() -> CapabilityInventory {
    CapabilityInventory {
        compiled_features: set(&["legacy_jpeg_dcmtk"]),
        executable_codec_backends: set(&[
            "dcmtk_dcmcjpeg_jpeg_lossless_process_14_command_writer",
            "dcmtk_dcmcjpeg_jpeg_lossless_sv1_command_writer",
        ]),
        available_executables: set(&["dcmcjpeg"]),
        executable_identities: BTreeMap::from([(
            "dcmcjpeg".into(),
            QualifiedExecutableIdentity {
                version: "fake-dcmcjpeg 3.6.9".into(),
                executable_sha256: "ab".repeat(32),
            },
        )]),
        ..CapabilityInventory::default()
    }
}

#[test]
fn locked_cases_plan_explicit_private_native_sources_and_imported_targets() {
    for case_id in [
        "classic/sc/mono2_u16_jpeg_lossless_process_14",
        "classic/sc/mono2_u16_jpeg_lossless_sv1",
    ] {
        let bundle =
            CuratedScCorpusPlanProvider::load(CuratedCatalogPaths::from_repository_root("."))
                .unwrap()
                .with_capability_inventory(qualified_inventory())
                .plan(&CuratedScPlanRequest {
                    selection: CuratedScSelection::CaseIds(vec![case_id.into()]),
                    seed: 1,
                    max_parallelism: 2,
                })
                .unwrap();
        assert!(bundle.pending.is_empty());
        assert_eq!(bundle.plan.artifacts.len(), 2);
        let PlannedArtifact::Dicom(source) = &bundle.plan.artifacts[0] else {
            panic!("locked source must be a neutral DICOM plan")
        };
        assert!(matches!(
            source.provenance,
            ArtifactProvenance::PrivateSource { .. }
        ));
        assert!(!source.output.publish);
        let PlannedArtifact::ImportedDicom(target) = &bundle.plan.artifacts[1] else {
            panic!("locked target must be an imported full-file result")
        };
        assert!(target.output.publish);
        assert_eq!(
            target.provider.source_assets.get("source_dicom"),
            Some(&source.logical_id)
        );
        assert_eq!(
            bundle.plan.topological_order().unwrap(),
            vec![source.logical_id.clone(), target.logical_id.clone()]
        );
        let SlotExecutionBinding::ProviderRequest { request } =
            &bundle.bindings[&target.logical_id].slots["dicom"]
        else {
            panic!("target needs the locked provider request")
        };
        assert_eq!(
            request.input_assets["source_dicom"].as_str(),
            format!("output:{}", source.logical_id)
        );
        assert!(
            bundle
                .locked_full_file_requests
                .contains_key(&target.logical_id)
        );
        bundle.plan.validate().unwrap();
    }
}
