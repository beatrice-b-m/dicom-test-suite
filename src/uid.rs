use crate::sha256_hex;

pub const PROJECT_NAMESPACE_UUID: &str = "4f5b3b66-8b91-4f3d-a6a1-6d9a7fc6d4d8";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UidRole {
    StudyInstance,
    SeriesInstance,
    SopInstance,
    FrameOfReference,
    ImplementationClass,
    DerivedReference,
}

impl UidRole {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::StudyInstance => "study_instance_uid",
            Self::SeriesInstance => "series_instance_uid",
            Self::SopInstance => "sop_instance_uid",
            Self::FrameOfReference => "frame_of_reference_uid",
            Self::ImplementationClass => "implementation_class_uid",
            Self::DerivedReference => "derived_reference_sop_instance_uid",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeterministicUidInput<'a> {
    pub standards_lock_sha256: &'a str,
    pub case_id: &'a str,
    pub recipe_version: &'a str,
    pub run_seed: u64,
    pub file_index: u32,
    pub frame_index: Option<u32>,
    pub referenced_object_index: Option<u32>,
    pub role: UidRole,
}

pub fn deterministic_uid(input: &DeterministicUidInput<'_>) -> String {
    let seed_material = format!(
        "namespace={};standards_lock_sha256={};case_id={};recipe_version={};run_seed={};file_index={};frame_index={};referenced_object_index={};role={}",
        PROJECT_NAMESPACE_UUID,
        input.standards_lock_sha256,
        input.case_id,
        input.recipe_version,
        input.run_seed,
        input.file_index,
        option_index(input.frame_index),
        option_index(input.referenced_object_index),
        input.role.as_str()
    );
    let digest = sha256_hex(seed_material.as_bytes());
    let mut uuid_bytes = first_16_digest_bytes(&digest);

    uuid_bytes[6] = (uuid_bytes[6] & 0x0f) | 0x50;
    uuid_bytes[8] = (uuid_bytes[8] & 0x3f) | 0x80;

    let uuid_as_u128 = u128::from_be_bytes(uuid_bytes);
    format!("2.25.{uuid_as_u128}")
}

pub fn is_valid_generated_uid(uid: &str) -> bool {
    let Some(decimal) = uid.strip_prefix("2.25.") else {
        return false;
    };
    uid.len() <= 64
        && !decimal.is_empty()
        && decimal.bytes().all(|byte| byte.is_ascii_digit())
        && (decimal == "0" || !decimal.starts_with('0'))
        && decimal.parse::<u128>().is_ok()
}

fn option_index(index: Option<u32>) -> String {
    index
        .map(|index| index.to_string())
        .unwrap_or_else(|| "none".to_string())
}

fn first_16_digest_bytes(hex_digest: &str) -> [u8; 16] {
    let mut bytes = [0u8; 16];
    for (index, byte) in bytes.iter_mut().enumerate() {
        let start = index * 2;
        *byte = u8::from_str_radix(&hex_digest[start..start + 2], 16)
            .expect("sha256_hex must return hexadecimal digits");
    }
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOCK_HASH: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    #[test]
    fn deterministic_uid_is_stable_for_same_inputs() {
        let input = sample_input(UidRole::SopInstance);

        assert_eq!(deterministic_uid(&input), deterministic_uid(&input));
    }

    #[test]
    fn deterministic_uid_uses_role_case_and_seed_inputs() {
        let sop_uid = deterministic_uid(&sample_input(UidRole::SopInstance));
        let series_uid = deterministic_uid(&sample_input(UidRole::SeriesInstance));

        let other_case_uid = deterministic_uid(&DeterministicUidInput {
            case_id: "classic/sc/mono1_u8_explicit_le",
            ..sample_input(UidRole::SopInstance)
        });
        let other_seed_uid = deterministic_uid(&DeterministicUidInput {
            run_seed: 2,
            ..sample_input(UidRole::SopInstance)
        });

        assert_ne!(sop_uid, series_uid);
        assert_ne!(sop_uid, other_case_uid);
        assert_ne!(sop_uid, other_seed_uid);
    }

    #[test]
    fn deterministic_uid_has_2_25_decimal_uuid_shape() {
        for role in [
            UidRole::StudyInstance,
            UidRole::SeriesInstance,
            UidRole::SopInstance,
            UidRole::FrameOfReference,
            UidRole::ImplementationClass,
            UidRole::DerivedReference,
        ] {
            let uid = deterministic_uid(&sample_input(role));

            assert!(
                is_valid_generated_uid(&uid),
                "{uid} must be a valid generated UID"
            );
            assert!(
                uid.len() <= 44,
                "{uid} should fit the maximum 2.25.<u128> decimal length"
            );
        }
    }

    #[test]
    fn deterministic_uid_uses_file_frame_and_reference_indexes() {
        let base = deterministic_uid(&sample_input(UidRole::DerivedReference));
        let file_uid = deterministic_uid(&DeterministicUidInput {
            file_index: 1,
            ..sample_input(UidRole::DerivedReference)
        });
        let frame_uid = deterministic_uid(&DeterministicUidInput {
            frame_index: Some(1),
            ..sample_input(UidRole::DerivedReference)
        });
        let reference_uid = deterministic_uid(&DeterministicUidInput {
            referenced_object_index: Some(1),
            ..sample_input(UidRole::DerivedReference)
        });

        assert_ne!(base, file_uid);
        assert_ne!(base, frame_uid);
        assert_ne!(base, reference_uid);
    }

    fn sample_input(role: UidRole) -> DeterministicUidInput<'static> {
        DeterministicUidInput {
            standards_lock_sha256: LOCK_HASH,
            case_id: "classic/sc/mono2_u8_explicit_le",
            recipe_version: "0.1.0",
            run_seed: 1,
            file_index: 0,
            frame_index: None,
            referenced_object_index: None,
            role,
        }
    }
}
