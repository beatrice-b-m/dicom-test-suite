#[path = "../src/sr_rt_manifest.rs"]
mod sr_rt_manifest;
#[path = "../src/sr_rt_validation.rs"]
mod sr_rt_validation;

use serde_json::{Value, json};

use sr_rt_manifest::*;
use sr_rt_validation::*;

const HASH: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

fn identity(modality: &str, sop_class_uid: &str) -> DicomIdentityObservation {
    DicomIdentityObservation {
        sop_class_uid: sop_class_uid.into(),
        sop_instance_uid: "1.2.3.4".into(),
        transfer_syntax_uid: "1.2.840.10008.1.2.1".into(),
        implementation_class_uid: "1.2.826.0.1".into(),
        synthetic_data: "YES".into(),
        modality: modality.into(),
    }
}

fn source(role: &str) -> SemanticReferenceObservation {
    SemanticReferenceObservation {
        role: role.into(),
        study_instance_uid: "1.2.3".into(),
        series_instance_uid: "1.2.3.1".into(),
        sop_class_uid: "1.2.840.10008.5.1.4.1.1.2.1".into(),
        sop_instance_uid: "1.2.3.1.1".into(),
        referenced_frames: vec![1],
    }
}

fn validation() -> SpecializedValidationEvidence {
    validate_native_sr(
        &NativeSrValidationContract {
            kind: NativeSrKind::BasicText,
            identity: identity("SR", "1.2.840.10008.5.1.4.1.1.88.11"),
            completion_flag: "COMPLETE".into(),
            verification_flag: "UNVERIFIED".into(),
            continuity_of_content: "SEPARATE".into(),
            title_code_value: "18748-4".into(),
            title_coding_scheme_designator: "LN".into(),
            title_code_meaning: "Diagnostic imaging report".into(),
            content_tree_sha256: HASH.into(),
            references: vec![source("source_image")],
        },
        &NativeSrObservation {
            kind: NativeSrKind::BasicText,
            identity: identity("SR", "1.2.840.10008.5.1.4.1.1.88.11"),
            completion_flag: "COMPLETE".into(),
            verification_flag: "UNVERIFIED".into(),
            continuity_of_content: "SEPARATE".into(),
            title_code_value: "18748-4".into(),
            title_coding_scheme_designator: "LN".into(),
            title_code_meaning: "Diagnostic imaging report".into(),
            content_tree_sha256: HASH.into(),
            references: vec![source("source_image")],
        },
    )
    .unwrap()
}

fn common(modality: &str, determinism: &str) -> SemanticManifestSpec {
    SemanticManifestSpec {
        case_id: "case".into(),
        profile_membership: vec!["extended".into()],
        recipe_id: "recipe".into(),
        recipe_version: "0.1.0".into(),
        recipe_parameters: json!({"typed": true}),
        output: ManifestOutputFacts {
            relative_path: "case/instance.dcm".into(),
            sha256: HASH.into(),
            size_bytes: 128,
        },
        determinism: determinism.into(),
        sop_class_uid: "1.2.840.10008.5.1.4.1.1.481.3".into(),
        sop_class_name: "Storage".into(),
        iod_name: "IOD".into(),
        modality: modality.into(),
        transfer_syntax_uid: "1.2.840.10008.1.2.1".into(),
        transfer_syntax_name: "Explicit VR Little Endian".into(),
        identities: ManifestIdentityFacts {
            study_instance_uid: "1.2.3".into(),
            series_instance_uid: "1.2.3.1".into(),
            sop_instance_uid: "1.2.3.1.2".into(),
            frame_of_reference_uid: Some("1.2.3.9".into()),
            implementation_class_uid: "1.2.826.0.1".into(),
            implementation_version_name: "DTS_0_1_0".into(),
        },
        references: vec![ManifestReferenceFacts {
            case_id: "source".into(),
            path: "source/instance.dcm".into(),
            sha256: HASH.into(),
            sop_class_uid: "1.2.3".into(),
            sop_instance_uid: "1.2.3.4".into(),
            role: "source_image".into(),
            referenced_frames: Some(vec![1]),
        }],
        expected_capabilities: vec!["open_file".into(), "read_metadata".into()],
        expected_semantics: json!({"synthetic_data": "YES"}),
        expected_visual_pattern: "metadata_only".into(),
        known_stressors: vec!["reference_graph".into()],
        standards_evidence: vec![json!({"part": "PS3.3"})],
    }
}

#[test]
fn native_sr_validation_is_observation_bound_and_projects_exact_legacy_shape() {
    let evidence = validation();
    assert_eq!(
        evidence
            .internal
            .iter()
            .map(|check| check.name.as_str())
            .collect::<Vec<_>>(),
        [
            "sr_part10_identity",
            "sr_document_kind",
            "sr_document_flags",
            "sr_title",
            "sr_content_tree",
            "sr_reference_graph"
        ]
    );

    let projected = project_sr_manifest_entry(
        &SrManifestProjection {
            kind: SrManifestKind::BasicText,
            common: common("SR", "byte_stable"),
            external_import: None,
        },
        &evidence,
    )
    .unwrap();
    assert_eq!(
        projected,
        json!({
            "case_id": "case", "profile_membership": ["extended"], "path": "case/instance.dcm",
            "sha256": HASH, "size_bytes": 128, "determinism": "byte_stable",
            "recipe": {"recipe_id": "recipe", "recipe_version": "0.1.0", "recipe_parameters": {"typed": true}},
            "dicom": {"sop_class_uid": "1.2.840.10008.5.1.4.1.1.481.3", "sop_class_name": "Storage", "iod_name": "IOD", "modality": "SR", "transfer_syntax_uid": "1.2.840.10008.1.2.1", "transfer_syntax_name": "Explicit VR Little Endian"},
            "uids": {"study_instance_uid": "1.2.3", "series_instance_uid": "1.2.3.1", "sop_instance_uid": "1.2.3.1.2", "frame_of_reference_uid": "1.2.3.9", "implementation_class_uid": "1.2.826.0.1", "implementation_version_name": "DTS_0_1_0"},
            "image": Value::Null, "pixel_data": Value::Null,
            "references": [{"case_id": "source", "path": "source/instance.dcm", "sha256": HASH, "sop_class_uid": "1.2.3", "sop_instance_uid": "1.2.3.4", "role": "source_image", "referenced_frames": [1]}],
            "expected_capabilities": ["open_file", "read_metadata"], "expected_semantics": {"synthetic_data": "YES"},
            "expected_visual_checks": {"pattern": "metadata_only"}, "validation": evidence.legacy_json(),
            "known_stressors": ["reference_graph"], "standards_evidence": [{"part": "PS3.3"}]
        })
    );

    let mut corrupt = NativeSrValidationContract {
        kind: NativeSrKind::BasicText,
        identity: identity("SR", "1"),
        completion_flag: "COMPLETE".into(),
        verification_flag: "UNVERIFIED".into(),
        continuity_of_content: "SEPARATE".into(),
        title_code_value: "x".into(),
        title_coding_scheme_designator: "x".into(),
        title_code_meaning: "x".into(),
        content_tree_sha256: HASH.into(),
        references: vec![],
    };
    let expected = corrupt.clone();
    corrupt.completion_flag = "PARTIAL".into();
    assert_eq!(
        validate_native_sr(&expected, &corrupt),
        Err(SrRtEvidenceError::ValidationFailed(
            "sr_document_flags".into()
        ))
    );
}

#[test]
fn all_six_rt_specializations_validate_and_project_their_historical_key() {
    let variants = vec![
        (
            RtManifestKind::StructureSet,
            RtObjectObservation::StructureSet {
                roi_count: 1,
                contour_count: 1,
                contour_points: 4,
            },
            "expected_rt_structure_set",
            false,
        ),
        (
            RtManifestKind::Dose,
            RtObjectObservation::Dose {
                rows: 2,
                columns: 2,
                frames: 2,
                dose_units: "GY".into(),
                dose_type: "PHYSICAL".into(),
                dose_summation_type: "RECORD".into(),
                dose_grid_scaling: "0.001".into(),
                pixel_sha256: HASH.into(),
            },
            "expected_rt_dose",
            true,
        ),
        (
            RtManifestKind::Plan,
            RtObjectObservation::Plan {
                fraction_group_count: 1,
                beam_count: 1,
                control_point_count: 2,
                plan_geometry: "PATIENT".into(),
            },
            "expected_rt_plan",
            false,
        ),
        (
            RtManifestKind::Image,
            RtObjectObservation::Image {
                rows: 4,
                columns: 4,
                referenced_beam_number: 1,
                referenced_fraction_group_number: 1,
                pixel_sha256: HASH.into(),
            },
            "expected_rt_image",
            true,
        ),
        (
            RtManifestKind::CarmRadiation,
            RtObjectObservation::CarmRadiation {
                treatment_position_count: 1,
                control_point_count: 2,
                rt_record_flag: "NO".into(),
            },
            "expected_rt_radiation",
            false,
        ),
        (
            RtManifestKind::RadiationSet,
            RtObjectObservation::RadiationSet {
                treatment_position_group_count: 1,
                radiation_count: 1,
                dose_contribution_absent: true,
            },
            "expected_rt_radiation_set",
            false,
        ),
    ];
    for (kind, object, expected_key, pixels) in variants {
        let contract = RtValidationContract {
            identity: identity("RT", "1.2.3"),
            label: "DTS".into(),
            object: object.clone(),
            references: vec![source("definition_source")],
            pixel_data_absent: !pixels,
        };
        let evidence = validate_rt_object(&contract, &contract).unwrap();
        let projection = RtManifestProjection {
            kind,
            common: common("RT", "byte_stable"),
            expected_rt_object: serde_json::to_value(&object).unwrap(),
            image: pixels.then(|| json!({"rows": 2})),
            pixel_data: pixels.then(|| json!({"vr": "OW"})),
        };
        let value = project_rt_manifest_entry(&projection, &evidence).unwrap();
        assert_eq!(value[expected_key], serde_json::to_value(object).unwrap());
        assert_eq!(value["image"].is_null(), !pixels);
        assert_eq!(value["pixel_data"].is_null(), !pixels);
        assert_eq!(value["validation"]["status"], "passed");
    }
}

#[test]
fn highdicom_sr_import_evidence_is_pinned_complete_and_fail_closed() {
    let required = vec!["tid1500_content_tree".into(), "measurement_group".into()];
    let evidence = HighDicomSrImportEvidence {
        backend_id: HIGH_DICOM_SR_BACKEND_ID.into(),
        protocol_version: HIGH_DICOM_SR_PROTOCOL_VERSION.into(),
        dependency: "highdicom".into(),
        version: HIGH_DICOM_SR_VERSION.into(),
        dependency_lock_sha256: HIGH_DICOM_SR_DEPENDENCY_SHA256.into(),
        executable_fingerprint: HASH.into(),
        entrypoint_fingerprint: HASH.into(),
        environment_fingerprint: HASH.into(),
        request_sha256: HASH.into(),
        response_sha256: HASH.into(),
        output_sha256: HASH.into(),
        output_size_bytes: 4096,
        maximum_output_bytes: 1_048_576,
        semantic_evidence: required.clone(),
        warnings: vec![],
    };
    let projected = validate_highdicom_sr_import(&evidence, &required).unwrap();
    assert_eq!(projected["backend_id"], HIGH_DICOM_SR_BACKEND_ID);
    assert_eq!(projected["version"], HIGH_DICOM_SR_VERSION);
    assert_eq!(
        projected["dependency_lock_sha256"],
        HIGH_DICOM_SR_DEPENDENCY_SHA256
    );
    assert_eq!(projected["determinism"], "semantic_stable");

    let mut drift = evidence;
    drift.version = "0.29.0".into();
    assert_eq!(
        validate_highdicom_sr_import(&drift, &required),
        Err(SrRtEvidenceError::InvalidExternalImport)
    );
}

#[test]
fn projection_rejects_kind_pixel_and_external_evidence_mismatches() {
    let evidence = validation();
    let invalid_rt = RtManifestProjection {
        kind: RtManifestKind::Dose,
        common: common("RTDOSE", "byte_stable"),
        expected_rt_object: json!({"dose": true}),
        image: None,
        pixel_data: None,
    };
    assert_eq!(
        project_rt_manifest_entry(&invalid_rt, &evidence),
        Err(SemanticManifestError::KindMismatch)
    );

    let invalid_sr = SrManifestProjection {
        kind: SrManifestKind::Tid1500,
        common: common("SR", "semantic_stable"),
        external_import: None,
    };
    assert_eq!(
        project_sr_manifest_entry(&invalid_sr, &evidence),
        Err(SemanticManifestError::KindMismatch)
    );
}

#[test]
fn additive_modules_have_no_filesystem_generator_or_frontend_dependencies() {
    for path in ["src/sr_rt_validation.rs", "src/sr_rt_manifest.rs"] {
        let source = std::fs::read_to_string(path).unwrap();
        for forbidden in [
            "std::fs",
            "generator",
            "crate::cli",
            "open_file(",
            "read_to_end",
        ] {
            assert!(
                !source.contains(forbidden),
                "{path} contains forbidden dependency {forbidden}"
            );
        }
    }
}
