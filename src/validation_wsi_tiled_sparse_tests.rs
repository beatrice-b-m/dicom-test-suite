use std::{fs, path::PathBuf};

use dicom_core::{DataElement, PrimitiveValue, VR, value::DataSetSequence};
use dicom_dictionary_std::{tags, uids};
use dicom_object::{FileMetaTableBuilder, InMemDicomObject};

use super::{
    Part10Expectations, PixelDataLengthFormula, validate_manifest_wsi_file,
    validate_wsi_tiled_sparse_file, wsi_tiled_full_tests,
};

const SOP_UID: &str = "2.25.8901";
const FOR_UID: &str = "2.25.8902";
const SPECIMEN_UID: &str = "2.25.8803";
const DIMENSION_UID: &str = "2.25.8904";
const IMPLEMENTATION_UID: &str = "2.25.8999";
const RED: [u8; 12] = [255, 0, 0, 255, 0, 0, 255, 0, 0, 255, 0, 0];
const WHITE: [u8; 12] = [255; 12];

#[derive(Clone, Copy)]
enum Mutation {
    None,
    WrongDimensionType,
    ReorderedDimensionIndices,
    WrongDimensionUid,
    WrongDimensionValues,
    WrongPositionVr,
    MissingFrameContent,
    DuplicatePosition,
    OffGridPosition,
    WrongCoordinate,
    WrongOpticalPath,
    SwappedFrames,
    ExtraSharedMacro,
    AddedTopLevelIcc,
}

#[test]
fn accepts_exact_tiled_sparse_wsi_contract() {
    let path = write_fixture("valid", Mutation::None);
    let validated = validate_wsi_tiled_sparse_file(&path, &identity(), &contract())
        .expect("exact TILED_SPARSE WSI fixture must validate");
    assert_eq!(validated.validation["status"], "passed");
    for finding in [
        "wsi_sparse_dimension_index_items",
        "wsi_sparse_occupancy_mask",
        "wsi_sparse_sentinel_matrix_sha256",
    ] {
        assert!(
            validated.validation["internal"]
                .as_array()
                .unwrap()
                .iter()
                .any(|item| item["name"] == finding && item["status"] == "passed"),
            "missing passing finding {finding}"
        );
    }
    cleanup(path);
}

#[test]
fn rejects_sparse_dimension_position_pixel_and_absence_mutations() {
    for (label, mutation, finding) in [
        (
            "dimension-type",
            Mutation::WrongDimensionType,
            "wsi_sparse_dimension_organization_type",
        ),
        (
            "dimension-order",
            Mutation::ReorderedDimensionIndices,
            "wsi_sparse_dimension_index_1_pointer",
        ),
        (
            "dimension-uid",
            Mutation::WrongDimensionUid,
            "wsi_sparse_dimension_organization_uid",
        ),
        (
            "dimension-values",
            Mutation::WrongDimensionValues,
            "wsi_sparse_frame_2_dimension_index_values",
        ),
        (
            "position-vr",
            Mutation::WrongPositionVr,
            "wsi_sparse_frame_2_position_vrs",
        ),
        (
            "missing-frame-content",
            Mutation::MissingFrameContent,
            "(0020,9111)",
        ),
        (
            "duplicate-position",
            Mutation::DuplicatePosition,
            "wsi_sparse_frame_2_column_position",
        ),
        (
            "off-grid-position",
            Mutation::OffGridPosition,
            "wsi_sparse_frame_2_column_position",
        ),
        (
            "coordinate",
            Mutation::WrongCoordinate,
            "wsi_sparse_frame_2_x_offset",
        ),
        (
            "optical-path",
            Mutation::WrongOpticalPath,
            "wsi_sparse_frame_2_optical_path_identifier",
        ),
        (
            "frame-order",
            Mutation::SwappedFrames,
            "wsi_sparse_frame_1_sha256",
        ),
        (
            "shared-macro",
            Mutation::ExtraSharedMacro,
            "wsi_sparse_shared_macro_set",
        ),
        (
            "top-level-icc",
            Mutation::AddedTopLevelIcc,
            "wsi_sparse_top_level_icc_profile_absent",
        ),
    ] {
        let path = write_fixture(label, mutation);
        let error = validate_wsi_tiled_sparse_file(&path, &identity(), &contract())
            .expect_err("mutated TILED_SPARSE WSI fixture must fail")
            .to_string();
        assert!(error.contains(finding), "{label}: {error}");
        cleanup(path);
    }
}

#[test]
fn rejects_sparse_manifest_contract_drift() {
    let path = write_fixture("contract-drift", Mutation::None);
    let mut expected = contract();
    expected["tiling"]["occupancy_mask"][1] = "present".into();
    let error = validate_wsi_tiled_sparse_file(&path, &identity(), &expected)
        .expect_err("noncanonical sparse manifest expectation must fail")
        .to_string();
    assert!(error.contains("wsi_sparse_expected_contract"), "{error}");
    cleanup(path);
}

#[test]
fn persisted_manifest_validation_rejects_sparse_geometry_mutation() {
    let external = crate::manifest_contract::ManifestContractKind::ExternalCorpus;
    let mut caller = manifest_file();
    caller["case_id"] = "caller/sparse".into();
    caller["dicom"]["iod_name"] = "VL Whole Slide Microscopy Image".into();
    let caller_path = write_fixture("external-renamed", Mutation::None);
    let object = dicom_object::open_file(&caller_path).unwrap();
    let mut failures = vec![];
    crate::validate_family_standard_elements_for_kind(
        external,
        &mut failures,
        "caller.dcm",
        &caller_path,
        std::path::Path::new("manifest.json"),
        &caller,
        &object,
    )
    .unwrap();
    assert!(failures.is_empty(), "{failures:?}");
    assert!(validate_manifest_wsi_file(&caller_path, &caller).is_err());
    let mut missing = caller.clone();
    missing
        .as_object_mut()
        .unwrap()
        .remove("expected_wsi_tiled_sparse");
    assert!(super::validate_manifest_wsi_file_for_kind(external, &caller_path, &missing).is_err());
    let mut crossed = caller.clone();
    crossed["expected_wsi_tiled_full"] = serde_json::json!({});
    assert!(super::validate_manifest_wsi_file_for_kind(external, &caller_path, &crossed).is_err());
    cleanup(caller_path);
    let corrupt_path = write_fixture("external-corrupt", Mutation::DuplicatePosition);
    let object = dicom_object::open_file(&corrupt_path).unwrap();
    crate::validate_family_standard_elements_for_kind(
        external,
        &mut failures,
        "caller.dcm",
        &corrupt_path,
        std::path::Path::new("manifest.json"),
        &caller,
        &object,
    )
    .unwrap();
    assert!(
        failures
            .iter()
            .any(|f| f.contains("wsi_sparse_frame_2_column_position")),
        "{failures:?}"
    );
    cleanup(corrupt_path);

    let valid_path = write_fixture("persisted-valid", Mutation::None);
    validate_manifest_wsi_file(&valid_path, &manifest_file())
        .expect("persisted valid sparse WSI must pass strict validation");
    cleanup(valid_path);

    let mutated_path = write_fixture("persisted-duplicate", Mutation::DuplicatePosition);
    let error = validate_manifest_wsi_file(&mutated_path, &manifest_file())
        .expect_err("persisted duplicate sparse position must fail")
        .to_string();
    assert!(
        error.contains("wsi_sparse_frame_2_column_position"),
        "{error}"
    );
    cleanup(mutated_path);
}

fn manifest_file() -> serde_json::Value {
    serde_json::json!({
        "case_id": "vl/wsi/tiled_sparse_small",
        "dicom": {
            "sop_class_uid": "1.2.840.10008.5.1.4.1.1.77.1.6",
            "transfer_syntax_uid": uids::EXPLICIT_VR_LITTLE_ENDIAN
        },
        "uids": {
            "sop_instance_uid": SOP_UID,
            "implementation_class_uid": IMPLEMENTATION_UID
        },
        "image": {
            "rows": 2,
            "columns": 2,
            "frames": 2,
            "samples_per_pixel": 3,
            "photometric_interpretation": "RGB",
            "bits_allocated": 8,
            "bits_stored": 8,
            "high_bit": 7,
            "pixel_representation": 0,
            "planar_configuration": 0
        },
        "pixel_data": {"vr": "OB", "frame_hashes": []},
        "expected_semantics": {"synthetic_data": "YES"},
        "expected_wsi_tiled_sparse": contract()
    })
}

fn identity() -> Part10Expectations<'static> {
    Part10Expectations {
        sop_class_uid: "1.2.840.10008.5.1.4.1.1.77.1.6",
        sop_instance_uid: SOP_UID,
        transfer_syntax_uid: uids::EXPLICIT_VR_LITTLE_ENDIAN,
        implementation_class_uid: IMPLEMENTATION_UID,
        synthetic_data: "YES",
        rows: 2,
        columns: 2,
        frames: 2,
        samples_per_pixel: 3,
        photometric_interpretation: "RGB",
        bits_allocated: 8,
        bits_stored: 8,
        high_bit: 7,
        pixel_representation: 0,
        planar_configuration: Some(0),
        pixel_data_vr: VR::OB,
        pixel_data_length_formula: PixelDataLengthFormula::ContiguousSamples,
        decoded_frame_hashes: &[],
        palette: None,
        padding: None,
        ct_image: None,
        enhanced_ct_image: None,
        enhanced_mr_image: None,
        enhanced_pet_image: None,
        mg_image: None,
        dx_image: None,
        xa_image: None,
        xrf_image: None,
        us_image: None,
        us_multiframe: None,
        nm_image: None,
        pet_image: None,
        cr_image: None,
        mr_image: None,
        segmentation: None,
    }
}

fn contract() -> serde_json::Value {
    crate::wsi_tiled_sparse_locked_contract(FOR_UID, SPECIMEN_UID, DIMENSION_UID)
}

fn write_fixture(label: &str, mutation: Mutation) -> PathBuf {
    let object = valid_object(mutation);
    let path = std::env::temp_dir().join(format!(
        "dicom-test-suite-wsi-sparse-validation-{}-{label}.dcm",
        std::process::id()
    ));
    object
        .with_meta(
            FileMetaTableBuilder::new()
                .media_storage_sop_class_uid("1.2.840.10008.5.1.4.1.1.77.1.6")
                .media_storage_sop_instance_uid(SOP_UID)
                .transfer_syntax(uids::EXPLICIT_VR_LITTLE_ENDIAN)
                .implementation_class_uid(IMPLEMENTATION_UID),
        )
        .unwrap()
        .write_to_file(&path)
        .unwrap();
    path
}

fn valid_object(mutation: Mutation) -> InMemDicomObject {
    let mut obj = wsi_tiled_full_tests::valid_object(wsi_tiled_full_tests::Mutation::None);
    put_str(&mut obj, tags::SOP_INSTANCE_UID, VR::UI, SOP_UID);
    put_str(&mut obj, tags::FRAME_OF_REFERENCE_UID, VR::UI, FOR_UID);
    put_str(
        &mut obj,
        tags::DIMENSION_ORGANIZATION_TYPE,
        VR::CS,
        if matches!(mutation, Mutation::WrongDimensionType) {
            "TILED_FULL"
        } else {
            "TILED_SPARSE"
        },
    );
    put_str(&mut obj, tags::NUMBER_OF_FRAMES, VR::IS, "2");

    let organization_uid = if matches!(mutation, Mutation::WrongDimensionUid) {
        "2.25.9999"
    } else {
        DIMENSION_UID
    };
    let mut organization = InMemDicomObject::new_empty();
    put_str(
        &mut organization,
        tags::DIMENSION_ORGANIZATION_UID,
        VR::UI,
        organization_uid,
    );
    put_sequence(
        &mut obj,
        tags::DIMENSION_ORGANIZATION_SEQUENCE,
        vec![organization],
    );

    let mut dimensions = [
        dimension_item(
            tags::COLUMN_POSITION_IN_TOTAL_IMAGE_PIXEL_MATRIX,
            "Column Position",
        ),
        dimension_item(
            tags::ROW_POSITION_IN_TOTAL_IMAGE_PIXEL_MATRIX,
            "Row Position",
        ),
    ];
    if matches!(mutation, Mutation::ReorderedDimensionIndices) {
        dimensions.swap(0, 1);
    }
    put_sequence(
        &mut obj,
        tags::DIMENSION_INDEX_SEQUENCE,
        dimensions.into_iter().collect(),
    );

    let frame_1 = per_frame_item(1, mutation);
    let frame_2 = per_frame_item(2, mutation);
    put_sequence(
        &mut obj,
        tags::PER_FRAME_FUNCTIONAL_GROUPS_SEQUENCE,
        vec![frame_1, frame_2],
    );

    if matches!(mutation, Mutation::ExtraSharedMacro) {
        let mut shared = obj
            .element(tags::SHARED_FUNCTIONAL_GROUPS_SEQUENCE)
            .unwrap()
            .items()
            .unwrap()[0]
            .clone();
        put_sequence(&mut shared, tags::DERIVATION_IMAGE_SEQUENCE, vec![]);
        put_sequence(
            &mut obj,
            tags::SHARED_FUNCTIONAL_GROUPS_SEQUENCE,
            vec![shared],
        );
    }

    let mut pixels = [RED, WHITE].concat();
    if matches!(mutation, Mutation::SwappedFrames) {
        let (first, second) = pixels.split_at_mut(12);
        first.swap_with_slice(second);
    }
    obj.put(DataElement::new(
        tags::PIXEL_DATA,
        VR::OB,
        PrimitiveValue::U8(pixels.into()),
    ));
    if matches!(mutation, Mutation::AddedTopLevelIcc) {
        obj.put(DataElement::new(
            tags::ICC_PROFILE,
            VR::OB,
            PrimitiveValue::U8(vec![0_u8; 128].into()),
        ));
    }
    obj
}

fn dimension_item(pointer: dicom_core::Tag, label: &str) -> InMemDicomObject {
    let mut item = InMemDicomObject::new_empty();
    item.put(DataElement::new(
        tags::DIMENSION_INDEX_POINTER,
        VR::AT,
        PrimitiveValue::Tags(vec![pointer].into()),
    ));
    item.put(DataElement::new(
        tags::FUNCTIONAL_GROUP_POINTER,
        VR::AT,
        PrimitiveValue::Tags(vec![tags::PLANE_POSITION_SLIDE_SEQUENCE].into()),
    ));
    put_str(
        &mut item,
        tags::DIMENSION_ORGANIZATION_UID,
        VR::UI,
        DIMENSION_UID,
    );
    put_str(&mut item, tags::DIMENSION_DESCRIPTION_LABEL, VR::LO, label);
    item
}

fn per_frame_item(frame_number: usize, mutation: Mutation) -> InMemDicomObject {
    let mut frame = InMemDicomObject::new_empty();
    if !(frame_number == 2 && matches!(mutation, Mutation::MissingFrameContent)) {
        let mut content = InMemDicomObject::new_empty();
        let values = if frame_number == 2 && matches!(mutation, Mutation::WrongDimensionValues) {
            vec![1_u32, 2]
        } else {
            vec![frame_number as u32, frame_number as u32]
        };
        content.put(DataElement::new(
            tags::DIMENSION_INDEX_VALUES,
            VR::UL,
            PrimitiveValue::U32(values.into()),
        ));
        put_sequence(&mut frame, tags::FRAME_CONTENT_SEQUENCE, vec![content]);
    }

    let mut column = if frame_number == 1 { 1 } else { 3 };
    let row = if frame_number == 1 { 1 } else { 3 };
    if frame_number == 2 && matches!(mutation, Mutation::DuplicatePosition) {
        column = 1;
    } else if frame_number == 2 && matches!(mutation, Mutation::OffGridPosition) {
        column = 2;
    }
    let mut plane = InMemDicomObject::new_empty();
    if frame_number == 2 && matches!(mutation, Mutation::WrongPositionVr) {
        put_u32(
            &mut plane,
            tags::COLUMN_POSITION_IN_TOTAL_IMAGE_PIXEL_MATRIX,
            column,
        );
        put_u32(
            &mut plane,
            tags::ROW_POSITION_IN_TOTAL_IMAGE_PIXEL_MATRIX,
            row,
        );
    } else {
        put_i32(
            &mut plane,
            tags::COLUMN_POSITION_IN_TOTAL_IMAGE_PIXEL_MATRIX,
            column as i32,
        );
        put_i32(
            &mut plane,
            tags::ROW_POSITION_IN_TOTAL_IMAGE_PIXEL_MATRIX,
            row as i32,
        );
    }
    let x = if frame_number == 1 {
        "0"
    } else if matches!(mutation, Mutation::WrongCoordinate) {
        "2"
    } else {
        "1"
    };
    put_str(
        &mut plane,
        tags::X_OFFSET_IN_SLIDE_COORDINATE_SYSTEM,
        VR::DS,
        x,
    );
    put_str(
        &mut plane,
        tags::Y_OFFSET_IN_SLIDE_COORDINATE_SYSTEM,
        VR::DS,
        if frame_number == 1 { "0" } else { "1" },
    );
    put_str(
        &mut plane,
        tags::Z_OFFSET_IN_SLIDE_COORDINATE_SYSTEM,
        VR::DS,
        "0",
    );
    put_sequence(&mut frame, tags::PLANE_POSITION_SLIDE_SEQUENCE, vec![plane]);

    let mut optical = InMemDicomObject::new_empty();
    put_str(
        &mut optical,
        tags::OPTICAL_PATH_IDENTIFIER,
        VR::SH,
        if frame_number == 2 && matches!(mutation, Mutation::WrongOpticalPath) {
            "OTHER"
        } else {
            "RGB"
        },
    );
    put_sequence(
        &mut frame,
        tags::OPTICAL_PATH_IDENTIFICATION_SEQUENCE,
        vec![optical],
    );
    frame
}

fn put_str(obj: &mut InMemDicomObject, tag: dicom_core::Tag, vr: VR, value: &str) {
    obj.put(DataElement::new(tag, vr, value));
}

fn put_u32(obj: &mut InMemDicomObject, tag: dicom_core::Tag, value: u32) {
    obj.put(DataElement::new(tag, VR::UL, PrimitiveValue::from(value)));
}

fn put_i32(obj: &mut InMemDicomObject, tag: dicom_core::Tag, value: i32) {
    obj.put(DataElement::new(tag, VR::SL, PrimitiveValue::from(value)));
}

fn put_sequence(obj: &mut InMemDicomObject, tag: dicom_core::Tag, items: Vec<InMemDicomObject>) {
    obj.put(DataElement::new(tag, VR::SQ, DataSetSequence::from(items)));
}

fn cleanup(path: PathBuf) {
    let _ = fs::remove_file(path);
}

#[path = "validation_wsi_reduced_reader_tests.rs"]
mod reduced_reader_tests;
