use dicom_core::{DataElement, PrimitiveValue, VR, value::DataSetSequence};
use dicom_dictionary_std::tags;
use dicom_object::InMemDicomObject;

pub(in crate::generator) const SPATIAL_REGISTRATION_STORAGE_UID: &str =
    "1.2.840.10008.5.1.4.1.1.66.1";
pub(in crate::generator) const SPATIAL_REGISTRATION_OUTPUT_FILE: &str = "instance.dcm";
pub(in crate::generator) const TARGET_IDENTITY_MATRIX: [&str; 16] = [
    "1", "0", "0", "0", "0", "1", "0", "0", "0", "0", "1", "0", "0", "0", "0", "1",
];
pub(in crate::generator) const SOURCE_TO_TARGET_MATRIX: [&str; 16] = [
    "1", "0", "0", "0", "0", "1", "0", "0", "0", "0", "1", "2.5", "0", "0", "0", "1",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::generator) struct SpatialRegistrationReference<'a> {
    pub(in crate::generator) study_instance_uid: &'a str,
    pub(in crate::generator) series_instance_uid: &'a str,
    pub(in crate::generator) sop_class_uid: &'a str,
    pub(in crate::generator) sop_instance_uid: &'a str,
    pub(in crate::generator) frame_of_reference_uid: &'a str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::generator) struct SpatialRegistrationInput<'a> {
    pub(in crate::generator) sop_instance_uid: &'a str,
    pub(in crate::generator) series_instance_uid: &'a str,
    pub(in crate::generator) target: SpatialRegistrationReference<'a>,
    pub(in crate::generator) source: SpatialRegistrationReference<'a>,
}

pub(in crate::generator) fn build_spatial_registration(
    input: SpatialRegistrationInput<'_>,
) -> Result<InMemDicomObject, String> {
    validate_input(input)?;

    let mut object = InMemDicomObject::new_empty();
    put_str(
        &mut object,
        tags::SOP_CLASS_UID,
        VR::UI,
        SPATIAL_REGISTRATION_STORAGE_UID,
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
        input.target.study_instance_uid,
    );
    put_str(&mut object, tags::STUDY_DATE, VR::DA, "20260101");
    put_str(&mut object, tags::STUDY_TIME, VR::TM, "000000");
    put_str(&mut object, tags::REFERRING_PHYSICIAN_NAME, VR::PN, "");
    // The REG instance belongs to the target Enhanced CT study, so its
    // study-level identity must remain byte-for-byte consistent with that source.
    put_str(&mut object, tags::STUDY_ID, VR::SH, "DTS-ECT");
    put_str(&mut object, tags::ACCESSION_NUMBER, VR::SH, "");

    put_str(&mut object, tags::MODALITY, VR::CS, "REG");
    put_str(
        &mut object,
        tags::SERIES_INSTANCE_UID,
        VR::UI,
        input.series_instance_uid,
    );
    put_str(&mut object, tags::SERIES_NUMBER, VR::IS, "8003");
    put_str(&mut object, tags::LATERALITY, VR::CS, "R");

    put_str(
        &mut object,
        tags::FRAME_OF_REFERENCE_UID,
        VR::UI,
        input.target.frame_of_reference_uid,
    );
    put_str(&mut object, tags::POSITION_REFERENCE_INDICATOR, VR::LO, "");

    put_str(&mut object, tags::MANUFACTURER, VR::LO, "dicom-test-suite");
    put_str(&mut object, tags::INSTITUTION_NAME, VR::LO, "");
    put_str(&mut object, tags::INSTITUTION_ADDRESS, VR::ST, "");
    put_str(
        &mut object,
        tags::MANUFACTURER_MODEL_NAME,
        VR::LO,
        "Native Spatial Registration",
    );
    put_str(
        &mut object,
        tags::DEVICE_SERIAL_NUMBER,
        VR::LO,
        "DTS-REG-001",
    );
    put_str(
        &mut object,
        tags::SOFTWARE_VERSIONS,
        VR::LO,
        crate::PACKAGE_VERSION,
    );

    put_str(&mut object, tags::INSTANCE_NUMBER, VR::IS, "1");
    put_str(&mut object, tags::CONTENT_DATE, VR::DA, "20260101");
    put_str(&mut object, tags::CONTENT_TIME, VR::TM, "000000");
    put_str(&mut object, tags::CONTENT_LABEL, VR::CS, "DTS_RIGID_REG");
    put_str(
        &mut object,
        tags::CONTENT_DESCRIPTION,
        VR::LO,
        "Rigid CT pair registration",
    );
    put_str(
        &mut object,
        tags::CONTENT_CREATOR_NAME,
        VR::PN,
        "DTS^Generator",
    );

    object.put(DataElement::new(
        tags::REGISTRATION_SEQUENCE,
        VR::SQ,
        DataSetSequence::from(vec![
            registration_item(
                input.target,
                &TARGET_IDENTITY_MATRIX,
                "Enhanced CT target identity",
            ),
            registration_item(
                input.source,
                &SOURCE_TO_TARGET_MATRIX,
                "Classic CT translated +2.5 mm along target z",
            ),
        ]),
    ));

    object.put(DataElement::new(
        tags::REFERENCED_SERIES_SEQUENCE,
        VR::SQ,
        DataSetSequence::from(vec![referenced_series_item(input.target)]),
    ));
    object.put(DataElement::new(
        tags::STUDIES_CONTAINING_OTHER_REFERENCED_INSTANCES_SEQUENCE,
        VR::SQ,
        DataSetSequence::from(vec![InMemDicomObject::from_element_iter([
            DataElement::new(
                tags::STUDY_INSTANCE_UID,
                VR::UI,
                input.source.study_instance_uid,
            ),
            DataElement::new(
                tags::REFERENCED_SERIES_SEQUENCE,
                VR::SQ,
                DataSetSequence::from(vec![referenced_series_item(input.source)]),
            ),
        ])]),
    ));

    Ok(object)
}

fn validate_input(input: SpatialRegistrationInput<'_>) -> Result<(), String> {
    for (name, value) in [
        ("SOP Instance UID", input.sop_instance_uid),
        ("Series Instance UID", input.series_instance_uid),
        ("target Study Instance UID", input.target.study_instance_uid),
        (
            "target Series Instance UID",
            input.target.series_instance_uid,
        ),
        ("target SOP Class UID", input.target.sop_class_uid),
        ("target SOP Instance UID", input.target.sop_instance_uid),
        (
            "target Frame of Reference UID",
            input.target.frame_of_reference_uid,
        ),
        ("source Study Instance UID", input.source.study_instance_uid),
        (
            "source Series Instance UID",
            input.source.series_instance_uid,
        ),
        ("source SOP Class UID", input.source.sop_class_uid),
        ("source SOP Instance UID", input.source.sop_instance_uid),
        (
            "source Frame of Reference UID",
            input.source.frame_of_reference_uid,
        ),
    ] {
        if value.is_empty() {
            return Err(format!("{name} must not be empty"));
        }
    }
    if input.target.study_instance_uid == input.source.study_instance_uid {
        return Err("the locked CT pair must be in two distinct studies".to_string());
    }
    if input.target.frame_of_reference_uid == input.source.frame_of_reference_uid {
        return Err("the locked CT pair must have distinct Frames of Reference".to_string());
    }
    if input.target.sop_instance_uid == input.source.sop_instance_uid {
        return Err("target and source SOP Instance UIDs must be distinct".to_string());
    }
    Ok(())
}

fn registration_item(
    reference: SpatialRegistrationReference<'_>,
    matrix: &[&str; 16],
    comment: &str,
) -> InMemDicomObject {
    InMemDicomObject::from_element_iter([
        DataElement::new(
            tags::REFERENCED_IMAGE_SEQUENCE,
            VR::SQ,
            DataSetSequence::from(vec![referenced_sop_item(reference)]),
        ),
        DataElement::new(
            tags::FRAME_OF_REFERENCE_UID,
            VR::UI,
            reference.frame_of_reference_uid,
        ),
        DataElement::new(
            tags::MATRIX_REGISTRATION_SEQUENCE,
            VR::SQ,
            DataSetSequence::from(vec![InMemDicomObject::from_element_iter([
                DataElement::new(
                    tags::REGISTRATION_TYPE_CODE_SEQUENCE,
                    VR::SQ,
                    DataSetSequence::empty(),
                ),
                DataElement::new(
                    tags::MATRIX_SEQUENCE,
                    VR::SQ,
                    DataSetSequence::from(vec![InMemDicomObject::from_element_iter([
                        DataElement::new(
                            tags::FRAME_OF_REFERENCE_TRANSFORMATION_MATRIX_TYPE,
                            VR::CS,
                            "RIGID",
                        ),
                        DataElement::new(
                            tags::FRAME_OF_REFERENCE_TRANSFORMATION_MATRIX,
                            VR::DS,
                            PrimitiveValue::Strs(
                                matrix
                                    .iter()
                                    .map(|value| (*value).to_string())
                                    .collect::<Vec<_>>()
                                    .into(),
                            ),
                        ),
                    ])]),
                ),
                DataElement::new(
                    tags::FRAME_OF_REFERENCE_TRANSFORMATION_COMMENT,
                    VR::LO,
                    comment,
                ),
            ])]),
        ),
    ])
}

fn referenced_series_item(reference: SpatialRegistrationReference<'_>) -> InMemDicomObject {
    InMemDicomObject::from_element_iter([
        DataElement::new(
            tags::REFERENCED_INSTANCE_SEQUENCE,
            VR::SQ,
            DataSetSequence::from(vec![referenced_sop_item(reference)]),
        ),
        DataElement::new(
            tags::SERIES_INSTANCE_UID,
            VR::UI,
            reference.series_instance_uid,
        ),
    ])
}

fn referenced_sop_item(reference: SpatialRegistrationReference<'_>) -> InMemDicomObject {
    InMemDicomObject::from_element_iter([
        DataElement::new(
            tags::REFERENCED_SOP_CLASS_UID,
            VR::UI,
            reference.sop_class_uid,
        ),
        DataElement::new(
            tags::REFERENCED_SOP_INSTANCE_UID,
            VR::UI,
            reference.sop_instance_uid,
        ),
    ])
}

fn put_str(object: &mut InMemDicomObject, tag: dicom_core::Tag, vr: VR, value: &str) {
    object.put(DataElement::new(tag, vr, value));
}
