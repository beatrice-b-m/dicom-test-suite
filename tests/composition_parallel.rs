use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use synth_dicom_gen::composition::{ComposeOptions, compose};
use serde_json::{Value, json};

static NEXT: AtomicU64 = AtomicU64::new(0);

fn root(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "dts-composition-parallel-{label}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ))
}

fn run(workspace: &PathBuf, parallelism: u32, label: &str) -> (PathBuf, Value) {
    let instances = (0..12)
        .map(|index| {
            json!({
                "instance_id": format!("instance-{index:02}"),
                "template": {"id":"classic/secondary-capture/monochrome"}
            })
        })
        .collect::<Vec<_>>();
    let spec = workspace.join(format!("{label}.json"));
    fs::write(
        &spec,
        serde_json::to_vec_pretty(&json!({
            "composition_spec_schema_version":"0.1.0",
            "parallelism": parallelism,
            "instances": instances
        }))
        .unwrap(),
    )
    .unwrap();
    let out = workspace.join(label);
    let (_, manifest) = compose(&ComposeOptions {
        spec_path: spec,
        out_dir: out.clone(),
        seed: 84,
        catalog_path: "templates/catalog.json".into(),
        dry_run: false,
    })
    .unwrap();
    (out, manifest)
}

#[test]
fn sequential_and_parallel_materialization_have_identical_canonical_outputs() {
    let workspace = root("equivalence");
    fs::create_dir(&workspace).unwrap();
    let (sequential, sequential_manifest) = run(&workspace, 1, "sequential");
    let (parallel, parallel_manifest) = run(&workspace, 4, "parallel");

    assert_eq!(sequential_manifest["run"]["parallelism"]["used"], 1);
    assert_eq!(parallel_manifest["run"]["parallelism"]["requested"], 4);
    assert_eq!(parallel_manifest["run"]["parallelism"]["used"], 4);
    assert_eq!(
        sequential_manifest["composition"],
        parallel_manifest["composition"]
    );
    for index in 0..12 {
        let relative = format!("instances/instance-{index:02}.dcm");
        assert_eq!(
            fs::read(sequential.join(&relative)).unwrap(),
            fs::read(parallel.join(&relative)).unwrap()
        );
    }
    fs::remove_dir_all(workspace).unwrap();
}
