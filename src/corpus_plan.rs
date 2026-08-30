//! Run-neutral, plan-first contracts for generated corpus artifacts.
//!
//! This module deliberately contains no frontend request types, registry JSON,
//! composition specification types, output-root paths, or file writers. Runtime
//! asset bindings and publication are executor concerns; the structures here
//! are the canonical, auditable description of a run before materialization.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::composition::ResolvedInstancePlan;
use crate::sha256_hex;

pub const CORPUS_PLAN_SCHEMA_VERSION: &str = "0.2.0";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CorpusPlan {
    pub schema_version: String,
    pub seed: u64,
    pub artifacts: Vec<PlannedArtifact>,
    #[serde(default)]
    pub dependencies: Vec<ArtifactDependency>,
    #[serde(default)]
    pub unavailable: Vec<UnavailableCapability>,
    pub publication: PublicationPlan,
    pub resources: ResourcePlan,
}

impl CorpusPlan {
    pub fn validate(&self) -> Result<(), CorpusPlanError> {
        if self.schema_version != CORPUS_PLAN_SCHEMA_VERSION {
            return Err(CorpusPlanError::UnsupportedSchemaVersion(
                self.schema_version.clone(),
            ));
        }
        self.publication.validate()?;
        self.resources.validate()?;

        let mut ids = BTreeSet::new();
        let mut orders = BTreeSet::new();
        let mut output_paths = BTreeSet::new();
        let mut planned_output_bytes = 0_u64;
        let mut peak_working_bytes = 0_u64;
        for artifact in &self.artifacts {
            artifact.validate()?;
            let id = artifact.logical_id();
            if !ids.insert(id.to_owned()) {
                return Err(CorpusPlanError::DuplicateArtifact(id.to_owned()));
            }
            if !orders.insert(artifact.order()) {
                return Err(CorpusPlanError::DuplicateArtifactOrder(artifact.order()));
            }
            if let Some(output) = artifact.output() {
                if !output_paths.insert(output.relative_path.clone()) {
                    return Err(CorpusPlanError::DuplicateOutputPath(
                        output.relative_path.to_string(),
                    ));
                }
            }
            let estimate = artifact.resource_estimate();
            planned_output_bytes = planned_output_bytes
                .checked_add(estimate.output_bytes)
                .ok_or(CorpusPlanError::ResourceEstimateOverflow)?;
            peak_working_bytes = peak_working_bytes.max(estimate.peak_working_bytes);
        }
        if self.artifacts.len() as u64 > self.resources.max_artifacts
            || planned_output_bytes > self.resources.max_total_output_bytes
            || peak_working_bytes > self.resources.max_peak_working_bytes
        {
            return Err(CorpusPlanError::ResourceEstimateExceedsLimit {
                artifacts: self.artifacts.len() as u64,
                output_bytes: planned_output_bytes,
                peak_working_bytes,
            });
        }
        if output_paths.contains(&self.publication.manifest_path) {
            return Err(CorpusPlanError::ManifestPathCollision(
                self.publication.manifest_path.to_string(),
            ));
        }

        let mut edges = BTreeSet::new();
        for dependency in &self.dependencies {
            dependency.validate()?;
            if !ids.contains(&dependency.artifact_id) {
                return Err(CorpusPlanError::UnknownArtifact(
                    dependency.artifact_id.clone(),
                ));
            }
            if !ids.contains(&dependency.depends_on) {
                return Err(CorpusPlanError::UnknownDependency {
                    artifact_id: dependency.artifact_id.clone(),
                    depends_on: dependency.depends_on.clone(),
                });
            }
            if dependency.artifact_id == dependency.depends_on {
                return Err(CorpusPlanError::SelfDependency(
                    dependency.artifact_id.clone(),
                ));
            }
            let identity = (
                dependency.artifact_id.clone(),
                dependency.depends_on.clone(),
                dependency.relationship.clone(),
            );
            if !edges.insert(identity) {
                return Err(CorpusPlanError::DuplicateDependency {
                    artifact_id: dependency.artifact_id.clone(),
                    depends_on: dependency.depends_on.clone(),
                    relationship: dependency.relationship.clone(),
                });
            }
        }

        let dependency_pairs = self
            .dependencies
            .iter()
            .map(|dependency| (&dependency.artifact_id, &dependency.depends_on))
            .collect::<BTreeSet<_>>();
        for artifact in &self.artifacts {
            let (references, private_source) = match artifact.provenance() {
                ArtifactProvenance::Requested => (&[][..], false),
                ArtifactProvenance::Dependency { requested_by } => (requested_by.as_slice(), false),
                ArtifactProvenance::PrivateSource { consumed_by } => (consumed_by.as_slice(), true),
            };
            if private_source && artifact.output().is_some_and(|output| output.publish) {
                return Err(CorpusPlanError::PrivateSourcePublished(
                    artifact.logical_id().to_owned(),
                ));
            }
            for reference in references {
                if !ids.contains(reference) {
                    return Err(CorpusPlanError::UnknownProvenance {
                        artifact_id: artifact.logical_id().to_owned(),
                        referenced_id: reference.clone(),
                    });
                }
                let forward =
                    dependency_pairs.contains(&(&artifact.logical_id().to_owned(), reference));
                let reverse =
                    dependency_pairs.contains(&(reference, &artifact.logical_id().to_owned()));
                if (private_source && !reverse) || (!private_source && !forward && !reverse) {
                    return Err(CorpusPlanError::ProvenanceDependencyMismatch {
                        artifact_id: artifact.logical_id().to_owned(),
                        referenced_id: reference.clone(),
                    });
                }
            }
            if let PlannedArtifact::Mutation(mutation) = artifact {
                if !ids.contains(&mutation.source_artifact_id) {
                    return Err(CorpusPlanError::UnknownDependency {
                        artifact_id: mutation.logical_id.clone(),
                        depends_on: mutation.source_artifact_id.clone(),
                    });
                }
                if !dependency_pairs.contains(&(&mutation.logical_id, &mutation.source_artifact_id))
                {
                    return Err(CorpusPlanError::MissingMutationDependency {
                        artifact_id: mutation.logical_id.clone(),
                        source_artifact_id: mutation.source_artifact_id.clone(),
                    });
                }
            }
            if let PlannedArtifact::ImportedDicom(imported) = artifact {
                for source_id in imported.provider.source_assets.values() {
                    if !ids.contains(source_id) {
                        return Err(CorpusPlanError::UnknownDependency {
                            artifact_id: imported.logical_id.clone(),
                            depends_on: source_id.clone(),
                        });
                    }
                    if !dependency_pairs.contains(&(&imported.logical_id, source_id)) {
                        return Err(CorpusPlanError::MissingImportedDicomDependency {
                            artifact_id: imported.logical_id.clone(),
                            source_artifact_id: source_id.clone(),
                        });
                    }
                }
            }
        }

        for capability in &self.unavailable {
            capability.validate()?;
            for artifact_id in &capability.affected_artifact_ids {
                if ids.contains(artifact_id) {
                    return Err(CorpusPlanError::AvailableArtifactMarkedUnavailable {
                        capability_id: capability.capability_id.clone(),
                        artifact_id: artifact_id.clone(),
                    });
                }
            }
        }

        self.topological_order().map(|_| ())
    }

    /// Return a deterministic dependency-first artifact order.
    ///
    /// When several nodes are ready, explicit artifact order and then logical
    /// ID provide the tie-breaker, so worker count and input vector order cannot
    /// affect the result.
    pub fn topological_order(&self) -> Result<Vec<String>, CorpusPlanError> {
        let ids = self
            .artifacts
            .iter()
            .map(|artifact| artifact.logical_id().to_owned())
            .collect::<BTreeSet<_>>();
        let artifact_orders = self
            .artifacts
            .iter()
            .map(|artifact| (artifact.logical_id().to_owned(), artifact.order()))
            .collect::<BTreeMap<_, _>>();
        let mut indegree = ids
            .iter()
            .map(|id| (id.clone(), 0_usize))
            .collect::<BTreeMap<_, _>>();
        let mut dependents = ids
            .iter()
            .map(|id| (id.clone(), BTreeSet::new()))
            .collect::<BTreeMap<_, _>>();
        let mut dependency_pairs = BTreeSet::new();

        for dependency in &self.dependencies {
            let Some(degree) = indegree.get_mut(&dependency.artifact_id) else {
                return Err(CorpusPlanError::UnknownArtifact(
                    dependency.artifact_id.clone(),
                ));
            };
            if !ids.contains(&dependency.depends_on) {
                return Err(CorpusPlanError::UnknownDependency {
                    artifact_id: dependency.artifact_id.clone(),
                    depends_on: dependency.depends_on.clone(),
                });
            }
            if !dependency_pairs.insert((
                dependency.artifact_id.clone(),
                dependency.depends_on.clone(),
            )) {
                continue;
            }
            *degree = degree
                .checked_add(1)
                .ok_or(CorpusPlanError::DependencyCountOverflow)?;
            dependents
                .get_mut(&dependency.depends_on)
                .expect("all dependency nodes were indexed")
                .insert(dependency.artifact_id.clone());
        }

        let mut ready = indegree
            .iter()
            .filter(|(_, degree)| **degree == 0)
            .map(|(id, _)| {
                (
                    *artifact_orders.get(id).expect("all artifacts have order"),
                    id.clone(),
                )
            })
            .collect::<BTreeSet<_>>();
        let mut ordered = Vec::with_capacity(ids.len());
        while let Some((_, next)) = ready.pop_first() {
            ordered.push(next.clone());
            for dependent in dependents
                .get(&next)
                .expect("all dependency nodes were indexed")
            {
                let degree = indegree
                    .get_mut(dependent)
                    .expect("all dependent nodes were indexed");
                *degree -= 1;
                if *degree == 0 {
                    ready.insert((
                        *artifact_orders
                            .get(dependent)
                            .expect("all artifacts have order"),
                        dependent.clone(),
                    ));
                }
            }
        }
        if ordered.len() != ids.len() {
            let cycle = indegree
                .into_iter()
                .filter(|(_, degree)| *degree != 0)
                .map(|(id, _)| id)
                .collect();
            return Err(CorpusPlanError::DependencyCycle(cycle));
        }
        Ok(ordered)
    }

    /// Serialize the normalized plan used for audit identity.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, CorpusPlanError> {
        self.validate()?;
        let order = self.topological_order()?;
        let by_id = self
            .artifacts
            .iter()
            .map(|artifact| (artifact.logical_id(), artifact))
            .collect::<BTreeMap<_, _>>();
        let artifacts = order
            .iter()
            .map(|id| (*by_id.get(id.as_str()).expect("validated artifact ID")).clone())
            .collect();
        let mut dependencies = self.dependencies.clone();
        dependencies.sort_by(|left, right| {
            (
                &left.artifact_id,
                &left.depends_on,
                &left.relationship,
                &left.frame_numbers,
            )
                .cmp(&(
                    &right.artifact_id,
                    &right.depends_on,
                    &right.relationship,
                    &right.frame_numbers,
                ))
        });
        let mut unavailable = self.unavailable.clone();
        unavailable.sort_by(|left, right| left.capability_id.cmp(&right.capability_id));
        for capability in &mut unavailable {
            capability.affected_artifact_ids.sort();
            capability.affected_artifact_ids.dedup();
        }
        let canonical = Self {
            schema_version: self.schema_version.clone(),
            seed: self.seed,
            artifacts,
            dependencies,
            unavailable,
            publication: self.publication.clone(),
            resources: self.resources.clone(),
        };
        serde_json::to_vec(&canonical).map_err(CorpusPlanError::Serialize)
    }

    pub fn canonical_sha256(&self) -> Result<String, CorpusPlanError> {
        self.canonical_bytes().map(|bytes| sha256_hex(&bytes))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum PlannedArtifact {
    Dicom(PlannedDicomArtifact),
    ImportedDicom(PlannedImportedDicomArtifact),
    Mutation(PlannedMutationArtifact),
    Qualification(PlannedQualification),
    Auxiliary(PlannedAuxiliaryArtifact),
}

impl PlannedArtifact {
    pub fn logical_id(&self) -> &str {
        match self {
            Self::Dicom(value) => &value.logical_id,
            Self::ImportedDicom(value) => &value.logical_id,
            Self::Mutation(value) => &value.logical_id,
            Self::Qualification(value) => &value.logical_id,
            Self::Auxiliary(value) => &value.logical_id,
        }
    }

    pub fn provenance(&self) -> &ArtifactProvenance {
        match self {
            Self::Dicom(value) => &value.provenance,
            Self::ImportedDicom(value) => &value.provenance,
            Self::Mutation(value) => &value.provenance,
            Self::Qualification(value) => &value.provenance,
            Self::Auxiliary(value) => &value.provenance,
        }
    }

    pub fn order(&self) -> u64 {
        match self {
            Self::Dicom(value) => value.order,
            Self::ImportedDicom(value) => value.order,
            Self::Mutation(value) => value.order,
            Self::Qualification(value) => value.order,
            Self::Auxiliary(value) => value.order,
        }
    }

    pub fn output(&self) -> Option<&OutputPlan> {
        match self {
            Self::Dicom(value) => Some(&value.output),
            Self::ImportedDicom(value) => Some(&value.output),
            Self::Mutation(value) => Some(&value.output),
            Self::Qualification(_) => None,
            Self::Auxiliary(value) => Some(&value.output),
        }
    }

    pub fn resource_estimate(&self) -> &ArtifactResourceEstimate {
        match self {
            Self::Dicom(value) => &value.resources,
            Self::ImportedDicom(value) => &value.resources,
            Self::Mutation(value) => &value.resources,
            Self::Qualification(value) => &value.resources,
            Self::Auxiliary(value) => &value.resources,
        }
    }

    fn validate(&self) -> Result<(), CorpusPlanError> {
        validate_identifier("artifact logical_id", self.logical_id())?;
        self.provenance().validate()?;
        match self {
            Self::Dicom(value) => value.validate(),
            Self::ImportedDicom(value) => value.validate(),
            Self::Mutation(value) => value.validate(),
            Self::Qualification(value) => value.validate(),
            Self::Auxiliary(value) => value.validate(),
        }
    }
}

/// A full Part 10 object produced by a pinned external provider.
///
/// Unlike native DICOM artifacts, this contract deliberately does not carry
/// dataset construction instructions. The provider output is an opaque Part 10
/// payload until execution, where its declared identity is verified before it
/// becomes a publication candidate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlannedImportedDicomArtifact {
    pub logical_id: String,
    pub order: u64,
    pub provenance: ArtifactProvenance,
    pub case_binding: CaseBinding,
    pub provider: ImportedDicomProviderPlan,
    pub declared_instance: ResolvedInstancePlan,
    pub output: OutputPlan,
    pub validation: ValidationPlan,
    pub evidence: EvidencePlan,
    pub resources: ArtifactResourceEstimate,
}

impl PlannedImportedDicomArtifact {
    fn validate(&self) -> Result<(), CorpusPlanError> {
        self.case_binding.validate()?;
        self.provider.validate(&self.logical_id)?;
        if self.declared_instance.instance_id != self.logical_id {
            return Err(CorpusPlanError::InstanceIdentityMismatch {
                logical_id: self.logical_id.clone(),
                instance_id: self.declared_instance.instance_id.clone(),
            });
        }
        if self.declared_instance.transfer_syntax_uid != self.provider.transfer_syntax_uid {
            return Err(CorpusPlanError::TransferSyntaxMismatch {
                logical_id: self.logical_id.clone(),
                instance_uid: self.declared_instance.transfer_syntax_uid.clone(),
                encoding_uid: self.provider.transfer_syntax_uid.clone(),
            });
        }
        self.output.validate()?;
        self.validation.validate()?;
        self.evidence.validate()?;
        self.resources.validate()?;
        if self.provider.maximum_size_bytes > self.resources.output_bytes
            || self.provider.maximum_size_bytes > self.resources.peak_working_bytes
        {
            return Err(CorpusPlanError::ImportedDicomResourceMismatch {
                logical_id: self.logical_id.clone(),
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImportedDicomProviderPlan {
    pub request_id: String,
    pub provider_id: String,
    pub required_version: String,
    pub output_slot: String,
    pub media_type: String,
    pub maximum_size_bytes: u64,
    pub expected_sha256: Option<String>,
    pub transfer_syntax_uid: String,
    #[serde(default)]
    pub parameters: BTreeMap<String, Value>,
    /// Provider input role to logical dependency/source artifact ID.
    #[serde(default)]
    pub source_assets: BTreeMap<String, String>,
}

impl ImportedDicomProviderPlan {
    fn validate(&self, artifact_id: &str) -> Result<(), CorpusPlanError> {
        validate_identifier("import request ID", &self.request_id)?;
        validate_identifier("import provider ID", &self.provider_id)?;
        validate_identifier("import provider version", &self.required_version)?;
        validate_identifier("import output slot", &self.output_slot)?;
        validate_identifier("import media type", &self.media_type)?;
        validate_uid("import transfer syntax UID", &self.transfer_syntax_uid)?;
        if self.media_type != "application/dicom" || self.maximum_size_bytes == 0 {
            return Err(CorpusPlanError::InvalidImportedDicomProvider(
                artifact_id.to_owned(),
            ));
        }
        if let Some(digest) = &self.expected_sha256 {
            validate_sha256("import expected SHA-256", digest)?;
        }
        for (role, source) in &self.source_assets {
            validate_identifier("import source role", role)?;
            validate_identifier("import source artifact ID", source)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ArtifactProvenance {
    Requested,
    Dependency { requested_by: Vec<String> },
    PrivateSource { consumed_by: Vec<String> },
}

impl ArtifactProvenance {
    fn validate(&self) -> Result<(), CorpusPlanError> {
        let values = match self {
            Self::Requested => return Ok(()),
            Self::Dependency { requested_by } => requested_by,
            Self::PrivateSource { consumed_by } => consumed_by,
        };
        if values.is_empty() {
            return Err(CorpusPlanError::EmptyProvenance);
        }
        let mut unique = BTreeSet::new();
        for value in values {
            validate_identifier("provenance artifact ID", value)?;
            if !unique.insert(value) {
                return Err(CorpusPlanError::DuplicateProvenance(value.clone()));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlannedDicomArtifact {
    pub logical_id: String,
    pub order: u64,
    pub provenance: ArtifactProvenance,
    pub case_binding: Option<CaseBinding>,
    pub instance: ResolvedInstancePlan,
    pub output: OutputPlan,
    pub encoding: EncodingPlan,
    pub validation: ValidationPlan,
    pub evidence: EvidencePlan,
    pub resources: ArtifactResourceEstimate,
}

impl PlannedDicomArtifact {
    fn validate(&self) -> Result<(), CorpusPlanError> {
        if let Some(binding) = &self.case_binding {
            binding.validate()?;
        }
        if self.instance.instance_id != self.logical_id {
            return Err(CorpusPlanError::InstanceIdentityMismatch {
                logical_id: self.logical_id.clone(),
                instance_id: self.instance.instance_id.clone(),
            });
        }
        if self.instance.transfer_syntax_uid != self.encoding.transfer_syntax_uid {
            return Err(CorpusPlanError::TransferSyntaxMismatch {
                logical_id: self.logical_id.clone(),
                instance_uid: self.instance.transfer_syntax_uid.clone(),
                encoding_uid: self.encoding.transfer_syntax_uid.clone(),
            });
        }
        let instance_implementation_uid = self
            .instance
            .identities
            .get(
                &crate::composition::CompositionUidRole::ImplementationClass,
                0,
            )
            .ok_or_else(|| CorpusPlanError::MissingImplementationIdentity {
                logical_id: self.logical_id.clone(),
            })?;
        if instance_implementation_uid != self.encoding.implementation.class_uid {
            return Err(CorpusPlanError::ImplementationIdentityMismatch {
                logical_id: self.logical_id.clone(),
                instance_uid: instance_implementation_uid.to_owned(),
                encoding_uid: self.encoding.implementation.class_uid.clone(),
            });
        }
        self.output.validate()?;
        self.encoding.validate()?;
        self.validation.validate()?;
        self.evidence.validate()?;
        self.resources.validate()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlannedMutationArtifact {
    pub logical_id: String,
    pub order: u64,
    pub provenance: ArtifactProvenance,
    pub source_artifact_id: String,
    pub mutation: MutationPlan,
    pub output: OutputPlan,
    pub validation: ValidationPlan,
    pub evidence: EvidencePlan,
    pub resources: ArtifactResourceEstimate,
}

impl PlannedMutationArtifact {
    fn validate(&self) -> Result<(), CorpusPlanError> {
        validate_identifier("mutation source_artifact_id", &self.source_artifact_id)?;
        self.mutation.validate()?;
        self.output.validate()?;
        self.validation.validate()?;
        self.evidence.validate()?;
        self.resources.validate()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlannedQualification {
    pub logical_id: String,
    pub order: u64,
    pub provenance: ArtifactProvenance,
    pub qualification_kind: String,
    pub parameters: BTreeMap<String, Value>,
    pub payload_policy: QualificationPayloadPolicy,
    pub validation: ValidationPlan,
    pub evidence: EvidencePlan,
    pub resources: ArtifactResourceEstimate,
}

impl PlannedQualification {
    fn validate(&self) -> Result<(), CorpusPlanError> {
        validate_identifier("qualification_kind", &self.qualification_kind)?;
        self.validation.validate()?;
        self.evidence.validate()?;
        self.resources.validate()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QualificationPayloadPolicy {
    NoPayload,
    EvidenceOnly,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlannedAuxiliaryArtifact {
    pub logical_id: String,
    pub order: u64,
    pub provenance: ArtifactProvenance,
    pub auxiliary_kind: String,
    pub output: OutputPlan,
    pub parameters: BTreeMap<String, Value>,
    pub validation: ValidationPlan,
    pub evidence: EvidencePlan,
    pub resources: ArtifactResourceEstimate,
}

impl PlannedAuxiliaryArtifact {
    fn validate(&self) -> Result<(), CorpusPlanError> {
        validate_identifier("auxiliary_kind", &self.auxiliary_kind)?;
        self.output.validate()?;
        self.validation.validate()?;
        self.evidence.validate()?;
        self.resources.validate()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaseBinding {
    pub case_id: String,
    pub recipe_id: String,
    pub recipe_version: String,
}

impl CaseBinding {
    fn validate(&self) -> Result<(), CorpusPlanError> {
        validate_identifier("case_id", &self.case_id)?;
        validate_identifier("recipe_id", &self.recipe_id)?;
        validate_identifier("recipe_version", &self.recipe_version)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactDependency {
    /// Artifact that cannot execute until `depends_on` has completed.
    pub artifact_id: String,
    pub depends_on: String,
    pub relationship: String,
    #[serde(default)]
    pub frame_numbers: Vec<u32>,
}

impl ArtifactDependency {
    fn validate(&self) -> Result<(), CorpusPlanError> {
        validate_identifier("dependency artifact_id", &self.artifact_id)?;
        validate_identifier("dependency depends_on", &self.depends_on)?;
        validate_identifier("dependency relationship", &self.relationship)?;
        let mut frames = BTreeSet::new();
        for frame in &self.frame_numbers {
            if *frame == 0 {
                return Err(CorpusPlanError::ZeroFrameNumber {
                    artifact_id: self.artifact_id.clone(),
                    depends_on: self.depends_on.clone(),
                });
            }
            if !frames.insert(frame) {
                return Err(CorpusPlanError::DuplicateFrameNumber {
                    artifact_id: self.artifact_id.clone(),
                    depends_on: self.depends_on.clone(),
                    frame: *frame,
                });
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct OutputRelativePath(String);

impl OutputRelativePath {
    pub fn new(value: impl Into<String>) -> Result<Self, CorpusPlanError> {
        let value = value.into();
        validate_output_path(&value)?;
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for OutputRelativePath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OutputPlan {
    pub relative_path: OutputRelativePath,
    pub role: String,
    pub publish: bool,
}

impl OutputPlan {
    fn validate(&self) -> Result<(), CorpusPlanError> {
        validate_output_path(self.relative_path.as_str())?;
        validate_identifier("output role", &self.role)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EncodingPlan {
    pub transfer_syntax_uid: String,
    pub sequence_length: SequenceLengthPolicy,
    pub item_length: ItemLengthPolicy,
    pub fragmentation: FragmentationPolicy,
    pub offset_table: OffsetTablePolicy,
    pub preamble: PreamblePolicy,
    pub file_meta: FileMetaPolicy,
    pub implementation: ImplementationIdentityPlan,
    pub backend_id: String,
}

impl EncodingPlan {
    pub fn validate(&self) -> Result<(), CorpusPlanError> {
        validate_uid("transfer_syntax_uid", &self.transfer_syntax_uid)?;
        validate_identifier("encoding backend_id", &self.backend_id)?;
        self.implementation.validate()?;

        let native = matches!(self.fragmentation, FragmentationPolicy::Native);
        let no_offset_table = self.offset_table == OffsetTablePolicy::NotApplicable;
        if native != no_offset_table {
            return Err(CorpusPlanError::InvalidEncodingCombination(
                "native fragmentation and a not-applicable offset table must be selected together",
            ));
        }
        if matches!(
            self.fragmentation,
            FragmentationPolicy::FixedMaximumBytes { maximum_bytes: 0 }
                | FragmentationPolicy::FixedFragmentsPerFrame {
                    fragments_per_frame: 0
                }
        ) {
            return Err(CorpusPlanError::ZeroFragmentSizeLimit);
        }
        if self.offset_table == OffsetTablePolicy::Extended
            && !matches!(
                self.fragmentation,
                FragmentationPolicy::OneFragmentPerFrame
                    | FragmentationPolicy::FixedFragmentsPerFrame { .. }
            )
        {
            return Err(CorpusPlanError::InvalidEncodingCombination(
                "extended offset tables require a deterministic per-frame fragmentation policy",
            ));
        }

        const IMPLICIT_VR_LE: &str = "1.2.840.10008.1.2";
        const EXPLICIT_VR_LE: &str = "1.2.840.10008.1.2.1";
        const EXPLICIT_VR_BE: &str = "1.2.840.10008.1.2.2";
        const RLE_LOSSLESS: &str = "1.2.840.10008.1.2.5";
        if self.transfer_syntax_uid == RLE_LOSSLESS && native {
            return Err(CorpusPlanError::InvalidEncodingCombination(
                "RLE Lossless requires encapsulated fragmentation",
            ));
        }
        if matches!(
            self.transfer_syntax_uid.as_str(),
            IMPLICIT_VR_LE | EXPLICIT_VR_LE | EXPLICIT_VR_BE
        ) && !native
        {
            return Err(CorpusPlanError::InvalidEncodingCombination(
                "native transfer syntaxes cannot use encapsulated fragmentation",
            ));
        }
        match self.backend_id.as_str() {
            "encoding.native.rle_lossless" if self.transfer_syntax_uid != RLE_LOSSLESS => {
                return Err(CorpusPlanError::BackendTransferSyntaxMismatch {
                    backend_id: self.backend_id.clone(),
                    transfer_syntax_uid: self.transfer_syntax_uid.clone(),
                });
            }
            "encoding.native.explicit_vr_big_endian"
                if self.transfer_syntax_uid != EXPLICIT_VR_BE =>
            {
                return Err(CorpusPlanError::BackendTransferSyntaxMismatch {
                    backend_id: self.backend_id.clone(),
                    transfer_syntax_uid: self.transfer_syntax_uid.clone(),
                });
            }
            _ => {}
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SequenceLengthPolicy {
    WriterDefault,
    Defined,
    Undefined,
    PreserveDeclared,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemLengthPolicy {
    WriterDefault,
    Defined,
    Undefined,
    PreserveDeclared,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum FragmentationPolicy {
    Native,
    OneFragmentPerFrame,
    FixedMaximumBytes { maximum_bytes: u64 },
    FixedFragmentsPerFrame { fragments_per_frame: u32 },
    PreserveEncodedFrames,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OffsetTablePolicy {
    NotApplicable,
    EmptyBasic,
    PopulatedBasic,
    Extended,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PreamblePolicy {
    ZeroFilled,
    DeterministicNonZero,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileMetaPolicy {
    Standard,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImplementationIdentityPlan {
    pub class_uid: String,
    pub version_name: Option<String>,
}

impl ImplementationIdentityPlan {
    fn validate(&self) -> Result<(), CorpusPlanError> {
        validate_uid("implementation class UID", &self.class_uid)?;
        if self
            .version_name
            .as_deref()
            .is_some_and(|value| value.is_empty() || value.len() > 16 || !value.is_ascii())
        {
            return Err(CorpusPlanError::InvalidImplementationVersion);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValidationPlan {
    pub rules: Vec<ValidationRule>,
}

impl ValidationPlan {
    fn validate(&self) -> Result<(), CorpusPlanError> {
        if self.rules.is_empty() {
            return Err(CorpusPlanError::EmptyValidationPlan);
        }
        let mut ids = BTreeSet::new();
        for rule in &self.rules {
            validate_identifier("validation rule_id", &rule.rule_id)?;
            if !ids.insert(&rule.rule_id) {
                return Err(CorpusPlanError::DuplicateValidationRule(
                    rule.rule_id.clone(),
                ));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValidationRule {
    pub rule_id: String,
    pub requirement: ValidationRequirement,
    #[serde(default)]
    pub parameters: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidationRequirement {
    Required,
    CapabilityConditional,
    IndependentRequired,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidencePlan {
    pub obligations: Vec<EvidenceObligation>,
}

impl EvidencePlan {
    fn validate(&self) -> Result<(), CorpusPlanError> {
        let mut ids = BTreeSet::new();
        for obligation in &self.obligations {
            validate_identifier("evidence obligation_id", &obligation.obligation_id)?;
            validate_identifier("evidence route_id", &obligation.route_id)?;
            if !ids.insert(&obligation.obligation_id) {
                return Err(CorpusPlanError::DuplicateEvidenceObligation(
                    obligation.obligation_id.clone(),
                ));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceObligation {
    pub obligation_id: String,
    pub route_id: String,
    pub independence: EvidenceIndependence,
    pub required: bool,
    #[serde(default)]
    pub parameters: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceIndependence {
    SameProject,
    IndependentTool,
    ExternalProvider,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublicationPlan {
    pub manifest_path: OutputRelativePath,
    pub transaction: PublicationTransaction,
    pub private_staging: bool,
    pub no_overwrite: bool,
}

impl PublicationPlan {
    fn validate(&self) -> Result<(), CorpusPlanError> {
        validate_output_path(self.manifest_path.as_str())?;
        if !self.private_staging || !self.no_overwrite {
            return Err(CorpusPlanError::UnsafePublicationPolicy);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PublicationTransaction {
    AtomicNoReplace,
    PlatformNoReplace,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResourcePlan {
    pub max_artifacts: u64,
    pub max_total_output_bytes: u64,
    pub max_peak_working_bytes: u64,
    pub max_parallelism: u32,
}

impl ResourcePlan {
    fn validate(&self) -> Result<(), CorpusPlanError> {
        if self.max_artifacts == 0
            || self.max_total_output_bytes == 0
            || self.max_peak_working_bytes == 0
            || self.max_parallelism == 0
        {
            return Err(CorpusPlanError::ZeroResourceLimit);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactResourceEstimate {
    pub output_bytes: u64,
    pub peak_working_bytes: u64,
}

impl ArtifactResourceEstimate {
    fn validate(&self) -> Result<(), CorpusPlanError> {
        if self.peak_working_bytes == 0 {
            return Err(CorpusPlanError::ZeroArtifactWorkingSet);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MutationPlan {
    pub contract_version: String,
    pub operations: Vec<PlannedMutationOperation>,
    pub expected_source_sha256: String,
    pub expected_output_sha256: String,
    pub expected_failure_layers: Vec<String>,
    pub acceptable_outcomes: Vec<String>,
}

impl MutationPlan {
    fn validate(&self) -> Result<(), CorpusPlanError> {
        validate_identifier("mutation contract_version", &self.contract_version)?;
        validate_sha256("mutation source SHA-256", &self.expected_source_sha256)?;
        validate_sha256("mutation output SHA-256", &self.expected_output_sha256)?;
        if self.operations.is_empty()
            || self.expected_failure_layers.is_empty()
            || self.acceptable_outcomes.is_empty()
        {
            return Err(CorpusPlanError::IncompleteMutationPlan);
        }
        for operation in &self.operations {
            validate_identifier("mutation operation_id", &operation.operation_id)?;
            for range in &operation.source_ranges {
                if range.start >= range.end {
                    return Err(CorpusPlanError::InvalidByteRange {
                        start: range.start,
                        end: range.end,
                    });
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlannedMutationOperation {
    pub operation_id: String,
    pub source_ranges: Vec<PlannedByteRange>,
    #[serde(default)]
    pub parameters: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlannedByteRange {
    pub start: u64,
    pub end: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityKind {
    Feature,
    Codec,
    Provider,
    ExternalBackend,
    Validator,
    ResourceScale,
    Platform,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UnavailableCapability {
    pub capability_id: String,
    pub kind: CapabilityKind,
    pub reason_code: String,
    pub message: String,
    #[serde(default)]
    pub affected_artifact_ids: Vec<String>,
    #[serde(default)]
    pub requirements: BTreeMap<String, Vec<String>>,
}

impl UnavailableCapability {
    fn validate(&self) -> Result<(), CorpusPlanError> {
        validate_identifier("capability_id", &self.capability_id)?;
        validate_identifier("unavailable reason_code", &self.reason_code)?;
        if self.message.trim().is_empty() {
            return Err(CorpusPlanError::EmptyUnavailableMessage(
                self.capability_id.clone(),
            ));
        }
        for artifact_id in &self.affected_artifact_ids {
            validate_identifier("unavailable artifact ID", artifact_id)?;
        }
        Ok(())
    }
}

fn validate_output_path(value: &str) -> Result<(), CorpusPlanError> {
    if value.is_empty()
        || value.starts_with('/')
        || value.ends_with('/')
        || value.contains('\\')
        || value.contains('\0')
        || value.chars().any(char::is_control)
        || value
            .split('/')
            .any(|part| part.is_empty() || matches!(part, "." | ".."))
    {
        return Err(CorpusPlanError::UnsafeOutputPath(value.to_owned()));
    }
    let first = value.split('/').next().expect("non-empty path");
    if first.len() == 2 && first.as_bytes()[1] == b':' && first.as_bytes()[0].is_ascii_alphabetic()
    {
        return Err(CorpusPlanError::UnsafeOutputPath(value.to_owned()));
    }
    Ok(())
}

fn validate_identifier(label: &'static str, value: &str) -> Result<(), CorpusPlanError> {
    if value.is_empty()
        || value.len() > 256
        || value.chars().any(char::is_control)
        || value.contains('\0')
    {
        return Err(CorpusPlanError::InvalidIdentifier {
            label,
            value: value.to_owned(),
        });
    }
    Ok(())
}

fn validate_uid(label: &'static str, value: &str) -> Result<(), CorpusPlanError> {
    if value.is_empty()
        || value.len() > 64
        || value.starts_with('.')
        || value.ends_with('.')
        || value.split('.').any(|part| part.is_empty())
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || byte == b'.')
    {
        return Err(CorpusPlanError::InvalidIdentifier {
            label,
            value: value.to_owned(),
        });
    }
    Ok(())
}

fn validate_sha256(label: &'static str, value: &str) -> Result<(), CorpusPlanError> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(CorpusPlanError::InvalidIdentifier {
            label,
            value: value.to_owned(),
        });
    }
    Ok(())
}

#[derive(Debug)]
pub enum CorpusPlanError {
    UnsupportedSchemaVersion(String),
    Serialize(serde_json::Error),
    DuplicateArtifact(String),
    DuplicateArtifactOrder(u64),
    DuplicateOutputPath(String),
    ManifestPathCollision(String),
    UnknownArtifact(String),
    UnknownDependency {
        artifact_id: String,
        depends_on: String,
    },
    SelfDependency(String),
    DuplicateDependency {
        artifact_id: String,
        depends_on: String,
        relationship: String,
    },
    DependencyCycle(Vec<String>),
    DependencyCountOverflow,
    MissingMutationDependency {
        artifact_id: String,
        source_artifact_id: String,
    },
    MissingImportedDicomDependency {
        artifact_id: String,
        source_artifact_id: String,
    },
    InvalidImportedDicomProvider(String),
    ImportedDicomResourceMismatch {
        logical_id: String,
    },
    UnknownProvenance {
        artifact_id: String,
        referenced_id: String,
    },
    ProvenanceDependencyMismatch {
        artifact_id: String,
        referenced_id: String,
    },
    PrivateSourcePublished(String),
    InvalidIdentifier {
        label: &'static str,
        value: String,
    },
    EmptyProvenance,
    DuplicateProvenance(String),
    UnsafeOutputPath(String),
    InstanceIdentityMismatch {
        logical_id: String,
        instance_id: String,
    },
    TransferSyntaxMismatch {
        logical_id: String,
        instance_uid: String,
        encoding_uid: String,
    },
    MissingImplementationIdentity {
        logical_id: String,
    },
    ImplementationIdentityMismatch {
        logical_id: String,
        instance_uid: String,
        encoding_uid: String,
    },
    InvalidEncodingCombination(&'static str),
    ZeroFragmentSizeLimit,
    BackendTransferSyntaxMismatch {
        backend_id: String,
        transfer_syntax_uid: String,
    },
    InvalidImplementationVersion,
    EmptyValidationPlan,
    DuplicateValidationRule(String),
    DuplicateEvidenceObligation(String),
    UnsafePublicationPolicy,
    ZeroResourceLimit,
    ResourceEstimateOverflow,
    ResourceEstimateExceedsLimit {
        artifacts: u64,
        output_bytes: u64,
        peak_working_bytes: u64,
    },
    ZeroArtifactWorkingSet,
    ZeroFrameNumber {
        artifact_id: String,
        depends_on: String,
    },
    DuplicateFrameNumber {
        artifact_id: String,
        depends_on: String,
        frame: u32,
    },
    IncompleteMutationPlan,
    InvalidByteRange {
        start: u64,
        end: u64,
    },
    EmptyUnavailableMessage(String),
    AvailableArtifactMarkedUnavailable {
        capability_id: String,
        artifact_id: String,
    },
}

impl fmt::Display for CorpusPlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedSchemaVersion(version) => {
                write!(
                    formatter,
                    "unsupported corpus plan schema version {version}"
                )
            }
            Self::Serialize(error) => write!(formatter, "serialize canonical corpus plan: {error}"),
            Self::DuplicateArtifact(id) => write!(formatter, "duplicate artifact {id}"),
            Self::DuplicateArtifactOrder(order) => {
                write!(formatter, "duplicate artifact order {order}")
            }
            Self::DuplicateOutputPath(path) => write!(formatter, "duplicate output path {path}"),
            Self::ManifestPathCollision(path) => {
                write!(
                    formatter,
                    "artifact output collides with manifest path {path}"
                )
            }
            Self::UnknownArtifact(id) => write!(formatter, "unknown artifact {id}"),
            Self::UnknownDependency {
                artifact_id,
                depends_on,
            } => write!(
                formatter,
                "artifact {artifact_id} depends on unknown artifact {depends_on}"
            ),
            Self::SelfDependency(id) => write!(formatter, "artifact {id} depends on itself"),
            Self::DuplicateDependency {
                artifact_id,
                depends_on,
                relationship,
            } => write!(
                formatter,
                "duplicate {relationship} dependency from {artifact_id} to {depends_on}"
            ),
            Self::DependencyCycle(ids) => {
                write!(
                    formatter,
                    "artifact dependency cycle includes {}",
                    ids.join(", ")
                )
            }
            Self::DependencyCountOverflow => {
                formatter.write_str("artifact dependency count overflow")
            }
            Self::MissingMutationDependency {
                artifact_id,
                source_artifact_id,
            } => write!(
                formatter,
                "mutation artifact {artifact_id} lacks an explicit dependency on source {source_artifact_id}"
            ),
            Self::MissingImportedDicomDependency {
                artifact_id,
                source_artifact_id,
            } => write!(
                formatter,
                "imported DICOM artifact {artifact_id} lacks an explicit dependency on source {source_artifact_id}"
            ),
            Self::InvalidImportedDicomProvider(id) => {
                write!(
                    formatter,
                    "invalid imported DICOM provider contract for {id}"
                )
            }
            Self::ImportedDicomResourceMismatch { logical_id } => write!(
                formatter,
                "imported DICOM provider maximum exceeds resource estimate for {logical_id}"
            ),
            Self::UnknownProvenance {
                artifact_id,
                referenced_id,
            } => write!(
                formatter,
                "artifact {artifact_id} provenance names unknown artifact {referenced_id}"
            ),
            Self::ProvenanceDependencyMismatch {
                artifact_id,
                referenced_id,
            } => write!(
                formatter,
                "artifact {artifact_id} provenance is not represented by a dependency involving {referenced_id}"
            ),
            Self::PrivateSourcePublished(id) => {
                write!(
                    formatter,
                    "private source artifact {id} cannot be published"
                )
            }
            Self::InvalidIdentifier { label, value } => {
                write!(formatter, "invalid {label}: {value:?}")
            }
            Self::EmptyProvenance => formatter.write_str("dependency provenance cannot be empty"),
            Self::DuplicateProvenance(id) => write!(formatter, "duplicate provenance ID {id}"),
            Self::UnsafeOutputPath(path) => {
                write!(formatter, "unsafe output-relative path {path:?}")
            }
            Self::InstanceIdentityMismatch {
                logical_id,
                instance_id,
            } => write!(
                formatter,
                "artifact {logical_id} contains instance plan {instance_id}"
            ),
            Self::TransferSyntaxMismatch {
                logical_id,
                instance_uid,
                encoding_uid,
            } => write!(
                formatter,
                "artifact {logical_id} instance transfer syntax {instance_uid} differs from encoding {encoding_uid}"
            ),
            Self::MissingImplementationIdentity { logical_id } => write!(
                formatter,
                "artifact {logical_id} has no implementation class identity"
            ),
            Self::ImplementationIdentityMismatch {
                logical_id,
                instance_uid,
                encoding_uid,
            } => write!(
                formatter,
                "artifact {logical_id} instance implementation class {instance_uid} differs from encoding {encoding_uid}"
            ),
            Self::InvalidEncodingCombination(message) => {
                write!(formatter, "invalid encoding policy combination: {message}")
            }
            Self::ZeroFragmentSizeLimit => {
                formatter.write_str("fragment maximum bytes must be non-zero")
            }
            Self::BackendTransferSyntaxMismatch {
                backend_id,
                transfer_syntax_uid,
            } => write!(
                formatter,
                "encoding backend {backend_id} does not support transfer syntax {transfer_syntax_uid}"
            ),
            Self::InvalidImplementationVersion => {
                formatter.write_str("implementation version must be 1-16 ASCII bytes")
            }
            Self::EmptyValidationPlan => formatter.write_str("validation plan cannot be empty"),
            Self::DuplicateValidationRule(id) => {
                write!(formatter, "duplicate validation rule {id}")
            }
            Self::DuplicateEvidenceObligation(id) => {
                write!(formatter, "duplicate evidence obligation {id}")
            }
            Self::UnsafePublicationPolicy => {
                formatter.write_str("publication requires private staging and no-overwrite")
            }
            Self::ZeroResourceLimit => formatter.write_str("resource limits must be non-zero"),
            Self::ResourceEstimateOverflow => {
                formatter.write_str("artifact resource estimate overflow")
            }
            Self::ResourceEstimateExceedsLimit {
                artifacts,
                output_bytes,
                peak_working_bytes,
            } => write!(
                formatter,
                "planned resources exceed run limits: artifacts={artifacts}, output_bytes={output_bytes}, peak_working_bytes={peak_working_bytes}"
            ),
            Self::ZeroArtifactWorkingSet => {
                formatter.write_str("artifact peak working bytes must be non-zero")
            }
            Self::ZeroFrameNumber {
                artifact_id,
                depends_on,
            } => write!(
                formatter,
                "dependency {artifact_id} -> {depends_on} contains frame zero"
            ),
            Self::DuplicateFrameNumber {
                artifact_id,
                depends_on,
                frame,
            } => write!(
                formatter,
                "dependency {artifact_id} -> {depends_on} repeats frame {frame}"
            ),
            Self::IncompleteMutationPlan => formatter.write_str("mutation plan is incomplete"),
            Self::InvalidByteRange { start, end } => {
                write!(formatter, "invalid mutation byte range {start}..{end}")
            }
            Self::EmptyUnavailableMessage(id) => {
                write!(formatter, "unavailable capability {id} has no message")
            }
            Self::AvailableArtifactMarkedUnavailable {
                capability_id,
                artifact_id,
            } => write!(
                formatter,
                "unavailable capability {capability_id} names planned artifact {artifact_id}"
            ),
        }
    }
}

impl Error for CorpusPlanError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Serialize(error) => Some(error),
            _ => None,
        }
    }
}
