use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::path::Path;

use dicom_dictionary_std::tags;
use dicom_object::open_file;
use serde_json::Value;

#[derive(Debug)]
struct GeometryInstance {
    path: String,
    case_id: String,
    study_instance_uid: String,
    series_instance_uid: String,
    frame_of_reference_uid: String,
    orientation: [f64; 6],
    projected_position: f64,
    instance_number: i32,
    expected_position: f64,
    expected_geometric_rank: usize,
    expected_instance_rank: usize,
    expected_count: usize,
    position_tolerance: f64,
    spacing_tolerance: f64,
    descending: bool,
    conflict_expected: bool,
}

pub(crate) fn validate_manifest_geometry(root: &Path, files: &[Value], failures: &mut Vec<String>) {
    let mut series = BTreeMap::<String, Vec<GeometryInstance>>::new();
    for file in files {
        let Some(expected) = file
            .get("expected_geometry")
            .filter(|value| !value.is_null())
        else {
            continue;
        };
        match read_instance(root, file, expected) {
            Ok(instance) => series
                .entry(instance.series_instance_uid.clone())
                .or_default()
                .push(instance),
            Err(failure) => failures.push(failure),
        }
    }

    for (series_uid, mut instances) in series {
        validate_series(&series_uid, &mut instances, failures);
    }
}

fn read_instance(root: &Path, file: &Value, expected: &Value) -> Result<GeometryInstance, String> {
    let path = string_at(file, "/path")?;
    let case_id = string_at(file, "/case_id")?;
    let object = open_file(root.join(path))
        .map_err(|error| format!("{path}: geometry_open_file: {error}"))?;
    let orientation = fixed_values::<6>(&object, tags::IMAGE_ORIENTATION_PATIENT, path)?;
    let position = fixed_values::<3>(&object, tags::IMAGE_POSITION_PATIENT, path)?;
    let row = [orientation[0], orientation[1], orientation[2]];
    let column = [orientation[3], orientation[4], orientation[5]];
    let normal = cross(row, column);
    let normal_length = dot(normal, normal).sqrt();
    if (normal_length - 1.0).abs() > 0.000_01 {
        return Err(format!(
            "{path}: geometry_orientation: slice normal length is {normal_length}, expected 1"
        ));
    }
    let projected_position = dot(normal, position);
    let instance_number = object
        .element(tags::INSTANCE_NUMBER)
        .map_err(|error| format!("{path}: geometry_instance_number: {error}"))?
        .to_int::<i32>()
        .map_err(|error| format!("{path}: geometry_instance_number: {error}"))?;
    let study_instance_uid = element_string(&object, tags::STUDY_INSTANCE_UID, path)?;
    let series_instance_uid = element_string(&object, tags::SERIES_INSTANCE_UID, path)?;
    let frame_of_reference_uid = element_string(&object, tags::FRAME_OF_REFERENCE_UID, path)?;

    let manifest_series_uid = string_at(file, "/uids/series_instance_uid")?;
    if series_instance_uid != manifest_series_uid {
        return Err(format!(
            "{path}: geometry_series_instance_uid: dataset {series_instance_uid}, manifest {manifest_series_uid}"
        ));
    }
    let expected_position = number_at(expected, "/position_along_normal_mm")?;
    let position_tolerance = number_at(expected, "/position_tolerance_mm")?;
    if (projected_position - expected_position).abs() > position_tolerance {
        return Err(format!(
            "{path}: geometry_projected_position: actual {projected_position}, expected {expected_position} ± {position_tolerance}"
        ));
    }
    let expected_instance_number = integer_at(expected, "/instance_number")?;
    if i64::from(instance_number) != expected_instance_number {
        return Err(format!(
            "{path}: geometry_instance_number: actual {instance_number}, expected {expected_instance_number}"
        ));
    }
    let sort_basis = string_at(expected, "/sort_basis")?;
    if sort_basis != "image_position_patient_projected_on_slice_normal" {
        return Err(format!(
            "{path}: geometry_sort_basis: unsupported {sort_basis}"
        ));
    }

    Ok(GeometryInstance {
        path: path.to_string(),
        case_id: case_id.to_string(),
        study_instance_uid,
        series_instance_uid,
        frame_of_reference_uid,
        orientation,
        projected_position,
        instance_number,
        expected_position,
        expected_geometric_rank: usize_at(expected, "/geometric_order_index")?,
        expected_instance_rank: usize_at(expected, "/instance_number_order_index")?,
        expected_count: usize_at(expected, "/series_instance_count")?,
        position_tolerance,
        spacing_tolerance: number_at(expected, "/spacing_tolerance_mm")?,
        descending: string_at(expected, "/sort_direction")? == "descending",
        conflict_expected: bool_at(expected, "/sorting_conflict_expected")?,
    })
}

fn validate_series(
    series_uid: &str,
    instances: &mut [GeometryInstance],
    failures: &mut Vec<String>,
) {
    let first_case_id = instances[0].case_id.clone();
    let first_study_uid = instances[0].study_instance_uid.clone();
    let first_frame_uid = instances[0].frame_of_reference_uid.clone();
    let first_orientation = instances[0].orientation;
    let expected_count = instances[0].expected_count;
    let descending = instances[0].descending;
    if instances.len() != expected_count {
        failures.push(format!(
            "{}: geometry_series_count: actual {}, expected {expected_count}",
            first_case_id,
            instances.len()
        ));
    }
    for instance in instances.iter().skip(1) {
        if instance.case_id != first_case_id
            || instance.study_instance_uid != first_study_uid
            || instance.frame_of_reference_uid != first_frame_uid
            || !vectors_close(
                instance.orientation,
                first_orientation,
                instance.position_tolerance,
            )
            || instance.expected_count != expected_count
            || instance.descending != descending
        {
            failures.push(format!(
                "{}: geometry_series_identity: series {series_uid} does not share case, Study, Frame of Reference, orientation, count, and direction",
                instance.path
            ));
        }
    }

    instances.sort_by(|left, right| {
        compare_f64(
            left.projected_position,
            right.projected_position,
            descending,
        )
        .then_with(|| left.path.cmp(&right.path))
    });
    for (index, instance) in instances.iter().enumerate() {
        let actual_rank = index + 1;
        if actual_rank != instance.expected_geometric_rank {
            failures.push(format!(
                "{}: geometry_order: actual rank {actual_rank}, expected {}",
                instance.path, instance.expected_geometric_rank
            ));
        }
    }
    for pair in instances.windows(2) {
        let actual_spacing = (pair[1].projected_position - pair[0].projected_position).abs();
        let expected_spacing = (pair[1].expected_position - pair[0].expected_position).abs();
        let tolerance = pair[0].spacing_tolerance.max(pair[1].spacing_tolerance);
        if (actual_spacing - expected_spacing).abs() > tolerance {
            failures.push(format!(
                "{}: geometry_spacing: actual {actual_spacing}, expected {expected_spacing} ± {tolerance}",
                pair[1].path
            ));
        }
        if actual_spacing <= pair[0].position_tolerance.max(pair[1].position_tolerance) {
            failures.push(format!(
                "{}: geometry_spacing: projected positions are duplicated within tolerance",
                pair[1].path
            ));
        }
    }

    let geometric_paths = instances
        .iter()
        .map(|instance| instance.path.as_str())
        .collect::<Vec<_>>();
    let mut instance_order = instances.iter().collect::<Vec<_>>();
    instance_order.sort_by(|left, right| {
        left.instance_number
            .cmp(&right.instance_number)
            .then_with(|| left.path.cmp(&right.path))
    });
    for (index, instance) in instance_order.iter().enumerate() {
        let actual_rank = index + 1;
        if actual_rank != instance.expected_instance_rank {
            failures.push(format!(
                "{}: instance_number_order: actual rank {actual_rank}, expected {}",
                instance.path, instance.expected_instance_rank
            ));
        }
    }
    let instance_paths = instance_order
        .iter()
        .map(|instance| instance.path.as_str())
        .collect::<Vec<_>>();
    let conflict = geometric_paths != instance_paths;
    for instance in instances {
        if conflict != instance.conflict_expected {
            failures.push(format!(
                "{}: geometry_sorting_conflict: actual {conflict}, expected {}",
                instance.path, instance.conflict_expected
            ));
        }
    }
}

fn fixed_values<const N: usize>(
    object: &crate::OpenedObject,
    tag: dicom_core::Tag,
    path: &str,
) -> Result<[f64; N], String> {
    let values = object
        .element(tag)
        .map_err(|error| format!("{path}: geometry_attribute_{tag}: {error}"))?
        .to_multi_float64()
        .map_err(|error| format!("{path}: geometry_attribute_{tag}: {error}"))?;
    values.try_into().map_err(|values: Vec<f64>| {
        format!(
            "{path}: geometry_attribute_{tag}: expected {N} values, got {}",
            values.len()
        )
    })
}

fn element_string(
    object: &crate::OpenedObject,
    tag: dicom_core::Tag,
    path: &str,
) -> Result<String, String> {
    object
        .element(tag)
        .map_err(|error| format!("{path}: geometry_attribute_{tag}: {error}"))?
        .to_str()
        .map(|value| value.trim_matches('\0').trim().to_string())
        .map_err(|error| format!("{path}: geometry_attribute_{tag}: {error}"))
}

fn cross(left: [f64; 3], right: [f64; 3]) -> [f64; 3] {
    [
        left[1] * right[2] - left[2] * right[1],
        left[2] * right[0] - left[0] * right[2],
        left[0] * right[1] - left[1] * right[0],
    ]
}

fn dot(left: [f64; 3], right: [f64; 3]) -> f64 {
    left[0] * right[0] + left[1] * right[1] + left[2] * right[2]
}

fn vectors_close<const N: usize>(left: [f64; N], right: [f64; N], tolerance: f64) -> bool {
    left.into_iter()
        .zip(right)
        .all(|(left, right)| (left - right).abs() <= tolerance)
}

fn compare_f64(left: f64, right: f64, descending: bool) -> Ordering {
    let ordering = left.total_cmp(&right);
    if descending {
        ordering.reverse()
    } else {
        ordering
    }
}

fn string_at<'a>(value: &'a Value, pointer: &str) -> Result<&'a str, String> {
    value
        .pointer(pointer)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("manifest geometry field {pointer} must be a string"))
}

fn number_at(value: &Value, pointer: &str) -> Result<f64, String> {
    value
        .pointer(pointer)
        .and_then(Value::as_f64)
        .ok_or_else(|| format!("manifest geometry field {pointer} must be a number"))
}

fn integer_at(value: &Value, pointer: &str) -> Result<i64, String> {
    value
        .pointer(pointer)
        .and_then(Value::as_i64)
        .ok_or_else(|| format!("manifest geometry field {pointer} must be an integer"))
}

fn usize_at(value: &Value, pointer: &str) -> Result<usize, String> {
    value
        .pointer(pointer)
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| format!("manifest geometry field {pointer} must be a positive integer"))
}

fn bool_at(value: &Value, pointer: &str) -> Result<bool, String> {
    value
        .pointer(pointer)
        .and_then(Value::as_bool)
        .ok_or_else(|| format!("manifest geometry field {pointer} must be a boolean"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn series_validator_rejects_a_false_geometric_rank() {
        let mut instances = vec![
            instance("slice-001.dcm", 0.0, 30, 2, 3),
            instance("slice-002.dcm", 5.0, 10, 2, 1),
            instance("slice-003.dcm", 10.0, 20, 3, 2),
        ];
        let mut failures = Vec::new();
        validate_series("1.2.3.2", &mut instances, &mut failures);
        assert!(
            failures
                .iter()
                .any(|failure| failure.contains("geometry_order")),
            "false geometric rank must be rejected: {failures:?}"
        );
    }

    fn instance(
        path: &str,
        position: f64,
        instance_number: i32,
        geometric_rank: usize,
        instance_rank: usize,
    ) -> GeometryInstance {
        GeometryInstance {
            path: path.to_string(),
            case_id: "geometry/ct/sort".to_string(),
            study_instance_uid: "1.2.3.1".to_string(),
            series_instance_uid: "1.2.3.2".to_string(),
            frame_of_reference_uid: "1.2.3.3".to_string(),
            orientation: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
            projected_position: position,
            instance_number,
            expected_position: position,
            expected_geometric_rank: geometric_rank,
            expected_instance_rank: instance_rank,
            expected_count: 3,
            position_tolerance: 0.000_01,
            spacing_tolerance: 0.000_01,
            descending: false,
            conflict_expected: true,
        }
    }
}
