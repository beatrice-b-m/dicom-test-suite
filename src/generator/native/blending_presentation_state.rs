use dicom_core::{DataElement, PrimitiveValue, VR, value::DataSetSequence};
use dicom_dictionary_std::tags;
use dicom_object::InMemDicomObject;

use super::icc_profile::{ICC_COLOR_SPACE, ICC_PROFILE_BYTES};

pub(in crate::generator) const BLENDING_PRESENTATION_STATE_STORAGE_UID: &str =
    "1.2.840.10008.5.1.4.1.1.11.4";
pub(in crate::generator) const BLENDING_PRESENTATION_STATE_OUTPUT_FILE: &str = "instance.dcm";
pub(in crate::generator) const BLENDING_PRESENTATION_STATE_SERIES_NUMBER: &str = "81";
pub(in crate::generator) const BLENDING_PRESENTATION_STATE_CONTENT_LABEL: &str = "DTSBLEND";
pub(in crate::generator) const BLENDING_PRESENTATION_STATE_CONTENT_DESCRIPTION: &str =
    "Synthetic DTSBLEND presentation state";
pub(in crate::generator) const BLENDING_PRESENTATION_STATE_CREATION_DATE: &str = "20260101";
pub(in crate::generator) const BLENDING_PRESENTATION_STATE_CREATION_TIME: &str = "000000";
pub(in crate::generator) const BLENDING_PRESENTATION_STATE_RELATIVE_OPACITY: f32 = 0.5;
pub(in crate::generator) const BLENDING_PRESENTATION_STATE_PALETTE_DESCRIPTOR: [u16; 3] =
    [256, 0, 16];
pub(in crate::generator) const BLENDING_PRESENTATION_STATE_PALETTE_BYTES: [u8; 512] =
    identity_palette_bytes();

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::generator) struct BlendingPresentationStateReference<'a> {
    pub(in crate::generator) study_instance_uid: &'a str,
    pub(in crate::generator) series_instance_uid: &'a str,
    pub(in crate::generator) sop_class_uid: &'a str,
    pub(in crate::generator) sop_instance_uid: &'a str,
    pub(in crate::generator) frame_of_reference_uid: &'a str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::generator) struct BlendingPresentationStateInput<'a> {
    pub(in crate::generator) sop_instance_uid: &'a str,
    pub(in crate::generator) series_instance_uid: &'a str,
    /// Source identities in locked `[series ordinal][slice ordinal]` order.
    pub(in crate::generator) sources: [[BlendingPresentationStateReference<'a>; 2]; 2],
}

pub(in crate::generator) fn build_blending_presentation_state(
    input: BlendingPresentationStateInput<'_>,
) -> Result<InMemDicomObject, String> {
    validate_input(input)?;
    let source = input.sources[0][0];

    let mut object = InMemDicomObject::new_empty();
    put_str(
        &mut object,
        tags::SOP_CLASS_UID,
        VR::UI,
        BLENDING_PRESENTATION_STATE_STORAGE_UID,
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
        BLENDING_PRESENTATION_STATE_SERIES_NUMBER,
    );
    put_str(&mut object, tags::LATERALITY, VR::CS, "R");

    put_str(&mut object, tags::MANUFACTURER, VR::LO, "dicom-test-suite");
    put_str(&mut object, tags::INSTITUTION_NAME, VR::LO, "");
    put_str(&mut object, tags::INSTITUTION_ADDRESS, VR::ST, "");
    put_str(
        &mut object,
        tags::MANUFACTURER_MODEL_NAME,
        VR::LO,
        "Native Blending Softcopy Presentation State",
    );
    put_str(
        &mut object,
        tags::DEVICE_SERIAL_NUMBER,
        VR::LO,
        "DTS-BLEND-001",
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
        BLENDING_PRESENTATION_STATE_CREATION_DATE,
    );
    put_str(
        &mut object,
        tags::PRESENTATION_CREATION_TIME,
        VR::TM,
        BLENDING_PRESENTATION_STATE_CREATION_TIME,
    );
    put_str(
        &mut object,
        tags::CONTENT_LABEL,
        VR::CS,
        BLENDING_PRESENTATION_STATE_CONTENT_LABEL,
    );
    put_str(
        &mut object,
        tags::CONTENT_DESCRIPTION,
        VR::LO,
        BLENDING_PRESENTATION_STATE_CONTENT_DESCRIPTION,
    );
    put_str(
        &mut object,
        tags::CONTENT_CREATOR_NAME,
        VR::PN,
        "DTS^Generator",
    );

    object.put(DataElement::new(
        tags::BLENDING_SEQUENCE,
        VR::SQ,
        DataSetSequence::from(vec![
            blending_item("UNDERLYING", input.sources[0]),
            blending_item("SUPERIMPOSED", input.sources[1]),
        ]),
    ));
    object.put(DataElement::new(
        tags::RELATIVE_OPACITY,
        VR::FL,
        PrimitiveValue::F32(vec![BLENDING_PRESENTATION_STATE_RELATIVE_OPACITY].into()),
    ));

    object.put(DataElement::new(
        tags::DISPLAYED_AREA_SELECTION_SEQUENCE,
        VR::SQ,
        DataSetSequence::from(vec![displayed_area_item()]),
    ));

    put_palette(&mut object);
    object.put(DataElement::new(
        tags::ICC_PROFILE,
        VR::OB,
        PrimitiveValue::U8(ICC_PROFILE_BYTES.to_vec().into()),
    ));
    put_str(&mut object, tags::COLOR_SPACE, VR::CS, ICC_COLOR_SPACE);

    Ok(object)
}

fn validate_input(input: BlendingPresentationStateInput<'_>) -> Result<(), String> {
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

    let source_sop_instance_uids = input
        .sources
        .iter()
        .flatten()
        .map(|reference| reference.sop_instance_uid)
        .collect::<Vec<_>>();
    for (index, uid) in source_sop_instance_uids.iter().enumerate() {
        if source_sop_instance_uids[..index].contains(uid) {
            return Err("the four source SOP Instance UIDs must be unique".to_string());
        }
        if *uid == input.sop_instance_uid {
            return Err("the presentation state must use a distinct SOP Instance UID".to_string());
        }
    }
    Ok(())
}

fn blending_item(
    position: &str,
    sources: [BlendingPresentationStateReference<'_>; 2],
) -> InMemDicomObject {
    InMemDicomObject::from_element_iter([
        DataElement::new(tags::BLENDING_POSITION, VR::CS, position),
        DataElement::new(
            tags::STUDY_INSTANCE_UID,
            VR::UI,
            sources[0].study_instance_uid,
        ),
        DataElement::new(
            tags::REFERENCED_SERIES_SEQUENCE,
            VR::SQ,
            DataSetSequence::from(vec![InMemDicomObject::from_element_iter([
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
            ])]),
        ),
        DataElement::new(tags::RESCALE_INTERCEPT, VR::DS, "-1024"),
        DataElement::new(tags::RESCALE_SLOPE, VR::DS, "1"),
        DataElement::new(tags::RESCALE_TYPE, VR::LO, "HU"),
    ])
}

fn displayed_area_item() -> InMemDicomObject {
    InMemDicomObject::from_element_iter([
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
    ])
}

fn put_palette(object: &mut InMemDicomObject) {
    for tag in [
        tags::RED_PALETTE_COLOR_LOOKUP_TABLE_DESCRIPTOR,
        tags::GREEN_PALETTE_COLOR_LOOKUP_TABLE_DESCRIPTOR,
        tags::BLUE_PALETTE_COLOR_LOOKUP_TABLE_DESCRIPTOR,
    ] {
        object.put(DataElement::new(
            tag,
            VR::US,
            PrimitiveValue::from(BLENDING_PRESENTATION_STATE_PALETTE_DESCRIPTOR),
        ));
    }
    for tag in [
        tags::RED_PALETTE_COLOR_LOOKUP_TABLE_DATA,
        tags::GREEN_PALETTE_COLOR_LOOKUP_TABLE_DATA,
        tags::BLUE_PALETTE_COLOR_LOOKUP_TABLE_DATA,
    ] {
        object.put(DataElement::new(
            tag,
            VR::OW,
            PrimitiveValue::U8(BLENDING_PRESENTATION_STATE_PALETTE_BYTES.to_vec().into()),
        ));
    }
}

fn referenced_sop_item(reference: BlendingPresentationStateReference<'_>) -> InMemDicomObject {
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

const fn identity_palette_bytes() -> [u8; 512] {
    let mut bytes = [0_u8; 512];
    let mut entry = 0_usize;
    while entry < 256 {
        let value = (entry as u16) * 0x0101;
        bytes[entry * 2] = value as u8;
        bytes[entry * 2 + 1] = (value >> 8) as u8;
        entry += 1;
    }
    bytes
}

fn put_str(object: &mut InMemDicomObject, tag: dicom_core::Tag, vr: VR, value: &str) {
    object.put(DataElement::new(tag, vr, value));
}
