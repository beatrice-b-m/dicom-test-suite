#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

const TWELVE_CASE_ID: &str = "non-image/waveform/twelve_lead_ecg";
const GENERAL_CASE_ID: &str = "non-image/waveform/general_ecg";
const ADAPTER_ID: &str = "pydicom-dicom-validator-waveform";

struct Fixture {
    evidence_root: PathBuf,
    evidence_path: PathBuf,
    allowlist: PathBuf,
    baseline: Value,
    sidecar_path: PathBuf,
    baseline_sidecar: Value,
}

#[test]
fn waveform_routes_bind_twelve_and_general_ordered_group_evidence() {
    for (label, case_id, expected) in [
        ("twelve", TWELVE_CASE_ID, twelve_expectation()),
        ("general", GENERAL_CASE_ID, general_expectation()),
    ] {
        let fixture = run_fixture(label, case_id, expected);
        assert!(
            verify(&fixture.evidence_root, &fixture.allowlist)
                .status
                .success(),
            "{case_id}"
        );
        let waveform = &fixture.baseline["instances"][0]["waveform"];
        assert_eq!(waveform["status"], "passed");
        assert_eq!(
            waveform["expected_group_payload_sha256"],
            fixture.baseline_sidecar["expected_contract"]["aggregate"]["group_payload_sha256"]
        );
        assert_eq!(
            waveform["actual_aggregate_payload_sha256"],
            fixture.baseline_sidecar["actual"]["aggregate"]["aggregate_payload_sha256"]
        );
    }
}

#[test]
fn waveform_route_requires_locked_secondary_iod_and_payload_evidence() {
    let fixture = run_fixture("required", TWELVE_CASE_ID, twelve_expectation());

    let mut missing_iod = fixture.baseline.clone();
    missing_iod["instances"][0]["results"]
        .as_array_mut()
        .unwrap()
        .retain(|result| result["adapter_id"] != ADAPTER_ID);
    write_json(&fixture.evidence_path, &missing_iod);
    assert_failure(
        &fixture.evidence_root,
        &fixture.allowlist,
        "required waveform secondary IOD validation incomplete",
    );

    let mut unlocked = fixture.baseline.clone();
    unlocked["tools"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|tool| tool["adapter_id"] == ADAPTER_ID)
        .unwrap()["lock_status"] = json!("mismatched");
    write_json(&fixture.evidence_path, &unlocked);
    assert_failure(
        &fixture.evidence_root,
        &fixture.allowlist,
        "required waveform secondary IOD validator is unlocked",
    );

    let mut missing_payload = fixture.baseline.clone();
    missing_payload["instances"][0]
        .as_object_mut()
        .unwrap()
        .remove("waveform");
    write_json(&fixture.evidence_path, &missing_payload);
    assert_failure(
        &fixture.evidence_root,
        &fixture.allowlist,
        "evidence schema",
    );
}

#[test]
fn general_waveform_sidecar_rejects_group_and_aggregate_tampering() {
    let fixture = run_fixture("tamper", GENERAL_CASE_ID, general_expectation());
    let mutations: Vec<Box<dyn Fn(&mut Value)>> = vec![
        Box::new(|sidecar| {
            sidecar["actual"]["multiplex_groups"]
                .as_array_mut()
                .unwrap()
                .pop();
        }),
        Box::new(|sidecar| {
            sidecar["actual"]["multiplex_groups"]
                .as_array_mut()
                .unwrap()
                .swap(0, 1);
        }),
        Box::new(|sidecar| {
            sidecar["actual"]["multiplex_groups"][1]["storage"]["payload_sha256"] =
                json!("0".repeat(64));
        }),
        Box::new(|sidecar| {
            sidecar["actual"]["multiplex_groups"][1]["channels"][3]["channel_sha256"] =
                json!("0".repeat(64));
        }),
        Box::new(|sidecar| {
            sidecar["actual"]["multiplex_groups"][1]["channels"][3]["label"] = json!("A5");
        }),
        Box::new(|sidecar| {
            sidecar["actual"]["aggregate"]["total_payload_length_bytes"] = json!(56_002);
        }),
        Box::new(|sidecar| {
            sidecar["actual"]["aggregate"]["aggregate_payload_sha256"] = json!("0".repeat(64));
        }),
        Box::new(|sidecar| {
            sidecar["source_instance_sha256"] = json!("0".repeat(64));
        }),
        Box::new(|sidecar| {
            sidecar["adapter_sha256"] = json!("0".repeat(64));
        }),
        Box::new(|sidecar| {
            sidecar["expected_contract"]["multiplex_groups"][1]["label"] = json!("ALTERED");
        }),
    ];
    for mutate in mutations {
        let mut sidecar = fixture.baseline_sidecar.clone();
        mutate(&mut sidecar);
        write_sidecar_and_link(&fixture, &sidecar);
        assert_failure(
            &fixture.evidence_root,
            &fixture.allowlist,
            "waveform payload evidence sidecar is not linked",
        );
    }
}

fn run_fixture(label: &str, case_id: &str, expected: Value) -> Fixture {
    let root = temp_dir(label);
    let generated = root.join("generated");
    write_waveform_manifest(&generated, case_id, expected);
    let manifest = read_json(generated.join("manifest.json"));
    let payload = root.join("waveform.json");
    fs::write(
        &payload,
        serde_json::to_vec(&waveform_payload(
            &manifest["files"][0]["expected_waveform"],
        ))
        .unwrap(),
    )
    .unwrap();
    let primary = fake_tool(&root, "primary", "exit 0");
    let entity = fake_tool(&root, "entity", "exit 0");
    let parser = fake_tool(&root, "parser", "exit 0");
    let waveform = fake_tool(
        &root,
        "waveform",
        &format!(
            "if [ \"$1\" = \"--waveform\" ]; then /bin/cat '{}'; fi\nexit 0",
            payload.display()
        ),
    );
    let config = root.join("validators.json");
    let mut waveform_adapter = adapter(ADAPTER_ID, "secondary_iod_validator", &waveform);
    waveform_adapter["supported_case_ids"] = json!([TWELVE_CASE_ID, GENERAL_CASE_ID]);
    waveform_adapter["waveform_arguments"] = json!(["--waveform", "{input}"]);
    fs::write(
        &config,
        serde_json::to_vec(&json!({
            "schema_version": "0.1.0",
            "adapters": [
                adapter("primary", "primary_iod_validator", &primary),
                waveform_adapter,
                adapter("entity", "entity_validator", &entity),
                adapter("parser", "independent_parser", &parser)
            ]
        }))
        .unwrap(),
    )
    .unwrap();
    let evidence_root = root.join("evidence");
    let run = Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args(["conformance", "run"])
        .arg(&generated)
        .args(["--out"])
        .arg(&evidence_root)
        .args(["--config"])
        .arg(&config)
        .output()
        .unwrap();
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    let evidence_path = evidence_root.join("conformance-run.json");
    let mut baseline = read_json(&evidence_path);
    for tool in baseline["tools"].as_array_mut().unwrap() {
        tool["lock_status"] = json!("matched");
    }
    write_json(&evidence_path, &baseline);
    let allowlist = root.join("allowlist.json");
    write_json(
        &allowlist,
        &json!({"schema_version": "0.1.0", "findings": []}),
    );
    let sidecar_relative = baseline["instances"][0]["waveform"]["evidence"]["path"]
        .as_str()
        .unwrap();
    let sidecar_path = evidence_root.join(sidecar_relative);
    let baseline_sidecar = read_json(&sidecar_path);
    Fixture {
        evidence_root,
        evidence_path,
        allowlist,
        baseline,
        sidecar_path,
        baseline_sidecar,
    }
}

fn write_sidecar_and_link(fixture: &Fixture, sidecar: &Value) {
    let bytes = serde_json::to_vec_pretty(sidecar).unwrap();
    fs::write(&fixture.sidecar_path, &bytes).unwrap();
    let mut evidence = fixture.baseline.clone();
    evidence["instances"][0]["waveform"]["evidence"]["sha256"] =
        json!(dicom_test_suite::sha256_hex(&bytes));
    write_json(&fixture.evidence_path, &evidence);
}

fn waveform_payload(expected: &Value) -> Value {
    let groups = expected["multiplex_groups"]
        .as_array()
        .unwrap()
        .iter()
        .map(|group| {
            let hashes = group["storage"]["channel_sha256"].as_array().unwrap();
            let expected_channels = group["channels"].as_array().unwrap();
            assert_eq!(expected_channels.len(), hashes.len());
            let channels = (0..expected_channels.len())
                .map(|index| {
                    let channel = &expected_channels[index];
                    json!({
                        "baseline": channel["baseline"],
                        "bits_stored": channel["bits_stored"],
                        "channel_number": channel["ordinal"],
                        "channel_sha256": hashes[index],
                        "correction_factor": channel["sensitivity_correction_factor"],
                        "label": channel["label"],
                        "sample_skew_present": false,
                        "sensitivity": channel["sensitivity"],
                        "sensitivity_unit": channel["sensitivity_units"],
                        "source": channel["source"],
                        "time_skew": channel["time_skew_seconds"]
                    })
                })
                .collect::<Vec<_>>();
            let mut storage = group["storage"].clone();
            storage["formula_match"] = json!(true);
            json!({
                "ordinal": group["ordinal"],
                "originality": group["originality"],
                "label": group["label"],
                "channel_count": group["channel_count"],
                "samples_per_channel": group["samples_per_channel"],
                "sampling_frequency_hz": group["sampling_frequency_hz"],
                "duration_seconds": group["duration_seconds"],
                "simultaneous_sampling": group["simultaneous_sampling"],
                "channels": channels,
                "storage": storage
            })
        })
        .collect::<Vec<_>>();
    json!({
        "adapter_id": ADAPTER_ID,
        "acquisition_context_items": expected["acquisition_context_items"],
        "absent_content": expected["absent_content"],
        "modality": expected["modality"],
        "multiplex_groups": groups,
        "pixel_data_present": false,
        "sop_class_uid": expected["sop_class_uid"],
        "transfer_syntax_uid": expected["transfer_syntax_uid"],
        "aggregate": expected["aggregate"]
    })
}

fn twelve_expectation() -> Value {
    let leads = standard_leads();
    let hashes = (0..12).map(|index| hex_hash(index + 1)).collect::<Vec<_>>();
    let group = expected_group(
        1,
        "RESTING_12_LEAD",
        500,
        500,
        1,
        leads,
        hashes.clone(),
        12_000,
        hex_hash(13),
        "((s * (c + 1) * 37 + c * 101) mod 2001) - 1000",
    );
    expectation(
        "twelve_lead_ecg",
        "1.2.840.10008.5.1.4.1.1.9.1.1",
        "12-lead ECG Waveform",
        vec![group],
        json!({
            "group_count": 1,
            "total_channel_count": 12,
            "common_duration_seconds": 1,
            "total_payload_length_bytes": 12000,
            "group_payload_sha256": [hex_hash(13)],
            "aggregate_payload_sha256": hex_hash(13)
        }),
    )
}

fn general_expectation() -> Value {
    let first_hashes = (0..12).map(|index| hex_hash(index + 1)).collect::<Vec<_>>();
    let second_hashes = (0..4).map(|index| hex_hash(index + 17)).collect::<Vec<_>>();
    let formula = "((s * (c + 1) * (g + 1) * 37 + c * 101 + g * 307) mod 2001) - 1000";
    let first = expected_group(
        1,
        "STD12_250HZ",
        1000,
        250,
        4,
        standard_leads(),
        first_hashes,
        24_000,
        hex_hash(14),
        formula,
    );
    let auxiliary = [
        ("A1", "2:75", "Auxiliary unipolar lead 1"),
        ("A2", "2:76", "Auxiliary unipolar lead 2"),
        ("A3", "2:77", "Auxiliary unipolar lead 3"),
        ("A4", "2:78", "Auxiliary unipolar lead 4"),
    ];
    let second = expected_group(
        2,
        "AUX4_1000HZ",
        4000,
        1000,
        4,
        auxiliary.to_vec(),
        second_hashes,
        32_000,
        hex_hash(15),
        formula,
    );
    expectation(
        "general_ecg",
        "1.2.840.10008.5.1.4.1.1.9.1.2",
        "General ECG Waveform",
        vec![first, second],
        json!({
            "group_count": 2,
            "total_channel_count": 16,
            "common_duration_seconds": 4,
            "total_payload_length_bytes": 56000,
            "group_payload_sha256": [hex_hash(14), hex_hash(15)],
            "aggregate_payload_sha256": hex_hash(16)
        }),
    )
}

#[allow(clippy::too_many_arguments)]
fn expected_group(
    ordinal: usize,
    label: &str,
    samples: usize,
    frequency: usize,
    duration: usize,
    leads: Vec<(&str, &str, &str)>,
    hashes: Vec<String>,
    payload_length: usize,
    payload_hash: String,
    formula: &str,
) -> Value {
    assert_eq!(leads.len(), hashes.len());
    let channels = leads
        .iter()
        .enumerate()
        .map(|(index, (label, code_value, code_meaning))| {
            json!({
                "ordinal": index + 1,
                "label": label,
                "source": {"code_value": code_value, "coding_scheme_designator": "MDC", "code_meaning": code_meaning},
                "sensitivity": 1,
                "sensitivity_units": {"code_value": "uV", "coding_scheme_designator": "UCUM", "code_meaning": "microvolt"},
                "sensitivity_correction_factor": 1,
                "baseline": 0,
                "bits_stored": 16,
                "time_skew_seconds": 0,
                "sample_skew_absent": true
            })
        })
        .collect::<Vec<_>>();
    json!({
        "ordinal": ordinal,
        "originality": "ORIGINAL",
        "label": label,
        "channel_count": channels.len(),
        "samples_per_channel": samples,
        "sampling_frequency_hz": frequency,
        "duration_seconds": duration,
        "simultaneous_sampling": true,
        "channels": channels,
        "storage": {
            "bits_allocated": 16,
            "sample_interpretation": "SS",
            "data_vr": "OW",
            "byte_order": "little_endian",
            "interleave_order": "channel_then_sample",
            "payload_length_bytes": payload_length,
            "payload_sha256": payload_hash,
            "channel_sha256": hashes,
            "sample_value_formula": formula,
            "sample_min": -1000,
            "sample_max": 1000,
            "waveform_padding_value_absent": true,
            "value_field_padding_bytes": 0
        }
    })
}

fn expectation(
    iod_kind: &str,
    sop_class_uid: &str,
    iod_name: &str,
    groups: Vec<Value>,
    aggregate: Value,
) -> Value {
    json!({
        "iod_kind": iod_kind,
        "sop_class_uid": sop_class_uid,
        "iod_name": iod_name,
        "modality": "ECG",
        "transfer_syntax_uid": "1.2.840.10008.1.2.1",
        "acquisition_context_items": 0,
        "multiplex_groups": groups,
        "aggregate": aggregate,
        "absent_content": {
            "annotation_module": true,
            "synchronization_module": true,
            "references": true,
            "image": true,
            "pixel_data": true
        }
    })
}

fn standard_leads() -> Vec<(&'static str, &'static str, &'static str)> {
    vec![
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
    ]
}

fn hex_hash(index: usize) -> String {
    format!("{:064x}", index)
}

fn adapter(id: &str, role: &str, executable: &Path) -> Value {
    json!({
        "id": id,
        "role": role,
        "executable": executable,
        "arguments": ["{input}"],
        "version_arguments": [],
        "timeout_seconds": 2,
        "required": true,
        "platforms": ["macos"],
        "capabilities": ["test"]
    })
}

fn write_waveform_manifest(root: &Path, case_id: &str, expected: Value) {
    let relative = format!("{case_id}/instance.dcm");
    let instance = root.join(&relative);
    fs::create_dir_all(instance.parent().unwrap()).unwrap();
    let bytes = b"synthetic waveform fixture";
    fs::write(&instance, bytes).unwrap();
    write_json(
        root.join("manifest.json"),
        &json!({
            "generator": {"name": "dicom-test-suite", "version": "test", "feature_flags": []},
            "run": {"seed": 1, "profile": "extended"},
            "standards": {"standards_lock_sha256": "1".repeat(64)},
            "files": [{
                "case_id": case_id,
                "path": relative,
                "sha256": dicom_test_suite::sha256_hex(bytes),
                "dicom": {
                    "sop_class_uid": expected["sop_class_uid"],
                    "transfer_syntax_uid": "1.2.840.10008.1.2.1"
                },
                "image": null,
                "pixel_data": null,
                "expected_waveform": expected
            }]
        }),
    );
}

fn verify(evidence: &Path, allowlist: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
        .args(["conformance", "verify"])
        .arg(evidence)
        .args(["--allowlist"])
        .arg(allowlist)
        .output()
        .unwrap()
}

fn assert_failure(evidence: &Path, allowlist: &Path, needle: &str) {
    let output = verify(evidence, allowlist);
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stdout).contains(needle),
        "expected {needle:?} in {}",
        String::from_utf8_lossy(&output.stdout)
    );
}

fn fake_tool(root: &Path, name: &str, body: &str) -> PathBuf {
    let path = root.join(name);
    fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
    let mut permissions = fs::metadata(&path).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&path, permissions).unwrap();
    path
}

fn read_json(path: impl AsRef<Path>) -> Value {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

fn write_json(path: impl AsRef<Path>, value: &Value) {
    fs::write(path, serde_json::to_vec_pretty(value).unwrap()).unwrap();
}

fn temp_dir(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "dts-conformance-{label}-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir_all(&root).unwrap();
    root
}
