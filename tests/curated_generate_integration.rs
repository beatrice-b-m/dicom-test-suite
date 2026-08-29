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
        let manifest_bytes = serde_json::to_vec(entry).unwrap();
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
    // These qualification hashes were captured from the private pre-cutover
    // baseline. Each binds the full ordered migrated file-entry Values and the
    // exact bytes of every corresponding Part 10 payload.
    let expected = [
        (
            "smoke",
            "085e81ed731d9248ed3d4d59d37071f64066796a8efcbfb0405d2a72184a6698",
        ),
        (
            "all",
            // U4 expands this derived selection from the SC slice to the
            // byte- and manifest-parity-qualified classic families.
            "c24bbafcbbd0ab7d72a47d38fadc3a28ad4233434438e72fba7685b931744f22",
        ),
        (
            "legacy",
            "d7109c4e8dbfc13eae57fba4272c8d524fed4b572026b96fc7f10fb2f6024e12",
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
    for required in [
        "prepare_curated_sc_plan",
        "execute_curated_sc_plan",
        "write_supported_cases_with_plan_first_sc",
    ] {
        assert!(
            generation.contains(required),
            "ordinary generation does not use {required}"
        );
    }
    for required in [
        "CuratedScCorpusPlanProvider",
        "CuratedExecutionServiceFactory",
        "CorpusExecutor",
        "execute_into_staging",
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
    let transaction = generation
        .find("OutputTransaction::begin")
        .expect("publication transaction");
    assert!(
        plan < transaction,
        "curated planning must finish before private staging exists"
    );

    let generator = fs::read_to_string("src/generator.rs").unwrap();
    let stage_start = generator.find("fn write_curated_recipe_stage(").unwrap();
    let stage_end = generator[stage_start..]
        .find("pub(crate) fn write_supported_cases(")
        .map(|offset| stage_start + offset)
        .unwrap();
    let stage = &generator[stage_start..stage_end];
    assert!(
        stage.contains("plan_first_files"),
        "secondary-capture stage does not consume executor results"
    );
    let consume = stage
        .find("plan_first_files.remove(case_id)")
        .expect("plan-first result lookup");
    let legacy = stage
        .find("implementation.generate")
        .expect("legacy fallback for later migrations");
    assert!(
        consume < legacy,
        "legacy generation runs before checking executor results"
    );
    let dispatcher_start = generator
        .find("pub(crate) fn write_supported_cases_with_plan_first_sc(")
        .unwrap();
    let dispatcher = &generator[dispatcher_start..];
    let secondary = dispatcher
        .find("CuratedRecipeStage::SecondaryCapture")
        .expect("secondary-capture dispatch");
    let following = &dispatcher[secondary..];
    let map = following
        .find("&mut plan_first_files_by_case")
        .expect("plan-first map passed to secondary-capture stage");
    let next_stage = following
        .find("CuratedRecipeStage::ClassicCt")
        .expect("next curated stage");
    assert!(
        map < next_stage,
        "plan-first results are not consumed by the secondary-capture stage"
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

fn copy_tree(source: &Path, destination: &Path) {
    fs::create_dir_all(destination).unwrap();
    for entry in fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let target = destination.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}

#[test]
fn failed_ordinary_run_leaves_no_destination_or_private_staging() {
    let workspace = TempRoot::absent("failed-run");
    fs::create_dir(&workspace.0).unwrap();
    copy_tree(
        Path::new("cases/recipes"),
        &workspace.0.join("cases/recipes"),
    );
    fs::copy(
        "cases/registry.json",
        workspace.0.join("cases/registry.json"),
    )
    .unwrap();
    fs::create_dir(workspace.0.join("templates")).unwrap();
    fs::copy(
        "templates/catalog.json",
        workspace.0.join("templates/catalog.json"),
    )
    .unwrap();
    fs::copy(
        "standards.lock.json",
        workspace.0.join("standards.lock.json"),
    )
    .unwrap();
    // Cargo.lock is deliberately absent. Planning has all of its inputs, so
    // this failure occurs after planning at the publication boundary.
    let destination = workspace.0.join("failed-output");
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
    assert!(!result.status.success(), "missing Cargo.lock must fail");
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
