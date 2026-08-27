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

#[derive(Debug, Clone, Copy, PartialEq)]
pub(in crate::generator) struct U1ScRecipe {
    pub(in crate::generator) case_id: &'static str,
    pub(in crate::generator) recipe_id: &'static str,
    pub(in crate::generator) rows: u16,
    pub(in crate::generator) columns: u16,
    pub(in crate::generator) frames: u16,
    pub(in crate::generator) pixel_values: &'static [i32; 18],
    pub(in crate::generator) decoded_pixel_bytes: &'static [u8; 18],
    pub(in crate::generator) packed_pixel_bytes: &'static [u8; 4],
    pub(in crate::generator) significant_packed_bytes: usize,
    pub(in crate::generator) pixel_data_sha256: &'static str,
    pub(in crate::generator) decoded_frame_sha256: &'static [&'static str; 2],
}

impl U1ScRecipe {
    pub(in crate::generator) fn decoded_frames(self) -> Vec<&'static [u8]> {
        self.decoded_pixel_bytes
            .chunks_exact(usize::from(self.rows) * usize::from(self.columns))
            .collect()
    }
}

const U1_PIXEL_VALUES: [i32; 18] = [
    1, 0, 1, 0, 1, 0, 1, 0, 1, // checkerboard frame 1
    0, 1, 0, 1, 0, 1, 0, 1, 0, // inverse checkerboard frame 2
];
const U1_DECODED_PIXEL_BYTES: [u8; 18] = [
    1, 0, 1, 0, 1, 0, 1, 0, 1, // checkerboard frame 1
    0, 1, 0, 1, 0, 1, 0, 1, 0, // inverse checkerboard frame 2
];
const U1_PACKED_PIXEL_BYTES: [u8; 4] = [0x55, 0x55, 0x01, 0x00];
const U1_DECODED_FRAME_SHA256: [&str; 2] = [
    "a6188710c09cfbc77383ee0588dec2f7affa6e03e78aa900e9ae597a8d8faba3",
    "c520efb8f894a1125bb1a513a9b64ef957f7c2cd63835fd7e130357c47f989ae",
];

pub(in crate::generator) const U1_SC_RECIPE: U1ScRecipe = U1ScRecipe {
    case_id: "classic/sc/mono2_u1_native",
    recipe_id: "classic_sc_mono2_u1_native",
    rows: 3,
    columns: 3,
    frames: 2,
    pixel_values: &U1_PIXEL_VALUES,
    decoded_pixel_bytes: &U1_DECODED_PIXEL_BYTES,
    packed_pixel_bytes: &U1_PACKED_PIXEL_BYTES,
    significant_packed_bytes: 3,
    pixel_data_sha256: "9d6baf87a79d40ef2b145f92945a05cf156a2741e2c2834a3a7721d52757594b",
    decoded_frame_sha256: &U1_DECODED_FRAME_SHA256,
};
