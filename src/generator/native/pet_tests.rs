use super::pet::{CLASSIC_PET_RECIPE, CLASSIC_PET_RECIPES};
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
