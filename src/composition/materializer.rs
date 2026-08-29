use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufWriter, Read, Write};
use std::path::{Path, PathBuf};

use dicom_core::value::{
    DataSetSequence, PixelFragmentSequence, PrimitiveValue as DicomPrimitiveValue,
};
use dicom_core::{DataElement, Tag};
use dicom_dictionary_std::{StandardDataDictionary, tags};
use dicom_object::{FileMetaTableBuilder, InMemDicomObject, open_file};
use dicom_transfer_syntax_registry::{TransferSyntaxIndex, TransferSyntaxRegistry};

use super::{
    AttributeAddress, AttributeItem, AttributeValue, CompositionUidRole, ContentMaterialization,
    DicomVr, PrimitiveValue, ResolvedAttribute, ResolvedInstancePlan,
};
use crate::corpus_plan::{
    EncodingPlan, FileMetaPolicy, ItemLengthPolicy, PreamblePolicy, SequenceLengthPolicy,
};
use crate::{IMPLEMENTATION_VERSION_NAME, sha256_hex};

type Dataset = InMemDicomObject<StandardDataDictionary>;

#[derive(Debug, Default, Clone, Copy)]
pub struct Part10Materializer;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaterializeOutcome {
    pub streamed_slots: Vec<String>,
}

impl Part10Materializer {
    pub fn materialize(
        &self,
        plan: &ResolvedInstancePlan,
        path: impl AsRef<Path>,
    ) -> Result<(), MaterializeError> {
        self.materialize_with_outcome(plan, path).map(|_| ())
    }

    pub fn materialize_with_outcome(
        &self,
        plan: &ResolvedInstancePlan,
        path: impl AsRef<Path>,
    ) -> Result<MaterializeOutcome, MaterializeError> {
        self.materialize_cancellable(plan, path, &|| false)
    }

    pub fn materialize_with_encoding(
        &self,
        plan: &ResolvedInstancePlan,
        encoding: &EncodingPlan,
        path: impl AsRef<Path>,
    ) -> Result<(), MaterializeError> {
        self.materialize_with_encoding_cancellable(plan, encoding, path, &|| false)
            .map(|_| ())
    }

    pub fn materialize_cancellable(
        &self,
        plan: &ResolvedInstancePlan,
        path: impl AsRef<Path>,
        is_cancelled: &dyn Fn() -> bool,
    ) -> Result<MaterializeOutcome, MaterializeError> {
        self.materialize_internal(plan, None, path, is_cancelled)
    }

    /// Materialize a resolved instance using the complete neutral encoding
    /// policy. Compatibility entry points above retain the historical writer
    /// defaults byte-for-byte.
    pub fn materialize_with_encoding_cancellable(
        &self,
        plan: &ResolvedInstancePlan,
        encoding: &EncodingPlan,
        path: impl AsRef<Path>,
        is_cancelled: &dyn Fn() -> bool,
    ) -> Result<MaterializeOutcome, MaterializeError> {
        self.materialize_internal(plan, Some(encoding), path, is_cancelled)
    }

    /// Serialize an inline resolved instance to the exact Part 10 bytes used
    /// by normal materialization without creating or reading any files.
    ///
    /// Staged content is intentionally unsupported: preview is a bounded
    /// validation primitive, not an alternate streaming materializer.
    pub(crate) fn preview_part10_bytes_with_encoding(
        &self,
        plan: &ResolvedInstancePlan,
        encoding: &EncodingPlan,
        max_size_bytes: u64,
    ) -> Result<Vec<u8>, MaterializeError> {
        self.preview_part10_bytes_with_encoding_cancellable(plan, encoding, max_size_bytes, &|| {
            false
        })
    }

    pub(crate) fn preview_part10_bytes_with_encoding_cancellable(
        &self,
        plan: &ResolvedInstancePlan,
        encoding: &EncodingPlan,
        max_size_bytes: u64,
        is_cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<u8>, MaterializeError> {
        check_materialization_cancelled(is_cancelled)?;
        validate_encoding_policy(plan, Some(encoding))?;
        if let Some(content) = plan.content.iter().find(|content| {
            matches!(
                content.materialization,
                Some(ContentMaterialization::StagedFile(_))
            )
        }) {
            return Err(MaterializeError::PreviewStagedContent(content.slot.clone()));
        }
        let file_object = build_file_object(plan, Some(encoding), None)?;
        check_materialization_cancelled(is_cancelled)?;
        let mut writer = BoundedPreviewWriter::new(max_size_bytes, is_cancelled);
        if let Err(error) = serialize_part10(&file_object, Some(encoding), &mut writer) {
            return match writer.failure {
                Some(PreviewWriterFailure::Cancelled) => Err(MaterializeError::Cancelled),
                Some(PreviewWriterFailure::LimitExceeded) => {
                    Err(MaterializeError::PreviewLimitExceeded {
                        limit: max_size_bytes,
                    })
                }
                None => Err(error),
            };
        }
        check_materialization_cancelled(is_cancelled)?;
        Ok(writer.bytes)
    }

    fn materialize_internal(
        &self,
        plan: &ResolvedInstancePlan,
        encoding: Option<&EncodingPlan>,
        path: impl AsRef<Path>,
        is_cancelled: &dyn Fn() -> bool,
    ) -> Result<MaterializeOutcome, MaterializeError> {
        let path = path.as_ref();
        check_materialization_cancelled(is_cancelled)?;
        validate_encoding_policy(plan, encoding)?;
        if path.exists() {
            return Err(MaterializeError::OutputExists(path.to_path_buf()));
        }
        let parent = path
            .parent()
            .ok_or_else(|| MaterializeError::MissingParent(path.to_path_buf()))?;
        fs::create_dir_all(parent).map_err(|source| MaterializeError::Io {
            path: parent.to_path_buf(),
            source,
        })?;

        let deferred = plan
            .content
            .iter()
            .filter(|content| {
                matches!(content.placement, super::ContentPlacement::TopLevel)
                    && matches!(
                        content.materialization,
                        Some(ContentMaterialization::StagedFile(_))
                    )
            })
            .collect::<Vec<_>>();
        let stream_content = if deferred.len() == 1
            && plan.transfer_syntax_uid == crate::part10_locator::EXPLICIT_VR_LITTLE_ENDIAN_UID
        {
            Some(deferred[0])
        } else {
            None
        };
        let file_object = build_file_object(
            plan,
            encoding,
            stream_content.map(|content| content.slot.as_str()),
        )?;
        check_materialization_cancelled(is_cancelled)?;
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(|source| MaterializeError::Io {
                path: path.to_path_buf(),
                source,
            })?;
        let mut writer = BufWriter::new(file);
        serialize_part10(&file_object, encoding, &mut writer)?;
        writer.flush().map_err(|source| MaterializeError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        check_materialization_cancelled(is_cancelled)?;

        if let Some(content) = stream_content {
            stream_staged_content(path, content, is_cancelled)?;
        }

        check_materialization_cancelled(is_cancelled)?;
        let reopened =
            open_file(path).map_err(|error| MaterializeError::Dicom(error.to_string()))?;
        let sop_instance_uid = plan
            .identities
            .get(&CompositionUidRole::SopInstance, 0)
            .ok_or(MaterializeError::MissingIdentity("sop_instance_uid"))?;
        let implementation_class_uid = plan
            .identities
            .get(&CompositionUidRole::ImplementationClass, 0)
            .ok_or(MaterializeError::MissingIdentity(
                "implementation_class_uid",
            ))?;
        let implementation_version = encoding
            .map(|value| value.implementation.version_name.as_deref())
            .unwrap_or(Some(IMPLEMENTATION_VERSION_NAME));
        if reopened.meta().transfer_syntax() != plan.transfer_syntax_uid
            || reopened.meta().media_storage_sop_class_uid() != plan.sop_class_uid
            || reopened.meta().media_storage_sop_instance_uid() != sop_instance_uid
        {
            return Err(MaterializeError::IdentityRoundTrip);
        }
        if let Some(encoding) = encoding {
            if reopened.meta().implementation_class_uid() != implementation_class_uid
                || reopened.meta().implementation_version_name.as_deref() != implementation_version
            {
                return Err(MaterializeError::IdentityRoundTrip);
            }
            let bytes = fs::read(path).map_err(|source| MaterializeError::Io {
                path: path.to_path_buf(),
                source,
            })?;
            let preamble = bytes
                .get(..128)
                .ok_or_else(|| MaterializeError::Dicom("truncated Part 10 preamble".into()))?;
            let valid = match encoding.preamble {
                PreamblePolicy::ZeroFilled => preamble.iter().all(|byte| *byte == 0),
                PreamblePolicy::DeterministicNonZero => preamble.iter().all(|byte| *byte != 0),
            };
            if !valid {
                return Err(MaterializeError::PreambleRoundTrip);
            }
        }
        Ok(MaterializeOutcome {
            streamed_slots: stream_content
                .map(|content| vec![content.slot.clone()])
                .unwrap_or_default(),
        })
    }
}

fn build_file_object(
    plan: &ResolvedInstancePlan,
    encoding: Option<&EncodingPlan>,
    deferred_slot: Option<&str>,
) -> Result<dicom_object::FileDicomObject<Dataset>, MaterializeError> {
    let mut object = build_dataset(plan, deferred_slot)?;
    let sop_instance_uid = plan
        .identities
        .get(&CompositionUidRole::SopInstance, 0)
        .ok_or(MaterializeError::MissingIdentity("sop_instance_uid"))?;
    let implementation_class_uid = plan
        .identities
        .get(&CompositionUidRole::ImplementationClass, 0)
        .ok_or(MaterializeError::MissingIdentity(
            "implementation_class_uid",
        ))?;
    ensure_string(
        &mut object,
        tags::SOP_CLASS_UID,
        DicomVr::UI,
        &plan.sop_class_uid,
    )?;
    ensure_string(
        &mut object,
        tags::SOP_INSTANCE_UID,
        DicomVr::UI,
        sop_instance_uid,
    )?;
    let implementation_version = encoding
        .map(|value| value.implementation.version_name.as_deref())
        .unwrap_or(Some(IMPLEMENTATION_VERSION_NAME));
    let mut meta = FileMetaTableBuilder::new()
        .transfer_syntax(&plan.transfer_syntax_uid)
        .implementation_class_uid(implementation_class_uid);
    if let Some(version) = implementation_version {
        meta = meta.implementation_version_name(version);
    }
    object
        .with_meta(meta)
        .map_err(|error| MaterializeError::Dicom(error.to_string()))
}

fn validate_encoding_policy(
    plan: &ResolvedInstancePlan,
    encoding: Option<&EncodingPlan>,
) -> Result<(), MaterializeError> {
    let Some(encoding) = encoding else {
        return Ok(());
    };
    encoding
        .validate()
        .map_err(|error| MaterializeError::UnsupportedEncodingPolicy(error.to_string()))?;
    if encoding.transfer_syntax_uid != plan.transfer_syntax_uid {
        return Err(MaterializeError::UnsupportedEncodingPolicy(
            "encoding and resolved-plan transfer syntaxes differ".into(),
        ));
    }
    let implementation = plan
        .identities
        .get(&CompositionUidRole::ImplementationClass, 0)
        .ok_or(MaterializeError::MissingIdentity(
            "implementation_class_uid",
        ))?;
    if implementation != encoding.implementation.class_uid {
        return Err(MaterializeError::UnsupportedEncodingPolicy(
            "encoding and resolved-plan implementation class UIDs differ".into(),
        ));
    }
    if !matches!(encoding.file_meta, FileMetaPolicy::Standard) {
        return Err(MaterializeError::UnsupportedEncodingPolicy(
            "unsupported File Meta policy".into(),
        ));
    }
    if matches!(
        encoding.sequence_length,
        SequenceLengthPolicy::PreserveDeclared
    ) || matches!(encoding.item_length, ItemLengthPolicy::PreserveDeclared)
    {
        return Err(MaterializeError::UnsupportedEncodingPolicy(
            "preserve-declared lengths require a declared-length source".into(),
        ));
    }
    let requires_length_rewrite = encoding.sequence_length != SequenceLengthPolicy::WriterDefault
        || encoding.item_length != ItemLengthPolicy::WriterDefault;
    if requires_length_rewrite
        && encoding.transfer_syntax_uid != crate::part10_locator::EXPLICIT_VR_LITTLE_ENDIAN_UID
    {
        return Err(MaterializeError::UnsupportedEncodingPolicy(
            "explicit sequence/item length control currently requires Explicit VR Little Endian"
                .into(),
        ));
    }
    Ok(())
}

fn serialize_part10(
    file_object: &dicom_object::FileDicomObject<Dataset>,
    encoding: Option<&EncodingPlan>,
    writer: &mut dyn Write,
) -> Result<(), MaterializeError> {
    if encoding.is_none()
        || encoding.is_some_and(|value| {
            value.preamble == PreamblePolicy::ZeroFilled
                && value.sequence_length == SequenceLengthPolicy::WriterDefault
                && value.item_length == ItemLengthPolicy::WriterDefault
        })
    {
        return file_object
            .write_all(writer)
            .map_err(|error| MaterializeError::Dicom(error.to_string()));
    }
    write_with_encoding_to(
        file_object,
        encoding.expect("non-default encoding checked above"),
        writer,
    )
}

fn write_with_encoding_to(
    file_object: &dicom_object::FileDicomObject<Dataset>,
    encoding: &EncodingPlan,
    writer: &mut dyn Write,
) -> Result<(), MaterializeError> {
    let transfer_syntax = TransferSyntaxRegistry
        .get(&encoding.transfer_syntax_uid)
        .ok_or_else(|| {
            MaterializeError::UnsupportedEncodingPolicy(format!(
                "unregistered transfer syntax {}",
                encoding.transfer_syntax_uid
            ))
        })?;
    let mut dataset = Vec::new();
    file_object
        .write_dataset_with_ts(&mut dataset, transfer_syntax)
        .map_err(|error| MaterializeError::Dicom(error.to_string()))?;
    if encoding.sequence_length != SequenceLengthPolicy::WriterDefault
        || encoding.item_length != ItemLengthPolicy::WriterDefault
    {
        dataset = rewrite_explicit_vr_le_lengths(
            &dataset,
            encoding.sequence_length,
            encoding.item_length,
        )?;
    }

    let preamble = match encoding.preamble {
        PreamblePolicy::ZeroFilled => [0_u8; 128],
        PreamblePolicy::DeterministicNonZero => {
            let mut value = [0_u8; 128];
            let seed = encoding.implementation.class_uid.as_bytes();
            for (index, byte) in value.iter_mut().enumerate() {
                *byte = seed[index % seed.len()].max(1);
            }
            value
        }
    };
    writer
        .write_all(&preamble)
        .and_then(|_| writer.write_all(b"DICM"))
        .map_err(|error| MaterializeError::Dicom(error.to_string()))?;
    file_object
        .write_meta(&mut *writer)
        .map_err(|error| MaterializeError::Dicom(error.to_string()))?;
    writer
        .write_all(&dataset)
        .and_then(|_| writer.flush())
        .map_err(|error| MaterializeError::Dicom(error.to_string()))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PreviewWriterFailure {
    Cancelled,
    LimitExceeded,
}

struct BoundedPreviewWriter<'a> {
    bytes: Vec<u8>,
    limit: u64,
    is_cancelled: &'a dyn Fn() -> bool,
    failure: Option<PreviewWriterFailure>,
}

impl<'a> BoundedPreviewWriter<'a> {
    fn new(limit: u64, is_cancelled: &'a dyn Fn() -> bool) -> Self {
        Self {
            bytes: Vec::new(),
            limit,
            is_cancelled,
            failure: None,
        }
    }
}

impl Write for BoundedPreviewWriter<'_> {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        if (self.is_cancelled)() {
            self.failure = Some(PreviewWriterFailure::Cancelled);
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "Part 10 preview cancelled",
            ));
        }
        let projected = (self.bytes.len() as u64)
            .checked_add(buffer.len() as u64)
            .ok_or_else(|| io::Error::other("Part 10 preview size overflow"))?;
        if projected > self.limit {
            self.failure = Some(PreviewWriterFailure::LimitExceeded);
            return Err(io::Error::new(
                io::ErrorKind::StorageFull,
                "Part 10 preview exceeds its byte limit",
            ));
        }
        self.bytes.extend_from_slice(buffer);
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn rewrite_explicit_vr_le_lengths(
    input: &[u8],
    sequence_policy: SequenceLengthPolicy,
    item_policy: ItemLengthPolicy,
) -> Result<Vec<u8>, MaterializeError> {
    rewrite_explicit_dataset(input, sequence_policy, item_policy, None).map(|(bytes, _)| bytes)
}

fn rewrite_explicit_dataset(
    input: &[u8],
    sequence_policy: SequenceLengthPolicy,
    item_policy: ItemLengthPolicy,
    stop_tag: Option<[u8; 4]>,
) -> Result<(Vec<u8>, usize), MaterializeError> {
    let mut output = Vec::new();
    let mut offset = 0usize;
    while offset < input.len() {
        let tag: [u8; 4] = input
            .get(offset..offset + 4)
            .and_then(|value| value.try_into().ok())
            .ok_or_else(|| MaterializeError::Dicom("truncated dataset tag".into()))?;
        if stop_tag == Some(tag) {
            let end = offset
                .checked_add(8)
                .ok_or(MaterializeError::NumericRange)?;
            if input.get(offset + 4..end) != Some(&[0, 0, 0, 0]) {
                return Err(MaterializeError::Dicom(
                    "invalid sequence/item delimiter".into(),
                ));
            }
            return Ok((output, end));
        }
        if tag[0..2] == [0xfe, 0xff] {
            return Err(MaterializeError::Dicom(
                "unexpected item token in dataset".into(),
            ));
        }
        let vr = input
            .get(offset + 4..offset + 6)
            .ok_or_else(|| MaterializeError::Dicom("truncated explicit VR".into()))?;
        let long = matches!(
            vr,
            b"OB" | b"OD" | b"OF" | b"OL" | b"OV" | b"OW" | b"SQ" | b"UC" | b"UR" | b"UT" | b"UN"
        );
        let header_len = if long { 12 } else { 8 };
        let length = if long {
            u32::from_le_bytes(
                input
                    .get(offset + 8..offset + 12)
                    .and_then(|value| value.try_into().ok())
                    .ok_or_else(|| MaterializeError::Dicom("truncated value length".into()))?,
            )
        } else {
            u16::from_le_bytes(
                input
                    .get(offset + 6..offset + 8)
                    .and_then(|value| value.try_into().ok())
                    .ok_or_else(|| MaterializeError::Dicom("truncated value length".into()))?,
            ) as u32
        };
        if vr == b"SQ" {
            let (sequence_value, consumed) = rewrite_sequence(
                &input[offset + header_len..],
                length,
                sequence_policy,
                item_policy,
            )?;
            output.extend_from_slice(&input[offset..offset + 8]);
            let defined = matches!(sequence_policy, SequenceLengthPolicy::Defined);
            output.extend_from_slice(
                &(if defined {
                    u32::try_from(sequence_value.len())
                        .map_err(|_| MaterializeError::NumericRange)?
                } else {
                    u32::MAX
                })
                .to_le_bytes(),
            );
            output.extend_from_slice(&sequence_value);
            if !defined {
                output.extend_from_slice(&[0xfe, 0xff, 0xdd, 0xe0, 0, 0, 0, 0]);
            }
            offset = offset
                .checked_add(header_len)
                .and_then(|value| value.checked_add(consumed))
                .ok_or(MaterializeError::NumericRange)?;
        } else if length == u32::MAX {
            // Encapsulated Pixel Data is already fully governed by the
            // fragmentation policy and must remain byte-identical here.
            let end = find_undefined_value_end(&input[offset + header_len..])?;
            output.extend_from_slice(&input[offset..offset + header_len + end]);
            offset += header_len + end;
        } else {
            let end = offset
                .checked_add(header_len)
                .and_then(|value| value.checked_add(length as usize))
                .ok_or(MaterializeError::NumericRange)?;
            let bytes = input
                .get(offset..end)
                .ok_or_else(|| MaterializeError::Dicom("truncated element value".into()))?;
            output.extend_from_slice(bytes);
            offset = end;
        }
    }
    if stop_tag.is_some() {
        return Err(MaterializeError::Dicom("missing dataset delimiter".into()));
    }
    Ok((output, offset))
}

fn rewrite_sequence(
    input: &[u8],
    declared_length: u32,
    sequence_policy: SequenceLengthPolicy,
    item_policy: ItemLengthPolicy,
) -> Result<(Vec<u8>, usize), MaterializeError> {
    let limit = if declared_length == u32::MAX {
        input.len()
    } else {
        usize::try_from(declared_length).map_err(|_| MaterializeError::NumericRange)?
    };
    let mut output = Vec::new();
    let mut offset = 0usize;
    while offset < limit {
        let tag = input
            .get(offset..offset + 4)
            .ok_or_else(|| MaterializeError::Dicom("truncated sequence item".into()))?;
        if tag == [0xfe, 0xff, 0xdd, 0xe0] {
            return Ok((output, offset + 8));
        }
        if tag != [0xfe, 0xff, 0x00, 0xe0] {
            return Err(MaterializeError::Dicom(
                "sequence item tag is missing".into(),
            ));
        }
        let length = u32::from_le_bytes(
            input
                .get(offset + 4..offset + 8)
                .and_then(|value| value.try_into().ok())
                .ok_or_else(|| MaterializeError::Dicom("truncated item length".into()))?,
        );
        let (item, consumed) = if length == u32::MAX {
            rewrite_explicit_dataset(
                &input[offset + 8..],
                sequence_policy,
                item_policy,
                Some([0xfe, 0xff, 0x0d, 0xe0]),
            )?
        } else {
            let length = length as usize;
            let (item, _) = rewrite_explicit_dataset(
                input
                    .get(offset + 8..offset + 8 + length)
                    .ok_or_else(|| MaterializeError::Dicom("truncated item value".into()))?,
                sequence_policy,
                item_policy,
                None,
            )?;
            (item, length)
        };
        let defined = matches!(item_policy, ItemLengthPolicy::Defined);
        output.extend_from_slice(&[0xfe, 0xff, 0x00, 0xe0]);
        output.extend_from_slice(
            &(if defined {
                u32::try_from(item.len()).map_err(|_| MaterializeError::NumericRange)?
            } else {
                u32::MAX
            })
            .to_le_bytes(),
        );
        output.extend_from_slice(&item);
        if !defined {
            output.extend_from_slice(&[0xfe, 0xff, 0x0d, 0xe0, 0, 0, 0, 0]);
        }
        offset += 8 + consumed;
        if declared_length != u32::MAX && offset == limit {
            return Ok((output, offset));
        }
    }
    if declared_length == u32::MAX {
        Err(MaterializeError::Dicom("missing sequence delimiter".into()))
    } else {
        Ok((output, offset))
    }
}

fn find_undefined_value_end(input: &[u8]) -> Result<usize, MaterializeError> {
    let mut offset = 0usize;
    loop {
        let tag = input
            .get(offset..offset + 4)
            .ok_or_else(|| MaterializeError::Dicom("truncated undefined value".into()))?;
        let length = u32::from_le_bytes(
            input
                .get(offset + 4..offset + 8)
                .and_then(|value| value.try_into().ok())
                .ok_or_else(|| MaterializeError::Dicom("truncated item length".into()))?,
        ) as usize;
        offset = offset
            .checked_add(8)
            .ok_or(MaterializeError::NumericRange)?;
        if tag == [0xfe, 0xff, 0xdd, 0xe0] {
            return Ok(offset);
        }
        offset = offset
            .checked_add(length)
            .ok_or(MaterializeError::NumericRange)?;
    }
}

fn build_dataset(
    plan: &ResolvedInstancePlan,
    deferred_slot: Option<&str>,
) -> Result<Dataset, MaterializeError> {
    let mut object = Dataset::new_empty();
    let mut creators = BTreeMap::new();
    let mut element_tags = BTreeSet::new();
    let mut attributes = plan.attributes.clone();
    for content in &plan.content {
        if let super::ContentPlacement::Nested { sequence_path } = &content.placement {
            let bytes = materialized_primitive_bytes(content)?;
            inject_nested_content(&mut attributes, sequence_path, content, bytes)?;
        }
    }
    let content_tags = plan
        .content
        .iter()
        .filter(|content| matches!(content.placement, super::ContentPlacement::TopLevel))
        .map(|content| content.address.clone())
        .collect::<BTreeSet<_>>();
    for attribute in &attributes {
        if attribute.address.group == 0x0002 {
            return Err(MaterializeError::StructuralConflict(
                attribute.address.normalized_tag(),
            ));
        }
        if !element_tags.insert(attribute.address.clone()) {
            return Err(MaterializeError::DuplicateElement(
                attribute.address.normalized_tag(),
            ));
        }
        if content_tags.contains(&attribute.address) {
            return Err(MaterializeError::DuplicateElement(
                attribute.address.normalized_tag(),
            ));
        }
        put_resolved_attribute(&mut object, attribute, &mut creators)?;
    }
    for content in &plan.content {
        if !matches!(content.placement, super::ContentPlacement::TopLevel) {
            continue;
        }
        if content.address.group == 0x0002 || !element_tags.insert(content.address.clone()) {
            return Err(MaterializeError::DuplicateElement(
                content.address.normalized_tag(),
            ));
        }
        let materialization = content
            .materialization
            .as_ref()
            .ok_or_else(|| MaterializeError::MissingContent(content.slot.clone()))?;
        if let ContentMaterialization::Encapsulated {
            basic_offset_table,
            fragments,
        } = materialization
        {
            let bytes = fragments.concat();
            validate_content_bytes(content, &bytes)?;
            put_private_creator(&mut object, &content.address, &mut creators)?;
            object.put(DataElement::new(
                content.address.tag(),
                content.vr.as_dicom(),
                PixelFragmentSequence::new(basic_offset_table.clone(), fragments.clone()),
            ));
            continue;
        }
        let deferred = deferred_slot == Some(content.slot.as_str());
        let bytes = if deferred {
            Vec::new()
        } else {
            materialized_primitive_bytes(content)?
        };
        if !deferred {
            validate_content_bytes(content, &bytes)?;
        }
        put_private_creator(&mut object, &content.address, &mut creators)?;
        object.put(DataElement::new(
            content.address.tag(),
            content.vr.as_dicom(),
            DicomPrimitiveValue::from(bytes),
        ));
    }
    Ok(object)
}

fn stream_staged_content(
    path: &Path,
    content: &super::CanonicalContent,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<(), MaterializeError> {
    check_materialization_cancelled(is_cancelled)?;
    let Some(ContentMaterialization::StagedFile(staged_path)) = &content.materialization else {
        return Err(MaterializeError::MissingContent(content.slot.clone()));
    };
    let skeleton = fs::read(path).map_err(|source| MaterializeError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let located = crate::part10_locator::locate_explicit_vr_le_part10(
        &skeleton,
        crate::part10_locator::LocatorLimits::default(),
    )
    .map_err(|error| MaterializeError::Dicom(error.to_string()))?;
    let tag = crate::part10_locator::Tag(content.address.group, content.address.element);
    let element = located
        .first(tag)
        .ok_or_else(|| MaterializeError::MissingContent(content.slot.clone()))?;
    if element.depth != 0 || element.declared_length != Some(0) {
        return Err(MaterializeError::InvalidContentPlacement(
            content.slot.clone(),
        ));
    }
    let padded_size = content
        .size_bytes
        .checked_add(content.size_bytes & 1)
        .ok_or(MaterializeError::NumericRange)?;
    let encoded_size = u32::try_from(padded_size).map_err(|_| MaterializeError::NumericRange)?;
    if element.length_field.len() != 4 {
        return Err(MaterializeError::Dicom(
            "streaming bulk content requires a 32-bit Explicit VR length field".into(),
        ));
    }

    let temporary = path.with_extension("dts-streaming");
    let mut destination = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|source| MaterializeError::Io {
            path: temporary.clone(),
            source,
        })?;
    destination
        .write_all(&skeleton[..element.length_field.start])
        .and_then(|_| destination.write_all(&encoded_size.to_le_bytes()))
        .and_then(|_| {
            destination.write_all(&skeleton[element.length_field.end..element.value.start])
        })
        .map_err(|source| MaterializeError::Io {
            path: temporary.clone(),
            source,
        })?;
    let mut staged = File::open(staged_path).map_err(|source| MaterializeError::Io {
        path: staged_path.clone(),
        source,
    })?;
    let mut hasher = super::content::StreamingSha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    let mut size = 0_u64;
    loop {
        check_materialization_cancelled(is_cancelled)?;
        let read = staged
            .read(&mut buffer)
            .map_err(|source| MaterializeError::Io {
                path: staged_path.clone(),
                source,
            })?;
        if read == 0 {
            break;
        }
        size = size
            .checked_add(read as u64)
            .ok_or(MaterializeError::NumericRange)?;
        if size > content.size_bytes {
            return Err(MaterializeError::ContentSize {
                slot: content.slot.clone(),
                expected: content.size_bytes,
                actual: size,
            });
        }
        hasher.update(&buffer[..read]);
        destination
            .write_all(&buffer[..read])
            .map_err(|source| MaterializeError::Io {
                path: temporary.clone(),
                source,
            })?;
    }
    let sha256 = hasher.finish_hex();
    if size != content.size_bytes {
        return Err(MaterializeError::ContentSize {
            slot: content.slot.clone(),
            expected: content.size_bytes,
            actual: size,
        });
    }
    if sha256 != content.sha256 {
        return Err(MaterializeError::ContentHash {
            slot: content.slot.clone(),
            expected: content.sha256.clone(),
            actual: sha256,
        });
    }
    if content.size_bytes & 1 == 1 {
        destination
            .write_all(&[0])
            .map_err(|source| MaterializeError::Io {
                path: temporary.clone(),
                source,
            })?;
    }
    destination
        .write_all(&skeleton[element.value.end..])
        .and_then(|_| destination.flush())
        .map_err(|source| MaterializeError::Io {
            path: temporary.clone(),
            source,
        })?;
    fs::remove_file(path).map_err(|source| MaterializeError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    fs::rename(&temporary, path).map_err(|source| MaterializeError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(())
}

fn check_materialization_cancelled(
    is_cancelled: &dyn Fn() -> bool,
) -> Result<(), MaterializeError> {
    if is_cancelled() {
        Err(MaterializeError::Cancelled)
    } else {
        Ok(())
    }
}

fn materialized_primitive_bytes(
    content: &super::CanonicalContent,
) -> Result<Vec<u8>, MaterializeError> {
    let materialization = content
        .materialization
        .as_ref()
        .ok_or_else(|| MaterializeError::MissingContent(content.slot.clone()))?;
    let bytes = match materialization {
        ContentMaterialization::Inline(bytes) => bytes.clone(),
        ContentMaterialization::StagedFile(path) => {
            fs::read(path).map_err(|source| MaterializeError::Io {
                path: path.clone(),
                source,
            })?
        }
        ContentMaterialization::Encapsulated { .. } => {
            return Err(MaterializeError::NestedEncapsulatedContent(
                content.slot.clone(),
            ));
        }
    };
    validate_content_bytes(content, &bytes)?;
    Ok(bytes)
}

fn inject_nested_content(
    attributes: &mut [ResolvedAttribute],
    path: &[super::SequenceItemPlacement],
    content: &super::CanonicalContent,
    bytes: Vec<u8>,
) -> Result<(), MaterializeError> {
    let Some((head, tail)) = path.split_first() else {
        return Err(MaterializeError::InvalidContentPlacement(
            content.slot.clone(),
        ));
    };
    let attribute = attributes
        .iter_mut()
        .find(|attribute| attribute.address == head.sequence)
        .ok_or_else(|| MaterializeError::InvalidContentPlacement(content.slot.clone()))?;
    let Some(AttributeValue::Sequence(items)) = attribute.value.as_mut() else {
        return Err(MaterializeError::InvalidContentPlacement(
            content.slot.clone(),
        ));
    };
    let item = items
        .get_mut(head.item_index)
        .ok_or_else(|| MaterializeError::InvalidContentPlacement(content.slot.clone()))?;
    inject_nested_item(item, tail, content, bytes)
}

fn inject_nested_item(
    item: &mut AttributeItem,
    path: &[super::SequenceItemPlacement],
    content: &super::CanonicalContent,
    bytes: Vec<u8>,
) -> Result<(), MaterializeError> {
    if let Some((head, tail)) = path.split_first() {
        let operation = item
            .attributes
            .iter_mut()
            .find(|operation| {
                matches!(operation, super::AttributeOperation::Set { address, .. } if address == &head.sequence)
            })
            .ok_or_else(|| MaterializeError::InvalidContentPlacement(content.slot.clone()))?;
        let super::AttributeOperation::Set {
            value: AttributeValue::Sequence(items),
            ..
        } = operation
        else {
            return Err(MaterializeError::InvalidContentPlacement(
                content.slot.clone(),
            ));
        };
        let nested = items
            .get_mut(head.item_index)
            .ok_or_else(|| MaterializeError::InvalidContentPlacement(content.slot.clone()))?;
        return inject_nested_item(nested, tail, content, bytes);
    }

    if let Some(operation) = item.attributes.iter_mut().find(|operation| {
        matches!(operation, super::AttributeOperation::Set { address, .. } if address == &content.address)
    }) {
        *operation = super::AttributeOperation::Set {
            address: content.address.clone(),
            vr: content.vr,
            value: AttributeValue::Binary(bytes),
        };
    } else {
        item.attributes.push(super::AttributeOperation::Set {
            address: content.address.clone(),
            vr: content.vr,
            value: AttributeValue::Binary(bytes),
        });
        fn address(operation: &super::AttributeOperation) -> &AttributeAddress {
            match operation {
                super::AttributeOperation::Set { address, .. }
                | super::AttributeOperation::Remove { address }
                | super::AttributeOperation::Empty { address, .. } => address,
            }
        }
        item.attributes.sort_by(|left, right| {
            address(left).cmp(address(right))
        });
    }
    Ok(())
}

fn validate_content_bytes(
    content: &super::CanonicalContent,
    bytes: &[u8],
) -> Result<(), MaterializeError> {
    if bytes.len() as u64 != content.size_bytes {
        return Err(MaterializeError::ContentSize {
            slot: content.slot.clone(),
            expected: content.size_bytes,
            actual: bytes.len() as u64,
        });
    }
    let actual_hash = sha256_hex(bytes);
    if actual_hash != content.sha256 {
        return Err(MaterializeError::ContentHash {
            slot: content.slot.clone(),
            expected: content.sha256.clone(),
            actual: actual_hash,
        });
    }
    Ok(())
}

fn put_resolved_attribute(
    object: &mut Dataset,
    attribute: &ResolvedAttribute,
    creators: &mut BTreeMap<Tag, String>,
) -> Result<(), MaterializeError> {
    put_private_creator(object, &attribute.address, creators)?;
    let value: dicom_core::value::Value<Dataset, Vec<u8>> = match &attribute.value {
        None => DicomPrimitiveValue::Empty.into(),
        Some(AttributeValue::Primitive(value)) => primitive(value, attribute.vr)?.into(),
        Some(AttributeValue::Multi(values)) => multi(values, attribute.vr)?.into(),
        Some(AttributeValue::EncodedText(bytes)) => DicomPrimitiveValue::from(bytes.clone()).into(),
        Some(AttributeValue::Binary(bytes)) => DicomPrimitiveValue::from(bytes.clone()).into(),
        Some(AttributeValue::Sequence(items)) => {
            let items = items
                .iter()
                .map(build_item)
                .collect::<Result<Vec<_>, _>>()?;
            DataSetSequence::from(items).into()
        }
    };
    object.put(DataElement::new(
        attribute.address.tag(),
        attribute.vr.as_dicom(),
        value,
    ));
    Ok(())
}

fn build_item(item: &AttributeItem) -> Result<Dataset, MaterializeError> {
    let mut object = Dataset::new_empty();
    let mut creators = BTreeMap::new();
    for operation in &item.attributes {
        let super::AttributeOperation::Set { address, vr, value } = operation else {
            return Err(MaterializeError::UnresolvedNestedOperation);
        };
        put_private_creator(&mut object, address, &mut creators)?;
        let attribute = ResolvedAttribute {
            address: address.clone(),
            vr: *vr,
            value: Some(value.clone()),
            origin: super::ValueOrigin::InstanceOverride,
        };
        put_resolved_attribute(&mut object, &attribute, &mut creators)?;
    }
    Ok(object)
}

fn put_private_creator(
    object: &mut Dataset,
    address: &AttributeAddress,
    creators: &mut BTreeMap<Tag, String>,
) -> Result<(), MaterializeError> {
    let Some(creator) = &address.private_creator else {
        return Ok(());
    };
    let creator_tag = Tag(address.group, address.element >> 8);
    if let Some(previous) = creators.insert(creator_tag, creator.clone()) {
        if previous != *creator {
            return Err(MaterializeError::PrivateCreatorConflict {
                tag: format!("{:04X},{:04X}", creator_tag.group(), creator_tag.element()),
            });
        }
        return Ok(());
    }
    object.put(DataElement::new(
        creator_tag,
        dicom_core::VR::LO,
        creator.as_str(),
    ));
    Ok(())
}

fn primitive(value: &PrimitiveValue, vr: DicomVr) -> Result<DicomPrimitiveValue, MaterializeError> {
    Ok(match value {
        PrimitiveValue::String(value) => DicomPrimitiveValue::from(value.clone()),
        PrimitiveValue::Signed(value) => match vr {
            DicomVr::SS => DicomPrimitiveValue::from(
                i16::try_from(*value).map_err(|_| MaterializeError::NumericRange)?,
            ),
            DicomVr::SL => DicomPrimitiveValue::from(
                i32::try_from(*value).map_err(|_| MaterializeError::NumericRange)?,
            ),
            DicomVr::SV => DicomPrimitiveValue::from(*value),
            _ => return Err(MaterializeError::NumericRange),
        },
        PrimitiveValue::Unsigned(value) => match vr {
            DicomVr::US => DicomPrimitiveValue::from(
                u16::try_from(*value).map_err(|_| MaterializeError::NumericRange)?,
            ),
            DicomVr::UL => DicomPrimitiveValue::from(
                u32::try_from(*value).map_err(|_| MaterializeError::NumericRange)?,
            ),
            DicomVr::UV => DicomPrimitiveValue::from(*value),
            _ => return Err(MaterializeError::NumericRange),
        },
        PrimitiveValue::Float32Bits(value) => DicomPrimitiveValue::from(f32::from_bits(*value)),
        PrimitiveValue::Float64Bits(value) => DicomPrimitiveValue::from(f64::from_bits(*value)),
        PrimitiveValue::Tag(value) => DicomPrimitiveValue::from(value.tag()),
    })
}

fn multi(values: &[PrimitiveValue], vr: DicomVr) -> Result<DicomPrimitiveValue, MaterializeError> {
    if values
        .iter()
        .all(|value| matches!(value, PrimitiveValue::String(_)))
    {
        let joined = values
            .iter()
            .map(|value| match value {
                PrimitiveValue::String(value) => value.as_str(),
                _ => unreachable!(),
            })
            .collect::<Vec<_>>()
            .join("\\");
        return Ok(DicomPrimitiveValue::from(joined));
    }
    macro_rules! numeric_vec {
        ($variant:ident, $source:ident, $type:ty) => {{
            let converted = values
                .iter()
                .map(|value| match value {
                    PrimitiveValue::$source(value) => {
                        <$type>::try_from(*value).map_err(|_| MaterializeError::NumericRange)
                    }
                    _ => Err(MaterializeError::NumericRange),
                })
                .collect::<Result<Vec<$type>, _>>()?;
            DicomPrimitiveValue::$variant(converted.into())
        }};
    }
    Ok(match vr {
        DicomVr::AT => {
            let tags = values
                .iter()
                .map(|value| match value {
                    PrimitiveValue::Tag(value) => Ok(value.tag()),
                    _ => Err(MaterializeError::NumericRange),
                })
                .collect::<Result<Vec<_>, _>>()?;
            DicomPrimitiveValue::Tags(tags.into())
        }
        DicomVr::SS => numeric_vec!(I16, Signed, i16),
        DicomVr::SL => numeric_vec!(I32, Signed, i32),
        DicomVr::SV => numeric_vec!(I64, Signed, i64),
        DicomVr::US => numeric_vec!(U16, Unsigned, u16),
        DicomVr::UL => numeric_vec!(U32, Unsigned, u32),
        DicomVr::UV => numeric_vec!(U64, Unsigned, u64),
        DicomVr::FL => DicomPrimitiveValue::F32(
            values
                .iter()
                .map(|value| match value {
                    PrimitiveValue::Float32Bits(value) => Ok(f32::from_bits(*value)),
                    _ => Err(MaterializeError::NumericRange),
                })
                .collect::<Result<Vec<_>, _>>()?
                .into(),
        ),
        DicomVr::FD => DicomPrimitiveValue::F64(
            values
                .iter()
                .map(|value| match value {
                    PrimitiveValue::Float64Bits(value) => Ok(f64::from_bits(*value)),
                    _ => Err(MaterializeError::NumericRange),
                })
                .collect::<Result<Vec<_>, _>>()?
                .into(),
        ),
        _ => return Err(MaterializeError::NumericRange),
    })
}

fn ensure_string(
    object: &mut Dataset,
    tag: Tag,
    vr: DicomVr,
    expected: &str,
) -> Result<(), MaterializeError> {
    if let Ok(element) = object.element(tag) {
        if element.vr() != vr.as_dicom() || element.to_str().ok().as_deref() != Some(expected) {
            return Err(MaterializeError::StructuralConflict(format!(
                "{:04X},{:04X}",
                tag.group(),
                tag.element()
            )));
        }
    } else {
        object.put(DataElement::new(tag, vr.as_dicom(), expected));
    }
    Ok(())
}

#[derive(Debug)]
pub enum MaterializeError {
    Cancelled,
    OutputExists(PathBuf),
    MissingParent(PathBuf),
    MissingIdentity(&'static str),
    MissingContent(String),
    PreviewStagedContent(String),
    PreviewLimitExceeded {
        limit: u64,
    },
    DuplicateElement(String),
    ContentSize {
        slot: String,
        expected: u64,
        actual: u64,
    },
    ContentHash {
        slot: String,
        expected: String,
        actual: String,
    },
    InvalidContentPlacement(String),
    NestedEncapsulatedContent(String),
    PrivateCreatorConflict {
        tag: String,
    },
    UnresolvedNestedOperation,
    NumericRange,
    StructuralConflict(String),
    UnsupportedEncodingPolicy(String),
    IdentityRoundTrip,
    PreambleRoundTrip,
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    Dicom(String),
}

impl fmt::Display for MaterializeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for MaterializeError {}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

    use super::*;
    use crate::composition::{AttributeItem, IdentityAllocator, TemplateId, ValueOrigin};
    use crate::corpus_plan::{
        FileMetaPolicy, FragmentationPolicy, ImplementationIdentityPlan, ItemLengthPolicy,
        OffsetTablePolicy, PreamblePolicy, SequenceLengthPolicy,
    };

    static NEXT: AtomicU64 = AtomicU64::new(0);
    const LOCK_HASH: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    fn plan(pixel_bytes: Vec<u8>) -> ResolvedInstancePlan {
        let template_id = TemplateId("classic/secondary-capture/monochrome".into());
        let version = "1.0.0".parse().unwrap();
        let identities = IdentityAllocator::new(LOCK_HASH, template_id.clone(), version, 1)
            .unwrap()
            .allocate_plan(
                "primary",
                [
                    (CompositionUidRole::SopInstance, 0),
                    (CompositionUidRole::ImplementationClass, 0),
                ],
            )
            .unwrap();
        let attr = |tag: &str, vr, value: &str| ResolvedAttribute {
            address: AttributeAddress::from_normalized_tag(tag).unwrap(),
            vr,
            value: Some(AttributeValue::Primitive(PrimitiveValue::String(
                value.to_string(),
            ))),
            origin: ValueOrigin::TemplateDefault,
        };
        ResolvedInstancePlan {
            plan_schema_version: "0.1.0".into(),
            instance_id: "primary".into(),
            template_id,
            template_version: version,
            sop_class_uid: "1.2.840.10008.5.1.4.1.1.7".into(),
            transfer_syntax_uid: "1.2.840.10008.1.2.1".into(),
            identities,
            attributes: vec![
                attr("0008,001C", DicomVr::CS, "YES"),
                attr("0010,0010", DicomVr::PN, "DTS^Synthetic"),
                ResolvedAttribute {
                    address: AttributeAddress::from_normalized_tag("0008,1115").unwrap(),
                    vr: DicomVr::SQ,
                    value: Some(AttributeValue::Sequence(vec![AttributeItem {
                        attributes: vec![super::super::AttributeOperation::Set {
                            address: AttributeAddress::from_normalized_tag("0020,000E").unwrap(),
                            vr: DicomVr::UI,
                            value: AttributeValue::Primitive(PrimitiveValue::String(
                                "2.25.99".into(),
                            )),
                        }],
                    }])),
                    origin: ValueOrigin::InstanceOverride,
                },
            ],
            content: vec![super::super::CanonicalContent {
                slot: "pixels".into(),
                kind: "native_pixels".into(),
                address: AttributeAddress::from_normalized_tag("7FE0,0010").unwrap(),
                vr: DicomVr::OB,
                size_bytes: pixel_bytes.len() as u64,
                sha256: sha256_hex(&pixel_bytes),
                properties: BTreeMap::new(),
                placement: super::super::ContentPlacement::TopLevel,
                materialization: Some(ContentMaterialization::Inline(pixel_bytes)),
            }],
            references: vec![],
        }
    }

    fn encoding(plan: &ResolvedInstancePlan) -> EncodingPlan {
        EncodingPlan {
            transfer_syntax_uid: plan.transfer_syntax_uid.clone(),
            sequence_length: SequenceLengthPolicy::WriterDefault,
            item_length: ItemLengthPolicy::WriterDefault,
            fragmentation: FragmentationPolicy::Native,
            offset_table: OffsetTablePolicy::NotApplicable,
            preamble: PreamblePolicy::ZeroFilled,
            file_meta: FileMetaPolicy::Standard,
            implementation: ImplementationIdentityPlan {
                class_uid: plan
                    .identities
                    .get(&CompositionUidRole::ImplementationClass, 0)
                    .unwrap()
                    .into(),
                version_name: Some(IMPLEMENTATION_VERSION_NAME.into()),
            },
            backend_id: "dicom-rs.part10".into(),
        }
    }

    fn representative_inline_plan(
        instance_id: &str,
        sop_class_uid: &str,
        content_kind: &str,
        bytes: Vec<u8>,
    ) -> ResolvedInstancePlan {
        let mut plan = plan(bytes);
        plan.instance_id = instance_id.into();
        plan.sop_class_uid = sop_class_uid.into();
        plan.content[0].kind = content_kind.into();
        plan
    }

    #[test]
    fn bounded_preview_matches_normal_enhanced_and_wsi_materialization_exactly() {
        let sentinel = std::env::temp_dir().join(format!(
            "dts-part10-preview-must-not-create-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        assert!(!sentinel.exists());
        for (name, plan, nondefault) in [
            (
                "enhanced",
                representative_inline_plan(
                    "enhanced-ct",
                    "1.2.840.10008.5.1.4.1.1.2.1",
                    "native_multiframe_pixels",
                    vec![0, 1, 2, 3, 4, 5, 6, 7],
                ),
                false,
            ),
            (
                "wsi",
                representative_inline_plan(
                    "wsi-volume",
                    "1.2.840.10008.5.1.4.1.1.77.1.6",
                    "native_wsi_tiles",
                    vec![10, 20, 30, 40, 50, 60, 70, 80],
                ),
                true,
            ),
        ] {
            let mut encoding = encoding(&plan);
            if nondefault {
                encoding.preamble = PreamblePolicy::DeterministicNonZero;
                encoding.sequence_length = SequenceLengthPolicy::Undefined;
                encoding.item_length = ItemLengthPolicy::Undefined;
            }
            let preview = Part10Materializer
                .preview_part10_bytes_with_encoding(&plan, &encoding, 1024 * 1024)
                .unwrap();
            let path = std::env::temp_dir().join(format!(
                "dts-part10-preview-parity-{name}-{}-{}.dcm",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            Part10Materializer
                .materialize_with_encoding(&plan, &encoding, &path)
                .unwrap();
            assert_eq!(preview, fs::read(&path).unwrap(), "{name} preview drift");
            fs::remove_file(path).unwrap();
        }
        assert!(!sentinel.exists());
    }

    #[test]
    fn preview_rejects_staged_content_without_reading_it() {
        let mut plan = plan(Vec::new());
        plan.content[0].materialization = Some(ContentMaterialization::StagedFile(PathBuf::from(
            "preview-must-not-read-this-file.raw",
        )));
        let encoding = encoding(&plan);
        assert!(matches!(
            Part10Materializer.preview_part10_bytes_with_encoding(
                &plan,
                &encoding,
                1024 * 1024
            ),
            Err(MaterializeError::PreviewStagedContent(slot)) if slot == "pixels"
        ));
    }

    #[test]
    fn preview_enforces_byte_limit_and_cancellation() {
        let plan = plan(vec![0; 4096]);
        let encoding = encoding(&plan);
        let complete = Part10Materializer
            .preview_part10_bytes_with_encoding(&plan, &encoding, 1024 * 1024)
            .unwrap();
        assert!(matches!(
            Part10Materializer.preview_part10_bytes_with_encoding(
                &plan,
                &encoding,
                complete.len() as u64 - 1
            ),
            Err(MaterializeError::PreviewLimitExceeded { limit })
                if limit == complete.len() as u64 - 1
        ));

        let polls = AtomicUsize::new(0);
        assert!(matches!(
            Part10Materializer.preview_part10_bytes_with_encoding_cancellable(
                &plan,
                &encoding,
                1024 * 1024,
                &|| polls.fetch_add(1, Ordering::Relaxed) >= 2,
            ),
            Err(MaterializeError::Cancelled)
        ));
        assert!(polls.load(Ordering::Relaxed) >= 3);
    }

    #[test]
    fn writes_reopenable_part10_only_from_resolved_plan() {
        let path = std::env::temp_dir().join(format!(
            "dts-composition-materializer-{}-{}.dcm",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        Part10Materializer
            .materialize(&plan(vec![0, 1, 2, 3]), &path)
            .unwrap();
        let object = open_file(&path).unwrap();
        assert_eq!(
            object
                .element(tags::SYNTHETIC_DATA)
                .unwrap()
                .to_str()
                .unwrap(),
            "YES"
        );
        assert_eq!(
            object
                .element(tags::PIXEL_DATA)
                .unwrap()
                .to_bytes()
                .unwrap()
                .as_ref(),
            &[0, 1, 2, 3]
        );
        assert_eq!(
            object
                .element(tags::REFERENCED_SERIES_SEQUENCE)
                .unwrap()
                .items()
                .unwrap()
                .len(),
            1
        );
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn writes_character_set_encoded_person_name_without_transcoding() {
        let path = std::env::temp_dir().join(format!(
            "dts-composition-encoded-text-{}-{}.dcm",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let raw = vec![0x1b, 0x24, 0x42, 0x30, 0x21, 0x1b, 0x28, 0x42];
        let mut instance = plan(vec![0, 1, 2, 3]);
        instance
            .attributes
            .iter_mut()
            .find(|attribute| attribute.address.normalized_tag() == "0010,0010")
            .unwrap()
            .value = Some(AttributeValue::EncodedText(raw.clone()));
        Part10Materializer.materialize(&instance, &path).unwrap();
        let object = open_file(&path).unwrap();
        assert_eq!(
            object
                .element(tags::PATIENT_NAME)
                .unwrap()
                .to_bytes()
                .unwrap()
                .as_ref(),
            raw
        );
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn cancellation_interrupts_streamed_content_copy() {
        let suffix = NEXT.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "dts-composition-cancel-stream-{}-{suffix}.dcm",
            std::process::id()
        ));
        let source = std::env::temp_dir().join(format!(
            "dts-composition-cancel-stream-{}-{suffix}.raw",
            std::process::id()
        ));
        let bytes = vec![7_u8; 2 * 1024 * 1024];
        fs::write(&source, &bytes).unwrap();
        let mut plan = plan(Vec::new());
        plan.content[0].size_bytes = bytes.len() as u64;
        plan.content[0].sha256 = sha256_hex(&bytes);
        plan.content[0].materialization = Some(ContentMaterialization::StagedFile(source.clone()));
        let polls = AtomicUsize::new(0);
        let error = Part10Materializer
            .materialize_cancellable(&plan, &path, &|| polls.fetch_add(1, Ordering::Relaxed) >= 5)
            .unwrap_err();
        assert!(matches!(error, MaterializeError::Cancelled));
        assert!(polls.load(Ordering::Relaxed) >= 6);
        let _ = fs::remove_file(path.with_extension("dts-streaming"));
        let _ = fs::remove_file(path);
        fs::remove_file(source).unwrap();
    }

    #[test]
    fn writes_hash_checked_bulk_content_into_a_sequence_item() {
        let path = std::env::temp_dir().join(format!(
            "dts-composition-nested-content-{}-{}.dcm",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let mut plan = plan(vec![0, 1, 2, 3]);
        let waveform_sequence = AttributeAddress::from_normalized_tag("5400,0100").unwrap();
        let waveform_data = AttributeAddress::from_normalized_tag("5400,1010").unwrap();
        plan.attributes.push(ResolvedAttribute {
            address: waveform_sequence.clone(),
            vr: DicomVr::SQ,
            value: Some(AttributeValue::Sequence(vec![AttributeItem {
                attributes: vec![],
            }])),
            origin: ValueOrigin::TemplateDefault,
        });
        let bytes = vec![4, 3, 2, 1];
        plan.content.push(super::super::CanonicalContent {
            slot: "waveform_samples".into(),
            kind: "waveform_samples".into(),
            address: waveform_data,
            vr: DicomVr::OW,
            size_bytes: bytes.len() as u64,
            sha256: sha256_hex(&bytes),
            properties: BTreeMap::new(),
            placement: super::super::ContentPlacement::Nested {
                sequence_path: vec![super::super::SequenceItemPlacement {
                    sequence: waveform_sequence,
                    item_index: 0,
                }],
            },
            materialization: Some(ContentMaterialization::Inline(bytes.clone())),
        });

        Part10Materializer.materialize(&plan, &path).unwrap();
        let object = open_file(&path).unwrap();
        let item = &object
            .element(Tag(0x5400, 0x0100))
            .unwrap()
            .items()
            .unwrap()[0];
        assert_eq!(
            item.element(Tag(0x5400, 0x1010))
                .unwrap()
                .to_bytes()
                .unwrap()
                .as_ref(),
            bytes
        );
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn materializes_multi_valued_attribute_tags_without_string_coercion() {
        let value = multi(
            &[
                PrimitiveValue::Tag(AttributeAddress::from_normalized_tag("0054,0010").unwrap()),
                PrimitiveValue::Tag(AttributeAddress::from_normalized_tag("0054,0020").unwrap()),
            ],
            DicomVr::AT,
        )
        .unwrap();
        assert_eq!(
            value,
            DicomPrimitiveValue::Tags(
                vec![
                    AttributeAddress::from_normalized_tag("0054,0010")
                        .unwrap()
                        .tag(),
                    AttributeAddress::from_normalized_tag("0054,0020")
                        .unwrap()
                        .tag(),
                ]
                .into()
            )
        );
        assert_eq!(
            multi(
                &[
                    PrimitiveValue::Float64Bits(0.75_f64.to_bits()),
                    PrimitiveValue::Float64Bits(1.5_f64.to_bits()),
                ],
                DicomVr::FD,
            )
            .unwrap(),
            DicomPrimitiveValue::F64(vec![0.75, 1.5].into())
        );
    }

    #[test]
    fn rejects_content_hash_drift_before_writing() {
        let path = std::env::temp_dir().join(format!(
            "dts-composition-materializer-bad-{}-{}.dcm",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let mut plan = plan(vec![0, 1]);
        plan.content[0].sha256 = "0".repeat(64);
        assert!(matches!(
            Part10Materializer.materialize(&plan, &path),
            Err(MaterializeError::ContentHash { .. })
        ));
        assert!(!path.exists());
    }

    #[test]
    fn compatibility_encoding_policy_preserves_exact_part10_bytes() {
        let suffix = NEXT.fetch_add(1, Ordering::Relaxed);
        let legacy = std::env::temp_dir().join(format!("dts-part10-legacy-{suffix}.dcm"));
        let encoded = std::env::temp_dir().join(format!("dts-part10-policy-{suffix}.dcm"));
        let plan = plan(vec![0, 1, 2, 3]);
        Part10Materializer.materialize(&plan, &legacy).unwrap();
        Part10Materializer
            .materialize_with_encoding(&plan, &encoding(&plan), &encoded)
            .unwrap();
        assert_eq!(fs::read(&legacy).unwrap(), fs::read(&encoded).unwrap());
        fs::remove_file(legacy).unwrap();
        fs::remove_file(encoded).unwrap();
    }

    #[test]
    fn writes_deterministic_nonzero_preamble_and_planned_file_meta_identity() {
        let path = std::env::temp_dir().join(format!(
            "dts-part10-preamble-{}.dcm",
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let plan = plan(vec![0, 1]);
        let mut encoding = encoding(&plan);
        encoding.preamble = PreamblePolicy::DeterministicNonZero;
        encoding.implementation.version_name = Some("DTSU34".into());
        Part10Materializer
            .materialize_with_encoding(&plan, &encoding, &path)
            .unwrap();
        let bytes = fs::read(&path).unwrap();
        assert!(bytes[..128].iter().all(|byte| *byte != 0));
        assert_eq!(&bytes[128..132], b"DICM");
        let object = open_file(&path).unwrap();
        assert_eq!(
            object.meta().implementation_class_uid(),
            encoding.implementation.class_uid
        );
        assert_eq!(
            object.meta().implementation_version_name.as_deref(),
            Some("DTSU34")
        );
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn writes_exact_defined_and_undefined_sequence_item_headers() {
        for (sequence, item, expected_sequence, expected_item, delimiters) in [
            (
                SequenceLengthPolicy::Defined,
                ItemLengthPolicy::Defined,
                false,
                false,
                false,
            ),
            (
                SequenceLengthPolicy::Undefined,
                ItemLengthPolicy::Undefined,
                true,
                true,
                true,
            ),
        ] {
            let path = std::env::temp_dir().join(format!(
                "dts-part10-lengths-{}.dcm",
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            let plan = plan(vec![0, 1]);
            let mut encoding = encoding(&plan);
            encoding.sequence_length = sequence;
            encoding.item_length = item;
            Part10Materializer
                .materialize_with_encoding(&plan, &encoding, &path)
                .unwrap();
            let bytes = fs::read(&path).unwrap();
            let tag = [0x08, 0x00, 0x15, 0x11, b'S', b'Q', 0, 0];
            let offset = bytes
                .windows(tag.len())
                .position(|value| value == tag)
                .unwrap();
            assert_eq!(
                &bytes[offset + 8..offset + 12] == &[0xff; 4],
                expected_sequence
            );
            assert_eq!(&bytes[offset + 12..offset + 16], &[0xfe, 0xff, 0x00, 0xe0]);
            assert_eq!(
                &bytes[offset + 16..offset + 20] == &[0xff; 4],
                expected_item
            );
            let tail = &bytes[offset..];
            assert_eq!(
                tail.windows(4)
                    .any(|value| value == [0xfe, 0xff, 0x0d, 0xe0]),
                delimiters
            );
            assert_eq!(
                tail.windows(4)
                    .any(|value| value == [0xfe, 0xff, 0xdd, 0xe0]),
                delimiters
            );
            open_file(&path).unwrap();
            fs::remove_file(path).unwrap();
        }
    }

    #[test]
    fn rejects_unrepresentable_policy_before_creating_output_parent() {
        let root = std::env::temp_dir().join(format!(
            "dts-part10-policy-reject-{}",
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let path = root.join("nested/output.dcm");
        let plan = plan(vec![0, 1]);
        let mut encoding = encoding(&plan);
        encoding.sequence_length = SequenceLengthPolicy::PreserveDeclared;
        assert!(matches!(
            Part10Materializer.materialize_with_encoding(&plan, &encoding, &path),
            Err(MaterializeError::UnsupportedEncodingPolicy(_))
        ));
        assert!(!root.exists());
    }

    #[test]
    fn preserves_native_payload_bytes_for_little_and_big_endian_transfer_syntaxes() {
        for (uid, backend, expected_header) in [
            (
                "1.2.840.10008.1.2.1",
                "dicom-rs.part10",
                vec![0xe0, 0x7f, 0x10, 0x00, b'O', b'B', 0, 0, 2, 0, 0, 0],
            ),
            (
                "1.2.840.10008.1.2.2",
                "encoding.native.explicit_vr_big_endian",
                vec![0x7f, 0xe0, 0x00, 0x10, b'O', b'B', 0, 0, 0, 0, 0, 2],
            ),
        ] {
            let path = std::env::temp_dir().join(format!(
                "dts-part10-native-endian-{}.dcm",
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            let mut plan = plan(vec![0x12, 0x34]);
            plan.transfer_syntax_uid = uid.into();
            let mut encoding = encoding(&plan);
            encoding.backend_id = backend.into();
            Part10Materializer
                .materialize_with_encoding(&plan, &encoding, &path)
                .unwrap();
            let bytes = fs::read(&path).unwrap();
            let offset = bytes
                .windows(expected_header.len())
                .position(|value| value == expected_header)
                .unwrap();
            assert_eq!(
                &bytes[offset + expected_header.len()..offset + expected_header.len() + 2],
                &[0x12, 0x34]
            );
            fs::remove_file(path).unwrap();
        }
    }

    #[test]
    fn preserves_signed_and_unsigned_pixel_padding_value_encodings() {
        let path = std::env::temp_dir().join(format!(
            "dts-part10-padding-{}.dcm",
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let mut plan = plan(vec![0, 1]);
        plan.attributes.extend([
            ResolvedAttribute {
                address: AttributeAddress::from_normalized_tag("0028,0120").unwrap(),
                vr: DicomVr::SS,
                value: Some(AttributeValue::Primitive(PrimitiveValue::Signed(-1))),
                origin: ValueOrigin::InstanceOverride,
            },
            ResolvedAttribute {
                address: AttributeAddress::from_normalized_tag("0028,0121").unwrap(),
                vr: DicomVr::US,
                value: Some(AttributeValue::Primitive(PrimitiveValue::Unsigned(65535))),
                origin: ValueOrigin::InstanceOverride,
            },
        ]);
        Part10Materializer
            .materialize_with_encoding(&plan, &encoding(&plan), &path)
            .unwrap();
        let bytes = fs::read(&path).unwrap();
        assert!(
            bytes
                .windows(10)
                .any(|value| { value == [0x28, 0x00, 0x20, 0x01, b'S', b'S', 2, 0, 0xff, 0xff] })
        );
        assert!(
            bytes
                .windows(10)
                .any(|value| { value == [0x28, 0x00, 0x21, 0x01, b'U', b'S', 2, 0, 0xff, 0xff] })
        );
        fs::remove_file(path).unwrap();
    }
}
