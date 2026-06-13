use std::fs;

use serde_json::Value;

#[test]
fn taxonomy_documents_all_supported_profiles() {
    let taxonomy =
        fs::read_to_string("cases/taxonomy.md").expect("case taxonomy document must be readable");

    for profile in [
        "smoke", "core", "extended", "legacy", "stress", "all", "negative", "fuzz",
    ] {
        assert!(
            taxonomy.contains(&format!("`{profile}`")),
            "taxonomy must document the {profile} profile"
        );
    }
}

#[test]
fn taxonomy_documents_canonical_case_id_order() {
    let taxonomy =
        fs::read_to_string("cases/taxonomy.md").expect("case taxonomy document must be readable");

    assert!(
        taxonomy.contains("<domain>/<iod_family>/<descriptor>"),
        "taxonomy must document the normalized case ID shape"
    );
    assert!(
        taxonomy.contains("classic/ct/mono2_i16_rescale_12bit_explicit_le"),
        "taxonomy must include a canonical classic CT example"
    );
    assert!(
        taxonomy.contains("Do not invert segments"),
        "taxonomy must reject non-canonical segment ordering"
    );
}

#[test]
fn registry_contains_initial_smoke_and_core_cases() {
    let registry = read_json("cases/registry.json");
    let cases = registry
        .get("cases")
        .and_then(Value::as_array)
        .expect("registry must contain cases");

    for (case_id, expected_status) in [
        ("classic/sc/mono2_u8_explicit_le", "implemented"),
        ("classic/sc/mono1_u8_explicit_le", "implemented"),
        ("classic/sc/rgb_planar0_explicit_le", "implemented"),
        ("classic/sc/mono2_u16_explicit_le", "implemented"),
        ("classic/sc/mono2_i16_explicit_le", "implemented"),
        ("classic/ct/mono2_i16_rescale_12bit_explicit_le", "planned"),
        (
            "classic/mg/for_presentation_mono1_u16_12bit_explicit_le",
            "planned",
        ),
        (
            "classic/mg/for_processing_mono2_u16_12bit_implicit_le",
            "planned",
        ),
        ("classic/cr/overlay_modality_voi_explicit_le", "planned"),
        ("classic/mr/multislice_oblique_explicit_le", "planned"),
        ("vl/photo/rgb_planar0_explicit_le", "planned"),
        ("vl/photo/palette_color_explicit_le", "planned"),
    ] {
        let case = cases
            .iter()
            .find(|case| case.get("case_id").and_then(Value::as_str) == Some(case_id))
            .unwrap_or_else(|| panic!("registry must contain {case_id}"));

        assert_eq!(
            case.get("status").and_then(Value::as_str),
            Some(expected_status),
            "{case_id} must have the expected implementation status"
        );
        assert!(
            case.get("standards_evidence")
                .and_then(Value::as_array)
                .is_some_and(|evidence| !evidence.is_empty()),
            "{case_id} must include standards evidence"
        );
    }
}

#[test]
fn transfer_syntax_matrix_records_required_capability_fields() {
    let matrix = read_json("transfer-syntax/capability-matrix.json");
    let entries = matrix
        .get("entries")
        .and_then(Value::as_array)
        .expect("transfer syntax matrix must contain entries");

    for uid in ["1.2.840.10008.1.2", "1.2.840.10008.1.2.1"] {
        let entry = entries
            .iter()
            .find(|entry| entry.get("uid").and_then(Value::as_str) == Some(uid))
            .unwrap_or_else(|| panic!("transfer syntax matrix must contain {uid}"));

        for field in [
            "read_dataset",
            "decode_pixel",
            "write_dataset",
            "encode_pixel",
            "feature_flags",
            "external_libraries",
            "determinism",
        ] {
            assert!(
                entry.get(field).is_some(),
                "transfer syntax {uid} must record {field}"
            );
        }
    }
}

#[test]
fn deterministic_policy_documents_all_determinism_levels() {
    let policy = fs::read_to_string("docs/deterministic-build-policy.md")
        .expect("deterministic build policy must be readable");

    for level in ["byte_stable", "semantic_stable", "unstable"] {
        assert!(
            policy.contains(&format!("`{level}`")),
            "deterministic policy must document {level}"
        );
    }

    assert!(
        policy.contains("2.25.<decimal uuid>"),
        "deterministic policy must document UID derivation format"
    );
    assert!(
        policy.contains("two-run smoke reproducibility check"),
        "deterministic policy must document the smoke reproducibility check"
    );
}

#[test]
fn kb_integration_workflow_documents_2026b_reference_policy() {
    let workflow = fs::read_to_string("standards/kb-integration.md")
        .expect("standards KB integration workflow must be readable");

    for required_text in [
        "dicom-standard-kb",
        "standards.lock.json",
        "2026b",
        "dicom_lookup_uid",
        "dicom_lookup_sop_class",
        "source_manifest_sha256",
        "9959bee76fd293c7eda3fc81ce2ced7528612faa1b2df28cccd01504a83f54b0",
        "Do not commit official",
    ] {
        assert!(
            workflow.contains(required_text),
            "KB integration workflow must mention {required_text}"
        );
    }
}

#[test]
fn standards_gap_workflow_documents_source_note_and_registry_policy() {
    let workflow = fs::read_to_string("standards/gap-workflow.md")
        .expect("standards gap workflow must be readable");
    let source_notes_readme = fs::read_to_string("standards/source-notes/README.md")
        .expect("source notes README must be readable");

    for required_text in [
        "2026b",
        "dicom-standard-kb",
        "standards/source-notes/",
        "KB patch",
        "blocked",
        "skipped",
        "cases/registry.json",
        "Do not fill a gap from memory",
    ] {
        assert!(
            workflow.contains(required_text),
            "standards gap workflow must mention {required_text}"
        );
    }

    for required_text in [
        "Note Template",
        "Affected Project Surface",
        "Required Decision",
        "KB Query",
        "Official Source Evidence",
        "Project Action",
        "Should become KB patch",
    ] {
        assert!(
            source_notes_readme.contains(required_text),
            "source notes README must mention {required_text}"
        );
    }
}

fn read_json(path: &str) -> Value {
    let contents =
        fs::read_to_string(path).unwrap_or_else(|err| panic!("failed to read {path}: {err}"));
    serde_json::from_str(&contents).unwrap_or_else(|err| panic!("failed to parse {path}: {err}"))
}
