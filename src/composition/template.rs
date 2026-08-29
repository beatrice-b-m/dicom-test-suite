use std::collections::BTreeSet;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use serde_json::Value;

const TEMPLATE_CATALOG_SCHEMA: &str = include_str!("../../schemas/template-catalog.schema.json");

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TemplateId(pub String);

impl fmt::Display for TemplateId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TemplateVersion {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
}

impl fmt::Display for TemplateVersion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

impl FromStr for TemplateVersion {
    type Err = TemplateError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let parts = value.split('.').collect::<Vec<_>>();
        if parts.len() != 3
            || parts
                .iter()
                .any(|part| part.is_empty() || (part.len() > 1 && part.starts_with('0')))
        {
            return Err(TemplateError::InvalidVersion(value.to_string()));
        }
        let parse = |part: &str| {
            part.parse::<u64>()
                .map_err(|_| TemplateError::InvalidVersion(value.to_string()))
        };
        Ok(Self {
            major: parse(parts[0])?,
            minor: parse(parts[1])?,
            patch: parse(parts[2])?,
        })
    }
}

impl Serialize for TemplateVersion {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for TemplateVersion {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        String::deserialize(deserializer)?
            .parse()
            .map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TemplateStatus {
    Planned,
    Qualified,
    Unavailable,
    Deprecated,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemplateRequirements {
    pub features: Vec<String>,
    pub external_codecs: Vec<String>,
    pub providers: Vec<String>,
    pub external_validators: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransferSyntaxDescriptor {
    pub uid: String,
    pub name: String,
    pub default: bool,
    pub determinism: String,
    pub requirements: TemplateRequirements,
    pub limitations: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TemplateDescriptor {
    pub template_id: TemplateId,
    pub template_version: TemplateVersion,
    pub status: TemplateStatus,
    pub iod_name: String,
    pub sop_class_name: String,
    pub sop_class_uid: String,
    pub default_modality: String,
    pub artifact_kind: String,
    pub determinism: String,
    pub modules: Vec<Value>,
    pub attributes: Vec<Value>,
    pub content_slots: Vec<Value>,
    pub reference_slots: Vec<Value>,
    pub default_bundle: Value,
    pub transfer_syntaxes: Vec<TransferSyntaxDescriptor>,
    pub requirements: TemplateRequirements,
    pub validation: Value,
    pub standards_evidence: Vec<Value>,
    pub limitations: Vec<String>,
    pub qualification_owner: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TemplateCatalog {
    pub template_catalog_schema_version: String,
    pub standards_lock_sha256: String,
    pub templates: Vec<TemplateDescriptor>,
}

impl TemplateCatalog {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, TemplateError> {
        let path = path.as_ref();
        let bytes = fs::read(path).map_err(|source| TemplateError::Read {
            path: path.to_path_buf(),
            source,
        })?;
        Self::from_slice(&bytes)
    }

    pub fn from_slice(bytes: &[u8]) -> Result<Self, TemplateError> {
        let value: Value = serde_json::from_slice(bytes).map_err(TemplateError::Parse)?;
        let schema: Value =
            serde_json::from_str(TEMPLATE_CATALOG_SCHEMA).expect("embedded catalog schema");
        let validator = jsonschema::validator_for(&schema).expect("catalog schema compiles");
        let errors = validator
            .iter_errors(&value)
            .map(|error| error.to_string())
            .collect::<Vec<_>>();
        if !errors.is_empty() {
            return Err(TemplateError::Schema(errors));
        }
        let catalog: Self = serde_json::from_value(value).map_err(TemplateError::Parse)?;
        catalog.validate_uniqueness()?;
        Ok(catalog)
    }

    fn validate_uniqueness(&self) -> Result<(), TemplateError> {
        let mut identities = BTreeSet::new();
        for template in &self.templates {
            let identity = (template.template_id.clone(), template.template_version);
            if !identities.insert(identity.clone()) {
                return Err(TemplateError::DuplicateTemplate {
                    template_id: identity.0,
                    version: identity.1,
                });
            }
            let defaults = template
                .transfer_syntaxes
                .iter()
                .filter(|transfer_syntax| transfer_syntax.default)
                .count();
            if defaults != 1 {
                return Err(TemplateError::DefaultTransferSyntaxCount {
                    template_id: template.template_id.clone(),
                    version: template.template_version,
                    count: defaults,
                });
            }
            ensure_unique_values(template, "attribute tag", &template.attributes, "tag")?;
            ensure_unique_values(template, "content slot", &template.content_slots, "slot")?;
            ensure_unique_values(
                template,
                "reference role",
                &template.reference_slots,
                "role",
            )?;
            let mut transfer_syntax_uids = BTreeSet::new();
            for transfer_syntax in &template.transfer_syntaxes {
                if !transfer_syntax_uids.insert(&transfer_syntax.uid) {
                    return Err(TemplateError::DuplicateMember {
                        template_id: template.template_id.clone(),
                        version: template.template_version,
                        member_kind: "transfer syntax UID",
                        member: transfer_syntax.uid.clone(),
                    });
                }
            }
        }
        Ok(())
    }

    pub fn resolve_qualified(
        &self,
        template_id: &TemplateId,
        version: Option<TemplateVersion>,
    ) -> Result<&TemplateDescriptor, TemplateError> {
        let mut matching = self
            .templates
            .iter()
            .filter(|template| &template.template_id == template_id)
            .collect::<Vec<_>>();
        if matching.is_empty() {
            return Err(TemplateError::UnknownTemplate(template_id.clone()));
        }
        if let Some(version) = version {
            let template = matching
                .into_iter()
                .find(|template| template.template_version == version)
                .ok_or_else(|| TemplateError::UnknownTemplateVersion {
                    template_id: template_id.clone(),
                    version,
                })?;
            return require_qualified(template);
        }
        matching.retain(|template| template.status == TemplateStatus::Qualified);
        matching.sort_by_key(|template| template.template_version);
        matching
            .pop()
            .ok_or_else(|| TemplateError::NoQualifiedVersion(template_id.clone()))
    }

    pub fn evaluate_requirements(
        &self,
        template: &TemplateDescriptor,
        transfer_syntax_uid: &str,
        capabilities: &CapabilitySet,
    ) -> Result<Vec<RequirementGap>, TemplateError> {
        let transfer_syntax = template
            .transfer_syntaxes
            .iter()
            .find(|transfer_syntax| transfer_syntax.uid == transfer_syntax_uid)
            .ok_or_else(|| TemplateError::UnsupportedTransferSyntax {
                template_id: template.template_id.clone(),
                uid: transfer_syntax_uid.to_string(),
            })?;
        let mut gaps = BTreeSet::new();
        collect_gaps(&template.requirements, capabilities, &mut gaps);
        collect_gaps(&transfer_syntax.requirements, capabilities, &mut gaps);
        Ok(gaps.into_iter().collect())
    }

    pub fn default_transfer_syntax<'a>(
        &self,
        template: &'a TemplateDescriptor,
    ) -> &'a TransferSyntaxDescriptor {
        template
            .transfer_syntaxes
            .iter()
            .find(|transfer_syntax| transfer_syntax.default)
            .expect("catalog uniqueness requires one default transfer syntax")
    }
}

fn require_qualified(template: &TemplateDescriptor) -> Result<&TemplateDescriptor, TemplateError> {
    if template.status == TemplateStatus::Qualified {
        Ok(template)
    } else {
        Err(TemplateError::VersionNotQualified {
            template_id: template.template_id.clone(),
            version: template.template_version,
            status: template.status,
        })
    }
}

fn ensure_unique_values(
    template: &TemplateDescriptor,
    member_kind: &'static str,
    values: &[Value],
    key: &'static str,
) -> Result<(), TemplateError> {
    let mut members = BTreeSet::new();
    for value in values {
        let member = value[key]
            .as_str()
            .expect("schema requires descriptor member identity");
        if !members.insert(member) {
            return Err(TemplateError::DuplicateMember {
                template_id: template.template_id.clone(),
                version: template.template_version,
                member_kind,
                member: member.to_string(),
            });
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CapabilitySet {
    pub features: BTreeSet<String>,
    pub external_codecs: BTreeSet<String>,
    pub providers: BTreeSet<String>,
    pub external_validators: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct RequirementGap {
    pub kind: &'static str,
    pub id: String,
}

fn collect_gaps(
    requirements: &TemplateRequirements,
    capabilities: &CapabilitySet,
    gaps: &mut BTreeSet<RequirementGap>,
) {
    let groups: [(&'static str, &[String], &BTreeSet<String>); 4] = [
        ("feature", &requirements.features, &capabilities.features),
        (
            "external_codec",
            &requirements.external_codecs,
            &capabilities.external_codecs,
        ),
        ("provider", &requirements.providers, &capabilities.providers),
        (
            "external_validator",
            &requirements.external_validators,
            &capabilities.external_validators,
        ),
    ];
    for (kind, required, available) in groups {
        for id in required {
            if !available.contains(id) {
                gaps.insert(RequirementGap {
                    kind,
                    id: id.clone(),
                });
            }
        }
    }
}

#[derive(Debug)]
pub enum TemplateError {
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    Parse(serde_json::Error),
    Schema(Vec<String>),
    InvalidVersion(String),
    DuplicateTemplate {
        template_id: TemplateId,
        version: TemplateVersion,
    },
    DuplicateMember {
        template_id: TemplateId,
        version: TemplateVersion,
        member_kind: &'static str,
        member: String,
    },
    DefaultTransferSyntaxCount {
        template_id: TemplateId,
        version: TemplateVersion,
        count: usize,
    },
    UnknownTemplate(TemplateId),
    UnknownTemplateVersion {
        template_id: TemplateId,
        version: TemplateVersion,
    },
    NoQualifiedVersion(TemplateId),
    VersionNotQualified {
        template_id: TemplateId,
        version: TemplateVersion,
        status: TemplateStatus,
    },
    UnsupportedTransferSyntax {
        template_id: TemplateId,
        uid: String,
    },
}

impl fmt::Display for TemplateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read { path, source } => write!(formatter, "read {}: {source}", path.display()),
            Self::Parse(source) => write!(formatter, "parse template catalog: {source}"),
            Self::Schema(errors) => {
                write!(formatter, "template catalog schema: {}", errors.join("; "))
            }
            Self::InvalidVersion(version) => {
                write!(formatter, "invalid template version {version:?}")
            }
            Self::DuplicateTemplate {
                template_id,
                version,
            } => write!(
                formatter,
                "duplicate template {template_id} version {version}"
            ),
            Self::DuplicateMember {
                template_id,
                version,
                member_kind,
                member,
            } => write!(
                formatter,
                "template {template_id} {version} repeats {member_kind} {member}"
            ),
            Self::DefaultTransferSyntaxCount {
                template_id,
                version,
                count,
            } => write!(
                formatter,
                "template {template_id} {version} has {count} default transfer syntaxes"
            ),
            Self::UnknownTemplate(template_id) => {
                write!(formatter, "unknown template {template_id}")
            }
            Self::UnknownTemplateVersion {
                template_id,
                version,
            } => write!(
                formatter,
                "unknown template {template_id} version {version}"
            ),
            Self::NoQualifiedVersion(template_id) => {
                write!(formatter, "template {template_id} has no qualified version")
            }
            Self::VersionNotQualified {
                template_id,
                version,
                status,
            } => write!(
                formatter,
                "template {template_id} version {version} is {status:?}, not qualified"
            ),
            Self::UnsupportedTransferSyntax { template_id, uid } => write!(
                formatter,
                "template {template_id} does not support transfer syntax {uid}"
            ),
        }
    }
}

impl std::error::Error for TemplateError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> Value {
        serde_json::from_str(include_str!(
            "../../tests/fixtures/composition/catalog/valid.json"
        ))
        .unwrap()
    }

    fn qualified_catalog_with_versions() -> TemplateCatalog {
        let mut value = fixture();
        value["templates"][0]["status"] = Value::String("qualified".into());
        let mut older = value["templates"][0].clone();
        older["template_version"] = Value::String("0.9.0".into());
        value["templates"].as_array_mut().unwrap().push(older);
        TemplateCatalog::from_slice(&serde_json::to_vec(&value).unwrap()).unwrap()
    }

    #[test]
    fn loads_schema_valid_descriptor() {
        let catalog =
            TemplateCatalog::from_slice(&serde_json::to_vec(&fixture()).unwrap()).unwrap();
        assert_eq!(catalog.templates.len(), 1);
        assert_eq!(catalog.templates[0].template_version.to_string(), "1.0.0");
        assert_eq!(
            catalog.default_transfer_syntax(&catalog.templates[0]).uid,
            "1.2.840.10008.1.2.1"
        );
    }

    #[test]
    fn rejects_duplicate_template_identity_and_members() {
        let mut duplicate = fixture();
        let descriptor = duplicate["templates"][0].clone();
        duplicate["templates"]
            .as_array_mut()
            .unwrap()
            .push(descriptor);
        assert!(matches!(
            TemplateCatalog::from_slice(&serde_json::to_vec(&duplicate).unwrap()),
            Err(TemplateError::DuplicateTemplate { .. })
        ));

        let mut repeated_slot = fixture();
        let slot = repeated_slot["templates"][0]["content_slots"][0].clone();
        repeated_slot["templates"][0]["content_slots"]
            .as_array_mut()
            .unwrap()
            .push(slot);
        assert!(matches!(
            TemplateCatalog::from_slice(&serde_json::to_vec(&repeated_slot).unwrap()),
            Err(TemplateError::DuplicateMember {
                member_kind: "content slot",
                ..
            })
        ));
    }

    #[test]
    fn resolves_exact_or_latest_qualified_version() {
        let catalog = qualified_catalog_with_versions();
        let id = TemplateId("classic/secondary-capture/monochrome".into());
        assert_eq!(
            catalog
                .resolve_qualified(&id, None)
                .unwrap()
                .template_version,
            "1.0.0".parse().unwrap()
        );
        assert_eq!(
            catalog
                .resolve_qualified(&id, Some("0.9.0".parse().unwrap()))
                .unwrap()
                .template_version,
            "0.9.0".parse().unwrap()
        );
    }

    #[test]
    fn planned_versions_do_not_resolve_as_qualified() {
        let catalog =
            TemplateCatalog::from_slice(&serde_json::to_vec(&fixture()).unwrap()).unwrap();
        let id = TemplateId("classic/secondary-capture/monochrome".into());
        assert!(matches!(
            catalog.resolve_qualified(&id, None),
            Err(TemplateError::NoQualifiedVersion(_))
        ));
    }

    #[test]
    fn evaluates_template_and_transfer_syntax_requirements() {
        let mut value = fixture();
        value["templates"][0]["status"] = Value::String("qualified".into());
        value["templates"][0]["requirements"]["features"] = serde_json::json!(["jpeg"]);
        value["templates"][0]["transfer_syntaxes"][0]["requirements"]["providers"] =
            serde_json::json!(["pixel_provider"]);
        let catalog = TemplateCatalog::from_slice(&serde_json::to_vec(&value).unwrap()).unwrap();
        let gaps = catalog
            .evaluate_requirements(
                &catalog.templates[0],
                "1.2.840.10008.1.2.1",
                &CapabilitySet::default(),
            )
            .unwrap();
        assert_eq!(
            gaps,
            vec![
                RequirementGap {
                    kind: "external_validator",
                    id: "dicom_validator".into()
                },
                RequirementGap {
                    kind: "feature",
                    id: "jpeg".into()
                },
                RequirementGap {
                    kind: "provider",
                    id: "pixel_provider".into()
                },
            ]
        );
    }
}
