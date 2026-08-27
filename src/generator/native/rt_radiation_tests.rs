use dicom_core::{Tag, VR, header::HasLength};
use dicom_dictionary_std::{tags, uids};
use dicom_object::InMemDicomObject;
use dicom_transfer_syntax_registry::{TransferSyntaxIndex, TransferSyntaxRegistry};

use super::rt_radiation::{
    C_ARM_PHOTON_ELECTRON_RADIATION_STORAGE_UID, EQUIPMENT_FRAME_OF_REFERENCE_UID, RT_DEVICE_LABEL,
    RT_DEVICE_SERIAL_NUMBER, RT_PLAN_STORAGE_UID, RT_RADIATION_OUTPUT_FILE,
    RT_RADIATION_SERIES_NUMBER, RT_RADIATION_SET_OUTPUT_FILE, RT_RADIATION_SET_SERIES_NUMBER,
    RT_RADIATION_SET_STORAGE_UID, RT_RADIATION_SET_USER_CONTENT_LABEL,
    RT_RADIATION_USER_CONTENT_LABEL, RT_TREATMENT_POSITION_GROUP_LABEL, RtRadiationInput,
    RtRadiationSetInput, build_rt_radiation, build_rt_radiation_set,
};

const STUDY_UID: &str = "2.25.320000000000000000000000000000000000001";
const FRAME_OF_REFERENCE_UID: &str = "2.25.320000000000000000000000000000000000002";
const PLAN_SERIES_UID: &str = "2.25.320000000000000000000000000000000000003";
const PLAN_SOP_UID: &str = "2.25.320000000000000000000000000000000000004";
const RADIATION_SERIES_UID: &str = "2.25.320000000000000000000000000000000000005";
const RADIATION_SOP_UID: &str = "2.25.320000000000000000000000000000000000006";
const SET_SERIES_UID: &str = "2.25.320000000000000000000000000000000000007";
const SET_SOP_UID: &str = "2.25.320000000000000000000000000000000000008";
const POSITION_GROUP_UID: &str = "2.25.320000000000000000000000000000000000009";

fn radiation_input() -> RtRadiationInput<'static> {
    RtRadiationInput {
        study_instance_uid: STUDY_UID,
        frame_of_reference_uid: FRAME_OF_REFERENCE_UID,
        series_instance_uid: RADIATION_SERIES_UID,
        sop_instance_uid: RADIATION_SOP_UID,
        plan_series_instance_uid: PLAN_SERIES_UID,
        plan_sop_class_uid: RT_PLAN_STORAGE_UID,
        plan_sop_instance_uid: PLAN_SOP_UID,
    }
}

fn set_input() -> RtRadiationSetInput<'static> {
    RtRadiationSetInput {
        study_instance_uid: STUDY_UID,
        frame_of_reference_uid: FRAME_OF_REFERENCE_UID,
        series_instance_uid: SET_SERIES_UID,
        sop_instance_uid: SET_SOP_UID,
        plan_series_instance_uid: PLAN_SERIES_UID,
        plan_sop_class_uid: RT_PLAN_STORAGE_UID,
        plan_sop_instance_uid: PLAN_SOP_UID,
        radiation_series_instance_uid: RADIATION_SERIES_UID,
        radiation_sop_class_uid: C_ARM_PHOTON_ELECTRON_RADIATION_STORAGE_UID,
        radiation_sop_instance_uid: RADIATION_SOP_UID,
        treatment_position_group_uid: POSITION_GROUP_UID,
    }
}

#[test]
fn rt_radiation_builds_locked_identity_and_mandatory_metadata() {
    let object = build_rt_radiation(radiation_input()).expect("locked input");
    assert_eq!(RT_RADIATION_OUTPUT_FILE, "instance.dcm");
    for (tag, vr, expected) in [
        (
            tags::SOP_CLASS_UID,
            VR::UI,
            C_ARM_PHOTON_ELECTRON_RADIATION_STORAGE_UID,
        ),
        (tags::SOP_INSTANCE_UID, VR::UI, RADIATION_SOP_UID),
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
        (tags::MODALITY, VR::CS, "RTRAD"),
        (tags::SERIES_INSTANCE_UID, VR::UI, RADIATION_SERIES_UID),
        (tags::SERIES_NUMBER, VR::IS, RT_RADIATION_SERIES_NUMBER),
        (tags::INSTANCE_NUMBER, VR::IS, "1"),
        (tags::SERIES_DATE, VR::DA, "20260101"),
        (tags::SERIES_TIME, VR::TM, "000000"),
        (tags::FRAME_OF_REFERENCE_UID, VR::UI, FRAME_OF_REFERENCE_UID),
        (tags::POSITION_REFERENCE_INDICATOR, VR::LO, ""),
        (tags::MANUFACTURER, VR::LO, "dicom-test-suite"),
        (
            tags::MANUFACTURER_MODEL_NAME,
            VR::LO,
            "Native C-Arm Photon-Electron Radiation",
        ),
        (tags::DEVICE_SERIAL_NUMBER, VR::LO, RT_DEVICE_SERIAL_NUMBER),
        (tags::SOFTWARE_VERSIONS, VR::LO, crate::PACKAGE_VERSION),
        (tags::INSTANCE_CREATION_DATE, VR::DA, "20260101"),
        (tags::INSTANCE_CREATION_TIME, VR::TM, "000000"),
        (tags::CONTENT_DATE, VR::DA, "20260101"),
        (tags::CONTENT_TIME, VR::TM, "000000"),
        (
            tags::USER_CONTENT_LABEL,
            VR::SH,
            RT_RADIATION_USER_CONTENT_LABEL,
        ),
        (tags::CONTENT_DESCRIPTION, VR::LO, ""),
        (
            tags::RT_RADIATION_PHYSICAL_AND_GEOMETRIC_CONTENT_DETAIL_FLAG,
            VR::CS,
            "IDENT_ONLY",
        ),
        (tags::RT_RECORD_FLAG, VR::CS, "NO"),
        (
            tags::EQUIPMENT_FRAME_OF_REFERENCE_UID,
            VR::UI,
            EQUIPMENT_FRAME_OF_REFERENCE_UID,
        ),
    ] {
        assert_text(&object, tag, vr, expected);
    }
    assert_empty_sequence(&object, tags::AUTHOR_IDENTIFICATION_SEQUENCE);
    assert_empty_sequence(
        &object,
        tags::EQUIPMENT_REFERENCE_POINT_COORDINATES_SEQUENCE,
    );
    assert_u16(&object, tags::NUMBER_OF_PATIENT_SUPPORT_DEVICES, 0);
    assert_f64(&object, tags::RT_BEAM_MODIFIER_DEFINITION_DISTANCE, 500.0);
    assert_f64(&object, tags::RADIATION_SOURCE_AXIS_DISTANCE, 1000.0);
}

#[test]
fn rt_radiation_links_plan_as_definition_source_and_common_reference() {
    let object = build_rt_radiation(radiation_input()).expect("locked input");
    let sources = sequence(&object, tags::DEFINITION_SOURCE_SEQUENCE, 1);
    assert_eq!(sources[0].iter().count(), 3);
    assert_sop_reference(&sources[0], RT_PLAN_STORAGE_UID, PLAN_SOP_UID);
    assert_text(&sources[0], tags::REFERENCED_BEAM_NUMBER, VR::IS, "1");

    let series = sequence(&object, tags::REFERENCED_SERIES_SEQUENCE, 1);
    assert_eq!(series[0].iter().count(), 2);
    assert_text(
        &series[0],
        tags::SERIES_INSTANCE_UID,
        VR::UI,
        PLAN_SERIES_UID,
    );
    let instances = sequence(&series[0], tags::REFERENCED_INSTANCE_SEQUENCE, 1);
    assert_sop_reference(&instances[0], RT_PLAN_STORAGE_UID, PLAN_SOP_UID);
}

#[test]
fn rt_radiation_builds_locked_device_codes_and_position() {
    let object = build_rt_radiation(radiation_input()).expect("locked input");
    let devices = sequence(&object, tags::TREATMENT_DEVICE_IDENTIFICATION_SEQUENCE, 1);
    let device = &devices[0];
    for (tag, vr, expected) in [
        (tags::MANUFACTURER, VR::LO, "dicom-test-suite"),
        (tags::MANUFACTURER_MODEL_NAME, VR::LO, "DTS C-Arm LINAC"),
        (tags::MANUFACTURER_MODEL_VERSION, VR::LO, "1"),
        (tags::MANUFACTURER_DEVICE_CLASS_UID, VR::UI, ""),
        (tags::DEVICE_SERIAL_NUMBER, VR::LO, RT_DEVICE_SERIAL_NUMBER),
        (tags::SOFTWARE_VERSIONS, VR::LO, crate::PACKAGE_VERSION),
        (tags::DEVICE_ALTERNATE_IDENTIFIER, VR::UC, ""),
        (tags::DEVICE_LABEL, VR::LO, RT_DEVICE_LABEL),
        (
            tags::MANUFACTURER_DEVICE_IDENTIFIER,
            VR::ST,
            RT_DEVICE_SERIAL_NUMBER,
        ),
    ] {
        assert_text(device, tag, vr, expected);
    }
    assert_code(
        device,
        tags::DEVICE_TYPE_CODE_SEQUENCE,
        "130361",
        "DCM",
        "Radiotherapy Treatment Device",
    );
    assert_code(
        &object,
        tags::RADIATION_DOSIMETER_UNIT_SEQUENCE,
        "{MU}",
        "UCUM",
        "Monitor Units",
    );
    assert_code(
        &object,
        tags::RT_DEVICE_DISTANCE_REFERENCE_LOCATION_CODE_SEQUENCE,
        "130358",
        "DCM",
        "Nominal Radiation Source Location",
    );
    assert_code(
        &object,
        tags::RT_TREATMENT_TECHNIQUE_CODE_SEQUENCE,
        "130102",
        "DCM",
        "Static Beam",
    );

    let positions = sequence(&object, tags::TREATMENT_POSITION_SEQUENCE, 1);
    let position = &positions[0];
    assert_u16(position, tags::TREATMENT_POSITION_INDEX, 1);
    assert_code(
        position,
        tags::PATIENT_ORIENTATION_CODE_SEQUENCE,
        "102538003",
        "SCT",
        "recumbent",
    );
    assert_code(
        position,
        tags::PATIENT_ORIENTATION_MODIFIER_CODE_SEQUENCE,
        "40199007",
        "SCT",
        "supine",
    );
    assert_code(
        position,
        tags::PATIENT_EQUIPMENT_RELATIONSHIP_CODE_SEQUENCE,
        "102540008",
        "SCT",
        "headfirst",
    );
    assert_text(
        position,
        tags::IMAGE_TO_EQUIPMENT_MAPPING_MATRIX,
        VR::DS,
        "1\\0\\0\\0\\0\\1\\0\\0\\0\\0\\1\\0\\0\\0\\0\\1",
    );
    assert_empty_sequence(position, tags::PATIENT_LOCATION_COORDINATES_SEQUENCE);
    assert_empty_sequence(position, tags::PATIENT_SUPPORT_POSITION_SEQUENCE);
}

#[test]
fn rt_radiation_locks_control_point_inheritance_and_absent_conditionals() {
    let object = build_rt_radiation(radiation_input()).expect("locked input");
    assert_u16(&object, tags::NUMBER_OF_RT_CONTROL_POINTS, 2);
    let points = sequence(
        &object,
        tags::C_ARM_PHOTON_ELECTRON_CONTROL_POINT_SEQUENCE,
        2,
    );
    let first = &points[0];
    assert_u16(first, tags::RT_CONTROL_POINT_INDEX, 1);
    assert_f64(first, tags::CUMULATIVE_METERSET, 0.0);
    assert_u16(first, tags::REFERENCED_TREATMENT_POSITION_INDEX, 1);
    assert_empty(first, tags::DELIVERY_RATE, VR::FD);
    assert_f64(first, tags::SOURCE_ROLL_ANGLE, 0.0);
    assert_f64(first, tags::RT_BEAM_LIMITING_DEVICE_ANGLE, 0.0);
    assert_empty(first, tags::SOURCE_TO_PATIENT_SURFACE_DISTANCE, VR::FD);
    assert_empty(first, tags::SOURCE_TO_EXTERNAL_CONTOUR_DISTANCE, VR::FL);

    let final_point = &points[1];
    assert_eq!(final_point.iter().count(), 2);
    assert_u16(final_point, tags::RT_CONTROL_POINT_INDEX, 2);
    assert_f64(final_point, tags::CUMULATIVE_METERSET, 100.0);
    for tag in [
        tags::REFERENCED_TREATMENT_POSITION_INDEX,
        tags::DELIVERY_RATE,
        tags::SOURCE_ROLL_ANGLE,
        tags::RT_BEAM_LIMITING_DEVICE_ANGLE,
        tags::SOURCE_TO_PATIENT_SURFACE_DISTANCE,
        tags::SOURCE_TO_EXTERNAL_CONTOUR_DISTANCE,
    ] {
        assert!(
            final_point.element(tag).is_err(),
            "the second control point must inherit {tag:?}"
        );
    }

    for tag in [
        tags::RECORDED_RT_CONTROL_POINT_DATE_TIME,
        tags::PATIENT_SUPPORT_DEVICES_SEQUENCE,
        tags::RT_ACCESSORY_HOLDER_SLOT_SEQUENCE,
        tags::RT_BEAM_LIMITING_DEVICE_OPENING_SEQUENCE,
        tags::RADIATION_DEVICE_CONFIGURATION_AND_COMMISSIONING_KEY_SEQUENCE,
        tags::RADIATION_GENERATION_MODE_SEQUENCE,
        tags::REFERENCED_PERFORMED_PROCEDURE_STEP_SEQUENCE,
        tags::TREATMENT_SESSION_UID,
        tags::SYNCHRONIZATION_FRAME_OF_REFERENCE_UID,
        tags::SYNCHRONIZATION_TRIGGER,
        tags::ACQUISITION_TIME_SYNCHRONIZED,
    ] {
        assert!(object.element(tag).is_err(), "{tag:?} must be absent");
    }
}

#[test]
fn rt_radiation_set_builds_locked_metadata_and_ordered_reference_graph() {
    let object = build_rt_radiation_set(set_input()).expect("locked input");
    assert_eq!(RT_RADIATION_SET_OUTPUT_FILE, "instance.dcm");
    for (tag, vr, expected) in [
        (tags::SOP_CLASS_UID, VR::UI, RT_RADIATION_SET_STORAGE_UID),
        (tags::SOP_INSTANCE_UID, VR::UI, SET_SOP_UID),
        (tags::SYNTHETIC_DATA, VR::CS, "YES"),
        (tags::PATIENT_NAME, VR::PN, "DTS^Synthetic^Patient001"),
        (tags::PATIENT_ID, VR::LO, "DTS-PATIENT-001"),
        (tags::PATIENT_BIRTH_DATE, VR::DA, "19700101"),
        (tags::PATIENT_SEX, VR::CS, "O"),
        (tags::MODALITY, VR::CS, "RTRAD"),
        (tags::STUDY_INSTANCE_UID, VR::UI, STUDY_UID),
        (tags::STUDY_DATE, VR::DA, "20260101"),
        (tags::STUDY_TIME, VR::TM, "000000"),
        (tags::REFERRING_PHYSICIAN_NAME, VR::PN, ""),
        (tags::STUDY_ID, VR::SH, "DTS-RTSTRUCT"),
        (tags::ACCESSION_NUMBER, VR::SH, ""),
        (tags::SERIES_INSTANCE_UID, VR::UI, SET_SERIES_UID),
        (tags::SERIES_NUMBER, VR::IS, RT_RADIATION_SET_SERIES_NUMBER),
        (tags::SERIES_DATE, VR::DA, "20260101"),
        (tags::SERIES_TIME, VR::TM, "000000"),
        (tags::FRAME_OF_REFERENCE_UID, VR::UI, FRAME_OF_REFERENCE_UID),
        (tags::POSITION_REFERENCE_INDICATOR, VR::LO, ""),
        (tags::MANUFACTURER, VR::LO, "dicom-test-suite"),
        (
            tags::MANUFACTURER_MODEL_NAME,
            VR::LO,
            "Native RT Radiation Set",
        ),
        (tags::DEVICE_SERIAL_NUMBER, VR::LO, RT_DEVICE_SERIAL_NUMBER),
        (tags::SOFTWARE_VERSIONS, VR::LO, crate::PACKAGE_VERSION),
        (tags::INSTANCE_CREATION_DATE, VR::DA, "20260101"),
        (tags::INSTANCE_CREATION_TIME, VR::TM, "000000"),
        (tags::CONTENT_DATE, VR::DA, "20260101"),
        (tags::CONTENT_TIME, VR::TM, "000000"),
        (
            tags::USER_CONTENT_LABEL,
            VR::SH,
            RT_RADIATION_SET_USER_CONTENT_LABEL,
        ),
        (tags::CONTENT_DESCRIPTION, VR::LO, ""),
        (tags::RT_RADIATION_SET_INTENT, VR::CS, "TREATMENT"),
    ] {
        assert_text(&object, tag, vr, expected);
    }
    assert_u16(&object, tags::INTENDED_NUMBER_OF_FRACTIONS, 1);
    assert_empty_sequence(&object, tags::AUTHOR_IDENTIFICATION_SEQUENCE);
    assert_empty_sequence(&object, tags::REFERENCED_RT_PHYSICIAN_INTENT_SEQUENCE);

    let definitions = sequence(&object, tags::DEFINITION_SOURCE_SEQUENCE, 1);
    assert_eq!(definitions[0].iter().count(), 2);
    assert_sop_reference(&definitions[0], RT_PLAN_STORAGE_UID, PLAN_SOP_UID);

    let direct = sequence(&object, tags::RT_RADIATION_SEQUENCE, 1);
    assert_sop_reference(
        &direct[0],
        C_ARM_PHOTON_ELECTRON_RADIATION_STORAGE_UID,
        RADIATION_SOP_UID,
    );
    let groups = sequence(&object, tags::TREATMENT_POSITION_GROUP_SEQUENCE, 1);
    assert_text(
        &groups[0],
        tags::TREATMENT_POSITION_GROUP_UID,
        VR::UI,
        POSITION_GROUP_UID,
    );
    assert_text(
        &groups[0],
        tags::TREATMENT_POSITION_GROUP_LABEL,
        VR::LO,
        RT_TREATMENT_POSITION_GROUP_LABEL,
    );
    let grouped = sequence(&groups[0], tags::REFERENCED_RT_RADIATION_SEQUENCE, 1);
    assert_sop_reference(
        &grouped[0],
        C_ARM_PHOTON_ELECTRON_RADIATION_STORAGE_UID,
        RADIATION_SOP_UID,
    );

    let series = sequence(&object, tags::REFERENCED_SERIES_SEQUENCE, 2);
    for (item, expected_series, expected_class, expected_instance) in [
        (
            &series[0],
            PLAN_SERIES_UID,
            RT_PLAN_STORAGE_UID,
            PLAN_SOP_UID,
        ),
        (
            &series[1],
            RADIATION_SERIES_UID,
            C_ARM_PHOTON_ELECTRON_RADIATION_STORAGE_UID,
            RADIATION_SOP_UID,
        ),
    ] {
        assert_text(item, tags::SERIES_INSTANCE_UID, VR::UI, expected_series);
        let instances = sequence(item, tags::REFERENCED_INSTANCE_SEQUENCE, 1);
        assert_sop_reference(&instances[0], expected_class, expected_instance);
    }
}

#[test]
fn rt_radiation_set_omits_locked_full_and_conditional_content() {
    let object = build_rt_radiation_set(set_input()).expect("locked input");
    for tag in [
        tags::TREATMENT_DEVICE_IDENTIFICATION_SEQUENCE,
        tags::RADIATION_DOSIMETER_UNIT_SEQUENCE,
        tags::RT_DEVICE_DISTANCE_REFERENCE_LOCATION_CODE_SEQUENCE,
        tags::TREATMENT_POSITION_SEQUENCE,
        tags::C_ARM_PHOTON_ELECTRON_CONTROL_POINT_SEQUENCE,
        tags::REFERENCED_PERFORMED_PROCEDURE_STEP_SEQUENCE,
        tags::TREATMENT_SESSION_UID,
        tags::SYNCHRONIZATION_FRAME_OF_REFERENCE_UID,
        tags::SYNCHRONIZATION_TRIGGER,
        tags::ACQUISITION_TIME_SYNCHRONIZED,
        tags::ROWS,
        tags::COLUMNS,
        tags::PIXEL_DATA,
    ] {
        assert!(object.element(tag).is_err(), "{tag:?} must be absent");
    }
}

#[test]
fn rt_radiation_builders_reject_wrong_malformed_and_reused_uid_roles() {
    let mut wrong_plan = radiation_input();
    wrong_plan.plan_sop_class_uid = RT_RADIATION_SET_STORAGE_UID;
    assert!(
        build_rt_radiation(wrong_plan)
            .unwrap_err()
            .contains("plan SOP Class UID")
    );
    let mut wrong_set_plan = set_input();
    wrong_set_plan.plan_sop_class_uid = RT_RADIATION_SET_STORAGE_UID;
    assert!(
        build_rt_radiation_set(wrong_set_plan)
            .unwrap_err()
            .contains("plan SOP Class UID")
    );
    let mut wrong_radiation = set_input();
    wrong_radiation.radiation_sop_class_uid = RT_RADIATION_SET_STORAGE_UID;
    assert!(
        build_rt_radiation_set(wrong_radiation)
            .unwrap_err()
            .contains("Radiation SOP Class UID")
    );

    for input in [
        RtRadiationInput {
            study_instance_uid: "",
            ..radiation_input()
        },
        RtRadiationInput {
            frame_of_reference_uid: "1.2.bad",
            ..radiation_input()
        },
        RtRadiationInput {
            series_instance_uid: ".1.2",
            ..radiation_input()
        },
        RtRadiationInput {
            sop_instance_uid: "1.2.",
            ..radiation_input()
        },
        RtRadiationInput {
            plan_series_instance_uid: "1..2",
            ..radiation_input()
        },
        RtRadiationInput {
            plan_sop_instance_uid: "1.02.3",
            ..radiation_input()
        },
    ] {
        assert!(build_rt_radiation(input).is_err());
    }
    for input in [
        RtRadiationSetInput {
            study_instance_uid: "",
            ..set_input()
        },
        RtRadiationSetInput {
            radiation_series_instance_uid: "1..2",
            ..set_input()
        },
        RtRadiationSetInput {
            treatment_position_group_uid: "1.02.3",
            ..set_input()
        },
    ] {
        assert!(build_rt_radiation_set(input).is_err());
    }

    let radiation_roles = [
        STUDY_UID,
        FRAME_OF_REFERENCE_UID,
        RADIATION_SERIES_UID,
        RADIATION_SOP_UID,
        PLAN_SERIES_UID,
        PLAN_SOP_UID,
    ];
    for left in 0..radiation_roles.len() {
        for right in left + 1..radiation_roles.len() {
            let mut values = radiation_roles;
            values[right] = values[left];
            let error = build_rt_radiation(RtRadiationInput {
                study_instance_uid: values[0],
                frame_of_reference_uid: values[1],
                series_instance_uid: values[2],
                sop_instance_uid: values[3],
                plan_series_instance_uid: values[4],
                plan_sop_class_uid: RT_PLAN_STORAGE_UID,
                plan_sop_instance_uid: values[5],
            })
            .unwrap_err();
            assert!(
                error.contains("must be distinct"),
                "{left}, {right}: {error}"
            );
        }
    }

    let set_roles = [
        STUDY_UID,
        FRAME_OF_REFERENCE_UID,
        SET_SERIES_UID,
        SET_SOP_UID,
        PLAN_SERIES_UID,
        PLAN_SOP_UID,
        RADIATION_SERIES_UID,
        RADIATION_SOP_UID,
        POSITION_GROUP_UID,
    ];
    for left in 0..set_roles.len() {
        for right in left + 1..set_roles.len() {
            let mut values = set_roles;
            values[right] = values[left];
            let error = build_rt_radiation_set(RtRadiationSetInput {
                study_instance_uid: values[0],
                frame_of_reference_uid: values[1],
                series_instance_uid: values[2],
                sop_instance_uid: values[3],
                plan_series_instance_uid: values[4],
                plan_sop_class_uid: RT_PLAN_STORAGE_UID,
                plan_sop_instance_uid: values[5],
                radiation_series_instance_uid: values[6],
                radiation_sop_class_uid: C_ARM_PHOTON_ELECTRON_RADIATION_STORAGE_UID,
                radiation_sop_instance_uid: values[7],
                treatment_position_group_uid: values[8],
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
fn rt_radiation_datasets_serialize_byte_deterministically() {
    assert_deterministic_dataset(|| build_rt_radiation(radiation_input()).expect("radiation"));
    assert_deterministic_dataset(|| build_rt_radiation_set(set_input()).expect("set"));
}

fn assert_deterministic_dataset(build: impl Fn() -> InMemDicomObject) {
    let transfer_syntax = TransferSyntaxRegistry
        .get(uids::EXPLICIT_VR_LITTLE_ENDIAN)
        .expect("Explicit VR Little Endian transfer syntax");
    let mut first = Vec::new();
    let mut second = Vec::new();
    build()
        .write_dataset_with_ts(&mut first, transfer_syntax)
        .expect("first serialization");
    build()
        .write_dataset_with_ts(&mut second, transfer_syntax)
        .expect("second serialization");
    assert!(!first.is_empty());
    assert_eq!(first, second);
}

fn sequence(object: &InMemDicomObject, tag: Tag, expected_len: usize) -> &[InMemDicomObject] {
    let element = object.element(tag).expect("sequence");
    assert_eq!(element.vr(), VR::SQ, "{tag:?}");
    let items = element.items().expect("sequence items");
    assert_eq!(items.len(), expected_len, "{tag:?}");
    items
}

fn assert_sop_reference(
    object: &InMemDicomObject,
    expected_class_uid: &str,
    expected_instance_uid: &str,
) {
    assert_text(
        object,
        tags::REFERENCED_SOP_CLASS_UID,
        VR::UI,
        expected_class_uid,
    );
    assert_text(
        object,
        tags::REFERENCED_SOP_INSTANCE_UID,
        VR::UI,
        expected_instance_uid,
    );
}

fn assert_code(object: &InMemDicomObject, tag: Tag, value: &str, scheme: &str, meaning: &str) {
    let items = sequence(object, tag, 1);
    assert_eq!(items[0].iter().count(), 3);
    assert_text(&items[0], tags::CODE_VALUE, VR::SH, value);
    assert_text(&items[0], tags::CODING_SCHEME_DESIGNATOR, VR::SH, scheme);
    assert_text(&items[0], tags::CODE_MEANING, VR::LO, meaning);
}

fn assert_empty_sequence(object: &InMemDicomObject, tag: Tag) {
    sequence(object, tag, 0);
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

fn assert_f64(object: &InMemDicomObject, tag: Tag, expected: f64) {
    let element = object.element(tag).expect("FD attribute");
    assert_eq!(element.vr(), VR::FD, "{tag:?}");
    assert_eq!(element.to_float64().expect("FD"), expected, "{tag:?}");
}

fn assert_empty(object: &InMemDicomObject, tag: Tag, expected_vr: VR) {
    let element = object.element(tag).expect("empty attribute");
    assert_eq!(element.vr(), expected_vr, "{tag:?}");
    assert!(element.value().is_empty(), "{tag:?}");
}
