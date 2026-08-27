use std::{fs, path::PathBuf};

use dicom_core::{DataElement, PrimitiveValue, VR, value::DataSetSequence};
use dicom_dictionary_std::{tags, uids};
use dicom_object::{FileMetaTableBuilder, InMemDicomObject};

use super::{
    ColorSoftcopyPresentationStateExpectations, validate_color_softcopy_presentation_state_file,
};
use crate::sha256_hex;

const SOP_CLASS_UID: &str = "1.2.840.10008.5.1.4.1.1.11.2";
const SOP_INSTANCE_UID: &str = "2.25.100000000000000000000000000000000000001";
const IMPLEMENTATION_UID: &str = "2.25.100000000000000000000000000000000000002";
const STUDY_UID: &str = "2.25.100000000000000000000000000000000000003";
const SERIES_UID: &str = "2.25.100000000000000000000000000000000000004";
const SOURCE_SERIES_UID: &str = "2.25.100000000000000000000000000000000000005";
const SOURCE_SOP_UID: &str = "2.25.100000000000000000000000000000000000006";
const WRONG_UID: &str = "2.25.999999999999999999999999999999999999999";

#[derive(Clone, Copy)]
enum Mutation {
    None,
    WrongSeries,
    DanglingSop,
    DisplayCorner,
    IccCorruption,
    ReferencedFrame,
    Graphics,
    PixelData,
}

#[test]
fn accepts_the_exact_color_softcopy_contract() {
    let profile = locked_test_icc_profile();
    let profile_hash = sha256_hex(&profile);
    let path = write_fixture("valid", Mutation::None, &profile);

    let validated = validate_color_softcopy_presentation_state_file(
        &path,
        &expectations(profile_hash.as_str()),
    )
    .expect("exact Color Softcopy Presentation State should validate");

    assert_eq!(validated.validation["status"], "passed");
    assert!(
        validated.validation["internal"]
            .as_array()
            .is_some_and(|rows| rows.iter().all(|row| row["status"] == "passed"))
    );
    cleanup(path);
}

#[test]
fn rejects_a_reference_to_the_wrong_series() {
    assert_rejects(
        "wrong-series",
        Mutation::WrongSeries,
        "color_softcopy_referenced_series_uid",
    );
}

#[test]
fn rejects_a_dangling_source_sop_reference() {
    assert_rejects(
        "dangling-sop",
        Mutation::DanglingSop,
        "color_softcopy_referenced_sop_instance_uid",
    );
}

#[test]
fn rejects_display_geometry_drift() {
    assert_rejects(
        "display-corner",
        Mutation::DisplayCorner,
        "color_softcopy_displayed_area_bottom_right",
    );
}

#[test]
fn rejects_icc_profile_corruption() {
    assert_rejects(
        "icc-corruption",
        Mutation::IccCorruption,
        "color_softcopy_icc_profile_sha256",
    );
}

#[test]
fn rejects_an_unexpected_referenced_frame_number() {
    assert_rejects(
        "referenced-frame",
        Mutation::ReferencedFrame,
        "color_softcopy_referenced_frame_numbers_absent",
    );
}

#[test]
fn rejects_unexpected_graphic_content() {
    assert_rejects(
        "graphics",
        Mutation::Graphics,
        "color_softcopy_graphic_annotation_sequence_absent",
    );
}

#[test]
fn rejects_unexpected_pixel_data() {
    assert_rejects(
        "pixel-data",
        Mutation::PixelData,
        "color_softcopy_pixel_data_absent",
    );
}

fn assert_rejects(label: &str, mutation: Mutation, finding: &str) {
    let profile = locked_test_icc_profile();
    let profile_hash = sha256_hex(&profile);
    let path = write_fixture(label, mutation, &profile);
    let error = validate_color_softcopy_presentation_state_file(
        &path,
        &expectations(profile_hash.as_str()),
    )
    .expect_err("mutated Color Softcopy Presentation State must fail")
    .to_string();

    assert!(
        error.contains(finding),
        "unexpected validation error: {error}"
    );
    cleanup(path);
}

fn expectations(icc_profile_sha256: &str) -> ColorSoftcopyPresentationStateExpectations<'_> {
    ColorSoftcopyPresentationStateExpectations {
        sop_class_uid: SOP_CLASS_UID,
        sop_instance_uid: SOP_INSTANCE_UID,
        transfer_syntax_uid: uids::EXPLICIT_VR_LITTLE_ENDIAN,
        implementation_class_uid: IMPLEMENTATION_UID,
        synthetic_data: "YES",
        study_instance_uid: STUDY_UID,
        series_instance_uid: SERIES_UID,
        source_study_instance_uid: STUDY_UID,
        source_series_instance_uid: SOURCE_SERIES_UID,
        source_sop_class_uid: uids::SECONDARY_CAPTURE_IMAGE_STORAGE,
        source_sop_instance_uid: SOURCE_SOP_UID,
        icc_profile_sha256,
    }
}

fn write_fixture(label: &str, mutation: Mutation, pristine_profile: &[u8]) -> PathBuf {
    let referenced_series_uid = if matches!(mutation, Mutation::WrongSeries) {
        WRONG_UID
    } else {
        SOURCE_SERIES_UID
    };
    let referenced_sop_uid = if matches!(mutation, Mutation::DanglingSop) {
        WRONG_UID
    } else {
        SOURCE_SOP_UID
    };
    let mut referenced_image = InMemDicomObject::from_element_iter([
        text(
            tags::REFERENCED_SOP_CLASS_UID,
            VR::UI,
            uids::SECONDARY_CAPTURE_IMAGE_STORAGE,
        ),
        text(
            tags::REFERENCED_SOP_INSTANCE_UID,
            VR::UI,
            referenced_sop_uid,
        ),
    ]);
    if matches!(mutation, Mutation::ReferencedFrame) {
        referenced_image.put(text(tags::REFERENCED_FRAME_NUMBER, VR::IS, "1"));
    }

    let bottom_right = if matches!(mutation, Mutation::DisplayCorner) {
        [3_i32, 2_i32]
    } else {
        [2_i32, 2_i32]
    };
    let displayed_area = InMemDicomObject::from_element_iter([
        DataElement::new(
            tags::DISPLAYED_AREA_TOP_LEFT_HAND_CORNER,
            VR::SL,
            PrimitiveValue::from([1_i32, 1_i32]),
        ),
        DataElement::new(
            tags::DISPLAYED_AREA_BOTTOM_RIGHT_HAND_CORNER,
            VR::SL,
            PrimitiveValue::from(bottom_right),
        ),
        text(tags::PRESENTATION_SIZE_MODE, VR::CS, "SCALE TO FIT"),
        DataElement::new(
            tags::PRESENTATION_PIXEL_ASPECT_RATIO,
            VR::IS,
            PrimitiveValue::Strs(vec!["1".to_string(), "1".to_string()].into()),
        ),
    ]);

    let mut profile = pristine_profile.to_vec();
    if matches!(mutation, Mutation::IccCorruption) {
        profile[100] ^= 0x01;
    }
    let mut object = InMemDicomObject::from_element_iter([
        text(tags::SOP_CLASS_UID, VR::UI, SOP_CLASS_UID),
        text(tags::SOP_INSTANCE_UID, VR::UI, SOP_INSTANCE_UID),
        text(tags::SYNTHETIC_DATA, VR::CS, "YES"),
        text(tags::PATIENT_NAME, VR::PN, "DICOMTEST^SMOKE"),
        text(tags::PATIENT_ID, VR::LO, "DICOMTEST-SMOKE-001"),
        text(tags::PATIENT_BIRTH_DATE, VR::DA, "19700101"),
        text(tags::PATIENT_SEX, VR::CS, "O"),
        text(tags::STUDY_INSTANCE_UID, VR::UI, STUDY_UID),
        text(tags::STUDY_DATE, VR::DA, "20260101"),
        text(tags::STUDY_TIME, VR::TM, "000000"),
        text(tags::REFERRING_PHYSICIAN_NAME, VR::PN, ""),
        text(tags::STUDY_ID, VR::SH, "SMOKE"),
        text(tags::ACCESSION_NUMBER, VR::SH, ""),
        text(tags::MODALITY, VR::CS, "PR"),
        text(tags::SERIES_INSTANCE_UID, VR::UI, SERIES_UID),
        text(tags::SERIES_NUMBER, VR::IS, "62"),
        text(tags::BODY_PART_EXAMINED, VR::CS, "HAND"),
        text(tags::LATERALITY, VR::CS, "R"),
        text(tags::MANUFACTURER, VR::LO, "dicom-test-suite"),
        text(
            tags::MANUFACTURER_MODEL_NAME,
            VR::LO,
            "Native Color Softcopy Presentation State",
        ),
        text(tags::DEVICE_SERIAL_NUMBER, VR::LO, "DTS-COLOR-PR-0001"),
        text(tags::SOFTWARE_VERSIONS, VR::LO, crate::PACKAGE_VERSION),
        text(tags::INSTANCE_NUMBER, VR::IS, "1"),
        text(tags::CONTENT_DATE, VR::DA, "20260101"),
        text(tags::CONTENT_TIME, VR::TM, "000000"),
        text(tags::PRESENTATION_CREATION_DATE, VR::DA, "20260101"),
        text(tags::PRESENTATION_CREATION_TIME, VR::TM, "000000"),
        text(tags::CONTENT_LABEL, VR::CS, "DTSCOLORPR"),
        text(
            tags::CONTENT_DESCRIPTION,
            VR::LO,
            "Synthetic RGB color presentation state",
        ),
        text(tags::CONTENT_CREATOR_NAME, VR::PN, "DTS^Generator"),
        sequence(
            tags::REFERENCED_SERIES_SEQUENCE,
            vec![InMemDicomObject::from_element_iter([
                text(tags::SERIES_INSTANCE_UID, VR::UI, referenced_series_uid),
                sequence(tags::REFERENCED_IMAGE_SEQUENCE, vec![referenced_image]),
            ])],
        ),
        sequence(
            tags::DISPLAYED_AREA_SELECTION_SEQUENCE,
            vec![displayed_area],
        ),
        DataElement::new(
            tags::ICC_PROFILE,
            VR::OB,
            PrimitiveValue::U8(profile.into()),
        ),
        text(tags::COLOR_SPACE, VR::CS, "SRGB"),
    ]);
    if matches!(mutation, Mutation::Graphics) {
        object.put(sequence(tags::GRAPHIC_ANNOTATION_SEQUENCE, vec![]));
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
        "dts-color-softcopy-validation-{label}-{}",
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

fn locked_test_icc_profile() -> Vec<u8> {
    let mut bytes = vec![0_u8; 736];
    let profile_size = bytes.len() as u32;
    bytes[0..4].copy_from_slice(&profile_size.to_be_bytes());
    bytes[12..16].copy_from_slice(b"scnr");
    bytes[16..20].copy_from_slice(b"RGB ");
    bytes[20..24].copy_from_slice(b"XYZ ");
    bytes[36..40].copy_from_slice(b"acsp");
    bytes
}

fn text(tag: dicom_core::Tag, vr: VR, value: &str) -> DataElement<InMemDicomObject> {
    DataElement::new(tag, vr, value)
}

fn sequence(tag: dicom_core::Tag, items: Vec<InMemDicomObject>) -> DataElement<InMemDicomObject> {
    DataElement::new(tag, VR::SQ, DataSetSequence::from(items))
}

fn cleanup(path: PathBuf) {
    fs::remove_file(&path).unwrap();
    fs::remove_dir(path.parent().unwrap()).unwrap();
}
