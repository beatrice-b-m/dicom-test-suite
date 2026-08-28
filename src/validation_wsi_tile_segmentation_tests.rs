use std::path::Path;

use dicom_core::{DataElement, PrimitiveValue, VR, value::DataSetSequence};
use dicom_dictionary_std::{tags, uids};
use dicom_object::{FileMetaTableBuilder, InMemDicomObject};

use super::{
    TAG_REFERENCED_SEGMENT_NUMBER, TAG_SEGMENT_IDENTIFICATION_SEQUENCE, fail_if_any_failed,
    reconstruct_wsi_tile_segmentation_matrix, validate_wsi_tile_segmentation_absences,
    validate_wsi_tile_segmentation_shared,
};
use crate::sha256_hex;

const PAYLOAD: [u8; 8] = [255, 0, 0, 255, 0, 255, 255, 0];
const MATRIX_HASH: &str = "a8ec6f910c0fb02685163a3251bed92517d1016c9173f1e4f021e6b4194f2467";

#[test]
fn reconstructs_locked_zero_filled_total_pixel_matrix() {
    let matrix = reconstruct_wsi_tile_segmentation_matrix(&PAYLOAD, &[(1, 1), (3, 3)])
        .expect("locked diagonal tile positions reconstruct");

    assert_eq!(
        matrix,
        [255, 0, 0, 0, 0, 255, 0, 0, 0, 0, 0, 255, 0, 0, 255, 0]
    );
    assert_eq!(sha256_hex(&matrix), MATRIX_HASH);
}

#[test]
fn one_byte_payload_mutation_changes_the_reconstruction_hash() {
    let mut mutated = PAYLOAD;
    mutated[1] = 1;

    let matrix = reconstruct_wsi_tile_segmentation_matrix(&mutated, &[(1, 1), (3, 3)])
        .expect("shape remains reconstructable");
    assert_ne!(sha256_hex(&matrix), MATRIX_HASH);
}

#[test]
fn rejects_truncated_duplicate_and_off_grid_tiles() {
    assert!(reconstruct_wsi_tile_segmentation_matrix(&PAYLOAD[..7], &[(1, 1), (3, 3)]).is_none());
    assert!(reconstruct_wsi_tile_segmentation_matrix(&PAYLOAD, &[(1, 1), (1, 1)]).is_none());
    assert!(reconstruct_wsi_tile_segmentation_matrix(&PAYLOAD, &[(1, 1), (2, 3)]).is_none());
}

#[test]
fn rejects_extra_pixel_measures_attribute_in_parseable_dicom() {
    let source = source_object();
    let segmentation = segmentation_with_shared_groups(true);
    let mut findings = Vec::new();

    validate_wsi_tile_segmentation_shared(
        Path::new("segmentation.dcm"),
        &segmentation,
        &source,
        Path::new("source.dcm"),
        &mut findings,
    )
    .expect("parseable functional groups can be inspected");
    let error = fail_if_any_failed(Path::new("segmentation.dcm"), &findings)
        .expect_err("extra Pixel Measures attribute must fail strict validation");
    assert!(error.to_string().contains("wsi_tile_seg_pixel_measures_attributes"));
}

#[test]
fn rejects_newly_locked_pyramid_and_concatenation_absences() {
    for tag in [
        tags::IN_CONCATENATION_TOTAL_NUMBER,
        tags::PYRAMID_LABEL,
        tags::PYRAMID_DESCRIPTION,
    ] {
        let value = if tag == tags::IN_CONCATENATION_TOTAL_NUMBER {
            PrimitiveValue::from(1_u16)
        } else {
            PrimitiveValue::from("forbidden")
        };
        let vr = if tag == tags::IN_CONCATENATION_TOTAL_NUMBER {
            VR::US
        } else {
            VR::LO
        };
        let object = InMemDicomObject::from_element_iter([DataElement::new(tag, vr, value)])
            .with_meta(
                FileMetaTableBuilder::new()
                    .media_storage_sop_class_uid("1.2.840.10008.5.1.4.1.1.66.4")
                    .media_storage_sop_instance_uid("2.25.1")
                    .transfer_syntax(uids::EXPLICIT_VR_LITTLE_ENDIAN),
            )
            .expect("file object");
        let mut findings = Vec::new();
        validate_wsi_tile_segmentation_absences(&object, &mut findings);
        fail_if_any_failed(Path::new("segmentation.dcm"), &findings)
            .expect_err("locked absence mutation must fail");
    }
}

fn source_object() -> super::OpenedObject {
    let measures = InMemDicomObject::from_element_iter([
        DataElement::new(
            tags::PIXEL_SPACING,
            VR::DS,
            PrimitiveValue::Strs(vec!["0.5".to_string(), "0.5".to_string()].into()),
        ),
        DataElement::new(
            tags::SLICE_THICKNESS,
            VR::DS,
            PrimitiveValue::from("0.001"),
        ),
    ]);
    let shared = InMemDicomObject::from_element_iter([DataElement::new(
        tags::PIXEL_MEASURES_SEQUENCE,
        VR::SQ,
        DataSetSequence::from(vec![measures]),
    )]);
    InMemDicomObject::from_element_iter([DataElement::new(
        tags::SHARED_FUNCTIONAL_GROUPS_SEQUENCE,
        VR::SQ,
        DataSetSequence::from(vec![shared]),
    )])
    .with_meta(
        FileMetaTableBuilder::new()
            .media_storage_sop_class_uid("1.2.840.10008.5.1.4.1.1.77.1.6")
            .media_storage_sop_instance_uid("2.25.2")
            .transfer_syntax(uids::EXPLICIT_VR_LITTLE_ENDIAN),
    )
    .expect("source file object")
}

fn segmentation_with_shared_groups(extra_measure: bool) -> super::OpenedObject {
    let mut measures = vec![
        DataElement::new(
            tags::PIXEL_SPACING,
            VR::DS,
            PrimitiveValue::Strs(vec!["0.5".to_string(), "0.5".to_string()].into()),
        ),
        DataElement::new(
            tags::SLICE_THICKNESS,
            VR::DS,
            PrimitiveValue::from("0.001"),
        ),
    ];
    if extra_measure {
        measures.push(DataElement::new(
            tags::SPACING_BETWEEN_SLICES,
            VR::DS,
            PrimitiveValue::from("0.001"),
        ));
    }
    let shared = InMemDicomObject::from_element_iter([
        DataElement::new(
            tags::PIXEL_MEASURES_SEQUENCE,
            VR::SQ,
            DataSetSequence::from(vec![InMemDicomObject::from_element_iter(measures)]),
        ),
        DataElement::new(
            TAG_SEGMENT_IDENTIFICATION_SEQUENCE,
            VR::SQ,
            DataSetSequence::from(vec![InMemDicomObject::from_element_iter([
                DataElement::new(
                    TAG_REFERENCED_SEGMENT_NUMBER,
                    VR::US,
                    PrimitiveValue::from(1_u16),
                ),
            ])]),
        ),
    ]);
    InMemDicomObject::from_element_iter([DataElement::new(
        tags::SHARED_FUNCTIONAL_GROUPS_SEQUENCE,
        VR::SQ,
        DataSetSequence::from(vec![shared]),
    )])
    .with_meta(
        FileMetaTableBuilder::new()
            .media_storage_sop_class_uid("1.2.840.10008.5.1.4.1.1.66.4")
            .media_storage_sop_instance_uid("2.25.1")
            .transfer_syntax(uids::EXPLICIT_VR_LITTLE_ENDIAN),
    )
    .expect("segmentation file object")
}
