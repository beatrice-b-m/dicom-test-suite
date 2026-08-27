use dicom_core::{DataElement, PrimitiveValue, VR, value::DataSetSequence};
use dicom_dictionary_std::tags;
use dicom_object::InMemDicomObject;

use super::twelve_lead_ecg::{TWELVE_LEAD_ECG_CHANNELS, TwelveLeadEcgChannel};

pub(in crate::generator) const GENERAL_ECG_STORAGE_UID: &str = "1.2.840.10008.5.1.4.1.1.9.1.2";
pub(in crate::generator) const GENERAL_ECG_OUTPUT_FILE: &str = "instance.dcm";
pub(in crate::generator) const GENERAL_ECG_SERIES_NUMBER: &str = "91";
pub(in crate::generator) const GENERAL_ECG_BITS_ALLOCATED: u16 = 16;
pub(in crate::generator) const GENERAL_ECG_SAMPLE_INTERPRETATION: &str = "SS";
pub(in crate::generator) const GENERAL_ECG_TOTAL_CHANNEL_COUNT: usize = 16;
pub(in crate::generator) const GENERAL_ECG_TOTAL_PAYLOAD_LENGTH: usize = 56_000;
pub(in crate::generator) const GENERAL_ECG_AGGREGATE_SHA256: &str =
    "c450f55360d6c07394600e4c0f71f951565cd0e1699edfbbb52f660221c6abea";

const AUXILIARY_CHANNELS: [TwelveLeadEcgChannel; 4] = [
    TwelveLeadEcgChannel {
        label: "A1",
        code_value: "2:75",
        code_meaning: "Auxiliary unipolar lead 1",
    },
    TwelveLeadEcgChannel {
        label: "A2",
        code_value: "2:76",
        code_meaning: "Auxiliary unipolar lead 2",
    },
    TwelveLeadEcgChannel {
        label: "A3",
        code_value: "2:77",
        code_meaning: "Auxiliary unipolar lead 3",
    },
    TwelveLeadEcgChannel {
        label: "A4",
        code_value: "2:78",
        code_meaning: "Auxiliary unipolar lead 4",
    },
];

const STANDARD_CHANNEL_SHA256: [&str; 12] = [
    "3211bada5580e8bd9c5a2934deb231122706b00aa92f8cdc78480c03b2352197",
    "8f66471e35940851acdd9ea55b422c738bf50ea7971822deed0edca1980e1ea2",
    "9652eb91f4f73f2654c922048a1a8c8731a08062eecd6f5b373256831d0e82b0",
    "97fb26e75907437a705e4e28eb6492d51020570a23265bdf765aca3c4e7b2708",
    "c9776b85b3bda6adef798d33d3c7c95d64a1a7d5bf525866ccf7b0cf5fc3209e",
    "95871f48d729a001eeb1543b36a27059916df360e04838fd322d006661bafb44",
    "04513ee1f1d5803f3f53093f016a606a7fa874c5af8d2651749b909b93392366",
    "c12790f5b1f233662a0a1c3f266cd2abb15af5a75b39258ff961e9b4afaf7913",
    "750913ccad5eb7ec8d8199451e6eb9aa41357eb21d2a0dac6ba75dce4e5708bd",
    "218d5f967ef253722359fee1846485331c63de9330af1f9fad183d779a196cca",
    "9027ec7a0fc7fea3d8236a16a5aa6f265ff20e18a2575f99e61807e102fb3d81",
    "9280ad35672b82a7847d3ccabadd4d85a94be3d39d0a836191384571f0a23ab6",
];

const AUXILIARY_CHANNEL_SHA256: [&str; 4] = [
    "5da46776ad84a78eb0c16066cb8ac7d5e05ca6ad87170264b227c71261def284",
    "7bd73425422f4e79504b55932040e481ccdfafecabe1dba613ee36074a51b9e3",
    "e56dad9647dfa50a10b40d244e29eaedbf23d97a558901f46fbccc07ad1a1766",
    "e1b68207c92fe2cc4c6765fc097668f2600eeda152eb5a1d6f0444f4c9e36fbc",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::generator) struct GeneralEcgGroup {
    pub(in crate::generator) label: &'static str,
    pub(in crate::generator) channels: &'static [TwelveLeadEcgChannel],
    pub(in crate::generator) sample_count: u32,
    pub(in crate::generator) sampling_frequency_hz: &'static str,
    pub(in crate::generator) payload_length: usize,
    pub(in crate::generator) payload_sha256: &'static str,
    pub(in crate::generator) channel_sha256: &'static [&'static str],
}

pub(in crate::generator) const GENERAL_ECG_GROUPS: [GeneralEcgGroup; 2] = [
    GeneralEcgGroup {
        label: "STD12_250HZ",
        channels: &TWELVE_LEAD_ECG_CHANNELS,
        sample_count: 1_000,
        sampling_frequency_hz: "250",
        payload_length: 24_000,
        payload_sha256: "e4bfb8a3290d9057fa5f5935fa6960ce2a44a07f18991d28c190522739008dbb",
        channel_sha256: &STANDARD_CHANNEL_SHA256,
    },
    GeneralEcgGroup {
        label: "AUX4_1000HZ",
        channels: &AUXILIARY_CHANNELS,
        sample_count: 4_000,
        sampling_frequency_hz: "1000",
        payload_length: 32_000,
        payload_sha256: "5b201d4fa7274ba36d6f7387c3d0217e1b5da161a915f983c2b63b995dde7bbe",
        channel_sha256: &AUXILIARY_CHANNEL_SHA256,
    },
];

const _: () = assert!(GENERAL_ECG_GROUPS.len() == 2);
const _: () = assert!(
    GENERAL_ECG_GROUPS[0].channels.len() + GENERAL_ECG_GROUPS[1].channels.len()
        == GENERAL_ECG_TOTAL_CHANNEL_COUNT
);
const _: () = assert!(
    GENERAL_ECG_GROUPS[0].payload_length + GENERAL_ECG_GROUPS[1].payload_length
        == GENERAL_ECG_TOTAL_PAYLOAD_LENGTH
);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::generator) struct GeneralEcgInput<'a> {
    pub(in crate::generator) study_instance_uid: &'a str,
    pub(in crate::generator) series_instance_uid: &'a str,
    pub(in crate::generator) sop_instance_uid: &'a str,
}

pub(in crate::generator) fn build_general_ecg(
    input: GeneralEcgInput<'_>,
) -> Result<InMemDicomObject, String> {
    validate_input(input)?;

    let mut object = InMemDicomObject::new_empty();
    put_str(
        &mut object,
        tags::SOP_CLASS_UID,
        VR::UI,
        GENERAL_ECG_STORAGE_UID,
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
    put_str(&mut object, tags::STUDY_ID, VR::SH, "DTS-ECG");
    put_str(&mut object, tags::ACCESSION_NUMBER, VR::SH, "");

    put_str(&mut object, tags::MODALITY, VR::CS, "ECG");
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
        GENERAL_ECG_SERIES_NUMBER,
    );

    put_str(&mut object, tags::MANUFACTURER, VR::LO, "dicom-test-suite");
    put_str(&mut object, tags::INSTITUTION_NAME, VR::LO, "");
    put_str(&mut object, tags::INSTITUTION_ADDRESS, VR::ST, "");
    put_str(
        &mut object,
        tags::MANUFACTURER_MODEL_NAME,
        VR::LO,
        "Native General ECG",
    );
    put_str(
        &mut object,
        tags::DEVICE_SERIAL_NUMBER,
        VR::LO,
        "DTS-GECG-001",
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
    put_str(
        &mut object,
        tags::ACQUISITION_DATE_TIME,
        VR::DT,
        "20260101000000",
    );
    object.put(DataElement::new(
        tags::ACQUISITION_CONTEXT_SEQUENCE,
        VR::SQ,
        DataSetSequence::empty(),
    ));

    object.put(DataElement::new(
        tags::WAVEFORM_SEQUENCE,
        VR::SQ,
        DataSetSequence::from(
            GENERAL_ECG_GROUPS
                .iter()
                .enumerate()
                .map(|(group_index, group)| waveform_group(group_index, *group))
                .collect::<Vec<_>>(),
        ),
    ));
    Ok(object)
}

pub(in crate::generator) fn general_ecg_sample(group: usize, sample: usize, channel: usize) -> i16 {
    (((sample * (channel + 1) * (group + 1) * 37 + channel * 101 + group * 307) % 2001) as i32
        - 1000) as i16
}

pub(in crate::generator) fn general_ecg_group_payload(group_index: usize) -> Vec<u8> {
    let group = GENERAL_ECG_GROUPS[group_index];
    let mut payload = Vec::with_capacity(group.payload_length);
    for sample in 0..group.sample_count as usize {
        for channel in 0..group.channels.len() {
            payload
                .extend_from_slice(&general_ecg_sample(group_index, sample, channel).to_le_bytes());
        }
    }
    payload
}

pub(in crate::generator) fn general_ecg_ordered_payload() -> Vec<u8> {
    let mut payload = Vec::with_capacity(GENERAL_ECG_TOTAL_PAYLOAD_LENGTH);
    for group_index in 0..GENERAL_ECG_GROUPS.len() {
        payload.extend_from_slice(&general_ecg_group_payload(group_index));
    }
    payload
}

fn validate_input(input: GeneralEcgInput<'_>) -> Result<(), String> {
    let uids = [
        ("Study Instance UID", input.study_instance_uid),
        ("Series Instance UID", input.series_instance_uid),
        ("SOP Instance UID", input.sop_instance_uid),
    ];
    for (name, value) in uids {
        if value.is_empty() {
            return Err(format!("{name} must not be empty"));
        }
    }
    if input.study_instance_uid == input.series_instance_uid
        || input.study_instance_uid == input.sop_instance_uid
        || input.series_instance_uid == input.sop_instance_uid
    {
        return Err("Study, Series, and SOP Instance UIDs must be distinct".to_string());
    }
    Ok(())
}

fn waveform_group(group_index: usize, group: GeneralEcgGroup) -> InMemDicomObject {
    InMemDicomObject::from_element_iter([
        DataElement::new(tags::WAVEFORM_ORIGINALITY, VR::CS, "ORIGINAL"),
        DataElement::new(
            tags::NUMBER_OF_WAVEFORM_CHANNELS,
            VR::US,
            PrimitiveValue::from(group.channels.len() as u16),
        ),
        DataElement::new(
            tags::NUMBER_OF_WAVEFORM_SAMPLES,
            VR::UL,
            PrimitiveValue::from(group.sample_count),
        ),
        DataElement::new(
            tags::SAMPLING_FREQUENCY,
            VR::DS,
            group.sampling_frequency_hz,
        ),
        DataElement::new(tags::MULTIPLEX_GROUP_LABEL, VR::SH, group.label),
        DataElement::new(
            tags::CHANNEL_DEFINITION_SEQUENCE,
            VR::SQ,
            DataSetSequence::from(
                group
                    .channels
                    .iter()
                    .enumerate()
                    .map(|(index, channel)| channel_definition(index + 1, *channel))
                    .collect::<Vec<_>>(),
            ),
        ),
        DataElement::new(
            tags::WAVEFORM_BITS_ALLOCATED,
            VR::US,
            PrimitiveValue::from(GENERAL_ECG_BITS_ALLOCATED),
        ),
        DataElement::new(
            tags::WAVEFORM_SAMPLE_INTERPRETATION,
            VR::CS,
            GENERAL_ECG_SAMPLE_INTERPRETATION,
        ),
        DataElement::new(
            tags::WAVEFORM_DATA,
            VR::OW,
            PrimitiveValue::U8(general_ecg_group_payload(group_index).into()),
        ),
    ])
}

fn channel_definition(ordinal: usize, channel: TwelveLeadEcgChannel) -> InMemDicomObject {
    InMemDicomObject::from_element_iter([
        DataElement::new(tags::WAVEFORM_CHANNEL_NUMBER, VR::IS, ordinal.to_string()),
        DataElement::new(tags::CHANNEL_LABEL, VR::SH, channel.label),
        DataElement::new(
            tags::CHANNEL_SOURCE_SEQUENCE,
            VR::SQ,
            DataSetSequence::from(vec![code_item(
                "MDC",
                channel.code_value,
                channel.code_meaning,
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
            PrimitiveValue::from(GENERAL_ECG_BITS_ALLOCATED),
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

fn put_str(object: &mut InMemDicomObject, tag: dicom_core::Tag, vr: VR, value: &str) {
    object.put(DataElement::new(tag, vr, value));
}
