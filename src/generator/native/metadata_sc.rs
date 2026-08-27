use dicom_core::VR;

use super::super::{EXPLICIT_VR_LITTLE_ENDIAN, PixelRecipe};

#[derive(Debug, Clone, Copy)]
pub(in crate::generator) struct PersonNameGroup {
    pub(in crate::generator) kind: &'static str,
    pub(in crate::generator) decoded_value: &'static str,
    pub(in crate::generator) components: [&'static str; 5],
}

#[derive(Debug, Clone, Copy)]
pub(in crate::generator) struct MetadataScRecipe {
    pub(in crate::generator) pixel: PixelRecipe,
    pub(in crate::generator) specific_character_sets: &'static [&'static str],
    pub(in crate::generator) patient_name_decoded: &'static str,
    pub(in crate::generator) patient_name_raw: &'static [u8],
    pub(in crate::generator) component_groups: &'static [PersonNameGroup],
    pub(in crate::generator) native_unicode_round_trip: bool,
    pub(in crate::generator) validation_name: &'static str,
    pub(in crate::generator) validation_message: &'static str,
}

const METADATA_PIXELS: [u8; 4] = [0, 85, 170, 255];
const METADATA_PIXEL_VALUES: [i32; 4] = [0, 85, 170, 255];

const UTF8_CHARACTER_SETS: &[&str] = &["ISO_IR 192"];
const UTF8_PERSON_NAME: &str = "Wang^XiaoDong=王^小東";
const UTF8_PERSON_NAME_GROUPS: &[PersonNameGroup] = &[
    PersonNameGroup {
        kind: "alphabetic",
        decoded_value: "Wang^XiaoDong",
        components: ["Wang", "XiaoDong", "", "", ""],
    },
    PersonNameGroup {
        kind: "ideographic",
        decoded_value: "王^小東",
        components: ["王", "小東", "", "", ""],
    },
];

const ISO2022_CHARACTER_SETS: &[&str] = &["", "ISO 2022 IR 87"];
const ISO2022_PERSON_NAME: &str = "Yamada^Tarou=山田^太郎=やまだ^たろう";
const ISO2022_PERSON_NAME_RAW: &[u8] = &[
    0x59, 0x61, 0x6D, 0x61, 0x64, 0x61, 0x5E, 0x54, 0x61, 0x72, 0x6F, 0x75, 0x3D, 0x1B, 0x24, 0x42,
    0x3B, 0x33, 0x45, 0x44, 0x1B, 0x28, 0x42, 0x5E, 0x1B, 0x24, 0x42, 0x42, 0x40, 0x4F, 0x3A, 0x1B,
    0x28, 0x42, 0x3D, 0x1B, 0x24, 0x42, 0x24, 0x64, 0x24, 0x5E, 0x24, 0x40, 0x1B, 0x28, 0x42, 0x5E,
    0x1B, 0x24, 0x42, 0x24, 0x3F, 0x24, 0x6D, 0x24, 0x26, 0x1B, 0x28, 0x42,
];
const ISO2022_PERSON_NAME_GROUPS: &[PersonNameGroup] = &[
    PersonNameGroup {
        kind: "alphabetic",
        decoded_value: "Yamada^Tarou",
        components: ["Yamada", "Tarou", "", "", ""],
    },
    PersonNameGroup {
        kind: "ideographic",
        decoded_value: "山田^太郎",
        components: ["山田", "太郎", "", "", ""],
    },
    PersonNameGroup {
        kind: "phonetic",
        decoded_value: "やまだ^たろう",
        components: ["やまだ", "たろう", "", "", ""],
    },
];

pub(in crate::generator) const METADATA_SC_RECIPES: &[MetadataScRecipe] = &[
    MetadataScRecipe {
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
            pixel_bytes: &METADATA_PIXELS,
            pixel_values: &METADATA_PIXEL_VALUES,
            pixel_min: 0,
            pixel_max: 255,
            visual_pattern: "2x2_monochrome_gradient_with_utf8_patient_name",
            semantic_note: "UTF-8 Person Name preserves alphabetic and ideographic component groups",
            palette: None,
            padding: None,
        },
        specific_character_sets: UTF8_CHARACTER_SETS,
        patient_name_decoded: UTF8_PERSON_NAME,
        patient_name_raw: UTF8_PERSON_NAME.as_bytes(),
        component_groups: UTF8_PERSON_NAME_GROUPS,
        native_unicode_round_trip: true,
        validation_name: "utf8_person_name_round_trip",
        validation_message: "The native writer output reopened with the exact declared UTF-8 character set and decoded Person Name.",
    },
    MetadataScRecipe {
        pixel: PixelRecipe {
            case_id: "metadata/sc/iso2022_person_name_component_groups",
            recipe_id: "metadata_sc_iso2022_person_name_component_groups",
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
            pixel_bytes: &METADATA_PIXELS,
            pixel_values: &METADATA_PIXEL_VALUES,
            pixel_min: 0,
            pixel_max: 255,
            visual_pattern: "2x2_monochrome_gradient_with_iso2022_person_name",
            semantic_note: "ISO 2022 Person Name switches from ASCII to JIS X 0208 in ideographic and phonetic groups",
            palette: None,
            padding: None,
        },
        specific_character_sets: ISO2022_CHARACTER_SETS,
        patient_name_decoded: ISO2022_PERSON_NAME,
        patient_name_raw: ISO2022_PERSON_NAME_RAW,
        component_groups: ISO2022_PERSON_NAME_GROUPS,
        native_unicode_round_trip: false,
        validation_name: "iso2022_person_name_encoded_round_trip",
        validation_message: "The native writer output reopened with the exact declared character-set values and ISO 2022 PN bytes; independent readers prove Unicode semantics.",
    },
];

#[cfg(test)]
mod tests {
    use super::{ISO2022_PERSON_NAME_RAW, METADATA_SC_RECIPES};
    use crate::sha256_hex;

    #[test]
    fn iso2022_recipe_locks_the_standard_example_bytes() {
        let recipe = METADATA_SC_RECIPES
            .iter()
            .find(|recipe| recipe.pixel.case_id.contains("iso2022"))
            .expect("ISO 2022 recipe must exist");
        assert_eq!(recipe.specific_character_sets, ["", "ISO 2022 IR 87"]);
        assert_eq!(recipe.patient_name_raw, ISO2022_PERSON_NAME_RAW);
        assert_eq!(recipe.patient_name_raw.len(), 60);
        assert_eq!(
            sha256_hex(recipe.patient_name_raw),
            "b206df163ce0b4d071469834428bf0b87b241931c81110362ce480d73d7490af"
        );
        assert_eq!(recipe.component_groups.len(), 3);
    }
}
