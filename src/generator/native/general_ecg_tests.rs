use dicom_core::VR;
use dicom_dictionary_std::tags;
use dicom_object::InMemDicomObject;

use crate::sha256_hex;

use super::general_ecg::{
    GENERAL_ECG_AGGREGATE_SHA256, GENERAL_ECG_BITS_ALLOCATED, GENERAL_ECG_GROUPS,
    GENERAL_ECG_OUTPUT_FILE, GENERAL_ECG_SAMPLE_INTERPRETATION, GENERAL_ECG_SERIES_NUMBER,
    GENERAL_ECG_STORAGE_UID, GENERAL_ECG_TOTAL_CHANNEL_COUNT, GENERAL_ECG_TOTAL_PAYLOAD_LENGTH,
    GeneralEcgInput, build_general_ecg, general_ecg_group_payload, general_ecg_ordered_payload,
};

const STUDY_UID: &str = "2.25.200000000000000000000000000000000000001";
const SERIES_UID: &str = "2.25.200000000000000000000000000000000000002";
const SOP_UID: &str = "2.25.200000000000000000000000000000000000003";

fn locked_input() -> GeneralEcgInput<'static> {
    GeneralEcgInput {
        study_instance_uid: STUDY_UID,
        series_instance_uid: SERIES_UID,
        sop_instance_uid: SOP_UID,
    }
}

#[test]
fn general_ecg_builds_locked_identity_and_required_modules() {
    let object = build_general_ecg(locked_input()).expect("locked input");
    assert_eq!(GENERAL_ECG_OUTPUT_FILE, "instance.dcm");

    for (tag, expected) in [
        (tags::SOP_CLASS_UID, GENERAL_ECG_STORAGE_UID),
        (tags::SOP_INSTANCE_UID, SOP_UID),
        (tags::SYNTHETIC_DATA, "YES"),
        (tags::PATIENT_NAME, "DTS^Synthetic^Patient001"),
        (tags::PATIENT_ID, "DTS-PATIENT-001"),
        (tags::STUDY_INSTANCE_UID, STUDY_UID),
        (tags::STUDY_ID, "DTS-ECG"),
        (tags::MODALITY, "ECG"),
        (tags::SERIES_INSTANCE_UID, SERIES_UID),
        (tags::SERIES_NUMBER, GENERAL_ECG_SERIES_NUMBER),
        (tags::INSTANCE_NUMBER, "1"),
        (tags::CONTENT_DATE, "20260101"),
        (tags::CONTENT_TIME, "000000"),
        (tags::ACQUISITION_DATE_TIME, "20260101000000"),
    ] {
        assert_eq!(text(&object, tag), expected, "{tag:?}");
    }

    assert!(sequence(&object, tags::ACQUISITION_CONTEXT_SEQUENCE).is_empty());
    for tag in [
        tags::SYNCHRONIZATION_FRAME_OF_REFERENCE_UID,
        tags::WAVEFORM_ANNOTATION_SEQUENCE,
        tags::REFERENCED_INSTANCE_SEQUENCE,
        tags::REFERENCED_IMAGE_SEQUENCE,
        tags::PIXEL_DATA,
        tags::ROWS,
        tags::COLUMNS,
    ] {
        assert!(object.element(tag).is_err(), "{tag:?} must be absent");
    }
}

#[test]
fn general_ecg_builds_two_ordered_heterogeneous_groups() {
    let object = build_general_ecg(locked_input()).expect("locked input");
    let groups = sequence(&object, tags::WAVEFORM_SEQUENCE);
    assert_eq!(groups.len(), 2);

    for (index, (item, expected)) in groups.iter().zip(GENERAL_ECG_GROUPS).enumerate() {
        assert_eq!(text(item, tags::WAVEFORM_ORIGINALITY), "ORIGINAL");
        assert_eq!(text(item, tags::MULTIPLEX_GROUP_LABEL), expected.label);
        assert_eq!(
            number_u16(item, tags::NUMBER_OF_WAVEFORM_CHANNELS),
            expected.channels.len() as u16
        );
        assert_eq!(
            number_u32(item, tags::NUMBER_OF_WAVEFORM_SAMPLES),
            expected.sample_count
        );
        assert_eq!(
            text(item, tags::SAMPLING_FREQUENCY),
            expected.sampling_frequency_hz
        );
        assert_eq!(
            number_u16(item, tags::WAVEFORM_BITS_ALLOCATED),
            GENERAL_ECG_BITS_ALLOCATED
        );
        assert_eq!(
            text(item, tags::WAVEFORM_SAMPLE_INTERPRETATION),
            GENERAL_ECG_SAMPLE_INTERPRETATION
        );

        let data = item.element(tags::WAVEFORM_DATA).expect("Waveform Data");
        assert_eq!(data.vr(), VR::OW);
        let bytes = data.to_bytes().expect("raw OW bytes");
        assert_eq!(bytes.len(), expected.payload_length);
        assert_eq!(
            sha256_hex(bytes.as_ref()),
            expected.payload_sha256,
            "group {index}"
        );

        for tag in [
            tags::MULTIPLEX_GROUP_TIME_OFFSET,
            tags::TRIGGER_TIME_OFFSET,
            tags::TRIGGER_SAMPLE_POSITION,
            tags::WAVEFORM_PADDING_VALUE,
        ] {
            assert!(
                item.element(tag).is_err(),
                "group {index} {tag:?} must be absent"
            );
        }
    }

    assert_eq!(
        number_u16(&groups[0], tags::NUMBER_OF_WAVEFORM_CHANNELS),
        12
    );
    assert_eq!(
        number_u32(&groups[0], tags::NUMBER_OF_WAVEFORM_SAMPLES),
        1_000
    );
    assert_eq!(text(&groups[0], tags::SAMPLING_FREQUENCY), "250");
    assert_eq!(number_u16(&groups[1], tags::NUMBER_OF_WAVEFORM_CHANNELS), 4);
    assert_eq!(
        number_u32(&groups[1], tags::NUMBER_OF_WAVEFORM_SAMPLES),
        4_000
    );
    assert_eq!(text(&groups[1], tags::SAMPLING_FREQUENCY), "1000");
    assert_eq!(
        number_u32(&groups[0], tags::NUMBER_OF_WAVEFORM_SAMPLES) / 250,
        4
    );
    assert_eq!(
        number_u32(&groups[1], tags::NUMBER_OF_WAVEFORM_SAMPLES) / 1_000,
        4
    );
}

#[test]
fn general_ecg_builds_locked_local_channel_definitions() {
    let object = build_general_ecg(locked_input()).expect("locked input");
    let groups = sequence(&object, tags::WAVEFORM_SEQUENCE);
    let mut source_codes = Vec::new();

    for (group_index, (group_item, expected_group)) in
        groups.iter().zip(GENERAL_ECG_GROUPS).enumerate()
    {
        let channels = sequence(group_item, tags::CHANNEL_DEFINITION_SEQUENCE);
        assert_eq!(channels.len(), expected_group.channels.len());
        for (channel_index, (item, expected)) in
            channels.iter().zip(expected_group.channels).enumerate()
        {
            assert_eq!(
                text(item, tags::WAVEFORM_CHANNEL_NUMBER),
                (channel_index + 1).to_string(),
                "group {group_index} local ordinal"
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
            source_codes.push(text(&source[0], tags::CODE_VALUE));

            let units = sequence(item, tags::CHANNEL_SENSITIVITY_UNITS_SEQUENCE);
            assert_eq!(units.len(), 1);
            assert_eq!(text(&units[0], tags::CODING_SCHEME_DESIGNATOR), "UCUM");
            assert_eq!(text(&units[0], tags::CODE_VALUE), "uV");
            assert_eq!(text(&units[0], tags::CODE_MEANING), "microvolt");
        }
    }

    assert_eq!(source_codes.len(), GENERAL_ECG_TOTAL_CHANNEL_COUNT);
    source_codes.sort();
    source_codes.dedup();
    assert_eq!(source_codes.len(), GENERAL_ECG_TOTAL_CHANNEL_COUNT);
}

#[test]
fn general_ecg_payloads_match_formula_group_and_channel_hashes() {
    for (group_index, group) in GENERAL_ECG_GROUPS.iter().enumerate() {
        let payload = general_ecg_group_payload(group_index);
        assert_eq!(payload.len(), group.payload_length);
        assert_eq!(sha256_hex(&payload), group.payload_sha256);

        let decoded = payload
            .chunks_exact(2)
            .map(|bytes| i16::from_le_bytes([bytes[0], bytes[1]]))
            .collect::<Vec<_>>();
        assert_eq!(
            decoded.len(),
            group.channels.len() * group.sample_count as usize
        );
        assert_eq!(decoded.iter().copied().min(), Some(-1000));
        assert_eq!(decoded.iter().copied().max(), Some(1000));

        for sample in 0..group.sample_count as usize {
            for channel in 0..group.channels.len() {
                let formula = (((sample * (channel + 1) * (group_index + 1) * 37
                    + channel * 101
                    + group_index * 307)
                    % 2001) as i32
                    - 1000) as i16;
                assert_eq!(
                    decoded[sample * group.channels.len() + channel],
                    formula,
                    "group {group_index}, sample {sample}, channel {channel}"
                );
            }
        }

        assert_eq!(group.channel_sha256.len(), group.channels.len());
        for (channel, expected_hash) in group.channel_sha256.iter().enumerate() {
            let mut channel_bytes = Vec::with_capacity(group.sample_count as usize * 2);
            for sample in 0..group.sample_count as usize {
                channel_bytes.extend_from_slice(
                    &decoded[sample * group.channels.len() + channel].to_le_bytes(),
                );
            }
            assert_eq!(
                &sha256_hex(&channel_bytes),
                expected_hash,
                "group {group_index}, channel {channel}"
            );
        }
    }
}

#[test]
fn general_ecg_locks_ordered_aggregate_payload_hash() {
    let payload = general_ecg_ordered_payload();
    assert_eq!(payload.len(), GENERAL_ECG_TOTAL_PAYLOAD_LENGTH);
    assert_eq!(sha256_hex(&payload), GENERAL_ECG_AGGREGATE_SHA256);
    assert_eq!(
        &payload[..GENERAL_ECG_GROUPS[0].payload_length],
        general_ecg_group_payload(0)
    );
    assert_eq!(
        &payload[GENERAL_ECG_GROUPS[0].payload_length..],
        general_ecg_group_payload(1)
    );
}

#[test]
fn general_ecg_rejects_missing_or_reused_uid_roles() {
    for input in [
        GeneralEcgInput {
            study_instance_uid: "",
            ..locked_input()
        },
        GeneralEcgInput {
            series_instance_uid: "",
            ..locked_input()
        },
        GeneralEcgInput {
            sop_instance_uid: "",
            ..locked_input()
        },
    ] {
        assert!(
            build_general_ecg(input)
                .unwrap_err()
                .contains("must not be empty")
        );
    }

    let error = build_general_ecg(GeneralEcgInput {
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
