use std::path::PathBuf;

use dicom_core::{DataElement, PrimitiveValue, VR, value::DataSetSequence};
use dicom_dictionary_std::{tags, uids};
use dicom_object::{FileMetaTableBuilder, InMemDicomObject};

use super::{GeneralEcgExpectations, validate_general_ecg_file};
use crate::waveform_manifest::{ExpectedMultiplexGroup, general_ecg_expected_waveform};

const SOP_UID: &str = "2.25.910000000000000000000000000000000000003";
const STUDY_UID: &str = "2.25.910000000000000000000000000000000000001";
const SERIES_UID: &str = "2.25.910000000000000000000000000000000000002";
const IMPLEMENTATION_UID: &str = "2.25.910000000000000000000000000000000000004";

#[derive(Clone, Copy)]
enum Mutation {
    None,
    FiveGroups,
    ReverseGroups,
    TwentyFiveChannels,
    FirstRate199,
    SecondRate1001,
    DuplicateCid,
    WrongCid,
    UnsignedSamples,
    MissingBothSkews,
    SamplePayloadMismatch,
    CorruptPayload,
}

#[test]
fn accepts_exact_general_ecg_contract() {
    let path = write_fixture("valid", Mutation::None);
    let validated = validate_general_ecg_file(&path, &expectations())
        .expect("exact General ECG contract should validate");
    assert_eq!(validated.validation["status"], "passed");
    assert!(
        validated.validation["internal"]
            .as_array()
            .unwrap()
            .iter()
            .all(|finding| finding["status"] == "passed")
    );
    assert!(
        validated.validation["standards"]
            .as_array()
            .unwrap()
            .iter()
            .any(|finding| finding["name"] == "general_ecg_waveform_sop_class")
    );
    std::fs::remove_file(path).ok();
}

#[test]
fn rejects_general_ecg_dataset_tampering() {
    for (label, mutation, finding) in [
        (
            "five-groups",
            Mutation::FiveGroups,
            "general_ecg_group_count",
        ),
        ("reversed", Mutation::ReverseGroups, "general_ecg_label"),
        (
            "25-channels",
            Mutation::TwentyFiveChannels,
            "general_ecg_channel_count",
        ),
        (
            "rate-199",
            Mutation::FirstRate199,
            "general_ecg_sampling_frequency",
        ),
        (
            "rate-1001",
            Mutation::SecondRate1001,
            "general_ecg_sampling_frequency",
        ),
        (
            "duplicate-cid",
            Mutation::DuplicateCid,
            "general_ecg_group_2_channel_4_source_value",
        ),
        (
            "wrong-cid",
            Mutation::WrongCid,
            "general_ecg_group_2_channel_1_source_value",
        ),
        (
            "unsigned",
            Mutation::UnsignedSamples,
            "general_ecg_sample_interpretation",
        ),
        (
            "missing-skews",
            Mutation::MissingBothSkews,
            "general_ecg_group_1_channel_1_time_skew_present",
        ),
        (
            "shape-payload",
            Mutation::SamplePayloadMismatch,
            "general_ecg_sample_count",
        ),
        (
            "payload-byte",
            Mutation::CorruptPayload,
            "general_ecg_payload_sha256",
        ),
    ] {
        let path = write_fixture(label, mutation);
        let error = validate_general_ecg_file(&path, &expectations())
            .expect_err("tampered General ECG must fail")
            .to_string();
        assert!(
            error.contains(finding),
            "{label}: unexpected validation error: {error}"
        );
        std::fs::remove_file(path).ok();
    }
}

#[test]
fn rejects_general_ecg_manifest_aggregate_mismatch() {
    let path = write_fixture("aggregate", Mutation::None);
    let mut expected = expectations();
    expected.waveform.aggregate.total_payload_length_bytes = 55_998;
    let error = validate_general_ecg_file(&path, &expected)
        .expect_err("aggregate mismatch must fail")
        .to_string();
    assert!(error.contains("general_ecg_manifest_aggregate_payload_length"));
    std::fs::remove_file(path).ok();
}

fn expectations() -> GeneralEcgExpectations<'static> {
    GeneralEcgExpectations {
        sop_instance_uid: SOP_UID,
        implementation_class_uid: IMPLEMENTATION_UID,
        study_instance_uid: STUDY_UID,
        series_instance_uid: SERIES_UID,
        waveform: general_ecg_expected_waveform(),
    }
}

fn write_fixture(label: &str, mutation: Mutation) -> PathBuf {
    let mut object = valid_object();
    apply_mutation(&mut object, mutation);
    let path = std::env::temp_dir().join(format!(
        "dicom-test-suite-general-ecg-validation-{}-{label}.dcm",
        std::process::id()
    ));
    object
        .with_meta(
            FileMetaTableBuilder::new()
                .media_storage_sop_class_uid("1.2.840.10008.5.1.4.1.1.9.1.2")
                .media_storage_sop_instance_uid(SOP_UID)
                .transfer_syntax(uids::EXPLICIT_VR_LITTLE_ENDIAN)
                .implementation_class_uid(IMPLEMENTATION_UID),
        )
        .expect("file meta")
        .write_to_file(&path)
        .expect("write fixture");
    path
}

fn apply_mutation(object: &mut InMemDicomObject, mutation: Mutation) {
    if matches!(mutation, Mutation::None) {
        return;
    }
    let mut waveform_element = object
        .take_element(tags::WAVEFORM_SEQUENCE)
        .expect("waveform sequence");
    let groups = waveform_element.items_mut().expect("waveform items");
    match mutation {
        Mutation::None => unreachable!(),
        Mutation::FiveGroups => {
            let cloned = groups[0].clone();
            while groups.len() < 5 {
                groups.push(cloned.clone());
            }
        }
        Mutation::ReverseGroups => groups.swap(0, 1),
        Mutation::TwentyFiveChannels => {
            groups[0].put(DataElement::new(
                tags::NUMBER_OF_WAVEFORM_CHANNELS,
                VR::US,
                PrimitiveValue::from(25_u16),
            ));
            let mut channels_element = groups[0]
                .take_element(tags::CHANNEL_DEFINITION_SEQUENCE)
                .expect("channel sequence");
            let channels = channels_element.items_mut().expect("channel items");
            let template = channels[0].clone();
            while channels.len() < 25 {
                let mut channel = template.clone();
                put_str(
                    &mut channel,
                    tags::WAVEFORM_CHANNEL_NUMBER,
                    VR::IS,
                    &(channels.len() + 1).to_string(),
                );
                channels.push(channel);
            }
            groups[0].put(channels_element);
        }
        Mutation::FirstRate199 => put_str(&mut groups[0], tags::SAMPLING_FREQUENCY, VR::DS, "199"),
        Mutation::SecondRate1001 => {
            put_str(&mut groups[1], tags::SAMPLING_FREQUENCY, VR::DS, "1001")
        }
        Mutation::UnsignedSamples => put_str(
            &mut groups[0],
            tags::WAVEFORM_SAMPLE_INTERPRETATION,
            VR::CS,
            "US",
        ),
        Mutation::SamplePayloadMismatch => {
            groups[0].put(DataElement::new(
                tags::NUMBER_OF_WAVEFORM_SAMPLES,
                VR::UL,
                PrimitiveValue::from(999_u32),
            ));
        }
        Mutation::CorruptPayload => {
            let mut payload = group_payload(1, 4_000, 4);
            payload[117] ^= 1;
            groups[1].put(DataElement::new(
                tags::WAVEFORM_DATA,
                VR::OW,
                PrimitiveValue::U8(payload.into()),
            ));
        }
        Mutation::DuplicateCid | Mutation::WrongCid | Mutation::MissingBothSkews => {
            let group_index = usize::from(!matches!(mutation, Mutation::MissingBothSkews));
            let mut channels_element = groups[group_index]
                .take_element(tags::CHANNEL_DEFINITION_SEQUENCE)
                .expect("channel sequence");
            let channels = channels_element.items_mut().expect("channel items");
            if matches!(mutation, Mutation::MissingBothSkews) {
                channels[0].take_element(tags::CHANNEL_TIME_SKEW).unwrap();
            } else {
                let channel_index = if matches!(mutation, Mutation::DuplicateCid) {
                    3
                } else {
                    0
                };
                let mut source_element = channels[channel_index]
                    .take_element(tags::CHANNEL_SOURCE_SEQUENCE)
                    .expect("source sequence");
                let source = source_element.items_mut().unwrap().first_mut().unwrap();
                put_str(
                    source,
                    tags::CODE_VALUE,
                    VR::SH,
                    if matches!(mutation, Mutation::DuplicateCid) {
                        "2:75"
                    } else {
                        "2:99"
                    },
                );
                channels[channel_index].put(source_element);
            }
            groups[group_index].put(channels_element);
        }
    }
    object.put(waveform_element);
}

fn valid_object() -> InMemDicomObject {
    let mut object = InMemDicomObject::new_empty();
    for (tag, vr, value) in [
        (tags::SOP_CLASS_UID, VR::UI, "1.2.840.10008.5.1.4.1.1.9.1.2"),
        (tags::SOP_INSTANCE_UID, VR::UI, SOP_UID),
        (tags::SYNTHETIC_DATA, VR::CS, "YES"),
        (tags::PATIENT_NAME, VR::PN, "DTS^Synthetic^Patient001"),
        (tags::PATIENT_ID, VR::LO, "DTS-PATIENT-001"),
        (tags::PATIENT_BIRTH_DATE, VR::DA, "19700101"),
        (tags::PATIENT_SEX, VR::CS, "O"),
        (tags::STUDY_INSTANCE_UID, VR::UI, STUDY_UID),
        (tags::STUDY_DATE, VR::DA, "20260101"),
        (tags::STUDY_TIME, VR::TM, "000000"),
        (tags::REFERRING_PHYSICIAN_NAME, VR::PN, ""),
        (tags::STUDY_ID, VR::SH, "DTS-ECG"),
        (tags::ACCESSION_NUMBER, VR::SH, ""),
        (tags::MODALITY, VR::CS, "ECG"),
        (tags::SERIES_INSTANCE_UID, VR::UI, SERIES_UID),
        (tags::SERIES_NUMBER, VR::IS, "91"),
        (tags::MANUFACTURER, VR::LO, "dicom-test-suite"),
        (tags::INSTITUTION_NAME, VR::LO, ""),
        (tags::INSTITUTION_ADDRESS, VR::ST, ""),
        (tags::MANUFACTURER_MODEL_NAME, VR::LO, "Native General ECG"),
        (tags::DEVICE_SERIAL_NUMBER, VR::LO, "DTS-GECG-001"),
        (
            tags::SOFTWARE_VERSIONS,
            VR::LO,
            crate::BYTE_STABLE_OUTPUT_VERSION,
        ),
        (tags::INSTANCE_NUMBER, VR::IS, "1"),
        (tags::CONTENT_DATE, VR::DA, "20260101"),
        (tags::CONTENT_TIME, VR::TM, "000000"),
        (tags::ACQUISITION_DATE_TIME, VR::DT, "20260101000000"),
    ] {
        put_str(&mut object, tag, vr, value);
    }
    object.put(DataElement::new(
        tags::ACQUISITION_CONTEXT_SEQUENCE,
        VR::SQ,
        DataSetSequence::empty(),
    ));
    let expected = general_ecg_expected_waveform();
    object.put(DataElement::new(
        tags::WAVEFORM_SEQUENCE,
        VR::SQ,
        DataSetSequence::from(
            expected
                .multiplex_groups
                .iter()
                .enumerate()
                .map(|(index, group)| valid_group(index, group))
                .collect::<Vec<_>>(),
        ),
    ));
    object
}

fn valid_group(index: usize, expected: &ExpectedMultiplexGroup<'_>) -> InMemDicomObject {
    InMemDicomObject::from_element_iter([
        DataElement::new(tags::WAVEFORM_ORIGINALITY, VR::CS, expected.originality),
        DataElement::new(
            tags::NUMBER_OF_WAVEFORM_CHANNELS,
            VR::US,
            PrimitiveValue::from(u16::from(expected.channel_count)),
        ),
        DataElement::new(
            tags::NUMBER_OF_WAVEFORM_SAMPLES,
            VR::UL,
            PrimitiveValue::from(u32::from(expected.samples_per_channel)),
        ),
        DataElement::new(
            tags::SAMPLING_FREQUENCY,
            VR::DS,
            expected.sampling_frequency_hz.to_string(),
        ),
        DataElement::new(tags::MULTIPLEX_GROUP_LABEL, VR::SH, expected.label),
        DataElement::new(
            tags::CHANNEL_DEFINITION_SEQUENCE,
            VR::SQ,
            DataSetSequence::from(
                expected
                    .channels
                    .iter()
                    .map(valid_channel)
                    .collect::<Vec<_>>(),
            ),
        ),
        DataElement::new(
            tags::WAVEFORM_BITS_ALLOCATED,
            VR::US,
            PrimitiveValue::from(u16::from(expected.storage.bits_allocated)),
        ),
        DataElement::new(
            tags::WAVEFORM_SAMPLE_INTERPRETATION,
            VR::CS,
            expected.storage.sample_interpretation,
        ),
        DataElement::new(
            tags::WAVEFORM_DATA,
            VR::OW,
            PrimitiveValue::U8(
                group_payload(
                    index,
                    usize::from(expected.samples_per_channel),
                    expected.channels.len(),
                )
                .into(),
            ),
        ),
    ])
}

fn valid_channel(
    expected: &crate::waveform_manifest::ExpectedWaveformChannel<'_>,
) -> InMemDicomObject {
    InMemDicomObject::from_element_iter([
        DataElement::new(
            tags::WAVEFORM_CHANNEL_NUMBER,
            VR::IS,
            expected.ordinal.to_string(),
        ),
        DataElement::new(tags::CHANNEL_LABEL, VR::SH, expected.label),
        DataElement::new(
            tags::CHANNEL_SOURCE_SEQUENCE,
            VR::SQ,
            DataSetSequence::from(vec![code_item(
                expected.source.coding_scheme_designator,
                expected.source.code_value,
                expected.source.code_meaning,
            )]),
        ),
        DataElement::new(tags::CHANNEL_SENSITIVITY, VR::DS, "1"),
        DataElement::new(
            tags::CHANNEL_SENSITIVITY_UNITS_SEQUENCE,
            VR::SQ,
            DataSetSequence::from(vec![code_item("UCUM", "uV", "microvolt")]),
        ),
        DataElement::new(tags::CHANNEL_SENSITIVITY_CORRECTION_FACTOR, VR::DS, "1"),
        DataElement::new(tags::CHANNEL_BASELINE, VR::DS, "0"),
        DataElement::new(tags::CHANNEL_TIME_SKEW, VR::DS, "0"),
        DataElement::new(
            tags::WAVEFORM_BITS_STORED,
            VR::US,
            PrimitiveValue::from(16_u16),
        ),
    ])
}

fn group_payload(group: usize, samples: usize, channels: usize) -> Vec<u8> {
    let mut payload = Vec::with_capacity(samples * channels * 2);
    for sample in 0..samples {
        for channel in 0..channels {
            let value = (((sample * (channel + 1) * (group + 1) * 37 + channel * 101 + group * 307)
                % 2001) as i32
                - 1000) as i16;
            payload.extend_from_slice(&value.to_le_bytes());
        }
    }
    payload
}

fn code_item(scheme: &str, value: &str, meaning: &str) -> InMemDicomObject {
    InMemDicomObject::from_element_iter([
        DataElement::new(tags::CODE_VALUE, VR::SH, value),
        DataElement::new(tags::CODING_SCHEME_DESIGNATOR, VR::SH, scheme),
        DataElement::new(tags::CODE_MEANING, VR::LO, meaning),
    ])
}

fn put_str(object: &mut InMemDicomObject, tag: dicom_core::Tag, vr: VR, value: &str) {
    object.put(DataElement::new(tag, vr, value));
}
