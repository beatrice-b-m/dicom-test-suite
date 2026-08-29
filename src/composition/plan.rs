use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::PathBuf;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{
    AttributeAddress, AttributeError, AttributeOperation, AttributeValue, DicomVr, IdentityPlan,
    MaterializedReference, PrimitiveValue, TemplateId, TemplateVersion,
};
use crate::sha256_hex;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValueOrigin {
    TemplateDefault,
    RunDefault,
    InstanceOverride,
    DerivedStructural,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttributeLayer {
    pub origin: ValueOrigin,
    pub operations: Vec<AttributeOperation>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AttributePolicy {
    pub tag: String,
    pub keyword: String,
    pub vr: DicomVr,
    pub requirement: String,
    pub behavior: String,
    pub condition: Option<Condition>,
    pub default: Option<Value>,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "operator", rename_all = "snake_case")]
pub enum Condition {
    Present { tag: String },
    Equals { tag: String, value: Value },
    ContentSlotSet { slot: String },
    ParameterEquals { parameter: String, value: Value },
    All { conditions: Vec<Condition> },
    Any { conditions: Vec<Condition> },
    Not { condition: Box<Condition> },
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ResolveContext {
    pub content_slots: BTreeSet<String>,
    pub parameters: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedAttribute {
    pub address: AttributeAddress,
    pub vr: DicomVr,
    pub value: Option<AttributeValue>,
    pub origin: ValueOrigin,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CanonicalContent {
    pub slot: String,
    pub kind: String,
    pub address: AttributeAddress,
    pub vr: DicomVr,
    pub size_bytes: u64,
    pub sha256: String,
    pub properties: BTreeMap<String, String>,
    #[serde(skip)]
    pub materialization: Option<ContentMaterialization>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContentMaterialization {
    Inline(Vec<u8>),
    StagedFile(PathBuf),
    Encapsulated {
        basic_offset_table: Vec<u32>,
        fragments: Vec<Vec<u8>>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedInstancePlan {
    pub plan_schema_version: String,
    pub instance_id: String,
    pub template_id: TemplateId,
    pub template_version: TemplateVersion,
    pub sop_class_uid: String,
    pub transfer_syntax_uid: String,
    pub identities: IdentityPlan,
    pub attributes: Vec<ResolvedAttribute>,
    pub content: Vec<CanonicalContent>,
    pub references: Vec<MaterializedReference>,
}

impl ResolvedInstancePlan {
    pub fn canonical_sha256(&self) -> String {
        let bytes = serde_json::to_vec(self).expect("resolved plan is serializable");
        sha256_hex(&bytes)
    }
}

#[derive(Debug, Clone)]
pub struct AttributeResolver {
    policies: BTreeMap<AttributeAddress, AttributePolicy>,
}

impl AttributeResolver {
    pub fn new(policies: Vec<AttributePolicy>) -> Result<Self, ResolveError> {
        let mut indexed = BTreeMap::new();
        for policy in policies {
            let address = AttributeAddress::from_normalized_tag(&policy.tag)?;
            if policy.vr != DicomVr::from_str(&policy.vr.to_string())? {
                unreachable!("VR round trip");
            }
            if indexed.insert(address.clone(), policy).is_some() {
                return Err(ResolveError::DuplicatePolicy(address.normalized_tag()));
            }
        }
        Ok(Self { policies: indexed })
    }

    pub fn from_descriptor_attributes(attributes: &[Value]) -> Result<Self, ResolveError> {
        let policies = attributes
            .iter()
            .cloned()
            .map(serde_json::from_value)
            .collect::<Result<Vec<_>, _>>()
            .map_err(ResolveError::PolicyParse)?;
        Self::new(policies)
    }

    pub fn template_default_layer(&self) -> Result<AttributeLayer, ResolveError> {
        let mut operations = Vec::new();
        for (address, policy) in &self.policies {
            if policy.behavior != "defaulted" {
                continue;
            }
            let default = policy
                .default
                .as_ref()
                .ok_or_else(|| ResolveError::MissingTemplateDefault(address.normalized_tag()))?;
            operations.push(default_operation(address.clone(), policy.vr, default)?);
        }
        Ok(AttributeLayer {
            origin: ValueOrigin::TemplateDefault,
            operations,
        })
    }

    pub fn resolve(
        &self,
        layers: &[AttributeLayer],
        context: &ResolveContext,
    ) -> Result<Vec<ResolvedAttribute>, ResolveError> {
        let derived_tags = layers
            .iter()
            .filter(|layer| layer.origin == ValueOrigin::DerivedStructural)
            .flat_map(|layer| layer.operations.iter().map(AttributeOperation::address))
            .cloned()
            .collect::<BTreeSet<_>>();
        let mut state = BTreeMap::<AttributeAddress, ResolvedAttribute>::new();
        let mut removals = BTreeMap::<AttributeAddress, ValueOrigin>::new();
        for layer in layers {
            let mut seen = BTreeSet::new();
            for operation in &layer.operations {
                operation.validate()?;
                let address = operation.address().clone();
                if !seen.insert(address.clone()) {
                    return Err(ResolveError::DuplicateLayerOperation {
                        tag: address.normalized_tag(),
                        origin: layer.origin,
                    });
                }
                let caller_layer = matches!(
                    layer.origin,
                    ValueOrigin::RunDefault | ValueOrigin::InstanceOverride
                );
                if caller_layer {
                    if derived_tags.contains(&address) {
                        return Err(ResolveError::ProtectedCollision {
                            tag: address.normalized_tag(),
                            reason: "derived structural field",
                        });
                    }
                    if let Some(policy) = self.policies.get(&address) {
                        if matches!(policy.behavior.as_str(), "protected" | "derived") {
                            return Err(ResolveError::ProtectedCollision {
                                tag: address.normalized_tag(),
                                reason: "template policy",
                            });
                        }
                    }
                }
                match operation {
                    AttributeOperation::Set { vr, value, .. } => {
                        state.insert(
                            address.clone(),
                            ResolvedAttribute {
                                address: address.clone(),
                                vr: *vr,
                                value: Some(value.clone()),
                                origin: layer.origin,
                            },
                        );
                        removals.remove(&address);
                    }
                    AttributeOperation::Empty { .. } => {
                        let vr = self
                            .policies
                            .get(&address)
                            .map(|policy| policy.vr)
                            .ok_or_else(|| {
                                ResolveError::EmptyUnknownVr(address.normalized_tag())
                            })?;
                        state.insert(
                            address.clone(),
                            ResolvedAttribute {
                                address: address.clone(),
                                vr,
                                value: None,
                                origin: layer.origin,
                            },
                        );
                        removals.remove(&address);
                    }
                    AttributeOperation::Remove { .. } => {
                        state.remove(&address);
                        removals.insert(address, layer.origin);
                    }
                }
            }
        }
        self.validate_requirements(&state, &removals, context)?;
        Ok(state.into_values().collect())
    }

    fn validate_requirements(
        &self,
        state: &BTreeMap<AttributeAddress, ResolvedAttribute>,
        removals: &BTreeMap<AttributeAddress, ValueOrigin>,
        context: &ResolveContext,
    ) -> Result<(), ResolveError> {
        for (address, policy) in &self.policies {
            let condition_satisfied = policy
                .condition
                .as_ref()
                .map(|condition| evaluate_condition(condition, state, context))
                .transpose()?
                .unwrap_or(true);
            let required = match policy.requirement.as_str() {
                "1" | "2" => true,
                "1C" | "2C" => condition_satisfied,
                "3" => false,
                other => return Err(ResolveError::UnknownRequirement(other.to_string())),
            };
            if removals.contains_key(address)
                && (required || matches!(policy.behavior.as_str(), "protected" | "derived"))
            {
                return Err(ResolveError::RequiredRemoval {
                    tag: address.normalized_tag(),
                    requirement: policy.requirement.clone(),
                });
            }
            if required {
                let Some(attribute) = state.get(address) else {
                    return Err(ResolveError::MissingRequired {
                        tag: address.normalized_tag(),
                        requirement: policy.requirement.clone(),
                    });
                };
                if matches!(policy.requirement.as_str(), "1" | "1C") && attribute.value.is_none() {
                    return Err(ResolveError::EmptyTypeOne(address.normalized_tag()));
                }
            }
        }
        Ok(())
    }
}

fn evaluate_condition(
    condition: &Condition,
    state: &BTreeMap<AttributeAddress, ResolvedAttribute>,
    context: &ResolveContext,
) -> Result<bool, ResolveError> {
    match condition {
        Condition::Present { tag } => {
            Ok(state.contains_key(&AttributeAddress::from_normalized_tag(tag)?))
        }
        Condition::Equals { tag, value } => {
            let address = AttributeAddress::from_normalized_tag(tag)?;
            Ok(state
                .get(&address)
                .and_then(|attribute| attribute.value.as_ref())
                .is_some_and(|actual| primitive_json(actual) == *value))
        }
        Condition::ContentSlotSet { slot } => Ok(context.content_slots.contains(slot)),
        Condition::ParameterEquals { parameter, value } => {
            Ok(context.parameters.get(parameter) == Some(value))
        }
        Condition::All { conditions } => {
            for condition in conditions {
                if !evaluate_condition(condition, state, context)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        Condition::Any { conditions } => {
            for condition in conditions {
                if evaluate_condition(condition, state, context)? {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        Condition::Not { condition } => Ok(!evaluate_condition(condition, state, context)?),
    }
}

fn primitive_json(value: &AttributeValue) -> Value {
    match value {
        AttributeValue::Primitive(PrimitiveValue::String(value)) => Value::String(value.clone()),
        AttributeValue::Primitive(PrimitiveValue::Signed(value)) => (*value).into(),
        AttributeValue::Primitive(PrimitiveValue::Unsigned(value)) => (*value).into(),
        _ => Value::Null,
    }
}

fn default_operation(
    address: AttributeAddress,
    vr: DicomVr,
    default: &Value,
) -> Result<AttributeOperation, ResolveError> {
    let kind = default["kind"]
        .as_str()
        .ok_or_else(|| ResolveError::InvalidTemplateDefault(address.normalized_tag()))?;
    match kind {
        "empty" => Ok(AttributeOperation::Empty { address }),
        "provider" => Err(ResolveError::ProviderDefaultRequiresContentResolver(
            address.normalized_tag(),
        )),
        "literal" => Ok(AttributeOperation::Set {
            address,
            vr,
            value: AttributeValue::Primitive(json_primitive(vr, &default["value"])?),
        }),
        "multi" => Ok(AttributeOperation::Set {
            address,
            vr,
            value: AttributeValue::Multi(
                default["values"]
                    .as_array()
                    .ok_or_else(|| ResolveError::InvalidTemplateDefault("multi".into()))?
                    .iter()
                    .map(|value| json_primitive(vr, value))
                    .collect::<Result<Vec<_>, _>>()?,
            ),
        }),
        _ => Err(ResolveError::InvalidTemplateDefault(
            address.normalized_tag(),
        )),
    }
}

fn json_primitive(vr: DicomVr, value: &Value) -> Result<PrimitiveValue, ResolveError> {
    if let Some(value) = value.as_str() {
        return Ok(PrimitiveValue::String(value.to_string()));
    }
    if let Some(value) = value.as_i64() {
        return Ok(if matches!(vr, DicomVr::SS | DicomVr::SL | DicomVr::SV) {
            PrimitiveValue::Signed(value)
        } else {
            PrimitiveValue::Unsigned(
                u64::try_from(value)
                    .map_err(|_| ResolveError::InvalidTemplateDefault(vr.to_string()))?,
            )
        });
    }
    if let Some(value) = value.as_u64() {
        return Ok(PrimitiveValue::Unsigned(value));
    }
    if let Some(value) = value.as_f64() {
        return Ok(match vr {
            DicomVr::FL => PrimitiveValue::Float32Bits((value as f32).to_bits()),
            DicomVr::FD => PrimitiveValue::Float64Bits(value.to_bits()),
            _ => return Err(ResolveError::InvalidTemplateDefault(vr.to_string())),
        });
    }
    Err(ResolveError::InvalidTemplateDefault(vr.to_string()))
}

#[derive(Debug)]
pub enum ResolveError {
    Attribute(AttributeError),
    PolicyParse(serde_json::Error),
    DuplicatePolicy(String),
    DuplicateLayerOperation { tag: String, origin: ValueOrigin },
    ProtectedCollision { tag: String, reason: &'static str },
    EmptyUnknownVr(String),
    MissingTemplateDefault(String),
    InvalidTemplateDefault(String),
    ProviderDefaultRequiresContentResolver(String),
    UnknownRequirement(String),
    RequiredRemoval { tag: String, requirement: String },
    MissingRequired { tag: String, requirement: String },
    EmptyTypeOne(String),
}

impl From<AttributeError> for ResolveError {
    fn from(error: AttributeError) -> Self {
        Self::Attribute(error)
    }
}

impl fmt::Display for ResolveError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Attribute(error) => error.fmt(formatter),
            Self::PolicyParse(error) => {
                write!(formatter, "parse template attribute policy: {error}")
            }
            Self::DuplicatePolicy(tag) => write!(formatter, "duplicate template policy for {tag}"),
            Self::DuplicateLayerOperation { tag, origin } => {
                write!(formatter, "{origin:?} repeats attribute {tag}")
            }
            Self::ProtectedCollision { tag, reason } => {
                write!(formatter, "caller attribute {tag} collides with {reason}")
            }
            Self::EmptyUnknownVr(tag) => {
                write!(formatter, "empty operation for {tag} has no template VR")
            }
            Self::MissingTemplateDefault(tag) => {
                write!(formatter, "defaulted template tag {tag} has no default")
            }
            Self::InvalidTemplateDefault(tag) => {
                write!(formatter, "invalid template default for {tag}")
            }
            Self::ProviderDefaultRequiresContentResolver(tag) => {
                write!(
                    formatter,
                    "provider default for {tag} requires content resolution"
                )
            }
            Self::UnknownRequirement(requirement) => {
                write!(formatter, "unknown DICOM requirement {requirement}")
            }
            Self::RequiredRemoval { tag, requirement } => {
                write!(formatter, "cannot remove {requirement} attribute {tag}")
            }
            Self::MissingRequired { tag, requirement } => {
                write!(formatter, "missing {requirement} attribute {tag}")
            }
            Self::EmptyTypeOne(tag) => write!(formatter, "Type 1 attribute {tag} cannot be empty"),
        }
    }
}

impl std::error::Error for ResolveError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::composition::{CompositionUidRole, IdentityAllocator};

    const LOCK_HASH: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    fn policy(tag: &str, vr: DicomVr, requirement: &str, behavior: &str) -> AttributePolicy {
        AttributePolicy {
            tag: tag.into(),
            keyword: "TestAttribute".into(),
            vr,
            requirement: requirement.into(),
            behavior: behavior.into(),
            condition: None,
            default: None,
            description: "test".into(),
        }
    }

    fn set(tag: &str, vr: DicomVr, value: PrimitiveValue) -> AttributeOperation {
        AttributeOperation::Set {
            address: AttributeAddress::from_normalized_tag(tag).unwrap(),
            vr,
            value: AttributeValue::Primitive(value),
        }
    }

    #[test]
    fn precedence_is_template_run_instance_then_derived() {
        let resolver = AttributeResolver::new(vec![policy(
            "0008,103E",
            DicomVr::LO,
            "3",
            "caller_settable",
        )])
        .unwrap();
        let resolved = resolver
            .resolve(
                &[
                    AttributeLayer {
                        origin: ValueOrigin::TemplateDefault,
                        operations: vec![set(
                            "0008,103E",
                            DicomVr::LO,
                            PrimitiveValue::String("template".into()),
                        )],
                    },
                    AttributeLayer {
                        origin: ValueOrigin::RunDefault,
                        operations: vec![set(
                            "0008,103E",
                            DicomVr::LO,
                            PrimitiveValue::String("run".into()),
                        )],
                    },
                    AttributeLayer {
                        origin: ValueOrigin::InstanceOverride,
                        operations: vec![set(
                            "0008,103E",
                            DicomVr::LO,
                            PrimitiveValue::String("instance".into()),
                        )],
                    },
                ],
                &ResolveContext::default(),
            )
            .unwrap();
        assert_eq!(resolved[0].origin, ValueOrigin::InstanceOverride);
        assert_eq!(
            resolved[0].value,
            Some(AttributeValue::Primitive(PrimitiveValue::String(
                "instance".into()
            )))
        );
    }

    #[test]
    fn protected_and_derived_collisions_fail_before_materialization() {
        let resolver =
            AttributeResolver::new(vec![policy("0008,0016", DicomVr::UI, "1", "protected")])
                .unwrap();
        assert!(matches!(
            resolver.resolve(
                &[AttributeLayer {
                    origin: ValueOrigin::InstanceOverride,
                    operations: vec![set(
                        "0008,0016",
                        DicomVr::UI,
                        PrimitiveValue::String("1.2.3".into())
                    )]
                }],
                &ResolveContext::default()
            ),
            Err(ResolveError::ProtectedCollision { .. })
        ));
    }

    #[test]
    fn conditional_type_one_and_remove_rules_are_enforced() {
        let mut conditional = policy("0008,103E", DicomVr::LO, "1C", "caller_settable");
        conditional.condition = Some(Condition::ParameterEquals {
            parameter: "need_description".into(),
            value: Value::Bool(true),
        });
        let resolver = AttributeResolver::new(vec![conditional]).unwrap();
        let context = ResolveContext {
            parameters: BTreeMap::from([("need_description".into(), Value::Bool(true))]),
            ..ResolveContext::default()
        };
        assert!(matches!(
            resolver.resolve(&[], &context),
            Err(ResolveError::MissingRequired { .. })
        ));
        assert!(matches!(
            resolver.resolve(
                &[AttributeLayer {
                    origin: ValueOrigin::InstanceOverride,
                    operations: vec![AttributeOperation::Remove {
                        address: AttributeAddress::from_normalized_tag("0008,103E").unwrap()
                    }]
                }],
                &context
            ),
            Err(ResolveError::RequiredRemoval { .. })
        ));
    }

    fn plan(description: &str) -> ResolvedInstancePlan {
        let template_id = TemplateId("classic/secondary-capture/monochrome".into());
        let version = "1.0.0".parse().unwrap();
        let identities = IdentityAllocator::new(LOCK_HASH, template_id.clone(), version, 1)
            .unwrap()
            .allocate_plan("primary", [(CompositionUidRole::SopInstance, 0)])
            .unwrap();
        ResolvedInstancePlan {
            plan_schema_version: "0.1.0".into(),
            instance_id: "primary".into(),
            template_id,
            template_version: version,
            sop_class_uid: "1.2.840.10008.5.1.4.1.1.7".into(),
            transfer_syntax_uid: "1.2.840.10008.1.2.1".into(),
            identities,
            attributes: vec![ResolvedAttribute {
                address: AttributeAddress::from_normalized_tag("0008,103E").unwrap(),
                vr: DicomVr::LO,
                value: Some(AttributeValue::Primitive(PrimitiveValue::String(
                    description.into(),
                ))),
                origin: ValueOrigin::InstanceOverride,
            }],
            content: vec![],
            references: vec![],
        }
    }

    #[test]
    fn canonical_hash_is_stable_and_input_sensitive() {
        assert_eq!(
            plan("one").canonical_sha256(),
            plan("one").canonical_sha256()
        );
        assert_ne!(
            plan("one").canonical_sha256(),
            plan("two").canonical_sha256()
        );
        let json = serde_json::to_string(&plan("one")).unwrap();
        assert!(!json.contains("/Users/") && !json.contains("/tmp/"));
    }
}
