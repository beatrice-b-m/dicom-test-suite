use std::path::{Path, PathBuf};

use synth_dicom_gen::composition::{AttributeOperation, AttributeValue, PrimitiveValue};
use synth_dicom_gen::recipes::classic_mr_cr::{ClassicMrCrPlanError, plan_mr_cr_recipe};
use synth_dicom_gen::recipes::{CaseRecipe, OrderedSeriesProvider};
use synth_dicom_gen::uid::{DeterministicUidInput, UidRole, deterministic_uid};

const LOCK_HASH: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn load(path: &str) -> CaseRecipe {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(path);
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

fn operation<'a>(operations: &'a [AttributeOperation], tag: &str) -> &'a AttributeOperation {
    operations
        .iter()
        .find(|operation| operation.address().normalized_tag() == tag)
        .unwrap_or_else(|| panic!("missing operation {tag}"))
}

fn strings(operation: &AttributeOperation) -> Vec<&str> {
    match operation {
        AttributeOperation::Set {
            value: AttributeValue::Primitive(PrimitiveValue::String(value)),
            ..
        } => vec![value],
        AttributeOperation::Set {
            value: AttributeValue::Multi(values),
            ..
        } => values
            .iter()
            .map(|value| match value {
                PrimitiveValue::String(value) => value.as_str(),
                _ => panic!("expected string value"),
            })
            .collect(),
        _ => panic!("expected string operation"),
    }
}

#[test]
fn mr_oblique_plan_preserves_exact_order_geometry_pixels_and_identities() {
    let recipe = load("cases/recipes/classic/mr/mr_multislice_oblique.json");
    assert_eq!(recipe.planning_order, Some(502));
    let requests = plan_mr_cr_recipe(&recipe, LOCK_HASH, 17).unwrap().unwrap();
    assert_eq!(requests.len(), 3);
    assert_eq!(
        requests
            .iter()
            .map(|request| request.output_relative_path.as_str())
            .collect::<Vec<_>>(),
        [
            "classic/mr/multislice_oblique_explicit_le/slice-001.dcm",
            "classic/mr/multislice_oblique_explicit_le/slice-002.dcm",
            "classic/mr/multislice_oblique_explicit_le/slice-003.dcm"
        ]
    );
    assert!(
        requests
            .windows(2)
            .all(|pair| pair[0].common.study.study_instance_uid
                == pair[1].common.study.study_instance_uid
                && pair[0].common.series.series_instance_uid
                    == pair[1].common.series.series_instance_uid)
    );
    assert_ne!(requests[0].sop_instance_uid, requests[1].sop_instance_uid);
    let expected_sop = deterministic_uid(&DeterministicUidInput {
        standards_lock_sha256: LOCK_HASH,
        case_id: "classic/mr/multislice_oblique_explicit_le",
        recipe_version: "0.1.0",
        run_seed: 17,
        file_index: 2,
        frame_index: None,
        referenced_object_index: None,
        role: UidRole::SopInstance,
    });
    assert_eq!(requests[2].sop_instance_uid, expected_sop);

    let third = requests[2].family[0].module();
    assert_eq!(
        strings(operation(&third.operations, "0020,0032")),
        ["7.071068", "-7.071068", "0"]
    );
    assert_eq!(
        strings(operation(&third.operations, "0020,0037")),
        ["0.70710678", "0.70710678", "0", "0", "0", "1"]
    );

    let planned = OrderedSeriesProvider.plan(requests).unwrap();
    assert_eq!(
        planned
            .iter()
            .map(|instance| instance.order)
            .collect::<Vec<_>>(),
        [0, 1, 2]
    );
    assert_eq!(
        planned[0].pixels.content.unpadded_bytes,
        [0, 0, 1, 0, 2, 0, 3, 0]
    );
    assert_eq!(
        planned[1].pixels.content.unpadded_bytes,
        [10, 0, 11, 0, 12, 0, 13, 0]
    );
    assert_eq!(
        planned[2].pixels.content.unpadded_bytes,
        [20, 0, 21, 0, 22, 0, 23, 0]
    );
}

#[test]
fn mr_rle_reuses_exact_native_object_plan() {
    let recipe = load("cases/recipes/classic/mr/mr_mono2_u16_rle_lossless.json");
    assert_eq!(recipe.planning_order, Some(503));
    let request = plan_mr_cr_recipe(&recipe, LOCK_HASH, 0)
        .unwrap()
        .unwrap()
        .pop()
        .unwrap();
    assert_eq!(request.pixels.pixels.stored_values, [0, 1, 2, 3]);
    assert_eq!(
        request.output_relative_path.as_str(),
        "classic/mr/mono2_u16_rle_lossless/slice-001.dcm"
    );
    assert_eq!(
        recipe.dicom.unwrap().artifacts[0]
            .encoding
            .offset_table_policy,
        "populated_basic"
    );
}

#[test]
fn cr_plan_contains_exact_overlay_and_lut_sequences() {
    let source = load("cases/recipes/classic/cr/cr_overlay_modality_voi.json");
    for name in ["caller/radiography", "classic/mr/misleading"] {
        let mut renamed = source.clone();
        renamed.binding.case_id = name.into();
        renamed.recipe_id = "caller_recipe".into();
        renamed.planning_order = Some(900);
        renamed.projection_order = Some(901);
        renamed.dicom.as_mut().unwrap().artifacts[0].output.path =
            Some("independent/cr.dcm".into());
        let requests = plan_mr_cr_recipe(&renamed, LOCK_HASH, 9).unwrap().unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(
            requests[0].output_relative_path.as_str(),
            "independent/cr.dcm"
        );
        let planned = OrderedSeriesProvider.plan(requests).unwrap();
        assert_eq!(planned[0].pixels.content.unpadded_bytes, [0, 1, 2, 3]);
    }
    for (field, value) in [
        ("rows", serde_json::json!(65536)),
        ("columns", serde_json::json!(u32::MAX)),
        ("pixel_min", serde_json::json!(-1)),
        ("stored_values", serde_json::json!([0, 1, 2, 256])),
        ("frame_sha256", serde_json::json!("00".repeat(32))),
    ] {
        let mut bad = source.clone();
        bad.dicom.as_mut().unwrap().artifacts[0]
            .parameters
            .insert(field.into(), value);
        assert!(plan_mr_cr_recipe(&bad, LOCK_HASH, 0).is_err(), "{field}");
    }
    for (section, field, value) in [
        ("overlay", "data", serde_json::json!([9, 1])),
        ("overlay", "origin", serde_json::json!([0, 0])),
        ("modality_lut", "descriptor", serde_json::json!([3, 0, 16])),
        ("voi_lut", "data", serde_json::json!([0, 1])),
        ("voi_lut", "lut_type", serde_json::json!("US")),
    ] {
        let mut bad = source.clone();
        bad.dicom.as_mut().unwrap().artifacts[0]
            .parameters
            .get_mut(section)
            .unwrap()[field] = value;
        assert!(
            plan_mr_cr_recipe(&bad, LOCK_HASH, 0).is_err(),
            "{section}.{field}"
        );
    }
    let mr = load("cases/recipes/classic/mr/mr_multislice_oblique.json")
        .dicom
        .unwrap()
        .artifacts[0]
        .classic_projection
        .as_ref()
        .unwrap()
        .mr
        .clone();
    let icc = load("cases/recipes/vl/photo/vl_photo_rgb_icc_profile_explicit_le.json")
        .dicom
        .unwrap()
        .artifacts[0]
        .classic_projection
        .as_ref()
        .unwrap()
        .icc
        .clone();
    assert!(mr.is_some() && icc.is_some());
    for override_kind in 0..9 {
        let mut bad = source.clone();
        let projection = bad.dicom.as_mut().unwrap().artifacts[0]
            .classic_projection
            .as_mut()
            .unwrap();
        match override_kind {
            0 => projection.mr = mr.clone(),
            1 => projection.icc = icc.clone(),
            2 => projection.include_implementation_version_name = true,
            3 => projection.semantic_labels = None,
            4 => projection.semantic_labels.as_mut().unwrap().overlay_pattern = None,
            5 => projection.semantic_labels.as_mut().unwrap().modality_lut = None,
            6 => projection.semantic_labels.as_mut().unwrap().voi_lut = None,
            7 => {
                projection
                    .semantic_labels
                    .as_mut()
                    .unwrap()
                    .photometric_semantics = Some("override".into())
            }
            _ => {
                bad.dicom.as_mut().unwrap().artifacts[0].public_profile_membership =
                    Some(vec!["core".into()])
            }
        }
        assert!(
            plan_mr_cr_recipe(&bad, LOCK_HASH, 0).is_err(),
            "override {override_kind}"
        );
    }
    let mut appended = serde_json::to_value(&source).unwrap();
    appended["dicom"]["artifacts"][0]["classic_projection"]["standards_evidence_append"] = serde_json::json!([{"source":"test", "part":"PS3.3", "anchor":"test", "edition":"2026b", "query":"test", "covered":true}]);
    let appended: CaseRecipe = serde_json::from_value(appended).unwrap();
    assert!(plan_mr_cr_recipe(&appended, LOCK_HASH, 0).is_err());
    for cross in 0..7 {
        let mut bad = source.clone();
        let a = &mut bad.dicom.as_mut().unwrap().artifacts[0];
        match cross {
            0 => a.template.as_mut().unwrap().template_id = "classic/mr".into(),
            1 => a.classic_projection = None,
            2 => a.content.provider_id = "content.case_default".into(),
            3 => a.algorithm_provider_id = None,
            4 => a.output.path = Some("../escape.dcm".into()),
            5 => a.encoding.fragments_per_frame = Some(2),
            _ => {
                let duplicate = a.clone();
                bad.dicom.as_mut().unwrap().artifacts.push(duplicate);
            }
        }
        assert!(
            plan_mr_cr_recipe(&bad, LOCK_HASH, 0).is_err(),
            "cross {cross}"
        );
    }
    for (path, planning_order) in [
        ("cases/recipes/classic/cr/cr_overlay_modality_voi.json", 500),
        (
            "cases/recipes/classic/cr/cr_overlay_modality_voi_rle_lossless.json",
            501,
        ),
    ] {
        let recipe = load(path);
        assert_eq!(recipe.planning_order, Some(planning_order));
        let requests = plan_mr_cr_recipe(&recipe, LOCK_HASH, 9).unwrap().unwrap();
        let fragment = requests[0].family[0].module();
        assert!(matches!(
            operation(&fragment.operations, "6000,3000"),
            AttributeOperation::Set { value: AttributeValue::Binary(bytes), .. } if bytes == &[9, 0]
        ));
        for tag in ["0028,3000", "0028,3010"] {
            let AttributeOperation::Set {
                value: AttributeValue::Sequence(items),
                ..
            } = operation(&fragment.operations, tag)
            else {
                panic!("{tag} must be a sequence")
            };
            assert_eq!(items.len(), 1);
            assert_eq!(
                strings(operation(&items[0].attributes, "0028,3003")).len(),
                1
            );
            assert!(matches!(
                operation(&items[0].attributes, "0028,3006"),
                AttributeOperation::Set { value: AttributeValue::Binary(bytes), .. } if bytes.len() == 8
            ));
        }
        let planned = OrderedSeriesProvider.plan(requests).unwrap();
        assert_eq!(planned[0].pixels.content.unpadded_bytes, [0, 1, 2, 3]);
    }
}

#[test]
fn provider_rejects_unknown_parameters_hash_drift_and_order_collisions() {
    let mut unknown = load("cases/recipes/classic/mr/mr_multislice_oblique.json");
    unknown.dicom.as_mut().unwrap().artifacts[0]
        .parameters
        .insert("unexpected".into(), serde_json::json!(true));
    assert!(matches!(
        plan_mr_cr_recipe(&unknown, LOCK_HASH, 0),
        Err(ClassicMrCrPlanError::Parameters(_))
    ));

    let mut drift = load("cases/recipes/classic/cr/cr_overlay_modality_voi.json");
    drift.dicom.as_mut().unwrap().artifacts[0]
        .parameters
        .insert("frame_sha256".into(), serde_json::json!("00".repeat(32)));
    assert!(matches!(
        plan_mr_cr_recipe(&drift, LOCK_HASH, 0),
        Err(ClassicMrCrPlanError::Contract("CR frame hash"))
    ));

    let mut collision = load("cases/recipes/classic/mr/mr_multislice_oblique.json");
    collision.dicom.as_mut().unwrap().artifacts[1].order = 0;
    assert!(matches!(
        plan_mr_cr_recipe(&collision, LOCK_HASH, 0),
        Err(ClassicMrCrPlanError::Contract("artifact provider binding"))
    ));
}

#[test]
fn planning_is_output_free_and_source_has_no_generator_or_writer_dependency() {
    let source = include_str!("../src/recipes/classic_mr_cr.rs");
    for forbidden in [
        "crate::generator",
        "std::fs",
        "PathBuf",
        "Part10Materializer",
        "InMemDicomObject",
        "out_dir",
    ] {
        assert!(!source.contains(forbidden), "source contains {forbidden}");
    }
    let absent = PathBuf::from(format!(
        "/tmp/dicom-test-suite-mr-cr-plan-must-not-exist-{}",
        std::process::id()
    ));
    assert!(!absent.exists());
    let recipe = load("cases/recipes/classic/cr/cr_overlay_modality_voi.json");
    let _ = plan_mr_cr_recipe(&recipe, LOCK_HASH, 0).unwrap();
    assert!(!absent.exists());
}

#[test]
fn non_owned_recipe_is_ignored() {
    let recipe = load("cases/recipes/classic/sc/sc_mono1_u8.json");
    assert!(plan_mr_cr_recipe(&recipe, LOCK_HASH, 0).unwrap().is_none());
}
