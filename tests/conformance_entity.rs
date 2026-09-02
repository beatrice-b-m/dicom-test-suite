#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

#[test]
fn entity_adapter_uses_file_list_and_normalizes_consistency_findings() {
    let root = temp_dir("entity with spaces");
    let generated = root.join("generated root");
    generate_smoke(&generated);
    let primary = fake_tool(&root, "primary", "exit 0");
    let entity = fake_tool(
        &root,
        "entity",
        "if [ \"$1\" = \"--version\" ]; then echo entity-1; exit 0; fi\n\
         test \"$1\" = \"-f\" || exit 8\n\
         test \"$(wc -l < \"$2\" | tr -d ' ')\" = \"3\" || exit 9\n\
         grep -q 'generated root' \"$2\" || exit 10\n\
         echo 'Error - SeriesInstanceUID reused for different StudyInstanceUID'\n\
         echo 'Warning - PatientName inconsistent for PatientID' >&2",
    );
    let config = root.join("validators.json");
    fs::write(
        &config,
        serde_json::to_vec(&json!({
            "schema_version": "0.1.0",
            "adapters": [
                adapter("primary", "primary_iod_validator", &primary),
                adapter("entity", "entity_validator", &entity)
            ]
        }))
        .unwrap(),
    )
    .unwrap();
    let evidence_root = root.join("evidence");
    let output = Command::new(env!("CARGO_BIN_EXE_synth-dicom-gen"))
        .args(["conformance", "run"])
        .arg(&generated)
        .args(["--out"])
        .arg(&evidence_root)
        .args(["--config"])
        .arg(&config)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let evidence: Value =
        serde_json::from_slice(&fs::read(evidence_root.join("conformance-run.json")).unwrap())
            .unwrap();
    assert_eq!(evidence["entity"]["status"], "completed");
    assert_eq!(evidence["entity"]["findings"][0]["severity"], "error");
    assert_eq!(evidence["entity"]["findings"][1]["severity"], "warning");
    assert!(evidence_root.join("entity/files.txt").is_file());
    assert_eq!(
        evidence["entity"]["stdout"]["sha256"],
        synth_dicom_gen::sha256_hex(
            &fs::read(evidence_root.join("entity/dcentvfy.stdout")).unwrap()
        )
    );
}

#[test]
fn u32_entity_projection_is_byte_preserving_hash_linked_and_tamper_evident() {
    let root = temp_dir("u32 entity projection");
    let generated = root.join("generated");
    let (source_path, source_bytes, pixel_payload) = generate_u32(&generated);
    let default_primary = fake_tool(&root, "default-primary", "exit 0");
    let u32_primary = fake_tool(
        &root,
        "u32-primary",
        &format!(
            "if [ \"$1\" = \"--pixel-u32\" ]; then printf '%s\\n' '{}'; fi\nexit 0",
            pixel_payload
        ),
    );
    let entity = fake_tool(
        &root,
        "entity-projection",
        "if [ \"$1\" = \"--version\" ]; then echo entity-1; exit 0; fi
         test \"$1\" = \"-f\" || exit 8
         test \"$(wc -l < \"$2\" | tr -d ' ')\" = \"1\" || exit 9
         grep -q 'entity/projections/.*projected.dcm' \"$2\" || exit 10
         exit 0",
    );
    let parser = fake_tool(&root, "parser", "exit 0");
    let mut specialized = adapter(
        "pydicom-dicom-validator-u32",
        "primary_iod_validator",
        &u32_primary,
    );
    specialized["supported_case_ids"] = json!(["classic/sc/mono2_u32_explicit_le"]);
    specialized["pixel_arguments"] = json!(["--pixel-u32", "{input}"]);
    let config = root.join("validators.json");
    fs::write(
        &config,
        serde_json::to_vec_pretty(&json!({
            "schema_version": "0.1.0",
            "adapters": [
                adapter("default", "primary_iod_validator", &default_primary),
                specialized,
                adapter("entity", "entity_validator", &entity),
                adapter("parser", "independent_parser", &parser)
            ]
        }))
        .unwrap(),
    )
    .unwrap();
    let evidence_root = root.join("evidence");
    let output = Command::new(env!("CARGO_BIN_EXE_synth-dicom-gen"))
        .args(["conformance", "run"])
        .arg(&generated)
        .args(["--out"])
        .arg(&evidence_root)
        .args(["--config"])
        .arg(&config)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let mut evidence: Value =
        serde_json::from_slice(&fs::read(evidence_root.join("conformance-run.json")).unwrap())
            .unwrap();
    assert_eq!(evidence["entity"]["status"], "completed");
    assert_eq!(
        evidence["entity"]["input_projection"]["method"],
        "terminal_pixel_data_element_redaction_v1"
    );
    let entry = &evidence["entity"]["input_projection"]["entries"][0];
    let source_copy = evidence_root.join(entry["source_copy"]["path"].as_str().unwrap());
    let projected = evidence_root.join(entry["projected_input"]["path"].as_str().unwrap());
    let element_offset = entry["removed_element"]["element_offset"].as_u64().unwrap() as usize;
    assert_eq!(fs::read(&source_copy).unwrap(), source_bytes);
    assert_eq!(
        fs::read(&projected).unwrap(),
        source_bytes[..element_offset]
    );
    assert_eq!(fs::read(&source_path).unwrap(), source_bytes);

    for tool in evidence["tools"].as_array_mut().unwrap() {
        tool["lock_status"] = json!("matched");
    }
    fs::write(
        evidence_root.join("conformance-run.json"),
        serde_json::to_vec_pretty(&evidence).unwrap(),
    )
    .unwrap();
    let allowlist = root.join("allowlist.json");
    fs::write(
        &allowlist,
        b"{\"schema_version\":\"0.1.0\",\"findings\":[]}",
    )
    .unwrap();
    let verified =
        synth_dicom_gen::conformance::verify_conformance(&evidence_root, &allowlist).unwrap();
    assert_eq!(verified["valid"], true, "{verified:#}");

    let mut tampered = fs::read(&projected).unwrap();
    tampered.push(0);
    fs::write(&projected, &tampered).unwrap();
    evidence["entity"]["input_projection"]["entries"][0]["projected_input"]["sha256"] =
        json!(synth_dicom_gen::sha256_hex(&tampered));
    fs::write(
        evidence_root.join("conformance-run.json"),
        serde_json::to_vec_pretty(&evidence).unwrap(),
    )
    .unwrap();
    let verified =
        synth_dicom_gen::conformance::verify_conformance(&evidence_root, &allowlist).unwrap();
    assert_eq!(verified["valid"], false);
    assert!(
        verified["failures"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(Value::as_str)
            .any(|failure| failure.contains("byte-preserving"))
    );
}

#[test]
fn u32_entity_projection_rejects_ineligible_manifest_shape() {
    let root = temp_dir("u32 entity rejection");
    let generated = root.join("generated");
    generate_u32(&generated);
    let manifest_path = generated.join("manifest.json");
    let mut manifest: Value = serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
    manifest["files"][0]["image"]["bits_stored"] = json!(31);
    fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();
    let primary = fake_tool(&root, "primary", "exit 0");
    let entity = fake_tool(&root, "entity", "exit 0");
    let config = root.join("validators.json");
    fs::write(
        &config,
        serde_json::to_vec_pretty(&json!({
            "schema_version": "0.1.0",
            "adapters": [
                adapter("primary", "primary_iod_validator", &primary),
                adapter("entity", "entity_validator", &entity)
            ]
        }))
        .unwrap(),
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_synth-dicom-gen"))
        .args(["conformance", "run"])
        .arg(&generated)
        .args(["--out"])
        .arg(root.join("evidence"))
        .args(["--config"])
        .arg(&config)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("manifest eligibility fields do not match the locked case")
    );
}

fn adapter(id: &str, role: &str, path: &Path) -> Value {
    json!({
        "id": id,
        "role": role,
        "executable": path,
        "arguments": [],
        "version_arguments": ["--version"],
        "timeout_seconds": 2,
        "required": true,
        "platforms": ["macos"],
        "capabilities": ["test"]
    })
}

fn generate_smoke(root: &Path) {
    let output = Command::new(env!("CARGO_BIN_EXE_synth-dicom-gen"))
        .args(["generate", "--profile", "smoke", "--out"])
        .arg(root)
        .output()
        .unwrap();
    assert!(output.status.success());
}

fn generate_u32(root: &Path) -> (PathBuf, Vec<u8>, Value) {
    let relative = Path::new("classic/sc/mono2_u32_explicit_le/instance.dcm");
    let source_path = root.join(relative);
    fs::create_dir_all(source_path.parent().unwrap()).unwrap();
    let pixel_bytes = [
        0x00, 0x00, 0x00, 0x00, 0xff, 0xff, 0x00, 0x00, 0x00, 0x00, 0x00, 0x80, 0xff, 0xff, 0xff,
        0xff,
    ];
    let mut source = b"byte-preserved entity metadata".to_vec();
    source.extend_from_slice(&[
        0xe0, 0x7f, 0x10, 0x00, b'O', b'W', 0x00, 0x00, 0x10, 0x00, 0x00, 0x00,
    ]);
    source.extend_from_slice(&pixel_bytes);
    fs::write(&source_path, &source).unwrap();
    let pixel_hash = synth_dicom_gen::sha256_hex(&pixel_bytes);
    let manifest = json!({
        "run": { "seed": 1, "profile": "test" },
        "generator": { "name": "u32-entity-fixture", "version": "1", "feature_flags": [] },
        "standards": { "standards_lock_sha256": "0".repeat(64) },
        "files": [{
            "case_id": "classic/sc/mono2_u32_explicit_le",
            "path": relative.to_str().unwrap(),
            "sha256": synth_dicom_gen::sha256_hex(&source),
            "dicom": {
                "sop_class_uid": "1.2.840.10008.5.1.4.1.1.7",
                "transfer_syntax_uid": "1.2.840.10008.1.2.1"
            },
            "image": {
                "rows": 2, "columns": 2, "frames": 1, "samples_per_pixel": 1,
                "photometric_interpretation": "MONOCHROME2", "bits_allocated": 32,
                "bits_stored": 32, "high_bit": 31, "pixel_representation": 0
            },
            "pixel_data": {
                "vr": "OW", "native_or_encapsulated": "native", "value_length": 16,
                "frame_count": 1, "frame_hashes": [pixel_hash]
            },
            "expected_u32_pixels": {
                "pixel_data_sha256": pixel_hash,
                "stored_values": [0_u64, 65_535, 2_147_483_648_u64, 4_294_967_295_u64]
            }
        }]
    });
    fs::write(
        root.join("manifest.json"),
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();
    let payload = json!({
        "adapter_id": "pydicom-dicom-validator-u32",
        "bits_allocated": 32, "bits_stored": 32, "byte_order": "little_endian",
        "columns": 2, "frame_hashes": [pixel_hash], "frames": 1, "high_bit": 31,
        "photometric_interpretation": "MONOCHROME2", "pixel_data_sha256": pixel_hash,
        "pixel_data_vr": "OW", "pixel_representation": 0, "rows": 2,
        "samples_per_pixel": 1,
        "stored_values": [0_u64, 65_535, 2_147_483_648_u64, 4_294_967_295_u64],
        "transfer_syntax_uid": "1.2.840.10008.1.2.1"
    });
    (source_path, source, payload)
}

fn fake_tool(root: &Path, name: &str, body: &str) -> PathBuf {
    let path = root.join(name);
    fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
    let mut permissions = fs::metadata(&path).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&path, permissions).unwrap();
    path
}

fn temp_dir(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("synth-dicom-gen-{label}-{nonce}"));
    fs::create_dir_all(&root).unwrap();
    root
}
