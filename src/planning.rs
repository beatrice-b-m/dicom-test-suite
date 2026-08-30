//! Neutral planning interfaces above individual resolved DICOM instances.
//!
//! Planning is intentionally capability-only: the context provides stable
//! identity, template, content, provider, and validation services, but no
//! output root, filesystem path, file handle, materializer, or writer.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::composition::{CanonicalContent, TemplateId, TemplateVersion};
use crate::corpus_plan::{
    ArtifactDependency, ArtifactResourceEstimate, CorpusPlan, CorpusPlanError, EvidenceObligation,
    PlannedArtifact, PublicationPlan, ResourcePlan, UnavailableCapability, ValidationPlan,
};
use crate::uid::{DeterministicUidInput, UidRole, deterministic_uid};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecipeIdentity {
    pub recipe_id: String,
    pub recipe_version: String,
}

impl fmt::Display for RecipeIdentity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}@{}", self.recipe_id, self.recipe_version)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CuratedCaseRequest {
    pub case_id: String,
    pub recipe: RecipeIdentity,
    #[serde(default)]
    pub parameters: BTreeMap<String, Value>,
}

pub trait CasePlanner: Send + Sync {
    fn identity(&self) -> RecipeIdentity;

    fn plan(
        &self,
        request: &CuratedCaseRequest,
        context: &PlanningContext<'_>,
    ) -> Result<PlannedCase, PlanningError>;
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanProviderRequest {
    pub provider_id: String,
    pub case_id: Option<String>,
    pub recipe: Option<RecipeIdentity>,
    #[serde(default)]
    pub parameters: BTreeMap<String, Value>,
    #[serde(default)]
    pub dependency_artifact_ids: Vec<String>,
}

pub trait PlanProvider: Send + Sync {
    fn provider_id(&self) -> &str;

    fn plan(
        &self,
        request: &PlanProviderRequest,
        context: &PlanningContext<'_>,
    ) -> Result<PlanFragment, PlanningError>;
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContentFactoryRequest {
    pub factory_id: String,
    pub logical_artifact_id: String,
    pub slot: String,
    #[serde(default)]
    pub parameters: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContentFactoryOutput {
    pub content: Vec<CanonicalContent>,
    pub resources: ArtifactResourceEstimate,
    #[serde(default)]
    pub evidence: Vec<EvidenceObligation>,
}

pub trait ContentFactory: Send + Sync {
    fn factory_id(&self) -> &str;

    fn plan_content(
        &self,
        request: &ContentFactoryRequest,
        identities: &dyn IdentityService,
    ) -> Result<ContentFactoryOutput, PlanningError>;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeterministicIdentityRequest {
    pub case_id: String,
    pub recipe_version: String,
    pub role: IdentityRole,
    pub file_index: u32,
    pub frame_index: Option<u32>,
    pub referenced_object_index: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdentityRole {
    StudyInstance,
    SeriesInstance,
    SopInstance,
    FrameOfReference,
    DimensionOrganization,
    IrradiationEvent,
    Concatenation,
    ConcatenationSource,
    ImplementationClass,
    DerivedReference,
}

impl From<IdentityRole> for UidRole {
    fn from(value: IdentityRole) -> Self {
        match value {
            IdentityRole::StudyInstance => Self::StudyInstance,
            IdentityRole::SeriesInstance => Self::SeriesInstance,
            IdentityRole::SopInstance => Self::SopInstance,
            IdentityRole::FrameOfReference => Self::FrameOfReference,
            IdentityRole::DimensionOrganization => Self::DimensionOrganization,
            IdentityRole::IrradiationEvent => Self::IrradiationEvent,
            IdentityRole::Concatenation => Self::Concatenation,
            IdentityRole::ConcatenationSource => Self::ConcatenationSource,
            IdentityRole::ImplementationClass => Self::ImplementationClass,
            IdentityRole::DerivedReference => Self::DerivedReference,
        }
    }
}

pub trait IdentityService: Send + Sync {
    fn allocate(&self, request: &DeterministicIdentityRequest) -> Result<String, PlanningError>;
}

#[derive(Debug, Clone)]
pub struct ProjectIdentityService {
    standards_lock_sha256: String,
    seed: u64,
}

impl ProjectIdentityService {
    pub fn new(standards_lock_sha256: impl Into<String>, seed: u64) -> Self {
        Self {
            standards_lock_sha256: standards_lock_sha256.into(),
            seed,
        }
    }
}

impl IdentityService for ProjectIdentityService {
    fn allocate(&self, request: &DeterministicIdentityRequest) -> Result<String, PlanningError> {
        Ok(deterministic_uid(&DeterministicUidInput {
            standards_lock_sha256: &self.standards_lock_sha256,
            case_id: &request.case_id,
            recipe_version: &request.recipe_version,
            run_seed: self.seed,
            file_index: request.file_index,
            frame_index: request.frame_index,
            referenced_object_index: request.referenced_object_index,
            role: request.role.into(),
        }))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilityRequirement {
    pub capability_id: String,
    pub minimum_version: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum CapabilityAvailability {
    Available {
        version: Option<String>,
    },
    Unavailable {
        reason_code: String,
        message: String,
    },
}

pub trait CapabilityService: Send + Sync {
    fn resolve(
        &self,
        requirement: &CapabilityRequirement,
    ) -> Result<CapabilityAvailability, PlanningError>;
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TemplateIdentity {
    pub template_id: TemplateId,
    pub template_version: TemplateVersion,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanningTemplate {
    pub identity: TemplateIdentity,
    pub sop_class_uid: String,
    pub artifact_kind: String,
    pub transfer_syntax_uids: Vec<String>,
    pub validation_rule_ids: Vec<String>,
}

pub trait TemplateService: Send + Sync {
    fn resolve(&self, identity: &TemplateIdentity) -> Result<PlanningTemplate, PlanningError>;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ValidationExecutorDescriptor {
    BuiltIn {
        executor_id: String,
    },
    ExternalTool {
        capability_id: String,
        route_id: String,
    },
    Provider {
        provider_id: String,
        route_id: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValidationRuleDescriptor {
    pub rule_id: String,
    pub layer: String,
    pub executor: ValidationExecutorDescriptor,
}

#[derive(Debug, Clone, Default)]
pub struct ValidationRuleRegistry {
    descriptors: BTreeMap<String, ValidationRuleDescriptor>,
}

impl ValidationRuleRegistry {
    pub fn new(
        descriptors: impl IntoIterator<Item = ValidationRuleDescriptor>,
    ) -> Result<Self, PlanningError> {
        let mut registry = Self::default();
        for descriptor in descriptors {
            let rule_id = descriptor.rule_id.clone();
            validate_identifier("validation rule_id", &rule_id)?;
            validate_identifier("validation layer", &descriptor.layer)?;
            match &descriptor.executor {
                ValidationExecutorDescriptor::BuiltIn { executor_id } => {
                    validate_identifier("validation executor_id", executor_id)?;
                }
                ValidationExecutorDescriptor::ExternalTool {
                    capability_id,
                    route_id,
                } => {
                    validate_identifier("validation capability_id", capability_id)?;
                    validate_identifier("validation route_id", route_id)?;
                }
                ValidationExecutorDescriptor::Provider {
                    provider_id,
                    route_id,
                } => {
                    validate_identifier("validation provider_id", provider_id)?;
                    validate_identifier("validation route_id", route_id)?;
                }
            }
            if registry
                .descriptors
                .insert(rule_id.clone(), descriptor)
                .is_some()
            {
                return Err(PlanningError::DuplicateValidationRule(rule_id));
            }
        }
        Ok(registry)
    }

    pub fn resolve(&self, rule_id: &str) -> Result<&ValidationRuleDescriptor, PlanningError> {
        self.descriptors
            .get(rule_id)
            .ok_or_else(|| PlanningError::UnregisteredValidationRule(rule_id.to_owned()))
    }

    pub fn contains(&self, rule_id: &str) -> bool {
        self.descriptors.contains_key(rule_id)
    }
}

#[derive(Default)]
pub struct CasePlannerRegistry {
    planners: BTreeMap<RecipeIdentity, Arc<dyn CasePlanner>>,
}

impl CasePlannerRegistry {
    pub fn new(
        planners: impl IntoIterator<Item = Arc<dyn CasePlanner>>,
    ) -> Result<Self, PlanningError> {
        let mut registry = Self::default();
        for planner in planners {
            let identity = planner.identity();
            validate_recipe_identity(&identity)?;
            if registry
                .planners
                .insert(identity.clone(), planner)
                .is_some()
            {
                return Err(PlanningError::DuplicateRecipeIdentity(identity));
            }
        }
        Ok(registry)
    }

    pub fn resolve(&self, identity: &RecipeIdentity) -> Result<&dyn CasePlanner, PlanningError> {
        self.planners
            .get(identity)
            .map(Arc::as_ref)
            .ok_or_else(|| PlanningError::UnregisteredRecipeIdentity(identity.clone()))
    }

    pub fn contains(&self, identity: &RecipeIdentity) -> bool {
        self.planners.contains_key(identity)
    }
}

#[derive(Default)]
pub struct PlanProviderRegistry {
    providers: BTreeMap<String, Arc<dyn PlanProvider>>,
}

impl PlanProviderRegistry {
    pub fn new(
        providers: impl IntoIterator<Item = Arc<dyn PlanProvider>>,
    ) -> Result<Self, PlanningError> {
        let mut registry = Self::default();
        for provider in providers {
            let provider_id = provider.provider_id().to_owned();
            validate_identifier("plan provider_id", &provider_id)?;
            if registry
                .providers
                .insert(provider_id.clone(), provider)
                .is_some()
            {
                return Err(PlanningError::DuplicatePlanProvider(provider_id));
            }
        }
        Ok(registry)
    }

    pub fn resolve(&self, provider_id: &str) -> Result<&dyn PlanProvider, PlanningError> {
        self.providers
            .get(provider_id)
            .map(Arc::as_ref)
            .ok_or_else(|| PlanningError::UnregisteredPlanProvider(provider_id.to_owned()))
    }

    pub fn contains(&self, provider_id: &str) -> bool {
        self.providers.contains_key(provider_id)
    }
}

#[derive(Default)]
pub struct ContentFactoryRegistry {
    factories: BTreeMap<String, Arc<dyn ContentFactory>>,
}

impl ContentFactoryRegistry {
    pub fn new(
        factories: impl IntoIterator<Item = Arc<dyn ContentFactory>>,
    ) -> Result<Self, PlanningError> {
        let mut registry = Self::default();
        for factory in factories {
            let factory_id = factory.factory_id().to_owned();
            validate_identifier("content factory_id", &factory_id)?;
            if registry
                .factories
                .insert(factory_id.clone(), factory)
                .is_some()
            {
                return Err(PlanningError::DuplicateContentFactory(factory_id));
            }
        }
        Ok(registry)
    }

    pub fn resolve(&self, factory_id: &str) -> Result<&dyn ContentFactory, PlanningError> {
        self.factories
            .get(factory_id)
            .map(Arc::as_ref)
            .ok_or_else(|| PlanningError::UnregisteredContentFactory(factory_id.to_owned()))
    }

    pub fn contains(&self, factory_id: &str) -> bool {
        self.factories.contains_key(factory_id)
    }
}

pub struct PlanningContext<'a> {
    pub seed: u64,
    pub standards_lock_sha256: &'a str,
    pub identities: &'a dyn IdentityService,
    pub capabilities: &'a dyn CapabilityService,
    pub templates: &'a dyn TemplateService,
    pub content_factories: &'a ContentFactoryRegistry,
    pub validation_rules: &'a ValidationRuleRegistry,
    pub plan_providers: &'a PlanProviderRegistry,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanFragment {
    pub artifacts: Vec<PlannedArtifact>,
    #[serde(default)]
    pub dependencies: Vec<ArtifactDependency>,
    #[serde(default)]
    pub unavailable: Vec<UnavailableCapability>,
    #[serde(default)]
    pub plan_provider_ids: Vec<String>,
    #[serde(default)]
    pub content_factory_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlannedCase {
    pub case_id: String,
    pub recipe: RecipeIdentity,
    pub fragment: PlanFragment,
}

pub struct CorpusPlanAssembler<'a> {
    seed: u64,
    publication: PublicationPlan,
    resources: ResourcePlan,
    planners: &'a CasePlannerRegistry,
    providers: &'a PlanProviderRegistry,
    content_factories: &'a ContentFactoryRegistry,
    validation_rules: &'a ValidationRuleRegistry,
    cases: Vec<PlannedCase>,
    fragments: Vec<PlanFragment>,
}

impl<'a> CorpusPlanAssembler<'a> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        seed: u64,
        publication: PublicationPlan,
        resources: ResourcePlan,
        planners: &'a CasePlannerRegistry,
        providers: &'a PlanProviderRegistry,
        content_factories: &'a ContentFactoryRegistry,
        validation_rules: &'a ValidationRuleRegistry,
    ) -> Self {
        Self {
            seed,
            publication,
            resources,
            planners,
            providers,
            content_factories,
            validation_rules,
            cases: Vec::new(),
            fragments: Vec::new(),
        }
    }

    pub fn add_case(&mut self, planned: PlannedCase) -> Result<(), PlanningError> {
        validate_identifier("planned case_id", &planned.case_id)?;
        validate_recipe_identity(&planned.recipe)?;
        if !self.planners.contains(&planned.recipe) {
            return Err(PlanningError::UnregisteredRecipeIdentity(planned.recipe));
        }
        if self
            .cases
            .iter()
            .any(|existing| existing.case_id == planned.case_id)
        {
            return Err(PlanningError::DuplicateCaseIdentity(planned.case_id));
        }
        if self
            .cases
            .iter()
            .any(|existing| existing.recipe == planned.recipe)
        {
            return Err(PlanningError::DuplicateRecipeIdentity(planned.recipe));
        }
        self.validate_fragment(&planned.fragment)?;
        self.cases.push(planned);
        Ok(())
    }

    pub fn add_fragment(&mut self, fragment: PlanFragment) -> Result<(), PlanningError> {
        self.validate_fragment(&fragment)?;
        self.fragments.push(fragment);
        Ok(())
    }

    pub fn assemble(self) -> Result<CorpusPlan, PlanningError> {
        let mut artifacts = Vec::new();
        let mut dependencies = Vec::new();
        let mut unavailable = Vec::new();
        for fragment in self
            .cases
            .into_iter()
            .map(|planned| planned.fragment)
            .chain(self.fragments)
        {
            artifacts.extend(fragment.artifacts);
            dependencies.extend(fragment.dependencies);
            unavailable.extend(fragment.unavailable);
        }
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
        unavailable.sort_by(|left, right| left.capability_id.cmp(&right.capability_id));
        let mut plan = CorpusPlan {
            schema_version: crate::corpus_plan::CORPUS_PLAN_SCHEMA_VERSION.into(),
            seed: self.seed,
            artifacts,
            dependencies,
            unavailable,
            publication: self.publication,
            resources: self.resources,
        };
        plan.validate().map_err(PlanningError::InvalidCorpusPlan)?;
        let order = plan
            .topological_order()
            .map_err(PlanningError::InvalidCorpusPlan)?;
        let mut by_id = plan
            .artifacts
            .into_iter()
            .map(|artifact| (artifact.logical_id().to_owned(), artifact))
            .collect::<BTreeMap<_, _>>();
        plan.artifacts = order
            .into_iter()
            .map(|id| by_id.remove(&id).expect("validated artifact identity"))
            .collect();
        Ok(plan)
    }

    fn validate_fragment(&self, fragment: &PlanFragment) -> Result<(), PlanningError> {
        let mut provider_ids = BTreeSet::new();
        for provider_id in &fragment.plan_provider_ids {
            if !provider_ids.insert(provider_id) {
                return Err(PlanningError::DuplicatePlanProvider(provider_id.clone()));
            }
            if !self.providers.contains(provider_id) {
                return Err(PlanningError::UnregisteredPlanProvider(provider_id.clone()));
            }
        }
        let mut factory_ids = BTreeSet::new();
        for factory_id in &fragment.content_factory_ids {
            if !factory_ids.insert(factory_id) {
                return Err(PlanningError::DuplicateContentFactory(factory_id.clone()));
            }
            if !self.content_factories.contains(factory_id) {
                return Err(PlanningError::UnregisteredContentFactory(
                    factory_id.clone(),
                ));
            }
        }
        for artifact in &fragment.artifacts {
            for rule in validation_plan(artifact).rules.iter() {
                let descriptor = self.validation_rules.resolve(&rule.rule_id)?;
                if let ValidationExecutorDescriptor::Provider { provider_id, .. } =
                    &descriptor.executor
                {
                    if !self.providers.contains(provider_id) {
                        return Err(PlanningError::UnregisteredPlanProvider(provider_id.clone()));
                    }
                }
            }
        }
        Ok(())
    }
}

fn validation_plan(artifact: &PlannedArtifact) -> &ValidationPlan {
    match artifact {
        PlannedArtifact::Dicom(value) => &value.validation,
        PlannedArtifact::ImportedDicom(value) => &value.validation,
        PlannedArtifact::Mutation(value) => &value.validation,
        PlannedArtifact::Qualification(value) => &value.validation,
        PlannedArtifact::Auxiliary(value) => &value.validation,
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManifestProjectionInput {
    pub corpus_plan_sha256: String,
    #[serde(default)]
    pub execution_evidence: BTreeMap<String, Value>,
}

pub trait ManifestProjector: Send + Sync {
    fn projector_id(&self) -> &str;

    fn project(
        &self,
        plan: &CorpusPlan,
        input: &ManifestProjectionInput,
    ) -> Result<Value, ProjectionError>;
}

fn validate_recipe_identity(identity: &RecipeIdentity) -> Result<(), PlanningError> {
    validate_identifier("recipe_id", &identity.recipe_id)?;
    validate_identifier("recipe_version", &identity.recipe_version)
}

fn validate_identifier(label: &'static str, value: &str) -> Result<(), PlanningError> {
    if value.is_empty()
        || value.len() > 256
        || value.contains('\0')
        || value.chars().any(char::is_control)
    {
        return Err(PlanningError::InvalidIdentifier {
            label,
            value: value.to_owned(),
        });
    }
    Ok(())
}

#[derive(Debug)]
pub enum PlanningError {
    InvalidIdentifier { label: &'static str, value: String },
    DuplicateRecipeIdentity(RecipeIdentity),
    UnregisteredRecipeIdentity(RecipeIdentity),
    DuplicateCaseIdentity(String),
    DuplicatePlanProvider(String),
    UnregisteredPlanProvider(String),
    DuplicateContentFactory(String),
    UnregisteredContentFactory(String),
    DuplicateValidationRule(String),
    UnregisteredValidationRule(String),
    Identity(String),
    Capability(String),
    Template(String),
    Content(String),
    Provider(String),
    InvalidCorpusPlan(CorpusPlanError),
}

impl fmt::Display for PlanningError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidIdentifier { label, value } => {
                write!(formatter, "invalid {label}: {value:?}")
            }
            Self::DuplicateRecipeIdentity(identity) => {
                write!(formatter, "duplicate recipe identity {identity}")
            }
            Self::UnregisteredRecipeIdentity(identity) => {
                write!(formatter, "unregistered recipe identity {identity}")
            }
            Self::DuplicateCaseIdentity(id) => write!(formatter, "duplicate case identity {id}"),
            Self::DuplicatePlanProvider(id) => write!(formatter, "duplicate plan provider {id}"),
            Self::UnregisteredPlanProvider(id) => {
                write!(formatter, "unregistered plan provider {id}")
            }
            Self::DuplicateContentFactory(id) => {
                write!(formatter, "duplicate content factory {id}")
            }
            Self::UnregisteredContentFactory(id) => {
                write!(formatter, "unregistered content factory {id}")
            }
            Self::DuplicateValidationRule(id) => {
                write!(formatter, "duplicate validation rule {id}")
            }
            Self::UnregisteredValidationRule(id) => {
                write!(formatter, "unregistered validation rule {id}")
            }
            Self::Identity(message) => write!(formatter, "identity planning: {message}"),
            Self::Capability(message) => write!(formatter, "capability planning: {message}"),
            Self::Template(message) => write!(formatter, "template planning: {message}"),
            Self::Content(message) => write!(formatter, "content planning: {message}"),
            Self::Provider(message) => write!(formatter, "plan provider: {message}"),
            Self::InvalidCorpusPlan(error) => write!(formatter, "invalid corpus plan: {error}"),
        }
    }
}

impl Error for PlanningError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidCorpusPlan(error) => Some(error),
            _ => None,
        }
    }
}

impl From<CorpusPlanError> for PlanningError {
    fn from(value: CorpusPlanError) -> Self {
        Self::InvalidCorpusPlan(value)
    }
}

#[derive(Debug)]
pub enum ProjectionError {
    InvalidInput(String),
    Projection(String),
}

impl fmt::Display for ProjectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput(message) => write!(formatter, "invalid projection input: {message}"),
            Self::Projection(message) => write!(formatter, "manifest projection: {message}"),
        }
    }
}

impl Error for ProjectionError {}
