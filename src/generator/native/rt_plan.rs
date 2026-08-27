use dicom_core::{DataElement, PrimitiveValue, VR, value::DataSetSequence};
use dicom_dictionary_std::tags;
use dicom_object::InMemDicomObject;

pub(in crate::generator) const RT_PLAN_STORAGE_UID: &str = "1.2.840.10008.5.1.4.1.1.481.5";
pub(in crate::generator) const RT_STRUCTURE_SET_STORAGE_UID: &str = "1.2.840.10008.5.1.4.1.1.481.3";
pub(in crate::generator) const RT_DOSE_STORAGE_UID: &str = "1.2.840.10008.5.1.4.1.1.481.2";
pub(in crate::generator) const RT_PLAN_OUTPUT_FILE: &str = "instance.dcm";
pub(in crate::generator) const RT_PLAN_SERIES_NUMBER: &str = "72";
pub(in crate::generator) const RT_PLAN_LABEL: &str = "DTS_PLAN";
pub(in crate::generator) const RT_PLAN_BEAM_NAME: &str = "DTS_STATIC_AP";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::generator) struct RtPlanInput<'a> {
    pub(in crate::generator) study_instance_uid: &'a str,
    pub(in crate::generator) frame_of_reference_uid: &'a str,
    pub(in crate::generator) series_instance_uid: &'a str,
    pub(in crate::generator) sop_instance_uid: &'a str,
    pub(in crate::generator) structure_set_sop_class_uid: &'a str,
    pub(in crate::generator) structure_set_sop_instance_uid: &'a str,
    pub(in crate::generator) dose_sop_class_uid: &'a str,
    pub(in crate::generator) dose_sop_instance_uid: &'a str,
}

pub(in crate::generator) fn build_rt_plan(
    input: RtPlanInput<'_>,
) -> Result<InMemDicomObject, String> {
    validate_input(input)?;

    let mut object = InMemDicomObject::new_empty();
    put_str(
        &mut object,
        tags::SOP_CLASS_UID,
        VR::UI,
        RT_PLAN_STORAGE_UID,
    );
    put_str(
        &mut object,
        tags::SOP_INSTANCE_UID,
        VR::UI,
        input.sop_instance_uid,
    );
    put_str(&mut object, tags::SYNTHETIC_DATA, VR::CS, "YES");

    put_str(
        &mut object,
        tags::PATIENT_NAME,
        VR::PN,
        "DTS^Synthetic^Patient001",
    );
    put_str(&mut object, tags::PATIENT_ID, VR::LO, "DTS-PATIENT-001");
    put_str(&mut object, tags::PATIENT_BIRTH_DATE, VR::DA, "19700101");
    put_str(&mut object, tags::PATIENT_SEX, VR::CS, "O");

    put_str(
        &mut object,
        tags::STUDY_INSTANCE_UID,
        VR::UI,
        input.study_instance_uid,
    );
    put_str(&mut object, tags::STUDY_DATE, VR::DA, "20260101");
    put_str(&mut object, tags::STUDY_TIME, VR::TM, "000000");
    put_str(&mut object, tags::REFERRING_PHYSICIAN_NAME, VR::PN, "");
    put_str(&mut object, tags::STUDY_ID, VR::SH, "DTS-RTSTRUCT");
    put_str(&mut object, tags::ACCESSION_NUMBER, VR::SH, "");

    put_str(&mut object, tags::MODALITY, VR::CS, "RTPLAN");
    put_str(
        &mut object,
        tags::SERIES_INSTANCE_UID,
        VR::UI,
        input.series_instance_uid,
    );
    put_str(
        &mut object,
        tags::SERIES_NUMBER,
        VR::IS,
        RT_PLAN_SERIES_NUMBER,
    );
    put_str(&mut object, tags::OPERATORS_NAME, VR::PN, "");

    put_str(
        &mut object,
        tags::FRAME_OF_REFERENCE_UID,
        VR::UI,
        input.frame_of_reference_uid,
    );
    put_str(&mut object, tags::POSITION_REFERENCE_INDICATOR, VR::LO, "");

    put_str(&mut object, tags::MANUFACTURER, VR::LO, "dicom-test-suite");
    put_str(&mut object, tags::INSTITUTION_NAME, VR::LO, "");
    put_str(&mut object, tags::INSTITUTION_ADDRESS, VR::ST, "");
    put_str(
        &mut object,
        tags::MANUFACTURER_MODEL_NAME,
        VR::LO,
        "Native Linked RT Plan",
    );
    put_str(
        &mut object,
        tags::DEVICE_SERIAL_NUMBER,
        VR::LO,
        "DTS-RTPLAN-001",
    );
    put_str(
        &mut object,
        tags::SOFTWARE_VERSIONS,
        VR::LO,
        crate::PACKAGE_VERSION,
    );

    put_str(&mut object, tags::INSTANCE_NUMBER, VR::IS, "1");
    put_str(&mut object, tags::RT_PLAN_LABEL, VR::SH, RT_PLAN_LABEL);
    put_str(&mut object, tags::RT_PLAN_DATE, VR::DA, "20260101");
    put_str(&mut object, tags::RT_PLAN_TIME, VR::TM, "000000");
    put_str(&mut object, tags::RT_PLAN_GEOMETRY, VR::CS, "PATIENT");

    put_reference_sequence(
        &mut object,
        tags::REFERENCED_STRUCTURE_SET_SEQUENCE,
        input.structure_set_sop_class_uid,
        input.structure_set_sop_instance_uid,
    );
    put_reference_sequence(
        &mut object,
        tags::REFERENCED_DOSE_SEQUENCE,
        input.dose_sop_class_uid,
        input.dose_sop_instance_uid,
    );
    put_fraction_group_sequence(&mut object);
    put_beam_sequence(&mut object);

    Ok(object)
}

fn put_fraction_group_sequence(object: &mut InMemDicomObject) {
    let mut referenced_beam = InMemDicomObject::new_empty();
    put_str(
        &mut referenced_beam,
        tags::REFERENCED_BEAM_NUMBER,
        VR::IS,
        "1",
    );

    let mut fraction_group = InMemDicomObject::new_empty();
    put_str(
        &mut fraction_group,
        tags::FRACTION_GROUP_NUMBER,
        VR::IS,
        "1",
    );
    put_str(
        &mut fraction_group,
        tags::NUMBER_OF_FRACTIONS_PLANNED,
        VR::IS,
        "1",
    );
    put_str(&mut fraction_group, tags::NUMBER_OF_BEAMS, VR::IS, "1");
    put_str(
        &mut fraction_group,
        tags::NUMBER_OF_BRACHY_APPLICATION_SETUPS,
        VR::IS,
        "0",
    );
    put_sequence(
        &mut fraction_group,
        tags::REFERENCED_BEAM_SEQUENCE,
        vec![referenced_beam],
    );
    put_sequence(object, tags::FRACTION_GROUP_SEQUENCE, vec![fraction_group]);
}

fn put_beam_sequence(object: &mut InMemDicomObject) {
    let mut beam = InMemDicomObject::new_empty();
    put_str(&mut beam, tags::TREATMENT_MACHINE_NAME, VR::SH, "DTS_LINAC");
    put_str(&mut beam, tags::PRIMARY_DOSIMETER_UNIT, VR::CS, "MU");
    put_str(&mut beam, tags::SOURCE_AXIS_DISTANCE, VR::DS, "1000");
    put_sequence(
        &mut beam,
        tags::BEAM_LIMITING_DEVICE_SEQUENCE,
        vec![beam_limiting_device("X"), beam_limiting_device("Y")],
    );
    put_str(&mut beam, tags::BEAM_NUMBER, VR::IS, "1");
    put_str(&mut beam, tags::BEAM_NAME, VR::LO, RT_PLAN_BEAM_NAME);
    put_str(&mut beam, tags::BEAM_TYPE, VR::CS, "STATIC");
    put_str(&mut beam, tags::RADIATION_TYPE, VR::CS, "PHOTON");
    put_str(
        &mut beam,
        tags::TREATMENT_DELIVERY_TYPE,
        VR::CS,
        "TREATMENT",
    );
    put_str(&mut beam, tags::NUMBER_OF_WEDGES, VR::IS, "0");
    put_str(&mut beam, tags::NUMBER_OF_COMPENSATORS, VR::IS, "0");
    put_str(&mut beam, tags::NUMBER_OF_BOLI, VR::IS, "0");
    put_str(&mut beam, tags::NUMBER_OF_BLOCKS, VR::IS, "0");
    put_str(
        &mut beam,
        tags::FINAL_CUMULATIVE_METERSET_WEIGHT,
        VR::DS,
        "1",
    );
    put_str(&mut beam, tags::NUMBER_OF_CONTROL_POINTS, VR::IS, "2");
    put_sequence(
        &mut beam,
        tags::CONTROL_POINT_SEQUENCE,
        vec![first_control_point(), final_control_point()],
    );
    put_sequence(object, tags::BEAM_SEQUENCE, vec![beam]);
}

fn beam_limiting_device(device_type: &str) -> InMemDicomObject {
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
    put_str(&mut item, tags::CONTROL_POINT_INDEX, VR::IS, "0");
    put_str(&mut item, tags::NOMINAL_BEAM_ENERGY, VR::DS, "6");
    put_sequence(
        &mut item,
        tags::BEAM_LIMITING_DEVICE_POSITION_SEQUENCE,
        vec![jaw_position("X"), jaw_position("Y")],
    );
    put_str(&mut item, tags::GANTRY_ANGLE, VR::DS, "0");
    put_str(&mut item, tags::GANTRY_ROTATION_DIRECTION, VR::CS, "NONE");
    put_str(&mut item, tags::BEAM_LIMITING_DEVICE_ANGLE, VR::DS, "0");
    put_str(
        &mut item,
        tags::BEAM_LIMITING_DEVICE_ROTATION_DIRECTION,
        VR::CS,
        "NONE",
    );
    put_str(&mut item, tags::PATIENT_SUPPORT_ANGLE, VR::DS, "0");
    put_str(
        &mut item,
        tags::PATIENT_SUPPORT_ROTATION_DIRECTION,
        VR::CS,
        "NONE",
    );
    put_str(&mut item, tags::TABLE_TOP_VERTICAL_POSITION, VR::DS, "0");
    put_str(
        &mut item,
        tags::TABLE_TOP_LONGITUDINAL_POSITION,
        VR::DS,
        "0",
    );
    put_str(&mut item, tags::TABLE_TOP_LATERAL_POSITION, VR::DS, "0");
    put_f32(&mut item, tags::TABLE_TOP_PITCH_ANGLE, 0.0);
    put_str(
        &mut item,
        tags::TABLE_TOP_PITCH_ROTATION_DIRECTION,
        VR::CS,
        "NONE",
    );
    put_f32(&mut item, tags::TABLE_TOP_ROLL_ANGLE, 0.0);
    put_str(
        &mut item,
        tags::TABLE_TOP_ROLL_ROTATION_DIRECTION,
        VR::CS,
        "NONE",
    );
    put_str(&mut item, tags::ISOCENTER_POSITION, VR::DS, "0\\0\\0");
    put_str(&mut item, tags::CUMULATIVE_METERSET_WEIGHT, VR::DS, "0");
    item
}

fn final_control_point() -> InMemDicomObject {
    let mut item = InMemDicomObject::new_empty();
    put_str(&mut item, tags::CONTROL_POINT_INDEX, VR::IS, "1");
    put_str(&mut item, tags::CUMULATIVE_METERSET_WEIGHT, VR::DS, "1");
    item
}

fn jaw_position(device_type: &str) -> InMemDicomObject {
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

fn put_reference_sequence(
    object: &mut InMemDicomObject,
    sequence_tag: dicom_core::Tag,
    sop_class_uid: &str,
    sop_instance_uid: &str,
) {
    let mut item = InMemDicomObject::new_empty();
    put_str(
        &mut item,
        tags::REFERENCED_SOP_CLASS_UID,
        VR::UI,
        sop_class_uid,
    );
    put_str(
        &mut item,
        tags::REFERENCED_SOP_INSTANCE_UID,
        VR::UI,
        sop_instance_uid,
    );
    put_sequence(object, sequence_tag, vec![item]);
}

fn put_sequence(object: &mut InMemDicomObject, tag: dicom_core::Tag, items: Vec<InMemDicomObject>) {
    object.put(DataElement::new(tag, VR::SQ, DataSetSequence::from(items)));
}

fn validate_input(input: RtPlanInput<'_>) -> Result<(), String> {
    if input.structure_set_sop_class_uid != RT_STRUCTURE_SET_STORAGE_UID {
        return Err(format!(
            "Structure Set SOP Class UID must be {RT_STRUCTURE_SET_STORAGE_UID}"
        ));
    }
    if input.dose_sop_class_uid != RT_DOSE_STORAGE_UID {
        return Err(format!("Dose SOP Class UID must be {RT_DOSE_STORAGE_UID}"));
    }

    let instance_uids = [
        ("Study Instance UID", input.study_instance_uid),
        ("Frame of Reference UID", input.frame_of_reference_uid),
        ("Series Instance UID", input.series_instance_uid),
        ("SOP Instance UID", input.sop_instance_uid),
        (
            "Structure Set SOP Instance UID",
            input.structure_set_sop_instance_uid,
        ),
        ("Dose SOP Instance UID", input.dose_sop_instance_uid),
    ];
    for (name, value) in instance_uids {
        validate_uid(name, value)?;
    }
    for left in 0..instance_uids.len() {
        for right in left + 1..instance_uids.len() {
            if instance_uids[left].1 == instance_uids[right].1 {
                return Err(format!(
                    "{} and {} must be distinct",
                    instance_uids[left].0, instance_uids[right].0
                ));
            }
        }
    }
    Ok(())
}

fn validate_uid(name: &str, value: &str) -> Result<(), String> {
    if value.is_empty() {
        return Err(format!("{name} must not be empty"));
    }
    if value.len() > 64
        || value.starts_with('.')
        || value.ends_with('.')
        || value.split('.').any(|part| part.is_empty())
        || value
            .split('.')
            .any(|part| part.len() > 1 && part.starts_with('0'))
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || byte == b'.')
    {
        return Err(format!("{name} must be a valid DICOM UID"));
    }
    Ok(())
}

fn put_str(object: &mut InMemDicomObject, tag: dicom_core::Tag, vr: VR, value: &str) {
    object.put(DataElement::new(tag, vr, value));
}

fn put_f32(object: &mut InMemDicomObject, tag: dicom_core::Tag, value: f32) {
    object.put(DataElement::new(tag, VR::FL, PrimitiveValue::from(value)));
}
