#[derive(Debug, Clone, Copy, PartialEq)]
pub(in crate::generator) struct ClassicXaRecipe {
    pub(in crate::generator) case_id: &'static str,
    pub(in crate::generator) recipe_id: &'static str,
    pub(in crate::generator) rows: u16,
    pub(in crate::generator) columns: u16,
    pub(in crate::generator) image_type: &'static str,
    pub(in crate::generator) body_part_examined: &'static str,
    pub(in crate::generator) pixel_intensity_relationship: &'static str,
    pub(in crate::generator) radiation_setting: &'static str,
    pub(in crate::generator) kvp: u16,
    pub(in crate::generator) exposure_mas: u16,
    pub(in crate::generator) imager_pixel_spacing_mm: &'static [f64; 2],
    pub(in crate::generator) positioner_primary_angle_degrees: i16,
    pub(in crate::generator) positioner_secondary_angle_degrees: i16,
    pub(in crate::generator) distance_source_to_detector_mm: u16,
    pub(in crate::generator) distance_source_to_patient_mm: u16,
    pub(in crate::generator) estimated_radiographic_magnification_factor: f64,
    pub(in crate::generator) lossy_image_compression: &'static str,
    pub(in crate::generator) pixel_values: &'static [u8; 16],
    pub(in crate::generator) pixel_bytes: &'static [u8; 16],
    pub(in crate::generator) frame_sha256: &'static str,
    pub(in crate::generator) payload_sha256: &'static str,
    pub(in crate::generator) multiframe_cine: bool,
    pub(in crate::generator) biplane_data_present: bool,
    pub(in crate::generator) contrast_used: bool,
    pub(in crate::generator) subtraction_applied: bool,
    pub(in crate::generator) table_motion_present: bool,
    pub(in crate::generator) patient_space_geometry_present: bool,
    pub(in crate::generator) pixel_spacing_calibrated: bool,
}

impl ClassicXaRecipe {
    pub(in crate::generator) const fn pixel_count(self) -> usize {
        self.rows as usize * self.columns as usize
    }

    pub(in crate::generator) const fn pixels_are_consistent(self) -> bool {
        if self.pixel_values.len() != self.pixel_count()
            || self.pixel_bytes.len() != self.pixel_count()
            || self.frame_sha256.len() != 64
            || self.payload_sha256.len() != 64
            || self.frame_sha256.as_bytes().len() != self.payload_sha256.as_bytes().len()
        {
            return false;
        }
        let mut index = 0;
        while index < self.pixel_values.len() {
            if self.pixel_values[index] != self.pixel_bytes[index] {
                return false;
            }
            index += 1;
        }
        true
    }

    pub(in crate::generator) const fn geometry_is_consistent(self) -> bool {
        self.distance_source_to_detector_mm == 1200
            && self.distance_source_to_patient_mm == 800
            && self.estimated_radiographic_magnification_factor == 1.5
            && self.imager_pixel_spacing_mm[0] == 0.2
            && self.imager_pixel_spacing_mm[1] == 0.2
    }

    pub(in crate::generator) const fn non_claims_are_consistent(self) -> bool {
        !self.multiframe_cine
            && !self.biplane_data_present
            && !self.contrast_used
            && !self.subtraction_applied
            && !self.table_motion_present
            && !self.patient_space_geometry_present
            && !self.pixel_spacing_calibrated
    }
}

const XA_PIXELS: [u8; 16] = [
    0, 16, 32, 48, 16, 64, 96, 64, 32, 96, 255, 96, 48, 64, 96, 64,
];
const XA_IMAGER_PIXEL_SPACING_MM: [f64; 2] = [0.2, 0.2];

pub(in crate::generator) const CLASSIC_XA_RECIPE: ClassicXaRecipe = ClassicXaRecipe {
    case_id: "classic/xa/monoplane_explicit_le",
    recipe_id: "classic_xa_monoplane_explicit_le",
    rows: 4,
    columns: 4,
    image_type: "ORIGINAL\\PRIMARY\\SINGLE PLANE",
    body_part_examined: "HEART",
    pixel_intensity_relationship: "LIN",
    radiation_setting: "GR",
    kvp: 80,
    exposure_mas: 4,
    imager_pixel_spacing_mm: &XA_IMAGER_PIXEL_SPACING_MM,
    positioner_primary_angle_degrees: 15,
    positioner_secondary_angle_degrees: -10,
    distance_source_to_detector_mm: 1200,
    distance_source_to_patient_mm: 800,
    estimated_radiographic_magnification_factor: 1.5,
    lossy_image_compression: "00",
    pixel_values: &XA_PIXELS,
    pixel_bytes: &XA_PIXELS,
    frame_sha256: "0b9c742cc3fafec4c1d0240048d27210f2da155b3574458ae26035ffa488c00e",
    payload_sha256: "0b9c742cc3fafec4c1d0240048d27210f2da155b3574458ae26035ffa488c00e",
    multiframe_cine: false,
    biplane_data_present: false,
    contrast_used: false,
    subtraction_applied: false,
    table_motion_present: false,
    patient_space_geometry_present: false,
    pixel_spacing_calibrated: false,
};

pub(in crate::generator) const CLASSIC_XA_RECIPES: &[ClassicXaRecipe] = &[CLASSIC_XA_RECIPE];

const _: () = assert!(CLASSIC_XA_RECIPE.pixel_count() == 16);
const _: () = assert!(CLASSIC_XA_RECIPE.pixels_are_consistent());
const _: () = assert!(CLASSIC_XA_RECIPE.geometry_is_consistent());
const _: () = assert!(CLASSIC_XA_RECIPE.non_claims_are_consistent());
