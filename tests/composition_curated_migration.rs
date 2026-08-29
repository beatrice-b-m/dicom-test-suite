use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);

fn output(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "dts-composition-migration-{label}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ))
}

#[test]
fn migrated_classic_recipes_record_shared_plan_materialization() {
    let root = output("sc-vl");
    let result = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args([
            "generate",
            "--profile",
            "extended",
            "--out",
            root.to_str().unwrap(),
            "--seed",
            "1",
        ])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("manifest.json")).unwrap()).unwrap();
    let entries = manifest["files"].as_array().unwrap();
    let migrated = entries.iter().filter(|entry| {
        entry["case_id"].as_str().is_some_and(|case_id| {
            case_id.starts_with("classic/sc/")
                || case_id.starts_with("classic/cr/")
                || case_id.starts_with("classic/ct/")
                || case_id.starts_with("classic/dx/")
                || case_id.starts_with("classic/mg/")
                || case_id.starts_with("classic/mr/")
                || case_id.starts_with("geometry/ct/")
                || case_id.starts_with("metadata/sc/")
                || case_id.starts_with("encapsulation/sc/")
                || case_id.starts_with("vl/endoscopic/")
                || case_id.starts_with("vl/microscopic/")
                || case_id.starts_with("vl/photo/")
        })
    });
    let mut observed = 0;
    for entry in migrated {
        observed += 1;
        assert!(
            entry["validation"]["internal"]
                .as_array()
                .unwrap()
                .iter()
                .any(|check| {
                    check["name"] == "curated_composition_plan"
                        && check["status"] == "passed"
                }),
            "{}",
            entry["case_id"]
        );
    }
    assert!(observed > 0);
    fs::remove_dir_all(root).unwrap();
}
