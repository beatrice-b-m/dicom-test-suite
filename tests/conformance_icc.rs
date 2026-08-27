#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

const ICC_CASE_ID: &str = "vl/photo/rgb_icc_profile_explicit_le";
const ICC_PROFILE_SHA256: &str = "8e069a3476b71a0e0ae7272d9278ba70540d1c4a0b19af1c7d52e56f49091fef";

#[test]
fn littlecms_transicc_collects_case_scoped_icc_evidence() {
    let fixture = IccFixture::new(true);
    let instance = &fixture.run["instances"][0];
    let icc = &instance["icc"];

    assert_eq!(instance["case_id"], ICC_CASE_ID);
    assert_eq!(icc["adapter_id"], "littlecms-transicc-icc");
    assert_eq!(icc["status"], "passed");
    assert_eq!(icc["independence"], "independent");

    let result = instance["results"]
        .as_array()
        .unwrap()
        .iter()
        .find(|result| result["role"] == "icc_validator")
        .expect("the ICC case must include a case-scoped validator result");
    assert_eq!(result["adapter_id"], "littlecms-transicc-icc");
    assert_eq!(result["status"], "completed");

    let relative = icc["evidence"]["path"]
        .as_str()
        .expect("passed ICC evidence requires a sidecar");
    let sidecar_bytes = fs::read(fixture.evidence.join(relative)).unwrap();
    assert_eq!(
        icc["evidence"]["sha256"],
        dicom_test_suite::sha256_hex(&sidecar_bytes)
    );
    let sidecar: Value = serde_json::from_slice(&sidecar_bytes).unwrap();
    assert_eq!(sidecar["adapter_id"], "littlecms-transicc-icc");
    assert_eq!(sidecar["extractor_adapter_id"], "dcmtk-dcmdump");
    assert_eq!(sidecar["source_profile_sha256"], ICC_PROFILE_SHA256);
    assert_eq!(sidecar["manifest_profile_sha256"], ICC_PROFILE_SHA256);
    assert_eq!(sidecar["dicom_color_space"], "SRGB");
    assert_eq!(sidecar["header"]["device_class"], "scnr");
    assert_eq!(sidecar["header"]["data_color_space"], "RGB ");
    assert_eq!(sidecar["header"]["profile_connection_space"], "XYZ ");
    assert_eq!(sidecar["header"]["signature"], "acsp");
    assert_eq!(sidecar["header"]["rendering_intent"], 0);
    assert_eq!(sidecar["tag_count"], 9);
    assert_eq!(
        sidecar["transforms"],
        json!([
            {"rgb": [255, 0, 0], "xyz": [43.6035, 22.2443, 1.3901]},
            {"rgb": [0, 255, 0], "xyz": [38.5101, 71.6934, 9.7076]},
            {"rgb": [0, 0, 255], "xyz": [14.3066, 6.0623, 71.3928]},
            {"rgb": [255, 255, 255], "xyz": [96.4203, 100.0, 82.4905]}
        ])
    );
}

#[test]
fn strict_verification_rejects_semantically_relinked_icc_sidecar() {
    let mut fixture = IccFixture::new(true);
    mark_tools_as_locked(&mut fixture.run);
    let relative = fixture.run["instances"][0]["icc"]["evidence"]["path"]
        .as_str()
        .unwrap()
        .to_string();
    let target = fixture.evidence.join(&relative);
    let mut sidecar: Value = serde_json::from_slice(&fs::read(&target).unwrap()).unwrap();

    sidecar["header"]["device_class"] = json!("mntr");
    let encoded = serde_json::to_vec_pretty(&sidecar).unwrap();
    fs::write(&target, &encoded).unwrap();
    fixture.run["instances"][0]["icc"]["evidence"]["sha256"] =
        json!(dicom_test_suite::sha256_hex(&encoded));
    write_run(&fixture.evidence, &fixture.run);

    let verified =
        dicom_test_suite::conformance::verify_conformance(&fixture.evidence, &fixture.allowlist)
            .unwrap();
    assert_eq!(verified["valid"], false);
    assert!(failures(&verified).iter().any(|failure| {
        failure.contains("ICC profile evidence sidecar is not linked")
            || failure.contains("ICC profile evidence") && failure.contains("device class")
    }));
}

#[test]
fn unavailable_icc_adapter_does_not_silently_pass() {
    let mut fixture = IccFixture::new(false);
    mark_tools_as_locked(&mut fixture.run);
    write_run(&fixture.evidence, &fixture.run);

    let instance = &fixture.run["instances"][0];
    assert_ne!(instance["icc"]["status"], "passed");
    let result = instance["results"]
        .as_array()
        .unwrap()
        .iter()
        .find(|result| result["role"] == "icc_validator")
        .expect("the unavailable adapter must still produce an explicit ICC result");
    assert_ne!(result["status"], "completed");

    let verified =
        dicom_test_suite::conformance::verify_conformance(&fixture.evidence, &fixture.allowlist)
            .unwrap();
    assert_eq!(verified["valid"], false);
    assert!(failures(&verified).iter().any(|failure| {
        failure.contains("independent ICC profile evidence failed")
            || failure.contains("ICC validation incomplete")
    }));
}

struct IccFixture {
    evidence: PathBuf,
    allowlist: PathBuf,
    run: Value,
}

impl IccFixture {
    fn new(available: bool) -> Self {
        let root = temp_dir();
        let generated = root.join("generated");
        let evidence = root.join("evidence");
        fs::create_dir_all(&generated).unwrap();

        let profile = embedded_icc_profile();
        assert_eq!(profile.len(), 736);
        assert_eq!(dicom_test_suite::sha256_hex(&profile), ICC_PROFILE_SHA256);
        let profile_source = root.join("profile.icc");
        fs::write(&profile_source, &profile).unwrap();

        let source = b"independent ICC tools own extraction and parsing";
        fs::write(generated.join("icc.dcm"), source).unwrap();
        let frame = [255_u8, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255];
        let manifest = json!({
            "run": {"seed": 1, "profile": "test"},
            "generator": {"name": "icc-fixture", "version": "1", "feature_flags": []},
            "standards": {"standards_lock_sha256": "0".repeat(64)},
            "files": [{
                "case_id": ICC_CASE_ID,
                "path": "icc.dcm",
                "sha256": dicom_test_suite::sha256_hex(source),
                "dicom": {
                    "sop_class_uid": "1.2.840.10008.5.1.4.1.1.77.1.4",
                    "transfer_syntax_uid": "1.2.840.10008.1.2.1"
                },
                "image": {
                    "rows": 2, "columns": 2, "frames": 1, "samples_per_pixel": 3,
                    "photometric_interpretation": "RGB", "planar_configuration": 0,
                    "bits_allocated": 8, "bits_stored": 8, "high_bit": 7,
                    "pixel_representation": 0
                },
                "pixel_data": {
                    "vr": "OB", "native_or_encapsulated": "native", "value_length": 12,
                    "frame_count": 1,
                    "frame_hashes": [dicom_test_suite::sha256_hex(&frame)]
                },
                "expected_icc_profile": {
                    "tag": "(0028,2000)", "vr": "OB",
                    "profile_sha256": ICC_PROFILE_SHA256,
                    "profile_size": 736, "declared_profile_size": 736,
                    "version": "2.1.0", "device_class": "scnr",
                    "data_color_space": "RGB ", "profile_connection_space": "XYZ ",
                    "signature": "acsp", "rendering_intent": "perceptual",
                    "rendering_intent_code": 0, "tag_count": 9,
                    "color_space": "SRGB", "description": "sRGB", "copyright": "CC0",
                    "source_profile": "DCMTK 3.7.0 DCMTK_SRGB_ICC_SAMPLE"
                }
            }]
        });
        fs::write(
            generated.join("manifest.json"),
            serde_json::to_vec_pretty(&manifest).unwrap(),
        )
        .unwrap();

        let primary = fake_tool(&root, "primary", "exit 0");
        let entity = fake_tool(&root, "entity", "exit 0");
        let extractor = fake_tool(
            &root,
            "dcmdump",
            &format!(
                "if [ \"$1\" = \"--version\" ]; then echo 'dcmdump 3.7.0'; exit 0; fi\nif [ \"$1\" = \"+W\" ]; then mkdir -p \"$2\"; cp '{}' \"$2/ICCProfile.raw\"; fi\nexit 0",
                profile_source.display()
            ),
        );
        let transicc = if available {
            fake_tool(
                &root,
                "transicc",
                "if [ \"$1\" = \"--version\" ]; then echo 'LittleCMS 2.19'; exit 0; fi\ncat >/dev/null\nprintf '43.6035 22.2443 1.3901\\n38.5101 71.6934 9.7076\\n14.3066 6.0623 71.3928\\n96.4203 100.0000 82.4905\\n'",
            )
        } else {
            root.join("transicc-unavailable")
        };
        let mut icc_adapter = adapter("littlecms-transicc-icc", "icc_validator", &transicc);
        icc_adapter["supported_case_ids"] = json!([ICC_CASE_ID]);
        icc_adapter["arguments"] = json!(["-n", "-i{profile}", "-o*XYZ", "-t0"]);
        let config = root.join("validators.json");
        fs::write(
            &config,
            serde_json::to_vec_pretty(&json!({
                "schema_version": "0.1.0",
                "adapters": [
                    adapter("primary", "primary_iod_validator", &primary),
                    adapter("entity", "entity_validator", &entity),
                    adapter("dcmtk-dcmdump", "independent_parser", &extractor),
                    icc_adapter
                ]
            }))
            .unwrap(),
        )
        .unwrap();
        let run =
            dicom_test_suite::conformance::run_conformance(&generated, &evidence, &config).unwrap();
        let allowlist = root.join("allowlist.json");
        fs::write(
            &allowlist,
            b"{\"schema_version\":\"0.1.0\",\"findings\":[]}",
        )
        .unwrap();
        Self {
            evidence,
            allowlist,
            run,
        }
    }
}

fn adapter(id: &str, role: &str, executable: &Path) -> Value {
    json!({
        "id": id,
        "role": role,
        "executable": executable,
        "arguments": [],
        "version_arguments": ["--version"],
        "timeout_seconds": 2,
        "required": true,
        "platforms": ["linux", "macos"],
        "capabilities": ["test"]
    })
}

fn fake_tool(root: &Path, name: &str, body: &str) -> PathBuf {
    let path = root.join(name);
    fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
    let mut permissions = fs::metadata(&path).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&path, permissions).unwrap();
    path
}

fn embedded_icc_profile() -> Vec<u8> {
    let compact = include_str!("../src/generator/native/dcmtk_srgb_input_profile.hex")
        .chars()
        .filter(|character| character.is_ascii_hexdigit())
        .collect::<String>();
    compact
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16)
                .expect("embedded ICC profile must be valid hexadecimal")
        })
        .collect()
}

fn mark_tools_as_locked(run: &mut Value) {
    for tool in run["tools"].as_array_mut().unwrap() {
        if tool["status"] == "available" {
            tool["lock_status"] = json!("matched");
        }
    }
}

fn write_run(evidence: &Path, run: &Value) {
    fs::write(
        evidence.join("conformance-run.json"),
        serde_json::to_vec_pretty(run).unwrap(),
    )
    .unwrap();
}

fn failures(report: &Value) -> Vec<&str> {
    report["failures"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(Value::as_str)
        .collect()
}

fn temp_dir() -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("dts-conformance-icc-{nonce}"));
    fs::create_dir_all(&root).unwrap();
    root
}
