use dicom_core::VR;

use super::super::{EXPLICIT_VR_LITTLE_ENDIAN, PixelRecipe};

#[derive(Debug, Clone, Copy)]
pub(in crate::generator) struct TimezoneBoundary {
    pub(in crate::generator) boundary_id: &'static str,
    pub(in crate::generator) study_date: &'static str,
    pub(in crate::generator) study_time: &'static str,
    pub(in crate::generator) acquisition_date_time: &'static str,
    pub(in crate::generator) timezone_offset: &'static str,
    pub(in crate::generator) offset_minutes: i16,
    pub(in crate::generator) normalized_utc: &'static str,
}

#[derive(Debug, Clone, Copy)]
pub(in crate::generator) struct TimezoneScRecipe {
    pub(in crate::generator) pixel: PixelRecipe,
    pub(in crate::generator) boundaries: &'static [TimezoneBoundary],
}

const PIXELS: [u8; 4] = [0, 85, 170, 255];
const PIXEL_VALUES: [i32; 4] = [0, 85, 170, 255];

const BOUNDARIES: &[TimezoneBoundary] = &[
    TimezoneBoundary {
        boundary_id: "positive_max",
        study_date: "20240229",
        study_time: "235959.999999",
        acquisition_date_time: "20240229235959.999999+1400",
        timezone_offset: "+1400",
        offset_minutes: 840,
        normalized_utc: "2024-02-29T09:59:59.999999Z",
    },
    TimezoneBoundary {
        boundary_id: "negative_min",
        study_date: "20240301",
        study_time: "000000.000000",
        acquisition_date_time: "20240301000000.000000-1200",
        timezone_offset: "-1200",
        offset_minutes: -720,
        normalized_utc: "2024-03-01T12:00:00.000000Z",
    },
];

pub(in crate::generator) const TIMEZONE_SC_RECIPE: TimezoneScRecipe = TimezoneScRecipe {
    pixel: PixelRecipe {
        case_id: "metadata/sc/timezone_boundaries",
        recipe_id: "metadata_sc_timezone_boundaries",
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
        visual_pattern: "two_2x2_monochrome_gradients_with_timezone_extrema",
        semantic_note: "Separate instances bind DA, TM, and DT boundary values to the legal +1400 and -1200 timezone offsets",
        palette: None,
        padding: None,
    },
    boundaries: BOUNDARIES,
};

#[cfg(test)]
mod tests {
    use super::TIMEZONE_SC_RECIPE;

    #[test]
    fn recipe_locks_both_asymmetric_timezone_extrema() {
        let boundaries = TIMEZONE_SC_RECIPE.boundaries;
        assert_eq!(boundaries.len(), 2);
        assert_eq!(boundaries[0].offset_minutes, 840);
        assert_eq!(boundaries[0].acquisition_date_time.len(), 26);
        assert_eq!(boundaries[1].offset_minutes, -720);
        assert_eq!(boundaries[1].acquisition_date_time.len(), 26);
    }
}
