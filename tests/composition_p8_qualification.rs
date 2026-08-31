use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use dicom_test_suite::composition::{
    ComposeOptions, TemplateCatalog, TemplateStatus, compose, composition_report,
    validate_composition_root,
};
use dicom_test_suite::sha256_hex;
use serde_json::{Value, json};

static NEXT: AtomicU64 = AtomicU64::new(0);

struct BackendOverride {
    previous: Option<OsString>,
}

impl BackendOverride {
    fn prepared() -> Self {
        let relative = Path::new("generation-backends/highdicom-pydicom/.venv/bin/python");
        assert!(
            relative.is_file(),
            "P8 full-catalog qualification requires the prepared locked backend"
        );
        let executable = std::env::current_dir().unwrap().join(relative);
        let previous = std::env::var_os("DTS_HIGHDICOM_PYTHON");
        // This integration-test binary runs one test, so no other thread can
        // observe the temporary explicit runtime selection.
        unsafe { std::env::set_var("DTS_HIGHDICOM_PYTHON", executable) };
        Self { previous }
    }
}

impl Drop for BackendOverride {
    fn drop(&mut self) {
        match self.previous.take() {
            Some(value) => unsafe { std::env::set_var("DTS_HIGHDICOM_PYTHON", value) },
            None => unsafe { std::env::remove_var("DTS_HIGHDICOM_PYTHON") },
        }
    }
}

fn workspace(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "dts-composition-p8-{label}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ))
}

fn write_full_catalog_spec(path: &Path, catalog: &TemplateCatalog, parallelism: u32) {
    let dependency_templates = catalog
        .templates
        .iter()
        .flat_map(|template| {
            template.default_bundle["dependencies"]
                .as_array()
                .into_iter()
                .flatten()
        })
        .filter_map(|dependency| dependency["template_id"].as_str())
        .collect::<BTreeSet<_>>();
    let instances = catalog
        .templates
        .iter()
        .filter(|template| !dependency_templates.contains(template.template_id.0.as_str()))
        .enumerate()
        .map(|(index, template)| {
            assert_eq!(template.status, TemplateStatus::Qualified);
            json!({
                "instance_id": format!("template-{index:03}"),
                "template": {
                    "id": template.template_id,
                    "version": template.template_version
                }
            })
        })
        .collect::<Vec<_>>();
    fs::write(
        path,
        serde_json::to_vec_pretty(&json!({
            "composition_spec_schema_version": "0.1.0",
            "parallelism": parallelism,
            "instances": instances
        }))
        .unwrap(),
    )
    .unwrap();
}

fn projection(manifest: &Value) -> BTreeMap<String, Value> {
    manifest["composition"]["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| {
            (
                entry["instance_id"].as_str().unwrap().to_string(),
                json!({
                    "template_id":entry["template_id"],
                    "uids":entry["uids"],
                    "sha256":entry["sha256"],
                    "resolved_plan_sha256":entry["resolved_plan_sha256"],
                    "content":entry["content"],
                    "references":entry["references"]
                }),
            )
        })
        .collect()
}

fn assert_independent_routes_are_accounted(catalog: &TemplateCatalog) {
    let evidence: Value =
        serde_json::from_slice(&fs::read("templates/qualification-evidence.json").unwrap())
            .unwrap();
    let accounted = evidence["independent_routes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|route| {
            assert_eq!(route["runtime_policy"], "pinned_or_explicitly_unavailable");
            for path in route["evidence_paths"].as_array().unwrap() {
                assert!(Path::new(path.as_str().unwrap()).is_file());
            }
            route["adapter_id"].as_str().unwrap()
        })
        .collect::<BTreeSet<_>>();
    let declared = catalog
        .templates
        .iter()
        .flat_map(|template| {
            template.validation["independent_routes"]
                .as_array()
                .into_iter()
                .flatten()
        })
        .map(|route| route["adapter_id"].as_str().unwrap())
        .collect::<BTreeSet<_>>();
    assert_eq!(accounted, declared);
}

#[test]
fn every_qualified_default_and_bundle_passes_p8_reproducibility_validation_and_report() {
    let _backend_override = BackendOverride::prepared();
    let root = workspace("full-catalog");
    fs::create_dir(&root).unwrap();
    let catalog = TemplateCatalog::load("templates/catalog.json").unwrap();
    assert_independent_routes_are_accounted(&catalog);
    assert!(!catalog.templates.is_empty());
    assert!(
        catalog
            .templates
            .iter()
            .all(|template| template.status == TemplateStatus::Qualified)
    );
    let sequential_spec = root.join("sequential.json");
    let parallel_spec = root.join("parallel.json");
    write_full_catalog_spec(&sequential_spec, &catalog, 1);
    write_full_catalog_spec(&parallel_spec, &catalog, 8);
    let sequential_out = root.join("sequential");
    let parallel_out = root.join("parallel");
    let (_, sequential) = compose(&ComposeOptions {
        spec_path: sequential_spec,
        out_dir: sequential_out.clone(),
        seed: 80,
        catalog_path: "templates/catalog.json".into(),
        dry_run: false,
    })
    .unwrap();
    let (_, parallel) = compose(&ComposeOptions {
        spec_path: parallel_spec,
        out_dir: parallel_out.clone(),
        seed: 80,
        catalog_path: "templates/catalog.json".into(),
        dry_run: false,
    })
    .unwrap();

    for (out, manifest) in [(&sequential_out, &sequential), (&parallel_out, &parallel)] {
        let (count, failures) = validate_composition_root(out, manifest);
        assert!(failures.is_empty(), "{failures:#?}");
        assert_eq!(
            count,
            manifest["composition"]["entries"].as_array().unwrap().len()
        );
        let report = composition_report(manifest);
        assert_eq!(report["report_kind"], "composition");
        assert_eq!(report["counts"]["instances"], count);
        assert!(!report.to_string().contains("case_id"));
        assert!(!report.to_string().contains("profile_membership"));
    }

    assert_eq!(projection(&sequential), projection(&parallel));
    for manifest in [&sequential, &parallel] {
        assert_eq!(
            manifest["run"]["corpus_plan_sha256"]
                .as_str()
                .unwrap()
                .len(),
            64
        );
    }
    let projection_sha256 = sha256_hex(
        &serde_json::to_vec(&projection(&sequential))
            .expect("full-catalog composition projection should serialize"),
    );
    assert_eq!(
        projection_sha256, "f77c3aba2f42dffbe1eb358f6cf83e26c437e84ff3a0f816ddda70affeec4ba1",
        "the terminal full-catalog plan, identity, content, and reference projection changed"
    );
    let observed_templates = sequential["composition"]["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["template_id"].as_str().unwrap())
        .collect::<BTreeSet<_>>();
    let expected_templates = catalog
        .templates
        .iter()
        .map(|template| template.template_id.0.as_str())
        .collect::<BTreeSet<_>>();
    assert_eq!(observed_templates, expected_templates);
    assert!(
        sequential["composition"]["entries"]
            .as_array()
            .unwrap()
            .iter()
            .all(|entry| entry["validation"]["status"] == "passed")
    );
    fs::remove_dir_all(root).unwrap();
}
