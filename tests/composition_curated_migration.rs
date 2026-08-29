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
fn migrated_curated_recipes_record_shared_plan_materialization() {
    let root = output("classic-families");
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
    let classic = |case_id: &str| {
        case_id.starts_with("classic/sc/")
            || case_id.starts_with("classic/cr/")
            || case_id.starts_with("classic/ct/")
            || case_id.starts_with("classic/dx/")
            || case_id.starts_with("classic/mg/")
            || case_id.starts_with("classic/mr/")
            || case_id.starts_with("classic/nm/")
            || case_id.starts_with("classic/pet/")
            || case_id.starts_with("classic/us/")
            || case_id.starts_with("classic/xa/")
            || case_id.starts_with("classic/xrf/")
            || case_id.starts_with("geometry/ct/")
            || case_id.starts_with("metadata/sc/")
            || case_id.starts_with("encapsulation/sc/")
            || case_id.starts_with("vl/endoscopic/")
            || case_id.starts_with("vl/microscopic/")
            || case_id.starts_with("vl/photo/")
    };
    let p5 = |case_id: &str| {
        case_id.starts_with("enhanced/ct/")
            || case_id.starts_with("enhanced/mr/")
            || case_id.starts_with("enhanced/pet/")
            || case_id.starts_with("vl/wsi/")
            || matches!(
                case_id,
                "derived/registration/spatial_ct_pair"
                    | "derived/registration/deformable_ct_pair"
                    | "derived/presentation-state/grayscale_softcopy_ct_window_explicit_le"
                    | "derived/presentation-state/color_softcopy"
                    | "derived/presentation-state/blending"
                    | "derived/presentation-state/advanced_blending"
            )
    };
    let mut observed_classic = 0;
    let mut observed_p5 = 0;
    for entry in entries {
        let case_id = entry["case_id"].as_str().unwrap();
        if !classic(case_id) && !p5(case_id) {
            continue;
        }
        observed_classic += usize::from(classic(case_id));
        observed_p5 += usize::from(p5(case_id));
        let internal = entry["validation"]["internal"].as_array().unwrap();
        assert!(
            internal.iter().any(|check| {
                check["name"] == "curated_composition_plan" && check["status"] == "passed"
            }),
            "{case_id}"
        );
        if p5(case_id) {
            assert!(
                internal
                    .iter()
                    .any(|check| check["name"] != "curated_composition_plan"),
                "{case_id} must retain its pre-migration validation oracle"
            );
        }
    }
    assert!(observed_classic > 0);
    assert!(observed_p5 > 0);
    fs::remove_dir_all(root).unwrap();
}
