use std::{
    fs,
    sync::atomic::{AtomicU64, Ordering},
};

use dicom_dictionary_std::tags;
use dicom_object::open_file;

use super::string_boundary_sc::STRING_BOUNDARY_SC_RECIPE;
use crate::{PreparedGenerationRun, generator::write_string_boundary_sc_case};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

#[test]
fn string_boundary_dataset_is_exact_and_byte_stable() {
    let first = write_fixture("first");
    let second = write_fixture("second");
    assert_eq!(
        fs::read(&first).expect("first DICOM file should be readable"),
        fs::read(&second).expect("second DICOM file should be readable")
    );

    let object = open_file(&first).expect("generated string boundary fixture should open");
    let recipe = STRING_BOUNDARY_SC_RECIPE;
    for (tag, values) in [
        (
            tags::IMAGE_COMMENTS,
            vec![
                recipe
                    .image_comments_pattern
                    .repeat(recipe.image_comments_repetitions),
            ],
        ),
        (
            tags::SOFTWARE_VERSIONS,
            recipe.software_versions.map(str::to_string).to_vec(),
        ),
        (
            tags::PIXEL_SPACING,
            recipe.pixel_spacing.map(str::to_string).to_vec(),
        ),
        (
            tags::ACQUISITION_NUMBER,
            vec![recipe.acquisition_number.to_string()],
        ),
    ] {
        let actual = object
            .element(tag)
            .expect("string boundary element should exist")
            .to_multi_str()
            .expect("string boundary element should decode")
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        assert_eq!(actual, values);
    }
}

fn write_fixture(label: &str) -> std::path::PathBuf {
    let serial = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let out_dir = std::env::temp_dir().join(format!(
        "dicom-test-suite-string-boundary-{}-{label}-{serial}",
        std::process::id()
    ));
    fs::create_dir_all(&out_dir).expect("temporary output directory should be created");
    let run = PreparedGenerationRun {
        profile: "extended".to_string(),
        manifest_path: out_dir.join("manifest.json"),
        out_dir: out_dir.clone(),
        seed: 43,
        include_stress: false,
    };
    let case = serde_json::json!({ "standards_evidence": [] });
    write_string_boundary_sc_case(
        &run,
        &case,
        STRING_BOUNDARY_SC_RECIPE,
        "standards-lock-fixture-sha256",
    )
    .expect("string boundary Secondary Capture should generate");
    out_dir.join("metadata/sc/long_multivalue_text_numeric_strings/instance.dcm")
}
