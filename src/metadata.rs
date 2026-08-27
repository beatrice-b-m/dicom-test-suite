use dicom_core::{Tag, VR};
use dicom_dictionary_std::{StandardDataDictionary, tags};
use dicom_object::{FileDicomObject, InMemDicomObject};
use serde_json::Value;

use crate::sha256_hex;

type OpenedObject = FileDicomObject<InMemDicomObject<StandardDataDictionary>>;

#[derive(Debug, Clone, Copy)]
struct RawElement<'a> {
    vr: &'a str,
    value: &'a [u8],
}

pub(crate) fn validate_manifest_metadata(
    relative_path: &str,
    bytes: &[u8],
    transfer_syntax_uid: &str,
    file: &Value,
    obj: &OpenedObject,
    failures: &mut Vec<String>,
) {
    let Some(expected) = file
        .get("expected_metadata")
        .filter(|value| !value.is_null())
    else {
        return;
    };

    if transfer_syntax_uid != dicom_dictionary_std::uids::EXPLICIT_VR_LITTLE_ENDIAN {
        failures.push(format!(
            "{relative_path}: metadata_raw_encoding: expected metadata validation currently requires Explicit VR Little Endian"
        ));
        return;
    }

    validate_character_sets(relative_path, bytes, expected, obj, failures);
    validate_person_names(relative_path, bytes, expected, obj, failures);
}

fn validate_character_sets(
    relative_path: &str,
    bytes: &[u8],
    expected: &Value,
    obj: &OpenedObject,
    failures: &mut Vec<String>,
) {
    let expected_values = expected
        .get("specific_character_sets")
        .and_then(Value::as_array)
        .map(|values| values.iter().filter_map(Value::as_str).collect::<Vec<_>>())
        .unwrap_or_default();

    let decoded_values = obj
        .element(tags::SPECIFIC_CHARACTER_SET)
        .ok()
        .and_then(|element| element.to_multi_str().ok())
        .map(|values| {
            values
                .iter()
                .map(|value| value.trim().to_string())
                .collect::<Vec<_>>()
        });
    match decoded_values {
        Some(actual) if actual == expected_values => {}
        Some(actual) => failures.push(format!(
            "{relative_path}: metadata_specific_character_sets: dataset {actual:?}, manifest {expected_values:?}"
        )),
        None => failures.push(format!(
            "{relative_path}: metadata_specific_character_sets: Specific Character Set is missing or unreadable"
        )),
    }

    match find_raw_element(bytes, Tag(0x0008, 0x0005)) {
        Some(element) => {
            if element.vr != "CS" {
                failures.push(format!(
                    "{relative_path}: metadata_specific_character_set_vr: dataset {}, expected CS",
                    element.vr
                ));
            }
            let raw = String::from_utf8_lossy(element.value)
                .trim_end_matches([' ', '\0'])
                .split('\\')
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>();
            if raw != expected_values {
                failures.push(format!(
                    "{relative_path}: metadata_specific_character_sets_raw: dataset {raw:?}, manifest {expected_values:?}"
                ));
            }
        }
        None => failures.push(format!(
            "{relative_path}: metadata_specific_character_sets_raw: raw Specific Character Set element is missing"
        )),
    }
}

fn validate_person_names(
    relative_path: &str,
    bytes: &[u8],
    expected: &Value,
    obj: &OpenedObject,
    failures: &mut Vec<String>,
) {
    let Some(person_names) = expected.get("person_names").and_then(Value::as_array) else {
        return;
    };

    for person_name in person_names {
        let Some(tag_text) = person_name.get("tag").and_then(Value::as_str) else {
            continue;
        };
        let Some(tag) = parse_tag(tag_text) else {
            failures.push(format!(
                "{relative_path}: metadata_person_name_tag: invalid manifest tag {tag_text}"
            ));
            continue;
        };
        let expected_value = person_name
            .get("decoded_value")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let keyword = person_name
            .get("keyword")
            .and_then(Value::as_str)
            .unwrap_or(tag_text);

        let manifest_requires_independent_decode = expected
            .get("specific_character_sets")
            .and_then(Value::as_array)
            .is_some_and(|values| {
                values
                    .iter()
                    .filter_map(Value::as_str)
                    .any(|value| value.starts_with("ISO 2022 ") && value != "ISO 2022 IR 6")
            });
        let dataset_requires_independent_decode = find_raw_element(bytes, Tag(0x0008, 0x0005))
            .is_some_and(|element| {
                String::from_utf8_lossy(element.value)
                    .trim_end_matches([' ', '\0'])
                    .split('\\')
                    .any(|value| value.starts_with("ISO 2022 ") && value != "ISO 2022 IR 6")
            });
        let requires_independent_decode =
            manifest_requires_independent_decode && dataset_requires_independent_decode;

        match obj.element(tag) {
            Ok(element) => {
                if element.vr() != VR::PN {
                    failures.push(format!(
                        "{relative_path}: metadata_person_name_vr: {keyword} dataset {:?}, expected PN",
                        element.vr()
                    ));
                }
                if !requires_independent_decode {
                    match element.to_str() {
                        Ok(actual) if actual.trim_end_matches([' ', '\0']) == expected_value => {}
                        Ok(actual) => failures.push(format!(
                            "{relative_path}: metadata_person_name_decoded: {keyword} dataset {:?}, manifest {:?}",
                            actual.trim_end_matches([' ', '\0']),
                            expected_value
                        )),
                        Err(err) => failures.push(format!(
                            "{relative_path}: metadata_person_name_decoded: {keyword} is unreadable: {err}"
                        )),
                    }
                }
            }
            Err(err) => failures.push(format!(
                "{relative_path}: metadata_person_name_present: {keyword} is missing: {err}"
            )),
        }

        validate_person_name_groups(
            relative_path,
            keyword,
            expected_value,
            person_name,
            failures,
        );

        match find_raw_element(bytes, tag) {
            Some(element) => {
                if element.vr != "PN" {
                    failures.push(format!(
                        "{relative_path}: metadata_person_name_raw_vr: {keyword} dataset {}, expected PN",
                        element.vr
                    ));
                }
                let expected_length = person_name
                    .get("raw_value_byte_length")
                    .and_then(Value::as_u64)
                    .unwrap_or_default() as usize;
                if element.value.len() != expected_length {
                    failures.push(format!(
                        "{relative_path}: metadata_person_name_raw_length: {keyword} dataset {}, manifest {expected_length}",
                        element.value.len()
                    ));
                }
                let expected_hash = person_name
                    .get("raw_value_sha256")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let actual_hash = sha256_hex(element.value);
                if actual_hash != expected_hash {
                    failures.push(format!(
                        "{relative_path}: metadata_person_name_raw_hash: {keyword} dataset {actual_hash}, manifest {expected_hash}"
                    ));
                }
                let expected_hex = person_name
                    .get("raw_value_hex")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let actual_hex = uppercase_hex(element.value);
                if actual_hex != expected_hex {
                    failures.push(format!(
                        "{relative_path}: metadata_person_name_raw_hex: {keyword} dataset {actual_hex}, manifest {expected_hex}"
                    ));
                }
            }
            None => failures.push(format!(
                "{relative_path}: metadata_person_name_raw: {keyword} raw element is missing"
            )),
        }
    }
}

fn uppercase_hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;

    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(&mut encoded, "{byte:02X}").expect("writing to a String cannot fail");
    }
    encoded
}

fn validate_person_name_groups(
    relative_path: &str,
    keyword: &str,
    decoded_value: &str,
    expected: &Value,
    failures: &mut Vec<String>,
) {
    let actual_groups = decoded_value.split('=').collect::<Vec<_>>();
    let expected_groups = expected
        .get("component_groups")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default();
    if actual_groups.len() != expected_groups.len() {
        failures.push(format!(
            "{relative_path}: metadata_person_name_component_group_count: {keyword} dataset {}, manifest {}",
            actual_groups.len(),
            expected_groups.len()
        ));
    }

    for (index, expected_group) in expected_groups.iter().enumerate() {
        let expected_position = expected_group
            .get("position")
            .and_then(Value::as_u64)
            .unwrap_or_default() as usize;
        if expected_position != index + 1 {
            failures.push(format!(
                "{relative_path}: metadata_person_name_component_group_position: {keyword} manifest position {expected_position}, expected {}",
                index + 1
            ));
        }
        let actual_group = actual_groups.get(index).copied().unwrap_or_default();
        let expected_group_value = expected_group
            .get("decoded_value")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if actual_group != expected_group_value {
            failures.push(format!(
                "{relative_path}: metadata_person_name_component_group: {keyword} group {} dataset {actual_group:?}, manifest {expected_group_value:?}",
                index + 1
            ));
        }

        let mut actual_components = actual_group.split('^').collect::<Vec<_>>();
        actual_components.resize(5, "");
        let expected_components = expected_group
            .get("components")
            .and_then(Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or_default();
        if actual_components.len() != expected_components.len() {
            failures.push(format!(
                "{relative_path}: metadata_person_name_component_count: {keyword} group {} dataset {}, manifest {}",
                index + 1,
                actual_components.len(),
                expected_components.len()
            ));
        }
        for (component_index, expected_component) in expected_components.iter().enumerate() {
            let expected_position = expected_component
                .get("position")
                .and_then(Value::as_u64)
                .unwrap_or_default() as usize;
            let expected_value = expected_component
                .get("decoded_value")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let actual_value = actual_components
                .get(component_index)
                .copied()
                .unwrap_or_default();
            if expected_position != component_index + 1 || expected_value != actual_value {
                failures.push(format!(
                    "{relative_path}: metadata_person_name_component: {keyword} group {} component {} dataset {actual_value:?}, manifest position {expected_position} value {expected_value:?}",
                    index + 1,
                    component_index + 1
                ));
            }
        }
    }
}

fn parse_tag(value: &str) -> Option<Tag> {
    let (group, element) = value.split_once(',')?;
    Some(Tag(
        u16::from_str_radix(group, 16).ok()?,
        u16::from_str_radix(element, 16).ok()?,
    ))
}

fn find_raw_element(bytes: &[u8], wanted: Tag) -> Option<RawElement<'_>> {
    let mut offset = dataset_start(bytes)?;
    while offset + 8 <= bytes.len() {
        let group = read_u16(bytes, offset)?;
        let element = read_u16(bytes, offset + 2)?;
        let vr = std::str::from_utf8(bytes.get(offset + 4..offset + 6)?).ok()?;
        let long_vr = matches!(
            vr,
            "OB" | "OD" | "OF" | "OL" | "OV" | "OW" | "SQ" | "UC" | "UR" | "UT" | "UN"
        );
        let (length, value_offset) = if long_vr {
            (read_u32(bytes, offset + 8)? as usize, offset + 12)
        } else {
            (read_u16(bytes, offset + 6)? as usize, offset + 8)
        };
        if length == 0xFFFF_FFFF {
            return None;
        }
        let value = bytes.get(value_offset..value_offset.checked_add(length)?)?;
        if Tag(group, element) == wanted {
            return Some(RawElement { vr, value });
        }
        offset = value_offset + length;
    }
    None
}

fn dataset_start(bytes: &[u8]) -> Option<usize> {
    if bytes.get(128..132)? != b"DICM" {
        return None;
    }
    let mut offset = 132;
    while read_u16(bytes, offset)? == 0x0002 {
        let vr = std::str::from_utf8(bytes.get(offset + 4..offset + 6)?).ok()?;
        let long_vr = matches!(
            vr,
            "OB" | "OD" | "OF" | "OL" | "OV" | "OW" | "SQ" | "UC" | "UR" | "UT" | "UN"
        );
        let (length, value_offset) = if long_vr {
            (read_u32(bytes, offset + 8)? as usize, offset + 12)
        } else {
            (read_u16(bytes, offset + 6)? as usize, offset + 8)
        };
        offset = value_offset.checked_add(length)?;
    }
    Some(offset)
}

fn read_u16(bytes: &[u8], offset: usize) -> Option<u16> {
    Some(u16::from_le_bytes(
        bytes.get(offset..offset + 2)?.try_into().ok()?,
    ))
}

fn read_u32(bytes: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        bytes.get(offset..offset + 4)?.try_into().ok()?,
    ))
}

#[cfg(test)]
mod tests {
    use super::parse_tag;
    use dicom_core::Tag;

    #[test]
    fn parses_canonical_manifest_tag() {
        assert_eq!(parse_tag("0010,0010"), Some(Tag(0x0010, 0x0010)));
        assert_eq!(parse_tag("PatientName"), None);
    }
}
