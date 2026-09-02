use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use dicom_core::value::Value as DicomValue;
use dicom_object::open_file;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::{CompositionUidRole, ResolvedInstancePlan};
use crate::product_resources::ProductResourceIdentity;
use crate::sha256_hex;

const MANIFEST_SCHEMA: &str = include_str!("../../schemas/composition-manifest.schema.json");

#[derive(Debug, Clone)]
pub struct CompositionManifestInputs {
    pub generated_at: String,
    pub generator: Value,
    pub standards: Value,
    pub dependencies: Value,
    pub product_resources: ProductResourceIdentity,
    pub seed: u64,
    pub composition_spec_schema_version: String,
    pub input_spec_sha256: String,
    pub template_catalog_schema_version: String,
    pub template_catalog_sha256: String,
    pub resource_limits: BTreeMap<String, u64>,
    pub requested_parallelism: u32,
    pub used_parallelism: u32,
}

#[derive(Debug, Clone)]
pub struct ManifestEntryInput<'a> {
    pub plan: &'a ResolvedInstancePlan,
    pub output_path: &'a Path,
    pub relative_path: String,
    pub requested: bool,
    pub bundle_root_instance_id: String,
    pub bundle_role: String,
    pub source_provenance: String,
    pub determinism: String,
}

#[derive(Debug, Clone)]
pub struct EvidenceManifestEntryInput<'a> {
    pub plan: &'a ResolvedInstancePlan,
    pub resolved_plan_sha256: String,
    pub relative_path: String,
    pub size_bytes: u64,
    pub sha256: String,
    pub checks: Vec<ValidationCheck>,
    pub requested: bool,
    pub bundle_root_instance_id: String,
    pub bundle_role: String,
    pub source_provenance: String,
    pub determinism: String,
}

#[derive(Debug, Clone)]
pub struct ImportedEvidenceManifestEntryInput<'a> {
    pub plan: &'a crate::corpus_plan::PlannedImportedDicomArtifact,
    pub observation: &'a crate::executor::evidence::ImportedDicomObservation,
    pub resolved_plan_sha256: String,
    pub relative_path: String,
    pub size_bytes: u64,
    pub sha256: String,
    pub checks: Vec<ValidationCheck>,
    pub requested: bool,
    pub bundle_root_instance_id: String,
    pub bundle_role: String,
    pub source_provenance: String,
}

#[derive(Debug, Clone)]
pub enum MixedEvidenceManifestEntryInput<'a> {
    Native(EvidenceManifestEntryInput<'a>),
    Imported(ImportedEvidenceManifestEntryInput<'a>),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationCheck {
    pub layer: String,
    pub rule_id: String,
    pub status: String,
    pub message: String,
}

impl ValidationCheck {
    fn passed(layer: &str, rule_id: &str, message: impl Into<String>) -> Self {
        Self {
            layer: layer.into(),
            rule_id: rule_id.into(),
            status: "passed".into(),
            message: message.into(),
        }
    }

    fn failed(layer: &str, rule_id: &str, message: impl Into<String>) -> Self {
        Self {
            layer: layer.into(),
            rule_id: rule_id.into(),
            status: "failed".into(),
            message: message.into(),
        }
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct GenericPlanValidator;

impl GenericPlanValidator {
    pub fn validate_file(
        &self,
        plan: &ResolvedInstancePlan,
        path: impl AsRef<Path>,
    ) -> Vec<ValidationCheck> {
        let path = path.as_ref();
        let mut checks = Vec::new();
        let bytes = match fs::read(path) {
            Ok(bytes) => bytes,
            Err(error) => {
                checks.push(ValidationCheck::failed(
                    "part10",
                    "file_readable",
                    format!("cannot read generated file: {error}"),
                ));
                return checks;
            }
        };
        if bytes.get(128..132) == Some(b"DICM") {
            checks.push(ValidationCheck::passed(
                "part10",
                "preamble_and_prefix",
                "128-byte preamble and DICM prefix are present",
            ));
        } else {
            checks.push(ValidationCheck::failed(
                "part10",
                "preamble_and_prefix",
                "missing Part 10 preamble or DICM prefix",
            ));
        }

        let object = match open_file(path) {
            Ok(object) => object,
            Err(error) => {
                checks.push(ValidationCheck::failed(
                    "part10",
                    "reopen",
                    format!("cannot reopen generated DICOM: {error}"),
                ));
                return checks;
            }
        };
        checks.push(ValidationCheck::passed(
            "part10",
            "reopen",
            "generated file reopens as a DICOM Part 10 object",
        ));

        let sop_instance_uid = plan
            .identities
            .get(&CompositionUidRole::SopInstance, 0)
            .unwrap_or("");
        let identity_matches = object.meta().transfer_syntax() == plan.transfer_syntax_uid
            && object.meta().media_storage_sop_class_uid() == plan.sop_class_uid
            && object.meta().media_storage_sop_instance_uid() == sop_instance_uid;
        checks.push(if identity_matches {
            ValidationCheck::passed(
                "part10",
                "meta_identity",
                "file meta SOP and transfer-syntax identities match the resolved plan",
            )
        } else {
            ValidationCheck::failed(
                "part10",
                "meta_identity",
                "file meta SOP or transfer-syntax identity differs from the resolved plan",
            )
        });

        let mut seen = BTreeSet::new();
        let mut attribute_errors = Vec::new();
        let explicit_vr =
            plan.transfer_syntax_uid != dicom_dictionary_std::uids::IMPLICIT_VR_LITTLE_ENDIAN;
        for attribute in &plan.attributes {
            if !seen.insert(attribute.address.clone()) {
                attribute_errors.push(format!(
                    "duplicate resolved attribute {}",
                    attribute.address.normalized_tag()
                ));
                continue;
            }
            match object.element(attribute.address.tag()) {
                // Implicit VR carries no VR on the wire. The reader must
                // recover one from its dictionary, which can intentionally
                // differ from a declared compatibility VR in the plan.
                Ok(_) if !explicit_vr => {}
                Ok(element) if element.vr() == attribute.vr.as_dicom() => {}
                Ok(element) => attribute_errors.push(format!(
                    "{} reopened as {:?}, expected {}",
                    attribute.address.normalized_tag(),
                    element.vr(),
                    attribute.vr
                )),
                Err(error) => attribute_errors.push(format!(
                    "{} is absent: {error}",
                    attribute.address.normalized_tag()
                )),
            }
        }
        checks.push(if attribute_errors.is_empty() {
            ValidationCheck::passed(
                "generic_data_elements",
                "resolved_attributes",
                format!(
                    "{} resolved attributes reopened with their planned VRs",
                    plan.attributes.len()
                ),
            )
        } else {
            ValidationCheck::failed(
                "generic_data_elements",
                "resolved_attributes",
                attribute_errors.join("; "),
            )
        });

        let mut content_errors = Vec::new();
        for content in &plan.content {
            if let super::ContentPlacement::Nested { sequence_path } = &content.placement {
                match nested_content_bytes(&object, sequence_path, &content.address) {
                    Ok((vr, _bytes)) if explicit_vr && vr != content.vr.as_dicom() => {
                        content_errors.push(format!(
                            "{} has an unexpected VR",
                            content.address.normalized_tag()
                        ))
                    }
                    Ok((_, bytes)) => {
                        validate_primitive_content(content, &bytes, &mut content_errors)
                    }
                    Err(error) => content_errors.push(format!(
                        "{} nested content is absent: {error}",
                        content.slot
                    )),
                }
                continue;
            }
            match object.element(content.address.tag()) {
                Ok(element) if explicit_vr && element.vr() != content.vr.as_dicom() => {
                    content_errors.push(format!(
                        "{} has an unexpected VR",
                        content.address.normalized_tag()
                    ))
                }
                Ok(element) if content.kind == "encapsulated_pixels" => match element.value() {
                    DicomValue::PixelSequence(sequence) => {
                        let bytes = sequence.fragments().concat();
                        if bytes.len() as u64 != content.size_bytes
                            || sha256_hex(&bytes) != content.sha256
                        {
                            content_errors.push(format!(
                                "{} encapsulated fragments differ from the resolved plan",
                                content.slot
                            ));
                        }
                    }
                    _ => content_errors
                        .push(format!("{} is not encapsulated Pixel Data", content.slot)),
                },
                Ok(element) => match element.to_bytes() {
                    Ok(bytes) => {
                        validate_primitive_content(content, bytes.as_ref(), &mut content_errors)
                    }
                    Err(error) => content_errors.push(format!(
                        "{} content cannot be decoded: {error}",
                        content.slot
                    )),
                },
                Err(error) => {
                    content_errors.push(format!("{} content is absent: {error}", content.slot))
                }
            }
        }
        checks.push(if content_errors.is_empty() {
            ValidationCheck::passed(
                "content",
                "content_integrity",
                format!(
                    "{} content slots match planned VR, size, and SHA-256",
                    plan.content.len()
                ),
            )
        } else {
            ValidationCheck::failed("content", "content_integrity", content_errors.join("; "))
        });

        let invalid_reference = plan.references.iter().find(|reference| {
            reference.source_instance_id != plan.instance_id
                || reference.referenced_sop_class_uid.is_empty()
                || reference.referenced_sop_instance_uid.is_empty()
        });
        checks.push(if let Some(reference) = invalid_reference {
            ValidationCheck::failed(
                "template",
                "reference_projection",
                format!(
                    "invalid materialized reference to {}",
                    reference.target_instance_id
                ),
            )
        } else {
            ValidationCheck::passed(
                "template",
                "reference_projection",
                format!(
                    "{} materialized references are closed and source-consistent",
                    plan.references.len()
                ),
            )
        });
        checks
    }
}

fn nested_content_bytes(
    object: &dicom_object::DefaultDicomObject,
    sequence_path: &[super::SequenceItemPlacement],
    address: &super::AttributeAddress,
) -> Result<(dicom_core::VR, Vec<u8>), String> {
    let mut dataset: &dicom_object::InMemDicomObject = object;
    for step in sequence_path {
        let sequence = dataset
            .element(step.sequence.tag())
            .map_err(|error| error.to_string())?;
        let items = sequence
            .items()
            .ok_or_else(|| "content path element is not a sequence".to_string())?;
        dataset = items
            .get(step.item_index)
            .ok_or_else(|| format!("sequence item {} is absent", step.item_index))?;
    }
    let element = dataset
        .element(address.tag())
        .map_err(|error| error.to_string())?;
    let vr = element.vr();
    let bytes = element
        .to_bytes()
        .map_err(|error| error.to_string())?
        .into_owned();
    Ok((vr, bytes))
}

fn validate_primitive_content(
    content: &super::CanonicalContent,
    bytes: &[u8],
    errors: &mut Vec<String>,
) {
    let expected_len = content.size_bytes as usize;
    let allowed_padding = bytes.len() == expected_len + 1
        && bytes.last().is_some_and(|byte| *byte == 0 || *byte == b' ');
    if bytes.len() < expected_len
        || (bytes.len() != expected_len && !allowed_padding)
        || sha256_hex(&bytes[..expected_len.min(bytes.len())]) != content.sha256
    {
        errors.push(format!(
            "{} content size or SHA-256 differs from the resolved plan",
            content.slot
        ));
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct CompositionManifestAssembler;

impl CompositionManifestAssembler {
    pub fn assemble_from_mixed_evidence(
        &self,
        inputs: CompositionManifestInputs,
        entries: &[MixedEvidenceManifestEntryInput<'_>],
    ) -> Result<Value, ManifestError> {
        if entries.is_empty() {
            return Err(ManifestError::NoEntries);
        }
        let mut manifest_entries = Vec::with_capacity(entries.len());
        let mut assets = BTreeMap::<String, Value>::new();
        let mut output_bytes = 0_u64;
        for entry in entries {
            match entry {
                MixedEvidenceManifestEntryInput::Native(input) => {
                    if input.checks.iter().any(|check| check.status == "failed") {
                        return Err(ManifestError::ValidationFailed {
                            instance_id: input.plan.instance_id.clone(),
                            checks: input.checks.clone(),
                        });
                    }
                    output_bytes = output_bytes
                        .checked_add(input.size_bytes)
                        .ok_or(ManifestError::OutputSizeOverflow)?;
                    collect_assets(input.plan, &mut assets)?;
                    let provenance = input
                        .plan
                        .attributes
                        .iter()
                        .map(|attribute| json!({"tag": attribute.address.normalized_tag(), "origin": attribute.origin}))
                        .collect::<Vec<_>>();
                    manifest_entries.push(json!({
                        "instance_id": input.plan.instance_id, "template_id": input.plan.template_id,
                        "template_version": input.plan.template_version, "requested": input.requested,
                        "bundle_root_instance_id": input.bundle_root_instance_id, "bundle_role": input.bundle_role,
                        "source_provenance": input.source_provenance, "path": input.relative_path,
                        "construction_origin": "native_plan",
                        "size_bytes": input.size_bytes, "sha256": input.sha256, "determinism": input.determinism,
                        "resolved_plan_sha256": input.resolved_plan_sha256,
                        "dicom": {"sop_class_uid": input.plan.sop_class_uid, "transfer_syntax_uid": input.plan.transfer_syntax_uid},
                        "uids": input.plan.identities.identities, "resolved_attributes": input.plan.attributes,
                        "value_provenance": provenance, "content": input.plan.content,
                        "references": input.plan.references,
                        "validation": {"status":"passed", "checks": input.checks}
                    }));
                }
                MixedEvidenceManifestEntryInput::Imported(input) => {
                    if input.checks.iter().any(|check| check.status == "failed") {
                        return Err(ManifestError::ValidationFailed {
                            instance_id: input.plan.logical_id.clone(),
                            checks: input.checks.clone(),
                        });
                    }
                    output_bytes = output_bytes
                        .checked_add(input.size_bytes)
                        .ok_or(ManifestError::OutputSizeOverflow)?;
                    let semantic = input
                        .plan
                        .provider
                        .parameters
                        .get("semantic_evidence")
                        .cloned()
                        .unwrap_or(Value::Null);
                    let content = input.observation.content.iter().map(|actual| {
                        let kind = match actual.tag.as_str() {
                            "7FE0,0008" => "float_pixels",
                            "7FE0,0009" => "double_float_pixels",
                            _ => "native_pixels",
                        };
                        json!({
                            "slot":"pixels", "kind":kind, "vr":actual.vr,
                            "size_bytes":actual.size_bytes, "sha256":actual.sha256,
                            "materialization": null,
                            "properties": {
                                "bulk_source":"external_provider_import",
                                "semantic_validator":"imported_dicom_observation",
                                "observed_tag":actual.tag,
                                "semantic_evidence": serde_json::to_string(&semantic).expect("semantic evidence serializes")
                            }
                        })
                    }).collect::<Vec<_>>();
                    manifest_entries.push(json!({
                        "instance_id": input.plan.logical_id,
                        "template_id": input.plan.declared_instance.template_id,
                        "template_version": input.plan.declared_instance.template_version,
                        "requested": input.requested,
                        "bundle_root_instance_id": input.bundle_root_instance_id,
                        "bundle_role": input.bundle_role,
                        "source_provenance": input.source_provenance,
                        "construction_origin": "external_provider_import",
                        "path": input.relative_path, "size_bytes": input.size_bytes,
                        "sha256": input.sha256, "determinism":"semantic_stable",
                        "resolved_plan_sha256": input.resolved_plan_sha256,
                        "dicom": {"sop_class_uid": input.observation.sop_class_uid, "transfer_syntax_uid": input.observation.transfer_syntax_uid},
                        "uids": input.plan.declared_instance.identities.identities,
                        "resolved_attributes": [], "value_provenance": [], "content": content,
                        "references": input.plan.declared_instance.references,
                        "validation": {"status":"passed", "checks":input.checks}
                    }));
                }
            }
        }
        finish_manifest(inputs, manifest_entries, assets, output_bytes)
    }

    /// Project an already-validated run without reopening materialized files.
    pub fn assemble_from_evidence(
        &self,
        inputs: CompositionManifestInputs,
        entries: &[EvidenceManifestEntryInput<'_>],
    ) -> Result<Value, ManifestError> {
        if entries.is_empty() {
            return Err(ManifestError::NoEntries);
        }
        let mut manifest_entries = Vec::with_capacity(entries.len());
        let mut assets = BTreeMap::<String, Value>::new();
        let mut output_bytes = 0_u64;
        for input in entries {
            if input.checks.iter().any(|check| check.status == "failed") {
                return Err(ManifestError::ValidationFailed {
                    instance_id: input.plan.instance_id.clone(),
                    checks: input.checks.clone(),
                });
            }
            output_bytes = output_bytes
                .checked_add(input.size_bytes)
                .ok_or(ManifestError::OutputSizeOverflow)?;
            collect_assets(input.plan, &mut assets)?;
            let provenance = input
                .plan
                .attributes
                .iter()
                .map(|attribute| {
                    json!({
                        "tag": attribute.address.normalized_tag(),
                        "origin": attribute.origin
                    })
                })
                .collect::<Vec<_>>();
            manifest_entries.push(json!({
                "instance_id": input.plan.instance_id,
                "template_id": input.plan.template_id,
                "template_version": input.plan.template_version,
                "requested": input.requested,
                "bundle_root_instance_id": input.bundle_root_instance_id,
                "bundle_role": input.bundle_role,
                "source_provenance": input.source_provenance,
                "construction_origin": "native_plan",
                "path": input.relative_path,
                "size_bytes": input.size_bytes,
                "sha256": input.sha256,
                "determinism": input.determinism,
                "resolved_plan_sha256": input.resolved_plan_sha256,
                "dicom": {
                    "sop_class_uid": input.plan.sop_class_uid,
                    "transfer_syntax_uid": input.plan.transfer_syntax_uid
                },
                "uids": input.plan.identities.identities,
                "resolved_attributes": input.plan.attributes,
                "value_provenance": provenance,
                "content": input.plan.content,
                "references": input.plan.references,
                "validation": { "status": "passed", "checks": input.checks }
            }));
        }
        finish_manifest(inputs, manifest_entries, assets, output_bytes)
    }

    pub fn assemble(
        &self,
        inputs: CompositionManifestInputs,
        entries: &[ManifestEntryInput<'_>],
    ) -> Result<Value, ManifestError> {
        if entries.is_empty() {
            return Err(ManifestError::NoEntries);
        }
        let validator = GenericPlanValidator;
        let mut manifest_entries = Vec::with_capacity(entries.len());
        let mut assets = BTreeMap::<String, Value>::new();
        let mut output_bytes = 0_u64;
        for input in entries {
            let bytes = fs::read(input.output_path).map_err(|source| ManifestError::Io {
                path: input.output_path.to_path_buf(),
                source,
            })?;
            output_bytes = output_bytes
                .checked_add(bytes.len() as u64)
                .ok_or(ManifestError::OutputSizeOverflow)?;
            let checks = validator.validate_file(input.plan, input.output_path);
            if checks.iter().any(|check| check.status == "failed") {
                return Err(ManifestError::ValidationFailed {
                    instance_id: input.plan.instance_id.clone(),
                    checks,
                });
            }
            for content in &input.plan.content {
                if let Some(spec_relative_path) = content.properties.get("spec_relative_path") {
                    let asset_id =
                        sha256_hex(format!("{spec_relative_path}\0{}", content.sha256).as_bytes());
                    let asset = assets.entry(asset_id.clone()).or_insert_with(|| {
                        json!({
                            "asset_id": asset_id,
                            "spec_relative_path": spec_relative_path,
                            "kind": content.kind,
                            "size_bytes": content.size_bytes,
                            "sha256": content.sha256,
                            "content_slots": [],
                            "staging_method": content.properties.get("staging_method")
                                .map(String::as_str).unwrap_or("stream_copy")
                        })
                    });
                    let slots = asset["content_slots"]
                        .as_array_mut()
                        .expect("asset content_slots is an array");
                    if !slots
                        .iter()
                        .any(|slot| slot.as_str() == Some(&content.slot))
                    {
                        slots.push(Value::String(content.slot.clone()));
                        slots.sort_by(|left, right| left.as_str().cmp(&right.as_str()));
                    }
                }
                if let Some(encoded_assets) = content.properties.get("encoded_frame_assets") {
                    let encoded_assets: Vec<Value> =
                        serde_json::from_str(encoded_assets).map_err(|error| {
                            ManifestError::Schema(format!(
                                "encoded frame asset provenance is invalid: {error}"
                            ))
                        })?;
                    for encoded_asset in encoded_assets {
                        let spec_relative_path = encoded_asset["spec_relative_path"]
                            .as_str()
                            .ok_or_else(|| {
                                ManifestError::Schema("encoded frame asset path is missing".into())
                            })?;
                        let sha256 = encoded_asset["sha256"].as_str().ok_or_else(|| {
                            ManifestError::Schema("encoded frame asset SHA-256 is missing".into())
                        })?;
                        let asset_id =
                            sha256_hex(format!("{spec_relative_path}\0{sha256}").as_bytes());
                        assets.entry(asset_id.clone()).or_insert_with(|| {
                            json!({
                                "asset_id": asset_id,
                                "spec_relative_path": spec_relative_path,
                                "kind": "encoded_frame",
                                "size_bytes": encoded_asset["size_bytes"],
                                "sha256": sha256,
                                "content_slots": [content.slot],
                                "staging_method": "stream_copy"
                            })
                        });
                    }
                }
            }
            let provenance = input
                .plan
                .attributes
                .iter()
                .map(|attribute| {
                    json!({
                        "tag": attribute.address.normalized_tag(),
                        "origin": attribute.origin
                    })
                })
                .collect::<Vec<_>>();
            manifest_entries.push(json!({
                "instance_id": input.plan.instance_id,
                "template_id": input.plan.template_id,
                "template_version": input.plan.template_version,
                "requested": input.requested,
                "bundle_root_instance_id": input.bundle_root_instance_id,
                "bundle_role": input.bundle_role,
                "source_provenance": input.source_provenance,
                "construction_origin": "native_plan",
                "path": input.relative_path,
                "size_bytes": bytes.len(),
                "sha256": sha256_hex(&bytes),
                "determinism": input.determinism,
                "resolved_plan_sha256": input.plan.canonical_sha256(),
                "dicom": {
                    "sop_class_uid": input.plan.sop_class_uid,
                    "transfer_syntax_uid": input.plan.transfer_syntax_uid
                },
                "uids": input.plan.identities.identities,
                "resolved_attributes": input.plan.attributes,
                "value_provenance": provenance,
                "content": input.plan.content,
                "references": input.plan.references,
                "validation": { "status": "passed", "checks": checks }
            }));
        }
        manifest_entries.sort_by(|left, right| {
            left["instance_id"]
                .as_str()
                .cmp(&right["instance_id"].as_str())
        });
        let mut bundle_members = BTreeMap::<String, Vec<Value>>::new();
        for entry in &manifest_entries {
            let root = entry["bundle_root_instance_id"]
                .as_str()
                .expect("manifest entry has bundle root")
                .to_string();
            bundle_members.entry(root).or_default().push(json!({
                "instance_id": entry["instance_id"],
                "bundle_role": entry["bundle_role"],
                "requested": entry["requested"],
                "source_provenance": entry["source_provenance"]
            }));
        }
        let bundles = bundle_members
            .into_iter()
            .map(|(root, mut members)| {
                members.sort_by(|left, right| {
                    left["instance_id"]
                        .as_str()
                        .cmp(&right["instance_id"].as_str())
                });
                let member_ids = members
                    .iter()
                    .filter_map(|member| member["instance_id"].as_str())
                    .collect::<BTreeSet<_>>();
                let references = manifest_entries
                    .iter()
                    .filter(|entry| {
                        member_ids.contains(
                            entry["instance_id"]
                                .as_str()
                                .expect("manifest entry has instance ID"),
                        )
                    })
                    .flat_map(|entry| {
                        entry["references"]
                            .as_array()
                            .expect("manifest references are an array")
                            .iter()
                            .cloned()
                    })
                    .collect::<Vec<_>>();
                json!({
                    "bundle_root_instance_id": root,
                    "members": members,
                    "dependency_closure": member_ids,
                    "references": references
                })
            })
            .collect::<Vec<_>>();
        let manifest = json!({
            "manifest_schema_version": "0.5.0",
            "generated_at": inputs.generated_at,
            "generator": inputs.generator,
            "standards": inputs.standards,
            "dependencies": inputs.dependencies,
            "product_resources": inputs.product_resources,
            "run": {
                "kind": "composition",
                "seed": inputs.seed,
                "composition_spec_schema_version": inputs.composition_spec_schema_version,
                "input_spec_sha256": inputs.input_spec_sha256,
                "template_catalog_schema_version": inputs.template_catalog_schema_version,
                "template_catalog_sha256": inputs.template_catalog_sha256,
                "resource_limits": inputs.resource_limits,
                "parallelism": {
                    "requested": inputs.requested_parallelism,
                    "used": inputs.used_parallelism
                }
            },
            "composition": {
                "entries": manifest_entries,
                "bundles": bundles,
                "assets": assets.into_values().collect::<Vec<_>>(),
                "unavailable_capabilities": [],
                "publication": {
                    "staging_complete": true,
                    "validation_complete": true,
                    "expected_entries": entries.len(),
                    "expected_output_bytes": output_bytes,
                    "atomic_promotion": "complete",
                    "cleanup_complete": true
                }
            }
        });
        validate_manifest_schema(&manifest)?;
        Ok(manifest)
    }
}

fn collect_assets(
    plan: &ResolvedInstancePlan,
    assets: &mut BTreeMap<String, Value>,
) -> Result<(), ManifestError> {
    for content in &plan.content {
        if let Some(spec_relative_path) = content.properties.get("spec_relative_path") {
            let asset_id =
                sha256_hex(format!("{spec_relative_path}\0{}", content.sha256).as_bytes());
            let asset = assets.entry(asset_id.clone()).or_insert_with(|| {
                json!({
                    "asset_id": asset_id,
                    "spec_relative_path": spec_relative_path,
                    "kind": content.kind,
                    "size_bytes": content.size_bytes,
                    "sha256": content.sha256,
                    "content_slots": [],
                    "staging_method": content.properties.get("staging_method")
                        .map(String::as_str).unwrap_or("stream_copy")
                })
            });
            let slots = asset["content_slots"].as_array_mut().expect("asset slots");
            if !slots
                .iter()
                .any(|slot| slot.as_str() == Some(&content.slot))
            {
                slots.push(Value::String(content.slot.clone()));
                slots.sort_by(|left, right| left.as_str().cmp(&right.as_str()));
            }
        }
        if let Some(encoded_assets) = content.properties.get("encoded_frame_assets") {
            let encoded_assets: Vec<Value> =
                serde_json::from_str(encoded_assets).map_err(|error| {
                    ManifestError::Schema(format!(
                        "encoded frame asset provenance is invalid: {error}"
                    ))
                })?;
            for encoded_asset in encoded_assets {
                let spec_relative_path =
                    encoded_asset["spec_relative_path"]
                        .as_str()
                        .ok_or_else(|| {
                            ManifestError::Schema("encoded frame asset path is missing".into())
                        })?;
                let sha256 = encoded_asset["sha256"].as_str().ok_or_else(|| {
                    ManifestError::Schema("encoded frame asset SHA-256 is missing".into())
                })?;
                let asset_id = sha256_hex(format!("{spec_relative_path}\0{sha256}").as_bytes());
                assets.entry(asset_id.clone()).or_insert_with(|| {
                    json!({
                        "asset_id": asset_id,
                        "spec_relative_path": spec_relative_path,
                        "kind": "encoded_frame",
                        "size_bytes": encoded_asset["size_bytes"],
                        "sha256": sha256,
                        "content_slots": [content.slot],
                        "staging_method": "stream_copy"
                    })
                });
            }
        }
    }
    Ok(())
}

fn finish_manifest(
    inputs: CompositionManifestInputs,
    mut manifest_entries: Vec<Value>,
    assets: BTreeMap<String, Value>,
    output_bytes: u64,
) -> Result<Value, ManifestError> {
    manifest_entries.sort_by(|left, right| {
        left["instance_id"]
            .as_str()
            .cmp(&right["instance_id"].as_str())
    });
    let mut bundle_members = BTreeMap::<String, Vec<Value>>::new();
    for entry in &manifest_entries {
        let root = entry["bundle_root_instance_id"]
            .as_str()
            .expect("bundle root")
            .to_string();
        bundle_members.entry(root).or_default().push(json!({
            "instance_id": entry["instance_id"],
            "bundle_role": entry["bundle_role"],
            "requested": entry["requested"],
            "source_provenance": entry["source_provenance"]
        }));
    }
    let bundles = bundle_members
        .into_iter()
        .map(|(root, mut members)| {
            members.sort_by(|left, right| {
                left["instance_id"]
                    .as_str()
                    .cmp(&right["instance_id"].as_str())
            });
            let member_ids = members
                .iter()
                .filter_map(|member| member["instance_id"].as_str())
                .collect::<BTreeSet<_>>();
            let references = manifest_entries
                .iter()
                .filter(|entry| {
                    member_ids.contains(entry["instance_id"].as_str().expect("instance ID"))
                })
                .flat_map(|entry| {
                    entry["references"]
                        .as_array()
                        .expect("references array")
                        .iter()
                        .cloned()
                })
                .collect::<Vec<_>>();
            json!({
                "bundle_root_instance_id": root,
                "members": members,
                "dependency_closure": member_ids,
                "references": references
            })
        })
        .collect::<Vec<_>>();
    let manifest = json!({
        "manifest_schema_version": "0.5.0",
        "generated_at": inputs.generated_at,
        "generator": inputs.generator,
        "standards": inputs.standards,
        "dependencies": inputs.dependencies,
        "product_resources": inputs.product_resources,
        "run": {
            "kind": "composition",
            "seed": inputs.seed,
            "composition_spec_schema_version": inputs.composition_spec_schema_version,
            "input_spec_sha256": inputs.input_spec_sha256,
            "template_catalog_schema_version": inputs.template_catalog_schema_version,
            "template_catalog_sha256": inputs.template_catalog_sha256,
            "resource_limits": inputs.resource_limits,
            "parallelism": { "requested": inputs.requested_parallelism, "used": inputs.used_parallelism }
        },
        "composition": {
            "entries": manifest_entries,
            "bundles": bundles,
            "assets": assets.into_values().collect::<Vec<_>>(),
            "unavailable_capabilities": [],
            "publication": {
                "staging_complete": true,
                "validation_complete": true,
                "expected_entries": manifest_entries.len(),
                "expected_output_bytes": output_bytes,
                "atomic_promotion": "complete",
                "cleanup_complete": true
            }
        }
    });
    validate_manifest_schema(&manifest)?;
    Ok(manifest)
}

pub(super) fn validate_manifest_schema(manifest: &Value) -> Result<(), ManifestError> {
    let schema: Value = serde_json::from_str(MANIFEST_SCHEMA)
        .map_err(|error| ManifestError::Schema(error.to_string()))?;
    let validator = jsonschema::validator_for(&schema)
        .map_err(|error| ManifestError::Schema(error.to_string()))?;
    let errors = validator
        .iter_errors(manifest)
        .map(|error| error.to_string())
        .collect::<Vec<_>>();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(ManifestError::Schema(errors.join("; ")))
    }
}

#[derive(Debug)]
pub enum ManifestError {
    NoEntries,
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    OutputSizeOverflow,
    ValidationFailed {
        instance_id: String,
        checks: Vec<ValidationCheck>,
    },
    Schema(String),
}

impl fmt::Display for ManifestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoEntries => {
                formatter.write_str("a composition manifest requires at least one entry")
            }
            Self::Io { path, source } => write!(formatter, "read {}: {source}", path.display()),
            Self::OutputSizeOverflow => formatter.write_str("composition output size overflow"),
            Self::ValidationFailed { instance_id, .. } => {
                write!(formatter, "generic validation failed for {instance_id}")
            }
            Self::Schema(message) => write!(formatter, "composition manifest schema: {message}"),
        }
    }
}

impl std::error::Error for ManifestError {}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;
    use crate::composition::{
        AttributeAddress, AttributeValue, CanonicalContent, CompositionUidRole,
        ContentMaterialization, DicomVr, IdentityAllocator, Part10Materializer, PrimitiveValue,
        ResolvedAttribute, TemplateId, ValueOrigin,
    };

    static NEXT: AtomicU64 = AtomicU64::new(0);
    const HASH: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    fn plan(content: Vec<u8>) -> ResolvedInstancePlan {
        let template_id = TemplateId("classic/secondary-capture/monochrome".into());
        let template_version = "1.0.0".parse().unwrap();
        let identities = IdentityAllocator::new(HASH, template_id.clone(), template_version, 7)
            .unwrap()
            .allocate_plan(
                "primary",
                [
                    (CompositionUidRole::SopInstance, 0),
                    (CompositionUidRole::ImplementationClass, 0),
                ],
            )
            .unwrap();
        ResolvedInstancePlan {
            plan_schema_version: "0.1.0".into(),
            instance_id: "primary".into(),
            template_id,
            template_version,
            sop_class_uid: "1.2.840.10008.5.1.4.1.1.7".into(),
            transfer_syntax_uid: "1.2.840.10008.1.2.1".into(),
            identities,
            attributes: vec![ResolvedAttribute {
                address: AttributeAddress::from_normalized_tag("0010,0010").unwrap(),
                vr: DicomVr::PN,
                value: Some(AttributeValue::Primitive(PrimitiveValue::String(
                    "DTS^Synthetic".into(),
                ))),
                origin: ValueOrigin::TemplateDefault,
            }],
            content: vec![CanonicalContent {
                slot: "pixels".into(),
                kind: "native_pixels".into(),
                address: AttributeAddress::from_normalized_tag("7FE0,0010").unwrap(),
                vr: DicomVr::OB,
                size_bytes: content.len() as u64,
                sha256: sha256_hex(&content),
                properties: BTreeMap::new(),
                placement: super::super::ContentPlacement::TopLevel,
                materialization: Some(ContentMaterialization::Inline(content)),
            }],
            references: vec![],
        }
    }

    fn inputs() -> CompositionManifestInputs {
        CompositionManifestInputs {
            generated_at: "2026-08-28T00:00:00Z".into(),
            generator: json!({"name": "synth-dicom-gen"}),
            standards: json!({"dicom_base_edition": "2026b"}),
            dependencies: json!({}),
            product_resources: crate::product_resources::ProductResources::embedded()
                .identity()
                .unwrap(),
            seed: 7,
            composition_spec_schema_version: "0.1.0".into(),
            input_spec_sha256: HASH.into(),
            template_catalog_schema_version: "0.1.0".into(),
            template_catalog_sha256: HASH.into(),
            resource_limits: BTreeMap::from([("max_output_bytes".into(), 4096)]),
            requested_parallelism: 1,
            used_parallelism: 1,
        }
    }

    #[test]
    fn projects_the_same_plan_used_by_the_writer_without_case_identity() {
        let path = std::env::temp_dir().join(format!(
            "dts-composition-manifest-{}-{}.dcm",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let plan = plan(vec![0, 1, 2, 3]);
        Part10Materializer.materialize(&plan, &path).unwrap();
        let manifest = CompositionManifestAssembler
            .assemble(
                inputs(),
                &[ManifestEntryInput {
                    plan: &plan,
                    output_path: &path,
                    relative_path: "instances/primary.dcm".into(),
                    requested: true,
                    bundle_root_instance_id: "primary".into(),
                    bundle_role: "root".into(),
                    source_provenance: "requested".into(),
                    determinism: "byte_stable".into(),
                }],
            )
            .unwrap();
        let bytes = fs::read(&path).unwrap();
        let evidence_manifest = CompositionManifestAssembler
            .assemble_from_evidence(
                inputs(),
                &[EvidenceManifestEntryInput {
                    plan: &plan,
                    resolved_plan_sha256: plan.canonical_sha256(),
                    relative_path: "instances/primary.dcm".into(),
                    size_bytes: bytes.len() as u64,
                    sha256: sha256_hex(&bytes),
                    checks: GenericPlanValidator.validate_file(&plan, &path),
                    requested: true,
                    bundle_root_instance_id: "primary".into(),
                    bundle_role: "root".into(),
                    source_provenance: "requested".into(),
                    determinism: "byte_stable".into(),
                }],
            )
            .unwrap();
        fs::remove_file(path).unwrap();

        assert_eq!(evidence_manifest, manifest);

        assert_eq!(manifest["run"]["kind"], "composition");
        assert_eq!(
            manifest["composition"]["entries"][0]["resolved_plan_sha256"],
            plan.canonical_sha256()
        );
        assert!(!manifest.to_string().contains("case_id"));
        assert_eq!(
            manifest["composition"]["entries"][0]["validation"]["status"],
            "passed"
        );
    }

    #[test]
    fn validator_reports_content_divergence_from_the_resolved_plan() {
        let path = std::env::temp_dir().join(format!(
            "dts-composition-validation-{}-{}.dcm",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let written = plan(vec![0, 1, 2, 3]);
        Part10Materializer.materialize(&written, &path).unwrap();
        let expected = plan(vec![4, 5, 6, 7]);
        let checks = GenericPlanValidator.validate_file(&expected, &path);
        fs::remove_file(path).unwrap();
        assert!(
            checks
                .iter()
                .any(|check| { check.rule_id == "content_integrity" && check.status == "failed" })
        );
    }

    #[test]
    fn validator_does_not_invent_wire_vr_for_implicit_transfer_syntax() {
        let path = std::env::temp_dir().join(format!(
            "dts-composition-implicit-vr-{}-{}.dcm",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let mut implicit = plan(vec![0, 1, 2, 3]);
        implicit.transfer_syntax_uid = dicom_dictionary_std::uids::IMPLICIT_VR_LITTLE_ENDIAN.into();
        implicit.attributes.push(ResolvedAttribute {
            address: AttributeAddress::from_normalized_tag("0018,1149").unwrap(),
            // This compatibility declaration intentionally differs from the
            // standard dictionary VR. Implicit VR does not encode either VR.
            vr: DicomVr::DS,
            value: Some(AttributeValue::Multi(vec![
                PrimitiveValue::String("2".into()),
                PrimitiveValue::String("2".into()),
            ])),
            origin: ValueOrigin::InstanceOverride,
        });
        Part10Materializer.materialize(&implicit, &path).unwrap();
        let checks = GenericPlanValidator.validate_file(&implicit, &path);
        fs::remove_file(path).unwrap();
        assert!(
            checks.iter().all(|check| check.status == "passed"),
            "{checks:#?}"
        );
    }
}
