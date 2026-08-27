use dicom_core::{DataElement, PrimitiveValue, VR, value::DataSetSequence};
use dicom_dictionary_std::tags;
use dicom_object::InMemDicomObject;

use super::icc_profile::{ICC_COLOR_SPACE, ICC_PROFILE_BYTES};

pub(in crate::generator) const ADVANCED_BLENDING_PRESENTATION_STATE_STORAGE_UID: &str =
    "1.2.840.10008.5.1.4.1.1.11.8";
pub(in crate::generator) const ADVANCED_BLENDING_PRESENTATION_STATE_OUTPUT_FILE: &str =
    "instance.dcm";
pub(in crate::generator) const ADVANCED_BLENDING_PRESENTATION_STATE_SERIES_NUMBER: &str = "80";
pub(in crate::generator) const ADVANCED_BLENDING_PRESENTATION_STATE_CONTENT_LABEL: &str =
    "DTSADVBLEND";
pub(in crate::generator) const ADVANCED_BLENDING_PRESENTATION_STATE_CONTENT_DESCRIPTION: &str =
    "Synthetic DTSADVBLEND presentation state";
pub(in crate::generator) const ADVANCED_BLENDING_PRESENTATION_STATE_CREATION_DATE: &str =
    "20260101";
pub(in crate::generator) const ADVANCED_BLENDING_PRESENTATION_STATE_CREATION_TIME: &str = "000000";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::generator) struct AdvancedBlendingPresentationStateReference<'a> {
    pub(in crate::generator) study_instance_uid: &'a str,
    pub(in crate::generator) series_instance_uid: &'a str,
    pub(in crate::generator) sop_class_uid: &'a str,
    pub(in crate::generator) sop_instance_uid: &'a str,
    pub(in crate::generator) frame_of_reference_uid: &'a str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::generator) struct AdvancedBlendingPresentationStateInput<'a> {
    pub(in crate::generator) sop_instance_uid: &'a str,
    pub(in crate::generator) series_instance_uid: &'a str,
    /// Source identities in locked `[series ordinal][slice ordinal]` order.
    pub(in crate::generator) sources: [[AdvancedBlendingPresentationStateReference<'a>; 2]; 2],
}

pub(in crate::generator) fn build_advanced_blending_presentation_state(
    input: AdvancedBlendingPresentationStateInput<'_>,
) -> Result<InMemDicomObject, String> {
    validate_input(input)?;
    let source = input.sources[0][0];

    let mut object = InMemDicomObject::new_empty();
    put_str(
        &mut object,
        tags::SOP_CLASS_UID,
        VR::UI,
        ADVANCED_BLENDING_PRESENTATION_STATE_STORAGE_UID,
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
        source.study_instance_uid,
    );
    put_str(&mut object, tags::STUDY_DATE, VR::DA, "20260101");
    put_str(&mut object, tags::STUDY_TIME, VR::TM, "000000");
    put_str(&mut object, tags::REFERRING_PHYSICIAN_NAME, VR::PN, "");
    put_str(&mut object, tags::STUDY_ID, VR::SH, "DTS-CT");
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
        ADVANCED_BLENDING_PRESENTATION_STATE_SERIES_NUMBER,
    );
    put_str(&mut object, tags::LATERALITY, VR::CS, "R");

    put_str(
        &mut object,
        tags::FRAME_OF_REFERENCE_UID,
        VR::UI,
        source.frame_of_reference_uid,
    );
    put_str(&mut object, tags::POSITION_REFERENCE_INDICATOR, VR::LO, "");

    put_str(&mut object, tags::MANUFACTURER, VR::LO, "dicom-test-suite");
    put_str(&mut object, tags::INSTITUTION_NAME, VR::LO, "");
    put_str(&mut object, tags::INSTITUTION_ADDRESS, VR::ST, "");
    put_str(
        &mut object,
        tags::MANUFACTURER_MODEL_NAME,
        VR::LO,
        "Native Advanced Blending Presentation State",
    );
    put_str(
        &mut object,
        tags::DEVICE_SERIAL_NUMBER,
        VR::LO,
        "DTS-ADVBLEND-001",
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
        ADVANCED_BLENDING_PRESENTATION_STATE_CREATION_DATE,
    );
    put_str(
        &mut object,
        tags::PRESENTATION_CREATION_TIME,
        VR::TM,
        ADVANCED_BLENDING_PRESENTATION_STATE_CREATION_TIME,
    );
    put_str(
        &mut object,
        tags::CONTENT_LABEL,
        VR::CS,
        ADVANCED_BLENDING_PRESENTATION_STATE_CONTENT_LABEL,
    );
    put_str(
        &mut object,
        tags::CONTENT_DESCRIPTION,
        VR::LO,
        ADVANCED_BLENDING_PRESENTATION_STATE_CONTENT_DESCRIPTION,
    );
    put_str(
        &mut object,
        tags::CONTENT_CREATOR_NAME,
        VR::PN,
        "DTS^Generator",
    );

    object.put(DataElement::new(
        tags::ADVANCED_BLENDING_SEQUENCE,
        VR::SQ,
        DataSetSequence::from(vec![
            advanced_blending_item(1, input.sources[0], true),
            advanced_blending_item(2, input.sources[1], false),
        ]),
    ));
    put_str(&mut object, tags::PIXEL_PRESENTATION, VR::CS, "TRUE_COLOR");
    object.put(DataElement::new(
        tags::BLENDING_DISPLAY_SEQUENCE,
        VR::SQ,
        DataSetSequence::from(vec![blending_display_item()]),
    ));

    object.put(DataElement::new(
        tags::ICC_PROFILE,
        VR::OB,
        PrimitiveValue::U8(ICC_PROFILE_BYTES.to_vec().into()),
    ));
    put_str(&mut object, tags::COLOR_SPACE, VR::CS, ICC_COLOR_SPACE);

    object.put(DataElement::new(
        tags::REFERENCED_SERIES_SEQUENCE,
        VR::SQ,
        DataSetSequence::from(vec![
            common_referenced_series_item(input.sources[0]),
            common_referenced_series_item(input.sources[1]),
        ]),
    ));

    Ok(object)
}

fn validate_input(input: AdvancedBlendingPresentationStateInput<'_>) -> Result<(), String> {
    for (name, value) in [
        ("SOP Instance UID", input.sop_instance_uid),
        ("Series Instance UID", input.series_instance_uid),
    ] {
        if value.is_empty() {
            return Err(format!("{name} must not be empty"));
        }
    }

    let first = input.sources[0][0];
    for (series_index, series) in input.sources.iter().enumerate() {
        for (slice_index, reference) in series.iter().enumerate() {
            for (name, value) in [
                ("Study Instance UID", reference.study_instance_uid),
                ("Series Instance UID", reference.series_instance_uid),
                ("SOP Class UID", reference.sop_class_uid),
                ("SOP Instance UID", reference.sop_instance_uid),
                ("Frame of Reference UID", reference.frame_of_reference_uid),
            ] {
                if value.is_empty() {
                    return Err(format!(
                        "source series {} slice {} {name} must not be empty",
                        series_index + 1,
                        slice_index + 1
                    ));
                }
            }
            if reference.study_instance_uid != first.study_instance_uid {
                return Err("all source images must share one Study Instance UID".to_string());
            }
            if reference.frame_of_reference_uid != first.frame_of_reference_uid {
                return Err("all source images must share one Frame of Reference UID".to_string());
            }
            if reference.series_instance_uid != series[0].series_instance_uid {
                return Err(
                    "source images within each input must share one Series Instance UID"
                        .to_string(),
                );
            }
        }
    }
    if input.sources[0][0].series_instance_uid == input.sources[1][0].series_instance_uid {
        return Err("the two blending inputs must use distinct Series Instance UIDs".to_string());
    }
    if input.series_instance_uid == input.sources[0][0].series_instance_uid
        || input.series_instance_uid == input.sources[1][0].series_instance_uid
    {
        return Err("the presentation state must use a distinct Series Instance UID".to_string());
    }

    let sop_instance_uids = input
        .sources
        .iter()
        .flatten()
        .map(|reference| reference.sop_instance_uid)
        .collect::<Vec<_>>();
    for (index, uid) in sop_instance_uids.iter().enumerate() {
        if sop_instance_uids[..index].contains(uid) {
            return Err("the four source SOP Instance UIDs must be unique".to_string());
        }
        if *uid == input.sop_instance_uid {
            return Err("the presentation state must use a distinct SOP Instance UID".to_string());
        }
    }
    Ok(())
}

fn advanced_blending_item(
    input_number: u16,
    sources: [AdvancedBlendingPresentationStateReference<'_>; 2],
    geometry_for_display: bool,
) -> InMemDicomObject {
    InMemDicomObject::from_element_iter([
        DataElement::new(
            tags::BLENDING_INPUT_NUMBER,
            VR::US,
            PrimitiveValue::U16(vec![input_number].into()),
        ),
        DataElement::new(
            tags::STUDY_INSTANCE_UID,
            VR::UI,
            sources[0].study_instance_uid,
        ),
        DataElement::new(
            tags::SERIES_INSTANCE_UID,
            VR::UI,
            sources[0].series_instance_uid,
        ),
        DataElement::new(
            tags::REFERENCED_IMAGE_SEQUENCE,
            VR::SQ,
            DataSetSequence::from(
                sources
                    .into_iter()
                    .map(referenced_sop_item)
                    .collect::<Vec<_>>(),
            ),
        ),
        DataElement::new(tags::TIME_SERIES_BLENDING, VR::CS, "FALSE"),
        DataElement::new(
            tags::GEOMETRY_FOR_DISPLAY,
            VR::CS,
            if geometry_for_display {
                "TRUE"
            } else {
                "FALSE"
            },
        ),
    ])
}

fn blending_display_item() -> InMemDicomObject {
    InMemDicomObject::from_element_iter([
        DataElement::new(
            tags::BLENDING_DISPLAY_INPUT_SEQUENCE,
            VR::SQ,
            DataSetSequence::from(vec![blending_display_input(1), blending_display_input(2)]),
        ),
        DataElement::new(tags::BLENDING_MODE, VR::CS, "EQUAL"),
    ])
}

fn blending_display_input(input_number: u16) -> InMemDicomObject {
    InMemDicomObject::from_element_iter([DataElement::new(
        tags::BLENDING_INPUT_NUMBER,
        VR::US,
        PrimitiveValue::U16(vec![input_number].into()),
    )])
}

fn common_referenced_series_item(
    sources: [AdvancedBlendingPresentationStateReference<'_>; 2],
) -> InMemDicomObject {
    InMemDicomObject::from_element_iter([
        DataElement::new(
            tags::SERIES_INSTANCE_UID,
            VR::UI,
            sources[0].series_instance_uid,
        ),
        DataElement::new(
            tags::REFERENCED_INSTANCE_SEQUENCE,
            VR::SQ,
            DataSetSequence::from(
                sources
                    .into_iter()
                    .map(referenced_sop_item)
                    .collect::<Vec<_>>(),
            ),
        ),
    ])
}

fn referenced_sop_item(
    reference: AdvancedBlendingPresentationStateReference<'_>,
) -> InMemDicomObject {
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
