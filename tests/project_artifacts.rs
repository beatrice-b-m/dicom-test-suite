use std::collections::BTreeSet;
use std::fs;
use std::process::Command;

use dicom_transfer_syntax_registry::{TransferSyntaxIndex, TransferSyntaxRegistry};
use serde_json::Value;

#[test]
fn ci_verifies_default_and_feature_gated_codec_paths() {
    let workflow =
        fs::read_to_string(".github/workflows/ci.yml").expect("CI workflow must be readable");

    for required in [
        "cargo test --locked --all-targets --no-default-features",
        "generate --profile smoke",
        "generate --profile core",
        "generate --profile extended",
        "jpeg",
        "charls",
        "jpegxl",
        "jpeg2000",
        "deflate",
        "htj2k_openjph",
        "legacy_jpeg_dcmtk",
    ] {
        assert!(
            workflow.contains(required),
            "CI workflow must cover {required}"
        );
    }
}

#[test]
fn external_codec_policy_requires_runtime_evidence() {
    let policy = fs::read_to_string("docs/external-codec-verification.md")
        .expect("external codec verification policy must be readable");

    for required in [
        "`ojph_compress`",
        "`dcmcjpeg`",
        "before a release",
        "at least once per calendar quarter",
        "validation reports zero failures",
        "executable fingerprint",
        "`semantic_stable`",
    ] {
        assert!(
            policy.contains(required),
            "external codec policy must document {required}"
        );
    }
}

#[test]
fn coverage_baseline_records_the_phase_zero_comparison_point() {
    let baseline = fs::read_to_string("docs/coverage-baseline.md")
        .expect("coverage baseline must be readable");
    for required in [
        "cc0bef6690dbd7a338608e8e2293e8b1b48eeb114c022cb0827a7fb74ca2483d",
        "106 implemented logical cases",
        "21 distinct SOP Classes",
        "70 Secondary Capture Image Storage cases",
        "179 logical entries: 106 implemented and 73 planned",
        "report gaps --format json",
        "geometry/ct/spatial_sort_conflicts_instance_number",
        "derived/parametric-map/float32_ct_derived_explicit_le",
        "Do not use generated file count",
    ] {
        assert!(
            baseline.contains(required),
            "coverage baseline must record {required}"
        );
    }

    let registry = read_json("cases/registry.json");
    let cases = registry
        .get("cases")
        .and_then(Value::as_array)
        .expect("registry must contain cases");
    assert!(
        cases
            .iter()
            .filter(|case| case.get("status").and_then(Value::as_str) == Some("implemented"))
            .count()
            >= 106,
        "implemented coverage must not fall below the Phase 0 baseline"
    );
    for proof_case in [
        "geometry/ct/spatial_sort_conflicts_instance_number",
        "derived/parametric-map/float32_ct_derived_explicit_le",
    ] {
        assert!(
            cases
                .iter()
                .any(|case| case.get("case_id").and_then(Value::as_str) == Some(proof_case)),
            "registry must retain selected proof case {proof_case}"
        );
    }
}

#[test]
fn uv_iod_validator_is_case_scoped_and_fully_locked() {
    let validators = read_json("conformance/validators.json");
    let adapter = validators["adapters"]
        .as_array()
        .unwrap()
        .iter()
        .find(|adapter| adapter["id"] == "pydicom-dicom-validator-u32")
        .expect("u32 IOD validator adapter must be configured");
    assert_eq!(adapter["role"], "primary_iod_validator");
    assert_eq!(adapter["required"], false);
    assert_eq!(adapter["executable_env"], "DTS_DICOM_VALIDATOR_PYTHON");
    assert_eq!(
        adapter["supported_case_ids"],
        serde_json::json!([
            "classic/sc/mono2_u32_explicit_le",
            "classic/sc/nonsquare_pixel_spacing"
        ])
    );
    let artifacts = adapter["artifacts"].as_array().unwrap();
    assert_eq!(artifacts.len(), 14);
    assert_eq!(
        artifacts
            .iter()
            .filter(|artifact| artifact.get("root_env").is_some())
            .count(),
        8
    );
    for artifact in &artifacts[..6] {
        let path = artifact["path"].as_str().unwrap();
        assert!(
            fs::metadata(path).is_ok(),
            "committed validator input must exist: {path}"
        );
    }

    let lock = read_json("conformance/validator-lock.json");
    let tool = lock["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|tool| tool["adapter_id"] == "pydicom-dicom-validator-u32")
        .expect("u32 IOD validator must have an accepted lock entry");
    assert_eq!(
        tool["adapter_sha256"],
        "3f20de6ca7d310e2e9f2920f368912f97b6cf62bdfdf750e1417eb5dc4b335b6"
    );
    assert_eq!(tool["platforms"], serde_json::json!(["arm64-macos"]));
    assert!(
        tool["package_identity"]
            .as_str()
            .unwrap()
            .contains("uv.lock sha256")
    );
    assert!(
        tool["definition_version"]
            .as_str()
            .unwrap()
            .contains("DICOM 2026b")
    );
}

#[test]
fn uv_conformance_docs_preserve_the_independent_gate() {
    let readme = fs::read_to_string("conformance/README.md").unwrap();
    for required in [
        "exact-case-first",
        "DTS_DICOM_VALIDATOR_PYTHON",
        "DTS_DICOM_VALIDATOR_STANDARD_HOME",
        "terminal",
        "untouched original",
    ] {
        assert!(
            readme.contains(required),
            "conformance docs require {required}"
        );
    }
    let backend = fs::read_to_string("conformance-backends/dicom-validator/README.md").unwrap();
    for required in [
        "`uv`",
        "Adapter version 0.3.0",
        "`--pixel-u32`",
        "`--nonsquare-spacing`",
        "does not use NumPy",
    ] {
        assert!(
            backend.contains(required),
            "backend docs require {required}"
        );
    }
    let source = fs::read_to_string("standards/source-notes/phase-2-u32-native-pixels.md").unwrap();
    for required in [
        "Independent Validator Qualification",
        "TagMissing",
        "EnumValueNotAllowed",
        "zero strict-verification failures",
        "b078217dad7f87238cfa3042ace25ec4fcc974dc2ced472b9974623b0caa19a4",
    ] {
        assert!(
            source.contains(required),
            "u32 source note requires {required}"
        );
    }
    let phase = fs::read_to_string("docs/phase-2-native-status.md").unwrap();
    for required in [
        "Unsigned 32-bit native Secondary Capture",
        "85 files",
        "silent entity validation",
        "One-bit native Multi-frame Secondary Capture",
        "All dependency-ordered Phase 2 milestones are closed",
    ] {
        assert!(phase.contains(required), "phase status requires {required}");
    }
    let plan = fs::read_to_string("docs/coverage-expansion-plan.md").unwrap();
    assert!(plan.contains("unsigned 32-bit native Secondary Capture"));
    assert!(plan.contains("1-bit native Multi-frame Secondary Capture slice is also"));
    assert!(plan.contains("ICC profile handling is complete"));
}

#[test]
fn u1_source_note_locks_cross_frame_bit_packing() {
    let source = fs::read_to_string("standards/source-notes/phase-2-u1-native-pixels.md")
        .expect("u1 source note must be readable");
    for required in [
        "classic/sc/mono2_u1_native",
        "Multi-frame Single Bit Secondary Capture",
        "18 samples",
        "`55 55 01 00`",
        "least significant bit",
        "without per-frame padding",
        "1.2.840.10008.5.1.4.1.1.7.1",
        "1cc11d28abf1e6f4efa4b07a73d4a7c953b3b3101b4112865c7170ccdeb84728",
        "Registry status: implemented",
        "Pydicom `dicom-validator` 0.8.2 was evaluated but rejected",
        "6a6103a7c516814b5eb44f53d198b111cbaf1678de5952ab7d31961732f112d5",
    ] {
        assert!(
            source.contains(required),
            "u1 source note requires {required}"
        );
    }

    let phase = fs::read_to_string("docs/phase-2-native-status.md").unwrap();
    for required in [
        "86 files",
        "continuous packing",
        "normalized errors despite a zero tool exit code",
    ] {
        assert!(phase.contains(required), "phase status requires {required}");
    }
    let readme = fs::read_to_string("conformance/README.md").unwrap();
    for required in [
        "U1 SC independent pixels",
        "did not reject an invalid `8/8/7`",
        "Every other native shape remains explicitly",
    ] {
        assert!(
            readme.contains(required),
            "conformance docs require {required}"
        );
    }
}

#[test]
fn icc_source_note_locks_dicom_input_profile_contract() {
    let source = fs::read_to_string("standards/source-notes/phase-2-icc-profile.md")
        .expect("ICC source note must be readable");
    for required in [
        "vl/photo/rgb_icc_profile_explicit_le",
        "DCMTK_SRGB_ICC_SAMPLE",
        "CC0",
        "`736`",
        "8e069a3476b71a0e0ae7272d9278ba70540d1c4a0b19af1c7d52e56f49091fef",
        "`scnr`",
        "`RGB `",
        "`XYZ `",
        "`acsp`",
        "`SRGB`",
        "Table C.11.15-1",
        "Section C.11.15.1.1",
        "1cc11d28abf1e6f4efa4b07a73d4a7c953b3b3101b4112865c7170ccdeb84728",
        "Independent Validator Qualification",
        "498f65088efa9f32a013a26232336348a3c195eb9cb8f487411f2fe51e085328",
        "strict conformance verification reported zero failures",
        "Registry status: implemented",
        "Should become KB patch: yes",
    ] {
        assert!(
            source.contains(required),
            "ICC source note requires {required}"
        );
    }

    let validators = read_json("conformance/validators.json");
    let adapter = validators["adapters"]
        .as_array()
        .unwrap()
        .iter()
        .find(|adapter| adapter["id"] == "littlecms-transicc-icc")
        .expect("LittleCMS ICC validator must be configured");
    assert_eq!(adapter["role"], "icc_validator");
    assert_eq!(
        adapter["supported_case_ids"],
        serde_json::json!(["vl/photo/rgb_icc_profile_explicit_le"])
    );
    assert_eq!(adapter["artifacts"][0]["root_env"], "DTS_LCMS_HOME");

    let lock = read_json("conformance/validator-lock.json");
    let tool = lock["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|tool| tool["adapter_id"] == "littlecms-transicc-icc")
        .expect("LittleCMS ICC validator must be locked");
    assert_eq!(
        tool["adapter_sha256"],
        "498f65088efa9f32a013a26232336348a3c195eb9cb8f487411f2fe51e085328"
    );
    assert_eq!(
        tool["supporting_artifacts"]["lib/liblcms2.2.dylib"],
        "c74076bc75654249cd88fee91aa4413c9cf00d3708710cf652bef04eec1a9ad1"
    );
    let readme = fs::read_to_string("conformance/README.md").unwrap();
    for required in [
        "ICC profile processing",
        "transicc -n -i<profile> -o*XYZ -t0",
        "no ICC failure can be",
    ] {
        assert!(
            readme.contains(required),
            "ICC conformance docs require {required}"
        );
    }
    let phase = fs::read_to_string("docs/phase-2-native-status.md").unwrap();
    for required in [
        "ICC input profile handling",
        "87 files",
        "23680ffd511565f585430e9cd3e6ac397b7c36c60027f190bee86a03afdd7ef0",
        "zero strict conformance failures",
    ] {
        assert!(
            phase.contains(required),
            "ICC phase status requires {required}"
        );
    }
}

#[test]
fn nonsquare_source_note_locks_distinct_spacing_and_aspect_axes() {
    let source =
        fs::read_to_string("standards/source-notes/phase-2-nonsquare-spacing-aspect-ratio.md")
            .expect("non-square source note must be readable");
    for required in [
        "classic/sc/nonsquare_pixel_spacing",
        "pixel-spacing.dcm",
        "pixel-aspect-ratio.dcm",
        "`0.6\\\\0.3`",
        "`2\\\\1`",
        "C.7.6.3.1.7",
        "Table C.7-11c",
        "Section 10.7.1.1",
        "e89b23efeade0dc3de624fc8982ea8b99adb35a3bb9a2fbf8b8ce675e10581a6",
        "1cc11d28abf1e6f4efa4b07a73d4a7c953b3b3101b4112865c7170ccdeb84728",
        "Registry status: implemented",
        "adapter fingerprint",
        "Should become KB patch: yes",
    ] {
        assert!(
            source.contains(required),
            "non-square source note requires {required}"
        );
    }
}

#[test]
fn float64_parametric_map_note_locks_od_and_binary64_contract() {
    let source = fs::read_to_string("standards/source-notes/parametric-map-float64.md")
        .expect("float64 Parametric Map source note must be readable");
    for required in [
        "derived/parametric-map/float64_ct_derived_explicit_le",
        "Double Float Pixel Data",
        "`(7FE0,0009)`",
        "VR `OD`",
        "Bits Allocated `(0028,0100)`",
        "`64`",
        "`(7FE0,0008)`",
        "`(7FE0,0010)`",
        "`(0040,9214)`",
        "`(0040,9213)`",
        "`2^-30`",
        "13866583252673691648",
        "921a8e74cc86e767d5436be2a4eb0c6d383bf3f210ec4c32e8f8c43c239f8abe",
        "be480ba76c1931f10052029005c539dd45b565f7020cc94a41a89825c3b6ea44",
        "ce1600d46bb7468f4a0f60c2d58cf96430234a89e50f0cacdd56bfd86bc3ec90",
        "21a27d41285f045a72c0de209c4b48ea98a09257d44520290bc6044b132fc002",
        "1cc11d28abf1e6f4efa4b07a73d4a7c953b3b3101b4112865c7170ccdeb84728",
        "highdicom `0.28.1`",
        "Independent Validator Qualification Plan",
        "dicom-validator` 0.8.2",
        "DCMTK `dcmdump`",
        "Registry status: remain planned",
        "Should become KB patch: yes",
    ] {
        assert!(
            source.contains(required),
            "float64 Parametric Map source note requires {required}"
        );
    }
}

#[test]
fn integer_parametric_map_retains_explicit_provider_blocker() {
    let registry = read_json("cases/registry.json");
    let case = registry_cases(&registry)
        .into_iter()
        .find(|case| {
            case.get("case_id").and_then(Value::as_str)
                == Some("derived/parametric-map/integer_ct_derived_explicit_le")
        })
        .expect("integer Parametric Map row must exist");
    assert_eq!(case["status"], "planned");
    assert_eq!(case["provider"]["id"], "dcmqi");
    assert_eq!(
        case["blockers"],
        serde_json::json!([{
            "code": "provider_capability_unavailable",
            "message": "Locked dcmqi v1.5.7 emits only the floating-point Parametric Map pixel module; the cross-implementation integer Image Pixel path is unavailable.",
            "recheck_phase": "phase-3"
        }])
    );

    let note =
        fs::read_to_string("standards/source-notes/phase-3-integer-parametric-map-provider.md")
            .expect("integer Parametric Map provider note must exist");
    for required in [
        "provider_capability_unavailable",
        "dcmqi v1.5.7",
        "506306a",
        "ec17425d3eaa7b58db0924138569508c833e9774ef48052ca85d3e5a1b6cf9b9",
        "IODFloatingPointImagePixelModule",
        "Do not silently substitute",
    ] {
        assert!(note.contains(required), "provider note requires {required}");
    }
}

#[test]
fn u1_pixel_decoder_is_case_scoped_and_locked() {
    let validators = read_json("conformance/validators.json");
    let adapter = validators["adapters"]
        .as_array()
        .unwrap()
        .iter()
        .find(|adapter| adapter["id"] == "dcmtk-dcm2img-u1")
        .expect("u1 pixel decoder must be configured");
    assert_eq!(adapter["role"], "pixel_decoder");
    assert_eq!(adapter["required"], false);
    assert_eq!(
        adapter["supported_case_ids"],
        serde_json::json!(["classic/sc/mono2_u1_native"])
    );
    assert_eq!(
        adapter["arguments"],
        serde_json::json!([
            "+Fa", "+Fn", "-M", "-W", "+Pid", "-O", "+opn", "1", "{input}", "{output}"
        ])
    );

    let lock = read_json("conformance/validator-lock.json");
    let tool = lock["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|tool| tool["adapter_id"] == "dcmtk-dcm2img-u1")
        .expect("u1 pixel decoder must have a lock entry");
    assert_eq!(
        tool["executable_sha256"],
        "6a6103a7c516814b5eb44f53d198b111cbaf1678de5952ab7d31961732f112d5"
    );
    assert_eq!(tool["platforms"], serde_json::json!(["arm64-macos"]));

    let decoders = fs::read_to_string("conformance/pixel-decoders.json").unwrap();
    assert!(decoders.contains("one-bit native frames packed continuously"));
    assert!(decoders.contains("every other native pixel shape remains"));
}

#[test]
fn readme_documents_supported_commands_and_codec_features() {
    let readme = fs::read_to_string("README.md").expect("README must be readable");

    for required in [
        "cargo run --locked -- generate",
        "cargo run --locked -- validate",
        "cargo run --locked -- report",
        "`smoke`",
        "`core`",
        "`extended`",
        "`jpeg`",
        "`charls`",
        "`jpegxl`",
        "`jpeg2000`",
        "`deflate`",
        "`htj2k_openjph`",
        "`legacy_jpeg_dcmtk`",
        "docs/external-codec-verification.md",
    ] {
        assert!(readme.contains(required), "README must document {required}");
    }
}

#[test]
fn corpus_consumption_guide_documents_complete_handoff() {
    let guide = fs::read_to_string("docs/corpus-consumption.md")
        .expect("corpus consumption guide must be readable");

    for required in [
        "`all` is the union of `smoke`, `core`, and `extended`",
        "Generate `legacy` separately",
        "--all-features",
        "ojph_compress",
        "dcmcjpeg",
        "validation_failures\\t0",
        "manifest.json",
        "skipped_cases",
        "stable `case_id`",
        "does not require",
        "Scope Boundary",
    ] {
        assert!(
            guide.contains(required),
            "corpus consumption guide must document {required}"
        );
    }
}

#[test]
fn conformance_agent_brief_is_tractable_and_complete() {
    let brief = fs::read_to_string("docs/conformance-validation-agent-brief.md")
        .expect("conformance validation agent brief must be readable");

    for required in [
        "dciodvfy -new",
        "dcentvfy",
        "conformance check-tools",
        "conformance run",
        "conformance verify",
        "validator-lock.json",
        "accepted-findings.json",
        "conformance-run.schema.json",
        "manifest-relative paths",
        "Independent Pixel Evidence",
        "Implementation Phases And Commits",
        "Complete Acceptance Criteria",
        "Stop And Escalate Conditions",
        "viewer-specific",
    ] {
        assert!(
            brief.contains(required),
            "conformance validation agent brief must document {required}"
        );
    }
}

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
        ("classic/sc/nonsquare_pixel_spacing", "implemented"),
        (
            "classic/ct/mono2_i16_rescale_12bit_explicit_le",
            "implemented",
        ),
        (
            "classic/ct/mono2_i16_rescale_12bit_rle_lossless",
            "implemented",
        ),
        (
            "classic/mg/for_presentation_mono1_u16_12bit_explicit_le",
            "implemented",
        ),
        (
            "classic/mg/for_presentation_mono1_u16_12bit_rle_lossless",
            "implemented",
        ),
        (
            "classic/mg/for_processing_mono2_u16_12bit_implicit_le",
            "implemented",
        ),
        (
            "classic/mg/for_processing_mono2_u16_12bit_rle_lossless",
            "implemented",
        ),
        ("classic/nm/multiframe_explicit_le", "implemented"),
        ("classic/pet/rescaled_activity_explicit_le", "implemented"),
        ("classic/xa/monoplane_explicit_le", "implemented"),
        ("classic/cr/overlay_modality_voi_explicit_le", "implemented"),
        (
            "classic/cr/overlay_modality_voi_rle_lossless",
            "implemented",
        ),
        ("classic/mr/multislice_oblique_explicit_le", "implemented"),
        (
            "classic/dx/display_shutter_mono2_u16_explicit_le",
            "implemented",
        ),
        (
            "classic/dx/display_shutter_mono2_u16_rle_lossless",
            "implemented",
        ),
        ("classic/us/mono2_u8_explicit_le", "implemented"),
        ("classic/us/mono2_u8_rle_lossless", "implemented"),
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
        ("classic/sc/mono1_u8_rle_lossless", "implemented"),
        ("classic/sc/mono1_u16_rle_lossless", "implemented"),
        ("classic/sc/mono2_u16_rle_lossless", "implemented"),
        ("classic/sc/mono2_u16_odd_3x3_rle_lossless", "implemented"),
        ("classic/sc/mono2_i16_odd_3x3_rle_lossless", "implemented"),
        ("classic/sc/mono1_i16_odd_3x3_rle_lossless", "implemented"),
        ("classic/sc/mono2_u16_rect_2x3_rle_lossless", "implemented"),
        ("classic/sc/mono1_u16_rect_2x3_rle_lossless", "implemented"),
        ("classic/sc/mono2_i16_rect_2x3_rle_lossless", "implemented"),
        (
            "classic/sc/mono2_u16_multiframe_rle_lossless",
            "implemented",
        ),
        (
            "classic/sc/mono1_u16_multiframe_rle_lossless",
            "implemented",
        ),
        ("classic/sc/mono2_u16_tiny_1x1_rle_lossless", "implemented"),
        ("classic/sc/mono1_u16_tiny_1x1_rle_lossless", "implemented"),
        ("classic/sc/mono2_i16_tiny_1x1_rle_lossless", "implemented"),
        ("classic/sc/mono1_i16_tiny_1x1_rle_lossless", "implemented"),
        ("classic/sc/mono2_u16_padding_rle_lossless", "implemented"),
        ("classic/sc/mono2_u8_padding_rle_lossless", "implemented"),
        ("classic/sc/mono1_u8_padding_rle_lossless", "implemented"),
        (
            "classic/sc/mono2_u8_padding_multiframe_rle_lossless",
            "implemented",
        ),
        (
            "classic/sc/mono1_u8_padding_multiframe_rle_lossless",
            "implemented",
        ),
        (
            "classic/sc/mono2_u16_padding_multiframe_rle_lossless",
            "implemented",
        ),
        (
            "classic/sc/mono1_u16_padding_multiframe_rle_lossless",
            "implemented",
        ),
        ("classic/sc/mono2_i16_padding_rle_lossless", "implemented"),
        ("classic/sc/mono1_i16_padding_rle_lossless", "implemented"),
        (
            "classic/sc/mono1_i16_padding_multiframe_rle_lossless",
            "implemented",
        ),
        (
            "classic/sc/mono2_i16_padding_multiframe_rle_lossless",
            "implemented",
        ),
        ("classic/sc/mono2_i16_rle_lossless", "implemented"),
        ("classic/sc/mono1_i16_rle_lossless", "implemented"),
        (
            "classic/sc/mono2_i16_multiframe_rle_lossless",
            "implemented",
        ),
        (
            "classic/sc/mono1_i16_multiframe_rle_lossless",
            "implemented",
        ),
        ("classic/sc/rgb_planar0_rle_lossless", "implemented"),
        (
            "classic/sc/rgb_planar0_multiframe_rle_lossless",
            "implemented",
        ),
        ("classic/sc/rgb_planar1_rle_lossless", "implemented"),
        (
            "classic/sc/rgb_planar1_multiframe_rle_lossless",
            "implemented",
        ),
        ("classic/sc/ybr_full_planar0_rle_lossless", "implemented"),
        (
            "classic/sc/ybr_full_planar0_multiframe_rle_lossless",
            "implemented",
        ),
        ("classic/sc/ybr_full_planar1_rle_lossless", "implemented"),
        (
            "classic/sc/ybr_full_planar1_multiframe_rle_lossless",
            "implemented",
        ),
        ("classic/sc/palette_color_u8_rle_lossless", "implemented"),
        (
            "classic/sc/palette_color_u8_multiframe_rle_lossless",
            "implemented",
        ),
        ("classic/sc/mono2_u8_multiframe_rle_lossless", "implemented"),
        (
            "classic/sc/mono2_u8_odd_fragment_rle_lossless",
            "implemented",
        ),
        (
            "classic/sc/mono1_u8_odd_fragment_rle_lossless",
            "implemented",
        ),
        ("vl/photo/rgb_planar0_rle_lossless", "implemented"),
        ("vl/photo/rgb_planar1_rle_lossless", "implemented"),
        ("vl/photo/palette_color_rle_lossless", "implemented"),
        ("vl/photo/rgb_planar0_explicit_le", "implemented"),
        ("vl/photo/palette_color_explicit_le", "implemented"),
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
fn feature_gated_registry_cases_carry_report_modality_metadata() {
    let registry = read_json("cases/registry.json");
    let cases = registry_cases(&registry);

    for (case_id, expected_modality) in [
        ("classic/sc/rgb_planar0_jpeg_baseline_8bit", "OT"),
        ("classic/sc/mono2_u8_jpeg_ls_lossless", "OT"),
        ("classic/sc/mono2_u16_jpeg2000_lossless", "OT"),
        ("classic/sc/rgb_planar0_jpegxl_lossless", "OT"),
        ("classic/sc/mono2_u16_htj2k_lossless", "OT"),
        ("classic/sc/mono2_u16_jpeg_lossless_process_14", "OT"),
        ("classic/sc/mono2_u16_jpeg_lossless_sv1", "OT"),
        ("derived/seg/binary_multiframe_deflated_image_frame", "SEG"),
    ] {
        let case = cases
            .iter()
            .find(|case| case.get("case_id").and_then(Value::as_str) == Some(case_id))
            .unwrap_or_else(|| panic!("registry must contain {case_id}"));

        assert!(
            case.pointer("/requirements/features")
                .and_then(Value::as_array)
                .is_some_and(|features| !features.is_empty()),
            "{case_id} should be feature-gated in default builds"
        );
        assert_eq!(
            case.get("modality").and_then(Value::as_str),
            Some(expected_modality),
            "{case_id} should expose registry modality metadata for unavailable report rows"
        );
    }
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
        vec![
            "rle_lossless",
            "jpeg_baseline_8bit",
            "jpeg_ls",
            "jpeg_xl",
            "jpeg_2000",
            "htj2k",
            "legacy_jpeg",
            "deflated_image_frame"
        ],
        "enabled compressed codec families should be explicit"
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

    let jpeg_xl = families
        .iter()
        .find(|family| family.get("family_id").and_then(Value::as_str) == Some("jpeg_xl"))
        .expect("JPEG XL decision must exist");
    assert_eq!(
        jpeg_xl.get("selected_backend").and_then(Value::as_str),
        Some("dicom_rs_jpegxl_lossless_writer")
    );
    assert_eq!(
        jpeg_xl.get("backend_kind").and_then(Value::as_str),
        Some("dicom_rs_feature")
    );
    assert_eq!(
        jpeg_xl.get("feature_gate").and_then(Value::as_str),
        Some("jpegxl")
    );
    assert_eq!(
        jpeg_xl.get("determinism").and_then(Value::as_str),
        Some("semantic_stable")
    );
    assert_eq!(
        jpeg_xl
            .get("transfer_syntax_uids")
            .and_then(Value::as_array)
            .expect("JPEG XL transfer syntax UID list should exist")
            .iter()
            .filter_map(Value::as_str)
            .collect::<Vec<_>>(),
        vec!["1.2.840.10008.1.2.4.110"],
        "implemented JPEG XL decision should be scoped to the lossless transfer syntax"
    );
    assert_eq!(
        jpeg_xl
            .pointer("/lossy_policy/classification")
            .and_then(Value::as_str),
        Some("defer"),
        "JPEG XL lossy policy should remain deferred after the lossless case is implemented"
    );

    let jpeg_2000 = families
        .iter()
        .find(|family| family.get("family_id").and_then(Value::as_str) == Some("jpeg_2000"))
        .expect("JPEG 2000 decision must exist");
    assert_eq!(
        jpeg_2000.get("selected_backend").and_then(Value::as_str),
        Some("project_jpeg2k_openjp2_lossless_adapter")
    );
    assert_eq!(
        jpeg_2000.get("backend_kind").and_then(Value::as_str),
        Some("rust_adapter")
    );
    assert_eq!(
        jpeg_2000.get("feature_gate").and_then(Value::as_str),
        Some("jpeg2000")
    );
    assert_eq!(
        jpeg_2000.get("determinism").and_then(Value::as_str),
        Some("semantic_stable")
    );
    assert_eq!(
        jpeg_2000
            .get("transfer_syntax_uids")
            .and_then(Value::as_array)
            .expect("JPEG 2000 transfer syntax UID list should exist")
            .iter()
            .filter_map(Value::as_str)
            .collect::<Vec<_>>(),
        vec!["1.2.840.10008.1.2.4.90"],
        "implemented JPEG 2000 decision should be scoped to lossless first"
    );
    assert_eq!(
        jpeg_2000
            .pointer("/lossy_policy/classification")
            .and_then(Value::as_str),
        Some("defer"),
        "JPEG 2000 lossy policy should stay deferred until lossy semantics are defined"
    );
}

#[test]
fn htj2k_lossless_backend_decision_selects_openjph_external_command() {
    let cargo_toml = fs::read_to_string("Cargo.toml").expect("Cargo.toml must be readable");
    assert!(
        cargo_toml.contains("htj2k_openjph = [") && cargo_toml.contains("\"jpeg2000\""),
        "Cargo.toml should expose a project htj2k_openjph feature through the JPEG 2000 reader stack"
    );

    let decisions = read_json("transfer-syntax/backend-decisions.json");
    let families = decisions
        .get("codec_families")
        .and_then(Value::as_array)
        .expect("backend decisions must contain codec_families");
    let htj2k = families
        .iter()
        .find(|family| family.get("family_id").and_then(Value::as_str) == Some("htj2k"))
        .expect("HTJ2K decision must exist");

    assert_eq!(
        htj2k.get("classification").and_then(Value::as_str),
        Some("implement_now"),
        "HTJ2K should be selected for implementation once the external command wrapper and fingerprint strategy are proven"
    );
    assert_eq!(
        htj2k.get("selected_backend").and_then(Value::as_str),
        Some("openjph_htj2k_lossless_command_writer")
    );
    assert_eq!(
        htj2k.get("backend_kind").and_then(Value::as_str),
        Some("external_command")
    );
    assert_eq!(
        htj2k.get("feature_gate").and_then(Value::as_str),
        Some("htj2k_openjph"),
        "HTJ2K should claim only the explicit project wrapper feature gate before corpus promotion"
    );
    assert_eq!(
        htj2k
            .pointer("/integration_mode/status")
            .and_then(Value::as_str),
        Some("selected"),
        "HTJ2K should record the selected integration mode"
    );
    assert_eq!(
        htj2k
            .pointer("/integration_mode/selected")
            .and_then(Value::as_str),
        Some("external_command")
    );
    assert_eq!(
        htj2k
            .pointer("/integration_mode/command")
            .and_then(Value::as_str),
        Some("ojph_compress")
    );
    assert_eq!(
        htj2k.pointer("/integration_mode/version_command"),
        Some(&Value::Null),
        "HTJ2K should not record a version command that the local OpenJPH binary rejects"
    );
    assert!(
        htj2k
            .pointer("/integration_mode/version_discovery")
            .and_then(Value::as_str)
            .is_some_and(|finding| finding.contains("SHA-256 executable fingerprint")),
        "HTJ2K should record the executable-fingerprint fallback for OpenJPH identity"
    );
    assert!(
        htj2k
            .pointer("/integration_mode/fingerprint_strategy")
            .and_then(Value::as_str)
            .is_some_and(|strategy| strategy.contains("SHA-256")),
        "HTJ2K should record the executable fingerprint strategy"
    );
    assert_eq!(
        htj2k
            .get("transfer_syntax_uids")
            .and_then(Value::as_array)
            .expect("HTJ2K transfer syntax UID list should exist")
            .iter()
            .filter_map(Value::as_str)
            .collect::<Vec<_>>(),
        vec!["1.2.840.10008.1.2.4.201"],
        "the first HTJ2K decision should be scoped to Lossless Only"
    );

    let candidates = htj2k
        .get("candidate_backends")
        .and_then(Value::as_array)
        .expect("HTJ2K decision should compare candidate backends");
    let openjph = candidates
        .iter()
        .find(|candidate| candidate.get("name").and_then(Value::as_str) == Some("OpenJPH"))
        .expect("OpenJPH candidate should be recorded");
    assert_eq!(
        openjph.get("status").and_then(Value::as_str),
        Some("selected_external_command_wrapper")
    );
    assert_eq!(
        openjph.get("license").and_then(Value::as_str),
        Some("BSD-2-Clause")
    );
    assert_eq!(
        openjph.get("backend_kind").and_then(Value::as_str),
        Some("external_command")
    );

    let grok = candidates
        .iter()
        .find(|candidate| candidate.get("name").and_then(Value::as_str) == Some("Grok"))
        .expect("Grok candidate should be recorded");
    assert_eq!(
        grok.get("status").and_then(Value::as_str),
        Some("defer_optional_experiment"),
        "Grok should remain outside default generation under the licensing policy"
    );
    assert_eq!(
        grok.get("license").and_then(Value::as_str),
        Some("AGPL-3.0")
    );

    let blockers = htj2k
        .get("blockers")
        .and_then(Value::as_array)
        .expect("HTJ2K blockers should be recorded")
        .iter()
        .filter_map(Value::as_str)
        .collect::<Vec<_>>();
    assert!(
        !blockers
            .iter()
            .any(|blocker| blocker.contains("generated HTJ2K Lossless")),
        "HTJ2K should not keep generated corpus integration blocked after generation"
    );
    assert!(
        blockers
            .iter()
            .any(|blocker| blocker.contains("executable SHA-256")),
        "HTJ2K should record executable fingerprinting as the current OpenJPH identity fallback"
    );
    assert!(
        !blockers
            .iter()
            .any(|blocker| blocker.contains("Full unsigned 16-bit sample-domain behavior")),
        "HTJ2K should not keep the raw sample-domain blocker after selecting the PGM path"
    );
    assert!(
        !blockers
            .iter()
            .any(|blocker| blocker.contains("integration mode is not selected")),
        "HTJ2K integration mode should no longer be unresolved"
    );

    let evidence = htj2k
        .get("evidence")
        .and_then(Value::as_array)
        .expect("HTJ2K evidence should be recorded");
    assert!(
        evidence.iter().any(|item| {
            item.get("source").and_then(Value::as_str) == Some("local-spike")
                && item
                    .get("finding")
                    .and_then(Value::as_str)
                    .is_some_and(|finding| {
                        finding.contains("zero, midrange, and high unsigned values")
                    })
        }),
        "HTJ2K decision should record the OpenJPH encode/decode proof"
    );
    assert!(
        evidence.iter().any(|item| {
            item.get("source").and_then(Value::as_str) == Some("local-verification")
                && item
                    .get("finding")
                    .and_then(Value::as_str)
                    .is_some_and(|finding| finding.contains("byte-identical HTJ2K codestream"))
        }),
        "HTJ2K decision should record the OpenJPH command reproducibility proof"
    );
    assert!(
        evidence.iter().any(|item| {
            item.get("source").and_then(Value::as_str) == Some("local-verification")
                && item
                    .get("finding")
                    .and_then(Value::as_str)
                    .is_some_and(|finding| finding.contains("PGM path with `-num_decomps 1`"))
        }),
        "HTJ2K decision should record the selected PGM path for unsigned sample interpretation"
    );
    assert!(
        evidence.iter().any(|item| {
            item.get("source").and_then(Value::as_str) == Some("local-decision")
                && item
                    .get("finding")
                    .and_then(Value::as_str)
                    .is_some_and(|finding| finding.contains("external-command wrapper"))
        }),
        "HTJ2K decision should record why the external-command mode was selected"
    );
    assert!(
        evidence.iter().any(|item| {
            item.get("source").and_then(Value::as_str) == Some("local-verification")
                && item.get("path").and_then(Value::as_str) == Some("src/codecs.rs")
                && item
                    .get("finding")
                    .and_then(Value::as_str)
                    .is_some_and(|finding| {
                        finding.contains("htj2k_openjph")
                            && finding.contains("fingerprints the executable")
                    })
        }),
        "HTJ2K decision should record the project wrapper verification"
    );
    assert!(
        evidence.iter().any(|item| {
            item.get("source").and_then(Value::as_str) == Some("local-verification")
                && item.get("path").and_then(Value::as_str) == Some("src/generator.rs")
                && item
                    .get("finding")
                    .and_then(Value::as_str)
                    .is_some_and(|finding| {
                        finding.contains("executable SHA-256 runtime identity")
                            && finding.contains("exact decoded native frame hashes")
                    })
        }),
        "HTJ2K decision should record generated-case validation evidence"
    );

    assert!(
        htj2k
            .pointer("/integration_mode/input_path")
            .and_then(Value::as_str)
            .is_some_and(|path| path.contains("16-bit MONOCHROME2 PGM input")),
        "HTJ2K should select PGM input for the first OpenJPH path"
    );
}

#[test]
fn legacy_jpeg_lossless_registry_rows_are_feature_gated_implemented() {
    let matrix = read_json("transfer-syntax/capability-matrix.json");
    let matrix_entries = matrix
        .get("entries")
        .and_then(Value::as_array)
        .expect("transfer syntax matrix must contain entries");
    let registry = read_json("cases/registry.json");
    let cases = registry_cases(&registry);

    for (case_id, uid, keyword, uid_query) in [
        (
            "classic/sc/mono2_u16_jpeg_lossless_process_14",
            "1.2.840.10008.1.2.4.57",
            "JPEGLossless",
            "lookup_uid JPEGLossless",
        ),
        (
            "classic/sc/mono2_u16_jpeg_lossless_sv1",
            "1.2.840.10008.1.2.4.70",
            "JPEGLosslessSV1",
            "lookup_uid JPEGLosslessSV1",
        ),
    ] {
        let matrix_entry = matrix_entries
            .iter()
            .find(|entry| entry.get("uid").and_then(Value::as_str) == Some(uid))
            .expect("transfer syntax matrix must contain legacy JPEG lossless UID");
        assert_eq!(
            matrix_entry.get("keyword").and_then(Value::as_str),
            Some(keyword)
        );
        assert_eq!(
            matrix_entry.get("status").and_then(Value::as_str),
            Some("feature_gated"),
            "{case_id} should be feature-gated after generated-case validation"
        );
        assert_eq!(
            matrix_entry
                .pointer("/feature_flags/0")
                .and_then(Value::as_str),
            Some("legacy_jpeg_dcmtk")
        );
        assert_eq!(
            matrix_entry.get("encode_pixel").and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(
            matrix_entry.get("decode_pixel").and_then(Value::as_bool),
            Some(true)
        );

        let case = cases
            .iter()
            .find(|case| case.get("case_id").and_then(Value::as_str) == Some(case_id))
            .expect("registry must contain legacy JPEG lossless case");
        assert_eq!(
            case.get("status").and_then(Value::as_str),
            Some("implemented")
        );
        assert_eq!(case.get("skip"), Some(&Value::Null));
        assert_eq!(
            case.get("transfer_syntax_uid").and_then(Value::as_str),
            Some(uid)
        );
        assert_eq!(
            case.get("determinism").and_then(Value::as_str),
            Some("semantic_stable")
        );
        assert_eq!(
            case.pointer("/requirements/features/0")
                .and_then(Value::as_str),
            Some("legacy_jpeg_dcmtk"),
            "{case_id} should name the project DCMTK wrapper feature gate"
        );
        assert!(
            case.get("standards_evidence")
                .and_then(Value::as_array)
                .is_some_and(|evidence| evidence.iter().any(|entry| {
                    entry.get("query").and_then(Value::as_str)
                        == Some("lookup_sop_class Secondary Capture Image Storage")
                }) && evidence.iter().any(|entry| {
                    entry.get("query").and_then(Value::as_str) == Some(uid_query)
                })),
            "{case_id} must carry SC SOP Class and transfer syntax evidence"
        );
    }
}

#[test]
fn jpeg_extended_12bit_remains_deferred_until_independent_validation_exists() {
    let matrix = read_json("transfer-syntax/capability-matrix.json");
    let matrix_entries = matrix
        .get("entries")
        .and_then(Value::as_array)
        .expect("transfer syntax matrix must contain entries");
    let entry = matrix_entries
        .iter()
        .find(|entry| entry.get("uid").and_then(Value::as_str) == Some("1.2.840.10008.1.2.4.51"))
        .expect("transfer syntax matrix must explicitly track JPEG Extended 12-bit");

    assert_eq!(
        entry.get("keyword").and_then(Value::as_str),
        Some("JPEGExtended12Bit")
    );
    assert_eq!(
        entry.get("status").and_then(Value::as_str),
        Some("unavailable"),
        "JPEG Extended 12-bit must stay unavailable until independent validation exists"
    );
    assert_eq!(
        entry.get("encode_pixel").and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        entry.get("decode_pixel").and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        entry.pointer("/feature_flags/0").and_then(Value::as_str),
        Some("legacy_jpeg_dcmtk")
    );
    assert!(
        entry
            .get("notes")
            .and_then(Value::as_str)
            .is_some_and(|notes| {
                notes.contains("independent 12-bit JPEG Extended validation path")
                    && notes.contains("Unsupported(SamplePrecision(12))")
                    && notes.contains("dcmdjpeg is not considered independent")
            }),
        "matrix row should explain the DCMTK encode proof and independent-validation defer reason"
    );
    assert!(
        entry
            .get("standards_evidence")
            .and_then(Value::as_array)
            .is_some_and(|evidence| evidence.iter().any(|item| {
                item.get("query").and_then(Value::as_str) == Some("lookup_uid JPEGExtended12Bit")
            })),
        "JPEG Extended 12-bit matrix row must carry PS3.6 evidence"
    );
}

#[test]
fn legacy_jpeg_backend_decision_records_dcmtk_generated_case_promotion() {
    let decisions = read_json("transfer-syntax/backend-decisions.json");
    let families = decisions
        .get("codec_families")
        .and_then(Value::as_array)
        .expect("backend decisions must contain codec_families");
    let legacy_jpeg = families
        .iter()
        .find(|family| family.get("family_id").and_then(Value::as_str) == Some("legacy_jpeg"))
        .expect("legacy JPEG decision must exist");

    assert_eq!(
        legacy_jpeg.get("classification").and_then(Value::as_str),
        Some("implement_now"),
        "legacy JPEG SV1 should be promoted after generated-case validation exists"
    );
    assert_eq!(
        legacy_jpeg.get("selected_backend").and_then(Value::as_str),
        Some("dcmtk_dcmcjpeg_external_command_wrapper")
    );
    assert_eq!(
        legacy_jpeg.get("backend_kind").and_then(Value::as_str),
        Some("external_command")
    );
    assert_eq!(
        legacy_jpeg.get("feature_gate").and_then(Value::as_str),
        Some("legacy_jpeg_dcmtk"),
        "legacy JPEG should record the project feature gate for the wrapper"
    );
    assert_eq!(
        legacy_jpeg
            .pointer("/integration_mode/status")
            .and_then(Value::as_str),
        Some("generated_case_added")
    );
    assert_eq!(
        legacy_jpeg
            .pointer("/integration_mode/command")
            .and_then(Value::as_str),
        Some("dcmcjpeg")
    );
    assert_eq!(
        legacy_jpeg
            .pointer("/integration_mode/preferred_first_uid")
            .and_then(Value::as_str),
        Some("1.2.840.10008.1.2.4.70"),
        "the first legacy JPEG spike should target JPEG Lossless SV1"
    );
    assert_eq!(
        legacy_jpeg.get("first_case_target").and_then(Value::as_str),
        Some("classic/sc/mono2_u16_jpeg_lossless_sv1")
    );
    assert_eq!(
        legacy_jpeg
            .get("transfer_syntax_uids")
            .and_then(Value::as_array)
            .expect("legacy JPEG transfer syntax UID list should exist")
            .iter()
            .filter_map(Value::as_str)
            .collect::<Vec<_>>(),
        vec![
            "1.2.840.10008.1.2.4.51",
            "1.2.840.10008.1.2.4.57",
            "1.2.840.10008.1.2.4.70"
        ]
    );

    let candidates = legacy_jpeg
        .get("candidate_backends")
        .and_then(Value::as_array)
        .expect("legacy JPEG decision should compare candidate backends");
    let dcmtk = candidates
        .iter()
        .find(|candidate| candidate.get("name").and_then(Value::as_str) == Some("DCMTK dcmcjpeg"))
        .expect("DCMTK dcmcjpeg candidate should be recorded");
    assert_eq!(
        dcmtk.get("status").and_then(Value::as_str),
        Some("passed_lossless_spikes_extended_deferred")
    );
    assert_eq!(
        dcmtk.get("backend_kind").and_then(Value::as_str),
        Some("external_command")
    );
    assert!(
        dcmtk
            .get("license")
            .and_then(Value::as_str)
            .is_some_and(|license| license.contains("BSD-style")),
        "DCMTK licensing evidence should stay compatible with optional local tooling"
    );

    let dicom_rs = candidates
        .iter()
        .find(|candidate| {
            candidate.get("name").and_then(Value::as_str) == Some("Pinned DICOM-rs JPEG adapter")
        })
        .expect("DICOM-rs JPEG adapter candidate should be recorded");
    assert_eq!(
        dicom_rs.get("status").and_then(Value::as_str),
        Some("decode_only_for_legacy_processes")
    );

    let blockers = legacy_jpeg
        .get("blockers")
        .and_then(Value::as_array)
        .expect("legacy JPEG blockers should be recorded")
        .iter()
        .filter_map(Value::as_str)
        .collect::<Vec<_>>();
    assert!(
        blockers.is_empty(),
        "legacy JPEG should have no active blockers once JPEG Extended 12-bit is deferred"
    );
    assert!(
        !blockers
            .iter()
            .any(|blocker| blocker.contains("JPEG Lossless Process 14")),
        "legacy JPEG should not leave Process 14 generated-case promotion as follow-up work after promotion"
    );

    let deferred_variants = legacy_jpeg
        .get("deferred_variants")
        .and_then(Value::as_array)
        .expect("legacy JPEG should record deferred variants");
    let jpeg_extended = deferred_variants
        .iter()
        .find(|variant| {
            variant.get("transfer_syntax_uid").and_then(Value::as_str)
                == Some("1.2.840.10008.1.2.4.51")
        })
        .expect("JPEG Extended 12-bit should be explicitly deferred");
    assert_eq!(
        jpeg_extended.get("classification").and_then(Value::as_str),
        Some("defer")
    );
    assert!(
        jpeg_extended
            .get("defer_reason")
            .and_then(Value::as_str)
            .is_some_and(|reason| {
                reason.contains("JPEG Extended 12-bit")
                    && reason.contains("Unsupported(SamplePrecision(12))")
            }),
        "legacy JPEG should record that JPEG Extended generated-case promotion is deferred on 12-bit decode validation"
    );
    assert!(
        jpeg_extended
            .get("required_before_implementation")
            .and_then(Value::as_array)
            .is_some_and(|requirements| requirements.iter().any(|item| {
                item.as_str()
                    .is_some_and(|text| text.contains("independent of DCMTK dcmcjpeg"))
            })),
        "deferred JPEG Extended work should require an independent validation path"
    );

    let cargo_toml = fs::read_to_string("Cargo.toml").expect("Cargo.toml must be readable");
    assert!(
        cargo_toml.contains("legacy_jpeg_dcmtk = [") && cargo_toml.contains("\"jpeg\""),
        "Cargo.toml should expose a legacy_jpeg_dcmtk feature through the JPEG reader stack"
    );

    let evidence = legacy_jpeg
        .get("evidence")
        .and_then(Value::as_array)
        .expect("legacy JPEG evidence should be recorded");
    assert!(
        evidence.iter().any(|item| {
            item.get("source").and_then(Value::as_str) == Some("official-upstream-docs")
                && item.get("url").and_then(Value::as_str)
                    == Some("https://support.dcmtk.org/docs/dcmcjpeg.html")
                && item
                    .get("finding")
                    .and_then(Value::as_str)
                    .is_some_and(|finding| {
                        finding.contains("JPEG Lossless SV1")
                            && finding.contains("Basic Offset Table")
                    })
        }),
        "legacy JPEG decision should cite the DCMTK dcmcjpeg command evidence"
    );
    assert!(
        evidence.iter().any(|item| {
            item.get("source").and_then(Value::as_str) == Some("local-environment")
                && item
                    .get("finding")
                    .and_then(Value::as_str)
                    .is_some_and(|finding| {
                        finding.contains("dcmcjpeg is available")
                            && finding.contains("/opt/homebrew/bin/cjpeg")
                    })
        }),
        "legacy JPEG decision should record the current local command availability"
    );
    assert!(
        evidence.iter().any(|item| {
            item.get("source").and_then(Value::as_str) == Some("local-spike-test")
                && item.get("path").and_then(Value::as_str) == Some("tests/legacy_jpeg_spike.rs")
                && item
                    .get("finding")
                    .and_then(Value::as_str)
                    .is_some_and(|finding| {
                        finding.contains("JPEG Lossless Process 14")
                            && finding.contains("decoded exactly")
                            && finding.contains("repeated byte-identically")
                    })
        }),
        "legacy JPEG decision should record the passed DCMTK SV1 and Process 14 spike evidence"
    );
    assert!(
        evidence.iter().any(|item| {
            item.get("source").and_then(Value::as_str) == Some("local-spike-test")
                && item.get("path").and_then(Value::as_str) == Some("tests/legacy_jpeg_spike.rs")
                && item
                    .get("finding")
                    .and_then(Value::as_str)
                    .is_some_and(|finding| {
                        finding.contains("JPEG Extended 12-bit")
                            && finding.contains("byte-identically")
                            && finding.contains("Unsupported(SamplePrecision(12))")
                    })
        }),
        "legacy JPEG decision should record the JPEG Extended 12-bit encode proof and decode blocker"
    );
    assert!(
        evidence.iter().any(|item| {
            item.get("source").and_then(Value::as_str) == Some("local-wrapper-test")
                && item.get("path").and_then(Value::as_str)
                    == Some("tests/legacy_jpeg_dcmtk_wrapper.rs")
                && item
                    .get("finding")
                    .and_then(Value::as_str)
                    .is_some_and(|finding| {
                        finding.contains("legacy_jpeg_dcmtk")
                            && finding.contains("executable SHA-256")
                            && finding.contains("exact decoded native frame bytes")
                    })
        }),
        "legacy JPEG decision should record the project wrapper evidence"
    );
    assert!(
        evidence.iter().any(|item| {
            item.get("source").and_then(Value::as_str) == Some("generated-case-verification")
                && item
                    .get("finding")
                    .and_then(Value::as_str)
                    .is_some_and(|finding| {
                        finding.contains("classic/sc/mono2_u16_jpeg_lossless_process_14")
                            && finding.contains("classic/sc/mono2_u16_jpeg_lossless_sv1")
                            && finding.contains("runtime executable SHA-256")
                            && finding.contains("exact decoded native frame hashes")
                    })
        }),
        "legacy JPEG decision should record generated-case verification evidence"
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
fn deflated_image_frame_decision_selects_segmentation_target_and_promotes_case() {
    let cargo_toml = fs::read_to_string("Cargo.toml").expect("Cargo.toml must be readable");
    assert!(
        cargo_toml.contains("deflate = [")
            && cargo_toml.contains("\"dicom-transfer-syntax-registry/deflate\""),
        "project deflate feature should expose the pinned DICOM-rs deflated image frame adapter"
    );

    let matrix = read_json("transfer-syntax/capability-matrix.json");
    let matrix_entries = matrix
        .get("entries")
        .and_then(Value::as_array)
        .expect("transfer syntax matrix must contain entries");
    let matrix_entry = matrix_entries
        .iter()
        .find(|entry| entry.get("uid").and_then(Value::as_str) == Some("1.2.840.10008.1.2.8.1"))
        .expect("transfer syntax matrix must contain Deflated Image Frame Compression");
    assert_eq!(
        matrix_entry.get("keyword").and_then(Value::as_str),
        Some("DeflatedImageFrameCompression")
    );
    assert_eq!(
        matrix_entry.get("status").and_then(Value::as_str),
        Some("feature_gated"),
        "Deflated Image Frame should be feature-gated after generated-case validation"
    );
    assert_eq!(
        matrix_entry.get("decode_pixel").and_then(Value::as_bool),
        Some(true),
        "matrix should claim Deflated Image Frame decode validation after CLI validation exists"
    );
    assert_eq!(
        matrix_entry.get("encode_pixel").and_then(Value::as_bool),
        Some(true),
        "matrix should claim Deflated Image Frame encode validation after corpus generation exists"
    );
    assert!(
        matrix_entry
            .get("feature_flags")
            .and_then(Value::as_array)
            .is_some_and(|features| features
                .iter()
                .any(|feature| feature.as_str() == Some("deflate"))),
        "matrix should record the project deflate feature gate"
    );

    let transfer_syntax = TransferSyntaxRegistry
        .get("1.2.840.10008.1.2.8.1")
        .expect("DICOM-rs registry must expose Deflated Image Frame Compression");
    assert!(
        transfer_syntax.is_encapsulated_pixel_data(),
        "Deflated Image Frame should use encapsulated Pixel Data"
    );
    if cfg!(feature = "deflate") {
        assert!(
            transfer_syntax.pixel_data_reader().is_some(),
            "deflate feature builds should expose a runtime Deflated Image Frame decoder"
        );
        assert!(
            transfer_syntax.pixel_data_writer().is_some(),
            "deflate feature builds should expose a runtime Deflated Image Frame encoder"
        );
    }

    let decisions = read_json("transfer-syntax/backend-decisions.json");
    let deflated = decisions
        .get("codec_families")
        .and_then(Value::as_array)
        .expect("backend decisions must contain codec_families")
        .iter()
        .find(|family| {
            family.get("family_id").and_then(Value::as_str) == Some("deflated_image_frame")
        })
        .expect("Deflated Image Frame backend decision must exist");
    assert_eq!(
        deflated.get("classification").and_then(Value::as_str),
        Some("implement_now")
    );
    assert_eq!(
        deflated.get("selected_backend").and_then(Value::as_str),
        Some("dicom_rs_deflated_image_frame_adapter")
    );
    assert_eq!(
        deflated.get("backend_kind").and_then(Value::as_str),
        Some("dicom_rs_feature")
    );
    assert_eq!(
        deflated.get("feature_gate").and_then(Value::as_str),
        Some("deflate")
    );
    assert_eq!(
        deflated.get("first_case_target").and_then(Value::as_str),
        Some("derived/seg/binary_multiframe_deflated_image_frame"),
        "standards suitability decision should target the existing binary Segmentation family first"
    );
    assert!(
        deflated
            .get("validation_strategy")
            .and_then(Value::as_array)
            .is_some_and(|steps| steps.iter().any(|step| {
                step.as_str()
                    .is_some_and(|text| text.contains("one-fragment-per-frame"))
            })),
        "Deflated Image Frame validation should preserve the one-fragment-per-frame rule"
    );
    assert!(
        deflated
            .get("evidence")
            .and_then(Value::as_array)
            .is_some_and(|evidence| evidence.iter().any(|item| {
                item.get("part").and_then(Value::as_str) == Some("PS3.5")
                    && item
                        .get("anchor")
                        .and_then(Value::as_str)
                        .is_some_and(|anchor| anchor.contains("sect_A.4.13"))
                    && item
                        .get("finding")
                        .and_then(Value::as_str)
                        .is_some_and(|finding| finding.contains("one and only one fragment"))
            })),
        "Deflated Image Frame decision should cite PS3.5 fragment layout evidence"
    );

    let registry = read_json("cases/registry.json");
    let deflated_seg_case = registry
        .get("cases")
        .and_then(Value::as_array)
        .expect("registry must contain cases")
        .iter()
        .find(|case| {
            case.get("case_id").and_then(Value::as_str)
                == Some("derived/seg/binary_multiframe_deflated_image_frame")
        })
        .expect("registry must contain the Deflated Image Frame SEG case");
    assert_eq!(
        deflated_seg_case.get("status").and_then(Value::as_str),
        Some("implemented")
    );
    assert_eq!(
        deflated_seg_case
            .get("transfer_syntax_uid")
            .and_then(Value::as_str),
        Some("1.2.840.10008.1.2.8.1")
    );
    assert_eq!(
        deflated_seg_case
            .pointer("/requirements/features/0")
            .and_then(Value::as_str),
        Some("deflate")
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
fn htj2k_lossless_transfer_syntax_has_project_openjph_feature_gate() {
    let matrix = read_json("transfer-syntax/capability-matrix.json");
    let entries = matrix
        .get("entries")
        .and_then(Value::as_array)
        .expect("transfer syntax matrix must contain entries");
    let uid = "1.2.840.10008.1.2.4.201";
    let entry = entries
        .iter()
        .find(|entry| entry.get("uid").and_then(Value::as_str) == Some(uid))
        .expect("transfer syntax matrix must contain HTJ2K Lossless");
    let transfer_syntax = TransferSyntaxRegistry
        .get(uid)
        .expect("DICOM-rs registry must expose HTJ2K Lossless");

    assert_eq!(
        entry.get("keyword").and_then(Value::as_str),
        Some("HTJ2KLossless")
    );
    assert_eq!(
        entry.get("status").and_then(Value::as_str),
        Some("feature_gated"),
        "HTJ2K generation should be available when the project htj2k_openjph feature is enabled"
    );
    assert_eq!(
        entry.get("decode_pixel").and_then(Value::as_bool),
        Some(true),
        "the committed matrix should claim HTJ2K decode validation behind the project feature"
    );
    assert_eq!(
        entry.get("encode_pixel").and_then(Value::as_bool),
        Some(true),
        "the committed matrix should claim HTJ2K encode validation behind the project wrapper"
    );
    assert!(
        transfer_syntax.is_encapsulated_pixel_data(),
        "HTJ2K should use encapsulated Pixel Data"
    );
    assert!(
        entry
            .get("feature_flags")
            .and_then(Value::as_array)
            .is_some_and(|features| features
                .iter()
                .any(|value| value.as_str() == Some("htj2k_openjph"))),
        "HTJ2K should record the project OpenJPH feature gate"
    );
    if cfg!(feature = "htj2k_openjph") {
        assert!(
            transfer_syntax.pixel_data_reader().is_some(),
            "htj2k_openjph feature builds should expose a runtime HTJ2K decoder"
        );
    }
    assert!(
        transfer_syntax.pixel_data_writer().is_none(),
        "pinned DICOM-rs HTJ2K support does not provide a pixel writer"
    );
}

#[test]
fn jpeg_xl_lossless_transfer_syntax_has_project_jpegxl_feature_gate() {
    let cargo_toml = fs::read_to_string("Cargo.toml").expect("Cargo.toml must be readable");
    assert!(
        cargo_toml.contains("jpegxl = [")
            && cargo_toml.contains("\"dicom-transfer-syntax-registry/jpegxl\""),
        "Cargo.toml should expose a project jpegxl feature for the pinned DICOM-rs adapter"
    );

    let matrix = read_json("transfer-syntax/capability-matrix.json");
    let entries = matrix
        .get("entries")
        .and_then(Value::as_array)
        .expect("transfer syntax matrix must contain entries");
    let entry = entries
        .iter()
        .find(|entry| entry.get("uid").and_then(Value::as_str) == Some("1.2.840.10008.1.2.4.110"))
        .expect("transfer syntax matrix must contain JPEG XL Lossless");
    let transfer_syntax = TransferSyntaxRegistry
        .get("1.2.840.10008.1.2.4.110")
        .expect("DICOM-rs registry must expose JPEG XL Lossless");

    assert_eq!(
        entry.get("keyword").and_then(Value::as_str),
        Some("JPEGXLLossless")
    );
    assert_eq!(
        entry.get("status").and_then(Value::as_str),
        Some("feature_gated"),
        "JPEG XL generation should be available when the project jpegxl feature is enabled"
    );
    assert_eq!(
        entry.get("decode_pixel").and_then(Value::as_bool),
        Some(true),
        "the committed matrix should claim JPEG XL decode validation behind the jpegxl feature"
    );
    assert_eq!(
        entry.get("encode_pixel").and_then(Value::as_bool),
        Some(true),
        "the committed matrix should claim JPEG XL encode validation behind the jpegxl feature"
    );
    assert!(
        entry
            .get("feature_flags")
            .and_then(Value::as_array)
            .is_some_and(|features| features
                .iter()
                .any(|value| value.as_str() == Some("jpegxl"))),
        "JPEG XL should record the project jpegxl feature gate"
    );
    if cfg!(feature = "jpegxl") {
        assert!(
            transfer_syntax.pixel_data_reader().is_some(),
            "jpegxl feature builds should expose a runtime JPEG XL decoder"
        );
        assert!(
            transfer_syntax.pixel_data_writer().is_some(),
            "jpegxl feature builds should expose a runtime JPEG XL Lossless encoder"
        );
    }

    let registry = read_json("cases/registry.json");
    let cases = registry_cases(&registry);
    let case = cases
        .iter()
        .find(|case| {
            case.get("case_id").and_then(Value::as_str)
                == Some("classic/sc/rgb_planar0_jpegxl_lossless")
        })
        .expect("registry must contain JPEG XL Lossless SC case");
    assert_eq!(
        case.get("status").and_then(Value::as_str),
        Some("implemented")
    );
    assert_eq!(
        case.pointer("/requirements/features/0")
            .and_then(Value::as_str),
        Some("jpegxl")
    );
    assert_eq!(
        case.get("skip"),
        Some(&Value::Null),
        "implemented JPEG XL case should not retain skip metadata"
    );

    let decisions = read_json("transfer-syntax/backend-decisions.json");
    let jpeg_xl = decisions
        .get("codec_families")
        .and_then(Value::as_array)
        .expect("backend decisions must contain codec_families")
        .iter()
        .find(|family| family.get("family_id").and_then(Value::as_str) == Some("jpeg_xl"))
        .expect("JPEG XL backend decision must exist");
    assert_eq!(
        jpeg_xl.get("feature_gate").and_then(Value::as_str),
        Some("jpegxl")
    );
    assert!(
        jpeg_xl
            .get("evidence")
            .and_then(Value::as_array)
            .is_some_and(|items| items.iter().any(|item| item
                .get("finding")
                .and_then(Value::as_str)
                .is_some_and(|finding| finding.contains("jxl-oxide 0.10.2")
                    && finding.contains("zune-jpegxl 0.4.0")))),
        "JPEG XL decision should record backend version behavior from the local codec test"
    );
    assert!(
        jpeg_xl
            .get("evidence")
            .and_then(Value::as_array)
            .is_some_and(|items| items.iter().any(|item| item
                .get("finding")
                .and_then(Value::as_str)
                .is_some_and(|finding| finding.contains("validates exact decoded frame hashes")))),
        "JPEG XL decision should record generated-case validation evidence"
    );
}

#[test]
fn jpeg_2000_lossless_transfer_syntax_has_project_jpeg2000_feature_gate() {
    let cargo_toml = fs::read_to_string("Cargo.toml").expect("Cargo.toml must be readable");
    assert!(
        cargo_toml.contains("jpeg2000 = ["),
        "the JPEG 2000 project feature should exist once the codec wrapper is added"
    );
    assert!(
        cargo_toml.contains("\"dicom-transfer-syntax-registry/openjp2\""),
        "the JPEG 2000 project feature should enable the pinned DICOM-rs OpenJPEG reader"
    );
    assert!(
        cargo_toml.contains("\"dep:openjp2\""),
        "the JPEG 2000 project feature should expose the project OpenJPEG-rs writer dependency"
    );

    let matrix = read_json("transfer-syntax/capability-matrix.json");
    let entries = matrix
        .get("entries")
        .and_then(Value::as_array)
        .expect("transfer syntax matrix must contain entries");
    let entry = entries
        .iter()
        .find(|entry| entry.get("uid").and_then(Value::as_str) == Some("1.2.840.10008.1.2.4.90"))
        .expect("transfer syntax matrix must contain JPEG 2000 Lossless");
    let transfer_syntax = TransferSyntaxRegistry
        .get("1.2.840.10008.1.2.4.90")
        .expect("DICOM-rs registry must expose JPEG 2000 Lossless");

    assert_eq!(
        entry.get("keyword").and_then(Value::as_str),
        Some("JPEG2000Lossless")
    );
    assert_eq!(
        entry.get("status").and_then(Value::as_str),
        Some("feature_gated"),
        "JPEG 2000 generation should be available when the project jpeg2000 feature is enabled"
    );
    assert_eq!(
        entry.get("decode_pixel").and_then(Value::as_bool),
        Some(true),
        "the committed matrix should claim JPEG 2000 decode validation behind the jpeg2000 feature"
    );
    assert_eq!(
        entry.get("encode_pixel").and_then(Value::as_bool),
        Some(true),
        "the committed matrix should claim JPEG 2000 encode validation behind the jpeg2000 feature"
    );
    assert!(
        entry
            .get("feature_flags")
            .and_then(Value::as_array)
            .is_some_and(|features| features
                .iter()
                .any(|value| value.as_str() == Some("jpeg2000"))),
        "JPEG 2000 should record the project jpeg2000 feature gate"
    );
    if cfg!(feature = "jpeg2000") {
        assert!(
            transfer_syntax.pixel_data_reader().is_some(),
            "jpeg2000 feature builds should expose a runtime JPEG 2000 decoder"
        );
    }
    assert!(
        transfer_syntax.pixel_data_writer().is_none(),
        "pinned DICOM-rs JPEG 2000 support does not provide a pixel writer"
    );

    let registry = read_json("cases/registry.json");
    let cases = registry
        .get("cases")
        .and_then(Value::as_array)
        .expect("case registry must contain cases");
    let case = cases
        .iter()
        .find(|case| {
            case.get("case_id").and_then(Value::as_str)
                == Some("classic/sc/mono2_u16_jpeg2000_lossless")
        })
        .expect("case registry must contain the planned JPEG 2000 Lossless case");
    assert_eq!(
        case.get("status").and_then(Value::as_str),
        Some("implemented"),
        "JPEG 2000 generation should be implemented behind the project feature"
    );
    assert_eq!(
        case.pointer("/requirements/features/0")
            .and_then(Value::as_str),
        Some("jpeg2000")
    );
    assert_eq!(
        case.get("skip"),
        Some(&Value::Null),
        "implemented JPEG 2000 case should not retain skip metadata"
    );

    let decisions = read_json("transfer-syntax/backend-decisions.json");
    let families = decisions
        .get("codec_families")
        .and_then(Value::as_array)
        .expect("backend decisions must contain codec_families");
    let jpeg_2000 = families
        .iter()
        .find(|family| family.get("family_id").and_then(Value::as_str) == Some("jpeg_2000"))
        .expect("JPEG 2000 backend decision must exist");
    assert_eq!(
        jpeg_2000.get("classification").and_then(Value::as_str),
        Some("implement_now")
    );
    assert_eq!(
        jpeg_2000.get("selected_backend").and_then(Value::as_str),
        Some("project_jpeg2k_openjp2_lossless_adapter")
    );
    assert!(
        jpeg_2000
            .get("evidence")
            .and_then(Value::as_array)
            .is_some_and(|items| items.iter().any(|item| item
                .get("finding")
                .and_then(Value::as_str)
                .is_some_and(|finding| finding.contains("NeverPixelAdapter")))),
        "JPEG 2000 decision should record why a project writer wrapper is needed"
    );
    assert!(
        jpeg_2000
            .get("evidence")
            .and_then(Value::as_array)
            .is_some_and(|items| items.iter().any(|item| item
                .get("finding")
                .and_then(Value::as_str)
                .is_some_and(
                    |finding| finding.contains("BSD-2-Clause") && finding.contains("openjp2")
                ))),
        "JPEG 2000 decision should record selected backend licensing evidence"
    );
    assert!(
        jpeg_2000
            .get("evidence")
            .and_then(Value::as_array)
            .is_some_and(|items| items.iter().any(|item| item
                .get("finding")
                .and_then(Value::as_str)
                .is_some_and(|finding| finding.contains("validates exact decoded frame hashes")))),
        "JPEG 2000 decision should record generated-case validation evidence"
    );
    assert_eq!(
        jpeg_2000
            .pointer("/lossy_policy/classification")
            .and_then(Value::as_str),
        Some("defer"),
        "lossy JPEG 2000 should remain deferred until lossy policy is selected"
    );
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
fn htj2k_lossless_registry_row_is_feature_gated_implemented() {
    let matrix = read_json("transfer-syntax/capability-matrix.json");
    let matrix_entries = matrix
        .get("entries")
        .and_then(Value::as_array)
        .expect("transfer syntax matrix must contain entries");
    let registry = read_json("cases/registry.json");
    let cases = registry_cases(&registry);

    let case_id = "classic/sc/mono2_u16_htj2k_lossless";
    let uid = "1.2.840.10008.1.2.4.201";
    let matrix_entry = matrix_entries
        .iter()
        .find(|entry| entry.get("uid").and_then(Value::as_str) == Some(uid))
        .expect("transfer syntax matrix must contain HTJ2K Lossless");
    assert_eq!(
        matrix_entry.get("keyword").and_then(Value::as_str),
        Some("HTJ2KLossless")
    );
    assert_eq!(
        matrix_entry.get("status").and_then(Value::as_str),
        Some("feature_gated"),
        "{case_id} should be feature-gated after generated-case validation"
    );

    let case = cases
        .iter()
        .find(|case| case.get("case_id").and_then(Value::as_str) == Some(case_id))
        .expect("registry must contain HTJ2K Lossless case");
    assert_eq!(
        case.get("status").and_then(Value::as_str),
        Some("implemented")
    );
    assert_eq!(case.get("skip"), Some(&Value::Null));
    assert_eq!(
        case.get("transfer_syntax_uid").and_then(Value::as_str),
        Some(uid)
    );
    assert_eq!(
        case.get("determinism").and_then(Value::as_str),
        Some("semantic_stable")
    );
    assert_eq!(
        case.pointer("/requirements/features/0")
            .and_then(Value::as_str),
        Some("htj2k_openjph"),
        "{case_id} should name the project OpenJPH wrapper feature gate"
    );
    assert!(
        case.get("standards_evidence")
            .and_then(Value::as_array)
            .is_some_and(|evidence| evidence.iter().any(|entry| {
                entry.get("query").and_then(Value::as_str)
                    == Some("lookup_sop_class Secondary Capture Image Storage")
            }) && evidence.iter().any(|entry| {
                entry.get("query").and_then(Value::as_str) == Some("lookup_uid HTJ2KLossless")
            })),
        "{case_id} must carry SC SOP Class and transfer syntax evidence"
    );
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

    let odd_3x3_case = cases
        .iter()
        .find(|case| {
            case.get("case_id").and_then(Value::as_str)
                == Some("classic/sc/mono2_u16_odd_3x3_rle_lossless")
        })
        .expect("registry must contain odd 3x3 RLE Lossless SC case");
    assert_eq!(
        odd_3x3_case.get("status").and_then(Value::as_str),
        Some("implemented")
    );
    assert_eq!(odd_3x3_case.get("skip"), Some(&Value::Null));
    assert_eq!(
        odd_3x3_case.get("determinism").and_then(Value::as_str),
        Some("byte_stable")
    );

    let mono1_odd_3x3_case = cases
        .iter()
        .find(|case| {
            case.get("case_id").and_then(Value::as_str)
                == Some("classic/sc/mono1_u16_odd_3x3_rle_lossless")
        })
        .expect("registry must contain MONOCHROME1 unsigned odd 3x3 RLE Lossless SC case");
    assert_eq!(
        mono1_odd_3x3_case.get("status").and_then(Value::as_str),
        Some("implemented")
    );
    assert_eq!(mono1_odd_3x3_case.get("skip"), Some(&Value::Null));
    assert_eq!(
        mono1_odd_3x3_case
            .get("determinism")
            .and_then(Value::as_str),
        Some("byte_stable")
    );

    let signed_odd_3x3_case = cases
        .iter()
        .find(|case| {
            case.get("case_id").and_then(Value::as_str)
                == Some("classic/sc/mono2_i16_odd_3x3_rle_lossless")
        })
        .expect("registry must contain signed odd 3x3 RLE Lossless SC case");
    assert_eq!(
        signed_odd_3x3_case.get("status").and_then(Value::as_str),
        Some("implemented")
    );
    assert_eq!(signed_odd_3x3_case.get("skip"), Some(&Value::Null));
    assert_eq!(
        signed_odd_3x3_case
            .get("determinism")
            .and_then(Value::as_str),
        Some("byte_stable")
    );

    let mono1_signed_odd_3x3_case = cases
        .iter()
        .find(|case| {
            case.get("case_id").and_then(Value::as_str)
                == Some("classic/sc/mono1_i16_odd_3x3_rle_lossless")
        })
        .expect("registry must contain MONOCHROME1 signed odd 3x3 RLE Lossless SC case");
    assert_eq!(
        mono1_signed_odd_3x3_case
            .get("status")
            .and_then(Value::as_str),
        Some("implemented")
    );
    assert_eq!(mono1_signed_odd_3x3_case.get("skip"), Some(&Value::Null));
    assert_eq!(
        mono1_signed_odd_3x3_case
            .get("determinism")
            .and_then(Value::as_str),
        Some("byte_stable")
    );

    let rect_case = cases
        .iter()
        .find(|case| {
            case.get("case_id").and_then(Value::as_str)
                == Some("classic/sc/mono2_u16_rect_2x3_rle_lossless")
        })
        .expect("registry must contain rectangular 2x3 RLE Lossless SC case");
    assert_eq!(
        rect_case.get("status").and_then(Value::as_str),
        Some("implemented")
    );
    assert_eq!(rect_case.get("skip"), Some(&Value::Null));
    assert_eq!(
        rect_case.get("determinism").and_then(Value::as_str),
        Some("byte_stable")
    );

    let mono1_rect_case = cases
        .iter()
        .find(|case| {
            case.get("case_id").and_then(Value::as_str)
                == Some("classic/sc/mono1_u16_rect_2x3_rle_lossless")
        })
        .expect("registry must contain MONOCHROME1 rectangular 2x3 RLE Lossless SC case");
    assert_eq!(
        mono1_rect_case.get("status").and_then(Value::as_str),
        Some("implemented")
    );
    assert_eq!(mono1_rect_case.get("skip"), Some(&Value::Null));
    assert_eq!(
        mono1_rect_case.get("determinism").and_then(Value::as_str),
        Some("byte_stable")
    );

    let signed_rect_case = cases
        .iter()
        .find(|case| {
            case.get("case_id").and_then(Value::as_str)
                == Some("classic/sc/mono2_i16_rect_2x3_rle_lossless")
        })
        .expect("registry must contain signed rectangular 2x3 RLE Lossless SC case");
    assert_eq!(
        signed_rect_case.get("status").and_then(Value::as_str),
        Some("implemented")
    );
    assert_eq!(signed_rect_case.get("skip"), Some(&Value::Null));
    assert_eq!(
        signed_rect_case.get("determinism").and_then(Value::as_str),
        Some("byte_stable")
    );

    let mono1_signed_rect_case = cases
        .iter()
        .find(|case| {
            case.get("case_id").and_then(Value::as_str)
                == Some("classic/sc/mono1_i16_rect_2x3_rle_lossless")
        })
        .expect("registry must contain MONOCHROME1 signed rectangular 2x3 RLE Lossless SC case");
    assert_eq!(
        mono1_signed_rect_case.get("status").and_then(Value::as_str),
        Some("implemented")
    );
    assert_eq!(mono1_signed_rect_case.get("skip"), Some(&Value::Null));
    assert_eq!(
        mono1_signed_rect_case
            .get("determinism")
            .and_then(Value::as_str),
        Some("byte_stable")
    );

    let tiny_case = cases
        .iter()
        .find(|case| {
            case.get("case_id").and_then(Value::as_str)
                == Some("classic/sc/mono2_u16_tiny_1x1_rle_lossless")
        })
        .expect("registry must contain tiny 1x1 RLE Lossless SC case");
    assert_eq!(
        tiny_case.get("status").and_then(Value::as_str),
        Some("implemented")
    );
    assert_eq!(tiny_case.get("skip"), Some(&Value::Null));
    assert_eq!(
        tiny_case.get("determinism").and_then(Value::as_str),
        Some("byte_stable")
    );

    let mono1_tiny_case = cases
        .iter()
        .find(|case| {
            case.get("case_id").and_then(Value::as_str)
                == Some("classic/sc/mono1_u16_tiny_1x1_rle_lossless")
        })
        .expect("registry must contain MONOCHROME1 tiny 1x1 RLE Lossless SC case");
    assert_eq!(
        mono1_tiny_case.get("status").and_then(Value::as_str),
        Some("implemented")
    );
    assert_eq!(mono1_tiny_case.get("skip"), Some(&Value::Null));
    assert_eq!(
        mono1_tiny_case.get("determinism").and_then(Value::as_str),
        Some("byte_stable")
    );

    let signed_tiny_case = cases
        .iter()
        .find(|case| {
            case.get("case_id").and_then(Value::as_str)
                == Some("classic/sc/mono2_i16_tiny_1x1_rle_lossless")
        })
        .expect("registry must contain signed tiny 1x1 RLE Lossless SC case");
    assert_eq!(
        signed_tiny_case.get("status").and_then(Value::as_str),
        Some("implemented")
    );
    assert_eq!(signed_tiny_case.get("skip"), Some(&Value::Null));
    assert_eq!(
        signed_tiny_case.get("determinism").and_then(Value::as_str),
        Some("byte_stable")
    );

    let mono1_signed_tiny_case = cases
        .iter()
        .find(|case| {
            case.get("case_id").and_then(Value::as_str)
                == Some("classic/sc/mono1_i16_tiny_1x1_rle_lossless")
        })
        .expect("registry must contain MONOCHROME1 signed tiny 1x1 RLE Lossless SC case");
    assert_eq!(
        mono1_signed_tiny_case.get("status").and_then(Value::as_str),
        Some("implemented")
    );
    assert_eq!(mono1_signed_tiny_case.get("skip"), Some(&Value::Null));
    assert_eq!(
        mono1_signed_tiny_case
            .get("determinism")
            .and_then(Value::as_str),
        Some("byte_stable")
    );
    assert_eq!(
        mono1_signed_tiny_case
            .get("standards_evidence")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(7)
    );

    let u16_multiframe_case = cases
        .iter()
        .find(|case| {
            case.get("case_id").and_then(Value::as_str)
                == Some("classic/sc/mono2_u16_multiframe_rle_lossless")
        })
        .expect("registry must contain 16-bit multi-frame RLE Lossless SC case");
    assert_eq!(
        u16_multiframe_case.get("status").and_then(Value::as_str),
        Some("implemented")
    );
    assert_eq!(u16_multiframe_case.get("skip"), Some(&Value::Null));
    assert_eq!(
        u16_multiframe_case
            .get("determinism")
            .and_then(Value::as_str),
        Some("byte_stable")
    );

    let mono1_case = cases
        .iter()
        .find(|case| {
            case.get("case_id").and_then(Value::as_str) == Some("classic/sc/mono1_u8_rle_lossless")
        })
        .expect("registry must contain MONOCHROME1 RLE Lossless SC case");
    assert_eq!(
        mono1_case.get("status").and_then(Value::as_str),
        Some("implemented")
    );
    assert_eq!(mono1_case.get("skip"), Some(&Value::Null));
    assert_eq!(
        mono1_case.get("determinism").and_then(Value::as_str),
        Some("byte_stable")
    );

    let mono1_multiframe_case = cases
        .iter()
        .find(|case| {
            case.get("case_id").and_then(Value::as_str)
                == Some("classic/sc/mono1_u8_multiframe_rle_lossless")
        })
        .expect("registry must contain MONOCHROME1 multi-frame RLE Lossless SC case");
    assert_eq!(
        mono1_multiframe_case.get("status").and_then(Value::as_str),
        Some("implemented")
    );
    assert_eq!(mono1_multiframe_case.get("skip"), Some(&Value::Null));
    assert_eq!(
        mono1_multiframe_case
            .get("determinism")
            .and_then(Value::as_str),
        Some("byte_stable")
    );

    let mono1_u16_case = cases
        .iter()
        .find(|case| {
            case.get("case_id").and_then(Value::as_str) == Some("classic/sc/mono1_u16_rle_lossless")
        })
        .expect("registry must contain 16-bit MONOCHROME1 RLE Lossless SC case");
    assert_eq!(
        mono1_u16_case.get("status").and_then(Value::as_str),
        Some("implemented")
    );
    assert_eq!(mono1_u16_case.get("skip"), Some(&Value::Null));
    assert_eq!(
        mono1_u16_case.get("determinism").and_then(Value::as_str),
        Some("byte_stable")
    );

    let mono1_u16_multiframe_case = cases
        .iter()
        .find(|case| {
            case.get("case_id").and_then(Value::as_str)
                == Some("classic/sc/mono1_u16_multiframe_rle_lossless")
        })
        .expect("registry must contain 16-bit MONOCHROME1 multi-frame RLE Lossless SC case");
    assert_eq!(
        mono1_u16_multiframe_case
            .get("status")
            .and_then(Value::as_str),
        Some("implemented")
    );
    assert_eq!(mono1_u16_multiframe_case.get("skip"), Some(&Value::Null));
    assert_eq!(
        mono1_u16_multiframe_case
            .get("determinism")
            .and_then(Value::as_str),
        Some("byte_stable")
    );

    let i16_case = cases
        .iter()
        .find(|case| {
            case.get("case_id").and_then(Value::as_str) == Some("classic/sc/mono2_i16_rle_lossless")
        })
        .expect("registry must contain signed 16-bit RLE Lossless SC case");
    assert_eq!(
        i16_case.get("status").and_then(Value::as_str),
        Some("implemented")
    );
    assert_eq!(i16_case.get("skip"), Some(&Value::Null));
    assert_eq!(
        i16_case.get("determinism").and_then(Value::as_str),
        Some("byte_stable")
    );

    let mono1_i16_case = cases
        .iter()
        .find(|case| {
            case.get("case_id").and_then(Value::as_str) == Some("classic/sc/mono1_i16_rle_lossless")
        })
        .expect("registry must contain signed 16-bit MONOCHROME1 RLE Lossless SC case");
    assert_eq!(
        mono1_i16_case.get("status").and_then(Value::as_str),
        Some("implemented")
    );
    assert_eq!(mono1_i16_case.get("skip"), Some(&Value::Null));
    assert_eq!(
        mono1_i16_case.get("determinism").and_then(Value::as_str),
        Some("byte_stable")
    );

    let i16_multiframe_case = cases
        .iter()
        .find(|case| {
            case.get("case_id").and_then(Value::as_str)
                == Some("classic/sc/mono2_i16_multiframe_rle_lossless")
        })
        .expect("registry must contain signed 16-bit multi-frame RLE Lossless SC case");
    assert_eq!(
        i16_multiframe_case.get("status").and_then(Value::as_str),
        Some("implemented")
    );
    assert_eq!(i16_multiframe_case.get("skip"), Some(&Value::Null));
    assert_eq!(
        i16_multiframe_case
            .get("determinism")
            .and_then(Value::as_str),
        Some("byte_stable")
    );

    let mono1_i16_multiframe_case = cases
        .iter()
        .find(|case| {
            case.get("case_id").and_then(Value::as_str)
                == Some("classic/sc/mono1_i16_multiframe_rle_lossless")
        })
        .expect("registry must contain signed 16-bit MONOCHROME1 multi-frame RLE Lossless SC case");
    assert_eq!(
        mono1_i16_multiframe_case
            .get("status")
            .and_then(Value::as_str),
        Some("implemented")
    );
    assert_eq!(mono1_i16_multiframe_case.get("skip"), Some(&Value::Null));
    assert_eq!(
        mono1_i16_multiframe_case
            .get("determinism")
            .and_then(Value::as_str),
        Some("byte_stable")
    );

    let padding_case = cases
        .iter()
        .find(|case| {
            case.get("case_id").and_then(Value::as_str)
                == Some("classic/sc/mono2_u16_padding_rle_lossless")
        })
        .expect("registry must contain Pixel Padding RLE Lossless SC case");
    assert_eq!(
        padding_case.get("status").and_then(Value::as_str),
        Some("implemented")
    );
    assert_eq!(padding_case.get("skip"), Some(&Value::Null));
    assert_eq!(
        padding_case.get("determinism").and_then(Value::as_str),
        Some("byte_stable")
    );

    let u8_padding_case = cases
        .iter()
        .find(|case| {
            case.get("case_id").and_then(Value::as_str)
                == Some("classic/sc/mono2_u8_padding_rle_lossless")
        })
        .expect("registry must contain 8-bit Pixel Padding RLE Lossless SC case");
    assert_eq!(
        u8_padding_case.get("status").and_then(Value::as_str),
        Some("implemented")
    );
    assert_eq!(u8_padding_case.get("skip"), Some(&Value::Null));
    assert_eq!(
        u8_padding_case.get("determinism").and_then(Value::as_str),
        Some("byte_stable")
    );
    assert_eq!(
        u8_padding_case
            .get("standards_evidence")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(8)
    );

    let mono1_u8_padding_case = cases
        .iter()
        .find(|case| {
            case.get("case_id").and_then(Value::as_str)
                == Some("classic/sc/mono1_u8_padding_rle_lossless")
        })
        .expect("registry must contain MONOCHROME1 8-bit Pixel Padding RLE Lossless SC case");
    assert_eq!(
        mono1_u8_padding_case.get("status").and_then(Value::as_str),
        Some("implemented")
    );
    assert_eq!(mono1_u8_padding_case.get("skip"), Some(&Value::Null));
    assert_eq!(
        mono1_u8_padding_case
            .get("determinism")
            .and_then(Value::as_str),
        Some("byte_stable")
    );
    assert_eq!(
        mono1_u8_padding_case
            .get("standards_evidence")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(9)
    );

    let u8_padding_multiframe_case = cases
        .iter()
        .find(|case| {
            case.get("case_id").and_then(Value::as_str)
                == Some("classic/sc/mono2_u8_padding_multiframe_rle_lossless")
        })
        .expect("registry must contain 8-bit multi-frame Pixel Padding RLE Lossless SC case");
    assert_eq!(
        u8_padding_multiframe_case
            .get("status")
            .and_then(Value::as_str),
        Some("implemented")
    );
    assert_eq!(u8_padding_multiframe_case.get("skip"), Some(&Value::Null));
    assert_eq!(
        u8_padding_multiframe_case
            .get("determinism")
            .and_then(Value::as_str),
        Some("byte_stable")
    );
    assert_eq!(
        u8_padding_multiframe_case
            .get("standards_evidence")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(9)
    );

    let mono1_u8_padding_multiframe_case = cases
        .iter()
        .find(|case| {
            case.get("case_id").and_then(Value::as_str)
                == Some("classic/sc/mono1_u8_padding_multiframe_rle_lossless")
        })
        .expect(
            "registry must contain MONOCHROME1 8-bit multi-frame Pixel Padding RLE Lossless SC case",
        );
    assert_eq!(
        mono1_u8_padding_multiframe_case
            .get("status")
            .and_then(Value::as_str),
        Some("implemented")
    );
    assert_eq!(
        mono1_u8_padding_multiframe_case.get("skip"),
        Some(&Value::Null)
    );
    assert_eq!(
        mono1_u8_padding_multiframe_case
            .get("determinism")
            .and_then(Value::as_str),
        Some("byte_stable")
    );
    assert_eq!(
        mono1_u8_padding_multiframe_case
            .get("standards_evidence")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(10)
    );

    let mono1_padding_case = cases
        .iter()
        .find(|case| {
            case.get("case_id").and_then(Value::as_str)
                == Some("classic/sc/mono1_u16_padding_rle_lossless")
        })
        .expect("registry must contain MONOCHROME1 unsigned Pixel Padding RLE Lossless SC case");
    assert_eq!(
        mono1_padding_case.get("status").and_then(Value::as_str),
        Some("implemented")
    );
    assert_eq!(mono1_padding_case.get("skip"), Some(&Value::Null));
    assert_eq!(
        mono1_padding_case
            .get("determinism")
            .and_then(Value::as_str),
        Some("byte_stable")
    );
    assert_eq!(
        mono1_padding_case
            .get("standards_evidence")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(9)
    );

    let mono1_padding_multiframe_case = cases
        .iter()
        .find(|case| {
            case.get("case_id").and_then(Value::as_str)
                == Some("classic/sc/mono1_u16_padding_multiframe_rle_lossless")
        })
        .expect(
            "registry must contain MONOCHROME1 unsigned multi-frame Pixel Padding RLE Lossless SC case",
        );
    assert_eq!(
        mono1_padding_multiframe_case
            .get("status")
            .and_then(Value::as_str),
        Some("implemented")
    );
    assert_eq!(
        mono1_padding_multiframe_case.get("skip"),
        Some(&Value::Null)
    );
    assert_eq!(
        mono1_padding_multiframe_case
            .get("determinism")
            .and_then(Value::as_str),
        Some("byte_stable")
    );
    assert_eq!(
        mono1_padding_multiframe_case
            .get("standards_evidence")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(10)
    );

    let signed_padding_case = cases
        .iter()
        .find(|case| {
            case.get("case_id").and_then(Value::as_str)
                == Some("classic/sc/mono2_i16_padding_rle_lossless")
        })
        .expect("registry must contain signed Pixel Padding RLE Lossless SC case");
    assert_eq!(
        signed_padding_case.get("status").and_then(Value::as_str),
        Some("implemented")
    );
    assert_eq!(signed_padding_case.get("skip"), Some(&Value::Null));
    assert_eq!(
        signed_padding_case
            .get("determinism")
            .and_then(Value::as_str),
        Some("byte_stable")
    );

    let mono1_signed_padding_case = cases
        .iter()
        .find(|case| {
            case.get("case_id").and_then(Value::as_str)
                == Some("classic/sc/mono1_i16_padding_rle_lossless")
        })
        .expect("registry must contain MONOCHROME1 signed Pixel Padding RLE Lossless SC case");
    assert_eq!(
        mono1_signed_padding_case
            .get("status")
            .and_then(Value::as_str),
        Some("implemented")
    );
    assert_eq!(mono1_signed_padding_case.get("skip"), Some(&Value::Null));
    assert_eq!(
        mono1_signed_padding_case
            .get("determinism")
            .and_then(Value::as_str),
        Some("byte_stable")
    );

    let mono1_signed_padding_multiframe_case = cases
        .iter()
        .find(|case| {
            case.get("case_id").and_then(Value::as_str)
                == Some("classic/sc/mono1_i16_padding_multiframe_rle_lossless")
        })
        .expect(
            "registry must contain MONOCHROME1 signed multi-frame Pixel Padding RLE Lossless SC case",
        );
    assert_eq!(
        mono1_signed_padding_multiframe_case
            .get("status")
            .and_then(Value::as_str),
        Some("implemented")
    );
    assert_eq!(
        mono1_signed_padding_multiframe_case.get("skip"),
        Some(&Value::Null)
    );
    assert_eq!(
        mono1_signed_padding_multiframe_case
            .get("determinism")
            .and_then(Value::as_str),
        Some("byte_stable")
    );

    let signed_padding_multiframe_case = cases
        .iter()
        .find(|case| {
            case.get("case_id").and_then(Value::as_str)
                == Some("classic/sc/mono2_i16_padding_multiframe_rle_lossless")
        })
        .expect("registry must contain signed multi-frame Pixel Padding RLE Lossless SC case");
    assert_eq!(
        signed_padding_multiframe_case
            .get("status")
            .and_then(Value::as_str),
        Some("implemented")
    );
    assert_eq!(
        signed_padding_multiframe_case.get("skip"),
        Some(&Value::Null)
    );
    assert_eq!(
        signed_padding_multiframe_case
            .get("determinism")
            .and_then(Value::as_str),
        Some("byte_stable")
    );

    let rgb_case = cases
        .iter()
        .find(|case| {
            case.get("case_id").and_then(Value::as_str)
                == Some("classic/sc/rgb_planar0_rle_lossless")
        })
        .expect("registry must contain RGB RLE Lossless SC case");
    assert_eq!(
        rgb_case.get("status").and_then(Value::as_str),
        Some("implemented")
    );
    assert_eq!(rgb_case.get("skip"), Some(&Value::Null));
    assert_eq!(
        rgb_case.get("determinism").and_then(Value::as_str),
        Some("byte_stable")
    );

    let rgb_multiframe_case = cases
        .iter()
        .find(|case| {
            case.get("case_id").and_then(Value::as_str)
                == Some("classic/sc/rgb_planar0_multiframe_rle_lossless")
        })
        .expect("registry must contain RGB multi-frame RLE Lossless SC case");
    assert_eq!(
        rgb_multiframe_case.get("status").and_then(Value::as_str),
        Some("implemented")
    );
    assert_eq!(rgb_multiframe_case.get("skip"), Some(&Value::Null));
    assert_eq!(
        rgb_multiframe_case
            .get("determinism")
            .and_then(Value::as_str),
        Some("byte_stable")
    );

    let rgb_planar1_multiframe_case = cases
        .iter()
        .find(|case| {
            case.get("case_id").and_then(Value::as_str)
                == Some("classic/sc/rgb_planar1_multiframe_rle_lossless")
        })
        .expect("registry must contain RGB planar-1 multi-frame RLE Lossless SC case");
    assert_eq!(
        rgb_planar1_multiframe_case
            .get("status")
            .and_then(Value::as_str),
        Some("implemented")
    );
    assert_eq!(rgb_planar1_multiframe_case.get("skip"), Some(&Value::Null));
    assert_eq!(
        rgb_planar1_multiframe_case
            .get("determinism")
            .and_then(Value::as_str),
        Some("byte_stable")
    );

    let ybr_case = cases
        .iter()
        .find(|case| {
            case.get("case_id").and_then(Value::as_str)
                == Some("classic/sc/ybr_full_planar0_rle_lossless")
        })
        .expect("registry must contain YBR_FULL RLE Lossless SC case");
    assert_eq!(
        ybr_case.get("status").and_then(Value::as_str),
        Some("implemented")
    );
    assert_eq!(ybr_case.get("skip"), Some(&Value::Null));
    assert_eq!(
        ybr_case.get("determinism").and_then(Value::as_str),
        Some("byte_stable")
    );

    let ybr_multiframe_case = cases
        .iter()
        .find(|case| {
            case.get("case_id").and_then(Value::as_str)
                == Some("classic/sc/ybr_full_planar0_multiframe_rle_lossless")
        })
        .expect("registry must contain YBR_FULL multi-frame RLE Lossless SC case");
    assert_eq!(
        ybr_multiframe_case.get("status").and_then(Value::as_str),
        Some("implemented")
    );
    assert_eq!(ybr_multiframe_case.get("skip"), Some(&Value::Null));
    assert_eq!(
        ybr_multiframe_case
            .get("determinism")
            .and_then(Value::as_str),
        Some("byte_stable")
    );

    let ybr_planar1_multiframe_case = cases
        .iter()
        .find(|case| {
            case.get("case_id").and_then(Value::as_str)
                == Some("classic/sc/ybr_full_planar1_multiframe_rle_lossless")
        })
        .expect("registry must contain YBR_FULL planar-1 multi-frame RLE Lossless SC case");
    assert_eq!(
        ybr_planar1_multiframe_case
            .get("status")
            .and_then(Value::as_str),
        Some("implemented")
    );
    assert_eq!(ybr_planar1_multiframe_case.get("skip"), Some(&Value::Null));
    assert_eq!(
        ybr_planar1_multiframe_case
            .get("determinism")
            .and_then(Value::as_str),
        Some("byte_stable")
    );

    let ybr_planar1_case = cases
        .iter()
        .find(|case| {
            case.get("case_id").and_then(Value::as_str)
                == Some("classic/sc/ybr_full_planar1_rle_lossless")
        })
        .expect("registry must contain YBR_FULL planar-1 RLE Lossless SC case");
    assert_eq!(
        ybr_planar1_case.get("status").and_then(Value::as_str),
        Some("implemented")
    );
    assert_eq!(ybr_planar1_case.get("skip"), Some(&Value::Null));
    assert_eq!(
        ybr_planar1_case.get("determinism").and_then(Value::as_str),
        Some("byte_stable")
    );

    let palette_case = cases
        .iter()
        .find(|case| {
            case.get("case_id").and_then(Value::as_str)
                == Some("classic/sc/palette_color_u8_rle_lossless")
        })
        .expect("registry must contain PALETTE COLOR RLE Lossless SC case");
    assert_eq!(
        palette_case.get("status").and_then(Value::as_str),
        Some("implemented")
    );
    assert_eq!(palette_case.get("skip"), Some(&Value::Null));
    assert_eq!(
        palette_case.get("determinism").and_then(Value::as_str),
        Some("byte_stable")
    );

    let palette_multiframe_case = cases
        .iter()
        .find(|case| {
            case.get("case_id").and_then(Value::as_str)
                == Some("classic/sc/palette_color_u8_multiframe_rle_lossless")
        })
        .expect("registry must contain PALETTE COLOR multi-frame RLE Lossless SC case");
    assert_eq!(
        palette_multiframe_case
            .get("status")
            .and_then(Value::as_str),
        Some("implemented")
    );
    assert_eq!(palette_multiframe_case.get("skip"), Some(&Value::Null));
    assert_eq!(
        palette_multiframe_case
            .get("determinism")
            .and_then(Value::as_str),
        Some("byte_stable")
    );

    let multiframe_case = cases
        .iter()
        .find(|case| {
            case.get("case_id").and_then(Value::as_str)
                == Some("classic/sc/mono2_u8_multiframe_rle_lossless")
        })
        .expect("registry must contain multi-frame RLE Lossless SC case");
    assert_eq!(
        multiframe_case.get("status").and_then(Value::as_str),
        Some("implemented")
    );
    assert_eq!(multiframe_case.get("skip"), Some(&Value::Null));
    assert_eq!(
        multiframe_case.get("determinism").and_then(Value::as_str),
        Some("byte_stable")
    );

    let odd_fragment_case = cases
        .iter()
        .find(|case| {
            case.get("case_id").and_then(Value::as_str)
                == Some("classic/sc/mono2_u8_odd_fragment_rle_lossless")
        })
        .expect("registry must contain odd-fragment RLE Lossless SC case");
    assert_eq!(
        odd_fragment_case.get("status").and_then(Value::as_str),
        Some("implemented")
    );
    assert_eq!(odd_fragment_case.get("skip"), Some(&Value::Null));
    assert_eq!(
        odd_fragment_case.get("determinism").and_then(Value::as_str),
        Some("byte_stable")
    );

    let mono1_odd_fragment_case = cases
        .iter()
        .find(|case| {
            case.get("case_id").and_then(Value::as_str)
                == Some("classic/sc/mono1_u8_odd_fragment_rle_lossless")
        })
        .expect("registry must contain MONOCHROME1 odd-fragment RLE Lossless SC case");
    assert_eq!(
        mono1_odd_fragment_case
            .get("status")
            .and_then(Value::as_str),
        Some("implemented")
    );
    assert_eq!(mono1_odd_fragment_case.get("skip"), Some(&Value::Null));
    assert_eq!(
        mono1_odd_fragment_case
            .get("determinism")
            .and_then(Value::as_str),
        Some("byte_stable")
    );
    assert_eq!(
        mono1_odd_fragment_case
            .get("standards_evidence")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(5)
    );

    let ct_case = cases
        .iter()
        .find(|case| {
            case.get("case_id").and_then(Value::as_str)
                == Some("classic/ct/mono2_i16_rescale_12bit_rle_lossless")
        })
        .expect("registry must contain CT RLE Lossless case");
    assert_eq!(
        ct_case.get("status").and_then(Value::as_str),
        Some("implemented")
    );
    assert_eq!(ct_case.get("skip"), Some(&Value::Null));
    assert_eq!(
        ct_case.get("determinism").and_then(Value::as_str),
        Some("byte_stable")
    );
    assert_eq!(
        ct_case.get("sop_class_uid").and_then(Value::as_str),
        Some("1.2.840.10008.5.1.4.1.1.2")
    );

    let mr_case = cases
        .iter()
        .find(|case| {
            case.get("case_id").and_then(Value::as_str) == Some("classic/mr/mono2_u16_rle_lossless")
        })
        .expect("registry must contain MR RLE Lossless case");
    assert_eq!(
        mr_case.get("status").and_then(Value::as_str),
        Some("implemented")
    );
    assert_eq!(mr_case.get("skip"), Some(&Value::Null));
    assert_eq!(
        mr_case.get("determinism").and_then(Value::as_str),
        Some("byte_stable")
    );
    assert_eq!(
        mr_case.get("sop_class_uid").and_then(Value::as_str),
        Some("1.2.840.10008.5.1.4.1.1.4")
    );

    let cr_case = cases
        .iter()
        .find(|case| {
            case.get("case_id").and_then(Value::as_str)
                == Some("classic/cr/overlay_modality_voi_rle_lossless")
        })
        .expect("registry must contain CR RLE Lossless case");
    assert_eq!(
        cr_case.get("status").and_then(Value::as_str),
        Some("implemented")
    );
    assert_eq!(cr_case.get("skip"), Some(&Value::Null));
    assert_eq!(
        cr_case.get("determinism").and_then(Value::as_str),
        Some("byte_stable")
    );
    assert_eq!(
        cr_case.get("sop_class_uid").and_then(Value::as_str),
        Some("1.2.840.10008.5.1.4.1.1.1")
    );

    let dx_case = cases
        .iter()
        .find(|case| {
            case.get("case_id").and_then(Value::as_str)
                == Some("classic/dx/display_shutter_mono2_u16_rle_lossless")
        })
        .expect("registry must contain DX RLE Lossless case");
    assert_eq!(
        dx_case.get("status").and_then(Value::as_str),
        Some("implemented")
    );
    assert_eq!(dx_case.get("skip"), Some(&Value::Null));
    assert_eq!(
        dx_case.get("determinism").and_then(Value::as_str),
        Some("byte_stable")
    );
    assert_eq!(
        dx_case.get("sop_class_uid").and_then(Value::as_str),
        Some("1.2.840.10008.5.1.4.1.1.1.1")
    );

    let mg_case = cases
        .iter()
        .find(|case| {
            case.get("case_id").and_then(Value::as_str)
                == Some("classic/mg/for_presentation_mono1_u16_12bit_rle_lossless")
        })
        .expect("registry must contain MG RLE Lossless case");
    assert_eq!(
        mg_case.get("status").and_then(Value::as_str),
        Some("implemented")
    );
    assert_eq!(mg_case.get("skip"), Some(&Value::Null));
    assert_eq!(
        mg_case.get("determinism").and_then(Value::as_str),
        Some("byte_stable")
    );
    assert_eq!(
        mg_case.get("sop_class_uid").and_then(Value::as_str),
        Some("1.2.840.10008.5.1.4.1.1.1.2")
    );

    let mg_processing_case = cases
        .iter()
        .find(|case| {
            case.get("case_id").and_then(Value::as_str)
                == Some("classic/mg/for_processing_mono2_u16_12bit_rle_lossless")
        })
        .expect("registry must contain MG For Processing RLE Lossless case");
    assert_eq!(
        mg_processing_case.get("status").and_then(Value::as_str),
        Some("implemented")
    );
    assert_eq!(mg_processing_case.get("skip"), Some(&Value::Null));
    assert_eq!(
        mg_processing_case
            .get("determinism")
            .and_then(Value::as_str),
        Some("byte_stable")
    );
    assert_eq!(
        mg_processing_case
            .get("sop_class_uid")
            .and_then(Value::as_str),
        Some("1.2.840.10008.5.1.4.1.1.1.2.1")
    );

    let us_case = cases
        .iter()
        .find(|case| {
            case.get("case_id").and_then(Value::as_str) == Some("classic/us/mono2_u8_rle_lossless")
        })
        .expect("registry must contain US RLE Lossless case");
    assert_eq!(
        us_case.get("status").and_then(Value::as_str),
        Some("implemented")
    );
    assert_eq!(us_case.get("skip"), Some(&Value::Null));
    assert_eq!(
        us_case.get("determinism").and_then(Value::as_str),
        Some("byte_stable")
    );
    assert_eq!(
        us_case.get("sop_class_uid").and_then(Value::as_str),
        Some("1.2.840.10008.5.1.4.1.1.6.1")
    );

    let vl_photo_case = cases
        .iter()
        .find(|case| {
            case.get("case_id").and_then(Value::as_str) == Some("vl/photo/rgb_planar0_rle_lossless")
        })
        .expect("registry must contain VL Photographic RGB RLE Lossless case");
    assert_eq!(
        vl_photo_case.get("status").and_then(Value::as_str),
        Some("implemented")
    );
    assert_eq!(vl_photo_case.get("skip"), Some(&Value::Null));
    assert_eq!(
        vl_photo_case.get("determinism").and_then(Value::as_str),
        Some("byte_stable")
    );
    assert_eq!(
        vl_photo_case.get("sop_class_uid").and_then(Value::as_str),
        Some("1.2.840.10008.5.1.4.1.1.77.1.4")
    );

    let vl_photo_planar1_case = cases
        .iter()
        .find(|case| {
            case.get("case_id").and_then(Value::as_str) == Some("vl/photo/rgb_planar1_rle_lossless")
        })
        .expect("registry must contain VL Photographic RGB planar-1 RLE Lossless case");
    assert_eq!(
        vl_photo_planar1_case.get("status").and_then(Value::as_str),
        Some("implemented")
    );
    assert_eq!(vl_photo_planar1_case.get("skip"), Some(&Value::Null));
    assert_eq!(
        vl_photo_planar1_case
            .get("determinism")
            .and_then(Value::as_str),
        Some("byte_stable")
    );
    assert_eq!(
        vl_photo_planar1_case
            .get("sop_class_uid")
            .and_then(Value::as_str),
        Some("1.2.840.10008.5.1.4.1.1.77.1.4")
    );

    let vl_photo_palette_case = cases
        .iter()
        .find(|case| {
            case.get("case_id").and_then(Value::as_str)
                == Some("vl/photo/palette_color_rle_lossless")
        })
        .expect("registry must contain VL Photographic PALETTE COLOR RLE Lossless case");
    assert_eq!(
        vl_photo_palette_case.get("status").and_then(Value::as_str),
        Some("implemented")
    );
    assert_eq!(vl_photo_palette_case.get("skip"), Some(&Value::Null));
    assert_eq!(
        vl_photo_palette_case
            .get("determinism")
            .and_then(Value::as_str),
        Some("byte_stable")
    );
    assert_eq!(
        vl_photo_palette_case
            .get("sop_class_uid")
            .and_then(Value::as_str),
        Some("1.2.840.10008.5.1.4.1.1.77.1.4")
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
        "1cc11d28abf1e6f4efa4b07a73d4a7c953b3b3101b4112865c7170ccdeb84728",
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

#[test]
fn xa_monoplane_registry_evidence_resolves_to_a_source_note() {
    let registry = read_json("cases/registry.json");
    let cases = registry_cases(&registry);
    let case = cases
        .iter()
        .find(|case| {
            case.get("case_id").and_then(Value::as_str) == Some("classic/xa/monoplane_explicit_le")
        })
        .expect("registry must contain the XA monoplane case");

    assert_eq!(
        case.get("status").and_then(Value::as_str),
        Some("implemented")
    );
    assert!(
        case.get("blockers")
            .and_then(Value::as_array)
            .is_some_and(Vec::is_empty),
        "promoted XA monoplane coverage must not retain controlled blockers"
    );
    let source_note = case
        .get("standards_evidence")
        .and_then(Value::as_array)
        .and_then(|evidence| {
            evidence.iter().find_map(|entry| {
                (entry.get("source").and_then(Value::as_str) == Some("local-source-note"))
                    .then(|| entry.get("query").and_then(Value::as_str))
                    .flatten()
            })
        })
        .expect("XA monoplane standards evidence must name its local source note");
    assert_eq!(
        source_note,
        "standards/source-notes/phase-2-xa-monoplane.md"
    );
    assert!(
        std::path::Path::new(source_note).is_file(),
        "XA monoplane local source-note evidence must resolve to a tracked artifact"
    );
}

#[test]
fn xrf_monoplane_registry_evidence_resolves_to_a_source_note() {
    let registry = read_json("cases/registry.json");
    let cases = registry_cases(&registry);
    let case = cases
        .iter()
        .find(|case| {
            case.get("case_id").and_then(Value::as_str) == Some("classic/xrf/monoplane_explicit_le")
        })
        .expect("registry must contain the XRF monoplane case");

    assert_eq!(
        case.get("status").and_then(Value::as_str),
        Some("implemented")
    );
    assert!(
        case.get("blockers")
            .and_then(Value::as_array)
            .is_some_and(Vec::is_empty),
        "promoted XRF monoplane coverage must not retain controlled blockers"
    );
    let source_note = case
        .get("standards_evidence")
        .and_then(Value::as_array)
        .and_then(|evidence| {
            evidence.iter().find_map(|entry| {
                (entry.get("source").and_then(Value::as_str) == Some("local-source-note"))
                    .then(|| entry.get("query").and_then(Value::as_str))
                    .flatten()
            })
        })
        .expect("XRF monoplane standards evidence must name its local source note");
    assert_eq!(
        source_note,
        "standards/source-notes/phase-2-xrf-monoplane.md"
    );
    assert!(
        std::path::Path::new(source_note).is_file(),
        "XRF monoplane local source-note evidence must resolve to a tracked artifact"
    );
}

#[test]
fn enhanced_pet_registry_evidence_resolves_to_a_source_note() {
    let registry = read_json("cases/registry.json");
    let cases = registry_cases(&registry);
    let case = cases
        .iter()
        .find(|case| {
            case.get("case_id").and_then(Value::as_str)
                == Some("enhanced/pet/multiframe_explicit_le")
        })
        .expect("registry must contain the Enhanced PET case");

    assert_eq!(
        case.get("status").and_then(Value::as_str),
        Some("implemented")
    );
    assert!(
        case.get("blockers")
            .and_then(Value::as_array)
            .is_some_and(Vec::is_empty),
        "promoted Enhanced PET coverage must not retain controlled blockers"
    );
    let source_note = case
        .get("standards_evidence")
        .and_then(Value::as_array)
        .and_then(|evidence| {
            evidence.iter().find_map(|entry| {
                (entry.get("source").and_then(Value::as_str) == Some("local-source-note"))
                    .then(|| entry.get("query").and_then(Value::as_str))
                    .flatten()
            })
        })
        .expect("Enhanced PET standards evidence must name its local source note");
    assert_eq!(
        source_note,
        "standards/source-notes/phase-2-enhanced-pet-multiframe.md"
    );
    assert!(
        std::path::Path::new(source_note).is_file(),
        "Enhanced PET local source-note evidence must resolve to a tracked artifact"
    );
}

#[test]
fn nonsquare_registry_evidence_resolves_to_a_source_note() {
    let registry = read_json("cases/registry.json");
    let cases = registry_cases(&registry);
    let case = cases
        .iter()
        .find(|case| {
            case.get("case_id").and_then(Value::as_str)
                == Some("classic/sc/nonsquare_pixel_spacing")
        })
        .expect("registry must contain the non-square spatial case");

    assert_eq!(
        case.get("status").and_then(Value::as_str),
        Some("implemented")
    );
    assert!(
        case.get("blockers")
            .and_then(Value::as_array)
            .is_some_and(Vec::is_empty),
        "promoted non-square spatial coverage must not retain controlled blockers"
    );
    let evidence = case
        .get("standards_evidence")
        .and_then(Value::as_array)
        .expect("non-square spatial coverage must carry standards evidence");
    assert!(
        evidence
            .iter()
            .filter(|entry| {
                entry.get("source_manifest_sha256").and_then(Value::as_str)
                    == Some("1cc11d28abf1e6f4efa4b07a73d4a7c953b3b3101b4112865c7170ccdeb84728")
            })
            .count()
            >= 4,
        "non-square spatial evidence must lock the 2026b KB source manifest"
    );
    let source_note = evidence
        .iter()
        .find_map(|entry| {
            (entry.get("source").and_then(Value::as_str) == Some("local-source-note"))
                .then(|| entry.get("query").and_then(Value::as_str))
                .flatten()
        })
        .expect("non-square spatial evidence must name its local source note");
    assert_eq!(
        source_note,
        "standards/source-notes/phase-2-nonsquare-spacing-aspect-ratio.md"
    );
    assert!(
        std::path::Path::new(source_note).is_file(),
        "non-square spatial local source-note evidence must resolve to a tracked artifact"
    );
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
    let mut case_ids = BTreeSet::new();
    for (path, prefixes) in [
        ("src/generator.rs", &["case_id: \""][..]),
        ("src/generator/native/ct_geometry.rs", &["case_id: \""][..]),
        (
            "src/generator/native/empty_type2_sc.rs",
            &["case_id: \""][..],
        ),
        (
            "src/generator/native/icc_profile.rs",
            &["ICC_CASE_ID: &str = \""][..],
        ),
        ("src/generator/native/metadata_sc.rs", &["case_id: \""][..]),
        ("src/generator/native/nm.rs", &["case_id: \""][..]),
        ("src/generator/native/pet.rs", &["case_id: \""][..]),
        (
            "src/generator/native/us_multiframe.rs",
            &["case_id: \""][..],
        ),
        ("src/generator/native/xa.rs", &["case_id: \""][..]),
        ("src/generator/native/xrf.rs", &["case_id: \""][..]),
        (
            "src/generator/native/private_creator_sc.rs",
            &["case_id: \""][..],
        ),
        (
            "src/generator/native/sc_integer_pixels.rs",
            &["case_id: \""][..],
        ),
        (
            "src/generator/native/sc_nonsquare_spacing.rs",
            &["case_id: \""][..],
        ),
        (
            "src/generator/native/sequence_length_sc.rs",
            &["case_id: \""][..],
        ),
        (
            "src/generator/native/string_boundary_sc.rs",
            &["case_id: \""][..],
        ),
        ("src/generator/native/timezone_sc.rs", &["case_id: \""][..]),
        (
            "src/generation_backends/parametric_map.rs",
            &["CASE_ID: &str = \""][..],
        ),
    ] {
        let source = fs::read_to_string(path).expect("generator source must be readable");
        for prefix in prefixes {
            let mut remaining = source.as_str();
            while let Some(start) = remaining.find(prefix) {
                remaining = &remaining[start + prefix.len()..];
                let Some(end) = remaining.find('"') else {
                    break;
                };
                let case_id = &remaining[..end];
                if is_suite_case_id(case_id) {
                    case_ids.insert(case_id.to_string());
                }
                remaining = &remaining[end + 1..];
            }
        }
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
        Some("classic" | "enhanced" | "derived" | "geometry" | "metadata" | "non-image" | "vl")
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
