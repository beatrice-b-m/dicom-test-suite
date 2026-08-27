//! Deterministic DICOM-constrained sRGB input profile recipe.
//!
//! The source bytes are DCMTK 3.7.0's `DCMTK_SRGB_ICC_SAMPLE`, derived from
//! saucecontrol's CC0 `sRGB-v2-magic.icc` 182-point-curve profile. Keeping the
//! reviewed hex in source avoids platform profile discovery and generated
//! binary assets. See `standards/source-notes/phase-2-icc-profile.md`.

pub(in crate::generator) const ICC_CASE_ID: &str = "vl/photo/rgb_icc_profile_explicit_le";
pub(in crate::generator) const ICC_RECIPE_ID: &str = "vl_photo_rgb_icc_profile_explicit_le";
pub(in crate::generator) const ICC_COLOR_SPACE: &str = "SRGB";
pub(in crate::generator) const ICC_PROFILE_SHA256: &str =
    "8e069a3476b71a0e0ae7272d9278ba70540d1c4a0b19af1c7d52e56f49091fef";
pub(in crate::generator) const ICC_PROFILE_SIZE: usize = 736;
pub(in crate::generator) const ICC_PROFILE_BYTES: [u8; ICC_PROFILE_SIZE] = decode_profile_hex();

const PROFILE_HEX: &[u8] = include_bytes!("dcmtk_srgb_input_profile.hex");
const REQUIRED_TAGS: [[u8; 4]; 9] = [
    *b"desc", *b"cprt", *b"wtpt", *b"rXYZ", *b"gXYZ", *b"bXYZ", *b"rTRC", *b"gTRC", *b"bTRC",
];

const fn decode_profile_hex() -> [u8; ICC_PROFILE_SIZE] {
    let mut output = [0_u8; ICC_PROFILE_SIZE];
    let mut input_index = 0;
    let mut output_index = 0;
    let mut high_nibble = 0;
    let mut have_high_nibble = false;
    while input_index < PROFILE_HEX.len() {
        let byte = PROFILE_HEX[input_index];
        input_index += 1;
        if matches!(byte, b' ' | b'\t' | b'\r' | b'\n') {
            continue;
        }
        let nibble = match byte {
            b'0'..=b'9' => byte - b'0',
            b'a'..=b'f' => byte - b'a' + 10,
            b'A'..=b'F' => byte - b'A' + 10,
            _ => panic!("ICC profile source contains a non-hex byte"),
        };
        if have_high_nibble {
            if output_index >= ICC_PROFILE_SIZE {
                panic!("ICC profile source exceeds its locked size");
            }
            output[output_index] = (high_nibble << 4) | nibble;
            output_index += 1;
            have_high_nibble = false;
        } else {
            high_nibble = nibble;
            have_high_nibble = true;
        }
    }
    if have_high_nibble || output_index != ICC_PROFILE_SIZE {
        panic!("ICC profile source does not match its locked size");
    }
    output
}

pub(in crate::generator) fn validate_locked_icc_profile(bytes: &[u8]) -> Result<(), String> {
    if bytes.len() != ICC_PROFILE_SIZE {
        return Err(format!(
            "ICC profile length {} does not match locked length {ICC_PROFILE_SIZE}",
            bytes.len()
        ));
    }
    let declared_size = read_be_u32(bytes, 0)? as usize;
    if declared_size != bytes.len() {
        return Err(format!(
            "ICC declared profile size {declared_size} does not match Value Field length {}",
            bytes.len()
        ));
    }
    for (offset, expected, label) in [
        (8, &b"\x02\x10\x00\x00"[..], "version 2.1.0"),
        (12, &b"scnr"[..], "Input Device class"),
        (16, &b"RGB "[..], "RGB input color space"),
        (20, &b"XYZ "[..], "XYZ profile connection space"),
        (36, &b"acsp"[..], "ICC profile signature"),
    ] {
        if bytes.get(offset..offset + expected.len()) != Some(expected) {
            return Err(format!("ICC profile does not declare locked {label}"));
        }
    }
    if read_be_u32(bytes, 64)? != 0 {
        return Err("ICC profile rendering intent is not perceptual (0)".to_string());
    }
    let tag_count = read_be_u32(bytes, 128)? as usize;
    if tag_count != REQUIRED_TAGS.len() {
        return Err(format!(
            "ICC tag count {tag_count} does not match locked count {}",
            REQUIRED_TAGS.len()
        ));
    }
    let tag_table_end = 132_usize
        .checked_add(
            tag_count
                .checked_mul(12)
                .ok_or("ICC tag table size overflow")?,
        )
        .ok_or("ICC tag table end overflow")?;
    if tag_table_end > bytes.len() {
        return Err("ICC tag table extends beyond the profile".to_string());
    }
    for (index, required_signature) in REQUIRED_TAGS.iter().enumerate() {
        let record_offset = 132 + index * 12;
        if bytes.get(record_offset..record_offset + 4) != Some(required_signature) {
            return Err(format!("ICC tag {index} has an unexpected signature"));
        }
        let payload_offset = read_be_u32(bytes, record_offset + 4)? as usize;
        let payload_size = read_be_u32(bytes, record_offset + 8)? as usize;
        if payload_offset % 4 != 0 {
            return Err(format!("ICC tag {index} payload is not four-byte aligned"));
        }
        let payload_end = payload_offset
            .checked_add(payload_size)
            .ok_or("ICC tag payload end overflow")?;
        if payload_offset < tag_table_end || payload_end > bytes.len() {
            return Err(format!("ICC tag {index} payload is outside profile bounds"));
        }
    }
    Ok(())
}

fn read_be_u32(bytes: &[u8], offset: usize) -> Result<u32, String> {
    let value = bytes
        .get(offset..offset + 4)
        .ok_or_else(|| format!("ICC profile is truncated at byte {offset}"))?;
    Ok(u32::from_be_bytes(
        value.try_into().expect("four-byte slice"),
    ))
}
