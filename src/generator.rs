use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;

use dicom_core::{
    DataElement, PrimitiveValue, Tag, VR,
    value::{DataSetSequence, PixelFragmentSequence},
};
use dicom_dictionary_std::{tags, uids};
use dicom_object::{FileMetaTableBuilder, InMemDicomObject};
use serde_json::Value;

use crate::{
    DeterministicUidInput, GenerateError, PreparedGenerationRun, UidRole,
    codecs::{
        DEFLATED_IMAGE_FRAME_TRANSFER_SYNTAX_UID, FrameEncodeInput, FrameEncoder,
        HTJ2K_LOSSLESS_TRANSFER_SYNTAX_UID, JPEG_2000_LOSSLESS_TRANSFER_SYNTAX_UID,
        JPEG_BASELINE_8BIT_TRANSFER_SYNTAX_UID, JPEG_LOSSLESS_PROCESS_14_TRANSFER_SYNTAX_UID,
        JPEG_LOSSLESS_SV1_TRANSFER_SYNTAX_UID, JPEG_LS_LOSSLESS_TRANSFER_SYNTAX_UID,
        JPEG_XL_LOSSLESS_TRANSFER_SYNTAX_UID, NativeRleLosslessEncoder,
        RLE_LOSSLESS_TRANSFER_SYNTAX_UID,
    },
    deterministic_uid,
    encapsulation::{BasicOffsetTablePolicy, EncapsulatedPixelData},
    sha256_hex,
    validation::{
        BasicTextSrExpectations, CrImageExpectations, CtImageExpectations, DxImageExpectations,
        EncapsulatedPdfExpectations, EnhancedCtConcatenationExpectations,
        EnhancedCtImageExpectations, EnhancedMrImageExpectations, MgImageExpectations,
        MrImageExpectations, Part10Expectations, PixelDataLengthFormula,
        PresentationStateExpectations, RealWorldValueMappingExpectations, RtDoseExpectations,
        RtStructureSetExpectations, SegmentationExpectations, UsImageExpectations,
        validate_basic_text_sr_file, validate_comprehensive_sr_file,
        validate_encapsulated_pdf_file, validate_key_object_selection_file, validate_part10_file,
        validate_presentation_state_file, validate_real_world_value_mapping_file,
        validate_rt_dose_file, validate_rt_structure_set_file,
    },
};

#[cfg(feature = "jpeg")]
use crate::encapsulation::encapsulate_frames;

#[cfg(feature = "deflate")]
use crate::codecs::DicomRsDeflatedImageFrameEncoder;
#[cfg(feature = "jpeg")]
use crate::codecs::DicomRsJpegBaselineEncoder;
#[cfg(feature = "charls")]
use crate::codecs::DicomRsJpegLsLosslessEncoder;
#[cfg(feature = "jpegxl")]
use crate::codecs::DicomRsJpegXlLosslessEncoder;
#[cfg(feature = "jpeg2000")]
use crate::codecs::OpenJp2Jpeg2000LosslessEncoder;
#[cfg(feature = "htj2k_openjph")]
use crate::codecs::OpenJphHtj2kLosslessEncoder;
#[cfg(feature = "legacy_jpeg_dcmtk")]
use crate::codecs::{DcmtkDcmcjpegLosslessProcess, DcmtkDcmcjpegLosslessSv1Encoder};
#[cfg(any(
    feature = "charls",
    feature = "deflate",
    feature = "htj2k_openjph",
    feature = "jpeg",
    feature = "jpegxl",
    feature = "jpeg2000"
))]
use crate::codecs::{FrameDecodeInput, FrameDecoder};
#[cfg(feature = "legacy_jpeg_dcmtk")]
use dicom_core::value::Value as DicomValue;
#[cfg(feature = "legacy_jpeg_dcmtk")]
use dicom_encoding::{Codec, adapters::PixelDataReader};
#[cfg(feature = "legacy_jpeg_dcmtk")]
use dicom_object::open_file;
#[cfg(feature = "legacy_jpeg_dcmtk")]
use dicom_transfer_syntax_registry::entries::{
    JPEG_LOSSLESS_NON_HIERARCHICAL, JPEG_LOSSLESS_NON_HIERARCHICAL_FIRST_ORDER_PREDICTION,
};

const PIXEL_RECIPE_VERSION: &str = "0.1.0";
const CLASSIC_CT_RECIPE_VERSION: &str = "0.1.0";
const ENHANCED_CT_RECIPE_VERSION: &str = "0.1.0";
const ENHANCED_MR_RECIPE_VERSION: &str = "0.1.0";
const CLASSIC_MG_RECIPE_VERSION: &str = "0.1.0";
const CLASSIC_DX_RECIPE_VERSION: &str = "0.1.0";
const CLASSIC_US_RECIPE_VERSION: &str = "0.1.0";
const CLASSIC_CR_RECIPE_VERSION: &str = "0.1.0";
const CLASSIC_MR_RECIPE_VERSION: &str = "0.1.0";
const SEGMENTATION_RECIPE_VERSION: &str = "0.1.0";
const GSPS_RECIPE_VERSION: &str = "0.1.0";
const RWVM_RECIPE_VERSION: &str = "0.1.0";
const BASIC_TEXT_SR_RECIPE_VERSION: &str = "0.1.0";
const COMPREHENSIVE_SR_RECIPE_VERSION: &str = "0.1.0";
const KEY_OBJECT_SELECTION_RECIPE_VERSION: &str = "0.2.0";
const RT_STRUCTURE_SET_RECIPE_VERSION: &str = "0.1.0";
const RT_DOSE_RECIPE_VERSION: &str = "0.1.0";
const ENCAPSULATED_PDF_RECIPE_VERSION: &str = "0.1.0";
const SEGMENTATION_STORAGE_UID: &str = "1.2.840.10008.5.1.4.1.1.66.4";
const LABEL_MAP_SEGMENTATION_STORAGE_UID: &str = "1.2.840.10008.5.1.4.1.1.66.7";
const GRAYSCALE_SOFTCOPY_PRESENTATION_STATE_STORAGE_UID: &str = "1.2.840.10008.5.1.4.1.1.11.1";
const REAL_WORLD_VALUE_MAPPING_STORAGE_UID: &str = "1.2.840.10008.5.1.4.1.1.67";
const BASIC_TEXT_SR_STORAGE_UID: &str = "1.2.840.10008.5.1.4.1.1.88.11";
const COMPREHENSIVE_SR_STORAGE_UID: &str = "1.2.840.10008.5.1.4.1.1.88.33";
const KEY_OBJECT_SELECTION_DOCUMENT_STORAGE_UID: &str = "1.2.840.10008.5.1.4.1.1.88.59";
const RT_STRUCTURE_SET_STORAGE_UID: &str = "1.2.840.10008.5.1.4.1.1.481.3";
const RT_DOSE_STORAGE_UID: &str = "1.2.840.10008.5.1.4.1.1.481.2";
const ENCAPSULATED_PDF_STORAGE_UID: &str = "1.2.840.10008.5.1.4.1.1.104.1";
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TransferSyntaxSpec {
    capability_keyword: &'static str,
    capability_name: &'static str,
    uid: &'static str,
    name: &'static str,
}

const IMPLICIT_VR_LITTLE_ENDIAN: TransferSyntaxSpec = TransferSyntaxSpec {
    capability_keyword: "ImplicitVRLittleEndian",
    capability_name: "Implicit VR Little Endian: Default Transfer Syntax for DICOM",
    uid: uids::IMPLICIT_VR_LITTLE_ENDIAN,
    name: "Implicit VR Little Endian",
};
const EXPLICIT_VR_LITTLE_ENDIAN: TransferSyntaxSpec = TransferSyntaxSpec {
    capability_keyword: "ExplicitVRLittleEndian",
    capability_name: "Explicit VR Little Endian",
    uid: uids::EXPLICIT_VR_LITTLE_ENDIAN,
    name: "Explicit VR Little Endian",
};
const EXPLICIT_VR_BIG_ENDIAN: TransferSyntaxSpec = TransferSyntaxSpec {
    capability_keyword: "ExplicitVRBigEndian",
    capability_name: "Explicit VR Big Endian",
    uid: "1.2.840.10008.1.2.2",
    name: "Explicit VR Big Endian",
};
const DEFLATED_EXPLICIT_VR_LITTLE_ENDIAN: TransferSyntaxSpec = TransferSyntaxSpec {
    capability_keyword: "DeflatedExplicitVRLittleEndian",
    capability_name: "Deflated Explicit VR Little Endian",
    uid: uids::DEFLATED_EXPLICIT_VR_LITTLE_ENDIAN,
    name: "Deflated Explicit VR Little Endian",
};
const RLE_LOSSLESS: TransferSyntaxSpec = TransferSyntaxSpec {
    capability_keyword: "RLELossless",
    capability_name: "RLE Lossless",
    uid: RLE_LOSSLESS_TRANSFER_SYNTAX_UID,
    name: "RLE Lossless",
};
const JPEG_BASELINE_8BIT: TransferSyntaxSpec = TransferSyntaxSpec {
    capability_keyword: "JPEGBaseline8Bit",
    capability_name: "JPEG Baseline (Process 1): Default Transfer Syntax for Lossy JPEG 8 Bit Image Compression",
    uid: JPEG_BASELINE_8BIT_TRANSFER_SYNTAX_UID,
    name: "JPEG Baseline (Process 1)",
};
const JPEG_LS_LOSSLESS: TransferSyntaxSpec = TransferSyntaxSpec {
    capability_keyword: "JPEGLSLossless",
    capability_name: "JPEG-LS Lossless Image Compression",
    uid: JPEG_LS_LOSSLESS_TRANSFER_SYNTAX_UID,
    name: "JPEG-LS Lossless",
};
const JPEG_XL_LOSSLESS: TransferSyntaxSpec = TransferSyntaxSpec {
    capability_keyword: "JPEGXLLossless",
    capability_name: "JPEG XL Lossless",
    uid: JPEG_XL_LOSSLESS_TRANSFER_SYNTAX_UID,
    name: "JPEG XL Lossless",
};
const JPEG_2000_LOSSLESS: TransferSyntaxSpec = TransferSyntaxSpec {
    capability_keyword: "JPEG2000Lossless",
    capability_name: "JPEG 2000 Image Compression (Lossless Only)",
    uid: JPEG_2000_LOSSLESS_TRANSFER_SYNTAX_UID,
    name: "JPEG 2000 Lossless",
};
const HTJ2K_LOSSLESS: TransferSyntaxSpec = TransferSyntaxSpec {
    capability_keyword: "HTJ2KLossless",
    capability_name: "High-Throughput JPEG 2000 Image Compression (Lossless Only)",
    uid: HTJ2K_LOSSLESS_TRANSFER_SYNTAX_UID,
    name: "HTJ2K Lossless",
};
const JPEG_LOSSLESS_PROCESS_14: TransferSyntaxSpec = TransferSyntaxSpec {
    capability_keyword: "JPEGLossless",
    capability_name: "JPEG Lossless, Non-Hierarchical (Process 14)",
    uid: JPEG_LOSSLESS_PROCESS_14_TRANSFER_SYNTAX_UID,
    name: "JPEG Lossless Process 14",
};
const JPEG_LOSSLESS_SV1: TransferSyntaxSpec = TransferSyntaxSpec {
    capability_keyword: "JPEGLosslessSV1",
    capability_name: "JPEG Lossless, Non-Hierarchical, First-Order Prediction (Process 14 [Selection Value 1])",
    uid: JPEG_LOSSLESS_SV1_TRANSFER_SYNTAX_UID,
    name: "JPEG Lossless SV1",
};
const DEFLATED_IMAGE_FRAME: TransferSyntaxSpec = TransferSyntaxSpec {
    capability_keyword: "DeflatedImageFrameCompression",
    capability_name: "Deflated Image Frame Compression",
    uid: DEFLATED_IMAGE_FRAME_TRANSFER_SYNTAX_UID,
    name: "Deflated Image Frame Compression",
};
const SEGMENTATION_SOURCE_CASE_ID: &str = "enhanced/ct/multiframe_shared_perframe_explicit_le";
const GSPS_SOURCE_CASE_ID: &str = "enhanced/ct/multiframe_shared_perframe_explicit_le";
const RWVM_SOURCE_CASE_ID: &str = "enhanced/ct/multiframe_shared_perframe_explicit_le";
const BASIC_TEXT_SR_SOURCE_CASE_ID: &str = "enhanced/ct/multiframe_shared_perframe_explicit_le";
const COMPREHENSIVE_SR_SOURCE_CASE_ID: &str = "enhanced/ct/multiframe_shared_perframe_explicit_le";
const KEY_OBJECT_SELECTION_IMAGE_SOURCE_CASE_ID: &str =
    "enhanced/ct/multiframe_shared_perframe_explicit_le";
const KEY_OBJECT_SELECTION_SEG_SOURCE_CASE_ID: &str = "derived/seg/binary_multiframe_explicit_le";
const RT_STRUCTURE_SET_SOURCE_CASE_ID: &str = "enhanced/ct/multiframe_shared_perframe_explicit_le";
const RT_DOSE_IMAGE_SOURCE_CASE_ID: &str = "enhanced/ct/multiframe_shared_perframe_explicit_le";
const RT_DOSE_STRUCTURE_SET_SOURCE_CASE_ID: &str =
    "non-image/rt/structure_set_single_roi_explicit_le";
const MONO_PIXELS: [u8; 4] = [0, 85, 170, 255];
const MONO_MULTIFRAME_PIXELS: [u8; 8] = [0, 85, 170, 255, 255, 170, 85, 0];
const MONO_MULTIFRAME_VALUES: [i32; 8] = [0, 85, 170, 255, 255, 170, 85, 0];
const MONO_ODD_RLE_PIXELS: [u8; 2] = [0, 255];
const MONO_ODD_RLE_VALUES: [i32; 2] = [0, 255];
const RGB_PLANAR0_PIXELS: [u8; 12] = [255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255];
const RGB_PLANAR0_MULTIFRAME_PIXELS: [u8; 24] = [
    255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255, 0, 255, 255, 255, 0, 255, 255, 255, 0, 0, 0, 0,
];
const RGB_PLANAR0_MULTIFRAME_VALUES: [i32; 24] = [
    255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255, 0, 255, 255, 255, 0, 255, 255, 255, 0, 0, 0, 0,
];
const RGB_PLANAR1_PIXELS: [u8; 12] = [255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255];
const RGB_PLANAR1_MULTIFRAME_PIXELS: [u8; 24] = [
    255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 0, 255, 255, 0, 255, 0, 255, 0, 255, 255, 0, 0,
];
const RGB_PLANAR1_MULTIFRAME_VALUES: [i32; 24] = [
    255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 0, 255, 255, 0, 255, 0, 255, 0, 255, 255, 0, 0,
];
const MONO_U16_PIXELS: [u8; 8] = [0, 0, 0x55, 0x55, 0xaa, 0xaa, 0xff, 0xff];
const MONO_U16_VALUES: [i32; 4] = [0, 21845, 43690, 65535];
const MONO_I16_PIXELS: [u8; 8] = [0x00, 0x80, 0x55, 0xd5, 0xaa, 0x2a, 0xff, 0x7f];
const MONO_I16_VALUES: [i32; 4] = [-32768, -10923, 10922, 32767];
const MONO_I16_MULTIFRAME_PIXELS: [u8; 16] = [
    0x00, 0x80, 0x55, 0xd5, 0xaa, 0x2a, 0xff, 0x7f, 0xff, 0x7f, 0xaa, 0x2a, 0x55, 0xd5, 0x00, 0x80,
];
const MONO_I16_MULTIFRAME_VALUES: [i32; 8] =
    [-32768, -10923, 10922, 32767, 32767, 10922, -10923, -32768];
const MONO_U16_ODD_3X3_PIXELS: [u8; 18] = [0, 0, 1, 0, 2, 0, 3, 0, 4, 0, 5, 0, 6, 0, 7, 0, 8, 0];
const MONO_U16_ODD_3X3_VALUES: [i32; 9] = [0, 1, 2, 3, 4, 5, 6, 7, 8];
const MONO_I16_ODD_3X3_PIXELS: [u8; 18] = [
    0xfc, 0xff, 0xfd, 0xff, 0xfe, 0xff, 0xff, 0xff, 0, 0, 1, 0, 2, 0, 3, 0, 4, 0,
];
const MONO_I16_ODD_3X3_VALUES: [i32; 9] = [-4, -3, -2, -1, 0, 1, 2, 3, 4];
const MONO_U16_RECT_2X3_PIXELS: [u8; 12] = [0, 0, 1, 0, 2, 0, 3, 0, 4, 0, 5, 0];
const MONO_U16_RECT_2X3_VALUES: [i32; 6] = [0, 1, 2, 3, 4, 5];
const MONO_I16_RECT_2X3_PIXELS: [u8; 12] = [0xfd, 0xff, 0xfe, 0xff, 0xff, 0xff, 0, 0, 1, 0, 2, 0];
const MONO_I16_RECT_2X3_VALUES: [i32; 6] = [-3, -2, -1, 0, 1, 2];
const MONO_U16_TINY_1X1_PIXELS: [u8; 2] = [0xff, 0xff];
const MONO_U16_TINY_1X1_VALUES: [i32; 1] = [65535];
const MONO_I16_TINY_1X1_PIXELS: [u8; 2] = [0x00, 0x80];
const MONO_I16_TINY_1X1_VALUES: [i32; 1] = [-32768];
const MONO_U16_MULTIFRAME_PIXELS: [u8; 16] = [
    0, 0, 0x55, 0x55, 0xaa, 0xaa, 0xff, 0xff, 0xff, 0xff, 0xaa, 0xaa, 0x55, 0x55, 0, 0,
];
const MONO_U16_MULTIFRAME_VALUES: [i32; 8] = [0, 21845, 43690, 65535, 65535, 43690, 21845, 0];
const MONO_U16_PADDING_PIXELS: [u8; 8] = [0, 0, 0xe8, 0x03, 0xd0, 0x07, 0xb8, 0x0b];
const MONO_U16_PADDING_VALUES: [i32; 4] = [0, 1000, 2000, 3000];
const MONO_U16_PADDING_MULTIFRAME_PIXELS: [u8; 16] = [
    0, 0, 0xe8, 0x03, 0xd0, 0x07, 0xb8, 0x0b, 0xb8, 0x0b, 0xd0, 0x07, 0xe8, 0x03, 0, 0,
];
const MONO_U16_PADDING_MULTIFRAME_VALUES: [i32; 8] = [0, 1000, 2000, 3000, 3000, 2000, 1000, 0];
const MONO_I16_PADDING_PIXELS: [u8; 8] = [0x00, 0x80, 0x18, 0xfc, 0xe8, 0x03, 0xb8, 0x0b];
const MONO_I16_PADDING_VALUES: [i32; 4] = [-32768, -1000, 1000, 3000];
const MONO_I16_PADDING_MULTIFRAME_PIXELS: [u8; 16] = [
    0x00, 0x80, 0x18, 0xfc, 0xe8, 0x03, 0xb8, 0x0b, 0xb8, 0x0b, 0xe8, 0x03, 0x18, 0xfc, 0x00, 0x80,
];
const MONO_I16_PADDING_MULTIFRAME_VALUES: [i32; 8] =
    [-32768, -1000, 1000, 3000, 3000, 1000, -1000, -32768];
const YBR_FULL_PLANAR0_PIXELS: [u8; 12] = [76, 85, 255, 150, 44, 21, 29, 255, 107, 255, 128, 128];
const YBR_FULL_PLANAR0_MULTIFRAME_PIXELS: [u8; 24] = [
    76, 85, 255, 150, 44, 21, 29, 255, 107, 255, 128, 128, 179, 171, 1, 105, 212, 235, 226, 1, 149,
    0, 128, 128,
];
const YBR_FULL_PLANAR0_MULTIFRAME_VALUES: [i32; 24] = [
    76, 85, 255, 150, 44, 21, 29, 255, 107, 255, 128, 128, 179, 171, 1, 105, 212, 235, 226, 1, 149,
    0, 128, 128,
];
const YBR_FULL_PLANAR1_PIXELS: [u8; 12] = [76, 150, 29, 255, 85, 44, 255, 128, 255, 21, 107, 128];
const YBR_FULL_PLANAR1_MULTIFRAME_PIXELS: [u8; 24] = [
    76, 150, 29, 255, 85, 44, 255, 128, 255, 21, 107, 128, 179, 105, 226, 0, 171, 212, 1, 128, 1,
    235, 149, 128,
];
const YBR_FULL_PLANAR1_MULTIFRAME_VALUES: [i32; 24] = [
    76, 150, 29, 255, 85, 44, 255, 128, 255, 21, 107, 128, 179, 105, 226, 0, 171, 212, 1, 128, 1,
    235, 149, 128,
];
const YBR_FULL_422_PIXELS: [u8; 8] = [76, 150, 65, 138, 29, 255, 192, 118];
const PALETTE_COLOR_PIXELS: [u8; 4] = [0, 1, 2, 3];
const PALETTE_COLOR_VALUES: [i32; 4] = [0, 1, 2, 3];
const PALETTE_COLOR_MULTIFRAME_PIXELS: [u8; 8] = [0, 1, 2, 3, 3, 2, 1, 0];
const PALETTE_COLOR_MULTIFRAME_VALUES: [i32; 8] = [0, 1, 2, 3, 3, 2, 1, 0];
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
const ENHANCED_CT_U16_PIXELS: [u8; 16] = [
    0, 0, 0x64, 0, 0xc8, 0, 0x2c, 1, 0x90, 1, 0xf4, 1, 0x58, 2, 0xbc, 2,
];
const ENHANCED_CT_U16_VALUES: [i32; 8] = [0, 100, 200, 300, 400, 500, 600, 700];
const SEG_BINARY_PIXELS: [u8; 2] = [0b0000_1001, 0b0000_0110];
const SEG_BINARY_VALUES: [i32; 8] = [1, 0, 0, 1, 0, 1, 1, 0];
const SEG_FRACTIONAL_PROBABILITY_PIXELS: [u8; 8] = [0, 64, 128, 255, 255, 128, 64, 0];
const SEG_FRACTIONAL_PROBABILITY_VALUES: [i32; 8] = [0, 64, 128, 255, 255, 128, 64, 0];
const SEG_LABELMAP_PIXELS: [u8; 8] = [0, 1, 0, 1, 1, 0, 1, 0];
const SEG_LABELMAP_VALUES: [i32; 8] = [0, 1, 0, 1, 1, 0, 1, 0];
const RT_DOSE_GRID_PIXELS: [u8; 16] = [
    0x00, 0x00, 0x64, 0x00, 0xc8, 0x00, 0x2c, 0x01, 0x90, 0x01, 0xf4, 0x01, 0x58, 0x02, 0xbc, 0x02,
];
const MINIMAL_PDF_BYTES: &[u8] = b"%PDF-1.4\n1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 72 72] >>\nendobj\nxref\n0 4\n0000000000 65535 f \n0000000009 00000 n \n0000000058 00000 n \n0000000115 00000 n \ntrailer\n<< /Size 4 /Root 1 0 R >>\nstartxref\n184\n%%EOF\n";
const SEG_REFERENCED_FRAMES: [u16; 2] = [1, 2];
const SR_REFERENCED_FRAMES: [u16; 2] = [1, 2];
const KEY_OBJECT_SELECTION_IMAGE_REFERENCED_FRAMES: [u16; 2] = [1, 2];
const TAG_IMAGE_TYPE: Tag = Tag(0x0008, 0x0008);
const TAG_REFERENCED_SERIES_SEQUENCE: Tag = Tag(0x0008, 0x1115);
const TAG_REFERENCED_INSTANCE_SEQUENCE: Tag = Tag(0x0008, 0x114A);
const TAG_REFERENCED_SOP_CLASS_UID: Tag = Tag(0x0008, 0x1150);
const TAG_REFERENCED_SOP_INSTANCE_UID: Tag = Tag(0x0008, 0x1155);
const TAG_REFERENCED_FRAME_NUMBER: Tag = Tag(0x0008, 0x1160);
const TAG_SOURCE_IMAGE_SEQUENCE: Tag = Tag(0x0008, 0x2112);
const TAG_DERIVATION_CODE_SEQUENCE: Tag = Tag(0x0008, 0x9215);
const TAG_DERIVATION_IMAGE_SEQUENCE: Tag = Tag(0x0008, 0x9124);
const TAG_CONTENT_LABEL: Tag = Tag(0x0070, 0x0080);
const TAG_CONTENT_DESCRIPTION: Tag = Tag(0x0070, 0x0081);
const TAG_PRESENTATION_CREATION_DATE: Tag = Tag(0x0070, 0x0082);
const TAG_PRESENTATION_CREATION_TIME: Tag = Tag(0x0070, 0x0083);
const TAG_CONTENT_CREATOR_NAME: Tag = Tag(0x0070, 0x0084);
const TAG_DISPLAYED_AREA_TOP_LEFT_HAND_CORNER: Tag = Tag(0x0070, 0x0052);
const TAG_DISPLAYED_AREA_BOTTOM_RIGHT_HAND_CORNER: Tag = Tag(0x0070, 0x0053);
const TAG_DISPLAYED_AREA_SELECTION_SEQUENCE: Tag = Tag(0x0070, 0x005A);
const TAG_PRESENTATION_SIZE_MODE: Tag = Tag(0x0070, 0x0100);
const TAG_PRESENTATION_PIXEL_ASPECT_RATIO: Tag = Tag(0x0070, 0x0102);
const TAG_SOFTCOPY_VOI_LUT_SEQUENCE: Tag = Tag(0x0028, 0x3110);
const TAG_WINDOW_EXPLANATION: Tag = Tag(0x0028, 0x1055);
const TAG_PRESENTATION_LUT_SHAPE: Tag = Tag(0x2050, 0x0020);
const TAG_REFERENCED_IMAGE_SEQUENCE: Tag = Tag(0x0008, 0x1140);
const TAG_REFERENCED_STRUCTURE_SET_SEQUENCE: Tag = Tag(0x300C, 0x0060);
const TAG_PURPOSE_OF_REFERENCE_CODE_SEQUENCE: Tag = Tag(0x0040, 0xA170);
const TAG_SEGMENTATION_TYPE: Tag = Tag(0x0062, 0x0001);
const TAG_SEGMENT_SEQUENCE: Tag = Tag(0x0062, 0x0002);
const TAG_SEGMENTED_PROPERTY_CATEGORY_CODE_SEQUENCE: Tag = Tag(0x0062, 0x0003);
const TAG_SEGMENT_NUMBER: Tag = Tag(0x0062, 0x0004);
const TAG_SEGMENT_LABEL: Tag = Tag(0x0062, 0x0005);
const TAG_SEGMENT_ALGORITHM_TYPE: Tag = Tag(0x0062, 0x0008);
const TAG_SEGMENT_ALGORITHM_NAME: Tag = Tag(0x0062, 0x0009);
const TAG_SEGMENT_IDENTIFICATION_SEQUENCE: Tag = Tag(0x0062, 0x000A);
const TAG_REFERENCED_SEGMENT_NUMBER: Tag = Tag(0x0062, 0x000B);
const TAG_RECOMMENDED_DISPLAY_CIELAB_VALUE: Tag = Tag(0x0062, 0x000D);
const TAG_MAXIMUM_FRACTIONAL_VALUE: Tag = Tag(0x0062, 0x000E);
const TAG_SEGMENTED_PROPERTY_TYPE_CODE_SEQUENCE: Tag = Tag(0x0062, 0x000F);
const TAG_SEGMENTATION_FRACTIONAL_TYPE: Tag = Tag(0x0062, 0x0010);
const ENHANCED_MR_U16_PIXELS: [u8; 16] = [
    0, 0, 0x32, 0, 0x64, 0, 0x96, 0, 0xc8, 0, 0xfa, 0, 0x2c, 1, 0x5e, 1,
];
const ENHANCED_MR_U16_VALUES: [i32; 8] = [0, 50, 100, 150, 200, 250, 300, 350];
const ENHANCED_MR_TEMPORAL_U16_PIXELS: [u8; 16] = [
    0, 0, 0x19, 0, 0x32, 0, 0x4b, 0, 0x96, 0, 0xaf, 0, 0xc8, 0, 0xe1, 0,
];
const ENHANCED_MR_TEMPORAL_U16_VALUES: [i32; 8] = [0, 25, 50, 75, 150, 175, 200, 225];
const ENHANCED_MR_PHASE_U16_PIXELS: [u8; 16] = [
    0, 0, 0x28, 0, 0x50, 0, 0x78, 0, 0xa0, 0, 0xc8, 0, 0xf0, 0, 0x18, 1,
];
const ENHANCED_MR_PHASE_U16_VALUES: [i32; 8] = [0, 40, 80, 120, 160, 200, 240, 280];
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
        transfer_syntax: EXPLICIT_VR_LITTLE_ENDIAN,
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
        transfer_syntax: EXPLICIT_VR_LITTLE_ENDIAN,
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
        transfer_syntax: EXPLICIT_VR_LITTLE_ENDIAN,
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
        case_id: "vl/photo/rgb_planar0_explicit_le",
        recipe_id: "vl_photo_rgb_planar0",
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
        transfer_syntax: EXPLICIT_VR_LITTLE_ENDIAN,
        pixel_bytes: &RGB_PLANAR0_PIXELS,
        pixel_values: &[255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255],
        pixel_min: 0,
        pixel_max: 255,
        visual_pattern: "2x2_vl_photo_rgb_red_green_blue_white",
        semantic_note: "VL Photographic RGB samples are interleaved color-by-pixel",
        palette: None,
        padding: None,
    },
    PixelRecipe {
        case_id: "vl/photo/palette_color_explicit_le",
        recipe_id: "vl_photo_palette_color",
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
        transfer_syntax: EXPLICIT_VR_LITTLE_ENDIAN,
        pixel_bytes: &PALETTE_COLOR_PIXELS,
        pixel_values: &PALETTE_COLOR_VALUES,
        pixel_min: 0,
        pixel_max: 3,
        visual_pattern: "2x2_vl_photo_palette_red_green_blue_white",
        semantic_note: "VL Photographic stored pixel values index 16-bit RGB palette lookup tables",
        palette: Some(PALETTE_COLOR_LUT),
        padding: None,
    },
    PixelRecipe {
        case_id: "classic/sc/mono2_u8_explicit_be",
        recipe_id: "sc_mono2_u8_explicit_be",
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
        transfer_syntax: EXPLICIT_VR_BIG_ENDIAN,
        pixel_bytes: &MONO_PIXELS,
        pixel_values: &[0, 85, 170, 255],
        pixel_min: 0,
        pixel_max: 255,
        visual_pattern: "2x2_monochrome_gradient",
        semantic_note: "minimum sample value displays as black with retired Explicit VR Big Endian dataset encoding",
        palette: None,
        padding: None,
    },
    PixelRecipe {
        case_id: "classic/sc/mono2_u8_deflated_explicit_le",
        recipe_id: "sc_mono2_u8_deflated_explicit_le",
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
        transfer_syntax: DEFLATED_EXPLICIT_VR_LITTLE_ENDIAN,
        pixel_bytes: &MONO_PIXELS,
        pixel_values: &[0, 85, 170, 255],
        pixel_min: 0,
        pixel_max: 255,
        visual_pattern: "2x2_monochrome_gradient",
        semantic_note: "minimum sample value displays as black with deflated Explicit VR Little Endian dataset encoding",
        palette: None,
        padding: None,
    },
    PixelRecipe {
        case_id: "classic/sc/mono2_u8_rle_lossless",
        recipe_id: "sc_mono2_u8_rle_lossless",
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
        transfer_syntax: RLE_LOSSLESS,
        pixel_bytes: &MONO_PIXELS,
        pixel_values: &[0, 85, 170, 255],
        pixel_min: 0,
        pixel_max: 255,
        visual_pattern: "2x2_monochrome_gradient",
        semantic_note: "minimum sample value displays as black after RLE Lossless decode",
        palette: None,
        padding: None,
    },
    PixelRecipe {
        case_id: "classic/sc/mono2_u8_padding_rle_lossless",
        recipe_id: "sc_mono2_u8_padding_rle_lossless",
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
        transfer_syntax: RLE_LOSSLESS,
        pixel_bytes: &MONO_PIXELS,
        pixel_values: &[0, 85, 170, 255],
        pixel_min: 0,
        pixel_max: 255,
        visual_pattern: "2x2_monochrome_u8_rle_lossless_with_padding_value",
        semantic_note: "Pixel Padding Value 0 identifies a padded unsigned 8-bit MONOCHROME2 sample after RLE Lossless decode",
        palette: None,
        padding: Some(PixelPaddingRecipe {
            value: 0,
            range_limit: Some(0),
        }),
    },
    PixelRecipe {
        case_id: "classic/sc/mono1_u8_padding_rle_lossless",
        recipe_id: "sc_mono1_u8_padding_rle_lossless",
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
        transfer_syntax: RLE_LOSSLESS,
        pixel_bytes: &MONO_PIXELS,
        pixel_values: &[0, 85, 170, 255],
        pixel_min: 0,
        pixel_max: 255,
        visual_pattern: "2x2_inverse_monochrome_u8_rle_lossless_with_padding_value",
        semantic_note: "Pixel Padding Value 0 identifies a padded unsigned 8-bit MONOCHROME1 sample with inverse grayscale polarity after RLE Lossless decode",
        palette: None,
        padding: Some(PixelPaddingRecipe {
            value: 0,
            range_limit: Some(0),
        }),
    },
    PixelRecipe {
        case_id: "classic/sc/mono2_u8_padding_multiframe_rle_lossless",
        recipe_id: "sc_mono2_u8_padding_multiframe_rle_lossless",
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
        transfer_syntax: RLE_LOSSLESS,
        pixel_bytes: &MONO_MULTIFRAME_PIXELS,
        pixel_values: &MONO_MULTIFRAME_VALUES,
        pixel_min: 0,
        pixel_max: 255,
        visual_pattern: "2x2x2_monochrome_u8_rle_lossless_padding_reversed",
        semantic_note: "two unsigned 8-bit MONOCHROME2 frames preserve Pixel Padding Value 0 after RLE Lossless decode",
        palette: None,
        padding: Some(PixelPaddingRecipe {
            value: 0,
            range_limit: Some(0),
        }),
    },
    PixelRecipe {
        case_id: "classic/sc/mono1_u8_padding_multiframe_rle_lossless",
        recipe_id: "sc_mono1_u8_padding_multiframe_rle_lossless",
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
        transfer_syntax: RLE_LOSSLESS,
        pixel_bytes: &MONO_MULTIFRAME_PIXELS,
        pixel_values: &MONO_MULTIFRAME_VALUES,
        pixel_min: 0,
        pixel_max: 255,
        visual_pattern: "2x2x2_inverse_monochrome_u8_rle_lossless_padding_reversed",
        semantic_note: "two unsigned 8-bit MONOCHROME1 frames preserve Pixel Padding Value 0 with inverse grayscale polarity after RLE Lossless decode",
        palette: None,
        padding: Some(PixelPaddingRecipe {
            value: 0,
            range_limit: Some(0),
        }),
    },
    PixelRecipe {
        case_id: "classic/sc/mono1_u8_rle_lossless",
        recipe_id: "sc_mono1_u8_rle_lossless",
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
        transfer_syntax: RLE_LOSSLESS,
        pixel_bytes: &MONO_PIXELS,
        pixel_values: &[0, 85, 170, 255],
        pixel_min: 0,
        pixel_max: 255,
        visual_pattern: "2x2_inverse_monochrome_rle_lossless_gradient",
        semantic_note: "minimum sample value displays as white after RLE Lossless decode",
        palette: None,
        padding: None,
    },
    PixelRecipe {
        case_id: "classic/sc/mono2_u16_rle_lossless",
        recipe_id: "sc_mono2_u16_rle_lossless",
        rows: 2,
        columns: 2,
        photometric_interpretation: "MONOCHROME2",
        samples_per_pixel: 1,
        planar_configuration: None,
        bits_allocated: 16,
        bits_stored: 16,
        high_bit: 15,
        pixel_representation: 0,
        pixel_vr: VR::OB,
        transfer_syntax: RLE_LOSSLESS,
        pixel_bytes: &MONO_U16_PIXELS,
        pixel_values: &MONO_U16_VALUES,
        pixel_min: 0,
        pixel_max: 65535,
        visual_pattern: "2x2_monochrome_u16_gradient",
        semantic_note: "16-bit unsigned MONOCHROME2 samples span the full stored range after RLE Lossless decode",
        palette: None,
        padding: None,
    },
    PixelRecipe {
        case_id: "classic/sc/mono1_u16_rle_lossless",
        recipe_id: "sc_mono1_u16_rle_lossless",
        rows: 2,
        columns: 2,
        photometric_interpretation: "MONOCHROME1",
        samples_per_pixel: 1,
        planar_configuration: None,
        bits_allocated: 16,
        bits_stored: 16,
        high_bit: 15,
        pixel_representation: 0,
        pixel_vr: VR::OB,
        transfer_syntax: RLE_LOSSLESS,
        pixel_bytes: &MONO_U16_PIXELS,
        pixel_values: &MONO_U16_VALUES,
        pixel_min: 0,
        pixel_max: 65535,
        visual_pattern: "2x2_inverse_monochrome_u16_rle_lossless_gradient",
        semantic_note: "16-bit unsigned MONOCHROME1 samples invert grayscale polarity after RLE Lossless decode",
        palette: None,
        padding: None,
    },
    PixelRecipe {
        case_id: "classic/sc/mono2_u16_odd_3x3_rle_lossless",
        recipe_id: "sc_mono2_u16_odd_3x3_rle_lossless",
        rows: 3,
        columns: 3,
        photometric_interpretation: "MONOCHROME2",
        samples_per_pixel: 1,
        planar_configuration: None,
        bits_allocated: 16,
        bits_stored: 16,
        high_bit: 15,
        pixel_representation: 0,
        pixel_vr: VR::OB,
        transfer_syntax: RLE_LOSSLESS,
        pixel_bytes: &MONO_U16_ODD_3X3_PIXELS,
        pixel_values: &MONO_U16_ODD_3X3_VALUES,
        pixel_min: 0,
        pixel_max: 8,
        visual_pattern: "3x3_monochrome_u16_odd_rle_lossless_gradient",
        semantic_note: "odd Rows and Columns use unsigned 16-bit MONOCHROME2 samples after RLE Lossless decode",
        palette: None,
        padding: None,
    },
    PixelRecipe {
        case_id: "classic/sc/mono1_u16_odd_3x3_rle_lossless",
        recipe_id: "sc_mono1_u16_odd_3x3_rle_lossless",
        rows: 3,
        columns: 3,
        photometric_interpretation: "MONOCHROME1",
        samples_per_pixel: 1,
        planar_configuration: None,
        bits_allocated: 16,
        bits_stored: 16,
        high_bit: 15,
        pixel_representation: 0,
        pixel_vr: VR::OB,
        transfer_syntax: RLE_LOSSLESS,
        pixel_bytes: &MONO_U16_ODD_3X3_PIXELS,
        pixel_values: &MONO_U16_ODD_3X3_VALUES,
        pixel_min: 0,
        pixel_max: 8,
        visual_pattern: "3x3_inverse_monochrome_u16_odd_rle_lossless_gradient",
        semantic_note: "odd Rows and Columns use unsigned 16-bit MONOCHROME1 samples with inverse grayscale polarity after RLE Lossless decode",
        palette: None,
        padding: None,
    },
    PixelRecipe {
        case_id: "classic/sc/mono2_i16_odd_3x3_rle_lossless",
        recipe_id: "sc_mono2_i16_odd_3x3_rle_lossless",
        rows: 3,
        columns: 3,
        photometric_interpretation: "MONOCHROME2",
        samples_per_pixel: 1,
        planar_configuration: None,
        bits_allocated: 16,
        bits_stored: 16,
        high_bit: 15,
        pixel_representation: 1,
        pixel_vr: VR::OB,
        transfer_syntax: RLE_LOSSLESS,
        pixel_bytes: &MONO_I16_ODD_3X3_PIXELS,
        pixel_values: &MONO_I16_ODD_3X3_VALUES,
        pixel_min: -4,
        pixel_max: 4,
        visual_pattern: "3x3_monochrome_i16_odd_rle_lossless_centered_gradient",
        semantic_note: "odd Rows and Columns use signed 16-bit MONOCHROME2 samples after RLE Lossless decode",
        palette: None,
        padding: None,
    },
    PixelRecipe {
        case_id: "classic/sc/mono1_i16_odd_3x3_rle_lossless",
        recipe_id: "sc_mono1_i16_odd_3x3_rle_lossless",
        rows: 3,
        columns: 3,
        photometric_interpretation: "MONOCHROME1",
        samples_per_pixel: 1,
        planar_configuration: None,
        bits_allocated: 16,
        bits_stored: 16,
        high_bit: 15,
        pixel_representation: 1,
        pixel_vr: VR::OB,
        transfer_syntax: RLE_LOSSLESS,
        pixel_bytes: &MONO_I16_ODD_3X3_PIXELS,
        pixel_values: &MONO_I16_ODD_3X3_VALUES,
        pixel_min: -4,
        pixel_max: 4,
        visual_pattern: "3x3_inverse_monochrome_i16_odd_rle_lossless_centered_gradient",
        semantic_note: "odd Rows and Columns use signed 16-bit MONOCHROME1 samples with inverse grayscale polarity after RLE Lossless decode",
        palette: None,
        padding: None,
    },
    PixelRecipe {
        case_id: "classic/sc/mono2_u16_rect_2x3_rle_lossless",
        recipe_id: "sc_mono2_u16_rect_2x3_rle_lossless",
        rows: 2,
        columns: 3,
        photometric_interpretation: "MONOCHROME2",
        samples_per_pixel: 1,
        planar_configuration: None,
        bits_allocated: 16,
        bits_stored: 16,
        high_bit: 15,
        pixel_representation: 0,
        pixel_vr: VR::OB,
        transfer_syntax: RLE_LOSSLESS,
        pixel_bytes: &MONO_U16_RECT_2X3_PIXELS,
        pixel_values: &MONO_U16_RECT_2X3_VALUES,
        pixel_min: 0,
        pixel_max: 5,
        visual_pattern: "2x3_monochrome_u16_rect_rle_lossless_gradient",
        semantic_note: "rectangular unsigned MONOCHROME2 Pixel Data preserves Rows and Columns after RLE Lossless decode",
        palette: None,
        padding: None,
    },
    PixelRecipe {
        case_id: "classic/sc/mono1_u16_rect_2x3_rle_lossless",
        recipe_id: "sc_mono1_u16_rect_2x3_rle_lossless",
        rows: 2,
        columns: 3,
        photometric_interpretation: "MONOCHROME1",
        samples_per_pixel: 1,
        planar_configuration: None,
        bits_allocated: 16,
        bits_stored: 16,
        high_bit: 15,
        pixel_representation: 0,
        pixel_vr: VR::OB,
        transfer_syntax: RLE_LOSSLESS,
        pixel_bytes: &MONO_U16_RECT_2X3_PIXELS,
        pixel_values: &MONO_U16_RECT_2X3_VALUES,
        pixel_min: 0,
        pixel_max: 5,
        visual_pattern: "2x3_inverse_monochrome_u16_rect_rle_lossless_gradient",
        semantic_note: "rectangular unsigned MONOCHROME1 Pixel Data preserves Rows and Columns with inverse grayscale polarity after RLE Lossless decode",
        palette: None,
        padding: None,
    },
    PixelRecipe {
        case_id: "classic/sc/mono2_i16_rect_2x3_rle_lossless",
        recipe_id: "sc_mono2_i16_rect_2x3_rle_lossless",
        rows: 2,
        columns: 3,
        photometric_interpretation: "MONOCHROME2",
        samples_per_pixel: 1,
        planar_configuration: None,
        bits_allocated: 16,
        bits_stored: 16,
        high_bit: 15,
        pixel_representation: 1,
        pixel_vr: VR::OB,
        transfer_syntax: RLE_LOSSLESS,
        pixel_bytes: &MONO_I16_RECT_2X3_PIXELS,
        pixel_values: &MONO_I16_RECT_2X3_VALUES,
        pixel_min: -3,
        pixel_max: 2,
        visual_pattern: "2x3_monochrome_i16_rect_rle_lossless_centered_gradient",
        semantic_note: "rectangular signed MONOCHROME2 Pixel Data preserves Rows and Columns after RLE Lossless decode",
        palette: None,
        padding: None,
    },
    PixelRecipe {
        case_id: "classic/sc/mono1_i16_rect_2x3_rle_lossless",
        recipe_id: "sc_mono1_i16_rect_2x3_rle_lossless",
        rows: 2,
        columns: 3,
        photometric_interpretation: "MONOCHROME1",
        samples_per_pixel: 1,
        planar_configuration: None,
        bits_allocated: 16,
        bits_stored: 16,
        high_bit: 15,
        pixel_representation: 1,
        pixel_vr: VR::OB,
        transfer_syntax: RLE_LOSSLESS,
        pixel_bytes: &MONO_I16_RECT_2X3_PIXELS,
        pixel_values: &MONO_I16_RECT_2X3_VALUES,
        pixel_min: -3,
        pixel_max: 2,
        visual_pattern: "2x3_inverse_monochrome_i16_rect_rle_lossless_centered_gradient",
        semantic_note: "rectangular signed MONOCHROME1 Pixel Data preserves Rows and Columns with inverse grayscale polarity after RLE Lossless decode",
        palette: None,
        padding: None,
    },
    PixelRecipe {
        case_id: "classic/sc/mono2_u16_tiny_1x1_rle_lossless",
        recipe_id: "sc_mono2_u16_tiny_1x1_rle_lossless",
        rows: 1,
        columns: 1,
        photometric_interpretation: "MONOCHROME2",
        samples_per_pixel: 1,
        planar_configuration: None,
        bits_allocated: 16,
        bits_stored: 16,
        high_bit: 15,
        pixel_representation: 0,
        pixel_vr: VR::OB,
        transfer_syntax: RLE_LOSSLESS,
        pixel_bytes: &MONO_U16_TINY_1X1_PIXELS,
        pixel_values: &MONO_U16_TINY_1X1_VALUES,
        pixel_min: 65535,
        pixel_max: 65535,
        visual_pattern: "1x1_monochrome_u16_rle_lossless_tiny_maximum",
        semantic_note: "very small unsigned MONOCHROME2 Pixel Data decodes from one RLE Lossless fragment",
        palette: None,
        padding: None,
    },
    PixelRecipe {
        case_id: "classic/sc/mono1_u16_tiny_1x1_rle_lossless",
        recipe_id: "sc_mono1_u16_tiny_1x1_rle_lossless",
        rows: 1,
        columns: 1,
        photometric_interpretation: "MONOCHROME1",
        samples_per_pixel: 1,
        planar_configuration: None,
        bits_allocated: 16,
        bits_stored: 16,
        high_bit: 15,
        pixel_representation: 0,
        pixel_vr: VR::OB,
        transfer_syntax: RLE_LOSSLESS,
        pixel_bytes: &MONO_U16_TINY_1X1_PIXELS,
        pixel_values: &MONO_U16_TINY_1X1_VALUES,
        pixel_min: 65535,
        pixel_max: 65535,
        visual_pattern: "1x1_inverse_monochrome_u16_rle_lossless_tiny_maximum",
        semantic_note: "very small unsigned MONOCHROME1 Pixel Data decodes from one RLE Lossless fragment with inverse grayscale polarity",
        palette: None,
        padding: None,
    },
    PixelRecipe {
        case_id: "classic/sc/mono2_i16_tiny_1x1_rle_lossless",
        recipe_id: "sc_mono2_i16_tiny_1x1_rle_lossless",
        rows: 1,
        columns: 1,
        photometric_interpretation: "MONOCHROME2",
        samples_per_pixel: 1,
        planar_configuration: None,
        bits_allocated: 16,
        bits_stored: 16,
        high_bit: 15,
        pixel_representation: 1,
        pixel_vr: VR::OB,
        transfer_syntax: RLE_LOSSLESS,
        pixel_bytes: &MONO_I16_TINY_1X1_PIXELS,
        pixel_values: &MONO_I16_TINY_1X1_VALUES,
        pixel_min: -32768,
        pixel_max: -32768,
        visual_pattern: "1x1_monochrome_i16_rle_lossless_tiny_minimum",
        semantic_note: "very small signed MONOCHROME2 Pixel Data decodes from one RLE Lossless fragment",
        palette: None,
        padding: None,
    },
    PixelRecipe {
        case_id: "classic/sc/mono1_i16_tiny_1x1_rle_lossless",
        recipe_id: "sc_mono1_i16_tiny_1x1_rle_lossless",
        rows: 1,
        columns: 1,
        photometric_interpretation: "MONOCHROME1",
        samples_per_pixel: 1,
        planar_configuration: None,
        bits_allocated: 16,
        bits_stored: 16,
        high_bit: 15,
        pixel_representation: 1,
        pixel_vr: VR::OB,
        transfer_syntax: RLE_LOSSLESS,
        pixel_bytes: &MONO_I16_TINY_1X1_PIXELS,
        pixel_values: &MONO_I16_TINY_1X1_VALUES,
        pixel_min: -32768,
        pixel_max: -32768,
        visual_pattern: "1x1_inverse_monochrome_i16_rle_lossless_tiny_minimum",
        semantic_note: "very small signed MONOCHROME1 Pixel Data decodes from one RLE Lossless fragment with inverse grayscale polarity",
        palette: None,
        padding: None,
    },
    PixelRecipe {
        case_id: "classic/sc/mono2_u16_padding_rle_lossless",
        recipe_id: "sc_mono2_u16_padding_rle_lossless",
        rows: 2,
        columns: 2,
        photometric_interpretation: "MONOCHROME2",
        samples_per_pixel: 1,
        planar_configuration: None,
        bits_allocated: 16,
        bits_stored: 16,
        high_bit: 15,
        pixel_representation: 0,
        pixel_vr: VR::OB,
        transfer_syntax: RLE_LOSSLESS,
        pixel_bytes: &MONO_U16_PADDING_PIXELS,
        pixel_values: &MONO_U16_PADDING_VALUES,
        pixel_min: 0,
        pixel_max: 3000,
        visual_pattern: "2x2_monochrome_u16_rle_lossless_with_padding_value",
        semantic_note: "Pixel Padding Value 0 identifies a padded unsigned MONOCHROME2 sample after RLE Lossless decode",
        palette: None,
        padding: Some(PixelPaddingRecipe {
            value: 0,
            range_limit: Some(0),
        }),
    },
    PixelRecipe {
        case_id: "classic/sc/mono1_u16_padding_rle_lossless",
        recipe_id: "sc_mono1_u16_padding_rle_lossless",
        rows: 2,
        columns: 2,
        photometric_interpretation: "MONOCHROME1",
        samples_per_pixel: 1,
        planar_configuration: None,
        bits_allocated: 16,
        bits_stored: 16,
        high_bit: 15,
        pixel_representation: 0,
        pixel_vr: VR::OB,
        transfer_syntax: RLE_LOSSLESS,
        pixel_bytes: &MONO_U16_PADDING_PIXELS,
        pixel_values: &MONO_U16_PADDING_VALUES,
        pixel_min: 0,
        pixel_max: 3000,
        visual_pattern: "2x2_inverse_monochrome_u16_rle_lossless_with_padding_value",
        semantic_note: "Pixel Padding Value 0 identifies a padded unsigned MONOCHROME1 sample with inverse grayscale polarity after RLE Lossless decode",
        palette: None,
        padding: Some(PixelPaddingRecipe {
            value: 0,
            range_limit: Some(0),
        }),
    },
    PixelRecipe {
        case_id: "classic/sc/mono2_u16_padding_multiframe_rle_lossless",
        recipe_id: "sc_mono2_u16_padding_multiframe_rle_lossless",
        rows: 2,
        columns: 2,
        photometric_interpretation: "MONOCHROME2",
        samples_per_pixel: 1,
        planar_configuration: None,
        bits_allocated: 16,
        bits_stored: 16,
        high_bit: 15,
        pixel_representation: 0,
        pixel_vr: VR::OB,
        transfer_syntax: RLE_LOSSLESS,
        pixel_bytes: &MONO_U16_PADDING_MULTIFRAME_PIXELS,
        pixel_values: &MONO_U16_PADDING_MULTIFRAME_VALUES,
        pixel_min: 0,
        pixel_max: 3000,
        visual_pattern: "2x2x2_monochrome_u16_rle_lossless_padding_reversed",
        semantic_note: "two unsigned MONOCHROME2 frames preserve Pixel Padding Value 0 after RLE Lossless decode",
        palette: None,
        padding: Some(PixelPaddingRecipe {
            value: 0,
            range_limit: Some(0),
        }),
    },
    PixelRecipe {
        case_id: "classic/sc/mono1_u16_padding_multiframe_rle_lossless",
        recipe_id: "sc_mono1_u16_padding_multiframe_rle_lossless",
        rows: 2,
        columns: 2,
        photometric_interpretation: "MONOCHROME1",
        samples_per_pixel: 1,
        planar_configuration: None,
        bits_allocated: 16,
        bits_stored: 16,
        high_bit: 15,
        pixel_representation: 0,
        pixel_vr: VR::OB,
        transfer_syntax: RLE_LOSSLESS,
        pixel_bytes: &MONO_U16_PADDING_MULTIFRAME_PIXELS,
        pixel_values: &MONO_U16_PADDING_MULTIFRAME_VALUES,
        pixel_min: 0,
        pixel_max: 3000,
        visual_pattern: "2x2x2_inverse_monochrome_u16_rle_lossless_padding_reversed",
        semantic_note: "two unsigned MONOCHROME1 frames preserve Pixel Padding Value 0 with inverse grayscale polarity after RLE Lossless decode",
        palette: None,
        padding: Some(PixelPaddingRecipe {
            value: 0,
            range_limit: Some(0),
        }),
    },
    PixelRecipe {
        case_id: "classic/sc/mono2_i16_padding_rle_lossless",
        recipe_id: "sc_mono2_i16_padding_rle_lossless",
        rows: 2,
        columns: 2,
        photometric_interpretation: "MONOCHROME2",
        samples_per_pixel: 1,
        planar_configuration: None,
        bits_allocated: 16,
        bits_stored: 16,
        high_bit: 15,
        pixel_representation: 1,
        pixel_vr: VR::OB,
        transfer_syntax: RLE_LOSSLESS,
        pixel_bytes: &MONO_I16_PADDING_PIXELS,
        pixel_values: &MONO_I16_PADDING_VALUES,
        pixel_min: -32768,
        pixel_max: 3000,
        visual_pattern: "2x2_monochrome_i16_rle_lossless_with_signed_padding_value",
        semantic_note: "signed Pixel Padding Value -32768 identifies a padded MONOCHROME2 sample after RLE Lossless decode",
        palette: None,
        padding: Some(PixelPaddingRecipe {
            value: -32768,
            range_limit: Some(-32768),
        }),
    },
    PixelRecipe {
        case_id: "classic/sc/mono1_i16_padding_rle_lossless",
        recipe_id: "sc_mono1_i16_padding_rle_lossless",
        rows: 2,
        columns: 2,
        photometric_interpretation: "MONOCHROME1",
        samples_per_pixel: 1,
        planar_configuration: None,
        bits_allocated: 16,
        bits_stored: 16,
        high_bit: 15,
        pixel_representation: 1,
        pixel_vr: VR::OB,
        transfer_syntax: RLE_LOSSLESS,
        pixel_bytes: &MONO_I16_PADDING_PIXELS,
        pixel_values: &MONO_I16_PADDING_VALUES,
        pixel_min: -32768,
        pixel_max: 3000,
        visual_pattern: "2x2_inverse_monochrome_i16_rle_lossless_with_signed_padding_value",
        semantic_note: "signed Pixel Padding Value -32768 identifies a padded MONOCHROME1 sample with inverse grayscale polarity after RLE Lossless decode",
        palette: None,
        padding: Some(PixelPaddingRecipe {
            value: -32768,
            range_limit: Some(-32768),
        }),
    },
    PixelRecipe {
        case_id: "classic/sc/mono1_i16_padding_multiframe_rle_lossless",
        recipe_id: "sc_mono1_i16_padding_multiframe_rle_lossless",
        rows: 2,
        columns: 2,
        photometric_interpretation: "MONOCHROME1",
        samples_per_pixel: 1,
        planar_configuration: None,
        bits_allocated: 16,
        bits_stored: 16,
        high_bit: 15,
        pixel_representation: 1,
        pixel_vr: VR::OB,
        transfer_syntax: RLE_LOSSLESS,
        pixel_bytes: &MONO_I16_PADDING_MULTIFRAME_PIXELS,
        pixel_values: &MONO_I16_PADDING_MULTIFRAME_VALUES,
        pixel_min: -32768,
        pixel_max: 3000,
        visual_pattern: "2x2x2_inverse_monochrome_i16_rle_lossless_signed_padding_reversed",
        semantic_note: "two signed MONOCHROME1 frames preserve Pixel Padding Value -32768 with inverse grayscale polarity after RLE Lossless decode",
        palette: None,
        padding: Some(PixelPaddingRecipe {
            value: -32768,
            range_limit: Some(-32768),
        }),
    },
    PixelRecipe {
        case_id: "classic/sc/mono2_i16_padding_multiframe_rle_lossless",
        recipe_id: "sc_mono2_i16_padding_multiframe_rle_lossless",
        rows: 2,
        columns: 2,
        photometric_interpretation: "MONOCHROME2",
        samples_per_pixel: 1,
        planar_configuration: None,
        bits_allocated: 16,
        bits_stored: 16,
        high_bit: 15,
        pixel_representation: 1,
        pixel_vr: VR::OB,
        transfer_syntax: RLE_LOSSLESS,
        pixel_bytes: &MONO_I16_PADDING_MULTIFRAME_PIXELS,
        pixel_values: &MONO_I16_PADDING_MULTIFRAME_VALUES,
        pixel_min: -32768,
        pixel_max: 3000,
        visual_pattern: "2x2x2_monochrome_i16_rle_lossless_signed_padding_reversed",
        semantic_note: "two signed MONOCHROME2 frames preserve Pixel Padding Value -32768 after RLE Lossless decode",
        palette: None,
        padding: Some(PixelPaddingRecipe {
            value: -32768,
            range_limit: Some(-32768),
        }),
    },
    PixelRecipe {
        case_id: "classic/sc/mono2_i16_rle_lossless",
        recipe_id: "sc_mono2_i16_rle_lossless",
        rows: 2,
        columns: 2,
        photometric_interpretation: "MONOCHROME2",
        samples_per_pixel: 1,
        planar_configuration: None,
        bits_allocated: 16,
        bits_stored: 16,
        high_bit: 15,
        pixel_representation: 1,
        pixel_vr: VR::OB,
        transfer_syntax: RLE_LOSSLESS,
        pixel_bytes: &MONO_I16_PIXELS,
        pixel_values: &MONO_I16_VALUES,
        pixel_min: -32768,
        pixel_max: 32767,
        visual_pattern: "2x2_monochrome_i16_rle_lossless_gradient",
        semantic_note: "16-bit signed MONOCHROME2 samples use 2's complement representation after RLE Lossless decode",
        palette: None,
        padding: None,
    },
    PixelRecipe {
        case_id: "classic/sc/mono1_i16_rle_lossless",
        recipe_id: "sc_mono1_i16_rle_lossless",
        rows: 2,
        columns: 2,
        photometric_interpretation: "MONOCHROME1",
        samples_per_pixel: 1,
        planar_configuration: None,
        bits_allocated: 16,
        bits_stored: 16,
        high_bit: 15,
        pixel_representation: 1,
        pixel_vr: VR::OB,
        transfer_syntax: RLE_LOSSLESS,
        pixel_bytes: &MONO_I16_PIXELS,
        pixel_values: &MONO_I16_VALUES,
        pixel_min: -32768,
        pixel_max: 32767,
        visual_pattern: "2x2_inverse_monochrome_i16_rle_lossless_gradient",
        semantic_note: "16-bit signed MONOCHROME1 samples use 2's complement representation with inverse grayscale polarity after RLE Lossless decode",
        palette: None,
        padding: None,
    },
    PixelRecipe {
        case_id: "classic/sc/mono2_u8_multiframe_rle_lossless",
        recipe_id: "sc_mono2_u8_multiframe_rle_lossless",
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
        transfer_syntax: RLE_LOSSLESS,
        pixel_bytes: &MONO_MULTIFRAME_PIXELS,
        pixel_values: &MONO_MULTIFRAME_VALUES,
        pixel_min: 0,
        pixel_max: 255,
        visual_pattern: "2x2x2_monochrome_rle_lossless_gradient_reversed",
        semantic_note: "two MONOCHROME2 frames decode from separate RLE Lossless fragments",
        palette: None,
        padding: None,
    },
    PixelRecipe {
        case_id: "classic/sc/mono1_u8_multiframe_rle_lossless",
        recipe_id: "sc_mono1_u8_multiframe_rle_lossless",
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
        transfer_syntax: RLE_LOSSLESS,
        pixel_bytes: &MONO_MULTIFRAME_PIXELS,
        pixel_values: &MONO_MULTIFRAME_VALUES,
        pixel_min: 0,
        pixel_max: 255,
        visual_pattern: "2x2x2_inverse_monochrome_rle_lossless_gradient_reversed",
        semantic_note: "two MONOCHROME1 frames decode from separate RLE Lossless fragments with inverse grayscale polarity",
        palette: None,
        padding: None,
    },
    PixelRecipe {
        case_id: "classic/sc/mono2_u16_multiframe_rle_lossless",
        recipe_id: "sc_mono2_u16_multiframe_rle_lossless",
        rows: 2,
        columns: 2,
        photometric_interpretation: "MONOCHROME2",
        samples_per_pixel: 1,
        planar_configuration: None,
        bits_allocated: 16,
        bits_stored: 16,
        high_bit: 15,
        pixel_representation: 0,
        pixel_vr: VR::OB,
        transfer_syntax: RLE_LOSSLESS,
        pixel_bytes: &MONO_U16_MULTIFRAME_PIXELS,
        pixel_values: &MONO_U16_MULTIFRAME_VALUES,
        pixel_min: 0,
        pixel_max: 65535,
        visual_pattern: "2x2x2_monochrome_u16_rle_lossless_gradient_reversed",
        semantic_note: "two unsigned 16-bit MONOCHROME2 frames decode from separate RLE Lossless fragments",
        palette: None,
        padding: None,
    },
    PixelRecipe {
        case_id: "classic/sc/mono1_u16_multiframe_rle_lossless",
        recipe_id: "sc_mono1_u16_multiframe_rle_lossless",
        rows: 2,
        columns: 2,
        photometric_interpretation: "MONOCHROME1",
        samples_per_pixel: 1,
        planar_configuration: None,
        bits_allocated: 16,
        bits_stored: 16,
        high_bit: 15,
        pixel_representation: 0,
        pixel_vr: VR::OB,
        transfer_syntax: RLE_LOSSLESS,
        pixel_bytes: &MONO_U16_MULTIFRAME_PIXELS,
        pixel_values: &MONO_U16_MULTIFRAME_VALUES,
        pixel_min: 0,
        pixel_max: 65535,
        visual_pattern: "2x2x2_inverse_monochrome_u16_rle_lossless_gradient_reversed",
        semantic_note: "two unsigned 16-bit MONOCHROME1 frames decode from separate RLE Lossless fragments with inverse grayscale polarity",
        palette: None,
        padding: None,
    },
    PixelRecipe {
        case_id: "classic/sc/mono2_i16_multiframe_rle_lossless",
        recipe_id: "sc_mono2_i16_multiframe_rle_lossless",
        rows: 2,
        columns: 2,
        photometric_interpretation: "MONOCHROME2",
        samples_per_pixel: 1,
        planar_configuration: None,
        bits_allocated: 16,
        bits_stored: 16,
        high_bit: 15,
        pixel_representation: 1,
        pixel_vr: VR::OB,
        transfer_syntax: RLE_LOSSLESS,
        pixel_bytes: &MONO_I16_MULTIFRAME_PIXELS,
        pixel_values: &MONO_I16_MULTIFRAME_VALUES,
        pixel_min: -32768,
        pixel_max: 32767,
        visual_pattern: "2x2x2_monochrome_i16_rle_lossless_gradient_reversed",
        semantic_note: "two signed 16-bit MONOCHROME2 frames preserve 2's complement samples after RLE Lossless decode",
        palette: None,
        padding: None,
    },
    PixelRecipe {
        case_id: "classic/sc/mono1_i16_multiframe_rle_lossless",
        recipe_id: "sc_mono1_i16_multiframe_rle_lossless",
        rows: 2,
        columns: 2,
        photometric_interpretation: "MONOCHROME1",
        samples_per_pixel: 1,
        planar_configuration: None,
        bits_allocated: 16,
        bits_stored: 16,
        high_bit: 15,
        pixel_representation: 1,
        pixel_vr: VR::OB,
        transfer_syntax: RLE_LOSSLESS,
        pixel_bytes: &MONO_I16_MULTIFRAME_PIXELS,
        pixel_values: &MONO_I16_MULTIFRAME_VALUES,
        pixel_min: -32768,
        pixel_max: 32767,
        visual_pattern: "2x2x2_inverse_monochrome_i16_rle_lossless_gradient_reversed",
        semantic_note: "two signed 16-bit MONOCHROME1 frames preserve 2's complement samples with inverse grayscale polarity after RLE Lossless decode",
        palette: None,
        padding: None,
    },
    PixelRecipe {
        case_id: "classic/sc/mono2_u8_odd_fragment_rle_lossless",
        recipe_id: "sc_mono2_u8_odd_fragment_rle_lossless",
        rows: 1,
        columns: 2,
        photometric_interpretation: "MONOCHROME2",
        samples_per_pixel: 1,
        planar_configuration: None,
        bits_allocated: 8,
        bits_stored: 8,
        high_bit: 7,
        pixel_representation: 0,
        pixel_vr: VR::OB,
        transfer_syntax: RLE_LOSSLESS,
        pixel_bytes: &MONO_ODD_RLE_PIXELS,
        pixel_values: &MONO_ODD_RLE_VALUES,
        pixel_min: 0,
        pixel_max: 255,
        visual_pattern: "1x2_monochrome_rle_lossless_odd_fragment",
        semantic_note: "two literal MONOCHROME2 samples produce an odd-length RLE fragment padded in encapsulated Pixel Data",
        palette: None,
        padding: None,
    },
    PixelRecipe {
        case_id: "classic/sc/mono1_u8_odd_fragment_rle_lossless",
        recipe_id: "sc_mono1_u8_odd_fragment_rle_lossless",
        rows: 1,
        columns: 2,
        photometric_interpretation: "MONOCHROME1",
        samples_per_pixel: 1,
        planar_configuration: None,
        bits_allocated: 8,
        bits_stored: 8,
        high_bit: 7,
        pixel_representation: 0,
        pixel_vr: VR::OB,
        transfer_syntax: RLE_LOSSLESS,
        pixel_bytes: &MONO_ODD_RLE_PIXELS,
        pixel_values: &MONO_ODD_RLE_VALUES,
        pixel_min: 0,
        pixel_max: 255,
        visual_pattern: "1x2_inverse_monochrome_rle_lossless_odd_fragment",
        semantic_note: "two literal MONOCHROME1 samples produce an odd-length RLE fragment padded in encapsulated Pixel Data",
        palette: None,
        padding: None,
    },
    PixelRecipe {
        case_id: "classic/sc/rgb_planar0_rle_lossless",
        recipe_id: "sc_rgb_planar0_rle_lossless",
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
        transfer_syntax: RLE_LOSSLESS,
        pixel_bytes: &RGB_PLANAR0_PIXELS,
        pixel_values: &[255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255],
        pixel_min: 0,
        pixel_max: 255,
        visual_pattern: "2x2_rgb_rle_lossless_red_green_blue_white",
        semantic_note: "RGB samples remain interleaved color-by-pixel after RLE Lossless decode",
        palette: None,
        padding: None,
    },
    PixelRecipe {
        case_id: "classic/sc/rgb_planar0_multiframe_rle_lossless",
        recipe_id: "sc_rgb_planar0_multiframe_rle_lossless",
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
        transfer_syntax: RLE_LOSSLESS,
        pixel_bytes: &RGB_PLANAR0_MULTIFRAME_PIXELS,
        pixel_values: &RGB_PLANAR0_MULTIFRAME_VALUES,
        pixel_min: 0,
        pixel_max: 255,
        visual_pattern: "2x2x2_rgb_planar0_rle_lossless_primary_secondary",
        semantic_note: "two RGB planar-configuration-0 frames decode from separate RLE Lossless fragments",
        palette: None,
        padding: None,
    },
    PixelRecipe {
        case_id: "classic/sc/rgb_planar1_rle_lossless",
        recipe_id: "sc_rgb_planar1_rle_lossless",
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
        transfer_syntax: RLE_LOSSLESS,
        pixel_bytes: &RGB_PLANAR1_PIXELS,
        pixel_values: &[255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255],
        pixel_min: 0,
        pixel_max: 255,
        visual_pattern: "2x2_rgb_planar1_rle_lossless_red_green_blue_white",
        semantic_note: "RGB samples remain color-by-plane after RLE Lossless decode",
        palette: None,
        padding: None,
    },
    PixelRecipe {
        case_id: "classic/sc/rgb_planar1_multiframe_rle_lossless",
        recipe_id: "sc_rgb_planar1_multiframe_rle_lossless",
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
        transfer_syntax: RLE_LOSSLESS,
        pixel_bytes: &RGB_PLANAR1_MULTIFRAME_PIXELS,
        pixel_values: &RGB_PLANAR1_MULTIFRAME_VALUES,
        pixel_min: 0,
        pixel_max: 255,
        visual_pattern: "2x2x2_rgb_planar1_rle_lossless_primary_secondary",
        semantic_note: "two RGB planar-configuration-1 frames decode from separate RLE Lossless fragments",
        palette: None,
        padding: None,
    },
    PixelRecipe {
        case_id: "classic/sc/ybr_full_planar0_rle_lossless",
        recipe_id: "sc_ybr_full_planar0_rle_lossless",
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
        transfer_syntax: RLE_LOSSLESS,
        pixel_bytes: &YBR_FULL_PLANAR0_PIXELS,
        pixel_values: &[76, 85, 255, 150, 44, 21, 29, 255, 107, 255, 128, 128],
        pixel_min: 21,
        pixel_max: 255,
        visual_pattern: "2x2_ybr_full_rle_lossless_red_green_blue_white",
        semantic_note: "YBR_FULL samples remain interleaved color-by-pixel after RLE Lossless decode",
        palette: None,
        padding: None,
    },
    PixelRecipe {
        case_id: "classic/sc/ybr_full_planar0_multiframe_rle_lossless",
        recipe_id: "sc_ybr_full_planar0_multiframe_rle_lossless",
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
        transfer_syntax: RLE_LOSSLESS,
        pixel_bytes: &YBR_FULL_PLANAR0_MULTIFRAME_PIXELS,
        pixel_values: &YBR_FULL_PLANAR0_MULTIFRAME_VALUES,
        pixel_min: 0,
        pixel_max: 255,
        visual_pattern: "2x2x2_ybr_full_planar0_rle_lossless_primary_secondary",
        semantic_note: "two YBR_FULL planar-configuration-0 frames decode from separate RLE Lossless fragments",
        palette: None,
        padding: None,
    },
    PixelRecipe {
        case_id: "classic/sc/ybr_full_planar1_rle_lossless",
        recipe_id: "sc_ybr_full_planar1_rle_lossless",
        rows: 2,
        columns: 2,
        photometric_interpretation: "YBR_FULL",
        samples_per_pixel: 3,
        planar_configuration: Some(1),
        bits_allocated: 8,
        bits_stored: 8,
        high_bit: 7,
        pixel_representation: 0,
        pixel_vr: VR::OB,
        transfer_syntax: RLE_LOSSLESS,
        pixel_bytes: &YBR_FULL_PLANAR1_PIXELS,
        pixel_values: &[76, 150, 29, 255, 85, 44, 255, 128, 255, 21, 107, 128],
        pixel_min: 21,
        pixel_max: 255,
        visual_pattern: "2x2_ybr_full_planar1_rle_lossless_red_green_blue_white",
        semantic_note: "YBR_FULL samples remain color-by-plane after RLE Lossless decode",
        palette: None,
        padding: None,
    },
    PixelRecipe {
        case_id: "classic/sc/ybr_full_planar1_multiframe_rle_lossless",
        recipe_id: "sc_ybr_full_planar1_multiframe_rle_lossless",
        rows: 2,
        columns: 2,
        photometric_interpretation: "YBR_FULL",
        samples_per_pixel: 3,
        planar_configuration: Some(1),
        bits_allocated: 8,
        bits_stored: 8,
        high_bit: 7,
        pixel_representation: 0,
        pixel_vr: VR::OB,
        transfer_syntax: RLE_LOSSLESS,
        pixel_bytes: &YBR_FULL_PLANAR1_MULTIFRAME_PIXELS,
        pixel_values: &YBR_FULL_PLANAR1_MULTIFRAME_VALUES,
        pixel_min: 0,
        pixel_max: 255,
        visual_pattern: "2x2x2_ybr_full_planar1_rle_lossless_primary_secondary",
        semantic_note: "two YBR_FULL planar-configuration-1 frames decode from separate RLE Lossless fragments",
        palette: None,
        padding: None,
    },
    PixelRecipe {
        case_id: "classic/sc/palette_color_u8_rle_lossless",
        recipe_id: "sc_palette_color_u8_rle_lossless",
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
        transfer_syntax: RLE_LOSSLESS,
        pixel_bytes: &PALETTE_COLOR_PIXELS,
        pixel_values: &PALETTE_COLOR_VALUES,
        pixel_min: 0,
        pixel_max: 3,
        visual_pattern: "2x2_palette_rle_lossless_red_green_blue_white",
        semantic_note: "stored RLE Lossless pixel values index 16-bit RGB palette lookup tables after decode",
        palette: Some(PALETTE_COLOR_LUT),
        padding: None,
    },
    PixelRecipe {
        case_id: "classic/sc/palette_color_u8_multiframe_rle_lossless",
        recipe_id: "sc_palette_color_u8_multiframe_rle_lossless",
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
        transfer_syntax: RLE_LOSSLESS,
        pixel_bytes: &PALETTE_COLOR_MULTIFRAME_PIXELS,
        pixel_values: &PALETTE_COLOR_MULTIFRAME_VALUES,
        pixel_min: 0,
        pixel_max: 3,
        visual_pattern: "2x2x2_palette_rle_lossless_palette_order_reversed",
        semantic_note: "two PALETTE COLOR frames share 16-bit RGB lookup tables after RLE Lossless decode",
        palette: Some(PALETTE_COLOR_LUT),
        padding: None,
    },
    PixelRecipe {
        case_id: "vl/photo/rgb_planar0_rle_lossless",
        recipe_id: "vl_photo_rgb_planar0_rle_lossless",
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
        transfer_syntax: RLE_LOSSLESS,
        pixel_bytes: &RGB_PLANAR0_PIXELS,
        pixel_values: &[255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255],
        pixel_min: 0,
        pixel_max: 255,
        visual_pattern: "2x2_vl_photo_rgb_rle_lossless_red_green_blue_white",
        semantic_note: "VL Photographic RGB samples remain interleaved color-by-pixel after RLE Lossless decode",
        palette: None,
        padding: None,
    },
    PixelRecipe {
        case_id: "vl/photo/rgb_planar1_rle_lossless",
        recipe_id: "vl_photo_rgb_planar1_rle_lossless",
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
        transfer_syntax: RLE_LOSSLESS,
        pixel_bytes: &RGB_PLANAR1_PIXELS,
        pixel_values: &[255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255],
        pixel_min: 0,
        pixel_max: 255,
        visual_pattern: "2x2_vl_photo_rgb_planar1_rle_lossless_red_green_blue_white",
        semantic_note: "VL Photographic RGB samples remain color-by-plane after RLE Lossless decode",
        palette: None,
        padding: None,
    },
    PixelRecipe {
        case_id: "vl/photo/palette_color_rle_lossless",
        recipe_id: "vl_photo_palette_color_rle_lossless",
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
        transfer_syntax: RLE_LOSSLESS,
        pixel_bytes: &PALETTE_COLOR_PIXELS,
        pixel_values: &PALETTE_COLOR_VALUES,
        pixel_min: 0,
        pixel_max: 3,
        visual_pattern: "2x2_vl_photo_palette_rle_lossless_red_green_blue_white",
        semantic_note: "VL Photographic stored RLE Lossless pixel values index 16-bit RGB palette lookup tables after decode",
        palette: Some(PALETTE_COLOR_LUT),
        padding: None,
    },
    PixelRecipe {
        case_id: "classic/sc/rgb_planar0_jpeg_baseline_8bit",
        recipe_id: "sc_rgb_planar0_jpeg_baseline_8bit",
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
        transfer_syntax: JPEG_BASELINE_8BIT,
        pixel_bytes: &RGB_PLANAR0_PIXELS,
        pixel_values: &[255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255],
        pixel_min: 0,
        pixel_max: 255,
        visual_pattern: "2x2_rgb_red_green_blue_white",
        semantic_note: "RGB samples are interleaved color-by-pixel before JPEG Baseline lossy compression",
        palette: None,
        padding: None,
    },
    PixelRecipe {
        case_id: "classic/sc/mono2_u8_jpeg_ls_lossless",
        recipe_id: "sc_mono2_u8_jpeg_ls_lossless",
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
        transfer_syntax: JPEG_LS_LOSSLESS,
        pixel_bytes: &MONO_PIXELS,
        pixel_values: &[0, 85, 170, 255],
        pixel_min: 0,
        pixel_max: 255,
        visual_pattern: "2x2_monochrome_gradient",
        semantic_note: "minimum sample value displays as black after JPEG-LS Lossless decode",
        palette: None,
        padding: None,
    },
    PixelRecipe {
        case_id: "classic/sc/rgb_planar0_jpegxl_lossless",
        recipe_id: "sc_rgb_planar0_jpegxl_lossless",
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
        transfer_syntax: JPEG_XL_LOSSLESS,
        pixel_bytes: &RGB_PLANAR0_PIXELS,
        pixel_values: &[255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255],
        pixel_min: 0,
        pixel_max: 255,
        visual_pattern: "2x2_rgb_red_green_blue_white",
        semantic_note: "RGB samples are interleaved color-by-pixel before JPEG XL Lossless compression",
        palette: None,
        padding: None,
    },
    PixelRecipe {
        case_id: "classic/sc/mono2_u16_jpeg2000_lossless",
        recipe_id: "sc_mono2_u16_jpeg2000_lossless",
        rows: 2,
        columns: 2,
        photometric_interpretation: "MONOCHROME2",
        samples_per_pixel: 1,
        planar_configuration: None,
        bits_allocated: 16,
        bits_stored: 16,
        high_bit: 15,
        pixel_representation: 0,
        pixel_vr: VR::OB,
        transfer_syntax: JPEG_2000_LOSSLESS,
        pixel_bytes: &MONO_U16_PIXELS,
        pixel_values: &MONO_U16_VALUES,
        pixel_min: 0,
        pixel_max: 65535,
        visual_pattern: "2x2_monochrome_u16_gradient",
        semantic_note: "16-bit unsigned MONOCHROME2 samples span the full stored range after JPEG 2000 Lossless decode",
        palette: None,
        padding: None,
    },
    PixelRecipe {
        case_id: "classic/sc/mono2_u16_htj2k_lossless",
        recipe_id: "sc_mono2_u16_htj2k_lossless",
        rows: 2,
        columns: 2,
        photometric_interpretation: "MONOCHROME2",
        samples_per_pixel: 1,
        planar_configuration: None,
        bits_allocated: 16,
        bits_stored: 16,
        high_bit: 15,
        pixel_representation: 0,
        pixel_vr: VR::OB,
        transfer_syntax: HTJ2K_LOSSLESS,
        pixel_bytes: &MONO_U16_PIXELS,
        pixel_values: &MONO_U16_VALUES,
        pixel_min: 0,
        pixel_max: 65535,
        visual_pattern: "2x2_monochrome_u16_gradient",
        semantic_note: "16-bit unsigned MONOCHROME2 samples span the full stored range after HTJ2K Lossless decode",
        palette: None,
        padding: None,
    },
    PixelRecipe {
        case_id: "classic/sc/mono2_u16_jpeg_lossless_process_14",
        recipe_id: "sc_mono2_u16_jpeg_lossless_process_14",
        rows: 2,
        columns: 2,
        photometric_interpretation: "MONOCHROME2",
        samples_per_pixel: 1,
        planar_configuration: None,
        bits_allocated: 16,
        bits_stored: 16,
        high_bit: 15,
        pixel_representation: 0,
        pixel_vr: VR::OB,
        transfer_syntax: JPEG_LOSSLESS_PROCESS_14,
        pixel_bytes: &MONO_U16_PIXELS,
        pixel_values: &MONO_U16_VALUES,
        pixel_min: 0,
        pixel_max: 65535,
        visual_pattern: "2x2_monochrome_u16_gradient",
        semantic_note: "16-bit unsigned MONOCHROME2 samples span the full stored range after JPEG Lossless Process 14 decode",
        palette: None,
        padding: None,
    },
    PixelRecipe {
        case_id: "classic/sc/mono2_u16_jpeg_lossless_sv1",
        recipe_id: "sc_mono2_u16_jpeg_lossless_sv1",
        rows: 2,
        columns: 2,
        photometric_interpretation: "MONOCHROME2",
        samples_per_pixel: 1,
        planar_configuration: None,
        bits_allocated: 16,
        bits_stored: 16,
        high_bit: 15,
        pixel_representation: 0,
        pixel_vr: VR::OB,
        transfer_syntax: JPEG_LOSSLESS_SV1,
        pixel_bytes: &MONO_U16_PIXELS,
        pixel_values: &MONO_U16_VALUES,
        pixel_min: 0,
        pixel_max: 65535,
        visual_pattern: "2x2_monochrome_u16_gradient",
        semantic_note: "16-bit unsigned MONOCHROME2 samples span the full stored range after JPEG Lossless SV1 decode",
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
        transfer_syntax: EXPLICIT_VR_LITTLE_ENDIAN,
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
        transfer_syntax: EXPLICIT_VR_LITTLE_ENDIAN,
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
        transfer_syntax: EXPLICIT_VR_LITTLE_ENDIAN,
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
        transfer_syntax: EXPLICIT_VR_LITTLE_ENDIAN,
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
        transfer_syntax: EXPLICIT_VR_LITTLE_ENDIAN,
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
        transfer_syntax: EXPLICIT_VR_LITTLE_ENDIAN,
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
        transfer_syntax: EXPLICIT_VR_LITTLE_ENDIAN,
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
        transfer_syntax: EXPLICIT_VR_LITTLE_ENDIAN,
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
        transfer_syntax: EXPLICIT_VR_LITTLE_ENDIAN,
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
        transfer_syntax: EXPLICIT_VR_LITTLE_ENDIAN,
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
    transfer_syntax: TransferSyntaxSpec,
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
    value: i16,
    range_limit: Option<i16>,
}

#[derive(Debug, Clone, Copy)]
struct ClassicCtRecipe {
    case_id: &'static str,
    recipe_id: &'static str,
    transfer_syntax: TransferSyntaxSpec,
    rows: u16,
    columns: u16,
    slices: &'static [ClassicCtSliceRecipe],
    rescale_intercept: &'static str,
    rescale_slope: &'static str,
    rescale_type: &'static str,
    window_center: &'static str,
    window_width: &'static str,
    pixel_spacing: &'static str,
    image_orientation_patient: &'static str,
    slice_thickness: &'static str,
    spacing_between_slices: Option<&'static str>,
    kvp: &'static str,
}

#[derive(Debug, Clone, Copy)]
struct ClassicCtSliceRecipe {
    instance_number: &'static str,
    image_position_patient: &'static str,
    position_along_normal: f64,
    pixel_bytes: &'static [u8],
    pixel_values: &'static [i32],
    pixel_min: i32,
    pixel_max: i32,
}

const CLASSIC_CT_SINGLE_SLICE: &[ClassicCtSliceRecipe] = &[ClassicCtSliceRecipe {
    instance_number: "1",
    image_position_patient: "-0.625\\-0.625\\0",
    position_along_normal: 0.0,
    pixel_bytes: &CT_I16_12BIT_PIXELS,
    pixel_values: &CT_I16_12BIT_VALUES,
    pixel_min: -1024,
    pixel_max: 2047,
}];

const CLASSIC_CT_SORT_CONFLICT_SLICES: &[ClassicCtSliceRecipe] = &[
    ClassicCtSliceRecipe {
        instance_number: "30",
        image_position_patient: "0\\0\\0",
        position_along_normal: 0.0,
        pixel_bytes: &CT_I16_12BIT_PIXELS,
        pixel_values: &CT_I16_12BIT_VALUES,
        pixel_min: -1024,
        pixel_max: 2047,
    },
    ClassicCtSliceRecipe {
        instance_number: "10",
        image_position_patient: "0\\0\\5",
        position_along_normal: 5.0,
        pixel_bytes: &CT_I16_12BIT_PIXELS,
        pixel_values: &CT_I16_12BIT_VALUES,
        pixel_min: -1024,
        pixel_max: 2047,
    },
    ClassicCtSliceRecipe {
        instance_number: "20",
        image_position_patient: "0\\0\\10",
        position_along_normal: 10.0,
        pixel_bytes: &CT_I16_12BIT_PIXELS,
        pixel_values: &CT_I16_12BIT_VALUES,
        pixel_min: -1024,
        pixel_max: 2047,
    },
];

const CLASSIC_CT_RECIPES: &[ClassicCtRecipe] = &[
    ClassicCtRecipe {
        case_id: "classic/ct/mono2_i16_rescale_12bit_explicit_le",
        recipe_id: "ct_mono2_i16_rescale",
        transfer_syntax: EXPLICIT_VR_LITTLE_ENDIAN,
        rows: 2,
        columns: 2,
        slices: CLASSIC_CT_SINGLE_SLICE,
        rescale_intercept: "-1024",
        rescale_slope: "1",
        rescale_type: "HU",
        window_center: "40",
        window_width: "400",
        pixel_spacing: "0.625\\0.625",
        image_orientation_patient: "1\\0\\0\\0\\1\\0",
        slice_thickness: "1",
        spacing_between_slices: None,
        kvp: "120",
    },
    ClassicCtRecipe {
        case_id: "classic/ct/mono2_i16_rescale_12bit_rle_lossless",
        recipe_id: "ct_mono2_i16_rescale_rle_lossless",
        transfer_syntax: RLE_LOSSLESS,
        rows: 2,
        columns: 2,
        slices: CLASSIC_CT_SINGLE_SLICE,
        rescale_intercept: "-1024",
        rescale_slope: "1",
        rescale_type: "HU",
        window_center: "40",
        window_width: "400",
        pixel_spacing: "0.625\\0.625",
        image_orientation_patient: "1\\0\\0\\0\\1\\0",
        slice_thickness: "1",
        spacing_between_slices: None,
        kvp: "120",
    },
    ClassicCtRecipe {
        case_id: "geometry/ct/spatial_sort_conflicts_instance_number",
        recipe_id: "geometry_ct_spatial_sort_conflicts_instance_number",
        transfer_syntax: EXPLICIT_VR_LITTLE_ENDIAN,
        rows: 2,
        columns: 2,
        slices: CLASSIC_CT_SORT_CONFLICT_SLICES,
        rescale_intercept: "-1024",
        rescale_slope: "1",
        rescale_type: "HU",
        window_center: "40",
        window_width: "400",
        pixel_spacing: "0.625\\0.625",
        image_orientation_patient: "1\\0\\0\\0\\1\\0",
        slice_thickness: "5",
        spacing_between_slices: Some("5"),
        kvp: "120",
    },
];

#[derive(Debug, Clone, Copy)]
struct EnhancedCtRecipe {
    case_id: &'static str,
    recipe_id: &'static str,
    rows: u16,
    columns: u16,
    frames: u16,
    pixel_bytes: &'static [u8],
    pixel_values: &'static [i32],
    pixel_min: i32,
    pixel_max: i32,
    pixel_spacing: &'static str,
    image_orientation_patient: &'static str,
    image_position_patient: &'static [&'static str],
    slice_thickness: &'static str,
    spacing_between_slices: &'static str,
    frame_type: &'static str,
    pixel_presentation: &'static str,
    volumetric_properties: &'static str,
    volume_based_calculation_technique: &'static str,
    rescale_intercept: &'static str,
    rescale_slope: &'static str,
    rescale_type: &'static str,
}

#[derive(Debug, Clone, Copy)]
struct EnhancedCtConcatenationPart {
    file_name: &'static str,
    in_concatenation_number: u16,
    concatenation_frame_offset_number: u32,
    image_position_patient: &'static [&'static str],
    dimension_index_values: &'static [u32],
    pixel_bytes: &'static [u8],
    pixel_values: &'static [i32],
}

#[derive(Debug, Clone, Copy)]
struct EnhancedCtConcatenationRecipe {
    base: EnhancedCtRecipe,
    parts: &'static [EnhancedCtConcatenationPart],
}

#[derive(Debug, Clone, Copy)]
struct EnhancedCtConcatenationManifest<'a> {
    concatenation_uid: &'a str,
    in_concatenation_number: u16,
    in_concatenation_total_number: u16,
    concatenation_frame_offset_number: u32,
    sop_instance_uid_of_concatenation_source: &'a str,
}

const ENHANCED_CT_IMAGE_POSITIONS: &[&str] = &["0\\0\\0", "0\\0\\2.5"];
const ENHANCED_CT_DIMENSION_INDEX_VALUES: &[u32] = &[1, 2];
const ENHANCED_CT_CONCAT_PART_1_IMAGE_POSITIONS: &[&str] = &["0\\0\\0"];
const ENHANCED_CT_CONCAT_PART_2_IMAGE_POSITIONS: &[&str] = &["0\\0\\2.5"];
const ENHANCED_CT_CONCAT_PART_1_DIMENSION_INDEX_VALUES: &[u32] = &[1];
const ENHANCED_CT_CONCAT_PART_2_DIMENSION_INDEX_VALUES: &[u32] = &[2];
const ENHANCED_CT_CONCAT_PART_1_PIXELS: [u8; 8] = [0x00, 0x00, 0x64, 0x00, 0xc8, 0x00, 0x2c, 0x01];
const ENHANCED_CT_CONCAT_PART_2_PIXELS: [u8; 8] = [0x90, 0x01, 0xf4, 0x01, 0x58, 0x02, 0xbc, 0x02];
const ENHANCED_CT_CONCAT_PART_1_VALUES: [i32; 4] = [0, 100, 200, 300];
const ENHANCED_CT_CONCAT_PART_2_VALUES: [i32; 4] = [400, 500, 600, 700];

const ENHANCED_CT_RECIPES: &[EnhancedCtRecipe] = &[EnhancedCtRecipe {
    case_id: "enhanced/ct/multiframe_shared_perframe_explicit_le",
    recipe_id: "enhanced_ct_multiframe_shared_perframe",
    rows: 2,
    columns: 2,
    frames: 2,
    pixel_bytes: &ENHANCED_CT_U16_PIXELS,
    pixel_values: &ENHANCED_CT_U16_VALUES,
    pixel_min: 0,
    pixel_max: 700,
    pixel_spacing: "0.75\\0.75",
    image_orientation_patient: "1\\0\\0\\0\\1\\0",
    image_position_patient: ENHANCED_CT_IMAGE_POSITIONS,
    slice_thickness: "2.5",
    spacing_between_slices: "2.5",
    frame_type: "DERIVED\\PRIMARY\\AXIAL\\NONE",
    pixel_presentation: "MONOCHROME",
    volumetric_properties: "VOLUME",
    volume_based_calculation_technique: "NONE",
    rescale_intercept: "-1024",
    rescale_slope: "1",
    rescale_type: "HU",
}];

const ENHANCED_CT_CONCATENATION_PARTS: &[EnhancedCtConcatenationPart] = &[
    EnhancedCtConcatenationPart {
        file_name: "part-001.dcm",
        in_concatenation_number: 1,
        concatenation_frame_offset_number: 0,
        image_position_patient: ENHANCED_CT_CONCAT_PART_1_IMAGE_POSITIONS,
        dimension_index_values: ENHANCED_CT_CONCAT_PART_1_DIMENSION_INDEX_VALUES,
        pixel_bytes: &ENHANCED_CT_CONCAT_PART_1_PIXELS,
        pixel_values: &ENHANCED_CT_CONCAT_PART_1_VALUES,
    },
    EnhancedCtConcatenationPart {
        file_name: "part-002.dcm",
        in_concatenation_number: 2,
        concatenation_frame_offset_number: 1,
        image_position_patient: ENHANCED_CT_CONCAT_PART_2_IMAGE_POSITIONS,
        dimension_index_values: ENHANCED_CT_CONCAT_PART_2_DIMENSION_INDEX_VALUES,
        pixel_bytes: &ENHANCED_CT_CONCAT_PART_2_PIXELS,
        pixel_values: &ENHANCED_CT_CONCAT_PART_2_VALUES,
    },
];

const ENHANCED_CT_CONCATENATION_RECIPES: &[EnhancedCtConcatenationRecipe] =
    &[EnhancedCtConcatenationRecipe {
        base: EnhancedCtRecipe {
            case_id: "enhanced/ct/concatenation_two_part_explicit_le",
            recipe_id: "enhanced_ct_concatenation_two_part",
            rows: 2,
            columns: 2,
            frames: 2,
            pixel_bytes: &ENHANCED_CT_U16_PIXELS,
            pixel_values: &ENHANCED_CT_U16_VALUES,
            pixel_min: 0,
            pixel_max: 700,
            pixel_spacing: "0.75\\0.75",
            image_orientation_patient: "1\\0\\0\\0\\1\\0",
            image_position_patient: ENHANCED_CT_IMAGE_POSITIONS,
            slice_thickness: "2.5",
            spacing_between_slices: "2.5",
            frame_type: "DERIVED\\PRIMARY\\AXIAL\\NONE",
            pixel_presentation: "MONOCHROME",
            volumetric_properties: "VOLUME",
            volume_based_calculation_technique: "NONE",
            rescale_intercept: "-1024",
            rescale_slope: "1",
            rescale_type: "HU",
        },
        parts: ENHANCED_CT_CONCATENATION_PARTS,
    }];

#[derive(Debug, Clone, Copy)]
struct SegmentationRecipe {
    case_id: &'static str,
    recipe_id: &'static str,
    source_case_id: &'static str,
    sop_class_uid: &'static str,
    sop_class_name: &'static str,
    transfer_syntax: TransferSyntaxSpec,
    rows: u16,
    columns: u16,
    frames: u16,
    bits_allocated: u16,
    bits_stored: u16,
    high_bit: u16,
    pixel_data_length_formula: PixelDataLengthFormula,
    pixel_bytes: &'static [u8],
    pixel_values: &'static [i32],
    pixel_min: i32,
    pixel_max: i32,
    referenced_frame_numbers: &'static [u16],
    segmentation_type: &'static str,
    segmentation_fractional_type: Option<&'static str>,
    maximum_fractional_value: Option<u16>,
    segment_label: &'static str,
    visual_pattern: &'static str,
    stressors: &'static [&'static str],
}

const SEGMENTATION_RECIPES: &[SegmentationRecipe] = &[
    SegmentationRecipe {
        case_id: "derived/seg/binary_multiframe_explicit_le",
        recipe_id: "seg_binary_multiframe",
        source_case_id: SEGMENTATION_SOURCE_CASE_ID,
        sop_class_uid: SEGMENTATION_STORAGE_UID,
        sop_class_name: "Segmentation Storage",
        transfer_syntax: EXPLICIT_VR_LITTLE_ENDIAN,
        rows: 2,
        columns: 2,
        frames: 2,
        bits_allocated: 1,
        bits_stored: 1,
        high_bit: 0,
        pixel_data_length_formula: PixelDataLengthFormula::BitPackedFrames,
        pixel_bytes: &SEG_BINARY_PIXELS,
        pixel_values: &SEG_BINARY_VALUES,
        pixel_min: 0,
        pixel_max: 1,
        referenced_frame_numbers: &SEG_REFERENCED_FRAMES,
        segmentation_type: "BINARY",
        segmentation_fractional_type: None,
        maximum_fractional_value: None,
        segment_label: "DTS_SYNTHETIC_REGION",
        visual_pattern: "two_frame_binary_segmentation_mask",
        stressors: &["binary_bit_packed_pixel_data"],
    },
    SegmentationRecipe {
        case_id: "derived/seg/binary_multiframe_deflated_image_frame",
        recipe_id: "seg_binary_multiframe_deflated_image_frame",
        source_case_id: SEGMENTATION_SOURCE_CASE_ID,
        sop_class_uid: SEGMENTATION_STORAGE_UID,
        sop_class_name: "Segmentation Storage",
        transfer_syntax: DEFLATED_IMAGE_FRAME,
        rows: 2,
        columns: 2,
        frames: 2,
        bits_allocated: 1,
        bits_stored: 1,
        high_bit: 0,
        pixel_data_length_formula: PixelDataLengthFormula::BitPackedFrames,
        pixel_bytes: &SEG_BINARY_PIXELS,
        pixel_values: &SEG_BINARY_VALUES,
        pixel_min: 0,
        pixel_max: 1,
        referenced_frame_numbers: &SEG_REFERENCED_FRAMES,
        segmentation_type: "BINARY",
        segmentation_fractional_type: None,
        maximum_fractional_value: None,
        segment_label: "DTS_SYNTHETIC_REGION_DEFLATED",
        visual_pattern: "two_frame_binary_segmentation_mask",
        stressors: &[
            "binary_bit_packed_pixel_data",
            "deflated_image_frame_transfer_syntax",
        ],
    },
    SegmentationRecipe {
        case_id: "derived/seg/fractional_probability_multiframe_explicit_le",
        recipe_id: "seg_fractional_probability_multiframe",
        source_case_id: SEGMENTATION_SOURCE_CASE_ID,
        sop_class_uid: SEGMENTATION_STORAGE_UID,
        sop_class_name: "Segmentation Storage",
        transfer_syntax: EXPLICIT_VR_LITTLE_ENDIAN,
        rows: 2,
        columns: 2,
        frames: 2,
        bits_allocated: 8,
        bits_stored: 8,
        high_bit: 7,
        pixel_data_length_formula: PixelDataLengthFormula::ContiguousSamples,
        pixel_bytes: &SEG_FRACTIONAL_PROBABILITY_PIXELS,
        pixel_values: &SEG_FRACTIONAL_PROBABILITY_VALUES,
        pixel_min: 0,
        pixel_max: 255,
        referenced_frame_numbers: &SEG_REFERENCED_FRAMES,
        segmentation_type: "FRACTIONAL",
        segmentation_fractional_type: Some("PROBABILITY"),
        maximum_fractional_value: Some(255),
        segment_label: "DTS_SYNTHETIC_PROBABILITY",
        visual_pattern: "two_frame_fractional_probability_segmentation",
        stressors: &["fractional_probability_pixel_data"],
    },
    SegmentationRecipe {
        case_id: "derived/seg/labelmap_multiframe_explicit_le",
        recipe_id: "seg_labelmap_multiframe",
        source_case_id: SEGMENTATION_SOURCE_CASE_ID,
        sop_class_uid: LABEL_MAP_SEGMENTATION_STORAGE_UID,
        sop_class_name: "Label Map Segmentation Storage",
        transfer_syntax: EXPLICIT_VR_LITTLE_ENDIAN,
        rows: 2,
        columns: 2,
        frames: 2,
        bits_allocated: 8,
        bits_stored: 8,
        high_bit: 7,
        pixel_data_length_formula: PixelDataLengthFormula::ContiguousSamples,
        pixel_bytes: &SEG_LABELMAP_PIXELS,
        pixel_values: &SEG_LABELMAP_VALUES,
        pixel_min: 0,
        pixel_max: 1,
        referenced_frame_numbers: &SEG_REFERENCED_FRAMES,
        segmentation_type: "LABELMAP",
        segmentation_fractional_type: None,
        maximum_fractional_value: None,
        segment_label: "DTS_SYNTHETIC_LABELMAP",
        visual_pattern: "two_frame_labelmap_segmentation",
        stressors: &["labelmap_pixel_data", "label_map_segmentation_storage"],
    },
];

#[derive(Debug, Clone, Copy)]
struct PresentationStateRecipe {
    case_id: &'static str,
    recipe_id: &'static str,
    source_case_id: &'static str,
    content_label: &'static str,
    content_description: &'static str,
    displayed_area_top_left: [i32; 2],
    displayed_area_bottom_right: [i32; 2],
    presentation_size_mode: &'static str,
    presentation_pixel_aspect_ratio: [i32; 2],
    window_center: &'static str,
    window_width: &'static str,
    window_explanation: &'static str,
    presentation_lut_shape: &'static str,
}

const PRESENTATION_STATE_RECIPES: &[PresentationStateRecipe] = &[PresentationStateRecipe {
    case_id: "derived/presentation-state/grayscale_softcopy_ct_window_explicit_le",
    recipe_id: "gsps_grayscale_softcopy_ct_window",
    source_case_id: GSPS_SOURCE_CASE_ID,
    content_label: "DTSGSPS",
    content_description: "Synthetic CT window presentation state",
    displayed_area_top_left: [1, 1],
    displayed_area_bottom_right: [2, 2],
    presentation_size_mode: "SCALE TO FIT",
    presentation_pixel_aspect_ratio: [1, 1],
    window_center: "350",
    window_width: "1400",
    window_explanation: "DTS CT softcopy window",
    presentation_lut_shape: "IDENTITY",
}];

#[derive(Debug, Clone, Copy)]
struct RealWorldValueMappingRecipe {
    case_id: &'static str,
    recipe_id: &'static str,
    source_case_id: &'static str,
    content_label: &'static str,
    content_description: &'static str,
    lut_label: &'static str,
    first_value_mapped: u16,
    last_value_mapped: u16,
    intercept: f64,
    slope: f64,
    unit_code_value: &'static str,
    unit_coding_scheme_designator: &'static str,
    unit_code_meaning: &'static str,
    referenced_frame_numbers: &'static [u16],
}

const REAL_WORLD_VALUE_MAPPING_RECIPES: &[RealWorldValueMappingRecipe] =
    &[RealWorldValueMappingRecipe {
        case_id: "derived/rwvm/linear_ct_mapping_explicit_le",
        recipe_id: "rwvm_linear_ct_mapping",
        source_case_id: RWVM_SOURCE_CASE_ID,
        content_label: "DTSRWVM",
        content_description: "Synthetic CT linear real world value mapping",
        lut_label: "DTS_HU",
        first_value_mapped: 0,
        last_value_mapped: 700,
        intercept: -1024.0,
        slope: 1.0,
        unit_code_value: "HU",
        unit_coding_scheme_designator: "UCUM",
        unit_code_meaning: "Hounsfield unit",
        referenced_frame_numbers: &SEG_REFERENCED_FRAMES,
    }];

#[derive(Debug, Clone, Copy)]
struct BasicTextSrRecipe {
    case_id: &'static str,
    recipe_id: &'static str,
    source_case_id: &'static str,
    completion_flag: &'static str,
    verification_flag: &'static str,
    root_value_type: &'static str,
    root_continuity_of_content: &'static str,
    title_code_value: &'static str,
    title_coding_scheme_designator: &'static str,
    title_code_meaning: &'static str,
    observation_relationship_type: &'static str,
    observation_value_type: &'static str,
    observation_code_value: &'static str,
    observation_coding_scheme_designator: &'static str,
    observation_code_meaning: &'static str,
    observation_text: &'static str,
}

const BASIC_TEXT_SR_RECIPES: &[BasicTextSrRecipe] = &[BasicTextSrRecipe {
    case_id: "derived/sr/basic_text_observation_explicit_le",
    recipe_id: "sr_basic_text_observation",
    source_case_id: BASIC_TEXT_SR_SOURCE_CASE_ID,
    completion_flag: "COMPLETE",
    verification_flag: "UNVERIFIED",
    root_value_type: "CONTAINER",
    root_continuity_of_content: "SEPARATE",
    title_code_value: "18748-4",
    title_coding_scheme_designator: "LN",
    title_code_meaning: "Diagnostic imaging study",
    observation_relationship_type: "CONTAINS",
    observation_value_type: "TEXT",
    observation_code_value: "121106",
    observation_coding_scheme_designator: "DCM",
    observation_code_meaning: "Comment",
    observation_text: "Synthetic Basic Text SR observation for Enhanced CT source images.",
}];

#[derive(Debug, Clone, Copy)]
struct ComprehensiveSrRecipe {
    case_id: &'static str,
    recipe_id: &'static str,
    source_case_id: &'static str,
    completion_flag: &'static str,
    verification_flag: &'static str,
    root_value_type: &'static str,
    root_continuity_of_content: &'static str,
    title_code_value: &'static str,
    title_coding_scheme_designator: &'static str,
    title_code_meaning: &'static str,
    measurement_relationship_type: &'static str,
    measurement_value_type: &'static str,
    measurement_code_value: &'static str,
    measurement_coding_scheme_designator: &'static str,
    measurement_code_meaning: &'static str,
    numeric_value: &'static str,
    unit_code_value: &'static str,
    unit_coding_scheme_designator: &'static str,
    unit_code_meaning: &'static str,
    image_relationship_type: &'static str,
    image_value_type: &'static str,
    image_code_value: &'static str,
    image_coding_scheme_designator: &'static str,
    image_code_meaning: &'static str,
    referenced_frame_numbers: &'static [u16],
}

const COMPREHENSIVE_SR_RECIPES: &[ComprehensiveSrRecipe] = &[ComprehensiveSrRecipe {
    case_id: "derived/sr/comprehensive_measurement_explicit_le",
    recipe_id: "sr_comprehensive_measurement",
    source_case_id: COMPREHENSIVE_SR_SOURCE_CASE_ID,
    completion_flag: "COMPLETE",
    verification_flag: "UNVERIFIED",
    root_value_type: "CONTAINER",
    root_continuity_of_content: "SEPARATE",
    title_code_value: "18748-4",
    title_coding_scheme_designator: "LN",
    title_code_meaning: "Diagnostic imaging study",
    measurement_relationship_type: "CONTAINS",
    measurement_value_type: "NUM",
    measurement_code_value: "121206",
    measurement_coding_scheme_designator: "DCM",
    measurement_code_meaning: "Distance",
    numeric_value: "12.5",
    unit_code_value: "mm",
    unit_coding_scheme_designator: "UCUM",
    unit_code_meaning: "millimeter",
    image_relationship_type: "CONTAINS",
    image_value_type: "IMAGE",
    image_code_value: "121112",
    image_coding_scheme_designator: "DCM",
    image_code_meaning: "Source image for measurement",
    referenced_frame_numbers: &SR_REFERENCED_FRAMES,
}];

#[derive(Debug, Clone, Copy)]
struct KeyObjectSelectionRecipe {
    case_id: &'static str,
    recipe_id: &'static str,
    image_source_case_id: &'static str,
    seg_source_case_id: &'static str,
    completion_flag: &'static str,
    verification_flag: &'static str,
    root_value_type: &'static str,
    root_continuity_of_content: &'static str,
    title_code_value: &'static str,
    title_coding_scheme_designator: &'static str,
    title_code_meaning: &'static str,
    mapping_resource: &'static str,
    template_identifier: &'static str,
    relationship_type: &'static str,
    image_value_type: &'static str,
    image_referenced_frame_numbers: &'static [u16],
}

const KEY_OBJECT_SELECTION_RECIPES: &[KeyObjectSelectionRecipe] = &[KeyObjectSelectionRecipe {
    case_id: "derived/sr/key_object_selection_explicit_le",
    recipe_id: "sr_key_object_selection",
    image_source_case_id: KEY_OBJECT_SELECTION_IMAGE_SOURCE_CASE_ID,
    seg_source_case_id: KEY_OBJECT_SELECTION_SEG_SOURCE_CASE_ID,
    completion_flag: "COMPLETE",
    verification_flag: "UNVERIFIED",
    root_value_type: "CONTAINER",
    root_continuity_of_content: "SEPARATE",
    title_code_value: "113000",
    title_coding_scheme_designator: "DCM",
    title_code_meaning: "Of Interest",
    mapping_resource: "DCMR",
    template_identifier: "2010",
    relationship_type: "CONTAINS",
    image_value_type: "IMAGE",
    image_referenced_frame_numbers: &KEY_OBJECT_SELECTION_IMAGE_REFERENCED_FRAMES,
}];

#[derive(Debug, Clone, Copy)]
struct RtStructureSetRecipe {
    case_id: &'static str,
    recipe_id: &'static str,
    source_case_id: &'static str,
    structure_set_label: &'static str,
    structure_set_name: &'static str,
    roi_number: u16,
    roi_name: &'static str,
    roi_generation_algorithm: &'static str,
    roi_generation_description: &'static str,
    roi_display_color: [i32; 3],
    contour_number: u16,
    contour_geometric_type: &'static str,
    contour_points: u16,
    contour_data: &'static str,
    roi_interpreted_type: &'static str,
    roi_interpreter: &'static str,
}

const RT_STRUCTURE_SET_RECIPES: &[RtStructureSetRecipe] = &[RtStructureSetRecipe {
    case_id: "non-image/rt/structure_set_single_roi_explicit_le",
    recipe_id: "rt_structure_set_single_roi",
    source_case_id: RT_STRUCTURE_SET_SOURCE_CASE_ID,
    structure_set_label: "DTS_RTSTRUCT",
    structure_set_name: "DTS synthetic single ROI",
    roi_number: 1,
    roi_name: "DTS_SYNTHETIC_ROI",
    roi_generation_algorithm: "MANUAL",
    roi_generation_description: "Synthetic closed planar contour for viewer detection.",
    roi_display_color: [255, 64, 64],
    contour_number: 1,
    contour_geometric_type: "CLOSED_PLANAR",
    contour_points: 4,
    contour_data: "0\\0\\0\\1.5\\0\\0\\1.5\\1.5\\0\\0\\1.5\\0",
    roi_interpreted_type: "ORGAN",
    roi_interpreter: "",
}];

#[derive(Debug, Clone, Copy)]
struct RtDoseRecipe {
    case_id: &'static str,
    recipe_id: &'static str,
    image_source_case_id: &'static str,
    structure_set_source_case_id: &'static str,
    rows: u16,
    columns: u16,
    frames: u16,
    pixel_bytes: &'static [u8],
    pixel_min: i32,
    pixel_max: i32,
    pixel_spacing: &'static str,
    image_orientation_patient: &'static str,
    image_position_patient: &'static str,
    slice_thickness: &'static str,
    frame_increment_pointer: Tag,
    grid_frame_offset_vector: &'static str,
    dose_units: &'static str,
    dose_type: &'static str,
    dose_summation_type: &'static str,
    dose_grid_scaling: &'static str,
}

const RT_DOSE_RECIPES: &[RtDoseRecipe] = &[RtDoseRecipe {
    case_id: "non-image/rt/dose_grid_u16_explicit_le",
    recipe_id: "rt_dose_grid_u16",
    image_source_case_id: RT_DOSE_IMAGE_SOURCE_CASE_ID,
    structure_set_source_case_id: RT_DOSE_STRUCTURE_SET_SOURCE_CASE_ID,
    rows: 2,
    columns: 2,
    frames: 2,
    pixel_bytes: &RT_DOSE_GRID_PIXELS,
    pixel_min: 0,
    pixel_max: 700,
    pixel_spacing: "1\\1",
    image_orientation_patient: "1\\0\\0\\0\\1\\0",
    image_position_patient: "0\\0\\0",
    slice_thickness: "2.5",
    frame_increment_pointer: tags::GRID_FRAME_OFFSET_VECTOR,
    grid_frame_offset_vector: "0\\2.5",
    dose_units: "GY",
    dose_type: "PHYSICAL",
    dose_summation_type: "RECORD",
    dose_grid_scaling: "0.001",
}];

#[derive(Debug, Clone, Copy)]
struct EncapsulatedPdfRecipe {
    case_id: &'static str,
    recipe_id: &'static str,
    document_title: &'static str,
    mime_type: &'static str,
    document_bytes: &'static [u8],
    burned_in_annotation: &'static str,
    recognizable_visual_features: &'static str,
}

const ENCAPSULATED_PDF_RECIPES: &[EncapsulatedPdfRecipe] = &[EncapsulatedPdfRecipe {
    case_id: "non-image/encapsulated-document/pdf_minimal_explicit_le",
    recipe_id: "encapsulated_pdf_minimal",
    document_title: "DTS Minimal Synthetic PDF",
    mime_type: "application/pdf",
    document_bytes: MINIMAL_PDF_BYTES,
    burned_in_annotation: "NO",
    recognizable_visual_features: "NO",
}];

#[derive(Debug, Clone, Copy)]
struct EnhancedMrRecipe {
    case_id: &'static str,
    recipe_id: &'static str,
    rows: u16,
    columns: u16,
    frames: u16,
    pixel_bytes: &'static [u8],
    pixel_values: &'static [i32],
    pixel_min: i32,
    pixel_max: i32,
    pixel_spacing: &'static str,
    image_orientation_patient: &'static str,
    image_position_patient: &'static [&'static str],
    slice_thickness: &'static str,
    spacing_between_slices: &'static str,
    frame_type: &'static str,
    pixel_presentation: &'static str,
    volumetric_properties: &'static str,
    volume_based_calculation_technique: &'static str,
    rescale_intercept: &'static str,
    rescale_slope: &'static str,
    rescale_type: &'static str,
    repetition_time: &'static str,
    flip_angle: &'static str,
    echo_train_length: &'static str,
    rf_echo_train_length: u16,
    gradient_echo_train_length: u16,
    effective_echo_times: Option<&'static [f64]>,
    temporal_position_time_offsets: Option<&'static [f64]>,
    velocity_encoding_directions: Option<&'static [[f64; 3]]>,
    velocity_encoding_minimum_value: Option<f64>,
    velocity_encoding_maximum_value: Option<f64>,
}

const ENHANCED_MR_IMAGE_POSITIONS: &[&str] = &["0\\0\\0", "0\\0\\4"];
const ENHANCED_MR_EFFECTIVE_ECHO_TIMES: &[f64] = &[12.5, 24.5];
const ENHANCED_MR_TEMPORAL_IMAGE_POSITIONS: &[&str] = &["0\\0\\0", "0\\0\\0"];
const ENHANCED_MR_TEMPORAL_POSITION_TIME_OFFSETS: &[f64] = &[0.0, 1.5];
const ENHANCED_MR_PHASE_IMAGE_POSITIONS: &[&str] = &["0\\0\\0", "0\\0\\0"];
const ENHANCED_MR_VELOCITY_ENCODING_DIRECTIONS: &[[f64; 3]] = &[[1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];

const ENHANCED_MR_RECIPES: &[EnhancedMrRecipe] = &[
    EnhancedMrRecipe {
        case_id: "enhanced/mr/multiframe_echo_perframe_explicit_le",
        recipe_id: "enhanced_mr_multiframe_echo_perframe",
        rows: 2,
        columns: 2,
        frames: 2,
        pixel_bytes: &ENHANCED_MR_U16_PIXELS,
        pixel_values: &ENHANCED_MR_U16_VALUES,
        pixel_min: 0,
        pixel_max: 350,
        pixel_spacing: "1.000\\1.000",
        image_orientation_patient: "1\\0\\0\\0\\1\\0",
        image_position_patient: ENHANCED_MR_IMAGE_POSITIONS,
        slice_thickness: "4",
        spacing_between_slices: "4",
        frame_type: "DERIVED\\PRIMARY\\STATIC\\NONE",
        pixel_presentation: "MONOCHROME",
        volumetric_properties: "VOLUME",
        volume_based_calculation_technique: "NONE",
        rescale_intercept: "0",
        rescale_slope: "1",
        rescale_type: "US",
        repetition_time: "2000",
        flip_angle: "90",
        echo_train_length: "1",
        rf_echo_train_length: 1,
        gradient_echo_train_length: 0,
        effective_echo_times: Some(ENHANCED_MR_EFFECTIVE_ECHO_TIMES),
        temporal_position_time_offsets: None,
        velocity_encoding_directions: None,
        velocity_encoding_minimum_value: None,
        velocity_encoding_maximum_value: None,
    },
    EnhancedMrRecipe {
        case_id: "enhanced/mr/multiframe_temporal_position_explicit_le",
        recipe_id: "enhanced_mr_multiframe_temporal_position",
        rows: 2,
        columns: 2,
        frames: 2,
        pixel_bytes: &ENHANCED_MR_TEMPORAL_U16_PIXELS,
        pixel_values: &ENHANCED_MR_TEMPORAL_U16_VALUES,
        pixel_min: 0,
        pixel_max: 225,
        pixel_spacing: "1.000\\1.000",
        image_orientation_patient: "1\\0\\0\\0\\1\\0",
        image_position_patient: ENHANCED_MR_TEMPORAL_IMAGE_POSITIONS,
        slice_thickness: "4",
        spacing_between_slices: "4",
        frame_type: "DERIVED\\PRIMARY\\DYNAMIC\\NONE",
        pixel_presentation: "MONOCHROME",
        volumetric_properties: "VOLUME",
        volume_based_calculation_technique: "NONE",
        rescale_intercept: "0",
        rescale_slope: "1",
        rescale_type: "US",
        repetition_time: "1500",
        flip_angle: "90",
        echo_train_length: "1",
        rf_echo_train_length: 1,
        gradient_echo_train_length: 0,
        effective_echo_times: None,
        temporal_position_time_offsets: Some(ENHANCED_MR_TEMPORAL_POSITION_TIME_OFFSETS),
        velocity_encoding_directions: None,
        velocity_encoding_minimum_value: None,
        velocity_encoding_maximum_value: None,
    },
    EnhancedMrRecipe {
        case_id: "enhanced/mr/multiframe_phase_velocity_encoding_explicit_le",
        recipe_id: "enhanced_mr_multiframe_phase_velocity_encoding",
        rows: 2,
        columns: 2,
        frames: 2,
        pixel_bytes: &ENHANCED_MR_PHASE_U16_PIXELS,
        pixel_values: &ENHANCED_MR_PHASE_U16_VALUES,
        pixel_min: 0,
        pixel_max: 280,
        pixel_spacing: "1.000\\1.000",
        image_orientation_patient: "1\\0\\0\\0\\1\\0",
        image_position_patient: ENHANCED_MR_PHASE_IMAGE_POSITIONS,
        slice_thickness: "4",
        spacing_between_slices: "4",
        frame_type: "DERIVED\\PRIMARY\\DYNAMIC\\NONE",
        pixel_presentation: "MONOCHROME",
        volumetric_properties: "VOLUME",
        volume_based_calculation_technique: "NONE",
        rescale_intercept: "0",
        rescale_slope: "1",
        rescale_type: "US",
        repetition_time: "1500",
        flip_angle: "90",
        echo_train_length: "1",
        rf_echo_train_length: 1,
        gradient_echo_train_length: 0,
        effective_echo_times: None,
        temporal_position_time_offsets: None,
        velocity_encoding_directions: Some(ENHANCED_MR_VELOCITY_ENCODING_DIRECTIONS),
        velocity_encoding_minimum_value: Some(-150.0),
        velocity_encoding_maximum_value: Some(150.0),
    },
];

#[derive(Debug, Clone, Copy)]
struct ClassicMgRecipe {
    case_id: &'static str,
    recipe_id: &'static str,
    sop_class_uid: &'static str,
    sop_class_name: &'static str,
    transfer_syntax: TransferSyntaxSpec,
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
        transfer_syntax: EXPLICIT_VR_LITTLE_ENDIAN,
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
        case_id: "classic/mg/for_presentation_mono1_u16_12bit_rle_lossless",
        recipe_id: "mg_for_presentation_mono1_u16_rle_lossless",
        sop_class_uid: uids::DIGITAL_MAMMOGRAPHY_X_RAY_IMAGE_STORAGE_FOR_PRESENTATION,
        sop_class_name: "Digital Mammography X-Ray Image Storage - For Presentation",
        transfer_syntax: RLE_LOSSLESS,
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
        transfer_syntax: IMPLICIT_VR_LITTLE_ENDIAN,
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
    ClassicMgRecipe {
        case_id: "classic/mg/for_processing_mono2_u16_12bit_rle_lossless",
        recipe_id: "mg_for_processing_mono2_u16_rle_lossless",
        sop_class_uid: uids::DIGITAL_MAMMOGRAPHY_X_RAY_IMAGE_STORAGE_FOR_PROCESSING,
        sop_class_name: "Digital Mammography X-Ray Image Storage - For Processing",
        transfer_syntax: RLE_LOSSLESS,
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
    transfer_syntax: TransferSyntaxSpec,
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

const CLASSIC_DX_RECIPES: &[ClassicDxRecipe] = &[
    ClassicDxRecipe {
        case_id: "classic/dx/display_shutter_mono2_u16_explicit_le",
        recipe_id: "dx_display_shutter_mono2_u16",
        transfer_syntax: EXPLICIT_VR_LITTLE_ENDIAN,
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
    },
    ClassicDxRecipe {
        case_id: "classic/dx/display_shutter_mono2_u16_rle_lossless",
        recipe_id: "dx_display_shutter_mono2_u16_rle_lossless",
        transfer_syntax: RLE_LOSSLESS,
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
    },
];

#[derive(Debug, Clone, Copy)]
struct ClassicUsRecipe {
    case_id: &'static str,
    recipe_id: &'static str,
    transfer_syntax: TransferSyntaxSpec,
    rows: u16,
    columns: u16,
    pixel_bytes: &'static [u8],
    pixel_values: &'static [i32],
    pixel_min: i32,
    pixel_max: i32,
    lossy_image_compression: &'static str,
    ultrasound_color_data_present: u16,
}

const CLASSIC_US_RECIPES: &[ClassicUsRecipe] = &[
    ClassicUsRecipe {
        case_id: "classic/us/mono2_u8_explicit_le",
        recipe_id: "us_mono2_u8",
        transfer_syntax: EXPLICIT_VR_LITTLE_ENDIAN,
        rows: 2,
        columns: 2,
        pixel_bytes: &MONO_PIXELS,
        pixel_values: &[0, 85, 170, 255],
        pixel_min: 0,
        pixel_max: 255,
        lossy_image_compression: "00",
        ultrasound_color_data_present: 0,
    },
    ClassicUsRecipe {
        case_id: "classic/us/mono2_u8_rle_lossless",
        recipe_id: "us_mono2_u8_rle_lossless",
        transfer_syntax: RLE_LOSSLESS,
        rows: 2,
        columns: 2,
        pixel_bytes: &MONO_PIXELS,
        pixel_values: &[0, 85, 170, 255],
        pixel_min: 0,
        pixel_max: 255,
        lossy_image_compression: "00",
        ultrasound_color_data_present: 0,
    },
];

#[derive(Debug, Clone, Copy)]
struct ClassicCrRecipe {
    case_id: &'static str,
    recipe_id: &'static str,
    transfer_syntax: TransferSyntaxSpec,
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

const CLASSIC_CR_RECIPES: &[ClassicCrRecipe] = &[
    ClassicCrRecipe {
        case_id: "classic/cr/overlay_modality_voi_explicit_le",
        recipe_id: "cr_overlay_modality_voi",
        transfer_syntax: EXPLICIT_VR_LITTLE_ENDIAN,
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
    },
    ClassicCrRecipe {
        case_id: "classic/cr/overlay_modality_voi_rle_lossless",
        recipe_id: "cr_overlay_modality_voi_rle_lossless",
        transfer_syntax: RLE_LOSSLESS,
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
    },
];

#[derive(Debug, Clone, Copy)]
struct ClassicMrRecipe {
    case_id: &'static str,
    recipe_id: &'static str,
    transfer_syntax: TransferSyntaxSpec,
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

const CLASSIC_MR_RLE_SLICES: &[ClassicMrSliceRecipe] = &[ClassicMrSliceRecipe {
    instance_number: "1",
    image_position_patient: "0\\0\\0",
    slice_location: "0",
    position_along_normal: 0.0,
    pixel_bytes: &MR_SLICE_1_PIXELS,
    pixel_values: &MR_SLICE_1_VALUES,
    pixel_min: 0,
    pixel_max: 3,
}];

const CLASSIC_MR_RECIPES: &[ClassicMrRecipe] = &[
    ClassicMrRecipe {
        case_id: "classic/mr/multislice_oblique_explicit_le",
        recipe_id: "mr_multislice_oblique",
        transfer_syntax: EXPLICIT_VR_LITTLE_ENDIAN,
        rows: 2,
        columns: 2,
        pixel_spacing: "1.000\\1.000",
        image_orientation_patient: "0.70710678\\0.70710678\\0\\0\\0\\1",
        slice_thickness: "5",
        spacing_between_slices: "5",
        slices: CLASSIC_MR_SLICES,
    },
    ClassicMrRecipe {
        case_id: "classic/mr/mono2_u16_rle_lossless",
        recipe_id: "mr_mono2_u16_rle_lossless",
        transfer_syntax: RLE_LOSSLESS,
        rows: 2,
        columns: 2,
        pixel_spacing: "1.000\\1.000",
        image_orientation_patient: "1\\0\\0\\0\\1\\0",
        slice_thickness: "5",
        spacing_between_slices: "5",
        slices: CLASSIC_MR_RLE_SLICES,
    },
];

#[derive(Debug, Clone)]
pub(crate) struct GeneratedFile {
    pub case_id: String,
    pub manifest_entry: Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GeneratedSourceObject {
    pub source_case_id: String,
    pub source_path: String,
    pub study_instance_uid: String,
    pub sop_class_uid: String,
    pub sop_instance_uid: String,
    pub series_instance_uid: Option<String>,
    pub frame_of_reference_uid: Option<String>,
    pub frame_count: Option<u64>,
}

impl GeneratedSourceObject {
    fn from_generated_file(file: &GeneratedFile) -> Result<Self, GenerateError> {
        let source_path = generated_manifest_str(
            &file.manifest_entry,
            "/path",
            "generated file manifest path must be a string",
        )?;
        let source_case_id = generated_manifest_str(
            &file.manifest_entry,
            "/case_id",
            "generated file manifest case_id must be a string",
        )?;
        if source_case_id != file.case_id {
            return Err(GenerateError::MetadataShape {
                path: PathBuf::from(source_path),
                message: "generated file case_id must match manifest case_id",
            });
        }
        let sop_class_uid = generated_manifest_str(
            &file.manifest_entry,
            "/dicom/sop_class_uid",
            "generated file manifest dicom sop_class_uid must be a string",
        )?;
        let study_instance_uid = generated_manifest_str(
            &file.manifest_entry,
            "/uids/study_instance_uid",
            "generated file manifest uids study_instance_uid must be a string",
        )?;
        let sop_instance_uid = generated_manifest_str(
            &file.manifest_entry,
            "/uids/sop_instance_uid",
            "generated file manifest uids sop_instance_uid must be a string",
        )?;
        let series_instance_uid = file
            .manifest_entry
            .pointer("/uids/series_instance_uid")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned);
        let frame_of_reference_uid = file
            .manifest_entry
            .pointer("/uids/frame_of_reference_uid")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned);
        let frame_count = file
            .manifest_entry
            .pointer("/image/frames")
            .and_then(Value::as_u64);

        Ok(Self {
            source_case_id: source_case_id.to_string(),
            source_path: source_path.to_string(),
            study_instance_uid: study_instance_uid.to_string(),
            sop_class_uid: sop_class_uid.to_string(),
            sop_instance_uid: sop_instance_uid.to_string(),
            series_instance_uid,
            frame_of_reference_uid,
            frame_count,
        })
    }

    #[allow(dead_code)]
    pub(crate) fn to_manifest_reference(
        &self,
        relationship: &str,
        frame_numbers: Option<Vec<u64>>,
    ) -> Value {
        let mut reference = serde_json::json!({
            "relationship": relationship,
            "source_case_id": self.source_case_id.as_str(),
            "source_path": self.source_path.as_str(),
            "sop_class_uid": self.sop_class_uid.as_str(),
            "sop_instance_uid": self.sop_instance_uid.as_str()
        });
        if let Some(object) = reference.as_object_mut() {
            if let Some(series_instance_uid) = &self.series_instance_uid {
                object.insert(
                    "series_instance_uid".to_string(),
                    Value::String(series_instance_uid.clone()),
                );
            }
            if let Some(frame_numbers) = frame_numbers {
                object.insert(
                    "frame_numbers".to_string(),
                    serde_json::json!(frame_numbers),
                );
            }
        }
        reference
    }
}

#[derive(Debug, Default)]
struct GeneratedSourceRegistry {
    objects: Vec<GeneratedSourceObject>,
    by_path: BTreeMap<String, usize>,
    by_case_id: BTreeMap<String, Vec<usize>>,
}

impl GeneratedSourceRegistry {
    fn register(&mut self, file: &GeneratedFile) -> Result<(), GenerateError> {
        let source = GeneratedSourceObject::from_generated_file(file)?;
        if self.by_path.contains_key(&source.source_path) {
            return Err(GenerateError::MetadataShape {
                path: PathBuf::from(&source.source_path),
                message: "generated source object path must be unique",
            });
        }

        let index = self.objects.len();
        self.by_path.insert(source.source_path.clone(), index);
        self.by_case_id
            .entry(source.source_case_id.clone())
            .or_default()
            .push(index);
        self.objects.push(source);
        Ok(())
    }

    #[allow(dead_code)]
    pub(crate) fn by_path(&self, source_path: &str) -> Option<&GeneratedSourceObject> {
        self.by_path
            .get(source_path)
            .map(|index| &self.objects[*index])
    }

    #[allow(dead_code)]
    pub(crate) fn sources_for_case(
        &self,
        case_id: &str,
    ) -> impl Iterator<Item = &GeneratedSourceObject> {
        self.by_case_id
            .get(case_id)
            .into_iter()
            .flat_map(|indexes| indexes.iter())
            .map(|index| &self.objects[*index])
    }

    #[allow(dead_code)]
    pub(crate) fn first_for_case(&self, case_id: &str) -> Option<&GeneratedSourceObject> {
        self.sources_for_case(case_id).next()
    }
}

#[derive(Debug, Default)]
struct GenerationContext {
    generated_files: Vec<GeneratedFile>,
    source_registry: GeneratedSourceRegistry,
}

impl GenerationContext {
    fn record_one(&mut self, file: GeneratedFile) -> Result<(), GenerateError> {
        self.source_registry.register(&file)?;
        self.generated_files.push(file);
        Ok(())
    }

    fn record_many(&mut self, files: Vec<GeneratedFile>) -> Result<(), GenerateError> {
        for file in files {
            self.record_one(file)?;
        }
        Ok(())
    }

    #[allow(dead_code)]
    pub(crate) fn source_registry(&self) -> &GeneratedSourceRegistry {
        &self.source_registry
    }

    fn into_generated_files(self) -> Vec<GeneratedFile> {
        self.generated_files
    }
}

pub(crate) fn write_supported_cases(
    run: &PreparedGenerationRun,
    registry: &Value,
    standards_lock_sha256: &str,
) -> Result<Vec<GeneratedFile>, GenerateError> {
    let mut context = GenerationContext::default();
    for recipe in PIXEL_RECIPES {
        let Some(case) = registry_case(registry, recipe.case_id)? else {
            continue;
        };
        if !should_generate_case(case, run)? {
            continue;
        }
        context.record_one(write_pixel_case(run, case, *recipe, standards_lock_sha256)?)?;
    }
    for recipe in CLASSIC_CT_RECIPES {
        let Some(case) = registry_case(registry, recipe.case_id)? else {
            continue;
        };
        if !should_generate_case(case, run)? {
            continue;
        }
        context.record_many(write_classic_ct_case(
            run,
            case,
            *recipe,
            standards_lock_sha256,
        )?)?;
    }
    for recipe in ENHANCED_CT_RECIPES {
        let Some(case) = registry_case(registry, recipe.case_id)? else {
            continue;
        };
        if !should_generate_case(case, run)? {
            continue;
        }
        context.record_one(write_enhanced_ct_case(
            run,
            case,
            *recipe,
            standards_lock_sha256,
        )?)?;
    }
    for recipe in SEGMENTATION_RECIPES {
        let Some(case) = registry_case(registry, recipe.case_id)? else {
            continue;
        };
        if !should_generate_case(case, run)? {
            continue;
        }
        let source = context
            .source_registry()
            .first_for_case(recipe.source_case_id)
            .cloned()
            .ok_or_else(|| GenerateError::MetadataShape {
                path: PathBuf::from(recipe.case_id),
                message: "segmentation source object must be generated before the derived recipe",
            })?;
        context.record_one(write_segmentation_case(
            run,
            case,
            *recipe,
            &source,
            standards_lock_sha256,
        )?)?;
    }
    for recipe in PRESENTATION_STATE_RECIPES {
        let Some(case) = registry_case(registry, recipe.case_id)? else {
            continue;
        };
        if !should_generate_case(case, run)? {
            continue;
        }
        let source = context
            .source_registry()
            .first_for_case(recipe.source_case_id)
            .cloned()
            .ok_or_else(|| GenerateError::MetadataShape {
                path: PathBuf::from(recipe.case_id),
                message: "presentation state source object must be generated before the derived recipe",
            })?;
        context.record_one(write_presentation_state_case(
            run,
            case,
            *recipe,
            &source,
            standards_lock_sha256,
        )?)?;
    }
    for recipe in REAL_WORLD_VALUE_MAPPING_RECIPES {
        let Some(case) = registry_case(registry, recipe.case_id)? else {
            continue;
        };
        if !should_generate_case(case, run)? {
            continue;
        }
        let source = context
            .source_registry()
            .first_for_case(recipe.source_case_id)
            .cloned()
            .ok_or_else(|| GenerateError::MetadataShape {
                path: PathBuf::from(recipe.case_id),
                message: "RWVM source object must be generated before the derived recipe",
            })?;
        context.record_one(write_real_world_value_mapping_case(
            run,
            case,
            *recipe,
            &source,
            standards_lock_sha256,
        )?)?;
    }
    for recipe in BASIC_TEXT_SR_RECIPES {
        let Some(case) = registry_case(registry, recipe.case_id)? else {
            continue;
        };
        if !should_generate_case(case, run)? {
            continue;
        }
        let source = context
            .source_registry()
            .first_for_case(recipe.source_case_id)
            .cloned()
            .ok_or_else(|| GenerateError::MetadataShape {
                path: PathBuf::from(recipe.case_id),
                message: "Basic Text SR source object must be generated before the derived recipe",
            })?;
        context.record_one(write_basic_text_sr_case(
            run,
            case,
            *recipe,
            &source,
            standards_lock_sha256,
        )?)?;
    }
    for recipe in COMPREHENSIVE_SR_RECIPES {
        let Some(case) = registry_case(registry, recipe.case_id)? else {
            continue;
        };
        if !should_generate_case(case, run)? {
            continue;
        }
        let source = context
            .source_registry()
            .first_for_case(recipe.source_case_id)
            .cloned()
            .ok_or_else(|| GenerateError::MetadataShape {
                path: PathBuf::from(recipe.case_id),
                message: "Comprehensive SR source object must be generated before the derived recipe",
            })?;
        context.record_one(write_comprehensive_sr_case(
            run,
            case,
            *recipe,
            &source,
            standards_lock_sha256,
        )?)?;
    }
    for recipe in KEY_OBJECT_SELECTION_RECIPES {
        let Some(case) = registry_case(registry, recipe.case_id)? else {
            continue;
        };
        if !should_generate_case(case, run)? {
            continue;
        }
        let image_source = context
            .source_registry()
            .first_for_case(recipe.image_source_case_id)
            .cloned()
            .ok_or_else(|| GenerateError::MetadataShape {
                path: PathBuf::from(recipe.case_id),
                message: "KOS image source object must be generated before the derived recipe",
            })?;
        let seg_source = context
            .source_registry()
            .first_for_case(recipe.seg_source_case_id)
            .cloned()
            .ok_or_else(|| GenerateError::MetadataShape {
                path: PathBuf::from(recipe.case_id),
                message: "KOS SEG source object must be generated before the derived recipe",
            })?;
        context.record_one(write_key_object_selection_case(
            run,
            case,
            *recipe,
            &image_source,
            &seg_source,
            standards_lock_sha256,
        )?)?;
    }
    for recipe in RT_STRUCTURE_SET_RECIPES {
        let Some(case) = registry_case(registry, recipe.case_id)? else {
            continue;
        };
        if !should_generate_case(case, run)? {
            continue;
        }
        let source = context
            .source_registry()
            .first_for_case(recipe.source_case_id)
            .cloned()
            .ok_or_else(|| GenerateError::MetadataShape {
                path: PathBuf::from(recipe.case_id),
                message: "RT Structure Set source object must be generated before the derived recipe",
            })?;
        context.record_one(write_rt_structure_set_case(
            run,
            case,
            *recipe,
            &source,
            standards_lock_sha256,
        )?)?;
    }
    for recipe in RT_DOSE_RECIPES {
        let Some(case) = registry_case(registry, recipe.case_id)? else {
            continue;
        };
        if !should_generate_case(case, run)? {
            continue;
        }
        let image_source = context
            .source_registry()
            .first_for_case(recipe.image_source_case_id)
            .cloned()
            .ok_or_else(|| GenerateError::MetadataShape {
                path: PathBuf::from(recipe.case_id),
                message: "RT Dose image source object must be generated before the derived recipe",
            })?;
        let structure_set_source = context
            .source_registry()
            .first_for_case(recipe.structure_set_source_case_id)
            .cloned()
            .ok_or_else(|| GenerateError::MetadataShape {
                path: PathBuf::from(recipe.case_id),
                message: "RT Dose Structure Set source object must be generated before the derived recipe",
            })?;
        context.record_one(write_rt_dose_case(
            run,
            case,
            *recipe,
            &image_source,
            &structure_set_source,
            standards_lock_sha256,
        )?)?;
    }
    for recipe in ENCAPSULATED_PDF_RECIPES {
        let Some(case) = registry_case(registry, recipe.case_id)? else {
            continue;
        };
        if !should_generate_case(case, run)? {
            continue;
        }
        context.record_one(write_encapsulated_pdf_case(
            run,
            case,
            *recipe,
            standards_lock_sha256,
        )?)?;
    }
    for recipe in ENHANCED_CT_CONCATENATION_RECIPES {
        let Some(case) = registry_case(registry, recipe.base.case_id)? else {
            continue;
        };
        if !should_generate_case(case, run)? {
            continue;
        }
        context.record_many(write_enhanced_ct_concatenation_case(
            run,
            case,
            *recipe,
            standards_lock_sha256,
        )?)?;
    }
    for recipe in ENHANCED_MR_RECIPES {
        let Some(case) = registry_case(registry, recipe.case_id)? else {
            continue;
        };
        if !should_generate_case(case, run)? {
            continue;
        }
        context.record_one(write_enhanced_mr_case(
            run,
            case,
            *recipe,
            standards_lock_sha256,
        )?)?;
    }
    for recipe in CLASSIC_MG_RECIPES {
        let Some(case) = registry_case(registry, recipe.case_id)? else {
            continue;
        };
        if !should_generate_case(case, run)? {
            continue;
        }
        context.record_one(write_classic_mg_case(
            run,
            case,
            *recipe,
            standards_lock_sha256,
        )?)?;
    }
    for recipe in CLASSIC_DX_RECIPES {
        let Some(case) = registry_case(registry, recipe.case_id)? else {
            continue;
        };
        if !should_generate_case(case, run)? {
            continue;
        }
        context.record_one(write_classic_dx_case(
            run,
            case,
            *recipe,
            standards_lock_sha256,
        )?)?;
    }
    for recipe in CLASSIC_US_RECIPES {
        let Some(case) = registry_case(registry, recipe.case_id)? else {
            continue;
        };
        if !should_generate_case(case, run)? {
            continue;
        }
        context.record_one(write_classic_us_case(
            run,
            case,
            *recipe,
            standards_lock_sha256,
        )?)?;
    }
    for recipe in CLASSIC_CR_RECIPES {
        let Some(case) = registry_case(registry, recipe.case_id)? else {
            continue;
        };
        if !should_generate_case(case, run)? {
            continue;
        }
        context.record_one(write_classic_cr_case(
            run,
            case,
            *recipe,
            standards_lock_sha256,
        )?)?;
    }
    for recipe in CLASSIC_MR_RECIPES {
        let Some(case) = registry_case(registry, recipe.case_id)? else {
            continue;
        };
        if !should_generate_case(case, run)? {
            continue;
        }
        context.record_many(write_classic_mr_case(
            run,
            case,
            *recipe,
            standards_lock_sha256,
        )?)?;
    }
    Ok(context.into_generated_files())
}

fn generated_manifest_str<'a>(
    entry: &'a Value,
    pointer: &str,
    message: &'static str,
) -> Result<&'a str, GenerateError> {
    entry
        .pointer(pointer)
        .and_then(Value::as_str)
        .ok_or_else(|| GenerateError::MetadataShape {
            path: PathBuf::from("generated manifest entry"),
            message,
        })
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
    let sop_class_uid = pixel_sop_class_uid(recipe);
    let is_vl_photographic = pixel_is_vl_photographic(recipe);
    put_str(&mut obj, tags::SOP_CLASS_UID, VR::UI, sop_class_uid);
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

    put_str(&mut obj, tags::MODALITY, VR::CS, pixel_modality(recipe));
    put_str(
        &mut obj,
        tags::SERIES_INSTANCE_UID,
        VR::UI,
        &series_instance_uid,
    );
    put_str(&mut obj, tags::SERIES_NUMBER, VR::IS, "1");

    if !is_vl_photographic {
        put_str(&mut obj, tags::CONVERSION_TYPE, VR::CS, "SYN");
    }
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
    if is_vl_photographic {
        put_str(&mut obj, tags::IMAGE_TYPE, VR::CS, "ORIGINAL\\PRIMARY");
        put_str(&mut obj, tags::LOSSY_IMAGE_COMPRESSION, VR::CS, "00");
        put_empty_sequence(&mut obj, tags::ACQUISITION_CONTEXT_SEQUENCE);
    }

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
    let frame_bytes =
        pixel_recipe_frame_bytes(recipe).map_err(|message| GenerateError::WriteDicomFile {
            path: path.clone(),
            message,
        })?;
    let frame_count =
        u16::try_from(frame_bytes.len()).map_err(|_| GenerateError::WriteDicomFile {
            path: path.clone(),
            message: "pixel recipe frame count exceeded u16".to_string(),
        })?;
    if frame_count > 1 {
        put_str(
            &mut obj,
            tags::NUMBER_OF_FRAMES,
            VR::IS,
            &frame_count.to_string(),
        );
    }
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
        put_pixel_padding(
            &mut obj,
            tags::PIXEL_PADDING_VALUE,
            padding.value,
            recipe.pixel_representation,
        );
        if let Some(range_limit) = padding.range_limit {
            put_pixel_padding(
                &mut obj,
                tags::PIXEL_PADDING_RANGE_LIMIT,
                range_limit,
                recipe.pixel_representation,
            );
        }
    }

    #[cfg(any(
        feature = "charls",
        feature = "htj2k_openjph",
        feature = "jpeg",
        feature = "jpegxl",
        feature = "jpeg2000",
        feature = "legacy_jpeg_dcmtk"
    ))]
    let mut codec_internal_validation = Vec::new();
    #[cfg(not(any(
        feature = "charls",
        feature = "htj2k_openjph",
        feature = "jpeg",
        feature = "jpegxl",
        feature = "jpeg2000",
        feature = "legacy_jpeg_dcmtk"
    )))]
    let codec_internal_validation = Vec::new();
    #[allow(unused_mut)]
    let mut lossy_image_compression_ratio: Option<String> = None;
    #[allow(unused_mut)]
    let mut compressed_pixel_data = if recipe.transfer_syntax == RLE_LOSSLESS {
        let rle_encoder = NativeRleLosslessEncoder::new();
        let compressed_frames = frame_bytes
            .iter()
            .map(|native_frame| {
                rle_encoder
                    .encode_frame(FrameEncodeInput {
                        native_frame,
                        rows: recipe.rows,
                        columns: recipe.columns,
                        samples_per_pixel: recipe.samples_per_pixel,
                        bits_allocated: recipe.bits_allocated,
                        bits_stored: recipe.bits_stored,
                        photometric_interpretation: recipe.photometric_interpretation,
                    })
                    .map(|encoded| encoded.bytes)
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(|err| GenerateError::WriteDicomFile {
                path: path.clone(),
                message: err.to_string(),
            })?;
        let basic_offset_table_policy =
            if recipe.case_id == "classic/sc/mono2_u8_multiframe_rle_lossless" {
                BasicOffsetTablePolicy::Empty
            } else {
                BasicOffsetTablePolicy::Populated
            };
        let encapsulated = EncapsulatedPixelData::one_fragment_per_frame(
            &compressed_frames,
            basic_offset_table_policy,
        )
        .map_err(|err| GenerateError::WriteDicomFile {
            path: path.clone(),
            message: err.to_string(),
        })?;
        obj.put(DataElement::new(
            tags::PIXEL_DATA,
            recipe.pixel_vr,
            PixelFragmentSequence::new(
                encapsulated.basic_offset_table.offsets.clone(),
                compressed_frames,
            ),
        ));
        Some((FrameEncoder::backend(&rle_encoder), encapsulated, None))
    } else if recipe.transfer_syntax == JPEG_BASELINE_8BIT {
        #[cfg(feature = "jpeg")]
        {
            let jpeg_encoder = DicomRsJpegBaselineEncoder::new();
            let encoded_frame = jpeg_encoder
                .encode_frame(FrameEncodeInput {
                    native_frame: recipe.pixel_bytes,
                    rows: recipe.rows,
                    columns: recipe.columns,
                    samples_per_pixel: recipe.samples_per_pixel,
                    bits_allocated: recipe.bits_allocated,
                    bits_stored: recipe.bits_stored,
                    photometric_interpretation: recipe.photometric_interpretation,
                })
                .map_err(|err| GenerateError::WriteDicomFile {
                    path: path.clone(),
                    message: err.to_string(),
                })?;
            codec_internal_validation.push(validate_jpeg_baseline_lossy_round_trip(
                &path,
                recipe,
                &encoded_frame.bytes,
            )?);
            let compression_ratio = format!(
                "{:.6}",
                recipe.pixel_bytes.len() as f64 / encoded_frame.bytes.len() as f64
            );
            put_str(&mut obj, tags::LOSSY_IMAGE_COMPRESSION, VR::CS, "01");
            put_str(
                &mut obj,
                tags::LOSSY_IMAGE_COMPRESSION_RATIO,
                VR::DS,
                &compression_ratio,
            );
            lossy_image_compression_ratio = Some(compression_ratio);
            put_str(
                &mut obj,
                tags::LOSSY_IMAGE_COMPRESSION_METHOD,
                VR::CS,
                "ISO_10918_1",
            );
            let compressed_frames = vec![encoded_frame.bytes];
            let encapsulated =
                encapsulate_frames(&compressed_frames, &[2], BasicOffsetTablePolicy::Populated)
                    .map_err(|err| GenerateError::WriteDicomFile {
                        path: path.clone(),
                        message: err.to_string(),
                    })?;
            let fragment_payloads = encapsulated.fragment_payloads.clone();
            obj.put(DataElement::new(
                tags::PIXEL_DATA,
                recipe.pixel_vr,
                PixelFragmentSequence::new(
                    encapsulated.basic_offset_table.offsets.clone(),
                    fragment_payloads,
                ),
            ));
            Some((FrameEncoder::backend(&jpeg_encoder), encapsulated, None))
        }
        #[cfg(not(feature = "jpeg"))]
        {
            return Err(GenerateError::WriteDicomFile {
                path: path.clone(),
                message: "JPEG Baseline generation requires the jpeg Cargo feature".to_string(),
            });
        }
    } else if recipe.transfer_syntax == JPEG_LS_LOSSLESS {
        #[cfg(feature = "charls")]
        {
            let jpeg_ls_encoder = DicomRsJpegLsLosslessEncoder::new();
            let encoded_frame = jpeg_ls_encoder
                .encode_frame(FrameEncodeInput {
                    native_frame: recipe.pixel_bytes,
                    rows: recipe.rows,
                    columns: recipe.columns,
                    samples_per_pixel: recipe.samples_per_pixel,
                    bits_allocated: recipe.bits_allocated,
                    bits_stored: recipe.bits_stored,
                    photometric_interpretation: recipe.photometric_interpretation,
                })
                .map_err(|err| GenerateError::WriteDicomFile {
                    path: path.clone(),
                    message: err.to_string(),
                })?;
            codec_internal_validation.push(validate_jpeg_ls_lossless_round_trip(
                &path,
                recipe,
                &encoded_frame.bytes,
            )?);
            let compressed_frames = vec![encoded_frame.bytes];
            let encapsulated = EncapsulatedPixelData::one_fragment_per_frame(
                &compressed_frames,
                BasicOffsetTablePolicy::Populated,
            )
            .map_err(|err| GenerateError::WriteDicomFile {
                path: path.clone(),
                message: err.to_string(),
            })?;
            obj.put(DataElement::new(
                tags::PIXEL_DATA,
                recipe.pixel_vr,
                PixelFragmentSequence::new(
                    encapsulated.basic_offset_table.offsets.clone(),
                    compressed_frames,
                ),
            ));
            Some((FrameEncoder::backend(&jpeg_ls_encoder), encapsulated, None))
        }
        #[cfg(not(feature = "charls"))]
        {
            return Err(GenerateError::WriteDicomFile {
                path: path.clone(),
                message: "JPEG-LS Lossless generation requires the charls Cargo feature"
                    .to_string(),
            });
        }
    } else if recipe.transfer_syntax == JPEG_XL_LOSSLESS {
        #[cfg(feature = "jpegxl")]
        {
            let jpeg_xl_encoder = DicomRsJpegXlLosslessEncoder::new();
            let encoded_frame = jpeg_xl_encoder
                .encode_frame(FrameEncodeInput {
                    native_frame: recipe.pixel_bytes,
                    rows: recipe.rows,
                    columns: recipe.columns,
                    samples_per_pixel: recipe.samples_per_pixel,
                    bits_allocated: recipe.bits_allocated,
                    bits_stored: recipe.bits_stored,
                    photometric_interpretation: recipe.photometric_interpretation,
                })
                .map_err(|err| GenerateError::WriteDicomFile {
                    path: path.clone(),
                    message: err.to_string(),
                })?;
            codec_internal_validation.push(validate_jpeg_xl_lossless_round_trip(
                &path,
                recipe,
                &encoded_frame.bytes,
            )?);
            let compressed_frames = vec![encoded_frame.bytes];
            let encapsulated = EncapsulatedPixelData::one_fragment_per_frame(
                &compressed_frames,
                BasicOffsetTablePolicy::Populated,
            )
            .map_err(|err| GenerateError::WriteDicomFile {
                path: path.clone(),
                message: err.to_string(),
            })?;
            obj.put(DataElement::new(
                tags::PIXEL_DATA,
                recipe.pixel_vr,
                PixelFragmentSequence::new(
                    encapsulated.basic_offset_table.offsets.clone(),
                    compressed_frames,
                ),
            ));
            Some((FrameEncoder::backend(&jpeg_xl_encoder), encapsulated, None))
        }
        #[cfg(not(feature = "jpegxl"))]
        {
            return Err(GenerateError::WriteDicomFile {
                path: path.clone(),
                message: "JPEG XL Lossless generation requires the jpegxl Cargo feature"
                    .to_string(),
            });
        }
    } else if recipe.transfer_syntax == JPEG_2000_LOSSLESS {
        #[cfg(feature = "jpeg2000")]
        {
            let jpeg_2000_encoder = OpenJp2Jpeg2000LosslessEncoder::new();
            let encoded_frame = jpeg_2000_encoder
                .encode_frame(FrameEncodeInput {
                    native_frame: recipe.pixel_bytes,
                    rows: recipe.rows,
                    columns: recipe.columns,
                    samples_per_pixel: recipe.samples_per_pixel,
                    bits_allocated: recipe.bits_allocated,
                    bits_stored: recipe.bits_stored,
                    photometric_interpretation: recipe.photometric_interpretation,
                })
                .map_err(|err| GenerateError::WriteDicomFile {
                    path: path.clone(),
                    message: err.to_string(),
                })?;
            codec_internal_validation.push(validate_jpeg_2000_lossless_round_trip(
                &path,
                recipe,
                &encoded_frame.bytes,
            )?);
            let compressed_frames = vec![encoded_frame.bytes];
            let encapsulated = EncapsulatedPixelData::one_fragment_per_frame(
                &compressed_frames,
                BasicOffsetTablePolicy::Populated,
            )
            .map_err(|err| GenerateError::WriteDicomFile {
                path: path.clone(),
                message: err.to_string(),
            })?;
            obj.put(DataElement::new(
                tags::PIXEL_DATA,
                recipe.pixel_vr,
                PixelFragmentSequence::new(
                    encapsulated.basic_offset_table.offsets.clone(),
                    compressed_frames,
                ),
            ));
            Some((
                FrameEncoder::backend(&jpeg_2000_encoder),
                encapsulated,
                None,
            ))
        }
        #[cfg(not(feature = "jpeg2000"))]
        {
            return Err(GenerateError::WriteDicomFile {
                path: path.clone(),
                message: "JPEG 2000 Lossless generation requires the jpeg2000 Cargo feature"
                    .to_string(),
            });
        }
    } else if recipe.transfer_syntax == HTJ2K_LOSSLESS {
        #[cfg(feature = "htj2k_openjph")]
        {
            let htj2k_encoder = OpenJphHtj2kLosslessEncoder::new();
            let encoded_frame = htj2k_encoder
                .encode_frame(FrameEncodeInput {
                    native_frame: recipe.pixel_bytes,
                    rows: recipe.rows,
                    columns: recipe.columns,
                    samples_per_pixel: recipe.samples_per_pixel,
                    bits_allocated: recipe.bits_allocated,
                    bits_stored: recipe.bits_stored,
                    photometric_interpretation: recipe.photometric_interpretation,
                })
                .map_err(|err| GenerateError::WriteDicomFile {
                    path: path.clone(),
                    message: err.to_string(),
                })?;
            codec_internal_validation.push(validate_htj2k_lossless_round_trip(
                &path,
                recipe,
                &encoded_frame.bytes,
            )?);
            let identity = htj2k_encoder.discover_backend_identity().map_err(|err| {
                GenerateError::WriteDicomFile {
                    path: path.clone(),
                    message: err.to_string(),
                }
            })?;
            let compressed_frames = vec![encoded_frame.bytes];
            let encapsulated = EncapsulatedPixelData::one_fragment_per_frame(
                &compressed_frames,
                BasicOffsetTablePolicy::Populated,
            )
            .map_err(|err| GenerateError::WriteDicomFile {
                path: path.clone(),
                message: err.to_string(),
            })?;
            obj.put(DataElement::new(
                tags::PIXEL_DATA,
                recipe.pixel_vr,
                PixelFragmentSequence::new(
                    encapsulated.basic_offset_table.offsets.clone(),
                    compressed_frames,
                ),
            ));
            let runtime_identity = serde_json::json!({
                "command": identity.command,
                "executable_path": identity.executable_path.to_string_lossy().to_string(),
                "executable_sha256": identity.executable_sha256,
                "version": identity.version,
                "version_source": identity.version_source,
                "encoder_options": {
                    "input_format": "pgm_u16_mono2_big_endian",
                    "reversible": true,
                    "num_decomps": 1
                }
            });
            Some((
                FrameEncoder::backend(&htj2k_encoder),
                encapsulated,
                Some(runtime_identity),
            ))
        }
        #[cfg(not(feature = "htj2k_openjph"))]
        {
            return Err(GenerateError::WriteDicomFile {
                path: path.clone(),
                message: "HTJ2K Lossless generation requires the htj2k_openjph Cargo feature"
                    .to_string(),
            });
        }
    } else {
        obj.put(DataElement::new(
            tags::PIXEL_DATA,
            recipe.pixel_vr,
            PrimitiveValue::from(recipe.pixel_bytes),
        ));
        None
    };

    let is_dcmtk_legacy_jpeg = matches!(
        recipe.transfer_syntax,
        JPEG_LOSSLESS_PROCESS_14 | JPEG_LOSSLESS_SV1
    );
    let file_transfer_syntax = if is_dcmtk_legacy_jpeg {
        EXPLICIT_VR_LITTLE_ENDIAN
    } else {
        recipe.transfer_syntax
    };
    let file_obj = obj
        .with_meta(
            FileMetaTableBuilder::new()
                .transfer_syntax(file_transfer_syntax.uid)
                .implementation_class_uid(&implementation_class_uid)
                .implementation_version_name(crate::IMPLEMENTATION_VERSION_NAME),
        )
        .map_err(|err| GenerateError::WriteDicomFile {
            path: path.clone(),
            message: err.to_string(),
        })?;

    #[cfg(feature = "legacy_jpeg_dcmtk")]
    let mut output_implementation_class_uid = implementation_class_uid.clone();
    #[cfg(not(feature = "legacy_jpeg_dcmtk"))]
    let output_implementation_class_uid = implementation_class_uid.clone();
    #[cfg(feature = "legacy_jpeg_dcmtk")]
    let mut output_implementation_version_name = crate::IMPLEMENTATION_VERSION_NAME.to_string();
    #[cfg(not(feature = "legacy_jpeg_dcmtk"))]
    let output_implementation_version_name = crate::IMPLEMENTATION_VERSION_NAME.to_string();

    if is_dcmtk_legacy_jpeg {
        #[cfg(feature = "legacy_jpeg_dcmtk")]
        {
            let process = dcmtk_lossless_process_for_transfer_syntax(recipe.transfer_syntax)?;
            let source_path = path.with_extension("native-source.dcm");
            file_obj
                .write_to_file(&source_path)
                .map_err(|err| GenerateError::WriteDicomFile {
                    path: source_path.clone(),
                    message: err.to_string(),
                })?;
            let encoder = DcmtkDcmcjpegLosslessSv1Encoder::new();
            let encoded = encoder
                .encode_file_with_process(process, &source_path, &path)
                .map_err(|err| GenerateError::WriteDicomFile {
                    path: path.clone(),
                    message: err.to_string(),
                })?;
            let _ = fs::remove_file(&source_path);

            codec_internal_validation.push(validate_legacy_jpeg_lossless_round_trip(
                &path, recipe, process,
            )?);
            let encapsulated = encapsulated_pixel_data_from_file(&path)?;
            let runtime_identity = serde_json::json!({
                "command": encoded.backend_identity.command,
                "executable_path": encoded.backend_identity.executable_path.to_string_lossy().to_string(),
                "executable_sha256": encoded.backend_identity.executable_sha256,
                "version": encoded.backend_identity.version,
                "version_source": encoded.backend_identity.version_source,
                "encoder_options": {
                    "mode": process.mode_label(),
                    "true_lossless": true,
                    "fragment_per_frame": true,
                    "offset_table": "create",
                    "uid_policy": "never"
                }
            });
            let compressed = open_file(&path).map_err(|err| GenerateError::ValidateDicomFile {
                path: path.clone(),
                message: err.to_string(),
            })?;
            output_implementation_class_uid = compressed
                .meta()
                .implementation_class_uid()
                .trim_end_matches('\0')
                .to_string();
            output_implementation_version_name = compressed
                .meta()
                .implementation_version_name
                .as_deref()
                .unwrap_or(crate::IMPLEMENTATION_VERSION_NAME)
                .trim_end()
                .to_string();
            compressed_pixel_data = Some((
                encoder.backend_for(process),
                encapsulated,
                Some(runtime_identity),
            ));
        }
        #[cfg(not(feature = "legacy_jpeg_dcmtk"))]
        {
            return Err(GenerateError::WriteDicomFile {
                path: path.clone(),
                message: format!(
                    "{} generation requires the legacy_jpeg_dcmtk Cargo feature",
                    recipe.transfer_syntax.name
                ),
            });
        }
    } else {
        file_obj
            .write_to_file(&path)
            .map_err(|err| GenerateError::WriteDicomFile {
                path: path.clone(),
                message: err.to_string(),
            })?;
    }

    let decoded_frame_hashes = frame_bytes
        .iter()
        .map(|frame| sha256_hex(frame))
        .collect::<Vec<_>>();
    let decoded_frame_hash_refs = decoded_frame_hashes
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    let mut validated = validate_part10_file(
        &path,
        &Part10Expectations {
            sop_class_uid,
            sop_instance_uid: &sop_instance_uid,
            transfer_syntax_uid: recipe.transfer_syntax.uid,
            implementation_class_uid: &output_implementation_class_uid,
            synthetic_data: "YES",
            rows: recipe.rows,
            columns: recipe.columns,
            frames: frame_count,
            samples_per_pixel: recipe.samples_per_pixel,
            photometric_interpretation: recipe.photometric_interpretation,
            bits_allocated: recipe.bits_allocated,
            bits_stored: recipe.bits_stored,
            high_bit: recipe.high_bit,
            pixel_representation: recipe.pixel_representation,
            planar_configuration: recipe.planar_configuration,
            pixel_data_vr: recipe.pixel_vr,
            pixel_data_length_formula: compressed_pixel_data
                .as_ref()
                .map(
                    |(_, encapsulated, _)| PixelDataLengthFormula::Encapsulated {
                        fragments: encapsulated.fragments.len(),
                        basic_offset_table_offsets: encapsulated.basic_offset_table.offsets.len(),
                    },
                )
                .unwrap_or_else(|| pixel_data_length_formula(recipe)),
            decoded_frame_hashes: if compressed_pixel_data.is_some() {
                &decoded_frame_hash_refs
            } else {
                &[]
            },
            palette: recipe.palette.map(|palette| palette.into()),
            padding: recipe.padding.map(|padding| padding.into()),
            ct_image: None,
            enhanced_ct_image: None,
            enhanced_mr_image: None,
            mg_image: None,
            dx_image: None,
            us_image: None,
            cr_image: None,
            mr_image: None,
            segmentation: None,
        },
    )?;
    for result in codec_internal_validation {
        append_internal_validation(&mut validated.validation, result);
    }

    Ok(GeneratedFile {
        case_id: recipe.case_id.to_string(),
        manifest_entry: pixel_manifest_entry(
            case,
            recipe,
            &relative_path,
            &study_instance_uid,
            &series_instance_uid,
            &sop_instance_uid,
            &output_implementation_class_uid,
            &output_implementation_version_name,
            &validated.bytes,
            validated.validation,
            compressed_pixel_data.as_ref(),
            lossy_image_compression_ratio.as_deref(),
            &decoded_frame_hash_refs,
        ),
    })
}

fn append_internal_validation(validation: &mut Value, result: Value) {
    if let Some(internal) = validation
        .get_mut("internal")
        .and_then(serde_json::Value::as_array_mut)
    {
        internal.push(result);
    }
}

#[cfg(feature = "jpeg")]
fn validate_jpeg_baseline_lossy_round_trip(
    path: &std::path::Path,
    recipe: PixelRecipe,
    encoded_frame: &[u8],
) -> Result<Value, GenerateError> {
    const MAX_ABS_DIFF: u8 = 10;

    let decoder = DicomRsJpegBaselineEncoder::new();
    let decoded = decoder
        .decode_frame(FrameDecodeInput {
            encoded_frame,
            rows: recipe.rows,
            columns: recipe.columns,
            samples_per_pixel: recipe.samples_per_pixel,
            bits_allocated: recipe.bits_allocated,
            bits_stored: recipe.bits_stored,
            photometric_interpretation: recipe.photometric_interpretation,
        })
        .map_err(|err| GenerateError::ValidateDicomFile {
            path: path.to_path_buf(),
            message: err.to_string(),
        })?;

    if decoded.native_bytes.len() != recipe.pixel_bytes.len() {
        return Err(GenerateError::ValidateDicomFile {
            path: path.to_path_buf(),
            message: format!(
                "JPEG Baseline decoded frame length is {}, expected {}",
                decoded.native_bytes.len(),
                recipe.pixel_bytes.len()
            ),
        });
    }

    let max_diff = recipe
        .pixel_bytes
        .iter()
        .copied()
        .zip(decoded.native_bytes.iter().copied())
        .map(|(expected, actual)| expected.abs_diff(actual))
        .max()
        .unwrap_or(0);
    if max_diff > MAX_ABS_DIFF {
        return Err(GenerateError::ValidateDicomFile {
            path: path.to_path_buf(),
            message: format!(
                "JPEG Baseline decoded frame maximum sample difference {max_diff} exceeded {MAX_ABS_DIFF}"
            ),
        });
    }

    Ok(serde_json::json!({
        "name": "jpeg_baseline_decoded_frame_tolerance",
        "status": "passed",
        "message": format!("JPEG Baseline decoded samples are within +/-{MAX_ABS_DIFF} of the native source frame.")
    }))
}

#[cfg(feature = "charls")]
fn validate_jpeg_ls_lossless_round_trip(
    path: &std::path::Path,
    recipe: PixelRecipe,
    encoded_frame: &[u8],
) -> Result<Value, GenerateError> {
    let decoder = DicomRsJpegLsLosslessEncoder::new();
    let decoded = decoder
        .decode_frame(FrameDecodeInput {
            encoded_frame,
            rows: recipe.rows,
            columns: recipe.columns,
            samples_per_pixel: recipe.samples_per_pixel,
            bits_allocated: recipe.bits_allocated,
            bits_stored: recipe.bits_stored,
            photometric_interpretation: recipe.photometric_interpretation,
        })
        .map_err(|err| GenerateError::ValidateDicomFile {
            path: path.to_path_buf(),
            message: err.to_string(),
        })?;

    let decoded_hash = sha256_hex(&decoded.native_bytes);
    let expected_hash = sha256_hex(recipe.pixel_bytes);
    if decoded_hash != expected_hash {
        return Err(GenerateError::ValidateDicomFile {
            path: path.to_path_buf(),
            message: format!(
                "JPEG-LS Lossless decoded frame hash {decoded_hash} did not match expected {expected_hash}"
            ),
        });
    }

    Ok(serde_json::json!({
        "name": "jpeg_ls_lossless_decoded_frame_hashes",
        "status": "passed",
        "message": "JPEG-LS Lossless decoded frame hash matches the native source frame."
    }))
}

#[cfg(feature = "jpegxl")]
fn validate_jpeg_xl_lossless_round_trip(
    path: &std::path::Path,
    recipe: PixelRecipe,
    encoded_frame: &[u8],
) -> Result<Value, GenerateError> {
    let decoder = DicomRsJpegXlLosslessEncoder::new();
    let decoded = decoder
        .decode_frame(FrameDecodeInput {
            encoded_frame,
            rows: recipe.rows,
            columns: recipe.columns,
            samples_per_pixel: recipe.samples_per_pixel,
            bits_allocated: recipe.bits_allocated,
            bits_stored: recipe.bits_stored,
            photometric_interpretation: recipe.photometric_interpretation,
        })
        .map_err(|err| GenerateError::ValidateDicomFile {
            path: path.to_path_buf(),
            message: err.to_string(),
        })?;

    let decoded_hash = sha256_hex(&decoded.native_bytes);
    let expected_hash = sha256_hex(recipe.pixel_bytes);
    if decoded_hash != expected_hash {
        return Err(GenerateError::ValidateDicomFile {
            path: path.to_path_buf(),
            message: format!(
                "JPEG XL Lossless decoded frame hash {decoded_hash} did not match expected {expected_hash}"
            ),
        });
    }

    Ok(serde_json::json!({
        "name": "jpeg_xl_lossless_decoded_frame_hashes",
        "status": "passed",
        "message": "JPEG XL Lossless decoded frame hash matches the native source frame."
    }))
}

#[cfg(feature = "jpeg2000")]
fn validate_jpeg_2000_lossless_round_trip(
    path: &std::path::Path,
    recipe: PixelRecipe,
    encoded_frame: &[u8],
) -> Result<Value, GenerateError> {
    let decoder = OpenJp2Jpeg2000LosslessEncoder::new();
    let decoded = decoder
        .decode_frame(FrameDecodeInput {
            encoded_frame,
            rows: recipe.rows,
            columns: recipe.columns,
            samples_per_pixel: recipe.samples_per_pixel,
            bits_allocated: recipe.bits_allocated,
            bits_stored: recipe.bits_stored,
            photometric_interpretation: recipe.photometric_interpretation,
        })
        .map_err(|err| GenerateError::ValidateDicomFile {
            path: path.to_path_buf(),
            message: err.to_string(),
        })?;

    let decoded_hash = sha256_hex(&decoded.native_bytes);
    let expected_hash = sha256_hex(recipe.pixel_bytes);
    if decoded_hash != expected_hash {
        return Err(GenerateError::ValidateDicomFile {
            path: path.to_path_buf(),
            message: format!(
                "JPEG 2000 Lossless decoded frame hash {decoded_hash} did not match expected {expected_hash}"
            ),
        });
    }

    Ok(serde_json::json!({
        "name": "jpeg_2000_lossless_decoded_frame_hashes",
        "status": "passed",
        "message": "JPEG 2000 Lossless decoded frame hash matches the native source frame."
    }))
}

#[cfg(feature = "htj2k_openjph")]
fn validate_htj2k_lossless_round_trip(
    path: &std::path::Path,
    recipe: PixelRecipe,
    encoded_frame: &[u8],
) -> Result<Value, GenerateError> {
    let decoder = OpenJphHtj2kLosslessEncoder::new();
    let decoded = decoder
        .decode_frame(FrameDecodeInput {
            encoded_frame,
            rows: recipe.rows,
            columns: recipe.columns,
            samples_per_pixel: recipe.samples_per_pixel,
            bits_allocated: recipe.bits_allocated,
            bits_stored: recipe.bits_stored,
            photometric_interpretation: recipe.photometric_interpretation,
        })
        .map_err(|err| GenerateError::ValidateDicomFile {
            path: path.to_path_buf(),
            message: err.to_string(),
        })?;

    let decoded_hash = sha256_hex(&decoded.native_bytes);
    let expected_hash = sha256_hex(recipe.pixel_bytes);
    if decoded_hash != expected_hash {
        return Err(GenerateError::ValidateDicomFile {
            path: path.to_path_buf(),
            message: format!(
                "HTJ2K Lossless decoded frame hash {decoded_hash} did not match expected {expected_hash}"
            ),
        });
    }

    Ok(serde_json::json!({
        "name": "htj2k_lossless_decoded_frame_hashes",
        "status": "passed",
        "message": "HTJ2K Lossless decoded frame hash matches the native source frame."
    }))
}

#[cfg(feature = "legacy_jpeg_dcmtk")]
fn dcmtk_lossless_process_for_transfer_syntax(
    transfer_syntax: TransferSyntaxSpec,
) -> Result<DcmtkDcmcjpegLosslessProcess, GenerateError> {
    if transfer_syntax == JPEG_LOSSLESS_PROCESS_14 {
        Ok(DcmtkDcmcjpegLosslessProcess::Process14)
    } else if transfer_syntax == JPEG_LOSSLESS_SV1 {
        Ok(DcmtkDcmcjpegLosslessProcess::Sv1)
    } else {
        Err(GenerateError::ValidateDicomFile {
            path: PathBuf::new(),
            message: format!(
                "{} is not supported by the DCMTK legacy JPEG wrapper",
                transfer_syntax.name
            ),
        })
    }
}

#[cfg(feature = "legacy_jpeg_dcmtk")]
fn validate_legacy_jpeg_lossless_round_trip(
    path: &std::path::Path,
    recipe: PixelRecipe,
    process: DcmtkDcmcjpegLosslessProcess,
) -> Result<Value, GenerateError> {
    let file = open_file(path).map_err(|err| GenerateError::ValidateDicomFile {
        path: path.to_path_buf(),
        message: err.to_string(),
    })?;
    let codec = match process {
        DcmtkDcmcjpegLosslessProcess::Process14 => JPEG_LOSSLESS_NON_HIERARCHICAL.codec(),
        DcmtkDcmcjpegLosslessProcess::Sv1 => {
            JPEG_LOSSLESS_NON_HIERARCHICAL_FIRST_ORDER_PREDICTION.codec()
        }
    };
    let Codec::EncapsulatedPixelData(Some(reader), _) = codec else {
        return Err(GenerateError::ValidateDicomFile {
            path: path.to_path_buf(),
            message: format!(
                "DICOM-rs {} decoder is unavailable",
                legacy_jpeg_lossless_label(process)
            ),
        });
    };
    let mut decoded = Vec::new();
    reader.decode_frame(&file, 0, &mut decoded).map_err(|err| {
        GenerateError::ValidateDicomFile {
            path: path.to_path_buf(),
            message: err.to_string(),
        }
    })?;

    let decoded_hash = sha256_hex(&decoded);
    let expected_hash = sha256_hex(recipe.pixel_bytes);
    if decoded_hash != expected_hash {
        return Err(GenerateError::ValidateDicomFile {
            path: path.to_path_buf(),
            message: format!(
                "{} decoded frame hash {decoded_hash} did not match expected {expected_hash}",
                legacy_jpeg_lossless_label(process)
            ),
        });
    }

    Ok(serde_json::json!({
        "name": legacy_jpeg_lossless_validation_name(process),
        "status": "passed",
        "message": format!(
            "{} decoded frame hash matches the native source frame.",
            legacy_jpeg_lossless_label(process)
        )
    }))
}

#[cfg(feature = "legacy_jpeg_dcmtk")]
fn legacy_jpeg_lossless_validation_name(process: DcmtkDcmcjpegLosslessProcess) -> &'static str {
    match process {
        DcmtkDcmcjpegLosslessProcess::Process14 => "jpeg_lossless_process_14_decoded_frame_hashes",
        DcmtkDcmcjpegLosslessProcess::Sv1 => "jpeg_lossless_sv1_decoded_frame_hashes",
    }
}

#[cfg(feature = "legacy_jpeg_dcmtk")]
fn legacy_jpeg_lossless_label(process: DcmtkDcmcjpegLosslessProcess) -> &'static str {
    match process {
        DcmtkDcmcjpegLosslessProcess::Process14 => "JPEG Lossless Process 14",
        DcmtkDcmcjpegLosslessProcess::Sv1 => "JPEG Lossless SV1",
    }
}

#[cfg(feature = "deflate")]
fn validate_deflated_image_frame_round_trip(
    path: &std::path::Path,
    recipe: SegmentationRecipe,
    native_frame: &[u8],
    encoded_frame: &[u8],
) -> Result<Value, GenerateError> {
    let decoder = DicomRsDeflatedImageFrameEncoder::new();
    let decoded = decoder
        .decode_frame(FrameDecodeInput {
            encoded_frame,
            rows: recipe.rows,
            columns: recipe.columns,
            samples_per_pixel: 1,
            bits_allocated: recipe.bits_allocated,
            bits_stored: recipe.bits_stored,
            photometric_interpretation: "MONOCHROME2",
        })
        .map_err(|err| GenerateError::ValidateDicomFile {
            path: path.to_path_buf(),
            message: err.to_string(),
        })?;

    let decoded_hash = sha256_hex(&decoded.native_bytes);
    let expected_hash = sha256_hex(native_frame);
    if decoded_hash != expected_hash {
        return Err(GenerateError::ValidateDicomFile {
            path: path.to_path_buf(),
            message: format!(
                "Deflated Image Frame decoded frame hash {decoded_hash} did not match expected {expected_hash}"
            ),
        });
    }

    Ok(serde_json::json!({
        "name": "deflated_image_frame_decoded_frame_hashes",
        "status": "passed",
        "message": "Deflated Image Frame decoded frame hash matches the native bit-packed source frame."
    }))
}

#[cfg(feature = "legacy_jpeg_dcmtk")]
fn encapsulated_pixel_data_from_file(
    path: &std::path::Path,
) -> Result<EncapsulatedPixelData, GenerateError> {
    let file = open_file(path).map_err(|err| GenerateError::ValidateDicomFile {
        path: path.to_path_buf(),
        message: err.to_string(),
    })?;
    let pixel_data =
        file.element(tags::PIXEL_DATA)
            .map_err(|err| GenerateError::ValidateDicomFile {
                path: path.to_path_buf(),
                message: err.to_string(),
            })?;
    let DicomValue::PixelSequence(sequence) = pixel_data.value() else {
        return Err(GenerateError::ValidateDicomFile {
            path: path.to_path_buf(),
            message: "JPEG Lossless SV1 output does not use encapsulated Pixel Data".to_string(),
        });
    };
    let policy = if sequence.offset_table().is_empty() {
        BasicOffsetTablePolicy::Empty
    } else {
        BasicOffsetTablePolicy::Populated
    };
    EncapsulatedPixelData::one_fragment_per_frame(sequence.fragments(), policy).map_err(|err| {
        GenerateError::ValidateDicomFile {
            path: path.to_path_buf(),
            message: err.to_string(),
        }
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
    implementation_version_name: &str,
    bytes: &[u8],
    validation: Value,
    compressed_pixel_data: Option<&(
        crate::codecs::CodecBackendInfo,
        EncapsulatedPixelData,
        Option<Value>,
    )>,
    lossy_image_compression_ratio: Option<&str>,
    frame_hashes: &[&str],
) -> Value {
    let mut standards_evidence = standards_evidence_from_case(case);
    if pixel_is_vl_photographic(recipe) {
        standards_evidence.extend([
            serde_json::json!({
                "source": "dicom-standard-kb",
                "edition": "2026b",
                "query": "lookup_sop_class VL Photographic Image Storage",
                "covered": true,
                "part": "PS3.4",
                "anchor": "table_B.5-1"
            }),
            serde_json::json!({
                "source": "dicom-standard-kb",
                "edition": "2026b",
                "query": "lookup_iod VL Photographic Image",
                "covered": true,
                "part": "PS3.3",
                "anchor": "table_A.32.4-1"
            }),
            serde_json::json!({
                "source": "dicom-standard-kb",
                "edition": "2026b",
                "query": "list_modules_for_iod VL Photographic Image",
                "covered": true,
                "part": "PS3.3",
                "anchor": "table_A.32.4-1"
            }),
            serde_json::json!({
                "source": "dicom-standard-kb",
                "edition": "2026b",
                "query": "list_attributes_for_module VL Image --expand-macros",
                "covered": true,
                "part": "PS3.3",
                "anchor": "table_C.8-77"
            }),
            serde_json::json!({
                "source": "dicom-standard-kb",
                "edition": "2026b",
                "query": "list_attributes_for_module Acquisition Context --expand-macros",
                "covered": true,
                "part": "PS3.3",
                "anchor": "table_C.7.6.14-1"
            }),
        ]);
    } else {
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
    }
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
    if recipe.transfer_syntax == RLE_LOSSLESS {
        standards_evidence.extend([
            serde_json::json!({
                "source": "dicom-standard-kb",
                "edition": "2026b",
                "query": "lookup_uid RLELossless",
                "covered": true,
                "part": "PS3.6",
                "anchor": "table_A-1"
            }),
            serde_json::json!({
                "source": "dicom-standard-kb",
                "edition": "2026b",
                "query": "search_standard_text RLE Lossless Transfer Syntax encapsulated Pixel Data",
                "covered": true,
                "part": "PS3.5",
                "anchor": "sect_8.2.2"
            }),
            serde_json::json!({
                "source": "dicom-standard-kb",
                "edition": "2026b",
                "query": "search_standard_text Basic Offset Table encapsulated Pixel Data Item padding Extended Offset Table",
                "covered": true,
                "part": "PS3.5",
                "anchor": "sect_A.4"
            }),
        ]);
    }
    if recipe.transfer_syntax == JPEG_BASELINE_8BIT {
        standards_evidence.extend([
            serde_json::json!({
                "source": "dicom-standard-kb",
                "edition": "2026b",
                "query": "lookup_uid JPEGBaseline8Bit",
                "covered": true,
                "part": "PS3.6",
                "anchor": "table_A-1"
            }),
            serde_json::json!({
                "source": "dicom-standard-kb",
                "edition": "2026b",
                "query": "lookup_data_element LossyImageCompression",
                "covered": true,
                "part": "PS3.6",
                "anchor": "table_6-1"
            }),
            serde_json::json!({
                "source": "dicom-standard-kb",
                "edition": "2026b",
                "query": "lookup_data_element LossyImageCompressionRatio",
                "covered": true,
                "part": "PS3.6",
                "anchor": "table_6-1"
            }),
            serde_json::json!({
                "source": "dicom-standard-kb",
                "edition": "2026b",
                "query": "search_standard_text Basic Offset Table encapsulated Pixel Data Item padding Extended Offset Table",
                "covered": true,
                "part": "PS3.5",
                "anchor": "sect_A.4"
            }),
        ]);
    }
    if recipe.transfer_syntax == JPEG_LS_LOSSLESS {
        standards_evidence.extend([
            serde_json::json!({
                "source": "dicom-standard-kb",
                "edition": "2026b",
                "query": "lookup_uid JPEGLSLossless",
                "covered": true,
                "part": "PS3.6",
                "anchor": "table_A-1"
            }),
            serde_json::json!({
                "source": "dicom-standard-kb",
                "edition": "2026b",
                "query": "search_standard_text Basic Offset Table encapsulated Pixel Data Item padding Extended Offset Table",
                "covered": true,
                "part": "PS3.5",
                "anchor": "sect_A.4"
            }),
        ]);
    }
    if recipe.transfer_syntax == JPEG_2000_LOSSLESS {
        standards_evidence.extend([
            serde_json::json!({
                "source": "dicom-standard-kb",
                "edition": "2026b",
                "query": "lookup_uid JPEG2000Lossless",
                "covered": true,
                "part": "PS3.6",
                "anchor": "table_A-1"
            }),
            serde_json::json!({
                "source": "dicom-standard-kb",
                "edition": "2026b",
                "query": "search_standard_text Basic Offset Table encapsulated Pixel Data Item padding Extended Offset Table",
                "covered": true,
                "part": "PS3.5",
                "anchor": "sect_A.4"
            }),
        ]);
    }
    if recipe.transfer_syntax == HTJ2K_LOSSLESS {
        standards_evidence.extend([
            serde_json::json!({
                "source": "dicom-standard-kb",
                "edition": "2026b",
                "query": "lookup_uid HTJ2KLossless",
                "covered": true,
                "part": "PS3.6",
                "anchor": "table_A-1"
            }),
            serde_json::json!({
                "source": "dicom-standard-kb",
                "edition": "2026b",
                "query": "search_standard_text Basic Offset Table encapsulated Pixel Data Item padding Extended Offset Table",
                "covered": true,
                "part": "PS3.5",
                "anchor": "sect_A.4"
            }),
        ]);
    }
    if recipe.transfer_syntax == JPEG_LOSSLESS_PROCESS_14
        || recipe.transfer_syntax == JPEG_LOSSLESS_SV1
    {
        let transfer_syntax_query = if recipe.transfer_syntax == JPEG_LOSSLESS_PROCESS_14 {
            "lookup_uid JPEGLossless"
        } else {
            "lookup_uid JPEGLosslessSV1"
        };
        standards_evidence.extend([
            serde_json::json!({
                "source": "dicom-standard-kb",
                "edition": "2026b",
                "query": transfer_syntax_query,
                "covered": true,
                "part": "PS3.6",
                "anchor": "table_A-1"
            }),
            serde_json::json!({
                "source": "dicom-standard-kb",
                "edition": "2026b",
                "query": "search_standard_text Basic Offset Table encapsulated Pixel Data Item padding Extended Offset Table",
                "covered": true,
                "part": "PS3.5",
                "anchor": "sect_A.4"
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
    let codec_manifest = compressed_pixel_data.map(|(backend, _, runtime_identity)| {
        let mut codec = serde_json::json!({
            "backend_id": backend.backend_id,
            "backend_kind": backend.backend_kind.as_str(),
            "display_name": backend.display_name,
            "version": backend.version,
            "transfer_syntax_uid": backend.transfer_syntax_uid,
            "feature_gate": backend.feature_gate,
            "determinism": backend.determinism.as_str()
        });
        if let Some(runtime_identity) = runtime_identity {
            codec["runtime_identity"] = runtime_identity.clone();
        }
        codec
    });
    let pixel_data_manifest = if let Some((_, encapsulated, _)) = compressed_pixel_data {
        serde_json::json!({
            "vr": pixel_vr_name(recipe.pixel_vr),
            "native_or_encapsulated": "encapsulated",
            "value_length": Value::Null,
            "frame_count": frame_hashes.len(),
            "frame_hashes": frame_hashes,
            "codec": codec_manifest,
            "encapsulated_pixel_data": {
                "basic_offset_table": {
                    "present": true,
                    "populated": encapsulated.basic_offset_table.is_populated(),
                    "offset_count": encapsulated.basic_offset_table.offsets.len(),
                    "offsets": encapsulated.basic_offset_table.offsets.clone()
                },
                "fragments_per_frame": encapsulated.fragments_per_frame.clone(),
                "fragments": encapsulated.fragments.iter().map(|fragment| {
                    serde_json::json!({
                        "frame_index": fragment.frame_index,
                        "item_start_offset": fragment.item_start_offset,
                        "compressed_length": fragment.compressed_length,
                        "padded_length": fragment.padded_length
                    })
                }).collect::<Vec<_>>(),
                "extended_offset_table": {
                    "present": false,
                    "lengths_present": false,
                    "offset_count": 0,
                    "length_count": 0
                },
                "compressed_frame_hashes": encapsulated.compressed_frame_hashes.clone()
            }
        })
    } else {
        serde_json::json!({
            "vr": pixel_vr_name(recipe.pixel_vr),
            "native_or_encapsulated": "native",
            "value_length": recipe.pixel_bytes.len(),
            "frame_count": frame_hashes.len(),
            "frame_hashes": frame_hashes
        })
    };

    serde_json::json!({
        "case_id": recipe.case_id,
        "profile_membership": pixel_profile_membership(recipe),
        "path": relative_path,
        "sha256": sha256_hex(bytes),
        "size_bytes": bytes.len(),
        "determinism": pixel_determinism(recipe),
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
            "sop_class_uid": pixel_sop_class_uid(recipe),
            "sop_class_name": pixel_sop_class_name(recipe),
            "iod_name": pixel_iod_name(recipe),
            "modality": pixel_modality(recipe),
            "transfer_syntax_uid": recipe.transfer_syntax.uid,
            "transfer_syntax_name": recipe.transfer_syntax.name
        },
        "uids": {
            "study_instance_uid": study_instance_uid,
            "series_instance_uid": series_instance_uid,
            "sop_instance_uid": sop_instance_uid,
            "frame_of_reference_uid": Value::Null,
            "implementation_class_uid": implementation_class_uid,
            "implementation_version_name": implementation_version_name
        },
        "image": {
            "rows": recipe.rows,
            "columns": recipe.columns,
            "frames": frame_hashes.len(),
            "samples_per_pixel": recipe.samples_per_pixel,
            "photometric_interpretation": recipe.photometric_interpretation,
            "bits_allocated": recipe.bits_allocated,
            "bits_stored": recipe.bits_stored,
            "high_bit": recipe.high_bit,
            "pixel_representation": recipe.pixel_representation,
            "planar_configuration": recipe.planar_configuration
        },
        "pixel_data": pixel_data_manifest,
        "expected_capabilities": pixel_expected_capabilities(recipe),
        "expected_semantics": {
            "synthetic_data": "YES",
            "conversion_type": pixel_conversion_type(recipe),
            "image_type": pixel_image_type(recipe),
            "pixel_min": recipe.pixel_min,
            "pixel_max": recipe.pixel_max,
            "pixel_padding": padding_manifest,
            "lossy_image_compression": if recipe.transfer_syntax == JPEG_BASELINE_8BIT { "01" } else { "00" },
            "lossy_image_compression_ratio": lossy_image_compression_ratio,
            "lossy_image_compression_method": pixel_lossy_image_compression_method(recipe),
            "photometric_semantics": recipe.semantic_note
        },
        "expected_visual_checks": {
            "pattern": recipe.visual_pattern
        },
        "validation": validation,
        "known_stressors": pixel_known_stressors(recipe),
        "standards_evidence": deduplicated_standards_evidence(standards_evidence)
    })
}

fn pixel_lossy_image_compression_method(recipe: PixelRecipe) -> Option<&'static str> {
    if recipe.transfer_syntax == JPEG_BASELINE_8BIT {
        Some("ISO_10918_1")
    } else {
        None
    }
}

fn pixel_known_stressors(recipe: PixelRecipe) -> Vec<&'static str> {
    let mut stressors = if pixel_is_vl_photographic(recipe) {
        let mut stressors = vec!["vl_photographic_image_storage"];
        if recipe.palette.is_some() {
            stressors.push("vl_palette_color_pixels");
        } else if recipe.samples_per_pixel > 1 {
            stressors.push("vl_rgb_pixels");
        }
        stressors
    } else {
        vec!["minimal_secondary_capture"]
    };
    if recipe.transfer_syntax == RLE_LOSSLESS {
        stressors.push("encapsulated_pixel_data");
        stressors.push("rle_lossless_transfer_syntax");
    } else if recipe.transfer_syntax == JPEG_BASELINE_8BIT {
        stressors.push("encapsulated_pixel_data");
        stressors.push("jpeg_baseline_8bit_transfer_syntax");
        stressors.push("lossy_image_compression");
        stressors.push("multi_fragment_frame");
    } else if recipe.transfer_syntax == JPEG_LS_LOSSLESS {
        stressors.push("encapsulated_pixel_data");
        stressors.push("jpeg_ls_lossless_transfer_syntax");
    } else if recipe.transfer_syntax == JPEG_XL_LOSSLESS {
        stressors.push("encapsulated_pixel_data");
        stressors.push("jpeg_xl_lossless_transfer_syntax");
    } else if recipe.transfer_syntax == JPEG_2000_LOSSLESS {
        stressors.push("encapsulated_pixel_data");
        stressors.push("jpeg_2000_lossless_transfer_syntax");
    } else if recipe.transfer_syntax == HTJ2K_LOSSLESS {
        stressors.push("encapsulated_pixel_data");
        stressors.push("htj2k_lossless_transfer_syntax");
    } else if recipe.transfer_syntax == JPEG_LOSSLESS_PROCESS_14 {
        stressors.push("encapsulated_pixel_data");
        stressors.push("jpeg_lossless_process_14_transfer_syntax");
        stressors.push("external_command_codec");
    } else if recipe.transfer_syntax == JPEG_LOSSLESS_SV1 {
        stressors.push("encapsulated_pixel_data");
        stressors.push("jpeg_lossless_sv1_transfer_syntax");
        stressors.push("external_command_codec");
    } else {
        stressors.push("native_ob_pixel_data");
    }
    if recipe.transfer_syntax == EXPLICIT_VR_BIG_ENDIAN {
        stressors.push("retired_transfer_syntax");
        stressors.push("explicit_vr_big_endian_dataset");
    }
    if recipe.transfer_syntax == DEFLATED_EXPLICIT_VR_LITTLE_ENDIAN {
        stressors.push("deflated_dataset_transfer_syntax");
    }
    if matches!(
        recipe.case_id,
        "classic/sc/mono2_u8_odd_fragment_rle_lossless"
            | "classic/sc/mono1_u8_odd_fragment_rle_lossless"
    ) {
        stressors.push("odd_compressed_fragment_length");
        stressors.push("encapsulated_item_padding");
    }
    if recipe.case_id == "classic/sc/mono2_u8_multiframe_rle_lossless" {
        stressors.push("empty_basic_offset_table");
    }
    if recipe.palette.is_some() {
        stressors.push("palette_color_pixels");
    }
    if recipe.photometric_interpretation == "YBR_FULL" {
        stressors.push("ybr_full_pixels");
    }
    stressors
}

fn pixel_profile_membership(recipe: PixelRecipe) -> &'static [&'static str] {
    match recipe.case_id {
        "classic/sc/mono2_u8_explicit_le"
        | "classic/sc/mono1_u8_explicit_le"
        | "classic/sc/rgb_planar0_explicit_le" => &["smoke"],
        "classic/sc/mono2_u8_explicit_be" => &["legacy"],
        "classic/sc/mono2_u8_deflated_explicit_le"
        | "classic/sc/mono2_u8_rle_lossless"
        | "classic/sc/mono2_u8_padding_rle_lossless"
        | "classic/sc/mono1_u8_padding_rle_lossless"
        | "classic/sc/mono1_u8_padding_multiframe_rle_lossless"
        | "classic/sc/mono1_u8_rle_lossless"
        | "classic/sc/mono2_u16_rle_lossless"
        | "classic/sc/mono2_u16_tiny_1x1_rle_lossless"
        | "classic/sc/mono1_u16_tiny_1x1_rle_lossless"
        | "classic/sc/mono2_i16_tiny_1x1_rle_lossless"
        | "classic/sc/mono1_i16_tiny_1x1_rle_lossless"
        | "classic/sc/mono2_u16_padding_rle_lossless"
        | "classic/sc/mono1_u16_padding_rle_lossless"
        | "classic/sc/mono2_u16_padding_multiframe_rle_lossless"
        | "classic/sc/mono1_u16_padding_multiframe_rle_lossless"
        | "classic/sc/mono2_i16_padding_rle_lossless"
        | "classic/sc/mono1_i16_padding_rle_lossless"
        | "classic/sc/mono1_i16_padding_multiframe_rle_lossless"
        | "classic/sc/mono2_i16_padding_multiframe_rle_lossless"
        | "classic/sc/mono2_i16_rle_lossless"
        | "classic/sc/mono1_i16_rle_lossless"
        | "classic/sc/rgb_planar0_rle_lossless"
        | "classic/sc/rgb_planar1_rle_lossless"
        | "classic/sc/mono1_u16_rect_2x3_rle_lossless"
        | "classic/sc/mono2_i16_rect_2x3_rle_lossless"
        | "classic/sc/mono1_i16_rect_2x3_rle_lossless"
        | "classic/sc/rgb_planar1_multiframe_rle_lossless"
        | "classic/sc/ybr_full_planar0_rle_lossless"
        | "classic/sc/ybr_full_planar0_multiframe_rle_lossless"
        | "classic/sc/ybr_full_planar1_rle_lossless"
        | "classic/sc/ybr_full_planar1_multiframe_rle_lossless"
        | "classic/sc/palette_color_u8_rle_lossless"
        | "classic/sc/palette_color_u8_multiframe_rle_lossless"
        | "classic/sc/mono1_u8_multiframe_rle_lossless"
        | "classic/sc/mono2_u16_multiframe_rle_lossless"
        | "classic/sc/mono1_u16_multiframe_rle_lossless"
        | "classic/sc/mono2_i16_multiframe_rle_lossless"
        | "classic/sc/mono1_i16_multiframe_rle_lossless"
        | "classic/sc/mono2_u8_odd_fragment_rle_lossless"
        | "classic/sc/mono1_u8_odd_fragment_rle_lossless"
        | "vl/photo/rgb_planar0_rle_lossless"
        | "vl/photo/rgb_planar1_rle_lossless"
        | "vl/photo/palette_color_rle_lossless"
        | "classic/sc/rgb_planar0_multiframe_rle_lossless"
        | "classic/sc/rgb_planar0_jpeg_baseline_8bit"
        | "classic/sc/mono2_u8_jpeg_ls_lossless"
        | "classic/sc/rgb_planar0_jpegxl_lossless"
        | "classic/sc/mono2_u16_jpeg2000_lossless"
        | "classic/sc/mono2_u16_htj2k_lossless"
        | "classic/sc/mono2_u16_jpeg_lossless_process_14"
        | "classic/sc/mono2_u16_jpeg_lossless_sv1" => &["extended"],
        _ => &["core"],
    }
}

fn pixel_expected_capabilities(recipe: PixelRecipe) -> Vec<&'static str> {
    if recipe.transfer_syntax == RLE_LOSSLESS {
        vec![
            "open_file",
            "read_metadata",
            "decode_rle_lossless_pixels",
            if recipe.palette.is_some() {
                "render_palette_color"
            } else if recipe.samples_per_pixel > 1 {
                "render_color"
            } else {
                "render_grayscale"
            },
        ]
    } else if recipe.transfer_syntax == JPEG_BASELINE_8BIT {
        vec![
            "open_file",
            "read_metadata",
            "decode_jpeg_baseline_pixels",
            "render_color",
        ]
    } else if recipe.transfer_syntax == JPEG_LS_LOSSLESS {
        vec![
            "open_file",
            "read_metadata",
            "decode_jpeg_ls_lossless_pixels",
            "render_grayscale",
        ]
    } else if recipe.transfer_syntax == JPEG_XL_LOSSLESS {
        vec![
            "open_file",
            "read_metadata",
            "decode_jpeg_xl_lossless_pixels",
            "render_color",
        ]
    } else if recipe.transfer_syntax == JPEG_2000_LOSSLESS {
        vec![
            "open_file",
            "read_metadata",
            "decode_jpeg_2000_lossless_pixels",
            "render_grayscale",
        ]
    } else if recipe.transfer_syntax == HTJ2K_LOSSLESS {
        vec![
            "open_file",
            "read_metadata",
            "decode_htj2k_lossless_pixels",
            "render_grayscale",
        ]
    } else if recipe.transfer_syntax == JPEG_LOSSLESS_PROCESS_14 {
        vec![
            "open_file",
            "read_metadata",
            "decode_jpeg_lossless_process_14_pixels",
            "render_grayscale",
        ]
    } else if recipe.transfer_syntax == JPEG_LOSSLESS_SV1 {
        vec![
            "open_file",
            "read_metadata",
            "decode_jpeg_lossless_sv1_pixels",
            "render_grayscale",
        ]
    } else {
        vec!["open_file", "read_metadata", "render_native_pixels"]
    }
}

fn pixel_is_vl_photographic(recipe: PixelRecipe) -> bool {
    recipe.case_id.starts_with("vl/photo/")
}

fn pixel_sop_class_uid(recipe: PixelRecipe) -> &'static str {
    if pixel_is_vl_photographic(recipe) {
        uids::VL_PHOTOGRAPHIC_IMAGE_STORAGE
    } else {
        uids::SECONDARY_CAPTURE_IMAGE_STORAGE
    }
}

fn pixel_sop_class_name(recipe: PixelRecipe) -> &'static str {
    if pixel_is_vl_photographic(recipe) {
        "VL Photographic Image Storage"
    } else {
        "Secondary Capture Image Storage"
    }
}

fn pixel_iod_name(recipe: PixelRecipe) -> &'static str {
    if pixel_is_vl_photographic(recipe) {
        "VL Photographic Image"
    } else {
        "Secondary Capture Image"
    }
}

fn pixel_modality(recipe: PixelRecipe) -> &'static str {
    if pixel_is_vl_photographic(recipe) {
        "XC"
    } else {
        "OT"
    }
}

fn pixel_conversion_type(recipe: PixelRecipe) -> Value {
    if pixel_is_vl_photographic(recipe) {
        Value::Null
    } else {
        Value::String("SYN".to_string())
    }
}

fn pixel_image_type(recipe: PixelRecipe) -> Value {
    if pixel_is_vl_photographic(recipe) {
        Value::String("ORIGINAL\\PRIMARY".to_string())
    } else {
        Value::Null
    }
}

fn pixel_determinism(recipe: PixelRecipe) -> &'static str {
    if recipe.transfer_syntax == JPEG_BASELINE_8BIT
        || recipe.transfer_syntax == JPEG_LS_LOSSLESS
        || recipe.transfer_syntax == JPEG_XL_LOSSLESS
        || recipe.transfer_syntax == JPEG_2000_LOSSLESS
        || recipe.transfer_syntax == HTJ2K_LOSSLESS
        || recipe.transfer_syntax == JPEG_LOSSLESS_PROCESS_14
        || recipe.transfer_syntax == JPEG_LOSSLESS_SV1
    {
        "semantic_stable"
    } else {
        "byte_stable"
    }
}

fn pixel_recipe_frame_bytes(recipe: PixelRecipe) -> Result<Vec<&'static [u8]>, String> {
    let frame_len = pixel_recipe_frame_len(recipe)?;
    if frame_len == 0 {
        return Err(format!(
            "pixel recipe {} produced a zero-length frame",
            recipe.case_id
        ));
    }
    if recipe.pixel_bytes.len() % frame_len != 0 {
        return Err(format!(
            "pixel recipe {} has {} pixel bytes, which is not divisible by frame length {}",
            recipe.case_id,
            recipe.pixel_bytes.len(),
            frame_len
        ));
    }
    Ok(recipe.pixel_bytes.chunks_exact(frame_len).collect())
}

fn pixel_recipe_frame_len(recipe: PixelRecipe) -> Result<usize, String> {
    if recipe.bits_allocated == 1 {
        let frame_bits = usize::from(recipe.rows)
            .checked_mul(usize::from(recipe.columns))
            .and_then(|value| value.checked_mul(usize::from(recipe.samples_per_pixel)))
            .ok_or_else(|| format!("pixel recipe {} frame size overflowed", recipe.case_id))?;
        return Ok(frame_bits.div_ceil(8));
    }
    if recipe.photometric_interpretation == "YBR_FULL_422" {
        let bytes_per_sample = usize::from(recipe.bits_allocated / 8);
        return usize::from(recipe.rows)
            .checked_mul(usize::from(recipe.columns))
            .and_then(|value| value.checked_mul(2))
            .and_then(|value| value.checked_mul(bytes_per_sample))
            .ok_or_else(|| format!("pixel recipe {} frame size overflowed", recipe.case_id));
    }

    let bytes_per_sample = usize::from(recipe.bits_allocated / 8);
    usize::from(recipe.rows)
        .checked_mul(usize::from(recipe.columns))
        .and_then(|value| value.checked_mul(usize::from(recipe.samples_per_pixel)))
        .and_then(|value| value.checked_mul(bytes_per_sample))
        .ok_or_else(|| format!("pixel recipe {} frame size overflowed", recipe.case_id))
}

fn write_classic_ct_case(
    run: &PreparedGenerationRun,
    case: &Value,
    recipe: ClassicCtRecipe,
    standards_lock_sha256: &str,
) -> Result<Vec<GeneratedFile>, GenerateError> {
    let study_instance_uid = deterministic_classic_ct_uid(
        standards_lock_sha256,
        recipe,
        run.seed,
        UidRole::StudyInstance,
        0,
    );
    let series_instance_uid = deterministic_classic_ct_uid(
        standards_lock_sha256,
        recipe,
        run.seed,
        UidRole::SeriesInstance,
        0,
    );
    let frame_of_reference_uid = deterministic_classic_ct_uid(
        standards_lock_sha256,
        recipe,
        run.seed,
        UidRole::FrameOfReference,
        0,
    );
    let implementation_class_uid = deterministic_implementation_uid(standards_lock_sha256);

    let mut generated_files = Vec::with_capacity(recipe.slices.len());
    for (slice_index, slice) in recipe.slices.iter().enumerate() {
        let sop_instance_uid = deterministic_classic_ct_uid(
            standards_lock_sha256,
            recipe,
            run.seed,
            UidRole::SopInstance,
            slice_index as u32,
        );
        let relative_path = if recipe.slices.len() == 1 {
            format!("{}/instance.dcm", recipe.case_id)
        } else {
            format!("{}/slice-{:03}.dcm", recipe.case_id, slice_index + 1)
        };
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
        if recipe.slices.len() > 1 {
            put_str(&mut obj, tags::PATIENT_POSITION, VR::CS, "");
            put_str(&mut obj, tags::IMAGE_LATERALITY, VR::CS, "U");
        }
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
        if let Some(spacing_between_slices) = recipe.spacing_between_slices {
            put_str(
                &mut obj,
                tags::SPACING_BETWEEN_SLICES,
                VR::DS,
                spacing_between_slices,
            );
        }

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

        let compressed_pixel_data = if recipe.transfer_syntax == RLE_LOSSLESS {
            let rle_encoder = NativeRleLosslessEncoder::new();
            let encoded_frame = rle_encoder
                .encode_frame(FrameEncodeInput {
                    native_frame: slice.pixel_bytes,
                    rows: recipe.rows,
                    columns: recipe.columns,
                    samples_per_pixel: 1,
                    bits_allocated: 16,
                    bits_stored: 12,
                    photometric_interpretation: "MONOCHROME2",
                })
                .map_err(|err| GenerateError::WriteDicomFile {
                    path: path.clone(),
                    message: err.to_string(),
                })?;
            let compressed_frames = vec![encoded_frame.bytes];
            let encapsulated = EncapsulatedPixelData::one_fragment_per_frame(
                &compressed_frames,
                BasicOffsetTablePolicy::Populated,
            )
            .map_err(|err| GenerateError::WriteDicomFile {
                path: path.clone(),
                message: err.to_string(),
            })?;
            obj.put(DataElement::new(
                tags::PIXEL_DATA,
                VR::OB,
                PixelFragmentSequence::new(
                    encapsulated.basic_offset_table.offsets.clone(),
                    compressed_frames,
                ),
            ));
            Some((FrameEncoder::backend(&rle_encoder), encapsulated))
        } else {
            obj.put(DataElement::new(
                tags::PIXEL_DATA,
                VR::OW,
                PrimitiveValue::from(slice.pixel_bytes),
            ));
            None
        };

        let file_obj = obj
            .with_meta(
                FileMetaTableBuilder::new()
                    .transfer_syntax(recipe.transfer_syntax.uid)
                    .implementation_class_uid(&implementation_class_uid)
                    .implementation_version_name(crate::IMPLEMENTATION_VERSION_NAME),
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

        let decoded_frame_hash = sha256_hex(slice.pixel_bytes);
        let decoded_frame_hashes = [decoded_frame_hash.as_str()];
        let validated = validate_part10_file(
            &path,
            &Part10Expectations {
                sop_class_uid: uids::CT_IMAGE_STORAGE,
                sop_instance_uid: &sop_instance_uid,
                transfer_syntax_uid: recipe.transfer_syntax.uid,
                implementation_class_uid: &implementation_class_uid,
                synthetic_data: "YES",
                rows: recipe.rows,
                columns: recipe.columns,
                frames: 1,
                samples_per_pixel: 1,
                photometric_interpretation: "MONOCHROME2",
                bits_allocated: 16,
                bits_stored: 12,
                high_bit: 11,
                pixel_representation: 1,
                planar_configuration: None,
                pixel_data_vr: if compressed_pixel_data.is_some() {
                    VR::OB
                } else {
                    VR::OW
                },
                pixel_data_length_formula: compressed_pixel_data
                    .as_ref()
                    .map(|(_, encapsulated)| PixelDataLengthFormula::Encapsulated {
                        fragments: encapsulated.fragments.len(),
                        basic_offset_table_offsets: encapsulated.basic_offset_table.offsets.len(),
                    })
                    .unwrap_or(PixelDataLengthFormula::ContiguousSamples),
                decoded_frame_hashes: if compressed_pixel_data.is_some() {
                    &decoded_frame_hashes
                } else {
                    &[]
                },
                palette: None,
                padding: None,
                ct_image: Some(CtImageExpectations {
                    modality: "CT",
                    frame_of_reference_uid: &frame_of_reference_uid,
                    image_type: "ORIGINAL\\PRIMARY\\AXIAL",
                    pixel_spacing: recipe.pixel_spacing,
                    image_orientation_patient: recipe.image_orientation_patient,
                    image_position_patient: slice.image_position_patient,
                    slice_thickness: recipe.slice_thickness,
                    kvp: recipe.kvp,
                    acquisition_number: "1",
                    rescale_intercept: recipe.rescale_intercept,
                    rescale_slope: recipe.rescale_slope,
                    rescale_type: recipe.rescale_type,
                    window_center: recipe.window_center,
                    window_width: recipe.window_width,
                }),
                enhanced_ct_image: None,
                enhanced_mr_image: None,
                mg_image: None,
                dx_image: None,
                us_image: None,
                cr_image: None,
                mr_image: None,
                segmentation: None,
            },
        )?;

        generated_files.push(GeneratedFile {
            case_id: recipe.case_id.to_string(),
            manifest_entry: classic_ct_manifest_entry(
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
                compressed_pixel_data.as_ref(),
            ),
        });
    }

    Ok(generated_files)
}

#[allow(clippy::too_many_arguments)]
fn classic_ct_manifest_entry(
    case: &Value,
    recipe: ClassicCtRecipe,
    slice: ClassicCtSliceRecipe,
    relative_path: &str,
    study_instance_uid: &str,
    series_instance_uid: &str,
    sop_instance_uid: &str,
    frame_of_reference_uid: &str,
    implementation_class_uid: &str,
    bytes: &[u8],
    validation: Value,
    slice_index: usize,
    compressed_pixel_data: Option<&(crate::codecs::CodecBackendInfo, EncapsulatedPixelData)>,
) -> Value {
    let mut standards_evidence = standards_evidence_from_case(case);
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

    if recipe.transfer_syntax == RLE_LOSSLESS {
        standards_evidence.extend([
            serde_json::json!({
                "source": "dicom-standard-kb",
                "edition": "2026b",
                "query": "lookup_uid RLELossless",
                "covered": true,
                "part": "PS3.6",
                "anchor": "table_A-1"
            }),
            serde_json::json!({
                "source": "dicom-standard-kb",
                "edition": "2026b",
                "query": "retrieve_standard_text PS3.5 sect_8.2.2",
                "covered": true,
                "part": "PS3.5",
                "anchor": "sect_8.2.2"
            }),
            serde_json::json!({
                "source": "dicom-standard-kb",
                "edition": "2026b",
                "query": "retrieve_standard_text PS3.5 sect_A.4",
                "covered": true,
                "part": "PS3.5",
                "anchor": "sect_A.4"
            }),
        ]);
    }

    let frame_hash = sha256_hex(slice.pixel_bytes);
    let pixel_data_manifest = if let Some((backend, encapsulated)) = compressed_pixel_data {
        serde_json::json!({
            "vr": "OB",
            "native_or_encapsulated": "encapsulated",
            "value_length": Value::Null,
            "frame_count": 1,
            "frame_hashes": [frame_hash],
            "codec": {
                "backend_id": backend.backend_id,
                "backend_kind": backend.backend_kind.as_str(),
                "display_name": backend.display_name,
                "version": backend.version,
                "transfer_syntax_uid": backend.transfer_syntax_uid,
                "feature_gate": backend.feature_gate,
                "determinism": backend.determinism.as_str()
            },
            "encapsulated_pixel_data": {
                "basic_offset_table": {
                    "present": true,
                    "populated": encapsulated.basic_offset_table.is_populated(),
                    "offset_count": encapsulated.basic_offset_table.offsets.len(),
                    "offsets": encapsulated.basic_offset_table.offsets.clone()
                },
                "fragments_per_frame": encapsulated.fragments_per_frame.clone(),
                "fragments": encapsulated.fragments.iter().map(|fragment| {
                    serde_json::json!({
                        "frame_index": fragment.frame_index,
                        "item_start_offset": fragment.item_start_offset,
                        "compressed_length": fragment.compressed_length,
                        "padded_length": fragment.padded_length
                    })
                }).collect::<Vec<_>>(),
                "extended_offset_table": {
                    "present": false,
                    "lengths_present": false,
                    "offset_count": 0,
                    "length_count": 0
                },
                "compressed_frame_hashes": encapsulated.compressed_frame_hashes.clone()
            }
        })
    } else {
        serde_json::json!({
            "vr": "OW",
            "native_or_encapsulated": "native",
            "value_length": slice.pixel_bytes.len(),
            "frame_count": 1,
            "frame_hashes": [frame_hash]
        })
    };

    let mut recipe_geometry = serde_json::json!({
        "pixel_spacing": recipe.pixel_spacing,
        "image_orientation_patient": recipe.image_orientation_patient,
        "image_position_patient": slice.image_position_patient,
        "slice_thickness": recipe.slice_thickness
    });
    let mut expected_semantics = serde_json::json!({
        "synthetic_data": "YES",
        "image_type": "ORIGINAL\\PRIMARY\\AXIAL",
        "pixel_min": slice.pixel_min,
        "pixel_max": slice.pixel_max,
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
    });
    if recipe.slices.len() > 1 {
        let geometry = recipe_geometry
            .as_object_mut()
            .expect("CT recipe geometry must be an object");
        geometry.insert(
            "spacing_between_slices".to_string(),
            Value::from(
                recipe
                    .spacing_between_slices
                    .expect("CT series spacing is required"),
            ),
        );
        geometry.insert(
            "position_along_normal".to_string(),
            Value::from(slice.position_along_normal),
        );
        geometry.insert(
            "slice_order_index".to_string(),
            Value::from(slice_index + 1),
        );
        geometry.insert("slice_count".to_string(), Value::from(recipe.slices.len()));

        let semantics = expected_semantics
            .as_object_mut()
            .expect("CT expected semantics must be an object");
        semantics.insert(
            "series_instance_count".to_string(),
            Value::from(recipe.slices.len()),
        );
        semantics.insert(
            "shared_study_series_frame_of_reference".to_string(),
            Value::Bool(true),
        );
        semantics.insert(
            "geometry_sort_key".to_string(),
            serde_json::json!({
                "image_orientation_patient": recipe.image_orientation_patient,
                "position_along_normal": slice.position_along_normal,
                "slice_order_index": slice_index + 1
            }),
        );
    }

    let mut manifest_entry = serde_json::json!({
        "case_id": recipe.case_id,
        "profile_membership": classic_ct_profile_membership(recipe),
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
                "pixel_values": slice.pixel_values,
                "rescale": {
                    "intercept": recipe.rescale_intercept,
                    "slope": recipe.rescale_slope,
                    "type": recipe.rescale_type
                },
                "window": {
                    "center": recipe.window_center,
                    "width": recipe.window_width
                },
                "kvp": recipe.kvp,
                "acquisition_number": "1",
                "geometry": recipe_geometry
            }
        },
        "dicom": {
            "sop_class_uid": uids::CT_IMAGE_STORAGE,
            "sop_class_name": "CT Image Storage",
            "iod_name": "CT Image",
            "modality": "CT",
            "transfer_syntax_uid": recipe.transfer_syntax.uid,
            "transfer_syntax_name": recipe.transfer_syntax.name
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
        "pixel_data": pixel_data_manifest,
        "expected_capabilities": classic_ct_expected_capabilities(recipe),
        "expected_semantics": expected_semantics,
        "expected_visual_checks": {
            "pattern": "2x2_signed_ct_hu_gradient"
        },
        "validation": validation,
        "known_stressors": classic_ct_known_stressors(recipe),
        "standards_evidence": deduplicated_standards_evidence(standards_evidence)
    });
    if recipe.slices.len() > 1 {
        manifest_entry
            .as_object_mut()
            .expect("CT manifest entry must be an object")
            .insert(
                "expected_geometry".to_string(),
                serde_json::json!({
                    "sort_basis": "image_position_patient_projected_on_slice_normal",
                    "sort_direction": "ascending",
                    "position_tolerance_mm": 0.00001,
                    "spacing_tolerance_mm": 0.00001,
                    "series_instance_count": recipe.slices.len(),
                    "geometric_order_index": slice_index + 1,
                    "position_along_normal_mm": slice.position_along_normal,
                    "instance_number": slice.instance_number.parse::<i64>().expect("CT Instance Number recipe must be numeric"),
                    "instance_number_order_index": classic_ct_instance_number_order_index(recipe, slice.instance_number),
                    "sorting_conflict_expected": true
                }),
            );
    }
    manifest_entry
}

fn classic_ct_instance_number_order_index(recipe: ClassicCtRecipe, instance_number: &str) -> usize {
    let instance_number = instance_number
        .parse::<i64>()
        .expect("CT Instance Number recipe must be numeric");
    let mut instance_numbers = recipe
        .slices
        .iter()
        .map(|slice| {
            slice
                .instance_number
                .parse::<i64>()
                .expect("CT Instance Number recipe must be numeric")
        })
        .collect::<Vec<_>>();
    instance_numbers.sort_unstable();
    instance_numbers
        .iter()
        .position(|candidate| *candidate == instance_number)
        .expect("CT Instance Number recipe must be present")
        + 1
}

fn classic_ct_profile_membership(recipe: ClassicCtRecipe) -> &'static [&'static str] {
    if recipe.transfer_syntax == RLE_LOSSLESS {
        &["extended"]
    } else {
        &["core"]
    }
}

fn classic_ct_expected_capabilities(recipe: ClassicCtRecipe) -> Vec<&'static str> {
    let mut capabilities = vec!["open_file", "read_metadata"];
    if recipe.transfer_syntax == RLE_LOSSLESS {
        capabilities.push("decode_rle_lossless_pixels");
    } else {
        capabilities.push("render_native_pixels");
    }
    capabilities.extend(["apply_modality_rescale", "apply_window"]);
    capabilities
}

fn classic_ct_known_stressors(recipe: ClassicCtRecipe) -> Vec<&'static str> {
    let mut stressors = vec![
        "ct_image_storage",
        "signed_12_bit_pixels",
        "modality_rescale",
        "window_center_width",
    ];
    if recipe.transfer_syntax == RLE_LOSSLESS {
        stressors.push("encapsulated_pixel_data");
        stressors.push("rle_lossless_transfer_syntax");
        stressors.push("compressed_modality_pixels");
    }
    stressors
}

fn write_enhanced_ct_case(
    run: &PreparedGenerationRun,
    case: &Value,
    recipe: EnhancedCtRecipe,
    standards_lock_sha256: &str,
) -> Result<GeneratedFile, GenerateError> {
    let study_instance_uid = deterministic_enhanced_ct_uid(
        standards_lock_sha256,
        recipe,
        run.seed,
        UidRole::StudyInstance,
    );
    let series_instance_uid = deterministic_enhanced_ct_uid(
        standards_lock_sha256,
        recipe,
        run.seed,
        UidRole::SeriesInstance,
    );
    let sop_instance_uid = deterministic_enhanced_ct_uid(
        standards_lock_sha256,
        recipe,
        run.seed,
        UidRole::SopInstance,
    );
    let frame_of_reference_uid = deterministic_enhanced_ct_uid(
        standards_lock_sha256,
        recipe,
        run.seed,
        UidRole::FrameOfReference,
    );
    let dimension_organization_uid = deterministic_enhanced_ct_uid(
        standards_lock_sha256,
        recipe,
        run.seed,
        UidRole::DimensionOrganization,
    );
    let irradiation_event_uid = deterministic_enhanced_ct_uid(
        standards_lock_sha256,
        recipe,
        run.seed,
        UidRole::IrradiationEvent,
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
        uids::ENHANCED_CT_IMAGE_STORAGE,
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
    put_str(&mut obj, tags::STUDY_ID, VR::SH, "DTS-ECT");
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
    put_str(&mut obj, tags::DEVICE_SERIAL_NUMBER, VR::LO, "DTS-ECT-0001");
    put_str(
        &mut obj,
        tags::SOFTWARE_VERSIONS,
        VR::LO,
        crate::PACKAGE_VERSION,
    );

    put_str(&mut obj, tags::IMAGE_TYPE, VR::CS, recipe.frame_type);
    put_str(&mut obj, tags::INSTANCE_NUMBER, VR::IS, "1");
    put_str(&mut obj, tags::CONTENT_DATE, VR::DA, "20260101");
    put_str(&mut obj, tags::CONTENT_TIME, VR::TM, "000000");
    put_empty_sequence(&mut obj, tags::ACQUISITION_CONTEXT_SEQUENCE);
    put_str(
        &mut obj,
        tags::PIXEL_PRESENTATION,
        VR::CS,
        recipe.pixel_presentation,
    );
    put_str(
        &mut obj,
        tags::VOLUMETRIC_PROPERTIES,
        VR::CS,
        recipe.volumetric_properties,
    );
    put_str(
        &mut obj,
        tags::VOLUME_BASED_CALCULATION_TECHNIQUE,
        VR::CS,
        recipe.volume_based_calculation_technique,
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
    put_u16(&mut obj, tags::BITS_STORED, VR::US, 16);
    put_u16(&mut obj, tags::HIGH_BIT, VR::US, 15);
    put_u16(&mut obj, tags::PIXEL_REPRESENTATION, VR::US, 0);
    put_str(
        &mut obj,
        tags::NUMBER_OF_FRAMES,
        VR::IS,
        &recipe.frames.to_string(),
    );

    put_enhanced_ct_dimension_sequences(&mut obj, &dimension_organization_uid);
    put_enhanced_ct_functional_groups(
        &mut obj,
        recipe,
        &irradiation_event_uid,
        recipe.image_position_patient,
        ENHANCED_CT_DIMENSION_INDEX_VALUES,
    );

    obj.put(DataElement::new(
        tags::PIXEL_DATA,
        VR::OW,
        PrimitiveValue::from(recipe.pixel_bytes),
    ));

    let file_obj = obj
        .with_meta(
            FileMetaTableBuilder::new()
                .transfer_syntax(EXPLICIT_VR_LITTLE_ENDIAN.uid)
                .implementation_class_uid(&implementation_class_uid)
                .implementation_version_name(crate::IMPLEMENTATION_VERSION_NAME),
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
            sop_class_uid: uids::ENHANCED_CT_IMAGE_STORAGE,
            sop_instance_uid: &sop_instance_uid,
            transfer_syntax_uid: EXPLICIT_VR_LITTLE_ENDIAN.uid,
            implementation_class_uid: &implementation_class_uid,
            synthetic_data: "YES",
            rows: recipe.rows,
            columns: recipe.columns,
            frames: recipe.frames,
            samples_per_pixel: 1,
            photometric_interpretation: "MONOCHROME2",
            bits_allocated: 16,
            bits_stored: 16,
            high_bit: 15,
            pixel_representation: 0,
            planar_configuration: None,
            pixel_data_vr: VR::OW,
            pixel_data_length_formula: PixelDataLengthFormula::ContiguousSamples,
            decoded_frame_hashes: &[],
            palette: None,
            padding: None,
            ct_image: None,
            enhanced_ct_image: Some(EnhancedCtImageExpectations {
                modality: "CT",
                frame_of_reference_uid: &frame_of_reference_uid,
                image_type: recipe.frame_type,
                number_of_frames: recipe.frames,
                shared_functional_groups: 1,
                per_frame_functional_groups: recipe.frames as usize,
                dimension_organization_uid: &dimension_organization_uid,
                dimension_index_count: 1,
                pixel_spacing: recipe.pixel_spacing,
                image_orientation_patient: recipe.image_orientation_patient,
                image_position_patient: recipe.image_position_patient,
                dimension_index_values: ENHANCED_CT_DIMENSION_INDEX_VALUES,
                frame_type: recipe.frame_type,
                pixel_presentation: recipe.pixel_presentation,
                volumetric_properties: recipe.volumetric_properties,
                volume_based_calculation_technique: recipe.volume_based_calculation_technique,
                rescale_intercept: recipe.rescale_intercept,
                rescale_slope: recipe.rescale_slope,
                rescale_type: recipe.rescale_type,
                irradiation_event_uid: &irradiation_event_uid,
                concatenation: None,
            }),
            enhanced_mr_image: None,
            mg_image: None,
            dx_image: None,
            us_image: None,
            cr_image: None,
            mr_image: None,
            segmentation: None,
        },
    )?;

    Ok(GeneratedFile {
        case_id: recipe.case_id.to_string(),
        manifest_entry: enhanced_ct_manifest_entry(
            case,
            recipe,
            &relative_path,
            recipe.frames,
            recipe.pixel_bytes,
            recipe.pixel_values,
            recipe.image_position_patient,
            ENHANCED_CT_DIMENSION_INDEX_VALUES,
            &study_instance_uid,
            &series_instance_uid,
            &sop_instance_uid,
            &frame_of_reference_uid,
            &dimension_organization_uid,
            &irradiation_event_uid,
            &implementation_class_uid,
            None,
            &validated.bytes,
            validated.validation,
        ),
    })
}

fn write_segmentation_case(
    run: &PreparedGenerationRun,
    case: &Value,
    recipe: SegmentationRecipe,
    source: &GeneratedSourceObject,
    standards_lock_sha256: &str,
) -> Result<GeneratedFile, GenerateError> {
    let series_instance_uid = deterministic_segmentation_uid(
        standards_lock_sha256,
        recipe,
        run.seed,
        UidRole::SeriesInstance,
    );
    let sop_instance_uid = deterministic_segmentation_uid(
        standards_lock_sha256,
        recipe,
        run.seed,
        UidRole::SopInstance,
    );
    let dimension_organization_uid = deterministic_segmentation_uid(
        standards_lock_sha256,
        recipe,
        run.seed,
        UidRole::DimensionOrganization,
    );
    let implementation_class_uid = deterministic_implementation_uid(standards_lock_sha256);
    let frame_of_reference_uid =
        source
            .frame_of_reference_uid
            .as_deref()
            .ok_or_else(|| GenerateError::MetadataShape {
                path: PathBuf::from(recipe.case_id),
                message: "segmentation source object must include a Frame of Reference UID",
            })?;

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
        &source.study_instance_uid,
    );
    put_str(&mut obj, tags::STUDY_DATE, VR::DA, "20260101");
    put_str(&mut obj, tags::STUDY_TIME, VR::TM, "000000");
    put_str(&mut obj, tags::REFERRING_PHYSICIAN_NAME, VR::PN, "");
    put_str(&mut obj, tags::STUDY_ID, VR::SH, "DTS-SEG");
    put_str(&mut obj, tags::ACCESSION_NUMBER, VR::SH, "");

    put_str(&mut obj, tags::MODALITY, VR::CS, "SEG");
    put_str(
        &mut obj,
        tags::SERIES_INSTANCE_UID,
        VR::UI,
        &series_instance_uid,
    );
    put_str(&mut obj, tags::SERIES_NUMBER, VR::IS, "51");
    put_str(
        &mut obj,
        tags::FRAME_OF_REFERENCE_UID,
        VR::UI,
        frame_of_reference_uid,
    );
    put_str(&mut obj, tags::POSITION_REFERENCE_INDICATOR, VR::LO, "");

    put_str(&mut obj, tags::MANUFACTURER, VR::LO, "dicom-test-suite");
    put_str(
        &mut obj,
        tags::MANUFACTURER_MODEL_NAME,
        VR::LO,
        recipe.recipe_id,
    );
    put_str(&mut obj, tags::DEVICE_SERIAL_NUMBER, VR::LO, "DTS-SEG-0001");
    put_str(
        &mut obj,
        tags::SOFTWARE_VERSIONS,
        VR::LO,
        crate::PACKAGE_VERSION,
    );

    put_str(&mut obj, TAG_IMAGE_TYPE, VR::CS, "DERIVED\\PRIMARY");
    put_str(&mut obj, tags::INSTANCE_NUMBER, VR::IS, "1");
    put_str(&mut obj, tags::CONTENT_DATE, VR::DA, "20260101");
    put_str(&mut obj, tags::CONTENT_TIME, VR::TM, "000000");
    put_str(&mut obj, TAG_CONTENT_LABEL, VR::CS, "DTSSEG");
    put_str(
        &mut obj,
        TAG_CONTENT_DESCRIPTION,
        VR::LO,
        "Synthetic segmentation",
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
    put_u16(
        &mut obj,
        tags::BITS_ALLOCATED,
        VR::US,
        recipe.bits_allocated,
    );
    put_u16(&mut obj, tags::BITS_STORED, VR::US, recipe.bits_stored);
    put_u16(&mut obj, tags::HIGH_BIT, VR::US, recipe.high_bit);
    put_u16(&mut obj, tags::PIXEL_REPRESENTATION, VR::US, 0);
    put_str(
        &mut obj,
        tags::NUMBER_OF_FRAMES,
        VR::IS,
        &recipe.frames.to_string(),
    );

    put_str(
        &mut obj,
        TAG_SEGMENTATION_TYPE,
        VR::CS,
        recipe.segmentation_type,
    );
    if let Some(segmentation_fractional_type) = recipe.segmentation_fractional_type {
        put_str(
            &mut obj,
            TAG_SEGMENTATION_FRACTIONAL_TYPE,
            VR::CS,
            segmentation_fractional_type,
        );
    }
    if let Some(maximum_fractional_value) = recipe.maximum_fractional_value {
        put_u16(
            &mut obj,
            TAG_MAXIMUM_FRACTIONAL_VALUE,
            VR::US,
            maximum_fractional_value,
        );
    }
    put_segmentation_segment_sequence(&mut obj, recipe);
    put_segmentation_dimension_sequences(&mut obj, &dimension_organization_uid);
    put_segmentation_functional_groups(&mut obj, recipe, source);
    put_common_instance_reference(&mut obj, source);

    let frame_byte_len = segmentation_frame_byte_len(recipe);
    let native_frames = recipe
        .pixel_bytes
        .chunks(frame_byte_len)
        .collect::<Vec<_>>();
    let decoded_frame_hash_values = native_frames
        .iter()
        .map(|frame| sha256_hex(frame))
        .collect::<Vec<_>>();
    let decoded_frame_hashes = decoded_frame_hash_values
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    let compressed_pixel_data: Option<(
        crate::codecs::CodecBackendInfo,
        EncapsulatedPixelData,
        Vec<Value>,
    )> = if recipe.transfer_syntax == DEFLATED_IMAGE_FRAME {
        #[cfg(feature = "deflate")]
        {
            let encoder = DicomRsDeflatedImageFrameEncoder::new();
            let mut compressed_frames = Vec::with_capacity(native_frames.len());
            let mut codec_internal_validation = Vec::new();
            for native_frame in &native_frames {
                let encoded_frame = encoder
                    .encode_frame(FrameEncodeInput {
                        native_frame,
                        rows: recipe.rows,
                        columns: recipe.columns,
                        samples_per_pixel: 1,
                        bits_allocated: recipe.bits_allocated,
                        bits_stored: recipe.bits_stored,
                        photometric_interpretation: "MONOCHROME2",
                    })
                    .map_err(|err| GenerateError::WriteDicomFile {
                        path: path.clone(),
                        message: err.to_string(),
                    })?;
                codec_internal_validation.push(validate_deflated_image_frame_round_trip(
                    &path,
                    recipe,
                    native_frame,
                    &encoded_frame.bytes,
                )?);
                compressed_frames.push(encoded_frame.bytes);
            }
            let encapsulated = EncapsulatedPixelData::one_fragment_per_frame(
                &compressed_frames,
                BasicOffsetTablePolicy::Populated,
            )
            .map_err(|err| GenerateError::WriteDicomFile {
                path: path.clone(),
                message: err.to_string(),
            })?;
            obj.put(DataElement::new(
                tags::PIXEL_DATA,
                VR::OB,
                PixelFragmentSequence::new(
                    encapsulated.basic_offset_table.offsets.clone(),
                    compressed_frames,
                ),
            ));
            Some((
                FrameEncoder::backend(&encoder),
                encapsulated,
                codec_internal_validation,
            ))
        }
        #[cfg(not(feature = "deflate"))]
        {
            return Err(GenerateError::WriteDicomFile {
                path: path.clone(),
                message: "Deflated Image Frame generation requires the deflate Cargo feature"
                    .to_string(),
            });
        }
    } else {
        obj.put(DataElement::new(
            tags::PIXEL_DATA,
            VR::OB,
            PrimitiveValue::from(recipe.pixel_bytes),
        ));
        None
    };

    let file_obj = obj
        .with_meta(
            FileMetaTableBuilder::new()
                .transfer_syntax(recipe.transfer_syntax.uid)
                .implementation_class_uid(&implementation_class_uid)
                .implementation_version_name(crate::IMPLEMENTATION_VERSION_NAME),
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

    let mut validated = validate_part10_file(
        &path,
        &Part10Expectations {
            sop_class_uid: recipe.sop_class_uid,
            sop_instance_uid: &sop_instance_uid,
            transfer_syntax_uid: recipe.transfer_syntax.uid,
            implementation_class_uid: &implementation_class_uid,
            synthetic_data: "YES",
            rows: recipe.rows,
            columns: recipe.columns,
            frames: recipe.frames,
            samples_per_pixel: 1,
            photometric_interpretation: "MONOCHROME2",
            bits_allocated: recipe.bits_allocated,
            bits_stored: recipe.bits_stored,
            high_bit: recipe.high_bit,
            pixel_representation: 0,
            planar_configuration: None,
            pixel_data_vr: VR::OB,
            pixel_data_length_formula: compressed_pixel_data
                .as_ref()
                .map(
                    |(_, encapsulated, _)| PixelDataLengthFormula::Encapsulated {
                        fragments: encapsulated.fragments.len(),
                        basic_offset_table_offsets: encapsulated.basic_offset_table.offsets.len(),
                    },
                )
                .unwrap_or(recipe.pixel_data_length_formula),
            decoded_frame_hashes: if compressed_pixel_data.is_some() {
                &decoded_frame_hashes
            } else {
                &[]
            },
            palette: None,
            padding: None,
            ct_image: None,
            enhanced_ct_image: None,
            enhanced_mr_image: None,
            mg_image: None,
            dx_image: None,
            us_image: None,
            cr_image: None,
            mr_image: None,
            segmentation: Some(SegmentationExpectations {
                modality: "SEG",
                frame_of_reference_uid,
                image_type: "DERIVED\\PRIMARY",
                segmentation_type: recipe.segmentation_type,
                segmentation_fractional_type: recipe.segmentation_fractional_type,
                maximum_fractional_value: recipe.maximum_fractional_value,
                segment_sequence_items: 1,
                shared_functional_groups: 1,
                per_frame_functional_groups: recipe.frames as usize,
                dimension_organization_uid: &dimension_organization_uid,
                dimension_index_count: 1,
                referenced_sop_class_uid: &source.sop_class_uid,
                referenced_sop_instance_uid: &source.sop_instance_uid,
                referenced_frame_numbers: recipe.referenced_frame_numbers,
            }),
        },
    )?;
    if let Some((_, _, codec_internal_validation)) = &compressed_pixel_data {
        for result in codec_internal_validation {
            append_internal_validation(&mut validated.validation, result.clone());
        }
    }

    Ok(GeneratedFile {
        case_id: recipe.case_id.to_string(),
        manifest_entry: segmentation_manifest_entry(
            case,
            recipe,
            source,
            &relative_path,
            &source.study_instance_uid,
            &series_instance_uid,
            &sop_instance_uid,
            frame_of_reference_uid,
            &dimension_organization_uid,
            &implementation_class_uid,
            &validated.bytes,
            validated.validation,
            compressed_pixel_data
                .as_ref()
                .map(|(backend, encapsulated, _)| (*backend, encapsulated)),
        ),
    })
}

fn write_presentation_state_case(
    run: &PreparedGenerationRun,
    case: &Value,
    recipe: PresentationStateRecipe,
    source: &GeneratedSourceObject,
    standards_lock_sha256: &str,
) -> Result<GeneratedFile, GenerateError> {
    let series_instance_uid = deterministic_presentation_state_uid(
        standards_lock_sha256,
        recipe,
        run.seed,
        UidRole::SeriesInstance,
    );
    let sop_instance_uid = deterministic_presentation_state_uid(
        standards_lock_sha256,
        recipe,
        run.seed,
        UidRole::SopInstance,
    );
    let implementation_class_uid = deterministic_implementation_uid(standards_lock_sha256);
    let source_series_instance_uid =
        source
            .series_instance_uid
            .as_deref()
            .ok_or_else(|| GenerateError::MetadataShape {
                path: PathBuf::from(recipe.case_id),
                message: "presentation state source object must include a Series Instance UID",
            })?;

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
        GRAYSCALE_SOFTCOPY_PRESENTATION_STATE_STORAGE_UID,
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
        &source.study_instance_uid,
    );
    put_str(&mut obj, tags::STUDY_DATE, VR::DA, "20260101");
    put_str(&mut obj, tags::STUDY_TIME, VR::TM, "000000");
    put_str(&mut obj, tags::REFERRING_PHYSICIAN_NAME, VR::PN, "");
    put_str(&mut obj, tags::STUDY_ID, VR::SH, "DTS-GSPS");
    put_str(&mut obj, tags::ACCESSION_NUMBER, VR::SH, "");

    put_str(&mut obj, tags::MODALITY, VR::CS, "PR");
    put_str(
        &mut obj,
        tags::SERIES_INSTANCE_UID,
        VR::UI,
        &series_instance_uid,
    );
    put_str(&mut obj, tags::SERIES_NUMBER, VR::IS, "61");

    put_str(&mut obj, tags::MANUFACTURER, VR::LO, "dicom-test-suite");
    put_str(
        &mut obj,
        tags::MANUFACTURER_MODEL_NAME,
        VR::LO,
        recipe.recipe_id,
    );
    put_str(
        &mut obj,
        tags::DEVICE_SERIAL_NUMBER,
        VR::LO,
        "DTS-GSPS-0001",
    );
    put_str(
        &mut obj,
        tags::SOFTWARE_VERSIONS,
        VR::LO,
        crate::PACKAGE_VERSION,
    );

    put_str(&mut obj, tags::INSTANCE_NUMBER, VR::IS, "1");
    put_str(&mut obj, tags::CONTENT_DATE, VR::DA, "20260101");
    put_str(&mut obj, tags::CONTENT_TIME, VR::TM, "000000");
    put_str(&mut obj, TAG_PRESENTATION_CREATION_DATE, VR::DA, "20260101");
    put_str(&mut obj, TAG_PRESENTATION_CREATION_TIME, VR::TM, "000000");
    put_str(&mut obj, TAG_CONTENT_LABEL, VR::CS, recipe.content_label);
    put_str(
        &mut obj,
        TAG_CONTENT_DESCRIPTION,
        VR::LO,
        recipe.content_description,
    );
    put_str(&mut obj, TAG_CONTENT_CREATOR_NAME, VR::PN, "DTS^Generator");

    put_presentation_state_relationship(&mut obj, source, source_series_instance_uid);
    put_displayed_area_selection(&mut obj, recipe);
    put_softcopy_voi_lut(&mut obj, recipe);
    put_str(
        &mut obj,
        TAG_PRESENTATION_LUT_SHAPE,
        VR::CS,
        recipe.presentation_lut_shape,
    );

    let file_obj = obj
        .with_meta(
            FileMetaTableBuilder::new()
                .transfer_syntax(EXPLICIT_VR_LITTLE_ENDIAN.uid)
                .implementation_class_uid(&implementation_class_uid)
                .implementation_version_name(crate::IMPLEMENTATION_VERSION_NAME),
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

    let validated = validate_presentation_state_file(
        &path,
        &PresentationStateExpectations {
            sop_class_uid: GRAYSCALE_SOFTCOPY_PRESENTATION_STATE_STORAGE_UID,
            sop_instance_uid: &sop_instance_uid,
            transfer_syntax_uid: EXPLICIT_VR_LITTLE_ENDIAN.uid,
            implementation_class_uid: &implementation_class_uid,
            synthetic_data: "YES",
            modality: "PR",
            presentation_label: recipe.content_label,
            referenced_series_instance_uid: source_series_instance_uid,
            referenced_sop_class_uid: &source.sop_class_uid,
            referenced_sop_instance_uid: &source.sop_instance_uid,
            displayed_area_top_left: recipe.displayed_area_top_left.to_vec(),
            displayed_area_bottom_right: recipe.displayed_area_bottom_right.to_vec(),
            presentation_size_mode: recipe.presentation_size_mode,
            presentation_pixel_aspect_ratio: recipe.presentation_pixel_aspect_ratio.to_vec(),
            window_center: recipe.window_center,
            window_width: recipe.window_width,
            presentation_lut_shape: recipe.presentation_lut_shape,
        },
    )?;

    Ok(GeneratedFile {
        case_id: recipe.case_id.to_string(),
        manifest_entry: presentation_state_manifest_entry(
            case,
            recipe,
            source,
            &relative_path,
            &source.study_instance_uid,
            &series_instance_uid,
            &sop_instance_uid,
            &implementation_class_uid,
            &validated.bytes,
            validated.validation,
        ),
    })
}

fn write_real_world_value_mapping_case(
    run: &PreparedGenerationRun,
    case: &Value,
    recipe: RealWorldValueMappingRecipe,
    source: &GeneratedSourceObject,
    standards_lock_sha256: &str,
) -> Result<GeneratedFile, GenerateError> {
    let series_instance_uid = deterministic_real_world_value_mapping_uid(
        standards_lock_sha256,
        recipe,
        run.seed,
        UidRole::SeriesInstance,
    );
    let sop_instance_uid = deterministic_real_world_value_mapping_uid(
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
        REAL_WORLD_VALUE_MAPPING_STORAGE_UID,
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
        &source.study_instance_uid,
    );
    put_str(&mut obj, tags::STUDY_DATE, VR::DA, "20260101");
    put_str(&mut obj, tags::STUDY_TIME, VR::TM, "000000");
    put_str(&mut obj, tags::REFERRING_PHYSICIAN_NAME, VR::PN, "");
    put_str(&mut obj, tags::STUDY_ID, VR::SH, "DTS-RWVM");
    put_str(&mut obj, tags::ACCESSION_NUMBER, VR::SH, "");

    put_str(&mut obj, tags::MODALITY, VR::CS, "RWV");
    put_str(
        &mut obj,
        tags::SERIES_INSTANCE_UID,
        VR::UI,
        &series_instance_uid,
    );
    put_str(&mut obj, tags::SERIES_NUMBER, VR::IS, "62");

    put_str(&mut obj, tags::MANUFACTURER, VR::LO, "dicom-test-suite");
    put_str(
        &mut obj,
        tags::MANUFACTURER_MODEL_NAME,
        VR::LO,
        recipe.recipe_id,
    );
    put_str(
        &mut obj,
        tags::DEVICE_SERIAL_NUMBER,
        VR::LO,
        "DTS-RWVM-0001",
    );
    put_str(
        &mut obj,
        tags::SOFTWARE_VERSIONS,
        VR::LO,
        crate::PACKAGE_VERSION,
    );

    put_str(&mut obj, tags::INSTANCE_NUMBER, VR::IS, "1");
    put_str(&mut obj, tags::CONTENT_DATE, VR::DA, "20260101");
    put_str(&mut obj, tags::CONTENT_TIME, VR::TM, "000000");
    put_str(&mut obj, TAG_CONTENT_LABEL, VR::CS, recipe.content_label);
    put_str(
        &mut obj,
        TAG_CONTENT_DESCRIPTION,
        VR::LO,
        recipe.content_description,
    );

    put_real_world_value_mapping_sequence(&mut obj, recipe, source);
    put_common_instance_reference(&mut obj, source);

    let file_obj = obj
        .with_meta(
            FileMetaTableBuilder::new()
                .transfer_syntax(EXPLICIT_VR_LITTLE_ENDIAN.uid)
                .implementation_class_uid(&implementation_class_uid)
                .implementation_version_name(crate::IMPLEMENTATION_VERSION_NAME),
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

    let validated = validate_real_world_value_mapping_file(
        &path,
        &RealWorldValueMappingExpectations {
            sop_class_uid: REAL_WORLD_VALUE_MAPPING_STORAGE_UID,
            sop_instance_uid: &sop_instance_uid,
            transfer_syntax_uid: EXPLICIT_VR_LITTLE_ENDIAN.uid,
            implementation_class_uid: &implementation_class_uid,
            synthetic_data: "YES",
            modality: "RWV",
            content_label: recipe.content_label,
            referenced_series_instance_uid: source.series_instance_uid.as_deref().unwrap_or(""),
            referenced_sop_class_uid: &source.sop_class_uid,
            referenced_sop_instance_uid: &source.sop_instance_uid,
            referenced_frame_numbers: recipe.referenced_frame_numbers,
            lut_label: recipe.lut_label,
            first_value_mapped: recipe.first_value_mapped,
            last_value_mapped: recipe.last_value_mapped,
            intercept: recipe.intercept,
            slope: recipe.slope,
            unit_code_value: recipe.unit_code_value,
            unit_coding_scheme_designator: recipe.unit_coding_scheme_designator,
            unit_code_meaning: recipe.unit_code_meaning,
        },
    )?;

    Ok(GeneratedFile {
        case_id: recipe.case_id.to_string(),
        manifest_entry: real_world_value_mapping_manifest_entry(
            case,
            recipe,
            source,
            &relative_path,
            &source.study_instance_uid,
            &series_instance_uid,
            &sop_instance_uid,
            &implementation_class_uid,
            &validated.bytes,
            validated.validation,
        ),
    })
}

fn write_basic_text_sr_case(
    run: &PreparedGenerationRun,
    case: &Value,
    recipe: BasicTextSrRecipe,
    source: &GeneratedSourceObject,
    standards_lock_sha256: &str,
) -> Result<GeneratedFile, GenerateError> {
    let series_instance_uid = deterministic_basic_text_sr_uid(
        standards_lock_sha256,
        recipe,
        run.seed,
        UidRole::SeriesInstance,
    );
    let sop_instance_uid = deterministic_basic_text_sr_uid(
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
        BASIC_TEXT_SR_STORAGE_UID,
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
        &source.study_instance_uid,
    );
    put_str(&mut obj, tags::STUDY_DATE, VR::DA, "20260101");
    put_str(&mut obj, tags::STUDY_TIME, VR::TM, "000000");
    put_str(&mut obj, tags::REFERRING_PHYSICIAN_NAME, VR::PN, "");
    put_str(&mut obj, tags::STUDY_ID, VR::SH, "DTS-SR");
    put_str(&mut obj, tags::ACCESSION_NUMBER, VR::SH, "");

    put_str(&mut obj, tags::MODALITY, VR::CS, "SR");
    put_str(
        &mut obj,
        tags::SERIES_INSTANCE_UID,
        VR::UI,
        &series_instance_uid,
    );
    put_str(&mut obj, tags::SERIES_NUMBER, VR::IS, "63");

    put_str(&mut obj, tags::MANUFACTURER, VR::LO, "dicom-test-suite");
    put_str(
        &mut obj,
        tags::MANUFACTURER_MODEL_NAME,
        VR::LO,
        recipe.recipe_id,
    );
    put_str(&mut obj, tags::DEVICE_SERIAL_NUMBER, VR::LO, "DTS-SR-0001");
    put_str(
        &mut obj,
        tags::SOFTWARE_VERSIONS,
        VR::LO,
        crate::PACKAGE_VERSION,
    );

    put_str(&mut obj, tags::INSTANCE_NUMBER, VR::IS, "1");
    put_str(&mut obj, tags::CONTENT_DATE, VR::DA, "20260101");
    put_str(&mut obj, tags::CONTENT_TIME, VR::TM, "000000");
    put_str(
        &mut obj,
        tags::COMPLETION_FLAG,
        VR::CS,
        recipe.completion_flag,
    );
    put_str(
        &mut obj,
        tags::VERIFICATION_FLAG,
        VR::CS,
        recipe.verification_flag,
    );

    put_current_requested_procedure_evidence(&mut obj, source);
    put_basic_text_sr_content_tree(&mut obj, recipe);

    let file_obj = obj
        .with_meta(
            FileMetaTableBuilder::new()
                .transfer_syntax(EXPLICIT_VR_LITTLE_ENDIAN.uid)
                .implementation_class_uid(&implementation_class_uid)
                .implementation_version_name(crate::IMPLEMENTATION_VERSION_NAME),
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

    let validated = validate_basic_text_sr_file(
        &path,
        &BasicTextSrExpectations {
            sop_class_uid: BASIC_TEXT_SR_STORAGE_UID,
            sop_instance_uid: &sop_instance_uid,
            transfer_syntax_uid: EXPLICIT_VR_LITTLE_ENDIAN.uid,
            implementation_class_uid: &implementation_class_uid,
            synthetic_data: "YES",
            modality: "SR",
            completion_flag: recipe.completion_flag,
            verification_flag: recipe.verification_flag,
            referenced_study_instance_uid: &source.study_instance_uid,
            referenced_series_instance_uid: source.series_instance_uid.as_deref().unwrap_or(""),
            referenced_sop_class_uid: &source.sop_class_uid,
            referenced_sop_instance_uid: &source.sop_instance_uid,
            root_value_type: recipe.root_value_type,
            root_continuity_of_content: recipe.root_continuity_of_content,
            title_code_value: recipe.title_code_value,
            title_coding_scheme_designator: recipe.title_coding_scheme_designator,
            title_code_meaning: recipe.title_code_meaning,
            observation_relationship_type: recipe.observation_relationship_type,
            observation_value_type: recipe.observation_value_type,
            observation_code_value: recipe.observation_code_value,
            observation_coding_scheme_designator: recipe.observation_coding_scheme_designator,
            observation_code_meaning: recipe.observation_code_meaning,
            observation_text: recipe.observation_text,
        },
    )?;

    Ok(GeneratedFile {
        case_id: recipe.case_id.to_string(),
        manifest_entry: basic_text_sr_manifest_entry(
            case,
            recipe,
            source,
            &relative_path,
            &source.study_instance_uid,
            &series_instance_uid,
            &sop_instance_uid,
            &implementation_class_uid,
            &validated.bytes,
            validated.validation,
        ),
    })
}

fn write_comprehensive_sr_case(
    run: &PreparedGenerationRun,
    case: &Value,
    recipe: ComprehensiveSrRecipe,
    source: &GeneratedSourceObject,
    standards_lock_sha256: &str,
) -> Result<GeneratedFile, GenerateError> {
    let series_instance_uid = deterministic_comprehensive_sr_uid(
        standards_lock_sha256,
        recipe,
        run.seed,
        UidRole::SeriesInstance,
    );
    let sop_instance_uid = deterministic_comprehensive_sr_uid(
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
        COMPREHENSIVE_SR_STORAGE_UID,
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
        &source.study_instance_uid,
    );
    put_str(&mut obj, tags::STUDY_DATE, VR::DA, "20260101");
    put_str(&mut obj, tags::STUDY_TIME, VR::TM, "000000");
    put_str(&mut obj, tags::REFERRING_PHYSICIAN_NAME, VR::PN, "");
    put_str(&mut obj, tags::STUDY_ID, VR::SH, "DTS-SR");
    put_str(&mut obj, tags::ACCESSION_NUMBER, VR::SH, "");

    put_str(&mut obj, tags::MODALITY, VR::CS, "SR");
    put_str(
        &mut obj,
        tags::SERIES_INSTANCE_UID,
        VR::UI,
        &series_instance_uid,
    );
    put_str(&mut obj, tags::SERIES_NUMBER, VR::IS, "64");

    put_str(&mut obj, tags::MANUFACTURER, VR::LO, "dicom-test-suite");
    put_str(
        &mut obj,
        tags::MANUFACTURER_MODEL_NAME,
        VR::LO,
        recipe.recipe_id,
    );
    put_str(&mut obj, tags::DEVICE_SERIAL_NUMBER, VR::LO, "DTS-SR-0002");
    put_str(
        &mut obj,
        tags::SOFTWARE_VERSIONS,
        VR::LO,
        crate::PACKAGE_VERSION,
    );

    put_str(&mut obj, tags::INSTANCE_NUMBER, VR::IS, "1");
    put_str(&mut obj, tags::CONTENT_DATE, VR::DA, "20260101");
    put_str(&mut obj, tags::CONTENT_TIME, VR::TM, "000000");
    put_str(
        &mut obj,
        tags::COMPLETION_FLAG,
        VR::CS,
        recipe.completion_flag,
    );
    put_str(
        &mut obj,
        tags::VERIFICATION_FLAG,
        VR::CS,
        recipe.verification_flag,
    );

    put_current_requested_procedure_evidence(&mut obj, source);
    put_comprehensive_sr_content_tree(&mut obj, recipe, source);

    let file_obj = obj
        .with_meta(
            FileMetaTableBuilder::new()
                .transfer_syntax(EXPLICIT_VR_LITTLE_ENDIAN.uid)
                .implementation_class_uid(&implementation_class_uid)
                .implementation_version_name(crate::IMPLEMENTATION_VERSION_NAME),
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

    let validated = validate_comprehensive_sr_file(
        &path,
        &crate::validation::ComprehensiveSrExpectations {
            sop_class_uid: COMPREHENSIVE_SR_STORAGE_UID,
            sop_instance_uid: &sop_instance_uid,
            transfer_syntax_uid: EXPLICIT_VR_LITTLE_ENDIAN.uid,
            implementation_class_uid: &implementation_class_uid,
            synthetic_data: "YES",
            modality: "SR",
            completion_flag: recipe.completion_flag,
            verification_flag: recipe.verification_flag,
            referenced_study_instance_uid: &source.study_instance_uid,
            referenced_series_instance_uid: source.series_instance_uid.as_deref().unwrap_or(""),
            referenced_sop_class_uid: &source.sop_class_uid,
            referenced_sop_instance_uid: &source.sop_instance_uid,
            root_value_type: recipe.root_value_type,
            root_continuity_of_content: recipe.root_continuity_of_content,
            title_code_value: recipe.title_code_value,
            title_coding_scheme_designator: recipe.title_coding_scheme_designator,
            title_code_meaning: recipe.title_code_meaning,
            measurement_relationship_type: recipe.measurement_relationship_type,
            measurement_value_type: recipe.measurement_value_type,
            measurement_code_value: recipe.measurement_code_value,
            measurement_coding_scheme_designator: recipe.measurement_coding_scheme_designator,
            measurement_code_meaning: recipe.measurement_code_meaning,
            numeric_value: recipe.numeric_value,
            unit_code_value: recipe.unit_code_value,
            unit_coding_scheme_designator: recipe.unit_coding_scheme_designator,
            unit_code_meaning: recipe.unit_code_meaning,
            image_relationship_type: recipe.image_relationship_type,
            image_value_type: recipe.image_value_type,
            image_code_value: recipe.image_code_value,
            image_coding_scheme_designator: recipe.image_coding_scheme_designator,
            image_code_meaning: recipe.image_code_meaning,
            referenced_frame_numbers: recipe.referenced_frame_numbers,
        },
    )?;

    Ok(GeneratedFile {
        case_id: recipe.case_id.to_string(),
        manifest_entry: comprehensive_sr_manifest_entry(
            case,
            recipe,
            source,
            &relative_path,
            &source.study_instance_uid,
            &series_instance_uid,
            &sop_instance_uid,
            &implementation_class_uid,
            &validated.bytes,
            validated.validation,
        ),
    })
}

fn write_key_object_selection_case(
    run: &PreparedGenerationRun,
    case: &Value,
    recipe: KeyObjectSelectionRecipe,
    image_source: &GeneratedSourceObject,
    seg_source: &GeneratedSourceObject,
    standards_lock_sha256: &str,
) -> Result<GeneratedFile, GenerateError> {
    let series_instance_uid = deterministic_key_object_selection_uid(
        standards_lock_sha256,
        recipe,
        run.seed,
        UidRole::SeriesInstance,
    );
    let sop_instance_uid = deterministic_key_object_selection_uid(
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
        KEY_OBJECT_SELECTION_DOCUMENT_STORAGE_UID,
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
        &image_source.study_instance_uid,
    );
    put_str(&mut obj, tags::STUDY_DATE, VR::DA, "20260101");
    put_str(&mut obj, tags::STUDY_TIME, VR::TM, "000000");
    put_str(&mut obj, tags::REFERRING_PHYSICIAN_NAME, VR::PN, "");
    put_str(&mut obj, tags::STUDY_ID, VR::SH, "DTS-KOS");
    put_str(&mut obj, tags::ACCESSION_NUMBER, VR::SH, "");

    put_str(&mut obj, tags::MODALITY, VR::CS, "KO");
    put_str(
        &mut obj,
        tags::SERIES_INSTANCE_UID,
        VR::UI,
        &series_instance_uid,
    );
    put_str(&mut obj, tags::SERIES_NUMBER, VR::IS, "65");

    put_str(&mut obj, tags::MANUFACTURER, VR::LO, "dicom-test-suite");
    put_str(
        &mut obj,
        tags::MANUFACTURER_MODEL_NAME,
        VR::LO,
        recipe.recipe_id,
    );
    put_str(&mut obj, tags::DEVICE_SERIAL_NUMBER, VR::LO, "DTS-KOS-0001");
    put_str(
        &mut obj,
        tags::SOFTWARE_VERSIONS,
        VR::LO,
        crate::PACKAGE_VERSION,
    );

    put_str(&mut obj, tags::INSTANCE_NUMBER, VR::IS, "1");
    put_str(&mut obj, tags::CONTENT_DATE, VR::DA, "20260101");
    put_str(&mut obj, tags::CONTENT_TIME, VR::TM, "000000");
    put_str(
        &mut obj,
        tags::COMPLETION_FLAG,
        VR::CS,
        recipe.completion_flag,
    );
    put_str(
        &mut obj,
        tags::VERIFICATION_FLAG,
        VR::CS,
        recipe.verification_flag,
    );

    put_current_requested_procedure_evidence_many(&mut obj, &[image_source, seg_source]);
    put_key_object_selection_content_tree(&mut obj, recipe, image_source, seg_source);

    let file_obj = obj
        .with_meta(
            FileMetaTableBuilder::new()
                .transfer_syntax(EXPLICIT_VR_LITTLE_ENDIAN.uid)
                .implementation_class_uid(&implementation_class_uid)
                .implementation_version_name(crate::IMPLEMENTATION_VERSION_NAME),
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

    let key_objects = [
        crate::validation::KeyObjectReferenceExpectations {
            referenced_series_instance_uid: image_source
                .series_instance_uid
                .as_deref()
                .unwrap_or(""),
            referenced_sop_class_uid: &image_source.sop_class_uid,
            referenced_sop_instance_uid: &image_source.sop_instance_uid,
            referenced_frame_numbers: Some(recipe.image_referenced_frame_numbers),
        },
        crate::validation::KeyObjectReferenceExpectations {
            referenced_series_instance_uid: seg_source.series_instance_uid.as_deref().unwrap_or(""),
            referenced_sop_class_uid: &seg_source.sop_class_uid,
            referenced_sop_instance_uid: &seg_source.sop_instance_uid,
            referenced_frame_numbers: None,
        },
    ];
    let validated = validate_key_object_selection_file(
        &path,
        &crate::validation::KeyObjectSelectionExpectations {
            sop_class_uid: KEY_OBJECT_SELECTION_DOCUMENT_STORAGE_UID,
            sop_instance_uid: &sop_instance_uid,
            transfer_syntax_uid: EXPLICIT_VR_LITTLE_ENDIAN.uid,
            implementation_class_uid: &implementation_class_uid,
            synthetic_data: "YES",
            modality: "KO",
            completion_flag: recipe.completion_flag,
            verification_flag: recipe.verification_flag,
            referenced_study_instance_uid: &image_source.study_instance_uid,
            root_value_type: recipe.root_value_type,
            root_continuity_of_content: recipe.root_continuity_of_content,
            title_code_value: recipe.title_code_value,
            title_coding_scheme_designator: recipe.title_coding_scheme_designator,
            title_code_meaning: recipe.title_code_meaning,
            mapping_resource: recipe.mapping_resource,
            template_identifier: recipe.template_identifier,
            relationship_type: recipe.relationship_type,
            image_value_type: recipe.image_value_type,
            key_objects: &key_objects,
        },
    )?;

    Ok(GeneratedFile {
        case_id: recipe.case_id.to_string(),
        manifest_entry: key_object_selection_manifest_entry(
            case,
            recipe,
            image_source,
            seg_source,
            &relative_path,
            &image_source.study_instance_uid,
            &series_instance_uid,
            &sop_instance_uid,
            &implementation_class_uid,
            &validated.bytes,
            validated.validation,
        ),
    })
}

fn write_rt_structure_set_case(
    run: &PreparedGenerationRun,
    case: &Value,
    recipe: RtStructureSetRecipe,
    source: &GeneratedSourceObject,
    standards_lock_sha256: &str,
) -> Result<GeneratedFile, GenerateError> {
    let series_instance_uid = deterministic_rt_structure_set_uid(
        standards_lock_sha256,
        recipe,
        run.seed,
        UidRole::SeriesInstance,
    );
    let sop_instance_uid = deterministic_rt_structure_set_uid(
        standards_lock_sha256,
        recipe,
        run.seed,
        UidRole::SopInstance,
    );
    let implementation_class_uid = deterministic_implementation_uid(standards_lock_sha256);
    let frame_of_reference_uid =
        source
            .frame_of_reference_uid
            .as_deref()
            .ok_or_else(|| GenerateError::MetadataShape {
                path: PathBuf::from(recipe.case_id),
                message: "RT Structure Set source object must expose a Frame of Reference UID",
            })?;

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
        RT_STRUCTURE_SET_STORAGE_UID,
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
        &source.study_instance_uid,
    );
    put_str(&mut obj, tags::STUDY_DATE, VR::DA, "20260101");
    put_str(&mut obj, tags::STUDY_TIME, VR::TM, "000000");
    put_str(&mut obj, tags::REFERRING_PHYSICIAN_NAME, VR::PN, "");
    put_str(&mut obj, tags::STUDY_ID, VR::SH, "DTS-RTSTRUCT");
    put_str(&mut obj, tags::ACCESSION_NUMBER, VR::SH, "");

    put_str(&mut obj, tags::MODALITY, VR::CS, "RTSTRUCT");
    put_str(
        &mut obj,
        tags::SERIES_INSTANCE_UID,
        VR::UI,
        &series_instance_uid,
    );
    put_str(&mut obj, tags::SERIES_NUMBER, VR::IS, "70");
    put_str(&mut obj, tags::OPERATORS_NAME, VR::PN, "");

    put_str(
        &mut obj,
        tags::FRAME_OF_REFERENCE_UID,
        VR::UI,
        frame_of_reference_uid,
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
        tags::DEVICE_SERIAL_NUMBER,
        VR::LO,
        "DTS-RTSTRUCT-0001",
    );
    put_str(
        &mut obj,
        tags::SOFTWARE_VERSIONS,
        VR::LO,
        crate::PACKAGE_VERSION,
    );

    put_str(&mut obj, tags::INSTANCE_NUMBER, VR::IS, "1");
    put_str(
        &mut obj,
        tags::STRUCTURE_SET_LABEL,
        VR::SH,
        recipe.structure_set_label,
    );
    put_str(
        &mut obj,
        tags::STRUCTURE_SET_NAME,
        VR::LO,
        recipe.structure_set_name,
    );
    put_str(&mut obj, tags::STRUCTURE_SET_DATE, VR::DA, "20260101");
    put_str(&mut obj, tags::STRUCTURE_SET_TIME, VR::TM, "000000");

    put_rt_structure_set_references(&mut obj, recipe, source, frame_of_reference_uid);
    put_rt_structure_set_roi_sequence(&mut obj, recipe, frame_of_reference_uid);
    put_rt_roi_contour_sequence(&mut obj, recipe, source);
    put_rt_roi_observations_sequence(&mut obj, recipe);
    put_common_instance_reference(&mut obj, source);

    let file_obj = obj
        .with_meta(
            FileMetaTableBuilder::new()
                .transfer_syntax(EXPLICIT_VR_LITTLE_ENDIAN.uid)
                .implementation_class_uid(&implementation_class_uid)
                .implementation_version_name(crate::IMPLEMENTATION_VERSION_NAME),
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

    let validated = validate_rt_structure_set_file(
        &path,
        &RtStructureSetExpectations {
            sop_class_uid: RT_STRUCTURE_SET_STORAGE_UID,
            sop_instance_uid: &sop_instance_uid,
            transfer_syntax_uid: EXPLICIT_VR_LITTLE_ENDIAN.uid,
            implementation_class_uid: &implementation_class_uid,
            synthetic_data: "YES",
            modality: "RTSTRUCT",
            frame_of_reference_uid,
            structure_set_label: recipe.structure_set_label,
            structure_set_roi_items: 1,
            roi_number: recipe.roi_number,
            roi_name: recipe.roi_name,
            roi_generation_algorithm: recipe.roi_generation_algorithm,
            roi_contour_items: 1,
            contour_items: 1,
            contour_geometric_type: recipe.contour_geometric_type,
            contour_points: recipe.contour_points,
            contour_data: recipe.contour_data,
            rt_roi_observation_items: 1,
            roi_interpreted_type: recipe.roi_interpreted_type,
            roi_interpreter: recipe.roi_interpreter,
            referenced_series_instance_uid: source.series_instance_uid.as_deref().unwrap_or(""),
            referenced_sop_class_uid: &source.sop_class_uid,
            referenced_sop_instance_uid: &source.sop_instance_uid,
        },
    )?;

    Ok(GeneratedFile {
        case_id: recipe.case_id.to_string(),
        manifest_entry: rt_structure_set_manifest_entry(
            case,
            recipe,
            source,
            &relative_path,
            &source.study_instance_uid,
            &series_instance_uid,
            &sop_instance_uid,
            frame_of_reference_uid,
            &implementation_class_uid,
            &validated.bytes,
            validated.validation,
        ),
    })
}

fn write_rt_dose_case(
    run: &PreparedGenerationRun,
    case: &Value,
    recipe: RtDoseRecipe,
    image_source: &GeneratedSourceObject,
    structure_set_source: &GeneratedSourceObject,
    standards_lock_sha256: &str,
) -> Result<GeneratedFile, GenerateError> {
    let series_instance_uid = deterministic_rt_dose_uid(
        standards_lock_sha256,
        recipe,
        run.seed,
        UidRole::SeriesInstance,
    );
    let sop_instance_uid = deterministic_rt_dose_uid(
        standards_lock_sha256,
        recipe,
        run.seed,
        UidRole::SopInstance,
    );
    let implementation_class_uid = deterministic_implementation_uid(standards_lock_sha256);
    let frame_of_reference_uid =
        image_source
            .frame_of_reference_uid
            .as_deref()
            .ok_or_else(|| GenerateError::MetadataShape {
                path: PathBuf::from(recipe.case_id),
                message: "RT Dose image source object must expose a Frame of Reference UID",
            })?;

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
    put_str(&mut obj, tags::SOP_CLASS_UID, VR::UI, RT_DOSE_STORAGE_UID);
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
        &image_source.study_instance_uid,
    );
    put_str(&mut obj, tags::STUDY_DATE, VR::DA, "20260101");
    put_str(&mut obj, tags::STUDY_TIME, VR::TM, "000000");
    put_str(&mut obj, tags::REFERRING_PHYSICIAN_NAME, VR::PN, "");
    put_str(&mut obj, tags::STUDY_ID, VR::SH, "DTS-RTDOSE");
    put_str(&mut obj, tags::ACCESSION_NUMBER, VR::SH, "");

    put_str(&mut obj, tags::MODALITY, VR::CS, "RTDOSE");
    put_str(
        &mut obj,
        tags::SERIES_INSTANCE_UID,
        VR::UI,
        &series_instance_uid,
    );
    put_str(&mut obj, tags::SERIES_NUMBER, VR::IS, "71");
    put_str(&mut obj, tags::OPERATORS_NAME, VR::PN, "");

    put_str(
        &mut obj,
        tags::FRAME_OF_REFERENCE_UID,
        VR::UI,
        frame_of_reference_uid,
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
        tags::DEVICE_SERIAL_NUMBER,
        VR::LO,
        "DTS-RTDOSE-0001",
    );
    put_str(
        &mut obj,
        tags::SOFTWARE_VERSIONS,
        VR::LO,
        crate::PACKAGE_VERSION,
    );

    put_str(&mut obj, TAG_IMAGE_TYPE, VR::CS, "DERIVED\\PRIMARY\\DOSE");
    put_str(&mut obj, tags::INSTANCE_NUMBER, VR::IS, "1");
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
    put_str(
        &mut obj,
        tags::NUMBER_OF_FRAMES,
        VR::IS,
        &recipe.frames.to_string(),
    );
    obj.put(DataElement::new(
        tags::FRAME_INCREMENT_POINTER,
        VR::AT,
        PrimitiveValue::Tags(vec![recipe.frame_increment_pointer].into()),
    ));
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
    put_u16(&mut obj, tags::BITS_ALLOCATED, VR::US, 16);
    put_u16(&mut obj, tags::BITS_STORED, VR::US, 16);
    put_u16(&mut obj, tags::HIGH_BIT, VR::US, 15);
    put_u16(&mut obj, tags::PIXEL_REPRESENTATION, VR::US, 0);
    put_str(&mut obj, tags::DOSE_UNITS, VR::CS, recipe.dose_units);
    put_str(&mut obj, tags::DOSE_TYPE, VR::CS, recipe.dose_type);
    put_str(
        &mut obj,
        tags::DOSE_SUMMATION_TYPE,
        VR::CS,
        recipe.dose_summation_type,
    );
    put_str(
        &mut obj,
        tags::GRID_FRAME_OFFSET_VECTOR,
        VR::DS,
        recipe.grid_frame_offset_vector,
    );
    put_str(
        &mut obj,
        tags::DOSE_GRID_SCALING,
        VR::DS,
        recipe.dose_grid_scaling,
    );
    put_rt_dose_references(&mut obj, image_source, structure_set_source);
    put_common_instance_reference(&mut obj, image_source);
    obj.put(DataElement::new(
        tags::PIXEL_DATA,
        VR::OW,
        PrimitiveValue::from(recipe.pixel_bytes),
    ));

    let file_obj = obj
        .with_meta(
            FileMetaTableBuilder::new()
                .transfer_syntax(EXPLICIT_VR_LITTLE_ENDIAN.uid)
                .implementation_class_uid(&implementation_class_uid)
                .implementation_version_name(crate::IMPLEMENTATION_VERSION_NAME),
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

    let validated = validate_rt_dose_file(
        &path,
        &RtDoseExpectations {
            sop_class_uid: RT_DOSE_STORAGE_UID,
            sop_instance_uid: &sop_instance_uid,
            transfer_syntax_uid: EXPLICIT_VR_LITTLE_ENDIAN.uid,
            implementation_class_uid: &implementation_class_uid,
            synthetic_data: "YES",
            modality: "RTDOSE",
            frame_of_reference_uid,
            rows: recipe.rows,
            columns: recipe.columns,
            frames: recipe.frames,
            pixel_bytes_len: recipe.pixel_bytes.len(),
            pixel_vr: VR::OW,
            pixel_spacing: recipe.pixel_spacing,
            image_orientation_patient: recipe.image_orientation_patient,
            image_position_patient: recipe.image_position_patient,
            slice_thickness: recipe.slice_thickness,
            frame_increment_pointer: recipe.frame_increment_pointer,
            grid_frame_offset_vector: recipe.grid_frame_offset_vector,
            dose_units: recipe.dose_units,
            dose_type: recipe.dose_type,
            dose_summation_type: recipe.dose_summation_type,
            dose_grid_scaling: recipe.dose_grid_scaling,
            referenced_image_sop_class_uid: &image_source.sop_class_uid,
            referenced_image_sop_instance_uid: &image_source.sop_instance_uid,
            referenced_structure_set_sop_class_uid: &structure_set_source.sop_class_uid,
            referenced_structure_set_sop_instance_uid: &structure_set_source.sop_instance_uid,
        },
    )?;

    Ok(GeneratedFile {
        case_id: recipe.case_id.to_string(),
        manifest_entry: rt_dose_manifest_entry(
            case,
            recipe,
            image_source,
            structure_set_source,
            &relative_path,
            &image_source.study_instance_uid,
            &series_instance_uid,
            &sop_instance_uid,
            frame_of_reference_uid,
            &implementation_class_uid,
            &validated.bytes,
            validated.validation,
        ),
    })
}

fn write_encapsulated_pdf_case(
    run: &PreparedGenerationRun,
    case: &Value,
    recipe: EncapsulatedPdfRecipe,
    standards_lock_sha256: &str,
) -> Result<GeneratedFile, GenerateError> {
    let study_instance_uid = deterministic_encapsulated_pdf_uid(
        standards_lock_sha256,
        recipe,
        run.seed,
        UidRole::StudyInstance,
    );
    let series_instance_uid = deterministic_encapsulated_pdf_uid(
        standards_lock_sha256,
        recipe,
        run.seed,
        UidRole::SeriesInstance,
    );
    let sop_instance_uid = deterministic_encapsulated_pdf_uid(
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
        ENCAPSULATED_PDF_STORAGE_UID,
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
    put_str(&mut obj, tags::STUDY_ID, VR::SH, "DTS-PDF");
    put_str(&mut obj, tags::ACCESSION_NUMBER, VR::SH, "");

    put_str(&mut obj, tags::MODALITY, VR::CS, "DOC");
    put_str(
        &mut obj,
        tags::SERIES_INSTANCE_UID,
        VR::UI,
        &series_instance_uid,
    );
    put_str(&mut obj, tags::SERIES_NUMBER, VR::IS, "81");
    put_str(
        &mut obj,
        tags::SERIES_DESCRIPTION,
        VR::LO,
        "DTS minimal synthetic PDF",
    );

    put_str(&mut obj, tags::MANUFACTURER, VR::LO, "dicom-test-suite");
    put_str(
        &mut obj,
        tags::MANUFACTURER_MODEL_NAME,
        VR::LO,
        recipe.recipe_id,
    );
    put_str(&mut obj, tags::DEVICE_SERIAL_NUMBER, VR::LO, "DTS-PDF-0001");
    put_str(
        &mut obj,
        tags::SOFTWARE_VERSIONS,
        VR::LO,
        crate::PACKAGE_VERSION,
    );
    put_str(&mut obj, tags::CONVERSION_TYPE, VR::CS, "SYN");

    put_str(&mut obj, tags::INSTANCE_NUMBER, VR::IS, "1");
    put_str(&mut obj, tags::CONTENT_DATE, VR::DA, "20260101");
    put_str(&mut obj, tags::CONTENT_TIME, VR::TM, "000000");
    put_str(
        &mut obj,
        tags::ACQUISITION_DATE_TIME,
        VR::DT,
        "20260101000000",
    );
    put_str(
        &mut obj,
        tags::BURNED_IN_ANNOTATION,
        VR::CS,
        recipe.burned_in_annotation,
    );
    put_str(
        &mut obj,
        tags::RECOGNIZABLE_VISUAL_FEATURES,
        VR::CS,
        recipe.recognizable_visual_features,
    );
    put_str(
        &mut obj,
        tags::DOCUMENT_TITLE,
        VR::ST,
        recipe.document_title,
    );
    put_empty_sequence(&mut obj, tags::CONCEPT_NAME_CODE_SEQUENCE);
    put_str(
        &mut obj,
        tags::MIME_TYPE_OF_ENCAPSULATED_DOCUMENT,
        VR::LO,
        recipe.mime_type,
    );
    put_u32(
        &mut obj,
        tags::ENCAPSULATED_DOCUMENT_LENGTH,
        VR::UL,
        recipe.document_bytes.len() as u32,
    );
    obj.put(DataElement::new(
        tags::ENCAPSULATED_DOCUMENT,
        VR::OB,
        PrimitiveValue::from(recipe.document_bytes),
    ));

    let file_obj = obj
        .with_meta(
            FileMetaTableBuilder::new()
                .transfer_syntax(EXPLICIT_VR_LITTLE_ENDIAN.uid)
                .implementation_class_uid(&implementation_class_uid)
                .implementation_version_name(crate::IMPLEMENTATION_VERSION_NAME),
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

    let validated = validate_encapsulated_pdf_file(
        &path,
        &EncapsulatedPdfExpectations {
            sop_class_uid: ENCAPSULATED_PDF_STORAGE_UID,
            sop_instance_uid: &sop_instance_uid,
            transfer_syntax_uid: EXPLICIT_VR_LITTLE_ENDIAN.uid,
            implementation_class_uid: &implementation_class_uid,
            synthetic_data: "YES",
            modality: "DOC",
            conversion_type: "SYN",
            instance_number: "1",
            content_date: "20260101",
            content_time: "000000",
            acquisition_datetime: "20260101000000",
            burned_in_annotation: recipe.burned_in_annotation,
            recognizable_visual_features: recipe.recognizable_visual_features,
            document_title: recipe.document_title,
            mime_type: recipe.mime_type,
            document_bytes: recipe.document_bytes,
        },
    )?;

    Ok(GeneratedFile {
        case_id: recipe.case_id.to_string(),
        manifest_entry: encapsulated_pdf_manifest_entry(
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

fn write_enhanced_ct_concatenation_case(
    run: &PreparedGenerationRun,
    case: &Value,
    recipe: EnhancedCtConcatenationRecipe,
    standards_lock_sha256: &str,
) -> Result<Vec<GeneratedFile>, GenerateError> {
    let base = recipe.base;
    let study_instance_uid = deterministic_enhanced_ct_uid(
        standards_lock_sha256,
        base,
        run.seed,
        UidRole::StudyInstance,
    );
    let series_instance_uid = deterministic_enhanced_ct_uid(
        standards_lock_sha256,
        base,
        run.seed,
        UidRole::SeriesInstance,
    );
    let frame_of_reference_uid = deterministic_enhanced_ct_uid(
        standards_lock_sha256,
        base,
        run.seed,
        UidRole::FrameOfReference,
    );
    let dimension_organization_uid = deterministic_enhanced_ct_uid(
        standards_lock_sha256,
        base,
        run.seed,
        UidRole::DimensionOrganization,
    );
    let irradiation_event_uid = deterministic_enhanced_ct_uid(
        standards_lock_sha256,
        base,
        run.seed,
        UidRole::IrradiationEvent,
    );
    let concatenation_uid = deterministic_enhanced_ct_uid(
        standards_lock_sha256,
        base,
        run.seed,
        UidRole::Concatenation,
    );
    let sop_instance_uid_of_concatenation_source = deterministic_enhanced_ct_uid(
        standards_lock_sha256,
        base,
        run.seed,
        UidRole::ConcatenationSource,
    );
    let implementation_class_uid = deterministic_implementation_uid(standards_lock_sha256);

    let mut generated_files = Vec::with_capacity(recipe.parts.len());
    for part in recipe.parts {
        let file_index = u32::from(part.in_concatenation_number - 1);
        let sop_instance_uid = deterministic_enhanced_ct_indexed_uid(
            standards_lock_sha256,
            base,
            run.seed,
            UidRole::SopInstance,
            file_index,
        );
        let relative_path = format!("{}/{}", base.case_id, part.file_name);
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
            uids::ENHANCED_CT_IMAGE_STORAGE,
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
        put_str(&mut obj, tags::STUDY_ID, VR::SH, "DTS-ECT");
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
            base.recipe_id,
        );
        put_str(&mut obj, tags::DEVICE_SERIAL_NUMBER, VR::LO, "DTS-ECT-0001");
        put_str(
            &mut obj,
            tags::SOFTWARE_VERSIONS,
            VR::LO,
            crate::PACKAGE_VERSION,
        );

        let part_frames = part.image_position_patient.len() as u16;
        put_str(&mut obj, tags::IMAGE_TYPE, VR::CS, base.frame_type);
        put_str(&mut obj, tags::INSTANCE_NUMBER, VR::IS, "1");
        put_str(&mut obj, tags::CONTENT_DATE, VR::DA, "20260101");
        put_str(&mut obj, tags::CONTENT_TIME, VR::TM, "000000");
        put_str(
            &mut obj,
            tags::CONCATENATION_UID,
            VR::UI,
            &concatenation_uid,
        );
        put_u16(
            &mut obj,
            tags::IN_CONCATENATION_NUMBER,
            VR::US,
            part.in_concatenation_number,
        );
        put_u16(
            &mut obj,
            tags::IN_CONCATENATION_TOTAL_NUMBER,
            VR::US,
            recipe.parts.len() as u16,
        );
        put_u32(
            &mut obj,
            tags::CONCATENATION_FRAME_OFFSET_NUMBER,
            VR::UL,
            part.concatenation_frame_offset_number,
        );
        put_str(
            &mut obj,
            tags::SOP_INSTANCE_UID_OF_CONCATENATION_SOURCE,
            VR::UI,
            &sop_instance_uid_of_concatenation_source,
        );
        put_empty_sequence(&mut obj, tags::ACQUISITION_CONTEXT_SEQUENCE);
        put_str(
            &mut obj,
            tags::PIXEL_PRESENTATION,
            VR::CS,
            base.pixel_presentation,
        );
        put_str(
            &mut obj,
            tags::VOLUMETRIC_PROPERTIES,
            VR::CS,
            base.volumetric_properties,
        );
        put_str(
            &mut obj,
            tags::VOLUME_BASED_CALCULATION_TECHNIQUE,
            VR::CS,
            base.volume_based_calculation_technique,
        );

        put_u16(&mut obj, tags::SAMPLES_PER_PIXEL, VR::US, 1);
        put_str(
            &mut obj,
            tags::PHOTOMETRIC_INTERPRETATION,
            VR::CS,
            "MONOCHROME2",
        );
        put_u16(&mut obj, tags::ROWS, VR::US, base.rows);
        put_u16(&mut obj, tags::COLUMNS, VR::US, base.columns);
        put_u16(&mut obj, tags::BITS_ALLOCATED, VR::US, 16);
        put_u16(&mut obj, tags::BITS_STORED, VR::US, 16);
        put_u16(&mut obj, tags::HIGH_BIT, VR::US, 15);
        put_u16(&mut obj, tags::PIXEL_REPRESENTATION, VR::US, 0);
        put_str(
            &mut obj,
            tags::NUMBER_OF_FRAMES,
            VR::IS,
            &part_frames.to_string(),
        );

        put_enhanced_ct_dimension_sequences(&mut obj, &dimension_organization_uid);
        put_enhanced_ct_functional_groups(
            &mut obj,
            base,
            &irradiation_event_uid,
            part.image_position_patient,
            part.dimension_index_values,
        );

        obj.put(DataElement::new(
            tags::PIXEL_DATA,
            VR::OW,
            PrimitiveValue::from(part.pixel_bytes),
        ));

        let file_obj = obj
            .with_meta(
                FileMetaTableBuilder::new()
                    .transfer_syntax(EXPLICIT_VR_LITTLE_ENDIAN.uid)
                    .implementation_class_uid(&implementation_class_uid)
                    .implementation_version_name(crate::IMPLEMENTATION_VERSION_NAME),
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

        let concatenation = EnhancedCtConcatenationManifest {
            concatenation_uid: &concatenation_uid,
            in_concatenation_number: part.in_concatenation_number,
            in_concatenation_total_number: recipe.parts.len() as u16,
            concatenation_frame_offset_number: part.concatenation_frame_offset_number,
            sop_instance_uid_of_concatenation_source: &sop_instance_uid_of_concatenation_source,
        };
        let validated = validate_part10_file(
            &path,
            &Part10Expectations {
                sop_class_uid: uids::ENHANCED_CT_IMAGE_STORAGE,
                sop_instance_uid: &sop_instance_uid,
                transfer_syntax_uid: EXPLICIT_VR_LITTLE_ENDIAN.uid,
                implementation_class_uid: &implementation_class_uid,
                synthetic_data: "YES",
                rows: base.rows,
                columns: base.columns,
                frames: part_frames,
                samples_per_pixel: 1,
                photometric_interpretation: "MONOCHROME2",
                bits_allocated: 16,
                bits_stored: 16,
                high_bit: 15,
                pixel_representation: 0,
                planar_configuration: None,
                pixel_data_vr: VR::OW,
                pixel_data_length_formula: PixelDataLengthFormula::ContiguousSamples,
                decoded_frame_hashes: &[],
                palette: None,
                padding: None,
                ct_image: None,
                enhanced_ct_image: Some(EnhancedCtImageExpectations {
                    modality: "CT",
                    frame_of_reference_uid: &frame_of_reference_uid,
                    image_type: base.frame_type,
                    number_of_frames: part_frames,
                    shared_functional_groups: 1,
                    per_frame_functional_groups: part_frames as usize,
                    dimension_organization_uid: &dimension_organization_uid,
                    dimension_index_count: 1,
                    pixel_spacing: base.pixel_spacing,
                    image_orientation_patient: base.image_orientation_patient,
                    image_position_patient: part.image_position_patient,
                    dimension_index_values: part.dimension_index_values,
                    frame_type: base.frame_type,
                    pixel_presentation: base.pixel_presentation,
                    volumetric_properties: base.volumetric_properties,
                    volume_based_calculation_technique: base.volume_based_calculation_technique,
                    rescale_intercept: base.rescale_intercept,
                    rescale_slope: base.rescale_slope,
                    rescale_type: base.rescale_type,
                    irradiation_event_uid: &irradiation_event_uid,
                    concatenation: Some(EnhancedCtConcatenationExpectations {
                        concatenation_uid: &concatenation_uid,
                        in_concatenation_number: part.in_concatenation_number,
                        in_concatenation_total_number: recipe.parts.len() as u16,
                        concatenation_frame_offset_number: part.concatenation_frame_offset_number,
                        sop_instance_uid_of_concatenation_source:
                            &sop_instance_uid_of_concatenation_source,
                    }),
                }),
                enhanced_mr_image: None,
                mg_image: None,
                dx_image: None,
                us_image: None,
                cr_image: None,
                mr_image: None,
                segmentation: None,
            },
        )?;

        generated_files.push(GeneratedFile {
            case_id: base.case_id.to_string(),
            manifest_entry: enhanced_ct_manifest_entry(
                case,
                base,
                &relative_path,
                part_frames,
                part.pixel_bytes,
                part.pixel_values,
                part.image_position_patient,
                part.dimension_index_values,
                &study_instance_uid,
                &series_instance_uid,
                &sop_instance_uid,
                &frame_of_reference_uid,
                &dimension_organization_uid,
                &irradiation_event_uid,
                &implementation_class_uid,
                Some(concatenation),
                &validated.bytes,
                validated.validation,
            ),
        });
    }

    Ok(generated_files)
}

fn put_enhanced_ct_dimension_sequences(
    obj: &mut InMemDicomObject,
    dimension_organization_uid: &str,
) {
    obj.put(DataElement::new(
        tags::DIMENSION_ORGANIZATION_SEQUENCE,
        VR::SQ,
        DataSetSequence::from(vec![InMemDicomObject::from_element_iter([
            DataElement::new(
                tags::DIMENSION_ORGANIZATION_UID,
                VR::UI,
                dimension_organization_uid,
            ),
        ])]),
    ));
    obj.put(DataElement::new(
        tags::DIMENSION_INDEX_SEQUENCE,
        VR::SQ,
        DataSetSequence::from(vec![InMemDicomObject::from_element_iter([
            DataElement::new(
                tags::DIMENSION_INDEX_POINTER,
                VR::AT,
                PrimitiveValue::Tags(vec![tags::IMAGE_POSITION_PATIENT].into()),
            ),
            DataElement::new(
                tags::FUNCTIONAL_GROUP_POINTER,
                VR::AT,
                PrimitiveValue::Tags(vec![tags::PLANE_POSITION_SEQUENCE].into()),
            ),
            DataElement::new(
                tags::DIMENSION_ORGANIZATION_UID,
                VR::UI,
                dimension_organization_uid,
            ),
            DataElement::new(tags::DIMENSION_DESCRIPTION_LABEL, VR::LO, "SlicePosition"),
        ])]),
    ));
}

fn put_enhanced_ct_functional_groups(
    obj: &mut InMemDicomObject,
    recipe: EnhancedCtRecipe,
    irradiation_event_uid: &str,
    image_position_patient: &[&str],
    dimension_index_values: &[u32],
) {
    obj.put(DataElement::new(
        tags::SHARED_FUNCTIONAL_GROUPS_SEQUENCE,
        VR::SQ,
        DataSetSequence::from(vec![InMemDicomObject::from_element_iter([
            DataElement::new(
                tags::PIXEL_MEASURES_SEQUENCE,
                VR::SQ,
                DataSetSequence::from(vec![InMemDicomObject::from_element_iter([
                    DataElement::new(tags::PIXEL_SPACING, VR::DS, recipe.pixel_spacing),
                    DataElement::new(tags::SLICE_THICKNESS, VR::DS, recipe.slice_thickness),
                    DataElement::new(
                        tags::SPACING_BETWEEN_SLICES,
                        VR::DS,
                        recipe.spacing_between_slices,
                    ),
                ])]),
            ),
            DataElement::new(
                tags::PLANE_ORIENTATION_SEQUENCE,
                VR::SQ,
                DataSetSequence::from(vec![InMemDicomObject::from_element_iter([
                    DataElement::new(
                        tags::IMAGE_ORIENTATION_PATIENT,
                        VR::DS,
                        recipe.image_orientation_patient,
                    ),
                ])]),
            ),
            DataElement::new(
                tags::FRAME_ANATOMY_SEQUENCE,
                VR::SQ,
                DataSetSequence::from(vec![InMemDicomObject::from_element_iter([
                    DataElement::new(tags::FRAME_LATERALITY, VR::CS, "U"),
                    DataElement::new(
                        tags::ANATOMIC_REGION_SEQUENCE,
                        VR::SQ,
                        DataSetSequence::from(vec![InMemDicomObject::from_element_iter([
                            DataElement::new(tags::CODE_VALUE, VR::SH, "T-D3000"),
                            DataElement::new(tags::CODING_SCHEME_DESIGNATOR, VR::SH, "SRT"),
                            DataElement::new(tags::CODE_MEANING, VR::LO, "Chest"),
                        ])]),
                    ),
                ])]),
            ),
            DataElement::new(
                tags::IRRADIATION_EVENT_IDENTIFICATION_SEQUENCE,
                VR::SQ,
                DataSetSequence::from(vec![InMemDicomObject::from_element_iter([
                    DataElement::new(tags::IRRADIATION_EVENT_UID, VR::UI, irradiation_event_uid),
                ])]),
            ),
            DataElement::new(
                tags::CT_IMAGE_FRAME_TYPE_SEQUENCE,
                VR::SQ,
                DataSetSequence::from(vec![InMemDicomObject::from_element_iter([
                    DataElement::new(tags::FRAME_TYPE, VR::CS, recipe.frame_type),
                    DataElement::new(tags::PIXEL_PRESENTATION, VR::CS, recipe.pixel_presentation),
                    DataElement::new(
                        tags::VOLUMETRIC_PROPERTIES,
                        VR::CS,
                        recipe.volumetric_properties,
                    ),
                    DataElement::new(
                        tags::VOLUME_BASED_CALCULATION_TECHNIQUE,
                        VR::CS,
                        recipe.volume_based_calculation_technique,
                    ),
                ])]),
            ),
            DataElement::new(
                tags::PIXEL_VALUE_TRANSFORMATION_SEQUENCE,
                VR::SQ,
                DataSetSequence::from(vec![InMemDicomObject::from_element_iter([
                    DataElement::new(tags::RESCALE_INTERCEPT, VR::DS, recipe.rescale_intercept),
                    DataElement::new(tags::RESCALE_SLOPE, VR::DS, recipe.rescale_slope),
                    DataElement::new(tags::RESCALE_TYPE, VR::LO, recipe.rescale_type),
                ])]),
            ),
        ])]),
    ));

    let per_frame_items = image_position_patient
        .iter()
        .enumerate()
        .map(|(index, image_position_patient)| {
            let dimension_index_value = dimension_index_values[index];
            InMemDicomObject::from_element_iter([
                DataElement::new(
                    tags::FRAME_CONTENT_SEQUENCE,
                    VR::SQ,
                    DataSetSequence::from(vec![InMemDicomObject::from_element_iter([
                        DataElement::new(
                            tags::DIMENSION_INDEX_VALUES,
                            VR::UL,
                            PrimitiveValue::from(dimension_index_value),
                        ),
                        DataElement::new(
                            tags::FRAME_ACQUISITION_NUMBER,
                            VR::US,
                            PrimitiveValue::from((index + 1) as u16),
                        ),
                    ])]),
                ),
                DataElement::new(
                    tags::PLANE_POSITION_SEQUENCE,
                    VR::SQ,
                    DataSetSequence::from(vec![InMemDicomObject::from_element_iter([
                        DataElement::new(
                            tags::IMAGE_POSITION_PATIENT,
                            VR::DS,
                            *image_position_patient,
                        ),
                    ])]),
                ),
            ])
        })
        .collect::<Vec<_>>();
    obj.put(DataElement::new(
        tags::PER_FRAME_FUNCTIONAL_GROUPS_SEQUENCE,
        VR::SQ,
        DataSetSequence::from(per_frame_items),
    ));
}

fn put_segmentation_segment_sequence(obj: &mut InMemDicomObject, recipe: SegmentationRecipe) {
    obj.put(DataElement::new(
        TAG_SEGMENT_SEQUENCE,
        VR::SQ,
        DataSetSequence::from(vec![InMemDicomObject::from_element_iter([
            DataElement::new(TAG_SEGMENT_NUMBER, VR::US, PrimitiveValue::from(1_u16)),
            DataElement::new(TAG_SEGMENT_LABEL, VR::LO, recipe.segment_label),
            DataElement::new(
                TAG_SEGMENTED_PROPERTY_CATEGORY_CODE_SEQUENCE,
                VR::SQ,
                DataSetSequence::from(vec![InMemDicomObject::from_element_iter([
                    DataElement::new(tags::CODE_VALUE, VR::SH, "T-D0050"),
                    DataElement::new(tags::CODING_SCHEME_DESIGNATOR, VR::SH, "SRT"),
                    DataElement::new(tags::CODE_MEANING, VR::LO, "Tissue"),
                ])]),
            ),
            DataElement::new(
                TAG_SEGMENTED_PROPERTY_TYPE_CODE_SEQUENCE,
                VR::SQ,
                DataSetSequence::from(vec![InMemDicomObject::from_element_iter([
                    DataElement::new(tags::CODE_VALUE, VR::SH, "113343"),
                    DataElement::new(tags::CODING_SCHEME_DESIGNATOR, VR::SH, "DCM"),
                    DataElement::new(tags::CODE_MEANING, VR::LO, "Organ"),
                ])]),
            ),
            DataElement::new(TAG_SEGMENT_ALGORITHM_TYPE, VR::CS, "AUTOMATIC"),
            DataElement::new(TAG_SEGMENT_ALGORITHM_NAME, VR::LO, "dicom-test-suite"),
            DataElement::new(
                TAG_RECOMMENDED_DISPLAY_CIELAB_VALUE,
                VR::US,
                PrimitiveValue::from([32768_u16, 49152, 32768]),
            ),
        ])]),
    ));
}

fn put_segmentation_dimension_sequences(
    obj: &mut InMemDicomObject,
    dimension_organization_uid: &str,
) {
    obj.put(DataElement::new(
        tags::DIMENSION_ORGANIZATION_SEQUENCE,
        VR::SQ,
        DataSetSequence::from(vec![InMemDicomObject::from_element_iter([
            DataElement::new(
                tags::DIMENSION_ORGANIZATION_UID,
                VR::UI,
                dimension_organization_uid,
            ),
        ])]),
    ));
    obj.put(DataElement::new(
        tags::DIMENSION_INDEX_SEQUENCE,
        VR::SQ,
        DataSetSequence::from(vec![InMemDicomObject::from_element_iter([
            DataElement::new(
                tags::DIMENSION_INDEX_POINTER,
                VR::AT,
                PrimitiveValue::Tags(vec![TAG_REFERENCED_SEGMENT_NUMBER].into()),
            ),
            DataElement::new(
                tags::FUNCTIONAL_GROUP_POINTER,
                VR::AT,
                PrimitiveValue::Tags(vec![TAG_SEGMENT_IDENTIFICATION_SEQUENCE].into()),
            ),
            DataElement::new(
                tags::DIMENSION_ORGANIZATION_UID,
                VR::UI,
                dimension_organization_uid,
            ),
            DataElement::new(tags::DIMENSION_DESCRIPTION_LABEL, VR::LO, "SegmentNumber"),
        ])]),
    ));
}

fn put_segmentation_functional_groups(
    obj: &mut InMemDicomObject,
    recipe: SegmentationRecipe,
    source: &GeneratedSourceObject,
) {
    obj.put(DataElement::new(
        tags::SHARED_FUNCTIONAL_GROUPS_SEQUENCE,
        VR::SQ,
        DataSetSequence::from(vec![InMemDicomObject::from_element_iter([
            DataElement::new(
                tags::PIXEL_MEASURES_SEQUENCE,
                VR::SQ,
                DataSetSequence::from(vec![InMemDicomObject::from_element_iter([
                    DataElement::new(tags::PIXEL_SPACING, VR::DS, "0.75\\0.75"),
                    DataElement::new(tags::SLICE_THICKNESS, VR::DS, "2.5"),
                ])]),
            ),
        ])]),
    ));

    let per_frame_items = recipe
        .referenced_frame_numbers
        .iter()
        .enumerate()
        .map(|(index, frame_number)| {
            InMemDicomObject::from_element_iter([
                DataElement::new(
                    tags::FRAME_CONTENT_SEQUENCE,
                    VR::SQ,
                    DataSetSequence::from(vec![InMemDicomObject::from_element_iter([
                        DataElement::new(
                            tags::DIMENSION_INDEX_VALUES,
                            VR::UL,
                            PrimitiveValue::from(1_u32),
                        ),
                    ])]),
                ),
                DataElement::new(
                    TAG_SEGMENT_IDENTIFICATION_SEQUENCE,
                    VR::SQ,
                    DataSetSequence::from(vec![InMemDicomObject::from_element_iter([
                        DataElement::new(
                            TAG_REFERENCED_SEGMENT_NUMBER,
                            VR::US,
                            PrimitiveValue::from(1_u16),
                        ),
                    ])]),
                ),
                DataElement::new(
                    TAG_DERIVATION_IMAGE_SEQUENCE,
                    VR::SQ,
                    DataSetSequence::from(vec![InMemDicomObject::from_element_iter([
                        DataElement::new(
                            TAG_SOURCE_IMAGE_SEQUENCE,
                            VR::SQ,
                            DataSetSequence::from(vec![InMemDicomObject::from_element_iter([
                                DataElement::new(
                                    TAG_REFERENCED_SOP_CLASS_UID,
                                    VR::UI,
                                    source.sop_class_uid.as_str(),
                                ),
                                DataElement::new(
                                    TAG_REFERENCED_SOP_INSTANCE_UID,
                                    VR::UI,
                                    source.sop_instance_uid.as_str(),
                                ),
                                DataElement::new(
                                    TAG_REFERENCED_FRAME_NUMBER,
                                    VR::IS,
                                    frame_number.to_string(),
                                ),
                                DataElement::new(
                                    TAG_PURPOSE_OF_REFERENCE_CODE_SEQUENCE,
                                    VR::SQ,
                                    DataSetSequence::from(vec![
                                        InMemDicomObject::from_element_iter([
                                            DataElement::new(tags::CODE_VALUE, VR::SH, "121322"),
                                            DataElement::new(
                                                tags::CODING_SCHEME_DESIGNATOR,
                                                VR::SH,
                                                "DCM",
                                            ),
                                            DataElement::new(
                                                tags::CODE_MEANING,
                                                VR::LO,
                                                "Source image for image processing operation",
                                            ),
                                        ]),
                                    ]),
                                ),
                            ])]),
                        ),
                        DataElement::new(
                            TAG_DERIVATION_CODE_SEQUENCE,
                            VR::SQ,
                            DataSetSequence::from(vec![InMemDicomObject::from_element_iter([
                                DataElement::new(tags::CODE_VALUE, VR::SH, "113076"),
                                DataElement::new(tags::CODING_SCHEME_DESIGNATOR, VR::SH, "DCM"),
                                DataElement::new(tags::CODE_MEANING, VR::LO, "Segmentation"),
                            ])]),
                        ),
                    ])]),
                ),
                DataElement::new(
                    tags::FRAME_ACQUISITION_NUMBER,
                    VR::US,
                    PrimitiveValue::from((index + 1) as u16),
                ),
            ])
        })
        .collect::<Vec<_>>();
    obj.put(DataElement::new(
        tags::PER_FRAME_FUNCTIONAL_GROUPS_SEQUENCE,
        VR::SQ,
        DataSetSequence::from(per_frame_items),
    ));
}

fn put_common_instance_reference(obj: &mut InMemDicomObject, source: &GeneratedSourceObject) {
    obj.put(DataElement::new(
        TAG_REFERENCED_SERIES_SEQUENCE,
        VR::SQ,
        DataSetSequence::from(vec![InMemDicomObject::from_element_iter([
            DataElement::new(
                tags::SERIES_INSTANCE_UID,
                VR::UI,
                source.series_instance_uid.as_deref().unwrap_or(""),
            ),
            DataElement::new(
                TAG_REFERENCED_INSTANCE_SEQUENCE,
                VR::SQ,
                DataSetSequence::from(vec![InMemDicomObject::from_element_iter([
                    DataElement::new(
                        TAG_REFERENCED_SOP_CLASS_UID,
                        VR::UI,
                        source.sop_class_uid.as_str(),
                    ),
                    DataElement::new(
                        TAG_REFERENCED_SOP_INSTANCE_UID,
                        VR::UI,
                        source.sop_instance_uid.as_str(),
                    ),
                ])]),
            ),
        ])]),
    ));
}

fn put_rt_structure_set_references(
    obj: &mut InMemDicomObject,
    _recipe: RtStructureSetRecipe,
    source: &GeneratedSourceObject,
    frame_of_reference_uid: &str,
) {
    obj.put(DataElement::new(
        tags::REFERENCED_FRAME_OF_REFERENCE_SEQUENCE,
        VR::SQ,
        DataSetSequence::from(vec![InMemDicomObject::from_element_iter([
            DataElement::new(tags::FRAME_OF_REFERENCE_UID, VR::UI, frame_of_reference_uid),
            DataElement::new(
                tags::RT_REFERENCED_STUDY_SEQUENCE,
                VR::SQ,
                DataSetSequence::from(vec![InMemDicomObject::from_element_iter([
                    DataElement::new(
                        tags::RT_REFERENCED_SERIES_SEQUENCE,
                        VR::SQ,
                        DataSetSequence::from(vec![InMemDicomObject::from_element_iter([
                            DataElement::new(
                                tags::SERIES_INSTANCE_UID,
                                VR::UI,
                                source.series_instance_uid.as_deref().unwrap_or(""),
                            ),
                            DataElement::new(
                                tags::CONTOUR_IMAGE_SEQUENCE,
                                VR::SQ,
                                DataSetSequence::from(vec![InMemDicomObject::from_element_iter([
                                    DataElement::new(
                                        TAG_REFERENCED_SOP_CLASS_UID,
                                        VR::UI,
                                        source.sop_class_uid.as_str(),
                                    ),
                                    DataElement::new(
                                        TAG_REFERENCED_SOP_INSTANCE_UID,
                                        VR::UI,
                                        source.sop_instance_uid.as_str(),
                                    ),
                                ])]),
                            ),
                        ])]),
                    ),
                ])]),
            ),
        ])]),
    ));
}

fn put_rt_structure_set_roi_sequence(
    obj: &mut InMemDicomObject,
    recipe: RtStructureSetRecipe,
    frame_of_reference_uid: &str,
) {
    obj.put(DataElement::new(
        tags::STRUCTURE_SET_ROI_SEQUENCE,
        VR::SQ,
        DataSetSequence::from(vec![InMemDicomObject::from_element_iter([
            DataElement::new(tags::ROI_NUMBER, VR::IS, recipe.roi_number.to_string()),
            DataElement::new(
                tags::REFERENCED_FRAME_OF_REFERENCE_UID,
                VR::UI,
                frame_of_reference_uid,
            ),
            DataElement::new(tags::ROI_NAME, VR::LO, recipe.roi_name),
            DataElement::new(
                tags::ROI_GENERATION_ALGORITHM,
                VR::CS,
                recipe.roi_generation_algorithm,
            ),
            DataElement::new(
                tags::ROI_GENERATION_DESCRIPTION,
                VR::LO,
                recipe.roi_generation_description,
            ),
        ])]),
    ));
}

fn put_rt_roi_contour_sequence(
    obj: &mut InMemDicomObject,
    recipe: RtStructureSetRecipe,
    source: &GeneratedSourceObject,
) {
    obj.put(DataElement::new(
        tags::ROI_CONTOUR_SEQUENCE,
        VR::SQ,
        DataSetSequence::from(vec![InMemDicomObject::from_element_iter([
            DataElement::new(
                tags::REFERENCED_ROI_NUMBER,
                VR::IS,
                recipe.roi_number.to_string(),
            ),
            DataElement::new(
                tags::ROI_DISPLAY_COLOR,
                VR::IS,
                format!(
                    "{}\\{}\\{}",
                    recipe.roi_display_color[0],
                    recipe.roi_display_color[1],
                    recipe.roi_display_color[2]
                ),
            ),
            DataElement::new(
                tags::CONTOUR_SEQUENCE,
                VR::SQ,
                DataSetSequence::from(vec![InMemDicomObject::from_element_iter([
                    DataElement::new(
                        tags::CONTOUR_NUMBER,
                        VR::IS,
                        recipe.contour_number.to_string(),
                    ),
                    DataElement::new(
                        tags::CONTOUR_IMAGE_SEQUENCE,
                        VR::SQ,
                        DataSetSequence::from(vec![InMemDicomObject::from_element_iter([
                            DataElement::new(
                                TAG_REFERENCED_SOP_CLASS_UID,
                                VR::UI,
                                source.sop_class_uid.as_str(),
                            ),
                            DataElement::new(
                                TAG_REFERENCED_SOP_INSTANCE_UID,
                                VR::UI,
                                source.sop_instance_uid.as_str(),
                            ),
                        ])]),
                    ),
                    DataElement::new(
                        tags::CONTOUR_GEOMETRIC_TYPE,
                        VR::CS,
                        recipe.contour_geometric_type,
                    ),
                    DataElement::new(
                        tags::NUMBER_OF_CONTOUR_POINTS,
                        VR::IS,
                        recipe.contour_points.to_string(),
                    ),
                    DataElement::new(tags::CONTOUR_DATA, VR::DS, recipe.contour_data),
                ])]),
            ),
        ])]),
    ));
}

fn put_rt_roi_observations_sequence(obj: &mut InMemDicomObject, recipe: RtStructureSetRecipe) {
    obj.put(DataElement::new(
        tags::RTROI_OBSERVATIONS_SEQUENCE,
        VR::SQ,
        DataSetSequence::from(vec![InMemDicomObject::from_element_iter([
            DataElement::new(
                tags::OBSERVATION_NUMBER,
                VR::IS,
                recipe.roi_number.to_string(),
            ),
            DataElement::new(
                tags::REFERENCED_ROI_NUMBER,
                VR::IS,
                recipe.roi_number.to_string(),
            ),
            DataElement::new(
                tags::RTROI_INTERPRETED_TYPE,
                VR::CS,
                recipe.roi_interpreted_type,
            ),
            DataElement::new(tags::ROI_INTERPRETER, VR::PN, recipe.roi_interpreter),
        ])]),
    ));
}

fn put_rt_dose_references(
    obj: &mut InMemDicomObject,
    image_source: &GeneratedSourceObject,
    structure_set_source: &GeneratedSourceObject,
) {
    obj.put(DataElement::new(
        TAG_REFERENCED_IMAGE_SEQUENCE,
        VR::SQ,
        DataSetSequence::from(vec![InMemDicomObject::from_element_iter([
            DataElement::new(
                TAG_REFERENCED_SOP_CLASS_UID,
                VR::UI,
                image_source.sop_class_uid.as_str(),
            ),
            DataElement::new(
                TAG_REFERENCED_SOP_INSTANCE_UID,
                VR::UI,
                image_source.sop_instance_uid.as_str(),
            ),
        ])]),
    ));
    obj.put(DataElement::new(
        TAG_REFERENCED_STRUCTURE_SET_SEQUENCE,
        VR::SQ,
        DataSetSequence::from(vec![InMemDicomObject::from_element_iter([
            DataElement::new(
                TAG_REFERENCED_SOP_CLASS_UID,
                VR::UI,
                structure_set_source.sop_class_uid.as_str(),
            ),
            DataElement::new(
                TAG_REFERENCED_SOP_INSTANCE_UID,
                VR::UI,
                structure_set_source.sop_instance_uid.as_str(),
            ),
        ])]),
    ));
}

fn put_real_world_value_mapping_sequence(
    obj: &mut InMemDicomObject,
    recipe: RealWorldValueMappingRecipe,
    source: &GeneratedSourceObject,
) {
    obj.put(DataElement::new(
        tags::REAL_WORLD_VALUE_MAPPING_SEQUENCE,
        VR::SQ,
        DataSetSequence::from(vec![InMemDicomObject::from_element_iter([
            DataElement::new(tags::LUT_LABEL, VR::SH, recipe.lut_label),
            DataElement::new(
                tags::REAL_WORLD_VALUE_FIRST_VALUE_MAPPED,
                VR::US,
                PrimitiveValue::from(recipe.first_value_mapped),
            ),
            DataElement::new(
                tags::REAL_WORLD_VALUE_LAST_VALUE_MAPPED,
                VR::US,
                PrimitiveValue::from(recipe.last_value_mapped),
            ),
            DataElement::new(
                tags::REAL_WORLD_VALUE_INTERCEPT,
                VR::FD,
                PrimitiveValue::from(recipe.intercept),
            ),
            DataElement::new(
                tags::REAL_WORLD_VALUE_SLOPE,
                VR::FD,
                PrimitiveValue::from(recipe.slope),
            ),
            DataElement::new(
                tags::MEASUREMENT_UNITS_CODE_SEQUENCE,
                VR::SQ,
                DataSetSequence::from(vec![InMemDicomObject::from_element_iter([
                    DataElement::new(tags::CODE_VALUE, VR::SH, recipe.unit_code_value),
                    DataElement::new(
                        tags::CODING_SCHEME_DESIGNATOR,
                        VR::SH,
                        recipe.unit_coding_scheme_designator,
                    ),
                    DataElement::new(tags::CODE_MEANING, VR::LO, recipe.unit_code_meaning),
                ])]),
            ),
            DataElement::new(
                TAG_REFERENCED_IMAGE_SEQUENCE,
                VR::SQ,
                DataSetSequence::from(vec![InMemDicomObject::from_element_iter([
                    DataElement::new(
                        TAG_REFERENCED_SOP_CLASS_UID,
                        VR::UI,
                        source.sop_class_uid.as_str(),
                    ),
                    DataElement::new(
                        TAG_REFERENCED_SOP_INSTANCE_UID,
                        VR::UI,
                        source.sop_instance_uid.as_str(),
                    ),
                    DataElement::new(
                        TAG_REFERENCED_FRAME_NUMBER,
                        VR::IS,
                        recipe
                            .referenced_frame_numbers
                            .iter()
                            .map(u16::to_string)
                            .collect::<Vec<_>>()
                            .join("\\"),
                    ),
                ])]),
            ),
        ])]),
    ));
}

fn put_current_requested_procedure_evidence(
    obj: &mut InMemDicomObject,
    source: &GeneratedSourceObject,
) {
    obj.put(DataElement::new(
        tags::CURRENT_REQUESTED_PROCEDURE_EVIDENCE_SEQUENCE,
        VR::SQ,
        DataSetSequence::from(vec![InMemDicomObject::from_element_iter([
            DataElement::new(
                tags::STUDY_INSTANCE_UID,
                VR::UI,
                source.study_instance_uid.as_str(),
            ),
            DataElement::new(
                tags::REFERENCED_SERIES_SEQUENCE,
                VR::SQ,
                DataSetSequence::from(vec![InMemDicomObject::from_element_iter([
                    DataElement::new(
                        tags::SERIES_INSTANCE_UID,
                        VR::UI,
                        source.series_instance_uid.as_deref().unwrap_or(""),
                    ),
                    DataElement::new(
                        tags::REFERENCED_SOP_SEQUENCE,
                        VR::SQ,
                        DataSetSequence::from(vec![InMemDicomObject::from_element_iter([
                            DataElement::new(
                                TAG_REFERENCED_SOP_CLASS_UID,
                                VR::UI,
                                source.sop_class_uid.as_str(),
                            ),
                            DataElement::new(
                                TAG_REFERENCED_SOP_INSTANCE_UID,
                                VR::UI,
                                source.sop_instance_uid.as_str(),
                            ),
                        ])]),
                    ),
                ])]),
            ),
        ])]),
    ));
}

fn put_current_requested_procedure_evidence_many(
    obj: &mut InMemDicomObject,
    sources: &[&GeneratedSourceObject],
) {
    let study_instance_uid = sources
        .first()
        .map(|source| source.study_instance_uid.as_str())
        .unwrap_or("");
    let series_items = sources
        .iter()
        .map(|source| {
            InMemDicomObject::from_element_iter([
                DataElement::new(
                    tags::SERIES_INSTANCE_UID,
                    VR::UI,
                    source.series_instance_uid.as_deref().unwrap_or(""),
                ),
                DataElement::new(
                    tags::REFERENCED_SOP_SEQUENCE,
                    VR::SQ,
                    DataSetSequence::from(vec![InMemDicomObject::from_element_iter([
                        DataElement::new(
                            TAG_REFERENCED_SOP_CLASS_UID,
                            VR::UI,
                            source.sop_class_uid.as_str(),
                        ),
                        DataElement::new(
                            TAG_REFERENCED_SOP_INSTANCE_UID,
                            VR::UI,
                            source.sop_instance_uid.as_str(),
                        ),
                    ])]),
                ),
            ])
        })
        .collect::<Vec<_>>();
    obj.put(DataElement::new(
        tags::CURRENT_REQUESTED_PROCEDURE_EVIDENCE_SEQUENCE,
        VR::SQ,
        DataSetSequence::from(vec![InMemDicomObject::from_element_iter([
            DataElement::new(tags::STUDY_INSTANCE_UID, VR::UI, study_instance_uid),
            DataElement::new(
                tags::REFERENCED_SERIES_SEQUENCE,
                VR::SQ,
                DataSetSequence::from(series_items),
            ),
        ])]),
    ));
}

fn put_basic_text_sr_content_tree(obj: &mut InMemDicomObject, recipe: BasicTextSrRecipe) {
    put_str(obj, tags::VALUE_TYPE, VR::CS, recipe.root_value_type);
    put_code_sequence(
        obj,
        tags::CONCEPT_NAME_CODE_SEQUENCE,
        recipe.title_code_value,
        recipe.title_coding_scheme_designator,
        recipe.title_code_meaning,
    );
    put_str(
        obj,
        tags::CONTINUITY_OF_CONTENT,
        VR::CS,
        recipe.root_continuity_of_content,
    );
    obj.put(DataElement::new(
        tags::CONTENT_SEQUENCE,
        VR::SQ,
        DataSetSequence::from(vec![InMemDicomObject::from_element_iter([
            DataElement::new(
                tags::RELATIONSHIP_TYPE,
                VR::CS,
                recipe.observation_relationship_type,
            ),
            DataElement::new(tags::VALUE_TYPE, VR::CS, recipe.observation_value_type),
            DataElement::new(
                tags::CONCEPT_NAME_CODE_SEQUENCE,
                VR::SQ,
                DataSetSequence::from(vec![InMemDicomObject::from_element_iter([
                    DataElement::new(tags::CODE_VALUE, VR::SH, recipe.observation_code_value),
                    DataElement::new(
                        tags::CODING_SCHEME_DESIGNATOR,
                        VR::SH,
                        recipe.observation_coding_scheme_designator,
                    ),
                    DataElement::new(tags::CODE_MEANING, VR::LO, recipe.observation_code_meaning),
                ])]),
            ),
            DataElement::new(tags::TEXT_VALUE, VR::UT, recipe.observation_text),
        ])]),
    ));
}

fn put_comprehensive_sr_content_tree(
    obj: &mut InMemDicomObject,
    recipe: ComprehensiveSrRecipe,
    source: &GeneratedSourceObject,
) {
    put_str(obj, tags::VALUE_TYPE, VR::CS, recipe.root_value_type);
    put_code_sequence(
        obj,
        tags::CONCEPT_NAME_CODE_SEQUENCE,
        recipe.title_code_value,
        recipe.title_coding_scheme_designator,
        recipe.title_code_meaning,
    );
    put_str(
        obj,
        tags::CONTINUITY_OF_CONTENT,
        VR::CS,
        recipe.root_continuity_of_content,
    );
    obj.put(DataElement::new(
        tags::CONTENT_SEQUENCE,
        VR::SQ,
        DataSetSequence::from(vec![
            InMemDicomObject::from_element_iter([
                DataElement::new(
                    tags::RELATIONSHIP_TYPE,
                    VR::CS,
                    recipe.measurement_relationship_type,
                ),
                DataElement::new(tags::VALUE_TYPE, VR::CS, recipe.measurement_value_type),
                DataElement::new(
                    tags::CONCEPT_NAME_CODE_SEQUENCE,
                    VR::SQ,
                    DataSetSequence::from(vec![InMemDicomObject::from_element_iter([
                        DataElement::new(tags::CODE_VALUE, VR::SH, recipe.measurement_code_value),
                        DataElement::new(
                            tags::CODING_SCHEME_DESIGNATOR,
                            VR::SH,
                            recipe.measurement_coding_scheme_designator,
                        ),
                        DataElement::new(
                            tags::CODE_MEANING,
                            VR::LO,
                            recipe.measurement_code_meaning,
                        ),
                    ])]),
                ),
                DataElement::new(
                    tags::MEASURED_VALUE_SEQUENCE,
                    VR::SQ,
                    DataSetSequence::from(vec![InMemDicomObject::from_element_iter([
                        DataElement::new(tags::NUMERIC_VALUE, VR::DS, recipe.numeric_value),
                        DataElement::new(
                            tags::MEASUREMENT_UNITS_CODE_SEQUENCE,
                            VR::SQ,
                            DataSetSequence::from(vec![InMemDicomObject::from_element_iter([
                                DataElement::new(tags::CODE_VALUE, VR::SH, recipe.unit_code_value),
                                DataElement::new(
                                    tags::CODING_SCHEME_DESIGNATOR,
                                    VR::SH,
                                    recipe.unit_coding_scheme_designator,
                                ),
                                DataElement::new(
                                    tags::CODE_MEANING,
                                    VR::LO,
                                    recipe.unit_code_meaning,
                                ),
                            ])]),
                        ),
                    ])]),
                ),
            ]),
            InMemDicomObject::from_element_iter([
                DataElement::new(
                    tags::RELATIONSHIP_TYPE,
                    VR::CS,
                    recipe.image_relationship_type,
                ),
                DataElement::new(tags::VALUE_TYPE, VR::CS, recipe.image_value_type),
                DataElement::new(
                    tags::CONCEPT_NAME_CODE_SEQUENCE,
                    VR::SQ,
                    DataSetSequence::from(vec![InMemDicomObject::from_element_iter([
                        DataElement::new(tags::CODE_VALUE, VR::SH, recipe.image_code_value),
                        DataElement::new(
                            tags::CODING_SCHEME_DESIGNATOR,
                            VR::SH,
                            recipe.image_coding_scheme_designator,
                        ),
                        DataElement::new(tags::CODE_MEANING, VR::LO, recipe.image_code_meaning),
                    ])]),
                ),
                DataElement::new(
                    tags::REFERENCED_SOP_SEQUENCE,
                    VR::SQ,
                    DataSetSequence::from(vec![InMemDicomObject::from_element_iter([
                        DataElement::new(
                            TAG_REFERENCED_SOP_CLASS_UID,
                            VR::UI,
                            source.sop_class_uid.as_str(),
                        ),
                        DataElement::new(
                            TAG_REFERENCED_SOP_INSTANCE_UID,
                            VR::UI,
                            source.sop_instance_uid.as_str(),
                        ),
                        DataElement::new(
                            TAG_REFERENCED_FRAME_NUMBER,
                            VR::IS,
                            recipe
                                .referenced_frame_numbers
                                .iter()
                                .map(u16::to_string)
                                .collect::<Vec<_>>()
                                .join("\\"),
                        ),
                    ])]),
                ),
            ]),
        ]),
    ));
}

fn put_key_object_selection_content_tree(
    obj: &mut InMemDicomObject,
    recipe: KeyObjectSelectionRecipe,
    image_source: &GeneratedSourceObject,
    seg_source: &GeneratedSourceObject,
) {
    put_str(obj, tags::VALUE_TYPE, VR::CS, recipe.root_value_type);
    put_code_sequence(
        obj,
        tags::CONCEPT_NAME_CODE_SEQUENCE,
        recipe.title_code_value,
        recipe.title_coding_scheme_designator,
        recipe.title_code_meaning,
    );
    put_str(
        obj,
        tags::CONTINUITY_OF_CONTENT,
        VR::CS,
        recipe.root_continuity_of_content,
    );
    obj.put(DataElement::new(
        tags::CONTENT_TEMPLATE_SEQUENCE,
        VR::SQ,
        DataSetSequence::from(vec![InMemDicomObject::from_element_iter([
            DataElement::new(tags::MAPPING_RESOURCE, VR::CS, recipe.mapping_resource),
            DataElement::new(
                tags::TEMPLATE_IDENTIFIER,
                VR::CS,
                recipe.template_identifier,
            ),
        ])]),
    ));
    obj.put(DataElement::new(
        tags::CONTENT_SEQUENCE,
        VR::SQ,
        DataSetSequence::from(vec![
            key_object_selection_image_item(
                recipe,
                image_source,
                Some(recipe.image_referenced_frame_numbers),
            ),
            key_object_selection_image_item(recipe, seg_source, None),
        ]),
    ));
}

fn key_object_selection_image_item(
    recipe: KeyObjectSelectionRecipe,
    source: &GeneratedSourceObject,
    referenced_frame_numbers: Option<&[u16]>,
) -> InMemDicomObject {
    let mut referenced_sop = InMemDicomObject::from_element_iter([
        DataElement::new(
            TAG_REFERENCED_SOP_CLASS_UID,
            VR::UI,
            source.sop_class_uid.as_str(),
        ),
        DataElement::new(
            TAG_REFERENCED_SOP_INSTANCE_UID,
            VR::UI,
            source.sop_instance_uid.as_str(),
        ),
    ]);
    if let Some(frame_numbers) = referenced_frame_numbers {
        referenced_sop.put(DataElement::new(
            TAG_REFERENCED_FRAME_NUMBER,
            VR::IS,
            frame_numbers
                .iter()
                .map(u16::to_string)
                .collect::<Vec<_>>()
                .join("\\"),
        ));
    }

    InMemDicomObject::from_element_iter([
        DataElement::new(tags::RELATIONSHIP_TYPE, VR::CS, recipe.relationship_type),
        DataElement::new(tags::VALUE_TYPE, VR::CS, recipe.image_value_type),
        DataElement::new(
            tags::REFERENCED_SOP_SEQUENCE,
            VR::SQ,
            DataSetSequence::from(vec![referenced_sop]),
        ),
    ])
}

fn put_presentation_state_relationship(
    obj: &mut InMemDicomObject,
    source: &GeneratedSourceObject,
    source_series_instance_uid: &str,
) {
    obj.put(DataElement::new(
        TAG_REFERENCED_SERIES_SEQUENCE,
        VR::SQ,
        DataSetSequence::from(vec![InMemDicomObject::from_element_iter([
            DataElement::new(
                tags::SERIES_INSTANCE_UID,
                VR::UI,
                source_series_instance_uid,
            ),
            DataElement::new(
                TAG_REFERENCED_IMAGE_SEQUENCE,
                VR::SQ,
                DataSetSequence::from(vec![InMemDicomObject::from_element_iter([
                    DataElement::new(
                        TAG_REFERENCED_SOP_CLASS_UID,
                        VR::UI,
                        source.sop_class_uid.as_str(),
                    ),
                    DataElement::new(
                        TAG_REFERENCED_SOP_INSTANCE_UID,
                        VR::UI,
                        source.sop_instance_uid.as_str(),
                    ),
                ])]),
            ),
        ])]),
    ));
}

fn put_displayed_area_selection(obj: &mut InMemDicomObject, recipe: PresentationStateRecipe) {
    obj.put(DataElement::new(
        TAG_DISPLAYED_AREA_SELECTION_SEQUENCE,
        VR::SQ,
        DataSetSequence::from(vec![InMemDicomObject::from_element_iter([
            DataElement::new(
                TAG_DISPLAYED_AREA_TOP_LEFT_HAND_CORNER,
                VR::SL,
                PrimitiveValue::from(recipe.displayed_area_top_left),
            ),
            DataElement::new(
                TAG_DISPLAYED_AREA_BOTTOM_RIGHT_HAND_CORNER,
                VR::SL,
                PrimitiveValue::from(recipe.displayed_area_bottom_right),
            ),
            DataElement::new(
                TAG_PRESENTATION_SIZE_MODE,
                VR::CS,
                recipe.presentation_size_mode,
            ),
            DataElement::new(
                TAG_PRESENTATION_PIXEL_ASPECT_RATIO,
                VR::IS,
                format!(
                    "{}\\{}",
                    recipe.presentation_pixel_aspect_ratio[0],
                    recipe.presentation_pixel_aspect_ratio[1]
                ),
            ),
        ])]),
    ));
}

fn put_softcopy_voi_lut(obj: &mut InMemDicomObject, recipe: PresentationStateRecipe) {
    obj.put(DataElement::new(
        TAG_SOFTCOPY_VOI_LUT_SEQUENCE,
        VR::SQ,
        DataSetSequence::from(vec![InMemDicomObject::from_element_iter([
            DataElement::new(tags::WINDOW_CENTER, VR::DS, recipe.window_center),
            DataElement::new(tags::WINDOW_WIDTH, VR::DS, recipe.window_width),
            DataElement::new(TAG_WINDOW_EXPLANATION, VR::LO, recipe.window_explanation),
        ])]),
    ));
}

fn segmentation_frame_byte_len(recipe: SegmentationRecipe) -> usize {
    match recipe.pixel_data_length_formula {
        PixelDataLengthFormula::BitPackedFrames => {
            (usize::from(recipe.rows) * usize::from(recipe.columns)).div_ceil(8)
        }
        PixelDataLengthFormula::ContiguousSamples => {
            usize::from(recipe.rows)
                * usize::from(recipe.columns)
                * usize::from(recipe.bits_allocated)
                / 8
        }
        PixelDataLengthFormula::YbrFull422 | PixelDataLengthFormula::Encapsulated { .. } => {
            unreachable!("segmentation recipes do not use this native frame length formula")
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn segmentation_manifest_entry(
    case: &Value,
    recipe: SegmentationRecipe,
    source: &GeneratedSourceObject,
    relative_path: &str,
    study_instance_uid: &str,
    series_instance_uid: &str,
    sop_instance_uid: &str,
    frame_of_reference_uid: &str,
    dimension_organization_uid: &str,
    implementation_class_uid: &str,
    bytes: &[u8],
    validation: Value,
    compressed_pixel_data: Option<(crate::codecs::CodecBackendInfo, &EncapsulatedPixelData)>,
) -> Value {
    let standards_evidence = standards_evidence_from_case(case);
    let frame_byte_len = segmentation_frame_byte_len(recipe);
    let frame_hashes = recipe
        .pixel_bytes
        .chunks(frame_byte_len)
        .map(sha256_hex)
        .collect::<Vec<_>>();
    let known_stressors = std::iter::once("segmentation_storage")
        .chain(recipe.stressors.iter().copied())
        .chain([
            "derived_source_reference",
            "multi_frame_functional_groups",
            "multi_frame_dimension",
        ])
        .collect::<Vec<_>>();
    let codec_manifest = compressed_pixel_data.map(|(backend, _)| {
        serde_json::json!({
            "backend_id": backend.backend_id,
            "backend_kind": backend.backend_kind.as_str(),
            "display_name": backend.display_name,
            "version": backend.version,
            "transfer_syntax_uid": backend.transfer_syntax_uid,
            "feature_gate": backend.feature_gate,
            "determinism": backend.determinism.as_str()
        })
    });
    let pixel_data_manifest = if let Some((_, encapsulated)) = compressed_pixel_data {
        serde_json::json!({
            "vr": "OB",
            "native_or_encapsulated": "encapsulated",
            "value_length": Value::Null,
            "frame_count": recipe.frames,
            "frame_hashes": frame_hashes,
            "codec": codec_manifest,
            "encapsulated_pixel_data": {
                "basic_offset_table": {
                    "present": true,
                    "populated": encapsulated.basic_offset_table.is_populated(),
                    "offset_count": encapsulated.basic_offset_table.offsets.len(),
                    "offsets": encapsulated.basic_offset_table.offsets.clone()
                },
                "fragments_per_frame": encapsulated.fragments_per_frame.clone(),
                "fragments": encapsulated.fragments.iter().map(|fragment| {
                    serde_json::json!({
                        "frame_index": fragment.frame_index,
                        "item_start_offset": fragment.item_start_offset,
                        "compressed_length": fragment.compressed_length,
                        "padded_length": fragment.padded_length
                    })
                }).collect::<Vec<_>>(),
                "extended_offset_table": {
                    "present": false,
                    "lengths_present": false,
                    "offset_count": 0,
                    "length_count": 0
                },
                "compressed_frame_hashes": encapsulated.compressed_frame_hashes.clone()
            }
        })
    } else {
        serde_json::json!({
            "vr": "OB",
            "native_or_encapsulated": "native",
            "value_length": recipe.pixel_bytes.len(),
            "frame_count": recipe.frames,
            "frame_hashes": frame_hashes
        })
    };
    serde_json::json!({
        "case_id": recipe.case_id,
        "profile_membership": ["extended"],
        "path": relative_path,
        "sha256": sha256_hex(bytes),
        "size_bytes": bytes.len(),
        "determinism": "byte_stable",
        "recipe": {
            "recipe_id": recipe.recipe_id,
            "recipe_version": SEGMENTATION_RECIPE_VERSION,
            "recipe_parameters": {
                "source_case_id": recipe.source_case_id,
                "rows": recipe.rows,
                "columns": recipe.columns,
                "frames": recipe.frames,
                "samples_per_pixel": 1,
                "photometric_interpretation": "MONOCHROME2",
                "bits_allocated": recipe.bits_allocated,
                "bits_stored": recipe.bits_stored,
                "high_bit": recipe.high_bit,
                "pixel_representation": 0,
                "pixel_values": recipe.pixel_values,
                "segmentation_type": recipe.segmentation_type,
                "segmentation_fractional_type": recipe.segmentation_fractional_type,
                "maximum_fractional_value": recipe.maximum_fractional_value,
                "segment_count": 1,
                "segment_label": recipe.segment_label,
                "referenced_frame_numbers": recipe.referenced_frame_numbers,
                "dimension_index": {
                    "dimension_organization_uid": dimension_organization_uid,
                    "dimension_index_pointer": "ReferencedSegmentNumber",
                    "functional_group_pointer": "SegmentIdentificationSequence"
                }
            }
        },
        "dicom": {
            "sop_class_uid": recipe.sop_class_uid,
            "sop_class_name": recipe.sop_class_name,
            "iod_name": "Segmentation",
            "modality": "SEG",
            "transfer_syntax_uid": recipe.transfer_syntax.uid,
            "transfer_syntax_name": recipe.transfer_syntax.name
        },
        "uids": {
            "study_instance_uid": study_instance_uid,
            "series_instance_uid": series_instance_uid,
            "sop_instance_uid": sop_instance_uid,
            "frame_of_reference_uid": frame_of_reference_uid,
            "dimension_organization_uid": dimension_organization_uid,
            "implementation_class_uid": implementation_class_uid
        },
        "image": {
            "rows": recipe.rows,
            "columns": recipe.columns,
            "frames": recipe.frames,
            "samples_per_pixel": 1,
            "photometric_interpretation": "MONOCHROME2",
            "bits_allocated": recipe.bits_allocated,
            "bits_stored": recipe.bits_stored,
            "high_bit": recipe.high_bit,
            "pixel_representation": 0,
            "planar_configuration": Value::Null
        },
        "pixel_data": pixel_data_manifest,
        "references": [
            source.to_manifest_reference(
                "source_image",
                Some(recipe.referenced_frame_numbers.iter().map(|frame| u64::from(*frame)).collect())
            )
        ],
        "expected_capabilities": ["open_file", "read_metadata", "show_unsupported_but_recognized", "parse_segmentation"],
        "expected_semantics": {
            "synthetic_data": "YES",
            "pixel_min": recipe.pixel_min,
            "pixel_max": recipe.pixel_max,
            "segmentation_type": recipe.segmentation_type,
            "segmentation_fractional_type": recipe.segmentation_fractional_type,
            "maximum_fractional_value": recipe.maximum_fractional_value,
            "segment_sequence_items": 1,
            "shared_functional_groups_sequence_items": 1,
            "per_frame_functional_groups_sequence_items": recipe.frames,
            "source_case_id": source.source_case_id,
            "source_sop_instance_uid": source.sop_instance_uid,
            "referenced_frame_numbers": recipe.referenced_frame_numbers
        },
        "expected_visual_checks": {
            "pattern": recipe.visual_pattern
        },
        "validation": validation,
        "known_stressors": known_stressors,
        "standards_evidence": deduplicated_standards_evidence(standards_evidence)
    })
}

#[allow(clippy::too_many_arguments)]
fn presentation_state_manifest_entry(
    case: &Value,
    recipe: PresentationStateRecipe,
    source: &GeneratedSourceObject,
    relative_path: &str,
    study_instance_uid: &str,
    series_instance_uid: &str,
    sop_instance_uid: &str,
    implementation_class_uid: &str,
    bytes: &[u8],
    validation: Value,
) -> Value {
    let standards_evidence = standards_evidence_from_case(case);
    serde_json::json!({
        "case_id": recipe.case_id,
        "profile_membership": ["extended"],
        "path": relative_path,
        "sha256": sha256_hex(bytes),
        "size_bytes": bytes.len(),
        "determinism": "byte_stable",
        "recipe": {
            "recipe_id": recipe.recipe_id,
            "recipe_version": GSPS_RECIPE_VERSION,
            "recipe_parameters": {
                "source_case_id": recipe.source_case_id,
                "content_label": recipe.content_label,
                "content_description": recipe.content_description,
                "displayed_area_top_left": recipe.displayed_area_top_left,
                "displayed_area_bottom_right": recipe.displayed_area_bottom_right,
                "presentation_size_mode": recipe.presentation_size_mode,
                "presentation_pixel_aspect_ratio": recipe.presentation_pixel_aspect_ratio,
                "window_center": recipe.window_center,
                "window_width": recipe.window_width,
                "window_explanation": recipe.window_explanation,
                "presentation_lut_shape": recipe.presentation_lut_shape
            }
        },
        "dicom": {
            "sop_class_uid": GRAYSCALE_SOFTCOPY_PRESENTATION_STATE_STORAGE_UID,
            "sop_class_name": "Grayscale Softcopy Presentation State Storage",
            "iod_name": "Grayscale Softcopy Presentation State",
            "modality": "PR",
            "transfer_syntax_uid": EXPLICIT_VR_LITTLE_ENDIAN.uid,
            "transfer_syntax_name": EXPLICIT_VR_LITTLE_ENDIAN.name
        },
        "uids": {
            "study_instance_uid": study_instance_uid,
            "series_instance_uid": series_instance_uid,
            "sop_instance_uid": sop_instance_uid,
            "implementation_class_uid": implementation_class_uid
        },
        "image": Value::Null,
        "pixel_data": Value::Null,
        "references": [
            source.to_manifest_reference("source_image", Some(vec![1, 2]))
        ],
        "expected_capabilities": ["open_file", "read_metadata", "show_unsupported_but_recognized", "apply_presentation_state"],
        "expected_semantics": {
            "synthetic_data": "YES",
            "source_case_id": source.source_case_id,
            "source_sop_instance_uid": source.sop_instance_uid,
            "presentation_state": {
                "displayed_area_top_left": recipe.displayed_area_top_left,
                "displayed_area_bottom_right": recipe.displayed_area_bottom_right,
                "presentation_size_mode": recipe.presentation_size_mode,
                "presentation_pixel_aspect_ratio": recipe.presentation_pixel_aspect_ratio,
                "window_center": recipe.window_center,
                "window_width": recipe.window_width,
                "presentation_lut_shape": recipe.presentation_lut_shape
            }
        },
        "expected_visual_checks": {
            "pattern": "source_ct_with_softcopy_window"
        },
        "validation": validation,
        "known_stressors": ["grayscale_softcopy_presentation_state", "derived_source_reference", "softcopy_voi_window", "displayed_area"],
        "standards_evidence": deduplicated_standards_evidence(standards_evidence)
    })
}

#[allow(clippy::too_many_arguments)]
fn real_world_value_mapping_manifest_entry(
    case: &Value,
    recipe: RealWorldValueMappingRecipe,
    source: &GeneratedSourceObject,
    relative_path: &str,
    study_instance_uid: &str,
    series_instance_uid: &str,
    sop_instance_uid: &str,
    implementation_class_uid: &str,
    bytes: &[u8],
    validation: Value,
) -> Value {
    let standards_evidence = standards_evidence_from_case(case);
    serde_json::json!({
        "case_id": recipe.case_id,
        "profile_membership": ["extended"],
        "path": relative_path,
        "sha256": sha256_hex(bytes),
        "size_bytes": bytes.len(),
        "determinism": "byte_stable",
        "recipe": {
            "recipe_id": recipe.recipe_id,
            "recipe_version": RWVM_RECIPE_VERSION,
            "recipe_parameters": {
                "source_case_id": recipe.source_case_id,
                "content_label": recipe.content_label,
                "content_description": recipe.content_description,
                "lut_label": recipe.lut_label,
                "first_value_mapped": recipe.first_value_mapped,
                "last_value_mapped": recipe.last_value_mapped,
                "intercept": recipe.intercept,
                "slope": recipe.slope,
                "measurement_units": {
                    "code_value": recipe.unit_code_value,
                    "coding_scheme_designator": recipe.unit_coding_scheme_designator,
                    "code_meaning": recipe.unit_code_meaning
                },
                "referenced_frame_numbers": recipe.referenced_frame_numbers
            }
        },
        "dicom": {
            "sop_class_uid": REAL_WORLD_VALUE_MAPPING_STORAGE_UID,
            "sop_class_name": "Real World Value Mapping Storage",
            "iod_name": "Real World Value Mapping",
            "modality": "RWV",
            "transfer_syntax_uid": EXPLICIT_VR_LITTLE_ENDIAN.uid,
            "transfer_syntax_name": EXPLICIT_VR_LITTLE_ENDIAN.name
        },
        "uids": {
            "study_instance_uid": study_instance_uid,
            "series_instance_uid": series_instance_uid,
            "sop_instance_uid": sop_instance_uid,
            "implementation_class_uid": implementation_class_uid
        },
        "image": Value::Null,
        "pixel_data": Value::Null,
        "references": [
            source.to_manifest_reference(
                "source_image",
                Some(
                    recipe
                        .referenced_frame_numbers
                        .iter()
                        .map(|frame| u64::from(*frame))
                        .collect::<Vec<_>>()
                )
            )
        ],
        "expected_capabilities": ["open_file", "read_metadata", "show_unsupported_but_recognized", "read_real_world_value_mapping"],
        "expected_semantics": {
            "synthetic_data": "YES",
            "source_case_id": source.source_case_id,
            "source_sop_instance_uid": source.sop_instance_uid,
            "real_world_value_mapping": {
                "lut_label": recipe.lut_label,
                "first_value_mapped": recipe.first_value_mapped,
                "last_value_mapped": recipe.last_value_mapped,
                "intercept": recipe.intercept,
                "slope": recipe.slope,
                "units": {
                    "code_value": recipe.unit_code_value,
                    "coding_scheme_designator": recipe.unit_coding_scheme_designator,
                    "code_meaning": recipe.unit_code_meaning
                },
                "referenced_frame_numbers": recipe.referenced_frame_numbers
            }
        },
        "expected_visual_checks": {
            "pattern": "source_ct_linear_hu_mapping_metadata"
        },
        "validation": validation,
        "known_stressors": ["real_world_value_mapping_storage", "derived_source_reference", "linear_real_world_value_mapping", "measurement_units_code_sequence"],
        "standards_evidence": deduplicated_standards_evidence(standards_evidence)
    })
}

#[allow(clippy::too_many_arguments)]
fn basic_text_sr_manifest_entry(
    case: &Value,
    recipe: BasicTextSrRecipe,
    source: &GeneratedSourceObject,
    relative_path: &str,
    study_instance_uid: &str,
    series_instance_uid: &str,
    sop_instance_uid: &str,
    implementation_class_uid: &str,
    bytes: &[u8],
    validation: Value,
) -> Value {
    let standards_evidence = standards_evidence_from_case(case);
    serde_json::json!({
        "case_id": recipe.case_id,
        "profile_membership": ["extended"],
        "path": relative_path,
        "sha256": sha256_hex(bytes),
        "size_bytes": bytes.len(),
        "determinism": "byte_stable",
        "recipe": {
            "recipe_id": recipe.recipe_id,
            "recipe_version": BASIC_TEXT_SR_RECIPE_VERSION,
            "recipe_parameters": {
                "source_case_id": recipe.source_case_id,
                "completion_flag": recipe.completion_flag,
                "verification_flag": recipe.verification_flag,
                "root_value_type": recipe.root_value_type,
                "root_continuity_of_content": recipe.root_continuity_of_content,
                "document_title": {
                    "code_value": recipe.title_code_value,
                    "coding_scheme_designator": recipe.title_coding_scheme_designator,
                    "code_meaning": recipe.title_code_meaning
                },
                "observation": {
                    "relationship_type": recipe.observation_relationship_type,
                    "value_type": recipe.observation_value_type,
                    "code_value": recipe.observation_code_value,
                    "coding_scheme_designator": recipe.observation_coding_scheme_designator,
                    "code_meaning": recipe.observation_code_meaning,
                    "text": recipe.observation_text
                }
            }
        },
        "dicom": {
            "sop_class_uid": BASIC_TEXT_SR_STORAGE_UID,
            "sop_class_name": "Basic Text SR Storage",
            "iod_name": "Basic Text SR",
            "modality": "SR",
            "transfer_syntax_uid": EXPLICIT_VR_LITTLE_ENDIAN.uid,
            "transfer_syntax_name": EXPLICIT_VR_LITTLE_ENDIAN.name
        },
        "uids": {
            "study_instance_uid": study_instance_uid,
            "series_instance_uid": series_instance_uid,
            "sop_instance_uid": sop_instance_uid,
            "implementation_class_uid": implementation_class_uid
        },
        "image": Value::Null,
        "pixel_data": Value::Null,
        "references": [
            source.to_manifest_reference("source_image", Some(vec![1, 2]))
        ],
        "expected_capabilities": ["open_file", "read_metadata", "show_unsupported_but_recognized", "read_structured_report"],
        "expected_semantics": {
            "synthetic_data": "YES",
            "source_case_id": source.source_case_id,
            "source_sop_instance_uid": source.sop_instance_uid,
            "structured_report": {
                "completion_flag": recipe.completion_flag,
                "verification_flag": recipe.verification_flag,
                "root_value_type": recipe.root_value_type,
                "root_continuity_of_content": recipe.root_continuity_of_content,
                "content_sequence_items": 1,
                "observation_text": recipe.observation_text
            }
        },
        "expected_visual_checks": {
            "pattern": "source_ct_basic_text_sr_observation"
        },
        "validation": validation,
        "known_stressors": ["basic_text_sr_storage", "derived_source_reference", "sr_document_content", "text_content_item"],
        "standards_evidence": deduplicated_standards_evidence(standards_evidence)
    })
}

#[allow(clippy::too_many_arguments)]
fn comprehensive_sr_manifest_entry(
    case: &Value,
    recipe: ComprehensiveSrRecipe,
    source: &GeneratedSourceObject,
    relative_path: &str,
    study_instance_uid: &str,
    series_instance_uid: &str,
    sop_instance_uid: &str,
    implementation_class_uid: &str,
    bytes: &[u8],
    validation: Value,
) -> Value {
    let standards_evidence = standards_evidence_from_case(case);
    serde_json::json!({
        "case_id": recipe.case_id,
        "profile_membership": ["extended"],
        "path": relative_path,
        "sha256": sha256_hex(bytes),
        "size_bytes": bytes.len(),
        "determinism": "byte_stable",
        "recipe": {
            "recipe_id": recipe.recipe_id,
            "recipe_version": COMPREHENSIVE_SR_RECIPE_VERSION,
            "recipe_parameters": {
                "source_case_id": recipe.source_case_id,
                "completion_flag": recipe.completion_flag,
                "verification_flag": recipe.verification_flag,
                "root_value_type": recipe.root_value_type,
                "root_continuity_of_content": recipe.root_continuity_of_content,
                "document_title": {
                    "code_value": recipe.title_code_value,
                    "coding_scheme_designator": recipe.title_coding_scheme_designator,
                    "code_meaning": recipe.title_code_meaning
                },
                "measurement": {
                    "relationship_type": recipe.measurement_relationship_type,
                    "value_type": recipe.measurement_value_type,
                    "code_value": recipe.measurement_code_value,
                    "coding_scheme_designator": recipe.measurement_coding_scheme_designator,
                    "code_meaning": recipe.measurement_code_meaning,
                    "numeric_value": recipe.numeric_value,
                    "units": {
                        "code_value": recipe.unit_code_value,
                        "coding_scheme_designator": recipe.unit_coding_scheme_designator,
                        "code_meaning": recipe.unit_code_meaning
                    }
                },
                "image_reference": {
                    "relationship_type": recipe.image_relationship_type,
                    "value_type": recipe.image_value_type,
                    "code_value": recipe.image_code_value,
                    "coding_scheme_designator": recipe.image_coding_scheme_designator,
                    "code_meaning": recipe.image_code_meaning,
                    "referenced_frame_numbers": recipe.referenced_frame_numbers
                }
            }
        },
        "dicom": {
            "sop_class_uid": COMPREHENSIVE_SR_STORAGE_UID,
            "sop_class_name": "Comprehensive SR Storage",
            "iod_name": "Comprehensive SR",
            "modality": "SR",
            "transfer_syntax_uid": EXPLICIT_VR_LITTLE_ENDIAN.uid,
            "transfer_syntax_name": EXPLICIT_VR_LITTLE_ENDIAN.name
        },
        "uids": {
            "study_instance_uid": study_instance_uid,
            "series_instance_uid": series_instance_uid,
            "sop_instance_uid": sop_instance_uid,
            "implementation_class_uid": implementation_class_uid
        },
        "image": Value::Null,
        "pixel_data": Value::Null,
        "references": [
            source.to_manifest_reference(
                "source_image",
                Some(
                    recipe
                        .referenced_frame_numbers
                        .iter()
                        .map(|frame| u64::from(*frame))
                        .collect::<Vec<_>>()
                )
            )
        ],
        "expected_capabilities": ["open_file", "read_metadata", "show_unsupported_but_recognized", "read_structured_report", "read_image_measurement"],
        "expected_semantics": {
            "synthetic_data": "YES",
            "source_case_id": source.source_case_id,
            "source_sop_instance_uid": source.sop_instance_uid,
            "structured_report": {
                "completion_flag": recipe.completion_flag,
                "verification_flag": recipe.verification_flag,
                "root_value_type": recipe.root_value_type,
                "root_continuity_of_content": recipe.root_continuity_of_content,
                "content_sequence_items": 2,
                "measurement": {
                    "relationship_type": recipe.measurement_relationship_type,
                    "value_type": recipe.measurement_value_type,
                    "code_value": recipe.measurement_code_value,
                    "coding_scheme_designator": recipe.measurement_coding_scheme_designator,
                    "code_meaning": recipe.measurement_code_meaning,
                    "numeric_value": recipe.numeric_value,
                    "units": {
                        "code_value": recipe.unit_code_value,
                        "coding_scheme_designator": recipe.unit_coding_scheme_designator,
                        "code_meaning": recipe.unit_code_meaning
                    }
                },
                "image_reference": {
                    "relationship_type": recipe.image_relationship_type,
                    "value_type": recipe.image_value_type,
                    "code_value": recipe.image_code_value,
                    "coding_scheme_designator": recipe.image_coding_scheme_designator,
                    "code_meaning": recipe.image_code_meaning,
                    "referenced_frame_numbers": recipe.referenced_frame_numbers
                }
            }
        },
        "expected_visual_checks": {
            "pattern": "source_ct_comprehensive_sr_measurement"
        },
        "validation": validation,
        "known_stressors": ["comprehensive_sr_storage", "derived_source_reference", "sr_document_content", "num_content_item", "image_content_item"],
        "standards_evidence": deduplicated_standards_evidence(standards_evidence)
    })
}

#[allow(clippy::too_many_arguments)]
fn key_object_selection_manifest_entry(
    case: &Value,
    recipe: KeyObjectSelectionRecipe,
    image_source: &GeneratedSourceObject,
    seg_source: &GeneratedSourceObject,
    relative_path: &str,
    study_instance_uid: &str,
    series_instance_uid: &str,
    sop_instance_uid: &str,
    implementation_class_uid: &str,
    bytes: &[u8],
    validation: Value,
) -> Value {
    let standards_evidence = standards_evidence_from_case(case);
    serde_json::json!({
        "case_id": recipe.case_id,
        "profile_membership": ["extended"],
        "path": relative_path,
        "sha256": sha256_hex(bytes),
        "size_bytes": bytes.len(),
        "determinism": "byte_stable",
        "recipe": {
            "recipe_id": recipe.recipe_id,
            "recipe_version": KEY_OBJECT_SELECTION_RECIPE_VERSION,
            "recipe_parameters": {
                "image_source_case_id": recipe.image_source_case_id,
                "seg_source_case_id": recipe.seg_source_case_id,
                "completion_flag": recipe.completion_flag,
                "verification_flag": recipe.verification_flag,
                "root_value_type": recipe.root_value_type,
                "root_continuity_of_content": recipe.root_continuity_of_content,
                "document_title": {
                    "code_value": recipe.title_code_value,
                    "coding_scheme_designator": recipe.title_coding_scheme_designator,
                    "code_meaning": recipe.title_code_meaning
                },
                "content_template": {
                    "mapping_resource": recipe.mapping_resource,
                    "template_identifier": recipe.template_identifier
                },
                "key_object_items": [
                    {
                        "relationship_type": recipe.relationship_type,
                        "value_type": recipe.image_value_type,
                        "source_case_id": image_source.source_case_id,
                        "referenced_frame_numbers": recipe.image_referenced_frame_numbers
                    },
                    {
                        "relationship_type": recipe.relationship_type,
                        "value_type": recipe.image_value_type,
                        "source_case_id": seg_source.source_case_id
                    }
                ]
            }
        },
        "dicom": {
            "sop_class_uid": KEY_OBJECT_SELECTION_DOCUMENT_STORAGE_UID,
            "sop_class_name": "Key Object Selection Document Storage",
            "iod_name": "Key Object Selection Document",
            "modality": "KO",
            "transfer_syntax_uid": EXPLICIT_VR_LITTLE_ENDIAN.uid,
            "transfer_syntax_name": EXPLICIT_VR_LITTLE_ENDIAN.name
        },
        "uids": {
            "study_instance_uid": study_instance_uid,
            "series_instance_uid": series_instance_uid,
            "sop_instance_uid": sop_instance_uid,
            "implementation_class_uid": implementation_class_uid
        },
        "image": Value::Null,
        "pixel_data": Value::Null,
        "references": [
            image_source.to_manifest_reference(
                "source_image",
                Some(
                    recipe
                        .image_referenced_frame_numbers
                        .iter()
                        .map(|frame| u64::from(*frame))
                        .collect::<Vec<_>>()
                )
            ),
            seg_source.to_manifest_reference("key_object_segmentation", None)
        ],
        "expected_capabilities": ["open_file", "read_metadata", "show_unsupported_but_recognized", "read_structured_report", "read_key_object_selection"],
        "expected_semantics": {
            "synthetic_data": "YES",
            "source_case_id": image_source.source_case_id,
            "source_sop_instance_uid": image_source.sop_instance_uid,
            "structured_report": {
                "completion_flag": recipe.completion_flag,
                "verification_flag": recipe.verification_flag,
                "root_value_type": recipe.root_value_type,
                "root_continuity_of_content": recipe.root_continuity_of_content,
                "content_template": {
                    "mapping_resource": recipe.mapping_resource,
                    "template_identifier": recipe.template_identifier
                },
                "content_sequence_items": 2,
                "key_objects": [
                    {
                        "relationship_type": recipe.relationship_type,
                        "value_type": recipe.image_value_type,
                        "source_case_id": image_source.source_case_id,
                        "sop_instance_uid": image_source.sop_instance_uid,
                        "referenced_frame_numbers": recipe.image_referenced_frame_numbers
                    },
                    {
                        "relationship_type": recipe.relationship_type,
                        "value_type": recipe.image_value_type,
                        "source_case_id": seg_source.source_case_id,
                        "sop_instance_uid": seg_source.sop_instance_uid
                    }
                ]
            }
        },
        "expected_visual_checks": {
            "pattern": "source_ct_and_seg_key_object_selection"
        },
        "validation": validation,
        "known_stressors": ["key_object_selection_document_storage", "derived_source_reference", "sr_document_content", "multiple_evidence_references"],
        "standards_evidence": deduplicated_standards_evidence(standards_evidence)
    })
}

#[allow(clippy::too_many_arguments)]
fn rt_structure_set_manifest_entry(
    case: &Value,
    recipe: RtStructureSetRecipe,
    source: &GeneratedSourceObject,
    relative_path: &str,
    study_instance_uid: &str,
    series_instance_uid: &str,
    sop_instance_uid: &str,
    frame_of_reference_uid: &str,
    implementation_class_uid: &str,
    bytes: &[u8],
    validation: Value,
) -> Value {
    let standards_evidence = standards_evidence_from_case(case);
    serde_json::json!({
        "case_id": recipe.case_id,
        "profile_membership": ["extended"],
        "path": relative_path,
        "sha256": sha256_hex(bytes),
        "size_bytes": bytes.len(),
        "determinism": "byte_stable",
        "recipe": {
            "recipe_id": recipe.recipe_id,
            "recipe_version": RT_STRUCTURE_SET_RECIPE_VERSION,
            "recipe_parameters": {
                "source_case_id": recipe.source_case_id,
                "structure_set_label": recipe.structure_set_label,
                "structure_set_name": recipe.structure_set_name,
                "roi_number": recipe.roi_number,
                "roi_name": recipe.roi_name,
                "roi_generation_algorithm": recipe.roi_generation_algorithm,
                "roi_generation_description": recipe.roi_generation_description,
                "roi_display_color": recipe.roi_display_color,
                "contour_number": recipe.contour_number,
                "contour_geometric_type": recipe.contour_geometric_type,
                "contour_points": recipe.contour_points,
                "contour_data": recipe.contour_data,
                "roi_interpreted_type": recipe.roi_interpreted_type
            }
        },
        "dicom": {
            "sop_class_uid": RT_STRUCTURE_SET_STORAGE_UID,
            "sop_class_name": "RT Structure Set Storage",
            "iod_name": "RT Structure Set",
            "modality": "RTSTRUCT",
            "transfer_syntax_uid": EXPLICIT_VR_LITTLE_ENDIAN.uid,
            "transfer_syntax_name": EXPLICIT_VR_LITTLE_ENDIAN.name
        },
        "uids": {
            "study_instance_uid": study_instance_uid,
            "series_instance_uid": series_instance_uid,
            "sop_instance_uid": sop_instance_uid,
            "frame_of_reference_uid": frame_of_reference_uid,
            "implementation_class_uid": implementation_class_uid
        },
        "image": Value::Null,
        "pixel_data": Value::Null,
        "references": [
            source.to_manifest_reference("source_image", Some(vec![1, 2]))
        ],
        "expected_capabilities": ["open_file", "read_metadata", "show_unsupported_but_recognized", "read_rt_structure_set"],
        "expected_semantics": {
            "synthetic_data": "YES",
            "source_case_id": source.source_case_id,
            "source_sop_instance_uid": source.sop_instance_uid,
            "rt_structure_set": {
                "structure_set_label": recipe.structure_set_label,
                "structure_set_roi_items": 1,
                "roi_number": recipe.roi_number,
                "roi_name": recipe.roi_name,
                "roi_generation_algorithm": recipe.roi_generation_algorithm,
                "roi_contour_items": 1,
                "contour_items": 1,
                "contour_geometric_type": recipe.contour_geometric_type,
                "contour_points": recipe.contour_points,
                "contour_data": recipe.contour_data,
                "rt_roi_observation_items": 1,
                "roi_interpreted_type": recipe.roi_interpreted_type
            }
        },
        "expected_visual_checks": {
            "pattern": "single_closed_planar_roi_on_source_ct"
        },
        "validation": validation,
        "known_stressors": ["rt_structure_set_storage", "derived_source_reference", "closed_planar_roi_contour", "rt_roi_observations"],
        "standards_evidence": deduplicated_standards_evidence(standards_evidence)
    })
}

#[allow(clippy::too_many_arguments)]
fn rt_dose_manifest_entry(
    case: &Value,
    recipe: RtDoseRecipe,
    image_source: &GeneratedSourceObject,
    structure_set_source: &GeneratedSourceObject,
    relative_path: &str,
    study_instance_uid: &str,
    series_instance_uid: &str,
    sop_instance_uid: &str,
    frame_of_reference_uid: &str,
    implementation_class_uid: &str,
    bytes: &[u8],
    validation: Value,
) -> Value {
    let standards_evidence = standards_evidence_from_case(case);
    let frame_byte_len = usize::from(recipe.rows) * usize::from(recipe.columns) * 2;
    let frame_hashes = recipe
        .pixel_bytes
        .chunks(frame_byte_len)
        .map(sha256_hex)
        .collect::<Vec<_>>();
    serde_json::json!({
        "case_id": recipe.case_id,
        "profile_membership": ["extended"],
        "path": relative_path,
        "sha256": sha256_hex(bytes),
        "size_bytes": bytes.len(),
        "determinism": "byte_stable",
        "recipe": {
            "recipe_id": recipe.recipe_id,
            "recipe_version": RT_DOSE_RECIPE_VERSION,
            "recipe_parameters": {
                "image_source_case_id": recipe.image_source_case_id,
                "structure_set_source_case_id": recipe.structure_set_source_case_id,
                "rows": recipe.rows,
                "columns": recipe.columns,
                "frames": recipe.frames,
                "pixel_spacing": recipe.pixel_spacing,
                "image_orientation_patient": recipe.image_orientation_patient,
                "image_position_patient": recipe.image_position_patient,
                "slice_thickness": recipe.slice_thickness,
                "frame_increment_pointer": "(3004,000C)",
                "grid_frame_offset_vector": recipe.grid_frame_offset_vector,
                "dose_units": recipe.dose_units,
                "dose_type": recipe.dose_type,
                "dose_summation_type": recipe.dose_summation_type,
                "dose_grid_scaling": recipe.dose_grid_scaling
            }
        },
        "dicom": {
            "sop_class_uid": RT_DOSE_STORAGE_UID,
            "sop_class_name": "RT Dose Storage",
            "iod_name": "RT Dose",
            "modality": "RTDOSE",
            "transfer_syntax_uid": EXPLICIT_VR_LITTLE_ENDIAN.uid,
            "transfer_syntax_name": EXPLICIT_VR_LITTLE_ENDIAN.name
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
            "frames": recipe.frames,
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
            "value_length": recipe.pixel_bytes.len(),
            "frame_count": recipe.frames,
            "frame_hashes": frame_hashes
        },
        "references": [
            image_source.to_manifest_reference("source_image", Some(vec![1, 2])),
            structure_set_source.to_manifest_reference("source_structure_set", None)
        ],
        "expected_capabilities": ["open_file", "read_metadata", "show_unsupported_but_recognized", "read_rt_dose_grid"],
        "expected_semantics": {
            "synthetic_data": "YES",
            "pixel_min": recipe.pixel_min,
            "pixel_max": recipe.pixel_max,
            "source_case_id": image_source.source_case_id,
            "source_sop_instance_uid": image_source.sop_instance_uid,
            "rt_dose": {
                "dose_units": recipe.dose_units,
                "dose_type": recipe.dose_type,
                "dose_summation_type": recipe.dose_summation_type,
                "dose_grid_scaling": recipe.dose_grid_scaling,
                "grid_frame_offset_vector": recipe.grid_frame_offset_vector,
                "referenced_image_sop_instance_uid": image_source.sop_instance_uid,
                "referenced_structure_set_sop_instance_uid": structure_set_source.sop_instance_uid
            }
        },
        "expected_visual_checks": {
            "pattern": "tiny_two_frame_rt_dose_grid"
        },
        "validation": validation,
        "known_stressors": ["rt_dose_storage", "grid_based_dose", "dose_grid_scaling", "derived_source_reference", "native_ow_pixel_data"],
        "standards_evidence": deduplicated_standards_evidence(standards_evidence)
    })
}

#[allow(clippy::too_many_arguments)]
fn encapsulated_pdf_manifest_entry(
    case: &Value,
    recipe: EncapsulatedPdfRecipe,
    relative_path: &str,
    study_instance_uid: &str,
    series_instance_uid: &str,
    sop_instance_uid: &str,
    implementation_class_uid: &str,
    bytes: &[u8],
    validation: Value,
) -> Value {
    let standards_evidence = standards_evidence_from_case(case);
    serde_json::json!({
        "case_id": recipe.case_id,
        "profile_membership": ["extended"],
        "path": relative_path,
        "sha256": sha256_hex(bytes),
        "size_bytes": bytes.len(),
        "determinism": "byte_stable",
        "recipe": {
            "recipe_id": recipe.recipe_id,
            "recipe_version": ENCAPSULATED_PDF_RECIPE_VERSION,
            "recipe_parameters": {
                "document_title": recipe.document_title,
                "mime_type": recipe.mime_type,
                "document_length": recipe.document_bytes.len(),
                "document_sha256": sha256_hex(recipe.document_bytes),
                "burned_in_annotation": recipe.burned_in_annotation,
                "recognizable_visual_features": recipe.recognizable_visual_features
            }
        },
        "dicom": {
            "sop_class_uid": ENCAPSULATED_PDF_STORAGE_UID,
            "sop_class_name": "Encapsulated PDF Storage",
            "iod_name": "Encapsulated PDF",
            "modality": "DOC",
            "transfer_syntax_uid": EXPLICIT_VR_LITTLE_ENDIAN.uid,
            "transfer_syntax_name": EXPLICIT_VR_LITTLE_ENDIAN.name
        },
        "uids": {
            "study_instance_uid": study_instance_uid,
            "series_instance_uid": series_instance_uid,
            "sop_instance_uid": sop_instance_uid,
            "implementation_class_uid": implementation_class_uid
        },
        "image": Value::Null,
        "pixel_data": Value::Null,
        "references": [],
        "expected_capabilities": ["open_file", "read_metadata", "show_unsupported_but_recognized", "extract_encapsulated_document"],
        "expected_semantics": {
            "synthetic_data": "YES",
            "conversion_type": "SYN",
            "encapsulated_document": {
                "document_title": recipe.document_title,
                "mime_type": recipe.mime_type,
                "document_length": recipe.document_bytes.len(),
                "document_sha256": sha256_hex(recipe.document_bytes),
                "burned_in_annotation": recipe.burned_in_annotation,
                "recognizable_visual_features": recipe.recognizable_visual_features
            }
        },
        "expected_visual_checks": {
            "pattern": "minimal_single_page_pdf_document"
        },
        "validation": validation,
        "known_stressors": ["encapsulated_pdf_storage", "encapsulated_document_ob", "non_image_object", "document_extraction"],
        "standards_evidence": deduplicated_standards_evidence(standards_evidence)
    })
}

#[allow(clippy::too_many_arguments)]
fn enhanced_ct_manifest_entry(
    case: &Value,
    recipe: EnhancedCtRecipe,
    relative_path: &str,
    frames: u16,
    pixel_bytes: &[u8],
    pixel_values: &[i32],
    image_position_patient: &[&str],
    dimension_index_values: &[u32],
    study_instance_uid: &str,
    series_instance_uid: &str,
    sop_instance_uid: &str,
    frame_of_reference_uid: &str,
    dimension_organization_uid: &str,
    irradiation_event_uid: &str,
    implementation_class_uid: &str,
    concatenation: Option<EnhancedCtConcatenationManifest<'_>>,
    bytes: &[u8],
    validation: Value,
) -> Value {
    let standards_evidence = standards_evidence_from_case(case);
    let frame_byte_len = usize::from(recipe.rows) * usize::from(recipe.columns) * 2;
    let frame_hashes = pixel_bytes
        .chunks(frame_byte_len)
        .map(sha256_hex)
        .collect::<Vec<_>>();
    let concatenation_value = concatenation
        .map(|concatenation| {
            serde_json::json!({
                "concatenation_uid": concatenation.concatenation_uid,
                "in_concatenation_number": concatenation.in_concatenation_number,
                "in_concatenation_total_number": concatenation.in_concatenation_total_number,
                "concatenation_frame_offset_number": concatenation.concatenation_frame_offset_number,
                "sop_instance_uid_of_concatenation_source": concatenation.sop_instance_uid_of_concatenation_source
            })
        })
        .unwrap_or(Value::Null);
    let mut known_stressors = vec![
        "enhanced_ct_image_storage",
        "native_multiframe_pixel_data",
        "shared_functional_groups_sequence",
        "per_frame_functional_groups_sequence",
        "multi_frame_dimension",
    ];
    if !concatenation_value.is_null() {
        known_stressors.push("concatenation");
    }
    serde_json::json!({
        "case_id": recipe.case_id,
        "profile_membership": ["extended"],
        "path": relative_path,
        "sha256": sha256_hex(bytes),
        "size_bytes": bytes.len(),
        "determinism": "byte_stable",
        "recipe": {
            "recipe_id": recipe.recipe_id,
            "recipe_version": ENHANCED_CT_RECIPE_VERSION,
            "recipe_parameters": {
                "rows": recipe.rows,
                "columns": recipe.columns,
                "frames": frames,
                "samples_per_pixel": 1,
                "photometric_interpretation": "MONOCHROME2",
                "bits_allocated": 16,
                "bits_stored": 16,
                "high_bit": 15,
                "pixel_representation": 0,
                "pixel_values": pixel_values,
                "frame_type": recipe.frame_type,
                "dimension_index": {
                    "dimension_organization_uid": dimension_organization_uid,
                    "dimension_index_pointer": "ImagePositionPatient",
                    "functional_group_pointer": "PlanePositionSequence"
                },
                "shared_functional_groups": {
                    "pixel_measures": {
                        "pixel_spacing": recipe.pixel_spacing,
                        "slice_thickness": recipe.slice_thickness,
                        "spacing_between_slices": recipe.spacing_between_slices
                    },
                    "plane_orientation_patient": recipe.image_orientation_patient,
                    "frame_anatomy": {
                        "frame_laterality": "U",
                        "anatomic_region_code_value": "T-D3000"
                    },
                    "irradiation_event_uid": irradiation_event_uid,
                    "ct_pixel_value_transformation": {
                        "intercept": recipe.rescale_intercept,
                        "slope": recipe.rescale_slope,
                        "type": recipe.rescale_type
                    }
                },
                "per_frame_functional_groups": {
                    "image_position_patient": image_position_patient
                },
                "concatenation": concatenation_value
            }
        },
        "dicom": {
            "sop_class_uid": uids::ENHANCED_CT_IMAGE_STORAGE,
            "sop_class_name": "Enhanced CT Image Storage",
            "iod_name": "Enhanced CT Image",
            "modality": "CT",
            "transfer_syntax_uid": EXPLICIT_VR_LITTLE_ENDIAN.uid,
            "transfer_syntax_name": EXPLICIT_VR_LITTLE_ENDIAN.name
        },
        "uids": {
            "study_instance_uid": study_instance_uid,
            "series_instance_uid": series_instance_uid,
            "sop_instance_uid": sop_instance_uid,
            "frame_of_reference_uid": frame_of_reference_uid,
            "dimension_organization_uid": dimension_organization_uid,
            "irradiation_event_uid": irradiation_event_uid,
            "implementation_class_uid": implementation_class_uid
        },
        "image": {
            "rows": recipe.rows,
            "columns": recipe.columns,
            "frames": frames,
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
            "value_length": pixel_bytes.len(),
            "frame_count": frames,
            "frame_hashes": frame_hashes
        },
        "expected_capabilities": ["open_file", "read_metadata", "render_native_pixels", "parse_multiframe_functional_groups"],
        "expected_semantics": {
            "synthetic_data": "YES",
            "pixel_min": recipe.pixel_min,
            "pixel_max": recipe.pixel_max,
            "shared_functional_groups_sequence_items": 1,
            "per_frame_functional_groups_sequence_items": frames,
            "dimension_index_values": dimension_index_values,
            "concatenation": concatenation_value
        },
        "expected_visual_checks": {
            "pattern": if concatenation_value.is_null() {
                "two_frame_enhanced_ct_unsigned_gradient_stack"
            } else {
                "single_member_enhanced_ct_concatenation_gradient"
            }
        },
        "validation": validation,
        "known_stressors": known_stressors,
        "standards_evidence": deduplicated_standards_evidence(standards_evidence)
    })
}

fn write_enhanced_mr_case(
    run: &PreparedGenerationRun,
    case: &Value,
    recipe: EnhancedMrRecipe,
    standards_lock_sha256: &str,
) -> Result<GeneratedFile, GenerateError> {
    let study_instance_uid = deterministic_enhanced_mr_uid(
        standards_lock_sha256,
        recipe,
        run.seed,
        UidRole::StudyInstance,
    );
    let series_instance_uid = deterministic_enhanced_mr_uid(
        standards_lock_sha256,
        recipe,
        run.seed,
        UidRole::SeriesInstance,
    );
    let sop_instance_uid = deterministic_enhanced_mr_uid(
        standards_lock_sha256,
        recipe,
        run.seed,
        UidRole::SopInstance,
    );
    let frame_of_reference_uid = deterministic_enhanced_mr_uid(
        standards_lock_sha256,
        recipe,
        run.seed,
        UidRole::FrameOfReference,
    );
    let dimension_organization_uid = deterministic_enhanced_mr_uid(
        standards_lock_sha256,
        recipe,
        run.seed,
        UidRole::DimensionOrganization,
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
        uids::ENHANCED_MR_IMAGE_STORAGE,
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
    put_str(&mut obj, tags::STUDY_ID, VR::SH, "DTS-EMR");
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
    put_str(&mut obj, tags::DEVICE_SERIAL_NUMBER, VR::LO, "DTS-EMR-0001");
    put_str(
        &mut obj,
        tags::SOFTWARE_VERSIONS,
        VR::LO,
        crate::PACKAGE_VERSION,
    );

    put_str(&mut obj, tags::IMAGE_TYPE, VR::CS, recipe.frame_type);
    put_str(&mut obj, tags::INSTANCE_NUMBER, VR::IS, "1");
    put_str(&mut obj, tags::CONTENT_DATE, VR::DA, "20260101");
    put_str(&mut obj, tags::CONTENT_TIME, VR::TM, "000000");
    put_empty_sequence(&mut obj, tags::ACQUISITION_CONTEXT_SEQUENCE);
    put_str(
        &mut obj,
        tags::PIXEL_PRESENTATION,
        VR::CS,
        recipe.pixel_presentation,
    );
    put_str(
        &mut obj,
        tags::VOLUMETRIC_PROPERTIES,
        VR::CS,
        recipe.volumetric_properties,
    );
    put_str(
        &mut obj,
        tags::VOLUME_BASED_CALCULATION_TECHNIQUE,
        VR::CS,
        recipe.volume_based_calculation_technique,
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
    put_u16(&mut obj, tags::BITS_STORED, VR::US, 16);
    put_u16(&mut obj, tags::HIGH_BIT, VR::US, 15);
    put_u16(&mut obj, tags::PIXEL_REPRESENTATION, VR::US, 0);
    put_str(
        &mut obj,
        tags::NUMBER_OF_FRAMES,
        VR::IS,
        &recipe.frames.to_string(),
    );

    put_enhanced_mr_dimension_sequences(&mut obj, recipe, &dimension_organization_uid);
    put_enhanced_mr_functional_groups(&mut obj, recipe);

    obj.put(DataElement::new(
        tags::PIXEL_DATA,
        VR::OW,
        PrimitiveValue::from(recipe.pixel_bytes),
    ));

    let file_obj = obj
        .with_meta(
            FileMetaTableBuilder::new()
                .transfer_syntax(EXPLICIT_VR_LITTLE_ENDIAN.uid)
                .implementation_class_uid(&implementation_class_uid)
                .implementation_version_name(crate::IMPLEMENTATION_VERSION_NAME),
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
            sop_class_uid: uids::ENHANCED_MR_IMAGE_STORAGE,
            sop_instance_uid: &sop_instance_uid,
            transfer_syntax_uid: EXPLICIT_VR_LITTLE_ENDIAN.uid,
            implementation_class_uid: &implementation_class_uid,
            synthetic_data: "YES",
            rows: recipe.rows,
            columns: recipe.columns,
            frames: recipe.frames,
            samples_per_pixel: 1,
            photometric_interpretation: "MONOCHROME2",
            bits_allocated: 16,
            bits_stored: 16,
            high_bit: 15,
            pixel_representation: 0,
            planar_configuration: None,
            pixel_data_vr: VR::OW,
            pixel_data_length_formula: PixelDataLengthFormula::ContiguousSamples,
            decoded_frame_hashes: &[],
            palette: None,
            padding: None,
            ct_image: None,
            enhanced_ct_image: None,
            enhanced_mr_image: Some(EnhancedMrImageExpectations {
                modality: "MR",
                frame_of_reference_uid: &frame_of_reference_uid,
                image_type: recipe.frame_type,
                number_of_frames: recipe.frames,
                shared_functional_groups: 1,
                per_frame_functional_groups: recipe.frames as usize,
                dimension_organization_uid: &dimension_organization_uid,
                dimension_index_count: 1,
                pixel_spacing: recipe.pixel_spacing,
                image_orientation_patient: recipe.image_orientation_patient,
                image_position_patient: recipe.image_position_patient,
                frame_type: recipe.frame_type,
                pixel_presentation: recipe.pixel_presentation,
                volumetric_properties: recipe.volumetric_properties,
                volume_based_calculation_technique: recipe.volume_based_calculation_technique,
                rescale_intercept: recipe.rescale_intercept,
                rescale_slope: recipe.rescale_slope,
                rescale_type: recipe.rescale_type,
                repetition_time: recipe.repetition_time,
                flip_angle: recipe.flip_angle,
                echo_train_length: recipe.echo_train_length,
                rf_echo_train_length: recipe.rf_echo_train_length,
                gradient_echo_train_length: recipe.gradient_echo_train_length,
                effective_echo_times: recipe.effective_echo_times,
                temporal_position_time_offsets: recipe.temporal_position_time_offsets,
                velocity_encoding_directions: recipe.velocity_encoding_directions,
                velocity_encoding_minimum_value: recipe.velocity_encoding_minimum_value,
                velocity_encoding_maximum_value: recipe.velocity_encoding_maximum_value,
            }),
            mg_image: None,
            dx_image: None,
            us_image: None,
            cr_image: None,
            mr_image: None,
            segmentation: None,
        },
    )?;

    Ok(GeneratedFile {
        case_id: recipe.case_id.to_string(),
        manifest_entry: enhanced_mr_manifest_entry(
            case,
            recipe,
            &relative_path,
            &study_instance_uid,
            &series_instance_uid,
            &sop_instance_uid,
            &frame_of_reference_uid,
            &dimension_organization_uid,
            &implementation_class_uid,
            &validated.bytes,
            validated.validation,
        ),
    })
}

fn put_enhanced_mr_dimension_sequences(
    obj: &mut InMemDicomObject,
    recipe: EnhancedMrRecipe,
    dimension_organization_uid: &str,
) {
    let (dimension_index_pointer, functional_group_pointer, dimension_description_label) =
        if recipe.temporal_position_time_offsets.is_some() {
            (
                tags::TEMPORAL_POSITION_TIME_OFFSET,
                tags::TEMPORAL_POSITION_SEQUENCE,
                "TemporalPositionTimeOffset",
            )
        } else if recipe.velocity_encoding_directions.is_some() {
            (
                tags::VELOCITY_ENCODING_DIRECTION,
                tags::MR_VELOCITY_ENCODING_SEQUENCE,
                "VelocityEncodingDirection",
            )
        } else {
            (
                tags::EFFECTIVE_ECHO_TIME,
                tags::MR_ECHO_SEQUENCE,
                "EffectiveEchoTime",
            )
        };
    obj.put(DataElement::new(
        tags::DIMENSION_ORGANIZATION_SEQUENCE,
        VR::SQ,
        DataSetSequence::from(vec![InMemDicomObject::from_element_iter([
            DataElement::new(
                tags::DIMENSION_ORGANIZATION_UID,
                VR::UI,
                dimension_organization_uid,
            ),
        ])]),
    ));
    obj.put(DataElement::new(
        tags::DIMENSION_INDEX_SEQUENCE,
        VR::SQ,
        DataSetSequence::from(vec![InMemDicomObject::from_element_iter([
            DataElement::new(
                tags::DIMENSION_INDEX_POINTER,
                VR::AT,
                PrimitiveValue::Tags(vec![dimension_index_pointer].into()),
            ),
            DataElement::new(
                tags::FUNCTIONAL_GROUP_POINTER,
                VR::AT,
                PrimitiveValue::Tags(vec![functional_group_pointer].into()),
            ),
            DataElement::new(
                tags::DIMENSION_ORGANIZATION_UID,
                VR::UI,
                dimension_organization_uid,
            ),
            DataElement::new(
                tags::DIMENSION_DESCRIPTION_LABEL,
                VR::LO,
                dimension_description_label,
            ),
        ])]),
    ));
}

fn put_enhanced_mr_functional_groups(obj: &mut InMemDicomObject, recipe: EnhancedMrRecipe) {
    obj.put(DataElement::new(
        tags::SHARED_FUNCTIONAL_GROUPS_SEQUENCE,
        VR::SQ,
        DataSetSequence::from(vec![InMemDicomObject::from_element_iter([
            DataElement::new(
                tags::PIXEL_MEASURES_SEQUENCE,
                VR::SQ,
                DataSetSequence::from(vec![InMemDicomObject::from_element_iter([
                    DataElement::new(tags::PIXEL_SPACING, VR::DS, recipe.pixel_spacing),
                    DataElement::new(tags::SLICE_THICKNESS, VR::DS, recipe.slice_thickness),
                    DataElement::new(
                        tags::SPACING_BETWEEN_SLICES,
                        VR::DS,
                        recipe.spacing_between_slices,
                    ),
                ])]),
            ),
            DataElement::new(
                tags::PLANE_ORIENTATION_SEQUENCE,
                VR::SQ,
                DataSetSequence::from(vec![InMemDicomObject::from_element_iter([
                    DataElement::new(
                        tags::IMAGE_ORIENTATION_PATIENT,
                        VR::DS,
                        recipe.image_orientation_patient,
                    ),
                ])]),
            ),
            DataElement::new(
                tags::FRAME_ANATOMY_SEQUENCE,
                VR::SQ,
                DataSetSequence::from(vec![InMemDicomObject::from_element_iter([
                    DataElement::new(tags::FRAME_LATERALITY, VR::CS, "U"),
                    DataElement::new(
                        tags::ANATOMIC_REGION_SEQUENCE,
                        VR::SQ,
                        DataSetSequence::from(vec![InMemDicomObject::from_element_iter([
                            DataElement::new(tags::CODE_VALUE, VR::SH, "T-D1100"),
                            DataElement::new(tags::CODING_SCHEME_DESIGNATOR, VR::SH, "SRT"),
                            DataElement::new(tags::CODE_MEANING, VR::LO, "Head"),
                        ])]),
                    ),
                ])]),
            ),
            DataElement::new(
                tags::MR_IMAGE_FRAME_TYPE_SEQUENCE,
                VR::SQ,
                DataSetSequence::from(vec![InMemDicomObject::from_element_iter([
                    DataElement::new(tags::FRAME_TYPE, VR::CS, recipe.frame_type),
                    DataElement::new(tags::PIXEL_PRESENTATION, VR::CS, recipe.pixel_presentation),
                    DataElement::new(
                        tags::VOLUMETRIC_PROPERTIES,
                        VR::CS,
                        recipe.volumetric_properties,
                    ),
                    DataElement::new(
                        tags::VOLUME_BASED_CALCULATION_TECHNIQUE,
                        VR::CS,
                        recipe.volume_based_calculation_technique,
                    ),
                ])]),
            ),
            DataElement::new(
                tags::PIXEL_VALUE_TRANSFORMATION_SEQUENCE,
                VR::SQ,
                DataSetSequence::from(vec![InMemDicomObject::from_element_iter([
                    DataElement::new(tags::RESCALE_INTERCEPT, VR::DS, recipe.rescale_intercept),
                    DataElement::new(tags::RESCALE_SLOPE, VR::DS, recipe.rescale_slope),
                    DataElement::new(tags::RESCALE_TYPE, VR::LO, recipe.rescale_type),
                ])]),
            ),
            DataElement::new(
                tags::MR_TIMING_AND_RELATED_PARAMETERS_SEQUENCE,
                VR::SQ,
                DataSetSequence::from(vec![InMemDicomObject::from_element_iter([
                    DataElement::new(tags::REPETITION_TIME, VR::DS, recipe.repetition_time),
                    DataElement::new(tags::FLIP_ANGLE, VR::DS, recipe.flip_angle),
                    DataElement::new(tags::ECHO_TRAIN_LENGTH, VR::IS, recipe.echo_train_length),
                    DataElement::new(
                        tags::RF_ECHO_TRAIN_LENGTH,
                        VR::US,
                        PrimitiveValue::from(recipe.rf_echo_train_length),
                    ),
                    DataElement::new(
                        tags::GRADIENT_ECHO_TRAIN_LENGTH,
                        VR::US,
                        PrimitiveValue::from(recipe.gradient_echo_train_length),
                    ),
                ])]),
            ),
        ])]),
    ));

    let per_frame_items = if let Some(temporal_position_time_offsets) =
        recipe.temporal_position_time_offsets
    {
        recipe
            .image_position_patient
            .iter()
            .zip(temporal_position_time_offsets.iter())
            .enumerate()
            .map(
                |(index, (image_position_patient, temporal_position_time_offset))| {
                    InMemDicomObject::from_element_iter([
                        DataElement::new(
                            tags::FRAME_CONTENT_SEQUENCE,
                            VR::SQ,
                            DataSetSequence::from(vec![InMemDicomObject::from_element_iter([
                                DataElement::new(
                                    tags::DIMENSION_INDEX_VALUES,
                                    VR::UL,
                                    PrimitiveValue::from((index + 1) as u32),
                                ),
                                DataElement::new(
                                    tags::TEMPORAL_POSITION_INDEX,
                                    VR::UL,
                                    PrimitiveValue::from((index + 1) as u32),
                                ),
                                DataElement::new(
                                    tags::FRAME_ACQUISITION_NUMBER,
                                    VR::US,
                                    PrimitiveValue::from((index + 1) as u16),
                                ),
                            ])]),
                        ),
                        DataElement::new(
                            tags::PLANE_POSITION_SEQUENCE,
                            VR::SQ,
                            DataSetSequence::from(vec![InMemDicomObject::from_element_iter([
                                DataElement::new(
                                    tags::IMAGE_POSITION_PATIENT,
                                    VR::DS,
                                    *image_position_patient,
                                ),
                            ])]),
                        ),
                        DataElement::new(
                            tags::TEMPORAL_POSITION_SEQUENCE,
                            VR::SQ,
                            DataSetSequence::from(vec![InMemDicomObject::from_element_iter([
                                DataElement::new(
                                    tags::TEMPORAL_POSITION_TIME_OFFSET,
                                    VR::FD,
                                    PrimitiveValue::from(*temporal_position_time_offset),
                                ),
                            ])]),
                        ),
                    ])
                },
            )
            .collect::<Vec<_>>()
    } else if let Some(velocity_encoding_directions) = recipe.velocity_encoding_directions {
        let velocity_encoding_minimum_value = recipe
            .velocity_encoding_minimum_value
            .expect("Enhanced MR velocity encoding recipes must define a minimum value");
        let velocity_encoding_maximum_value = recipe
            .velocity_encoding_maximum_value
            .expect("Enhanced MR velocity encoding recipes must define a maximum value");
        recipe
            .image_position_patient
            .iter()
            .zip(velocity_encoding_directions.iter())
            .enumerate()
            .map(
                |(index, (image_position_patient, velocity_encoding_direction))| {
                    InMemDicomObject::from_element_iter([
                        DataElement::new(
                            tags::FRAME_CONTENT_SEQUENCE,
                            VR::SQ,
                            DataSetSequence::from(vec![InMemDicomObject::from_element_iter([
                                DataElement::new(
                                    tags::DIMENSION_INDEX_VALUES,
                                    VR::UL,
                                    PrimitiveValue::from((index + 1) as u32),
                                ),
                                DataElement::new(
                                    tags::FRAME_ACQUISITION_NUMBER,
                                    VR::US,
                                    PrimitiveValue::from((index + 1) as u16),
                                ),
                            ])]),
                        ),
                        DataElement::new(
                            tags::PLANE_POSITION_SEQUENCE,
                            VR::SQ,
                            DataSetSequence::from(vec![InMemDicomObject::from_element_iter([
                                DataElement::new(
                                    tags::IMAGE_POSITION_PATIENT,
                                    VR::DS,
                                    *image_position_patient,
                                ),
                            ])]),
                        ),
                        DataElement::new(
                            tags::MR_VELOCITY_ENCODING_SEQUENCE,
                            VR::SQ,
                            DataSetSequence::from(vec![InMemDicomObject::from_element_iter([
                                DataElement::new(
                                    tags::VELOCITY_ENCODING_DIRECTION,
                                    VR::FD,
                                    PrimitiveValue::from(*velocity_encoding_direction),
                                ),
                                DataElement::new(
                                    tags::VELOCITY_ENCODING_MINIMUM_VALUE,
                                    VR::FD,
                                    PrimitiveValue::from(velocity_encoding_minimum_value),
                                ),
                                DataElement::new(
                                    tags::VELOCITY_ENCODING_MAXIMUM_VALUE,
                                    VR::FD,
                                    PrimitiveValue::from(velocity_encoding_maximum_value),
                                ),
                            ])]),
                        ),
                    ])
                },
            )
            .collect::<Vec<_>>()
    } else {
        let effective_echo_times = recipe.effective_echo_times.expect(
            "Enhanced MR recipes without Temporal Position offsets must define Effective Echo Times",
        );
        recipe
            .image_position_patient
            .iter()
            .zip(effective_echo_times.iter())
            .enumerate()
            .map(|(index, (image_position_patient, effective_echo_time))| {
                InMemDicomObject::from_element_iter([
                    DataElement::new(
                        tags::FRAME_CONTENT_SEQUENCE,
                        VR::SQ,
                        DataSetSequence::from(vec![InMemDicomObject::from_element_iter([
                            DataElement::new(
                                tags::DIMENSION_INDEX_VALUES,
                                VR::UL,
                                PrimitiveValue::from((index + 1) as u32),
                            ),
                            DataElement::new(
                                tags::FRAME_ACQUISITION_NUMBER,
                                VR::US,
                                PrimitiveValue::from((index + 1) as u16),
                            ),
                        ])]),
                    ),
                    DataElement::new(
                        tags::PLANE_POSITION_SEQUENCE,
                        VR::SQ,
                        DataSetSequence::from(vec![InMemDicomObject::from_element_iter([
                            DataElement::new(
                                tags::IMAGE_POSITION_PATIENT,
                                VR::DS,
                                *image_position_patient,
                            ),
                        ])]),
                    ),
                    DataElement::new(
                        tags::MR_ECHO_SEQUENCE,
                        VR::SQ,
                        DataSetSequence::from(vec![InMemDicomObject::from_element_iter([
                            DataElement::new(
                                tags::EFFECTIVE_ECHO_TIME,
                                VR::FD,
                                PrimitiveValue::from(*effective_echo_time),
                            ),
                        ])]),
                    ),
                ])
            })
            .collect::<Vec<_>>()
    };
    obj.put(DataElement::new(
        tags::PER_FRAME_FUNCTIONAL_GROUPS_SEQUENCE,
        VR::SQ,
        DataSetSequence::from(per_frame_items),
    ));
}

#[allow(clippy::too_many_arguments)]
fn enhanced_mr_manifest_entry(
    case: &Value,
    recipe: EnhancedMrRecipe,
    relative_path: &str,
    study_instance_uid: &str,
    series_instance_uid: &str,
    sop_instance_uid: &str,
    frame_of_reference_uid: &str,
    dimension_organization_uid: &str,
    implementation_class_uid: &str,
    bytes: &[u8],
    validation: Value,
) -> Value {
    let standards_evidence = standards_evidence_from_case(case);
    let (
        dimension_index_pointer,
        functional_group_pointer,
        per_frame_dimension_name,
        per_frame_dimension_values,
        visual_pattern,
        dimension_stressor,
    ) = if let Some(temporal_position_time_offsets) = recipe.temporal_position_time_offsets {
        (
            "TemporalPositionTimeOffset",
            "TemporalPositionSequence",
            "temporal_position_time_offset",
            serde_json::json!(temporal_position_time_offsets),
            "two_frame_enhanced_mr_temporal_gradient_stack",
            "per_frame_temporal_position",
        )
    } else if let Some(velocity_encoding_directions) = recipe.velocity_encoding_directions {
        (
            "VelocityEncodingDirection",
            "MRVelocityEncodingSequence",
            "velocity_encoding_direction",
            serde_json::json!(velocity_encoding_directions),
            "two_frame_enhanced_mr_phase_velocity_encoding_stack",
            "per_frame_mr_velocity_encoding",
        )
    } else {
        (
            "EffectiveEchoTime",
            "MREchoSequence",
            "effective_echo_time",
            serde_json::json!(
                recipe
                    .effective_echo_times
                    .expect("Enhanced MR echo case must define Effective Echo Times")
            ),
            "two_frame_enhanced_mr_echo_gradient_stack",
            "per_frame_mr_echo",
        )
    };
    let mut per_frame_functional_groups = serde_json::json!({
        "image_position_patient": recipe.image_position_patient
    });
    per_frame_functional_groups[per_frame_dimension_name] = per_frame_dimension_values.clone();
    if recipe.velocity_encoding_directions.is_some() {
        per_frame_functional_groups["velocity_encoding_minimum_value"] =
            serde_json::json!(recipe.velocity_encoding_minimum_value);
        per_frame_functional_groups["velocity_encoding_maximum_value"] =
            serde_json::json!(recipe.velocity_encoding_maximum_value);
    }
    let mut expected_semantics = serde_json::json!({
        "synthetic_data": "YES",
        "pixel_min": recipe.pixel_min,
        "pixel_max": recipe.pixel_max,
        "shared_functional_groups_sequence_items": 1,
        "per_frame_functional_groups_sequence_items": recipe.frames,
        "dimension_index_values": [1, 2]
    });
    expected_semantics[per_frame_dimension_name] = per_frame_dimension_values;
    if recipe.velocity_encoding_directions.is_some() {
        expected_semantics["velocity_encoding_minimum_value"] =
            serde_json::json!(recipe.velocity_encoding_minimum_value);
        expected_semantics["velocity_encoding_maximum_value"] =
            serde_json::json!(recipe.velocity_encoding_maximum_value);
    }
    let known_stressors = [
        "enhanced_mr_image_storage",
        "native_multiframe_pixel_data",
        "shared_functional_groups_sequence",
        "per_frame_functional_groups_sequence",
        dimension_stressor,
        "multi_frame_dimension",
    ];
    serde_json::json!({
        "case_id": recipe.case_id,
        "profile_membership": ["extended"],
        "path": relative_path,
        "sha256": sha256_hex(bytes),
        "size_bytes": bytes.len(),
        "determinism": "byte_stable",
        "recipe": {
            "recipe_id": recipe.recipe_id,
            "recipe_version": ENHANCED_MR_RECIPE_VERSION,
            "recipe_parameters": {
                "rows": recipe.rows,
                "columns": recipe.columns,
                "frames": recipe.frames,
                "samples_per_pixel": 1,
                "photometric_interpretation": "MONOCHROME2",
                "bits_allocated": 16,
                "bits_stored": 16,
                "high_bit": 15,
                "pixel_representation": 0,
                "pixel_values": recipe.pixel_values,
                "frame_type": recipe.frame_type,
                "dimension_index": {
                    "dimension_organization_uid": dimension_organization_uid,
                    "dimension_index_pointer": dimension_index_pointer,
                    "functional_group_pointer": functional_group_pointer
                },
                "shared_functional_groups": {
                    "pixel_measures": {
                        "pixel_spacing": recipe.pixel_spacing,
                        "slice_thickness": recipe.slice_thickness,
                        "spacing_between_slices": recipe.spacing_between_slices
                    },
                    "plane_orientation_patient": recipe.image_orientation_patient,
                    "frame_anatomy": {
                        "frame_laterality": "U",
                        "anatomic_region_code_value": "T-D1100"
                    },
                    "mr_timing": {
                        "repetition_time": recipe.repetition_time,
                        "flip_angle": recipe.flip_angle,
                        "echo_train_length": recipe.echo_train_length,
                        "rf_echo_train_length": recipe.rf_echo_train_length,
                        "gradient_echo_train_length": recipe.gradient_echo_train_length
                    },
                    "pixel_value_transformation": {
                        "intercept": recipe.rescale_intercept,
                        "slope": recipe.rescale_slope,
                        "type": recipe.rescale_type
                    }
                },
                "per_frame_functional_groups": per_frame_functional_groups
            }
        },
        "dicom": {
            "sop_class_uid": uids::ENHANCED_MR_IMAGE_STORAGE,
            "sop_class_name": "Enhanced MR Image Storage",
            "iod_name": "Enhanced MR Image",
            "modality": "MR",
            "transfer_syntax_uid": EXPLICIT_VR_LITTLE_ENDIAN.uid,
            "transfer_syntax_name": EXPLICIT_VR_LITTLE_ENDIAN.name
        },
        "uids": {
            "study_instance_uid": study_instance_uid,
            "series_instance_uid": series_instance_uid,
            "sop_instance_uid": sop_instance_uid,
            "frame_of_reference_uid": frame_of_reference_uid,
            "dimension_organization_uid": dimension_organization_uid,
            "implementation_class_uid": implementation_class_uid
        },
        "image": {
            "rows": recipe.rows,
            "columns": recipe.columns,
            "frames": recipe.frames,
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
            "value_length": recipe.pixel_bytes.len(),
            "frame_count": recipe.frames,
            "frame_hashes": [
                sha256_hex(&recipe.pixel_bytes[0..8]),
                sha256_hex(&recipe.pixel_bytes[8..16])
            ]
        },
        "expected_capabilities": ["open_file", "read_metadata", "render_native_pixels", "parse_multiframe_functional_groups"],
        "expected_semantics": expected_semantics,
        "expected_visual_checks": {
            "pattern": visual_pattern
        },
        "validation": validation,
        "known_stressors": known_stressors,
        "standards_evidence": deduplicated_standards_evidence(standards_evidence)
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

    let compressed_pixel_data = if recipe.transfer_syntax == RLE_LOSSLESS {
        let rle_encoder = NativeRleLosslessEncoder::new();
        let encoded_frame = rle_encoder
            .encode_frame(FrameEncodeInput {
                native_frame: recipe.pixel_bytes,
                rows: recipe.rows,
                columns: recipe.columns,
                samples_per_pixel: 1,
                bits_allocated: 16,
                bits_stored: 12,
                photometric_interpretation: recipe.photometric_interpretation,
            })
            .map_err(|err| GenerateError::WriteDicomFile {
                path: path.clone(),
                message: err.to_string(),
            })?;
        let compressed_frames = vec![encoded_frame.bytes];
        let encapsulated = EncapsulatedPixelData::one_fragment_per_frame(
            &compressed_frames,
            BasicOffsetTablePolicy::Populated,
        )
        .map_err(|err| GenerateError::WriteDicomFile {
            path: path.clone(),
            message: err.to_string(),
        })?;
        obj.put(DataElement::new(
            tags::PIXEL_DATA,
            VR::OB,
            PixelFragmentSequence::new(
                encapsulated.basic_offset_table.offsets.clone(),
                compressed_frames,
            ),
        ));
        Some((FrameEncoder::backend(&rle_encoder), encapsulated))
    } else {
        obj.put(DataElement::new(
            tags::PIXEL_DATA,
            VR::OW,
            PrimitiveValue::from(recipe.pixel_bytes),
        ));
        None
    };

    let file_obj = obj
        .with_meta(
            FileMetaTableBuilder::new()
                .transfer_syntax(recipe.transfer_syntax.uid)
                .implementation_class_uid(&implementation_class_uid)
                .implementation_version_name(crate::IMPLEMENTATION_VERSION_NAME),
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

    let decoded_frame_hash = sha256_hex(recipe.pixel_bytes);
    let decoded_frame_hashes = [decoded_frame_hash.as_str()];
    let validated = validate_part10_file(
        &path,
        &Part10Expectations {
            sop_class_uid: recipe.sop_class_uid,
            sop_instance_uid: &sop_instance_uid,
            transfer_syntax_uid: recipe.transfer_syntax.uid,
            implementation_class_uid: &implementation_class_uid,
            synthetic_data: "YES",
            rows: recipe.rows,
            columns: recipe.columns,
            frames: 1,
            samples_per_pixel: 1,
            photometric_interpretation: recipe.photometric_interpretation,
            bits_allocated: 16,
            bits_stored: 12,
            high_bit: 11,
            pixel_representation: 0,
            planar_configuration: None,
            pixel_data_vr: if compressed_pixel_data.is_some() {
                VR::OB
            } else {
                VR::OW
            },
            pixel_data_length_formula: compressed_pixel_data
                .as_ref()
                .map(|(_, encapsulated)| PixelDataLengthFormula::Encapsulated {
                    fragments: encapsulated.fragments.len(),
                    basic_offset_table_offsets: encapsulated.basic_offset_table.offsets.len(),
                })
                .unwrap_or(PixelDataLengthFormula::ContiguousSamples),
            decoded_frame_hashes: if compressed_pixel_data.is_some() {
                &decoded_frame_hashes
            } else {
                &[]
            },
            palette: None,
            padding: None,
            ct_image: None,
            enhanced_ct_image: None,
            enhanced_mr_image: None,
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
            us_image: None,
            cr_image: None,
            mr_image: None,
            segmentation: None,
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
            compressed_pixel_data.as_ref(),
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
    compressed_pixel_data: Option<&(crate::codecs::CodecBackendInfo, EncapsulatedPixelData)>,
) -> Value {
    let mut standards_evidence = standards_evidence_from_case(case);
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

    if recipe.transfer_syntax == RLE_LOSSLESS {
        standards_evidence.extend([
            serde_json::json!({
                "source": "dicom-standard-kb",
                "edition": "2026b",
                "query": "lookup_uid RLELossless",
                "covered": true,
                "part": "PS3.6",
                "anchor": "table_A-1"
            }),
            serde_json::json!({
                "source": "dicom-standard-kb",
                "edition": "2026b",
                "query": "search_standard_text PS3.5 RLE Lossless image compression Photometric Interpretation Bits Allocated",
                "covered": true,
                "part": "PS3.5",
                "anchor": "sect_8.2.2"
            }),
            serde_json::json!({
                "source": "dicom-standard-kb",
                "edition": "2026b",
                "query": "retrieve_standard_text PS3.5 sect_A.4",
                "covered": true,
                "part": "PS3.5",
                "anchor": "sect_A.4"
            }),
        ]);
    }

    let window_manifest = serde_json::json!({
        "center": recipe.window_center,
        "width": recipe.window_width
    });
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
    let frame_hash = sha256_hex(recipe.pixel_bytes);
    let pixel_data_manifest = if let Some((backend, encapsulated)) = compressed_pixel_data {
        serde_json::json!({
            "vr": "OB",
            "native_or_encapsulated": "encapsulated",
            "value_length": Value::Null,
            "frame_count": 1,
            "frame_hashes": [frame_hash],
            "codec": {
                "backend_id": backend.backend_id,
                "backend_kind": backend.backend_kind.as_str(),
                "display_name": backend.display_name,
                "version": backend.version,
                "transfer_syntax_uid": backend.transfer_syntax_uid,
                "feature_gate": backend.feature_gate,
                "determinism": backend.determinism.as_str()
            },
            "encapsulated_pixel_data": {
                "basic_offset_table": {
                    "present": true,
                    "populated": encapsulated.basic_offset_table.is_populated(),
                    "offset_count": encapsulated.basic_offset_table.offsets.len(),
                    "offsets": encapsulated.basic_offset_table.offsets.clone()
                },
                "fragments_per_frame": encapsulated.fragments_per_frame.clone(),
                "fragments": encapsulated.fragments.iter().map(|fragment| {
                    serde_json::json!({
                        "frame_index": fragment.frame_index,
                        "item_start_offset": fragment.item_start_offset,
                        "compressed_length": fragment.compressed_length,
                        "padded_length": fragment.padded_length
                    })
                }).collect::<Vec<_>>(),
                "extended_offset_table": {
                    "present": false,
                    "lengths_present": false,
                    "offset_count": 0,
                    "length_count": 0
                },
                "compressed_frame_hashes": encapsulated.compressed_frame_hashes.clone()
            }
        })
    } else {
        serde_json::json!({
            "vr": "OW",
            "native_or_encapsulated": "native",
            "value_length": recipe.pixel_bytes.len(),
            "frame_count": 1,
            "frame_hashes": [frame_hash]
        })
    };

    serde_json::json!({
        "case_id": recipe.case_id,
        "profile_membership": classic_mg_profile_membership(recipe),
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
                "body_part_examined": "BREAST",
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
            "transfer_syntax_uid": recipe.transfer_syntax.uid,
            "transfer_syntax_name": recipe.transfer_syntax.name
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
        "pixel_data": pixel_data_manifest,
        "expected_capabilities": classic_mg_expected_capabilities(recipe),
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
        "known_stressors": classic_mg_known_stressors(recipe),
        "standards_evidence": deduplicated_standards_evidence(standards_evidence)
    })
}

fn classic_mg_profile_membership(recipe: ClassicMgRecipe) -> &'static [&'static str] {
    if recipe.transfer_syntax == RLE_LOSSLESS {
        &["extended"]
    } else {
        &["core"]
    }
}

fn classic_mg_expected_capabilities(recipe: ClassicMgRecipe) -> Vec<&'static str> {
    let mut capabilities = vec!["open_file", "read_metadata"];
    if recipe.transfer_syntax == RLE_LOSSLESS {
        capabilities.push("decode_rle_lossless_pixels");
    } else {
        capabilities.push("render_native_pixels");
    }
    if recipe.window_center.is_some() {
        capabilities.push("apply_window");
    }
    capabilities
}

fn classic_mg_known_stressors(recipe: ClassicMgRecipe) -> Vec<&'static str> {
    let mut stressors = if recipe.presentation_intent_type == "FOR PROCESSING" {
        vec![
            "digital_mammography_for_processing",
            "mono2_processing_pixels",
            "unsigned_12_bit_pixels",
        ]
    } else {
        vec![
            "digital_mammography_for_presentation",
            "mono1_inversion",
            "unsigned_12_bit_pixels",
            "presentation_lut_inverse",
        ]
    };
    if recipe.transfer_syntax == IMPLICIT_VR_LITTLE_ENDIAN {
        stressors.push("implicit_vr_little_endian");
    }
    if recipe.transfer_syntax == RLE_LOSSLESS {
        stressors.extend([
            "encapsulated_pixel_data",
            "rle_lossless_transfer_syntax",
            "compressed_modality_pixels",
        ]);
    }
    stressors
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

    let compressed_pixel_data = if recipe.transfer_syntax == RLE_LOSSLESS {
        let rle_encoder = NativeRleLosslessEncoder::new();
        let encoded_frame = rle_encoder
            .encode_frame(FrameEncodeInput {
                native_frame: recipe.pixel_bytes,
                rows: recipe.rows,
                columns: recipe.columns,
                samples_per_pixel: 1,
                bits_allocated: 16,
                bits_stored: 12,
                photometric_interpretation: "MONOCHROME2",
            })
            .map_err(|err| GenerateError::WriteDicomFile {
                path: path.clone(),
                message: err.to_string(),
            })?;
        let compressed_frames = vec![encoded_frame.bytes];
        let encapsulated = EncapsulatedPixelData::one_fragment_per_frame(
            &compressed_frames,
            BasicOffsetTablePolicy::Populated,
        )
        .map_err(|err| GenerateError::WriteDicomFile {
            path: path.clone(),
            message: err.to_string(),
        })?;
        obj.put(DataElement::new(
            tags::PIXEL_DATA,
            VR::OB,
            PixelFragmentSequence::new(
                encapsulated.basic_offset_table.offsets.clone(),
                compressed_frames,
            ),
        ));
        Some((FrameEncoder::backend(&rle_encoder), encapsulated))
    } else {
        obj.put(DataElement::new(
            tags::PIXEL_DATA,
            VR::OW,
            PrimitiveValue::from(recipe.pixel_bytes),
        ));
        None
    };

    let file_obj = obj
        .with_meta(
            FileMetaTableBuilder::new()
                .transfer_syntax(recipe.transfer_syntax.uid)
                .implementation_class_uid(&implementation_class_uid)
                .implementation_version_name(crate::IMPLEMENTATION_VERSION_NAME),
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

    let decoded_frame_hash = sha256_hex(recipe.pixel_bytes);
    let decoded_frame_hashes = [decoded_frame_hash.as_str()];
    let validated = validate_part10_file(
        &path,
        &Part10Expectations {
            sop_class_uid: uids::DIGITAL_X_RAY_IMAGE_STORAGE_FOR_PRESENTATION,
            sop_instance_uid: &sop_instance_uid,
            transfer_syntax_uid: recipe.transfer_syntax.uid,
            implementation_class_uid: &implementation_class_uid,
            synthetic_data: "YES",
            rows: recipe.rows,
            columns: recipe.columns,
            frames: 1,
            samples_per_pixel: 1,
            photometric_interpretation: "MONOCHROME2",
            bits_allocated: 16,
            bits_stored: 12,
            high_bit: 11,
            pixel_representation: 0,
            planar_configuration: None,
            pixel_data_vr: if compressed_pixel_data.is_some() {
                VR::OB
            } else {
                VR::OW
            },
            pixel_data_length_formula: compressed_pixel_data
                .as_ref()
                .map(|(_, encapsulated)| PixelDataLengthFormula::Encapsulated {
                    fragments: encapsulated.fragments.len(),
                    basic_offset_table_offsets: encapsulated.basic_offset_table.offsets.len(),
                })
                .unwrap_or(PixelDataLengthFormula::ContiguousSamples),
            decoded_frame_hashes: if compressed_pixel_data.is_some() {
                &decoded_frame_hashes
            } else {
                &[]
            },
            palette: None,
            padding: None,
            ct_image: None,
            enhanced_ct_image: None,
            enhanced_mr_image: None,
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
            us_image: None,
            cr_image: None,
            mr_image: None,
            segmentation: None,
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
            compressed_pixel_data.as_ref(),
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
    compressed_pixel_data: Option<&(crate::codecs::CodecBackendInfo, EncapsulatedPixelData)>,
) -> Value {
    let mut standards_evidence = standards_evidence_from_case(case);
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

    if recipe.transfer_syntax == RLE_LOSSLESS {
        standards_evidence.extend([
            serde_json::json!({
                "source": "dicom-standard-kb",
                "edition": "2026b",
                "query": "lookup_uid RLELossless",
                "covered": true,
                "part": "PS3.6",
                "anchor": "table_A-1"
            }),
            serde_json::json!({
                "source": "dicom-standard-kb",
                "edition": "2026b",
                "query": "retrieve_standard_text PS3.5 sect_8.2.2",
                "covered": true,
                "part": "PS3.5",
                "anchor": "sect_8.2.2"
            }),
            serde_json::json!({
                "source": "dicom-standard-kb",
                "edition": "2026b",
                "query": "retrieve_standard_text PS3.5 sect_A.4",
                "covered": true,
                "part": "PS3.5",
                "anchor": "sect_A.4"
            }),
        ]);
    }

    let frame_hash = sha256_hex(recipe.pixel_bytes);
    let pixel_data_manifest = if let Some((backend, encapsulated)) = compressed_pixel_data {
        serde_json::json!({
            "vr": "OB",
            "native_or_encapsulated": "encapsulated",
            "value_length": Value::Null,
            "frame_count": 1,
            "frame_hashes": [frame_hash],
            "codec": {
                "backend_id": backend.backend_id,
                "backend_kind": backend.backend_kind.as_str(),
                "display_name": backend.display_name,
                "version": backend.version,
                "transfer_syntax_uid": backend.transfer_syntax_uid,
                "feature_gate": backend.feature_gate,
                "determinism": backend.determinism.as_str()
            },
            "encapsulated_pixel_data": {
                "basic_offset_table": {
                    "present": true,
                    "populated": encapsulated.basic_offset_table.is_populated(),
                    "offset_count": encapsulated.basic_offset_table.offsets.len(),
                    "offsets": encapsulated.basic_offset_table.offsets.clone()
                },
                "fragments_per_frame": encapsulated.fragments_per_frame.clone(),
                "fragments": encapsulated.fragments.iter().map(|fragment| {
                    serde_json::json!({
                        "frame_index": fragment.frame_index,
                        "item_start_offset": fragment.item_start_offset,
                        "compressed_length": fragment.compressed_length,
                        "padded_length": fragment.padded_length
                    })
                }).collect::<Vec<_>>(),
                "extended_offset_table": {
                    "present": false,
                    "lengths_present": false,
                    "offset_count": 0,
                    "length_count": 0
                },
                "compressed_frame_hashes": encapsulated.compressed_frame_hashes.clone()
            }
        })
    } else {
        serde_json::json!({
            "vr": "OW",
            "native_or_encapsulated": "native",
            "value_length": recipe.pixel_bytes.len(),
            "frame_count": 1,
            "frame_hashes": [frame_hash]
        })
    };

    serde_json::json!({
        "case_id": recipe.case_id,
        "profile_membership": classic_dx_profile_membership(recipe),
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
                "body_part_examined": "CHEST",
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
            "transfer_syntax_uid": recipe.transfer_syntax.uid,
            "transfer_syntax_name": recipe.transfer_syntax.name
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
        "pixel_data": pixel_data_manifest,
        "expected_capabilities": classic_dx_expected_capabilities(recipe),
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
        "known_stressors": classic_dx_known_stressors(recipe),
        "standards_evidence": deduplicated_standards_evidence(standards_evidence)
    })
}

fn classic_dx_profile_membership(recipe: ClassicDxRecipe) -> &'static [&'static str] {
    if recipe.transfer_syntax == RLE_LOSSLESS {
        &["extended"]
    } else {
        &["core"]
    }
}

fn classic_dx_expected_capabilities(recipe: ClassicDxRecipe) -> Vec<&'static str> {
    let mut capabilities = vec!["open_file", "read_metadata"];
    if recipe.transfer_syntax == RLE_LOSSLESS {
        capabilities.push("decode_rle_lossless_pixels");
    } else {
        capabilities.push("render_native_pixels");
    }
    capabilities.extend(["apply_window", "apply_display_shutter"]);
    capabilities
}

fn classic_dx_known_stressors(recipe: ClassicDxRecipe) -> Vec<&'static str> {
    let mut stressors = vec![
        "digital_x_ray_for_presentation",
        "display_shutter",
        "unsigned_12_bit_pixels",
        "voi_window",
    ];
    if recipe.transfer_syntax == RLE_LOSSLESS {
        stressors.extend([
            "encapsulated_pixel_data",
            "rle_lossless_transfer_syntax",
            "compressed_modality_pixels",
        ]);
    }
    stressors
}

fn write_classic_us_case(
    run: &PreparedGenerationRun,
    case: &Value,
    recipe: ClassicUsRecipe,
    standards_lock_sha256: &str,
) -> Result<GeneratedFile, GenerateError> {
    let study_instance_uid = deterministic_classic_us_uid(
        standards_lock_sha256,
        recipe,
        run.seed,
        UidRole::StudyInstance,
    );
    let series_instance_uid = deterministic_classic_us_uid(
        standards_lock_sha256,
        recipe,
        run.seed,
        UidRole::SeriesInstance,
    );
    let sop_instance_uid = deterministic_classic_us_uid(
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
        uids::ULTRASOUND_IMAGE_STORAGE,
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
    put_str(&mut obj, tags::STUDY_ID, VR::SH, "DTS-US");
    put_str(&mut obj, tags::ACCESSION_NUMBER, VR::SH, "");

    put_str(&mut obj, tags::MODALITY, VR::CS, "US");
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
    put_str(
        &mut obj,
        tags::LOSSY_IMAGE_COMPRESSION,
        VR::CS,
        recipe.lossy_image_compression,
    );
    put_u16(
        &mut obj,
        tags::ULTRASOUND_COLOR_DATA_PRESENT,
        VR::US,
        recipe.ultrasound_color_data_present,
    );

    let compressed_pixel_data = if recipe.transfer_syntax == RLE_LOSSLESS {
        let rle_encoder = NativeRleLosslessEncoder::new();
        let encoded_frame = rle_encoder
            .encode_frame(FrameEncodeInput {
                native_frame: recipe.pixel_bytes,
                rows: recipe.rows,
                columns: recipe.columns,
                samples_per_pixel: 1,
                bits_allocated: 8,
                bits_stored: 8,
                photometric_interpretation: "MONOCHROME2",
            })
            .map_err(|err| GenerateError::WriteDicomFile {
                path: path.clone(),
                message: err.to_string(),
            })?;
        let compressed_frames = vec![encoded_frame.bytes];
        let encapsulated = EncapsulatedPixelData::one_fragment_per_frame(
            &compressed_frames,
            BasicOffsetTablePolicy::Populated,
        )
        .map_err(|err| GenerateError::WriteDicomFile {
            path: path.clone(),
            message: err.to_string(),
        })?;
        obj.put(DataElement::new(
            tags::PIXEL_DATA,
            VR::OB,
            PixelFragmentSequence::new(
                encapsulated.basic_offset_table.offsets.clone(),
                compressed_frames,
            ),
        ));
        Some((FrameEncoder::backend(&rle_encoder), encapsulated))
    } else {
        obj.put(DataElement::new(
            tags::PIXEL_DATA,
            VR::OB,
            PrimitiveValue::from(recipe.pixel_bytes),
        ));
        None
    };

    let file_obj = obj
        .with_meta(
            FileMetaTableBuilder::new()
                .transfer_syntax(recipe.transfer_syntax.uid)
                .implementation_class_uid(&implementation_class_uid)
                .implementation_version_name(crate::IMPLEMENTATION_VERSION_NAME),
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

    let decoded_frame_hash = sha256_hex(recipe.pixel_bytes);
    let decoded_frame_hashes = [decoded_frame_hash.as_str()];
    let validated = validate_part10_file(
        &path,
        &Part10Expectations {
            sop_class_uid: uids::ULTRASOUND_IMAGE_STORAGE,
            sop_instance_uid: &sop_instance_uid,
            transfer_syntax_uid: recipe.transfer_syntax.uid,
            implementation_class_uid: &implementation_class_uid,
            synthetic_data: "YES",
            rows: recipe.rows,
            columns: recipe.columns,
            frames: 1,
            samples_per_pixel: 1,
            photometric_interpretation: "MONOCHROME2",
            bits_allocated: 8,
            bits_stored: 8,
            high_bit: 7,
            pixel_representation: 0,
            planar_configuration: None,
            pixel_data_vr: VR::OB,
            pixel_data_length_formula: compressed_pixel_data
                .as_ref()
                .map(|(_, encapsulated)| PixelDataLengthFormula::Encapsulated {
                    fragments: encapsulated.fragments.len(),
                    basic_offset_table_offsets: encapsulated.basic_offset_table.offsets.len(),
                })
                .unwrap_or(PixelDataLengthFormula::ContiguousSamples),
            decoded_frame_hashes: if compressed_pixel_data.is_some() {
                &decoded_frame_hashes
            } else {
                &[]
            },
            palette: None,
            padding: None,
            ct_image: None,
            enhanced_ct_image: None,
            enhanced_mr_image: None,
            mg_image: None,
            dx_image: None,
            us_image: Some(UsImageExpectations {
                modality: "US",
                image_type: "ORIGINAL\\PRIMARY",
                lossy_image_compression: recipe.lossy_image_compression,
                ultrasound_color_data_present: recipe.ultrasound_color_data_present,
            }),
            cr_image: None,
            mr_image: None,
            segmentation: None,
        },
    )?;

    Ok(GeneratedFile {
        case_id: recipe.case_id.to_string(),
        manifest_entry: classic_us_manifest_entry(
            case,
            recipe,
            &relative_path,
            &study_instance_uid,
            &series_instance_uid,
            &sop_instance_uid,
            &implementation_class_uid,
            &validated.bytes,
            validated.validation,
            compressed_pixel_data.as_ref(),
        ),
    })
}

#[allow(clippy::too_many_arguments)]
fn classic_us_manifest_entry(
    case: &Value,
    recipe: ClassicUsRecipe,
    relative_path: &str,
    study_instance_uid: &str,
    series_instance_uid: &str,
    sop_instance_uid: &str,
    implementation_class_uid: &str,
    bytes: &[u8],
    validation: Value,
    compressed_pixel_data: Option<&(crate::codecs::CodecBackendInfo, EncapsulatedPixelData)>,
) -> Value {
    let mut standards_evidence = standards_evidence_from_case(case);
    standards_evidence.extend([
        serde_json::json!({
            "source": "dicom-standard-kb",
            "edition": "2026b",
            "query": "lookup_iod Ultrasound Image",
            "covered": true,
            "part": "PS3.3",
            "anchor": "table_A.6-1"
        }),
        serde_json::json!({
            "source": "dicom-standard-kb",
            "edition": "2026b",
            "query": "list_modules_for_iod Ultrasound Image",
            "covered": true,
            "part": "PS3.3",
            "anchor": "table_A.6-1"
        }),
        serde_json::json!({
            "source": "dicom-standard-kb",
            "edition": "2026b",
            "query": "list_attributes_for_module US Image --expand-macros",
            "covered": true,
            "part": "PS3.3",
            "anchor": "table_C.8-18"
        }),
        serde_json::json!({
            "source": "dicom-standard-kb",
            "edition": "2026b",
            "query": "resolve_attribute_context PhotometricInterpretation --iod Ultrasound Image",
            "covered": true,
            "part": "PS3.3",
            "anchor": "table_C.8-18"
        }),
        serde_json::json!({
            "source": "dicom-standard-kb",
            "edition": "2026b",
            "query": "resolve_attribute_context BitsAllocated --iod Ultrasound Image",
            "covered": true,
            "part": "PS3.3",
            "anchor": "table_C.8-18"
        }),
        serde_json::json!({
            "source": "dicom-standard-kb",
            "edition": "2026b",
            "query": "lookup_data_element UltrasoundColorDataPresent",
            "covered": true,
            "part": "PS3.6",
            "anchor": "table_6-1"
        }),
        serde_json::json!({
            "source": "dicom-standard-kb",
            "edition": "2026b",
            "query": "lookup_data_element LossyImageCompression",
            "covered": true,
            "part": "PS3.6",
            "anchor": "table_6-1"
        }),
    ]);

    if recipe.transfer_syntax == RLE_LOSSLESS {
        standards_evidence.extend([
            serde_json::json!({
                "source": "dicom-standard-kb",
                "edition": "2026b",
                "query": "lookup_uid RLELossless",
                "covered": true,
                "part": "PS3.6",
                "anchor": "table_A-1"
            }),
            serde_json::json!({
                "source": "dicom-standard-kb",
                "edition": "2026b",
                "query": "retrieve_standard_text PS3.5 sect_8.2.2",
                "covered": true,
                "part": "PS3.5",
                "anchor": "sect_8.2.2"
            }),
            serde_json::json!({
                "source": "dicom-standard-kb",
                "edition": "2026b",
                "query": "retrieve_standard_text PS3.5 sect_A.4",
                "covered": true,
                "part": "PS3.5",
                "anchor": "sect_A.4"
            }),
        ]);
    }

    let frame_hash = sha256_hex(recipe.pixel_bytes);
    let pixel_data_manifest = if let Some((backend, encapsulated)) = compressed_pixel_data {
        serde_json::json!({
            "vr": "OB",
            "native_or_encapsulated": "encapsulated",
            "value_length": Value::Null,
            "frame_count": 1,
            "frame_hashes": [frame_hash],
            "codec": {
                "backend_id": backend.backend_id,
                "backend_kind": backend.backend_kind.as_str(),
                "display_name": backend.display_name,
                "version": backend.version,
                "transfer_syntax_uid": backend.transfer_syntax_uid,
                "feature_gate": backend.feature_gate,
                "determinism": backend.determinism.as_str()
            },
            "encapsulated_pixel_data": {
                "basic_offset_table": {
                    "present": true,
                    "populated": encapsulated.basic_offset_table.is_populated(),
                    "offset_count": encapsulated.basic_offset_table.offsets.len(),
                    "offsets": encapsulated.basic_offset_table.offsets.clone()
                },
                "fragments_per_frame": encapsulated.fragments_per_frame.clone(),
                "fragments": encapsulated.fragments.iter().map(|fragment| {
                    serde_json::json!({
                        "frame_index": fragment.frame_index,
                        "item_start_offset": fragment.item_start_offset,
                        "compressed_length": fragment.compressed_length,
                        "padded_length": fragment.padded_length
                    })
                }).collect::<Vec<_>>(),
                "extended_offset_table": {
                    "present": false,
                    "lengths_present": false,
                    "offset_count": 0,
                    "length_count": 0
                },
                "compressed_frame_hashes": encapsulated.compressed_frame_hashes.clone()
            }
        })
    } else {
        serde_json::json!({
            "vr": "OB",
            "native_or_encapsulated": "native",
            "value_length": recipe.pixel_bytes.len(),
            "frame_count": 1,
            "frame_hashes": [frame_hash]
        })
    };

    serde_json::json!({
        "case_id": recipe.case_id,
        "profile_membership": classic_us_profile_membership(recipe),
        "path": relative_path,
        "sha256": sha256_hex(bytes),
        "size_bytes": bytes.len(),
        "determinism": "byte_stable",
        "recipe": {
            "recipe_id": recipe.recipe_id,
            "recipe_version": CLASSIC_US_RECIPE_VERSION,
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
                "lossy_image_compression": recipe.lossy_image_compression,
                "ultrasound_color_data_present": recipe.ultrasound_color_data_present
            }
        },
        "dicom": {
            "sop_class_uid": uids::ULTRASOUND_IMAGE_STORAGE,
            "sop_class_name": "Ultrasound Image Storage",
            "iod_name": "Ultrasound Image",
            "modality": "US",
            "transfer_syntax_uid": recipe.transfer_syntax.uid,
            "transfer_syntax_name": recipe.transfer_syntax.name
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
        "pixel_data": pixel_data_manifest,
        "expected_capabilities": classic_us_expected_capabilities(recipe),
        "expected_semantics": {
            "synthetic_data": "YES",
            "image_type": "ORIGINAL\\PRIMARY",
            "pixel_min": recipe.pixel_min,
            "pixel_max": recipe.pixel_max,
            "lossy_image_compression": recipe.lossy_image_compression,
            "ultrasound_color_data_present": recipe.ultrasound_color_data_present
        },
        "expected_visual_checks": {
            "pattern": "2x2_ultrasound_mono2_gradient"
        },
        "validation": validation,
        "known_stressors": classic_us_known_stressors(recipe),
        "standards_evidence": deduplicated_standards_evidence(standards_evidence)
    })
}

fn classic_us_profile_membership(recipe: ClassicUsRecipe) -> &'static [&'static str] {
    if recipe.transfer_syntax == RLE_LOSSLESS {
        &["extended"]
    } else {
        &["core"]
    }
}

fn classic_us_expected_capabilities(recipe: ClassicUsRecipe) -> Vec<&'static str> {
    let mut capabilities = vec!["open_file", "read_metadata"];
    if recipe.transfer_syntax == RLE_LOSSLESS {
        capabilities.push("decode_rle_lossless_pixels");
    } else {
        capabilities.push("render_native_pixels");
    }
    capabilities
}

fn classic_us_known_stressors(recipe: ClassicUsRecipe) -> Vec<&'static str> {
    let mut stressors = vec![
        "ultrasound_image_storage",
        "single_frame_us",
        "mono2_u8_pixels",
    ];
    if recipe.transfer_syntax == RLE_LOSSLESS {
        stressors.extend([
            "encapsulated_pixel_data",
            "rle_lossless_transfer_syntax",
            "compressed_modality_pixels",
        ]);
    }
    stressors
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

    let compressed_pixel_data = if recipe.transfer_syntax == RLE_LOSSLESS {
        let rle_encoder = NativeRleLosslessEncoder::new();
        let encoded_frame = rle_encoder
            .encode_frame(FrameEncodeInput {
                native_frame: recipe.pixel_bytes,
                rows: recipe.rows,
                columns: recipe.columns,
                samples_per_pixel: 1,
                bits_allocated: 8,
                bits_stored: 8,
                photometric_interpretation: "MONOCHROME2",
            })
            .map_err(|err| GenerateError::WriteDicomFile {
                path: path.clone(),
                message: err.to_string(),
            })?;
        let compressed_frames = vec![encoded_frame.bytes];
        let encapsulated = EncapsulatedPixelData::one_fragment_per_frame(
            &compressed_frames,
            BasicOffsetTablePolicy::Populated,
        )
        .map_err(|err| GenerateError::WriteDicomFile {
            path: path.clone(),
            message: err.to_string(),
        })?;
        obj.put(DataElement::new(
            tags::PIXEL_DATA,
            VR::OB,
            PixelFragmentSequence::new(
                encapsulated.basic_offset_table.offsets.clone(),
                compressed_frames,
            ),
        ));
        Some((FrameEncoder::backend(&rle_encoder), encapsulated))
    } else {
        obj.put(DataElement::new(
            tags::PIXEL_DATA,
            VR::OB,
            PrimitiveValue::from(recipe.pixel_bytes),
        ));
        None
    };

    let file_obj = obj
        .with_meta(
            FileMetaTableBuilder::new()
                .transfer_syntax(recipe.transfer_syntax.uid)
                .implementation_class_uid(&implementation_class_uid)
                .implementation_version_name(crate::IMPLEMENTATION_VERSION_NAME),
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

    let decoded_frame_hash = sha256_hex(recipe.pixel_bytes);
    let decoded_frame_hashes = [decoded_frame_hash.as_str()];
    let validated = validate_part10_file(
        &path,
        &Part10Expectations {
            sop_class_uid: uids::COMPUTED_RADIOGRAPHY_IMAGE_STORAGE,
            sop_instance_uid: &sop_instance_uid,
            transfer_syntax_uid: recipe.transfer_syntax.uid,
            implementation_class_uid: &implementation_class_uid,
            synthetic_data: "YES",
            rows: recipe.rows,
            columns: recipe.columns,
            frames: 1,
            samples_per_pixel: 1,
            photometric_interpretation: "MONOCHROME2",
            bits_allocated: 8,
            bits_stored: 8,
            high_bit: 7,
            pixel_representation: 0,
            planar_configuration: None,
            pixel_data_vr: VR::OB,
            pixel_data_length_formula: compressed_pixel_data
                .as_ref()
                .map(|(_, encapsulated)| PixelDataLengthFormula::Encapsulated {
                    fragments: encapsulated.fragments.len(),
                    basic_offset_table_offsets: encapsulated.basic_offset_table.offsets.len(),
                })
                .unwrap_or(PixelDataLengthFormula::ContiguousSamples),
            decoded_frame_hashes: if compressed_pixel_data.is_some() {
                &decoded_frame_hashes
            } else {
                &[]
            },
            palette: None,
            padding: None,
            ct_image: None,
            enhanced_ct_image: None,
            enhanced_mr_image: None,
            mg_image: None,
            dx_image: None,
            us_image: None,
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
            segmentation: None,
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
            compressed_pixel_data.as_ref(),
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
    compressed_pixel_data: Option<&(crate::codecs::CodecBackendInfo, EncapsulatedPixelData)>,
) -> Value {
    let mut standards_evidence = standards_evidence_from_case(case);
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

    if recipe.transfer_syntax == RLE_LOSSLESS {
        standards_evidence.extend([
            serde_json::json!({
                "source": "dicom-standard-kb",
                "edition": "2026b",
                "query": "lookup_uid RLELossless",
                "covered": true,
                "part": "PS3.6",
                "anchor": "table_A-1"
            }),
            serde_json::json!({
                "source": "dicom-standard-kb",
                "edition": "2026b",
                "query": "retrieve_standard_text PS3.5 sect_8.2.2",
                "covered": true,
                "part": "PS3.5",
                "anchor": "sect_8.2.2"
            }),
            serde_json::json!({
                "source": "dicom-standard-kb",
                "edition": "2026b",
                "query": "retrieve_standard_text PS3.5 sect_A.4",
                "covered": true,
                "part": "PS3.5",
                "anchor": "sect_A.4"
            }),
        ]);
    }

    let frame_hash = sha256_hex(recipe.pixel_bytes);
    let pixel_data_manifest = if let Some((backend, encapsulated)) = compressed_pixel_data {
        serde_json::json!({
            "vr": "OB",
            "native_or_encapsulated": "encapsulated",
            "value_length": Value::Null,
            "frame_count": 1,
            "frame_hashes": [frame_hash],
            "codec": {
                "backend_id": backend.backend_id,
                "backend_kind": backend.backend_kind.as_str(),
                "display_name": backend.display_name,
                "version": backend.version,
                "transfer_syntax_uid": backend.transfer_syntax_uid,
                "feature_gate": backend.feature_gate,
                "determinism": backend.determinism.as_str()
            },
            "encapsulated_pixel_data": {
                "basic_offset_table": {
                    "present": true,
                    "populated": encapsulated.basic_offset_table.is_populated(),
                    "offset_count": encapsulated.basic_offset_table.offsets.len(),
                    "offsets": encapsulated.basic_offset_table.offsets.clone()
                },
                "fragments_per_frame": encapsulated.fragments_per_frame.clone(),
                "fragments": encapsulated.fragments.iter().map(|fragment| {
                    serde_json::json!({
                        "frame_index": fragment.frame_index,
                        "item_start_offset": fragment.item_start_offset,
                        "compressed_length": fragment.compressed_length,
                        "padded_length": fragment.padded_length
                    })
                }).collect::<Vec<_>>(),
                "extended_offset_table": {
                    "present": false,
                    "lengths_present": false,
                    "offset_count": 0,
                    "length_count": 0
                },
                "compressed_frame_hashes": encapsulated.compressed_frame_hashes.clone()
            }
        })
    } else {
        serde_json::json!({
            "vr": "OB",
            "native_or_encapsulated": "native",
            "value_length": recipe.pixel_bytes.len(),
            "frame_count": 1,
            "frame_hashes": [frame_hash]
        })
    };

    serde_json::json!({
        "case_id": recipe.case_id,
        "profile_membership": classic_cr_profile_membership(recipe),
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
                "body_part_examined": recipe.body_part_examined,
                "view_position": recipe.view_position,
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
            "transfer_syntax_uid": recipe.transfer_syntax.uid,
            "transfer_syntax_name": recipe.transfer_syntax.name
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
        "pixel_data": pixel_data_manifest,
        "expected_capabilities": classic_cr_expected_capabilities(recipe),
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
        "known_stressors": classic_cr_known_stressors(recipe),
        "standards_evidence": deduplicated_standards_evidence(standards_evidence)
    })
}

fn classic_cr_profile_membership(recipe: ClassicCrRecipe) -> &'static [&'static str] {
    if recipe.transfer_syntax == RLE_LOSSLESS {
        &["extended"]
    } else {
        &["core"]
    }
}

fn classic_cr_expected_capabilities(recipe: ClassicCrRecipe) -> Vec<&'static str> {
    let mut capabilities = vec!["open_file", "read_metadata"];
    if recipe.transfer_syntax == RLE_LOSSLESS {
        capabilities.push("decode_rle_lossless_pixels");
    } else {
        capabilities.push("render_native_pixels");
    }
    capabilities.extend(["read_overlay_plane", "apply_modality_lut", "apply_voi_lut"]);
    capabilities
}

fn classic_cr_known_stressors(recipe: ClassicCrRecipe) -> Vec<&'static str> {
    let mut stressors = vec![
        "computed_radiography_image_storage",
        "overlay_plane",
        "modality_lut_sequence",
        "voi_lut_sequence",
    ];
    if recipe.transfer_syntax == RLE_LOSSLESS {
        stressors.extend([
            "encapsulated_pixel_data",
            "rle_lossless_transfer_syntax",
            "compressed_modality_pixels",
        ]);
    }
    stressors
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

        let compressed_pixel_data = if recipe.transfer_syntax == RLE_LOSSLESS {
            let rle_encoder = NativeRleLosslessEncoder::new();
            let encoded_frame = rle_encoder
                .encode_frame(FrameEncodeInput {
                    native_frame: slice.pixel_bytes,
                    rows: recipe.rows,
                    columns: recipe.columns,
                    samples_per_pixel: 1,
                    bits_allocated: 16,
                    bits_stored: 16,
                    photometric_interpretation: "MONOCHROME2",
                })
                .map_err(|err| GenerateError::WriteDicomFile {
                    path: path.clone(),
                    message: err.to_string(),
                })?;
            let compressed_frames = vec![encoded_frame.bytes];
            let encapsulated = EncapsulatedPixelData::one_fragment_per_frame(
                &compressed_frames,
                BasicOffsetTablePolicy::Populated,
            )
            .map_err(|err| GenerateError::WriteDicomFile {
                path: path.clone(),
                message: err.to_string(),
            })?;
            obj.put(DataElement::new(
                tags::PIXEL_DATA,
                VR::OB,
                PixelFragmentSequence::new(
                    encapsulated.basic_offset_table.offsets.clone(),
                    compressed_frames,
                ),
            ));
            Some((FrameEncoder::backend(&rle_encoder), encapsulated))
        } else {
            obj.put(DataElement::new(
                tags::PIXEL_DATA,
                VR::OW,
                PrimitiveValue::from(slice.pixel_bytes),
            ));
            None
        };

        let file_obj = obj
            .with_meta(
                FileMetaTableBuilder::new()
                    .transfer_syntax(recipe.transfer_syntax.uid)
                    .implementation_class_uid(&implementation_class_uid)
                    .implementation_version_name(crate::IMPLEMENTATION_VERSION_NAME),
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

        let decoded_frame_hash = sha256_hex(slice.pixel_bytes);
        let decoded_frame_hashes = [decoded_frame_hash.as_str()];
        let validated = validate_part10_file(
            &path,
            &Part10Expectations {
                sop_class_uid: uids::MR_IMAGE_STORAGE,
                sop_instance_uid: &sop_instance_uid,
                transfer_syntax_uid: recipe.transfer_syntax.uid,
                implementation_class_uid: &implementation_class_uid,
                synthetic_data: "YES",
                rows: recipe.rows,
                columns: recipe.columns,
                frames: 1,
                samples_per_pixel: 1,
                photometric_interpretation: "MONOCHROME2",
                bits_allocated: 16,
                bits_stored: 16,
                high_bit: 15,
                pixel_representation: 0,
                planar_configuration: None,
                pixel_data_vr: if compressed_pixel_data.is_some() {
                    VR::OB
                } else {
                    VR::OW
                },
                pixel_data_length_formula: compressed_pixel_data
                    .as_ref()
                    .map(|(_, encapsulated)| PixelDataLengthFormula::Encapsulated {
                        fragments: encapsulated.fragments.len(),
                        basic_offset_table_offsets: encapsulated.basic_offset_table.offsets.len(),
                    })
                    .unwrap_or(PixelDataLengthFormula::ContiguousSamples),
                decoded_frame_hashes: if compressed_pixel_data.is_some() {
                    &decoded_frame_hashes
                } else {
                    &[]
                },
                palette: None,
                padding: None,
                ct_image: None,
                enhanced_ct_image: None,
                enhanced_mr_image: None,
                mg_image: None,
                dx_image: None,
                us_image: None,
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
                segmentation: None,
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
                compressed_pixel_data.as_ref(),
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
    compressed_pixel_data: Option<&(crate::codecs::CodecBackendInfo, EncapsulatedPixelData)>,
) -> Value {
    let mut standards_evidence = standards_evidence_from_case(case);
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

    if recipe.transfer_syntax == RLE_LOSSLESS {
        standards_evidence.extend([
            serde_json::json!({
                "source": "dicom-standard-kb",
                "edition": "2026b",
                "query": "lookup_uid RLELossless",
                "covered": true,
                "part": "PS3.6",
                "anchor": "table_A-1"
            }),
            serde_json::json!({
                "source": "dicom-standard-kb",
                "edition": "2026b",
                "query": "retrieve_standard_text PS3.5 sect_8.2.2",
                "covered": true,
                "part": "PS3.5",
                "anchor": "sect_8.2.2"
            }),
            serde_json::json!({
                "source": "dicom-standard-kb",
                "edition": "2026b",
                "query": "retrieve_standard_text PS3.5 sect_A.4",
                "covered": true,
                "part": "PS3.5",
                "anchor": "sect_A.4"
            }),
        ]);
    }

    let frame_hash = sha256_hex(slice.pixel_bytes);
    let pixel_data_manifest = if let Some((backend, encapsulated)) = compressed_pixel_data {
        serde_json::json!({
            "vr": "OB",
            "native_or_encapsulated": "encapsulated",
            "value_length": Value::Null,
            "frame_count": 1,
            "frame_hashes": [frame_hash],
            "codec": {
                "backend_id": backend.backend_id,
                "backend_kind": backend.backend_kind.as_str(),
                "display_name": backend.display_name,
                "version": backend.version,
                "transfer_syntax_uid": backend.transfer_syntax_uid,
                "feature_gate": backend.feature_gate,
                "determinism": backend.determinism.as_str()
            },
            "encapsulated_pixel_data": {
                "basic_offset_table": {
                    "present": true,
                    "populated": encapsulated.basic_offset_table.is_populated(),
                    "offset_count": encapsulated.basic_offset_table.offsets.len(),
                    "offsets": encapsulated.basic_offset_table.offsets.clone()
                },
                "fragments_per_frame": encapsulated.fragments_per_frame.clone(),
                "fragments": encapsulated.fragments.iter().map(|fragment| {
                    serde_json::json!({
                        "frame_index": fragment.frame_index,
                        "item_start_offset": fragment.item_start_offset,
                        "compressed_length": fragment.compressed_length,
                        "padded_length": fragment.padded_length
                    })
                }).collect::<Vec<_>>(),
                "extended_offset_table": {
                    "present": false,
                    "lengths_present": false,
                    "offset_count": 0,
                    "length_count": 0
                },
                "compressed_frame_hashes": encapsulated.compressed_frame_hashes.clone()
            }
        })
    } else {
        serde_json::json!({
            "vr": "OW",
            "native_or_encapsulated": "native",
            "value_length": slice.pixel_bytes.len(),
            "frame_count": 1,
            "frame_hashes": [frame_hash]
        })
    };

    serde_json::json!({
        "case_id": recipe.case_id,
        "profile_membership": classic_mr_profile_membership(recipe),
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
            "transfer_syntax_uid": recipe.transfer_syntax.uid,
            "transfer_syntax_name": recipe.transfer_syntax.name
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
        "pixel_data": pixel_data_manifest,
        "expected_capabilities": classic_mr_expected_capabilities(recipe),
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
            "pattern": if recipe.transfer_syntax == RLE_LOSSLESS {
                "single_slice_mr_rle_lossless_gradient"
            } else {
                "3_slice_oblique_mr_gradient_stack"
            }
        },
        "validation": validation,
        "known_stressors": classic_mr_known_stressors(recipe),
        "standards_evidence": deduplicated_standards_evidence(standards_evidence)
    })
}

fn classic_mr_profile_membership(recipe: ClassicMrRecipe) -> &'static [&'static str] {
    if recipe.transfer_syntax == RLE_LOSSLESS {
        &["extended"]
    } else {
        &["core"]
    }
}

fn classic_mr_expected_capabilities(recipe: ClassicMrRecipe) -> Vec<&'static str> {
    let mut capabilities = vec!["open_file", "read_metadata"];
    if recipe.transfer_syntax == RLE_LOSSLESS {
        capabilities.push("decode_rle_lossless_pixels");
    } else {
        capabilities.push("render_native_pixels");
    }
    capabilities.push("sort_series_by_geometry");
    capabilities
}

fn classic_mr_known_stressors(recipe: ClassicMrRecipe) -> Vec<&'static str> {
    let mut stressors = vec!["mr_image_storage"];
    if recipe.slices.len() > 1 {
        stressors.extend([
            "multi_instance_series",
            "oblique_image_orientation_patient",
            "geometry_slice_sorting",
        ]);
    }
    if recipe.transfer_syntax == RLE_LOSSLESS {
        stressors.extend([
            "encapsulated_pixel_data",
            "rle_lossless_transfer_syntax",
            "compressed_modality_pixels",
        ]);
    }
    stressors
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
    file_index: u32,
) -> String {
    deterministic_uid(&DeterministicUidInput {
        standards_lock_sha256,
        case_id: recipe.case_id,
        recipe_version: CLASSIC_CT_RECIPE_VERSION,
        run_seed,
        file_index,
        frame_index: None,
        referenced_object_index: None,
        role,
    })
}

fn deterministic_enhanced_ct_uid(
    standards_lock_sha256: &str,
    recipe: EnhancedCtRecipe,
    run_seed: u64,
    role: UidRole,
) -> String {
    deterministic_uid(&DeterministicUidInput {
        standards_lock_sha256,
        case_id: recipe.case_id,
        recipe_version: ENHANCED_CT_RECIPE_VERSION,
        run_seed,
        file_index: 0,
        frame_index: None,
        referenced_object_index: None,
        role,
    })
}

fn deterministic_segmentation_uid(
    standards_lock_sha256: &str,
    recipe: SegmentationRecipe,
    run_seed: u64,
    role: UidRole,
) -> String {
    deterministic_uid(&DeterministicUidInput {
        standards_lock_sha256,
        case_id: recipe.case_id,
        recipe_version: SEGMENTATION_RECIPE_VERSION,
        run_seed,
        file_index: 0,
        frame_index: None,
        referenced_object_index: Some(0),
        role,
    })
}

fn deterministic_presentation_state_uid(
    standards_lock_sha256: &str,
    recipe: PresentationStateRecipe,
    run_seed: u64,
    role: UidRole,
) -> String {
    deterministic_uid(&DeterministicUidInput {
        standards_lock_sha256,
        case_id: recipe.case_id,
        recipe_version: GSPS_RECIPE_VERSION,
        run_seed,
        file_index: 0,
        frame_index: None,
        referenced_object_index: Some(0),
        role,
    })
}

fn deterministic_real_world_value_mapping_uid(
    standards_lock_sha256: &str,
    recipe: RealWorldValueMappingRecipe,
    run_seed: u64,
    role: UidRole,
) -> String {
    deterministic_uid(&DeterministicUidInput {
        standards_lock_sha256,
        case_id: recipe.case_id,
        recipe_version: RWVM_RECIPE_VERSION,
        run_seed,
        file_index: 0,
        frame_index: None,
        referenced_object_index: Some(0),
        role,
    })
}

fn deterministic_basic_text_sr_uid(
    standards_lock_sha256: &str,
    recipe: BasicTextSrRecipe,
    run_seed: u64,
    role: UidRole,
) -> String {
    deterministic_uid(&DeterministicUidInput {
        standards_lock_sha256,
        case_id: recipe.case_id,
        recipe_version: BASIC_TEXT_SR_RECIPE_VERSION,
        run_seed,
        file_index: 0,
        frame_index: None,
        referenced_object_index: Some(0),
        role,
    })
}

fn deterministic_comprehensive_sr_uid(
    standards_lock_sha256: &str,
    recipe: ComprehensiveSrRecipe,
    run_seed: u64,
    role: UidRole,
) -> String {
    deterministic_uid(&DeterministicUidInput {
        standards_lock_sha256,
        case_id: recipe.case_id,
        recipe_version: COMPREHENSIVE_SR_RECIPE_VERSION,
        run_seed,
        file_index: 0,
        frame_index: None,
        referenced_object_index: Some(0),
        role,
    })
}

fn deterministic_key_object_selection_uid(
    standards_lock_sha256: &str,
    recipe: KeyObjectSelectionRecipe,
    run_seed: u64,
    role: UidRole,
) -> String {
    deterministic_uid(&DeterministicUidInput {
        standards_lock_sha256,
        case_id: recipe.case_id,
        recipe_version: KEY_OBJECT_SELECTION_RECIPE_VERSION,
        run_seed,
        file_index: 0,
        frame_index: None,
        referenced_object_index: Some(0),
        role,
    })
}

fn deterministic_rt_structure_set_uid(
    standards_lock_sha256: &str,
    recipe: RtStructureSetRecipe,
    run_seed: u64,
    role: UidRole,
) -> String {
    deterministic_uid(&DeterministicUidInput {
        standards_lock_sha256,
        case_id: recipe.case_id,
        recipe_version: RT_STRUCTURE_SET_RECIPE_VERSION,
        run_seed,
        file_index: 0,
        frame_index: None,
        referenced_object_index: Some(0),
        role,
    })
}

fn deterministic_rt_dose_uid(
    standards_lock_sha256: &str,
    recipe: RtDoseRecipe,
    run_seed: u64,
    role: UidRole,
) -> String {
    deterministic_uid(&DeterministicUidInput {
        standards_lock_sha256,
        case_id: recipe.case_id,
        recipe_version: RT_DOSE_RECIPE_VERSION,
        run_seed,
        file_index: 0,
        frame_index: None,
        referenced_object_index: Some(0),
        role,
    })
}

fn deterministic_encapsulated_pdf_uid(
    standards_lock_sha256: &str,
    recipe: EncapsulatedPdfRecipe,
    run_seed: u64,
    role: UidRole,
) -> String {
    deterministic_uid(&DeterministicUidInput {
        standards_lock_sha256,
        case_id: recipe.case_id,
        recipe_version: ENCAPSULATED_PDF_RECIPE_VERSION,
        run_seed,
        file_index: 0,
        frame_index: None,
        referenced_object_index: None,
        role,
    })
}

fn deterministic_enhanced_ct_indexed_uid(
    standards_lock_sha256: &str,
    recipe: EnhancedCtRecipe,
    run_seed: u64,
    role: UidRole,
    file_index: u32,
) -> String {
    deterministic_uid(&DeterministicUidInput {
        standards_lock_sha256,
        case_id: recipe.case_id,
        recipe_version: ENHANCED_CT_RECIPE_VERSION,
        run_seed,
        file_index,
        frame_index: None,
        referenced_object_index: None,
        role,
    })
}

fn deterministic_enhanced_mr_uid(
    standards_lock_sha256: &str,
    recipe: EnhancedMrRecipe,
    run_seed: u64,
    role: UidRole,
) -> String {
    deterministic_uid(&DeterministicUidInput {
        standards_lock_sha256,
        case_id: recipe.case_id,
        recipe_version: ENHANCED_MR_RECIPE_VERSION,
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

fn deterministic_classic_us_uid(
    standards_lock_sha256: &str,
    recipe: ClassicUsRecipe,
    run_seed: u64,
    role: UidRole,
) -> String {
    deterministic_uid(&DeterministicUidInput {
        standards_lock_sha256,
        case_id: recipe.case_id,
        recipe_version: CLASSIC_US_RECIPE_VERSION,
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

fn put_u32(obj: &mut InMemDicomObject, tag: dicom_core::Tag, vr: VR, value: u32) {
    obj.put(DataElement::new(tag, vr, PrimitiveValue::from(value)));
}

fn put_i16(obj: &mut InMemDicomObject, tag: dicom_core::Tag, vr: VR, value: i16) {
    obj.put(DataElement::new(tag, vr, PrimitiveValue::from(value)));
}

fn put_pixel_padding(
    obj: &mut InMemDicomObject,
    tag: dicom_core::Tag,
    value: i16,
    pixel_representation: u16,
) {
    if pixel_representation == 1 {
        put_i16(obj, tag, VR::SS, value);
    } else {
        put_u16(
            obj,
            tag,
            VR::US,
            u16::try_from(value).expect("unsigned Pixel Padding values must be non-negative"),
        );
    }
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

fn should_generate_case(case: &Value, run: &PreparedGenerationRun) -> Result<bool, GenerateError> {
    let profiles = string_array(case.get("profiles"))?;
    if !case_matches_profile(&profiles, &run.profile, run.include_stress) {
        return Ok(false);
    }

    let status =
        case.get("status")
            .and_then(Value::as_str)
            .ok_or(GenerateError::MetadataShape {
                path: PathBuf::from("cases/registry.json"),
                message: "case status must be a string",
            })?;

    if status != "implemented" {
        return Ok(false);
    }

    let required_features = string_array(case.pointer("/requirements/features"))?;
    Ok(required_features
        .iter()
        .all(|feature| crate::ACTIVE_FEATURE_FLAGS.contains(&feature.as_str())))
}

fn standards_evidence_from_case(case: &Value) -> Vec<Value> {
    case.get("standards_evidence")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

fn deduplicated_standards_evidence(evidence: Vec<Value>) -> Vec<Value> {
    let mut seen = BTreeSet::new();
    let mut deduplicated = Vec::new();

    for entry in evidence {
        if seen.insert(standards_evidence_key(&entry)) {
            deduplicated.push(entry);
        }
    }

    deduplicated
}

fn standards_evidence_key(entry: &Value) -> String {
    let source = entry.get("source").and_then(Value::as_str).unwrap_or("");
    let edition = entry.get("edition").and_then(Value::as_str).unwrap_or("");
    if let Some(query) = entry.get("query").and_then(Value::as_str) {
        return format!("query|{source}|{edition}|{query}");
    }

    let part = entry.get("part").and_then(Value::as_str).unwrap_or("");
    let anchor = entry.get("anchor").and_then(Value::as_str).unwrap_or("");
    let reason = entry.get("reason").and_then(Value::as_str).unwrap_or("");
    if !part.is_empty() || !anchor.is_empty() || !reason.is_empty() {
        return format!("anchor|{source}|{edition}|{part}|{anchor}|{reason}");
    }

    serde_json::to_string(entry).unwrap_or_else(|_| format!("{entry:?}"))
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
            matches!(profile.as_str(), "smoke" | "core" | "extended")
                || (include_stress && profile == "stress")
        }),
        profile => profiles.iter().any(|case_profile| case_profile == profile),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transfer_syntax_specs_are_backed_by_capability_matrix() {
        let matrix: Value =
            serde_json::from_str(include_str!("../transfer-syntax/capability-matrix.json"))
                .expect("transfer syntax capability matrix should parse");
        let entries = matrix
            .get("entries")
            .and_then(Value::as_array)
            .expect("transfer syntax capability matrix should contain entries");

        for spec in [
            IMPLICIT_VR_LITTLE_ENDIAN,
            EXPLICIT_VR_LITTLE_ENDIAN,
            EXPLICIT_VR_BIG_ENDIAN,
            DEFLATED_EXPLICIT_VR_LITTLE_ENDIAN,
            JPEG_BASELINE_8BIT,
            JPEG_LS_LOSSLESS,
        ] {
            let entry = entries
                .iter()
                .find(|entry| {
                    entry.get("keyword").and_then(Value::as_str) == Some(spec.capability_keyword)
                })
                .unwrap_or_else(|| {
                    panic!(
                        "transfer syntax capability matrix should contain {}",
                        spec.capability_keyword
                    )
                });

            assert_eq!(entry.get("uid").and_then(Value::as_str), Some(spec.uid));
            assert_eq!(
                entry.get("name").and_then(Value::as_str),
                Some(spec.capability_name)
            );
            let feature_flags = entry
                .get("feature_flags")
                .and_then(Value::as_array)
                .expect("transfer syntax matrix entry should contain feature_flags");
            if feature_flags.is_empty() {
                assert_eq!(
                    entry.get("status").and_then(Value::as_str),
                    Some("available")
                );
            } else {
                assert_eq!(
                    entry.get("status").and_then(Value::as_str),
                    Some("feature_gated")
                );
            }
            assert_eq!(
                entry.get("write_dataset").and_then(Value::as_bool),
                Some(true)
            );
        }
    }

    #[test]
    fn generated_source_registry_extracts_identity_for_manifest_references() {
        let file = generated_source_fixture(
            "enhanced/ct/multiframe_shared_perframe_explicit_le",
            "enhanced/ct/multiframe_shared_perframe_explicit_le/instance.dcm",
            "1.2.840.10008.5.1.4.1.1.2.1",
            "2.25.100",
            Some("2.25.200"),
            Some(2),
        );
        let mut registry = GeneratedSourceRegistry::default();

        registry
            .register(&file)
            .expect("generated source object should register from manifest identity");

        let source = registry
            .first_for_case("enhanced/ct/multiframe_shared_perframe_explicit_le")
            .expect("registered source should be visible by case ID");
        assert_eq!(
            registry.by_path("enhanced/ct/multiframe_shared_perframe_explicit_le/instance.dcm"),
            Some(source)
        );
        assert_eq!(source.source_case_id, file.case_id);
        assert_eq!(source.frame_count, Some(2));

        let reference = source.to_manifest_reference("source_image", Some(vec![1, 2]));
        assert_eq!(
            reference.get("relationship").and_then(Value::as_str),
            Some("source_image")
        );
        assert_eq!(
            reference.get("source_case_id").and_then(Value::as_str),
            Some("enhanced/ct/multiframe_shared_perframe_explicit_le")
        );
        assert_eq!(
            reference.get("source_path").and_then(Value::as_str),
            Some("enhanced/ct/multiframe_shared_perframe_explicit_le/instance.dcm")
        );
        assert_eq!(
            reference.get("sop_instance_uid").and_then(Value::as_str),
            Some("2.25.100")
        );
        assert_eq!(
            reference.get("series_instance_uid").and_then(Value::as_str),
            Some("2.25.200")
        );
        assert_eq!(
            reference
                .get("frame_numbers")
                .and_then(Value::as_array)
                .map(Vec::len),
            Some(2)
        );
    }

    #[test]
    fn generation_context_exposes_only_already_recorded_sources() {
        let mut context = GenerationContext::default();
        let source_case_id = "enhanced/ct/multiframe_shared_perframe_explicit_le";
        let later_case_id = "derived/seg/binary_multiframe_explicit_le";

        assert!(
            context
                .source_registry()
                .first_for_case(source_case_id)
                .is_none(),
            "no source should be visible before its writer records output"
        );

        context
            .record_one(generated_source_fixture(
                source_case_id,
                "enhanced/ct/multiframe_shared_perframe_explicit_le/instance.dcm",
                "1.2.840.10008.5.1.4.1.1.2.1",
                "2.25.100",
                Some("2.25.200"),
                Some(2),
            ))
            .expect("source image should record");

        assert!(
            context
                .source_registry()
                .first_for_case(source_case_id)
                .is_some(),
            "later derived writers must be able to see prior generated sources"
        );
        assert!(
            context
                .source_registry()
                .first_for_case(later_case_id)
                .is_none(),
            "future cases must not be visible before their own writers run"
        );

        context
            .record_one(generated_source_fixture(
                later_case_id,
                "derived/seg/binary_multiframe_explicit_le/instance.dcm",
                "1.2.840.10008.5.1.4.1.1.66.4",
                "2.25.300",
                Some("2.25.400"),
                Some(2),
            ))
            .expect("derived object should record after source image");

        let source_paths: Vec<&str> = context
            .source_registry()
            .sources_for_case(source_case_id)
            .map(|source| source.source_path.as_str())
            .collect();
        assert_eq!(
            source_paths,
            vec!["enhanced/ct/multiframe_shared_perframe_explicit_le/instance.dcm"]
        );
        assert_eq!(context.into_generated_files().len(), 2);
    }

    #[test]
    fn standards_evidence_deduplication_keeps_first_matching_query() {
        let evidence = vec![
            serde_json::json!({
                "source": "dicom-standard-kb",
                "edition": "2026b",
                "query": "lookup_uid ExplicitVRLittleEndian",
                "part": "PS3.6",
                "anchor": "table_A-1",
                "origin": "registry"
            }),
            serde_json::json!({
                "source": "dicom-standard-kb",
                "edition": "2026b",
                "query": "lookup_uid ExplicitVRLittleEndian",
                "part": "PS3.6",
                "anchor": "table_A-1",
                "origin": "recipe"
            }),
            serde_json::json!({
                "source": "dicom-standard-kb",
                "edition": "2026b",
                "query": "lookup_data_element SyntheticData",
                "part": "PS3.6",
                "anchor": "table_6-1"
            }),
        ];

        let deduplicated = deduplicated_standards_evidence(evidence);

        assert_eq!(deduplicated.len(), 2);
        assert_eq!(
            deduplicated[0].get("origin").and_then(Value::as_str),
            Some("registry"),
            "registry evidence should win when a recipe repeats the same query"
        );
        assert_eq!(
            deduplicated[1].get("query").and_then(Value::as_str),
            Some("lookup_data_element SyntheticData")
        );
    }

    fn generated_source_fixture(
        case_id: &str,
        path: &str,
        sop_class_uid: &str,
        sop_instance_uid: &str,
        series_instance_uid: Option<&str>,
        frames: Option<u64>,
    ) -> GeneratedFile {
        let mut manifest_entry = serde_json::json!({
            "path": path,
            "case_id": case_id,
            "dicom": {
                "sop_class_uid": sop_class_uid
            },
            "uids": {
                "study_instance_uid": "2.25.50",
                "sop_instance_uid": sop_instance_uid
            },
            "image": Value::Null
        });

        if let Some(series_instance_uid) = series_instance_uid {
            manifest_entry
                .pointer_mut("/uids")
                .and_then(Value::as_object_mut)
                .expect("fixture uids object should exist")
                .insert(
                    "series_instance_uid".to_string(),
                    Value::String(series_instance_uid.to_string()),
                );
        }
        if let Some(frames) = frames {
            manifest_entry
                .as_object_mut()
                .expect("fixture manifest object should exist")
                .insert("image".to_string(), serde_json::json!({ "frames": frames }));
        }

        GeneratedFile {
            case_id: case_id.to_string(),
            manifest_entry,
        }
    }
}
