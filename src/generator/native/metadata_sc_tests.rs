use std::{
    fs,
    sync::atomic::{AtomicU64, Ordering},
};

use dicom_dictionary_std::{tags, uids};
use dicom_object::open_file;

use super::metadata_sc::METADATA_SC_RECIPES;
use crate::{PreparedGenerationRun, generator::write_metadata_sc_case};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

#[test]
fn utf8_person_name_dataset_is_exact_and_byte_stable() {
    let first = write_fixture("first");
    let second = write_fixture("second");

    assert_eq!(
        fs::read(&first).expect("first DICOM file should be readable"),
        fs::read(&second).expect("second DICOM file should be readable"),
        "identical standards lock, recipe, and seed must produce identical Part 10 bytes"
    );

    let object = open_file(&first).expect("generated UTF-8 DICOM file should open");
    assert_eq!(
        object.meta().transfer_syntax().trim_end_matches('\0'),
        "1.2.840.10008.1.2.1"
    );
    assert_eq!(
        object
            .element(tags::SOP_CLASS_UID)
            .expect("SOP Class UID should be present")
            .to_str()
            .expect("SOP Class UID should decode")
            .trim_end_matches('\0'),
        uids::SECONDARY_CAPTURE_IMAGE_STORAGE
    );
    assert_eq!(
        object
            .element(tags::SPECIFIC_CHARACTER_SET)
            .expect("Specific Character Set should be present")
            .to_str()
            .expect("Specific Character Set should decode")
            .as_ref(),
        "ISO_IR 192"
    );
    assert_eq!(
        object
            .element(tags::PATIENT_NAME)
            .expect("Patient Name should be present")
            .to_str()
            .expect("Patient Name should decode")
            .as_ref(),
        "Wang^XiaoDong=王^小東"
    );
    assert_eq!(
        object
            .element(tags::PIXEL_DATA)
            .expect("Pixel Data should be present")
            .to_bytes()
            .expect("native Pixel Data should decode")
            .as_ref(),
        &[0, 85, 170, 255]
    );
}

fn write_fixture(label: &str) -> std::path::PathBuf {
    let serial = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let out_dir = std::env::temp_dir().join(format!(
        "dicom-test-suite-metadata-sc-{}-{label}-{serial}",
        std::process::id()
    ));
    fs::create_dir_all(&out_dir).expect("temporary output directory should be created");
    let run = PreparedGenerationRun {
        profile: "core".to_string(),
        manifest_path: out_dir.join("manifest.json"),
        out_dir: out_dir.clone(),
        seed: 37,
        include_stress: false,
    };
    let recipe = METADATA_SC_RECIPES[0];
    let case = serde_json::json!({ "standards_evidence": [] });

    write_metadata_sc_case(&run, &case, recipe, "standards-lock-fixture-sha256")
        .expect("UTF-8 metadata Secondary Capture should generate");

    out_dir.join("metadata/sc/utf8_person_name/instance.dcm")
}
