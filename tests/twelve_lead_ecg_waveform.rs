use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::{Value, json};

const CASE_ID: &str = "non-image/waveform/twelve_lead_ecg";
const RELATIVE_PATH: &str = "non-image/waveform/twelve_lead_ecg/instance.dcm";
const SOP_CLASS_UID: &str = "1.2.840.10008.5.1.4.1.1.9.1.1";
const PAYLOAD_SHA256: &str = "98b7a9b1be25d9d64ffa75bc6e16ea80f60deed1891aeed8dfb440c1c19e6713";
const CHANNELS: [(&str, &str, &str, &str); 12] = [
    ("I", "2:1", "MDC", "Lead I"),
    ("II", "2:2", "MDC", "Lead II"),
    ("III", "2:61", "MDC", "Lead III"),
    ("aVR", "2:62", "MDC", "aVR, augmented voltage, right"),
    ("aVL", "2:63", "MDC", "aVL, augmented voltage, left"),
    ("aVF", "2:64", "MDC", "aVF, augmented voltage, foot"),
    ("V1", "2:3", "MDC", "Lead V1"),
    ("V2", "2:4", "MDC", "Lead V2"),
    ("V3", "2:5", "MDC", "Lead V3"),
    ("V4", "2:6", "MDC", "Lead V4"),
    ("V5", "2:7", "MDC", "Lead V5"),
    ("V6", "2:8", "MDC", "Lead V6"),
];
const CHANNEL_SHA256: [&str; 12] = [
    "7b4aee068e05c2bdff3896937c78a4c7a32f9ed2bde64d91b1d925913bf29476",
    "bd775dc70f76ea153a25832ad622b0cc26fbe6a37cf3ec6548a30965c4d17fba",
    "19d26b694df281209aa1296abbfa8f7d360e24a03a091422aba6f67663e2f3b1",
    "bb4c99d7857dbfcee5ee620bcff09b7060b61c5f2432427affc6139cb8d3cf9b",
    "230f52ed2ac57624a9a35214d7867711008dd56014f4176ce258623e5b596d3a",
    "60e167db3c081ba5bca957aba820afb519b790d048b660634d49566df88105f2",
    "cf8c73bebf746b799b1fe8aa2c908ca69bc7acc72311c64cbf4131fc8976609f",
    "0f11e5fb5105dac699fa4bcfc01c79fbe696a81db04606f39a719de57b4c7c30",
    "a41d5962abceb6dbe25f8421091ce3df6a69202c45b24ab6b0736159d15e253b",
    "d655e2cbb23d70e229ed52fedba9c45573e22729fed0a794ab690df8d7f33804",
    "005c539f9f4256a86d9e0a212b3bfe73741f99942b0677fb483c0c48db9583cd",
    "f448df95acb226c5c992363e27707a42efc3ffb974ebeff38e2a81522b57d82c",
];

#[test]
fn twelve_lead_ecg_vertical_slice_is_byte_deterministic_and_closed() {
    let first_workspace = temporary_workspace("twelve-lead-first");
    let second_workspace = temporary_workspace("twelve-lead-second");
    let first_root = first_workspace.join("generated");
    let second_root = second_workspace.join("generated");
    let first_manifest = generate_extended(&first_workspace, &first_root);
    let second_manifest = generate_extended(&second_workspace, &second_root);
    let first_manifest_projection = deterministic_manifest_projection(&first_manifest);
    let second_manifest_projection = deterministic_manifest_projection(&second_manifest);
    let first = case_file(&first_manifest);
    let second = case_file(&second_manifest);
    let first_bytes = fs::read(first_root.join(RELATIVE_PATH)).expect("first ECG instance");
    let second_bytes = fs::read(second_root.join(RELATIVE_PATH)).expect("second ECG instance");

    assert_eq!(
        first_manifest_projection, second_manifest_projection,
        "seed-7 deterministic manifest projections must match"
    );
    assert_eq!(first, second, "seed-7 ECG entries must match");
    assert_eq!(first_bytes, second_bytes, "seed-7 ECG bytes must match");
    assert_eq!(first["sha256"], synth_dicom_gen::sha256_hex(&first_bytes));
    assert_eq!(second["sha256"], synth_dicom_gen::sha256_hex(&second_bytes));
    assert_eq!(first["determinism"], "byte_stable");

    assert_schema_valid("schemas/manifest.schema.json", &first_manifest);
    assert_manifest_contract(first);

    for root in [&first_root, &second_root] {
        let validation = synth_dicom_gen::validate_generated_root(root)
            .expect("generated extended root should validate");
        assert!(validation.failures.is_empty(), "{:?}", validation.failures);
        assert_eq!(
            validation.files_checked,
            first_manifest["files"].as_array().unwrap().len()
        );
    }

    let report =
        synth_dicom_gen::build_coverage_report(&first_root).expect("coverage report should build");
    assert_schema_valid("schemas/coverage-report.schema.json", &report);
    assert_report_contract(&report);

    fs::remove_dir_all(first_workspace).expect("remove first workspace");
    fs::remove_dir_all(second_workspace).expect("remove second workspace");
}

fn deterministic_manifest_projection(manifest: &Value) -> Value {
    let mut projection = manifest.clone();
    for file in projection["files"]
        .as_array_mut()
        .expect("manifest files should be an array")
    {
        if file["determinism"] == "semantic_stable" {
            let object = file
                .as_object_mut()
                .expect("manifest file should be an object");
            object.remove("sha256");
            object.remove("size_bytes");
            if let Some(backend) = object
                .get_mut("generation_backend")
                .and_then(Value::as_object_mut)
            {
                backend.remove("invocation_elapsed_milliseconds");
            }
        }
    }
    projection
}

fn assert_manifest_contract(file: &Value) {
    assert_eq!(file["path"], RELATIVE_PATH);
    assert_eq!(
        file.pointer("/recipe/recipe_id"),
        Some(&json!("non_image_waveform_twelve_lead_ecg"))
    );
    assert_eq!(
        file.pointer("/dicom/sop_class_uid"),
        Some(&json!(SOP_CLASS_UID))
    );
    assert_eq!(
        file.pointer("/dicom/transfer_syntax_uid"),
        Some(&json!("1.2.840.10008.1.2.1"))
    );
    assert!(file["image"].is_null());
    assert!(file["pixel_data"].is_null());
    assert_eq!(file["references"], json!([]));

    let expected = &file["expected_waveform"];
    assert_eq!(expected["iod_kind"], "twelve_lead_ecg");
    assert_eq!(expected["sop_class_uid"], SOP_CLASS_UID);
    assert_eq!(expected["iod_name"], "12-lead ECG Waveform");
    assert_eq!(expected["modality"], "ECG");
    assert_eq!(expected["transfer_syntax_uid"], "1.2.840.10008.1.2.1");
    assert_eq!(expected["acquisition_context_items"], 0);
    let groups = expected["multiplex_groups"]
        .as_array()
        .expect("waveform multiplex groups");
    assert_eq!(groups.len(), 1);
    let group = &groups[0];
    for (field, value) in [
        ("ordinal", json!(1)),
        ("originality", json!("ORIGINAL")),
        ("label", json!("RESTING_12_LEAD")),
        ("channel_count", json!(12)),
        ("samples_per_channel", json!(500)),
        ("sampling_frequency_hz", json!(500)),
        ("duration_seconds", json!(1)),
        ("simultaneous_sampling", json!(true)),
    ] {
        assert_eq!(group[field], value, "multiplex group {field}");
    }

    let channels = group["channels"].as_array().expect("waveform channels");
    assert_eq!(channels.len(), CHANNELS.len());
    for (index, ((label, code, scheme, meaning), channel)) in
        CHANNELS.iter().zip(channels).enumerate()
    {
        assert_eq!(channel["ordinal"], index + 1);
        assert_eq!(channel["label"], *label);
        assert_eq!(channel["source"]["code_value"], *code);
        assert_eq!(channel["source"]["coding_scheme_designator"], *scheme);
        assert_eq!(channel["source"]["code_meaning"], *meaning);
        assert_eq!(channel["sensitivity"], 1);
        assert_eq!(
            channel["sensitivity_units"],
            json!({"code_value": "uV", "coding_scheme_designator": "UCUM", "code_meaning": "microvolt"})
        );
        assert_eq!(channel["sensitivity_correction_factor"], 1);
        assert_eq!(channel["baseline"], 0);
        assert_eq!(channel["bits_stored"], 16);
        assert_eq!(channel["time_skew_seconds"], 0);
        assert_eq!(channel["sample_skew_absent"], true);
    }

    let storage = &group["storage"];
    assert_eq!(storage["bits_allocated"], 16);
    assert_eq!(storage["sample_interpretation"], "SS");
    assert_eq!(storage["data_vr"], "OW");
    assert_eq!(storage["byte_order"], "little_endian");
    assert_eq!(storage["interleave_order"], "channel_then_sample");
    assert_eq!(storage["payload_length_bytes"], 12_000);
    assert_eq!(storage["payload_sha256"], PAYLOAD_SHA256);
    assert_eq!(storage["channel_sha256"], json!(CHANNEL_SHA256));
    assert_eq!(
        storage["sample_value_formula"],
        "((s * (c + 1) * 37 + c * 101) mod 2001) - 1000"
    );
    assert_eq!(storage["sample_min"], -1000);
    assert_eq!(storage["sample_max"], 1000);
    assert_eq!(storage["waveform_padding_value_absent"], true);
    assert_eq!(storage["value_field_padding_bytes"], 0);
    assert_eq!(
        expected["aggregate"],
        json!({
            "group_count": 1,
            "total_channel_count": 12,
            "common_duration_seconds": 1,
            "total_payload_length_bytes": 12_000,
            "group_payload_sha256": [PAYLOAD_SHA256],
            "aggregate_payload_sha256": PAYLOAD_SHA256
        })
    );
    assert_eq!(
        expected["absent_content"],
        json!({
            "annotation_module": true,
            "synchronization_module": true,
            "references": true,
            "image": true,
            "pixel_data": true
        })
    );

    assert_eq!(file.pointer("/validation/status"), Some(&json!("passed")));
    assert_eq!(file.pointer("/validation/external"), Some(&json!([])));
    let internal = file
        .pointer("/validation/internal")
        .and_then(Value::as_array)
        .expect("internal validation evidence");
    let standards = file
        .pointer("/validation/standards")
        .and_then(Value::as_array)
        .expect("standards validation evidence");
    assert!(
        internal
            .iter()
            .chain(standards)
            .all(|row| row["status"] == "passed")
    );
    for required in [
        "twelve_lead_ecg_part10_preamble",
        "twelve_lead_ecg_group_count",
        "twelve_lead_ecg_channel_count",
        "twelve_lead_ecg_formula_and_interleave",
        "twelve_lead_ecg_payload_length",
        "twelve_lead_ecg_payload_sha256",
        "twelve_lead_ecg_channel_hash_count",
        "twelve_lead_ecg_channel_1_sha256",
        "twelve_lead_ecg_channel_12_sha256",
        "twelve_lead_ecg_waveform_padding_absent",
        "twelve_lead_ecg_pixel_data_absent",
    ] {
        assert!(
            internal.iter().any(|row| row["name"] == required),
            "missing internal evidence {required}"
        );
    }
    assert_eq!(
        standards
            .iter()
            .map(|row| row["name"].as_str().unwrap())
            .collect::<Vec<_>>(),
        [
            "twelve_lead_ecg_waveform_sop_class",
            "explicit_vr_little_endian_transfer_syntax",
            "twelve_lead_ecg_waveform_modules"
        ]
    );
}

fn assert_report_contract(report: &Value) {
    let row = report["coverage_matrix"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["case_id"] == CASE_ID)
        .expect("Twelve-lead report row");
    for (field, expected) in [
        ("waveform_group_count", 1),
        ("waveform_channel_count", 12),
        ("waveform_samples_per_channel", 500),
        ("waveform_sampling_frequency_hz", 500),
        ("waveform_duration_seconds", 1),
        ("waveform_bits_allocated", 16),
        ("waveform_bits_stored", 16),
        ("waveform_payload_length_bytes", 12_000),
        ("waveform_channel_hash_count", 12),
    ] {
        assert_eq!(row[field], expected, "{field}");
    }
    assert_eq!(row["waveform_iod_kind"], "twelve_lead_ecg");
    assert_eq!(row["waveform_group_shapes"], "RESTING_12_LEAD:12x500@500Hz");
    assert_eq!(
        row["waveform_group_channel_labels"],
        "RESTING_12_LEAD[I, II, III, aVR, aVL, aVF, V1, V2, V3, V4, V5, V6]"
    );
    assert_eq!(row["waveform_group_payload_lengths_bytes"], "12000");
    assert_eq!(row["waveform_group_payload_sha256_values"], PAYLOAD_SHA256);
    assert_eq!(row["waveform_total_channel_count"], 12);
    assert_eq!(row["waveform_total_payload_length_bytes"], 12_000);
    assert_eq!(row["waveform_aggregate_payload_sha256"], PAYLOAD_SHA256);
    assert_eq!(row["waveform_total_channel_hash_count"], 12);
    assert_eq!(row["waveform_all_groups_simultaneous_sampling"], true);
    assert_eq!(row["waveform_common_duration_seconds"], 1);
    assert_eq!(
        row["waveform_channel_labels"],
        "I; II; III; aVR; aVL; aVF; V1; V2; V3; V4; V5; V6"
    );
    assert_eq!(row["waveform_sample_interpretation"], "SS");
    assert_eq!(row["waveform_storage_vr"], "OW");
    assert_eq!(row["waveform_payload_sha256"], PAYLOAD_SHA256);
    assert_eq!(row["waveform_interleave_order"], "channel_then_sample");
    assert_eq!(row["waveform_simultaneous_sampling"], true);
    assert_eq!(row["waveform_pixel_data_absent"], true);
    assert_eq!(
        row["waveform_external_validator_disposition"],
        "external conformance evidence not embedded; run conformance separately"
    );
    for pointer in [
        "/grouped_coverage/waveform_iod_kinds/twelve_lead_ecg",
        "/grouped_coverage/waveform_group_counts/1",
        "/grouped_coverage/waveform_group_shape_orders/RESTING_12_LEAD:12x500@500Hz",
        "/grouped_coverage/waveform_total_channel_counts/12",
        "/grouped_coverage/waveform_total_payload_lengths_bytes/12000",
        "/grouped_coverage/waveform_aggregate_payload_sha256_values/98b7a9b1be25d9d64ffa75bc6e16ea80f60deed1891aeed8dfb440c1c19e6713",
        "/grouped_coverage/waveform_total_channel_hash_counts/12",
        "/grouped_coverage/waveform_common_durations_seconds/1",
        "/grouped_coverage/waveform_channel_counts/12",
        "/grouped_coverage/waveform_samples_per_channel/500",
        "/grouped_coverage/waveform_sampling_frequencies_hz/500",
        "/grouped_coverage/waveform_durations_seconds/1",
        "/grouped_coverage/waveform_bits_allocated/16",
        "/grouped_coverage/waveform_bits_stored/16",
        "/grouped_coverage/waveform_payload_length_bytes/12000",
        "/grouped_coverage/waveform_payload_sha256_values/98b7a9b1be25d9d64ffa75bc6e16ea80f60deed1891aeed8dfb440c1c19e6713",
        "/grouped_coverage/waveform_interleave_orders/channel_then_sample",
        "/grouped_coverage/waveform_channel_hash_counts/12",
        "/grouped_coverage/waveform_simultaneous_sampling_states/true",
    ] {
        assert_eq!(report.pointer(pointer), Some(&Value::from(1)), "{pointer}");
    }
    for pointer in [
        "/grouped_coverage/waveform_all_groups_simultaneous_sampling_states/true",
        "/grouped_coverage/waveform_pixel_data_absent_states/true",
    ] {
        assert_eq!(report.pointer(pointer), Some(&Value::from(2)), "{pointer}");
    }
}

fn case_file(manifest: &Value) -> &Value {
    manifest["files"]
        .as_array()
        .unwrap()
        .iter()
        .find(|file| file["case_id"] == CASE_ID)
        .expect("Twelve-lead ECG manifest entry")
}

fn generate_extended(workspace: &Path, root: &Path) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_synth-dicom-gen"))
        .current_dir(workspace)
        .args([
            "generate",
            "--profile",
            "extended",
            "--out",
            root.to_str().unwrap(),
            "--seed",
            "7",
        ])
        .output()
        .expect("extended generation");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&fs::read(root.join("manifest.json")).unwrap()).unwrap()
}

fn temporary_workspace(label: &str) -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let path = std::env::temp_dir().join(format!(
        "dicom-test-suite-{label}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    if path.exists() {
        fs::remove_dir_all(&path).unwrap();
    }
    fs::create_dir_all(path.join("cases")).expect("create temporary project cases directory");
    for metadata in [
        "Cargo.lock",
        "standards.lock.json",
        "generation-backends.lock.json",
    ] {
        fs::copy(repo_path(metadata), path.join(metadata)).expect("copy locked metadata");
    }
    #[cfg(unix)]
    std::os::unix::fs::symlink(
        repo_path("generation-backends"),
        path.join("generation-backends"),
    )
    .expect("link locked generation backends");

    let mut registry = read_repo_json("cases/registry.json");
    let case = registry["cases"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|case| case["case_id"] == CASE_ID)
        .expect("Twelve-lead registry row");
    assert!(
        matches!(case["status"].as_str(), Some("planned" | "implemented")),
        "Twelve-lead row must be promotable or already promoted"
    );
    case["status"] = json!("implemented");
    case["blockers"] = json!([]);
    case["determinism"] = json!("byte_stable");
    fs::write(
        path.join("cases/registry.json"),
        serde_json::to_vec_pretty(&registry).unwrap(),
    )
    .expect("write temporary registry");
    path
}

fn assert_schema_valid(path: &str, value: &Value) {
    let schema = read_repo_json(path);
    let validator = jsonschema::validator_for(&schema).expect("JSON schema should compile");
    let errors = validator
        .iter_errors(value)
        .map(|error| error.to_string())
        .collect::<Vec<_>>();
    assert!(errors.is_empty(), "{path} failures: {errors:#?}");
}

fn read_repo_json(path: &str) -> Value {
    serde_json::from_slice(&fs::read(repo_path(path)).unwrap()).unwrap()
}

fn repo_path(path: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(path)
}
