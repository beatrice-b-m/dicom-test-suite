use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::Value;
use synth_dicom_gen::codecs::{FrameEncodeInput, FrameEncoder, NativeRleLosslessEncoder};
use synth_dicom_gen::composition::{
    ContentMaterialization, DicomVr, Part10Materializer, TemplateCatalog,
};
use synth_dicom_gen::encapsulation::{BasicOffsetTablePolicy, EncapsulatedPixelData};
use synth_dicom_gen::recipes::classic_nuclear::{
    ClassicNuclearArtifactParameters, ClassicNuclearPlanError, plan_nuclear_recipe,
};
use synth_dicom_gen::recipes::{
    CLASSIC_PIXEL_SLOT, ClassicResolvedPlanInput, OrderedSeriesProvider, RecipeCatalog,
    resolved_classic_instance_plan,
};
use synth_dicom_gen::{GenerateOptions, prepare_generation_run, sha256_hex, write_generation_run};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

fn temp_path(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "dicom-test-suite-classic-nuclear-{label}-{}-{}",
        std::process::id(),
        TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
    ))
}

fn load() -> (RecipeCatalog, TemplateCatalog, String) {
    (
        RecipeCatalog::load(
            "cases/recipes",
            "cases/registry.json",
            "templates/catalog.json",
        )
        .unwrap(),
        TemplateCatalog::load("templates/catalog.json").unwrap(),
        sha256_hex(&fs::read("standards.lock.json").unwrap()),
    )
}

fn owned<'a>(catalog: &'a RecipeCatalog) -> Vec<&'a synth_dicom_gen::recipes::CaseRecipe> {
    let mut recipes = catalog
        .recipes()
        .values()
        .filter(|recipe| {
            recipe.dicom.as_ref().is_some_and(|dicom| {
                dicom.artifacts.iter().any(|artifact| {
                    artifact.algorithm_provider_id.as_deref() == Some("algorithm.classic_nuclear")
                })
            })
        })
        .collect::<Vec<_>>();
    recipes.sort_by_key(|recipe| recipe.planning_order);
    recipes
}

fn make_rle_content(
    plan: &mut synth_dicom_gen::composition::ResolvedInstancePlan,
    native: &[u8],
    rows: u32,
    columns: u32,
    bits_allocated: u16,
) {
    let encoded = NativeRleLosslessEncoder::new()
        .encode_frame(FrameEncodeInput {
            native_frame: native,
            rows: u16::try_from(rows).unwrap(),
            columns: u16::try_from(columns).unwrap(),
            samples_per_pixel: 1,
            bits_allocated,
            bits_stored: bits_allocated,
            photometric_interpretation: "MONOCHROME2",
        })
        .unwrap()
        .bytes;
    let encapsulated = EncapsulatedPixelData::one_fragment_per_frame(
        std::slice::from_ref(&encoded),
        BasicOffsetTablePolicy::Populated,
    )
    .unwrap();
    let content = &mut plan.content[0];
    content.kind = "encapsulated_pixels".into();
    content.vr = DicomVr::OB;
    content.size_bytes = encoded.len() as u64;
    content.sha256 = sha256_hex(&encoded);
    content.materialization = Some(ContentMaterialization::Encapsulated {
        basic_offset_table: encapsulated.basic_offset_table.offsets,
        fragments: encapsulated.fragment_payloads,
    });
}

#[test]
fn nuclear_catalog_owns_exact_historical_slice() {
    let (catalog, _, _) = load();
    let recipes = owned(&catalog);
    assert_eq!(
        recipes
            .iter()
            .map(|recipe| recipe.binding.case_id.as_str())
            .collect::<Vec<_>>(),
        [
            "classic/nm/multiframe_explicit_le",
            "classic/pet/rescaled_activity_explicit_le",
            "classic/us/multiframe_explicit_le",
            "classic/us/mono2_u8_explicit_le",
            "classic/us/mono2_u8_rle_lossless",
        ]
    );
    assert_eq!(
        recipes
            .iter()
            .map(|recipe| recipe.planning_order.unwrap())
            .collect::<Vec<_>>(),
        (400..=404).collect::<Vec<_>>()
    );
    for recipe in recipes {
        assert_eq!(recipe.plan_provider_id, "native.classic_plan");
        let artifacts = &recipe.dicom.as_ref().unwrap().artifacts;
        assert_eq!(artifacts.len(), 1);
        let artifact = &artifacts[0];
        assert_eq!(artifact.order, 0);
        assert_eq!(artifact.content.provider_id, "content.native_pixels");
        assert!(artifact.output.path.is_some());
        assert_eq!(
            artifact.algorithm_provider_id.as_deref(),
            Some("algorithm.classic_nuclear")
        );
        assert!(artifact.attribute_operations.is_empty());
    }
}

#[test]
fn nuclear_planning_is_output_free_strict_and_excludes_enhanced_pet() {
    let (catalog, _, lock_hash) = load();
    // Every historical nuclear family retains its existing plan route.
    for historical in owned(&catalog) {
        assert!(
            plan_nuclear_recipe(historical, &lock_hash, 7)
                .unwrap()
                .is_some()
        );
    }
    let pet = catalog
        .recipes()
        .values()
        .find(|r| r.binding.case_id == "classic/pet/rescaled_activity_explicit_le")
        .unwrap();
    for name in [
        "caller/activity",
        "classic/us/mono2_u8_explicit_le",
        "classic/mr/caller",
    ] {
        let mut input = pet.clone();
        input.binding.case_id = name.into();
        input.recipe_id = "caller_activity".into();
        input.planning_order = Some(900);
        input.projection_order = Some(901);
        input.dicom.as_mut().unwrap().artifacts[0].output.path = Some("independent/pet.dcm".into());
        let plans = plan_nuclear_recipe(&input, &lock_hash, 7).unwrap().unwrap();
        assert_eq!(plans.len(), 1);
        assert_eq!(
            plans[0].output_relative_path.as_str(),
            "independent/pet.dcm"
        );
    }
    let pet_source = serde_json::to_value(pet).unwrap();
    // Every scalar/array source parameter and synthetic provider field is bound.
    for section in ["/provider_parameters", "/dicom/artifacts/0/parameters"] {
        for key in pet_source
            .pointer(section)
            .unwrap()
            .as_object()
            .unwrap()
            .keys()
        {
            let mut changed = pet_source.clone();
            changed
                .pointer_mut(section)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .remove(key);
            if let Ok(recipe) = serde_json::from_value(changed) {
                assert!(
                    plan_nuclear_recipe(&recipe, &lock_hash, 7).is_err(),
                    "missing {section}/{key}"
                );
            }
        }
    }
    for (pointer, value) in [
        (
            "/dicom/artifacts/0/parameters/rescale_slope",
            serde_json::json!("NaN"),
        ),
        (
            "/dicom/artifacts/0/parameters/frame_reference_time_ms",
            serde_json::json!("inf"),
        ),
        (
            "/dicom/artifacts/0/parameters/pixels/stored_values",
            serde_json::json!([0, 100, 200, 65536]),
        ),
        (
            "/dicom/artifacts/0/parameters/pixels/rows",
            serde_json::json!(65535),
        ),
        (
            "/dicom/artifacts/0/parameters/pixels/frame_sha256",
            serde_json::json!(["bad"]),
        ),
        (
            "/dicom/artifacts/0/parameters/pixels/pixel_max",
            serde_json::json!(399),
        ),
        (
            "/dicom/artifacts/0/template/template_id",
            serde_json::json!("classic/mr"),
        ),
        (
            "/dicom/artifacts/0/content/provider_id",
            serde_json::json!("different"),
        ),
        (
            "/dicom/artifacts/0/algorithm_provider_id",
            serde_json::json!("different"),
        ),
        (
            "/dicom/artifacts/0/output/path",
            serde_json::json!("../escape.dcm"),
        ),
        (
            "/dicom/artifacts/0/classic_projection/include_implementation_version_name",
            serde_json::json!(true),
        ),
    ] {
        let mut changed = pet_source.clone();
        *changed.pointer_mut(pointer).unwrap() = value;
        let recipe = serde_json::from_value(changed).unwrap();
        assert!(
            plan_nuclear_recipe(&recipe, &lock_hash, 7).is_err(),
            "{pointer}"
        );
    }
    let mut lexical = pet.clone();
    lexical.dicom.as_mut().unwrap().artifacts[0]
        .parameters
        .insert("rescale_slope".into(), serde_json::json!("2.50"));
    assert!(plan_nuclear_recipe(&lexical, &lock_hash, 7).is_err());
    let mut fragmented = pet.clone();
    fragmented.dicom.as_mut().unwrap().artifacts[0]
        .encoding
        .fragments_per_frame = Some(1);
    assert!(plan_nuclear_recipe(&fragmented, &lock_hash, 7).is_err());
    let us = catalog
        .recipes()
        .values()
        .find(|r| r.binding.case_id == "classic/us/mono2_u8_explicit_le")
        .unwrap();
    for name in [
        "caller/ultrasound",
        "classic/pet/rescaled_activity_explicit_le",
        "classic/mr/caller",
    ] {
        let mut input = us.clone();
        input.binding.case_id = name.into();
        input.recipe_id = "mr_multislice_oblique".into();
        input.planning_order = Some(900);
        input.projection_order = Some(901);
        input.dicom.as_mut().unwrap().artifacts[0].output.path = Some("independent/us.dcm".into());
        let plans = plan_nuclear_recipe(&input, &lock_hash, 7).unwrap().unwrap();
        assert_eq!(plans.len(), 1);
        assert_eq!(plans[0].output_relative_path.as_str(), "independent/us.dcm");
    }
    let source = serde_json::to_value(us).unwrap();
    for (pointer, value) in [
        (
            "/dicom/artifacts/0/parameters/pixels/rows",
            serde_json::json!(0),
        ),
        (
            "/dicom/artifacts/0/parameters/pixels/columns",
            serde_json::json!(65535),
        ),
        (
            "/dicom/artifacts/0/parameters/pixels/frames",
            serde_json::json!(u32::MAX),
        ),
        (
            "/dicom/artifacts/0/parameters/pixels/stored_value_type",
            serde_json::json!("u16"),
        ),
        (
            "/dicom/artifacts/0/parameters/pixels/stored_values",
            serde_json::json!([0, 85, 170, 256]),
        ),
        (
            "/dicom/artifacts/0/parameters/pixels/pixel_min",
            serde_json::json!(1),
        ),
        (
            "/dicom/artifacts/0/parameters/pixels/frame_sha256",
            serde_json::json!(["00".repeat(32)]),
        ),
        (
            "/dicom/artifacts/0/parameters/image_type",
            serde_json::json!(["DERIVED", "PRIMARY"]),
        ),
        (
            "/dicom/artifacts/0/parameters/lossy_image_compression",
            serde_json::json!("01"),
        ),
        (
            "/dicom/artifacts/0/parameters/ultrasound_color_data_present",
            serde_json::json!(1),
        ),
        ("/provider_parameters/modality", serde_json::json!("NM")),
        (
            "/dicom/artifacts/0/template/template_id",
            serde_json::json!("classic/mr"),
        ),
        (
            "/dicom/artifacts/0/algorithm_provider_id",
            serde_json::json!("algorithm.classic_mr_cr"),
        ),
        (
            "/dicom/artifacts/0/classic_projection/include_implementation_version_name",
            serde_json::json!(true),
        ),
        (
            "/dicom/artifacts/0/output/path",
            serde_json::json!("../escape"),
        ),
    ] {
        let mut bad = source.clone();
        *bad.pointer_mut(pointer).unwrap() = value;
        let bad = serde_json::from_value(bad).unwrap();
        assert!(
            plan_nuclear_recipe(&bad, &lock_hash, 7).is_err(),
            "{pointer}"
        );
    }
    for field in ["template", "classic_projection", "algorithm_provider_id"] {
        let mut bad = source.clone();
        bad["dicom"]["artifacts"][0]
            .as_object_mut()
            .unwrap()
            .remove(field);
        let bad = serde_json::from_value(bad).unwrap();
        assert!(
            plan_nuclear_recipe(&bad, &lock_hash, 7).is_err(),
            "missing {field}"
        );
    }
    for field in us.provider_parameters.keys() {
        let mut bad = us.clone();
        bad.provider_parameters
            .insert(field.clone(), serde_json::json!("different"));
        assert!(
            plan_nuclear_recipe(&bad, &lock_hash, 7).is_err(),
            "provider {field}"
        );
    }
    for field in ["series_date", "series_time", "body_part_examined"] {
        let mut bad = source.clone();
        bad["provider_parameters"][field] = serde_json::json!("override");
        assert!(plan_nuclear_recipe(&serde_json::from_value(bad).unwrap(), &lock_hash, 7).is_err());
    }
    for field in ["semantic_labels", "mr", "icc"] {
        let mut bad = us.clone();
        let projection = bad.dicom.as_mut().unwrap().artifacts[0]
            .classic_projection
            .as_mut()
            .unwrap();
        match field {
            "semantic_labels" => {
                projection.semantic_labels =
                    Some(serde_json::from_value(serde_json::json!({})).unwrap())
            }
            "mr" => {
                projection.mr = catalog.recipes().values().find_map(|r| {
                    r.dicom
                        .as_ref()?
                        .artifacts
                        .first()?
                        .classic_projection
                        .as_ref()?
                        .mr
                        .clone()
                })
            }
            _ => {
                projection.icc = catalog.recipes().values().find_map(|r| {
                    r.dicom
                        .as_ref()?
                        .artifacts
                        .first()?
                        .classic_projection
                        .as_ref()?
                        .icc
                        .clone()
                })
            }
        }
        assert!(plan_nuclear_recipe(&bad, &lock_hash, 7).is_err(), "{field}");
    }
    let mut fragment = us.clone();
    fragment.dicom.as_mut().unwrap().artifacts[0]
        .encoding
        .fragments_per_frame = Some(1);
    assert!(plan_nuclear_recipe(&fragment, &lock_hash, 7).is_err());
    let recipe = owned(&catalog)[0];
    let absent = temp_path("no-output");
    assert!(!absent.exists());
    plan_nuclear_recipe(recipe, &lock_hash, 7).unwrap().unwrap();
    assert!(!absent.exists());

    let mut unknown = recipe.clone();
    unknown
        .provider_parameters
        .insert("untyped_escape_hatch".into(), Value::Bool(true));
    assert!(matches!(
        plan_nuclear_recipe(&unknown, &lock_hash, 7),
        Err(ClassicNuclearPlanError::Parameters { .. })
    ));

    let mut corrupt = recipe.clone();
    corrupt.dicom.as_mut().unwrap().artifacts[0]
        .parameters
        .insert("counts_accumulated".into(), Value::from(903));
    assert!(matches!(
        plan_nuclear_recipe(&corrupt, &lock_hash, 7),
        Err(ClassicNuclearPlanError::Contract(_))
    ));

    let enhanced = catalog
        .recipes()
        .values()
        .find(|candidate| candidate.binding.case_id.starts_with("enhanced/pet/"))
        .unwrap();
    assert!(
        plan_nuclear_recipe(enhanced, &lock_hash, 7)
            .unwrap()
            .is_none()
    );
}

#[test]
fn us_multiframe_planning_is_caller_owned_and_fail_closed() {
    let (catalog, _, lock_hash) = load();
    let historical = catalog
        .recipes()
        .values()
        .find(|recipe| recipe.binding.case_id == "classic/us/multiframe_explicit_le")
        .unwrap();
    let mut source = serde_json::to_value(historical).unwrap();
    source["recipe_id"] = serde_json::json!("cine_recipe_with_no_historical_name");
    source["binding"]["case_id"] = serde_json::json!("caller/acquisition/cardiac-cine");
    source["planning_order"] = serde_json::json!(973);
    source["projection_order"] = serde_json::json!(811);
    source["provider_parameters"] = serde_json::json!({
        "patient_name": "CALLER^CINE", "patient_id": "SUBJECT-CINE-42",
        "patient_birth_date": "19880229", "patient_sex": "F",
        "study_date": "20260905", "study_time": "123456", "study_id": "CINE-STUDY",
        "accession_number": "ACC-CINE", "referring_physician_name": "REFERRER^CALLER",
        "modality": "US", "series_number": "17", "series_date": "20260905",
        "series_time": "123500", "manufacturer": "Caller Imaging",
        "manufacturer_model_name": "Portable Cine 7", "software_versions": "42.5",
        "acquisition_number": "8", "acquisition_date": "20260905",
        "acquisition_time": "123501", "instance_number": "23",
        "body_part_examined": "HEART"
    });
    let artifact = &mut source["dicom"]["artifacts"][0];
    artifact["logical_id"] = serde_json::json!("cine_loop");
    artifact["order"] = serde_json::json!(0);
    artifact["output"]["role"] = serde_json::json!("motion_review");
    artifact["output"]["path"] = serde_json::json!("independent/caller-cine.dcm");
    artifact["classic_projection"]["visual_pattern"] =
        serde_json::json!("three_frame_caller_pattern");
    let values = vec![1_u8, 3, 5, 7, 9, 11, 2, 4, 6, 8, 10, 12, 12, 10, 8, 6, 4, 2];
    let frame_hashes = values.chunks(6).map(sha256_hex).collect::<Vec<_>>();
    artifact["parameters"] = serde_json::json!({
        "family": "ultrasound_multiframe",
        "pixels": {"rows": 3, "columns": 2, "frames": 3, "stored_value_type": "u8",
            "stored_values": values, "pixel_min": 1, "pixel_max": 12,
            "frame_sha256": frame_hashes},
        "image_type": ["DERIVED", "SECONDARY", "CARDIAC", "CINE"],
        "frame_increment_pointer": "0018,1063", "frame_time_ms": 75,
        "frame_relative_times_ms": [0, 75, 150],
        "payload_sha256": sha256_hex(&values), "lossy_image_compression": "00",
        "color_data_present": false, "spatially_related_frames": false,
        "region_calibrated": false
    });

    let recipe = serde_json::from_value(source.clone()).unwrap();
    let request = plan_nuclear_recipe(&recipe, &lock_hash, 7)
        .unwrap()
        .unwrap()
        .remove(0);
    assert_eq!(request.logical_id, "cine_loop");
    assert_eq!(request.order, 0);
    assert_eq!(
        request.output_relative_path.as_str(),
        "independent/caller-cine.dcm"
    );
    assert_eq!(request.common.series.modality, "US");
    assert_eq!(
        request.common.equipment.manufacturer_model_name,
        synth_dicom_gen::recipes::ElementPresence::Value("Portable Cine 7".into())
    );

    for (pointer, replacement) in [
        ("/provider_parameters/modality", serde_json::json!("NM")),
        (
            "/provider_parameters/manufacturer_model_name",
            serde_json::json!(""),
        ),
        (
            "/provider_parameters/body_part_examined",
            serde_json::json!(""),
        ),
        (
            "/provider_parameters/instance_number",
            serde_json::json!("0"),
        ),
        (
            "/dicom/artifacts/0/template/template_version",
            serde_json::json!("2.0.0"),
        ),
        (
            "/dicom/artifacts/0/output/path",
            serde_json::json!("../escape.dcm"),
        ),
        (
            "/dicom/artifacts/0/parameters/pixels/frames",
            serde_json::json!(1),
        ),
        (
            "/dicom/artifacts/0/parameters/pixels/stored_values/0",
            serde_json::json!(256),
        ),
        (
            "/dicom/artifacts/0/parameters/pixels/frame_sha256/0",
            serde_json::json!("bad"),
        ),
        (
            "/dicom/artifacts/0/parameters/frame_time_ms",
            serde_json::json!(0),
        ),
        (
            "/dicom/artifacts/0/parameters/frame_relative_times_ms/2",
            serde_json::json!(149),
        ),
        (
            "/dicom/artifacts/0/parameters/image_type",
            serde_json::json!([]),
        ),
        (
            "/dicom/artifacts/0/parameters/payload_sha256",
            serde_json::json!("bad"),
        ),
        (
            "/dicom/artifacts/0/parameters/color_data_present",
            serde_json::json!(true),
        ),
    ] {
        let mut bad = source.clone();
        *bad.pointer_mut(pointer).unwrap() = replacement;
        let bad = serde_json::from_value(bad).unwrap();
        assert!(
            plan_nuclear_recipe(&bad, &lock_hash, 7).is_err(),
            "{pointer}"
        );
    }
}

#[test]
fn nm_multiframe_planning_is_caller_owned_and_fail_closed() {
    let (catalog, _, lock_hash) = load();
    let historical = catalog
        .recipes()
        .values()
        .find(|recipe| recipe.binding.case_id == "classic/nm/multiframe_explicit_le")
        .unwrap();
    let mut source = serde_json::to_value(historical).unwrap();
    source["recipe_id"] = serde_json::json!("spect_recipe_without_catalog_name");
    source["binding"]["case_id"] = serde_json::json!("caller/acquisition/rotating-study");
    source["planning_order"] = serde_json::json!(981);
    source["projection_order"] = serde_json::json!(817);
    source["provider_parameters"] = serde_json::json!({
        "patient_name": "CALLER^NUCLEAR", "patient_id": "SUBJECT-NM-77",
        "patient_birth_date": "19840312", "patient_sex": "F",
        "study_date": "20260905", "study_time": "151200", "study_id": "SPECT-STUDY",
        "accession_number": "NM-ACCESSION", "referring_physician_name": "ORDERING^CLINICIAN",
        "modality": "NM", "series_number": "31", "series_date": "20260905",
        "series_time": "151215", "manufacturer": "Caller Nuclear Systems",
        "manufacturer_model_name": "Orbit Three", "software_versions": "8.4",
        "acquisition_number": "9", "acquisition_date": "20260905",
        "acquisition_time": "151220", "instance_number": "14",
        "body_part_examined": "CHEST"
    });
    let artifact = &mut source["dicom"]["artifacts"][0];
    artifact["logical_id"] = serde_json::json!("rotating_counts");
    artifact["output"]["role"] = serde_json::json!("quantitative_review");
    artifact["output"]["path"] = serde_json::json!("caller-results/orbit-counts.dcm");
    artifact["classic_projection"]["visual_pattern"] =
        serde_json::json!("three_frame_rotating_counts");
    let values = (1_i64..=18).collect::<Vec<_>>();
    let frame_hashes = values
        .chunks(6)
        .map(|frame| {
            let bytes = frame
                .iter()
                .flat_map(|value| (*value as u16).to_le_bytes())
                .collect::<Vec<_>>();
            sha256_hex(&bytes)
        })
        .collect::<Vec<_>>();
    artifact["parameters"] = serde_json::json!({
        "family": "nuclear_medicine",
        "pixels": {"rows": 3, "columns": 2, "frames": 3, "stored_value_type": "u16",
            "stored_values": values, "pixel_min": 1, "pixel_max": 18,
            "frame_sha256": frame_hashes},
        "image_type": ["DERIVED", "SECONDARY", "TOMO"],
        "pixel_spacing": ["2.5", "3.25"],
        "energy_window_vector": [3, 1, 2], "detector_vector": [2, 1, 2],
        "energy_windows": [
            {"index": 1, "name": "Lower", "lower_limit_kev": "70", "upper_limit_kev": "90"},
            {"index": 2, "name": "Middle", "lower_limit_kev": "91", "upper_limit_kev": "110"},
            {"index": 3, "name": "Upper", "lower_limit_kev": "111", "upper_limit_kev": "140"}
        ],
        "detectors": [
            {"index": 1, "collimator_type": "FANB", "focal_distance_mm": "250",
             "start_angle_degrees": "45", "image_orientation_patient": ["0", "1", "0", "-1", "0", "0"],
             "image_position_patient": ["10", "20", "30"]},
            {"index": 2, "collimator_type": "CONE", "focal_distance_mm": "500",
             "start_angle_degrees": "225", "image_orientation_patient": ["0", "0", "1", "0", "1", "0"],
             "image_position_patient": ["-5", "12", "40"]}
        ],
        "actual_frame_duration_ms": 750, "counts_accumulated": 171
    });

    let recipe = serde_json::from_value(source.clone()).unwrap();
    let request = plan_nuclear_recipe(&recipe, &lock_hash, 7)
        .unwrap()
        .unwrap()
        .remove(0);
    assert_eq!(request.logical_id, "rotating_counts");
    assert_eq!(
        request.output_relative_path.as_str(),
        "caller-results/orbit-counts.dcm"
    );
    assert_eq!(request.common.series.modality, "NM");

    for (pointer, replacement) in [
        ("/provider_parameters/modality", serde_json::json!("US")),
        (
            "/provider_parameters/manufacturer_model_name",
            serde_json::json!(""),
        ),
        (
            "/provider_parameters/instance_number",
            serde_json::json!("0"),
        ),
        (
            "/dicom/artifacts/0/template/template_version",
            serde_json::json!("2.0.0"),
        ),
        (
            "/dicom/artifacts/0/output/path",
            serde_json::json!("../escape.dcm"),
        ),
        (
            "/dicom/artifacts/0/parameters/pixels/frames",
            serde_json::json!(1),
        ),
        (
            "/dicom/artifacts/0/parameters/pixels/stored_values/0",
            serde_json::json!(65536),
        ),
        (
            "/dicom/artifacts/0/parameters/pixels/frame_sha256/0",
            serde_json::json!("bad"),
        ),
        (
            "/dicom/artifacts/0/parameters/image_type",
            serde_json::json!([]),
        ),
        (
            "/dicom/artifacts/0/parameters/pixel_spacing/0",
            serde_json::json!("0"),
        ),
        (
            "/dicom/artifacts/0/parameters/energy_window_vector/0",
            serde_json::json!(4),
        ),
        (
            "/dicom/artifacts/0/parameters/detector_vector/0",
            serde_json::json!(0),
        ),
        (
            "/dicom/artifacts/0/parameters/energy_windows/1/index",
            serde_json::json!(3),
        ),
        (
            "/dicom/artifacts/0/parameters/energy_windows/0/upper_limit_kev",
            serde_json::json!("60"),
        ),
        (
            "/dicom/artifacts/0/parameters/detectors/0/collimator_type",
            serde_json::json!("BAD"),
        ),
        (
            "/dicom/artifacts/0/parameters/detectors/0/image_orientation_patient/3",
            serde_json::json!("0"),
        ),
        (
            "/dicom/artifacts/0/parameters/actual_frame_duration_ms",
            serde_json::json!(0),
        ),
        (
            "/dicom/artifacts/0/parameters/counts_accumulated",
            serde_json::json!(170),
        ),
    ] {
        let mut bad = source.clone();
        *bad.pointer_mut(pointer).unwrap() = replacement;
        let bad = serde_json::from_value(bad).unwrap();
        assert!(
            plan_nuclear_recipe(&bad, &lock_hash, 7).is_err(),
            "{pointer}"
        );
    }
}

#[test]
fn direct_nuclear_plans_match_current_bytes_and_manifest_facts() {
    let generated_root = temp_path("legacy");
    let planned_root = temp_path("planned");
    let run = prepare_generation_run(GenerateOptions {
        profile: "all".into(),
        out_dir: generated_root.clone(),
        seed: 7,
        include_stress: false,
    })
    .unwrap();
    write_generation_run(&run).unwrap();
    fs::create_dir(&planned_root).unwrap();
    let manifest: Value =
        serde_json::from_slice(&fs::read(generated_root.join("manifest.json")).unwrap()).unwrap();
    let (catalog, templates, lock_hash) = load();

    for recipe in owned(&catalog) {
        let artifact = &recipe.dicom.as_ref().unwrap().artifacts[0];
        let parameters: ClassicNuclearArtifactParameters =
            serde_json::from_value(Value::Object(artifact.parameters.clone())).unwrap();
        let requests = plan_nuclear_recipe(recipe, &lock_hash, 7).unwrap().unwrap();
        assert!(
            requests
                .iter()
                .all(|request| request.pixels.slot == CLASSIC_PIXEL_SLOT)
        );
        let planned = OrderedSeriesProvider.plan(requests).unwrap().remove(0);
        let native = planned.pixels.content.unpadded_bytes.clone();
        let shape = planned.pixels.content.plan.shape.clone();
        let reference = artifact.template.as_ref().unwrap();
        let template = templates
            .resolve_qualified(
                &synth_dicom_gen::composition::TemplateId(reference.template_id.clone()),
                Some(reference.template_version.parse().unwrap()),
            )
            .unwrap();
        let mut resolved = resolved_classic_instance_plan(ClassicResolvedPlanInput {
            planned,
            template,
            transfer_syntax_uid: &artifact.encoding.transfer_syntax_uid,
            encoding_backend_id: artifact
                .encoding
                .non_template_encoding_provider_id
                .as_deref()
                .unwrap_or("dicom-rs.part10"),
        })
        .unwrap();
        if artifact.encoding.transfer_syntax_uid
            == synth_dicom_gen::codecs::RLE_LOSSLESS_TRANSFER_SYNTAX_UID
        {
            make_rle_content(
                &mut resolved,
                &native,
                shape.rows,
                shape.columns,
                shape.bits_allocated,
            );
        }
        let direct_path = planned_root.join(artifact.output.path.as_ref().unwrap());
        Part10Materializer
            .materialize(&resolved, &direct_path)
            .unwrap();
        let legacy_path = generated_root.join(artifact.output.path.as_ref().unwrap());
        assert_eq!(
            fs::read(&direct_path).unwrap(),
            fs::read(&legacy_path).unwrap(),
            "{} changed bytes",
            recipe.binding.case_id
        );

        let entry = manifest["files"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["case_id"] == recipe.binding.case_id)
            .unwrap();
        assert_eq!(entry["path"].as_str(), artifact.output.path.as_deref());
        assert_eq!(entry["recipe"]["recipe_id"], recipe.recipe_id);
        assert_eq!(
            entry["dicom"]["transfer_syntax_uid"],
            artifact.encoding.transfer_syntax_uid
        );
        assert_eq!(
            entry["known_stressors"],
            serde_json::to_value(&artifact.stressors).unwrap()
        );
        let pixels = match parameters {
            ClassicNuclearArtifactParameters::UltrasoundSingleFrame { pixels, .. }
            | ClassicNuclearArtifactParameters::UltrasoundMultiframe { pixels, .. }
            | ClassicNuclearArtifactParameters::NuclearMedicine { pixels, .. }
            | ClassicNuclearArtifactParameters::Pet { pixels, .. } => pixels,
        };
        assert_eq!(entry["image"]["rows"], pixels.rows);
        assert_eq!(entry["image"]["columns"], pixels.columns);
        assert_eq!(entry["image"]["frames"], pixels.frames);
        assert_eq!(
            entry["pixel_data"]["frame_hashes"],
            serde_json::to_value(pixels.frame_sha256).unwrap()
        );
    }
}
