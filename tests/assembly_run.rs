use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use dicom_core::{DataElement, VR};
use dicom_dictionary_std::tags;
use dicom_object::open_file;
use synth_dicom_gen::assembly::{AssembleOptions, assemble};
use synth_dicom_gen::engine_resources::EngineResources;
use synth_dicom_gen::executor::cancellation::CancellationToken;
use synth_dicom_gen::{build_coverage_report, validate_generated_root};

static NEXT: AtomicU64 = AtomicU64::new(0);

fn output(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "dicom-test-suite-assembly-{label}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ))
}

fn request() -> Vec<u8> {
    include_bytes!("fixtures/cli/assembly-request-seed5.json").to_vec()
}

#[test]
fn structural_assembly_executes_through_shared_writer_and_manifest() {
    let root = output("published");
    assert_eq!(
        synth_dicom_gen::sha256_hex(&request()),
        "29a0da03979c74631959851cc103cdcb9114a703de3f2b658ef714efd062f664"
    );
    let summary = assemble(
        &AssembleOptions {
            request_bytes: request(),
            caller_asset_root: PathBuf::from("."),
            output_root: root.clone(),
            seed: 5,
            parallelism: 1,
            dry_run: false,
        },
        &CancellationToken::new(),
        &EngineResources::embedded(),
    )
    .unwrap();
    assert!(summary.published);
    assert_eq!(summary.artifacts_written, 1);
    assert!(summary.output_bytes > 0);
    let object = open_file(root.join("instances/primary.dcm")).unwrap();
    assert_eq!(
        object.meta().implementation_class_uid(),
        "2.25.229836331705510893456395357084592248116"
    );
    assert_eq!(
        object.meta().implementation_version_name.as_deref(),
        Some("DICOMTS010")
    );

    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("manifest.json")).unwrap()).unwrap();
    let schema: serde_json::Value = serde_json::from_slice(
        &fs::read("schemas/structural-assembly-manifest-v2.schema.json").unwrap(),
    )
    .unwrap();
    let identity_schema: serde_json::Value =
        serde_json::from_slice(&fs::read("schemas/version-result-v2.schema.json").unwrap())
            .unwrap();
    let validator = jsonschema::options()
        .with_draft(jsonschema::Draft::Draft202012)
        .with_resource(
            "https://synth-dicom-gen.local/schemas/version-result-v2.schema.json",
            jsonschema::Resource::from_contents(identity_schema).unwrap(),
        )
        .build(&schema)
        .unwrap();
    assert!(validator.is_valid(&manifest));
    assert_eq!(manifest["manifest_schema_version"], "2.0.0");
    assert_eq!(
        manifest["identity_projection"]["corpus_definition"]["state"],
        "transitional_embedded_unverified"
    );
    assert!(manifest["identity_projection"]["corpus_definition"]["identity"].is_null());
    assert_eq!(
        manifest["identity_projection"]["external_runtime"],
        serde_json::json!([])
    );
    assert_eq!(
        manifest["identity_projection"]["legacy_provenance"]["resource_count"],
        240
    );
    assert_eq!(
        manifest["identity_projection"]["legacy_provenance"]["resource_set_sha256"],
        "dc61cc012f983297fef864f68e6cd172a9d33ac9ad4faab4cc66d3526b688410"
    );
    assert_eq!(
        manifest["run"]["corpus_plan_sha256"],
        "d4ebd5a7ca2081375022b0d9bf8726d5e8f508afe59e1c1ad7d68d65f5ebda45"
    );
    assert_eq!(
        synth_dicom_gen::sha256_hex(&fs::read(root.join("instances/primary.dcm")).unwrap()),
        "7da898187a4f13054d48268660770f86c6939c082a5731d8f3737a5a855c7ba7"
    );
    assert_eq!(
        fs::metadata(root.join("instances/primary.dcm"))
            .unwrap()
            .len(),
        774
    );
    assert_eq!(
        manifest["instances"][0]["identity"]["study_instance_uid"],
        "2.25.131391886742213678286033606434638135815"
    );
    assert_eq!(
        manifest["instances"][0]["identity"]["series_instance_uid"],
        "2.25.39607469728402369607566362072293772613"
    );
    assert_eq!(
        manifest["instances"][0]["identity"]["sop_instance_uid"],
        "2.25.46553092604571578884474658719670022502"
    );
    assert_eq!(
        manifest["instances"][0]["identity"]["frame_of_reference_uid"],
        "2.25.245763833390795330413760970236265638444"
    );
    let mut normalized = manifest.clone();
    normalized
        .as_object_mut()
        .unwrap()
        .remove("identity_projection");
    normalized["manifest_schema_version"] = "1.0.0".into();
    let legacy: serde_json::Value =
        serde_json::from_slice(include_bytes!("fixtures/cli/assembly-manifest-v1.json")).unwrap();
    assert_eq!(normalized, legacy);
    assert_eq!(manifest["run"]["kind"], "structural_assembly");
    assert_eq!(manifest["run"]["iod_conformance"], "not_assessed");
    assert_eq!(manifest["instances"][0]["iod_conformance"], "not_assessed");
    for forbidden in ["case_id", "profile", "template_id", "qualification_status"] {
        assert!(
            !contains_key(&manifest, forbidden),
            "forbidden claim {forbidden}"
        );
    }
    let validation = validate_generated_root(&root).unwrap();
    assert_eq!(validation.files_checked, 1);
    assert!(validation.failures.is_empty(), "{:?}", validation.failures);
    let report = build_coverage_report(&root).unwrap();
    assert_eq!(report["report_kind"], "structural_assembly");
    assert_eq!(report["iod_conformance"], "not_assessed");
    assert!(report.get("coverage_matrix").is_none());
    let report_schema: serde_json::Value = serde_json::from_slice(
        &fs::read("schemas/structural-assembly-report.schema.json").unwrap(),
    )
    .unwrap();
    assert!(
        jsonschema::options()
            .with_draft(jsonschema::Draft::Draft202012)
            .build(&report_schema)
            .unwrap()
            .is_valid(&report)
    );
    let perturbed_root = output("request-identity-perturbation");
    let mut perturbed_request: serde_json::Value = serde_json::from_slice(&request()).unwrap();
    perturbed_request["instances"][0]["elements"][0]["value"]["value"] =
        "SYNTHETIC^ASSEMBLY^PERTURBED".into();
    assemble(
        &AssembleOptions {
            request_bytes: serde_json::to_vec(&perturbed_request).unwrap(),
            caller_asset_root: PathBuf::from("."),
            output_root: perturbed_root.clone(),
            seed: 5,
            parallelism: 1,
            dry_run: false,
        },
        &CancellationToken::new(),
        &EngineResources::embedded(),
    )
    .unwrap();
    let perturbed_manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(perturbed_root.join("manifest.json")).unwrap()).unwrap();
    assert_ne!(
        manifest["run"]["request_sha256"],
        perturbed_manifest["run"]["request_sha256"]
    );
    assert_eq!(
        manifest["identity_projection"], perturbed_manifest["identity_projection"],
        "caller request content and assets are not installed identity domains"
    );
    fs::remove_dir_all(root).unwrap();
    fs::remove_dir_all(perturbed_root).unwrap();
}

#[test]
fn structural_validation_detects_post_publication_tampering() {
    let root = output("tampered");
    assemble(
        &AssembleOptions {
            request_bytes: request(),
            caller_asset_root: PathBuf::from("."),
            output_root: root.clone(),
            seed: 5,
            parallelism: 1,
            dry_run: false,
        },
        &CancellationToken::new(),
        &EngineResources::embedded(),
    )
    .unwrap();
    let path = root.join("instances/primary.dcm");
    let mut bytes = fs::read(&path).unwrap();
    let last = bytes.len() - 1;
    bytes[last] ^= 0x01;
    fs::write(&path, bytes).unwrap();
    let validation = validate_generated_root(&root).unwrap();
    assert!(!validation.failures.is_empty());
    assert!(
        validation
            .failures
            .iter()
            .any(|failure| failure.contains("identity mismatch"))
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn structural_validation_compares_manifest_element_evidence() {
    let root = output("semantic-tamper");
    assemble(
        &AssembleOptions {
            request_bytes: request(),
            caller_asset_root: PathBuf::from("."),
            output_root: root.clone(),
            seed: 5,
            parallelism: 1,
            dry_run: false,
        },
        &CancellationToken::new(),
        &EngineResources::embedded(),
    )
    .unwrap();
    let path = root.join("instances/primary.dcm");
    let mut object = open_file(&path).unwrap();
    object.put(DataElement::new(
        tags::PATIENT_NAME,
        VR::PN,
        "TAMPERED^VALUE",
    ));
    let replacement = root.join("replacement.dcm");
    object.write_to_file(&replacement).unwrap();
    fs::remove_file(&path).unwrap();
    fs::rename(&replacement, &path).unwrap();

    let bytes = fs::read(&path).unwrap();
    let manifest_path = root.join("manifest.json");
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
    manifest["instances"][0]["size_bytes"] = serde_json::json!(bytes.len());
    manifest["instances"][0]["sha256"] = serde_json::json!(synth_dicom_gen::sha256_hex(&bytes));
    fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();

    let validation = validate_generated_root(&root).unwrap();
    assert!(
        validation
            .failures
            .iter()
            .any(|failure| failure.contains("0010,0010 value mismatch")),
        "{:?}",
        validation.failures
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn structural_dry_run_has_same_plan_hash_and_creates_nothing() {
    let published_root = output("hash-published");
    let dry_root = output("hash-dry");
    let options = |root, dry_run| AssembleOptions {
        request_bytes: request(),
        caller_asset_root: PathBuf::from("."),
        output_root: root,
        seed: 5,
        parallelism: 1,
        dry_run,
    };
    let published = assemble(
        &options(published_root.clone(), false),
        &CancellationToken::new(),
        &EngineResources::embedded(),
    )
    .unwrap();
    let dry = assemble(
        &options(dry_root.clone(), true),
        &CancellationToken::new(),
        &EngineResources::embedded(),
    )
    .unwrap();
    assert_eq!(published.corpus_plan_sha256, dry.corpus_plan_sha256);
    assert!(!dry.published);
    assert!(dry.manifest_path.is_none());
    assert!(!dry_root.exists());
    fs::remove_dir_all(published_root).unwrap();
}

#[test]
fn structural_cancellation_publishes_nothing() {
    let root = output("cancelled");
    let cancellation = CancellationToken::new();
    cancellation.cancel();
    let error = assemble(
        &AssembleOptions {
            request_bytes: request(),
            caller_asset_root: PathBuf::from("."),
            output_root: root.clone(),
            seed: 5,
            parallelism: 1,
            dry_run: false,
        },
        &cancellation,
        &EngineResources::embedded(),
    )
    .unwrap_err();
    assert!(
        error.to_string().to_ascii_lowercase().contains("cancel"),
        "{error}"
    );
    assert!(!root.exists());
}

#[test]
fn structural_destination_race_preserves_one_valid_winner_and_cleans_staging() {
    let parent = output("race-parent");
    fs::create_dir_all(&parent).unwrap();
    let destination = parent.join("winner");
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    let workers = (0..2)
        .map(|_| {
            let destination = destination.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                assemble(
                    &AssembleOptions {
                        request_bytes: request(),
                        caller_asset_root: PathBuf::from("."),
                        output_root: destination,
                        seed: 5,
                        parallelism: 2,
                        dry_run: false,
                    },
                    &CancellationToken::new(),
                    &EngineResources::embedded(),
                )
            })
        })
        .collect::<Vec<_>>();
    let results = workers
        .into_iter()
        .map(|worker| worker.join().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(results.iter().filter(|result| result.is_err()).count(), 1);
    assert!(
        validate_generated_root(&destination)
            .unwrap()
            .failures
            .is_empty()
    );
    assert!(fs::read_dir(&parent).unwrap().all(|entry| {
        !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".synth-dicom-gen-staging-")
    }));
    fs::remove_dir_all(parent).unwrap();
}

fn contains_key(value: &serde_json::Value, key: &str) -> bool {
    match value {
        serde_json::Value::Object(object) => {
            object.contains_key(key) || object.values().any(|value| contains_key(value, key))
        }
        serde_json::Value::Array(values) => values.iter().any(|value| contains_key(value, key)),
        _ => false,
    }
}
