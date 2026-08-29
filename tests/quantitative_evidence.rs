#[path = "../src/quantitative_evidence.rs"]
mod quantitative_evidence;

use quantitative_evidence::*;

fn segmentation() -> SegmentationValidationContract {
    SegmentationValidationContract {
        transfer_syntax_uid: "1.2.840.10008.1.2.1".into(),
        modality: "SEG".into(),
        frame_of_reference_uid: "2.25.9".into(),
        image_type: "DERIVED\\PRIMARY".into(),
        segmentation_type: "BINARY".into(),
        segmentation_fractional_type: None,
        maximum_fractional_value: None,
        segment_sequence_items: 1,
        shared_functional_groups_sequence_items: 1,
        per_frame_functional_groups_sequence_items: 2,
        dimension_index_count: 1,
        dimension_organization_uid: "2.25.10".into(),
        referenced_sop_class_uid: "1.2.840.10008.5.1.4.1.1.2.1".into(),
        referenced_sop_instance_uid: "2.25.11".into(),
        referenced_frame_numbers: vec![1, 2],
        frame_sha256: vec!["a".repeat(64), "b".repeat(64)],
    }
}

fn segmentation_observation(expected: &SegmentationValidationContract) -> SegmentationObservation {
    SegmentationObservation {
        modality: expected.modality.clone(),
        frame_of_reference_uid: expected.frame_of_reference_uid.clone(),
        image_type: expected.image_type.clone(),
        lossy_image_compression: "00".into(),
        segmentation_type: expected.segmentation_type.clone(),
        segmentation_fractional_type: None,
        maximum_fractional_value: None,
        segment_sequence_items: 1,
        segment_number: 1,
        segment_algorithm_type: "AUTOMATIC".into(),
        shared_functional_groups_sequence_items: 1,
        per_frame_functional_groups_sequence_items: 2,
        dimension_organization_sequence_items: 1,
        dimension_index_sequence_items: 1,
        dimension_organization_uid: expected.dimension_organization_uid.clone(),
        referenced_sop_class_uid: expected.referenced_sop_class_uid.clone(),
        referenced_sop_instance_uid: expected.referenced_sop_instance_uid.clone(),
        common_reference_sop_class_uid: expected.referenced_sop_class_uid.clone(),
        common_reference_sop_instance_uid: expected.referenced_sop_instance_uid.clone(),
        frames: expected
            .referenced_frame_numbers
            .iter()
            .map(|number| SegmentationFrameObservation {
                referenced_segment_number: 1,
                source_sop_class_uid: expected.referenced_sop_class_uid.clone(),
                source_sop_instance_uid: expected.referenced_sop_instance_uid.clone(),
                source_frame_number: *number,
            })
            .collect(),
        frame_sha256: expected.frame_sha256.clone(),
    }
}

#[test]
fn segmentation_adapter_uses_observed_values_and_fails_corruption() {
    let expected = segmentation();
    let observed = segmentation_observation(&expected);
    let report = validate_native_segmentation(&expected, &observed).unwrap();
    assert_eq!(report.internal[0].name, "segmentation_modality");
    assert_eq!(report.legacy_json()["status"], "passed");
    let mut corrupted = observed;
    corrupted.frame_sha256[1] = "c".repeat(64);
    assert_eq!(
        validate_native_segmentation(&expected, &corrupted)
            .unwrap_err()
            .to_string(),
        "quantitative validation failed: segmentation_frame_payload_hashes"
    );
}

#[test]
fn rwvm_adapter_preserves_legacy_order_and_rejects_pixel_data() {
    let expected = RwvmValidationContract {
        modality: "RWV".into(),
        content_label: "DTSRWVM".into(),
        lut_label: "HU".into(),
        first_value_mapped: 0,
        last_value_mapped: 4095,
        intercept: -1024.0,
        slope: 1.0,
        unit_code_value: "HU".into(),
        unit_coding_scheme_designator: "UCUM".into(),
        unit_code_meaning: "Hounsfield unit".into(),
        referenced_sop_class_uid: "1.2.3".into(),
        referenced_sop_instance_uid: "2.25.3".into(),
        referenced_series_instance_uid: "2.25.2".into(),
        referenced_frame_numbers: vec![1, 2],
    };
    let mut observed = RwvmObservation {
        modality: expected.modality.clone(),
        content_label: expected.content_label.clone(),
        lut_label: expected.lut_label.clone(),
        first_value_mapped: 0,
        last_value_mapped: 4095,
        intercept: -1024.0,
        slope: 1.0,
        unit_code_value: "HU".into(),
        unit_coding_scheme_designator: "UCUM".into(),
        unit_code_meaning: "Hounsfield unit".into(),
        referenced_sop_class_uid: "1.2.3".into(),
        referenced_sop_instance_uid: "2.25.3".into(),
        referenced_series_instance_uid: "2.25.2".into(),
        referenced_frame_numbers: vec![1, 2],
        pixel_data_absent: true,
    };
    let report = validate_native_rwvm(&expected, &observed).unwrap();
    assert_eq!(
        report.internal.last().unwrap().name,
        "rwvm_pixel_data_absent"
    );
    let projected = project_native_rwvm_manifest_fields(
        &NativeRwvmManifestProjection {
            source_case_id: "enhanced/ct/source".into(),
            source_sop_instance_uid: "2.25.3".into(),
            content_label: "DTSRWVM".into(),
            content_description: "Synthetic CT linear real world value mapping".into(),
            lut_label: "HU".into(),
            first_value_mapped: 0,
            last_value_mapped: 4095,
            intercept: -1024.0,
            slope: 1.0,
            unit_code_value: "HU".into(),
            unit_coding_scheme_designator: "UCUM".into(),
            unit_code_meaning: "Hounsfield unit".into(),
            referenced_frame_numbers: vec![1, 2],
        },
        &report,
    );
    assert!(projected["image"].is_null());
    assert!(projected["pixel_data"].is_null());
    assert_eq!(
        projected["expected_semantics"]["real_world_value_mapping"]["slope"],
        1.0
    );
    observed.pixel_data_absent = false;
    assert!(validate_native_rwvm(&expected, &observed).is_err());
}

#[test]
fn native_projection_preserves_nulls_and_public_semantics() {
    let contract = segmentation();
    let observed = segmentation_observation(&contract);
    let report = validate_native_segmentation(&contract, &observed).unwrap();
    let projected = project_native_seg_manifest_fields(
        &NativeSegManifestProjection {
            source_case_id: "enhanced/ct/source".into(),
            source_sop_instance_uid: "2.25.11".into(),
            rows: 2,
            columns: 2,
            frames: 2,
            bits_allocated: 1,
            bits_stored: 1,
            high_bit: 0,
            pixel_values: vec![1, 0, 0, 1, 0, 1, 1, 0],
            segmentation_type: "BINARY".into(),
            segmentation_fractional_type: None,
            maximum_fractional_value: None,
            segment_label: "DTS_SYNTHETIC_REGION".into(),
            referenced_frame_numbers: vec![1, 2],
            dimension_organization_uid: "2.25.10".into(),
            pixel_min: 0,
            pixel_max: 1,
            frame_sha256: contract.frame_sha256,
            pixel_value_length: Some(2),
            visual_pattern: "two_frame_binary_segmentation_mask".into(),
            stressors: vec!["binary_bit_packed_pixel_data".into()],
        },
        &report,
    );
    assert!(projected["recipe_parameters"]["segmentation_fractional_type"].is_null());
    assert_eq!(projected["pixel_data"]["native_or_encapsulated"], "native");
    assert_eq!(projected["known_stressors"][0], "segmentation_storage");
    assert_eq!(projected["validation"], report.legacy_json());
}

#[test]
fn external_import_and_unavailable_evidence_are_strict_and_exact() {
    let sha = "a".repeat(64);
    let evidence = ExternalBackendEvidence {
        backend_id: "highdicom_pydicom".into(),
        protocol_version: "0.1.0".into(),
        name: "highdicom-pydicom adapter".into(),
        version: "0.5.0".into(),
        dependency_lock_sha256: sha.clone(),
        executable_fingerprint: sha.clone(),
        entrypoint_fingerprint: sha.clone(),
        environment_fingerprint: sha,
        runtime_identity: "python-3.12".into(),
        invocation_elapsed_milliseconds: 4,
        warnings: vec![],
    };
    let projected = project_external_import_evidence(
        &evidence,
        vec![QuantitativeCheck {
            name: "external_backend_contract".into(),
            status: "passed".into(),
            message: "The locked backend response and provenance satisfied protocol 0.1.0.".into(),
        }],
        vec![],
    )
    .unwrap();
    assert_eq!(
        projected["generation_backend"]["determinism"],
        "semantic_stable"
    );
    assert_eq!(
        projected["validation"]["internal"][0]["name"],
        "external_backend_contract"
    );
    let unavailable = project_external_unavailable(
        "derived/parametric-map/float32_ct_derived_explicit_le",
        "missing",
        "backend not installed",
        "phase-1",
        vec![serde_json::json!({"source":"PS3.3"})],
    )
    .unwrap();
    assert_eq!(
        unavailable,
        serde_json::json!({"case_id":"derived/parametric-map/float32_ct_derived_explicit_le","status":"unavailable","reason_code":"external_backend_unavailable","message":"missing: backend not installed","recheck_phase":"phase-1","standards_evidence":[{"source":"PS3.3"}]})
    );
    assert!(project_external_unavailable("case", "missing", "message", "phase-9", vec![]).is_err());
}
