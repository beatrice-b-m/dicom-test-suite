use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{
    CompositionSpec, IdentityChoice, SpecInstance, SpecReference, TemplateCatalog, TemplateError,
    TemplateId, TemplateSelector, TemplateVersion,
};

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct DefaultBundleDescriptor {
    #[serde(default)]
    pub dependencies: Vec<DefaultBundleDependency>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct DefaultBundleDependency {
    pub logical_role: String,
    pub reference_role: String,
    pub instance_suffix: Option<String>,
    pub template_id: TemplateId,
    pub template_version: TemplateVersion,
    #[serde(default)]
    pub frames: Vec<u32>,
    #[serde(default)]
    pub parameters: BTreeMap<String, Value>,
    #[serde(default = "default_true")]
    pub share_study: bool,
    #[serde(default)]
    pub share_series: bool,
    #[serde(default)]
    pub share_frame_of_reference: bool,
}

const fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BundleMemberProvenance {
    pub instance_id: String,
    pub requested: bool,
    pub bundle_root_instance_id: String,
    pub bundle_role: String,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BundleResolution {
    pub spec: CompositionSpec,
    pub members: BTreeMap<String, BundleMemberProvenance>,
}

impl BundleResolution {
    pub fn member(&self, instance_id: &str) -> &BundleMemberProvenance {
        self.members
            .get(instance_id)
            .expect("bundle resolver records every instance")
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct BundleResolver;

impl BundleResolver {
    pub fn resolve(
        &self,
        mut spec: CompositionSpec,
        catalog: &TemplateCatalog,
    ) -> Result<BundleResolution, BundleError> {
        let requested_ids = spec
            .instances
            .iter()
            .map(|instance| instance.instance_id.clone())
            .collect::<BTreeSet<_>>();
        let mut ids = requested_ids.clone();
        let mut members = spec
            .instances
            .iter()
            .map(|instance| {
                (
                    instance.instance_id.clone(),
                    BundleMemberProvenance {
                        instance_id: instance.instance_id.clone(),
                        requested: true,
                        bundle_root_instance_id: instance.instance_id.clone(),
                        bundle_role: "root".into(),
                        source: "requested".into(),
                    },
                )
            })
            .collect::<BTreeMap<_, _>>();
        let mut pending = (0..spec.instances.len()).collect::<VecDeque<_>>();

        while let Some(index) = pending.pop_front() {
            let instance_id = spec.instances[index].instance_id.clone();
            let provenance = members
                .get(&instance_id)
                .expect("current instance has provenance")
                .clone();
            let template = catalog.resolve_qualified(
                &spec.instances[index].template.id,
                spec.instances[index].template.version,
            )?;
            let bundle: DefaultBundleDescriptor =
                serde_json::from_value(template.default_bundle.clone()).map_err(|source| {
                    BundleError::Descriptor {
                        template_id: template.template_id.0.clone(),
                        message: source.to_string(),
                    }
                })?;
            let mut dependency_roles = BTreeSet::new();
            for dependency in bundle.dependencies {
                validate_dependency(&template.template_id, &dependency)?;
                if !dependency_roles.insert(dependency.reference_role.clone()) {
                    return Err(BundleError::DuplicateDependencyRole {
                        template_id: template.template_id.0.clone(),
                        role: dependency.reference_role,
                    });
                }
                if spec.instances[index]
                    .references
                    .iter()
                    .any(|reference| reference.role == dependency.reference_role)
                {
                    continue;
                }

                let suffix = dependency
                    .instance_suffix
                    .as_deref()
                    .unwrap_or(&dependency.logical_role);
                let target_id = format!("{}__{}", provenance.bundle_root_instance_id, suffix);
                if target_id.len() > 128 {
                    return Err(BundleError::GeneratedIdTooLong(target_id));
                }
                if !ids.contains(&target_id) {
                    let identities =
                        shared_identities(&provenance.bundle_root_instance_id, &dependency);
                    let target = SpecInstance {
                        instance_id: target_id.clone(),
                        template: TemplateSelector {
                            id: dependency.template_id.clone(),
                            version: Some(dependency.template_version),
                        },
                        transfer_syntax_uid: None,
                        identities,
                        attributes: vec![],
                        content: vec![],
                        references: vec![],
                        parameters: dependency.parameters.clone(),
                    };
                    catalog.resolve_qualified(&target.template.id, target.template.version)?;
                    ids.insert(target_id.clone());
                    members.insert(
                        target_id.clone(),
                        BundleMemberProvenance {
                            instance_id: target_id.clone(),
                            requested: false,
                            bundle_root_instance_id: provenance.bundle_root_instance_id.clone(),
                            bundle_role: dependency.logical_role.clone(),
                            source: "default_template_dependency".into(),
                        },
                    );
                    spec.instances.push(target);
                    pending.push_back(spec.instances.len() - 1);
                    if spec.instances.len() as u64 > spec.resource_limits.max_instances {
                        return Err(BundleError::InstanceLimit {
                            count: spec.instances.len(),
                            limit: spec.resource_limits.max_instances,
                        });
                    }
                } else {
                    let member = members
                        .get(&target_id)
                        .ok_or_else(|| BundleError::GeneratedIdCollision(target_id.clone()))?;
                    if member.bundle_root_instance_id != provenance.bundle_root_instance_id
                        || member.bundle_role != dependency.logical_role
                    {
                        return Err(BundleError::GeneratedIdCollision(target_id));
                    }
                }
                spec.instances[index].references.push(SpecReference {
                    role: dependency.reference_role,
                    target_instance_id: target_id,
                    frames: dependency.frames,
                });
            }
        }

        for instance in &spec.instances {
            for reference in &instance.references {
                if !ids.contains(&reference.target_instance_id) {
                    return Err(BundleError::UnknownReferenceTarget {
                        source: instance.instance_id.clone(),
                        target: reference.target_instance_id.clone(),
                    });
                }
            }
        }
        Ok(BundleResolution { spec, members })
    }
}

fn shared_identities(
    root: &str,
    dependency: &DefaultBundleDependency,
) -> BTreeMap<String, IdentityChoice> {
    let mut identities = BTreeMap::new();
    let mut share = |role: &str| {
        identities.insert(
            role.to_string(),
            IdentityChoice::Shared {
                share_with: root.to_string(),
            },
        );
    };
    if dependency.share_study {
        share("study");
    }
    if dependency.share_series {
        share("series");
    }
    if dependency.share_frame_of_reference {
        share("frame_of_reference");
    }
    identities
}

fn validate_dependency(
    template_id: &TemplateId,
    dependency: &DefaultBundleDependency,
) -> Result<(), BundleError> {
    let valid_id = |value: &str| {
        !value.is_empty()
            && value.len() <= 64
            && value
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
            && value
                .as_bytes()
                .first()
                .is_some_and(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
    };
    if dependency.logical_role.is_empty()
        || dependency.reference_role.is_empty()
        || !valid_id(
            dependency
                .instance_suffix
                .as_deref()
                .unwrap_or(&dependency.logical_role),
        )
    {
        return Err(BundleError::InvalidDependency {
            template_id: template_id.0.clone(),
            role: dependency.logical_role.clone(),
        });
    }
    let mut frames = BTreeSet::new();
    if dependency
        .frames
        .iter()
        .any(|frame| *frame == 0 || !frames.insert(*frame))
    {
        return Err(BundleError::InvalidDependencyFrames {
            template_id: template_id.0.clone(),
            role: dependency.logical_role.clone(),
        });
    }
    Ok(())
}

#[derive(Debug)]
pub enum BundleError {
    Template(TemplateError),
    Descriptor {
        template_id: String,
        message: String,
    },
    DuplicateDependencyRole {
        template_id: String,
        role: String,
    },
    InvalidDependency {
        template_id: String,
        role: String,
    },
    InvalidDependencyFrames {
        template_id: String,
        role: String,
    },
    GeneratedIdTooLong(String),
    GeneratedIdCollision(String),
    UnknownReferenceTarget {
        source: String,
        target: String,
    },
    InstanceLimit {
        count: usize,
        limit: u64,
    },
}

impl From<TemplateError> for BundleError {
    fn from(value: TemplateError) -> Self {
        Self::Template(value)
    }
}

impl fmt::Display for BundleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "bundle resolution failed: {self:?}")
    }
}

impl std::error::Error for BundleError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalog(default_bundle: Value) -> TemplateCatalog {
        let base =
            TemplateCatalog::from_slice(include_bytes!("../../templates/catalog.json")).unwrap();
        let mut root = base.templates[0].clone();
        root.template_id = TemplateId("derived/root".into());
        root.sop_class_uid = "1.2.3".into();
        root.default_bundle = default_bundle;
        let mut source = base.templates[1].clone();
        source.template_id = TemplateId("classic/source".into());
        source.sop_class_uid = "1.2.4".into();
        source.default_bundle = serde_json::json!({"dependencies": []});
        TemplateCatalog {
            template_catalog_schema_version: base.template_catalog_schema_version,
            standards_lock_sha256: base.standards_lock_sha256,
            templates: vec![root, source],
        }
    }

    fn spec() -> CompositionSpec {
        serde_json::from_value(serde_json::json!({
            "composition_spec_schema_version": "0.1.0",
            "instances": [{"instance_id":"root","template":{"id":"derived/root"}}]
        }))
        .unwrap()
    }

    #[test]
    fn creates_deterministic_default_source_closure() {
        let catalog = catalog(serde_json::json!({"dependencies":[{
            "logical_role":"image_source", "reference_role":"source_image",
            "instance_suffix":"source", "template_id":"classic/source", "template_version":"1.0.0",
            "share_study":true
        }]}));
        let resolution = BundleResolver.resolve(spec(), &catalog).unwrap();
        assert_eq!(resolution.spec.instances.len(), 2);
        assert_eq!(
            resolution.spec.instances[0].references[0].target_instance_id,
            "root__source"
        );
        assert!(!resolution.member("root__source").requested);
        assert_eq!(
            resolution.member("root__source").bundle_root_instance_id,
            "root"
        );
    }

    #[test]
    fn explicit_reference_suppresses_default_dependency() {
        let catalog = catalog(serde_json::json!({"dependencies":[{
            "logical_role":"image_source", "reference_role":"source_image",
            "instance_suffix":"source", "template_id":"classic/source", "template_version":"1.0.0"
        }]}));
        let mut spec = spec();
        spec.instances.push(SpecInstance {
            instance_id: "caller_source".into(),
            template: TemplateSelector {
                id: TemplateId("classic/source".into()),
                version: None,
            },
            transfer_syntax_uid: None,
            identities: BTreeMap::new(),
            attributes: vec![],
            content: vec![],
            references: vec![],
            parameters: BTreeMap::new(),
        });
        spec.instances[0].references.push(SpecReference {
            role: "source_image".into(),
            target_instance_id: "caller_source".into(),
            frames: vec![],
        });
        let resolution = BundleResolver.resolve(spec, &catalog).unwrap();
        assert_eq!(resolution.spec.instances.len(), 2);
        assert!(!resolution.members.contains_key("root__source"));
    }
}
