use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

#[cfg(unix)]
#[test]
fn dcmtk_dcmdump_matches_native_float32_little_endian_frame_hashes() {
    let fixture = FloatFixture::new(false);
    let instance = &fixture.run["instances"][0];
    assert_eq!(instance["pixel"]["status"], "passed");
    assert_eq!(instance["pixel"]["independence"], "independent");
    assert_eq!(
        instance["pixel"]["actual_frame_hashes"],
        fixture.expected_hashes
    );

    let sidecar_path = instance["pixel"]["evidence"]["path"].as_str().unwrap();
    let sidecar: Value =
        serde_json::from_slice(&fs::read(fixture.evidence.join(sidecar_path)).unwrap()).unwrap();
    assert_eq!(sidecar["source_element"], "(7FE0,0008) Float Pixel Data");
    assert_eq!(sidecar["byte_order"], "little_endian");
    assert_eq!(
        sidecar["extraction_method"],
        "dcmdump_full_float_values_reconstructed_as_ieee754_binary32"
    );
    assert_eq!(sidecar["extracted_value_count"], 4);
}

#[cfg(unix)]
#[test]
fn strict_verification_rejects_native_float32_hash_mismatch() {
    let mut fixture = FloatFixture::new(true);
    assert_eq!(fixture.run["instances"][0]["pixel"]["status"], "failed");
    for tool in fixture.run["tools"].as_array_mut().unwrap() {
        tool["lock_status"] = json!("matched");
    }
    fs::write(
        fixture.evidence.join("conformance-run.json"),
        serde_json::to_vec_pretty(&fixture.run).unwrap(),
    )
    .unwrap();
    let result =
        dicom_test_suite::conformance::verify_conformance(&fixture.evidence, &fixture.allowlist)
            .unwrap();
    assert_eq!(result["valid"], false);
    assert!(
        result["failures"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(Value::as_str)
            .any(|failure| failure.contains("independent native float32 pixel evidence failed"))
    );
}

#[test]
fn real_dcmtk_rle_adapter_matches_all_manifest_frame_hashes_when_enabled() {
    if std::env::var("DTS_REAL_CONFORMANCE").as_deref() != Ok("1") {
        return;
    }
    for command in ["dcmdump", "dcmdrle"] {
        assert!(
            Command::new(command).arg("--version").status().is_ok(),
            "{command} must be installed"
        );
    }
    let root = temp_dir();
    let generated = root.join("generated");
    let evidence = root.join("evidence");
    assert!(
        Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
            .args(["generate", "--profile", "all", "--out"])
            .arg(&generated)
            .status()
            .unwrap()
            .success()
    );
    assert!(
        Command::new(env!("CARGO_BIN_EXE_dicom-test-suite"))
            .args(["conformance", "run"])
            .arg(&generated)
            .args(["--out"])
            .arg(&evidence)
            .status()
            .unwrap()
            .success()
    );
    let run: Value =
        serde_json::from_slice(&fs::read(evidence.join("conformance-run.json")).unwrap()).unwrap();
    let rle = run["instances"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|instance| instance["transfer_syntax_uid"] == "1.2.840.10008.1.2.5")
        .collect::<Vec<_>>();
    assert!(!rle.is_empty());
    assert!(
        rle.iter()
            .all(|instance| instance["pixel"]["status"] == "passed")
    );
}

fn temp_dir() -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("dts-real-pixel-{nonce}"));
    fs::create_dir_all(&root).unwrap();
    root
}

#[cfg(unix)]
struct FloatFixture {
    evidence: PathBuf,
    allowlist: PathBuf,
    run: Value,
    expected_hashes: Value,
}

#[cfg(unix)]
impl FloatFixture {
    fn new(mismatch: bool) -> Self {
        let root = temp_dir();
        let generated = root.join("generated");
        let evidence = root.join("evidence");
        fs::create_dir_all(&generated).unwrap();
        fs::write(
            generated.join("float.dcm"),
            b"independent tools own parsing",
        )
        .unwrap();

        let raw_bytes = [
            0x00, 0x00, 0x80, 0x3f, 0x00, 0x00, 0x20, 0xc0, // 1.0, -2.5
            0x00, 0x00, 0x00, 0x3f, 0x00, 0x00, 0x28, 0x42, // 0.5, 42.0
        ];
        let expected_hashes = json!([
            dicom_test_suite::sha256_hex(&raw_bytes[..8]),
            dicom_test_suite::sha256_hex(&raw_bytes[8..])
        ]);
        let mut manifest_hashes = expected_hashes.clone();
        if mismatch {
            manifest_hashes[1] = json!("0".repeat(64));
        }
        let manifest = json!({
            "run": { "seed": 1, "profile": "test" },
            "generator": { "name": "float-fixture", "version": "1", "feature_flags": [] },
            "standards": { "standards_lock_sha256": "0".repeat(64) },
            "files": [{
                "case_id": "derived/parametric-map/float32_fixture",
                "path": "float.dcm",
                "dicom": {
                    "sop_class_uid": "1.2.840.10008.5.1.4.1.1.30",
                    "transfer_syntax_uid": "1.2.840.10008.1.2.1"
                },
                "image": {
                    "sample_type": "float32", "rows": 1, "columns": 2, "frames": 2,
                    "samples_per_pixel": 1, "photometric_interpretation": "MONOCHROME2",
                    "bits_allocated": 32, "planar_configuration": null
                },
                "pixel_data": {
                    "vr": "OF", "native_or_encapsulated": "native", "value_length": 16,
                    "frame_count": 2, "frame_hashes": manifest_hashes
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
        let parser = fake_tool(
            &root,
            "dcmdump",
            "if [ \"$1\" = \"+L\" ]; then printf '%s\\n' '(7fe0,0008) OF 1\\-2.5\\0.5\\42 # 16, 4 FloatPixelData'; fi\nexit 0",
        );
        let config = root.join("validators.json");
        fs::write(
            &config,
            serde_json::to_vec_pretty(&json!({
                "schema_version": "0.1.0",
                "adapters": [
                    adapter("primary", "primary_iod_validator", &primary),
                    adapter("entity", "entity_validator", &entity),
                    adapter("dcmtk-dcmdump", "independent_parser", &parser)
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
            expected_hashes,
        }
    }
}

#[cfg(unix)]
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

#[cfg(unix)]
fn fake_tool(root: &Path, name: &str, body: &str) -> PathBuf {
    let path = root.join(name);
    fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
    let mut permissions = fs::metadata(&path).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&path, permissions).unwrap();
    path
}
