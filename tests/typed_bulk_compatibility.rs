pub use dicom_test_suite::{curated_validation, recipes, sha256_hex};

#[path = "../src/recipes/typed_bulk_compatibility.rs"]
#[allow(dead_code, unused_imports)]
mod typed_bulk_compatibility;

use std::collections::BTreeMap;

use dicom_test_suite::recipes::{
    CaseRecipe, EncapsulatedPayload, encapsulated_payload_input_from_recipe,
    waveform_input_from_recipe,
};
use serde_json::Value;
use typed_bulk_compatibility::{
    ObservedSpecializedContent, SpecializedValidationObservation, project_encapsulated_payload,
    project_waveform, validate_encapsulated_payload, validate_waveform,
};

const CASES: [(&str, &str, &str); 4] = [
    (
        "cases/recipes/non-image/waveform/non_image_waveform_twelve_lead_ecg.json",
        "e3696bf58c94ba4c9cfebcbc0c519a13badd40534df5d2c2ffd7002d16519918",
        "6070ee6baeecb27fc01887b6ede7f6fe73760615678438a0978fe53d3e2aad0f",
    ),
    (
        "cases/recipes/non-image/waveform/non_image_waveform_general_ecg.json",
        "bb3106edd3796b1574b23fd90a88d61e012b9b4e38b25407fa02af832faa0b66",
        "44a7ea47a58511fe3a5b364ffd882acdad19a48fa9324112bf3d69c7d8aac4c0",
    ),
    (
        "cases/recipes/non-image/encapsulated-document/encapsulated_pdf_minimal.json",
        "2f448638a434ee07dd347666944a99ab2066c171e25d0a5eb951c2b7e6cc985a",
        "cdf81edd6987c9b59c188358cbb8b13fcde9c5ccc22db67757bb5e67b9f17eeb",
    ),
    (
        "cases/recipes/derived/mesh/derived_mesh_encapsulated_stl.json",
        "1938f8275afd7af152a880ce20a402096eb81fcfcdaac1b1d25038d3a78be358",
        "a70bce244744683387f06b61b349809204cd2e5ea3cb9e25a38861faef3a9712",
    ),
];

#[test]
fn typed_projection_and_validation_match_frozen_legacy_digests() {
    for (path, expected_projection, expected_validation) in CASES {
        let recipe = load(path);
        let (projection, validation) =
            if let Some(input) = waveform_input_from_recipe(&recipe).unwrap() {
                let projection = project_waveform(&input).unwrap();
                let mut content = BTreeMap::new();
                for group in &input.groups {
                    content.insert(
                        group.slot.clone(),
                        ObservedSpecializedContent {
                            size_bytes: group.declared_size_bytes,
                            sha256: group.declared_sha256.clone(),
                            vr: "OW".into(),
                        },
                    );
                }
                let observed = observation(&input.sop_class_uid, content);
                (
                    projection.legacy_fields(),
                    validate_waveform(&input, &observed)
                        .unwrap()
                        .legacy_validation_json(),
                )
            } else {
                let input = encapsulated_payload_input_from_recipe(&recipe)
                    .unwrap()
                    .unwrap();
                let (size_bytes, sha256) = match &input.payload {
                    EncapsulatedPayload::MinimalPdf {
                        declared_size_bytes,
                        declared_sha256,
                        ..
                    }
                    | EncapsulatedPayload::ClosedTetrahedronBinaryStl {
                        declared_size_bytes,
                        declared_sha256,
                        ..
                    } => (*declared_size_bytes, declared_sha256.clone()),
                };
                let content = BTreeMap::from([(
                    "encapsulated_document".into(),
                    ObservedSpecializedContent {
                        size_bytes,
                        sha256,
                        vr: "OB".into(),
                    },
                )]);
                let observed = observation(&input.sop_class_uid, content);
                (
                    project_encapsulated_payload(&input)
                        .unwrap()
                        .legacy_fields(),
                    validate_encapsulated_payload(&input, &observed)
                        .unwrap()
                        .legacy_validation_json(),
                )
            };
        assert_eq!(
            canonical_sha256(&projection),
            expected_projection,
            "projection {path}"
        );
        let validation_digest = canonical_sha256(&validation);
        assert_eq!(validation_digest, expected_validation, "validation {path}");
    }
}

#[test]
fn adapters_reject_hash_size_vr_and_common_evidence_drift() {
    let recipe = load(CASES[0].0);
    let input = waveform_input_from_recipe(&recipe).unwrap().unwrap();
    let group = &input.groups[0];
    for content in [
        ObservedSpecializedContent {
            size_bytes: group.declared_size_bytes + 2,
            sha256: group.declared_sha256.clone(),
            vr: "OW".into(),
        },
        ObservedSpecializedContent {
            size_bytes: group.declared_size_bytes,
            sha256: "0".repeat(64),
            vr: "OW".into(),
        },
        ObservedSpecializedContent {
            size_bytes: group.declared_size_bytes,
            sha256: group.declared_sha256.clone(),
            vr: "OB".into(),
        },
    ] {
        let observed = observation(
            &input.sop_class_uid,
            BTreeMap::from([(group.slot.clone(), content)]),
        );
        assert!(validate_waveform(&input, &observed).is_err());
    }
    let mut observed = observation(
        &input.sop_class_uid,
        BTreeMap::from([(
            group.slot.clone(),
            ObservedSpecializedContent {
                size_bytes: group.declared_size_bytes,
                sha256: group.declared_sha256.clone(),
                vr: "OW".into(),
            },
        )]),
    );
    observed.generic_plan_validation_passed = false;
    assert!(validate_waveform(&input, &observed).is_err());
}

#[test]
fn compatibility_sources_are_frontend_writer_and_filesystem_free() {
    for source in [
        include_str!("../src/recipes/typed_bulk_compatibility.rs"),
        include_str!("../src/recipes/typed_bulk_compatibility/projection.rs"),
        include_str!("../src/recipes/typed_bulk_compatibility/validation.rs"),
    ] {
        for forbidden in [
            "generator::",
            "curated_manifest",
            "curated_execution",
            "open_file",
            "std::fs",
            "PathBuf",
            "output_root",
        ] {
            assert!(
                !source.contains(forbidden),
                "forbidden dependency {forbidden}"
            );
        }
    }
}

fn observation(
    sop_class_uid: &str,
    content: BTreeMap<String, ObservedSpecializedContent>,
) -> SpecializedValidationObservation {
    SpecializedValidationObservation {
        generic_plan_validation_passed: true,
        part10_preamble_present: true,
        transfer_syntax_uid: "1.2.840.10008.1.2.1".into(),
        sop_class_uid: sop_class_uid.into(),
        implementation_identity_matches: true,
        pixel_data_absent: true,
        content,
    }
}

fn load(path: &str) -> CaseRecipe {
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

fn canonical_sha256(value: &Value) -> String {
    sha256_hex(&serde_json::to_vec(&sort(value)).unwrap())
}

fn sort(value: &Value) -> Value {
    match value {
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(key, value)| (key.clone(), sort(value)))
                .collect::<BTreeMap<_, _>>()
                .into_iter()
                .collect(),
        ),
        Value::Array(values) => Value::Array(values.iter().map(sort).collect()),
        value => value.clone(),
    }
}
