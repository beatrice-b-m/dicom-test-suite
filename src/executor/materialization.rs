//! Shared artifact materialization dispatch under a private staging root.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::{Value, json};

use crate::composition::{ContentMaterialization, Part10Materializer};
use crate::corpus_plan::{
    FragmentationPolicy, OffsetTablePolicy, PlannedArtifact, PlannedAuxiliaryArtifact,
    PlannedDicomArtifact, PlannedMutationArtifact, PlannedMutationOperation, PlannedQualification,
    QualificationPayloadPolicy,
};
use crate::encapsulation::{BasicOffsetTablePolicy, EncapsulatedPixelData, encapsulate_frames};
use crate::executor::cancellation::CancellationToken;
use crate::mutation::{
    AcceptableOutcome, ByteRange, FailureLayer, LengthWidth, MutationParameters, MutationRequest,
    TruncationTarget, apply_named_mutation,
};
use crate::{PACKAGE_VERSION, sha256_hex};

use super::services::{
    ArtifactExecutionBindings, AssetDeclaration, AssetVisibility, ByteBinding,
    MaterializationRequest, MaterializationResult, MaterializationService, ProducedAsset,
    ServiceError, ServiceEvidence, SlotExecutionBinding, StagedAssetHandle, StagedAssetRegistry,
    StagingRelativePath, ToolIdentity,
};

pub trait AuxiliaryMaterializationHandler: Send + Sync {
    fn render(
        &self,
        artifact: &PlannedAuxiliaryArtifact,
        bindings: &ArtifactExecutionBindings,
        assets: &StagedAssetRegistry,
    ) -> Result<AuxiliaryPayload, MaterializationError>;
}

#[derive(Debug, Clone, PartialEq)]
pub struct AuxiliaryPayload {
    pub bytes: Vec<u8>,
    pub media_type: String,
    pub backend: ToolIdentity,
    pub evidence: Vec<ServiceEvidence>,
}

#[derive(Clone)]
pub struct MaterializationDispatcher {
    staging_root: PathBuf,
    auxiliary: Arc<dyn AuxiliaryMaterializationHandler>,
}

impl fmt::Debug for MaterializationDispatcher {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MaterializationDispatcher")
            .field("staging_root", &self.staging_root)
            .finish_non_exhaustive()
    }
}

impl MaterializationDispatcher {
    pub fn new(
        staging_root: impl Into<PathBuf>,
        auxiliary: Arc<dyn AuxiliaryMaterializationHandler>,
    ) -> Result<Self, MaterializationError> {
        let staging_root = staging_root.into();
        let metadata =
            fs::symlink_metadata(&staging_root).map_err(|source| MaterializationError::Io {
                path: staging_root.clone(),
                source,
            })?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err(MaterializationError::UnsafeStagingRoot(staging_root));
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&staging_root, fs::Permissions::from_mode(0o700)).map_err(
                |source| MaterializationError::Io {
                    path: staging_root.clone(),
                    source,
                },
            )?;
        }
        Ok(Self {
            staging_root,
            auxiliary,
        })
    }

    pub fn dispatch(
        &self,
        request: &MaterializationRequest,
        assets: &StagedAssetRegistry,
    ) -> Result<MaterializationResult, MaterializationError> {
        self.dispatch_cancellable(request, assets, &CancellationToken::new())
    }

    pub fn dispatch_cancellable(
        &self,
        request: &MaterializationRequest,
        assets: &StagedAssetRegistry,
        cancellation: &CancellationToken,
    ) -> Result<MaterializationResult, MaterializationError> {
        if cancellation.is_cancelled() {
            return Err(MaterializationError::Cancelled);
        }
        request.validate(assets)?;
        let result = match &request.artifact {
            PlannedArtifact::Dicom(artifact) => {
                self.materialize_dicom(artifact, request, assets, cancellation)
            }
            PlannedArtifact::Mutation(artifact) => {
                self.materialize_mutation(artifact, request, assets)
            }
            PlannedArtifact::Qualification(artifact) => {
                self.materialize_qualification(artifact, request)
            }
            PlannedArtifact::Auxiliary(artifact) => {
                self.materialize_auxiliary(artifact, request, assets)
            }
        }?;
        if cancellation.is_cancelled() {
            return Err(MaterializationError::Cancelled);
        }
        Ok(result)
    }

    fn materialize_dicom(
        &self,
        artifact: &PlannedDicomArtifact,
        request: &MaterializationRequest,
        assets: &StagedAssetRegistry,
        cancellation: &CancellationToken,
    ) -> Result<MaterializationResult, MaterializationError> {
        validate_dicom_encoding_bindings(artifact, &request.bindings)?;
        let mut instance = artifact.instance.clone();
        let mut materialized_content = Vec::new();
        let mut extended_table_values = None;
        for content in &mut instance.content {
            if cancellation.is_cancelled() {
                return Err(MaterializationError::Cancelled);
            }
            let Some(binding) = request.bindings.slots.get(&content.slot) else {
                continue;
            };
            content.materialization = Some(match binding {
                SlotExecutionBinding::StagedAsset { asset } => {
                    materialized_content.push(json!({
                        "slot": content.slot,
                        "kind": content.kind,
                        "vr": content.vr.to_string(),
                        "size_bytes": content.size_bytes,
                        "sha256": content.sha256,
                        "basic_offset_table": [],
                        "compressed_frame_sha256": [],
                        "fragment_count": 0,
                        "compressed_lengths": [],
                        "padded_fragment_lengths": [],
                        "extended_offset_table": [],
                        "extended_offset_table_lengths": [],
                        "writer_materialization": "stream_copy",
                    }));
                    ContentMaterialization::StagedFile(self.asset_path(asset, assets, false)?)
                }
                SlotExecutionBinding::NativeFrames { frames } => {
                    let mut ordered_frames = frames.iter().collect::<Vec<_>>();
                    ordered_frames.sort_by_key(|frame| frame.frame_number);
                    for (index, frame) in ordered_frames.iter().enumerate() {
                        if frame.frame_number != index as u32 + 1 {
                            return Err(MaterializationError::NativeFrameOrder {
                                expected: index as u32 + 1,
                                actual: frame.frame_number,
                            });
                        }
                    }
                    let first = ordered_frames[0];
                    let mut bytes = Vec::new();
                    let mut native_frame_sha256 = Vec::with_capacity(ordered_frames.len());
                    let mut native_frame_lengths = Vec::with_capacity(ordered_frames.len());
                    for frame in &ordered_frames {
                        if frame.rows != first.rows
                            || frame.columns != first.columns
                            || frame.samples_per_pixel != first.samples_per_pixel
                            || frame.bits_allocated != first.bits_allocated
                            || frame.photometric_interpretation != first.photometric_interpretation
                        {
                            return Err(MaterializationError::NativeFrameShape(frame.frame_number));
                        }
                        let frame_bytes = self.read_binding(&frame.bytes, assets)?;
                        native_frame_lengths.push(frame_bytes.len() as u64);
                        native_frame_sha256.push(sha256_hex(&frame_bytes));
                        bytes.extend(frame_bytes);
                    }
                    if bytes.len() as u64 != content.size_bytes {
                        return Err(MaterializationError::NativeContentSize {
                            slot: content.slot.clone(),
                            expected: content.size_bytes,
                            actual: bytes.len() as u64,
                        });
                    }
                    let aggregate_sha256 = sha256_hex(&bytes);
                    if aggregate_sha256 != content.sha256 {
                        return Err(MaterializationError::NativeContentHash {
                            slot: content.slot.clone(),
                            expected: content.sha256.clone(),
                            actual: aggregate_sha256,
                        });
                    }
                    let mut native_bit_packing = Value::Null;
                    let (decoded_frame_sha256, decoded_frame_lengths) = if first.bits_allocated == 1
                    {
                        let values_per_frame = u64::from(first.rows)
                            .checked_mul(u64::from(first.columns))
                            .and_then(|value| value.checked_mul(u64::from(first.samples_per_pixel)))
                            .ok_or(MaterializationError::NativeFrameSizeOverflow)?;
                        let total_values = values_per_frame
                            .checked_mul(ordered_frames.len() as u64)
                            .ok_or(MaterializationError::NativeFrameSizeOverflow)?;
                        let required_bytes = total_values
                            .checked_add(7)
                            .ok_or(MaterializationError::NativeFrameSizeOverflow)?
                            / 8;
                        if required_bytes != bytes.len() as u64 {
                            return Err(MaterializationError::NativeBitPackingSize {
                                expected: required_bytes,
                                actual: bytes.len() as u64,
                            });
                        }
                        let unused_trailing_bits = required_bytes
                            .checked_mul(8)
                            .and_then(|value| value.checked_sub(total_values))
                            .ok_or(MaterializationError::NativeFrameSizeOverflow)?;
                        native_bit_packing = json!({
                            "bit_order": "lsb_first",
                            "continuous_across_frames": true,
                            "stored_values_per_frame": values_per_frame,
                            "total_stored_values": total_values,
                            "packed_size_bytes": required_bytes,
                            "unused_trailing_bits": unused_trailing_bits,
                        });
                        let frame_capacity = usize::try_from(values_per_frame)
                            .map_err(|_| MaterializationError::NativeFrameSizeOverflow)?;
                        let mut hashes = Vec::with_capacity(ordered_frames.len());
                        let mut lengths = Vec::with_capacity(ordered_frames.len());
                        for frame_index in 0..ordered_frames.len() {
                            let start = (frame_index as u64)
                                .checked_mul(values_per_frame)
                                .ok_or(MaterializationError::NativeFrameSizeOverflow)?;
                            let mut decoded = Vec::with_capacity(frame_capacity);
                            for offset in 0..values_per_frame {
                                let bit = start + offset;
                                decoded.push((bytes[(bit / 8) as usize] >> (bit % 8)) & 1);
                            }
                            lengths.push(decoded.len() as u64);
                            hashes.push(sha256_hex(&decoded));
                        }
                        (hashes, lengths)
                    } else {
                        (native_frame_sha256.clone(), native_frame_lengths.clone())
                    };
                    let native_byte_order = if first.bits_allocated > 8 {
                        if artifact.encoding.transfer_syntax_uid == "1.2.840.10008.1.2.2" {
                            json!("big_endian")
                        } else {
                            json!("little_endian")
                        }
                    } else {
                        Value::Null
                    };
                    materialized_content.push(json!({
                        "slot": content.slot,
                        "kind": content.kind,
                        "vr": content.vr.to_string(),
                        "size_bytes": content.size_bytes,
                        "sha256": content.sha256,
                        "basic_offset_table": [],
                        "compressed_frame_sha256": [],
                        "native_frame_sha256": native_frame_sha256,
                        "native_frame_lengths": native_frame_lengths,
                        "decoded_frame_sha256": decoded_frame_sha256,
                        "decoded_frame_lengths": decoded_frame_lengths,
                        "native_byte_order": native_byte_order,
                        "native_bit_packing": native_bit_packing,
                        "fragment_count": 0,
                        "compressed_lengths": [],
                        "padded_fragment_lengths": [],
                        "extended_offset_table": [],
                        "extended_offset_table_lengths": [],
                    }));
                    ContentMaterialization::Inline(bytes)
                }
                SlotExecutionBinding::EncodedFrames { frames } => {
                    let mut ordered_frames = frames.iter().collect::<Vec<_>>();
                    ordered_frames.sort_by_key(|frame| frame.frame_number);
                    for (index, frame) in ordered_frames.iter().enumerate() {
                        if frame.frame_number != index as u32 + 1 {
                            return Err(MaterializationError::EncodedFrameOrder);
                        }
                    }
                    let encoded_frames = ordered_frames
                        .iter()
                        .map(|frame| {
                            let bytes = self.read_binding(&frame.bytes, assets)?;
                            if bytes.len() as u64 != frame.encoded_size_bytes
                                || sha256_hex(&bytes) != frame.encoded_sha256
                            {
                                return Err(MaterializationError::EncodedFrameIdentity(
                                    frame.frame_number,
                                ));
                            }
                            Ok(bytes)
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    let policy = match artifact.encoding.offset_table {
                        OffsetTablePolicy::PopulatedBasic => BasicOffsetTablePolicy::Populated,
                        OffsetTablePolicy::EmptyBasic | OffsetTablePolicy::Extended => {
                            BasicOffsetTablePolicy::Empty
                        }
                        OffsetTablePolicy::NotApplicable => unreachable!("prevalidated"),
                    };
                    let encapsulated = match artifact.encoding.fragmentation {
                        FragmentationPolicy::OneFragmentPerFrame
                        | FragmentationPolicy::PreserveEncodedFrames => {
                            if artifact.encoding.offset_table == OffsetTablePolicy::Extended {
                                EncapsulatedPixelData::one_fragment_per_frame_with_extended_offset_table(
                                    &encoded_frames,
                                )
                            } else {
                                EncapsulatedPixelData::one_fragment_per_frame(&encoded_frames, policy)
                            }
                        }
                        FragmentationPolicy::FixedMaximumBytes { maximum_bytes } => {
                            let maximum = usize::try_from(maximum_bytes)
                                .map_err(|_| MaterializationError::FragmentMaximumRange)?;
                            let even_maximum = maximum & !1;
                            let fragments_per_frame = encoded_frames
                                .iter()
                                .map(|frame| {
                                    if frame.len() <= maximum {
                                        return Ok(1);
                                    }
                                    if even_maximum == 0 {
                                        return Err(MaterializationError::FragmentMaximumTooSmall);
                                    }
                                    frame.len()
                                        .checked_add(even_maximum - 1)
                                        .map(|value| value / even_maximum)
                                        .ok_or(MaterializationError::FragmentMaximumRange)
                                })
                                .collect::<Result<Vec<_>, _>>()?;
                            encapsulate_frames(&encoded_frames, &fragments_per_frame, policy)
                        }
                        FragmentationPolicy::Native => unreachable!("prevalidated"),
                    }
                    .map_err(MaterializationError::Encapsulation)?;
                    let basic_offset_table = encapsulated.basic_offset_table.offsets.clone();
                    let compressed_frame_sha256 = encapsulated.compressed_frame_hashes.clone();
                    let fragment_count = encapsulated.fragments.len() as u64;
                    let compressed_lengths = encapsulated
                        .fragments
                        .iter()
                        .map(|fragment| fragment.compressed_length as u64)
                        .collect::<Vec<_>>();
                    let padded_fragment_lengths = encapsulated
                        .fragments
                        .iter()
                        .map(|fragment| fragment.padded_length as u64)
                        .collect::<Vec<_>>();
                    let fragments_per_frame = encapsulated
                        .fragments_per_frame
                        .iter()
                        .map(|count| *count as u64)
                        .collect::<Vec<_>>();
                    let fragment_evidence = encapsulated
                        .fragments
                        .iter()
                        .map(|fragment| {
                            json!({
                                "frame_index": fragment.frame_index as u64,
                                "item_start_offset": fragment.item_start_offset as u64,
                                "compressed_length": fragment.compressed_length as u64,
                                "padded_length": fragment.padded_length as u64,
                            })
                        })
                        .collect::<Vec<_>>();
                    let (extended_offset_table, extended_offset_table_lengths) = encapsulated
                        .extended_offset_table
                        .as_ref()
                        .map(|table| (table.offsets.clone(), table.lengths.clone()))
                        .unwrap_or_default();
                    if let Some(table) = &encapsulated.extended_offset_table {
                        extended_table_values = Some((
                            table.offset_value_bytes.clone(),
                            table.length_value_bytes.clone(),
                        ));
                    }
                    let mut fragments = encapsulated.fragment_payloads;
                    for fragment in &mut fragments {
                        if fragment.len() % 2 != 0 {
                            fragment.push(0);
                        }
                    }
                    let aggregate = fragments.concat();
                    content.kind = "encapsulated_pixels".into();
                    content.vr = crate::composition::DicomVr::OB;
                    content.size_bytes = aggregate.len() as u64;
                    content.sha256 = sha256_hex(&aggregate);
                    content.properties.insert(
                        "compressed_frame_sha256".into(),
                        serde_json::to_string(&compressed_frame_sha256)
                            .expect("frame hashes serialize"),
                    );
                    materialized_content.push(json!({
                        "slot": content.slot,
                        "kind": content.kind,
                        "vr": content.vr.to_string(),
                        "size_bytes": content.size_bytes,
                        "sha256": content.sha256,
                        "basic_offset_table": basic_offset_table,
                        "compressed_frame_sha256": compressed_frame_sha256,
                        "fragment_count": fragment_count,
                        "compressed_lengths": compressed_lengths,
                        "padded_fragment_lengths": padded_fragment_lengths,
                        "fragments_per_frame": fragments_per_frame,
                        "fragments": fragment_evidence,
                        "extended_offset_table": extended_offset_table,
                        "extended_offset_table_lengths": extended_offset_table_lengths,
                        "writer_materialization": null,
                    }));
                    ContentMaterialization::Encapsulated {
                        basic_offset_table,
                        fragments,
                    }
                }
                SlotExecutionBinding::ProviderRequest { .. }
                | SlotExecutionBinding::CodecRequest { .. }
                | SlotExecutionBinding::ProviderCodecPipeline { .. } => {
                    return Err(MaterializationError::UnresolvedProviderBinding {
                        artifact_id: artifact.logical_id.clone(),
                        slot: content.slot.clone(),
                    });
                }
            });
        }
        if let Some((offsets, lengths)) = extended_table_values {
            upsert_binary_attribute(
                &mut instance,
                "7FE0,0001",
                crate::composition::DicomVr::OV,
                offsets,
            )?;
            upsert_binary_attribute(
                &mut instance,
                "7FE0,0002",
                crate::composition::DicomVr::OV,
                lengths,
            )?;
        }
        let relative_path = artifact.output.relative_path.as_str();
        let path = self.output_path(relative_path)?;
        Part10Materializer.materialize_with_encoding_cancellable(
            &instance,
            &artifact.encoding,
            &path,
            &|| cancellation.is_cancelled(),
        )?;
        let materialized_instance_plan_sha256 = instance.canonical_sha256();
        let output = self.produced_file(
            &artifact.logical_id,
            relative_path,
            "application/dicom",
            artifact.output.publish,
        )?;
        let artifact_bytes = fs::read(&path).map_err(|source| MaterializationError::Io {
            path: path.clone(),
            source,
        })?;
        let meta_end = standard_file_meta_end(&artifact_bytes)?;
        let preamble_sha256 = sha256_hex(&artifact_bytes[..128]);
        let file_meta_sha256 = sha256_hex(&artifact_bytes[132..meta_end]);
        let file_meta_size_bytes = (meta_end - 132) as u64;
        let materialized_encoding_sha256 = sha256_hex(
            &serde_json::to_vec(&json!({
                "encoding": artifact.encoding,
                "content": materialized_content,
                "preamble_sha256": preamble_sha256,
                "file_meta_sha256": file_meta_sha256,
            }))
            .expect("materialized encoding evidence serializes"),
        );
        let materialized_artifact_sha256 = output.observed_sha256.clone();
        Ok(MaterializationResult {
            artifact_id: artifact.logical_id.clone(),
            output: Some(output),
            backend: built_in_identity("part10_materializer"),
            evidence: vec![ServiceEvidence {
                evidence_id: format!("materialized_plan:{}", artifact.logical_id),
                evidence_kind: "materialized_instance_plan".into(),
                producer: built_in_identity("part10_materializer"),
                claims: BTreeMap::from([
                    (
                        "materialized_instance_plan_sha256".into(),
                        json!(materialized_instance_plan_sha256),
                    ),
                    (
                        "materialized_content".into(),
                        Value::Array(materialized_content),
                    ),
                    (
                        "materialized_encoding_sha256".into(),
                        json!(materialized_encoding_sha256),
                    ),
                    (
                        "materialized_artifact_sha256".into(),
                        json!(materialized_artifact_sha256),
                    ),
                    ("preamble_policy".into(), json!(artifact.encoding.preamble)),
                    ("preamble_sha256".into(), json!(preamble_sha256)),
                    (
                        "file_meta_policy".into(),
                        json!(artifact.encoding.file_meta),
                    ),
                    ("file_meta_sha256".into(), json!(file_meta_sha256)),
                    ("file_meta_size_bytes".into(), json!(file_meta_size_bytes)),
                    (
                        "implementation_class_uid".into(),
                        json!(artifact.encoding.implementation.class_uid),
                    ),
                    (
                        "implementation_version_name".into(),
                        json!(artifact.encoding.implementation.version_name),
                    ),
                ]),
            }],
        })
    }

    fn materialize_mutation(
        &self,
        artifact: &PlannedMutationArtifact,
        request: &MaterializationRequest,
        assets: &StagedAssetRegistry,
    ) -> Result<MaterializationResult, MaterializationError> {
        if artifact.mutation.contract_version != crate::mutation::MUTATION_CONTRACT_VERSION {
            return Err(MaterializationError::UnsupportedMutationContract(
                artifact.mutation.contract_version.clone(),
            ));
        }
        let source_handle = match request.bindings.slots.get("source") {
            Some(SlotExecutionBinding::StagedAsset { asset }) => asset,
            _ => {
                return Err(MaterializationError::MissingPrivateMutationSource(
                    artifact.logical_id.clone(),
                ));
            }
        };
        let expected_source_handle = format!("output:{}", artifact.source_artifact_id);
        if source_handle.as_str() != expected_source_handle {
            return Err(MaterializationError::MutationSourceIdentity {
                expected: expected_source_handle,
                actual: source_handle.to_string(),
            });
        }
        let source = self.read_asset(source_handle, assets, true)?;
        let source_sha256 = sha256_hex(&source);
        if source_sha256 != artifact.mutation.expected_source_sha256 {
            return Err(MaterializationError::MutationSourceHash {
                expected: artifact.mutation.expected_source_sha256.clone(),
                actual: source_sha256,
            });
        }
        let outcomes = artifact
            .mutation
            .acceptable_outcomes
            .iter()
            .map(|value| parse_outcome(value))
            .collect::<Result<Vec<_>, _>>()?;
        let mut bytes = source;
        let mut steps = Vec::new();
        for (index, operation) in artifact.mutation.operations.iter().enumerate() {
            let parameters = mutation_parameters(operation)?;
            let layer_name = artifact
                .mutation
                .expected_failure_layers
                .get(index)
                .or_else(|| artifact.mutation.expected_failure_layers.first())
                .expect("validated mutation plan has a failure layer");
            let result = apply_named_mutation(
                &bytes,
                MutationRequest::new(
                    parameters,
                    parse_failure_layer(layer_name)?,
                    outcomes.clone(),
                ),
            )?;
            steps.push(json!({
                "operation_id": result.mutation_id,
                "source_sha256": result.source_sha256,
                "output_sha256": result.output_sha256,
            }));
            bytes = result.bytes;
        }
        let actual = sha256_hex(&bytes);
        if actual != artifact.mutation.expected_output_sha256 {
            return Err(MaterializationError::MutationOutputHash {
                expected: artifact.mutation.expected_output_sha256.clone(),
                actual,
            });
        }
        let relative_path = artifact.output.relative_path.as_str();
        let path = self.output_path(relative_path)?;
        write_new(&path, &bytes)?;
        let output = self.produced_file(
            &artifact.logical_id,
            relative_path,
            "application/dicom",
            artifact.output.publish,
        )?;
        Ok(MaterializationResult {
            artifact_id: artifact.logical_id.clone(),
            output: Some(output),
            backend: built_in_identity("named_mutation"),
            evidence: vec![ServiceEvidence {
                evidence_id: "ordered_mutation_steps".into(),
                evidence_kind: "mutation".into(),
                producer: built_in_identity("named_mutation"),
                claims: BTreeMap::from([("steps".into(), Value::Array(steps))]),
            }],
        })
    }

    fn materialize_qualification(
        &self,
        artifact: &PlannedQualification,
        request: &MaterializationRequest,
    ) -> Result<MaterializationResult, MaterializationError> {
        if !request.bindings.slots.is_empty() {
            return Err(MaterializationError::QualificationPayloadForbidden(
                artifact.logical_id.clone(),
            ));
        }
        if artifact.payload_policy == QualificationPayloadPolicy::EvidenceOnly
            && artifact.evidence.obligations.is_empty()
        {
            return Err(MaterializationError::MissingQualificationEvidence(
                artifact.logical_id.clone(),
            ));
        }
        Ok(MaterializationResult {
            artifact_id: artifact.logical_id.clone(),
            output: None,
            backend: built_in_identity("qualification_dispatch"),
            evidence: artifact
                .evidence
                .obligations
                .iter()
                .map(|obligation| ServiceEvidence {
                    evidence_id: obligation.obligation_id.clone(),
                    evidence_kind: artifact.qualification_kind.clone(),
                    producer: built_in_identity("qualification_dispatch"),
                    claims: obligation.parameters.clone(),
                })
                .collect(),
        })
    }

    fn materialize_auxiliary(
        &self,
        artifact: &PlannedAuxiliaryArtifact,
        request: &MaterializationRequest,
        assets: &StagedAssetRegistry,
    ) -> Result<MaterializationResult, MaterializationError> {
        let payload = self.auxiliary.render(artifact, &request.bindings, assets)?;
        payload.backend.validate()?;
        let relative_path = artifact.output.relative_path.as_str();
        let path = self.output_path(relative_path)?;
        write_new(&path, &payload.bytes)?;
        let output = self.produced_file(
            &artifact.logical_id,
            relative_path,
            &payload.media_type,
            artifact.output.publish,
        )?;
        Ok(MaterializationResult {
            artifact_id: artifact.logical_id.clone(),
            output: Some(output),
            backend: payload.backend,
            evidence: payload.evidence,
        })
    }

    fn output_path(&self, relative_path: &str) -> Result<PathBuf, MaterializationError> {
        let relative = StagingRelativePath::new(relative_path)?;
        let path = self.staging_root.join(relative.as_str());
        ensure_safe_parent(&self.staging_root, &path)?;
        Ok(path)
    }

    fn asset_path(
        &self,
        handle: &StagedAssetHandle,
        assets: &StagedAssetRegistry,
        require_private: bool,
    ) -> Result<PathBuf, MaterializationError> {
        let declaration = assets.resolve(handle)?;
        if require_private && declaration.visibility != AssetVisibility::Private {
            return Err(MaterializationError::MutationSourceNotPrivate(
                handle.clone(),
            ));
        }
        let path = self.staging_root.join(declaration.relative_path.as_str());
        ensure_safe_parent(&self.staging_root, &path)?;
        Ok(path)
    }

    fn read_asset(
        &self,
        handle: &StagedAssetHandle,
        assets: &StagedAssetRegistry,
        require_private: bool,
    ) -> Result<Vec<u8>, MaterializationError> {
        let declaration = assets.resolve(handle)?;
        let path = self.asset_path(handle, assets, require_private)?;
        let bytes = fs::read(&path).map_err(|source| MaterializationError::Io {
            path: path.clone(),
            source,
        })?;
        let actual_hash = sha256_hex(&bytes);
        if bytes.len() as u64 != declaration.size_bytes || actual_hash != declaration.sha256 {
            return Err(MaterializationError::StagedAssetChanged(handle.clone()));
        }
        Ok(bytes)
    }

    fn read_binding(
        &self,
        binding: &ByteBinding,
        assets: &StagedAssetRegistry,
    ) -> Result<Vec<u8>, MaterializationError> {
        match binding {
            ByteBinding::Inline { bytes, sha256 } => {
                if sha256_hex(bytes) != *sha256 {
                    return Err(MaterializationError::InlineBindingChanged);
                }
                Ok(bytes.clone())
            }
            ByteBinding::StagedRange {
                asset,
                offset,
                length,
                sha256,
            } => {
                let bytes = self.read_asset(asset, assets, false)?;
                let start =
                    usize::try_from(*offset).map_err(|_| MaterializationError::BindingRange)?;
                let length =
                    usize::try_from(*length).map_err(|_| MaterializationError::BindingRange)?;
                let end = start
                    .checked_add(length)
                    .ok_or(MaterializationError::BindingRange)?;
                let selected = bytes
                    .get(start..end)
                    .ok_or(MaterializationError::BindingRange)?
                    .to_vec();
                if sha256_hex(&selected) != *sha256 {
                    return Err(MaterializationError::InlineBindingChanged);
                }
                Ok(selected)
            }
            ByteBinding::VerifiedAssetRange {
                asset,
                offset,
                length,
            } => {
                let bytes = self.read_asset(asset, assets, false)?;
                let start =
                    usize::try_from(*offset).map_err(|_| MaterializationError::BindingRange)?;
                let length =
                    usize::try_from(*length).map_err(|_| MaterializationError::BindingRange)?;
                let end = start
                    .checked_add(length)
                    .ok_or(MaterializationError::BindingRange)?;
                Ok(bytes
                    .get(start..end)
                    .ok_or(MaterializationError::BindingRange)?
                    .to_vec())
            }
        }
    }

    fn produced_file(
        &self,
        artifact_id: &str,
        relative_path: &str,
        media_type: &str,
        publish: bool,
    ) -> Result<ProducedAsset, MaterializationError> {
        let path = self.output_path(relative_path)?;
        let bytes = fs::read(&path).map_err(|source| MaterializationError::Io { path, source })?;
        let sha256 = sha256_hex(&bytes);
        Ok(ProducedAsset {
            declaration: AssetDeclaration {
                handle: StagedAssetHandle::new(format!("output:{artifact_id}"))?,
                relative_path: StagingRelativePath::new(relative_path)?,
                size_bytes: bytes.len() as u64,
                sha256: sha256.clone(),
                media_type: media_type.into(),
                visibility: if publish {
                    AssetVisibility::PublicationCandidate
                } else {
                    AssetVisibility::Private
                },
            },
            observed_size_bytes: bytes.len() as u64,
            observed_sha256: sha256,
        })
    }
}

impl MaterializationService for MaterializationDispatcher {
    fn materialize(
        &self,
        request: &MaterializationRequest,
        assets: &StagedAssetRegistry,
    ) -> Result<MaterializationResult, ServiceError> {
        self.dispatch(request, assets)
            .map_err(|error| ServiceError::BackendFailure {
                backend_id: "materialization_dispatch".into(),
                operation: request.artifact.logical_id().into(),
                message: error.to_string(),
            })
    }
}

fn ensure_safe_parent(root: &Path, path: &Path) -> Result<(), MaterializationError> {
    let parent = path
        .parent()
        .ok_or_else(|| MaterializationError::UnsafeOutputPath(path.to_path_buf()))?;
    let relative = parent
        .strip_prefix(root)
        .map_err(|_| MaterializationError::UnsafeOutputPath(path.to_path_buf()))?;
    let mut current = root.to_path_buf();
    for component in relative.components() {
        current.push(component);
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
                return Err(MaterializationError::UnsafeOutputPath(path.to_path_buf()));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                match fs::create_dir(&current) {
                    Ok(()) => {}
                    Err(source) if source.kind() == std::io::ErrorKind::AlreadyExists => {
                        let metadata = fs::symlink_metadata(&current).map_err(|source| {
                            MaterializationError::Io {
                                path: current.clone(),
                                source,
                            }
                        })?;
                        if metadata.file_type().is_symlink() || !metadata.is_dir() {
                            return Err(MaterializationError::UnsafeOutputPath(path.to_path_buf()));
                        }
                    }
                    Err(source) => {
                        return Err(MaterializationError::Io {
                            path: current.clone(),
                            source,
                        });
                    }
                }
            }
            Err(source) => {
                return Err(MaterializationError::Io {
                    path: current,
                    source,
                });
            }
        }
    }
    Ok(())
}

fn standard_file_meta_end(bytes: &[u8]) -> Result<usize, MaterializationError> {
    if bytes.get(128..132) != Some(b"DICM") {
        return Err(MaterializationError::InvalidPart10FileMeta);
    }
    let mut offset = 132usize;
    while bytes.get(offset..offset + 2) == Some(&[0x02, 0x00]) {
        let vr = bytes
            .get(offset + 4..offset + 6)
            .ok_or(MaterializationError::InvalidPart10FileMeta)?;
        let long = matches!(
            vr,
            b"OB" | b"OD" | b"OF" | b"OL" | b"OV" | b"OW" | b"SQ" | b"UC" | b"UR" | b"UT" | b"UN"
        );
        let (header, length) = if long {
            (
                12usize,
                u32::from_le_bytes(
                    bytes
                        .get(offset + 8..offset + 12)
                        .and_then(|value| value.try_into().ok())
                        .ok_or(MaterializationError::InvalidPart10FileMeta)?,
                ) as usize,
            )
        } else {
            (
                8usize,
                u16::from_le_bytes(
                    bytes
                        .get(offset + 6..offset + 8)
                        .and_then(|value| value.try_into().ok())
                        .ok_or(MaterializationError::InvalidPart10FileMeta)?,
                ) as usize,
            )
        };
        offset = offset
            .checked_add(header)
            .and_then(|value| value.checked_add(length))
            .ok_or(MaterializationError::InvalidPart10FileMeta)?;
        if offset > bytes.len() {
            return Err(MaterializationError::InvalidPart10FileMeta);
        }
    }
    if offset == 132 {
        return Err(MaterializationError::InvalidPart10FileMeta);
    }
    Ok(offset)
}

fn write_new(path: &Path, bytes: &[u8]) -> Result<(), MaterializationError> {
    use std::io::Write;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|source| MaterializationError::Io {
            path: path.to_path_buf(),
            source,
        })?;
    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .map_err(|source| MaterializationError::Io {
            path: path.to_path_buf(),
            source,
        })
}

fn built_in_identity(backend_id: &str) -> ToolIdentity {
    ToolIdentity {
        backend_id: backend_id.into(),
        version: PACKAGE_VERSION.into(),
        protocol_version: Some("0.1.0".into()),
        executable_sha256: None,
    }
}

fn mutation_parameters(
    operation: &PlannedMutationOperation,
) -> Result<MutationParameters, MaterializationError> {
    let range = |index: usize| -> Result<ByteRange, MaterializationError> {
        let range = operation.source_ranges.get(index).ok_or_else(|| {
            MaterializationError::MutationParameters(operation.operation_id.clone())
        })?;
        Ok(ByteRange::new(
            usize::try_from(range.start).map_err(|_| {
                MaterializationError::MutationParameters(operation.operation_id.clone())
            })?,
            usize::try_from(range.end).map_err(|_| {
                MaterializationError::MutationParameters(operation.operation_id.clone())
            })?,
        ))
    };
    let value = |name: &str| {
        operation.parameters.get(name).ok_or_else(|| {
            MaterializationError::MutationParameters(format!("{}:{name}", operation.operation_id))
        })
    };
    let u64_value = |name: &str| {
        value(name)?.as_u64().ok_or_else(|| {
            MaterializationError::MutationParameters(format!("{}:{name}", operation.operation_id))
        })
    };
    let bytes_value = |name: &str| -> Result<Vec<u8>, MaterializationError> {
        match value(name)? {
            Value::String(value) => Ok(value.as_bytes().to_vec()),
            Value::Array(values) => values
                .iter()
                .map(|value| {
                    value
                        .as_u64()
                        .and_then(|value| u8::try_from(value).ok())
                        .ok_or_else(|| {
                            MaterializationError::MutationParameters(format!(
                                "{}:{name}",
                                operation.operation_id
                            ))
                        })
                })
                .collect(),
            _ => Err(MaterializationError::MutationParameters(format!(
                "{}:{name}",
                operation.operation_id
            ))),
        }
    };
    let width = || -> Result<LengthWidth, MaterializationError> {
        match value("width")?.as_str() {
            Some("u16") => Ok(LengthWidth::U16),
            Some("u32") => Ok(LengthWidth::U32),
            Some("u64") => Ok(LengthWidth::U64),
            _ => Err(MaterializationError::MutationParameters(
                operation.operation_id.clone(),
            )),
        }
    };
    Ok(match operation.operation_id.as_str() {
        id @ ("truncate_file_meta"
        | "truncate_dataset"
        | "truncate_sequence"
        | "truncate_item"
        | "truncate_fragment"
        | "truncate_pixel_value") => {
            let target = match id {
                "truncate_file_meta" => TruncationTarget::FileMeta,
                "truncate_dataset" => TruncationTarget::Dataset,
                "truncate_sequence" => TruncationTarget::Sequence,
                "truncate_item" => TruncationTarget::Item,
                "truncate_fragment" => TruncationTarget::Fragment,
                _ => TruncationTarget::PixelValue,
            };
            MutationParameters::Truncate {
                target,
                offset: range(0)?.start,
            }
        }
        "incorrect_explicit_vr_length" => MutationParameters::IncorrectExplicitVrLength {
            length_field: range(0)?,
            width: width()?,
            declared_length: u64_value("declared_length")?,
        },
        "illegal_vr_bytes" => MutationParameters::IllegalVr {
            vr_field: range(0)?,
            replacement: bytes_value("replacement")?.try_into().map_err(|_| {
                MaterializationError::MutationParameters(operation.operation_id.clone())
            })?,
        },
        "transfer_syntax_mismatch" => MutationParameters::TransferSyntaxMismatch {
            file_meta_uid_value: range(0)?,
            replacement: bytes_value("replacement")?,
        },
        "file_meta_dataset_uid_mismatch" => MutationParameters::UidMismatch {
            dataset_uid_value: range(0)?,
            replacement: bytes_value("replacement")?,
        },
        "missing_type_1_element" => MutationParameters::MissingType1Element { element: range(0)? },
        "invalid_bits_stored_high_bit" => MutationParameters::InvalidBitsStoredHighBit {
            bits_stored_value: range(0)?,
            high_bit_value: range(1)?,
            bits_stored: u16::try_from(u64_value("bits_stored")?).map_err(|_| {
                MaterializationError::MutationParameters(operation.operation_id.clone())
            })?,
            high_bit: u16::try_from(u64_value("high_bit")?).map_err(|_| {
                MaterializationError::MutationParameters(operation.operation_id.clone())
            })?,
        },
        "invalid_pixel_byte_length" => MutationParameters::InvalidPixelByteLength {
            length_field: range(0)?,
            width: width()?,
            declared_length: u64_value("declared_length")?,
        },
        "broken_basic_offset_table" => MutationParameters::BrokenBasicOffsetTable {
            entry: range(0)?,
            offset: u32::try_from(u64_value("offset")?).map_err(|_| {
                MaterializationError::MutationParameters(operation.operation_id.clone())
            })?,
        },
        "broken_extended_offset_table" => MutationParameters::BrokenExtendedOffsetTable {
            entry: range(0)?,
            offset: u64_value("offset")?,
        },
        "undefined_length_without_delimitation" => {
            MutationParameters::UndefinedLengthWithoutDelimitation {
                length_field: (operation.source_ranges.len() > 1)
                    .then(|| range(0))
                    .transpose()?,
                delimitation_item: range(operation.source_ranges.len().saturating_sub(1))?,
            }
        }
        "invalid_nested_item_length" => MutationParameters::InvalidNestedItemLength {
            length_field: range(0)?,
            declared_length: u32::try_from(u64_value("declared_length")?).map_err(|_| {
                MaterializationError::MutationParameters(operation.operation_id.clone())
            })?,
        },
        "invalid_character_set_declaration" => MutationParameters::InvalidCharacterSetDeclaration {
            value: range(0)?,
            replacement: bytes_value("replacement")?,
        },
        "malformed_encoded_text" => MutationParameters::MalformedEncodedText {
            value: range(0)?,
            replacement: bytes_value("replacement")?,
        },
        _ => {
            return Err(MaterializationError::UnsupportedMutation(
                operation.operation_id.clone(),
            ));
        }
    })
}

fn parse_failure_layer(value: &str) -> Result<FailureLayer, MaterializationError> {
    match value {
        "file_meta" => Ok(FailureLayer::FileMeta),
        "dataset_parser" => Ok(FailureLayer::DatasetParser),
        "value_decoding" => Ok(FailureLayer::ValueDecoding),
        "semantic_validation" => Ok(FailureLayer::SemanticValidation),
        "pixel_decoding" => Ok(FailureLayer::PixelDecoding),
        "encapsulation" => Ok(FailureLayer::Encapsulation),
        "text_decoding" => Ok(FailureLayer::TextDecoding),
        _ => Err(MaterializationError::MutationParameters(value.into())),
    }
}

fn parse_outcome(value: &str) -> Result<AcceptableOutcome, MaterializationError> {
    match value {
        "clean_rejection" => Ok(AcceptableOutcome::CleanRejection),
        "parse_failure" => Ok(AcceptableOutcome::ParseFailure),
        "validation_failure" => Ok(AcceptableOutcome::ValidationFailure),
        "decode_failure" => Ok(AcceptableOutcome::DecodeFailure),
        "accepted_with_bounded_warning" => Ok(AcceptableOutcome::AcceptedWithBoundedWarning),
        _ => Err(MaterializationError::MutationParameters(value.into())),
    }
}

fn validate_dicom_encoding_bindings(
    artifact: &PlannedDicomArtifact,
    bindings: &ArtifactExecutionBindings,
) -> Result<(), MaterializationError> {
    artifact
        .encoding
        .validate()
        .map_err(|error| MaterializationError::EncodingPolicy(error.to_string()))?;
    for content in &artifact.instance.content {
        let Some(ContentMaterialization::Encapsulated {
            basic_offset_table, ..
        }) = &content.materialization
        else {
            continue;
        };
        let matches_table = match artifact.encoding.offset_table {
            OffsetTablePolicy::EmptyBasic => basic_offset_table.is_empty(),
            OffsetTablePolicy::PopulatedBasic => !basic_offset_table.is_empty(),
            OffsetTablePolicy::Extended | OffsetTablePolicy::NotApplicable => false,
        };
        if !matches_table {
            return Err(MaterializationError::EncodingBindingMismatch {
                artifact_id: artifact.logical_id.clone(),
                slot: content.slot.clone(),
            });
        }
    }
    for (slot, binding) in &bindings.slots {
        let is_pixel_data = artifact.instance.content.iter().any(|content| {
            content.slot == *slot && content.address.normalized_tag() == "7FE0,0010"
        });
        if !is_pixel_data {
            continue;
        }
        let encoded = matches!(binding, SlotExecutionBinding::EncodedFrames { .. });
        if encoded != !matches!(artifact.encoding.fragmentation, FragmentationPolicy::Native) {
            return Err(MaterializationError::EncodingBindingMismatch {
                artifact_id: artifact.logical_id.clone(),
                slot: slot.clone(),
            });
        }
    }
    Ok(())
}

fn upsert_binary_attribute(
    instance: &mut crate::composition::ResolvedInstancePlan,
    tag: &str,
    vr: crate::composition::DicomVr,
    bytes: Vec<u8>,
) -> Result<(), MaterializationError> {
    let address = crate::composition::AttributeAddress::from_normalized_tag(tag)
        .map_err(|error| MaterializationError::EncodingPolicy(error.to_string()))?;
    if instance
        .content
        .iter()
        .any(|content| content.address == address)
    {
        return Err(MaterializationError::EncodingPolicy(format!(
            "encoding-owned attribute {tag} conflicts with canonical content"
        )));
    }
    let attribute = crate::composition::ResolvedAttribute {
        address: address.clone(),
        vr,
        value: Some(crate::composition::AttributeValue::Binary(bytes)),
        origin: crate::composition::ValueOrigin::InstanceOverride,
    };
    if let Some(existing) = instance
        .attributes
        .iter_mut()
        .find(|existing| existing.address == address)
    {
        *existing = attribute;
    } else {
        instance.attributes.push(attribute);
        instance
            .attributes
            .sort_by(|left, right| left.address.cmp(&right.address));
    }
    Ok(())
}

#[derive(Debug)]
pub enum MaterializationError {
    Cancelled,
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    UnsafeStagingRoot(PathBuf),
    UnsafeOutputPath(PathBuf),
    Service(ServiceError),
    Dicom(crate::composition::MaterializeError),
    Mutation(crate::mutation::MutationError),
    Encapsulation(crate::encapsulation::EncapsulationError),
    UnresolvedProviderBinding {
        artifact_id: String,
        slot: String,
    },
    EncodingPolicy(String),
    EncodingBindingMismatch {
        artifact_id: String,
        slot: String,
    },
    FragmentMaximumRange,
    FragmentMaximumTooSmall,
    EncodedFrameOrder,
    EncodedFrameIdentity(u32),
    NativeFrameOrder {
        expected: u32,
        actual: u32,
    },
    NativeFrameShape(u32),
    NativeFrameSizeOverflow,
    NativeBitPackingSize {
        expected: u64,
        actual: u64,
    },
    NativeContentSize {
        slot: String,
        expected: u64,
        actual: u64,
    },
    NativeContentHash {
        slot: String,
        expected: String,
        actual: String,
    },
    InvalidPart10FileMeta,
    MissingPrivateMutationSource(String),
    MutationSourceNotPrivate(StagedAssetHandle),
    StagedAssetChanged(StagedAssetHandle),
    InlineBindingChanged,
    BindingRange,
    MutationSourceHash {
        expected: String,
        actual: String,
    },
    MutationSourceIdentity {
        expected: String,
        actual: String,
    },
    MutationOutputHash {
        expected: String,
        actual: String,
    },
    MutationParameters(String),
    UnsupportedMutation(String),
    UnsupportedMutationContract(String),
    QualificationPayloadForbidden(String),
    MissingQualificationEvidence(String),
    Auxiliary(String),
}

impl fmt::Display for MaterializationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl Error for MaterializationError {}

impl From<ServiceError> for MaterializationError {
    fn from(value: ServiceError) -> Self {
        Self::Service(value)
    }
}

impl From<crate::composition::MaterializeError> for MaterializationError {
    fn from(value: crate::composition::MaterializeError) -> Self {
        Self::Dicom(value)
    }
}

impl From<crate::mutation::MutationError> for MaterializationError {
    fn from(value: crate::mutation::MutationError) -> Self {
        Self::Mutation(value)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;
    use crate::composition::{
        CompositionUidRole, IdentityPlan, ResolvedInstancePlan, TemplateId, TemplateVersion,
    };
    use crate::corpus_plan::{
        ArtifactProvenance, ArtifactResourceEstimate, EncodingPlan, EvidenceIndependence,
        EvidenceObligation, EvidencePlan, FileMetaPolicy, FragmentationPolicy,
        ImplementationIdentityPlan, ItemLengthPolicy, MutationPlan, OffsetTablePolicy, OutputPlan,
        OutputRelativePath, PlannedByteRange, PlannedMutationOperation, PreamblePolicy,
        SequenceLengthPolicy, ValidationPlan, ValidationRequirement, ValidationRule,
    };

    static NEXT: AtomicU64 = AtomicU64::new(0);

    fn root(label: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "dts-materialization-{label}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        root
    }

    fn validation() -> ValidationPlan {
        ValidationPlan {
            rules: vec![ValidationRule {
                rule_id: "part10".into(),
                requirement: ValidationRequirement::Required,
                parameters: BTreeMap::new(),
            }],
        }
    }

    fn evidence() -> EvidencePlan {
        EvidencePlan {
            obligations: vec![EvidenceObligation {
                obligation_id: "same_project".into(),
                route_id: "unit_test".into(),
                independence: EvidenceIndependence::SameProject,
                required: true,
                parameters: BTreeMap::new(),
            }],
        }
    }

    fn output(path: &str, publish: bool) -> OutputPlan {
        OutputPlan {
            relative_path: OutputRelativePath::new(path).unwrap(),
            role: "test".into(),
            publish,
        }
    }

    fn dicom(id: &str, path: &str, publish: bool) -> PlannedArtifact {
        let identities = IdentityPlan::from_exact_values(
            id,
            [
                (CompositionUidRole::SopInstance, 0, "2.25.1001".into()),
                (
                    CompositionUidRole::ImplementationClass,
                    0,
                    "2.25.1002".into(),
                ),
            ],
        )
        .unwrap();
        PlannedArtifact::Dicom(PlannedDicomArtifact {
            logical_id: id.into(),
            order: 0,
            provenance: if publish {
                ArtifactProvenance::Requested
            } else {
                ArtifactProvenance::PrivateSource {
                    consumed_by: vec!["mutated".into()],
                }
            },
            case_binding: None,
            instance: ResolvedInstancePlan {
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
                identities,
                attributes: vec![],
                content: vec![],
                references: vec![],
            },
            output: output(path, publish),
            encoding: EncodingPlan {
                transfer_syntax_uid: "1.2.840.10008.1.2.1".into(),
                sequence_length: SequenceLengthPolicy::WriterDefault,
                item_length: ItemLengthPolicy::WriterDefault,
                fragmentation: FragmentationPolicy::Native,
                offset_table: OffsetTablePolicy::NotApplicable,
                preamble: PreamblePolicy::ZeroFilled,
                file_meta: FileMetaPolicy::Standard,
                implementation: ImplementationIdentityPlan {
                    class_uid: "2.25.1002".into(),
                    version_name: Some("DICOMTS010".into()),
                },
                backend_id: "part10_materializer".into(),
            },
            validation: validation(),
            evidence: evidence(),
            resources: ArtifactResourceEstimate {
                output_bytes: 0,
                peak_working_bytes: 1,
            },
        })
    }

    #[derive(Debug)]
    struct TextAuxiliary;
    impl AuxiliaryMaterializationHandler for TextAuxiliary {
        fn render(
            &self,
            artifact: &PlannedAuxiliaryArtifact,
            _: &ArtifactExecutionBindings,
            _: &StagedAssetRegistry,
        ) -> Result<AuxiliaryPayload, MaterializationError> {
            Ok(AuxiliaryPayload {
                bytes: artifact.auxiliary_kind.as_bytes().to_vec(),
                media_type: "text/plain".into(),
                backend: built_in_identity("text_auxiliary"),
                evidence: vec![],
            })
        }
    }

    fn dispatcher(root: &Path) -> MaterializationDispatcher {
        MaterializationDispatcher::new(root, Arc::new(TextAuxiliary)).unwrap()
    }

    fn native_request(
        path: &str,
        bytes: &[u8],
        frames: Vec<super::super::services::NativeFrameBinding>,
    ) -> MaterializationRequest {
        let mut artifact = dicom("primary", path, true);
        let PlannedArtifact::Dicom(value) = &mut artifact else {
            unreachable!()
        };
        value
            .instance
            .content
            .push(crate::composition::CanonicalContent {
                slot: "pixels".into(),
                kind: "native_pixels".into(),
                address: crate::composition::AttributeAddress::from_normalized_tag("7FE0,0010")
                    .unwrap(),
                vr: crate::composition::DicomVr::OB,
                size_bytes: bytes.len() as u64,
                sha256: sha256_hex(bytes),
                properties: BTreeMap::new(),
                placement: crate::composition::ContentPlacement::TopLevel,
                materialization: None,
            });
        MaterializationRequest {
            bindings: ArtifactExecutionBindings {
                artifact_id: "primary".into(),
                slots: BTreeMap::from([(
                    "pixels".into(),
                    SlotExecutionBinding::NativeFrames { frames },
                )]),
            },
            artifact,
        }
    }

    fn native_frame(
        number: u32,
        bytes: Vec<u8>,
        rows: u32,
        columns: u32,
        bits_allocated: u16,
    ) -> super::super::services::NativeFrameBinding {
        super::super::services::NativeFrameBinding {
            frame_number: number,
            bytes: ByteBinding::Inline {
                sha256: sha256_hex(&bytes),
                bytes,
            },
            rows,
            columns,
            samples_per_pixel: 1,
            bits_allocated,
            photometric_interpretation: "MONOCHROME2".into(),
        }
    }

    #[test]
    fn dicom_dispatch_writes_and_verifies_only_beneath_private_staging() {
        let root = root("dicom");
        let artifact = dicom("primary", "instances/primary.dcm", true);
        let request = MaterializationRequest {
            bindings: ArtifactExecutionBindings {
                artifact_id: "primary".into(),
                slots: BTreeMap::new(),
            },
            artifact,
        };
        let result = dispatcher(&root)
            .dispatch(&request, &StagedAssetRegistry::default())
            .unwrap();
        result.validate(&request).unwrap();
        let output = result.output.unwrap();
        assert_eq!(
            output.declaration.visibility,
            AssetVisibility::PublicationCandidate
        );
        assert_eq!(
            &fs::read(root.join("instances/primary.dcm")).unwrap()[128..132],
            b"DICM"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn native_frames_are_ordered_verified_and_report_typed_frame_evidence() {
        let root = root("native-evidence");
        let request = native_request(
            "instances/native.dcm",
            &[1, 2, 3, 4],
            vec![
                native_frame(2, vec![3, 4], 1, 1, 16),
                native_frame(1, vec![1, 2], 1, 1, 16),
            ],
        );
        let result = dispatcher(&root)
            .dispatch(&request, &StagedAssetRegistry::default())
            .unwrap();
        let content = &result.evidence[0].claims["materialized_content"][0];
        assert_eq!(content["native_frame_lengths"], json!([2, 2]));
        assert_eq!(
            content["native_frame_sha256"],
            json!([sha256_hex(&[1, 2]), sha256_hex(&[3, 4])])
        );
        assert_eq!(content["decoded_frame_lengths"], json!([2, 2]));
        assert_eq!(content["native_byte_order"], "little_endian");
        assert_eq!(content["fragment_count"], 0);
        assert_eq!(content["compressed_frame_sha256"], json!([]));
        assert_eq!(content["padded_fragment_lengths"], json!([]));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn native_frame_gaps_and_aggregate_drift_fail_before_output_creation() {
        let duplicate_root = root("native-duplicate");
        let duplicate = native_request(
            "new/native.dcm",
            &[1, 2],
            vec![
                native_frame(1, vec![1], 1, 1, 8),
                native_frame(1, vec![2], 1, 1, 8),
            ],
        );
        assert!(matches!(
            dispatcher(&duplicate_root).dispatch(&duplicate, &StagedAssetRegistry::default()),
            Err(MaterializationError::Service(
                ServiceError::DuplicateFrameNumber { .. }
            ))
        ));
        assert!(!duplicate_root.join("new").exists());

        let gap_root = root("native-gap");
        let gap = native_request(
            "new/native.dcm",
            &[1, 2],
            vec![
                native_frame(1, vec![1], 1, 1, 8),
                native_frame(3, vec![2], 1, 1, 8),
            ],
        );
        assert!(matches!(
            dispatcher(&gap_root).dispatch(&gap, &StagedAssetRegistry::default()),
            Err(MaterializationError::NativeFrameOrder {
                expected: 2,
                actual: 3
            })
        ));
        assert!(!gap_root.join("new").exists());

        let drift_root = root("native-drift");
        let mut drift = native_request(
            "new/native.dcm",
            &[1, 2],
            vec![native_frame(1, vec![1, 3], 1, 2, 8)],
        );
        let PlannedArtifact::Dicom(artifact) = &mut drift.artifact else {
            unreachable!()
        };
        artifact.instance.content[0].sha256 = sha256_hex(&[1, 2]);
        assert!(matches!(
            dispatcher(&drift_root).dispatch(&drift, &StagedAssetRegistry::default()),
            Err(MaterializationError::NativeContentHash { .. })
        ));
        assert!(!drift_root.join("new").exists());
        fs::remove_dir_all(duplicate_root).unwrap();
        fs::remove_dir_all(gap_root).unwrap();
        fs::remove_dir_all(drift_root).unwrap();
    }

    #[test]
    fn u1_frames_decode_from_one_continuous_lsb_first_bitstream() {
        let root = root("native-u1");
        // Eighteen alternating samples. Frame two begins at bit 9, not a byte
        // boundary; the input chunks must first be concatenated exactly.
        let packed = [0x55, 0x55, 0x01];
        let request = native_request(
            "instances/u1.dcm",
            &packed,
            vec![
                native_frame(2, vec![0x01], 3, 3, 1),
                native_frame(1, vec![0x55, 0x55], 3, 3, 1),
            ],
        );
        let result = dispatcher(&root)
            .dispatch(&request, &StagedAssetRegistry::default())
            .unwrap();
        let content = &result.evidence[0].claims["materialized_content"][0];
        let frame_one = (0..9)
            .map(|index| (packed[index / 8] >> (index % 8)) & 1)
            .collect::<Vec<_>>();
        let frame_two = (9..18)
            .map(|index| (packed[index / 8] >> (index % 8)) & 1)
            .collect::<Vec<_>>();
        assert_eq!(content["native_frame_lengths"], json!([2, 1]));
        assert_eq!(content["decoded_frame_lengths"], json!([9, 9]));
        assert_eq!(content["native_bit_packing"]["bit_order"], "lsb_first");
        assert_eq!(
            content["native_bit_packing"]["continuous_across_frames"],
            true
        );
        assert_eq!(content["native_bit_packing"]["stored_values_per_frame"], 9);
        assert_eq!(content["native_bit_packing"]["unused_trailing_bits"], 6);
        assert_eq!(
            content["decoded_frame_sha256"],
            json!([sha256_hex(&frame_one), sha256_hex(&frame_two)])
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn extended_offsets_are_materialized_from_execution_local_frame_arithmetic() {
        let root = root("extended-offsets");
        let mut artifact = dicom("primary", "instances/primary.dcm", true);
        let PlannedArtifact::Dicom(value) = &mut artifact else {
            unreachable!()
        };
        value.instance.transfer_syntax_uid = "1.2.840.10008.1.2.5".into();
        value.encoding.transfer_syntax_uid = "1.2.840.10008.1.2.5".into();
        value.encoding.fragmentation = FragmentationPolicy::OneFragmentPerFrame;
        value.encoding.offset_table = OffsetTablePolicy::Extended;
        value.encoding.backend_id = "encoding.native.rle_lossless".into();
        value
            .instance
            .content
            .push(crate::composition::CanonicalContent {
                slot: "pixels".into(),
                kind: "encoded_frames".into(),
                address: crate::composition::AttributeAddress::from_normalized_tag("7FE0,0010")
                    .unwrap(),
                vr: crate::composition::DicomVr::OB,
                size_bytes: 0,
                sha256: sha256_hex(&[]),
                properties: BTreeMap::new(),
                placement: crate::composition::ContentPlacement::TopLevel,
                materialization: None,
            });
        let frames = [vec![1, 2, 3], vec![4, 5, 6, 7]];
        let encoded = frames
            .iter()
            .enumerate()
            .map(
                |(index, bytes)| super::super::services::EncodedFrameResult {
                    frame_number: index as u32 + 1,
                    bytes: ByteBinding::Inline {
                        bytes: bytes.clone(),
                        sha256: sha256_hex(bytes),
                    },
                    encoded_size_bytes: bytes.len() as u64,
                    encoded_sha256: sha256_hex(bytes),
                },
            )
            .collect();
        let request = MaterializationRequest {
            bindings: ArtifactExecutionBindings {
                artifact_id: "primary".into(),
                slots: BTreeMap::from([(
                    "pixels".into(),
                    SlotExecutionBinding::EncodedFrames { frames: encoded },
                )]),
            },
            artifact,
        };
        let result = dispatcher(&root)
            .dispatch(&request, &StagedAssetRegistry::default())
            .unwrap();
        let claims = &result.evidence[0].claims;
        let content = claims["materialized_content"].as_array().unwrap();
        assert_eq!(content[0]["fragment_count"], 2);
        assert_eq!(content[0]["compressed_lengths"], json!([3, 4]));
        assert_eq!(content[0]["padded_fragment_lengths"], json!([4, 4]));
        assert_eq!(content[0]["fragments_per_frame"], json!([1, 1]));
        assert_eq!(
            content[0]["fragments"],
            json!([
                {
                    "frame_index": 0,
                    "item_start_offset": 8,
                    "compressed_length": 3,
                    "padded_length": 4
                },
                {
                    "frame_index": 1,
                    "item_start_offset": 20,
                    "compressed_length": 4,
                    "padded_length": 4
                }
            ])
        );
        assert_eq!(content[0]["basic_offset_table"], json!([]));
        assert_eq!(content[0]["extended_offset_table"], json!([0, 12]));
        assert_eq!(content[0]["extended_offset_table_lengths"], json!([3, 4]));
        assert_eq!(
            claims["materialized_artifact_sha256"],
            result.output.unwrap().observed_sha256
        );

        let object = dicom_object::open_file(root.join("instances/primary.dcm")).unwrap();
        assert_eq!(
            object
                .element(dicom_core::Tag(0x7fe0, 0x0001))
                .unwrap()
                .to_bytes()
                .unwrap()
                .as_ref(),
            &[0, 0, 0, 0, 0, 0, 0, 0, 12, 0, 0, 0, 0, 0, 0, 0]
        );
        assert_eq!(
            object
                .element(dicom_core::Tag(0x7fe0, 0x0002))
                .unwrap()
                .to_bytes()
                .unwrap()
                .as_ref(),
            &[3, 0, 0, 0, 0, 0, 0, 0, 4, 0, 0, 0, 0, 0, 0, 0]
        );
        let basic_offsets = object
            .element(dicom_dictionary_std::tags::PIXEL_DATA)
            .unwrap()
            .value()
            .offset_table()
            .unwrap();
        assert!(basic_offsets.is_empty());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_native_binding_for_encapsulated_policy_before_output_creation() {
        let root = root("binding-reject");
        let mut artifact = dicom("primary", "new/primary.dcm", true);
        let PlannedArtifact::Dicom(value) = &mut artifact else {
            unreachable!()
        };
        value.instance.transfer_syntax_uid = "1.2.840.10008.1.2.5".into();
        value.encoding.transfer_syntax_uid = "1.2.840.10008.1.2.5".into();
        value.encoding.fragmentation = FragmentationPolicy::OneFragmentPerFrame;
        value.encoding.offset_table = OffsetTablePolicy::EmptyBasic;
        value.encoding.backend_id = "encoding.native.rle_lossless".into();
        value
            .instance
            .content
            .push(crate::composition::CanonicalContent {
                slot: "pixels".into(),
                kind: "native_pixels".into(),
                address: crate::composition::AttributeAddress::from_normalized_tag("7FE0,0010")
                    .unwrap(),
                vr: crate::composition::DicomVr::OB,
                size_bytes: 2,
                sha256: sha256_hex(&[1, 2]),
                properties: BTreeMap::new(),
                placement: crate::composition::ContentPlacement::TopLevel,
                materialization: None,
            });
        let request = MaterializationRequest {
            bindings: ArtifactExecutionBindings {
                artifact_id: "primary".into(),
                slots: BTreeMap::from([(
                    "pixels".into(),
                    SlotExecutionBinding::NativeFrames {
                        frames: vec![super::super::services::NativeFrameBinding {
                            frame_number: 1,
                            bytes: ByteBinding::Inline {
                                bytes: vec![1, 2],
                                sha256: sha256_hex(&[1, 2]),
                            },
                            rows: 1,
                            columns: 2,
                            samples_per_pixel: 1,
                            bits_allocated: 8,
                            photometric_interpretation: "MONOCHROME2".into(),
                        }],
                    },
                )]),
            },
            artifact,
        };
        assert!(matches!(
            dispatcher(&root).dispatch(&request, &StagedAssetRegistry::default()),
            Err(MaterializationError::EncodingBindingMismatch { .. })
        ));
        assert!(!root.join("new").exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn dispatcher_rejects_a_symlinked_staging_parent() {
        use std::os::unix::fs::symlink;

        let staging = root("symlink");
        let outside = root("outside");
        symlink(&outside, staging.join("instances")).unwrap();
        let request = MaterializationRequest {
            bindings: ArtifactExecutionBindings {
                artifact_id: "primary".into(),
                slots: BTreeMap::new(),
            },
            artifact: dicom("primary", "instances/primary.dcm", true),
        };
        assert!(matches!(
            dispatcher(&staging).dispatch(&request, &StagedAssetRegistry::default()),
            Err(MaterializationError::UnsafeOutputPath(_))
        ));
        assert!(fs::read_dir(&outside).unwrap().next().is_none());
        fs::remove_dir_all(staging).unwrap();
        fs::remove_dir_all(outside).unwrap();
    }

    #[test]
    fn ordered_mutation_consumes_only_a_declared_private_source() {
        let root = root("mutation");
        let source_request = MaterializationRequest {
            artifact: dicom("source", "private/source.dcm", false),
            bindings: ArtifactExecutionBindings {
                artifact_id: "source".into(),
                slots: BTreeMap::new(),
            },
        };
        let dispatcher = dispatcher(&root);
        let source_result = dispatcher
            .dispatch(&source_request, &StagedAssetRegistry::default())
            .unwrap();
        let source_asset = source_result.output.unwrap();
        let source_bytes = fs::read(root.join("private/source.dcm")).unwrap();
        let truncate_at = source_bytes.len() - 2;
        let expected = &source_bytes[..truncate_at];
        let source_handle = source_asset.declaration.handle.clone();
        let mut assets = StagedAssetRegistry::default();
        assets.register(source_asset).unwrap();
        let artifact = PlannedArtifact::Mutation(PlannedMutationArtifact {
            logical_id: "mutated".into(),
            order: 1,
            provenance: ArtifactProvenance::Requested,
            source_artifact_id: "source".into(),
            mutation: MutationPlan {
                contract_version: crate::mutation::MUTATION_CONTRACT_VERSION.into(),
                operations: vec![PlannedMutationOperation {
                    operation_id: "truncate_dataset".into(),
                    source_ranges: vec![PlannedByteRange {
                        start: truncate_at as u64,
                        end: source_bytes.len() as u64,
                    }],
                    parameters: BTreeMap::new(),
                }],
                expected_source_sha256: sha256_hex(&source_bytes),
                expected_output_sha256: sha256_hex(expected),
                expected_failure_layers: vec!["dataset_parser".into()],
                acceptable_outcomes: vec!["clean_rejection".into()],
            },
            output: output("negative/mutated.dcm", true),
            validation: validation(),
            evidence: evidence(),
            resources: ArtifactResourceEstimate {
                output_bytes: expected.len() as u64,
                peak_working_bytes: source_bytes.len() as u64,
            },
        });
        let request = MaterializationRequest {
            bindings: ArtifactExecutionBindings {
                artifact_id: "mutated".into(),
                slots: BTreeMap::from([(
                    "source".into(),
                    SlotExecutionBinding::StagedAsset {
                        asset: source_handle,
                    },
                )]),
            },
            artifact,
        };
        let result = dispatcher.dispatch(&request, &assets).unwrap();
        assert_eq!(
            fs::read(root.join("negative/mutated.dcm")).unwrap(),
            expected
        );
        assert_eq!(result.evidence[0].evidence_id, "ordered_mutation_steps");

        let mut public_assets = StagedAssetRegistry::default();
        let public_source = produced_for_test(
            "output:source",
            "private/source.dcm",
            &source_bytes,
            AssetVisibility::PublicationCandidate,
        );
        public_assets.register(public_source).unwrap();
        let mut public_request = request;
        public_request.bindings.slots.insert(
            "source".into(),
            SlotExecutionBinding::StagedAsset {
                asset: StagedAssetHandle::new("output:source").unwrap(),
            },
        );
        assert!(matches!(
            dispatcher.dispatch(&public_request, &public_assets),
            Err(MaterializationError::MutationSourceNotPrivate(_))
        ));
        fs::remove_dir_all(root).unwrap();
    }

    fn produced_for_test(
        handle: &str,
        path: &str,
        bytes: &[u8],
        visibility: AssetVisibility,
    ) -> ProducedAsset {
        let digest = sha256_hex(bytes);
        ProducedAsset {
            declaration: AssetDeclaration {
                handle: StagedAssetHandle::new(handle).unwrap(),
                relative_path: StagingRelativePath::new(path).unwrap(),
                size_bytes: bytes.len() as u64,
                sha256: digest.clone(),
                media_type: "application/dicom".into(),
                visibility,
            },
            observed_size_bytes: bytes.len() as u64,
            observed_sha256: digest,
        }
    }

    #[test]
    fn qualification_is_payload_free_and_auxiliary_uses_injected_handler() {
        let root = root("other-kinds");
        let dispatcher = dispatcher(&root);
        let qualification = PlannedArtifact::Qualification(PlannedQualification {
            logical_id: "qualification".into(),
            order: 0,
            provenance: ArtifactProvenance::Requested,
            qualification_kind: "bounded_check".into(),
            parameters: BTreeMap::new(),
            payload_policy: QualificationPayloadPolicy::EvidenceOnly,
            validation: validation(),
            evidence: evidence(),
            resources: ArtifactResourceEstimate {
                output_bytes: 0,
                peak_working_bytes: 1,
            },
        });
        let request = MaterializationRequest {
            artifact: qualification,
            bindings: ArtifactExecutionBindings {
                artifact_id: "qualification".into(),
                slots: BTreeMap::new(),
            },
        };
        let result = dispatcher
            .dispatch(&request, &StagedAssetRegistry::default())
            .unwrap();
        assert!(result.output.is_none());
        assert_eq!(result.evidence.len(), 1);

        let payload = produced_for_test(
            "qualification-payload",
            "assets/qualification.bin",
            b"forbidden",
            AssetVisibility::Private,
        );
        let payload_handle = payload.declaration.handle.clone();
        let mut payload_assets = StagedAssetRegistry::default();
        payload_assets.register(payload).unwrap();
        let mut payload_request = request;
        payload_request.bindings.slots.insert(
            "payload".into(),
            SlotExecutionBinding::StagedAsset {
                asset: payload_handle,
            },
        );
        assert!(matches!(
            dispatcher.dispatch(&payload_request, &payload_assets),
            Err(MaterializationError::QualificationPayloadForbidden(_))
        ));

        let auxiliary = PlannedArtifact::Auxiliary(PlannedAuxiliaryArtifact {
            logical_id: "report".into(),
            order: 1,
            provenance: ArtifactProvenance::Requested,
            auxiliary_kind: "coverage_report".into(),
            output: output("reports/coverage.txt", true),
            parameters: BTreeMap::new(),
            validation: validation(),
            evidence: evidence(),
            resources: ArtifactResourceEstimate {
                output_bytes: 15,
                peak_working_bytes: 15,
            },
        });
        let request = MaterializationRequest {
            artifact: auxiliary,
            bindings: ArtifactExecutionBindings {
                artifact_id: "report".into(),
                slots: BTreeMap::new(),
            },
        };
        let result = dispatcher
            .dispatch(&request, &StagedAssetRegistry::default())
            .unwrap();
        result.validate(&request).unwrap();
        assert_eq!(
            fs::read(root.join("reports/coverage.txt")).unwrap(),
            b"coverage_report"
        );
        fs::remove_dir_all(root).unwrap();
    }
}
