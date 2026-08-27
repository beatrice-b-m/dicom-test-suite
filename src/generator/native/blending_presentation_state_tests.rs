use dicom_dictionary_std::tags;

use crate::sha256_hex;

use super::{
    blending_presentation_state::{
        BLENDING_PRESENTATION_STATE_CONTENT_DESCRIPTION, BLENDING_PRESENTATION_STATE_CONTENT_LABEL,
        BLENDING_PRESENTATION_STATE_CREATION_DATE, BLENDING_PRESENTATION_STATE_CREATION_TIME,
        BLENDING_PRESENTATION_STATE_PALETTE_BYTES, BLENDING_PRESENTATION_STATE_PALETTE_DESCRIPTOR,
        BLENDING_PRESENTATION_STATE_RELATIVE_OPACITY, BLENDING_PRESENTATION_STATE_SERIES_NUMBER,
        BLENDING_PRESENTATION_STATE_STORAGE_UID, BlendingPresentationStateInput,
        BlendingPresentationStateReference, build_blending_presentation_state,
    },
    icc_profile::{ICC_COLOR_SPACE, ICC_PROFILE_BYTES},
};

const CT_IMAGE_STORAGE_UID: &str = "1.2.840.10008.5.1.4.1.1.2";
const STUDY_UID: &str = "2.25.100000000000000000000000000000000000001";
const FRAME_UID: &str = "2.25.100000000000000000000000000000000000002";
const SERIES_1_UID: &str = "2.25.100000000000000000000000000000000000003";
const SERIES_2_UID: &str = "2.25.100000000000000000000000000000000000004";

fn reference(
    series_instance_uid: &'static str,
    sop_instance_uid: &'static str,
) -> BlendingPresentationStateReference<'static> {
    BlendingPresentationStateReference {
        study_instance_uid: STUDY_UID,
        series_instance_uid,
        sop_class_uid: CT_IMAGE_STORAGE_UID,
        sop_instance_uid,
        frame_of_reference_uid: FRAME_UID,
    }
}

fn locked_input() -> BlendingPresentationStateInput<'static> {
    BlendingPresentationStateInput {
        sop_instance_uid: "2.25.100000000000000000000000000000000000009",
        series_instance_uid: "2.25.100000000000000000000000000000000000008",
        sources: [
            [
                reference(SERIES_1_UID, "2.25.100000000000000000000000000000000000005"),
                reference(SERIES_1_UID, "2.25.100000000000000000000000000000000000006"),
            ],
            [
                reference(SERIES_2_UID, "2.25.100000000000000000000000000000000000007"),
                reference(SERIES_2_UID, "2.25.100000000000000000000000000000000000010"),
            ],
        ],
    }
}

#[test]
fn blending_builds_locked_identity_without_frame_module() {
    let input = locked_input();
    let object = build_blending_presentation_state(input).expect("locked input");

    for (tag, expected) in [
        (tags::PATIENT_NAME, "DTS^Synthetic^Patient001"),
        (tags::PATIENT_ID, "DTS-PATIENT-001"),
        (tags::SOP_CLASS_UID, BLENDING_PRESENTATION_STATE_STORAGE_UID),
        (tags::SOP_INSTANCE_UID, input.sop_instance_uid),
        (tags::STUDY_INSTANCE_UID, STUDY_UID),
        (tags::STUDY_ID, "DTS-CT"),
        (tags::SERIES_INSTANCE_UID, input.series_instance_uid),
        (
            tags::SERIES_NUMBER,
            BLENDING_PRESENTATION_STATE_SERIES_NUMBER,
        ),
        (tags::MODALITY, "PR"),
        (tags::LATERALITY, "R"),
        (
            tags::CONTENT_LABEL,
            BLENDING_PRESENTATION_STATE_CONTENT_LABEL,
        ),
        (
            tags::CONTENT_DESCRIPTION,
            BLENDING_PRESENTATION_STATE_CONTENT_DESCRIPTION,
        ),
        (
            tags::PRESENTATION_CREATION_DATE,
            BLENDING_PRESENTATION_STATE_CREATION_DATE,
        ),
        (
            tags::PRESENTATION_CREATION_TIME,
            BLENDING_PRESENTATION_STATE_CREATION_TIME,
        ),
    ] {
        assert_eq!(
            object
                .element(tag)
                .expect("locked attribute")
                .to_str()
                .expect("text"),
            expected
        );
    }

    assert!(object.element(tags::FRAME_OF_REFERENCE_UID).is_err());
    assert!(object.element(tags::POSITION_REFERENCE_INDICATOR).is_err());
}

#[test]
fn blending_emits_two_ordered_complete_instance_sets_with_rescale() {
    let input = locked_input();
    let object = build_blending_presentation_state(input).expect("locked input");
    let blending = object
        .element(tags::BLENDING_SEQUENCE)
        .expect("Blending Sequence")
        .items()
        .expect("items");

    assert_eq!(blending.len(), 2);
    for (series_index, item) in blending.iter().enumerate() {
        assert_eq!(
            item.element(tags::BLENDING_POSITION)
                .expect("position")
                .to_str()
                .unwrap(),
            if series_index == 0 {
                "UNDERLYING"
            } else {
                "SUPERIMPOSED"
            }
        );
        assert_eq!(
            item.element(tags::STUDY_INSTANCE_UID)
                .expect("Study UID")
                .to_str()
                .unwrap(),
            STUDY_UID
        );
        for (tag, expected) in [
            (tags::RESCALE_INTERCEPT, "-1024"),
            (tags::RESCALE_SLOPE, "1"),
            (tags::RESCALE_TYPE, "HU"),
        ] {
            assert_eq!(item.element(tag).unwrap().to_str().unwrap(), expected);
        }
        assert!(item.element(tags::SOFTCOPY_VOILUT_SEQUENCE).is_err());
        assert!(
            item.element(tags::REFERENCED_SPATIAL_REGISTRATION_SEQUENCE)
                .is_err()
        );

        let series = item
            .element(tags::REFERENCED_SERIES_SEQUENCE)
            .expect("Referenced Series Sequence")
            .items()
            .expect("items");
        assert_eq!(series.len(), 1);
        assert_eq!(
            series[0]
                .element(tags::SERIES_INSTANCE_UID)
                .unwrap()
                .to_str()
                .unwrap(),
            input.sources[series_index][0].series_instance_uid
        );
        let images = series[0]
            .element(tags::REFERENCED_IMAGE_SEQUENCE)
            .expect("Referenced Image Sequence")
            .items()
            .expect("items");
        assert_eq!(images.len(), 2);
        for (slice_index, image) in images.iter().enumerate() {
            assert_eq!(
                image
                    .element(tags::REFERENCED_SOP_CLASS_UID)
                    .unwrap()
                    .to_str()
                    .unwrap(),
                CT_IMAGE_STORAGE_UID
            );
            assert_eq!(
                image
                    .element(tags::REFERENCED_SOP_INSTANCE_UID)
                    .unwrap()
                    .to_str()
                    .unwrap(),
                input.sources[series_index][slice_index].sop_instance_uid
            );
            assert!(image.element(tags::REFERENCED_FRAME_NUMBER).is_err());
        }
    }

    assert_eq!(
        object
            .element(tags::RELATIVE_OPACITY)
            .expect("Relative Opacity")
            .to_float32()
            .expect("FL"),
        BLENDING_PRESENTATION_STATE_RELATIVE_OPACITY
    );
}

#[test]
fn blending_locks_global_displayed_area() {
    let object = build_blending_presentation_state(locked_input()).expect("locked input");
    let areas = object
        .element(tags::DISPLAYED_AREA_SELECTION_SEQUENCE)
        .expect("Displayed Area Selection Sequence")
        .items()
        .expect("items");
    assert_eq!(areas.len(), 1);
    let area = &areas[0];
    assert_eq!(
        area.element(tags::DISPLAYED_AREA_TOP_LEFT_HAND_CORNER)
            .unwrap()
            .to_multi_int::<i32>()
            .unwrap(),
        [1, 1]
    );
    assert_eq!(
        area.element(tags::DISPLAYED_AREA_BOTTOM_RIGHT_HAND_CORNER)
            .unwrap()
            .to_multi_int::<i32>()
            .unwrap(),
        [2, 2]
    );
    assert_eq!(
        area.element(tags::PRESENTATION_SIZE_MODE)
            .unwrap()
            .to_str()
            .unwrap(),
        "SCALE TO FIT"
    );
    assert_eq!(
        area.element(tags::PRESENTATION_PIXEL_ASPECT_RATIO)
            .unwrap()
            .to_multi_int::<i32>()
            .unwrap(),
        [1, 1]
    );
    for tag in [
        tags::REFERENCED_IMAGE_SEQUENCE,
        tags::PRESENTATION_PIXEL_SPACING,
        tags::PRESENTATION_PIXEL_MAGNIFICATION_RATIO,
    ] {
        assert!(area.element(tag).is_err(), "{tag:?} must be absent");
    }
}

#[test]
fn blending_locks_identity_palette_and_icc_profile() {
    let object = build_blending_presentation_state(locked_input()).expect("locked input");
    assert_eq!(
        sha256_hex(&BLENDING_PRESENTATION_STATE_PALETTE_BYTES),
        "f393097e80ec38db493eb054a0886181eb2c0e8cf7b5cdf1de392fbe94b0d1f5"
    );
    for (descriptor_tag, data_tag) in [
        (
            tags::RED_PALETTE_COLOR_LOOKUP_TABLE_DESCRIPTOR,
            tags::RED_PALETTE_COLOR_LOOKUP_TABLE_DATA,
        ),
        (
            tags::GREEN_PALETTE_COLOR_LOOKUP_TABLE_DESCRIPTOR,
            tags::GREEN_PALETTE_COLOR_LOOKUP_TABLE_DATA,
        ),
        (
            tags::BLUE_PALETTE_COLOR_LOOKUP_TABLE_DESCRIPTOR,
            tags::BLUE_PALETTE_COLOR_LOOKUP_TABLE_DATA,
        ),
    ] {
        assert_eq!(
            object
                .element(descriptor_tag)
                .unwrap()
                .to_multi_int::<u16>()
                .unwrap(),
            BLENDING_PRESENTATION_STATE_PALETTE_DESCRIPTOR
        );
        assert_eq!(
            object
                .element(data_tag)
                .unwrap()
                .to_bytes()
                .unwrap()
                .as_ref(),
            BLENDING_PRESENTATION_STATE_PALETTE_BYTES
        );
    }
    assert_eq!(
        object
            .element(tags::ICC_PROFILE)
            .unwrap()
            .to_bytes()
            .unwrap()
            .as_ref(),
        ICC_PROFILE_BYTES
    );
    assert_eq!(
        object.element(tags::COLOR_SPACE).unwrap().to_str().unwrap(),
        ICC_COLOR_SPACE
    );
}

#[test]
fn blending_omits_forbidden_modules_common_references_and_pixels() {
    let object = build_blending_presentation_state(locked_input()).expect("locked input");
    for tag in [
        tags::CONTENT_DATE,
        tags::CONTENT_TIME,
        tags::REFERENCED_SERIES_SEQUENCE,
        tags::STUDIES_CONTAINING_OTHER_REFERENCED_INSTANCES_SEQUENCE,
        tags::PRESENTATION_LUT_SEQUENCE,
        tags::PRESENTATION_LUT_SHAPE,
        tags::VOILUT_SEQUENCE,
        tags::SOFTCOPY_VOILUT_SEQUENCE,
        tags::GRAPHIC_ANNOTATION_SEQUENCE,
        tags::GRAPHIC_LAYER_SEQUENCE,
        tags::SHUTTER_SHAPE,
        tags::PALETTE_COLOR_LOOKUP_TABLE_UID,
        tags::SEGMENTED_RED_PALETTE_COLOR_LOOKUP_TABLE_DATA,
        tags::SEGMENTED_GREEN_PALETTE_COLOR_LOOKUP_TABLE_DATA,
        tags::SEGMENTED_BLUE_PALETTE_COLOR_LOOKUP_TABLE_DATA,
        tags::PIXEL_DATA,
    ] {
        assert!(object.element(tag).is_err(), "{tag:?} must be absent");
    }
}

#[test]
fn blending_rejects_broken_source_topology() {
    let mut duplicate_series = locked_input();
    duplicate_series.sources[1][0].series_instance_uid = SERIES_1_UID;
    duplicate_series.sources[1][1].series_instance_uid = SERIES_1_UID;
    assert_eq!(
        build_blending_presentation_state(duplicate_series).unwrap_err(),
        "the two blending inputs must use distinct Series Instance UIDs"
    );

    let mut cross_frame = locked_input();
    cross_frame.sources[1][1].frame_of_reference_uid =
        "2.25.100000000000000000000000000000000000011";
    assert_eq!(
        build_blending_presentation_state(cross_frame).unwrap_err(),
        "all source images must share one Frame of Reference UID"
    );

    let mut duplicate_instance = locked_input();
    duplicate_instance.sources[1][1].sop_instance_uid =
        duplicate_instance.sources[0][0].sop_instance_uid;
    assert_eq!(
        build_blending_presentation_state(duplicate_instance).unwrap_err(),
        "the four source SOP Instance UIDs must be unique"
    );
}
