use super::nm::{CLASSIC_NM_RECIPE, CLASSIC_NM_RECIPES};
use crate::sha256_hex;

#[test]
fn classic_nm_recipe_preserves_dimension_cardinality_and_order() {
    let recipe = CLASSIC_NM_RECIPE;

    assert_eq!(CLASSIC_NM_RECIPES, &[recipe]);
    assert_eq!(recipe.case_id, "classic/nm/multiframe_explicit_le");
    assert_eq!(recipe.recipe_id, "classic_nm_multiframe_explicit_le");
    assert_eq!((recipe.rows, recipe.columns), (2, 2));
    assert_eq!(recipe.image_type, "ORIGINAL\\PRIMARY\\STATIC\\EMISSION");
    assert_eq!(recipe.frame_count(), 4);
    assert_eq!(recipe.energy_window_vector, &[1, 1, 2, 2]);
    assert_eq!(recipe.detector_vector, &[1, 2, 1, 2]);
    assert!(recipe.dimensions_are_consistent());

    let frame_dimensions = recipe
        .frames
        .iter()
        .map(|frame| {
            (
                frame.frame_number,
                frame.energy_window_index,
                frame.detector_index,
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        frame_dimensions,
        vec![(1, 1, 1), (2, 1, 2), (3, 2, 1), (4, 2, 2)]
    );
}

#[test]
fn classic_nm_recipe_matches_locked_energy_windows_and_detectors() {
    let recipe = CLASSIC_NM_RECIPE;

    assert_eq!(recipe.energy_windows.len(), 2);
    assert_eq!(
        recipe
            .energy_windows
            .iter()
            .map(|window| (
                window.index,
                window.name,
                window.lower_limit_kev,
                window.upper_limit_kev,
            ))
            .collect::<Vec<_>>(),
        vec![
            (1, "Tc99m Photopeak", 126.0, 154.0),
            (2, "Tc99m Scatter", 100.0, 120.0),
        ]
    );

    assert_eq!(recipe.detectors.len(), 2);
    assert_eq!(recipe.detectors[0].index, 1);
    assert_eq!(recipe.detectors[0].collimator_type, "PARA");
    assert_eq!(recipe.detectors[0].focal_distance_mm, 0.0);
    assert_eq!(recipe.detectors[0].start_angle_degrees, 0.0);
    assert_eq!(
        recipe.detectors[0].image_orientation_patient,
        [1.0, 0.0, 0.0, 0.0, 1.0, 0.0]
    );
    assert_eq!(recipe.detectors[0].image_position_patient, [0.0, 0.0, 0.0]);
    assert_eq!(recipe.detectors[1].index, 2);
    assert_eq!(recipe.detectors[1].collimator_type, "PARA");
    assert_eq!(recipe.detectors[1].focal_distance_mm, 0.0);
    assert_eq!(recipe.detectors[1].start_angle_degrees, 180.0);
    assert_eq!(
        recipe.detectors[1].image_orientation_patient,
        [-1.0, 0.0, 0.0, 0.0, 1.0, 0.0]
    );
    assert_eq!(recipe.detectors[1].image_position_patient, [0.0, 0.0, 0.0]);
}

#[test]
fn classic_nm_recipe_has_exact_little_endian_frames_and_hashes() {
    let expected_values = [
        [0, 1, 2, 3],
        [10, 11, 12, 13],
        [100, 101, 102, 103],
        [110, 111, 112, 113],
    ];
    let expected_bytes = [
        [0, 0, 1, 0, 2, 0, 3, 0],
        [10, 0, 11, 0, 12, 0, 13, 0],
        [100, 0, 101, 0, 102, 0, 103, 0],
        [110, 0, 111, 0, 112, 0, 113, 0],
    ];
    let expected_hashes = [
        "245bbd9d484dcf27c714e2690cd6544973de5d54aa9cd82eab23d6046a65faa8",
        "a58214fbfec2da6f1e9fc6a2641c8a0af73fb383860180a73d4439fe31b44189",
        "4908c41ec85a7552278ed886fa3c43819f44d4df5b73138a9c5855926c750a58",
        "a12837f26e181e5420b019bae0940e221d2927e13fea963ad899945c34c697fe",
    ];

    for (index, frame) in CLASSIC_NM_RECIPE.frames.iter().enumerate() {
        assert_eq!(frame.pixel_values, &expected_values[index]);
        assert_eq!(frame.pixel_bytes_le, &expected_bytes[index]);
        assert_eq!(frame.frame_sha256, expected_hashes[index]);
        assert_eq!(sha256_hex(frame.pixel_bytes_le), expected_hashes[index]);

        let encoded_again = frame
            .pixel_values
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect::<Vec<_>>();
        assert_eq!(encoded_again, frame.pixel_bytes_le);
    }
}

#[test]
fn classic_nm_recipe_counts_are_exact_and_byte_stable() {
    let recipe = CLASSIC_NM_RECIPE;
    assert_eq!(recipe.actual_frame_duration_ms, 1_000);
    assert_eq!(recipe.counts_accumulated, 904);
    assert_eq!(recipe.computed_counts_accumulated(), 904);

    let first = recipe
        .frames
        .iter()
        .flat_map(|frame| frame.pixel_bytes_le.iter().copied())
        .collect::<Vec<_>>();
    let second = recipe
        .frames
        .iter()
        .flat_map(|frame| {
            frame
                .pixel_values
                .iter()
                .flat_map(|value| value.to_le_bytes())
        })
        .collect::<Vec<_>>();
    assert_eq!(first, second);
    assert_eq!(first.len(), 32);
    assert_eq!(sha256_hex(&first), sha256_hex(&second));
}
