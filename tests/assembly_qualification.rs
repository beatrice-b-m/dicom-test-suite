use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use dicom_core::Tag;
use dicom_dictionary_std::tags;
use dicom_object::open_file;
use synth_dicom_gen::assembly::{AssembleOptions, assemble};
use synth_dicom_gen::engine_resources::EngineResources;
use synth_dicom_gen::executor::cancellation::CancellationToken;
use synth_dicom_gen::validate_generated_root;

static NEXT: AtomicU64 = AtomicU64::new(0);

fn output(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "dicom-test-suite-assembly-qualification-{label}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ))
}

fn run(request: &[u8], root: PathBuf, parallelism: u32) {
    assemble(
        &AssembleOptions {
            request_bytes: request.to_vec(),
            caller_asset_root: PathBuf::from("."),
            output_root: root,
            seed: 17,
            parallelism,
            dry_run: false,
        },
        &CancellationToken::new(),
        &EngineResources::embedded(),
    )
    .unwrap();
}

#[test]
fn assembly_materializes_primitive_private_sequence_and_reference_contracts() {
    let request = br#"{
      "assembly_request_schema_version":"1.0.0",
      "instances":[
        {"instance_id":"source","sop_class_uid":"1.2.840.10008.5.1.4.1.1.7","transfer_syntax_uid":"1.2.840.10008.1.2.1","elements":[
          {"address":{"keyword":"PatientName"},"value":{"kind":"string","value":"SYNTHETIC^STRUCTURAL"}},
          {"address":{"keyword":"ImageType"},"value":{"kind":"strings","values":["ORIGINAL","PRIMARY"]}},
          {"address":{"keyword":"PatientID"},"value":{"kind":"empty"}},
          {"address":{"tag":"7776,0010"},"vr":"UL","value":{"kind":"integers","values":[1,4294967295]}},
          {"address":{"tag":"7776,0012"},"vr":"FD","value":{"kind":"floats","values":[1.5,-2.25]}},
          {"address":{"tag":"7776,0014"},"vr":"AT","value":{"kind":"tags","values":["0010,0010","7FE0,0010"]}},
          {"address":{"tag":"7776,0016"},"vr":"OB","value":{"kind":"bytes","base64":"AAECAw=="}},
          {"address":{"private_group":"0011","private_creator":"DTS_ONE","private_offset":"10"},"vr":"LO","value":{"kind":"string","value":"ONE"}},
          {"address":{"private_group":"0011","private_creator":"DTS_TWO","private_offset":"10"},"vr":"LO","value":{"kind":"string","value":"TWO"}},
          {"address":{"keyword":"ReferencedImageSequence"},"value":{"kind":"sequence","items":[{"elements":[
            {"address":{"keyword":"ReferencedSOPClassUID"},"value":{"kind":"string","value":"1.2.840.10008.5.1.4.1.1.7"}},
            {"address":{"keyword":"ReferencedSOPInstanceUID"},"value":{"kind":"string","value":"1.2.3.4.5"}},
            {"address":{"keyword":"PatientID"},"value":{"kind":"empty"}}
          ]}]}}
        ],"references":[{"relationship":"derived_from","target_instance_id":"target","target_role":"sop"}]},
        {"instance_id":"target","sop_class_uid":"1.2.840.10008.5.1.4.1.1.7","transfer_syntax_uid":"1.2.840.10008.1.2","identity":{"study_instance_uid":"1.2.3.10","series_instance_uid":"1.2.3.11","sop_instance_uid":"1.2.3.12","frame_of_reference_uid":"1.2.3.13"},"elements":[]}
      ]
    }"#;
    let root = output("elements");
    run(request, root.clone(), 2);
    let object = open_file(root.join("instances/source.dcm")).unwrap();
    assert_eq!(
        object
            .element(tags::PATIENT_NAME)
            .unwrap()
            .to_str()
            .unwrap(),
        "SYNTHETIC^STRUCTURAL"
    );
    assert_eq!(
        object.element(tags::IMAGE_TYPE).unwrap().to_str().unwrap(),
        "ORIGINAL\\PRIMARY"
    );
    assert!(
        object
            .element(tags::PATIENT_ID)
            .unwrap()
            .to_bytes()
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        object
            .element(Tag(0x7776, 0x0010))
            .unwrap()
            .to_multi_int::<u32>()
            .unwrap(),
        [1, u32::MAX]
    );
    assert_eq!(
        object
            .element(Tag(0x7776, 0x0012))
            .unwrap()
            .to_multi_float64()
            .unwrap(),
        [1.5, -2.25]
    );
    assert_eq!(
        object
            .element(Tag(0x7776, 0x0016))
            .unwrap()
            .to_bytes()
            .unwrap()
            .as_ref(),
        [0, 1, 2, 3]
    );
    assert_eq!(
        object
            .element(Tag(0x0011, 0x1010))
            .unwrap()
            .to_str()
            .unwrap(),
        "ONE"
    );
    assert_eq!(
        object
            .element(Tag(0x0011, 0x1110))
            .unwrap()
            .to_str()
            .unwrap(),
        "TWO"
    );
    let sequence = object
        .element(tags::REFERENCED_IMAGE_SEQUENCE)
        .unwrap()
        .items()
        .unwrap();
    assert_eq!(sequence.len(), 1);
    assert_eq!(
        sequence[0]
            .element(tags::REFERENCED_SOP_INSTANCE_UID)
            .unwrap()
            .to_str()
            .unwrap(),
        "1.2.3.4.5"
    );
    assert!(
        sequence[0]
            .element(tags::PATIENT_ID)
            .unwrap()
            .to_bytes()
            .unwrap()
            .is_empty()
    );

    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(
        manifest["instances"][0]["references"][0]["target_instance_id"],
        "target"
    );
    assert_eq!(
        manifest["instances"][0]["references"][0]["referenced_sop_instance_uid"],
        "1.2.3.12"
    );
    assert_eq!(
        manifest["instances"][0]["identity"]["provenance"]["sop"],
        "deterministic"
    );
    assert_eq!(
        manifest["instances"][1]["identity"]["provenance"]["sop"],
        "explicit"
    );
    let private_creators = manifest["instances"][0]["elements"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|element| element["address"]["private_creator"].as_str())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        private_creators,
        ["DTS_ONE", "DTS_TWO"].into_iter().collect()
    );
    let private_elements = manifest["instances"][0]["elements"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|element| element["address"]["private_creator"].is_string())
        .map(|element| element["address"]["element"].as_u64().unwrap())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(private_elements, [0x1010, 0x1110].into_iter().collect());
    assert!(open_file(root.join("instances/target.dcm")).is_ok());
    let validation = validate_generated_root(&root).unwrap();
    assert!(
        validation.failures.is_empty(),
        "qualified structural values and references must validate: {:?}",
        validation.failures
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn assembly_materializes_every_advertised_bulk_kind_deterministically() {
    let request = br#"{
      "assembly_request_schema_version":"1.0.0",
      "instances":[
        {"instance_id":"integer","sop_class_uid":"1.2.3.1","elements":[],"bulk":[{"kind":"integer_pixel_data","source":{"kind":"inline_base64","base64":"AAECAw=="},"rows":2,"columns":2,"bits_allocated":8,"bits_stored":8}]},
        {"instance_id":"float","sop_class_uid":"1.2.3.2","elements":[],"bulk":[{"kind":"float_pixel_data","source":{"kind":"inline_base64","base64":"AACAPw=="},"rows":1,"columns":1}]},
        {"instance_id":"double","sop_class_uid":"1.2.3.3","elements":[],"bulk":[{"kind":"double_float_pixel_data","source":{"kind":"inline_base64","base64":"AAAAAAAA8D8="},"rows":1,"columns":1}]},
        {"instance_id":"waveform","sop_class_uid":"1.2.3.4","elements":[],"bulk":[{"kind":"waveform_data","source":{"kind":"inline_base64","base64":"AAAAAAAAAAA="},"channels":2,"samples":2,"bits_allocated":16}]},
        {"instance_id":"document","sop_class_uid":"1.2.3.5","elements":[],"bulk":[{"kind":"encapsulated_document","source":{"kind":"inline_base64","base64":"JVBERi0xLjQ="},"media_type":"application/pdf"}]},
        {"instance_id":"mesh","sop_class_uid":"1.2.3.6","elements":[],"bulk":[{"kind":"mesh","source":{"kind":"inline_base64","base64":"AAAAAAAAAAAAAAAA"}}]},
        {"instance_id":"general","sop_class_uid":"1.2.3.7","transfer_syntax_uid":"1.2.840.10008.1.2","elements":[],"bulk":[{"kind":"general","tag":"7776,1000","vr":"OB","source":{"kind":"inline_base64","base64":"AQIDBA=="}}]}
      ]
    }"#;
    let serial = output("bulk-serial");
    let parallel = output("bulk-parallel");
    run(request, serial.clone(), 1);
    run(request, parallel.clone(), 8);

    for id in [
        "integer", "float", "double", "waveform", "document", "mesh", "general",
    ] {
        assert_eq!(
            fs::read(serial.join(format!("instances/{id}.dcm"))).unwrap(),
            fs::read(parallel.join(format!("instances/{id}.dcm"))).unwrap(),
            "{id} must be worker-count independent"
        );
    }
    let serial_manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(serial.join("manifest.json")).unwrap()).unwrap();
    let parallel_manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(parallel.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(
        serial_manifest["run"]["corpus_plan_sha256"],
        parallel_manifest["run"]["corpus_plan_sha256"]
    );
    assert_eq!(serial_manifest["instances"], parallel_manifest["instances"]);
    let values = [
        ("integer", Tag(0x7FE0, 0x0010), vec![0, 1, 2, 3]),
        ("float", Tag(0x7FE0, 0x0008), 1_f32.to_le_bytes().to_vec()),
        ("double", Tag(0x7FE0, 0x0009), 1_f64.to_le_bytes().to_vec()),
        ("waveform", Tag(0x5400, 0x1010), vec![0; 8]),
        ("document", Tag(0x0042, 0x0011), b"%PDF-1.4".to_vec()),
        ("mesh", Tag(0x0066, 0x0023), vec![0; 12]),
        ("general", Tag(0x7776, 0x1000), vec![1, 2, 3, 4]),
    ];
    for (id, tag, expected) in values {
        let object = open_file(serial.join(format!("instances/{id}.dcm"))).unwrap();
        assert_eq!(
            object.element(tag).unwrap().to_bytes().unwrap().as_ref(),
            expected
        );
    }
    for instance in serial_manifest["instances"].as_array().unwrap() {
        let bulk = &instance["bulk"][0];
        assert_eq!(bulk["sha256"], bulk["properties"]["resolved_sha256"]);
        assert_eq!(bulk["sha256"].as_str().unwrap().len(), 64);
        assert_ne!(
            bulk["sha256"], instance["sha256"],
            "artifact and bulk hashes must remain distinct evidence"
        );
    }
    assert!(
        validate_generated_root(&serial)
            .unwrap()
            .failures
            .is_empty()
    );
    assert!(
        validate_generated_root(&parallel)
            .unwrap()
            .failures
            .is_empty()
    );
    fs::remove_dir_all(serial).unwrap();
    fs::remove_dir_all(parallel).unwrap();
}
