use std::{
    fs,
    sync::atomic::{AtomicU64, Ordering},
};

use dicom_object::open_file;

use super::empty_type2_sc::EMPTY_TYPE2_SC_RECIPE;
use crate::{PreparedGenerationRun, generator::write_empty_type2_sc_case};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

#[test]
fn empty_type2_dataset_is_exact_and_byte_stable() {
    let first = write_fixture("first");
    let second = write_fixture("second");

    assert_eq!(
        fs::read(&first).expect("first DICOM file should be readable"),
        fs::read(&second).expect("second DICOM file should be readable"),
        "identical standards lock, recipe, and seed must produce identical Part 10 bytes"
    );

    let object = open_file(&first).expect("generated empty Type 2 DICOM file should open");
    for attribute in EMPTY_TYPE2_SC_RECIPE.attributes {
        let element = object
            .element(attribute.tag)
            .unwrap_or_else(|_| panic!("{} should be present", attribute.keyword));
        assert_eq!(element.vr(), attribute.vr, "{} VR", attribute.keyword);
        assert_eq!(
            element
                .to_bytes()
                .unwrap_or_else(|_| panic!("{} should decode", attribute.keyword))
                .len(),
            0,
            "{} should have an empty value",
            attribute.keyword
        );
    }
}

fn write_fixture(label: &str) -> std::path::PathBuf {
    let serial = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let out_dir = std::env::temp_dir().join(format!(
        "dicom-test-suite-empty-type2-{}-{label}-{serial}",
        std::process::id()
    ));
    fs::create_dir_all(&out_dir).expect("temporary output directory should be created");
    let run = PreparedGenerationRun {
        profile: "core".to_string(),
        manifest_path: out_dir.join("manifest.json"),
        out_dir: out_dir.clone(),
        seed: 41,
        include_stress: false,
    };
    let case = serde_json::json!({ "standards_evidence": [] });

    write_empty_type2_sc_case(
        &run,
        &case,
        EMPTY_TYPE2_SC_RECIPE,
        "standards-lock-fixture-sha256",
    )
    .expect("empty Type 2 Secondary Capture should generate");

    out_dir.join("metadata/sc/empty_type2_attributes/instance.dcm")
}
