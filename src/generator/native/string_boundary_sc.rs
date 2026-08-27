use dicom_core::VR;

use super::super::{EXPLICIT_VR_LITTLE_ENDIAN, PixelRecipe};

#[derive(Debug, Clone, Copy)]
pub(in crate::generator) struct StringBoundaryScRecipe {
    pub(in crate::generator) pixel: PixelRecipe,
    pub(in crate::generator) image_comments_pattern: &'static str,
    pub(in crate::generator) image_comments_repetitions: usize,
    pub(in crate::generator) software_versions: [&'static str; 2],
    pub(in crate::generator) pixel_spacing: [&'static str; 2],
    pub(in crate::generator) acquisition_number: &'static str,
}

const PIXELS: [u8; 4] = [0, 85, 170, 255];
const PIXEL_VALUES: [i32; 4] = [0, 85, 170, 255];

pub(in crate::generator) const STRING_BOUNDARY_SC_RECIPE: StringBoundaryScRecipe =
    StringBoundaryScRecipe {
        pixel: PixelRecipe {
            case_id: "metadata/sc/long_multivalue_text_numeric_strings",
            recipe_id: "metadata_sc_long_multivalue_text_numeric_strings",
            rows: 2,
            columns: 2,
            photometric_interpretation: "MONOCHROME2",
            samples_per_pixel: 1,
            planar_configuration: None,
            bits_allocated: 8,
            bits_stored: 8,
            high_bit: 7,
            pixel_representation: 0,
            pixel_vr: VR::OB,
            transfer_syntax: EXPLICIT_VR_LITTLE_ENDIAN,
            pixel_bytes: &PIXELS,
            pixel_values: &PIXEL_VALUES,
            pixel_min: 0,
            pixel_max: 255,
            visual_pattern: "2x2_monochrome_gradient_with_string_vr_boundaries",
            semantic_note: "LT, LO, DS, and IS exercise maximum component lengths, multi-value delimiters, and even-length padding",
            palette: None,
            padding: None,
        },
        image_comments_pattern: "0123456789ABCDEF",
        image_comments_repetitions: 640,
        software_versions: [
            "DTS-A-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
            "DTS-B-BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB",
        ],
        pixel_spacing: ["0.12345678901234", "0.98765432109876"],
        acquisition_number: "+02147483647",
    };

#[cfg(test)]
mod tests {
    use super::STRING_BOUNDARY_SC_RECIPE;
    use crate::sha256_hex;

    #[test]
    fn recipe_locks_vr_boundary_lengths_and_raw_hashes() {
        let recipe = STRING_BOUNDARY_SC_RECIPE;
        let comments = recipe
            .image_comments_pattern
            .repeat(recipe.image_comments_repetitions);
        assert_eq!(comments.len(), 10_240);
        assert_eq!(
            sha256_hex(comments.as_bytes()),
            "75497849c172d88a38e271cc6ce82f31adbba1f16b6191d8ddaeb4e9f6268e52"
        );

        let mut versions = recipe.software_versions.join("\\").into_bytes();
        versions.push(b' ');
        assert_eq!(recipe.software_versions.map(str::len), [64, 64]);
        assert_eq!(versions.len(), 130);
        assert_eq!(
            sha256_hex(&versions),
            "e79f64c5853732dd713d14c3530ef494d800f684653fc5bf0aced3933241a260"
        );

        let mut spacing = recipe.pixel_spacing.join("\\").into_bytes();
        spacing.push(b' ');
        assert_eq!(recipe.pixel_spacing.map(str::len), [16, 16]);
        assert_eq!(spacing.len(), 34);
        assert_eq!(
            sha256_hex(&spacing),
            "e09885a80758e44eaa4b9b544e7301c852395d3ee14ed7b7588e62a5f3b2db6a"
        );
        assert_eq!(recipe.acquisition_number.len(), 12);
        assert_eq!(
            sha256_hex(recipe.acquisition_number.as_bytes()),
            "f9cf9c74b83f0c66cdb48d3536a5a5d884babc2cfda813d01b3577b473de20cf"
        );
    }
}
