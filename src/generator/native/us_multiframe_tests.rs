use super::us_multiframe::{CLASSIC_US_MULTIFRAME_RECIPE, CLASSIC_US_MULTIFRAME_RECIPES};
use crate::sha256_hex;

#[test]
fn classic_us_multiframe_recipe_locks_identity_timing_and_non_claims() {
    let recipe = CLASSIC_US_MULTIFRAME_RECIPE;

    assert_eq!(CLASSIC_US_MULTIFRAME_RECIPES, &[recipe]);
    assert_eq!(recipe.case_id, "classic/us/multiframe_explicit_le");
    assert_eq!(recipe.recipe_id, "classic_us_multiframe_explicit_le");
    assert_eq!((recipe.rows, recipe.columns), (4, 4));
    assert_eq!(recipe.image_type, "ORIGINAL\\PRIMARY\\ABDOMINAL\\0001");
    assert_eq!(recipe.frame_increment_pointer, "0018,1063");
    assert_eq!(recipe.frame_time_ms, 100);
    assert_eq!(recipe.frame_relative_times_ms, &[0, 100, 200, 300]);
    assert_eq!(recipe.frame_count(), 4);
    assert!(recipe.dimensions_and_order_are_consistent());
    assert!(recipe.relative_times_are_derived());
    assert_eq!(recipe.lossy_image_compression, "00");
    assert!(!recipe.color_data_present);
    assert!(!recipe.spatially_related_frames);
    assert!(!recipe.region_calibrated);
}

#[test]
fn classic_us_multiframe_recipe_has_exact_ordered_frames_and_hashes() {
    let recipe = CLASSIC_US_MULTIFRAME_RECIPE;
    let expected_values = [
        [
            0, 16, 32, 48, 16, 64, 80, 64, 32, 80, 255, 80, 48, 64, 80, 64,
        ],
        [
            0, 16, 32, 48, 16, 64, 80, 64, 32, 80, 80, 255, 48, 64, 80, 80,
        ],
        [
            0, 16, 32, 48, 16, 64, 80, 64, 32, 80, 80, 80, 48, 64, 255, 80,
        ],
        [
            0, 16, 32, 48, 16, 64, 80, 64, 32, 80, 80, 80, 48, 255, 80, 64,
        ],
    ];
    let expected_hashes = [
        "be422fa58b70ec0d940f28a4dba3dadac62d4583b9ecba1e73d65b37ee9733e7",
        "303d53edfa9bf6eeeb81dba8a6a4c1a9c2e1cb0ea773f90afb583d1132d88eee",
        "7f8a6e2fa2665b2465075b9e0cf86dfb0646f6f21a2a647525476e5bb6e489bb",
        "8c213da26d1c57661b68238ac5c1f1d9417f661e0ab578846bf84040e753f650",
    ];

    for (index, frame) in recipe.frames.iter().enumerate() {
        assert_eq!(frame.frame_number as usize, index + 1);
        assert_eq!(frame.pixel_values, &expected_values[index]);
        assert_eq!(frame.pixel_bytes, &expected_values[index]);
        assert_eq!(frame.frame_sha256, expected_hashes[index]);
        assert_eq!(sha256_hex(frame.pixel_bytes), expected_hashes[index]);
    }
}

#[test]
fn classic_us_multiframe_recipe_has_exact_concatenated_payload() {
    let recipe = CLASSIC_US_MULTIFRAME_RECIPE;
    let first = recipe
        .frames
        .iter()
        .flat_map(|frame| frame.pixel_bytes.iter().copied())
        .collect::<Vec<_>>();
    let second = recipe
        .frames
        .iter()
        .flat_map(|frame| frame.pixel_values.iter().copied())
        .collect::<Vec<_>>();

    assert_eq!(first, second);
    assert_eq!(first.len(), 64);
    assert_eq!(
        recipe.payload_sha256,
        "060e2c56c9728f787339515ef16bc8c1adfbfb4fb85b2d2c18f115c17b439bc9"
    );
    assert_eq!(sha256_hex(&first), recipe.payload_sha256);
    assert!(recipe.hash_lengths_are_consistent());
}
