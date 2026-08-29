use super::icc_profile::{
    ICC_COLOR_SPACE, ICC_PROFILE_BYTES, ICC_PROFILE_SHA256, ICC_PROFILE_SIZE,
    validate_locked_icc_profile,
};
use crate::sha256_hex;

#[test]
fn dcmtk_cc0_profile_matches_the_locked_dicom_input_contract() {
    assert_eq!(ICC_COLOR_SPACE, "SRGB");
    assert_eq!(ICC_PROFILE_BYTES.len(), ICC_PROFILE_SIZE);
    assert_eq!(sha256_hex(&ICC_PROFILE_BYTES), ICC_PROFILE_SHA256);
    validate_locked_icc_profile(&ICC_PROFILE_BYTES).unwrap();
    assert!(
        ICC_PROFILE_BYTES
            .windows(5)
            .any(|window| window == b"sRGB\0")
    );
    assert!(
        ICC_PROFILE_BYTES
            .windows(4)
            .any(|window| window == b"CC0\0")
    );
}

#[test]
fn icc_profile_contract_rejects_header_and_tag_table_tampering() {
    for (offset, replacement, expected_message) in [
        (12, b"mntr".as_slice(), "Input Device class"),
        (16, b"CMYK".as_slice(), "RGB input color space"),
        (20, b"Lab ".as_slice(), "XYZ profile connection space"),
        (36, b"zzzz".as_slice(), "ICC profile signature"),
    ] {
        let mut tampered = ICC_PROFILE_BYTES;
        tampered[offset..offset + 4].copy_from_slice(replacement);
        assert!(
            validate_locked_icc_profile(&tampered)
                .unwrap_err()
                .contains(expected_message)
        );
    }

    let mut bad_size = ICC_PROFILE_BYTES;
    bad_size[0..4].copy_from_slice(&735_u32.to_be_bytes());
    assert!(
        validate_locked_icc_profile(&bad_size)
            .unwrap_err()
            .contains("declared profile size")
    );

    let mut bad_offset = ICC_PROFILE_BYTES;
    bad_offset[136..140].copy_from_slice(&733_u32.to_be_bytes());
    assert!(
        validate_locked_icc_profile(&bad_offset)
            .unwrap_err()
            .contains("four-byte aligned")
    );
}
