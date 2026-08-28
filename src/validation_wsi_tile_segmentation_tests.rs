use super::reconstruct_wsi_tile_segmentation_matrix;
use crate::sha256_hex;

const PAYLOAD: [u8; 8] = [255, 0, 0, 255, 0, 255, 255, 0];
const MATRIX_HASH: &str = "a8ec6f910c0fb02685163a3251bed92517d1016c9173f1e4f021e6b4194f2467";

#[test]
fn reconstructs_locked_zero_filled_total_pixel_matrix() {
    let matrix = reconstruct_wsi_tile_segmentation_matrix(&PAYLOAD, &[(1, 1), (3, 3)])
        .expect("locked diagonal tile positions reconstruct");

    assert_eq!(
        matrix,
        [255, 0, 0, 0, 0, 255, 0, 0, 0, 0, 0, 255, 0, 0, 255, 0]
    );
    assert_eq!(sha256_hex(&matrix), MATRIX_HASH);
}

#[test]
fn one_byte_payload_mutation_changes_the_reconstruction_hash() {
    let mut mutated = PAYLOAD;
    mutated[1] = 1;

    let matrix = reconstruct_wsi_tile_segmentation_matrix(&mutated, &[(1, 1), (3, 3)])
        .expect("shape remains reconstructable");
    assert_ne!(sha256_hex(&matrix), MATRIX_HASH);
}

#[test]
fn rejects_truncated_duplicate_and_off_grid_tiles() {
    assert!(reconstruct_wsi_tile_segmentation_matrix(&PAYLOAD[..7], &[(1, 1), (3, 3)]).is_none());
    assert!(reconstruct_wsi_tile_segmentation_matrix(&PAYLOAD, &[(1, 1), (1, 1)]).is_none());
    assert!(reconstruct_wsi_tile_segmentation_matrix(&PAYLOAD, &[(1, 1), (2, 3)]).is_none());
}
