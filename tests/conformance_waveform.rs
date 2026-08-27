#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

const CASE_ID: &str = "non-image/waveform/twelve_lead_ecg";
const ADAPTER_ID: &str = "pydicom-dicom-validator-waveform";

#[test]
fn waveform_route_requires_locked_iod_and_hash_linked_payload_evidence() {
    let root = temp_dir("waveform");
    let generated = root.join("generated");
    write_waveform_manifest(&generated);

    let manifest = read_json(generated.join("manifest.json"));
    let expected = &manifest["files"][0]["expected_waveform"];
    let payload = root.join("waveform.json");
    fs::write(
        &payload,
        serde_json::to_vec(&waveform_payload(expected)).unwrap(),
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
    waveform_adapter["supported_case_ids"] = json!([CASE_ID]);
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
    let mut evidence = read_json(&evidence_path);
    for tool in evidence["tools"].as_array_mut().unwrap() {
        tool["lock_status"] = json!("matched");
    }
    write_json(&evidence_path, &evidence);
    let allowlist = root.join("allowlist.json");
    write_json(
        &allowlist,
        &json!({"schema_version": "0.1.0", "findings": []}),
    );
    assert!(verify(&evidence_root, &allowlist).status.success());

    let baseline = evidence.clone();
    let sidecar_relative = baseline["instances"][0]["waveform"]["evidence"]["path"]
        .as_str()
        .unwrap();
    let sidecar_path = evidence_root.join(sidecar_relative);
    let baseline_sidecar = fs::read(&sidecar_path).unwrap();

    let mut missing_iod = baseline.clone();
    missing_iod["instances"][0]["results"]
        .as_array_mut()
        .unwrap()
        .retain(|result| result["adapter_id"] != ADAPTER_ID);
    write_json(&evidence_path, &missing_iod);
    assert_failure(
        &evidence_root,
        &allowlist,
        "required waveform secondary IOD validation incomplete",
    );

    let mut unlocked = baseline.clone();
    unlocked["tools"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|tool| tool["adapter_id"] == ADAPTER_ID)
        .unwrap()["lock_status"] = json!("mismatched");
    write_json(&evidence_path, &unlocked);
    assert_failure(
        &evidence_root,
        &allowlist,
        "required waveform secondary IOD validator is unlocked",
    );

    let mut missing_payload = baseline.clone();
    missing_payload["instances"][0]
        .as_object_mut()
        .unwrap()
        .remove("waveform");
    write_json(&evidence_path, &missing_payload);
    assert_failure(&evidence_root, &allowlist, "evidence schema");

    let mut tampered_sidecar: Value = serde_json::from_slice(&baseline_sidecar).unwrap();
    tampered_sidecar["expected_contract"]["storage"]["payload_sha256"] = json!("0".repeat(64));
    let tampered_bytes = serde_json::to_vec_pretty(&tampered_sidecar).unwrap();
    fs::write(&sidecar_path, &tampered_bytes).unwrap();
    let mut tampered_evidence = baseline;
    tampered_evidence["instances"][0]["waveform"]["evidence"]["sha256"] =
        json!(dicom_test_suite::sha256_hex(&tampered_bytes));
    write_json(&evidence_path, &tampered_evidence);
    assert_failure(
        &evidence_root,
        &allowlist,
        "waveform payload evidence sidecar is not linked",
    );
}

fn waveform_payload(expected: &Value) -> Value {
    let hashes = expected["storage"]["channel_sha256"].as_array().unwrap();
    let channels = expected["channels"]
        .as_array()
        .unwrap()
        .iter()
        .zip(hashes)
        .map(|(channel, hash)| {
            json!({
                "baseline": channel["baseline"],
                "bits_stored": channel["bits_stored"],
                "channel_number": channel["ordinal"],
                "channel_sha256": hash,
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
    json!({
        "adapter_id": ADAPTER_ID,
        "bits_allocated": expected["storage"]["bits_allocated"],
        "byte_order": expected["storage"]["byte_order"],
        "channel_count": expected["multiplex_group"]["channel_count"],
        "channel_definitions": channels,
        "channel_hashes": hashes,
        "duration_seconds": expected["multiplex_group"]["duration_seconds"],
        "formula_match": true,
        "interleave_order": expected["storage"]["interleave_order"],
        "modality": expected["modality"],
        "multiplex_group_count": expected["multiplex_group"]["group_count"],
        "multiplex_group_label": expected["multiplex_group"]["label"],
        "originality": expected["multiplex_group"]["originality"],
        "pixel_data_present": false,
        "sample_count": expected["multiplex_group"]["samples_per_channel"],
        "sample_interpretation": expected["storage"]["sample_interpretation"],
        "sampling_frequency_hz": expected["multiplex_group"]["sampling_frequency_hz"],
        "sop_class_uid": expected["sop_class_uid"],
        "stored_value_max": expected["storage"]["sample_max"],
        "stored_value_min": expected["storage"]["sample_min"],
        "transfer_syntax_uid": expected["transfer_syntax_uid"],
        "waveform_data_length": expected["storage"]["payload_length_bytes"],
        "waveform_data_sha256": expected["storage"]["payload_sha256"],
        "waveform_data_vr": expected["storage"]["data_vr"],
        "waveform_padding_present": false
    })
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

fn write_waveform_manifest(root: &Path) {
    let relative = "non-image/waveform/twelve_lead_ecg/instance.dcm";
    let instance = root.join(relative);
    fs::create_dir_all(instance.parent().unwrap()).unwrap();
    let bytes = b"synthetic waveform fixture";
    fs::write(&instance, bytes).unwrap();
    let expected = waveform_expectation();
    write_json(
        root.join("manifest.json"),
        &json!({
            "generator": {"name": "dicom-test-suite", "version": "test", "feature_flags": []},
            "run": {"seed": 1, "profile": "extended"},
            "standards": {"standards_lock_sha256": "1".repeat(64)},
            "files": [{
                "case_id": CASE_ID,
                "path": relative,
                "sha256": dicom_test_suite::sha256_hex(bytes),
                "dicom": {
                    "sop_class_uid": "1.2.840.10008.5.1.4.1.1.9.1.1",
                    "transfer_syntax_uid": "1.2.840.10008.1.2.1"
                },
                "image": null,
                "pixel_data": null,
                "expected_waveform": expected
            }]
        }),
    );
}

fn waveform_expectation() -> Value {
    let leads = [
        (
            "I",
            "2:1",
            "Lead I",
            "7b4aee068e05c2bdff3896937c78a4c7a32f9ed2bde64d91b1d925913bf29476",
        ),
        (
            "II",
            "2:2",
            "Lead II",
            "bd775dc70f76ea153a25832ad622b0cc26fbe6a37cf3ec6548a30965c4d17fba",
        ),
        (
            "III",
            "2:61",
            "Lead III",
            "19d26b694df281209aa1296abbfa8f7d360e24a03a091422aba6f67663e2f3b1",
        ),
        (
            "aVR",
            "2:62",
            "aVR, augmented voltage, right",
            "bb4c99d7857dbfcee5ee620bcff09b7060b61c5f2432427affc6139cb8d3cf9b",
        ),
        (
            "aVL",
            "2:63",
            "aVL, augmented voltage, left",
            "230f52ed2ac57624a9a35214d7867711008dd56014f4176ce258623e5b596d3a",
        ),
        (
            "aVF",
            "2:64",
            "aVF, augmented voltage, foot",
            "60e167db3c081ba5bca957aba820afb519b790d048b660634d49566df88105f2",
        ),
        (
            "V1",
            "2:3",
            "Lead V1",
            "cf8c73bebf746b799b1fe8aa2c908ca69bc7acc72311c64cbf4131fc8976609f",
        ),
        (
            "V2",
            "2:4",
            "Lead V2",
            "0f11e5fb5105dac699fa4bcfc01c79fbe696a81db04606f39a719de57b4c7c30",
        ),
        (
            "V3",
            "2:5",
            "Lead V3",
            "a41d5962abceb6dbe25f8421091ce3df6a69202c45b24ab6b0736159d15e253b",
        ),
        (
            "V4",
            "2:6",
            "Lead V4",
            "d655e2cbb23d70e229ed52fedba9c45573e22729fed0a794ab690df8d7f33804",
        ),
        (
            "V5",
            "2:7",
            "Lead V5",
            "005c539f9f4256a86d9e0a212b3bfe73741f99942b0677fb483c0c48db9583cd",
        ),
        (
            "V6",
            "2:8",
            "Lead V6",
            "f448df95acb226c5c992363e27707a42efc3ffb974ebeff38e2a81522b57d82c",
        ),
    ];
    let channels = leads
        .iter()
        .enumerate()
        .map(|(index, (label, code_value, code_meaning, _))| json!({
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
        }))
        .collect::<Vec<_>>();
    json!({
        "iod_kind": "twelve_lead_ecg",
        "sop_class_uid": "1.2.840.10008.5.1.4.1.1.9.1.1",
        "iod_name": "12-lead ECG Waveform",
        "modality": "ECG",
        "transfer_syntax_uid": "1.2.840.10008.1.2.1",
        "acquisition_context_items": 0,
        "multiplex_group": {
            "group_count": 1, "originality": "ORIGINAL", "label": "RESTING_12_LEAD",
            "channel_count": 12, "samples_per_channel": 500, "sampling_frequency_hz": 500,
            "duration_seconds": 1, "simultaneous_sampling": true
        },
        "channels": channels,
        "storage": {
            "bits_allocated": 16, "sample_interpretation": "SS", "data_vr": "OW",
            "byte_order": "little_endian", "interleave_order": "channel_then_sample",
            "payload_length_bytes": 12000,
            "payload_sha256": "98b7a9b1be25d9d64ffa75bc6e16ea80f60deed1891aeed8dfb440c1c19e6713",
            "channel_sha256": leads.iter().map(|lead| lead.3).collect::<Vec<_>>(),
            "sample_value_formula": "((s * (c + 1) * 37 + c * 101) mod 2001) - 1000",
            "sample_min": -1000, "sample_max": 1000,
            "waveform_padding_value_absent": true, "value_field_padding_bytes": 0
        },
        "absent_content": {
            "annotation_module": true, "synchronization_module": true, "references": true,
            "image": true, "pixel_data": true
        }
    })
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
