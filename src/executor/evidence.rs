use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde::{Deserialize, Serialize};

pub const RUN_EVIDENCE_SCHEMA_VERSION: &str = "0.1.0";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunEvidence {
    pub schema_version: String,
    pub corpus_plan_sha256: String,
    pub artifacts: Vec<ArtifactExecutionEvidence>,
    #[serde(default)]
    pub unavailable: Vec<UnavailableExecutionEvidence>,
    pub resources: RunResourceEvidence,
    pub publication: PublicationEvidence,
}

impl RunEvidence {
    pub fn validate(&self, expected_artifact_order: &[String]) -> Result<(), EvidenceError> {
        if self.schema_version != RUN_EVIDENCE_SCHEMA_VERSION {
            return Err(EvidenceError::UnsupportedSchemaVersion(
                self.schema_version.clone(),
            ));
        }
        validate_sha256("corpus plan", &self.corpus_plan_sha256)?;
        let actual_order = self
            .artifacts
            .iter()
            .map(|artifact| artifact.logical_id.clone())
            .collect::<Vec<_>>();
        if actual_order != expected_artifact_order {
            return Err(EvidenceError::ArtifactOrderMismatch {
                expected: expected_artifact_order.to_vec(),
                actual: actual_order,
            });
        }

        let mut logical_ids = BTreeSet::new();
        let mut orders = BTreeSet::new();
        let mut output_paths = BTreeSet::new();
        let mut artifact_output_bytes = 0_u64;
        for artifact in &self.artifacts {
            artifact.validate()?;
            if !logical_ids.insert(&artifact.logical_id) {
                return Err(EvidenceError::DuplicateArtifact(
                    artifact.logical_id.clone(),
                ));
            }
            if !orders.insert(artifact.order) {
                return Err(EvidenceError::DuplicateArtifactOrder(artifact.order));
            }
            if let Some(output) = &artifact.output {
                if !output_paths.insert(&output.relative_path) {
                    return Err(EvidenceError::DuplicateOutputPath(
                        output.relative_path.clone(),
                    ));
                }
                artifact_output_bytes = artifact_output_bytes
                    .checked_add(output.size_bytes)
                    .ok_or(EvidenceError::ResourceOverflow)?;
            }
        }
        if artifact_output_bytes != self.resources.actual_artifact_output_bytes {
            return Err(EvidenceError::ArtifactOutputTotalMismatch {
                expected: artifact_output_bytes,
                actual: self.resources.actual_artifact_output_bytes,
            });
        }
        self.resources.validate()?;

        let mut unavailable_ids = BTreeSet::new();
        for unavailable in &self.unavailable {
            unavailable.validate()?;
            if !unavailable_ids.insert(&unavailable.capability_id) {
                return Err(EvidenceError::DuplicateUnavailableCapability(
                    unavailable.capability_id.clone(),
                ));
            }
        }
        self.publication.validate()?;
        if self.publication.state == PublicationState::Promoted {
            if self
                .artifacts
                .iter()
                .any(|artifact| artifact.status != ExecutionStatus::Succeeded)
            {
                return Err(EvidenceError::PromotedWithIncompleteArtifact);
            }
            if !self.publication.cleanup_complete || self.publication.manifest_sha256.is_none() {
                return Err(EvidenceError::IncompletePromotionEvidence);
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactKind {
    Dicom,
    Mutation,
    Qualification,
    Auxiliary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionStatus {
    Succeeded,
    Failed,
    Cancelled,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactExecutionEvidence {
    pub logical_id: String,
    pub order: u64,
    pub artifact_kind: ArtifactKind,
    pub status: ExecutionStatus,
    pub corpus_plan_sha256: String,
    pub instance_plan_sha256: Option<String>,
    pub output: Option<OutputEvidence>,
    pub materialization: Option<MaterializationEvidence>,
    #[serde(default)]
    pub validation: Vec<ValidationResult>,
    #[serde(default)]
    pub obligations: Vec<ObligationResult>,
    #[serde(default)]
    pub providers: Vec<ProviderEvidence>,
    #[serde(default)]
    pub codecs: Vec<CodecEvidence>,
    pub resources: ArtifactResourceEvidence,
}

impl ArtifactExecutionEvidence {
    fn validate(&self) -> Result<(), EvidenceError> {
        validate_identifier("artifact logical ID", &self.logical_id)?;
        validate_sha256("artifact corpus plan", &self.corpus_plan_sha256)?;
        if let Some(hash) = &self.instance_plan_sha256 {
            validate_sha256("instance plan", hash)?;
        }
        if self.artifact_kind == ArtifactKind::Dicom && self.instance_plan_sha256.is_none() {
            return Err(EvidenceError::MissingInstancePlanHash(
                self.logical_id.clone(),
            ));
        }
        if let Some(output) = &self.output {
            output.validate()?;
        }
        if let Some(materialization) = &self.materialization {
            materialization.validate()?;
        }
        if self.status == ExecutionStatus::Succeeded
            && self.artifact_kind == ArtifactKind::Dicom
            && (self.output.is_none() || self.materialization.is_none())
        {
            return Err(EvidenceError::IncompleteDicomEvidence(
                self.logical_id.clone(),
            ));
        }
        validate_unique_results(
            "validation rule",
            self.validation.iter().map(|result| &result.rule_id),
        )?;
        validate_unique_results(
            "evidence obligation",
            self.obligations.iter().map(|result| &result.obligation_id),
        )?;
        for result in &self.validation {
            result.validate()?;
        }
        for result in &self.obligations {
            result.validate()?;
        }
        for provider in &self.providers {
            provider.validate()?;
        }
        for codec in &self.codecs {
            codec.validate()?;
        }
        self.resources.validate()?;
        if let Some(output) = &self.output {
            if output.size_bytes != self.resources.actual_output_bytes {
                return Err(EvidenceError::ArtifactResourceMismatch {
                    logical_id: self.logical_id.clone(),
                });
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OutputEvidence {
    pub relative_path: String,
    pub publish: bool,
    pub size_bytes: u64,
    pub sha256: String,
}

impl OutputEvidence {
    fn validate(&self) -> Result<(), EvidenceError> {
        validate_relative_path(&self.relative_path)?;
        validate_sha256("output", &self.sha256)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MaterializationEvidence {
    pub backend_id: String,
    pub transfer_syntax_uid: Option<String>,
    #[serde(default)]
    pub streamed_slots: Vec<String>,
    pub completed: bool,
    pub materialized_instance_plan_sha256: Option<String>,
    /// Hash of the encoding policy together with the execution-observed
    /// fragmentation/table facts. This deliberately excludes payload bytes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub materialized_encoding_sha256: Option<String>,
    /// Hash of the completed staged artifact.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub materialized_artifact_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preamble_policy: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preamble_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_meta_policy: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_meta_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_meta_size_bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub implementation_class_uid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub implementation_version_name: Option<String>,
    #[serde(default)]
    pub content: Vec<MaterializedContentEvidence>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub imported_dicom: Option<ImportedDicomObservation>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub service_evidence: Vec<MaterializationServiceEvidence>,
}

pub const MAX_MATERIALIZATION_SERVICE_EVIDENCE: usize = 64;
pub const MAX_MATERIALIZATION_SERVICE_CLAIMS: usize = 64;
pub const MAX_MATERIALIZATION_SERVICE_CLAIMS_BYTES: usize = 256 * 1024;

/// Service-originated materialization facts retained without depending on the
/// executor service contract layer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MaterializationServiceEvidence {
    pub evidence_id: String,
    pub evidence_kind: String,
    pub producer_id: String,
    pub producer_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub producer_executable_sha256: Option<String>,
    #[serde(default)]
    pub claims: BTreeMap<String, serde_json::Value>,
}

impl MaterializationServiceEvidence {
    fn validate(&self) -> Result<(), EvidenceError> {
        validate_identifier("materialization service evidence ID", &self.evidence_id)?;
        validate_identifier("materialization service evidence kind", &self.evidence_kind)?;
        validate_identifier("materialization service producer ID", &self.producer_id)?;
        validate_identifier(
            "materialization service producer version",
            &self.producer_version,
        )?;
        if let Some(hash) = &self.producer_executable_sha256 {
            validate_sha256("materialization service executable", hash)?;
        }
        if self.claims.len() > MAX_MATERIALIZATION_SERVICE_CLAIMS {
            return Err(EvidenceError::MaterializationServiceEvidenceBounds);
        }
        for key in self.claims.keys() {
            validate_identifier("materialization service claim", key)?;
        }
        let size = serde_json::to_vec(&self.claims)
            .map_err(|_| EvidenceError::MaterializationServiceEvidenceBounds)?
            .len();
        if size > MAX_MATERIALIZATION_SERVICE_CLAIMS_BYTES {
            return Err(EvidenceError::MaterializationServiceEvidenceBounds);
        }
        Ok(())
    }
}

pub const IMPORTED_DICOM_OBSERVATION_SCHEMA_VERSION: &str = "0.1.0";
pub const MAX_IMPORTED_DICOM_REFERENCES: usize = 64;
pub const MAX_IMPORTED_DICOM_CONTENT_FIELDS: usize = 8;
pub const MAX_IMPORTED_DICOM_FRAMES_PER_REFERENCE: usize = 1024;
pub const MAX_IMPORTED_DICOM_TOTAL_REFERENCED_FRAMES: usize = 4096;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImportedDicomObservation {
    pub schema_version: String,
    pub sop_class_uid: String,
    pub sop_instance_uid: String,
    pub transfer_syntax_uid: String,
    pub study_instance_uid: Option<String>,
    pub series_instance_uid: Option<String>,
    pub frame_of_reference_uid: Option<String>,
    pub rows: Option<u32>,
    pub columns: Option<u32>,
    pub frames: Option<u32>,
    #[serde(default)]
    pub content: Vec<ImportedDicomContentObservation>,
    #[serde(default)]
    pub references: Vec<ImportedDicomReferenceObservation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImportedDicomContentObservation {
    pub tag: String,
    pub vr: String,
    pub size_bytes: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImportedDicomReferenceObservation {
    pub role: String,
    pub sop_class_uid: String,
    pub sop_instance_uid: String,
    #[serde(default)]
    pub frame_numbers: Vec<u32>,
}

impl ImportedDicomObservation {
    pub fn validate(&self) -> Result<(), EvidenceError> {
        if self.schema_version != IMPORTED_DICOM_OBSERVATION_SCHEMA_VERSION {
            return Err(EvidenceError::UnsupportedImportedDicomObservationVersion(
                self.schema_version.clone(),
            ));
        }
        for uid in [
            Some(&self.sop_class_uid),
            Some(&self.sop_instance_uid),
            Some(&self.transfer_syntax_uid),
            self.study_instance_uid.as_ref(),
            self.series_instance_uid.as_ref(),
            self.frame_of_reference_uid.as_ref(),
        ]
        .into_iter()
        .flatten()
        {
            validate_identifier("imported DICOM UID", uid)?;
        }
        if self.content.len() > MAX_IMPORTED_DICOM_CONTENT_FIELDS
            || self.references.len() > MAX_IMPORTED_DICOM_REFERENCES
        {
            return Err(EvidenceError::ImportedDicomObservationBounds);
        }
        if [self.rows, self.columns, self.frames]
            .into_iter()
            .flatten()
            .any(|value| value == 0)
        {
            return Err(EvidenceError::ImportedDicomObservationBounds);
        }
        let mut content_tags = BTreeSet::new();
        for content in &self.content {
            validate_identifier("imported content tag", &content.tag)?;
            validate_identifier("imported content VR", &content.vr)?;
            validate_sha256("imported content", &content.sha256)?;
            if content.size_bytes == 0 || !content_tags.insert(&content.tag) {
                return Err(EvidenceError::ImportedDicomObservationBounds);
            }
        }
        let mut references = BTreeSet::new();
        let mut total_frames = 0usize;
        for reference in &self.references {
            validate_identifier("imported reference role", &reference.role)?;
            validate_identifier("imported reference SOP class", &reference.sop_class_uid)?;
            validate_identifier(
                "imported reference SOP instance",
                &reference.sop_instance_uid,
            )?;
            if !references.insert((
                &reference.role,
                &reference.sop_class_uid,
                &reference.sop_instance_uid,
            )) || reference.frame_numbers.len() > MAX_IMPORTED_DICOM_FRAMES_PER_REFERENCE
            {
                return Err(EvidenceError::ImportedDicomObservationBounds);
            }
            let mut frames = BTreeSet::new();
            for frame in &reference.frame_numbers {
                if *frame == 0 || !frames.insert(frame) {
                    return Err(EvidenceError::ImportedDicomObservationBounds);
                }
            }
            total_frames = total_frames
                .checked_add(reference.frame_numbers.len())
                .ok_or(EvidenceError::ImportedDicomObservationBounds)?;
        }
        if total_frames > MAX_IMPORTED_DICOM_TOTAL_REFERENCED_FRAMES {
            return Err(EvidenceError::ImportedDicomObservationBounds);
        }
        Ok(())
    }
}

impl MaterializationEvidence {
    fn validate(&self) -> Result<(), EvidenceError> {
        validate_identifier("materialization backend", &self.backend_id)?;
        validate_unique_results("streamed slot", self.streamed_slots.iter())?;
        if let Some(hash) = &self.materialized_instance_plan_sha256 {
            validate_sha256("materialized instance plan", hash)?;
        }
        if let Some(hash) = &self.materialized_encoding_sha256 {
            validate_sha256("materialized encoding", hash)?;
        }
        if let Some(hash) = &self.materialized_artifact_sha256 {
            validate_sha256("materialized artifact", hash)?;
        }
        if let Some(hash) = &self.preamble_sha256 {
            validate_sha256("Part 10 preamble", hash)?;
        }
        if let Some(hash) = &self.file_meta_sha256 {
            validate_sha256("Part 10 File Meta", hash)?;
        }
        validate_unique_results(
            "materialized content slot",
            self.content.iter().map(|c| &c.slot),
        )?;
        if self.service_evidence.len() > MAX_MATERIALIZATION_SERVICE_EVIDENCE {
            return Err(EvidenceError::MaterializationServiceEvidenceBounds);
        }
        validate_unique_results(
            "materialization service evidence",
            self.service_evidence
                .iter()
                .map(|evidence| &evidence.evidence_id),
        )?;
        for evidence in &self.service_evidence {
            evidence.validate()?;
        }
        for content in &self.content {
            validate_identifier("materialized content kind", &content.kind)?;
            validate_identifier("materialized content VR", &content.vr)?;
            validate_sha256("materialized content", &content.sha256)?;
            for hash in &content.compressed_frame_sha256 {
                validate_sha256("compressed frame", hash)?;
            }
            for hash in &content.native_frame_sha256 {
                validate_sha256("native frame", hash)?;
            }
            for hash in &content.decoded_frame_sha256 {
                validate_sha256("decoded frame", hash)?;
            }
            if content.native_frame_sha256.len() != content.native_frame_lengths.len()
                || content.decoded_frame_sha256.len() != content.decoded_frame_lengths.len()
            {
                return Err(EvidenceError::FrameEvidenceCardinality(
                    content.slot.clone(),
                ));
            }
            content.validate_encoding_facts()?;
        }
        if let Some(observation) = &self.imported_dicom {
            observation.validate()?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MaterializedFragmentEvidence {
    pub frame_index: u64,
    pub item_start_offset: u64,
    pub compressed_length: u64,
    pub padded_length: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeByteOrderEvidence {
    LittleEndian,
    BigEndian,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeBitPackingEvidence {
    pub bit_order: String,
    pub continuous_across_frames: bool,
    pub stored_values_per_frame: u64,
    pub total_stored_values: u64,
    pub packed_size_bytes: u64,
    pub unused_trailing_bits: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MaterializedContentEvidence {
    pub slot: String,
    pub kind: String,
    pub vr: String,
    pub size_bytes: u64,
    pub sha256: String,
    #[serde(default)]
    pub basic_offset_table: Vec<u32>,
    #[serde(default)]
    pub compressed_frame_sha256: Vec<String>,
    /// Execution-observed hashes of the ordered native frame bindings. For
    /// continuously packed U1, these retain the binding chunk identities.
    #[serde(default)]
    pub native_frame_sha256: Vec<String>,
    #[serde(default)]
    pub native_frame_lengths: Vec<u64>,
    /// Hashes of logical decoded frames. U1 frames are expanded to one 0/1
    /// byte per sample from the continuous LSB-first aggregate.
    #[serde(default)]
    pub decoded_frame_sha256: Vec<String>,
    #[serde(default)]
    pub decoded_frame_lengths: Vec<u64>,
    #[serde(default)]
    pub fragment_count: u64,
    #[serde(default)]
    pub compressed_lengths: Vec<u64>,
    #[serde(default)]
    pub padded_fragment_lengths: Vec<u64>,
    #[serde(default)]
    pub fragments_per_frame: Vec<u64>,
    #[serde(default)]
    pub fragments: Vec<MaterializedFragmentEvidence>,
    #[serde(default)]
    pub extended_offset_table: Vec<u64>,
    #[serde(default)]
    pub extended_offset_table_lengths: Vec<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub native_byte_order: Option<NativeByteOrderEvidence>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub native_bit_packing: Option<NativeBitPackingEvidence>,
    /// Exact native Pixel Data Value Field identity after DICOM's mandatory
    /// even-length VR padding. The canonical size/hash above remain unpadded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub native_value_field_size_bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub native_value_field_sha256: Option<String>,
    /// How the writer consumed this slot when execution used a distinct
    /// materialization path (for example, bounded file streaming).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub writer_materialization: Option<String>,
}

impl MaterializedContentEvidence {
    fn validate_encoding_facts(&self) -> Result<(), EvidenceError> {
        let fragment_count = usize::try_from(self.fragment_count)
            .map_err(|_| EvidenceError::FragmentEvidenceCardinality(self.slot.clone()))?;
        if fragment_count != self.fragments.len()
            || fragment_count != self.compressed_lengths.len()
            || fragment_count != self.padded_fragment_lengths.len()
        {
            return Err(EvidenceError::FragmentEvidenceCardinality(
                self.slot.clone(),
            ));
        }
        let declared_fragments = self
            .fragments_per_frame
            .iter()
            .try_fold(0_u64, |total, count| total.checked_add(*count));
        if declared_fragments != Some(self.fragment_count)
            || (!self.fragments_per_frame.is_empty()
                && self.compressed_frame_sha256.len() != self.fragments_per_frame.len())
        {
            return Err(EvidenceError::FragmentEvidenceCardinality(
                self.slot.clone(),
            ));
        }
        if fragment_count > 0 {
            let first_item_start = 8_u64
                .checked_add(
                    u64::try_from(self.basic_offset_table.len())
                        .map_err(|_| EvidenceError::FragmentEvidenceArithmetic(self.slot.clone()))?
                        .checked_mul(4)
                        .ok_or_else(|| {
                            EvidenceError::FragmentEvidenceArithmetic(self.slot.clone())
                        })?,
                )
                .ok_or_else(|| EvidenceError::FragmentEvidenceArithmetic(self.slot.clone()))?;
            let mut expected_start = first_item_start;
            let mut fragment_index = 0usize;
            let mut frame_item_offsets = Vec::with_capacity(self.fragments_per_frame.len());
            let mut frame_compressed_lengths = Vec::with_capacity(self.fragments_per_frame.len());
            for (frame_index, count) in self.fragments_per_frame.iter().enumerate() {
                if *count == 0 {
                    return Err(EvidenceError::FragmentEvidenceCardinality(
                        self.slot.clone(),
                    ));
                }
                frame_item_offsets.push(expected_start - first_item_start);
                let mut frame_compressed_length = 0_u64;
                for _ in 0..*count {
                    let fragment = &self.fragments[fragment_index];
                    let expected_padded_length = fragment
                        .compressed_length
                        .checked_add(fragment.compressed_length % 2)
                        .ok_or_else(|| {
                            EvidenceError::FragmentEvidenceArithmetic(self.slot.clone())
                        })?;
                    if fragment.frame_index != frame_index as u64
                        || fragment.item_start_offset != expected_start
                        || fragment.compressed_length != self.compressed_lengths[fragment_index]
                        || fragment.padded_length != self.padded_fragment_lengths[fragment_index]
                        || fragment.padded_length != expected_padded_length
                    {
                        return Err(EvidenceError::FragmentEvidenceArithmetic(self.slot.clone()));
                    }
                    expected_start = expected_start
                        .checked_add(8)
                        .and_then(|value| value.checked_add(fragment.padded_length))
                        .ok_or_else(|| {
                            EvidenceError::FragmentEvidenceArithmetic(self.slot.clone())
                        })?;
                    frame_compressed_length = frame_compressed_length
                        .checked_add(fragment.compressed_length)
                        .ok_or_else(|| {
                            EvidenceError::FragmentEvidenceArithmetic(self.slot.clone())
                        })?;
                    fragment_index += 1;
                }
                frame_compressed_lengths.push(frame_compressed_length);
            }
            if (!self.basic_offset_table.is_empty()
                && (self.basic_offset_table.len() != frame_item_offsets.len()
                    || self
                        .basic_offset_table
                        .iter()
                        .map(|offset| u64::from(*offset))
                        .ne(frame_item_offsets.iter().copied())))
                || (!self.extended_offset_table.is_empty()
                    && (self.extended_offset_table != frame_item_offsets
                        || self.extended_offset_table_lengths != frame_compressed_lengths))
            {
                return Err(EvidenceError::FragmentEvidenceArithmetic(self.slot.clone()));
            }
            let padded_total = self
                .padded_fragment_lengths
                .iter()
                .try_fold(0_u64, |total, length| total.checked_add(*length));
            if padded_total != Some(self.size_bytes) {
                return Err(EvidenceError::FragmentEvidenceArithmetic(self.slot.clone()));
            }
        }
        if self.extended_offset_table.len() != self.extended_offset_table_lengths.len()
            || (!self.extended_offset_table.is_empty()
                && self.extended_offset_table.len() != self.fragments_per_frame.len())
        {
            return Err(EvidenceError::FragmentEvidenceCardinality(
                self.slot.clone(),
            ));
        }
        if let Some(bits) = &self.native_bit_packing {
            if bits.bit_order != "lsb_first"
                || !bits.continuous_across_frames
                || bits.stored_values_per_frame == 0
                || bits.total_stored_values == 0
                || bits
                    .stored_values_per_frame
                    .checked_mul(self.decoded_frame_lengths.len() as u64)
                    != Some(bits.total_stored_values)
                || self
                    .decoded_frame_lengths
                    .iter()
                    .any(|length| *length != bits.stored_values_per_frame)
                || bits.packed_size_bytes != self.size_bytes
                || bits.unused_trailing_bits > 7
                || bits
                    .packed_size_bytes
                    .checked_mul(8)
                    .and_then(|value| value.checked_sub(u64::from(bits.unused_trailing_bits)))
                    != Some(bits.total_stored_values)
            {
                return Err(EvidenceError::NativeBitPacking(self.slot.clone()));
            }
        }
        match (
            self.native_value_field_size_bytes,
            &self.native_value_field_sha256,
        ) {
            (None, None) => {}
            (Some(value_size), Some(value_sha256)) => {
                validate_sha256("native Pixel Data Value Field", value_sha256)?;
                let expected_size = self
                    .size_bytes
                    .checked_add(self.size_bytes % 2)
                    .ok_or_else(|| EvidenceError::NativeValueField(self.slot.clone()))?;
                if self.fragment_count != 0
                    || self.native_frame_sha256.is_empty()
                    || value_size != expected_size
                    || (self.size_bytes % 2 == 0 && value_sha256 != &self.sha256)
                {
                    return Err(EvidenceError::NativeValueField(self.slot.clone()));
                }
            }
            _ => return Err(EvidenceError::NativeValueField(self.slot.clone())),
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResultStatus {
    Passed,
    Failed,
    Unavailable,
    NotRequired,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValidationResult {
    pub rule_id: String,
    pub layer: String,
    pub required: bool,
    pub status: ResultStatus,
    pub message: String,
    #[serde(default)]
    pub details: BTreeMap<String, serde_json::Value>,
}

impl ValidationResult {
    fn validate(&self) -> Result<(), EvidenceError> {
        validate_identifier("validation rule", &self.rule_id)?;
        validate_identifier("validation layer", &self.layer)?;
        validate_message("validation result", &self.message)
    }
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
pub struct ObligationResult {
    pub obligation_id: String,
    pub route_id: String,
    pub independence: EvidenceIndependence,
    pub required: bool,
    pub status: ResultStatus,
    pub message: String,
    pub tool: Option<ToolEvidence>,
}

impl ObligationResult {
    fn validate(&self) -> Result<(), EvidenceError> {
        validate_identifier("obligation", &self.obligation_id)?;
        validate_identifier("evidence route", &self.route_id)?;
        validate_message("obligation result", &self.message)?;
        if let Some(tool) = &self.tool {
            tool.validate()?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolEvidence {
    pub tool_id: String,
    pub version: String,
    pub executable_sha256: String,
}

impl ToolEvidence {
    fn validate(&self) -> Result<(), EvidenceError> {
        validate_identifier("tool ID", &self.tool_id)?;
        validate_identifier("tool version", &self.version)?;
        validate_sha256("tool executable", &self.executable_sha256)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderEvidence {
    pub provider_id: String,
    pub provider_version: String,
    pub status: ResultStatus,
    pub executable_sha256: Option<String>,
    pub argument_sha256: String,
    pub request_sha256: String,
    pub response_sha256: String,
    pub outputs: BTreeMap<String, String>,
    #[serde(default)]
    pub claims: BTreeMap<String, serde_json::Value>,
}

impl ProviderEvidence {
    fn validate(&self) -> Result<(), EvidenceError> {
        validate_identifier("provider ID", &self.provider_id)?;
        validate_identifier("provider version", &self.provider_version)?;
        if let Some(hash) = &self.executable_sha256 {
            validate_sha256("provider executable", hash)?;
        }
        for (label, hash) in [
            ("provider arguments", &self.argument_sha256),
            ("provider request", &self.request_sha256),
            ("provider response", &self.response_sha256),
        ] {
            validate_sha256(label, hash)?;
        }
        if self.outputs.is_empty() {
            return Err(EvidenceError::EmptyProviderOutputs);
        }
        for (slot, hash) in &self.outputs {
            validate_identifier("provider output slot", slot)?;
            validate_sha256("provider output", hash)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CodecEvidence {
    pub backend_id: String,
    pub backend_version: String,
    #[serde(default)]
    pub backend_kind: String,
    #[serde(default)]
    pub display_name: String,
    #[serde(default)]
    pub feature_gate: Option<String>,
    pub slot: String,
    pub request_sha256: String,
    pub transfer_syntax_uid: String,
    pub status: ResultStatus,
    pub determinism: String,
    #[serde(default)]
    pub encoded_frame_sha256: Vec<String>,
    #[serde(default)]
    pub decoded_frame_sha256: Vec<String>,
    #[serde(default)]
    pub metrics: BTreeMap<String, f64>,
    #[serde(default)]
    pub claims: BTreeMap<String, serde_json::Value>,
    pub tool: Option<ToolEvidence>,
}

impl CodecEvidence {
    fn validate(&self) -> Result<(), EvidenceError> {
        validate_identifier("codec backend", &self.backend_id)?;
        validate_identifier("codec version", &self.backend_version)?;
        if !self.backend_kind.is_empty() {
            validate_identifier("codec backend kind", &self.backend_kind)?;
        }
        if self.display_name.trim().is_empty() && !self.backend_kind.is_empty() {
            return Err(EvidenceError::InvalidIdentifier {
                label: "codec display name",
                value: self.display_name.clone(),
            });
        }
        if let Some(feature_gate) = &self.feature_gate {
            validate_identifier("codec feature gate", feature_gate)?;
        }
        validate_identifier("codec slot", &self.slot)?;
        validate_sha256("codec request", &self.request_sha256)?;
        validate_identifier("codec determinism", &self.determinism)?;
        for hash in self
            .encoded_frame_sha256
            .iter()
            .chain(&self.decoded_frame_sha256)
        {
            validate_sha256("codec frame", hash)?;
        }
        if self.metrics.values().any(|value| !value.is_finite()) {
            return Err(EvidenceError::NonFiniteCodecMetric);
        }
        if let Some(tool) = &self.tool {
            tool.validate()?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactResourceEvidence {
    pub planned_output_bytes: u64,
    pub planned_peak_working_bytes: u64,
    pub actual_output_bytes: u64,
    pub actual_peak_working_bytes: Option<u64>,
    pub elapsed_milliseconds: u64,
}

impl ArtifactResourceEvidence {
    fn validate(&self) -> Result<(), EvidenceError> {
        if self.actual_output_bytes > self.planned_output_bytes {
            return Err(EvidenceError::ArtifactOutputLimitExceeded);
        }
        if self
            .actual_peak_working_bytes
            .is_some_and(|actual| actual > self.planned_peak_working_bytes)
        {
            return Err(EvidenceError::ArtifactWorkingLimitExceeded);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunResourceEvidence {
    pub planned_max_artifacts: u64,
    pub planned_max_total_output_bytes: u64,
    pub planned_max_peak_working_bytes: u64,
    pub requested_parallelism: u32,
    pub used_parallelism: u32,
    pub actual_artifact_output_bytes: u64,
    pub actual_publication_bytes: u64,
    pub actual_peak_working_bytes: Option<u64>,
}

impl RunResourceEvidence {
    fn validate(&self) -> Result<(), EvidenceError> {
        if self.planned_max_artifacts == 0
            || self.planned_max_total_output_bytes == 0
            || self.planned_max_peak_working_bytes == 0
            || self.requested_parallelism == 0
            || self.used_parallelism == 0
            || self.used_parallelism > self.requested_parallelism
        {
            return Err(EvidenceError::InvalidRunResourceEnvelope);
        }
        if self.actual_publication_bytes > self.planned_max_total_output_bytes
            || self.actual_artifact_output_bytes > self.actual_publication_bytes
            || self
                .actual_peak_working_bytes
                .is_some_and(|actual| actual > self.planned_max_peak_working_bytes)
        {
            return Err(EvidenceError::RunResourceLimitExceeded);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UnavailableExecutionEvidence {
    pub capability_id: String,
    pub kind: String,
    pub reason_code: String,
    pub message: String,
    #[serde(default)]
    pub affected_artifact_ids: Vec<String>,
}

impl UnavailableExecutionEvidence {
    fn validate(&self) -> Result<(), EvidenceError> {
        validate_identifier("capability ID", &self.capability_id)?;
        validate_identifier("capability kind", &self.kind)?;
        validate_identifier("unavailable reason", &self.reason_code)?;
        validate_message("unavailable capability", &self.message)?;
        validate_unique_results("unavailable artifact", self.affected_artifact_ids.iter())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PublicationState {
    NotStarted,
    Staging,
    ManifestReady,
    Promoted,
    Cancelled,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublicationEvidence {
    pub manifest_relative_path: String,
    pub state: PublicationState,
    pub private_staging: bool,
    pub no_overwrite: bool,
    pub validation_complete: bool,
    pub cleanup_complete: bool,
    pub manifest_sha256: Option<String>,
}

impl PublicationEvidence {
    fn validate(&self) -> Result<(), EvidenceError> {
        validate_relative_path(&self.manifest_relative_path)?;
        if !self.private_staging || !self.no_overwrite {
            return Err(EvidenceError::UnsafePublication);
        }
        if let Some(hash) = &self.manifest_sha256 {
            validate_sha256("manifest", hash)?;
        }
        if matches!(
            self.state,
            PublicationState::ManifestReady | PublicationState::Promoted
        ) && (!self.validation_complete || self.manifest_sha256.is_none())
        {
            return Err(EvidenceError::IncompleteManifestEvidence);
        }
        Ok(())
    }
}

fn validate_unique_results<'a>(
    label: &'static str,
    values: impl IntoIterator<Item = &'a String>,
) -> Result<(), EvidenceError> {
    let mut unique = BTreeSet::new();
    for value in values {
        validate_identifier(label, value)?;
        if !unique.insert(value) {
            return Err(EvidenceError::DuplicateResult {
                label,
                value: value.clone(),
            });
        }
    }
    Ok(())
}

fn validate_identifier(label: &'static str, value: &str) -> Result<(), EvidenceError> {
    if value.is_empty()
        || value.len() > 256
        || value.contains('\0')
        || value.chars().any(char::is_control)
    {
        return Err(EvidenceError::InvalidIdentifier {
            label,
            value: value.to_owned(),
        });
    }
    Ok(())
}

fn validate_message(label: &'static str, value: &str) -> Result<(), EvidenceError> {
    if value.trim().is_empty() || value.contains('\0') {
        return Err(EvidenceError::InvalidIdentifier {
            label,
            value: value.to_owned(),
        });
    }
    Ok(())
}

fn validate_sha256(label: &'static str, value: &str) -> Result<(), EvidenceError> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(EvidenceError::InvalidSha256 {
            label,
            value: value.to_owned(),
        });
    }
    Ok(())
}

fn validate_relative_path(value: &str) -> Result<(), EvidenceError> {
    let path = std::path::Path::new(value);
    if value.is_empty()
        || value.contains('\\')
        || path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                std::path::Component::ParentDir
                    | std::path::Component::CurDir
                    | std::path::Component::RootDir
                    | std::path::Component::Prefix(_)
            )
        })
    {
        return Err(EvidenceError::UnsafeRelativePath(value.to_owned()));
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvidenceError {
    UnsupportedImportedDicomObservationVersion(String),
    ImportedDicomObservationBounds,
    UnsupportedSchemaVersion(String),
    ArtifactOrderMismatch {
        expected: Vec<String>,
        actual: Vec<String>,
    },
    DuplicateArtifact(String),
    DuplicateArtifactOrder(u64),
    DuplicateOutputPath(String),
    MissingInstancePlanHash(String),
    IncompleteDicomEvidence(String),
    DuplicateResult {
        label: &'static str,
        value: String,
    },
    InvalidIdentifier {
        label: &'static str,
        value: String,
    },
    InvalidSha256 {
        label: &'static str,
        value: String,
    },
    FrameEvidenceCardinality(String),
    FragmentEvidenceCardinality(String),
    FragmentEvidenceArithmetic(String),
    NativeBitPacking(String),
    NativeValueField(String),
    MaterializationServiceEvidenceBounds,
    UnsafeRelativePath(String),
    ArtifactOutputLimitExceeded,
    ArtifactWorkingLimitExceeded,
    InvalidRunResourceEnvelope,
    RunResourceLimitExceeded,
    ResourceOverflow,
    ArtifactResourceMismatch {
        logical_id: String,
    },
    ArtifactOutputTotalMismatch {
        expected: u64,
        actual: u64,
    },
    DuplicateUnavailableCapability(String),
    NonFiniteCodecMetric,
    EmptyProviderOutputs,
    UnsafePublication,
    IncompleteManifestEvidence,
    PromotedWithIncompleteArtifact,
    IncompletePromotionEvidence,
}

impl fmt::Display for EvidenceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for EvidenceError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn encapsulated_content() -> MaterializedContentEvidence {
        MaterializedContentEvidence {
            slot: "pixels".into(),
            kind: "encapsulated_pixels".into(),
            vr: "OB".into(),
            size_bytes: 8,
            sha256: "a".repeat(64),
            basic_offset_table: vec![],
            compressed_frame_sha256: vec!["b".repeat(64), "c".repeat(64)],
            native_frame_sha256: vec![],
            native_frame_lengths: vec![],
            decoded_frame_sha256: vec![],
            decoded_frame_lengths: vec![],
            fragment_count: 2,
            compressed_lengths: vec![3, 4],
            padded_fragment_lengths: vec![4, 4],
            fragments_per_frame: vec![1, 1],
            fragments: vec![
                MaterializedFragmentEvidence {
                    frame_index: 0,
                    item_start_offset: 8,
                    compressed_length: 3,
                    padded_length: 4,
                },
                MaterializedFragmentEvidence {
                    frame_index: 1,
                    item_start_offset: 20,
                    compressed_length: 4,
                    padded_length: 4,
                },
            ],
            extended_offset_table: vec![],
            extended_offset_table_lengths: vec![],
            native_byte_order: None,
            native_bit_packing: None,
            native_value_field_size_bytes: None,
            native_value_field_sha256: None,
            writer_materialization: None,
        }
    }

    fn imported_observation() -> ImportedDicomObservation {
        ImportedDicomObservation {
            schema_version: IMPORTED_DICOM_OBSERVATION_SCHEMA_VERSION.into(),
            sop_class_uid: "1.2.3".into(),
            sop_instance_uid: "1.2.3.4".into(),
            transfer_syntax_uid: "1.2.840.10008.1.2.1".into(),
            study_instance_uid: Some("1.2.5".into()),
            series_instance_uid: Some("1.2.6".into()),
            frame_of_reference_uid: Some("1.2.7".into()),
            rows: Some(2),
            columns: Some(2),
            frames: Some(1),
            content: vec![ImportedDicomContentObservation {
                tag: "7FE0,0010".into(),
                vr: "OB".into(),
                size_bytes: 4,
                sha256: crate::sha256_hex(&[0, 1, 2, 3]),
            }],
            references: vec![],
        }
    }

    #[test]
    fn imported_observation_is_strict_and_bounded() {
        imported_observation().validate().unwrap();
        let mut value = serde_json::to_value(imported_observation()).unwrap();
        value
            .as_object_mut()
            .unwrap()
            .insert("unknown".into(), 1.into());
        assert!(serde_json::from_value::<ImportedDicomObservation>(value).is_err());

        let mut oversized = imported_observation();
        oversized.references = (0..=MAX_IMPORTED_DICOM_REFERENCES)
            .map(|index| ImportedDicomReferenceObservation {
                role: format!("source_{index}"),
                sop_class_uid: "1.2.3".into(),
                sop_instance_uid: format!("1.2.3.{index}"),
                frame_numbers: vec![1],
            })
            .collect();
        assert!(matches!(
            oversized.validate(),
            Err(EvidenceError::ImportedDicomObservationBounds)
        ));

        let mut invalid = imported_observation();
        invalid.rows = Some(0);
        assert!(matches!(
            invalid.validate(),
            Err(EvidenceError::ImportedDicomObservationBounds)
        ));
        let mut invalid = imported_observation();
        invalid.references.push(ImportedDicomReferenceObservation {
            role: "source".into(),
            sop_class_uid: "1.2.3".into(),
            sop_instance_uid: "1.2.3.8".into(),
            frame_numbers: vec![1, 1],
        });
        assert!(matches!(
            invalid.validate(),
            Err(EvidenceError::ImportedDicomObservationBounds)
        ));
    }

    #[test]
    fn typed_fragment_evidence_validates_exact_item_arithmetic() {
        encapsulated_content().validate_encoding_facts().unwrap();
        let mut changed = encapsulated_content();
        changed.fragments[1].item_start_offset += 2;
        assert!(matches!(
            changed.validate_encoding_facts(),
            Err(EvidenceError::FragmentEvidenceArithmetic(slot)) if slot == "pixels"
        ));
        let mut changed = encapsulated_content();
        changed.extended_offset_table = vec![0, 13];
        changed.extended_offset_table_lengths = vec![3, 4];
        assert!(matches!(
            changed.validate_encoding_facts(),
            Err(EvidenceError::FragmentEvidenceArithmetic(slot)) if slot == "pixels"
        ));
    }

    #[test]
    fn typed_fragment_and_u1_evidence_reject_cardinality_and_bit_drift() {
        let mut changed = encapsulated_content();
        changed.fragments_per_frame = vec![2];
        assert!(matches!(
            changed.validate_encoding_facts(),
            Err(EvidenceError::FragmentEvidenceCardinality(slot)) if slot == "pixels"
        ));

        let mut native = MaterializedContentEvidence {
            slot: "pixels".into(),
            kind: "native_pixels".into(),
            vr: "OB".into(),
            size_bytes: 3,
            sha256: "d".repeat(64),
            basic_offset_table: vec![],
            compressed_frame_sha256: vec![],
            native_frame_sha256: vec!["e".repeat(64), "f".repeat(64)],
            native_frame_lengths: vec![2, 1],
            decoded_frame_sha256: vec!["1".repeat(64), "2".repeat(64)],
            decoded_frame_lengths: vec![9, 9],
            fragment_count: 0,
            compressed_lengths: vec![],
            padded_fragment_lengths: vec![],
            fragments_per_frame: vec![],
            fragments: vec![],
            extended_offset_table: vec![],
            extended_offset_table_lengths: vec![],
            native_byte_order: None,
            native_bit_packing: Some(NativeBitPackingEvidence {
                bit_order: "lsb_first".into(),
                continuous_across_frames: true,
                stored_values_per_frame: 9,
                total_stored_values: 18,
                packed_size_bytes: 3,
                unused_trailing_bits: 6,
            }),
            native_value_field_size_bytes: Some(4),
            native_value_field_sha256: Some("3".repeat(64)),
            writer_materialization: None,
        };
        native.validate_encoding_facts().unwrap();
        native
            .native_bit_packing
            .as_mut()
            .unwrap()
            .unused_trailing_bits = 5;
        assert!(matches!(
            native.validate_encoding_facts(),
            Err(EvidenceError::NativeBitPacking(slot)) if slot == "pixels"
        ));

        let mut odd_size_drift = native.clone();
        odd_size_drift
            .native_bit_packing
            .as_mut()
            .unwrap()
            .unused_trailing_bits = 6;
        odd_size_drift.native_value_field_size_bytes = Some(3);
        assert!(matches!(
            odd_size_drift.validate_encoding_facts(),
            Err(EvidenceError::NativeValueField(slot)) if slot == "pixels"
        ));

        let mut even_hash_drift = native;
        even_hash_drift.size_bytes = 4;
        even_hash_drift.sha256 = "4".repeat(64);
        even_hash_drift.native_bit_packing = None;
        even_hash_drift.native_value_field_size_bytes = Some(4);
        even_hash_drift.native_value_field_sha256 = Some("5".repeat(64));
        assert!(matches!(
            even_hash_drift.validate_encoding_facts(),
            Err(EvidenceError::NativeValueField(slot)) if slot == "pixels"
        ));
    }
}
