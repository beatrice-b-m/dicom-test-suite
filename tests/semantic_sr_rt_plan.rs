use std::collections::BTreeMap;

use dicom_test_suite::composition::{
    CompositionUidRole, IdentityPlan, MaterializedReference, TemplateVersion,
};
use dicom_test_suite::corpus_plan::{
    ArtifactResourceEstimate, EncodingPlan, FileMetaPolicy, FragmentationPolicy,
    ImplementationIdentityPlan, ItemLengthPolicy, OffsetTablePolicy, OutputPlan,
    OutputRelativePath, PreamblePolicy, SequenceLengthPolicy,
};
use dicom_test_suite::planning::RecipeIdentity;
use dicom_test_suite::recipes::{
    CodedConcept, CompletionFlag, ContentProviderRequest, DoseParameters,
    HIGH_DICOM_SR_IMPORT_PROVIDER_ID, RecipeCatalog, RtDocumentParameters, RtObjectParameters,
    RtPlanInput, RtPlanProvider, RtSourceDeclaration, SR_PLAN_PROVIDER_ID, SemanticPlanContext,
    SemanticSource, SrDocumentKind, SrDocumentParameters, SrPlanInput, SrPlanProvider,
    SrSourceDeclaration, VerificationFlag,
};

fn context(logical_id: &str, sources: Vec<SemanticSource>) -> SemanticPlanContext {
    let implementation = "1.2.826.0.1.3680043.10.543.1".to_string();
    SemanticPlanContext {
        case_id: format!("test/{logical_id}"),
        recipe: RecipeIdentity {
            recipe_id: format!("{logical_id}_recipe"),
            recipe_version: "0.1.0".into(),
        },
        logical_id: logical_id.into(),
        order: 1,
        output: OutputPlan {
            role: "primary".into(),
            relative_path: OutputRelativePath::new(format!("test/{logical_id}/instance.dcm"))
                .unwrap(),
            publish: true,
        },
        template_id: "test/semantic".into(),
        template_version: "1.0.0".into(),
        identities: IdentityPlan::from_exact_values(
            logical_id,
            [
                (
                    CompositionUidRole::ImplementationClass,
                    0,
                    implementation.clone(),
                ),
                (
                    CompositionUidRole::SopInstance,
                    0,
                    "2.25.700000000000000000000000000000000001".into(),
                ),
                (
                    CompositionUidRole::StudyInstance,
                    0,
                    "2.25.700000000000000000000000000000000002".into(),
                ),
                (
                    CompositionUidRole::SeriesInstance,
                    0,
                    "2.25.700000000000000000000000000000000003".into(),
                ),
            ],
        )
        .unwrap(),
        encoding: EncodingPlan {
            transfer_syntax_uid: "1.2.840.10008.1.2.1".into(),
            sequence_length: SequenceLengthPolicy::WriterDefault,
            item_length: ItemLengthPolicy::WriterDefault,
            fragmentation: FragmentationPolicy::Native,
            offset_table: OffsetTablePolicy::NotApplicable,
            preamble: PreamblePolicy::ZeroFilled,
            file_meta: FileMetaPolicy::Standard,
            implementation: ImplementationIdentityPlan {
                class_uid: implementation,
                version_name: Some("DTS_TEST".into()),
            },
            backend_id: "dicom-rs.part10".into(),
        },
        base_attributes: vec![],
        sources,
        resources: ArtifactResourceEstimate {
            output_bytes: 1024 * 1024,
            peak_working_bytes: 2 * 1024 * 1024,
        },
    }
}

fn source(owner: &str, artifact: &str, role: &str) -> SemanticSource {
    SemanticSource {
        recipe: RecipeIdentity {
            recipe_id: "source_recipe".into(),
            recipe_version: "0.1.0".into(),
        },
        recipe_artifact_logical_id: artifact.into(),
        artifact_id: artifact.into(),
        role: role.into(),
        study_instance_uid: "2.25.20".into(),
        series_instance_uid: "2.25.21".into(),
        reference: MaterializedReference {
            source_instance_id: owner.into(),
            target_instance_id: artifact.into(),
            role: role.into(),
            frame_role: None,
            referenced_sop_class_uid: "1.2.840.10008.5.1.4.1.1.2.1".into(),
            referenced_sop_instance_uid: "2.25.800000000000000000000000000000000001".into(),
            referenced_frames: vec![1, 2],
        },
    }
}

#[test]
fn native_sr_plan_is_deterministic_and_rejects_source_role_drift() {
    let semantic_source = source("sr", "source", "source_image");
    let input = SrPlanInput {
        parameters: SrDocumentParameters {
            series_number: "63".into(),
            instance_number: 1,
            content_date: "20260101".into(),
            content_time: "000000".into(),
            completion_flag: CompletionFlag::Complete,
            verification_flag: VerificationFlag::Unverified,
            continuity_of_content: "SEPARATE".into(),
            title: CodedConcept {
                code_value: "18748-4".into(),
                coding_scheme_designator: "LN".into(),
                code_meaning: "Diagnostic imaging study".into(),
            },
            document: SrDocumentKind::BasicText {
                observation: CodedConcept {
                    code_value: "121106".into(),
                    coding_scheme_designator: "DCM".into(),
                    code_meaning: "Comment".into(),
                },
                observation_text: "Synthetic observation".into(),
            },
            sources: vec![SrSourceDeclaration {
                recipe: dicom_test_suite::recipes::RecipeReference {
                    recipe_id: "source_recipe".into(),
                    recipe_version: "0.1.0".into(),
                },
                artifact_logical_id: "source".into(),
                role: "source_image".into(),
                referenced_frames: vec![1, 2],
            }],
        },
        context: context("sr", vec![semantic_source]),
    };
    let first = SrPlanProvider.plan_native(&input).unwrap();
    let second = SrPlanProvider.plan_native(&input).unwrap();
    assert_eq!(first, second);
    assert!(first.artifact.instance.content.is_empty());
    assert!(
        first
            .artifact
            .instance
            .attributes
            .iter()
            .any(|attribute| { attribute.address.normalized_tag() == "0040,A730" })
    );

    let mut invalid = input;
    invalid.context.sources[0].role = "wrong".into();
    assert!(SrPlanProvider.plan_native(&invalid).is_err());
}

#[test]
fn rt_dose_plan_uses_neutral_pixels_and_checked_dimensions() {
    let sources = vec![
        source("dose", "image", "source_image"),
        source("dose", "structure", "referenced_structure_set"),
    ];
    let declarations = sources
        .iter()
        .map(|source| RtSourceDeclaration {
            recipe: dicom_test_suite::recipes::RecipeReference {
                recipe_id: source.recipe.recipe_id.clone(),
                recipe_version: source.recipe.recipe_version.clone(),
            },
            artifact_logical_id: source.artifact_id.clone(),
            role: source.role.clone(),
        })
        .collect();
    let input = RtPlanInput {
        parameters: RtDocumentParameters {
            series_number: "71".into(),
            instance_number: 1,
            label: "DTS_DOSE".into(),
            object: RtObjectParameters::Dose(DoseParameters {
                rows: 2,
                columns: 2,
                frames: 2,
                stored_values: vec![0, 100, 200, 300, 400, 500, 600, 700],
                pixel_spacing: ["1".into(), "1".into()],
                image_orientation_patient: [
                    "1".into(),
                    "0".into(),
                    "0".into(),
                    "0".into(),
                    "1".into(),
                    "0".into(),
                ],
                image_position_patient: ["0".into(), "0".into(), "0".into()],
                slice_thickness: "2.5".into(),
                grid_frame_offset_vector: vec!["0".into(), "2.5".into()],
                dose_units: "GY".into(),
                dose_type: "PHYSICAL".into(),
                dose_summation_type: "RECORD".into(),
                dose_grid_scaling: "0.001".into(),
            }),
            sources: declarations,
        },
        context: context("dose", sources),
    };
    let output = RtPlanProvider.plan(&input).unwrap();
    assert_eq!(output.artifact.instance.content.len(), 1);
    assert_eq!(output.artifact.instance.content[0].size_bytes, 16);
    assert!(matches!(
        output.artifact.instance.content[0].materialization,
        Some(dicom_test_suite::composition::ContentMaterialization::Inline(_))
    ));

    let mut invalid = input;
    let RtObjectParameters::Dose(parameters) = &mut invalid.parameters.object else {
        unreachable!()
    };
    parameters.frames = 3;
    assert!(RtPlanProvider.plan(&invalid).is_err());
}

#[test]
fn semantic_provider_sources_have_no_filesystem_or_frontend_dependencies() {
    let _ = BTreeMap::<String, ContentProviderRequest>::new();
    let _ = "1.0.0".parse::<TemplateVersion>().unwrap();
    for path in [
        "src/recipes/semantic.rs",
        "src/recipes/sr.rs",
        "src/recipes/rt.rs",
    ] {
        let source = std::fs::read_to_string(path).unwrap();
        for forbidden in [
            "std::fs",
            "std::path",
            "crate::generator",
            "crate::cli",
            "open_file",
            "write_to_file",
        ] {
            assert!(!source.contains(forbidden), "{path} contains {forbidden}");
        }
    }
}

#[test]
fn sr_recipe_documents_are_typed_ordered_and_dependency_complete() {
    let catalog = RecipeCatalog::load(
        "cases/recipes",
        "cases/registry.json",
        "templates/catalog.json",
    )
    .unwrap();
    let recipes = catalog
        .recipes()
        .values()
        .filter(|recipe| {
            matches!(
                recipe.plan_provider_id.as_str(),
                SR_PLAN_PROVIDER_ID | HIGH_DICOM_SR_IMPORT_PROVIDER_ID
            )
        })
        .collect::<Vec<_>>();
    assert!(!recipes.is_empty());
    let orders = recipes
        .iter()
        .map(|recipe| recipe.planning_order.unwrap())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(orders.len(), recipes.len());
    for recipe in recipes {
        let parameters: SrDocumentParameters = serde_json::from_value(serde_json::Value::Object(
            recipe.provider_parameters.clone(),
        ))
        .unwrap();
        assert_eq!(parameters.sources.len(), recipe.dependencies.len());
        assert!(parameters.sources.iter().all(|source| {
            recipe.dependencies.iter().any(|dependency| {
                dependency.recipe == source.recipe && dependency.role == source.role
            })
        }));
        let [artifact] = recipe.dicom.as_ref().unwrap().artifacts.as_slice() else {
            panic!("SR recipes have one output")
        };
        assert!(artifact.output.path.is_some());
        if recipe.plan_provider_id == HIGH_DICOM_SR_IMPORT_PROVIDER_ID {
            let boundary = match parameters.document {
                SrDocumentKind::Comprehensive3d { import, .. }
                | SrDocumentKind::Tid1500 { import, .. } => import,
                _ => panic!("external SR recipe must declare an import boundary"),
            };
            assert_eq!(boundary.provider_id, "highdicom_pydicom");
            assert_eq!(
                boundary.tool_fingerprint_policy,
                "runtime_composite_sha256_required"
            );
            assert_eq!(boundary.dependency_sha256.len(), 64);
            assert!(!boundary.semantic_evidence.is_empty());
        } else {
            assert!(matches!(
                parameters.document,
                SrDocumentKind::BasicText { .. }
                    | SrDocumentKind::Comprehensive { .. }
                    | SrDocumentKind::KeyObjectSelection { .. }
            ));
        }
    }
}

#[test]
fn rt_recipe_documents_form_one_typed_ordered_reference_dag() {
    let catalog = RecipeCatalog::load(
        "cases/recipes",
        "cases/registry.json",
        "templates/catalog.json",
    )
    .unwrap();
    let recipes = catalog
        .recipes()
        .values()
        .filter(|recipe| recipe.plan_provider_id == dicom_test_suite::recipes::RT_PLAN_PROVIDER_ID)
        .collect::<Vec<_>>();
    assert!(!recipes.is_empty());
    assert_eq!(
        recipes
            .iter()
            .map(|recipe| recipe.planning_order.unwrap())
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        recipes.len()
    );
    for recipe in recipes {
        let parameters: RtDocumentParameters = serde_json::from_value(serde_json::Value::Object(
            recipe.provider_parameters.clone(),
        ))
        .unwrap();
        assert_eq!(parameters.sources.len(), recipe.dependencies.len());
        assert!(parameters.sources.iter().all(|source| {
            recipe.dependencies.iter().any(|dependency| {
                dependency.recipe == source.recipe && dependency.role == source.role
            })
        }));
        let [artifact] = recipe.dicom.as_ref().unwrap().artifacts.as_slice() else {
            panic!("RT recipes have one output")
        };
        assert_eq!(artifact.content.provider_id, "content.rt_semantics");
        assert_eq!(
            artifact.algorithm_provider_id.as_deref(),
            Some("algorithm.rt_semantics")
        );
        assert!(artifact.output.path.is_some());
    }
}
