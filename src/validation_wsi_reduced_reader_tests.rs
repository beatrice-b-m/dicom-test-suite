use super::*;

// Source planning and synthetic observations only: this is not an executed stress run.
fn reduced_source_manifest() -> serde_json::Value {
    use crate::corpus_plan::PlannedArtifact;
    use crate::curated_plan::{
        CuratedCatalogPaths, CuratedScCorpusPlanProvider, CuratedScPlanRequest, CuratedScSelection,
    };
    use crate::executor::adapters::{ManifestProjectionArtifact, ManifestProjectionInput};
    use serde_json::json;
    let provider =
        CuratedScCorpusPlanProvider::load(CuratedCatalogPaths::from_repository_root(".")).unwrap();
    let bundle = provider
        .plan(&CuratedScPlanRequest {
            selection: CuratedScSelection::CaseIds(vec!["stress/wsi/large_pyramid".into()]),
            seed: 7,
            max_parallelism: 1,
        })
        .unwrap();
    let artifacts = bundle.plan.artifacts.iter().enumerate().map(|(index, planned)| {
    let PlannedArtifact::Dicom(dicom) = planned else { panic!("DICOM required"); };
    let frames = [16,4,1][index];
    let frame = vec![0_u8; 256 * 256 * 3];
    let frame_hash = crate::sha256_hex(&frame);
    ManifestProjectionArtifact { planned:planned.clone(), execution:serde_json::from_value(json!({
        "logical_id":dicom.logical_id,"order":dicom.order,"artifact_kind":"dicom","status":"succeeded",
        "corpus_plan_sha256":"0".repeat(64),"instance_plan_sha256":dicom.instance.canonical_sha256(),
        "output":{"relative_path":dicom.output.relative_path.as_str(),"publish":true,"size_bytes":1000,"sha256":"0".repeat(64)},
        "materialization":{"backend_id":"synthetic_test","transfer_syntax_uid":dicom.encoding.transfer_syntax_uid,"completed":true,
            "materialized_instance_plan_sha256":dicom.instance.canonical_sha256(),"materialized_artifact_sha256":"0".repeat(64),
            "implementation_class_uid":dicom.encoding.implementation.class_uid,
            "content":[{"slot":"pixels","kind":"native","vr":"OB","size_bytes":frame.len()*frames,"sha256":"0".repeat(64),"native_frame_sha256":vec![frame_hash;frames]}]},
        "validation":[{"rule_id":"synthetic_test","layer":"internal","required":true,"status":"passed","message":"synthetic projection observation","details":{"checks":[]}}],
        "obligations":[],"providers":[],"codecs":[],
        "resources":{"planned_output_bytes":2000,"planned_peak_working_bytes":2000,"actual_output_bytes":1000,"actual_peak_working_bytes":null,"elapsed_milliseconds":0}
    })).unwrap() }
}).collect();
    let input = ManifestProjectionInput {
    corpus_plan_sha256:"0".repeat(64), artifacts, unavailable:vec![],
    resources:serde_json::from_value(json!({"planned_max_artifacts":3,"planned_max_total_output_bytes":6000,"planned_max_peak_working_bytes":6000,"requested_parallelism":1,"used_parallelism":1,"actual_artifact_output_bytes":3000,"actual_publication_bytes":0,"actual_peak_working_bytes":null})).unwrap(),
    publication:serde_json::from_value(json!({"manifest_relative_path":"manifest.json","state":"staging","private_staging":true,"no_overwrite":true,"validation_complete":false,"cleanup_complete":false,"manifest_sha256":null})).unwrap(),
};
    let files =
        crate::curated_manifest::project_curated_file_entries(&bundle.projection, &input).unwrap();
    let qualifications =
        crate::curated_manifest::project_curated_stress_qualifications(&bundle.projection, &input)
            .unwrap();
    json!({"run":{"profile":"stress"},"qualifications":qualifications,
    "selection_ledger":[{"case_id":files[0]["case_id"],"case_definition":bundle.projection.artifacts[0].registry_case,
        "outcome":"generated","artifact_paths":files.iter().map(|f|f["path"].clone()).collect::<Vec<_>>()}],"files":files})
}

#[test]
fn reduced_wsi_context_requires_source_projected_complete_evidence() {
    let manifest = reduced_source_manifest();
    let kind = crate::manifest_contract::ManifestContractKind::ExternalCorpus;
    let path = std::path::Path::new("synthetic-manifest.json");
    let check = |m: &serde_json::Value| {
        crate::reduced_stress_wsi_contexts(kind, path, m, m["files"].as_array().unwrap())
    };
    let contexts = check(&manifest).unwrap();
    assert_eq!(contexts.len(), 3);
    for file in manifest["files"].as_array().unwrap() {
        let context = contexts.get(file["path"].as_str().unwrap()).unwrap();
        crate::validate_external_family_evidence_scope_with_context(
            kind,
            path,
            file,
            Some(context),
        )
        .unwrap();
        assert!(
            crate::validate_external_family_evidence_scope_with_context(kind, path, file, None)
                .is_err()
        );
    }
    for mutation in 0..14 {
        let mut changed = manifest.clone();
        match mutation {
            0 => changed["qualifications"] = serde_json::json!([]),
            1 => {
                let q = changed["qualifications"][0].clone();
                changed["qualifications"].as_array_mut().unwrap().push(q);
            }
            2 => {
                changed["selection_ledger"][0]["case_definition"]["profiles"] =
                    serde_json::json!(["core"])
            }
            3 => {
                changed["files"].as_array_mut().unwrap().pop();
            }
            4 => {
                let f = changed["files"][0].clone();
                changed["files"].as_array_mut().unwrap().push(f);
            }
            5 => changed["files"][1]["recipe"]["recipe_parameters"]["level_index"] = 0.into(),
            6 => {
                changed["files"][1]["expected_semantics"]["shared_pyramid_uid"] = "2.25.999".into()
            }
            7 => changed["files"][0]["expected_wsi_pyramid"] = serde_json::json!({}),
            8 => {
                changed["files"][0]["recipe"]["recipe_parameters"]["total_pixel_matrix_rows"] =
                    512.into()
            }
            9 => changed["selection_ledger"][0]["artifact_paths"][0] = "wrong.dcm".into(),
            10 => changed["selection_ledger"][0]["outcome"] = "unavailable".into(),
            11 => changed["qualifications"][0]["outcome"] = "unavailable".into(),
            12 => changed["files"][0]["uids"]["study_instance_uid"] = "2.25.999".into(),
            _ => {
                changed["files"][0]["recipe"]["recipe_parameters"]["pixel_spacing"] = "1\\1".into()
            }
        }
        if mutation == 3 || mutation == 4 {
            // Reach group completeness after preserving the run's byte accounting.
            let total = changed["files"].as_array().unwrap().len() as u64 * 1000;
            changed["qualifications"][0]["actual"]["output_bytes"] = total.into();
            changed["qualifications"][0]["observation"]["output_bytes"] = total.into();
            crate::validate_stress_profile_qualifications_for_kind(
                kind,
                path,
                &changed,
                changed["files"].as_array().unwrap(),
            )
            .unwrap();
        }
        assert!(check(&changed).is_err(), "mutation {mutation}");
    }
    let mut unavailable = manifest.clone();
    unavailable["files"] = serde_json::json!([]);
    unavailable["qualifications"] = serde_json::json!([]);
    unavailable["selection_ledger"][0]["outcome"] = "unavailable".into();
    unavailable["selection_ledger"][0]["artifact_paths"] = serde_json::json!([]);
    assert!(check(&unavailable).unwrap().is_empty());
    let files = manifest["files"].as_array().unwrap();
    assert!(
        crate::validate_external_family_evidence_scope_with_context(
            kind,
            path,
            &files[1],
            contexts.get(files[0]["path"].as_str().unwrap())
        )
        .is_err()
    );
}

#[test]
fn reduced_wsi_reopened_context_preserves_payload_checks() {
    let manifest = reduced_source_manifest();
    let kind = crate::manifest_contract::ManifestContractKind::ExternalCorpus;
    let manifest_path = std::path::Path::new("synthetic-manifest.json");
    let files = manifest["files"].as_array().unwrap();
    let contexts =
        crate::reduced_stress_wsi_contexts(kind, manifest_path, &manifest, files).unwrap();
    let file = &files[2];
    let context = contexts.get(file["path"].as_str().unwrap()).unwrap();
    for mutation in 0..3 {
        let mut obj = valid_object(Mutation::None);
        put_str(
            &mut obj,
            tags::SOP_INSTANCE_UID,
            VR::UI,
            file["uids"]["sop_instance_uid"].as_str().unwrap(),
        );
        put_str(&mut obj, tags::NUMBER_OF_FRAMES, VR::IS, "1");
        put_str(
            &mut obj,
            tags::DIMENSION_ORGANIZATION_TYPE,
            VR::CS,
            "TILED_FULL",
        );
        put_str(
            &mut obj,
            tags::PYRAMID_UID,
            VR::UI,
            if mutation == 2 {
                "2.25.999"
            } else {
                file["expected_semantics"]["shared_pyramid_uid"]
                    .as_str()
                    .unwrap()
            },
        );
        obj.put(DataElement::new(
            tags::ROWS,
            VR::US,
            PrimitiveValue::from(256_u16),
        ));
        obj.put(DataElement::new(
            tags::COLUMNS,
            VR::US,
            PrimitiveValue::from(256_u16),
        ));
        obj.put(DataElement::new(
            tags::TOTAL_PIXEL_MATRIX_ROWS,
            VR::UL,
            PrimitiveValue::from(if mutation == 1 { 512_u32 } else { 256_u32 }),
        ));
        obj.put(DataElement::new(
            tags::TOTAL_PIXEL_MATRIX_COLUMNS,
            VR::UL,
            PrimitiveValue::from(256_u32),
        ));
        obj.put(DataElement::new(
            tags::PIXEL_DATA,
            VR::OB,
            PrimitiveValue::from(vec![0_u8; 256 * 256 * 3]),
        ));
        let path = std::env::temp_dir().join(format!(
            "synth-dicom-gen-reduced-wsi-reader-{}-{mutation}.dcm",
            std::process::id()
        ));
        obj.with_meta(
            FileMetaTableBuilder::new()
                .media_storage_sop_class_uid("1.2.840.10008.5.1.4.1.1.77.1.6")
                .media_storage_sop_instance_uid(file["uids"]["sop_instance_uid"].as_str().unwrap())
                .transfer_syntax(uids::EXPLICIT_VR_LITTLE_ENDIAN)
                .implementation_class_uid(
                    file["uids"]["implementation_class_uid"].as_str().unwrap(),
                ),
        )
        .unwrap()
        .write_to_file(&path)
        .unwrap();
        crate::validate_external_family_evidence_scope_with_context(
            kind,
            manifest_path,
            file,
            Some(context),
        )
        .unwrap();
        let obj = dicom_object::open_file(&path).unwrap();
        let mut failures = vec![];
        crate::validate_family_standard_elements_with_context(
            kind,
            &mut failures,
            file["path"].as_str().unwrap(),
            &path,
            manifest_path,
            file,
            &obj,
            Some(context),
        )
        .unwrap();
        if mutation == 0 {
            assert!(failures.is_empty(), "{failures:?}");
            validate_manifest_wsi_file(&path, file).unwrap();
            assert!(
                crate::validation::validate_manifest_wsi_file_for_kind(kind, &path, file).is_err()
            );
        } else {
            assert!(
                failures
                    .iter()
                    .any(|failure| failure.contains("stress WSI matrix, Pyramid UID")),
                "{failures:?}"
            );
        }
        cleanup(path);
    }
}
