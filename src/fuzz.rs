//! Bounded, deterministic fuzzing primitives.
//!
//! This module deliberately contains no DICOM payload fixtures and performs no
//! file I/O. Committed seeds describe how a known-good generated case is
//! resolved; callers provide those bytes at runtime. Candidate bytes and
//! minimized bytes are build artifacts and must not be committed.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use serde::Serialize;

pub const FUZZ_CONTRACT_VERSION: &str = "0.1.0";

const ABSOLUTE_MAX_INPUT_BYTES: usize = 64 * 1024 * 1024;
const ABSOLUTE_MAX_ITERATIONS: u64 = 1_000_000;
const ABSOLUTE_MAX_MUTATIONS: u64 = 16_000_000;
const ABSOLUTE_MAX_MUTATIONS_PER_CANDIDATE: u32 = 4_096;
const ABSOLUTE_MAX_TARGET_OPERATIONS: u64 = 1_000_000_000;

/// A byte region which a runtime locator may expose to the generic mutator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MutationSurface {
    FileMeta,
    DatasetHeaders,
    SequenceStructure,
    Encapsulation,
    PixelData,
    TextValues,
}

/// Committed identity for a runtime-generated seed, never the seed payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FuzzSeedDescription {
    pub id: &'static str,
    pub source_case_id: &'static str,
    pub source_recipe_id: &'static str,
    pub source_recipe_version: &'static str,
    pub source_generation_seed: u64,
    pub surfaces: &'static [MutationSurface],
}

/// Initial seed descriptions intentionally contain no DICOM bytes.
pub const INITIAL_SEED_DESCRIPTIONS: &[FuzzSeedDescription] = &[
    FuzzSeedDescription {
        id: "part10-explicit-vr-le-v1",
        source_case_id: "classic/sc/mono2_u8_explicit_le",
        source_recipe_id: "sc_mono2_u8",
        source_recipe_version: "0.1.0",
        source_generation_seed: 7,
        surfaces: &[
            MutationSurface::FileMeta,
            MutationSurface::DatasetHeaders,
            MutationSurface::PixelData,
        ],
    },
    FuzzSeedDescription {
        id: "encapsulated-rle-v1",
        source_case_id: "classic/sc/mono1_u8_rle_lossless",
        source_recipe_id: "sc_mono1_u8_rle_lossless",
        source_recipe_version: "0.1.0",
        source_generation_seed: 7,
        surfaces: &[
            MutationSurface::DatasetHeaders,
            MutationSurface::Encapsulation,
            MutationSurface::PixelData,
        ],
    },
];

/// All limits are operation or byte counts; none depend on elapsed time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct FuzzBudget {
    pub max_iterations: u64,
    pub max_candidates: u64,
    pub max_mutations_per_candidate: u32,
    pub max_total_mutations: u64,
    pub max_bytes_per_mutation: usize,
    pub max_input_bytes: usize,
    pub max_output_bytes: usize,
    pub max_minimization_attempts: u64,
    pub max_total_target_operations: u64,
    pub max_target_operations: u64,
}

impl FuzzBudget {
    pub fn validate(self) -> Result<Self, FuzzError> {
        if self.max_iterations == 0
            || self.max_candidates == 0
            || self.max_mutations_per_candidate == 0
            || self.max_total_mutations == 0
            || self.max_bytes_per_mutation == 0
            || self.max_input_bytes == 0
            || self.max_output_bytes == 0
            || self.max_minimization_attempts == 0
            || self.max_total_target_operations == 0
            || self.max_target_operations == 0
        {
            return Err(FuzzError::InvalidBudget(
                "all budget limits must be non-zero",
            ));
        }
        if self.max_candidates > self.max_iterations {
            return Err(FuzzError::InvalidBudget(
                "max_candidates cannot exceed max_iterations",
            ));
        }
        if self.max_input_bytes > ABSOLUTE_MAX_INPUT_BYTES
            || self.max_output_bytes > ABSOLUTE_MAX_INPUT_BYTES
        {
            return Err(FuzzError::InvalidBudget(
                "input and output limits exceed the absolute 64 MiB ceiling",
            ));
        }
        if self.max_iterations > ABSOLUTE_MAX_ITERATIONS
            || self.max_total_mutations > ABSOLUTE_MAX_MUTATIONS
            || self.max_mutations_per_candidate > ABSOLUTE_MAX_MUTATIONS_PER_CANDIDATE
        {
            return Err(FuzzError::InvalidBudget(
                "iteration or mutation limit exceeds the absolute ceiling",
            ));
        }
        if self.max_target_operations > self.max_total_target_operations
            || self.max_total_target_operations > ABSOLUTE_MAX_TARGET_OPERATIONS
        {
            return Err(FuzzError::InvalidBudget(
                "target operation limits are inconsistent or exceed the absolute ceiling",
            ));
        }
        Ok(self)
    }
}

impl Default for FuzzBudget {
    fn default() -> Self {
        Self {
            max_iterations: 1_024,
            max_candidates: 1_024,
            max_mutations_per_candidate: 8,
            max_total_mutations: 8_192,
            max_bytes_per_mutation: 64,
            max_input_bytes: 8 * 1024 * 1024,
            max_output_bytes: 8 * 1024 * 1024,
            max_minimization_attempts: 2_048,
            max_total_target_operations: 100_000_000,
            max_target_operations: 1_000_000,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FuzzError {
    InvalidBudget(&'static str),
    InvalidSeedDescription(&'static str),
    InputTooLarge {
        actual: usize,
        limit: usize,
    },
    BudgetExhausted(BudgetKind),
    TargetOperationLimitExceeded {
        actual: u64,
        limit: u64,
    },
    OutcomeNotReproduced {
        expected: TargetOutcomeClass,
        actual: TargetOutcomeClass,
    },
    InvalidPromotionRecipe(String),
    InvalidSourceIdentity(String),
    DuplicateSource(String),
    Cancelled,
}

impl fmt::Display for FuzzError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidBudget(detail) => write!(formatter, "invalid fuzz budget: {detail}"),
            Self::InvalidSeedDescription(detail) => {
                write!(formatter, "invalid fuzz seed description: {detail}")
            }
            Self::InputTooLarge { actual, limit } => {
                write!(formatter, "fuzz input is {actual} bytes; limit is {limit}")
            }
            Self::BudgetExhausted(kind) => write!(formatter, "{kind:?} budget exhausted"),
            Self::TargetOperationLimitExceeded { actual, limit } => write!(
                formatter,
                "target used {actual} operations; deterministic limit is {limit}"
            ),
            Self::OutcomeNotReproduced { expected, actual } => write!(
                formatter,
                "candidate outcome {actual:?} does not reproduce expected {expected:?}"
            ),
            Self::InvalidPromotionRecipe(recipe) => {
                write!(
                    formatter,
                    "promotion recipe must be a named negative/ recipe: {recipe}"
                )
            }
            Self::InvalidSourceIdentity(detail) => {
                write!(formatter, "invalid fuzz source identity: {detail}")
            }
            Self::DuplicateSource(id) => write!(formatter, "duplicate fuzz source {id}"),
            Self::Cancelled => write!(formatter, "fuzz qualification was cancelled"),
        }
    }
}

impl Error for FuzzError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BudgetKind {
    Iterations,
    Candidates,
    Mutations,
    MinimizationAttempts,
    TargetOperations,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CandidateMutation {
    Insert {
        offset: usize,
        value: u8,
    },
    Replace {
        offset: usize,
        before: u8,
        after: u8,
    },
    Delete {
        offset: usize,
        length: usize,
    },
    Duplicate {
        offset: usize,
        length: usize,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FuzzCandidate {
    pub contract_version: &'static str,
    pub seed_description_id: &'static str,
    pub run_seed: u64,
    pub candidate_seed: u64,
    pub iteration: u64,
    pub mutations: Vec<CandidateMutation>,
    pub bytes: Vec<u8>,
}

/// Stateful counters make aggregate limits impossible to bypass accidentally.
#[derive(Debug, Clone)]
pub struct FuzzSession {
    description: FuzzSeedDescription,
    run_seed: u64,
    budget: FuzzBudget,
    next_iteration: u64,
    candidates: u64,
    mutations: u64,
}

impl FuzzSession {
    pub fn new(
        description: FuzzSeedDescription,
        run_seed: u64,
        budget: FuzzBudget,
    ) -> Result<Self, FuzzError> {
        let budget = budget.validate()?;
        validate_description(description)?;
        Ok(Self {
            description,
            run_seed,
            budget,
            next_iteration: 0,
            candidates: 0,
            mutations: 0,
        })
    }

    pub fn next_candidate(&mut self, source: &[u8]) -> Result<FuzzCandidate, FuzzError> {
        self.next_candidate_cancellable(source, &|| false)
    }

    pub fn next_candidate_cancellable(
        &mut self,
        source: &[u8],
        cancelled: &dyn Fn() -> bool,
    ) -> Result<FuzzCandidate, FuzzError> {
        check_cancelled(cancelled)?;
        if source.len() > self.budget.max_input_bytes {
            return Err(FuzzError::InputTooLarge {
                actual: source.len(),
                limit: self.budget.max_input_bytes,
            });
        }
        if source.len() > self.budget.max_output_bytes {
            return Err(FuzzError::InputTooLarge {
                actual: source.len(),
                limit: self.budget.max_output_bytes,
            });
        }
        if self.next_iteration >= self.budget.max_iterations {
            return Err(FuzzError::BudgetExhausted(BudgetKind::Iterations));
        }
        if self.candidates >= self.budget.max_candidates {
            return Err(FuzzError::BudgetExhausted(BudgetKind::Candidates));
        }

        let iteration = self.next_iteration;
        let candidate_seed = derive_candidate_seed(self.description.id, self.run_seed, iteration);
        let mut rng = SplitMix64::new(candidate_seed);
        let requested = 1 + rng.bounded(self.budget.max_mutations_per_candidate as u64) as u32;
        let available = self
            .budget
            .max_total_mutations
            .saturating_sub(self.mutations);
        if available == 0 {
            return Err(FuzzError::BudgetExhausted(BudgetKind::Mutations));
        }
        let mutation_count = u64::from(requested).min(available) as u32;
        let mut bytes = source.to_vec();
        let mut mutations = Vec::with_capacity(mutation_count as usize);
        for _ in 0..mutation_count {
            check_cancelled(cancelled)?;
            let mutation = mutate_once(&mut bytes, &mut rng, self.budget);
            mutations.push(mutation);
        }

        self.next_iteration += 1;
        self.candidates += 1;
        self.mutations += u64::from(mutation_count);
        Ok(FuzzCandidate {
            contract_version: FUZZ_CONTRACT_VERSION,
            seed_description_id: self.description.id,
            run_seed: self.run_seed,
            candidate_seed,
            iteration,
            mutations,
            bytes,
        })
    }

    pub const fn counters(&self) -> FuzzCounters {
        FuzzCounters {
            iterations: self.next_iteration,
            candidates: self.candidates,
            mutations: self.mutations,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FuzzCounters {
    pub iterations: u64,
    pub candidates: u64,
    pub mutations: u64,
}

/// Outcomes are deliberately distinct; a timeout is never a clean rejection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetOutcome {
    Accepted,
    CleanRejection,
    ParseFailure,
    ValidationFailure,
    DecodeFailure,
    Crash { signal_or_code: i32 },
    Hang,
    Timeout,
    ResourceLimit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetOutcomeClass {
    Accepted,
    CleanRejection,
    ParseFailure,
    ValidationFailure,
    DecodeFailure,
    Crash,
    Hang,
    Timeout,
    ResourceLimit,
}

impl TargetOutcome {
    pub const fn class(&self) -> TargetOutcomeClass {
        match self {
            Self::Accepted => TargetOutcomeClass::Accepted,
            Self::CleanRejection => TargetOutcomeClass::CleanRejection,
            Self::ParseFailure => TargetOutcomeClass::ParseFailure,
            Self::ValidationFailure => TargetOutcomeClass::ValidationFailure,
            Self::DecodeFailure => TargetOutcomeClass::DecodeFailure,
            Self::Crash { .. } => TargetOutcomeClass::Crash,
            Self::Hang => TargetOutcomeClass::Hang,
            Self::Timeout => TargetOutcomeClass::Timeout,
            Self::ResourceLimit => TargetOutcomeClass::ResourceLimit,
        }
    }

    pub const fn is_unacceptable(&self) -> bool {
        matches!(
            self,
            Self::Crash { .. } | Self::Hang | Self::Timeout | Self::ResourceLimit
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetObservation {
    pub outcome: TargetOutcome,
    /// Deterministic parser/decoder instruction counter supplied by the harness.
    pub operations: u64,
}

impl TargetObservation {
    pub fn checked(self, operation_limit: u64) -> Result<Self, FuzzError> {
        if self.operations > operation_limit {
            return Err(FuzzError::TargetOperationLimitExceeded {
                actual: self.operations,
                limit: operation_limit,
            });
        }
        Ok(self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MinimizationResult {
    pub bytes: Vec<u8>,
    pub attempts: u64,
    pub target_operations: u64,
    pub preserved_outcome: TargetOutcomeClass,
}

/// A private source supplied by the executor. The bytes are borrowed only for
/// the duration of execution and are never copied into qualification evidence.
#[derive(Debug, Clone, Copy)]
pub struct FuzzSourceAsset<'a> {
    pub private_asset_id: &'a str,
    pub description: FuzzSeedDescription,
    pub declared_sha256: &'a str,
    pub declared_size_bytes: usize,
    pub bytes: &'a [u8],
}

#[derive(Debug, Clone, Copy)]
pub struct FuzzQualificationRequest<'a> {
    pub case_id: &'a str,
    pub profile: &'a str,
    pub run_seed: u64,
    pub budget: FuzzBudget,
    pub iterations_per_source: u64,
    pub sources: &'a [FuzzSourceAsset<'a>],
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FuzzProviderEvidence {
    pub kind: &'static str,
    pub id: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FuzzTargetEvidence {
    pub kind: &'static str,
    pub independence: &'static str,
    pub operation_unit: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FuzzSeedEvidence {
    pub id: &'static str,
    pub source_case_id: &'static str,
    pub source_recipe_id: &'static str,
    pub source_recipe_version: &'static str,
    pub source_generation_seed: u64,
    pub source_sha256: String,
    pub source_size_bytes: usize,
    pub surfaces: Vec<&'static str>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct FuzzQualificationCounters {
    pub iterations: u64,
    pub candidates: u64,
    pub mutations: u64,
    pub target_operations: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FuzzMinimizationEvidence {
    pub seed_description_id: &'static str,
    pub candidate_iteration: u64,
    pub candidate_seed: u64,
    pub outcome: &'static str,
    pub original_size: usize,
    pub minimized_size: usize,
    pub attempts: u64,
    pub target_operations: u64,
    pub minimized_fingerprint: String,
}

/// Payload-free qualification result. No field can contain source, candidate,
/// or minimized bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FuzzQualificationEvidence {
    pub case_id: String,
    pub kind: &'static str,
    pub contract_version: &'static str,
    pub profile: String,
    pub run_seed: u64,
    pub provider: FuzzProviderEvidence,
    pub target: FuzzTargetEvidence,
    pub budget: FuzzBudget,
    pub seeds: Vec<FuzzSeedEvidence>,
    pub counters: FuzzQualificationCounters,
    pub outcomes: BTreeMap<&'static str, u64>,
    pub minimizations: Vec<FuzzMinimizationEvidence>,
    pub unacceptable_outcomes: Vec<&'static str>,
    pub payload_policy: &'static str,
    pub status: &'static str,
}

impl FuzzQualificationEvidence {
    pub fn to_json(&self) -> Result<serde_json::Value, serde_json::Error> {
        serde_json::to_value(self)
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct BoundedFuzzQualificationService;

impl BoundedFuzzQualificationService {
    pub fn execute<F>(
        &self,
        request: FuzzQualificationRequest<'_>,
        mut observe: F,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<FuzzQualificationEvidence, FuzzError>
    where
        F: FnMut(&[u8], u64) -> TargetObservation,
    {
        let budget = request.budget.validate()?;
        if request.case_id.is_empty() || request.profile.is_empty() {
            return Err(FuzzError::InvalidSourceIdentity(
                "qualification case and profile must be non-empty".into(),
            ));
        }
        if request.iterations_per_source == 0
            || request.iterations_per_source > budget.max_iterations
            || request.iterations_per_source > budget.max_candidates
        {
            return Err(FuzzError::InvalidBudget(
                "iterations_per_source exceeds the session budget",
            ));
        }
        if request.sources.is_empty() {
            return Err(FuzzError::InvalidSourceIdentity(
                "at least one private source is required".into(),
            ));
        }
        let requested_candidates = u64::try_from(request.sources.len())
            .ok()
            .and_then(|count| count.checked_mul(request.iterations_per_source))
            .ok_or(FuzzError::BudgetExhausted(BudgetKind::Candidates))?;
        if requested_candidates > budget.max_iterations {
            return Err(FuzzError::BudgetExhausted(BudgetKind::Iterations));
        }
        if requested_candidates > budget.max_candidates {
            return Err(FuzzError::BudgetExhausted(BudgetKind::Candidates));
        }

        let mut seen = BTreeSet::new();
        let mut seed_records = Vec::with_capacity(request.sources.len());
        let mut outcomes = outcome_counts();
        let mut minimizations = Vec::new();
        let mut counters = FuzzQualificationCounters {
            iterations: 0,
            candidates: 0,
            mutations: 0,
            target_operations: 0,
        };

        for source in request.sources {
            check_cancelled(cancelled)?;
            validate_source_asset(*source, budget)?;
            if !seen.insert(source.description.id) {
                return Err(FuzzError::DuplicateSource(source.description.id.into()));
            }
            seed_records.push(FuzzSeedEvidence {
                id: source.description.id,
                source_case_id: source.description.source_case_id,
                source_recipe_id: source.description.source_recipe_id,
                source_recipe_version: source.description.source_recipe_version,
                source_generation_seed: source.description.source_generation_seed,
                source_sha256: source.declared_sha256.into(),
                source_size_bytes: source.bytes.len(),
                surfaces: source
                    .description
                    .surfaces
                    .iter()
                    .map(mutation_surface_name)
                    .collect(),
            });

            let mut session = FuzzSession::new(source.description, request.run_seed, budget)?;
            let mut first_rejection = None;
            for _ in 0..request.iterations_per_source {
                check_cancelled(cancelled)?;
                let candidate = session.next_candidate_cancellable(source.bytes, cancelled)?;
                let remaining = budget
                    .max_total_target_operations
                    .saturating_sub(counters.target_operations);
                if remaining == 0 {
                    return Err(FuzzError::BudgetExhausted(BudgetKind::TargetOperations));
                }
                let operation_limit = remaining.min(budget.max_target_operations);
                let observation =
                    observe(&candidate.bytes, operation_limit).checked(operation_limit)?;
                counters.target_operations = counters
                    .target_operations
                    .checked_add(observation.operations)
                    .filter(|total| *total <= budget.max_total_target_operations)
                    .ok_or(FuzzError::BudgetExhausted(BudgetKind::TargetOperations))?;
                *outcomes
                    .get_mut(outcome_name(observation.outcome.class()))
                    .expect("all outcomes are initialized") += 1;
                if first_rejection.is_none()
                    && matches!(
                        observation.outcome.class(),
                        TargetOutcomeClass::CleanRejection | TargetOutcomeClass::ParseFailure
                    )
                {
                    first_rejection = Some((candidate, observation.outcome.class()));
                }
            }
            let session_counters = session.counters();
            counters.iterations = counters
                .iterations
                .checked_add(session_counters.iterations)
                .filter(|total| *total <= budget.max_iterations)
                .ok_or(FuzzError::BudgetExhausted(BudgetKind::Iterations))?;
            counters.candidates = counters
                .candidates
                .checked_add(session_counters.candidates)
                .filter(|total| *total <= budget.max_candidates)
                .ok_or(FuzzError::BudgetExhausted(BudgetKind::Candidates))?;
            counters.mutations = counters
                .mutations
                .checked_add(session_counters.mutations)
                .filter(|total| *total <= budget.max_total_mutations)
                .ok_or(FuzzError::BudgetExhausted(BudgetKind::Mutations))?;

            if let Some((candidate, outcome)) = first_rejection {
                let remaining = budget
                    .max_total_target_operations
                    .saturating_sub(counters.target_operations);
                if remaining == 0 {
                    return Err(FuzzError::BudgetExhausted(BudgetKind::TargetOperations));
                }
                let mut minimization_budget = budget;
                minimization_budget.max_total_target_operations = remaining;
                minimization_budget.max_target_operations =
                    minimization_budget.max_target_operations.min(remaining);
                let minimized = minimize_candidate_cancellable(
                    &candidate.bytes,
                    outcome,
                    minimization_budget,
                    &mut observe,
                    cancelled,
                )?;
                counters.target_operations = counters
                    .target_operations
                    .checked_add(minimized.target_operations)
                    .filter(|total| *total <= budget.max_total_target_operations)
                    .ok_or(FuzzError::BudgetExhausted(BudgetKind::TargetOperations))?;
                minimizations.push(FuzzMinimizationEvidence {
                    seed_description_id: source.description.id,
                    candidate_iteration: candidate.iteration,
                    candidate_seed: candidate.candidate_seed,
                    outcome: outcome_name(outcome),
                    original_size: candidate.bytes.len(),
                    minimized_size: minimized.bytes.len(),
                    attempts: minimized.attempts,
                    target_operations: minimized.target_operations,
                    minimized_fingerprint: payload_fingerprint(&minimized.bytes),
                });
            }
        }

        let unacceptable = ["crash", "hang", "timeout", "resource_limit"]
            .iter()
            .map(|name| outcomes[name])
            .sum::<u64>();
        Ok(FuzzQualificationEvidence {
            case_id: request.case_id.into(),
            kind: "bounded_fuzz_run",
            contract_version: FUZZ_CONTRACT_VERSION,
            profile: request.profile.into(),
            run_seed: request.run_seed,
            provider: FuzzProviderEvidence {
                kind: "mutation_layer",
                id: "bounded_deterministic_fuzz",
            },
            target: FuzzTargetEvidence {
                kind: "same_project_bounded_part10_probe",
                independence: "same_project",
                operation_unit: "input_byte",
            },
            budget,
            seeds: seed_records,
            counters,
            outcomes,
            minimizations,
            unacceptable_outcomes: vec!["crash", "hang", "timeout", "resource_limit"],
            payload_policy: "generated_payloads_uncommitted",
            status: if unacceptable == 0 {
                "passed"
            } else {
                "failed"
            },
        })
    }
}

fn validate_source_asset(source: FuzzSourceAsset<'_>, budget: FuzzBudget) -> Result<(), FuzzError> {
    validate_description(source.description)?;
    if !INITIAL_SEED_DESCRIPTIONS.contains(&source.description) {
        return Err(FuzzError::InvalidSourceIdentity(format!(
            "{} description is not committed",
            source.private_asset_id
        )));
    }
    if source.private_asset_id.is_empty() {
        return Err(FuzzError::InvalidSourceIdentity(
            "private asset handle must be non-empty".into(),
        ));
    }
    if source.declared_size_bytes != source.bytes.len() {
        return Err(FuzzError::InvalidSourceIdentity(format!(
            "{} size differs from private bytes",
            source.private_asset_id
        )));
    }
    if source.bytes.len() > budget.max_input_bytes {
        return Err(FuzzError::InputTooLarge {
            actual: source.bytes.len(),
            limit: budget.max_input_bytes,
        });
    }
    let actual = crate::sha256_hex(source.bytes);
    if source.declared_sha256 != actual {
        return Err(FuzzError::InvalidSourceIdentity(format!(
            "{} SHA-256 differs from private bytes",
            source.private_asset_id
        )));
    }
    Ok(())
}

fn outcome_counts() -> BTreeMap<&'static str, u64> {
    BTreeMap::from([
        ("accepted", 0),
        ("clean_rejection", 0),
        ("parse_failure", 0),
        ("validation_failure", 0),
        ("decode_failure", 0),
        ("crash", 0),
        ("hang", 0),
        ("timeout", 0),
        ("resource_limit", 0),
    ])
}

const fn mutation_surface_name(surface: &MutationSurface) -> &'static str {
    match surface {
        MutationSurface::FileMeta => "file_meta",
        MutationSurface::DatasetHeaders => "dataset_headers",
        MutationSurface::SequenceStructure => "sequence_structure",
        MutationSurface::Encapsulation => "encapsulation",
        MutationSurface::PixelData => "pixel_data",
        MutationSurface::TextValues => "text_values",
    }
}

const fn outcome_name(outcome: TargetOutcomeClass) -> &'static str {
    match outcome {
        TargetOutcomeClass::Accepted => "accepted",
        TargetOutcomeClass::CleanRejection => "clean_rejection",
        TargetOutcomeClass::ParseFailure => "parse_failure",
        TargetOutcomeClass::ValidationFailure => "validation_failure",
        TargetOutcomeClass::DecodeFailure => "decode_failure",
        TargetOutcomeClass::Crash => "crash",
        TargetOutcomeClass::Hang => "hang",
        TargetOutcomeClass::Timeout => "timeout",
        TargetOutcomeClass::ResourceLimit => "resource_limit",
    }
}

fn payload_fingerprint(bytes: &[u8]) -> String {
    format!("fnv1a64:{:016x}", stable_hash(bytes))
}

/// Deterministically removes chunks, then canonicalizes individual bytes.
///
/// The observer receives the strict operation allowance for each invocation.
/// Its declared count is checked against that allowance and the aggregate cap.
pub fn minimize_candidate<F>(
    candidate: &[u8],
    preserved_outcome: TargetOutcomeClass,
    budget: FuzzBudget,
    observe: F,
) -> Result<MinimizationResult, FuzzError>
where
    F: FnMut(&[u8], u64) -> TargetObservation,
{
    minimize_candidate_cancellable(candidate, preserved_outcome, budget, observe, &|| false)
}

pub fn minimize_candidate_cancellable<F>(
    candidate: &[u8],
    preserved_outcome: TargetOutcomeClass,
    budget: FuzzBudget,
    mut observe: F,
    cancelled: &dyn Fn() -> bool,
) -> Result<MinimizationResult, FuzzError>
where
    F: FnMut(&[u8], u64) -> TargetObservation,
{
    let budget = budget.validate()?;
    if candidate.len() > budget.max_output_bytes {
        return Err(FuzzError::InputTooLarge {
            actual: candidate.len(),
            limit: budget.max_output_bytes,
        });
    }
    let mut current = candidate.to_vec();
    let mut attempts = 0_u64;
    let mut target_operations = 0_u64;
    let mut granularity = 2_usize;

    check_cancelled(cancelled)?;
    attempts += 1;
    let actual = observe_and_account(&current, budget, &mut observe, &mut target_operations)?;
    if actual != preserved_outcome {
        return Err(FuzzError::OutcomeNotReproduced {
            expected: preserved_outcome,
            actual,
        });
    }

    while current.len() > 1 && attempts < budget.max_minimization_attempts {
        check_cancelled(cancelled)?;
        let chunk_size = current.len().div_ceil(granularity);
        let mut reduced = false;
        let mut start = 0_usize;
        while start < current.len() && attempts < budget.max_minimization_attempts {
            check_cancelled(cancelled)?;
            let end = (start + chunk_size).min(current.len());
            let mut trial = Vec::with_capacity(current.len() - (end - start));
            trial.extend_from_slice(&current[..start]);
            trial.extend_from_slice(&current[end..]);
            attempts += 1;
            if observe_and_account(&trial, budget, &mut observe, &mut target_operations)?
                == preserved_outcome
            {
                current = trial;
                granularity = 2;
                reduced = true;
                break;
            }
            start = end;
        }
        if !reduced {
            if granularity >= current.len() {
                break;
            }
            granularity = (granularity * 2).min(current.len());
        }
    }

    for index in 0..current.len() {
        check_cancelled(cancelled)?;
        if attempts >= budget.max_minimization_attempts {
            break;
        }
        if current[index] == 0 {
            continue;
        }
        let original = current[index];
        current[index] = 0;
        attempts += 1;
        if observe_and_account(&current, budget, &mut observe, &mut target_operations)?
            != preserved_outcome
        {
            current[index] = original;
        }
    }

    Ok(MinimizationResult {
        bytes: current,
        attempts,
        target_operations,
        preserved_outcome,
    })
}

fn check_cancelled(cancelled: &dyn Fn() -> bool) -> Result<(), FuzzError> {
    if cancelled() {
        Err(FuzzError::Cancelled)
    } else {
        Ok(())
    }
}

fn observe_and_account<F>(
    bytes: &[u8],
    budget: FuzzBudget,
    observe: &mut F,
    total: &mut u64,
) -> Result<TargetOutcomeClass, FuzzError>
where
    F: FnMut(&[u8], u64) -> TargetObservation,
{
    let remaining = budget.max_total_target_operations.saturating_sub(*total);
    if remaining == 0 {
        return Err(FuzzError::BudgetExhausted(BudgetKind::TargetOperations));
    }
    let operation_limit = remaining.min(budget.max_target_operations);
    let observation = observe(bytes, operation_limit).checked(operation_limit)?;
    *total = total
        .checked_add(observation.operations)
        .ok_or(FuzzError::BudgetExhausted(BudgetKind::TargetOperations))?;
    Ok(observation.outcome.class())
}

/// Payload-free record used when converting an interesting input into a stable
/// named negative recipe. The actual bytes remain an uncommitted artifact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromotionRecord {
    pub contract_version: &'static str,
    pub named_negative_recipe: String,
    pub seed_description_id: &'static str,
    pub run_seed: u64,
    pub candidate_iteration: u64,
    pub candidate_seed: u64,
    pub mutations: Vec<CandidateMutation>,
    pub original_size: usize,
    pub minimized_size: usize,
    pub minimized_fingerprint: String,
    pub reproduced_outcome: TargetOutcomeClass,
}

impl PromotionRecord {
    pub fn new(
        named_negative_recipe: impl Into<String>,
        candidate: &FuzzCandidate,
        minimized: &MinimizationResult,
    ) -> Result<Self, FuzzError> {
        let named_negative_recipe = named_negative_recipe.into();
        if !named_negative_recipe.starts_with("negative/")
            || named_negative_recipe.len() == "negative/".len()
            || named_negative_recipe
                .bytes()
                .any(|byte| byte.is_ascii_whitespace())
        {
            return Err(FuzzError::InvalidPromotionRecipe(named_negative_recipe));
        }
        Ok(Self {
            contract_version: FUZZ_CONTRACT_VERSION,
            named_negative_recipe,
            seed_description_id: candidate.seed_description_id,
            run_seed: candidate.run_seed,
            candidate_iteration: candidate.iteration,
            candidate_seed: candidate.candidate_seed,
            mutations: candidate.mutations.clone(),
            original_size: candidate.bytes.len(),
            minimized_size: minimized.bytes.len(),
            minimized_fingerprint: format!("fnv1a64:{:016x}", stable_hash(&minimized.bytes)),
            reproduced_outcome: minimized.preserved_outcome,
        })
    }
}

fn validate_description(description: FuzzSeedDescription) -> Result<(), FuzzError> {
    if description.id.is_empty()
        || description.source_case_id.is_empty()
        || description.source_recipe_id.is_empty()
        || description.source_recipe_version.is_empty()
    {
        return Err(FuzzError::InvalidSeedDescription(
            "identity fields must be non-empty",
        ));
    }
    if description.surfaces.is_empty() {
        return Err(FuzzError::InvalidSeedDescription(
            "at least one mutation surface is required",
        ));
    }
    Ok(())
}

fn mutate_once(bytes: &mut Vec<u8>, rng: &mut SplitMix64, budget: FuzzBudget) -> CandidateMutation {
    if bytes.is_empty() {
        let value = rng.next_u64() as u8;
        bytes.push(value);
        return CandidateMutation::Insert { offset: 0, value };
    }

    let can_grow = bytes.len() < budget.max_output_bytes;
    match rng.bounded(if can_grow { 4 } else { 2 }) {
        0 => {
            let offset = rng.bounded(bytes.len() as u64) as usize;
            let before = bytes[offset];
            let mut after = rng.next_u64() as u8;
            if after == before {
                after ^= 0x80;
            }
            bytes[offset] = after;
            CandidateMutation::Replace {
                offset,
                before,
                after,
            }
        }
        1 => {
            let offset = rng.bounded(bytes.len() as u64) as usize;
            let available = bytes.len() - offset;
            let length =
                1 + rng.bounded(available.min(budget.max_bytes_per_mutation) as u64) as usize;
            bytes.drain(offset..offset + length);
            CandidateMutation::Delete { offset, length }
        }
        2 => {
            let offset = rng.bounded((bytes.len() + 1) as u64) as usize;
            let value = rng.next_u64() as u8;
            bytes.insert(offset, value);
            CandidateMutation::Insert { offset, value }
        }
        _ => {
            let offset = rng.bounded(bytes.len() as u64) as usize;
            let available_source = bytes.len() - offset;
            let available_output = budget.max_output_bytes - bytes.len();
            let maximum = available_source
                .min(available_output)
                .min(budget.max_bytes_per_mutation);
            if maximum == 0 {
                let before = bytes[offset];
                let after = before ^ 1;
                bytes[offset] = after;
                return CandidateMutation::Replace {
                    offset,
                    before,
                    after,
                };
            }
            let length = 1 + rng.bounded(maximum as u64) as usize;
            let copy = bytes[offset..offset + length].to_vec();
            bytes.splice(offset + length..offset + length, copy);
            CandidateMutation::Duplicate { offset, length }
        }
    }
}

fn derive_candidate_seed(description_id: &str, run_seed: u64, iteration: u64) -> u64 {
    let identity = stable_hash(description_id.as_bytes());
    mix64(identity ^ run_seed.rotate_left(17) ^ iteration.wrapping_mul(0x9e3779b97f4a7c15))
}

fn stable_hash(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
    })
}

fn mix64(mut value: u64) -> u64 {
    value ^= value >> 30;
    value = value.wrapping_mul(0xbf58476d1ce4e5b9);
    value ^= value >> 27;
    value = value.wrapping_mul(0x94d049bb133111eb);
    value ^ (value >> 31)
}

#[derive(Debug, Clone)]
struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    const fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9e3779b97f4a7c15);
        mix64(self.state)
    }

    fn bounded(&mut self, upper_exclusive: u64) -> u64 {
        debug_assert!(upper_exclusive > 0);
        let threshold = upper_exclusive.wrapping_neg() % upper_exclusive;
        loop {
            let value = self.next_u64();
            if value >= threshold {
                return value % upper_exclusive;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    fn small_budget() -> FuzzBudget {
        FuzzBudget {
            max_iterations: 4,
            max_candidates: 4,
            max_mutations_per_candidate: 3,
            max_total_mutations: 8,
            max_bytes_per_mutation: 4,
            max_input_bytes: 32,
            max_output_bytes: 32,
            max_minimization_attempts: 64,
            max_total_target_operations: 1_000,
            max_target_operations: 100,
        }
    }

    fn source_assets<'a>(first: &'a [u8], second: &'a [u8]) -> [FuzzSourceAsset<'a>; 2] {
        [
            FuzzSourceAsset {
                private_asset_id: "private:explicit",
                description: INITIAL_SEED_DESCRIPTIONS[0],
                declared_sha256: Box::leak(crate::sha256_hex(first).into_boxed_str()),
                declared_size_bytes: first.len(),
                bytes: first,
            },
            FuzzSourceAsset {
                private_asset_id: "private:rle",
                description: INITIAL_SEED_DESCRIPTIONS[1],
                declared_sha256: Box::leak(crate::sha256_hex(second).into_boxed_str()),
                declared_size_bytes: second.len(),
                bytes: second,
            },
        ]
    }

    fn qualification<'a>(sources: &'a [FuzzSourceAsset<'a>]) -> FuzzQualificationRequest<'a> {
        let mut budget = small_budget();
        budget.max_iterations = 8;
        budget.max_candidates = 8;
        budget.max_total_mutations = 16;
        FuzzQualificationRequest {
            case_id: "fuzz/parser/bounded_seed_corpus",
            profile: "fuzz",
            run_seed: 19,
            budget,
            iterations_per_source: 4,
            sources,
        }
    }

    fn clean_rejection(bytes: &[u8], _limit: u64) -> TargetObservation {
        TargetObservation {
            outcome: TargetOutcome::CleanRejection,
            operations: u64::try_from(bytes.len()).unwrap_or(u64::MAX).max(1),
        }
    }

    #[test]
    fn committed_seeds_are_descriptions_without_payloads() {
        assert_eq!(INITIAL_SEED_DESCRIPTIONS.len(), 2);
        for seed in INITIAL_SEED_DESCRIPTIONS {
            validate_description(*seed).expect("valid committed seed description");
            assert!(!seed.source_case_id.is_empty());
            assert!(!seed.surfaces.is_empty());
        }
    }

    #[test]
    fn candidate_generation_is_reproducible_and_seed_separated() {
        let description = INITIAL_SEED_DESCRIPTIONS[0];
        let source = b"DICM deterministic source";
        let mut first = FuzzSession::new(description, 41, small_budget()).unwrap();
        let mut second = FuzzSession::new(description, 41, small_budget()).unwrap();
        for _ in 0..4 {
            assert_eq!(
                first.next_candidate(source).unwrap(),
                second.next_candidate(source).unwrap()
            );
        }
        let mut other = FuzzSession::new(description, 42, small_budget()).unwrap();
        assert_ne!(
            FuzzSession::new(description, 41, small_budget())
                .unwrap()
                .next_candidate(source)
                .unwrap(),
            other.next_candidate(source).unwrap()
        );
    }

    #[test]
    fn candidate_bytes_never_exceed_strict_limits() {
        let mut budget = small_budget();
        budget.max_output_bytes = 8;
        budget.max_input_bytes = 8;
        let mut session = FuzzSession::new(INITIAL_SEED_DESCRIPTIONS[0], 9, budget).unwrap();
        for _ in 0..4 {
            let candidate = session.next_candidate(b"12345678").unwrap();
            assert!(candidate.bytes.len() <= 8);
            assert!(candidate.mutations.len() <= 3);
        }
        assert!(session.counters().mutations <= 8);
    }

    #[test]
    fn iteration_candidate_and_mutation_budgets_stop_sessions() {
        let mut budget = small_budget();
        budget.max_candidates = 2;
        let mut session = FuzzSession::new(INITIAL_SEED_DESCRIPTIONS[0], 1, budget).unwrap();
        session.next_candidate(b"abc").unwrap();
        session.next_candidate(b"abc").unwrap();
        assert_eq!(
            session.next_candidate(b"abc").unwrap_err(),
            FuzzError::BudgetExhausted(BudgetKind::Candidates)
        );

        let mut budget = small_budget();
        budget.max_total_mutations = 1;
        let mut session = FuzzSession::new(INITIAL_SEED_DESCRIPTIONS[0], 1, budget).unwrap();
        assert_eq!(session.next_candidate(b"abc").unwrap().mutations.len(), 1);
        assert_eq!(
            session.next_candidate(b"abc").unwrap_err(),
            FuzzError::BudgetExhausted(BudgetKind::Mutations)
        );
    }

    #[test]
    fn oversized_and_unreasonable_budgets_are_rejected() {
        let mut budget = small_budget();
        budget.max_input_bytes = ABSOLUTE_MAX_INPUT_BYTES + 1;
        assert!(matches!(
            budget.validate(),
            Err(FuzzError::InvalidBudget(_))
        ));

        let mut session =
            FuzzSession::new(INITIAL_SEED_DESCRIPTIONS[0], 1, small_budget()).unwrap();
        assert_eq!(
            session.next_candidate(&[0; 33]).unwrap_err(),
            FuzzError::InputTooLarge {
                actual: 33,
                limit: 32
            }
        );
    }

    #[test]
    fn empty_inputs_are_mutated_without_panics() {
        let mut session =
            FuzzSession::new(INITIAL_SEED_DESCRIPTIONS[0], 3, small_budget()).unwrap();
        let candidate = session.next_candidate(&[]).unwrap();
        assert!(matches!(
            candidate.mutations[0],
            CandidateMutation::Insert { .. }
        ));
        assert!(candidate.bytes.len() <= small_budget().max_output_bytes);
    }

    #[test]
    fn outcomes_keep_crash_hang_timeout_distinct() {
        assert_eq!(
            TargetOutcome::Crash { signal_or_code: 11 }.class(),
            TargetOutcomeClass::Crash
        );
        assert_eq!(TargetOutcome::Hang.class(), TargetOutcomeClass::Hang);
        assert_eq!(TargetOutcome::Timeout.class(), TargetOutcomeClass::Timeout);
        for outcome in [
            TargetOutcome::Crash { signal_or_code: 11 },
            TargetOutcome::Hang,
            TargetOutcome::Timeout,
            TargetOutcome::ResourceLimit,
        ] {
            assert!(outcome.is_unacceptable());
        }
        assert!(!TargetOutcome::CleanRejection.is_unacceptable());
    }

    #[test]
    fn target_operation_limit_is_checked_without_wall_clock_time() {
        let observation = TargetObservation {
            outcome: TargetOutcome::Timeout,
            operations: 101,
        };
        assert_eq!(
            observation
                .checked(small_budget().max_target_operations)
                .unwrap_err(),
            FuzzError::TargetOperationLimitExceeded {
                actual: 101,
                limit: 100
            }
        );
    }

    #[test]
    fn minimization_is_deterministic_and_bounded() {
        let candidate = b"prefix-BUG-suffix";
        let observer = |bytes: &[u8], _operation_limit: u64| TargetObservation {
            outcome: if bytes.windows(3).any(|window| window == b"BUG") {
                TargetOutcome::Crash { signal_or_code: 11 }
            } else {
                TargetOutcome::Accepted
            },
            operations: bytes.len() as u64,
        };
        let first = minimize_candidate(
            candidate,
            TargetOutcomeClass::Crash,
            small_budget(),
            observer,
        )
        .unwrap();
        let second = minimize_candidate(
            candidate,
            TargetOutcomeClass::Crash,
            small_budget(),
            observer,
        )
        .unwrap();
        assert_eq!(first, second);
        assert_eq!(first.bytes, b"BUG");
        assert!(first.attempts <= small_budget().max_minimization_attempts);
    }

    #[test]
    fn minimizer_rejects_target_operation_overruns() {
        let error =
            minimize_candidate(b"BUG", TargetOutcomeClass::Crash, small_budget(), |_, _| {
                TargetObservation {
                    outcome: TargetOutcome::Crash { signal_or_code: 11 },
                    operations: 101,
                }
            })
            .unwrap_err();
        assert!(matches!(
            error,
            FuzzError::TargetOperationLimitExceeded { .. }
        ));
    }

    #[test]
    fn minimizer_requires_the_original_candidate_to_reproduce() {
        let error = minimize_candidate(
            b"ordinary input",
            TargetOutcomeClass::Crash,
            small_budget(),
            |bytes, _| TargetObservation {
                outcome: TargetOutcome::Accepted,
                operations: bytes.len() as u64,
            },
        )
        .unwrap_err();
        assert_eq!(
            error,
            FuzzError::OutcomeNotReproduced {
                expected: TargetOutcomeClass::Crash,
                actual: TargetOutcomeClass::Accepted
            }
        );
    }

    #[test]
    fn minimizer_rejects_aggregate_target_operation_overruns() {
        let mut budget = small_budget();
        budget.max_total_target_operations = 3;
        budget.max_target_operations = 3;
        let error = minimize_candidate(
            b"BUG",
            TargetOutcomeClass::Crash,
            budget,
            |_, operation_limit| TargetObservation {
                outcome: TargetOutcome::Crash { signal_or_code: 11 },
                operations: 2.min(operation_limit + 1),
            },
        )
        .unwrap_err();
        assert_eq!(
            error,
            FuzzError::TargetOperationLimitExceeded {
                actual: 2,
                limit: 1
            }
        );
    }

    #[test]
    fn promotion_is_payload_free_and_names_a_negative_recipe() {
        let mut session =
            FuzzSession::new(INITIAL_SEED_DESCRIPTIONS[0], 4, small_budget()).unwrap();
        let candidate = session.next_candidate(b"prefix-BUG-suffix").unwrap();
        let minimized = MinimizationResult {
            bytes: b"BUG".to_vec(),
            attempts: 3,
            target_operations: 12,
            preserved_outcome: TargetOutcomeClass::Crash,
        };
        let record =
            PromotionRecord::new("negative/regression/parser_bug_001", &candidate, &minimized)
                .unwrap();
        assert_eq!(record.minimized_size, 3);
        assert!(record.minimized_fingerprint.starts_with("fnv1a64:"));
        assert_eq!(record.reproduced_outcome, TargetOutcomeClass::Crash);
        assert_eq!(
            PromotionRecord::new("fuzz/generated-payload", &candidate, &minimized).unwrap_err(),
            FuzzError::InvalidPromotionRecipe("fuzz/generated-payload".into())
        );
    }

    #[test]
    fn qualification_is_reproducible_payload_free_and_historically_shaped() {
        let first = b"DICM private explicit source";
        let second = b"DICM private encapsulated source";
        let sources = source_assets(first, second);
        let service = BoundedFuzzQualificationService;
        let left = service
            .execute(qualification(&sources), clean_rejection, &|| false)
            .unwrap();
        let right = service
            .execute(qualification(&sources), clean_rejection, &|| false)
            .unwrap();
        assert_eq!(left, right);
        assert_eq!(left.status, "passed");
        assert_eq!(left.counters.iterations, 8);
        assert_eq!(left.counters.candidates, 8);
        assert_eq!(left.outcomes["clean_rejection"], 8);
        assert_eq!(left.minimizations.len(), 2);

        let value = left.to_json().unwrap();
        assert_eq!(value["kind"], "bounded_fuzz_run");
        assert_eq!(value["provider"]["id"], "bounded_deterministic_fuzz");
        assert_eq!(value["target"]["operation_unit"], "input_byte");
        assert_eq!(value["payload_policy"], "generated_payloads_uncommitted");
        let encoded = serde_json::to_vec(&value).unwrap();
        assert!(!encoded.windows(first.len()).any(|window| window == first));
        assert!(!encoded.windows(second.len()).any(|window| window == second));
        for forbidden in ["bytes", "payload", "candidate_payload", "minimized_payload"] {
            assert!(
                !value.as_object().unwrap().contains_key(forbidden),
                "evidence retained {forbidden}"
            );
        }
        assert_eq!(
            crate::sha256_hex(&encoded),
            "110407a9e3b6486e6d5733aa0e89fd72028fd5efbce1b108ff860e626412be09"
        );
    }

    #[test]
    fn qualification_rejects_identity_budget_and_duplicate_drift() {
        let first = b"first";
        let second = b"second";
        let mut sources = source_assets(first, second);
        sources[0].declared_sha256 = "0";
        assert!(matches!(
            BoundedFuzzQualificationService.execute(
                qualification(&sources),
                clean_rejection,
                &|| false
            ),
            Err(FuzzError::InvalidSourceIdentity(_))
        ));

        let mut sources = source_assets(first, second);
        sources[1].description = sources[0].description;
        assert!(matches!(
            BoundedFuzzQualificationService.execute(
                qualification(&sources),
                clean_rejection,
                &|| false
            ),
            Err(FuzzError::DuplicateSource(_))
        ));

        let sources = source_assets(&[0; 33], second);
        assert_eq!(
            BoundedFuzzQualificationService
                .execute(qualification(&sources), clean_rejection, &|| false)
                .unwrap_err(),
            FuzzError::InputTooLarge {
                actual: 33,
                limit: 32,
            }
        );

        let sources = source_assets(first, second);
        assert!(matches!(
            BoundedFuzzQualificationService.execute(
                qualification(&sources),
                |_, limit| TargetObservation {
                    outcome: TargetOutcome::Accepted,
                    operations: limit + 1,
                },
                &|| false,
            ),
            Err(FuzzError::TargetOperationLimitExceeded { .. })
        ));
    }

    #[test]
    fn qualification_cancels_without_evidence_or_payload_retention() {
        let sources = source_assets(b"first", b"second");
        let observations = Cell::new(0_u64);
        let result = BoundedFuzzQualificationService.execute(
            qualification(&sources),
            |bytes, limit| {
                observations.set(observations.get() + 1);
                clean_rejection(bytes, limit)
            },
            &|| true,
        );
        assert_eq!(result.unwrap_err(), FuzzError::Cancelled);
        assert_eq!(observations.get(), 0);
    }

    #[test]
    fn unacceptable_outcomes_are_explicit_failures() {
        let sources = source_assets(b"first", b"second");
        let evidence = BoundedFuzzQualificationService
            .execute(
                qualification(&sources),
                |bytes, _| TargetObservation {
                    outcome: TargetOutcome::Crash { signal_or_code: 11 },
                    operations: bytes.len() as u64,
                },
                &|| false,
            )
            .unwrap();
        assert_eq!(evidence.status, "failed");
        assert_eq!(evidence.outcomes["crash"], 8);
        assert!(evidence.minimizations.is_empty());
    }
}
