use std::{fs, path::PathBuf};

use dicom_core::{DataElement, PrimitiveValue, VR, value::DataSetSequence};
use dicom_dictionary_std::{tags, uids};
use dicom_object::{FileMetaTableBuilder, InMemDicomObject};

use super::{
    BlendingPresentationStateExpectations, BlendingSourceSeriesExpectations,
    validate_blending_presentation_state_file,
};
use crate::sha256_hex;

const SOP_UID: &str = "2.25.810000000000000000000000000000000000001";
const IMPLEMENTATION_UID: &str = "2.25.810000000000000000000000000000000000002";
const STUDY_UID: &str = "2.25.810000000000000000000000000000000000003";
const PR_SERIES_UID: &str = "2.25.810000000000000000000000000000000000004";
const SERIES_1_UID: &str = "2.25.810000000000000000000000000000000000011";
const SERIES_2_UID: &str = "2.25.810000000000000000000000000000000000012";
const SOP_11_UID: &str = "2.25.810000000000000000000000000000000000111";
const SOP_12_UID: &str = "2.25.810000000000000000000000000000000000112";
const SOP_21_UID: &str = "2.25.810000000000000000000000000000000000121";
const SOP_22_UID: &str = "2.25.810000000000000000000000000000000000122";
const WRONG_UID: &str = "2.25.819999999999999999999999999999999999999";

#[derive(Clone, Copy)]
enum Mutation {
    None,
    OneItem,
    DuplicatePositions,
    OpacityOutOfRange,
    RedirectedStudy,
    RedirectedSeries,
    RedirectedSop,
    ReorderedImages,
    WrongRescale,
    UnexpectedVoi,
    CorruptPalette,
    CorruptIcc,
    UnexpectedFrameOfReference,
    PixelData,
}

#[test]
fn accepts_exact_blending_softcopy_contract() {
    let (palette, profile) = locked_payloads();
    let path = write_fixture("valid", Mutation::None, &palette, &profile);
    let validated = validate_blending_presentation_state_file(
        &path,
        &expectations(&sha256_hex(&palette), &sha256_hex(&profile)),
    )
    .expect("exact Blending Softcopy Presentation State should validate");
    assert_eq!(validated.validation["status"], "passed");
    cleanup(path);
}

#[test]
fn rejects_cardinality_position_opacity_and_reference_gaps() {
    for (label, mutation, finding) in [
        ("one-item", Mutation::OneItem, "(0070,0402)"),
        (
            "duplicate-position",
            Mutation::DuplicatePositions,
            "blending_item_2_position",
        ),
        (
            "opacity-range",
            Mutation::OpacityOutOfRange,
            "blending_opacity_range",
        ),
        (
            "redirect-study",
            Mutation::RedirectedStudy,
            "blending_item_1_study",
        ),
        (
            "redirect-series",
            Mutation::RedirectedSeries,
            "blending_item_1_series",
        ),
        (
            "reorder-images",
            Mutation::ReorderedImages,
            "blending_item_1_image_1_sop_instance",
        ),
        (
            "redirect-sop",
            Mutation::RedirectedSop,
            "blending_item_1_image_1_sop_instance",
        ),
    ] {
        assert_rejects(label, mutation, finding);
    }
}

#[test]
fn rejects_transform_payload_and_forbidden_module_mutations() {
    for (label, mutation, finding) in [
        (
            "rescale",
            Mutation::WrongRescale,
            "blending_item_1_rescale_slope",
        ),
        (
            "voi",
            Mutation::UnexpectedVoi,
            "blending_item_1_softcopy_voi_absent",
        ),
        (
            "palette",
            Mutation::CorruptPalette,
            "blending_palette_red_data_sha256",
        ),
        ("icc", Mutation::CorruptIcc, "blending_icc_sha256"),
        (
            "frame",
            Mutation::UnexpectedFrameOfReference,
            "blending_frame_of_reference_absent",
        ),
        ("pixel", Mutation::PixelData, "blending_pixel_data_absent"),
    ] {
        assert_rejects(label, mutation, finding);
    }
}

fn assert_rejects(label: &str, mutation: Mutation, finding: &str) {
    let (palette, profile) = locked_payloads();
    let path = write_fixture(label, mutation, &palette, &profile);
    let error = validate_blending_presentation_state_file(
        &path,
        &expectations(&sha256_hex(&palette), &sha256_hex(&profile)),
    )
    .expect_err("mutated Blending Presentation State must fail")
    .to_string();
    assert!(
        error.contains(finding),
        "unexpected validation error: {error}"
    );
    cleanup(path);
}

fn expectations<'a>(
    palette_hash: &'a str,
    icc_hash: &'a str,
) -> BlendingPresentationStateExpectations<'a> {
    BlendingPresentationStateExpectations {
        sop_class_uid: "1.2.840.10008.5.1.4.1.1.11.4",
        sop_instance_uid: SOP_UID,
        transfer_syntax_uid: uids::EXPLICIT_VR_LITTLE_ENDIAN,
        implementation_class_uid: IMPLEMENTATION_UID,
        synthetic_data: "YES",
        study_instance_uid: STUDY_UID,
        series_instance_uid: PR_SERIES_UID,
        source_series: [
            BlendingSourceSeriesExpectations {
                series_instance_uid: SERIES_1_UID,
                sop_class_uid: uids::CT_IMAGE_STORAGE,
                sop_instance_uids: [SOP_11_UID, SOP_12_UID],
            },
            BlendingSourceSeriesExpectations {
                series_instance_uid: SERIES_2_UID,
                sop_class_uid: uids::CT_IMAGE_STORAGE,
                sop_instance_uids: [SOP_21_UID, SOP_22_UID],
            },
        ],
        palette_channel_sha256: palette_hash,
        icc_profile_sha256: icc_hash,
    }
}

fn write_fixture(
    label: &str,
    mutation: Mutation,
    pristine_palette: &[u8],
    pristine_profile: &[u8],
) -> PathBuf {
    let mut palette = pristine_palette.to_vec();
    let mut profile = pristine_profile.to_vec();
    if matches!(mutation, Mutation::CorruptPalette) {
        palette[100] ^= 1;
    }
    if matches!(mutation, Mutation::CorruptIcc) {
        profile[100] ^= 1;
    }
    let input_1_sops = if matches!(mutation, Mutation::ReorderedImages) {
        [SOP_12_UID, SOP_11_UID]
    } else if matches!(mutation, Mutation::RedirectedSop) {
        [WRONG_UID, SOP_12_UID]
    } else {
        [SOP_11_UID, SOP_12_UID]
    };
    let first = blending_item(
        "UNDERLYING",
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
        if matches!(mutation, Mutation::WrongRescale) {
            "2"
        } else {
            "1"
        },
        matches!(mutation, Mutation::UnexpectedVoi),
    );
    let second = blending_item(
        if matches!(mutation, Mutation::DuplicatePositions) {
            "UNDERLYING"
        } else {
            "SUPERIMPOSED"
        },
        STUDY_UID,
        SERIES_2_UID,
        [SOP_21_UID, SOP_22_UID],
        "1",
        false,
    );
    let blending_items = if matches!(mutation, Mutation::OneItem) {
        vec![first]
    } else {
        vec![first, second]
    };

    let mut object = InMemDicomObject::from_element_iter([
        text(tags::SOP_CLASS_UID, VR::UI, "1.2.840.10008.5.1.4.1.1.11.4"),
        text(tags::SOP_INSTANCE_UID, VR::UI, SOP_UID),
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
        text(tags::SERIES_INSTANCE_UID, VR::UI, PR_SERIES_UID),
        text(tags::SERIES_NUMBER, VR::IS, "81"),
        text(tags::LATERALITY, VR::CS, "R"),
        text(tags::MANUFACTURER, VR::LO, "dicom-test-suite"),
        text(tags::INSTITUTION_NAME, VR::LO, ""),
        text(tags::INSTITUTION_ADDRESS, VR::ST, ""),
        text(
            tags::MANUFACTURER_MODEL_NAME,
            VR::LO,
            "Native Blending Softcopy Presentation State",
        ),
        text(tags::DEVICE_SERIAL_NUMBER, VR::LO, "DTS-BLEND-001"),
        text(
            tags::SOFTWARE_VERSIONS,
            VR::LO,
            crate::BYTE_STABLE_OUTPUT_VERSION,
        ),
        text(tags::INSTANCE_NUMBER, VR::IS, "1"),
        text(tags::PRESENTATION_CREATION_DATE, VR::DA, "20260101"),
        text(tags::PRESENTATION_CREATION_TIME, VR::TM, "000000"),
        text(tags::CONTENT_LABEL, VR::CS, "DTSBLEND"),
        text(
            tags::CONTENT_DESCRIPTION,
            VR::LO,
            "Synthetic DTSBLEND presentation state",
        ),
        text(tags::CONTENT_CREATOR_NAME, VR::PN, "DTS^Generator"),
        sequence(tags::BLENDING_SEQUENCE, blending_items),
        DataElement::new(
            tags::RELATIVE_OPACITY,
            VR::FL,
            PrimitiveValue::F32(
                vec![if matches!(mutation, Mutation::OpacityOutOfRange) {
                    1.5
                } else {
                    0.5
                }]
                .into(),
            ),
        ),
        sequence(
            tags::DISPLAYED_AREA_SELECTION_SEQUENCE,
            vec![displayed_area()],
        ),
        us3(tags::RED_PALETTE_COLOR_LOOKUP_TABLE_DESCRIPTOR),
        us3(tags::GREEN_PALETTE_COLOR_LOOKUP_TABLE_DESCRIPTOR),
        us3(tags::BLUE_PALETTE_COLOR_LOOKUP_TABLE_DESCRIPTOR),
        ow(tags::RED_PALETTE_COLOR_LOOKUP_TABLE_DATA, palette.clone()),
        ow(
            tags::GREEN_PALETTE_COLOR_LOOKUP_TABLE_DATA,
            pristine_palette.to_vec(),
        ),
        ow(
            tags::BLUE_PALETTE_COLOR_LOOKUP_TABLE_DATA,
            pristine_palette.to_vec(),
        ),
        DataElement::new(
            tags::ICC_PROFILE,
            VR::OB,
            PrimitiveValue::U8(profile.into()),
        ),
        text(tags::COLOR_SPACE, VR::CS, "SRGB"),
    ]);
    if matches!(mutation, Mutation::UnexpectedFrameOfReference) {
        object.put(text(tags::FRAME_OF_REFERENCE_UID, VR::UI, WRONG_UID));
    }
    if matches!(mutation, Mutation::PixelData) {
        object.put(DataElement::new(
            tags::PIXEL_DATA,
            VR::OB,
            PrimitiveValue::U8(vec![0, 0].into()),
        ));
    }
    let dir = std::env::temp_dir().join(format!(
        "dts-blending-validation-{label}-{}",
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

fn blending_item(
    position: &str,
    study: &str,
    series: &str,
    sop_uids: [&str; 2],
    slope: &str,
    voi: bool,
) -> InMemDicomObject {
    let images = sop_uids
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
        .collect();
    let referenced_series = InMemDicomObject::from_element_iter([
        text(tags::SERIES_INSTANCE_UID, VR::UI, series),
        sequence(tags::REFERENCED_IMAGE_SEQUENCE, images),
    ]);
    let mut item = InMemDicomObject::from_element_iter([
        text(tags::BLENDING_POSITION, VR::CS, position),
        text(tags::STUDY_INSTANCE_UID, VR::UI, study),
        sequence(tags::REFERENCED_SERIES_SEQUENCE, vec![referenced_series]),
        text(tags::RESCALE_INTERCEPT, VR::DS, "-1024"),
        text(tags::RESCALE_SLOPE, VR::DS, slope),
        text(tags::RESCALE_TYPE, VR::LO, "HU"),
    ]);
    if voi {
        item.put(sequence(tags::SOFTCOPY_VOILUT_SEQUENCE, vec![]));
    }
    item
}

fn displayed_area() -> InMemDicomObject {
    InMemDicomObject::from_element_iter([
        DataElement::new(
            tags::DISPLAYED_AREA_TOP_LEFT_HAND_CORNER,
            VR::SL,
            PrimitiveValue::from([1_i32, 1]),
        ),
        DataElement::new(
            tags::DISPLAYED_AREA_BOTTOM_RIGHT_HAND_CORNER,
            VR::SL,
            PrimitiveValue::from([2_i32, 2]),
        ),
        text(tags::PRESENTATION_SIZE_MODE, VR::CS, "SCALE TO FIT"),
        DataElement::new(
            tags::PRESENTATION_PIXEL_ASPECT_RATIO,
            VR::IS,
            PrimitiveValue::from([1_i32, 1]),
        ),
    ])
}

fn locked_payloads() -> (Vec<u8>, Vec<u8>) {
    let palette = (0_u16..=255)
        .flat_map(|value| (value * 0x0101).to_le_bytes())
        .collect();
    let mut profile = vec![0_u8; 736];
    profile[0..4].copy_from_slice(&736_u32.to_be_bytes());
    profile[12..16].copy_from_slice(b"scnr");
    profile[16..20].copy_from_slice(b"RGB ");
    profile[20..24].copy_from_slice(b"XYZ ");
    profile[36..40].copy_from_slice(b"acsp");
    (palette, profile)
}

fn text(tag: dicom_core::Tag, vr: VR, value: &str) -> DataElement<InMemDicomObject> {
    DataElement::new(tag, vr, value)
}
fn sequence(tag: dicom_core::Tag, items: Vec<InMemDicomObject>) -> DataElement<InMemDicomObject> {
    DataElement::new(tag, VR::SQ, DataSetSequence::from(items))
}
fn us3(tag: dicom_core::Tag) -> DataElement<InMemDicomObject> {
    DataElement::new(tag, VR::US, PrimitiveValue::U16(vec![256, 0, 16].into()))
}
fn ow(tag: dicom_core::Tag, bytes: Vec<u8>) -> DataElement<InMemDicomObject> {
    DataElement::new(tag, VR::OW, PrimitiveValue::U8(bytes.into()))
}
fn cleanup(path: PathBuf) {
    fs::remove_file(&path).unwrap();
    fs::remove_dir(path.parent().unwrap()).unwrap();
}
