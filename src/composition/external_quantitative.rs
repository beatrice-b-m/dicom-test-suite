//! Execution-only adapters for pinned external quantitative DICOM providers.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::executor::cancellation::CancellationToken;
use crate::executor::engine::ServiceInvocationError;
use crate::executor::services::{
    AssetDeclaration, AssetVisibility, ProducedAsset, ProviderRequest, ProviderResult,
    ServiceEvidence, StagedAssetHandle, StagedAssetRegistry, StagingRelativePath, ToolIdentity,
};
use crate::generation_backends::{
    ControlledMetadata, FLOAT32_SPEC, FLOAT64_SPEC, ParametricMapGenerationInput,
    ParametricMapIdentities, ParametricMapSource, ParametricMapVariantOutcome, StandardsProvenance,
    WsiTileSegmentationGenerationInput, WsiTileSegmentationIdentities, WsiTileSegmentationOutcome,
    generate_parametric_map_for_spec_cancellable, generate_wsi_tile_segmentation_cancellable,
};
use crate::sha256_hex;

use super::executor_adapter::CompositionExternalDicomProvider;
use super::{
    AdvancedFamilyProfile, AttributeAddress, AttributeItem, AttributeOperation, AttributeValue,
    CanonicalContent, ComposeError, CompositionUidRole, ContentMaterialization, ContentPlacement,
    DicomVr, LocalContentResolver, PrimitiveValue, ResolvedAttribute, ResolvedInstancePlan,
    SpecInstance, TemplateDescriptor, ValueOrigin,
};

/// Builds the native execution plan used only when the caller supplies
/// Parametric Map pixels. Static family construction belongs here rather than
/// in the composition dispatcher; the dispatcher merely selects this provider.
pub(crate) fn plan_caller_parametric_map(
    mut plan: ResolvedInstancePlan,
    instance: &SpecInstance,
    template: &TemplateDescriptor,
    content_resolver: &mut LocalContentResolver,
) -> Result<ResolvedInstancePlan, ComposeError> {
    if !template
        .template_id
        .0
        .starts_with("derived/parametric-map/")
    {
        return Err(ComposeError::AdvancedDefaults(format!(
            "caller content is not supported for {}",
            template.template_id
        )));
    }
    let (tag, vr, size, kind) = if template.template_id.0.ends_with("float64") {
        ("7FE0,0009", DicomVr::OD, 6_144_u64, "double_float_pixels")
    } else {
        ("7FE0,0008", DicomVr::OF, 3_072_u64, "float_pixels")
    };
    let identity = |role| {
        plan.identities
            .get(&role, 0)
            .map(str::to_owned)
            .ok_or_else(|| ComposeError::AdvancedDefaults(format!("missing {}", role.as_str())))
    };
    let values = [
        ("0008,0016", DicomVr::UI, plan.sop_class_uid.clone()),
        (
            "0008,0018",
            DicomVr::UI,
            identity(CompositionUidRole::SopInstance)?,
        ),
        ("0008,0060", DicomVr::CS, "OT".into()),
        ("0010,0010", DicomVr::PN, "DTS^Synthetic^Patient001".into()),
        ("0010,0020", DicomVr::LO, "DTS-PATIENT-001".into()),
        (
            "0020,000D",
            DicomVr::UI,
            identity(CompositionUidRole::StudyInstance)?,
        ),
        (
            "0020,000E",
            DicomVr::UI,
            identity(CompositionUidRole::SeriesInstance)?,
        ),
        (
            "0020,0052",
            DicomVr::UI,
            identity(CompositionUidRole::FrameOfReference)?,
        ),
        ("0028,0008", DicomVr::IS, "3".into()),
    ];
    plan.attributes = values
        .into_iter()
        .map(|(tag, vr, value)| {
            Ok(ResolvedAttribute {
                address: AttributeAddress::from_normalized_tag(tag)
                    .map_err(|error| ComposeError::AdvancedDefaults(error.to_string()))?,
                vr,
                value: Some(AttributeValue::Primitive(PrimitiveValue::String(value))),
                origin: ValueOrigin::TemplateDefault,
            })
        })
        .collect::<Result<Vec<_>, ComposeError>>()?;
    for (tag, value) in [("0028,0010", 16_u64), ("0028,0011", 16_u64)] {
        plan.attributes.push(ResolvedAttribute {
            address: AttributeAddress::from_normalized_tag(tag)
                .map_err(|error| ComposeError::AdvancedDefaults(error.to_string()))?,
            vr: DicomVr::US,
            value: Some(AttributeValue::Primitive(PrimitiveValue::Unsigned(value))),
            origin: ValueOrigin::TemplateDefault,
        });
    }
    let bytes = vec![0_u8; size as usize];
    plan.content = vec![CanonicalContent {
        slot: "pixels".into(),
        kind: kind.into(),
        address: AttributeAddress::from_normalized_tag(tag)
            .map_err(|error| ComposeError::AdvancedDefaults(error.to_string()))?,
        vr,
        size_bytes: size,
        sha256: sha256_hex(&bytes),
        properties: BTreeMap::new(),
        placement: ContentPlacement::TopLevel,
        materialization: Some(ContentMaterialization::Inline(bytes)),
    }];
    AdvancedFamilyProfile::for_template(&template.template_id.0)
        .expect("parametric map has an advanced profile")
        .customize_direct_plan(instance, &mut plan, content_resolver)?;
    plan.attributes
        .sort_by(|left, right| left.address.cmp(&right.address));
    Ok(plan)
}

/// Seeds the standard Source Image Sequence from the already-resolved logical
/// references so the common reference rewriter can qualify caller content.
pub(crate) fn seed_parametric_reference_sequence(
    plan: &mut ResolvedInstancePlan,
) -> Result<(), ComposeError> {
    if plan.references.is_empty() {
        return Ok(());
    }
    let sequence = AttributeAddress::from_normalized_tag("0008,2112")
        .map_err(|error| ComposeError::AdvancedDefaults(error.to_string()))?;
    let items = plan
        .references
        .iter()
        .map(|reference| AttributeItem {
            attributes: vec![
                AttributeOperation::Set {
                    address: AttributeAddress::from_normalized_tag("0008,1150")
                        .expect("reference SOP Class tag is valid"),
                    vr: DicomVr::UI,
                    value: AttributeValue::Primitive(PrimitiveValue::String(
                        reference.referenced_sop_class_uid.clone(),
                    )),
                },
                AttributeOperation::Set {
                    address: AttributeAddress::from_normalized_tag("0008,1155")
                        .expect("reference SOP Instance tag is valid"),
                    vr: DicomVr::UI,
                    value: AttributeValue::Primitive(PrimitiveValue::String(
                        reference.referenced_sop_instance_uid.clone(),
                    )),
                },
            ],
        })
        .collect();
    plan.attributes.push(ResolvedAttribute {
        address: sequence,
        vr: DicomVr::SQ,
        value: Some(AttributeValue::Sequence(items)),
        origin: ValueOrigin::DerivedStructural,
    });
    plan.attributes
        .sort_by(|left, right| left.address.cmp(&right.address));
    Ok(())
}

#[derive(Debug, Clone)]
pub(crate) struct ExternalQuantitativeSource {
    pub role: String,
    pub case_id: String,
    pub sop_class_uid: String,
    pub sop_instance_uid: String,
    pub series_instance_uid: Option<String>,
    pub frame_numbers: Vec<u32>,
}

#[derive(Debug, Clone)]
pub(crate) struct WsiSegExternalProvider {
    pub repository_root: PathBuf,
    pub standards_lock_path: PathBuf,
    pub seed: u64,
    pub standards_lock_sha256: String,
    pub study_instance_uid: String,
    pub series_instance_uid: String,
    pub frame_of_reference_uid: String,
    pub sop_instance_uid: String,
    pub dimension_organization_uid: String,
    pub source: ExternalQuantitativeSource,
}

#[derive(Debug, Clone)]
pub(crate) struct ParametricMapExternalProvider {
    pub repository_root: PathBuf,
    pub standards_lock_path: PathBuf,
    pub seed: u64,
    pub standards_lock_sha256: String,
    pub study_instance_uid: String,
    pub series_instance_uid: String,
    pub frame_of_reference_uid: String,
    pub sop_instance_uid: String,
    pub dimension_organization_uid: String,
    pub sources: Vec<ExternalQuantitativeSource>,
    pub float64: bool,
    pub stored_value_scale: f32,
    pub spatial_rank_increment: f32,
}

impl CompositionExternalDicomProvider for ParametricMapExternalProvider {
    fn invoke(
        &self,
        request: &ProviderRequest,
        assets: &StagedAssetRegistry,
        private_staging_root: &Path,
        cancellation: &CancellationToken,
    ) -> Result<ProviderResult, ServiceInvocationError> {
        let request_root = private_staging_root
            .join(".providers")
            .join(&request.request_id);
        fs::create_dir_all(request_root.parent().expect("request root has a parent"))
            .map_err(|error| service_error("provider staging", error))?;
        fs::create_dir(&request_root).map_err(|error| service_error("provider staging", error))?;
        let sources = self
            .sources
            .iter()
            .map(|source| {
                let handle = request.input_assets.get(&source.role).ok_or_else(|| {
                    service_error(
                        "provider source",
                        format!("missing {} binding", source.role),
                    )
                })?;
                let declaration = assets
                    .resolve(handle)
                    .map_err(|error| service_error("provider source", error))?;
                Ok(ParametricMapSource {
                    role: "source_image".into(),
                    source_case_id: source.case_id.clone(),
                    relative_path: declaration.relative_path.as_str().into(),
                    sha256: declaration.sha256.clone(),
                    sop_class_uid: source.sop_class_uid.clone(),
                    sop_instance_uid: source.sop_instance_uid.clone(),
                    series_instance_uid: source.series_instance_uid.clone(),
                    frame_numbers: None,
                })
            })
            .collect::<Result<Vec<_>, ServiceInvocationError>>()?;
        let spec = if self.float64 {
            FLOAT64_SPEC
        } else {
            FLOAT32_SPEC
        };
        let input = ParametricMapGenerationInput {
            repository_root: self.repository_root.clone(),
            generated_root: private_staging_root.to_owned(),
            staging_root: request_root.join("work"),
            destination_root: request_root.join("published"),
            seed: self.seed,
            standards: standards(&self.standards_lock_path, &self.standards_lock_sha256)?,
            controlled_metadata: controlled_metadata(spec.recipe_id),
            identities: ParametricMapIdentities {
                study_instance_uid: self.study_instance_uid.clone(),
                series_instance_uid: self.series_instance_uid.clone(),
                frame_of_reference_uid: self.frame_of_reference_uid.clone(),
                sop_instance_uid: self.sop_instance_uid.clone(),
                dimension_organization_uid: self.dimension_organization_uid.clone(),
            },
            sources,
            stored_value_scale: self.stored_value_scale,
            spatial_rank_increment: self.spatial_rank_increment,
        };
        match generate_parametric_map_for_spec_cancellable(&input, spec, &|| {
            cancellation.is_cancelled()
        })
        .map_err(|error| service_error("external quantitative provider", error))?
        {
            ParametricMapVariantOutcome::Unavailable { code, message } => Err(service_error(
                "external quantitative unavailable",
                format!("{code}: {message}"),
            )),
            ParametricMapVariantOutcome::Generated(generated) => provider_result(
                request,
                private_staging_root,
                generated.output_path,
                generated.output_bytes,
                generated.backend.backend_id,
                generated.backend.version,
                generated.backend.executable_fingerprint,
                generated.response,
            ),
        }
    }
}

impl CompositionExternalDicomProvider for WsiSegExternalProvider {
    fn invoke(
        &self,
        request: &ProviderRequest,
        assets: &StagedAssetRegistry,
        private_staging_root: &Path,
        cancellation: &CancellationToken,
    ) -> Result<ProviderResult, ServiceInvocationError> {
        let handle = request
            .input_assets
            .get(&self.source.role)
            .ok_or_else(|| service_error("provider source", "missing WSI source binding"))?;
        let declaration = assets
            .resolve(handle)
            .map_err(|error| service_error("provider source", error))?;
        let request_root = private_staging_root
            .join(".providers")
            .join(&request.request_id);
        let staging_root = request_root.join("work");
        let destination_root = request_root.join("published");
        fs::create_dir_all(request_root.parent().expect("request root has a parent"))
            .map_err(|error| service_error("provider staging", error))?;
        fs::create_dir(&request_root).map_err(|error| service_error("provider staging", error))?;
        let input = WsiTileSegmentationGenerationInput {
            repository_root: self.repository_root.clone(),
            generated_root: private_staging_root.to_owned(),
            staging_root,
            destination_root,
            seed: self.seed,
            standards: standards(&self.standards_lock_path, &self.standards_lock_sha256)?,
            controlled_metadata: controlled_metadata("derived_seg_wsi_tile_reference"),
            identities: WsiTileSegmentationIdentities {
                study_instance_uid: self.study_instance_uid.clone(),
                series_instance_uid: self.series_instance_uid.clone(),
                frame_of_reference_uid: self.frame_of_reference_uid.clone(),
                sop_instance_uid: self.sop_instance_uid.clone(),
                dimension_organization_uid: self.dimension_organization_uid.clone(),
            },
            source: ParametricMapSource {
                role: "source_image".into(),
                source_case_id: self.source.case_id.clone(),
                relative_path: declaration.relative_path.as_str().into(),
                sha256: declaration.sha256.clone(),
                sop_class_uid: self.source.sop_class_uid.clone(),
                sop_instance_uid: self.source.sop_instance_uid.clone(),
                series_instance_uid: self.source.series_instance_uid.clone(),
                frame_numbers: Some(
                    self.source
                        .frame_numbers
                        .iter()
                        .copied()
                        .map(u64::from)
                        .collect(),
                ),
            },
        };
        let outcome =
            generate_wsi_tile_segmentation_cancellable(&input, &|| cancellation.is_cancelled())
                .map_err(|error| service_error("external quantitative provider", error))?;
        match outcome {
            WsiTileSegmentationOutcome::Unavailable { code, message } => Err(service_error(
                "external quantitative unavailable",
                format!("{code}: {message}"),
            )),
            WsiTileSegmentationOutcome::Generated(generated) => provider_result(
                request,
                private_staging_root,
                generated.output_path,
                generated.output_bytes,
                generated.backend.backend_id,
                generated.backend.version,
                generated.backend.executable_fingerprint,
                generated.response,
            ),
        }
    }
}

pub(crate) fn standards(
    path: &Path,
    expected_sha256: &str,
) -> Result<StandardsProvenance, ServiceInvocationError> {
    let bytes = fs::read(path).map_err(|error| service_error("standards lock", error))?;
    if sha256_hex(&bytes) != expected_sha256 {
        return Err(service_error(
            "standards lock",
            "digest changed after planning",
        ));
    }
    let value: Value =
        serde_json::from_slice(&bytes).map_err(|error| service_error("standards lock", error))?;
    Ok(StandardsProvenance {
        standards_lock_sha256: expected_sha256.into(),
        dicom_base_edition: value["dicom_base_edition"]
            .as_str()
            .ok_or_else(|| service_error("standards lock", "missing DICOM base edition"))?
            .into(),
        kb_source_manifest_sha256: value
            .pointer("/dicom_standard_kb/source_manifest_sha256")
            .and_then(Value::as_str)
            .map(str::to_owned),
    })
}

pub(crate) fn controlled_metadata(model_name: &str) -> ControlledMetadata {
    ControlledMetadata {
        patient_name: "DTS^Synthetic^Patient001".into(),
        patient_id: "DTS-PATIENT-001".into(),
        manufacturer: "dicom-test-suite".into(),
        model_name: model_name.into(),
        software_versions: env!("CARGO_PKG_VERSION").into(),
        study_date: "20260101".into(),
        study_time: "000000".into(),
        content_date: "20260101".into(),
        content_time: "000000".into(),
        timezone_offset_from_utc: "+0000".into(),
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn provider_result(
    request: &ProviderRequest,
    private_staging_root: &Path,
    output_path: PathBuf,
    output_bytes: Vec<u8>,
    backend_id: String,
    runtime_version: String,
    executable_sha256: String,
    response: Value,
) -> Result<ProviderResult, ServiceInvocationError> {
    let expectation = request
        .expected_outputs
        .first()
        .ok_or_else(|| service_error("provider output", "missing expectation"))?;
    let relative = output_path
        .strip_prefix(private_staging_root)
        .map_err(|_| service_error("provider output", "output escaped private staging"))?;
    let digest = sha256_hex(&output_bytes);
    let asset = ProducedAsset {
        declaration: AssetDeclaration {
            handle: StagedAssetHandle::new(format!(
                "provider:{}:{}",
                request.request_id, expectation.slot
            ))
            .map_err(|error| service_error("provider output", error))?,
            relative_path: StagingRelativePath::new(relative.to_string_lossy())
                .map_err(|error| service_error("provider output", error))?,
            size_bytes: output_bytes.len() as u64,
            sha256: digest.clone(),
            media_type: expectation.media_type.clone(),
            visibility: AssetVisibility::Private,
        },
        observed_size_bytes: output_bytes.len() as u64,
        observed_sha256: digest,
    };
    let tool = ToolIdentity {
        backend_id,
        version: request.required_version.clone(),
        protocol_version: Some(crate::generation_backends::PROTOCOL_VERSION.into()),
        executable_sha256: Some(executable_sha256),
    };
    Ok(ProviderResult {
        request_id: request.request_id.clone(),
        provider: tool.clone(),
        outputs: BTreeMap::from([(expectation.slot.clone(), asset)]),
        evidence: vec![ServiceEvidence {
            evidence_id: format!("external_quantitative:{}", request.request_id),
            evidence_kind: "external_quantitative_import".into(),
            producer: tool,
            claims: BTreeMap::from([
                ("network_policy".into(), json!("disabled")),
                ("resource_outcome".into(), json!("within_limits")),
                ("runtime_version".into(), json!(runtime_version)),
                ("termination".into(), json!("exit_zero")),
                ("response".into(), response),
            ]),
        }],
    })
}

pub(crate) fn service_error(
    stage: &'static str,
    error: impl std::fmt::Display,
) -> ServiceInvocationError {
    ServiceInvocationError::new(stage, error.to_string())
}
