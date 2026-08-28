//! Deterministic, payload-free evidence for protocol interoperability runs.
//!
//! This module models transaction results only. It does not open sockets,
//! execute tools, or accept certificate/private-key bytes. Harness adapters
//! supply public fingerprints and ordered observations after bounded runs.

use std::error::Error;
use std::fmt;

use serde::Serialize;
use serde_json::Value;

pub const PROTOCOL_QUALIFICATION_VERSION: &str = "0.1.0";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderRelationship {
    Independent,
    SameProvider,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ToolFingerprint {
    pub id: String,
    pub version: String,
    pub executable_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PeerFingerprint {
    pub id: String,
    pub implementation: String,
    pub version: String,
    pub artifact_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SourceCaseLink {
    pub case_id: String,
    pub path: String,
    pub sha256: String,
    pub sop_instance_uid: String,
}

/// Public-only identity material from `security/fixtures/fixtures.lock.json`.
/// There is deliberately no private-key path, hash, or byte field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PkiFixtureFingerprint {
    pub identity_id: String,
    pub certificate_sha256: String,
    pub certificate_fingerprint_sha256: String,
    pub public_key_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum StepOutcome {
    Passed,
    Rejected {
        code: String,
    },
    Timeout {
        limit_milliseconds: u64,
    },
    Crash {
        exit_code: Option<i32>,
        signal: Option<i32>,
    },
    Unavailable {
        blocker_code: String,
        reason: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProtocolStep {
    pub ordinal: u32,
    pub operation: String,
    pub outcome: StepOutcome,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DimseSection {
    pub calling_ae_title: String,
    pub called_ae_title: String,
    pub presentation_contexts: Vec<String>,
    pub operations: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DicomwebSection {
    pub server_identity: String,
    pub services: Vec<String>,
    pub media_types: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TlsSection {
    pub protocol_version: String,
    pub cipher_suite: String,
    pub mutual_tls: bool,
    pub user_identity_type: Option<String>,
    pub root_ca_identity_id: String,
    pub server_identity_id: String,
    pub client_identity_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ProtocolSection {
    Dimse(DimseSection),
    Dicomweb(DicomwebSection),
    Tls(TlsSection),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum QualificationStatus {
    Passed,
    Failed,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtocolQualificationInput {
    pub case_id: String,
    pub run_seed: u64,
    pub transaction_ordinal: u32,
    pub harness: ToolFingerprint,
    pub peer: PeerFingerprint,
    pub provider_relationship: ProviderRelationship,
    pub sources: Vec<SourceCaseLink>,
    pub steps: Vec<ProtocolStep>,
    pub section: ProtocolSection,
    pub pki_fixtures: Vec<PkiFixtureFingerprint>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProtocolQualification {
    pub contract_version: &'static str,
    pub case_id: String,
    pub transaction_id: String,
    pub harness: ToolFingerprint,
    pub peer: PeerFingerprint,
    pub provider_relationship: ProviderRelationship,
    pub sources: Vec<SourceCaseLink>,
    pub steps: Vec<ProtocolStep>,
    pub section: ProtocolSection,
    pub pki_fixtures: Vec<PkiFixtureFingerprint>,
    pub status: QualificationStatus,
}

impl ProtocolQualification {
    pub fn new(input: ProtocolQualificationInput) -> Result<Self, ProtocolEvidenceError> {
        validate_case_id(&input.case_id)?;
        if input.transaction_ordinal == 0 {
            return Err(ProtocolEvidenceError::InvalidField {
                field: "transaction_ordinal",
                message: "must be one-based",
            });
        }
        validate_tool(&input.harness)?;
        validate_peer(&input.peer)?;
        if input.sources.is_empty() {
            return Err(ProtocolEvidenceError::EmptyCollection("sources"));
        }
        for source in &input.sources {
            validate_source(source)?;
        }
        validate_steps(&input.steps)?;
        for fixture in &input.pki_fixtures {
            validate_pki_fixture(fixture)?;
        }
        validate_section(&input.section, &input.pki_fixtures)?;
        reject_secret_text(&input)?;

        let status = qualification_status(&input.steps);
        Ok(Self {
            contract_version: PROTOCOL_QUALIFICATION_VERSION,
            transaction_id: deterministic_transaction_id(
                &input.case_id,
                input.run_seed,
                input.transaction_ordinal,
            ),
            case_id: input.case_id,
            harness: input.harness,
            peer: input.peer,
            provider_relationship: input.provider_relationship,
            sources: input.sources,
            steps: input.steps,
            section: input.section,
            pki_fixtures: input.pki_fixtures,
            status,
        })
    }

    pub fn to_json(&self) -> Result<Value, serde_json::Error> {
        serde_json::to_value(self)
    }

    pub fn is_independent(&self) -> bool {
        self.provider_relationship == ProviderRelationship::Independent
    }

    pub fn unavailable_evidence(&self) -> impl Iterator<Item = (&str, &str)> {
        self.steps.iter().filter_map(|step| match &step.outcome {
            StepOutcome::Unavailable {
                blocker_code,
                reason,
            } => Some((blocker_code.as_str(), reason.as_str())),
            _ => None,
        })
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct ProtocolOutcomeCounts {
    pub passed: u64,
    pub failed: u64,
    pub unavailable: u64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct ProtocolReportSummary {
    pub dimse: ProtocolOutcomeCounts,
    pub dicomweb: ProtocolOutcomeCounts,
    pub tls: ProtocolOutcomeCounts,
    pub total: ProtocolOutcomeCounts,
}

/// A deterministic report projection which keeps protocol families separate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProtocolReport {
    pub contract_version: &'static str,
    pub transactions: Vec<ProtocolQualification>,
    pub summary: ProtocolReportSummary,
}

impl ProtocolReport {
    pub fn new(transactions: Vec<ProtocolQualification>) -> Result<Self, ProtocolEvidenceError> {
        if transactions.is_empty() {
            return Err(ProtocolEvidenceError::EmptyCollection("transactions"));
        }

        let mut transaction_ids = Vec::with_capacity(transactions.len());
        let mut summary = ProtocolReportSummary::default();
        for transaction in &transactions {
            if transaction_ids.contains(&transaction.transaction_id) {
                return Err(ProtocolEvidenceError::DuplicateTransaction(
                    transaction.transaction_id.clone(),
                ));
            }
            transaction_ids.push(transaction.transaction_id.clone());
            increment_outcome(&mut summary.total, transaction.status);
            let section_counts = match &transaction.section {
                ProtocolSection::Dimse(_) => &mut summary.dimse,
                ProtocolSection::Dicomweb(_) => &mut summary.dicomweb,
                ProtocolSection::Tls(_) => &mut summary.tls,
            };
            increment_outcome(section_counts, transaction.status);
        }

        Ok(Self {
            contract_version: PROTOCOL_QUALIFICATION_VERSION,
            transactions,
            summary,
        })
    }

    pub fn to_json(&self) -> Result<Value, serde_json::Error> {
        serde_json::to_value(self)
    }
}

fn increment_outcome(counts: &mut ProtocolOutcomeCounts, status: QualificationStatus) {
    match status {
        QualificationStatus::Passed => counts.passed += 1,
        QualificationStatus::Failed => counts.failed += 1,
        QualificationStatus::Unavailable => counts.unavailable += 1,
    }
}

pub fn deterministic_transaction_id(case_id: &str, run_seed: u64, ordinal: u32) -> String {
    let mut first = 0xcbf29ce484222325_u64;
    let mut second = 0x84222325cbf29ce4_u64;
    for byte in case_id
        .bytes()
        .chain(run_seed.to_le_bytes())
        .chain(ordinal.to_le_bytes())
    {
        first ^= u64::from(byte);
        first = first.wrapping_mul(0x100000001b3);
        second ^= u64::from(byte.rotate_left(1));
        second = second.wrapping_mul(0x100000001b3);
    }
    format!("txn-{first:016x}{second:016x}")
}

fn qualification_status(steps: &[ProtocolStep]) -> QualificationStatus {
    if steps.iter().any(|step| {
        matches!(
            step.outcome,
            StepOutcome::Timeout { .. } | StepOutcome::Crash { .. } | StepOutcome::Rejected { .. }
        )
    }) {
        QualificationStatus::Failed
    } else if steps
        .iter()
        .any(|step| matches!(step.outcome, StepOutcome::Unavailable { .. }))
    {
        QualificationStatus::Unavailable
    } else {
        QualificationStatus::Passed
    }
}

fn validate_case_id(case_id: &str) -> Result<(), ProtocolEvidenceError> {
    if !case_id.starts_with("protocol/")
        || case_id.split('/').count() < 3
        || !case_id.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"/_-".contains(&byte)
        })
    {
        return Err(ProtocolEvidenceError::InvalidField {
            field: "case_id",
            message: "must be a lowercase protocol/*/* case ID",
        });
    }
    Ok(())
}

fn validate_tool(tool: &ToolFingerprint) -> Result<(), ProtocolEvidenceError> {
    require_text("harness.id", &tool.id)?;
    require_text("harness.version", &tool.version)?;
    require_sha256("harness.executable_sha256", &tool.executable_sha256)
}

fn validate_peer(peer: &PeerFingerprint) -> Result<(), ProtocolEvidenceError> {
    require_text("peer.id", &peer.id)?;
    require_text("peer.implementation", &peer.implementation)?;
    require_text("peer.version", &peer.version)?;
    require_sha256("peer.artifact_sha256", &peer.artifact_sha256)
}

fn validate_source(source: &SourceCaseLink) -> Result<(), ProtocolEvidenceError> {
    if !source.case_id.contains('/') {
        return Err(ProtocolEvidenceError::InvalidField {
            field: "sources.case_id",
            message: "must be a stable registry case ID",
        });
    }
    require_text("sources.path", &source.path)?;
    require_sha256("sources.sha256", &source.sha256)?;
    if !valid_uid(&source.sop_instance_uid) {
        return Err(ProtocolEvidenceError::InvalidField {
            field: "sources.sop_instance_uid",
            message: "must be a valid DICOM UID",
        });
    }
    Ok(())
}

fn validate_steps(steps: &[ProtocolStep]) -> Result<(), ProtocolEvidenceError> {
    if steps.is_empty() {
        return Err(ProtocolEvidenceError::EmptyCollection("steps"));
    }
    for (index, step) in steps.iter().enumerate() {
        if step.ordinal != index as u32 + 1 {
            return Err(ProtocolEvidenceError::StepOrder {
                expected: index as u32 + 1,
                actual: step.ordinal,
            });
        }
        require_text("steps.operation", &step.operation)?;
        require_text("steps.detail", &step.detail)?;
        match &step.outcome {
            StepOutcome::Rejected { code } => require_text("steps.rejected.code", code)?,
            StepOutcome::Timeout { limit_milliseconds } if *limit_milliseconds == 0 => {
                return Err(ProtocolEvidenceError::InvalidField {
                    field: "steps.timeout.limit_milliseconds",
                    message: "must be greater than zero",
                });
            }
            StepOutcome::Crash {
                exit_code: None,
                signal: None,
            } => {
                return Err(ProtocolEvidenceError::InvalidField {
                    field: "steps.crash",
                    message: "must record an exit code or signal",
                });
            }
            StepOutcome::Unavailable {
                blocker_code,
                reason,
            } => {
                require_text("steps.unavailable.blocker_code", blocker_code)?;
                require_text("steps.unavailable.reason", reason)?;
            }
            _ => {}
        }
    }
    Ok(())
}

fn validate_pki_fixture(fixture: &PkiFixtureFingerprint) -> Result<(), ProtocolEvidenceError> {
    require_text("pki_fixtures.identity_id", &fixture.identity_id)?;
    require_sha256(
        "pki_fixtures.certificate_sha256",
        &fixture.certificate_sha256,
    )?;
    require_sha256(
        "pki_fixtures.certificate_fingerprint_sha256",
        &fixture.certificate_fingerprint_sha256,
    )?;
    require_sha256("pki_fixtures.public_key_sha256", &fixture.public_key_sha256)
}

fn validate_section(
    section: &ProtocolSection,
    fixtures: &[PkiFixtureFingerprint],
) -> Result<(), ProtocolEvidenceError> {
    match section {
        ProtocolSection::Dimse(dimse) => {
            require_text("dimse.calling_ae_title", &dimse.calling_ae_title)?;
            require_text("dimse.called_ae_title", &dimse.called_ae_title)?;
            require_nonempty_strings("dimse.presentation_contexts", &dimse.presentation_contexts)?;
            require_nonempty_strings("dimse.operations", &dimse.operations)
        }
        ProtocolSection::Dicomweb(web) => {
            require_text("dicomweb.server_identity", &web.server_identity)?;
            require_nonempty_strings("dicomweb.services", &web.services)?;
            require_nonempty_strings("dicomweb.media_types", &web.media_types)
        }
        ProtocolSection::Tls(tls) => {
            require_text("tls.protocol_version", &tls.protocol_version)?;
            require_text("tls.cipher_suite", &tls.cipher_suite)?;
            let fixture_ids = fixtures
                .iter()
                .map(|fixture| fixture.identity_id.as_str())
                .collect::<Vec<_>>();
            for (field, identity) in [
                (
                    "tls.root_ca_identity_id",
                    Some(tls.root_ca_identity_id.as_str()),
                ),
                (
                    "tls.server_identity_id",
                    Some(tls.server_identity_id.as_str()),
                ),
                ("tls.client_identity_id", tls.client_identity_id.as_deref()),
            ] {
                if let Some(identity) = identity {
                    if !fixture_ids.contains(&identity) {
                        return Err(ProtocolEvidenceError::UnknownPkiIdentity {
                            field,
                            identity: identity.to_string(),
                        });
                    }
                }
            }
            if tls.mutual_tls && tls.client_identity_id.is_none() {
                return Err(ProtocolEvidenceError::InvalidField {
                    field: "tls.client_identity_id",
                    message: "is required when mutual_tls is true",
                });
            }
            Ok(())
        }
    }
}

fn require_nonempty_strings(
    field: &'static str,
    values: &[String],
) -> Result<(), ProtocolEvidenceError> {
    if values.is_empty() {
        return Err(ProtocolEvidenceError::EmptyCollection(field));
    }
    for value in values {
        require_text(field, value)?;
    }
    Ok(())
}

fn require_text(field: &'static str, value: &str) -> Result<(), ProtocolEvidenceError> {
    if value.trim().is_empty() {
        return Err(ProtocolEvidenceError::InvalidField {
            field,
            message: "must not be empty",
        });
    }
    Ok(())
}

fn require_sha256(field: &'static str, value: &str) -> Result<(), ProtocolEvidenceError> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(ProtocolEvidenceError::InvalidField {
            field,
            message: "must be a lowercase SHA-256 digest",
        });
    }
    Ok(())
}

fn valid_uid(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && !value.starts_with('.')
        && !value.ends_with('.')
        && value.split('.').all(|component| {
            !component.is_empty()
                && component.bytes().all(|byte| byte.is_ascii_digit())
                && (component == "0" || !component.starts_with('0'))
        })
}

fn reject_secret_text(input: &ProtocolQualificationInput) -> Result<(), ProtocolEvidenceError> {
    let serialized = serde_json::to_string(&input_for_secret_scan(input))
        .expect("protocol secret-scan view is serializable");
    let lowercase = serialized.to_ascii_lowercase();
    for marker in [
        "begin private key",
        "begin rsa private key",
        "authorization: bearer",
        "password=",
    ] {
        if lowercase.contains(marker) {
            return Err(ProtocolEvidenceError::SecretMaterial(marker));
        }
    }
    Ok(())
}

#[derive(Serialize)]
struct SecretScanView<'a> {
    case_id: &'a str,
    harness: &'a ToolFingerprint,
    peer: &'a PeerFingerprint,
    sources: &'a [SourceCaseLink],
    steps: &'a [ProtocolStep],
    section: &'a ProtocolSection,
    pki_fixtures: &'a [PkiFixtureFingerprint],
}

fn input_for_secret_scan(input: &ProtocolQualificationInput) -> SecretScanView<'_> {
    SecretScanView {
        case_id: &input.case_id,
        harness: &input.harness,
        peer: &input.peer,
        sources: &input.sources,
        steps: &input.steps,
        section: &input.section,
        pki_fixtures: &input.pki_fixtures,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProtocolEvidenceError {
    InvalidField {
        field: &'static str,
        message: &'static str,
    },
    EmptyCollection(&'static str),
    StepOrder {
        expected: u32,
        actual: u32,
    },
    UnknownPkiIdentity {
        field: &'static str,
        identity: String,
    },
    DuplicateTransaction(String),
    SecretMaterial(&'static str),
}

impl fmt::Display for ProtocolEvidenceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidField { field, message } => write!(formatter, "{field} {message}"),
            Self::EmptyCollection(field) => write!(formatter, "{field} must not be empty"),
            Self::StepOrder { expected, actual } => write!(
                formatter,
                "protocol step ordinal {actual} is out of order; expected {expected}"
            ),
            Self::UnknownPkiIdentity { field, identity } => {
                write!(
                    formatter,
                    "{field} references unknown public fixture {identity}"
                )
            }
            Self::DuplicateTransaction(transaction_id) => {
                write!(formatter, "duplicate protocol transaction {transaction_id}")
            }
            Self::SecretMaterial(marker) => {
                write!(
                    formatter,
                    "protocol evidence contains forbidden secret marker {marker}"
                )
            }
        }
    }
}

impl Error for ProtocolEvidenceError {}
