use std::collections::BTreeSet;

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

    if expected.get("specific_character_sets").is_some() {
        validate_character_sets(relative_path, bytes, expected, obj, failures);
    }
    validate_person_names(relative_path, bytes, expected, obj, failures);
    validate_temporal_metadata(relative_path, bytes, expected, obj, failures);
    validate_empty_type2_attributes(relative_path, bytes, expected, obj, failures);
    validate_string_elements(relative_path, bytes, expected, obj, failures);
}

pub(crate) fn validate_manifest_metadata_corpus(files: &[Value], failures: &mut Vec<String>) {
    let temporal_files = files
        .iter()
        .filter(|file| {
            file.get("case_id").and_then(Value::as_str) == Some("metadata/sc/timezone_boundaries")
        })
        .collect::<Vec<_>>();
    if temporal_files.is_empty() {
        return;
    }

    let boundary_ids = temporal_files
        .iter()
        .filter_map(|file| file.pointer("/expected_metadata/temporal/boundary_id"))
        .filter_map(Value::as_str)
        .collect::<BTreeSet<_>>();
    let expected = BTreeSet::from(["negative_min", "positive_max"]);
    if temporal_files.len() != 2 || boundary_ids != expected {
        failures.push(format!(
            "metadata/sc/timezone_boundaries: metadata_temporal_boundary_set: expected exactly one negative_min and one positive_max instance, found {} files and {boundary_ids:?}",
            temporal_files.len()
        ));
    }
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

fn validate_empty_type2_attributes(
    relative_path: &str,
    bytes: &[u8],
    expected: &Value,
    obj: &OpenedObject,
    failures: &mut Vec<String>,
) {
    let Some(attributes) = expected
        .get("empty_type2_attributes")
        .and_then(Value::as_array)
    else {
        return;
    };

    let required = BTreeSet::from([
        ("0008,0050", "AccessionNumber", "SH"),
        ("0008,0090", "ReferringPhysicianName", "PN"),
        ("0010,0010", "PatientName", "PN"),
        ("0010,0030", "PatientBirthDate", "DA"),
        ("0010,0040", "PatientSex", "CS"),
    ]);
    let declared = attributes
        .iter()
        .map(|attribute| {
            (
                attribute.get("tag").and_then(Value::as_str).unwrap_or(""),
                attribute
                    .get("keyword")
                    .and_then(Value::as_str)
                    .unwrap_or(""),
                attribute.get("vr").and_then(Value::as_str).unwrap_or(""),
            )
        })
        .collect::<BTreeSet<_>>();
    if attributes.len() != required.len() || declared != required {
        failures.push(format!(
            "{relative_path}: metadata_empty_type2_attribute_set: manifest {declared:?}, expected {required:?}"
        ));
    }

    for attribute in attributes {
        let tag_text = attribute.get("tag").and_then(Value::as_str).unwrap_or("");
        let keyword = attribute
            .get("keyword")
            .and_then(Value::as_str)
            .unwrap_or(tag_text);
        let expected_vr = attribute.get("vr").and_then(Value::as_str).unwrap_or("");
        if attribute.get("value_length").and_then(Value::as_u64) != Some(0) {
            failures.push(format!(
                "{relative_path}: metadata_empty_type2_manifest_value_length: {keyword} must declare value_length 0"
            ));
        }
        let Some(tag) = parse_tag(tag_text) else {
            failures.push(format!(
                "{relative_path}: metadata_empty_type2_tag: invalid manifest tag {tag_text}"
            ));
            continue;
        };

        match obj.element(tag) {
            Ok(element) => {
                if format!("{:?}", element.vr()) != expected_vr {
                    failures.push(format!(
                        "{relative_path}: metadata_empty_type2_vr: {keyword} dataset {:?}, manifest {expected_vr}",
                        element.vr()
                    ));
                }
                match element.to_bytes() {
                    Ok(value) if value.is_empty() => {}
                    Ok(value) => failures.push(format!(
                        "{relative_path}: metadata_empty_type2_decoded_value: {keyword} decoded length {}, expected 0",
                        value.len()
                    )),
                    Err(error) => failures.push(format!(
                        "{relative_path}: metadata_empty_type2_decoded_value: {keyword} is unreadable: {error}"
                    )),
                }
            }
            Err(_) => failures.push(format!(
                "{relative_path}: metadata_empty_type2_presence: {keyword} is missing"
            )),
        }

        match find_raw_element(bytes, tag) {
            Some(raw) => {
                if raw.vr != expected_vr {
                    failures.push(format!(
                        "{relative_path}: metadata_empty_type2_raw_vr: {keyword} dataset {}, manifest {expected_vr}",
                        raw.vr
                    ));
                }
                if !raw.value.is_empty() {
                    failures.push(format!(
                        "{relative_path}: metadata_empty_type2_raw_value_length: {keyword} encoded length {}, expected 0",
                        raw.value.len()
                    ));
                }
            }
            None => failures.push(format!(
                "{relative_path}: metadata_empty_type2_raw_presence: raw {keyword} element is missing"
            )),
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct StringBoundarySpec {
    tag: &'static str,
    keyword: &'static str,
    vr: &'static str,
    value_multiplicity: usize,
    decoded_value_lengths: &'static [usize],
    raw_value_byte_length: usize,
    raw_value_sha256: &'static str,
    padding: &'static str,
}

const STRING_BOUNDARY_SPECS: &[StringBoundarySpec] = &[
    StringBoundarySpec {
        tag: "0020,4000",
        keyword: "ImageComments",
        vr: "LT",
        value_multiplicity: 1,
        decoded_value_lengths: &[10_240],
        raw_value_byte_length: 10_240,
        raw_value_sha256: "75497849c172d88a38e271cc6ce82f31adbba1f16b6191d8ddaeb4e9f6268e52",
        padding: "none",
    },
    StringBoundarySpec {
        tag: "0018,1020",
        keyword: "SoftwareVersions",
        vr: "LO",
        value_multiplicity: 2,
        decoded_value_lengths: &[64, 64],
        raw_value_byte_length: 130,
        raw_value_sha256: "e79f64c5853732dd713d14c3530ef494d800f684653fc5bf0aced3933241a260",
        padding: "space",
    },
    StringBoundarySpec {
        tag: "0028,0030",
        keyword: "PixelSpacing",
        vr: "DS",
        value_multiplicity: 2,
        decoded_value_lengths: &[16, 16],
        raw_value_byte_length: 34,
        raw_value_sha256: "e09885a80758e44eaa4b9b544e7301c852395d3ee14ed7b7588e62a5f3b2db6a",
        padding: "space",
    },
    StringBoundarySpec {
        tag: "0020,0012",
        keyword: "AcquisitionNumber",
        vr: "IS",
        value_multiplicity: 1,
        decoded_value_lengths: &[12],
        raw_value_byte_length: 12,
        raw_value_sha256: "f9cf9c74b83f0c66cdb48d3536a5a5d884babc2cfda813d01b3577b473de20cf",
        padding: "none",
    },
];

fn validate_string_elements(
    relative_path: &str,
    bytes: &[u8],
    expected: &Value,
    obj: &OpenedObject,
    failures: &mut Vec<String>,
) {
    let Some(elements) = expected.get("string_elements").and_then(Value::as_array) else {
        return;
    };
    let declared = elements
        .iter()
        .map(|element| {
            (
                element.get("tag").and_then(Value::as_str).unwrap_or(""),
                element.get("keyword").and_then(Value::as_str).unwrap_or(""),
                element.get("vr").and_then(Value::as_str).unwrap_or(""),
            )
        })
        .collect::<BTreeSet<_>>();
    let required = STRING_BOUNDARY_SPECS
        .iter()
        .map(|spec| (spec.tag, spec.keyword, spec.vr))
        .collect::<BTreeSet<_>>();
    if elements.len() != STRING_BOUNDARY_SPECS.len() || declared != required {
        failures.push(format!(
            "{relative_path}: metadata_string_element_set: manifest {declared:?}, expected {required:?}"
        ));
    }

    for expected_element in elements {
        let tag_text = expected_element
            .get("tag")
            .and_then(Value::as_str)
            .unwrap_or("");
        let Some(spec) = STRING_BOUNDARY_SPECS
            .iter()
            .find(|spec| spec.tag == tag_text)
        else {
            continue;
        };
        let Some(tag) = parse_tag(tag_text) else {
            continue;
        };
        let values = expected_element
            .get("decoded_values")
            .and_then(Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let lengths = values.iter().map(|value| value.len()).collect::<Vec<_>>();
        let manifest_lengths = expected_element
            .get("decoded_value_lengths")
            .and_then(Value::as_array)
            .map(|lengths| {
                lengths
                    .iter()
                    .filter_map(Value::as_u64)
                    .map(|length| length as usize)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let manifest_vm = expected_element
            .get("value_multiplicity")
            .and_then(Value::as_u64)
            .unwrap_or_default() as usize;
        let manifest_raw_length = expected_element
            .get("raw_value_byte_length")
            .and_then(Value::as_u64)
            .unwrap_or_default() as usize;
        let manifest_hash = expected_element
            .get("raw_value_sha256")
            .and_then(Value::as_str)
            .unwrap_or("");
        let manifest_padding = expected_element
            .get("padding")
            .and_then(Value::as_str)
            .unwrap_or("");
        if manifest_vm != values.len()
            || manifest_vm != spec.value_multiplicity
            || lengths != manifest_lengths
            || lengths != spec.decoded_value_lengths
        {
            failures.push(format!(
                "{relative_path}: metadata_string_vm_lengths: {} values {} lengths {lengths:?}, manifest VM {manifest_vm} lengths {manifest_lengths:?}, required VM {} lengths {:?}",
                spec.keyword,
                values.len(),
                spec.value_multiplicity,
                spec.decoded_value_lengths
            ));
        }
        if manifest_raw_length != spec.raw_value_byte_length
            || manifest_hash != spec.raw_value_sha256
            || manifest_padding != spec.padding
        {
            failures.push(format!(
                "{relative_path}: metadata_string_raw_contract: {} manifest VL {manifest_raw_length} hash {manifest_hash} padding {manifest_padding}, required VL {} hash {} padding {}",
                spec.keyword,
                spec.raw_value_byte_length,
                spec.raw_value_sha256,
                spec.padding
            ));
        }

        match obj.element(tag) {
            Ok(element) => {
                if format!("{:?}", element.vr()) != spec.vr {
                    failures.push(format!(
                        "{relative_path}: metadata_string_vr: {} dataset {:?}, expected {}",
                        spec.keyword,
                        element.vr(),
                        spec.vr
                    ));
                }
                let actual = element
                    .to_multi_str()
                    .map(|values| values.iter().map(ToString::to_string).collect::<Vec<_>>());
                match actual {
                    Ok(actual) if actual == values => {}
                    Ok(actual) => failures.push(format!(
                        "{relative_path}: metadata_string_decoded_values: {} dataset VM {} lengths {:?}, manifest VM {} lengths {:?}",
                        spec.keyword,
                        actual.len(),
                        actual.iter().map(|value| value.len()).collect::<Vec<_>>(),
                        values.len(),
                        lengths
                    )),
                    Err(error) => failures.push(format!(
                        "{relative_path}: metadata_string_decoded_values: {} is unreadable: {error}",
                        spec.keyword
                    )),
                }
            }
            Err(_) => failures.push(format!(
                "{relative_path}: metadata_string_presence: {} is missing",
                spec.keyword
            )),
        }

        match find_raw_element(bytes, tag) {
            Some(raw) => {
                if raw.vr != spec.vr {
                    failures.push(format!(
                        "{relative_path}: metadata_string_raw_vr: {} dataset {}, expected {}",
                        spec.keyword, raw.vr, spec.vr
                    ));
                }
                if raw.value.len() != manifest_raw_length {
                    failures.push(format!(
                        "{relative_path}: metadata_string_raw_length: {} dataset {}, manifest {manifest_raw_length}",
                        spec.keyword,
                        raw.value.len()
                    ));
                }
                if sha256_hex(raw.value) != manifest_hash {
                    failures.push(format!(
                        "{relative_path}: metadata_string_raw_hash: {} dataset {}, manifest {manifest_hash}",
                        spec.keyword,
                        sha256_hex(raw.value)
                    ));
                }
                let padding_matches = match manifest_padding {
                    "space" => raw.value.last() == Some(&b' '),
                    "none" => raw.value.last() != Some(&b' '),
                    _ => false,
                };
                if !padding_matches {
                    failures.push(format!(
                        "{relative_path}: metadata_string_padding: {} raw ending does not match {manifest_padding}",
                        spec.keyword
                    ));
                }
            }
            None => failures.push(format!(
                "{relative_path}: metadata_string_raw_presence: raw {} element is missing",
                spec.keyword
            )),
        }
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

fn validate_temporal_metadata(
    relative_path: &str,
    bytes: &[u8],
    expected: &Value,
    obj: &OpenedObject,
    failures: &mut Vec<String>,
) {
    let Some(temporal) = expected.get("temporal") else {
        return;
    };

    let offset_value = temporal
        .get("timezone_offset_from_utc")
        .unwrap_or(&Value::Null);
    validate_encoded_temporal_element(relative_path, bytes, offset_value, obj, failures);
    let offset_text = offset_value
        .get("decoded_value")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let offset_minutes = match parse_timezone_offset(offset_text) {
        Ok(value) => value,
        Err(message) => {
            failures.push(format!(
                "{relative_path}: metadata_temporal_timezone_offset: {message}"
            ));
            0
        }
    };
    let manifest_offset = offset_value
        .get("offset_minutes")
        .and_then(Value::as_i64)
        .unwrap_or_default();
    if i64::from(offset_minutes) != manifest_offset {
        failures.push(format!(
            "{relative_path}: metadata_temporal_timezone_offset_minutes: parsed {offset_minutes}, manifest {manifest_offset}"
        ));
    }

    let mut study_date = None;
    for value in temporal
        .get("date_values")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        validate_encoded_temporal_element(relative_path, bytes, value, obj, failures);
        let decoded = value
            .get("decoded_value")
            .and_then(Value::as_str)
            .unwrap_or_default();
        match parse_dicom_date(decoded) {
            Ok(date) => {
                if value.get("keyword").and_then(Value::as_str) == Some("StudyDate") {
                    study_date = Some(date);
                }
            }
            Err(message) => failures.push(format!(
                "{relative_path}: metadata_temporal_date_lexical: {message}"
            )),
        }
    }

    let mut study_time = None;
    for value in temporal
        .get("time_values")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        validate_encoded_temporal_element(relative_path, bytes, value, obj, failures);
        let decoded = value
            .get("decoded_value")
            .and_then(Value::as_str)
            .unwrap_or_default();
        match parse_dicom_time(decoded) {
            Ok(time) => {
                if value.get("keyword").and_then(Value::as_str) == Some("StudyTime") {
                    study_time = Some(time);
                }
            }
            Err(message) => failures.push(format!(
                "{relative_path}: metadata_temporal_time_lexical: {message}"
            )),
        }
    }

    for value in temporal
        .get("date_time_values")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        validate_encoded_temporal_element(relative_path, bytes, value, obj, failures);
        let decoded = value
            .get("decoded_value")
            .and_then(Value::as_str)
            .unwrap_or_default();
        match parse_dicom_date_time(decoded) {
            Ok((date, time, embedded_offset)) => {
                let manifest_embedded = value
                    .get("embedded_offset_minutes")
                    .and_then(Value::as_i64)
                    .unwrap_or_default();
                if embedded_offset != offset_minutes
                    || i64::from(embedded_offset) != manifest_embedded
                {
                    failures.push(format!(
                        "{relative_path}: metadata_temporal_embedded_offset: DT {embedded_offset}, global {offset_minutes}, manifest {manifest_embedded}"
                    ));
                }
                let normalized = normalize_utc(date, time, embedded_offset);
                let manifest_normalized = value
                    .get("normalized_utc")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                if normalized != manifest_normalized {
                    failures.push(format!(
                        "{relative_path}: metadata_temporal_date_time_utc: computed {normalized}, manifest {manifest_normalized}"
                    ));
                }
            }
            Err(message) => failures.push(format!(
                "{relative_path}: metadata_temporal_date_time_lexical: {message}"
            )),
        }
    }

    if let (Some(date), Some(time)) = (study_date, study_time) {
        let normalized = normalize_utc(date, time, offset_minutes);
        let manifest_normalized = temporal
            .get("combined_da_tm_utc")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if normalized != manifest_normalized {
            failures.push(format!(
                "{relative_path}: metadata_temporal_combined_utc: computed {normalized}, manifest {manifest_normalized}"
            ));
        }
    } else {
        failures.push(format!(
            "{relative_path}: metadata_temporal_combined_source: StudyDate and StudyTime expectations are required"
        ));
    }
}

fn validate_encoded_temporal_element(
    relative_path: &str,
    bytes: &[u8],
    expected: &Value,
    obj: &OpenedObject,
    failures: &mut Vec<String>,
) {
    let tag_text = expected
        .get("tag")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let Some(tag) = parse_tag(tag_text) else {
        failures.push(format!(
            "{relative_path}: metadata_temporal_tag: invalid manifest tag {tag_text:?}"
        ));
        return;
    };
    let keyword = expected
        .get("keyword")
        .and_then(Value::as_str)
        .unwrap_or(tag_text);
    let expected_vr = expected
        .get("vr")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let expected_value = expected
        .get("decoded_value")
        .and_then(Value::as_str)
        .unwrap_or_default();

    match obj.element(tag) {
        Ok(element) => {
            if format!("{:?}", element.vr()) != expected_vr {
                failures.push(format!(
                    "{relative_path}: metadata_temporal_vr: {keyword} dataset {:?}, manifest {expected_vr}",
                    element.vr()
                ));
            }
            match element.to_str() {
                Ok(actual) if actual.trim_end_matches([' ', '\0']) == expected_value => {}
                Ok(actual) => failures.push(format!(
                    "{relative_path}: metadata_temporal_decoded: {keyword} dataset {:?}, manifest {expected_value:?}",
                    actual.trim_end_matches([' ', '\0'])
                )),
                Err(error) => failures.push(format!(
                    "{relative_path}: metadata_temporal_decoded: {keyword} is unreadable: {error}"
                )),
            }
        }
        Err(error) => failures.push(format!(
            "{relative_path}: metadata_temporal_present: {keyword} is missing: {error}"
        )),
    }

    match find_raw_element(bytes, tag) {
        Some(element) => {
            if element.vr != expected_vr {
                failures.push(format!(
                    "{relative_path}: metadata_temporal_raw_vr: {keyword} dataset {}, manifest {expected_vr}",
                    element.vr
                ));
            }
            let expected_length = expected
                .get("raw_value_byte_length")
                .and_then(Value::as_u64)
                .unwrap_or_default() as usize;
            if element.value.len() != expected_length {
                failures.push(format!(
                    "{relative_path}: metadata_temporal_raw_length: {keyword} dataset {}, manifest {expected_length}",
                    element.value.len()
                ));
            }
            let expected_hash = expected
                .get("raw_value_sha256")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let actual_hash = sha256_hex(element.value);
            if actual_hash != expected_hash {
                failures.push(format!(
                    "{relative_path}: metadata_temporal_raw_hash: {keyword} dataset {actual_hash}, manifest {expected_hash}"
                ));
            }
            let expected_hex = expected
                .get("raw_value_hex")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let actual_hex = uppercase_hex(element.value);
            if actual_hex != expected_hex {
                failures.push(format!(
                    "{relative_path}: metadata_temporal_raw_hex: {keyword} dataset {actual_hex}, manifest {expected_hex}"
                ));
            }
        }
        None => failures.push(format!(
            "{relative_path}: metadata_temporal_raw: {keyword} raw element is missing"
        )),
    }
}

#[derive(Debug, Clone, Copy)]
struct DicomDate {
    year: i64,
    month: i64,
    day: i64,
}

#[derive(Debug, Clone, Copy)]
struct DicomTime {
    hour: i64,
    minute: i64,
    second: i64,
    micros: i64,
}

fn parse_dicom_date(value: &str) -> Result<DicomDate, String> {
    if value.len() != 8 || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(format!("DA {value:?} must contain exactly YYYYMMDD"));
    }
    let year = parse_digits(&value[0..4])?;
    let month = parse_digits(&value[4..6])?;
    let day = parse_digits(&value[6..8])?;
    if year == 0 || !(1..=12).contains(&month) {
        return Err(format!("DA {value:?} has an invalid year or month"));
    }
    let max_day = days_in_month(year, month);
    if !(1..=max_day).contains(&day) {
        return Err(format!("DA {value:?} has an invalid day"));
    }
    Ok(DicomDate { year, month, day })
}

fn parse_dicom_time(value: &str) -> Result<DicomTime, String> {
    let (base, fraction) = value
        .split_once('.')
        .ok_or_else(|| format!("TM {value:?} must include fractional seconds"))?;
    if base.len() != 6
        || !base.bytes().all(|byte| byte.is_ascii_digit())
        || fraction.is_empty()
        || fraction.len() > 6
        || !fraction.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(format!(
            "TM {value:?} must contain HHMMSS and one through six fractional digits"
        ));
    }
    let hour = parse_digits(&base[0..2])?;
    let minute = parse_digits(&base[2..4])?;
    let second = parse_digits(&base[4..6])?;
    if hour > 23 || minute > 59 || second > 60 {
        return Err(format!("TM {value:?} is outside the 24-hour range"));
    }
    let micros = parse_digits(fraction)? * 10_i64.pow((6 - fraction.len()) as u32);
    Ok(DicomTime {
        hour,
        minute,
        second,
        micros,
    })
}

fn parse_dicom_date_time(value: &str) -> Result<(DicomDate, DicomTime, i16), String> {
    if value.len() != 26 {
        return Err(format!(
            "DT {value:?} must contain YYYYMMDDHHMMSS.ffffff&ZZXX"
        ));
    }
    let date = parse_dicom_date(&value[0..8])?;
    let time = parse_dicom_time(&value[8..21])?;
    let offset = parse_timezone_offset(&value[21..26])?;
    Ok((date, time, offset))
}

fn parse_timezone_offset(value: &str) -> Result<i16, String> {
    let bytes = value.as_bytes();
    if bytes.len() != 5
        || !matches!(bytes[0], b'+' | b'-')
        || !bytes[1..].iter().all(u8::is_ascii_digit)
    {
        return Err(format!("timezone offset {value:?} must use [+-]HHMM"));
    }
    let hours = parse_digits(&value[1..3])?;
    let minutes = parse_digits(&value[3..5])?;
    if minutes > 59 {
        return Err(format!("timezone offset {value:?} has invalid minutes"));
    }
    let magnitude = hours * 60 + minutes;
    let signed = if bytes[0] == b'-' {
        -magnitude
    } else {
        magnitude
    };
    if !(-720..=840).contains(&signed) {
        return Err(format!(
            "timezone offset {value:?} is outside -1200 through +1400"
        ));
    }
    Ok(signed as i16)
}

fn parse_digits(value: &str) -> Result<i64, String> {
    value
        .parse::<i64>()
        .map_err(|_| format!("{value:?} is not numeric"))
}

fn days_in_month(year: i64, month: i64) -> i64 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) => 29,
        2 => 28,
        _ => 0,
    }
}

fn normalize_utc(date: DicomDate, time: DicomTime, offset_minutes: i16) -> String {
    let local_seconds = days_from_civil(date.year, date.month, date.day) * 86_400
        + time.hour * 3_600
        + time.minute * 60
        + time.second;
    let utc_seconds = local_seconds - i64::from(offset_minutes) * 60;
    let utc_days = utc_seconds.div_euclid(86_400);
    let day_seconds = utc_seconds.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(utc_days);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{:06}Z",
        day_seconds / 3_600,
        day_seconds % 3_600 / 60,
        day_seconds % 60,
        time.micros
    )
}

fn days_from_civil(mut year: i64, month: i64, day: i64) -> i64 {
    year -= i64::from(month <= 2);
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let year_of_era = year - era * 400;
    let month_prime = month + if month > 2 { -3 } else { 9 };
    let day_of_year = (153 * month_prime + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

fn civil_from_days(days_since_epoch: i64) -> (i64, i64, i64) {
    let z = days_since_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    (year, month, day)
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
    use super::{
        normalize_utc, parse_dicom_date, parse_dicom_date_time, parse_dicom_time, parse_tag,
        parse_timezone_offset,
    };
    use dicom_core::Tag;

    #[test]
    fn parses_canonical_manifest_tag() {
        assert_eq!(parse_tag("0010,0010"), Some(Tag(0x0010, 0x0010)));
        assert_eq!(parse_tag("PatientName"), None);
    }

    #[test]
    fn parses_and_normalizes_timezone_extrema() {
        let positive_date = parse_dicom_date("20240229").expect("leap day must parse");
        let positive_time = parse_dicom_time("235959.999999").expect("max local time must parse");
        assert_eq!(parse_timezone_offset("+1400"), Ok(840));
        assert_eq!(
            normalize_utc(positive_date, positive_time, 840),
            "2024-02-29T09:59:59.999999Z"
        );

        let (date, time, offset) = parse_dicom_date_time("20240301000000.000000-1200")
            .expect("negative boundary DT must parse");
        assert_eq!(offset, -720);
        assert_eq!(
            normalize_utc(date, time, offset),
            "2024-03-01T12:00:00.000000Z"
        );
    }

    #[test]
    fn rejects_invalid_temporal_lexemes() {
        assert!(parse_dicom_date("20230229").is_err());
        assert!(parse_dicom_time("240000.000000").is_err());
        assert!(parse_timezone_offset("+1401").is_err());
        assert!(parse_timezone_offset("-1201").is_err());
        assert!(parse_dicom_date_time("20240229235959+1400").is_err());
    }
}
