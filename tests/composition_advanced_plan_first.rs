use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use synth_dicom_gen::composition::{ComposeOptions, compose};

static NEXT: AtomicU64 = AtomicU64::new(0);

fn output_path(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "dts-composition-advanced-plan-first-{label}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ))
}

#[test]
fn advanced_default_planning_is_output_free_for_image_and_reference_groups() {
    for (label, fixture) in [
        (
            "enhanced",
            "tests/fixtures/composition/valid/enhanced-defaults.json",
        ),
        (
            "references",
            "tests/fixtures/composition/valid/registration-presentation-defaults.json",
        ),
        ("wsi", "tests/fixtures/composition/valid/wsi-defaults.json"),
    ] {
        let out = output_path(label);
        assert!(!out.exists());
        let (_summary, plan) = compose(&ComposeOptions {
            spec_path: fixture.into(),
            out_dir: out.clone(),
            seed: 73,
            catalog_path: "templates/catalog.json".into(),
            dry_run: true,
        })
        .unwrap();
        assert!(
            plan["plans"]
                .as_array()
                .is_some_and(|plans| !plans.is_empty())
        );
        assert!(!out.exists(), "planning created {}", out.display());
    }
}

#[test]
fn advanced_default_adapter_has_no_writer_or_readback_boundary() {
    let source = include_str!("../src/composition/advanced_defaults.rs");
    for forbidden in [
        "crate::generator",
        "write_composition_default_artifacts",
        "open_file",
        "resolved_plan_from_curated_dataset",
        "std::fs",
    ] {
        assert!(
            !source.contains(forbidden),
            "advanced default adapter contains forbidden boundary {forbidden}"
        );
    }
}
