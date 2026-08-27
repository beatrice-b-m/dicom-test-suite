use std::{fs, path::PathBuf};

use dicom_core::{DataElement, PrimitiveValue, VR, value::DataSetSequence};
use dicom_dictionary_std::{tags, uids};
use dicom_object::{FileMetaTableBuilder, InMemDicomObject};

use super::{
    AdvancedBlendingPresentationStateExpectations, AdvancedBlendingSourceSeriesExpectations,
    validate_advanced_blending_presentation_state_file,
};
use crate::sha256_hex;

const SOP_INSTANCE_UID: &str = "2.25.800000000000000000000000000000000000001";
const IMPLEMENTATION_UID: &str = "2.25.800000000000000000000000000000000000002";
const STUDY_UID: &str = "2.25.800000000000000000000000000000000000003";
const PRESENTATION_SERIES_UID: &str = "2.25.800000000000000000000000000000000000004";
const FRAME_UID: &str = "2.25.800000000000000000000000000000000000005";
const SERIES_1_UID: &str = "2.25.800000000000000000000000000000000000011";
const SERIES_2_UID: &str = "2.25.800000000000000000000000000000000000012";
const SOP_11_UID: &str = "2.25.800000000000000000000000000000000000111";
const SOP_12_UID: &str = "2.25.800000000000000000000000000000000000112";
const SOP_21_UID: &str = "2.25.800000000000000000000000000000000000121";
const SOP_22_UID: &str = "2.25.800000000000000000000000000000000000122";
const WRONG_UID: &str = "2.25.899999999999999999999999999999999999999";

#[derive(Clone, Copy, Debug)]
enum Mutation {
    None,
    MissingInputNumber,
    DuplicateInputNumber,
    NonOrdinalInputNumber,
    DanglingDisplayInput,
    ReversedDisplayInputs,
    RedirectedStudy,
    RedirectedSeries,
    RedirectedSop,
    ReorderedImages,
    WrongTimeSeries,
    MultipleGeometrySources,
    WrongPixelPresentation,
    WrongBlendMode,
    AddedOpacity,
    AddedOutputNumber,
    CorruptIcc,
    MissingCommonReference,
    RedirectedCommonReference,
    ReorderedCommonReference,
    CrossStudyCommonReference,
    ReferencedFrame,
    OptionalTransform,
    GraphicLayer,
    PixelData,
}

#[test]
fn accepts_the_exact_advanced_blending_contract() {
    let profile = locked_test_icc_profile();
    let profile_hash = sha256_hex(&profile);
    let path = write_fixture("valid", Mutation::None, &profile);
    let validated = validate_advanced_blending_presentation_state_file(
        &path,
        &expectations(profile_hash.as_str()),
    )
    .expect("exact Advanced Blending Presentation State should validate");

    assert_eq!(validated.validation["status"], "passed");
    assert!(
        validated.validation["internal"]
            .as_array()
            .is_some_and(|rows| rows.iter().all(|row| row["status"] == "passed"))
    );
    cleanup(path);
}

#[test]
fn rejects_invalid_input_numbering_and_display_graphs() {
    for (label, mutation, finding) in [
        (
            "duplicate-input",
            Mutation::DuplicateInputNumber,
            "advanced_blending_input_2_number",
        ),
        (
            "missing-input-number",
            Mutation::MissingInputNumber,
            "(0070,1B02)",
        ),
        (
            "nonordinal-input",
            Mutation::NonOrdinalInputNumber,
            "advanced_blending_input_2_number",
        ),
        (
            "dangling-display",
            Mutation::DanglingDisplayInput,
            "advanced_blending_display_input_2_order",
        ),
        (
            "reversed-display",
            Mutation::ReversedDisplayInputs,
            "advanced_blending_display_input_1_order",
        ),
        (
            "multiple-geometry",
            Mutation::MultipleGeometrySources,
            "advanced_blending_single_geometry_source",
        ),
        (
            "wrong-mode",
            Mutation::WrongBlendMode,
            "advanced_blending_mode",
        ),
        (
            "opacity",
            Mutation::AddedOpacity,
            "advanced_blending_display_relative_opacity_absent",
        ),
        (
            "output-number",
            Mutation::AddedOutputNumber,
            "advanced_blending_display_output_input_number_absent",
        ),
    ] {
        assert_rejects(label, mutation, finding);
    }
}

#[test]
fn rejects_redirected_reordered_and_incomplete_source_closure() {
    for (label, mutation, finding) in [
        (
            "redirect-study",
            Mutation::RedirectedStudy,
            "advanced_blending_input_1_study",
        ),
        (
            "redirect-series",
            Mutation::RedirectedSeries,
            "advanced_blending_input_1_series",
        ),
        (
            "redirect-sop",
            Mutation::RedirectedSop,
            "advanced_blending_input_1_image_1_sop_instance",
        ),
        (
            "reorder-images",
            Mutation::ReorderedImages,
            "advanced_blending_input_1_image_1_sop_instance",
        ),
        (
            "missing-common",
            Mutation::MissingCommonReference,
            "advanced_blending_common_series_2_instance_count",
        ),
        (
            "redirect-common",
            Mutation::RedirectedCommonReference,
            "advanced_blending_common_series_1_image_1_sop_instance",
        ),
        (
            "reorder-common",
            Mutation::ReorderedCommonReference,
            "advanced_blending_common_series_1_image_1_sop_instance",
        ),
        (
            "cross-study",
            Mutation::CrossStudyCommonReference,
            "advanced_blending_other_studies_absent",
        ),
        (
            "referenced-frame",
            Mutation::ReferencedFrame,
            "advanced_blending_input_1_image_1_complete_instance",
        ),
    ] {
        assert_rejects(label, mutation, finding);
    }
}

#[test]
fn rejects_locked_value_icc_and_forbidden_content_mutations() {
    for (label, mutation, finding) in [
        (
            "time-series",
            Mutation::WrongTimeSeries,
            "advanced_blending_input_1_time_series",
        ),
        (
            "pixel-presentation",
            Mutation::WrongPixelPresentation,
            "advanced_blending_pixel_presentation",
        ),
        ("icc", Mutation::CorruptIcc, "advanced_blending_icc_sha256"),
        (
            "optional-transform",
            Mutation::OptionalTransform,
            "advanced_blending_input_1_threshold_absent",
        ),
        (
            "graphic-layer",
            Mutation::GraphicLayer,
            "advanced_blending_graphic_layer_absent",
        ),
        (
            "pixel-data",
            Mutation::PixelData,
            "advanced_blending_pixel_data_absent",
        ),
    ] {
        assert_rejects(label, mutation, finding);
    }
}

fn assert_rejects(label: &str, mutation: Mutation, finding: &str) {
    let profile = locked_test_icc_profile();
    let profile_hash = sha256_hex(&profile);
    let path = write_fixture(label, mutation, &profile);
    let error = validate_advanced_blending_presentation_state_file(
        &path,
        &expectations(profile_hash.as_str()),
    )
    .expect_err("mutated Advanced Blending Presentation State must fail")
    .to_string();
    assert!(
        error.contains(finding),
        "unexpected validation error: {error}"
    );
    cleanup(path);
}

fn expectations(icc_profile_sha256: &str) -> AdvancedBlendingPresentationStateExpectations<'_> {
    AdvancedBlendingPresentationStateExpectations {
        sop_class_uid: uids::ADVANCED_BLENDING_PRESENTATION_STATE_STORAGE,
        sop_instance_uid: SOP_INSTANCE_UID,
        transfer_syntax_uid: uids::EXPLICIT_VR_LITTLE_ENDIAN,
        implementation_class_uid: IMPLEMENTATION_UID,
        synthetic_data: "YES",
        study_instance_uid: STUDY_UID,
        series_instance_uid: PRESENTATION_SERIES_UID,
        frame_of_reference_uid: FRAME_UID,
        source_series: [
            AdvancedBlendingSourceSeriesExpectations {
                series_instance_uid: SERIES_1_UID,
                sop_class_uid: uids::CT_IMAGE_STORAGE,
                sop_instance_uids: [SOP_11_UID, SOP_12_UID],
            },
            AdvancedBlendingSourceSeriesExpectations {
                series_instance_uid: SERIES_2_UID,
                sop_class_uid: uids::CT_IMAGE_STORAGE,
                sop_instance_uids: [SOP_21_UID, SOP_22_UID],
            },
        ],
        icc_profile_sha256,
    }
}

fn write_fixture(label: &str, mutation: Mutation, pristine_profile: &[u8]) -> PathBuf {
    let mut profile = pristine_profile.to_vec();
    if matches!(mutation, Mutation::CorruptIcc) {
        profile[100] ^= 1;
    }

    let input_numbers = match mutation {
        Mutation::DuplicateInputNumber => [1, 1],
        Mutation::NonOrdinalInputNumber => [1, 3],
        _ => [1, 2],
    };
    let input_1_sops = if matches!(mutation, Mutation::ReorderedImages) {
        [SOP_12_UID, SOP_11_UID]
    } else if matches!(mutation, Mutation::RedirectedSop) {
        [WRONG_UID, SOP_12_UID]
    } else {
        [SOP_11_UID, SOP_12_UID]
    };
    let input_1 = blending_input(
        Some(input_numbers[0]),
        if matches!(mutation, Mutation::RedirectedStudy) {
            WRONG_UID
        } else {
            STUDY_UID
        },
        if matches!(mutation, Mutation::RedirectedSeries) {
            WRONG_UID
        } else {
            SERIES_1_UID
        },
        input_1_sops,
        if matches!(mutation, Mutation::WrongTimeSeries) {
            "TRUE"
        } else {
            "FALSE"
        },
        "TRUE",
        matches!(mutation, Mutation::ReferencedFrame),
        matches!(mutation, Mutation::OptionalTransform),
    );
    let input_2 = blending_input(
        if matches!(mutation, Mutation::MissingInputNumber) {
            None
        } else {
            Some(input_numbers[1])
        },
        STUDY_UID,
        SERIES_2_UID,
        [SOP_21_UID, SOP_22_UID],
        "FALSE",
        if matches!(mutation, Mutation::MultipleGeometrySources) {
            "TRUE"
        } else {
            "FALSE"
        },
        false,
        false,
    );

    let display_inputs = match mutation {
        Mutation::DanglingDisplayInput => [1, 3],
        Mutation::ReversedDisplayInputs => [2, 1],
        _ => [1, 2],
    };
    let mut display = InMemDicomObject::from_element_iter([
        sequence(
            tags::BLENDING_DISPLAY_INPUT_SEQUENCE,
            display_inputs.into_iter().map(display_input).collect(),
        ),
        text(
            tags::BLENDING_MODE,
            VR::CS,
            if matches!(mutation, Mutation::WrongBlendMode) {
                "FOREGROUND"
            } else {
                "EQUAL"
            },
        ),
    ]);
    if matches!(mutation, Mutation::AddedOpacity) {
        display.put(DataElement::new(
            tags::RELATIVE_OPACITY,
            VR::FL,
            PrimitiveValue::F32(vec![0.5].into()),
        ));
    }
    if matches!(mutation, Mutation::AddedOutputNumber) {
        display.put(us(tags::BLENDING_INPUT_NUMBER, 3));
    }

    let common_1_sops = if matches!(mutation, Mutation::ReorderedCommonReference) {
        vec![SOP_12_UID, SOP_11_UID]
    } else if matches!(mutation, Mutation::RedirectedCommonReference) {
        vec![WRONG_UID, SOP_12_UID]
    } else {
        vec![SOP_11_UID, SOP_12_UID]
    };
    let common_2_sops = if matches!(mutation, Mutation::MissingCommonReference) {
        vec![SOP_21_UID]
    } else {
        vec![SOP_21_UID, SOP_22_UID]
    };

    let mut object = InMemDicomObject::from_element_iter([
        text(
            tags::SOP_CLASS_UID,
            VR::UI,
            uids::ADVANCED_BLENDING_PRESENTATION_STATE_STORAGE,
        ),
        text(tags::SOP_INSTANCE_UID, VR::UI, SOP_INSTANCE_UID),
        text(tags::SYNTHETIC_DATA, VR::CS, "YES"),
        text(tags::PATIENT_NAME, VR::PN, "DTS^Synthetic^Patient001"),
        text(tags::PATIENT_ID, VR::LO, "DTS-PATIENT-001"),
        text(tags::PATIENT_BIRTH_DATE, VR::DA, "19700101"),
        text(tags::PATIENT_SEX, VR::CS, "O"),
        text(tags::STUDY_INSTANCE_UID, VR::UI, STUDY_UID),
        text(tags::STUDY_DATE, VR::DA, "20260101"),
        text(tags::STUDY_TIME, VR::TM, "000000"),
        text(tags::REFERRING_PHYSICIAN_NAME, VR::PN, ""),
        text(tags::STUDY_ID, VR::SH, "DTS-CT"),
        text(tags::ACCESSION_NUMBER, VR::SH, ""),
        text(tags::MODALITY, VR::CS, "PR"),
        text(tags::SERIES_INSTANCE_UID, VR::UI, PRESENTATION_SERIES_UID),
        text(tags::SERIES_NUMBER, VR::IS, "80"),
        text(tags::LATERALITY, VR::CS, "R"),
        text(tags::FRAME_OF_REFERENCE_UID, VR::UI, FRAME_UID),
        text(tags::POSITION_REFERENCE_INDICATOR, VR::LO, ""),
        text(tags::MANUFACTURER, VR::LO, "dicom-test-suite"),
        text(tags::INSTITUTION_NAME, VR::LO, ""),
        text(tags::INSTITUTION_ADDRESS, VR::ST, ""),
        text(
            tags::MANUFACTURER_MODEL_NAME,
            VR::LO,
            "Native Advanced Blending Presentation State",
        ),
        text(tags::DEVICE_SERIAL_NUMBER, VR::LO, "DTS-ADVBLEND-001"),
        text(tags::SOFTWARE_VERSIONS, VR::LO, crate::PACKAGE_VERSION),
        text(tags::INSTANCE_NUMBER, VR::IS, "1"),
        text(tags::PRESENTATION_CREATION_DATE, VR::DA, "20260101"),
        text(tags::PRESENTATION_CREATION_TIME, VR::TM, "000000"),
        text(tags::CONTENT_LABEL, VR::CS, "DTSADVBLEND"),
        text(
            tags::CONTENT_DESCRIPTION,
            VR::LO,
            "Synthetic DTSADVBLEND presentation state",
        ),
        text(tags::CONTENT_CREATOR_NAME, VR::PN, "DTS^Generator"),
        text(
            tags::PIXEL_PRESENTATION,
            VR::CS,
            if matches!(mutation, Mutation::WrongPixelPresentation) {
                "MONOCHROME"
            } else {
                "TRUE_COLOR"
            },
        ),
        sequence(tags::ADVANCED_BLENDING_SEQUENCE, vec![input_1, input_2]),
        sequence(tags::BLENDING_DISPLAY_SEQUENCE, vec![display]),
        DataElement::new(
            tags::ICC_PROFILE,
            VR::OB,
            PrimitiveValue::U8(profile.into()),
        ),
        text(tags::COLOR_SPACE, VR::CS, "SRGB"),
        sequence(
            tags::REFERENCED_SERIES_SEQUENCE,
            vec![
                common_series(SERIES_1_UID, common_1_sops),
                common_series(SERIES_2_UID, common_2_sops),
            ],
        ),
    ]);
    if matches!(mutation, Mutation::CrossStudyCommonReference) {
        object.put(sequence(
            tags::STUDIES_CONTAINING_OTHER_REFERENCED_INSTANCES_SEQUENCE,
            vec![InMemDicomObject::new_empty()],
        ));
    }
    if matches!(mutation, Mutation::GraphicLayer) {
        object.put(sequence(tags::GRAPHIC_LAYER_SEQUENCE, vec![]));
    }
    if matches!(mutation, Mutation::PixelData) {
        object.put(DataElement::new(
            tags::PIXEL_DATA,
            VR::OB,
            PrimitiveValue::U8(vec![0_u8, 0].into()),
        ));
    }

    let dir = std::env::temp_dir().join(format!(
        "dts-advanced-blending-validation-{label}-{}",
        std::process::id()
    ));
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("instance.dcm");
    object
        .with_meta(
            FileMetaTableBuilder::new()
                .transfer_syntax(uids::EXPLICIT_VR_LITTLE_ENDIAN)
                .implementation_class_uid(IMPLEMENTATION_UID)
                .implementation_version_name("DTS_TEST"),
        )
        .unwrap()
        .write_to_file(&path)
        .unwrap();
    path
}

fn blending_input(
    number: Option<u16>,
    study_uid: &str,
    series_uid: &str,
    sop_uids: [&str; 2],
    time_series: &str,
    geometry: &str,
    referenced_frame: bool,
    optional_transform: bool,
) -> InMemDicomObject {
    let images = sop_uids
        .into_iter()
        .enumerate()
        .map(|(index, sop_uid)| {
            let mut image = InMemDicomObject::from_element_iter([
                text(
                    tags::REFERENCED_SOP_CLASS_UID,
                    VR::UI,
                    uids::CT_IMAGE_STORAGE,
                ),
                text(tags::REFERENCED_SOP_INSTANCE_UID, VR::UI, sop_uid),
            ]);
            if referenced_frame && index == 0 {
                image.put(text(tags::REFERENCED_FRAME_NUMBER, VR::IS, "1"));
            }
            image
        })
        .collect();
    let mut elements = vec![
        text(tags::STUDY_INSTANCE_UID, VR::UI, study_uid),
        text(tags::SERIES_INSTANCE_UID, VR::UI, series_uid),
        sequence(tags::REFERENCED_IMAGE_SEQUENCE, images),
        text(tags::TIME_SERIES_BLENDING, VR::CS, time_series),
        text(tags::GEOMETRY_FOR_DISPLAY, VR::CS, geometry),
    ];
    if let Some(number) = number {
        elements.insert(0, us(tags::BLENDING_INPUT_NUMBER, number));
    }
    let mut input = InMemDicomObject::from_element_iter(elements);
    if optional_transform {
        input.put(sequence(tags::THRESHOLD_SEQUENCE, vec![]));
    }
    input
}

fn display_input(number: u16) -> InMemDicomObject {
    InMemDicomObject::from_element_iter([us(tags::BLENDING_INPUT_NUMBER, number)])
}

fn common_series(series_uid: &str, sop_uids: Vec<&str>) -> InMemDicomObject {
    InMemDicomObject::from_element_iter([
        text(tags::SERIES_INSTANCE_UID, VR::UI, series_uid),
        sequence(
            tags::REFERENCED_INSTANCE_SEQUENCE,
            sop_uids
                .into_iter()
                .map(|uid| {
                    InMemDicomObject::from_element_iter([
                        text(
                            tags::REFERENCED_SOP_CLASS_UID,
                            VR::UI,
                            uids::CT_IMAGE_STORAGE,
                        ),
                        text(tags::REFERENCED_SOP_INSTANCE_UID, VR::UI, uid),
                    ])
                })
                .collect(),
        ),
    ])
}

fn locked_test_icc_profile() -> Vec<u8> {
    let mut bytes = vec![0_u8; 736];
    let declared_size = bytes.len() as u32;
    bytes[0..4].copy_from_slice(&declared_size.to_be_bytes());
    bytes[12..16].copy_from_slice(b"scnr");
    bytes[16..20].copy_from_slice(b"RGB ");
    bytes[20..24].copy_from_slice(b"XYZ ");
    bytes[36..40].copy_from_slice(b"acsp");
    bytes
}

fn text(tag: dicom_core::Tag, vr: VR, value: &str) -> DataElement<InMemDicomObject> {
    DataElement::new(tag, vr, value)
}

fn us(tag: dicom_core::Tag, value: u16) -> DataElement<InMemDicomObject> {
    DataElement::new(tag, VR::US, PrimitiveValue::U16(vec![value].into()))
}

fn sequence(tag: dicom_core::Tag, items: Vec<InMemDicomObject>) -> DataElement<InMemDicomObject> {
    DataElement::new(tag, VR::SQ, DataSetSequence::from(items))
}

fn cleanup(path: PathBuf) {
    fs::remove_file(&path).unwrap();
    fs::remove_dir(path.parent().unwrap()).unwrap();
}
