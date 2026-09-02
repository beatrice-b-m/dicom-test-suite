use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use synth_dicom_gen::composition::{ComposeOptions, compose};

static NEXT: AtomicU64 = AtomicU64::new(0);

fn executable(name: &str) -> Option<PathBuf> {
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths)
            .map(|directory| directory.join(name))
            .find(|path| path.is_file())
    })
}

fn output(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "dts-composition-iod-{label}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ))
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

fn qualify(executable: &Path, label: &str, spec_path: &str) {
    let root = output(label);
    compose(&ComposeOptions {
        spec_path: spec_path.into(),
        out_dir: root.clone(),
        seed: 1,
        catalog_path: "templates/catalog.json".into(),
        dry_run: false,
    })
    .unwrap();
    let result = Command::new(executable)
        .args(["-new"])
        .arg(root.join("instances/primary.dcm"))
        .output()
        .unwrap();
    let findings = format!(
        "{}{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(result.status.success(), "{findings}");
    assert!(!findings.contains("Error -"), "{findings}");
    assert!(!findings.contains("Warning -"), "{findings}");
    assert!(findings.contains("SCImage"), "{findings}");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn default_sc_templates_pass_the_pinned_independent_iod_route_when_available() {
    let Some(executable) = executable("dciodvfy") else {
        eprintln!("dciodvfy unavailable; independent P2 IOD evidence is environment-unavailable");
        return;
    };
    let executable = fs::canonicalize(executable).unwrap();
    assert_eq!(
        synth_dicom_gen::sha256_hex(&fs::read(&executable).unwrap()),
        locked_dciodvfy_sha256(),
        "the independent validator must match conformance/validator-lock.json"
    );
    qualify(
        &executable,
        "mono",
        "tests/fixtures/composition/valid/template-only.json",
    );
    qualify(
        &executable,
        "rgb",
        "tests/fixtures/composition/valid/rgb-template-only.json",
    );
}
