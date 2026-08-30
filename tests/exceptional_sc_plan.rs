use std::collections::BTreeSet;
use std::fs;

#[cfg(feature = "deflate")]
use dicom_test_suite::composition::Part10Materializer;
use dicom_test_suite::composition::{TemplateCatalog, TemplateId};
use dicom_test_suite::recipes::{
    BackendBoundary, CodecEvidenceRequirement, ExceptionalScEncodingRequest,
    ExceptionalScPlanError, ExceptionalScPlanInput, RecipeCatalog, TransferSyntaxBackendRegistry,
    plan_exceptional_sc,
};
use dicom_test_suite::sha256_hex;
use serde_json::Value;

fn loaded() -> (RecipeCatalog, TemplateCatalog, String) {
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

fn owned_cases() -> BTreeSet<String> {
    let registry: Value =
        serde_json::from_slice(&fs::read("cases/registry.json").unwrap()).unwrap();
    registry["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|case| {
            case["status"] == "implemented"
                && case["case_id"]
                    .as_str()
                    .is_some_and(|id| id.starts_with("classic/sc/"))
                && (case["requirements"]["features"]
                    .as_array()
                    .is_some_and(|values| !values.is_empty())
                    || case["requirements"]["external_codecs"]
                        .as_array()
                        .is_some_and(|values| !values.is_empty()))
        })
        .map(|case| case["case_id"].as_str().unwrap().to_owned())
        .collect()
}

#[test]
fn exceptional_sc_catalog_is_complete_explicit_and_ordered() {
    let (catalog, _, _) = loaded();
    let owned = owned_cases();
    let recipes = catalog
        .recipes()
        .values()
        .filter(|recipe| recipe.plan_provider_id == "native.exceptional_sc_plan")
        .collect::<Vec<_>>();
    assert_eq!(
        recipes
            .iter()
            .map(|recipe| recipe.binding.case_id.clone())
            .collect::<BTreeSet<_>>(),
        owned
    );
    let orders = recipes
        .iter()
        .map(|recipe| recipe.planning_order.unwrap())
        .collect::<BTreeSet<_>>();
    assert_eq!(orders.len(), recipes.len());
    for recipe in recipes {
        let artifact = &recipe.dicom.as_ref().unwrap().artifacts[0];
        assert_eq!(artifact.logical_id, "instance");
        assert_eq!(artifact.order, 0);
        assert_eq!(artifact.content.provider_id, "content.sc.pixel_pattern");
        assert!(artifact.content.parameters.is_empty());
        assert!(artifact.algorithm_provider_id.is_none());
        let expected_path = format!("{}/instance.dcm", recipe.binding.case_id);
        assert_eq!(
            artifact.output.path.as_deref(),
            Some(expected_path.as_str())
        );
        assert_eq!(
            artifact.encoding.preamble_policy.as_deref(),
            Some("zero_filled")
        );
        assert_eq!(
            artifact.encoding.file_meta_policy.as_deref(),
            Some("standard")
        );
        assert!(artifact.secondary_capture.is_some());
    }
}

#[test]
fn exceptional_provider_resolves_every_boundary_before_staging() {
    let (catalog, templates, lock) = loaded();
    let codecs = TransferSyntaxBackendRegistry::load_committed().unwrap();
    let mut observed = [false; 3];
    for case_id in owned_cases() {
        let identity = catalog.binding_for_case(&case_id).unwrap();
        let recipe = catalog.recipes().get(identity).unwrap();
        let artifact = &recipe.dicom.as_ref().unwrap().artifacts[0];
        let reference = artifact.template.as_ref().unwrap();
        let template = templates
            .resolve_qualified(
                &TemplateId(reference.template_id.clone()),
                Some(reference.template_version.parse().unwrap()),
            )
            .unwrap();
        let output = plan_exceptional_sc(ExceptionalScPlanInput {
            recipe,
            artifact,
            template,
            instance_id: &recipe.recipe_id,
            standards_lock_sha256: &lock,
            seed: 1,
        })
        .unwrap_or_else(|error| panic!("{case_id}: {error}"));
        assert_eq!(
            output.instance.transfer_syntax_uid,
            artifact.encoding.transfer_syntax_uid
        );
        assert_eq!(
            output.native_pixels.frames.len(),
            usize::try_from(artifact.secondary_capture.as_ref().unwrap().frames).unwrap()
        );
        let descriptor = codecs
            .for_transfer_syntax(&artifact.encoding.transfer_syntax_uid)
            .unwrap();
        observed[match descriptor.boundary {
            BackendBoundary::DatasetWriter => 0,
            BackendBoundary::EncodedFrames => 1,
            BackendBoundary::LockedFullFileTransform => 2,
        }] = true;
        match (&output.encoding, descriptor.boundary) {
            (ExceptionalScEncodingRequest::Dataset(request), BackendBoundary::DatasetWriter) => {
                assert_eq!(request.backend_id, descriptor.backend_id);
            }
            (
                ExceptionalScEncodingRequest::EncodedFrames(request),
                BackendBoundary::EncodedFrames,
            ) => {
                assert_eq!(request.backend_id, descriptor.backend_id);
                assert_eq!(request.frames.len(), output.native_pixels.frames.len());
                assert_eq!(request.source_transfer_syntax_uid, "1.2.840.10008.1.2.1");
            }
            (
                ExceptionalScEncodingRequest::LockedFullFile(request),
                BackendBoundary::LockedFullFileTransform,
            ) => {
                assert_eq!(request.backend_id, descriptor.backend_id);
                assert_eq!(
                    request.source_plan.transfer_syntax_uid,
                    "1.2.840.10008.1.2.1"
                );
                assert_eq!(request.source_plan.identities, output.instance.identities);
                assert_eq!(request.source_plan.instance_id, output.instance.instance_id);
            }
            mismatch => panic!("boundary mismatch for {case_id}: {mismatch:?}"),
        }
        if case_id.ends_with("lossy") || case_id.contains("jpeg_baseline") {
            assert!(
                output
                    .evidence_requirements
                    .contains(&CodecEvidenceRequirement::LossySampleMetrics)
            );
        } else if descriptor.boundary != BackendBoundary::DatasetWriter {
            assert!(
                output
                    .evidence_requirements
                    .contains(&CodecEvidenceRequirement::ExactDecodedFrameHashes)
            );
        }
    }
    assert_eq!(observed, [true, true, true]);
}

#[test]
fn exceptional_provider_rejects_backend_and_parameter_corruption() {
    let (catalog, templates, lock) = loaded();
    let identity = catalog
        .binding_for_case("classic/sc/rgb_jpegxl_lossy")
        .unwrap();
    let mut recipe = catalog.recipes().get(identity).unwrap().clone();
    let artifact = &recipe.dicom.as_ref().unwrap().artifacts[0];
    let reference = artifact.template.as_ref().unwrap();
    let template = templates
        .resolve_qualified(
            &TemplateId(reference.template_id.clone()),
            Some(reference.template_version.parse().unwrap()),
        )
        .unwrap();
    recipe.dicom.as_mut().unwrap().artifacts[0]
        .parameters
        .insert("surprise".into(), Value::Bool(true));
    let artifact = &recipe.dicom.as_ref().unwrap().artifacts[0];
    assert!(matches!(
        plan_exceptional_sc(ExceptionalScPlanInput {
            recipe: &recipe,
            artifact,
            template,
            instance_id: "poison",
            standards_lock_sha256: &lock,
            seed: 1,
        }),
        Err(ExceptionalScPlanError::Parameters(_))
    ));
}

#[cfg(feature = "deflate")]
#[test]
fn deflated_dataset_plan_has_frozen_seed_one_bytes() {
    let (catalog, templates, lock) = loaded();
    let identity = catalog
        .binding_for_case("classic/sc/mono2_u8_deflated_explicit_le")
        .unwrap();
    let recipe = catalog.recipes().get(identity).unwrap();
    let artifact = &recipe.dicom.as_ref().unwrap().artifacts[0];
    let reference = artifact.template.as_ref().unwrap();
    let template = templates
        .resolve_qualified(
            &TemplateId(reference.template_id.clone()),
            Some(reference.template_version.parse().unwrap()),
        )
        .unwrap();
    let output = plan_exceptional_sc(ExceptionalScPlanInput {
        recipe,
        artifact,
        template,
        instance_id: &recipe.recipe_id,
        standards_lock_sha256: &lock,
        seed: 1,
    })
    .unwrap();
    let path = std::env::temp_dir().join(format!(
        "dts-exceptional-deflated-{}-{}.dcm",
        std::process::id(),
        std::thread::current().name().unwrap_or("test")
    ));
    assert!(!path.exists(), "planning must not create output");
    Part10Materializer
        .materialize(&output.instance, &path)
        .unwrap();
    let bytes = fs::read(&path).unwrap();
    fs::remove_file(&path).unwrap();
    assert_eq!(
        sha256_hex(&bytes),
        "9c780f9b9fb61e458679355c7c5ad2a81b34f62af40a1ca54b9e4027477639e5"
    );
}
