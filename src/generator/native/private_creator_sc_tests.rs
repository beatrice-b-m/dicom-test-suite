use std::{
    fs,
    sync::atomic::{AtomicU64, Ordering},
};

use dicom_core::{DataElement, PrimitiveValue, Tag, VR};
use dicom_object::{InMemDicomObject, open_file};

use super::private_creator_sc::{PRIVATE_CREATOR_SC_RECIPE, PrivateValue};
use crate::{
    PreparedGenerationRun,
    generator::{put_private_creator_blocks, write_private_creator_sc_case},
};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

#[test]
fn private_creator_dataset_is_exact_and_byte_stable() {
    let first = write_fixture("first");
    let second = write_fixture("second");
    assert_eq!(
        fs::read(&first).expect("first DICOM file should be readable"),
        fs::read(&second).expect("second DICOM file should be readable")
    );

    let object = open_file(&first).expect("generated private creator fixture should open");
    for block in PRIVATE_CREATOR_SC_RECIPE.blocks {
        let creator = object
            .element(block.creator_tag)
            .expect("private creator should exist");
        assert_eq!(creator.vr(), VR::LO);
        assert_eq!(
            creator.to_str().expect("creator should decode"),
            block.creator_id
        );
        for element in block.elements {
            let actual = object
                .element(element.tag)
                .expect("private element should exist");
            match element.value {
                PrivateValue::Lo(value) => {
                    assert_eq!(actual.vr(), VR::LO);
                    assert_eq!(actual.to_str().expect("LO should decode"), value);
                }
                PrivateValue::Us(value) => {
                    assert_eq!(actual.vr(), VR::US);
                    assert_eq!(actual.to_int::<u16>().expect("US should decode"), value);
                }
            }
        }
    }
}

#[test]
fn private_creator_insertion_rejects_occupied_creator_tag() {
    let mut object = InMemDicomObject::new_empty();
    object.put(DataElement::new(
        Tag(0x0011, 0x0010),
        VR::LO,
        PrimitiveValue::from("PREEXISTING"),
    ));
    let error = put_private_creator_blocks(&mut object, PRIVATE_CREATOR_SC_RECIPE)
        .expect_err("occupied creator slot must fail closed");
    assert!(error.contains("0011,0010 is already occupied"), "{error}");
}

fn write_fixture(label: &str) -> std::path::PathBuf {
    let serial = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let out_dir = std::env::temp_dir().join(format!(
        "dicom-test-suite-private-creator-{}-{label}-{serial}",
        std::process::id()
    ));
    fs::create_dir_all(&out_dir).expect("temporary output directory should be created");
    let run = PreparedGenerationRun {
        profile: "core".to_string(),
        manifest_path: out_dir.join("manifest.json"),
        out_dir: out_dir.clone(),
        seed: 43,
        include_stress: false,
    };
    let case = serde_json::json!({ "standards_evidence": [] });
    write_private_creator_sc_case(
        &run,
        &case,
        PRIVATE_CREATOR_SC_RECIPE,
        "standards-lock-fixture-sha256",
    )
    .expect("private creator Secondary Capture should generate");
    out_dir.join("metadata/sc/private_creator_blocks/instance.dcm")
}
