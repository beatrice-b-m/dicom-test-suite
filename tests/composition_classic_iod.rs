use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use dicom_test_suite::composition::{ComposeOptions, compose};

static NEXT: AtomicU64 = AtomicU64::new(0);

fn executable(name: &str) -> Option<PathBuf> {
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths)
            .map(|directory| directory.join(name))
            .find(|path| path.is_file())
    })
}

fn locked_dciodvfy_sha256() -> String {
    let lock: serde_json::Value =
        serde_json::from_slice(&fs::read("conformance/validator-lock.json").unwrap()).unwrap();
    lock["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|tool| tool["adapter_id"] == "dicom3tools-dciodvfy")
        .unwrap()["executable_sha256"]
        .as_str()
        .unwrap()
        .to_string()
}

#[test]
fn p3_3_defaults_pass_pinned_independent_iod_validation_when_available() {
    let Some(executable) = executable("dciodvfy") else {
        eprintln!("dciodvfy unavailable; independent P3.3 evidence is environment-unavailable");
        return;
    };
    let executable = fs::canonicalize(executable).unwrap();
    assert_eq!(
        dicom_test_suite::sha256_hex(&fs::read(&executable).unwrap()),
        locked_dciodvfy_sha256(),
        "the independent validator must match conformance/validator-lock.json"
    );
    let root = std::env::temp_dir().join(format!(
        "dts-composition-classic-iod-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    compose(&ComposeOptions {
        spec_path: "tests/fixtures/composition/valid/classic-p3-3-defaults.json".into(),
        out_dir: root.clone(),
        seed: 33,
        catalog_path: "templates/catalog.json".into(),
        dry_run: false,
    })
    .unwrap();
    for (instance, iod) in [("cr", "CRImage"), ("ct", "CTImage"), ("mr", "MRImage")] {
        let result = Command::new(&executable)
            .args(["-new"])
            .arg(root.join(format!("instances/{instance}.dcm")))
            .output()
            .unwrap();
        let findings = format!(
            "{}{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(result.status.success(), "{instance}: {findings}");
        assert!(!findings.contains("Error -"), "{instance}: {findings}");
        assert!(!findings.contains("Warning -"), "{instance}: {findings}");
        assert!(findings.contains(iod), "{instance}: {findings}");
    }
    fs::remove_dir_all(root).unwrap();
}
