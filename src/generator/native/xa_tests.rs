use super::xa::CLASSIC_XA_RECIPE;
use crate::sha256_hex;

#[test]
fn classic_xa_recipe_locks_identity_acquisition_and_geometry() {
    let recipe = CLASSIC_XA_RECIPE;
    assert_eq!(recipe.case_id, "classic/xa/monoplane_explicit_le");
    assert_eq!(recipe.recipe_id, "classic_xa_monoplane_explicit_le");
    assert_eq!(recipe.image_type, "ORIGINAL\\PRIMARY\\SINGLE PLANE");
    assert_eq!(recipe.body_part_examined, "HEART");
    assert_eq!(recipe.pixel_intensity_relationship, "LIN");
    assert_eq!(recipe.radiation_setting, "GR");
    assert_eq!((recipe.kvp, recipe.exposure_mas), (80, 4));
    assert_eq!(recipe.imager_pixel_spacing_mm, &[0.2, 0.2]);
    assert_eq!(recipe.positioner_primary_angle_degrees, 15);
    assert_eq!(recipe.positioner_secondary_angle_degrees, -10);
    assert_eq!(recipe.distance_source_to_detector_mm, 1200);
    assert_eq!(recipe.distance_source_to_patient_mm, 800);
    assert_eq!(recipe.estimated_radiographic_magnification_factor, 1.5);
    assert!(recipe.geometry_is_consistent());
}

#[test]
fn classic_xa_recipe_has_exact_native_frame_and_hashes() {
    let recipe = CLASSIC_XA_RECIPE;
    assert_eq!((recipe.rows, recipe.columns), (4, 4));
    assert_eq!(recipe.pixel_count(), 16);
    assert_eq!(recipe.pixel_values, recipe.pixel_bytes);
    assert_eq!(sha256_hex(recipe.pixel_bytes), recipe.frame_sha256);
    assert_eq!(sha256_hex(recipe.pixel_bytes), recipe.payload_sha256);
    assert!(recipe.pixels_are_consistent());
}

#[test]
fn classic_xa_recipe_locks_single_plane_non_claims() {
    let recipe = CLASSIC_XA_RECIPE;
    assert_eq!(recipe.lossy_image_compression, "00");
    assert!(!recipe.multiframe_cine);
    assert!(!recipe.biplane_data_present);
    assert!(!recipe.contrast_used);
    assert!(!recipe.subtraction_applied);
    assert!(!recipe.table_motion_present);
    assert!(!recipe.patient_space_geometry_present);
    assert!(!recipe.pixel_spacing_calibrated);
    assert!(recipe.non_claims_are_consistent());
}
