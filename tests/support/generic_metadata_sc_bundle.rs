use serde_json::{Value, json};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
use synth_dicom_gen::sdk::{CorpusSelector, DicomTestSuite, InspectCorpusRequest};

pub const CASE_IDS: [&str; 3] = [
    "caller/metadata/empty",
    "caller/metadata/name",
    "caller/metadata/private",
];
pub const DICOM_PATHS: [&str; 3] = [
    "independent/empty.dcm",
    "independent/name.dcm",
    "independent/private.dcm",
];
pub const ORIGINAL_IDS: [&str; 3] = [
    "metadata/sc/utf8_person_name",
    "metadata/sc/empty_type2_attributes",
    "metadata/sc/private_creator_blocks",
];
pub struct GenericMetadataScBundle {
    pub root: PathBuf,
    pub members: PathBuf,
    pub descriptor: PathBuf,
    pub identity: Value,
}
impl GenericMetadataScBundle {
    pub fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = fs::canonicalize(std::env::temp_dir())
            .unwrap()
            .join(format!(
                "generic-metadata-sc-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
        fs::create_dir(&root).unwrap();
        let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/generic-metadata-sc-corpus");
        assert_fixture(&source);
        let members = root.join("members");
        fs::create_dir_all(members.join("cases/recipes")).unwrap();
        let descriptor_raw = fs::read(source.join("definition.json")).unwrap();
        let definition: Value = serde_json::from_slice(&descriptor_raw).unwrap();
        fs::write(root.join("definition.json"), descriptor_raw).unwrap();
        let member_paths = std::iter::once(definition["registry"]["path"].as_str().unwrap())
            .chain(
                definition["cases"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|case| case["recipe"]["path"].as_str().unwrap()),
            )
            .chain(
                definition["evidence"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|note| note["path"].as_str().unwrap()),
            );
        for name in member_paths {
            let destination = members.join(name);
            fs::create_dir_all(destination.parent().unwrap()).unwrap();
            fs::write(
                destination,
                fs::read(source.join("members").join(name)).unwrap(),
            )
            .unwrap();
        }
        let descriptor = root.join("definition.json");
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
            json!({
                "schema_version":"1.0.0", "definition_id":"fixture.generic-metadata-sc", "definition_version":"1.0.0",
                "manifest_sha256":"8e06f07cdc80da927d185607fffa9ab5b0b2cb972f6dd5ab559a1a12d5002be7",
                "corpus_definition_sha256":"7ab6abe3e65dc9c950fa56d4b0fbbdb931d9c33e75a9fe8cbfa25f0ab58d65f9",
                "file_count":8, "total_size_bytes":28914
            })
        );
        Self {
            root,
            members,
            descriptor,
            identity,
        }
    }
    pub fn assert_closure(&self, name: &str) {
        let root = self.root.join(name);
        let mut top = fs::read_dir(&root)
            .unwrap()
            .map(|x| x.unwrap().file_name().into_string().unwrap())
            .collect::<Vec<_>>();
        top.sort();
        assert_eq!(top, ["independent", "manifest.json"]);
        let mut files = fs::read_dir(root.join("independent"))
            .unwrap()
            .map(|x| {
                let x = x.unwrap();
                assert!(x.file_type().unwrap().is_file());
                x.file_name().into_string().unwrap()
            })
            .collect::<Vec<_>>();
        files.sort();
        assert_eq!(files, ["empty.dcm", "name.dcm", "private.dcm"]);
    }
}
impl Drop for GenericMetadataScBundle {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}
pub fn selector() -> CorpusSelector {
    CorpusSelector::CaseIds {
        profile: "core".into(),
        include_stress: false,
        case_ids: CASE_IDS.map(String::from).to_vec(),
    }
}
pub fn assert_manifest(manifest: &Value, identity: &Value) {
    assert_eq!(manifest["manifest_schema_version"], "2.0.0");
    assert_eq!(manifest["run"]["kind"], "external_corpus");
    assert_eq!(manifest["run"]["profile"], "core");
    assert_eq!(manifest["run"]["seed"], 1);
    assert_eq!(manifest["run"]["include_stress"], false);
    assert_eq!(
        manifest["run"]["selector"],
        json!({"kind":"case_ids", "case_ids":CASE_IDS})
    );
    assert_eq!(
        manifest["identity_projection"]["corpus_definition"],
        json!({"state":"verified_bundle", "identity":identity})
    );
    let ledger = manifest["selection_ledger"].as_array().unwrap();
    assert_eq!(ledger.len(), 3);
    for (i, row) in ledger.iter().enumerate() {
        assert_eq!(row["case_id"], CASE_IDS[i]);
        assert_eq!(row["selection"], "direct");
        assert_eq!(row["outcome"], "generated");
        assert_eq!(row["dependency_case_ids"], json!([]));
        assert_eq!(row["artifact_paths"], json!([DICOM_PATHS[i]]));
    }
    let files = manifest["files"].as_array().unwrap();
    assert_eq!(files.len(), 3);
    let payloads = [
        (
            962,
            "16f828849a97a809fed56fad7d04c5c1b7fffea071809ea5ec8cbd8e848065aa",
        ),
        (
            910,
            "51df3b4077125281dde05747812a83574160b343a9da22974973339b9777731f",
        ),
        (
            1094,
            "62acb19d45bc19ef63cf0574634165bbf1c21be65db660a386e1d8d16017a46a",
        ),
    ];
    for (position, (file, i)) in files.iter().zip([1, 0, 2]).enumerate() {
        assert_eq!(file["size_bytes"], payloads[position].0);
        assert_eq!(file["sha256"], payloads[position].1);
        assert_eq!(file["expected_metadata"], expected_metadata()[position]);
        assert_eq!(file["case_id"], CASE_IDS[i]);
        assert_eq!(file["path"], DICOM_PATHS[i]);
        assert_eq!(file["determinism"], "byte_stable");
        assert_eq!(file["validation"]["status"], "passed");
        assert!(
            file["validation"]["internal"]
                .as_array()
                .unwrap()
                .iter()
                .all(|c| c["status"] == "passed")
        );
    }
    println!(
        "caller metadata payload measurements: {}",
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

// Static semantic oracle from the accepted original three-case projection; no runtime baseline dependency.
pub fn expected_metadata() -> Value {
    serde_json::from_str(r#"[{"person_names":[{"component_groups":[{"components":[{"decoded_value":"Wang","position":1},{"decoded_value":"XiaoDong","position":2},{"decoded_value":"","position":3},{"decoded_value":"","position":4},{"decoded_value":"","position":5}],"decoded_value":"Wang^XiaoDong","kind":"alphabetic","position":1},{"components":[{"decoded_value":"王","position":1},{"decoded_value":"小東","position":2},{"decoded_value":"","position":3},{"decoded_value":"","position":4},{"decoded_value":"","position":5}],"decoded_value":"王^小東","kind":"ideographic","position":2}],"decoded_value":"Wang^XiaoDong=王^小東","keyword":"PatientName","raw_value_byte_length":24,"raw_value_hex":"57616E675E5869616F446F6E673DE78E8B5EE5B08FE69DB1","raw_value_sha256":"64a9d3d6b55142162489a8679e8643caa94efcff26dd30bf24650ac5186c1382","tag":"0010,0010","vr":"PN"}],"specific_character_sets":["ISO_IR 192"]},{"empty_type2_attributes":[{"keyword":"PatientName","tag":"0010,0010","value_length":0,"vr":"PN"},{"keyword":"PatientBirthDate","tag":"0010,0030","value_length":0,"vr":"DA"},{"keyword":"PatientSex","tag":"0010,0040","value_length":0,"vr":"CS"},{"keyword":"ReferringPhysicianName","tag":"0008,0090","value_length":0,"vr":"PN"},{"keyword":"AccessionNumber","tag":"0008,0050","value_length":0,"vr":"SH"}]},{"private_creator_blocks":[{"block_end_tag":"0011,10FF","block_start_tag":"0011,1000","creator_id":"DTS_PRIVATE_ALPHA","creator_tag":"0011,0010","elements":[{"decoded_value":"ALPHA-GROUP-0011","raw_value_byte_length":16,"raw_value_hex":"414C5048412D47524F55502D30303131","raw_value_sha256":"6b95b0cd9835f0ab50173c42a37511a7e8a547af8837f67e0a9bd0d6ff0da1ae","tag":"0011,1001","vr":"LO"},{"decoded_value":4660,"raw_value_byte_length":2,"raw_value_hex":"3412","raw_value_sha256":"e74d0e44a658ffcdc0ee7266ebd171413b8fcf182c97a27254d9f48abaea6266","tag":"0011,10F0","vr":"US"}],"raw_value_byte_length":18,"raw_value_hex":"4454535F505249564154455F414C50484120","raw_value_sha256":"02a7ccdec62f131efea4bb7c0954d15df2b1efd67abec69123ff0afcb197f8c3","vr":"LO"},{"block_end_tag":"0011,12FF","block_start_tag":"0011,1200","creator_id":"DTS_PRIVATE_BETA","creator_tag":"0011,0012","elements":[{"decoded_value":"BETA-BLOCK-12","raw_value_byte_length":14,"raw_value_hex":"424554412D424C4F434B2D313220","raw_value_sha256":"3329e2d8d73e62f294fd73110474122239fd4d75a8a2aefbe16c117f0265b328","tag":"0011,1201","vr":"LO"}],"raw_value_byte_length":16,"raw_value_hex":"4454535F505249564154455F42455441","raw_value_sha256":"df2316ffa7d764760e6c7f6174d3b15a2d59687834a90474b7446ff323df073d","vr":"LO"},{"block_end_tag":"0013,11FF","block_start_tag":"0013,1100","creator_id":"DTS_PRIVATE_ALPHA","creator_tag":"0013,0011","elements":[{"decoded_value":"ALPHA-GROUP-0013","raw_value_byte_length":16,"raw_value_hex":"414C5048412D47524F55502D30303133","raw_value_sha256":"6374ee55ea117a6d46b516c6ca6f2550d95c849a16221c58bfea5c054b9e6919","tag":"0013,1101","vr":"LO"}],"raw_value_byte_length":18,"raw_value_hex":"4454535F505249564154455F414C50484120","raw_value_sha256":"02a7ccdec62f131efea4bb7c0954d15df2b1efd67abec69123ff0afcb197f8c3","vr":"LO"}]}]"#).unwrap()
}

pub fn assert_payload_hash(path: &std::path::Path, expected_size: &Value, expected_hash: &Value) {
    let raw = fs::read(path).unwrap();
    assert_eq!(raw.len() as u64, expected_size.as_u64().unwrap());
    let digest = std::process::Command::new("python3")
        .args(["-c", "import hashlib,pathlib,sys;print(hashlib.sha256(pathlib.Path(sys.argv[1]).read_bytes()).hexdigest())"])
        .arg(path).output().unwrap();
    assert!(digest.status.success());
    assert!(digest.stderr.is_empty());
    assert_eq!(
        String::from_utf8(digest.stdout).unwrap().trim(),
        expected_hash.as_str().unwrap()
    );
}

fn assert_fixture(root: &std::path::Path) {
    fn inventory(
        root: &std::path::Path,
        current: &std::path::Path,
        files: &mut Vec<String>,
        directories: &mut Vec<String>,
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
            let kind = entry.file_type().unwrap();
            if kind.is_dir() {
                directories.push(name);
                inventory(root, &path, files, directories);
            } else {
                assert!(kind.is_file(), "fixture members must be ordinary files");
                files.push(name);
            }
        }
    }
    let mut files = Vec::new();
    let mut directories = Vec::new();
    inventory(root, root, &mut files, &mut directories);
    files.sort();
    directories.sort();
    assert_eq!(
        files,
        [
            "definition.json",
            "members/cases/recipes/caller_empty.json",
            "members/cases/recipes/caller_name.json",
            "members/cases/recipes/caller_private.json",
            "members/cases/registry.json",
            "members/evidence/phase-2-empty-type2-attributes.md",
            "members/evidence/phase-2-private-creator-blocks.md",
            "members/evidence/phase-2-utf8-person-name.md"
        ]
    );
    assert_eq!(
        directories,
        [
            "members",
            "members/cases",
            "members/cases/recipes",
            "members/evidence"
        ]
    );
    let registry: Value =
        serde_json::from_slice(&fs::read(root.join("members/cases/registry.json")).unwrap())
            .unwrap();
    assert_eq!(registry["cases"].as_array().unwrap().len(), 3);
    for (position, name) in ["name", "empty", "private"].into_iter().enumerate() {
        let id = format!("caller/metadata/{name}");
        let recipe_id = format!("caller_{name}");
        let row = &registry["cases"][position];
        assert_eq!(row["case_id"], id);
        assert_eq!(row["recipe_id"], recipe_id);
        let recipe: Value = serde_json::from_slice(
            &fs::read(root.join(format!("members/cases/recipes/{recipe_id}.json"))).unwrap(),
        )
        .unwrap();
        assert_eq!(recipe["binding"]["case_id"], id);
        assert_eq!(recipe["recipe_id"], recipe_id);
        assert_eq!(recipe["planning_order"], 900 + position);
        assert_eq!(recipe["projection_order"], 900 + position);
        assert_eq!(
            recipe["dicom"]["artifacts"][0]["output"]["path"],
            format!("independent/{name}.dcm")
        );
    }
}
