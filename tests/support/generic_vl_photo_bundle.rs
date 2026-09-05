use serde_json::{Value, json};
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};
use synth_dicom_gen::sdk::{CorpusSelector, DicomTestSuite, InspectCorpusRequest};

pub fn oracle() -> Value {
    static VERIFIED: std::sync::Once = std::sync::Once::new();
    VERIFIED.call_once(|| {
        let output = Command::new("python3")
            .args(["-c", "import hashlib,pathlib,sys;print(hashlib.sha256(pathlib.Path(sys.argv[1]).read_bytes()).hexdigest())"])
            .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/generic-vl-photo-semantics.json"))
            .output().unwrap();
        assert!(output.status.success());
        assert_eq!(String::from_utf8(output.stdout).unwrap().trim(), "7c392a41768638be10e1cb4fb432a96f7d066d84c672111b9164f7e9c40b0165");
    });
    let value: Value = serde_json::from_slice(include_bytes!(
        "../fixtures/generic-vl-photo-semantics.json"
    ))
    .unwrap();
    assert_eq!(value["oracle_version"], "1.0.0");
    assert_eq!(
        value["source_receipt_sha256"],
        "6ba6acb4251c3c97bcab36762e5dd68d3ab7cca1492fb24fcf11f5243d2d32c1"
    );
    assert_eq!(
        value["parity_receipt_sha256"],
        "af400bbd6097d7eb51eb84739b34841769d83e2c2d7f466196fa41c8b120b68c"
    );
    assert_eq!(value["cases"].as_array().unwrap().len(), 2);
    for (row, name) in value["cases"]
        .as_array()
        .unwrap()
        .iter()
        .zip(["rgb", "palette"])
    {
        assert_eq!(row["caller_case"], format!("caller/photo/{name}"));
        assert_eq!(row["recipe_id"], format!("caller_photo_{name}"));
        assert_eq!(row["output_path"], format!("independent/{name}.dcm"));
        assert_eq!(row["profile"], "core");
    }
    value
}
pub fn selector_ids() -> Vec<String> {
    let mut ids = oracle()["cases"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["caller_case"].as_str().unwrap().to_owned())
        .collect::<Vec<_>>();
    ids.sort();
    ids
}
pub fn selector() -> CorpusSelector {
    CorpusSelector::CaseIds {
        profile: "core".into(),
        include_stress: false,
        case_ids: selector_ids(),
    }
}
pub struct GenericVlPhotoBundle {
    pub root: PathBuf,
    pub members: PathBuf,
    pub descriptor: PathBuf,
    pub identity: Value,
}
impl GenericVlPhotoBundle {
    pub fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = fs::canonicalize(std::env::temp_dir())
            .unwrap()
            .join(format!(
                "generic-vl-photo-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
        fs::create_dir(&root).unwrap();
        let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/generic-vl-photo-corpus");
        let expected = oracle();
        let rows = expected["cases"].as_array().unwrap();
        assert_eq!(rows.len(), 2);
        let mut members_expected = BTreeSet::from([
            "definition.json".to_owned(),
            "members/cases/registry.json".to_owned(),
        ]);
        for row in rows {
            members_expected.insert(format!(
                "members/cases/recipes/{}.json",
                row["recipe_id"].as_str().unwrap()
            ));
        }
        let (files, dirs) = inventory(&source);
        assert_eq!(files, members_expected);
        assert_eq!(
            dirs,
            BTreeSet::from([
                "members".into(),
                "members/cases".into(),
                "members/cases/recipes".into(),
            ])
        );
        for name in files {
            let destination = root.join(&name);
            fs::create_dir_all(destination.parent().unwrap()).unwrap();
            fs::write(destination, fs::read(source.join(&name)).unwrap()).unwrap();
        }
        let members = root.join("members");
        let descriptor = root.join("definition.json");
        let registry: Value =
            serde_json::from_slice(&fs::read(members.join("cases/registry.json")).unwrap())
                .unwrap();
        // Preserve core membership while exercising a neutral caller identity.
        assert_eq!(registry["cases"].as_array().unwrap().len(), 2);
        for (position, row) in rows.iter().enumerate() {
            let r = &registry["cases"][position];
            assert_eq!(r["case_id"], row["caller_case"]);
            assert_eq!(r["recipe_id"], row["recipe_id"]);
            assert_eq!(r["profiles"], json!(["core"]));
            let recipe: Value = serde_json::from_slice(
                &fs::read(members.join(format!(
                    "cases/recipes/{}.json",
                    row["recipe_id"].as_str().unwrap()
                )))
                .unwrap(),
            )
            .unwrap();
            assert_eq!(recipe["binding"]["case_id"], row["caller_case"]);
            assert_eq!(recipe["recipe_id"], row["recipe_id"]);
            assert_eq!(recipe["planning_order"], 900 + position);
            assert_eq!(recipe["projection_order"], 902 + position);
            let artifact = &recipe["dicom"]["artifacts"][0];
            assert_eq!(recipe["dicom"]["artifacts"].as_array().unwrap().len(), 1);
            assert_eq!(artifact["logical_id"], "instance");
            assert_eq!(artifact["order"], 0);
            assert_eq!(artifact["output"]["role"], "primary_1");
            assert_eq!(artifact["template"]["template_id"], "vl/photographic");

            assert_eq!(
                recipe["dicom"]["artifacts"][0]["output"]["path"],
                row["output_path"]
            );
        }
        let inspected = DicomTestSuite::embedded()
            .unwrap()
            .inspect_corpus(
                InspectCorpusRequest::from_file(&descriptor, &members)
                    .with_selection(selector())
                    .with_seed(1)
                    .with_parallelism(4),
            )
            .unwrap();
        let identity = inspected.corpus_definition_identity().clone();
        assert_eq!(
            identity,
            json!({"corpus_definition_sha256":"133a2312df485a2f3875af4b7484c81325d3d76bd1291231180e2ce8b23d6421","definition_id":"fixture.generic-vl-photo","definition_version":"1.0.0","file_count":4,"manifest_sha256":"c37e4fdfd60e5da7efd0724ba2d1bab58641bc6ae9c116f2a1b5debe6d9bb53e","schema_version":"1.0.0","total_size_bytes":17347})
        );
        Self {
            root,
            members,
            descriptor,
            identity,
        }
    }
    pub fn selection_args(&self, command: &str) -> Vec<String> {
        let mut args = [
            command,
            "--corpus",
            "./definition.json",
            "--asset-root",
            "members",
            "--profile",
            "core",
            "--seed",
            "1",
            "--parallelism",
            "4",
            "--format",
            "json",
        ]
        .map(str::to_owned)
        .to_vec();
        for id in selector_ids() {
            args.extend(["--case-id".into(), id]);
        }
        args
    }
    pub fn assert_closure(&self, name: &str) {
        let (files, dirs) = inventory(&self.root.join(name));
        let mut expected = BTreeSet::from(["manifest.json".into()]);
        for row in oracle()["cases"].as_array().unwrap() {
            expected.insert(row["output_path"].as_str().unwrap().to_owned());
        }
        assert_eq!(files, expected);
        assert_eq!(dirs, BTreeSet::from(["independent".into()]));
    }
}
impl Drop for GenericVlPhotoBundle {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}
fn inventory(root: &Path) -> (BTreeSet<String>, BTreeSet<String>) {
    fn walk(
        root: &Path,
        current: &Path,
        files: &mut BTreeSet<String>,
        dirs: &mut BTreeSet<String>,
    ) {
        for e in fs::read_dir(current).unwrap() {
            let e = e.unwrap();
            let path = e.path();
            let name = path
                .strip_prefix(root)
                .unwrap()
                .to_str()
                .unwrap()
                .to_owned();
            let kind = e.file_type().unwrap();
            if kind.is_dir() {
                dirs.insert(name);
                walk(root, &path, files, dirs);
            } else {
                assert!(kind.is_file());
                files.insert(name);
            }
        }
    }
    let mut files = BTreeSet::new();
    let mut dirs = BTreeSet::new();
    walk(root, root, &mut files, &mut dirs);
    (files, dirs)
}
pub fn assert_semantics(file: &Value, row: &Value) {
    for key in [
        "image",
        "pixel_data",
        "expected_semantics",
        "expected_capabilities",
        "expected_visual_checks",
        "known_stressors",
        "references",
        "validation",
        "standards_evidence",
    ] {
        assert_eq!(file[key], row[key], "preserved VL photographic field {key}");
    }
    assert_eq!(
        file["uids"]["implementation_version_name"],
        row["implementation_version_name"]
    );
    assert_eq!(
        file["recipe"]["recipe_parameters"],
        row["recipe_parameters"]
    );
}
pub fn assert_manifest(manifest: &Value, identity: &Value) {
    assert_eq!(manifest["manifest_schema_version"], "2.0.0");
    assert_eq!(
        manifest["run"],
        json!({"kind":"external_corpus","profile":"core","seed":1,"include_stress":false,"selector":{"kind":"case_ids","case_ids":selector_ids()}})
    );
    assert_eq!(
        manifest["identity_projection"]["corpus_definition"],
        json!({"state":"verified_bundle","identity":identity})
    );
    let source = oracle();
    let rows = source["cases"].as_array().unwrap();
    let files = manifest["files"].as_array().unwrap();
    assert_eq!(files.len(), 2);
    for (file, row) in files.iter().zip(rows) {
        assert_eq!(file["size_bytes"], row["caller_size_bytes"]);
        assert_eq!(file["sha256"], row["caller_sha256"]);
        assert_eq!(file["dicom"]["modality"], row["modality"]);
        assert_eq!(file["dicom"]["sop_class_uid"], row["sop_class_uid"]);
        assert_eq!(file["case_id"], row["caller_case"]);
        assert_eq!(file["path"], row["output_path"]);
        assert_eq!(file["recipe"]["recipe_id"], row["recipe_id"]);
        assert_eq!(file["determinism"], "byte_stable");
        assert_eq!(file["validation"]["status"], "passed");
        assert!(
            file["validation"]["internal"]
                .as_array()
                .unwrap()
                .iter()
                .all(|c| c["status"] == "passed")
        );
        assert_semantics(file, row);
    }
    let ledger = manifest["selection_ledger"].as_array().unwrap();
    assert_eq!(ledger.len(), 2);
    for (entry, id) in ledger.iter().zip(selector_ids()) {
        let row = rows.iter().find(|r| r["caller_case"] == id).unwrap();
        assert_eq!(entry["case_id"], id);
        assert_eq!(entry["selection"], "direct");
        assert_eq!(entry["outcome"], "generated");
        assert_eq!(entry["dependency_case_ids"], json!([]));
        assert_eq!(entry["artifact_paths"], json!([row["output_path"]]));
    }
    println!(
        "caller VL photographic payloads: {}",
        json!(
            files
                .iter()
                .map(
                    |f| json!({"path":f["path"],"size_bytes":f["size_bytes"],"sha256":f["sha256"]})
                )
                .collect::<Vec<_>>()
        )
    );
}
pub fn assert_payload(path: &Path, size: &Value, sha: &Value) {
    assert_eq!(fs::metadata(path).unwrap().len(), size.as_u64().unwrap());
    let bytes = fs::read(path).unwrap();
    let empty_sq = [
        0x40, 0x00, 0x55, 0x05, b'S', b'Q', 0, 0, 255, 255, 255, 255, 254, 255, 221, 224, 0, 0, 0,
        0,
    ];
    assert_eq!(
        bytes
            .windows(empty_sq.len())
            .filter(|w| *w == empty_sq)
            .count(),
        1
    );
    assert!(bytes.windows(10).any(|w| w == b"DICOMTS010"));
    let output=Command::new("python3").args(["-c","import hashlib,pathlib,sys;print(hashlib.sha256(pathlib.Path(sys.argv[1]).read_bytes()).hexdigest())"]).arg(path).output().unwrap();
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap().trim(),
        sha.as_str().unwrap()
    );
}
