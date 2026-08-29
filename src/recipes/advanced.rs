//! Frontend-neutral contracts for advanced DICOM plan providers.
//!
//! The contract deliberately stops at immutable plan data and execution
//! bindings. It has no output root, filesystem handle, materializer, or writer.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::composition::MaterializedReference;
use crate::corpus_plan::{
    ArtifactDependency, CORPUS_PLAN_SCHEMA_VERSION, CorpusPlan, CorpusPlanError, PlannedArtifact,
    PlannedDicomArtifact, PublicationPlan, ResourcePlan,
};
use crate::executor::services::ArtifactExecutionBindings;
use crate::planning::RecipeIdentity;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdvancedProviderFamily {
    Enhanced,
    WholeSlide,
    Registration,
    PresentationState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WholeSlideArtifactKind {
    Volume,
    Thumbnail,
    Label,
}

/// Stable artifact role within an advanced provider response.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum AdvancedArtifactRole {
    EnhancedInstance {
        ordinal: u32,
    },
    WholeSlidePyramid {
        level: u32,
        artifact_kind: WholeSlideArtifactKind,
    },
    Registration {
        ordinal: u32,
    },
    PresentationState {
        ordinal: u32,
    },
}

impl AdvancedArtifactRole {
    fn family(&self) -> AdvancedProviderFamily {
        match self {
            Self::EnhancedInstance { .. } => AdvancedProviderFamily::Enhanced,
            Self::WholeSlidePyramid { .. } => AdvancedProviderFamily::WholeSlide,
            Self::Registration { .. } => AdvancedProviderFamily::Registration,
            Self::PresentationState { .. } => AdvancedProviderFamily::PresentationState,
        }
    }

    fn validate(&self) -> Result<(), AdvancedProviderContractError> {
        let positive = match self {
            Self::EnhancedInstance { ordinal }
            | Self::Registration { ordinal }
            | Self::PresentationState { ordinal } => *ordinal,
            Self::WholeSlidePyramid { level, .. } => level.saturating_add(1),
        };
        if positive == 0 {
            return Err(AdvancedProviderContractError::ZeroRoleOrdinal);
        }
        Ok(())
    }
}

/// The owned semantic role of a referenced source artifact.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum AdvancedSourceRole {
    EnhancedSourceImage,
    WholeSlidePyramidSource,
    RegistrationFixed,
    RegistrationMoving,
    PresentationSourceImage,
    PresentationBlendingInput { input_number: u16 },
}

impl AdvancedSourceRole {
    fn family(&self) -> AdvancedProviderFamily {
        match self {
            Self::EnhancedSourceImage => AdvancedProviderFamily::Enhanced,
            Self::WholeSlidePyramidSource => AdvancedProviderFamily::WholeSlide,
            Self::RegistrationFixed | Self::RegistrationMoving => {
                AdvancedProviderFamily::Registration
            }
            Self::PresentationSourceImage | Self::PresentationBlendingInput { .. } => {
                AdvancedProviderFamily::PresentationState
            }
        }
    }

    pub fn dependency_relationship(&self) -> &'static str {
        match self {
            Self::EnhancedSourceImage => "enhanced_source_image",
            Self::WholeSlidePyramidSource => "whole_slide_pyramid_source",
            Self::RegistrationFixed => "registration_fixed",
            Self::RegistrationMoving => "registration_moving",
            Self::PresentationSourceImage => "presentation_source_image",
            Self::PresentationBlendingInput { .. } => "presentation_blending_input",
        }
    }

    fn validate(&self) -> Result<(), AdvancedProviderContractError> {
        if matches!(self, Self::PresentationBlendingInput { input_number: 0 }) {
            return Err(AdvancedProviderContractError::ZeroSourceOrdinal);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdvancedSourceConsumer {
    pub artifact_id: String,
    pub role: AdvancedSourceRole,
}

/// Typed ownership provenance, cross-checked with the canonical provenance on
/// the planned artifact and with its graph dependencies.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum AdvancedArtifactProvenance {
    Requested,
    Dependency {
        requested_by: Vec<AdvancedSourceConsumer>,
    },
    PrivateSource {
        consumed_by: Vec<AdvancedSourceConsumer>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdvancedProviderLimits {
    pub max_artifacts: u64,
    pub max_references: u64,
    pub max_binding_slots: u64,
    pub max_total_output_bytes: u64,
    pub max_peak_working_bytes: u64,
    pub max_parallelism: u32,
}

impl AdvancedProviderLimits {
    fn validate(&self) -> Result<(), AdvancedProviderContractError> {
        if self.max_artifacts == 0
            || self.max_references == 0
            || self.max_binding_slots == 0
            || self.max_total_output_bytes == 0
            || self.max_peak_working_bytes == 0
            || self.max_parallelism == 0
        {
            return Err(AdvancedProviderContractError::ZeroResourceLimit);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdvancedPlanProviderRequest {
    pub provider_id: String,
    pub family: AdvancedProviderFamily,
    pub case_id: String,
    pub recipe: RecipeIdentity,
    pub seed: u64,
    pub limits: AdvancedProviderLimits,
}

impl AdvancedPlanProviderRequest {
    pub fn validate(&self) -> Result<(), AdvancedProviderContractError> {
        validate_identifier("provider_id", &self.provider_id)?;
        validate_identifier("case_id", &self.case_id)?;
        validate_identifier("recipe_id", &self.recipe.recipe_id)?;
        validate_identifier("recipe_version", &self.recipe.recipe_version)?;
        self.limits.validate()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdvancedPlannedArtifact {
    pub role: AdvancedArtifactRole,
    pub planned: PlannedDicomArtifact,
    pub provenance: AdvancedArtifactProvenance,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdvancedSourceReference {
    pub owner_artifact_id: String,
    pub source_artifact_id: String,
    pub source_role: AdvancedSourceRole,
    pub reference: MaterializedReference,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdvancedPlanProviderOutput {
    pub artifacts: Vec<AdvancedPlannedArtifact>,
    #[serde(default)]
    pub dependencies: Vec<ArtifactDependency>,
    #[serde(default)]
    pub references: Vec<AdvancedSourceReference>,
    pub bindings: Vec<ArtifactExecutionBindings>,
}

impl AdvancedPlanProviderOutput {
    pub fn validate(
        &self,
        request: &AdvancedPlanProviderRequest,
    ) -> Result<(), AdvancedProviderContractError> {
        request.validate()?;
        if self.artifacts.is_empty() {
            return Err(AdvancedProviderContractError::EmptyOutput);
        }
        if self.artifacts.len() as u64 > request.limits.max_artifacts
            || self.references.len() as u64 > request.limits.max_references
        {
            return Err(AdvancedProviderContractError::ResourceLimitExceeded);
        }

        let mut artifact_ids = BTreeSet::new();
        let mut instance_to_artifact = BTreeMap::new();
        let mut roles = BTreeSet::new();
        let mut paths = BTreeSet::new();
        let mut prior_order = None;
        let mut output_bytes = 0_u64;
        let mut peak_working_bytes = 0_u64;
        for artifact in &self.artifacts {
            artifact.role.validate()?;
            if artifact.role.family() != request.family {
                return Err(AdvancedProviderContractError::FamilyRoleMismatch);
            }
            validate_identifier("artifact_id", &artifact.planned.logical_id)?;
            if !artifact_ids.insert(artifact.planned.logical_id.clone()) {
                return Err(AdvancedProviderContractError::DuplicateArtifact(
                    artifact.planned.logical_id.clone(),
                ));
            }
            if instance_to_artifact
                .insert(
                    artifact.planned.instance.instance_id.clone(),
                    artifact.planned.logical_id.clone(),
                )
                .is_some()
            {
                return Err(AdvancedProviderContractError::DuplicateInstance(
                    artifact.planned.instance.instance_id.clone(),
                ));
            }
            if !roles.insert(artifact.role.clone()) {
                return Err(AdvancedProviderContractError::DuplicateArtifactRole);
            }
            if !paths.insert(artifact.planned.output.relative_path.clone()) {
                return Err(AdvancedProviderContractError::DuplicateOutputPath(
                    artifact.planned.output.relative_path.to_string(),
                ));
            }
            if prior_order.is_some_and(|prior| artifact.planned.order <= prior) {
                return Err(AdvancedProviderContractError::MisorderedArtifacts);
            }
            prior_order = Some(artifact.planned.order);
            if artifact.planned.resources.peak_working_bytes == 0 {
                return Err(AdvancedProviderContractError::ZeroArtifactWorkingSet(
                    artifact.planned.logical_id.clone(),
                ));
            }
            output_bytes = output_bytes
                .checked_add(artifact.planned.resources.output_bytes)
                .ok_or(AdvancedProviderContractError::ResourceOverflow)?;
            peak_working_bytes =
                peak_working_bytes.max(artifact.planned.resources.peak_working_bytes);
        }
        if output_bytes > request.limits.max_total_output_bytes
            || peak_working_bytes > request.limits.max_peak_working_bytes
        {
            return Err(AdvancedProviderContractError::ResourceLimitExceeded);
        }

        let mut dependency_pairs = BTreeSet::new();
        for dependency in &self.dependencies {
            if !artifact_ids.contains(&dependency.artifact_id)
                || !artifact_ids.contains(&dependency.depends_on)
            {
                return Err(AdvancedProviderContractError::UnknownDependency);
            }
            if !dependency_pairs.insert((
                dependency.artifact_id.as_str(),
                dependency.depends_on.as_str(),
                dependency.relationship.as_str(),
            )) {
                return Err(AdvancedProviderContractError::DuplicateDependency);
            }
        }

        let mut reference_keys = BTreeSet::new();
        for source in &self.references {
            source.source_role.validate()?;
            if source.source_role.family() != request.family {
                return Err(AdvancedProviderContractError::FamilyRoleMismatch);
            }
            let Some(owner_instance) = self
                .artifacts
                .iter()
                .find(|artifact| artifact.planned.logical_id == source.owner_artifact_id)
            else {
                return Err(AdvancedProviderContractError::UnknownReferenceOwner(
                    source.owner_artifact_id.clone(),
                ));
            };
            let Some(source_instance) = self
                .artifacts
                .iter()
                .find(|artifact| artifact.planned.logical_id == source.source_artifact_id)
            else {
                return Err(AdvancedProviderContractError::UnknownReferenceSource(
                    source.source_artifact_id.clone(),
                ));
            };
            if source.reference.source_instance_id != owner_instance.planned.instance.instance_id
                || source.reference.target_instance_id
                    != source_instance.planned.instance.instance_id
            {
                return Err(AdvancedProviderContractError::ReferenceOwnershipMismatch);
            }
            if !owner_instance
                .planned
                .instance
                .references
                .contains(&source.reference)
            {
                return Err(AdvancedProviderContractError::ReferenceMissingFromInstance);
            }
            if !dependency_pairs.contains(&(
                source.owner_artifact_id.as_str(),
                source.source_artifact_id.as_str(),
                source.source_role.dependency_relationship(),
            )) {
                return Err(AdvancedProviderContractError::MissingReferenceDependency);
            }
            if !reference_keys.insert((
                &source.owner_artifact_id,
                &source.source_artifact_id,
                &source.source_role,
            )) {
                return Err(AdvancedProviderContractError::DuplicateReference);
            }
        }
        let planned_reference_count =
            self.artifacts.iter().try_fold(0_usize, |count, artifact| {
                count
                    .checked_add(artifact.planned.instance.references.len())
                    .ok_or(AdvancedProviderContractError::ResourceOverflow)
            })?;
        if planned_reference_count != self.references.len() {
            return Err(AdvancedProviderContractError::UnboundMaterializedReference);
        }

        for artifact in &self.artifacts {
            validate_provenance(artifact, request.family, &artifact_ids, &dependency_pairs)?;
        }

        let mut bound_ids = BTreeSet::new();
        let mut slot_count = 0_u64;
        for binding in &self.bindings {
            if !artifact_ids.contains(&binding.artifact_id) {
                return Err(AdvancedProviderContractError::UnknownBinding(
                    binding.artifact_id.clone(),
                ));
            }
            if !bound_ids.insert(binding.artifact_id.clone()) {
                return Err(AdvancedProviderContractError::DuplicateBinding(
                    binding.artifact_id.clone(),
                ));
            }
            slot_count = slot_count
                .checked_add(binding.slots.len() as u64)
                .ok_or(AdvancedProviderContractError::ResourceOverflow)?;
            let expected = self
                .artifacts
                .iter()
                .find(|artifact| artifact.planned.logical_id == binding.artifact_id)
                .expect("bound artifact was indexed")
                .planned
                .instance
                .content
                .iter()
                .map(|content| content.slot.as_str())
                .collect::<BTreeSet<_>>();
            let actual = binding
                .slots
                .keys()
                .map(String::as_str)
                .collect::<BTreeSet<_>>();
            if expected != actual {
                return Err(AdvancedProviderContractError::BindingSlotMismatch(
                    binding.artifact_id.clone(),
                ));
            }
        }
        if bound_ids != artifact_ids {
            return Err(AdvancedProviderContractError::MissingBinding);
        }
        if slot_count > request.limits.max_binding_slots {
            return Err(AdvancedProviderContractError::ResourceLimitExceeded);
        }
        Ok(())
    }

    /// Assemble the validated provider response into the canonical corpus plan.
    /// Publication remains a caller-owned run concern and is still relative.
    pub fn to_corpus_plan(
        &self,
        request: &AdvancedPlanProviderRequest,
        publication: PublicationPlan,
    ) -> Result<CorpusPlan, AdvancedProviderContractError> {
        self.validate(request)?;
        let plan = CorpusPlan {
            schema_version: CORPUS_PLAN_SCHEMA_VERSION.to_owned(),
            seed: request.seed,
            artifacts: self
                .artifacts
                .iter()
                .map(|artifact| PlannedArtifact::Dicom(artifact.planned.clone()))
                .collect(),
            dependencies: self.dependencies.clone(),
            unavailable: Vec::new(),
            publication,
            resources: ResourcePlan {
                max_artifacts: request.limits.max_artifacts,
                max_total_output_bytes: request.limits.max_total_output_bytes,
                max_peak_working_bytes: request.limits.max_peak_working_bytes,
                max_parallelism: request.limits.max_parallelism,
            },
        };
        plan.validate()
            .map_err(AdvancedProviderContractError::CorpusPlan)?;
        Ok(plan)
    }
}

pub trait AdvancedPlanProvider: Send + Sync {
    type ProviderInput: Send + Sync;

    fn provider_id(&self) -> &str;

    fn plan(
        &self,
        request: &AdvancedPlanProviderRequest,
        input: &Self::ProviderInput,
    ) -> Result<AdvancedPlanProviderOutput, AdvancedProviderContractError>;
}

fn validate_provenance<'a>(
    artifact: &'a AdvancedPlannedArtifact,
    family: AdvancedProviderFamily,
    artifact_ids: &BTreeSet<String>,
    dependencies: &BTreeSet<(&'a str, &'a str, &'a str)>,
) -> Result<(), AdvancedProviderContractError> {
    use crate::corpus_plan::ArtifactProvenance;

    let (consumers, canonical, private) = match &artifact.provenance {
        AdvancedArtifactProvenance::Requested => {
            if artifact.planned.provenance != ArtifactProvenance::Requested {
                return Err(AdvancedProviderContractError::ProvenanceMismatch);
            }
            return Ok(());
        }
        AdvancedArtifactProvenance::Dependency { requested_by } => (
            requested_by,
            match &artifact.planned.provenance {
                ArtifactProvenance::Dependency { requested_by } => requested_by,
                _ => return Err(AdvancedProviderContractError::ProvenanceMismatch),
            },
            false,
        ),
        AdvancedArtifactProvenance::PrivateSource { consumed_by } => (
            consumed_by,
            match &artifact.planned.provenance {
                ArtifactProvenance::PrivateSource { consumed_by } => consumed_by,
                _ => return Err(AdvancedProviderContractError::ProvenanceMismatch),
            },
            true,
        ),
    };
    if consumers.is_empty() {
        return Err(AdvancedProviderContractError::EmptyProvenance);
    }
    let mut typed_ids = BTreeSet::new();
    for consumer in consumers {
        consumer.role.validate()?;
        if consumer.role.family() != family {
            return Err(AdvancedProviderContractError::FamilyRoleMismatch);
        }
        if !artifact_ids.contains(&consumer.artifact_id)
            || !typed_ids.insert(consumer.artifact_id.as_str())
        {
            return Err(AdvancedProviderContractError::InvalidProvenanceConsumer);
        }
        let pair = if private {
            (
                consumer.artifact_id.as_str(),
                artifact.planned.logical_id.as_str(),
                consumer.role.dependency_relationship(),
            )
        } else {
            (
                artifact.planned.logical_id.as_str(),
                consumer.artifact_id.as_str(),
                consumer.role.dependency_relationship(),
            )
        };
        if !dependencies.contains(&pair) {
            return Err(AdvancedProviderContractError::ProvenanceDependencyMismatch);
        }
    }
    if canonical
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>()
        != typed_ids
    {
        return Err(AdvancedProviderContractError::ProvenanceMismatch);
    }
    if private && artifact.planned.output.publish {
        return Err(AdvancedProviderContractError::PrivateSourcePublished);
    }
    Ok(())
}

fn validate_identifier(
    field: &'static str,
    value: &str,
) -> Result<(), AdvancedProviderContractError> {
    if value.is_empty()
        || value.len() > 256
        || value.contains('\0')
        || value.chars().any(char::is_control)
    {
        return Err(AdvancedProviderContractError::InvalidIdentifier {
            field,
            value: value.to_owned(),
        });
    }
    Ok(())
}

#[derive(Debug)]
pub enum AdvancedProviderContractError {
    InvalidIdentifier { field: &'static str, value: String },
    ZeroResourceLimit,
    EmptyOutput,
    ResourceLimitExceeded,
    ResourceOverflow,
    ZeroRoleOrdinal,
    ZeroSourceOrdinal,
    FamilyRoleMismatch,
    DuplicateArtifact(String),
    DuplicateInstance(String),
    DuplicateArtifactRole,
    DuplicateOutputPath(String),
    MisorderedArtifacts,
    ZeroArtifactWorkingSet(String),
    UnknownDependency,
    DuplicateDependency,
    UnknownReferenceOwner(String),
    UnknownReferenceSource(String),
    ReferenceOwnershipMismatch,
    ReferenceMissingFromInstance,
    MissingReferenceDependency,
    DuplicateReference,
    UnboundMaterializedReference,
    ProvenanceMismatch,
    EmptyProvenance,
    InvalidProvenanceConsumer,
    ProvenanceDependencyMismatch,
    PrivateSourcePublished,
    UnknownBinding(String),
    DuplicateBinding(String),
    MissingBinding,
    BindingSlotMismatch(String),
    InvalidProviderOutput(String),
    CorpusPlan(CorpusPlanError),
}

impl fmt::Display for AdvancedProviderContractError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidIdentifier { field, value } => {
                write!(formatter, "invalid {field} `{value}`")
            }
            Self::ZeroResourceLimit => formatter.write_str("advanced provider limit is zero"),
            Self::EmptyOutput => formatter.write_str("advanced provider returned no artifacts"),
            Self::ResourceLimitExceeded => {
                formatter.write_str("advanced provider output exceeds its request limits")
            }
            Self::ResourceOverflow => formatter.write_str("advanced provider resource overflow"),
            Self::ZeroRoleOrdinal => {
                formatter.write_str("advanced artifact ordinal must be positive")
            }
            Self::ZeroSourceOrdinal => {
                formatter.write_str("advanced source ordinal must be positive")
            }
            Self::FamilyRoleMismatch => formatter
                .write_str("artifact role does not belong to the requested advanced family"),
            Self::DuplicateArtifact(id) => write!(formatter, "duplicate advanced artifact `{id}`"),
            Self::DuplicateInstance(id) => write!(formatter, "duplicate advanced instance `{id}`"),
            Self::DuplicateArtifactRole => formatter.write_str("duplicate advanced artifact role"),
            Self::DuplicateOutputPath(path) => {
                write!(formatter, "duplicate advanced output path `{path}`")
            }
            Self::MisorderedArtifacts => {
                formatter.write_str("advanced artifacts are not in strictly increasing order")
            }
            Self::ZeroArtifactWorkingSet(id) => {
                write!(formatter, "advanced artifact `{id}` has a zero working set")
            }
            Self::UnknownDependency => {
                formatter.write_str("advanced dependency names an unknown artifact")
            }
            Self::DuplicateDependency => formatter.write_str("duplicate advanced dependency"),
            Self::UnknownReferenceOwner(id) => write!(formatter, "unknown reference owner `{id}`"),
            Self::UnknownReferenceSource(id) => {
                write!(formatter, "unknown reference source `{id}`")
            }
            Self::ReferenceOwnershipMismatch => formatter.write_str(
                "materialized reference instance ownership does not match its artifacts",
            ),
            Self::ReferenceMissingFromInstance => {
                formatter.write_str("materialized reference is absent from its owner instance plan")
            }
            Self::MissingReferenceDependency => {
                formatter.write_str("materialized reference has no matching typed dependency")
            }
            Self::DuplicateReference => formatter.write_str("duplicate advanced source reference"),
            Self::UnboundMaterializedReference => formatter
                .write_str("every materialized reference must have one typed source binding"),
            Self::ProvenanceMismatch => {
                formatter.write_str("typed and canonical artifact provenance differ")
            }
            Self::EmptyProvenance => formatter.write_str("advanced dependency provenance is empty"),
            Self::InvalidProvenanceConsumer => {
                formatter.write_str("advanced provenance has an unknown or duplicate consumer")
            }
            Self::ProvenanceDependencyMismatch => {
                formatter.write_str("advanced provenance has no matching graph dependency")
            }
            Self::PrivateSourcePublished => {
                formatter.write_str("private advanced source cannot be published")
            }
            Self::UnknownBinding(id) => {
                write!(formatter, "execution binding names unknown artifact `{id}`")
            }
            Self::DuplicateBinding(id) => {
                write!(formatter, "duplicate execution binding for `{id}`")
            }
            Self::MissingBinding => {
                formatter.write_str("an advanced artifact is missing its execution binding")
            }
            Self::BindingSlotMismatch(id) => write!(
                formatter,
                "execution slots do not match resolved content for `{id}`"
            ),
            Self::InvalidProviderOutput(error) => {
                write!(formatter, "advanced provider output is invalid: {error}")
            }
            Self::CorpusPlan(error) => {
                write!(formatter, "assembled corpus plan is invalid: {error}")
            }
        }
    }
}

impl Error for AdvancedProviderContractError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::CorpusPlan(error) => Some(error),
            _ => None,
        }
    }
}
