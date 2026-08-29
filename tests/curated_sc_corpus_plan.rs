use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::sync::atomic::{AtomicU64, Ordering};

use dicom_test_suite::composition::ContentMaterialization;
use dicom_test_suite::corpus_plan::{OffsetTablePolicy, PlannedArtifact};
use dicom_test_suite::curated_plan::{
    CuratedCatalogPaths, CuratedScCorpusPlanProvider, CuratedScPlanRequest, CuratedScSelection,
};
use dicom_test_suite::executor::services::SlotExecutionBinding;
use dicom_test_suite::native_pixel::ByteOrder;
use dicom_test_suite::recipes::{CLASSIC_PIXEL_SLOT, RecipeCatalog};
use serde_json::Value;

static NEXT: AtomicU64 = AtomicU64::new(0);

fn provider() -> CuratedScCorpusPlanProvider {
    CuratedScCorpusPlanProvider::load(CuratedCatalogPaths::from_repository_root(".")).unwrap()
}

fn all_request() -> CuratedScPlanRequest {
    CuratedScPlanRequest {
        selection: CuratedScSelection::AllFeatureFree,
        seed: 19,
        max_parallelism: 4,
    }
}

fn source_inventory() -> (Vec<(String, String, String)>, BTreeSet<String>) {
    let registry: Value =
        serde_json::from_slice(&fs::read("cases/registry.json").unwrap()).unwrap();
    let recipes = RecipeCatalog::load(
        "cases/recipes",
        "cases/registry.json",
        "templates/catalog.json",
    )
    .unwrap();
    let mut artifacts = Vec::new();
    let pending = BTreeSet::new();
    let mut registry_cases = registry["cases"]
        .as_array()
        .unwrap()
        .iter()
        .collect::<Vec<_>>();
    registry_cases.sort_by_key(|case| {
        case["case_id"]
            .as_str()
            .and_then(|case_id| recipes.binding_for_case(case_id))
            .and_then(|identity| recipes.recipes().get(identity))
            .and_then(|recipe| recipe.planning_order)
            .unwrap_or(u32::MAX)
    });
    for case in registry_cases {
        let requirements = &case["requirements"];
        let feature_free = ["features", "external_codecs", "external_validators"]
            .iter()
            .all(|field| requirements[field].as_array().unwrap().is_empty());
        if case["status"] != "implemented" || !feature_free {
            continue;
        }
        let case_id = case["case_id"].as_str().unwrap();
        let Some(identity) = recipes.binding_for_case(case_id) else {
            continue;
        };
        let recipe = &recipes.recipes()[identity];
        if !matches!(
            recipe.plan_provider_id.as_str(),
            "native.sc_plan"
                | "native.metadata_sc_plan"
                | "native.classic_plan"
                | "native.enhanced_plan"
                | "native.wsi_plan"
        ) {
            continue;
        }
        let mut members = recipe
            .dicom
            .as_ref()
            .unwrap()
            .artifacts
            .iter()
            .collect::<Vec<_>>();
        members.sort_by_key(|artifact| artifact.order);
        for artifact in members {
            artifacts.push((
                if matches!(
                    recipe.plan_provider_id.as_str(),
                    "native.enhanced_plan" | "native.wsi_plan"
                ) {
                    artifact.logical_id.clone()
                } else {
                    format!("curated_{}_{}", recipe.recipe_id, artifact.logical_id)
                },
                artifact.output.path.clone().unwrap(),
                case_id.into(),
            ));
        }
    }
    (artifacts, pending)
}

#[test]
fn full_feature_free_slice_joins_registry_recipe_template_and_order_exactly() {
    let (expected, expected_pending) = source_inventory();
    assert!(!expected.is_empty());
    let bundle = provider().plan(&all_request()).unwrap();
    bundle.plan.validate().unwrap();

    let actual = bundle
        .plan
        .artifacts
        .iter()
        .map(|artifact| {
            let PlannedArtifact::Dicom(artifact) = artifact else {
                panic!("SC provider emitted a non-DICOM artifact")
            };
            (
                artifact.logical_id.clone(),
                artifact.output.relative_path.as_str().to_owned(),
                artifact.case_binding.as_ref().unwrap().case_id.clone(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(actual, expected);
    assert_eq!(
        bundle
            .plan
            .artifacts
            .iter()
            .map(|artifact| artifact.order())
            .collect::<Vec<_>>(),
        (0..bundle.plan.artifacts.len() as u64).collect::<Vec<_>>()
    );
    assert_eq!(
        bundle.bindings.keys().cloned().collect::<BTreeSet<_>>(),
        expected
            .iter()
            .map(|(artifact_id, _, _)| artifact_id.clone())
            .collect()
    );
    assert_eq!(
        bundle
            .pending
            .iter()
            .map(|pending| pending.case_id.clone())
            .collect::<BTreeSet<_>>(),
        expected_pending
    );
    assert_eq!(
        bundle.plan.unavailable.len(),
        usize::from(!expected_pending.is_empty())
    );
    let recipes = RecipeCatalog::load(
        "cases/recipes",
        "cases/registry.json",
        "templates/catalog.json",
    )
    .unwrap();
    for artifact in &bundle.plan.artifacts {
        let PlannedArtifact::Dicom(artifact) = artifact else {
            unreachable!()
        };
        let binding = artifact.case_binding.as_ref().unwrap();
        let identity = dicom_test_suite::planning::RecipeIdentity {
            recipe_id: binding.recipe_id.clone(),
            recipe_version: binding.recipe_version.clone(),
        };
        let recipe = &recipes.recipes()[&identity];
        let member = recipe
            .dicom
            .as_ref()
            .unwrap()
            .artifacts
            .iter()
            .find(|member| member.output.role == artifact.output.role)
            .unwrap();
        let mut seen = BTreeSet::new();
        let expected_rules = recipe
            .validation_rule_ids
            .iter()
            .chain(&member.validation_rule_ids)
            .filter(|rule| seen.insert((*rule).clone()))
            .cloned()
            .collect::<Vec<_>>();
        let actual_rules = artifact
            .validation
            .rules
            .iter()
            .map(|rule| rule.rule_id.clone())
            .collect::<Vec<_>>();
        if matches!(
            recipe.plan_provider_id.as_str(),
            "native.enhanced_plan" | "native.wsi_plan"
        ) {
            assert!(!actual_rules.is_empty());
        } else {
            assert_eq!(actual_rules, expected_rules);
        }
    }
    let artifact_ids = bundle
        .plan
        .artifacts
        .iter()
        .map(|artifact| artifact.logical_id())
        .collect::<BTreeSet<_>>();
    assert!(bundle.plan.dependencies.iter().all(|dependency| {
        artifact_ids.contains(dependency.artifact_id.as_str())
            && artifact_ids.contains(dependency.depends_on.as_str())
    }));
}

#[test]
fn projection_context_is_lossless_ordered_and_one_to_one_with_plan() {
    let bundle = provider().plan(&all_request()).unwrap();
    bundle.projection.validate(&bundle.plan).unwrap();
    assert_eq!(
        bundle.projection.artifacts.len(),
        bundle.plan.artifacts.len()
    );

    let registry: Value =
        serde_json::from_slice(&fs::read("cases/registry.json").unwrap()).unwrap();
    let registry_by_id = registry["cases"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
        .map(|(order, case)| (case["case_id"].as_str().unwrap(), (order, case)))
        .collect::<BTreeMap<_, _>>();
    for (planned, projected) in bundle
        .plan
        .artifacts
        .iter()
        .zip(&bundle.projection.artifacts)
    {
        assert_eq!(projected.artifact_id, planned.logical_id());
        assert_eq!(projected.plan_order, planned.order());
        let (registry_order, source_case) =
            registry_by_id[projected.registry_case.case_id.as_str()];
        assert_eq!(projected.registry_order, registry_order as u64);
        assert_eq!(
            serde_json::to_value(&projected.registry_case).unwrap(),
            *source_case
        );
        assert_eq!(
            projected.case_recipe.planning_order,
            Some(projected.historical_recipe_order)
        );
        assert_eq!(
            projected.artifact_recipe.order,
            projected.historical_artifact_order
        );
    }

    let encoded = serde_json::to_vec(&bundle.projection).unwrap();
    let decoded: dicom_test_suite::curated_plan::CuratedScProjectionContext =
        serde_json::from_slice(&encoded).unwrap();
    assert_eq!(decoded, bundle.projection);
}

#[test]
fn projection_context_rejects_missing_duplicate_and_cross_bound_artifacts() {
    let bundle = provider().plan(&all_request()).unwrap();

    let mut missing = bundle.projection.clone();
    missing.artifacts.pop();
    assert!(missing.validate(&bundle.plan).is_err());

    let mut duplicate = bundle.projection.clone();
    duplicate.artifacts[1] = duplicate.artifacts[0].clone();
    assert!(duplicate.validate(&bundle.plan).is_err());

    let mut crossed = bundle.projection.clone();
    crossed.artifacts.swap(0, 1);
    assert!(crossed.validate(&bundle.plan).is_err());
}

#[test]
fn native_and_rle_requests_preserve_be_and_all_bot_policy_boundaries() {
    let bundle = provider().plan(&all_request()).unwrap();
    let native_requests = bundle
        .native_content_requests
        .iter()
        .map(|request| (request.artifact_id.as_str(), request))
        .collect::<BTreeMap<_, _>>();
    let mut observed_empty_bot = false;
    let mut observed_populated_bot = false;
    let mut observed_extended = false;
    let mut observed_big_endian = false;

    for artifact in &bundle.plan.artifacts {
        let PlannedArtifact::Dicom(artifact) = artifact else {
            unreachable!()
        };
        let binding = &bundle.bindings[&artifact.logical_id];
        binding
            .validate(&dicom_test_suite::executor::services::StagedAssetRegistry::default())
            .unwrap();
        let Some(native) = native_requests.get(artifact.logical_id.as_str()) else {
            assert!(binding.slots.contains_key("pixels"));
            continue;
        };
        assert_eq!(
            native.frame_sha256.len(),
            native.request.shape.frames as usize
        );
        assert_eq!(native.unpadded_sha256, artifact.instance.content[0].sha256);
        if artifact.encoding.transfer_syntax_uid == "1.2.840.10008.1.2.2" {
            observed_big_endian = true;
            assert_eq!(native.request.shape.byte_order, ByteOrder::Big);
            let ContentMaterialization::Inline(bytes) = artifact.instance.content[0]
                .materialization
                .as_ref()
                .unwrap()
            else {
                panic!("BE native pixels were not canonical inline content")
            };
            assert_eq!(
                artifact.instance.content[0].sha256,
                dicom_test_suite::sha256_hex(bytes)
            );
            assert!(matches!(
                binding.slots["pixels"],
                SlotExecutionBinding::NativeFrames { .. }
            ));
        } else if artifact.encoding.transfer_syntax_uid == "1.2.840.10008.1.2.5" {
            match artifact.encoding.offset_table {
                OffsetTablePolicy::EmptyBasic => observed_empty_bot = true,
                OffsetTablePolicy::PopulatedBasic => observed_populated_bot = true,
                OffsetTablePolicy::Extended => observed_extended = true,
                OffsetTablePolicy::NotApplicable => panic!("RLE cannot omit its table policy"),
            }
            let SlotExecutionBinding::CodecRequest { request } = &binding.slots["pixels"] else {
                panic!("RLE artifact lacks a deferred codec request")
            };
            assert_eq!(request.backend_id, "native_project_rle_encoder");
            assert_eq!(request.frames.len(), native.request.shape.frames as usize);
            assert_eq!(
                request.target_transfer_syntax_uid,
                artifact.encoding.transfer_syntax_uid
            );
            assert!(request.frames.iter().all(|frame| {
                matches!(
                    frame.bytes,
                    dicom_test_suite::executor::services::ByteBinding::Inline { .. }
                )
            }));
        } else {
            let SlotExecutionBinding::NativeFrames { frames } = &binding.slots["pixels"] else {
                panic!("native artifact lacks ordered frame bindings")
            };
            let bytes = frames
                .iter()
                .flat_map(|frame| match &frame.bytes {
                    dicom_test_suite::executor::services::ByteBinding::Inline { bytes, .. } => {
                        bytes.clone()
                    }
                    _ => panic!("native request is not inline"),
                })
                .collect::<Vec<_>>();
            let ContentMaterialization::Inline(expected) = artifact.instance.content[0]
                .materialization
                .as_ref()
                .unwrap()
            else {
                panic!("native plan content is not inline")
            };
            assert_eq!(&bytes, expected);
        }
    }
    assert!(observed_big_endian);
    assert!(observed_empty_bot && observed_populated_bot && observed_extended);
}

#[test]
fn planning_is_deterministic_output_root_free_and_creates_no_files() {
    let would_be_output = std::env::temp_dir().join(format!(
        "dts-curated-plan-must-not-create-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    assert!(!would_be_output.exists());
    let provider = provider();
    let first = provider.plan(&all_request()).unwrap();
    let second = provider.plan(&all_request()).unwrap();
    assert_eq!(first, second);
    assert_eq!(
        first.plan.canonical_sha256().unwrap(),
        second.plan.canonical_sha256().unwrap()
    );
    assert!(!would_be_output.exists());

    // The request and provider API contain selection/identity controls only;
    // this compile-time construction has no output-root field to populate.
    let _request_without_output_root = CuratedScPlanRequest {
        selection: CuratedScSelection::Profile {
            profile: "all".into(),
            include_stress: false,
        },
        seed: 19,
        max_parallelism: 2,
    };
}

#[test]
fn explicit_metadata_selection_uses_the_exported_neutral_planner() {
    let bundle = provider()
        .plan(&CuratedScPlanRequest {
            selection: CuratedScSelection::CaseIds(vec!["metadata/sc/utf8_person_name".into()]),
            seed: 1,
            max_parallelism: 1,
        })
        .unwrap();
    assert!(!bundle.plan.artifacts.is_empty());
    assert_eq!(bundle.bindings.len(), bundle.plan.artifacts.len());
    assert!(bundle.pending.is_empty());
    assert!(bundle.plan.unavailable.is_empty());
    bundle.plan.validate().unwrap();
}

#[test]
fn mixed_sc_and_every_classic_provider_share_one_ordered_artifact_set() {
    let selected = vec![
        "classic/sc/mono2_u8_explicit_le".to_string(),
        "classic/ct/mono2_i16_rescale_12bit_explicit_le".to_string(),
        "classic/dx/display_shutter_mono2_u16_explicit_le".to_string(),
        "classic/nm/multiframe_explicit_le".to_string(),
        "classic/mr/multislice_oblique_explicit_le".to_string(),
        "vl/photo/rgb_planar0_explicit_le".to_string(),
    ];
    let recipes = RecipeCatalog::load(
        "cases/recipes",
        "cases/registry.json",
        "templates/catalog.json",
    )
    .unwrap();
    let mut expected = selected
        .iter()
        .map(|case_id| {
            let identity = recipes.binding_for_case(case_id).unwrap();
            &recipes.recipes()[identity]
        })
        .flat_map(|recipe| {
            let mut artifacts = recipe
                .dicom
                .as_ref()
                .unwrap()
                .artifacts
                .iter()
                .collect::<Vec<_>>();
            artifacts.sort_by_key(|artifact| artifact.order);
            artifacts.into_iter().map(move |artifact| {
                (
                    recipe.planning_order.unwrap(),
                    artifact.order,
                    format!("curated_{}_{}", recipe.recipe_id, artifact.logical_id),
                    artifact.output.path.clone().unwrap(),
                    recipe.binding.case_id.clone(),
                )
            })
        })
        .collect::<Vec<_>>();
    expected.sort_by_key(|(recipe_order, artifact_order, ..)| (*recipe_order, *artifact_order));

    let bundle = provider()
        .plan(&CuratedScPlanRequest {
            selection: CuratedScSelection::CaseIds(selected),
            seed: 7,
            max_parallelism: 3,
        })
        .unwrap();
    bundle.plan.validate().unwrap();
    bundle.projection.validate(&bundle.plan).unwrap();

    let actual = bundle
        .plan
        .artifacts
        .iter()
        .map(|artifact| {
            let PlannedArtifact::Dicom(artifact) = artifact else {
                panic!("curated classic provider emitted a non-DICOM artifact")
            };
            (
                artifact.logical_id.clone(),
                artifact.output.relative_path.as_str().to_string(),
                artifact.case_binding.as_ref().unwrap().case_id.clone(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        actual,
        expected
            .iter()
            .map(|(_, _, id, path, case_id)| (id.clone(), path.clone(), case_id.clone()))
            .collect::<Vec<_>>()
    );
    let expected_ids = expected
        .iter()
        .map(|(_, _, id, _, _)| id.clone())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        bundle.bindings.keys().cloned().collect::<BTreeSet<_>>(),
        expected_ids
    );
    assert_eq!(
        bundle
            .native_content_requests
            .iter()
            .map(|request| request.artifact_id.clone())
            .collect::<BTreeSet<_>>(),
        expected_ids
    );
    assert_eq!(
        bundle
            .projection
            .artifacts
            .iter()
            .map(|projection| projection.artifact_id.clone())
            .collect::<BTreeSet<_>>(),
        expected_ids
    );
    for artifact in &bundle.plan.artifacts {
        let PlannedArtifact::Dicom(artifact) = artifact else {
            unreachable!()
        };
        assert_eq!(artifact.instance.instance_id, artifact.logical_id);
        assert_eq!(
            artifact.instance.identities.logical_instance_id,
            artifact.logical_id
        );
        assert!(
            bundle.bindings[&artifact.logical_id]
                .slots
                .contains_key(CLASSIC_PIXEL_SLOT)
        );
    }
}
