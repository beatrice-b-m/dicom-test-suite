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
use crate::{IMPLEMENTATION_VERSION_NAME, PACKAGE_VERSION};

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
    Ok(Some(WaveformPlanInput {
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
    }))
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

fn validate_input(input: &WaveformPlanInput) -> Result<(), WaveformPlanError> {
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
                .and_then(|value| value.checked_mul(u64::from(group.formula_group_index + 1)))
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
            set_string("ChannelSensitivity", DicomVr::DS, "1")?,
            set_sequence(
                "ChannelSensitivityUnitsSequence",
                vec![code_item("UCUM", "uV", "microvolt")?],
            )?,
            set_string("ChannelSensitivityCorrectionFactor", DicomVr::DS, "1")?,
            set_string("ChannelBaseline", DicomVr::DS, "0")?,
            set_string("ChannelTimeSkew", DicomVr::DS, "0")?,
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
    let mut attributes = Vec::new();
    for (keyword, vr, value) in [
        ("SOPClassUID", DicomVr::UI, input.sop_class_uid.as_str()),
        ("SOPInstanceUID", DicomVr::UI, ids.sop.as_str()),
        ("SyntheticData", DicomVr::CS, "YES"),
        ("PatientName", DicomVr::PN, "DTS^Synthetic^Patient001"),
        ("PatientID", DicomVr::LO, "DTS-PATIENT-001"),
        ("PatientBirthDate", DicomVr::DA, "19700101"),
        ("PatientSex", DicomVr::CS, "O"),
        ("StudyInstanceUID", DicomVr::UI, ids.study.as_str()),
        ("StudyDate", DicomVr::DA, "20260101"),
        ("StudyTime", DicomVr::TM, "000000"),
        ("ReferringPhysicianName", DicomVr::PN, ""),
        ("StudyID", DicomVr::SH, input.study_id.as_str()),
        ("AccessionNumber", DicomVr::SH, ""),
        ("Modality", DicomVr::CS, input.modality.as_str()),
        ("SeriesInstanceUID", DicomVr::UI, ids.series.as_str()),
        ("SeriesNumber", DicomVr::IS, input.series_number.as_str()),
        ("Manufacturer", DicomVr::LO, "dicom-test-suite"),
        ("InstitutionName", DicomVr::LO, ""),
        ("InstitutionAddress", DicomVr::ST, ""),
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
        ("SoftwareVersions", DicomVr::LO, PACKAGE_VERSION),
        ("InstanceNumber", DicomVr::IS, "1"),
        ("ContentDate", DicomVr::DA, "20260101"),
        ("ContentTime", DicomVr::TM, "000000"),
        ("AcquisitionDateTime", DicomVr::DT, "20260101000000"),
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
