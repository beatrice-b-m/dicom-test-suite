#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::generator) struct ClassicUsMultiframeRecipe {
    pub(in crate::generator) case_id: &'static str,
    pub(in crate::generator) recipe_id: &'static str,
    pub(in crate::generator) rows: u16,
    pub(in crate::generator) columns: u16,
    pub(in crate::generator) image_type: &'static str,
    pub(in crate::generator) frame_increment_pointer: &'static str,
    pub(in crate::generator) frame_time_ms: u32,
    pub(in crate::generator) frame_relative_times_ms: &'static [u32],
    pub(in crate::generator) frames: &'static [ClassicUsFrameRecipe],
    pub(in crate::generator) payload_sha256: &'static str,
    pub(in crate::generator) lossy_image_compression: &'static str,
    pub(in crate::generator) color_data_present: bool,
    pub(in crate::generator) spatially_related_frames: bool,
    pub(in crate::generator) region_calibrated: bool,
}

impl ClassicUsMultiframeRecipe {
    pub(in crate::generator) const fn pixel_count_per_frame(self) -> usize {
        self.rows as usize * self.columns as usize
    }

    pub(in crate::generator) const fn frame_count(self) -> usize {
        self.frames.len()
    }

    pub(in crate::generator) const fn dimensions_and_order_are_consistent(self) -> bool {
        if self.frames.len() != self.frame_relative_times_ms.len() {
            return false;
        }

        let mut index = 0;
        while index < self.frames.len() {
            let frame = self.frames[index];
            if frame.frame_number as usize != index + 1
                || frame.pixel_values.len() != self.pixel_count_per_frame()
                || frame.pixel_bytes.len() != self.pixel_count_per_frame()
            {
                return false;
            }

            let mut pixel_index = 0;
            while pixel_index < frame.pixel_values.len() {
                if frame.pixel_values[pixel_index] != frame.pixel_bytes[pixel_index] {
                    return false;
                }
                pixel_index += 1;
            }
            index += 1;
        }
        true
    }

    pub(in crate::generator) const fn relative_times_are_derived(self) -> bool {
        let mut index = 0;
        while index < self.frame_relative_times_ms.len() {
            if self.frame_relative_times_ms[index] != index as u32 * self.frame_time_ms {
                return false;
            }
            index += 1;
        }
        true
    }

    pub(in crate::generator) const fn hash_lengths_are_consistent(self) -> bool {
        if self.payload_sha256.len() != 64 {
            return false;
        }

        let mut index = 0;
        while index < self.frames.len() {
            if self.frames[index].frame_sha256.len() != 64 {
                return false;
            }
            index += 1;
        }
        true
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::generator) struct ClassicUsFrameRecipe {
    pub(in crate::generator) frame_number: u16,
    pub(in crate::generator) pixel_values: &'static [u8; 16],
    pub(in crate::generator) pixel_bytes: &'static [u8; 16],
    pub(in crate::generator) frame_sha256: &'static str,
}

const FRAME_1_PIXELS: [u8; 16] = [
    0, 16, 32, 48, 16, 64, 80, 64, 32, 80, 255, 80, 48, 64, 80, 64,
];
const FRAME_2_PIXELS: [u8; 16] = [
    0, 16, 32, 48, 16, 64, 80, 64, 32, 80, 80, 255, 48, 64, 80, 80,
];
const FRAME_3_PIXELS: [u8; 16] = [
    0, 16, 32, 48, 16, 64, 80, 64, 32, 80, 80, 80, 48, 64, 255, 80,
];
const FRAME_4_PIXELS: [u8; 16] = [
    0, 16, 32, 48, 16, 64, 80, 64, 32, 80, 80, 80, 48, 255, 80, 64,
];

const CLASSIC_US_FRAMES: &[ClassicUsFrameRecipe] = &[
    ClassicUsFrameRecipe {
        frame_number: 1,
        pixel_values: &FRAME_1_PIXELS,
        pixel_bytes: &FRAME_1_PIXELS,
        frame_sha256: "be422fa58b70ec0d940f28a4dba3dadac62d4583b9ecba1e73d65b37ee9733e7",
    },
    ClassicUsFrameRecipe {
        frame_number: 2,
        pixel_values: &FRAME_2_PIXELS,
        pixel_bytes: &FRAME_2_PIXELS,
        frame_sha256: "303d53edfa9bf6eeeb81dba8a6a4c1a9c2e1cb0ea773f90afb583d1132d88eee",
    },
    ClassicUsFrameRecipe {
        frame_number: 3,
        pixel_values: &FRAME_3_PIXELS,
        pixel_bytes: &FRAME_3_PIXELS,
        frame_sha256: "7f8a6e2fa2665b2465075b9e0cf86dfb0646f6f21a2a647525476e5bb6e489bb",
    },
    ClassicUsFrameRecipe {
        frame_number: 4,
        pixel_values: &FRAME_4_PIXELS,
        pixel_bytes: &FRAME_4_PIXELS,
        frame_sha256: "8c213da26d1c57661b68238ac5c1f1d9417f661e0ab578846bf84040e753f650",
    },
];

const FRAME_RELATIVE_TIMES_MS: &[u32] = &[0, 100, 200, 300];

pub(in crate::generator) const CLASSIC_US_MULTIFRAME_RECIPE: ClassicUsMultiframeRecipe =
    ClassicUsMultiframeRecipe {
        case_id: "classic/us/multiframe_explicit_le",
        recipe_id: "classic_us_multiframe_explicit_le",
        rows: 4,
        columns: 4,
        image_type: "ORIGINAL\\PRIMARY\\ABDOMINAL\\0001",
        frame_increment_pointer: "0018,1063",
        frame_time_ms: 100,
        frame_relative_times_ms: FRAME_RELATIVE_TIMES_MS,
        frames: CLASSIC_US_FRAMES,
        payload_sha256: "060e2c56c9728f787339515ef16bc8c1adfbfb4fb85b2d2c18f115c17b439bc9",
        lossy_image_compression: "00",
        color_data_present: false,
        spatially_related_frames: false,
        region_calibrated: false,
    };

pub(in crate::generator) const CLASSIC_US_MULTIFRAME_RECIPES: &[ClassicUsMultiframeRecipe] =
    &[CLASSIC_US_MULTIFRAME_RECIPE];

const _: () = assert!(CLASSIC_US_MULTIFRAME_RECIPE.pixel_count_per_frame() == 16);
const _: () = assert!(CLASSIC_US_MULTIFRAME_RECIPE.frame_count() == 4);
const _: () = assert!(CLASSIC_US_MULTIFRAME_RECIPE.dimensions_and_order_are_consistent());
const _: () = assert!(CLASSIC_US_MULTIFRAME_RECIPE.relative_times_are_derived());
const _: () = assert!(CLASSIC_US_MULTIFRAME_RECIPE.hash_lengths_are_consistent());
