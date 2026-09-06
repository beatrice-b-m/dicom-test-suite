//! Direct, filesystem-free plans for deterministic ECG Waveform objects.

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::composition::{
    AttributeAddress, AttributeItem, AttributeOperation, AttributeValue, CanonicalContent,
    CompositionUidRole, ContentPlacement, DicomVr, PrimitiveValue, ResolvedAttribute,
    ResolvedInstancePlan, SequenceItemPlacement, TemplateId, TemplateVersion, ValueOrigin,
};
use crate::corpus_plan::{
    ArtifactProvenance, ArtifactResourceEstimate, CaseBinding, EncodingPlan, EvidenceIndependence,
    EvidenceObligation, EvidencePlan, FileMetaPolicy, FragmentationPolicy,
    ImplementationIdentityPlan, ItemLengthPolicy, OffsetTablePolicy, PlannedDicomArtifact,
    PreamblePolicy, SequenceLengthPolicy, ValidationPlan, ValidationRequirement, ValidationRule,
};
use crate::executor::services::{
    ArtifactExecutionBindings, ByteBinding, NativeFrameBinding, SlotExecutionBinding,
};
use crate::sha256_hex;
use crate::{BYTE_STABLE_OUTPUT_VERSION, IMPLEMENTATION_VERSION_NAME};

use super::typed_bulk::{TypedBulkPlanProviderOutput, TypedBulkPlanningContext};
use super::{
    CaseRecipe, ContentByteOrder, ContentProviderLimits, ContentProviderRequest, ContentTarget,
    IntegerSamples, NeutralContentProvider, RecipeIdentity, WaveformContract,
};

pub const WAVEFORM_PLAN_PROVIDER_ID: &str = "native.waveform_plan";
pub const WAVEFORM_CONTENT_PROVIDER_ID: &str = "content.waveform_samples";
pub const WAVEFORM_ALGORITHM_PROVIDER_ID: &str = "algorithm.waveform_deterministic_multiplex";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WaveformPlanInput {
    pub case_id: String,
    pub recipe: RecipeIdentity,
    pub artifact_logical_id: String,
    pub template_id: String,
    pub sop_class_uid: String,
    pub output_path: String,
    pub modality: String,
    pub study_id: String,
    pub series_number: String,
    pub manufacturer_model_name: String,
    pub device_serial_number: String,
    pub groups: Vec<WaveformGroupInput>,
    pub formula: WaveformFormula,
    pub projection: WaveformProjection,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub caller_metadata: Option<WaveformCallerMetadata>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WaveformCallerMetadata {
    pub patient_name: String,
    pub patient_id: String,
    pub patient_birth_date: String,
    pub patient_sex: String,
    pub study_date: String,
    pub study_time: String,
    pub content_date: String,
    pub content_time: String,
    pub acquisition_datetime: String,
    pub manufacturer: String,
    pub software_versions: String,
    pub instance_number: String,
    pub referring_physician_name: String,
    pub accession_number: String,
    pub institution_name: String,
    pub institution_address: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WaveformGroupInput {
    pub slot: String,
    pub label: String,
    pub channels: Vec<WaveformChannelInput>,
    pub samples_per_channel: u32,
    pub sampling_frequency_hz: String,
    pub formula_group_index: u32,
    pub declared_size_bytes: u64,
    pub declared_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WaveformChannelInput {
    pub label: String,
    pub code_value: String,
    pub code_meaning: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub caller_calibration: Option<WaveformChannelCalibration>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WaveformChannelCalibration {
    pub sensitivity: String,
    pub sensitivity_correction_factor: String,
    pub baseline: String,
    pub time_skew_seconds: String,
    pub unit_code_value: String,
    pub unit_coding_scheme: String,
    pub unit_code_meaning: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WaveformFormula {
    pub sample_multiplier: u32,
    pub channel_bias_multiplier: u32,
    pub group_bias_multiplier: u32,
    pub modulus: u32,
    pub baseline: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum WaveformProjection {
    TwelveLead {
        expected_capabilities: Vec<String>,
        expected_visual_pattern: String,
        known_stressors: Vec<String>,
        simultaneous_sampling: bool,
        one_second_duration: bool,
        diagnostic_use: bool,
    },
    General {
        expected_capabilities: Vec<String>,
        expected_visual_pattern: String,
        known_stressors: Vec<String>,
        simultaneous_sampling_within_groups: bool,
        common_duration_seconds: u32,
        cross_group_synchronization_asserted: bool,
        diagnostic_use: bool,
    },
}

pub fn waveform_input_from_recipe(
    recipe: &CaseRecipe,
) -> Result<Option<WaveformPlanInput>, WaveformPlanError> {
    if recipe.plan_provider_id != WAVEFORM_PLAN_PROVIDER_ID {
        return Ok(None);
    }
    let [artifact] = recipe
        .dicom
        .as_ref()
        .ok_or(WaveformPlanError::Recipe("missing DICOM artifact".into()))?
        .artifacts
        .as_slice()
    else {
        return Err(WaveformPlanError::Recipe(
            "waveform recipe requires one artifact".into(),
        ));
    };
    if artifact.content.provider_id != WAVEFORM_CONTENT_PROVIDER_ID
        || artifact.algorithm_provider_id.as_deref() != Some(WAVEFORM_ALGORITHM_PROVIDER_ID)
    {
        return Err(WaveformPlanError::Recipe(
            "waveform content or algorithm provider is not registered".into(),
        ));
    }
    let parameters: WaveformDocumentParameters = serde_json::from_value(serde_json::Value::Object(
        recipe.provider_parameters.clone(),
    ))
    .map_err(|error| WaveformPlanError::Recipe(error.to_string()))?;
    let template_id = artifact
        .template
        .as_ref()
        .ok_or(WaveformPlanError::Recipe(
            "missing waveform template".into(),
        ))?
        .template_id
        .clone();
    let output_path = artifact
        .output
        .path
        .clone()
        .ok_or(WaveformPlanError::Recipe(
            "missing waveform output path".into(),
        ))?;
    let input = WaveformPlanInput {
        case_id: recipe.binding.case_id.clone(),
        recipe: recipe.identity(),
        artifact_logical_id: artifact.logical_id.clone(),
        template_id,
        sop_class_uid: parameters.sop_class_uid,
        output_path,
        modality: parameters.modality,
        study_id: parameters.study_id,
        series_number: parameters.series_number,
        manufacturer_model_name: parameters.manufacturer_model_name,
        device_serial_number: parameters.device_serial_number,
        groups: parameters.groups,
        formula: parameters.formula,
        projection: parameters.projection,
        caller_metadata: parameters.caller_metadata,
    };
    if recipe.case_recipe_schema_version == "0.2.0" {
        if recipe.planning_order.is_none()
            || recipe.projection_order.is_none()
            || !recipe.dependencies.is_empty()
            || artifact.output.provider_derived == Some(true)
            || !artifact.attribute_operations.is_empty()
            || !artifact.parameters.is_empty()
            || !artifact.content.parameters.is_empty()
            || artifact.secondary_capture.is_some()
            || artifact.metadata_sc.is_some()
            || artifact.classic_projection.is_some()
            || artifact.public_profile_membership.is_some()
            || artifact.encoding.transfer_syntax_uid != "1.2.840.10008.1.2.1"
            || artifact.encoding.sequence_length_policy != "default"
            || artifact.encoding.item_length_policy != "default"
            || artifact.encoding.offset_table_policy != "none"
            || artifact.encoding.fragmentation_policy != "native"
            || artifact
                .encoding
                .non_template_encoding_provider_id
                .is_some()
            || artifact.encoding.preamble_policy.as_deref() != Some("zero_filled")
            || artifact.encoding.file_meta_policy.as_deref() != Some("standard")
            || artifact
                .template
                .as_ref()
                .is_none_or(|t| t.template_version != "1.0.0")
        {
            return Err(WaveformPlanError::Shape);
        }
        for rules in [&recipe.validation_rule_ids, &artifact.validation_rule_ids] {
            if rules.len() != 3
                || ![
                    "validation.waveform.topology",
                    "validation.waveform.samples",
                    "validation.content.integrity",
                ]
                .iter()
                .all(|required| rules.iter().any(|rule| rule == required))
            {
                return Err(WaveformPlanError::Recipe(
                    "caller waveform requires exact native waveform validation rules".into(),
                ));
            }
        }
        for rules in [&recipe.projection_rule_ids, &artifact.projection_rule_ids] {
            if rules.as_slice() != ["projection.waveform"] {
                return Err(WaveformPlanError::Recipe(
                    "caller waveform requires its native projection rule".into(),
                ));
            }
        }
        validate_caller_waveform_input(&input)?;
    } else if input.caller_metadata.is_some()
        || input
            .groups
            .iter()
            .flat_map(|g| &g.channels)
            .any(|c| c.caller_calibration.is_some())
    {
        return Err(WaveformPlanError::Recipe(
            "caller waveform metadata requires recipe0.2".into(),
        ));
    }
    Ok(Some(input))
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct WaveformDocumentParameters {
    sop_class_uid: String,
    modality: String,
    study_id: String,
    series_number: String,
    manufacturer_model_name: String,
    device_serial_number: String,
    groups: Vec<WaveformGroupInput>,
    formula: WaveformFormula,
    projection: WaveformProjection,
    #[serde(default)]
    caller_metadata: Option<WaveformCallerMetadata>,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct WaveformPlanProvider;

impl WaveformPlanProvider {
    pub fn plan(
        &self,
        recipe: &WaveformPlanInput,
        context: &TypedBulkPlanningContext,
        limits: ContentProviderLimits,
    ) -> Result<TypedBulkPlanProviderOutput, WaveformPlanError> {
        context
            .validate(&recipe.artifact_logical_id)
            .map_err(WaveformPlanError::Context)?;
        validate_input(recipe)?;
        if recipe.caller_metadata.is_some() {
            validate_caller_waveform_input(recipe)?;
        }
        let ids = Identities::from_context(context)?;
        let waveform_sequence = address("WaveformSequence")?;
        let mut items = Vec::with_capacity(recipe.groups.len());
        let mut contents = Vec::with_capacity(recipe.groups.len());
        let mut slots = BTreeMap::new();
        let content_provider = NeutralContentProvider;
        let mut total_payload = 0_u64;
        for (item_index, group) in recipe.groups.iter().enumerate() {
            let samples = samples(recipe, group)?;
            let output = content_provider
                .expand(
                    &ContentProviderRequest::Waveform(WaveformContract {
                        target: ContentTarget {
                            slot: group.slot.clone(),
                            content_kind: "waveform_samples".into(),
                            address: address("WaveformData")?,
                            vr: DicomVr::OW,
                        },
                        channels: u32::try_from(group.channels.len())
                            .map_err(|_| WaveformPlanError::ResourceOverflow)?,
                        samples_per_channel: group.samples_per_channel,
                        bits_allocated: 16,
                        byte_order: ContentByteOrder::LittleEndian,
                        samples: IntegerSamples::Signed { values: samples },
                    }),
                    limits,
                )
                .map_err(|error| WaveformPlanError::Content(error.to_string()))?;
            let mut content =
                output
                    .contents
                    .into_iter()
                    .next()
                    .ok_or(WaveformPlanError::Content(
                        "provider omitted content".into(),
                    ))?;
            if content.size_bytes != group.declared_size_bytes
                || content.sha256 != group.declared_sha256
            {
                return Err(WaveformPlanError::DeclaredDigest(group.slot.clone()));
            }
            content.placement = ContentPlacement::Nested {
                sequence_path: vec![SequenceItemPlacement {
                    sequence: waveform_sequence.clone(),
                    item_index,
                }],
            };
            total_payload = total_payload
                .checked_add(content.size_bytes)
                .ok_or(WaveformPlanError::ResourceOverflow)?;
            slots.insert(group.slot.clone(), native_binding(group, &content)?);
            contents.push(content);
            items.push(waveform_item(group)?);
        }
        let attributes = common_attributes(recipe, &ids, items)?;
        let artifact_id = context.target_instance_id.clone();
        let planned = PlannedDicomArtifact {
            logical_id: artifact_id.clone(),
            order: context.order,
            provenance: ArtifactProvenance::Requested,
            case_binding: Some(CaseBinding {
                case_id: recipe.case_id.clone(),
                recipe_id: recipe.recipe.recipe_id.clone(),
                recipe_version: recipe.recipe.recipe_version.clone(),
            }),
            instance: ResolvedInstancePlan {
                plan_schema_version: "0.1.0".into(),
                instance_id: artifact_id.clone(),
                template_id: TemplateId(recipe.template_id.clone()),
                template_version: "1.0.0"
                    .parse::<TemplateVersion>()
                    .map_err(|error| WaveformPlanError::Template(error.to_string()))?,
                sop_class_uid: recipe.sop_class_uid.clone(),
                transfer_syntax_uid: "1.2.840.10008.1.2.1".into(),
                identities: context.identities.clone(),
                attributes,
                content: contents,
                references: vec![],
            },
            output: context.output.clone(),
            encoding: encoding(&ids.implementation),
            validation: validation(),
            evidence: evidence(&artifact_id),
            resources: ArtifactResourceEstimate {
                output_bytes: total_payload
                    .checked_add(256 * 1024)
                    .ok_or(WaveformPlanError::ResourceOverflow)?,
                peak_working_bytes: total_payload
                    .checked_mul(2)
                    .and_then(|value| value.checked_add(512 * 1024))
                    .ok_or(WaveformPlanError::ResourceOverflow)?,
            },
        };
        Ok(TypedBulkPlanProviderOutput {
            artifact: planned,
            bindings: ArtifactExecutionBindings { artifact_id, slots },
        })
    }
}

/// Source-note-backed CID 3001 subset; codes are independent of caller labels,
/// channel order and group layout.
fn supported_ecg_source(code: &str) -> Option<&'static str> {
    match code {
        "2:1" => Some("Lead I"),
        "2:2" => Some("Lead II"),
        "2:61" => Some("Lead III"),
        "2:62" => Some("aVR, augmented voltage, right"),
        "2:63" => Some("aVL, augmented voltage, left"),
        "2:64" => Some("aVF, augmented voltage, foot"),
        "2:3" => Some("Lead V1"),
        "2:4" => Some("Lead V2"),
        "2:5" => Some("Lead V3"),
        "2:6" => Some("Lead V4"),
        "2:7" => Some("Lead V5"),
        "2:8" => Some("Lead V6"),
        "2:75" => Some("Auxiliary unipolar lead 1"),
        "2:76" => Some("Auxiliary unipolar lead 2"),
        "2:77" => Some("Auxiliary unipolar lead 3"),
        "2:78" => Some("Auxiliary unipolar lead 4"),
        _ => None,
    }
}

fn validate_waveform_text(keyword: &str, vr: DicomVr, text: &str) -> Result<(), WaveformPlanError> {
    if !text.is_ascii() || text.contains('\\') {
        return Err(WaveformPlanError::Attribute(
            "caller waveform text requires singleton ASCII".into(),
        ));
    }
    AttributeOperation::Set {
        address: address(keyword)?,
        vr,
        value: AttributeValue::Primitive(PrimitiveValue::String(text.into())),
    }
    .validate_declared_vr()
    .map_err(|e| WaveformPlanError::Attribute(e.to_string()))
}

pub fn validate_caller_waveform_input(input: &WaveformPlanInput) -> Result<(), WaveformPlanError> {
    validate_input(input)?;
    let m = input.caller_metadata.as_ref().ok_or_else(|| {
        WaveformPlanError::Recipe("complete caller waveform metadata required".into())
    })?;
    let twelve = matches!(input.projection, WaveformProjection::TwelveLead { .. });
    let (sop, template, max_groups, max_channels) = if twelve {
        (
            "1.2.840.10008.5.1.4.1.1.9.1.1",
            "non-image/waveform/twelve-lead-ecg",
            5,
            13,
        )
    } else {
        (
            "1.2.840.10008.5.1.4.1.1.9.1.2",
            "non-image/waveform/general-ecg",
            4,
            24,
        )
    };
    if input.sop_class_uid != sop
        || input.template_id != template
        || input.modality != "ECG"
        || input.groups.len() > max_groups
        || (twelve && input.groups.iter().map(|g| g.channels.len()).sum::<usize>() > 13)
        || [
            m.instance_number.as_str(),
            m.content_date.as_str(),
            m.content_time.as_str(),
            m.acquisition_datetime.as_str(),
        ]
        .iter()
        .any(|value| value.trim().is_empty())
        || !matches!(m.patient_sex.as_str(), "" | "M" | "F" | "O")
    {
        return Err(WaveformPlanError::Shape);
    }
    for (keyword, vr, text) in [
        ("PatientName", DicomVr::PN, m.patient_name.as_str()),
        ("PatientID", DicomVr::LO, m.patient_id.as_str()),
        (
            "PatientBirthDate",
            DicomVr::DA,
            m.patient_birth_date.as_str(),
        ),
        ("PatientSex", DicomVr::CS, m.patient_sex.as_str()),
        ("StudyDate", DicomVr::DA, m.study_date.as_str()),
        ("StudyTime", DicomVr::TM, m.study_time.as_str()),
        ("ContentDate", DicomVr::DA, m.content_date.as_str()),
        ("ContentTime", DicomVr::TM, m.content_time.as_str()),
        (
            "AcquisitionDateTime",
            DicomVr::DT,
            m.acquisition_datetime.as_str(),
        ),
        ("Manufacturer", DicomVr::LO, m.manufacturer.as_str()),
        (
            "SoftwareVersions",
            DicomVr::LO,
            m.software_versions.as_str(),
        ),
        ("InstanceNumber", DicomVr::IS, m.instance_number.as_str()),
        (
            "ReferringPhysicianName",
            DicomVr::PN,
            m.referring_physician_name.as_str(),
        ),
        ("AccessionNumber", DicomVr::SH, m.accession_number.as_str()),
        ("InstitutionName", DicomVr::LO, m.institution_name.as_str()),
        (
            "InstitutionAddress",
            DicomVr::ST,
            m.institution_address.as_str(),
        ),
        ("StudyID", DicomVr::SH, input.study_id.as_str()),
        ("SeriesNumber", DicomVr::IS, input.series_number.as_str()),
        (
            "ManufacturerModelName",
            DicomVr::LO,
            input.manufacturer_model_name.as_str(),
        ),
        (
            "DeviceSerialNumber",
            DicomVr::LO,
            input.device_serial_number.as_str(),
        ),
    ] {
        validate_waveform_text(keyword, vr, text)?;
    }
    let mut labels = std::collections::BTreeSet::new();
    let mut duration = None;
    let mut total_values = 0_u64;
    for (index, group) in input.groups.iter().enumerate() {
        let frequency = group
            .sampling_frequency_hz
            .parse::<u64>()
            .map_err(|_| WaveformPlanError::Shape)?;
        if group.channels.len() > max_channels
            || !(200..=1000).contains(&frequency)
            || (twelve && group.samples_per_channel > 16384)
            || u64::from(group.samples_per_channel) % frequency != 0
            || !labels.insert(group.label.as_str())
            || group.label.trim().is_empty()
        {
            return Err(WaveformPlanError::Shape);
        }
        validate_waveform_text("MultiplexGroupLabel", DicomVr::SH, &group.label)?;
        validate_waveform_text(
            "SamplingFrequency",
            DicomVr::DS,
            &group.sampling_frequency_hz,
        )?;
        let current = u64::from(group.samples_per_channel) / frequency;
        if duration.is_some_and(|d| d != current) {
            return Err(WaveformPlanError::Shape);
        }
        duration = Some(current);
        total_values = total_values
            .checked_add(u64::from(group.samples_per_channel) * group.channels.len() as u64)
            .ok_or(WaveformPlanError::ResourceOverflow)?;
        if total_values > ContentProviderLimits::default().max_elements {
            return Err(WaveformPlanError::ResourceOverflow);
        }
        let mut codes = std::collections::BTreeSet::new();
        for channel in &group.channels {
            if !codes.insert(channel.code_value.as_str())
                || supported_ecg_source(&channel.code_value) != Some(channel.code_meaning.as_str())
                || channel.label.trim().is_empty()
            {
                return Err(WaveformPlanError::Shape);
            }
            validate_waveform_text("ChannelLabel", DicomVr::SH, &channel.label)?;
            let calibration = channel.caller_calibration.as_ref().ok_or_else(|| {
                WaveformPlanError::Recipe("caller channel calibration is required".into())
            })?;
            for (keyword, text, positive) in [
                ("ChannelSensitivity", calibration.sensitivity.as_str(), true),
                (
                    "ChannelSensitivityCorrectionFactor",
                    calibration.sensitivity_correction_factor.as_str(),
                    true,
                ),
                ("ChannelBaseline", calibration.baseline.as_str(), false),
                (
                    "ChannelTimeSkew",
                    calibration.time_skew_seconds.as_str(),
                    false,
                ),
            ] {
                validate_waveform_text(keyword, DicomVr::DS, text)?;
                let value = text
                    .trim()
                    .parse::<f64>()
                    .map_err(|_| WaveformPlanError::Shape)?;
                if !value.is_finite()
                    || (positive && value <= 0.0)
                    || (keyword == "ChannelTimeSkew" && value != 0.0)
                {
                    return Err(WaveformPlanError::Shape);
                }
            }
            for (keyword, vr, text) in [
                (
                    "CodeValue",
                    DicomVr::SH,
                    calibration.unit_code_value.as_str(),
                ),
                (
                    "CodingSchemeDesignator",
                    DicomVr::SH,
                    calibration.unit_coding_scheme.as_str(),
                ),
                (
                    "CodeMeaning",
                    DicomVr::LO,
                    calibration.unit_code_meaning.as_str(),
                ),
            ] {
                if text.trim().is_empty() {
                    return Err(WaveformPlanError::Shape);
                }
                validate_waveform_text(keyword, vr, text)?;
            }
        }
        caller_waveform_group_bytes(input, index)?;
    }
    match input.projection {
        WaveformProjection::TwelveLead {
            simultaneous_sampling,
            one_second_duration,
            diagnostic_use,
            ..
        } => {
            if !simultaneous_sampling
                || diagnostic_use
                || one_second_duration != (duration == Some(1))
            {
                return Err(WaveformPlanError::Shape);
            }
        }
        WaveformProjection::General {
            simultaneous_sampling_within_groups,
            common_duration_seconds,
            cross_group_synchronization_asserted,
            diagnostic_use,
            ..
        } => {
            if !simultaneous_sampling_within_groups
                || cross_group_synchronization_asserted
                || diagnostic_use
                || duration != Some(u64::from(common_duration_seconds))
            {
                return Err(WaveformPlanError::Shape);
            }
        }
    }
    Ok(())
}

pub fn caller_waveform_group_bytes(
    input: &WaveformPlanInput,
    index: usize,
) -> Result<Vec<u8>, WaveformPlanError> {
    let group = input.groups.get(index).ok_or(WaveformPlanError::Shape)?;
    let count = (group.channels.len() as u64)
        .checked_mul(u64::from(group.samples_per_channel))
        .ok_or(WaveformPlanError::ResourceOverflow)?;
    if count == 0
        || count > ContentProviderLimits::default().max_elements
        || input.formula.modulus == 0
    {
        return Err(WaveformPlanError::ResourceOverflow);
    }
    // The complete modulo domain must fit signed 16-bit storage, including
    // negative baselines. Measured extrema are still derived from actual samples.
    let minimum = -i64::from(input.formula.baseline);
    let maximum = i64::from(input.formula.modulus) - 1 - i64::from(input.formula.baseline);
    if i16::try_from(minimum).is_err() || i16::try_from(maximum).is_err() {
        return Err(WaveformPlanError::Shape);
    }
    // Bound every formula term before allocating the sample vector.
    u64::from(group.samples_per_channel - 1)
        .checked_mul(group.channels.len() as u64)
        .and_then(|v| v.checked_mul(u64::from(group.formula_group_index) + 1))
        .and_then(|v| v.checked_mul(u64::from(input.formula.sample_multiplier)))
        .and_then(|v| {
            v.checked_add(
                (group.channels.len() as u64 - 1)
                    * u64::from(input.formula.channel_bias_multiplier),
            )
        })
        .and_then(|v| {
            v.checked_add(
                u64::from(group.formula_group_index)
                    * u64::from(input.formula.group_bias_multiplier),
            )
        })
        .ok_or(WaveformPlanError::ResourceOverflow)?;
    let output = NeutralContentProvider
        .expand(
            &ContentProviderRequest::Waveform(WaveformContract {
                target: ContentTarget {
                    slot: group.slot.clone(),
                    content_kind: "waveform_samples".into(),
                    address: address("WaveformData")?,
                    vr: DicomVr::OW,
                },
                channels: u32::try_from(group.channels.len())
                    .map_err(|_| WaveformPlanError::ResourceOverflow)?,
                samples_per_channel: group.samples_per_channel,
                bits_allocated: 16,
                byte_order: ContentByteOrder::LittleEndian,
                samples: IntegerSamples::Signed {
                    values: samples(input, group)?,
                },
            }),
            ContentProviderLimits::default(),
        )
        .map_err(|e| WaveformPlanError::Content(e.to_string()))?;
    let Some(crate::composition::ContentMaterialization::Inline(bytes)) = output
        .contents
        .into_iter()
        .next()
        .and_then(|c| c.materialization)
    else {
        return Err(WaveformPlanError::Shape);
    };
    if bytes.len() as u64 != group.declared_size_bytes
        || sha256_hex(&bytes) != group.declared_sha256
    {
        return Err(WaveformPlanError::DeclaredDigest(group.slot.clone()));
    }
    Ok(bytes)
}

fn validate_input(input: &WaveformPlanInput) -> Result<(), WaveformPlanError> {
    if input.caller_metadata.is_none()
        && input
            .groups
            .iter()
            .flat_map(|g| &g.channels)
            .any(|c| c.caller_calibration.is_some())
    {
        return Err(WaveformPlanError::Recipe(
            "channel calibration requires complete caller waveform metadata".into(),
        ));
    }
    if input.groups.is_empty() || input.groups.len() > 8 || input.formula.modulus == 0 {
        return Err(WaveformPlanError::Shape);
    }
    let mut slots = std::collections::BTreeSet::new();
    for group in &input.groups {
        if group.channels.is_empty()
            || group.channels.len() > 64
            || group.samples_per_channel == 0
            || group.declared_size_bytes == 0
            || !slots.insert(group.slot.as_str())
        {
            return Err(WaveformPlanError::Shape);
        }
        let expected = (group.channels.len() as u64)
            .checked_mul(u64::from(group.samples_per_channel))
            .and_then(|value| value.checked_mul(2))
            .ok_or(WaveformPlanError::ResourceOverflow)?;
        if expected != group.declared_size_bytes {
            return Err(WaveformPlanError::DeclaredDigest(group.slot.clone()));
        }
    }
    Ok(())
}

fn samples(
    input: &WaveformPlanInput,
    group: &WaveformGroupInput,
) -> Result<Vec<i64>, WaveformPlanError> {
    let count = (group.channels.len() as u64)
        .checked_mul(u64::from(group.samples_per_channel))
        .ok_or(WaveformPlanError::ResourceOverflow)?;
    let mut values = Vec::with_capacity(
        usize::try_from(count).map_err(|_| WaveformPlanError::ResourceOverflow)?,
    );
    for sample in 0..group.samples_per_channel {
        for channel in 0..group.channels.len() as u32 {
            let product = u64::from(sample)
                .checked_mul(u64::from(channel + 1))
                .and_then(|value| value.checked_mul(u64::from(group.formula_group_index) + 1))
                .and_then(|value| value.checked_mul(u64::from(input.formula.sample_multiplier)))
                .ok_or(WaveformPlanError::ResourceOverflow)?;
            let channel_bias = u64::from(channel)
                .checked_mul(u64::from(input.formula.channel_bias_multiplier))
                .ok_or(WaveformPlanError::ResourceOverflow)?;
            let group_bias = u64::from(group.formula_group_index)
                .checked_mul(u64::from(input.formula.group_bias_multiplier))
                .ok_or(WaveformPlanError::ResourceOverflow)?;
            let value = product
                .checked_add(channel_bias)
                .and_then(|value| value.checked_add(group_bias))
                .ok_or(WaveformPlanError::ResourceOverflow)?
                % u64::from(input.formula.modulus);
            values.push(
                i64::try_from(value).map_err(|_| WaveformPlanError::ResourceOverflow)?
                    - i64::from(input.formula.baseline),
            );
        }
    }
    Ok(values)
}

fn waveform_item(group: &WaveformGroupInput) -> Result<AttributeItem, WaveformPlanError> {
    Ok(AttributeItem {
        attributes: vec![
            set_string("WaveformOriginality", DicomVr::CS, "ORIGINAL")?,
            set_unsigned(
                "NumberOfWaveformChannels",
                DicomVr::US,
                group.channels.len() as u64,
            )?,
            set_unsigned(
                "NumberOfWaveformSamples",
                DicomVr::UL,
                u64::from(group.samples_per_channel),
            )?,
            set_string(
                "SamplingFrequency",
                DicomVr::DS,
                &group.sampling_frequency_hz,
            )?,
            set_string("MultiplexGroupLabel", DicomVr::SH, &group.label)?,
            set_sequence(
                "ChannelDefinitionSequence",
                group
                    .channels
                    .iter()
                    .enumerate()
                    .map(|(index, channel)| channel_item(index + 1, channel))
                    .collect::<Result<Vec<_>, _>>()?,
            )?,
            set_unsigned("WaveformBitsAllocated", DicomVr::US, 16)?,
            set_string("WaveformSampleInterpretation", DicomVr::CS, "SS")?,
        ],
    })
}

fn channel_item(
    ordinal: usize,
    channel: &WaveformChannelInput,
) -> Result<AttributeItem, WaveformPlanError> {
    let calibration = channel.caller_calibration.as_ref();
    Ok(AttributeItem {
        attributes: vec![
            set_string("WaveformChannelNumber", DicomVr::IS, &ordinal.to_string())?,
            set_string("ChannelLabel", DicomVr::SH, &channel.label)?,
            set_sequence(
                "ChannelSourceSequence",
                vec![code_item(
                    "MDC",
                    &channel.code_value,
                    &channel.code_meaning,
                )?],
            )?,
            set_string(
                "ChannelSensitivity",
                DicomVr::DS,
                calibration.map(|c| c.sensitivity.as_str()).unwrap_or("1"),
            )?,
            set_sequence(
                "ChannelSensitivityUnitsSequence",
                vec![code_item(
                    calibration
                        .map(|c| c.unit_coding_scheme.as_str())
                        .unwrap_or("UCUM"),
                    calibration
                        .map(|c| c.unit_code_value.as_str())
                        .unwrap_or("uV"),
                    calibration
                        .map(|c| c.unit_code_meaning.as_str())
                        .unwrap_or("microvolt"),
                )?],
            )?,
            set_string(
                "ChannelSensitivityCorrectionFactor",
                DicomVr::DS,
                calibration
                    .map(|c| c.sensitivity_correction_factor.as_str())
                    .unwrap_or("1"),
            )?,
            set_string(
                "ChannelBaseline",
                DicomVr::DS,
                calibration.map(|c| c.baseline.as_str()).unwrap_or("0"),
            )?,
            set_string(
                "ChannelTimeSkew",
                DicomVr::DS,
                calibration
                    .map(|c| c.time_skew_seconds.as_str())
                    .unwrap_or("0"),
            )?,
            set_unsigned("WaveformBitsStored", DicomVr::US, 16)?,
        ],
    })
}

fn code_item(scheme: &str, value: &str, meaning: &str) -> Result<AttributeItem, WaveformPlanError> {
    Ok(AttributeItem {
        attributes: vec![
            set_string("CodeValue", DicomVr::SH, value)?,
            set_string("CodingSchemeDesignator", DicomVr::SH, scheme)?,
            set_string("CodeMeaning", DicomVr::LO, meaning)?,
        ],
    })
}

fn common_attributes(
    input: &WaveformPlanInput,
    ids: &Identities,
    waveform_items: Vec<AttributeItem>,
) -> Result<Vec<ResolvedAttribute>, WaveformPlanError> {
    let m = input.caller_metadata.as_ref();
    let mut attributes = Vec::new();
    for (keyword, vr, value) in [
        ("SOPClassUID", DicomVr::UI, input.sop_class_uid.as_str()),
        ("SOPInstanceUID", DicomVr::UI, ids.sop.as_str()),
        ("SyntheticData", DicomVr::CS, "YES"),
        (
            "PatientName",
            DicomVr::PN,
            m.map(|m| m.patient_name.as_str())
                .unwrap_or("DTS^Synthetic^Patient001"),
        ),
        (
            "PatientID",
            DicomVr::LO,
            m.map(|m| m.patient_id.as_str())
                .unwrap_or("DTS-PATIENT-001"),
        ),
        (
            "PatientBirthDate",
            DicomVr::DA,
            m.map(|m| m.patient_birth_date.as_str())
                .unwrap_or("19700101"),
        ),
        (
            "PatientSex",
            DicomVr::CS,
            m.map(|m| m.patient_sex.as_str()).unwrap_or("O"),
        ),
        ("StudyInstanceUID", DicomVr::UI, ids.study.as_str()),
        (
            "StudyDate",
            DicomVr::DA,
            m.map(|m| m.study_date.as_str()).unwrap_or("20260101"),
        ),
        (
            "StudyTime",
            DicomVr::TM,
            m.map(|m| m.study_time.as_str()).unwrap_or("000000"),
        ),
        (
            "ReferringPhysicianName",
            DicomVr::PN,
            m.map(|m| m.referring_physician_name.as_str()).unwrap_or(""),
        ),
        ("StudyID", DicomVr::SH, input.study_id.as_str()),
        (
            "AccessionNumber",
            DicomVr::SH,
            m.map(|m| m.accession_number.as_str()).unwrap_or(""),
        ),
        ("Modality", DicomVr::CS, input.modality.as_str()),
        ("SeriesInstanceUID", DicomVr::UI, ids.series.as_str()),
        ("SeriesNumber", DicomVr::IS, input.series_number.as_str()),
        (
            "Manufacturer",
            DicomVr::LO,
            m.map(|m| m.manufacturer.as_str())
                .unwrap_or("dicom-test-suite"),
        ),
        (
            "InstitutionName",
            DicomVr::LO,
            m.map(|m| m.institution_name.as_str()).unwrap_or(""),
        ),
        (
            "InstitutionAddress",
            DicomVr::ST,
            m.map(|m| m.institution_address.as_str()).unwrap_or(""),
        ),
        (
            "ManufacturerModelName",
            DicomVr::LO,
            input.manufacturer_model_name.as_str(),
        ),
        (
            "DeviceSerialNumber",
            DicomVr::LO,
            input.device_serial_number.as_str(),
        ),
        (
            "SoftwareVersions",
            DicomVr::LO,
            m.map(|m| m.software_versions.as_str())
                .unwrap_or(BYTE_STABLE_OUTPUT_VERSION),
        ),
        (
            "InstanceNumber",
            DicomVr::IS,
            m.map(|m| m.instance_number.as_str()).unwrap_or("1"),
        ),
        (
            "ContentDate",
            DicomVr::DA,
            m.map(|m| m.content_date.as_str()).unwrap_or("20260101"),
        ),
        (
            "ContentTime",
            DicomVr::TM,
            m.map(|m| m.content_time.as_str()).unwrap_or("000000"),
        ),
        (
            "AcquisitionDateTime",
            DicomVr::DT,
            m.map(|m| m.acquisition_datetime.as_str())
                .unwrap_or("20260101000000"),
        ),
    ] {
        attributes.push(resolved_string(keyword, vr, value, ids)?);
    }
    attributes.push(ResolvedAttribute {
        address: address("AcquisitionContextSequence")?,
        vr: DicomVr::SQ,
        value: Some(AttributeValue::Sequence(vec![])),
        origin: ValueOrigin::TemplateDefault,
    });
    attributes.push(ResolvedAttribute {
        address: address("WaveformSequence")?,
        vr: DicomVr::SQ,
        value: Some(AttributeValue::Sequence(waveform_items)),
        origin: ValueOrigin::TemplateDefault,
    });
    attributes.sort_by(|left, right| left.address.cmp(&right.address));
    Ok(attributes)
}

fn resolved_string(
    keyword: &str,
    vr: DicomVr,
    value: &str,
    ids: &Identities,
) -> Result<ResolvedAttribute, WaveformPlanError> {
    let structural = matches!(
        keyword,
        "SOPInstanceUID" | "StudyInstanceUID" | "SeriesInstanceUID"
    );
    let _ = ids;
    Ok(ResolvedAttribute {
        address: address(keyword)?,
        vr,
        value: (!value.is_empty())
            .then(|| AttributeValue::Primitive(PrimitiveValue::String(value.to_owned()))),
        origin: if structural {
            ValueOrigin::DerivedStructural
        } else {
            ValueOrigin::TemplateDefault
        },
    })
}

fn native_binding(
    group: &WaveformGroupInput,
    content: &CanonicalContent,
) -> Result<SlotExecutionBinding, WaveformPlanError> {
    let bytes = match &content.materialization {
        Some(crate::composition::ContentMaterialization::Inline(bytes)) => bytes.clone(),
        _ => {
            return Err(WaveformPlanError::Content(
                "waveform bytes are not inline".into(),
            ));
        }
    };
    Ok(SlotExecutionBinding::NativeFrames {
        frames: vec![NativeFrameBinding {
            frame_number: 1,
            bytes: ByteBinding::Inline {
                sha256: sha256_hex(&bytes),
                bytes,
            },
            rows: 1,
            columns: group.samples_per_channel,
            samples_per_pixel: u16::try_from(group.channels.len())
                .map_err(|_| WaveformPlanError::ResourceOverflow)?,
            bits_allocated: 16,
            photometric_interpretation: "WAVEFORM".into(),
        }],
    })
}

fn address(keyword: &str) -> Result<AttributeAddress, WaveformPlanError> {
    AttributeAddress::from_keyword(keyword)
        .map_err(|error| WaveformPlanError::Attribute(error.to_string()))
}

fn set_string(
    keyword: &str,
    vr: DicomVr,
    value: &str,
) -> Result<AttributeOperation, WaveformPlanError> {
    Ok(AttributeOperation::Set {
        address: address(keyword)?,
        vr,
        value: AttributeValue::Primitive(PrimitiveValue::String(value.into())),
    })
}

fn set_unsigned(
    keyword: &str,
    vr: DicomVr,
    value: u64,
) -> Result<AttributeOperation, WaveformPlanError> {
    Ok(AttributeOperation::Set {
        address: address(keyword)?,
        vr,
        value: AttributeValue::Primitive(PrimitiveValue::Unsigned(value)),
    })
}

fn set_sequence(
    keyword: &str,
    items: Vec<AttributeItem>,
) -> Result<AttributeOperation, WaveformPlanError> {
    Ok(AttributeOperation::Set {
        address: address(keyword)?,
        vr: DicomVr::SQ,
        value: AttributeValue::Sequence(items),
    })
}

struct Identities {
    study: String,
    series: String,
    sop: String,
    implementation: String,
}

impl Identities {
    fn from_context(context: &TypedBulkPlanningContext) -> Result<Self, WaveformPlanError> {
        let get = |role| {
            context
                .identities
                .get(&role, 0)
                .map(str::to_owned)
                .ok_or(WaveformPlanError::Identity(role))
        };
        Ok(Self {
            study: get(CompositionUidRole::StudyInstance)?,
            series: get(CompositionUidRole::SeriesInstance)?,
            sop: get(CompositionUidRole::SopInstance)?,
            implementation: get(CompositionUidRole::ImplementationClass)?,
        })
    }
}

fn encoding(implementation: &str) -> EncodingPlan {
    EncodingPlan {
        transfer_syntax_uid: "1.2.840.10008.1.2.1".into(),
        sequence_length: SequenceLengthPolicy::WriterDefault,
        item_length: ItemLengthPolicy::WriterDefault,
        fragmentation: FragmentationPolicy::Native,
        offset_table: OffsetTablePolicy::NotApplicable,
        preamble: PreamblePolicy::ZeroFilled,
        file_meta: FileMetaPolicy::Standard,
        implementation: ImplementationIdentityPlan {
            class_uid: implementation.into(),
            version_name: Some(IMPLEMENTATION_VERSION_NAME.into()),
        },
        backend_id: "dicom-rs.part10".into(),
    }
}

fn validation() -> ValidationPlan {
    ValidationPlan {
        rules: [
            "validation.waveform.topology",
            "validation.waveform.samples",
            "validation.content.integrity",
        ]
        .into_iter()
        .map(|rule_id| ValidationRule {
            rule_id: rule_id.into(),
            requirement: ValidationRequirement::Required,
            parameters: BTreeMap::new(),
        })
        .collect(),
    }
}

fn evidence(artifact_id: &str) -> EvidencePlan {
    EvidencePlan {
        obligations: vec![EvidenceObligation {
            obligation_id: format!("same-project:{artifact_id}"),
            route_id: "builtin.strict".into(),
            independence: EvidenceIndependence::SameProject,
            required: true,
            parameters: BTreeMap::new(),
        }],
    }
}

#[derive(Debug)]
pub enum WaveformPlanError {
    Recipe(String),
    Context(String),
    Shape,
    ResourceOverflow,
    DeclaredDigest(String),
    Content(String),
    Attribute(String),
    Template(String),
    Identity(CompositionUidRole),
}

impl fmt::Display for WaveformPlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Recipe(message)
            | Self::Context(message)
            | Self::Content(message)
            | Self::Attribute(message)
            | Self::Template(message) => formatter.write_str(message),
            Self::Shape => formatter.write_str("invalid waveform group shape"),
            Self::ResourceOverflow => formatter.write_str("waveform resource arithmetic overflow"),
            Self::DeclaredDigest(slot) => {
                write!(
                    formatter,
                    "waveform slot `{slot}` differs from its declared bytes"
                )
            }
            Self::Identity(role) => write!(formatter, "missing waveform identity {role:?}"),
        }
    }
}

impl std::error::Error for WaveformPlanError {}
