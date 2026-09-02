use std::collections::BTreeSet;
use std::fs;
use std::path::{Component, Path};

use dicom_core::header::HasLength;
use dicom_object::open_file;
use serde_json::{Value, json};

use crate::composition::{
    AttributeItem, AttributeOperation, AttributeValue, CanonicalContent, DicomVr, PrimitiveValue,
    ResolvedAttribute,
};

type Dataset = dicom_object::InMemDicomObject<dicom_dictionary_std::StandardDataDictionary>;

pub fn validate_assembly_root(root: &Path, manifest: &Value) -> (usize, Vec<String>) {
    let mut failures = Vec::new();
    if manifest
        .pointer("/run/iod_conformance")
        .and_then(Value::as_str)
        != Some("not_assessed")
    {
        failures.push("structural run must record iod_conformance=not_assessed".into());
    }
    for forbidden in ["case_id", "profile", "template_id", "qualification_status"] {
        if contains_key(manifest, forbidden) {
            failures.push(format!(
                "structural manifest contains forbidden claim {forbidden}"
            ));
        }
    }
    let Some(instances) = manifest.get("instances").and_then(Value::as_array) else {
        return (0, vec!["structural manifest is missing instances".into()]);
    };
    validate_reference_closure(instances, &mut failures);
    let mut paths = BTreeSet::new();
    for instance in instances {
        let Some(relative) = instance.get("output_path").and_then(Value::as_str) else {
            failures.push("structural instance output_path is missing".into());
            continue;
        };
        if !safe_relative_path(relative) || !paths.insert(relative.to_owned()) {
            failures.push(format!("unsafe or duplicate structural path {relative}"));
            continue;
        }
        if instance.get("iod_conformance").and_then(Value::as_str) != Some("not_assessed") {
            failures.push(format!(
                "structural instance {relative} overclaims IOD conformance"
            ));
        }
        let path = root.join(relative);
        let Ok(metadata) = fs::symlink_metadata(&path) else {
            failures.push(format!("structural artifact is missing: {relative}"));
            continue;
        };
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            failures.push(format!(
                "structural artifact is not a regular file: {relative}"
            ));
            continue;
        }
        let Ok(bytes) = fs::read(&path) else {
            failures.push(format!("structural artifact cannot be read: {relative}"));
            continue;
        };
        let expected_size = instance.get("size_bytes").and_then(Value::as_u64);
        let expected_sha = instance.get("sha256").and_then(Value::as_str);
        if expected_size != Some(bytes.len() as u64)
            || expected_sha != Some(crate::sha256_hex(&bytes).as_str())
        {
            failures.push(format!("structural artifact identity mismatch: {relative}"));
            continue;
        }
        match open_file(&path) {
            Ok(object) => {
                let sop_class = instance.get("sop_class_uid").and_then(Value::as_str);
                let sop_instance = instance.get("sop_instance_uid").and_then(Value::as_str);
                let transfer = instance.get("transfer_syntax_uid").and_then(Value::as_str);
                if sop_class != Some(object.meta().media_storage_sop_class_uid())
                    || sop_instance != Some(object.meta().media_storage_sop_instance_uid())
                    || transfer != Some(object.meta().transfer_syntax())
                {
                    failures.push(format!("structural Part 10 identity mismatch: {relative}"));
                }
                for (field, tag) in [
                    (
                        "study_instance_uid",
                        dicom_dictionary_std::tags::STUDY_INSTANCE_UID,
                    ),
                    (
                        "series_instance_uid",
                        dicom_dictionary_std::tags::SERIES_INSTANCE_UID,
                    ),
                    (
                        "sop_instance_uid",
                        dicom_dictionary_std::tags::SOP_INSTANCE_UID,
                    ),
                    (
                        "frame_of_reference_uid",
                        dicom_dictionary_std::tags::FRAME_OF_REFERENCE_UID,
                    ),
                ] {
                    let expected = instance
                        .pointer(&format!("/identity/{field}"))
                        .and_then(Value::as_str);
                    let actual = object
                        .element(tag)
                        .ok()
                        .and_then(|element| element.to_str().ok());
                    if actual.as_deref() != expected {
                        failures.push(format!(
                            "structural {field} identity mismatch in {relative}"
                        ));
                    }
                }
                match serde_json::from_value::<Vec<ResolvedAttribute>>(
                    instance.get("elements").cloned().unwrap_or_default(),
                ) {
                    Ok(elements) => validate_elements(
                        &object,
                        &elements,
                        transfer != Some("1.2.840.10008.1.2"),
                        relative,
                        &mut failures,
                    ),
                    Err(error) => failures.push(format!(
                        "structural element evidence is invalid for {relative}: {error}"
                    )),
                }
                match serde_json::from_value::<Vec<CanonicalContent>>(
                    instance.get("bulk").cloned().unwrap_or_default(),
                ) {
                    Ok(content) => validate_bulk(
                        &object,
                        &content,
                        transfer != Some("1.2.840.10008.1.2"),
                        relative,
                        &mut failures,
                    ),
                    Err(error) => failures.push(format!(
                        "structural bulk evidence is invalid for {relative}: {error}"
                    )),
                }
            }
            Err(error) => failures.push(format!(
                "structural Part 10 reopen failed for {relative}: {error}"
            )),
        }
    }
    let declared = paths;
    if let Ok(entries) = walk_files(root) {
        for relative in entries {
            if relative != "manifest.json" && !declared.contains(&relative) {
                failures.push(format!("undeclared structural output file: {relative}"));
            }
        }
    }
    (instances.len(), failures)
}

fn validate_reference_closure(instances: &[Value], failures: &mut Vec<String>) {
    let identities = instances
        .iter()
        .filter_map(|instance| {
            Some((
                instance.get("instance_id")?.as_str()?,
                instance.get("sop_class_uid")?.as_str()?,
                instance.get("sop_instance_uid")?.as_str()?,
            ))
        })
        .map(|(id, class, instance)| (id, (class, instance)))
        .collect::<std::collections::BTreeMap<_, _>>();
    for source in instances {
        let source_id = source
            .get("instance_id")
            .and_then(Value::as_str)
            .unwrap_or("<unknown>");
        for reference in source
            .get("references")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let target_id = reference.get("target_instance_id").and_then(Value::as_str);
            let valid = target_id
                .and_then(|target| identities.get(target).copied())
                .is_some_and(|(class, instance)| {
                    reference.get("source_instance_id").and_then(Value::as_str) == Some(source_id)
                        && reference
                            .get("referenced_sop_class_uid")
                            .and_then(Value::as_str)
                            == Some(class)
                        && reference
                            .get("referenced_sop_instance_uid")
                            .and_then(Value::as_str)
                            == Some(instance)
                });
            if !valid {
                failures.push(format!(
                    "structural reference closure mismatch from {source_id} to {}",
                    target_id.unwrap_or("<missing>")
                ));
            }
        }
    }
}

fn validate_elements(
    object: &Dataset,
    elements: &[ResolvedAttribute],
    enforce_vr: bool,
    relative: &str,
    failures: &mut Vec<String>,
) {
    for expected in elements {
        validate_element(
            object,
            expected.address.tag(),
            expected.vr,
            expected.value.as_ref(),
            enforce_vr,
            relative,
            failures,
        );
        if let Some(creator) = &expected.address.private_creator {
            let creator_tag =
                dicom_core::Tag(expected.address.group, expected.address.element >> 8);
            let matches = object
                .element(creator_tag)
                .ok()
                .and_then(|element| element.to_str().ok())
                .is_some_and(|actual| actual.trim_end() == creator);
            if !matches {
                failures.push(format!(
                    "structural private creator mismatch at {} in {relative}",
                    expected.address.normalized_tag()
                ));
            }
        }
    }
}

fn validate_element(
    object: &Dataset,
    tag: dicom_core::Tag,
    vr: DicomVr,
    expected: Option<&AttributeValue>,
    enforce_vr: bool,
    relative: &str,
    failures: &mut Vec<String>,
) {
    let Ok(element) = object.element(tag) else {
        failures.push(format!(
            "structural element {:04X},{:04X} is missing in {relative}",
            tag.group(),
            tag.element()
        ));
        return;
    };
    if enforce_vr && element.vr() != vr.as_dicom() {
        failures.push(format!(
            "structural element {:04X},{:04X} VR mismatch in {relative}",
            tag.group(),
            tag.element()
        ));
        return;
    }
    let matches = match expected {
        None => element.value().is_empty(),
        Some(AttributeValue::Primitive(value)) => primitive_matches(element, value),
        Some(AttributeValue::Multi(values)) => multi_matches(element, values),
        Some(AttributeValue::EncodedText(bytes)) | Some(AttributeValue::Binary(bytes)) => element
            .to_bytes()
            .is_ok_and(|actual| padded_bytes_match(actual.as_ref(), bytes)),
        Some(AttributeValue::Sequence(items)) => element
            .items()
            .is_some_and(|actual| sequence_matches(actual, items, enforce_vr, relative, failures)),
    };
    if !matches {
        failures.push(format!(
            "structural element {:04X},{:04X} value mismatch in {relative}",
            tag.group(),
            tag.element()
        ));
    }
}

fn primitive_matches(
    element: &dicom_core::DataElement<Dataset>,
    expected: &PrimitiveValue,
) -> bool {
    match expected {
        PrimitiveValue::String(value) => element
            .to_str()
            .is_ok_and(|actual| actual.as_ref() == value),
        PrimitiveValue::Signed(value) => {
            element.to_int::<i64>().is_ok_and(|actual| actual == *value)
        }
        PrimitiveValue::Unsigned(value) => {
            element.to_int::<u64>().is_ok_and(|actual| actual == *value)
        }
        PrimitiveValue::Float32Bits(value) => element
            .to_float32()
            .is_ok_and(|actual| actual.to_bits() == *value),
        PrimitiveValue::Float64Bits(value) => element
            .to_float64()
            .is_ok_and(|actual| actual.to_bits() == *value),
        PrimitiveValue::Tag(value) => matches!(
            element.value().primitive(),
            Some(dicom_core::value::PrimitiveValue::Tags(actual))
                if actual.as_ref() == [value.tag()]
        ),
    }
}

fn multi_matches(element: &dicom_core::DataElement<Dataset>, expected: &[PrimitiveValue]) -> bool {
    if expected
        .iter()
        .all(|value| matches!(value, PrimitiveValue::String(_)))
    {
        let joined = expected
            .iter()
            .map(|value| match value {
                PrimitiveValue::String(value) => value.as_str(),
                _ => unreachable!(),
            })
            .collect::<Vec<_>>()
            .join("\\");
        return element.to_str().is_ok_and(|actual| actual == joined);
    }
    if expected
        .iter()
        .all(|value| matches!(value, PrimitiveValue::Signed(_)))
    {
        let expected = expected
            .iter()
            .map(|value| match value {
                PrimitiveValue::Signed(value) => *value,
                _ => unreachable!(),
            })
            .collect::<Vec<_>>();
        return element
            .to_multi_int::<i64>()
            .is_ok_and(|actual| actual == expected);
    }
    if expected
        .iter()
        .all(|value| matches!(value, PrimitiveValue::Unsigned(_)))
    {
        let expected = expected
            .iter()
            .map(|value| match value {
                PrimitiveValue::Unsigned(value) => *value,
                _ => unreachable!(),
            })
            .collect::<Vec<_>>();
        return element
            .to_multi_int::<u64>()
            .is_ok_and(|actual| actual == expected);
    }
    if expected
        .iter()
        .all(|value| matches!(value, PrimitiveValue::Float32Bits(_)))
    {
        let expected = expected
            .iter()
            .map(|value| match value {
                PrimitiveValue::Float32Bits(value) => *value,
                _ => unreachable!(),
            })
            .collect::<Vec<_>>();
        return element
            .to_multi_float32()
            .is_ok_and(|actual| actual.iter().map(|value| value.to_bits()).eq(expected));
    }
    if expected
        .iter()
        .all(|value| matches!(value, PrimitiveValue::Float64Bits(_)))
    {
        let expected = expected
            .iter()
            .map(|value| match value {
                PrimitiveValue::Float64Bits(value) => *value,
                _ => unreachable!(),
            })
            .collect::<Vec<_>>();
        return element
            .to_multi_float64()
            .is_ok_and(|actual| actual.iter().map(|value| value.to_bits()).eq(expected));
    }
    if expected
        .iter()
        .all(|value| matches!(value, PrimitiveValue::Tag(_)))
    {
        let expected = expected
            .iter()
            .map(|value| match value {
                PrimitiveValue::Tag(value) => value.tag(),
                _ => unreachable!(),
            })
            .collect::<Vec<_>>();
        return matches!(
            element.value().primitive(),
            Some(dicom_core::value::PrimitiveValue::Tags(actual))
                if actual.as_ref() == expected
        );
    }
    false
}

fn sequence_matches(
    actual: &[Dataset],
    expected: &[AttributeItem],
    enforce_vr: bool,
    relative: &str,
    failures: &mut Vec<String>,
) -> bool {
    if actual.len() != expected.len() {
        return false;
    }
    let before = failures.len();
    for (actual, expected) in actual.iter().zip(expected) {
        for operation in &expected.attributes {
            match operation {
                AttributeOperation::Set { address, vr, value } => validate_element(
                    actual,
                    address.tag(),
                    *vr,
                    Some(value),
                    enforce_vr,
                    relative,
                    failures,
                ),
                AttributeOperation::Empty { address } => match actual.element(address.tag()) {
                    Ok(element) if element.value().is_empty() => {}
                    _ => failures.push(format!(
                        "structural nested empty element {} mismatch in {relative}",
                        address.normalized_tag()
                    )),
                },
                AttributeOperation::Remove { .. } => return false,
            }
        }
    }
    before == failures.len()
}

fn validate_bulk(
    object: &Dataset,
    content: &[CanonicalContent],
    enforce_vr: bool,
    relative: &str,
    failures: &mut Vec<String>,
) {
    for expected in content {
        let Ok(element) = object.element(expected.address.tag()) else {
            failures.push(format!(
                "structural bulk {} is missing in {relative}",
                expected.slot
            ));
            continue;
        };
        if enforce_vr && element.vr() != expected.vr.as_dicom() {
            failures.push(format!(
                "structural bulk {} VR mismatch in {relative}",
                expected.slot
            ));
            continue;
        }
        let valid = element.to_bytes().is_ok_and(|actual| {
            let size = usize::try_from(expected.size_bytes).ok();
            size.is_some_and(|size| {
                actual.len() >= size
                    && actual.len() <= size.saturating_add(1)
                    && crate::sha256_hex(&actual[..size]) == expected.sha256
                    && (actual.len() == size || actual[size] == 0)
            })
        });
        if !valid {
            failures.push(format!(
                "structural bulk {} identity mismatch in {relative}",
                expected.slot
            ));
        }
    }
}

fn padded_bytes_match(actual: &[u8], expected: &[u8]) -> bool {
    actual == expected
        || (expected.len() % 2 == 1
            && actual.len() == expected.len() + 1
            && actual.starts_with(expected)
            && actual[expected.len()] == 0)
}

pub fn assembly_report(manifest: &Value) -> Result<Value, String> {
    let instances = manifest
        .get("instances")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let total_bytes = instances
        .iter()
        .filter_map(|instance| instance.get("size_bytes").and_then(Value::as_u64))
        .sum::<u64>();
    let bulk_kinds = instances
        .iter()
        .flat_map(|instance| {
            instance
                .get("bulk")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
        })
        .filter_map(|bulk| bulk.get("kind").and_then(Value::as_str))
        .fold(
            std::collections::BTreeMap::<String, u64>::new(),
            |mut counts, kind| {
                *counts.entry(kind.to_owned()).or_default() += 1;
                counts
            },
        );
    let current_identity_projection = match manifest["manifest_schema_version"].as_str() {
        Some("1.0.0") => None,
        Some("2.0.0") => Some(
            manifest
                .get("identity_projection")
                .cloned()
                .ok_or_else(|| "assembly manifest 2.0 identity_projection missing".to_string())?,
        ),
        Some(version) => return Err(format!("unsupported assembly manifest version {version}")),
        None => return Err("assembly manifest schema version missing".to_string()),
    };
    let mut report = json!({
        "structural_assembly_report_schema_version": "1.0.0",
        "report_kind": "structural_assembly",
        "iod_conformance": "not_assessed",
        "counts": { "instances": instances.len(), "output_bytes": total_bytes },
        "bulk_kinds": bulk_kinds,
        "instances": instances.iter().map(|instance| json!({
            "instance_id": instance.get("instance_id"), "output_path": instance.get("output_path"),
            "sop_class_uid": instance.get("sop_class_uid"), "transfer_syntax_uid": instance.get("transfer_syntax_uid"),
            "iod_conformance": "not_assessed"
        })).collect::<Vec<_>>(),
        "warnings": ["IOD conformance was not assessed; structural results are excluded from curated and qualified coverage"]
    });
    if let Some(identity_projection) = current_identity_projection {
        report["structural_assembly_report_schema_version"] = "2.0.0".into();
        report["identity_projection"] = identity_projection;
    }
    Ok(report)
}

fn contains_key(value: &Value, key: &str) -> bool {
    match value {
        Value::Object(object) => {
            object.contains_key(key) || object.values().any(|value| contains_key(value, key))
        }
        Value::Array(values) => values.iter().any(|value| contains_key(value, key)),
        _ => false,
    }
}

fn safe_relative_path(value: &str) -> bool {
    let path = Path::new(value);
    !path.is_absolute()
        && !value.contains('\\')
        && !value.contains(':')
        && path
            .components()
            .all(|part| matches!(part, Component::Normal(_)))
}

fn walk_files(root: &Path) -> std::io::Result<Vec<String>> {
    fn walk(root: &Path, current: &Path, out: &mut Vec<String>) -> std::io::Result<()> {
        for entry in fs::read_dir(current)? {
            let entry = entry?;
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path)?;
            if metadata.is_dir() {
                walk(root, &path, out)?;
            } else if metadata.is_file() {
                out.push(
                    path.strip_prefix(root)
                        .unwrap()
                        .to_string_lossy()
                        .replace('\\', "/"),
                );
            }
        }
        Ok(())
    }
    let mut out = Vec::new();
    walk(root, root, &mut out)?;
    out.sort();
    Ok(out)
}
