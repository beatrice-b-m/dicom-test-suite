#[derive(Debug, Clone, Copy, PartialEq)]
pub(in crate::generator) struct ClassicPetRecipe {
    pub(in crate::generator) case_id: &'static str,
    pub(in crate::generator) recipe_id: &'static str,
    pub(in crate::generator) rows: u16,
    pub(in crate::generator) columns: u16,
    pub(in crate::generator) image_type: &'static str,
    pub(in crate::generator) pixel_values: &'static [u16; 4],
    pub(in crate::generator) pixel_bytes_le: &'static [u8; 8],
    pub(in crate::generator) frame_sha256: &'static str,
    pub(in crate::generator) rescale_intercept: &'static str,
    pub(in crate::generator) rescale_slope: &'static str,
    pub(in crate::generator) expected_activity_bqml: &'static [f64; 4],
    pub(in crate::generator) units: &'static str,
    pub(in crate::generator) counts_source: &'static str,
    pub(in crate::generator) series_type: &'static str,
    pub(in crate::generator) number_of_slices: u16,
    pub(in crate::generator) corrected_image: &'static str,
    pub(in crate::generator) decay_correction: &'static str,
    pub(in crate::generator) dose_calibration_factor: &'static str,
    pub(in crate::generator) frame_reference_time_ms: &'static str,
    pub(in crate::generator) actual_frame_duration_ms: &'static str,
    pub(in crate::generator) image_index: u16,
    pub(in crate::generator) pixel_spacing: &'static str,
    pub(in crate::generator) image_orientation_patient: &'static str,
    pub(in crate::generator) image_position_patient: &'static str,
    pub(in crate::generator) slice_thickness: &'static str,
}

impl ClassicPetRecipe {
    pub(in crate::generator) const fn pixel_count(self) -> usize {
        self.rows as usize * self.columns as usize
    }

    pub(in crate::generator) const fn pixel_bytes_are_consistent(self) -> bool {
        if self.pixel_count() != self.pixel_values.len()
            || self.pixel_bytes_le.len() != self.pixel_values.len() * 2
        {
            return false;
        }

        let mut index = 0;
        while index < self.pixel_values.len() {
            let bytes = self.pixel_values[index].to_le_bytes();
            if self.pixel_bytes_le[index * 2] != bytes[0]
                || self.pixel_bytes_le[index * 2 + 1] != bytes[1]
            {
                return false;
            }
            index += 1;
        }
        true
    }

    pub(in crate::generator) const fn activity_mapping_is_consistent(self) -> bool {
        if self.pixel_values.len() != self.expected_activity_bqml.len() {
            return false;
        }

        let mut index = 0;
        while index < self.pixel_values.len() {
            let expected = self.pixel_values[index] as f64 * RESCALE_SLOPE_VALUE;
            if self.expected_activity_bqml[index] != expected {
                return false;
            }
            index += 1;
        }
        true
    }
}

const RESCALE_SLOPE_VALUE: f64 = 2.5;
const PIXEL_VALUES: [u16; 4] = [0, 100, 200, 400];
const PIXEL_BYTES_LE: [u8; 8] = [0, 0, 100, 0, 200, 0, 144, 1];
const EXPECTED_ACTIVITY_BQML: [f64; 4] = [0.0, 250.0, 500.0, 1_000.0];

pub(in crate::generator) const CLASSIC_PET_RECIPE: ClassicPetRecipe = ClassicPetRecipe {
    case_id: "classic/pet/rescaled_activity_explicit_le",
    recipe_id: "classic_pet_rescaled_activity_explicit_le",
    rows: 2,
    columns: 2,
    image_type: "ORIGINAL\\PRIMARY",
    pixel_values: &PIXEL_VALUES,
    pixel_bytes_le: &PIXEL_BYTES_LE,
    frame_sha256: "03ec353fd2407afb09c8d65712ef9aa30f03c8243f6f3f1675dca7ea5f6a4784",
    rescale_intercept: "0",
    rescale_slope: "2.5",
    expected_activity_bqml: &EXPECTED_ACTIVITY_BQML,
    units: "BQML",
    counts_source: "EMISSION",
    series_type: "STATIC\\IMAGE",
    number_of_slices: 1,
    corrected_image: "DCAL",
    decay_correction: "NONE",
    dose_calibration_factor: "1",
    frame_reference_time_ms: "30000",
    actual_frame_duration_ms: "60000",
    image_index: 1,
    pixel_spacing: "4\\4",
    image_orientation_patient: "1\\0\\0\\0\\1\\0",
    image_position_patient: "0\\0\\0",
    slice_thickness: "4",
};

pub(in crate::generator) const CLASSIC_PET_RECIPES: &[ClassicPetRecipe] = &[CLASSIC_PET_RECIPE];

const _: () = assert!(CLASSIC_PET_RECIPE.pixel_count() == 4);
const _: () = assert!(CLASSIC_PET_RECIPE.pixel_bytes_are_consistent());
const _: () = assert!(CLASSIC_PET_RECIPE.activity_mapping_is_consistent());
const _: () = assert!(CLASSIC_PET_RECIPE.number_of_slices == 1);
const _: () = assert!(CLASSIC_PET_RECIPE.image_index == 1);
