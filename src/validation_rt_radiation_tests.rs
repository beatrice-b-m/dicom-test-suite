use std::path::PathBuf;

use dicom_core::{
    DataElement, PrimitiveValue, VR,
    value::{C, DataSetSequence},
};
use dicom_dictionary_std::{tags, uids};
use dicom_object::{FileMetaTableBuilder, InMemDicomObject};

use super::{
    RtRadiationExpectations, RtRadiationSetExpectations, validate_rt_radiation_file,
    validate_rt_radiation_set_file,
};
use crate::rt_radiation_manifest::{
    CArmRtRadiationInput, RtRadiationSetInput, minimal_carm_rt_radiation_expected,
    minimal_rt_radiation_set_expected,
};

const PLAN_CLASS: &str = "1.2.840.10008.5.1.4.1.1.481.5";
const RADIATION_CLASS: &str = "1.2.840.10008.5.1.4.1.1.481.13";
const SET_CLASS: &str = "1.2.840.10008.5.1.4.1.1.481.12";
const STUDY_UID: &str = "2.25.801";
const FRAME_UID: &str = "2.25.802";
const PLAN_SERIES_UID: &str = "2.25.803";
const PLAN_SOP_UID: &str = "2.25.804";
const RADIATION_SERIES_UID: &str = "2.25.805";
const RADIATION_SOP_UID: &str = "2.25.806";
const SET_SERIES_UID: &str = "2.25.807";
const SET_SOP_UID: &str = "2.25.808";
const GROUP_UID: &str = "2.25.809";
const IMPLEMENTATION_UID: &str = "2.25.999";
const HASH_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const HASH_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

#[derive(Clone, Copy)]
enum RadiationMutation {
    None,
    WrongLabelVr,
    WrongPlan,
    WrongTechnique,
    WrongDevice,
    WrongMatrix,
    DuplicateControlPoint,
    ReverseControlPoints,
    BreakInheritance,
    AddRecordedAttribute,
    AddSynchronization,
    AddImageMetadata,
}

#[derive(Clone, Copy)]
enum SetMutation {
    None,
    WrongPlan,
    WrongDirectRadiation,
    DuplicateMembership,
    SwapCommonReferences,
    AddDoseContribution,
    AddSynchronization,
    AddImageMetadata,
}

#[test]
fn accepts_exact_second_generation_rt_pair() {
    let radiation = write_radiation("valid", RadiationMutation::None);
    let set = write_set("valid", SetMutation::None);
    assert_eq!(
        validate_rt_radiation_file(&radiation, &radiation_expectations())
            .expect("valid Radiation")
            .validation["status"],
        "passed"
    );
    assert_eq!(
        validate_rt_radiation_set_file(&set, &set_expectations())
            .expect("valid Set")
            .validation["status"],
        "passed"
    );
    std::fs::remove_file(radiation).ok();
    std::fs::remove_file(set).ok();
}

#[test]
fn rejects_radiation_vr_graph_code_device_position_control_point_and_absence_mutations() {
    for (label, mutation, finding) in [
        (
            "label-vr",
            RadiationMutation::WrongLabelVr,
            "rt_radiation_user_content_label",
        ),
        (
            "plan",
            RadiationMutation::WrongPlan,
            "rt_radiation_definition_source_sop_instance_uid",
        ),
        (
            "technique",
            RadiationMutation::WrongTechnique,
            "rt_radiation_treatment_technique_value",
        ),
        (
            "device",
            RadiationMutation::WrongDevice,
            "rt_radiation_device_serial",
        ),
        (
            "matrix",
            RadiationMutation::WrongMatrix,
            "rt_radiation_mapping_matrix",
        ),
        (
            "duplicate-cp",
            RadiationMutation::DuplicateControlPoint,
            "rt_radiation_control_point_sequence_count",
        ),
        (
            "reverse-cp",
            RadiationMutation::ReverseControlPoints,
            "rt_radiation_control_point_1_index",
        ),
        (
            "inheritance",
            RadiationMutation::BreakInheritance,
            "rt_radiation_control_point_2_position_inherited",
        ),
        (
            "recorded",
            RadiationMutation::AddRecordedAttribute,
            "rt_radiation_control_point_2_recorded_absent",
        ),
        (
            "synchronization",
            RadiationMutation::AddSynchronization,
            "rt_radiation_synchronization_for_absent",
        ),
        (
            "image-metadata",
            RadiationMutation::AddImageMetadata,
            "rt_radiation_rows_absent",
        ),
    ] {
        let path = write_radiation(label, mutation);
        let error = validate_rt_radiation_file(&path, &radiation_expectations())
            .expect_err("mutation must fail")
            .to_string();
        assert!(error.contains(finding), "{label}: {error}");
        std::fs::remove_file(path).ok();
    }
}

#[test]
fn rejects_set_reference_order_membership_and_absence_mutations() {
    for (label, mutation, finding) in [
        (
            "plan",
            SetMutation::WrongPlan,
            "rt_radiation_set_definition_source_sop_instance_uid",
        ),
        (
            "direct",
            SetMutation::WrongDirectRadiation,
            "rt_radiation_set_direct_radiation_sop_instance_uid",
        ),
        (
            "membership",
            SetMutation::DuplicateMembership,
            "rt_radiation_set_group_membership_count",
        ),
        (
            "common-order",
            SetMutation::SwapCommonReferences,
            "rt_radiation_set_common_plan_series_uid",
        ),
        (
            "dose-contribution",
            SetMutation::AddDoseContribution,
            "rt_radiation_set_dose_contribution_absent",
        ),
        (
            "synchronization",
            SetMutation::AddSynchronization,
            "rt_radiation_set_synchronization_for_absent",
        ),
        (
            "image-metadata",
            SetMutation::AddImageMetadata,
            "rt_radiation_set_rows_absent",
        ),
    ] {
        let path = write_set(label, mutation);
        let error = validate_rt_radiation_set_file(&path, &set_expectations())
            .expect_err("mutation must fail")
            .to_string();
        assert!(error.contains(finding), "{label}: {error}");
        std::fs::remove_file(path).ok();
    }
}

#[test]
fn rejects_manifest_graph_and_absence_weakening() {
    let radiation = write_radiation("manifest", RadiationMutation::None);
    let mut radiation_expected = radiation_expectations();
    radiation_expected
        .expected_rt_radiation
        .absent_content
        .pixel_data = false;
    assert!(
        validate_rt_radiation_file(&radiation, &radiation_expected)
            .expect_err("absence weakening")
            .to_string()
            .contains("rt_radiation_manifest_absence_contract")
    );
    let set = write_set("manifest", SetMutation::None);
    let mut set_expected = set_expectations();
    set_expected
        .expected_rt_radiation_set
        .common_instance_references
        .swap(0, 1);
    assert!(
        validate_rt_radiation_set_file(&set, &set_expected)
            .expect_err("graph drift")
            .to_string()
            .contains("rt_radiation_set_manifest_graph_contract")
    );
    std::fs::remove_file(radiation).ok();
    std::fs::remove_file(set).ok();
}

fn radiation_expectations() -> RtRadiationExpectations<'static> {
    RtRadiationExpectations {
        implementation_class_uid: IMPLEMENTATION_UID,
        synthetic_data: "YES",
        expected_rt_radiation: minimal_carm_rt_radiation_expected(CArmRtRadiationInput {
            sop_instance_uid: RADIATION_SOP_UID,
            study_instance_uid: STUDY_UID,
            series_instance_uid: RADIATION_SERIES_UID,
            frame_of_reference_uid: FRAME_UID,
            plan_series_instance_uid: PLAN_SERIES_UID,
            plan_sop_instance_uid: PLAN_SOP_UID,
            plan_sha256: HASH_A,
            software_versions: crate::PACKAGE_VERSION,
        }),
    }
}

fn set_expectations() -> RtRadiationSetExpectations<'static> {
    RtRadiationSetExpectations {
        implementation_class_uid: IMPLEMENTATION_UID,
        synthetic_data: "YES",
        expected_rt_radiation_set: minimal_rt_radiation_set_expected(RtRadiationSetInput {
            sop_instance_uid: SET_SOP_UID,
            study_instance_uid: STUDY_UID,
            series_instance_uid: SET_SERIES_UID,
            frame_of_reference_uid: FRAME_UID,
            treatment_position_group_uid: GROUP_UID,
            plan_series_instance_uid: PLAN_SERIES_UID,
            plan_sop_instance_uid: PLAN_SOP_UID,
            plan_sha256: HASH_A,
            radiation_series_instance_uid: RADIATION_SERIES_UID,
            radiation_sop_instance_uid: RADIATION_SOP_UID,
            radiation_sha256: HASH_B,
            software_versions: crate::PACKAGE_VERSION,
        }),
    }
}

fn write_radiation(label: &str, mutation: RadiationMutation) -> PathBuf {
    let mut object = radiation_object();
    match mutation {
        RadiationMutation::None => {}
        RadiationMutation::WrongLabelVr => put_str(
            &mut object,
            tags::USER_CONTENT_LABEL,
            VR::LO,
            "DTS_RADIATION",
        ),
        RadiationMutation::WrongPlan => {
            with_sequence_items_mut(&mut object, tags::DEFINITION_SOURCE_SEQUENCE, |items| {
                items[0].put(DataElement::new(
                    tags::REFERENCED_SOP_INSTANCE_UID,
                    VR::UI,
                    "2.25.9999",
                ));
            })
        }
        RadiationMutation::WrongTechnique => with_sequence_items_mut(
            &mut object,
            tags::RT_TREATMENT_TECHNIQUE_CODE_SEQUENCE,
            |items| {
                items[0].put(DataElement::new(tags::CODE_VALUE, VR::SH, "BAD"));
            },
        ),
        RadiationMutation::WrongDevice => with_sequence_items_mut(
            &mut object,
            tags::TREATMENT_DEVICE_IDENTIFICATION_SEQUENCE,
            |items| {
                items[0].put(DataElement::new(tags::DEVICE_SERIAL_NUMBER, VR::LO, "BAD"));
            },
        ),
        RadiationMutation::WrongMatrix => {
            with_sequence_items_mut(&mut object, tags::TREATMENT_POSITION_SEQUENCE, |items| {
                items[0].put(DataElement::new(
                    tags::IMAGE_TO_EQUIPMENT_MAPPING_MATRIX,
                    VR::DS,
                    r"0\0\0\0\0\1\0\0\0\0\1\0\0\0\0\1",
                ));
            })
        }
        RadiationMutation::DuplicateControlPoint => {
            let item = sequence_items(&object, tags::C_ARM_PHOTON_ELECTRON_CONTROL_POINT_SEQUENCE)
                [0]
            .clone();
            with_sequence_items_mut(
                &mut object,
                tags::C_ARM_PHOTON_ELECTRON_CONTROL_POINT_SEQUENCE,
                |items| items.push(item),
            );
        }
        RadiationMutation::ReverseControlPoints => with_sequence_items_mut(
            &mut object,
            tags::C_ARM_PHOTON_ELECTRON_CONTROL_POINT_SEQUENCE,
            |items| items.reverse(),
        ),
        RadiationMutation::BreakInheritance => with_sequence_items_mut(
            &mut object,
            tags::C_ARM_PHOTON_ELECTRON_CONTROL_POINT_SEQUENCE,
            |items| {
                items[1].put(DataElement::new(
                    tags::REFERENCED_TREATMENT_POSITION_INDEX,
                    VR::US,
                    PrimitiveValue::from(1_u16),
                ));
            },
        ),
        RadiationMutation::AddRecordedAttribute => with_sequence_items_mut(
            &mut object,
            tags::C_ARM_PHOTON_ELECTRON_CONTROL_POINT_SEQUENCE,
            |items| {
                items[1].put(DataElement::new(
                    tags::RECORDED_RT_CONTROL_POINT_DATE_TIME,
                    VR::DT,
                    "20260101000000",
                ));
            },
        ),
        RadiationMutation::AddSynchronization => put_str(
            &mut object,
            tags::SYNCHRONIZATION_FRAME_OF_REFERENCE_UID,
            VR::UI,
            "2.25.9998",
        ),
        RadiationMutation::AddImageMetadata => {
            object.put(DataElement::new(
                tags::ROWS,
                VR::US,
                PrimitiveValue::from(1_u16),
            ));
        }
    };
    write_part10(
        label,
        "radiation",
        object,
        RADIATION_CLASS,
        RADIATION_SOP_UID,
    )
}

fn write_set(label: &str, mutation: SetMutation) -> PathBuf {
    let mut object = set_object();
    match mutation {
        SetMutation::None => {}
        SetMutation::WrongPlan => {
            with_sequence_items_mut(&mut object, tags::DEFINITION_SOURCE_SEQUENCE, |items| {
                items[0].put(DataElement::new(
                    tags::REFERENCED_SOP_INSTANCE_UID,
                    VR::UI,
                    "2.25.9999",
                ));
            })
        }
        SetMutation::WrongDirectRadiation => {
            with_sequence_items_mut(&mut object, tags::RT_RADIATION_SEQUENCE, |items| {
                items[0].put(DataElement::new(
                    tags::REFERENCED_SOP_INSTANCE_UID,
                    VR::UI,
                    "2.25.9999",
                ));
            })
        }
        SetMutation::DuplicateMembership => {
            with_sequence_items_mut(
                &mut object,
                tags::TREATMENT_POSITION_GROUP_SEQUENCE,
                |groups| {
                    let group = &mut groups[0];
                    let item =
                        sequence_items(group, tags::REFERENCED_RT_RADIATION_SEQUENCE)[0].clone();
                    with_sequence_items_mut(
                        group,
                        tags::REFERENCED_RT_RADIATION_SEQUENCE,
                        |items| items.push(item),
                    );
                },
            );
        }
        SetMutation::SwapCommonReferences => {
            with_sequence_items_mut(&mut object, tags::REFERENCED_SERIES_SEQUENCE, |items| {
                items.reverse()
            })
        }
        SetMutation::AddDoseContribution => {
            put_sequence(&mut object, tags::RADIATION_DOSE_SEQUENCE, vec![])
        }
        SetMutation::AddSynchronization => put_str(
            &mut object,
            tags::SYNCHRONIZATION_FRAME_OF_REFERENCE_UID,
            VR::UI,
            "2.25.9998",
        ),
        SetMutation::AddImageMetadata => {
            object.put(DataElement::new(
                tags::ROWS,
                VR::US,
                PrimitiveValue::from(1_u16),
            ));
        }
    };
    write_part10(label, "set", object, SET_CLASS, SET_SOP_UID)
}

fn write_part10(
    label: &str,
    kind: &str,
    object: InMemDicomObject,
    class: &str,
    sop: &str,
) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "dts-rt-radiation-validation-{}-{kind}-{label}.dcm",
        std::process::id()
    ));
    object
        .with_meta(
            FileMetaTableBuilder::new()
                .media_storage_sop_class_uid(class)
                .media_storage_sop_instance_uid(sop)
                .transfer_syntax(uids::EXPLICIT_VR_LITTLE_ENDIAN)
                .implementation_class_uid(IMPLEMENTATION_UID),
        )
        .expect("meta")
        .write_to_file(&path)
        .expect("fixture");
    path
}

fn radiation_object() -> InMemDicomObject {
    let mut object = base(
        RADIATION_CLASS,
        RADIATION_SOP_UID,
        RADIATION_SERIES_UID,
        "74",
        "Native C-Arm Photon-Electron Radiation",
    );
    common(&mut object, "DTS_RADIATION");
    put_str(
        &mut object,
        tags::RT_RADIATION_PHYSICAL_AND_GEOMETRIC_CONTENT_DETAIL_FLAG,
        VR::CS,
        "IDENT_ONLY",
    );
    put_str(&mut object, tags::RT_RECORD_FLAG, VR::CS, "NO");
    put_code(
        &mut object,
        tags::RT_TREATMENT_TECHNIQUE_CODE_SEQUENCE,
        "130102",
        "DCM",
        "Static Beam",
    );
    let mut definition = sop_reference(PLAN_CLASS, PLAN_SOP_UID);
    put_str(&mut definition, tags::REFERENCED_BEAM_NUMBER, VR::IS, "1");
    put_sequence(
        &mut object,
        tags::DEFINITION_SOURCE_SEQUENCE,
        vec![definition],
    );
    let mut device = InMemDicomObject::new_empty();
    for (tag, vr, value) in [
        (tags::MANUFACTURER, VR::LO, "dicom-test-suite"),
        (tags::MANUFACTURER_MODEL_NAME, VR::LO, "DTS C-Arm LINAC"),
        (tags::MANUFACTURER_MODEL_VERSION, VR::LO, "1"),
        (tags::MANUFACTURER_DEVICE_CLASS_UID, VR::UI, ""),
        (tags::DEVICE_SERIAL_NUMBER, VR::LO, "DTS-LINAC-001"),
        (tags::SOFTWARE_VERSIONS, VR::LO, crate::PACKAGE_VERSION),
        (tags::DEVICE_ALTERNATE_IDENTIFIER, VR::UC, ""),
        (tags::DEVICE_LABEL, VR::LO, "DTS_LINAC"),
        (
            tags::MANUFACTURER_DEVICE_IDENTIFIER,
            VR::ST,
            "DTS-LINAC-001",
        ),
    ] {
        put_str(&mut device, tag, vr, value);
    }
    put_code(
        &mut device,
        tags::DEVICE_TYPE_CODE_SEQUENCE,
        "130361",
        "DCM",
        "Radiotherapy Treatment Device",
    );
    put_sequence(
        &mut object,
        tags::TREATMENT_DEVICE_IDENTIFICATION_SEQUENCE,
        vec![device],
    );
    put_code(
        &mut object,
        tags::RADIATION_DOSIMETER_UNIT_SEQUENCE,
        "{MU}",
        "UCUM",
        "Monitor Units",
    );
    put_code(
        &mut object,
        tags::RT_DEVICE_DISTANCE_REFERENCE_LOCATION_CODE_SEQUENCE,
        "130358",
        "DCM",
        "Nominal Radiation Source Location",
    );
    put_f64(
        &mut object,
        tags::RT_BEAM_MODIFIER_DEFINITION_DISTANCE,
        500.0,
    );
    put_str(
        &mut object,
        tags::EQUIPMENT_FRAME_OF_REFERENCE_UID,
        VR::UI,
        "1.2.840.10008.1.4.3.1",
    );
    put_sequence(
        &mut object,
        tags::EQUIPMENT_REFERENCE_POINT_COORDINATES_SEQUENCE,
        vec![],
    );
    put_u16(&mut object, tags::NUMBER_OF_PATIENT_SUPPORT_DEVICES, 0);
    put_f64(&mut object, tags::RADIATION_SOURCE_AXIS_DISTANCE, 1000.0);
    let mut position = InMemDicomObject::new_empty();
    put_u16(&mut position, tags::TREATMENT_POSITION_INDEX, 1);
    put_code(
        &mut position,
        tags::PATIENT_ORIENTATION_CODE_SEQUENCE,
        "102538003",
        "SCT",
        "recumbent",
    );
    put_code(
        &mut position,
        tags::PATIENT_ORIENTATION_MODIFIER_CODE_SEQUENCE,
        "40199007",
        "SCT",
        "supine",
    );
    put_code(
        &mut position,
        tags::PATIENT_EQUIPMENT_RELATIONSHIP_CODE_SEQUENCE,
        "102540008",
        "SCT",
        "headfirst",
    );
    put_str(
        &mut position,
        tags::IMAGE_TO_EQUIPMENT_MAPPING_MATRIX,
        VR::DS,
        r"1\0\0\0\0\1\0\0\0\0\1\0\0\0\0\1",
    );
    put_sequence(
        &mut position,
        tags::PATIENT_LOCATION_COORDINATES_SEQUENCE,
        vec![],
    );
    put_sequence(
        &mut position,
        tags::PATIENT_SUPPORT_POSITION_SEQUENCE,
        vec![],
    );
    put_sequence(
        &mut object,
        tags::TREATMENT_POSITION_SEQUENCE,
        vec![position],
    );
    let mut first = InMemDicomObject::new_empty();
    put_u16(&mut first, tags::RT_CONTROL_POINT_INDEX, 1);
    put_f64(&mut first, tags::CUMULATIVE_METERSET, 0.0);
    put_u16(&mut first, tags::REFERENCED_TREATMENT_POSITION_INDEX, 1);
    put_empty(&mut first, tags::DELIVERY_RATE, VR::FD);
    put_f64(&mut first, tags::SOURCE_ROLL_ANGLE, 0.0);
    put_f64(&mut first, tags::RT_BEAM_LIMITING_DEVICE_ANGLE, 0.0);
    put_empty(&mut first, tags::SOURCE_TO_PATIENT_SURFACE_DISTANCE, VR::FD);
    put_empty(
        &mut first,
        tags::SOURCE_TO_EXTERNAL_CONTOUR_DISTANCE,
        VR::FL,
    );
    let mut second = InMemDicomObject::new_empty();
    put_u16(&mut second, tags::RT_CONTROL_POINT_INDEX, 2);
    put_f64(&mut second, tags::CUMULATIVE_METERSET, 100.0);
    put_u16(&mut object, tags::NUMBER_OF_RT_CONTROL_POINTS, 2);
    put_sequence(
        &mut object,
        tags::C_ARM_PHOTON_ELECTRON_CONTROL_POINT_SEQUENCE,
        vec![first, second],
    );
    put_common(&mut object, &[(PLAN_SERIES_UID, PLAN_CLASS, PLAN_SOP_UID)]);
    object
}

fn set_object() -> InMemDicomObject {
    let mut object = base(
        SET_CLASS,
        SET_SOP_UID,
        SET_SERIES_UID,
        "75",
        "Native RT Radiation Set",
    );
    common(&mut object, "DTS_RADSET");
    put_u16(&mut object, tags::INTENDED_NUMBER_OF_FRACTIONS, 1);
    put_sequence(
        &mut object,
        tags::REFERENCED_RT_PHYSICIAN_INTENT_SEQUENCE,
        vec![],
    );
    put_str(
        &mut object,
        tags::RT_RADIATION_SET_INTENT,
        VR::CS,
        "TREATMENT",
    );
    let reference = sop_reference(RADIATION_CLASS, RADIATION_SOP_UID);
    let mut group = InMemDicomObject::new_empty();
    put_str(
        &mut group,
        tags::TREATMENT_POSITION_GROUP_UID,
        VR::UI,
        GROUP_UID,
    );
    put_str(
        &mut group,
        tags::TREATMENT_POSITION_GROUP_LABEL,
        VR::LO,
        "DTS_TPG_1",
    );
    put_sequence(
        &mut group,
        tags::REFERENCED_RT_RADIATION_SEQUENCE,
        vec![reference.clone()],
    );
    put_sequence(
        &mut object,
        tags::TREATMENT_POSITION_GROUP_SEQUENCE,
        vec![group],
    );
    put_sequence(&mut object, tags::RT_RADIATION_SEQUENCE, vec![reference]);
    put_sequence(
        &mut object,
        tags::DEFINITION_SOURCE_SEQUENCE,
        vec![sop_reference(PLAN_CLASS, PLAN_SOP_UID)],
    );
    put_common(
        &mut object,
        &[
            (PLAN_SERIES_UID, PLAN_CLASS, PLAN_SOP_UID),
            (RADIATION_SERIES_UID, RADIATION_CLASS, RADIATION_SOP_UID),
        ],
    );
    object
}

fn base(class: &str, sop: &str, series: &str, number: &str, model: &str) -> InMemDicomObject {
    let mut object = InMemDicomObject::new_empty();
    for (tag, vr, value) in [
        (tags::SOP_CLASS_UID, VR::UI, class),
        (tags::SOP_INSTANCE_UID, VR::UI, sop),
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
        (tags::SERIES_INSTANCE_UID, VR::UI, series),
        (tags::SERIES_NUMBER, VR::IS, number),
        (tags::SERIES_DATE, VR::DA, "20260101"),
        (tags::SERIES_TIME, VR::TM, "000000"),
        (tags::FRAME_OF_REFERENCE_UID, VR::UI, FRAME_UID),
        (tags::POSITION_REFERENCE_INDICATOR, VR::LO, ""),
        (tags::MANUFACTURER, VR::LO, "dicom-test-suite"),
        (tags::MANUFACTURER_MODEL_NAME, VR::LO, model),
        (tags::DEVICE_SERIAL_NUMBER, VR::LO, "DTS-LINAC-001"),
        (tags::SOFTWARE_VERSIONS, VR::LO, crate::PACKAGE_VERSION),
    ] {
        put_str(&mut object, tag, vr, value);
    }
    object
}

fn common(object: &mut InMemDicomObject, label: &str) {
    for (tag, vr, value) in [
        (tags::INSTANCE_CREATION_DATE, VR::DA, "20260101"),
        (tags::INSTANCE_CREATION_TIME, VR::TM, "000000"),
        (tags::CONTENT_DATE, VR::DA, "20260101"),
        (tags::CONTENT_TIME, VR::TM, "000000"),
        (tags::USER_CONTENT_LABEL, VR::SH, label),
        (tags::CONTENT_DESCRIPTION, VR::LO, ""),
    ] {
        put_str(object, tag, vr, value);
    }
    put_sequence(object, tags::AUTHOR_IDENTIFICATION_SEQUENCE, vec![]);
}
fn put_common(object: &mut InMemDicomObject, refs: &[(&str, &str, &str)]) {
    let items = refs
        .iter()
        .map(|(series, class, sop)| {
            let mut item = InMemDicomObject::new_empty();
            put_str(&mut item, tags::SERIES_INSTANCE_UID, VR::UI, series);
            put_sequence(
                &mut item,
                tags::REFERENCED_INSTANCE_SEQUENCE,
                vec![sop_reference(class, sop)],
            );
            item
        })
        .collect();
    put_sequence(object, tags::REFERENCED_SERIES_SEQUENCE, items);
}
fn sop_reference(class: &str, sop: &str) -> InMemDicomObject {
    InMemDicomObject::from_element_iter([
        DataElement::new(tags::REFERENCED_SOP_CLASS_UID, VR::UI, class),
        DataElement::new(tags::REFERENCED_SOP_INSTANCE_UID, VR::UI, sop),
    ])
}
fn put_code(
    object: &mut InMemDicomObject,
    tag: dicom_core::Tag,
    value: &str,
    scheme: &str,
    meaning: &str,
) {
    put_sequence(
        object,
        tag,
        vec![InMemDicomObject::from_element_iter([
            DataElement::new(tags::CODE_VALUE, VR::SH, value),
            DataElement::new(tags::CODING_SCHEME_DESIGNATOR, VR::SH, scheme),
            DataElement::new(tags::CODE_MEANING, VR::LO, meaning),
        ])],
    );
}
fn put_sequence(object: &mut InMemDicomObject, tag: dicom_core::Tag, items: Vec<InMemDicomObject>) {
    object.put(DataElement::new(tag, VR::SQ, DataSetSequence::from(items)));
}
fn put_str(object: &mut InMemDicomObject, tag: dicom_core::Tag, vr: VR, value: &str) {
    object.put(DataElement::new(tag, vr, value));
}
fn put_u16(object: &mut InMemDicomObject, tag: dicom_core::Tag, value: u16) {
    object.put(DataElement::new(tag, VR::US, PrimitiveValue::from(value)));
}
fn put_f64(object: &mut InMemDicomObject, tag: dicom_core::Tag, value: f64) {
    object.put(DataElement::new(tag, VR::FD, PrimitiveValue::from(value)));
}
fn put_empty(object: &mut InMemDicomObject, tag: dicom_core::Tag, vr: VR) {
    object.put(DataElement::new(tag, vr, PrimitiveValue::Empty));
}
fn sequence_items(object: &InMemDicomObject, tag: dicom_core::Tag) -> &[InMemDicomObject] {
    object
        .element(tag)
        .expect("sequence")
        .items()
        .expect("items")
}
fn with_sequence_items_mut(
    object: &mut InMemDicomObject,
    tag: dicom_core::Tag,
    mutate: impl FnOnce(&mut C<InMemDicomObject>),
) {
    let mut element = object.take_element(tag).expect("sequence");
    mutate(element.items_mut().expect("items"));
    object.put(element);
}
