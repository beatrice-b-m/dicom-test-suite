use std::path::PathBuf;

use dicom_core::{DataElement, PrimitiveValue, Tag, VR, value::DataSetSequence};
use dicom_dictionary_std::{tags, uids};
use dicom_object::{FileMetaTableBuilder, InMemDicomObject};

use super::{RtImageExpectations, validate_rt_image_file};
use crate::rt_manifest::{LinkedRtImageInput, linked_rt_image_expected};

const SOP_UID: &str = "2.25.601";
const STUDY_UID: &str = "2.25.602";
const SERIES_UID: &str = "2.25.603";
const FOR_UID: &str = "2.25.604";
const PLAN_SERIES_UID: &str = "2.25.605";
const PLAN_SOP_UID: &str = "2.25.606";
const IMPLEMENTATION_UID: &str = "2.25.999";
const IMAGE_CLASS: &str = "1.2.840.10008.5.1.4.1.1.481.1";
const PLAN_CLASS: &str = "1.2.840.10008.5.1.4.1.1.481.5";
const PIXELS: [u8; 16] = [
    0, 17, 34, 51, 68, 85, 102, 119, 136, 153, 170, 187, 204, 221, 238, 255,
];

#[derive(Clone, Copy, Debug)]
enum Mutation {
    None,
    MissingImageType,
    MissingLabel,
    MissingPlane,
    NonNormalWithoutOrientation,
    PortalWithoutReportedValuesOrigin,
    WrongPlanClass,
    WrongPlanUid,
    WrongBeam,
    WrongReferencedFraction,
    WrongFractionNumber,
    DuplicatePlanReference,
    WrongRows,
    ShortPayload,
    BitsStored,
    HighBit,
    PixelRepresentation,
    Spacing,
    Position,
    Sad,
    Sid,
    PixelByte,
    WrongStudy,
    WrongFrameOfReference,
}

#[test]
fn accepts_exact_linked_rt_image() {
    let path = write_fixture("valid", Mutation::None);
    let validated = validate_rt_image_file(&path, &expectations()).expect("valid linked RT Image");
    assert_eq!(validated.validation["status"], "passed");
    assert_eq!(
        validated.validation["standards"][0]["name"],
        "rt_image_sop_class"
    );
    std::fs::remove_file(path).ok();
}

#[test]
fn rejects_every_locked_rt_image_mutation() {
    for (label, mutation, finding) in [
        ("missing-image-type", Mutation::MissingImageType, None),
        ("missing-label", Mutation::MissingLabel, None),
        ("missing-plane", Mutation::MissingPlane, None),
        (
            "non-normal",
            Mutation::NonNormalWithoutOrientation,
            Some("rt_image_non_normal_orientation"),
        ),
        (
            "portal",
            Mutation::PortalWithoutReportedValuesOrigin,
            Some("rt_image_portal_reported_values_origin"),
        ),
        (
            "wrong-plan-class",
            Mutation::WrongPlanClass,
            Some("rt_image_plan_sop_class_uid"),
        ),
        (
            "wrong-plan-uid",
            Mutation::WrongPlanUid,
            Some("rt_image_plan_sop_instance_uid"),
        ),
        (
            "wrong-beam",
            Mutation::WrongBeam,
            Some("rt_image_referenced_beam_number"),
        ),
        (
            "wrong-ref-fraction",
            Mutation::WrongReferencedFraction,
            Some("rt_image_referenced_fraction_group_number"),
        ),
        (
            "wrong-fraction-number",
            Mutation::WrongFractionNumber,
            Some("rt_image_fraction_number"),
        ),
        (
            "duplicate-plan",
            Mutation::DuplicatePlanReference,
            Some("rt_image_plan_reference_count"),
        ),
        ("wrong-rows", Mutation::WrongRows, Some("rt_image_rows")),
        (
            "short-payload",
            Mutation::ShortPayload,
            Some("rt_image_pixel_length"),
        ),
        (
            "bits-stored",
            Mutation::BitsStored,
            Some("rt_image_bits_stored"),
        ),
        ("high-bit", Mutation::HighBit, Some("rt_image_high_bit")),
        (
            "pixel-representation",
            Mutation::PixelRepresentation,
            Some("rt_image_pixel_representation"),
        ),
        ("spacing", Mutation::Spacing, Some("rt_image_pixel_spacing")),
        ("position", Mutation::Position, Some("rt_image_position")),
        ("sad", Mutation::Sad, Some("rt_image_sad")),
        ("sid", Mutation::Sid, Some("rt_image_sid")),
        (
            "pixel-byte",
            Mutation::PixelByte,
            Some("rt_image_pixel_values"),
        ),
        (
            "wrong-study",
            Mutation::WrongStudy,
            Some("rt_image_study_instance_uid"),
        ),
        (
            "wrong-for",
            Mutation::WrongFrameOfReference,
            Some("rt_image_frame_of_reference_uid"),
        ),
    ] {
        let path = write_fixture(label, mutation);
        let error = validate_rt_image_file(&path, &expectations())
            .expect_err("mutation must fail")
            .to_string();
        if let Some(finding) = finding {
            assert!(error.contains(finding), "{label}: {error}");
        }
        std::fs::remove_file(path).ok();
    }
}

#[test]
fn rejects_every_locked_absence_category() {
    for (label, tag, vr, value, finding) in [
        (
            "patient-study",
            tags::PATIENT_AGE,
            VR::AS,
            "050Y",
            "rt_image_patient_study_module_absent",
        ),
        (
            "contrast",
            tags::CONTRAST_BOLUS_AGENT,
            VR::LO,
            "NONE",
            "rt_image_contrast_bolus_module_absent",
        ),
        (
            "cine",
            tags::CINE_RATE,
            VR::IS,
            "1",
            "rt_image_cine_module_absent",
        ),
        (
            "multiframe",
            tags::NUMBER_OF_FRAMES,
            VR::IS,
            "1",
            "rt_image_multi_frame_module_absent",
        ),
        (
            "approval",
            tags::APPROVAL_STATUS,
            VR::CS,
            "UNAPPROVED",
            "rt_image_approval_module_absent",
        ),
        (
            "clinical-trial",
            tags::CLINICAL_TRIAL_SPONSOR_NAME,
            VR::LO,
            "DTS",
            "rt_image_clinical_trial_module_absent",
        ),
        (
            "reported-origin",
            tags::REPORTED_VALUES_ORIGIN,
            VR::CS,
            "ACTUAL",
            "rt_image_reported_values_origin_absent",
        ),
        (
            "orientation",
            tags::RT_IMAGE_ORIENTATION,
            VR::DS,
            "1\\0\\0\\0\\1\\0",
            "rt_image_rt_image_orientation_absent",
        ),
        (
            "isocenter",
            tags::ISOCENTER_POSITION,
            VR::DS,
            "0\\0\\0",
            "rt_image_isocenter_position_absent",
        ),
        (
            "patient-position",
            tags::PATIENT_POSITION,
            VR::CS,
            "HFS",
            "rt_image_patient_position_absent",
        ),
        (
            "lossy",
            tags::LOSSY_IMAGE_COMPRESSION,
            VR::CS,
            "01",
            "rt_image_lossy_pixel_attributes_absent",
        ),
    ] {
        let mut object = valid_object();
        put_str(&mut object, tag, vr, value);
        assert_absence_rejected(label, object, finding);
    }
    for (label, tag, finding) in [
        (
            "modality-lut",
            tags::MODALITY_LUT_SEQUENCE,
            "rt_image_modality_lut_module_absent",
        ),
        (
            "voi-lut",
            tags::VOILUT_SEQUENCE,
            "rt_image_voi_lut_module_absent",
        ),
        (
            "frame-extraction",
            tags::FRAME_EXTRACTION_SEQUENCE,
            "rt_image_frame_extraction_module_absent",
        ),
        (
            "common-reference",
            tags::REFERENCED_SERIES_SEQUENCE,
            "rt_image_common_instance_reference_module_absent",
        ),
        (
            "fluence",
            tags::FLUENCE_MAP_SEQUENCE,
            "rt_image_fluence_map_sequence_absent",
        ),
        (
            "exposure",
            tags::EXPOSURE_SEQUENCE,
            "rt_image_exposure_sequence_absent",
        ),
    ] {
        let mut object = valid_object();
        put_sequence(&mut object, tag, vec![]);
        assert_absence_rejected(label, object, finding);
    }
    let mut overlay = valid_object();
    overlay.put(DataElement::new(
        Tag(0x6000, 0x3000),
        VR::OW,
        PrimitiveValue::from([0_u8, 0].as_slice()),
    ));
    assert_absence_rejected("overlay", overlay, "rt_image_overlays_absent");
    let mut encapsulated = valid_object();
    encapsulated.put(DataElement::new(
        tags::PIXEL_DATA,
        VR::OW,
        PrimitiveValue::from(PIXELS.as_slice()),
    ));
    assert_absence_rejected("encapsulated", encapsulated, "rt_image_pixel_vr");
}

#[test]
fn rejects_manifest_identity_storage_and_absence_drift() {
    let path = write_fixture("manifest", Mutation::None);
    let mut expected = expectations();
    expected.expected_rt_image.plan_reference.study_instance_uid = "2.25.777";
    assert!(
        validate_rt_image_file(&path, &expected)
            .unwrap_err()
            .to_string()
            .contains("rt_image_manifest_shared_identity")
    );
    let mut expected = expectations();
    expected.expected_rt_image.plan_reference.source_sha256 = "BAD";
    assert!(
        validate_rt_image_file(&path, &expected)
            .unwrap_err()
            .to_string()
            .contains("rt_image_manifest_source_hash")
    );
    let mut expected = expectations();
    expected.expected_rt_image.storage.pixel_value_formula = "r + c";
    assert!(
        validate_rt_image_file(&path, &expected)
            .unwrap_err()
            .to_string()
            .contains("rt_image_manifest_storage_contract")
    );
    let mut expected = expectations();
    expected.expected_rt_image.absent_content.overlays = false;
    assert!(
        validate_rt_image_file(&path, &expected)
            .unwrap_err()
            .to_string()
            .contains("rt_image_manifest_absence_contract")
    );
    std::fs::remove_file(path).ok();
}

fn expectations() -> RtImageExpectations<'static> {
    RtImageExpectations {
        implementation_class_uid: IMPLEMENTATION_UID,
        synthetic_data: "YES",
        expected_rt_image: linked_rt_image_expected(LinkedRtImageInput {
            sop_instance_uid: SOP_UID,
            study_instance_uid: STUDY_UID,
            series_instance_uid: SERIES_UID,
            frame_of_reference_uid: FOR_UID,
            plan_series_instance_uid: PLAN_SERIES_UID,
            plan_sop_instance_uid: PLAN_SOP_UID,
            plan_sha256: "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
        }),
    }
}

fn write_fixture(label: &str, mutation: Mutation) -> PathBuf {
    let mut object = valid_object();
    apply_mutation(&mut object, mutation);
    write_object(label, object)
}

fn write_object(label: &str, object: InMemDicomObject) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "dicom-test-suite-rt-image-validation-{}-{label}.dcm",
        std::process::id()
    ));
    object
        .with_meta(
            FileMetaTableBuilder::new()
                .media_storage_sop_class_uid(IMAGE_CLASS)
                .media_storage_sop_instance_uid(SOP_UID)
                .transfer_syntax(uids::EXPLICIT_VR_LITTLE_ENDIAN)
                .implementation_class_uid(IMPLEMENTATION_UID),
        )
        .expect("file meta")
        .write_to_file(&path)
        .expect("write fixture");
    path
}

fn assert_absence_rejected(label: &str, object: InMemDicomObject, finding: &str) {
    let path = write_object(label, object);
    let error = validate_rt_image_file(&path, &expectations())
        .expect_err("presence must fail")
        .to_string();
    assert!(error.contains(finding), "{label}: {error}");
    std::fs::remove_file(path).ok();
}

fn valid_object() -> InMemDicomObject {
    let mut object = InMemDicomObject::new_empty();
    for (tag, vr, value) in [
        (tags::SOP_CLASS_UID, VR::UI, IMAGE_CLASS),
        (tags::SOP_INSTANCE_UID, VR::UI, SOP_UID),
        (tags::SYNTHETIC_DATA, VR::CS, "YES"),
        (tags::PATIENT_NAME, VR::PN, "DTS^Synthetic^Patient001"),
        (tags::PATIENT_ID, VR::LO, "DTS-PATIENT-001"),
        (tags::PATIENT_BIRTH_DATE, VR::DA, "19700101"),
        (tags::PATIENT_SEX, VR::CS, "O"),
        (tags::STUDY_INSTANCE_UID, VR::UI, STUDY_UID),
        (tags::STUDY_DATE, VR::DA, "20260101"),
        (tags::STUDY_TIME, VR::TM, "000000"),
        (tags::REFERRING_PHYSICIAN_NAME, VR::PN, ""),
        (tags::STUDY_ID, VR::SH, "DTS-RTSTRUCT"),
        (tags::ACCESSION_NUMBER, VR::SH, ""),
        (tags::MODALITY, VR::CS, "RTIMAGE"),
        (tags::SERIES_INSTANCE_UID, VR::UI, SERIES_UID),
        (tags::SERIES_NUMBER, VR::IS, "73"),
        (tags::OPERATORS_NAME, VR::PN, ""),
        (tags::FRAME_OF_REFERENCE_UID, VR::UI, FOR_UID),
        (tags::POSITION_REFERENCE_INDICATOR, VR::LO, ""),
        (tags::MANUFACTURER, VR::LO, "dicom-test-suite"),
        (tags::INSTITUTION_NAME, VR::LO, ""),
        (tags::INSTITUTION_ADDRESS, VR::ST, ""),
        (
            tags::MANUFACTURER_MODEL_NAME,
            VR::LO,
            "Native Linked RT Image",
        ),
        (tags::DEVICE_SERIAL_NUMBER, VR::LO, "DTS-RTIMAGE-001"),
        (tags::SOFTWARE_VERSIONS, VR::LO, crate::PACKAGE_VERSION),
        (tags::ACQUISITION_DATE, VR::DA, "20260101"),
        (tags::ACQUISITION_TIME, VR::TM, "000000"),
        (tags::IMAGE_TYPE, VR::CS, "DERIVED\\SECONDARY\\DRR"),
        (tags::CONVERSION_TYPE, VR::CS, "WSD"),
        (tags::INSTANCE_NUMBER, VR::IS, "1"),
        (tags::CONTENT_DATE, VR::DA, "20260101"),
        (tags::CONTENT_TIME, VR::TM, "000000"),
        (tags::PHOTOMETRIC_INTERPRETATION, VR::CS, "MONOCHROME2"),
        (tags::RT_IMAGE_LABEL, VR::SH, "DTS_DRR"),
        (tags::RT_IMAGE_PLANE, VR::CS, "NORMAL"),
        (tags::X_RAY_IMAGE_RECEPTOR_ANGLE, VR::DS, "0"),
        (tags::IMAGE_PLANE_PIXEL_SPACING, VR::DS, "1\\1"),
        (tags::RT_IMAGE_POSITION, VR::DS, "-1.5\\1.5"),
        (tags::RADIATION_MACHINE_NAME, VR::SH, "DTS_LINAC"),
        (tags::RADIATION_MACHINE_SAD, VR::DS, "1000"),
        (tags::RT_IMAGE_SID, VR::DS, "1500"),
        (tags::PRIMARY_DOSIMETER_UNIT, VR::CS, "MU"),
        (tags::FRACTION_NUMBER, VR::IS, "1"),
    ] {
        put_str(&mut object, tag, vr, value);
    }
    for (tag, value) in [
        (tags::SAMPLES_PER_PIXEL, 1),
        (tags::ROWS, 4),
        (tags::COLUMNS, 4),
        (tags::BITS_ALLOCATED, 8),
        (tags::BITS_STORED, 8),
        (tags::HIGH_BIT, 7),
        (tags::PIXEL_REPRESENTATION, 0),
    ] {
        object.put(DataElement::new(tag, VR::US, PrimitiveValue::from(value)));
    }
    let mut plan = InMemDicomObject::new_empty();
    for (tag, vr, value) in [
        (tags::REFERENCED_BEAM_NUMBER, VR::IS, "1"),
        (tags::REFERENCED_FRACTION_GROUP_NUMBER, VR::IS, "1"),
        (tags::REFERENCED_SOP_CLASS_UID, VR::UI, PLAN_CLASS),
        (tags::REFERENCED_SOP_INSTANCE_UID, VR::UI, PLAN_SOP_UID),
    ] {
        put_str(&mut plan, tag, vr, value);
    }
    put_sequence(&mut object, tags::REFERENCED_RT_PLAN_SEQUENCE, vec![plan]);
    object.put(DataElement::new(
        tags::PIXEL_DATA,
        VR::OB,
        PrimitiveValue::from(PIXELS.as_slice()),
    ));
    object
}

fn apply_mutation(object: &mut InMemDicomObject, mutation: Mutation) {
    match mutation {
        Mutation::None => {}
        Mutation::MissingImageType => {
            object.take_element(tags::IMAGE_TYPE).unwrap();
        }
        Mutation::MissingLabel => {
            object.take_element(tags::RT_IMAGE_LABEL).unwrap();
        }
        Mutation::MissingPlane => {
            object.take_element(tags::RT_IMAGE_PLANE).unwrap();
        }
        Mutation::NonNormalWithoutOrientation => {
            put_str(object, tags::RT_IMAGE_PLANE, VR::CS, "NON_NORMAL")
        }
        Mutation::PortalWithoutReportedValuesOrigin => put_str(
            object,
            tags::IMAGE_TYPE,
            VR::CS,
            "DERIVED\\SECONDARY\\PORTAL",
        ),
        Mutation::WrongPlanClass => {
            mutate_plan(object, tags::REFERENCED_SOP_CLASS_UID, VR::UI, IMAGE_CLASS)
        }
        Mutation::WrongPlanUid => mutate_plan(
            object,
            tags::REFERENCED_SOP_INSTANCE_UID,
            VR::UI,
            "2.25.800",
        ),
        Mutation::WrongBeam => mutate_plan(object, tags::REFERENCED_BEAM_NUMBER, VR::IS, "2"),
        Mutation::WrongReferencedFraction => {
            mutate_plan(object, tags::REFERENCED_FRACTION_GROUP_NUMBER, VR::IS, "2")
        }
        Mutation::WrongFractionNumber => put_str(object, tags::FRACTION_NUMBER, VR::IS, "2"),
        Mutation::DuplicatePlanReference => {
            let mut sequence = object
                .take_element(tags::REFERENCED_RT_PLAN_SEQUENCE)
                .unwrap();
            let duplicate = sequence.items().unwrap()[0].clone();
            sequence.items_mut().unwrap().push(duplicate);
            object.put(sequence);
        }
        Mutation::WrongRows => {
            object.put(DataElement::new(
                tags::ROWS,
                VR::US,
                PrimitiveValue::from(5_u16),
            ));
        }
        Mutation::ShortPayload => {
            object.put(DataElement::new(
                tags::PIXEL_DATA,
                VR::OB,
                PrimitiveValue::from(&PIXELS[..14]),
            ));
        }
        Mutation::BitsStored => {
            object.put(DataElement::new(
                tags::BITS_STORED,
                VR::US,
                PrimitiveValue::from(7_u16),
            ));
        }
        Mutation::HighBit => {
            object.put(DataElement::new(
                tags::HIGH_BIT,
                VR::US,
                PrimitiveValue::from(6_u16),
            ));
        }
        Mutation::PixelRepresentation => {
            object.put(DataElement::new(
                tags::PIXEL_REPRESENTATION,
                VR::US,
                PrimitiveValue::from(1_u16),
            ));
        }
        Mutation::Spacing => put_str(object, tags::IMAGE_PLANE_PIXEL_SPACING, VR::DS, "1\\2"),
        Mutation::Position => put_str(object, tags::RT_IMAGE_POSITION, VR::DS, "-1\\1.5"),
        Mutation::Sad => put_str(object, tags::RADIATION_MACHINE_SAD, VR::DS, "999"),
        Mutation::Sid => put_str(object, tags::RT_IMAGE_SID, VR::DS, "1499"),
        Mutation::PixelByte => {
            let mut pixels = PIXELS;
            pixels[0] = 1;
            object.put(DataElement::new(
                tags::PIXEL_DATA,
                VR::OB,
                PrimitiveValue::from(pixels.as_slice()),
            ));
        }
        Mutation::WrongStudy => put_str(object, tags::STUDY_INSTANCE_UID, VR::UI, "2.25.700"),
        Mutation::WrongFrameOfReference => {
            put_str(object, tags::FRAME_OF_REFERENCE_UID, VR::UI, "2.25.701")
        }
    }
}

fn mutate_plan(object: &mut InMemDicomObject, tag: Tag, vr: VR, value: &str) {
    let mut sequence = object
        .take_element(tags::REFERENCED_RT_PLAN_SEQUENCE)
        .unwrap();
    put_str(&mut sequence.items_mut().unwrap()[0], tag, vr, value);
    object.put(sequence);
}

fn put_str(object: &mut InMemDicomObject, tag: Tag, vr: VR, value: &str) {
    object.put(DataElement::new(tag, vr, value));
}

fn put_sequence(object: &mut InMemDicomObject, tag: Tag, items: Vec<InMemDicomObject>) {
    object.put(DataElement::new(tag, VR::SQ, DataSetSequence::from(items)));
}
