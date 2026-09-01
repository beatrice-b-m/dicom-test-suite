use dicom_test_suite::corpus_plan::{
    FileMetaPolicy, FragmentationPolicy, ImplementationIdentityPlan, ItemLengthPolicy,
    OffsetTablePolicy, PreamblePolicy, SequenceLengthPolicy,
};
use dicom_test_suite::recipes::{
    EncodingPolicy, RecipeCatalog, RecipeEncodingError, encoding_plan_from_recipe,
};

fn implementation() -> ImplementationIdentityPlan {
    ImplementationIdentityPlan {
        class_uid: "2.25.100".into(),
        version_name: Some("DICOMTS010".into()),
    }
}

fn policy() -> EncodingPolicy {
    EncodingPolicy {
        transfer_syntax_uid: "1.2.840.10008.1.2.1".into(),
        sequence_length_policy: "default".into(),
        item_length_policy: "default".into(),
        offset_table_policy: "none".into(),
        fragmentation_policy: "native".into(),
        fragments_per_frame: None,
        preamble_policy: Some("zero_filled".into()),
        file_meta_policy: Some("standard".into()),
        non_template_encoding_provider_id: None,
    }
}

#[test]
fn adapter_maps_each_concrete_recipe_policy_to_distinct_typed_fields() {
    let mut input = policy();
    input.sequence_length_policy = "defined".into();
    input.item_length_policy = "undefined".into();
    input.preamble_policy = Some("deterministic_nonzero".into());
    let plan = encoding_plan_from_recipe(&input, implementation()).unwrap();
    assert_eq!(plan.sequence_length, SequenceLengthPolicy::Defined);
    assert_eq!(plan.item_length, ItemLengthPolicy::Undefined);
    assert_eq!(plan.fragmentation, FragmentationPolicy::Native);
    assert_eq!(plan.offset_table, OffsetTablePolicy::NotApplicable);
    assert_eq!(plan.preamble, PreamblePolicy::DeterministicNonZero);
    assert_eq!(plan.file_meta, FileMetaPolicy::Standard);
    assert_eq!(plan.backend_id, "dicom-rs.part10");

    let mut input = policy();
    input.transfer_syntax_uid = "1.2.840.10008.1.2.5".into();
    input.fragmentation_policy = "one_per_frame".into();
    input.non_template_encoding_provider_id = Some("encoding.native.rle_lossless".into());
    for (value, expected) in [
        ("empty_basic", OffsetTablePolicy::EmptyBasic),
        ("populated_basic", OffsetTablePolicy::PopulatedBasic),
        ("extended", OffsetTablePolicy::Extended),
    ] {
        input.offset_table_policy = value.into();
        assert_eq!(
            encoding_plan_from_recipe(&input, implementation())
                .unwrap()
                .offset_table,
            expected
        );
    }

    let mut input = policy();
    input.transfer_syntax_uid = "1.2.840.10008.1.2.4.50".into();
    input.offset_table_policy = "populated_basic".into();
    input.fragmentation_policy = "fixed_per_frame".into();
    input.fragments_per_frame = Some(2);
    input.non_template_encoding_provider_id = Some("encoding.dicom_rs.jpeg_baseline".into());
    assert_eq!(
        encoding_plan_from_recipe(&input, implementation())
            .unwrap()
            .fragmentation,
        FragmentationPolicy::FixedFragmentsPerFrame {
            fragments_per_frame: 2
        }
    );
}

#[test]
fn adapter_rejects_every_unresolved_provider_field_before_plan_construction() {
    for field in [
        "sequence_length_policy",
        "item_length_policy",
        "offset_table_policy",
        "fragmentation_policy",
        "preamble_policy",
        "file_meta_policy",
        "non_template_encoding_provider_id",
    ] {
        let mut input = policy();
        match field {
            "sequence_length_policy" => input.sequence_length_policy = "provider".into(),
            "item_length_policy" => input.item_length_policy = "provider".into(),
            "offset_table_policy" => input.offset_table_policy = "provider".into(),
            "fragmentation_policy" => input.fragmentation_policy = "provider".into(),
            "preamble_policy" => input.preamble_policy = Some("provider".into()),
            "file_meta_policy" => input.file_meta_policy = Some("provider".into()),
            "non_template_encoding_provider_id" => {
                input.non_template_encoding_provider_id = Some("provider".into())
            }
            _ => unreachable!(),
        }
        assert!(matches!(
            encoding_plan_from_recipe(&input, implementation()),
            Err(RecipeEncodingError::UnresolvedProviderPolicy(actual)) if actual == field
        ));
    }
}

#[test]
fn catalog_concrete_sc_and_metadata_policies_all_convert_and_legacy_bridges_do_not() {
    let catalog = RecipeCatalog::load(
        "cases/recipes",
        "cases/registry.json",
        "templates/catalog.json",
    )
    .unwrap();
    let mut converted = 0_usize;
    let mut rejected_provider = 0_usize;
    let mut provider_parameterized = 0_usize;
    for recipe in catalog.recipes().values() {
        let Some(dicom) = &recipe.dicom else {
            continue;
        };
        for artifact in &dicom.artifacts {
            let fields = [
                Some(artifact.encoding.sequence_length_policy.as_str()),
                Some(artifact.encoding.item_length_policy.as_str()),
                Some(artifact.encoding.offset_table_policy.as_str()),
                Some(artifact.encoding.fragmentation_policy.as_str()),
                artifact.encoding.preamble_policy.as_deref(),
                artifact.encoding.file_meta_policy.as_deref(),
            ];
            let unresolved = fields
                .into_iter()
                .flatten()
                .any(|value| value == "provider");
            let result = encoding_plan_from_recipe(&artifact.encoding, implementation());
            if artifact.encoding.fragmentation_policy == "bounded_fragments" {
                assert!(matches!(
                    result,
                    Err(RecipeEncodingError::MissingFragmentMaximum)
                ));
                provider_parameterized += 1;
            } else if unresolved {
                assert!(matches!(
                    result,
                    Err(RecipeEncodingError::UnresolvedProviderPolicy(_))
                ));
                rejected_provider += 1;
            } else {
                result.unwrap_or_else(|error| {
                    panic!(
                        "{} artifact {} has an invalid concrete encoding policy: {error}",
                        recipe.recipe_id, artifact.logical_id
                    )
                });
                converted += 1;
            }
        }
    }
    assert!(
        converted > 0,
        "the catalog must exercise concrete U3 policies"
    );
    assert!(
        rejected_provider > 0,
        "the catalog must retain explicit future provider bridges"
    );
    assert!(
        provider_parameterized > 0,
        "bounded fragmentation must remain parameterized by its typed provider"
    );
}

#[test]
fn adapter_rejects_backend_drift_and_unparameterized_fragment_limits() {
    let mut input = policy();
    input.transfer_syntax_uid = "1.2.840.10008.1.2.5".into();
    input.fragmentation_policy = "one_per_frame".into();
    input.offset_table_policy = "populated_basic".into();
    assert!(matches!(
        encoding_plan_from_recipe(&input, implementation()),
        Err(RecipeEncodingError::BackendMismatch { .. })
    ));

    let mut input = policy();
    input.fragmentation_policy = "bounded_fragments".into();
    assert!(matches!(
        encoding_plan_from_recipe(&input, implementation()),
        Err(RecipeEncodingError::MissingFragmentMaximum)
    ));
}

#[test]
fn adapter_rejects_missing_and_unknown_frontend_policies() {
    let mut input = policy();
    input.preamble_policy = None;
    assert!(matches!(
        encoding_plan_from_recipe(&input, implementation()),
        Err(RecipeEncodingError::MissingPolicy("preamble_policy"))
    ));

    let mut input = policy();
    input.file_meta_policy = None;
    assert!(matches!(
        encoding_plan_from_recipe(&input, implementation()),
        Err(RecipeEncodingError::MissingPolicy("file_meta_policy"))
    ));

    for field in [
        "sequence_length_policy",
        "item_length_policy",
        "offset_table_policy",
        "fragmentation_policy",
        "preamble_policy",
        "file_meta_policy",
    ] {
        let mut input = policy();
        match field {
            "sequence_length_policy" => input.sequence_length_policy = "mystery".into(),
            "item_length_policy" => input.item_length_policy = "mystery".into(),
            "offset_table_policy" => input.offset_table_policy = "mystery".into(),
            "fragmentation_policy" => input.fragmentation_policy = "mystery".into(),
            "preamble_policy" => input.preamble_policy = Some("mystery".into()),
            "file_meta_policy" => input.file_meta_policy = Some("mystery".into()),
            _ => unreachable!(),
        }
        assert!(matches!(
            encoding_plan_from_recipe(&input, implementation()),
            Err(RecipeEncodingError::UnknownPolicy { field: actual, .. }) if actual == field
        ));
    }
}
