use dicom_core::VR;
use dicom_dictionary_std::tags;
use dicom_object::InMemDicomObject;

use crate::sha256_hex;

use super::twelve_lead_ecg::{
    TWELVE_LEAD_ECG_BITS_ALLOCATED, TWELVE_LEAD_ECG_CHANNEL_COUNT, TWELVE_LEAD_ECG_CHANNEL_SHA256,
    TWELVE_LEAD_ECG_CHANNELS, TWELVE_LEAD_ECG_INTERLEAVE, TWELVE_LEAD_ECG_OUTPUT_FILE,
    TWELVE_LEAD_ECG_PAYLOAD_LENGTH, TWELVE_LEAD_ECG_PAYLOAD_SHA256, TWELVE_LEAD_ECG_SAMPLE_COUNT,
    TWELVE_LEAD_ECG_SAMPLE_INTERPRETATION, TWELVE_LEAD_ECG_SAMPLING_FREQUENCY_HZ,
    TWELVE_LEAD_ECG_SERIES_NUMBER, TWELVE_LEAD_ECG_STORAGE_UID, TwelveLeadEcgInput,
    build_twelve_lead_ecg, twelve_lead_ecg_payload, twelve_lead_ecg_sample,
};

const STUDY_UID: &str = "2.25.100000000000000000000000000000000000001";
const SERIES_UID: &str = "2.25.100000000000000000000000000000000000002";
const SOP_UID: &str = "2.25.100000000000000000000000000000000000003";

fn locked_input() -> TwelveLeadEcgInput<'static> {
    TwelveLeadEcgInput {
        study_instance_uid: STUDY_UID,
        series_instance_uid: SERIES_UID,
        sop_instance_uid: SOP_UID,
    }
}

#[test]
fn twelve_lead_ecg_builds_locked_identity_and_required_modules() {
    let object = build_twelve_lead_ecg(locked_input()).expect("locked input");
    assert_eq!(TWELVE_LEAD_ECG_OUTPUT_FILE, "instance.dcm");

    for (tag, expected) in [
        (tags::SOP_CLASS_UID, TWELVE_LEAD_ECG_STORAGE_UID),
        (tags::SOP_INSTANCE_UID, SOP_UID),
        (tags::SYNTHETIC_DATA, "YES"),
        (tags::PATIENT_NAME, "DTS^Synthetic^Patient001"),
        (tags::PATIENT_ID, "DTS-PATIENT-001"),
        (tags::STUDY_INSTANCE_UID, STUDY_UID),
        (tags::STUDY_ID, "DTS-ECG"),
        (tags::MODALITY, "ECG"),
        (tags::SERIES_INSTANCE_UID, SERIES_UID),
        (tags::SERIES_NUMBER, TWELVE_LEAD_ECG_SERIES_NUMBER),
        (tags::INSTANCE_NUMBER, "1"),
        (tags::CONTENT_DATE, "20260101"),
        (tags::CONTENT_TIME, "000000"),
        (tags::ACQUISITION_DATE_TIME, "20260101000000"),
    ] {
        assert_eq!(text(&object, tag), expected, "{tag:?}");
    }

    assert!(sequence(&object, tags::ACQUISITION_CONTEXT_SEQUENCE).is_empty());
    for tag in [
        tags::REFERENCED_IMAGE_SEQUENCE,
        tags::PIXEL_DATA,
        tags::ROWS,
        tags::COLUMNS,
    ] {
        assert!(object.element(tag).is_err(), "{tag:?} must be absent");
    }
}

#[test]
fn twelve_lead_ecg_builds_one_locked_multiplex_group() {
    let object = build_twelve_lead_ecg(locked_input()).expect("locked input");
    let groups = sequence(&object, tags::WAVEFORM_SEQUENCE);
    assert_eq!(groups.len(), 1);
    let group = &groups[0];

    assert_eq!(text(group, tags::WAVEFORM_ORIGINALITY), "ORIGINAL");
    assert_eq!(number_u16(group, tags::NUMBER_OF_WAVEFORM_CHANNELS), 12);
    assert_eq!(number_u32(group, tags::NUMBER_OF_WAVEFORM_SAMPLES), 500);
    assert_eq!(
        text(group, tags::SAMPLING_FREQUENCY),
        TWELVE_LEAD_ECG_SAMPLING_FREQUENCY_HZ
    );
    assert_eq!(text(group, tags::MULTIPLEX_GROUP_LABEL), "RESTING_12_LEAD");
    assert_eq!(
        number_u16(group, tags::WAVEFORM_BITS_ALLOCATED),
        TWELVE_LEAD_ECG_BITS_ALLOCATED
    );
    assert_eq!(
        text(group, tags::WAVEFORM_SAMPLE_INTERPRETATION),
        TWELVE_LEAD_ECG_SAMPLE_INTERPRETATION
    );
    assert!(group.element(tags::WAVEFORM_PADDING_VALUE).is_err());

    let waveform_data = group.element(tags::WAVEFORM_DATA).expect("Waveform Data");
    assert_eq!(waveform_data.vr(), VR::OW);
    assert_eq!(
        waveform_data.to_bytes().unwrap().len(),
        TWELVE_LEAD_ECG_PAYLOAD_LENGTH
    );
}

#[test]
fn twelve_lead_ecg_builds_ordered_cid_3001_channels() {
    let object = build_twelve_lead_ecg(locked_input()).expect("locked input");
    let groups = sequence(&object, tags::WAVEFORM_SEQUENCE);
    let channels = sequence(&groups[0], tags::CHANNEL_DEFINITION_SEQUENCE);
    assert_eq!(channels.len(), TWELVE_LEAD_ECG_CHANNEL_COUNT as usize);

    for (index, (item, expected)) in channels.iter().zip(TWELVE_LEAD_ECG_CHANNELS).enumerate() {
        assert_eq!(
            text(item, tags::WAVEFORM_CHANNEL_NUMBER),
            (index + 1).to_string()
        );
        assert_eq!(text(item, tags::CHANNEL_LABEL), expected.label);
        assert_eq!(text(item, tags::CHANNEL_SENSITIVITY), "1");
        assert_eq!(text(item, tags::CHANNEL_SENSITIVITY_CORRECTION_FACTOR), "1");
        assert_eq!(text(item, tags::CHANNEL_BASELINE), "0");
        assert_eq!(text(item, tags::CHANNEL_TIME_SKEW), "0");
        assert!(item.element(tags::CHANNEL_SAMPLE_SKEW).is_err());
        assert_eq!(number_u16(item, tags::WAVEFORM_BITS_STORED), 16);

        let source = sequence(item, tags::CHANNEL_SOURCE_SEQUENCE);
        assert_eq!(source.len(), 1);
        assert_eq!(text(&source[0], tags::CODING_SCHEME_DESIGNATOR), "MDC");
        assert_eq!(text(&source[0], tags::CODE_VALUE), expected.code_value);
        assert_eq!(text(&source[0], tags::CODE_MEANING), expected.code_meaning);

        let units = sequence(item, tags::CHANNEL_SENSITIVITY_UNITS_SEQUENCE);
        assert_eq!(units.len(), 1);
        assert_eq!(text(&units[0], tags::CODING_SCHEME_DESIGNATOR), "UCUM");
        assert_eq!(text(&units[0], tags::CODE_VALUE), "uV");
        assert_eq!(text(&units[0], tags::CODE_MEANING), "microvolt");
    }
}

#[test]
fn twelve_lead_ecg_payload_matches_formula_interleave_and_hashes() {
    assert_eq!(TWELVE_LEAD_ECG_INTERLEAVE, "channel_then_sample");
    let payload = twelve_lead_ecg_payload();
    assert_eq!(payload.len(), TWELVE_LEAD_ECG_PAYLOAD_LENGTH);
    assert_eq!(sha256_hex(&payload), TWELVE_LEAD_ECG_PAYLOAD_SHA256);

    let decoded = payload
        .chunks_exact(2)
        .map(|bytes| i16::from_le_bytes([bytes[0], bytes[1]]))
        .collect::<Vec<_>>();
    assert_eq!(
        decoded.len(),
        TWELVE_LEAD_ECG_CHANNEL_COUNT as usize * TWELVE_LEAD_ECG_SAMPLE_COUNT as usize
    );
    assert_eq!(decoded.iter().copied().min(), Some(-1000));
    assert_eq!(decoded.iter().copied().max(), Some(1000));

    for sample in 0..TWELVE_LEAD_ECG_SAMPLE_COUNT as usize {
        for channel in 0..TWELVE_LEAD_ECG_CHANNEL_COUNT as usize {
            assert_eq!(
                decoded[sample * TWELVE_LEAD_ECG_CHANNEL_COUNT as usize + channel],
                twelve_lead_ecg_sample(sample, channel),
                "sample {sample}, channel {channel}"
            );
        }
    }

    for (channel, expected_hash) in TWELVE_LEAD_ECG_CHANNEL_SHA256.iter().enumerate() {
        let mut channel_bytes = Vec::with_capacity(TWELVE_LEAD_ECG_SAMPLE_COUNT as usize * 2);
        for sample in 0..TWELVE_LEAD_ECG_SAMPLE_COUNT as usize {
            channel_bytes.extend_from_slice(
                &decoded[sample * TWELVE_LEAD_ECG_CHANNEL_COUNT as usize + channel].to_le_bytes(),
            );
        }
        assert_eq!(
            &sha256_hex(&channel_bytes),
            expected_hash,
            "channel {channel}"
        );
    }
}

#[test]
fn twelve_lead_ecg_rejects_missing_or_reused_uid_roles() {
    for input in [
        TwelveLeadEcgInput {
            study_instance_uid: "",
            ..locked_input()
        },
        TwelveLeadEcgInput {
            series_instance_uid: "",
            ..locked_input()
        },
        TwelveLeadEcgInput {
            sop_instance_uid: "",
            ..locked_input()
        },
    ] {
        assert!(
            build_twelve_lead_ecg(input)
                .unwrap_err()
                .contains("must not be empty")
        );
    }

    let error = build_twelve_lead_ecg(TwelveLeadEcgInput {
        study_instance_uid: STUDY_UID,
        series_instance_uid: STUDY_UID,
        sop_instance_uid: SOP_UID,
    })
    .unwrap_err();
    assert!(error.contains("must be distinct"));
}

fn sequence(object: &InMemDicomObject, tag: dicom_core::Tag) -> &[InMemDicomObject] {
    object
        .element(tag)
        .expect("sequence")
        .items()
        .expect("items")
}

fn text(object: &InMemDicomObject, tag: dicom_core::Tag) -> String {
    object
        .element(tag)
        .expect("attribute")
        .to_str()
        .expect("text")
        .into_owned()
}

fn number_u16(object: &InMemDicomObject, tag: dicom_core::Tag) -> u16 {
    object
        .element(tag)
        .expect("attribute")
        .to_int()
        .expect("US")
}

fn number_u32(object: &InMemDicomObject, tag: dicom_core::Tag) -> u32 {
    object
        .element(tag)
        .expect("attribute")
        .to_int()
        .expect("UL")
}
