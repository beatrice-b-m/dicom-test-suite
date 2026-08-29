use std::collections::{BTreeMap, BTreeSet};

use dicom_test_suite::composition::{
    CompositionUidRole, IdentityPlan, ResolvedInstancePlan, TemplateId, TemplateVersion,
};
use dicom_test_suite::corpus_plan::{
    ArtifactProvenance, ArtifactResourceEstimate, CaseBinding, EncodingPlan, EvidencePlan,
    FileMetaPolicy, FragmentationPolicy, ImplementationIdentityPlan, ItemLengthPolicy,
    OffsetTablePolicy, OutputPlan, OutputRelativePath, PlannedDicomArtifact, PreamblePolicy,
    SequenceLengthPolicy, ValidationPlan,
};
use dicom_test_suite::executor::services::ArtifactExecutionBindings;
use dicom_test_suite::recipes::{
    ExternalDependencyContract, ExternalImportBoundary, ExternalImportKind,
    ExternalSemanticEvidence, QUANTITATIVE_EXTERNAL_PROVIDER_ID, QUANTITATIVE_NATIVE_PROVIDER_ID,
    QuantitativeArtifactContext, QuantitativePlanInput, QuantitativePlanOutput,
    QuantitativePlanProvider, QuantitativeProviderLimits, QuantitativeSourceInput,
    QuantitativeSourceRole, RecipeCatalog, RecipeIdentity, SegmentationInput, SegmentationKind,
    quantitative_input_from_recipe,
};

fn identities(owner: &str, suffix: &str) -> IdentityPlan {
    IdentityPlan::from_exact_values(
        owner,
        [
            (
                CompositionUidRole::StudyInstance,
                0,
                format!("2.25.100{suffix}"),
            ),
            (
                CompositionUidRole::SeriesInstance,
                0,
                format!("2.25.200{suffix}"),
            ),
            (
                CompositionUidRole::SopInstance,
                0,
                format!("2.25.300{suffix}"),
            ),
            (
                CompositionUidRole::FrameOfReference,
                0,
                format!("2.25.400{suffix}"),
            ),
            (
                CompositionUidRole::ImplementationClass,
                0,
                "2.25.999".into(),
            ),
        ],
    )
    .unwrap()
}

fn encoding() -> EncodingPlan {
    EncodingPlan {
        transfer_syntax_uid: "1.2.840.10008.1.2.1".into(),
        sequence_length: SequenceLengthPolicy::WriterDefault,
        item_length: ItemLengthPolicy::WriterDefault,
        fragmentation: FragmentationPolicy::Native,
        offset_table: OffsetTablePolicy::NotApplicable,
        preamble: PreamblePolicy::ZeroFilled,
        file_meta: FileMetaPolicy::Standard,
        implementation: ImplementationIdentityPlan {
            class_uid: "2.25.999".into(),
            version_name: Some("DICOMTS010".into()),
        },
        backend_id: "dicom-rs.part10".into(),
    }
}

fn source(id: &str, recipe_id: &str, role: QuantitativeSourceRole) -> QuantitativeSourceInput {
    let artifact = PlannedDicomArtifact {
        logical_id: id.into(),
        order: 10,
        provenance: ArtifactProvenance::Requested,
        case_binding: Some(CaseBinding {
            case_id: format!("source/{id}"),
            recipe_id: recipe_id.into(),
            recipe_version: "0.1.0".into(),
        }),
        instance: ResolvedInstancePlan {
            plan_schema_version: "0.1.0".into(),
            instance_id: id.into(),
            template_id: TemplateId("source/template".into()),
            template_version: "1.0.0".parse::<TemplateVersion>().unwrap(),
            sop_class_uid: "1.2.840.10008.5.1.4.1.1.2.1".into(),
            transfer_syntax_uid: "1.2.840.10008.1.2.1".into(),
            identities: identities(id, "1"),
            attributes: Vec::new(),
            content: Vec::new(),
            references: Vec::new(),
        },
        output: OutputPlan {
            relative_path: OutputRelativePath::new(format!("source/{id}.dcm")).unwrap(),
            role: "source".into(),
            publish: true,
        },
        encoding: encoding(),
        validation: ValidationPlan { rules: Vec::new() },
        evidence: EvidencePlan {
            obligations: Vec::new(),
        },
        resources: ArtifactResourceEstimate {
            output_bytes: 1024,
            peak_working_bytes: 2048,
        },
    };
    QuantitativeSourceInput {
        bindings: ArtifactExecutionBindings {
            artifact_id: id.into(),
            slots: BTreeMap::new(),
        },
        artifact,
        role,
        referenced_frames: vec![1, 2],
    }
}

fn context() -> QuantitativeArtifactContext {
    QuantitativeArtifactContext {
        recipe_artifact_logical_id: "segmentation".into(),
        target_instance_id: "caller_target".into(),
        order: 2,
        output: OutputPlan {
            relative_path: OutputRelativePath::new("derived/seg/test/instance.dcm").unwrap(),
            role: "segmentation".into(),
            publish: true,
        },
        identities: identities("caller_target", "2"),
    }
}

fn binary_input() -> QuantitativePlanInput {
    QuantitativePlanInput::NativeSeg {
        recipe: RecipeIdentity {
            recipe_id: "seg_binary_multiframe".into(),
            recipe_version: "0.1.0".into(),
        },
        case_id: "derived/seg/binary_multiframe_explicit_le".into(),
        artifact: context(),
        segmentation: SegmentationInput {
            kind: SegmentationKind::Binary,
            rows: 2,
            columns: 2,
            frames: 2,
            transfer_syntax_uid: "1.2.840.10008.1.2.1".into(),
            segment_label: "DTS_SYNTHETIC_REGION".into(),
            segment_number: 1,
            stored_values: vec![1, 0, 0, 1, 0, 1, 1, 0],
            visual_pattern: "two_frame_binary_segmentation_mask".into(),
        },
        sources: vec![source(
            "source_ct",
            "enhanced_ct_multiframe_shared_perframe",
            QuantitativeSourceRole::SegmentationSourceImage,
        )],
    }
}

#[test]
fn native_seg_preserves_caller_context_and_expands_bounded_content() {
    let output = QuantitativePlanProvider
        .plan(&binary_input(), QuantitativeProviderLimits::default())
        .unwrap();
    let QuantitativePlanOutput::Native {
        artifact,
        dependencies,
        ..
    } = output
    else {
        panic!("expected native output")
    };
    assert_eq!(artifact.logical_id, "caller_target");
    assert_eq!(artifact.order, 2);
    assert_eq!(
        artifact.output.relative_path.as_str(),
        "derived/seg/test/instance.dcm"
    );
    assert_eq!(artifact.instance.identities, context().identities);
    assert_eq!(dependencies[0].depends_on, "source_ct");
    assert_eq!(dependencies[0].frame_numbers, [1, 2]);
    assert_eq!(artifact.instance.content[0].size_bytes, 2);
    assert_eq!(artifact.instance.references[0].referenced_frames, [1, 2]);
}

#[test]
fn native_seg_rejects_bad_source_role_and_resource_bounds() {
    let mut input = binary_input();
    let QuantitativePlanInput::NativeSeg { sources, .. } = &mut input else {
        unreachable!()
    };
    sources[0].role = QuantitativeSourceRole::RealWorldValueSourceImage;
    assert!(
        QuantitativePlanProvider
            .plan(&input, QuantitativeProviderLimits::default())
            .is_err()
    );

    let mut input = binary_input();
    let QuantitativePlanInput::NativeSeg { segmentation, .. } = &mut input else {
        unreachable!()
    };
    segmentation.frames = 257;
    assert!(
        QuantitativePlanProvider
            .plan(&input, QuantitativeProviderLimits::default())
            .is_err()
    );
}

#[test]
fn fractional_segmentation_selects_its_qualified_template() {
    let mut input = binary_input();
    let QuantitativePlanInput::NativeSeg { segmentation, .. } = &mut input else {
        unreachable!()
    };
    segmentation.kind = SegmentationKind::FractionalProbability;
    segmentation.stored_values = vec![0, 64, 128, 255, 255, 128, 64, 0];
    let QuantitativePlanOutput::Native { artifact, .. } = QuantitativePlanProvider
        .plan(&input, QuantitativeProviderLimits::default())
        .unwrap()
    else {
        panic!("expected native output")
    };
    assert_eq!(
        artifact.instance.template_id.0,
        "derived/segmentation/fractional-probability"
    );
}

#[test]
fn catalog_quantitative_slice_is_explicit_unique_and_bounded() {
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
                QUANTITATIVE_NATIVE_PROVIDER_ID | QUANTITATIVE_EXTERNAL_PROVIDER_ID
            )
        })
        .collect::<Vec<_>>();
    assert!(!recipes.is_empty());
    let mut orders = BTreeSet::new();
    for recipe in recipes {
        assert!(orders.insert(recipe.planning_order.unwrap()));
        assert_eq!(recipe.validation_rule_ids.len(), 1);
        assert_eq!(recipe.projection_rule_ids, ["projection.quantitative"]);
        let dicom = recipe.dicom.as_ref().unwrap();
        assert_eq!(dicom.artifacts.len(), 1);
        let artifact = &dicom.artifacts[0];
        assert_eq!(artifact.content.provider_id, "content.neutral");
        assert_eq!(
            artifact.algorithm_provider_id.as_deref(),
            Some("algorithm.quantitative")
        );
        assert!(
            artifact
                .output
                .path
                .as_deref()
                .is_some_and(|path| path.ends_with(".dcm"))
        );
        assert!(!recipe.dependencies.is_empty());
        let sources = recipe.provider_parameters["sources"].as_array().unwrap();
        let source_recipes = sources
            .iter()
            .map(|source| {
                (
                    source["recipe"]["recipe_id"].as_str().unwrap(),
                    source["recipe"]["recipe_version"].as_str().unwrap(),
                )
            })
            .collect::<BTreeSet<_>>();
        let dependency_recipes = recipe
            .dependencies
            .iter()
            .map(|dependency| {
                (
                    dependency.recipe.recipe_id.as_str(),
                    dependency.recipe.recipe_version.as_str(),
                )
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(source_recipes, dependency_recipes);
        if recipe.plan_provider_id == QUANTITATIVE_EXTERNAL_PROVIDER_ID {
            let external = &recipe.provider_parameters["import"];
            assert_eq!(
                external["dependency"]["executable_provider_id"],
                "highdicom_pydicom"
            );
            assert_eq!(external["dependency"]["required_tool_version"], "0.5.0");
            assert_eq!(external["dependency"]["protocol_version"], "0.1.0");
            assert!(external["maximum_output_bytes"].as_u64().unwrap() > 0);
            assert!(external["timeout_seconds"].as_u64().unwrap() > 0);
            assert!(
                !external["semantic_evidence"]["required_validation_names"]
                    .as_array()
                    .unwrap()
                    .is_empty()
            );
        }
    }
}

#[test]
fn external_import_boundary_is_typed_bounded_and_source_closed() {
    let import = ExternalImportBoundary {
        kind: ExternalImportKind::WholeSlideTileSegmentation,
        request_id: "wsi_tile_segmentation".into(),
        output_media_type: "application/dicom".into(),
        maximum_output_bytes: 16 * 1024,
        timeout_seconds: 5,
        dependency: ExternalDependencyContract {
            executable_provider_id: "highdicom_pydicom".into(),
            required_tool_version: "0.5.0".into(),
            dependency_lock_sha256:
                "253612f2a540d29071556c238e15abeb00929167e348edd6fa15e267e5189378".into(),
            protocol_version: "0.1.0".into(),
        },
        semantic_evidence: ExternalSemanticEvidence {
            sop_class_uid: "1.2.840.10008.5.1.4.1.1.66.4".into(),
            transfer_syntax_uid: "1.2.840.10008.1.2.1".into(),
            pixel_vr: "OB".into(),
            frame_count: 2,
            rows: 2,
            columns: 2,
            source_frame_numbers: vec![1, 4],
            required_validation_names: vec!["wsi_tile_segmentation_pixel_payload".into()],
        },
    };
    let input = QuantitativePlanInput::ExternalImport {
        recipe: RecipeIdentity {
            recipe_id: "derived_seg_wsi_tile_reference".into(),
            recipe_version: "0.1.0".into(),
        },
        case_id: "derived/seg/wsi_tile_reference".into(),
        artifact: context(),
        import: import.clone(),
        sources: vec![source(
            "wsi_source",
            "vl_wsi_tiled_full_small",
            QuantitativeSourceRole::WholeSlideSourceImage,
        )],
    };
    let output = QuantitativePlanProvider
        .plan(&input, QuantitativeProviderLimits::default())
        .unwrap();
    let QuantitativePlanOutput::ExternalImport {
        dependencies,
        references,
        ..
    } = output
    else {
        panic!("expected import boundary")
    };
    assert_eq!(dependencies.len(), 1);
    assert_eq!(references.len(), 1);

    let mut invalid = input;
    let QuantitativePlanInput::ExternalImport { import, .. } = &mut invalid else {
        unreachable!()
    };
    import.dependency.dependency_lock_sha256 = "unlocked".into();
    assert!(
        QuantitativePlanProvider
            .plan(&invalid, QuantitativeProviderLimits::default())
            .is_err()
    );
}

#[test]
fn catalog_document_strictly_builds_typed_native_input() {
    let catalog = RecipeCatalog::load(
        "cases/recipes",
        "cases/registry.json",
        "templates/catalog.json",
    )
    .unwrap();
    let identity = catalog
        .binding_for_case("derived/seg/binary_multiframe_explicit_le")
        .unwrap();
    let recipe = &catalog.recipes()[identity];
    let recipe_artifact = &recipe.dicom.as_ref().unwrap().artifacts[0];
    let artifact = QuantitativeArtifactContext {
        recipe_artifact_logical_id: recipe_artifact.logical_id.clone(),
        target_instance_id: "typed_catalog_target".into(),
        order: recipe_artifact.order as u64,
        output: OutputPlan {
            relative_path: OutputRelativePath::new(recipe_artifact.output.path.as_deref().unwrap())
                .unwrap(),
            role: recipe_artifact.output.role.clone(),
            publish: true,
        },
        identities: identities("typed_catalog_target", "7"),
    };
    let source = source(
        "advanced_enhanced_ct_multiframe_shared_perframe_artifact_1",
        "enhanced_ct_multiframe_shared_perframe",
        QuantitativeSourceRole::SegmentationSourceImage,
    );
    let typed = quantitative_input_from_recipe(recipe, artifact.clone(), vec![source.clone()])
        .unwrap()
        .unwrap();
    assert!(matches!(typed, QuantitativePlanInput::NativeSeg { .. }));

    let mut malformed = recipe.clone();
    malformed
        .provider_parameters
        .insert("untyped_escape_hatch".into(), serde_json::json!(true));
    assert!(quantitative_input_from_recipe(&malformed, artifact, vec![source]).is_err());
}
