use dicom_core::{Tag, value::PrimitiveValue};
use dicom_dictionary_std::tags;

use super::wsi_tiled_sparse::{
    WSI_SPARSE_FRAME_SHA256, WSI_SPARSE_NUMBER_OF_FRAMES, WSI_SPARSE_OCCUPANCY,
    WSI_SPARSE_PIXEL_BYTES, WSI_SPARSE_PIXEL_DATA_SHA256, WSI_SPARSE_TOTAL_PIXEL_MATRIX_SHA256,
    WsiTiledSparseInput, build_wsi_tiled_sparse, reconstructed_sparse_total_pixel_matrix,
};
use crate::sha256_hex;

fn input() -> WsiTiledSparseInput<'static> {
    WsiTiledSparseInput {
        study_instance_uid: "1.2.826.0.1.3680043.10.543.11",
        series_instance_uid: "1.2.826.0.1.3680043.10.543.12",
        sop_instance_uid: "1.2.826.0.1.3680043.10.543.13",
        frame_of_reference_uid: "1.2.826.0.1.3680043.10.543.14",
        dimension_organization_uid: "1.2.826.0.1.3680043.10.543.15",
        specimen_uid: "1.2.826.0.1.3680043.10.543.16",
    }
}

#[test]
fn tiled_sparse_wsi_builds_explicit_dimensions_and_per_frame_positions() {
    let object = build_wsi_tiled_sparse(input()).unwrap();
    assert_eq!(
        object
            .element(tags::NUMBER_OF_FRAMES)
            .unwrap()
            .to_int::<u16>()
            .unwrap(),
        WSI_SPARSE_NUMBER_OF_FRAMES
    );
    assert_eq!(
        object
            .element(tags::DIMENSION_ORGANIZATION_TYPE)
            .unwrap()
            .to_str()
            .unwrap(),
        "TILED_SPARSE"
    );

    let dimensions = object
        .element(tags::DIMENSION_INDEX_SEQUENCE)
        .unwrap()
        .items()
        .unwrap();
    assert_eq!(dimensions.len(), 2);
    assert_eq!(
        dimensions[0]
            .element(tags::DIMENSION_DESCRIPTION_LABEL)
            .unwrap()
            .to_str()
            .unwrap(),
        "Column Position"
    );
    assert_eq!(
        dimensions[1]
            .element(tags::DIMENSION_DESCRIPTION_LABEL)
            .unwrap()
            .to_str()
            .unwrap(),
        "Row Position"
    );
    assert_eq!(
        attribute_tag(&dimensions[0], tags::DIMENSION_INDEX_POINTER),
        tags::COLUMN_POSITION_IN_TOTAL_IMAGE_PIXEL_MATRIX
    );
    assert_eq!(
        attribute_tag(&dimensions[1], tags::DIMENSION_INDEX_POINTER),
        tags::ROW_POSITION_IN_TOTAL_IMAGE_PIXEL_MATRIX
    );
    for dimension in dimensions {
        assert_eq!(
            attribute_tag(dimension, tags::FUNCTIONAL_GROUP_POINTER),
            tags::PLANE_POSITION_SLIDE_SEQUENCE
        );
    }

    let per_frame = object
        .element(tags::PER_FRAME_FUNCTIONAL_GROUPS_SEQUENCE)
        .unwrap()
        .items()
        .unwrap();
    assert_eq!(per_frame.len(), 2);
    assert_eq!(per_frame[0].iter().count(), 3);
    assert_eq!(per_frame[1].iter().count(), 3);
    assert_frame(&per_frame[0], &[1, 1], 1, 1, "0", "0");
    assert_frame(&per_frame[1], &[2, 2], 3, 3, "1", "1");

    let shared = object
        .element(tags::SHARED_FUNCTIONAL_GROUPS_SEQUENCE)
        .unwrap()
        .items()
        .unwrap();
    assert!(shared[0].element(tags::FRAME_CONTENT_SEQUENCE).is_err());
    assert_eq!(WSI_SPARSE_OCCUPANCY, [true, false, false, true]);
}

#[test]
fn tiled_sparse_wsi_locks_frames_payload_and_sentinel_reconstruction() {
    assert_eq!(
        sha256_hex(&WSI_SPARSE_PIXEL_BYTES),
        WSI_SPARSE_PIXEL_DATA_SHA256
    );
    for (frame, expected_hash) in WSI_SPARSE_PIXEL_BYTES
        .chunks_exact(12)
        .zip(WSI_SPARSE_FRAME_SHA256)
    {
        assert_eq!(sha256_hex(frame), expected_hash);
    }
    let matrix = reconstructed_sparse_total_pixel_matrix();
    assert_eq!(sha256_hex(&matrix), WSI_SPARSE_TOTAL_PIXEL_MATRIX_SHA256);
    assert_eq!(&matrix[6..12], &[0; 6]);
    assert_eq!(&matrix[24..30], &[0; 6]);
}

#[test]
fn tiled_sparse_wsi_reuses_strict_uid_rejection() {
    let mut invalid = input();
    invalid.dimension_organization_uid = "1.02.3";
    assert!(
        build_wsi_tiled_sparse(invalid)
            .unwrap_err()
            .contains("Dimension Organization UID")
    );
}

fn assert_frame(
    frame: &dicom_object::InMemDicomObject,
    dimension_values: &[u32],
    column: i32,
    row: i32,
    x: &str,
    y: &str,
) {
    let content = one_item(frame, tags::FRAME_CONTENT_SEQUENCE);
    assert_eq!(
        content
            .element(tags::DIMENSION_INDEX_VALUES)
            .unwrap()
            .to_multi_int::<u32>()
            .unwrap(),
        dimension_values
    );
    let position = one_item(frame, tags::PLANE_POSITION_SLIDE_SEQUENCE);
    assert_eq!(
        position
            .element(tags::COLUMN_POSITION_IN_TOTAL_IMAGE_PIXEL_MATRIX)
            .unwrap()
            .to_int::<i32>()
            .unwrap(),
        column
    );
    assert_eq!(
        position
            .element(tags::ROW_POSITION_IN_TOTAL_IMAGE_PIXEL_MATRIX)
            .unwrap()
            .to_int::<i32>()
            .unwrap(),
        row
    );
    assert_eq!(
        position
            .element(tags::X_OFFSET_IN_SLIDE_COORDINATE_SYSTEM)
            .unwrap()
            .to_str()
            .unwrap(),
        x
    );
    assert_eq!(
        position
            .element(tags::Y_OFFSET_IN_SLIDE_COORDINATE_SYSTEM)
            .unwrap()
            .to_str()
            .unwrap(),
        y
    );
    assert_eq!(
        position
            .element(tags::Z_OFFSET_IN_SLIDE_COORDINATE_SYSTEM)
            .unwrap()
            .to_str()
            .unwrap(),
        "0"
    );
    let optical_path = one_item(frame, tags::OPTICAL_PATH_IDENTIFICATION_SEQUENCE);
    assert_eq!(
        optical_path
            .element(tags::OPTICAL_PATH_IDENTIFIER)
            .unwrap()
            .to_str()
            .unwrap(),
        "RGB"
    );
}

fn one_item<'a>(
    object: &'a dicom_object::InMemDicomObject,
    tag: Tag,
) -> &'a dicom_object::InMemDicomObject {
    let items = object.element(tag).unwrap().items().unwrap();
    assert_eq!(items.len(), 1);
    &items[0]
}

fn attribute_tag(object: &dicom_object::InMemDicomObject, tag: Tag) -> Tag {
    match object.element(tag).unwrap().value() {
        dicom_core::value::Value::Primitive(PrimitiveValue::Tags(values)) => values[0],
        other => panic!("expected AT value, got {other:?}"),
    }
}
