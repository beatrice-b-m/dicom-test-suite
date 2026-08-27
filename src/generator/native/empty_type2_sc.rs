use dicom_core::{Tag, VR};
use dicom_dictionary_std::tags;

use super::super::{EXPLICIT_VR_LITTLE_ENDIAN, PixelRecipe};

#[derive(Debug, Clone, Copy)]
pub(in crate::generator) struct EmptyType2Attribute {
    pub(in crate::generator) tag: Tag,
    pub(in crate::generator) tag_text: &'static str,
    pub(in crate::generator) keyword: &'static str,
    pub(in crate::generator) vr: VR,
}

#[derive(Debug, Clone, Copy)]
pub(in crate::generator) struct EmptyType2ScRecipe {
    pub(in crate::generator) pixel: PixelRecipe,
    pub(in crate::generator) attributes: &'static [EmptyType2Attribute],
}

const PIXELS: [u8; 4] = [0, 85, 170, 255];
const PIXEL_VALUES: [i32; 4] = [0, 85, 170, 255];

const EMPTY_ATTRIBUTES: &[EmptyType2Attribute] = &[
    EmptyType2Attribute {
        tag: tags::PATIENT_NAME,
        tag_text: "0010,0010",
        keyword: "PatientName",
        vr: VR::PN,
    },
    EmptyType2Attribute {
        tag: tags::PATIENT_BIRTH_DATE,
        tag_text: "0010,0030",
        keyword: "PatientBirthDate",
        vr: VR::DA,
    },
    EmptyType2Attribute {
        tag: tags::PATIENT_SEX,
        tag_text: "0010,0040",
        keyword: "PatientSex",
        vr: VR::CS,
    },
    EmptyType2Attribute {
        tag: tags::REFERRING_PHYSICIAN_NAME,
        tag_text: "0008,0090",
        keyword: "ReferringPhysicianName",
        vr: VR::PN,
    },
    EmptyType2Attribute {
        tag: tags::ACCESSION_NUMBER,
        tag_text: "0008,0050",
        keyword: "AccessionNumber",
        vr: VR::SH,
    },
];

pub(in crate::generator) const EMPTY_TYPE2_SC_RECIPE: EmptyType2ScRecipe = EmptyType2ScRecipe {
    pixel: PixelRecipe {
        case_id: "metadata/sc/empty_type2_attributes",
        recipe_id: "metadata_sc_empty_type2_attributes",
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
        visual_pattern: "2x2_monochrome_gradient_with_empty_type2_attributes",
        semantic_note: "Five required Patient and General Study Type 2 attributes are present with zero Value Length",
        palette: None,
        padding: None,
    },
    attributes: EMPTY_ATTRIBUTES,
};

#[cfg(test)]
mod tests {
    use super::EMPTY_TYPE2_SC_RECIPE;

    #[test]
    fn recipe_locks_the_five_required_empty_attributes() {
        let attributes = EMPTY_TYPE2_SC_RECIPE.attributes;
        assert_eq!(attributes.len(), 5);
        assert_eq!(attributes[0].tag_text, "0010,0010");
        assert_eq!(attributes[4].keyword, "AccessionNumber");
    }
}
