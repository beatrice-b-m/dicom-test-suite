use dicom_core::{Tag, VR};

use super::super::{EXPLICIT_VR_LITTLE_ENDIAN, PixelRecipe};

#[derive(Debug, Clone, Copy)]
pub(in crate::generator) enum PrivateValue {
    Lo(&'static str),
    Us(u16),
}

#[derive(Debug, Clone, Copy)]
pub(in crate::generator) struct PrivateElementRecipe {
    pub(in crate::generator) tag: Tag,
    pub(in crate::generator) tag_text: &'static str,
    pub(in crate::generator) value: PrivateValue,
}

#[derive(Debug, Clone, Copy)]
pub(in crate::generator) struct PrivateCreatorBlockRecipe {
    pub(in crate::generator) creator_tag: Tag,
    pub(in crate::generator) creator_tag_text: &'static str,
    pub(in crate::generator) creator_id: &'static str,
    pub(in crate::generator) block_start_tag: &'static str,
    pub(in crate::generator) block_end_tag: &'static str,
    pub(in crate::generator) elements: &'static [PrivateElementRecipe],
}

#[derive(Debug, Clone, Copy)]
pub(in crate::generator) struct PrivateCreatorScRecipe {
    pub(in crate::generator) pixel: PixelRecipe,
    pub(in crate::generator) blocks: &'static [PrivateCreatorBlockRecipe],
}

const PIXELS: [u8; 4] = [0, 85, 170, 255];
const PIXEL_VALUES: [i32; 4] = [0, 85, 170, 255];

const ALPHA_0011_ELEMENTS: &[PrivateElementRecipe] = &[
    PrivateElementRecipe {
        tag: Tag(0x0011, 0x1001),
        tag_text: "0011,1001",
        value: PrivateValue::Lo("ALPHA-GROUP-0011"),
    },
    PrivateElementRecipe {
        tag: Tag(0x0011, 0x10F0),
        tag_text: "0011,10F0",
        value: PrivateValue::Us(0x1234),
    },
];
const BETA_0011_ELEMENTS: &[PrivateElementRecipe] = &[PrivateElementRecipe {
    tag: Tag(0x0011, 0x1201),
    tag_text: "0011,1201",
    value: PrivateValue::Lo("BETA-BLOCK-12"),
}];
const ALPHA_0013_ELEMENTS: &[PrivateElementRecipe] = &[PrivateElementRecipe {
    tag: Tag(0x0013, 0x1101),
    tag_text: "0013,1101",
    value: PrivateValue::Lo("ALPHA-GROUP-0013"),
}];

const BLOCKS: &[PrivateCreatorBlockRecipe] = &[
    PrivateCreatorBlockRecipe {
        creator_tag: Tag(0x0011, 0x0010),
        creator_tag_text: "0011,0010",
        creator_id: "DTS_PRIVATE_ALPHA",
        block_start_tag: "0011,1000",
        block_end_tag: "0011,10FF",
        elements: ALPHA_0011_ELEMENTS,
    },
    PrivateCreatorBlockRecipe {
        creator_tag: Tag(0x0011, 0x0012),
        creator_tag_text: "0011,0012",
        creator_id: "DTS_PRIVATE_BETA",
        block_start_tag: "0011,1200",
        block_end_tag: "0011,12FF",
        elements: BETA_0011_ELEMENTS,
    },
    PrivateCreatorBlockRecipe {
        creator_tag: Tag(0x0013, 0x0011),
        creator_tag_text: "0013,0011",
        creator_id: "DTS_PRIVATE_ALPHA",
        block_start_tag: "0013,1100",
        block_end_tag: "0013,11FF",
        elements: ALPHA_0013_ELEMENTS,
    },
];

pub(in crate::generator) const PRIVATE_CREATOR_SC_RECIPE: PrivateCreatorScRecipe =
    PrivateCreatorScRecipe {
        pixel: PixelRecipe {
            case_id: "metadata/sc/private_creator_blocks",
            recipe_id: "metadata_sc_private_creator_blocks",
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
            visual_pattern: "2x2_monochrome_gradient_with_private_creator_blocks",
            semantic_note: "Nonsequential private blocks in group 0011 and an independent reused creator in group 0013 preserve creator ownership",
            palette: None,
            padding: None,
        },
        blocks: BLOCKS,
    };

#[cfg(test)]
mod tests {
    use super::PRIVATE_CREATOR_SC_RECIPE;

    #[test]
    fn recipe_locks_nonsequential_and_cross_group_creator_scopes() {
        let blocks = PRIVATE_CREATOR_SC_RECIPE.blocks;
        assert_eq!(blocks.len(), 3);
        assert_eq!(blocks[0].creator_tag_text, "0011,0010");
        assert_eq!(blocks[1].creator_tag_text, "0011,0012");
        assert_eq!(blocks[2].creator_tag_text, "0013,0011");
        assert_eq!(blocks[0].creator_id, blocks[2].creator_id);
    }
}
