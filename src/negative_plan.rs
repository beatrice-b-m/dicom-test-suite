//! Neutral plan-first provider for expected-invalid mutation artifacts.
//!
//! The provider previews an already-resolved, inline valid source with the
//! shared Part 10 materializer, derives the checked mutation contract entirely
//! in memory, and returns fragments for the common CorpusPlan transaction.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use serde_json::{Value, json};

use crate::corpus_plan::{
    ArtifactDependency, ArtifactProvenance, ArtifactResourceEstimate, CaseBinding,
    EvidenceIndependence, EvidenceObligation, EvidencePlan, MutationPlan, OutputPlan,
    PlannedArtifact, PlannedByteRange, PlannedChangedByteRange, PlannedDicomArtifact,
    PlannedMutationArtifact, PlannedMutationOperation, PlannedMutationSource, ValidationPlan,
    ValidationRequirement, ValidationRule,
};
use crate::executor::services::{
    ArtifactExecutionBindings, SlotExecutionBinding, StagedAssetHandle,
};
use crate::mutation::{
    AcceptableOutcome, ByteRange, FailureLayer, LengthWidth, MutationParameters,
};
use crate::negative::{
    NEGATIVE_RECIPE_VERSION, NegativeError, NegativeParserProbeEvidence, RecipeNegativeEvidence,
    build_negative_from_recipe, classify_negative_parser_probe,
};
use crate::planning_preview::{PlanningPreviewLimits, preview_planned_dicom};
use crate::recipes::MutationRecipe;

pub const NEGATIVE_PLAN_PROVIDER_ID: &str = "native.negative_mutation_plan";
pub const NEGATIVE_PARSER_RULE_ID: &str = "expected_invalid_parser_probe";

#[derive(Debug, Clone)]
pub struct NegativePlanProviderRequest {
    pub case_binding: CaseBinding,
    pub logical_id: String,
    pub order: u64,
    pub output: OutputPlan,
    pub mutation_recipe: MutationRecipe,
    pub source: PlannedDicomArtifact,
    pub source_logical_role: String,
    pub source_bindings: ArtifactExecutionBindings,
    pub max_source_bytes: u64,
}

#[derive(Debug, Clone)]
pub struct NegativePlanProviderOutput {
    pub artifacts: Vec<PlannedArtifact>,
    pub dependencies: Vec<ArtifactDependency>,
    pub bindings: BTreeMap<String, ArtifactExecutionBindings>,
    pub evidence: RecipeNegativeEvidence,
    pub parser_probe: NegativeParserProbeEvidence,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct NegativePlanProvider;

impl NegativePlanProvider {
    pub fn plan(
        &self,
        request: NegativePlanProviderRequest,
    ) -> Result<NegativePlanProviderOutput, NegativePlanProviderError> {
        if request.case_binding.recipe_version != NEGATIVE_RECIPE_VERSION {
            return Err(NegativePlanProviderError::RecipeVersion {
                expected: NEGATIVE_RECIPE_VERSION.into(),
                actual: request.case_binding.recipe_version,
            });
        }
        if request.max_source_bytes == 0 {
            return Err(NegativePlanProviderError::ZeroSourceLimit);
        }
        if request.output.publish == false || request.output.role != "expected_invalid" {
            return Err(NegativePlanProviderError::InvalidMutationOutput);
        }
        if request.source_bindings.artifact_id != request.source.logical_id {
            return Err(NegativePlanProviderError::SourceBindingIdentity);
        }
        let source_binding = request
            .source
            .case_binding
            .clone()
            .ok_or(NegativePlanProviderError::MissingSourceRecipeIdentity)?;
        if request.mutation_recipe.source.recipe_id != source_binding.recipe_id
            || request.mutation_recipe.source.recipe_version != source_binding.recipe_version
            || request.mutation_recipe.source_logical_role != request.source_logical_role
        {
            return Err(NegativePlanProviderError::WrongSourceRecipeIdentity);
        }
        if request.mutation_recipe.output.role != request.output.role
            || request.mutation_recipe.output.path.as_deref()
                != Some(request.output.relative_path.as_str())
            || request.mutation_recipe.retention != "expected_invalid_only"
        {
            return Err(NegativePlanProviderError::MutationOutputContract);
        }
        if request.logical_id == request.source.logical_id || request.order == request.source.order
        {
            return Err(NegativePlanProviderError::DuplicateArtifactIdentity);
        }

        let source_bytes = preview_planned_dicom(
            &request.source,
            &request.source_bindings,
            PlanningPreviewLimits {
                max_output_bytes: request.max_source_bytes,
                ..PlanningPreviewLimits::default()
            },
            &|| false,
        )
        .map_err(|error| NegativePlanProviderError::BoundPreview(error.to_string()))?
        .bytes;
        let negative = build_negative_from_recipe(
            &request.case_binding.case_id,
            &request.case_binding.recipe_version,
            &request.mutation_recipe,
            &source_binding.case_id,
            &source_bytes,
        )?;
        let mut source = request.source;
        source.provenance = ArtifactProvenance::PrivateSource {
            consumed_by: vec![request.logical_id.clone()],
        };
        source.output.publish = false;
        let source_id = source.logical_id.clone();
        let source_size = u64::try_from(source_bytes.len())
            .map_err(|_| NegativePlanProviderError::ResourceOverflow)?;
        source.resources.output_bytes = source_size;
        source.resources.peak_working_bytes = source.resources.peak_working_bytes.max(source_size);

        let operations = negative
            .evidence
            .steps
            .iter()
            .enumerate()
            .map(|(order, step)| mutation_operation(order, step))
            .collect::<Result<Vec<_>, _>>()?;
        let expected_failure_layers = operations
            .iter()
            .map(|operation| operation.expected_failure_layer.clone())
            .collect();
        let acceptable_outcomes = operations
            .iter()
            .flat_map(|operation| operation.acceptable_outcomes.iter().cloned())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let output_size = u64::try_from(negative.bytes.len())
            .map_err(|_| NegativePlanProviderError::ResourceOverflow)?;
        let peak_working_bytes = source_size
            .checked_add(output_size)
            .ok_or(NegativePlanProviderError::ResourceOverflow)?;
        let final_layer = operations
            .last()
            .map(|operation| operation.expected_failure_layer.as_str())
            .ok_or(NegativePlanProviderError::MissingMutationStep)?;
        let parser_probe = classify_negative_parser_probe(
            &request.case_binding.case_id,
            &negative.bytes,
            final_layer,
        );
        let mutation = PlannedMutationArtifact {
            logical_id: request.logical_id.clone(),
            order: request.order,
            provenance: ArtifactProvenance::Requested,
            case_binding: request.case_binding,
            source_artifact_id: source_id.clone(),
            mutation: MutationPlan {
                contract_version: crate::mutation::MUTATION_CONTRACT_VERSION.into(),
                source_identity: PlannedMutationSource {
                    artifact_id: source_id.clone(),
                    case_id: source_binding.case_id,
                    recipe_id: source_binding.recipe_id,
                    recipe_version: source_binding.recipe_version,
                    expected_sha256: negative.evidence.source.sha256.clone(),
                },
                operations,
                expected_source_sha256: negative.evidence.source.sha256.clone(),
                expected_output_sha256: negative.evidence.output_sha256.clone(),
                expected_failure_layers,
                acceptable_outcomes,
            },
            output: request.output,
            validation: ValidationPlan {
                rules: vec![ValidationRule {
                    rule_id: NEGATIVE_PARSER_RULE_ID.into(),
                    requirement: ValidationRequirement::Required,
                    parameters: BTreeMap::from([
                        ("ordinary_valid_dicom_validation".into(), Value::Bool(false)),
                        ("probe_kind".into(), json!(parser_probe.kind)),
                        (
                            "probe_independence".into(),
                            json!(parser_probe.independence),
                        ),
                        ("expected_outcome".into(), json!(parser_probe.outcome)),
                    ]),
                }],
            },
            evidence: EvidencePlan {
                obligations: vec![EvidenceObligation {
                    obligation_id: "negative_mutation_chain".into(),
                    route_id: "typed_mutation_materialization".into(),
                    independence: EvidenceIndependence::SameProject,
                    required: true,
                    parameters: BTreeMap::from([
                        ("source_shape".into(), json!(negative.evidence.source_shape)),
                        ("probe_detail".into(), json!(parser_probe.detail)),
                    ]),
                }],
            },
            resources: ArtifactResourceEstimate {
                output_bytes: output_size,
                peak_working_bytes,
            },
        };
        let mutation_bindings = ArtifactExecutionBindings {
            artifact_id: request.logical_id.clone(),
            slots: BTreeMap::from([(
                "source".into(),
                SlotExecutionBinding::StagedAsset {
                    asset: StagedAssetHandle::new(format!("output:{source_id}"))
                        .map_err(NegativePlanProviderError::SourceHandle)?,
                },
            )]),
        };
        let dependencies = vec![ArtifactDependency {
            artifact_id: request.logical_id.clone(),
            depends_on: source_id.clone(),
            relationship: "valid_source_for_negative_mutation".into(),
            frame_numbers: Vec::new(),
        }];
        let mut bindings = BTreeMap::new();
        bindings.insert(source_id, request.source_bindings);
        bindings.insert(request.logical_id, mutation_bindings);
        Ok(NegativePlanProviderOutput {
            artifacts: vec![
                PlannedArtifact::Dicom(source),
                PlannedArtifact::Mutation(mutation),
            ],
            dependencies,
            bindings,
            evidence: negative.evidence,
            parser_probe,
        })
    }
}

fn mutation_operation(
    order: usize,
    step: &crate::negative::MutationStepEvidence,
) -> Result<PlannedMutationOperation, NegativePlanProviderError> {
    let changed_byte_ranges = step
        .changed_byte_ranges
        .iter()
        .map(|range| PlannedChangedByteRange {
            source: planned_range(range.source),
            output: planned_range(range.output),
        })
        .collect::<Vec<_>>();
    Ok(PlannedMutationOperation {
        order: u64::try_from(order).map_err(|_| NegativePlanProviderError::ResourceOverflow)?,
        operation_id: step.mutation_id.to_owned(),
        source_ranges: changed_byte_ranges
            .iter()
            .map(|range| range.source)
            .collect(),
        changed_byte_ranges,
        expected_source_sha256: step.source_sha256.clone(),
        expected_output_sha256: step.output_sha256.clone(),
        expected_failure_layer: failure_layer_name(step.expected_failure_layer).into(),
        acceptable_outcomes: step
            .acceptable_outcomes
            .iter()
            .copied()
            .map(acceptable_outcome_name)
            .map(str::to_owned)
            .collect(),
        parameters: mutation_parameters(&step.parameters),
    })
}

fn planned_range(range: ByteRange) -> PlannedByteRange {
    PlannedByteRange {
        start: range.start as u64,
        end: range.end as u64,
    }
}

fn mutation_parameters(parameters: &MutationParameters) -> BTreeMap<String, Value> {
    let mut values = BTreeMap::new();
    match parameters {
        MutationParameters::Truncate { .. }
        | MutationParameters::MissingType1Element { .. }
        | MutationParameters::UndefinedLengthWithoutDelimitation { .. } => {}
        MutationParameters::IncorrectExplicitVrLength {
            width,
            declared_length,
            ..
        }
        | MutationParameters::InvalidPixelByteLength {
            width,
            declared_length,
            ..
        } => {
            values.insert("width".into(), json!(length_width_name(*width)));
            values.insert("declared_length".into(), json!(declared_length));
        }
        MutationParameters::IllegalVr { replacement, .. } => {
            values.insert("replacement".into(), json!(replacement));
        }
        MutationParameters::TransferSyntaxMismatch { replacement, .. }
        | MutationParameters::UidMismatch { replacement, .. }
        | MutationParameters::InvalidCharacterSetDeclaration { replacement, .. }
        | MutationParameters::MalformedEncodedText { replacement, .. } => {
            values.insert("replacement".into(), json!(replacement));
        }
        MutationParameters::InvalidBitsStoredHighBit {
            bits_stored,
            high_bit,
            ..
        } => {
            values.insert("bits_stored".into(), json!(bits_stored));
            values.insert("high_bit".into(), json!(high_bit));
        }
        MutationParameters::BrokenBasicOffsetTable { offset, .. } => {
            values.insert("offset".into(), json!(offset));
        }
        MutationParameters::BrokenExtendedOffsetTable { offset, .. } => {
            values.insert("offset".into(), json!(offset));
        }
        MutationParameters::InvalidNestedItemLength {
            declared_length, ..
        } => {
            values.insert("declared_length".into(), json!(declared_length));
        }
    }
    values
}

fn length_width_name(width: LengthWidth) -> &'static str {
    match width {
        LengthWidth::U16 => "u16",
        LengthWidth::U32 => "u32",
        LengthWidth::U64 => "u64",
    }
}

fn failure_layer_name(layer: FailureLayer) -> &'static str {
    match layer {
        FailureLayer::FileMeta => "file_meta",
        FailureLayer::DatasetParser => "dataset_parser",
        FailureLayer::ValueDecoding => "value_decoding",
        FailureLayer::SemanticValidation => "semantic_validation",
        FailureLayer::PixelDecoding => "pixel_decoding",
        FailureLayer::Encapsulation => "encapsulation",
        FailureLayer::TextDecoding => "text_decoding",
    }
}

fn acceptable_outcome_name(outcome: AcceptableOutcome) -> &'static str {
    match outcome {
        AcceptableOutcome::CleanRejection => "clean_rejection",
        AcceptableOutcome::ParseFailure => "parse_failure",
        AcceptableOutcome::ValidationFailure => "validation_failure",
        AcceptableOutcome::DecodeFailure => "decode_failure",
        AcceptableOutcome::AcceptedWithBoundedWarning => "accepted_with_bounded_warning",
    }
}

#[derive(Debug)]
pub enum NegativePlanProviderError {
    RecipeVersion { expected: String, actual: String },
    ZeroSourceLimit,
    InvalidMutationOutput,
    SourceBindingIdentity,
    MissingSourceRecipeIdentity,
    WrongSourceRecipeIdentity,
    MutationOutputContract,
    DuplicateArtifactIdentity,
    MissingMutationStep,
    ResourceOverflow,
    BoundPreview(String),
    Negative(NegativeError),
    SourceHandle(crate::executor::services::ServiceError),
}

impl fmt::Display for NegativePlanProviderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl Error for NegativePlanProviderError {}

impl From<NegativeError> for NegativePlanProviderError {
    fn from(value: NegativeError) -> Self {
        Self::Negative(value)
    }
}
