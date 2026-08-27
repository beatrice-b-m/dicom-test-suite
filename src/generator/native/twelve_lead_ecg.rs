use dicom_core::{DataElement, PrimitiveValue, VR, value::DataSetSequence};
use dicom_dictionary_std::tags;
use dicom_object::InMemDicomObject;

pub(in crate::generator) const TWELVE_LEAD_ECG_STORAGE_UID: &str = "1.2.840.10008.5.1.4.1.1.9.1.1";
pub(in crate::generator) const TWELVE_LEAD_ECG_OUTPUT_FILE: &str = "instance.dcm";
pub(in crate::generator) const TWELVE_LEAD_ECG_SERIES_NUMBER: &str = "90";
pub(in crate::generator) const TWELVE_LEAD_ECG_CHANNEL_COUNT: u16 = 12;
pub(in crate::generator) const TWELVE_LEAD_ECG_SAMPLE_COUNT: u32 = 500;
pub(in crate::generator) const TWELVE_LEAD_ECG_SAMPLING_FREQUENCY_HZ: &str = "500";
pub(in crate::generator) const TWELVE_LEAD_ECG_BITS_ALLOCATED: u16 = 16;
pub(in crate::generator) const TWELVE_LEAD_ECG_SAMPLE_INTERPRETATION: &str = "SS";
#[cfg(test)]
pub(in crate::generator) const TWELVE_LEAD_ECG_INTERLEAVE: &str = "channel_then_sample";
pub(in crate::generator) const TWELVE_LEAD_ECG_PAYLOAD_LENGTH: usize = 12_000;
#[cfg(test)]
pub(in crate::generator) const TWELVE_LEAD_ECG_PAYLOAD_SHA256: &str =
    "98b7a9b1be25d9d64ffa75bc6e16ea80f60deed1891aeed8dfb440c1c19e6713";
#[cfg(test)]
pub(in crate::generator) const TWELVE_LEAD_ECG_CHANNEL_SHA256: [&str; 12] = [
    "7b4aee068e05c2bdff3896937c78a4c7a32f9ed2bde64d91b1d925913bf29476",
    "bd775dc70f76ea153a25832ad622b0cc26fbe6a37cf3ec6548a30965c4d17fba",
    "19d26b694df281209aa1296abbfa8f7d360e24a03a091422aba6f67663e2f3b1",
    "bb4c99d7857dbfcee5ee620bcff09b7060b61c5f2432427affc6139cb8d3cf9b",
    "230f52ed2ac57624a9a35214d7867711008dd56014f4176ce258623e5b596d3a",
    "60e167db3c081ba5bca957aba820afb519b790d048b660634d49566df88105f2",
    "cf8c73bebf746b799b1fe8aa2c908ca69bc7acc72311c64cbf4131fc8976609f",
    "0f11e5fb5105dac699fa4bcfc01c79fbe696a81db04606f39a719de57b4c7c30",
    "a41d5962abceb6dbe25f8421091ce3df6a69202c45b24ab6b0736159d15e253b",
    "d655e2cbb23d70e229ed52fedba9c45573e22729fed0a794ab690df8d7f33804",
    "005c539f9f4256a86d9e0a212b3bfe73741f99942b0677fb483c0c48db9583cd",
    "f448df95acb226c5c992363e27707a42efc3ffb974ebeff38e2a81522b57d82c",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::generator) struct TwelveLeadEcgChannel {
    pub(in crate::generator) label: &'static str,
    pub(in crate::generator) code_value: &'static str,
    pub(in crate::generator) code_meaning: &'static str,
}

pub(in crate::generator) const TWELVE_LEAD_ECG_CHANNELS: [TwelveLeadEcgChannel; 12] = [
    TwelveLeadEcgChannel {
        label: "I",
        code_value: "2:1",
        code_meaning: "Lead I",
    },
    TwelveLeadEcgChannel {
        label: "II",
        code_value: "2:2",
        code_meaning: "Lead II",
    },
    TwelveLeadEcgChannel {
        label: "III",
        code_value: "2:61",
        code_meaning: "Lead III",
    },
    TwelveLeadEcgChannel {
        label: "aVR",
        code_value: "2:62",
        code_meaning: "aVR, augmented voltage, right",
    },
    TwelveLeadEcgChannel {
        label: "aVL",
        code_value: "2:63",
        code_meaning: "aVL, augmented voltage, left",
    },
    TwelveLeadEcgChannel {
        label: "aVF",
        code_value: "2:64",
        code_meaning: "aVF, augmented voltage, foot",
    },
    TwelveLeadEcgChannel {
        label: "V1",
        code_value: "2:3",
        code_meaning: "Lead V1",
    },
    TwelveLeadEcgChannel {
        label: "V2",
        code_value: "2:4",
        code_meaning: "Lead V2",
    },
    TwelveLeadEcgChannel {
        label: "V3",
        code_value: "2:5",
        code_meaning: "Lead V3",
    },
    TwelveLeadEcgChannel {
        label: "V4",
        code_value: "2:6",
        code_meaning: "Lead V4",
    },
    TwelveLeadEcgChannel {
        label: "V5",
        code_value: "2:7",
        code_meaning: "Lead V5",
    },
    TwelveLeadEcgChannel {
        label: "V6",
        code_value: "2:8",
        code_meaning: "Lead V6",
    },
];

const _: () = assert!(TWELVE_LEAD_ECG_CHANNELS.len() == TWELVE_LEAD_ECG_CHANNEL_COUNT as usize);
const _: () = assert!(
    TWELVE_LEAD_ECG_PAYLOAD_LENGTH
        == TWELVE_LEAD_ECG_CHANNEL_COUNT as usize
            * TWELVE_LEAD_ECG_SAMPLE_COUNT as usize
            * size_of::<i16>()
);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::generator) struct TwelveLeadEcgInput<'a> {
    pub(in crate::generator) study_instance_uid: &'a str,
    pub(in crate::generator) series_instance_uid: &'a str,
    pub(in crate::generator) sop_instance_uid: &'a str,
}

pub(in crate::generator) fn build_twelve_lead_ecg(
    input: TwelveLeadEcgInput<'_>,
) -> Result<InMemDicomObject, String> {
    validate_input(input)?;

    let mut object = InMemDicomObject::new_empty();
    put_str(
        &mut object,
        tags::SOP_CLASS_UID,
        VR::UI,
        TWELVE_LEAD_ECG_STORAGE_UID,
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
        TWELVE_LEAD_ECG_SERIES_NUMBER,
    );

    put_str(&mut object, tags::MANUFACTURER, VR::LO, "dicom-test-suite");
    put_str(&mut object, tags::INSTITUTION_NAME, VR::LO, "");
    put_str(&mut object, tags::INSTITUTION_ADDRESS, VR::ST, "");
    put_str(
        &mut object,
        tags::MANUFACTURER_MODEL_NAME,
        VR::LO,
        "Native Twelve-lead ECG",
    );
    put_str(
        &mut object,
        tags::DEVICE_SERIAL_NUMBER,
        VR::LO,
        "DTS-ECG-001",
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
        DataSetSequence::from(vec![waveform_group()]),
    ));
    Ok(object)
}

pub(in crate::generator) fn twelve_lead_ecg_sample(sample: usize, channel: usize) -> i16 {
    (((sample * (channel + 1) * 37 + channel * 101) % 2001) as i32 - 1000) as i16
}

pub(in crate::generator) fn twelve_lead_ecg_payload() -> Vec<u8> {
    let mut payload = Vec::with_capacity(TWELVE_LEAD_ECG_PAYLOAD_LENGTH);
    for sample in 0..TWELVE_LEAD_ECG_SAMPLE_COUNT as usize {
        for channel in 0..TWELVE_LEAD_ECG_CHANNEL_COUNT as usize {
            payload.extend_from_slice(&twelve_lead_ecg_sample(sample, channel).to_le_bytes());
        }
    }
    payload
}

fn validate_input(input: TwelveLeadEcgInput<'_>) -> Result<(), String> {
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

fn waveform_group() -> InMemDicomObject {
    InMemDicomObject::from_element_iter([
        DataElement::new(tags::WAVEFORM_ORIGINALITY, VR::CS, "ORIGINAL"),
        DataElement::new(
            tags::NUMBER_OF_WAVEFORM_CHANNELS,
            VR::US,
            PrimitiveValue::from(TWELVE_LEAD_ECG_CHANNEL_COUNT),
        ),
        DataElement::new(
            tags::NUMBER_OF_WAVEFORM_SAMPLES,
            VR::UL,
            PrimitiveValue::from(TWELVE_LEAD_ECG_SAMPLE_COUNT),
        ),
        DataElement::new(
            tags::SAMPLING_FREQUENCY,
            VR::DS,
            TWELVE_LEAD_ECG_SAMPLING_FREQUENCY_HZ,
        ),
        DataElement::new(tags::MULTIPLEX_GROUP_LABEL, VR::SH, "RESTING_12_LEAD"),
        DataElement::new(
            tags::CHANNEL_DEFINITION_SEQUENCE,
            VR::SQ,
            DataSetSequence::from(
                TWELVE_LEAD_ECG_CHANNELS
                    .iter()
                    .enumerate()
                    .map(|(index, channel)| channel_definition(index + 1, *channel))
                    .collect::<Vec<_>>(),
            ),
        ),
        DataElement::new(
            tags::WAVEFORM_BITS_ALLOCATED,
            VR::US,
            PrimitiveValue::from(TWELVE_LEAD_ECG_BITS_ALLOCATED),
        ),
        DataElement::new(
            tags::WAVEFORM_SAMPLE_INTERPRETATION,
            VR::CS,
            TWELVE_LEAD_ECG_SAMPLE_INTERPRETATION,
        ),
        DataElement::new(
            tags::WAVEFORM_DATA,
            VR::OW,
            PrimitiveValue::U8(twelve_lead_ecg_payload().into()),
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
            PrimitiveValue::from(TWELVE_LEAD_ECG_BITS_ALLOCATED),
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
