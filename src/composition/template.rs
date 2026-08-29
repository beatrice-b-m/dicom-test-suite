use std::collections::BTreeSet;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::family::ClassicFamilyProfile;
use super::{PhotometricInterpretation, SampleType};

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

#[derive(Debug, Deserialize)]
struct CatalogDocument {
    template_catalog_schema_version: String,
    standards_lock_sha256: String,
    templates: Vec<TemplateDescriptor>,
    #[serde(default)]
    classic_family_templates: Vec<ClassicFamilyTemplateDeclaration>,
    #[serde(default)]
    advanced_family_templates: Vec<ClassicFamilyTemplateDeclaration>,
}

#[derive(Debug, Deserialize)]
struct ClassicFamilyTemplateDeclaration {
    template_id: TemplateId,
    template_version: TemplateVersion,
    status: TemplateStatus,
    iod_name: String,
    sop_class_name: String,
    sop_class_uid: String,
    default_modality: String,
    artifact_kind: String,
    transfer_syntaxes: Vec<TransferSyntaxDescriptor>,
    standards_evidence: Vec<Value>,
    limitations: Vec<String>,
    qualification_owner: String,
    #[serde(default)]
    reference_slots: Vec<Value>,
    #[serde(default)]
    default_bundle: Option<Value>,
    #[serde(default)]
    modules: Vec<Value>,
    #[serde(default)]
    content_slots: Vec<Value>,
    #[serde(default)]
    validation: Option<Value>,
    #[serde(default)]
    reference_only: bool,
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
        let document: CatalogDocument =
            serde_json::from_value(value).map_err(TemplateError::Parse)?;
        let mut templates = document.templates;
        for declaration in document.classic_family_templates {
            templates.push(declaration.expand()?);
        }
        for declaration in document.advanced_family_templates {
            templates.push(declaration.expand_advanced());
        }
        let catalog = Self {
            template_catalog_schema_version: document.template_catalog_schema_version,
            standards_lock_sha256: document.standards_lock_sha256,
            templates,
        };
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

    pub fn render_reference_markdown(&self) -> String {
        let mut output = String::from(
            "# Composition template reference\n\n\
             This file is rendered from `templates/catalog.json`. Use `templates describe` for the complete attribute policies, content constraints, requirements, evidence, and limitations of one template.\n\n\
             | Template | IOD | SOP Class UID | Transfer syntaxes | Determinism | Independent routes |\n\
             |---|---|---|---|---|---|\n",
        );
        let mut templates = self.templates.iter().collect::<Vec<_>>();
        templates.sort_by_key(|template| (&template.template_id, template.template_version));
        for template in templates {
            let transfer_syntaxes = template
                .transfer_syntaxes
                .iter()
                .map(|transfer_syntax| {
                    if transfer_syntax.default {
                        format!("`{}` (default)", transfer_syntax.uid)
                    } else {
                        format!("`{}`", transfer_syntax.uid)
                    }
                })
                .collect::<Vec<_>>()
                .join("<br>");
            let routes = template.validation["independent_routes"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|route| route["adapter_id"].as_str())
                .collect::<Vec<_>>()
                .join(", ");
            let iod = template.iod_name.replace('|', "\\|");
            output.push_str(&format!(
                "| `{}`@{} | {} | `{}` | {} | {} | {} |\n",
                template.template_id,
                template.template_version,
                iod,
                template.sop_class_uid,
                transfer_syntaxes,
                template.determinism,
                routes,
            ));
        }
        output
    }
}

impl ClassicFamilyTemplateDeclaration {
    fn expand(self) -> Result<TemplateDescriptor, TemplateError> {
        let profile = ClassicFamilyProfile::for_template(&self.template_id)
            .ok_or_else(|| TemplateError::UnknownClassicFamily(self.template_id.clone()))?;
        let declared = (
            self.iod_name.as_str(),
            self.sop_class_name.as_str(),
            self.sop_class_uid.as_str(),
            self.default_modality.as_str(),
        );
        let executable = (
            profile.iod_name,
            profile.sop_class_name,
            profile.sop_class_uid,
            profile.modality,
        );
        if declared != executable {
            return Err(TemplateError::ClassicFamilyIdentityMismatch {
                template_id: self.template_id,
            });
        }
        let declared_default = self
            .transfer_syntaxes
            .iter()
            .find(|syntax| syntax.default)
            .map(|syntax| syntax.uid.as_str());
        if declared_default != Some(profile.default_transfer_syntax_uid) {
            return Err(TemplateError::ClassicFamilyDefaultTransferSyntaxMismatch {
                template_id: self.template_id,
            });
        }
        let modules = classic_family_modules(&profile);
        let attributes = classic_family_attribute_policies(&profile);
        let content_slots = vec![classic_family_content_slot(&profile)];
        Ok(TemplateDescriptor {
            template_id: profile.template_id,
            template_version: self.template_version,
            status: self.status,
            iod_name: self.iod_name,
            sop_class_name: self.sop_class_name,
            sop_class_uid: self.sop_class_uid,
            default_modality: self.default_modality,
            artifact_kind: self.artifact_kind,
            determinism: "byte_stable".into(),
            modules,
            attributes,
            content_slots,
            reference_slots: Vec::new(),
            default_bundle: serde_json::json!({ "dependencies": [] }),
            transfer_syntaxes: self.transfer_syntaxes,
            requirements: TemplateRequirements {
                features: Vec::new(),
                external_codecs: Vec::new(),
                providers: Vec::new(),
                external_validators: vec!["dicom_validator".into()],
            },
            validation: serde_json::json!({
                "generic_rule_ids": ["meta_identity", "resolved_attributes"],
                "template_rule_ids": ["classic_family_iod"],
                "content_rule_ids": ["content_integrity", "native_pixel_length", "pixel_model"],
                "independent_routes": [{
                    "adapter_id": "dicom_validator",
                    "kind": "iod",
                    "required_for_qualification": true
                }]
            }),
            standards_evidence: self.standards_evidence,
            limitations: self.limitations,
            qualification_owner: self.qualification_owner,
        })
    }
}

impl ClassicFamilyTemplateDeclaration {
    fn expand_advanced(self) -> TemplateDescriptor {
        let wsi = self.artifact_kind == "whole_slide_image";
        let reference_only = self.reference_only
            || matches!(
                self.artifact_kind.as_str(),
                "presentation_state" | "registration"
            );
        let declared_content_slots = self.content_slots;
        let pixel_bearing = declared_content_slots.is_empty() && !reference_only;
        let mut attributes = [
            ("0008,0016", "SOPClassUID", "UI"),
            ("0008,0018", "SOPInstanceUID", "UI"),
            ("0020,000D", "StudyInstanceUID", "UI"),
            ("0020,000E", "SeriesInstanceUID", "UI"),
        ]
        .into_iter()
        .map(|(tag, keyword, vr)| {
            json!({
                "tag": tag,
                "keyword": keyword,
                "vr": vr,
                "requirement": "1",
                "behavior": "protected",
                "condition": null,
                "default": null,
                "description": "Derived from the selected template, identity plan, or typed frame content."
            })
        })
        .collect::<Vec<_>>();
        if pixel_bearing {
            attributes.extend(
                [
                    ("0028,0008", "NumberOfFrames", "IS"),
                    ("7FE0,0010", "PixelData", "OB"),
                ]
                .into_iter()
                .map(|(tag, keyword, vr)| {
                    json!({
                        "tag": tag, "keyword": keyword, "vr": vr,
                        "requirement": "1", "behavior": "protected", "condition": null,
                        "default": null,
                        "description": "Derived from the selected template or typed content."
                    })
                }),
            );
        }
        TemplateDescriptor {
            template_id: self.template_id,
            template_version: self.template_version,
            status: self.status,
            iod_name: self.iod_name,
            sop_class_name: self.sop_class_name,
            sop_class_uid: self.sop_class_uid,
            default_modality: self.default_modality,
            artifact_kind: self.artifact_kind,
            determinism: "byte_stable".into(),
            modules: if self.modules.is_empty() { vec![json!({
                    "name": if wsi { "VL Whole Slide Microscopy Image" } else if reference_only { "Reference Graph" } else { "Multi-frame Functional Groups" },
                    "usage": "mandatory",
                    "condition": null
                })] } else { self.modules },
            attributes,
            content_slots: if !declared_content_slots.is_empty() {
                declared_content_slots
            } else if reference_only {
                vec![]
            } else {
                vec![json!({
                    "slot": "pixels",
                    "kind": "native_pixels",
                    "required": true,
                    "default_provider": "qualified_curated_default",
                    "allowed_sources": ["default", "local_file"],
                    "constraints": {
                        "photometric_interpretations": if wsi { json!(["RGB"]) } else { json!(["MONOCHROME2"]) },
                        "samples_per_pixel": if wsi { json!([3]) } else { json!([1]) },
                        "bits_allocated": if wsi { json!([8]) } else { json!([16]) },
                        "sample_types": ["uint"],
                        "min_frames": 1,
                        "max_frames": 65535
                    },
                    "description": "Default qualified frames or an exact-shape caller native frame payload."
                })]
            },
            reference_slots: self.reference_slots,
            default_bundle: self
                .default_bundle
                .unwrap_or_else(|| json!({ "dependencies": [] })),
            transfer_syntaxes: self.transfer_syntaxes,
            requirements: TemplateRequirements {
                features: vec![],
                external_codecs: vec![],
                providers: vec![],
                external_validators: vec!["dicom_validator".into()],
            },
            validation: self.validation.unwrap_or_else(|| json!({
                "generic_rule_ids": ["meta_identity", "resolved_attributes"],
                "template_rule_ids": if reference_only { json!(["reference_closure", "derived_object"]) } else { json!(["functional_groups", "dimensions", if wsi { "tiling" } else { "enhanced_image" }]) },
                "content_rule_ids": if reference_only { json!([]) } else { json!(["content_integrity", "native_pixel_length"]) },
                "independent_routes": [{"adapter_id":"dicom_validator","kind":"iod","required_for_qualification":true}]
            })),
            standards_evidence: self.standards_evidence,
            limitations: self.limitations,
            qualification_owner: self.qualification_owner,
        }
    }
}

fn classic_family_modules(profile: &ClassicFamilyProfile) -> Vec<Value> {
    let mut names = vec![
        "Patient",
        "General Study",
        "General Series",
        "General Equipment",
        "General Image",
        "Image Pixel",
        "SOP Common",
    ];
    if profile.include_geometry {
        names.push("Frame of Reference");
        names.push("Image Plane");
    }
    names
        .into_iter()
        .map(|name| {
            serde_json::json!({
                "name": name,
                "usage": "mandatory",
                "condition": null
            })
        })
        .collect()
}

fn classic_family_attribute_policies(profile: &ClassicFamilyProfile) -> Vec<Value> {
    vec![
        serde_json::json!({
            "tag": "0008,0016", "keyword": "SOPClassUID", "vr": "UI",
            "requirement": "1", "behavior": "protected", "condition": null,
            "default": null, "description": "Protected by the selected family profile."
        }),
        serde_json::json!({
            "tag": "0008,0018", "keyword": "SOPInstanceUID", "vr": "UI",
            "requirement": "1", "behavior": "derived", "condition": null,
            "default": null, "description": "Derived from the deterministic identity plan."
        }),
        serde_json::json!({
            "tag": "0008,0060", "keyword": "Modality", "vr": "CS",
            "requirement": "1", "behavior": "defaulted", "condition": null,
            "default": { "kind": "literal", "value": profile.modality },
            "description": "Stable family modality default."
        }),
        serde_json::json!({
            "tag": "0010,0010", "keyword": "PatientName", "vr": "PN",
            "requirement": "2", "behavior": "caller_settable", "condition": null,
            "default": null, "description": "Caller may replace the synthetic non-PHI default."
        }),
        serde_json::json!({
            "tag": "0020,0020", "keyword": "PatientOrientation", "vr": "CS",
            "requirement": "2C", "behavior": "defaulted",
            "condition": { "operator": "parameter_equals", "parameter": "patient_orientation_required", "value": true },
            "default": { "kind": "empty" },
            "description": "Conditional image-plane orientation is represented explicitly."
        }),
        serde_json::json!({
            "tag": "0028,0010", "keyword": "Rows", "vr": "US",
            "requirement": "1", "behavior": "protected", "condition": null,
            "default": null, "description": "Derived from and protected by the pixel plan."
        }),
        serde_json::json!({
            "tag": "7FE0,0010", "keyword": "PixelData", "vr": "OW",
            "requirement": "1C", "behavior": "derived",
            "condition": { "operator": "content_slot_set", "slot": "pixels" },
            "default": null, "description": "Materialized from the validated native pixel slot."
        }),
    ]
}

fn classic_family_content_slot(profile: &ClassicFamilyProfile) -> Value {
    let shape = &profile.default_shape;
    let photometric = match shape.photometric_interpretation {
        PhotometricInterpretation::Monochrome1 => "MONOCHROME1",
        PhotometricInterpretation::Monochrome2 => "MONOCHROME2",
        PhotometricInterpretation::Rgb => "RGB",
        PhotometricInterpretation::PaletteColor => "PALETTE COLOR",
        PhotometricInterpretation::YbrFull => "YBR_FULL",
        PhotometricInterpretation::YbrFull422 => "YBR_FULL_422",
    };
    let sample_type = match shape.sample_type {
        SampleType::UnsignedInteger => "uint",
        SampleType::SignedInteger => "int",
        SampleType::Bit1 => "bit1",
        SampleType::Float32 => "float32",
        SampleType::Float64 => "float64",
    };
    let multiframe = matches!(
        profile.kind,
        super::ClassicFamilyKind::ScSingleBit
            | super::ClassicFamilyKind::ScGrayscaleByte
            | super::ClassicFamilyKind::UltrasoundMultiFrame
            | super::ClassicFamilyKind::NuclearMedicine
    );
    serde_json::json!({
        "slot": "pixels",
        "kind": "native_pixels",
        "required": true,
        "default_provider": "classic_family_default_pixels",
        "allowed_sources": ["default", "local_file"],
        "constraints": {
            "photometric_interpretations": [photometric],
            "samples_per_pixel": [shape.samples_per_pixel],
            "bits_allocated": [shape.bits_allocated],
            "sample_types": [sample_type],
            "min_frames": 1,
            "max_frames": if multiframe { 65535 } else { 1 }
        },
        "description": "Native pixel input must match the qualified family pixel model exactly."
    })
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
    UnknownClassicFamily(TemplateId),
    ClassicFamilyIdentityMismatch {
        template_id: TemplateId,
    },
    ClassicFamilyDefaultTransferSyntaxMismatch {
        template_id: TemplateId,
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
            Self::UnknownClassicFamily(template_id) => {
                write!(formatter, "unknown classic family profile {template_id}")
            }
            Self::ClassicFamilyIdentityMismatch { template_id } => write!(
                formatter,
                "classic family declaration identity does not match executable profile {template_id}"
            ),
            Self::ClassicFamilyDefaultTransferSyntaxMismatch { template_id } => write!(
                formatter,
                "classic family declaration default transfer syntax does not match executable profile {template_id}"
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

    #[test]
    fn expands_classic_family_declaration_into_public_descriptor() {
        let mut value = fixture();
        value["classic_family_templates"] = serde_json::json!([{
            "template_id": "classic/ct",
            "template_version": "1.0.0",
            "status": "planned",
            "iod_name": "CT Image",
            "sop_class_name": "CT Image Storage",
            "sop_class_uid": "1.2.840.10008.5.1.4.1.1.2",
            "default_modality": "CT",
            "artifact_kind": "classic_image",
            "transfer_syntaxes": [{
                "uid": "1.2.840.10008.1.2.1",
                "name": "Explicit VR Little Endian",
                "default": true,
                "determinism": "byte_stable",
                "requirements": { "features": [], "external_codecs": [], "providers": [], "external_validators": [] },
                "limitations": []
            }],
            "standards_evidence": [{
                "source": "official-dicom-standard",
                "part": "PS3.3",
                "anchor": "CT Image IOD module table",
                "reason": "Locks mandatory CT modules.",
                "checked_date": "2026-08-28",
                "source_note": "standards/source-notes/phase-2-ct-geometry.md"
            }],
            "limitations": ["Native pixels only."],
            "qualification_owner": "classic_ct"
        }]);
        let catalog = TemplateCatalog::from_slice(&serde_json::to_vec(&value).unwrap()).unwrap();
        let descriptor = catalog
            .templates
            .iter()
            .find(|template| template.template_id.0 == "classic/ct")
            .unwrap();
        let behaviors = descriptor
            .attributes
            .iter()
            .filter_map(|attribute| attribute["behavior"].as_str())
            .collect::<BTreeSet<_>>();
        assert_eq!(
            behaviors,
            BTreeSet::from(["caller_settable", "defaulted", "derived", "protected"])
        );
        assert_eq!(descriptor.content_slots[0]["slot"], "pixels");
        assert!(
            descriptor
                .modules
                .iter()
                .any(|module| module["name"] == "Image Plane")
        );
    }

    #[test]
    fn rejects_classic_family_declaration_identity_drift() {
        let mut value = fixture();
        value["classic_family_templates"] = serde_json::json!([{
            "template_id": "classic/ct",
            "template_version": "1.0.0",
            "status": "planned",
            "iod_name": "CT Image",
            "sop_class_name": "CT Image Storage",
            "sop_class_uid": "1.2.840.10008.5.1.4.1.1.4",
            "default_modality": "CT",
            "artifact_kind": "classic_image",
            "transfer_syntaxes": [{
                "uid": "1.2.840.10008.1.2.1", "name": "Explicit VR Little Endian",
                "default": true, "determinism": "byte_stable",
                "requirements": { "features": [], "external_codecs": [], "providers": [], "external_validators": [] },
                "limitations": []
            }],
            "standards_evidence": [{
                "source": "official-dicom-standard", "part": "PS3.3",
                "anchor": "CT Image IOD module table", "reason": "Locks mandatory CT modules.",
                "checked_date": "2026-08-28"
            }],
            "limitations": [], "qualification_owner": "classic_ct"
        }]);
        assert!(matches!(
            TemplateCatalog::from_slice(&serde_json::to_vec(&value).unwrap()),
            Err(TemplateError::ClassicFamilyIdentityMismatch { .. })
        ));
    }

    #[test]
    fn markdown_reference_is_sorted_and_descriptor_driven() {
        let catalog = TemplateCatalog::load("templates/catalog.json").unwrap();
        let reference = catalog.render_reference_markdown();
        assert!(reference.contains("classic/ct`@1.0.0"));
        assert!(reference.contains("dicom_validator"));
        assert_eq!(reference.matches("\n| `").count(), catalog.templates.len());
        let cr = reference.find("classic/cr`@1.0.0").unwrap();
        let ct = reference.find("classic/ct`@1.0.0").unwrap();
        assert!(cr < ct);
    }
}
