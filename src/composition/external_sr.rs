//! Execution-only adapters for pinned external Structured Report providers.

use std::fs;
use std::path::{Path, PathBuf};

use crate::executor::cancellation::CancellationToken;
use crate::executor::engine::ServiceInvocationError;
use crate::executor::services::{ProviderRequest, ProviderResult, StagedAssetRegistry};
use crate::generation_backends::{
    ParametricMapSource, Scoord3dGenerationInput, Scoord3dIdentities, Scoord3dOutcome,
    Scoord3dParameters, Tid1500GenerationInput, Tid1500Identities, Tid1500Outcome,
    Tid1500Parameters, generate_scoord3d_with_parameters_cancellable,
    generate_tid1500_with_parameters_cancellable,
};

use super::executor_adapter::CompositionExternalDicomProvider;
use super::external_quantitative::{
    controlled_metadata, provider_result, service_error, standards,
};

#[derive(Debug, Clone)]
pub(crate) struct ExternalSrSource {
    pub binding_role: String,
    pub backend_role: String,
    pub case_id: String,
    pub sop_class_uid: String,
    pub sop_instance_uid: String,
    pub series_instance_uid: String,
    pub frame_numbers: Option<Vec<u64>>,
}

#[derive(Debug, Clone)]
pub(crate) enum ExternalSrKind {
    Comprehensive3d {
        tracking_uid: String,
        observer_uid: String,
        fiducial_uid: String,
        parameters: Scoord3dParameters,
    },
    Tid1500 {
        tracking_uid: String,
        observer_uid: String,
        parameters: Tid1500Parameters,
    },
}

#[derive(Debug, Clone)]
pub(crate) struct ExternalSrProvider {
    pub repository_root: PathBuf,
    pub standards_lock_path: PathBuf,
    pub seed: u64,
    pub standards_lock_sha256: String,
    pub study_instance_uid: String,
    pub series_instance_uid: String,
    pub frame_of_reference_uid: String,
    pub sop_instance_uid: String,
    pub sources: Vec<ExternalSrSource>,
    pub kind: ExternalSrKind,
}

impl CompositionExternalDicomProvider for ExternalSrProvider {
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
        fs::create_dir_all(request_root.parent().expect("provider root has a parent"))
            .map_err(|error| service_error("SR provider staging", error))?;
        fs::create_dir(&request_root)
            .map_err(|error| service_error("SR provider staging", error))?;
        let sources = self
            .sources
            .iter()
            .map(|source| {
                let handle = request
                    .input_assets
                    .get(&source.binding_role)
                    .ok_or_else(|| {
                        service_error(
                            "SR provider source",
                            format!("missing {} binding", source.binding_role),
                        )
                    })?;
                let declaration = assets
                    .resolve(handle)
                    .map_err(|error| service_error("SR provider source", error))?;
                Ok(ParametricMapSource {
                    role: source.backend_role.clone(),
                    source_case_id: source.case_id.clone(),
                    relative_path: declaration.relative_path.as_str().into(),
                    sha256: declaration.sha256.clone(),
                    sop_class_uid: source.sop_class_uid.clone(),
                    sop_instance_uid: source.sop_instance_uid.clone(),
                    series_instance_uid: Some(source.series_instance_uid.clone()),
                    frame_numbers: source.frame_numbers.clone(),
                })
            })
            .collect::<Result<Vec<_>, ServiceInvocationError>>()?;
        let provenance = standards(&self.standards_lock_path, &self.standards_lock_sha256)?;
        match &self.kind {
            ExternalSrKind::Comprehensive3d {
                tracking_uid,
                observer_uid,
                fiducial_uid,
                parameters,
            } => {
                let input = Scoord3dGenerationInput {
                    repository_root: self.repository_root.clone(),
                    generated_root: private_staging_root.to_owned(),
                    staging_root: request_root.join("work"),
                    destination_root: request_root.join("published"),
                    seed: self.seed,
                    standards: provenance,
                    controlled_metadata: controlled_metadata("derived_sr_comprehensive3d_scoord3d"),
                    identities: Scoord3dIdentities {
                        study_instance_uid: self.study_instance_uid.clone(),
                        series_instance_uid: self.series_instance_uid.clone(),
                        frame_of_reference_uid: self.frame_of_reference_uid.clone(),
                        sop_instance_uid: self.sop_instance_uid.clone(),
                        tracking_uid: tracking_uid.clone(),
                        observer_uid: observer_uid.clone(),
                        fiducial_uid: fiducial_uid.clone(),
                    },
                    sources,
                };
                match generate_scoord3d_with_parameters_cancellable(&input, parameters, &|| {
                    cancellation.is_cancelled()
                })
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
                        generated.response,
                    ),
                }
            }
            ExternalSrKind::Tid1500 {
                tracking_uid,
                observer_uid,
                parameters,
            } => {
                let input = Tid1500GenerationInput {
                    repository_root: self.repository_root.clone(),
                    generated_root: private_staging_root.to_owned(),
                    staging_root: request_root.join("work"),
                    destination_root: request_root.join("published"),
                    seed: self.seed,
                    standards: provenance,
                    controlled_metadata: controlled_metadata(
                        "derived_sr_tid1500_ct_measurement_report",
                    ),
                    identities: Tid1500Identities {
                        study_instance_uid: self.study_instance_uid.clone(),
                        series_instance_uid: self.series_instance_uid.clone(),
                        frame_of_reference_uid: self.frame_of_reference_uid.clone(),
                        sop_instance_uid: self.sop_instance_uid.clone(),
                        tracking_uid: tracking_uid.clone(),
                        observer_uid: observer_uid.clone(),
                    },
                    sources,
                };
                match generate_tid1500_with_parameters_cancellable(&input, *parameters, &|| {
                    cancellation.is_cancelled()
                })
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
                        generated.response,
                    ),
                }
            }
        }
    }
}
