use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::Value;
use synth_dicom_gen::build_coverage_report;
use synth_dicom_gen::composition::TemplateCatalog;
use synth_dicom_gen::recipes::{ClassicProjectionFamily, RecipeCatalog};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

struct TempRoot(PathBuf);

impl TempRoot {
    fn absent(label: &str) -> Self {
        let parent = std::env::temp_dir()
            .canonicalize()
            .expect("temporary directory should resolve without a symlink component");
        Self(parent.join(format!(
            "dicom-test-suite-u4-qualification-{label}-{}-{}",
            std::process::id(),
            TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
        )))
    }
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn generate(profile: &str, root: &Path) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_synth-dicom-gen"))
        .args([
            "generate",
            "--profile",
            profile,
            "--out",
            root.to_str()
                .expect("temporary output path should be UTF-8"),
            "--seed",
            "7",
        ])
        .output()
        .expect("documented generate command should start");
    assert!(
        output.status.success(),
        "generate --profile {profile} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&fs::read(root.join("manifest.json")).unwrap()).unwrap()
}

fn registry_statuses() -> BTreeMap<String, String> {
    let registry: Value =
        serde_json::from_slice(&fs::read("cases/registry.json").unwrap()).unwrap();
    registry["cases"]
        .as_array()
        .unwrap()
        .iter()
        .map(|case| {
            (
                case["case_id"].as_str().unwrap().to_owned(),
                case["status"].as_str().unwrap().to_owned(),
            )
        })
        .collect()
}

fn report_value_matches(manifest: &Value, report: &Value, manifest_pointer: &str, field: &str) {
    if let Some(expected) = manifest.pointer(manifest_pointer) {
        assert_eq!(
            report.get(field),
            Some(expected),
            "report field {field} lost manifest axis {manifest_pointer} for {}",
            manifest["path"].as_str().unwrap()
        );
    }
}

fn report_axis_matches(
    manifest: &Value,
    report: &Value,
    manifest_pointer: &str,
    field: &str,
) -> bool {
    manifest
        .pointer(manifest_pointer)
        .is_none_or(|expected| report.get(field) == Some(expected))
}

#[test]
fn every_implemented_classic_artifact_has_independent_and_report_evidence() {
    let catalog = RecipeCatalog::load(
        "cases/recipes",
        "cases/registry.json",
        "templates/catalog.json",
    )
    .unwrap();
    let templates = TemplateCatalog::load("templates/catalog.json").unwrap();
    let statuses = registry_statuses();

    let mut expected = BTreeMap::new();
    for recipe in catalog.recipes().values().filter(|recipe| {
        recipe.plan_provider_id == "native.classic_plan"
            && statuses.get(&recipe.binding.case_id).map(String::as_str) == Some("implemented")
    }) {
        let artifacts = &recipe
            .dicom
            .as_ref()
            .expect("classic recipes are DICOM recipes")
            .artifacts;
        for artifact in artifacts {
            let path = artifact
                .output
                .path
                .as_ref()
                .expect("implemented classic artifact has a declared output path")
                .clone();
            let projection = artifact
                .classic_projection
                .as_ref()
                .expect("implemented classic artifact has typed projection facts");
            let template_ref = artifact
                .template
                .as_ref()
                .expect("implemented classic artifact has a qualified template");
            let template = templates
                .templates
                .iter()
                .find(|candidate| {
                    candidate.template_id.0 == template_ref.template_id
                        && candidate.template_version.to_string() == template_ref.template_version
                })
                .expect("classic artifact template should resolve");
            let routes = template.validation["independent_routes"]
                .as_array()
                .expect("qualified classic template declares independent routes");
            assert!(!routes.is_empty(), "{path} has no independent route");
            assert!(routes.iter().all(|route| {
                route["adapter_id"]
                    .as_str()
                    .is_some_and(|id| !id.is_empty())
                    && route["kind"].as_str().is_some_and(|kind| !kind.is_empty())
                    && route["required_for_qualification"] == true
            }));
            assert!(
                expected
                    .insert(
                        path,
                        (
                            recipe.binding.case_id.clone(),
                            recipe.recipe_id.clone(),
                            projection.family.clone(),
                            projection.expected_capabilities.clone(),
                            routes
                                .iter()
                                .map(|route| route["adapter_id"].as_str().unwrap().to_owned())
                                .collect::<BTreeSet<_>>(),
                        ),
                    )
                    .is_none(),
                "classic artifact paths must be unique"
            );
        }
    }
    assert!(
        !expected.is_empty(),
        "catalog contains no implemented classic artifacts"
    );

    let all_root = TempRoot::absent("all");
    let runs = [(
        generate("all", &all_root.0),
        build_coverage_report(&all_root.0).unwrap(),
    )];

    let mut represented = BTreeSet::new();
    for (manifest, report) in &runs {
        let rows = report["coverage_matrix"].as_array().unwrap();
        for file in manifest["files"].as_array().unwrap() {
            let Some(path) = file["path"].as_str() else {
                continue;
            };
            let Some((case_id, recipe_id, family, expected_capabilities, route_ids)) =
                expected.get(path)
            else {
                continue;
            };
            assert_eq!(file["case_id"], case_id.as_str());
            assert_eq!(file["recipe"]["recipe_id"], recipe_id.as_str());
            assert_eq!(file["validation"]["status"], "passed");
            assert!(
                file["validation"]["internal"]
                    .as_array()
                    .is_some_and(|checks| !checks.is_empty()
                        && checks.iter().all(|check| check["status"] == "passed")),
                "{path} lacks passing specialized internal validation"
            );
            assert_eq!(
                file["expected_capabilities"],
                serde_json::json!(expected_capabilities)
            );
            assert!(!route_ids.is_empty());

            let matching_rows = rows
                .iter()
                .filter(|row| row["case_id"] == file["case_id"] && row["status"] == "generated")
                .collect::<Vec<_>>();
            assert!(
                !matching_rows.is_empty(),
                "{path} is absent from coverage report"
            );
            let row = matching_rows
                .into_iter()
                .find(|row| {
                    row["sop_class_uid"] == file["dicom"]["sop_class_uid"]
                        && row["transfer_syntax"] == file["dicom"]["transfer_syntax_uid"]
                        && row["geometry"]["rows"] == file["image"]["rows"]
                        && row["geometry"]["columns"] == file["image"]["columns"]
                        && report_axis_matches(
                            file,
                            row,
                            "/expected_geometry/position_along_normal_mm",
                            "geometry_position_along_normal_mm",
                        )
                        && report_axis_matches(
                            file,
                            row,
                            "/expected_geometry/instance_number",
                            "geometry_instance_number",
                        )
                })
                .expect("classic report row should retain its DICOM identity axes");
            assert_eq!(row["validation_status"], "passed");
            assert_eq!(row["profile_membership"], file["profile_membership"]);
            assert_eq!(row["known_stressors"], file["known_stressors"]);
            assert_eq!(row["determinism"], file["determinism"]);
            assert_eq!(row["modality"], file["dicom"]["modality"]);
            assert_eq!(row["image_type"], file["expected_semantics"]["image_type"]);

            report_value_matches(
                file,
                row,
                "/expected_geometry/sort_basis",
                "geometry_sort_basis",
            );
            report_value_matches(
                file,
                row,
                "/expected_series_organization/group_id",
                "series_organization_group_id",
            );
            report_value_matches(file, row, "/recipe/recipe_parameters/kvp", "kvp");
            report_value_matches(
                file,
                row,
                "/recipe/recipe_parameters/imager_pixel_spacing",
                "imager_pixel_spacing",
            );
            report_value_matches(
                file,
                row,
                "/recipe/recipe_parameters/mr/scanning_sequence",
                "mr_scanning_sequence",
            );
            report_value_matches(file, row, "/expected_pet_activity/units", "pet_units");
            report_value_matches(
                file,
                row,
                "/expected_vl_single_frame/laterality",
                "laterality",
            );

            match family {
                ClassicProjectionFamily::Ct => {
                    assert!(row["kvp"] != Value::Null || row["geometry_sort_basis"] != Value::Null)
                }
                ClassicProjectionFamily::DxMg => {
                    assert!(row["imager_pixel_spacing"] != Value::Null)
                }
                ClassicProjectionFamily::MrCr => assert!(
                    row["mr_scanning_sequence"] != Value::Null || row["image_type"] != Value::Null
                ),
                ClassicProjectionFamily::Nuclear => assert!(
                    row["us_frame_count"] != Value::Null
                        || row["nm_frame_increment_pointers"] != Value::Null
                        || row["pet_units"] != Value::Null
                        || row["image_type"] != Value::Null,
                    "{path} lacks a nuclear-family report axis"
                ),
                ClassicProjectionFamily::VlProjection => assert!(
                    row["laterality"] != Value::Null
                        || row["xa_image_type"] != Value::Null
                        || row["xrf_image_type"] != Value::Null
                        || row["image_type"] != Value::Null
                ),
            }
            represented.insert(path.to_owned());
        }
    }
    assert_eq!(
        represented,
        expected.keys().cloned().collect(),
        "the documented all profile must represent the complete implemented classic inventory"
    );
}

#[test]
fn migrated_classic_symbols_are_plan_first_and_fail_closed() {
    let catalog = RecipeCatalog::load(
        "cases/recipes",
        "cases/registry.json",
        "templates/catalog.json",
    )
    .unwrap();
    assert!(
        catalog
            .recipes()
            .values()
            .any(|recipe| recipe.plan_provider_id == "native.classic_plan")
    );
    assert!(
        !Path::new("src/generator.rs").exists(),
        "the manual classic dispatcher and family writers must be deleted"
    );
}
