use std::collections::BTreeSet;
use std::fs;
use std::process::Command;

use dicom_transfer_syntax_registry::{TransferSyntaxIndex, TransferSyntaxRegistry};
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
        ("classic/sc/rgb_planar1_explicit_le", "implemented"),
        ("classic/sc/palette_color_u8_explicit_le", "implemented"),
        ("classic/sc/ybr_full_planar0_explicit_le", "implemented"),
        ("classic/sc/ybr_full_422_explicit_le", "implemented"),
        ("classic/sc/mono2_u16_odd_3x3_explicit_le", "implemented"),
        ("classic/sc/mono2_u16_rect_2x3_explicit_le", "implemented"),
        ("classic/sc/mono2_u16_tiny_1x1_explicit_le", "implemented"),
        ("classic/sc/mono2_u16_padding_explicit_le", "implemented"),
        (
            "classic/ct/mono2_i16_rescale_12bit_explicit_le",
            "implemented",
        ),
        (
            "classic/mg/for_presentation_mono1_u16_12bit_explicit_le",
            "implemented",
        ),
        (
            "classic/mg/for_processing_mono2_u16_12bit_implicit_le",
            "implemented",
        ),
        ("classic/cr/overlay_modality_voi_explicit_le", "implemented"),
        ("classic/mr/multislice_oblique_explicit_le", "implemented"),
        (
            "classic/dx/display_shutter_mono2_u16_explicit_le",
            "implemented",
        ),
        ("classic/us/mono2_u8_explicit_le", "implemented"),
        (
            "enhanced/ct/multiframe_shared_perframe_explicit_le",
            "implemented",
        ),
        (
            "enhanced/ct/concatenation_two_part_explicit_le",
            "implemented",
        ),
        (
            "enhanced/mr/multiframe_echo_perframe_explicit_le",
            "implemented",
        ),
        (
            "enhanced/mr/multiframe_temporal_position_explicit_le",
            "implemented",
        ),
        (
            "enhanced/mr/multiframe_phase_velocity_encoding_explicit_le",
            "implemented",
        ),
        ("derived/seg/binary_multiframe_explicit_le", "implemented"),
        (
            "derived/seg/fractional_probability_multiframe_explicit_le",
            "implemented",
        ),
        ("derived/seg/labelmap_multiframe_explicit_le", "implemented"),
        (
            "derived/presentation-state/grayscale_softcopy_ct_window_explicit_le",
            "implemented",
        ),
        ("derived/rwvm/linear_ct_mapping_explicit_le", "implemented"),
        (
            "derived/sr/basic_text_observation_explicit_le",
            "implemented",
        ),
        (
            "derived/sr/comprehensive_measurement_explicit_le",
            "implemented",
        ),
        ("derived/sr/key_object_selection_explicit_le", "implemented"),
        (
            "non-image/rt/structure_set_single_roi_explicit_le",
            "implemented",
        ),
        ("non-image/rt/dose_grid_u16_explicit_le", "implemented"),
        (
            "non-image/encapsulated-document/pdf_minimal_explicit_le",
            "implemented",
        ),
        ("classic/sc/mono2_u8_deflated_explicit_le", "implemented"),
        ("classic/sc/rgb_planar0_jpeg_baseline_8bit", "implemented"),
        ("classic/sc/mono2_u16_rle_lossless", "implemented"),
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
fn implemented_registry_cases_match_generator_recipes() {
    let registry = read_json("cases/registry.json");
    let cases = registry_cases(&registry);
    let generator_case_ids = generator_recipe_case_ids();
    let implemented_registry_case_ids = cases
        .iter()
        .filter(|case| case.get("status").and_then(Value::as_str) == Some("implemented"))
        .map(|case| {
            case.get("case_id")
                .and_then(Value::as_str)
                .expect("registry case_id should be a string")
                .to_string()
        })
        .collect::<BTreeSet<_>>();

    let missing_recipes = implemented_registry_case_ids
        .difference(&generator_case_ids)
        .cloned()
        .collect::<Vec<_>>();
    assert!(
        missing_recipes.is_empty(),
        "implemented registry cases must have generator recipes: {missing_recipes:?}"
    );

    let orphan_recipes = generator_case_ids
        .difference(&implemented_registry_case_ids)
        .cloned()
        .collect::<Vec<_>>();
    assert!(
        orphan_recipes.is_empty(),
        "generator recipes must have implemented registry cases: {orphan_recipes:?}"
    );
}

#[test]
fn initial_priority_cases_are_represented_in_registry() {
    let registry = read_json("cases/registry.json");
    let cases = registry_cases(&registry);
    for case_id in [
        "classic/sc/mono2_u8_explicit_le",
        "classic/sc/mono1_u8_explicit_le",
        "classic/sc/rgb_planar0_explicit_le",
        "classic/ct/mono2_i16_rescale_12bit_explicit_le",
        "classic/mg/for_presentation_mono1_u16_12bit_explicit_le",
        "classic/mg/for_processing_mono2_u16_12bit_implicit_le",
        "classic/cr/overlay_modality_voi_explicit_le",
        "classic/mr/multislice_oblique_explicit_le",
        "enhanced/ct/multiframe_shared_perframe_explicit_le",
        "derived/seg/binary_multiframe_explicit_le",
        "vl/photo/rgb_planar0_explicit_le",
        "vl/photo/palette_color_explicit_le",
    ] {
        let case = cases
            .iter()
            .find(|case| case.get("case_id").and_then(Value::as_str) == Some(case_id))
            .unwrap_or_else(|| panic!("initial priority case {case_id} must be in registry"));
        assert!(
            matches!(
                case.get("status").and_then(Value::as_str),
                Some("implemented" | "planned" | "skipped" | "blocked" | "deprecated")
            ),
            "initial priority case {case_id} must have an explicit registry status"
        );
    }
}

#[test]
fn generated_payload_artifacts_are_not_tracked_or_staged() {
    let mut offenders = generated_payload_paths(git_paths(&["ls-files"]));
    offenders.extend(generated_payload_paths(git_paths(&[
        "diff",
        "--cached",
        "--name-only",
        "--diff-filter=ACMRT",
    ])));
    offenders.sort();
    offenders.dedup();

    assert!(
        offenders.is_empty(),
        "generated payload artifacts must not be tracked or staged: {offenders:?}"
    );
}

#[test]
fn transfer_syntax_matrix_records_required_capability_fields() {
    let matrix = read_json("transfer-syntax/capability-matrix.json");
    let entries = matrix
        .get("entries")
        .and_then(Value::as_array)
        .expect("transfer syntax matrix must contain entries");

    for entry in entries {
        let uid = entry
            .get("uid")
            .and_then(Value::as_str)
            .expect("transfer syntax matrix entry must contain uid");
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
fn codec_backend_decisions_cover_phase_zero_target_families() {
    let decisions = read_json("transfer-syntax/backend-decisions.json");
    assert_eq!(
        decisions.get("schema_version").and_then(Value::as_str),
        Some("0.1.0")
    );
    assert_eq!(
        decisions.get("dicom_rs_version").and_then(Value::as_str),
        Some("0.9.1"),
        "backend decisions must be tied to the pinned DICOM-rs version"
    );

    let families = decisions
        .get("codec_families")
        .and_then(Value::as_array)
        .expect("backend decisions must contain codec_families");

    for family_id in [
        "rle_lossless",
        "jpeg_baseline_8bit",
        "jpeg_ls",
        "jpeg_xl",
        "jpeg_2000",
        "htj2k",
        "legacy_jpeg",
        "deflated_image_frame",
    ] {
        let family = families
            .iter()
            .find(|family| family.get("family_id").and_then(Value::as_str) == Some(family_id))
            .unwrap_or_else(|| panic!("backend decisions must include {family_id}"));

        for field in [
            "classification",
            "transfer_syntax_uids",
            "selected_backend",
            "backend_kind",
            "determinism",
            "validation_strategy",
            "blockers",
            "evidence",
            "next_action",
        ] {
            assert!(
                family.get(field).is_some(),
                "{family_id} must record {field}"
            );
        }
        assert!(
            family
                .get("transfer_syntax_uids")
                .and_then(Value::as_array)
                .is_some_and(|uids| !uids.is_empty()),
            "{family_id} must name at least one transfer syntax UID"
        );
        assert!(
            family
                .get("validation_strategy")
                .and_then(Value::as_array)
                .is_some_and(|strategy| !strategy.is_empty()),
            "{family_id} must describe validation strategy"
        );
    }
}

#[test]
fn codec_backend_decisions_track_enabled_low_risk_codecs() {
    let decisions = read_json("transfer-syntax/backend-decisions.json");
    let families = decisions
        .get("codec_families")
        .and_then(Value::as_array)
        .expect("backend decisions must contain codec_families");

    let implement_now = families
        .iter()
        .filter(|family| {
            family.get("classification").and_then(Value::as_str) == Some("implement_now")
        })
        .map(|family| {
            family
                .get("family_id")
                .and_then(Value::as_str)
                .expect("family_id should be a string")
        })
        .collect::<Vec<_>>();
    assert_eq!(
        implement_now,
        vec!["rle_lossless", "jpeg_baseline_8bit", "jpeg_ls"],
        "enabled Phase 3 codec families should be explicit"
    );

    let rle = families
        .iter()
        .find(|family| family.get("family_id").and_then(Value::as_str) == Some("rle_lossless"))
        .expect("RLE decision must exist");
    assert_eq!(
        rle.get("selected_backend").and_then(Value::as_str),
        Some("native_project_rle_encoder")
    );
    assert_eq!(
        rle.get("backend_kind").and_then(Value::as_str),
        Some("native")
    );
    assert_eq!(
        rle.get("determinism").and_then(Value::as_str),
        Some("byte_stable")
    );
    assert_eq!(
        rle.get("first_case_target").and_then(Value::as_str),
        Some("classic/sc/mono2_u8_rle_lossless")
    );

    let jpeg = families
        .iter()
        .find(|family| {
            family.get("family_id").and_then(Value::as_str) == Some("jpeg_baseline_8bit")
        })
        .expect("JPEG Baseline decision must exist");
    assert_eq!(
        jpeg.get("selected_backend").and_then(Value::as_str),
        Some("dicom_rs_jpeg_baseline_writer")
    );
    assert_eq!(
        jpeg.get("backend_kind").and_then(Value::as_str),
        Some("dicom_rs_feature")
    );
    assert_eq!(
        jpeg.get("feature_gate").and_then(Value::as_str),
        Some("jpeg")
    );
    assert_eq!(
        jpeg.get("determinism").and_then(Value::as_str),
        Some("semantic_stable")
    );

    let jpeg_ls = families
        .iter()
        .find(|family| family.get("family_id").and_then(Value::as_str) == Some("jpeg_ls"))
        .expect("JPEG-LS decision must exist");
    assert_eq!(
        jpeg_ls.get("selected_backend").and_then(Value::as_str),
        Some("dicom_rs_charls_jpeg_ls_lossless_writer")
    );
    assert_eq!(
        jpeg_ls.get("backend_kind").and_then(Value::as_str),
        Some("dicom_rs_feature")
    );
    assert_eq!(
        jpeg_ls.get("feature_gate").and_then(Value::as_str),
        Some("charls")
    );
    assert_eq!(
        jpeg_ls.get("determinism").and_then(Value::as_str),
        Some("semantic_stable")
    );
    assert_eq!(
        jpeg_ls
            .get("transfer_syntax_uids")
            .and_then(Value::as_array)
            .expect("JPEG-LS transfer syntax UID list should exist")
            .iter()
            .filter_map(Value::as_str)
            .collect::<Vec<_>>(),
        vec!["1.2.840.10008.1.2.4.80"],
        "implemented JPEG-LS decision should be scoped to the lossless transfer syntax"
    );
    assert_eq!(
        jpeg_ls
            .pointer("/near_lossless_policy/classification")
            .and_then(Value::as_str),
        Some("defer"),
        "JPEG-LS Near-Lossless should have an explicit defer policy before JPEG XL work starts"
    );
    assert_eq!(
        jpeg_ls
            .pointer("/near_lossless_policy/transfer_syntax_uid")
            .and_then(Value::as_str),
        Some("1.2.840.10008.1.2.4.81")
    );
}

#[test]
fn codec_backend_decision_uids_are_known_to_capability_matrix_or_deferred() {
    let matrix = read_json("transfer-syntax/capability-matrix.json");
    let matrix_uids = matrix
        .get("entries")
        .and_then(Value::as_array)
        .expect("transfer syntax matrix must contain entries")
        .iter()
        .map(|entry| {
            entry
                .get("uid")
                .and_then(Value::as_str)
                .expect("matrix entry uid should be a string")
                .to_string()
        })
        .collect::<BTreeSet<_>>();

    let decisions = read_json("transfer-syntax/backend-decisions.json");
    let families = decisions
        .get("codec_families")
        .and_then(Value::as_array)
        .expect("backend decisions must contain codec_families");

    for family in families {
        let family_id = family
            .get("family_id")
            .and_then(Value::as_str)
            .expect("family_id should be a string");
        let classification = family
            .get("classification")
            .and_then(Value::as_str)
            .expect("classification should be a string");
        let uids = family
            .get("transfer_syntax_uids")
            .and_then(Value::as_array)
            .expect("transfer_syntax_uids should be an array");

        if classification == "implement_now" {
            assert!(
                uids.iter()
                    .filter_map(Value::as_str)
                    .any(|uid| matrix_uids.contains(uid)),
                "{family_id} must have at least one UID represented in the capability matrix before implementation"
            );
        }
    }
}

#[test]
fn deflated_transfer_syntax_is_feature_gated_in_cargo_and_registry() {
    let cargo_toml = fs::read_to_string("Cargo.toml").expect("Cargo.toml must be readable");
    assert!(
        cargo_toml.contains("deflate = ["),
        "Cargo.toml must expose the project deflate feature"
    );
    assert!(
        cargo_toml.contains("\"dicom-object/deflate\"")
            && cargo_toml.contains("\"dicom-transfer-syntax-registry/deflate\""),
        "project deflate feature must enable matching DICOM-rs deflate features"
    );

    let registry = read_json("cases/registry.json");
    let cases = registry_cases(&registry);
    let case = cases
        .iter()
        .find(|case| {
            case.get("case_id").and_then(Value::as_str)
                == Some("classic/sc/mono2_u8_deflated_explicit_le")
        })
        .expect("registry must contain the implemented deflated SC case");

    assert_eq!(
        case.get("status").and_then(Value::as_str),
        Some("implemented")
    );
    assert_eq!(
        case.get("transfer_syntax_uid").and_then(Value::as_str),
        Some("1.2.840.10008.1.2.1.99")
    );
    assert_eq!(
        case.pointer("/requirements/features/0")
            .and_then(Value::as_str),
        Some("deflate")
    );
    assert_eq!(
        case.get("determinism").and_then(Value::as_str),
        Some("byte_stable")
    );
}

#[test]
fn transfer_syntax_matrix_matches_dicom_rs_native_writer_support() {
    let matrix = read_json("transfer-syntax/capability-matrix.json");
    let entries = matrix
        .get("entries")
        .and_then(Value::as_array)
        .expect("transfer syntax matrix must contain entries");

    for uid in [
        "1.2.840.10008.1.2",
        "1.2.840.10008.1.2.1",
        "1.2.840.10008.1.2.2",
    ] {
        let entry = entries
            .iter()
            .find(|entry| entry.get("uid").and_then(Value::as_str) == Some(uid))
            .unwrap_or_else(|| panic!("transfer syntax matrix must contain {uid}"));
        let transfer_syntax = TransferSyntaxRegistry
            .get(uid)
            .unwrap_or_else(|| panic!("DICOM-rs registry must expose {uid}"));

        assert_eq!(
            entry.get("status").and_then(Value::as_str),
            Some("available"),
            "{uid} should be available in the matrix"
        );
        assert_eq!(
            entry.get("read_dataset").and_then(Value::as_bool),
            Some(transfer_syntax.can_decode_dataset()),
            "{uid} read_dataset should match DICOM-rs registry"
        );
        assert_eq!(
            entry.get("write_dataset").and_then(Value::as_bool),
            Some(transfer_syntax.encoder().is_some()),
            "{uid} write_dataset should match DICOM-rs registry"
        );
        assert_eq!(
            entry.get("encode_pixel").and_then(Value::as_bool),
            Some(!transfer_syntax.is_encapsulated_pixel_data()),
            "{uid} encode_pixel should reflect native uncompressed pixels"
        );
        assert!(
            transfer_syntax.is_codec_free(),
            "{uid} should not require a pixel codec"
        );
    }
}

#[test]
fn compressed_transfer_syntax_matrix_matches_current_dicom_rs_stubs() {
    let matrix = read_json("transfer-syntax/capability-matrix.json");
    let entries = matrix
        .get("entries")
        .and_then(Value::as_array)
        .expect("transfer syntax matrix must contain entries");

    for (uid, keyword, feature) in [
        (
            "1.2.840.10008.1.2.4.90",
            "JPEG2000Lossless",
            "dicom-transfer-syntax-registry/openjp2",
        ),
        (
            "1.2.840.10008.1.2.4.110",
            "JPEGXLLossless",
            "dicom-transfer-syntax-registry/jpegxl",
        ),
        (
            "1.2.840.10008.1.2.4.201",
            "HTJ2KLossless",
            "dicom-transfer-syntax-registry/openjp2",
        ),
    ] {
        let entry = entries
            .iter()
            .find(|entry| entry.get("uid").and_then(Value::as_str) == Some(uid))
            .unwrap_or_else(|| panic!("transfer syntax matrix must contain {uid}"));
        let transfer_syntax = TransferSyntaxRegistry
            .get(uid)
            .unwrap_or_else(|| panic!("DICOM-rs registry must expose {uid}"));

        assert_eq!(
            entry.get("keyword").and_then(Value::as_str),
            Some(keyword),
            "{uid} keyword should match PS3.6 evidence"
        );
        assert_eq!(
            entry.get("status").and_then(Value::as_str),
            Some("unavailable"),
            "{uid} should remain unavailable until compressed generation is verified"
        );
        assert_eq!(
            entry.get("read_dataset").and_then(Value::as_bool),
            Some(transfer_syntax.can_decode_dataset()),
            "{uid} read_dataset should match DICOM-rs registry"
        );
        assert_eq!(
            entry.get("write_dataset").and_then(Value::as_bool),
            Some(transfer_syntax.encoder().is_some()),
            "{uid} write_dataset should match DICOM-rs registry"
        );
        let runtime_can_decode_pixel = transfer_syntax.pixel_data_reader().is_some();
        let runtime_can_encode_pixel = transfer_syntax.pixel_data_writer().is_some();
        assert_eq!(
            entry.get("decode_pixel").and_then(Value::as_bool),
            Some(runtime_can_decode_pixel),
            "{uid} decode_pixel should match the current DICOM-rs feature set"
        );
        assert_eq!(
            entry.get("encode_pixel").and_then(Value::as_bool),
            Some(runtime_can_encode_pixel),
            "{uid} encode_pixel should match the current DICOM-rs feature set"
        );
        assert!(
            transfer_syntax.is_encapsulated_pixel_data(),
            "{uid} should use encapsulated Pixel Data"
        );
        assert!(
            entry
                .get("feature_flags")
                .and_then(Value::as_array)
                .is_some_and(|features| features
                    .iter()
                    .any(|value| value.as_str() == Some(feature))),
            "{uid} should record the DICOM-rs feature gate {feature}"
        );
    }
}

#[test]
fn jpeg_ls_lossless_transfer_syntax_has_project_charls_feature_gate() {
    let matrix = read_json("transfer-syntax/capability-matrix.json");
    let entries = matrix
        .get("entries")
        .and_then(Value::as_array)
        .expect("transfer syntax matrix must contain entries");
    let entry = entries
        .iter()
        .find(|entry| entry.get("uid").and_then(Value::as_str) == Some("1.2.840.10008.1.2.4.80"))
        .expect("transfer syntax matrix must contain JPEG-LS Lossless");
    let transfer_syntax = TransferSyntaxRegistry
        .get("1.2.840.10008.1.2.4.80")
        .expect("DICOM-rs registry must expose JPEG-LS Lossless");

    assert_eq!(
        entry.get("keyword").and_then(Value::as_str),
        Some("JPEGLSLossless")
    );
    assert_eq!(
        entry.get("status").and_then(Value::as_str),
        Some("feature_gated"),
        "JPEG-LS generation should be available when the project charls feature is enabled"
    );
    assert_eq!(
        entry.get("decode_pixel").and_then(Value::as_bool),
        Some(true),
        "the committed matrix should claim JPEG-LS decode validation behind the charls feature"
    );
    assert_eq!(
        entry.get("encode_pixel").and_then(Value::as_bool),
        Some(true),
        "the committed matrix should claim JPEG-LS encode validation behind the charls feature"
    );
    assert!(
        entry
            .get("feature_flags")
            .and_then(Value::as_array)
            .is_some_and(|features| features
                .iter()
                .any(|value| value.as_str() == Some("charls"))),
        "JPEG-LS should record the project charls feature gate"
    );
    if cfg!(feature = "charls") {
        assert!(
            transfer_syntax.pixel_data_reader().is_some(),
            "charls feature builds should expose a runtime JPEG-LS decoder"
        );
        assert!(
            transfer_syntax.pixel_data_writer().is_some(),
            "charls feature builds should expose a runtime JPEG-LS encoder"
        );
    }

    let registry = read_json("cases/registry.json");
    let cases = registry_cases(&registry);
    let case = cases
        .iter()
        .find(|case| {
            case.get("case_id").and_then(Value::as_str)
                == Some("classic/sc/mono2_u8_jpeg_ls_lossless")
        })
        .expect("registry must contain JPEG-LS Lossless SC case");
    assert_eq!(
        case.get("status").and_then(Value::as_str),
        Some("implemented")
    );
    assert_eq!(case.get("skip"), Some(&Value::Null));
    assert_eq!(
        case.pointer("/requirements/features/0")
            .and_then(Value::as_str),
        Some("charls")
    );
    assert_eq!(
        case.get("determinism").and_then(Value::as_str),
        Some("semantic_stable")
    );
}

#[test]
fn jpeg_ls_near_lossless_policy_is_deferred_until_lossy_semantics_are_defined() {
    let matrix = read_json("transfer-syntax/capability-matrix.json");
    let entries = matrix
        .get("entries")
        .and_then(Value::as_array)
        .expect("transfer syntax matrix must contain entries");
    let entry = entries
        .iter()
        .find(|entry| entry.get("uid").and_then(Value::as_str) == Some("1.2.840.10008.1.2.4.81"))
        .expect("transfer syntax matrix must contain JPEG-LS Near-Lossless");
    let transfer_syntax = TransferSyntaxRegistry
        .get("1.2.840.10008.1.2.4.81")
        .expect("DICOM-rs registry must expose JPEG-LS Near-Lossless");

    assert_eq!(
        entry.get("keyword").and_then(Value::as_str),
        Some("JPEGLSNearLossless")
    );
    assert_eq!(
        entry.get("status").and_then(Value::as_str),
        Some("unavailable"),
        "near-lossless generation should stay unavailable until lossy semantics are defined"
    );
    assert_eq!(
        entry.get("decode_pixel").and_then(Value::as_bool),
        Some(false),
        "the project matrix should not claim near-lossless decode validation yet"
    );
    assert_eq!(
        entry.get("encode_pixel").and_then(Value::as_bool),
        Some(false),
        "the project matrix should not claim near-lossless encode validation yet"
    );
    assert!(
        entry
            .get("feature_flags")
            .and_then(Value::as_array)
            .is_some_and(|features| features
                .iter()
                .any(|value| value.as_str() == Some("charls"))),
        "near-lossless should record the likely project feature gate"
    );
    if cfg!(feature = "charls") {
        assert!(
            transfer_syntax.pixel_data_reader().is_some(),
            "charls feature builds should expose a runtime JPEG-LS Near-Lossless decoder"
        );
        assert!(
            transfer_syntax.pixel_data_writer().is_some(),
            "charls feature builds should expose a runtime JPEG-LS Near-Lossless encoder candidate"
        );
    }

    let decisions = read_json("transfer-syntax/backend-decisions.json");
    let jpeg_ls = decisions
        .get("codec_families")
        .and_then(Value::as_array)
        .expect("backend decisions must contain codec_families")
        .iter()
        .find(|family| family.get("family_id").and_then(Value::as_str) == Some("jpeg_ls"))
        .expect("JPEG-LS backend decision must exist");
    assert_eq!(
        jpeg_ls
            .pointer("/near_lossless_policy/classification")
            .and_then(Value::as_str),
        Some("defer")
    );
    assert!(
        jpeg_ls
            .pointer("/near_lossless_policy/required_before_implementation")
            .and_then(Value::as_array)
            .is_some_and(|items| items.len() >= 4),
        "near-lossless policy should list the missing design and verification work"
    );
}

#[test]
fn jpeg_baseline_transfer_syntax_is_feature_gated_through_dicom_rs() {
    let matrix = read_json("transfer-syntax/capability-matrix.json");
    let entries = matrix
        .get("entries")
        .and_then(Value::as_array)
        .expect("transfer syntax matrix must contain entries");
    let entry = entries
        .iter()
        .find(|entry| entry.get("uid").and_then(Value::as_str) == Some("1.2.840.10008.1.2.4.50"))
        .expect("transfer syntax matrix must contain JPEG Baseline");
    let transfer_syntax = TransferSyntaxRegistry
        .get("1.2.840.10008.1.2.4.50")
        .expect("DICOM-rs registry must expose JPEG Baseline");

    assert_eq!(
        entry.get("keyword").and_then(Value::as_str),
        Some("JPEGBaseline8Bit")
    );
    assert_eq!(
        entry.get("status").and_then(Value::as_str),
        Some("feature_gated")
    );
    assert_eq!(
        entry.get("decode_pixel").and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        entry.get("encode_pixel").and_then(Value::as_bool),
        Some(true)
    );
    assert!(
        entry
            .get("feature_flags")
            .and_then(Value::as_array)
            .is_some_and(|features| features.iter().any(|value| value.as_str() == Some("jpeg"))),
        "JPEG Baseline should record the project jpeg feature gate"
    );
    if cfg!(feature = "jpeg") {
        assert!(
            transfer_syntax.pixel_data_reader().is_some(),
            "JPEG feature builds should expose a runtime decoder"
        );
        assert!(
            transfer_syntax.pixel_data_writer().is_some(),
            "JPEG feature builds should expose a runtime encoder"
        );
    }

    let registry = read_json("cases/registry.json");
    let cases = registry_cases(&registry);
    let case = cases
        .iter()
        .find(|case| {
            case.get("case_id").and_then(Value::as_str)
                == Some("classic/sc/rgb_planar0_jpeg_baseline_8bit")
        })
        .expect("registry must contain JPEG Baseline SC case");
    assert_eq!(
        case.get("status").and_then(Value::as_str),
        Some("implemented")
    );
    assert_eq!(case.get("skip"), Some(&Value::Null));
    assert_eq!(
        case.pointer("/requirements/features/0")
            .and_then(Value::as_str),
        Some("jpeg")
    );
    assert_eq!(
        case.get("determinism").and_then(Value::as_str),
        Some("semantic_stable")
    );
}

#[test]
fn compressed_transfer_syntax_registry_rows_remain_skipped_until_verified() {
    let matrix = read_json("transfer-syntax/capability-matrix.json");
    let matrix_entries = matrix
        .get("entries")
        .and_then(Value::as_array)
        .expect("transfer syntax matrix must contain entries");
    let registry = read_json("cases/registry.json");
    let cases = registry_cases(&registry);

    for (case_id, uid, keyword) in [
        (
            "classic/sc/mono2_u16_jpeg2000_lossless",
            "1.2.840.10008.1.2.4.90",
            "JPEG2000Lossless",
        ),
        (
            "classic/sc/rgb_planar0_jpegxl_lossless",
            "1.2.840.10008.1.2.4.110",
            "JPEGXLLossless",
        ),
        (
            "classic/sc/mono2_u16_htj2k_lossless",
            "1.2.840.10008.1.2.4.201",
            "HTJ2KLossless",
        ),
    ] {
        let matrix_entry = matrix_entries
            .iter()
            .find(|entry| entry.get("uid").and_then(Value::as_str) == Some(uid))
            .unwrap_or_else(|| panic!("transfer syntax matrix must contain {uid}"));
        assert_eq!(
            matrix_entry.get("keyword").and_then(Value::as_str),
            Some(keyword)
        );
        assert_eq!(
            matrix_entry.get("status").and_then(Value::as_str),
            Some("unavailable"),
            "{case_id} must not be implemented until matrix support is available"
        );

        let case = cases
            .iter()
            .find(|case| case.get("case_id").and_then(Value::as_str) == Some(case_id))
            .unwrap_or_else(|| panic!("registry must contain {case_id}"));
        assert_eq!(case.get("status").and_then(Value::as_str), Some("skipped"));
        assert_eq!(
            case.get("transfer_syntax_uid").and_then(Value::as_str),
            Some(uid)
        );
        assert_eq!(
            case.pointer("/skip/reason_code").and_then(Value::as_str),
            Some("codec_unavailable")
        );
        assert_eq!(
            case.pointer("/skip/recheck_phase").and_then(Value::as_str),
            Some("phase-6")
        );
        assert_eq!(
            case.get("determinism").and_then(Value::as_str),
            Some("semantic_stable")
        );
        assert!(
            case.get("standards_evidence")
                .and_then(Value::as_array)
                .is_some_and(|evidence| evidence.iter().any(|entry| {
                    entry.get("query").and_then(Value::as_str)
                        == Some("lookup_sop_class Secondary Capture Image Storage")
                }) && evidence.iter().any(|entry| {
                    entry
                        .get("query")
                        .and_then(Value::as_str)
                        .is_some_and(|query| query == format!("lookup_uid {keyword}"))
                })),
            "{case_id} must carry SC SOP Class and transfer syntax evidence"
        );
    }
}

#[test]
fn rle_lossless_transfer_syntax_is_available_through_native_backend() {
    let matrix = read_json("transfer-syntax/capability-matrix.json");
    let matrix_entries = matrix
        .get("entries")
        .and_then(Value::as_array)
        .expect("transfer syntax matrix must contain entries");
    let entry = matrix_entries
        .iter()
        .find(|entry| entry.get("uid").and_then(Value::as_str) == Some("1.2.840.10008.1.2.5"))
        .expect("transfer syntax matrix must contain RLE Lossless");
    assert_eq!(
        entry.get("keyword").and_then(Value::as_str),
        Some("RLELossless")
    );
    assert_eq!(
        entry.get("status").and_then(Value::as_str),
        Some("available")
    );
    assert_eq!(
        entry.get("encode_pixel").and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        entry.get("determinism").and_then(Value::as_str),
        Some("byte_stable")
    );
    assert!(
        entry
            .get("feature_flags")
            .and_then(Value::as_array)
            .is_some_and(Vec::is_empty),
        "native RLE backend should not require a Cargo codec feature"
    );

    let registry = read_json("cases/registry.json");
    let cases = registry_cases(&registry);
    let case = cases
        .iter()
        .find(|case| {
            case.get("case_id").and_then(Value::as_str) == Some("classic/sc/mono2_u8_rle_lossless")
        })
        .expect("registry must contain RLE Lossless SC case");
    assert_eq!(
        case.get("status").and_then(Value::as_str),
        Some("implemented")
    );
    assert_eq!(case.get("skip"), Some(&Value::Null));
    assert_eq!(
        case.get("determinism").and_then(Value::as_str),
        Some("byte_stable")
    );

    let u16_case = cases
        .iter()
        .find(|case| {
            case.get("case_id").and_then(Value::as_str) == Some("classic/sc/mono2_u16_rle_lossless")
        })
        .expect("registry must contain 16-bit RLE Lossless SC case");
    assert_eq!(
        u16_case.get("status").and_then(Value::as_str),
        Some("implemented")
    );
    assert_eq!(u16_case.get("skip"), Some(&Value::Null));
    assert_eq!(
        u16_case.get("determinism").and_then(Value::as_str),
        Some("byte_stable")
    );
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

fn registry_cases(registry: &Value) -> Vec<&Value> {
    registry
        .get("cases")
        .and_then(Value::as_array)
        .expect("registry cases should be an array")
        .iter()
        .collect()
}

fn generator_recipe_case_ids() -> BTreeSet<String> {
    let source = fs::read_to_string("src/generator.rs").expect("generator source must be readable");
    let mut case_ids = BTreeSet::new();
    let mut remaining = source.as_str();
    while let Some(start) = remaining.find("case_id: \"") {
        remaining = &remaining[start + "case_id: \"".len()..];
        let Some(end) = remaining.find('"') else {
            break;
        };
        let case_id = &remaining[..end];
        if is_suite_case_id(case_id) {
            case_ids.insert(case_id.to_string());
        }
        remaining = &remaining[end + 1..];
    }
    assert!(
        !case_ids.is_empty(),
        "generator source should declare recipe case IDs"
    );
    case_ids
}

fn is_suite_case_id(case_id: &str) -> bool {
    matches!(
        case_id.split('/').next(),
        Some("classic" | "enhanced" | "derived" | "non-image" | "vl")
    )
}

fn git_paths(args: &[&str]) -> Vec<String> {
    let output = Command::new("git")
        .args(args)
        .output()
        .expect("git command must run");
    assert!(
        output.status.success(),
        "git {} should succeed: {}",
        args.join(" "),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .expect("git output should be UTF-8")
        .lines()
        .map(str::to_string)
        .collect()
}

fn generated_payload_paths(paths: Vec<String>) -> Vec<String> {
    paths
        .into_iter()
        .filter(|path| is_generated_payload_path(path))
        .collect()
}

fn is_generated_payload_path(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    lower.ends_with(".dcm")
        || lower.ends_with(".dicom")
        || lower.ends_with(".ima")
        || lower.ends_with(".part10")
        || lower.ends_with("/manifest.json")
        || lower == "manifest.json"
        || lower.ends_with(".validation.json")
        || lower.ends_with(".expected.json")
        || lower.ends_with(".coverage.json")
}
