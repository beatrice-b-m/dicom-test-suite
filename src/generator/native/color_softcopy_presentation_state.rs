use dicom_core::{DataElement, PrimitiveValue, VR, value::DataSetSequence};
use dicom_dictionary_std::tags;
use dicom_object::InMemDicomObject;

use super::icc_profile::{ICC_COLOR_SPACE, ICC_PROFILE_BYTES};

pub(in crate::generator) const COLOR_SOFTCOPY_PRESENTATION_STATE_STORAGE_UID: &str =
    "1.2.840.10008.5.1.4.1.1.11.2";
pub(in crate::generator) const COLOR_SOFTCOPY_PRESENTATION_STATE_OUTPUT_FILE: &str = "instance.dcm";
pub(in crate::generator) const COLOR_SOFTCOPY_PRESENTATION_STATE_SERIES_NUMBER: &str = "62";
pub(in crate::generator) const COLOR_SOFTCOPY_PRESENTATION_STATE_CONTENT_LABEL: &str = "DTSCOLORPR";
pub(in crate::generator) const COLOR_SOFTCOPY_PRESENTATION_STATE_CONTENT_DESCRIPTION: &str =
    "Synthetic RGB color presentation state";
pub(in crate::generator) const COLOR_SOFTCOPY_PRESENTATION_STATE_CREATION_DATE: &str = "20260101";
pub(in crate::generator) const COLOR_SOFTCOPY_PRESENTATION_STATE_CREATION_TIME: &str = "000000";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::generator) struct ColorSoftcopyPresentationStateReference<'a> {
    pub(in crate::generator) study_instance_uid: &'a str,
    pub(in crate::generator) series_instance_uid: &'a str,
    pub(in crate::generator) sop_class_uid: &'a str,
    pub(in crate::generator) sop_instance_uid: &'a str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::generator) struct ColorSoftcopyPresentationStateInput<'a> {
    pub(in crate::generator) sop_instance_uid: &'a str,
    pub(in crate::generator) series_instance_uid: &'a str,
    pub(in crate::generator) source: ColorSoftcopyPresentationStateReference<'a>,
}

pub(in crate::generator) fn build_color_softcopy_presentation_state(
    input: ColorSoftcopyPresentationStateInput<'_>,
) -> Result<InMemDicomObject, String> {
    validate_input(input)?;

    let mut object = InMemDicomObject::new_empty();
    put_str(
        &mut object,
        tags::SOP_CLASS_UID,
        VR::UI,
        COLOR_SOFTCOPY_PRESENTATION_STATE_STORAGE_UID,
    );
    put_str(
        &mut object,
        tags::SOP_INSTANCE_UID,
        VR::UI,
        input.sop_instance_uid,
    );
    put_str(&mut object, tags::SYNTHETIC_DATA, VR::CS, "YES");

    put_str(&mut object, tags::PATIENT_NAME, VR::PN, "DICOMTEST^SMOKE");
    put_str(&mut object, tags::PATIENT_ID, VR::LO, "DICOMTEST-SMOKE-001");
    put_str(&mut object, tags::PATIENT_BIRTH_DATE, VR::DA, "19700101");
    put_str(&mut object, tags::PATIENT_SEX, VR::CS, "O");

    put_str(
        &mut object,
        tags::STUDY_INSTANCE_UID,
        VR::UI,
        input.source.study_instance_uid,
    );
    put_str(&mut object, tags::STUDY_DATE, VR::DA, "20260101");
    put_str(&mut object, tags::STUDY_TIME, VR::TM, "000000");
    put_str(&mut object, tags::REFERRING_PHYSICIAN_NAME, VR::PN, "");
    // The PR instance belongs to the source RGB object's study, whose locked
    // study-level identity uses this value.
    put_str(&mut object, tags::STUDY_ID, VR::SH, "SMOKE");
    put_str(&mut object, tags::ACCESSION_NUMBER, VR::SH, "");

    put_str(&mut object, tags::MODALITY, VR::CS, "PR");
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
        COLOR_SOFTCOPY_PRESENTATION_STATE_SERIES_NUMBER,
    );
    put_str(&mut object, tags::BODY_PART_EXAMINED, VR::CS, "HAND");
    put_str(&mut object, tags::LATERALITY, VR::CS, "R");

    put_str(&mut object, tags::MANUFACTURER, VR::LO, "dicom-test-suite");
    put_str(
        &mut object,
        tags::MANUFACTURER_MODEL_NAME,
        VR::LO,
        "Native Color Softcopy Presentation State",
    );
    put_str(
        &mut object,
        tags::DEVICE_SERIAL_NUMBER,
        VR::LO,
        "DTS-COLOR-PR-0001",
    );
    put_str(
        &mut object,
        tags::SOFTWARE_VERSIONS,
        VR::LO,
        crate::PACKAGE_VERSION,
    );

    put_str(&mut object, tags::INSTANCE_NUMBER, VR::IS, "1");
    put_str(
        &mut object,
        tags::PRESENTATION_CREATION_DATE,
        VR::DA,
        COLOR_SOFTCOPY_PRESENTATION_STATE_CREATION_DATE,
    );
    put_str(
        &mut object,
        tags::PRESENTATION_CREATION_TIME,
        VR::TM,
        COLOR_SOFTCOPY_PRESENTATION_STATE_CREATION_TIME,
    );
    put_str(
        &mut object,
        tags::CONTENT_LABEL,
        VR::CS,
        COLOR_SOFTCOPY_PRESENTATION_STATE_CONTENT_LABEL,
    );
    put_str(
        &mut object,
        tags::CONTENT_DESCRIPTION,
        VR::LO,
        COLOR_SOFTCOPY_PRESENTATION_STATE_CONTENT_DESCRIPTION,
    );
    put_str(
        &mut object,
        tags::CONTENT_CREATOR_NAME,
        VR::PN,
        "DTS^Generator",
    );

    object.put(DataElement::new(
        tags::REFERENCED_SERIES_SEQUENCE,
        VR::SQ,
        DataSetSequence::from(vec![InMemDicomObject::from_element_iter([
            DataElement::new(
                tags::SERIES_INSTANCE_UID,
                VR::UI,
                input.source.series_instance_uid,
            ),
            DataElement::new(
                tags::REFERENCED_IMAGE_SEQUENCE,
                VR::SQ,
                DataSetSequence::from(vec![referenced_sop_item(input.source)]),
            ),
        ])]),
    ));

    object.put(DataElement::new(
        tags::DISPLAYED_AREA_SELECTION_SEQUENCE,
        VR::SQ,
        DataSetSequence::from(vec![InMemDicomObject::from_element_iter([
            DataElement::new(
                tags::DISPLAYED_AREA_TOP_LEFT_HAND_CORNER,
                VR::SL,
                PrimitiveValue::from([1_i32, 1_i32]),
            ),
            DataElement::new(
                tags::DISPLAYED_AREA_BOTTOM_RIGHT_HAND_CORNER,
                VR::SL,
                PrimitiveValue::from([2_i32, 2_i32]),
            ),
            DataElement::new(tags::PRESENTATION_SIZE_MODE, VR::CS, "SCALE TO FIT"),
            DataElement::new(
                tags::PRESENTATION_PIXEL_ASPECT_RATIO,
                VR::IS,
                PrimitiveValue::Strs(vec!["1".to_string(), "1".to_string()].into()),
            ),
        ])]),
    ));

    object.put(DataElement::new(
        tags::ICC_PROFILE,
        VR::OB,
        PrimitiveValue::U8(ICC_PROFILE_BYTES.to_vec().into()),
    ));
    put_str(&mut object, tags::COLOR_SPACE, VR::CS, ICC_COLOR_SPACE);

    Ok(object)
}

fn validate_input(input: ColorSoftcopyPresentationStateInput<'_>) -> Result<(), String> {
    for (name, value) in [
        ("SOP Instance UID", input.sop_instance_uid),
        ("Series Instance UID", input.series_instance_uid),
        ("source Study Instance UID", input.source.study_instance_uid),
        (
            "source Series Instance UID",
            input.source.series_instance_uid,
        ),
        ("source SOP Class UID", input.source.sop_class_uid),
        ("source SOP Instance UID", input.source.sop_instance_uid),
    ] {
        if value.is_empty() {
            return Err(format!("{name} must not be empty"));
        }
    }
    if input.series_instance_uid == input.source.series_instance_uid {
        return Err(
            "the presentation state and source must use distinct Series Instance UIDs".to_string(),
        );
    }
    if input.sop_instance_uid == input.source.sop_instance_uid {
        return Err(
            "the presentation state and source must use distinct SOP Instance UIDs".to_string(),
        );
    }
    Ok(())
}

fn referenced_sop_item(reference: ColorSoftcopyPresentationStateReference<'_>) -> InMemDicomObject {
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
