use dicom_test_suite::recipes::{
    EOT_ARITHMETIC_PLAN_PROVIDER_ID, FUZZ_PLAN_PROVIDER_ID, QualificationParameters, RecipeCatalog,
    qualification_parameters,
};

#[test]
fn catalog_qualification_contracts_are_public_typed_and_ordered() {
    let catalog = RecipeCatalog::load(
        "cases/recipes",
        "cases/registry.json",
        "templates/catalog.json",
    )
    .unwrap();

    let fuzz = &catalog.recipes()[catalog
        .binding_for_case("fuzz/parser/bounded_seed_corpus")
        .unwrap()];
    assert_eq!(fuzz.plan_provider_id, FUZZ_PLAN_PROVIDER_ID);
    let QualificationParameters::BoundedDeterministicFuzz {
        source_generation_seed,
        candidates_per_source,
        sources,
        budget,
    } = qualification_parameters(fuzz).unwrap()
    else {
        panic!("fuzz recipe did not produce the bounded fuzz contract")
    };
    assert_eq!(source_generation_seed, 7);
    assert_eq!(candidates_per_source, 32);
    assert_eq!(
        sources
            .iter()
            .map(|source| (
                source.seed_description_id.as_str(),
                source.dependency_role.as_str(),
                source.recipe.recipe_id.as_str(),
                source.artifact_logical_id.as_str(),
            ))
            .collect::<Vec<_>>(),
        vec![
            (
                "part10-explicit-vr-le-v1",
                "part10_explicit_vr_le",
                "sc_mono2_u8",
                "instance",
            ),
            (
                "encapsulated-rle-v1",
                "encapsulated_rle",
                "sc_mono1_u8_rle_lossless",
                "instance",
            ),
        ]
    );
    assert_eq!(budget.max_iterations, 64);
    assert_eq!(budget.max_candidates, 64);
    assert_eq!(budget.max_total_mutations, 512);
    assert_eq!(budget.max_total_target_operations, 100_000_000);

    let eot = &catalog.recipes()[catalog
        .binding_for_case("qualification/encapsulation/eot_u64_overflow")
        .unwrap()];
    assert_eq!(eot.plan_provider_id, EOT_ARITHMETIC_PLAN_PROVIDER_ID);
    let QualificationParameters::CheckedEotU64Overflow {
        fragment_lengths,
        arithmetic_steps,
        expected_error,
    } = qualification_parameters(eot).unwrap()
    else {
        panic!("EOT recipe did not produce the checked arithmetic contract")
    };
    assert_eq!(fragment_lengths, [u64::MAX]);
    assert_eq!(
        arithmetic_steps,
        [
            "pad_fragment_to_even",
            "add_item_header",
            "accumulate_frame_offset",
        ]
    );
    assert_eq!(expected_error, "fragment_padding_overflow");
}
