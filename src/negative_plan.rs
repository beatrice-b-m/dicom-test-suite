//! Neutral plan-first provider for expected-invalid mutation artifacts.
//!
//! The provider previews an already-resolved, inline valid source with the
//! shared Part 10 materializer, derives the checked mutation contract entirely
//! in memory, and returns fragments for the common CorpusPlan transaction.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use serde_json::{Value, json};

use crate::codecs::{FrameEncodeInput, FrameEncoder, NativeRleLosslessEncoder};
use crate::composition::Part10Materializer;
use crate::corpus_plan::{
    ArtifactDependency, ArtifactProvenance, ArtifactResourceEstimate, CaseBinding,
    EvidenceIndependence, EvidenceObligation, EvidencePlan, MutationPlan, OutputPlan,
    PlannedArtifact, PlannedByteRange, PlannedChangedByteRange, PlannedDicomArtifact,
    PlannedMutationArtifact, PlannedMutationOperation, PlannedMutationSource, ValidationPlan,
    ValidationRequirement, ValidationRule,
};
use crate::encoded_content::{EncodedSlotInput, resolve_encoded_content};
use crate::executor::services::{
    ArtifactExecutionBindings, ByteBinding, SlotExecutionBinding, StagedAssetHandle,
};
use crate::mutation::{
    AcceptableOutcome, ByteRange, FailureLayer, LengthWidth, MutationParameters,
};
use crate::negative::{
    NEGATIVE_RECIPE_VERSION, NegativeError, NegativeParserProbeEvidence, RecipeNegativeEvidence,
    build_negative_from_recipe, classify_negative_parser_probe,
};
use crate::recipes::MutationRecipe;
use crate::sha256_hex;

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

        let preview_bindings = resolve_builtin_codec_preflight(&request.source_bindings)?;
        let source_bytes = if preview_bindings
            .slots
            .values()
            .any(|binding| matches!(binding, SlotExecutionBinding::EncodedFrames { .. }))
        {
            preview_encoded_source(&request.source, &preview_bindings, request.max_source_bytes)?
        } else {
            Part10Materializer
                .preview_part10_bytes_with_encoding(
                    &request.source.instance,
                    &request.source.encoding,
                    request.max_source_bytes,
                )
                .map_err(NegativePlanProviderError::Preview)?
        };
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

fn preview_encoded_source(
    source: &PlannedDicomArtifact,
    bindings: &ArtifactExecutionBindings,
    max_source_bytes: u64,
) -> Result<Vec<u8>, NegativePlanProviderError> {
    let mut inputs = Vec::new();
    for (slot, binding) in &bindings.slots {
        let SlotExecutionBinding::EncodedFrames { frames } = binding else {
            return Err(NegativePlanProviderError::BoundPreview(
                "encoded planning preview accepts encoded inline frames only".into(),
            ));
        };
        let mut ordered = frames.iter().collect::<Vec<_>>();
        ordered.sort_by_key(|frame| frame.frame_number);
        let mut payloads = Vec::with_capacity(ordered.len());
        for (index, frame) in ordered.into_iter().enumerate() {
            if frame.frame_number != u32::try_from(index + 1).unwrap_or(u32::MAX) {
                return Err(NegativePlanProviderError::BoundPreview(
                    "encoded planning preview frames are not contiguous".into(),
                ));
            }
            let ByteBinding::Inline { bytes, sha256 } = &frame.bytes else {
                return Err(NegativePlanProviderError::BoundPreview(
                    "encoded planning preview rejects staged frames".into(),
                ));
            };
            if bytes.len() as u64 != frame.encoded_size_bytes
                || sha256_hex(bytes) != frame.encoded_sha256
                || sha256_hex(bytes) != *sha256
            {
                return Err(NegativePlanProviderError::BoundPreview(
                    "encoded planning preview frame identity drifted".into(),
                ));
            }
            payloads.push(bytes.clone());
        }
        inputs.push(EncodedSlotInput {
            slot: slot.clone(),
            ordered_frames: payloads,
        });
    }
    let resolved = resolve_encoded_content(&source.instance, &source.encoding, &inputs)
        .map_err(NegativePlanProviderError::BoundPreview)?;
    Part10Materializer
        .preview_part10_bytes_with_encoding(&resolved.instance, &source.encoding, max_source_bytes)
        .map_err(NegativePlanProviderError::Preview)
}

fn resolve_builtin_codec_preflight(
    bindings: &ArtifactExecutionBindings,
) -> Result<ArtifactExecutionBindings, NegativePlanProviderError> {
    let mut resolved = bindings.clone();
    for binding in resolved.slots.values_mut() {
        let SlotExecutionBinding::CodecRequest { request } = binding else {
            continue;
        };
        if request.backend_id != NativeRleLosslessEncoder::BACKEND_ID {
            return Err(NegativePlanProviderError::BoundPreview(format!(
                "codec {} is not eligible for deterministic plan preflight",
                request.backend_id
            )));
        }
        if request.source_transfer_syntax_uid != "1.2.840.10008.1.2.1"
            || request.target_transfer_syntax_uid != crate::codecs::RLE_LOSSLESS_TRANSFER_SYNTAX_UID
            || request.frames.is_empty()
            || request.frames.len() > 4_096
        {
            return Err(NegativePlanProviderError::BoundPreview(
                "native RLE preflight request is outside the bounded contract".into(),
            ));
        }
        let bits_stored = request
            .parameters
            .get("bits_stored")
            .and_then(Value::as_u64)
            .and_then(|value| u16::try_from(value).ok());
        let mut frames = request.frames.iter().collect::<Vec<_>>();
        frames.sort_by_key(|frame| frame.frame_number);
        let mut encoded_frames = Vec::with_capacity(frames.len());
        let mut total_encoded = 0usize;
        for (index, frame) in frames.into_iter().enumerate() {
            if frame.frame_number != u32::try_from(index + 1).unwrap_or(u32::MAX) {
                return Err(NegativePlanProviderError::BoundPreview(
                    "native RLE preflight frames are not contiguous".into(),
                ));
            }
            let ByteBinding::Inline { bytes, sha256 } = &frame.bytes else {
                return Err(NegativePlanProviderError::BoundPreview(
                    "native RLE preflight accepts inline frames only".into(),
                ));
            };
            if bytes.len() > 256 * 1024 * 1024 || sha256_hex(bytes) != *sha256 {
                return Err(NegativePlanProviderError::BoundPreview(
                    "native RLE preflight frame exceeds bounds or has hash drift".into(),
                ));
            }
            let rows = u16::try_from(frame.rows).map_err(|_| {
                NegativePlanProviderError::BoundPreview("native RLE rows exceed u16".into())
            })?;
            let columns = u16::try_from(frame.columns).map_err(|_| {
                NegativePlanProviderError::BoundPreview("native RLE columns exceed u16".into())
            })?;
            let encoded = NativeRleLosslessEncoder::new()
                .encode_frame(FrameEncodeInput {
                    native_frame: bytes,
                    rows,
                    columns,
                    samples_per_pixel: frame.samples_per_pixel,
                    bits_allocated: frame.bits_allocated,
                    bits_stored: bits_stored.unwrap_or(frame.bits_allocated),
                    photometric_interpretation: &frame.photometric_interpretation,
                })
                .map_err(|error| NegativePlanProviderError::BoundPreview(error.to_string()))?;
            total_encoded = total_encoded
                .checked_add(encoded.bytes.len())
                .ok_or_else(|| {
                    NegativePlanProviderError::BoundPreview("native RLE size overflow".into())
                })?;
            if encoded.bytes.len() > 256 * 1024 * 1024 || total_encoded > 512 * 1024 * 1024 {
                return Err(NegativePlanProviderError::BoundPreview(
                    "native RLE encoded output exceeds planning bounds".into(),
                ));
            }
            let encoded_size_bytes = encoded.bytes.len() as u64;
            let encoded_sha256 = sha256_hex(&encoded.bytes);
            encoded_frames.push(crate::executor::services::EncodedFrameResult {
                frame_number: frame.frame_number,
                bytes: ByteBinding::Inline {
                    bytes: encoded.bytes,
                    sha256: encoded_sha256.clone(),
                },
                encoded_size_bytes,
                encoded_sha256,
            });
        }
        *binding = SlotExecutionBinding::EncodedFrames {
            frames: encoded_frames,
        };
    }
    Ok(resolved)
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
    Preview(crate::composition::MaterializeError),
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
