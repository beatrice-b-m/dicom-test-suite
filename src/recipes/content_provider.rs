use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::composition::{
    AttributeAddress, AttributeItem, AttributeOperation, AttributeValue, CanonicalContent,
    ContentMaterialization, ContentPlacement, DicomVr, PrimitiveValue,
};
use crate::sha256_hex;

const ABSOLUTE_MAX_ELEMENTS: u64 = 64 * 1024 * 1024;
const ABSOLUTE_MAX_OUTPUT_BYTES: u64 = 256 * 1024 * 1024;
const ABSOLUTE_MAX_ATTRIBUTE_OPERATIONS: u32 = 4096;
const ABSOLUTE_MAX_REFERENCES: u32 = 1024;
const ABSOLUTE_MAX_TEXT_BYTES: u64 = 4 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContentProviderLimits {
    pub max_elements: u64,
    pub max_output_bytes: u64,
    pub max_attribute_operations: u32,
    pub max_references: u32,
    pub max_text_bytes: u64,
}

impl Default for ContentProviderLimits {
    fn default() -> Self {
        Self {
            max_elements: 16 * 1024 * 1024,
            max_output_bytes: 64 * 1024 * 1024,
            max_attribute_operations: 1024,
            max_references: 256,
            max_text_bytes: 1024 * 1024,
        }
    }
}

impl ContentProviderLimits {
    pub fn validate(self) -> Result<Self, ContentProviderError> {
        for (field, value, ceiling) in [
            ("max_elements", self.max_elements, ABSOLUTE_MAX_ELEMENTS),
            (
                "max_output_bytes",
                self.max_output_bytes,
                ABSOLUTE_MAX_OUTPUT_BYTES,
            ),
            (
                "max_attribute_operations",
                u64::from(self.max_attribute_operations),
                u64::from(ABSOLUTE_MAX_ATTRIBUTE_OPERATIONS),
            ),
            (
                "max_references",
                u64::from(self.max_references),
                u64::from(ABSOLUTE_MAX_REFERENCES),
            ),
            (
                "max_text_bytes",
                self.max_text_bytes,
                ABSOLUTE_MAX_TEXT_BYTES,
            ),
        ] {
            if value == 0 || value > ceiling {
                return Err(ContentProviderError::InvalidLimit {
                    field,
                    value,
                    ceiling,
                });
            }
        }
        Ok(self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ContentProviderRequest {
    IntegerPixels(IntegerPixelsContract),
    FloatPixels(FloatPixelsContract),
    Waveform(WaveformContract),
    EncapsulatedDocument(BytePayloadContract),
    Mesh(MeshContract),
    StructuredReport(StructuredReportContract),
    RtObject(RtSemanticContract),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContentTarget {
    pub slot: String,
    pub content_kind: String,
    pub address: AttributeAddress,
    pub vr: DicomVr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ByteOrder {
    LittleEndian,
    BigEndian,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "signedness", rename_all = "snake_case", deny_unknown_fields)]
pub enum IntegerSamples {
    Signed { values: Vec<i64> },
    Unsigned { values: Vec<u64> },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntegerPixelsContract {
    pub target: ContentTarget,
    pub dimensions: Vec<u32>,
    pub bits_allocated: u8,
    pub byte_order: ByteOrder,
    pub samples: IntegerSamples,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "precision", rename_all = "snake_case", deny_unknown_fields)]
pub enum FloatSamples {
    F32Bits { values: Vec<u32> },
    F64Bits { values: Vec<u64> },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FloatPixelsContract {
    pub target: ContentTarget,
    pub dimensions: Vec<u32>,
    pub byte_order: ByteOrder,
    pub samples: FloatSamples,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WaveformContract {
    pub target: ContentTarget,
    pub channels: u32,
    pub samples_per_channel: u32,
    pub bits_allocated: u8,
    pub byte_order: ByteOrder,
    pub samples: IntegerSamples,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BytePayloadContract {
    pub target: ContentTarget,
    pub media_type: String,
    pub declared_size_bytes: u64,
    pub declared_sha256: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MeshFormat {
    BinaryStl,
    Utf8Obj,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MeshContract {
    pub target: ContentTarget,
    pub format: MeshFormat,
    pub declared_size_bytes: u64,
    pub declared_sha256: String,
    pub triangle_count: Option<u32>,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CodedConcept {
    pub code_value: String,
    pub coding_scheme_designator: String,
    pub code_meaning: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SemanticReferenceRole {
    Evidence,
    SourceImage,
    ReferencedPlan,
    ReferencedStructureSet,
    ReferencedDose,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticReference {
    pub role: SemanticReferenceRole,
    pub sop_class_uid: String,
    pub sop_instance_uid: String,
    #[serde(default)]
    pub frames: Vec<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum CompletionFlag {
    Partial,
    Complete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum VerificationFlag {
    Unverified,
    Verified,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StructuredReportContract {
    pub content_date: String,
    pub content_time: String,
    pub completion_flag: CompletionFlag,
    pub verification_flag: VerificationFlag,
    pub concept_name: CodedConcept,
    #[serde(default)]
    pub references: Vec<SemanticReference>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RtObjectKind {
    StructureSet,
    Plan,
    Dose,
    Image,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RtSemanticContract {
    pub object_kind: RtObjectKind,
    pub label: String,
    pub instance_number: u32,
    #[serde(default)]
    pub references: Vec<SemanticReference>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentDigest {
    pub slot: String,
    pub size_bytes: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ContentProviderOutput {
    pub contents: Vec<CanonicalContent>,
    pub attribute_operations: Vec<AttributeOperation>,
    pub digests: Vec<ContentDigest>,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct NeutralContentProvider;

impl NeutralContentProvider {
    pub fn expand(
        &self,
        request: &ContentProviderRequest,
        limits: ContentProviderLimits,
    ) -> Result<ContentProviderOutput, ContentProviderError> {
        let limits = limits.validate()?;
        let output = match request {
            ContentProviderRequest::IntegerPixels(contract) => {
                let count = dimension_product(&contract.dimensions, limits.max_elements)?;
                let bytes = encode_integer_samples(
                    &contract.samples,
                    contract.bits_allocated,
                    contract.byte_order,
                    count,
                    limits.max_output_bytes,
                )?;
                content_output(&contract.target, bytes, BTreeMap::new())?
            }
            ContentProviderRequest::FloatPixels(contract) => {
                let count = dimension_product(&contract.dimensions, limits.max_elements)?;
                let bytes = encode_float_samples(
                    &contract.samples,
                    contract.byte_order,
                    count,
                    limits.max_output_bytes,
                )?;
                content_output(&contract.target, bytes, BTreeMap::new())?
            }
            ContentProviderRequest::Waveform(contract) => {
                let count = u64::from(contract.channels)
                    .checked_mul(u64::from(contract.samples_per_channel))
                    .ok_or(ContentProviderError::ArithmeticOverflow)?;
                if contract.channels == 0
                    || contract.samples_per_channel == 0
                    || count > limits.max_elements
                {
                    return Err(ContentProviderError::ElementCount {
                        declared: count,
                        limit: limits.max_elements,
                    });
                }
                let bytes = encode_integer_samples(
                    &contract.samples,
                    contract.bits_allocated,
                    contract.byte_order,
                    count,
                    limits.max_output_bytes,
                )?;
                let mut properties = BTreeMap::new();
                properties.insert("channels".into(), contract.channels.to_string());
                properties.insert(
                    "samples_per_channel".into(),
                    contract.samples_per_channel.to_string(),
                );
                properties.insert("multiplex_order".into(), "sample_then_channel".into());
                content_output(&contract.target, bytes, properties)?
            }
            ContentProviderRequest::EncapsulatedDocument(contract) => {
                validate_declared_payload(
                    contract.declared_size_bytes,
                    &contract.declared_sha256,
                    &contract.bytes,
                    limits.max_output_bytes,
                )?;
                validate_text("media_type", &contract.media_type, 128)?;
                enforce_semantic_text_limit(
                    std::iter::once(contract.media_type.as_str()),
                    limits.max_text_bytes,
                )?;
                let mut properties = BTreeMap::new();
                properties.insert("media_type".into(), contract.media_type.clone());
                content_output(&contract.target, contract.bytes.clone(), properties)?
            }
            ContentProviderRequest::Mesh(contract) => {
                validate_declared_payload(
                    contract.declared_size_bytes,
                    &contract.declared_sha256,
                    &contract.bytes,
                    limits.max_output_bytes,
                )?;
                validate_mesh(contract, limits.max_elements)?;
                let mut properties = BTreeMap::new();
                properties.insert(
                    "mesh_format".into(),
                    match contract.format {
                        MeshFormat::BinaryStl => "binary_stl",
                        MeshFormat::Utf8Obj => "utf8_obj",
                    }
                    .into(),
                );
                if let Some(count) = contract.triangle_count {
                    properties.insert("triangle_count".into(), count.to_string());
                }
                content_output(&contract.target, contract.bytes.clone(), properties)?
            }
            ContentProviderRequest::StructuredReport(contract) => ContentProviderOutput {
                contents: vec![],
                attribute_operations: structured_report_operations(contract, limits)?,
                digests: vec![],
            },
            ContentProviderRequest::RtObject(contract) => ContentProviderOutput {
                contents: vec![],
                attribute_operations: rt_operations(contract, limits)?,
                digests: vec![],
            },
        };
        if output.attribute_operations.len() as u32 > limits.max_attribute_operations {
            return Err(ContentProviderError::OperationCount {
                actual: output.attribute_operations.len() as u32,
                limit: limits.max_attribute_operations,
            });
        }
        Ok(output)
    }
}

fn dimension_product(dimensions: &[u32], limit: u64) -> Result<u64, ContentProviderError> {
    if dimensions.is_empty() || dimensions.len() > 8 || dimensions.iter().any(|value| *value == 0) {
        return Err(ContentProviderError::InvalidDimensions);
    }
    let count = dimensions.iter().try_fold(1_u64, |count, value| {
        count
            .checked_mul(u64::from(*value))
            .ok_or(ContentProviderError::ArithmeticOverflow)
    })?;
    if count > limit {
        return Err(ContentProviderError::ElementCount {
            declared: count,
            limit,
        });
    }
    Ok(count)
}

fn encode_integer_samples(
    samples: &IntegerSamples,
    bits: u8,
    order: ByteOrder,
    expected_count: u64,
    byte_limit: u64,
) -> Result<Vec<u8>, ContentProviderError> {
    if !matches!(bits, 8 | 16 | 32 | 64) {
        return Err(ContentProviderError::InvalidBitsAllocated(bits));
    }
    let actual_count = match samples {
        IntegerSamples::Signed { values } => values.len(),
        IntegerSamples::Unsigned { values } => values.len(),
    } as u64;
    if actual_count != expected_count {
        return Err(ContentProviderError::SampleCount {
            expected: expected_count,
            actual: actual_count,
        });
    }
    let byte_count = expected_count
        .checked_mul(u64::from(bits / 8))
        .ok_or(ContentProviderError::ArithmeticOverflow)?;
    enforce_byte_limit(byte_count, byte_limit)?;
    let mut output = Vec::with_capacity(byte_count as usize);
    match samples {
        IntegerSamples::Signed { values } => {
            for value in values {
                let (minimum, maximum) = signed_range(bits);
                if i128::from(*value) < minimum || i128::from(*value) > maximum {
                    return Err(ContentProviderError::IntegerRange);
                }
                append_integer(&mut output, *value as u64, bits, order);
            }
        }
        IntegerSamples::Unsigned { values } => {
            let maximum = unsigned_max(bits);
            for value in values {
                if u128::from(*value) > maximum {
                    return Err(ContentProviderError::IntegerRange);
                }
                append_integer(&mut output, *value, bits, order);
            }
        }
    }
    Ok(output)
}

fn encode_float_samples(
    samples: &FloatSamples,
    order: ByteOrder,
    expected_count: u64,
    byte_limit: u64,
) -> Result<Vec<u8>, ContentProviderError> {
    let (actual, width) = match samples {
        FloatSamples::F32Bits { values } => (values.len() as u64, 4_u64),
        FloatSamples::F64Bits { values } => (values.len() as u64, 8_u64),
    };
    if actual != expected_count {
        return Err(ContentProviderError::SampleCount {
            expected: expected_count,
            actual,
        });
    }
    let byte_count = actual
        .checked_mul(width)
        .ok_or(ContentProviderError::ArithmeticOverflow)?;
    enforce_byte_limit(byte_count, byte_limit)?;
    let mut output = Vec::with_capacity(byte_count as usize);
    match samples {
        FloatSamples::F32Bits { values } => values.iter().for_each(|value| match order {
            ByteOrder::LittleEndian => output.extend_from_slice(&value.to_le_bytes()),
            ByteOrder::BigEndian => output.extend_from_slice(&value.to_be_bytes()),
        }),
        FloatSamples::F64Bits { values } => values.iter().for_each(|value| match order {
            ByteOrder::LittleEndian => output.extend_from_slice(&value.to_le_bytes()),
            ByteOrder::BigEndian => output.extend_from_slice(&value.to_be_bytes()),
        }),
    }
    Ok(output)
}

fn append_integer(output: &mut Vec<u8>, value: u64, bits: u8, order: ByteOrder) {
    let bytes = match order {
        ByteOrder::LittleEndian => value.to_le_bytes(),
        ByteOrder::BigEndian => value.to_be_bytes(),
    };
    let width = usize::from(bits / 8);
    match order {
        ByteOrder::LittleEndian => output.extend_from_slice(&bytes[..width]),
        ByteOrder::BigEndian => output.extend_from_slice(&bytes[8 - width..]),
    }
}

fn signed_range(bits: u8) -> (i128, i128) {
    if bits == 64 {
        (i128::from(i64::MIN), i128::from(i64::MAX))
    } else {
        let magnitude = 1_i128 << (bits - 1);
        (-magnitude, magnitude - 1)
    }
}

fn unsigned_max(bits: u8) -> u128 {
    if bits == 64 {
        u128::from(u64::MAX)
    } else {
        (1_u128 << bits) - 1
    }
}

fn enforce_byte_limit(actual: u64, limit: u64) -> Result<(), ContentProviderError> {
    if actual > limit {
        Err(ContentProviderError::ByteLimit { actual, limit })
    } else {
        Ok(())
    }
}

fn validate_declared_payload(
    declared_size: u64,
    declared_sha256: &str,
    bytes: &[u8],
    limit: u64,
) -> Result<(), ContentProviderError> {
    enforce_byte_limit(bytes.len() as u64, limit)?;
    if declared_size != bytes.len() as u64 {
        return Err(ContentProviderError::DeclaredSize {
            declared: declared_size,
            actual: bytes.len() as u64,
        });
    }
    let actual = sha256_hex(bytes);
    if declared_sha256 != actual {
        return Err(ContentProviderError::DeclaredHash {
            declared: declared_sha256.into(),
            actual,
        });
    }
    Ok(())
}

fn validate_mesh(contract: &MeshContract, element_limit: u64) -> Result<(), ContentProviderError> {
    match contract.format {
        MeshFormat::BinaryStl => {
            let triangles = contract
                .triangle_count
                .ok_or(ContentProviderError::MeshContract)?;
            if u64::from(triangles) > element_limit {
                return Err(ContentProviderError::ElementCount {
                    declared: u64::from(triangles),
                    limit: element_limit,
                });
            }
            let expected = 84_u64
                .checked_add(
                    u64::from(triangles)
                        .checked_mul(50)
                        .ok_or(ContentProviderError::ArithmeticOverflow)?,
                )
                .ok_or(ContentProviderError::ArithmeticOverflow)?;
            if contract.bytes.len() as u64 != expected
                || contract.bytes.get(80..84) != Some(triangles.to_le_bytes().as_slice())
            {
                return Err(ContentProviderError::MeshContract);
            }
        }
        MeshFormat::Utf8Obj => {
            if contract.triangle_count.is_some()
                || std::str::from_utf8(&contract.bytes).is_err()
                || contract.bytes.is_empty()
            {
                return Err(ContentProviderError::MeshContract);
            }
        }
    }
    Ok(())
}

fn content_output(
    target: &ContentTarget,
    bytes: Vec<u8>,
    properties: BTreeMap<String, String>,
) -> Result<ContentProviderOutput, ContentProviderError> {
    validate_identifier("slot", &target.slot)?;
    validate_identifier("content_kind", &target.content_kind)?;
    if !matches!(
        target.vr,
        DicomVr::OB | DicomVr::OW | DicomVr::OF | DicomVr::OD | DicomVr::UN
    ) {
        return Err(ContentProviderError::InvalidContentVr(target.vr));
    }
    let digest = ContentDigest {
        slot: target.slot.clone(),
        size_bytes: bytes.len() as u64,
        sha256: sha256_hex(&bytes),
    };
    Ok(ContentProviderOutput {
        contents: vec![CanonicalContent {
            slot: target.slot.clone(),
            kind: target.content_kind.clone(),
            address: target.address.clone(),
            vr: target.vr,
            size_bytes: digest.size_bytes,
            sha256: digest.sha256.clone(),
            properties,
            placement: ContentPlacement::TopLevel,
            materialization: Some(ContentMaterialization::Inline(bytes)),
        }],
        attribute_operations: vec![],
        digests: vec![digest],
    })
}

fn structured_report_operations(
    contract: &StructuredReportContract,
    limits: ContentProviderLimits,
) -> Result<Vec<AttributeOperation>, ContentProviderError> {
    validate_date(&contract.content_date)?;
    validate_time(&contract.content_time)?;
    validate_code(&contract.concept_name)?;
    validate_references(&contract.references, limits.max_references, true)?;
    enforce_semantic_text_limit(
        [
            contract.content_date.as_str(),
            contract.content_time.as_str(),
            contract.concept_name.code_value.as_str(),
            contract.concept_name.coding_scheme_designator.as_str(),
            contract.concept_name.code_meaning.as_str(),
        ]
        .into_iter()
        .chain(reference_text(&contract.references)),
        limits.max_text_bytes,
    )?;
    let mut operations = vec![
        set_string("0008,0023", DicomVr::DA, &contract.content_date)?,
        set_string("0008,0033", DicomVr::TM, &contract.content_time)?,
        set_string(
            "0040,A491",
            DicomVr::CS,
            match contract.completion_flag {
                CompletionFlag::Partial => "PARTIAL",
                CompletionFlag::Complete => "COMPLETE",
            },
        )?,
        set_string(
            "0040,A493",
            DicomVr::CS,
            match contract.verification_flag {
                VerificationFlag::Unverified => "UNVERIFIED",
                VerificationFlag::Verified => "VERIFIED",
            },
        )?,
        code_sequence("0040,A043", &contract.concept_name)?,
    ];
    if !contract.references.is_empty() {
        operations.push(reference_sequence("0008,1140", &contract.references)?);
    }
    Ok(operations)
}

fn rt_operations(
    contract: &RtSemanticContract,
    limits: ContentProviderLimits,
) -> Result<Vec<AttributeOperation>, ContentProviderError> {
    validate_text("label", &contract.label, 64)?;
    if contract.instance_number == 0 {
        return Err(ContentProviderError::InvalidSemanticField(
            "instance_number",
        ));
    }
    validate_references(&contract.references, limits.max_references, false)?;
    enforce_semantic_text_limit(
        std::iter::once(contract.label.as_str()).chain(reference_text(&contract.references)),
        limits.max_text_bytes,
    )?;
    let (modality, label_tag, label_vr) = match contract.object_kind {
        RtObjectKind::StructureSet => ("RTSTRUCT", "3006,0002", DicomVr::SH),
        RtObjectKind::Plan => ("RTPLAN", "300A,0002", DicomVr::SH),
        RtObjectKind::Dose => ("RTDOSE", "3004,0006", DicomVr::LT),
        RtObjectKind::Image => ("RTIMAGE", "3002,0002", DicomVr::SH),
    };
    let mut operations = vec![
        set_string("0008,0060", DicomVr::CS, modality)?,
        set_string(label_tag, label_vr, &contract.label)?,
        set_string(
            "0020,0013",
            DicomVr::IS,
            &contract.instance_number.to_string(),
        )?,
    ];
    for (role, tag) in [
        (SemanticReferenceRole::ReferencedPlan, "300C,0002"),
        (SemanticReferenceRole::ReferencedStructureSet, "300C,0060"),
        (SemanticReferenceRole::ReferencedDose, "300C,0080"),
        (SemanticReferenceRole::SourceImage, "0008,1140"),
    ] {
        let selected = contract
            .references
            .iter()
            .filter(|reference| reference.role == role)
            .cloned()
            .collect::<Vec<_>>();
        if !selected.is_empty() {
            operations.push(reference_sequence(tag, &selected)?);
        }
    }
    Ok(operations)
}

fn validate_references(
    references: &[SemanticReference],
    limit: u32,
    sr: bool,
) -> Result<(), ContentProviderError> {
    if references.len() as u32 > limit {
        return Err(ContentProviderError::ReferenceCount {
            actual: references.len() as u32,
            limit,
        });
    }
    let mut identities = BTreeSet::new();
    for reference in references {
        if sr
            && !matches!(
                reference.role,
                SemanticReferenceRole::Evidence | SemanticReferenceRole::SourceImage
            )
        {
            return Err(ContentProviderError::ReferenceRole);
        }
        if !sr && reference.role == SemanticReferenceRole::Evidence {
            return Err(ContentProviderError::ReferenceRole);
        }
        validate_uid(&reference.sop_class_uid)?;
        validate_uid(&reference.sop_instance_uid)?;
        if !identities.insert((reference.role, reference.sop_instance_uid.as_str())) {
            return Err(ContentProviderError::DuplicateReference);
        }
        let mut frames = BTreeSet::new();
        if reference
            .frames
            .iter()
            .any(|frame| *frame == 0 || !frames.insert(*frame))
        {
            return Err(ContentProviderError::InvalidReferenceFrames);
        }
    }
    Ok(())
}

fn reference_text(references: &[SemanticReference]) -> impl Iterator<Item = &str> {
    references.iter().flat_map(|reference| {
        [
            reference.sop_class_uid.as_str(),
            reference.sop_instance_uid.as_str(),
        ]
    })
}

fn enforce_semantic_text_limit<'a>(
    mut values: impl Iterator<Item = &'a str>,
    limit: u64,
) -> Result<(), ContentProviderError> {
    let actual = values.try_fold(0_u64, |total, value| {
        total
            .checked_add(value.len() as u64)
            .ok_or(ContentProviderError::ArithmeticOverflow)
    })?;
    if actual > limit {
        Err(ContentProviderError::TextLimit { actual, limit })
    } else {
        Ok(())
    }
}

fn reference_sequence(
    tag: &str,
    references: &[SemanticReference],
) -> Result<AttributeOperation, ContentProviderError> {
    let items = references
        .iter()
        .map(|reference| {
            let mut attributes = vec![
                set_string("0008,1150", DicomVr::UI, &reference.sop_class_uid)?,
                set_string("0008,1155", DicomVr::UI, &reference.sop_instance_uid)?,
            ];
            if !reference.frames.is_empty() {
                attributes.push(set_string(
                    "0008,1160",
                    DicomVr::IS,
                    &reference
                        .frames
                        .iter()
                        .map(u32::to_string)
                        .collect::<Vec<_>>()
                        .join("\\"),
                )?);
            }
            Ok(AttributeItem { attributes })
        })
        .collect::<Result<Vec<_>, ContentProviderError>>()?;
    Ok(AttributeOperation::Set {
        address: address(tag)?,
        vr: DicomVr::SQ,
        value: AttributeValue::Sequence(items),
    })
}

fn code_sequence(
    tag: &str,
    code: &CodedConcept,
) -> Result<AttributeOperation, ContentProviderError> {
    Ok(AttributeOperation::Set {
        address: address(tag)?,
        vr: DicomVr::SQ,
        value: AttributeValue::Sequence(vec![AttributeItem {
            attributes: vec![
                set_string("0008,0100", DicomVr::SH, &code.code_value)?,
                set_string("0008,0102", DicomVr::SH, &code.coding_scheme_designator)?,
                set_string("0008,0104", DicomVr::LO, &code.code_meaning)?,
            ],
        }]),
    })
}

fn set_string(
    tag: &str,
    vr: DicomVr,
    value: &str,
) -> Result<AttributeOperation, ContentProviderError> {
    Ok(AttributeOperation::Set {
        address: address(tag)?,
        vr,
        value: AttributeValue::Primitive(PrimitiveValue::String(value.into())),
    })
}

fn address(tag: &str) -> Result<AttributeAddress, ContentProviderError> {
    AttributeAddress::from_normalized_tag(tag)
        .map_err(|_| ContentProviderError::InvalidSemanticField("tag"))
}

fn validate_identifier(field: &'static str, value: &str) -> Result<(), ContentProviderError> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
    {
        return Err(ContentProviderError::InvalidIdentifier(field));
    }
    Ok(())
}

fn validate_uid(value: &str) -> Result<(), ContentProviderError> {
    if value.is_empty()
        || value.len() > 64
        || value.starts_with('.')
        || value.ends_with('.')
        || value.split('.').any(|component| {
            component.is_empty() || !component.bytes().all(|byte| byte.is_ascii_digit())
        })
    {
        return Err(ContentProviderError::InvalidUid);
    }
    Ok(())
}

fn validate_date(value: &str) -> Result<(), ContentProviderError> {
    if value.len() != 8 || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        Err(ContentProviderError::InvalidSemanticField("content_date"))
    } else {
        Ok(())
    }
}

fn validate_time(value: &str) -> Result<(), ContentProviderError> {
    if value.len() < 6
        || value.len() > 16
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || byte == b'.')
    {
        Err(ContentProviderError::InvalidSemanticField("content_time"))
    } else {
        Ok(())
    }
}

fn validate_code(code: &CodedConcept) -> Result<(), ContentProviderError> {
    validate_text("code_value", &code.code_value, 16)?;
    validate_text(
        "coding_scheme_designator",
        &code.coding_scheme_designator,
        16,
    )?;
    validate_text("code_meaning", &code.code_meaning, 64)
}

fn validate_text(
    field: &'static str,
    value: &str,
    field_limit: usize,
) -> Result<(), ContentProviderError> {
    if value.is_empty() || value.len() > field_limit || value.chars().any(char::is_control) {
        Err(ContentProviderError::InvalidSemanticField(field))
    } else {
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContentProviderError {
    InvalidLimit {
        field: &'static str,
        value: u64,
        ceiling: u64,
    },
    InvalidDimensions,
    ArithmeticOverflow,
    ElementCount {
        declared: u64,
        limit: u64,
    },
    SampleCount {
        expected: u64,
        actual: u64,
    },
    InvalidBitsAllocated(u8),
    IntegerRange,
    ByteLimit {
        actual: u64,
        limit: u64,
    },
    DeclaredSize {
        declared: u64,
        actual: u64,
    },
    DeclaredHash {
        declared: String,
        actual: String,
    },
    MeshContract,
    InvalidIdentifier(&'static str),
    InvalidContentVr(DicomVr),
    InvalidSemanticField(&'static str),
    InvalidUid,
    ReferenceCount {
        actual: u32,
        limit: u32,
    },
    TextLimit {
        actual: u64,
        limit: u64,
    },
    ReferenceRole,
    DuplicateReference,
    InvalidReferenceFrames,
    OperationCount {
        actual: u32,
        limit: u32,
    },
}

impl fmt::Display for ContentProviderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for ContentProviderError {}
