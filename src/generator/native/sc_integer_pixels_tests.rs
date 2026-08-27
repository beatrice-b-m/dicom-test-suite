use super::sc_integer_pixels::U32_SC_RECIPE;
use crate::sha256_hex;

#[test]
fn u32_sc_recipe_spans_unsigned_boundaries_with_exact_little_endian_bytes() {
    let recipe = U32_SC_RECIPE;

    assert_eq!(recipe.case_id, "classic/sc/mono2_u32_explicit_le");
    assert_eq!(recipe.recipe_id, "classic_sc_mono2_u32_explicit_le");
    assert_eq!((recipe.rows, recipe.columns), (2, 2));
    assert_eq!(
        recipe.pixel_values,
        &[0, 65_535, 2_147_483_648, 4_294_967_295]
    );
    assert!(recipe.pixel_bytes_are_consistent());
    assert_eq!(sha256_hex(recipe.pixel_bytes_le), recipe.pixel_data_sha256);
}
