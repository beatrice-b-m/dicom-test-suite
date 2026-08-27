use super::pet::{
    CLASSIC_PET_RECIPE, CLASSIC_PET_RECIPES, ENHANCED_PET_RECIPE, ENHANCED_PET_RECIPES,
};
use crate::sha256_hex;

#[test]
fn classic_pet_recipe_has_locked_identity_and_geometry() {
    let recipe = CLASSIC_PET_RECIPE;

    assert_eq!(CLASSIC_PET_RECIPES, &[recipe]);
    assert_eq!(recipe.case_id, "classic/pet/rescaled_activity_explicit_le");
    assert_eq!(
        recipe.recipe_id,
        "classic_pet_rescaled_activity_explicit_le"
    );
    assert_eq!((recipe.rows, recipe.columns), (2, 2));
    assert_eq!(recipe.pixel_count(), 4);
    assert_eq!(recipe.image_type, "ORIGINAL\\PRIMARY");
    assert_eq!(recipe.pixel_spacing, "4\\4");
    assert_eq!(recipe.image_orientation_patient, "1\\0\\0\\0\\1\\0");
    assert_eq!(recipe.image_position_patient, "0\\0\\0");
    assert_eq!(recipe.slice_thickness, "4");
}

#[test]
fn classic_pet_recipe_has_exact_little_endian_pixels_and_hash() {
    let recipe = CLASSIC_PET_RECIPE;

    assert_eq!(recipe.pixel_values, &[0, 100, 200, 400]);
    assert_eq!(recipe.pixel_bytes_le, &[0, 0, 100, 0, 200, 0, 144, 1]);
    assert!(recipe.pixel_bytes_are_consistent());
    assert_eq!(
        recipe.frame_sha256,
        "03ec353fd2407afb09c8d65712ef9aa30f03c8243f6f3f1675dca7ea5f6a4784"
    );
    assert_eq!(sha256_hex(recipe.pixel_bytes_le), recipe.frame_sha256);

    let encoded_again = recipe
        .pixel_values
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect::<Vec<_>>();
    assert_eq!(encoded_again, recipe.pixel_bytes_le);
}

#[test]
fn classic_pet_recipe_maps_stored_values_to_bqml_exactly() {
    let recipe = CLASSIC_PET_RECIPE;

    assert_eq!(recipe.rescale_intercept, "0");
    assert_eq!(recipe.rescale_slope, "2.5");
    assert_eq!(recipe.units, "BQML");
    assert_eq!(recipe.expected_activity_bqml, &[0.0, 250.0, 500.0, 1_000.0]);
    assert!(recipe.activity_mapping_is_consistent());

    let mapped = recipe
        .pixel_values
        .iter()
        .map(|value| f64::from(*value) * 2.5)
        .collect::<Vec<_>>();
    assert_eq!(mapped, recipe.expected_activity_bqml);
}

#[test]
fn classic_pet_recipe_has_locked_series_and_timing_metadata() {
    let recipe = CLASSIC_PET_RECIPE;

    assert_eq!(recipe.counts_source, "EMISSION");
    assert_eq!(recipe.series_type, "STATIC\\IMAGE");
    assert_eq!(recipe.number_of_slices, 1);
    assert_eq!(recipe.corrected_image, "DCAL");
    assert_eq!(recipe.decay_correction, "NONE");
    assert_eq!(recipe.dose_calibration_factor, "1");
    assert_eq!(recipe.frame_reference_time_ms, "30000");
    assert_eq!(recipe.actual_frame_duration_ms, "60000");
    assert_eq!(recipe.image_index, 1);
}

#[test]
fn enhanced_pet_recipe_has_locked_identity_and_static_stack_geometry() {
    let recipe = ENHANCED_PET_RECIPE;

    assert_eq!(ENHANCED_PET_RECIPES, &[recipe]);
    assert_eq!(recipe.case_id, "enhanced/pet/multiframe_explicit_le");
    assert_eq!(recipe.recipe_id, "enhanced_pet_multiframe_explicit_le");
    assert_eq!((recipe.rows, recipe.columns, recipe.frames), (2, 2, 2));
    assert_eq!(recipe.frame_pixel_count(), 4);
    assert_eq!(recipe.pixel_count(), 8);
    assert_eq!(
        recipe.image_type,
        "DERIVED\\PRIMARY\\STATIC\\MULTIPLICATION"
    );
    assert_eq!(recipe.frame_type, recipe.image_type);
    assert_eq!(recipe.pixel_spacing, "2\\2");
    assert_eq!(recipe.image_orientation_patient, "1\\0\\0\\0\\1\\0");
    assert_eq!(recipe.image_position_patient, &["0\\0\\0", "0\\0\\5"]);
    assert_eq!(recipe.slice_thickness, "5");
    assert_eq!(recipe.spacing_between_slices, "5");
    assert_eq!(recipe.stack_id, "1");
    assert_eq!(recipe.in_stack_position_numbers, &[1, 2]);
    assert_eq!(recipe.dimension_index_values, &[1, 2]);
    assert_eq!(recipe.temporal_position_indices, &[1, 1]);
    assert!(recipe.frame_metadata_is_consistent());
}

#[test]
fn enhanced_pet_recipe_has_two_identical_exact_native_frames() {
    let recipe = ENHANCED_PET_RECIPE;

    assert_eq!(recipe.pixel_values, &[0, 100, 200, 400, 0, 100, 200, 400]);
    assert_eq!(
        recipe.pixel_bytes_le,
        &[0, 0, 100, 0, 200, 0, 144, 1, 0, 0, 100, 0, 200, 0, 144, 1]
    );
    assert!(recipe.pixel_bytes_are_consistent());
    assert!(recipe.frames_are_identical());

    let frame_bytes = recipe.frame_pixel_count() * 2;
    let actual_frame_hashes = recipe
        .pixel_bytes_le
        .chunks_exact(frame_bytes)
        .map(sha256_hex)
        .collect::<Vec<_>>();
    assert_eq!(actual_frame_hashes, recipe.frame_sha256);
    assert_eq!(
        recipe.frame_sha256,
        &[
            "03ec353fd2407afb09c8d65712ef9aa30f03c8243f6f3f1675dca7ea5f6a4784",
            "03ec353fd2407afb09c8d65712ef9aa30f03c8243f6f3f1675dca7ea5f6a4784"
        ]
    );
    assert_eq!(sha256_hex(recipe.pixel_bytes_le), recipe.pixel_data_sha256);
    assert_eq!(
        recipe.pixel_data_sha256,
        "3a43b45e2f6d4d04fe4fc357dfc0efaa21caa5415ffc5db96fc19428d34a7bb5"
    );
}

#[test]
fn enhanced_pet_recipe_maps_each_frame_to_bqml_exactly() {
    let recipe = ENHANCED_PET_RECIPE;

    assert_eq!(recipe.rescale_intercept, "0");
    assert_eq!(recipe.rescale_slope, "2.5");
    assert_eq!(recipe.units, "BQML");
    assert_eq!(recipe.counts_source, "EMISSION");
    assert_eq!(
        recipe.expected_activity_bqml,
        &[0.0, 250.0, 500.0, 1_000.0, 0.0, 250.0, 500.0, 1_000.0]
    );
    assert!(recipe.activity_mapping_is_consistent());

    let mapped = recipe
        .pixel_values
        .iter()
        .map(|value| f64::from(*value) * 2.5)
        .collect::<Vec<_>>();
    assert_eq!(mapped, recipe.expected_activity_bqml);
}
