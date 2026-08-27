use dicom_dictionary_std::tags;

use super::{
    advanced_blending_presentation_state::{
        ADVANCED_BLENDING_PRESENTATION_STATE_CONTENT_DESCRIPTION,
        ADVANCED_BLENDING_PRESENTATION_STATE_CONTENT_LABEL,
        ADVANCED_BLENDING_PRESENTATION_STATE_CREATION_DATE,
        ADVANCED_BLENDING_PRESENTATION_STATE_CREATION_TIME,
        ADVANCED_BLENDING_PRESENTATION_STATE_SERIES_NUMBER,
        ADVANCED_BLENDING_PRESENTATION_STATE_STORAGE_UID, AdvancedBlendingPresentationStateInput,
        AdvancedBlendingPresentationStateReference, build_advanced_blending_presentation_state,
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
) -> AdvancedBlendingPresentationStateReference<'static> {
    AdvancedBlendingPresentationStateReference {
        study_instance_uid: STUDY_UID,
        series_instance_uid,
        sop_class_uid: CT_IMAGE_STORAGE_UID,
        sop_instance_uid,
        frame_of_reference_uid: FRAME_UID,
    }
}

fn locked_input() -> AdvancedBlendingPresentationStateInput<'static> {
    AdvancedBlendingPresentationStateInput {
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
fn advanced_blending_builds_locked_identity_and_mandatory_frame() {
    let input = locked_input();
    let object = build_advanced_blending_presentation_state(input).expect("locked input");

    for (tag, expected) in [
        (tags::PATIENT_NAME, "DTS^Synthetic^Patient001"),
        (tags::PATIENT_ID, "DTS-PATIENT-001"),
        (
            tags::SOP_CLASS_UID,
            ADVANCED_BLENDING_PRESENTATION_STATE_STORAGE_UID,
        ),
        (tags::SOP_INSTANCE_UID, input.sop_instance_uid),
        (tags::STUDY_INSTANCE_UID, STUDY_UID),
        (tags::STUDY_ID, "DTS-CT"),
        (tags::SERIES_INSTANCE_UID, input.series_instance_uid),
        (
            tags::SERIES_NUMBER,
            ADVANCED_BLENDING_PRESENTATION_STATE_SERIES_NUMBER,
        ),
        (tags::MODALITY, "PR"),
        (tags::LATERALITY, "R"),
        (tags::FRAME_OF_REFERENCE_UID, FRAME_UID),
        (tags::POSITION_REFERENCE_INDICATOR, ""),
        (
            tags::CONTENT_LABEL,
            ADVANCED_BLENDING_PRESENTATION_STATE_CONTENT_LABEL,
        ),
        (
            tags::CONTENT_DESCRIPTION,
            ADVANCED_BLENDING_PRESENTATION_STATE_CONTENT_DESCRIPTION,
        ),
        (
            tags::PRESENTATION_CREATION_DATE,
            ADVANCED_BLENDING_PRESENTATION_STATE_CREATION_DATE,
        ),
        (
            tags::PRESENTATION_CREATION_TIME,
            ADVANCED_BLENDING_PRESENTATION_STATE_CREATION_TIME,
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
fn advanced_blending_emits_two_ordered_complete_instance_inputs() {
    let input = locked_input();
    let object = build_advanced_blending_presentation_state(input).expect("locked input");
    let blending = object
        .element(tags::ADVANCED_BLENDING_SEQUENCE)
        .expect("Advanced Blending Sequence")
        .items()
        .expect("items");

    assert_eq!(blending.len(), 2);
    for (index, item) in blending.iter().enumerate() {
        assert_eq!(
            item.element(tags::BLENDING_INPUT_NUMBER)
                .expect("input number")
                .to_int::<u16>()
                .expect("US"),
            index as u16 + 1
        );
        assert_eq!(
            item.element(tags::STUDY_INSTANCE_UID)
                .expect("Study UID")
                .to_str()
                .unwrap(),
            STUDY_UID
        );
        assert_eq!(
            item.element(tags::SERIES_INSTANCE_UID)
                .expect("Series UID")
                .to_str()
                .unwrap(),
            input.sources[index][0].series_instance_uid
        );
        assert_eq!(
            item.element(tags::TIME_SERIES_BLENDING)
                .expect("time series")
                .to_str()
                .unwrap(),
            "FALSE"
        );
        assert_eq!(
            item.element(tags::GEOMETRY_FOR_DISPLAY)
                .expect("geometry flag")
                .to_str()
                .unwrap(),
            if index == 0 { "TRUE" } else { "FALSE" }
        );
        let images = item
            .element(tags::REFERENCED_IMAGE_SEQUENCE)
            .expect("Referenced Image Sequence")
            .items()
            .expect("items");
        assert_eq!(images.len(), 2);
        for (slice_index, image) in images.iter().enumerate() {
            assert_eq!(
                image
                    .element(tags::REFERENCED_SOP_CLASS_UID)
                    .expect("SOP Class")
                    .to_str()
                    .unwrap(),
                CT_IMAGE_STORAGE_UID
            );
            assert_eq!(
                image
                    .element(tags::REFERENCED_SOP_INSTANCE_UID)
                    .expect("SOP Instance")
                    .to_str()
                    .unwrap(),
                input.sources[index][slice_index].sop_instance_uid
            );
            assert!(image.element(tags::REFERENCED_FRAME_NUMBER).is_err());
        }
    }
}

#[test]
fn advanced_blending_locks_final_display_and_icc_profile() {
    let object = build_advanced_blending_presentation_state(locked_input()).expect("locked input");
    assert_eq!(
        object
            .element(tags::PIXEL_PRESENTATION)
            .unwrap()
            .to_str()
            .unwrap(),
        "TRUE_COLOR"
    );
    let display = object
        .element(tags::BLENDING_DISPLAY_SEQUENCE)
        .expect("Blending Display Sequence")
        .items()
        .expect("items");
    assert_eq!(display.len(), 1);
    assert_eq!(
        display[0]
            .element(tags::BLENDING_MODE)
            .unwrap()
            .to_str()
            .unwrap(),
        "EQUAL"
    );
    assert!(display[0].element(tags::BLENDING_INPUT_NUMBER).is_err());
    assert!(display[0].element(tags::RELATIVE_OPACITY).is_err());
    let display_inputs = display[0]
        .element(tags::BLENDING_DISPLAY_INPUT_SEQUENCE)
        .expect("display inputs")
        .items()
        .expect("items");
    assert_eq!(display_inputs.len(), 2);
    assert_eq!(
        display_inputs
            .iter()
            .map(|item| item
                .element(tags::BLENDING_INPUT_NUMBER)
                .unwrap()
                .to_int::<u16>()
                .unwrap())
            .collect::<Vec<_>>(),
        [1, 2]
    );
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
fn advanced_blending_common_references_mirror_all_four_sources() {
    let input = locked_input();
    let object = build_advanced_blending_presentation_state(input).expect("locked input");
    let series = object
        .element(tags::REFERENCED_SERIES_SEQUENCE)
        .expect("Referenced Series Sequence")
        .items()
        .expect("items");
    assert_eq!(series.len(), 2);
    for (series_index, series_item) in series.iter().enumerate() {
        assert_eq!(
            series_item
                .element(tags::SERIES_INSTANCE_UID)
                .unwrap()
                .to_str()
                .unwrap(),
            input.sources[series_index][0].series_instance_uid
        );
        let instances = series_item
            .element(tags::REFERENCED_INSTANCE_SEQUENCE)
            .expect("Referenced Instance Sequence")
            .items()
            .expect("items");
        assert_eq!(instances.len(), 2);
        assert_eq!(
            instances
                .iter()
                .map(|item| item
                    .element(tags::REFERENCED_SOP_INSTANCE_UID)
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .into_owned())
                .collect::<Vec<_>>(),
            input.sources[series_index]
                .iter()
                .map(|reference| reference.sop_instance_uid.to_string())
                .collect::<Vec<_>>()
        );
    }
    assert!(
        object
            .element(tags::STUDIES_CONTAINING_OTHER_REFERENCED_INSTANCES_SEQUENCE)
            .is_err()
    );
}

#[test]
fn advanced_blending_omits_optional_transforms_and_pixels() {
    let object = build_advanced_blending_presentation_state(locked_input()).expect("locked input");
    for tag in [
        tags::CONTENT_DATE,
        tags::CONTENT_TIME,
        tags::DISPLAYED_AREA_SELECTION_SEQUENCE,
        tags::GRAPHIC_ANNOTATION_SEQUENCE,
        tags::GRAPHIC_LAYER_SEQUENCE,
        tags::PIXEL_DATA,
    ] {
        assert!(object.element(tag).is_err(), "{tag:?} must be absent");
    }
    for input in object
        .element(tags::ADVANCED_BLENDING_SEQUENCE)
        .unwrap()
        .items()
        .unwrap()
    {
        for tag in [
            tags::REFERENCED_SPATIAL_REGISTRATION_SEQUENCE,
            tags::SOFTCOPY_VOILUT_SEQUENCE,
            tags::PALETTE_COLOR_LOOKUP_TABLE_SEQUENCE,
            tags::THRESHOLD_SEQUENCE,
        ] {
            assert!(input.element(tag).is_err(), "{tag:?} must be absent");
        }
    }
}

#[test]
fn advanced_blending_rejects_broken_source_topology() {
    let mut duplicate_series = locked_input();
    duplicate_series.sources[1][0].series_instance_uid = SERIES_1_UID;
    duplicate_series.sources[1][1].series_instance_uid = SERIES_1_UID;
    assert_eq!(
        build_advanced_blending_presentation_state(duplicate_series).unwrap_err(),
        "the two blending inputs must use distinct Series Instance UIDs"
    );

    let mut cross_study = locked_input();
    cross_study.sources[1][1].study_instance_uid = "2.25.100000000000000000000000000000000000011";
    assert_eq!(
        build_advanced_blending_presentation_state(cross_study).unwrap_err(),
        "all source images must share one Study Instance UID"
    );

    let mut duplicate_instance = locked_input();
    duplicate_instance.sources[1][1].sop_instance_uid =
        duplicate_instance.sources[0][0].sop_instance_uid;
    assert_eq!(
        build_advanced_blending_presentation_state(duplicate_instance).unwrap_err(),
        "the four source SOP Instance UIDs must be unique"
    );
}
