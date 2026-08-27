use super::xrf::{CLASSIC_XRF_RECIPE, CLASSIC_XRF_RECIPES};
use crate::sha256_hex;

#[test]
fn classic_xrf_recipe_locks_identity_acquisition_and_geometry() {
    let recipe = CLASSIC_XRF_RECIPE;
    assert_eq!(CLASSIC_XRF_RECIPES, &[recipe]);
    assert_eq!(recipe.case_id, "classic/xrf/monoplane_explicit_le");
    assert_eq!(recipe.recipe_id, "classic_xrf_monoplane_explicit_le");
    assert_eq!(recipe.recipe_version, "0.1.0");
    assert_eq!(recipe.image_type, "ORIGINAL\\PRIMARY\\SINGLE PLANE");
    assert_eq!(recipe.body_part_examined, "ABDOMEN");
    assert_eq!(recipe.patient_orientation, "");
    assert_eq!(recipe.pixel_intensity_relationship, "LIN");
    assert_eq!(recipe.radiation_setting, "SC");
    assert_eq!((recipe.kvp, recipe.exposure_mas), (70, 1));
    assert_eq!(recipe.imager_pixel_spacing_mm, &[0.2, 0.2]);
    assert_eq!(recipe.distance_source_to_detector_mm, 1200);
    assert_eq!(recipe.distance_source_to_patient_mm, 800);
    assert_eq!(recipe.estimated_radiographic_magnification_factor, 1.5);
    assert_eq!(recipe.column_angulation_degrees, 10);
    assert!(recipe.geometry_is_consistent());
}

#[test]
fn classic_xrf_recipe_has_exact_native_frame_and_hashes() {
    let recipe = CLASSIC_XRF_RECIPE;
    assert_eq!((recipe.rows, recipe.columns), (4, 4));
    assert_eq!(recipe.pixel_count(), 16);
    assert_eq!(recipe.pixel_values, recipe.pixel_bytes);
    assert_eq!(
        recipe.pixel_values,
        &[
            0, 16, 32, 48, 16, 64, 96, 64, 32, 96, 255, 96, 48, 64, 96, 64
        ]
    );
    assert_eq!(sha256_hex(recipe.pixel_bytes), recipe.frame_sha256);
    assert_eq!(sha256_hex(recipe.pixel_bytes), recipe.payload_sha256);
    assert!(recipe.pixels_are_consistent());
}

#[test]
fn classic_xrf_recipe_locks_single_plane_non_claims() {
    let recipe = CLASSIC_XRF_RECIPE;
    assert_eq!(recipe.lossy_image_compression, "00");
    assert!(!recipe.laterality_present);
    assert!(!recipe.multiframe_cine);
    assert!(!recipe.biplane_data_present);
    assert!(!recipe.contrast_used);
    assert!(!recipe.subtraction_applied);
    assert!(!recipe.table_position_present);
    assert!(!recipe.table_motion_present);
    assert!(!recipe.table_tilt_present);
    assert!(!recipe.tomography_present);
    assert!(!recipe.patient_space_geometry_present);
    assert!(!recipe.pixel_spacing_calibrated);
    assert!(!recipe.xa_positioner_angles_present);
    assert!(recipe.non_claims_are_consistent());
}
