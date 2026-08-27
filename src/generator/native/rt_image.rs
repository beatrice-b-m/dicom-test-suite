use dicom_core::{DataElement, PrimitiveValue, VR, value::DataSetSequence};
use dicom_dictionary_std::tags;
use dicom_object::InMemDicomObject;

pub(in crate::generator) const RT_IMAGE_STORAGE_UID: &str = "1.2.840.10008.5.1.4.1.1.481.1";
pub(in crate::generator) const RT_PLAN_STORAGE_UID: &str = "1.2.840.10008.5.1.4.1.1.481.5";
pub(in crate::generator) const RT_IMAGE_OUTPUT_FILE: &str = "instance.dcm";
pub(in crate::generator) const RT_IMAGE_SERIES_NUMBER: &str = "73";
pub(in crate::generator) const RT_IMAGE_LABEL: &str = "DTS_DRR";
pub(in crate::generator) const RT_IMAGE_PIXEL_SHA256: &str =
    "a8faed6abbf35c12a4b26e40f6feb19d736d90045c83b9f9a31f638d323e6811";
pub(in crate::generator) const RT_IMAGE_PIXEL_BYTES: [u8; 16] = rt_image_pixels();

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::generator) struct RtImageInput<'a> {
    pub(in crate::generator) study_instance_uid: &'a str,
    pub(in crate::generator) frame_of_reference_uid: &'a str,
    pub(in crate::generator) series_instance_uid: &'a str,
    pub(in crate::generator) sop_instance_uid: &'a str,
    pub(in crate::generator) plan_sop_class_uid: &'a str,
    pub(in crate::generator) plan_sop_instance_uid: &'a str,
}

pub(in crate::generator) fn build_rt_image(
    input: RtImageInput<'_>,
) -> Result<InMemDicomObject, String> {
    validate_input(input)?;

    let mut object = InMemDicomObject::new_empty();
    put_str(
        &mut object,
        tags::SOP_CLASS_UID,
        VR::UI,
        RT_IMAGE_STORAGE_UID,
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
    put_str(&mut object, tags::STUDY_ID, VR::SH, "DTS-RT");
    put_str(&mut object, tags::ACCESSION_NUMBER, VR::SH, "");

    put_str(&mut object, tags::MODALITY, VR::CS, "RTIMAGE");
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
        RT_IMAGE_SERIES_NUMBER,
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
        "Native Linked RT Image",
    );
    put_str(
        &mut object,
        tags::DEVICE_SERIAL_NUMBER,
        VR::LO,
        "DTS-RTIMAGE-001",
    );
    put_str(
        &mut object,
        tags::SOFTWARE_VERSIONS,
        VR::LO,
        crate::PACKAGE_VERSION,
    );

    put_str(&mut object, tags::ACQUISITION_DATE, VR::DA, "20260101");
    put_str(&mut object, tags::ACQUISITION_TIME, VR::TM, "000000");
    put_str(
        &mut object,
        tags::IMAGE_TYPE,
        VR::CS,
        "DERIVED\\SECONDARY\\DRR",
    );
    put_str(&mut object, tags::CONVERSION_TYPE, VR::CS, "WSD");
    put_str(&mut object, tags::INSTANCE_NUMBER, VR::IS, "1");
    put_str(&mut object, tags::CONTENT_DATE, VR::DA, "20260101");
    put_str(&mut object, tags::CONTENT_TIME, VR::TM, "000000");

    put_u16(&mut object, tags::SAMPLES_PER_PIXEL, 1);
    put_str(
        &mut object,
        tags::PHOTOMETRIC_INTERPRETATION,
        VR::CS,
        "MONOCHROME2",
    );
    put_u16(&mut object, tags::ROWS, 4);
    put_u16(&mut object, tags::COLUMNS, 4);
    put_u16(&mut object, tags::BITS_ALLOCATED, 8);
    put_u16(&mut object, tags::BITS_STORED, 8);
    put_u16(&mut object, tags::HIGH_BIT, 7);
    put_u16(&mut object, tags::PIXEL_REPRESENTATION, 0);

    put_str(&mut object, tags::RT_IMAGE_LABEL, VR::SH, RT_IMAGE_LABEL);
    put_str(&mut object, tags::RT_IMAGE_PLANE, VR::CS, "NORMAL");
    put_str(&mut object, tags::X_RAY_IMAGE_RECEPTOR_ANGLE, VR::DS, "0");
    put_str(&mut object, tags::IMAGE_PLANE_PIXEL_SPACING, VR::DS, "1\\1");
    put_str(&mut object, tags::RT_IMAGE_POSITION, VR::DS, "-1.5\\1.5");
    put_str(
        &mut object,
        tags::RADIATION_MACHINE_NAME,
        VR::SH,
        "DTS_LINAC",
    );
    put_str(&mut object, tags::RADIATION_MACHINE_SAD, VR::DS, "1000");
    put_str(&mut object, tags::RT_IMAGE_SID, VR::DS, "1500");
    put_str(&mut object, tags::PRIMARY_DOSIMETER_UNIT, VR::CS, "MU");
    put_str(&mut object, tags::FRACTION_NUMBER, VR::IS, "1");
    put_plan_reference(&mut object, input);

    object.put(DataElement::new(
        tags::PIXEL_DATA,
        VR::OB,
        PrimitiveValue::from(RT_IMAGE_PIXEL_BYTES.as_slice()),
    ));
    Ok(object)
}

const fn rt_image_pixels() -> [u8; 16] {
    let mut pixels = [0_u8; 16];
    let mut row = 0;
    while row < 4 {
        let mut column = 0;
        while column < 4 {
            pixels[4 * row + column] = (17 * (4 * row + column)) as u8;
            column += 1;
        }
        row += 1;
    }
    pixels
}

fn put_plan_reference(object: &mut InMemDicomObject, input: RtImageInput<'_>) {
    let mut item = InMemDicomObject::new_empty();
    put_str(&mut item, tags::REFERENCED_BEAM_NUMBER, VR::IS, "1");
    put_str(
        &mut item,
        tags::REFERENCED_FRACTION_GROUP_NUMBER,
        VR::IS,
        "1",
    );
    put_str(
        &mut item,
        tags::REFERENCED_SOP_CLASS_UID,
        VR::UI,
        input.plan_sop_class_uid,
    );
    put_str(
        &mut item,
        tags::REFERENCED_SOP_INSTANCE_UID,
        VR::UI,
        input.plan_sop_instance_uid,
    );
    object.put(DataElement::new(
        tags::REFERENCED_RT_PLAN_SEQUENCE,
        VR::SQ,
        DataSetSequence::from(vec![item]),
    ));
}

fn validate_input(input: RtImageInput<'_>) -> Result<(), String> {
    if input.plan_sop_class_uid != RT_PLAN_STORAGE_UID {
        return Err(format!("Plan SOP Class UID must be {RT_PLAN_STORAGE_UID}"));
    }
    let uids = [
        ("Study Instance UID", input.study_instance_uid),
        ("Frame of Reference UID", input.frame_of_reference_uid),
        ("Series Instance UID", input.series_instance_uid),
        ("SOP Instance UID", input.sop_instance_uid),
        ("Plan SOP Instance UID", input.plan_sop_instance_uid),
    ];
    for (name, uid) in uids {
        validate_uid(name, uid)?;
    }
    for left in 0..uids.len() {
        for right in left + 1..uids.len() {
            if uids[left].1 == uids[right].1 {
                return Err(format!(
                    "{} and {} must be distinct",
                    uids[left].0, uids[right].0
                ));
            }
        }
    }
    Ok(())
}

fn validate_uid(name: &str, value: &str) -> Result<(), String> {
    if value.is_empty()
        || value.len() > 64
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

fn put_u16(object: &mut InMemDicomObject, tag: dicom_core::Tag, value: u16) {
    object.put(DataElement::new(tag, VR::US, PrimitiveValue::from(value)));
}
