use dicom_core::{Tag, VR, header::Header};
use dicom_dictionary_std::{tags, uids};
use dicom_object::InMemDicomObject;
use dicom_transfer_syntax_registry::{TransferSyntaxIndex, TransferSyntaxRegistry};

use super::rt_plan::{
    RT_DOSE_STORAGE_UID, RT_PLAN_BEAM_NAME, RT_PLAN_LABEL, RT_PLAN_OUTPUT_FILE,
    RT_PLAN_SERIES_NUMBER, RT_PLAN_STORAGE_UID, RT_STRUCTURE_SET_STORAGE_UID, RtPlanInput,
    build_rt_plan,
};

const STUDY_UID: &str = "2.25.310000000000000000000000000000000000001";
const FRAME_OF_REFERENCE_UID: &str = "2.25.310000000000000000000000000000000000002";
const SERIES_UID: &str = "2.25.310000000000000000000000000000000000003";
const SOP_UID: &str = "2.25.310000000000000000000000000000000000004";
const STRUCTURE_SET_SOP_UID: &str = "2.25.310000000000000000000000000000000000005";
const DOSE_SOP_UID: &str = "2.25.310000000000000000000000000000000000006";

fn locked_input() -> RtPlanInput<'static> {
    RtPlanInput {
        study_instance_uid: STUDY_UID,
        frame_of_reference_uid: FRAME_OF_REFERENCE_UID,
        series_instance_uid: SERIES_UID,
        sop_instance_uid: SOP_UID,
        structure_set_sop_class_uid: RT_STRUCTURE_SET_STORAGE_UID,
        structure_set_sop_instance_uid: STRUCTURE_SET_SOP_UID,
        dose_sop_class_uid: RT_DOSE_STORAGE_UID,
        dose_sop_instance_uid: DOSE_SOP_UID,
    }
}

#[test]
fn rt_plan_builds_locked_identity_and_mandatory_metadata() {
    let object = build_rt_plan(locked_input()).expect("locked input");
    assert_eq!(RT_PLAN_OUTPUT_FILE, "instance.dcm");
    for (tag, vr, expected) in [
        (tags::SOP_CLASS_UID, VR::UI, RT_PLAN_STORAGE_UID),
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
        (tags::STUDY_ID, VR::SH, "DTS-RT"),
        (tags::ACCESSION_NUMBER, VR::SH, ""),
        (tags::MODALITY, VR::CS, "RTPLAN"),
        (tags::SERIES_INSTANCE_UID, VR::UI, SERIES_UID),
        (tags::SERIES_NUMBER, VR::IS, RT_PLAN_SERIES_NUMBER),
        (tags::OPERATORS_NAME, VR::PN, ""),
        (tags::FRAME_OF_REFERENCE_UID, VR::UI, FRAME_OF_REFERENCE_UID),
        (tags::POSITION_REFERENCE_INDICATOR, VR::LO, ""),
        (tags::MANUFACTURER, VR::LO, "dicom-test-suite"),
        (tags::INSTITUTION_NAME, VR::LO, ""),
        (tags::INSTITUTION_ADDRESS, VR::ST, ""),
        (
            tags::MANUFACTURER_MODEL_NAME,
            VR::LO,
            "Native Linked RT Plan",
        ),
        (tags::DEVICE_SERIAL_NUMBER, VR::LO, "DTS-RTPLAN-001"),
        (tags::SOFTWARE_VERSIONS, VR::LO, crate::PACKAGE_VERSION),
        (tags::INSTANCE_NUMBER, VR::IS, "1"),
        (tags::RT_PLAN_LABEL, VR::SH, RT_PLAN_LABEL),
        (tags::RT_PLAN_DATE, VR::DA, "20260101"),
        (tags::RT_PLAN_TIME, VR::TM, "000000"),
        (tags::RT_PLAN_GEOMETRY, VR::CS, "PATIENT"),
    ] {
        assert_text(&object, tag, vr, expected);
    }
}

#[test]
fn rt_plan_links_exact_structure_set_and_dose_once_and_in_order() {
    let object = build_rt_plan(locked_input()).expect("locked input");
    assert_reference(
        &object,
        tags::REFERENCED_STRUCTURE_SET_SEQUENCE,
        RT_STRUCTURE_SET_STORAGE_UID,
        STRUCTURE_SET_SOP_UID,
    );
    assert_reference(
        &object,
        tags::REFERENCED_DOSE_SEQUENCE,
        RT_DOSE_STORAGE_UID,
        DOSE_SOP_UID,
    );
    assert!(
        object.element(tags::REFERENCED_RT_PLAN_SEQUENCE).is_err(),
        "the Plan must not recursively reference an RT Plan"
    );

    let top_level_sequences = object
        .iter()
        .filter(|element| element.vr() == VR::SQ)
        .map(|element| element.tag())
        .collect::<Vec<_>>();
    assert_eq!(
        top_level_sequences,
        [
            tags::FRACTION_GROUP_SEQUENCE,
            tags::BEAM_SEQUENCE,
            tags::REFERENCED_STRUCTURE_SET_SEQUENCE,
            tags::REFERENCED_DOSE_SEQUENCE,
        ],
        "InMemDicomObject serialization order is tag order, with both reference roles explicit"
    );
}

#[test]
fn rt_plan_builds_one_fraction_group_referencing_one_beam() {
    let object = build_rt_plan(locked_input()).expect("locked input");
    let groups = sequence(&object, tags::FRACTION_GROUP_SEQUENCE, 1);
    let group = &groups[0];
    assert_eq!(group.iter().count(), 5);
    for (tag, expected) in [
        (tags::FRACTION_GROUP_NUMBER, "1"),
        (tags::NUMBER_OF_FRACTIONS_PLANNED, "1"),
        (tags::NUMBER_OF_BEAMS, "1"),
        (tags::NUMBER_OF_BRACHY_APPLICATION_SETUPS, "0"),
    ] {
        assert_text(group, tag, VR::IS, expected);
    }
    let references = sequence(group, tags::REFERENCED_BEAM_SEQUENCE, 1);
    assert_eq!(references[0].iter().count(), 1);
    assert_text(&references[0], tags::REFERENCED_BEAM_NUMBER, VR::IS, "1");
}

#[test]
fn rt_plan_builds_exact_static_photon_beam_and_ordered_jaws() {
    let object = build_rt_plan(locked_input()).expect("locked input");
    let beams = sequence(&object, tags::BEAM_SEQUENCE, 1);
    let beam = &beams[0];
    assert_eq!(beam.iter().count(), 16);
    for (tag, vr, expected) in [
        (tags::TREATMENT_MACHINE_NAME, VR::SH, "DTS_LINAC"),
        (tags::PRIMARY_DOSIMETER_UNIT, VR::CS, "MU"),
        (tags::SOURCE_AXIS_DISTANCE, VR::DS, "1000"),
        (tags::BEAM_NUMBER, VR::IS, "1"),
        (tags::BEAM_NAME, VR::LO, RT_PLAN_BEAM_NAME),
        (tags::BEAM_TYPE, VR::CS, "STATIC"),
        (tags::RADIATION_TYPE, VR::CS, "PHOTON"),
        (tags::TREATMENT_DELIVERY_TYPE, VR::CS, "TREATMENT"),
        (tags::NUMBER_OF_WEDGES, VR::IS, "0"),
        (tags::NUMBER_OF_COMPENSATORS, VR::IS, "0"),
        (tags::NUMBER_OF_BOLI, VR::IS, "0"),
        (tags::NUMBER_OF_BLOCKS, VR::IS, "0"),
        (tags::FINAL_CUMULATIVE_METERSET_WEIGHT, VR::DS, "1"),
        (tags::NUMBER_OF_CONTROL_POINTS, VR::IS, "2"),
    ] {
        assert_text(beam, tag, vr, expected);
    }

    let devices = sequence(beam, tags::BEAM_LIMITING_DEVICE_SEQUENCE, 2);
    for (device, expected_type) in devices.iter().zip(["X", "Y"]) {
        assert_eq!(device.iter().count(), 3);
        assert_text(
            device,
            tags::RT_BEAM_LIMITING_DEVICE_TYPE,
            VR::CS,
            expected_type,
        );
        assert_text(device, tags::NUMBER_OF_LEAF_JAW_PAIRS, VR::IS, "1");
        assert_text(
            device,
            tags::SOURCE_TO_BEAM_LIMITING_DEVICE_DISTANCE,
            VR::DS,
            "500",
        );
    }

    for tag in [
        tags::WEDGE_SEQUENCE,
        tags::COMPENSATOR_SEQUENCE,
        tags::REFERENCED_BOLUS_SEQUENCE,
        tags::BLOCK_SEQUENCE,
    ] {
        assert!(beam.element(tag).is_err(), "{tag:?} must be absent");
    }
}

#[test]
fn rt_plan_locks_control_point_order_geometry_and_inheritance() {
    let object = build_rt_plan(locked_input()).expect("locked input");
    let beams = sequence(&object, tags::BEAM_SEQUENCE, 1);
    let control_points = sequence(&beams[0], tags::CONTROL_POINT_SEQUENCE, 2);
    let first = &control_points[0];
    let final_point = &control_points[1];
    assert_eq!(first.iter().count(), 18);

    for (tag, vr, expected) in [
        (tags::CONTROL_POINT_INDEX, VR::IS, "0"),
        (tags::NOMINAL_BEAM_ENERGY, VR::DS, "6"),
        (tags::GANTRY_ANGLE, VR::DS, "0"),
        (tags::GANTRY_ROTATION_DIRECTION, VR::CS, "NONE"),
        (tags::BEAM_LIMITING_DEVICE_ANGLE, VR::DS, "0"),
        (
            tags::BEAM_LIMITING_DEVICE_ROTATION_DIRECTION,
            VR::CS,
            "NONE",
        ),
        (tags::PATIENT_SUPPORT_ANGLE, VR::DS, "0"),
        (tags::PATIENT_SUPPORT_ROTATION_DIRECTION, VR::CS, "NONE"),
        (tags::TABLE_TOP_VERTICAL_POSITION, VR::DS, "0"),
        (tags::TABLE_TOP_LONGITUDINAL_POSITION, VR::DS, "0"),
        (tags::TABLE_TOP_LATERAL_POSITION, VR::DS, "0"),
        (tags::TABLE_TOP_PITCH_ROTATION_DIRECTION, VR::CS, "NONE"),
        (tags::TABLE_TOP_ROLL_ROTATION_DIRECTION, VR::CS, "NONE"),
        (tags::ISOCENTER_POSITION, VR::DS, "0\\0\\0"),
        (tags::CUMULATIVE_METERSET_WEIGHT, VR::DS, "0"),
    ] {
        assert_text(first, tag, vr, expected);
    }
    assert_float(first, tags::TABLE_TOP_PITCH_ANGLE, 0.0);
    assert_float(first, tags::TABLE_TOP_ROLL_ANGLE, 0.0);

    let positions = sequence(first, tags::BEAM_LIMITING_DEVICE_POSITION_SEQUENCE, 2);
    for (position, expected_type) in positions.iter().zip(["X", "Y"]) {
        assert_eq!(position.iter().count(), 2);
        assert_text(
            position,
            tags::RT_BEAM_LIMITING_DEVICE_TYPE,
            VR::CS,
            expected_type,
        );
        assert_text(position, tags::LEAF_JAW_POSITIONS, VR::DS, "-50\\50");
    }

    assert_eq!(final_point.iter().count(), 2);
    assert_text(final_point, tags::CONTROL_POINT_INDEX, VR::IS, "1");
    assert_text(final_point, tags::CUMULATIVE_METERSET_WEIGHT, VR::DS, "1");
    for tag in [
        tags::NOMINAL_BEAM_ENERGY,
        tags::BEAM_LIMITING_DEVICE_POSITION_SEQUENCE,
        tags::GANTRY_ANGLE,
        tags::GANTRY_ROTATION_DIRECTION,
        tags::BEAM_LIMITING_DEVICE_ANGLE,
        tags::BEAM_LIMITING_DEVICE_ROTATION_DIRECTION,
        tags::PATIENT_SUPPORT_ANGLE,
        tags::PATIENT_SUPPORT_ROTATION_DIRECTION,
        tags::TABLE_TOP_VERTICAL_POSITION,
        tags::TABLE_TOP_LONGITUDINAL_POSITION,
        tags::TABLE_TOP_LATERAL_POSITION,
        tags::TABLE_TOP_PITCH_ANGLE,
        tags::TABLE_TOP_PITCH_ROTATION_DIRECTION,
        tags::TABLE_TOP_ROLL_ANGLE,
        tags::TABLE_TOP_ROLL_ROTATION_DIRECTION,
        tags::ISOCENTER_POSITION,
    ] {
        assert!(
            final_point.element(tag).is_err(),
            "final control point must inherit {tag:?}"
        );
    }
}

#[test]
fn rt_plan_omits_locked_modules_and_all_image_content() {
    let object = build_rt_plan(locked_input()).expect("locked input");
    for tag in [
        tags::REFERENCED_RT_PLAN_SEQUENCE,
        tags::DOSE_REFERENCE_SEQUENCE,
        tags::TOLERANCE_TABLE_SEQUENCE,
        tags::PATIENT_SETUP_SEQUENCE,
        tags::APPLICATION_SETUP_SEQUENCE,
        tags::APPROVAL_STATUS,
        tags::STUDIES_CONTAINING_OTHER_REFERENCED_INSTANCES_SEQUENCE,
        tags::REFERENCED_SERIES_SEQUENCE,
        tags::ROWS,
        tags::COLUMNS,
        tags::SAMPLES_PER_PIXEL,
        tags::PHOTOMETRIC_INTERPRETATION,
        tags::BITS_ALLOCATED,
        tags::BITS_STORED,
        tags::HIGH_BIT,
        tags::PIXEL_REPRESENTATION,
        tags::PIXEL_DATA,
    ] {
        assert!(object.element(tag).is_err(), "{tag:?} must be absent");
    }
}

#[test]
fn rt_plan_rejects_wrong_missing_malformed_and_reused_uid_roles() {
    let mut wrong_structure_class = locked_input();
    wrong_structure_class.structure_set_sop_class_uid = RT_DOSE_STORAGE_UID;
    assert!(
        build_rt_plan(wrong_structure_class)
            .unwrap_err()
            .contains("Structure Set SOP Class UID")
    );
    let mut wrong_dose_class = locked_input();
    wrong_dose_class.dose_sop_class_uid = RT_STRUCTURE_SET_STORAGE_UID;
    assert!(
        build_rt_plan(wrong_dose_class)
            .unwrap_err()
            .contains("Dose SOP Class UID")
    );

    for input in [
        RtPlanInput {
            study_instance_uid: "",
            ..locked_input()
        },
        RtPlanInput {
            frame_of_reference_uid: "1.2.bad",
            ..locked_input()
        },
        RtPlanInput {
            series_instance_uid: ".1.2",
            ..locked_input()
        },
        RtPlanInput {
            sop_instance_uid: "1.2.",
            ..locked_input()
        },
        RtPlanInput {
            structure_set_sop_instance_uid: "1..2",
            ..locked_input()
        },
        RtPlanInput {
            structure_set_sop_instance_uid: "1.02.3",
            ..locked_input()
        },
        RtPlanInput {
            dose_sop_instance_uid: "1.234567890123456789012345678901234567890123456789012345678901234",
            ..locked_input()
        },
    ] {
        assert!(build_rt_plan(input).is_err());
    }

    let roles = [
        STUDY_UID,
        FRAME_OF_REFERENCE_UID,
        SERIES_UID,
        SOP_UID,
        STRUCTURE_SET_SOP_UID,
        DOSE_SOP_UID,
    ];
    for left in 0..roles.len() {
        for right in left + 1..roles.len() {
            let mut values = roles;
            values[right] = values[left];
            let error = build_rt_plan(RtPlanInput {
                study_instance_uid: values[0],
                frame_of_reference_uid: values[1],
                series_instance_uid: values[2],
                sop_instance_uid: values[3],
                structure_set_sop_class_uid: RT_STRUCTURE_SET_STORAGE_UID,
                structure_set_sop_instance_uid: values[4],
                dose_sop_class_uid: RT_DOSE_STORAGE_UID,
                dose_sop_instance_uid: values[5],
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
fn rt_plan_dataset_serialization_is_byte_deterministic() {
    let transfer_syntax = TransferSyntaxRegistry
        .get(uids::EXPLICIT_VR_LITTLE_ENDIAN)
        .expect("Explicit VR Little Endian transfer syntax");
    let mut first = Vec::new();
    let mut second = Vec::new();
    build_rt_plan(locked_input())
        .expect("first object")
        .write_dataset_with_ts(&mut first, transfer_syntax)
        .expect("first serialization");
    build_rt_plan(locked_input())
        .expect("second object")
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

fn assert_reference(
    object: &InMemDicomObject,
    tag: Tag,
    expected_class_uid: &str,
    expected_instance_uid: &str,
) {
    let items = sequence(object, tag, 1);
    assert_eq!(items[0].iter().count(), 2);
    assert_text(
        &items[0],
        tags::REFERENCED_SOP_CLASS_UID,
        VR::UI,
        expected_class_uid,
    );
    assert_text(
        &items[0],
        tags::REFERENCED_SOP_INSTANCE_UID,
        VR::UI,
        expected_instance_uid,
    );
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

fn assert_float(object: &InMemDicomObject, tag: Tag, expected: f32) {
    let element = object.element(tag).expect("float attribute");
    assert_eq!(element.vr(), VR::FL, "{tag:?}");
    assert_eq!(element.to_float32().expect("FL"), expected, "{tag:?}");
}
