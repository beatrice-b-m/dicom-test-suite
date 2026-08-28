//! Deterministic Phase 8 baseline reports when independent peers are absent.
//!
//! This builder records explicit unavailability. It does not turn a
//! same-provider DCMTK exchange into independent interoperability evidence.

use std::error::Error;
use std::fmt;
use std::fs;
use std::path::Path;

use serde::Deserialize;

use crate::protocol::{
    DicomwebSection, DimseSection, PeerFingerprint, PkiFixtureFingerprint, ProtocolEvidenceError,
    ProtocolQualification, ProtocolQualificationInput, ProtocolReport, ProtocolSection,
    ProtocolStep, ProviderRelationship, SourceCaseLink, StepOutcome, TlsSection, ToolFingerprint,
};

const EMPTY_SHA256: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtocolBaselineInput {
    pub run_seed: u64,
    pub harness: ToolFingerprint,
    pub sources: Vec<SourceCaseLink>,
}

/// Build the three Phase 8 protocol-family records without claiming that
/// unconfigured independent infrastructure was exercised.
pub fn build_unavailable_protocol_baseline(
    input: ProtocolBaselineInput,
    fixture_lock_path: &Path,
) -> Result<ProtocolReport, ProtocolBaselineError> {
    if input.sources.is_empty() {
        return Err(ProtocolBaselineError::EmptySources);
    }

    let mut sources = input.sources;
    sources.sort_by(|left, right| {
        (
            &left.case_id,
            &left.path,
            &left.sop_instance_uid,
            &left.sha256,
        )
            .cmp(&(
                &right.case_id,
                &right.path,
                &right.sop_instance_uid,
                &right.sha256,
            ))
    });
    let fixtures = load_tls_public_fingerprints(fixture_lock_path)?;

    let transactions = vec![
        unavailable_dimse(input.run_seed, &input.harness, &sources)?,
        unavailable_dicomweb(input.run_seed, &input.harness, &sources)?,
        unavailable_tls(input.run_seed, &input.harness, &sources, fixtures)?,
    ];
    ProtocolReport::new(transactions).map_err(ProtocolBaselineError::Evidence)
}

pub fn protocol_report_markdown(report: &ProtocolReport) -> String {
    let mut output = String::from("# Protocol interoperability baseline\n\n");
    output.push_str(
        "This report records bounded qualification evidence; unavailable rows are not passes.\n\n",
    );
    output.push_str("| Family | Case | Status | Independent peer | Blocker |\n");
    output.push_str("| --- | --- | --- | --- | --- |\n");
    for transaction in &report.transactions {
        let family = match transaction.section {
            ProtocolSection::Dimse(_) => "DIMSE",
            ProtocolSection::Dicomweb(_) => "DICOMweb",
            ProtocolSection::Tls(_) => "TLS / user identity",
        };
        let blockers = transaction
            .unavailable_evidence()
            .map(|(code, _)| code)
            .collect::<Vec<_>>()
            .join(", ");
        output.push_str(&format!(
            "| {family} | `{}` | {:?} | {} | `{blockers}` |\n",
            transaction.case_id,
            transaction.status,
            if transaction.is_independent() {
                "required"
            } else {
                "no"
            },
        ));
    }
    output.push_str(&format!(
        "\nSummary: {} unavailable, {} passed, {} failed.\n",
        report.summary.total.unavailable, report.summary.total.passed, report.summary.total.failed,
    ));
    output
}

fn unavailable_dimse(
    seed: u64,
    harness: &ToolFingerprint,
    sources: &[SourceCaseLink],
) -> Result<ProtocolQualification, ProtocolBaselineError> {
    qualification(
        "protocol/dimse/storage_query_retrieve",
        seed,
        1,
        harness,
        unavailable_peer("dcm4che-peer-unconfigured", "dcm4che"),
        sources,
        ProtocolSection::Dimse(DimseSection {
            calling_ae_title: "DTS_SCU".into(),
            called_ae_title: "DTS_SCP".into(),
            presentation_contexts: vec![
                "Verification SOP Class".into(),
                "CT Image Storage".into(),
                "Study Root Query/Retrieve Information Model".into(),
            ],
            operations: vec![
                "C-ECHO".into(),
                "C-STORE".into(),
                "C-FIND".into(),
                "C-MOVE/C-GET".into(),
            ],
        }),
        "independent_dcm4che_peer_unavailable",
        "No replaceable dcm4che peer is configured. A DCMTK-to-DCMTK exchange is same-provider evidence and does not satisfy independent interoperability qualification.",
        Vec::new(),
    )
}

fn unavailable_dicomweb(
    seed: u64,
    harness: &ToolFingerprint,
    sources: &[SourceCaseLink],
) -> Result<ProtocolQualification, ProtocolBaselineError> {
    qualification(
        "protocol/dicomweb/stow_qido_wado",
        seed,
        2,
        harness,
        unavailable_peer(
            "dicomweb-server-unconfigured",
            "independent DICOMweb server",
        ),
        sources,
        ProtocolSection::Dicomweb(DicomwebSection {
            server_identity: "unconfigured-independent-server".into(),
            services: vec!["STOW-RS".into(), "QIDO-RS".into(), "WADO-RS".into()],
            media_types: vec![
                "application/dicom".into(),
                "multipart/related; type=application/dicom".into(),
            ],
        }),
        "pinned_independent_dicomweb_server_unavailable",
        "No pinned, replaceable independent DICOMweb server artifact is configured for bounded STOW/QIDO/WADO qualification.",
        Vec::new(),
    )
}

fn unavailable_tls(
    seed: u64,
    harness: &ToolFingerprint,
    sources: &[SourceCaseLink],
    fixtures: Vec<PkiFixtureFingerprint>,
) -> Result<ProtocolQualification, ProtocolBaselineError> {
    qualification(
        "protocol/security/tls_user_identity",
        seed,
        3,
        harness,
        unavailable_peer("tls-peer-unconfigured", "replaceable independent TLS peer"),
        sources,
        ProtocolSection::Tls(TlsSection {
            protocol_version: "not-negotiated".into(),
            cipher_suite: "not-negotiated".into(),
            mutual_tls: true,
            user_identity_type: Some("username-passcode (synthetic test identity)".into()),
            root_ca_identity_id: "test_root_ca".into(),
            server_identity_id: "tls_server".into(),
            client_identity_id: Some("tls_client".into()),
        }),
        "replaceable_tls_peer_unavailable",
        "The approved synthetic public PKI is fingerprinted, but no replaceable independent TLS/user-identity peer is configured; no handshake or authentication was attempted.",
        fixtures,
    )
}

#[allow(clippy::too_many_arguments)]
fn qualification(
    case_id: &str,
    seed: u64,
    ordinal: u32,
    harness: &ToolFingerprint,
    peer: PeerFingerprint,
    sources: &[SourceCaseLink],
    section: ProtocolSection,
    blocker_code: &str,
    reason: &str,
    pki_fixtures: Vec<PkiFixtureFingerprint>,
) -> Result<ProtocolQualification, ProtocolBaselineError> {
    ProtocolQualification::new(ProtocolQualificationInput {
        case_id: case_id.into(),
        run_seed: seed,
        transaction_ordinal: ordinal,
        harness: harness.clone(),
        peer,
        provider_relationship: ProviderRelationship::Independent,
        sources: sources.to_vec(),
        steps: vec![ProtocolStep {
            ordinal: 1,
            operation: "independent qualification".into(),
            outcome: StepOutcome::Unavailable {
                blocker_code: blocker_code.into(),
                reason: reason.into(),
            },
            detail: "No network transaction was started.".into(),
        }],
        section,
        pki_fixtures,
    })
    .map_err(ProtocolBaselineError::Evidence)
}

fn unavailable_peer(id: &str, implementation: &str) -> PeerFingerprint {
    PeerFingerprint {
        id: id.into(),
        implementation: implementation.into(),
        version: "unavailable".into(),
        artifact_sha256: EMPTY_SHA256.into(),
    }
}

fn load_tls_public_fingerprints(
    fixture_lock_path: &Path,
) -> Result<Vec<PkiFixtureFingerprint>, ProtocolBaselineError> {
    let bytes = fs::read(fixture_lock_path).map_err(ProtocolBaselineError::ReadFixtureLock)?;
    let lock: FixtureLock =
        serde_json::from_slice(&bytes).map_err(ProtocolBaselineError::ParseFixtureLock)?;
    ["test_root_ca", "tls_server", "tls_client"]
        .into_iter()
        .map(|id| {
            let identity = lock
                .identities
                .iter()
                .find(|identity| identity.id == id)
                .ok_or_else(|| ProtocolBaselineError::MissingPkiIdentity(id.into()))?;
            Ok(PkiFixtureFingerprint {
                identity_id: identity.id.clone(),
                certificate_sha256: identity.certificate_sha256.clone(),
                certificate_fingerprint_sha256: identity.certificate_fingerprint_sha256.clone(),
                public_key_sha256: identity.public_key_sha256.clone(),
            })
        })
        .collect()
}

#[derive(Deserialize)]
struct FixtureLock {
    identities: Vec<PublicFixtureIdentity>,
}

#[derive(Deserialize)]
struct PublicFixtureIdentity {
    id: String,
    certificate_sha256: String,
    certificate_fingerprint_sha256: String,
    public_key_sha256: String,
}

#[derive(Debug)]
pub enum ProtocolBaselineError {
    EmptySources,
    ReadFixtureLock(std::io::Error),
    ParseFixtureLock(serde_json::Error),
    MissingPkiIdentity(String),
    Evidence(ProtocolEvidenceError),
}

impl fmt::Display for ProtocolBaselineError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptySources => write!(formatter, "protocol baseline sources must not be empty"),
            Self::ReadFixtureLock(error) => {
                write!(formatter, "cannot read PKI fixture lock: {error}")
            }
            Self::ParseFixtureLock(error) => {
                write!(formatter, "cannot parse PKI fixture lock: {error}")
            }
            Self::MissingPkiIdentity(identity) => {
                write!(formatter, "PKI fixture lock is missing {identity}")
            }
            Self::Evidence(error) => write!(formatter, "invalid protocol evidence: {error}"),
        }
    }
}

impl Error for ProtocolBaselineError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::ReadFixtureLock(error) => Some(error),
            Self::ParseFixtureLock(error) => Some(error),
            Self::Evidence(error) => Some(error),
            Self::EmptySources | Self::MissingPkiIdentity(_) => None,
        }
    }
}
