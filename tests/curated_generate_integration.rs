use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use dicom_test_suite::curated_plan::{
    CuratedCatalogPaths, CuratedScCorpusPlanProvider, CuratedScPlanRequest, CuratedScSelection,
};
use dicom_test_suite::{GenerateOptions, prepare_generation_run, sha256_hex};
use serde_json::Value;

const SEED: u64 = 7;
static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

struct TempRoot(PathBuf);

impl TempRoot {
    fn absent(label: &str) -> Self {
        Self(std::env::temp_dir().join(format!(
            "dicom-test-suite-curated-generate-{label}-{}-{}",
            std::process::id(),
            TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
        )))
    }
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn migrated_paths(profile: &str) -> BTreeSet<String> {
    let provider =
        CuratedScCorpusPlanProvider::load(CuratedCatalogPaths::from_repository_root(".")).unwrap();
    let bundle = provider
        .plan(&CuratedScPlanRequest {
            selection: CuratedScSelection::Profile {
                profile: profile.into(),
                include_stress: false,
            },
            seed: SEED,
            max_parallelism: 4,
        })
        .unwrap();
    bundle
        .plan
        .artifacts
        .iter()
        .map(|artifact| {
            artifact
                .output()
                .expect("curated DICOM artifact has output")
                .relative_path
                .as_str()
                .to_owned()
        })
        .collect()
}

fn run_generate(profile: &str, output: &Path) -> Value {
    let result = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .env_remove("DTS_HIGHDICOM_PYTHON")
        .args([
            "generate",
            "--profile",
            profile,
            "--out",
            output.to_str().unwrap(),
            "--seed",
            &SEED.to_string(),
        ])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "generate {profile} failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    serde_json::from_slice(&fs::read(output.join("manifest.json")).unwrap()).unwrap()
}

/// Locks the exact historical manifest values, their order, and the emitted
/// payload bytes without checking generated payloads into the repository.
fn migrated_slice_digest(root: &Path, manifest: &Value, selected: &BTreeSet<String>) -> String {
    let mut remaining = selected.clone();
    let mut qualified = Vec::new();
    for entry in manifest["files"].as_array().unwrap() {
        let path = entry["path"].as_str().unwrap();
        if !selected.contains(path) {
            continue;
        }
        assert!(remaining.remove(path), "duplicate migrated output {path}");
        let mut deterministic_entry = entry.clone();
        if let Some(backend) = deterministic_entry
            .get_mut("generation_backend")
            .and_then(Value::as_object_mut)
        {
            backend.remove("invocation_elapsed_milliseconds");
        }
        let manifest_bytes = serde_json::to_vec(&deterministic_entry).unwrap();
        let payload_bytes = fs::read(root.join(path)).unwrap();
        qualified.extend_from_slice(&(manifest_bytes.len() as u64).to_le_bytes());
        qualified.extend_from_slice(&manifest_bytes);
        qualified.extend_from_slice(&(payload_bytes.len() as u64).to_le_bytes());
        qualified.extend_from_slice(&payload_bytes);
    }
    assert!(
        remaining.is_empty(),
        "missing migrated outputs: {remaining:?}"
    );
    sha256_hex(&qualified)
}

#[test]
fn ordinary_generate_preserves_locked_curated_history_for_public_profiles() {
    // These qualification hashes bind the promoted terminal projection: the
    // full ordered migrated file-entry Values after removing the explicitly
    // nondeterministic backend elapsed time, plan provenance, and the exact
    // bytes of every corresponding Part 10 payload.
    let expected = [
        (
            "smoke",
            "798319444e6a0cd0b34607ebee9f4b2d88987e9c8cd0bb2e4a95480aa4f6a68e",
        ),
        (
            "all",
            "a50de8b288b3543876e4e58bcc2b435f41b81e84201e78508f093e894b8f4c36",
        ),
        (
            "legacy",
            "162112cb5b497bce5111a5f1a95d003f63b67ca444f37931b78f097fda86a864",
        ),
    ];
    let mut actual = Vec::new();
    for (profile, _) in expected {
        let root = TempRoot::absent(profile);
        let manifest = run_generate(profile, &root.0);
        let selected = migrated_paths(profile);
        assert!(
            !selected.is_empty(),
            "{profile} migrated selection is empty"
        );
        actual.push((
            profile,
            migrated_slice_digest(&root.0, &manifest, &selected),
        ));
    }
    assert_eq!(
        actual,
        expected
            .into_iter()
            .map(|(profile, digest)| (profile, digest.to_owned()))
            .collect::<Vec<_>>()
    );
}

#[test]
fn ordinary_generate_routes_curated_sc_through_the_shared_executor_only() {
    let library = fs::read_to_string("src/lib.rs").unwrap();
    let start = library.find("pub fn write_generation_run(").unwrap();
    let end = library[start..]
        .find("pub fn validate_generated_root(")
        .map(|offset| start + offset)
        .unwrap();
    let generation = &library[start..end];
    for required in ["prepare_curated_sc_plan", ".execute("] {
        assert!(
            generation.contains(required),
            "ordinary generation does not use {required}"
        );
    }
    assert!(
        !generation.contains("write_supported_cases_with_plan_first_sc"),
        "ordinary generation must not enter the compatibility dispatcher"
    );
    for required in [
        "CuratedScCorpusPlanProvider",
        "CuratedExecutionServiceFactory",
        "CorpusExecutor",
        "CuratedGenerationManifestProjector",
        "project_curated_file_entries",
    ] {
        assert!(
            library.contains(required),
            "generation integration does not use {required}"
        );
    }
    let plan = generation
        .find("prepare_curated_sc_plan")
        .expect("curated plan call");
    let transaction = generation.find(".execute(").expect("executor publication");
    assert!(
        plan < transaction,
        "curated planning must finish before private staging exists"
    );
    assert!(
        !generation.contains("OutputTransaction::begin")
            && !generation.contains("execute_into_staging"),
        "the generation frontend must not own a parallel transaction loop"
    );
}

#[test]
fn public_planning_and_preparation_leave_the_destination_absent() {
    let destination = TempRoot::absent("planning-absent");
    let prepared = prepare_generation_run(GenerateOptions {
        profile: "smoke".into(),
        out_dir: destination.0.clone(),
        seed: SEED,
        include_stress: false,
    })
    .unwrap();
    assert!(!prepared.out_dir.exists());

    let provider =
        CuratedScCorpusPlanProvider::load(CuratedCatalogPaths::from_repository_root(".")).unwrap();
    let bundle = provider
        .plan(&CuratedScPlanRequest {
            selection: CuratedScSelection::Profile {
                profile: "smoke".into(),
                include_stress: false,
            },
            seed: SEED,
            max_parallelism: 4,
        })
        .unwrap();
    assert!(!bundle.plan.artifacts.is_empty());
    assert!(
        !destination.0.exists(),
        "planning created public or private output state"
    );
}

#[test]
fn failed_ordinary_run_leaves_no_destination_or_private_staging() {
    let workspace = TempRoot::absent("failed-run");
    fs::create_dir(&workspace.0).unwrap();
    let blocked_parent = workspace.0.join("blocked-parent");
    fs::write(&blocked_parent, b"not a directory").unwrap();
    // Embedded product resources allow planning to finish independently of
    // the current directory. The non-directory parent then forces a failure
    // at the publication boundary, before any transaction can be created.
    let destination = blocked_parent.join("failed-output");
    let result = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .current_dir(&workspace.0)
        .args([
            "generate",
            "--profile",
            "smoke",
            "--out",
            destination.to_str().unwrap(),
            "--seed",
            &SEED.to_string(),
        ])
        .output()
        .unwrap();
    assert!(
        !result.status.success(),
        "non-directory publication parent must fail"
    );
    assert!(
        !destination.exists(),
        "failed run published its destination"
    );
    let leaked = fs::read_dir(&workspace.0)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| name.contains(".dicom-test-suite-staging-"))
        .collect::<Vec<_>>();
    assert!(leaked.is_empty(), "failed run leaked staging: {leaked:?}");
}
