use dicom_core::{Tag, VR, value::Value as DicomValue};
use dicom_dictionary_std::{tags, uids};
use dicom_object::InMemDicomObject;
use dicom_transfer_syntax_registry::{TransferSyntaxIndex, TransferSyntaxRegistry};

use super::rt_image::{
    RT_IMAGE_LABEL, RT_IMAGE_OUTPUT_FILE, RT_IMAGE_PIXEL_BYTES, RT_IMAGE_PIXEL_SHA256,
    RT_IMAGE_SERIES_NUMBER, RT_IMAGE_STORAGE_UID, RT_PLAN_STORAGE_UID, RtImageInput,
    build_rt_image,
};

const STUDY_UID: &str = "2.25.320000000000000000000000000000000000001";
const FRAME_UID: &str = "2.25.320000000000000000000000000000000000002";
const SERIES_UID: &str = "2.25.320000000000000000000000000000000000003";
const SOP_UID: &str = "2.25.320000000000000000000000000000000000004";
const PLAN_SOP_UID: &str = "2.25.320000000000000000000000000000000000005";

fn locked_input() -> RtImageInput<'static> {
    RtImageInput {
        study_instance_uid: STUDY_UID,
        frame_of_reference_uid: FRAME_UID,
        series_instance_uid: SERIES_UID,
        sop_instance_uid: SOP_UID,
        plan_sop_class_uid: RT_PLAN_STORAGE_UID,
        plan_sop_instance_uid: PLAN_SOP_UID,
    }
}

#[test]
fn rt_image_builds_locked_identity_and_mandatory_metadata() {
    let object = build_rt_image(locked_input()).expect("locked RT Image input");
    assert_eq!(RT_IMAGE_OUTPUT_FILE, "instance.dcm");
    for (tag, vr, expected) in [
        (tags::SOP_CLASS_UID, VR::UI, RT_IMAGE_STORAGE_UID),
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
        (tags::SERIES_NUMBER, VR::IS, RT_IMAGE_SERIES_NUMBER),
        (tags::OPERATORS_NAME, VR::PN, ""),
        (tags::FRAME_OF_REFERENCE_UID, VR::UI, FRAME_UID),
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
    ] {
        assert_text(&object, tag, vr, expected);
    }
}

#[test]
fn rt_image_links_exact_plan_beam_and_fraction_once() {
    let object = build_rt_image(locked_input()).expect("locked RT Image input");
    let references = sequence(&object, tags::REFERENCED_RT_PLAN_SEQUENCE, 1);
    assert_eq!(references[0].iter().count(), 4);
    for (tag, vr, expected) in [
        (tags::REFERENCED_SOP_CLASS_UID, VR::UI, RT_PLAN_STORAGE_UID),
        (tags::REFERENCED_SOP_INSTANCE_UID, VR::UI, PLAN_SOP_UID),
        (tags::REFERENCED_BEAM_NUMBER, VR::IS, "1"),
        (tags::REFERENCED_FRACTION_GROUP_NUMBER, VR::IS, "1"),
    ] {
        assert_text(&references[0], tag, vr, expected);
    }
    assert_text(&object, tags::FRACTION_NUMBER, VR::IS, "1");
}

#[test]
fn rt_image_builds_exact_geometry_and_native_gradient() {
    let object = build_rt_image(locked_input()).expect("locked RT Image input");
    for (tag, vr, expected) in [
        (tags::RT_IMAGE_LABEL, VR::SH, RT_IMAGE_LABEL),
        (tags::RT_IMAGE_PLANE, VR::CS, "NORMAL"),
        (tags::X_RAY_IMAGE_RECEPTOR_ANGLE, VR::DS, "0"),
        (tags::IMAGE_PLANE_PIXEL_SPACING, VR::DS, "1\\1"),
        (tags::RT_IMAGE_POSITION, VR::DS, "-1.5\\1.5"),
        (tags::RADIATION_MACHINE_NAME, VR::SH, "DTS_LINAC"),
        (tags::RADIATION_MACHINE_SAD, VR::DS, "1000"),
        (tags::RT_IMAGE_SID, VR::DS, "1500"),
        (tags::PRIMARY_DOSIMETER_UNIT, VR::CS, "MU"),
        (tags::PHOTOMETRIC_INTERPRETATION, VR::CS, "MONOCHROME2"),
    ] {
        assert_text(&object, tag, vr, expected);
    }
    for (tag, expected) in [
        (tags::SAMPLES_PER_PIXEL, 1),
        (tags::ROWS, 4),
        (tags::COLUMNS, 4),
        (tags::BITS_ALLOCATED, 8),
        (tags::BITS_STORED, 8),
        (tags::HIGH_BIT, 7),
        (tags::PIXEL_REPRESENTATION, 0),
    ] {
        assert_u16(&object, tag, expected);
    }

    let expected = std::array::from_fn::<_, 16, _>(|index| {
        let row = index / 4;
        let column = index % 4;
        (17 * (4 * row + column)) as u8
    });
    assert_eq!(RT_IMAGE_PIXEL_BYTES, expected);
    assert_eq!(crate::sha256_hex(&expected), RT_IMAGE_PIXEL_SHA256);
    let pixel_data = object.element(tags::PIXEL_DATA).expect("Pixel Data");
    assert_eq!(pixel_data.vr(), VR::OB);
    let bytes = pixel_data.to_bytes().expect("native pixel bytes");
    assert_eq!(bytes.as_ref(), expected);
    assert_eq!(bytes.len(), 16, "even native OB value needs no padding");
    assert!(matches!(pixel_data.value(), DicomValue::Primitive(_)));
}

#[test]
fn rt_image_omits_locked_conditional_modules_and_attributes() {
    let object = build_rt_image(locked_input()).expect("locked RT Image input");
    for tag in [
        tags::ADDITIONAL_PATIENT_HISTORY,
        tags::CONTRAST_BOLUS_AGENT,
        tags::CINE_RATE,
        tags::NUMBER_OF_FRAMES,
        tags::FRAME_INCREMENT_POINTER,
        tags::RESCALE_INTERCEPT,
        tags::RESCALE_SLOPE,
        tags::WINDOW_CENTER,
        tags::WINDOW_WIDTH,
        tags::APPROVAL_STATUS,
        tags::FRAME_EXTRACTION_SEQUENCE,
        tags::STUDIES_CONTAINING_OTHER_REFERENCED_INSTANCES_SEQUENCE,
        tags::REFERENCED_SERIES_SEQUENCE,
        tags::REPORTED_VALUES_ORIGIN,
        tags::RT_IMAGE_ORIENTATION,
        tags::ISOCENTER_POSITION,
        tags::PATIENT_POSITION,
        tags::FLUENCE_MAP_SEQUENCE,
        tags::EXPOSURE_SEQUENCE,
        tags::LOSSY_IMAGE_COMPRESSION,
        tags::LOSSY_IMAGE_COMPRESSION_RATIO,
        tags::LOSSY_IMAGE_COMPRESSION_METHOD,
        Tag(0x6000, 0x3000),
    ] {
        assert!(object.element(tag).is_err(), "{tag:?} must be absent");
    }
}

#[test]
fn rt_image_rejects_wrong_plan_class_and_invalid_or_reused_uids() {
    let mut wrong_class = locked_input();
    wrong_class.plan_sop_class_uid = RT_IMAGE_STORAGE_UID;
    assert!(
        build_rt_image(wrong_class)
            .unwrap_err()
            .contains("Plan SOP Class UID")
    );
    for input in [
        RtImageInput {
            study_instance_uid: "",
            ..locked_input()
        },
        RtImageInput {
            frame_of_reference_uid: "1.2.bad",
            ..locked_input()
        },
        RtImageInput {
            series_instance_uid: ".1.2",
            ..locked_input()
        },
        RtImageInput {
            sop_instance_uid: "1.2.",
            ..locked_input()
        },
        RtImageInput {
            plan_sop_instance_uid: "1.02.3",
            ..locked_input()
        },
    ] {
        assert!(build_rt_image(input).is_err());
    }

    let roles = [STUDY_UID, FRAME_UID, SERIES_UID, SOP_UID, PLAN_SOP_UID];
    for left in 0..roles.len() {
        for right in left + 1..roles.len() {
            let mut values = roles;
            values[right] = values[left];
            let error = build_rt_image(RtImageInput {
                study_instance_uid: values[0],
                frame_of_reference_uid: values[1],
                series_instance_uid: values[2],
                sop_instance_uid: values[3],
                plan_sop_class_uid: RT_PLAN_STORAGE_UID,
                plan_sop_instance_uid: values[4],
            })
            .unwrap_err();
            assert!(
                error.contains("must be distinct"),
                "{left}, {right}: {error}"
            );
        }
    }
}

#[test]
fn rt_image_dataset_serialization_is_byte_deterministic() {
    let transfer_syntax = TransferSyntaxRegistry
        .get(uids::EXPLICIT_VR_LITTLE_ENDIAN)
        .expect("Explicit VR Little Endian transfer syntax");
    let mut first = Vec::new();
    let mut second = Vec::new();
    build_rt_image(locked_input())
        .expect("first object")
        .write_dataset_with_ts(&mut first, transfer_syntax)
        .expect("first serialization");
    build_rt_image(locked_input())
        .expect("second object")
        .write_dataset_with_ts(&mut second, transfer_syntax)
        .expect("second serialization");
    assert!(!first.is_empty());
    assert_eq!(first, second);
    assert_eq!(crate::sha256_hex(&first), crate::sha256_hex(&second));
}

fn sequence(object: &InMemDicomObject, tag: Tag, expected_len: usize) -> &[InMemDicomObject] {
    let element = object.element(tag).expect("sequence");
    assert_eq!(element.vr(), VR::SQ, "{tag:?}");
    let items = element.items().expect("sequence items");
    assert_eq!(items.len(), expected_len, "{tag:?}");
    items
}

fn assert_text(object: &InMemDicomObject, tag: Tag, expected_vr: VR, expected: &str) {
    let element = object.element(tag).expect("attribute");
    assert_eq!(element.vr(), expected_vr, "{tag:?}");
    assert_eq!(
        element.to_str().expect("text").as_ref(),
        expected,
        "{tag:?}"
    );
}

fn assert_u16(object: &InMemDicomObject, tag: Tag, expected: u16) {
    let element = object.element(tag).expect("US attribute");
    assert_eq!(element.vr(), VR::US, "{tag:?}");
    assert_eq!(element.to_int::<u16>().expect("US"), expected, "{tag:?}");
}
