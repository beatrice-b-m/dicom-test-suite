use std::path::PathBuf;

use dicom_core::{DataElement, PrimitiveValue, VR, value::DataSetSequence};
use dicom_dictionary_std::{tags, uids};
use dicom_object::{FileMetaTableBuilder, InMemDicomObject};

use super::{RtPlanExpectations, validate_rt_plan_file};
use crate::rt_manifest::{LinkedRtPlanInput, linked_rt_plan_expected};

const SOP_UID: &str = "2.25.501";
const STUDY_UID: &str = "2.25.502";
const SERIES_UID: &str = "2.25.503";
const FOR_UID: &str = "2.25.504";
const STRUCT_SERIES_UID: &str = "2.25.505";
const STRUCT_SOP_UID: &str = "2.25.506";
const DOSE_SERIES_UID: &str = "2.25.507";
const DOSE_SOP_UID: &str = "2.25.508";
const IMPLEMENTATION_UID: &str = "2.25.999";
const PLAN_CLASS: &str = "1.2.840.10008.5.1.4.1.1.481.5";
const STRUCT_CLASS: &str = "1.2.840.10008.5.1.4.1.1.481.3";
const DOSE_CLASS: &str = "1.2.840.10008.5.1.4.1.1.481.2";

#[derive(Clone, Copy, Debug)]
enum Mutation {
    None,
    MissingLabel,
    MissingStructure,
    WrongStructureReference,
    DanglingDoseReference,
    DuplicateReferences,
    SwapReferences,
    FractionBeamMismatch,
    DanglingBeam,
    DuplicateBeam,
    MissingZeroCount,
    ReverseDevices,
    ChangedJaw,
    ControlPointCount,
    ControlPointIndex,
    ReverseControlPoints,
    FirstMeterset,
    FinalMeterset,
    Isocenter,
    WrongStudy,
    WrongFrameOfReference,
}

#[test]
fn accepts_exact_linked_rt_plan() {
    let path = write_fixture("valid", Mutation::None);
    let validated = validate_rt_plan_file(&path, &expectations()).expect("valid linked RT Plan");
    assert_eq!(validated.validation["status"], "passed");
    assert_eq!(
        validated.validation["standards"][0]["name"],
        "rt_plan_sop_class"
    );
    std::fs::remove_file(path).ok();
}

#[test]
fn rejects_locked_rt_plan_mutations() {
    for (label, mutation, finding) in [
        ("missing-label", Mutation::MissingLabel, None),
        (
            "patient-without-structure",
            Mutation::MissingStructure,
            None,
        ),
        (
            "wrong-structure",
            Mutation::WrongStructureReference,
            Some("rt_plan_structure_sop_class_uid"),
        ),
        (
            "dangling-dose",
            Mutation::DanglingDoseReference,
            Some("rt_plan_dose_sop_instance_uid"),
        ),
        (
            "duplicate-references",
            Mutation::DuplicateReferences,
            Some("rt_plan_dose_sop_class_uid"),
        ),
        (
            "swapped-references",
            Mutation::SwapReferences,
            Some("rt_plan_structure_sop_class_uid"),
        ),
        (
            "fraction-beam-mismatch",
            Mutation::FractionBeamMismatch,
            Some("rt_plan_fraction_beam_count"),
        ),
        (
            "dangling-beam",
            Mutation::DanglingBeam,
            Some("rt_plan_referenced_beam_number"),
        ),
        (
            "duplicate-beam",
            Mutation::DuplicateBeam,
            Some("rt_plan_beam_count"),
        ),
        ("missing-zero-count", Mutation::MissingZeroCount, None),
        (
            "reverse-devices",
            Mutation::ReverseDevices,
            Some("rt_plan_device_1_type"),
        ),
        (
            "changed-jaw",
            Mutation::ChangedJaw,
            Some("rt_plan_jaw_1_positions"),
        ),
        (
            "control-point-count",
            Mutation::ControlPointCount,
            Some("rt_plan_control_point_count"),
        ),
        (
            "control-point-index",
            Mutation::ControlPointIndex,
            Some("rt_plan_control_point_2_index"),
        ),
        (
            "reverse-control-points",
            Mutation::ReverseControlPoints,
            None,
        ),
        (
            "first-meterset",
            Mutation::FirstMeterset,
            Some("rt_plan_control_point_1_meterset"),
        ),
        (
            "final-meterset",
            Mutation::FinalMeterset,
            Some("rt_plan_control_point_2_meterset"),
        ),
        (
            "isocenter",
            Mutation::Isocenter,
            Some("rt_plan_control_point_1_isocenter"),
        ),
        (
            "wrong-study",
            Mutation::WrongStudy,
            Some("rt_plan_study_instance_uid"),
        ),
        (
            "wrong-for",
            Mutation::WrongFrameOfReference,
            Some("rt_plan_frame_of_reference_uid"),
        ),
    ] {
        let path = write_fixture(label, mutation);
        let error = validate_rt_plan_file(&path, &expectations())
            .expect_err("mutation must fail")
            .to_string();
        if let Some(finding) = finding {
            assert!(error.contains(finding), "{label}: {error}");
        }
        std::fs::remove_file(path).ok();
    }
}

#[test]
fn rejects_manifest_reference_identity_drift() {
    let path = write_fixture("manifest-reference", Mutation::None);
    let mut expected = expectations();
    expected.expected_rt_plan.references[1].study_instance_uid = "2.25.777";
    let error = validate_rt_plan_file(&path, &expected)
        .expect_err("reference Study drift must fail")
        .to_string();
    assert!(error.contains("rt_plan_manifest_shared_identity"));
    std::fs::remove_file(path).ok();
}

#[test]
fn rejects_manifest_absence_weakening() {
    let path = write_fixture("manifest-absence", Mutation::None);
    let mut expected = expectations();
    expected.expected_rt_plan.absent_content.pixel_data = false;
    let error = validate_rt_plan_file(&path, &expected)
        .expect_err("weakened absence must fail")
        .to_string();
    assert!(error.contains("rt_plan_manifest_absence_contract"));
    std::fs::remove_file(path).ok();
}

fn expectations() -> RtPlanExpectations<'static> {
    RtPlanExpectations {
        implementation_class_uid: IMPLEMENTATION_UID,
        synthetic_data: "YES",
        expected_rt_plan: linked_rt_plan_expected(LinkedRtPlanInput {
            sop_instance_uid: SOP_UID,
            study_instance_uid: STUDY_UID,
            series_instance_uid: SERIES_UID,
            frame_of_reference_uid: FOR_UID,
            structure_set_series_instance_uid: STRUCT_SERIES_UID,
            structure_set_sop_instance_uid: STRUCT_SOP_UID,
            structure_set_sha256: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            dose_series_instance_uid: DOSE_SERIES_UID,
            dose_sop_instance_uid: DOSE_SOP_UID,
            dose_sha256: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        }),
    }
}

fn write_fixture(label: &str, mutation: Mutation) -> PathBuf {
    let mut object = valid_object();
    apply_mutation(&mut object, mutation);
    let path = std::env::temp_dir().join(format!(
        "dicom-test-suite-rt-plan-validation-{}-{label}.dcm",
        std::process::id()
    ));
    object
        .with_meta(
            FileMetaTableBuilder::new()
                .media_storage_sop_class_uid(PLAN_CLASS)
                .media_storage_sop_instance_uid(SOP_UID)
                .transfer_syntax(uids::EXPLICIT_VR_LITTLE_ENDIAN)
                .implementation_class_uid(IMPLEMENTATION_UID),
        )
        .expect("file meta")
        .write_to_file(&path)
        .expect("write fixture");
    path
}

fn valid_object() -> InMemDicomObject {
    let mut object = InMemDicomObject::new_empty();
    for (tag, vr, value) in [
        (tags::SOP_CLASS_UID, VR::UI, PLAN_CLASS),
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
        (tags::SERIES_NUMBER, VR::IS, "72"),
        (tags::OPERATORS_NAME, VR::PN, ""),
        (tags::FRAME_OF_REFERENCE_UID, VR::UI, FOR_UID),
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
        (tags::RT_PLAN_LABEL, VR::SH, "DTS_PLAN"),
        (tags::RT_PLAN_DATE, VR::DA, "20260101"),
        (tags::RT_PLAN_TIME, VR::TM, "000000"),
        (tags::RT_PLAN_GEOMETRY, VR::CS, "PATIENT"),
    ] {
        put_str(&mut object, tag, vr, value);
    }
    put_sequence(
        &mut object,
        tags::REFERENCED_STRUCTURE_SET_SEQUENCE,
        vec![reference(STRUCT_CLASS, STRUCT_SOP_UID)],
    );
    put_sequence(
        &mut object,
        tags::REFERENCED_DOSE_SEQUENCE,
        vec![reference(DOSE_CLASS, DOSE_SOP_UID)],
    );
    put_sequence(
        &mut object,
        tags::FRACTION_GROUP_SEQUENCE,
        vec![fraction_group()],
    );
    put_sequence(&mut object, tags::BEAM_SEQUENCE, vec![beam()]);
    object
}

fn reference(class_uid: &str, instance_uid: &str) -> InMemDicomObject {
    let mut item = InMemDicomObject::new_empty();
    put_str(&mut item, tags::REFERENCED_SOP_CLASS_UID, VR::UI, class_uid);
    put_str(
        &mut item,
        tags::REFERENCED_SOP_INSTANCE_UID,
        VR::UI,
        instance_uid,
    );
    item
}

fn fraction_group() -> InMemDicomObject {
    let mut referenced_beam = InMemDicomObject::new_empty();
    put_str(
        &mut referenced_beam,
        tags::REFERENCED_BEAM_NUMBER,
        VR::IS,
        "1",
    );
    let mut group = InMemDicomObject::new_empty();
    for (tag, value) in [
        (tags::FRACTION_GROUP_NUMBER, "1"),
        (tags::NUMBER_OF_FRACTIONS_PLANNED, "1"),
        (tags::NUMBER_OF_BEAMS, "1"),
        (tags::NUMBER_OF_BRACHY_APPLICATION_SETUPS, "0"),
    ] {
        put_str(&mut group, tag, VR::IS, value);
    }
    put_sequence(
        &mut group,
        tags::REFERENCED_BEAM_SEQUENCE,
        vec![referenced_beam],
    );
    group
}

fn beam() -> InMemDicomObject {
    let mut beam = InMemDicomObject::new_empty();
    for (tag, vr, value) in [
        (tags::TREATMENT_MACHINE_NAME, VR::SH, "DTS_LINAC"),
        (tags::PRIMARY_DOSIMETER_UNIT, VR::CS, "MU"),
        (tags::SOURCE_AXIS_DISTANCE, VR::DS, "1000"),
        (tags::BEAM_NUMBER, VR::IS, "1"),
        (tags::BEAM_NAME, VR::LO, "DTS_STATIC_AP"),
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
        put_str(&mut beam, tag, vr, value);
    }
    put_sequence(
        &mut beam,
        tags::BEAM_LIMITING_DEVICE_SEQUENCE,
        vec![device("X"), device("Y")],
    );
    put_sequence(
        &mut beam,
        tags::CONTROL_POINT_SEQUENCE,
        vec![first_control_point(), final_control_point()],
    );
    beam
}

fn device(device_type: &str) -> InMemDicomObject {
    let mut item = InMemDicomObject::new_empty();
    put_str(
        &mut item,
        tags::RT_BEAM_LIMITING_DEVICE_TYPE,
        VR::CS,
        device_type,
    );
    put_str(&mut item, tags::NUMBER_OF_LEAF_JAW_PAIRS, VR::IS, "1");
    put_str(
        &mut item,
        tags::SOURCE_TO_BEAM_LIMITING_DEVICE_DISTANCE,
        VR::DS,
        "500",
    );
    item
}

fn first_control_point() -> InMemDicomObject {
    let mut item = InMemDicomObject::new_empty();
    for (tag, vr, value) in [
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
        put_str(&mut item, tag, vr, value);
    }
    item.put(DataElement::new(
        tags::TABLE_TOP_PITCH_ANGLE,
        VR::FL,
        PrimitiveValue::from(0.0_f32),
    ));
    item.put(DataElement::new(
        tags::TABLE_TOP_ROLL_ANGLE,
        VR::FL,
        PrimitiveValue::from(0.0_f32),
    ));
    put_sequence(
        &mut item,
        tags::BEAM_LIMITING_DEVICE_POSITION_SEQUENCE,
        vec![jaw("X"), jaw("Y")],
    );
    item
}

fn final_control_point() -> InMemDicomObject {
    let mut item = InMemDicomObject::new_empty();
    put_str(&mut item, tags::CONTROL_POINT_INDEX, VR::IS, "1");
    put_str(&mut item, tags::CUMULATIVE_METERSET_WEIGHT, VR::DS, "1");
    item
}

fn jaw(device_type: &str) -> InMemDicomObject {
    let mut item = InMemDicomObject::new_empty();
    put_str(
        &mut item,
        tags::RT_BEAM_LIMITING_DEVICE_TYPE,
        VR::CS,
        device_type,
    );
    put_str(&mut item, tags::LEAF_JAW_POSITIONS, VR::DS, "-50\\50");
    item
}

fn apply_mutation(object: &mut InMemDicomObject, mutation: Mutation) {
    match mutation {
        Mutation::None => {}
        Mutation::MissingLabel => {
            object.take_element(tags::RT_PLAN_LABEL).unwrap();
        }
        Mutation::MissingStructure => {
            object
                .take_element(tags::REFERENCED_STRUCTURE_SET_SEQUENCE)
                .unwrap();
        }
        Mutation::WrongStructureReference => {
            mutate_reference(
                object,
                tags::REFERENCED_STRUCTURE_SET_SEQUENCE,
                DOSE_CLASS,
                STRUCT_SOP_UID,
            );
        }
        Mutation::DanglingDoseReference => {
            mutate_reference(
                object,
                tags::REFERENCED_DOSE_SEQUENCE,
                DOSE_CLASS,
                "2.25.9999",
            );
        }
        Mutation::DuplicateReferences => {
            mutate_reference(
                object,
                tags::REFERENCED_DOSE_SEQUENCE,
                STRUCT_CLASS,
                STRUCT_SOP_UID,
            );
        }
        Mutation::SwapReferences => {
            mutate_reference(
                object,
                tags::REFERENCED_STRUCTURE_SET_SEQUENCE,
                DOSE_CLASS,
                DOSE_SOP_UID,
            );
            mutate_reference(
                object,
                tags::REFERENCED_DOSE_SEQUENCE,
                STRUCT_CLASS,
                STRUCT_SOP_UID,
            );
        }
        Mutation::WrongStudy => put_str(object, tags::STUDY_INSTANCE_UID, VR::UI, "2.25.700"),
        Mutation::WrongFrameOfReference => {
            put_str(object, tags::FRAME_OF_REFERENCE_UID, VR::UI, "2.25.701")
        }
        _ => mutate_fraction_or_beam(object, mutation),
    }
}

fn mutate_reference(object: &mut InMemDicomObject, tag: dicom_core::Tag, class: &str, uid: &str) {
    let mut element = object.take_element(tag).unwrap();
    let item = element.items_mut().unwrap().first_mut().unwrap();
    put_str(item, tags::REFERENCED_SOP_CLASS_UID, VR::UI, class);
    put_str(item, tags::REFERENCED_SOP_INSTANCE_UID, VR::UI, uid);
    object.put(element);
}

fn mutate_fraction_or_beam(object: &mut InMemDicomObject, mutation: Mutation) {
    if matches!(
        mutation,
        Mutation::FractionBeamMismatch | Mutation::DanglingBeam
    ) {
        let mut element = object.take_element(tags::FRACTION_GROUP_SEQUENCE).unwrap();
        let fraction = element.items_mut().unwrap().first_mut().unwrap();
        if matches!(mutation, Mutation::FractionBeamMismatch) {
            put_str(fraction, tags::NUMBER_OF_BEAMS, VR::IS, "0");
        } else {
            let mut references = fraction
                .take_element(tags::REFERENCED_BEAM_SEQUENCE)
                .unwrap();
            put_str(
                references.items_mut().unwrap().first_mut().unwrap(),
                tags::REFERENCED_BEAM_NUMBER,
                VR::IS,
                "2",
            );
            fraction.put(references);
        }
        object.put(element);
        return;
    }
    let mut beam_element = object.take_element(tags::BEAM_SEQUENCE).unwrap();
    let beams = beam_element.items_mut().unwrap();
    if matches!(mutation, Mutation::DuplicateBeam) {
        beams.push(beams[0].clone());
        object.put(beam_element);
        return;
    }
    let beam = beams.first_mut().unwrap();
    match mutation {
        Mutation::MissingZeroCount => {
            beam.take_element(tags::NUMBER_OF_WEDGES).unwrap();
        }
        Mutation::ReverseDevices => {
            let mut element = beam
                .take_element(tags::BEAM_LIMITING_DEVICE_SEQUENCE)
                .unwrap();
            element.items_mut().unwrap().swap(0, 1);
            beam.put(element);
        }
        Mutation::ChangedJaw => {
            let mut points = beam.take_element(tags::CONTROL_POINT_SEQUENCE).unwrap();
            let first = &mut points.items_mut().unwrap()[0];
            let mut positions = first
                .take_element(tags::BEAM_LIMITING_DEVICE_POSITION_SEQUENCE)
                .unwrap();
            put_str(
                &mut positions.items_mut().unwrap()[0],
                tags::LEAF_JAW_POSITIONS,
                VR::DS,
                "-49\\50",
            );
            first.put(positions);
            beam.put(points);
        }
        Mutation::ControlPointCount => {
            let mut points = beam.take_element(tags::CONTROL_POINT_SEQUENCE).unwrap();
            points.items_mut().unwrap().pop();
            beam.put(points);
        }
        Mutation::ControlPointIndex
        | Mutation::ReverseControlPoints
        | Mutation::FirstMeterset
        | Mutation::FinalMeterset
        | Mutation::Isocenter => {
            let mut points = beam.take_element(tags::CONTROL_POINT_SEQUENCE).unwrap();
            let items = points.items_mut().unwrap();
            match mutation {
                Mutation::ControlPointIndex => {
                    put_str(&mut items[1], tags::CONTROL_POINT_INDEX, VR::IS, "0")
                }
                Mutation::ReverseControlPoints => items.swap(0, 1),
                Mutation::FirstMeterset => put_str(
                    &mut items[0],
                    tags::CUMULATIVE_METERSET_WEIGHT,
                    VR::DS,
                    "0.5",
                ),
                Mutation::FinalMeterset => put_str(
                    &mut items[1],
                    tags::CUMULATIVE_METERSET_WEIGHT,
                    VR::DS,
                    "0.5",
                ),
                Mutation::Isocenter => {
                    put_str(&mut items[0], tags::ISOCENTER_POSITION, VR::DS, "1\\0\\0")
                }
                _ => unreachable!(),
            }
            beam.put(points);
        }
        _ => unreachable!(),
    }
    object.put(beam_element);
}

fn put_str(object: &mut InMemDicomObject, tag: dicom_core::Tag, vr: VR, value: &str) {
    object.put(DataElement::new(tag, vr, value));
}

fn put_sequence(object: &mut InMemDicomObject, tag: dicom_core::Tag, items: Vec<InMemDicomObject>) {
    object.put(DataElement::new(tag, VR::SQ, DataSetSequence::from(items)));
}
