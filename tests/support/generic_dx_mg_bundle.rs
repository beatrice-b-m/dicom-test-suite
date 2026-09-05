use serde_json::{Value, json};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
use synth_dicom_gen::sdk::{CorpusSelector, DicomTestSuite, InspectCorpusRequest};

pub const CASE_IDS: [&str; 3] = [
    "caller/acquisition/digital",
    "caller/acquisition/presentation",
    "caller/acquisition/processing",
];
pub const DICOM_PATHS: [&str; 3] = [
    "independent/image-0.dcm",
    "independent/image-1.dcm",
    "independent/image-2.dcm",
];
pub const ORIGINAL_IDS: [&str; 3] = [
    "classic/dx/display_shutter_mono2_u16_explicit_le",
    "classic/mg/for_presentation_mono1_u16_12bit_explicit_le",
    "classic/mg/for_processing_mono2_u16_12bit_implicit_le",
];
pub struct GenericDxMgBundle {
    pub root: PathBuf,
    pub members: PathBuf,
    pub descriptor: PathBuf,
    pub identity: Value,
}
impl GenericDxMgBundle {
    pub fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = fs::canonicalize(std::env::temp_dir())
            .unwrap()
            .join(format!(
                "generic-dx-mg-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
        fs::create_dir(&root).unwrap();
        let source =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/generic-dx-mg-corpus");
        let members = root.join("members");
        fs::create_dir_all(members.join("cases/recipes")).unwrap();
        for name in [
            "definition.json",
            "members/cases/registry.json",
            "members/cases/recipes/caller_digital.json",
            "members/cases/recipes/caller_presentation.json",
            "members/cases/recipes/caller_processing.json",
        ] {
            let raw = fs::read(source.join(name)).unwrap();
            for original in ORIGINAL_IDS.into_iter().chain([
                "cases/recipes/classic/dx/dx_display_shutter_mono2_u16.json",
                "cases/recipes/classic/mg/mg_for_presentation_mono1_u16.json",
                "cases/recipes/classic/mg/mg_for_processing_mono2_u16.json",
                "classic/dx/display_shutter_mono2_u16_explicit_le/instance.dcm",
                "classic/mg/for_presentation_mono1_u16_12bit_explicit_le/instance.dcm",
                "classic/mg/for_processing_mono2_u16_12bit_implicit_le/instance.dcm",
            ]) {
                assert!(
                    !raw.windows(original.len())
                        .any(|part| part == original.as_bytes())
                );
            }
            fs::write(root.join(name), raw).unwrap();
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
                "schema_version":"1.0.0", "definition_id":"fixture.generic-dx-mg", "definition_version":"1.0.0",
                "manifest_sha256":"60528101a95fa25b27ef19e99a5e8811688ddf9737798030d516b4a8c356f90b",
                "corpus_definition_sha256":"40d6fa4aba53a857f1f0f15808cc069f2604f128405f0740f312e89946e08d7b",
                "file_count":5, "total_size_bytes":29453
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
        assert_eq!(files, ["image-0.dcm", "image-1.dcm", "image-2.dcm"]);
    }
}
impl Drop for GenericDxMgBundle {
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
            1482,
            "3b62a81fb80067bcf87194ae5d964751adc71ec26b92345578489f13725138e4",
        ),
        (
            1578,
            "c9c18f9bc81b83cadd9b94ebf2624c58392de050a1aefc12ab1498de105a9471",
        ),
        (
            1536,
            "6af836bc12a1fe6656588c3e8af351708fa9b9e131adcec0bed37125cf8e2a36",
        ),
    ];
    for (file, i) in files.iter().zip([1, 2, 0]) {
        assert_eq!(file["size_bytes"], payloads[i].0);
        assert_eq!(file["sha256"], payloads[i].1);
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
        "caller DX/MG payload measurements: {}",
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
