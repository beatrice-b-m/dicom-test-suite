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
            .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/generic-pet-semantics.json"))
            .output().unwrap();
        assert!(output.status.success());
        assert_eq!(String::from_utf8(output.stdout).unwrap().trim(), "c78f2a5e709fe91e9fb1fc42c3dd9b39623760a53399913af4a095150627c855");
    });
    let value: Value =
        serde_json::from_slice(include_bytes!("../fixtures/generic-pet-semantics.json")).unwrap();
    assert_eq!(value["oracle_version"], "1.0.0");
    assert_eq!(
        value["source_receipt_sha256"],
        "0aec8cb252130433ba4626bf9c46d219f088e95fe5e3c7c3de958d2acd57c164"
    );
    assert_eq!(
        value["parity_receipt_sha256"],
        "6bb4c99b48bb97baa7d6c9b4eccc20a627017800f1397761a9188a0c68190a7d"
    );
    assert_eq!(value["cases"].as_array().unwrap().len(), 1);
    let row = &value["cases"][0];
    assert_eq!(
        row["source_case"],
        "classic/pet/rescaled_activity_explicit_le"
    );
    assert_eq!(row["caller_case"], "caller/acquisition/activity");
    assert_eq!(row["recipe_id"], "caller_activity");
    assert_eq!(row["output_path"], "independent/activity.dcm");
    assert_eq!(row["profile"], "core");
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
pub struct GenericPetBundle {
    pub root: PathBuf,
    pub members: PathBuf,
    pub descriptor: PathBuf,
    pub identity: Value,
}
impl GenericPetBundle {
    pub fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = fs::canonicalize(std::env::temp_dir())
            .unwrap()
            .join(format!(
                "generic-pet-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
        fs::create_dir(&root).unwrap();
        let source =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/generic-pet-corpus");
        let expected = oracle();
        let rows = expected["cases"].as_array().unwrap();
        assert_eq!(rows.len(), 1);
        let mut members_expected = BTreeSet::from([
            "definition.json".to_owned(),
            "members/cases/registry.json".to_owned(),
            "members/evidence/phase-2-pet-rescaled-activity.md".to_owned(),
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
                "members/evidence".into()
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
        assert_eq!(registry["cases"].as_array().unwrap().len(), 1);
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
            assert_eq!(recipe["projection_order"], 901 + position);
            let artifact = &recipe["dicom"]["artifacts"][0];
            assert_eq!(recipe["dicom"]["artifacts"].as_array().unwrap().len(), 1);
            assert_eq!(artifact["logical_id"], "instance");
            assert_eq!(artifact["order"], 0);
            assert_eq!(artifact["output"]["role"], "primary");
            assert_eq!(artifact["template"]["template_id"], "classic/pet");

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
            json!({"corpus_definition_sha256":"a63baf6de99ba21ae32e02fb5b40562567f8cdd014b7fb29858365edbaab7ac3","definition_id":"fixture.generic-pet","definition_version":"1.0.0","file_count":4,"manifest_sha256":"f188d23526cec4137932162011320d2204610070cc7174579d9ed2c720285a27","schema_version":"1.0.0","total_size_bytes":14416})
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
impl Drop for GenericPetBundle {
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
        "expected_pet_activity",
    ] {
        assert_eq!(file[key], row[key], "preserved PET field {key}");
    }
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
    assert_eq!(files.len(), 1);
    for (file, row) in files.iter().zip(rows) {
        assert_eq!(file["size_bytes"], row["caller_size_bytes"]);
        assert_eq!(file["sha256"], row["caller_sha256"]);
        assert_eq!(file["dicom"]["modality"], "PT");
        assert_eq!(
            file["dicom"]["sop_class_uid"],
            "1.2.840.10008.5.1.4.1.1.128"
        );
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
    assert_eq!(ledger.len(), 1);
    for (entry, id) in ledger.iter().zip(selector_ids()) {
        let row = rows.iter().find(|r| r["caller_case"] == id).unwrap();
        assert_eq!(entry["case_id"], id);
        assert_eq!(entry["selection"], "direct");
        assert_eq!(entry["outcome"], "generated");
        assert_eq!(entry["dependency_case_ids"], json!([]));
        assert_eq!(entry["artifact_paths"], json!([row["output_path"]]));
    }
    println!(
        "caller PET payloads: {}",
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
    let output=Command::new("python3").args(["-c","import hashlib,pathlib,sys;print(hashlib.sha256(pathlib.Path(sys.argv[1]).read_bytes()).hexdigest())"]).arg(path).output().unwrap();
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap().trim(),
        sha.as_str().unwrap()
    );
}
