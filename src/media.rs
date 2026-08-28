//! Policy primitives for deterministic DICOM media File-sets.
//!
//! This module performs no file I/O and invokes no external programs. It models
//! the inputs and evidence needed by a future media generator: conforming File
//! IDs, a closed mixed-object File-set, the locked DCMTK provider result, and a
//! semantic-stable qualification decision. DICOM files and DICOMDIR outputs
//! remain generated artifacts rather than committed fixtures.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

pub const MEDIA_CONTRACT_VERSION: &str = "0.1.0";
pub const DICOMDIR_FILE_ID: &str = "DICOMDIR";
pub const MEDIA_STORAGE_DIRECTORY_SOP_CLASS_UID: &str = "1.2.840.10008.1.3.10";
pub const EXPLICIT_VR_LITTLE_ENDIAN_UID: &str = "1.2.840.10008.1.2.1";
pub const LOCKED_PROVIDER_ID: &str = "dcmtk";
pub const LOCKED_PROVIDER_VERSION: &str = "3.7.0";
pub const LOCKED_FILE_SET_ID: &str = "DTSMIXED";

/// A PS3.10 File ID with one through eight restricted Components.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct FileId(Vec<String>);

impl FileId {
    pub fn new<I, S>(components: I) -> Result<Self, MediaError>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let components = components.into_iter().map(Into::into).collect::<Vec<_>>();
        if components.is_empty() || components.len() > 8 {
            return Err(MediaError::InvalidFileId(
                "a File ID must contain one through eight Components",
            ));
        }
        if components.iter().any(|component| {
            component.is_empty()
                || component.len() > 8
                || !component
                    .bytes()
                    .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
        }) {
            return Err(MediaError::InvalidFileId(
                "Components must contain one through eight uppercase letters, digits, or underscores",
            ));
        }
        Ok(Self(components))
    }

    /// Assign a stable, extension-free File ID for a one-based role ordinal.
    pub fn for_member(role: MemberRole, ordinal: u32) -> Result<Self, MediaError> {
        if ordinal == 0 || ordinal > 999_999 {
            return Err(MediaError::InvalidFileId(
                "deterministic member ordinals must be in 1..=999999",
            ));
        }
        let (directory, prefix) = match role {
            MemberRole::Image => ("IMAGE", "IM"),
            MemberRole::Derived => ("DERIVED", "DR"),
            MemberRole::NonImage => ("NONIMAGE", "NI"),
        };
        Self::new([directory.to_owned(), format!("{prefix}{ordinal:06}")])
    }

    pub fn components(&self) -> &[String] {
        &self.0
    }

    pub fn display(&self) -> String {
        self.0.join("\\")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MemberRole {
    Image,
    Derived,
    NonImage,
}

/// A generated DICOM member before it is copied into private media staging.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileSetMember {
    pub case_id: String,
    pub role: MemberRole,
    pub file_id: FileId,
    pub sha256: String,
    pub sop_class_uid: String,
    pub sop_instance_uid: String,
    /// SOP Instance UIDs referenced by this member's dataset.
    pub referenced_sop_instance_uids: Vec<String>,
}

/// The identity recorded by one DICOMDIR Directory Record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectoryRecordReference {
    pub file_id: FileId,
    pub referenced_sop_class_uid: String,
    pub referenced_sop_instance_uid: String,
}

/// A validated initial File-set containing image, derived, and non-image data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MixedFileSet {
    members: Vec<FileSetMember>,
}

impl MixedFileSet {
    pub fn validate(
        mut members: Vec<FileSetMember>,
        directory_records: &[DirectoryRecordReference],
    ) -> Result<Self, MediaError> {
        if members.is_empty() {
            return Err(MediaError::EmptyFileSet);
        }

        members.sort_by(|left, right| left.file_id.cmp(&right.file_id));
        let mut file_ids = BTreeSet::new();
        let mut sop_instances = BTreeSet::new();
        let mut roles = BTreeSet::new();
        for member in &members {
            validate_member(member)?;
            if !file_ids.insert(member.file_id.clone()) {
                return Err(MediaError::DuplicateFileId(member.file_id.display()));
            }
            if !sop_instances.insert(member.sop_instance_uid.clone()) {
                return Err(MediaError::DuplicateSopInstance(
                    member.sop_instance_uid.clone(),
                ));
            }
            roles.insert(member.role);
        }

        for role in [MemberRole::Image, MemberRole::Derived, MemberRole::NonImage] {
            if !roles.contains(&role) {
                return Err(MediaError::MissingMemberRole(role));
            }
        }

        for member in &members {
            if member.role == MemberRole::Derived && member.referenced_sop_instance_uids.is_empty()
            {
                return Err(MediaError::DerivedMemberWithoutSource(
                    member.sop_instance_uid.clone(),
                ));
            }
            for referenced_uid in &member.referenced_sop_instance_uids {
                if !sop_instances.contains(referenced_uid) {
                    return Err(MediaError::ReferenceOutsideFileSet {
                        member: member.sop_instance_uid.clone(),
                        referenced: referenced_uid.clone(),
                    });
                }
            }
        }

        let expected = members
            .iter()
            .map(|member| {
                (
                    member.file_id.clone(),
                    (
                        member.sop_class_uid.as_str(),
                        member.sop_instance_uid.as_str(),
                    ),
                )
            })
            .collect::<BTreeMap<_, _>>();
        let mut observed = BTreeMap::new();
        for record in directory_records {
            validate_uid(&record.referenced_sop_class_uid)?;
            validate_uid(&record.referenced_sop_instance_uid)?;
            if observed
                .insert(
                    record.file_id.clone(),
                    (
                        record.referenced_sop_class_uid.as_str(),
                        record.referenced_sop_instance_uid.as_str(),
                    ),
                )
                .is_some()
            {
                return Err(MediaError::DuplicateDirectoryReference(
                    record.file_id.display(),
                ));
            }
        }
        if expected != observed {
            return Err(MediaError::DirectoryClosureMismatch);
        }

        Ok(Self { members })
    }

    pub fn members(&self) -> &[FileSetMember] {
        &self.members
    }
}

fn validate_member(member: &FileSetMember) -> Result<(), MediaError> {
    if member.case_id.is_empty() {
        return Err(MediaError::InvalidMember("case_id must not be empty"));
    }
    validate_sha256(&member.sha256)?;
    validate_uid(&member.sop_class_uid)?;
    validate_uid(&member.sop_instance_uid)?;
    if member
        .referenced_sop_instance_uids
        .iter()
        .collect::<BTreeSet<_>>()
        .len()
        != member.referenced_sop_instance_uids.len()
    {
        return Err(MediaError::InvalidMember(
            "a member must not repeat an instance reference",
        ));
    }
    for uid in &member.referenced_sop_instance_uids {
        validate_uid(uid)?;
    }
    Ok(())
}

fn validate_uid(uid: &str) -> Result<(), MediaError> {
    if uid.is_empty()
        || uid.len() > 64
        || uid.starts_with('.')
        || uid.ends_with('.')
        || uid.split('.').any(|component| {
            component.is_empty() || !component.bytes().all(|byte| byte.is_ascii_digit())
        })
    {
        return Err(MediaError::InvalidUid(uid.to_owned()));
    }
    Ok(())
}

fn validate_sha256(value: &str) -> Result<(), MediaError> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(MediaError::InvalidSha256(value.to_owned()));
    }
    Ok(())
}

/// Locked identity and exact invocation metadata for the DCMTK provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DcmtkProviderFingerprint {
    pub provider_id: String,
    pub executable_name: String,
    pub version: String,
    pub executable_sha256: String,
    /// The argument vector excludes the executable and is never shell text.
    pub arguments: Vec<String>,
}

impl DcmtkProviderFingerprint {
    pub fn validate(&self) -> Result<(), MediaError> {
        if self.provider_id != LOCKED_PROVIDER_ID
            || self.executable_name != "dcmmkdir"
            || self.version != LOCKED_PROVIDER_VERSION
        {
            return Err(MediaError::ProviderLockMismatch);
        }
        validate_sha256(&self.executable_sha256)?;
        let required = ["-Pgp", "+F", "+id", "+r", "+D"];
        if required
            .iter()
            .any(|argument| !self.arguments.iter().any(|actual| actual == argument))
            || !argument_value(&self.arguments, "+F")
                .is_some_and(|value| value == LOCKED_FILE_SET_ID)
            || !argument_value(&self.arguments, "+id").is_some_and(|value| !value.is_empty())
            || !argument_value(&self.arguments, "+D")
                .is_some_and(|value| value.ends_with(DICOMDIR_FILE_ID))
        {
            return Err(MediaError::InvalidProviderArguments);
        }
        Ok(())
    }
}

fn argument_value<'a>(arguments: &'a [String], name: &str) -> Option<&'a str> {
    arguments
        .windows(2)
        .find(|pair| pair[0] == name)
        .map(|pair| pair[1].as_str())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DcmtkProviderResult {
    pub fingerprint: DcmtkProviderFingerprint,
    pub exit_code: i32,
    pub file_set_id: String,
    pub file_set_uid: String,
    /// Identities parsed back from DICOMDIR File Meta Information.
    pub dicomdir_sop_class_uid: String,
    pub dicomdir_sop_instance_uid: String,
    pub dicomdir_transfer_syntax_uid: String,
    pub dicomdir_sha256: String,
    /// File hashes observed after promotion staging, keyed by File ID.
    pub member_sha256: BTreeMap<FileId, String>,
    pub warnings: Vec<String>,
}

impl DcmtkProviderResult {
    fn validate(&self, file_set: &MixedFileSet) -> Result<(), MediaError> {
        self.fingerprint.validate()?;
        if self.exit_code != 0 {
            return Err(MediaError::ProviderFailed(self.exit_code));
        }
        if self.file_set_id != LOCKED_FILE_SET_ID {
            return Err(MediaError::ProviderLockMismatch);
        }
        validate_uid(&self.file_set_uid)?;
        validate_uid(&self.dicomdir_sop_class_uid)?;
        validate_uid(&self.dicomdir_sop_instance_uid)?;
        validate_uid(&self.dicomdir_transfer_syntax_uid)?;
        if self.dicomdir_sop_class_uid != MEDIA_STORAGE_DIRECTORY_SOP_CLASS_UID
            || self.dicomdir_sop_instance_uid != self.file_set_uid
            || self.dicomdir_transfer_syntax_uid != EXPLICIT_VR_LITTLE_ENDIAN_UID
        {
            return Err(MediaError::DicomDirIdentityMismatch);
        }
        validate_sha256(&self.dicomdir_sha256)?;
        let expected = file_set
            .members()
            .iter()
            .map(|member| (member.file_id.clone(), member.sha256.clone()))
            .collect::<BTreeMap<_, _>>();
        if expected != self.member_sha256 {
            return Err(MediaError::ProviderMemberHashMismatch);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckStatus {
    Passed,
    Unavailable,
    Failed,
}

/// Evidence classes remain separate so same-provider checks cannot be
/// mislabeled as independent interoperability evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MediaValidationEvidence {
    pub rust_closure: CheckStatus,
    pub dicom3tools_dciodvfy: CheckStatus,
    pub dicom3tools_dcentvfy: CheckStatus,
    pub dcmtk_parser_same_family: CheckStatus,
    pub dcm4che_independent_peer: CheckStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaDeterminism {
    SemanticStable,
}

/// Payload-free decision record for a generated DICOMDIR File-set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DicomDirQualification {
    pub contract_version: &'static str,
    pub determinism: MediaDeterminism,
    pub file_set_id: String,
    pub file_set_uid: String,
    pub sop_class_uid: &'static str,
    pub transfer_syntax_uid: &'static str,
    pub dicomdir_sha256: String,
    pub member_count: usize,
    pub provider: DcmtkProviderFingerprint,
    pub provider_warnings: Vec<String>,
    pub evidence: MediaValidationEvidence,
    pub independent_interoperability_proven: bool,
}

impl DicomDirQualification {
    pub fn qualify(
        file_set: &MixedFileSet,
        provider_result: DcmtkProviderResult,
        evidence: MediaValidationEvidence,
    ) -> Result<Self, MediaError> {
        provider_result.validate(file_set)?;
        if evidence.rust_closure != CheckStatus::Passed
            || evidence.dicom3tools_dciodvfy != CheckStatus::Passed
            || !matches!(
                evidence.dicom3tools_dcentvfy,
                CheckStatus::Passed | CheckStatus::Unavailable
            )
            || evidence.dcmtk_parser_same_family != CheckStatus::Passed
            || evidence.dcm4che_independent_peer == CheckStatus::Failed
        {
            return Err(MediaError::ValidationEvidenceInsufficient);
        }
        let independent_interoperability_proven =
            evidence.dcm4che_independent_peer == CheckStatus::Passed;
        Ok(Self {
            contract_version: MEDIA_CONTRACT_VERSION,
            determinism: MediaDeterminism::SemanticStable,
            file_set_id: provider_result.file_set_id,
            file_set_uid: provider_result.file_set_uid,
            sop_class_uid: MEDIA_STORAGE_DIRECTORY_SOP_CLASS_UID,
            transfer_syntax_uid: EXPLICIT_VR_LITTLE_ENDIAN_UID,
            dicomdir_sha256: provider_result.dicomdir_sha256,
            member_count: file_set.members().len(),
            provider: provider_result.fingerprint,
            provider_warnings: provider_result.warnings,
            evidence,
            independent_interoperability_proven,
        })
    }

    pub fn is_promotable(&self) -> bool {
        self.independent_interoperability_proven
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MediaError {
    InvalidFileId(&'static str),
    EmptyFileSet,
    InvalidMember(&'static str),
    InvalidUid(String),
    InvalidSha256(String),
    DuplicateFileId(String),
    DuplicateSopInstance(String),
    MissingMemberRole(MemberRole),
    DerivedMemberWithoutSource(String),
    ReferenceOutsideFileSet { member: String, referenced: String },
    DuplicateDirectoryReference(String),
    DirectoryClosureMismatch,
    ProviderLockMismatch,
    InvalidProviderArguments,
    ProviderFailed(i32),
    DicomDirIdentityMismatch,
    ProviderMemberHashMismatch,
    ValidationEvidenceInsufficient,
}

impl fmt::Display for MediaError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidFileId(detail) => write!(formatter, "invalid DICOM File ID: {detail}"),
            Self::EmptyFileSet => write!(formatter, "the DICOM File-set is empty"),
            Self::InvalidMember(detail) => write!(formatter, "invalid File-set member: {detail}"),
            Self::InvalidUid(uid) => write!(formatter, "invalid DICOM UID: {uid}"),
            Self::InvalidSha256(value) => write!(formatter, "invalid SHA-256: {value}"),
            Self::DuplicateFileId(file_id) => write!(formatter, "duplicate File ID: {file_id}"),
            Self::DuplicateSopInstance(uid) => {
                write!(formatter, "duplicate SOP Instance UID: {uid}")
            }
            Self::MissingMemberRole(role) => write!(formatter, "mixed File-set lacks {role:?}"),
            Self::DerivedMemberWithoutSource(uid) => {
                write!(formatter, "derived instance {uid} has no source reference")
            }
            Self::ReferenceOutsideFileSet { member, referenced } => write!(
                formatter,
                "instance {member} references {referenced} outside the File-set"
            ),
            Self::DuplicateDirectoryReference(file_id) => {
                write!(
                    formatter,
                    "File ID {file_id} is directly referenced more than once"
                )
            }
            Self::DirectoryClosureMismatch => write!(
                formatter,
                "Directory Records do not reference every File-set member exactly once"
            ),
            Self::ProviderLockMismatch => write!(formatter, "DCMTK provider lock mismatch"),
            Self::InvalidProviderArguments => {
                write!(
                    formatter,
                    "dcmmkdir arguments lack the locked media options"
                )
            }
            Self::ProviderFailed(code) => write!(formatter, "dcmmkdir exited with status {code}"),
            Self::DicomDirIdentityMismatch => write!(
                formatter,
                "DICOMDIR File Meta identities do not match the File-set contract"
            ),
            Self::ProviderMemberHashMismatch => {
                write!(
                    formatter,
                    "provider result does not preserve every member hash"
                )
            }
            Self::ValidationEvidenceInsufficient => {
                write!(formatter, "DICOMDIR validation evidence is insufficient")
            }
        }
    }
}

impl Error for MediaError {}

#[cfg(test)]
mod tests {
    use super::*;

    const IMAGE_UID: &str = "1.2.826.0.1.3680043.10.543.1";
    const DERIVED_UID: &str = "1.2.826.0.1.3680043.10.543.2";
    const NON_IMAGE_UID: &str = "1.2.826.0.1.3680043.10.543.3";

    fn hash(byte: char) -> String {
        std::iter::repeat_n(byte, 64).collect()
    }

    fn fixture() -> (Vec<FileSetMember>, Vec<DirectoryRecordReference>) {
        let members = vec![
            FileSetMember {
                case_id: "classic/ct/source".into(),
                role: MemberRole::Image,
                file_id: FileId::for_member(MemberRole::Image, 1).unwrap(),
                sha256: hash('a'),
                sop_class_uid: "1.2.840.10008.5.1.4.1.1.2".into(),
                sop_instance_uid: IMAGE_UID.into(),
                referenced_sop_instance_uids: vec![],
            },
            FileSetMember {
                case_id: "derived/seg/source".into(),
                role: MemberRole::Derived,
                file_id: FileId::for_member(MemberRole::Derived, 1).unwrap(),
                sha256: hash('b'),
                sop_class_uid: "1.2.840.10008.5.1.4.1.1.66.4".into(),
                sop_instance_uid: DERIVED_UID.into(),
                referenced_sop_instance_uids: vec![IMAGE_UID.into()],
            },
            FileSetMember {
                case_id: "nonimage/sr/report".into(),
                role: MemberRole::NonImage,
                file_id: FileId::for_member(MemberRole::NonImage, 1).unwrap(),
                sha256: hash('c'),
                sop_class_uid: "1.2.840.10008.5.1.4.1.1.88.33".into(),
                sop_instance_uid: NON_IMAGE_UID.into(),
                referenced_sop_instance_uids: vec![],
            },
        ];
        let records = members
            .iter()
            .map(|member| DirectoryRecordReference {
                file_id: member.file_id.clone(),
                referenced_sop_class_uid: member.sop_class_uid.clone(),
                referenced_sop_instance_uid: member.sop_instance_uid.clone(),
            })
            .collect();
        (members, records)
    }

    fn provider_result(file_set: &MixedFileSet) -> DcmtkProviderResult {
        DcmtkProviderResult {
            fingerprint: DcmtkProviderFingerprint {
                provider_id: LOCKED_PROVIDER_ID.into(),
                executable_name: "dcmmkdir".into(),
                version: LOCKED_PROVIDER_VERSION.into(),
                executable_sha256: hash('d'),
                arguments: vec![
                    "-Pgp".into(),
                    "+F".into(),
                    "DTSMIXED".into(),
                    "+id".into(),
                    "/staging".into(),
                    "+r".into(),
                    "+D".into(),
                    "/staging/DICOMDIR".into(),
                ],
            },
            exit_code: 0,
            file_set_id: "DTSMIXED".into(),
            file_set_uid: "1.2.826.0.1.3680043.10.543.8".into(),
            dicomdir_sop_class_uid: MEDIA_STORAGE_DIRECTORY_SOP_CLASS_UID.into(),
            dicomdir_sop_instance_uid: "1.2.826.0.1.3680043.10.543.8".into(),
            dicomdir_transfer_syntax_uid: EXPLICIT_VR_LITTLE_ENDIAN_UID.into(),
            dicomdir_sha256: hash('e'),
            member_sha256: file_set
                .members()
                .iter()
                .map(|member| (member.file_id.clone(), member.sha256.clone()))
                .collect(),
            warnings: vec![],
        }
    }

    fn evidence() -> MediaValidationEvidence {
        MediaValidationEvidence {
            rust_closure: CheckStatus::Passed,
            dicom3tools_dciodvfy: CheckStatus::Passed,
            dicom3tools_dcentvfy: CheckStatus::Passed,
            dcmtk_parser_same_family: CheckStatus::Passed,
            dcm4che_independent_peer: CheckStatus::Unavailable,
        }
    }

    #[test]
    fn deterministic_file_ids_are_conforming_and_extension_free() {
        let file_id = FileId::for_member(MemberRole::Derived, 42).unwrap();
        assert_eq!(file_id.display(), "DERIVED\\DR000042");
        assert!(
            file_id
                .components()
                .iter()
                .all(|component| component.len() <= 8)
        );
        assert!(!file_id.display().contains('.'));
    }

    #[test]
    fn rejects_invalid_file_id_components_and_counts() {
        assert!(FileId::new(Vec::<String>::new()).is_err());
        assert!(FileId::new(["lower"]).is_err());
        assert!(FileId::new(["TOO_LONG9"]).is_err());
        assert!(FileId::new(["HAS.DOT"]).is_err());
        assert!(FileId::new(["A", "B", "C", "D", "E", "F", "G", "H", "I"]).is_err());
    }

    #[test]
    fn accepts_a_closed_mixed_file_set_independent_of_input_order() {
        let (mut members, records) = fixture();
        members.reverse();
        let file_set = MixedFileSet::validate(members, &records).unwrap();
        assert_eq!(file_set.members().len(), 3);
        assert!(
            file_set
                .members()
                .windows(2)
                .all(|pair| pair[0].file_id < pair[1].file_id)
        );
    }

    #[test]
    fn rejects_missing_and_duplicate_directory_references() {
        let (members, mut records) = fixture();
        records.pop();
        assert_eq!(
            MixedFileSet::validate(members.clone(), &records),
            Err(MediaError::DirectoryClosureMismatch)
        );
        records.push(records[0].clone());
        assert!(matches!(
            MixedFileSet::validate(members, &records),
            Err(MediaError::DuplicateDirectoryReference(_))
        ));
    }

    #[test]
    fn rejects_external_dataset_reference() {
        let (mut members, records) = fixture();
        members[1].referenced_sop_instance_uids = vec!["1.2.3.999".into()];
        assert!(matches!(
            MixedFileSet::validate(members, &records),
            Err(MediaError::ReferenceOutsideFileSet { .. })
        ));
    }

    #[test]
    fn requires_each_initial_object_family_and_a_derived_source() {
        let (members, records) = fixture();
        assert_eq!(
            MixedFileSet::validate(members[..2].to_vec(), &records[..2]),
            Err(MediaError::MissingMemberRole(MemberRole::NonImage))
        );
        let (mut members, records) = fixture();
        members[1].referenced_sop_instance_uids.clear();
        assert!(matches!(
            MixedFileSet::validate(members, &records),
            Err(MediaError::DerivedMemberWithoutSource(_))
        ));
    }

    #[test]
    fn provider_result_locks_dcmtk_identity_arguments_and_member_hashes() {
        let (members, records) = fixture();
        let file_set = MixedFileSet::validate(members, &records).unwrap();
        provider_result(&file_set).validate(&file_set).unwrap();

        let mut bad = provider_result(&file_set);
        bad.fingerprint.version = "3.6.9".into();
        assert_eq!(
            bad.validate(&file_set),
            Err(MediaError::ProviderLockMismatch)
        );

        let mut bad = provider_result(&file_set);
        bad.member_sha256.clear();
        assert_eq!(
            bad.validate(&file_set),
            Err(MediaError::ProviderMemberHashMismatch)
        );

        let mut bad = provider_result(&file_set);
        bad.dicomdir_transfer_syntax_uid = "1.2.840.10008.1.2".into();
        assert_eq!(
            bad.validate(&file_set),
            Err(MediaError::DicomDirIdentityMismatch)
        );

        let mut bad = provider_result(&file_set);
        bad.dicomdir_sop_instance_uid = "1.2.826.0.1.3680043.10.543.99".into();
        assert_eq!(
            bad.validate(&file_set),
            Err(MediaError::DicomDirIdentityMismatch)
        );
    }

    #[test]
    fn qualifies_semantic_stability_with_explicit_peer_unavailability() {
        let (members, records) = fixture();
        let file_set = MixedFileSet::validate(members, &records).unwrap();
        let qualification =
            DicomDirQualification::qualify(&file_set, provider_result(&file_set), evidence())
                .unwrap();
        assert!(!qualification.is_promotable());
        assert_eq!(qualification.determinism, MediaDeterminism::SemanticStable);
        assert!(!qualification.independent_interoperability_proven);
        assert_eq!(qualification.file_set_uid, "1.2.826.0.1.3680043.10.543.8");
        assert_eq!(qualification.member_count, 3);
    }

    #[test]
    fn independent_peer_pass_is_the_only_interoperability_proof() {
        let (members, records) = fixture();
        let file_set = MixedFileSet::validate(members, &records).unwrap();
        let mut checks = evidence();
        checks.dcm4che_independent_peer = CheckStatus::Passed;
        let qualification =
            DicomDirQualification::qualify(&file_set, provider_result(&file_set), checks).unwrap();
        assert!(qualification.independent_interoperability_proven);
        assert!(qualification.is_promotable());
    }

    #[test]
    fn required_validator_or_same_family_parser_failure_blocks_qualification() {
        let (members, records) = fixture();
        let file_set = MixedFileSet::validate(members, &records).unwrap();
        for checks in [
            MediaValidationEvidence {
                dicom3tools_dciodvfy: CheckStatus::Unavailable,
                ..evidence()
            },
            MediaValidationEvidence {
                dcmtk_parser_same_family: CheckStatus::Failed,
                ..evidence()
            },
        ] {
            assert_eq!(
                DicomDirQualification::qualify(&file_set, provider_result(&file_set), checks),
                Err(MediaError::ValidationEvidenceInsufficient)
            );
        }
    }

    #[test]
    fn dcentvfy_may_be_explicitly_unavailable_where_unsupported() {
        let (members, records) = fixture();
        let file_set = MixedFileSet::validate(members, &records).unwrap();
        let mut checks = evidence();
        checks.dicom3tools_dcentvfy = CheckStatus::Unavailable;
        assert!(
            DicomDirQualification::qualify(&file_set, provider_result(&file_set), checks).is_ok()
        );
    }

    #[test]
    fn failed_independent_peer_is_not_silently_downgraded_to_unavailable() {
        let (members, records) = fixture();
        let file_set = MixedFileSet::validate(members, &records).unwrap();
        let mut checks = evidence();
        checks.dcm4che_independent_peer = CheckStatus::Failed;
        assert_eq!(
            DicomDirQualification::qualify(&file_set, provider_result(&file_set), checks),
            Err(MediaError::ValidationEvidenceInsufficient)
        );
    }
}
