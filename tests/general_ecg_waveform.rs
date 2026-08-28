use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use dicom_core::VR;
use dicom_dictionary_std::tags;
use dicom_object::{InMemDicomObject, open_file};
use serde_json::{Value, json};

const CASE_ID: &str = "non-image/waveform/general_ecg";
const RELATIVE_PATH: &str = "non-image/waveform/general_ecg/instance.dcm";
const SOP_CLASS_UID: &str = "1.2.840.10008.5.1.4.1.1.9.1.2";
const GROUP_PAYLOAD_SHA256: [&str; 2] = [
    "e4bfb8a3290d9057fa5f5935fa6960ce2a44a07f18991d28c190522739008dbb",
    "5b201d4fa7274ba36d6f7387c3d0217e1b5da161a915f983c2b63b995dde7bbe",
];
const AGGREGATE_PAYLOAD_SHA256: &str =
    "c450f55360d6c07394600e4c0f71f951565cd0e1699edfbbb52f660221c6abea";
const SEED_7_FILE_SHA256: &str = "a656720538672c95aacdf068ba89b0c6d6f78042610f3a665d55065d0a4ab40c";
const STANDARD_CHANNELS: [(&str, &str, &str); 12] = [
    ("I", "2:1", "Lead I"),
    ("II", "2:2", "Lead II"),
    ("III", "2:61", "Lead III"),
    ("aVR", "2:62", "aVR, augmented voltage, right"),
    ("aVL", "2:63", "aVL, augmented voltage, left"),
    ("aVF", "2:64", "aVF, augmented voltage, foot"),
    ("V1", "2:3", "Lead V1"),
    ("V2", "2:4", "Lead V2"),
    ("V3", "2:5", "Lead V3"),
    ("V4", "2:6", "Lead V4"),
    ("V5", "2:7", "Lead V5"),
    ("V6", "2:8", "Lead V6"),
];
const AUXILIARY_CHANNELS: [(&str, &str, &str); 4] = [
    ("A1", "2:75", "Auxiliary unipolar lead 1"),
    ("A2", "2:76", "Auxiliary unipolar lead 2"),
    ("A3", "2:77", "Auxiliary unipolar lead 3"),
    ("A4", "2:78", "Auxiliary unipolar lead 4"),
];
const STANDARD_CHANNEL_SHA256: [&str; 12] = [
    "3211bada5580e8bd9c5a2934deb231122706b00aa92f8cdc78480c03b2352197",
    "8f66471e35940851acdd9ea55b422c738bf50ea7971822deed0edca1980e1ea2",
    "9652eb91f4f73f2654c922048a1a8c8731a08062eecd6f5b373256831d0e82b0",
    "97fb26e75907437a705e4e28eb6492d51020570a23265bdf765aca3c4e7b2708",
    "c9776b85b3bda6adef798d33d3c7c95d64a1a7d5bf525866ccf7b0cf5fc3209e",
    "95871f48d729a001eeb1543b36a27059916df360e04838fd322d006661bafb44",
    "04513ee1f1d5803f3f53093f016a606a7fa874c5af8d2651749b909b93392366",
    "c12790f5b1f233662a0a1c3f266cd2abb15af5a75b39258ff961e9b4afaf7913",
    "750913ccad5eb7ec8d8199451e6eb9aa41357eb21d2a0dac6ba75dce4e5708bd",
    "218d5f967ef253722359fee1846485331c63de9330af1f9fad183d779a196cca",
    "9027ec7a0fc7fea3d8236a16a5aa6f265ff20e18a2575f99e61807e102fb3d81",
    "9280ad35672b82a7847d3ccabadd4d85a94be3d39d0a836191384571f0a23ab6",
];
const AUXILIARY_CHANNEL_SHA256: [&str; 4] = [
    "5da46776ad84a78eb0c16066cb8ac7d5e05ca6ad87170264b227c71261def284",
    "7bd73425422f4e79504b55932040e481ccdfafecabe1dba613ee36074a51b9e3",
    "e56dad9647dfa50a10b40d244e29eaedbf23d97a558901f46fbccc07ad1a1766",
    "e1b68207c92fe2cc4c6765fc097668f2600eeda152eb5a1d6f0444f4c9e36fbc",
];

#[test]
fn general_ecg_vertical_slice_is_byte_deterministic_and_closed() {
    let first_workspace = temporary_workspace("general-ecg-first");
    let second_workspace = temporary_workspace("general-ecg-second");
    let first_root = first_workspace.join("generated");
    let second_root = second_workspace.join("generated");
    let first_manifest = generate_extended(&first_workspace, &first_root);
    let second_manifest = generate_extended(&second_workspace, &second_root);
    let first_manifest_projection = deterministic_manifest_projection(&first_manifest);
    let second_manifest_projection = deterministic_manifest_projection(&second_manifest);
    let first = case_file(&first_manifest);
    let second = case_file(&second_manifest);
    let first_bytes = fs::read(first_root.join(RELATIVE_PATH)).expect("first General ECG");
    let second_bytes = fs::read(second_root.join(RELATIVE_PATH)).expect("second General ECG");

    assert_eq!(
        first_manifest_projection, second_manifest_projection,
        "seed-7 deterministic manifest projections"
    );
    assert_eq!(first, second, "General ECG entries");
    assert_eq!(first_bytes, second_bytes, "General ECG bytes");
    assert_eq!(
        dicom_test_suite::sha256_hex(&first_bytes),
        SEED_7_FILE_SHA256
    );
    assert_eq!(first["sha256"], dicom_test_suite::sha256_hex(&first_bytes));
    assert_eq!(first["determinism"], "byte_stable");
    assert_eq!(first_manifest["manifest_schema_version"], "0.2.0");
    assert_eq!(first_manifest["files"].as_array().map(Vec::len), Some(113));

    assert_schema_valid("schemas/manifest.schema.json", &first_manifest);
    assert_manifest_contract(first);
    assert_independent_dicom_parse(&first_root);

    for root in [&first_root, &second_root] {
        let validation = dicom_test_suite::validate_generated_root(root)
            .expect("generated extended root should validate");
        assert!(validation.failures.is_empty(), "{:?}", validation.failures);
        assert_eq!(validation.files_checked, 113);
    }

    let report =
        dicom_test_suite::build_coverage_report(&first_root).expect("coverage report should build");
    assert_schema_valid("schemas/coverage-report.schema.json", &report);
    assert_report_contract(&report);
    assert_registry_and_skip_closure(&first_manifest);

    fs::remove_dir_all(first_workspace).expect("remove first workspace");
    fs::remove_dir_all(second_workspace).expect("remove second workspace");
}

fn deterministic_manifest_projection(manifest: &Value) -> Value {
    let mut projection = manifest.clone();
    let files = projection["files"]
        .as_array_mut()
        .expect("manifest files should be an array");
    for file in files {
        if file["determinism"] == "semantic_stable" {
            let object = file
                .as_object_mut()
                .expect("manifest file should be an object");
            object.remove("sha256");
            object.remove("size_bytes");
            object
                .get_mut("generation_backend")
                .and_then(Value::as_object_mut)
                .expect("semantic-stable file should record its generation backend")
                .remove("invocation_elapsed_milliseconds");
        }
    }
    projection
}

fn assert_manifest_contract(file: &Value) {
    assert_eq!(file["path"], RELATIVE_PATH);
    assert_eq!(
        file.pointer("/recipe/recipe_id"),
        Some(&json!("non_image_waveform_general_ecg"))
    );
    assert_eq!(
        file.pointer("/dicom/sop_class_uid"),
        Some(&json!(SOP_CLASS_UID))
    );
    assert_eq!(
        file.pointer("/dicom/iod_name"),
        Some(&json!("General ECG Waveform"))
    );
    assert_eq!(
        file.pointer("/dicom/transfer_syntax_uid"),
        Some(&json!("1.2.840.10008.1.2.1"))
    );
    assert!(file["image"].is_null() && file["pixel_data"].is_null());
    assert_eq!(file["references"], json!([]));

    let expected = &file["expected_waveform"];
    assert_eq!(expected["iod_kind"], "general_ecg");
    assert_eq!(expected["sop_class_uid"], SOP_CLASS_UID);
    assert_eq!(expected["iod_name"], "General ECG Waveform");
    assert_eq!(expected["modality"], "ECG");
    assert_eq!(expected["acquisition_context_items"], 0);
    let groups = expected["multiplex_groups"].as_array().expect("groups");
    assert_eq!(groups.len(), 2);
    assert_group(
        &groups[0],
        1,
        "STD12_250HZ",
        1_000,
        250,
        &STANDARD_CHANNELS,
        24_000,
        GROUP_PAYLOAD_SHA256[0],
        &STANDARD_CHANNEL_SHA256,
    );
    assert_group(
        &groups[1],
        2,
        "AUX4_1000HZ",
        4_000,
        1_000,
        &AUXILIARY_CHANNELS,
        32_000,
        GROUP_PAYLOAD_SHA256[1],
        &AUXILIARY_CHANNEL_SHA256,
    );
    assert_eq!(
        expected["aggregate"],
        json!({
            "group_count": 2,
            "total_channel_count": 16,
            "common_duration_seconds": 4,
            "total_payload_length_bytes": 56_000,
            "group_payload_sha256": GROUP_PAYLOAD_SHA256,
            "aggregate_payload_sha256": AGGREGATE_PAYLOAD_SHA256
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
    let internal = file
        .pointer("/validation/internal")
        .and_then(Value::as_array)
        .expect("internal validation");
    let standards = file
        .pointer("/validation/standards")
        .and_then(Value::as_array)
        .expect("standards validation");
    assert!(
        internal
            .iter()
            .chain(standards)
            .all(|row| row["status"] == "passed")
    );
    for required in [
        "general_ecg_part10_preamble",
        "general_ecg_group_count",
        "general_ecg_formula_and_interleave",
        "general_ecg_group_1_channel_1_sha256",
        "general_ecg_group_2_channel_4_sha256",
        "general_ecg_aggregate_payload_length",
        "general_ecg_aggregate_payload_sha256",
        "general_ecg_pixel_data_absent",
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
            "general_ecg_waveform_sop_class",
            "explicit_vr_little_endian_transfer_syntax",
            "general_ecg_waveform_modules"
        ]
    );
}

#[allow(clippy::too_many_arguments)]
fn assert_group(
    group: &Value,
    ordinal: usize,
    label: &str,
    samples: u64,
    frequency: u64,
    expected_channels: &[(&str, &str, &str)],
    payload_length: u64,
    payload_sha256: &str,
    channel_sha256: &[&str],
) {
    assert_eq!(group["ordinal"], ordinal);
    assert_eq!(group["originality"], "ORIGINAL");
    assert_eq!(group["label"], label);
    assert_eq!(group["channel_count"], expected_channels.len());
    assert_eq!(group["samples_per_channel"], samples);
    assert_eq!(group["sampling_frequency_hz"], frequency);
    assert_eq!(group["duration_seconds"], 4);
    assert_eq!(group["simultaneous_sampling"], true);
    let channels = group["channels"].as_array().expect("channels");
    assert_eq!(channels.len(), expected_channels.len());
    for (index, ((label, code, meaning), channel)) in
        expected_channels.iter().zip(channels).enumerate()
    {
        assert_eq!(channel["ordinal"], index + 1);
        assert_eq!(channel["label"], *label);
        assert_eq!(channel["source"]["code_value"], *code);
        assert_eq!(channel["source"]["coding_scheme_designator"], "MDC");
        assert_eq!(channel["source"]["code_meaning"], *meaning);
        assert_eq!(channel["sensitivity"], 1);
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
    assert_eq!(storage["payload_length_bytes"], payload_length);
    assert_eq!(storage["payload_sha256"], payload_sha256);
    assert_eq!(storage["channel_sha256"], json!(channel_sha256));
    assert_eq!(
        storage["sample_value_formula"],
        "((s * (c + 1) * (g + 1) * 37 + c * 101 + g * 307) mod 2001) - 1000"
    );
    assert_eq!(storage["sample_min"], -1000);
    assert_eq!(storage["sample_max"], 1000);
    assert_eq!(storage["waveform_padding_value_absent"], true);
    assert_eq!(storage["value_field_padding_bytes"], 0);
}

fn assert_independent_dicom_parse(root: &Path) {
    let object = open_file(root.join(RELATIVE_PATH)).expect("independent DICOM parse");
    assert_eq!(object.meta().transfer_syntax(), "1.2.840.10008.1.2.1");
    assert_eq!(text(&object, tags::SOP_CLASS_UID), SOP_CLASS_UID);
    assert_eq!(text(&object, tags::MODALITY), "ECG");
    assert_eq!(text(&object, tags::SERIES_NUMBER), "91");
    assert!(object.element(tags::PIXEL_DATA).is_err());
    let groups = sequence(&object, tags::WAVEFORM_SEQUENCE);
    assert_eq!(groups.len(), 2);
    for (group, (label, channels, samples, frequency, length)) in groups.iter().zip([
        ("STD12_250HZ", 12_u16, 1_000_u32, "250", 24_000_usize),
        ("AUX4_1000HZ", 4_u16, 4_000_u32, "1000", 32_000_usize),
    ]) {
        assert_eq!(text(group, tags::MULTIPLEX_GROUP_LABEL), label);
        assert_eq!(
            number_u16(group, tags::NUMBER_OF_WAVEFORM_CHANNELS),
            channels
        );
        assert_eq!(number_u32(group, tags::NUMBER_OF_WAVEFORM_SAMPLES), samples);
        assert_eq!(text(group, tags::SAMPLING_FREQUENCY), frequency);
        let data = group.element(tags::WAVEFORM_DATA).expect("Waveform Data");
        assert_eq!(data.vr(), VR::OW);
        assert_eq!(data.to_bytes().expect("OW bytes").len(), length);
    }
}

fn assert_report_contract(report: &Value) {
    let row = report["coverage_matrix"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["case_id"] == CASE_ID)
        .expect("General ECG report row");
    assert_eq!(row["status"], "generated");
    assert_eq!(row["waveform_iod_kind"], "general_ecg");
    assert_eq!(row["waveform_group_count"], 2);
    assert_eq!(row["waveform_group_shapes"], "12x1000@250Hz; 4x4000@1000Hz");
    assert_eq!(
        row["waveform_group_channel_labels"],
        "STD12_250HZ[I, II, III, aVR, aVL, aVF, V1, V2, V3, V4, V5, V6]; AUX4_1000HZ[A1, A2, A3, A4]"
    );
    assert_eq!(row["waveform_group_payload_lengths_bytes"], "24000; 32000");
    assert_eq!(
        row["waveform_group_payload_sha256_values"],
        GROUP_PAYLOAD_SHA256.join("; ")
    );
    assert_eq!(row["waveform_total_channel_count"], 16);
    assert_eq!(row["waveform_total_channel_hash_count"], 16);
    assert_eq!(row["waveform_total_payload_length_bytes"], 56_000);
    assert_eq!(
        row["waveform_aggregate_payload_sha256"],
        AGGREGATE_PAYLOAD_SHA256
    );
    assert_eq!(row["waveform_all_groups_simultaneous_sampling"], true);
    assert_eq!(row["waveform_common_duration_seconds"], 4);
    assert_eq!(row["waveform_pixel_data_absent"], true);
    for field in [
        "waveform_channel_count",
        "waveform_samples_per_channel",
        "waveform_sampling_frequency_hz",
        "waveform_payload_length_bytes",
        "waveform_payload_sha256",
    ] {
        assert!(row[field].is_null(), "heterogeneous scalar {field}");
    }
    for pointer in [
        "/grouped_coverage/waveform_iod_kinds/general_ecg",
        "/grouped_coverage/waveform_group_counts/2",
        "/grouped_coverage/waveform_group_shape_orders/12x1000@250Hz; 4x4000@1000Hz",
        "/grouped_coverage/waveform_total_channel_counts/16",
        "/grouped_coverage/waveform_total_payload_lengths_bytes/56000",
        "/grouped_coverage/waveform_total_channel_hash_counts/16",
        "/grouped_coverage/waveform_common_durations_seconds/4",
    ] {
        assert_eq!(report.pointer(pointer), Some(&Value::from(1)), "{pointer}");
    }
    assert_eq!(
        report.pointer("/grouped_coverage/waveform_all_groups_simultaneous_sampling_states/true"),
        Some(&Value::from(2)),
        "Twelve-lead and General ECG both sample simultaneously within their groups"
    );
}

fn assert_registry_and_skip_closure(manifest: &Value) {
    let registry = read_repo_json("cases/registry.json");
    let row = registry["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["case_id"] == CASE_ID)
        .expect("General ECG registry row");
    assert_eq!(row["status"], "implemented");
    assert_eq!(row["blockers"], json!([]));
    assert_eq!(row["determinism"], "byte_stable");
    assert_eq!(row.pointer("/provider/id"), Some(&json!("rust_native")));
    assert!(
        manifest["skipped_cases"]
            .as_array()
            .unwrap()
            .iter()
            .all(|row| row["case_id"] != CASE_ID),
        "implemented General ECG must not be skipped"
    );
}

fn case_file(manifest: &Value) -> &Value {
    manifest["files"]
        .as_array()
        .unwrap()
        .iter()
        .find(|file| file["case_id"] == CASE_ID)
        .expect("General ECG manifest entry")
}

fn generate_extended(workspace: &Path, root: &Path) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
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
    let registry = read_repo_json("cases/registry.json");
    let row = registry["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["case_id"] == CASE_ID)
        .expect("General ECG registry row");
    assert_eq!(row["status"], "implemented");
    assert_eq!(row["blockers"], json!([]));
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

fn sequence(object: &InMemDicomObject, tag: dicom_core::Tag) -> &[InMemDicomObject] {
    object
        .element(tag)
        .expect("sequence")
        .items()
        .expect("items")
}

fn text(object: &InMemDicomObject, tag: dicom_core::Tag) -> String {
    object
        .element(tag)
        .expect("text element")
        .to_str()
        .expect("text")
        .trim_end_matches(['\0', ' '])
        .to_string()
}

fn number_u16(object: &InMemDicomObject, tag: dicom_core::Tag) -> u16 {
    object
        .element(tag)
        .expect("u16 element")
        .to_int()
        .expect("u16")
}

fn number_u32(object: &InMemDicomObject, tag: dicom_core::Tag) -> u32 {
    object
        .element(tag)
        .expect("u32 element")
        .to_int()
        .expect("u32")
}

fn read_repo_json(path: &str) -> Value {
    serde_json::from_slice(&fs::read(repo_path(path)).unwrap()).unwrap()
}

fn repo_path(path: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(path)
}
