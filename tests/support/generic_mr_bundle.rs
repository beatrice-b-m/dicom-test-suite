use serde_json::{Value, json};
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};
use synth_dicom_gen::sdk::{CorpusSelector, DicomTestSuite, InspectCorpusRequest};

pub const CASE_ID: &str = "caller/volume/tilted-series";
pub const RECIPE_ID: &str = "caller_tilted_series";
pub const DICOM_PATHS: [&str; 3] = [
    "caller-output/tilted-a.dcm",
    "caller-output/tilted-b.dcm",
    "caller-output/tilted-c.dcm",
];

pub fn oracle() -> Value {
    static VERIFIED: std::sync::Once = std::sync::Once::new();
    VERIFIED.call_once(|| {
        let output = Command::new("python3")
            .args(["-c", "import hashlib,pathlib,sys;print(hashlib.sha256(pathlib.Path(sys.argv[1]).read_bytes()).hexdigest())"])
            .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/generic-mr-semantics.json"))
            .output().unwrap();
        assert!(output.status.success());
        assert_eq!(String::from_utf8(output.stdout).unwrap().trim(), "26acfc3d00e1e43496fb09ba325c5f194996b76baf36384c579d0277bb137138");
    });
    let value: Value =
        serde_json::from_slice(include_bytes!("../fixtures/generic-mr-semantics.json")).unwrap();
    assert_eq!(value["oracle_version"], "1.0.0");
    assert_eq!(
        value["source_receipt_sha256"],
        "cad5d8128468b853b3d3f3a1e9a3e5a31faec48a76ef155c26f0cfd7d6cb3186"
    );
    assert_eq!(
        value["source_case"],
        "classic/mr/multislice_oblique_explicit_le"
    );
    assert_eq!(value["caller_case"], CASE_ID);
    assert_eq!(value["recipe_id"], RECIPE_ID);
    assert_eq!(value["source_files"].as_array().unwrap().len(), 3);
    assert_eq!(value["caller_files"].as_array().unwrap().len(), 3);
    value
}

pub struct GenericMrBundle {
    pub root: PathBuf,
    pub members: PathBuf,
    pub descriptor: PathBuf,
    pub identity: Value,
}

impl GenericMrBundle {
    pub fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = fs::canonicalize(std::env::temp_dir())
            .unwrap()
            .join(format!(
                "generic-mr-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
        fs::create_dir(&root).unwrap();
        let source =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/generic-mr-corpus");
        let source_bytes =
            fs::read(source.join("members/cases/recipes/caller_tilted_series.json")).unwrap();
        let source_text = String::from_utf8(source_bytes).unwrap();
        assert!(!source_text.contains("classic/mr/multislice_oblique_explicit_le"));
        assert!(!source_text.contains("mr_multislice_oblique"));
        assert!(!source_text.contains("slice-001.dcm"));
        let recipe: Value = serde_json::from_str(&source_text).unwrap();
        assert_eq!(recipe["binding"]["case_id"], CASE_ID);
        assert_eq!(recipe["recipe_id"], RECIPE_ID);
        assert_eq!(recipe["planning_order"], 913);
        assert_eq!(recipe["projection_order"], 407);
        assert_eq!(
            recipe["provider_parameters"]["patient"]["patient_id"],
            "PUBLIC-MR-314"
        );
        assert_eq!(
            recipe["provider_parameters"]["equipment"]["manufacturer"],
            "Independent Synthetic Systems"
        );
        assert_eq!(
            recipe["provider_parameters"]["image_type"],
            json!(["DERIVED", "SECONDARY"])
        );
        let artifacts = recipe["dicom"]["artifacts"].as_array().unwrap();
        assert_eq!(artifacts.len(), 3);
        for (index, (artifact, (logical_id, role))) in artifacts
            .iter()
            .zip([
                ("volume_part_alpha", "acquisition_start"),
                ("volume_part_beta", "acquisition_middle"),
                ("volume_part_gamma", "acquisition_end"),
            ])
            .enumerate()
        {
            assert_eq!(artifact["logical_id"], logical_id);
            assert_eq!(artifact["output"]["role"], role);
            assert_eq!(artifact["output"]["path"], DICOM_PATHS[index]);
            assert_eq!(artifact["order"], index);
            assert_eq!(
                artifact["classic_projection"]["mr"]["repetition_time"],
                "750"
            );
        }
        let (files, dirs) = inventory(&source);
        assert_eq!(
            files,
            BTreeSet::from([
                "definition.json".into(),
                "members/cases/recipes/caller_tilted_series.json".into(),
                "members/cases/registry.json".into(),
            ])
        );
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
        let inspected = DicomTestSuite::embedded()
            .unwrap()
            .inspect_corpus(
                InspectCorpusRequest::from_file(&descriptor, &members)
                    .with_selection(selector())
                    .with_seed(1)
                    .with_parallelism(4),
            )
            .unwrap_or_else(|error| panic!("{}", error.diagnostic()));
        let identity = inspected.corpus_definition_identity().clone();
        Self {
            root,
            members,
            descriptor,
            identity,
        }
    }

    pub fn selection_args(&self, command: &str) -> Vec<String> {
        [
            command,
            "--corpus",
            "./definition.json",
            "--asset-root",
            "members",
            "--profile",
            "core",
            "--case-id",
            CASE_ID,
            "--seed",
            "1",
            "--parallelism",
            "4",
            "--format",
            "json",
        ]
        .map(str::to_owned)
        .to_vec()
    }

    pub fn assert_closure(&self, name: &str) {
        let (files, dirs) = inventory(&self.root.join(name));
        assert_eq!(
            files,
            BTreeSet::from([
                "manifest.json".into(),
                DICOM_PATHS[0].into(),
                DICOM_PATHS[1].into(),
                DICOM_PATHS[2].into(),
            ])
        );
        assert_eq!(dirs, BTreeSet::from(["caller-output".into()]));
    }
}

impl Drop for GenericMrBundle {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}

pub fn selector() -> CorpusSelector {
    CorpusSelector::CaseIds {
        profile: "core".into(),
        include_stress: false,
        case_ids: vec![CASE_ID.into()],
    }
}

fn inventory(root: &Path) -> (BTreeSet<String>, BTreeSet<String>) {
    fn walk(
        root: &Path,
        current: &Path,
        files: &mut BTreeSet<String>,
        dirs: &mut BTreeSet<String>,
    ) {
        for entry in fs::read_dir(current).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            let name = path
                .strip_prefix(root)
                .unwrap()
                .to_str()
                .unwrap()
                .to_owned();
            if entry.file_type().unwrap().is_dir() {
                dirs.insert(name);
                walk(root, &path, files, dirs);
            } else {
                files.insert(name);
            }
        }
    }
    let mut files = BTreeSet::new();
    let mut dirs = BTreeSet::new();
    walk(root, root, &mut files, &mut dirs);
    (files, dirs)
}

pub fn assert_identity(identity: &Value) {
    assert_eq!(
        *identity,
        json!({
            "schema_version": "1.0.0",
            "definition_id": "fixture.generic-mr",
            "definition_version": "1.0.0",
            "manifest_sha256": "69c43f9b81e1d235dd09d477834e0f29635248d536b3bd365037623763862504",
            "corpus_definition_sha256": "8b1ed53df47f91f911eb98043b7a3fe78b7db20ecc9be2c6658c04341fe235f0",
            "file_count": 3,
            "total_size_bytes": 10103
        })
    );
}

pub fn assert_assessment(value: &Value) {
    let assessment = &value["loaded_corpus"]["assessment"];
    assert_eq!(assessment["publication"], "not_run");
    assert_eq!(assessment["validation"], "not_run");
    assert_eq!(assessment["parallelism"], 4);
    assert_eq!(
        assessment["artifact_ids"],
        json!([
            "curated_caller_tilted_series_volume_part_alpha",
            "curated_caller_tilted_series_volume_part_beta",
            "curated_caller_tilted_series_volume_part_gamma"
        ])
    );
}

pub fn assert_manifest(manifest: &Value, identity: &Value) {
    assert_eq!(manifest["manifest_schema_version"], "2.0.0");
    assert_eq!(
        manifest["run"],
        json!({"kind":"external_corpus","profile":"core","seed":1,"include_stress":false,"selector":{"kind":"case_ids","case_ids":[CASE_ID]}})
    );
    assert_eq!(
        manifest["identity_projection"]["corpus_definition"],
        json!({"state":"verified_bundle","identity":identity})
    );
    let expected = oracle();
    let rows = expected["caller_files"].as_array().unwrap();
    let files = manifest["files"].as_array().unwrap();
    assert_eq!(files.len(), 3);
    let mut sop_uids = BTreeSet::new();
    for (file, row) in files.iter().zip(rows) {
        assert_eq!(file["case_id"], CASE_ID);
        assert_eq!(file["path"], row["path"]);
        assert_eq!(file["size_bytes"], row["size_bytes"]);
        assert_eq!(file["sha256"], row["sha256"]);
        assert_eq!(file["recipe"]["recipe_id"], RECIPE_ID);
        assert_eq!(file["recipe"]["recipe_version"], "2.3.0");
        assert_eq!(file["dicom"]["modality"], "MR");
        assert_eq!(file["dicom"]["sop_class_uid"], "1.2.840.10008.5.1.4.1.1.4");
        assert_eq!(file["dicom"]["transfer_syntax_uid"], "1.2.840.10008.1.2.1");
        assert_eq!(file["determinism"], "byte_stable");
        assert_eq!(file["validation"]["status"], "passed");
        assert!(
            file["validation"]["internal"]
                .as_array()
                .unwrap()
                .iter()
                .all(|c| c["status"] == "passed")
        );
        assert_eq!(
            file["expected_semantics"]["image_type"],
            "DERIVED\\SECONDARY"
        );
        assert_eq!(file["expected_semantics"]["series_instance_count"], 3);
        assert_eq!(
            file["expected_semantics"]["shared_study_series_frame_of_reference"],
            true
        );
        assert_eq!(
            file["expected_semantics"]["geometry_sort_key"]["image_orientation_patient"],
            "0.6\\0.8\\0\\0\\0\\1"
        );
        assert_eq!(
            file["expected_semantics"]["geometry_sort_key"]["position_along_normal"],
            row["position_along_normal"]
        );
        assert_eq!(
            file["expected_semantics"]["geometry_sort_key"]["slice_order_index"],
            row["slice_order_index"]
        );
        assert_eq!(file["expected_semantics"]["pixel_min"], row["pixel_min"]);
        assert_eq!(file["expected_semantics"]["pixel_max"], row["pixel_max"]);
        assert_eq!(
            file["pixel_data"]["frame_hashes"],
            json!([row["frame_sha256"]])
        );
        for key in [
            "study_instance_uid",
            "series_instance_uid",
            "frame_of_reference_uid",
        ] {
            assert_eq!(file["uids"][key], expected["shared_uids"][key]);
        }
        assert_eq!(file["uids"]["sop_instance_uid"], row["sop_instance_uid"]);
        sop_uids.insert(file["uids"]["sop_instance_uid"].as_str().unwrap());
    }
    assert_eq!(sop_uids.len(), 3);
    assert_eq!(manifest["selection_ledger"].as_array().unwrap().len(), 1);
    let ledger = &manifest["selection_ledger"][0];
    assert_eq!(ledger["case_id"], CASE_ID);
    assert_eq!(ledger["selection"], "direct");
    assert_eq!(ledger["outcome"], "generated");
    assert_eq!(ledger["dependency_case_ids"], json!([]));
    assert_eq!(ledger["artifact_paths"], json!(DICOM_PATHS));
}

pub fn assert_payload(path: &Path, size: &Value, sha: &Value) {
    assert_eq!(fs::metadata(path).unwrap().len(), size.as_u64().unwrap());
    let output = Command::new("python3")
        .args(["-c", "import hashlib,pathlib,sys;print(hashlib.sha256(pathlib.Path(sys.argv[1]).read_bytes()).hexdigest())"])
        .arg(path)
        .output().unwrap();
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap().trim(),
        sha.as_str().unwrap()
    );
}
