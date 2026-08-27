#[derive(Debug, Clone, Copy, PartialEq)]
pub(in crate::generator) struct U32ScRecipe {
    pub(in crate::generator) case_id: &'static str,
    pub(in crate::generator) recipe_id: &'static str,
    pub(in crate::generator) rows: u16,
    pub(in crate::generator) columns: u16,
    pub(in crate::generator) pixel_values: &'static [u32; 4],
    pub(in crate::generator) pixel_bytes_le: &'static [u8; 16],
    pub(in crate::generator) pixel_data_sha256: &'static str,
}

impl U32ScRecipe {
    pub(in crate::generator) const fn pixel_bytes_are_consistent(self) -> bool {
        let mut index = 0;
        while index < self.pixel_values.len() {
            let bytes = self.pixel_values[index].to_le_bytes();
            let offset = index * 4;
            if self.pixel_bytes_le[offset] != bytes[0]
                || self.pixel_bytes_le[offset + 1] != bytes[1]
                || self.pixel_bytes_le[offset + 2] != bytes[2]
                || self.pixel_bytes_le[offset + 3] != bytes[3]
            {
                return false;
            }
            index += 1;
        }
        true
    }
}

const PIXEL_VALUES: [u32; 4] = [0, 65_535, 2_147_483_648, 4_294_967_295];
const PIXEL_BYTES_LE: [u8; 16] = [
    0x00, 0x00, 0x00, 0x00, 0xff, 0xff, 0x00, 0x00, 0x00, 0x00, 0x00, 0x80, 0xff, 0xff, 0xff, 0xff,
];

pub(in crate::generator) const U32_SC_RECIPE: U32ScRecipe = U32ScRecipe {
    case_id: "classic/sc/mono2_u32_explicit_le",
    recipe_id: "classic_sc_mono2_u32_explicit_le",
    rows: 2,
    columns: 2,
    pixel_values: &PIXEL_VALUES,
    pixel_bytes_le: &PIXEL_BYTES_LE,
    pixel_data_sha256: "56bca1a85c2838126b1d1a5fbedfe731839496d972df2c6ab33e1a1183392b41",
};
