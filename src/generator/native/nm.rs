#[derive(Debug, Clone, Copy, PartialEq)]
pub(in crate::generator) struct ClassicNmRecipe {
    pub(in crate::generator) case_id: &'static str,
    pub(in crate::generator) recipe_id: &'static str,
    pub(in crate::generator) rows: u16,
    pub(in crate::generator) columns: u16,
    pub(in crate::generator) image_type: &'static str,
    pub(in crate::generator) frames: &'static [ClassicNmFrameRecipe],
    pub(in crate::generator) energy_window_vector: &'static [u16],
    pub(in crate::generator) detector_vector: &'static [u16],
    pub(in crate::generator) energy_windows: &'static [ClassicNmEnergyWindowRecipe],
    pub(in crate::generator) detectors: &'static [ClassicNmDetectorRecipe],
    pub(in crate::generator) actual_frame_duration_ms: u32,
    pub(in crate::generator) counts_accumulated: u32,
}

impl ClassicNmRecipe {
    pub(in crate::generator) const fn frame_count(self) -> usize {
        self.frames.len()
    }

    pub(in crate::generator) const fn computed_counts_accumulated(self) -> u32 {
        let mut total = 0;
        let mut frame_index = 0;
        while frame_index < self.frames.len() {
            let values = self.frames[frame_index].pixel_values;
            let mut value_index = 0;
            while value_index < values.len() {
                total += values[value_index] as u32;
                value_index += 1;
            }
            frame_index += 1;
        }
        total
    }

    pub(in crate::generator) const fn dimensions_are_consistent(self) -> bool {
        if self.frames.len() != self.energy_window_vector.len()
            || self.frames.len() != self.detector_vector.len()
        {
            return false;
        }

        let mut index = 0;
        while index < self.frames.len() {
            let frame = self.frames[index];
            if frame.frame_number as usize != index + 1
                || frame.energy_window_index != self.energy_window_vector[index]
                || frame.detector_index != self.detector_vector[index]
                || frame.energy_window_index == 0
                || frame.energy_window_index as usize > self.energy_windows.len()
                || frame.detector_index == 0
                || frame.detector_index as usize > self.detectors.len()
            {
                return false;
            }
            index += 1;
        }
        true
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::generator) struct ClassicNmFrameRecipe {
    pub(in crate::generator) frame_number: u16,
    pub(in crate::generator) energy_window_index: u16,
    pub(in crate::generator) detector_index: u16,
    pub(in crate::generator) pixel_values: &'static [u16; 4],
    pub(in crate::generator) pixel_bytes_le: &'static [u8; 8],
    pub(in crate::generator) frame_sha256: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(in crate::generator) struct ClassicNmEnergyWindowRecipe {
    pub(in crate::generator) index: u16,
    pub(in crate::generator) name: &'static str,
    pub(in crate::generator) lower_limit_kev: f64,
    pub(in crate::generator) upper_limit_kev: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(in crate::generator) struct ClassicNmDetectorRecipe {
    pub(in crate::generator) index: u16,
    pub(in crate::generator) collimator_type: &'static str,
    pub(in crate::generator) focal_distance_mm: f64,
    pub(in crate::generator) start_angle_degrees: f64,
    pub(in crate::generator) image_orientation_patient: [f64; 6],
    pub(in crate::generator) image_position_patient: [f64; 3],
}

const FRAME_1_VALUES: [u16; 4] = [0, 1, 2, 3];
const FRAME_2_VALUES: [u16; 4] = [10, 11, 12, 13];
const FRAME_3_VALUES: [u16; 4] = [100, 101, 102, 103];
const FRAME_4_VALUES: [u16; 4] = [110, 111, 112, 113];

const FRAME_1_BYTES_LE: [u8; 8] = [0, 0, 1, 0, 2, 0, 3, 0];
const FRAME_2_BYTES_LE: [u8; 8] = [10, 0, 11, 0, 12, 0, 13, 0];
const FRAME_3_BYTES_LE: [u8; 8] = [100, 0, 101, 0, 102, 0, 103, 0];
const FRAME_4_BYTES_LE: [u8; 8] = [110, 0, 111, 0, 112, 0, 113, 0];

const CLASSIC_NM_FRAMES: &[ClassicNmFrameRecipe] = &[
    ClassicNmFrameRecipe {
        frame_number: 1,
        energy_window_index: 1,
        detector_index: 1,
        pixel_values: &FRAME_1_VALUES,
        pixel_bytes_le: &FRAME_1_BYTES_LE,
        frame_sha256: "245bbd9d484dcf27c714e2690cd6544973de5d54aa9cd82eab23d6046a65faa8",
    },
    ClassicNmFrameRecipe {
        frame_number: 2,
        energy_window_index: 1,
        detector_index: 2,
        pixel_values: &FRAME_2_VALUES,
        pixel_bytes_le: &FRAME_2_BYTES_LE,
        frame_sha256: "a58214fbfec2da6f1e9fc6a2641c8a0af73fb383860180a73d4439fe31b44189",
    },
    ClassicNmFrameRecipe {
        frame_number: 3,
        energy_window_index: 2,
        detector_index: 1,
        pixel_values: &FRAME_3_VALUES,
        pixel_bytes_le: &FRAME_3_BYTES_LE,
        frame_sha256: "4908c41ec85a7552278ed886fa3c43819f44d4df5b73138a9c5855926c750a58",
    },
    ClassicNmFrameRecipe {
        frame_number: 4,
        energy_window_index: 2,
        detector_index: 2,
        pixel_values: &FRAME_4_VALUES,
        pixel_bytes_le: &FRAME_4_BYTES_LE,
        frame_sha256: "a12837f26e181e5420b019bae0940e221d2927e13fea963ad899945c34c697fe",
    },
];

const CLASSIC_NM_ENERGY_WINDOWS: &[ClassicNmEnergyWindowRecipe] = &[
    ClassicNmEnergyWindowRecipe {
        index: 1,
        name: "Tc99m Photopeak",
        lower_limit_kev: 126.0,
        upper_limit_kev: 154.0,
    },
    ClassicNmEnergyWindowRecipe {
        index: 2,
        name: "Tc99m Scatter",
        lower_limit_kev: 100.0,
        upper_limit_kev: 120.0,
    },
];

const CLASSIC_NM_DETECTORS: &[ClassicNmDetectorRecipe] = &[
    ClassicNmDetectorRecipe {
        index: 1,
        collimator_type: "PARA",
        focal_distance_mm: 0.0,
        start_angle_degrees: 0.0,
        image_orientation_patient: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
        image_position_patient: [0.0, 0.0, 0.0],
    },
    ClassicNmDetectorRecipe {
        index: 2,
        collimator_type: "PARA",
        focal_distance_mm: 0.0,
        start_angle_degrees: 180.0,
        image_orientation_patient: [-1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
        image_position_patient: [0.0, 0.0, 0.0],
    },
];

const ENERGY_WINDOW_VECTOR: &[u16] = &[1, 1, 2, 2];
const DETECTOR_VECTOR: &[u16] = &[1, 2, 1, 2];

pub(in crate::generator) const CLASSIC_NM_RECIPE: ClassicNmRecipe = ClassicNmRecipe {
    case_id: "classic/nm/multiframe_explicit_le",
    recipe_id: "classic_nm_multiframe_explicit_le",
    rows: 2,
    columns: 2,
    image_type: "ORIGINAL\\PRIMARY\\STATIC\\EMISSION",
    frames: CLASSIC_NM_FRAMES,
    energy_window_vector: ENERGY_WINDOW_VECTOR,
    detector_vector: DETECTOR_VECTOR,
    energy_windows: CLASSIC_NM_ENERGY_WINDOWS,
    detectors: CLASSIC_NM_DETECTORS,
    actual_frame_duration_ms: 1_000,
    counts_accumulated: 904,
};

pub(in crate::generator) const CLASSIC_NM_RECIPES: &[ClassicNmRecipe] = &[CLASSIC_NM_RECIPE];

const _: () = assert!(CLASSIC_NM_RECIPE.frame_count() == 4);
const _: () = assert!(CLASSIC_NM_RECIPE.dimensions_are_consistent());
const _: () = assert!(
    CLASSIC_NM_RECIPE.computed_counts_accumulated() == CLASSIC_NM_RECIPE.counts_accumulated
);
