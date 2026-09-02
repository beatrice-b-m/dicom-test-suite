#![cfg(unix)]

use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use serde_json::json;
use synth_dicom_gen::composition::{
    CONTENT_PROVIDER_PROTOCOL_VERSION, ComposeError, ComposeOptions, ProviderError,
    ProviderInvocation, ProviderOutputDeclaration, ProviderRequest, compose,
    invoke_content_provider, provider_arguments_sha256,
};
use synth_dicom_gen::sha256_hex;

static NEXT: AtomicU64 = AtomicU64::new(0);

fn private_root(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "dts-composition-provider-{label}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ))
}

fn write_executable(path: &Path, script: &str) {
    fs::write(path, script).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
}

fn request() -> ProviderRequest {
    let bytes = b"abc";
    let mut request = ProviderRequest {
        protocol_version: CONTENT_PROVIDER_PROTOCOL_VERSION.into(),
        request_id: String::new(),
        provider_id: "fixture.provider".into(),
        expected_provider_version: "1.2.3".into(),
        argument_sha256: provider_arguments_sha256(&[]),
        instance_id: "primary".into(),
        template_id: "classic/secondary-capture/monochrome".into(),
        template_version: "1.0.0".into(),
        identities: BTreeMap::from([
            ("series_instance_uid#0".into(), "1.2.3.4".into()),
            ("sop_instance_uid#0".into(), "1.2.3.5".into()),
            ("study_instance_uid#0".into(), "1.2.3.6".into()),
        ]),
        output: ProviderOutputDeclaration {
            slot: "pixels".into(),
            size_bytes: bytes.len() as u64,
            sha256: sha256_hex(bytes),
            max_size_bytes: 16,
            media_type: Some("application/octet-stream".into()),
            pixel: None,
        },
        parameters: BTreeMap::new(),
        network_policy: "disabled".into(),
    };
    request.request_id = request.canonical_request_id();
    request
}

fn success_script(extra_output: bool) -> String {
    let extra = if extra_output {
        "printf 'undeclared' > \"$DTS_COMPOSITION_PROVIDER_OUTPUTS/extra.bin\""
    } else {
        ""
    };
    format!(
        r#"#!/bin/sh
request_id=$(/usr/bin/sed -n 's/.*"request_id": "\([^"]*\)".*/\1/p' "$DTS_COMPOSITION_PROVIDER_REQUEST")
argument_sha256=$(/usr/bin/sed -n 's/.*"argument_sha256": "\([^"]*\)".*/\1/p' "$DTS_COMPOSITION_PROVIDER_REQUEST")
printf 'abc' > "$DTS_COMPOSITION_PROVIDER_OUTPUTS/content.bin"
{extra}
printf '{{"protocol_version":"1.0.0","request_id":"%s","provider_id":"fixture.provider","provider_version":"1.2.3","executable_sha256":"%s","argument_sha256":"%s","output":{{"slot":"pixels","relative_path":"content.bin","size_bytes":3,"sha256":"{}"}}}}' "$request_id" "$1" "$argument_sha256" > "$DTS_COMPOSITION_PROVIDER_RESPONSE"
"#,
        sha256_hex(b"abc")
    )
}

#[test]
fn provider_binds_request_identity_and_audited_output() {
    let root = private_root("success");
    fs::create_dir(&root).unwrap();
    let executable = root.join("provider.sh");
    write_executable(&executable, &success_script(false));
    let executable_sha256 = sha256_hex(&fs::read(&executable).unwrap());
    let arguments = vec![executable_sha256.clone()];
    let mut request = request();
    request.argument_sha256 = provider_arguments_sha256(&arguments);
    request.request_id = request.canonical_request_id();
    let output = invoke_content_provider(
        &ProviderInvocation {
            executable,
            executable_sha256: executable_sha256.clone(),
            arguments,
            timeout: Duration::from_secs(2),
        },
        &request,
        &root.join("run"),
    )
    .unwrap();
    assert_eq!(fs::read(&output.path).unwrap(), b"abc");
    assert_eq!(output.executable_sha256, executable_sha256);
    assert_eq!(output.provider_version, "1.2.3");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn provider_rejects_undeclared_files_without_publication() {
    let root = private_root("undeclared");
    fs::create_dir(&root).unwrap();
    let executable = root.join("provider.sh");
    write_executable(&executable, &success_script(true));
    let executable_sha256 = sha256_hex(&fs::read(&executable).unwrap());
    let arguments = vec![executable_sha256.clone()];
    let mut request = request();
    request.argument_sha256 = provider_arguments_sha256(&arguments);
    request.request_id = request.canonical_request_id();
    let error = invoke_content_provider(
        &ProviderInvocation {
            executable,
            executable_sha256: executable_sha256.clone(),
            arguments,
            timeout: Duration::from_secs(2),
        },
        &request,
        &root.join("run"),
    )
    .unwrap_err();
    assert!(matches!(error, ProviderError::Invalid { .. }));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn provider_crash_and_hang_are_bounded() {
    let root = private_root("failures");
    fs::create_dir(&root).unwrap();
    let crash = root.join("crash.sh");
    write_executable(&crash, "#!/bin/sh\nexit 17\n");
    let crash_sha256 = sha256_hex(&fs::read(&crash).unwrap());
    assert!(matches!(
        invoke_content_provider(
            &ProviderInvocation {
                executable: crash,
                executable_sha256: crash_sha256,
                arguments: vec![],
                timeout: Duration::from_secs(1),
            },
            &request(),
            &root.join("crash-run"),
        ),
        Err(ProviderError::Invalid { .. })
    ));

    let hang = root.join("hang.sh");
    write_executable(&hang, "#!/bin/sh\n/bin/sleep 5\n");
    let hang_sha256 = sha256_hex(&fs::read(&hang).unwrap());
    assert!(matches!(
        invoke_content_provider(
            &ProviderInvocation {
                executable: hang,
                executable_sha256: hang_sha256,
                arguments: vec![],
                timeout: Duration::from_millis(30),
            },
            &request(),
            &root.join("hang-run"),
        ),
        Err(ProviderError::Timeout { .. })
    ));

    let flood = root.join("flood.sh");
    write_executable(
        &flood,
        "#!/bin/sh\n/usr/bin/yes x | /usr/bin/head -c 1048576 > \"$DTS_COMPOSITION_PROVIDER_OUTPUTS/content.bin\"\n/bin/sleep 5\n",
    );
    let flood_sha256 = sha256_hex(&fs::read(&flood).unwrap());
    assert!(matches!(
        invoke_content_provider(
            &ProviderInvocation {
                executable: flood,
                executable_sha256: flood_sha256,
                arguments: vec![],
                timeout: Duration::from_secs(2),
            },
            &request(),
            &root.join("flood-run"),
        ),
        Err(ProviderError::Invalid { .. })
    ));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn provider_rejects_malformed_mismatched_and_symlink_outputs() {
    let root = private_root("adversarial-protocol");
    fs::create_dir(&root).unwrap();

    let malformed = root.join("malformed.sh");
    write_executable(
        &malformed,
        "#!/bin/sh\nprintf 'abc' > \"$DTS_COMPOSITION_PROVIDER_OUTPUTS/content.bin\"\nprintf 'not-json' > \"$DTS_COMPOSITION_PROVIDER_RESPONSE\"\n",
    );
    let malformed_sha256 = sha256_hex(&fs::read(&malformed).unwrap());
    assert!(matches!(
        invoke_content_provider(
            &ProviderInvocation {
                executable: malformed,
                executable_sha256: malformed_sha256,
                arguments: vec![],
                timeout: Duration::from_secs(1),
            },
            &request(),
            &root.join("malformed-run"),
        ),
        Err(ProviderError::Parse { .. })
    ));

    let mismatch = root.join("mismatch.sh");
    let mismatch_script = success_script(false).replace("printf 'abc'", "printf 'abd'");
    write_executable(&mismatch, &mismatch_script);
    let mismatch_sha256 = sha256_hex(&fs::read(&mismatch).unwrap());
    let arguments = vec![mismatch_sha256.clone()];
    let mut mismatch_request = request();
    mismatch_request.argument_sha256 = provider_arguments_sha256(&arguments);
    mismatch_request.request_id = mismatch_request.canonical_request_id();
    assert!(matches!(
        invoke_content_provider(
            &ProviderInvocation {
                executable: mismatch,
                executable_sha256: mismatch_sha256,
                arguments,
                timeout: Duration::from_secs(1),
            },
            &mismatch_request,
            &root.join("mismatch-run"),
        ),
        Err(ProviderError::Invalid { .. })
    ));

    let symlink_output = root.join("symlink-output.sh");
    write_executable(
        &symlink_output,
        "#!/bin/sh\n/bin/ln -s /dev/null \"$DTS_COMPOSITION_PROVIDER_OUTPUTS/content.bin\"\n",
    );
    let symlink_output_sha256 = sha256_hex(&fs::read(&symlink_output).unwrap());
    assert!(matches!(
        invoke_content_provider(
            &ProviderInvocation {
                executable: symlink_output,
                executable_sha256: symlink_output_sha256,
                arguments: vec![],
                timeout: Duration::from_secs(1),
            },
            &request(),
            &root.join("symlink-output-run"),
        ),
        Err(ProviderError::Invalid { .. })
    ));

    let target = root.join("target.sh");
    write_executable(&target, "#!/bin/sh\nexit 0\n");
    let executable_link = root.join("executable-link.sh");
    std::os::unix::fs::symlink(&target, &executable_link).unwrap();
    assert!(matches!(
        invoke_content_provider(
            &ProviderInvocation {
                executable: executable_link,
                executable_sha256: sha256_hex(&fs::read(&target).unwrap()),
                arguments: vec![],
                timeout: Duration::from_secs(1),
            },
            &request(),
            &root.join("symlink-executable-run"),
        ),
        Err(ProviderError::Invalid { .. })
    ));
    fs::remove_dir_all(root).unwrap();
}

fn composition_script(payload_sha256: &str) -> String {
    format!(
        r#"#!/bin/sh
request_id=$(/usr/bin/sed -n 's/.*"request_id": "\([^"]*\)".*/\1/p' "$DTS_COMPOSITION_PROVIDER_REQUEST")
argument_sha256=$(/usr/bin/sed -n 's/.*"argument_sha256": "\([^"]*\)".*/\1/p' "$DTS_COMPOSITION_PROVIDER_REQUEST")
printf 'abcd' > "$DTS_COMPOSITION_PROVIDER_OUTPUTS/pixels.raw"
printf '{{"protocol_version":"1.0.0","request_id":"%s","provider_id":"fixture.provider","provider_version":"1.2.3","executable_sha256":"%s","argument_sha256":"%s","output":{{"slot":"pixels","relative_path":"pixels.raw","size_bytes":4,"sha256":"{payload_sha256}"}}}}' "$request_id" "$1" "$argument_sha256" > "$DTS_COMPOSITION_PROVIDER_RESPONSE"
"#
    )
}

fn provider_spec(executable: &Path, executable_sha256: &str) -> serde_json::Value {
    json!({
        "composition_spec_schema_version": "0.1.0",
        "instances": [{
            "instance_id": "primary",
            "template": {"id": "classic/secondary-capture/monochrome"},
            "content": [{
                "slot": "pixels",
                "source": {
                    "kind": "provider",
                    "provider_id": "fixture.provider",
                    "provider_version": "1.2.3",
                    "executable": executable,
                    "executable_sha256": executable_sha256,
                    "arguments": [executable_sha256],
                    "timeout_ms": 2000,
                    "size_bytes": 4,
                    "sha256": sha256_hex(b"abcd"),
                    "media_type": "application/octet-stream",
                    "pixel": {
                        "rows": 2,
                        "columns": 2,
                        "frames": 1,
                        "samples_per_pixel": 1,
                        "photometric_interpretation": "MONOCHROME2",
                        "sample_type": "uint",
                        "bits_allocated": 8,
                        "bits_stored": 8,
                        "high_bit": 7,
                        "byte_order": "little"
                    },
                    "parameters": {"fixture": "neutral-gradient"}
                }
            }]
        }]
    })
}

#[test]
fn compose_consumes_provider_content_and_records_full_provenance() {
    let root = private_root("compose-success");
    fs::create_dir(&root).unwrap();
    let executable = root.join("provider.sh");
    write_executable(&executable, &composition_script(&sha256_hex(b"abcd")));
    let executable_sha256 = sha256_hex(&fs::read(&executable).unwrap());
    let spec_path = root.join("spec.json");
    fs::write(
        &spec_path,
        serde_json::to_vec_pretty(&provider_spec(&executable, &executable_sha256)).unwrap(),
    )
    .unwrap();
    let out = root.join("out");
    let (_, manifest) = compose(&ComposeOptions {
        spec_path,
        out_dir: out.clone(),
        seed: 71,
        catalog_path: "templates/catalog.json".into(),
        dry_run: false,
    })
    .unwrap();
    let object = dicom_object::open_file(out.join("instances/primary.dcm")).unwrap();
    assert_eq!(
        object
            .element_by_name("PixelData")
            .unwrap()
            .to_bytes()
            .unwrap()
            .as_ref(),
        b"abcd"
    );
    let properties = &manifest["composition"]["entries"][0]["content"][0]["properties"];
    assert_eq!(properties["content_origin"], "provider");
    assert_eq!(properties["provider_id"], "fixture.provider");
    assert_eq!(properties["provider_version"], "1.2.3");
    assert_eq!(properties["provider_executable_sha256"], executable_sha256);
    assert_eq!(
        properties["provider_argument_sha256"],
        provider_arguments_sha256(&[executable_sha256.clone()])
    );
    assert_eq!(properties["provider_protocol_version"], "1.0.0");
    assert_eq!(properties["provider_network_policy"], "disabled");
    assert_eq!(properties["provider_resource_outcome"], "within_limits");
    assert_eq!(properties["provider_termination"], "exit_zero");
    assert!(properties["provider_request_sha256"].as_str().is_some());
    assert!(properties["provider_response_sha256"].as_str().is_some());
    assert!(!out.join(".providers").exists());
    assert!(!out.join(".assets").exists());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn compose_pipelines_provider_native_pixels_through_rle_without_planning_bytes() {
    let root = private_root("compose-provider-rle");
    fs::create_dir(&root).unwrap();
    let native = vec![0_u8; 512];
    let native_sha256 = sha256_hex(&native);
    let executable = root.join("provider.sh");
    write_executable(
        &executable,
        &format!(
            r#"#!/bin/sh
request_id=$(/usr/bin/sed -n 's/.*"request_id": "\([^"]*\)".*/\1/p' "$DTS_COMPOSITION_PROVIDER_REQUEST")
argument_sha256=$(/usr/bin/sed -n 's/.*"argument_sha256": "\([^"]*\)".*/\1/p' "$DTS_COMPOSITION_PROVIDER_REQUEST")
/usr/bin/head -c 512 /dev/zero > "$DTS_COMPOSITION_PROVIDER_OUTPUTS/pixels.raw"
printf '{{"protocol_version":"1.0.0","request_id":"%s","provider_id":"fixture.provider","provider_version":"1.2.3","executable_sha256":"%s","argument_sha256":"%s","output":{{"slot":"pixels","relative_path":"pixels.raw","size_bytes":512,"sha256":"{native_sha256}"}}}}' "$request_id" "$1" "$argument_sha256" > "$DTS_COMPOSITION_PROVIDER_RESPONSE"
"#
        ),
    );
    let executable_sha256 = sha256_hex(&fs::read(&executable).unwrap());

    let mut provider = provider_spec(&executable, &executable_sha256);
    provider["instances"][0]["template"]["id"] = json!("classic/xa");
    provider["instances"][0]["transfer_syntax_uid"] = json!("1.2.840.10008.1.2.5");
    provider["instances"][0]["content"][0]["source"]["size_bytes"] = json!(512);
    provider["instances"][0]["content"][0]["source"]["sha256"] = json!(native_sha256);
    provider["instances"][0]["content"][0]["source"]["pixel"] = json!({
        "rows": 16, "columns": 16, "frames": 1,
        "samples_per_pixel": 1, "photometric_interpretation": "MONOCHROME2",
        "sample_type": "uint", "bits_allocated": 16,
        "bits_stored": 12, "high_bit": 11, "byte_order": "little"
    });
    let provider_spec_path = root.join("provider.json");
    fs::write(
        &provider_spec_path,
        serde_json::to_vec_pretty(&provider).unwrap(),
    )
    .unwrap();
    let provider_out = root.join("provider-out");
    let (_, provider_manifest) = compose(&ComposeOptions {
        spec_path: provider_spec_path,
        out_dir: provider_out.clone(),
        seed: 91,
        catalog_path: "templates/catalog.json".into(),
        dry_run: false,
    })
    .unwrap();

    fs::write(root.join("pixels.raw"), &native).unwrap();
    let local = json!({
        "composition_spec_schema_version": "0.1.0",
        "instances": [{
            "instance_id": "primary",
            "template": {"id": "classic/xa"},
            "transfer_syntax_uid": "1.2.840.10008.1.2.5",
            "content": [{
                "slot": "pixels",
                "source": {
                    "kind": "local_file",
                    "path": "pixels.raw",
                    "sha256": sha256_hex(&native),
                    "pixel": {
                        "rows": 16, "columns": 16, "frames": 1,
                        "samples_per_pixel": 1,
                        "photometric_interpretation": "MONOCHROME2",
                        "sample_type": "uint", "bits_allocated": 16,
                        "bits_stored": 12, "high_bit": 11, "byte_order": "little"
                    }
                }
            }]
        }]
    });
    let local_spec_path = root.join("local.json");
    fs::write(&local_spec_path, serde_json::to_vec_pretty(&local).unwrap()).unwrap();
    let local_out = root.join("local-out");
    compose(&ComposeOptions {
        spec_path: local_spec_path,
        out_dir: local_out.clone(),
        seed: 91,
        catalog_path: "templates/catalog.json".into(),
        dry_run: false,
    })
    .unwrap();

    assert_eq!(
        fs::read(provider_out.join("instances/primary.dcm")).unwrap(),
        fs::read(local_out.join("instances/primary.dcm")).unwrap()
    );
    let properties = &provider_manifest["composition"]["entries"][0]["content"][0]["properties"];
    assert_eq!(properties["content_origin"], "provider");
    assert_eq!(properties["codec_backend_kind"], "native");
    assert_eq!(
        properties["codec_semantic_validation"],
        "decoded_frame_hashes_match"
    );
    assert!(!provider_out.join(".providers").exists());
    assert!(!provider_out.join(".composition-inputs").exists());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn compose_provider_crash_cannot_publish_a_partial_corpus() {
    let root = private_root("compose-crash");
    fs::create_dir(&root).unwrap();
    let executable = root.join("provider.sh");
    write_executable(&executable, "#!/bin/sh\nexit 19\n");
    let executable_sha256 = sha256_hex(&fs::read(&executable).unwrap());
    let spec_path = root.join("spec.json");
    fs::write(
        &spec_path,
        serde_json::to_vec_pretty(&provider_spec(&executable, &executable_sha256)).unwrap(),
    )
    .unwrap();
    let out = root.join("out");
    assert!(matches!(
        compose(&ComposeOptions {
            spec_path,
            out_dir: out.clone(),
            seed: 71,
            catalog_path: "templates/catalog.json".into(),
            dry_run: false,
        }),
        Err(ComposeError::Provider(ProviderError::Invalid { .. }))
    ));
    assert!(!out.exists());
    fs::remove_dir_all(root).unwrap();
}
