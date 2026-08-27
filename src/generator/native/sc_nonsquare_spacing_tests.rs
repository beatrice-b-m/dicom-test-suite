use super::sc_nonsquare_spacing::{NONSQUARE_SPACING_SC_RECIPE, NonsquareGeometryVariantId};
use crate::sha256_hex;

#[test]
fn nonsquare_recipe_locks_the_common_checkerboard_payload() {
    let recipe = NONSQUARE_SPACING_SC_RECIPE;

    assert_eq!(recipe.pixel.case_id, "classic/sc/nonsquare_pixel_spacing");
    assert_eq!(recipe.pixel.recipe_id, "classic_sc_nonsquare_pixel_spacing");
    assert_eq!((recipe.pixel.rows, recipe.pixel.columns), (4, 6));
    assert_eq!(recipe.pixel.pixel_bytes.len(), 24);
    assert_eq!(recipe.pixel.pixel_values.len(), 24);
    assert_eq!(recipe.pixel.pixel_min, 0);
    assert_eq!(recipe.pixel.pixel_max, 255);
    assert_eq!(
        sha256_hex(recipe.pixel.pixel_bytes),
        "e89b23efeade0dc3de624fc8982ea8b99adb35a3bb9a2fbf8b8ce675e10581a6"
    );
    assert_eq!(
        sha256_hex(recipe.pixel.pixel_bytes),
        recipe.pixel_data_sha256
    );

    for row in 0..usize::from(recipe.pixel.rows) {
        for column in 0..usize::from(recipe.pixel.columns) {
            let offset = row * usize::from(recipe.pixel.columns) + column;
            let expected = if (row + column) % 2 == 0 { 0 } else { 255 };
            assert_eq!(recipe.pixel.pixel_bytes[offset], expected as u8);
            assert_eq!(recipe.pixel.pixel_values[offset], expected);
        }
    }
}

#[test]
fn physical_spacing_variant_uses_matching_row_column_ds_values_only() {
    let variant = NONSQUARE_SPACING_SC_RECIPE.variants[0];

    assert_eq!(variant.variant_id, NonsquareGeometryVariantId::PixelSpacing);
    assert_eq!(variant.variant_id.as_str(), "pixel_spacing");
    assert_eq!(variant.file_name, "pixel-spacing.dcm");
    assert_eq!(variant.pixel_spacing_mm, Some(["0.6", "0.3"]));
    assert_eq!(
        variant.nominal_scanned_pixel_spacing_mm,
        Some(["0.6", "0.3"])
    );
    assert_eq!(variant.pixel_aspect_ratio, None);
    assert!(variant.uses_physical_spacing());
    assert!(!variant.uses_pixel_aspect_ratio());

    let spacing = variant.pixel_spacing_mm.expect("spacing must be present");
    let row_mm: f32 = spacing[0].parse().expect("row spacing must be numeric");
    let column_mm: f32 = spacing[1].parse().expect("column spacing must be numeric");
    assert_eq!(row_mm / column_mm, 2.0);
}

#[test]
fn aspect_ratio_variant_uses_integer_vertical_horizontal_values_only() {
    let variant = NONSQUARE_SPACING_SC_RECIPE.variants[1];

    assert_eq!(
        variant.variant_id,
        NonsquareGeometryVariantId::PixelAspectRatio
    );
    assert_eq!(variant.variant_id.as_str(), "pixel_aspect_ratio");
    assert_eq!(variant.file_name, "pixel-aspect-ratio.dcm");
    assert_eq!(variant.pixel_spacing_mm, None);
    assert_eq!(variant.nominal_scanned_pixel_spacing_mm, None);
    assert_eq!(variant.pixel_aspect_ratio, Some([2, 1]));
    assert!(!variant.uses_physical_spacing());
    assert!(variant.uses_pixel_aspect_ratio());
}

#[test]
fn recipe_contains_exactly_the_two_mutually_exclusive_geometry_variants() {
    let variants = NONSQUARE_SPACING_SC_RECIPE.variants;

    assert_eq!(variants.len(), 2);
    assert_ne!(variants[0].variant_id, variants[1].variant_id);
    assert!(
        variants
            .iter()
            .all(|variant| { variant.uses_physical_spacing() ^ variant.uses_pixel_aspect_ratio() })
    );
}
