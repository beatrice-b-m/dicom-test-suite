//! Bounded structural validation of caller-declared ICC v2 RGB input profiles.
//!
//! This establishes profile structure and declaration agreement. It does not
//! establish independent color-management accuracy or an sRGB transform claim.
use std::collections::BTreeMap;

use crate::recipes::ClassicIccProjection;

fn be32(bytes: &[u8], offset: usize) -> Result<u32, String> {
    bytes
        .get(offset..offset + 4)
        .and_then(|v| v.try_into().ok())
        .map(u32::from_be_bytes)
        .ok_or_else(|| "truncated ICC integer".into())
}

/// Validate an ICC profile and every machine-checkable declared profile field.
/// Source identity is caller provenance, not a property inferred from bytes.
pub(crate) fn validate_profile(
    bytes: &[u8],
    declared: &ClassicIccProjection,
    expected_sha256: &str,
    color_space: Option<&str>,
) -> Result<(), String> {
    if color_space.is_some() {
        return Err("caller ICC Color Space claims are not yet semantically qualified".into());
    }
    if !(132..=1024 * 1024).contains(&bytes.len())
        || bytes.len() % 4 != 0
        || be32(bytes, 0)? as usize != bytes.len()
        || crate::sha256_hex(bytes) != expected_sha256
    {
        return Err("ICC size or declared hash differs".into());
    }
    if &bytes[12..16] != b"scnr"
        || &bytes[16..20] != b"RGB "
        || &bytes[20..24] != b"XYZ "
        || &bytes[36..40] != b"acsp"
        || bytes[8] != 2
        || bytes[9] >> 4 > 9
        || bytes[9] & 15 > 9
        || bytes[10..12] != [0, 0]
        || bytes[84..128].iter().any(|b| *b != 0)
    {
        return Err("unsupported ICC v2 RGB input-profile header".into());
    }
    let date = bytes[24..36]
        .chunks_exact(2)
        .map(|v| u16::from_be_bytes([v[0], v[1]]))
        .collect::<Vec<_>>();
    let year = date[0];
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days = match date[1] {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if leap {
                29
            } else {
                28
            }
        }
        _ => 0,
    };
    if year == 0
        || date[2] == 0
        || date[2] > days
        || date[3] > 23
        || date[4] > 59
        || date[5] > 59
        || bytes[68..80] != [0, 0, 0xf6, 0xd6, 0, 1, 0, 0, 0, 0, 0xd3, 0x2d]
    {
        return Err("invalid ICC creation date or non-D50 PCS illuminant".into());
    }
    let intent = be32(bytes, 64)?;
    let intent_name = match intent {
        0 => "perceptual",
        1 => "media_relative_colorimetric",
        2 => "saturation",
        3 => "icc_absolute_colorimetric",
        _ => return Err("invalid ICC rendering intent".into()),
    };
    let version = format!("{}.{}.{}", bytes[8], bytes[9] >> 4, bytes[9] & 15);
    let count = be32(bytes, 128)? as usize;
    if count == 0 || count > 4096 {
        return Err("ICC tag count outside bounded profile capability".into());
    }
    let table_end = 132usize
        .checked_add(count.checked_mul(12).ok_or("ICC table overflow")?)
        .ok_or("ICC table overflow")?;
    if table_end > bytes.len() {
        return Err("ICC tag table exceeds profile".into());
    }
    let mut tags = BTreeMap::new();
    let mut extents = Vec::new();
    for index in 0..count {
        let entry = 132 + 12 * index;
        let signature: [u8; 4] = bytes[entry..entry + 4].try_into().unwrap();
        let offset = be32(bytes, entry + 4)? as usize;
        let length = be32(bytes, entry + 8)? as usize;
        let end = offset.checked_add(length).ok_or("ICC tag overflow")?;
        if extents.iter().any(|&(start, finish)| {
            offset < finish && start < end && (offset, end) != (start, finish)
        }) {
            return Err("ICC tag payloads partially overlap".into());
        }
        extents.push((offset, end));
        if !signature.iter().all(|b| b.is_ascii_graphic())
            || offset < table_end
            || offset % 4 != 0
            || length < 8
            || end > bytes.len()
            || bytes[offset + 4..offset + 8] != [0, 0, 0, 0]
            || tags.insert(signature, &bytes[offset..end]).is_some()
        {
            return Err("invalid ICC tag signature, extent, reserved field or duplicate".into());
        }
    }
    for signature in [*b"wtpt", *b"rXYZ", *b"gXYZ", *b"bXYZ"] {
        let value = tags
            .get(&signature)
            .ok_or("missing ICC matrix/white-point tag")?;
        if value.len() != 20 || &value[..4] != b"XYZ " {
            return Err("invalid ICC XYZ tag".into());
        }
    }
    for signature in [*b"rTRC", *b"gTRC", *b"bTRC"] {
        let value = tags
            .get(&signature)
            .ok_or("missing ICC tone-response curve")?;
        if &value[..4] != b"curv" || value.len() < 12 {
            return Err("unsupported ICC tone-response curve type".into());
        }
        let points = be32(value, 8)? as usize;
        let size = 12usize
            .checked_add(points.checked_mul(2).ok_or("ICC curve overflow")?)
            .ok_or("ICC curve overflow")?;
        if size != value.len() {
            return Err("ICC curve extent differs from point count".into());
        }
        if points == 1 && value[12..14] == [0, 0] {
            return Err("ICC single-value curve requires positive gamma".into());
        }
        if points > 1 {
            let samples = value[12..]
                .chunks_exact(2)
                .map(|v| u16::from_be_bytes([v[0], v[1]]))
                .collect::<Vec<_>>();
            if samples.windows(2).any(|w| w[0] > w[1]) {
                return Err("ICC tone-response curve is not monotonic".into());
            }
        }
    }
    let description = tags.get(b"desc").ok_or("missing ICC profile description")?;
    if description.len() < 12 || &description[..4] != b"desc" {
        return Err("unsupported ICC description type".into());
    }
    let length = be32(description, 8)? as usize;
    let text = description
        .get(
            12..12usize
                .checked_add(length)
                .ok_or("ICC description overflow")?,
        )
        .ok_or("truncated ICC description")?;
    let description_text = ascii_z(text)?;
    // ICC v2 textDescriptionType contains the ASCII field followed by the
    // Unicode language/count/text and a fixed Macintosh description field.
    let unicode_offset = 12usize
        .checked_add(length)
        .ok_or("ICC description overflow")?;
    let language = be32(description, unicode_offset)?;
    let unicode_count = be32(description, unicode_offset + 4)? as usize;
    let unicode_end = unicode_offset
        .checked_add(8)
        .and_then(|start| {
            unicode_count
                .checked_mul(2)
                .and_then(|length| start.checked_add(length))
        })
        .ok_or("ICC Unicode description overflow")?;
    if unicode_end.checked_add(70) != Some(description.len()) {
        return Err(
            "ICC description does not contain the complete Unicode/Macintosh fields".into(),
        );
    }
    // This capability deliberately supports only absent optional descriptions.
    // Their reserved storage must still be present and zero filled.
    if language != 0 || unicode_count != 0 || description[unicode_end..].iter().any(|b| *b != 0) {
        return Err("unsupported nonempty ICC Unicode/Macintosh description".into());
    }

    let copyright = tags.get(b"cprt").ok_or("missing ICC copyright")?;
    if &copyright[..4] != b"text" {
        return Err("unsupported ICC copyright type".into());
    }
    let copyright_text = ascii_z(&copyright[8..])?;
    if declared.tag != "(0028,2000)"
        || declared.vr != "OB"
        || declared.profile_signature != "acsp"
        || declared.device_class != "scnr"
        || declared.data_color_space != "RGB"
        || declared.profile_connection_space != "XYZ"
        || declared.profile_version != version
        || declared.rendering_intent_code != intent
        || declared.rendering_intent != intent_name
        || declared.tag_count as usize != count
        || declared.profile_description != description_text
        || declared.copyright != copyright_text
        || declared.source_identity.trim().is_empty()
    {
        return Err("ICC projected metadata differs from profile".into());
    }
    Ok(())
}

fn ascii_z(bytes: &[u8]) -> Result<&str, String> {
    let end = bytes
        .iter()
        .position(|b| *b == 0)
        .ok_or("ICC text is not terminated")?;
    if bytes[end..].iter().any(|b| *b != 0) || !bytes[..end].is_ascii() {
        return Err("invalid ICC ASCII text".into());
    }
    std::str::from_utf8(&bytes[..end]).map_err(|_| "invalid ICC text".into())
}
