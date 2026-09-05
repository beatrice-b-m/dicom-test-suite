use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};
use synth_dicom_gen::{sdk::CorpusSelector, sha256_hex};

pub fn oracle() -> Value {
    let bytes = include_bytes!("../fixtures/generic-us-multiframe-semantics.json");
    assert_eq!(
        sha256_hex(bytes),
        "fc114cd712ff5e54be05177d0ae4a4d95add7097ed82d9a5f45d182be340fd05",
        "the reviewed caller-owned US multiframe oracle is immutable"
    );
    let oracle: Value = serde_json::from_slice(bytes).unwrap();
    assert_eq!(oracle["oracle_version"], "1.0.0");
    assert_eq!(
        oracle["historical_source"]["sha256"],
        "6f97371d5746d00e10ddadbdf436a29717a7bc241f53993a5fa9bc21ea41206d"
    );
    oracle
}

pub fn selector() -> CorpusSelector {
    CorpusSelector::CaseIds {
        profile: "core".into(),
        include_stress: false,
        case_ids: vec![oracle()["caller"]["case_id"].as_str().unwrap().into()],
    }
}

pub struct GenericUsMultiframeBundle {
    pub root: PathBuf,
    pub members: PathBuf,
    pub descriptor: PathBuf,
}

impl GenericUsMultiframeBundle {
    pub fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = fs::canonicalize(std::env::temp_dir())
            .unwrap()
            .join(format!(
                "generic-us-multiframe-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
        fs::create_dir(&root).unwrap();
        let source = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/generic-us-multiframe-corpus");
        for relative in [
            "definition.json",
            "members/cases/registry.json",
            "members/cases/recipes/caller_ultrasound_cine.json",
        ] {
            let destination = root.join(relative);
            fs::create_dir_all(destination.parent().unwrap()).unwrap();
            fs::copy(source.join(relative), destination).unwrap();
        }
        let recipe: Value = serde_json::from_slice(
            &fs::read(root.join("members/cases/recipes/caller_ultrasound_cine.json")).unwrap(),
        )
        .unwrap();
        let expected = &oracle()["caller"];
        assert_eq!(
            recipe["dicom"]["artifacts"][0]["logical_id"],
            expected["logical_id"]
        );
        assert_eq!(
            recipe["dicom"]["artifacts"][0]["output"]["role"],
            expected["role"]
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
            "core".into(),
            "--case-id".into(),
            "caller/acquisition/cardiac-cine".into(),
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

impl Drop for GenericUsMultiframeBundle {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}

pub fn assert_manifest(manifest: &Value) {
    let expected = &oracle()["caller"];
    assert_eq!(manifest["manifest_schema_version"], "2.0.0");
    assert_eq!(manifest["run"]["kind"], "external_corpus");
    assert_eq!(manifest["files"].as_array().unwrap().len(), 1);
    let file = &manifest["files"][0];
    for key in [
        "case_id",
        "path",
        "size_bytes",
        "sha256",
        "image",
        "pixel_data",
        "expected_semantics",
        "expected_us_multiframe",
    ] {
        let oracle_key = if key == "path" { "output_path" } else { key };
        assert_eq!(file[key], expected[oracle_key], "{key}");
    }
    assert_eq!(file["recipe"]["recipe_id"], expected["recipe_id"]);
}

pub fn assert_payload(path: &Path) {
    let expected = &oracle()["caller"];
    let bytes = fs::read(path).unwrap();
    assert_eq!(bytes.len() as u64, expected["size_bytes"].as_u64().unwrap());
    assert_eq!(sha256_hex(&bytes), expected["sha256"].as_str().unwrap());
}

pub fn assert_report(report: &Value) {
    assert_eq!(report["coverage_report_schema_version"], "2.0.0");
    assert_eq!(report["report_kind"], "external_corpus");
    assert_eq!(report["summary"]["emitted_files"], 1);
    assert_manifest(&report["source_manifest"]);
    assert_eq!(
        report["artifact_dimensions"]["modalities"][0],
        json!({"count":1,"members":["independent/caller-cine.dcm"],"value":"\"US\""})
    );
}
