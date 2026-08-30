//! Pinned external DICOM import execution shared by curated frontends.
//!
//! Planning supplies every identity, source role, and bounded import contract.
//! This adapter performs only private-staging provider invocation and never
//! constructs an IOD or reads a published output.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_json::{Value, json};

use crate::composition::{CompositionUidRole, IdentityPlan};
use crate::executor::cancellation::CancellationToken;
use crate::executor::engine::ServiceInvocationError;
use crate::executor::services::{
    AssetDeclaration, AssetVisibility, ProducedAsset, ProviderRequest, ProviderResult,
    ServiceEvidence, StagedAssetHandle, StagedAssetRegistry, StagingRelativePath, ToolIdentity,
};
use crate::generation_backends::{
    ControlledMetadata, FLOAT32_SPEC, FLOAT64_SPEC, ParametricMapGenerationInput,
    ParametricMapIdentities, ParametricMapSource, ParametricMapVariantOutcome,
    Scoord3dGenerationInput, Scoord3dIdentities, Scoord3dOutcome, Scoord3dParameters,
    StandardsProvenance, Tid1500GenerationInput, Tid1500Identities, Tid1500Outcome,
    Tid1500Parameters, WsiTileSegmentationGenerationInput, WsiTileSegmentationIdentities,
    WsiTileSegmentationOutcome, generate_parametric_map_for_spec_cancellable,
    generate_scoord3d_with_parameters_cancellable, generate_tid1500_with_parameters_cancellable,
    generate_wsi_tile_segmentation_cancellable,
};
use crate::recipes::{ExternalImportBoundary, ExternalImportKind, SrDocumentKind, SrPlanInput};
use crate::sha256_hex;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct QuantitativeParameters {
    route: String,
    seed: u64,
    standards_lock_sha256: String,
    import: ExternalImportBoundary,
    artifact_parameters: serde_json::Map<String, Value>,
    sources: Vec<ProviderSource>,
    identities: IdentityPlan,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct StructuredReportParameters {
    route: String,
    seed: u64,
    standards_lock_sha256: String,
    input: SrPlanInput,
    sources: Vec<ProviderSource>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProviderSource {
    binding_role: String,
    #[serde(default)]
    backend_role: Option<String>,
    case_id: String,
    sop_class_uid: String,
    sop_instance_uid: String,
    #[serde(default)]
    series_instance_uid: Option<String>,
    #[serde(default)]
    frame_numbers: Vec<u32>,
}

pub(super) fn invoke(
    request: &ProviderRequest,
    assets: &StagedAssetRegistry,
    private_staging_root: &Path,
    repository_root: &Path,
    standards_lock_path: &Path,
    planned_standards_lock_sha256: &str,
    cancellation: &CancellationToken,
) -> Result<ProviderResult, ServiceInvocationError> {
    let route = request
        .parameters
        .get("route")
        .and_then(Value::as_str)
        .ok_or_else(|| service_error("external import", "missing typed route"))?;
    match route {
        "quantitative" => invoke_quantitative(
            request,
            assets,
            private_staging_root,
            repository_root,
            standards_lock_path,
            planned_standards_lock_sha256,
            cancellation,
        ),
        "structured_report" => invoke_sr(
            request,
            assets,
            private_staging_root,
            repository_root,
            standards_lock_path,
            planned_standards_lock_sha256,
            cancellation,
        ),
        other => Err(service_error(
            "external import",
            format!("unsupported typed route {other}"),
        )),
    }
}

#[allow(clippy::too_many_arguments)]
fn invoke_quantitative(
    request: &ProviderRequest,
    assets: &StagedAssetRegistry,
    private_staging_root: &Path,
    repository_root: &Path,
    standards_lock_path: &Path,
    planned_standards_lock_sha256: &str,
    cancellation: &CancellationToken,
) -> Result<ProviderResult, ServiceInvocationError> {
    let parameters: QuantitativeParameters = parameters(request)?;
    validate_common(
        &parameters.route,
        parameters.seed,
        &parameters.standards_lock_sha256,
        planned_standards_lock_sha256,
    )?;
    let sources = resolve_sources(request, assets, &parameters.sources)?;
    let request_root = prepare_request_root(private_staging_root, &request.request_id)?;
    let standards = standards(standards_lock_path, planned_standards_lock_sha256)?;
    let identities = &parameters.identities;
    match parameters.import.kind {
        ExternalImportKind::ParametricMapFloat32 | ExternalImportKind::ParametricMapFloat64 => {
            let float64 = parameters.import.kind == ExternalImportKind::ParametricMapFloat64;
            let stored_value_scale =
                number(&parameters.artifact_parameters, "stored_value_scale")? as f32;
            let spatial_rank_increment =
                number(&parameters.artifact_parameters, "spatial_rank_increment")? as f32;
            let input = ParametricMapGenerationInput {
                repository_root: repository_root.to_owned(),
                generated_root: private_staging_root.to_owned(),
                staging_root: request_root.join("work"),
                destination_root: request_root.join("published"),
                seed: parameters.seed,
                standards,
                controlled_metadata: controlled_metadata(if float64 {
                    FLOAT64_SPEC.recipe_id
                } else {
                    FLOAT32_SPEC.recipe_id
                }),
                identities: ParametricMapIdentities {
                    study_instance_uid: identity(identities, CompositionUidRole::StudyInstance)?,
                    series_instance_uid: identity(identities, CompositionUidRole::SeriesInstance)?,
                    frame_of_reference_uid: identity(
                        identities,
                        CompositionUidRole::FrameOfReference,
                    )?,
                    sop_instance_uid: identity(identities, CompositionUidRole::SopInstance)?,
                    dimension_organization_uid: identity(
                        identities,
                        CompositionUidRole::DimensionOrganization,
                    )?,
                },
                sources,
                stored_value_scale,
                spatial_rank_increment,
            };
            let spec = if float64 { FLOAT64_SPEC } else { FLOAT32_SPEC };
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
                    generated.backend.entrypoint_fingerprint,
                    generated.backend.environment_fingerprint,
                    generated.backend.runtime_identity,
                    generated.response,
                ),
            }
        }
        ExternalImportKind::WholeSlideTileSegmentation => {
            let [source] = sources.as_slice() else {
                return Err(service_error(
                    "external quantitative provider",
                    "WSI SEG requires exactly one source",
                ));
            };
            let input = WsiTileSegmentationGenerationInput {
                repository_root: repository_root.to_owned(),
                generated_root: private_staging_root.to_owned(),
                staging_root: request_root.join("work"),
                destination_root: request_root.join("published"),
                seed: parameters.seed,
                standards,
                controlled_metadata: controlled_metadata("derived_seg_wsi_tile_reference"),
                identities: WsiTileSegmentationIdentities {
                    study_instance_uid: identity(identities, CompositionUidRole::StudyInstance)?,
                    series_instance_uid: identity(identities, CompositionUidRole::SeriesInstance)?,
                    frame_of_reference_uid: identity(
                        identities,
                        CompositionUidRole::FrameOfReference,
                    )?,
                    sop_instance_uid: identity(identities, CompositionUidRole::SopInstance)?,
                    dimension_organization_uid: identity(
                        identities,
                        CompositionUidRole::DimensionOrganization,
                    )?,
                },
                source: source.clone(),
            };
            match generate_wsi_tile_segmentation_cancellable(&input, &|| {
                cancellation.is_cancelled()
            })
            .map_err(|error| service_error("external quantitative provider", error))?
            {
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
                    generated.backend.entrypoint_fingerprint,
                    generated.backend.environment_fingerprint,
                    generated.backend.runtime_identity,
                    generated.response,
                ),
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn invoke_sr(
    request: &ProviderRequest,
    assets: &StagedAssetRegistry,
    private_staging_root: &Path,
    repository_root: &Path,
    standards_lock_path: &Path,
    planned_standards_lock_sha256: &str,
    cancellation: &CancellationToken,
) -> Result<ProviderResult, ServiceInvocationError> {
    let parameters: StructuredReportParameters = parameters(request)?;
    validate_common(
        &parameters.route,
        parameters.seed,
        &parameters.standards_lock_sha256,
        planned_standards_lock_sha256,
    )?;
    let sources = resolve_sources(request, assets, &parameters.sources)?;
    let request_root = prepare_request_root(private_staging_root, &request.request_id)?;
    let standards = standards(standards_lock_path, planned_standards_lock_sha256)?;
    let context = &parameters.input.context;
    let planned_derived_uid = |name: &str| {
        identity(
            &context.identities,
            CompositionUidRole::TemplateDefined(name.into()),
        )
    };
    match &parameters.input.parameters.document {
        SrDocumentKind::Tid1500 { numeric_value, .. } => {
            let input = Tid1500GenerationInput {
                repository_root: repository_root.to_owned(),
                generated_root: private_staging_root.to_owned(),
                staging_root: request_root.join("work"),
                destination_root: request_root.join("published"),
                seed: parameters.seed,
                standards,
                controlled_metadata: controlled_metadata(
                    "derived_sr_tid1500_ct_measurement_report",
                ),
                identities: Tid1500Identities {
                    study_instance_uid: identity(
                        &context.identities,
                        CompositionUidRole::StudyInstance,
                    )?,
                    series_instance_uid: identity(
                        &context.identities,
                        CompositionUidRole::SeriesInstance,
                    )?,
                    frame_of_reference_uid: source_frame_of_reference(&parameters.input)?,
                    sop_instance_uid: identity(
                        &context.identities,
                        CompositionUidRole::SopInstance,
                    )?,
                    tracking_uid: planned_derived_uid("tracking_uid")?,
                    observer_uid: planned_derived_uid("observer_uid")?,
                },
                sources,
            };
            let numeric = numeric_value
                .parse::<f64>()
                .map_err(|error| service_error("external SR parameters", error))?;
            match generate_tid1500_with_parameters_cancellable(
                &input,
                Tid1500Parameters {
                    measurement_value_mm3: numeric,
                },
                &|| cancellation.is_cancelled(),
            )
            .map_err(|error| service_error("external SR provider", error))?
            {
                Tid1500Outcome::Unavailable { code, message } => Err(service_error(
                    "external SR unavailable",
                    format!("{code}: {message}"),
                )),
                Tid1500Outcome::Generated(generated) => provider_result(
                    request,
                    private_staging_root,
                    generated.output_path,
                    generated.output_bytes,
                    generated.backend.backend_id,
                    generated.backend.version,
                    generated.backend.executable_fingerprint,
                    generated.backend.entrypoint_fingerprint,
                    generated.backend.environment_fingerprint,
                    generated.backend.runtime_identity,
                    generated.response,
                ),
            }
        }
        SrDocumentKind::Comprehensive3d { .. } => {
            let input = Scoord3dGenerationInput {
                repository_root: repository_root.to_owned(),
                generated_root: private_staging_root.to_owned(),
                staging_root: request_root.join("work"),
                destination_root: request_root.join("published"),
                seed: parameters.seed,
                standards,
                controlled_metadata: controlled_metadata("derived_sr_comprehensive3d_scoord3d"),
                identities: Scoord3dIdentities {
                    study_instance_uid: identity(
                        &context.identities,
                        CompositionUidRole::StudyInstance,
                    )?,
                    series_instance_uid: identity(
                        &context.identities,
                        CompositionUidRole::SeriesInstance,
                    )?,
                    frame_of_reference_uid: source_frame_of_reference(&parameters.input)?,
                    sop_instance_uid: identity(
                        &context.identities,
                        CompositionUidRole::SopInstance,
                    )?,
                    tracking_uid: planned_derived_uid("tracking_uid")?,
                    observer_uid: planned_derived_uid("observer_uid")?,
                    fiducial_uid: planned_derived_uid("fiducial_uid")?,
                },
                sources,
            };
            match generate_scoord3d_with_parameters_cancellable(
                &input,
                &Scoord3dParameters::default(),
                &|| cancellation.is_cancelled(),
            )
            .map_err(|error| service_error("external SR provider", error))?
            {
                Scoord3dOutcome::Unavailable { code, message } => Err(service_error(
                    "external SR unavailable",
                    format!("{code}: {message}"),
                )),
                Scoord3dOutcome::Generated(generated) => provider_result(
                    request,
                    private_staging_root,
                    generated.output_path,
                    generated.output_bytes,
                    generated.backend.backend_id,
                    generated.backend.version,
                    generated.backend.executable_fingerprint,
                    generated.backend.entrypoint_fingerprint,
                    generated.backend.environment_fingerprint,
                    generated.backend.runtime_identity,
                    generated.response,
                ),
            }
        }
        _ => Err(service_error(
            "external SR provider",
            "native SR document reached the external route",
        )),
    }
}

fn parameters<T: for<'de> Deserialize<'de>>(
    request: &ProviderRequest,
) -> Result<T, ServiceInvocationError> {
    serde_json::from_value(Value::Object(
        request.parameters.clone().into_iter().collect(),
    ))
    .map_err(|error| service_error("external import parameters", error))
}

fn validate_common(
    route: &str,
    _seed: u64,
    declared_sha256: &str,
    planned_sha256: &str,
) -> Result<(), ServiceInvocationError> {
    if route.is_empty() || declared_sha256 != planned_sha256 {
        return Err(service_error(
            "external import contract",
            "runtime request differs from the immutable standards plan",
        ));
    }
    Ok(())
}

fn resolve_sources(
    request: &ProviderRequest,
    assets: &StagedAssetRegistry,
    sources: &[ProviderSource],
) -> Result<Vec<ParametricMapSource>, ServiceInvocationError> {
    sources
        .iter()
        .map(|source| {
            let handle = request
                .input_assets
                .get(&source.binding_role)
                .ok_or_else(|| {
                    service_error(
                        "external provider source",
                        format!("missing {} binding", source.binding_role),
                    )
                })?;
            let declaration = assets
                .resolve(handle)
                .map_err(|error| service_error("external provider source", error))?;
            Ok(ParametricMapSource {
                role: source
                    .backend_role
                    .clone()
                    .unwrap_or_else(|| "source_image".into()),
                source_case_id: source.case_id.clone(),
                relative_path: declaration.relative_path.as_str().into(),
                sha256: declaration.sha256.clone(),
                sop_class_uid: source.sop_class_uid.clone(),
                sop_instance_uid: source.sop_instance_uid.clone(),
                series_instance_uid: source.series_instance_uid.clone(),
                frame_numbers: (!source.frame_numbers.is_empty()).then(|| {
                    source
                        .frame_numbers
                        .iter()
                        .copied()
                        .map(u64::from)
                        .collect()
                }),
            })
        })
        .collect()
}

fn source_frame_of_reference(input: &SrPlanInput) -> Result<String, ServiceInvocationError> {
    input
        .context
        .identities
        .get(&CompositionUidRole::FrameOfReference, 0)
        .map(str::to_owned)
        .or_else(|| {
            input.context.sources.first().and_then(|source| {
                // Semantic contexts deliberately carry the source study and
                // series identities; the shared frame identity is allocated
                // into the target context by curated planning.
                source
                    .reference
                    .frame_role
                    .as_ref()
                    .filter(|value| value.starts_with("2.25."))
                    .cloned()
            })
        })
        .ok_or_else(|| service_error("external SR identity", "missing frame of reference UID"))
}

fn identity(
    identities: &IdentityPlan,
    role: CompositionUidRole,
) -> Result<String, ServiceInvocationError> {
    identities.get(&role, 0).map(str::to_owned).ok_or_else(|| {
        service_error(
            "external provider identity",
            format!("missing {}", role.as_str()),
        )
    })
}

fn number(
    parameters: &serde_json::Map<String, Value>,
    name: &str,
) -> Result<f64, ServiceInvocationError> {
    parameters.get(name).and_then(Value::as_f64).ok_or_else(|| {
        service_error(
            "external quantitative parameters",
            format!("missing {name}"),
        )
    })
}

fn prepare_request_root(
    private_staging_root: &Path,
    request_id: &str,
) -> Result<PathBuf, ServiceInvocationError> {
    let root = private_staging_root.join(".providers").join(request_id);
    fs::create_dir_all(root.parent().expect("provider root has a parent"))
        .map_err(|error| service_error("external provider staging", error))?;
    fs::create_dir(&root).map_err(|error| service_error("external provider staging", error))?;
    Ok(root)
}

fn standards(
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

fn controlled_metadata(model_name: &str) -> ControlledMetadata {
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
fn provider_result(
    request: &ProviderRequest,
    private_staging_root: &Path,
    output_path: PathBuf,
    output_bytes: Vec<u8>,
    backend_id: String,
    runtime_version: String,
    executable_sha256: String,
    entrypoint_sha256: String,
    environment_sha256: String,
    runtime_identity: Value,
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
            evidence_id: format!("external_import:{}", request.request_id),
            evidence_kind: "external_dicom_import".into(),
            producer: tool,
            claims: BTreeMap::from([
                ("network_policy".into(), json!("disabled")),
                ("resource_outcome".into(), json!("within_limits")),
                ("runtime_version".into(), json!(runtime_version)),
                ("entrypoint_fingerprint".into(), json!(entrypoint_sha256)),
                ("environment_fingerprint".into(), json!(environment_sha256)),
                ("runtime_identity".into(), runtime_identity),
                ("termination".into(), json!("exit_zero")),
                ("response".into(), response),
            ]),
        }],
    })
}

fn service_error(stage: &'static str, error: impl std::fmt::Display) -> ServiceInvocationError {
    ServiceInvocationError::new(stage, error.to_string())
}
