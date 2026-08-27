use std::{fs, path::PathBuf};

use dicom_core::{DataElement, PrimitiveValue, VR, value::DataSetSequence};
use dicom_dictionary_std::{tags, uids};
use dicom_object::{FileMetaTableBuilder, InMemDicomObject};

use super::{TwelveLeadEcgExpectations, validate_twelve_lead_ecg_file};
use crate::waveform_manifest::{TWELVE_LEAD_ECG_CHANNELS, twelve_lead_ecg_expected_waveform};

const SOP_UID: &str = "2.25.910000000000000000000000000000000000001";
const IMPLEMENTATION_UID: &str = "2.25.910000000000000000000000000000000000002";
const STUDY_UID: &str = "2.25.910000000000000000000000000000000000003";
const SERIES_UID: &str = "2.25.910000000000000000000000000000000000004";

#[derive(Clone, Copy)]
enum Mutation {
    None,
    WrongSopClass,
    NonEmptyAcquisitionContext,
    WrongChannelCount,
    WrongSampleCount,
    WrongSamplingFrequency,
    UnsignedSamples,
    ReorderedChannels,
    DuplicateLeadCode,
    WrongSensitivity,
    WrongUnits,
    WrongBitsStored,
    WrongTimeSkew,
    AddedSampleSkew,
    WrongDataVr,
    CorruptPayload,
    SampleMajorPayload,
    AddedPadding,
    AddedAnnotation,
    AddedSynchronization,
    AddedReference,
    AddedPixels,
}

#[test]
fn accepts_exact_twelve_lead_ecg_contract() {
    let path = write_fixture("valid", Mutation::None);
    let validated = validate_twelve_lead_ecg_file(&path, &expectations())
        .expect("exact Twelve-lead ECG should validate");
    assert_eq!(validated.validation["status"], "passed");
    assert!(
        validated.validation["internal"]
            .as_array()
            .is_some_and(|rows| rows.iter().all(|row| row["status"] == "passed"))
    );
    cleanup(path);
}

#[test]
fn rejects_iod_topology_and_storage_mutations() {
    for (label, mutation, finding) in [
        (
            "sop",
            Mutation::WrongSopClass,
            "twelve_lead_ecg_sop_class_uid",
        ),
        (
            "context",
            Mutation::NonEmptyAcquisitionContext,
            "twelve_lead_ecg_acquisition_context_count",
        ),
        (
            "channels",
            Mutation::WrongChannelCount,
            "twelve_lead_ecg_channel_count",
        ),
        (
            "samples",
            Mutation::WrongSampleCount,
            "twelve_lead_ecg_sample_count",
        ),
        (
            "rate",
            Mutation::WrongSamplingFrequency,
            "twelve_lead_ecg_sampling_frequency",
        ),
        (
            "unsigned",
            Mutation::UnsignedSamples,
            "twelve_lead_ecg_sample_interpretation",
        ),
        (
            "vr",
            Mutation::WrongDataVr,
            "twelve_lead_ecg_waveform_data_vr",
        ),
        (
            "padding",
            Mutation::AddedPadding,
            "twelve_lead_ecg_waveform_padding_absent",
        ),
    ] {
        assert_rejects(label, mutation, finding);
    }
}

#[test]
fn rejects_channel_order_code_sensitivity_bits_and_skew_mutations() {
    for (label, mutation, finding) in [
        (
            "channel-order",
            Mutation::ReorderedChannels,
            "twelve_lead_ecg_channel_1_ordinal",
        ),
        (
            "duplicate-code",
            Mutation::DuplicateLeadCode,
            "twelve_lead_ecg_channel_12_source_value",
        ),
        (
            "sensitivity",
            Mutation::WrongSensitivity,
            "twelve_lead_ecg_channel_1_sensitivity",
        ),
        (
            "units",
            Mutation::WrongUnits,
            "twelve_lead_ecg_channel_1_sensitivity_units_value",
        ),
        (
            "bits-stored",
            Mutation::WrongBitsStored,
            "twelve_lead_ecg_channel_1_bits_stored",
        ),
        (
            "time-skew",
            Mutation::WrongTimeSkew,
            "twelve_lead_ecg_channel_1_time_skew",
        ),
        (
            "sample-skew",
            Mutation::AddedSampleSkew,
            "twelve_lead_ecg_channel_1_sample_skew_absent",
        ),
    ] {
        assert_rejects(label, mutation, finding);
    }
}

#[test]
fn rejects_formula_interleave_hash_and_absent_module_mutations() {
    for (label, mutation, finding) in [
        (
            "payload-byte",
            Mutation::CorruptPayload,
            "twelve_lead_ecg_payload_sha256",
        ),
        (
            "sample-major",
            Mutation::SampleMajorPayload,
            "twelve_lead_ecg_formula_and_interleave",
        ),
        (
            "annotation",
            Mutation::AddedAnnotation,
            "twelve_lead_ecg_waveform_annotation_absent",
        ),
        (
            "synchronization",
            Mutation::AddedSynchronization,
            "twelve_lead_ecg_synchronization_frame_of_reference_absent",
        ),
        (
            "reference",
            Mutation::AddedReference,
            "twelve_lead_ecg_referenced_instance_absent",
        ),
        (
            "pixels",
            Mutation::AddedPixels,
            "twelve_lead_ecg_rows_absent",
        ),
    ] {
        assert_rejects(label, mutation, finding);
    }
}

fn assert_rejects(label: &str, mutation: Mutation, finding: &str) {
    let path = write_fixture(label, mutation);
    let error = validate_twelve_lead_ecg_file(&path, &expectations())
        .expect_err("mutated Twelve-lead ECG must fail")
        .to_string();
    assert!(
        error.contains(finding),
        "unexpected validation error: {error}"
    );
    cleanup(path);
}

fn expectations() -> TwelveLeadEcgExpectations<'static> {
    TwelveLeadEcgExpectations {
        sop_instance_uid: SOP_UID,
        implementation_class_uid: IMPLEMENTATION_UID,
        study_instance_uid: STUDY_UID,
        series_instance_uid: SERIES_UID,
        waveform: twelve_lead_ecg_expected_waveform(),
    }
}

fn write_fixture(label: &str, mutation: Mutation) -> PathBuf {
    let mut object = valid_object();
    apply_mutation(&mut object, mutation);
    let path = std::env::temp_dir().join(format!(
        "dicom-test-suite-twelve-lead-validation-{}-{label}.dcm",
        std::process::id()
    ));
    object
        .with_meta(
            FileMetaTableBuilder::new()
                .media_storage_sop_class_uid(if matches!(mutation, Mutation::WrongSopClass) {
                    uids::CT_IMAGE_STORAGE
                } else {
                    "1.2.840.10008.5.1.4.1.1.9.1.1"
                })
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
    match mutation {
        Mutation::None => {}
        Mutation::WrongSopClass => {
            put_str(object, tags::SOP_CLASS_UID, VR::UI, uids::CT_IMAGE_STORAGE)
        }
        Mutation::NonEmptyAcquisitionContext => {
            object.put(DataElement::new(
                tags::ACQUISITION_CONTEXT_SEQUENCE,
                VR::SQ,
                DataSetSequence::from(vec![InMemDicomObject::new_empty()]),
            ));
        }
        Mutation::AddedAnnotation => {
            object.put(DataElement::new(
                tags::WAVEFORM_ANNOTATION_SEQUENCE,
                VR::SQ,
                DataSetSequence::empty(),
            ));
        }
        Mutation::AddedSynchronization => put_str(
            object,
            tags::SYNCHRONIZATION_FRAME_OF_REFERENCE_UID,
            VR::UI,
            "2.25.919999999999999999999999999999999999999",
        ),
        Mutation::AddedReference => {
            object.put(DataElement::new(
                tags::REFERENCED_INSTANCE_SEQUENCE,
                VR::SQ,
                DataSetSequence::empty(),
            ));
        }
        Mutation::AddedPixels => {
            object.put(DataElement::new(
                tags::ROWS,
                VR::US,
                PrimitiveValue::from(1_u16),
            ));
            object.put(DataElement::new(
                tags::PIXEL_DATA,
                VR::OB,
                PrimitiveValue::U8(vec![0, 0].into()),
            ));
        }
        other => {
            let mut waveform_element = object
                .take_element(tags::WAVEFORM_SEQUENCE)
                .expect("waveform sequence");
            let waveform_sequence = waveform_element.items_mut().expect("waveform items");
            let group = waveform_sequence.first_mut().expect("group");
            match other {
                Mutation::WrongChannelCount => {
                    group.put(DataElement::new(
                        tags::NUMBER_OF_WAVEFORM_CHANNELS,
                        VR::US,
                        PrimitiveValue::from(11_u16),
                    ));
                }
                Mutation::WrongSampleCount => {
                    group.put(DataElement::new(
                        tags::NUMBER_OF_WAVEFORM_SAMPLES,
                        VR::UL,
                        PrimitiveValue::from(501_u32),
                    ));
                }
                Mutation::WrongSamplingFrequency => {
                    put_str(group, tags::SAMPLING_FREQUENCY, VR::DS, "199")
                }
                Mutation::UnsignedSamples => {
                    put_str(group, tags::WAVEFORM_SAMPLE_INTERPRETATION, VR::CS, "US")
                }
                Mutation::WrongDataVr => {
                    group.put(DataElement::new(
                        tags::WAVEFORM_DATA,
                        VR::OB,
                        PrimitiveValue::U8(locked_payload().into()),
                    ));
                }
                Mutation::CorruptPayload => {
                    let mut payload = locked_payload();
                    payload[100] ^= 1;
                    group.put(DataElement::new(
                        tags::WAVEFORM_DATA,
                        VR::OW,
                        PrimitiveValue::U8(payload.into()),
                    ));
                }
                Mutation::SampleMajorPayload => {
                    group.put(DataElement::new(
                        tags::WAVEFORM_DATA,
                        VR::OW,
                        PrimitiveValue::U8(sample_major_payload().into()),
                    ));
                }
                Mutation::AddedPadding => {
                    group.put(DataElement::new(
                        tags::WAVEFORM_PADDING_VALUE,
                        VR::OW,
                        PrimitiveValue::U8(vec![0, 0].into()),
                    ));
                }
                channel_mutation => {
                    let mut channel_element = group
                        .take_element(tags::CHANNEL_DEFINITION_SEQUENCE)
                        .expect("channel sequence");
                    let channels = channel_element.items_mut().expect("channel items");
                    match channel_mutation {
                        Mutation::ReorderedChannels => channels.swap(0, 1),
                        Mutation::DuplicateLeadCode => {
                            let mut source_element = channels[11]
                                .take_element(tags::CHANNEL_SOURCE_SEQUENCE)
                                .unwrap();
                            let source_sequence = source_element.items_mut().unwrap();
                            let code = source_sequence.first_mut().unwrap();
                            put_str(code, tags::CODE_VALUE, VR::SH, "2:1");
                            channels[11].put(source_element);
                        }
                        Mutation::WrongSensitivity => {
                            put_str(&mut channels[0], tags::CHANNEL_SENSITIVITY, VR::DS, "2")
                        }
                        Mutation::WrongUnits => {
                            let mut units_element = channels[0]
                                .take_element(tags::CHANNEL_SENSITIVITY_UNITS_SEQUENCE)
                                .unwrap();
                            let units_sequence = units_element.items_mut().unwrap();
                            let units = units_sequence.first_mut().unwrap();
                            put_str(units, tags::CODE_VALUE, VR::SH, "mV");
                            channels[0].put(units_element);
                        }
                        Mutation::WrongBitsStored => {
                            channels[0].put(DataElement::new(
                                tags::WAVEFORM_BITS_STORED,
                                VR::US,
                                PrimitiveValue::from(12_u16),
                            ));
                        }
                        Mutation::WrongTimeSkew => {
                            put_str(&mut channels[0], tags::CHANNEL_TIME_SKEW, VR::DS, "1")
                        }
                        Mutation::AddedSampleSkew => {
                            put_str(&mut channels[0], tags::CHANNEL_SAMPLE_SKEW, VR::DS, "0")
                        }
                        _ => unreachable!(),
                    }
                    group.put(channel_element);
                }
            }
            object.put(waveform_element);
        }
    }
}

fn valid_object() -> InMemDicomObject {
    let mut object = InMemDicomObject::new_empty();
    for (tag, vr, value) in [
        (tags::SOP_CLASS_UID, VR::UI, "1.2.840.10008.5.1.4.1.1.9.1.1"),
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
        (tags::SERIES_NUMBER, VR::IS, "90"),
        (tags::MANUFACTURER, VR::LO, "dicom-test-suite"),
        (tags::INSTITUTION_NAME, VR::LO, ""),
        (tags::INSTITUTION_ADDRESS, VR::ST, ""),
        (
            tags::MANUFACTURER_MODEL_NAME,
            VR::LO,
            "Native Twelve-lead ECG",
        ),
        (tags::DEVICE_SERIAL_NUMBER, VR::LO, "DTS-ECG-001"),
        (tags::SOFTWARE_VERSIONS, VR::LO, crate::PACKAGE_VERSION),
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
    object.put(DataElement::new(
        tags::WAVEFORM_SEQUENCE,
        VR::SQ,
        DataSetSequence::from(vec![valid_group()]),
    ));
    object
}

fn valid_group() -> InMemDicomObject {
    InMemDicomObject::from_element_iter([
        DataElement::new(tags::WAVEFORM_ORIGINALITY, VR::CS, "ORIGINAL"),
        DataElement::new(
            tags::NUMBER_OF_WAVEFORM_CHANNELS,
            VR::US,
            PrimitiveValue::from(12_u16),
        ),
        DataElement::new(
            tags::NUMBER_OF_WAVEFORM_SAMPLES,
            VR::UL,
            PrimitiveValue::from(500_u32),
        ),
        DataElement::new(tags::SAMPLING_FREQUENCY, VR::DS, "500"),
        DataElement::new(tags::MULTIPLEX_GROUP_LABEL, VR::SH, "RESTING_12_LEAD"),
        DataElement::new(
            tags::CHANNEL_DEFINITION_SEQUENCE,
            VR::SQ,
            DataSetSequence::from(
                TWELVE_LEAD_ECG_CHANNELS
                    .iter()
                    .map(valid_channel)
                    .collect::<Vec<_>>(),
            ),
        ),
        DataElement::new(
            tags::WAVEFORM_BITS_ALLOCATED,
            VR::US,
            PrimitiveValue::from(16_u16),
        ),
        DataElement::new(tags::WAVEFORM_SAMPLE_INTERPRETATION, VR::CS, "SS"),
        DataElement::new(
            tags::WAVEFORM_DATA,
            VR::OW,
            PrimitiveValue::U8(locked_payload().into()),
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

fn code_item(scheme: &str, value: &str, meaning: &str) -> InMemDicomObject {
    InMemDicomObject::from_element_iter([
        DataElement::new(tags::CODE_VALUE, VR::SH, value),
        DataElement::new(tags::CODING_SCHEME_DESIGNATOR, VR::SH, scheme),
        DataElement::new(tags::CODE_MEANING, VR::LO, meaning),
    ])
}

fn locked_payload() -> Vec<u8> {
    let mut payload = Vec::with_capacity(12_000);
    for sample in 0..500 {
        for channel in 0..12 {
            payload.extend_from_slice(&sample_value(sample, channel).to_le_bytes());
        }
    }
    payload
}

fn sample_major_payload() -> Vec<u8> {
    let mut payload = Vec::with_capacity(12_000);
    for channel in 0..12 {
        for sample in 0..500 {
            payload.extend_from_slice(&sample_value(sample, channel).to_le_bytes());
        }
    }
    payload
}

fn sample_value(sample: usize, channel: usize) -> i16 {
    (((sample * (channel + 1) * 37 + channel * 101) % 2001) as i32 - 1000) as i16
}

fn put_str(object: &mut InMemDicomObject, tag: dicom_core::Tag, vr: VR, value: &str) {
    object.put(DataElement::new(tag, vr, value));
}

fn cleanup(path: PathBuf) {
    let _ = fs::remove_file(path);
}
