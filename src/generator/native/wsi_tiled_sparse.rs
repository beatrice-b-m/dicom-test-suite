use dicom_core::{DataElement, PrimitiveValue, VR, value::DataSetSequence};
use dicom_dictionary_std::tags;
use dicom_object::InMemDicomObject;

use super::wsi_tiled_full::{
    WSI_TILE_COLUMNS, WSI_TILE_ROWS, WSI_TILED_FULL_STORAGE_UID, WSI_TOTAL_PIXEL_MATRIX_COLUMNS,
    WsiTiledFullInput, build_wsi_tiled_full,
};

pub(in crate::generator) const WSI_TILED_SPARSE_CASE_ID: &str = "vl/wsi/tiled_sparse_small";
pub(in crate::generator) const WSI_TILED_SPARSE_RECIPE_ID: &str = "vl_wsi_tiled_sparse_small";
pub(in crate::generator) const WSI_TILED_SPARSE_RECIPE_VERSION: &str = "0.1.0";
pub(in crate::generator) const WSI_TILED_SPARSE_STORAGE_UID: &str = WSI_TILED_FULL_STORAGE_UID;
pub(in crate::generator) const WSI_TILED_SPARSE_OUTPUT_FILE: &str = "instance.dcm";
pub(in crate::generator) const WSI_SPARSE_NUMBER_OF_FRAMES: u16 = 2;
pub(in crate::generator) const WSI_SPARSE_PIXEL_DATA_SHA256: &str =
    "94a57aca44c4a97d424e8e546b2673fa91f711694de1ccb36f062aabbc9b55ee";
pub(in crate::generator) const WSI_SPARSE_TOTAL_PIXEL_MATRIX_SHA256: &str =
    "d10a587875f14a0b74a9e4935ce83cdb73377bd7357a172db8e9f7347c030eb3";
pub(in crate::generator) const WSI_SPARSE_FRAME_SHA256: [&str; 2] = [
    "fcf067f6323bb42b8292a565a8f826ec5fdb1b142b7a69bf7f7721f0d5d46ef8",
    "8688d249e9d047b4fc2fb89ce05afe9ec89252ffccdd969de6eef260dd7ffb21",
];
pub(in crate::generator) const WSI_SPARSE_OCCUPANCY: [bool; 4] = [true, false, false, true];
pub(in crate::generator) const WSI_SPARSE_PIXEL_BYTES: [u8; 24] = sparse_pixel_bytes();

pub(in crate::generator) type WsiTiledSparseInput<'a> = WsiTiledFullInput<'a>;

pub(in crate::generator) fn build_wsi_tiled_sparse(
    input: WsiTiledSparseInput<'_>,
) -> Result<InMemDicomObject, String> {
    let dimension_organization_uid = input.dimension_organization_uid;
    let mut object = build_wsi_tiled_full(input)?;

    object.put(DataElement::new(tags::SERIES_NUMBER, VR::IS, "42"));
    object.put(DataElement::new(
        tags::MANUFACTURER_MODEL_NAME,
        VR::LO,
        "Native TILED_SPARSE WSI",
    ));
    object.put(DataElement::new(
        tags::NUMBER_OF_FRAMES,
        VR::IS,
        WSI_SPARSE_NUMBER_OF_FRAMES.to_string(),
    ));
    object.put(DataElement::new(
        tags::DIMENSION_ORGANIZATION_TYPE,
        VR::CS,
        "TILED_SPARSE",
    ));
    put_dimension_indices(&mut object, dimension_organization_uid);
    put_per_frame_functional_groups(&mut object);
    object.put(DataElement::new(
        tags::PIXEL_DATA,
        VR::OB,
        PrimitiveValue::from(WSI_SPARSE_PIXEL_BYTES.as_slice()),
    ));

    Ok(object)
}

pub(in crate::generator) fn reconstructed_sparse_total_pixel_matrix() -> [u8; 48] {
    let mut matrix = [0_u8; 48];
    for (frame_index, (tile_column, tile_row)) in
        [(0_usize, 0_usize), (1, 1)].into_iter().enumerate()
    {
        for row in 0..usize::from(WSI_TILE_ROWS) {
            let source = frame_index * 12 + row * usize::from(WSI_TILE_COLUMNS) * 3;
            let destination = ((tile_row * usize::from(WSI_TILE_ROWS) + row)
                * WSI_TOTAL_PIXEL_MATRIX_COLUMNS as usize
                + tile_column * usize::from(WSI_TILE_COLUMNS))
                * 3;
            matrix[destination..destination + usize::from(WSI_TILE_COLUMNS) * 3].copy_from_slice(
                &WSI_SPARSE_PIXEL_BYTES[source..source + usize::from(WSI_TILE_COLUMNS) * 3],
            );
        }
    }
    matrix
}

fn put_dimension_indices(object: &mut InMemDicomObject, uid: &str) {
    let item = |index_pointer, label| {
        InMemDicomObject::from_element_iter([
            DataElement::new(
                tags::DIMENSION_INDEX_POINTER,
                VR::AT,
                PrimitiveValue::Tags(vec![index_pointer].into()),
            ),
            DataElement::new(
                tags::FUNCTIONAL_GROUP_POINTER,
                VR::AT,
                PrimitiveValue::Tags(vec![tags::PLANE_POSITION_SLIDE_SEQUENCE].into()),
            ),
            DataElement::new(tags::DIMENSION_ORGANIZATION_UID, VR::UI, uid),
            DataElement::new(tags::DIMENSION_DESCRIPTION_LABEL, VR::LO, label),
        ])
    };
    object.put(DataElement::new(
        tags::DIMENSION_INDEX_SEQUENCE,
        VR::SQ,
        DataSetSequence::from(vec![
            item(
                tags::COLUMN_POSITION_IN_TOTAL_IMAGE_PIXEL_MATRIX,
                "Column Position",
            ),
            item(
                tags::ROW_POSITION_IN_TOTAL_IMAGE_PIXEL_MATRIX,
                "Row Position",
            ),
        ]),
    ));
}

fn put_per_frame_functional_groups(object: &mut InMemDicomObject) {
    let frame = |dimension_values: [u32; 2], column: i32, row: i32, x: &str, y: &str| {
        let frame_content = InMemDicomObject::from_element_iter([DataElement::new(
            tags::DIMENSION_INDEX_VALUES,
            VR::UL,
            PrimitiveValue::U32(dimension_values.to_vec().into()),
        )]);
        let plane_position = InMemDicomObject::from_element_iter([
            DataElement::new(
                tags::COLUMN_POSITION_IN_TOTAL_IMAGE_PIXEL_MATRIX,
                VR::SL,
                PrimitiveValue::from(column),
            ),
            DataElement::new(
                tags::ROW_POSITION_IN_TOTAL_IMAGE_PIXEL_MATRIX,
                VR::SL,
                PrimitiveValue::from(row),
            ),
            DataElement::new(tags::X_OFFSET_IN_SLIDE_COORDINATE_SYSTEM, VR::DS, x),
            DataElement::new(tags::Y_OFFSET_IN_SLIDE_COORDINATE_SYSTEM, VR::DS, y),
            DataElement::new(tags::Z_OFFSET_IN_SLIDE_COORDINATE_SYSTEM, VR::DS, "0"),
        ]);
        let optical_path = InMemDicomObject::from_element_iter([DataElement::new(
            tags::OPTICAL_PATH_IDENTIFIER,
            VR::SH,
            "RGB",
        )]);
        InMemDicomObject::from_element_iter([
            DataElement::new(
                tags::FRAME_CONTENT_SEQUENCE,
                VR::SQ,
                DataSetSequence::from(vec![frame_content]),
            ),
            DataElement::new(
                tags::PLANE_POSITION_SLIDE_SEQUENCE,
                VR::SQ,
                DataSetSequence::from(vec![plane_position]),
            ),
            DataElement::new(
                tags::OPTICAL_PATH_IDENTIFICATION_SEQUENCE,
                VR::SQ,
                DataSetSequence::from(vec![optical_path]),
            ),
        ])
    };
    object.put(DataElement::new(
        tags::PER_FRAME_FUNCTIONAL_GROUPS_SEQUENCE,
        VR::SQ,
        DataSetSequence::from(vec![
            frame([1, 1], 1, 1, "0", "0"),
            frame([2, 2], 3, 3, "1", "1"),
        ]),
    ));
}

const fn sparse_pixel_bytes() -> [u8; 24] {
    let mut bytes = [0_u8; 24];
    let mut pixel = 0;
    while pixel < 4 {
        bytes[pixel * 3] = 255;
        pixel += 1;
    }
    while pixel < 8 {
        bytes[pixel * 3] = 255;
        bytes[pixel * 3 + 1] = 255;
        bytes[pixel * 3 + 2] = 255;
        pixel += 1;
    }
    bytes
}
