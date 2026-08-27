use dicom_core::VR;

use super::super::{EXPLICIT_VR_LITTLE_ENDIAN, PixelRecipe};

#[derive(Debug, Clone, Copy)]
pub(in crate::generator) struct MetadataScRecipe {
    pub(in crate::generator) pixel: PixelRecipe,
    pub(in crate::generator) specific_character_set: &'static str,
    pub(in crate::generator) patient_name: &'static str,
}

const UTF8_PERSON_NAME_PIXELS: [u8; 4] = [0, 85, 170, 255];
const UTF8_PERSON_NAME_VALUES: [i32; 4] = [0, 85, 170, 255];

pub(in crate::generator) const METADATA_SC_RECIPES: &[MetadataScRecipe] = &[MetadataScRecipe {
    pixel: PixelRecipe {
        case_id: "metadata/sc/utf8_person_name",
        recipe_id: "metadata_sc_utf8_person_name",
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
        pixel_bytes: &UTF8_PERSON_NAME_PIXELS,
        pixel_values: &UTF8_PERSON_NAME_VALUES,
        pixel_min: 0,
        pixel_max: 255,
        visual_pattern: "2x2_monochrome_gradient_with_utf8_patient_name",
        semantic_note: "UTF-8 Person Name preserves alphabetic and ideographic component groups",
        palette: None,
        padding: None,
    },
    specific_character_set: "ISO_IR 192",
    patient_name: "Wang^XiaoDong=王^小東",
}];
