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
        "3efaeeced2c7603dc0b2764c0f6a255c3d3a4c36ad27df4667da6b985010373b",
        "the reviewed caller-owned nonsquare SC oracle is immutable"
    );
    let value: Value = serde_json::from_slice(bytes).unwrap();
    assert_eq!(value["oracle_version"], "1.0.0");
    assert_eq!(
        value["historical_source"]["part10_sha256"],
        json!([
            "50f897625dcc489d212a81674086d1183569d6e0ac7a847d55afc8dd599276d4",
            "dc330a2b51d1381d943e5ba0f50086114eb95102852228e7ffcb62e0bdec93b9"
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
}
