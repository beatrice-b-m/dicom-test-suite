#![cfg(feature = "legacy_jpeg_dcmtk")]

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use synth_dicom_gen::curated_execution::CuratedExecutionServiceFactory;
use synth_dicom_gen::curated_plan::{
    CuratedCatalogPaths, CuratedScCorpusPlan, CuratedScCorpusPlanProvider, CuratedScPlanRequest,
    CuratedScSelection,
};
use synth_dicom_gen::executor::adapters::ManifestProjectionInput;
use synth_dicom_gen::executor::cancellation::CancellationToken;
use synth_dicom_gen::executor::engine::{
    CorpusExecutor, ManifestProjectionError, ManifestProjector,
};
use synth_dicom_gen::runtime_capabilities::{CapabilityInventory, QualifiedExecutableIdentity};
use synth_dicom_gen::sha256_hex;

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Projector;
impl ManifestProjector for Projector {
    fn project(&self, _: &ManifestProjectionInput) -> Result<Vec<u8>, ManifestProjectionError> {
        Ok(b"{}\n".to_vec())
    }
}

fn set(values: &[&str]) -> BTreeSet<String> {
    values.iter().map(|value| (*value).into()).collect()
}

fn command(script_body: &str) -> (PathBuf, QualifiedExecutableIdentity, PathBuf) {
    let root = std::env::temp_dir().canonicalize().unwrap().join(format!(
        "dts-locked-fake-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(&root).unwrap();
    let path = root.join("dcmcjpeg");
    let bytes = format!(
        "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then echo 'fake-dcmcjpeg 3.6.9'; exit 0; fi\n{script_body}\n"
    )
    .into_bytes();
    fs::write(&path, &bytes).unwrap();
    let mut permissions = fs::metadata(&path).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&path, permissions).unwrap();
    (
        path,
        QualifiedExecutableIdentity {
            version: "fake-dcmcjpeg 3.6.9".into(),
            executable_sha256: sha256_hex(&bytes),
        },
        root,
    )
}

fn plan(
    case_id: &str,
    backend_id: &str,
    identity: QualifiedExecutableIdentity,
) -> CuratedScCorpusPlan {
    CuratedScCorpusPlanProvider::load(CuratedCatalogPaths::from_repository_root("."))
        .unwrap()
        .with_capability_inventory(CapabilityInventory {
            compiled_features: set(&["legacy_jpeg_dcmtk"]),
            executable_codec_backends: set(&[backend_id]),
            available_executables: set(&["dcmcjpeg"]),
            executable_identities: BTreeMap::from([("dcmcjpeg".into(), identity)]),
            ..CapabilityInventory::default()
        })
        .plan(&CuratedScPlanRequest {
            selection: CuratedScSelection::CaseIds(vec![case_id.into()]),
            seed: 1,
            max_parallelism: 1,
        })
        .unwrap()
}

#[test]
fn mismatched_command_fingerprint_fails_before_execution() {
    let (path, mut identity, root) = command("exit 7");
    identity.executable_sha256 = "00".repeat(32);
    let bundle = plan(
        "classic/sc/mono2_u16_jpeg_lossless_process_14",
        "dcmtk_dcmcjpeg_jpeg_lossless_process_14_command_writer",
        identity,
    );
    let error = match CuratedExecutionServiceFactory::with_dcmtk_command(&bundle, path) {
        Ok(_) => panic!("identity mismatch must fail"),
        Err(error) => error,
    };
    assert!(
        error
            .to_string()
            .contains("differs from planning inventory")
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn invalid_fake_output_is_rejected_and_never_published() {
    let (path, identity, root) = command(
        "prev=''; for arg in \"$@\"; do prev2=\"$prev\"; prev=\"$arg\"; done; cp \"$prev2\" \"$prev\"",
    );
    let bundle = plan(
        "classic/sc/mono2_u16_jpeg_lossless_process_14",
        "dcmtk_dcmcjpeg_jpeg_lossless_process_14_command_writer",
        identity,
    );
    let destination = root.join("published");
    let factory = CuratedExecutionServiceFactory::with_dcmtk_command(&bundle, path).unwrap();
    let error = CorpusExecutor::new(factory, Projector)
        .execute(&bundle.plan, &destination, 1, &CancellationToken::new())
        .unwrap_err();
    assert!(error.to_string().contains("transfer syntax"), "{error}");
    assert!(!destination.exists());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn cancellation_kills_the_locked_command_and_prevents_publication() {
    let (path, identity, root) = command("sleep 10; exit 7");
    let bundle = plan(
        "classic/sc/mono2_u16_jpeg_lossless_process_14",
        "dcmtk_dcmcjpeg_jpeg_lossless_process_14_command_writer",
        identity,
    );
    let destination = root.join("published");
    let factory = CuratedExecutionServiceFactory::with_dcmtk_command(&bundle, path).unwrap();
    let cancellation = CancellationToken::new();
    let worker_token = cancellation.clone();
    let started = Instant::now();
    let worker = std::thread::spawn(move || {
        CorpusExecutor::new(factory, Projector).execute(
            &bundle.plan,
            &destination,
            1,
            &worker_token,
        )
    });
    std::thread::sleep(Duration::from_millis(250));
    cancellation.cancel();
    let error = worker.join().unwrap().unwrap_err();
    assert!(
        error.to_string().to_ascii_lowercase().contains("cancel"),
        "{error}"
    );
    assert!(started.elapsed() < Duration::from_secs(3));
    assert!(!root.join("published").exists());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn real_qualified_command_executes_both_locked_cases_when_available() {
    let which = std::process::Command::new("which")
        .arg("dcmcjpeg")
        .output()
        .unwrap();
    if !which.status.success() {
        eprintln!("skipping: dcmcjpeg is unavailable");
        return;
    }
    let path = PathBuf::from(String::from_utf8(which.stdout).unwrap().trim());
    let version = std::process::Command::new(&path)
        .arg("--version")
        .output()
        .unwrap();
    let identity = QualifiedExecutableIdentity {
        version: String::from_utf8_lossy(&version.stdout).trim().into(),
        executable_sha256: sha256_hex(&fs::read(&path).unwrap()),
    };
    for (case_id, backend_id, relative) in [
        (
            "classic/sc/mono2_u16_jpeg_lossless_process_14",
            "dcmtk_dcmcjpeg_jpeg_lossless_process_14_command_writer",
            "classic/sc/mono2_u16_jpeg_lossless_process_14/instance.dcm",
        ),
        (
            "classic/sc/mono2_u16_jpeg_lossless_sv1",
            "dcmtk_dcmcjpeg_jpeg_lossless_sv1_command_writer",
            "classic/sc/mono2_u16_jpeg_lossless_sv1/instance.dcm",
        ),
    ] {
        let bundle = plan(case_id, backend_id, identity.clone());
        let root = std::env::temp_dir().canonicalize().unwrap().join(format!(
            "dts-locked-real-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let factory =
            CuratedExecutionServiceFactory::with_dcmtk_command(&bundle, path.clone()).unwrap();
        CorpusExecutor::new(factory, Projector)
            .execute(&bundle.plan, &root, 1, &CancellationToken::new())
            .unwrap();
        assert!(root.join(relative).is_file());
        fs::remove_dir_all(root).unwrap();
    }
}
