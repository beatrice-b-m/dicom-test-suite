use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use base64::Engine;

use super::request::resolve_address;
use super::{
    AssemblyBulk, AssemblyElement, AssemblyError, AssemblyRequest, AssemblyValue, BulkSource,
};
use crate::composition::{
    AttributeAddress, AttributeItem, AttributeOperation, AttributeValue, CanonicalContent,
    CompositionUidRole, ContentMaterialization, ContentPlacement, DicomVr, IdentityPlan,
    MaterializedReference, PrimitiveValue, ResolvedAttribute, ResolvedInstancePlan, TemplateId,
    TemplateVersion, ValueOrigin,
};
use crate::corpus_plan::{
    ArtifactDependency, ArtifactProvenance, ArtifactResourceEstimate, CORPUS_PLAN_SCHEMA_VERSION,
    CorpusPlan, EncodingPlan, EvidenceIndependence, EvidenceObligation, EvidencePlan,
    FileMetaPolicy, FragmentationPolicy, ImplementationIdentityPlan, ItemLengthPolicy,
    OffsetTablePolicy, OutputPlan, OutputRelativePath, PlannedArtifact, PlannedDicomArtifact,
    PreamblePolicy, PublicationPlan, PublicationTransaction, ResourcePlan, SequenceLengthPolicy,
    ValidationPlan, ValidationRequirement, ValidationRule,
};
use crate::uid::{DeterministicUidInput, UidRole, deterministic_uid};
use crate::{IMPLEMENTATION_VERSION_NAME, sha256_hex};

#[derive(Debug, Clone)]
pub struct AssemblyPlan {
    pub request_sha256: String,
    pub corpus: CorpusPlan,
    pub instances: BTreeMap<String, ResolvedInstancePlan>,
}

pub fn plan_assembly(
    request_bytes: &[u8],
    caller_asset_root: &Path,
    seed: u64,
    parallelism: u32,
    resource_identity_sha256: &str,
) -> Result<AssemblyPlan, AssemblyError> {
    let request = AssemblyRequest::from_slice(request_bytes)?;
    if parallelism == 0 || parallelism > request.limits.max_parallelism {
        return Err(AssemblyError::Limit("parallelism"));
    }
    let request_sha256 = sha256_hex(request_bytes);
    let implementation_uid = uid(
        resource_identity_sha256,
        "product",
        seed,
        0,
        UidRole::ImplementationClass,
    );
    let mut identities = BTreeMap::new();
    for (index, instance) in request.instances.iter().enumerate() {
        let index = u32::try_from(index).map_err(|_| AssemblyError::Limit("instance index"))?;
        let identity = &instance.identity;
        let study_key = identity
            .study_scope
            .as_deref()
            .unwrap_or(&instance.instance_id);
        let series_key = identity
            .series_scope
            .as_deref()
            .unwrap_or(&instance.instance_id);
        let frame_key = identity
            .frame_of_reference_scope
            .as_deref()
            .unwrap_or(&instance.instance_id);
        identities.insert(
            instance.instance_id.clone(),
            ResolvedIdentities {
                study: identity.study_instance_uid.clone().unwrap_or_else(|| {
                    uid(
                        resource_identity_sha256,
                        study_key,
                        seed,
                        0,
                        UidRole::StudyInstance,
                    )
                }),
                series: identity.series_instance_uid.clone().unwrap_or_else(|| {
                    uid(
                        resource_identity_sha256,
                        series_key,
                        seed,
                        0,
                        UidRole::SeriesInstance,
                    )
                }),
                sop: identity.sop_instance_uid.clone().unwrap_or_else(|| {
                    uid(
                        resource_identity_sha256,
                        &instance.instance_id,
                        seed,
                        index,
                        UidRole::SopInstance,
                    )
                }),
                frame: identity.frame_of_reference_uid.clone().unwrap_or_else(|| {
                    uid(
                        resource_identity_sha256,
                        frame_key,
                        seed,
                        0,
                        UidRole::FrameOfReference,
                    )
                }),
            },
        );
    }
    let mut seen_uids = BTreeSet::new();
    for identity in identities.values() {
        if !seen_uids.insert(identity.sop.clone()) {
            return Err(AssemblyError::Value("duplicate SOP Instance UID".into()));
        }
    }

    let mut resolved = BTreeMap::new();
    for instance in &request.instances {
        let ids = &identities[&instance.instance_id];
        let mut attributes = resolved_elements(&instance.elements)?;
        push_string(&mut attributes, "0020,000D", DicomVr::UI, &ids.study)?;
        push_string(&mut attributes, "0020,000E", DicomVr::UI, &ids.series)?;
        push_string(&mut attributes, "0020,0052", DicomVr::UI, &ids.frame)?;
        if let Some(modality) = &instance.modality {
            push_string(&mut attributes, "0008,0060", DicomVr::CS, modality)?;
        }
        let content = instance
            .bulk
            .iter()
            .enumerate()
            .map(|(i, bulk)| {
                resolve_bulk(bulk, i, caller_asset_root, &request.limits, &mut attributes)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let identity_plan = IdentityPlan::from_exact_values(
            &instance.instance_id,
            [
                (CompositionUidRole::StudyInstance, 0, ids.study.clone()),
                (CompositionUidRole::SeriesInstance, 0, ids.series.clone()),
                (CompositionUidRole::SopInstance, 0, ids.sop.clone()),
                (CompositionUidRole::FrameOfReference, 0, ids.frame.clone()),
                (
                    CompositionUidRole::ImplementationClass,
                    0,
                    implementation_uid.clone(),
                ),
            ],
        )
        .map_err(|error| AssemblyError::Value(error.to_string()))?;
        let references = instance
            .references
            .iter()
            .map(|reference| {
                let target = request
                    .instances
                    .iter()
                    .find(|candidate| candidate.instance_id == reference.target_instance_id)
                    .expect("request validation closed references");
                let target_ids = &identities[&reference.target_instance_id];
                MaterializedReference {
                    source_instance_id: instance.instance_id.clone(),
                    target_instance_id: reference.target_instance_id.clone(),
                    role: reference.relationship.clone(),
                    frame_role: Some(format!("{:?}", reference.target_role).to_ascii_lowercase()),
                    referenced_sop_class_uid: target.sop_class_uid.clone(),
                    referenced_sop_instance_uid: target_ids.sop.clone(),
                    referenced_frames: reference.frames.clone().unwrap_or_default(),
                }
            })
            .collect();
        resolved.insert(
            instance.instance_id.clone(),
            ResolvedInstancePlan {
                plan_schema_version: "0.1.0".into(),
                instance_id: instance.instance_id.clone(),
                template_id: TemplateId("structural/assembly".into()),
                template_version: TemplateVersion {
                    major: 1,
                    minor: 0,
                    patch: 0,
                },
                sop_class_uid: instance.sop_class_uid.clone(),
                transfer_syntax_uid: instance.transfer_syntax_uid.clone(),
                identities: identity_plan,
                attributes,
                content,
                references,
            },
        );
    }

    let per_artifact = request
        .limits
        .max_output_bytes
        .checked_div(request.instances.len() as u64)
        .unwrap_or(1)
        .max(1);
    let artifacts = request
        .instances
        .iter()
        .enumerate()
        .map(|(index, instance)| {
            let plan = resolved[&instance.instance_id].clone();
            Ok(PlannedArtifact::Dicom(PlannedDicomArtifact {
                logical_id: instance.instance_id.clone(),
                order: index as u64,
                provenance: ArtifactProvenance::Requested,
                case_binding: None,
                instance: plan,
                output: OutputPlan {
                    relative_path: OutputRelativePath::new(
                        instance
                            .output_path
                            .clone()
                            .unwrap_or_else(|| format!("instances/{}.dcm", instance.instance_id)),
                    )
                    .map_err(|e| AssemblyError::Value(e.to_string()))?,
                    role: "structural_instance".into(),
                    publish: true,
                },
                encoding: EncodingPlan {
                    transfer_syntax_uid: instance.transfer_syntax_uid.clone(),
                    sequence_length: SequenceLengthPolicy::WriterDefault,
                    item_length: ItemLengthPolicy::WriterDefault,
                    fragmentation: FragmentationPolicy::Native,
                    offset_table: OffsetTablePolicy::NotApplicable,
                    preamble: PreamblePolicy::ZeroFilled,
                    file_meta: FileMetaPolicy::Standard,
                    implementation: ImplementationIdentityPlan {
                        class_uid: implementation_uid.clone(),
                        version_name: Some(IMPLEMENTATION_VERSION_NAME.into()),
                    },
                    backend_id: "structural_part10".into(),
                },
                validation: ValidationPlan {
                    rules: vec![ValidationRule {
                        rule_id: "structural_round_trip".into(),
                        requirement: ValidationRequirement::Required,
                        parameters: BTreeMap::new(),
                    }],
                },
                evidence: EvidencePlan {
                    obligations: vec![EvidenceObligation {
                        obligation_id: "structural_manifest_validation".into(),
                        route_id: "structural_manifest".into(),
                        independence: EvidenceIndependence::SameProject,
                        required: true,
                        parameters: BTreeMap::new(),
                    }],
                },
                resources: ArtifactResourceEstimate {
                    output_bytes: per_artifact,
                    peak_working_bytes: per_artifact,
                },
            }))
        })
        .collect::<Result<Vec<_>, AssemblyError>>()?;
    let dependencies = request
        .instances
        .iter()
        .flat_map(|instance| {
            instance
                .references
                .iter()
                .map(move |reference| ArtifactDependency {
                    artifact_id: instance.instance_id.clone(),
                    depends_on: reference.target_instance_id.clone(),
                    relationship: reference.relationship.clone(),
                    frame_numbers: reference.frames.clone().unwrap_or_default(),
                })
        })
        .collect();
    let corpus = CorpusPlan {
        schema_version: CORPUS_PLAN_SCHEMA_VERSION.into(),
        seed,
        artifacts,
        dependencies,
        unavailable: vec![],
        publication: PublicationPlan {
            manifest_path: OutputRelativePath::new("manifest.json")
                .map_err(|e| AssemblyError::Value(e.to_string()))?,
            transaction: PublicationTransaction::AtomicNoReplace,
            private_staging: true,
            no_overwrite: true,
        },
        resources: ResourcePlan {
            max_artifacts: request.limits.max_instances as u64,
            max_total_output_bytes: request.limits.max_output_bytes,
            max_peak_working_bytes: request.limits.max_output_bytes,
            max_parallelism: request.limits.max_parallelism,
        },
    };
    corpus
        .validate()
        .map_err(|error| AssemblyError::Value(error.to_string()))?;
    Ok(AssemblyPlan {
        request_sha256,
        corpus,
        instances: resolved,
    })
}

#[derive(Debug)]
struct ResolvedIdentities {
    study: String,
    series: String,
    sop: String,
    frame: String,
}

fn uid(resource_hash: &str, key: &str, seed: u64, index: u32, role: UidRole) -> String {
    deterministic_uid(&DeterministicUidInput {
        standards_lock_sha256: resource_hash,
        case_id: key,
        recipe_version: "assembly-1.0.0",
        run_seed: seed,
        file_index: index,
        frame_index: None,
        referenced_object_index: None,
        role,
    })
}

fn resolved_elements(
    elements: &[AssemblyElement],
) -> Result<Vec<ResolvedAttribute>, AssemblyError> {
    let mut private_blocks = BTreeMap::new();
    let mut used_private_blocks = BTreeSet::new();
    elements
        .iter()
        .map(|element| {
            let (mut address, inferred) = resolve_address(&element.address)?;
            allocate_private_address(&mut address, &mut private_blocks, &mut used_private_blocks)?;
            let vr = element
                .vr
                .or(inferred)
                .ok_or_else(|| AssemblyError::VrRequired(address.normalized_tag()))?;
            Ok(ResolvedAttribute {
                address,
                vr,
                value: resolved_value(vr, &element.value)?,
                origin: ValueOrigin::InstanceOverride,
            })
        })
        .collect()
}

fn allocate_private_address(
    address: &mut AttributeAddress,
    private_blocks: &mut BTreeMap<(u16, String), u16>,
    used_private_blocks: &mut BTreeSet<(u16, u16)>,
) -> Result<(), AssemblyError> {
    let Some(creator) = address.private_creator.clone() else {
        return Ok(());
    };
    let key = (address.group, creator);
    let block = if let Some(block) = private_blocks.get(&key) {
        *block
    } else {
        let block = (0x10_u16..=0xFF)
            .find(|block| !used_private_blocks.contains(&(address.group, *block)))
            .ok_or_else(|| AssemblyError::Limit("private creator blocks"))?;
        private_blocks.insert(key, block);
        used_private_blocks.insert((address.group, block));
        block
    };
    address.element = (block << 8) | (address.element & 0x00FF);
    Ok(())
}

fn resolved_value(
    vr: DicomVr,
    value: &AssemblyValue,
) -> Result<Option<AttributeValue>, AssemblyError> {
    let primitive = |value| Some(AttributeValue::Primitive(value));
    Ok(match value {
        AssemblyValue::Empty => None,
        AssemblyValue::String { value } => primitive(PrimitiveValue::String(value.clone())),
        AssemblyValue::Strings { values } => Some(AttributeValue::Multi(
            values.iter().cloned().map(PrimitiveValue::String).collect(),
        )),
        AssemblyValue::Integer { value } => primitive(integer_value(vr, *value)?),
        AssemblyValue::Integers { values } => Some(AttributeValue::Multi(
            values
                .iter()
                .map(|value| integer_value(vr, *value))
                .collect::<Result<Vec<_>, _>>()?,
        )),
        AssemblyValue::Float { value } if vr == DicomVr::FL => {
            primitive(PrimitiveValue::Float32Bits((*value as f32).to_bits()))
        }
        AssemblyValue::Float { value } => primitive(PrimitiveValue::Float64Bits(value.to_bits())),
        AssemblyValue::Floats { values } if vr == DicomVr::FL => Some(AttributeValue::Multi(
            values
                .iter()
                .map(|v| PrimitiveValue::Float32Bits((*v as f32).to_bits()))
                .collect(),
        )),
        AssemblyValue::Floats { values } => Some(AttributeValue::Multi(
            values
                .iter()
                .map(|v| PrimitiveValue::Float64Bits(v.to_bits()))
                .collect(),
        )),
        AssemblyValue::Tag { value } => primitive(PrimitiveValue::Tag(
            AttributeAddress::from_normalized_tag(value)
                .map_err(|e| AssemblyError::Address(e.to_string()))?,
        )),
        AssemblyValue::Tags { values } => Some(AttributeValue::Multi(
            values
                .iter()
                .map(|value| {
                    AttributeAddress::from_normalized_tag(value)
                        .map(PrimitiveValue::Tag)
                        .map_err(|e| AssemblyError::Address(e.to_string()))
                })
                .collect::<Result<Vec<_>, _>>()?,
        )),
        AssemblyValue::Bytes { base64 } => Some(AttributeValue::Binary(
            base64::engine::general_purpose::STANDARD
                .decode(base64)
                .map_err(|_| AssemblyError::Value("invalid base64".into()))?,
        )),
        AssemblyValue::Sequence { items } => Some(AttributeValue::Sequence(
            items
                .iter()
                .map(|item| {
                    let mut private_blocks = BTreeMap::new();
                    let mut used_private_blocks = BTreeSet::new();
                    Ok(AttributeItem {
                        attributes: item
                            .elements
                            .iter()
                            .map(|element| {
                                let (mut address, inferred) = resolve_address(&element.address)?;
                                allocate_private_address(
                                    &mut address,
                                    &mut private_blocks,
                                    &mut used_private_blocks,
                                )?;
                                let vr = element.vr.or(inferred).ok_or_else(|| {
                                    AssemblyError::VrRequired(address.normalized_tag())
                                })?;
                                Ok(match resolved_value(vr, &element.value)? {
                                    Some(value) => AttributeOperation::Set { address, vr, value },
                                    None => AttributeOperation::Set {
                                        address,
                                        vr,
                                        value: AttributeValue::Binary(Vec::new()),
                                    },
                                })
                            })
                            .collect::<Result<Vec<_>, AssemblyError>>()?,
                    })
                })
                .collect::<Result<Vec<_>, AssemblyError>>()?,
        )),
    })
}

fn integer_value(vr: DicomVr, value: i64) -> Result<PrimitiveValue, AssemblyError> {
    if matches!(vr, DicomVr::US | DicomVr::UL | DicomVr::UV) {
        Ok(PrimitiveValue::Unsigned(u64::try_from(value).map_err(
            |_| AssemblyError::Value("unsigned integer is negative".into()),
        )?))
    } else {
        Ok(PrimitiveValue::Signed(value))
    }
}

fn push_string(
    attributes: &mut Vec<ResolvedAttribute>,
    tag: &str,
    vr: DicomVr,
    value: &str,
) -> Result<(), AssemblyError> {
    attributes.push(ResolvedAttribute {
        address: AttributeAddress::from_normalized_tag(tag)
            .map_err(|e| AssemblyError::Address(e.to_string()))?,
        vr,
        value: Some(AttributeValue::Primitive(PrimitiveValue::String(
            value.into(),
        ))),
        origin: ValueOrigin::DerivedStructural,
    });
    Ok(())
}

fn resolve_bulk(
    bulk: &AssemblyBulk,
    index: usize,
    root: &Path,
    limits: &super::AssemblyLimits,
    attributes: &mut Vec<ResolvedAttribute>,
) -> Result<CanonicalContent, AssemblyError> {
    let source = source_bytes(&bulk.source, root)?;
    if source.bytes.len() as u64 > limits.max_value_bytes {
        return Err(AssemblyError::Limit("bulk bytes"));
    }
    validate_bulk_shape(bulk, &source.bytes)?;
    let (tag, vr) = bulk_tag_vr(bulk)?;
    if matches!(
        bulk.kind.as_str(),
        "integer_pixel_data" | "float_pixel_data" | "double_float_pixel_data"
    ) {
        push_number(attributes, "0028,0010", bulk.rows.unwrap_or(1) as u64)?;
        push_number(attributes, "0028,0011", bulk.columns.unwrap_or(1) as u64)?;
        push_number(attributes, "0028,0008", bulk.frames.unwrap_or(1) as u64)?;
        push_number(
            attributes,
            "0028,0002",
            bulk.samples_per_pixel.unwrap_or(1) as u64,
        )?;
        push_number(
            attributes,
            "0028,0100",
            match bulk.kind.as_str() {
                "float_pixel_data" => 32,
                "double_float_pixel_data" => 64,
                _ => bulk.bits_allocated.unwrap_or(8) as u64,
            },
        )?;
        if bulk.kind == "integer_pixel_data" {
            push_number(
                attributes,
                "0028,0101",
                bulk.bits_stored.unwrap_or(bulk.bits_allocated.unwrap_or(8)) as u64,
            )?;
            push_number(
                attributes,
                "0028,0102",
                bulk.bits_stored
                    .unwrap_or(bulk.bits_allocated.unwrap_or(8))
                    .saturating_sub(1) as u64,
            )?;
            push_number(
                attributes,
                "0028,0103",
                u64::from(bulk.signed.unwrap_or(false)),
            )?;
        }
        push_string(
            attributes,
            "0028,0004",
            DicomVr::CS,
            bulk.photometric_interpretation
                .as_deref()
                .unwrap_or("MONOCHROME2"),
        )?;
    } else if bulk.kind == "waveform_data" {
        push_number(attributes, "003A,0005", bulk.channels.unwrap_or(1) as u64)?;
        push_number(attributes, "003A,0010", bulk.samples.unwrap_or(1) as u64)?;
        push_number(
            attributes,
            "5400,1004",
            bulk.bits_allocated.unwrap_or(16) as u64,
        )?;
    } else if bulk.kind == "encapsulated_document" {
        push_string(
            attributes,
            "0042,0012",
            DicomVr::LO,
            bulk.media_type
                .as_deref()
                .unwrap_or("application/octet-stream"),
        )?;
    }
    let resolved_sha256 = sha256_hex(&source.bytes);
    let mut properties = BTreeMap::from([
        ("iod_conformance".into(), "not_assessed".into()),
        ("source_kind".into(), source.kind),
        ("source_sha256".into(), source.sha256),
        ("resolved_sha256".into(), resolved_sha256.clone()),
        (
            "padding".into(),
            if source.bytes.len() % 2 == 0 {
                "none"
            } else {
                "dicom_even_length"
            }
            .into(),
        ),
    ]);
    if let Some(path) = source.path {
        properties.insert("source_path".into(), path);
    }
    for (name, value) in [
        ("rows", bulk.rows.map(u64::from)),
        ("columns", bulk.columns.map(u64::from)),
        ("frames", bulk.frames.map(u64::from)),
        ("samples_per_pixel", bulk.samples_per_pixel.map(u64::from)),
        ("bits_allocated", bulk.bits_allocated.map(u64::from)),
        ("channels", bulk.channels.map(u64::from)),
        ("samples", bulk.samples.map(u64::from)),
    ] {
        if let Some(value) = value {
            properties.insert(name.into(), value.to_string());
        }
    }
    Ok(CanonicalContent {
        slot: format!("bulk_{index}"),
        kind: bulk.kind.clone(),
        address: tag,
        vr,
        size_bytes: source.bytes.len() as u64,
        sha256: resolved_sha256,
        properties,
        placement: ContentPlacement::TopLevel,
        materialization: Some(ContentMaterialization::Inline(source.bytes)),
    })
}

fn push_number(
    attributes: &mut Vec<ResolvedAttribute>,
    tag: &str,
    value: u64,
) -> Result<(), AssemblyError> {
    attributes.push(ResolvedAttribute {
        address: AttributeAddress::from_normalized_tag(tag)
            .map_err(|e| AssemblyError::Address(e.to_string()))?,
        vr: DicomVr::US,
        value: Some(AttributeValue::Primitive(PrimitiveValue::Unsigned(value))),
        origin: ValueOrigin::DerivedStructural,
    });
    Ok(())
}

fn bulk_tag_vr(bulk: &AssemblyBulk) -> Result<(AttributeAddress, DicomVr), AssemblyError> {
    let (tag, vr) = match bulk.kind.as_str() {
        "integer_pixel_data" => ("7FE0,0010", bulk.vr.unwrap_or(DicomVr::OB)),
        "float_pixel_data" => ("7FE0,0008", DicomVr::OF),
        "double_float_pixel_data" => ("7FE0,0009", DicomVr::OD),
        "waveform_data" => ("5400,1010", bulk.vr.unwrap_or(DicomVr::OW)),
        "encapsulated_document" => ("0042,0011", DicomVr::OB),
        "mesh" => ("0066,0023", bulk.vr.unwrap_or(DicomVr::OF)),
        "general" => (
            bulk.tag
                .as_deref()
                .ok_or_else(|| AssemblyError::Value("general bulk tag missing".into()))?,
            bulk.vr
                .ok_or_else(|| AssemblyError::Value("general bulk VR missing".into()))?,
        ),
        _ => return Err(AssemblyError::Value("bulk kind unsupported".into())),
    };
    Ok((
        AttributeAddress::from_normalized_tag(tag)
            .map_err(|e| AssemblyError::Address(e.to_string()))?,
        vr,
    ))
}

struct ResolvedBulkSource {
    bytes: Vec<u8>,
    kind: String,
    path: Option<String>,
    sha256: String,
}

fn source_bytes(source: &BulkSource, root: &Path) -> Result<ResolvedBulkSource, AssemblyError> {
    let (bytes, expected, kind, source_path) = match source {
        BulkSource::InlineBase64 { base64, sha256 } => (
            base64::engine::general_purpose::STANDARD
                .decode(base64)
                .map_err(|_| AssemblyError::Value("bulk base64 invalid".into()))?,
            sha256.as_deref(),
            "inline_base64".to_string(),
            None,
        ),
        BulkSource::File { path, sha256 } => {
            let canonical_root = fs::canonicalize(root)
                .map_err(|e| AssemblyError::Value(format!("caller asset root read failed: {e}")))?;
            let candidate = root.join(path);
            let mut inspected = root.to_path_buf();
            for component in Path::new(path).components() {
                inspected.push(component);
                let metadata = fs::symlink_metadata(&inspected)
                    .map_err(|e| AssemblyError::Value(format!("caller asset read failed: {e}")))?;
                if metadata.file_type().is_symlink() {
                    return Err(AssemblyError::UnsafePath(path.clone()));
                }
            }
            if !fs::metadata(&candidate)
                .map_err(|e| AssemblyError::Value(format!("caller asset read failed: {e}")))?
                .is_file()
            {
                return Err(AssemblyError::Value(
                    "caller asset is not a regular file".into(),
                ));
            }
            let canonical = fs::canonicalize(&candidate)
                .map_err(|e| AssemblyError::Value(format!("caller asset read failed: {e}")))?;
            if !canonical.starts_with(&canonical_root) {
                return Err(AssemblyError::UnsafePath(path.clone()));
            }
            (
                fs::read(canonical)
                    .map_err(|e| AssemblyError::Value(format!("caller asset read failed: {e}")))?,
                Some(sha256.as_str()),
                "file".to_string(),
                Some(path.clone()),
            )
        }
    };
    let observed = sha256_hex(&bytes);
    if expected.is_some_and(|hash| hash != observed) {
        return Err(AssemblyError::Value("caller asset SHA-256 mismatch".into()));
    }
    Ok(ResolvedBulkSource {
        bytes,
        kind,
        path: source_path,
        sha256: observed,
    })
}

fn validate_bulk_shape(bulk: &AssemblyBulk, bytes: &[u8]) -> Result<(), AssemblyError> {
    let actual = bytes.len() as u64;
    let multiply = |values: &[u64]| {
        values
            .iter()
            .try_fold(1_u64, |total, value| total.checked_mul(*value))
            .ok_or(AssemblyError::Limit("bulk shape overflow"))
    };
    let expected = match bulk.kind.as_str() {
        "integer_pixel_data" => {
            let samples = multiply(&[
                bulk.rows.unwrap_or(1) as u64,
                bulk.columns.unwrap_or(1) as u64,
                bulk.frames.unwrap_or(1) as u64,
                bulk.samples_per_pixel.unwrap_or(1) as u64,
            ])?;
            let bits = samples
                .checked_mul(bulk.bits_allocated.unwrap_or(8) as u64)
                .ok_or(AssemblyError::Limit("bulk shape overflow"))?;
            Some(bits.div_ceil(8))
        }
        "float_pixel_data" => Some(multiply(&[
            bulk.rows.unwrap_or(1) as u64,
            bulk.columns.unwrap_or(1) as u64,
            bulk.frames.unwrap_or(1) as u64,
            bulk.samples_per_pixel.unwrap_or(1) as u64,
            4,
        ])?),
        "double_float_pixel_data" => Some(multiply(&[
            bulk.rows.unwrap_or(1) as u64,
            bulk.columns.unwrap_or(1) as u64,
            bulk.frames.unwrap_or(1) as u64,
            bulk.samples_per_pixel.unwrap_or(1) as u64,
            8,
        ])?),
        "waveform_data" => Some(multiply(&[
            bulk.channels.unwrap_or(1) as u64,
            bulk.samples.unwrap_or(1) as u64,
            (bulk.bits_allocated.unwrap_or(16) as u64).div_ceil(8),
        ])?),
        "mesh" if actual % 4 != 0 => {
            return Err(AssemblyError::Value(
                "mesh bulk length must contain complete float32 values".into(),
            ));
        }
        "encapsulated_document" if bulk.media_type.as_deref() == Some("application/pdf") => {
            if !bytes.starts_with(b"%PDF-") {
                return Err(AssemblyError::Value(
                    "PDF document bulk has an invalid signature".into(),
                ));
            }
            None
        }
        _ => None,
    };
    if let Some(expected) = expected {
        if expected != actual {
            return Err(AssemblyError::Value(format!(
                "bulk length mismatch: expected {expected}, got {actual}"
            )));
        }
    }
    Ok(())
}
