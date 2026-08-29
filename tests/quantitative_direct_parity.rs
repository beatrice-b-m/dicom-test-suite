use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use dicom_core::Tag;
use dicom_test_suite::composition::{
    AttributeAddress, CompositionUidRole, IdentityPlan, ResolvedInstancePlan, TemplateId,
    TemplateVersion,
};
use dicom_test_suite::corpus_plan::{
    ArtifactProvenance, ArtifactResourceEstimate, CaseBinding, EncodingPlan, EvidencePlan,
    FileMetaPolicy, FragmentationPolicy, ImplementationIdentityPlan, ItemLengthPolicy,
    OffsetTablePolicy, OutputPlan, OutputRelativePath, PlannedArtifact, PlannedDicomArtifact,
    PreamblePolicy, SequenceLengthPolicy, ValidationPlan,
};
use dicom_test_suite::executor::materialization::{
    AuxiliaryMaterializationHandler, AuxiliaryPayload, MaterializationDispatcher,
    MaterializationError,
};
use dicom_test_suite::executor::services::{
    ArtifactExecutionBindings, MaterializationRequest, StagedAssetRegistry,
};
use dicom_test_suite::recipes::{
    QUANTITATIVE_NATIVE_PROVIDER_ID, QuantitativeArtifactContext, QuantitativePlanOutput,
    QuantitativePlanProvider, QuantitativeProviderLimits, QuantitativeSourceInput,
    QuantitativeSourceRole, RecipeCatalog, quantitative_input_from_recipe,
};
use dicom_test_suite::sha256_hex;
use serde_json::Value;

const BASELINE: &str = "/tmp/dts-unified-baseline-20260829-52e1d20/all";

struct NoAuxiliary;

impl AuxiliaryMaterializationHandler for NoAuxiliary {
    fn render(
        &self,
        _: &dicom_test_suite::corpus_plan::PlannedAuxiliaryArtifact,
        _: &ArtifactExecutionBindings,
        _: &StagedAssetRegistry,
    ) -> Result<AuxiliaryPayload, MaterializationError> {
        panic!("quantitative recipes contain no auxiliary artifacts")
    }
}

struct TempRoot(PathBuf);

impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn baseline_manifest() -> Value {
    serde_json::from_slice(&fs::read(format!("{BASELINE}/manifest.json")).unwrap()).unwrap()
}

fn file<'a>(manifest: &'a Value, case_id: &str) -> Option<&'a Value> {
    manifest["files"]
        .as_array()
        .unwrap()
        .iter()
        .find(|file| file["case_id"] == case_id)
}

fn identities(logical_id: &str, uids: &Value) -> IdentityPlan {
    let mut values = vec![
        (
            CompositionUidRole::StudyInstance,
            0,
            uids["study_instance_uid"].as_str().unwrap().to_owned(),
        ),
        (
            CompositionUidRole::SeriesInstance,
            0,
            uids["series_instance_uid"].as_str().unwrap().to_owned(),
        ),
        (
            CompositionUidRole::SopInstance,
            0,
            uids["sop_instance_uid"].as_str().unwrap().to_owned(),
        ),
        (
            CompositionUidRole::ImplementationClass,
            0,
            uids["implementation_class_uid"]
                .as_str()
                .unwrap()
                .to_owned(),
        ),
    ];
    for (key, role) in [
        (
            "frame_of_reference_uid",
            CompositionUidRole::FrameOfReference,
        ),
        (
            "dimension_organization_uid",
            CompositionUidRole::DimensionOrganization,
        ),
    ] {
        if let Some(uid) = uids[key].as_str() {
            values.push((role, 0, uid.to_owned()));
        }
    }
    IdentityPlan::from_exact_values(logical_id, values).unwrap()
}

fn source(manifest: &Value) -> QuantitativeSourceInput {
    let source = file(
        manifest,
        "enhanced/ct/multiframe_shared_perframe_explicit_le",
    )
    .unwrap();
    let logical_id = "advanced_enhanced_ct_multiframe_shared_perframe_artifact_1";
    let source_identities = identities(logical_id, &source["uids"]);
    let artifact = PlannedDicomArtifact {
        logical_id: logical_id.into(),
        order: 0,
        provenance: ArtifactProvenance::Requested,
        case_binding: Some(CaseBinding {
            case_id: source["case_id"].as_str().unwrap().into(),
            recipe_id: "enhanced_ct_multiframe_shared_perframe".into(),
            recipe_version: "0.1.0".into(),
        }),
        instance: ResolvedInstancePlan {
            plan_schema_version: "0.1.0".into(),
            instance_id: logical_id.into(),
            template_id: TemplateId("enhanced/ct/multiframe".into()),
            template_version: "1.0.0".parse::<TemplateVersion>().unwrap(),
            sop_class_uid: source["dicom"]["sop_class_uid"].as_str().unwrap().into(),
            transfer_syntax_uid: source["dicom"]["transfer_syntax_uid"]
                .as_str()
                .unwrap()
                .into(),
            identities: source_identities,
            attributes: vec![],
            content: vec![],
            references: vec![],
        },
        output: OutputPlan {
            relative_path: OutputRelativePath::new(source["path"].as_str().unwrap()).unwrap(),
            role: "source".into(),
            publish: true,
        },
        encoding: EncodingPlan {
            transfer_syntax_uid: "1.2.840.10008.1.2.1".into(),
            sequence_length: SequenceLengthPolicy::WriterDefault,
            item_length: ItemLengthPolicy::WriterDefault,
            fragmentation: FragmentationPolicy::Native,
            offset_table: OffsetTablePolicy::NotApplicable,
            preamble: PreamblePolicy::ZeroFilled,
            file_meta: FileMetaPolicy::Standard,
            implementation: ImplementationIdentityPlan {
                class_uid: source["uids"]["implementation_class_uid"]
                    .as_str()
                    .unwrap()
                    .into(),
                version_name: Some("DICOMTS010".into()),
            },
            backend_id: "dicom-rs.part10".into(),
        },
        validation: ValidationPlan { rules: vec![] },
        evidence: EvidencePlan {
            obligations: vec![],
        },
        resources: ArtifactResourceEstimate {
            output_bytes: 1,
            peak_working_bytes: 1,
        },
    };
    QuantitativeSourceInput {
        role: QuantitativeSourceRole::SegmentationSourceImage,
        bindings: ArtifactExecutionBindings {
            artifact_id: logical_id.into(),
            slots: BTreeMap::new(),
        },
        artifact,
        referenced_frames: vec![1, 2],
    }
}

#[test]
fn native_quantitative_plans_match_frozen_seed1_part10_bytes_and_validator_order() {
    assert!(
        PathBuf::from(BASELINE).is_dir(),
        "private seed-1 baseline is required"
    );
    let manifest = baseline_manifest();
    let catalog = RecipeCatalog::load(
        "cases/recipes",
        "cases/registry.json",
        "templates/catalog.json",
    )
    .unwrap();
    let temp = TempRoot(
        std::env::temp_dir().join(format!("dts-quantitative-parity-{}", std::process::id())),
    );
    fs::create_dir_all(&temp.0).unwrap();
    let dispatcher = MaterializationDispatcher::new(&temp.0, Arc::new(NoAuxiliary)).unwrap();
    let assets = StagedAssetRegistry::default();
    let mut materialized = 0;
    let mut feature_gated = 0;
    for recipe in catalog
        .recipes()
        .values()
        .filter(|recipe| recipe.plan_provider_id == QUANTITATIVE_NATIVE_PROVIDER_ID)
    {
        let recipe_artifact = &recipe.dicom.as_ref().unwrap().artifacts[0];
        let Some(baseline) = file(&manifest, &recipe.binding.case_id) else {
            assert_eq!(
                recipe.binding.case_id,
                "derived/seg/binary_multiframe_deflated_image_frame"
            );
            let donor = file(&manifest, "derived/seg/binary_multiframe_explicit_le").unwrap();
            let logical_id = "quantitative_parity_deflated";
            let context = QuantitativeArtifactContext {
                recipe_artifact_logical_id: recipe_artifact.logical_id.clone(),
                target_instance_id: logical_id.into(),
                order: recipe_artifact.order as u64,
                output: OutputPlan {
                    relative_path: OutputRelativePath::new(
                        recipe_artifact.output.path.as_deref().unwrap(),
                    )
                    .unwrap(),
                    role: recipe_artifact.output.role.clone(),
                    publish: true,
                },
                identities: identities(logical_id, &donor["uids"]),
            };
            let input = quantitative_input_from_recipe(recipe, context, vec![source(&manifest)])
                .unwrap()
                .unwrap();
            let QuantitativePlanOutput::Native { artifact, .. } = QuantitativePlanProvider
                .plan(&input, QuantitativeProviderLimits::default())
                .unwrap()
            else {
                panic!("deflated SEG did not return native plan")
            };
            assert_eq!(
                artifact.instance.transfer_syntax_uid,
                "1.2.840.10008.1.2.8.1"
            );
            for tag in [
                Tag(0x0020, 0x9221),
                Tag(0x0020, 0x9222),
                Tag(0x5200, 0x9229),
                Tag(0x5200, 0x9230),
                Tag(0x0008, 0x1115),
            ] {
                let address = AttributeAddress::standard(tag).unwrap();
                assert!(
                    artifact
                        .instance
                        .attributes
                        .iter()
                        .any(|attribute| attribute.address == address),
                    "deflated SEG misses {tag}"
                );
            }
            feature_gated += 1;
            continue;
        };
        let logical_id = format!("quantitative_parity_{materialized}");
        let context = QuantitativeArtifactContext {
            recipe_artifact_logical_id: recipe_artifact.logical_id.clone(),
            target_instance_id: logical_id.clone(),
            order: recipe_artifact.order as u64,
            output: OutputPlan {
                relative_path: OutputRelativePath::new(baseline["path"].as_str().unwrap()).unwrap(),
                role: recipe_artifact.output.role.clone(),
                publish: true,
            },
            identities: identities(&logical_id, &baseline["uids"]),
        };
        let mut source = source(&manifest);
        source.role = if recipe.binding.case_id.starts_with("derived/rwvm/") {
            QuantitativeSourceRole::RealWorldValueSourceImage
        } else {
            QuantitativeSourceRole::SegmentationSourceImage
        };
        let input = quantitative_input_from_recipe(recipe, context, vec![source])
            .unwrap()
            .unwrap();
        let QuantitativePlanOutput::Native {
            artifact, bindings, ..
        } = QuantitativePlanProvider
            .plan(&input, QuantitativeProviderLimits::default())
            .unwrap()
        else {
            panic!("native recipe returned external boundary")
        };
        dispatcher
            .dispatch(
                &MaterializationRequest {
                    artifact: PlannedArtifact::Dicom(artifact),
                    bindings,
                },
                &assets,
            )
            .unwrap();
        let actual = fs::read(temp.0.join(baseline["path"].as_str().unwrap())).unwrap();
        let expected =
            fs::read(PathBuf::from(BASELINE).join(baseline["path"].as_str().unwrap())).unwrap();
        assert_eq!(
            actual,
            expected,
            "Part 10 mismatch for {} (actual sha256 {})",
            recipe.binding.case_id,
            sha256_hex(&actual)
        );
        let validation_names = baseline["validation"]["internal"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| row["name"].as_str().unwrap())
            .collect::<Vec<_>>();
        if recipe.binding.case_id.starts_with("derived/seg/") {
            assert!(validation_names.ends_with(&[
                "segmentation_source_image_sop_class_uid",
                "segmentation_source_image_sop_instance_uid",
                "segmentation_source_image_frame_number",
                "curated_composition_plan"
            ]));
        } else {
            assert!(validation_names.contains(&"rwvm_pixel_data_absent"));
        }
        materialized += 1;
    }
    assert_eq!(materialized, 4);
    assert_eq!(feature_gated, 1);
}
