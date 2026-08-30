//! Explicit locked full-file adapter for DCMTK legacy JPEG lossless.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use dicom_encoding::{Codec, adapters::PixelDataReader};
use dicom_object::open_file;
use dicom_transfer_syntax_registry::entries::{
    JPEG_LOSSLESS_NON_HIERARCHICAL, JPEG_LOSSLESS_NON_HIERARCHICAL_FIRST_ORDER_PREDICTION,
};
use serde_json::json;

use crate::codecs::{DcmtkDcmcjpegLosslessProcess, DcmtkDcmcjpegLosslessSv1Encoder};
use crate::composition::ContentMaterialization;
use crate::executor::cancellation::CancellationToken;
use crate::executor::engine::ServiceInvocationError;
use crate::executor::services::{
    AssetDeclaration, AssetVisibility, ProducedAsset, ProviderRequest, ProviderResult,
    ServiceEvidence, StagedAssetHandle, StagedAssetRegistry, StagingRelativePath, ToolIdentity,
};
use crate::recipes::LockedFullFileCodecRequest;
use crate::runtime_capabilities::QualifiedExecutableIdentity;
use crate::runtime_capabilities::qualified_executable_version_id;
use crate::sha256_hex;

#[derive(Debug, Clone)]
pub struct DcmtkLockedFullFileService {
    encoder: DcmtkDcmcjpegLosslessSv1Encoder,
    expected: QualifiedExecutableIdentity,
}

impl DcmtkLockedFullFileService {
    pub fn new(
        command: PathBuf,
        expected: QualifiedExecutableIdentity,
    ) -> Result<Self, ServiceInvocationError> {
        let service = Self {
            encoder: DcmtkDcmcjpegLosslessSv1Encoder::with_command(command),
            expected,
        };
        service.verify_identity()?;
        Ok(service)
    }

    pub fn invoke(
        &self,
        request: &ProviderRequest,
        locked: &LockedFullFileCodecRequest,
        staging_root: &Path,
        assets: &StagedAssetRegistry,
        cancellation: &CancellationToken,
    ) -> Result<ProviderResult, ServiceInvocationError> {
        self.verify_identity()?;
        if cancellation.is_cancelled() {
            return Err(error("execution cancelled"));
        }
        if request.request_id != locked.request_id
            || request.provider_id != locked.backend_id
            || request.required_version != qualified_executable_version_id(&self.expected)
            || request.parameters != locked.parameters
        {
            return Err(error(
                "provider request differs from locked planning contract",
            ));
        }
        let process = match request.provider_id.as_str() {
            "dcmtk_dcmcjpeg_jpeg_lossless_process_14_command_writer" => {
                DcmtkDcmcjpegLosslessProcess::Process14
            }
            "dcmtk_dcmcjpeg_jpeg_lossless_sv1_command_writer" => DcmtkDcmcjpegLosslessProcess::Sv1,
            _ => return Err(error("unsupported locked DCMTK backend")),
        };
        if process.transfer_syntax_uid() != locked.target_transfer_syntax_uid {
            return Err(error("locked target transfer syntax differs from backend"));
        }
        let source_handle = request
            .input_assets
            .get("source_dicom")
            .ok_or_else(|| error("missing explicit private source DICOM binding"))?;
        let source = assets
            .resolve(source_handle)
            .map_err(|source| error(source.to_string()))?;
        if source.visibility != AssetVisibility::Private || source.media_type != "application/dicom"
        {
            return Err(error("locked source must be a private DICOM asset"));
        }
        let source_path = staging_root.join(source.relative_path.as_str());
        let relative = StagingRelativePath::new(format!(".providers/{}.dcm", request.request_id))
            .map_err(|source| error(source.to_string()))?;
        let output_path = staging_root.join(relative.as_str());
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent).map_err(|source| error(source.to_string()))?;
        }
        let encoded = self
            .encoder
            .encode_file_with_process_cancellable(process, &source_path, &output_path, &|| {
                cancellation.is_cancelled()
            })
            .map_err(|source| error(source.to_string()))?;
        self.compare_identity(
            encoded.backend_identity.version.as_deref(),
            &encoded.backend_identity.executable_sha256,
        )?;
        validate_output(&output_path, process, locked)?;
        let sha256 = sha256_hex(&encoded.output_bytes);
        let size_bytes = encoded.output_bytes.len() as u64;
        let identity = ToolIdentity {
            backend_id: request.provider_id.clone(),
            version: request.required_version.clone(),
            protocol_version: Some("command_stdout_and_executable_sha256".into()),
            executable_sha256: Some(self.expected.executable_sha256.clone()),
        };
        let asset = ProducedAsset {
            declaration: AssetDeclaration {
                handle: StagedAssetHandle::new(format!("provider:{}", request.request_id))
                    .map_err(|source| error(source.to_string()))?,
                relative_path: relative,
                size_bytes,
                sha256: sha256.clone(),
                media_type: "application/dicom".into(),
                visibility: AssetVisibility::Private,
            },
            observed_size_bytes: size_bytes,
            observed_sha256: sha256,
        };
        Ok(ProviderResult {
            request_id: request.request_id.clone(),
            provider: identity.clone(),
            outputs: BTreeMap::from([("dicom".into(), asset)]),
            evidence: vec![ServiceEvidence {
                evidence_id: format!("locked_dcmtk:{}", request.request_id),
                evidence_kind: "locked_full_file_codec".into(),
                producer: identity,
                claims: BTreeMap::from([
                    ("source_boundary".into(), json!("private_native_part10")),
                    (
                        "target_transfer_syntax_uid".into(),
                        json!(locked.target_transfer_syntax_uid),
                    ),
                    (
                        "decoded_native_sha256".into(),
                        json!(expected_native_sha256(locked)?),
                    ),
                ]),
            }],
        })
    }

    fn verify_identity(&self) -> Result<(), ServiceInvocationError> {
        let actual = self
            .encoder
            .discover_backend_identity()
            .map_err(|e| error(e.to_string()))?;
        self.compare_identity(actual.version.as_deref(), &actual.executable_sha256)
    }

    fn compare_identity(
        &self,
        version: Option<&str>,
        digest: &str,
    ) -> Result<(), ServiceInvocationError> {
        if version != Some(self.expected.version.as_str())
            || digest != self.expected.executable_sha256
        {
            return Err(error("dcmcjpeg identity differs from planning inventory"));
        }
        Ok(())
    }
}

fn expected_native_sha256(
    locked: &LockedFullFileCodecRequest,
) -> Result<String, ServiceInvocationError> {
    let content = locked
        .source_plan
        .content
        .first()
        .ok_or_else(|| error("source plan has no pixel content"))?;
    let Some(ContentMaterialization::Inline(bytes)) = &content.materialization else {
        return Err(error("source plan pixel content is not inline"));
    };
    Ok(sha256_hex(bytes))
}

fn validate_output(
    path: &Path,
    process: DcmtkDcmcjpegLosslessProcess,
    locked: &LockedFullFileCodecRequest,
) -> Result<(), ServiceInvocationError> {
    let object = open_file(path).map_err(|source| error(source.to_string()))?;
    if object.meta().transfer_syntax() != locked.target_transfer_syntax_uid {
        return Err(error("dcmcjpeg output transfer syntax differs from plan"));
    }
    let codec = match process {
        DcmtkDcmcjpegLosslessProcess::Process14 => JPEG_LOSSLESS_NON_HIERARCHICAL.codec(),
        DcmtkDcmcjpegLosslessProcess::Sv1 => {
            JPEG_LOSSLESS_NON_HIERARCHICAL_FIRST_ORDER_PREDICTION.codec()
        }
    };
    let Codec::EncapsulatedPixelData(Some(reader), _) = codec else {
        return Err(error("legacy JPEG decoder is unavailable"));
    };
    let mut decoded = Vec::new();
    reader
        .decode_frame(&object, 0, &mut decoded)
        .map_err(|source| error(source.to_string()))?;
    if sha256_hex(&decoded) != expected_native_sha256(locked)? {
        return Err(error(
            "decoded legacy JPEG frame differs from neutral source",
        ));
    }
    Ok(())
}

fn error(message: impl Into<String>) -> ServiceInvocationError {
    ServiceInvocationError::new("locked DCMTK full-file codec", message)
}
