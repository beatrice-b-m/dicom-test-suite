use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::{Value, json};
use synth_dicom_gen::sdk::{CorpusSelector, DicomTestSuite, InspectCorpusRequest};

pub const CASE_ID: &str = "caller/arbitrary/signed-ct";
pub const RECIPE_ID: &str = "caller_signed_ct";
pub const DICOM_PATH: &str = "caller/arbitrary/signed-ct/caller-instance.dcm";
const ORIGINAL_CASE_ID: &str = "classic/ct/mono2_i16_rescale_12bit_explicit_le";
const ORIGINAL_RECIPE_PATH: &str = "cases/recipes/classic/ct/ct_mono2_i16_rescale.json";
const ORIGINAL_OUTPUT_PATH: &str = "classic/ct/mono2_i16_rescale_12bit_explicit_le/instance.dcm";
const PLAN_SHA256: &str = "d3a5a83f33caf7abdce7a6df5c3675754e48e40e78d17968fe83236b1fdfadb4";

pub struct GenericCtBundle {
    pub root: PathBuf,
    pub members: PathBuf,
    pub descriptor: PathBuf,
    pub identity: Value,
}

impl GenericCtBundle {
    pub fn new(label: &str) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = fs::canonicalize(std::env::temp_dir())
            .unwrap()
            .join(format!(
                "synth-dicom-gen-generic-ct-{label}-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
        fs::create_dir(&root).unwrap();
        let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/generic-ct-corpus");
        let descriptor_bytes = fs::read(source.join("definition.json")).unwrap();
        let registry_bytes = fs::read(source.join("members/cases/registry.json")).unwrap();
        let recipe_bytes =
            fs::read(source.join("members/cases/recipes/caller-signed-ct.json")).unwrap();
        for bytes in [&descriptor_bytes, &registry_bytes, &recipe_bytes] {
            assert!(
                !bytes
                    .windows(ORIGINAL_CASE_ID.len())
                    .any(|w| w == ORIGINAL_CASE_ID.as_bytes())
            );
            assert!(
                !bytes
                    .windows(ORIGINAL_RECIPE_PATH.len())
                    .any(|w| w == ORIGINAL_RECIPE_PATH.as_bytes())
            );
            assert!(
                !bytes
                    .windows(ORIGINAL_OUTPUT_PATH.len())
                    .any(|w| w == ORIGINAL_OUTPUT_PATH.as_bytes())
            );
        }
        let members = root.join("members");
        fs::create_dir_all(members.join("cases/recipes")).unwrap();
        fs::write(members.join("cases/registry.json"), registry_bytes).unwrap();
        fs::write(
            members.join("cases/recipes/caller-signed-ct.json"),
            recipe_bytes,
        )
        .unwrap();
        let descriptor = root.join("definition.json");
        fs::write(&descriptor, descriptor_bytes).unwrap();
        let product = DicomTestSuite::embedded().unwrap();
        let inspected = product
            .inspect_corpus(
                InspectCorpusRequest::from_file(&descriptor, &members)
                    .with_selection(selector())
                    .with_seed(1)
                    .with_parallelism(4),
            )
            .unwrap();
        assert_eq!(
            inspected.assessment().unwrap().corpus_plan_sha256(),
            PLAN_SHA256
        );
        let identity = inspected.corpus_definition_identity().clone();
        assert_eq!(
            identity,
            json!({
                "schema_version": "1.0.0",
                "definition_id": "fixture.generic-ct",
                "definition_version": "1.0.0",
                "manifest_sha256": "1f33541dfba0df229be6e3d9d3aadc405d842f8842cb7ca7eff9ea7cf29efb5d",
                "corpus_definition_sha256": "8e99cc8d2983f3063583e7f2bf558380a7cdbb9d2001772ec00f4ec5f5079544",
                "file_count": 3,
                "total_size_bytes": 6498,
            })
        );
        Self {
            root,
            members,
            descriptor,
            identity,
        }
    }

    pub fn output_files(&self, name: &str) -> Vec<String> {
        let output = self.root.join(name);
        let mut files = Vec::new();
        collect_files(&output, &output, &mut files);
        files.sort();
        files
    }

    pub fn output_directories(&self, name: &str) -> BTreeSet<String> {
        let output = self.root.join(name);
        let mut directories = BTreeSet::new();
        collect_directories(&output, &output, &mut directories);
        directories
    }
}

pub fn selector() -> CorpusSelector {
    CorpusSelector::CaseIds {
        profile: "core".into(),
        include_stress: false,
        case_ids: vec![CASE_ID.into()],
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
        json!({"kind":"case_ids", "case_ids":[CASE_ID]})
    );
    assert_eq!(
        manifest["identity_projection"]["corpus_definition"]["state"],
        "verified_bundle"
    );
    assert_eq!(
        manifest["identity_projection"]["corpus_definition"]["identity"],
        *identity
    );
    let ledger = manifest["selection_ledger"].as_array().unwrap();
    assert_eq!(ledger.len(), 1);
    assert_eq!(ledger[0]["case_id"], CASE_ID);
    assert_eq!(ledger[0]["selection"], "direct");
    assert_eq!(ledger[0]["registry_status"], "implemented");
    assert_eq!(ledger[0]["outcome"], "generated");
    assert!(ledger[0]["reason_code"].is_null());
    assert_eq!(ledger[0]["dependency_case_ids"], json!([]));
    assert_eq!(ledger[0]["artifact_paths"], json!([DICOM_PATH]));
    assert_eq!(ledger[0]["case_definition"]["case_id"], CASE_ID);
    assert_eq!(ledger[0]["case_definition"]["recipe_id"], RECIPE_ID);
    let files = manifest["files"].as_array().unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0]["case_id"], CASE_ID);
    assert_eq!(files[0]["path"], DICOM_PATH);
    assert_eq!(files[0]["size_bytes"], 1194);
    assert_eq!(
        files[0]["sha256"],
        "c292a81584998e9afe56330545f455c8894684e475c817d60c1c93ef755e1ce1"
    );
    assert_eq!(files[0]["determinism"], "byte_stable");
    assert_eq!(files[0]["validation"]["status"], "passed");
    assert!(
        files[0]["validation"]["internal"]
            .as_array()
            .unwrap()
            .iter()
            .all(|check| check["status"] == "passed")
    );
}

pub fn assert_output_closure(bundle: &GenericCtBundle, name: &str) {
    assert!(output_closure_is_exact(bundle, name));
}

pub fn output_closure_is_exact(bundle: &GenericCtBundle, name: &str) -> bool {
    bundle.output_files(name) == vec![DICOM_PATH, "manifest.json"]
        && bundle.output_directories(name)
            == ["caller", "caller/arbitrary", "caller/arbitrary/signed-ct"]
                .into_iter()
                .map(str::to_owned)
                .collect()
}

pub fn assert_report(report: &Value, manifest: &Value) {
    assert_eq!(
        report
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect::<BTreeSet<_>>(),
        [
            "artifact_dimensions",
            "case_dimensions",
            "coverage_report_schema_version",
            "evidence",
            "identity_projection",
            "report_kind",
            "source_manifest",
            "summary",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect()
    );
    assert_eq!(report["coverage_report_schema_version"], "2.0.0");
    assert_eq!(report["report_kind"], "external_corpus");
    assert_eq!(report["source_manifest"], *manifest);
    assert_eq!(
        report["identity_projection"],
        manifest["identity_projection"]
    );
    assert_eq!(report["evidence"]["class"], "manifest_projection");
    assert_eq!(report["evidence"]["validation"], "not_assessed");
    assert_eq!(
        report["evidence"]["independent_conformance"],
        "not_assessed"
    );
    assert_eq!(report["evidence"]["payloads_reopened"], false);
    assert_eq!(report["summary"]["logical_cases"], 1);
    assert_eq!(report["summary"]["direct_cases"], 1);
    assert_eq!(report["summary"]["dependency_cases"], 0);
    assert_eq!(report["summary"]["emitted_files"], 1);
}

impl Drop for GenericCtBundle {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}

fn collect_files(root: &Path, path: &Path, output: &mut Vec<String>) {
    for entry in fs::read_dir(path).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            collect_files(root, &path, output);
        } else {
            output.push(relative(root, &path));
        }
    }
}

fn collect_directories(root: &Path, path: &Path, output: &mut BTreeSet<String>) {
    for entry in fs::read_dir(path).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            output.insert(relative(root, &path));
            collect_directories(root, &path, output);
        }
    }
}

fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap()
        .to_string_lossy()
        .replace('\\', "/")
}
