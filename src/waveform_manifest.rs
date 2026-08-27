use serde::Serialize;

pub(crate) const TWELVE_LEAD_ECG_PAYLOAD_SHA256: &str =
    "98b7a9b1be25d9d64ffa75bc6e16ea80f60deed1891aeed8dfb440c1c19e6713";

pub(crate) const TWELVE_LEAD_ECG_CHANNEL_SHA256: [&str; 12] = [
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

pub(crate) const GENERAL_ECG_GROUP_PAYLOAD_SHA256: [&str; 2] = [
    "e4bfb8a3290d9057fa5f5935fa6960ce2a44a07f18991d28c190522739008dbb",
    "5b201d4fa7274ba36d6f7387c3d0217e1b5da161a915f983c2b63b995dde7bbe",
];

pub(crate) const GENERAL_ECG_AGGREGATE_PAYLOAD_SHA256: &str =
    "c450f55360d6c07394600e4c0f71f951565cd0e1699edfbbb52f660221c6abea";

const GENERAL_ECG_STANDARD_CHANNEL_SHA256: [&str; 12] = [
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

const GENERAL_ECG_AUXILIARY_CHANNEL_SHA256: [&str; 4] = [
    "5da46776ad84a78eb0c16066cb8ac7d5e05ca6ad87170264b227c71261def284",
    "7bd73425422f4e79504b55932040e481ccdfafecabe1dba613ee36074a51b9e3",
    "e56dad9647dfa50a10b40d244e29eaedbf23d97a558901f46fbccc07ad1a1766",
    "e1b68207c92fe2cc4c6765fc097668f2600eeda152eb5a1d6f0444f4c9e36fbc",
];

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub(crate) struct ExpectedWaveform<'a> {
    pub iod_kind: &'a str,
    pub sop_class_uid: &'a str,
    pub iod_name: &'a str,
    pub modality: &'a str,
    pub transfer_syntax_uid: &'a str,
    pub acquisition_context_items: u8,
    pub multiplex_groups: &'a [ExpectedMultiplexGroup<'a>],
    pub aggregate: ExpectedWaveformAggregate<'a>,
    pub absent_content: ExpectedWaveformAbsentContent,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub(crate) struct ExpectedMultiplexGroup<'a> {
    pub ordinal: u8,
    pub originality: &'a str,
    pub label: &'a str,
    pub channel_count: u8,
    pub samples_per_channel: u16,
    pub sampling_frequency_hz: u16,
    pub duration_seconds: u8,
    pub simultaneous_sampling: bool,
    pub channels: &'a [ExpectedWaveformChannel<'a>],
    pub storage: ExpectedWaveformStorage<'a>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub(crate) struct ExpectedWaveformAggregate<'a> {
    pub group_count: u8,
    pub total_channel_count: u8,
    pub common_duration_seconds: u8,
    pub total_payload_length_bytes: u16,
    pub group_payload_sha256: &'a [&'a str],
    pub aggregate_payload_sha256: &'a str,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub(crate) struct ExpectedWaveformChannel<'a> {
    pub ordinal: u8,
    pub label: &'a str,
    pub source: ExpectedWaveformCode<'a>,
    pub sensitivity: u8,
    pub sensitivity_units: ExpectedWaveformCode<'a>,
    pub sensitivity_correction_factor: u8,
    pub baseline: i8,
    pub bits_stored: u8,
    pub time_skew_seconds: u8,
    pub sample_skew_absent: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub(crate) struct ExpectedWaveformCode<'a> {
    pub code_value: &'a str,
    pub coding_scheme_designator: &'a str,
    pub code_meaning: &'a str,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub(crate) struct ExpectedWaveformStorage<'a> {
    pub bits_allocated: u8,
    pub sample_interpretation: &'a str,
    pub data_vr: &'a str,
    pub byte_order: &'a str,
    pub interleave_order: &'a str,
    pub payload_length_bytes: u16,
    pub payload_sha256: &'a str,
    pub channel_sha256: &'a [&'a str],
    pub sample_value_formula: &'a str,
    pub sample_min: i16,
    pub sample_max: i16,
    pub waveform_padding_value_absent: bool,
    pub value_field_padding_bytes: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub(crate) struct ExpectedWaveformAbsentContent {
    pub annotation_module: bool,
    pub synchronization_module: bool,
    pub references: bool,
    pub image: bool,
    pub pixel_data: bool,
}

const MICROVOLT: ExpectedWaveformCode<'static> = ExpectedWaveformCode {
    code_value: "uV",
    coding_scheme_designator: "UCUM",
    code_meaning: "microvolt",
};

pub(crate) const TWELVE_LEAD_ECG_CHANNELS: [ExpectedWaveformChannel<'static>; 12] = [
    channel(1, "I", "2:1", "Lead I"),
    channel(2, "II", "2:2", "Lead II"),
    channel(3, "III", "2:61", "Lead III"),
    channel(4, "aVR", "2:62", "aVR, augmented voltage, right"),
    channel(5, "aVL", "2:63", "aVL, augmented voltage, left"),
    channel(6, "aVF", "2:64", "aVF, augmented voltage, foot"),
    channel(7, "V1", "2:3", "Lead V1"),
    channel(8, "V2", "2:4", "Lead V2"),
    channel(9, "V3", "2:5", "Lead V3"),
    channel(10, "V4", "2:6", "Lead V4"),
    channel(11, "V5", "2:7", "Lead V5"),
    channel(12, "V6", "2:8", "Lead V6"),
];

const GENERAL_ECG_AUXILIARY_CHANNELS: [ExpectedWaveformChannel<'static>; 4] = [
    channel(1, "A1", "2:75", "Auxiliary unipolar lead 1"),
    channel(2, "A2", "2:76", "Auxiliary unipolar lead 2"),
    channel(3, "A3", "2:77", "Auxiliary unipolar lead 3"),
    channel(4, "A4", "2:78", "Auxiliary unipolar lead 4"),
];

const TWELVE_LEAD_ECG_GROUP_PAYLOAD_SHA256: [&str; 1] = [TWELVE_LEAD_ECG_PAYLOAD_SHA256];

const TWELVE_LEAD_ECG_MULTIPLEX_GROUPS: [ExpectedMultiplexGroup<'static>; 1] =
    [ExpectedMultiplexGroup {
        ordinal: 1,
        originality: "ORIGINAL",
        label: "RESTING_12_LEAD",
        channel_count: 12,
        samples_per_channel: 500,
        sampling_frequency_hz: 500,
        duration_seconds: 1,
        simultaneous_sampling: true,
        channels: &TWELVE_LEAD_ECG_CHANNELS,
        storage: ExpectedWaveformStorage {
            bits_allocated: 16,
            sample_interpretation: "SS",
            data_vr: "OW",
            byte_order: "little_endian",
            interleave_order: "channel_then_sample",
            payload_length_bytes: 12_000,
            payload_sha256: TWELVE_LEAD_ECG_PAYLOAD_SHA256,
            channel_sha256: &TWELVE_LEAD_ECG_CHANNEL_SHA256,
            sample_value_formula: "((s * (c + 1) * 37 + c * 101) mod 2001) - 1000",
            sample_min: -1000,
            sample_max: 1000,
            waveform_padding_value_absent: true,
            value_field_padding_bytes: 0,
        },
    }];

const GENERAL_ECG_SAMPLE_FORMULA: &str =
    "((s * (c + 1) * (g + 1) * 37 + c * 101 + g * 307) mod 2001) - 1000";

const GENERAL_ECG_MULTIPLEX_GROUPS: [ExpectedMultiplexGroup<'static>; 2] = [
    ExpectedMultiplexGroup {
        ordinal: 1,
        originality: "ORIGINAL",
        label: "STD12_250HZ",
        channel_count: 12,
        samples_per_channel: 1_000,
        sampling_frequency_hz: 250,
        duration_seconds: 4,
        simultaneous_sampling: true,
        channels: &TWELVE_LEAD_ECG_CHANNELS,
        storage: ExpectedWaveformStorage {
            bits_allocated: 16,
            sample_interpretation: "SS",
            data_vr: "OW",
            byte_order: "little_endian",
            interleave_order: "channel_then_sample",
            payload_length_bytes: 24_000,
            payload_sha256: GENERAL_ECG_GROUP_PAYLOAD_SHA256[0],
            channel_sha256: &GENERAL_ECG_STANDARD_CHANNEL_SHA256,
            sample_value_formula: GENERAL_ECG_SAMPLE_FORMULA,
            sample_min: -1000,
            sample_max: 1000,
            waveform_padding_value_absent: true,
            value_field_padding_bytes: 0,
        },
    },
    ExpectedMultiplexGroup {
        ordinal: 2,
        originality: "ORIGINAL",
        label: "AUX4_1000HZ",
        channel_count: 4,
        samples_per_channel: 4_000,
        sampling_frequency_hz: 1_000,
        duration_seconds: 4,
        simultaneous_sampling: true,
        channels: &GENERAL_ECG_AUXILIARY_CHANNELS,
        storage: ExpectedWaveformStorage {
            bits_allocated: 16,
            sample_interpretation: "SS",
            data_vr: "OW",
            byte_order: "little_endian",
            interleave_order: "channel_then_sample",
            payload_length_bytes: 32_000,
            payload_sha256: GENERAL_ECG_GROUP_PAYLOAD_SHA256[1],
            channel_sha256: &GENERAL_ECG_AUXILIARY_CHANNEL_SHA256,
            sample_value_formula: GENERAL_ECG_SAMPLE_FORMULA,
            sample_min: -1000,
            sample_max: 1000,
            waveform_padding_value_absent: true,
            value_field_padding_bytes: 0,
        },
    },
];

const fn channel(
    ordinal: u8,
    label: &'static str,
    code_value: &'static str,
    code_meaning: &'static str,
) -> ExpectedWaveformChannel<'static> {
    ExpectedWaveformChannel {
        ordinal,
        label,
        source: ExpectedWaveformCode {
            code_value,
            coding_scheme_designator: "MDC",
            code_meaning,
        },
        sensitivity: 1,
        sensitivity_units: MICROVOLT,
        sensitivity_correction_factor: 1,
        baseline: 0,
        bits_stored: 16,
        time_skew_seconds: 0,
        sample_skew_absent: true,
    }
}

pub(crate) fn twelve_lead_ecg_expected_waveform() -> ExpectedWaveform<'static> {
    ExpectedWaveform {
        iod_kind: "twelve_lead_ecg",
        sop_class_uid: "1.2.840.10008.5.1.4.1.1.9.1.1",
        iod_name: "12-lead ECG Waveform",
        modality: "ECG",
        transfer_syntax_uid: "1.2.840.10008.1.2.1",
        acquisition_context_items: 0,
        multiplex_groups: &TWELVE_LEAD_ECG_MULTIPLEX_GROUPS,
        aggregate: ExpectedWaveformAggregate {
            group_count: 1,
            total_channel_count: 12,
            common_duration_seconds: 1,
            total_payload_length_bytes: 12_000,
            group_payload_sha256: &TWELVE_LEAD_ECG_GROUP_PAYLOAD_SHA256,
            aggregate_payload_sha256: TWELVE_LEAD_ECG_PAYLOAD_SHA256,
        },
        absent_content: ExpectedWaveformAbsentContent {
            annotation_module: true,
            synchronization_module: true,
            references: true,
            image: true,
            pixel_data: true,
        },
    }
}

pub(crate) fn general_ecg_expected_waveform() -> ExpectedWaveform<'static> {
    ExpectedWaveform {
        iod_kind: "general_ecg",
        sop_class_uid: "1.2.840.10008.5.1.4.1.1.9.1.2",
        iod_name: "General ECG Waveform",
        modality: "ECG",
        transfer_syntax_uid: "1.2.840.10008.1.2.1",
        acquisition_context_items: 0,
        multiplex_groups: &GENERAL_ECG_MULTIPLEX_GROUPS,
        aggregate: ExpectedWaveformAggregate {
            group_count: 2,
            total_channel_count: 16,
            common_duration_seconds: 4,
            total_payload_length_bytes: 56_000,
            group_payload_sha256: &GENERAL_ECG_GROUP_PAYLOAD_SHA256,
            aggregate_payload_sha256: GENERAL_ECG_AGGREGATE_PAYLOAD_SHA256,
        },
        absent_content: ExpectedWaveformAbsentContent {
            annotation_module: true,
            synchronization_module: true,
            references: true,
            image: true,
            pixel_data: true,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn twelve_lead_contract_serializes_exact_locked_values() {
        let value = serde_json::to_value(twelve_lead_ecg_expected_waveform())
            .expect("waveform expectation should serialize");

        assert!(value.get("multiplex_group").is_none());
        assert!(value.get("channels").is_none());
        assert!(value.get("storage").is_none());
        assert_eq!(value["multiplex_groups"].as_array().map(Vec::len), Some(1));
        let group = &value["multiplex_groups"][0];
        assert_eq!(group["ordinal"], 1);
        assert_eq!(group["channels"].as_array().map(Vec::len), Some(12));
        assert_eq!(group["channels"][0]["source"]["code_value"], "2:1");
        assert_eq!(group["channels"][11]["source"]["code_value"], "2:8");
        assert_eq!(group["storage"]["payload_length_bytes"], 12_000);
        assert_eq!(
            group["storage"]["payload_sha256"],
            TWELVE_LEAD_ECG_PAYLOAD_SHA256
        );
        assert_eq!(
            group["storage"]["channel_sha256"].as_array().map(Vec::len),
            Some(12)
        );
        assert_eq!(group["simultaneous_sampling"], true);
        assert_eq!(value["aggregate"]["group_count"], 1);
        assert_eq!(value["aggregate"]["total_channel_count"], 12);
        assert_eq!(value["aggregate"]["common_duration_seconds"], 1);
        assert_eq!(value["aggregate"]["total_payload_length_bytes"], 12_000);
        assert_eq!(
            value["aggregate"]["group_payload_sha256"],
            serde_json::json!([TWELVE_LEAD_ECG_PAYLOAD_SHA256])
        );
        assert_eq!(
            value["aggregate"]["aggregate_payload_sha256"],
            TWELVE_LEAD_ECG_PAYLOAD_SHA256
        );
        assert_eq!(value["absent_content"]["pixel_data"], true);
    }

    #[test]
    fn general_ecg_contract_serializes_exact_ordered_groups() {
        let value = serde_json::to_value(general_ecg_expected_waveform())
            .expect("General ECG expectation should serialize");
        let groups = value["multiplex_groups"].as_array().expect("groups");

        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0]["ordinal"], 1);
        assert_eq!(groups[0]["label"], "STD12_250HZ");
        assert_eq!(groups[0]["channel_count"], 12);
        assert_eq!(groups[0]["samples_per_channel"], 1_000);
        assert_eq!(groups[0]["sampling_frequency_hz"], 250);
        assert_eq!(groups[0]["storage"]["payload_length_bytes"], 24_000);
        assert_eq!(groups[0]["channels"][0]["source"]["code_value"], "2:1");
        assert_eq!(groups[1]["ordinal"], 2);
        assert_eq!(groups[1]["label"], "AUX4_1000HZ");
        assert_eq!(groups[1]["channel_count"], 4);
        assert_eq!(groups[1]["samples_per_channel"], 4_000);
        assert_eq!(groups[1]["sampling_frequency_hz"], 1_000);
        assert_eq!(groups[1]["storage"]["payload_length_bytes"], 32_000);
        assert_eq!(groups[1]["channels"][0]["source"]["code_value"], "2:75");
        assert_eq!(groups[1]["channels"][3]["source"]["code_value"], "2:78");
        assert_eq!(value["aggregate"]["group_count"], 2);
        assert_eq!(value["aggregate"]["total_channel_count"], 16);
        assert_eq!(value["aggregate"]["total_payload_length_bytes"], 56_000);
        assert_eq!(
            value["aggregate"]["aggregate_payload_sha256"],
            GENERAL_ECG_AGGREGATE_PAYLOAD_SHA256
        );
    }
}
