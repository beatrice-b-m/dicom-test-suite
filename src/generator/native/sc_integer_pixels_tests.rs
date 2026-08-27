use super::sc_integer_pixels::{U1_SC_RECIPE, U32_SC_RECIPE};
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

#[test]
fn u1_sc_recipe_packs_frames_continuously_and_pads_only_the_value_field() {
    let recipe = U1_SC_RECIPE;

    assert_eq!(recipe.case_id, "classic/sc/mono2_u1_native");
    assert_eq!(recipe.recipe_id, "classic_sc_mono2_u1_native");
    assert_eq!((recipe.rows, recipe.columns, recipe.frames), (3, 3, 2));
    assert_eq!(recipe.packed_pixel_bytes, &[0x55, 0x55, 0x01, 0x00]);
    assert_eq!(recipe.significant_packed_bytes, 3);
    assert_eq!(
        sha256_hex(recipe.packed_pixel_bytes),
        recipe.pixel_data_sha256
    );

    let frames = recipe.decoded_frames();
    assert_eq!(frames.len(), 2);
    assert_eq!(frames[0], &[1, 0, 1, 0, 1, 0, 1, 0, 1]);
    assert_eq!(frames[1], &[0, 1, 0, 1, 0, 1, 0, 1, 0]);
    assert_eq!(sha256_hex(frames[0]), recipe.decoded_frame_sha256[0]);
    assert_eq!(sha256_hex(frames[1]), recipe.decoded_frame_sha256[1]);

    let decoded = (0..18)
        .map(|bit| (recipe.packed_pixel_bytes[bit / 8] >> (bit % 8)) & 1)
        .collect::<Vec<_>>();
    assert_eq!(decoded, recipe.decoded_pixel_bytes);
    assert_eq!(recipe.packed_pixel_bytes[2] & 0b1111_1100, 0);
    assert_eq!(recipe.packed_pixel_bytes[3], 0);
}
