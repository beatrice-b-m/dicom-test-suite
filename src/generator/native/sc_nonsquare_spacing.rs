use dicom_core::VR;

use super::super::{EXPLICIT_VR_LITTLE_ENDIAN, PixelRecipe};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::generator) enum NonsquareGeometryVariantId {
    PixelSpacing,
    PixelAspectRatio,
}

impl NonsquareGeometryVariantId {
    pub(in crate::generator) const fn as_str(self) -> &'static str {
        match self {
            Self::PixelSpacing => "pixel_spacing",
            Self::PixelAspectRatio => "pixel_aspect_ratio",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::generator) struct NonsquareGeometryVariant {
    pub(in crate::generator) variant_id: NonsquareGeometryVariantId,
    pub(in crate::generator) file_name: &'static str,
    pub(in crate::generator) pixel_spacing_mm: Option<[&'static str; 2]>,
    pub(in crate::generator) nominal_scanned_pixel_spacing_mm: Option<[&'static str; 2]>,
    pub(in crate::generator) pixel_aspect_ratio: Option<[u16; 2]>,
}

impl NonsquareGeometryVariant {
    pub(in crate::generator) const fn uses_physical_spacing(self) -> bool {
        self.pixel_spacing_mm.is_some()
            && self.nominal_scanned_pixel_spacing_mm.is_some()
            && self.pixel_aspect_ratio.is_none()
    }

    pub(in crate::generator) const fn uses_pixel_aspect_ratio(self) -> bool {
        self.pixel_spacing_mm.is_none()
            && self.nominal_scanned_pixel_spacing_mm.is_none()
            && self.pixel_aspect_ratio.is_some()
    }
}

#[derive(Debug, Clone, Copy)]
pub(in crate::generator) struct NonsquareSpacingScRecipe {
    pub(in crate::generator) pixel: PixelRecipe,
    pub(in crate::generator) variants: &'static [NonsquareGeometryVariant],
    pub(in crate::generator) pixel_data_sha256: &'static str,
}

const PIXELS: [u8; 24] = [
    0x00, 0xff, 0x00, 0xff, 0x00, 0xff, // row 1
    0xff, 0x00, 0xff, 0x00, 0xff, 0x00, // row 2
    0x00, 0xff, 0x00, 0xff, 0x00, 0xff, // row 3
    0xff, 0x00, 0xff, 0x00, 0xff, 0x00, // row 4
];
const PIXEL_VALUES: [i32; 24] = [
    0, 255, 0, 255, 0, 255, // row 1
    255, 0, 255, 0, 255, 0, // row 2
    0, 255, 0, 255, 0, 255, // row 3
    255, 0, 255, 0, 255, 0, // row 4
];

const VARIANTS: &[NonsquareGeometryVariant] = &[
    NonsquareGeometryVariant {
        variant_id: NonsquareGeometryVariantId::PixelSpacing,
        file_name: "pixel-spacing.dcm",
        pixel_spacing_mm: Some(["0.6", "0.3"]),
        nominal_scanned_pixel_spacing_mm: Some(["0.6", "0.3"]),
        pixel_aspect_ratio: None,
    },
    NonsquareGeometryVariant {
        variant_id: NonsquareGeometryVariantId::PixelAspectRatio,
        file_name: "pixel-aspect-ratio.dcm",
        pixel_spacing_mm: None,
        nominal_scanned_pixel_spacing_mm: None,
        pixel_aspect_ratio: Some([2, 1]),
    },
];

pub(in crate::generator) const NONSQUARE_SPACING_SC_RECIPE: NonsquareSpacingScRecipe =
    NonsquareSpacingScRecipe {
        pixel: PixelRecipe {
            case_id: "classic/sc/nonsquare_pixel_spacing",
            recipe_id: "classic_sc_nonsquare_pixel_spacing",
            rows: 4,
            columns: 6,
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
            visual_pattern: "4x6_monochrome_checkerboard_with_nonsquare_pixels",
            semantic_note: "Equivalent non-square geometry is declared independently by physical pixel spacing and integer pixel aspect ratio",
            palette: None,
            padding: None,
        },
        variants: VARIANTS,
        pixel_data_sha256: "e89b23efeade0dc3de624fc8982ea8b99adb35a3bb9a2fbf8b8ce675e10581a6",
    };
