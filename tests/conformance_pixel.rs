use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;

#[test]
fn real_dcmtk_rle_adapter_matches_all_manifest_frame_hashes_when_enabled() {
    if std::env::var("DTS_REAL_CONFORMANCE").as_deref() != Ok("1") {
        return;
    }
    for command in ["dcmdump", "dcmdrle"] {
        assert!(
            Command::new(command).arg("--version").status().is_ok(),
            "{command} must be installed"
        );
    }
    let root = temp_dir();
    let generated = root.join("generated");
    let evidence = root.join("evidence");
    assert!(
        Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
            .args(["generate", "--profile", "all", "--out"])
            .arg(&generated)
            .status()
            .unwrap()
            .success()
    );
    assert!(
        Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
            .args(["conformance", "run"])
            .arg(&generated)
            .args(["--out"])
            .arg(&evidence)
            .status()
            .unwrap()
            .success()
    );
    let run: Value =
        serde_json::from_slice(&fs::read(evidence.join("conformance-run.json")).unwrap()).unwrap();
    let rle = run["instances"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|instance| instance["transfer_syntax_uid"] == "1.2.840.10008.1.2.5")
        .collect::<Vec<_>>();
    assert!(!rle.is_empty());
    assert!(
        rle.iter()
            .all(|instance| instance["pixel"]["status"] == "passed")
    );
}

fn temp_dir() -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("dts-real-pixel-{nonce}"));
    fs::create_dir_all(&root).unwrap();
    root
}
