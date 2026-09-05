use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};
use synth_dicom_gen::{sdk::CorpusSelector, sha256_hex};

pub fn oracle() -> Value {
    let bytes = include_bytes!("../fixtures/generic-nonsquare-sc-semantics.json");
    assert_eq!(
        sha256_hex(bytes),
        "853ec2f7aab25684ee0394eab8bfe221a72943cd80996b0c3f3232828c414b02",
        "the reviewed caller-owned nonsquare SC oracle is immutable"
    );
    let value: Value = serde_json::from_slice(bytes).unwrap();
    assert_eq!(value["oracle_version"], "1.1.0");
    assert_eq!(
        value["current_embedded_seed7_regression"]["part10_sha256"],
        json!([
            "50f897625dcc489d212a81674086d1183569d6e0ac7a847d55afc8dd599276d4",
            "dc330a2b51d1381d943e5ba0f50086114eb95102852228e7ffcb62e0bdec93b9"
        ])
    );
    assert_eq!(value["current_embedded_seed7_regression"]["seed"], 7);
    assert_eq!(value["accepted_original_pin_baseline2"]["seed"], 1);
    assert_eq!(
        value["accepted_original_pin_baseline2"]["source_revision"],
        "232b9de41f97ee95abe1ecc40b6b8b70ebeeea5f"
    );
    assert_eq!(
        value["accepted_original_pin_baseline2"]["receipt_sha256"],
        "1bbf330ba59bc4164cfa71d3fce9c86394046352eb26293a61f8a3bc115903ef"
    );
    assert_eq!(
        value["accepted_original_pin_baseline2"]["artifacts"],
        json!([
            {
                "path":"classic/sc/nonsquare_pixel_spacing/pixel-spacing.dcm",
                "size_bytes":1010,
                "part10_sha256":"f66374f55860fe732345c7a0faebe3cd142647321ce7c13eba24a8ba1b58fb14"
            },
            {
                "path":"classic/sc/nonsquare_pixel_spacing/pixel-aspect-ratio.dcm",
                "size_bytes":988,
                "part10_sha256":"3e66422f5c55d68b60f169f4fb6a5e42cf43a9c904c72da3132dd854f8ae6da6"
            }
        ])
    );
    value
}

pub fn selector() -> CorpusSelector {
    CorpusSelector::CaseIds {
        profile: "core".into(),
        include_stress: false,
        case_ids: vec![oracle()["caller"]["case_id"].as_str().unwrap().into()],
    }
}

pub struct GenericNonsquareScBundle {
    pub root: PathBuf,
    pub members: PathBuf,
    pub descriptor: PathBuf,
}

impl GenericNonsquareScBundle {
    pub fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = fs::canonicalize(std::env::temp_dir())
            .unwrap()
            .join(format!(
                "generic-nonsquare-sc-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
        fs::create_dir(&root).unwrap();
        let source = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/generic-nonsquare-sc-corpus");
        for relative in [
            "definition.json",
            "members/cases/registry.json",
            "members/cases/recipes/caller_independent_rectangles.json",
        ] {
            let destination = root.join(relative);
            fs::create_dir_all(destination.parent().unwrap()).unwrap();
            fs::copy(source.join(relative), destination).unwrap();
        }
        let recipe: Value = serde_json::from_slice(
            &fs::read(root.join("members/cases/recipes/caller_independent_rectangles.json"))
                .unwrap(),
        )
        .unwrap();
        let files = oracle()["caller"]["files"].as_array().unwrap().clone();
        for (artifact, expected) in recipe["dicom"]["artifacts"]
            .as_array()
            .unwrap()
            .iter()
            .zip(files)
        {
            assert_eq!(artifact["logical_id"], expected["logical_id"]);
            assert_eq!(artifact["output"]["role"], expected["role"]);
            assert_eq!(artifact["output"]["path"], expected["path"]);
        }
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
            "core".into(),
            "--case-id".into(),
            "caller/geometry/independent-rectangles".into(),
            "--seed".into(),
            "1".into(),
            "--parallelism".into(),
            "4".into(),
        ];
        if let Some(out) = out {
            args.extend(["--out".into(), out.into()]);
        }
        args.extend(["--format".into(), "json".into()]);
        args
    }
}

impl Drop for GenericNonsquareScBundle {
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
    assert_eq!(files.len(), 2);
    for (file, row) in files
        .iter()
        .zip(expected["caller"]["files"].as_array().unwrap())
    {
        assert_eq!(file["case_id"], expected["caller"]["case_id"]);
        assert_eq!(file["recipe"]["recipe_id"], expected["caller"]["recipe_id"]);
        for key in ["path", "size_bytes", "sha256"] {
            assert_eq!(file[key], row[key], "{key}");
        }
        let contract = &file["expected_nonsquare_spacing"];
        assert_eq!(contract["variant_id"], row["variant_id"]);
        assert_eq!(contract["pixel_data_sha256"], row["pixel_data_sha256"]);
        assert_eq!(contract["uncalibrated"], true);
        assert_eq!(contract["patient_space_geometry_present"], false);
        let axis = if row["variant_id"] == "pixel_spacing" {
            "pixel_spacing"
        } else {
            "pixel_aspect_ratio"
        };
        assert_eq!(contract[axis]["lexical_value"], row["lexical_value"]);
    }
}

pub fn assert_payload(path: &Path, row: &Value) {
    let bytes = fs::read(path).unwrap();
    assert_eq!(bytes.len() as u64, row["size_bytes"].as_u64().unwrap());
    assert_eq!(sha256_hex(&bytes), row["sha256"].as_str().unwrap());
}

pub fn assert_report(report: &Value) {
    assert_eq!(report["coverage_report_schema_version"], "2.0.0");
    assert_eq!(report["report_kind"], "external_corpus");
    assert_eq!(report["summary"]["emitted_files"], 2);
    assert_manifest(&report["source_manifest"]);
    let files = report["source_manifest"]["files"].as_array().unwrap();
    assert_eq!(files.len(), 2, "report2 must retain both nonsquare rows");
    assert!(
        files.iter().all(|file| {
            file["expected_nonsquare_spacing"]
                .as_object()
                .is_some_and(|contract| contract.len() == 7)
        }),
        "each report2 row must retain all seven nonsquare axes"
    );
    assert_eq!(
        files[0]["expected_nonsquare_spacing"],
        json!({
            "variant_id":"pixel_spacing",
            "pixel_spacing":{
                "keyword":"PixelSpacing","tag":"0028,0030","vr":"DS","vm":2,
                "lexical_value":"1.2\\0.6","row_spacing_mm":1.2,"column_spacing_mm":0.6
            },
            "nominal_scanned_pixel_spacing":{
                "keyword":"NominalScannedPixelSpacing","tag":"0018,2010","vr":"DS","vm":2,
                "lexical_value":"1.2\\0.6","row_spacing_mm":1.2,"column_spacing_mm":0.6
            },
            "pixel_aspect_ratio":null,
            "uncalibrated":true,
            "patient_space_geometry_present":false,
            "pixel_data_sha256":"fff3a9bcdd37363d703c1c4f9512533686157868f0d4f16a0f02d0f1da24f9a2"
        })
    );
    assert_eq!(
        files[1]["expected_nonsquare_spacing"],
        json!({
            "variant_id":"pixel_aspect_ratio",
            "pixel_spacing":null,
            "nominal_scanned_pixel_spacing":null,
            "pixel_aspect_ratio":{
                "keyword":"PixelAspectRatio","tag":"0028,0034","vr":"IS","vm":2,
                "lexical_value":"6\\3","vertical_extent":6,"horizontal_extent":3
            },
            "uncalibrated":true,
            "patient_space_geometry_present":false,
            "pixel_data_sha256":"fabff30883aac31048a8a5ac6a2eeb7c421b9f0dc1f2221e87d5df15f403bac7"
        })
    );
    let paths = json!([
        "caller-space/display-ratio.dcm",
        "caller-space/measured-grid.dcm"
    ]);
    assert_eq!(
        report["artifact_dimensions"]["providers"],
        json!([{"count":2,"members":paths.clone(),"value":"null"}]),
        "external artifact provider projection must remain the literal null group, not an empty group"
    );
    for (axis, value) in [
        ("determinism", "\"byte_stable\""),
        ("modalities", "\"OT\""),
        ("profiles", "core"),
        ("sop_classes", "\"1.2.840.10008.5.1.4.1.1.7\""),
        ("transfer_syntaxes", "\"1.2.840.10008.1.2.1\""),
    ] {
        assert_eq!(
            report["artifact_dimensions"][axis],
            json!([{"count":2,"members":paths.clone(),"value":value}]),
            "artifact grouping axis {axis}"
        );
    }
}
