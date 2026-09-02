#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use synth_dicom_gen::composition::{
    BundleError, ComposeCancellationToken, ComposeError, ComposeOptions, ContentError,
    RawContentError, SpecError, compose, compose_with_cancellation,
};
use synth_dicom_gen::sha256_hex;

static NEXT: AtomicU64 = AtomicU64::new(0);

fn workspace(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "dts-composition-resources-{label}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ))
}

fn write_json(path: &Path, value: &Value) {
    fs::write(path, serde_json::to_vec_pretty(value).unwrap()).unwrap();
}

fn options(spec_path: PathBuf, out_dir: PathBuf) -> ComposeOptions {
    ComposeOptions {
        spec_path,
        out_dir,
        seed: 76,
        catalog_path: "templates/catalog.json".into(),
        dry_run: false,
    }
}

fn assert_no_private_staging(root: &Path) {
    assert!(fs::read_dir(root).unwrap().all(|entry| {
        !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .contains(".synth-dicom-gen-staging-")
    }));
}

#[test]
fn resource_envelopes_fail_transactionally_and_clean_staging() {
    let root = workspace("limits");
    fs::create_dir(&root).unwrap();
    let instance_limited = root.join("instance-limit.json");
    write_json(
        &instance_limited,
        &json!({
            "composition_spec_schema_version":"0.1.0",
            "resource_limits":{"max_instances":1},
            "instances":[
                {"instance_id":"one","template":{"id":"classic/secondary-capture/monochrome"}},
                {"instance_id":"two","template":{"id":"classic/secondary-capture/monochrome"}}
            ]
        }),
    );
    let instance_out = root.join("instance-out");
    assert!(matches!(
        compose(&options(instance_limited, instance_out.clone())),
        Err(ComposeError::Spec(SpecError::InstanceLimit { .. }))
    ));
    assert!(!instance_out.exists());

    let policy_limited = root.join("policy-limit.json");
    write_json(
        &policy_limited,
        &json!({
            "composition_spec_schema_version":"0.1.0",
            "resource_limits":{"max_file_bytes":1073741825_u64},
            "instances":[
                {"instance_id":"one","template":{"id":"classic/secondary-capture/monochrome"}}
            ]
        }),
    );
    let policy_out = root.join("policy-out");
    assert!(matches!(
        compose(&options(policy_limited, policy_out.clone())),
        Err(ComposeError::Spec(SpecError::ResourceLimitAbovePolicy {
            name: "max_file_bytes",
            ..
        }))
    ));
    assert!(!policy_out.exists());

    let output_limited = root.join("output-limit.json");
    write_json(
        &output_limited,
        &json!({
            "composition_spec_schema_version":"0.1.0",
            "resource_limits":{"max_total_output_bytes":1},
            "instances":[
                {"instance_id":"one","template":{"id":"classic/secondary-capture/monochrome"}}
            ]
        }),
    );
    let output_out = root.join("output-out");
    assert!(matches!(
        compose(&options(output_limited, output_out.clone())),
        Err(ComposeError::OutputLimit { .. })
    ));
    assert!(!output_out.exists());

    let baseline_spec = root.join("baseline.json");
    write_json(
        &baseline_spec,
        &json!({
            "composition_spec_schema_version":"0.1.0",
            "instances":[
                {"instance_id":"one","template":{"id":"classic/secondary-capture/monochrome"}}
            ]
        }),
    );
    let baseline_out = root.join("baseline-out");
    let (baseline, _) = compose(&options(baseline_spec, baseline_out.clone())).unwrap();
    fs::remove_dir_all(baseline_out).unwrap();
    let manifest_limited = root.join("manifest-limit.json");
    write_json(
        &manifest_limited,
        &json!({
            "composition_spec_schema_version":"0.1.0",
            "resource_limits":{"max_total_output_bytes":baseline.output_bytes},
            "instances":[
                {"instance_id":"one","template":{"id":"classic/secondary-capture/monochrome"}}
            ]
        }),
    );
    let manifest_out = root.join("manifest-out");
    assert!(matches!(
        compose(&options(manifest_limited, manifest_out.clone())),
        Err(ComposeError::OutputLimit { size, limit }) if size > limit
    ));
    assert!(!manifest_out.exists());

    let bundle_limited = root.join("bundle-limit.json");
    write_json(
        &bundle_limited,
        &json!({
            "composition_spec_schema_version":"0.1.0",
            "resource_limits":{"max_instances":1},
            "instances":[
                {"instance_id":"registration","template":{"id":"derived/registration/spatial"}}
            ]
        }),
    );
    let bundle_out = root.join("bundle-out");
    let bundle_result = compose(&options(bundle_limited, bundle_out.clone()));
    assert!(
        matches!(
            bundle_result,
            Err(ComposeError::Bundle(BundleError::InstanceLimit { count, limit: 1 })) if count > 1
        ),
        "unexpected bundle limit result: {bundle_result:?}"
    );
    assert!(!bundle_out.exists());

    let pixels = vec![0_u8; 16 * 16 * 2];
    fs::write(root.join("pixels.raw"), &pixels).unwrap();
    let file_limited = root.join("file-limit.json");
    let content = json!([{"slot":"pixels","source":{
        "kind":"local_file", "path":"pixels.raw", "sha256":sha256_hex(&pixels),
        "pixel":{"rows":16,"columns":16,"frames":1,"samples_per_pixel":1,
            "photometric_interpretation":"MONOCHROME2","sample_type":"uint",
            "bits_allocated":16,"bits_stored":12,"high_bit":11,"byte_order":"little"}
    }}]);
    write_json(
        &file_limited,
        &json!({
            "composition_spec_schema_version":"0.1.0",
            "resource_limits":{"max_input_files":1},
            "instances":[
                {"instance_id":"one","template":{"id":"classic/cr"},"content":content},
                {"instance_id":"two","template":{"id":"classic/cr"},"content":content}
            ]
        }),
    );
    let file_out = root.join("file-out");
    let file_result = compose(&options(file_limited, file_out.clone()));
    assert!(
        matches!(
            file_result,
            Err(ComposeError::RawContent(RawContentError::Content(
                ContentError::FileCountLimit { .. }
            )))
        ),
        "unexpected file limit result: {file_result:?}"
    );
    assert!(!file_out.exists());
    assert_no_private_staging(&root);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn cancellation_terminates_a_provider_and_publishes_nothing() {
    let root = workspace("cancel-provider");
    fs::create_dir(&root).unwrap();
    let executable = root.join("provider.sh");
    fs::write(
        &executable,
        "#!/bin/sh\nprintf 'started' > \"$1\"\n/bin/sleep 30\n",
    )
    .unwrap();
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
    let executable_sha256 = sha256_hex(&fs::read(&executable).unwrap());
    let marker = root.join("provider-started");
    let spec_path = root.join("spec.json");
    write_json(
        &spec_path,
        &json!({
            "composition_spec_schema_version":"0.1.0",
            "instances":[{"instance_id":"one","template":{"id":"classic/secondary-capture/monochrome"},
                "content":[{"slot":"pixels","source":{
                    "kind":"provider","provider_id":"cancel.fixture","provider_version":"1.0.0",
                    "executable":executable,"executable_sha256":executable_sha256,
                    "arguments":[marker],"timeout_ms":300000,"size_bytes":4,
                    "sha256":sha256_hex(b"abcd"),
                    "pixel":{"rows":2,"columns":2,"frames":1,"samples_per_pixel":1,
                        "photometric_interpretation":"MONOCHROME2","sample_type":"uint",
                        "bits_allocated":8,"bits_stored":8,"high_bit":7,"byte_order":"little"}
                }}]
            }]
        }),
    );
    let out = root.join("out");
    let cancellation = ComposeCancellationToken::new();
    let worker_token = cancellation.clone();
    let worker_options = options(spec_path, out.clone());
    let worker = thread::spawn(move || compose_with_cancellation(&worker_options, &worker_token));
    let wait_deadline = Instant::now() + Duration::from_secs(3);
    while !marker.exists() && Instant::now() < wait_deadline {
        thread::sleep(Duration::from_millis(10));
    }
    assert!(marker.exists(), "provider must start before cancellation");
    let cancelled_at = Instant::now();
    cancellation.cancel();
    assert!(matches!(
        worker.join().unwrap(),
        Err(ComposeError::Cancelled)
    ));
    assert!(cancelled_at.elapsed() < Duration::from_secs(2));
    assert!(!out.exists());
    assert_no_private_staging(&root);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn large_parallel_corpus_has_stable_order_identities_and_hashes() {
    let root = workspace("large-corpus");
    fs::create_dir(&root).unwrap();
    let instances = (0..128)
        .map(|index| {
            json!({
                "instance_id":format!("instance-{index:04}"),
                "template":{"id":"classic/secondary-capture/monochrome"}
            })
        })
        .collect::<Vec<_>>();
    let sequential_spec = root.join("sequential.json");
    let parallel_spec = root.join("parallel.json");
    write_json(
        &sequential_spec,
        &json!({"composition_spec_schema_version":"0.1.0","parallelism":1,"instances":instances}),
    );
    write_json(
        &parallel_spec,
        &json!({"composition_spec_schema_version":"0.1.0","parallelism":8,"instances":instances}),
    );
    let sequential_out = root.join("sequential");
    let parallel_out = root.join("parallel");
    let (_, sequential) = compose(&options(sequential_spec, sequential_out)).unwrap();
    let (_, parallel) = compose(&options(parallel_spec, parallel_out)).unwrap();
    let projection = |manifest: &Value| {
        manifest["composition"]["entries"]
            .as_array()
            .unwrap()
            .iter()
            .map(|entry| {
                (
                    entry["instance_id"].as_str().unwrap().to_string(),
                    entry["uids"].clone(),
                    entry["sha256"].as_str().unwrap().to_string(),
                    entry["resolved_plan_sha256"].as_str().unwrap().to_string(),
                )
            })
            .collect::<Vec<_>>()
    };
    let sequential_projection = projection(&sequential);
    assert_eq!(sequential_projection, projection(&parallel));
    assert_eq!(sequential_projection.len(), 128);
    assert!(
        sequential_projection
            .windows(2)
            .all(|pair| pair[0].0 < pair[1].0)
    );
    fs::remove_dir_all(root).unwrap();
}
