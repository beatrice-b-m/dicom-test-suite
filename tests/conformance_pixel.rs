use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
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

#[cfg(unix)]
#[test]
fn pydicom_u32_adapter_matches_unsigned_values_metadata_and_frame_hash() {
    let fixture = U32Fixture::new(false);
    let repeat = U32Fixture::new(false);
    let pixel = &fixture.run["instances"][0]["pixel"];
    assert_eq!(pixel["status"], "passed");
    assert_eq!(pixel["independence"], "independent");
    assert_eq!(pixel["actual_frame_hashes"], fixture.expected_hashes);

    let sidecar_path = pixel["evidence"]["path"].as_str().unwrap();
    let sidecar: Value =
        serde_json::from_slice(&fs::read(fixture.evidence.join(sidecar_path)).unwrap()).unwrap();
    assert_eq!(sidecar["adapter_id"], "pydicom-dicom-validator-u32");
    assert_eq!(
        sidecar["actual"]["stored_values"],
        json!([0_u64, 65_535, 2_147_483_648_u64, 4_294_967_295_u64])
    );
    assert_eq!(sidecar["actual"]["pixel_data_vr"], "OW");
    assert_eq!(sidecar["actual"]["byte_order"], "little_endian");
    let repeat_path = repeat.run["instances"][0]["pixel"]["evidence"]["path"]
        .as_str()
        .unwrap();
    assert_eq!(
        fs::read(fixture.evidence.join(sidecar_path)).unwrap(),
        fs::read(repeat.evidence.join(repeat_path)).unwrap(),
        "u32 pixel evidence must not depend on temporary paths"
    );
}

#[cfg(unix)]
#[test]
fn strict_verification_rejects_native_u32_hash_mismatch() {
    let mut fixture = U32Fixture::new(true);
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
            .any(|failure| failure.contains("independent native u32 pixel evidence failed"))
    );
}

#[cfg(unix)]
#[test]
fn strict_verification_rejects_semantically_relinked_u32_sidecar() {
    let mut fixture = U32Fixture::new(false);
    for tool in fixture.run["tools"].as_array_mut().unwrap() {
        tool["lock_status"] = json!("matched");
    }
    let relative = fixture.run["instances"][0]["pixel"]["evidence"]["path"]
        .as_str()
        .unwrap()
        .to_string();
    let target = fixture.evidence.join(&relative);
    let mut sidecar: Value = serde_json::from_slice(&fs::read(&target).unwrap()).unwrap();
    sidecar["actual"]["bits_stored"] = json!(31);
    let encoded = serde_json::to_vec_pretty(&sidecar).unwrap();
    fs::write(&target, &encoded).unwrap();
    fixture.run["instances"][0]["pixel"]["evidence"]["sha256"] =
        json!(dicom_test_suite::sha256_hex(&encoded));
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
            .any(|failure| failure.contains("sidecar is not linked"))
    );
}

#[cfg(unix)]
#[test]
fn dcmtk_u1_adapter_matches_continuous_frames_and_raw_payload() {
    let fixture = U1Fixture::new(false);
    let pixel = &fixture.run["instances"][0]["pixel"];
    assert_eq!(pixel["status"], "passed");
    assert_eq!(pixel["actual_frame_hashes"], fixture.expected_hashes);
    let relative = pixel["evidence"]["path"].as_str().unwrap();
    let sidecar: Value =
        serde_json::from_slice(&fs::read(fixture.evidence.join(relative)).unwrap()).unwrap();
    assert_eq!(sidecar["adapter_id"], "dcmtk-dcm2img-u1");
    assert_eq!(sidecar["source_pixel_data_sha256"], fixture.pixel_hash);
    assert_eq!(
        sidecar["decoded_values"],
        json!([1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0])
    );
}

#[cfg(unix)]
#[test]
fn strict_verification_rejects_semantically_relinked_u1_sidecar() {
    let mut fixture = U1Fixture::new(false);
    for tool in fixture.run["tools"].as_array_mut().unwrap() {
        tool["lock_status"] = json!("matched");
    }
    let relative = fixture.run["instances"][0]["pixel"]["evidence"]["path"]
        .as_str()
        .unwrap()
        .to_string();
    let target = fixture.evidence.join(&relative);
    let mut sidecar: Value = serde_json::from_slice(&fs::read(&target).unwrap()).unwrap();
    sidecar["decoded_values"][9] = json!(1);
    let encoded = serde_json::to_vec_pretty(&sidecar).unwrap();
    fs::write(&target, &encoded).unwrap();
    fixture.run["instances"][0]["pixel"]["evidence"]["sha256"] =
        json!(dicom_test_suite::sha256_hex(&encoded));
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
            .any(|failure| {
                failure.as_str().is_some_and(|failure| {
                    failure.contains("u1 pixel evidence sidecar is not linked")
                })
            })
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
    static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let sequence = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "dts-real-pixel-{}-{nonce}-{sequence}",
        std::process::id()
    ));
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
struct U32Fixture {
    evidence: PathBuf,
    allowlist: PathBuf,
    run: Value,
    expected_hashes: Value,
}

#[cfg(unix)]
struct U1Fixture {
    evidence: PathBuf,
    allowlist: PathBuf,
    run: Value,
    expected_hashes: Value,
    pixel_hash: String,
}

#[cfg(unix)]
impl U1Fixture {
    fn new(mismatch: bool) -> Self {
        let root = temp_dir();
        let generated = root.join("generated");
        let evidence = root.join("evidence");
        fs::create_dir_all(&generated).unwrap();
        let source = b"u1 fixture bytes";
        fs::write(generated.join("u1.dcm"), source).unwrap();
        let frame_one = [1_u8, 0, 1, 0, 1, 0, 1, 0, 1];
        let frame_two = [0_u8, 1, 0, 1, 0, 1, 0, 1, 0];
        let expected_hashes = json!([
            dicom_test_suite::sha256_hex(&frame_one),
            if mismatch {
                "0".repeat(64)
            } else {
                dicom_test_suite::sha256_hex(&frame_two)
            }
        ]);
        let pixel_bytes = [0x55_u8, 0x55, 0x01, 0x00];
        let pixel_hash = dicom_test_suite::sha256_hex(&pixel_bytes);
        let manifest = json!({
            "run": {"seed": 1, "profile": "test"},
            "generator": {"name": "u1-fixture", "version": "1", "feature_flags": []},
            "standards": {"standards_lock_sha256": "0".repeat(64)},
            "files": [{
                "case_id": "classic/sc/mono2_u1_native",
                "path": "u1.dcm",
                "sha256": dicom_test_suite::sha256_hex(source),
                "dicom": {
                    "sop_class_uid": "1.2.840.10008.5.1.4.1.1.7.1",
                    "transfer_syntax_uid": "1.2.840.10008.1.2.1"
                },
                "image": {"rows": 3, "columns": 3, "frames": 2},
                "pixel_data": {"frame_hashes": expected_hashes},
                "expected_u1_pixels": {
                    "packing_order": "least_significant_bit_first",
                    "frame_boundary_policy": "continuous_without_per_frame_padding",
                    "stored_values": [1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0],
                    "decoded_frame_sha256": [
                        dicom_test_suite::sha256_hex(&frame_one),
                        dicom_test_suite::sha256_hex(&frame_two)
                    ],
                    "pixel_data_sha256": pixel_hash
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
            "if [ \"$1\" = \"+W\" ]; then printf '\\125\\125\\001\\000' > \"$2/pixel.raw\"; fi\nexit 0",
        );
        let decoder = fake_tool(
            &root,
            "dcm2img",
            "if [ \"$1\" = \"--version\" ]; then printf 'fake dcm2img 1\\n'; exit 0; fi\nfor output; do :; done\nprintf 'P2\\n3 3\\n1\\n1 0 1 0 1 0 1 0 1\\n' > \"${output}.f1.pgm\"\nprintf 'P2\\n3 3\\n1\\n0 1 0 1 0 1 0 1 0\\n' > \"${output}.f2.pgm\"\nexit 0",
        );
        let mut decoder_adapter = adapter("dcmtk-dcm2img-u1", "pixel_decoder", &decoder);
        decoder_adapter["arguments"] = json!(["{input}", "{output}"]);
        decoder_adapter["supported_case_ids"] = json!(["classic/sc/mono2_u1_native"]);
        let config = root.join("validators.json");
        fs::write(
            &config,
            serde_json::to_vec_pretty(&json!({
                "schema_version": "0.1.0",
                "adapters": [
                    adapter("primary", "primary_iod_validator", &primary),
                    adapter("entity", "entity_validator", &entity),
                    adapter("dcmtk-dcmdump", "independent_parser", &parser),
                    decoder_adapter
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
            pixel_hash,
        }
    }
}

#[cfg(unix)]
impl U32Fixture {
    fn new(mismatch: bool) -> Self {
        let root = temp_dir();
        let generated = root.join("generated");
        let evidence = root.join("evidence");
        fs::create_dir_all(&generated).unwrap();

        let raw_bytes = [
            0x00, 0x00, 0x00, 0x00, 0xff, 0xff, 0x00, 0x00, 0x00, 0x00, 0x00, 0x80, 0xff, 0xff,
            0xff, 0xff,
        ];
        let mut source = b"independent adapter owns parsing".to_vec();
        source.extend_from_slice(&[
            0xe0, 0x7f, 0x10, 0x00, b'O', b'W', 0x00, 0x00, 0x10, 0x00, 0x00, 0x00,
        ]);
        source.extend_from_slice(&raw_bytes);
        fs::write(generated.join("u32.dcm"), &source).unwrap();
        let pixel_hash = dicom_test_suite::sha256_hex(&raw_bytes);
        let expected_hashes = json!([pixel_hash]);
        let manifest = json!({
            "run": { "seed": 1, "profile": "test" },
            "generator": { "name": "u32-fixture", "version": "1", "feature_flags": [] },
            "standards": { "standards_lock_sha256": "0".repeat(64) },
            "files": [{
                "case_id": "classic/sc/mono2_u32_explicit_le",
                "path": "u32.dcm",
                "sha256": dicom_test_suite::sha256_hex(&source),
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
                    "frame_count": 1, "frame_hashes": expected_hashes
                },
                "expected_u32_pixels": {
                    "pixel_data_sha256": pixel_hash,
                    "stored_values": [0_u64, 65_535, 2_147_483_648_u64, 4_294_967_295_u64]
                }
            }]
        });
        fs::write(
            generated.join("manifest.json"),
            serde_json::to_vec_pretty(&manifest).unwrap(),
        )
        .unwrap();

        let default_primary = fake_tool(&root, "default-primary", "exit 0");
        let actual_frame_hash = if mismatch {
            "0".repeat(64)
        } else {
            pixel_hash.clone()
        };
        let u32_payload = json!({
            "adapter_id": "pydicom-dicom-validator-u32",
            "bits_allocated": 32, "bits_stored": 32, "byte_order": "little_endian",
            "columns": 2, "frame_hashes": [actual_frame_hash], "frames": 1, "high_bit": 31,
            "photometric_interpretation": "MONOCHROME2", "pixel_data_sha256": pixel_hash,
            "pixel_data_vr": "OW", "pixel_representation": 0, "rows": 2,
            "samples_per_pixel": 1,
            "stored_values": [0_u64, 65_535, 2_147_483_648_u64, 4_294_967_295_u64],
            "transfer_syntax_uid": "1.2.840.10008.1.2.1"
        });
        let specialized = fake_tool(
            &root,
            "u32-adapter",
            &format!(
                "if [ \"$1\" = \"--pixel-u32\" ]; then printf '%s\\n' '{}'; fi\nexit 0",
                u32_payload
            ),
        );
        let entity = fake_tool(&root, "entity", "exit 0");
        let parser = fake_tool(&root, "parser", "exit 0");
        let mut specialized_adapter = adapter(
            "pydicom-dicom-validator-u32",
            "primary_iod_validator",
            &specialized,
        );
        specialized_adapter["supported_case_ids"] = json!(["classic/sc/mono2_u32_explicit_le"]);
        specialized_adapter["pixel_arguments"] = json!(["--pixel-u32", "{input}"]);
        let config = root.join("validators.json");
        fs::write(
            &config,
            serde_json::to_vec_pretty(&json!({
                "schema_version": "0.1.0",
                "adapters": [
                    adapter("default", "primary_iod_validator", &default_primary),
                    specialized_adapter,
                    adapter("entity", "entity_validator", &entity),
                    adapter("parser", "independent_parser", &parser)
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
