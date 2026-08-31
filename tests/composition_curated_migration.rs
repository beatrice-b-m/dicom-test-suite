use std::collections::{BTreeMap, BTreeSet};
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
    let prepared_backend = PathBuf::from("generation-backends/highdicom-pydicom/.venv/bin/python");
    let mut command = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"));
    if prepared_backend.is_file() {
        command.env(
            "DTS_HIGHDICOM_PYTHON",
            fs::canonicalize(&prepared_backend).unwrap(),
        );
    }
    let result = command
        .args([
            "generate",
            "--profile",
            "all",
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
    let p6 = |case_id: &str| {
        case_id.starts_with("derived/seg/")
            || case_id.starts_with("derived/parametric-map/")
            || case_id.starts_with("derived/rwvm/")
            || case_id.starts_with("derived/sr/")
            || case_id.starts_with("non-image/rt/")
            || case_id.starts_with("non-image/waveform/")
            || case_id == "non-image/encapsulated-document/pdf_minimal_explicit_le"
            || case_id == "derived/mesh/encapsulated_stl"
    };
    let mut observed_classic = 0;
    let mut observed_p5 = 0;
    let mut observed_p6 = 0;
    for entry in entries {
        let case_id = entry["case_id"].as_str().unwrap();
        assert_eq!(entry["corpus_plan_sha256"].as_str().unwrap().len(), 64);
        if entry.get("validity").and_then(serde_json::Value::as_str) != Some("expected_invalid") {
            assert_eq!(entry["resolved_plan_sha256"].as_str().unwrap().len(), 64);
        }
        if !classic(case_id) && !p5(case_id) && !p6(case_id) {
            continue;
        }
        observed_classic += usize::from(classic(case_id));
        observed_p5 += usize::from(p5(case_id));
        observed_p6 += usize::from(p6(case_id));
        let internal = entry["validation"]["internal"].as_array().unwrap();
        assert!(
            internal.iter().any(|check| {
                check["name"] == "curated_composition_plan" && check["status"] == "passed"
            }),
            "{case_id}"
        );
        let bytes = fs::read(root.join(entry["path"].as_str().unwrap())).unwrap();
        assert_eq!(
            entry["sha256"],
            dicom_test_suite::sha256_hex(&bytes),
            "{case_id}"
        );
        assert_eq!(entry["size_bytes"], bytes.len(), "{case_id}");
        if p5(case_id) || p6(case_id) {
            let object =
                dicom_object::open_file(root.join(entry["path"].as_str().unwrap())).unwrap();
            assert_eq!(
                entry["uids"]["implementation_version_name"],
                object
                    .meta()
                    .implementation_version_name
                    .as_deref()
                    .unwrap()
                    .trim_end(),
                "{case_id}"
            );
        }
        if p5(case_id) || p6(case_id) {
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
    assert!(observed_p6 > 0);
    let emitted = entries
        .iter()
        .filter_map(|entry| entry["case_id"].as_str())
        .collect::<BTreeSet<_>>();
    let skipped = manifest["skipped_cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|entry| Some((entry["case_id"].as_str()?, entry["reason_code"].as_str()?)))
        .collect::<BTreeMap<_, _>>();
    let registry: serde_json::Value =
        serde_json::from_slice(&fs::read("cases/registry.json").unwrap()).unwrap();
    for case in registry["cases"].as_array().unwrap() {
        let case_id = case["case_id"].as_str().unwrap();
        let selected_by_all = case["profiles"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(serde_json::Value::as_str)
            .any(|profile| matches!(profile, "smoke" | "core" | "extended"));
        if case["status"] != "implemented" || !selected_by_all || !p6(case_id) {
            continue;
        }
        if emitted.contains(case_id) {
            continue;
        }
        let reason = skipped
            .get(case_id)
            .unwrap_or_else(|| panic!("implemented selected P6 case {case_id} is unaccounted"));
        assert!(
            matches!(
                *reason,
                "external_backend_unavailable" | "feature_gated_case_unavailable"
            ),
            "implemented selected P6 case {case_id} has unexpected unavailable reason {reason}"
        );
    }
    if !cfg!(feature = "deflate") {
        assert!(
            manifest["skipped_cases"]
                .as_array()
                .unwrap()
                .iter()
                .any(|entry| {
                    entry["case_id"] == "derived/seg/binary_multiframe_deflated_image_frame"
                        && entry["reason_code"] == "feature_gated_case_unavailable"
                })
        );
    }
    fs::remove_dir_all(root).unwrap();
}
