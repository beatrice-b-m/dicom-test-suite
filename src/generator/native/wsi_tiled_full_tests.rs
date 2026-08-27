use dicom_core::Tag;
use dicom_dictionary_std::tags;

use super::{
    icc_profile::ICC_PROFILE_BYTES,
    wsi_tiled_full::{
        WSI_FRAME_SHA256, WSI_NUMBER_OF_FRAMES, WSI_PIXEL_BYTES, WSI_PIXEL_DATA_SHA256,
        WSI_TOTAL_PIXEL_MATRIX_SHA256, WsiTiledFullInput, build_wsi_tiled_full,
        reconstructed_total_pixel_matrix,
    },
};
use crate::sha256_hex;

fn input() -> WsiTiledFullInput<'static> {
    WsiTiledFullInput {
        study_instance_uid: "1.2.826.0.1.3680043.10.543.1",
        series_instance_uid: "1.2.826.0.1.3680043.10.543.2",
        sop_instance_uid: "1.2.826.0.1.3680043.10.543.3",
        frame_of_reference_uid: "1.2.826.0.1.3680043.10.543.4",
        dimension_organization_uid: "1.2.826.0.1.3680043.10.543.5",
        specimen_uid: "1.2.826.0.1.3680043.10.543.6",
    }
}

#[test]
fn tiled_full_wsi_builds_locked_geometry_modules_and_implicit_order() {
    let object = build_wsi_tiled_full(input()).unwrap();

    assert_eq!(
        object
            .element(tags::NUMBER_OF_FRAMES)
            .unwrap()
            .to_int::<u16>()
            .unwrap(),
        WSI_NUMBER_OF_FRAMES
    );
    assert_eq!(
        object
            .element(tags::TOTAL_PIXEL_MATRIX_ROWS)
            .unwrap()
            .to_int::<u32>()
            .unwrap(),
        4
    );
    assert_eq!(
        object
            .element(tags::TOTAL_PIXEL_MATRIX_COLUMNS)
            .unwrap()
            .to_int::<u32>()
            .unwrap(),
        4
    );
    assert_eq!(
        object
            .element(tags::DIMENSION_ORGANIZATION_TYPE)
            .unwrap()
            .to_str()
            .unwrap(),
        "TILED_FULL"
    );
    assert!(object.element(tags::DIMENSION_INDEX_SEQUENCE).is_err());
    assert!(
        object
            .element(tags::PER_FRAME_FUNCTIONAL_GROUPS_SEQUENCE)
            .is_err()
    );

    let shared = object
        .element(tags::SHARED_FUNCTIONAL_GROUPS_SEQUENCE)
        .unwrap()
        .items()
        .unwrap();
    assert_eq!(shared.len(), 1);
    assert!(shared[0].element(tags::PIXEL_MEASURES_SEQUENCE).is_ok());
    assert!(
        shared[0]
            .element(tags::WHOLE_SLIDE_MICROSCOPY_IMAGE_FRAME_TYPE_SEQUENCE)
            .is_ok()
    );

    let optical_paths = object
        .element(tags::OPTICAL_PATH_SEQUENCE)
        .unwrap()
        .items()
        .unwrap();
    assert_eq!(optical_paths.len(), 1);
    assert_eq!(
        optical_paths[0]
            .element(tags::ICC_PROFILE)
            .unwrap()
            .to_bytes()
            .unwrap()
            .as_ref(),
        ICC_PROFILE_BYTES
    );
    assert_eq!(
        object
            .element(tags::SPECIMEN_DESCRIPTION_SEQUENCE)
            .unwrap()
            .items()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        object.element(tags::LABEL_TEXT).unwrap().to_str().unwrap(),
        "DTS SYNTHETIC SLIDE 001"
    );
}

#[test]
fn tiled_full_wsi_locks_frame_and_reconstructed_matrix_hashes() {
    assert_eq!(sha256_hex(&WSI_PIXEL_BYTES), WSI_PIXEL_DATA_SHA256);
    for (frame, expected_hash) in WSI_PIXEL_BYTES.chunks_exact(12).zip(WSI_FRAME_SHA256) {
        assert_eq!(sha256_hex(frame), expected_hash);
    }
    assert_eq!(
        sha256_hex(&reconstructed_total_pixel_matrix()),
        WSI_TOTAL_PIXEL_MATRIX_SHA256
    );
}

#[test]
fn tiled_full_wsi_rejects_invalid_or_colliding_uids() {
    let mut invalid = input();
    invalid.specimen_uid = "1.02.3";
    assert!(
        build_wsi_tiled_full(invalid)
            .unwrap_err()
            .contains("Specimen UID")
    );

    let mut collision = input();
    collision.specimen_uid = collision.sop_instance_uid;
    assert!(
        build_wsi_tiled_full(collision)
            .unwrap_err()
            .contains("must be distinct")
    );
}

#[test]
fn standard_wsi_tags_use_expected_numeric_identifiers() {
    assert_eq!(tags::DIMENSION_ORGANIZATION_TYPE, Tag(0x0020, 0x9311));
    assert_eq!(
        tags::TOTAL_PIXEL_MATRIX_ORIGIN_SEQUENCE,
        Tag(0x0048, 0x0008)
    );
    assert_eq!(tags::OPTICAL_PATH_SEQUENCE, Tag(0x0048, 0x0105));
}
