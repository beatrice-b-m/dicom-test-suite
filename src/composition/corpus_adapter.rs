//! No-op adapter from the resolved composition model to the neutral run plan.
//!
//! Composition continues to use its established materializer and manifest
//! projector during U1, but every resolved instance is represented in one
//! validated `CorpusPlan` before that materializer can run.

use std::collections::BTreeMap;

use crate::IMPLEMENTATION_VERSION_NAME;
use crate::corpus_plan::{
    ArtifactDependency, ArtifactProvenance, ArtifactResourceEstimate, CORPUS_PLAN_SCHEMA_VERSION,
    CorpusPlan, CorpusPlanError, EncodingPlan, EvidenceIndependence, EvidenceObligation,
    EvidencePlan, FileMetaPolicy, FragmentationPolicy, ImplementationIdentityPlan,
    ItemLengthPolicy, OffsetTablePolicy, OutputPlan, OutputRelativePath, PlannedArtifact,
    PlannedDicomArtifact, PreamblePolicy, PublicationPlan, PublicationTransaction, ResourcePlan,
    SequenceLengthPolicy, ValidationPlan, ValidationRequirement, ValidationRule,
};

#[derive(Debug, Clone)]
pub(crate) enum AdvancedCompositionArtifact {
    Native(PlannedDicomArtifact),
    Imported(crate::corpus_plan::PlannedImportedDicomArtifact),
}

impl From<PlannedDicomArtifact> for AdvancedCompositionArtifact {
    fn from(value: PlannedDicomArtifact) -> Self {
        Self::Native(value)
    }
}

impl AdvancedCompositionArtifact {
    pub(crate) fn native(&self) -> Option<&PlannedDicomArtifact> {
        match self {
            Self::Native(value) => Some(value),
            Self::Imported(_) => None,
        }
    }
    pub(crate) fn resolved_instance_mut(&mut self) -> &mut ResolvedInstancePlan {
        match self {
            Self::Native(value) => &mut value.instance,
            Self::Imported(value) => &mut value.declared_instance,
        }
    }
}

use super::{
    BundleMemberProvenance, CompositionUidRole, ContentMaterialization, ResolvedInstancePlan,
    ResourceLimits,
};

pub(crate) fn resolved_composition_corpus_plan_with_advanced(
    seed: u64,
    plans: &[ResolvedInstancePlan],
    members: &BTreeMap<String, BundleMemberProvenance>,
    limits: &ResourceLimits,
    parallelism: u32,
    advanced: &BTreeMap<String, AdvancedCompositionArtifact>,
    advanced_dependencies: &[ArtifactDependency],
) -> Result<CorpusPlan, CorpusPlanError> {
    let per_artifact_output_limit = limits
        .max_total_output_bytes
        .checked_div(plans.len().max(1) as u64)
        .unwrap_or(limits.max_total_output_bytes)
        .max(1);
    let artifacts = plans
        .iter()
        .enumerate()
        .map(|(index, plan)| {
            if let Some(artifact) = advanced.get(&plan.instance_id) {
                let mut artifact = artifact.clone();
                let (order, provenance, output) = match &mut artifact {
                    AdvancedCompositionArtifact::Native(value) => {
                        (&mut value.order, &mut value.provenance, &mut value.output)
                    }
                    AdvancedCompositionArtifact::Imported(value) => {
                        (&mut value.order, &mut value.provenance, &mut value.output)
                    }
                };
                *order =
                    u64::try_from(index).map_err(|_| CorpusPlanError::ResourceEstimateOverflow)?;
                let member = members
                    .get(&plan.instance_id)
                    .ok_or_else(|| CorpusPlanError::UnknownArtifact(plan.instance_id.clone()))?;
                *provenance = if member.requested {
                    ArtifactProvenance::Requested
                } else {
                    let consumers = advanced_dependencies
                        .iter()
                        .filter_map(|dependency| {
                            (dependency.depends_on == plan.instance_id)
                                .then_some(dependency.artifact_id.clone())
                        })
                        .collect::<Vec<_>>();
                    ArtifactProvenance::Dependency {
                        requested_by: if consumers.is_empty() {
                            vec![member.bundle_root_instance_id.clone()]
                        } else {
                            consumers
                        },
                    }
                };
                output.publish = true;
                Ok(match artifact {
                    AdvancedCompositionArtifact::Native(value) => PlannedArtifact::Dicom(value),
                    AdvancedCompositionArtifact::Imported(value) => {
                        PlannedArtifact::ImportedDicom(value)
                    }
                })
            } else {
                planned_artifact(index, plan, members, per_artifact_output_limit)
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    let advanced_nodes = advanced_dependencies
        .iter()
        .flat_map(|dependency| [&dependency.artifact_id, &dependency.depends_on])
        .collect::<std::collections::BTreeSet<_>>();
    let mut dependencies = plans
        .iter()
        .filter_map(|plan| {
            let member = members.get(&plan.instance_id)?;
            (!member.requested && !advanced_nodes.contains(&plan.instance_id)).then(|| {
                ArtifactDependency {
                    artifact_id: plan.instance_id.clone(),
                    depends_on: member.bundle_root_instance_id.clone(),
                    relationship: "bundle_dependency".into(),
                    frame_numbers: vec![],
                }
            })
        })
        .collect::<Vec<_>>();
    dependencies.extend_from_slice(advanced_dependencies);
    let corpus_plan = CorpusPlan {
        schema_version: CORPUS_PLAN_SCHEMA_VERSION.into(),
        seed,
        artifacts,
        dependencies,
        unavailable: vec![],
        publication: PublicationPlan {
            manifest_path: OutputRelativePath::new("manifest.json")?,
            transaction: PublicationTransaction::AtomicNoReplace,
            private_staging: true,
            no_overwrite: true,
        },
        resources: ResourcePlan {
            max_artifacts: limits.max_instances,
            max_total_output_bytes: limits.max_total_output_bytes,
            max_peak_working_bytes: limits.max_total_output_bytes,
            max_parallelism: parallelism.max(1),
        },
    };
    corpus_plan.validate()?;
    Ok(corpus_plan)
}

pub(crate) fn planned_artifact(
    index: usize,
    plan: &ResolvedInstancePlan,
    members: &BTreeMap<String, BundleMemberProvenance>,
    output_limit: u64,
) -> Result<PlannedArtifact, CorpusPlanError> {
    let member = members
        .get(&plan.instance_id)
        .ok_or_else(|| CorpusPlanError::UnknownArtifact(plan.instance_id.clone()))?;
    let encapsulated = plan.content.iter().find_map(|content| {
        let ContentMaterialization::Encapsulated {
            basic_offset_table, ..
        } = content.materialization.as_ref()?
        else {
            return None;
        };
        Some(!basic_offset_table.is_empty())
    });
    let implementation_class_uid = plan
        .identities
        .get(&CompositionUidRole::ImplementationClass, 0)
        .ok_or_else(|| CorpusPlanError::InvalidIdentifier {
            label: "implementation class UID",
            value: String::new(),
        })?
        .to_owned();
    let provenance = if member.requested {
        ArtifactProvenance::Requested
    } else {
        ArtifactProvenance::Dependency {
            requested_by: vec![member.bundle_root_instance_id.clone()],
        }
    };
    Ok(PlannedArtifact::Dicom(PlannedDicomArtifact {
        logical_id: plan.instance_id.clone(),
        order: u64::try_from(index).map_err(|_| CorpusPlanError::ResourceEstimateOverflow)?,
        provenance,
        case_binding: None,
        instance: plan.clone(),
        output: OutputPlan {
            relative_path: OutputRelativePath::new(format!("instances/{}.dcm", plan.instance_id))?,
            role: "composition_instance".into(),
            publish: true,
        },
        encoding: EncodingPlan {
            transfer_syntax_uid: plan.transfer_syntax_uid.clone(),
            sequence_length: SequenceLengthPolicy::WriterDefault,
            item_length: ItemLengthPolicy::WriterDefault,
            fragmentation: if encapsulated.is_some()
                || plan.transfer_syntax_uid == crate::codecs::RLE_LOSSLESS_TRANSFER_SYNTAX_UID
            {
                FragmentationPolicy::PreserveEncodedFrames
            } else {
                FragmentationPolicy::Native
            },
            offset_table: match encapsulated {
                Some(true) => OffsetTablePolicy::PopulatedBasic,
                Some(false) => OffsetTablePolicy::EmptyBasic,
                None if plan.transfer_syntax_uid
                    == crate::codecs::RLE_LOSSLESS_TRANSFER_SYNTAX_UID =>
                {
                    OffsetTablePolicy::PopulatedBasic
                }
                None => OffsetTablePolicy::NotApplicable,
            },
            preamble: PreamblePolicy::ZeroFilled,
            file_meta: FileMetaPolicy::Standard,
            implementation: ImplementationIdentityPlan {
                class_uid: implementation_class_uid,
                version_name: Some(IMPLEMENTATION_VERSION_NAME.into()),
            },
            backend_id: "composition_part10".into(),
        },
        validation: ValidationPlan {
            rules: vec![ValidationRule {
                rule_id: "composition_resolved_plan".into(),
                requirement: ValidationRequirement::Required,
                parameters: BTreeMap::new(),
            }],
        },
        evidence: EvidencePlan {
            obligations: vec![EvidenceObligation {
                obligation_id: "composition_manifest_validation".into(),
                route_id: "composition_manifest".into(),
                independence: EvidenceIndependence::SameProject,
                required: true,
                parameters: BTreeMap::new(),
            }],
        },
        resources: ArtifactResourceEstimate {
            // The composition contract exposes a run-wide output ceiling, not
            // exact Part 10 sizes. Divide that ceiling deterministically across
            // planned artifacts; the executor enforces both this conservative
            // per-artifact envelope and the aggregate observed total.
            output_bytes: output_limit,
            peak_working_bytes: output_limit,
        },
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::composition::{IdentityPlan, TemplateId, TemplateVersion};

    fn resolved(id: &str) -> ResolvedInstancePlan {
        ResolvedInstancePlan {
            plan_schema_version: "0.1.0".into(),
            instance_id: id.into(),
            template_id: TemplateId("classic/secondary-capture/monochrome".into()),
            template_version: TemplateVersion {
                major: 1,
                minor: 0,
                patch: 0,
            },
            sop_class_uid: "1.2.840.10008.5.1.4.1.1.7".into(),
            transfer_syntax_uid: "1.2.840.10008.1.2.1".into(),
            identities: IdentityPlan::from_exact_values(
                id,
                [
                    (
                        CompositionUidRole::SopInstance,
                        0,
                        format!("2.25.{}", if id == "first" { 11 } else { 12 }),
                    ),
                    (
                        CompositionUidRole::ImplementationClass,
                        0,
                        "2.25.999".into(),
                    ),
                ],
            )
            .unwrap(),
            attributes: vec![],
            content: vec![],
            references: vec![],
        }
    }

    fn member(id: &str, requested: bool, root: &str) -> BundleMemberProvenance {
        BundleMemberProvenance {
            instance_id: id.into(),
            requested,
            bundle_root_instance_id: root.into(),
            bundle_role: if requested { "root" } else { "source" }.into(),
            source: if requested {
                "requested"
            } else {
                "default_template_dependency"
            }
            .into(),
        }
    }

    #[test]
    fn adapter_preserves_explicit_composition_order_and_hashes_deterministically() {
        let plans = vec![resolved("first"), resolved("second")];
        let members = BTreeMap::from([
            ("first".into(), member("first", true, "first")),
            ("second".into(), member("second", true, "second")),
        ]);
        let first = resolved_composition_corpus_plan_with_advanced(
            41,
            &plans,
            &members,
            &ResourceLimits::default(),
            4,
            &BTreeMap::new(),
            &[],
        )
        .unwrap();
        let second = resolved_composition_corpus_plan_with_advanced(
            41,
            &plans,
            &members,
            &ResourceLimits::default(),
            4,
            &BTreeMap::new(),
            &[],
        )
        .unwrap();

        assert_eq!(
            first
                .artifacts
                .iter()
                .map(|artifact| (artifact.order(), artifact.logical_id()))
                .collect::<Vec<_>>(),
            vec![(0, "first"), (1, "second")]
        );
        assert_eq!(first.topological_order().unwrap(), vec!["first", "second"]);
        assert_eq!(
            first.canonical_sha256().unwrap(),
            second.canonical_sha256().unwrap()
        );
    }

    #[test]
    fn bundle_provenance_keeps_root_before_generated_dependency() {
        let plans = vec![resolved("first"), resolved("second")];
        let members = BTreeMap::from([
            ("first".into(), member("first", true, "first")),
            ("second".into(), member("second", false, "first")),
        ]);
        let corpus = resolved_composition_corpus_plan_with_advanced(
            41,
            &plans,
            &members,
            &ResourceLimits::default(),
            2,
            &BTreeMap::new(),
            &[],
        )
        .unwrap();

        assert_eq!(corpus.topological_order().unwrap(), vec!["first", "second"]);
        assert!(matches!(
            corpus.artifacts[1].provenance(),
            ArtifactProvenance::Dependency { requested_by }
                if requested_by == &["first"]
        ));
    }
}
