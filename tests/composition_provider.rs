#![cfg(unix)]

use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use dicom_test_suite::composition::{
    CONTENT_PROVIDER_PROTOCOL_VERSION, ProviderError, ProviderInvocation,
    ProviderOutputDeclaration, ProviderRequest, invoke_content_provider,
};
use dicom_test_suite::sha256_hex;

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
printf 'abc' > "$DTS_COMPOSITION_PROVIDER_OUTPUTS/content.bin"
{extra}
printf '{{"protocol_version":"1.0.0","request_id":"%s","provider_id":"fixture.provider","provider_version":"1.2.3","executable_sha256":"%s","output":{{"slot":"pixels","relative_path":"content.bin","size_bytes":3,"sha256":"{}"}}}}' "$request_id" "$1" > "$DTS_COMPOSITION_PROVIDER_RESPONSE"
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
    let output = invoke_content_provider(
        &ProviderInvocation {
            executable,
            executable_sha256: executable_sha256.clone(),
            arguments: vec![executable_sha256.clone()],
            timeout: Duration::from_secs(2),
        },
        &request(),
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
    let error = invoke_content_provider(
        &ProviderInvocation {
            executable,
            executable_sha256: executable_sha256.clone(),
            arguments: vec![executable_sha256],
            timeout: Duration::from_secs(2),
        },
        &request(),
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
    fs::remove_dir_all(root).unwrap();
}
