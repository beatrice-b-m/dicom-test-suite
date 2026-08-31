use std::collections::BTreeSet;
use std::fmt;
use std::path::{Component, Path};
use std::str::FromStr;

use base64::Engine;
use dicom_core::dictionary::{DataDictionary, DataDictionaryEntry, VirtualVr};
use dicom_dictionary_std::StandardDataDictionary;
use serde::{Deserialize, Serialize};

use crate::composition::{AttributeAddress, DicomVr};

pub const ASSEMBLY_REQUEST_SCHEMA_VERSION: &str = "1.0.0";
const REQUEST_SCHEMA: &str = include_str!("../../schemas/assembly-request.schema.json");
const DEFAULT_TRANSFER_SYNTAX: &str = "1.2.840.10008.1.2.1";

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct AssemblyRequest {
    pub assembly_request_schema_version: String,
    pub instances: Vec<AssemblyInstance>,
    #[serde(default)]
    pub limits: AssemblyLimits,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssemblyLimits {
    #[serde(default = "default_max_instances")]
    pub max_instances: usize,
    #[serde(default = "default_max_elements")]
    pub max_elements_per_instance: usize,
    #[serde(default = "default_max_depth")]
    pub max_sequence_depth: usize,
    #[serde(default = "default_max_parallelism")]
    pub max_parallelism: u32,
    #[serde(default = "default_max_value_bytes")]
    pub max_value_bytes: u64,
    #[serde(default = "default_max_output_bytes")]
    pub max_output_bytes: u64,
}

const fn default_max_instances() -> usize {
    1024
}
const fn default_max_elements() -> usize {
    4096
}
const fn default_max_depth() -> usize {
    16
}
const fn default_max_parallelism() -> u32 {
    256
}
const fn default_max_value_bytes() -> u64 {
    268_435_456
}
const fn default_max_output_bytes() -> u64 {
    1_073_741_824
}

impl Default for AssemblyLimits {
    fn default() -> Self {
        Self {
            max_instances: default_max_instances(),
            max_elements_per_instance: default_max_elements(),
            max_sequence_depth: default_max_depth(),
            max_parallelism: default_max_parallelism(),
            max_value_bytes: default_max_value_bytes(),
            max_output_bytes: default_max_output_bytes(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct AssemblyInstance {
    pub instance_id: String,
    pub sop_class_uid: String,
    #[serde(default = "default_transfer_syntax")]
    pub transfer_syntax_uid: String,
    pub modality: Option<String>,
    pub output_path: Option<String>,
    #[serde(default)]
    pub identity: AssemblyIdentity,
    pub elements: Vec<AssemblyElement>,
    #[serde(default)]
    pub bulk: Vec<AssemblyBulk>,
    #[serde(default)]
    pub references: Vec<AssemblyReference>,
}

fn default_transfer_syntax() -> String {
    DEFAULT_TRANSFER_SYNTAX.to_string()
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct AssemblyIdentity {
    pub study_instance_uid: Option<String>,
    pub series_instance_uid: Option<String>,
    pub sop_instance_uid: Option<String>,
    pub frame_of_reference_uid: Option<String>,
    pub study_scope: Option<String>,
    pub series_scope: Option<String>,
    pub frame_of_reference_scope: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct AssemblyElement {
    pub address: AssemblyAddress,
    pub vr: Option<DicomVr>,
    pub value: AssemblyValue,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(untagged)]
pub enum AssemblyAddress {
    Keyword {
        keyword: String,
    },
    Tag {
        tag: String,
    },
    Private {
        private_group: String,
        private_creator: String,
        private_offset: String,
    },
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AssemblyValue {
    Empty,
    String { value: String },
    Strings { values: Vec<String> },
    Integer { value: i64 },
    Integers { values: Vec<i64> },
    Float { value: f64 },
    Floats { values: Vec<f64> },
    Tag { value: String },
    Tags { values: Vec<String> },
    Bytes { base64: String },
    Sequence { items: Vec<SequenceItem> },
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct SequenceItem {
    pub elements: Vec<AssemblyElement>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BulkSource {
    InlineBase64 {
        base64: String,
        sha256: Option<String>,
    },
    File {
        path: String,
        sha256: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct AssemblyBulk {
    pub kind: String,
    pub tag: Option<String>,
    pub vr: Option<DicomVr>,
    pub source: BulkSource,
    pub rows: Option<u32>,
    pub columns: Option<u32>,
    pub frames: Option<u32>,
    pub samples_per_pixel: Option<u8>,
    pub bits_allocated: Option<u8>,
    pub bits_stored: Option<u8>,
    pub signed: Option<bool>,
    pub photometric_interpretation: Option<String>,
    pub channels: Option<u32>,
    pub samples: Option<u32>,
    pub media_type: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct AssemblyReference {
    pub relationship: String,
    pub target_instance_id: String,
    pub target_role: ReferenceRole,
    pub frames: Option<Vec<u32>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReferenceRole {
    Sop,
    Series,
    Study,
    FrameOfReference,
}

impl AssemblyRequest {
    pub fn from_slice(bytes: &[u8]) -> Result<Self, AssemblyError> {
        let raw: serde_json::Value = serde_json::from_slice(bytes)
            .map_err(|error| AssemblyError::Json(error.to_string()))?;
        preflight_raw_request(&raw)?;
        let schema: serde_json::Value = serde_json::from_str(REQUEST_SCHEMA)
            .map_err(|error| AssemblyError::Schema(error.to_string()))?;
        let validator = jsonschema::options()
            .with_draft(jsonschema::Draft::Draft202012)
            .build(&schema)
            .map_err(|error| AssemblyError::Schema(error.to_string()))?;
        if let Err(error) = validator.validate(&raw) {
            return Err(AssemblyError::Schema(error.to_string()));
        }
        let request: Self =
            serde_json::from_value(raw).map_err(|error| AssemblyError::Json(error.to_string()))?;
        request.validate()?;
        Ok(request)
    }

    pub fn validate(&self) -> Result<(), AssemblyError> {
        if self.assembly_request_schema_version != ASSEMBLY_REQUEST_SCHEMA_VERSION {
            return Err(AssemblyError::UnsupportedVersion(
                self.assembly_request_schema_version.clone(),
            ));
        }
        if self.instances.len() > self.limits.max_instances {
            return Err(AssemblyError::Limit("instance count"));
        }
        let mut ids = BTreeSet::new();
        let mut paths = BTreeSet::new();
        for instance in &self.instances {
            if !ids.insert(instance.instance_id.clone()) {
                return Err(AssemblyError::DuplicateInstance(
                    instance.instance_id.clone(),
                ));
            }
            let path = instance
                .output_path
                .clone()
                .unwrap_or_else(|| format!("instances/{}.dcm", instance.instance_id));
            validate_relative_path(&path)?;
            if !paths.insert(path.clone()) {
                return Err(AssemblyError::DuplicatePath(path));
            }
            if instance.elements.len() > self.limits.max_elements_per_instance {
                return Err(AssemblyError::Limit("element count"));
            }
            if !matches!(
                instance.transfer_syntax_uid.as_str(),
                "1.2.840.10008.1.2" | "1.2.840.10008.1.2.1"
            ) {
                return Err(AssemblyError::TransferSyntax(
                    instance.transfer_syntax_uid.clone(),
                ));
            }
            let mut element_count = 0_usize;
            validate_elements(&instance.elements, 0, &self.limits, &mut element_count)?;
            validate_bulk(instance, &self.limits)?;
            validate_bulk_ownership(instance)?;
        }
        for instance in &self.instances {
            for reference in &instance.references {
                if !ids.contains(&reference.target_instance_id) {
                    return Err(AssemblyError::MissingReference(
                        reference.target_instance_id.clone(),
                    ));
                }
                if let Some(frames) = &reference.frames {
                    let target = self
                        .instances
                        .iter()
                        .find(|candidate| candidate.instance_id == reference.target_instance_id)
                        .expect("target existence checked above");
                    let target_frames =
                        target.bulk.iter().find_map(|bulk| bulk.frames).unwrap_or(1);
                    if frames.iter().any(|frame| *frame > target_frames) {
                        return Err(AssemblyError::Value(format!(
                            "referenced frame exceeds target frame count {target_frames}"
                        )));
                    }
                }
            }
        }
        Ok(())
    }
}

fn preflight_raw_request(raw: &serde_json::Value) -> Result<(), AssemblyError> {
    if let Some(version) = raw
        .get("assembly_request_schema_version")
        .and_then(serde_json::Value::as_str)
    {
        if version != ASSEMBLY_REQUEST_SCHEMA_VERSION {
            return Err(AssemblyError::UnsupportedVersion(version.to_owned()));
        }
    }
    let Some(instances) = raw.get("instances").and_then(serde_json::Value::as_array) else {
        return Ok(());
    };
    for instance in instances {
        if let Some(path) = instance
            .get("output_path")
            .and_then(serde_json::Value::as_str)
        {
            validate_relative_path(path)?;
        }
        if let Some(transfer_syntax) = instance
            .get("transfer_syntax_uid")
            .and_then(serde_json::Value::as_str)
        {
            if !matches!(transfer_syntax, "1.2.840.10008.1.2" | "1.2.840.10008.1.2.1") {
                return Err(AssemblyError::TransferSyntax(transfer_syntax.to_owned()));
            }
        }
        if let Some(bulk) = instance.get("bulk").and_then(serde_json::Value::as_array) {
            for declaration in bulk {
                if declaration
                    .pointer("/source/kind")
                    .and_then(serde_json::Value::as_str)
                    == Some("file")
                {
                    if let Some(path) = declaration
                        .pointer("/source/path")
                        .and_then(serde_json::Value::as_str)
                    {
                        validate_relative_path(path)?;
                    }
                }
            }
        }
    }
    Ok(())
}

fn validate_elements(
    elements: &[AssemblyElement],
    depth: usize,
    limits: &AssemblyLimits,
    element_count: &mut usize,
) -> Result<(), AssemblyError> {
    if depth > limits.max_sequence_depth {
        return Err(AssemblyError::Limit("Sequence depth"));
    }
    let mut addresses = BTreeSet::new();
    for element in elements {
        *element_count = element_count
            .checked_add(1)
            .ok_or(AssemblyError::Limit("element count"))?;
        if *element_count > limits.max_elements_per_instance {
            return Err(AssemblyError::Limit("element count"));
        }
        let (address, inferred_vr) = resolve_address(&element.address)?;
        let normalized = address.normalized_tag();
        if !addresses.insert((normalized.clone(), address.private_creator.clone())) {
            return Err(AssemblyError::DuplicateElement(normalized));
        }
        if address.group == 0x0002
            || matches!(
                (address.group, address.element),
                (0x0008, 0x0016 | 0x0018) | (0x0020, 0x000D | 0x000E | 0x0052)
            )
        {
            return Err(AssemblyError::ProtectedElement(normalized));
        }
        let vr = element
            .vr
            .or(inferred_vr)
            .ok_or_else(|| AssemblyError::VrRequired(normalized.clone()))?;
        validate_value(vr, &element.value, depth, limits, element_count)?;
    }
    Ok(())
}

pub(super) fn resolve_address(
    address: &AssemblyAddress,
) -> Result<(AttributeAddress, Option<DicomVr>), AssemblyError> {
    match address {
        AssemblyAddress::Keyword { keyword } => {
            let dictionary = StandardDataDictionary;
            let entry = dictionary
                .by_name(keyword)
                .ok_or_else(|| AssemblyError::Address(format!("unknown keyword {keyword}")))?;
            Ok((
                AttributeAddress::standard(entry.tag())
                    .map_err(|e| AssemblyError::Address(e.to_string()))?,
                virtual_vr(entry.vr()),
            ))
        }
        AssemblyAddress::Tag { tag } => {
            let address = AttributeAddress::from_normalized_tag(tag)
                .map_err(|e| AssemblyError::Address(e.to_string()))?;
            let inferred = StandardDataDictionary
                .by_tag(address.tag())
                .and_then(|entry| virtual_vr(entry.vr()));
            Ok((address, inferred))
        }
        AssemblyAddress::Private {
            private_group,
            private_creator,
            private_offset,
        } => {
            let group = u16::from_str_radix(private_group, 16)
                .map_err(|_| AssemblyError::Address("invalid private group".into()))?;
            let offset = u16::from_str_radix(private_offset, 16)
                .map_err(|_| AssemblyError::Address("invalid private offset".into()))?;
            if group & 1 == 0 || matches!(group, 0x0001 | 0x0003) {
                return Err(AssemblyError::Address(
                    "private group must be a safe odd group".into(),
                ));
            }
            let address = AttributeAddress::private(
                dicom_core::Tag(group, 0x1000 | offset),
                private_creator.clone(),
            )
            .map_err(|e| AssemblyError::Address(e.to_string()))?;
            Ok((address, None))
        }
    }
}

fn virtual_vr(vr: VirtualVr) -> Option<DicomVr> {
    match vr {
        VirtualVr::Exact(vr) => DicomVr::from_str(&vr.to_string()).ok(),
        _ => None,
    }
}

fn validate_value(
    vr: DicomVr,
    value: &AssemblyValue,
    depth: usize,
    limits: &AssemblyLimits,
    element_count: &mut usize,
) -> Result<(), AssemblyError> {
    let compatible = match value {
        AssemblyValue::Empty => true,
        AssemblyValue::String { value } => {
            string_vr(vr) && value.len() as u64 <= limits.max_value_bytes
        }
        AssemblyValue::Strings { values } => {
            string_vr(vr)
                && !values.is_empty()
                && values
                    .iter()
                    .try_fold(0_u64, |total, value| total.checked_add(value.len() as u64))
                    .is_some_and(|total| total <= limits.max_value_bytes)
        }
        AssemblyValue::Integer { value } => integer_fits(vr, *value),
        AssemblyValue::Integers { values } => {
            !values.is_empty() && values.iter().all(|value| integer_fits(vr, *value))
        }
        AssemblyValue::Float { value } => float_fits(vr, *value),
        AssemblyValue::Floats { values } => {
            !values.is_empty() && values.iter().all(|value| float_fits(vr, *value))
        }
        AssemblyValue::Tag { value } => {
            vr == DicomVr::AT && AttributeAddress::from_normalized_tag(value).is_ok()
        }
        AssemblyValue::Tags { values } => {
            vr == DicomVr::AT
                && !values.is_empty()
                && values
                    .iter()
                    .all(|v| AttributeAddress::from_normalized_tag(v).is_ok())
        }
        AssemblyValue::Bytes { base64 } => {
            binary_vr(vr)
                && base64::engine::general_purpose::STANDARD
                    .decode(base64)
                    .is_ok_and(|v| v.len() as u64 <= limits.max_value_bytes)
        }
        AssemblyValue::Sequence { items } => {
            if vr != DicomVr::SQ {
                false
            } else {
                for item in items {
                    validate_elements(&item.elements, depth + 1, limits, element_count)?;
                }
                true
            }
        }
    };
    if compatible {
        Ok(())
    } else {
        Err(AssemblyError::Value(format!(
            "value incompatible with VR {vr}"
        )))
    }
}

fn integer_fits(vr: DicomVr, value: i64) -> bool {
    match vr {
        DicomVr::SS => i16::try_from(value).is_ok(),
        DicomVr::SL => i32::try_from(value).is_ok(),
        DicomVr::SV => true,
        DicomVr::US => u16::try_from(value).is_ok(),
        DicomVr::UL => u32::try_from(value).is_ok(),
        DicomVr::UV => u64::try_from(value).is_ok(),
        _ => false,
    }
}

fn float_fits(vr: DicomVr, value: f64) -> bool {
    value.is_finite()
        && match vr {
            DicomVr::FL => value.abs() <= f64::from(f32::MAX),
            DicomVr::FD => true,
            _ => false,
        }
}

fn string_vr(vr: DicomVr) -> bool {
    matches!(
        vr,
        DicomVr::AE
            | DicomVr::AS
            | DicomVr::CS
            | DicomVr::DA
            | DicomVr::DS
            | DicomVr::DT
            | DicomVr::IS
            | DicomVr::LO
            | DicomVr::LT
            | DicomVr::PN
            | DicomVr::SH
            | DicomVr::ST
            | DicomVr::TM
            | DicomVr::UC
            | DicomVr::UI
            | DicomVr::UR
            | DicomVr::UT
    )
}
fn binary_vr(vr: DicomVr) -> bool {
    matches!(
        vr,
        DicomVr::OB
            | DicomVr::OD
            | DicomVr::OF
            | DicomVr::OL
            | DicomVr::OV
            | DicomVr::OW
            | DicomVr::UN
    )
}

fn validate_bulk(
    instance: &AssemblyInstance,
    limits: &AssemblyLimits,
) -> Result<(), AssemblyError> {
    for bulk in &instance.bulk {
        match &bulk.source {
            BulkSource::File { path, .. } => validate_relative_path(path)?,
            BulkSource::InlineBase64 { base64, .. } => {
                let bytes = base64::engine::general_purpose::STANDARD
                    .decode(base64)
                    .map_err(|_| AssemblyError::Value("bulk base64 invalid".into()))?;
                if bytes.len() as u64 > limits.max_value_bytes {
                    return Err(AssemblyError::Limit("bulk value bytes"));
                }
            }
        }
        if matches!(
            bulk.kind.as_str(),
            "integer_pixel_data" | "float_pixel_data" | "double_float_pixel_data"
        ) && (bulk.rows.is_none() || bulk.columns.is_none() || bulk.bits_allocated.is_none())
        {
            if bulk.kind == "integer_pixel_data" {
                return Err(AssemblyError::Value(
                    "integer pixel bulk requires rows, columns, and bits_allocated".into(),
                ));
            }
            if bulk.rows.is_none() || bulk.columns.is_none() {
                return Err(AssemblyError::Value(
                    "floating pixel bulk requires rows and columns".into(),
                ));
            }
        }
        if bulk.kind == "waveform_data"
            && (bulk.channels.is_none() || bulk.samples.is_none() || bulk.bits_allocated.is_none())
        {
            return Err(AssemblyError::Value(
                "waveform bulk requires channels, samples, and bits_allocated".into(),
            ));
        }
        if bulk.kind == "encapsulated_document" && bulk.media_type.is_none() {
            return Err(AssemblyError::Value(
                "encapsulated document bulk requires media_type".into(),
            ));
        }
        if bulk.kind == "general" && (bulk.tag.is_none() || bulk.vr.is_none()) {
            return Err(AssemblyError::Value(
                "general bulk requires tag and VR".into(),
            ));
        }
        if bulk.kind == "general" && !bulk.vr.is_some_and(binary_vr) {
            return Err(AssemblyError::Value(
                "general bulk requires a binary VR".into(),
            ));
        }
        if bulk.kind == "integer_pixel_data"
            && bulk
                .bits_stored
                .zip(bulk.bits_allocated)
                .is_some_and(|(stored, allocated)| stored > allocated)
        {
            return Err(AssemblyError::Value(
                "integer pixel bits_stored exceeds bits_allocated".into(),
            ));
        }
    }
    Ok(())
}

fn validate_bulk_ownership(instance: &AssemblyInstance) -> Result<(), AssemblyError> {
    let mut occupied = BTreeSet::new();
    for element in &instance.elements {
        let (address, _) = resolve_address(&element.address)?;
        occupied.insert((address.group, address.element));
    }
    for bulk in &instance.bulk {
        for tag in bulk_owned_tags(bulk)? {
            if !occupied.insert(tag) {
                return Err(AssemblyError::ProtectedElement(format!(
                    "{:04X},{:04X}",
                    tag.0, tag.1
                )));
            }
        }
    }
    Ok(())
}

fn bulk_owned_tags(bulk: &AssemblyBulk) -> Result<Vec<(u16, u16)>, AssemblyError> {
    let tags = match bulk.kind.as_str() {
        "integer_pixel_data" => vec![
            "0028,0002",
            "0028,0004",
            "0028,0008",
            "0028,0010",
            "0028,0011",
            "0028,0100",
            "0028,0101",
            "0028,0102",
            "0028,0103",
            "7FE0,0010",
        ],
        "float_pixel_data" => vec![
            "0028,0002",
            "0028,0004",
            "0028,0008",
            "0028,0010",
            "0028,0011",
            "0028,0100",
            "7FE0,0008",
        ],
        "double_float_pixel_data" => vec![
            "0028,0002",
            "0028,0004",
            "0028,0008",
            "0028,0010",
            "0028,0011",
            "0028,0100",
            "7FE0,0009",
        ],
        "waveform_data" => vec!["003A,0005", "003A,0010", "5400,1004", "5400,1010"],
        "encapsulated_document" => vec!["0042,0011", "0042,0012"],
        "mesh" => vec!["0066,0023"],
        "general" => vec![
            bulk.tag
                .as_deref()
                .ok_or_else(|| AssemblyError::Value("general bulk tag missing".into()))?,
        ],
        _ => return Err(AssemblyError::Value("bulk kind unsupported".into())),
    };
    tags.into_iter()
        .map(|tag| {
            let address = AttributeAddress::from_normalized_tag(tag)
                .map_err(|error| AssemblyError::Address(error.to_string()))?;
            if address.group == 0x0002
                || address.group & 1 == 1
                || matches!(
                    (address.group, address.element),
                    (0x0008, 0x0016 | 0x0018) | (0x0020, 0x000D | 0x000E | 0x0052)
                )
            {
                return Err(AssemblyError::ProtectedElement(address.normalized_tag()));
            }
            Ok((address.group, address.element))
        })
        .collect()
}

fn validate_relative_path(value: &str) -> Result<(), AssemblyError> {
    let path = Path::new(value);
    if path.is_absolute()
        || path
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
        || value.contains('\\')
        || value.contains(':')
    {
        return Err(AssemblyError::UnsafePath(value.to_string()));
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum AssemblyError {
    Json(String),
    Schema(String),
    UnsupportedVersion(String),
    DuplicateInstance(String),
    DuplicatePath(String),
    DuplicateElement(String),
    ProtectedElement(String),
    VrRequired(String),
    Address(String),
    Value(String),
    TransferSyntax(String),
    MissingReference(String),
    UnsafePath(String),
    Limit(&'static str),
}

impl fmt::Display for AssemblyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(v) => write!(f, "assembly request JSON invalid: {v}"),
            Self::Schema(v) => write!(f, "assembly request schema invalid: {v}"),
            Self::UnsupportedVersion(v) => {
                write!(f, "unsupported assembly request schema version {v}")
            }
            Self::DuplicateInstance(v) => write!(f, "duplicate assembly instance {v}"),
            Self::DuplicatePath(v) => write!(f, "duplicate assembly output path {v}"),
            Self::DuplicateElement(v) => write!(f, "duplicate assembly element {v}"),
            Self::ProtectedElement(v) => write!(f, "assembly request protected element {v}"),
            Self::VrRequired(v) => write!(f, "assembly element {v} requires explicit VR"),
            Self::Address(v) => write!(f, "assembly element address invalid: {v}"),
            Self::Value(v) => write!(f, "assembly element value invalid: {v}"),
            Self::TransferSyntax(v) => write!(f, "assembly transfer syntax unavailable: {v}"),
            Self::MissingReference(v) => write!(f, "assembly reference target missing: {v}"),
            Self::UnsafePath(v) => write!(f, "unsafe assembly path: {v}"),
            Self::Limit(v) => write!(f, "assembly resource limit exceeded: {v}"),
        }
    }
}

impl std::error::Error for AssemblyError {}
