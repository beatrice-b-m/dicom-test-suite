use std::{
    fs,
    sync::atomic::{AtomicU64, Ordering},
};

use dicom_dictionary_std::tags;
use dicom_object::open_file;

use super::sequence_length_sc::SEQUENCE_LENGTH_SC_RECIPE;
use crate::{PreparedGenerationRun, generator::write_sequence_length_sc_case};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

#[test]
fn sequence_length_variants_preserve_raw_encoding_and_semantics() {
    let first = write_fixtures("first");
    let second = write_fixtures("second");

    for file_name in ["defined.dcm", "undefined.dcm"] {
        let first_path = first.join(file_name);
        let second_path = second.join(file_name);
        assert_eq!(
            fs::read(&first_path).expect("first DICOM should be readable"),
            fs::read(&second_path).expect("second DICOM should be readable")
        );
        let object = open_file(&first_path).expect("sequence length fixture should open");
        let items = object
            .element(tags::ANATOMIC_REGION_SEQUENCE)
            .expect("Anatomic Region Sequence should exist")
            .items()
            .expect("Anatomic Region Sequence should contain items");
        assert_eq!(items.len(), 1);
        assert_eq!(
            items[0]
                .element(tags::CODE_VALUE)
                .expect("Code Value should exist")
                .to_str()
                .expect("Code Value should decode"),
            "69536005"
        );
    }

    let defined = fs::read(first.join("defined.dcm")).expect("defined DICOM");
    let undefined = fs::read(first.join("undefined.dcm")).expect("undefined DICOM");
    let defined_offset = sequence_offset(&defined);
    let undefined_offset = sequence_offset(&undefined);
    assert_eq!(
        &defined[defined_offset + 8..defined_offset + 12],
        &[0x38, 0, 0, 0]
    );
    assert_eq!(
        &undefined[undefined_offset + 8..undefined_offset + 12],
        &[0xFF; 4]
    );
    assert_ne!(
        &defined[defined_offset + 12 + 56..defined_offset + 12 + 64],
        &[0xFE, 0xFF, 0xDD, 0xE0, 0, 0, 0, 0]
    );
    assert_eq!(
        &undefined[undefined_offset + 12 + 56..undefined_offset + 12 + 64],
        &[0xFE, 0xFF, 0xDD, 0xE0, 0, 0, 0, 0]
    );
}

fn write_fixtures(label: &str) -> std::path::PathBuf {
    let serial = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let out_dir = std::env::temp_dir().join(format!(
        "dicom-test-suite-sequence-length-{}-{label}-{serial}",
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
    let files = write_sequence_length_sc_case(
        &run,
        &case,
        SEQUENCE_LENGTH_SC_RECIPE,
        "standards-lock-fixture-sha256",
    )
    .expect("sequence length Secondary Capture fixtures should generate");
    assert_eq!(files.len(), 2);
    out_dir.join("metadata/sc/defined_undefined_sequence_lengths")
}

fn sequence_offset(bytes: &[u8]) -> usize {
    bytes
        .windows(6)
        .position(|window| window == [0x08, 0x00, 0x18, 0x22, b'S', b'Q'])
        .expect("Anatomic Region Sequence raw header should exist")
}
