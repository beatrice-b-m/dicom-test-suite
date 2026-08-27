use dicom_core::header::Header;
use dicom_dictionary_std::tags;

use super::{
    color_softcopy_presentation_state::{
        COLOR_SOFTCOPY_PRESENTATION_STATE_CONTENT_DESCRIPTION,
        COLOR_SOFTCOPY_PRESENTATION_STATE_CONTENT_LABEL,
        COLOR_SOFTCOPY_PRESENTATION_STATE_CREATION_DATE,
        COLOR_SOFTCOPY_PRESENTATION_STATE_CREATION_TIME,
        COLOR_SOFTCOPY_PRESENTATION_STATE_SERIES_NUMBER,
        COLOR_SOFTCOPY_PRESENTATION_STATE_STORAGE_UID, ColorSoftcopyPresentationStateInput,
        ColorSoftcopyPresentationStateReference, build_color_softcopy_presentation_state,
    },
    icc_profile::{ICC_COLOR_SPACE, ICC_PROFILE_BYTES},
};

const RGB_PHOTO_STORAGE_UID: &str = "1.2.840.10008.5.1.4.1.1.77.1.4";

fn locked_input() -> ColorSoftcopyPresentationStateInput<'static> {
    ColorSoftcopyPresentationStateInput {
        sop_instance_uid: "2.25.100000000000000000000000000000000000001",
        series_instance_uid: "2.25.100000000000000000000000000000000000002",
        source: ColorSoftcopyPresentationStateReference {
            study_instance_uid: "2.25.100000000000000000000000000000000000003",
            series_instance_uid: "2.25.100000000000000000000000000000000000004",
            sop_class_uid: RGB_PHOTO_STORAGE_UID,
            sop_instance_uid: "2.25.100000000000000000000000000000000000005",
        },
    }
}

#[test]
fn color_softcopy_presentation_state_builds_locked_identity_and_series() {
    let input = locked_input();
    let object = build_color_softcopy_presentation_state(input).expect("locked input should build");

    for (tag, expected) in [
        (tags::PATIENT_NAME, "DICOMTEST^SMOKE"),
        (tags::PATIENT_ID, "DICOMTEST-SMOKE-001"),
        (
            tags::SOP_CLASS_UID,
            COLOR_SOFTCOPY_PRESENTATION_STATE_STORAGE_UID,
        ),
        (tags::SOP_INSTANCE_UID, input.sop_instance_uid),
        (tags::STUDY_INSTANCE_UID, input.source.study_instance_uid),
        (tags::STUDY_ID, "SMOKE"),
        (tags::SERIES_INSTANCE_UID, input.series_instance_uid),
        (
            tags::SERIES_NUMBER,
            COLOR_SOFTCOPY_PRESENTATION_STATE_SERIES_NUMBER,
        ),
        (tags::MODALITY, "PR"),
        (tags::BODY_PART_EXAMINED, "HAND"),
        (tags::LATERALITY, "R"),
        (
            tags::CONTENT_LABEL,
            COLOR_SOFTCOPY_PRESENTATION_STATE_CONTENT_LABEL,
        ),
        (
            tags::CONTENT_DESCRIPTION,
            COLOR_SOFTCOPY_PRESENTATION_STATE_CONTENT_DESCRIPTION,
        ),
        (
            tags::PRESENTATION_CREATION_DATE,
            COLOR_SOFTCOPY_PRESENTATION_STATE_CREATION_DATE,
        ),
        (
            tags::PRESENTATION_CREATION_TIME,
            COLOR_SOFTCOPY_PRESENTATION_STATE_CREATION_TIME,
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
}

#[test]
fn color_softcopy_presentation_state_references_exactly_one_source_image() {
    let input = locked_input();
    let object = build_color_softcopy_presentation_state(input).expect("locked input should build");
    let series = object
        .element(tags::REFERENCED_SERIES_SEQUENCE)
        .expect("Referenced Series Sequence")
        .items()
        .expect("sequence items");

    assert_eq!(series.len(), 1);
    assert_eq!(
        series[0]
            .element(tags::SERIES_INSTANCE_UID)
            .expect("referenced Series Instance UID")
            .to_str()
            .expect("text"),
        input.source.series_instance_uid
    );
    let images = series[0]
        .element(tags::REFERENCED_IMAGE_SEQUENCE)
        .expect("Referenced Image Sequence")
        .items()
        .expect("sequence items");
    assert_eq!(images.len(), 1);
    assert_eq!(
        images[0]
            .element(tags::REFERENCED_SOP_CLASS_UID)
            .expect("Referenced SOP Class UID")
            .to_str()
            .expect("text"),
        input.source.sop_class_uid
    );
    assert_eq!(
        images[0]
            .element(tags::REFERENCED_SOP_INSTANCE_UID)
            .expect("Referenced SOP Instance UID")
            .to_str()
            .expect("text"),
        input.source.sop_instance_uid
    );
}

#[test]
fn color_softcopy_presentation_state_locks_global_displayed_area_and_color_profile() {
    let object =
        build_color_softcopy_presentation_state(locked_input()).expect("locked input should build");
    let areas = object
        .element(tags::DISPLAYED_AREA_SELECTION_SEQUENCE)
        .expect("Displayed Area Selection Sequence")
        .items()
        .expect("sequence items");

    assert_eq!(areas.len(), 1);
    assert_eq!(
        areas[0]
            .element(tags::DISPLAYED_AREA_TOP_LEFT_HAND_CORNER)
            .expect("top-left")
            .to_multi_int::<i32>()
            .expect("SL values"),
        [1, 1]
    );
    assert_eq!(
        areas[0]
            .element(tags::DISPLAYED_AREA_BOTTOM_RIGHT_HAND_CORNER)
            .expect("bottom-right")
            .to_multi_int::<i32>()
            .expect("SL values"),
        [2, 2]
    );
    assert_eq!(
        areas[0]
            .element(tags::PRESENTATION_SIZE_MODE)
            .expect("size mode")
            .to_str()
            .expect("text"),
        "SCALE TO FIT"
    );
    assert_eq!(
        areas[0]
            .element(tags::PRESENTATION_PIXEL_ASPECT_RATIO)
            .expect("pixel aspect ratio")
            .to_multi_int::<i32>()
            .expect("IS values"),
        [1, 1]
    );
    assert!(areas[0].element(tags::REFERENCED_IMAGE_SEQUENCE).is_err());
    assert!(areas[0].element(tags::PRESENTATION_PIXEL_SPACING).is_err());
    assert!(
        areas[0]
            .element(tags::PRESENTATION_PIXEL_MAGNIFICATION_RATIO)
            .is_err()
    );

    assert_eq!(
        object
            .element(tags::ICC_PROFILE)
            .expect("ICC Profile")
            .to_bytes()
            .expect("bytes")
            .as_ref(),
        ICC_PROFILE_BYTES
    );
    assert_eq!(
        object
            .element(tags::COLOR_SPACE)
            .expect("Color Space")
            .to_str()
            .expect("text"),
        ICC_COLOR_SPACE
    );
}

#[test]
fn color_softcopy_presentation_state_omits_optional_rendering_content_and_pixels() {
    let object =
        build_color_softcopy_presentation_state(locked_input()).expect("locked input should build");

    for tag in [
        tags::CONTENT_DATE,
        tags::CONTENT_TIME,
        tags::SHUTTER_SHAPE,
        tags::GRAPHIC_ANNOTATION_SEQUENCE,
        tags::GRAPHIC_LAYER_SEQUENCE,
        tags::IMAGE_ROTATION,
        tags::IMAGE_HORIZONTAL_FLIP,
        tags::PIXEL_DATA,
    ] {
        assert!(object.element(tag).is_err(), "{tag:?} must be absent");
    }
    assert!(
        object.iter().all(|element| element.tag().group() != 0x6000),
        "overlay groups must be absent"
    );
}

#[test]
fn color_softcopy_presentation_state_rejects_shared_series_and_empty_identity() {
    let mut shared_series = locked_input();
    shared_series.series_instance_uid = shared_series.source.series_instance_uid;
    assert_eq!(
        build_color_softcopy_presentation_state(shared_series)
            .expect_err("shared series must fail"),
        "the presentation state and source must use distinct Series Instance UIDs"
    );

    let mut missing_sop_class = locked_input();
    missing_sop_class.source.sop_class_uid = "";
    assert_eq!(
        build_color_softcopy_presentation_state(missing_sop_class)
            .expect_err("missing source SOP Class UID must fail"),
        "source SOP Class UID must not be empty"
    );
}
