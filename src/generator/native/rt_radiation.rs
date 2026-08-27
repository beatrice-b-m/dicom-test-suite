use dicom_core::{DataElement, PrimitiveValue, VR, value::DataSetSequence};
use dicom_dictionary_std::tags;
use dicom_object::InMemDicomObject;

pub(in crate::generator) const C_ARM_PHOTON_ELECTRON_RADIATION_STORAGE_UID: &str =
    "1.2.840.10008.5.1.4.1.1.481.13";
pub(in crate::generator) const RT_RADIATION_SET_STORAGE_UID: &str =
    "1.2.840.10008.5.1.4.1.1.481.12";
pub(in crate::generator) const RT_PLAN_STORAGE_UID: &str = "1.2.840.10008.5.1.4.1.1.481.5";
pub(in crate::generator) const RT_RADIATION_OUTPUT_FILE: &str = "instance.dcm";
pub(in crate::generator) const RT_RADIATION_SET_OUTPUT_FILE: &str = "instance.dcm";
pub(in crate::generator) const RT_RADIATION_SERIES_NUMBER: &str = "74";
pub(in crate::generator) const RT_RADIATION_SET_SERIES_NUMBER: &str = "75";
pub(in crate::generator) const RT_RADIATION_USER_CONTENT_LABEL: &str = "DTS_RADIATION";
pub(in crate::generator) const RT_RADIATION_SET_USER_CONTENT_LABEL: &str = "DTS_RADSET";
pub(in crate::generator) const RT_TREATMENT_POSITION_GROUP_LABEL: &str = "DTS_TPG_1";
pub(in crate::generator) const RT_DEVICE_LABEL: &str = "DTS_LINAC";
pub(in crate::generator) const RT_DEVICE_SERIAL_NUMBER: &str = "DTS-LINAC-001";
pub(in crate::generator) const EQUIPMENT_FRAME_OF_REFERENCE_UID: &str = "1.2.840.10008.1.4.3.1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::generator) struct RtRadiationInput<'a> {
    pub(in crate::generator) study_instance_uid: &'a str,
    pub(in crate::generator) frame_of_reference_uid: &'a str,
    pub(in crate::generator) series_instance_uid: &'a str,
    pub(in crate::generator) sop_instance_uid: &'a str,
    pub(in crate::generator) plan_series_instance_uid: &'a str,
    pub(in crate::generator) plan_sop_class_uid: &'a str,
    pub(in crate::generator) plan_sop_instance_uid: &'a str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::generator) struct RtRadiationSetInput<'a> {
    pub(in crate::generator) study_instance_uid: &'a str,
    pub(in crate::generator) frame_of_reference_uid: &'a str,
    pub(in crate::generator) series_instance_uid: &'a str,
    pub(in crate::generator) sop_instance_uid: &'a str,
    pub(in crate::generator) plan_series_instance_uid: &'a str,
    pub(in crate::generator) plan_sop_class_uid: &'a str,
    pub(in crate::generator) plan_sop_instance_uid: &'a str,
    pub(in crate::generator) radiation_series_instance_uid: &'a str,
    pub(in crate::generator) radiation_sop_class_uid: &'a str,
    pub(in crate::generator) radiation_sop_instance_uid: &'a str,
    pub(in crate::generator) treatment_position_group_uid: &'a str,
}

pub(in crate::generator) fn build_rt_radiation(
    input: RtRadiationInput<'_>,
) -> Result<InMemDicomObject, String> {
    validate_radiation_input(input)?;

    let mut object = base_object(
        C_ARM_PHOTON_ELECTRON_RADIATION_STORAGE_UID,
        input.sop_instance_uid,
        input.study_instance_uid,
        input.series_instance_uid,
        input.frame_of_reference_uid,
        RT_RADIATION_SERIES_NUMBER,
        "Native C-Arm Photon-Electron Radiation",
    );
    put_radiotherapy_common(&mut object, RT_RADIATION_USER_CONTENT_LABEL);
    put_treatment_device(&mut object);
    put_code_sequence(
        &mut object,
        tags::RADIATION_DOSIMETER_UNIT_SEQUENCE,
        "{MU}",
        "UCUM",
        "Monitor Units",
    );
    put_code_sequence(
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
        EQUIPMENT_FRAME_OF_REFERENCE_UID,
    );
    put_empty_sequence(
        &mut object,
        tags::EQUIPMENT_REFERENCE_POINT_COORDINATES_SEQUENCE,
    );
    put_u16(&mut object, tags::NUMBER_OF_PATIENT_SUPPORT_DEVICES, 0);
    put_f64(&mut object, tags::RADIATION_SOURCE_AXIS_DISTANCE, 1000.0);

    put_str(
        &mut object,
        tags::RT_RADIATION_PHYSICAL_AND_GEOMETRIC_CONTENT_DETAIL_FLAG,
        VR::CS,
        "IDENT_ONLY",
    );
    put_str(&mut object, tags::RT_RECORD_FLAG, VR::CS, "NO");
    put_code_sequence(
        &mut object,
        tags::RT_TREATMENT_TECHNIQUE_CODE_SEQUENCE,
        "130102",
        "DCM",
        "Static Beam",
    );
    put_definition_source(
        &mut object,
        input.plan_sop_class_uid,
        input.plan_sop_instance_uid,
        Some("1"),
    );
    put_common_instance_references(
        &mut object,
        &[(
            input.plan_series_instance_uid,
            input.plan_sop_class_uid,
            input.plan_sop_instance_uid,
        )],
    );
    put_treatment_position(&mut object);
    put_control_points(&mut object);

    Ok(object)
}

pub(in crate::generator) fn build_rt_radiation_set(
    input: RtRadiationSetInput<'_>,
) -> Result<InMemDicomObject, String> {
    validate_set_input(input)?;

    let mut object = base_object(
        RT_RADIATION_SET_STORAGE_UID,
        input.sop_instance_uid,
        input.study_instance_uid,
        input.series_instance_uid,
        input.frame_of_reference_uid,
        RT_RADIATION_SET_SERIES_NUMBER,
        "Native RT Radiation Set",
    );
    put_radiotherapy_common(&mut object, RT_RADIATION_SET_USER_CONTENT_LABEL);
    put_u16(&mut object, tags::INTENDED_NUMBER_OF_FRACTIONS, 1);
    put_empty_sequence(&mut object, tags::REFERENCED_RT_PHYSICIAN_INTENT_SEQUENCE);
    put_str(
        &mut object,
        tags::RT_RADIATION_SET_INTENT,
        VR::CS,
        "TREATMENT",
    );

    let radiation_reference = referenced_sop_item(
        input.radiation_sop_class_uid,
        input.radiation_sop_instance_uid,
    );
    let mut treatment_position_group = InMemDicomObject::new_empty();
    put_str(
        &mut treatment_position_group,
        tags::TREATMENT_POSITION_GROUP_UID,
        VR::UI,
        input.treatment_position_group_uid,
    );
    put_str(
        &mut treatment_position_group,
        tags::TREATMENT_POSITION_GROUP_LABEL,
        VR::LO,
        RT_TREATMENT_POSITION_GROUP_LABEL,
    );
    put_sequence(
        &mut treatment_position_group,
        tags::REFERENCED_RT_RADIATION_SEQUENCE,
        vec![radiation_reference.clone()],
    );
    put_sequence(
        &mut object,
        tags::TREATMENT_POSITION_GROUP_SEQUENCE,
        vec![treatment_position_group],
    );
    put_sequence(
        &mut object,
        tags::RT_RADIATION_SEQUENCE,
        vec![radiation_reference],
    );
    put_definition_source(
        &mut object,
        input.plan_sop_class_uid,
        input.plan_sop_instance_uid,
        None,
    );
    put_common_instance_references(
        &mut object,
        &[
            (
                input.plan_series_instance_uid,
                input.plan_sop_class_uid,
                input.plan_sop_instance_uid,
            ),
            (
                input.radiation_series_instance_uid,
                input.radiation_sop_class_uid,
                input.radiation_sop_instance_uid,
            ),
        ],
    );

    Ok(object)
}

fn base_object(
    sop_class_uid: &str,
    sop_instance_uid: &str,
    study_instance_uid: &str,
    series_instance_uid: &str,
    frame_of_reference_uid: &str,
    series_number: &str,
    model_name: &str,
) -> InMemDicomObject {
    let mut object = InMemDicomObject::new_empty();
    put_str(&mut object, tags::SOP_CLASS_UID, VR::UI, sop_class_uid);
    put_str(
        &mut object,
        tags::SOP_INSTANCE_UID,
        VR::UI,
        sop_instance_uid,
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
        study_instance_uid,
    );
    put_str(&mut object, tags::STUDY_DATE, VR::DA, "20260101");
    put_str(&mut object, tags::STUDY_TIME, VR::TM, "000000");
    put_str(&mut object, tags::REFERRING_PHYSICIAN_NAME, VR::PN, "");
    put_str(&mut object, tags::STUDY_ID, VR::SH, "DTS-RTSTRUCT");
    put_str(&mut object, tags::ACCESSION_NUMBER, VR::SH, "");

    put_str(&mut object, tags::MODALITY, VR::CS, "RTRAD");
    put_str(
        &mut object,
        tags::SERIES_INSTANCE_UID,
        VR::UI,
        series_instance_uid,
    );
    put_str(&mut object, tags::SERIES_NUMBER, VR::IS, series_number);
    put_str(&mut object, tags::SERIES_DATE, VR::DA, "20260101");
    put_str(&mut object, tags::SERIES_TIME, VR::TM, "000000");

    put_str(
        &mut object,
        tags::FRAME_OF_REFERENCE_UID,
        VR::UI,
        frame_of_reference_uid,
    );
    put_str(&mut object, tags::POSITION_REFERENCE_INDICATOR, VR::LO, "");

    put_str(&mut object, tags::MANUFACTURER, VR::LO, "dicom-test-suite");
    put_str(
        &mut object,
        tags::MANUFACTURER_MODEL_NAME,
        VR::LO,
        model_name,
    );
    put_str(
        &mut object,
        tags::DEVICE_SERIAL_NUMBER,
        VR::LO,
        RT_DEVICE_SERIAL_NUMBER,
    );
    put_str(
        &mut object,
        tags::SOFTWARE_VERSIONS,
        VR::LO,
        crate::PACKAGE_VERSION,
    );
    object
}

fn put_radiotherapy_common(object: &mut InMemDicomObject, label: &str) {
    put_str(object, tags::INSTANCE_CREATION_DATE, VR::DA, "20260101");
    put_str(object, tags::INSTANCE_CREATION_TIME, VR::TM, "000000");
    put_str(object, tags::CONTENT_DATE, VR::DA, "20260101");
    put_str(object, tags::CONTENT_TIME, VR::TM, "000000");
    put_empty_sequence(object, tags::AUTHOR_IDENTIFICATION_SEQUENCE);
    put_str(object, tags::USER_CONTENT_LABEL, VR::SH, label);
    put_str(object, tags::CONTENT_DESCRIPTION, VR::LO, "");
}

fn put_treatment_device(object: &mut InMemDicomObject) {
    let mut device = InMemDicomObject::new_empty();
    put_str(&mut device, tags::MANUFACTURER, VR::LO, "dicom-test-suite");
    put_str(
        &mut device,
        tags::MANUFACTURER_MODEL_NAME,
        VR::LO,
        "DTS C-Arm LINAC",
    );
    put_str(&mut device, tags::MANUFACTURER_MODEL_VERSION, VR::LO, "1");
    put_str(&mut device, tags::MANUFACTURER_DEVICE_CLASS_UID, VR::UI, "");
    put_str(
        &mut device,
        tags::DEVICE_SERIAL_NUMBER,
        VR::LO,
        RT_DEVICE_SERIAL_NUMBER,
    );
    put_str(
        &mut device,
        tags::SOFTWARE_VERSIONS,
        VR::LO,
        crate::PACKAGE_VERSION,
    );
    put_str(&mut device, tags::DEVICE_ALTERNATE_IDENTIFIER, VR::UC, "");
    put_str(&mut device, tags::DEVICE_LABEL, VR::LO, RT_DEVICE_LABEL);
    put_code_sequence(
        &mut device,
        tags::DEVICE_TYPE_CODE_SEQUENCE,
        "130361",
        "DCM",
        "Radiotherapy Treatment Device",
    );
    put_str(
        &mut device,
        tags::MANUFACTURER_DEVICE_IDENTIFIER,
        VR::ST,
        RT_DEVICE_SERIAL_NUMBER,
    );
    put_sequence(
        object,
        tags::TREATMENT_DEVICE_IDENTIFICATION_SEQUENCE,
        vec![device],
    );
}

fn put_treatment_position(object: &mut InMemDicomObject) {
    let mut position = InMemDicomObject::new_empty();
    put_u16(&mut position, tags::TREATMENT_POSITION_INDEX, 1);
    put_code_sequence(
        &mut position,
        tags::PATIENT_ORIENTATION_CODE_SEQUENCE,
        "102538003",
        "SCT",
        "recumbent",
    );
    put_code_sequence(
        &mut position,
        tags::PATIENT_ORIENTATION_MODIFIER_CODE_SEQUENCE,
        "40199007",
        "SCT",
        "supine",
    );
    put_code_sequence(
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
        "1\\0\\0\\0\\0\\1\\0\\0\\0\\0\\1\\0\\0\\0\\0\\1",
    );
    put_empty_sequence(&mut position, tags::PATIENT_LOCATION_COORDINATES_SEQUENCE);
    put_empty_sequence(&mut position, tags::PATIENT_SUPPORT_POSITION_SEQUENCE);
    put_sequence(object, tags::TREATMENT_POSITION_SEQUENCE, vec![position]);
}

fn put_control_points(object: &mut InMemDicomObject) {
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

    let mut final_point = InMemDicomObject::new_empty();
    put_u16(&mut final_point, tags::RT_CONTROL_POINT_INDEX, 2);
    put_f64(&mut final_point, tags::CUMULATIVE_METERSET, 100.0);

    put_u16(object, tags::NUMBER_OF_RT_CONTROL_POINTS, 2);
    put_sequence(
        object,
        tags::C_ARM_PHOTON_ELECTRON_CONTROL_POINT_SEQUENCE,
        vec![first, final_point],
    );
}

fn put_definition_source(
    object: &mut InMemDicomObject,
    sop_class_uid: &str,
    sop_instance_uid: &str,
    beam_number: Option<&str>,
) {
    let mut reference = referenced_sop_item(sop_class_uid, sop_instance_uid);
    if let Some(beam_number) = beam_number {
        put_str(
            &mut reference,
            tags::REFERENCED_BEAM_NUMBER,
            VR::IS,
            beam_number,
        );
    }
    put_sequence(object, tags::DEFINITION_SOURCE_SEQUENCE, vec![reference]);
}

fn put_common_instance_references(
    object: &mut InMemDicomObject,
    references: &[(&str, &str, &str)],
) {
    let series = references
        .iter()
        .map(|(series_uid, sop_class_uid, sop_instance_uid)| {
            let mut item = InMemDicomObject::new_empty();
            put_str(&mut item, tags::SERIES_INSTANCE_UID, VR::UI, series_uid);
            put_sequence(
                &mut item,
                tags::REFERENCED_INSTANCE_SEQUENCE,
                vec![referenced_sop_item(sop_class_uid, sop_instance_uid)],
            );
            item
        })
        .collect();
    put_sequence(object, tags::REFERENCED_SERIES_SEQUENCE, series);
}

fn referenced_sop_item(sop_class_uid: &str, sop_instance_uid: &str) -> InMemDicomObject {
    InMemDicomObject::from_element_iter([
        DataElement::new(tags::REFERENCED_SOP_CLASS_UID, VR::UI, sop_class_uid),
        DataElement::new(tags::REFERENCED_SOP_INSTANCE_UID, VR::UI, sop_instance_uid),
    ])
}

fn code_item(value: &str, scheme: &str, meaning: &str) -> InMemDicomObject {
    InMemDicomObject::from_element_iter([
        DataElement::new(tags::CODE_VALUE, VR::SH, value),
        DataElement::new(tags::CODING_SCHEME_DESIGNATOR, VR::SH, scheme),
        DataElement::new(tags::CODE_MEANING, VR::LO, meaning),
    ])
}

fn put_code_sequence(
    object: &mut InMemDicomObject,
    tag: dicom_core::Tag,
    value: &str,
    scheme: &str,
    meaning: &str,
) {
    put_sequence(object, tag, vec![code_item(value, scheme, meaning)]);
}

fn put_sequence(object: &mut InMemDicomObject, tag: dicom_core::Tag, items: Vec<InMemDicomObject>) {
    object.put(DataElement::new(tag, VR::SQ, DataSetSequence::from(items)));
}

fn put_empty_sequence(object: &mut InMemDicomObject, tag: dicom_core::Tag) {
    put_sequence(object, tag, Vec::new());
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

fn validate_radiation_input(input: RtRadiationInput<'_>) -> Result<(), String> {
    if input.plan_sop_class_uid != RT_PLAN_STORAGE_UID {
        return Err(format!(
            "plan SOP Class UID must be {RT_PLAN_STORAGE_UID}, got {}",
            input.plan_sop_class_uid
        ));
    }
    validate_distinct_uids(&[
        ("Study Instance UID", input.study_instance_uid),
        ("Frame of Reference UID", input.frame_of_reference_uid),
        ("Radiation Series Instance UID", input.series_instance_uid),
        ("Radiation SOP Instance UID", input.sop_instance_uid),
        ("Plan Series Instance UID", input.plan_series_instance_uid),
        ("Plan SOP Instance UID", input.plan_sop_instance_uid),
    ])?;
    Ok(())
}

fn validate_set_input(input: RtRadiationSetInput<'_>) -> Result<(), String> {
    if input.plan_sop_class_uid != RT_PLAN_STORAGE_UID {
        return Err(format!(
            "plan SOP Class UID must be {RT_PLAN_STORAGE_UID}, got {}",
            input.plan_sop_class_uid
        ));
    }
    if input.radiation_sop_class_uid != C_ARM_PHOTON_ELECTRON_RADIATION_STORAGE_UID {
        return Err(format!(
            "Radiation SOP Class UID must be {C_ARM_PHOTON_ELECTRON_RADIATION_STORAGE_UID}, got {}",
            input.radiation_sop_class_uid
        ));
    }
    validate_distinct_uids(&[
        ("Study Instance UID", input.study_instance_uid),
        ("Frame of Reference UID", input.frame_of_reference_uid),
        ("Set Series Instance UID", input.series_instance_uid),
        ("Set SOP Instance UID", input.sop_instance_uid),
        ("Plan Series Instance UID", input.plan_series_instance_uid),
        ("Plan SOP Instance UID", input.plan_sop_instance_uid),
        (
            "Radiation Series Instance UID",
            input.radiation_series_instance_uid,
        ),
        (
            "Radiation SOP Instance UID",
            input.radiation_sop_instance_uid,
        ),
        (
            "Treatment Position Group UID",
            input.treatment_position_group_uid,
        ),
    ])?;
    Ok(())
}

fn validate_distinct_uids(values: &[(&str, &str)]) -> Result<(), String> {
    for (name, value) in values {
        validate_uid(name, value)?;
    }
    for left in 0..values.len() {
        for right in left + 1..values.len() {
            if values[left].1 == values[right].1 {
                return Err(format!(
                    "{} and {} must be distinct",
                    values[left].0, values[right].0
                ));
            }
        }
    }
    Ok(())
}

fn validate_uid(label: &str, value: &str) -> Result<(), String> {
    if value.is_empty() {
        return Err(format!("{label} must not be empty"));
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
        return Err(format!("{label} must be a valid DICOM UID"));
    }
    Ok(())
}
