use super::super::{EXPLICIT_VR_LITTLE_ENDIAN, PixelRecipe};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::generator) enum SequenceLengthVariantId {
    Defined,
    Undefined,
}

impl SequenceLengthVariantId {
    pub(in crate::generator) const fn as_str(self) -> &'static str {
        match self {
            Self::Defined => "defined",
            Self::Undefined => "undefined",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(in crate::generator) struct SequenceLengthVariant {
    pub(in crate::generator) variant_id: SequenceLengthVariantId,
    pub(in crate::generator) file_name: &'static str,
}

#[derive(Debug, Clone, Copy)]
pub(in crate::generator) struct SequenceLengthScRecipe {
    pub(in crate::generator) pixel: PixelRecipe,
    pub(in crate::generator) variants: &'static [SequenceLengthVariant],
}

pub(in crate::generator) const CODE_VALUE: &str = "69536005";
pub(in crate::generator) const CODING_SCHEME_DESIGNATOR: &str = "SCT";
pub(in crate::generator) const CODE_MEANING: &str = "Head";
pub(in crate::generator) const ITEM_DATASET_ENCODED_LENGTH: u32 = 40;
pub(in crate::generator) const UNDEFINED_ITEM_ENCODED_LENGTH: u32 = 56;

const PIXELS: [u8; 4] = [0, 85, 170, 255];
const PIXEL_VALUES: [i32; 4] = [0, 85, 170, 255];
const VARIANTS: &[SequenceLengthVariant] = &[
    SequenceLengthVariant {
        variant_id: SequenceLengthVariantId::Defined,
        file_name: "defined.dcm",
    },
    SequenceLengthVariant {
        variant_id: SequenceLengthVariantId::Undefined,
        file_name: "undefined.dcm",
    },
];

pub(in crate::generator) const SEQUENCE_LENGTH_SC_RECIPE: SequenceLengthScRecipe =
    SequenceLengthScRecipe {
        pixel: PixelRecipe {
            case_id: "metadata/sc/defined_undefined_sequence_lengths",
            recipe_id: "metadata_sc_defined_undefined_sequence_lengths",
            rows: 2,
            columns: 2,
            photometric_interpretation: "MONOCHROME2",
            samples_per_pixel: 1,
            planar_configuration: None,
            bits_allocated: 8,
            bits_stored: 8,
            high_bit: 7,
            pixel_representation: 0,
            pixel_vr: dicom_core::VR::OB,
            transfer_syntax: EXPLICIT_VR_LITTLE_ENDIAN,
            pixel_bytes: &PIXELS,
            pixel_values: &PIXEL_VALUES,
            pixel_min: 0,
            pixel_max: 255,
            visual_pattern: "2x2_monochrome_gradient_with_sequence_length_variants",
            semantic_note: "Equivalent Anatomic Region Sequence content is encoded once with defined SQ length and once with undefined SQ length",
            palette: None,
            padding: None,
        },
        variants: VARIANTS,
    };

#[cfg(test)]
mod tests {
    use super::{SEQUENCE_LENGTH_SC_RECIPE, SequenceLengthVariantId};

    #[test]
    fn recipe_locks_defined_and_undefined_sequence_variants() {
        let variants = SEQUENCE_LENGTH_SC_RECIPE.variants;
        assert_eq!(variants.len(), 2);
        assert_eq!(variants[0].variant_id, SequenceLengthVariantId::Defined);
        assert_eq!(variants[1].variant_id, SequenceLengthVariantId::Undefined);
        assert_eq!(variants[0].file_name, "defined.dcm");
        assert_eq!(variants[1].file_name, "undefined.dcm");
    }
}
