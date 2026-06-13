use std::fs;
use std::path::PathBuf;

use dicom_core::{DataElement, PrimitiveValue, Tag, VR, value::DataSetSequence};
use dicom_dictionary_std::{tags, uids};
use dicom_object::{FileMetaTableBuilder, InMemDicomObject};
use serde_json::Value;

use crate::{
    DeterministicUidInput, GenerateError, PreparedGenerationRun, UidRole, deterministic_uid,
    sha256_hex,
    validation::{
        CrImageExpectations, CtImageExpectations, DxImageExpectations, MgImageExpectations,
        MrImageExpectations, Part10Expectations, PixelDataLengthFormula, validate_part10_file,
    },
};

const PIXEL_RECIPE_VERSION: &str = "0.1.0";
const CLASSIC_CT_RECIPE_VERSION: &str = "0.1.0";
const CLASSIC_MG_RECIPE_VERSION: &str = "0.1.0";
const CLASSIC_DX_RECIPE_VERSION: &str = "0.1.0";
const CLASSIC_CR_RECIPE_VERSION: &str = "0.1.0";
const CLASSIC_MR_RECIPE_VERSION: &str = "0.1.0";
const MONO_PIXELS: [u8; 4] = [0, 85, 170, 255];
const RGB_PLANAR0_PIXELS: [u8; 12] = [255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255];
const RGB_PLANAR1_PIXELS: [u8; 12] = [255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255];
const MONO_U16_PIXELS: [u8; 8] = [0, 0, 0x55, 0x55, 0xaa, 0xaa, 0xff, 0xff];
const MONO_U16_VALUES: [i32; 4] = [0, 21845, 43690, 65535];
const MONO_I16_PIXELS: [u8; 8] = [0x00, 0x80, 0x55, 0xd5, 0xaa, 0x2a, 0xff, 0x7f];
const MONO_I16_VALUES: [i32; 4] = [-32768, -10923, 10922, 32767];
const MONO_U16_ODD_3X3_PIXELS: [u8; 18] = [0, 0, 1, 0, 2, 0, 3, 0, 4, 0, 5, 0, 6, 0, 7, 0, 8, 0];
const MONO_U16_ODD_3X3_VALUES: [i32; 9] = [0, 1, 2, 3, 4, 5, 6, 7, 8];
const MONO_U16_RECT_2X3_PIXELS: [u8; 12] = [0, 0, 1, 0, 2, 0, 3, 0, 4, 0, 5, 0];
const MONO_U16_RECT_2X3_VALUES: [i32; 6] = [0, 1, 2, 3, 4, 5];
const MONO_U16_TINY_1X1_PIXELS: [u8; 2] = [0xff, 0xff];
const MONO_U16_TINY_1X1_VALUES: [i32; 1] = [65535];
const MONO_U16_PADDING_PIXELS: [u8; 8] = [0, 0, 0xe8, 0x03, 0xd0, 0x07, 0xb8, 0x0b];
const MONO_U16_PADDING_VALUES: [i32; 4] = [0, 1000, 2000, 3000];
const YBR_FULL_PLANAR0_PIXELS: [u8; 12] = [76, 85, 255, 150, 44, 21, 29, 255, 107, 255, 128, 128];
const YBR_FULL_422_PIXELS: [u8; 8] = [76, 150, 65, 138, 29, 255, 192, 118];
const PALETTE_COLOR_PIXELS: [u8; 4] = [0, 1, 2, 3];
const PALETTE_COLOR_VALUES: [i32; 4] = [0, 1, 2, 3];
const PALETTE_DESCRIPTOR: [u16; 3] = [4, 0, 16];
const PALETTE_RED_DATA: [u8; 8] = [0xff, 0xff, 0, 0, 0, 0, 0xff, 0xff];
const PALETTE_GREEN_DATA: [u8; 8] = [0, 0, 0xff, 0xff, 0, 0, 0xff, 0xff];
const PALETTE_BLUE_DATA: [u8; 8] = [0, 0, 0, 0, 0xff, 0xff, 0xff, 0xff];
const PALETTE_COLOR_LUT: PaletteRecipe = PaletteRecipe {
    descriptor: PALETTE_DESCRIPTOR,
    red_data: &PALETTE_RED_DATA,
    green_data: &PALETTE_GREEN_DATA,
    blue_data: &PALETTE_BLUE_DATA,
};
const CT_I16_12BIT_PIXELS: [u8; 8] = [0x00, 0x0c, 0x00, 0x00, 0x00, 0x04, 0xff, 0x07];
const CT_I16_12BIT_VALUES: [i32; 4] = [-1024, 0, 1024, 2047];
const MG_U16_12BIT_PIXELS: [u8; 8] = [0x00, 0x00, 0x55, 0x05, 0xaa, 0x0a, 0xff, 0x0f];
const MG_U16_12BIT_VALUES: [i32; 4] = [0, 1365, 2730, 4095];
const DX_U16_12BIT_PIXELS: [u8; 8] = [0x00, 0x00, 0x00, 0x04, 0x00, 0x08, 0xff, 0x0f];
const DX_U16_12BIT_VALUES: [i32; 4] = [0, 1024, 2048, 4095];
const CR_U8_PIXELS: [u8; 4] = [0, 1, 2, 3];
const CR_U8_VALUES: [i32; 4] = [0, 1, 2, 3];
const CR_OVERLAY_PIXELS: [u8; 2] = [0x09, 0x00];
const CR_MODALITY_LUT_DATA: [u8; 8] = [0, 0, 0, 4, 0, 8, 0xff, 0x0f];
const CR_VOI_LUT_DATA: [u8; 8] = [0, 0, 0x55, 0x55, 0xaa, 0xaa, 0xff, 0xff];
const MR_SLICE_1_PIXELS: [u8; 8] = [0, 0, 1, 0, 2, 0, 3, 0];
const MR_SLICE_2_PIXELS: [u8; 8] = [10, 0, 11, 0, 12, 0, 13, 0];
const MR_SLICE_3_PIXELS: [u8; 8] = [20, 0, 21, 0, 22, 0, 23, 0];
const MR_SLICE_1_VALUES: [i32; 4] = [0, 1, 2, 3];
const MR_SLICE_2_VALUES: [i32; 4] = [10, 11, 12, 13];
const MR_SLICE_3_VALUES: [i32; 4] = [20, 21, 22, 23];

const PIXEL_RECIPES: &[PixelRecipe] = &[
    PixelRecipe {
        case_id: "classic/sc/mono2_u8_explicit_le",
        recipe_id: "sc_mono2_u8",
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
        pixel_bytes: &MONO_PIXELS,
        pixel_values: &[0, 85, 170, 255],
        pixel_min: 0,
        pixel_max: 255,
        visual_pattern: "2x2_monochrome_gradient",
        semantic_note: "minimum sample value displays as black",
        palette: None,
        padding: None,
    },
    PixelRecipe {
        case_id: "classic/sc/mono1_u8_explicit_le",
        recipe_id: "sc_mono1_u8",
        rows: 2,
        columns: 2,
        photometric_interpretation: "MONOCHROME1",
        samples_per_pixel: 1,
        planar_configuration: None,
        bits_allocated: 8,
        bits_stored: 8,
        high_bit: 7,
        pixel_representation: 0,
        pixel_vr: VR::OB,
        pixel_bytes: &MONO_PIXELS,
        pixel_values: &[0, 85, 170, 255],
        pixel_min: 0,
        pixel_max: 255,
        visual_pattern: "2x2_inverse_monochrome_gradient",
        semantic_note: "minimum sample value displays as white",
        palette: None,
        padding: None,
    },
    PixelRecipe {
        case_id: "classic/sc/rgb_planar0_explicit_le",
        recipe_id: "sc_rgb_planar0",
        rows: 2,
        columns: 2,
        photometric_interpretation: "RGB",
        samples_per_pixel: 3,
        planar_configuration: Some(0),
        bits_allocated: 8,
        bits_stored: 8,
        high_bit: 7,
        pixel_representation: 0,
        pixel_vr: VR::OB,
        pixel_bytes: &RGB_PLANAR0_PIXELS,
        pixel_values: &[255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255],
        pixel_min: 0,
        pixel_max: 255,
        visual_pattern: "2x2_rgb_red_green_blue_white",
        semantic_note: "RGB samples are interleaved color-by-pixel",
        palette: None,
        padding: None,
    },
    PixelRecipe {
        case_id: "classic/sc/rgb_planar1_explicit_le",
        recipe_id: "sc_rgb_planar1",
        rows: 2,
        columns: 2,
        photometric_interpretation: "RGB",
        samples_per_pixel: 3,
        planar_configuration: Some(1),
        bits_allocated: 8,
        bits_stored: 8,
        high_bit: 7,
        pixel_representation: 0,
        pixel_vr: VR::OB,
        pixel_bytes: &RGB_PLANAR1_PIXELS,
        pixel_values: &[255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255],
        pixel_min: 0,
        pixel_max: 255,
        visual_pattern: "2x2_rgb_planar1_red_green_blue_white",
        semantic_note: "RGB samples are stored contiguously by color plane",
        palette: None,
        padding: None,
    },
    PixelRecipe {
        case_id: "classic/sc/palette_color_u8_explicit_le",
        recipe_id: "sc_palette_color_u8",
        rows: 2,
        columns: 2,
        photometric_interpretation: "PALETTE COLOR",
        samples_per_pixel: 1,
        planar_configuration: None,
        bits_allocated: 8,
        bits_stored: 8,
        high_bit: 7,
        pixel_representation: 0,
        pixel_vr: VR::OB,
        pixel_bytes: &PALETTE_COLOR_PIXELS,
        pixel_values: &PALETTE_COLOR_VALUES,
        pixel_min: 0,
        pixel_max: 3,
        visual_pattern: "2x2_palette_red_green_blue_white",
        semantic_note: "stored pixel values index 16-bit RGB palette lookup tables",
        palette: Some(PALETTE_COLOR_LUT),
        padding: None,
    },
    PixelRecipe {
        case_id: "classic/sc/ybr_full_planar0_explicit_le",
        recipe_id: "sc_ybr_full_planar0",
        rows: 2,
        columns: 2,
        photometric_interpretation: "YBR_FULL",
        samples_per_pixel: 3,
        planar_configuration: Some(0),
        bits_allocated: 8,
        bits_stored: 8,
        high_bit: 7,
        pixel_representation: 0,
        pixel_vr: VR::OB,
        pixel_bytes: &YBR_FULL_PLANAR0_PIXELS,
        pixel_values: &[76, 85, 255, 150, 44, 21, 29, 255, 107, 255, 128, 128],
        pixel_min: 21,
        pixel_max: 255,
        visual_pattern: "2x2_ybr_full_red_green_blue_white",
        semantic_note: "YBR_FULL samples are interleaved color-by-pixel",
        palette: None,
        padding: None,
    },
    PixelRecipe {
        case_id: "classic/sc/ybr_full_422_explicit_le",
        recipe_id: "sc_ybr_full_422",
        rows: 2,
        columns: 2,
        photometric_interpretation: "YBR_FULL_422",
        samples_per_pixel: 3,
        planar_configuration: Some(0),
        bits_allocated: 8,
        bits_stored: 8,
        high_bit: 7,
        pixel_representation: 0,
        pixel_vr: VR::OB,
        pixel_bytes: &YBR_FULL_422_PIXELS,
        pixel_values: &[76, 150, 65, 138, 29, 255, 192, 118],
        pixel_min: 29,
        pixel_max: 255,
        visual_pattern: "2x2_ybr_full_422_red_green_blue_white",
        semantic_note: "YBR_FULL_422 stores horizontally downsampled chroma with Planar Configuration 0",
        palette: None,
        padding: None,
    },
    PixelRecipe {
        case_id: "classic/sc/mono2_u16_explicit_le",
        recipe_id: "sc_mono2_u16",
        rows: 2,
        columns: 2,
        photometric_interpretation: "MONOCHROME2",
        samples_per_pixel: 1,
        planar_configuration: None,
        bits_allocated: 16,
        bits_stored: 16,
        high_bit: 15,
        pixel_representation: 0,
        pixel_vr: VR::OW,
        pixel_bytes: &MONO_U16_PIXELS,
        pixel_values: &MONO_U16_VALUES,
        pixel_min: 0,
        pixel_max: 65535,
        visual_pattern: "2x2_monochrome_u16_gradient",
        semantic_note: "16-bit unsigned MONOCHROME2 samples span the full stored range",
        palette: None,
        padding: None,
    },
    PixelRecipe {
        case_id: "classic/sc/mono2_i16_explicit_le",
        recipe_id: "sc_mono2_i16",
        rows: 2,
        columns: 2,
        photometric_interpretation: "MONOCHROME2",
        samples_per_pixel: 1,
        planar_configuration: None,
        bits_allocated: 16,
        bits_stored: 16,
        high_bit: 15,
        pixel_representation: 1,
        pixel_vr: VR::OW,
        pixel_bytes: &MONO_I16_PIXELS,
        pixel_values: &MONO_I16_VALUES,
        pixel_min: -32768,
        pixel_max: 32767,
        visual_pattern: "2x2_monochrome_i16_gradient",
        semantic_note: "16-bit signed MONOCHROME2 samples use 2's complement representation",
        palette: None,
        padding: None,
    },
    PixelRecipe {
        case_id: "classic/sc/mono2_u16_odd_3x3_explicit_le",
        recipe_id: "sc_mono2_u16_odd_3x3",
        rows: 3,
        columns: 3,
        photometric_interpretation: "MONOCHROME2",
        samples_per_pixel: 1,
        planar_configuration: None,
        bits_allocated: 16,
        bits_stored: 16,
        high_bit: 15,
        pixel_representation: 0,
        pixel_vr: VR::OW,
        pixel_bytes: &MONO_U16_ODD_3X3_PIXELS,
        pixel_values: &MONO_U16_ODD_3X3_VALUES,
        pixel_min: 0,
        pixel_max: 8,
        visual_pattern: "3x3_monochrome_u16_odd_gradient",
        semantic_note: "odd rows and columns use unsigned 16-bit MONOCHROME2 samples",
        palette: None,
        padding: None,
    },
    PixelRecipe {
        case_id: "classic/sc/mono2_u16_rect_2x3_explicit_le",
        recipe_id: "sc_mono2_u16_rect_2x3",
        rows: 2,
        columns: 3,
        photometric_interpretation: "MONOCHROME2",
        samples_per_pixel: 1,
        planar_configuration: None,
        bits_allocated: 16,
        bits_stored: 16,
        high_bit: 15,
        pixel_representation: 0,
        pixel_vr: VR::OW,
        pixel_bytes: &MONO_U16_RECT_2X3_PIXELS,
        pixel_values: &MONO_U16_RECT_2X3_VALUES,
        pixel_min: 0,
        pixel_max: 5,
        visual_pattern: "2x3_monochrome_u16_rect_gradient",
        semantic_note: "rectangular native Pixel Data uses Rows and Columns from the recipe",
        palette: None,
        padding: None,
    },
    PixelRecipe {
        case_id: "classic/sc/mono2_u16_tiny_1x1_explicit_le",
        recipe_id: "sc_mono2_u16_tiny_1x1",
        rows: 1,
        columns: 1,
        photometric_interpretation: "MONOCHROME2",
        samples_per_pixel: 1,
        planar_configuration: None,
        bits_allocated: 16,
        bits_stored: 16,
        high_bit: 15,
        pixel_representation: 0,
        pixel_vr: VR::OW,
        pixel_bytes: &MONO_U16_TINY_1X1_PIXELS,
        pixel_values: &MONO_U16_TINY_1X1_VALUES,
        pixel_min: 65535,
        pixel_max: 65535,
        visual_pattern: "1x1_monochrome_u16_tiny_maximum",
        semantic_note: "very small native Pixel Data uses a single unsigned 16-bit sample",
        palette: None,
        padding: None,
    },
    PixelRecipe {
        case_id: "classic/sc/mono2_u16_padding_explicit_le",
        recipe_id: "sc_mono2_u16_padding",
        rows: 2,
        columns: 2,
        photometric_interpretation: "MONOCHROME2",
        samples_per_pixel: 1,
        planar_configuration: None,
        bits_allocated: 16,
        bits_stored: 16,
        high_bit: 15,
        pixel_representation: 0,
        pixel_vr: VR::OW,
        pixel_bytes: &MONO_U16_PADDING_PIXELS,
        pixel_values: &MONO_U16_PADDING_VALUES,
        pixel_min: 0,
        pixel_max: 3000,
        visual_pattern: "2x2_monochrome_u16_with_padding_value",
        semantic_note: "Pixel Padding Value 0 identifies a padded unsigned MONOCHROME2 sample",
        palette: None,
        padding: Some(PixelPaddingRecipe {
            value: 0,
            range_limit: Some(0),
        }),
    },
];

#[derive(Debug, Clone, Copy)]
struct PixelRecipe {
    case_id: &'static str,
    recipe_id: &'static str,
    rows: u16,
    columns: u16,
    photometric_interpretation: &'static str,
    samples_per_pixel: u16,
    planar_configuration: Option<u16>,
    bits_allocated: u16,
    bits_stored: u16,
    high_bit: u16,
    pixel_representation: u16,
    pixel_vr: VR,
    pixel_bytes: &'static [u8],
    pixel_values: &'static [i32],
    pixel_min: i32,
    pixel_max: i32,
    visual_pattern: &'static str,
    semantic_note: &'static str,
    palette: Option<PaletteRecipe>,
    padding: Option<PixelPaddingRecipe>,
}

#[derive(Debug, Clone, Copy)]
struct PaletteRecipe {
    descriptor: [u16; 3],
    red_data: &'static [u8],
    green_data: &'static [u8],
    blue_data: &'static [u8],
}

#[derive(Debug, Clone, Copy)]
struct PixelPaddingRecipe {
    value: u16,
    range_limit: Option<u16>,
}

#[derive(Debug, Clone, Copy)]
struct ClassicCtRecipe {
    case_id: &'static str,
    recipe_id: &'static str,
    rows: u16,
    columns: u16,
    pixel_bytes: &'static [u8],
    pixel_values: &'static [i32],
    pixel_min: i32,
    pixel_max: i32,
    rescale_intercept: &'static str,
    rescale_slope: &'static str,
    rescale_type: &'static str,
    window_center: &'static str,
    window_width: &'static str,
    pixel_spacing: &'static str,
    image_orientation_patient: &'static str,
    image_position_patient: &'static str,
    slice_thickness: &'static str,
    kvp: &'static str,
}

const CLASSIC_CT_RECIPES: &[ClassicCtRecipe] = &[ClassicCtRecipe {
    case_id: "classic/ct/mono2_i16_rescale_12bit_explicit_le",
    recipe_id: "ct_mono2_i16_rescale",
    rows: 2,
    columns: 2,
    pixel_bytes: &CT_I16_12BIT_PIXELS,
    pixel_values: &CT_I16_12BIT_VALUES,
    pixel_min: -1024,
    pixel_max: 2047,
    rescale_intercept: "-1024",
    rescale_slope: "1",
    rescale_type: "HU",
    window_center: "40",
    window_width: "400",
    pixel_spacing: "0.625\\0.625",
    image_orientation_patient: "1\\0\\0\\0\\1\\0",
    image_position_patient: "-0.625\\-0.625\\0",
    slice_thickness: "1",
    kvp: "120",
}];

#[derive(Debug, Clone, Copy)]
struct ClassicMgRecipe {
    case_id: &'static str,
    recipe_id: &'static str,
    sop_class_uid: &'static str,
    sop_class_name: &'static str,
    transfer_syntax_uid: &'static str,
    transfer_syntax_name: &'static str,
    presentation_intent_type: &'static str,
    photometric_interpretation: &'static str,
    presentation_lut_shape: &'static str,
    rows: u16,
    columns: u16,
    pixel_bytes: &'static [u8],
    pixel_values: &'static [i32],
    pixel_min: i32,
    pixel_max: i32,
    imager_pixel_spacing: &'static str,
    window_center: Option<&'static str>,
    window_width: Option<&'static str>,
}

const CLASSIC_MG_RECIPES: &[ClassicMgRecipe] = &[
    ClassicMgRecipe {
        case_id: "classic/mg/for_presentation_mono1_u16_12bit_explicit_le",
        recipe_id: "mg_for_presentation_mono1_u16",
        sop_class_uid: uids::DIGITAL_MAMMOGRAPHY_X_RAY_IMAGE_STORAGE_FOR_PRESENTATION,
        sop_class_name: "Digital Mammography X-Ray Image Storage - For Presentation",
        transfer_syntax_uid: uids::EXPLICIT_VR_LITTLE_ENDIAN,
        transfer_syntax_name: "Explicit VR Little Endian",
        presentation_intent_type: "FOR PRESENTATION",
        photometric_interpretation: "MONOCHROME1",
        presentation_lut_shape: "INVERSE",
        rows: 2,
        columns: 2,
        pixel_bytes: &MG_U16_12BIT_PIXELS,
        pixel_values: &MG_U16_12BIT_VALUES,
        pixel_min: 0,
        pixel_max: 4095,
        imager_pixel_spacing: "0.070\\0.070",
        window_center: Some("2048"),
        window_width: Some("4096"),
    },
    ClassicMgRecipe {
        case_id: "classic/mg/for_processing_mono2_u16_12bit_implicit_le",
        recipe_id: "mg_for_processing_mono2_u16",
        sop_class_uid: uids::DIGITAL_MAMMOGRAPHY_X_RAY_IMAGE_STORAGE_FOR_PROCESSING,
        sop_class_name: "Digital Mammography X-Ray Image Storage - For Processing",
        transfer_syntax_uid: uids::IMPLICIT_VR_LITTLE_ENDIAN,
        transfer_syntax_name: "Implicit VR Little Endian",
        presentation_intent_type: "FOR PROCESSING",
        photometric_interpretation: "MONOCHROME2",
        presentation_lut_shape: "IDENTITY",
        rows: 2,
        columns: 2,
        pixel_bytes: &MG_U16_12BIT_PIXELS,
        pixel_values: &MG_U16_12BIT_VALUES,
        pixel_min: 0,
        pixel_max: 4095,
        imager_pixel_spacing: "0.070\\0.070",
        window_center: None,
        window_width: None,
    },
];

#[derive(Debug, Clone, Copy)]
struct ClassicDxRecipe {
    case_id: &'static str,
    recipe_id: &'static str,
    rows: u16,
    columns: u16,
    pixel_bytes: &'static [u8],
    pixel_values: &'static [i32],
    pixel_min: i32,
    pixel_max: i32,
    imager_pixel_spacing: &'static str,
    window_center: &'static str,
    window_width: &'static str,
    shutter_left_vertical_edge: &'static str,
    shutter_right_vertical_edge: &'static str,
    shutter_upper_horizontal_edge: &'static str,
    shutter_lower_horizontal_edge: &'static str,
    shutter_presentation_value: u16,
}

const CLASSIC_DX_RECIPES: &[ClassicDxRecipe] = &[ClassicDxRecipe {
    case_id: "classic/dx/display_shutter_mono2_u16_explicit_le",
    recipe_id: "dx_display_shutter_mono2_u16",
    rows: 2,
    columns: 2,
    pixel_bytes: &DX_U16_12BIT_PIXELS,
    pixel_values: &DX_U16_12BIT_VALUES,
    pixel_min: 0,
    pixel_max: 4095,
    imager_pixel_spacing: "0.150\\0.150",
    window_center: "2048",
    window_width: "4096",
    shutter_left_vertical_edge: "1",
    shutter_right_vertical_edge: "2",
    shutter_upper_horizontal_edge: "1",
    shutter_lower_horizontal_edge: "2",
    shutter_presentation_value: 0,
}];

#[derive(Debug, Clone, Copy)]
struct ClassicCrRecipe {
    case_id: &'static str,
    recipe_id: &'static str,
    rows: u16,
    columns: u16,
    pixel_bytes: &'static [u8],
    pixel_values: &'static [i32],
    pixel_min: i32,
    pixel_max: i32,
    overlay_bytes: &'static [u8],
    modality_lut_descriptor: [u16; 3],
    modality_lut_data: &'static [u8],
    modality_lut_type: &'static str,
    voi_lut_descriptor: [u16; 3],
    voi_lut_data: &'static [u8],
    body_part_examined: &'static str,
    view_position: &'static str,
}

const CLASSIC_CR_RECIPES: &[ClassicCrRecipe] = &[ClassicCrRecipe {
    case_id: "classic/cr/overlay_modality_voi_explicit_le",
    recipe_id: "cr_overlay_modality_voi",
    rows: 2,
    columns: 2,
    pixel_bytes: &CR_U8_PIXELS,
    pixel_values: &CR_U8_VALUES,
    pixel_min: 0,
    pixel_max: 3,
    overlay_bytes: &CR_OVERLAY_PIXELS,
    modality_lut_descriptor: [4, 0, 16],
    modality_lut_data: &CR_MODALITY_LUT_DATA,
    modality_lut_type: "US",
    voi_lut_descriptor: [4, 0, 16],
    voi_lut_data: &CR_VOI_LUT_DATA,
    body_part_examined: "CHEST",
    view_position: "PA",
}];

#[derive(Debug, Clone, Copy)]
struct ClassicMrRecipe {
    case_id: &'static str,
    recipe_id: &'static str,
    rows: u16,
    columns: u16,
    pixel_spacing: &'static str,
    image_orientation_patient: &'static str,
    slice_thickness: &'static str,
    spacing_between_slices: &'static str,
    slices: &'static [ClassicMrSliceRecipe],
}

#[derive(Debug, Clone, Copy)]
struct ClassicMrSliceRecipe {
    instance_number: &'static str,
    image_position_patient: &'static str,
    slice_location: &'static str,
    position_along_normal: f64,
    pixel_bytes: &'static [u8],
    pixel_values: &'static [i32],
    pixel_min: i32,
    pixel_max: i32,
}

const CLASSIC_MR_SLICES: &[ClassicMrSliceRecipe] = &[
    ClassicMrSliceRecipe {
        instance_number: "1",
        image_position_patient: "0\\0\\0",
        slice_location: "0",
        position_along_normal: 0.0,
        pixel_bytes: &MR_SLICE_1_PIXELS,
        pixel_values: &MR_SLICE_1_VALUES,
        pixel_min: 0,
        pixel_max: 3,
    },
    ClassicMrSliceRecipe {
        instance_number: "2",
        image_position_patient: "3.535534\\-3.535534\\0",
        slice_location: "5",
        position_along_normal: 5.0,
        pixel_bytes: &MR_SLICE_2_PIXELS,
        pixel_values: &MR_SLICE_2_VALUES,
        pixel_min: 10,
        pixel_max: 13,
    },
    ClassicMrSliceRecipe {
        instance_number: "3",
        image_position_patient: "7.071068\\-7.071068\\0",
        slice_location: "10",
        position_along_normal: 10.0,
        pixel_bytes: &MR_SLICE_3_PIXELS,
        pixel_values: &MR_SLICE_3_VALUES,
        pixel_min: 20,
        pixel_max: 23,
    },
];

const CLASSIC_MR_RECIPES: &[ClassicMrRecipe] = &[ClassicMrRecipe {
    case_id: "classic/mr/multislice_oblique_explicit_le",
    recipe_id: "mr_multislice_oblique",
    rows: 2,
    columns: 2,
    pixel_spacing: "1.000\\1.000",
    image_orientation_patient: "0.70710678\\0.70710678\\0\\0\\0\\1",
    slice_thickness: "5",
    spacing_between_slices: "5",
    slices: CLASSIC_MR_SLICES,
}];

#[derive(Debug, Clone)]
pub(crate) struct GeneratedFile {
    pub case_id: String,
    pub manifest_entry: Value,
}

pub(crate) fn write_supported_cases(
    run: &PreparedGenerationRun,
    registry: &Value,
    standards_lock_sha256: &str,
) -> Result<Vec<GeneratedFile>, GenerateError> {
    let mut generated_files = Vec::new();
    for recipe in PIXEL_RECIPES {
        let Some(case) = registry_case(registry, recipe.case_id)? else {
            continue;
        };
        let profiles = string_array(case.get("profiles"))?;
        if !case_matches_profile(&profiles, &run.profile, run.include_stress) {
            continue;
        }
        generated_files.push(write_pixel_case(run, case, *recipe, standards_lock_sha256)?);
    }
    for recipe in CLASSIC_CT_RECIPES {
        let Some(case) = registry_case(registry, recipe.case_id)? else {
            continue;
        };
        let profiles = string_array(case.get("profiles"))?;
        if !case_matches_profile(&profiles, &run.profile, run.include_stress) {
            continue;
        }
        generated_files.push(write_classic_ct_case(
            run,
            case,
            *recipe,
            standards_lock_sha256,
        )?);
    }
    for recipe in CLASSIC_MG_RECIPES {
        let Some(case) = registry_case(registry, recipe.case_id)? else {
            continue;
        };
        let profiles = string_array(case.get("profiles"))?;
        if !case_matches_profile(&profiles, &run.profile, run.include_stress) {
            continue;
        }
        generated_files.push(write_classic_mg_case(
            run,
            case,
            *recipe,
            standards_lock_sha256,
        )?);
    }
    for recipe in CLASSIC_DX_RECIPES {
        let Some(case) = registry_case(registry, recipe.case_id)? else {
            continue;
        };
        let profiles = string_array(case.get("profiles"))?;
        if !case_matches_profile(&profiles, &run.profile, run.include_stress) {
            continue;
        }
        generated_files.push(write_classic_dx_case(
            run,
            case,
            *recipe,
            standards_lock_sha256,
        )?);
    }
    for recipe in CLASSIC_CR_RECIPES {
        let Some(case) = registry_case(registry, recipe.case_id)? else {
            continue;
        };
        let profiles = string_array(case.get("profiles"))?;
        if !case_matches_profile(&profiles, &run.profile, run.include_stress) {
            continue;
        }
        generated_files.push(write_classic_cr_case(
            run,
            case,
            *recipe,
            standards_lock_sha256,
        )?);
    }
    for recipe in CLASSIC_MR_RECIPES {
        let Some(case) = registry_case(registry, recipe.case_id)? else {
            continue;
        };
        let profiles = string_array(case.get("profiles"))?;
        if !case_matches_profile(&profiles, &run.profile, run.include_stress) {
            continue;
        }
        generated_files.extend(write_classic_mr_case(
            run,
            case,
            *recipe,
            standards_lock_sha256,
        )?);
    }
    Ok(generated_files)
}

fn write_pixel_case(
    run: &PreparedGenerationRun,
    case: &Value,
    recipe: PixelRecipe,
    standards_lock_sha256: &str,
) -> Result<GeneratedFile, GenerateError> {
    let study_instance_uid = deterministic_case_uid(
        standards_lock_sha256,
        recipe,
        run.seed,
        UidRole::StudyInstance,
    );
    let series_instance_uid = deterministic_case_uid(
        standards_lock_sha256,
        recipe,
        run.seed,
        UidRole::SeriesInstance,
    );
    let sop_instance_uid = deterministic_case_uid(
        standards_lock_sha256,
        recipe,
        run.seed,
        UidRole::SopInstance,
    );
    let implementation_class_uid = deterministic_implementation_uid(standards_lock_sha256);

    let relative_path = format!("{}/instance.dcm", recipe.case_id);
    let path = run.out_dir.join(&relative_path);
    let case_dir = path.parent().ok_or_else(|| GenerateError::MetadataShape {
        path: PathBuf::from(&relative_path),
        message: "generated DICOM path must have a parent directory",
    })?;
    fs::create_dir_all(case_dir).map_err(|source| GenerateError::CreateCaseOutputDir {
        path: case_dir.to_path_buf(),
        source,
    })?;

    let mut obj = InMemDicomObject::new_empty();
    put_str(
        &mut obj,
        tags::SOP_CLASS_UID,
        VR::UI,
        uids::SECONDARY_CAPTURE_IMAGE_STORAGE,
    );
    put_str(&mut obj, tags::SOP_INSTANCE_UID, VR::UI, &sop_instance_uid);
    put_str(&mut obj, tags::SYNTHETIC_DATA, VR::CS, "YES");

    put_str(&mut obj, tags::PATIENT_NAME, VR::PN, "DICOMTEST^SMOKE");
    put_str(&mut obj, tags::PATIENT_ID, VR::LO, "DICOMTEST-SMOKE-001");
    put_str(&mut obj, tags::PATIENT_BIRTH_DATE, VR::DA, "19700101");
    put_str(&mut obj, tags::PATIENT_SEX, VR::CS, "O");

    put_str(
        &mut obj,
        tags::STUDY_INSTANCE_UID,
        VR::UI,
        &study_instance_uid,
    );
    put_str(&mut obj, tags::STUDY_DATE, VR::DA, "20260101");
    put_str(&mut obj, tags::STUDY_TIME, VR::TM, "000000");
    put_str(&mut obj, tags::REFERRING_PHYSICIAN_NAME, VR::PN, "");
    put_str(&mut obj, tags::STUDY_ID, VR::SH, "SMOKE");
    put_str(&mut obj, tags::ACCESSION_NUMBER, VR::SH, "");

    put_str(&mut obj, tags::MODALITY, VR::CS, "OT");
    put_str(
        &mut obj,
        tags::SERIES_INSTANCE_UID,
        VR::UI,
        &series_instance_uid,
    );
    put_str(&mut obj, tags::SERIES_NUMBER, VR::IS, "1");

    put_str(&mut obj, tags::CONVERSION_TYPE, VR::CS, "SYN");
    put_str(&mut obj, tags::MANUFACTURER, VR::LO, "dicom-test-suite");
    put_str(
        &mut obj,
        tags::MANUFACTURER_MODEL_NAME,
        VR::LO,
        recipe.recipe_id,
    );
    put_str(
        &mut obj,
        tags::SOFTWARE_VERSIONS,
        VR::LO,
        crate::PACKAGE_VERSION,
    );

    put_str(&mut obj, tags::INSTANCE_NUMBER, VR::IS, "1");
    put_str(&mut obj, tags::PATIENT_ORIENTATION, VR::CS, "");
    put_str(&mut obj, tags::CONTENT_DATE, VR::DA, "20260101");
    put_str(&mut obj, tags::CONTENT_TIME, VR::TM, "000000");

    put_u16(
        &mut obj,
        tags::SAMPLES_PER_PIXEL,
        VR::US,
        recipe.samples_per_pixel,
    );
    put_str(
        &mut obj,
        tags::PHOTOMETRIC_INTERPRETATION,
        VR::CS,
        recipe.photometric_interpretation,
    );
    if let Some(planar_configuration) = recipe.planar_configuration {
        put_u16(
            &mut obj,
            tags::PLANAR_CONFIGURATION,
            VR::US,
            planar_configuration,
        );
    }
    put_u16(&mut obj, tags::ROWS, VR::US, recipe.rows);
    put_u16(&mut obj, tags::COLUMNS, VR::US, recipe.columns);
    put_u16(
        &mut obj,
        tags::BITS_ALLOCATED,
        VR::US,
        recipe.bits_allocated,
    );
    put_u16(&mut obj, tags::BITS_STORED, VR::US, recipe.bits_stored);
    put_u16(&mut obj, tags::HIGH_BIT, VR::US, recipe.high_bit);
    put_u16(
        &mut obj,
        tags::PIXEL_REPRESENTATION,
        VR::US,
        recipe.pixel_representation,
    );
    if let Some(palette) = recipe.palette {
        put_palette(&mut obj, palette);
    }
    if let Some(padding) = recipe.padding {
        put_u16(&mut obj, tags::PIXEL_PADDING_VALUE, VR::US, padding.value);
        if let Some(range_limit) = padding.range_limit {
            put_u16(
                &mut obj,
                tags::PIXEL_PADDING_RANGE_LIMIT,
                VR::US,
                range_limit,
            );
        }
    }
    obj.put(DataElement::new(
        tags::PIXEL_DATA,
        recipe.pixel_vr,
        PrimitiveValue::from(recipe.pixel_bytes),
    ));

    let file_obj = obj
        .with_meta(
            FileMetaTableBuilder::new()
                .transfer_syntax(uids::EXPLICIT_VR_LITTLE_ENDIAN)
                .implementation_class_uid(&implementation_class_uid)
                .implementation_version_name("DICOMTS010"),
        )
        .map_err(|err| GenerateError::WriteDicomFile {
            path: path.clone(),
            message: err.to_string(),
        })?;

    file_obj
        .write_to_file(&path)
        .map_err(|err| GenerateError::WriteDicomFile {
            path: path.clone(),
            message: err.to_string(),
        })?;

    let validated = validate_part10_file(
        &path,
        &Part10Expectations {
            sop_class_uid: uids::SECONDARY_CAPTURE_IMAGE_STORAGE,
            sop_instance_uid: &sop_instance_uid,
            transfer_syntax_uid: uids::EXPLICIT_VR_LITTLE_ENDIAN,
            implementation_class_uid: &implementation_class_uid,
            synthetic_data: "YES",
            rows: recipe.rows,
            columns: recipe.columns,
            samples_per_pixel: recipe.samples_per_pixel,
            photometric_interpretation: recipe.photometric_interpretation,
            bits_allocated: recipe.bits_allocated,
            bits_stored: recipe.bits_stored,
            high_bit: recipe.high_bit,
            pixel_representation: recipe.pixel_representation,
            planar_configuration: recipe.planar_configuration,
            pixel_data_vr: recipe.pixel_vr,
            pixel_data_length_formula: pixel_data_length_formula(recipe),
            palette: recipe.palette.map(|palette| palette.into()),
            padding: recipe.padding.map(|padding| padding.into()),
            ct_image: None,
            mg_image: None,
            dx_image: None,
            cr_image: None,
            mr_image: None,
        },
    )?;

    Ok(GeneratedFile {
        case_id: recipe.case_id.to_string(),
        manifest_entry: pixel_manifest_entry(
            case,
            recipe,
            &relative_path,
            &study_instance_uid,
            &series_instance_uid,
            &sop_instance_uid,
            &implementation_class_uid,
            &validated.bytes,
            validated.validation,
        ),
    })
}

fn pixel_manifest_entry(
    case: &Value,
    recipe: PixelRecipe,
    relative_path: &str,
    study_instance_uid: &str,
    series_instance_uid: &str,
    sop_instance_uid: &str,
    implementation_class_uid: &str,
    bytes: &[u8],
    validation: Value,
) -> Value {
    let mut standards_evidence = case
        .get("standards_evidence")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    standards_evidence.extend([
        serde_json::json!({
            "source": "dicom-standard-kb",
            "edition": "2026b",
            "query": "lookup_sop_class SecondaryCaptureImageStorage",
            "covered": true,
            "part": "PS3.3",
            "anchor": "table_A.8-1"
        }),
        serde_json::json!({
            "source": "dicom-standard-kb",
            "edition": "2026b",
            "query": "lookup_data_element SyntheticData",
            "covered": true,
            "part": "PS3.6",
            "anchor": "table_6-1"
        }),
        serde_json::json!({
            "source": "dicom-standard-kb",
            "edition": "2026b",
            "query": "search_standard_text Image Pixel Description Macro",
            "covered": true,
            "part": "PS3.3",
            "anchor": "table_C.7-11c"
        }),
        serde_json::json!({
            "source": "dicom-standard-kb",
            "edition": "2026b",
            "query": "retrieve_standard_text sect_C.7.6.3.1.2",
            "covered": true,
            "part": "PS3.3",
            "anchor": "sect_C.7.6.3.1.2"
        }),
    ]);
    if recipe.planar_configuration.is_some() {
        standards_evidence.extend([
            serde_json::json!({
                "source": "dicom-standard-kb",
                "edition": "2026b",
                "query": "lookup_data_element PlanarConfiguration",
                "covered": true,
                "part": "PS3.6",
                "anchor": "table_6-1"
            }),
            serde_json::json!({
                "source": "dicom-standard-kb",
                "edition": "2026b",
                "query": "retrieve_standard_text sect_C.7.6.3.1.3",
                "covered": true,
                "part": "PS3.3",
                "anchor": "sect_C.7.6.3.1.3"
            }),
        ]);
    }
    if recipe.palette.is_some() {
        standards_evidence.extend([
            serde_json::json!({
                "source": "dicom-standard-kb",
                "edition": "2026b",
                "query": "retrieve_standard_text sect_C.7.6.3.1.5",
                "covered": true,
                "part": "PS3.3",
                "anchor": "sect_C.7.6.3.1.5"
            }),
            serde_json::json!({
                "source": "dicom-standard-kb",
                "edition": "2026b",
                "query": "lookup_data_element RedPaletteColorLookupTableDescriptor",
                "covered": true,
                "part": "PS3.6",
                "anchor": "table_6-1"
            }),
            serde_json::json!({
                "source": "dicom-standard-kb",
                "edition": "2026b",
                "query": "lookup_data_element GreenPaletteColorLookupTableDescriptor",
                "covered": true,
                "part": "PS3.6",
                "anchor": "table_6-1"
            }),
            serde_json::json!({
                "source": "dicom-standard-kb",
                "edition": "2026b",
                "query": "lookup_data_element BluePaletteColorLookupTableDescriptor",
                "covered": true,
                "part": "PS3.6",
                "anchor": "table_6-1"
            }),
            serde_json::json!({
                "source": "dicom-standard-kb",
                "edition": "2026b",
                "query": "lookup_data_element RedPaletteColorLookupTableData",
                "covered": true,
                "part": "PS3.6",
                "anchor": "table_6-1"
            }),
            serde_json::json!({
                "source": "dicom-standard-kb",
                "edition": "2026b",
                "query": "lookup_data_element GreenPaletteColorLookupTableData",
                "covered": true,
                "part": "PS3.6",
                "anchor": "table_6-1"
            }),
            serde_json::json!({
                "source": "dicom-standard-kb",
                "edition": "2026b",
                "query": "lookup_data_element BluePaletteColorLookupTableData",
                "covered": true,
                "part": "PS3.6",
                "anchor": "table_6-1"
            }),
        ]);
    }
    if recipe.padding.is_some() {
        standards_evidence.extend([
            serde_json::json!({
                "source": "dicom-standard-kb",
                "edition": "2026b",
                "query": "lookup_data_element PixelPaddingValue",
                "covered": true,
                "part": "PS3.6",
                "anchor": "table_6-1"
            }),
            serde_json::json!({
                "source": "dicom-standard-kb",
                "edition": "2026b",
                "query": "lookup_data_element PixelPaddingRangeLimit",
                "covered": true,
                "part": "PS3.6",
                "anchor": "table_6-1"
            }),
            serde_json::json!({
                "source": "dicom-standard-kb",
                "edition": "2026b",
                "query": "retrieve_standard_text sect_C.7.5.1.1.2",
                "covered": true,
                "part": "PS3.3",
                "anchor": "sect_C.7.5.1.1.2"
            }),
        ]);
    }

    let palette_manifest = recipe.palette.map(|palette| {
        serde_json::json!({
            "descriptor": palette.descriptor,
            "red_data_value_length": palette.red_data.len(),
            "green_data_value_length": palette.green_data.len(),
            "blue_data_value_length": palette.blue_data.len()
        })
    });
    let padding_manifest = recipe.padding.map(|padding| {
        serde_json::json!({
            "value": padding.value,
            "range_limit": padding.range_limit
        })
    });

    serde_json::json!({
        "case_id": recipe.case_id,
        "profile_membership": ["smoke"],
        "path": relative_path,
        "sha256": sha256_hex(bytes),
        "size_bytes": bytes.len(),
        "determinism": "byte_stable",
        "recipe": {
            "recipe_id": recipe.recipe_id,
            "recipe_version": PIXEL_RECIPE_VERSION,
            "recipe_parameters": {
                "rows": recipe.rows,
                "columns": recipe.columns,
                "samples_per_pixel": recipe.samples_per_pixel,
                "photometric_interpretation": recipe.photometric_interpretation,
                "bits_allocated": recipe.bits_allocated,
                "bits_stored": recipe.bits_stored,
                "planar_configuration": recipe.planar_configuration,
                "pixel_values": recipe.pixel_values,
                "palette": palette_manifest,
                "pixel_padding": padding_manifest
            }
        },
        "dicom": {
            "sop_class_uid": uids::SECONDARY_CAPTURE_IMAGE_STORAGE,
            "sop_class_name": "Secondary Capture Image Storage",
            "iod_name": "Secondary Capture Image",
            "modality": "OT",
            "transfer_syntax_uid": uids::EXPLICIT_VR_LITTLE_ENDIAN,
            "transfer_syntax_name": "Explicit VR Little Endian"
        },
        "uids": {
            "study_instance_uid": study_instance_uid,
            "series_instance_uid": series_instance_uid,
            "sop_instance_uid": sop_instance_uid,
            "frame_of_reference_uid": Value::Null,
            "implementation_class_uid": implementation_class_uid
        },
        "image": {
            "rows": recipe.rows,
            "columns": recipe.columns,
            "frames": 1,
            "samples_per_pixel": recipe.samples_per_pixel,
            "photometric_interpretation": recipe.photometric_interpretation,
            "bits_allocated": recipe.bits_allocated,
            "bits_stored": recipe.bits_stored,
            "high_bit": recipe.high_bit,
            "pixel_representation": recipe.pixel_representation,
            "planar_configuration": recipe.planar_configuration
        },
        "pixel_data": {
            "vr": pixel_vr_name(recipe.pixel_vr),
            "native_or_encapsulated": "native",
            "value_length": recipe.pixel_bytes.len(),
            "frame_count": 1,
            "frame_hashes": [sha256_hex(recipe.pixel_bytes)]
        },
        "expected_capabilities": ["open_file", "read_metadata", "render_native_pixels"],
        "expected_semantics": {
            "synthetic_data": "YES",
            "conversion_type": "SYN",
            "pixel_min": recipe.pixel_min,
            "pixel_max": recipe.pixel_max,
            "pixel_padding": padding_manifest,
            "photometric_semantics": recipe.semantic_note
        },
        "expected_visual_checks": {
            "pattern": recipe.visual_pattern
        },
        "validation": validation,
        "known_stressors": ["minimal_secondary_capture", "native_ob_pixel_data"],
        "standards_evidence": standards_evidence
    })
}

fn write_classic_ct_case(
    run: &PreparedGenerationRun,
    case: &Value,
    recipe: ClassicCtRecipe,
    standards_lock_sha256: &str,
) -> Result<GeneratedFile, GenerateError> {
    let study_instance_uid = deterministic_classic_ct_uid(
        standards_lock_sha256,
        recipe,
        run.seed,
        UidRole::StudyInstance,
    );
    let series_instance_uid = deterministic_classic_ct_uid(
        standards_lock_sha256,
        recipe,
        run.seed,
        UidRole::SeriesInstance,
    );
    let sop_instance_uid = deterministic_classic_ct_uid(
        standards_lock_sha256,
        recipe,
        run.seed,
        UidRole::SopInstance,
    );
    let frame_of_reference_uid = deterministic_classic_ct_uid(
        standards_lock_sha256,
        recipe,
        run.seed,
        UidRole::FrameOfReference,
    );
    let implementation_class_uid = deterministic_implementation_uid(standards_lock_sha256);

    let relative_path = format!("{}/instance.dcm", recipe.case_id);
    let path = run.out_dir.join(&relative_path);
    let case_dir = path.parent().ok_or_else(|| GenerateError::MetadataShape {
        path: PathBuf::from(&relative_path),
        message: "generated DICOM path must have a parent directory",
    })?;
    fs::create_dir_all(case_dir).map_err(|source| GenerateError::CreateCaseOutputDir {
        path: case_dir.to_path_buf(),
        source,
    })?;

    let mut obj = InMemDicomObject::new_empty();
    put_str(
        &mut obj,
        tags::SOP_CLASS_UID,
        VR::UI,
        uids::CT_IMAGE_STORAGE,
    );
    put_str(&mut obj, tags::SOP_INSTANCE_UID, VR::UI, &sop_instance_uid);
    put_str(&mut obj, tags::SYNTHETIC_DATA, VR::CS, "YES");

    put_str(
        &mut obj,
        tags::PATIENT_NAME,
        VR::PN,
        "DTS^Synthetic^Patient001",
    );
    put_str(&mut obj, tags::PATIENT_ID, VR::LO, "DTS-PATIENT-001");
    put_str(&mut obj, tags::PATIENT_BIRTH_DATE, VR::DA, "19700101");
    put_str(&mut obj, tags::PATIENT_SEX, VR::CS, "O");

    put_str(
        &mut obj,
        tags::STUDY_INSTANCE_UID,
        VR::UI,
        &study_instance_uid,
    );
    put_str(&mut obj, tags::STUDY_DATE, VR::DA, "20260101");
    put_str(&mut obj, tags::STUDY_TIME, VR::TM, "000000");
    put_str(&mut obj, tags::REFERRING_PHYSICIAN_NAME, VR::PN, "");
    put_str(&mut obj, tags::STUDY_ID, VR::SH, "DTS-CT");
    put_str(&mut obj, tags::ACCESSION_NUMBER, VR::SH, "");

    put_str(&mut obj, tags::MODALITY, VR::CS, "CT");
    put_str(
        &mut obj,
        tags::SERIES_INSTANCE_UID,
        VR::UI,
        &series_instance_uid,
    );
    put_str(&mut obj, tags::SERIES_NUMBER, VR::IS, "1");
    put_str(
        &mut obj,
        tags::FRAME_OF_REFERENCE_UID,
        VR::UI,
        &frame_of_reference_uid,
    );
    put_str(&mut obj, tags::POSITION_REFERENCE_INDICATOR, VR::LO, "");

    put_str(&mut obj, tags::MANUFACTURER, VR::LO, "dicom-test-suite");
    put_str(
        &mut obj,
        tags::MANUFACTURER_MODEL_NAME,
        VR::LO,
        recipe.recipe_id,
    );
    put_str(
        &mut obj,
        tags::SOFTWARE_VERSIONS,
        VR::LO,
        crate::PACKAGE_VERSION,
    );

    put_str(&mut obj, tags::ACQUISITION_NUMBER, VR::IS, "1");
    put_str(&mut obj, tags::ACQUISITION_DATE, VR::DA, "20260101");
    put_str(&mut obj, tags::ACQUISITION_TIME, VR::TM, "000000");

    put_str(
        &mut obj,
        tags::IMAGE_TYPE,
        VR::CS,
        "ORIGINAL\\PRIMARY\\AXIAL",
    );
    put_str(&mut obj, tags::INSTANCE_NUMBER, VR::IS, "1");
    put_str(&mut obj, tags::PATIENT_ORIENTATION, VR::CS, "");
    put_str(&mut obj, tags::CONTENT_DATE, VR::DA, "20260101");
    put_str(&mut obj, tags::CONTENT_TIME, VR::TM, "000000");

    put_str(&mut obj, tags::PIXEL_SPACING, VR::DS, recipe.pixel_spacing);
    put_str(
        &mut obj,
        tags::IMAGE_ORIENTATION_PATIENT,
        VR::DS,
        recipe.image_orientation_patient,
    );
    put_str(
        &mut obj,
        tags::IMAGE_POSITION_PATIENT,
        VR::DS,
        recipe.image_position_patient,
    );
    put_str(
        &mut obj,
        tags::SLICE_THICKNESS,
        VR::DS,
        recipe.slice_thickness,
    );

    put_u16(&mut obj, tags::SAMPLES_PER_PIXEL, VR::US, 1);
    put_str(
        &mut obj,
        tags::PHOTOMETRIC_INTERPRETATION,
        VR::CS,
        "MONOCHROME2",
    );
    put_u16(&mut obj, tags::ROWS, VR::US, recipe.rows);
    put_u16(&mut obj, tags::COLUMNS, VR::US, recipe.columns);
    put_u16(&mut obj, tags::BITS_ALLOCATED, VR::US, 16);
    put_u16(&mut obj, tags::BITS_STORED, VR::US, 12);
    put_u16(&mut obj, tags::HIGH_BIT, VR::US, 11);
    put_u16(&mut obj, tags::PIXEL_REPRESENTATION, VR::US, 1);

    put_str(&mut obj, tags::KVP, VR::DS, recipe.kvp);
    put_str(
        &mut obj,
        tags::RESCALE_INTERCEPT,
        VR::DS,
        recipe.rescale_intercept,
    );
    put_str(&mut obj, tags::RESCALE_SLOPE, VR::DS, recipe.rescale_slope);
    put_str(&mut obj, tags::RESCALE_TYPE, VR::LO, recipe.rescale_type);
    put_str(&mut obj, tags::WINDOW_CENTER, VR::DS, recipe.window_center);
    put_str(&mut obj, tags::WINDOW_WIDTH, VR::DS, recipe.window_width);

    obj.put(DataElement::new(
        tags::PIXEL_DATA,
        VR::OW,
        PrimitiveValue::from(recipe.pixel_bytes),
    ));

    let file_obj = obj
        .with_meta(
            FileMetaTableBuilder::new()
                .transfer_syntax(uids::EXPLICIT_VR_LITTLE_ENDIAN)
                .implementation_class_uid(&implementation_class_uid)
                .implementation_version_name("DICOMTS010"),
        )
        .map_err(|err| GenerateError::WriteDicomFile {
            path: path.clone(),
            message: err.to_string(),
        })?;

    file_obj
        .write_to_file(&path)
        .map_err(|err| GenerateError::WriteDicomFile {
            path: path.clone(),
            message: err.to_string(),
        })?;

    let validated = validate_part10_file(
        &path,
        &Part10Expectations {
            sop_class_uid: uids::CT_IMAGE_STORAGE,
            sop_instance_uid: &sop_instance_uid,
            transfer_syntax_uid: uids::EXPLICIT_VR_LITTLE_ENDIAN,
            implementation_class_uid: &implementation_class_uid,
            synthetic_data: "YES",
            rows: recipe.rows,
            columns: recipe.columns,
            samples_per_pixel: 1,
            photometric_interpretation: "MONOCHROME2",
            bits_allocated: 16,
            bits_stored: 12,
            high_bit: 11,
            pixel_representation: 1,
            planar_configuration: None,
            pixel_data_vr: VR::OW,
            pixel_data_length_formula: PixelDataLengthFormula::ContiguousSamples,
            palette: None,
            padding: None,
            ct_image: Some(CtImageExpectations {
                modality: "CT",
                frame_of_reference_uid: &frame_of_reference_uid,
                image_type: "ORIGINAL\\PRIMARY\\AXIAL",
                pixel_spacing: recipe.pixel_spacing,
                image_orientation_patient: recipe.image_orientation_patient,
                image_position_patient: recipe.image_position_patient,
                slice_thickness: recipe.slice_thickness,
                kvp: recipe.kvp,
                acquisition_number: "1",
                rescale_intercept: recipe.rescale_intercept,
                rescale_slope: recipe.rescale_slope,
                rescale_type: recipe.rescale_type,
                window_center: recipe.window_center,
                window_width: recipe.window_width,
            }),
            mg_image: None,
            dx_image: None,
            cr_image: None,
            mr_image: None,
        },
    )?;

    Ok(GeneratedFile {
        case_id: recipe.case_id.to_string(),
        manifest_entry: classic_ct_manifest_entry(
            case,
            recipe,
            &relative_path,
            &study_instance_uid,
            &series_instance_uid,
            &sop_instance_uid,
            &frame_of_reference_uid,
            &implementation_class_uid,
            &validated.bytes,
            validated.validation,
        ),
    })
}

#[allow(clippy::too_many_arguments)]
fn classic_ct_manifest_entry(
    case: &Value,
    recipe: ClassicCtRecipe,
    relative_path: &str,
    study_instance_uid: &str,
    series_instance_uid: &str,
    sop_instance_uid: &str,
    frame_of_reference_uid: &str,
    implementation_class_uid: &str,
    bytes: &[u8],
    validation: Value,
) -> Value {
    let mut standards_evidence = case
        .get("standards_evidence")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    standards_evidence.extend([
        serde_json::json!({
            "source": "dicom-standard-kb",
            "edition": "2026b",
            "query": "lookup_iod CT Image",
            "covered": true,
            "part": "PS3.3",
            "anchor": "table_A.3-1"
        }),
        serde_json::json!({
            "source": "dicom-standard-kb",
            "edition": "2026b",
            "query": "list_modules_for_iod CT Image",
            "covered": true,
            "part": "PS3.3",
            "anchor": "table_A.3-1"
        }),
        serde_json::json!({
            "source": "dicom-standard-kb",
            "edition": "2026b",
            "query": "list_attributes_for_module CT Image",
            "covered": true,
            "part": "PS3.3",
            "anchor": "table_C.8-3"
        }),
        serde_json::json!({
            "source": "dicom-standard-kb",
            "edition": "2026b",
            "query": "list_attributes_for_module Image Plane",
            "covered": true,
            "part": "PS3.3",
            "anchor": "table_C.7-10"
        }),
        serde_json::json!({
            "source": "dicom-standard-kb",
            "edition": "2026b",
            "query": "list_attributes_for_module Frame of Reference",
            "covered": true,
            "part": "PS3.3",
            "anchor": "table_C.7-6"
        }),
        serde_json::json!({
            "source": "dicom-standard-kb",
            "edition": "2026b",
            "query": "resolve_attribute_context RescaleIntercept --iod CT Image",
            "covered": true,
            "part": "PS3.3",
            "anchor": "table_C.8-3"
        }),
        serde_json::json!({
            "source": "dicom-standard-kb",
            "edition": "2026b",
            "query": "resolve_attribute_context RescaleSlope --iod CT Image",
            "covered": true,
            "part": "PS3.3",
            "anchor": "table_C.8-3"
        }),
        serde_json::json!({
            "source": "dicom-standard-kb",
            "edition": "2026b",
            "query": "resolve_attribute_context WindowCenter --iod CT Image",
            "covered": true,
            "part": "PS3.3",
            "anchor": "table_C.11-2b"
        }),
        serde_json::json!({
            "source": "dicom-standard-kb",
            "edition": "2026b",
            "query": "resolve_attribute_context WindowWidth --iod CT Image",
            "covered": true,
            "part": "PS3.3",
            "anchor": "table_C.11-2b"
        }),
    ]);

    serde_json::json!({
        "case_id": recipe.case_id,
        "profile_membership": ["core"],
        "path": relative_path,
        "sha256": sha256_hex(bytes),
        "size_bytes": bytes.len(),
        "determinism": "byte_stable",
        "recipe": {
            "recipe_id": recipe.recipe_id,
            "recipe_version": CLASSIC_CT_RECIPE_VERSION,
            "recipe_parameters": {
                "rows": recipe.rows,
                "columns": recipe.columns,
                "samples_per_pixel": 1,
                "photometric_interpretation": "MONOCHROME2",
                "bits_allocated": 16,
                "bits_stored": 12,
                "high_bit": 11,
                "pixel_representation": 1,
                "pixel_values": recipe.pixel_values,
                "rescale": {
                    "intercept": recipe.rescale_intercept,
                    "slope": recipe.rescale_slope,
                    "type": recipe.rescale_type
                },
                "window": {
                    "center": recipe.window_center,
                    "width": recipe.window_width
                },
                "geometry": {
                    "pixel_spacing": recipe.pixel_spacing,
                    "image_orientation_patient": recipe.image_orientation_patient,
                    "image_position_patient": recipe.image_position_patient,
                    "slice_thickness": recipe.slice_thickness
                }
            }
        },
        "dicom": {
            "sop_class_uid": uids::CT_IMAGE_STORAGE,
            "sop_class_name": "CT Image Storage",
            "iod_name": "CT Image",
            "modality": "CT",
            "transfer_syntax_uid": uids::EXPLICIT_VR_LITTLE_ENDIAN,
            "transfer_syntax_name": "Explicit VR Little Endian"
        },
        "uids": {
            "study_instance_uid": study_instance_uid,
            "series_instance_uid": series_instance_uid,
            "sop_instance_uid": sop_instance_uid,
            "frame_of_reference_uid": frame_of_reference_uid,
            "implementation_class_uid": implementation_class_uid
        },
        "image": {
            "rows": recipe.rows,
            "columns": recipe.columns,
            "frames": 1,
            "samples_per_pixel": 1,
            "photometric_interpretation": "MONOCHROME2",
            "bits_allocated": 16,
            "bits_stored": 12,
            "high_bit": 11,
            "pixel_representation": 1,
            "planar_configuration": Value::Null
        },
        "pixel_data": {
            "vr": "OW",
            "native_or_encapsulated": "native",
            "value_length": recipe.pixel_bytes.len(),
            "frame_count": 1,
            "frame_hashes": [sha256_hex(recipe.pixel_bytes)]
        },
        "expected_capabilities": ["open_file", "read_metadata", "render_native_pixels", "apply_modality_rescale", "apply_window"],
        "expected_semantics": {
            "synthetic_data": "YES",
            "image_type": "ORIGINAL\\PRIMARY\\AXIAL",
            "pixel_min": recipe.pixel_min,
            "pixel_max": recipe.pixel_max,
            "rescale": {
                "intercept": recipe.rescale_intercept,
                "slope": recipe.rescale_slope,
                "type": recipe.rescale_type,
                "output_min": -2048,
                "output_max": 1023
            },
            "window": {
                "center": recipe.window_center,
                "width": recipe.window_width
            }
        },
        "expected_visual_checks": {
            "pattern": "2x2_signed_ct_hu_gradient"
        },
        "validation": validation,
        "known_stressors": ["ct_image_storage", "signed_12_bit_pixels", "modality_rescale", "window_center_width"],
        "standards_evidence": standards_evidence
    })
}

fn write_classic_mg_case(
    run: &PreparedGenerationRun,
    case: &Value,
    recipe: ClassicMgRecipe,
    standards_lock_sha256: &str,
) -> Result<GeneratedFile, GenerateError> {
    let study_instance_uid = deterministic_classic_mg_uid(
        standards_lock_sha256,
        recipe,
        run.seed,
        UidRole::StudyInstance,
    );
    let series_instance_uid = deterministic_classic_mg_uid(
        standards_lock_sha256,
        recipe,
        run.seed,
        UidRole::SeriesInstance,
    );
    let sop_instance_uid = deterministic_classic_mg_uid(
        standards_lock_sha256,
        recipe,
        run.seed,
        UidRole::SopInstance,
    );
    let implementation_class_uid = deterministic_implementation_uid(standards_lock_sha256);

    let relative_path = format!("{}/instance.dcm", recipe.case_id);
    let path = run.out_dir.join(&relative_path);
    let case_dir = path.parent().ok_or_else(|| GenerateError::MetadataShape {
        path: PathBuf::from(&relative_path),
        message: "generated DICOM path must have a parent directory",
    })?;
    fs::create_dir_all(case_dir).map_err(|source| GenerateError::CreateCaseOutputDir {
        path: case_dir.to_path_buf(),
        source,
    })?;

    let mut obj = InMemDicomObject::new_empty();
    put_str(&mut obj, tags::SOP_CLASS_UID, VR::UI, recipe.sop_class_uid);
    put_str(&mut obj, tags::SOP_INSTANCE_UID, VR::UI, &sop_instance_uid);
    put_str(&mut obj, tags::SYNTHETIC_DATA, VR::CS, "YES");

    put_str(
        &mut obj,
        tags::PATIENT_NAME,
        VR::PN,
        "DTS^Synthetic^Patient001",
    );
    put_str(&mut obj, tags::PATIENT_ID, VR::LO, "DTS-PATIENT-001");
    put_str(&mut obj, tags::PATIENT_BIRTH_DATE, VR::DA, "19700101");
    put_str(&mut obj, tags::PATIENT_SEX, VR::CS, "O");

    put_str(
        &mut obj,
        tags::STUDY_INSTANCE_UID,
        VR::UI,
        &study_instance_uid,
    );
    put_str(&mut obj, tags::STUDY_DATE, VR::DA, "20260101");
    put_str(&mut obj, tags::STUDY_TIME, VR::TM, "000000");
    put_str(&mut obj, tags::REFERRING_PHYSICIAN_NAME, VR::PN, "");
    put_str(&mut obj, tags::STUDY_ID, VR::SH, "DTS-MG");
    put_str(&mut obj, tags::ACCESSION_NUMBER, VR::SH, "");

    put_str(&mut obj, tags::MODALITY, VR::CS, "MG");
    put_str(
        &mut obj,
        tags::PRESENTATION_INTENT_TYPE,
        VR::CS,
        recipe.presentation_intent_type,
    );
    put_str(
        &mut obj,
        tags::SERIES_INSTANCE_UID,
        VR::UI,
        &series_instance_uid,
    );
    put_str(&mut obj, tags::SERIES_NUMBER, VR::IS, "1");

    put_str(&mut obj, tags::MANUFACTURER, VR::LO, "dicom-test-suite");
    put_str(
        &mut obj,
        tags::MANUFACTURER_MODEL_NAME,
        VR::LO,
        recipe.recipe_id,
    );
    put_str(
        &mut obj,
        tags::SOFTWARE_VERSIONS,
        VR::LO,
        crate::PACKAGE_VERSION,
    );

    put_str(&mut obj, tags::ACQUISITION_NUMBER, VR::IS, "1");
    put_str(&mut obj, tags::ACQUISITION_DATE, VR::DA, "20260101");
    put_str(&mut obj, tags::ACQUISITION_TIME, VR::TM, "000000");

    put_str(&mut obj, tags::IMAGE_TYPE, VR::CS, "ORIGINAL\\PRIMARY");
    put_str(&mut obj, tags::INSTANCE_NUMBER, VR::IS, "1");
    put_str(&mut obj, tags::PATIENT_ORIENTATION, VR::CS, "A\\FR");
    put_str(&mut obj, tags::CONTENT_DATE, VR::DA, "20260101");
    put_str(&mut obj, tags::CONTENT_TIME, VR::TM, "000000");
    put_str(&mut obj, tags::BODY_PART_EXAMINED, VR::CS, "BREAST");
    put_str(&mut obj, tags::IMAGE_LATERALITY, VR::CS, "L");

    put_u16(&mut obj, tags::SAMPLES_PER_PIXEL, VR::US, 1);
    put_str(
        &mut obj,
        tags::PHOTOMETRIC_INTERPRETATION,
        VR::CS,
        recipe.photometric_interpretation,
    );
    put_u16(&mut obj, tags::ROWS, VR::US, recipe.rows);
    put_u16(&mut obj, tags::COLUMNS, VR::US, recipe.columns);
    put_u16(&mut obj, tags::BITS_ALLOCATED, VR::US, 16);
    put_u16(&mut obj, tags::BITS_STORED, VR::US, 12);
    put_u16(&mut obj, tags::HIGH_BIT, VR::US, 11);
    put_u16(&mut obj, tags::PIXEL_REPRESENTATION, VR::US, 0);

    put_str(&mut obj, tags::PIXEL_INTENSITY_RELATIONSHIP, VR::CS, "LIN");
    put_i16(
        &mut obj,
        tags::PIXEL_INTENSITY_RELATIONSHIP_SIGN,
        VR::SS,
        -1,
    );
    put_str(&mut obj, tags::RESCALE_INTERCEPT, VR::DS, "0");
    put_str(&mut obj, tags::RESCALE_SLOPE, VR::DS, "1");
    put_str(&mut obj, tags::RESCALE_TYPE, VR::LO, "US");
    put_str(
        &mut obj,
        tags::PRESENTATION_LUT_SHAPE,
        VR::CS,
        recipe.presentation_lut_shape,
    );
    put_str(&mut obj, tags::LOSSY_IMAGE_COMPRESSION, VR::CS, "00");
    put_str(&mut obj, tags::BURNED_IN_ANNOTATION, VR::CS, "NO");
    if let (Some(window_center), Some(window_width)) = (recipe.window_center, recipe.window_width) {
        put_str(&mut obj, tags::WINDOW_CENTER, VR::DS, window_center);
        put_str(&mut obj, tags::WINDOW_WIDTH, VR::DS, window_width);
    }

    put_str(
        &mut obj,
        tags::IMAGER_PIXEL_SPACING,
        VR::DS,
        recipe.imager_pixel_spacing,
    );
    put_str(&mut obj, tags::DETECTOR_TYPE, VR::CS, "DIRECT");
    put_str(&mut obj, tags::DETECTOR_CONFIGURATION, VR::CS, "AREA");
    put_str(
        &mut obj,
        tags::DETECTOR_DESCRIPTION,
        VR::LT,
        "synthetic detector",
    );
    put_str(&mut obj, tags::DETECTOR_ID, VR::SH, "DTS-MG-DET");
    put_str(
        &mut obj,
        tags::DETECTOR_ELEMENT_SPACING,
        VR::DS,
        recipe.imager_pixel_spacing,
    );
    put_str(&mut obj, tags::FIELD_OF_VIEW_SHAPE, VR::CS, "RECTANGLE");
    put_str(
        &mut obj,
        tags::FIELD_OF_VIEW_DIMENSIONS,
        VR::DS,
        "0.14\\0.14",
    );

    put_str(&mut obj, tags::POSITIONER_TYPE, VR::CS, "MAMMOGRAPHIC");
    put_str(&mut obj, tags::VIEW_POSITION, VR::CS, "MLO");
    put_str(&mut obj, tags::ORGAN_EXPOSED, VR::CS, "BREAST");
    put_str(&mut obj, tags::BREAST_IMPLANT_PRESENT, VR::CS, "NO");
    put_code_sequence(
        &mut obj,
        tags::ANATOMIC_REGION_SEQUENCE,
        "76752008",
        "SCT",
        "Breast structure",
    );
    put_code_sequence(
        &mut obj,
        tags::VIEW_CODE_SEQUENCE,
        "399162004",
        "SCT",
        "Mediolateral oblique projection",
    );
    put_empty_sequence(&mut obj, tags::ACQUISITION_CONTEXT_SEQUENCE);

    obj.put(DataElement::new(
        tags::PIXEL_DATA,
        VR::OW,
        PrimitiveValue::from(recipe.pixel_bytes),
    ));

    let file_obj = obj
        .with_meta(
            FileMetaTableBuilder::new()
                .transfer_syntax(recipe.transfer_syntax_uid)
                .implementation_class_uid(&implementation_class_uid)
                .implementation_version_name("DICOMTS010"),
        )
        .map_err(|err| GenerateError::WriteDicomFile {
            path: path.clone(),
            message: err.to_string(),
        })?;

    file_obj
        .write_to_file(&path)
        .map_err(|err| GenerateError::WriteDicomFile {
            path: path.clone(),
            message: err.to_string(),
        })?;

    let validated = validate_part10_file(
        &path,
        &Part10Expectations {
            sop_class_uid: recipe.sop_class_uid,
            sop_instance_uid: &sop_instance_uid,
            transfer_syntax_uid: recipe.transfer_syntax_uid,
            implementation_class_uid: &implementation_class_uid,
            synthetic_data: "YES",
            rows: recipe.rows,
            columns: recipe.columns,
            samples_per_pixel: 1,
            photometric_interpretation: recipe.photometric_interpretation,
            bits_allocated: 16,
            bits_stored: 12,
            high_bit: 11,
            pixel_representation: 0,
            planar_configuration: None,
            pixel_data_vr: VR::OW,
            pixel_data_length_formula: PixelDataLengthFormula::ContiguousSamples,
            palette: None,
            padding: None,
            ct_image: None,
            mg_image: Some(MgImageExpectations {
                modality: "MG",
                presentation_intent_type: recipe.presentation_intent_type,
                image_type: "ORIGINAL\\PRIMARY",
                image_laterality: "L",
                view_position: "MLO",
                body_part_examined: "BREAST",
                organ_exposed: "BREAST",
                positioner_type: "MAMMOGRAPHIC",
                imager_pixel_spacing: recipe.imager_pixel_spacing,
                detector_type: "DIRECT",
                detector_configuration: "AREA",
                detector_id: "DTS-MG-DET",
                pixel_intensity_relationship: "LIN",
                pixel_intensity_relationship_sign: -1,
                rescale_intercept: "0",
                rescale_slope: "1",
                rescale_type: "US",
                presentation_lut_shape: recipe.presentation_lut_shape,
                lossy_image_compression: "00",
                burned_in_annotation: "NO",
                breast_implant_present: "NO",
                window_center: recipe.window_center,
                window_width: recipe.window_width,
                anatomic_region_code_value: "76752008",
                view_code_value: "399162004",
                acquisition_context_items: 0,
            }),
            dx_image: None,
            cr_image: None,
            mr_image: None,
        },
    )?;

    Ok(GeneratedFile {
        case_id: recipe.case_id.to_string(),
        manifest_entry: classic_mg_manifest_entry(
            case,
            recipe,
            &relative_path,
            &study_instance_uid,
            &series_instance_uid,
            &sop_instance_uid,
            &implementation_class_uid,
            &validated.bytes,
            validated.validation,
        ),
    })
}

#[allow(clippy::too_many_arguments)]
fn classic_mg_manifest_entry(
    case: &Value,
    recipe: ClassicMgRecipe,
    relative_path: &str,
    study_instance_uid: &str,
    series_instance_uid: &str,
    sop_instance_uid: &str,
    implementation_class_uid: &str,
    bytes: &[u8],
    validation: Value,
) -> Value {
    let mut standards_evidence = case
        .get("standards_evidence")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    standards_evidence.extend([
        serde_json::json!({
            "source": "dicom-standard-kb",
            "edition": "2026b",
            "query": "lookup_iod Digital Mammography X-Ray Image",
            "covered": true,
            "part": "PS3.3",
            "anchor": "table_A.27-1"
        }),
        serde_json::json!({
            "source": "dicom-standard-kb",
            "edition": "2026b",
            "query": "list_modules_for_iod Digital Mammography X-Ray Image",
            "covered": true,
            "part": "PS3.3",
            "anchor": "table_A.27-1"
        }),
        serde_json::json!({
            "source": "dicom-standard-kb",
            "edition": "2026b",
            "query": "list_attributes_for_module DX Series",
            "covered": true,
            "part": "PS3.3",
            "anchor": "table_C.8-68"
        }),
        serde_json::json!({
            "source": "dicom-standard-kb",
            "edition": "2026b",
            "query": "list_attributes_for_module DX Image",
            "covered": true,
            "part": "PS3.3",
            "anchor": "table_C.8-70"
        }),
        serde_json::json!({
            "source": "dicom-standard-kb",
            "edition": "2026b",
            "query": "list_attributes_for_module DX Detector --expand-macros",
            "covered": true,
            "part": "PS3.3",
            "anchor": "table_C.8-71"
        }),
        serde_json::json!({
            "source": "dicom-standard-kb",
            "edition": "2026b",
            "query": "list_attributes_for_module DX Anatomy Imaged",
            "covered": true,
            "part": "PS3.3",
            "anchor": "table_C.8-69"
        }),
        serde_json::json!({
            "source": "dicom-standard-kb",
            "edition": "2026b",
            "query": "list_attributes_for_module Mammography Series",
            "covered": true,
            "part": "PS3.3",
            "anchor": "table_C.8-73"
        }),
        serde_json::json!({
            "source": "dicom-standard-kb",
            "edition": "2026b",
            "query": "list_attributes_for_module Mammography Image",
            "covered": true,
            "part": "PS3.3",
            "anchor": "table_C.8-74"
        }),
        serde_json::json!({
            "source": "dicom-standard-kb",
            "edition": "2026b",
            "query": "list_attributes_for_module Acquisition Context",
            "covered": true,
            "part": "PS3.3",
            "anchor": "table_C.7.6.14-1"
        }),
        serde_json::json!({
            "source": "dicom-standard-kb",
            "edition": "2026b",
            "query": "lookup_data_element PresentationIntentType",
            "covered": true,
            "part": "PS3.6",
            "anchor": "table_6-1"
        }),
        serde_json::json!({
            "source": "dicom-standard-kb",
            "edition": "2026b",
            "query": "lookup_data_element ImagerPixelSpacing",
            "covered": true,
            "part": "PS3.6",
            "anchor": "table_6-1"
        }),
        serde_json::json!({
            "source": "dicom-standard-kb",
            "edition": "2026b",
            "query": "lookup_data_element PresentationLUTShape",
            "covered": true,
            "part": "PS3.6",
            "anchor": "table_6-1"
        }),
    ]);

    let window_manifest = serde_json::json!({
        "center": recipe.window_center,
        "width": recipe.window_width
    });
    let expected_capabilities = if recipe.window_center.is_some() {
        serde_json::json!([
            "open_file",
            "read_metadata",
            "render_native_pixels",
            "apply_window"
        ])
    } else {
        serde_json::json!(["open_file", "read_metadata", "render_native_pixels"])
    };
    let photometric_semantics = if recipe.photometric_interpretation == "MONOCHROME1" {
        "MONOCHROME1 with Presentation LUT Shape INVERSE maps stored values to P-Values for presentation"
    } else {
        "MONOCHROME2 with Presentation LUT Shape IDENTITY preserves increasing stored values for processing"
    };
    let visual_pattern = if recipe.photometric_interpretation == "MONOCHROME1" {
        "2x2_mammography_mono1_12bit_gradient"
    } else {
        "2x2_mammography_mono2_12bit_processing_gradient"
    };
    let known_stressors = if recipe.presentation_intent_type == "FOR PROCESSING" {
        serde_json::json!([
            "digital_mammography_for_processing",
            "implicit_vr_little_endian",
            "mono2_processing_pixels",
            "unsigned_12_bit_pixels"
        ])
    } else {
        serde_json::json!([
            "digital_mammography_for_presentation",
            "mono1_inversion",
            "unsigned_12_bit_pixels",
            "presentation_lut_inverse"
        ])
    };

    serde_json::json!({
        "case_id": recipe.case_id,
        "profile_membership": ["core"],
        "path": relative_path,
        "sha256": sha256_hex(bytes),
        "size_bytes": bytes.len(),
        "determinism": "byte_stable",
        "recipe": {
            "recipe_id": recipe.recipe_id,
            "recipe_version": CLASSIC_MG_RECIPE_VERSION,
            "recipe_parameters": {
                "rows": recipe.rows,
                "columns": recipe.columns,
                "samples_per_pixel": 1,
                "photometric_interpretation": recipe.photometric_interpretation,
                "bits_allocated": 16,
                "bits_stored": 12,
                "high_bit": 11,
                "pixel_representation": 0,
                "pixel_values": recipe.pixel_values,
                "presentation_intent_type": recipe.presentation_intent_type,
                "image_laterality": "L",
                "view_position": "MLO",
                "imager_pixel_spacing": recipe.imager_pixel_spacing,
                "presentation_lut_shape": recipe.presentation_lut_shape,
                "window": window_manifest
            }
        },
        "dicom": {
            "sop_class_uid": recipe.sop_class_uid,
            "sop_class_name": recipe.sop_class_name,
            "iod_name": "Digital Mammography X-Ray Image",
            "modality": "MG",
            "transfer_syntax_uid": recipe.transfer_syntax_uid,
            "transfer_syntax_name": recipe.transfer_syntax_name
        },
        "uids": {
            "study_instance_uid": study_instance_uid,
            "series_instance_uid": series_instance_uid,
            "sop_instance_uid": sop_instance_uid,
            "frame_of_reference_uid": Value::Null,
            "implementation_class_uid": implementation_class_uid
        },
        "image": {
            "rows": recipe.rows,
            "columns": recipe.columns,
            "frames": 1,
            "samples_per_pixel": 1,
            "photometric_interpretation": recipe.photometric_interpretation,
            "bits_allocated": 16,
            "bits_stored": 12,
            "high_bit": 11,
            "pixel_representation": 0,
            "planar_configuration": Value::Null
        },
        "pixel_data": {
            "vr": "OW",
            "native_or_encapsulated": "native",
            "value_length": recipe.pixel_bytes.len(),
            "frame_count": 1,
            "frame_hashes": [sha256_hex(recipe.pixel_bytes)]
        },
        "expected_capabilities": expected_capabilities,
        "expected_semantics": {
            "synthetic_data": "YES",
            "presentation_intent_type": recipe.presentation_intent_type,
            "pixel_min": recipe.pixel_min,
            "pixel_max": recipe.pixel_max,
            "photometric_semantics": photometric_semantics,
            "window": window_manifest
        },
        "expected_visual_checks": {
            "pattern": visual_pattern
        },
        "validation": validation,
        "known_stressors": known_stressors,
        "standards_evidence": standards_evidence
    })
}

fn write_classic_dx_case(
    run: &PreparedGenerationRun,
    case: &Value,
    recipe: ClassicDxRecipe,
    standards_lock_sha256: &str,
) -> Result<GeneratedFile, GenerateError> {
    let study_instance_uid = deterministic_classic_dx_uid(
        standards_lock_sha256,
        recipe,
        run.seed,
        UidRole::StudyInstance,
    );
    let series_instance_uid = deterministic_classic_dx_uid(
        standards_lock_sha256,
        recipe,
        run.seed,
        UidRole::SeriesInstance,
    );
    let sop_instance_uid = deterministic_classic_dx_uid(
        standards_lock_sha256,
        recipe,
        run.seed,
        UidRole::SopInstance,
    );
    let implementation_class_uid = deterministic_implementation_uid(standards_lock_sha256);

    let relative_path = format!("{}/instance.dcm", recipe.case_id);
    let path = run.out_dir.join(&relative_path);
    let case_dir = path.parent().ok_or_else(|| GenerateError::MetadataShape {
        path: PathBuf::from(&relative_path),
        message: "generated DICOM path must have a parent directory",
    })?;
    fs::create_dir_all(case_dir).map_err(|source| GenerateError::CreateCaseOutputDir {
        path: case_dir.to_path_buf(),
        source,
    })?;

    let mut obj = InMemDicomObject::new_empty();
    put_str(
        &mut obj,
        tags::SOP_CLASS_UID,
        VR::UI,
        uids::DIGITAL_X_RAY_IMAGE_STORAGE_FOR_PRESENTATION,
    );
    put_str(&mut obj, tags::SOP_INSTANCE_UID, VR::UI, &sop_instance_uid);
    put_str(&mut obj, tags::SYNTHETIC_DATA, VR::CS, "YES");

    put_str(
        &mut obj,
        tags::PATIENT_NAME,
        VR::PN,
        "DTS^Synthetic^Patient001",
    );
    put_str(&mut obj, tags::PATIENT_ID, VR::LO, "DTS-PATIENT-001");
    put_str(&mut obj, tags::PATIENT_BIRTH_DATE, VR::DA, "19700101");
    put_str(&mut obj, tags::PATIENT_SEX, VR::CS, "O");

    put_str(
        &mut obj,
        tags::STUDY_INSTANCE_UID,
        VR::UI,
        &study_instance_uid,
    );
    put_str(&mut obj, tags::STUDY_DATE, VR::DA, "20260101");
    put_str(&mut obj, tags::STUDY_TIME, VR::TM, "000000");
    put_str(&mut obj, tags::REFERRING_PHYSICIAN_NAME, VR::PN, "");
    put_str(&mut obj, tags::STUDY_ID, VR::SH, "DTS-DX");
    put_str(&mut obj, tags::ACCESSION_NUMBER, VR::SH, "");

    put_str(&mut obj, tags::MODALITY, VR::CS, "DX");
    put_str(
        &mut obj,
        tags::PRESENTATION_INTENT_TYPE,
        VR::CS,
        "FOR PRESENTATION",
    );
    put_str(
        &mut obj,
        tags::SERIES_INSTANCE_UID,
        VR::UI,
        &series_instance_uid,
    );
    put_str(&mut obj, tags::SERIES_NUMBER, VR::IS, "1");

    put_str(&mut obj, tags::MANUFACTURER, VR::LO, "dicom-test-suite");
    put_str(
        &mut obj,
        tags::MANUFACTURER_MODEL_NAME,
        VR::LO,
        recipe.recipe_id,
    );
    put_str(
        &mut obj,
        tags::SOFTWARE_VERSIONS,
        VR::LO,
        crate::PACKAGE_VERSION,
    );

    put_str(&mut obj, tags::ACQUISITION_NUMBER, VR::IS, "1");
    put_str(&mut obj, tags::ACQUISITION_DATE, VR::DA, "20260101");
    put_str(&mut obj, tags::ACQUISITION_TIME, VR::TM, "000000");

    put_str(&mut obj, tags::IMAGE_TYPE, VR::CS, "ORIGINAL\\PRIMARY");
    put_str(&mut obj, tags::INSTANCE_NUMBER, VR::IS, "1");
    put_str(&mut obj, tags::PATIENT_ORIENTATION, VR::CS, "P\\F");
    put_str(&mut obj, tags::CONTENT_DATE, VR::DA, "20260101");
    put_str(&mut obj, tags::CONTENT_TIME, VR::TM, "000000");
    put_str(&mut obj, tags::BODY_PART_EXAMINED, VR::CS, "CHEST");
    put_str(&mut obj, tags::IMAGE_LATERALITY, VR::CS, "U");

    put_u16(&mut obj, tags::SAMPLES_PER_PIXEL, VR::US, 1);
    put_str(
        &mut obj,
        tags::PHOTOMETRIC_INTERPRETATION,
        VR::CS,
        "MONOCHROME2",
    );
    put_u16(&mut obj, tags::ROWS, VR::US, recipe.rows);
    put_u16(&mut obj, tags::COLUMNS, VR::US, recipe.columns);
    put_u16(&mut obj, tags::BITS_ALLOCATED, VR::US, 16);
    put_u16(&mut obj, tags::BITS_STORED, VR::US, 12);
    put_u16(&mut obj, tags::HIGH_BIT, VR::US, 11);
    put_u16(&mut obj, tags::PIXEL_REPRESENTATION, VR::US, 0);

    put_str(&mut obj, tags::PIXEL_INTENSITY_RELATIONSHIP, VR::CS, "LIN");
    put_i16(
        &mut obj,
        tags::PIXEL_INTENSITY_RELATIONSHIP_SIGN,
        VR::SS,
        -1,
    );
    put_str(&mut obj, tags::RESCALE_INTERCEPT, VR::DS, "0");
    put_str(&mut obj, tags::RESCALE_SLOPE, VR::DS, "1");
    put_str(&mut obj, tags::RESCALE_TYPE, VR::LO, "US");
    put_str(&mut obj, tags::PRESENTATION_LUT_SHAPE, VR::CS, "IDENTITY");
    put_str(&mut obj, tags::LOSSY_IMAGE_COMPRESSION, VR::CS, "00");
    put_str(&mut obj, tags::BURNED_IN_ANNOTATION, VR::CS, "NO");
    put_str(&mut obj, tags::WINDOW_CENTER, VR::DS, recipe.window_center);
    put_str(&mut obj, tags::WINDOW_WIDTH, VR::DS, recipe.window_width);

    put_str(
        &mut obj,
        tags::IMAGER_PIXEL_SPACING,
        VR::DS,
        recipe.imager_pixel_spacing,
    );
    put_str(&mut obj, tags::DETECTOR_TYPE, VR::CS, "DIRECT");
    put_str(&mut obj, tags::DETECTOR_CONFIGURATION, VR::CS, "AREA");
    put_str(
        &mut obj,
        tags::DETECTOR_DESCRIPTION,
        VR::LT,
        "synthetic detector",
    );
    put_str(&mut obj, tags::DETECTOR_ID, VR::SH, "DTS-DX-DET");
    put_str(
        &mut obj,
        tags::DETECTOR_ELEMENT_SPACING,
        VR::DS,
        recipe.imager_pixel_spacing,
    );
    put_str(&mut obj, tags::FIELD_OF_VIEW_SHAPE, VR::CS, "RECTANGLE");
    put_str(
        &mut obj,
        tags::FIELD_OF_VIEW_DIMENSIONS,
        VR::DS,
        "0.30\\0.30",
    );

    put_code_sequence(
        &mut obj,
        tags::ANATOMIC_REGION_SEQUENCE,
        "51185008",
        "SCT",
        "Thoracic structure",
    );
    put_empty_sequence(&mut obj, tags::ACQUISITION_CONTEXT_SEQUENCE);

    put_str(&mut obj, tags::SHUTTER_SHAPE, VR::CS, "RECTANGULAR");
    put_str(
        &mut obj,
        tags::SHUTTER_LEFT_VERTICAL_EDGE,
        VR::IS,
        recipe.shutter_left_vertical_edge,
    );
    put_str(
        &mut obj,
        tags::SHUTTER_RIGHT_VERTICAL_EDGE,
        VR::IS,
        recipe.shutter_right_vertical_edge,
    );
    put_str(
        &mut obj,
        tags::SHUTTER_UPPER_HORIZONTAL_EDGE,
        VR::IS,
        recipe.shutter_upper_horizontal_edge,
    );
    put_str(
        &mut obj,
        tags::SHUTTER_LOWER_HORIZONTAL_EDGE,
        VR::IS,
        recipe.shutter_lower_horizontal_edge,
    );
    put_u16(
        &mut obj,
        tags::SHUTTER_PRESENTATION_VALUE,
        VR::US,
        recipe.shutter_presentation_value,
    );

    obj.put(DataElement::new(
        tags::PIXEL_DATA,
        VR::OW,
        PrimitiveValue::from(recipe.pixel_bytes),
    ));

    let file_obj = obj
        .with_meta(
            FileMetaTableBuilder::new()
                .transfer_syntax(uids::EXPLICIT_VR_LITTLE_ENDIAN)
                .implementation_class_uid(&implementation_class_uid)
                .implementation_version_name("DICOMTS010"),
        )
        .map_err(|err| GenerateError::WriteDicomFile {
            path: path.clone(),
            message: err.to_string(),
        })?;

    file_obj
        .write_to_file(&path)
        .map_err(|err| GenerateError::WriteDicomFile {
            path: path.clone(),
            message: err.to_string(),
        })?;

    let validated = validate_part10_file(
        &path,
        &Part10Expectations {
            sop_class_uid: uids::DIGITAL_X_RAY_IMAGE_STORAGE_FOR_PRESENTATION,
            sop_instance_uid: &sop_instance_uid,
            transfer_syntax_uid: uids::EXPLICIT_VR_LITTLE_ENDIAN,
            implementation_class_uid: &implementation_class_uid,
            synthetic_data: "YES",
            rows: recipe.rows,
            columns: recipe.columns,
            samples_per_pixel: 1,
            photometric_interpretation: "MONOCHROME2",
            bits_allocated: 16,
            bits_stored: 12,
            high_bit: 11,
            pixel_representation: 0,
            planar_configuration: None,
            pixel_data_vr: VR::OW,
            pixel_data_length_formula: PixelDataLengthFormula::ContiguousSamples,
            palette: None,
            padding: None,
            ct_image: None,
            mg_image: None,
            dx_image: Some(DxImageExpectations {
                modality: "DX",
                presentation_intent_type: "FOR PRESENTATION",
                image_type: "ORIGINAL\\PRIMARY",
                image_laterality: "U",
                body_part_examined: "CHEST",
                imager_pixel_spacing: recipe.imager_pixel_spacing,
                detector_type: "DIRECT",
                detector_configuration: "AREA",
                detector_id: "DTS-DX-DET",
                pixel_intensity_relationship: "LIN",
                pixel_intensity_relationship_sign: -1,
                rescale_intercept: "0",
                rescale_slope: "1",
                rescale_type: "US",
                presentation_lut_shape: "IDENTITY",
                lossy_image_compression: "00",
                burned_in_annotation: "NO",
                window_center: recipe.window_center,
                window_width: recipe.window_width,
                anatomic_region_code_value: "51185008",
                acquisition_context_items: 0,
                shutter_shape: "RECTANGULAR",
                shutter_left_vertical_edge: recipe.shutter_left_vertical_edge,
                shutter_right_vertical_edge: recipe.shutter_right_vertical_edge,
                shutter_upper_horizontal_edge: recipe.shutter_upper_horizontal_edge,
                shutter_lower_horizontal_edge: recipe.shutter_lower_horizontal_edge,
                shutter_presentation_value: recipe.shutter_presentation_value,
            }),
            cr_image: None,
            mr_image: None,
        },
    )?;

    Ok(GeneratedFile {
        case_id: recipe.case_id.to_string(),
        manifest_entry: classic_dx_manifest_entry(
            case,
            recipe,
            &relative_path,
            &study_instance_uid,
            &series_instance_uid,
            &sop_instance_uid,
            &implementation_class_uid,
            &validated.bytes,
            validated.validation,
        ),
    })
}

#[allow(clippy::too_many_arguments)]
fn classic_dx_manifest_entry(
    case: &Value,
    recipe: ClassicDxRecipe,
    relative_path: &str,
    study_instance_uid: &str,
    series_instance_uid: &str,
    sop_instance_uid: &str,
    implementation_class_uid: &str,
    bytes: &[u8],
    validation: Value,
) -> Value {
    let mut standards_evidence = case
        .get("standards_evidence")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    standards_evidence.extend([
        serde_json::json!({
            "source": "dicom-standard-kb",
            "edition": "2026b",
            "query": "lookup_iod Digital X-Ray Image",
            "covered": true,
            "part": "PS3.3",
            "anchor": "table_A.26-1"
        }),
        serde_json::json!({
            "source": "dicom-standard-kb",
            "edition": "2026b",
            "query": "list_modules_for_iod Digital X-Ray Image",
            "covered": true,
            "part": "PS3.3",
            "anchor": "table_A.26-1"
        }),
        serde_json::json!({
            "source": "dicom-standard-kb",
            "edition": "2026b",
            "query": "list_attributes_for_module DX Series --expand-macros",
            "covered": true,
            "part": "PS3.3",
            "anchor": "table_C.8-68"
        }),
        serde_json::json!({
            "source": "dicom-standard-kb",
            "edition": "2026b",
            "query": "list_attributes_for_module DX Anatomy Imaged --expand-macros",
            "covered": true,
            "part": "PS3.3",
            "anchor": "table_C.8-69"
        }),
        serde_json::json!({
            "source": "dicom-standard-kb",
            "edition": "2026b",
            "query": "list_attributes_for_module DX Image --expand-macros",
            "covered": true,
            "part": "PS3.3",
            "anchor": "table_C.8-70"
        }),
        serde_json::json!({
            "source": "dicom-standard-kb",
            "edition": "2026b",
            "query": "list_attributes_for_module DX Detector --expand-macros",
            "covered": true,
            "part": "PS3.3",
            "anchor": "table_C.8-71"
        }),
        serde_json::json!({
            "source": "dicom-standard-kb",
            "edition": "2026b",
            "query": "list_attributes_for_module Display Shutter --expand-macros",
            "covered": true,
            "part": "PS3.3",
            "anchor": "table_C.7-17"
        }),
        serde_json::json!({
            "source": "dicom-standard-kb",
            "edition": "2026b",
            "query": "lookup_data_element PresentationIntentType",
            "covered": true,
            "part": "PS3.6",
            "anchor": "table_6-1"
        }),
        serde_json::json!({
            "source": "dicom-standard-kb",
            "edition": "2026b",
            "query": "lookup_data_element ShutterShape",
            "covered": true,
            "part": "PS3.6",
            "anchor": "table_6-1"
        }),
        serde_json::json!({
            "source": "dicom-standard-kb",
            "edition": "2026b",
            "query": "lookup_data_element ShutterPresentationValue",
            "covered": true,
            "part": "PS3.6",
            "anchor": "table_6-1"
        }),
    ]);

    serde_json::json!({
        "case_id": recipe.case_id,
        "profile_membership": ["core"],
        "path": relative_path,
        "sha256": sha256_hex(bytes),
        "size_bytes": bytes.len(),
        "determinism": "byte_stable",
        "recipe": {
            "recipe_id": recipe.recipe_id,
            "recipe_version": CLASSIC_DX_RECIPE_VERSION,
            "recipe_parameters": {
                "rows": recipe.rows,
                "columns": recipe.columns,
                "samples_per_pixel": 1,
                "photometric_interpretation": "MONOCHROME2",
                "bits_allocated": 16,
                "bits_stored": 12,
                "high_bit": 11,
                "pixel_representation": 0,
                "pixel_values": recipe.pixel_values,
                "presentation_intent_type": "FOR PRESENTATION",
                "image_laterality": "U",
                "imager_pixel_spacing": recipe.imager_pixel_spacing,
                "presentation_lut_shape": "IDENTITY",
                "window": {
                    "center": recipe.window_center,
                    "width": recipe.window_width
                },
                "display_shutter": {
                    "shape": "RECTANGULAR",
                    "left_vertical_edge": recipe.shutter_left_vertical_edge,
                    "right_vertical_edge": recipe.shutter_right_vertical_edge,
                    "upper_horizontal_edge": recipe.shutter_upper_horizontal_edge,
                    "lower_horizontal_edge": recipe.shutter_lower_horizontal_edge,
                    "presentation_value": recipe.shutter_presentation_value
                }
            }
        },
        "dicom": {
            "sop_class_uid": uids::DIGITAL_X_RAY_IMAGE_STORAGE_FOR_PRESENTATION,
            "sop_class_name": "Digital X-Ray Image Storage - For Presentation",
            "iod_name": "Digital X-Ray Image",
            "modality": "DX",
            "transfer_syntax_uid": uids::EXPLICIT_VR_LITTLE_ENDIAN,
            "transfer_syntax_name": "Explicit VR Little Endian"
        },
        "uids": {
            "study_instance_uid": study_instance_uid,
            "series_instance_uid": series_instance_uid,
            "sop_instance_uid": sop_instance_uid,
            "frame_of_reference_uid": Value::Null,
            "implementation_class_uid": implementation_class_uid
        },
        "image": {
            "rows": recipe.rows,
            "columns": recipe.columns,
            "frames": 1,
            "samples_per_pixel": 1,
            "photometric_interpretation": "MONOCHROME2",
            "bits_allocated": 16,
            "bits_stored": 12,
            "high_bit": 11,
            "pixel_representation": 0,
            "planar_configuration": Value::Null
        },
        "pixel_data": {
            "vr": "OW",
            "native_or_encapsulated": "native",
            "value_length": recipe.pixel_bytes.len(),
            "frame_count": 1,
            "frame_hashes": [sha256_hex(recipe.pixel_bytes)]
        },
        "expected_capabilities": ["open_file", "read_metadata", "render_native_pixels", "apply_window", "apply_display_shutter"],
        "expected_semantics": {
            "synthetic_data": "YES",
            "presentation_intent_type": "FOR PRESENTATION",
            "pixel_min": recipe.pixel_min,
            "pixel_max": recipe.pixel_max,
            "display_shutter": {
                "shape": "RECTANGULAR",
                "left_vertical_edge": recipe.shutter_left_vertical_edge,
                "right_vertical_edge": recipe.shutter_right_vertical_edge,
                "upper_horizontal_edge": recipe.shutter_upper_horizontal_edge,
                "lower_horizontal_edge": recipe.shutter_lower_horizontal_edge,
                "presentation_value": recipe.shutter_presentation_value
            }
        },
        "expected_visual_checks": {
            "pattern": "2x2_dx_mono2_12bit_display_shutter"
        },
        "validation": validation,
        "known_stressors": ["digital_x_ray_for_presentation", "display_shutter", "unsigned_12_bit_pixels", "voi_window"],
        "standards_evidence": standards_evidence
    })
}

fn write_classic_cr_case(
    run: &PreparedGenerationRun,
    case: &Value,
    recipe: ClassicCrRecipe,
    standards_lock_sha256: &str,
) -> Result<GeneratedFile, GenerateError> {
    let study_instance_uid = deterministic_classic_cr_uid(
        standards_lock_sha256,
        recipe,
        run.seed,
        UidRole::StudyInstance,
    );
    let series_instance_uid = deterministic_classic_cr_uid(
        standards_lock_sha256,
        recipe,
        run.seed,
        UidRole::SeriesInstance,
    );
    let sop_instance_uid = deterministic_classic_cr_uid(
        standards_lock_sha256,
        recipe,
        run.seed,
        UidRole::SopInstance,
    );
    let implementation_class_uid = deterministic_implementation_uid(standards_lock_sha256);

    let relative_path = format!("{}/instance.dcm", recipe.case_id);
    let path = run.out_dir.join(&relative_path);
    let case_dir = path.parent().ok_or_else(|| GenerateError::MetadataShape {
        path: PathBuf::from(&relative_path),
        message: "generated DICOM path must have a parent directory",
    })?;
    fs::create_dir_all(case_dir).map_err(|source| GenerateError::CreateCaseOutputDir {
        path: case_dir.to_path_buf(),
        source,
    })?;

    let overlay_rows = tags::OVERLAY_ROWS.inner();
    let overlay_columns = tags::OVERLAY_COLUMNS.inner();
    let overlay_type = tags::OVERLAY_TYPE.inner();
    let overlay_origin = tags::OVERLAY_ORIGIN.inner();
    let overlay_bits_allocated = tags::OVERLAY_BITS_ALLOCATED.inner();
    let overlay_bit_position = tags::OVERLAY_BIT_POSITION.inner();
    let overlay_data = tags::OVERLAY_DATA.inner();

    let mut obj = InMemDicomObject::new_empty();
    put_str(
        &mut obj,
        tags::SOP_CLASS_UID,
        VR::UI,
        uids::COMPUTED_RADIOGRAPHY_IMAGE_STORAGE,
    );
    put_str(&mut obj, tags::SOP_INSTANCE_UID, VR::UI, &sop_instance_uid);
    put_str(&mut obj, tags::SYNTHETIC_DATA, VR::CS, "YES");

    put_str(
        &mut obj,
        tags::PATIENT_NAME,
        VR::PN,
        "DTS^Synthetic^Patient001",
    );
    put_str(&mut obj, tags::PATIENT_ID, VR::LO, "DTS-PATIENT-001");
    put_str(&mut obj, tags::PATIENT_BIRTH_DATE, VR::DA, "19700101");
    put_str(&mut obj, tags::PATIENT_SEX, VR::CS, "O");

    put_str(
        &mut obj,
        tags::STUDY_INSTANCE_UID,
        VR::UI,
        &study_instance_uid,
    );
    put_str(&mut obj, tags::STUDY_DATE, VR::DA, "20260101");
    put_str(&mut obj, tags::STUDY_TIME, VR::TM, "000000");
    put_str(&mut obj, tags::REFERRING_PHYSICIAN_NAME, VR::PN, "");
    put_str(&mut obj, tags::STUDY_ID, VR::SH, "DTS-CR");
    put_str(&mut obj, tags::ACCESSION_NUMBER, VR::SH, "");

    put_str(&mut obj, tags::MODALITY, VR::CS, "CR");
    put_str(
        &mut obj,
        tags::SERIES_INSTANCE_UID,
        VR::UI,
        &series_instance_uid,
    );
    put_str(&mut obj, tags::SERIES_NUMBER, VR::IS, "1");
    put_str(
        &mut obj,
        tags::BODY_PART_EXAMINED,
        VR::CS,
        recipe.body_part_examined,
    );
    put_str(&mut obj, tags::VIEW_POSITION, VR::CS, recipe.view_position);

    put_str(&mut obj, tags::MANUFACTURER, VR::LO, "dicom-test-suite");
    put_str(
        &mut obj,
        tags::MANUFACTURER_MODEL_NAME,
        VR::LO,
        recipe.recipe_id,
    );
    put_str(
        &mut obj,
        tags::SOFTWARE_VERSIONS,
        VR::LO,
        crate::PACKAGE_VERSION,
    );

    put_str(&mut obj, tags::ACQUISITION_NUMBER, VR::IS, "1");
    put_str(&mut obj, tags::ACQUISITION_DATE, VR::DA, "20260101");
    put_str(&mut obj, tags::ACQUISITION_TIME, VR::TM, "000000");

    put_str(&mut obj, tags::IMAGE_TYPE, VR::CS, "ORIGINAL\\PRIMARY");
    put_str(&mut obj, tags::INSTANCE_NUMBER, VR::IS, "1");
    put_str(&mut obj, tags::PATIENT_ORIENTATION, VR::CS, "");
    put_str(&mut obj, tags::CONTENT_DATE, VR::DA, "20260101");
    put_str(&mut obj, tags::CONTENT_TIME, VR::TM, "000000");

    put_u16(&mut obj, tags::SAMPLES_PER_PIXEL, VR::US, 1);
    put_str(
        &mut obj,
        tags::PHOTOMETRIC_INTERPRETATION,
        VR::CS,
        "MONOCHROME2",
    );
    put_u16(&mut obj, tags::ROWS, VR::US, recipe.rows);
    put_u16(&mut obj, tags::COLUMNS, VR::US, recipe.columns);
    put_u16(&mut obj, tags::BITS_ALLOCATED, VR::US, 8);
    put_u16(&mut obj, tags::BITS_STORED, VR::US, 8);
    put_u16(&mut obj, tags::HIGH_BIT, VR::US, 7);
    put_u16(&mut obj, tags::PIXEL_REPRESENTATION, VR::US, 0);

    put_lut_sequence(
        &mut obj,
        tags::MODALITY_LUT_SEQUENCE,
        recipe.modality_lut_descriptor,
        "Synthetic CR modality LUT",
        Some(recipe.modality_lut_type),
        recipe.modality_lut_data,
    );
    put_lut_sequence(
        &mut obj,
        tags::VOILUT_SEQUENCE,
        recipe.voi_lut_descriptor,
        "Synthetic CR VOI LUT",
        None,
        recipe.voi_lut_data,
    );

    put_u16(&mut obj, overlay_rows, VR::US, recipe.rows);
    put_u16(&mut obj, overlay_columns, VR::US, recipe.columns);
    put_str(&mut obj, overlay_type, VR::CS, "G");
    obj.put(DataElement::new(
        overlay_origin,
        VR::SS,
        PrimitiveValue::from([1_i16, 1_i16]),
    ));
    put_u16(&mut obj, overlay_bits_allocated, VR::US, 1);
    put_u16(&mut obj, overlay_bit_position, VR::US, 0);
    obj.put(DataElement::new(
        overlay_data,
        VR::OW,
        PrimitiveValue::from(recipe.overlay_bytes),
    ));

    obj.put(DataElement::new(
        tags::PIXEL_DATA,
        VR::OB,
        PrimitiveValue::from(recipe.pixel_bytes),
    ));

    let file_obj = obj
        .with_meta(
            FileMetaTableBuilder::new()
                .transfer_syntax(uids::EXPLICIT_VR_LITTLE_ENDIAN)
                .implementation_class_uid(&implementation_class_uid)
                .implementation_version_name("DICOMTS010"),
        )
        .map_err(|err| GenerateError::WriteDicomFile {
            path: path.clone(),
            message: err.to_string(),
        })?;

    file_obj
        .write_to_file(&path)
        .map_err(|err| GenerateError::WriteDicomFile {
            path: path.clone(),
            message: err.to_string(),
        })?;

    let validated = validate_part10_file(
        &path,
        &Part10Expectations {
            sop_class_uid: uids::COMPUTED_RADIOGRAPHY_IMAGE_STORAGE,
            sop_instance_uid: &sop_instance_uid,
            transfer_syntax_uid: uids::EXPLICIT_VR_LITTLE_ENDIAN,
            implementation_class_uid: &implementation_class_uid,
            synthetic_data: "YES",
            rows: recipe.rows,
            columns: recipe.columns,
            samples_per_pixel: 1,
            photometric_interpretation: "MONOCHROME2",
            bits_allocated: 8,
            bits_stored: 8,
            high_bit: 7,
            pixel_representation: 0,
            planar_configuration: None,
            pixel_data_vr: VR::OB,
            pixel_data_length_formula: PixelDataLengthFormula::ContiguousSamples,
            palette: None,
            padding: None,
            ct_image: None,
            mg_image: None,
            dx_image: None,
            cr_image: Some(CrImageExpectations {
                modality: "CR",
                image_type: "ORIGINAL\\PRIMARY",
                body_part_examined: recipe.body_part_examined,
                view_position: recipe.view_position,
                acquisition_number: "1",
                overlay_rows: recipe.rows,
                overlay_columns: recipe.columns,
                overlay_type: "G",
                overlay_origin: vec![1, 1],
                overlay_bits_allocated: 1,
                overlay_bit_position: 0,
                overlay_data_length: recipe.overlay_bytes.len(),
                modality_lut_descriptor: recipe.modality_lut_descriptor,
                modality_lut_type: recipe.modality_lut_type,
                modality_lut_data_length: recipe.modality_lut_data.len(),
                voi_lut_descriptor: recipe.voi_lut_descriptor,
                voi_lut_data_length: recipe.voi_lut_data.len(),
            }),
            mr_image: None,
        },
    )?;

    Ok(GeneratedFile {
        case_id: recipe.case_id.to_string(),
        manifest_entry: classic_cr_manifest_entry(
            case,
            recipe,
            &relative_path,
            &study_instance_uid,
            &series_instance_uid,
            &sop_instance_uid,
            &implementation_class_uid,
            &validated.bytes,
            validated.validation,
        ),
    })
}

#[allow(clippy::too_many_arguments)]
fn classic_cr_manifest_entry(
    case: &Value,
    recipe: ClassicCrRecipe,
    relative_path: &str,
    study_instance_uid: &str,
    series_instance_uid: &str,
    sop_instance_uid: &str,
    implementation_class_uid: &str,
    bytes: &[u8],
    validation: Value,
) -> Value {
    let mut standards_evidence = case
        .get("standards_evidence")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    standards_evidence.extend([
        serde_json::json!({
            "source": "dicom-standard-kb",
            "edition": "2026b",
            "query": "lookup_iod Computed Radiography Image",
            "covered": true,
            "part": "PS3.3",
            "anchor": "table_A.2-1"
        }),
        serde_json::json!({
            "source": "dicom-standard-kb",
            "edition": "2026b",
            "query": "list_modules_for_iod Computed Radiography Image",
            "covered": true,
            "part": "PS3.3",
            "anchor": "table_A.2-1"
        }),
        serde_json::json!({
            "source": "dicom-standard-kb",
            "edition": "2026b",
            "query": "list_attributes_for_module CR Series --expand-macros",
            "covered": true,
            "part": "PS3.3",
            "anchor": "table_C.8-1"
        }),
        serde_json::json!({
            "source": "dicom-standard-kb",
            "edition": "2026b",
            "query": "list_attributes_for_module CR Image --expand-macros",
            "covered": true,
            "part": "PS3.3",
            "anchor": "table_C.8-2"
        }),
        serde_json::json!({
            "source": "dicom-standard-kb",
            "edition": "2026b",
            "query": "list_attributes_for_module Overlay Plane --expand-macros",
            "covered": true,
            "part": "PS3.3",
            "anchor": "table_C.9-2"
        }),
        serde_json::json!({
            "source": "dicom-standard-kb",
            "edition": "2026b",
            "query": "list_attributes_for_module Modality LUT --expand-macros",
            "covered": true,
            "part": "PS3.3",
            "anchor": "table_C.11-1"
        }),
        serde_json::json!({
            "source": "dicom-standard-kb",
            "edition": "2026b",
            "query": "list_attributes_for_module VOI LUT --expand-macros",
            "covered": true,
            "part": "PS3.3",
            "anchor": "table_C.11-2"
        }),
    ]);

    serde_json::json!({
        "case_id": recipe.case_id,
        "profile_membership": ["core"],
        "path": relative_path,
        "sha256": sha256_hex(bytes),
        "size_bytes": bytes.len(),
        "determinism": "byte_stable",
        "recipe": {
            "recipe_id": recipe.recipe_id,
            "recipe_version": CLASSIC_CR_RECIPE_VERSION,
            "recipe_parameters": {
                "rows": recipe.rows,
                "columns": recipe.columns,
                "samples_per_pixel": 1,
                "photometric_interpretation": "MONOCHROME2",
                "bits_allocated": 8,
                "bits_stored": 8,
                "high_bit": 7,
                "pixel_representation": 0,
                "pixel_values": recipe.pixel_values,
                "overlay": {
                    "rows": recipe.rows,
                    "columns": recipe.columns,
                    "type": "G",
                    "origin": [1, 1],
                    "bits_allocated": 1,
                    "bit_position": 0,
                    "value_length": recipe.overlay_bytes.len()
                },
                "modality_lut": {
                    "descriptor": recipe.modality_lut_descriptor,
                    "type": recipe.modality_lut_type,
                    "data_value_length": recipe.modality_lut_data.len()
                },
                "voi_lut": {
                    "descriptor": recipe.voi_lut_descriptor,
                    "data_value_length": recipe.voi_lut_data.len()
                }
            }
        },
        "dicom": {
            "sop_class_uid": uids::COMPUTED_RADIOGRAPHY_IMAGE_STORAGE,
            "sop_class_name": "Computed Radiography Image Storage",
            "iod_name": "Computed Radiography Image",
            "modality": "CR",
            "transfer_syntax_uid": uids::EXPLICIT_VR_LITTLE_ENDIAN,
            "transfer_syntax_name": "Explicit VR Little Endian"
        },
        "uids": {
            "study_instance_uid": study_instance_uid,
            "series_instance_uid": series_instance_uid,
            "sop_instance_uid": sop_instance_uid,
            "frame_of_reference_uid": Value::Null,
            "implementation_class_uid": implementation_class_uid
        },
        "image": {
            "rows": recipe.rows,
            "columns": recipe.columns,
            "frames": 1,
            "samples_per_pixel": 1,
            "photometric_interpretation": "MONOCHROME2",
            "bits_allocated": 8,
            "bits_stored": 8,
            "high_bit": 7,
            "pixel_representation": 0,
            "planar_configuration": Value::Null
        },
        "pixel_data": {
            "vr": "OB",
            "native_or_encapsulated": "native",
            "value_length": recipe.pixel_bytes.len(),
            "frame_count": 1,
            "frame_hashes": [sha256_hex(recipe.pixel_bytes)]
        },
        "expected_capabilities": ["open_file", "read_metadata", "render_native_pixels", "read_overlay_plane", "apply_modality_lut", "apply_voi_lut"],
        "expected_semantics": {
            "synthetic_data": "YES",
            "image_type": "ORIGINAL\\PRIMARY",
            "pixel_min": recipe.pixel_min,
            "pixel_max": recipe.pixel_max,
            "overlay_pattern": "2x2_diagonal_overlay",
            "modality_lut": "stored values 0..3 map through a four-entry 16-bit Modality LUT",
            "voi_lut": "post-modality values can be windowed through a four-entry 16-bit VOI LUT"
        },
        "expected_visual_checks": {
            "pattern": "2x2_cr_overlay_lut_gradient"
        },
        "validation": validation,
        "known_stressors": ["computed_radiography_image_storage", "overlay_plane", "modality_lut_sequence", "voi_lut_sequence"],
        "standards_evidence": standards_evidence
    })
}

fn write_classic_mr_case(
    run: &PreparedGenerationRun,
    case: &Value,
    recipe: ClassicMrRecipe,
    standards_lock_sha256: &str,
) -> Result<Vec<GeneratedFile>, GenerateError> {
    let study_instance_uid = deterministic_classic_mr_uid(
        standards_lock_sha256,
        recipe,
        run.seed,
        UidRole::StudyInstance,
        0,
    );
    let series_instance_uid = deterministic_classic_mr_uid(
        standards_lock_sha256,
        recipe,
        run.seed,
        UidRole::SeriesInstance,
        0,
    );
    let frame_of_reference_uid = deterministic_classic_mr_uid(
        standards_lock_sha256,
        recipe,
        run.seed,
        UidRole::FrameOfReference,
        0,
    );
    let implementation_class_uid = deterministic_implementation_uid(standards_lock_sha256);

    let mut generated_files = Vec::with_capacity(recipe.slices.len());
    for (slice_index, slice) in recipe.slices.iter().enumerate() {
        let sop_instance_uid = deterministic_classic_mr_uid(
            standards_lock_sha256,
            recipe,
            run.seed,
            UidRole::SopInstance,
            u32::try_from(slice_index).expect("MR slice index must fit in u32"),
        );
        let relative_path = format!(
            "{}/slice-{:03}.dcm",
            recipe.case_id,
            slice_index.saturating_add(1)
        );
        let path = run.out_dir.join(&relative_path);
        let case_dir = path.parent().ok_or_else(|| GenerateError::MetadataShape {
            path: PathBuf::from(&relative_path),
            message: "generated DICOM path must have a parent directory",
        })?;
        fs::create_dir_all(case_dir).map_err(|source| GenerateError::CreateCaseOutputDir {
            path: case_dir.to_path_buf(),
            source,
        })?;

        let mut obj = InMemDicomObject::new_empty();
        put_str(
            &mut obj,
            tags::SOP_CLASS_UID,
            VR::UI,
            uids::MR_IMAGE_STORAGE,
        );
        put_str(&mut obj, tags::SOP_INSTANCE_UID, VR::UI, &sop_instance_uid);
        put_str(&mut obj, tags::SYNTHETIC_DATA, VR::CS, "YES");

        put_str(
            &mut obj,
            tags::PATIENT_NAME,
            VR::PN,
            "DTS^Synthetic^Patient001",
        );
        put_str(&mut obj, tags::PATIENT_ID, VR::LO, "DTS-PATIENT-001");
        put_str(&mut obj, tags::PATIENT_BIRTH_DATE, VR::DA, "19700101");
        put_str(&mut obj, tags::PATIENT_SEX, VR::CS, "O");

        put_str(
            &mut obj,
            tags::STUDY_INSTANCE_UID,
            VR::UI,
            &study_instance_uid,
        );
        put_str(&mut obj, tags::STUDY_DATE, VR::DA, "20260101");
        put_str(&mut obj, tags::STUDY_TIME, VR::TM, "000000");
        put_str(&mut obj, tags::REFERRING_PHYSICIAN_NAME, VR::PN, "");
        put_str(&mut obj, tags::STUDY_ID, VR::SH, "DTS-MR");
        put_str(&mut obj, tags::ACCESSION_NUMBER, VR::SH, "");

        put_str(&mut obj, tags::MODALITY, VR::CS, "MR");
        put_str(
            &mut obj,
            tags::SERIES_INSTANCE_UID,
            VR::UI,
            &series_instance_uid,
        );
        put_str(&mut obj, tags::SERIES_NUMBER, VR::IS, "1");
        put_str(
            &mut obj,
            tags::FRAME_OF_REFERENCE_UID,
            VR::UI,
            &frame_of_reference_uid,
        );
        put_str(&mut obj, tags::POSITION_REFERENCE_INDICATOR, VR::LO, "");

        put_str(&mut obj, tags::MANUFACTURER, VR::LO, "dicom-test-suite");
        put_str(
            &mut obj,
            tags::MANUFACTURER_MODEL_NAME,
            VR::LO,
            recipe.recipe_id,
        );
        put_str(
            &mut obj,
            tags::SOFTWARE_VERSIONS,
            VR::LO,
            crate::PACKAGE_VERSION,
        );

        put_str(&mut obj, tags::ACQUISITION_NUMBER, VR::IS, "1");
        put_str(&mut obj, tags::ACQUISITION_DATE, VR::DA, "20260101");
        put_str(&mut obj, tags::ACQUISITION_TIME, VR::TM, "000000");

        put_str(&mut obj, tags::IMAGE_TYPE, VR::CS, "ORIGINAL\\PRIMARY");
        put_str(
            &mut obj,
            tags::INSTANCE_NUMBER,
            VR::IS,
            slice.instance_number,
        );
        put_str(&mut obj, tags::PATIENT_ORIENTATION, VR::CS, "");
        put_str(&mut obj, tags::CONTENT_DATE, VR::DA, "20260101");
        put_str(&mut obj, tags::CONTENT_TIME, VR::TM, "000000");

        put_str(&mut obj, tags::PIXEL_SPACING, VR::DS, recipe.pixel_spacing);
        put_str(
            &mut obj,
            tags::IMAGE_ORIENTATION_PATIENT,
            VR::DS,
            recipe.image_orientation_patient,
        );
        put_str(
            &mut obj,
            tags::IMAGE_POSITION_PATIENT,
            VR::DS,
            slice.image_position_patient,
        );
        put_str(
            &mut obj,
            tags::SLICE_THICKNESS,
            VR::DS,
            recipe.slice_thickness,
        );
        put_str(
            &mut obj,
            tags::SPACING_BETWEEN_SLICES,
            VR::DS,
            recipe.spacing_between_slices,
        );
        put_str(&mut obj, tags::SLICE_LOCATION, VR::DS, slice.slice_location);

        put_u16(&mut obj, tags::SAMPLES_PER_PIXEL, VR::US, 1);
        put_str(
            &mut obj,
            tags::PHOTOMETRIC_INTERPRETATION,
            VR::CS,
            "MONOCHROME2",
        );
        put_u16(&mut obj, tags::ROWS, VR::US, recipe.rows);
        put_u16(&mut obj, tags::COLUMNS, VR::US, recipe.columns);
        put_u16(&mut obj, tags::BITS_ALLOCATED, VR::US, 16);
        put_u16(&mut obj, tags::BITS_STORED, VR::US, 16);
        put_u16(&mut obj, tags::HIGH_BIT, VR::US, 15);
        put_u16(&mut obj, tags::PIXEL_REPRESENTATION, VR::US, 0);

        put_str(&mut obj, tags::SCANNING_SEQUENCE, VR::CS, "SE");
        put_str(&mut obj, tags::SEQUENCE_VARIANT, VR::CS, "NONE");
        put_str(&mut obj, tags::SCAN_OPTIONS, VR::CS, "");
        put_str(&mut obj, tags::MR_ACQUISITION_TYPE, VR::CS, "2D");
        put_str(&mut obj, tags::REPETITION_TIME, VR::DS, "500");
        put_str(&mut obj, tags::ECHO_TIME, VR::DS, "20");
        put_str(&mut obj, tags::ECHO_TRAIN_LENGTH, VR::IS, "1");
        put_str(&mut obj, tags::MAGNETIC_FIELD_STRENGTH, VR::DS, "1.5");

        obj.put(DataElement::new(
            tags::PIXEL_DATA,
            VR::OW,
            PrimitiveValue::from(slice.pixel_bytes),
        ));

        let file_obj = obj
            .with_meta(
                FileMetaTableBuilder::new()
                    .transfer_syntax(uids::EXPLICIT_VR_LITTLE_ENDIAN)
                    .implementation_class_uid(&implementation_class_uid)
                    .implementation_version_name("DICOMTS010"),
            )
            .map_err(|err| GenerateError::WriteDicomFile {
                path: path.clone(),
                message: err.to_string(),
            })?;

        file_obj
            .write_to_file(&path)
            .map_err(|err| GenerateError::WriteDicomFile {
                path: path.clone(),
                message: err.to_string(),
            })?;

        let validated = validate_part10_file(
            &path,
            &Part10Expectations {
                sop_class_uid: uids::MR_IMAGE_STORAGE,
                sop_instance_uid: &sop_instance_uid,
                transfer_syntax_uid: uids::EXPLICIT_VR_LITTLE_ENDIAN,
                implementation_class_uid: &implementation_class_uid,
                synthetic_data: "YES",
                rows: recipe.rows,
                columns: recipe.columns,
                samples_per_pixel: 1,
                photometric_interpretation: "MONOCHROME2",
                bits_allocated: 16,
                bits_stored: 16,
                high_bit: 15,
                pixel_representation: 0,
                planar_configuration: None,
                pixel_data_vr: VR::OW,
                pixel_data_length_formula: PixelDataLengthFormula::ContiguousSamples,
                palette: None,
                padding: None,
                ct_image: None,
                mg_image: None,
                dx_image: None,
                cr_image: None,
                mr_image: Some(MrImageExpectations {
                    modality: "MR",
                    frame_of_reference_uid: &frame_of_reference_uid,
                    image_type: "ORIGINAL\\PRIMARY",
                    instance_number: slice.instance_number,
                    acquisition_number: "1",
                    pixel_spacing: recipe.pixel_spacing,
                    image_orientation_patient: recipe.image_orientation_patient,
                    image_position_patient: slice.image_position_patient,
                    slice_thickness: recipe.slice_thickness,
                    spacing_between_slices: recipe.spacing_between_slices,
                    slice_location: slice.slice_location,
                    scanning_sequence: "SE",
                    sequence_variant: "NONE",
                    scan_options: "",
                    mr_acquisition_type: "2D",
                    repetition_time: "500",
                    echo_time: "20",
                    echo_train_length: "1",
                    magnetic_field_strength: "1.5",
                    slice_order_index: slice_index + 1,
                    slice_count: recipe.slices.len(),
                    position_along_normal: slice.position_along_normal,
                }),
            },
        )?;

        generated_files.push(GeneratedFile {
            case_id: recipe.case_id.to_string(),
            manifest_entry: classic_mr_manifest_entry(
                case,
                recipe,
                *slice,
                &relative_path,
                &study_instance_uid,
                &series_instance_uid,
                &sop_instance_uid,
                &frame_of_reference_uid,
                &implementation_class_uid,
                &validated.bytes,
                validated.validation,
                slice_index,
            ),
        });
    }

    Ok(generated_files)
}

#[allow(clippy::too_many_arguments)]
fn classic_mr_manifest_entry(
    case: &Value,
    recipe: ClassicMrRecipe,
    slice: ClassicMrSliceRecipe,
    relative_path: &str,
    study_instance_uid: &str,
    series_instance_uid: &str,
    sop_instance_uid: &str,
    frame_of_reference_uid: &str,
    implementation_class_uid: &str,
    bytes: &[u8],
    validation: Value,
    slice_index: usize,
) -> Value {
    let mut standards_evidence = case
        .get("standards_evidence")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    standards_evidence.extend([
        serde_json::json!({
            "source": "dicom-standard-kb",
            "edition": "2026b",
            "query": "lookup_iod MR Image",
            "covered": true,
            "part": "PS3.3",
            "anchor": "table_A.4-1"
        }),
        serde_json::json!({
            "source": "dicom-standard-kb",
            "edition": "2026b",
            "query": "list_modules_for_iod MR Image",
            "covered": true,
            "part": "PS3.3",
            "anchor": "table_A.4-1"
        }),
        serde_json::json!({
            "source": "dicom-standard-kb",
            "edition": "2026b",
            "query": "list_attributes_for_module MR Image --expand-macros",
            "covered": true,
            "part": "PS3.3",
            "anchor": "table_C.8-4"
        }),
        serde_json::json!({
            "source": "dicom-standard-kb",
            "edition": "2026b",
            "query": "list_attributes_for_module Image Plane --expand-macros",
            "covered": true,
            "part": "PS3.3",
            "anchor": "table_C.7-10"
        }),
        serde_json::json!({
            "source": "dicom-standard-kb",
            "edition": "2026b",
            "query": "list_attributes_for_module Frame of Reference --expand-macros",
            "covered": true,
            "part": "PS3.3",
            "anchor": "table_C.7-6"
        }),
        serde_json::json!({
            "source": "dicom-standard-kb",
            "edition": "2026b",
            "query": "lookup_data_element ScanningSequence",
            "covered": true,
            "part": "PS3.6",
            "anchor": "table_6-1"
        }),
        serde_json::json!({
            "source": "dicom-standard-kb",
            "edition": "2026b",
            "query": "lookup_data_element SequenceVariant",
            "covered": true,
            "part": "PS3.6",
            "anchor": "table_6-1"
        }),
        serde_json::json!({
            "source": "dicom-standard-kb",
            "edition": "2026b",
            "query": "lookup_data_element MRAcquisitionType",
            "covered": true,
            "part": "PS3.6",
            "anchor": "table_6-1"
        }),
        serde_json::json!({
            "source": "dicom-standard-kb",
            "edition": "2026b",
            "query": "lookup_data_element SpacingBetweenSlices",
            "covered": true,
            "part": "PS3.6",
            "anchor": "table_6-1"
        }),
    ]);

    serde_json::json!({
        "case_id": recipe.case_id,
        "profile_membership": ["core"],
        "path": relative_path,
        "sha256": sha256_hex(bytes),
        "size_bytes": bytes.len(),
        "determinism": "byte_stable",
        "recipe": {
            "recipe_id": recipe.recipe_id,
            "recipe_version": CLASSIC_MR_RECIPE_VERSION,
            "recipe_parameters": {
                "rows": recipe.rows,
                "columns": recipe.columns,
                "samples_per_pixel": 1,
                "photometric_interpretation": "MONOCHROME2",
                "bits_allocated": 16,
                "bits_stored": 16,
                "high_bit": 15,
                "pixel_representation": 0,
                "pixel_values": slice.pixel_values,
                "geometry": {
                    "pixel_spacing": recipe.pixel_spacing,
                    "image_orientation_patient": recipe.image_orientation_patient,
                    "image_position_patient": slice.image_position_patient,
                    "slice_thickness": recipe.slice_thickness,
                    "spacing_between_slices": recipe.spacing_between_slices,
                    "slice_location": slice.slice_location,
                    "position_along_normal": slice.position_along_normal,
                    "slice_order_index": slice_index + 1,
                    "slice_count": recipe.slices.len()
                },
                "mr": {
                    "scanning_sequence": "SE",
                    "sequence_variant": "NONE",
                    "scan_options": "",
                    "mr_acquisition_type": "2D",
                    "repetition_time": "500",
                    "echo_time": "20",
                    "echo_train_length": "1",
                    "magnetic_field_strength": "1.5"
                }
            }
        },
        "dicom": {
            "sop_class_uid": uids::MR_IMAGE_STORAGE,
            "sop_class_name": "MR Image Storage",
            "iod_name": "MR Image",
            "modality": "MR",
            "transfer_syntax_uid": uids::EXPLICIT_VR_LITTLE_ENDIAN,
            "transfer_syntax_name": "Explicit VR Little Endian"
        },
        "uids": {
            "study_instance_uid": study_instance_uid,
            "series_instance_uid": series_instance_uid,
            "sop_instance_uid": sop_instance_uid,
            "frame_of_reference_uid": frame_of_reference_uid,
            "implementation_class_uid": implementation_class_uid
        },
        "image": {
            "rows": recipe.rows,
            "columns": recipe.columns,
            "frames": 1,
            "samples_per_pixel": 1,
            "photometric_interpretation": "MONOCHROME2",
            "bits_allocated": 16,
            "bits_stored": 16,
            "high_bit": 15,
            "pixel_representation": 0,
            "planar_configuration": Value::Null
        },
        "pixel_data": {
            "vr": "OW",
            "native_or_encapsulated": "native",
            "value_length": slice.pixel_bytes.len(),
            "frame_count": 1,
            "frame_hashes": [sha256_hex(slice.pixel_bytes)]
        },
        "expected_capabilities": ["open_file", "read_metadata", "render_native_pixels", "sort_series_by_geometry"],
        "expected_semantics": {
            "synthetic_data": "YES",
            "image_type": "ORIGINAL\\PRIMARY",
            "pixel_min": slice.pixel_min,
            "pixel_max": slice.pixel_max,
            "series_instance_count": recipe.slices.len(),
            "shared_study_series_frame_of_reference": true,
            "geometry_sort_key": {
                "image_orientation_patient": recipe.image_orientation_patient,
                "position_along_normal": slice.position_along_normal,
                "slice_order_index": slice_index + 1
            }
        },
        "expected_visual_checks": {
            "pattern": "3_slice_oblique_mr_gradient_stack"
        },
        "validation": validation,
        "known_stressors": ["mr_image_storage", "multi_instance_series", "oblique_image_orientation_patient", "geometry_slice_sorting"],
        "standards_evidence": standards_evidence
    })
}

fn deterministic_case_uid(
    standards_lock_sha256: &str,
    recipe: PixelRecipe,
    run_seed: u64,
    role: UidRole,
) -> String {
    deterministic_uid(&DeterministicUidInput {
        standards_lock_sha256,
        case_id: recipe.case_id,
        recipe_version: PIXEL_RECIPE_VERSION,
        run_seed,
        file_index: 0,
        frame_index: None,
        referenced_object_index: None,
        role,
    })
}

fn deterministic_classic_ct_uid(
    standards_lock_sha256: &str,
    recipe: ClassicCtRecipe,
    run_seed: u64,
    role: UidRole,
) -> String {
    deterministic_uid(&DeterministicUidInput {
        standards_lock_sha256,
        case_id: recipe.case_id,
        recipe_version: CLASSIC_CT_RECIPE_VERSION,
        run_seed,
        file_index: 0,
        frame_index: None,
        referenced_object_index: None,
        role,
    })
}

fn deterministic_classic_mg_uid(
    standards_lock_sha256: &str,
    recipe: ClassicMgRecipe,
    run_seed: u64,
    role: UidRole,
) -> String {
    deterministic_uid(&DeterministicUidInput {
        standards_lock_sha256,
        case_id: recipe.case_id,
        recipe_version: CLASSIC_MG_RECIPE_VERSION,
        run_seed,
        file_index: 0,
        frame_index: None,
        referenced_object_index: None,
        role,
    })
}

fn deterministic_classic_dx_uid(
    standards_lock_sha256: &str,
    recipe: ClassicDxRecipe,
    run_seed: u64,
    role: UidRole,
) -> String {
    deterministic_uid(&DeterministicUidInput {
        standards_lock_sha256,
        case_id: recipe.case_id,
        recipe_version: CLASSIC_DX_RECIPE_VERSION,
        run_seed,
        file_index: 0,
        frame_index: None,
        referenced_object_index: None,
        role,
    })
}

fn deterministic_classic_cr_uid(
    standards_lock_sha256: &str,
    recipe: ClassicCrRecipe,
    run_seed: u64,
    role: UidRole,
) -> String {
    deterministic_uid(&DeterministicUidInput {
        standards_lock_sha256,
        case_id: recipe.case_id,
        recipe_version: CLASSIC_CR_RECIPE_VERSION,
        run_seed,
        file_index: 0,
        frame_index: None,
        referenced_object_index: None,
        role,
    })
}

fn deterministic_classic_mr_uid(
    standards_lock_sha256: &str,
    recipe: ClassicMrRecipe,
    run_seed: u64,
    role: UidRole,
    file_index: u32,
) -> String {
    deterministic_uid(&DeterministicUidInput {
        standards_lock_sha256,
        case_id: recipe.case_id,
        recipe_version: CLASSIC_MR_RECIPE_VERSION,
        run_seed,
        file_index,
        frame_index: None,
        referenced_object_index: None,
        role,
    })
}

fn deterministic_implementation_uid(standards_lock_sha256: &str) -> String {
    deterministic_uid(&DeterministicUidInput {
        standards_lock_sha256,
        case_id: "dicom-test-suite/implementation",
        recipe_version: crate::PACKAGE_VERSION,
        run_seed: 0,
        file_index: 0,
        frame_index: None,
        referenced_object_index: None,
        role: UidRole::ImplementationClass,
    })
}

fn pixel_vr_name(vr: VR) -> &'static str {
    match vr {
        VR::OB => "OB",
        VR::OW => "OW",
        _ => "UN",
    }
}

fn pixel_data_length_formula(recipe: PixelRecipe) -> PixelDataLengthFormula {
    match recipe.photometric_interpretation {
        "YBR_FULL_422" => PixelDataLengthFormula::YbrFull422,
        _ => PixelDataLengthFormula::ContiguousSamples,
    }
}

fn put_str(obj: &mut InMemDicomObject, tag: dicom_core::Tag, vr: VR, value: &str) {
    obj.put(DataElement::new(tag, vr, value));
}

fn put_u16(obj: &mut InMemDicomObject, tag: dicom_core::Tag, vr: VR, value: u16) {
    obj.put(DataElement::new(tag, vr, PrimitiveValue::from(value)));
}

fn put_i16(obj: &mut InMemDicomObject, tag: dicom_core::Tag, vr: VR, value: i16) {
    obj.put(DataElement::new(tag, vr, PrimitiveValue::from(value)));
}

fn put_empty_sequence(obj: &mut InMemDicomObject, tag: dicom_core::Tag) {
    obj.put(DataElement::new(tag, VR::SQ, DataSetSequence::empty()));
}

fn put_code_sequence(
    obj: &mut InMemDicomObject,
    tag: dicom_core::Tag,
    code_value: &str,
    coding_scheme: &str,
    code_meaning: &str,
) {
    obj.put(DataElement::new(
        tag,
        VR::SQ,
        DataSetSequence::from(vec![InMemDicomObject::from_element_iter([
            DataElement::new(tags::CODE_VALUE, VR::SH, code_value),
            DataElement::new(tags::CODING_SCHEME_DESIGNATOR, VR::SH, coding_scheme),
            DataElement::new(tags::CODE_MEANING, VR::LO, code_meaning),
        ])]),
    ));
}

fn put_lut_sequence(
    obj: &mut InMemDicomObject,
    sequence_tag: Tag,
    descriptor: [u16; 3],
    explanation: &str,
    modality_lut_type: Option<&str>,
    data: &[u8],
) {
    let mut elements = vec![
        DataElement::new(
            tags::LUT_DESCRIPTOR,
            VR::US,
            PrimitiveValue::from(descriptor),
        ),
        DataElement::new(tags::LUT_EXPLANATION, VR::LO, explanation),
        DataElement::new(tags::LUT_DATA, VR::OW, PrimitiveValue::from(data)),
    ];
    if let Some(modality_lut_type) = modality_lut_type {
        elements.push(DataElement::new(
            tags::MODALITY_LUT_TYPE,
            VR::LO,
            modality_lut_type,
        ));
    }

    obj.put(DataElement::new(
        sequence_tag,
        VR::SQ,
        DataSetSequence::from(vec![InMemDicomObject::from_element_iter(elements)]),
    ));
}

fn put_palette(obj: &mut InMemDicomObject, palette: PaletteRecipe) {
    for tag in [
        tags::RED_PALETTE_COLOR_LOOKUP_TABLE_DESCRIPTOR,
        tags::GREEN_PALETTE_COLOR_LOOKUP_TABLE_DESCRIPTOR,
        tags::BLUE_PALETTE_COLOR_LOOKUP_TABLE_DESCRIPTOR,
    ] {
        obj.put(DataElement::new(
            tag,
            VR::US,
            PrimitiveValue::from(palette.descriptor),
        ));
    }
    obj.put(DataElement::new(
        tags::RED_PALETTE_COLOR_LOOKUP_TABLE_DATA,
        VR::OW,
        PrimitiveValue::from(palette.red_data),
    ));
    obj.put(DataElement::new(
        tags::GREEN_PALETTE_COLOR_LOOKUP_TABLE_DATA,
        VR::OW,
        PrimitiveValue::from(palette.green_data),
    ));
    obj.put(DataElement::new(
        tags::BLUE_PALETTE_COLOR_LOOKUP_TABLE_DATA,
        VR::OW,
        PrimitiveValue::from(palette.blue_data),
    ));
}

impl From<PaletteRecipe> for crate::validation::PaletteExpectations {
    fn from(palette: PaletteRecipe) -> Self {
        Self {
            descriptor: palette.descriptor,
            red_data_length: palette.red_data.len(),
            green_data_length: palette.green_data.len(),
            blue_data_length: palette.blue_data.len(),
        }
    }
}

impl From<PixelPaddingRecipe> for crate::validation::PixelPaddingExpectations {
    fn from(padding: PixelPaddingRecipe) -> Self {
        Self {
            value: padding.value,
            range_limit: padding.range_limit,
        }
    }
}

fn registry_case<'a>(
    registry: &'a Value,
    case_id: &str,
) -> Result<Option<&'a Value>, GenerateError> {
    let cases =
        registry
            .get("cases")
            .and_then(Value::as_array)
            .ok_or(GenerateError::MetadataShape {
                path: PathBuf::from("cases/registry.json"),
                message: "missing cases array",
            })?;
    Ok(cases
        .iter()
        .find(|case| case.get("case_id").and_then(Value::as_str) == Some(case_id)))
}

fn string_array(value: Option<&Value>) -> Result<Vec<String>, GenerateError> {
    let values = value
        .and_then(Value::as_array)
        .ok_or(GenerateError::MetadataShape {
            path: PathBuf::from("cases/registry.json"),
            message: "case profiles must be a string array",
        })?;
    values
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(ToOwned::to_owned)
                .ok_or(GenerateError::MetadataShape {
                    path: PathBuf::from("cases/registry.json"),
                    message: "case profiles must be a string array",
                })
        })
        .collect()
}

fn case_matches_profile(profiles: &[String], requested: &str, include_stress: bool) -> bool {
    match requested {
        "all" => profiles.iter().any(|profile| {
            matches!(profile.as_str(), "smoke" | "core" | "extended" | "legacy")
                || (include_stress && profile == "stress")
        }),
        profile => profiles.iter().any(|case_profile| case_profile == profile),
    }
}
