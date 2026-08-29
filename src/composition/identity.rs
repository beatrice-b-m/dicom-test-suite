use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};

use super::{TemplateId, TemplateVersion};
use crate::sha256_hex;
use crate::uid::{PROJECT_NAMESPACE_UUID, is_valid_generated_uid};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompositionUidRole {
    StudyInstance,
    SeriesInstance,
    SopInstance,
    FrameOfReference,
    DimensionOrganization,
    IrradiationEvent,
    Concatenation,
    ConcatenationSource,
    Tracking,
    ImplementationClass,
    TemplateDefined(String),
}

impl CompositionUidRole {
    pub fn as_str(&self) -> &str {
        match self {
            Self::StudyInstance => "study_instance_uid",
            Self::SeriesInstance => "series_instance_uid",
            Self::SopInstance => "sop_instance_uid",
            Self::FrameOfReference => "frame_of_reference_uid",
            Self::DimensionOrganization => "dimension_organization_uid",
            Self::IrradiationEvent => "irradiation_event_uid",
            Self::Concatenation => "concatenation_uid",
            Self::ConcatenationSource => "sop_instance_uid_of_concatenation_source",
            Self::Tracking => "tracking_uid",
            Self::ImplementationClass => "implementation_class_uid",
            Self::TemplateDefined(role) => role,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdentityAllocator {
    standards_lock_sha256: String,
    template_id: TemplateId,
    template_version: TemplateVersion,
    run_seed: u64,
}

impl IdentityAllocator {
    pub fn new(
        standards_lock_sha256: impl Into<String>,
        template_id: TemplateId,
        template_version: TemplateVersion,
        run_seed: u64,
    ) -> Result<Self, IdentityError> {
        let standards_lock_sha256 = standards_lock_sha256.into();
        if standards_lock_sha256.len() != 64
            || !standards_lock_sha256
                .bytes()
                .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
        {
            return Err(IdentityError::InvalidStandardsLockHash(
                standards_lock_sha256,
            ));
        }
        Ok(Self {
            standards_lock_sha256,
            template_id,
            template_version,
            run_seed,
        })
    }

    pub fn allocate(
        &self,
        logical_instance_id: &str,
        role: &CompositionUidRole,
        index: u32,
    ) -> Result<String, IdentityError> {
        if !is_valid_logical_id(logical_instance_id) {
            return Err(IdentityError::InvalidLogicalInstanceId(
                logical_instance_id.to_string(),
            ));
        }
        let role_name = role.as_str();
        if !is_valid_role(role_name) {
            return Err(IdentityError::InvalidRole(role_name.to_string()));
        }
        let seed_material = format!(
            "namespace={PROJECT_NAMESPACE_UUID};standards_lock_sha256={};template_id={};template_version={};run_seed={};logical_instance_id={logical_instance_id};role={role_name};index={index}",
            self.standards_lock_sha256, self.template_id, self.template_version, self.run_seed
        );
        let digest = sha256_hex(seed_material.as_bytes());
        let mut uuid_bytes = [0_u8; 16];
        for (position, byte) in uuid_bytes.iter_mut().enumerate() {
            let start = position * 2;
            *byte = u8::from_str_radix(&digest[start..start + 2], 16)
                .expect("sha256_hex returns lowercase hexadecimal");
        }
        uuid_bytes[6] = (uuid_bytes[6] & 0x0f) | 0x50;
        uuid_bytes[8] = (uuid_bytes[8] & 0x3f) | 0x80;
        let uid = format!("2.25.{}", u128::from_be_bytes(uuid_bytes));
        debug_assert!(is_valid_generated_uid(&uid));
        Ok(uid)
    }

    pub fn allocate_plan(
        &self,
        logical_instance_id: impl Into<String>,
        roles: impl IntoIterator<Item = (CompositionUidRole, u32)>,
    ) -> Result<IdentityPlan, IdentityError> {
        let logical_instance_id = logical_instance_id.into();
        let mut identities = BTreeMap::new();
        for (role, index) in roles {
            let key = identity_key(&role, index);
            if identities
                .insert(key, self.allocate(&logical_instance_id, &role, index)?)
                .is_some()
            {
                return Err(IdentityError::DuplicateRoleIndex { role, index });
            }
        }
        Ok(IdentityPlan {
            logical_instance_id,
            identities,
        })
    }
}

fn is_valid_logical_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.bytes().enumerate().all(|(index, byte)| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || (index > 0 && matches!(byte, b'_' | b'-'))
        })
}

fn is_valid_role(value: &str) -> bool {
    is_valid_logical_id(value)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdentityPlan {
    pub logical_instance_id: String,
    pub identities: BTreeMap<String, String>,
}

impl IdentityPlan {
    pub fn from_exact_values(
        logical_instance_id: impl Into<String>,
        values: impl IntoIterator<Item = (CompositionUidRole, u32, String)>,
    ) -> Result<Self, IdentityError> {
        let logical_instance_id = logical_instance_id.into();
        if !is_valid_logical_id(&logical_instance_id) {
            return Err(IdentityError::InvalidLogicalInstanceId(logical_instance_id));
        }
        let mut identities = BTreeMap::new();
        for (role, index, value) in values {
            let key = identity_key(&role, index);
            if identities.insert(key, value).is_some() {
                return Err(IdentityError::DuplicateRoleIndex { role, index });
            }
        }
        Ok(Self {
            logical_instance_id,
            identities,
        })
    }

    pub fn get(&self, role: &CompositionUidRole, index: u32) -> Option<&str> {
        self.identities
            .get(&identity_key(role, index))
            .map(String::as_str)
    }
}

fn identity_key(role: &CompositionUidRole, index: u32) -> String {
    format!("{}#{index}", role.as_str())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdentityError {
    InvalidStandardsLockHash(String),
    InvalidLogicalInstanceId(String),
    InvalidRole(String),
    DuplicateRoleIndex {
        role: CompositionUidRole,
        index: u32,
    },
}

impl fmt::Display for IdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidStandardsLockHash(_) => {
                formatter.write_str("standards lock identity must be a lowercase SHA-256")
            }
            Self::InvalidLogicalInstanceId(value) => {
                write!(formatter, "invalid logical instance ID {value:?}")
            }
            Self::InvalidRole(value) => write!(formatter, "invalid UID role {value:?}"),
            Self::DuplicateRoleIndex { role, index } => {
                write!(
                    formatter,
                    "duplicate UID role {} index {index}",
                    role.as_str()
                )
            }
        }
    }
}

impl std::error::Error for IdentityError {}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    const LOCK_HASH: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    fn allocator(seed: u64) -> IdentityAllocator {
        IdentityAllocator::new(
            LOCK_HASH,
            TemplateId("classic/secondary-capture/monochrome".into()),
            "1.0.0".parse().unwrap(),
            seed,
        )
        .unwrap()
    }

    #[test]
    fn allocation_is_stable_and_valid() {
        let allocator = allocator(1);
        let first = allocator
            .allocate("primary", &CompositionUidRole::SopInstance, 0)
            .unwrap();
        assert_eq!(
            first,
            allocator
                .allocate("primary", &CompositionUidRole::SopInstance, 0)
                .unwrap()
        );
        assert!(is_valid_generated_uid(&first));
        assert!(first.len() <= 44);
    }

    #[test]
    fn every_contract_input_separates_identity() {
        let base = allocator(1)
            .allocate("primary", &CompositionUidRole::SopInstance, 0)
            .unwrap();
        let other_seed = allocator(2)
            .allocate("primary", &CompositionUidRole::SopInstance, 0)
            .unwrap();
        let other_template = IdentityAllocator::new(
            LOCK_HASH,
            TemplateId("classic/secondary-capture/rgb".into()),
            "1.0.0".parse().unwrap(),
            1,
        )
        .unwrap()
        .allocate("primary", &CompositionUidRole::SopInstance, 0)
        .unwrap();
        let other_version = IdentityAllocator::new(
            LOCK_HASH,
            TemplateId("classic/secondary-capture/monochrome".into()),
            "1.0.1".parse().unwrap(),
            1,
        )
        .unwrap()
        .allocate("primary", &CompositionUidRole::SopInstance, 0)
        .unwrap();
        let other_instance = allocator(1)
            .allocate("secondary", &CompositionUidRole::SopInstance, 0)
            .unwrap();
        let other_role = allocator(1)
            .allocate("primary", &CompositionUidRole::SeriesInstance, 0)
            .unwrap();
        let other_index = allocator(1)
            .allocate("primary", &CompositionUidRole::SopInstance, 1)
            .unwrap();
        let identities = BTreeSet::from([
            base,
            other_seed,
            other_template,
            other_version,
            other_instance,
            other_role,
            other_index,
        ]);
        assert_eq!(identities.len(), 7);
    }

    #[test]
    fn plan_rejects_duplicate_role_and_index() {
        assert!(matches!(
            allocator(1).allocate_plan(
                "primary",
                [
                    (CompositionUidRole::SeriesInstance, 0),
                    (CompositionUidRole::SeriesInstance, 0),
                ]
            ),
            Err(IdentityError::DuplicateRoleIndex { .. })
        ));
    }

    #[test]
    fn allocator_input_has_no_path_or_environment_channel() {
        let allocator = allocator(1);
        let plan = allocator
            .allocate_plan(
                "primary",
                [
                    (CompositionUidRole::StudyInstance, 0),
                    (CompositionUidRole::SeriesInstance, 0),
                    (CompositionUidRole::SopInstance, 0),
                ],
            )
            .unwrap();
        let json = serde_json::to_string(&plan).unwrap();
        assert!(!json.contains('/') && !json.contains("Users") && !json.contains("tmp"));
    }
}
