use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};
use synth_dicom_gen::{sdk::CorpusSelector, sha256_hex};

pub fn oracle() -> Value {
    let bytes = include_bytes!("../fixtures/generic-ct-geometry-semantics.json");
    assert_eq!(
        sha256_hex(bytes),
        "31dc5d0d2d0dd43d20fb898e6eb162219b95d751016df653a6e94f29a6572fb0",
        "the reviewed caller-owned CT geometry oracle is immutable"
    );
    let value: Value = serde_json::from_slice(bytes).unwrap();
    assert_eq!(value["oracle_version"], "1.0.0");
    assert_eq!(
        value["accepted_original_pin_baseline3"]["source_revision"],
        "232b9de41f97ee95abe1ecc40b6b8b70ebeeea5f"
    );
    assert_eq!(value["accepted_original_pin_baseline3"]["seed"], 1);
    assert_eq!(
        value["accepted_original_pin_baseline3"]["artifact_count"],
        16
    );
    value
}

pub fn selector() -> CorpusSelector {
    CorpusSelector::CaseIds {
        profile: "extended".into(),
        include_stress: false,
        case_ids: vec![oracle()["caller"]["case_id"].as_str().unwrap().into()],
    }
}

pub struct GenericCtGeometryBundle {
    pub root: PathBuf,
    pub members: PathBuf,
    pub descriptor: PathBuf,
}

impl GenericCtGeometryBundle {
    pub fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = fs::canonicalize(std::env::temp_dir())
            .unwrap()
            .join(format!(
                "generic-ct-geometry-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
        fs::create_dir(&root).unwrap();
        let source =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/generic-ct-geometry-corpus");
        for relative in [
            "definition.json",
            "members/cases/registry.json",
            "members/cases/recipes/caller-angled-order-study.json",
        ] {
            let destination = root.join(relative);
            fs::create_dir_all(destination.parent().unwrap()).unwrap();
            fs::copy(source.join(relative), destination).unwrap();
        }
        let serialized =
            fs::read_to_string(root.join("members/cases/recipes/caller-angled-order-study.json"))
                .unwrap();
        for historical in [
            "geometry/ct/spatial_sort_conflicts_instance_number",
            "geometry/ct/nonuniform_slice_spacing",
            "geometry/ct/gantry_tilt_series",
            "geometry/ct/duplicate_missing_instance_number",
            "geometry/ct/multiseries_shared_frame_of_reference",
            "geometry_ct_spatial_sort_conflicts_instance_number",
            "slice_001",
        ] {
            assert!(!serialized.contains(historical));
        }
        let recipe: Value = serde_json::from_str(&serialized).unwrap();
        let expected = oracle();
        let artifacts = recipe["dicom"]["artifacts"].as_array().unwrap();
        assert_eq!(artifacts.len(), 6);
        for row in expected["caller"]["files"].as_array().unwrap() {
            assert!(artifacts.iter().any(|artifact| {
                artifact["logical_id"] == row["logical_id"]
                    && artifact["output"]["role"] == row["role"]
                    && artifact["output"]["path"] == row["path"]
            }));
        }
        assert_eq!(
            artifacts
                .iter()
                .map(|artifact| artifact["order"].as_u64().unwrap())
                .collect::<Vec<_>>(),
            vec![3, 0, 5, 1, 4, 2],
            "declaration order must remain distinct from caller artifact order"
        );
        Self {
            members: root.join("members"),
            descriptor: root.join("definition.json"),
            root,
        }
    }

    pub fn args(&self, command: &str, out: Option<&str>) -> Vec<String> {
        let mut args = vec![
            command.into(),
            "--corpus".into(),
            "./definition.json".into(),
            "--asset-root".into(),
            "members".into(),
            "--profile".into(),
            "extended".into(),
            "--case-id".into(),
            "caller/volumes/angled-order-study".into(),
            "--seed".into(),
            "13".into(),
            "--parallelism".into(),
            "3".into(),
        ];
        if let Some(out) = out {
            args.extend(["--out".into(), out.into()]);
        }
        args.extend(["--format".into(), "json".into()]);
        args
    }
}

impl Drop for GenericCtGeometryBundle {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}

pub fn assert_manifest(manifest: &Value) {
    let expected = oracle();
    assert_eq!(manifest["manifest_schema_version"], "2.0.0");
    assert_eq!(manifest["run"]["kind"], "external_corpus");
    assert_eq!(
        manifest["identity_projection"]["corpus_definition"]["identity"],
        expected["caller"]["identity"]
    );
    let files = manifest["files"].as_array().unwrap();
    assert_eq!(files.len(), 6);
    for (file, row) in files
        .iter()
        .zip(expected["caller"]["files"].as_array().unwrap())
    {
        assert_eq!(file["case_id"], expected["caller"]["case_id"]);
        assert_eq!(file["recipe"]["recipe_id"], expected["caller"]["recipe_id"]);
        for key in ["path", "size_bytes", "sha256"] {
            assert_eq!(file[key], row[key], "{key}");
        }
        assert_eq!(
            file["pixel_data"]["frame_hashes"][0],
            row["pixel_data_sha256"]
        );
        let geometry = &file["expected_geometry"];
        assert_eq!(
            geometry["geometric_order_index"],
            row["geometric_order_index"]
        );
        assert_eq!(
            geometry["instance_number_order_index"],
            row["instance_number_order_index"]
        );
        assert_eq!(geometry["adjacent_spacing_mm"], json!([4.0, 7.0]));
        assert_eq!(geometry["spacing_uniform"], false);
        assert_eq!(geometry["sorting_conflict_expected"], true);
        assert_eq!(geometry["gantry_detector_tilt_degrees"], 11.30993247);
        let organization = &file["expected_series_organization"];
        assert_eq!(organization["group_id"], "caller-shared-coordinate-space");
        assert_eq!(organization["study_series_count"], 2);
        assert_eq!(organization["series_instance_count"], 3);
        assert_eq!(organization["series_ordinal"], row["series_ordinal"]);
        for key in [
            "shared_study_instance_uid_expected",
            "shared_frame_of_reference_uid_expected",
            "distinct_series_instance_uids_expected",
        ] {
            assert_eq!(organization[key], true, "{key}");
        }
    }
}

pub fn assert_payload(path: &Path, row: &Value) {
    let bytes = fs::read(path).unwrap();
    assert_eq!(bytes.len() as u64, row["size_bytes"].as_u64().unwrap());
    assert_eq!(sha256_hex(&bytes), row["sha256"].as_str().unwrap());
}

fn assert_ct_report_projection(report: &Value) -> Result<(), String> {
    let rows = report["coverage_matrix"]
        .as_array()
        .ok_or("report2 coverage_matrix missing")?;
    let expected = oracle();
    let files = expected["caller"]["files"].as_array().unwrap();
    if rows.len() != files.len() {
        return Err("report2 must retain exactly six caller CT rows".into());
    }
    for (row, file) in rows.iter().zip(files) {
        for (field, expected) in [
            (
                "geometry_geometric_order_index",
                file["geometric_order_index"].clone(),
            ),
            (
                "geometry_instance_number_order_index",
                file["instance_number_order_index"].clone(),
            ),
            ("geometry_adjacent_spacing_mm", json!([4.0, 7.0])),
            ("geometry_spacing_uniform", json!(false)),
            ("geometry_gantry_detector_tilt_degrees", json!(11.30993247)),
            ("geometry_sorting_conflict_expected", json!(true)),
            (
                "series_organization_group_id",
                json!("caller-shared-coordinate-space"),
            ),
            ("study_series_count", json!(2)),
            ("series_organization_instance_count", json!(3)),
            ("series_ordinal", file["series_ordinal"].clone()),
            ("shared_study_instance_uid_expected", json!(true)),
            ("shared_frame_of_reference_uid_expected", json!(true)),
            ("distinct_series_instance_uids_expected", json!(true)),
            (
                "image_position_patient",
                file["image_position_patient"].clone(),
            ),
            ("image_orientation_patient", json!("1\\0\\0\\0\\1\\0")),
            ("pixel_spacing", json!("0.73\\0.41")),
            ("slice_thickness", json!("2.5")),
            ("spacing_between_slices", Value::Null),
        ] {
            if row[field] != expected {
                return Err(format!("report2 caller CT field mismatch: {field}"));
            }
        }
    }
    for (field, expected) in [
        ("profiles", json!({"extended":6})),
        ("profile_memberships", json!({"extended":6})),
        ("modalities", json!({"CT":6})),
        ("geometries", json!({"2x2":6})),
        ("bit_depths", json!({"12":6})),
        ("image_orientations_patient", json!({"1\\0\\0\\0\\1\\0":6})),
        ("pixel_spacings", json!({"0.73\\0.41":6})),
        ("slice_thicknesses", json!({"2.5":6})),
        ("spacing_between_slices", json!({})),
        ("ct_acquisition_numbers", json!({"4":3,"6":3})),
    ] {
        if report["grouped_coverage"][field] != expected {
            return Err(format!("report2 caller CT group mismatch: {field}"));
        }
    }
    Ok(())
}

pub fn assert_report(report: &Value) {
    assert_eq!(report["coverage_report_schema_version"], "2.0.0");
    assert_eq!(report["report_kind"], "external_corpus");
    assert_eq!(report["summary"]["emitted_files"], 6);
    assert_manifest(&report["source_manifest"]);
    assert_ct_report_projection(report).unwrap();
}

pub fn assert_report_mutations_fail(report: &Value) {
    for field in [
        "geometry_geometric_order_index",
        "geometry_instance_number_order_index",
        "geometry_adjacent_spacing_mm",
        "geometry_spacing_uniform",
        "geometry_gantry_detector_tilt_degrees",
        "geometry_sorting_conflict_expected",
        "series_organization_group_id",
        "study_series_count",
        "series_organization_instance_count",
        "series_ordinal",
        "shared_study_instance_uid_expected",
        "shared_frame_of_reference_uid_expected",
        "distinct_series_instance_uids_expected",
        "image_position_patient",
        "image_orientation_patient",
        "pixel_spacing",
        "slice_thickness",
        "spacing_between_slices",
    ] {
        let mut mutated = report.clone();
        mutated["coverage_matrix"][0][field] = json!("tampered");
        assert!(assert_ct_report_projection(&mutated).is_err(), "{field}");
    }
    for field in [
        "profiles",
        "profile_memberships",
        "modalities",
        "geometries",
        "bit_depths",
        "image_orientations_patient",
        "pixel_spacings",
        "slice_thicknesses",
        "spacing_between_slices",
        "ct_acquisition_numbers",
    ] {
        let mut mutated = report.clone();
        mutated["grouped_coverage"][field] = json!({"tampered":99});
        assert!(assert_ct_report_projection(&mutated).is_err(), "{field}");
    }
}
