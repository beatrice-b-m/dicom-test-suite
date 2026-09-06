use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::recipes::{
    EncapsulatedPayload, EncapsulatedPayloadPlanInput, WaveformPlanInput, WaveformProjection,
};
use crate::sha256_hex;

use super::validation::SpecializedValidationError;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "family", rename_all = "snake_case", deny_unknown_fields)]
pub enum SpecializedManifestProjection {
    Waveform(WaveformManifestProjection),
    EncapsulatedPayload(EncapsulatedPayloadManifestProjection),
}

impl SpecializedManifestProjection {
    /// Family-owned legacy fields. Common path, identity, DICOM, UID, output,
    /// standards, and validation fields remain owned by the shared projector.
    pub fn legacy_fields(&self) -> Value {
        match self {
            Self::Waveform(value) => value.legacy_fields(),
            Self::EncapsulatedPayload(value) => value.legacy_fields(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WaveformManifestProjection {
    pub recipe_id: String,
    pub recipe_version: String,
    pub recipe_parameters: WaveformRecipeParameters,
    pub expected_capabilities: Vec<String>,
    pub expected_semantics: WaveformExpectedSemantics,
    pub expected_visual_pattern: String,
    pub expected_waveform: ExpectedWaveform,
    pub known_stressors: Vec<String>,
}

impl WaveformManifestProjection {
    pub fn legacy_fields(&self) -> Value {
        json!({
            "recipe": {
                "recipe_id": self.recipe_id,
                "recipe_version": self.recipe_version,
                "recipe_parameters": self.recipe_parameters,
            },
            "expected_capabilities": self.expected_capabilities,
            "expected_semantics": self.expected_semantics,
            "expected_waveform": self.expected_waveform,
            "expected_visual_checks": {"pattern": self.expected_visual_pattern},
            "known_stressors": self.known_stressors,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum WaveformRecipeParameters {
    Caller {
        waveform_capability_version: String,
        waveform_contract: WaveformPlanInput,
    },
    TwelveLead {
        multiplex_group_label: String,
        channel_count: u64,
        samples_per_channel: u32,
        sampling_frequency_hz: u64,
        sample_formula: String,
    },
    General {
        multiplex_groups: Vec<WaveformRecipeGroup>,
        total_channel_count: u64,
        total_payload_length_bytes: u64,
        aggregate_payload_sha256: String,
        sample_formula: String,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WaveformRecipeGroup {
    pub label: String,
    pub channel_count: u64,
    pub samples_per_channel: u32,
    pub sampling_frequency_hz: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum WaveformExpectedSemantics {
    TwelveLead {
        synthetic_data: String,
        simultaneous_sampling: bool,
        one_second_duration: bool,
        pixel_data_absent: bool,
        diagnostic_use: bool,
    },
    General {
        synthetic_data: String,
        simultaneous_sampling_within_groups: bool,
        common_duration_seconds: u32,
        cross_group_synchronization_asserted: bool,
        pixel_data_absent: bool,
        diagnostic_use: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExpectedWaveform {
    pub iod_kind: String,
    pub sop_class_uid: String,
    pub iod_name: String,
    pub modality: String,
    pub transfer_syntax_uid: String,
    pub acquisition_context_items: u8,
    pub multiplex_groups: Vec<ExpectedMultiplexGroup>,
    pub aggregate: ExpectedWaveformAggregate,
    pub absent_content: ExpectedWaveformAbsentContent,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExpectedMultiplexGroup {
    pub ordinal: u64,
    pub originality: String,
    pub label: String,
    pub channel_count: u64,
    pub samples_per_channel: u32,
    pub sampling_frequency_hz: u64,
    pub duration_seconds: u64,
    pub simultaneous_sampling: bool,
    pub channels: Vec<ExpectedWaveformChannel>,
    pub storage: ExpectedWaveformStorage,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum WaveformDecimalProjection {
    HistoricalNumber(i64),
    CallerLexeme(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExpectedWaveformChannel {
    pub ordinal: u64,
    pub label: String,
    pub source: ExpectedWaveformCode,
    pub sensitivity: WaveformDecimalProjection,
    pub sensitivity_units: ExpectedWaveformCode,
    pub sensitivity_correction_factor: WaveformDecimalProjection,
    pub baseline: WaveformDecimalProjection,
    pub bits_stored: u8,
    pub time_skew_seconds: WaveformDecimalProjection,
    pub sample_skew_absent: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExpectedWaveformCode {
    pub code_value: String,
    pub coding_scheme_designator: String,
    pub code_meaning: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExpectedWaveformStorage {
    pub bits_allocated: u8,
    pub sample_interpretation: String,
    pub data_vr: String,
    pub byte_order: String,
    pub interleave_order: String,
    pub payload_length_bytes: u64,
    pub payload_sha256: String,
    pub channel_sha256: Vec<String>,
    pub sample_value_formula: String,
    pub sample_min: i16,
    pub sample_max: i16,
    pub waveform_padding_value_absent: bool,
    pub value_field_padding_bytes: u8,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExpectedWaveformAggregate {
    pub group_count: u64,
    pub total_channel_count: u64,
    pub common_duration_seconds: u64,
    pub total_payload_length_bytes: u64,
    pub group_payload_sha256: Vec<String>,
    pub aggregate_payload_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExpectedWaveformAbsentContent {
    pub annotation_module: bool,
    pub synchronization_module: bool,
    pub references: bool,
    pub image: bool,
    pub pixel_data: bool,
}

pub fn project_waveform(
    input: &WaveformPlanInput,
) -> Result<WaveformManifestProjection, SpecializedValidationError> {
    if input.caller_metadata.is_none()
        && input
            .groups
            .iter()
            .flat_map(|g| &g.channels)
            .any(|c| c.caller_calibration.is_some())
    {
        return Err(SpecializedValidationError::Projection(
            "channel calibration requires caller waveform contract".into(),
        ));
    }
    if input.caller_metadata.is_some() {
        crate::recipes::validate_caller_waveform_input(input)
            .map_err(|e| SpecializedValidationError::Projection(e.to_string()))?;
    }
    let formula = formula_text(input);
    let mut groups = Vec::with_capacity(input.groups.len());
    let mut aggregate_bytes = Vec::new();
    for (index, group) in input.groups.iter().enumerate() {
        let frequency = group.sampling_frequency_hz.parse::<u64>().map_err(|_| {
            SpecializedValidationError::Projection("invalid sampling frequency".into())
        })?;
        if frequency == 0 || u64::from(group.samples_per_channel) % frequency != 0 {
            return Err(SpecializedValidationError::Projection(
                "waveform duration is not integral".into(),
            ));
        }
        let bytes = waveform_bytes(input, index)?;
        if bytes.len() as u64 != group.declared_size_bytes
            || sha256_hex(&bytes) != group.declared_sha256
        {
            return Err(SpecializedValidationError::Projection(format!(
                "declared waveform content drift for {}",
                group.slot
            )));
        }
        let mut channel_sha256 = Vec::with_capacity(group.channels.len());
        for channel in 0..group.channels.len() {
            let mut channel_bytes = Vec::with_capacity(group.samples_per_channel as usize * 2);
            for sample in 0..group.samples_per_channel as usize {
                let offset = (sample * group.channels.len() + channel) * 2;
                channel_bytes.extend_from_slice(&bytes[offset..offset + 2]);
            }
            channel_sha256.push(sha256_hex(&channel_bytes));
        }
        let channels = group
            .channels
            .iter()
            .enumerate()
            .map(|(channel_index, channel)| ExpectedWaveformChannel {
                ordinal: channel_index as u64 + 1,
                label: channel.label.clone(),
                source: ExpectedWaveformCode {
                    code_value: channel.code_value.clone(),
                    coding_scheme_designator: "MDC".into(),
                    code_meaning: channel.code_meaning.clone(),
                },
                sensitivity: channel
                    .caller_calibration
                    .as_ref()
                    .map(|c| WaveformDecimalProjection::CallerLexeme(c.sensitivity.clone()))
                    .unwrap_or(WaveformDecimalProjection::HistoricalNumber(1)),
                sensitivity_units: ExpectedWaveformCode {
                    code_value: channel
                        .caller_calibration
                        .as_ref()
                        .map(|c| c.unit_code_value.clone())
                        .unwrap_or_else(|| "uV".into()),
                    coding_scheme_designator: channel
                        .caller_calibration
                        .as_ref()
                        .map(|c| c.unit_coding_scheme.clone())
                        .unwrap_or_else(|| "UCUM".into()),
                    code_meaning: channel
                        .caller_calibration
                        .as_ref()
                        .map(|c| c.unit_code_meaning.clone())
                        .unwrap_or_else(|| "microvolt".into()),
                },
                sensitivity_correction_factor: channel
                    .caller_calibration
                    .as_ref()
                    .map(|c| {
                        WaveformDecimalProjection::CallerLexeme(
                            c.sensitivity_correction_factor.clone(),
                        )
                    })
                    .unwrap_or(WaveformDecimalProjection::HistoricalNumber(1)),
                baseline: channel
                    .caller_calibration
                    .as_ref()
                    .map(|c| WaveformDecimalProjection::CallerLexeme(c.baseline.clone()))
                    .unwrap_or(WaveformDecimalProjection::HistoricalNumber(0)),
                bits_stored: 16,
                time_skew_seconds: channel
                    .caller_calibration
                    .as_ref()
                    .map(|c| WaveformDecimalProjection::CallerLexeme(c.time_skew_seconds.clone()))
                    .unwrap_or(WaveformDecimalProjection::HistoricalNumber(0)),
                sample_skew_absent: true,
            })
            .collect();
        groups.push(ExpectedMultiplexGroup {
            ordinal: index as u64 + 1,
            originality: "ORIGINAL".into(),
            label: group.label.clone(),
            channel_count: group.channels.len() as u64,
            samples_per_channel: group.samples_per_channel,
            sampling_frequency_hz: frequency,
            duration_seconds: u64::from(group.samples_per_channel) / frequency,
            simultaneous_sampling: true,
            channels,
            storage: ExpectedWaveformStorage {
                bits_allocated: 16,
                sample_interpretation: "SS".into(),
                data_vr: "OW".into(),
                byte_order: "little_endian".into(),
                interleave_order: "channel_then_sample".into(),
                payload_length_bytes: group.declared_size_bytes,
                payload_sha256: group.declared_sha256.clone(),
                channel_sha256,
                sample_value_formula: formula.clone(),
                sample_min: if input.caller_metadata.is_some() {
                    bytes
                        .chunks_exact(2)
                        .map(|v| i16::from_le_bytes(v.try_into().unwrap()))
                        .min()
                        .unwrap()
                        .into()
                } else {
                    -1000
                },
                sample_max: if input.caller_metadata.is_some() {
                    bytes
                        .chunks_exact(2)
                        .map(|v| i16::from_le_bytes(v.try_into().unwrap()))
                        .max()
                        .unwrap()
                        .into()
                } else {
                    1000
                },
                waveform_padding_value_absent: true,
                value_field_padding_bytes: 0,
            },
        });
        aggregate_bytes.extend_from_slice(&bytes);
    }
    let total_channels = groups.iter().map(|group| group.channel_count).sum();
    let total_bytes = groups
        .iter()
        .map(|group| group.storage.payload_length_bytes)
        .sum();
    let common_duration = groups.first().map_or(0, |group| group.duration_seconds);
    if groups
        .iter()
        .any(|group| group.duration_seconds != common_duration)
    {
        return Err(SpecializedValidationError::Projection(
            "waveform groups do not share a duration".into(),
        ));
    }
    let (iod_kind, iod_name, recipe_parameters, semantics, capabilities, visual, stressors) =
        match &input.projection {
            WaveformProjection::TwelveLead {
                expected_capabilities,
                expected_visual_pattern,
                known_stressors,
                simultaneous_sampling,
                one_second_duration,
                diagnostic_use,
            } => {
                let group = if input.caller_metadata.is_some() {
                    groups.first().ok_or_else(|| {
                        SpecializedValidationError::Projection(
                            "caller waveform groups missing".into(),
                        )
                    })?
                } else {
                    let [group] = groups.as_slice() else {
                        return Err(SpecializedValidationError::Projection(
                            "twelve-lead projection requires one group".into(),
                        ));
                    };
                    group
                };
                (
                    "twelve_lead_ecg",
                    "12-lead ECG Waveform",
                    WaveformRecipeParameters::TwelveLead {
                        multiplex_group_label: group.label.clone(),
                        channel_count: group.channel_count,
                        samples_per_channel: group.samples_per_channel,
                        sampling_frequency_hz: group.sampling_frequency_hz,
                        sample_formula: formula.clone(),
                    },
                    WaveformExpectedSemantics::TwelveLead {
                        synthetic_data: "YES".into(),
                        simultaneous_sampling: *simultaneous_sampling,
                        one_second_duration: *one_second_duration,
                        pixel_data_absent: true,
                        diagnostic_use: *diagnostic_use,
                    },
                    expected_capabilities.clone(),
                    expected_visual_pattern.clone(),
                    known_stressors.clone(),
                )
            }
            WaveformProjection::General {
                expected_capabilities,
                expected_visual_pattern,
                known_stressors,
                simultaneous_sampling_within_groups,
                common_duration_seconds,
                cross_group_synchronization_asserted,
                diagnostic_use,
            } => (
                "general_ecg",
                "General ECG Waveform",
                WaveformRecipeParameters::General {
                    multiplex_groups: groups
                        .iter()
                        .map(|group| WaveformRecipeGroup {
                            label: group.label.clone(),
                            channel_count: group.channel_count,
                            samples_per_channel: group.samples_per_channel,
                            sampling_frequency_hz: group.sampling_frequency_hz,
                        })
                        .collect(),
                    total_channel_count: total_channels,
                    total_payload_length_bytes: total_bytes,
                    aggregate_payload_sha256: sha256_hex(&aggregate_bytes),
                    sample_formula: formula.clone(),
                },
                WaveformExpectedSemantics::General {
                    synthetic_data: "YES".into(),
                    simultaneous_sampling_within_groups: *simultaneous_sampling_within_groups,
                    common_duration_seconds: *common_duration_seconds,
                    cross_group_synchronization_asserted: *cross_group_synchronization_asserted,
                    pixel_data_absent: true,
                    diagnostic_use: *diagnostic_use,
                },
                expected_capabilities.clone(),
                expected_visual_pattern.clone(),
                known_stressors.clone(),
            ),
        };
    let recipe_parameters = if input.caller_metadata.is_some() {
        WaveformRecipeParameters::Caller {
            waveform_capability_version: "1.0.0".into(),
            waveform_contract: input.clone(),
        }
    } else {
        recipe_parameters
    };
    Ok(WaveformManifestProjection {
        recipe_id: input.recipe.recipe_id.clone(),
        recipe_version: input.recipe.recipe_version.clone(),
        recipe_parameters,
        expected_capabilities: capabilities,
        expected_semantics: semantics,
        expected_visual_pattern: visual,
        expected_waveform: ExpectedWaveform {
            iod_kind: iod_kind.into(),
            sop_class_uid: input.sop_class_uid.clone(),
            iod_name: iod_name.into(),
            modality: input.modality.clone(),
            transfer_syntax_uid: "1.2.840.10008.1.2.1".into(),
            acquisition_context_items: 0,
            aggregate: ExpectedWaveformAggregate {
                group_count: groups.len() as u64,
                total_channel_count: total_channels,
                common_duration_seconds: common_duration,
                total_payload_length_bytes: total_bytes,
                group_payload_sha256: groups
                    .iter()
                    .map(|group| group.storage.payload_sha256.clone())
                    .collect(),
                aggregate_payload_sha256: sha256_hex(&aggregate_bytes),
            },
            multiplex_groups: groups,
            absent_content: ExpectedWaveformAbsentContent {
                annotation_module: true,
                synchronization_module: true,
                references: true,
                image: true,
                pixel_data: true,
            },
        },
        known_stressors: stressors,
    })
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EncapsulatedPayloadManifestProjection {
    pub recipe_id: String,
    pub recipe_version: String,
    pub recipe_parameters: EncapsulatedRecipeParameters,
    pub expected_capabilities: Vec<String>,
    pub expected_semantics: EncapsulatedExpectedSemantics,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_encapsulated_stl: Option<ExpectedEncapsulatedStlContract>,
    pub expected_visual_pattern: String,
    pub known_stressors: Vec<String>,
}

impl EncapsulatedPayloadManifestProjection {
    pub fn legacy_fields(&self) -> Value {
        let mut value = json!({
            "recipe": {"recipe_id": self.recipe_id, "recipe_version": self.recipe_version,
                "recipe_parameters": self.recipe_parameters},
            "expected_capabilities": self.expected_capabilities,
            "expected_semantics": self.expected_semantics,
            "expected_visual_checks": {"pattern": self.expected_visual_pattern},
            "known_stressors": self.known_stressors,
        });
        if let Some(stl) = &self.expected_encapsulated_stl {
            value["expected_encapsulated_stl"] = serde_json::to_value(stl).expect("typed STL");
        }
        value
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum EncapsulatedRecipeParameters {
    Caller {
        encapsulated_capability_version: String,
        encapsulated_contract: EncapsulatedPayloadPlanInput,
    },
    Pdf {
        document_title: String,
        mime_type: String,
        document_length: u64,
        document_sha256: String,
        burned_in_annotation: String,
        recognizable_visual_features: String,
    },
    Stl {
        document_title: String,
        content_description: String,
        payload_format: String,
        payload_length: u64,
        payload_sha256: String,
        triangle_count: u32,
        bounds_min: [i32; 3],
        bounds_max: [i32; 3],
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EncapsulatedExpectedSemantics {
    pub synthetic_data: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conversion_type: Option<String>,
    pub encapsulated_document: ExpectedEncapsulatedDocument,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExpectedEncapsulatedDocument {
    pub document_title: String,
    pub mime_type: String,
    pub document_length: u64,
    pub document_sha256: String,
    pub burned_in_annotation: String,
    pub recognizable_visual_features: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ExpectedEncapsulatedStlContract {
    Historical(ExpectedEncapsulatedStl),
    Caller(Value),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExpectedEncapsulatedStl {
    pub iod_kind: String,
    pub profile: String,
    pub payload: ExpectedStlPayload,
    pub units: ExpectedWaveformCode,
    pub geometry: ExpectedStlGeometry,
    pub independent_validator_disposition: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExpectedStlPayload {
    pub format: String,
    pub mime_type: String,
    pub length: u64,
    pub sha256: String,
    pub triangle_count: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExpectedStlGeometry {
    pub bounds_min: [i32; 3],
    pub bounds_max: [i32; 3],
    pub closed_manifold: bool,
    pub outward_winding: bool,
    pub nondegenerate_faces: bool,
}

pub fn project_encapsulated_payload(
    input: &EncapsulatedPayloadPlanInput,
) -> Result<EncapsulatedPayloadManifestProjection, SpecializedValidationError> {
    let (parameters, document, stl) = match &input.payload {
        EncapsulatedPayload::CallerPdf {
            mime_type,
            declared_size_bytes,
            declared_sha256,
            ..
        }
        | EncapsulatedPayload::CallerBinaryStl {
            mime_type,
            declared_size_bytes,
            declared_sha256,
            ..
        } => {
            crate::recipes::validate_caller_encapsulated_input(input)
                .map_err(|e| SpecializedValidationError::Projection(e.to_string()))?;
            let stl = if let EncapsulatedPayload::CallerBinaryStl {
                triangle_count,
                unit_code_value,
                unit_coding_scheme,
                unit_code_meaning,
                ..
            } = &input.payload
            {
                let bytes = crate::recipes::caller_encapsulated_bytes(input)
                    .map_err(|e| SpecializedValidationError::Projection(e.to_string()))?;
                let (low, high) = crate::recipes::caller_stl_bounds(&bytes)
                    .map_err(|e| SpecializedValidationError::Projection(e.to_string()))?;
                Some(ExpectedEncapsulatedStlContract::Caller(
                    json!({"iod_kind":"encapsulated_stl",
                    "payload":{"format":"binary_stl","mime_type":mime_type,"length":declared_size_bytes,"sha256":declared_sha256,"triangle_count":triangle_count},
                    "units":{"code_value":unit_code_value,"coding_scheme_designator":unit_coding_scheme,"code_meaning":unit_code_meaning},
                    "geometry":{"bounds_min":low,"bounds_max":high},
                    "independent_validator_disposition":"not_assessed"}),
                ))
            } else {
                None
            };
            (
                EncapsulatedRecipeParameters::Caller {
                    encapsulated_capability_version: "1.0.0".into(),
                    encapsulated_contract: input.clone(),
                },
                ExpectedEncapsulatedDocument {
                    document_title: input.document_title.clone(),
                    mime_type: mime_type.clone(),
                    document_length: *declared_size_bytes,
                    document_sha256: declared_sha256.clone(),
                    burned_in_annotation: input.burned_in_annotation.clone(),
                    recognizable_visual_features: input.recognizable_visual_features.clone(),
                },
                stl,
            )
        }
        EncapsulatedPayload::MinimalPdf {
            mime_type,
            declared_size_bytes,
            declared_sha256,
        } => (
            EncapsulatedRecipeParameters::Pdf {
                document_title: input.document_title.clone(),
                mime_type: mime_type.clone(),
                document_length: *declared_size_bytes,
                document_sha256: declared_sha256.clone(),
                burned_in_annotation: input.burned_in_annotation.clone(),
                recognizable_visual_features: input.recognizable_visual_features.clone(),
            },
            ExpectedEncapsulatedDocument {
                document_title: input.document_title.clone(),
                mime_type: mime_type.clone(),
                document_length: *declared_size_bytes,
                document_sha256: declared_sha256.clone(),
                burned_in_annotation: input.burned_in_annotation.clone(),
                recognizable_visual_features: input.recognizable_visual_features.clone(),
            },
            None,
        ),
        EncapsulatedPayload::ClosedTetrahedronBinaryStl {
            mime_type,
            declared_size_bytes,
            declared_sha256,
            triangle_count,
            unit_code_value,
            unit_coding_scheme,
            unit_code_meaning,
        } => (
            EncapsulatedRecipeParameters::Stl {
                document_title: input.document_title.clone(),
                content_description: input.content_description.clone().ok_or_else(|| {
                    SpecializedValidationError::Projection("STL content description missing".into())
                })?,
                payload_format: "binary_stl".into(),
                payload_length: *declared_size_bytes,
                payload_sha256: declared_sha256.clone(),
                triangle_count: *triangle_count,
                bounds_min: [0, 0, 0],
                bounds_max: [10, 10, 10],
            },
            ExpectedEncapsulatedDocument {
                document_title: input.document_title.clone(),
                mime_type: mime_type.clone(),
                document_length: *declared_size_bytes,
                document_sha256: declared_sha256.clone(),
                burned_in_annotation: input.burned_in_annotation.clone(),
                recognizable_visual_features: input.recognizable_visual_features.clone(),
            },
            Some(ExpectedEncapsulatedStlContract::Historical(
                ExpectedEncapsulatedStl {
                    iod_kind: "encapsulated_stl".into(),
                    profile: "extended".into(),
                    payload: ExpectedStlPayload {
                        format: "binary_stl".into(),
                        mime_type: mime_type.clone(),
                        length: *declared_size_bytes,
                        sha256: declared_sha256.clone(),
                        triangle_count: *triangle_count,
                    },
                    units: ExpectedWaveformCode {
                        code_value: unit_code_value.clone(),
                        coding_scheme_designator: unit_coding_scheme.clone(),
                        code_meaning: unit_code_meaning.clone(),
                    },
                    geometry: ExpectedStlGeometry {
                        bounds_min: [0, 0, 0],
                        bounds_max: [10, 10, 10],
                        closed_manifold: true,
                        outward_winding: true,
                        nondegenerate_faces: true,
                    },
                    independent_validator_disposition: "required".into(),
                },
            )),
        ),
    };
    Ok(EncapsulatedPayloadManifestProjection {
        recipe_id: input.recipe.recipe_id.clone(),
        recipe_version: input.recipe.recipe_version.clone(),
        recipe_parameters: parameters,
        expected_capabilities: input.projection.expected_capabilities.clone(),
        expected_semantics: EncapsulatedExpectedSemantics {
            synthetic_data: "YES".into(),
            conversion_type: matches!(
                input.payload,
                EncapsulatedPayload::MinimalPdf { .. } | EncapsulatedPayload::CallerPdf { .. }
            )
            .then(|| "SYN".into()),
            encapsulated_document: document,
        },
        expected_encapsulated_stl: stl,
        expected_visual_pattern: input.projection.expected_visual_pattern.clone(),
        known_stressors: input.projection.known_stressors.clone(),
    })
}

fn formula_text(input: &WaveformPlanInput) -> String {
    if input.caller_metadata.is_some() {
        return format!(
            "((s * (c + 1) * (g + 1) * {} + c * {} + g * {}) mod {}) - {}; g is declared formula_group_index",
            input.formula.sample_multiplier,
            input.formula.channel_bias_multiplier,
            input.formula.group_bias_multiplier,
            input.formula.modulus,
            input.formula.baseline
        );
    }
    match input.projection {
        WaveformProjection::TwelveLead { .. } => {
            "((s * (c + 1) * 37 + c * 101) mod 2001) - 1000".into()
        }
        WaveformProjection::General { .. } => {
            "((s * (c + 1) * (g + 1) * 37 + c * 101 + g * 307) mod 2001) - 1000".into()
        }
    }
}

fn waveform_bytes(
    input: &WaveformPlanInput,
    group_index: usize,
) -> Result<Vec<u8>, SpecializedValidationError> {
    if input.caller_metadata.is_some() {
        return crate::recipes::caller_waveform_group_bytes(input, group_index)
            .map_err(|e| SpecializedValidationError::Projection(e.to_string()));
    }
    let group = &input.groups[group_index];
    let capacity = group
        .channels
        .len()
        .checked_mul(group.samples_per_channel as usize)
        .and_then(|value| value.checked_mul(2))
        .ok_or_else(|| SpecializedValidationError::Projection("waveform size overflow".into()))?;
    let mut bytes = Vec::with_capacity(capacity);
    for sample in 0..group.samples_per_channel as u64 {
        for channel in 0..group.channels.len() as u64 {
            let value = sample
                .checked_mul(channel + 1)
                .and_then(|value| value.checked_mul(u64::from(group.formula_group_index + 1)))
                .and_then(|value| value.checked_mul(u64::from(input.formula.sample_multiplier)))
                .and_then(|value| {
                    value.checked_add(channel * u64::from(input.formula.channel_bias_multiplier))
                })
                .and_then(|value| {
                    value.checked_add(
                        u64::from(group.formula_group_index)
                            * u64::from(input.formula.group_bias_multiplier),
                    )
                })
                .ok_or_else(|| {
                    SpecializedValidationError::Projection("waveform formula overflow".into())
                })?
                % u64::from(input.formula.modulus);
            let value = i64::try_from(value).unwrap() - i64::from(input.formula.baseline);
            let value = i16::try_from(value).map_err(|_| {
                SpecializedValidationError::Projection("waveform sample out of range".into())
            })?;
            bytes.extend_from_slice(&value.to_le_bytes());
        }
    }
    Ok(bytes)
}
