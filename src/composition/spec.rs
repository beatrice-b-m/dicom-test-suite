use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use dicom_core::Tag;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{
    AttributeAddress, AttributeItem, AttributeOperation, AttributeValue, ByteOrder, DicomVr,
    PhotometricInterpretation, PixelShape, PlanarConfiguration, PrimitiveValue, SampleType,
    TemplateId, TemplateVersion,
};

const COMPOSITION_SPEC_SCHEMA: &str = include_str!("../../schemas/composition-spec.schema.json");

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct CompositionSpec {
    pub composition_spec_schema_version: String,
    #[serde(default)]
    pub defaults: SpecDefaults,
    pub instances: Vec<SpecInstance>,
    #[serde(default)]
    pub resource_limits: ResourceLimits,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct SpecDefaults {
    #[serde(default)]
    pub patient: Option<AttributeScope>,
    #[serde(default)]
    pub study: Option<AttributeScope>,
    #[serde(default)]
    pub series: Option<AttributeScope>,
    #[serde(default)]
    pub equipment: Option<AttributeScope>,
    pub transfer_syntax_uid: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct AttributeScope {
    pub attributes: Vec<Value>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct SpecInstance {
    pub instance_id: String,
    pub template: TemplateSelector,
    pub transfer_syntax_uid: Option<String>,
    #[serde(default)]
    pub identities: BTreeMap<String, IdentityChoice>,
    #[serde(default)]
    pub attributes: Vec<Value>,
    #[serde(default)]
    pub content: Vec<ContentAssignment>,
    #[serde(default)]
    pub references: Vec<SpecReference>,
    #[serde(default)]
    pub parameters: BTreeMap<String, Value>,
}

impl SpecInstance {
    pub fn typed_attributes(&self) -> Result<Vec<AttributeOperation>, SpecError> {
        self.attributes.iter().map(parse_operation).collect()
    }
}

impl SpecDefaults {
    pub fn typed_attributes(&self) -> Result<Vec<AttributeOperation>, SpecError> {
        let mut attributes = Vec::new();
        for scope in [&self.patient, &self.study, &self.series, &self.equipment]
            .into_iter()
            .flatten()
        {
            for operation in &scope.attributes {
                attributes.push(parse_operation(operation)?);
            }
        }
        Ok(attributes)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct TemplateSelector {
    pub id: TemplateId,
    pub version: Option<TemplateVersion>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(untagged)]
pub enum IdentityChoice {
    Auto { auto: bool },
    Explicit { uid: String },
    Shared { share_with: String },
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ContentAssignment {
    pub slot: String,
    pub source: ContentSource,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ContentSource {
    Default,
    LocalFile {
        path: String,
        sha256: Option<String>,
        media_type: Option<String>,
        pixel: Option<PixelDeclaration>,
    },
    InlineSmallFixture {
        base64: String,
        sha256: Option<String>,
        media_type: Option<String>,
        pixel: Option<PixelDeclaration>,
    },
    EncodedFrames {
        transfer_syntax_uid: String,
        frames: Vec<EncodedFrame>,
        pixel: Option<PixelDeclaration>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct EncodedFrame {
    pub path: String,
    pub sha256: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct PixelDeclaration {
    pub rows: u32,
    pub columns: u32,
    pub frames: u32,
    pub samples_per_pixel: u8,
    pub photometric_interpretation: PhotometricInterpretation,
    pub sample_type: SpecSampleType,
    pub bits_allocated: u8,
    pub bits_stored: u8,
    pub high_bit: u8,
    pub byte_order: ByteOrder,
    pub planar_configuration: Option<u8>,
}

impl PixelDeclaration {
    pub fn shape(&self) -> Result<PixelShape, SpecError> {
        Ok(PixelShape {
            rows: self.rows,
            columns: self.columns,
            frames: self.frames,
            samples_per_pixel: self.samples_per_pixel,
            photometric_interpretation: self.photometric_interpretation,
            sample_type: self.sample_type.into(),
            bits_allocated: self.bits_allocated,
            bits_stored: self.bits_stored,
            high_bit: self.high_bit,
            byte_order: self.byte_order,
            planar_configuration: self
                .planar_configuration
                .map(|value| match value {
                    0 => Ok(PlanarConfiguration::Interleaved),
                    1 => Ok(PlanarConfiguration::Planar),
                    _ => Err(SpecError::InvalidPlanarConfiguration(value)),
                })
                .transpose()?,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpecSampleType {
    Uint,
    Int,
    Float32,
    Float64,
    Bit1,
}

impl From<SpecSampleType> for SampleType {
    fn from(value: SpecSampleType) -> Self {
        match value {
            SpecSampleType::Uint => Self::UnsignedInteger,
            SpecSampleType::Int => Self::SignedInteger,
            SpecSampleType::Float32 => Self::Float32,
            SpecSampleType::Float64 => Self::Float64,
            SpecSampleType::Bit1 => Self::Bit1,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct SpecReference {
    pub role: String,
    pub target_instance_id: String,
    #[serde(default)]
    pub frames: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceLimits {
    #[serde(default = "default_max_instances")]
    pub max_instances: u64,
    #[serde(default = "default_max_input_files")]
    pub max_input_files: u64,
    #[serde(default = "default_max_file_bytes")]
    pub max_file_bytes: u64,
    #[serde(default = "default_max_total_input_bytes")]
    pub max_total_input_bytes: u64,
    #[serde(default = "default_max_total_output_bytes")]
    pub max_total_output_bytes: u64,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_instances: default_max_instances(),
            max_input_files: default_max_input_files(),
            max_file_bytes: default_max_file_bytes(),
            max_total_input_bytes: default_max_total_input_bytes(),
            max_total_output_bytes: default_max_total_output_bytes(),
        }
    }
}

const fn default_max_instances() -> u64 {
    1024
}
const fn default_max_input_files() -> u64 {
    1024
}
const fn default_max_file_bytes() -> u64 {
    1024 * 1024 * 1024
}
const fn default_max_total_input_bytes() -> u64 {
    4 * 1024 * 1024 * 1024
}
const fn default_max_total_output_bytes() -> u64 {
    8 * 1024 * 1024 * 1024
}

impl CompositionSpec {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, SpecError> {
        let path = path.as_ref();
        let bytes = fs::read(path).map_err(|source| SpecError::Read {
            path: path.to_path_buf(),
            source,
        })?;
        Self::from_slice(&bytes)
    }

    pub fn from_slice(bytes: &[u8]) -> Result<Self, SpecError> {
        let value: Value = serde_json::from_slice(bytes).map_err(SpecError::Parse)?;
        let schema: Value = serde_json::from_str(COMPOSITION_SPEC_SCHEMA)
            .expect("embedded composition schema parses");
        let validator = jsonschema::validator_for(&schema).expect("composition schema compiles");
        let errors = validator
            .iter_errors(&value)
            .map(|error| error.to_string())
            .collect::<Vec<_>>();
        if !errors.is_empty() {
            return Err(SpecError::Schema(errors));
        }
        let spec: Self = serde_json::from_value(value).map_err(SpecError::Parse)?;
        if spec.instances.len() as u64 > spec.resource_limits.max_instances {
            return Err(SpecError::InstanceLimit {
                count: spec.instances.len(),
                limit: spec.resource_limits.max_instances,
            });
        }
        let mut ids = std::collections::BTreeSet::new();
        for instance in &spec.instances {
            if !ids.insert(&instance.instance_id) {
                return Err(SpecError::DuplicateInstance(instance.instance_id.clone()));
            }
            for operation in instance.typed_attributes()? {
                operation.validate()?;
            }
        }
        for operation in spec.defaults.typed_attributes()? {
            operation.validate()?;
        }
        Ok(spec)
    }
}

fn parse_operation(value: &Value) -> Result<AttributeOperation, SpecError> {
    let address = parse_address(&value["address"])?;
    match value["operation"]
        .as_str()
        .expect("schema requires operation")
    {
        "empty" => Ok(AttributeOperation::Empty { address }),
        "remove" => Ok(AttributeOperation::Remove { address }),
        "set" => {
            let vr = DicomVr::from_str(value["vr"].as_str().expect("schema requires VR"))?;
            Ok(AttributeOperation::Set {
                address,
                vr,
                value: parse_attribute_value(vr, &value["value"])?,
            })
        }
        other => Err(SpecError::Operation(other.to_string())),
    }
}

fn parse_address(value: &Value) -> Result<AttributeAddress, SpecError> {
    if let Some(keyword) = value.get("keyword").and_then(Value::as_str) {
        return Ok(AttributeAddress::from_keyword(keyword)?);
    }
    let tag = value["tag"].as_str().expect("schema requires tag");
    let address = AttributeAddress::from_normalized_tag(tag);
    if let Some(creator) = value.get("private_creator").and_then(Value::as_str) {
        let group = u16::from_str_radix(&tag[..4], 16).expect("schema checked tag");
        let element = u16::from_str_radix(&tag[5..], 16).expect("schema checked tag");
        Ok(AttributeAddress::private(Tag(group, element), creator)?)
    } else {
        Ok(address?)
    }
}

fn parse_attribute_value(vr: DicomVr, value: &Value) -> Result<AttributeValue, SpecError> {
    match value["kind"].as_str().expect("schema requires value kind") {
        "string" | "integer" | "number" | "tag" => {
            Ok(AttributeValue::Primitive(parse_primitive(vr, value)?))
        }
        "multi" => Ok(AttributeValue::Multi(
            value["values"]
                .as_array()
                .expect("schema requires values")
                .iter()
                .map(|value| parse_primitive(vr, value))
                .collect::<Result<Vec<_>, _>>()?,
        )),
        "binary" => Ok(AttributeValue::Binary(decode_base64(
            value["base64"].as_str().expect("schema requires base64"),
        )?)),
        "sequence" => Ok(AttributeValue::Sequence(
            value["items"]
                .as_array()
                .expect("schema requires items")
                .iter()
                .map(|item| {
                    Ok(AttributeItem {
                        attributes: item["attributes"]
                            .as_array()
                            .expect("schema requires item attributes")
                            .iter()
                            .map(parse_operation)
                            .collect::<Result<Vec<_>, _>>()?,
                    })
                })
                .collect::<Result<Vec<_>, SpecError>>()?,
        )),
        other => Err(SpecError::ValueKind(other.to_string())),
    }
}

fn parse_primitive(vr: DicomVr, value: &Value) -> Result<PrimitiveValue, SpecError> {
    match value["kind"].as_str().expect("schema requires scalar kind") {
        "string" => Ok(PrimitiveValue::String(
            value["value"]
                .as_str()
                .expect("schema checked string")
                .into(),
        )),
        "integer" => {
            let number = value["value"].as_i64().expect("schema checked integer");
            if matches!(vr, DicomVr::SS | DicomVr::SL | DicomVr::SV) {
                Ok(PrimitiveValue::Signed(number))
            } else {
                Ok(PrimitiveValue::Unsigned(
                    u64::try_from(number).map_err(|_| SpecError::NegativeUnsigned(number))?,
                ))
            }
        }
        "number" => {
            let number = value["value"].as_f64().expect("schema checked number");
            match vr {
                DicomVr::FL => Ok(PrimitiveValue::Float32Bits((number as f32).to_bits())),
                DicomVr::FD => Ok(PrimitiveValue::Float64Bits(number.to_bits())),
                _ => Err(SpecError::NumberVr(vr)),
            }
        }
        "tag" => Ok(PrimitiveValue::Tag(AttributeAddress::from_normalized_tag(
            value["value"].as_str().expect("schema checked tag"),
        )?)),
        other => Err(SpecError::ValueKind(other.to_string())),
    }
}

fn decode_base64(value: &str) -> Result<Vec<u8>, SpecError> {
    if value.len() % 4 != 0 {
        return Err(SpecError::Base64);
    }
    let decode = |byte| match byte {
        b'A'..=b'Z' => Some(byte - b'A'),
        b'a'..=b'z' => Some(byte - b'a' + 26),
        b'0'..=b'9' => Some(byte - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    };
    let mut output = Vec::with_capacity(value.len() / 4 * 3);
    for chunk in value.as_bytes().chunks_exact(4) {
        let a = decode(chunk[0]).ok_or(SpecError::Base64)?;
        let b = decode(chunk[1]).ok_or(SpecError::Base64)?;
        let c = if chunk[2] == b'=' {
            0
        } else {
            decode(chunk[2]).ok_or(SpecError::Base64)?
        };
        let d = if chunk[3] == b'=' {
            0
        } else {
            decode(chunk[3]).ok_or(SpecError::Base64)?
        };
        output.push((a << 2) | (b >> 4));
        if chunk[2] != b'=' {
            output.push((b << 4) | (c >> 2));
        }
        if chunk[3] != b'=' {
            output.push((c << 6) | d);
        }
    }
    Ok(output)
}

#[derive(Debug)]
pub enum SpecError {
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    Parse(serde_json::Error),
    Schema(Vec<String>),
    Attribute(super::AttributeError),
    DuplicateInstance(String),
    InstanceLimit {
        count: usize,
        limit: u64,
    },
    InvalidPlanarConfiguration(u8),
    NegativeUnsigned(i64),
    NumberVr(DicomVr),
    Base64,
    Operation(String),
    ValueKind(String),
}

impl From<super::AttributeError> for SpecError {
    fn from(error: super::AttributeError) -> Self {
        Self::Attribute(error)
    }
}

impl fmt::Display for SpecError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for SpecError {}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use dicom_core::Tag;
    use dicom_dictionary_std::tags;
    use dicom_object::open_file;

    use super::*;
    use crate::composition::{
        CompositionUidRole, IdentityAllocator, Part10Materializer, ResolvedInstancePlan,
        TemplateCatalog, resolved_sc_plan, sc_default_pixels,
    };

    static NEXT: AtomicU64 = AtomicU64::new(0);
    const LOCK_HASH: &str = "823230c5932b81b504434330d118fba286d5ff41d4e2f7766372633f4a49e559";

    #[test]
    fn parses_all_p2_attribute_forms_into_typed_operations() {
        let spec =
            CompositionSpec::load("tests/fixtures/composition/valid/typed-local-content.json")
                .unwrap();
        let operations = spec.instances[0].typed_attributes().unwrap();
        assert_eq!(operations.len(), 5);
        assert!(matches!(operations[1], AttributeOperation::Empty { .. }));
        assert!(matches!(operations[2], AttributeOperation::Remove { .. }));
        assert!(
            matches!(&operations[3], AttributeOperation::Set { value: AttributeValue::Sequence(items), .. } if items.len() == 1)
        );
        assert!(
            matches!(&operations[4], AttributeOperation::Set { value: AttributeValue::Binary(bytes), .. } if bytes == &[0, 1, 2, 3])
        );
        assert_eq!(
            operations[4].address().private_creator.as_deref(),
            Some("DTS_COMPOSE")
        );
    }

    #[test]
    fn p2_attribute_forms_round_trip_through_a_resolved_sc_plan() {
        let spec =
            CompositionSpec::load("tests/fixtures/composition/valid/typed-local-content.json")
                .unwrap();
        let source = &spec.instances[0];
        let catalog = TemplateCatalog::load("templates/catalog.json").unwrap();
        let template = catalog
            .resolve_qualified(&source.template.id, source.template.version)
            .unwrap();
        let identities = IdentityAllocator::new(
            LOCK_HASH,
            source.template.id.clone(),
            template.template_version,
            1,
        )
        .unwrap()
        .allocate_plan(
            "source",
            [
                (CompositionUidRole::StudyInstance, 0),
                (CompositionUidRole::SeriesInstance, 0),
                (CompositionUidRole::SopInstance, 0),
                (CompositionUidRole::ImplementationClass, 0),
            ],
        )
        .unwrap();
        let plan = ResolvedInstancePlan {
            plan_schema_version: "0.1.0".into(),
            instance_id: "source".into(),
            template_id: source.template.id.clone(),
            template_version: template.template_version,
            sop_class_uid: template.sop_class_uid.clone(),
            transfer_syntax_uid: catalog.default_transfer_syntax(template).uid.clone(),
            identities,
            attributes: vec![],
            content: vec![],
            references: vec![],
        };
        let plan = resolved_sc_plan(
            plan,
            template,
            &spec.defaults.typed_attributes().unwrap(),
            &source.typed_attributes().unwrap(),
            sc_default_pixels(&template.template_id).unwrap(),
        )
        .unwrap();
        let path = std::env::temp_dir().join(format!(
            "dts-p2-attributes-{}-{}.dcm",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        Part10Materializer.materialize(&plan, &path).unwrap();
        let object = open_file(&path).unwrap();
        assert_eq!(
            object
                .element(tags::SERIES_DESCRIPTION)
                .unwrap()
                .to_str()
                .unwrap(),
            "Synthetic\\Composition"
        );
        assert_eq!(
            object
                .element(tags::PATIENT_BIRTH_DATE)
                .unwrap()
                .to_bytes()
                .unwrap()
                .len(),
            0
        );
        assert!(object.element(Tag(0x0010, 0x1000)).is_err());
        assert_eq!(
            object
                .element(Tag(0x0011, 0x1010))
                .unwrap()
                .to_bytes()
                .unwrap()
                .as_ref(),
            &[0, 1, 2, 3]
        );
        let item = object
            .element(tags::REFERENCED_SERIES_SEQUENCE)
            .unwrap()
            .items()
            .unwrap()[0]
            .element(tags::SERIES_INSTANCE_UID)
            .unwrap()
            .to_str()
            .unwrap();
        assert_eq!(item, "2.25.987654321");
        fs::remove_file(path).unwrap();
    }
}
