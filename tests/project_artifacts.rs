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
        "2813c20e61cd625955429a999de42c52c9b1fec25f3e2a3b168dc0b41b46b72c"
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
fn registration_secondary_iod_validator_is_additive_and_locked() {
    let validators = read_json("conformance/validators.json");
    let adapter = validators["adapters"]
        .as_array()
        .unwrap()
        .iter()
        .find(|adapter| adapter["id"] == "pydicom-dicom-validator-registration")
        .expect("registration secondary IOD validator must be configured");
    assert_eq!(adapter["role"], "secondary_iod_validator");
    assert_eq!(adapter["required"], false);
    assert_eq!(
        adapter["supported_case_ids"],
        serde_json::json!([
            "derived/registration/spatial_ct_pair",
            "derived/registration/deformable_ct_pair"
        ])
    );

    let primary = validators["adapters"]
        .as_array()
        .unwrap()
        .iter()
        .find(|adapter| adapter["id"] == "dicom3tools-dciodvfy")
        .unwrap();
    assert_eq!(primary["role"], "primary_iod_validator");
    assert!(primary.get("supported_case_ids").is_none());

    let lock = read_json("conformance/validator-lock.json");
    let tool = lock["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|tool| tool["adapter_id"] == "pydicom-dicom-validator-registration")
        .expect("registration secondary validator must have an accepted lock entry");
    assert_eq!(tool["role"], "secondary_iod_validator");
    assert_eq!(
        tool["adapter_sha256"],
        "2813c20e61cd625955429a999de42c52c9b1fec25f3e2a3b168dc0b41b46b72c"
    );

    let readme = fs::read_to_string("conformance/README.md").unwrap();
    for required in [
        "Secondary IOD routing is additive",
        "did not reject a VM 15 transformation matrix",
        "cannot replace `dciodvfy`",
        "no finding is allowlisted",
    ] {
        assert!(
            readme.contains(required),
            "registration route requires {required}"
        );
    }
}

#[test]
fn presentation_state_secondary_iod_validator_is_additive_and_locked() {
    let validators = read_json("conformance/validators.json");
    let adapter = validators["adapters"]
        .as_array()
        .unwrap()
        .iter()
        .find(|adapter| adapter["id"] == "pydicom-dicom-validator-presentation-state")
        .expect("presentation-state secondary IOD validator must be configured");
    assert_eq!(adapter["role"], "secondary_iod_validator");
    assert_eq!(adapter["required"], false);
    assert_eq!(
        adapter["supported_case_ids"],
        serde_json::json!([
            "derived/presentation-state/color_softcopy",
            "derived/presentation-state/advanced_blending",
            "derived/presentation-state/blending"
        ])
    );

    let lock = read_json("conformance/validator-lock.json");
    let tool = lock["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|tool| tool["adapter_id"] == "pydicom-dicom-validator-presentation-state")
        .expect("presentation-state secondary validator must have an accepted lock entry");
    assert_eq!(tool["role"], "secondary_iod_validator");
    assert_eq!(
        tool["adapter_sha256"],
        "2813c20e61cd625955429a999de42c52c9b1fec25f3e2a3b168dc0b41b46b72c"
    );

    let readme = fs::read_to_string("conformance/README.md").unwrap();
    for required in [
        "pydicom-dicom-validator-presentation-state",
        "Content Label was removed",
        "dangling referenced SOP Instance UID did not alter",
        "missed absent conditional palette LUT data",
        "no finding is allowlisted",
    ] {
        assert!(
            readme.contains(required),
            "presentation-state route requires {required}"
        );
    }
}

#[test]
fn linked_rt_secondary_iod_validator_is_additive_and_locked() {
    let validators = read_json("conformance/validators.json");
    let adapter = validators["adapters"]
        .as_array()
        .unwrap()
        .iter()
        .find(|adapter| adapter["id"] == "pydicom-dicom-validator-rt")
        .expect("linked RT secondary IOD validator must be configured");
    assert_eq!(adapter["role"], "secondary_iod_validator");
    assert_eq!(adapter["required"], false);
    assert_eq!(adapter["executable_env"], "DTS_DICOM_VALIDATOR_PYTHON");
    assert_eq!(
        adapter["supported_case_ids"],
        serde_json::json!(["non-image/rt/plan_linked", "non-image/rt/image_linked"])
    );
    assert_eq!(adapter["artifacts"].as_array().unwrap().len(), 14);
    for capability in ["rt_plan_iod_validation", "rt_image_iod_validation"] {
        assert!(
            adapter["capabilities"]
                .as_array()
                .unwrap()
                .iter()
                .any(|value| value == capability),
            "linked RT adapter requires {capability}"
        );
    }

    let lock = read_json("conformance/validator-lock.json");
    let tool = lock["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|tool| tool["adapter_id"] == "pydicom-dicom-validator-rt")
        .expect("linked RT secondary IOD validator must have an accepted lock entry");
    assert_eq!(tool["role"], "secondary_iod_validator");
    assert_eq!(
        tool["version"],
        "dicom-validator 0.8.2; adapter 0.7.0; CPython 3.12.12"
    );
    assert_eq!(
        tool["adapter_sha256"],
        "2813c20e61cd625955429a999de42c52c9b1fec25f3e2a3b168dc0b41b46b72c"
    );
    let shared = lock["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|tool| tool["adapter_id"] == "pydicom-dicom-validator-waveform")
        .unwrap();
    assert_eq!(tool["supporting_artifacts"], shared["supporting_artifacts"]);
    assert_eq!(tool["package_identity"], shared["package_identity"]);
    let notes = tool["notes"].as_str().expect("linked RT lock notes");
    for required in [
        "e9337a6c46fe85b56f1f563120dd3caf56ea1335355792db42386db959be6db2",
        "Study ID DTS-RTSTRUCT",
        "Of 20 Plan mutations",
        "dciodvfy alone caught wrong control-point count",
        "dcentvfy alone added missing-SOP evidence",
        "Strict Rust owns the other 15 misses",
        "460d525ab06aaf74df963029f3ab39c2536e4e1c5bf4b75fcf16b500382db20c",
        "Across 20 Image mutations dciodvfy detected 10",
        "the pixel route detected 6",
        "rejecting a stale source digest",
        "Dose DTS-RTDOSE and enhanced CT DTS-ECT",
        "no additive missing/dangling reference finding",
        "d0d78ffccf44218a27944cf1b80dec63c8afa7162b0e085532feb51706a04714",
        "qualification itself made no registry status change",
    ] {
        assert!(
            notes.contains(required),
            "linked RT lock notes require {required}"
        );
    }

    let readme = fs::read_to_string("conformance/README.md").unwrap();
    for required in [
        "pydicom-dicom-validator-rt",
        "non-image/rt/plan_linked",
        "non-image/rt/image_linked",
        "exact SOP Class UIDs selected the locked 2026b `RT Plan IOD` and\n`RT Image IOD`",
        "e9337a6c46fe85b56f1f563120dd3caf56ea1335355792db42386db959be6db2",
        "Study ID `DTS-RTSTRUCT`",
        "460d525ab06aaf74df963029f3ab39c2536e4e1c5bf4b75fcf16b500382db20c",
        "a8faed6abbf35c12a4b26e40f6feb19d736d90045c83b9f9a31f638d323e6811",
        "omission of the whole RT Beams Module",
        "Across all 20 locked Plan mutations",
        "`dciodvfy` alone detected a one-item Control Point Sequence",
        "Isolated `dcentvfy` alone added a missing-referenced-SOP finding",
        "Both IOD validators missed the wrong Structure SOP\nClass",
        "Strict Rust owns every semantic miss",
        "Dose `DTS-RTDOSE` versus Plan/Structure\n`DTS-RTSTRUCT`",
        "enhanced CT `DTS-ECT` versus Plan/Structure\n`DTS-RTSTRUCT`",
        "no additive missing or dangling reference finding",
        "does not mean a silent `dcentvfy` run or a\nzero exit code",
        "Across all 20 locked Image mutations",
        "the uv-locked\nIOD adapter detected 6",
        "Strict Rust owns wrong beam and\nfraction linkage",
        "reopening the generated Plan",
        "d0d78ffccf44218a27944cf1b80dec63c8afa7162b0e085532feb51706a04714",
        "does not\npromote the planned RT Image registry row",
        "No linked RT finding is allowlisted",
    ] {
        assert!(
            readme.contains(required),
            "linked RT route requires {required}"
        );
    }
}

#[test]
fn second_generation_rt_primary_iod_validator_is_exact_case_locked() {
    let validators = read_json("conformance/validators.json");
    let adapter = validators["adapters"]
        .as_array()
        .unwrap()
        .iter()
        .find(|adapter| adapter["id"] == "pydicom-dicom-validator-rt-radiation")
        .expect("second-generation RT primary validator must be configured");
    assert_eq!(adapter["role"], "primary_iod_validator");
    assert_eq!(adapter["required"], false);
    assert_eq!(adapter["executable_env"], "DTS_DICOM_VALIDATOR_PYTHON");
    assert_eq!(
        adapter["supported_case_ids"],
        serde_json::json!([
            "non-image/rt/carm_photon_electron_radiation_minimal",
            "non-image/rt/radiation_set_minimal"
        ])
    );
    assert_eq!(adapter["artifacts"].as_array().unwrap().len(), 14);
    for capability in [
        "carm_photon_electron_radiation_iod_validation",
        "rt_radiation_set_iod_validation",
        "guarded_rt_record_condition_correction",
        "guarded_rt_device_not_empty_condition_correction",
    ] {
        assert!(
            adapter["capabilities"]
                .as_array()
                .unwrap()
                .iter()
                .any(|value| value == capability),
            "second-generation RT adapter requires {capability}"
        );
    }

    let lock = read_json("conformance/validator-lock.json");
    let tool = lock["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|tool| tool["adapter_id"] == "pydicom-dicom-validator-rt-radiation")
        .expect("second-generation RT validator must have a lock entry");
    assert_eq!(tool["role"], "primary_iod_validator");
    assert_eq!(
        tool["version"],
        "dicom-validator 0.8.2; adapter 0.7.0; CPython 3.12.12"
    );
    assert_eq!(
        tool["adapter_sha256"],
        "2813c20e61cd625955429a999de42c52c9b1fec25f3e2a3b168dc0b41b46b72c"
    );
    let shared = lock["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|tool| tool["adapter_id"] == "pydicom-dicom-validator-waveform")
        .unwrap();
    assert_eq!(tool["supporting_artifacts"], shared["supporting_artifacts"]);
    assert_eq!(tool["package_identity"], shared["package_identity"]);
    let notes = tool["notes"].as_str().expect("RT Radiation lock notes");
    for required in [
        "do not recognize these current IODs",
        "missing other_cond branch",
        "has-a-Value semantics",
        "empty Device Alternate Identifier",
        "either companion missing",
        "Patient Orientation Macro scope",
        "fail-closed definition drift",
        "NO with recorded content absent",
        "YES/absent mutations",
        "No finding is allowlisted",
        "no registry status change",
    ] {
        assert!(
            notes.contains(required),
            "RT Radiation route requires {required}"
        );
    }
}

#[test]
fn waveform_secondary_iod_and_payload_validator_is_additive_and_locked() {
    let validators = read_json("conformance/validators.json");
    let adapter = validators["adapters"]
        .as_array()
        .unwrap()
        .iter()
        .find(|adapter| adapter["id"] == "pydicom-dicom-validator-waveform")
        .expect("waveform validator must be configured");
    assert_eq!(adapter["role"], "secondary_iod_validator");
    assert_eq!(adapter["required"], false);
    assert_eq!(
        adapter["supported_case_ids"],
        serde_json::json!([
            "non-image/waveform/twelve_lead_ecg",
            "non-image/waveform/general_ecg"
        ])
    );
    assert!(
        adapter["waveform_arguments"]
            .as_array()
            .unwrap()
            .iter()
            .any(|argument| argument == "--waveform")
    );
    for capability in [
        "general_ecg_iod_validation",
        "ordered_multi_group_waveform_validation",
        "raw_waveform_payload_validation",
    ] {
        assert!(
            adapter["capabilities"]
                .as_array()
                .unwrap()
                .iter()
                .any(|value| value == capability),
            "waveform adapter requires {capability}"
        );
    }

    let lock = read_json("conformance/validator-lock.json");
    let tool = lock["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|tool| tool["adapter_id"] == "pydicom-dicom-validator-waveform")
        .expect("waveform validator must have an accepted lock entry");
    assert_eq!(tool["role"], "secondary_iod_validator");
    assert_eq!(
        tool["version"],
        "dicom-validator 0.8.2; adapter 0.7.0; CPython 3.12.12"
    );
    assert_eq!(
        tool["adapter_sha256"],
        "2813c20e61cd625955429a999de42c52c9b1fec25f3e2a3b168dc0b41b46b72c"
    );
    assert_eq!(
        tool["supporting_artifacts"]["pyproject.toml"],
        "84a3860fe240736fcb7b82258f5b327e397c79514dd9770361bb6cbc39fae640"
    );
    assert_eq!(
        tool["supporting_artifacts"]["uv.lock"],
        "988c01b0da2b433a4a26cb566cbbcfb4f18b31099ddd679520119c47309afdc0"
    );
    assert_eq!(
        tool["supporting_artifacts"]["adapter/__main__.py"],
        "66899beb38d34ac1cbb97b9a587c77f4b385aaac14ef2e6b421d3c1a1ba582af"
    );
    for shared_adapter in [
        "pydicom-dicom-validator-u32",
        "pydicom-dicom-validator-registration",
        "pydicom-dicom-validator-presentation-state",
        "pydicom-dicom-validator-rt",
        "pydicom-dicom-validator-rt-radiation",
        "pydicom-dicom-validator-waveform",
    ] {
        let shared = lock["tools"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["adapter_id"] == shared_adapter)
            .unwrap();
        assert_eq!(shared["version"], tool["version"], "{shared_adapter}");
        assert_eq!(
            shared["adapter_sha256"], tool["adapter_sha256"],
            "{shared_adapter}"
        );
        assert_eq!(
            shared["supporting_artifacts"], tool["supporting_artifacts"],
            "{shared_adapter}"
        );
    }

    let readme = fs::read_to_string("conformance/README.md").unwrap();
    for required in [
        "pydicom-dicom-validator-waveform",
        "non-image/waveform/general_ecg",
        "a656720538672c95aacdf068ba89b0c6d6f78042610f3a665d55065d0a4ab40c",
        "c450f55360d6c07394600e4c0f71f951565cd0e1699edfbbb52f660221c6abea",
        "five groups, 25 channels",
        "199 or\n1,001 Hz",
        "The group-aware raw route rejected every one of those mutations",
        "channel-then-sample interleave",
        "no\nwaveform finding is allowlisted",
    ] {
        assert!(
            readme.contains(required),
            "waveform route requires {required}"
        );
    }
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
        "Adapter version 0.4.0",
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
        "Independent Validator Qualification",
        "dicom-validator` 0.8.2",
        "DCMTK `dcmdump`",
        "Registry status: implemented",
        "Should become KB patch: yes",
    ] {
        assert!(
            source.contains(required),
            "float64 Parametric Map source note requires {required}"
        );
    }

    let status = fs::read_to_string("docs/phase-3-complex-object-status.md")
        .expect("Phase 3 status note must be readable");
    for required in [
        "provider_capability_unavailable",
        "1f50196e425771c51284f03893826e7dcb7910b4529190445151e26677358d21",
        "dciodvfy -new",
        "DCMTK",
        "nine Parametric Map functional-group macro gaps",
        "TID 1500 Measurement Report",
    ] {
        assert!(
            status.contains(required),
            "Phase 3 status requires {required}"
        );
    }
}

#[test]
fn phase3_status_records_completed_derived_vertical_gates() {
    let status = fs::read_to_string("docs/phase-3-derived-status.md")
        .expect("Phase 3 derived status must be readable");
    for required in [
        "derived/sr/tid1500_ct_measurement_report",
        "DCMR TID 1500",
        "TID 1411",
        "5.625 mm3",
        "89 files",
        "defa75675e4c28e369323d22b1ed3e0dc427caa8034ff549c76c539a74f4e0e0",
        "PixelMed 20260608",
        "no findings",
        "derived/sr/comprehensive3d_scoord3d",
        "TID 1501",
        "2.5 mm",
        "90 files",
        "b13ec046baf600f1b47a918b80dc450b86e1f6eb7d79a7cbe274b48935c86379",
        "2601144c7df81cc9b5999b67c707ed747b66e2b76e35c2e55e76216ed70f95d1",
        "68bc95709add383d0f6cb06c2607e29046c22b83c56354bf6a6897abc2d87f32",
        "195 unresolved older failures",
        "derived/registration/spatial_ct_pair",
        "92 files",
        "8b3b8498c3e90dc13e52cceb9c584fbb41d5898e28c2f3d3f86baf4a1654ac8",
        "522f2627658dd11ae6e5b88ad5e673659cacfdb2abf45fe4cb43adfb90feb7ea",
        "dicom-validator` 0.8.2",
        "207 older or unrelated findings",
        "derived/registration/deformable_ct_pair",
        "93 files",
        "d8c539ad4ac9e72a8a597f9bf8a6588feac4d110d97464a70f6d543a033e5114",
        "225bef48a5503e4ed2adc88490d9f28d9f8c314e0bc34d3fa8bff0d144b4127e",
        "208 older or unrelated findings",
        "derived/presentation-state/color_softcopy",
        "95 files",
        "99832aaabe9ca4e36e4c108db44974de352b113ca1ccf0e4a41df74e88ced62a",
        "4e737e1429b7b2463bc412e4c6ff330411259f321070b32d9ce68cdef0bc0543",
        "b1e494962d40634300fb488fdf95c92ad80bad9b2d1e0f0be6bff9b4e8503b0a",
        "213 older or unrelated findings",
        "derived/presentation-state/advanced_blending",
        "100 files",
        "52ae3faf72563b66069cb9546396e9d291ae324ec7302012f2eaadf3c491786a",
        "4bf58b3a29f168c6d24398603f98ebaa5b40ee62353eb30449e3c193b84ad75d",
        "c6a017c46b7e489059dd3bc71b1be66e1ff70008af853aaf393880a4e4f69c73",
        "211 older or unrelated failures",
        "derived/presentation-state/blending",
        "101 files",
        "0e5a934186cdba5667b4cef14ad7475d0d222f8e0286b8a49a29bb3106b5a200",
        "d6fd50ea537157dea62e878e6c455d69f8bb239ce7456c3d7bb5a2893f159918",
        "5df5c921ae704341109f1c095258b0f99ebf856e0b91a2eb60deab6531a4a1e3",
        "Blending adds no external finding",
        "non-image/waveform/twelve_lead_ecg",
        "102 files",
        "898ccec3c6c8e09f91ddcc255a45e397ca19ae69c32b41c1aec4aa5240a9ba3d",
        "1a14c3f7097e8c7482deb6c5c228b9dd33dbbc97206a3c3f865d3118d713e4c6",
        "09391f4644f6ad827a2a635ccc0df6d74201e5d6cc45ee8b2d2144d9c0d8e232",
        "b28021744fc73da06f3b1c4af979eb2c61084102558ba0e6c3831bc77f705ce6",
        "6e0c8f5880ccf65ba78f031b4687c6ea33ca62560e883e78e487935b6c795faf",
        "211 older or unrelated failures",
        "accepted_findings",
        "non-image/waveform/general_ecg",
        "12x1000@250Hz; 4x4000@1000Hz",
        "sixteen channels",
        "56,000 bytes",
        "e4bfb8a3290d9057fa5f5935fa6960ce2a44a07f18991d28c190522739008dbb",
        "5b201d4fa7274ba36d6f7387c3d0217e1b5da161a915f983c2b63b995dde7bbe",
        "c450f55360d6c07394600e4c0f71f951565cd0e1699edfbbb52f660221c6abea",
        "103 files",
        "cb2e19a667a302f781e4ce8c1f44041fbb96273acff2debbecbad8160929d301",
        "a656720538672c95aacdf068ba89b0c6d6f78042610f3a665d55065d0a4ab40c",
        "GeneralECG",
        "uv`-locked `dicom-validator` 0.8.2",
        "dcmdump",
        "dcentvfy",
        "16175e687c81729fd428510c26a60c518a7271553afc4a22a5a127f32a47168a",
        "8b262be912c625cc16df43e3935fef2fa1dfbd0d5fea4ba3cb6dba535b6048df",
        "e2613c273b6fe464a6b3308c4ec4a768103af61d0702033d8999e509dc69d23d",
        "565f7db1d5f26cb74256bc9a6d84b6319667d90c7b6a07ef7ddc5be03f929d2c",
        "non-image/rt/plan_linked",
        "non-image/rt/image_linked",
        "105 files",
        "b061e5f654eb426bbab0da9cce0ac945aadcf3cf506182eb6bf33acd3d7a3659",
        "e9337a6c46fe85b56f1f563120dd3caf56ea1335355792db42386db959be6db2",
        "460d525ab06aaf74df963029f3ab39c2536e4e1c5bf4b75fcf16b500382db20c",
        "141 implemented and 41\nplanned logical cases",
        "d0d78ffccf44218a27944cf1b80dec63c8afa7162b0e085532feb51706a04714",
        "milestone-6 decision checkpoint authorized selecting and locking",
        "registered second-generation\nC-Arm Photon-Electron Radiation companion",
        "Both cases are now implemented as byte-stable native",
        "143 implemented and 39 planned logical cases",
    ] {
        assert!(
            status.contains(required),
            "Phase 3 status requires {required}"
        );
    }

    let plan = fs::read_to_string("docs/coverage-expansion-plan.md")
        .expect("coverage expansion plan must be readable");
    for required in [
        "Twelve-lead ECG Waveform Storage is complete",
        "General ECG\nWaveform Storage completes milestone 5",
        "12x1000@250Hz; 4x4000@1000Hz",
        "103\nstrictly valid files",
        "linked\nRT Plan and RT Image are complete",
        "105 strictly valid files",
        "explicit milestone-6 decision checkpoint\nis now authorized",
        "registered C-Arm Photon-Electron Radiation companion",
    ] {
        assert!(plan.contains(required), "coverage plan requires {required}");
    }
}

#[test]
fn phase4_single_frame_vl_qualification_is_recorded() {
    let note = fs::read_to_string("standards/source-notes/phase-4-vl-single-frame.md")
        .expect("Phase 4 single-frame VL source note must be readable");
    for required in [
        "Registry status: implemented",
        "169ed3a7878986cb289420cef935c6f8598467f240c9a8ce88bf960d30fb1958",
        "dc3b2e155c9be0b728412df6fed7432a238a150512b176305fc6104c63bd6a3e",
        "5785f387d79f79e4b168390bb1def6520d165ac7279374b141beb2c2804f41e3",
        "f410d948b8761b9a1f6802f4fce81c2b90355c62214f5f333ac33ffba130b0d3",
        "1c5c2a6477b81f01222d61f30ce7499046a1299522c45c6c5691e3fcfa92159b",
        "145 implemented and 37 planned logical cases",
        "small `TILED_FULL` WSI",
    ] {
        assert!(
            note.contains(required),
            "Phase 4 single-frame VL note requires {required}"
        );
    }

    let plan = fs::read_to_string("docs/coverage-expansion-plan.md")
        .expect("coverage expansion plan must be readable");
    for required in [
        "Milestones 1 and 2 are complete. VL Endoscopic",
        "Two seed-7 extended roots each contain 109\nstrictly valid files",
        "authorized `uv`-locked secondary IOD route",
        "registry now contains 146 implemented and 36 planned cases",
        "the deliberately incomplete `TILED_SPARSE` counterpart, is complete",
    ] {
        assert!(
            plan.contains(required),
            "coverage expansion plan requires {required}"
        );
    }
}

#[test]
fn phase4_wsi_qualification_is_recorded() {
    let status = fs::read_to_string("docs/phase-4-pathology-status.md")
        .expect("Phase 4 pathology status must be readable");
    for required in [
        "`vl/wsi/tiled_full_small` completes milestone 2",
        "Two independent seed-7 extended generations each wrote 110 files",
        "0dc0e975bcacc89a282130e69b2a84620cbe5d5e1eb736d074915781aa6fbe1a",
        "a04f2f5b8e4f8526d1f2b7594427adeab255701087157d49c3db7a9622872f2b",
        "530414e9b8b02637566f085c64234f23ec0cfe4e6f1520383d347ec09bb8c200",
        "zero errors from both locked IOD validators",
        "clean `dcmdump` parsing",
        "6b3f67bfc1aae4609ba7ccc399d78119e326556a64613621403b3b7b7a788716",
        "62d9532d46c3f71b045a1393d95c49c4757ef5e62bb043a61baf4fffed189a2a",
        "isolated from generation",
        "229 unrelated visible and\nunallowlisted failures",
        "146 implemented and 36 planned logical cases",
        "`vl/wsi/tiled_sparse_small` completes milestone 3",
        "456d571b7121bb67ece6593870dc4d6ef103b83c1488ccb74e84627f347186df",
        "84251b2108b6cacb39c18de12c628bc00e0ab3d166310bcf5b82b6291955ceb3",
        "0c347e699e40876d0fdd4ae20e8bbb76ecdb2859a10f596019202a8acefa26b1",
        "a89f55577263f84a27291a6d3adf6659ccebedb76e68dd8b9c06f8b0b3ce7f4e",
        "147 implemented and 35 planned logical cases",
        "Phase 4 milestone 4",
    ] {
        assert!(
            status.contains(required),
            "Phase 4 pathology status requires {required}"
        );
    }

    let sparse_note = fs::read_to_string("standards/source-notes/phase-4-wsi-tiled-sparse.md")
        .expect("Phase 4 tiled-sparse WSI source note must be readable");
    for required in [
        "## Qualification And Promotion Result",
        "111 DICOM files",
        "456d571b7121bb67ece6593870dc4d6ef103b83c1488ccb74e84627f347186df",
        "84251b2108b6cacb39c18de12c628bc00e0ab3d166310bcf5b82b6291955ceb3",
        "dicom-validator 0.8.2 route as passed",
        "unallowlisted `iod_characterization` result",
        "provider `rust_native`",
    ] {
        assert!(
            sparse_note.contains(required),
            "Phase 4 tiled-sparse WSI note requires {required}"
        );
    }
}

#[test]
fn phase4_wsi_pyramid_standards_lock_and_promotion_are_recorded() {
    let note = fs::read_to_string("standards/source-notes/phase-4-wsi-pyramid.md")
        .expect("Phase 4 WSI pyramid source note must be readable");
    for required in [
        "exactly three native DICOM instances",
        "`ORIGINAL\\\\PRIMARY\\\\VOLUME\\\\NONE`",
        "`DERIVED\\\\PRIMARY\\\\THUMBNAIL\\\\RESAMPLED`",
        "`ORIGINAL\\\\PRIMARY\\\\LABEL\\\\NONE`",
        "b40b0afc9b180d5ebfb54a7db428e13fe09a33dcc9a8f76220f395ba2c68d2db",
        "6733cdd08e5c7ef0453e2759ef0d28fbd43ea2aa7883b55422a13dac38e23ecc",
        "ad078f83d3ea66f075867d116c8c126e9c8a8a9dd873cd27280371c173d8ad02",
        "8,794 total DICOM bytes",
        "no more than 65,536 total DICOM bytes",
        "no more than 5 seconds",
        "distinct from\n`stress/wsi/large_pyramid`",
    ] {
        assert!(
            note.contains(required),
            "Phase 4 WSI pyramid note requires {required}"
        );
    }

    let registry = read_json("cases/registry.json");
    let case = registry_cases(&registry)
        .into_iter()
        .find(|case| {
            case.get("case_id").and_then(Value::as_str) == Some("vl/wsi/pyramid_multiresolution")
        })
        .expect("WSI pyramid registry row must exist");
    assert_eq!(case["status"], "implemented");
    assert_eq!(
        case["provider"],
        serde_json::json!({"kind": "rust_native", "id": "rust_native"})
    );
    assert_eq!(case["determinism"], "byte_stable");
    assert_eq!(case["profiles"], serde_json::json!(["stress"]));
    assert_eq!(case["roadmap"], Value::Null);
    assert_eq!(case["blockers"], serde_json::json!([]));
    assert!(case["standards_evidence"].as_array().is_some_and(|items| {
        items.iter().any(|item| {
            item["query"] == "standards/source-notes/phase-4-wsi-pyramid.md"
                && item["covered"] == true
        })
    }));

    let plan = fs::read_to_string("docs/coverage-expansion-plan.md")
        .expect("coverage expansion plan must be readable");
    for required in [
        "registry now contains 148 implemented and 34\nplanned cases",
        "Ordinary `all` remains unchanged because stress cases require\nexplicit selection",
        "Phase 4 milestone 5, multiple optical paths or focal\nplanes, is next",
    ] {
        assert!(
            plan.contains(required),
            "coverage expansion plan requires {required}"
        );
    }
}

#[test]
fn phase4_multiple_optical_path_wsi_is_native_and_standards_locked() {
    let note = fs::read_to_string("standards/source-notes/phase-4-wsi-multiple-optical-paths.md")
        .expect("Phase 4 multiple-optical-path WSI source note must be readable");
    for required in [
        "`BRIGHTFIELD`",
        "`ALTERNATE`",
        "exactly eight Frames",
        "831fe6e50cbc3f3d82e3f57c984d3c273cdb18dd3bd3ab511b3633dc293f708f",
        "62d9532d46c3f71b045a1393d95c49c4757ef5e62bb043a61baf4fffed189a2a",
        "caa1a1abb84ec283bbf92a0f00d5bd89650420d0b1fa911e191ddb368f50e09f",
        "16,384 total DICOM bytes",
        "5 seconds",
        "triggers no explicit decision checkpoint",
        "473e822fe1b82b7217635a980757a1a88f77f3e2448b0e02964122d888a16bf3",
        "a2099a90b53b4ecb9c76f895f02d4f7f62ff8655adcb54d8654e8e80507bea48",
        "c2203223e9d8ce0b716175329769b7f3bb947ac48da44a510843d5a82d8b3dcc",
        "fa838f3b3c398913f2f05e71cad2515cf038fba65dc8f1a30484f88164c48167",
    ] {
        assert!(note.contains(required), "source note requires {required}");
    }

    let registry = read_json("cases/registry.json");
    let case = registry_cases(&registry)
        .into_iter()
        .find(|case| {
            case.get("case_id").and_then(Value::as_str) == Some("vl/wsi/multiple_optical_paths")
        })
        .expect("multiple-optical-path WSI registry row must exist");
    assert_eq!(case["status"], "implemented");
    assert_eq!(
        case["provider"],
        serde_json::json!({"kind": "rust_native", "id": "rust_native"})
    );
    assert_eq!(case["determinism"], "byte_stable");
    assert_eq!(case["profiles"], serde_json::json!(["extended"]));
    assert_eq!(case["roadmap"], Value::Null);
    assert_eq!(case["blockers"], serde_json::json!([]));
    assert!(case["standards_evidence"].as_array().is_some_and(|items| {
        items.iter().any(|item| {
            item["query"] == "standards/source-notes/phase-4-wsi-multiple-optical-paths.md"
                && item["covered"] == true
        })
    }));
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
fn tid1500_source_note_locks_template_measurement_and_validator_gate() {
    let source = fs::read_to_string("standards/source-notes/phase-3-tid1500-measurement-report.md")
        .expect("TID 1500 source note must be readable");
    for required in [
        "derived/sr/tid1500_ct_measurement_report",
        "derived/seg/binary_multiframe_explicit_le",
        "DCMR",
        "Identifier `1500`",
        "TID 1411",
        "5.625",
        "118565006",
        "25045-6",
        "121233",
        "Source image for segmentation",
        "PixelMed 20260608",
        "DicomSRValidator -checktemplateid",
        "Should become KB patch: yes",
    ] {
        assert!(
            source.contains(required),
            "TID 1500 note requires {required}"
        );
    }

    let lock = read_json("standards.lock.json");
    assert!(
        lock["source_artifacts"]
            .as_array()
            .is_some_and(|artifacts| artifacts.iter().any(|artifact| {
                artifact["part"] == "PS3.16" && artifact["status"] == "unavailable_not_downloaded"
            })),
        "standards lock must explicitly account for the reviewed PS3.16 source"
    );
}

#[test]
fn comprehensive3d_scoord3d_source_note_locks_geometry_and_validator_gate() {
    let source = fs::read_to_string("standards/source-notes/phase-3-comprehensive3d-scoord3d.md")
        .expect("Comprehensive 3D SCOORD3D source note must be readable");
    for required in [
        "derived/sr/comprehensive3d_scoord3d",
        "Identifier `1500`",
        "TID 1501",
        "TID 300",
        "POLYLINE",
        "[0.0, 0.0, 0.0, 0.0, 0.0, 2.5]",
        "Referenced Frame of Reference UID",
        "Source of Measurement",
        "PixelMed 20260608",
        "DicomSRValidator -checktemplateid",
        "No new allowlist entry",
        "Should become KB patch: yes",
    ] {
        assert!(
            source.contains(required),
            "SCOORD3D note requires {required}"
        );
    }

    let registry = read_json("cases/registry.json");
    let case = registry_cases(&registry)
        .into_iter()
        .find(|case| {
            case.get("case_id").and_then(Value::as_str)
                == Some("derived/sr/comprehensive3d_scoord3d")
        })
        .expect("Comprehensive 3D SCOORD3D row must exist");
    assert_eq!(case["status"], "implemented");
    assert_eq!(case["roadmap"], Value::Null);
    assert_eq!(case["blockers"], serde_json::json!([]));
    assert!(
        case["standards_evidence"]
            .as_array()
            .is_some_and(|evidence| evidence.iter().any(|entry| {
                entry["part"] == "PS3.16" && entry["anchor"] == "TID_1500_TID_1501_TID_300"
            }))
    );
}

#[test]
fn spatial_registration_source_note_locks_native_rigid_contract() {
    let source = fs::read_to_string("standards/source-notes/phase-3-spatial-registration.md")
        .expect("Spatial Registration source note must be readable");
    for required in [
        "derived/registration/spatial_ct_pair",
        "Provider: `rust_native`",
        "enhanced/ct/multiframe_shared_perframe_explicit_le",
        "classic/ct/mono2_i16_rescale_12bit_explicit_le",
        "Source RCS to Registered RCS",
        "geometry-derived translation `[+0.625,+0.625,+2.5]` mm",
        "`[-0.625,-0.625,0]` maps to",
        "Registration Type Code Sequence",
        "Studies Containing Other Referenced Instances Sequence",
        "expected_spatial_registration",
        "dicom-validator` 0.8.2",
        "secondary IOD evidence",
        "detect a VM 15",
        "strict Rust validation owns rigid",
        "owns reference closure",
        "must not be silently allowlisted",
        "Registry status: implemented",
        "522f2627658dd11ae6e5b88ad5e673659cacfdb2abf45fe4cb43adfb90feb7ea",
        "8b3b8498c3e90dc13e52cceb9c584fbb41d5898e28c2f3d3f86baf4a1654ac8",
        "Full-corpus conformance verification still reports 207",
        "Should become KB patch: yes",
    ] {
        assert!(
            source.contains(required),
            "Spatial Registration note requires {required}"
        );
    }

    let registry = read_json("cases/registry.json");
    let case = registry_cases(&registry)
        .into_iter()
        .find(|case| {
            case.get("case_id").and_then(Value::as_str)
                == Some("derived/registration/spatial_ct_pair")
        })
        .expect("Spatial Registration row must exist");
    assert_eq!(case["status"], "implemented");
    assert_eq!(case["provider"]["kind"], "rust_native");
    assert_eq!(case["provider"]["id"], "rust_native");
    assert_eq!(case["roadmap"], serde_json::Value::Null);
    assert_eq!(case["blockers"], serde_json::json!([]));
    assert_eq!(case["determinism"], "byte_stable");

    for (part, anchor) in [
        ("PS3.3", "A.39.1_C.20.1_C.20.2_C.12.2"),
        ("PS3.4", "table_B.5-1"),
        ("PS3.6", "table_A-1"),
        ("PS3.6", "table_6-1"),
        ("PS3.17", "O.1_O.3_O.5"),
    ] {
        assert!(
            case["standards_evidence"]
                .as_array()
                .is_some_and(|evidence| evidence
                    .iter()
                    .any(|entry| { entry["part"] == part && entry["anchor"] == anchor })),
            "Spatial Registration evidence requires {part} {anchor}"
        );
    }
}

#[test]
fn deformable_registration_source_note_locks_grid_sampling_contract() {
    let source =
        fs::read_to_string("standards/source-notes/phase-3-deformable-spatial-registration.md")
            .expect("Deformable Spatial Registration source note must be readable");
    for required in [
        "derived/registration/deformable_ct_pair",
        "Recommended provider: `rust_native`",
        "1.2.840.10008.5.1.4.1.1.66.3",
        "Registered RCS to Source RCS sampling",
        "M_post(M_pre(P_registered) + D)",
        "Grid Dimensions `(0064,0007)`: UL VM 3, `[2,2,1]`",
        "Grid Resolution `(0064,0008)`: FD VM 3, `[0.75,0.75,2.5]` mm",
        "Vector Grid Data `(0064,0009)`: OF VM 1, 48 bytes",
        "`i` (left to right) varying",
        "[-0.625, -0.625, -2.5]",
        "[-0.75,  -0.75,  -2.5]",
        "d0673d2da1b415db6465047e607b7f16f1a886dfae4ede91764c71bf7df72f47",
        "Pre Deformation Matrix Registration Sequence",
        "Post Deformation Matrix Registration Sequence",
        "Studies Containing Other Referenced Instances Sequence",
        "expected_deformable_spatial_registration",
        "926ab093e7f66bc9d7fb75ddaded704274325e19a878d3999d5ebd17de583672",
        "OF byte-count equation",
        "Only isolated `dcentvfy` detected a dangling SOP",
        "Registry status: implemented and byte-stable",
        "Registry provider: `rust_native`",
        "Registry blocker: none",
        "9a449b434db4863b3f6f848edf761b920ce5cc713e3d5142fd1801106ed912fe",
        "d8c539ad4ac9e72a8a597f9bf8a6588feac4d110d97464a70f6d543a033e5114",
        "225bef48a5503e4ed2adc88490d9f28d9f8c314e0bc34d3fa8bff0d144b4127e",
        "e6a78f3868532d08691c6570ad52e137ffce85661b0cc4ebb810cdff234e63ca",
        "No new finding may be silently allowlisted",
        "Should become KB patch: yes",
    ] {
        assert!(
            source.contains(required),
            "Deformable Spatial Registration note requires {required}"
        );
    }

    let registry = read_json("cases/registry.json");
    let case = registry_cases(&registry)
        .into_iter()
        .find(|case| {
            case.get("case_id").and_then(Value::as_str)
                == Some("derived/registration/deformable_ct_pair")
        })
        .expect("Deformable Spatial Registration row must exist");
    assert_eq!(case["status"], "implemented");
    assert_eq!(case["provider"]["kind"], "rust_native");
    assert_eq!(case["provider"]["id"], "rust_native");
    assert!(case["roadmap"].is_null());
    assert_eq!(case["blockers"], serde_json::json!([]));
    assert_eq!(case["determinism"], "byte_stable");
}

#[test]
fn color_softcopy_source_note_locks_native_color_contract() {
    let source =
        fs::read_to_string("standards/source-notes/phase-3-color-softcopy-presentation-state.md")
            .expect("Color Softcopy Presentation State source note must be readable");
    for required in [
        "derived/presentation-state/color_softcopy",
        "Selected provider: `rust_native`",
        "classic/sc/rgb_planar0_explicit_le",
        "1.2.840.10008.5.1.4.1.1.11.2",
        "same Study",
        "separate Presentation Series",
        "Referenced Series Sequence `(0008,1115)` contains exactly one Item",
        "Referenced Frame Number is absent",
        "Displayed Area Top Left Hand Corner `(0070,0052)`: SL VM 2, `[1,1]`",
        "Displayed Area Bottom Right Hand Corner `(0070,0053)`: SL VM 2, `[2,2]`",
        "Presentation Size Mode `(0070,0100)`: `SCALE TO FIT`",
        "Presentation Pixel Aspect Ratio `(0070,0102)`: IS VM 2, `[1,1]`",
        "ICC Profile `(0028,2000)` Type 1",
        "8e069a3476b71a0e0ae7272d9278ba70540d1c4a0b19af1c7d52e56f49091fef",
        "input color space at bytes 16 through 19: `RGB `",
        "profile connection space at bytes 20 through 23: `XYZ `",
        "DICOM Color Space `(0028,2002)`: `SRGB`",
        "expected_color_softcopy_presentation_state",
        "a3044e2dd64dcd2fa1e37620172db176495e68c598d3620986aaa194c436e982",
        "wrong enclosing referenced Series Instance UID",
        "Isolated `dcentvfy` detected a dangling referenced SOP Instance UID",
        "Strict Rust validation owns every exact semantic invariant",
        "No new finding may be silently allowlisted",
        "99832aaabe9ca4e36e4c108db44974de352b113ca1ccf0e4a41df74e88ced62a",
        "4e737e1429b7b2463bc412e4c6ff330411259f321070b32d9ce68cdef0bc0543",
        "b1e494962d40634300fb488fdf95c92ad80bad9b2d1e0f0be6bff9b4e8503b0a",
        "3dad35670aba58140d84cd326fd2624348b8f6215cd72e30d3ca76d35eae1801",
        "Registry status: implemented and byte-stable",
        "Registry provider: `rust_native`",
        "Registry blocker: none",
        "Should become KB patch: yes",
    ] {
        assert!(
            source.contains(required),
            "Color Softcopy Presentation State note requires {required}"
        );
    }

    let registry = read_json("cases/registry.json");
    let case = registry_cases(&registry)
        .into_iter()
        .find(|case| {
            case.get("case_id").and_then(Value::as_str)
                == Some("derived/presentation-state/color_softcopy")
        })
        .expect("Color Softcopy Presentation State row must exist");
    assert_eq!(case["status"], "implemented");
    assert_eq!(case["provider"]["kind"], "rust_native");
    assert_eq!(case["provider"]["id"], "rust_native");
    assert_eq!(case["blockers"], serde_json::json!([]));
    assert_eq!(case["determinism"], "byte_stable");
}

#[test]
fn advanced_blending_source_note_locks_native_two_input_contract() {
    let source = fs::read_to_string(
        "standards/source-notes/phase-3-advanced-blending-presentation-state.md",
    )
    .expect("Advanced Blending Presentation State source note must be readable");
    for required in [
        "derived/presentation-state/advanced_blending",
        "Selected provider: `rust_native`",
        "geometry/ct/multiseries_shared_frame_of_reference",
        "geometry/ct/multiseries_shared_frame_of_reference/series-001/slice-001.dcm",
        "geometry/ct/multiseries_shared_frame_of_reference/series-002/slice-002.dcm",
        "UIDs and whole-file hashes remain deterministic functions of the selected run",
        "1.2.840.10008.5.1.4.1.1.11.8",
        "PS3.3 Table A.33.7-1",
        "Advanced Blending Sequence `(0070,1B01)` is SQ VM 1 with exactly two Items",
        "Blending Input Number `(0070,1B02)` US VM 1 value `1`",
        "Time Series Blending `(0070,1B07)` CS VM 1 is `FALSE`",
        "Display `(0070,1B08)` CS VM 1 is `TRUE`",
        "Geometry for Display `TRUE`",
        "Pixel Presentation `(0008,9205)` is CS VM 1 value `TRUE_COLOR`",
        "Display Sequence `(0070,1B04)` is SQ VM 1 with exactly one Item",
        "Blending Mode `(0070,1B06)` is CS VM 1 value `EQUAL`",
        "Relative Opacity `(0070,0403)` is absent",
        "8e069a3476b71a0e0ae7272d9278ba70540d1c4a0b19af1c7d52e56f49091fef",
        "Referenced Series Sequence\n`(0008,1115)` contains exactly two Items",
        "expected_advanced_blending_presentation_state",
        "3e3f753545385fb448f3c5eb8618977663c7230158736fc943b9708ed62320d1",
        "Frame of Reference UID `(0020,0052)` and\nPosition Reference Indicator",
        "warnings remain unresolved independent-conformance findings",
        "They are not\nallowlisted",
        "Both IOD validators accepted duplicate Advanced Blending Input Numbers",
        "Strict Rust validation owns all cardinality, ordering, uniqueness, graph",
        "No new finding may be silently allowlisted",
        "Registry status: planned",
        "Registry provider: `rust_native`",
        "Registry blocker: exactly `recipe_unimplemented`",
        "Should become KB patch: yes",
    ] {
        assert!(
            source.contains(required),
            "Advanced Blending Presentation State note requires {required}"
        );
    }

    let registry = read_json("cases/registry.json");
    let case = registry_cases(&registry)
        .into_iter()
        .find(|case| {
            case.get("case_id").and_then(Value::as_str)
                == Some("derived/presentation-state/advanced_blending")
        })
        .expect("Advanced Blending Presentation State row must exist");
    assert_eq!(case["status"], "implemented");
    assert_eq!(case["provider"]["kind"], "rust_native");
    assert_eq!(case["provider"]["id"], "rust_native");
    assert_eq!(case["blockers"], serde_json::json!([]));
    assert_eq!(case["determinism"], "byte_stable");
    assert!(
        case["standards_evidence"]
            .as_array()
            .unwrap()
            .iter()
            .any(|evidence| evidence["query"]
                == "standards/source-notes/phase-3-advanced-blending-presentation-state.md")
    );
}

#[test]
fn blending_source_note_locks_audited_contract_before_provider_selection() {
    let source =
        fs::read_to_string("standards/source-notes/phase-3-blending-presentation-state.md")
            .expect("Blending Softcopy Presentation State source note must be readable");
    for required in [
        "derived/presentation-state/blending",
        "Recommended provider: `rust_native`",
        "geometry/ct/multiseries_shared_frame_of_reference",
        "geometry/ct/multiseries_shared_frame_of_reference/series-001/slice-001.dcm",
        "geometry/ct/multiseries_shared_frame_of_reference/series-002/slice-002.dcm",
        "1.2.840.10008.5.1.4.1.1.11.4",
        "PS3.3 Table A.33.4-1",
        "Presentation State Blending Sequence `(0070,0402)` is SQ VM 1 with exactly two",
        "`UNDERLYING`",
        "`SUPERIMPOSED`",
        "Relative Opacity `(0070,0403)` is FL VM 1 with exact value `0.5`",
        "Displayed Area Selection Sequence `(0070,005A)` is SQ VM 1 with exactly one",
        "each with\nUS VM 3 value `[256,0,16]`",
        "f393097e80ec38db493eb054a0886181eb2c0e8cf7b5cdf1de392fbe94b0d1f5",
        "8e069a3476b71a0e0ae7272d9278ba70540d1c4a0b19af1c7d52e56f49091fef",
        "expected_blending_presentation_state",
        "b6382bbc750feb18f25d3450ea14cf65aa5344950ee69c7e900926e6948056d4",
        "emitted no errors or\nwarnings",
        "`Passed` with zero errors",
        "Both IOD validators accepted two\nItems with duplicate `UNDERLYING`",
        "accepted Relative\nOpacity outside",
        "Strict Rust validation owns every cardinality, ordering, uniqueness, source",
        "triggers no Section 11 decision checkpoint",
        "Current registry status: planned and `semantic_stable`",
        "Current registry provider: external backend `dcmtk`",
        "`backend_contract_unimplemented` and\n  `independent_iod_validator_unavailable`",
        "this evidence\n  commit intentionally does not change registry state",
        "Should become KB patch: yes",
    ] {
        assert!(
            source.contains(required),
            "Blending Softcopy Presentation State note requires {required}"
        );
    }

    let registry = read_json("cases/registry.json");
    let case = registry_cases(&registry)
        .into_iter()
        .find(|case| {
            case.get("case_id").and_then(Value::as_str)
                == Some("derived/presentation-state/blending")
        })
        .expect("Blending Softcopy Presentation State row must exist");
    assert_eq!(case["status"], "implemented");
    assert_eq!(case["provider"]["kind"], "rust_native");
    assert_eq!(case["provider"]["id"], "rust_native");
    assert_eq!(case["determinism"], "byte_stable");
    assert_eq!(
        case["blockers"]
            .as_array()
            .expect("planned Blending blockers must be an array")
            .iter()
            .map(|blocker| blocker["code"].as_str().unwrap())
            .collect::<Vec<_>>(),
        Vec::<&str>::new()
    );
}

#[test]
fn general_ecg_source_note_locks_multigroup_contract_and_native_provider() {
    let source = fs::read_to_string("standards/source-notes/phase-3-general-ecg-waveform.md")
        .expect("General ECG Waveform source note must be readable");
    for required in [
        "non-image/waveform/general_ecg",
        "Recommended provider: `rust_native`",
        "1.2.840.10008.5.1.4.1.1.9.1.2",
        "PS3.3 A.34.4 and Table A.34.4-1",
        "Acquisition Context Sequence `(0040,0555)` is the required Type",
        "one through four\nItems",
        "one through twenty-four channels",
        "200 through 1,000 Hz inclusive",
        "| 1 | `STD12_250HZ` | 12 | 1,000 | 250 Hz | 4 s |",
        "| 2 | `AUX4_1000HZ` | 4 | 4,000 | 1,000 Hz | 4 s |",
        "Group 1 therefore contains exactly 24,000 bytes and Group\n2 exactly 32,000 bytes; their ordered aggregate is 56,000 bytes",
        "| 2 | 1 | A1 | `2:75` | Auxiliary unipolar lead 1 |",
        "| 2 | 4 | A4 | `2:78` | Auxiliary unipolar lead 4 |",
        "sixteen distinct sources deliberately exceed the 12-lead ECG IOD's\nthirteen-channel total",
        "Channel Time\nSkew `(003A,0214)` is `0`",
        "f8ee9bcd0797f85bc1a9fc3a47b828328931562fef6d8c645b4c85aae9b3f227",
        "((s * (c + 1) * (g + 1) * 37 + c * 101 + g * 307) mod 2001) - 1000",
        "Each Item owns a separate OW payload",
        "generalized `expected_waveform`",
        "ordered `multiplex_groups` array",
        "12x1000@250Hz; 4x4000@1000Hz",
        "4967dac55719ba63cbc7f404f444e00d4adf50c785c8353e89c94db0259ede05",
        "ca5c4a56d05a57c6587d84fffc31a842e8e369b09f1186e6542a619b69dac683",
        "five Waveform Sequence Items; twenty-five channels",
        "Sampling\nFrequency `199` and `1001`",
        "No\nfinding may be silently allowlisted",
        "triggers no Section 11 decision checkpoint",
        "user has adopted `uv`",
        "Current registry status: planned and `semantic_stable`",
        "Current registry provider: external backend `dcmtk`",
        "`backend_contract_unimplemented` and\n  `independent_payload_validator_unavailable`",
        "this evidence commit\n  intentionally does not change registry state",
        "Should become KB patch: yes",
    ] {
        assert!(
            source.contains(required),
            "General ECG Waveform note requires {required}"
        );
    }

    let registry = read_json("cases/registry.json");
    let case = registry_cases(&registry)
        .into_iter()
        .find(|case| {
            case.get("case_id").and_then(Value::as_str) == Some("non-image/waveform/general_ecg")
        })
        .expect("General ECG Waveform row must exist");
    assert_eq!(case["status"], "implemented");
    assert_eq!(case["provider"]["kind"], "rust_native");
    assert_eq!(case["provider"]["id"], "rust_native");
    assert_eq!(case["determinism"], "byte_stable");
    assert_eq!(
        case["blockers"]
            .as_array()
            .expect("implemented General ECG blockers must be an array")
            .iter()
            .map(|blocker| blocker["code"].as_str().unwrap())
            .collect::<Vec<_>>(),
        Vec::<&str>::new()
    );
    assert!(
        case["standards_evidence"]
            .as_array()
            .unwrap()
            .iter()
            .any(|entry| {
                entry["query"] == "standards/source-notes/phase-3-general-ecg-waveform.md"
                    && entry["anchor"] == "table_A.34.4-1"
            })
    );
}

#[test]
fn linked_rt_plan_image_source_note_and_native_providers_are_locked() {
    let source = fs::read_to_string("standards/source-notes/phase-3-rt-plan-image-linked.md")
        .expect("linked RT Plan and RT Image source note must be readable");
    for required in [
        "non-image/rt/plan_linked",
        "non-image/rt/image_linked",
        "Recommended provider: `rust_native`",
        "1.2.840.10008.5.1.4.1.1.481.5",
        "1.2.840.10008.5.1.4.1.1.481.1",
        "PS3.3 A.20 and Table A.20.3-1",
        "PS3.3 A.17 and Table A.17.3-1",
        "Enhanced CT -> existing RT Structure Set -> existing RT Dose",
        "Dose remains\n`DoseSummationType=RECORD`",
        "bytes, manifest contract, and references shall\nnot change",
        "directly references exactly one existing RT Structure Set",
        "standard-optional Referenced Dose Sequence with exactly one\nItem",
        "Referenced Beam Number `1`, Referenced Fraction\nGroup Number `1`, and Fraction Number `1`",
        "one fraction group and one beam, not a zero-beam\nPlan",
        "Beam Type is `STATIC`",
        "Radiation Type is `PHOTON`",
        "Number of Wedges, Number of Compensators, Number of Boli, and Number of Blocks\nare all `0`",
        "ordered Beam Limiting\nDevice Sequence has exactly two Items: `X` then `Y`",
        "Control Point `0`",
        "Cumulative Meterset Weight `0`",
        "Control Point `1` contains its index and\nCumulative Meterset Weight `1`",
        "Image Type `(0008,0008)` is `DERIVED\\\\SECONDARY\\\\DRR`",
        "Conversion Type `(0008,0064)` is `WSD`",
        "RT Image Plane `(3002,000C)` is `NORMAL`",
        "exactly 4 rows by 4 columns",
        "Photometric Interpretation `MONOCHROME2`",
        "Native Pixel Data uses OB",
        "00 11 22 33 44 55 66 77 88 99 aa bb cc dd ee ff",
        "Equivalently, zero-based pixel `(r,c)` is `17 * (4*r + c)`",
        "expected_rt_plan",
        "expected_rt_image",
        "No finding may be silently allowlisted",
        "`uv`-locked `dicom-validator` 0.8.2",
        "`dciodvfy -new`",
        "`dcentvfy -f`",
        "DCMTK `dcm2img`",
        "Plan Study ID is `DTS-RTSTRUCT`",
        "immutable enhanced CT and\nDose retain their historical Study IDs `DTS-ECT` and `DTS-RTDOSE`",
        "e9337a6c46fe85b56f1f563120dd3caf56ea1335355792db42386db959be6db2",
        "returned `Passed` with zero errors",
        "460d525ab06aaf74df963029f3ab39c2536e4e1c5bf4b75fcf16b500382db20c",
        "b061e5f654eb426bbab0da9cce0ac945aadcf3cf506182eb6bf33acd3d7a3659",
        "a8faed6abbf35c12a4b26e40f6feb19d736d90045c83b9f9a31f638d323e6811",
        "d0d78ffccf44218a27944cf1b80dec63c8afa7162b0e085532feb51706a04714",
        "87846c587a4f721b90624008a3f7abfc9ae70a31d83e28449e82528b408b3ce7",
        "146c7c29a15a573ab0348addd424b8e88547985f54d687bb6e793dcd88ac71d4",
        "071b32384d1648222424f77a0392e90ca11d6e51df0d5bd1fc0a241754bec1fc",
        "Strict verification reports 211 older or unrelated failures and zero accepted\nfindings",
        "All 20 Image controls remained parseable by `dcmdump`",
        "`PORTAL` without Reported Values Origin | detected | missed",
        "Wrong referenced Beam Number | missed | missed",
        "Changed Pixel Data byte | missed | missed | detected",
        "detected 10 of 20 mutations",
        "Removing the CT, Structure Set, Dose, or Plan\nindividually added",
        "Changing only\n`expected_rt_image.plan_reference.source_sha256`",
        "qualification step did not itself change registry status",
        "All 20 Plan controls remained parseable by `dcmdump`",
        "Wrong control-point count | detected | missed",
        "Wrong Frame of Reference UID | missed | missed",
        "These two immutable upstream diagnostics remain visible\nand unallowlisted",
        "no *additive* missing or dangling reference\nfinding",
        "where `-f` expects a file list is invalid evidence",
        "4967dac55719ba63cbc7f404f444e00d4adf50c785c8353e89c94db0259ede05",
        "ca5c4a56d05a57c6587d84fffc31a842e8e369b09f1186e6542a619b69dac683",
        "trigger no Section 11\ndecision checkpoint",
        "Evaluation of a current RT Radiation Set is a separate subsequent\ndecision",
        "both cases are planned and `semantic_stable`",
        "Current registry provider: external backend `dcmtk`",
        "`backend_contract_unimplemented` and\n  `independent_iod_validator_unavailable`",
        "This source-note commit\n  intentionally leaves the registry unchanged",
        "Should become KB patch: yes",
    ] {
        assert!(
            source.contains(required),
            "linked RT Plan/Image note requires {required}"
        );
    }

    let registry = read_json("cases/registry.json");
    for (case_id, status, blocker_codes) in [
        (
            "non-image/rt/plan_linked",
            "implemented",
            Vec::<&str>::new(),
        ),
        (
            "non-image/rt/image_linked",
            "implemented",
            Vec::<&str>::new(),
        ),
    ] {
        let case = registry_cases(&registry)
            .into_iter()
            .find(|case| case.get("case_id").and_then(Value::as_str) == Some(case_id))
            .unwrap_or_else(|| panic!("registry must retain {case_id}"));
        assert_eq!(case["status"], status);
        assert_eq!(case["provider"]["kind"], "rust_native");
        assert_eq!(case["provider"]["id"], "rust_native");
        assert_eq!(case["determinism"], "byte_stable");
        assert_eq!(
            case["blockers"]
                .as_array()
                .expect("linked RT blockers must be an array")
                .iter()
                .map(|blocker| blocker["code"].as_str().unwrap())
                .collect::<Vec<_>>(),
            blocker_codes
        );
        assert!(
            case["standards_evidence"]
                .as_array()
                .unwrap()
                .iter()
                .any(|entry| {
                    entry["query"] == "standards/source-notes/phase-3-rt-plan-image-linked.md"
                })
        );
    }
}

#[test]
fn minimal_rt_radiation_set_source_note_locks_required_companion_graph() {
    let source = fs::read_to_string("standards/source-notes/phase-3-rt-radiation-set-minimal.md")
        .expect("minimal RT Radiation Set source note must be readable");
    for required in [
        "non-image/rt/carm_photon_electron_radiation_minimal",
        "non-image/rt/radiation_set_minimal",
        "Recommended provider: `rust_native`",
        "1.2.840.10008.5.1.4.1.1.481.13",
        "1.2.840.10008.5.1.4.1.1.481.12",
        "An RT Radiation Set cannot be implemented as a standalone instance",
        "RT Radiation Sequence `(300A,0616)` Type 1",
        "second-generation\nRT Radiation SOP Instance",
        "The C-Arm Radiation is a distinct registry case",
        "Referenced Beam Number `1`",
        "once and only once",
        "PS3.3 A.86.1.5 and Table A.86.1.5-1",
        "Detail Flag `(300A,0638)` is\n`IDENT_ONLY`",
        "RT Record Flag `(300A,0639)` is `NO`",
        "(130102, DCM, \"Static Beam\")",
        "(130361, DCM, \"Radiotherapy Treatment Device\")",
        "({MU}, UCUM, \"Monitor Units\")",
        "(130358, DCM, \"Nominal Radiation Source Location\")",
        "1.2.840.10008.1.4.3.1",
        "(102538003, SCT, \"recumbent\")",
        "(40199007, SCT, \"supine\")",
        "(102540008, SCT, \"headfirst\")",
        "Number of RT Control Points `(300A,0604)` is `2`",
        "Control Point `1` contains RT Control Point Index `1`",
        "Cumulative Meterset `100`",
        "Zero counts must not be substituted",
        "PS3.3 A.86.1.4 and Table A.86.1.4-1",
        "Intended Number of Fractions `(300A,0636)` is `1`",
        "RT Radiation Set\nIntent `(300A,0637)` is `TREATMENT`",
        "DTS_TPG_1",
        "RT Dose Contribution is conditional and is absent",
        "expected_rt_radiation",
        "expected_rt_radiation_set",
        "Every cardinality is checked before indexing",
        "No finding may be silently allowlisted",
        "`dicom3tools dciodvfy` knows the SOP UID names but returns `Information Object\nNot found`",
        "Locked PixelMed 20260608 likewise reports\nthe IOD unrecognized",
        "`uv`-locked `dicom-validator` 0.8.2",
        "selected as the required primary IOD\nvalidator for exactly these two cases, subject to the locked defect correction",
        "KeyError: 'other_cond'",
        "Changing RT Record Flag to `YES` makes the validator pass but\nviolates A.86.1.5.4.3 and is forbidden",
        "narrowly guarded adapter corrections",
        "Patient Orientation Macro is invoked at the RT Treatment Position Macro\nscope",
        "engine's `NotEmpty`\noperator also returns true for an empty string",
        "Any definition that no longer matches\nthe expected input fails closed",
        "Both registry cases are implemented with no roadmap or\nblocker",
        "dcentvfy -f` evaluation reports both current SOP Classes as unrecognized",
        "574fa1caa3248a75b8c19f754a2ce70eb6452addb037f6fe9f5c8a9d1fc62d43",
        "4967dac55719ba63cbc7f404f444e00d4adf50c785c8353e89c94db0259ede05",
        "9f4853924ef520dd9b97ada0f14abd206fb15e6d8622e4d24a90f8b404a3e8c3",
    ] {
        assert!(
            source.contains(required),
            "minimal RT Radiation Set source note must contain {required:?}"
        );
    }

    let registry: Value = serde_json::from_str(
        &fs::read_to_string("cases/registry.json").expect("registry must be readable"),
    )
    .expect("registry must be valid JSON");
    let cases = registry["cases"].as_array().unwrap();
    for case_id in [
        "non-image/rt/carm_photon_electron_radiation_minimal",
        "non-image/rt/radiation_set_minimal",
    ] {
        let case = cases
            .iter()
            .find(|case| case.get("case_id").and_then(Value::as_str) == Some(case_id))
            .unwrap_or_else(|| panic!("registry must retain {case_id}"));
        assert_eq!(case["status"], "implemented");
        assert_eq!(case["provider"]["kind"], "rust_native");
        assert_eq!(case["provider"]["id"], "rust_native");
        assert_eq!(case["determinism"], "byte_stable");
        assert_eq!(case["roadmap"], Value::Null);
        assert_eq!(case["blockers"], serde_json::json!([]));
        assert!(
            case["standards_evidence"]
                .as_array()
                .unwrap()
                .iter()
                .any(|entry| {
                    entry["query"] == "standards/source-notes/phase-3-rt-radiation-set-minimal.md"
                })
        );
    }
}

#[test]
fn twelve_lead_ecg_registry_promotes_complete_native_slice() {
    let source = fs::read_to_string("standards/source-notes/phase-3-twelve-lead-ecg-waveform.md")
        .expect("Twelve-lead ECG Waveform source note must be readable");
    for required in [
        "non-image/waveform/twelve_lead_ecg",
        "Recommended provider: `rust_native`",
        "1.2.840.10008.5.1.4.1.1.9.1.1",
        "PS3.3 A.34.3 and Table A.34.3-1",
        "Acquisition Context Sequence `(0040,0555)` is the required\nType 2 empty Sequence",
        "Waveform Sequence `(5400,0100)` is SQ VM 1 with exactly one multiplex",
        "Number of\nWaveform Channels `(003A,0005)` `12`",
        "Number of Waveform Samples\n`(003A,0010)` `500`",
        "Sampling Frequency `(003A,001A)` `500`",
        "Waveform Sample Interpretation `(5400,1006)` is `SS`",
        "Waveform Data `(5400,1010)` is OW with exactly 12,000 little-endian bytes",
        "| 1 | I | `2:1` | Lead I |",
        "| 6 | aVF | `2:64` | aVF, augmented voltage, foot |",
        "| 12 | V6 | `2:8` | Lead V6 |",
        "Coding Scheme Designator `MDC`",
        "Channel Time Skew\n`(003A,0214)` is `0`",
        "f8ee9bcd0797f85bc1a9fc3a47b828328931562fef6d8c645b4c85aae9b3f227",
        "((s * (c + 1) * 37 + c * 101) mod 2001) - 1000",
        "98b7a9b1be25d9d64ffa75bc6e16ea80f60deed1891aeed8dfb440c1c19e6713",
        "expected_waveform",
        "9ce36490d6da3628223b1d18fe3157d040412e183e28015fde5a62d815b1ab80",
        "emitted no\nerrors or warnings",
        "`Passed` with zero errors",
        "Both accepted Sampling Frequency `199`",
        "Number of Waveform Samples `501` with the unchanged 12,000-byte",
        "Strict Rust validation therefore owns all IOD content constraints",
        "triggers no Section 11 decision checkpoint",
        "Current registry status: planned and `semantic_stable`",
        "Current registry provider: external backend `dcmtk`",
        "`backend_contract_unimplemented` and\n  `independent_payload_validator_unavailable`",
        "this evidence\n  commit intentionally does not change registry state",
        "Should become KB patch: yes",
    ] {
        assert!(
            source.contains(required),
            "Twelve-lead ECG Waveform note requires {required}"
        );
    }

    let registry = read_json("cases/registry.json");
    let case = registry_cases(&registry)
        .into_iter()
        .find(|case| {
            case.get("case_id").and_then(Value::as_str)
                == Some("non-image/waveform/twelve_lead_ecg")
        })
        .expect("Twelve-lead ECG Waveform row must exist");
    assert_eq!(case["status"], "implemented");
    assert_eq!(case["provider"]["kind"], "rust_native");
    assert_eq!(case["provider"]["id"], "rust_native");
    assert_eq!(case["determinism"], "byte_stable");
    assert_eq!(
        case["blockers"]
            .as_array()
            .expect("planned waveform blockers must be an array")
            .iter()
            .map(|blocker| blocker["code"].as_str().unwrap())
            .collect::<Vec<_>>(),
        Vec::<&str>::new()
    );
    assert!(
        case["standards_evidence"]
            .as_array()
            .unwrap()
            .iter()
            .any(|entry| {
                entry["query"] == "standards/source-notes/phase-3-twelve-lead-ecg-waveform.md"
                    && entry["anchor"] == "table_A.34.3-1"
            })
    );
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
        (
            "src/generator.rs",
            &[
                "case_id: \"",
                "SPATIAL_REGISTRATION_CASE_ID: &str = \"",
                "COLOR_SOFTCOPY_PRESENTATION_STATE_CASE_ID: &str = \"",
                "ADVANCED_BLENDING_PRESENTATION_STATE_CASE_ID: &str =\n    \"",
                "BLENDING_PRESENTATION_STATE_CASE_ID: &str = \"",
                "TWELVE_LEAD_ECG_CASE_ID: &str = \"",
                "GENERAL_ECG_CASE_ID: &str = \"",
                "RT_PLAN_CASE_ID: &str = \"",
                "RT_IMAGE_CASE_ID: &str = \"",
                "RT_RADIATION_CASE_ID: &str = \"",
                "RT_RADIATION_SET_CASE_ID: &str = \"",
            ][..],
        ),
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
        (
            "src/generator/native/wsi_tiled_full.rs",
            &["WSI_TILED_FULL_CASE_ID: &str = \""][..],
        ),
        (
            "src/generator/native/wsi_tiled_sparse.rs",
            &["WSI_TILED_SPARSE_CASE_ID: &str = \""][..],
        ),
        (
            "src/generator/native/wsi_pyramid.rs",
            &["WSI_PYRAMID_CASE_ID: &str = \""][..],
        ),
        (
            "src/generator/native/wsi_multiple_optical_paths.rs",
            &["WSI_MULTIPLE_OPTICAL_PATHS_CASE_ID: &str =\n    \""][..],
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
        (
            "src/generation_backends/tid1500.rs",
            &["CASE_ID: &str = \""][..],
        ),
        (
            "src/generation_backends/scoord3d.rs",
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
