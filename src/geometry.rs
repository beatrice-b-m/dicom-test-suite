use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
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
    instance_number: Option<i32>,
    expected_position: f64,
    expected_geometric_rank: usize,
    expected_instance_rank: Option<usize>,
    expected_count: usize,
    expected_adjacent_spacing: Vec<f64>,
    expected_spacing_uniform: bool,
    position_tolerance: f64,
    spacing_tolerance: f64,
    descending: bool,
    conflict_expected: Option<bool>,
}

#[derive(Debug)]
struct SeriesOrganizationInstance {
    path: String,
    case_id: String,
    group_id: String,
    study_instance_uid: String,
    series_instance_uid: String,
    frame_of_reference_uid: String,
    expected_study_series_count: usize,
    expected_series_ordinal: usize,
    expected_series_instance_count: usize,
    expected_shared_study: bool,
    expected_shared_frame: bool,
    expected_distinct_series: bool,
}

pub(crate) fn validate_manifest_geometry(root: &Path, files: &[Value], failures: &mut Vec<String>) {
    let mut series = BTreeMap::<String, Vec<GeometryInstance>>::new();
    let mut organization_groups =
        BTreeMap::<(String, String), Vec<SeriesOrganizationInstance>>::new();
    for file in files {
        if let Some(expected) = file
            .get("expected_geometry")
            .filter(|value| !value.is_null())
        {
            match read_instance(root, file, expected) {
                Ok(instance) => series
                    .entry(instance.series_instance_uid.clone())
                    .or_default()
                    .push(instance),
                Err(failure) => failures.push(failure),
            }
        }
        if let Some(expected) = file
            .get("expected_series_organization")
            .filter(|value| !value.is_null())
        {
            match read_series_organization_instance(root, file, expected) {
                Ok(instance) => organization_groups
                    .entry((instance.case_id.clone(), instance.group_id.clone()))
                    .or_default()
                    .push(instance),
                Err(failure) => failures.push(failure),
            }
        }
    }

    for (series_uid, mut instances) in series {
        validate_series(&series_uid, &mut instances, failures);
    }
    for ((case_id, group_id), instances) in organization_groups {
        validate_series_organization(&case_id, &group_id, &instances, failures);
    }
}

fn read_instance(root: &Path, file: &Value, expected: &Value) -> Result<GeometryInstance, String> {
    let path = string_at(file, "/path")?;
    let case_id = string_at(file, "/case_id")?;
    let object = open_file(root.join(path))
        .map_err(|error| format!("{path}: geometry_open_file: {error}"))?;
    let orientation = fixed_values::<6>(&object, tags::IMAGE_ORIENTATION_PATIENT, path)?;
    let position = fixed_values::<3>(&object, tags::IMAGE_POSITION_PATIENT, path)?;
    let expected_position_array = fixed_numbers::<3>(expected, "/image_position_patient")?;
    let expected_orientation = fixed_numbers::<6>(expected, "/image_orientation_patient")?;
    let position_tolerance = number_at(expected, "/position_tolerance_mm")?;
    if !vectors_close(position, expected_position_array, position_tolerance) {
        return Err(format!(
            "{path}: geometry_image_position_patient: actual {position:?}, expected {expected_position_array:?} ± {position_tolerance}"
        ));
    }
    if !vectors_close(orientation, expected_orientation, position_tolerance) {
        return Err(format!(
            "{path}: geometry_image_orientation_patient: actual {orientation:?}, expected {expected_orientation:?} ± {position_tolerance}"
        ));
    }
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
    let instance_number_element = object.element(tags::INSTANCE_NUMBER).map_err(|error| {
        format!("{path}: geometry_instance_number: Type 2 element missing: {error}")
    })?;
    let instance_number_state = string_at(expected, "/instance_number_state")?;
    let instance_number = match instance_number_state {
        "numeric" => Some(instance_number_element.to_int::<i32>().map_err(|error| {
            format!("{path}: geometry_instance_number: expected numeric value: {error}")
        })?),
        "empty" => {
            let value = instance_number_element
                .to_str()
                .map_err(|error| format!("{path}: geometry_instance_number: {error}"))?;
            if !value.trim_matches('\0').trim().is_empty() {
                return Err(format!(
                    "{path}: geometry_instance_number: expected empty Type 2 value, actual {value:?}"
                ));
            }
            None
        }
        state => {
            return Err(format!(
                "{path}: geometry_instance_number_state: unsupported {state}"
            ));
        }
    };
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
    if (projected_position - expected_position).abs() > position_tolerance {
        return Err(format!(
            "{path}: geometry_projected_position: actual {projected_position}, expected {expected_position} ± {position_tolerance}"
        ));
    }
    let expected_instance_number = optional_i64_at(expected, "/instance_number")?;
    if instance_number.map(i64::from) != expected_instance_number {
        return Err(format!(
            "{path}: geometry_instance_number: actual {:?}, expected {expected_instance_number:?}",
            instance_number
        ));
    }
    if let Some(expected_tilt) = optional_number_at(expected, "/gantry_detector_tilt_degrees")? {
        let actual_tilt = object
            .element(tags::GANTRY_DETECTOR_TILT)
            .map_err(|error| format!("{path}: geometry_gantry_detector_tilt: {error}"))?
            .to_float64()
            .map_err(|error| format!("{path}: geometry_gantry_detector_tilt: {error}"))?;
        if (actual_tilt - expected_tilt).abs() > position_tolerance {
            return Err(format!(
                "{path}: geometry_gantry_detector_tilt: actual {actual_tilt}, expected {expected_tilt} ± {position_tolerance}"
            ));
        }
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
        expected_instance_rank: optional_usize_at(expected, "/instance_number_order_index")?,
        expected_count: usize_at(expected, "/series_instance_count")?,
        expected_adjacent_spacing: numbers_at(expected, "/adjacent_spacing_mm")?,
        expected_spacing_uniform: bool_at(expected, "/spacing_uniform")?,
        position_tolerance,
        spacing_tolerance: number_at(expected, "/spacing_tolerance_mm")?,
        descending: string_at(expected, "/sort_direction")? == "descending",
        conflict_expected: optional_bool_at(expected, "/sorting_conflict_expected")?,
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
    let expected_adjacent_spacing = instances[0].expected_adjacent_spacing.clone();
    let expected_spacing_uniform = instances[0].expected_spacing_uniform;
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
            || instance.expected_adjacent_spacing != expected_adjacent_spacing
            || instance.expected_spacing_uniform != expected_spacing_uniform
            || instance.descending != descending
        {
            failures.push(format!(
                "{}: geometry_series_identity: series {series_uid} does not share case, Study, Frame of Reference, orientation, count, spacing declaration, and direction",
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
    let mut actual_spacings = Vec::with_capacity(instances.len().saturating_sub(1));
    for pair in instances.windows(2) {
        let actual_spacing = (pair[1].projected_position - pair[0].projected_position).abs();
        actual_spacings.push(actual_spacing);
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
    if actual_spacings.len() != expected_adjacent_spacing.len() {
        failures.push(format!(
            "{first_case_id}: geometry_adjacent_spacing: actual vector length {}, expected {}",
            actual_spacings.len(),
            expected_adjacent_spacing.len()
        ));
    } else {
        for (index, (actual, expected)) in actual_spacings
            .iter()
            .zip(&expected_adjacent_spacing)
            .enumerate()
        {
            let tolerance = instances[index]
                .spacing_tolerance
                .max(instances[index + 1].spacing_tolerance);
            if (actual - expected).abs() > tolerance {
                failures.push(format!(
                    "{}: geometry_adjacent_spacing: interval {} actual {actual}, declared {expected} ± {tolerance}",
                    instances[index + 1].path,
                    index + 1
                ));
            }
        }
    }
    let actual_uniform = actual_spacings.first().is_none_or(|first| {
        actual_spacings.iter().enumerate().all(|(index, spacing)| {
            let tolerance = instances[index]
                .spacing_tolerance
                .max(instances[index + 1].spacing_tolerance);
            (spacing - first).abs() <= tolerance
        })
    });
    if actual_uniform != expected_spacing_uniform {
        failures.push(format!(
            "{first_case_id}: geometry_spacing_uniform: actual {actual_uniform}, expected {expected_spacing_uniform}"
        ));
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
        let duplicate = instance.instance_number.is_some()
            && instances
                .iter()
                .filter(|candidate| candidate.instance_number == instance.instance_number)
                .count()
                > 1;
        if duplicate {
            if instance.expected_instance_rank.is_some() {
                failures.push(format!(
                    "{}: instance_number_order: duplicate numeric values require a null rank",
                    instance.path
                ));
            }
        } else if instance.instance_number.is_none() {
            if instance.expected_instance_rank.is_some() {
                failures.push(format!(
                    "{}: instance_number_order: empty Type 2 value requires a null rank",
                    instance.path
                ));
            }
        } else if let Some(expected_rank) = instance.expected_instance_rank {
            let actual_rank = index + 1;
            if actual_rank != expected_rank {
                failures.push(format!(
                    "{}: instance_number_order: actual rank {actual_rank}, expected {expected_rank}",
                    instance.path
                ));
            }
        } else if instance.instance_number.is_some() {
            failures.push(format!(
                "{}: instance_number_order: unique numeric value requires a rank",
                instance.path
            ));
        }
    }
    let instance_paths = instance_order
        .iter()
        .map(|instance| instance.path.as_str())
        .collect::<Vec<_>>();
    let instance_order_is_defined = instances
        .iter()
        .all(|instance| instance.instance_number.is_some())
        && instances
            .iter()
            .filter_map(|instance| instance.instance_number)
            .collect::<BTreeSet<_>>()
            .len()
            == instances.len();
    let conflict = instance_order_is_defined.then_some(geometric_paths != instance_paths);
    for instance in instances {
        if conflict != instance.conflict_expected {
            failures.push(format!(
                "{}: geometry_sorting_conflict: actual {conflict:?}, expected {:?}",
                instance.path, instance.conflict_expected
            ));
        }
    }
}

fn read_series_organization_instance(
    root: &Path,
    file: &Value,
    expected: &Value,
) -> Result<SeriesOrganizationInstance, String> {
    let path = string_at(file, "/path")?;
    let object = open_file(root.join(path))
        .map_err(|error| format!("{path}: series_organization_open_file: {error}"))?;
    Ok(SeriesOrganizationInstance {
        path: path.to_string(),
        case_id: string_at(file, "/case_id")?.to_string(),
        group_id: string_at(expected, "/group_id")?.to_string(),
        study_instance_uid: element_string(&object, tags::STUDY_INSTANCE_UID, path)?,
        series_instance_uid: element_string(&object, tags::SERIES_INSTANCE_UID, path)?,
        frame_of_reference_uid: element_string(&object, tags::FRAME_OF_REFERENCE_UID, path)?,
        expected_study_series_count: usize_at(expected, "/study_series_count")?,
        expected_series_ordinal: usize_at(expected, "/series_ordinal")?,
        expected_series_instance_count: usize_at(expected, "/series_instance_count")?,
        expected_shared_study: bool_at(expected, "/shared_study_instance_uid_expected")?,
        expected_shared_frame: bool_at(expected, "/shared_frame_of_reference_uid_expected")?,
        expected_distinct_series: bool_at(expected, "/distinct_series_instance_uids_expected")?,
    })
}

fn validate_series_organization(
    case_id: &str,
    group_id: &str,
    instances: &[SeriesOrganizationInstance],
    failures: &mut Vec<String>,
) {
    let first = &instances[0];
    for instance in instances.iter().skip(1) {
        if instance.expected_study_series_count != first.expected_study_series_count
            || instance.expected_shared_study != first.expected_shared_study
            || instance.expected_shared_frame != first.expected_shared_frame
            || instance.expected_distinct_series != first.expected_distinct_series
        {
            failures.push(format!(
                "{}: series_organization_declaration: group {group_id} has inconsistent group-level expectations",
                instance.path
            ));
        }
    }

    let mut by_series = BTreeMap::<&str, Vec<&SeriesOrganizationInstance>>::new();
    for instance in instances {
        by_series
            .entry(&instance.series_instance_uid)
            .or_default()
            .push(instance);
    }
    if by_series.len() != first.expected_study_series_count {
        failures.push(format!(
            "{case_id}/{group_id}: series_organization_study_series_count: actual {}, expected {}",
            by_series.len(),
            first.expected_study_series_count
        ));
    }

    let mut ordinal_to_uid = BTreeMap::<usize, &str>::new();
    for (series_uid, members) in &by_series {
        let series_first = members[0];
        if members.len() != series_first.expected_series_instance_count {
            failures.push(format!(
                "{}: series_organization_series_instance_count: series {series_uid} actual {}, expected {}",
                series_first.path,
                members.len(),
                series_first.expected_series_instance_count
            ));
        }
        for member in members.iter().skip(1) {
            if member.expected_series_ordinal != series_first.expected_series_ordinal
                || member.expected_series_instance_count
                    != series_first.expected_series_instance_count
            {
                failures.push(format!(
                    "{}: series_organization_series_declaration: series {series_uid} has inconsistent ordinal or instance count",
                    member.path
                ));
            }
        }
        if let Some(previous_uid) = ordinal_to_uid.insert(
            series_first.expected_series_ordinal,
            series_first.series_instance_uid.as_str(),
        ) {
            if previous_uid != *series_uid {
                failures.push(format!(
                    "{}: series_organization_series_ordinal: ordinal {} is shared by series {previous_uid} and {series_uid}",
                    series_first.path, series_first.expected_series_ordinal
                ));
            }
        }
    }
    let actual_ordinals = ordinal_to_uid.keys().copied().collect::<BTreeSet<_>>();
    let expected_ordinals = (1..=first.expected_study_series_count).collect::<BTreeSet<_>>();
    if actual_ordinals != expected_ordinals {
        failures.push(format!(
            "{case_id}/{group_id}: series_organization_series_ordinals: actual {actual_ordinals:?}, expected {expected_ordinals:?}"
        ));
    }

    let study_count = instances
        .iter()
        .map(|instance| &instance.study_instance_uid)
        .collect::<BTreeSet<_>>()
        .len();
    let frame_count = instances
        .iter()
        .map(|instance| &instance.frame_of_reference_uid)
        .collect::<BTreeSet<_>>()
        .len();
    let shared_study = study_count == 1;
    let shared_frame = frame_count == 1;
    let distinct_series = by_series.len() == first.expected_study_series_count;
    if shared_study != first.expected_shared_study {
        failures.push(format!(
            "{case_id}/{group_id}: series_organization_shared_study: actual {shared_study}, expected {}",
            first.expected_shared_study
        ));
    }
    if shared_frame != first.expected_shared_frame {
        failures.push(format!(
            "{case_id}/{group_id}: series_organization_shared_frame_of_reference: actual {shared_frame}, expected {}",
            first.expected_shared_frame
        ));
    }
    if distinct_series != first.expected_distinct_series {
        failures.push(format!(
            "{case_id}/{group_id}: series_organization_distinct_series: actual {distinct_series}, expected {}",
            first.expected_distinct_series
        ));
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

fn fixed_numbers<const N: usize>(value: &Value, pointer: &str) -> Result<[f64; N], String> {
    numbers_at(value, pointer)?
        .try_into()
        .map_err(|values: Vec<f64>| {
            format!(
                "manifest geometry field {pointer} must contain {N} numbers, got {}",
                values.len()
            )
        })
}

fn numbers_at(value: &Value, pointer: &str) -> Result<Vec<f64>, String> {
    value
        .pointer(pointer)
        .and_then(Value::as_array)
        .ok_or_else(|| format!("manifest geometry field {pointer} must be an array"))?
        .iter()
        .map(|value| {
            value
                .as_f64()
                .ok_or_else(|| format!("manifest geometry field {pointer} must contain numbers"))
        })
        .collect()
}

fn optional_i64_at(value: &Value, pointer: &str) -> Result<Option<i64>, String> {
    let field = value
        .pointer(pointer)
        .ok_or_else(|| format!("manifest geometry field {pointer} is missing"))?;
    if field.is_null() {
        Ok(None)
    } else {
        field
            .as_i64()
            .map(Some)
            .ok_or_else(|| format!("manifest geometry field {pointer} must be an integer or null"))
    }
}

fn optional_number_at(value: &Value, pointer: &str) -> Result<Option<f64>, String> {
    let Some(field) = value.pointer(pointer) else {
        return Ok(None);
    };
    if field.is_null() {
        Ok(None)
    } else {
        field
            .as_f64()
            .map(Some)
            .ok_or_else(|| format!("manifest geometry field {pointer} must be a number or null"))
    }
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

fn optional_usize_at(value: &Value, pointer: &str) -> Result<Option<usize>, String> {
    let field = value
        .pointer(pointer)
        .ok_or_else(|| format!("manifest geometry field {pointer} is missing"))?;
    if field.is_null() {
        Ok(None)
    } else {
        field
            .as_u64()
            .and_then(|value| usize::try_from(value).ok())
            .map(Some)
            .ok_or_else(|| {
                format!("manifest geometry field {pointer} must be a positive integer or null")
            })
    }
}

fn optional_bool_at(value: &Value, pointer: &str) -> Result<Option<bool>, String> {
    let field = value
        .pointer(pointer)
        .ok_or_else(|| format!("manifest geometry field {pointer} is missing"))?;
    if field.is_null() {
        Ok(None)
    } else {
        field
            .as_bool()
            .map(Some)
            .ok_or_else(|| format!("manifest geometry field {pointer} must be a boolean or null"))
    }
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

    #[test]
    fn series_validator_accepts_null_ranks_for_duplicate_instance_numbers() {
        let mut instances = vec![
            instance("slice-001.dcm", 0.0, 7, 1, 1),
            instance("slice-002.dcm", 5.0, 7, 2, 2),
            instance("slice-003.dcm", 10.0, 9, 3, 3),
        ];
        instances[0].expected_instance_rank = None;
        instances[1].expected_instance_rank = None;
        instances[2].expected_instance_rank = Some(3);
        for instance in &mut instances {
            instance.conflict_expected = None;
        }

        let mut failures = Vec::new();
        validate_series("1.2.3.2", &mut instances, &mut failures);
        assert!(
            failures.is_empty(),
            "duplicates must remain unordered: {failures:?}"
        );
    }

    #[test]
    fn series_validator_requires_null_rank_for_empty_instance_number() {
        let mut instances = vec![
            instance("slice-001.dcm", 0.0, 1, 1, 1),
            instance("slice-002.dcm", 5.0, 2, 2, 2),
            instance("slice-003.dcm", 10.0, 3, 3, 3),
        ];
        instances[1].instance_number = None;
        instances[1].expected_instance_rank = Some(2);
        for instance in &mut instances {
            instance.conflict_expected = None;
        }

        let mut failures = Vec::new();
        validate_series("1.2.3.2", &mut instances, &mut failures);
        assert!(
            failures
                .iter()
                .any(|failure| failure.contains("empty Type 2 value requires a null rank")),
            "empty Instance Number must not acquire a synthetic rank: {failures:?}"
        );
    }

    #[test]
    fn series_validator_checks_declared_spacing_vector_and_uniformity() {
        let mut instances = vec![
            instance("slice-001.dcm", 0.0, 1, 1, 1),
            instance("slice-002.dcm", 5.0, 2, 2, 2),
            instance("slice-003.dcm", 12.0, 3, 3, 3),
        ];
        for instance in &mut instances {
            instance.expected_adjacent_spacing = vec![5.0, 6.0];
            instance.expected_spacing_uniform = true;
            instance.conflict_expected = Some(false);
        }

        let mut failures = Vec::new();
        validate_series("1.2.3.2", &mut instances, &mut failures);
        assert!(
            failures
                .iter()
                .any(|failure| failure.contains("geometry_adjacent_spacing")),
            "false spacing vector must be rejected: {failures:?}"
        );
        assert!(
            failures
                .iter()
                .any(|failure| failure.contains("geometry_spacing_uniform")),
            "false uniformity must be rejected: {failures:?}"
        );
    }

    #[test]
    fn organization_validator_checks_ordinals_and_series_counts() {
        let instances = vec![
            organization_instance("a-1.dcm", "1.2.3.10", 1),
            organization_instance("a-2.dcm", "1.2.3.10", 1),
            organization_instance("b-1.dcm", "1.2.3.20", 1),
            organization_instance("b-2.dcm", "1.2.3.20", 1),
        ];
        let mut failures = Vec::new();
        validate_series_organization("geometry/ct/multi", "study-a", &instances, &mut failures);
        assert!(
            failures
                .iter()
                .any(|failure| failure.contains("series_organization_series_ordinals")),
            "duplicate series ordinal must be rejected: {failures:?}"
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
            instance_number: Some(instance_number),
            expected_position: position,
            expected_geometric_rank: geometric_rank,
            expected_instance_rank: Some(instance_rank),
            expected_count: 3,
            expected_adjacent_spacing: vec![5.0, 5.0],
            expected_spacing_uniform: true,
            position_tolerance: 0.000_01,
            spacing_tolerance: 0.000_01,
            descending: false,
            conflict_expected: Some(true),
        }
    }

    fn organization_instance(
        path: &str,
        series_instance_uid: &str,
        series_ordinal: usize,
    ) -> SeriesOrganizationInstance {
        SeriesOrganizationInstance {
            path: path.to_string(),
            case_id: "geometry/ct/multi".to_string(),
            group_id: "study-a".to_string(),
            study_instance_uid: "1.2.3.1".to_string(),
            series_instance_uid: series_instance_uid.to_string(),
            frame_of_reference_uid: "1.2.3.3".to_string(),
            expected_study_series_count: 2,
            expected_series_ordinal: series_ordinal,
            expected_series_instance_count: 2,
            expected_shared_study: true,
            expected_shared_frame: true,
            expected_distinct_series: true,
        }
    }
}
