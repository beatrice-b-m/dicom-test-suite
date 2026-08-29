use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use dicom_core::{
    DataElement, Length, PrimitiveValue, Tag, VR,
    value::{DataSetSequence, PixelFragmentSequence},
};
use dicom_dictionary_std::{tags, uids};
use dicom_object::{FileMetaTableBuilder, InMemDicomObject, open_file};
use dicom_parser::dataset::write::{DataSetWriterOptions, ExplicitLengthSqItemStrategy};
use dicom_transfer_syntax_registry::{TransferSyntaxIndex, TransferSyntaxRegistry};
use serde_json::Value;

mod native;

use native::advanced_blending_presentation_state::{
    ADVANCED_BLENDING_PRESENTATION_STATE_CONTENT_DESCRIPTION,
    ADVANCED_BLENDING_PRESENTATION_STATE_CONTENT_LABEL,
    ADVANCED_BLENDING_PRESENTATION_STATE_CREATION_DATE,
    ADVANCED_BLENDING_PRESENTATION_STATE_CREATION_TIME,
    ADVANCED_BLENDING_PRESENTATION_STATE_OUTPUT_FILE,
    ADVANCED_BLENDING_PRESENTATION_STATE_STORAGE_UID, AdvancedBlendingPresentationStateInput,
    AdvancedBlendingPresentationStateReference, build_advanced_blending_presentation_state,
};
use native::blending_presentation_state::{
    BLENDING_PRESENTATION_STATE_CONTENT_DESCRIPTION, BLENDING_PRESENTATION_STATE_CONTENT_LABEL,
    BLENDING_PRESENTATION_STATE_CREATION_DATE, BLENDING_PRESENTATION_STATE_CREATION_TIME,
    BLENDING_PRESENTATION_STATE_OUTPUT_FILE, BLENDING_PRESENTATION_STATE_PALETTE_BYTES,
    BLENDING_PRESENTATION_STATE_PALETTE_DESCRIPTOR, BLENDING_PRESENTATION_STATE_RELATIVE_OPACITY,
    BLENDING_PRESENTATION_STATE_STORAGE_UID, BlendingPresentationStateInput,
    BlendingPresentationStateReference, build_blending_presentation_state,
};
use native::color_softcopy_presentation_state::{
    COLOR_SOFTCOPY_PRESENTATION_STATE_CONTENT_DESCRIPTION,
    COLOR_SOFTCOPY_PRESENTATION_STATE_CONTENT_LABEL,
    COLOR_SOFTCOPY_PRESENTATION_STATE_CREATION_DATE,
    COLOR_SOFTCOPY_PRESENTATION_STATE_CREATION_TIME, COLOR_SOFTCOPY_PRESENTATION_STATE_OUTPUT_FILE,
    COLOR_SOFTCOPY_PRESENTATION_STATE_STORAGE_UID, ColorSoftcopyPresentationStateInput,
    ColorSoftcopyPresentationStateReference, build_color_softcopy_presentation_state,
};
use native::ct_geometry::{
    CLASSIC_CT_RECIPES, ClassicCtInstanceNumber, ClassicCtRecipe, ClassicCtSeriesRecipe,
    ClassicCtSliceRecipe,
};
use native::deformable_spatial_registration::{
    DEFORMABLE_SPATIAL_REGISTRATION_OUTPUT_FILE, DEFORMABLE_SPATIAL_REGISTRATION_STORAGE_UID,
    DeformableRegistrationReference, DeformableSpatialRegistrationInput,
    GRID_DIMENSIONS as DEFORMABLE_GRID_DIMENSIONS, GRID_RESOLUTION as DEFORMABLE_GRID_RESOLUTION,
    IDENTITY_MATRIX as DEFORMABLE_IDENTITY_MATRIX,
    VECTOR_GRID_BYTES as DEFORMABLE_VECTOR_GRID_BYTES,
    VECTOR_GRID_VALUES as DEFORMABLE_VECTOR_GRID_VALUES, build_deformable_spatial_registration,
};
use native::empty_type2_sc::{EMPTY_TYPE2_SC_RECIPE, EmptyType2ScRecipe};
use native::encapsulated_stl::{
    MIME_TYPE as STL_MIME_TYPE, PAYLOAD_LEN as STL_PAYLOAD_LEN,
    TRIANGLE_COUNT as STL_TRIANGLE_COUNT, UNIT_CODE_MEANING as STL_UNIT_CODE_MEANING,
    UNIT_CODE_VALUE as STL_UNIT_CODE_VALUE, UNIT_CODING_SCHEME as STL_UNIT_CODING_SCHEME,
    closed_tetrahedron_binary_stl,
};
use native::general_ecg::{
    GENERAL_ECG_AGGREGATE_SHA256, GENERAL_ECG_OUTPUT_FILE, GENERAL_ECG_STORAGE_UID,
    GENERAL_ECG_TOTAL_CHANNEL_COUNT, GENERAL_ECG_TOTAL_PAYLOAD_LENGTH, GeneralEcgInput,
    build_general_ecg,
};
use native::icc_profile::{ICC_COLOR_SPACE, ICC_PROFILE_SHA256, ICC_PROFILE_SIZE};
use native::metadata_sc::{METADATA_SC_RECIPES, MetadataScRecipe};
use native::private_creator_sc::{
    PRIVATE_CREATOR_SC_RECIPE, PrivateCreatorBlockRecipe, PrivateCreatorScRecipe, PrivateValue,
};
use native::rt_image::{
    RT_IMAGE_OUTPUT_FILE, RT_IMAGE_PIXEL_BYTES, RT_IMAGE_PIXEL_SHA256, RT_IMAGE_STORAGE_UID,
    RT_PLAN_STORAGE_UID as RT_IMAGE_REFERENCED_PLAN_STORAGE_UID, RtImageInput, build_rt_image,
};
use native::rt_plan::{
    RT_DOSE_STORAGE_UID as RT_PLAN_REFERENCED_DOSE_STORAGE_UID, RT_PLAN_OUTPUT_FILE,
    RT_PLAN_STORAGE_UID,
    RT_STRUCTURE_SET_STORAGE_UID as RT_PLAN_REFERENCED_STRUCTURE_SET_STORAGE_UID, RtPlanInput,
    build_rt_plan,
};
use native::rt_radiation::{
    C_ARM_PHOTON_ELECTRON_RADIATION_STORAGE_UID,
    RT_PLAN_STORAGE_UID as RT_RADIATION_PLAN_STORAGE_UID, RT_RADIATION_OUTPUT_FILE,
    RT_RADIATION_SET_OUTPUT_FILE, RT_RADIATION_SET_STORAGE_UID, RtRadiationInput,
    RtRadiationSetInput as NativeRtRadiationSetInput, build_rt_radiation, build_rt_radiation_set,
};
use native::sc_integer_pixels::{U1_SC_RECIPE, U32_SC_RECIPE};
use native::sc_nonsquare_spacing::{
    NONSQUARE_SPACING_SC_RECIPE, NonsquareGeometryVariant, NonsquareSpacingScRecipe,
};
use native::sequence_length_sc::{
    CODE_MEANING as SEQUENCE_CODE_MEANING, CODE_VALUE as SEQUENCE_CODE_VALUE,
    CODING_SCHEME_DESIGNATOR as SEQUENCE_CODING_SCHEME_DESIGNATOR, ITEM_DATASET_ENCODED_LENGTH,
    SEQUENCE_LENGTH_SC_RECIPE, SequenceLengthScRecipe, SequenceLengthVariant,
    SequenceLengthVariantId, UNDEFINED_ITEM_ENCODED_LENGTH,
};
use native::spatial_registration::{
    SOURCE_TO_TARGET_MATRIX, SPATIAL_REGISTRATION_OUTPUT_FILE, SPATIAL_REGISTRATION_STORAGE_UID,
    SpatialRegistrationInput, SpatialRegistrationReference, TARGET_IDENTITY_MATRIX,
    build_spatial_registration,
};
use native::string_boundary_sc::{STRING_BOUNDARY_SC_RECIPE, StringBoundaryScRecipe};
use native::timezone_sc::{TIMEZONE_SC_RECIPE, TimezoneBoundary, TimezoneScRecipe};
use native::twelve_lead_ecg::{
    TWELVE_LEAD_ECG_OUTPUT_FILE, TWELVE_LEAD_ECG_STORAGE_UID, TwelveLeadEcgInput,
    build_twelve_lead_ecg,
};

use crate::{
    DeterministicUidInput, GenerateError, PreparedGenerationRun, UidRole,
    codecs::{
        DEFLATED_IMAGE_FRAME_TRANSFER_SYNTAX_UID, FrameEncodeInput, FrameEncoder,
        HTJ2K_LOSSLESS_TRANSFER_SYNTAX_UID, HTJ2K_LOSSY_TRANSFER_SYNTAX_UID,
        JPEG_2000_LOSSLESS_TRANSFER_SYNTAX_UID, JPEG_BASELINE_8BIT_TRANSFER_SYNTAX_UID,
        JPEG_LOSSLESS_PROCESS_14_TRANSFER_SYNTAX_UID, JPEG_LOSSLESS_SV1_TRANSFER_SYNTAX_UID,
        JPEG_LS_LOSSLESS_TRANSFER_SYNTAX_UID, JPEG_XL_LOSSLESS_TRANSFER_SYNTAX_UID,
        JPEG_XL_LOSSY_TRANSFER_SYNTAX_UID, NativeRleLosslessEncoder,
        RLE_LOSSLESS_TRANSFER_SYNTAX_UID,
    },
    deterministic_uid,
    encapsulation::{BasicOffsetTablePolicy, EncapsulatedPixelData},
    generation_backends::{
        ControlledMetadata, FLOAT32_SPEC, FLOAT64_SPEC, ParametricMapGenerationInput,
        ParametricMapIdentities, ParametricMapPayload, ParametricMapSampleKind,
        ParametricMapSource, ParametricMapSpec, ParametricMapVariantGenerated,
        ParametricMapVariantOutcome, Scoord3dGenerated, Scoord3dGenerationInput,
        Scoord3dIdentities, Scoord3dOutcome, StandardsProvenance, Tid1500Generated,
        Tid1500GenerationInput, Tid1500Identities, Tid1500Outcome, WSI_TILE_SEGMENTATION_CASE_ID,
        WSI_TILE_SEGMENTATION_FRAME_SHA256, WSI_TILE_SEGMENTATION_OUTPUT_FILE,
        WSI_TILE_SEGMENTATION_RECIPE_ID, WSI_TILE_SEGMENTATION_RECIPE_VERSION,
        WSI_TILE_SEGMENTATION_SOURCE_CASE_ID, WSI_TILE_SEGMENTATION_SOURCE_FRAME_NUMBERS,
        WsiTileSegmentationGenerated, WsiTileSegmentationGenerationInput,
        WsiTileSegmentationIdentities, WsiTileSegmentationOutcome,
        generate_parametric_map_for_spec, generate_scoord3d, generate_tid1500,
        generate_wsi_tile_segmentation,
    },
    mutation::MUTATION_CONTRACT_VERSION,
    negative::{NEGATIVE_CASE_IDS, NegativeOutput, build_negative_case},
    rt_manifest::{
        LinkedRtImageInput, LinkedRtPlanInput, linked_rt_image_expected, linked_rt_plan_expected,
    },
    rt_radiation_manifest::{
        CArmRtRadiationInput, RtRadiationSetInput, minimal_carm_rt_radiation_expected,
        minimal_rt_radiation_set_expected,
    },
    sha256_hex,
    stress::{
        ResourceObservation, STRESS_CONTRACT_VERSION, StressExecutionOutcome,
        StressQualificationRecord, StressRecipeKind, StressRequest, StressResourceGuard,
        StressScale,
    },
    validation::{
        AdvancedBlendingPresentationStateExpectations, AdvancedBlendingSourceSeriesExpectations,
        BasicTextSrExpectations, BlendingPresentationStateExpectations,
        BlendingSourceSeriesExpectations, ColorSoftcopyPresentationStateExpectations,
        CtImageExpectations, DeformableSpatialRegistrationExpectations,
        EncapsulatedPdfExpectations, GeneralEcgExpectations, Part10Expectations,
        PixelDataLengthFormula, PresentationStateExpectations, RealWorldValueMappingExpectations,
        RtDoseExpectations, RtImageExpectations, RtPlanExpectations, RtRadiationExpectations,
        RtRadiationSetExpectations, RtStructureSetExpectations, Scoord3dExpectations,
        SegmentationExpectations, SpatialRegistrationExpectations,
        SpatialRegistrationReferenceExpectations, Tid1500Expectations, TwelveLeadEcgExpectations,
        WsiTileSegmentationExpectations, validate_advanced_blending_presentation_state_file,
        validate_basic_text_sr_file, validate_blending_presentation_state_file,
        validate_color_softcopy_presentation_state_file, validate_comprehensive_sr_file,
        validate_deformable_spatial_registration_file, validate_encapsulated_pdf_file,
        validate_general_ecg_file, validate_key_object_selection_file, validate_part10_file,
        validate_presentation_state_file, validate_real_world_value_mapping_file,
        validate_rt_dose_file, validate_rt_image_file, validate_rt_plan_file,
        validate_rt_radiation_file, validate_rt_radiation_set_file, validate_rt_structure_set_file,
        validate_scoord3d_file, validate_spatial_registration_file, validate_tid1500_file,
        validate_twelve_lead_ecg_file, validate_wsi_tile_segmentation_file,
    },
    waveform_manifest::{general_ecg_expected_waveform, twelve_lead_ecg_expected_waveform},
};

#[cfg(feature = "jpeg")]
use crate::encapsulation::encapsulate_frames;

#[cfg(feature = "deflate")]
use crate::codecs::DicomRsDeflatedImageFrameEncoder;
#[cfg(feature = "jpeg")]
use crate::codecs::DicomRsJpegBaselineEncoder;
#[cfg(feature = "charls")]
use crate::codecs::DicomRsJpegLsLosslessEncoder;
#[cfg(feature = "jpeg2000")]
use crate::codecs::OpenJp2Jpeg2000LosslessEncoder;
#[cfg(feature = "jpegxl")]
use crate::codecs::{CjxlJpegXlLossyEncoder, DicomRsJpegXlLosslessEncoder};
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
use crate::codecs::{FrameDecodeInput, FrameDecoder, calculate_lossy_frame_metrics};
#[cfg(feature = "htj2k_openjph")]
use crate::codecs::{OpenJphHtj2kLosslessEncoder, OpenJphHtj2kLossyEncoder};
#[cfg(feature = "legacy_jpeg_dcmtk")]
use dicom_encoding::{Codec, adapters::PixelDataReader};
#[cfg(feature = "legacy_jpeg_dcmtk")]
use dicom_transfer_syntax_registry::entries::{
    JPEG_LOSSLESS_NON_HIERARCHICAL, JPEG_LOSSLESS_NON_HIERARCHICAL_FIRST_ORDER_PREDICTION,
};

const PIXEL_RECIPE_VERSION: &str = "0.1.0";
const CLASSIC_CT_RECIPE_VERSION: &str = "0.1.0";
const SEGMENTATION_RECIPE_VERSION: &str = "0.1.0";
const GSPS_RECIPE_VERSION: &str = "0.1.0";
const RWVM_RECIPE_VERSION: &str = "0.1.0";
const BASIC_TEXT_SR_RECIPE_VERSION: &str = "0.1.0";
const COMPREHENSIVE_SR_RECIPE_VERSION: &str = "0.1.0";
const KEY_OBJECT_SELECTION_RECIPE_VERSION: &str = "0.2.0";
const RT_STRUCTURE_SET_RECIPE_VERSION: &str = "0.1.0";
const RT_DOSE_RECIPE_VERSION: &str = "0.1.0";
const RT_PLAN_RECIPE_VERSION: &str = "0.1.0";
const RT_IMAGE_RECIPE_VERSION: &str = "0.1.0";
const RT_RADIATION_RECIPE_VERSION: &str = "0.1.0";
const RT_RADIATION_SET_RECIPE_VERSION: &str = "0.1.0";
const ENCAPSULATED_PDF_RECIPE_VERSION: &str = "0.1.0";
const ENCAPSULATED_STL_RECIPE_VERSION: &str = "0.1.0";
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
const ENCAPSULATED_STL_STORAGE_UID: &str = "1.2.840.10008.5.1.4.1.1.104.3";
const PARAMETRIC_MAP_RECIPE_VERSION: &str = "0.1.0";
const PARAMETRIC_MAP_SOP_CLASS_UID: &str = "1.2.840.10008.5.1.4.1.1.30";
const PARAMETRIC_MAP_TRANSFER_SYNTAX_UID: &str = "1.2.840.10008.1.2.1";
const PARAMETRIC_MAP_SOURCE_CASE_ID: &str = "geometry/ct/spatial_sort_conflicts_instance_number";
const PARAMETRIC_MAP_STORED_VALUE_SCALE: f32 = 0.25;
const PARAMETRIC_MAP_FLOAT32_SPATIAL_RANK_INCREMENT: f32 = 0.25;
const PARAMETRIC_MAP_FLOAT64_SPATIAL_RANK_INCREMENT: f32 = 9.313_226e-10;
static PARAMETRIC_MAP_STAGING_COUNTER: AtomicU64 = AtomicU64::new(0);
const TID1500_CASE_ID: &str = "derived/sr/tid1500_ct_measurement_report";
const TID1500_RECIPE_ID: &str = "derived_sr_tid1500_ct_measurement_report";
const TID1500_RECIPE_VERSION: &str = "0.1.0";
const TID1500_OUTPUT_FILE: &str = "measurement-report.dcm";
const TID1500_SOP_CLASS_UID: &str = "1.2.840.10008.5.1.4.1.1.88.34";
const TID1500_CT_SOURCE_CASE_ID: &str = "enhanced/ct/multiframe_shared_perframe_explicit_le";
const TID1500_SEG_SOURCE_CASE_ID: &str = "derived/seg/binary_multiframe_explicit_le";
const SCOORD3D_CASE_ID: &str = "derived/sr/comprehensive3d_scoord3d";
const SCOORD3D_RECIPE_ID: &str = "derived_sr_comprehensive3d_scoord3d";
const SCOORD3D_RECIPE_VERSION: &str = "0.1.0";
const SCOORD3D_OUTPUT_FILE: &str = "scoord3d-report.dcm";
const SCOORD3D_SOP_CLASS_UID: &str = "1.2.840.10008.5.1.4.1.1.88.34";
const SCOORD3D_CT_SOURCE_CASE_ID: &str = "enhanced/ct/multiframe_shared_perframe_explicit_le";
const SPATIAL_REGISTRATION_CASE_ID: &str = "derived/registration/spatial_ct_pair";
const SPATIAL_REGISTRATION_RECIPE_ID: &str = "derived_registration_spatial_ct_pair";
const SPATIAL_REGISTRATION_RECIPE_VERSION: &str = "0.1.0";
const SPATIAL_REGISTRATION_TARGET_CASE_ID: &str =
    "enhanced/ct/multiframe_shared_perframe_explicit_le";
const SPATIAL_REGISTRATION_SOURCE_CASE_ID: &str = "classic/ct/mono2_i16_rescale_12bit_explicit_le";
const DEFORMABLE_SPATIAL_REGISTRATION_CASE_ID: &str = "derived/registration/deformable_ct_pair";
const DEFORMABLE_SPATIAL_REGISTRATION_RECIPE_ID: &str = "derived_registration_deformable_ct_pair";
const DEFORMABLE_SPATIAL_REGISTRATION_RECIPE_VERSION: &str = "0.1.0";
const COLOR_SOFTCOPY_PRESENTATION_STATE_CASE_ID: &str = "derived/presentation-state/color_softcopy";
const COLOR_SOFTCOPY_PRESENTATION_STATE_RECIPE_ID: &str =
    "derived_presentation_state_color_softcopy";
const COLOR_SOFTCOPY_PRESENTATION_STATE_RECIPE_VERSION: &str = "0.1.0";
const COLOR_SOFTCOPY_PRESENTATION_STATE_SOURCE_CASE_ID: &str = "classic/sc/rgb_planar0_explicit_le";
const ADVANCED_BLENDING_PRESENTATION_STATE_CASE_ID: &str =
    "derived/presentation-state/advanced_blending";
const ADVANCED_BLENDING_PRESENTATION_STATE_RECIPE_ID: &str =
    "derived_presentation_state_advanced_blending";
const ADVANCED_BLENDING_PRESENTATION_STATE_RECIPE_VERSION: &str = "0.1.0";
const ADVANCED_BLENDING_PRESENTATION_STATE_SOURCE_CASE_ID: &str =
    "geometry/ct/multiseries_shared_frame_of_reference";
const BLENDING_PRESENTATION_STATE_CASE_ID: &str = "derived/presentation-state/blending";
const BLENDING_PRESENTATION_STATE_RECIPE_ID: &str = "derived_presentation_state_blending";
const BLENDING_PRESENTATION_STATE_RECIPE_VERSION: &str = "0.1.0";
const BLENDING_PRESENTATION_STATE_SOURCE_CASE_ID: &str =
    "geometry/ct/multiseries_shared_frame_of_reference";
const BLENDING_PRESENTATION_STATE_PALETTE_SHA256: &str =
    "f393097e80ec38db493eb054a0886181eb2c0e8cf7b5cdf1de392fbe94b0d1f5";
const TWELVE_LEAD_ECG_CASE_ID: &str = "non-image/waveform/twelve_lead_ecg";
const TWELVE_LEAD_ECG_RECIPE_ID: &str = "non_image_waveform_twelve_lead_ecg";
const TWELVE_LEAD_ECG_RECIPE_VERSION: &str = "0.1.0";
const GENERAL_ECG_CASE_ID: &str = "non-image/waveform/general_ecg";
const GENERAL_ECG_RECIPE_ID: &str = "non_image_waveform_general_ecg";
const GENERAL_ECG_RECIPE_VERSION: &str = "0.1.0";
const DEFORMABLE_VECTOR_GRID_DATA_SHA256: &str =
    "d0673d2da1b415db6465047e607b7f16f1a886dfae4ede91764c71bf7df72f47";
const DEFORMABLE_REGISTERED_POINTS_MM: [[f64; 3]; 4] = [
    [0.0, 0.0, 2.5],
    [0.75, 0.0, 2.5],
    [0.0, 0.75, 2.5],
    [0.75, 0.75, 2.5],
];
const DEFORMABLE_SOURCE_POINTS_MM: [[f64; 3]; 4] = [
    [-0.625, -0.625, 0.0],
    [0.0, -0.625, 0.0],
    [-0.625, 0.0, 0.0],
    [0.0, 0.0, 0.0],
];
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TransferSyntaxSpec {
    capability_keyword: &'static str,
    capability_name: &'static str,
    uid: &'static str,
    name: &'static str,
}

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
const JPEG_XL_LOSSY: TransferSyntaxSpec = TransferSyntaxSpec {
    capability_keyword: "JPEGXL",
    capability_name: "JPEG XL",
    uid: JPEG_XL_LOSSY_TRANSFER_SYNTAX_UID,
    name: "JPEG XL",
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
const HTJ2K_LOSSY: TransferSyntaxSpec = TransferSyntaxSpec {
    capability_keyword: "HTJ2K",
    capability_name: "High-Throughput JPEG 2000 Image Compression",
    uid: HTJ2K_LOSSY_TRANSFER_SYNTAX_UID,
    name: "HTJ2K",
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
const RT_PLAN_CASE_ID: &str = "non-image/rt/plan_linked";
const RT_PLAN_RECIPE_ID: &str = "non_image_rt_plan_linked";
const RT_PLAN_STRUCTURE_SET_SOURCE_CASE_ID: &str =
    "non-image/rt/structure_set_single_roi_explicit_le";
const RT_PLAN_DOSE_SOURCE_CASE_ID: &str = "non-image/rt/dose_grid_u16_explicit_le";
const RT_IMAGE_CASE_ID: &str = "non-image/rt/image_linked";
const RT_IMAGE_RECIPE_ID: &str = "non_image_rt_image_linked";
const RT_IMAGE_PLAN_SOURCE_CASE_ID: &str = RT_PLAN_CASE_ID;
const RT_RADIATION_CASE_ID: &str = "non-image/rt/carm_photon_electron_radiation_minimal";
const RT_RADIATION_RECIPE_ID: &str = "non_image_rt_carm_photon_electron_radiation_minimal";
const RT_RADIATION_PLAN_SOURCE_CASE_ID: &str = RT_PLAN_CASE_ID;
const RT_RADIATION_SET_CASE_ID: &str = "non-image/rt/radiation_set_minimal";
const RT_RADIATION_SET_RECIPE_ID: &str = "non_image_rt_radiation_set_minimal";
const MONO_PIXELS: [u8; 4] = [0, 85, 170, 255];
const MONO_MULTIFRAME_PIXELS: [u8; 8] = [0, 85, 170, 255, 255, 170, 85, 0];
const MONO_MULTIFRAME_VALUES: [i32; 8] = [0, 85, 170, 255, 255, 170, 85, 0];
const EOT_CASE_ID: &str = "encapsulation/sc/eot_single_fragment_multiframe";
const EOT_MULTIFRAME_PIXELS: [u8; 12] = [0, 85, 170, 255, 17, 17, 17, 17, 255, 170, 85, 0];
const EOT_MULTIFRAME_VALUES: [i32; 12] = [0, 85, 170, 255, 17, 17, 17, 17, 255, 170, 85, 0];
const EOT_ENCODED_LENGTHS: [u64; 3] = [69, 66, 69];
const EOT_OFFSETS: [u64; 3] = [0, 78, 152];
const MONO_ODD_RLE_PIXELS: [u8; 2] = [0, 255];
const MONO_ODD_RLE_VALUES: [i32; 2] = [0, 255];
const RGB_PLANAR0_PIXELS: [u8; 12] = [255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255];
const LOSSY_DIAGNOSTIC_ROWS: u16 = 32;
const LOSSY_DIAGNOSTIC_COLUMNS: u16 = 32;
const JPEG_XL_LOSSY_PIXELS: [u8; 32 * 32 * 3] = jpeg_xl_lossy_diagnostic_pixels();
const HTJ2K_LOSSY_PIXELS: [u8; 32 * 32 * 2] = htj2k_lossy_diagnostic_pixels();

const fn jpeg_xl_lossy_diagnostic_pixels() -> [u8; 32 * 32 * 3] {
    let mut pixels = [0; 32 * 32 * 3];
    let mut row = 0_u16;
    while row < 32 {
        let mut column = 0_u16;
        while column < 32 {
            let bar = (column / 4) as u8;
            let offset = ((row as usize) * 32 + column as usize) * 3;
            pixels[offset] = if row < 16 {
                (column * 8) as u8
            } else {
                bar * 32
            };
            pixels[offset + 1] = if column < 16 {
                (row * 8) as u8
            } else {
                255 - bar * 32
            };
            pixels[offset + 2] = if (row / 4 + column / 4) % 2 == 0 {
                16
            } else {
                240
            };
            column += 1;
        }
        row += 1;
    }
    pixels
}

const fn htj2k_lossy_diagnostic_pixels() -> [u8; 32 * 32 * 2] {
    let mut pixels = [0; 32 * 32 * 2];
    let mut row = 0_u32;
    while row < 32 {
        let mut column = 0_u32;
        while column < 32 {
            let sample = if row < 8 {
                let value = column * 2048;
                if value < 65535 { value } else { 65535 }
            } else if row < 16 {
                if column < 16 { 0 } else { 65535 }
            } else if (row / 4 + column / 4) % 2 == 0 {
                4096
            } else {
                61440
            } as u16;
            let bytes = sample.to_le_bytes();
            let offset = ((row as usize) * 32 + column as usize) * 2;
            pixels[offset] = bytes[0];
            pixels[offset + 1] = bytes[1];
            column += 1;
        }
        row += 1;
    }
    pixels
}
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
const SEG_BINARY_CONTINUOUS_PIXELS: [u8; 2] = [0b0110_1001, 0];
const SEG_BINARY_ENCAPSULATED_FRAMES: [u8; 2] = [0b0000_1001, 0b0000_0110];
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
const COLOR_SOFTCOPY_PRIVATE_SOURCE_PIXEL_RECIPE: PixelRecipe = PixelRecipe {
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
};
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
        case_id: EOT_CASE_ID,
        recipe_id: "encapsulation_sc_eot_single_fragment_multiframe",
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
        pixel_bytes: &EOT_MULTIFRAME_PIXELS,
        pixel_values: &EOT_MULTIFRAME_VALUES,
        pixel_min: 0,
        pixel_max: 255,
        visual_pattern: "2x2x3_monochrome_rle_lossless_literal_repeat_reverse",
        semantic_note: "three MONOCHROME2 frames use exact Extended Offset Table frame boundaries",
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
        case_id: "classic/sc/rgb_jpegxl_lossy",
        recipe_id: "classic_sc_rgb_jpegxl_lossy",
        rows: LOSSY_DIAGNOSTIC_ROWS,
        columns: LOSSY_DIAGNOSTIC_COLUMNS,
        photometric_interpretation: "RGB",
        samples_per_pixel: 3,
        planar_configuration: Some(0),
        bits_allocated: 8,
        bits_stored: 8,
        high_bit: 7,
        pixel_representation: 0,
        pixel_vr: VR::OB,
        transfer_syntax: JPEG_XL_LOSSY,
        pixel_bytes: &JPEG_XL_LOSSY_PIXELS,
        pixel_values: &[],
        pixel_min: 0,
        pixel_max: 248,
        visual_pattern: "32x32_rgb_gradients_bars_checkerboard",
        semantic_note: "interleaved RGB diagnostic samples are decoded within the approved JPEG XL lossy error bounds",
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
        case_id: "classic/sc/mono2_u16_htj2k_lossy",
        recipe_id: "classic_sc_mono2_u16_htj2k_lossy",
        rows: LOSSY_DIAGNOSTIC_ROWS,
        columns: LOSSY_DIAGNOSTIC_COLUMNS,
        photometric_interpretation: "MONOCHROME2",
        samples_per_pixel: 1,
        planar_configuration: None,
        bits_allocated: 16,
        bits_stored: 16,
        high_bit: 15,
        pixel_representation: 0,
        pixel_vr: VR::OB,
        transfer_syntax: HTJ2K_LOSSY,
        pixel_bytes: &HTJ2K_LOSSY_PIXELS,
        pixel_values: &[],
        pixel_min: 0,
        pixel_max: 65535,
        visual_pattern: "32x32_monochrome_gradient_edges_checkerboard",
        semantic_note: "unsigned MONOCHROME2 diagnostic samples are decoded within the approved HTJ2K lossy error bounds",
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
    PixelRecipe {
        case_id: U1_SC_RECIPE.case_id,
        recipe_id: U1_SC_RECIPE.recipe_id,
        rows: U1_SC_RECIPE.rows,
        columns: U1_SC_RECIPE.columns,
        photometric_interpretation: "MONOCHROME2",
        samples_per_pixel: 1,
        planar_configuration: None,
        bits_allocated: 1,
        bits_stored: 1,
        high_bit: 0,
        pixel_representation: 0,
        pixel_vr: VR::OB,
        transfer_syntax: EXPLICIT_VR_LITTLE_ENDIAN,
        pixel_bytes: U1_SC_RECIPE.packed_pixel_bytes,
        pixel_values: U1_SC_RECIPE.pixel_values,
        pixel_min: 0,
        pixel_max: 1,
        visual_pattern: "3x3x2_continuous_lsb_first_checkerboards",
        semantic_note: "two one-bit MONOCHROME2 frames cross a byte boundary and receive padding only at the end of the complete Value Field",
        palette: None,
        padding: None,
    },
    PixelRecipe {
        case_id: U32_SC_RECIPE.case_id,
        recipe_id: U32_SC_RECIPE.recipe_id,
        rows: U32_SC_RECIPE.rows,
        columns: U32_SC_RECIPE.columns,
        photometric_interpretation: "MONOCHROME2",
        samples_per_pixel: 1,
        planar_configuration: None,
        bits_allocated: 32,
        bits_stored: 32,
        high_bit: 31,
        pixel_representation: 0,
        pixel_vr: VR::OW,
        transfer_syntax: EXPLICIT_VR_LITTLE_ENDIAN,
        pixel_bytes: U32_SC_RECIPE.pixel_bytes_le,
        pixel_values: &[],
        pixel_min: 0,
        pixel_max: 4_294_967_295,
        visual_pattern: "2x2_monochrome_u32_unsigned_boundaries",
        semantic_note: "unsigned 32-bit MONOCHROME2 samples span both sides of the signed boundary",
        palette: None,
        padding: None,
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
    pixel_min: i64,
    pixel_max: i64,
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
        pixel_data_length_formula: PixelDataLengthFormula::BitPackedContinuousFrames,
        pixel_bytes: &SEG_BINARY_CONTINUOUS_PIXELS,
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
        pixel_bytes: &SEG_BINARY_ENCAPSULATED_FRAMES,
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
struct RtPlanRecipe {
    case_id: &'static str,
    recipe_id: &'static str,
    structure_set_source_case_id: &'static str,
    dose_source_case_id: &'static str,
}

const RT_PLAN_RECIPES: &[RtPlanRecipe] = &[RtPlanRecipe {
    case_id: RT_PLAN_CASE_ID,
    recipe_id: RT_PLAN_RECIPE_ID,
    structure_set_source_case_id: RT_PLAN_STRUCTURE_SET_SOURCE_CASE_ID,
    dose_source_case_id: RT_PLAN_DOSE_SOURCE_CASE_ID,
}];

#[derive(Debug, Clone, Copy)]
struct RtImageRecipe {
    case_id: &'static str,
    recipe_id: &'static str,
    plan_source_case_id: &'static str,
}

const RT_IMAGE_RECIPES: &[RtImageRecipe] = &[RtImageRecipe {
    case_id: RT_IMAGE_CASE_ID,
    recipe_id: RT_IMAGE_RECIPE_ID,
    plan_source_case_id: RT_IMAGE_PLAN_SOURCE_CASE_ID,
}];

#[derive(Debug, Clone, Copy)]
struct RtRadiationRecipe {
    case_id: &'static str,
    recipe_id: &'static str,
    plan_source_case_id: &'static str,
}

const RT_RADIATION_RECIPES: &[RtRadiationRecipe] = &[RtRadiationRecipe {
    case_id: RT_RADIATION_CASE_ID,
    recipe_id: RT_RADIATION_RECIPE_ID,
    plan_source_case_id: RT_RADIATION_PLAN_SOURCE_CASE_ID,
}];

#[derive(Debug, Clone, Copy)]
struct RtRadiationSetRecipe {
    case_id: &'static str,
    recipe_id: &'static str,
    plan_source_case_id: &'static str,
    radiation_source_case_id: &'static str,
}

const RT_RADIATION_SET_RECIPES: &[RtRadiationSetRecipe] = &[RtRadiationSetRecipe {
    case_id: RT_RADIATION_SET_CASE_ID,
    recipe_id: RT_RADIATION_SET_RECIPE_ID,
    plan_source_case_id: RT_PLAN_CASE_ID,
    radiation_source_case_id: RT_RADIATION_CASE_ID,
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
struct EncapsulatedStlRecipe {
    case_id: &'static str,
    recipe_id: &'static str,
    document_title: &'static str,
    content_description: &'static str,
}

const ENCAPSULATED_STL_RECIPES: &[EncapsulatedStlRecipe] = &[EncapsulatedStlRecipe {
    case_id: "derived/mesh/encapsulated_stl",
    recipe_id: "derived_mesh_encapsulated_stl",
    document_title: "DTS Synthetic Closed Tetrahedron",
    content_description: "Deterministic closed tetrahedron manufacturing model",
}];

#[derive(Debug, Clone)]
pub(crate) struct GeneratedFile {
    pub case_id: String,
    pub manifest_entry: Value,
}

#[derive(Debug, Default)]
pub(crate) struct GenerationOutput {
    pub files: Vec<GeneratedFile>,
    pub unavailable_cases: Vec<Value>,
    pub qualifications: Vec<Value>,
    pub completed_case_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GeneratedSourceObject {
    pub source_case_id: String,
    pub source_path: String,
    pub sha256: String,
    pub study_instance_uid: String,
    pub sop_class_uid: String,
    pub sop_instance_uid: String,
    pub series_instance_uid: Option<String>,
    pub frame_of_reference_uid: Option<String>,
    pub frame_count: Option<u64>,
    pub specimen_uid: Option<String>,
    pub container_identifier: Option<String>,
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
        let sha256 = generated_manifest_str(
            &file.manifest_entry,
            "/sha256",
            "generated file manifest sha256 must be a string",
        )?;
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
        let specimen_uid = file
            .manifest_entry
            .pointer("/expected_wsi_tiled_full/specimen/specimen_uid")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned);
        let container_identifier = file
            .manifest_entry
            .pointer("/expected_wsi_tiled_full/specimen/container_identifier")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned);

        Ok(Self {
            source_case_id: source_case_id.to_string(),
            source_path: source_path.to_string(),
            sha256: sha256.to_string(),
            study_instance_uid: study_instance_uid.to_string(),
            sop_class_uid: sop_class_uid.to_string(),
            sop_instance_uid: sop_instance_uid.to_string(),
            series_instance_uid,
            frame_of_reference_uid,
            frame_count,
            specimen_uid,
            container_identifier,
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
    unavailable_cases: Vec<Value>,
    qualifications: Vec<Value>,
    stress_guard: Option<StressResourceGuard>,
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

    fn record_qualification(&mut self, qualification: Value) {
        self.qualifications.push(qualification);
    }

    fn preflight_stress(
        &mut self,
        recipe: StressRecipeKind,
        planned_output_bytes: u64,
        planned_peak_rss_bytes: u64,
    ) -> Result<(StressRequest, Instant), GenerateError> {
        let request = StressRequest::approved(recipe, StressScale::Reduced);
        self.stress_guard
            .get_or_insert_with(|| StressResourceGuard::new(StressScale::Reduced))
            .preflight(request, planned_output_bytes, planned_peak_rss_bytes)
            .map_err(|error| GenerateError::WriteDicomFile {
                path: PathBuf::from(recipe.name()),
                message: format!("reduced stress preflight: {error}"),
            })?;
        Ok((request, Instant::now()))
    }

    fn record_stress_files(
        &mut self,
        case_id: &str,
        request: StressRequest,
        started: Instant,
        files: Vec<GeneratedFile>,
    ) -> Result<(), GenerateError> {
        let output_bytes = files
            .iter()
            .map(|file| {
                file.manifest_entry
                    .get("size_bytes")
                    .and_then(Value::as_u64)
                    .ok_or_else(|| GenerateError::MetadataShape {
                        path: PathBuf::from(case_id),
                        message: "stress generated file size_bytes must be an integer",
                    })
            })
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .try_fold(0_u64, u64::checked_add)
            .ok_or_else(|| GenerateError::MetadataShape {
                path: PathBuf::from(case_id),
                message: "stress output byte accounting overflowed",
            })?;
        let observation = ResourceObservation {
            output_bytes,
            elapsed_milliseconds: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
            peak_rss_bytes: None,
        };
        self.stress_guard
            .get_or_insert_with(|| StressResourceGuard::new(StressScale::Reduced))
            .record_case(request, observation)
            .map_err(|error| GenerateError::WriteDicomFile {
                path: PathBuf::from(case_id),
                message: format!("reduced stress observation: {error}"),
            })?;
        let mut actual = request.parameters;
        actual.output_bytes = output_bytes;
        let qualification = StressQualificationRecord {
            contract_version: STRESS_CONTRACT_VERSION,
            request,
            actual,
            observation,
            outcome: StressExecutionOutcome::Completed,
        };
        if !qualification.is_promotable() {
            return Err(GenerateError::MetadataShape {
                path: PathBuf::from(case_id),
                message: "reduced stress output is not promotable under its approved contract",
            });
        }
        self.record_many(files)?;
        self.record_qualification(qualification.to_manifest_value(case_id));
        Ok(())
    }

    #[allow(dead_code)]
    pub(crate) fn source_registry(&self) -> &GeneratedSourceRegistry {
        &self.source_registry
    }

    fn into_output(self) -> GenerationOutput {
        GenerationOutput {
            files: self.generated_files,
            unavailable_cases: self.unavailable_cases,
            qualifications: self.qualifications,
            completed_case_ids: Vec::new(),
        }
    }
}

const NEGATIVE_NATIVE_SOURCE_CASE_ID: &str = "classic/sc/mono2_u8_explicit_le";
const NEGATIVE_CHARSET_SOURCE_CASE_ID: &str = "metadata/sc/utf8_person_name";
const NEGATIVE_NESTED_SOURCE_CASE_ID: &str = "metadata/sc/defined_undefined_sequence_lengths";
const NEGATIVE_RLE_SOURCE_CASE_ID: &str = "classic/sc/mono1_u8_rle_lossless";

struct NegativeSourceStagingGuard(PathBuf);

impl NegativeSourceStagingGuard {
    fn new(run: &PreparedGenerationRun) -> Self {
        Self(run.out_dir.join(".negative-private-sources"))
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for NegativeSourceStagingGuard {
    fn drop(&mut self) {
        if self.0.exists() {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
}

#[derive(Debug)]
struct NegativeSourceArtifact {
    bytes: Vec<u8>,
}

fn write_negative_cases(
    run: &PreparedGenerationRun,
    registry: &Value,
    standards_lock_sha256: &str,
) -> Result<GenerationOutput, GenerateError> {
    let mut selected_cases = Vec::new();
    for case_id in NEGATIVE_CASE_IDS {
        let Some(case) = registry_case(registry, case_id)? else {
            continue;
        };
        if should_generate_case(case, run)? {
            selected_cases.push((*case_id, case));
        }
    }
    if selected_cases.is_empty() {
        return Ok(GenerationOutput::default());
    }

    let source_staging = NegativeSourceStagingGuard::new(run);
    let source_run = PreparedGenerationRun {
        profile: "negative-private-source".to_string(),
        out_dir: source_staging.path().to_path_buf(),
        manifest_path: source_staging.path().join("manifest.json"),
        seed: run.seed,
        include_stress: false,
    };
    let sources = write_negative_source_artifacts(
        &source_run,
        registry,
        standards_lock_sha256,
        source_staging.path(),
    )?;

    let mut generated = Vec::with_capacity(selected_cases.len());
    for (case_id, case) in selected_cases {
        let expected_source_case_id = negative_source_case_id(case_id);
        let source =
            sources
                .get(expected_source_case_id)
                .ok_or_else(|| GenerateError::MetadataShape {
                    path: PathBuf::from(case_id),
                    message: "negative recipe source was not generated in private staging",
                })?;
        let output = build_negative_case(case_id, &source.bytes).map_err(|error| {
            GenerateError::WriteDicomFile {
                path: PathBuf::from(case_id),
                message: error.to_string(),
            }
        })?;
        generated.push(write_negative_output(run, case, source, output)?);
    }
    drop(source_staging);
    if run.out_dir.join(".negative-private-sources").exists() {
        return Err(GenerateError::MetadataShape {
            path: run.out_dir.clone(),
            message: "private negative source staging survived cleanup",
        });
    }
    Ok(GenerationOutput {
        files: generated,
        unavailable_cases: Vec::new(),
        qualifications: Vec::new(),
        completed_case_ids: Vec::new(),
    })
}

fn negative_source_case_id(case_id: &str) -> &'static str {
    match case_id {
        "negative/charset/malformed_encoded_text" => NEGATIVE_CHARSET_SOURCE_CASE_ID,
        "negative/dataset/invalid_nested_item_length"
        | "negative/dataset/truncated_sequence_item"
        | "negative/dataset/undefined_length_without_delimitation" => {
            NEGATIVE_NESTED_SOURCE_CASE_ID
        }
        "negative/encapsulation/broken_offset_table" => EOT_CASE_ID,
        "negative/encapsulation/truncated_fragment" => NEGATIVE_RLE_SOURCE_CASE_ID,
        _ => NEGATIVE_NATIVE_SOURCE_CASE_ID,
    }
}

fn write_negative_source_artifacts(
    run: &PreparedGenerationRun,
    registry: &Value,
    standards_lock_sha256: &str,
    staging_root: &Path,
) -> Result<BTreeMap<&'static str, NegativeSourceArtifact>, GenerateError> {
    let source_case = |case_id| {
        registry_case(registry, case_id)?.ok_or_else(|| GenerateError::MetadataShape {
            path: PathBuf::from(case_id),
            message: "negative source case is missing from the registry",
        })
    };
    let pixel_recipe = |case_id| {
        PIXEL_RECIPES
            .iter()
            .copied()
            .find(|recipe| recipe.case_id == case_id)
            .ok_or_else(|| GenerateError::MetadataShape {
                path: PathBuf::from(case_id),
                message: "negative source Pixel recipe is unavailable",
            })
    };

    let native = write_pixel_case(
        run,
        source_case(NEGATIVE_NATIVE_SOURCE_CASE_ID)?,
        pixel_recipe(NEGATIVE_NATIVE_SOURCE_CASE_ID)?,
        standards_lock_sha256,
    )?;
    let charset_recipe = METADATA_SC_RECIPES
        .iter()
        .copied()
        .find(|recipe| recipe.pixel.case_id == NEGATIVE_CHARSET_SOURCE_CASE_ID)
        .ok_or_else(|| GenerateError::MetadataShape {
            path: PathBuf::from(NEGATIVE_CHARSET_SOURCE_CASE_ID),
            message: "negative UTF-8 source recipe is unavailable",
        })?;
    let charset = write_metadata_sc_case(
        run,
        source_case(NEGATIVE_CHARSET_SOURCE_CASE_ID)?,
        charset_recipe,
        standards_lock_sha256,
    )?;
    let nested = write_sequence_length_sc_case(
        run,
        source_case(NEGATIVE_NESTED_SOURCE_CASE_ID)?,
        SEQUENCE_LENGTH_SC_RECIPE,
        standards_lock_sha256,
    )?
    .into_iter()
    .find(|file| {
        file.manifest_entry
            .pointer("/path")
            .and_then(Value::as_str)
            .is_some_and(|path| path.ends_with("/undefined.dcm"))
    })
    .ok_or_else(|| GenerateError::MetadataShape {
        path: PathBuf::from(NEGATIVE_NESTED_SOURCE_CASE_ID),
        message: "undefined-length negative source variant was not generated",
    })?;
    let rle = write_pixel_case(
        run,
        source_case(NEGATIVE_RLE_SOURCE_CASE_ID)?,
        pixel_recipe(NEGATIVE_RLE_SOURCE_CASE_ID)?,
        standards_lock_sha256,
    )?;
    let eot = write_pixel_case(
        run,
        source_case(EOT_CASE_ID)?,
        pixel_recipe(EOT_CASE_ID)?,
        standards_lock_sha256,
    )?;

    let mut sources = BTreeMap::new();
    for (case_id, file) in [
        (NEGATIVE_NATIVE_SOURCE_CASE_ID, native),
        (NEGATIVE_CHARSET_SOURCE_CASE_ID, charset),
        (NEGATIVE_NESTED_SOURCE_CASE_ID, nested),
        (NEGATIVE_RLE_SOURCE_CASE_ID, rle),
        (EOT_CASE_ID, eot),
    ] {
        let relative_path = generated_manifest_str(
            &file.manifest_entry,
            "/path",
            "private negative source path must be a string",
        )?;
        let bytes = fs::read(staging_root.join(relative_path)).map_err(|source| {
            GenerateError::ReadMetadata {
                path: staging_root.join(relative_path),
                source,
            }
        })?;
        if sources
            .insert(case_id, NegativeSourceArtifact { bytes })
            .is_some()
        {
            return Err(GenerateError::MetadataShape {
                path: PathBuf::from(case_id),
                message: "negative source case IDs must be unique",
            });
        }
    }
    Ok(sources)
}

fn write_negative_output(
    run: &PreparedGenerationRun,
    case: &Value,
    source: &NegativeSourceArtifact,
    output: NegativeOutput,
) -> Result<GeneratedFile, GenerateError> {
    let case_id = output.evidence.case_id;
    let relative_path = format!("{case_id}/instance.dcm");
    let path = run.out_dir.join(&relative_path);
    fs::create_dir_all(path.parent().expect("negative case path has a parent")).map_err(
        |source| GenerateError::CreateCaseOutputDir {
            path: path.parent().unwrap().to_path_buf(),
            source,
        },
    )?;
    fs::write(&path, &output.bytes).map_err(|source| GenerateError::WriteDicomFile {
        path: path.clone(),
        message: source.to_string(),
    })?;
    let final_sha256 = sha256_hex(&output.bytes);
    if final_sha256 != output.evidence.output_sha256
        || output.evidence.source.sha256 != sha256_hex(&source.bytes)
        || output.evidence.source.expected_case_id != negative_source_case_id(case_id)
    {
        return Err(GenerateError::MetadataShape {
            path,
            message: "negative evidence does not bind the exact source and final bytes",
        });
    }

    let mutation_steps = output
        .evidence
        .steps
        .iter()
        .enumerate()
        .map(|(index, step)| {
            let ranges = step
                .changed_byte_ranges
                .iter()
                .map(|range| {
                    (
                        range.source.start,
                        range.source.end,
                        range.output.start,
                        range.output.end,
                    )
                })
                .collect::<Vec<_>>();
            serde_json::json!({
                "ordinal": index + 1,
                "mutation_id": step.mutation_id,
                "parameters": negative_mutation_parameters(step.mutation_id, &ranges, &output.bytes),
                "changed_byte_ranges": ranges.iter().map(|(source_start, source_end, output_start, output_end)| {
                    serde_json::json!({
                        "source": {"start": source_start, "end": source_end},
                        "output": {"start": output_start, "end": output_end}
                    })
                }).collect::<Vec<_>>(),
                "source_sha256": step.source_sha256,
                "output_sha256": step.output_sha256,
                "expected_failure_layer": failure_layer_name(&step.expected_failure_layer),
                "acceptable_outcomes": step.acceptable_outcomes.iter().map(acceptable_outcome_name).collect::<Vec<_>>()
            })
        })
        .collect::<Vec<_>>();
    let expected_failure_layer = output
        .evidence
        .steps
        .last()
        .map(|step| failure_layer_name(&step.expected_failure_layer))
        .expect("every negative recipe has at least one mutation step");
    let (probe_outcome, probe_detail) =
        super::classify_negative_rejection_probe(case_id, &output.bytes, expected_failure_layer);
    let recipe_id = case
        .get("recipe_id")
        .and_then(Value::as_str)
        .ok_or_else(|| GenerateError::MetadataShape {
            path: PathBuf::from(case_id),
            message: "negative registry recipe_id must be a string",
        })?;
    let source_transfer_syntax_uid = output.evidence.source.transfer_syntax_uid.clone();
    let standards_evidence = standards_evidence_from_case(case);
    Ok(GeneratedFile {
        case_id: case_id.to_string(),
        manifest_entry: serde_json::json!({
            "case_id": case_id,
            "profile_membership": ["negative"],
            "path": relative_path,
            "sha256": final_sha256,
            "size_bytes": output.bytes.len(),
            "determinism": "byte_stable",
            "validity": "expected_invalid",
            "provider": {
                "kind": "mutation_layer",
                "id": "checked_part10_mutation"
            },
            "recipe": {
                "recipe_id": recipe_id,
                "recipe_version": output.evidence.recipe_version,
                "recipe_parameters": {
                    "source_case_id": output.evidence.source.expected_case_id,
                    "mutation_operations": mutation_steps.iter().map(|step| step["mutation_id"].clone()).collect::<Vec<_>>()
                }
            },
            "standards_evidence": standards_evidence,
            "negative_evidence": {
                "contract_version": MUTATION_CONTRACT_VERSION,
                "recipe_version": output.evidence.recipe_version,
                "source": {
                    "case_id": output.evidence.source.expected_case_id,
                    "sha256": output.evidence.source.sha256,
                    "transfer_syntax_uid": source_transfer_syntax_uid,
                    "size_bytes": source.bytes.len()
                },
                "source_shape": output.evidence.source_shape,
                "mutation_steps": mutation_steps,
                "unacceptable_outcomes": ["timeout", "crash", "hang"],
                "probe": {
                    "kind": "same_project_bounded_parser_classifier",
                    "independence": "same_project",
                    "outcome": probe_outcome,
                    "detail": probe_detail
                },
                "final_sha256": output.evidence.output_sha256
            }
        }),
    })
}

fn negative_mutation_parameters(
    mutation_id: &str,
    ranges: &[(usize, usize, usize, usize)],
    output: &[u8],
) -> Value {
    let source_range = |index: usize| {
        let (start, end, _, _) = ranges[index];
        serde_json::json!({"start": start, "end": end})
    };
    let replacement = |index: usize| {
        let (_, _, start, end) = ranges[index];
        output.get(start..end).unwrap_or_default().to_vec()
    };
    let little_endian = |index: usize| {
        let bytes = replacement(index);
        bytes
            .iter()
            .enumerate()
            .fold(0_u64, |value, (shift, byte)| {
                value | (u64::from(*byte) << (shift * 8))
            })
    };
    let width = |index: usize| match ranges[index].1 - ranges[index].0 {
        2 => "u16",
        4 => "u32",
        8 => "u64",
        other => panic!("negative length field has unsupported width {other}"),
    };

    match mutation_id {
        id if id.starts_with("truncate_") => serde_json::json!({
            "target": id.trim_start_matches("truncate_"),
            "offset": ranges[0].0
        }),
        "incorrect_explicit_vr_length" => serde_json::json!({
            "length_field": source_range(0),
            "width": width(0),
            "declared_length": little_endian(0)
        }),
        "illegal_vr_bytes" => serde_json::json!({
            "vr_field": source_range(0),
            "replacement": replacement(0)
        }),
        "transfer_syntax_mismatch" => serde_json::json!({
            "file_meta_uid_value": source_range(0),
            "replacement": replacement(0)
        }),
        "file_meta_dataset_uid_mismatch" => serde_json::json!({
            "dataset_uid_value": source_range(0),
            "replacement": replacement(0)
        }),
        "missing_type_1_element" => serde_json::json!({"element": source_range(0)}),
        "invalid_bits_stored_high_bit" => serde_json::json!({
            "bits_stored_value": source_range(0),
            "high_bit_value": source_range(1),
            "bits_stored": little_endian(0),
            "high_bit": little_endian(1)
        }),
        "invalid_pixel_byte_length" => serde_json::json!({
            "length_field": source_range(0),
            "width": width(0),
            "declared_length": little_endian(0)
        }),
        "broken_basic_offset_table" => serde_json::json!({
            "entry": source_range(0),
            "offset": little_endian(0)
        }),
        "broken_extended_offset_table" => serde_json::json!({
            "entry": source_range(0),
            "offset": little_endian(0)
        }),
        "undefined_length_without_delimitation" => serde_json::json!({
            "length_field": Value::Null,
            "delimitation_item": source_range(ranges.len() - 1)
        }),
        "invalid_nested_item_length" => serde_json::json!({
            "length_field": source_range(0),
            "declared_length": little_endian(0)
        }),
        "invalid_character_set_declaration" | "malformed_encoded_text" => serde_json::json!({
            "value": source_range(0),
            "replacement": replacement(0)
        }),
        other => panic!("unknown negative mutation {other}"),
    }
}

fn failure_layer_name(layer: &impl std::fmt::Debug) -> &'static str {
    match format!("{layer:?}").as_str() {
        "FileMeta" => "file_meta",
        "DatasetParser" => "dataset_parser",
        "ValueDecoding" => "value_decoding",
        "SemanticValidation" => "semantic_validation",
        "PixelDecoding" => "pixel_decoding",
        "Encapsulation" => "encapsulation",
        "TextDecoding" => "text_decoding",
        other => panic!("unknown failure layer {other}"),
    }
}

fn acceptable_outcome_name(outcome: &impl std::fmt::Debug) -> &'static str {
    match format!("{outcome:?}").as_str() {
        "CleanRejection" => "clean_rejection",
        "ParseFailure" => "parse_failure",
        "ValidationFailure" => "validation_failure",
        "DecodeFailure" => "decode_failure",
        "AcceptedWithBoundedWarning" => "accepted_with_bounded_warning",
        other => panic!("unknown acceptable outcome {other}"),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CuratedRecipeStage {
    SecondaryCapture,
    ClassicCt,
    ClassicImagesBeforeEnhancedPet,
    ClassicImagesAfterEnhancedPet,
}

const PLAN_FIRST_NATIVE_VL_CASE_IDS: &[&str] = &[
    "vl/photo/rgb_planar0_explicit_le",
    "vl/endoscopic/rgb_explicit_le",
    "vl/microscopic/rgb_explicit_le",
    "vl/photo/rgb_icc_profile_explicit_le",
    "vl/photo/palette_color_explicit_le",
];

const PLAN_FIRST_RLE_VL_CASE_IDS: &[&str] = &[
    "vl/photo/rgb_planar0_rle_lossless",
    "vl/photo/rgb_planar1_rle_lossless",
    "vl/photo/palette_color_rle_lossless",
];

const PLAN_FIRST_NATIVE_VL_INSERT_AFTER_CASE_ID: &str = "classic/sc/mono1_u8_explicit_le";
const PLAN_FIRST_RLE_VL_INSERT_AFTER_CASE_ID: &str =
    "classic/sc/palette_color_u8_multiframe_rle_lossless";

const PLAN_FIRST_CLASSIC_MG_DX_NM_CASE_IDS: &[&str] = &[
    "classic/mg/for_presentation_mono1_u16_12bit_explicit_le",
    "classic/mg/for_presentation_mono1_u16_12bit_rle_lossless",
    "classic/mg/for_processing_mono2_u16_12bit_implicit_le",
    "classic/mg/for_processing_mono2_u16_12bit_rle_lossless",
    "classic/dx/display_shutter_mono2_u16_explicit_le",
    "classic/dx/display_shutter_mono2_u16_rle_lossless",
    "classic/nm/multiframe_explicit_le",
];

const PLAN_FIRST_CLASSIC_PET_CASE_IDS: &[&str] = &["classic/pet/rescaled_activity_explicit_le"];

// Temporary U9 dispatcher compatibility lists. The unified curated planner
// owns the enhanced recipes and their artifact construction; this legacy
// dispatcher retains only their historical insertion positions until U9.3
// removes the manual generation stages entirely.
const U9_PLAN_FIRST_ENHANCED_CT_CASE_IDS: &[&str] =
    &["enhanced/ct/multiframe_shared_perframe_explicit_le"];
const STRESS_ENHANCED_CT_CASE_ID: &str = "stress/enhanced-ct/many_frames";
const STRESS_HIGH_INSTANCE_CT_CASE_ID: &str = "stress/study/high_instance_count_ct";
const STRESS_LARGE_BULK_CASE_ID: &str = "stress/sc/large_bulk_data";
const STRESS_DEEP_NESTED_CASE_ID: &str = "stress/sc/deep_nested_sequences";
const STRESS_LONG_METADATA_CASE_ID: &str = "stress/sc/long_value_metadata";
const STRESS_ENCAPSULATED_CASE_ID: &str = "stress/sc/large_encapsulated_multifragment";
const STRESS_WSI_PYRAMID_CASE_ID: &str = "stress/wsi/large_pyramid";
const U9_PLAN_FIRST_ENHANCED_CT_CONCATENATION_CASE_IDS: &[&str] =
    &["enhanced/ct/concatenation_two_part_explicit_le"];
const U9_PLAN_FIRST_ENHANCED_MR_CASE_IDS: &[&str] = &[
    "enhanced/mr/multiframe_echo_perframe_explicit_le",
    "enhanced/mr/multiframe_temporal_position_explicit_le",
    "enhanced/mr/multiframe_phase_velocity_encoding_explicit_le",
];
const U9_PLAN_FIRST_ENHANCED_PET_CASE_IDS: &[&str] = &["enhanced/pet/multiframe_explicit_le"];
const U9_PLAN_FIRST_WSI_CASE_IDS: &[&str] = &[
    "vl/wsi/tiled_full_small",
    "vl/wsi/tiled_sparse_small",
    "vl/wsi/multiple_optical_paths",
    "vl/wsi/pyramid_multiresolution",
];

const PLAN_FIRST_CLASSIC_US_MULTIFRAME_XA_XRF_CASE_IDS: &[&str] = &[
    "classic/us/multiframe_explicit_le",
    "classic/xa/monoplane_explicit_le",
    "classic/xrf/monoplane_explicit_le",
];

const PLAN_FIRST_CLASSIC_US_CR_MR_CASE_IDS: &[&str] = &[
    "classic/us/mono2_u8_explicit_le",
    "classic/us/mono2_u8_rle_lossless",
    "classic/cr/overlay_modality_voi_explicit_le",
    "classic/cr/overlay_modality_voi_rle_lossless",
    "classic/mr/multislice_oblique_explicit_le",
    "classic/mr/mono2_u16_rle_lossless",
];

#[derive(Debug, Clone, Copy)]
enum CuratedRecipeImplementation {
    Pixel(PixelRecipe),
    MetadataSc(MetadataScRecipe),
    TimezoneSc(TimezoneScRecipe),
    EmptyType2Sc(EmptyType2ScRecipe),
    StringBoundarySc(StringBoundaryScRecipe),
    PrivateCreatorSc(PrivateCreatorScRecipe),
    SequenceLengthSc(SequenceLengthScRecipe),
    NonsquareSpacingSc(NonsquareSpacingScRecipe),
}

impl CuratedRecipeImplementation {
    fn case_id(self) -> &'static str {
        match self {
            Self::Pixel(recipe) => recipe.case_id,
            Self::MetadataSc(recipe) => recipe.pixel.case_id,
            Self::TimezoneSc(recipe) => recipe.pixel.case_id,
            Self::EmptyType2Sc(recipe) => recipe.pixel.case_id,
            Self::StringBoundarySc(recipe) => recipe.pixel.case_id,
            Self::PrivateCreatorSc(recipe) => recipe.pixel.case_id,
            Self::SequenceLengthSc(recipe) => recipe.pixel.case_id,
            Self::NonsquareSpacingSc(recipe) => recipe.pixel.case_id,
        }
    }

    fn generate(
        self,
        run: &PreparedGenerationRun,
        case: &Value,
        standards_lock_sha256: &str,
    ) -> Result<Vec<GeneratedFile>, GenerateError> {
        match self {
            Self::Pixel(recipe) => Ok(vec![write_pixel_case(
                run,
                case,
                recipe,
                standards_lock_sha256,
            )?]),
            Self::MetadataSc(recipe) => Ok(vec![write_metadata_sc_case(
                run,
                case,
                recipe,
                standards_lock_sha256,
            )?]),
            Self::TimezoneSc(recipe) => {
                write_timezone_sc_case(run, case, recipe, standards_lock_sha256)
            }
            Self::EmptyType2Sc(recipe) => Ok(vec![write_empty_type2_sc_case(
                run,
                case,
                recipe,
                standards_lock_sha256,
            )?]),
            Self::StringBoundarySc(recipe) => Ok(vec![write_string_boundary_sc_case(
                run,
                case,
                recipe,
                standards_lock_sha256,
            )?]),
            Self::PrivateCreatorSc(recipe) => Ok(vec![write_private_creator_sc_case(
                run,
                case,
                recipe,
                standards_lock_sha256,
            )?]),
            Self::SequenceLengthSc(recipe) => {
                write_sequence_length_sc_case(run, case, recipe, standards_lock_sha256)
            }
            Self::NonsquareSpacingSc(recipe) => {
                write_nonsquare_spacing_sc_case(run, case, recipe, standards_lock_sha256)
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct PlanFirstStageEntry {
    stage: CuratedRecipeStage,
    case_id: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PlanFirstStageError {
    MissingSelectedCase {
        stage: CuratedRecipeStage,
        case_id: &'static str,
    },
    UnmatchedCases {
        case_ids: Vec<String>,
    },
    MissingSelectedAdvancedCase {
        stage: &'static str,
        case_id: &'static str,
    },
}

impl std::fmt::Display for PlanFirstStageError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingSelectedCase { stage, case_id } => write!(
                formatter,
                "selected classic case {case_id} has no plan-first output in {}",
                stage.name()
            ),
            Self::UnmatchedCases { case_ids } => write!(
                formatter,
                "plan-first outputs did not match the curated stage dispatcher: {}",
                case_ids.join(", ")
            ),
            Self::MissingSelectedAdvancedCase { stage, case_id } => write!(
                formatter,
                "selected advanced case {case_id} has no plan-first output in {stage}"
            ),
        }
    }
}

fn take_plan_first_advanced_case(
    run: &PreparedGenerationRun,
    registry: &Value,
    plan_first_files: &mut BTreeMap<String, Vec<GeneratedFile>>,
    case_id: &'static str,
    stage: &'static str,
) -> Result<Option<Vec<GeneratedFile>>, GenerateError> {
    let Some(case) = registry_case(registry, case_id)? else {
        return Ok(None);
    };
    if !should_generate_case(case, run)? {
        return Ok(None);
    }
    plan_first_files
        .remove(case_id)
        .map(Some)
        .ok_or_else(|| GenerateError::PlanFirst {
            stage: "advanced stage dispatch",
            message: PlanFirstStageError::MissingSelectedAdvancedCase { stage, case_id }
                .to_string(),
        })
}

impl CuratedRecipeStage {
    fn name(self) -> &'static str {
        match self {
            Self::SecondaryCapture => "secondary-capture stage",
            Self::ClassicCt => "classic CT stage",
            Self::ClassicImagesBeforeEnhancedPet => "classic pre-enhanced-PET stage",
            Self::ClassicImagesAfterEnhancedPet => "classic post-enhanced-PET stage",
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum CuratedStageEntry {
    Legacy(CuratedRecipeImplementation),
    PlanFirst(PlanFirstStageEntry),
}

impl CuratedStageEntry {
    fn case_id(self) -> &'static str {
        match self {
            Self::Legacy(implementation) => implementation.case_id(),
            Self::PlanFirst(entry) => entry.case_id,
        }
    }
}

fn curated_recipe_registry(stage: CuratedRecipeStage) -> Vec<CuratedStageEntry> {
    let mut recipes = Vec::new();
    match stage {
        CuratedRecipeStage::SecondaryCapture => {
            for recipe in PIXEL_RECIPES.iter().copied() {
                recipes.push(CuratedStageEntry::Legacy(
                    CuratedRecipeImplementation::Pixel(recipe),
                ));
                if recipe.case_id == PLAN_FIRST_NATIVE_VL_INSERT_AFTER_CASE_ID {
                    recipes.push(CuratedStageEntry::PlanFirst(PlanFirstStageEntry {
                        stage,
                        case_id: COLOR_SOFTCOPY_PRESENTATION_STATE_SOURCE_CASE_ID,
                    }));
                    recipes.extend(
                        PLAN_FIRST_NATIVE_VL_CASE_IDS
                            .iter()
                            .copied()
                            .map(|case_id| PlanFirstStageEntry { stage, case_id })
                            .map(CuratedStageEntry::PlanFirst),
                    );
                }
                if recipe.case_id == PLAN_FIRST_RLE_VL_INSERT_AFTER_CASE_ID {
                    recipes.extend(
                        PLAN_FIRST_RLE_VL_CASE_IDS
                            .iter()
                            .copied()
                            .map(|case_id| PlanFirstStageEntry { stage, case_id })
                            .map(CuratedStageEntry::PlanFirst),
                    );
                }
            }
            recipes.extend(
                METADATA_SC_RECIPES
                    .iter()
                    .copied()
                    .map(CuratedRecipeImplementation::MetadataSc)
                    .map(CuratedStageEntry::Legacy),
            );
            recipes.extend([
                CuratedStageEntry::Legacy(CuratedRecipeImplementation::TimezoneSc(
                    TIMEZONE_SC_RECIPE,
                )),
                CuratedStageEntry::Legacy(CuratedRecipeImplementation::EmptyType2Sc(
                    EMPTY_TYPE2_SC_RECIPE,
                )),
                CuratedStageEntry::Legacy(CuratedRecipeImplementation::StringBoundarySc(
                    STRING_BOUNDARY_SC_RECIPE,
                )),
                CuratedStageEntry::Legacy(CuratedRecipeImplementation::PrivateCreatorSc(
                    PRIVATE_CREATOR_SC_RECIPE,
                )),
                CuratedStageEntry::Legacy(CuratedRecipeImplementation::SequenceLengthSc(
                    SEQUENCE_LENGTH_SC_RECIPE,
                )),
                CuratedStageEntry::Legacy(CuratedRecipeImplementation::NonsquareSpacingSc(
                    NONSQUARE_SPACING_SC_RECIPE,
                )),
            ]);
        }
        CuratedRecipeStage::ClassicCt => recipes.extend(
            CLASSIC_CT_RECIPES
                .iter()
                .map(|recipe| PlanFirstStageEntry {
                    stage,
                    case_id: recipe.case_id,
                })
                .map(CuratedStageEntry::PlanFirst),
        ),
        CuratedRecipeStage::ClassicImagesBeforeEnhancedPet => {
            recipes.extend(
                PLAN_FIRST_CLASSIC_MG_DX_NM_CASE_IDS
                    .iter()
                    .copied()
                    .map(|case_id| PlanFirstStageEntry { stage, case_id })
                    .map(CuratedStageEntry::PlanFirst),
            );
            recipes.extend(
                PLAN_FIRST_CLASSIC_PET_CASE_IDS
                    .iter()
                    .copied()
                    .map(|case_id| PlanFirstStageEntry { stage, case_id })
                    .map(CuratedStageEntry::PlanFirst),
            );
        }
        CuratedRecipeStage::ClassicImagesAfterEnhancedPet => {
            recipes.extend(
                PLAN_FIRST_CLASSIC_US_MULTIFRAME_XA_XRF_CASE_IDS
                    .iter()
                    .copied()
                    .map(|case_id| PlanFirstStageEntry { stage, case_id })
                    .map(CuratedStageEntry::PlanFirst),
            );
            recipes.extend(
                PLAN_FIRST_CLASSIC_US_CR_MR_CASE_IDS
                    .iter()
                    .copied()
                    .map(|case_id| PlanFirstStageEntry { stage, case_id })
                    .map(CuratedStageEntry::PlanFirst),
            );
        }
    }
    recipes
}

fn write_curated_recipe_stage(
    context: &mut GenerationContext,
    run: &PreparedGenerationRun,
    registry: &Value,
    standards_lock_sha256: &str,
    stage: CuratedRecipeStage,
    plan_first_files: &mut BTreeMap<String, Vec<GeneratedFile>>,
) -> Result<(), GenerateError> {
    for entry in curated_recipe_registry(stage) {
        let case_id = entry.case_id();
        let Some(case) = registry_case(registry, case_id)? else {
            continue;
        };
        if should_generate_case(case, run)? {
            if let Some(files) = plan_first_files.remove(case_id) {
                context.record_many(files)?;
            } else {
                match entry {
                    CuratedStageEntry::Legacy(implementation) => context.record_many(
                        implementation.generate(run, case, standards_lock_sha256)?,
                    )?,
                    CuratedStageEntry::PlanFirst(entry) => {
                        return Err(GenerateError::PlanFirst {
                            stage: "classic stage dispatch",
                            message: PlanFirstStageError::MissingSelectedCase {
                                stage: entry.stage,
                                case_id: entry.case_id,
                            }
                            .to_string(),
                        });
                    }
                }
            }
        }
    }
    Ok(())
}

fn resolve_and_write_u5_color_softcopy_private_source(
    run: &PreparedGenerationRun,
    registry: &Value,
    standards_lock_sha256: &str,
) -> Result<GeneratedFile, GenerateError> {
    let source_case = registry_case(registry, COLOR_SOFTCOPY_PRESENTATION_STATE_SOURCE_CASE_ID)?
        .ok_or_else(|| GenerateError::MetadataShape {
            path: PathBuf::from(COLOR_SOFTCOPY_PRESENTATION_STATE_CASE_ID),
            message: "Color Softcopy Presentation State RGB source registry row is missing",
        })?;
    write_pixel_case(
        run,
        source_case,
        COLOR_SOFTCOPY_PRIVATE_SOURCE_PIXEL_RECIPE,
        standards_lock_sha256,
    )
}

pub(crate) fn write_supported_cases_with_plan_first_sc(
    run: &PreparedGenerationRun,
    registry: &Value,
    standards_lock_sha256: &str,
    plan_first_files: Vec<GeneratedFile>,
) -> Result<GenerationOutput, GenerateError> {
    if run.profile == "negative" {
        return write_negative_cases(run, registry, standards_lock_sha256);
    }
    if run.profile == "fuzz" {
        return write_fuzz_cases(run, registry, standards_lock_sha256);
    }
    let mut plan_first_files_by_case = BTreeMap::<String, Vec<GeneratedFile>>::new();
    let mut plan_first_case_ids = BTreeSet::new();
    for file in plan_first_files {
        plan_first_case_ids.insert(file.case_id.clone());
        plan_first_files_by_case
            .entry(file.case_id.clone())
            .or_default()
            .push(file);
    }
    let mut context = GenerationContext::default();
    for case_id in U9_PLAN_FIRST_WSI_CASE_IDS.iter().copied() {
        if let Some(files) = take_plan_first_advanced_case(
            run,
            registry,
            &mut plan_first_files_by_case,
            case_id,
            "WSI stage",
        )? {
            context.record_many(files)?;
        }
    }
    if let Some(files) = take_plan_first_advanced_case(
        run,
        registry,
        &mut plan_first_files_by_case,
        STRESS_WSI_PYRAMID_CASE_ID,
        "reduced-stress WSI stage",
    )? {
        let (request, started) = context.preflight_stress(
            StressRecipeKind::WsiPyramid,
            16 * 1024 * 1024,
            64 * 1024 * 1024,
        )?;
        context.record_stress_files(STRESS_WSI_PYRAMID_CASE_ID, request, started, files)?;
    }
    if let Some(case) = registry_case(registry, WSI_TILE_SEGMENTATION_CASE_ID)? {
        if should_generate_case(case, run)? {
            let source = context
                .source_registry()
                .first_for_case(WSI_TILE_SEGMENTATION_SOURCE_CASE_ID)
                .cloned()
                .ok_or_else(|| GenerateError::MetadataShape {
                    path: PathBuf::from(WSI_TILE_SEGMENTATION_CASE_ID),
                    message: "WSI tile segmentation source must be generated before the derived object",
                })?;
            match write_wsi_tile_segmentation_case(run, case, &source, standards_lock_sha256)? {
                WsiTileSegmentationCaseOutcome::Generated(file) => context.record_one(file)?,
                WsiTileSegmentationCaseOutcome::Unavailable(row) => {
                    context.unavailable_cases.push(row)
                }
            }
        }
    }
    write_curated_recipe_stage(
        &mut context,
        run,
        registry,
        standards_lock_sha256,
        CuratedRecipeStage::SecondaryCapture,
        &mut plan_first_files_by_case,
    )?;
    if let Some(case) = registry_case(registry, STRESS_HIGH_INSTANCE_CT_CASE_ID)? {
        if should_generate_case(case, run)? {
            let (request, started) = context.preflight_stress(
                StressRecipeKind::CtStudy,
                8 * 1024 * 1024,
                128 * 1024 * 1024,
            )?;
            let files = write_stress_high_instance_ct_case(run, case, standards_lock_sha256)?;
            context.record_stress_files(
                STRESS_HIGH_INSTANCE_CT_CASE_ID,
                request,
                started,
                files,
            )?;
        }
    }
    if let Some(case) = registry_case(registry, STRESS_LARGE_BULK_CASE_ID)? {
        if should_generate_case(case, run)? {
            let (request, started) = context.preflight_stress(
                StressRecipeKind::NativeBulkData,
                72 * 1024 * 1024,
                384 * 1024 * 1024,
            )?;
            let file = write_stress_large_bulk_case(run, case, standards_lock_sha256)?;
            context.record_stress_files(STRESS_LARGE_BULK_CASE_ID, request, started, vec![file])?;
        }
    }
    if let Some(case) = registry_case(registry, STRESS_DEEP_NESTED_CASE_ID)? {
        if should_generate_case(case, run)? {
            let (request, started) = context.preflight_stress(
                StressRecipeKind::NestedSequences,
                20 * 1024 * 1024,
                128 * 1024 * 1024,
            )?;
            let file = write_stress_deep_nested_case(run, case, standards_lock_sha256)?;
            context.record_stress_files(
                STRESS_DEEP_NESTED_CASE_ID,
                request,
                started,
                vec![file],
            )?;
        }
    }
    if let Some(case) = registry_case(registry, STRESS_LONG_METADATA_CASE_ID)? {
        if should_generate_case(case, run)? {
            let (request, started) = context.preflight_stress(
                StressRecipeKind::LongMetadata,
                4 * 1024 * 1024,
                64 * 1024 * 1024,
            )?;
            let file = write_stress_long_metadata_case(run, case, standards_lock_sha256)?;
            context.record_stress_files(
                STRESS_LONG_METADATA_CASE_ID,
                request,
                started,
                vec![file],
            )?;
        }
    }
    if let Some(case) = registry_case(registry, STRESS_ENCAPSULATED_CASE_ID)? {
        if should_generate_case(case, run)? {
            let (request, started) = context.preflight_stress(
                StressRecipeKind::EncapsulatedEot,
                80 * 1024 * 1024,
                384 * 1024 * 1024,
            )?;
            let file = write_stress_encapsulated_case(run, case, standards_lock_sha256)?;
            context.record_stress_files(
                STRESS_ENCAPSULATED_CASE_ID,
                request,
                started,
                vec![file],
            )?;
        }
    }
    write_curated_recipe_stage(
        &mut context,
        run,
        registry,
        standards_lock_sha256,
        CuratedRecipeStage::ClassicCt,
        &mut plan_first_files_by_case,
    )?;
    for spec in [FLOAT32_SPEC, FLOAT64_SPEC] {
        if let Some(case) = registry_case(registry, spec.case_id)? {
            if !should_generate_case(case, run)? {
                continue;
            }
            let mut sources = context
                .source_registry()
                .sources_for_case(PARAMETRIC_MAP_SOURCE_CASE_ID)
                .cloned()
                .collect::<Vec<_>>();
            sources.sort_by(|left, right| left.source_path.cmp(&right.source_path));
            match write_parametric_map_case(run, case, spec, &sources, standards_lock_sha256)? {
                ParametricMapCaseOutcome::Generated(file) => context.record_one(file)?,
                ParametricMapCaseOutcome::Unavailable(row) => context.unavailable_cases.push(row),
            }
        }
    }
    if let Some(files) = take_plan_first_advanced_case(
        run,
        registry,
        &mut plan_first_files_by_case,
        STRESS_ENHANCED_CT_CASE_ID,
        "reduced-stress enhanced CT stage",
    )? {
        let (request, started) = context.preflight_stress(
            StressRecipeKind::EnhancedCt,
            8 * 1024 * 1024,
            128 * 1024 * 1024,
        )?;
        context.record_stress_files(STRESS_ENHANCED_CT_CASE_ID, request, started, files)?;
    }
    for case_id in U9_PLAN_FIRST_ENHANCED_CT_CASE_IDS {
        if let Some(files) = take_plan_first_advanced_case(
            run,
            registry,
            &mut plan_first_files_by_case,
            case_id,
            "enhanced CT stage",
        )? {
            context.record_many(files)?;
        }
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
    if let Some(case) = registry_case(registry, TID1500_CASE_ID)? {
        if should_generate_case(case, run)? {
            let ct_source = context
                .source_registry()
                .first_for_case(TID1500_CT_SOURCE_CASE_ID)
                .cloned()
                .ok_or_else(|| GenerateError::MetadataShape {
                    path: PathBuf::from(TID1500_CASE_ID),
                    message: "TID 1500 Enhanced CT source must be generated before the report",
                })?;
            let seg_source = context
                .source_registry()
                .first_for_case(TID1500_SEG_SOURCE_CASE_ID)
                .cloned()
                .ok_or_else(|| GenerateError::MetadataShape {
                    path: PathBuf::from(TID1500_CASE_ID),
                    message: "TID 1500 SEG source must be generated before the report",
                })?;
            match write_tid1500_case(run, case, &ct_source, &seg_source, standards_lock_sha256)? {
                Tid1500CaseOutcome::Generated(file) => context.record_one(file)?,
                Tid1500CaseOutcome::Unavailable(row) => context.unavailable_cases.push(row),
            }
        }
    }
    if let Some(case) = registry_case(registry, SCOORD3D_CASE_ID)? {
        if should_generate_case(case, run)? {
            let ct_source = context
                .source_registry()
                .first_for_case(SCOORD3D_CT_SOURCE_CASE_ID)
                .cloned()
                .ok_or_else(|| GenerateError::MetadataShape {
                    path: PathBuf::from(SCOORD3D_CASE_ID),
                    message: "SCOORD3D Enhanced CT source must be generated before the report",
                })?;
            match write_scoord3d_case(run, case, &ct_source, standards_lock_sha256)? {
                Scoord3dCaseOutcome::Generated(file) => context.record_one(file)?,
                Scoord3dCaseOutcome::Unavailable(row) => context.unavailable_cases.push(row),
            }
        }
    }
    if let Some(case) = registry_case(registry, SPATIAL_REGISTRATION_CASE_ID)? {
        if should_generate_case(case, run)? {
            if context
                .source_registry()
                .first_for_case(SPATIAL_REGISTRATION_SOURCE_CASE_ID)
                .is_none()
            {
                let source_case = registry_case(registry, SPATIAL_REGISTRATION_SOURCE_CASE_ID)?
                    .ok_or_else(|| GenerateError::MetadataShape {
                        path: PathBuf::from(SPATIAL_REGISTRATION_CASE_ID),
                        message: "Spatial Registration moving CT registry row is missing",
                    })?;
                let source_recipe = CLASSIC_CT_RECIPES
                    .iter()
                    .find(|recipe| recipe.case_id == SPATIAL_REGISTRATION_SOURCE_CASE_ID)
                    .copied()
                    .ok_or_else(|| GenerateError::MetadataShape {
                        path: PathBuf::from(SPATIAL_REGISTRATION_CASE_ID),
                        message: "Spatial Registration moving CT native recipe is missing",
                    })?;
                context.record_many(write_classic_ct_case(
                    run,
                    source_case,
                    source_recipe,
                    standards_lock_sha256,
                )?)?;
            }
            let target = context
                .source_registry()
                .first_for_case(SPATIAL_REGISTRATION_TARGET_CASE_ID)
                .cloned()
                .ok_or_else(|| GenerateError::MetadataShape {
                    path: PathBuf::from(SPATIAL_REGISTRATION_CASE_ID),
                    message: "Spatial Registration target Enhanced CT must be generated first",
                })?;
            let source = context
                .source_registry()
                .first_for_case(SPATIAL_REGISTRATION_SOURCE_CASE_ID)
                .cloned()
                .ok_or_else(|| GenerateError::MetadataShape {
                    path: PathBuf::from(SPATIAL_REGISTRATION_CASE_ID),
                    message: "Spatial Registration moving classic CT must be generated first",
                })?;
            context.record_one(write_spatial_registration_case(
                run,
                case,
                &target,
                &source,
                standards_lock_sha256,
            )?)?;
        }
    }
    if let Some(case) = registry_case(registry, DEFORMABLE_SPATIAL_REGISTRATION_CASE_ID)? {
        if should_generate_case(case, run)? {
            if context
                .source_registry()
                .first_for_case(SPATIAL_REGISTRATION_SOURCE_CASE_ID)
                .is_none()
            {
                let source_case = registry_case(registry, SPATIAL_REGISTRATION_SOURCE_CASE_ID)?
                    .ok_or_else(|| GenerateError::MetadataShape {
                        path: PathBuf::from(DEFORMABLE_SPATIAL_REGISTRATION_CASE_ID),
                        message: "Deformable Registration source CT registry row is missing",
                    })?;
                let source_recipe = CLASSIC_CT_RECIPES
                    .iter()
                    .find(|recipe| recipe.case_id == SPATIAL_REGISTRATION_SOURCE_CASE_ID)
                    .copied()
                    .ok_or_else(|| GenerateError::MetadataShape {
                        path: PathBuf::from(DEFORMABLE_SPATIAL_REGISTRATION_CASE_ID),
                        message: "Deformable Registration source CT native recipe is missing",
                    })?;
                context.record_many(write_classic_ct_case(
                    run,
                    source_case,
                    source_recipe,
                    standards_lock_sha256,
                )?)?;
            }
            let target = context
                .source_registry()
                .first_for_case(SPATIAL_REGISTRATION_TARGET_CASE_ID)
                .cloned()
                .ok_or_else(|| GenerateError::MetadataShape {
                    path: PathBuf::from(DEFORMABLE_SPATIAL_REGISTRATION_CASE_ID),
                    message: "Deformable Registration target Enhanced CT must be generated first",
                })?;
            let source = context
                .source_registry()
                .first_for_case(SPATIAL_REGISTRATION_SOURCE_CASE_ID)
                .cloned()
                .ok_or_else(|| GenerateError::MetadataShape {
                    path: PathBuf::from(DEFORMABLE_SPATIAL_REGISTRATION_CASE_ID),
                    message: "Deformable Registration source classic CT must be generated first",
                })?;
            context.record_one(write_deformable_spatial_registration_case(
                run,
                case,
                &target,
                &source,
                standards_lock_sha256,
            )?)?;
        }
    }
    if let Some(case) = registry_case(registry, COLOR_SOFTCOPY_PRESENTATION_STATE_CASE_ID)? {
        if should_generate_case(case, run)? {
            if context
                .source_registry()
                .first_for_case(COLOR_SOFTCOPY_PRESENTATION_STATE_SOURCE_CASE_ID)
                .is_none()
            {
                context.record_one(resolve_and_write_u5_color_softcopy_private_source(
                    run,
                    registry,
                    standards_lock_sha256,
                )?)?;
            }
            let source = context
                .source_registry()
                .first_for_case(COLOR_SOFTCOPY_PRESENTATION_STATE_SOURCE_CASE_ID)
                .cloned()
                .ok_or_else(|| GenerateError::MetadataShape {
                    path: PathBuf::from(COLOR_SOFTCOPY_PRESENTATION_STATE_CASE_ID),
                    message: "Color Softcopy Presentation State RGB source must be generated first",
                })?;
            context.record_one(write_color_softcopy_presentation_state_case(
                run,
                case,
                &source,
                standards_lock_sha256,
            )?)?;
        }
    }
    if let Some(case) = registry_case(registry, ADVANCED_BLENDING_PRESENTATION_STATE_CASE_ID)? {
        if should_generate_case(case, run)? {
            if context
                .source_registry()
                .first_for_case(ADVANCED_BLENDING_PRESENTATION_STATE_SOURCE_CASE_ID)
                .is_none()
            {
                let source_case = registry_case(
                    registry,
                    ADVANCED_BLENDING_PRESENTATION_STATE_SOURCE_CASE_ID,
                )?
                .ok_or_else(|| GenerateError::MetadataShape {
                    path: PathBuf::from(ADVANCED_BLENDING_PRESENTATION_STATE_CASE_ID),
                    message: "Advanced Blending source CT registry row is missing",
                })?;
                let source_recipe = CLASSIC_CT_RECIPES
                    .iter()
                    .find(|recipe| {
                        recipe.case_id == ADVANCED_BLENDING_PRESENTATION_STATE_SOURCE_CASE_ID
                    })
                    .copied()
                    .ok_or_else(|| GenerateError::MetadataShape {
                        path: PathBuf::from(ADVANCED_BLENDING_PRESENTATION_STATE_CASE_ID),
                        message: "Advanced Blending source CT native recipe is missing",
                    })?;
                context.record_many(write_classic_ct_case(
                    run,
                    source_case,
                    source_recipe,
                    standards_lock_sha256,
                )?)?;
            }
            let sources = context
                .source_registry()
                .sources_for_case(ADVANCED_BLENDING_PRESENTATION_STATE_SOURCE_CASE_ID)
                .cloned()
                .collect::<Vec<_>>();
            let sources: [GeneratedSourceObject; 4] = sources.try_into().map_err(|sources: Vec<_>| {
                GenerateError::MetadataShape {
                    path: PathBuf::from(ADVANCED_BLENDING_PRESENTATION_STATE_CASE_ID),
                    message: if sources.len() < 4 {
                        "Advanced Blending requires all four source CT instances to be generated first"
                    } else {
                        "Advanced Blending source CT case must contain exactly four instances"
                    },
                }
            })?;
            context.record_one(write_advanced_blending_presentation_state_case(
                run,
                case,
                &sources,
                standards_lock_sha256,
            )?)?;
        }
    }
    if let Some(case) = registry_case(registry, BLENDING_PRESENTATION_STATE_CASE_ID)? {
        if should_generate_case(case, run)? {
            if context
                .source_registry()
                .first_for_case(BLENDING_PRESENTATION_STATE_SOURCE_CASE_ID)
                .is_none()
            {
                let source_case = registry_case(
                    registry,
                    BLENDING_PRESENTATION_STATE_SOURCE_CASE_ID,
                )?
                .ok_or_else(|| GenerateError::MetadataShape {
                    path: PathBuf::from(BLENDING_PRESENTATION_STATE_CASE_ID),
                    message: "Blending Presentation State source CT registry row is missing",
                })?;
                let source_recipe = CLASSIC_CT_RECIPES
                    .iter()
                    .find(|recipe| recipe.case_id == BLENDING_PRESENTATION_STATE_SOURCE_CASE_ID)
                    .copied()
                    .ok_or_else(|| GenerateError::MetadataShape {
                        path: PathBuf::from(BLENDING_PRESENTATION_STATE_CASE_ID),
                        message: "Blending Presentation State source CT native recipe is missing",
                    })?;
                context.record_many(write_classic_ct_case(
                    run,
                    source_case,
                    source_recipe,
                    standards_lock_sha256,
                )?)?;
            }
            let sources = context
                .source_registry()
                .sources_for_case(BLENDING_PRESENTATION_STATE_SOURCE_CASE_ID)
                .cloned()
                .collect::<Vec<_>>();
            let sources: [GeneratedSourceObject; 4] = sources.try_into().map_err(|sources: Vec<_>| {
                GenerateError::MetadataShape {
                    path: PathBuf::from(BLENDING_PRESENTATION_STATE_CASE_ID),
                    message: if sources.len() < 4 {
                        "Blending Presentation State requires all four source CT instances first"
                    } else {
                        "Blending Presentation State source CT case must contain exactly four instances"
                    },
                }
            })?;
            context.record_one(write_blending_presentation_state_case(
                run,
                case,
                &sources,
                standards_lock_sha256,
            )?)?;
        }
    }
    if let Some(case) = registry_case(registry, TWELVE_LEAD_ECG_CASE_ID)? {
        if should_generate_case(case, run)? {
            context.record_one(write_twelve_lead_ecg_case(
                run,
                case,
                standards_lock_sha256,
            )?)?;
        }
    }
    if let Some(case) = registry_case(registry, GENERAL_ECG_CASE_ID)? {
        if should_generate_case(case, run)? {
            context.record_one(write_general_ecg_case(run, case, standards_lock_sha256)?)?;
        }
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
                message:
                    "presentation state source object must be generated before the derived recipe",
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
                message:
                    "Comprehensive SR source object must be generated before the derived recipe",
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
                message:
                    "RT Structure Set source object must be generated before the derived recipe",
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
    for recipe in RT_PLAN_RECIPES {
        let Some(case) = registry_case(registry, recipe.case_id)? else {
            continue;
        };
        if !should_generate_case(case, run)? {
            continue;
        }
        let structure_set_source = context
            .source_registry()
            .first_for_case(recipe.structure_set_source_case_id)
            .cloned()
            .ok_or_else(|| GenerateError::MetadataShape {
                path: PathBuf::from(recipe.case_id),
                message: "RT Plan Structure Set source object must be generated before the derived recipe",
            })?;
        let dose_source = context
            .source_registry()
            .first_for_case(recipe.dose_source_case_id)
            .cloned()
            .ok_or_else(|| GenerateError::MetadataShape {
                path: PathBuf::from(recipe.case_id),
                message: "RT Plan Dose source object must be generated before the derived recipe",
            })?;
        context.record_one(write_rt_plan_case(
            run,
            case,
            *recipe,
            &structure_set_source,
            &dose_source,
            standards_lock_sha256,
        )?)?;
    }
    for recipe in RT_RADIATION_RECIPES {
        let Some(case) = registry_case(registry, recipe.case_id)? else {
            continue;
        };
        if !should_generate_case(case, run)? {
            continue;
        }
        let plan_source = context
            .source_registry()
            .first_for_case(recipe.plan_source_case_id)
            .cloned()
            .ok_or_else(|| GenerateError::MetadataShape {
                path: PathBuf::from(recipe.case_id),
                message: "C-Arm RT Radiation Plan source must be generated before the derived recipe",
            })?;
        context.record_one(write_rt_radiation_case(
            run,
            case,
            *recipe,
            &plan_source,
            standards_lock_sha256,
        )?)?;
    }
    for recipe in RT_RADIATION_SET_RECIPES {
        let Some(case) = registry_case(registry, recipe.case_id)? else {
            continue;
        };
        if !should_generate_case(case, run)? {
            continue;
        }
        let plan_source = context
            .source_registry()
            .first_for_case(recipe.plan_source_case_id)
            .cloned()
            .ok_or_else(|| GenerateError::MetadataShape {
                path: PathBuf::from(recipe.case_id),
                message: "RT Radiation Set Plan source must be generated before the derived recipe",
            })?;
        let radiation_source = context
            .source_registry()
            .first_for_case(recipe.radiation_source_case_id)
            .cloned()
            .ok_or_else(|| GenerateError::MetadataShape {
                path: PathBuf::from(recipe.case_id),
                message: "RT Radiation Set Radiation source must be generated before the derived recipe",
            })?;
        context.record_one(write_rt_radiation_set_case(
            run,
            case,
            *recipe,
            &plan_source,
            &radiation_source,
            standards_lock_sha256,
        )?)?;
    }
    for recipe in RT_IMAGE_RECIPES {
        let Some(case) = registry_case(registry, recipe.case_id)? else {
            continue;
        };
        if !should_generate_case(case, run)? {
            continue;
        }
        let plan_source = context
            .source_registry()
            .first_for_case(recipe.plan_source_case_id)
            .cloned()
            .ok_or_else(|| GenerateError::MetadataShape {
                path: PathBuf::from(recipe.case_id),
                message: "RT Image Plan source object must be generated before the derived recipe",
            })?;
        context.record_one(write_rt_image_case(
            run,
            case,
            *recipe,
            &plan_source,
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
    for recipe in ENCAPSULATED_STL_RECIPES {
        let Some(case) = registry_case(registry, recipe.case_id)? else {
            continue;
        };
        if !should_generate_case(case, run)? {
            continue;
        }
        context.record_one(write_encapsulated_stl_case(
            run,
            case,
            *recipe,
            standards_lock_sha256,
        )?)?;
    }
    for case_id in U9_PLAN_FIRST_ENHANCED_CT_CONCATENATION_CASE_IDS {
        if let Some(files) = take_plan_first_advanced_case(
            run,
            registry,
            &mut plan_first_files_by_case,
            case_id,
            "enhanced CT concatenation stage",
        )? {
            context.record_many(files)?;
        }
    }
    for case_id in U9_PLAN_FIRST_ENHANCED_MR_CASE_IDS {
        if let Some(files) = take_plan_first_advanced_case(
            run,
            registry,
            &mut plan_first_files_by_case,
            case_id,
            "enhanced MR stage",
        )? {
            context.record_many(files)?;
        }
    }
    write_curated_recipe_stage(
        &mut context,
        run,
        registry,
        standards_lock_sha256,
        CuratedRecipeStage::ClassicImagesBeforeEnhancedPet,
        &mut plan_first_files_by_case,
    )?;
    for case_id in U9_PLAN_FIRST_ENHANCED_PET_CASE_IDS {
        if let Some(files) = take_plan_first_advanced_case(
            run,
            registry,
            &mut plan_first_files_by_case,
            case_id,
            "enhanced PET stage",
        )? {
            context.record_many(files)?;
        }
    }
    write_curated_recipe_stage(
        &mut context,
        run,
        registry,
        standards_lock_sha256,
        CuratedRecipeStage::ClassicImagesAfterEnhancedPet,
        &mut plan_first_files_by_case,
    )?;
    if !plan_first_files_by_case.is_empty() {
        return Err(GenerateError::PlanFirst {
            stage: "curated stage dispatch",
            message: PlanFirstStageError::UnmatchedCases {
                case_ids: plan_first_files_by_case.into_keys().collect(),
            }
            .to_string(),
        });
    }
    migrate_shared_plan_curated_files(run, &mut context.generated_files, &plan_first_case_ids)?;
    Ok(context.into_output())
}

fn migrate_shared_plan_curated_files(
    run: &PreparedGenerationRun,
    files: &mut [GeneratedFile],
    plan_first_case_ids: &BTreeSet<String>,
) -> Result<(), GenerateError> {
    for (index, file) in files.iter_mut().enumerate() {
        if plan_first_case_ids.contains(&file.case_id) {
            continue;
        }
        let Some(template_family) = shared_plan_curated_template_family(&file.case_id) else {
            continue;
        };
        let relative_path = generated_manifest_str(
            &file.manifest_entry,
            "/path",
            "shared-plan curated path must be a string",
        )?;
        let path = run.out_dir.join(relative_path);
        let before = fs::read(&path).map_err(|source| GenerateError::ReadGeneratedFile {
            path: path.clone(),
            source,
        })?;
        let file_object = open_file(&path).map_err(|source| GenerateError::ValidateDicomFile {
            path: path.clone(),
            message: source.to_string(),
        })?;
        let transfer_syntax_uid = file_object.meta().transfer_syntax().to_string();
        let implementation_class_uid = file_object.meta().implementation_class_uid.clone();
        let object = file_object.into_inner();
        let string = |name: &str| {
            object
                .element_by_name(name)
                .ok()
                .and_then(|element| element.to_str().ok())
                .map(|value| value.trim().to_string())
        };
        let sop_class_uid = string("SOPClassUID").ok_or_else(|| GenerateError::MetadataShape {
            path: path.clone(),
            message: "shared-plan curated object is missing SOP Class UID",
        })?;
        let sop_instance_uid =
            string("SOPInstanceUID").ok_or_else(|| GenerateError::MetadataShape {
                path: path.clone(),
                message: "shared-plan curated object is missing SOP Instance UID",
            })?;
        let study_instance_uid = string("StudyInstanceUID");
        let series_instance_uid = string("SeriesInstanceUID");
        let plan = crate::composition::resolved_plan_from_curated_dataset(
            &object,
            crate::composition::CuratedPlanInput {
                instance_id: &format!("curated_shared_{index}"),
                template_id: crate::composition::TemplateId(template_family.into()),
                template_version: "1.0.0".parse().expect("static template version"),
                sop_class_uid: &sop_class_uid,
                transfer_syntax_uid: &transfer_syntax_uid,
                study_instance_uid: study_instance_uid.as_deref(),
                series_instance_uid: series_instance_uid.as_deref(),
                sop_instance_uid: &sop_instance_uid,
                implementation_class_uid: &implementation_class_uid,
            },
        )
        .map_err(|error| GenerateError::WriteDicomFile {
            path: path.clone(),
            message: format!("resolve shared curated composition plan: {error}"),
        })?;
        fs::remove_file(&path).map_err(|source| GenerateError::WriteDicomFile {
            path: path.clone(),
            message: format!("replace reserved shared curated instance: {source}"),
        })?;
        crate::composition::Part10Materializer
            .materialize(&plan, &path)
            .map_err(|error| GenerateError::WriteDicomFile {
                path: path.clone(),
                message: error.to_string(),
            })?;
        let after = fs::read(&path).map_err(|source| GenerateError::ReadGeneratedFile {
            path: path.clone(),
            source,
        })?;
        let semantic_stable = file.manifest_entry["determinism"] == "semantic_stable";
        if after != before && !semantic_stable {
            return Err(GenerateError::MetadataShape {
                path,
                message: "shared curated composition migration changed byte-stable output",
            });
        }
        if semantic_stable {
            file.manifest_entry["sha256"] = Value::String(sha256_hex(&after));
            file.manifest_entry["size_bytes"] = serde_json::json!(after.len());
        }
        file.manifest_entry["uids"]["implementation_version_name"] =
            Value::String(crate::IMPLEMENTATION_VERSION_NAME.into());
        append_curated_plan_validation(&mut file.manifest_entry["validation"]);
    }
    Ok(())
}

fn shared_plan_curated_template_family(case_id: &str) -> Option<&'static str> {
    if case_id.starts_with("enhanced/ct/") {
        Some("enhanced/ct")
    } else if case_id.starts_with("enhanced/mr/") {
        Some("enhanced/mr")
    } else if case_id.starts_with("enhanced/pet/") {
        Some("enhanced/pet")
    } else if case_id.starts_with("vl/wsi/") {
        Some("vl/wsi")
    } else {
        match case_id {
            "derived/registration/spatial_ct_pair" => Some("derived/registration/spatial"),
            "derived/registration/deformable_ct_pair" => Some("derived/registration/deformable"),
            "derived/presentation-state/grayscale_softcopy_ct_window_explicit_le" => {
                Some("derived/presentation-state/grayscale")
            }
            "derived/presentation-state/color_softcopy" => Some("derived/presentation-state/color"),
            "derived/presentation-state/blending" => Some("derived/presentation-state/blending"),
            "derived/presentation-state/advanced_blending" => {
                Some("derived/presentation-state/advanced-blending")
            }
            "derived/seg/binary_multiframe_explicit_le"
            | "derived/seg/binary_multiframe_deflated_image_frame" => {
                Some("derived/segmentation/binary")
            }
            "derived/seg/fractional_probability_multiframe_explicit_le" => {
                Some("derived/segmentation/fractional-probability")
            }
            "derived/seg/labelmap_multiframe_explicit_le" => Some("derived/segmentation/labelmap"),
            "derived/seg/wsi_tile_reference" => Some("derived/segmentation/wsi-tile"),
            "derived/parametric-map/float32_ct_derived_explicit_le" => {
                Some("derived/parametric-map/float32")
            }
            "derived/parametric-map/float64_ct_derived_explicit_le" => {
                Some("derived/parametric-map/float64")
            }
            "derived/rwvm/linear_ct_mapping_explicit_le" => {
                Some("derived/real-world-value-mapping/linear")
            }
            "derived/sr/basic_text_observation_explicit_le" => {
                Some("derived/structured-report/basic-text")
            }
            "derived/sr/comprehensive_measurement_explicit_le" => {
                Some("derived/structured-report/comprehensive")
            }
            "derived/sr/comprehensive3d_scoord3d" => {
                Some("derived/structured-report/comprehensive-3d")
            }
            "derived/sr/tid1500_ct_measurement_report" => Some("derived/structured-report/tid1500"),
            "derived/sr/key_object_selection_explicit_le" => {
                Some("derived/structured-report/key-object")
            }
            "non-image/rt/structure_set_single_roi_explicit_le" => {
                Some("non-image/rt/structure-set")
            }
            "non-image/rt/dose_grid_u16_explicit_le" => Some("non-image/rt/dose"),
            "non-image/rt/plan_linked" => Some("non-image/rt/plan"),
            "non-image/rt/image_linked" => Some("non-image/rt/image"),
            "non-image/rt/carm_photon_electron_radiation_minimal" => {
                Some("non-image/rt/c-arm-photon-electron-radiation")
            }
            "non-image/rt/radiation_set_minimal" => Some("non-image/rt/radiation-set"),
            "non-image/waveform/twelve_lead_ecg" => Some("non-image/waveform/twelve-lead-ecg"),
            "non-image/waveform/general_ecg" => Some("non-image/waveform/general-ecg"),
            "non-image/encapsulated-document/pdf_minimal_explicit_le" => {
                Some("non-image/encapsulated-document/pdf")
            }
            "derived/mesh/encapsulated_stl" => Some("non-image/mesh/stl"),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CompositionDefaultArtifact {
    pub case_id: String,
    pub path: PathBuf,
}

pub(crate) fn write_composition_default_artifacts(
    staging_root: &Path,
    seed: u64,
    template_id: &str,
    variant: Option<&str>,
) -> Result<Vec<CompositionDefaultArtifact>, GenerateError> {
    let (case_ids, selected_case, selected_file): (&[&str], Option<&str>, Option<&str>) =
        match (template_id, variant) {
            ("enhanced/ct", _) => (
                &["enhanced/ct/multiframe_shared_perframe_explicit_le"],
                None,
                None,
            ),
            ("enhanced/mr", _) => (
                &["enhanced/mr/multiframe_echo_perframe_explicit_le"],
                None,
                None,
            ),
            ("enhanced/pet", _) => (&["enhanced/pet/multiframe_explicit_le"], None, None),
            ("enhanced/ct/concatenation-part-1", _) => (
                &["enhanced/ct/concatenation_two_part_explicit_le"],
                None,
                Some("part-001.dcm"),
            ),
            ("enhanced/ct/concatenation-part-2", _) => (
                &["enhanced/ct/concatenation_two_part_explicit_le"],
                None,
                Some("part-002.dcm"),
            ),
            ("vl/wsi/tiled-full", _) => (&["vl/wsi/tiled_full_small"], None, None),
            ("vl/wsi/tiled-sparse", _) => (&["vl/wsi/tiled_sparse_small"], None, None),
            ("vl/wsi/multiple-optical-paths", _) => {
                (&["vl/wsi/multiple_optical_paths"], None, None)
            }
            ("vl/wsi/pyramid-volume", _) => (
                &["vl/wsi/pyramid_multiresolution"],
                None,
                Some("volume.dcm"),
            ),
            ("vl/wsi/pyramid-thumbnail", _) => (
                &["vl/wsi/pyramid_multiresolution"],
                None,
                Some("thumbnail.dcm"),
            ),
            ("vl/wsi/pyramid-label", _) => {
                (&["vl/wsi/pyramid_multiresolution"], None, Some("label.dcm"))
            }
            ("derived/registration/spatial", _) => (
                &[
                    "enhanced/ct/multiframe_shared_perframe_explicit_le",
                    "classic/ct/mono2_i16_rescale_12bit_explicit_le",
                    "derived/registration/spatial_ct_pair",
                ],
                Some("derived/registration/spatial_ct_pair"),
                None,
            ),
            ("derived/registration/deformable", _) => (
                &[
                    "enhanced/ct/multiframe_shared_perframe_explicit_le",
                    "classic/ct/mono2_i16_rescale_12bit_explicit_le",
                    "derived/registration/deformable_ct_pair",
                ],
                Some("derived/registration/deformable_ct_pair"),
                None,
            ),
            ("derived/presentation-state/grayscale", _) => (
                &[
                    "enhanced/ct/multiframe_shared_perframe_explicit_le",
                    "derived/presentation-state/grayscale_softcopy_ct_window_explicit_le",
                ],
                Some("derived/presentation-state/grayscale_softcopy_ct_window_explicit_le"),
                None,
            ),
            ("derived/presentation-state/color", _) => (
                &[
                    "classic/sc/rgb_planar0_explicit_le",
                    "derived/presentation-state/color_softcopy",
                ],
                Some("derived/presentation-state/color_softcopy"),
                None,
            ),
            ("derived/presentation-state/blending", _) => (
                &[
                    "geometry/ct/multiseries_shared_frame_of_reference",
                    "derived/presentation-state/blending",
                ],
                Some("derived/presentation-state/blending"),
                None,
            ),
            ("derived/presentation-state/advanced-blending", _) => (
                &[
                    "geometry/ct/multiseries_shared_frame_of_reference",
                    "derived/presentation-state/advanced_blending",
                ],
                Some("derived/presentation-state/advanced_blending"),
                None,
            ),
            ("non-image/waveform/twelve-lead-ecg", _) => {
                (&["non-image/waveform/twelve_lead_ecg"], None, None)
            }
            ("non-image/waveform/general-ecg", _) => {
                (&["non-image/waveform/general_ecg"], None, None)
            }
            ("non-image/encapsulated-document/pdf", _) => (
                &["non-image/encapsulated-document/pdf_minimal_explicit_le"],
                None,
                None,
            ),
            ("non-image/mesh/stl", _) => (&["derived/mesh/encapsulated_stl"], None, None),
            ("derived/segmentation/binary", _) => (
                &[
                    "enhanced/ct/multiframe_shared_perframe_explicit_le",
                    "derived/seg/binary_multiframe_explicit_le",
                ],
                Some("derived/seg/binary_multiframe_explicit_le"),
                None,
            ),
            ("derived/segmentation/fractional-probability", _) => (
                &[
                    "enhanced/ct/multiframe_shared_perframe_explicit_le",
                    "derived/seg/fractional_probability_multiframe_explicit_le",
                ],
                Some("derived/seg/fractional_probability_multiframe_explicit_le"),
                None,
            ),
            ("derived/segmentation/labelmap", _) => (
                &[
                    "enhanced/ct/multiframe_shared_perframe_explicit_le",
                    "derived/seg/labelmap_multiframe_explicit_le",
                ],
                Some("derived/seg/labelmap_multiframe_explicit_le"),
                None,
            ),
            ("derived/segmentation/wsi-tile", _) => (
                &["vl/wsi/tiled_full_small", "derived/seg/wsi_tile_reference"],
                Some("derived/seg/wsi_tile_reference"),
                None,
            ),
            ("derived/parametric-map/float32", _) => (
                &[
                    "geometry/ct/spatial_sort_conflicts_instance_number",
                    "derived/parametric-map/float32_ct_derived_explicit_le",
                ],
                Some("derived/parametric-map/float32_ct_derived_explicit_le"),
                None,
            ),
            ("derived/parametric-map/float64", _) => (
                &[
                    "geometry/ct/spatial_sort_conflicts_instance_number",
                    "derived/parametric-map/float64_ct_derived_explicit_le",
                ],
                Some("derived/parametric-map/float64_ct_derived_explicit_le"),
                None,
            ),
            ("derived/real-world-value-mapping/linear", _) => (
                &[
                    "enhanced/ct/multiframe_shared_perframe_explicit_le",
                    "derived/rwvm/linear_ct_mapping_explicit_le",
                ],
                Some("derived/rwvm/linear_ct_mapping_explicit_le"),
                None,
            ),
            ("derived/structured-report/basic-text", _) => (
                &[
                    "enhanced/ct/multiframe_shared_perframe_explicit_le",
                    "derived/sr/basic_text_observation_explicit_le",
                ],
                Some("derived/sr/basic_text_observation_explicit_le"),
                None,
            ),
            ("derived/structured-report/comprehensive", _) => (
                &[
                    "enhanced/ct/multiframe_shared_perframe_explicit_le",
                    "derived/sr/comprehensive_measurement_explicit_le",
                ],
                Some("derived/sr/comprehensive_measurement_explicit_le"),
                None,
            ),
            ("derived/structured-report/comprehensive-3d", _) => (
                &[
                    "enhanced/ct/multiframe_shared_perframe_explicit_le",
                    "derived/sr/comprehensive3d_scoord3d",
                ],
                Some("derived/sr/comprehensive3d_scoord3d"),
                None,
            ),
            ("derived/structured-report/tid1500", _) => (
                &[
                    "enhanced/ct/multiframe_shared_perframe_explicit_le",
                    "derived/seg/binary_multiframe_explicit_le",
                    "derived/sr/tid1500_ct_measurement_report",
                ],
                Some("derived/sr/tid1500_ct_measurement_report"),
                None,
            ),
            ("derived/structured-report/key-object", _) => (
                &[
                    "enhanced/ct/multiframe_shared_perframe_explicit_le",
                    "derived/seg/binary_multiframe_explicit_le",
                    "derived/sr/key_object_selection_explicit_le",
                ],
                Some("derived/sr/key_object_selection_explicit_le"),
                None,
            ),
            ("non-image/rt/structure-set", _) => (
                &[
                    "enhanced/ct/multiframe_shared_perframe_explicit_le",
                    "non-image/rt/structure_set_single_roi_explicit_le",
                ],
                Some("non-image/rt/structure_set_single_roi_explicit_le"),
                None,
            ),
            ("non-image/rt/dose", _) => (
                &[
                    "enhanced/ct/multiframe_shared_perframe_explicit_le",
                    "non-image/rt/structure_set_single_roi_explicit_le",
                    "non-image/rt/dose_grid_u16_explicit_le",
                ],
                Some("non-image/rt/dose_grid_u16_explicit_le"),
                None,
            ),
            ("non-image/rt/plan", _) => (
                &[
                    "enhanced/ct/multiframe_shared_perframe_explicit_le",
                    "non-image/rt/structure_set_single_roi_explicit_le",
                    "non-image/rt/dose_grid_u16_explicit_le",
                    "non-image/rt/plan_linked",
                ],
                Some("non-image/rt/plan_linked"),
                None,
            ),
            ("non-image/rt/image", _) => (
                &[
                    "enhanced/ct/multiframe_shared_perframe_explicit_le",
                    "non-image/rt/structure_set_single_roi_explicit_le",
                    "non-image/rt/dose_grid_u16_explicit_le",
                    "non-image/rt/plan_linked",
                    "non-image/rt/image_linked",
                ],
                Some("non-image/rt/image_linked"),
                None,
            ),
            ("non-image/rt/c-arm-photon-electron-radiation", _) => (
                &[
                    "enhanced/ct/multiframe_shared_perframe_explicit_le",
                    "non-image/rt/structure_set_single_roi_explicit_le",
                    "non-image/rt/dose_grid_u16_explicit_le",
                    "non-image/rt/plan_linked",
                    "non-image/rt/carm_photon_electron_radiation_minimal",
                ],
                Some("non-image/rt/carm_photon_electron_radiation_minimal"),
                None,
            ),
            ("non-image/rt/radiation-set", _) => (
                &[
                    "enhanced/ct/multiframe_shared_perframe_explicit_le",
                    "non-image/rt/structure_set_single_roi_explicit_le",
                    "non-image/rt/dose_grid_u16_explicit_le",
                    "non-image/rt/plan_linked",
                    "non-image/rt/carm_photon_electron_radiation_minimal",
                    "non-image/rt/radiation_set_minimal",
                ],
                Some("non-image/rt/radiation_set_minimal"),
                None,
            ),
            _ => {
                return Err(GenerateError::MetadataShape {
                    path: PathBuf::from(template_id),
                    message: "composition template has no curated default artifact mapping",
                });
            }
        };
    let registry: Value = serde_json::from_str(include_str!("../cases/registry.json"))
        .expect("embedded case registry parses");
    let selected = registry["cases"]
        .as_array()
        .expect("embedded registry cases")
        .iter()
        .filter(|case| {
            case["case_id"]
                .as_str()
                .is_some_and(|case_id| case_ids.contains(&case_id))
        })
        .cloned()
        .collect::<Vec<_>>();
    if selected.len() != case_ids.len() {
        return Err(GenerateError::MetadataShape {
            path: PathBuf::from(template_id),
            message: "composition default artifact mapping is absent from the registry",
        });
    }
    let private_registry = serde_json::json!({ "cases": selected });
    let run = PreparedGenerationRun {
        profile: "all".into(),
        out_dir: staging_root.to_path_buf(),
        manifest_path: staging_root.join("manifest.json"),
        seed,
        include_stress: true,
    };
    let standards_lock_sha256 = sha256_hex(include_bytes!("../standards.lock.json"));
    let migrated_source_case_ids = case_ids
        .iter()
        .copied()
        .filter(|case_id| {
            case_id.starts_with("classic/ct/")
                || case_id.starts_with("geometry/ct/")
                || *case_id == COLOR_SOFTCOPY_PRESENTATION_STATE_SOURCE_CASE_ID
        })
        .map(str::to_owned)
        .collect::<Vec<_>>();
    fs::create_dir_all(staging_root).map_err(|source| GenerateError::CreateOutputDir {
        path: staging_root.to_path_buf(),
        source,
    })?;
    let migrated_source_plan = crate::prepare_curated_case_plan(migrated_source_case_ids, seed)?;
    let migrated_source_files =
        crate::execute_curated_sc_plan(migrated_source_plan.as_ref(), staging_root)?;
    let output = write_supported_cases_with_plan_first_sc(
        &run,
        &private_registry,
        &standards_lock_sha256,
        migrated_source_files,
    )?;
    let mut artifacts = output
        .files
        .into_iter()
        .map(|file| {
            let relative_path = generated_manifest_str(
                &file.manifest_entry,
                "/path",
                "composition default artifact path must be a string",
            )?;
            Ok(CompositionDefaultArtifact {
                case_id: file.case_id,
                path: staging_root.join(relative_path),
            })
        })
        .collect::<Result<Vec<_>, GenerateError>>()?;
    if let Some(selected_case) = selected_case {
        artifacts.retain(|artifact| artifact.case_id == selected_case);
    }
    if let Some(selected_file) = selected_file {
        artifacts.retain(|artifact| {
            artifact.path.file_name().and_then(|name| name.to_str()) == Some(selected_file)
        });
    }
    artifacts.sort_by(|left, right| left.path.cmp(&right.path));
    if artifacts.is_empty() {
        return Err(GenerateError::MetadataShape {
            path: PathBuf::from(template_id),
            message: "composition default artifact generation emitted no files",
        });
    }
    Ok(artifacts)
}

struct FuzzSourceStagingGuard(PathBuf);

impl Drop for FuzzSourceStagingGuard {
    fn drop(&mut self) {
        if self.0.exists() {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
}

fn write_fuzz_cases(
    run: &PreparedGenerationRun,
    registry: &Value,
    standards_lock_sha256: &str,
) -> Result<GenerationOutput, GenerateError> {
    const CASE_ID: &str = "fuzz/parser/bounded_seed_corpus";
    let Some(case) = registry_case(registry, CASE_ID)? else {
        return Ok(GenerationOutput::default());
    };
    if !should_generate_case(case, run)? {
        return Ok(GenerationOutput::default());
    }

    let source_staging = FuzzSourceStagingGuard(run.out_dir.join(".fuzz-private-sources"));
    let source_run = PreparedGenerationRun {
        profile: "fuzz-private-source".to_string(),
        out_dir: source_staging.0.clone(),
        manifest_path: source_staging.0.join("manifest.json"),
        seed: 7,
        include_stress: false,
    };
    let sources = write_negative_source_artifacts(
        &source_run,
        registry,
        standards_lock_sha256,
        &source_staging.0,
    )?;

    let budget = crate::fuzz::FuzzBudget {
        max_iterations: 64,
        max_candidates: 64,
        max_mutations_per_candidate: 8,
        max_total_mutations: 512,
        max_bytes_per_mutation: 64,
        max_input_bytes: 8 * 1024 * 1024,
        max_output_bytes: 8 * 1024 * 1024,
        max_minimization_attempts: 256,
        max_total_target_operations: 100_000_000,
        max_target_operations: 1_000_000,
    };
    let mut seed_records = Vec::new();
    let mut outcome_counts = BTreeMap::<&'static str, u64>::from([
        ("accepted", 0),
        ("clean_rejection", 0),
        ("parse_failure", 0),
        ("validation_failure", 0),
        ("decode_failure", 0),
        ("crash", 0),
        ("hang", 0),
        ("timeout", 0),
        ("resource_limit", 0),
    ]);
    let mut total_iterations = 0_u64;
    let mut total_candidates = 0_u64;
    let mut total_mutations = 0_u64;
    let mut total_target_operations = 0_u64;
    let mut minimizations = Vec::new();

    for description in crate::fuzz::INITIAL_SEED_DESCRIPTIONS {
        let source = sources.get(description.source_case_id).ok_or_else(|| {
            GenerateError::MetadataShape {
                path: PathBuf::from(description.source_case_id),
                message: "fuzz seed source was not generated in private staging",
            }
        })?;
        seed_records.push(serde_json::json!({
            "id": description.id,
            "source_case_id": description.source_case_id,
            "source_recipe_id": description.source_recipe_id,
            "source_recipe_version": description.source_recipe_version,
            "source_generation_seed": description.source_generation_seed,
            "source_sha256": super::sha256_hex(&source.bytes),
            "source_size_bytes": source.bytes.len(),
            "surfaces": description.surfaces.iter().map(fuzz_surface_name).collect::<Vec<_>>()
        }));

        let mut session = crate::fuzz::FuzzSession::new(*description, run.seed, budget)
            .map_err(|error| fuzz_generation_error(CASE_ID, error))?;
        let mut first_rejection = None;
        for _ in 0..32 {
            let candidate = session
                .next_candidate(&source.bytes)
                .map_err(|error| fuzz_generation_error(CASE_ID, error))?;
            let observation =
                fuzz_target_observation(&candidate.bytes, budget.max_target_operations);
            total_target_operations = total_target_operations
                .checked_add(observation.operations)
                .ok_or_else(|| GenerateError::MetadataShape {
                    path: PathBuf::from(CASE_ID),
                    message: "fuzz target operation counter overflowed",
                })?;
            *outcome_counts
                .get_mut(fuzz_outcome_name(observation.outcome.class()))
                .expect("all fuzz outcomes have initialized counters") += 1;
            if first_rejection.is_none()
                && matches!(
                    observation.outcome.class(),
                    crate::fuzz::TargetOutcomeClass::CleanRejection
                        | crate::fuzz::TargetOutcomeClass::ParseFailure
                )
            {
                first_rejection = Some((candidate, observation.outcome.class()));
            }
        }
        let counters = session.counters();
        total_iterations += counters.iterations;
        total_candidates += counters.candidates;
        total_mutations += counters.mutations;

        if let Some((candidate, outcome)) = first_rejection {
            let minimized = crate::fuzz::minimize_candidate(
                &candidate.bytes,
                outcome,
                budget,
                fuzz_target_observation,
            )
            .map_err(|error| fuzz_generation_error(CASE_ID, error))?;
            total_target_operations = total_target_operations
                .checked_add(minimized.target_operations)
                .ok_or_else(|| GenerateError::MetadataShape {
                    path: PathBuf::from(CASE_ID),
                    message: "fuzz minimization operation counter overflowed",
                })?;
            minimizations.push(serde_json::json!({
                "seed_description_id": description.id,
                "candidate_iteration": candidate.iteration,
                "candidate_seed": candidate.candidate_seed,
                "outcome": fuzz_outcome_name(outcome),
                "original_size": candidate.bytes.len(),
                "minimized_size": minimized.bytes.len(),
                "attempts": minimized.attempts,
                "target_operations": minimized.target_operations,
                "minimized_fingerprint": fuzz_payload_fingerprint(&minimized.bytes)
            }));
        }
    }
    drop(source_staging);
    if run.out_dir.join(".fuzz-private-sources").exists() {
        return Err(GenerateError::MetadataShape {
            path: run.out_dir.clone(),
            message: "private fuzz sources survived cleanup",
        });
    }

    let unacceptable = ["crash", "hang", "timeout", "resource_limit"]
        .iter()
        .map(|name| outcome_counts[name])
        .sum::<u64>();
    let qualification = serde_json::json!({
        "case_id": CASE_ID,
        "kind": "bounded_fuzz_run",
        "contract_version": crate::fuzz::FUZZ_CONTRACT_VERSION,
        "profile": "fuzz",
        "run_seed": run.seed,
        "provider": {"kind": "mutation_layer", "id": "bounded_deterministic_fuzz"},
        "target": {
            "kind": "same_project_bounded_part10_probe",
            "independence": "same_project",
            "operation_unit": "input_byte"
        },
        "budget": {
            "max_iterations": budget.max_iterations,
            "max_candidates": budget.max_candidates,
            "max_mutations_per_candidate": budget.max_mutations_per_candidate,
            "max_total_mutations": budget.max_total_mutations,
            "max_bytes_per_mutation": budget.max_bytes_per_mutation,
            "max_input_bytes": budget.max_input_bytes,
            "max_output_bytes": budget.max_output_bytes,
            "max_minimization_attempts": budget.max_minimization_attempts,
            "max_total_target_operations": budget.max_total_target_operations,
            "max_target_operations": budget.max_target_operations
        },
        "seeds": seed_records,
        "counters": {
            "iterations": total_iterations,
            "candidates": total_candidates,
            "mutations": total_mutations,
            "target_operations": total_target_operations
        },
        "outcomes": outcome_counts,
        "minimizations": minimizations,
        "unacceptable_outcomes": ["crash", "hang", "timeout", "resource_limit"],
        "payload_policy": "generated_payloads_uncommitted",
        "status": if unacceptable == 0 { "passed" } else { "failed" }
    });
    Ok(GenerationOutput {
        files: Vec::new(),
        unavailable_cases: Vec::new(),
        qualifications: vec![qualification],
        completed_case_ids: vec![CASE_ID.to_string()],
    })
}

fn fuzz_generation_error(case_id: &str, error: crate::fuzz::FuzzError) -> GenerateError {
    GenerateError::MetadataShape {
        path: PathBuf::from(case_id),
        message: error.to_string().leak(),
    }
}

fn fuzz_target_observation(bytes: &[u8], operation_limit: u64) -> crate::fuzz::TargetObservation {
    let operations = u64::try_from(bytes.len()).unwrap_or(u64::MAX).max(1);
    if operations > operation_limit {
        return crate::fuzz::TargetObservation {
            outcome: crate::fuzz::TargetOutcome::ResourceLimit,
            operations: operation_limit,
        };
    }
    let outcome = match crate::part10_locator::locate_explicit_vr_le_part10(
        bytes,
        crate::part10_locator::LocatorLimits {
            max_elements: 100_000,
            max_depth: 32,
            max_items: 100_000,
            max_fragments: 100_000,
        },
    ) {
        Ok(_) => crate::fuzz::TargetOutcome::Accepted,
        Err(crate::part10_locator::LocatorError::NotPart10) => {
            crate::fuzz::TargetOutcome::CleanRejection
        }
        Err(_) => crate::fuzz::TargetOutcome::ParseFailure,
    };
    crate::fuzz::TargetObservation {
        outcome,
        operations,
    }
}

fn fuzz_surface_name(surface: &crate::fuzz::MutationSurface) -> &'static str {
    match surface {
        crate::fuzz::MutationSurface::FileMeta => "file_meta",
        crate::fuzz::MutationSurface::DatasetHeaders => "dataset_headers",
        crate::fuzz::MutationSurface::SequenceStructure => "sequence_structure",
        crate::fuzz::MutationSurface::Encapsulation => "encapsulation",
        crate::fuzz::MutationSurface::PixelData => "pixel_data",
        crate::fuzz::MutationSurface::TextValues => "text_values",
    }
}

fn fuzz_outcome_name(outcome: crate::fuzz::TargetOutcomeClass) -> &'static str {
    match outcome {
        crate::fuzz::TargetOutcomeClass::Accepted => "accepted",
        crate::fuzz::TargetOutcomeClass::CleanRejection => "clean_rejection",
        crate::fuzz::TargetOutcomeClass::ParseFailure => "parse_failure",
        crate::fuzz::TargetOutcomeClass::ValidationFailure => "validation_failure",
        crate::fuzz::TargetOutcomeClass::DecodeFailure => "decode_failure",
        crate::fuzz::TargetOutcomeClass::Crash => "crash",
        crate::fuzz::TargetOutcomeClass::Hang => "hang",
        crate::fuzz::TargetOutcomeClass::Timeout => "timeout",
        crate::fuzz::TargetOutcomeClass::ResourceLimit => "resource_limit",
    }
}

fn fuzz_payload_fingerprint(bytes: &[u8]) -> String {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("fnv1a64:{hash:016x}")
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

enum ParametricMapCaseOutcome {
    Generated(GeneratedFile),
    Unavailable(Value),
}

enum WsiTileSegmentationCaseOutcome {
    Generated(GeneratedFile),
    Unavailable(Value),
}

struct ParametricMapStagingGuard(PathBuf);

impl ParametricMapStagingGuard {
    fn new() -> Self {
        let counter = PARAMETRIC_MAP_STAGING_COUNTER.fetch_add(1, Ordering::Relaxed);
        Self(std::env::temp_dir().join(format!(
            "dicom-test-suite-parametric-map-{}-{counter}",
            std::process::id()
        )))
    }

    fn path(&self) -> &std::path::Path {
        &self.0
    }
}

impl Drop for ParametricMapStagingGuard {
    fn drop(&mut self) {
        if self.0.exists() {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
}

fn write_parametric_map_case(
    run: &PreparedGenerationRun,
    case: &Value,
    spec: ParametricMapSpec,
    sources: &[GeneratedSourceObject],
    standards_lock_sha256: &str,
) -> Result<ParametricMapCaseOutcome, GenerateError> {
    if sources.len() != 3 {
        return Err(GenerateError::MetadataShape {
            path: PathBuf::from(spec.case_id),
            message: "Parametric Map proof requires three generated CT source instances",
        });
    }
    let first = &sources[0];
    let source_series_instance_uid =
        first
            .series_instance_uid
            .as_deref()
            .ok_or(GenerateError::MetadataShape {
                path: PathBuf::from(spec.case_id),
                message: "Parametric Map source must record a Series Instance UID",
            })?;
    let frame_of_reference_uid =
        first
            .frame_of_reference_uid
            .as_deref()
            .ok_or(GenerateError::MetadataShape {
                path: PathBuf::from(spec.case_id),
                message: "Parametric Map source must record a Frame of Reference UID",
            })?;
    if sources.iter().any(|source| {
        source.study_instance_uid != first.study_instance_uid
            || source.series_instance_uid.as_deref() != Some(source_series_instance_uid)
            || source.frame_of_reference_uid.as_deref() != Some(frame_of_reference_uid)
            || source.frame_count != Some(1)
    }) {
        return Err(GenerateError::MetadataShape {
            path: PathBuf::from(spec.case_id),
            message: "Parametric Map CT sources must share Study, Series, and Frame of Reference identity and be single-frame",
        });
    }

    let uid = |role| {
        deterministic_uid(&DeterministicUidInput {
            standards_lock_sha256,
            case_id: spec.case_id,
            recipe_version: PARAMETRIC_MAP_RECIPE_VERSION,
            run_seed: run.seed,
            file_index: 0,
            frame_index: None,
            referenced_object_index: None,
            role,
        })
    };
    let identities = ParametricMapIdentities {
        study_instance_uid: first.study_instance_uid.clone(),
        series_instance_uid: uid(UidRole::SeriesInstance),
        frame_of_reference_uid: frame_of_reference_uid.to_string(),
        sop_instance_uid: uid(UidRole::SopInstance),
        dimension_organization_uid: uid(UidRole::DimensionOrganization),
    };
    let standards_lock_path = PathBuf::from("standards.lock.json");
    let standards_lock_bytes =
        fs::read(&standards_lock_path).map_err(|source| GenerateError::ReadMetadata {
            path: standards_lock_path.clone(),
            source,
        })?;
    let standards_lock: Value =
        serde_json::from_slice(&standards_lock_bytes).map_err(|source| {
            GenerateError::ParseMetadata {
                path: standards_lock_path,
                source,
            }
        })?;
    let staging = ParametricMapStagingGuard::new();
    let input = ParametricMapGenerationInput {
        repository_root: PathBuf::from("."),
        generated_root: run.out_dir.clone(),
        staging_root: staging.path().to_path_buf(),
        destination_root: run.out_dir.join(spec.case_id),
        seed: run.seed,
        standards: StandardsProvenance {
            standards_lock_sha256: standards_lock_sha256.to_string(),
            dicom_base_edition: standards_lock["dicom_base_edition"]
                .as_str()
                .ok_or(GenerateError::MetadataShape {
                    path: PathBuf::from("standards.lock.json"),
                    message: "standards lock dicom_base_edition must be a string",
                })?
                .to_string(),
            kb_source_manifest_sha256: standards_lock
                .pointer("/dicom_standard_kb/source_manifest_sha256")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
        },
        controlled_metadata: ControlledMetadata {
            patient_name: "DTS^Synthetic^Patient001".to_string(),
            patient_id: "DTS-PATIENT-001".to_string(),
            manufacturer: "dicom-test-suite".to_string(),
            model_name: spec.recipe_id.to_string(),
            software_versions: env!("CARGO_PKG_VERSION").to_string(),
            study_date: "20260101".to_string(),
            study_time: "000000".to_string(),
            content_date: "20260101".to_string(),
            content_time: "000000".to_string(),
            timezone_offset_from_utc: "+0000".to_string(),
        },
        identities: identities.clone(),
        sources: sources
            .iter()
            .map(|source| ParametricMapSource {
                role: "source_image".to_string(),
                source_case_id: source.source_case_id.clone(),
                relative_path: source.source_path.clone(),
                sha256: source.sha256.clone(),
                sop_class_uid: source.sop_class_uid.clone(),
                sop_instance_uid: source.sop_instance_uid.clone(),
                series_instance_uid: source.series_instance_uid.clone(),
                frame_numbers: None,
            })
            .collect(),
        stored_value_scale: PARAMETRIC_MAP_STORED_VALUE_SCALE,
        spatial_rank_increment: match spec.sample_kind {
            ParametricMapSampleKind::Float32 => PARAMETRIC_MAP_FLOAT32_SPATIAL_RANK_INCREMENT,
            ParametricMapSampleKind::Float64 => PARAMETRIC_MAP_FLOAT64_SPATIAL_RANK_INCREMENT,
        },
    };
    let outcome = generate_parametric_map_for_spec(&input, spec).map_err(|error| {
        GenerateError::WriteDicomFile {
            path: PathBuf::from(spec.case_id),
            message: error.to_string(),
        }
    })?;
    match outcome {
        ParametricMapVariantOutcome::Unavailable { code, message } => {
            Ok(ParametricMapCaseOutcome::Unavailable(serde_json::json!({
                "case_id": spec.case_id,
                "status": "unavailable",
                "reason_code": "external_backend_unavailable",
                "message": format!("{code}: {message}"),
                "recheck_phase": "phase-1",
                "standards_evidence": standards_evidence_from_case(case)
            })))
        }
        ParametricMapVariantOutcome::Generated(generated) => {
            Ok(ParametricMapCaseOutcome::Generated(
                parametric_map_generated_file(case, sources, generated)?,
            ))
        }
    }
}

fn parametric_map_generated_file(
    case: &Value,
    sources: &[GeneratedSourceObject],
    generated: ParametricMapVariantGenerated,
) -> Result<GeneratedFile, GenerateError> {
    let object =
        open_file(&generated.output_path).map_err(|error| GenerateError::ValidateDicomFile {
            path: generated.output_path.clone(),
            message: format!("reopen promoted Parametric Map: {error}"),
        })?;
    let (
        pixel_tag,
        expected_vr,
        pixel_label,
        sample_type,
        bits_allocated,
        rows,
        columns,
        frames,
        expected_bytes,
        frame_hashes,
        minimum,
        maximum,
        bits_key,
        bits_value,
        render_capability,
        visual_pattern,
        pixel_stressor,
        spatial_rank_increment,
    ) = match &generated.payload {
        ParametricMapPayload::Float32(payload) => (
            tags::FLOAT_PIXEL_DATA,
            VR::OF,
            "Float Pixel Data",
            "float32",
            32,
            payload.rows,
            payload.columns,
            payload.frames,
            payload.little_endian_bytes.as_slice(),
            payload.frame_sha256.clone(),
            serde_json::json!(payload.minimum),
            serde_json::json!(payload.maximum),
            "little_endian_float32_bits",
            serde_json::json!(payload.little_endian_float32_bits),
            "render_float_pixels",
            "three_frame_ct_derived_float32_parametric_map",
            "float_pixel_data",
            PARAMETRIC_MAP_FLOAT32_SPATIAL_RANK_INCREMENT,
        ),
        ParametricMapPayload::Float64(payload) => (
            tags::DOUBLE_FLOAT_PIXEL_DATA,
            VR::OD,
            "Double Float Pixel Data",
            "float64",
            64,
            payload.rows,
            payload.columns,
            payload.frames,
            payload.little_endian_bytes.as_slice(),
            payload.frame_sha256.clone(),
            serde_json::json!(payload.minimum),
            serde_json::json!(payload.maximum),
            "little_endian_float64_bits",
            serde_json::json!(payload.little_endian_float64_bits),
            "render_double_float_pixels",
            "three_frame_ct_derived_float64_parametric_map",
            "double_float_pixel_data",
            PARAMETRIC_MAP_FLOAT64_SPATIAL_RANK_INCREMENT,
        ),
    };
    let pixel_data =
        object
            .element(pixel_tag)
            .map_err(|error| GenerateError::ValidateDicomFile {
                path: generated.output_path.clone(),
                message: format!("read promoted {pixel_label}: {error}"),
            })?;
    if pixel_data.vr() != expected_vr {
        return Err(GenerateError::ValidateDicomFile {
            path: generated.output_path.clone(),
            message: format!(
                "promoted {pixel_label} VR is {:?}, expected {expected_vr:?}",
                pixel_data.vr()
            ),
        });
    }
    let actual_bytes =
        pixel_data
            .value()
            .to_bytes()
            .map_err(|error| GenerateError::ValidateDicomFile {
                path: generated.output_path.clone(),
                message: format!("decode promoted {pixel_label}: {error}"),
            })?;
    if actual_bytes.as_ref() != expected_bytes {
        return Err(GenerateError::ValidateDicomFile {
            path: generated.output_path.clone(),
            message: format!(
                "promoted {pixel_label} differs from Rust source-derived expectations"
            ),
        });
    }
    let forbidden_pixel_tags = match generated.spec.sample_kind {
        ParametricMapSampleKind::Float32 => [
            (tags::PIXEL_DATA, "integer Pixel Data"),
            (tags::DOUBLE_FLOAT_PIXEL_DATA, "Double Float Pixel Data"),
        ],
        ParametricMapSampleKind::Float64 => [
            (tags::PIXEL_DATA, "integer Pixel Data"),
            (tags::FLOAT_PIXEL_DATA, "Float Pixel Data"),
        ],
    };
    for (tag, label) in forbidden_pixel_tags.into_iter().chain([
        (tags::BITS_STORED, "Bits Stored"),
        (tags::HIGH_BIT, "High Bit"),
        (tags::PIXEL_REPRESENTATION, "Pixel Representation"),
    ]) {
        if object.element_opt(tag).ok().flatten().is_some() {
            return Err(GenerateError::ValidateDicomFile {
                path: generated.output_path.clone(),
                message: format!(
                    "promoted {sample_type} Parametric Map contains unexpected {label}"
                ),
            });
        }
    }
    let meta = object.meta();
    let implementation_version_name = meta
        .implementation_version_name
        .clone()
        .map(|value| value.trim().to_string())
        .unwrap_or_else(|| "UNKNOWN".to_string());
    let response_backend = &generated.response["backend"];
    let warnings = generated.response["warnings"].clone();
    let output = generated
        .response
        .get("outputs")
        .and_then(Value::as_array)
        .and_then(|outputs| outputs.first())
        .ok_or(GenerateError::MetadataShape {
            path: generated.output_path.clone(),
            message: "Parametric Map response must contain one output",
        })?;
    let references = sources
        .iter()
        .map(|source| source.to_manifest_reference("source_image", None))
        .collect::<Vec<_>>();
    let validation = serde_json::json!({
        "status": "passed",
        "internal": [
            {"name": "external_backend_contract", "status": "passed", "message": "The locked backend response and provenance satisfied protocol 0.1.0."},
            {"name": "floating_payload_recomputed", "status": "passed", "message": format!("Rust recomputed every {sample_type} bit pattern and frame hash from the staged CT sources.")},
            {"name": "promoted_part10_reopened", "status": "passed", "message": "The promoted Parametric Map reopened as a Part 10 file."}
        ],
        "standards": [
            {"name": "parametric_map_storage_sop_class", "status": "passed", "message": "The output uses Parametric Map Storage."},
            {"name": pixel_stressor, "status": "passed", "message": format!("The payload is native {pixel_label} with {expected_vr:?} VR.")}
        ],
        "external": []
    });
    let mut recipe_parameters = serde_json::json!({
        "stored_value_scale": PARAMETRIC_MAP_STORED_VALUE_SCALE,
        "spatial_rank_increment": spatial_rank_increment,
        "dimension_organization_uid": generated.identities.dimension_organization_uid
    });
    recipe_parameters
        .as_object_mut()
        .expect("recipe parameters are an object")
        .insert(bits_key.to_string(), bits_value.clone());
    let expected_capabilities = vec![
        "open_file",
        "read_metadata",
        render_capability,
        "parse_multiframe_functional_groups",
        "apply_real_world_value_mapping",
    ];
    let known_stressors = vec![
        "parametric_map_storage",
        pixel_stressor,
        "native_multiframe_pixel_data",
        "real_world_value_mapping",
        "cross_instance_references",
        "external_generation_backend",
    ];
    let mut expected_semantics = serde_json::json!({
        "synthetic_data": "YES",
        "sample_type": sample_type,
        "pixel_min": minimum,
        "pixel_max": maximum,
        "shared_functional_groups_sequence_items": 1,
        "per_frame_functional_groups_sequence_items": frames,
        "dimension_organization_uid": generated.identities.dimension_organization_uid,
        "source_reference_count": sources.len(),
        "real_world_value_mapping": {
            "lut_label": output["expected_semantics"]["real_world_value_mapping"]["lut_label"],
            "slope": output["expected_semantics"]["real_world_value_mapping"]["slope"],
            "intercept": output["expected_semantics"]["real_world_value_mapping"]["intercept"],
            "units": {
                "code_value": output["expected_semantics"]["real_world_value_mapping"]["unit"]["value"],
                "coding_scheme_designator": output["expected_semantics"]["real_world_value_mapping"]["unit"]["scheme"],
                "code_meaning": output["expected_semantics"]["real_world_value_mapping"]["unit"]["meaning"]
            },
            "quantity_definition": {
                "code_value": output["expected_semantics"]["real_world_value_mapping"]["quantity"]["value"],
                "coding_scheme_designator": output["expected_semantics"]["real_world_value_mapping"]["quantity"]["scheme"],
                "code_meaning": output["expected_semantics"]["real_world_value_mapping"]["quantity"]["meaning"]
            }
        }
    });
    expected_semantics
        .as_object_mut()
        .expect("expected semantics are an object")
        .insert(bits_key.to_string(), bits_value);
    Ok(GeneratedFile {
        case_id: generated.spec.case_id.to_string(),
        manifest_entry: serde_json::json!({
            "case_id": generated.spec.case_id,
            "profile_membership": ["extended"],
            "path": format!("{}/{}", generated.spec.case_id, generated.spec.output_file),
            "sha256": sha256_hex(&generated.output_bytes),
            "size_bytes": generated.output_bytes.len(),
            "determinism": "semantic_stable",
            "recipe": {
                "recipe_id": generated.spec.recipe_id,
                "recipe_version": PARAMETRIC_MAP_RECIPE_VERSION,
                "recipe_parameters": recipe_parameters
            },
            "dicom": {
                "sop_class_uid": PARAMETRIC_MAP_SOP_CLASS_UID,
                "sop_class_name": "Parametric Map Storage",
                "iod_name": "Parametric Map",
                "modality": "OT",
                "transfer_syntax_uid": PARAMETRIC_MAP_TRANSFER_SYNTAX_UID,
                "transfer_syntax_name": "Explicit VR Little Endian"
            },
            "uids": {
                "study_instance_uid": generated.identities.study_instance_uid,
                "series_instance_uid": generated.identities.series_instance_uid,
                "sop_instance_uid": generated.identities.sop_instance_uid,
                "frame_of_reference_uid": generated.identities.frame_of_reference_uid,
                "implementation_class_uid": meta.implementation_class_uid,
                "implementation_version_name": implementation_version_name
            },
            "image": {
                "sample_type": sample_type,
                "rows": rows,
                "columns": columns,
                "frames": frames,
                "samples_per_pixel": 1,
                "photometric_interpretation": "MONOCHROME2",
                "bits_allocated": bits_allocated,
                "planar_configuration": Value::Null
            },
            "pixel_data": {
                "vr": format!("{expected_vr:?}"),
                "native_or_encapsulated": "native",
                "value_length": expected_bytes.len(),
                "frame_count": frames,
                "frame_hashes": frame_hashes
            },
            "generation_backend": {
                "backend_id": generated.backend.backend_id,
                "protocol_version": crate::generation_backends::PROTOCOL_VERSION,
                "name": response_backend["name"],
                "version": response_backend["version"],
                "dependency_lock_sha256": generated.backend.dependency_lock_sha256,
                "executable_fingerprint": generated.backend.executable_fingerprint,
                "entrypoint_fingerprint": generated.backend.entrypoint_fingerprint,
                "environment_fingerprint": generated.backend.environment_fingerprint,
                "runtime_identity": generated.backend.runtime_identity,
                "determinism": "semantic_stable",
                "warnings": warnings
            },
            "references": references,
            "expected_capabilities": expected_capabilities,
            "expected_semantics": expected_semantics,
            "expected_visual_checks": {"pattern": visual_pattern},
            "validation": validation,
            "known_stressors": known_stressors,
            "standards_evidence": deduplicated_standards_evidence(standards_evidence_from_case(case))
        }),
    })
}

fn write_wsi_tile_segmentation_case(
    run: &PreparedGenerationRun,
    case: &Value,
    source: &GeneratedSourceObject,
    standards_lock_sha256: &str,
) -> Result<WsiTileSegmentationCaseOutcome, GenerateError> {
    let source_series_instance_uid =
        source
            .series_instance_uid
            .as_deref()
            .ok_or(GenerateError::MetadataShape {
                path: PathBuf::from(WSI_TILE_SEGMENTATION_CASE_ID),
                message: "WSI tile segmentation source must record a Series Instance UID",
            })?;
    let frame_of_reference_uid =
        source
            .frame_of_reference_uid
            .as_deref()
            .ok_or(GenerateError::MetadataShape {
                path: PathBuf::from(WSI_TILE_SEGMENTATION_CASE_ID),
                message: "WSI tile segmentation source must record a Frame of Reference UID",
            })?;
    if source.frame_count != Some(4)
        || source.specimen_uid.is_none()
        || source.container_identifier.as_deref() != Some("DTS-SLIDE-001")
    {
        return Err(GenerateError::MetadataShape {
            path: PathBuf::from(WSI_TILE_SEGMENTATION_CASE_ID),
            message: "WSI tile segmentation source must expose the locked four-frame specimen contract",
        });
    }
    let uid = |role| {
        deterministic_uid(&DeterministicUidInput {
            standards_lock_sha256,
            case_id: WSI_TILE_SEGMENTATION_CASE_ID,
            recipe_version: WSI_TILE_SEGMENTATION_RECIPE_VERSION,
            run_seed: run.seed,
            file_index: 0,
            frame_index: None,
            referenced_object_index: None,
            role,
        })
    };
    let identities = WsiTileSegmentationIdentities {
        study_instance_uid: source.study_instance_uid.clone(),
        series_instance_uid: uid(UidRole::SeriesInstance),
        frame_of_reference_uid: frame_of_reference_uid.to_string(),
        sop_instance_uid: uid(UidRole::SopInstance),
        dimension_organization_uid: uid(UidRole::DimensionOrganization),
    };
    let standards_lock_path = PathBuf::from("standards.lock.json");
    let standards_lock_bytes =
        fs::read(&standards_lock_path).map_err(|source| GenerateError::ReadMetadata {
            path: standards_lock_path.clone(),
            source,
        })?;
    let standards_lock: Value =
        serde_json::from_slice(&standards_lock_bytes).map_err(|source| {
            GenerateError::ParseMetadata {
                path: standards_lock_path,
                source,
            }
        })?;
    let staging = ParametricMapStagingGuard::new();
    let input = WsiTileSegmentationGenerationInput {
        repository_root: PathBuf::from("."),
        generated_root: run.out_dir.clone(),
        staging_root: staging.path().to_path_buf(),
        destination_root: run.out_dir.join(WSI_TILE_SEGMENTATION_CASE_ID),
        seed: run.seed,
        standards: StandardsProvenance {
            standards_lock_sha256: standards_lock_sha256.to_string(),
            dicom_base_edition: standards_lock["dicom_base_edition"]
                .as_str()
                .ok_or(GenerateError::MetadataShape {
                    path: PathBuf::from("standards.lock.json"),
                    message: "standards lock dicom_base_edition must be a string",
                })?
                .to_string(),
            kb_source_manifest_sha256: standards_lock
                .pointer("/dicom_standard_kb/source_manifest_sha256")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
        },
        controlled_metadata: ControlledMetadata {
            patient_name: "DTS^Synthetic^Patient001".to_string(),
            patient_id: "DTS-PATIENT-001".to_string(),
            manufacturer: "dicom-test-suite".to_string(),
            model_name: WSI_TILE_SEGMENTATION_RECIPE_ID.to_string(),
            software_versions: env!("CARGO_PKG_VERSION").to_string(),
            study_date: "20260101".to_string(),
            study_time: "000000".to_string(),
            content_date: "20260101".to_string(),
            content_time: "000000".to_string(),
            timezone_offset_from_utc: "+0000".to_string(),
        },
        identities,
        source: ParametricMapSource {
            role: "source_image".to_string(),
            source_case_id: source.source_case_id.clone(),
            relative_path: source.source_path.clone(),
            sha256: source.sha256.clone(),
            sop_class_uid: source.sop_class_uid.clone(),
            sop_instance_uid: source.sop_instance_uid.clone(),
            series_instance_uid: Some(source_series_instance_uid.to_string()),
            frame_numbers: Some(WSI_TILE_SEGMENTATION_SOURCE_FRAME_NUMBERS.to_vec()),
        },
    };
    let source_path = input.generated_root.join(&source.source_path);
    match generate_wsi_tile_segmentation(&input).map_err(|error| GenerateError::WriteDicomFile {
        path: PathBuf::from(WSI_TILE_SEGMENTATION_CASE_ID),
        message: error.to_string(),
    })? {
        WsiTileSegmentationOutcome::Unavailable { code, message } => Ok(
            WsiTileSegmentationCaseOutcome::Unavailable(serde_json::json!({
                "case_id": WSI_TILE_SEGMENTATION_CASE_ID,
                "status": "unavailable",
                "reason_code": "external_backend_unavailable",
                "message": format!("{code}: {message}"),
                "recheck_phase": "phase-4",
                "standards_evidence": standards_evidence_from_case(case)
            })),
        ),
        WsiTileSegmentationOutcome::Generated(generated) => {
            Ok(WsiTileSegmentationCaseOutcome::Generated(
                wsi_tile_segmentation_generated_file(case, source, &source_path, generated)?,
            ))
        }
    }
}

fn wsi_tile_segmentation_generated_file(
    case: &Value,
    source: &GeneratedSourceObject,
    source_path: &Path,
    generated: WsiTileSegmentationGenerated,
) -> Result<GeneratedFile, GenerateError> {
    let object =
        open_file(&generated.output_path).map_err(|error| GenerateError::ValidateDicomFile {
            path: generated.output_path.clone(),
            message: format!("reopen promoted WSI tile segmentation: {error}"),
        })?;
    let meta = object.meta();
    let implementation_version_name = meta
        .implementation_version_name
        .clone()
        .map(|value| value.trim().to_string())
        .unwrap_or_else(|| "UNKNOWN".to_string());
    let implementation_class_uid = meta.implementation_class_uid().to_string();
    let identity = Part10Expectations {
        sop_class_uid: SEGMENTATION_STORAGE_UID,
        sop_instance_uid: &generated.identities.sop_instance_uid,
        transfer_syntax_uid: PARAMETRIC_MAP_TRANSFER_SYNTAX_UID,
        implementation_class_uid: &implementation_class_uid,
        synthetic_data: "YES",
        rows: 2,
        columns: 2,
        frames: 2,
        samples_per_pixel: 1,
        photometric_interpretation: "MONOCHROME2",
        bits_allocated: 8,
        bits_stored: 8,
        high_bit: 7,
        pixel_representation: 0,
        planar_configuration: None,
        pixel_data_vr: VR::OB,
        pixel_data_length_formula: PixelDataLengthFormula::ContiguousSamples,
        decoded_frame_hashes: &WSI_TILE_SEGMENTATION_FRAME_SHA256,
        palette: None,
        padding: None,
        ct_image: None,
        enhanced_ct_image: None,
        enhanced_mr_image: None,
        enhanced_pet_image: None,
        mg_image: None,
        dx_image: None,
        xa_image: None,
        xrf_image: None,
        us_image: None,
        us_multiframe: None,
        nm_image: None,
        pet_image: None,
        cr_image: None,
        mr_image: None,
        segmentation: None,
    };
    let strict = WsiTileSegmentationExpectations {
        source_path,
        source_sha256: &source.sha256,
        source_study_instance_uid: &source.study_instance_uid,
        source_series_instance_uid: source
            .series_instance_uid
            .as_deref()
            .expect("source validated series UID"),
        source_sop_class_uid: &source.sop_class_uid,
        source_sop_instance_uid: &source.sop_instance_uid,
        frame_of_reference_uid: source
            .frame_of_reference_uid
            .as_deref()
            .expect("source validated Frame of Reference UID"),
        dimension_organization_uid: &generated.identities.dimension_organization_uid,
        specimen_uid: source
            .specimen_uid
            .as_deref()
            .expect("source validated specimen UID"),
        container_identifier: source
            .container_identifier
            .as_deref()
            .expect("source validated container identifier"),
    };
    let mut validated =
        validate_wsi_tile_segmentation_file(&generated.output_path, &identity, &strict)?;
    append_internal_validation(
        &mut validated.validation,
        serde_json::json!({
            "name": "external_backend_contract",
            "status": "passed",
            "message": "The locked highdicom response, provenance, payload, source Frames 1 and 4, and resource ceilings satisfied protocol 0.1.0."
        }),
    );
    let source_series = source
        .series_instance_uid
        .as_deref()
        .expect("source validated series UID");
    let specimen_uid = source
        .specimen_uid
        .as_deref()
        .expect("source validated specimen UID");
    let expected =
        crate::wsi_tile_segmentation_locked_contract(crate::WsiTileSegmentationLockedInputs {
            source_case_id: &source.source_case_id,
            source_path: &source.source_path,
            source_sha256: &source.sha256,
            source_study_instance_uid: &source.study_instance_uid,
            source_series_instance_uid: source_series,
            source_sop_class_uid: &source.sop_class_uid,
            source_sop_instance_uid: &source.sop_instance_uid,
            frame_of_reference_uid: &generated.identities.frame_of_reference_uid,
            specimen_uid,
            dimension_organization_uid: &generated.identities.dimension_organization_uid,
        });
    let invocation_elapsed_milliseconds =
        (generated.invocation_elapsed_seconds * 1_000.0).ceil() as u64;
    let response_backend = &generated.response["backend"];
    let warnings = generated.response["warnings"].clone();
    Ok(GeneratedFile {
        case_id: WSI_TILE_SEGMENTATION_CASE_ID.to_string(),
        manifest_entry: serde_json::json!({
            "case_id": WSI_TILE_SEGMENTATION_CASE_ID,
            "profile_membership": ["extended"],
            "path": format!("{WSI_TILE_SEGMENTATION_CASE_ID}/{WSI_TILE_SEGMENTATION_OUTPUT_FILE}"),
            "sha256": sha256_hex(&generated.output_bytes),
            "size_bytes": generated.output_bytes.len(),
            "determinism": "semantic_stable",
            "recipe": {"recipe_id": WSI_TILE_SEGMENTATION_RECIPE_ID, "recipe_version": WSI_TILE_SEGMENTATION_RECIPE_VERSION, "recipe_parameters": {"source_case_id": source.source_case_id, "source_frame_numbers": WSI_TILE_SEGMENTATION_SOURCE_FRAME_NUMBERS, "dimension_organization_uid": generated.identities.dimension_organization_uid, "segmentation_type": "FRACTIONAL", "segmentation_fractional_type": "OCCUPANCY", "maximum_fractional_value": 255, "segment_count": 1, "segment_label": "DTS_SYNTHETIC_REGION"}},
            "dicom": {"sop_class_uid": SEGMENTATION_STORAGE_UID, "sop_class_name": "Segmentation Storage", "iod_name": "Segmentation", "modality": "SEG", "transfer_syntax_uid": PARAMETRIC_MAP_TRANSFER_SYNTAX_UID, "transfer_syntax_name": "Explicit VR Little Endian"},
            "uids": {"study_instance_uid": generated.identities.study_instance_uid, "series_instance_uid": generated.identities.series_instance_uid, "sop_instance_uid": generated.identities.sop_instance_uid, "frame_of_reference_uid": generated.identities.frame_of_reference_uid, "dimension_organization_uid": generated.identities.dimension_organization_uid, "implementation_class_uid": implementation_class_uid, "implementation_version_name": implementation_version_name},
            "image": {"rows": 2, "columns": 2, "frames": 2, "samples_per_pixel": 1, "photometric_interpretation": "MONOCHROME2", "bits_allocated": 8, "bits_stored": 8, "high_bit": 7, "pixel_representation": 0, "planar_configuration": Value::Null},
            "pixel_data": {"vr": "OB", "native_or_encapsulated": "native", "value_length": 8, "frame_count": 2, "frame_hashes": WSI_TILE_SEGMENTATION_FRAME_SHA256},
            "generation_backend": {"backend_id": generated.backend.backend_id, "protocol_version": crate::generation_backends::PROTOCOL_VERSION, "name": response_backend["name"], "version": response_backend["version"], "dependency_lock_sha256": generated.backend.dependency_lock_sha256, "executable_fingerprint": generated.backend.executable_fingerprint, "entrypoint_fingerprint": generated.backend.entrypoint_fingerprint, "environment_fingerprint": generated.backend.environment_fingerprint, "runtime_identity": generated.backend.runtime_identity, "determinism": "semantic_stable", "invocation_elapsed_milliseconds": invocation_elapsed_milliseconds, "warnings": warnings},
            "references": [source.to_manifest_reference("source_image_for_segmentation", Some(WSI_TILE_SEGMENTATION_SOURCE_FRAME_NUMBERS.to_vec()))],
            "expected_capabilities": ["open_file", "read_metadata", "parse_segmentation", "reconstruct_wsi_tile_segmentation", "resolve_frame_references"],
            "expected_semantics": {"synthetic_data": "YES", "pixel_min": 0, "pixel_max": 255, "segmentation_type": "FRACTIONAL", "segmentation_fractional_type": "OCCUPANCY", "maximum_fractional_value": 255, "segment_sequence_items": 1, "shared_functional_groups_sequence_items": 1, "per_frame_functional_groups_sequence_items": 2, "source_case_id": source.source_case_id, "source_sop_instance_uid": source.sop_instance_uid, "referenced_frame_numbers": WSI_TILE_SEGMENTATION_SOURCE_FRAME_NUMBERS},
            "expected_visual_checks": {"pattern": "two_diagonal_wsi_tile_occupancy_masks"},
            "expected_wsi_tile_segmentation": expected,
            "validation": validated.validation,
            "known_stressors": ["segmentation_storage", "fractional_occupancy_pixel_data", "tiled_sparse", "wsi_tile_references", "slide_coordinate_system", "external_generation_backend"],
            "standards_evidence": deduplicated_standards_evidence(standards_evidence_from_case(case))
        }),
    })
}

enum Tid1500CaseOutcome {
    Generated(GeneratedFile),
    Unavailable(Value),
}

fn write_tid1500_case(
    run: &PreparedGenerationRun,
    case: &Value,
    ct_source: &GeneratedSourceObject,
    seg_source: &GeneratedSourceObject,
    standards_lock_sha256: &str,
) -> Result<Tid1500CaseOutcome, GenerateError> {
    let ct_series_instance_uid =
        ct_source
            .series_instance_uid
            .as_deref()
            .ok_or(GenerateError::MetadataShape {
                path: PathBuf::from(TID1500_CASE_ID),
                message: "TID 1500 Enhanced CT source must record a Series Instance UID",
            })?;
    let seg_series_instance_uid =
        seg_source
            .series_instance_uid
            .as_deref()
            .ok_or(GenerateError::MetadataShape {
                path: PathBuf::from(TID1500_CASE_ID),
                message: "TID 1500 SEG source must record a Series Instance UID",
            })?;
    let frame_of_reference_uid =
        ct_source
            .frame_of_reference_uid
            .as_deref()
            .ok_or(GenerateError::MetadataShape {
                path: PathBuf::from(TID1500_CASE_ID),
                message: "TID 1500 Enhanced CT source must record a Frame of Reference UID",
            })?;
    if ct_source.study_instance_uid != seg_source.study_instance_uid
        || seg_source.frame_of_reference_uid.as_deref() != Some(frame_of_reference_uid)
        || ct_source.frame_count != Some(2)
        || seg_source.frame_count != Some(2)
    {
        return Err(GenerateError::MetadataShape {
            path: PathBuf::from(TID1500_CASE_ID),
            message: "TID 1500 sources must share Study and Frame of Reference identity and contain two frames",
        });
    }

    let uid = |role, referenced_object_index| {
        deterministic_uid(&DeterministicUidInput {
            standards_lock_sha256,
            case_id: TID1500_CASE_ID,
            recipe_version: TID1500_RECIPE_VERSION,
            run_seed: run.seed,
            file_index: 0,
            frame_index: None,
            referenced_object_index,
            role,
        })
    };
    let identities = Tid1500Identities {
        study_instance_uid: ct_source.study_instance_uid.clone(),
        series_instance_uid: uid(UidRole::SeriesInstance, None),
        frame_of_reference_uid: frame_of_reference_uid.to_string(),
        sop_instance_uid: uid(UidRole::SopInstance, None),
        tracking_uid: uid(UidRole::DerivedReference, Some(0)),
        observer_uid: uid(UidRole::DerivedReference, Some(1)),
    };
    let standards_lock_path = PathBuf::from("standards.lock.json");
    let standards_lock_bytes =
        fs::read(&standards_lock_path).map_err(|source| GenerateError::ReadMetadata {
            path: standards_lock_path.clone(),
            source,
        })?;
    let standards_lock: Value =
        serde_json::from_slice(&standards_lock_bytes).map_err(|source| {
            GenerateError::ParseMetadata {
                path: standards_lock_path,
                source,
            }
        })?;
    let staging = ParametricMapStagingGuard::new();
    let sources = vec![
        ParametricMapSource {
            role: "source_image".to_string(),
            source_case_id: ct_source.source_case_id.clone(),
            relative_path: ct_source.source_path.clone(),
            sha256: ct_source.sha256.clone(),
            sop_class_uid: ct_source.sop_class_uid.clone(),
            sop_instance_uid: ct_source.sop_instance_uid.clone(),
            series_instance_uid: Some(ct_series_instance_uid.to_string()),
            frame_numbers: Some(vec![1, 2]),
        },
        ParametricMapSource {
            role: "segmentation".to_string(),
            source_case_id: seg_source.source_case_id.clone(),
            relative_path: seg_source.source_path.clone(),
            sha256: seg_source.sha256.clone(),
            sop_class_uid: seg_source.sop_class_uid.clone(),
            sop_instance_uid: seg_source.sop_instance_uid.clone(),
            series_instance_uid: Some(seg_series_instance_uid.to_string()),
            frame_numbers: None,
        },
    ];
    let input = Tid1500GenerationInput {
        repository_root: PathBuf::from("."),
        generated_root: run.out_dir.clone(),
        staging_root: staging.path().to_path_buf(),
        destination_root: run.out_dir.join(TID1500_CASE_ID),
        seed: run.seed,
        standards: StandardsProvenance {
            standards_lock_sha256: standards_lock_sha256.to_string(),
            dicom_base_edition: standards_lock["dicom_base_edition"]
                .as_str()
                .ok_or(GenerateError::MetadataShape {
                    path: PathBuf::from("standards.lock.json"),
                    message: "standards lock dicom_base_edition must be a string",
                })?
                .to_string(),
            kb_source_manifest_sha256: standards_lock
                .pointer("/dicom_standard_kb/source_manifest_sha256")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
        },
        controlled_metadata: ControlledMetadata {
            patient_name: "DTS^Synthetic^Patient001".to_string(),
            patient_id: "DTS-PATIENT-001".to_string(),
            manufacturer: "dicom-test-suite".to_string(),
            model_name: TID1500_RECIPE_ID.to_string(),
            software_versions: env!("CARGO_PKG_VERSION").to_string(),
            study_date: "20260101".to_string(),
            study_time: "000000".to_string(),
            content_date: "20260101".to_string(),
            content_time: "000000".to_string(),
            timezone_offset_from_utc: "+0000".to_string(),
        },
        identities,
        sources,
    };
    match generate_tid1500(&input).map_err(|error| GenerateError::WriteDicomFile {
        path: PathBuf::from(TID1500_CASE_ID),
        message: error.to_string(),
    })? {
        Tid1500Outcome::Unavailable { code, message } => {
            Ok(Tid1500CaseOutcome::Unavailable(serde_json::json!({
                "case_id": TID1500_CASE_ID,
                "status": "unavailable",
                "reason_code": "external_backend_unavailable",
                "message": format!("{code}: {message}"),
                "recheck_phase": "phase-3",
                "standards_evidence": standards_evidence_from_case(case)
            })))
        }
        Tid1500Outcome::Generated(generated) => Ok(Tid1500CaseOutcome::Generated(
            tid1500_generated_file(case, ct_source, seg_source, generated)?,
        )),
    }
}

fn tid1500_code(value: &str, scheme: &str, meaning: &str) -> Value {
    serde_json::json!({
        "code_value": value,
        "coding_scheme_designator": scheme,
        "code_meaning": meaning
    })
}

fn tid1500_generated_file(
    case: &Value,
    ct_source: &GeneratedSourceObject,
    seg_source: &GeneratedSourceObject,
    generated: Tid1500Generated,
) -> Result<GeneratedFile, GenerateError> {
    let object =
        open_file(&generated.output_path).map_err(|error| GenerateError::ValidateDicomFile {
            path: generated.output_path.clone(),
            message: format!("reopen promoted TID 1500 report: {error}"),
        })?;
    if object
        .element_opt(tags::PIXEL_DATA)
        .ok()
        .flatten()
        .is_some()
        || object
            .element_opt(tags::FLOAT_PIXEL_DATA)
            .ok()
            .flatten()
            .is_some()
        || object
            .element_opt(tags::DOUBLE_FLOAT_PIXEL_DATA)
            .ok()
            .flatten()
            .is_some()
    {
        return Err(GenerateError::ValidateDicomFile {
            path: generated.output_path,
            message: "promoted TID 1500 report unexpectedly contains pixel data".to_string(),
        });
    }
    let meta = object.meta();
    let implementation_version_name = meta
        .implementation_version_name
        .clone()
        .map(|value| value.trim().to_string())
        .unwrap_or_else(|| "UNKNOWN".to_string());
    let response_backend = &generated.response["backend"];
    let warnings = generated.response["warnings"].clone();
    let ct_series =
        ct_source
            .series_instance_uid
            .as_deref()
            .ok_or(GenerateError::MetadataShape {
                path: generated.output_path.clone(),
                message: "TID 1500 CT source series UID is missing",
            })?;
    let seg_series =
        seg_source
            .series_instance_uid
            .as_deref()
            .ok_or(GenerateError::MetadataShape {
                path: generated.output_path.clone(),
                message: "TID 1500 SEG source series UID is missing",
            })?;
    let source_frame_numbers = [1, 2];
    let validated = validate_tid1500_file(
        &generated.output_path,
        &Tid1500Expectations {
            sop_class_uid: TID1500_SOP_CLASS_UID,
            sop_instance_uid: &generated.identities.sop_instance_uid,
            transfer_syntax_uid: PARAMETRIC_MAP_TRANSFER_SYNTAX_UID,
            implementation_class_uid: meta.implementation_class_uid(),
            synthetic_data: "YES",
            modality: "SR",
            completion_flag: "COMPLETE",
            verification_flag: "UNVERIFIED",
            preliminary_flag: "FINAL",
            referenced_study_instance_uid: &generated.identities.study_instance_uid,
            observer_uid: &generated.identities.observer_uid,
            tracking_identifier: "DTS-TID1500-ROI-1",
            tracking_uid: &generated.identities.tracking_uid,
            source_series_instance_uid: ct_series,
            source_sop_class_uid: &ct_source.sop_class_uid,
            source_sop_instance_uid: &ct_source.sop_instance_uid,
            source_frame_numbers: &source_frame_numbers,
            segmentation_series_instance_uid: seg_series,
            segmentation_sop_class_uid: &seg_source.sop_class_uid,
            segmentation_sop_instance_uid: &seg_source.sop_instance_uid,
            referenced_segment_number: 1,
        },
    )?;
    let mut validation = validated.validation;
    validation["internal"]
        .as_array_mut()
        .expect("TID 1500 validation internal results are an array")
        .push(serde_json::json!({
            "name": "external_backend_contract",
            "status": "passed",
            "message": "The locked backend response and provenance satisfied protocol 0.1.0."
        }));
    let expected_tid1500 = serde_json::json!({
        "completion_flag": "COMPLETE",
        "preliminary_flag": "FINAL",
        "verification_flag": "UNVERIFIED",
        "root_template": {"mapping_resource": "DCMR", "template_identifier": "1500"},
        "document_title": tid1500_code("126000", "DCM", "Imaging Measurement Report"),
        "observation_context": {
            "observer_type": "DEVICE",
            "device_observer_uid": generated.identities.observer_uid
        },
        "procedure_reported": tid1500_code("25045-6", "LN", "CT unspecified body region"),
        "imaging_measurements": tid1500_code("126010", "DCM", "Imaging Measurements"),
        "measurement_group": {
            "container": tid1500_code("125007", "DCM", "Measurement Group"),
            "tracking_identifier": "DTS-TID1500-ROI-1",
            "tracking_uid": generated.identities.tracking_uid,
            "finding": tid1500_code("123037004", "SCT", "Body structure"),
            "referenced_segment": {
                "source_case_id": seg_source.source_case_id,
                "sop_class_uid": seg_source.sop_class_uid,
                "sop_instance_uid": seg_source.sop_instance_uid,
                "series_instance_uid": seg_series,
                "segment_number": 1,
                "referenced_frame_numbers": Value::Null,
                "source_image": {
                    "source_case_id": ct_source.source_case_id,
                    "sop_class_uid": ct_source.sop_class_uid,
                    "sop_instance_uid": ct_source.sop_instance_uid,
                    "series_instance_uid": ct_series,
                    "referenced_frame_numbers": [1, 2]
                }
            },
            "measurement": {
                "name": tid1500_code("118565006", "SCT", "Volume"),
                "numeric_value": "5.625",
                "units": tid1500_code("mm3", "UCUM", "cubic millimeter")
            }
        },
        "evidence": [
            {
                "role": "source_image",
                "source_case_id": ct_source.source_case_id,
                "sop_class_uid": ct_source.sop_class_uid,
                "sop_instance_uid": ct_source.sop_instance_uid,
                "series_instance_uid": ct_series
            },
            {
                "role": "referenced_segmentation",
                "source_case_id": seg_source.source_case_id,
                "sop_class_uid": seg_source.sop_class_uid,
                "sop_instance_uid": seg_source.sop_instance_uid,
                "series_instance_uid": seg_series
            }
        ]
    });
    let references = vec![
        ct_source.to_manifest_reference("source_image_for_segmentation", Some(vec![1, 2])),
        seg_source.to_manifest_reference("referenced_segment", None),
    ];
    Ok(GeneratedFile {
        case_id: TID1500_CASE_ID.to_string(),
        manifest_entry: serde_json::json!({
            "case_id": TID1500_CASE_ID,
            "profile_membership": ["extended"],
            "path": format!("{TID1500_CASE_ID}/{TID1500_OUTPUT_FILE}"),
            "sha256": sha256_hex(&generated.output_bytes),
            "size_bytes": generated.output_bytes.len(),
            "determinism": "semantic_stable",
            "recipe": {
                "recipe_id": TID1500_RECIPE_ID,
                "recipe_version": TID1500_RECIPE_VERSION,
                "recipe_parameters": {
                    "segment_number": 1,
                    "measurement_value_mm3": 5.625,
                    "tracking_identifier": "DTS-TID1500-ROI-1",
                    "source_frame_numbers": [1, 2]
                }
            },
            "dicom": {
                "sop_class_uid": TID1500_SOP_CLASS_UID,
                "sop_class_name": "Comprehensive 3D SR Storage",
                "iod_name": "Comprehensive 3D SR",
                "modality": "SR",
                "transfer_syntax_uid": PARAMETRIC_MAP_TRANSFER_SYNTAX_UID,
                "transfer_syntax_name": "Explicit VR Little Endian"
            },
            "uids": {
                "study_instance_uid": generated.identities.study_instance_uid,
                "series_instance_uid": generated.identities.series_instance_uid,
                "sop_instance_uid": generated.identities.sop_instance_uid,
                "frame_of_reference_uid": generated.identities.frame_of_reference_uid,
                "implementation_class_uid": meta.implementation_class_uid,
                "implementation_version_name": implementation_version_name
            },
            "image": Value::Null,
            "pixel_data": Value::Null,
            "generation_backend": {
                "backend_id": generated.backend.backend_id,
                "protocol_version": crate::generation_backends::PROTOCOL_VERSION,
                "name": response_backend["name"],
                "version": response_backend["version"],
                "dependency_lock_sha256": generated.backend.dependency_lock_sha256,
                "executable_fingerprint": generated.backend.executable_fingerprint,
                "entrypoint_fingerprint": generated.backend.entrypoint_fingerprint,
                "environment_fingerprint": generated.backend.environment_fingerprint,
                "runtime_identity": generated.backend.runtime_identity,
                "determinism": "semantic_stable",
                "warnings": warnings
            },
            "references": references,
            "expected_capabilities": [
                "open_file", "read_metadata", "parse_structured_report",
                "resolve_references", "interpret_tid1500_measurements"
            ],
            "expected_semantics": {
                "synthetic_data": "YES",
                "source_sop_instance_uid": ct_source.sop_instance_uid,
                "structured_report": {
                    "completion_flag": "COMPLETE",
                    "preliminary_flag": "FINAL",
                    "verification_flag": "UNVERIFIED",
                    "root_value_type": "CONTAINER",
                    "root_continuity_of_content": "CONTINUOUS",
                    "content_sequence_items": 8
                }
            },
            "expected_tid1500": expected_tid1500,
            "expected_visual_checks": {"pattern": "tid1500_volume_measurement_from_binary_segmentation"},
            "validation": validation,
            "known_stressors": [
                "comprehensive_3d_sr_storage", "tid1500_measurement_report",
                "tid1411_measurement_group", "referenced_segment",
                "cross_instance_references", "external_generation_backend"
            ],
            "standards_evidence": deduplicated_standards_evidence(standards_evidence_from_case(case))
        }),
    })
}

enum Scoord3dCaseOutcome {
    Generated(GeneratedFile),
    Unavailable(Value),
}

fn write_scoord3d_case(
    run: &PreparedGenerationRun,
    case: &Value,
    ct_source: &GeneratedSourceObject,
    standards_lock_sha256: &str,
) -> Result<Scoord3dCaseOutcome, GenerateError> {
    let ct_series_instance_uid =
        ct_source
            .series_instance_uid
            .as_deref()
            .ok_or(GenerateError::MetadataShape {
                path: PathBuf::from(SCOORD3D_CASE_ID),
                message: "SCOORD3D Enhanced CT source must record a Series Instance UID",
            })?;
    let frame_of_reference_uid =
        ct_source
            .frame_of_reference_uid
            .as_deref()
            .ok_or(GenerateError::MetadataShape {
                path: PathBuf::from(SCOORD3D_CASE_ID),
                message: "SCOORD3D Enhanced CT source must record a Frame of Reference UID",
            })?;
    if ct_source.frame_count != Some(2) {
        return Err(GenerateError::MetadataShape {
            path: PathBuf::from(SCOORD3D_CASE_ID),
            message: "SCOORD3D Enhanced CT source must contain exactly two frames",
        });
    }

    let uid = |role, referenced_object_index| {
        deterministic_uid(&DeterministicUidInput {
            standards_lock_sha256,
            case_id: SCOORD3D_CASE_ID,
            recipe_version: SCOORD3D_RECIPE_VERSION,
            run_seed: run.seed,
            file_index: 0,
            frame_index: None,
            referenced_object_index,
            role,
        })
    };
    let identities = Scoord3dIdentities {
        study_instance_uid: ct_source.study_instance_uid.clone(),
        series_instance_uid: uid(UidRole::SeriesInstance, None),
        frame_of_reference_uid: frame_of_reference_uid.to_string(),
        sop_instance_uid: uid(UidRole::SopInstance, None),
        tracking_uid: uid(UidRole::DerivedReference, Some(0)),
        observer_uid: uid(UidRole::DerivedReference, Some(1)),
        fiducial_uid: uid(UidRole::DerivedReference, Some(2)),
    };
    let standards_lock_path = PathBuf::from("standards.lock.json");
    let standards_lock_bytes =
        fs::read(&standards_lock_path).map_err(|source| GenerateError::ReadMetadata {
            path: standards_lock_path.clone(),
            source,
        })?;
    let standards_lock: Value =
        serde_json::from_slice(&standards_lock_bytes).map_err(|source| {
            GenerateError::ParseMetadata {
                path: standards_lock_path,
                source,
            }
        })?;
    let staging = ParametricMapStagingGuard::new();
    let input = Scoord3dGenerationInput {
        repository_root: PathBuf::from("."),
        generated_root: run.out_dir.clone(),
        staging_root: staging.path().to_path_buf(),
        destination_root: run.out_dir.join(SCOORD3D_CASE_ID),
        seed: run.seed,
        standards: StandardsProvenance {
            standards_lock_sha256: standards_lock_sha256.to_string(),
            dicom_base_edition: standards_lock["dicom_base_edition"]
                .as_str()
                .ok_or(GenerateError::MetadataShape {
                    path: PathBuf::from("standards.lock.json"),
                    message: "standards lock dicom_base_edition must be a string",
                })?
                .to_string(),
            kb_source_manifest_sha256: standards_lock
                .pointer("/dicom_standard_kb/source_manifest_sha256")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
        },
        controlled_metadata: ControlledMetadata {
            patient_name: "DTS^Synthetic^Patient001".to_string(),
            patient_id: "DTS-PATIENT-001".to_string(),
            manufacturer: "dicom-test-suite".to_string(),
            model_name: SCOORD3D_RECIPE_ID.to_string(),
            software_versions: env!("CARGO_PKG_VERSION").to_string(),
            study_date: "20260101".to_string(),
            study_time: "000000".to_string(),
            content_date: "20260101".to_string(),
            content_time: "000000".to_string(),
            timezone_offset_from_utc: "+0000".to_string(),
        },
        identities,
        sources: vec![ParametricMapSource {
            role: "source_image".to_string(),
            source_case_id: ct_source.source_case_id.clone(),
            relative_path: ct_source.source_path.clone(),
            sha256: ct_source.sha256.clone(),
            sop_class_uid: ct_source.sop_class_uid.clone(),
            sop_instance_uid: ct_source.sop_instance_uid.clone(),
            series_instance_uid: Some(ct_series_instance_uid.to_string()),
            frame_numbers: Some(vec![1, 2]),
        }],
    };
    match generate_scoord3d(&input).map_err(|error| GenerateError::WriteDicomFile {
        path: PathBuf::from(SCOORD3D_CASE_ID),
        message: error.to_string(),
    })? {
        Scoord3dOutcome::Unavailable { code, message } => {
            Ok(Scoord3dCaseOutcome::Unavailable(serde_json::json!({
                "case_id": SCOORD3D_CASE_ID,
                "status": "unavailable",
                "reason_code": "external_backend_unavailable",
                "message": format!("{code}: {message}"),
                "recheck_phase": "phase-3",
                "standards_evidence": standards_evidence_from_case(case)
            })))
        }
        Scoord3dOutcome::Generated(generated) => Ok(Scoord3dCaseOutcome::Generated(
            scoord3d_generated_file(case, ct_source, generated)?,
        )),
    }
}

fn scoord3d_generated_file(
    case: &Value,
    ct_source: &GeneratedSourceObject,
    generated: Scoord3dGenerated,
) -> Result<GeneratedFile, GenerateError> {
    let object =
        open_file(&generated.output_path).map_err(|error| GenerateError::ValidateDicomFile {
            path: generated.output_path.clone(),
            message: format!("reopen promoted SCOORD3D report: {error}"),
        })?;
    let meta = object.meta();
    let implementation_version_name = meta
        .implementation_version_name
        .clone()
        .map(|value| value.trim().to_string())
        .unwrap_or_else(|| "UNKNOWN".to_string());
    let ct_series =
        ct_source
            .series_instance_uid
            .as_deref()
            .ok_or(GenerateError::MetadataShape {
                path: generated.output_path.clone(),
                message: "SCOORD3D CT source series UID is missing",
            })?;
    let source_frame_numbers = [1, 2];
    let validated = validate_scoord3d_file(
        &generated.output_path,
        &Scoord3dExpectations {
            sop_class_uid: SCOORD3D_SOP_CLASS_UID,
            sop_instance_uid: &generated.identities.sop_instance_uid,
            transfer_syntax_uid: PARAMETRIC_MAP_TRANSFER_SYNTAX_UID,
            implementation_class_uid: meta.implementation_class_uid(),
            synthetic_data: "YES",
            modality: "SR",
            completion_flag: "COMPLETE",
            verification_flag: "UNVERIFIED",
            preliminary_flag: "FINAL",
            referenced_study_instance_uid: &generated.identities.study_instance_uid,
            observer_uid: &generated.identities.observer_uid,
            tracking_identifier: "DTS-SCOORD3D-ROI-1",
            tracking_uid: &generated.identities.tracking_uid,
            frame_of_reference_uid: &generated.identities.frame_of_reference_uid,
            fiducial_uid: &generated.identities.fiducial_uid,
            source_series_instance_uid: ct_series,
            source_sop_class_uid: &ct_source.sop_class_uid,
            source_sop_instance_uid: &ct_source.sop_instance_uid,
            source_frame_numbers: &source_frame_numbers,
        },
    )?;
    let mut validation = validated.validation;
    validation["internal"]
        .as_array_mut()
        .expect("SCOORD3D validation internal results are an array")
        .push(serde_json::json!({
            "name": "external_backend_contract",
            "status": "passed",
            "message": "The locked backend response and provenance satisfied protocol 0.1.0."
        }));
    let code = |value: &str, scheme: &str, meaning: &str| {
        serde_json::json!({
            "code_value": value,
            "coding_scheme_designator": scheme,
            "code_meaning": meaning
        })
    };
    let expected_scoord3d = serde_json::json!({
        "completion_flag": "COMPLETE",
        "preliminary_flag": "FINAL",
        "verification_flag": "UNVERIFIED",
        "root_template": {"mapping_resource": "DCMR", "template_identifier": "1500"},
        "document_title": code("126000", "DCM", "Imaging Measurement Report"),
        "observation_context": {
            "observer_type": "DEVICE",
            "device_observer_uid": generated.identities.observer_uid
        },
        "procedure_reported": code("25045-6", "LN", "CT unspecified body region"),
        "imaging_measurements": code("126010", "DCM", "Imaging Measurements"),
        "measurement_group": {
            "template": {"mapping_resource": "DCMR", "template_identifier": "1501"},
            "container": code("125007", "DCM", "Measurement Group"),
            "tracking_identifier": "DTS-SCOORD3D-ROI-1",
            "tracking_uid": generated.identities.tracking_uid,
            "finding": code("123037004", "SCT", "Body structure"),
            "measurement": {
                "name": code("121206", "DCM", "Distance"),
                "numeric_value": "2.5",
                "units": code("mm", "UCUM", "millimeter"),
                "spatial_coordinates": {
                    "relationship": "INFERRED FROM",
                    "value_type": "SCOORD3D",
                    "concept_name": code("260753009", "SCT", "Source"),
                    "graphic_type": "POLYLINE",
                    "graphic_data_mm": [0.0, 0.0, 0.0, 0.0, 0.0, 2.5],
                    "frame_of_reference_uid": generated.identities.frame_of_reference_uid,
                    "fiducial_uid": generated.identities.fiducial_uid
                }
            },
            "source_image": {
                "relationship": "CONTAINS",
                "value_type": "IMAGE",
                "concept_name": code("121112", "DCM", "Source of Measurement"),
                "source_case_id": ct_source.source_case_id,
                "sop_class_uid": ct_source.sop_class_uid,
                "sop_instance_uid": ct_source.sop_instance_uid,
                "series_instance_uid": ct_series,
                "referenced_frame_numbers": [1, 2]
            }
        },
        "image_library_present": false,
        "evidence": [{
            "role": "source_image",
            "source_case_id": ct_source.source_case_id,
            "sop_class_uid": ct_source.sop_class_uid,
            "sop_instance_uid": ct_source.sop_instance_uid,
            "series_instance_uid": ct_series
        }]
    });
    let response_backend = &generated.response["backend"];
    let warnings = generated.response["warnings"].clone();
    Ok(GeneratedFile {
        case_id: SCOORD3D_CASE_ID.to_string(),
        manifest_entry: serde_json::json!({
            "case_id": SCOORD3D_CASE_ID,
            "profile_membership": ["extended"],
            "path": format!("{SCOORD3D_CASE_ID}/{SCOORD3D_OUTPUT_FILE}"),
            "sha256": sha256_hex(&generated.output_bytes),
            "size_bytes": generated.output_bytes.len(),
            "determinism": "semantic_stable",
            "recipe": {
                "recipe_id": SCOORD3D_RECIPE_ID,
                "recipe_version": SCOORD3D_RECIPE_VERSION,
                "recipe_parameters": {
                    "tracking_identifier": "DTS-SCOORD3D-ROI-1",
                    "source_frame_numbers": [1, 2],
                    "graphic_type": "POLYLINE",
                    "graphic_data_patient_mm": [[0.0, 0.0, 0.0], [0.0, 0.0, 2.5]],
                    "measurement_value_mm": 2.5
                }
            },
            "dicom": {
                "sop_class_uid": SCOORD3D_SOP_CLASS_UID,
                "sop_class_name": "Comprehensive 3D SR Storage",
                "iod_name": "Comprehensive 3D SR",
                "modality": "SR",
                "transfer_syntax_uid": PARAMETRIC_MAP_TRANSFER_SYNTAX_UID,
                "transfer_syntax_name": "Explicit VR Little Endian"
            },
            "uids": {
                "study_instance_uid": generated.identities.study_instance_uid,
                "series_instance_uid": generated.identities.series_instance_uid,
                "sop_instance_uid": generated.identities.sop_instance_uid,
                "frame_of_reference_uid": generated.identities.frame_of_reference_uid,
                "implementation_class_uid": meta.implementation_class_uid,
                "implementation_version_name": implementation_version_name
            },
            "image": Value::Null,
            "pixel_data": Value::Null,
            "generation_backend": {
                "backend_id": generated.backend.backend_id,
                "protocol_version": crate::generation_backends::PROTOCOL_VERSION,
                "name": response_backend["name"],
                "version": response_backend["version"],
                "dependency_lock_sha256": generated.backend.dependency_lock_sha256,
                "executable_fingerprint": generated.backend.executable_fingerprint,
                "entrypoint_fingerprint": generated.backend.entrypoint_fingerprint,
                "environment_fingerprint": generated.backend.environment_fingerprint,
                "runtime_identity": generated.backend.runtime_identity,
                "determinism": "semantic_stable",
                "warnings": warnings
            },
            "references": [ct_source.to_manifest_reference("source_of_measurement", Some(vec![1, 2]))],
            "expected_capabilities": [
                "parse_structured_report", "parse_scoord3d", "resolve_references",
                "render_spatial_annotation"
            ],
            "expected_semantics": {
                "synthetic_data": "YES",
                "source_sop_instance_uid": ct_source.sop_instance_uid,
                "structured_report": {
                    "completion_flag": "COMPLETE",
                    "preliminary_flag": "FINAL",
                    "verification_flag": "UNVERIFIED",
                    "root_value_type": "CONTAINER",
                    "root_continuity_of_content": "CONTINUOUS",
                    "content_sequence_items": 8
                },
                "scoord3d": {
                    "graphic_type": "POLYLINE",
                    "graphic_data_patient_mm": [[0.0, 0.0, 0.0], [0.0, 0.0, 2.5]],
                    "frame_of_reference_uid": generated.identities.frame_of_reference_uid,
                    "fiducial_uid": generated.identities.fiducial_uid
                }
            },
            "expected_scoord3d": expected_scoord3d,
            "expected_visual_checks": {"pattern": "scoord3d_polyline_between_enhanced_ct_frames"},
            "validation": validation,
            "known_stressors": [
                "comprehensive_3d_sr_storage", "tid1500_measurement_report",
                "tid1501_measurement_group", "scoord3d_patient_coordinates",
                "frame_of_reference_geometry", "cross_instance_references",
                "external_generation_backend"
            ],
            "standards_evidence": deduplicated_standards_evidence(standards_evidence_from_case(case))
        }),
    })
}

fn write_spatial_registration_case(
    run: &PreparedGenerationRun,
    case: &Value,
    target: &GeneratedSourceObject,
    source: &GeneratedSourceObject,
    standards_lock_sha256: &str,
) -> Result<GeneratedFile, GenerateError> {
    validate_spatial_registration_sources(&run.out_dir, target, source)?;
    let target_series = required_source_uid(
        target.series_instance_uid.as_deref(),
        SPATIAL_REGISTRATION_CASE_ID,
        "Spatial Registration target Series Instance UID is missing",
    )?;
    let target_for = required_source_uid(
        target.frame_of_reference_uid.as_deref(),
        SPATIAL_REGISTRATION_CASE_ID,
        "Spatial Registration target Frame of Reference UID is missing",
    )?;
    let source_series = required_source_uid(
        source.series_instance_uid.as_deref(),
        SPATIAL_REGISTRATION_CASE_ID,
        "Spatial Registration source Series Instance UID is missing",
    )?;
    let source_for = required_source_uid(
        source.frame_of_reference_uid.as_deref(),
        SPATIAL_REGISTRATION_CASE_ID,
        "Spatial Registration source Frame of Reference UID is missing",
    )?;
    let uid = |role| {
        deterministic_uid(&DeterministicUidInput {
            standards_lock_sha256,
            case_id: SPATIAL_REGISTRATION_CASE_ID,
            recipe_version: SPATIAL_REGISTRATION_RECIPE_VERSION,
            run_seed: run.seed,
            file_index: 0,
            frame_index: None,
            referenced_object_index: None,
            role,
        })
    };
    let series_instance_uid = uid(UidRole::SeriesInstance);
    let sop_instance_uid = uid(UidRole::SopInstance);
    let implementation_class_uid = deterministic_implementation_uid(standards_lock_sha256);
    let relative_path =
        format!("{SPATIAL_REGISTRATION_CASE_ID}/{SPATIAL_REGISTRATION_OUTPUT_FILE}");
    let path = run.out_dir.join(&relative_path);
    let case_dir = path.parent().ok_or_else(|| GenerateError::MetadataShape {
        path: PathBuf::from(&relative_path),
        message: "Spatial Registration output must have a parent directory",
    })?;
    fs::create_dir_all(case_dir).map_err(|source| GenerateError::CreateCaseOutputDir {
        path: case_dir.to_path_buf(),
        source,
    })?;

    let object = build_spatial_registration(SpatialRegistrationInput {
        sop_instance_uid: &sop_instance_uid,
        series_instance_uid: &series_instance_uid,
        target: SpatialRegistrationReference {
            study_instance_uid: &target.study_instance_uid,
            series_instance_uid: target_series,
            sop_class_uid: &target.sop_class_uid,
            sop_instance_uid: &target.sop_instance_uid,
            frame_of_reference_uid: target_for,
        },
        source: SpatialRegistrationReference {
            study_instance_uid: &source.study_instance_uid,
            series_instance_uid: source_series,
            sop_class_uid: &source.sop_class_uid,
            sop_instance_uid: &source.sop_instance_uid,
            frame_of_reference_uid: source_for,
        },
    })
    .map_err(|message| GenerateError::WriteDicomFile {
        path: path.clone(),
        message,
    })?;
    object
        .with_meta(
            FileMetaTableBuilder::new()
                .transfer_syntax(EXPLICIT_VR_LITTLE_ENDIAN.uid)
                .implementation_class_uid(&implementation_class_uid)
                .implementation_version_name(crate::IMPLEMENTATION_VERSION_NAME),
        )
        .map_err(|error| GenerateError::WriteDicomFile {
            path: path.clone(),
            message: error.to_string(),
        })?
        .write_to_file(&path)
        .map_err(|error| GenerateError::WriteDicomFile {
            path: path.clone(),
            message: error.to_string(),
        })?;
    let target_matrix = TARGET_IDENTITY_MATRIX.map(|value| {
        value
            .parse::<f64>()
            .expect("locked target matrix values are numeric")
    });
    let source_matrix = SOURCE_TO_TARGET_MATRIX.map(|value| {
        value
            .parse::<f64>()
            .expect("locked source matrix values are numeric")
    });
    let validated = validate_spatial_registration_file(
        &path,
        &SpatialRegistrationExpectations {
            sop_class_uid: SPATIAL_REGISTRATION_STORAGE_UID,
            sop_instance_uid: &sop_instance_uid,
            transfer_syntax_uid: EXPLICIT_VR_LITTLE_ENDIAN.uid,
            implementation_class_uid: &implementation_class_uid,
            synthetic_data: "YES",
            patient_id: "DTS-PATIENT-001",
            study_instance_uid: &target.study_instance_uid,
            study_id: "DTS-ECT",
            series_instance_uid: &series_instance_uid,
            series_number: "8003",
            laterality: "R",
            modality: "REG",
            instance_number: "1",
            content_date: "20260101",
            content_time: "000000",
            content_label: "DTS_RIGID_REG",
            content_description: "Rigid CT pair registration",
            content_creator_name: "DTS^Generator",
            manufacturer: "dicom-test-suite",
            manufacturer_model_name: "Native Spatial Registration",
            device_serial_number: "DTS-REG-001",
            software_versions: crate::PACKAGE_VERSION,
            registered_frame_of_reference_uid: target_for,
            target: SpatialRegistrationReferenceExpectations {
                study_instance_uid: &target.study_instance_uid,
                series_instance_uid: target_series,
                sop_class_uid: &target.sop_class_uid,
                sop_instance_uid: &target.sop_instance_uid,
                frame_of_reference_uid: target_for,
            },
            source: SpatialRegistrationReferenceExpectations {
                study_instance_uid: &source.study_instance_uid,
                series_instance_uid: source_series,
                sop_class_uid: &source.sop_class_uid,
                sop_instance_uid: &source.sop_instance_uid,
                frame_of_reference_uid: source_for,
            },
            target_matrix,
            source_to_registered_matrix: source_matrix,
            source_landmark_mm: [-0.625, -0.625, 0.0],
            registered_landmark_mm: [0.0, 0.0, 2.5],
            rigid_tolerance: 0.000001,
        },
    )?;
    let mut validation = validated.validation;
    validation["internal"]
        .as_array_mut()
        .expect("Spatial Registration validation internal results are an array")
        .push(serde_json::json!({
            "name": "spatial_registration_source_geometry",
            "status": "passed",
            "message": "Rust reopened both CT sources and verified identities, hashes, Frames of Reference, and locked geometry before construction."
        }));
    let bytes = validated.bytes;
    let source_identity = |object: &GeneratedSourceObject| {
        serde_json::json!({
            "source_case_id": object.source_case_id,
            "source_path": object.source_path,
            "source_sha256": object.sha256,
            "study_instance_uid": object.study_instance_uid,
            "series_instance_uid": object.series_instance_uid,
            "sop_class_uid": object.sop_class_uid,
            "sop_instance_uid": object.sop_instance_uid,
            "frame_of_reference_uid": object.frame_of_reference_uid
        })
    };
    let target_identity = source_identity(target);
    let source_identity = source_identity(source);
    let expected_spatial_registration = serde_json::json!({
        "registered_frame_of_reference_uid": target_for,
        "matrix_direction": "source_to_registered",
        "registration_items": [
            {
                "role": "registered_target",
                "source": target_identity,
                "complete_instance": true,
                "matrix_registration_items": 1,
                "registration_type_code_items": 0,
                "matrix_items": 1,
                "matrix": {"type": "RIGID", "values": target_matrix}
            },
            {
                "role": "moving_source",
                "source": source_identity,
                "complete_instance": true,
                "matrix_registration_items": 1,
                "registration_type_code_items": 0,
                "matrix_items": 1,
                "matrix": {"type": "RIGID", "values": source_matrix}
            }
        ],
        "rigid_tolerances": {
            "orthonormal_abs": 0.000001,
            "determinant_abs": 0.000001,
            "homogeneous_abs": 0.000001
        },
        "landmark": {
            "source_point_mm": [-0.625, -0.625, 0.0],
            "registered_point_mm": [0.0, 0.0, 2.5],
            "tolerance_mm": 0.000001
        },
        "common_instance_reference": {
            "same_study": target_identity,
            "other_studies": [source_identity]
        },
        "pixel_data_absent": true
    });
    Ok(GeneratedFile {
        case_id: SPATIAL_REGISTRATION_CASE_ID.to_string(),
        manifest_entry: serde_json::json!({
            "case_id": SPATIAL_REGISTRATION_CASE_ID,
            "profile_membership": ["extended"],
            "path": relative_path,
            "sha256": sha256_hex(&bytes),
            "size_bytes": bytes.len(),
            "determinism": "byte_stable",
            "recipe": {
                "recipe_id": SPATIAL_REGISTRATION_RECIPE_ID,
                "recipe_version": SPATIAL_REGISTRATION_RECIPE_VERSION,
                "recipe_parameters": {
                    "matrix_direction": "source_to_registered",
                    "target_identity_matrix": target_matrix,
                    "source_to_registered_matrix": source_matrix,
                    "landmark_source_mm": [-0.625, -0.625, 0.0],
                    "landmark_registered_mm": [0.0, 0.0, 2.5]
                }
            },
            "dicom": {
                "sop_class_uid": SPATIAL_REGISTRATION_STORAGE_UID,
                "sop_class_name": "Spatial Registration Storage",
                "iod_name": "Spatial Registration",
                "modality": "REG",
                "transfer_syntax_uid": EXPLICIT_VR_LITTLE_ENDIAN.uid,
                "transfer_syntax_name": EXPLICIT_VR_LITTLE_ENDIAN.name
            },
            "uids": {
                "study_instance_uid": target.study_instance_uid,
                "series_instance_uid": series_instance_uid,
                "sop_instance_uid": sop_instance_uid,
                "frame_of_reference_uid": target_for,
                "implementation_class_uid": implementation_class_uid,
                "implementation_version_name": crate::IMPLEMENTATION_VERSION_NAME
            },
            "image": Value::Null,
            "pixel_data": Value::Null,
            "references": [
                target.to_manifest_reference("registered_target", None),
                source.to_manifest_reference("moving_source", None)
            ],
            "expected_capabilities": [
                "open_file", "read_metadata", "resolve_references",
                "read_spatial_registration", "apply_rigid_transform",
                "fuse_registered_images"
            ],
            "expected_semantics": {
                "synthetic_data": "YES",
                "registered_frame_of_reference_uid": target_for,
                "matrix_direction": "source_to_registered",
                "pixel_data_absent": true
            },
            "expected_spatial_registration": expected_spatial_registration,
            "expected_visual_checks": {
                "pattern": "moving_ct_origin_maps_to_enhanced_ct_frame_2_origin"
            },
            "validation": validation,
            "known_stressors": [
                "spatial_registration_storage", "two_frames_of_reference",
                "identity_and_nonidentity_rigid_matrices", "matrix_directionality",
                "cross_study_references", "landmark_transform"
            ],
            "standards_evidence": deduplicated_standards_evidence(standards_evidence_from_case(case))
        }),
    })
}

fn write_deformable_spatial_registration_case(
    run: &PreparedGenerationRun,
    case: &Value,
    target: &GeneratedSourceObject,
    source: &GeneratedSourceObject,
    standards_lock_sha256: &str,
) -> Result<GeneratedFile, GenerateError> {
    validate_spatial_registration_sources(&run.out_dir, target, source)?;
    let target_series = required_source_uid(
        target.series_instance_uid.as_deref(),
        DEFORMABLE_SPATIAL_REGISTRATION_CASE_ID,
        "Deformable Registration target Series Instance UID is missing",
    )?;
    let target_for = required_source_uid(
        target.frame_of_reference_uid.as_deref(),
        DEFORMABLE_SPATIAL_REGISTRATION_CASE_ID,
        "Deformable Registration target Frame of Reference UID is missing",
    )?;
    let source_series = required_source_uid(
        source.series_instance_uid.as_deref(),
        DEFORMABLE_SPATIAL_REGISTRATION_CASE_ID,
        "Deformable Registration source Series Instance UID is missing",
    )?;
    let source_for = required_source_uid(
        source.frame_of_reference_uid.as_deref(),
        DEFORMABLE_SPATIAL_REGISTRATION_CASE_ID,
        "Deformable Registration source Frame of Reference UID is missing",
    )?;
    let uid = |role| {
        deterministic_uid(&DeterministicUidInput {
            standards_lock_sha256,
            case_id: DEFORMABLE_SPATIAL_REGISTRATION_CASE_ID,
            recipe_version: DEFORMABLE_SPATIAL_REGISTRATION_RECIPE_VERSION,
            run_seed: run.seed,
            file_index: 0,
            frame_index: None,
            referenced_object_index: None,
            role,
        })
    };
    let series_instance_uid = uid(UidRole::SeriesInstance);
    let sop_instance_uid = uid(UidRole::SopInstance);
    let implementation_class_uid = deterministic_implementation_uid(standards_lock_sha256);
    let relative_path = format!(
        "{DEFORMABLE_SPATIAL_REGISTRATION_CASE_ID}/{DEFORMABLE_SPATIAL_REGISTRATION_OUTPUT_FILE}"
    );
    let path = run.out_dir.join(&relative_path);
    let case_dir = path.parent().ok_or_else(|| GenerateError::MetadataShape {
        path: PathBuf::from(&relative_path),
        message: "Deformable Spatial Registration output must have a parent directory",
    })?;
    fs::create_dir_all(case_dir).map_err(|source| GenerateError::CreateCaseOutputDir {
        path: case_dir.to_path_buf(),
        source,
    })?;

    let object = build_deformable_spatial_registration(DeformableSpatialRegistrationInput {
        sop_instance_uid: &sop_instance_uid,
        series_instance_uid: &series_instance_uid,
        target: DeformableRegistrationReference {
            study_instance_uid: &target.study_instance_uid,
            series_instance_uid: target_series,
            sop_class_uid: &target.sop_class_uid,
            sop_instance_uid: &target.sop_instance_uid,
            frame_of_reference_uid: target_for,
        },
        source: DeformableRegistrationReference {
            study_instance_uid: &source.study_instance_uid,
            series_instance_uid: source_series,
            sop_class_uid: &source.sop_class_uid,
            sop_instance_uid: &source.sop_instance_uid,
            frame_of_reference_uid: source_for,
        },
    })
    .map_err(|message| GenerateError::WriteDicomFile {
        path: path.clone(),
        message,
    })?;
    object
        .with_meta(
            FileMetaTableBuilder::new()
                .transfer_syntax(EXPLICIT_VR_LITTLE_ENDIAN.uid)
                .implementation_class_uid(&implementation_class_uid)
                .implementation_version_name(crate::IMPLEMENTATION_VERSION_NAME),
        )
        .map_err(|error| GenerateError::WriteDicomFile {
            path: path.clone(),
            message: error.to_string(),
        })?
        .write_to_file(&path)
        .map_err(|error| GenerateError::WriteDicomFile {
            path: path.clone(),
            message: error.to_string(),
        })?;

    let identity_matrix = DEFORMABLE_IDENTITY_MATRIX.map(|value| {
        value
            .parse::<f64>()
            .expect("locked deformable identity matrix values are numeric")
    });
    let decoded_vectors = DEFORMABLE_VECTOR_GRID_VALUES
        .chunks_exact(3)
        .map(|values| [values[0], values[1], values[2]])
        .collect::<Vec<_>>();
    let validated = validate_deformable_spatial_registration_file(
        &path,
        &DeformableSpatialRegistrationExpectations {
            sop_class_uid: DEFORMABLE_SPATIAL_REGISTRATION_STORAGE_UID,
            sop_instance_uid: &sop_instance_uid,
            transfer_syntax_uid: EXPLICIT_VR_LITTLE_ENDIAN.uid,
            implementation_class_uid: &implementation_class_uid,
            synthetic_data: "YES",
            patient_id: "DTS-PATIENT-001",
            study_instance_uid: &target.study_instance_uid,
            study_id: "DTS-ECT",
            series_instance_uid: &series_instance_uid,
            series_number: "8004",
            laterality: "R",
            modality: "REG",
            instance_number: "1",
            content_date: "20260101",
            content_time: "000000",
            content_label: "DTS_DEFORM_REG",
            content_description: "Deformable CT pair registration",
            content_creator_name: "DTS^Generator",
            manufacturer: "dicom-test-suite",
            manufacturer_model_name: "Native Deformable Registration",
            device_serial_number: "DTS-DEFREG-001",
            software_versions: crate::PACKAGE_VERSION,
            registered_frame_of_reference_uid: target_for,
            target: SpatialRegistrationReferenceExpectations {
                study_instance_uid: &target.study_instance_uid,
                series_instance_uid: target_series,
                sop_class_uid: &target.sop_class_uid,
                sop_instance_uid: &target.sop_instance_uid,
                frame_of_reference_uid: target_for,
            },
            source: SpatialRegistrationReferenceExpectations {
                study_instance_uid: &source.study_instance_uid,
                series_instance_uid: source_series,
                sop_class_uid: &source.sop_class_uid,
                sop_instance_uid: &source.sop_instance_uid,
                frame_of_reference_uid: source_for,
            },
            pre_matrix: identity_matrix,
            post_matrix: identity_matrix,
            image_orientation_patient: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
            image_position_patient: [0.0, 0.0, 2.5],
            grid_dimensions: DEFORMABLE_GRID_DIMENSIONS,
            grid_resolution: DEFORMABLE_GRID_RESOLUTION,
            vector_grid_data_sha256: DEFORMABLE_VECTOR_GRID_DATA_SHA256,
            decoded_vectors_mm: &decoded_vectors,
            registered_points_mm: &DEFORMABLE_REGISTERED_POINTS_MM,
            source_points_mm: &DEFORMABLE_SOURCE_POINTS_MM,
            tolerance: 0.000001,
        },
    )?;
    let mut validation = validated.validation;
    validation["internal"]
        .as_array_mut()
        .expect("Deformable Registration validation internal results are an array")
        .push(serde_json::json!({
            "name": "deformable_registration_source_geometry",
            "status": "passed",
            "message": "Rust reopened both CT sources and verified identities, hashes, Frames of Reference, and locked geometry before construction."
        }));
    let bytes = validated.bytes;
    let source_identity = |object: &GeneratedSourceObject| {
        serde_json::json!({
            "source_case_id": object.source_case_id,
            "source_path": object.source_path,
            "source_sha256": object.sha256,
            "study_instance_uid": object.study_instance_uid,
            "series_instance_uid": object.series_instance_uid,
            "sop_class_uid": object.sop_class_uid,
            "sop_instance_uid": object.sop_instance_uid,
            "frame_of_reference_uid": object.frame_of_reference_uid
        })
    };
    let target_identity = source_identity(target);
    let source_identity = source_identity(source);
    let point_mappings = DEFORMABLE_REGISTERED_POINTS_MM
        .iter()
        .zip(DEFORMABLE_SOURCE_POINTS_MM.iter())
        .map(|(registered, source)| {
            serde_json::json!({
                "registered_point_mm": registered,
                "source_point_mm": source,
                "tolerance_mm": 0.000001
            })
        })
        .collect::<Vec<_>>();
    let expected_deformable_spatial_registration = serde_json::json!({
        "registered_frame_of_reference_uid": target_for,
        "sampling_direction": "registered_to_source",
        "source": source_identity,
        "complete_instance": true,
        "deformable_registration_items": 1,
        "registration_type_code_items": 0,
        "pre_deformation_matrix": {
            "items": 1, "type": "RIGID", "values": identity_matrix
        },
        "post_deformation_matrix": {
            "items": 1, "type": "RIGID", "values": identity_matrix
        },
        "grid": {
            "items": 1,
            "image_position_patient_mm": [0.0, 0.0, 2.5],
            "image_orientation_patient": [1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
            "dimensions": DEFORMABLE_GRID_DIMENSIONS,
            "resolution_mm": DEFORMABLE_GRID_RESOLUTION,
            "vector_data_vr": "OF",
            "vector_data_vm": 1,
            "vector_count": decoded_vectors.len(),
            "component_count": DEFORMABLE_VECTOR_GRID_VALUES.len(),
            "byte_length": DEFORMABLE_VECTOR_GRID_BYTES.len(),
            "payload_sha256": DEFORMABLE_VECTOR_GRID_DATA_SHA256,
            "byte_order": "little_endian_ieee754_binary32",
            "index_order": "i_fastest_then_j_then_k",
            "vectors_mm": decoded_vectors
        },
        "point_mappings": point_mappings,
        "common_instance_reference": {
            "same_study": target_identity,
            "other_studies": [source_identity]
        },
        "pixel_data_absent": true
    });

    Ok(GeneratedFile {
        case_id: DEFORMABLE_SPATIAL_REGISTRATION_CASE_ID.to_string(),
        manifest_entry: serde_json::json!({
            "case_id": DEFORMABLE_SPATIAL_REGISTRATION_CASE_ID,
            "profile_membership": ["extended"],
            "path": relative_path,
            "sha256": sha256_hex(&bytes),
            "size_bytes": bytes.len(),
            "determinism": "byte_stable",
            "recipe": {
                "recipe_id": DEFORMABLE_SPATIAL_REGISTRATION_RECIPE_ID,
                "recipe_version": DEFORMABLE_SPATIAL_REGISTRATION_RECIPE_VERSION,
                "recipe_parameters": {
                    "sampling_direction": "registered_to_source",
                    "pre_deformation_matrix": identity_matrix,
                    "post_deformation_matrix": identity_matrix,
                    "grid_dimensions": DEFORMABLE_GRID_DIMENSIONS,
                    "grid_resolution_mm": DEFORMABLE_GRID_RESOLUTION,
                    "vector_grid_data_sha256": DEFORMABLE_VECTOR_GRID_DATA_SHA256,
                    "vector_index_order": "i_fastest_then_j_then_k"
                }
            },
            "dicom": {
                "sop_class_uid": DEFORMABLE_SPATIAL_REGISTRATION_STORAGE_UID,
                "sop_class_name": "Deformable Spatial Registration Storage",
                "iod_name": "Deformable Spatial Registration",
                "modality": "REG",
                "transfer_syntax_uid": EXPLICIT_VR_LITTLE_ENDIAN.uid,
                "transfer_syntax_name": EXPLICIT_VR_LITTLE_ENDIAN.name
            },
            "uids": {
                "study_instance_uid": target.study_instance_uid,
                "series_instance_uid": series_instance_uid,
                "sop_instance_uid": sop_instance_uid,
                "frame_of_reference_uid": target_for,
                "implementation_class_uid": implementation_class_uid,
                "implementation_version_name": crate::IMPLEMENTATION_VERSION_NAME
            },
            "image": Value::Null,
            "pixel_data": Value::Null,
            "references": [
                target.to_manifest_reference("registered_target", None),
                source.to_manifest_reference("deformation_source", None)
            ],
            "expected_capabilities": [
                "open_file", "read_metadata", "resolve_references",
                "read_deformable_spatial_registration", "apply_deformation_field",
                "resample_registered_image"
            ],
            "expected_semantics": {
                "synthetic_data": "YES",
                "registered_frame_of_reference_uid": target_for,
                "sampling_direction": "registered_to_source",
                "pixel_data_absent": true
            },
            "expected_deformable_spatial_registration": expected_deformable_spatial_registration,
            "expected_visual_checks": {
                "pattern": "enhanced_ct_frame_2_grid_maps_to_classic_ct_pixel_centers"
            },
            "validation": validation,
            "known_stressors": [
                "deformable_spatial_registration_storage", "two_frames_of_reference",
                "identity_pre_and_post_matrices", "registered_to_source_sampling",
                "nonuniform_vector_grid", "of_little_endian_binary32",
                "i_fastest_vector_order", "cross_study_references"
            ],
            "standards_evidence": deduplicated_standards_evidence(standards_evidence_from_case(case))
        }),
    })
}

fn write_color_softcopy_presentation_state_case(
    run: &PreparedGenerationRun,
    case: &Value,
    source: &GeneratedSourceObject,
    standards_lock_sha256: &str,
) -> Result<GeneratedFile, GenerateError> {
    validate_color_softcopy_presentation_state_source(&run.out_dir, source)?;
    let source_series_instance_uid = required_source_uid(
        source.series_instance_uid.as_deref(),
        COLOR_SOFTCOPY_PRESENTATION_STATE_CASE_ID,
        "Color Softcopy Presentation State source Series Instance UID is missing",
    )?;
    let uid = |role| {
        deterministic_uid(&DeterministicUidInput {
            standards_lock_sha256,
            case_id: COLOR_SOFTCOPY_PRESENTATION_STATE_CASE_ID,
            recipe_version: COLOR_SOFTCOPY_PRESENTATION_STATE_RECIPE_VERSION,
            run_seed: run.seed,
            file_index: 0,
            frame_index: None,
            referenced_object_index: None,
            role,
        })
    };
    let series_instance_uid = uid(UidRole::SeriesInstance);
    let sop_instance_uid = uid(UidRole::SopInstance);
    if series_instance_uid == source_series_instance_uid {
        return Err(color_softcopy_presentation_state_source_error(
            "Color Softcopy Presentation State and source Series Instance UIDs must differ",
        ));
    }
    let implementation_class_uid = deterministic_implementation_uid(standards_lock_sha256);
    let relative_path = format!(
        "{COLOR_SOFTCOPY_PRESENTATION_STATE_CASE_ID}/{COLOR_SOFTCOPY_PRESENTATION_STATE_OUTPUT_FILE}"
    );
    let path = run.out_dir.join(&relative_path);
    let case_dir = path.parent().ok_or_else(|| GenerateError::MetadataShape {
        path: PathBuf::from(&relative_path),
        message: "Color Softcopy Presentation State output must have a parent directory",
    })?;
    fs::create_dir_all(case_dir).map_err(|source| GenerateError::CreateCaseOutputDir {
        path: case_dir.to_path_buf(),
        source,
    })?;

    let object = build_color_softcopy_presentation_state(ColorSoftcopyPresentationStateInput {
        sop_instance_uid: &sop_instance_uid,
        series_instance_uid: &series_instance_uid,
        source: ColorSoftcopyPresentationStateReference {
            study_instance_uid: &source.study_instance_uid,
            series_instance_uid: source_series_instance_uid,
            sop_class_uid: &source.sop_class_uid,
            sop_instance_uid: &source.sop_instance_uid,
        },
    })
    .map_err(|message| GenerateError::WriteDicomFile {
        path: path.clone(),
        message,
    })?;
    object
        .with_meta(
            FileMetaTableBuilder::new()
                .transfer_syntax(EXPLICIT_VR_LITTLE_ENDIAN.uid)
                .implementation_class_uid(&implementation_class_uid)
                .implementation_version_name(crate::IMPLEMENTATION_VERSION_NAME),
        )
        .map_err(|error| GenerateError::WriteDicomFile {
            path: path.clone(),
            message: error.to_string(),
        })?
        .write_to_file(&path)
        .map_err(|error| GenerateError::WriteDicomFile {
            path: path.clone(),
            message: error.to_string(),
        })?;

    let validated = validate_color_softcopy_presentation_state_file(
        &path,
        &ColorSoftcopyPresentationStateExpectations {
            sop_class_uid: COLOR_SOFTCOPY_PRESENTATION_STATE_STORAGE_UID,
            sop_instance_uid: &sop_instance_uid,
            transfer_syntax_uid: EXPLICIT_VR_LITTLE_ENDIAN.uid,
            implementation_class_uid: &implementation_class_uid,
            synthetic_data: "YES",
            study_instance_uid: &source.study_instance_uid,
            series_instance_uid: &series_instance_uid,
            source_study_instance_uid: &source.study_instance_uid,
            source_series_instance_uid,
            source_sop_class_uid: &source.sop_class_uid,
            source_sop_instance_uid: &source.sop_instance_uid,
            icc_profile_sha256: ICC_PROFILE_SHA256,
        },
    )?;
    let mut validation = validated.validation;
    validation["internal"]
        .as_array_mut()
        .expect("Color Softcopy validation internal results are an array")
        .push(serde_json::json!({
            "name": "color_softcopy_source_precheck",
            "status": "passed",
            "message": "Rust reopened and hashed the RGB source, then verified its manifest identity, Explicit VR Little Endian encoding, single-frame 2x2 interleaved RGB shape, and 8-bit depth before construction."
        }));
    let bytes = validated.bytes;
    let expected_color_softcopy_presentation_state = serde_json::json!({
        "presentation_state": {
            "modality": "PR",
            "body_part_examined": "HAND",
            "laterality": "R",
            "content_label": COLOR_SOFTCOPY_PRESENTATION_STATE_CONTENT_LABEL,
            "content_description": COLOR_SOFTCOPY_PRESENTATION_STATE_CONTENT_DESCRIPTION,
            "presentation_creation_date": COLOR_SOFTCOPY_PRESENTATION_STATE_CREATION_DATE,
            "presentation_creation_time": COLOR_SOFTCOPY_PRESENTATION_STATE_CREATION_TIME,
            "instance_number": 1,
            "series_number": 62
        },
        "source": {
            "source_case_id": source.source_case_id,
            "source_path": source.source_path,
            "source_sha256": source.sha256,
            "study_instance_uid": source.study_instance_uid,
            "series_instance_uid": source_series_instance_uid,
            "sop_class_uid": source.sop_class_uid,
            "sop_instance_uid": source.sop_instance_uid,
            "rows": 2,
            "columns": 2,
            "photometric_interpretation": "RGB",
            "samples_per_pixel": 3,
            "planar_configuration": 0,
            "complete_instance": true
        },
        "same_study": true,
        "different_series": true,
        "relationship": {
            "referenced_series_items": 1,
            "referenced_image_items": 1,
            "referenced_frame_numbers": [],
            "applies_to_complete_instance": true
        },
        "displayed_area": {
            "items": 1,
            "applies_to_all_references": true,
            "top_left": [1, 1],
            "bottom_right": [2, 2],
            "presentation_size_mode": "SCALE TO FIT",
            "presentation_pixel_aspect_ratio": [1, 1],
            "presentation_pixel_spacing": Value::Null,
            "presentation_pixel_magnification_ratio": Value::Null
        },
        "icc_profile": {
            "vr": "OB",
            "size_bytes": ICC_PROFILE_SIZE,
            "sha256": ICC_PROFILE_SHA256,
            "device_class": "scnr",
            "data_color_space": "RGB ",
            "profile_connection_space": "XYZ ",
            "signature": "acsp",
            "dicom_color_space": ICC_COLOR_SPACE
        },
        "shutter_items": 0,
        "graphic_annotation_items": 0,
        "graphic_layer_items": 0,
        "overlay_items": 0,
        "spatial_transform_present": false,
        "pixel_data_absent": true
    });

    Ok(GeneratedFile {
        case_id: COLOR_SOFTCOPY_PRESENTATION_STATE_CASE_ID.to_string(),
        manifest_entry: serde_json::json!({
            "case_id": COLOR_SOFTCOPY_PRESENTATION_STATE_CASE_ID,
            "profile_membership": ["extended"],
            "path": relative_path,
            "sha256": sha256_hex(&bytes),
            "size_bytes": bytes.len(),
            "determinism": "byte_stable",
            "recipe": {
                "recipe_id": COLOR_SOFTCOPY_PRESENTATION_STATE_RECIPE_ID,
                "recipe_version": COLOR_SOFTCOPY_PRESENTATION_STATE_RECIPE_VERSION,
                "recipe_parameters": {
                    "source_case_id": COLOR_SOFTCOPY_PRESENTATION_STATE_SOURCE_CASE_ID,
                    "complete_instance": true,
                    "displayed_area_top_left": [1, 1],
                    "displayed_area_bottom_right": [2, 2],
                    "presentation_size_mode": "SCALE TO FIT",
                    "presentation_pixel_aspect_ratio": [1, 1],
                    "icc_profile_sha256": ICC_PROFILE_SHA256
                }
            },
            "dicom": {
                "sop_class_uid": COLOR_SOFTCOPY_PRESENTATION_STATE_STORAGE_UID,
                "sop_class_name": "Color Softcopy Presentation State Storage",
                "iod_name": "Color Softcopy Presentation State",
                "modality": "PR",
                "transfer_syntax_uid": EXPLICIT_VR_LITTLE_ENDIAN.uid,
                "transfer_syntax_name": EXPLICIT_VR_LITTLE_ENDIAN.name
            },
            "uids": {
                "study_instance_uid": source.study_instance_uid,
                "series_instance_uid": series_instance_uid,
                "sop_instance_uid": sop_instance_uid,
                "implementation_class_uid": implementation_class_uid,
                "implementation_version_name": crate::IMPLEMENTATION_VERSION_NAME
            },
            "image": Value::Null,
            "pixel_data": Value::Null,
            "references": [
                source.to_manifest_reference("source_image", None)
            ],
            "expected_capabilities": [
                "open_file", "read_metadata", "resolve_references",
                "apply_color_presentation_state", "apply_displayed_area",
                "color_manage_icc_profile"
            ],
            "expected_semantics": {
                "synthetic_data": "YES",
                "same_study_as_source": true,
                "different_series_from_source": true,
                "complete_instance_reference": true,
                "global_displayed_area": true,
                "pixel_data_absent": true
            },
            "expected_color_softcopy_presentation_state":
                expected_color_softcopy_presentation_state,
            "expected_visual_checks": {
                "pattern": "color_pr_displays_entire_2x2_rgb_source_with_srgb_profile"
            },
            "validation": validation,
            "known_stressors": [
                "color_softcopy_presentation_state_storage", "same_study_reference",
                "distinct_presentation_series", "complete_instance_reference",
                "global_displayed_area", "one_based_display_coordinates",
                "mandatory_exact_icc_profile", "optional_rendering_modules_absent"
            ],
            "standards_evidence":
                deduplicated_standards_evidence(standards_evidence_from_case(case))
        }),
    })
}

fn write_advanced_blending_presentation_state_case(
    run: &PreparedGenerationRun,
    case: &Value,
    sources: &[GeneratedSourceObject; 4],
    standards_lock_sha256: &str,
) -> Result<GeneratedFile, GenerateError> {
    validate_advanced_blending_sources(&run.out_dir, sources)?;
    let study_instance_uid = sources[0].study_instance_uid.as_str();
    let frame_of_reference_uid = required_source_uid(
        sources[0].frame_of_reference_uid.as_deref(),
        ADVANCED_BLENDING_PRESENTATION_STATE_CASE_ID,
        "Advanced Blending source Frame of Reference UID is missing",
    )?;
    let source_series_uids = [
        required_source_uid(
            sources[0].series_instance_uid.as_deref(),
            ADVANCED_BLENDING_PRESENTATION_STATE_CASE_ID,
            "Advanced Blending source series 1 UID is missing",
        )?,
        required_source_uid(
            sources[2].series_instance_uid.as_deref(),
            ADVANCED_BLENDING_PRESENTATION_STATE_CASE_ID,
            "Advanced Blending source series 2 UID is missing",
        )?,
    ];
    let uid = |role| {
        deterministic_uid(&DeterministicUidInput {
            standards_lock_sha256,
            case_id: ADVANCED_BLENDING_PRESENTATION_STATE_CASE_ID,
            recipe_version: ADVANCED_BLENDING_PRESENTATION_STATE_RECIPE_VERSION,
            run_seed: run.seed,
            file_index: 0,
            frame_index: None,
            referenced_object_index: None,
            role,
        })
    };
    let series_instance_uid = uid(UidRole::SeriesInstance);
    let sop_instance_uid = uid(UidRole::SopInstance);
    if source_series_uids.contains(&series_instance_uid.as_str()) {
        return Err(advanced_blending_source_error(
            "Advanced Blending Presentation State Series UID must differ from both source Series UIDs",
        ));
    }
    let implementation_class_uid = deterministic_implementation_uid(standards_lock_sha256);
    let relative_path = format!(
        "{ADVANCED_BLENDING_PRESENTATION_STATE_CASE_ID}/{ADVANCED_BLENDING_PRESENTATION_STATE_OUTPUT_FILE}"
    );
    let path = run.out_dir.join(&relative_path);
    let case_dir = path.parent().ok_or_else(|| GenerateError::MetadataShape {
        path: PathBuf::from(&relative_path),
        message: "Advanced Blending output must have a parent directory",
    })?;
    fs::create_dir_all(case_dir).map_err(|source| GenerateError::CreateCaseOutputDir {
        path: case_dir.to_path_buf(),
        source,
    })?;

    let source_references = [
        advanced_blending_reference(&sources[0])?,
        advanced_blending_reference(&sources[1])?,
        advanced_blending_reference(&sources[2])?,
        advanced_blending_reference(&sources[3])?,
    ];
    let object =
        build_advanced_blending_presentation_state(AdvancedBlendingPresentationStateInput {
            sop_instance_uid: &sop_instance_uid,
            series_instance_uid: &series_instance_uid,
            sources: [
                [source_references[0], source_references[1]],
                [source_references[2], source_references[3]],
            ],
        })
        .map_err(|message| GenerateError::WriteDicomFile {
            path: path.clone(),
            message,
        })?;
    object
        .with_meta(
            FileMetaTableBuilder::new()
                .transfer_syntax(EXPLICIT_VR_LITTLE_ENDIAN.uid)
                .implementation_class_uid(&implementation_class_uid)
                .implementation_version_name(crate::IMPLEMENTATION_VERSION_NAME),
        )
        .map_err(|error| GenerateError::WriteDicomFile {
            path: path.clone(),
            message: error.to_string(),
        })?
        .write_to_file(&path)
        .map_err(|error| GenerateError::WriteDicomFile {
            path: path.clone(),
            message: error.to_string(),
        })?;

    let validated = validate_advanced_blending_presentation_state_file(
        &path,
        &AdvancedBlendingPresentationStateExpectations {
            sop_class_uid: ADVANCED_BLENDING_PRESENTATION_STATE_STORAGE_UID,
            sop_instance_uid: &sop_instance_uid,
            transfer_syntax_uid: EXPLICIT_VR_LITTLE_ENDIAN.uid,
            implementation_class_uid: &implementation_class_uid,
            synthetic_data: "YES",
            study_instance_uid,
            series_instance_uid: &series_instance_uid,
            frame_of_reference_uid,
            source_series: [
                AdvancedBlendingSourceSeriesExpectations {
                    series_instance_uid: source_series_uids[0],
                    sop_class_uid: &sources[0].sop_class_uid,
                    sop_instance_uids: [&sources[0].sop_instance_uid, &sources[1].sop_instance_uid],
                },
                AdvancedBlendingSourceSeriesExpectations {
                    series_instance_uid: source_series_uids[1],
                    sop_class_uid: &sources[2].sop_class_uid,
                    sop_instance_uids: [&sources[2].sop_instance_uid, &sources[3].sop_instance_uid],
                },
            ],
            icc_profile_sha256: ICC_PROFILE_SHA256,
        },
    )?;
    let mut validation = validated.validation;
    validation["internal"]
        .as_array_mut()
        .expect("Advanced Blending validation results must be an array")
        .push(serde_json::json!({
            "name": "advanced_blending_source_precheck",
            "status": "passed",
            "message": "Rust reopened and hashed all four source CT files and verified exact Study, Series, Frame of Reference, SOP, transfer syntax, geometry, and ordering before construction."
        }));
    let bytes = validated.bytes;

    let source_manifest = sources
        .iter()
        .enumerate()
        .map(|(index, source)| {
            let series_order = index / 2 + 1;
            let image_order = index % 2 + 1;
            serde_json::json!({
                "source_case_id": source.source_case_id,
                "source_path": source.source_path,
                "source_sha256": source.sha256,
                "study_instance_uid": source.study_instance_uid,
                "series_instance_uid": source.series_instance_uid,
                "frame_of_reference_uid": source.frame_of_reference_uid,
                "sop_class_uid": source.sop_class_uid,
                "sop_instance_uid": source.sop_instance_uid,
                "series_order": series_order,
                "image_order": image_order,
                "rows": 2,
                "columns": 2,
                "image_orientation_patient": [1, 0, 0, 0, 1, 0],
                "image_position_patient_mm": [0, 0, if image_order == 1 { 0 } else { 5 }],
                "referenced_frame_numbers": [],
                "complete_instance": true
            })
        })
        .collect::<Vec<_>>();
    let expected = serde_json::json!({
        "presentation_state": {
            "study_instance_uid": study_instance_uid,
            "series_instance_uid": series_instance_uid,
            "sop_instance_uid": sop_instance_uid,
            "frame_of_reference_uid": frame_of_reference_uid,
            "position_reference_indicator": "",
            "modality": "PR",
            "laterality": "R",
            "content_label": ADVANCED_BLENDING_PRESENTATION_STATE_CONTENT_LABEL,
            "content_description": ADVANCED_BLENDING_PRESENTATION_STATE_CONTENT_DESCRIPTION,
            "content_creator_name": "DTS^Generator",
            "presentation_creation_date": ADVANCED_BLENDING_PRESENTATION_STATE_CREATION_DATE,
            "presentation_creation_time": ADVANCED_BLENDING_PRESENTATION_STATE_CREATION_TIME,
            "instance_number": 1,
            "series_number": 80
        },
        "sources": source_manifest,
        "same_study": true,
        "shared_frame_of_reference": true,
        "different_series": true,
        "blending_inputs": [
            {"input_number": 1, "source_series_order": 1, "study_instance_uid": study_instance_uid, "series_instance_uid": source_series_uids[0], "referenced_source_indices": [1, 2], "time_series_blending": "FALSE", "geometry_for_display": "TRUE", "complete_instances": true},
            {"input_number": 2, "source_series_order": 2, "study_instance_uid": study_instance_uid, "series_instance_uid": source_series_uids[1], "referenced_source_indices": [3, 4], "time_series_blending": "FALSE", "geometry_for_display": "FALSE", "complete_instances": true}
        ],
        "pixel_presentation": "TRUE_COLOR",
        "display_operation": {"items": 1, "input_numbers": [1, 2], "blending_mode": "EQUAL", "relative_opacity": Value::Null, "output_blending_input_number": Value::Null, "final_output": true},
        "icc_profile": {"vr": "OB", "size_bytes": ICC_PROFILE_SIZE, "sha256": ICC_PROFILE_SHA256, "device_class": "scnr", "data_color_space": "RGB ", "profile_connection_space": "XYZ ", "signature": "acsp", "dicom_color_space": ICC_COLOR_SPACE},
        "common_instance_reference": {"series": [
            {"series_order": 1, "series_instance_uid": source_series_uids[0], "referenced_source_indices": [1, 2]},
            {"series_order": 2, "series_instance_uid": source_series_uids[1], "referenced_source_indices": [3, 4]}
        ], "other_study_items": 0, "mirrors_blending_inputs": true},
        "optional_transforms": {"referenced_spatial_registration_items": 0, "optical_path_selection_items": 0, "softcopy_voi_lut_items": 0, "palette_color_lut_items": 0, "threshold_items": 0, "displayed_area_items": 0, "graphic_annotation_items": 0, "graphic_group_items": 0, "specimen_items": 0, "spatial_transform_present": false, "graphic_layer_items": 0},
        "pixel_data_absent": true
    });

    Ok(GeneratedFile {
        case_id: ADVANCED_BLENDING_PRESENTATION_STATE_CASE_ID.to_string(),
        manifest_entry: serde_json::json!({
            "case_id": ADVANCED_BLENDING_PRESENTATION_STATE_CASE_ID,
            "profile_membership": ["extended"],
            "path": relative_path,
            "sha256": sha256_hex(&bytes),
            "size_bytes": bytes.len(),
            "determinism": "byte_stable",
            "recipe": {"recipe_id": ADVANCED_BLENDING_PRESENTATION_STATE_RECIPE_ID, "recipe_version": ADVANCED_BLENDING_PRESENTATION_STATE_RECIPE_VERSION, "recipe_parameters": {"source_case_id": ADVANCED_BLENDING_PRESENTATION_STATE_SOURCE_CASE_ID, "blending_input_numbers": [1, 2], "display_input_numbers": [1, 2], "blending_mode": "EQUAL", "icc_profile_sha256": ICC_PROFILE_SHA256}},
            "dicom": {"sop_class_uid": ADVANCED_BLENDING_PRESENTATION_STATE_STORAGE_UID, "sop_class_name": "Advanced Blending Presentation State Storage", "iod_name": "Advanced Blending Presentation State", "modality": "PR", "transfer_syntax_uid": EXPLICIT_VR_LITTLE_ENDIAN.uid, "transfer_syntax_name": EXPLICIT_VR_LITTLE_ENDIAN.name},
            "uids": {"study_instance_uid": study_instance_uid, "series_instance_uid": series_instance_uid, "sop_instance_uid": sop_instance_uid, "frame_of_reference_uid": frame_of_reference_uid, "implementation_class_uid": implementation_class_uid, "implementation_version_name": crate::IMPLEMENTATION_VERSION_NAME},
            "image": Value::Null,
            "pixel_data": Value::Null,
            "references": sources.iter().map(|source| source.to_manifest_reference("blending_input", None)).collect::<Vec<_>>(),
            "expected_capabilities": ["open_file", "read_metadata", "resolve_references", "apply_advanced_blending_presentation_state", "render_true_color_blend", "color_manage_icc_profile"],
            "expected_semantics": {"synthetic_data": "YES", "same_study_as_sources": true, "shared_frame_of_reference": true, "two_input_equal_blend": true, "pixel_data_absent": true},
            "expected_advanced_blending_presentation_state": expected,
            "expected_visual_checks": {"pattern": "equal_true_color_blend_of_two_registered_ct_series"},
            "validation": validation,
            "known_stressors": ["advanced_blending_presentation_state_storage", "two_source_series", "four_complete_instance_references", "ordered_blending_graph", "single_geometry_source", "mandatory_exact_icc_profile", "common_instance_reference_closure", "optional_transformations_absent"],
            "standards_evidence": deduplicated_standards_evidence(standards_evidence_from_case(case))
        }),
    })
}

fn write_blending_presentation_state_case(
    run: &PreparedGenerationRun,
    case: &Value,
    sources: &[GeneratedSourceObject; 4],
    standards_lock_sha256: &str,
) -> Result<GeneratedFile, GenerateError> {
    validate_advanced_blending_sources(&run.out_dir, sources)?;
    let study_instance_uid = sources[0].study_instance_uid.as_str();
    let source_series_uids = [
        required_source_uid(
            sources[0].series_instance_uid.as_deref(),
            BLENDING_PRESENTATION_STATE_CASE_ID,
            "Blending source series 1 UID is missing",
        )?,
        required_source_uid(
            sources[2].series_instance_uid.as_deref(),
            BLENDING_PRESENTATION_STATE_CASE_ID,
            "Blending source series 2 UID is missing",
        )?,
    ];
    let uid = |role| {
        deterministic_uid(&DeterministicUidInput {
            standards_lock_sha256,
            case_id: BLENDING_PRESENTATION_STATE_CASE_ID,
            recipe_version: BLENDING_PRESENTATION_STATE_RECIPE_VERSION,
            run_seed: run.seed,
            file_index: 0,
            frame_index: None,
            referenced_object_index: None,
            role,
        })
    };
    let series_instance_uid = uid(UidRole::SeriesInstance);
    let sop_instance_uid = uid(UidRole::SopInstance);
    if source_series_uids.contains(&series_instance_uid.as_str()) {
        return Err(blending_source_error(
            "Blending Presentation State Series UID must differ from both source Series UIDs",
        ));
    }
    let implementation_class_uid = deterministic_implementation_uid(standards_lock_sha256);
    let relative_path =
        format!("{BLENDING_PRESENTATION_STATE_CASE_ID}/{BLENDING_PRESENTATION_STATE_OUTPUT_FILE}");
    let path = run.out_dir.join(&relative_path);
    let case_dir = path.parent().ok_or_else(|| GenerateError::MetadataShape {
        path: PathBuf::from(&relative_path),
        message: "Blending Presentation State output must have a parent directory",
    })?;
    fs::create_dir_all(case_dir).map_err(|source| GenerateError::CreateCaseOutputDir {
        path: case_dir.to_path_buf(),
        source,
    })?;
    let source_references = [
        blending_reference(&sources[0])?,
        blending_reference(&sources[1])?,
        blending_reference(&sources[2])?,
        blending_reference(&sources[3])?,
    ];
    let object = build_blending_presentation_state(BlendingPresentationStateInput {
        sop_instance_uid: &sop_instance_uid,
        series_instance_uid: &series_instance_uid,
        sources: [
            [source_references[0], source_references[1]],
            [source_references[2], source_references[3]],
        ],
    })
    .map_err(|message| GenerateError::WriteDicomFile {
        path: path.clone(),
        message,
    })?;
    object
        .with_meta(
            FileMetaTableBuilder::new()
                .transfer_syntax(EXPLICIT_VR_LITTLE_ENDIAN.uid)
                .implementation_class_uid(&implementation_class_uid)
                .implementation_version_name(crate::IMPLEMENTATION_VERSION_NAME),
        )
        .map_err(|error| GenerateError::WriteDicomFile {
            path: path.clone(),
            message: error.to_string(),
        })?
        .write_to_file(&path)
        .map_err(|error| GenerateError::WriteDicomFile {
            path: path.clone(),
            message: error.to_string(),
        })?;

    let validated = validate_blending_presentation_state_file(
        &path,
        &BlendingPresentationStateExpectations {
            sop_class_uid: BLENDING_PRESENTATION_STATE_STORAGE_UID,
            sop_instance_uid: &sop_instance_uid,
            transfer_syntax_uid: EXPLICIT_VR_LITTLE_ENDIAN.uid,
            implementation_class_uid: &implementation_class_uid,
            synthetic_data: "YES",
            study_instance_uid,
            series_instance_uid: &series_instance_uid,
            source_series: [
                BlendingSourceSeriesExpectations {
                    series_instance_uid: source_series_uids[0],
                    sop_class_uid: &sources[0].sop_class_uid,
                    sop_instance_uids: [&sources[0].sop_instance_uid, &sources[1].sop_instance_uid],
                },
                BlendingSourceSeriesExpectations {
                    series_instance_uid: source_series_uids[1],
                    sop_class_uid: &sources[2].sop_class_uid,
                    sop_instance_uids: [&sources[2].sop_instance_uid, &sources[3].sop_instance_uid],
                },
            ],
            palette_channel_sha256: BLENDING_PRESENTATION_STATE_PALETTE_SHA256,
            icc_profile_sha256: ICC_PROFILE_SHA256,
        },
    )?;
    let mut validation = validated.validation;
    validation["internal"]
        .as_array_mut()
        .expect("Blending validation results must be an array")
        .push(serde_json::json!({
            "name": "blending_source_precheck",
            "status": "passed",
            "message": "Rust reopened and hashed all four source CT files and verified exact Study, Series, Frame of Reference, SOP, transfer syntax, geometry, and ordering before construction."
        }));
    let bytes = validated.bytes;
    let source_manifest = sources
        .iter()
        .enumerate()
        .map(|(index, source)| {
            let series_order = index / 2 + 1;
            let image_order = index % 2 + 1;
            serde_json::json!({
                "source_case_id": source.source_case_id,
                "source_path": source.source_path,
                "source_sha256": source.sha256,
                "study_instance_uid": source.study_instance_uid,
                "series_instance_uid": source.series_instance_uid,
                "frame_of_reference_uid": source.frame_of_reference_uid,
                "sop_class_uid": source.sop_class_uid,
                "sop_instance_uid": source.sop_instance_uid,
                "series_order": series_order,
                "image_order": image_order,
                "rows": 2,
                "columns": 2,
                "image_orientation_patient": [1, 0, 0, 0, 1, 0],
                "image_position_patient_mm": [0, 0, if image_order == 1 { 0 } else { 5 }],
                "referenced_frame_numbers": [],
                "complete_instance": true
            })
        })
        .collect::<Vec<_>>();
    let palette_channel = |channel: &str| {
        serde_json::json!({
            "channel": channel,
            "descriptor": BLENDING_PRESENTATION_STATE_PALETTE_DESCRIPTOR,
            "data_vr": "OW",
            "data_size_bytes": BLENDING_PRESENTATION_STATE_PALETTE_BYTES.len(),
            "data_sha256": BLENDING_PRESENTATION_STATE_PALETTE_SHA256,
            "storage": "identity_u16_little_endian"
        })
    };
    let absent_modules = serde_json::json!({
        "clinical_trial_subject": true,
        "clinical_trial_study": true,
        "clinical_trial_series": true,
        "clinical_trial_equipment": true,
        "patient_study": true,
        "specimen": true,
        "graphic_annotation": true,
        "graphic_layer": true,
        "graphic_group": true,
        "spatial_transformation": true,
        "frame_of_reference": true,
        "common_instance_reference": true,
        "softcopy_presentation_lut": true,
        "voi_lut": true,
        "softcopy_voi_lut": true,
        "overlay_plane": true,
        "overlay_activation": true,
        "display_shutter": true,
        "bitmap_display_shutter": true
    });
    let expected = serde_json::json!({
        "presentation_state": {
            "study_instance_uid": study_instance_uid,
            "series_instance_uid": series_instance_uid,
            "sop_instance_uid": sop_instance_uid,
            "modality": "PR",
            "laterality": "R",
            "content_label": BLENDING_PRESENTATION_STATE_CONTENT_LABEL,
            "content_description": BLENDING_PRESENTATION_STATE_CONTENT_DESCRIPTION,
            "content_creator_name": "DTS^Generator",
            "presentation_creation_date": BLENDING_PRESENTATION_STATE_CREATION_DATE,
            "presentation_creation_time": BLENDING_PRESENTATION_STATE_CREATION_TIME,
            "instance_number": 1,
            "series_number": 81
        },
        "sources": source_manifest,
        "same_study": true,
        "shared_frame_of_reference": true,
        "different_series": true,
        "blending_items": [
            {"blending_position": "UNDERLYING", "source_series_order": 1, "study_instance_uid": study_instance_uid, "series_instance_uid": source_series_uids[0], "referenced_source_indices": [1, 2], "referenced_frame_numbers": [], "rescale_intercept": -1024, "rescale_slope": 1, "rescale_type": "HU", "softcopy_voi_lut_items": 0, "referenced_spatial_registration_items": 0, "complete_instances": true},
            {"blending_position": "SUPERIMPOSED", "source_series_order": 2, "study_instance_uid": study_instance_uid, "series_instance_uid": source_series_uids[1], "referenced_source_indices": [3, 4], "referenced_frame_numbers": [], "rescale_intercept": -1024, "rescale_slope": 1, "rescale_type": "HU", "softcopy_voi_lut_items": 0, "referenced_spatial_registration_items": 0, "complete_instances": true}
        ],
        "relative_opacity": BLENDING_PRESENTATION_STATE_RELATIVE_OPACITY,
        "displayed_area": {"items": 1, "applies_to_all_references": true, "referenced_image_items": 0, "top_left": [1, 1], "bottom_right": [2, 2], "presentation_size_mode": "SCALE TO FIT", "presentation_pixel_aspect_ratio": [1, 1], "presentation_pixel_spacing": Value::Null, "presentation_pixel_magnification_ratio": Value::Null},
        "palette_color_lut": {"channels": [palette_channel("red"), palette_channel("green"), palette_channel("blue")], "segmented_data_present": false, "palette_uid_present": false},
        "icc_profile": {"vr": "OB", "size_bytes": ICC_PROFILE_SIZE, "sha256": ICC_PROFILE_SHA256, "device_class": "scnr", "data_color_space": "RGB ", "profile_connection_space": "XYZ ", "signature": "acsp", "dicom_color_space": ICC_COLOR_SPACE},
        "absent_modules": absent_modules,
        "pixel_data_absent": true
    });

    Ok(GeneratedFile {
        case_id: BLENDING_PRESENTATION_STATE_CASE_ID.to_string(),
        manifest_entry: serde_json::json!({
            "case_id": BLENDING_PRESENTATION_STATE_CASE_ID,
            "profile_membership": ["extended"],
            "path": relative_path,
            "sha256": sha256_hex(&bytes),
            "size_bytes": bytes.len(),
            "determinism": "byte_stable",
            "recipe": {"recipe_id": BLENDING_PRESENTATION_STATE_RECIPE_ID, "recipe_version": BLENDING_PRESENTATION_STATE_RECIPE_VERSION, "recipe_parameters": {"source_case_id": BLENDING_PRESENTATION_STATE_SOURCE_CASE_ID, "blending_positions": ["UNDERLYING", "SUPERIMPOSED"], "relative_opacity": BLENDING_PRESENTATION_STATE_RELATIVE_OPACITY, "palette_channel_sha256": BLENDING_PRESENTATION_STATE_PALETTE_SHA256, "icc_profile_sha256": ICC_PROFILE_SHA256}},
            "dicom": {"sop_class_uid": BLENDING_PRESENTATION_STATE_STORAGE_UID, "sop_class_name": "Blending Softcopy Presentation State Storage", "iod_name": "Blending Softcopy Presentation State", "modality": "PR", "transfer_syntax_uid": EXPLICIT_VR_LITTLE_ENDIAN.uid, "transfer_syntax_name": EXPLICIT_VR_LITTLE_ENDIAN.name},
            "uids": {"study_instance_uid": study_instance_uid, "series_instance_uid": series_instance_uid, "sop_instance_uid": sop_instance_uid, "implementation_class_uid": implementation_class_uid, "implementation_version_name": crate::IMPLEMENTATION_VERSION_NAME},
            "image": Value::Null,
            "pixel_data": Value::Null,
            "references": sources.iter().map(|source| source.to_manifest_reference("blending_source", None)).collect::<Vec<_>>(),
            "expected_capabilities": ["open_file", "read_metadata", "resolve_references", "apply_blending_presentation_state", "render_palette_color_blend", "color_manage_icc_profile"],
            "expected_semantics": {"synthetic_data": "YES", "same_study_as_sources": true, "shared_source_frame_of_reference": true, "underlying_superimposed_blend": true, "pixel_data_absent": true},
            "expected_blending_presentation_state": expected,
            "expected_visual_checks": {"pattern": "equal_opacity_identity_palette_blend_of_two_registered_ct_series"},
            "validation": validation,
            "known_stressors": ["blending_softcopy_presentation_state_storage", "two_source_series", "four_complete_instance_references", "underlying_superimposed_positions", "relative_opacity", "per_item_rescale", "global_displayed_area", "mandatory_palette_color_lut", "mandatory_exact_icc_profile", "optional_modules_absent"],
            "standards_evidence": deduplicated_standards_evidence(standards_evidence_from_case(case))
        }),
    })
}

fn write_twelve_lead_ecg_case(
    run: &PreparedGenerationRun,
    case: &Value,
    standards_lock_sha256: &str,
) -> Result<GeneratedFile, GenerateError> {
    let uid = |role| {
        deterministic_uid(&DeterministicUidInput {
            standards_lock_sha256,
            case_id: TWELVE_LEAD_ECG_CASE_ID,
            recipe_version: TWELVE_LEAD_ECG_RECIPE_VERSION,
            run_seed: run.seed,
            file_index: 0,
            frame_index: None,
            referenced_object_index: None,
            role,
        })
    };
    let study_instance_uid = uid(UidRole::StudyInstance);
    let series_instance_uid = uid(UidRole::SeriesInstance);
    let sop_instance_uid = uid(UidRole::SopInstance);
    let implementation_class_uid = deterministic_implementation_uid(standards_lock_sha256);
    let relative_path = format!("{TWELVE_LEAD_ECG_CASE_ID}/{TWELVE_LEAD_ECG_OUTPUT_FILE}");
    let path = run.out_dir.join(&relative_path);
    let case_dir = path.parent().ok_or_else(|| GenerateError::MetadataShape {
        path: PathBuf::from(&relative_path),
        message: "Twelve-lead ECG output must have a parent directory",
    })?;
    fs::create_dir_all(case_dir).map_err(|source| GenerateError::CreateCaseOutputDir {
        path: case_dir.to_path_buf(),
        source,
    })?;

    let object = build_twelve_lead_ecg(TwelveLeadEcgInput {
        study_instance_uid: &study_instance_uid,
        series_instance_uid: &series_instance_uid,
        sop_instance_uid: &sop_instance_uid,
    })
    .map_err(|message| GenerateError::WriteDicomFile {
        path: path.clone(),
        message,
    })?;
    object
        .with_meta(
            FileMetaTableBuilder::new()
                .transfer_syntax(EXPLICIT_VR_LITTLE_ENDIAN.uid)
                .implementation_class_uid(&implementation_class_uid)
                .implementation_version_name(crate::IMPLEMENTATION_VERSION_NAME),
        )
        .map_err(|error| GenerateError::WriteDicomFile {
            path: path.clone(),
            message: error.to_string(),
        })?
        .write_to_file(&path)
        .map_err(|error| GenerateError::WriteDicomFile {
            path: path.clone(),
            message: error.to_string(),
        })?;

    let expected_waveform = twelve_lead_ecg_expected_waveform();
    let validated = validate_twelve_lead_ecg_file(
        &path,
        &TwelveLeadEcgExpectations {
            sop_instance_uid: &sop_instance_uid,
            implementation_class_uid: &implementation_class_uid,
            study_instance_uid: &study_instance_uid,
            series_instance_uid: &series_instance_uid,
            waveform: expected_waveform,
        },
    )?;
    let bytes = validated.bytes;
    let validation = validated.validation;
    let expected_waveform = serde_json::to_value(expected_waveform)
        .expect("Twelve-lead ECG expectation serialization is infallible");

    Ok(GeneratedFile {
        case_id: TWELVE_LEAD_ECG_CASE_ID.to_string(),
        manifest_entry: serde_json::json!({
            "case_id": TWELVE_LEAD_ECG_CASE_ID,
            "profile_membership": ["extended"],
            "path": relative_path,
            "sha256": sha256_hex(&bytes),
            "size_bytes": bytes.len(),
            "determinism": "byte_stable",
            "recipe": {
                "recipe_id": TWELVE_LEAD_ECG_RECIPE_ID,
                "recipe_version": TWELVE_LEAD_ECG_RECIPE_VERSION,
                "recipe_parameters": {
                    "multiplex_group_label": "RESTING_12_LEAD",
                    "channel_count": 12,
                    "samples_per_channel": 500,
                    "sampling_frequency_hz": 500,
                    "sample_formula": "((s * (c + 1) * 37 + c * 101) mod 2001) - 1000"
                }
            },
            "dicom": {
                "sop_class_uid": TWELVE_LEAD_ECG_STORAGE_UID,
                "sop_class_name": "12-lead ECG Waveform Storage",
                "iod_name": "12-lead ECG Waveform",
                "modality": "ECG",
                "transfer_syntax_uid": EXPLICIT_VR_LITTLE_ENDIAN.uid,
                "transfer_syntax_name": EXPLICIT_VR_LITTLE_ENDIAN.name
            },
            "uids": {
                "study_instance_uid": study_instance_uid,
                "series_instance_uid": series_instance_uid,
                "sop_instance_uid": sop_instance_uid,
                "implementation_class_uid": implementation_class_uid,
                "implementation_version_name": crate::IMPLEMENTATION_VERSION_NAME
            },
            "image": Value::Null,
            "pixel_data": Value::Null,
            "references": [],
            "expected_capabilities": [
                "open_file", "read_metadata", "read_waveform", "display_twelve_lead_ecg"
            ],
            "expected_semantics": {
                "synthetic_data": "YES",
                "simultaneous_sampling": true,
                "one_second_duration": true,
                "pixel_data_absent": true,
                "diagnostic_use": false
            },
            "expected_waveform": expected_waveform,
            "expected_visual_checks": {
                "pattern": "one_second_deterministic_resting_twelve_lead_trace"
            },
            "validation": validation,
            "known_stressors": [
                "twelve_lead_ecg_waveform_storage",
                "twelve_simultaneous_channels",
                "signed_16_bit_ow_payload",
                "channel_then_sample_interleave",
                "cid_3001_channel_sources"
            ],
            "standards_evidence": deduplicated_standards_evidence(standards_evidence_from_case(case))
        }),
    })
}

fn write_general_ecg_case(
    run: &PreparedGenerationRun,
    case: &Value,
    standards_lock_sha256: &str,
) -> Result<GeneratedFile, GenerateError> {
    let uid = |role| {
        deterministic_uid(&DeterministicUidInput {
            standards_lock_sha256,
            case_id: GENERAL_ECG_CASE_ID,
            recipe_version: GENERAL_ECG_RECIPE_VERSION,
            run_seed: run.seed,
            file_index: 0,
            frame_index: None,
            referenced_object_index: None,
            role,
        })
    };
    let study_instance_uid = uid(UidRole::StudyInstance);
    let series_instance_uid = uid(UidRole::SeriesInstance);
    let sop_instance_uid = uid(UidRole::SopInstance);
    let implementation_class_uid = deterministic_implementation_uid(standards_lock_sha256);
    let relative_path = format!("{GENERAL_ECG_CASE_ID}/{GENERAL_ECG_OUTPUT_FILE}");
    let path = run.out_dir.join(&relative_path);
    let case_dir = path.parent().ok_or_else(|| GenerateError::MetadataShape {
        path: PathBuf::from(&relative_path),
        message: "General ECG output must have a parent directory",
    })?;
    fs::create_dir_all(case_dir).map_err(|source| GenerateError::CreateCaseOutputDir {
        path: case_dir.to_path_buf(),
        source,
    })?;

    let object = build_general_ecg(GeneralEcgInput {
        study_instance_uid: &study_instance_uid,
        series_instance_uid: &series_instance_uid,
        sop_instance_uid: &sop_instance_uid,
    })
    .map_err(|message| GenerateError::WriteDicomFile {
        path: path.clone(),
        message,
    })?;
    object
        .with_meta(
            FileMetaTableBuilder::new()
                .transfer_syntax(EXPLICIT_VR_LITTLE_ENDIAN.uid)
                .implementation_class_uid(&implementation_class_uid)
                .implementation_version_name(crate::IMPLEMENTATION_VERSION_NAME),
        )
        .map_err(|error| GenerateError::WriteDicomFile {
            path: path.clone(),
            message: error.to_string(),
        })?
        .write_to_file(&path)
        .map_err(|error| GenerateError::WriteDicomFile {
            path: path.clone(),
            message: error.to_string(),
        })?;

    let expected_waveform = general_ecg_expected_waveform();
    let validated = validate_general_ecg_file(
        &path,
        &GeneralEcgExpectations {
            sop_instance_uid: &sop_instance_uid,
            implementation_class_uid: &implementation_class_uid,
            study_instance_uid: &study_instance_uid,
            series_instance_uid: &series_instance_uid,
            waveform: expected_waveform,
        },
    )?;
    let bytes = validated.bytes;
    let validation = validated.validation;
    let expected_waveform = serde_json::to_value(expected_waveform)
        .expect("General ECG expectation serialization is infallible");

    Ok(GeneratedFile {
        case_id: GENERAL_ECG_CASE_ID.to_string(),
        manifest_entry: serde_json::json!({
            "case_id": GENERAL_ECG_CASE_ID,
            "profile_membership": ["extended"],
            "path": relative_path,
            "sha256": sha256_hex(&bytes),
            "size_bytes": bytes.len(),
            "determinism": "byte_stable",
            "recipe": {
                "recipe_id": GENERAL_ECG_RECIPE_ID,
                "recipe_version": GENERAL_ECG_RECIPE_VERSION,
                "recipe_parameters": {
                    "multiplex_groups": [
                        {
                            "label": "STD12_250HZ",
                            "channel_count": 12,
                            "samples_per_channel": 1000,
                            "sampling_frequency_hz": 250
                        },
                        {
                            "label": "AUX4_1000HZ",
                            "channel_count": 4,
                            "samples_per_channel": 4000,
                            "sampling_frequency_hz": 1000
                        }
                    ],
                    "total_channel_count": GENERAL_ECG_TOTAL_CHANNEL_COUNT,
                    "total_payload_length_bytes": GENERAL_ECG_TOTAL_PAYLOAD_LENGTH,
                    "aggregate_payload_sha256": GENERAL_ECG_AGGREGATE_SHA256,
                    "sample_formula": "((s * (c + 1) * (g + 1) * 37 + c * 101 + g * 307) mod 2001) - 1000"
                }
            },
            "dicom": {
                "sop_class_uid": GENERAL_ECG_STORAGE_UID,
                "sop_class_name": "General ECG Waveform Storage",
                "iod_name": "General ECG Waveform",
                "modality": "ECG",
                "transfer_syntax_uid": EXPLICIT_VR_LITTLE_ENDIAN.uid,
                "transfer_syntax_name": EXPLICIT_VR_LITTLE_ENDIAN.name
            },
            "uids": {
                "study_instance_uid": study_instance_uid,
                "series_instance_uid": series_instance_uid,
                "sop_instance_uid": sop_instance_uid,
                "implementation_class_uid": implementation_class_uid,
                "implementation_version_name": crate::IMPLEMENTATION_VERSION_NAME
            },
            "image": Value::Null,
            "pixel_data": Value::Null,
            "references": [],
            "expected_capabilities": [
                "open_file", "read_metadata", "read_waveform", "display_general_ecg"
            ],
            "expected_semantics": {
                "synthetic_data": "YES",
                "simultaneous_sampling_within_groups": true,
                "common_duration_seconds": 4,
                "cross_group_synchronization_asserted": false,
                "pixel_data_absent": true,
                "diagnostic_use": false
            },
            "expected_waveform": expected_waveform,
            "expected_visual_checks": {
                "pattern": "two_group_deterministic_general_ecg_trace"
            },
            "validation": validation,
            "known_stressors": [
                "general_ecg_waveform_storage",
                "two_heterogeneous_multiplex_groups",
                "sixteen_total_channels",
                "different_group_sampling_frequencies",
                "signed_16_bit_ow_payloads",
                "separate_channel_then_sample_interleave",
                "cid_3001_channel_sources"
            ],
            "standards_evidence": deduplicated_standards_evidence(standards_evidence_from_case(case))
        }),
    })
}

fn blending_reference(
    source: &GeneratedSourceObject,
) -> Result<BlendingPresentationStateReference<'_>, GenerateError> {
    Ok(BlendingPresentationStateReference {
        study_instance_uid: source.study_instance_uid.as_str(),
        series_instance_uid: required_source_uid(
            source.series_instance_uid.as_deref(),
            BLENDING_PRESENTATION_STATE_CASE_ID,
            "Blending source Series UID is missing",
        )?,
        sop_class_uid: source.sop_class_uid.as_str(),
        sop_instance_uid: source.sop_instance_uid.as_str(),
        frame_of_reference_uid: required_source_uid(
            source.frame_of_reference_uid.as_deref(),
            BLENDING_PRESENTATION_STATE_CASE_ID,
            "Blending source Frame of Reference UID is missing",
        )?,
    })
}

fn blending_source_error(message: impl Into<String>) -> GenerateError {
    GenerateError::ValidateDicomFile {
        path: PathBuf::from(BLENDING_PRESENTATION_STATE_CASE_ID),
        message: message.into(),
    }
}

fn validate_advanced_blending_sources(
    generated_root: &std::path::Path,
    sources: &[GeneratedSourceObject; 4],
) -> Result<(), GenerateError> {
    let expected_paths = [
        "geometry/ct/multiseries_shared_frame_of_reference/series-001/slice-001.dcm",
        "geometry/ct/multiseries_shared_frame_of_reference/series-001/slice-002.dcm",
        "geometry/ct/multiseries_shared_frame_of_reference/series-002/slice-001.dcm",
        "geometry/ct/multiseries_shared_frame_of_reference/series-002/slice-002.dcm",
    ];
    let mut study_uid = None;
    let mut frame_uid = None;
    let mut series_uids = [None, None];
    for (index, source) in sources.iter().enumerate() {
        let source_path = generated_root.join(&source.source_path);
        let bytes = fs::read(&source_path).map_err(|source| GenerateError::ReadMetadata {
            path: source_path.clone(),
            source,
        })?;
        let object = open_file(&source_path).map_err(|error| GenerateError::ValidateDicomFile {
            path: source_path.clone(),
            message: error.to_string(),
        })?;
        let text = |tag| {
            object
                .element(tag)
                .map_err(|error| advanced_blending_source_error(error.to_string()))?
                .to_str()
                .map(|value| value.trim_end_matches(['\0', ' ']).to_string())
                .map_err(|error| advanced_blending_source_error(error.to_string()))
        };
        let unsigned = |tag| {
            object
                .element(tag)
                .map_err(|error| advanced_blending_source_error(error.to_string()))?
                .to_int::<u16>()
                .map_err(|error| advanced_blending_source_error(error.to_string()))
        };
        let series_index = index / 2;
        let expected_position = if index % 2 == 0 { "0\\0\\0" } else { "0\\0\\5" };
        let source_series_uid = required_source_uid(
            source.series_instance_uid.as_deref(),
            ADVANCED_BLENDING_PRESENTATION_STATE_CASE_ID,
            "Advanced Blending source Series UID is missing",
        )?;
        let source_frame_uid = required_source_uid(
            source.frame_of_reference_uid.as_deref(),
            ADVANCED_BLENDING_PRESENTATION_STATE_CASE_ID,
            "Advanced Blending source Frame of Reference UID is missing",
        )?;
        if source.source_case_id != ADVANCED_BLENDING_PRESENTATION_STATE_SOURCE_CASE_ID
            || source.source_path != expected_paths[index]
            || source.sop_class_uid != uids::CT_IMAGE_STORAGE
            || source.frame_count != Some(1)
            || sha256_hex(&bytes) != source.sha256
            || object.meta().media_storage_sop_class_uid() != source.sop_class_uid
            || object.meta().media_storage_sop_instance_uid() != source.sop_instance_uid
            || object.meta().transfer_syntax() != EXPLICIT_VR_LITTLE_ENDIAN.uid
            || text(tags::SOP_CLASS_UID)? != source.sop_class_uid
            || text(tags::SOP_INSTANCE_UID)? != source.sop_instance_uid
            || text(tags::STUDY_INSTANCE_UID)? != source.study_instance_uid
            || text(tags::SERIES_INSTANCE_UID)? != source_series_uid
            || text(tags::FRAME_OF_REFERENCE_UID)? != source_frame_uid
            || unsigned(tags::ROWS)? != 2
            || unsigned(tags::COLUMNS)? != 2
            || text(tags::IMAGE_ORIENTATION_PATIENT)? != "1\\0\\0\\0\\1\\0"
            || text(tags::IMAGE_POSITION_PATIENT)? != expected_position
        {
            return Err(advanced_blending_source_error(
                "Advanced Blending source identity, bytes, transfer syntax, geometry, or order differs from the locked four-CT recipe",
            ));
        }
        if study_uid.get_or_insert(source.study_instance_uid.as_str()) != &source.study_instance_uid
        {
            return Err(advanced_blending_source_error(
                "Advanced Blending sources must share one Study Instance UID",
            ));
        }
        if frame_uid.get_or_insert(source_frame_uid) != &source_frame_uid {
            return Err(advanced_blending_source_error(
                "Advanced Blending sources must share one Frame of Reference UID",
            ));
        }
        if let Some(series_uid) = series_uids[series_index] {
            if series_uid != source_series_uid {
                return Err(advanced_blending_source_error(
                    "Advanced Blending source images in each ordered pair must share a Series UID",
                ));
            }
        } else {
            series_uids[series_index] = Some(source_series_uid);
        }
    }
    if series_uids[0] == series_uids[1] {
        return Err(advanced_blending_source_error(
            "Advanced Blending source Series UIDs must differ",
        ));
    }
    Ok(())
}

fn advanced_blending_reference(
    source: &GeneratedSourceObject,
) -> Result<AdvancedBlendingPresentationStateReference<'_>, GenerateError> {
    Ok(AdvancedBlendingPresentationStateReference {
        study_instance_uid: source.study_instance_uid.as_str(),
        series_instance_uid: required_source_uid(
            source.series_instance_uid.as_deref(),
            ADVANCED_BLENDING_PRESENTATION_STATE_CASE_ID,
            "Advanced Blending source Series UID is missing",
        )?,
        sop_class_uid: source.sop_class_uid.as_str(),
        sop_instance_uid: source.sop_instance_uid.as_str(),
        frame_of_reference_uid: required_source_uid(
            source.frame_of_reference_uid.as_deref(),
            ADVANCED_BLENDING_PRESENTATION_STATE_CASE_ID,
            "Advanced Blending source Frame of Reference UID is missing",
        )?,
    })
}

fn advanced_blending_source_error(message: impl Into<String>) -> GenerateError {
    GenerateError::ValidateDicomFile {
        path: PathBuf::from(ADVANCED_BLENDING_PRESENTATION_STATE_CASE_ID),
        message: message.into(),
    }
}

fn validate_color_softcopy_presentation_state_source(
    generated_root: &std::path::Path,
    source: &GeneratedSourceObject,
) -> Result<(), GenerateError> {
    if source.source_case_id != COLOR_SOFTCOPY_PRESENTATION_STATE_SOURCE_CASE_ID
        || source.source_path
            != format!("{COLOR_SOFTCOPY_PRESENTATION_STATE_SOURCE_CASE_ID}/instance.dcm")
        || source.sop_class_uid != uids::SECONDARY_CAPTURE_IMAGE_STORAGE
        || source.frame_count != Some(1)
    {
        return Err(color_softcopy_presentation_state_source_error(
            "Color Softcopy Presentation State requires the locked single-frame RGB Secondary Capture source",
        ));
    }
    let source_series_instance_uid = required_source_uid(
        source.series_instance_uid.as_deref(),
        COLOR_SOFTCOPY_PRESENTATION_STATE_CASE_ID,
        "Color Softcopy Presentation State source Series Instance UID is missing",
    )?;
    let source_path = generated_root.join(&source.source_path);
    let bytes = fs::read(&source_path).map_err(|error| GenerateError::ReadMetadata {
        path: source_path.clone(),
        source: error,
    })?;
    if sha256_hex(&bytes) != source.sha256 {
        return Err(color_softcopy_presentation_state_source_error(
            "Color Softcopy Presentation State source bytes differ from the manifest hash",
        ));
    }
    let object = open_file(&source_path).map_err(|error| GenerateError::ValidateDicomFile {
        path: source_path.clone(),
        message: error.to_string(),
    })?;
    let text = |tag| {
        object
            .element(tag)
            .map_err(|error| color_softcopy_presentation_state_source_error(error.to_string()))?
            .to_str()
            .map(|value| value.trim_end_matches(['\0', ' ']).to_string())
            .map_err(|error| color_softcopy_presentation_state_source_error(error.to_string()))
    };
    let unsigned = |tag| {
        object
            .element(tag)
            .map_err(|error| color_softcopy_presentation_state_source_error(error.to_string()))?
            .to_int::<u16>()
            .map_err(|error| color_softcopy_presentation_state_source_error(error.to_string()))
    };
    let frames = object
        .element(tags::NUMBER_OF_FRAMES)
        .ok()
        .and_then(|element| element.to_int::<u64>().ok())
        .unwrap_or(1);
    let pixel_data = object
        .element(tags::PIXEL_DATA)
        .map_err(|error| color_softcopy_presentation_state_source_error(error.to_string()))?;
    let pixel_value_length = pixel_data
        .to_bytes()
        .map_err(|error| color_softcopy_presentation_state_source_error(error.to_string()))?
        .len();
    if object.meta().media_storage_sop_class_uid() != source.sop_class_uid
        || object.meta().media_storage_sop_instance_uid() != source.sop_instance_uid
        || object.meta().transfer_syntax() != EXPLICIT_VR_LITTLE_ENDIAN.uid
        || text(tags::SOP_CLASS_UID)? != source.sop_class_uid
        || text(tags::SOP_INSTANCE_UID)? != source.sop_instance_uid
        || text(tags::STUDY_INSTANCE_UID)? != source.study_instance_uid
        || text(tags::SERIES_INSTANCE_UID)? != source_series_instance_uid
        || unsigned(tags::ROWS)? != 2
        || unsigned(tags::COLUMNS)? != 2
        || text(tags::PHOTOMETRIC_INTERPRETATION)? != "RGB"
        || unsigned(tags::SAMPLES_PER_PIXEL)? != 3
        || unsigned(tags::PLANAR_CONFIGURATION)? != 0
        || unsigned(tags::BITS_ALLOCATED)? != 8
        || unsigned(tags::BITS_STORED)? != 8
        || unsigned(tags::HIGH_BIT)? != 7
        || unsigned(tags::PIXEL_REPRESENTATION)? != 0
        || frames != 1
        || pixel_data.vr() != VR::OB
        || pixel_value_length != 12
    {
        return Err(color_softcopy_presentation_state_source_error(
            "Color Softcopy Presentation State source DICOM identity or RGB shape differs from the generated-source registry and locked recipe",
        ));
    }
    Ok(())
}

fn color_softcopy_presentation_state_source_error(message: impl Into<String>) -> GenerateError {
    GenerateError::ValidateDicomFile {
        path: PathBuf::from(COLOR_SOFTCOPY_PRESENTATION_STATE_CASE_ID),
        message: message.into(),
    }
}

fn required_source_uid<'a>(
    value: Option<&'a str>,
    case_id: &'static str,
    message: &'static str,
) -> Result<&'a str, GenerateError> {
    value.ok_or(GenerateError::MetadataShape {
        path: PathBuf::from(case_id),
        message,
    })
}

fn validate_spatial_registration_sources(
    generated_root: &std::path::Path,
    target: &GeneratedSourceObject,
    source: &GeneratedSourceObject,
) -> Result<(), GenerateError> {
    let target_path = generated_root.join(&target.source_path);
    let source_path = generated_root.join(&source.source_path);
    let target_bytes = fs::read(&target_path).map_err(|error| GenerateError::ReadMetadata {
        path: target_path.clone(),
        source: error,
    })?;
    let source_bytes = fs::read(&source_path).map_err(|error| GenerateError::ReadMetadata {
        path: source_path.clone(),
        source: error,
    })?;
    if sha256_hex(&target_bytes) != target.sha256 || sha256_hex(&source_bytes) != source.sha256 {
        return Err(spatial_registration_source_error(
            "Spatial Registration source bytes differ from their manifest hashes",
        ));
    }
    let target_object =
        open_file(&target_path).map_err(|error| GenerateError::ValidateDicomFile {
            path: target_path.clone(),
            message: error.to_string(),
        })?;
    let source_object =
        open_file(&source_path).map_err(|error| GenerateError::ValidateDicomFile {
            path: source_path.clone(),
            message: error.to_string(),
        })?;
    validate_spatial_registration_source_identity(&target_object, target, 2)?;
    validate_spatial_registration_source_identity(&source_object, source, 1)?;
    if target.source_case_id != SPATIAL_REGISTRATION_TARGET_CASE_ID
        || target.sop_class_uid != uids::ENHANCED_CT_IMAGE_STORAGE
        || source.source_case_id != SPATIAL_REGISTRATION_SOURCE_CASE_ID
        || source.sop_class_uid != uids::CT_IMAGE_STORAGE
        || target.study_instance_uid == source.study_instance_uid
        || target.frame_of_reference_uid == source.frame_of_reference_uid
    {
        return Err(spatial_registration_source_error(
            "Spatial Registration requires the locked distinct-study, distinct-Frame-of-Reference CT pair",
        ));
    }

    let shared_element = target_object
        .element(tags::SHARED_FUNCTIONAL_GROUPS_SEQUENCE)
        .map_err(|error| spatial_registration_source_error(error.to_string()))?;
    let shared = shared_element.items().ok_or_else(|| {
        spatial_registration_source_error("Enhanced CT Shared Functional Groups is not a sequence")
    })?;
    let orientation = shared
        .first()
        .and_then(|item| item.element(tags::PLANE_ORIENTATION_SEQUENCE).ok())
        .and_then(|element| element.items())
        .and_then(|items| items.first())
        .and_then(|item| item.element(tags::IMAGE_ORIENTATION_PATIENT).ok())
        .and_then(|element| element.to_multi_float64().ok())
        .ok_or_else(|| spatial_registration_source_error("Enhanced CT orientation is missing"))?;
    if orientation != [1.0, 0.0, 0.0, 0.0, 1.0, 0.0] {
        return Err(spatial_registration_source_error(
            "Enhanced CT orientation differs from the locked axial geometry",
        ));
    }
    let pixel_measures = shared
        .first()
        .and_then(|item| item.element(tags::PIXEL_MEASURES_SEQUENCE).ok())
        .and_then(|element| element.items())
        .and_then(|items| items.first())
        .ok_or_else(|| {
            spatial_registration_source_error("Enhanced CT Pixel Measures is missing")
        })?;
    let pixel_spacing = pixel_measures
        .element(tags::PIXEL_SPACING)
        .ok()
        .and_then(|element| element.to_multi_float64().ok());
    let slice_thickness = pixel_measures
        .element(tags::SLICE_THICKNESS)
        .ok()
        .and_then(|element| element.to_multi_float64().ok());
    let spacing_between_slices = pixel_measures
        .element(tags::SPACING_BETWEEN_SLICES)
        .ok()
        .and_then(|element| element.to_multi_float64().ok());
    if pixel_spacing.as_deref() != Some(&[0.75, 0.75])
        || slice_thickness.as_deref() != Some(&[2.5])
        || spacing_between_slices.as_deref() != Some(&[2.5])
    {
        return Err(spatial_registration_source_error(
            "Enhanced CT Pixel Measures differ from the locked registration geometry",
        ));
    }
    let per_frame_element = target_object
        .element(tags::PER_FRAME_FUNCTIONAL_GROUPS_SEQUENCE)
        .map_err(|error| spatial_registration_source_error(error.to_string()))?;
    let per_frame = per_frame_element.items().ok_or_else(|| {
        spatial_registration_source_error(
            "Enhanced CT Per-frame Functional Groups is not a sequence",
        )
    })?;
    let positions = per_frame
        .iter()
        .map(|item| {
            item.element(tags::PLANE_POSITION_SEQUENCE)
                .ok()
                .and_then(|element| element.items())
                .and_then(|items| items.first())
                .and_then(|item| item.element(tags::IMAGE_POSITION_PATIENT).ok())
                .and_then(|element| element.to_multi_float64().ok())
        })
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| {
            spatial_registration_source_error("Enhanced CT frame positions are missing")
        })?;
    if positions != [[0.0, 0.0, 0.0], [0.0, 0.0, 2.5]] {
        return Err(spatial_registration_source_error(
            "Enhanced CT frame positions differ from the locked registration landmark",
        ));
    }
    let source_orientation = source_object
        .element(tags::IMAGE_ORIENTATION_PATIENT)
        .map_err(|error| spatial_registration_source_error(error.to_string()))?
        .to_multi_float64()
        .map_err(|error| spatial_registration_source_error(error.to_string()))?;
    let source_position = source_object
        .element(tags::IMAGE_POSITION_PATIENT)
        .map_err(|error| spatial_registration_source_error(error.to_string()))?
        .to_multi_float64()
        .map_err(|error| spatial_registration_source_error(error.to_string()))?;
    if source_orientation != [1.0, 0.0, 0.0, 0.0, 1.0, 0.0]
        || source_position != [-0.625, -0.625, 0.0]
    {
        return Err(spatial_registration_source_error(
            "Classic CT source geometry differs from the locked first-pixel center",
        ));
    }
    Ok(())
}

fn validate_spatial_registration_source_identity(
    object: &dicom_object::FileDicomObject<InMemDicomObject>,
    source: &GeneratedSourceObject,
    expected_frames: u64,
) -> Result<(), GenerateError> {
    let text = |tag| {
        object
            .element(tag)
            .map_err(|error| spatial_registration_source_error(error.to_string()))?
            .to_str()
            .map(|value| value.trim_end_matches(['\0', ' ']).to_string())
            .map_err(|error| spatial_registration_source_error(error.to_string()))
    };
    let frames = object
        .element(tags::NUMBER_OF_FRAMES)
        .ok()
        .and_then(|element| element.to_int::<u64>().ok())
        .unwrap_or(1);
    if object.meta().media_storage_sop_class_uid() != source.sop_class_uid
        || object.meta().media_storage_sop_instance_uid() != source.sop_instance_uid
        || text(tags::SOP_CLASS_UID)? != source.sop_class_uid
        || text(tags::SOP_INSTANCE_UID)? != source.sop_instance_uid
        || text(tags::STUDY_INSTANCE_UID)? != source.study_instance_uid
        || text(tags::SERIES_INSTANCE_UID)?
            != source.series_instance_uid.as_deref().unwrap_or_default()
        || text(tags::FRAME_OF_REFERENCE_UID)?
            != source.frame_of_reference_uid.as_deref().unwrap_or_default()
        || frames != expected_frames
    {
        return Err(spatial_registration_source_error(
            "Spatial Registration source DICOM identity differs from the generated-source registry",
        ));
    }
    Ok(())
}

fn spatial_registration_source_error(message: impl Into<String>) -> GenerateError {
    GenerateError::ValidateDicomFile {
        path: PathBuf::from(SPATIAL_REGISTRATION_CASE_ID),
        message: message.into(),
    }
}

fn write_pixel_case(
    run: &PreparedGenerationRun,
    case: &Value,
    recipe: PixelRecipe,
    standards_lock_sha256: &str,
) -> Result<GeneratedFile, GenerateError> {
    write_pixel_case_with_metadata(
        run,
        case,
        recipe,
        standards_lock_sha256,
        None,
        0,
        "instance.dcm",
    )
}

#[derive(Debug, Clone, Copy)]
enum ScMetadataPayload {
    PersonName(MetadataScRecipe),
    Temporal(TimezoneBoundary),
    EmptyType2(EmptyType2ScRecipe),
    StringBoundary(StringBoundaryScRecipe),
    PrivateCreator(PrivateCreatorScRecipe),
    SequenceLength(SequenceLengthVariant),
    Nonsquare(NonsquareGeometryVariant),
}

fn legacy_metadata_validation_contract(
    metadata: ScMetadataPayload,
) -> crate::recipes::MetadataScParameters {
    use crate::recipes::{
        EmptyType2AttributeMetadata, MetadataScParameters, PersonNameComponentGroup,
        PersonNameMetadata, PrivateCreatorBlockMetadata, PrivateElementMetadata,
        PrivateElementValue, SequenceLengthMetadata, StringBoundaryElementMetadata,
        StringValueSource, TimezoneBoundaryMetadata,
    };
    match metadata {
        ScMetadataPayload::PersonName(recipe) => {
            MetadataScParameters::PersonName(PersonNameMetadata {
                specific_character_sets: recipe
                    .specific_character_sets
                    .iter()
                    .map(|value| (*value).to_string())
                    .collect(),
                patient_name_decoded: recipe.patient_name_decoded.to_string(),
                patient_name_raw_hex: uppercase_hex(recipe.patient_name_raw),
                patient_name_raw_sha256: sha256_hex(recipe.patient_name_raw),
                native_unicode_round_trip: recipe.native_unicode_round_trip,
                component_groups: recipe
                    .component_groups
                    .iter()
                    .map(|group| PersonNameComponentGroup {
                        kind: group.kind.to_string(),
                        decoded_value: group.decoded_value.to_string(),
                        components: group.components.map(str::to_string),
                    })
                    .collect(),
            })
        }
        ScMetadataPayload::Temporal(boundary) => {
            MetadataScParameters::TimezoneBoundary(TimezoneBoundaryMetadata {
                boundary_id: boundary.boundary_id.to_string(),
                study_date: boundary.study_date.to_string(),
                study_time: boundary.study_time.to_string(),
                acquisition_date_time: boundary.acquisition_date_time.to_string(),
                timezone_offset: boundary.timezone_offset.to_string(),
                offset_minutes: boundary.offset_minutes,
                normalized_utc: boundary.normalized_utc.to_string(),
            })
        }
        ScMetadataPayload::EmptyType2(recipe) => MetadataScParameters::EmptyType2 {
            attributes: recipe
                .attributes
                .iter()
                .map(|attribute| EmptyType2AttributeMetadata {
                    tag: format!("{:04X},{:04X}", attribute.tag.0, attribute.tag.1),
                    keyword: attribute.keyword.to_string(),
                    vr: attribute.vr.to_string().to_owned(),
                })
                .collect(),
        },
        ScMetadataPayload::StringBoundary(recipe) => {
            let elements = [
                legacy_string_element(
                    "0020,4000",
                    "ImageComments",
                    "LT",
                    StringValueSource::Repeated {
                        pattern: recipe.image_comments_pattern.to_string(),
                        repetitions: recipe.image_comments_repetitions as u32,
                    },
                ),
                legacy_string_element(
                    "0018,1020",
                    "SoftwareVersions",
                    "LO",
                    StringValueSource::Literal {
                        values: recipe.software_versions.map(str::to_string).to_vec(),
                    },
                ),
                legacy_string_element(
                    "0028,0030",
                    "PixelSpacing",
                    "DS",
                    StringValueSource::Literal {
                        values: recipe.pixel_spacing.map(str::to_string).to_vec(),
                    },
                ),
                legacy_string_element(
                    "0020,0012",
                    "AcquisitionNumber",
                    "IS",
                    StringValueSource::Literal {
                        values: vec![recipe.acquisition_number.to_string()],
                    },
                ),
            ];
            MetadataScParameters::StringBoundaries {
                elements: elements
                    .into_iter()
                    .collect::<Vec<StringBoundaryElementMetadata>>(),
            }
        }
        ScMetadataPayload::PrivateCreator(recipe) => MetadataScParameters::PrivateCreators {
            blocks: recipe
                .blocks
                .iter()
                .map(|block| PrivateCreatorBlockMetadata {
                    creator_tag: block.creator_tag_text.to_string(),
                    creator_id: block.creator_id.to_string(),
                    block_start_tag: block.block_start_tag.to_string(),
                    block_end_tag: block.block_end_tag.to_string(),
                    elements: block
                        .elements
                        .iter()
                        .map(|element| PrivateElementMetadata {
                            tag: element.tag_text.to_string(),
                            value: match element.value {
                                PrivateValue::Lo(text) => PrivateElementValue::Lo {
                                    text: text.to_string(),
                                },
                                PrivateValue::Us(number) => PrivateElementValue::Us { number },
                            },
                        })
                        .collect(),
                })
                .collect(),
        },
        ScMetadataPayload::SequenceLength(variant) => {
            let defined = variant.variant_id == SequenceLengthVariantId::Defined;
            MetadataScParameters::SequenceLengths(SequenceLengthMetadata {
                variant_id: variant.variant_id.as_str().to_string(),
                sequence_tag: "0008,2218".to_string(),
                sequence_vr: "SQ".to_string(),
                code_value: SEQUENCE_CODE_VALUE.to_string(),
                coding_scheme_designator: SEQUENCE_CODING_SCHEME_DESIGNATOR.to_string(),
                code_meaning: SEQUENCE_CODE_MEANING.to_string(),
                item_dataset_encoded_length: ITEM_DATASET_ENCODED_LENGTH,
                undefined_item_encoded_length: UNDEFINED_ITEM_ENCODED_LENGTH,
                sequence_length_field_hex: if defined { "38000000" } else { "FFFFFFFF" }
                    .to_string(),
                item_length_field_hex: "FFFFFFFF".to_string(),
                item_delimitation_present: true,
                sequence_delimitation_present: !defined,
            })
        }
        ScMetadataPayload::Nonsquare(_) => {
            unreachable!("nonsquare validation uses its dedicated typed contract")
        }
    }
}

fn legacy_string_element(
    tag: &str,
    keyword: &str,
    vr: &str,
    source: crate::recipes::StringValueSource,
) -> crate::recipes::StringBoundaryElementMetadata {
    let values = match &source {
        crate::recipes::StringValueSource::Repeated {
            pattern,
            repetitions,
        } => vec![pattern.repeat(*repetitions as usize)],
        crate::recipes::StringValueSource::Literal { values } => values.clone(),
    };
    let mut raw = values.join("\\").into_bytes();
    let padding = if raw.len() % 2 == 1 {
        raw.push(b' ');
        "space"
    } else {
        "none"
    };
    crate::recipes::StringBoundaryElementMetadata {
        tag: tag.to_string(),
        keyword: keyword.to_string(),
        vr: vr.to_string(),
        source,
        padding: padding.to_string(),
        raw_value_byte_length: raw.len() as u32,
        raw_value_sha256: sha256_hex(&raw),
    }
}

fn legacy_nonsquare_validation_spec(
    variant: NonsquareGeometryVariant,
) -> crate::curated_validation::NonsquareValidationSpec {
    crate::curated_validation::NonsquareValidationSpec {
        variant_id: variant.variant_id.as_str().to_string(),
        pixel_spacing: variant
            .pixel_spacing_mm
            .map(|values| values.map(str::to_string).to_vec()),
        nominal_scanned_pixel_spacing: variant
            .nominal_scanned_pixel_spacing_mm
            .map(|values| values.map(str::to_string).to_vec()),
        pixel_aspect_ratio: variant
            .pixel_aspect_ratio
            .map(|values| values.map(|value| value.to_string()).to_vec()),
    }
}

fn write_metadata_sc_case(
    run: &PreparedGenerationRun,
    case: &Value,
    recipe: MetadataScRecipe,
    standards_lock_sha256: &str,
) -> Result<GeneratedFile, GenerateError> {
    write_pixel_case_with_metadata(
        run,
        case,
        recipe.pixel,
        standards_lock_sha256,
        Some(ScMetadataPayload::PersonName(recipe)),
        0,
        "instance.dcm",
    )
}

fn write_timezone_sc_case(
    run: &PreparedGenerationRun,
    case: &Value,
    recipe: TimezoneScRecipe,
    standards_lock_sha256: &str,
) -> Result<Vec<GeneratedFile>, GenerateError> {
    recipe
        .boundaries
        .iter()
        .enumerate()
        .map(|(index, boundary)| {
            write_pixel_case_with_metadata(
                run,
                case,
                recipe.pixel,
                standards_lock_sha256,
                Some(ScMetadataPayload::Temporal(*boundary)),
                index as u32,
                &format!("{}.dcm", boundary.boundary_id),
            )
        })
        .collect()
}

fn write_empty_type2_sc_case(
    run: &PreparedGenerationRun,
    case: &Value,
    recipe: EmptyType2ScRecipe,
    standards_lock_sha256: &str,
) -> Result<GeneratedFile, GenerateError> {
    write_pixel_case_with_metadata(
        run,
        case,
        recipe.pixel,
        standards_lock_sha256,
        Some(ScMetadataPayload::EmptyType2(recipe)),
        0,
        "instance.dcm",
    )
}

fn write_string_boundary_sc_case(
    run: &PreparedGenerationRun,
    case: &Value,
    recipe: StringBoundaryScRecipe,
    standards_lock_sha256: &str,
) -> Result<GeneratedFile, GenerateError> {
    write_pixel_case_with_metadata(
        run,
        case,
        recipe.pixel,
        standards_lock_sha256,
        Some(ScMetadataPayload::StringBoundary(recipe)),
        0,
        "instance.dcm",
    )
}

fn write_private_creator_sc_case(
    run: &PreparedGenerationRun,
    case: &Value,
    recipe: PrivateCreatorScRecipe,
    standards_lock_sha256: &str,
) -> Result<GeneratedFile, GenerateError> {
    write_pixel_case_with_metadata(
        run,
        case,
        recipe.pixel,
        standards_lock_sha256,
        Some(ScMetadataPayload::PrivateCreator(recipe)),
        0,
        "instance.dcm",
    )
}

fn write_sequence_length_sc_case(
    run: &PreparedGenerationRun,
    case: &Value,
    recipe: SequenceLengthScRecipe,
    standards_lock_sha256: &str,
) -> Result<Vec<GeneratedFile>, GenerateError> {
    recipe
        .variants
        .iter()
        .enumerate()
        .map(|(index, variant)| {
            write_pixel_case_with_metadata(
                run,
                case,
                recipe.pixel,
                standards_lock_sha256,
                Some(ScMetadataPayload::SequenceLength(*variant)),
                index as u32,
                variant.file_name,
            )
        })
        .collect()
}

fn write_nonsquare_spacing_sc_case(
    run: &PreparedGenerationRun,
    case: &Value,
    recipe: NonsquareSpacingScRecipe,
    standards_lock_sha256: &str,
) -> Result<Vec<GeneratedFile>, GenerateError> {
    if sha256_hex(recipe.pixel.pixel_bytes) != recipe.pixel_data_sha256
        || recipe
            .variants
            .iter()
            .any(|variant| !variant.uses_physical_spacing() && !variant.uses_pixel_aspect_ratio())
    {
        return Err(GenerateError::MetadataShape {
            path: PathBuf::from(recipe.pixel.case_id),
            message: "non-square geometry recipe does not match its locked pixels or exclusive variants",
        });
    }
    recipe
        .variants
        .iter()
        .enumerate()
        .map(|(index, variant)| {
            write_pixel_case_with_metadata(
                run,
                case,
                recipe.pixel,
                standards_lock_sha256,
                Some(ScMetadataPayload::Nonsquare(*variant)),
                index as u32,
                variant.file_name,
            )
        })
        .collect()
}

fn write_pixel_case_with_metadata(
    run: &PreparedGenerationRun,
    case: &Value,
    recipe: PixelRecipe,
    standards_lock_sha256: &str,
    metadata: Option<ScMetadataPayload>,
    file_index: u32,
    file_name: &str,
) -> Result<GeneratedFile, GenerateError> {
    if recipe.case_id == U32_SC_RECIPE.case_id
        && (!U32_SC_RECIPE.pixel_bytes_are_consistent()
            || sha256_hex(U32_SC_RECIPE.pixel_bytes_le) != U32_SC_RECIPE.pixel_data_sha256)
    {
        return Err(GenerateError::MetadataShape {
            path: PathBuf::from(recipe.case_id),
            message: "unsigned 32-bit source pixels do not match their locked bytes/hash",
        });
    }
    let study_instance_uid = deterministic_case_uid_with_file_index(
        standards_lock_sha256,
        recipe,
        run.seed,
        UidRole::StudyInstance,
        file_index,
    );
    let series_instance_uid = deterministic_case_uid_with_file_index(
        standards_lock_sha256,
        recipe,
        run.seed,
        UidRole::SeriesInstance,
        file_index,
    );
    let sop_instance_uid = deterministic_case_uid_with_file_index(
        standards_lock_sha256,
        recipe,
        run.seed,
        UidRole::SopInstance,
        file_index,
    );
    let implementation_class_uid = deterministic_implementation_uid(standards_lock_sha256);

    let relative_path = format!("{}/{file_name}", recipe.case_id);
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
    let is_u1_sc = recipe.case_id == U1_SC_RECIPE.case_id;
    let is_multiframe_sc = is_u1_sc || recipe.case_id == EOT_CASE_ID;
    put_str(&mut obj, tags::SOP_CLASS_UID, VR::UI, sop_class_uid);
    put_str(&mut obj, tags::SOP_INSTANCE_UID, VR::UI, &sop_instance_uid);
    put_str(&mut obj, tags::SYNTHETIC_DATA, VR::CS, "YES");

    if let Some(ScMetadataPayload::PersonName(metadata)) = metadata {
        put_str(
            &mut obj,
            tags::SPECIFIC_CHARACTER_SET,
            VR::CS,
            &metadata.specific_character_sets.join("\\"),
        );
    }
    if let Some(ScMetadataPayload::PersonName(metadata)) = metadata {
        obj.put(DataElement::new(
            tags::PATIENT_NAME,
            VR::PN,
            PrimitiveValue::U8(metadata.patient_name_raw.into()),
        ));
    } else if !matches!(metadata, Some(ScMetadataPayload::EmptyType2(_))) {
        put_str(&mut obj, tags::PATIENT_NAME, VR::PN, "DICOMTEST^SMOKE");
    } else {
        put_str(&mut obj, tags::PATIENT_NAME, VR::PN, "");
    }
    put_str(&mut obj, tags::PATIENT_ID, VR::LO, "DICOMTEST-SMOKE-001");
    let empty_type2 = matches!(metadata, Some(ScMetadataPayload::EmptyType2(_)));
    put_str(
        &mut obj,
        tags::PATIENT_BIRTH_DATE,
        VR::DA,
        if empty_type2 { "" } else { "19700101" },
    );
    put_str(
        &mut obj,
        tags::PATIENT_SEX,
        VR::CS,
        if empty_type2 { "" } else { "O" },
    );

    put_str(
        &mut obj,
        tags::STUDY_INSTANCE_UID,
        VR::UI,
        &study_instance_uid,
    );
    let (study_date, study_time) = match metadata {
        Some(ScMetadataPayload::Temporal(boundary)) => (boundary.study_date, boundary.study_time),
        _ => ("20260101", "000000"),
    };
    put_str(&mut obj, tags::STUDY_DATE, VR::DA, study_date);
    put_str(&mut obj, tags::STUDY_TIME, VR::TM, study_time);
    if let Some(ScMetadataPayload::Temporal(boundary)) = metadata {
        put_str(
            &mut obj,
            tags::ACQUISITION_DATE_TIME,
            VR::DT,
            boundary.acquisition_date_time,
        );
        put_str(
            &mut obj,
            tags::TIMEZONE_OFFSET_FROM_UTC,
            VR::SH,
            boundary.timezone_offset,
        );
    }
    if is_multiframe_sc {
        put_str(
            &mut obj,
            tags::ACQUISITION_DATE_TIME,
            VR::DT,
            "20260101000000",
        );
        put_str(&mut obj, tags::ACQUISITION_NUMBER, VR::IS, "1");
    }
    put_str(&mut obj, tags::REFERRING_PHYSICIAN_NAME, VR::PN, "");
    put_str(&mut obj, tags::STUDY_ID, VR::SH, "SMOKE");
    put_str(&mut obj, tags::ACCESSION_NUMBER, VR::SH, "");

    if let Some(ScMetadataPayload::StringBoundary(metadata)) = metadata {
        put_str(
            &mut obj,
            tags::IMAGE_COMMENTS,
            VR::LT,
            &metadata
                .image_comments_pattern
                .repeat(metadata.image_comments_repetitions),
        );
        put_str(
            &mut obj,
            tags::ACQUISITION_NUMBER,
            VR::IS,
            metadata.acquisition_number,
        );
        put_str(
            &mut obj,
            tags::PIXEL_SPACING,
            VR::DS,
            &metadata.pixel_spacing.join("\\"),
        );
    }
    if let Some(ScMetadataPayload::PrivateCreator(metadata)) = metadata {
        put_private_creator_blocks(&mut obj, metadata).map_err(|message| {
            GenerateError::WriteDicomFile {
                path: path.clone(),
                message,
            }
        })?;
    }
    if let Some(ScMetadataPayload::SequenceLength(variant)) = metadata {
        put_sequence_length_metadata(&mut obj, variant).map_err(|message| {
            GenerateError::WriteDicomFile {
                path: path.clone(),
                message,
            }
        })?;
    }
    if let Some(ScMetadataPayload::Nonsquare(variant)) = metadata {
        if let Some(pixel_spacing) = variant.pixel_spacing_mm {
            put_str(
                &mut obj,
                tags::PIXEL_SPACING,
                VR::DS,
                &pixel_spacing.join("\\"),
            );
        }
        if let Some(nominal_spacing) = variant.nominal_scanned_pixel_spacing_mm {
            put_str(
                &mut obj,
                tags::NOMINAL_SCANNED_PIXEL_SPACING,
                VR::DS,
                &nominal_spacing.join("\\"),
            );
        }
        if let Some([vertical, horizontal]) = variant.pixel_aspect_ratio {
            put_str(
                &mut obj,
                tags::PIXEL_ASPECT_RATIO,
                VR::IS,
                &format!("{vertical}\\{horizontal}"),
            );
        }
    }

    put_str(&mut obj, tags::MODALITY, VR::CS, "OT");
    put_str(
        &mut obj,
        tags::SERIES_INSTANCE_UID,
        VR::UI,
        &series_instance_uid,
    );
    put_str(&mut obj, tags::SERIES_NUMBER, VR::IS, "1");
    if metadata.is_some() && !matches!(metadata, Some(ScMetadataPayload::SequenceLength(_))) {
        put_str(&mut obj, tags::LATERALITY, VR::CS, "R");
    } else if matches!(
        recipe.transfer_syntax.uid,
        JPEG_XL_LOSSY_TRANSFER_SYNTAX_UID | HTJ2K_LOSSY_TRANSFER_SYNTAX_UID
    ) {
        // The General Series Laterality condition is not applicable to these
        // synthetic OT objects, but an empty Type 2 value gives independent
        // IOD validators explicit evidence of that absence.
        put_str(&mut obj, tags::LATERALITY, VR::CS, "");
    }

    put_str(&mut obj, tags::CONVERSION_TYPE, VR::CS, "SYN");
    put_str(&mut obj, tags::MANUFACTURER, VR::LO, "dicom-test-suite");
    put_str(
        &mut obj,
        tags::MANUFACTURER_MODEL_NAME,
        VR::LO,
        recipe.recipe_id,
    );
    if let Some(ScMetadataPayload::StringBoundary(metadata)) = metadata {
        put_str(
            &mut obj,
            tags::SOFTWARE_VERSIONS,
            VR::LO,
            &metadata.software_versions.join("\\"),
        );
    } else {
        put_str(
            &mut obj,
            tags::SOFTWARE_VERSIONS,
            VR::LO,
            crate::PACKAGE_VERSION,
        );
    }

    put_str(&mut obj, tags::INSTANCE_NUMBER, VR::IS, "1");
    put_str(&mut obj, tags::PATIENT_ORIENTATION, VR::CS, "");
    put_str(&mut obj, tags::CONTENT_DATE, VR::DA, "20260101");
    put_str(&mut obj, tags::CONTENT_TIME, VR::TM, "000000");
    if is_multiframe_sc {
        put_str(&mut obj, tags::BODY_PART_EXAMINED, VR::CS, "CHEST");
        put_str(&mut obj, tags::BURNED_IN_ANNOTATION, VR::CS, "NO");
        put_str(&mut obj, tags::LOSSY_IMAGE_COMPRESSION, VR::CS, "00");
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
    if is_multiframe_sc {
        obj.put(DataElement::new(
            tags::FRAME_INCREMENT_POINTER,
            VR::AT,
            PrimitiveValue::Tags(vec![tags::PAGE_NUMBER_VECTOR].into()),
        ));
        put_str(
            &mut obj,
            tags::PAGE_NUMBER_VECTOR,
            VR::IS,
            if recipe.case_id == EOT_CASE_ID {
                "1\\2\\3"
            } else {
                "1\\2"
            },
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
    if recipe.case_id == EOT_CASE_ID {
        put_str(&mut obj, tags::RESCALE_INTERCEPT, VR::DS, "0");
        put_str(&mut obj, tags::RESCALE_SLOPE, VR::DS, "1");
        put_str(&mut obj, tags::RESCALE_TYPE, VR::LO, "US");
        put_str(&mut obj, tags::PRESENTATION_LUT_SHAPE, VR::CS, "IDENTITY");
    }
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
    let mut codec_internal_validation = Vec::new();
    #[allow(unused_mut)]
    let mut lossy_image_compression_ratio: Option<String> = None;
    #[allow(unused_mut)]
    let mut expected_lossy_metrics: Option<Value> = None;
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
        let encapsulated = if recipe.case_id == EOT_CASE_ID {
            let encapsulated =
                EncapsulatedPixelData::one_fragment_per_frame_with_extended_offset_table(
                    &compressed_frames,
                )
                .map_err(|err| GenerateError::WriteDicomFile {
                    path: path.clone(),
                    message: err.to_string(),
                })?;
            let extended = encapsulated
                .extended_offset_table
                .as_ref()
                .expect("EOT constructor must attach its table");
            if extended.lengths != EOT_ENCODED_LENGTHS || extended.offsets != EOT_OFFSETS {
                return Err(GenerateError::WriteDicomFile {
                    path: path.clone(),
                    message: format!(
                        "EOT RLE oracle changed: offsets {:?}, lengths {:?}",
                        extended.offsets, extended.lengths
                    ),
                });
            }
            obj.put(DataElement::new(
                tags::EXTENDED_OFFSET_TABLE,
                VR::OV,
                PrimitiveValue::U8(extended.offset_value_bytes.clone().into()),
            ));
            obj.put(DataElement::new(
                tags::EXTENDED_OFFSET_TABLE_LENGTHS,
                VR::OV,
                PrimitiveValue::U8(extended.length_value_bytes.clone().into()),
            ));
            encapsulated
        } else {
            let basic_offset_table_policy =
                if recipe.case_id == "classic/sc/mono2_u8_multiframe_rle_lossless" {
                    BasicOffsetTablePolicy::Empty
                } else {
                    BasicOffsetTablePolicy::Populated
                };
            EncapsulatedPixelData::one_fragment_per_frame(
                &compressed_frames,
                basic_offset_table_policy,
            )
            .map_err(|err| GenerateError::WriteDicomFile {
                path: path.clone(),
                message: err.to_string(),
            })?
        };
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
    } else if recipe.transfer_syntax == JPEG_XL_LOSSY {
        #[cfg(feature = "jpegxl")]
        {
            let encoder = CjxlJpegXlLossyEncoder::new();
            let identity = encoder.discover_backend_identity().map_err(|err| {
                GenerateError::WriteDicomFile {
                    path: path.clone(),
                    message: err.to_string(),
                }
            })?;
            let encoded = encoder
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
            let decoded = encoder
                .decode_frame(FrameDecodeInput {
                    encoded_frame: &encoded.bytes,
                    rows: recipe.rows,
                    columns: recipe.columns,
                    samples_per_pixel: recipe.samples_per_pixel,
                    bits_allocated: recipe.bits_allocated,
                    bits_stored: recipe.bits_stored,
                    photometric_interpretation: recipe.photometric_interpretation,
                })
                .map_err(|err| GenerateError::ValidateDicomFile {
                    path: path.clone(),
                    message: err.to_string(),
                })?;
            let metrics = calculate_lossy_frame_metrics(
                recipe.pixel_bytes,
                &decoded.native_bytes,
                recipe.rows,
                recipe.columns,
                recipe.samples_per_pixel,
                recipe.bits_allocated,
            )
            .map_err(|err| GenerateError::ValidateDicomFile {
                path: path.clone(),
                message: err.to_string(),
            })?;
            if metrics
                .channels
                .iter()
                .any(|channel| channel.max_absolute_error > 8)
                || metrics.overall_rmse > 3.0
            {
                return Err(GenerateError::ValidateDicomFile {
                    path: path.clone(),
                    message: format!("JPEG XL lossy metrics exceeded approved limits: {metrics:?}"),
                });
            }
            let options = serde_json::json!({
                "input_format": "binary_ppm_rgb8",
                "argument_vector": CjxlJpegXlLossyEncoder::fixed_option_arguments(),
                "distance": 0.05,
                "effort": 7,
                "num_threads": 0,
                "container": false,
                "modular": false
            });
            let compression_ratio = format!(
                "{:.6}",
                recipe.pixel_bytes.len() as f64 / encoded.bytes.len() as f64
            );
            expected_lossy_metrics = Some(lossy_metrics_manifest(
                recipe,
                &metrics,
                &encoded.bytes,
                &compression_ratio,
                ["R", "G", "B"].as_slice(),
                8,
                3.0,
                "cjxl_jpegxl_lossy_encoder",
                "0.11.2",
                &identity.executable_sha256,
                options.clone(),
                CjxlJpegXlLossyEncoder::LOSSY_IMAGE_COMPRESSION_METHOD,
                CjxlJpegXlLossyEncoder::DECODER_ID,
                CjxlJpegXlLossyEncoder::DECODER_VERSION,
                CjxlJpegXlLossyEncoder::DECODER_INDEPENDENCE,
            ));
            codec_internal_validation.push(lossy_metrics_validation("jpeg_xl_lossy", &metrics));
            put_str(&mut obj, tags::LOSSY_IMAGE_COMPRESSION, VR::CS, "01");
            put_str(
                &mut obj,
                tags::LOSSY_IMAGE_COMPRESSION_RATIO,
                VR::DS,
                &compression_ratio,
            );
            put_str(
                &mut obj,
                tags::LOSSY_IMAGE_COMPRESSION_METHOD,
                VR::CS,
                CjxlJpegXlLossyEncoder::LOSSY_IMAGE_COMPRESSION_METHOD,
            );
            lossy_image_compression_ratio = Some(compression_ratio);
            let compressed_frames = vec![encoded.bytes];
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
            Some((FrameEncoder::backend(&encoder), encapsulated, None))
        }
        #[cfg(not(feature = "jpegxl"))]
        {
            return Err(GenerateError::WriteDicomFile {
                path: path.clone(),
                message:
                    "JPEG XL lossy generation requires the jpegxl Cargo feature and cjxl 0.11.2"
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
    } else if recipe.transfer_syntax == HTJ2K_LOSSY {
        #[cfg(feature = "htj2k_openjph")]
        {
            let encoder = OpenJphHtj2kLossyEncoder::new();
            let identity = encoder.discover_backend_identity().map_err(|err| {
                GenerateError::WriteDicomFile {
                    path: path.clone(),
                    message: err.to_string(),
                }
            })?;
            let encoded = encoder
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
            let decoded = encoder
                .decode_frame(FrameDecodeInput {
                    encoded_frame: &encoded.bytes,
                    rows: recipe.rows,
                    columns: recipe.columns,
                    samples_per_pixel: recipe.samples_per_pixel,
                    bits_allocated: recipe.bits_allocated,
                    bits_stored: recipe.bits_stored,
                    photometric_interpretation: recipe.photometric_interpretation,
                })
                .map_err(|err| GenerateError::ValidateDicomFile {
                    path: path.clone(),
                    message: err.to_string(),
                })?;
            let metrics = calculate_lossy_frame_metrics(
                recipe.pixel_bytes,
                &decoded.native_bytes,
                recipe.rows,
                recipe.columns,
                recipe.samples_per_pixel,
                recipe.bits_allocated,
            )
            .map_err(|err| GenerateError::ValidateDicomFile {
                path: path.clone(),
                message: err.to_string(),
            })?;
            if metrics.channels[0].max_absolute_error > 64 || metrics.overall_rmse > 16.0 {
                return Err(GenerateError::ValidateDicomFile {
                    path: path.clone(),
                    message: format!("HTJ2K lossy metrics exceeded approved limits: {metrics:?}"),
                });
            }
            let options = serde_json::json!({
                "input_format": "binary_pgm_u16_big_endian",
                "argument_vector": OpenJphHtj2kLossyEncoder::fixed_option_arguments(),
                "qstep": 0.00025,
                "reversible": false,
                "num_decompositions": 2,
                "colour_transform": false,
                "progression": "LRCP"
            });
            let compression_ratio = format!(
                "{:.6}",
                recipe.pixel_bytes.len() as f64 / encoded.bytes.len() as f64
            );
            expected_lossy_metrics = Some(lossy_metrics_manifest(
                recipe,
                &metrics,
                &encoded.bytes,
                &compression_ratio,
                ["MONOCHROME2"].as_slice(),
                64,
                16.0,
                "openjph_htj2k_lossy_command_encoder",
                "OpenJPH 0.27.3",
                &identity.executable_sha256,
                options.clone(),
                OpenJphHtj2kLossyEncoder::LOSSY_IMAGE_COMPRESSION_METHOD,
                OpenJphHtj2kLossyEncoder::DECODER_ID,
                OpenJphHtj2kLossyEncoder::DECODER_VERSION,
                OpenJphHtj2kLossyEncoder::DECODER_INDEPENDENCE,
            ));
            codec_internal_validation.push(lossy_metrics_validation("htj2k_lossy", &metrics));
            put_str(&mut obj, tags::LOSSY_IMAGE_COMPRESSION, VR::CS, "01");
            put_str(
                &mut obj,
                tags::LOSSY_IMAGE_COMPRESSION_RATIO,
                VR::DS,
                &compression_ratio,
            );
            put_str(
                &mut obj,
                tags::LOSSY_IMAGE_COMPRESSION_METHOD,
                VR::CS,
                OpenJphHtj2kLossyEncoder::LOSSY_IMAGE_COMPRESSION_METHOD,
            );
            lossy_image_compression_ratio = Some(compression_ratio);
            let compressed_frames = vec![encoded.bytes];
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
            Some((FrameEncoder::backend(&encoder), encapsulated, None))
        }
        #[cfg(not(feature = "htj2k_openjph"))]
        {
            return Err(GenerateError::WriteDicomFile {
                path: path.clone(),
                message: "HTJ2K lossy generation requires the htj2k_openjph Cargo feature and ojph_compress 0.27.3".to_string(),
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
    let curated_plan = crate::composition::resolved_plan_from_curated_dataset(
        &obj,
        crate::composition::CuratedPlanInput {
            instance_id: recipe.recipe_id,
            template_id: crate::composition::TemplateId(
                curated_pixel_template_family(recipe).to_string(),
            ),
            template_version: "1.0.0".parse().expect("static template version"),
            sop_class_uid,
            transfer_syntax_uid: file_transfer_syntax.uid,
            study_instance_uid: Some(&study_instance_uid),
            series_instance_uid: Some(&series_instance_uid),
            sop_instance_uid: &sop_instance_uid,
            implementation_class_uid: &implementation_class_uid,
        },
    )
    .map_err(|error| GenerateError::WriteDicomFile {
        path: path.clone(),
        message: format!("resolve curated composition plan: {error}"),
    })?;
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

    if path.exists() {
        fs::remove_file(&path).map_err(|source| GenerateError::WriteDicomFile {
            path: path.clone(),
            message: format!("replace reserved curated instance: {source}"),
        })?;
    }
    if is_dcmtk_legacy_jpeg {
        #[cfg(feature = "legacy_jpeg_dcmtk")]
        {
            let process = dcmtk_lossless_process_for_transfer_syntax(recipe.transfer_syntax)?;
            let source_path = path.with_extension("native-source.dcm");
            crate::composition::Part10Materializer
                .materialize(&curated_plan, &source_path)
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
    } else if matches!(metadata, Some(ScMetadataPayload::SequenceLength(_))) {
        write_part10_preserving_sequence_lengths(&file_obj, &path)?;
    } else {
        crate::composition::Part10Materializer
            .materialize(&curated_plan, &path)
            .map_err(|err| GenerateError::WriteDicomFile {
                path: path.clone(),
                message: err.to_string(),
            })?;
    }

    if recipe.case_id == EOT_CASE_ID {
        codec_internal_validation.push(validate_extended_offset_table_round_trip(&path)?);
    }

    let decoded_frame_hashes = frame_bytes
        .iter()
        .map(|frame| sha256_hex(frame))
        .collect::<Vec<_>>();
    let decoded_frame_hash_refs = decoded_frame_hashes
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    let typed_validation = crate::curated_validation::validate_sc_part10(
        &path,
        &crate::curated_validation::ScPart10ValidationInput {
            sop_class_uid,
            sop_instance_uid: &sop_instance_uid,
            transfer_syntax_uid: recipe.transfer_syntax.uid,
            implementation_class_uid: &output_implementation_class_uid,
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
                .map(|(_, encapsulated, _)| {
                    crate::curated_validation::ScPixelLengthFormula::Encapsulated {
                        fragments: encapsulated.fragments.len(),
                        basic_offset_table_offsets: encapsulated.basic_offset_table.offsets.len(),
                    }
                })
                .unwrap_or_else(|| match pixel_data_length_formula(recipe) {
                    PixelDataLengthFormula::ContiguousSamples => {
                        crate::curated_validation::ScPixelLengthFormula::ContiguousSamples
                    }
                    PixelDataLengthFormula::YbrFull422 => {
                        crate::curated_validation::ScPixelLengthFormula::YbrFull422
                    }
                    PixelDataLengthFormula::BitPackedContinuousFrames => {
                        crate::curated_validation::ScPixelLengthFormula::BitPackedContinuousFrames
                    }
                    PixelDataLengthFormula::BitPackedFrames => {
                        unreachable!("SC recipes do not use per-frame bit packing")
                    }
                    PixelDataLengthFormula::Encapsulated { .. } => {
                        unreachable!("encapsulation is handled above")
                    }
                }),
            decoded_frame_hashes: if compressed_pixel_data.is_some() {
                &decoded_frame_hash_refs
            } else {
                &[]
            },
            palette: recipe
                .palette
                .map(|palette| crate::curated_validation::ScPaletteValidation {
                    descriptor: palette.descriptor,
                    red_data_length: palette.red_data.len(),
                    green_data_length: palette.green_data.len(),
                    blue_data_length: palette.blue_data.len(),
                }),
            padding: recipe
                .padding
                .map(|padding| crate::curated_validation::ScPaddingValidation {
                    value: padding.value,
                    range_limit: padding.range_limit,
                }),
        },
    )
    .map_err(|error| GenerateError::ValidateDicomFile {
        path: path.clone(),
        message: error.to_string(),
    })?;
    let mut validation = typed_validation.legacy_validation_json();
    append_internal_validation(
        &mut validation,
        serde_json::json!({
            "name": "curated_composition_plan",
            "status": "passed",
            "message": "The curated dataset resolved through the shared composition plan before Part 10 materialization."
        }),
    );
    for result in codec_internal_validation {
        append_internal_validation(&mut validation, result);
    }
    if let Some(metadata) = metadata {
        let result = match metadata {
            ScMetadataPayload::PersonName(recipe) => {
                validate_text_metadata_round_trip(&path, recipe)?
            }
            ScMetadataPayload::Temporal(boundary) => {
                validate_temporal_metadata_round_trip(&path, boundary)?
            }
            ScMetadataPayload::EmptyType2(recipe) => {
                validate_empty_type2_metadata_round_trip(&path, recipe)?
            }
            ScMetadataPayload::StringBoundary(recipe) => {
                validate_string_boundary_metadata_round_trip(&path, recipe)?
            }
            ScMetadataPayload::PrivateCreator(recipe) => {
                validate_private_creator_metadata_round_trip(&path, recipe)?
            }
            ScMetadataPayload::SequenceLength(variant) => {
                validate_sequence_length_metadata_round_trip(&path, variant)?
            }
            ScMetadataPayload::Nonsquare(variant) => {
                validate_nonsquare_geometry_round_trip(&path, variant)?
            }
        };
        append_internal_validation(&mut validation, result);
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
            &typed_validation.bytes,
            validation,
            compressed_pixel_data.as_ref(),
            lossy_image_compression_ratio.as_deref(),
            expected_lossy_metrics,
            &decoded_frame_hash_refs,
            metadata,
        ),
    })
}

fn curated_pixel_template_family(recipe: PixelRecipe) -> &'static str {
    if recipe.case_id == U1_SC_RECIPE.case_id {
        "classic/secondary-capture/multiframe-single-bit"
    } else if recipe.case_id == EOT_CASE_ID {
        "classic/secondary-capture/multiframe-grayscale-byte"
    } else {
        "classic/secondary-capture"
    }
}

#[allow(clippy::too_many_arguments)]
fn materialize_curated_classic_dataset(
    object: &InMemDicomObject,
    path: &Path,
    instance_id: &str,
    template_family: &str,
    sop_class_uid: &str,
    transfer_syntax_uid: &str,
    study_instance_uid: &str,
    series_instance_uid: &str,
    sop_instance_uid: &str,
    implementation_class_uid: &str,
) -> Result<(), GenerateError> {
    let plan = crate::composition::resolved_plan_from_curated_dataset(
        object,
        crate::composition::CuratedPlanInput {
            instance_id,
            template_id: crate::composition::TemplateId(template_family.to_string()),
            template_version: "1.0.0".parse().expect("static template version"),
            sop_class_uid,
            transfer_syntax_uid,
            study_instance_uid: Some(study_instance_uid),
            series_instance_uid: Some(series_instance_uid),
            sop_instance_uid,
            implementation_class_uid,
        },
    )
    .map_err(|error| GenerateError::WriteDicomFile {
        path: path.to_path_buf(),
        message: format!("resolve curated composition plan: {error}"),
    })?;
    if path.exists() {
        fs::remove_file(path).map_err(|source| GenerateError::WriteDicomFile {
            path: path.to_path_buf(),
            message: format!("replace reserved curated instance: {source}"),
        })?;
    }
    crate::composition::Part10Materializer
        .materialize(&plan, path)
        .map_err(|error| GenerateError::WriteDicomFile {
            path: path.to_path_buf(),
            message: error.to_string(),
        })
}

fn append_curated_plan_validation(validation: &mut Value) {
    append_internal_validation(
        validation,
        serde_json::json!({
            "name": "curated_composition_plan",
            "status": "passed",
            "message": "The curated dataset resolved through the shared composition plan before Part 10 materialization."
        }),
    );
}

#[allow(clippy::too_many_arguments)]
#[cfg(any(feature = "jpegxl", feature = "htj2k_openjph"))]
fn lossy_metrics_manifest(
    recipe: PixelRecipe,
    metrics: &crate::codecs::LossyFrameMetrics,
    encoded_frame: &[u8],
    dicom_compression_ratio: &str,
    channel_names: &[&str],
    max_absolute_error_limit: u64,
    rmse_limit: f64,
    encoder_id: &str,
    encoder_version: &str,
    executable_sha256: &str,
    encoder_options: Value,
    compression_method: &str,
    decoder_id: &str,
    decoder_version: &str,
    decoder_independence: &str,
) -> Value {
    let options_fingerprint = sha256_hex(
        &serde_json::to_vec(&encoder_options)
            .expect("serializing a JSON encoder options object cannot fail"),
    );
    serde_json::json!({
        "sample_domain": metrics.sample_domain.as_str(),
        "sample_order": if recipe.samples_per_pixel == 1 { "monochrome" } else { "interleaved_by_pixel" },
        "sample_count": metrics.sample_count,
        "dimensions": { "rows": recipe.rows, "columns": recipe.columns, "frames": 1 },
        "channels": metrics.channels.iter().zip(channel_names).map(|(channel, name)| serde_json::json!({
            "index": channel.channel_index,
            "name": name,
            "sample_count": channel.sample_count,
            "max_absolute_error": {
                "observed": channel.max_absolute_error,
                "limit": max_absolute_error_limit
            },
            "rmse": {
                "observed": round_lossy_metric(channel.rmse),
                "limit": rmse_limit
            }
        })).collect::<Vec<_>>(),
        "encoder": {
            "id": encoder_id,
            "version": encoder_version,
            "executable_sha256": executable_sha256,
            "options": encoder_options,
            "options_fingerprint": options_fingerprint
        },
        "overall_rmse": { "observed": round_lossy_metric(metrics.overall_rmse), "limit": rmse_limit },
        "uncompressed_bytes": recipe.pixel_bytes.len(),
        "compressed_bytes": encoded_frame.len(),
        "compression_ratio": {
            "numerator": recipe.pixel_bytes.len(),
            "denominator": encoded_frame.len(),
            "computed": recipe.pixel_bytes.len() as f64 / encoded_frame.len() as f64,
            "dicom_value": dicom_compression_ratio
        },
        "lossy_image_compression": "01",
        "lossy_image_compression_method": compression_method,
        "decoder": {
            "id": decoder_id,
            "version": decoder_version,
            "independence": decoder_independence
        }
    })
}

#[cfg(any(feature = "jpegxl", feature = "htj2k_openjph"))]
fn round_lossy_metric(value: f64) -> f64 {
    (value * 10_000_000_000.0).round() / 10_000_000_000.0
}

#[cfg(any(feature = "jpegxl", feature = "htj2k_openjph"))]
fn lossy_metrics_validation(codec_name: &str, metrics: &crate::codecs::LossyFrameMetrics) -> Value {
    serde_json::json!({
        "name": format!("{codec_name}_independent_decode_metrics"),
        "status": "passed",
        "message": format!(
            "An independent decoder reproduced the complete frame within approved bounds (maximum channel error {}, overall RMSE {:.10}).",
            metrics.channels.iter().map(|channel| channel.max_absolute_error).max().unwrap_or(0),
            metrics.overall_rmse
        )
    })
}

fn validate_extended_offset_table_round_trip(
    path: &std::path::Path,
) -> Result<Value, GenerateError> {
    crate::curated_validation::validate_extended_offset_table_round_trip(
        path,
        &crate::curated_validation::ExtendedOffsetTableValidationSpec {
            offsets: EOT_OFFSETS.to_vec(),
            lengths: EOT_ENCODED_LENGTHS.to_vec(),
            compressed_fragment_lengths: EOT_ENCODED_LENGTHS.to_vec(),
            padded_fragment_lengths: vec![70, 66, 70],
            fragments_per_frame: vec![1, 1, 1],
            fragment_item_start_offsets: vec![8, 86, 160],
            page_numbers: vec![1, 2, 3],
            offset_origin: "first_fragment_item_tag".into(),
            item_header_bytes: 8,
        },
    )
    .map(|(check, _)| check.legacy_json())
    .map_err(|error| GenerateError::ValidateDicomFile {
        path: path.to_path_buf(),
        message: error.to_string(),
    })
}
fn validate_nonsquare_geometry_round_trip(
    path: &std::path::Path,
    variant: NonsquareGeometryVariant,
) -> Result<Value, GenerateError> {
    crate::curated_validation::validate_nonsquare_spec(
        path,
        &legacy_nonsquare_validation_spec(variant),
    )
    .map(|(check, _)| check.legacy_json())
    .map_err(|error| GenerateError::ValidateDicomFile {
        path: path.to_path_buf(),
        message: error.to_string(),
    })
}
fn validate_text_metadata_round_trip(
    path: &std::path::Path,
    recipe: MetadataScRecipe,
) -> Result<Value, GenerateError> {
    let (check, _) = crate::curated_validation::validate_metadata_round_trip(
        path,
        &legacy_metadata_validation_contract(ScMetadataPayload::PersonName(recipe)),
    )
    .map_err(|error| GenerateError::ValidateDicomFile {
        path: path.to_path_buf(),
        message: error.to_string(),
    })?;
    if check.name != recipe.validation_name || check.message != recipe.validation_message {
        return Err(GenerateError::ValidateDicomFile {
            path: path.to_path_buf(),
            message: "shared Person Name validation identity differs from the legacy contract"
                .to_string(),
        });
    }
    Ok(check.legacy_json())
}
fn validate_temporal_metadata_round_trip(
    path: &std::path::Path,
    boundary: TimezoneBoundary,
) -> Result<Value, GenerateError> {
    crate::curated_validation::validate_metadata_round_trip(
        path,
        &legacy_metadata_validation_contract(ScMetadataPayload::Temporal(boundary)),
    )
    .map(|(check, _)| check.legacy_json())
    .map_err(|error| GenerateError::ValidateDicomFile {
        path: path.to_path_buf(),
        message: error.to_string(),
    })
}
fn validate_empty_type2_metadata_round_trip(
    path: &std::path::Path,
    recipe: EmptyType2ScRecipe,
) -> Result<Value, GenerateError> {
    crate::curated_validation::validate_metadata_round_trip(
        path,
        &legacy_metadata_validation_contract(ScMetadataPayload::EmptyType2(recipe)),
    )
    .map(|(check, _)| check.legacy_json())
    .map_err(|error| GenerateError::ValidateDicomFile {
        path: path.to_path_buf(),
        message: error.to_string(),
    })
}
fn validate_string_boundary_metadata_round_trip(
    path: &std::path::Path,
    recipe: StringBoundaryScRecipe,
) -> Result<Value, GenerateError> {
    crate::curated_validation::validate_metadata_round_trip(
        path,
        &legacy_metadata_validation_contract(ScMetadataPayload::StringBoundary(recipe)),
    )
    .map(|(check, _)| check.legacy_json())
    .map_err(|error| GenerateError::ValidateDicomFile {
        path: path.to_path_buf(),
        message: error.to_string(),
    })
}
fn put_private_creator_blocks(
    obj: &mut InMemDicomObject,
    recipe: PrivateCreatorScRecipe,
) -> Result<(), String> {
    let mut planned_tags = BTreeSet::new();
    let mut scoped_creator_ids = BTreeSet::new();
    let mut previous_creator_tag = None;
    for block in recipe.blocks {
        if previous_creator_tag.is_some_and(|tag| tag >= block.creator_tag) {
            return Err("private creator blocks must use ascending creator tags".to_string());
        }
        previous_creator_tag = Some(block.creator_tag);
        let Tag(group, creator_element) = block.creator_tag;
        if group % 2 == 0 || !(0x0010..=0x00FF).contains(&creator_element) {
            return Err(format!(
                "{} is not a valid private creator tag",
                block.creator_tag_text
            ));
        }
        if !block
            .creator_id
            .bytes()
            .all(|byte| (0x20..=0x7E).contains(&byte))
            || block.creator_id.contains(['\\', '~'])
        {
            return Err(format!(
                "private creator {:?} is outside the permitted repertoire",
                block.creator_id
            ));
        }
        if !scoped_creator_ids.insert((group, block.creator_id)) {
            return Err(format!(
                "private creator {:?} is duplicated in group {group:04X}",
                block.creator_id
            ));
        }
        for slot in 0x0010..=0x00FF {
            if let Ok(existing) = obj.element(Tag(group, slot)) {
                if existing.to_str().ok().as_deref() == Some(block.creator_id) {
                    return Err(format!(
                        "private creator {:?} already exists in group {group:04X}",
                        block.creator_id
                    ));
                }
            }
        }
        if obj.element(block.creator_tag).is_ok() || !planned_tags.insert(block.creator_tag) {
            return Err(format!(
                "private creator tag {} is already occupied",
                block.creator_tag_text
            ));
        }

        let expected_high = creator_element << 8;
        let expected_start = format!("{group:04X},{expected_high:04X}");
        let expected_end = format!("{group:04X},{:04X}", expected_high | 0x00FF);
        if block.block_start_tag != expected_start || block.block_end_tag != expected_end {
            return Err(format!(
                "private creator {} declares the wrong block range",
                block.creator_tag_text
            ));
        }
        let mut previous_element = None;
        for element in block.elements {
            if element.tag.0 != group || element.tag.1 & 0xFF00 != expected_high {
                return Err(format!(
                    "private element {} is outside its creator block",
                    element.tag_text
                ));
            }
            if previous_element.is_some_and(|tag| tag >= element.tag) {
                return Err(format!(
                    "private elements in {} must use ascending tags",
                    block.creator_tag_text
                ));
            }
            previous_element = Some(element.tag);
            if obj.element(element.tag).is_ok() || !planned_tags.insert(element.tag) {
                return Err(format!(
                    "private element tag {} is already occupied",
                    element.tag_text
                ));
            }
        }
    }

    for block in recipe.blocks {
        put_str(obj, block.creator_tag, VR::LO, block.creator_id);
        for element in block.elements {
            match element.value {
                PrivateValue::Lo(value) => put_str(obj, element.tag, VR::LO, value),
                PrivateValue::Us(value) => put_u16(obj, element.tag, VR::US, value),
            }
        }
    }
    Ok(())
}

fn validate_private_creator_metadata_round_trip(
    path: &std::path::Path,
    recipe: PrivateCreatorScRecipe,
) -> Result<Value, GenerateError> {
    crate::curated_validation::validate_metadata_round_trip(
        path,
        &legacy_metadata_validation_contract(ScMetadataPayload::PrivateCreator(recipe)),
    )
    .map(|(check, _)| check.legacy_json())
    .map_err(|error| GenerateError::ValidateDicomFile {
        path: path.to_path_buf(),
        message: error.to_string(),
    })
}
fn sequence_code_item() -> InMemDicomObject {
    InMemDicomObject::from_element_iter([
        DataElement::new(tags::CODE_VALUE, VR::SH, SEQUENCE_CODE_VALUE),
        DataElement::new(
            tags::CODING_SCHEME_DESIGNATOR,
            VR::SH,
            SEQUENCE_CODING_SCHEME_DESIGNATOR,
        ),
        DataElement::new(tags::CODE_MEANING, VR::LO, SEQUENCE_CODE_MEANING),
    ])
}

fn sequence_writer_options() -> DataSetWriterOptions {
    DataSetWriterOptions::default()
        .explicit_length_sq_item_strategy(ExplicitLengthSqItemStrategy::NoChange)
}

fn explicit_vr_little_endian_transfer_syntax()
-> Result<&'static dicom_encoding::TransferSyntax, String> {
    TransferSyntaxRegistry
        .get(EXPLICIT_VR_LITTLE_ENDIAN.uid)
        .ok_or_else(|| "Explicit VR Little Endian transfer syntax is unavailable".to_string())
}

fn put_sequence_length_metadata(
    obj: &mut InMemDicomObject,
    variant: SequenceLengthVariant,
) -> Result<(), String> {
    let item = sequence_code_item();
    let transfer_syntax = explicit_vr_little_endian_transfer_syntax()?;
    let mut encoded_item = Vec::new();
    item.write_dataset_with_ts_options(
        &mut encoded_item,
        transfer_syntax,
        sequence_writer_options(),
    )
    .map_err(|error| format!("encode Anatomic Region Sequence item: {error}"))?;
    if encoded_item.len() != ITEM_DATASET_ENCODED_LENGTH as usize {
        return Err(format!(
            "Anatomic Region Sequence item encoded to {} bytes, expected {ITEM_DATASET_ENCODED_LENGTH}",
            encoded_item.len()
        ));
    }
    let declared_defined_length = ITEM_DATASET_ENCODED_LENGTH
        .checked_add(8)
        .and_then(|length| length.checked_add(8))
        .ok_or_else(|| "Anatomic Region Sequence length overflow".to_string())?;
    if declared_defined_length != UNDEFINED_ITEM_ENCODED_LENGTH {
        return Err(format!(
            "computed defined SQ length {declared_defined_length}, expected {UNDEFINED_ITEM_ENCODED_LENGTH}"
        ));
    }
    let sequence_length = match variant.variant_id {
        SequenceLengthVariantId::Defined => Length(declared_defined_length),
        SequenceLengthVariantId::Undefined => Length::UNDEFINED,
    };
    obj.put(DataElement::new(
        tags::ANATOMIC_REGION_SEQUENCE,
        VR::SQ,
        DataSetSequence::new(vec![item], sequence_length),
    ));
    Ok(())
}

fn write_part10_preserving_sequence_lengths(
    file_obj: &dicom_object::FileDicomObject<InMemDicomObject>,
    path: &std::path::Path,
) -> Result<(), GenerateError> {
    let transfer_syntax = explicit_vr_little_endian_transfer_syntax().map_err(|message| {
        GenerateError::WriteDicomFile {
            path: path.to_path_buf(),
            message,
        }
    })?;
    let file = File::create(path).map_err(|error| GenerateError::WriteDicomFile {
        path: path.to_path_buf(),
        message: error.to_string(),
    })?;
    let mut writer = BufWriter::new(file);
    writer
        .write_all(&[0_u8; 128])
        .and_then(|_| writer.write_all(b"DICM"))
        .map_err(|error| GenerateError::WriteDicomFile {
            path: path.to_path_buf(),
            message: error.to_string(),
        })?;
    file_obj
        .write_meta(&mut writer)
        .map_err(|error| GenerateError::WriteDicomFile {
            path: path.to_path_buf(),
            message: error.to_string(),
        })?;
    file_obj
        .write_dataset_with_ts_options(&mut writer, transfer_syntax, sequence_writer_options())
        .map_err(|error| GenerateError::WriteDicomFile {
            path: path.to_path_buf(),
            message: error.to_string(),
        })?;
    writer
        .flush()
        .map_err(|error| GenerateError::WriteDicomFile {
            path: path.to_path_buf(),
            message: error.to_string(),
        })
}

fn validate_sequence_length_metadata_round_trip(
    path: &std::path::Path,
    variant: SequenceLengthVariant,
) -> Result<Value, GenerateError> {
    crate::curated_validation::validate_metadata_round_trip(
        path,
        &legacy_metadata_validation_contract(ScMetadataPayload::SequenceLength(variant)),
    )
    .map(|(check, _)| check.legacy_json())
    .map_err(|error| GenerateError::ValidateDicomFile {
        path: path.to_path_buf(),
        message: error.to_string(),
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
    expected_lossy_metrics: Option<Value>,
    frame_hashes: &[&str],
    metadata: Option<ScMetadataPayload>,
) -> Value {
    let mut standards_evidence = standards_evidence_from_case(case);
    if recipe.case_id == U1_SC_RECIPE.case_id {
        standards_evidence.extend([
            serde_json::json!({
                "source": "dicom-standard-kb",
                "edition": "2026b",
                "query": "lookup_sop_class Multi-frame Single Bit Secondary Capture Image Storage",
                "covered": true,
                "part": "PS3.4",
                "anchor": "table_B.5-1"
            }),
            serde_json::json!({
                "source": "dicom-standard-kb",
                "edition": "2026b",
                "query": "lookup_iod Multi-frame Single Bit Secondary Capture Image",
                "covered": true,
                "part": "PS3.3",
                "anchor": "table_A.8-2"
            }),
            serde_json::json!({
                "source": "dicom-standard-kb",
                "edition": "2026b",
                "query": "retrieve_standard_text sect_A.8.2.4",
                "covered": true,
                "part": "PS3.3",
                "anchor": "sect_A.8.2.4"
            }),
            serde_json::json!({
                "source": "local-source-note",
                "edition": "2026b",
                "query": "standards/source-notes/phase-2-u1-native-pixels.md",
                "covered": true,
                "part": "PS3.5",
                "anchor": "sect_8.1.1"
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
    let extended_offset_table_manifest = compressed_pixel_data
        .and_then(|(_, encapsulated, _)| encapsulated.extended_offset_table.as_ref())
        .map(|extended| {
            serde_json::json!({
                "present": true,
                "lengths_present": true,
                "offset_count": extended.offsets.len(),
                "length_count": extended.lengths.len(),
                "offsets": extended.offsets,
                "lengths": extended.lengths
            })
        })
        .unwrap_or_else(|| {
            serde_json::json!({
                "present": false,
                "lengths_present": false,
                "offset_count": 0,
                "length_count": 0
            })
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
                "extended_offset_table": extended_offset_table_manifest,
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

    let declared_pixel_values = if recipe.case_id == U32_SC_RECIPE.case_id {
        serde_json::json!(U32_SC_RECIPE.pixel_values)
    } else {
        serde_json::json!(recipe.pixel_values)
    };
    let mut manifest = serde_json::json!({
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
                "pixel_values": declared_pixel_values,
                "palette": palette_manifest,
                "pixel_padding": padding_manifest
            }
        },
        "dicom": {
            "sop_class_uid": pixel_sop_class_uid(recipe),
            "sop_class_name": pixel_sop_class_name(recipe),
            "iod_name": pixel_iod_name(recipe),
            "modality": "OT",
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
            "conversion_type": "SYN",
            "image_type": Value::Null,
            "pixel_min": recipe.pixel_min,
            "pixel_max": recipe.pixel_max,
            "pixel_padding": padding_manifest,
            "lossy_image_compression": if matches!(recipe.transfer_syntax, JPEG_BASELINE_8BIT | JPEG_XL_LOSSY | HTJ2K_LOSSY) { "01" } else { "00" },
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
    });
    if recipe.case_id == U32_SC_RECIPE.case_id {
        manifest["expected_u32_pixels"] = serde_json::json!({
            "stored_values": U32_SC_RECIPE.pixel_values,
            "pixel_data_sha256": U32_SC_RECIPE.pixel_data_sha256,
            "word_byte_order": "little_endian",
            "full_unsigned_range": true
        });
    }
    if recipe.case_id == U1_SC_RECIPE.case_id {
        manifest["expected_u1_pixels"] = serde_json::json!({
            "packing_order": "least_significant_bit_first",
            "frame_boundary_policy": "continuous_without_per_frame_padding",
            "stored_values": U1_SC_RECIPE.pixel_values,
            "decoded_frame_sha256": U1_SC_RECIPE.decoded_frame_sha256,
            "pixel_data_sha256": U1_SC_RECIPE.pixel_data_sha256,
            "significant_bits": 18,
            "significant_packed_bytes": U1_SC_RECIPE.significant_packed_bytes,
            "unused_high_bits": 6,
            "value_field_padding_bytes": 1,
            "frame_two_bit_offset": 9
        });
    }
    if recipe.case_id == EOT_CASE_ID {
        manifest["expected_eot"] = serde_json::json!({
            "origin": "first_fragment_item_tag",
            "item_header_bytes": 8,
            "frame_encoded_lengths": EOT_ENCODED_LENGTHS,
            "offsets": EOT_OFFSETS,
            "lengths": EOT_ENCODED_LENGTHS
        });
    }
    if let Some(expected_lossy_metrics) = expected_lossy_metrics {
        manifest["expected_lossy_metrics"] = expected_lossy_metrics;
    }
    if let Some(ScMetadataPayload::PersonName(metadata)) = metadata {
        let mut raw_value = metadata.patient_name_raw.to_vec();
        if raw_value.len() % 2 == 1 {
            raw_value.push(b' ');
        }
        let component_groups = metadata
            .component_groups
            .iter()
            .enumerate()
            .map(|(group_index, group)| {
                let components = group
                    .components
                    .iter()
                    .enumerate()
                    .map(|(component_index, decoded_value)| {
                        serde_json::json!({
                            "position": component_index + 1,
                            "decoded_value": decoded_value
                        })
                    })
                    .collect::<Vec<_>>();
                serde_json::json!({
                    "position": group_index + 1,
                    "kind": group.kind,
                    "decoded_value": group.decoded_value,
                    "components": components
                })
            })
            .collect::<Vec<_>>();
        manifest["expected_metadata"] = serde_json::json!({
            "specific_character_sets": metadata.specific_character_sets,
            "person_names": [{
                "tag": "0010,0010",
                "keyword": "PatientName",
                "vr": "PN",
                "decoded_value": metadata.patient_name_decoded,
                "raw_value_hex": uppercase_hex(&raw_value),
                "raw_value_sha256": sha256_hex(&raw_value),
                "raw_value_byte_length": raw_value.len(),
                "component_groups": component_groups
            }]
        });
        manifest["recipe"]["recipe_parameters"]["specific_character_sets"] =
            serde_json::json!(metadata.specific_character_sets);
        manifest["recipe"]["recipe_parameters"]["patient_name"] =
            Value::from(metadata.patient_name_decoded);
    } else if let Some(ScMetadataPayload::Temporal(boundary)) = metadata {
        let mut timezone_offset = encoded_temporal_value(
            "0008,0201",
            "TimezoneOffsetFromUTC",
            "SH",
            boundary.timezone_offset,
        );
        timezone_offset["offset_minutes"] = Value::from(boundary.offset_minutes);
        let mut acquisition_date_time = encoded_temporal_value(
            "0008,002A",
            "AcquisitionDateTime",
            "DT",
            boundary.acquisition_date_time,
        );
        acquisition_date_time["embedded_offset_minutes"] = Value::from(boundary.offset_minutes);
        acquisition_date_time["normalized_utc"] = Value::from(boundary.normalized_utc);
        manifest["expected_metadata"] = serde_json::json!({
            "temporal": {
                "boundary_id": boundary.boundary_id,
                "timezone_offset_from_utc": timezone_offset,
                "date_values": [encoded_temporal_value(
                    "0008,0020",
                    "StudyDate",
                    "DA",
                    boundary.study_date,
                )],
                "time_values": [encoded_temporal_value(
                    "0008,0030",
                    "StudyTime",
                    "TM",
                    boundary.study_time,
                )],
                "date_time_values": [acquisition_date_time],
                "combined_da_tm_utc": boundary.normalized_utc
            }
        });
        manifest["recipe"]["recipe_parameters"]["temporal_boundary_id"] =
            Value::from(boundary.boundary_id);
        manifest["recipe"]["recipe_parameters"]["timezone_offset_from_utc"] =
            Value::from(boundary.timezone_offset);
    } else if let Some(ScMetadataPayload::EmptyType2(metadata)) = metadata {
        manifest["expected_metadata"] = serde_json::json!({
            "empty_type2_attributes": metadata.attributes.iter().map(|attribute| {
                serde_json::json!({
                    "tag": attribute.tag_text,
                    "keyword": attribute.keyword,
                    "vr": format!("{:?}", attribute.vr),
                    "value_length": 0
                })
            }).collect::<Vec<_>>()
        });
        manifest["recipe"]["recipe_parameters"]["empty_type2_attribute_count"] =
            Value::from(metadata.attributes.len() as u64);
    } else if let Some(ScMetadataPayload::StringBoundary(metadata)) = metadata {
        let comments = metadata
            .image_comments_pattern
            .repeat(metadata.image_comments_repetitions);
        manifest["expected_metadata"] = serde_json::json!({
            "string_elements": [
                encoded_string_element("0020,4000", "ImageComments", "LT", &[comments.as_str()]),
                encoded_string_element("0018,1020", "SoftwareVersions", "LO", &metadata.software_versions),
                encoded_string_element("0028,0030", "PixelSpacing", "DS", &metadata.pixel_spacing),
                encoded_string_element("0020,0012", "AcquisitionNumber", "IS", &[metadata.acquisition_number])
            ]
        });
        manifest["recipe"]["recipe_parameters"]["string_boundary_element_count"] = Value::from(4);
    } else if let Some(ScMetadataPayload::PrivateCreator(metadata)) = metadata {
        manifest["expected_metadata"] = serde_json::json!({
            "private_creator_blocks": metadata
                .blocks
                .iter()
                .map(|block| private_creator_manifest_block(block))
                .collect::<Vec<_>>()
        });
        manifest["recipe"]["recipe_parameters"]["private_creator_block_count"] =
            Value::from(metadata.blocks.len() as u64);
    } else if let Some(ScMetadataPayload::SequenceLength(variant)) = metadata {
        let defined = variant.variant_id == SequenceLengthVariantId::Defined;
        manifest["expected_metadata"] = serde_json::json!({
            "sequence_length_encoding": {
                "variant_id": variant.variant_id.as_str(),
                "sequence_tag": "0008,2218",
                "keyword": "AnatomicRegionSequence",
                "vr": "SQ",
                "sequence_value_length": if defined {
                    Value::from(UNDEFINED_ITEM_ENCODED_LENGTH)
                } else {
                    Value::Null
                },
                "sequence_length_field_hex": if defined { "38000000" } else { "FFFFFFFF" },
                "sequence_delimitation_present": !defined,
                "item_count": 1,
                "item_length_encoding": "undefined",
                "item_length_field_hex": "FFFFFFFF",
                "item_delimitation_present": true,
                "decoded_items": [{
                    "code_value": SEQUENCE_CODE_VALUE,
                    "coding_scheme_designator": SEQUENCE_CODING_SCHEME_DESIGNATOR,
                    "code_meaning": SEQUENCE_CODE_MEANING
                }]
            }
        });
        manifest["recipe"]["recipe_parameters"]["sequence_length_variant"] =
            Value::from(variant.variant_id.as_str());
    } else if let Some(ScMetadataPayload::Nonsquare(variant)) = metadata {
        let pixel_spacing = variant.pixel_spacing_mm.map(|[row, column]| {
            serde_json::json!({
                "tag": "0028,0030",
                "keyword": "PixelSpacing",
                "vr": "DS",
                "vm": 2,
                "lexical_value": format!("{row}\\{column}"),
                "row_spacing_mm": row.parse::<f64>().expect("locked DS row spacing should parse"),
                "column_spacing_mm": column.parse::<f64>().expect("locked DS column spacing should parse")
            })
        });
        let nominal_scanned_pixel_spacing =
            variant
                .nominal_scanned_pixel_spacing_mm
                .map(|[row, column]| {
                    serde_json::json!({
                        "tag": "0018,2010",
                        "keyword": "NominalScannedPixelSpacing",
                        "vr": "DS",
                        "vm": 2,
                        "lexical_value": format!("{row}\\{column}"),
                        "row_spacing_mm": row.parse::<f64>().expect("locked DS row spacing should parse"),
                        "column_spacing_mm": column.parse::<f64>().expect("locked DS column spacing should parse")
                    })
                });
        let pixel_aspect_ratio = variant.pixel_aspect_ratio.map(|[vertical, horizontal]| {
            serde_json::json!({
                "tag": "0028,0034",
                "keyword": "PixelAspectRatio",
                "vr": "IS",
                "vm": 2,
                "lexical_value": format!("{vertical}\\{horizontal}"),
                "vertical_extent": vertical,
                "horizontal_extent": horizontal
            })
        });
        manifest["expected_nonsquare_spacing"] = serde_json::json!({
            "variant_id": variant.variant_id.as_str(),
            "pixel_spacing": pixel_spacing,
            "nominal_scanned_pixel_spacing": nominal_scanned_pixel_spacing,
            "pixel_aspect_ratio": pixel_aspect_ratio,
            "uncalibrated": true,
            "patient_space_geometry_present": false,
            "pixel_data_sha256": NONSQUARE_SPACING_SC_RECIPE.pixel_data_sha256
        });
        manifest["recipe"]["recipe_parameters"]["nonsquare_variant"] =
            Value::from(variant.variant_id.as_str());
        manifest["recipe"]["recipe_parameters"]["row_to_column_ratio"] = Value::from(2.0);
    }
    manifest
}

fn private_creator_manifest_block(block: &PrivateCreatorBlockRecipe) -> Value {
    let creator_raw = padded_text_bytes(block.creator_id);
    let elements = block
        .elements
        .iter()
        .map(|element| {
            let (vr, decoded_value, raw_value) = match element.value {
                PrivateValue::Lo(value) => ("LO", Value::from(value), padded_text_bytes(value)),
                PrivateValue::Us(value) => ("US", Value::from(value), value.to_le_bytes().to_vec()),
            };
            serde_json::json!({
                "tag": element.tag_text,
                "vr": vr,
                "decoded_value": decoded_value,
                "raw_value_hex": uppercase_hex(&raw_value),
                "raw_value_sha256": sha256_hex(&raw_value),
                "raw_value_byte_length": raw_value.len()
            })
        })
        .collect::<Vec<_>>();
    serde_json::json!({
        "creator_tag": block.creator_tag_text,
        "creator_id": block.creator_id,
        "vr": "LO",
        "raw_value_hex": uppercase_hex(&creator_raw),
        "raw_value_sha256": sha256_hex(&creator_raw),
        "raw_value_byte_length": creator_raw.len(),
        "block_start_tag": block.block_start_tag,
        "block_end_tag": block.block_end_tag,
        "elements": elements
    })
}

fn padded_text_bytes(value: &str) -> Vec<u8> {
    let mut raw = value.as_bytes().to_vec();
    if raw.len() % 2 == 1 {
        raw.push(b' ');
    }
    raw
}

fn encoded_temporal_value(tag: &str, keyword: &str, vr: &str, decoded_value: &str) -> Value {
    let mut raw_value = decoded_value.as_bytes().to_vec();
    if raw_value.len() % 2 == 1 {
        raw_value.push(b' ');
    }
    serde_json::json!({
        "tag": tag,
        "keyword": keyword,
        "vr": vr,
        "decoded_value": decoded_value,
        "raw_value_hex": uppercase_hex(&raw_value),
        "raw_value_sha256": sha256_hex(&raw_value),
        "raw_value_byte_length": raw_value.len()
    })
}

fn encoded_string_element(tag: &str, keyword: &str, vr: &str, values: &[&str]) -> Value {
    let joined = values.join("\\");
    let mut raw_value = joined.as_bytes().to_vec();
    let padding = if raw_value.len() % 2 == 1 {
        raw_value.push(b' ');
        "space"
    } else {
        "none"
    };
    serde_json::json!({
        "tag": tag,
        "keyword": keyword,
        "vr": vr,
        "decoded_values": values,
        "value_multiplicity": values.len(),
        "decoded_value_lengths": values.iter().map(|value| value.len()).collect::<Vec<_>>(),
        "raw_value_byte_length": raw_value.len(),
        "raw_value_sha256": sha256_hex(&raw_value),
        "padding": padding
    })
}

fn uppercase_hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;

    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(&mut encoded, "{byte:02X}").expect("writing to a String cannot fail");
    }
    encoded
}

fn pixel_lossy_image_compression_method(recipe: PixelRecipe) -> Option<&'static str> {
    if recipe.transfer_syntax == JPEG_BASELINE_8BIT {
        Some("ISO_10918_1")
    } else if recipe.transfer_syntax == JPEG_XL_LOSSY {
        Some("ISO_18181_1")
    } else if recipe.transfer_syntax == HTJ2K_LOSSY {
        Some("ISO_15444_15")
    } else {
        None
    }
}

fn pixel_known_stressors(recipe: PixelRecipe) -> Vec<&'static str> {
    let mut stressors = vec!["minimal_secondary_capture"];
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
    } else if recipe.transfer_syntax == JPEG_XL_LOSSY {
        stressors.push("encapsulated_pixel_data");
        stressors.push("jpeg_xl_lossy_transfer_syntax");
        stressors.push("lossy_image_compression");
        stressors.push("external_command_codec");
    } else if recipe.transfer_syntax == JPEG_2000_LOSSLESS {
        stressors.push("encapsulated_pixel_data");
        stressors.push("jpeg_2000_lossless_transfer_syntax");
    } else if recipe.transfer_syntax == HTJ2K_LOSSLESS {
        stressors.push("encapsulated_pixel_data");
        stressors.push("htj2k_lossless_transfer_syntax");
    } else if recipe.transfer_syntax == HTJ2K_LOSSY {
        stressors.push("encapsulated_pixel_data");
        stressors.push("htj2k_lossy_transfer_syntax");
        stressors.push("lossy_image_compression");
        stressors.push("external_command_codec");
    } else if recipe.transfer_syntax == JPEG_LOSSLESS_PROCESS_14 {
        stressors.push("encapsulated_pixel_data");
        stressors.push("jpeg_lossless_process_14_transfer_syntax");
        stressors.push("external_command_codec");
    } else if recipe.transfer_syntax == JPEG_LOSSLESS_SV1 {
        stressors.push("encapsulated_pixel_data");
        stressors.push("jpeg_lossless_sv1_transfer_syntax");
        stressors.push("external_command_codec");
    } else if recipe.case_id == U32_SC_RECIPE.case_id {
        stressors.push("native_ow_pixel_data");
    } else {
        stressors.push("native_ob_pixel_data");
    }
    if recipe.transfer_syntax == EXPLICIT_VR_BIG_ENDIAN {
        stressors.push("retired_transfer_syntax");
        stressors.push("explicit_vr_big_endian_dataset");
    }
    if recipe.case_id == U1_SC_RECIPE.case_id {
        stressors.push("native_bit_packed_pixel_data");
        stressors.push("continuous_cross_frame_bit_packing");
        stressors.push("multi_frame_single_bit_secondary_capture");
        stressors.push("whole_value_field_even_length_padding");
    }
    if recipe.case_id == NONSQUARE_SPACING_SC_RECIPE.pixel.case_id {
        stressors.push("nonsquare_pixel_geometry");
        stressors.push("independent_spacing_and_aspect_ratio_axes");
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
    if recipe.case_id == EOT_CASE_ID {
        stressors.push("empty_basic_offset_table");
        stressors.push("extended_offset_table");
        stressors.push("one_fragment_per_frame");
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
        | "classic/sc/rgb_planar0_multiframe_rle_lossless"
        | "classic/sc/rgb_planar0_jpeg_baseline_8bit"
        | "classic/sc/mono2_u8_jpeg_ls_lossless"
        | "classic/sc/rgb_planar0_jpegxl_lossless"
        | "classic/sc/rgb_jpegxl_lossy"
        | "classic/sc/mono2_u16_jpeg2000_lossless"
        | "classic/sc/mono2_u16_htj2k_lossless"
        | "classic/sc/mono2_u16_htj2k_lossy"
        | "classic/sc/mono2_u16_jpeg_lossless_process_14"
        | "classic/sc/mono2_u16_jpeg_lossless_sv1"
        | "classic/sc/mono2_u32_explicit_le"
        | "classic/sc/mono2_u1_native"
        | EOT_CASE_ID => &["extended"],
        _ => &["core"],
    }
}

fn pixel_expected_capabilities(recipe: PixelRecipe) -> Vec<&'static str> {
    if recipe.case_id == NONSQUARE_SPACING_SC_RECIPE.pixel.case_id {
        return vec![
            "open_file",
            "read_metadata",
            "interpret_pixel_geometry",
            "render_native_pixels",
        ];
    }
    if recipe.case_id == U1_SC_RECIPE.case_id {
        return vec![
            "open_file",
            "read_metadata",
            "unpack_native_bit_packed_pixels",
            "render_native_pixels",
        ];
    }
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
    } else if recipe.transfer_syntax == JPEG_XL_LOSSY {
        vec![
            "open_file",
            "read_metadata",
            "decode_jpeg_xl_lossy_pixels",
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
    } else if recipe.transfer_syntax == HTJ2K_LOSSY {
        vec![
            "open_file",
            "read_metadata",
            "decode_htj2k_lossy_pixels",
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

fn pixel_sop_class_uid(recipe: PixelRecipe) -> &'static str {
    if recipe.case_id == U1_SC_RECIPE.case_id {
        uids::MULTI_FRAME_SINGLE_BIT_SECONDARY_CAPTURE_IMAGE_STORAGE
    } else if recipe.case_id == EOT_CASE_ID {
        uids::MULTI_FRAME_GRAYSCALE_BYTE_SECONDARY_CAPTURE_IMAGE_STORAGE
    } else {
        uids::SECONDARY_CAPTURE_IMAGE_STORAGE
    }
}

fn pixel_sop_class_name(recipe: PixelRecipe) -> &'static str {
    if recipe.case_id == U1_SC_RECIPE.case_id {
        "Multi-frame Single Bit Secondary Capture Image Storage"
    } else if recipe.case_id == EOT_CASE_ID {
        "Multi-frame Grayscale Byte Secondary Capture Image Storage"
    } else {
        "Secondary Capture Image Storage"
    }
}

fn pixel_iod_name(recipe: PixelRecipe) -> &'static str {
    if recipe.case_id == U1_SC_RECIPE.case_id {
        "Multi-frame Single Bit Secondary Capture Image"
    } else if recipe.case_id == EOT_CASE_ID {
        "Multi-frame Grayscale Byte Secondary Capture Image"
    } else {
        "Secondary Capture Image"
    }
}

fn pixel_determinism(recipe: PixelRecipe) -> &'static str {
    if recipe.transfer_syntax == JPEG_BASELINE_8BIT
        || recipe.transfer_syntax == JPEG_LS_LOSSLESS
        || recipe.transfer_syntax == JPEG_XL_LOSSLESS
        || recipe.transfer_syntax == JPEG_XL_LOSSY
        || recipe.transfer_syntax == JPEG_2000_LOSSLESS
        || recipe.transfer_syntax == HTJ2K_LOSSLESS
        || recipe.transfer_syntax == HTJ2K_LOSSY
        || recipe.transfer_syntax == JPEG_LOSSLESS_PROCESS_14
        || recipe.transfer_syntax == JPEG_LOSSLESS_SV1
    {
        "semantic_stable"
    } else {
        "byte_stable"
    }
}

fn pixel_recipe_frame_bytes(recipe: PixelRecipe) -> Result<Vec<&'static [u8]>, String> {
    if recipe.case_id == U1_SC_RECIPE.case_id {
        return Ok(U1_SC_RECIPE.decoded_frames());
    }
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
    let frame_of_reference_uid = deterministic_classic_ct_uid(
        standards_lock_sha256,
        recipe,
        run.seed,
        UidRole::FrameOfReference,
        0,
    );
    let implementation_class_uid = deterministic_implementation_uid(standards_lock_sha256);

    let series_recipes = classic_ct_series_recipes(recipe);
    let total_instance_count = series_recipes
        .iter()
        .map(|series| series.slices.len())
        .sum();
    let mut generated_files = Vec::with_capacity(total_instance_count);
    let mut file_index = 0_u32;
    for (series_index, series) in series_recipes.iter().enumerate() {
        let series_instance_uid = deterministic_classic_ct_uid(
            standards_lock_sha256,
            recipe,
            run.seed,
            UidRole::SeriesInstance,
            series_index as u32,
        );
        for (slice_index, slice) in series.slices.iter().enumerate() {
            let sop_instance_uid = deterministic_classic_ct_uid(
                standards_lock_sha256,
                recipe,
                run.seed,
                UidRole::SopInstance,
                file_index,
            );
            file_index += 1;
            let relative_path = if series_recipes.len() > 1 {
                format!(
                    "{}/series-{:03}/slice-{:03}.dcm",
                    recipe.case_id,
                    series_index + 1,
                    slice_index + 1
                )
            } else if series.slices.len() == 1 {
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
            put_str(&mut obj, tags::SERIES_NUMBER, VR::IS, series.series_number);
            if series.slices.len() > 1 {
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

            put_str(
                &mut obj,
                tags::ACQUISITION_NUMBER,
                VR::IS,
                series.acquisition_number,
            );
            put_str(&mut obj, tags::ACQUISITION_DATE, VR::DA, "20260101");
            put_str(&mut obj, tags::ACQUISITION_TIME, VR::TM, "000000");

            put_str(
                &mut obj,
                tags::IMAGE_TYPE,
                VR::CS,
                "ORIGINAL\\PRIMARY\\AXIAL",
            );
            match slice.instance_number {
                ClassicCtInstanceNumber::Numeric(value) => {
                    put_str(&mut obj, tags::INSTANCE_NUMBER, VR::IS, value);
                }
                ClassicCtInstanceNumber::Empty => {
                    obj.put(DataElement::empty(tags::INSTANCE_NUMBER, VR::IS));
                }
            }
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
            if let Some(gantry_detector_tilt_degrees) = recipe.gantry_detector_tilt_degrees {
                put_str(
                    &mut obj,
                    tags::GANTRY_DETECTOR_TILT,
                    VR::DS,
                    gantry_detector_tilt_degrees,
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

            materialize_curated_classic_dataset(
                &obj,
                &path,
                recipe.recipe_id,
                "classic/ct",
                uids::CT_IMAGE_STORAGE,
                recipe.transfer_syntax.uid,
                &study_instance_uid,
                &series_instance_uid,
                &sop_instance_uid,
                &implementation_class_uid,
            )?;

            let decoded_frame_hash = sha256_hex(slice.pixel_bytes);
            let decoded_frame_hashes = [decoded_frame_hash.as_str()];
            let mut validated = validate_part10_file(
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
                            basic_offset_table_offsets: encapsulated
                                .basic_offset_table
                                .offsets
                                .len(),
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
                        acquisition_number: series.acquisition_number,
                        rescale_intercept: recipe.rescale_intercept,
                        rescale_slope: recipe.rescale_slope,
                        rescale_type: recipe.rescale_type,
                        window_center: recipe.window_center,
                        window_width: recipe.window_width,
                    }),
                    enhanced_ct_image: None,
                    enhanced_mr_image: None,
                    enhanced_pet_image: None,
                    mg_image: None,
                    dx_image: None,
                    xa_image: None,
                    xrf_image: None,
                    us_image: None,
                    us_multiframe: None,
                    nm_image: None,
                    pet_image: None,
                    cr_image: None,
                    mr_image: None,
                    segmentation: None,
                },
            )?;
            append_curated_plan_validation(&mut validated.validation);

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
                    *series,
                    series_index,
                    series_recipes.len(),
                    compressed_pixel_data.as_ref(),
                ),
            });
        }
    }

    Ok(generated_files)
}

fn write_stress_high_instance_ct_case(
    run: &PreparedGenerationRun,
    case: &Value,
    standards_lock_sha256: &str,
) -> Result<Vec<GeneratedFile>, GenerateError> {
    const ROWS: u16 = 64;
    const COLUMNS: u16 = 64;
    const INSTANCES: usize = 128;
    let sample_count = usize::from(ROWS) * usize::from(COLUMNS);
    let mut pixel_bytes = Vec::with_capacity(sample_count * 2);
    let mut pixel_values = Vec::with_capacity(sample_count);
    for index in 0..sample_count {
        let value = (index % 3072) as i16 - 1024;
        pixel_bytes.extend_from_slice(&value.to_le_bytes());
        pixel_values.push(i32::from(value));
    }
    let pixel_bytes: &'static [u8] = Box::leak(pixel_bytes.into_boxed_slice());
    let pixel_values: &'static [i32] = Box::leak(pixel_values.into_boxed_slice());
    let slices = (0..INSTANCES)
        .map(|index| {
            let instance_number: &'static str = Box::leak((index + 1).to_string().into_boxed_str());
            let position: &'static str =
                Box::leak(format!("0\\0\\{}", index as f64 * 2.5).into_boxed_str());
            ClassicCtSliceRecipe {
                instance_number: ClassicCtInstanceNumber::Numeric(instance_number),
                image_position_patient: position,
                position_along_normal: index as f64 * 2.5,
                pixel_bytes,
                pixel_values,
                pixel_min: -1024,
                pixel_max: 2047,
            }
        })
        .collect::<Vec<_>>();
    let recipe = ClassicCtRecipe {
        case_id: STRESS_HIGH_INSTANCE_CT_CASE_ID,
        recipe_id: "stress_high_instance_count_ct",
        transfer_syntax: EXPLICIT_VR_LITTLE_ENDIAN,
        rows: ROWS,
        columns: COLUMNS,
        slices: Box::leak(slices.into_boxed_slice()),
        series: &[],
        series_organization: None,
        rescale_intercept: "-1024",
        rescale_slope: "1",
        rescale_type: "HU",
        window_center: "40",
        window_width: "400",
        pixel_spacing: "0.75\\0.75",
        image_orientation_patient: "1\\0\\0\\0\\1\\0",
        slice_thickness: "2.5",
        spacing_between_slices: Some("2.5"),
        gantry_detector_tilt_degrees: None,
        sorting_conflict_expected: Some(false),
        kvp: "120",
    };
    write_classic_ct_case(run, case, recipe, standards_lock_sha256)
}

struct StressScIdentity {
    study_instance_uid: String,
    series_instance_uid: String,
    sop_instance_uid: String,
    implementation_class_uid: String,
}

fn stress_sc_uid(
    standards_lock_sha256: &str,
    case_id: &str,
    run_seed: u64,
    role: UidRole,
) -> String {
    deterministic_uid(&DeterministicUidInput {
        standards_lock_sha256,
        case_id,
        recipe_version: "0.1.0",
        run_seed,
        file_index: 0,
        frame_index: None,
        referenced_object_index: None,
        role,
    })
}

fn base_stress_sc_object(
    run: &PreparedGenerationRun,
    case_id: &str,
    recipe_id: &str,
    standards_lock_sha256: &str,
) -> (InMemDicomObject, StressScIdentity) {
    let identity = StressScIdentity {
        study_instance_uid: stress_sc_uid(
            standards_lock_sha256,
            case_id,
            run.seed,
            UidRole::StudyInstance,
        ),
        series_instance_uid: stress_sc_uid(
            standards_lock_sha256,
            case_id,
            run.seed,
            UidRole::SeriesInstance,
        ),
        sop_instance_uid: stress_sc_uid(
            standards_lock_sha256,
            case_id,
            run.seed,
            UidRole::SopInstance,
        ),
        implementation_class_uid: deterministic_implementation_uid(standards_lock_sha256),
    };
    let mut obj = InMemDicomObject::new_empty();
    put_str(
        &mut obj,
        tags::SOP_CLASS_UID,
        VR::UI,
        uids::SECONDARY_CAPTURE_IMAGE_STORAGE,
    );
    put_str(
        &mut obj,
        tags::SOP_INSTANCE_UID,
        VR::UI,
        &identity.sop_instance_uid,
    );
    put_str(&mut obj, tags::SYNTHETIC_DATA, VR::CS, "YES");
    put_str(&mut obj, tags::PATIENT_NAME, VR::PN, "DICOMTEST^STRESS");
    put_str(&mut obj, tags::PATIENT_ID, VR::LO, "DICOMTEST-STRESS-001");
    put_str(&mut obj, tags::PATIENT_BIRTH_DATE, VR::DA, "19700101");
    put_str(&mut obj, tags::PATIENT_SEX, VR::CS, "O");
    put_str(
        &mut obj,
        tags::STUDY_INSTANCE_UID,
        VR::UI,
        &identity.study_instance_uid,
    );
    put_str(&mut obj, tags::STUDY_DATE, VR::DA, "20260101");
    put_str(&mut obj, tags::STUDY_TIME, VR::TM, "000000");
    put_str(&mut obj, tags::REFERRING_PHYSICIAN_NAME, VR::PN, "");
    put_str(&mut obj, tags::STUDY_ID, VR::SH, "DTS-STRESS");
    put_str(&mut obj, tags::ACCESSION_NUMBER, VR::SH, "");
    put_str(&mut obj, tags::MODALITY, VR::CS, "OT");
    put_str(
        &mut obj,
        tags::SERIES_INSTANCE_UID,
        VR::UI,
        &identity.series_instance_uid,
    );
    put_str(&mut obj, tags::SERIES_NUMBER, VR::IS, "1");
    put_str(&mut obj, tags::LATERALITY, VR::CS, "");
    put_str(&mut obj, tags::CONVERSION_TYPE, VR::CS, "SYN");
    put_str(&mut obj, tags::MANUFACTURER, VR::LO, "dicom-test-suite");
    put_str(&mut obj, tags::MANUFACTURER_MODEL_NAME, VR::LO, recipe_id);
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
    (obj, identity)
}

fn write_stress_large_bulk_case(
    run: &PreparedGenerationRun,
    case: &Value,
    standards_lock_sha256: &str,
) -> Result<GeneratedFile, GenerateError> {
    const ROWS: u16 = 8192;
    const COLUMNS: u16 = 4096;
    const PAYLOAD_BYTES: usize = 64 * 1024 * 1024;
    const RECIPE_ID: &str = "stress_sc_large_bulk_data";
    let relative_path = format!("{STRESS_LARGE_BULK_CASE_ID}/instance.dcm");
    let path = run.out_dir.join(&relative_path);
    fs::create_dir_all(path.parent().expect("stress path has parent")).map_err(|source| {
        GenerateError::CreateCaseOutputDir {
            path: path.parent().unwrap().to_path_buf(),
            source,
        }
    })?;
    let (mut obj, identity) = base_stress_sc_object(
        run,
        STRESS_LARGE_BULK_CASE_ID,
        RECIPE_ID,
        standards_lock_sha256,
    );
    put_u16(&mut obj, tags::SAMPLES_PER_PIXEL, VR::US, 1);
    put_str(
        &mut obj,
        tags::PHOTOMETRIC_INTERPRETATION,
        VR::CS,
        "MONOCHROME2",
    );
    put_u16(&mut obj, tags::ROWS, VR::US, ROWS);
    put_u16(&mut obj, tags::COLUMNS, VR::US, COLUMNS);
    put_u16(&mut obj, tags::BITS_ALLOCATED, VR::US, 16);
    put_u16(&mut obj, tags::BITS_STORED, VR::US, 16);
    put_u16(&mut obj, tags::HIGH_BIT, VR::US, 15);
    put_u16(&mut obj, tags::PIXEL_REPRESENTATION, VR::US, 0);
    let pixel_bytes = vec![0_u8; PAYLOAD_BYTES];
    let pixel_sha256 = sha256_hex(&pixel_bytes);
    obj.put(DataElement::new(
        tags::PIXEL_DATA,
        VR::OW,
        PrimitiveValue::from(pixel_bytes),
    ));
    let file_obj = obj
        .with_meta(
            FileMetaTableBuilder::new()
                .transfer_syntax(EXPLICIT_VR_LITTLE_ENDIAN.uid)
                .implementation_class_uid(&identity.implementation_class_uid)
                .implementation_version_name(crate::IMPLEMENTATION_VERSION_NAME),
        )
        .map_err(|error| GenerateError::WriteDicomFile {
            path: path.clone(),
            message: error.to_string(),
        })?;
    file_obj
        .write_to_file(&path)
        .map_err(|error| GenerateError::WriteDicomFile {
            path: path.clone(),
            message: error.to_string(),
        })?;
    let validated = validate_part10_file(
        &path,
        &Part10Expectations {
            sop_class_uid: uids::SECONDARY_CAPTURE_IMAGE_STORAGE,
            sop_instance_uid: &identity.sop_instance_uid,
            transfer_syntax_uid: EXPLICIT_VR_LITTLE_ENDIAN.uid,
            implementation_class_uid: &identity.implementation_class_uid,
            synthetic_data: "YES",
            rows: ROWS,
            columns: COLUMNS,
            frames: 1,
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
            enhanced_mr_image: None,
            enhanced_pet_image: None,
            mg_image: None,
            dx_image: None,
            xa_image: None,
            xrf_image: None,
            us_image: None,
            us_multiframe: None,
            nm_image: None,
            pet_image: None,
            cr_image: None,
            mr_image: None,
            segmentation: None,
        },
    )?;
    Ok(GeneratedFile {
        case_id: STRESS_LARGE_BULK_CASE_ID.to_string(),
        manifest_entry: serde_json::json!({
            "case_id": STRESS_LARGE_BULK_CASE_ID,
            "profile_membership": ["stress"],
            "path": relative_path,
            "sha256": sha256_hex(&validated.bytes),
            "size_bytes": validated.bytes.len(),
            "determinism": "byte_stable",
            "recipe": {
                "recipe_id": RECIPE_ID,
                "recipe_version": "0.1.0",
                "recipe_parameters": {"rows": ROWS, "columns": COLUMNS, "payload_bytes": PAYLOAD_BYTES}
            },
            "dicom": {
                "sop_class_uid": uids::SECONDARY_CAPTURE_IMAGE_STORAGE,
                "sop_class_name": "Secondary Capture Image Storage",
                "iod_name": "Secondary Capture Image",
                "modality": "OT",
                "transfer_syntax_uid": EXPLICIT_VR_LITTLE_ENDIAN.uid,
                "transfer_syntax_name": EXPLICIT_VR_LITTLE_ENDIAN.name
            },
            "uids": {
                "study_instance_uid": identity.study_instance_uid,
                "series_instance_uid": identity.series_instance_uid,
                "sop_instance_uid": identity.sop_instance_uid,
                "implementation_class_uid": identity.implementation_class_uid
            },
            "image": {
                "rows": ROWS, "columns": COLUMNS, "frames": 1,
                "samples_per_pixel": 1, "photometric_interpretation": "MONOCHROME2",
                "bits_allocated": 16, "bits_stored": 16, "high_bit": 15,
                "pixel_representation": 0, "planar_configuration": Value::Null
            },
            "pixel_data": {
                "vr": "OW", "native_or_encapsulated": "native", "value_length": PAYLOAD_BYTES,
                "frame_count": 1, "frame_hashes": [pixel_sha256]
            },
            "references": [],
            "expected_capabilities": ["open_file", "read_metadata", "render_native_pixels", "stream_large_bulk_data"],
            "expected_semantics": {"synthetic_data": "YES", "conversion_type": "SYN", "pixel_min": 0, "pixel_max": 0},
            "expected_visual_checks": {"pattern": "uniform_zero_reduced_64_mib_native_pixel_data"},
            "validation": validated.validation,
            "known_stressors": ["reduced_stress_scale", "large_native_bulk_data", "64_mib_pixel_data"],
            "standards_evidence": deduplicated_standards_evidence(standards_evidence_from_case(case))
        }),
    })
}

#[allow(clippy::too_many_arguments)]
fn finish_stress_tiny_sc_case(
    run: &PreparedGenerationRun,
    case: &Value,
    case_id: &str,
    recipe_id: &str,
    mut obj: InMemDicomObject,
    identity: StressScIdentity,
    recipe_parameters: Value,
    expected_semantics: Value,
    visual_pattern: &str,
    known_stressors: &[&str],
) -> Result<GeneratedFile, GenerateError> {
    const PIXELS: &[u8] = &[0, 85, 170, 255];
    put_u16(&mut obj, tags::SAMPLES_PER_PIXEL, VR::US, 1);
    put_str(
        &mut obj,
        tags::PHOTOMETRIC_INTERPRETATION,
        VR::CS,
        "MONOCHROME2",
    );
    put_u16(&mut obj, tags::ROWS, VR::US, 2);
    put_u16(&mut obj, tags::COLUMNS, VR::US, 2);
    put_u16(&mut obj, tags::BITS_ALLOCATED, VR::US, 8);
    put_u16(&mut obj, tags::BITS_STORED, VR::US, 8);
    put_u16(&mut obj, tags::HIGH_BIT, VR::US, 7);
    put_u16(&mut obj, tags::PIXEL_REPRESENTATION, VR::US, 0);
    obj.put(DataElement::new(
        tags::PIXEL_DATA,
        VR::OB,
        PrimitiveValue::from(PIXELS),
    ));
    let relative_path = format!("{case_id}/instance.dcm");
    let path = run.out_dir.join(&relative_path);
    fs::create_dir_all(path.parent().expect("stress path has parent")).map_err(|source| {
        GenerateError::CreateCaseOutputDir {
            path: path.parent().unwrap().to_path_buf(),
            source,
        }
    })?;
    let file_obj = obj
        .with_meta(
            FileMetaTableBuilder::new()
                .transfer_syntax(EXPLICIT_VR_LITTLE_ENDIAN.uid)
                .implementation_class_uid(&identity.implementation_class_uid)
                .implementation_version_name(crate::IMPLEMENTATION_VERSION_NAME),
        )
        .map_err(|error| GenerateError::WriteDicomFile {
            path: path.clone(),
            message: error.to_string(),
        })?;
    file_obj
        .write_to_file(&path)
        .map_err(|error| GenerateError::WriteDicomFile {
            path: path.clone(),
            message: error.to_string(),
        })?;
    let validated = validate_part10_file(
        &path,
        &Part10Expectations {
            sop_class_uid: uids::SECONDARY_CAPTURE_IMAGE_STORAGE,
            sop_instance_uid: &identity.sop_instance_uid,
            transfer_syntax_uid: EXPLICIT_VR_LITTLE_ENDIAN.uid,
            implementation_class_uid: &identity.implementation_class_uid,
            synthetic_data: "YES",
            rows: 2,
            columns: 2,
            frames: 1,
            samples_per_pixel: 1,
            photometric_interpretation: "MONOCHROME2",
            bits_allocated: 8,
            bits_stored: 8,
            high_bit: 7,
            pixel_representation: 0,
            planar_configuration: None,
            pixel_data_vr: VR::OB,
            pixel_data_length_formula: PixelDataLengthFormula::ContiguousSamples,
            decoded_frame_hashes: &[],
            palette: None,
            padding: None,
            ct_image: None,
            enhanced_ct_image: None,
            enhanced_mr_image: None,
            enhanced_pet_image: None,
            mg_image: None,
            dx_image: None,
            xa_image: None,
            xrf_image: None,
            us_image: None,
            us_multiframe: None,
            nm_image: None,
            pet_image: None,
            cr_image: None,
            mr_image: None,
            segmentation: None,
        },
    )?;
    Ok(GeneratedFile {
        case_id: case_id.to_string(),
        manifest_entry: serde_json::json!({
            "case_id": case_id,
            "profile_membership": ["stress"],
            "path": relative_path,
            "sha256": sha256_hex(&validated.bytes),
            "size_bytes": validated.bytes.len(),
            "determinism": "byte_stable",
            "recipe": {"recipe_id": recipe_id, "recipe_version": "0.1.0", "recipe_parameters": recipe_parameters},
            "dicom": {
                "sop_class_uid": uids::SECONDARY_CAPTURE_IMAGE_STORAGE,
                "sop_class_name": "Secondary Capture Image Storage",
                "iod_name": "Secondary Capture Image",
                "modality": "OT",
                "transfer_syntax_uid": EXPLICIT_VR_LITTLE_ENDIAN.uid,
                "transfer_syntax_name": EXPLICIT_VR_LITTLE_ENDIAN.name
            },
            "uids": {
                "study_instance_uid": identity.study_instance_uid,
                "series_instance_uid": identity.series_instance_uid,
                "sop_instance_uid": identity.sop_instance_uid,
                "implementation_class_uid": identity.implementation_class_uid
            },
            "image": {
                "rows": 2, "columns": 2, "frames": 1, "samples_per_pixel": 1,
                "photometric_interpretation": "MONOCHROME2", "bits_allocated": 8,
                "bits_stored": 8, "high_bit": 7, "pixel_representation": 0,
                "planar_configuration": Value::Null
            },
            "pixel_data": {
                "vr": "OB", "native_or_encapsulated": "native", "value_length": PIXELS.len(),
                "frame_count": 1, "frame_hashes": [sha256_hex(PIXELS)]
            },
            "references": [],
            "expected_capabilities": ["open_file", "read_metadata", "render_native_pixels", "bounded_metadata_traversal"],
            "expected_semantics": expected_semantics,
            "expected_visual_checks": {"pattern": visual_pattern},
            "validation": validated.validation,
            "known_stressors": known_stressors,
            "standards_evidence": deduplicated_standards_evidence(standards_evidence_from_case(case))
        }),
    })
}

fn write_stress_deep_nested_case(
    run: &PreparedGenerationRun,
    case: &Value,
    standards_lock_sha256: &str,
) -> Result<GeneratedFile, GenerateError> {
    const DEPTH: usize = 32;
    const PAYLOAD_BYTES: usize = 16 * 1024 * 1024;
    const RECIPE_ID: &str = "stress_sc_deep_nested_sequences";
    let (mut obj, identity) = base_stress_sc_object(
        run,
        STRESS_DEEP_NESTED_CASE_ID,
        RECIPE_ID,
        standards_lock_sha256,
    );
    let mut item = InMemDicomObject::from_element_iter([
        DataElement::new(Tag(0x7777, 0x0010), VR::LO, "DTS_STRESS_NESTED"),
        DataElement::new(
            Tag(0x7777, 0x1001),
            VR::OB,
            PrimitiveValue::from(vec![0x5a_u8; PAYLOAD_BYTES]),
        ),
    ]);
    for _ in 1..DEPTH {
        item = InMemDicomObject::from_element_iter([
            DataElement::new(Tag(0x7777, 0x0010), VR::LO, "DTS_STRESS_NESTED"),
            DataElement::new(
                Tag(0x7777, 0x1002),
                VR::SQ,
                DataSetSequence::from(vec![item]),
            ),
        ]);
    }
    obj.put(DataElement::new(
        Tag(0x7777, 0x1002),
        VR::SQ,
        DataSetSequence::from(vec![item]),
    ));
    finish_stress_tiny_sc_case(
        run,
        case,
        STRESS_DEEP_NESTED_CASE_ID,
        RECIPE_ID,
        obj,
        identity,
        serde_json::json!({"sequence_depth": DEPTH, "payload_bytes": PAYLOAD_BYTES}),
        serde_json::json!({"synthetic_data": "YES", "conversion_type": "SYN", "sequence_depth": DEPTH, "nested_payload_bytes": PAYLOAD_BYTES}),
        "tiny_gradient_with_32_level_private_sequence",
        &[
            "reduced_stress_scale",
            "deep_nested_sequences",
            "16_mib_nested_bulk_value",
        ],
    )
}

fn write_stress_long_metadata_case(
    run: &PreparedGenerationRun,
    case: &Value,
    standards_lock_sha256: &str,
) -> Result<GeneratedFile, GenerateError> {
    const VALUE_COUNT: usize = 1024;
    const VALUE_BYTES: usize = 1024;
    const RECIPE_ID: &str = "stress_sc_long_value_metadata";
    let (mut obj, identity) = base_stress_sc_object(
        run,
        STRESS_LONG_METADATA_CASE_ID,
        RECIPE_ID,
        standards_lock_sha256,
    );
    let value = "M".repeat(VALUE_BYTES);
    for block in 0..4_u16 {
        obj.put(DataElement::new(
            Tag(0x7777, 0x0010 + block),
            VR::LO,
            format!("DTS_STRESS_LONG_{block}"),
        ));
        for element in 0..256_u16 {
            obj.put(DataElement::new(
                Tag(0x7777, ((0x10 + block) << 8) | element),
                VR::UT,
                value.as_str(),
            ));
        }
    }
    finish_stress_tiny_sc_case(
        run,
        case,
        STRESS_LONG_METADATA_CASE_ID,
        RECIPE_ID,
        obj,
        identity,
        serde_json::json!({"metadata_values": VALUE_COUNT, "metadata_value_bytes": VALUE_BYTES, "payload_bytes": VALUE_COUNT * VALUE_BYTES}),
        serde_json::json!({"synthetic_data": "YES", "conversion_type": "SYN", "metadata_values": VALUE_COUNT, "metadata_total_value_bytes": VALUE_COUNT * VALUE_BYTES}),
        "tiny_gradient_with_1024_private_ut_values",
        &[
            "reduced_stress_scale",
            "long_value_metadata",
            "1024_private_ut_values",
            "1_mib_metadata_values",
        ],
    )
}

fn write_stress_encapsulated_case(
    run: &PreparedGenerationRun,
    case: &Value,
    standards_lock_sha256: &str,
) -> Result<GeneratedFile, GenerateError> {
    const ROWS: u16 = 512;
    const COLUMNS: u16 = 512;
    const FRAMES: usize = 256;
    const FRAGMENTS_PER_FRAME: usize = 64;
    const RECIPE_ID: &str = "stress_sc_large_encapsulated_multifragment";
    let relative_path = format!("{STRESS_ENCAPSULATED_CASE_ID}/instance.dcm");
    let path = run.out_dir.join(&relative_path);
    fs::create_dir_all(path.parent().expect("stress path has parent")).map_err(|source| {
        GenerateError::CreateCaseOutputDir {
            path: path.parent().unwrap().to_path_buf(),
            source,
        }
    })?;
    let (mut obj, identity) = base_stress_sc_object(
        run,
        STRESS_ENCAPSULATED_CASE_ID,
        RECIPE_ID,
        standards_lock_sha256,
    );
    put_str(
        &mut obj,
        tags::SOP_CLASS_UID,
        VR::UI,
        uids::MULTI_FRAME_GRAYSCALE_BYTE_SECONDARY_CAPTURE_IMAGE_STORAGE,
    );
    put_str(
        &mut obj,
        tags::ACQUISITION_DATE_TIME,
        VR::DT,
        "20260101000000",
    );
    put_str(&mut obj, tags::ACQUISITION_NUMBER, VR::IS, "1");
    put_str(&mut obj, tags::LATERALITY, VR::CS, "");
    obj.remove_element(tags::BODY_PART_EXAMINED);
    put_str(&mut obj, tags::BURNED_IN_ANNOTATION, VR::CS, "NO");
    put_str(&mut obj, tags::LOSSY_IMAGE_COMPRESSION, VR::CS, "00");
    put_u16(&mut obj, tags::SAMPLES_PER_PIXEL, VR::US, 1);
    put_str(
        &mut obj,
        tags::PHOTOMETRIC_INTERPRETATION,
        VR::CS,
        "MONOCHROME2",
    );
    put_u16(&mut obj, tags::ROWS, VR::US, ROWS);
    put_u16(&mut obj, tags::COLUMNS, VR::US, COLUMNS);
    put_str(
        &mut obj,
        tags::NUMBER_OF_FRAMES,
        VR::IS,
        &FRAMES.to_string(),
    );
    obj.put(DataElement::new(
        tags::FRAME_INCREMENT_POINTER,
        VR::AT,
        PrimitiveValue::Tags(vec![tags::PAGE_NUMBER_VECTOR].into()),
    ));
    put_str(
        &mut obj,
        tags::PAGE_NUMBER_VECTOR,
        VR::IS,
        &(1..=FRAMES)
            .map(|value| value.to_string())
            .collect::<Vec<_>>()
            .join("\\"),
    );
    put_u16(&mut obj, tags::BITS_ALLOCATED, VR::US, 8);
    put_u16(&mut obj, tags::BITS_STORED, VR::US, 8);
    put_u16(&mut obj, tags::HIGH_BIT, VR::US, 7);
    put_u16(&mut obj, tags::PIXEL_REPRESENTATION, VR::US, 0);
    put_str(&mut obj, tags::RESCALE_INTERCEPT, VR::DS, "0");
    put_str(&mut obj, tags::RESCALE_SLOPE, VR::DS, "1");
    put_str(&mut obj, tags::RESCALE_TYPE, VR::LO, "US");
    put_str(&mut obj, tags::PRESENTATION_LUT_SHAPE, VR::CS, "IDENTITY");

    let encoder = NativeRleLosslessEncoder::new();
    let mut compressed_frames = Vec::with_capacity(FRAMES);
    let mut decoded_frame_hashes = Vec::with_capacity(FRAMES);
    for frame_index in 0..FRAMES {
        let native = (0..usize::from(ROWS) * usize::from(COLUMNS))
            .map(|index| {
                ((index.wrapping_mul(37) + frame_index.wrapping_mul(17)) ^ (index >> 8)) as u8
            })
            .collect::<Vec<_>>();
        decoded_frame_hashes.push(sha256_hex(&native));
        compressed_frames.push(
            encoder
                .encode_frame(FrameEncodeInput {
                    native_frame: &native,
                    rows: ROWS,
                    columns: COLUMNS,
                    samples_per_pixel: 1,
                    bits_allocated: 8,
                    bits_stored: 8,
                    photometric_interpretation: "MONOCHROME2",
                })
                .map_err(|error| GenerateError::WriteDicomFile {
                    path: path.clone(),
                    message: error.to_string(),
                })?
                .bytes,
        );
    }
    let compressed_bytes = compressed_frames.iter().map(Vec::len).sum::<usize>();
    let fragments_per_frame = vec![FRAGMENTS_PER_FRAME; FRAMES];
    let encapsulated = crate::encapsulation::encapsulate_frames(
        &compressed_frames,
        &fragments_per_frame,
        BasicOffsetTablePolicy::Empty,
    )
    .map_err(|error| GenerateError::WriteDicomFile {
        path: path.clone(),
        message: error.to_string(),
    })?;
    let first_fragment_item_start = encapsulated.fragments[0].item_start_offset;
    let mut eot_offsets = Vec::with_capacity(FRAMES);
    let mut eot_lengths = Vec::with_capacity(FRAMES);
    for frame_index in 0..FRAMES {
        let frame_fragments = encapsulated
            .fragments
            .iter()
            .filter(|fragment| fragment.frame_index == frame_index)
            .collect::<Vec<_>>();
        eot_offsets.push(u64::from(
            frame_fragments[0].item_start_offset - first_fragment_item_start,
        ));
        eot_lengths.push(
            frame_fragments
                .iter()
                .map(|fragment| fragment.compressed_length as u64)
                .sum::<u64>(),
        );
    }
    obj.put(DataElement::new(
        tags::EXTENDED_OFFSET_TABLE,
        VR::OV,
        PrimitiveValue::U8(
            crate::encapsulation::serialize_ov_words_little_endian(&eot_offsets).into(),
        ),
    ));
    obj.put(DataElement::new(
        tags::EXTENDED_OFFSET_TABLE_LENGTHS,
        VR::OV,
        PrimitiveValue::U8(
            crate::encapsulation::serialize_ov_words_little_endian(&eot_lengths).into(),
        ),
    ));
    obj.put(DataElement::new(
        tags::PIXEL_DATA,
        VR::OB,
        PixelFragmentSequence::new(Vec::new(), encapsulated.fragment_payloads.clone()),
    ));
    let file_obj = obj
        .with_meta(
            FileMetaTableBuilder::new()
                .transfer_syntax(RLE_LOSSLESS.uid)
                .implementation_class_uid(&identity.implementation_class_uid)
                .implementation_version_name(crate::IMPLEMENTATION_VERSION_NAME),
        )
        .map_err(|error| GenerateError::WriteDicomFile {
            path: path.clone(),
            message: error.to_string(),
        })?;
    file_obj
        .write_to_file(&path)
        .map_err(|error| GenerateError::WriteDicomFile {
            path: path.clone(),
            message: error.to_string(),
        })?;
    let decoded_frame_hash_refs = decoded_frame_hashes
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    let validated = validate_part10_file(
        &path,
        &Part10Expectations {
            sop_class_uid: uids::MULTI_FRAME_GRAYSCALE_BYTE_SECONDARY_CAPTURE_IMAGE_STORAGE,
            sop_instance_uid: &identity.sop_instance_uid,
            transfer_syntax_uid: RLE_LOSSLESS.uid,
            implementation_class_uid: &identity.implementation_class_uid,
            synthetic_data: "YES",
            rows: ROWS,
            columns: COLUMNS,
            frames: FRAMES as u16,
            samples_per_pixel: 1,
            photometric_interpretation: "MONOCHROME2",
            bits_allocated: 8,
            bits_stored: 8,
            high_bit: 7,
            pixel_representation: 0,
            planar_configuration: None,
            pixel_data_vr: VR::OB,
            pixel_data_length_formula: PixelDataLengthFormula::Encapsulated {
                fragments: FRAMES * FRAGMENTS_PER_FRAME,
                basic_offset_table_offsets: 0,
            },
            decoded_frame_hashes: &decoded_frame_hash_refs,
            palette: None,
            padding: None,
            ct_image: None,
            enhanced_ct_image: None,
            enhanced_mr_image: None,
            enhanced_pet_image: None,
            mg_image: None,
            dx_image: None,
            xa_image: None,
            xrf_image: None,
            us_image: None,
            us_multiframe: None,
            nm_image: None,
            pet_image: None,
            cr_image: None,
            mr_image: None,
            segmentation: None,
        },
    )?;
    let backend = FrameEncoder::backend(&encoder);
    Ok(GeneratedFile {
        case_id: STRESS_ENCAPSULATED_CASE_ID.to_string(),
        manifest_entry: serde_json::json!({
            "case_id": STRESS_ENCAPSULATED_CASE_ID,
            "profile_membership": ["stress"],
            "path": relative_path,
            "sha256": sha256_hex(&validated.bytes),
            "size_bytes": validated.bytes.len(),
            "determinism": "byte_stable",
            "recipe": {"recipe_id": RECIPE_ID, "recipe_version": "0.1.0", "recipe_parameters": {
                "rows": ROWS, "columns": COLUMNS, "frames": FRAMES,
                "fragments_per_frame": FRAGMENTS_PER_FRAME,
                "fragment_count": FRAMES * FRAGMENTS_PER_FRAME,
                "native_payload_bytes": FRAMES * usize::from(ROWS) * usize::from(COLUMNS),
                "compressed_payload_bytes": compressed_bytes
            }},
            "dicom": {
                "sop_class_uid": uids::MULTI_FRAME_GRAYSCALE_BYTE_SECONDARY_CAPTURE_IMAGE_STORAGE,
                "sop_class_name": "Multi-frame Grayscale Byte Secondary Capture Image Storage",
                "iod_name": "Multi-frame Grayscale Byte Secondary Capture Image",
                "modality": "OT", "transfer_syntax_uid": RLE_LOSSLESS.uid,
                "transfer_syntax_name": RLE_LOSSLESS.name
            },
            "uids": {
                "study_instance_uid": identity.study_instance_uid,
                "series_instance_uid": identity.series_instance_uid,
                "sop_instance_uid": identity.sop_instance_uid,
                "implementation_class_uid": identity.implementation_class_uid
            },
            "image": {"rows": ROWS, "columns": COLUMNS, "frames": FRAMES, "samples_per_pixel": 1,
                "photometric_interpretation": "MONOCHROME2", "bits_allocated": 8,
                "bits_stored": 8, "high_bit": 7, "pixel_representation": 0,
                "planar_configuration": Value::Null},
            "pixel_data": {"vr": "OB", "native_or_encapsulated": "encapsulated", "value_length": Value::Null,
                "frame_count": FRAMES, "frame_hashes": decoded_frame_hashes,
                "codec": {"backend_id": backend.backend_id, "backend_kind": backend.backend_kind.as_str(),
                    "display_name": backend.display_name, "version": backend.version,
                    "transfer_syntax_uid": backend.transfer_syntax_uid, "feature_gate": backend.feature_gate,
                    "determinism": backend.determinism.as_str()},
                "encapsulated_pixel_data": {
                    "basic_offset_table": {"present": true, "populated": false, "offset_count": 0, "offsets": []},
                    "fragments_per_frame": fragments_per_frame,
                    "extended_offset_table": {"present": true, "lengths_present": true,
                        "offset_count": FRAMES, "length_count": FRAMES,
                        "offsets": eot_offsets, "lengths": eot_lengths},
                    "compressed_frame_hashes": encapsulated.compressed_frame_hashes
                }
            },
            "references": [],
            "expected_capabilities": ["open_file", "read_metadata", "decode_rle_lossless_pixels", "parse_extended_offset_table", "stream_multifragment_pixel_data"],
            "expected_semantics": {"synthetic_data": "YES", "conversion_type": "SYN", "pixel_min": 0, "pixel_max": 255},
            "expected_visual_checks": {"pattern": "256_deterministic_pseudorandom_monochrome_frames"},
            "validation": validated.validation,
            "known_stressors": ["reduced_stress_scale", "large_encapsulated_pixel_data", "multi_fragment_frames", "extended_offset_table", "empty_basic_offset_table"],
            "standards_evidence": deduplicated_standards_evidence(standards_evidence_from_case(case))
        }),
    })
}

fn classic_ct_series_recipes(recipe: ClassicCtRecipe) -> Vec<ClassicCtSeriesRecipe> {
    if recipe.series.is_empty() {
        vec![ClassicCtSeriesRecipe {
            series_number: "1",
            acquisition_number: "1",
            slices: recipe.slices,
        }]
    } else {
        recipe.series.to_vec()
    }
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
    series: ClassicCtSeriesRecipe,
    series_index: usize,
    study_series_count: usize,
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
    if series.slices.len() > 1 {
        let geometry = recipe_geometry
            .as_object_mut()
            .expect("CT recipe geometry must be an object");
        if let Some(spacing_between_slices) = recipe.spacing_between_slices {
            geometry.insert(
                "spacing_between_slices".to_string(),
                Value::from(spacing_between_slices),
            );
        }
        if let Some(gantry_detector_tilt_degrees) = recipe.gantry_detector_tilt_degrees {
            geometry.insert(
                "gantry_detector_tilt_degrees".to_string(),
                Value::from(classic_ct_ds_number(gantry_detector_tilt_degrees)),
            );
        }
        geometry.insert(
            "position_along_normal".to_string(),
            Value::from(slice.position_along_normal),
        );
        geometry.insert(
            "slice_order_index".to_string(),
            Value::from(slice_index + 1),
        );
        geometry.insert("slice_count".to_string(), Value::from(series.slices.len()));
        geometry.insert("series_ordinal".to_string(), Value::from(series_index + 1));
        geometry.insert(
            "study_series_count".to_string(),
            Value::from(study_series_count),
        );

        let semantics = expected_semantics
            .as_object_mut()
            .expect("CT expected semantics must be an object");
        semantics.insert(
            "series_instance_count".to_string(),
            Value::from(series.slices.len()),
        );
        semantics.insert(
            "shared_study_series_frame_of_reference".to_string(),
            Value::Bool(true),
        );
        if recipe.series_organization.is_some() {
            semantics.insert(
                "study_series_count".to_string(),
                Value::from(study_series_count),
            );
            semantics.insert("series_ordinal".to_string(), Value::from(series_index + 1));
        }
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
                "acquisition_number": series.acquisition_number,
                "series_number": series.series_number,
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
    if series.slices.len() > 1 {
        let mut expected_geometry = serde_json::json!({
            "sort_basis": "image_position_patient_projected_on_slice_normal",
            "sort_direction": "ascending",
            "position_tolerance_mm": 0.00001,
            "spacing_tolerance_mm": 0.00001,
            "series_instance_count": series.slices.len(),
            "geometric_order_index": slice_index + 1,
            "position_along_normal_mm": slice.position_along_normal,
            "image_position_patient": classic_ct_ds_values::<3>(slice.image_position_patient),
            "image_orientation_patient": classic_ct_ds_values::<6>(recipe.image_orientation_patient),
            "adjacent_spacing_mm": classic_ct_adjacent_spacing(series.slices),
            "spacing_uniform": classic_ct_spacing_is_uniform(series.slices),
            "instance_number_state": classic_ct_instance_number_state(slice.instance_number),
            "instance_number": classic_ct_instance_number_value(slice.instance_number),
            "instance_number_order_index": classic_ct_instance_number_order_index(series.slices, slice.instance_number),
            "sorting_conflict_expected": recipe.sorting_conflict_expected
        });
        if let Some(gantry_detector_tilt_degrees) = recipe.gantry_detector_tilt_degrees {
            expected_geometry
                .as_object_mut()
                .expect("CT expected geometry must be an object")
                .insert(
                    "gantry_detector_tilt_degrees".to_string(),
                    Value::from(classic_ct_ds_number(gantry_detector_tilt_degrees)),
                );
        }
        manifest_entry
            .as_object_mut()
            .expect("CT manifest entry must be an object")
            .insert("expected_geometry".to_string(), expected_geometry);
    }
    if let Some(organization) = recipe.series_organization {
        manifest_entry
            .as_object_mut()
            .expect("CT manifest entry must be an object")
            .insert(
                "expected_series_organization".to_string(),
                serde_json::json!({
                    "group_id": organization.group_id,
                    "study_series_count": study_series_count,
                    "series_ordinal": series_index + 1,
                    "series_instance_count": series.slices.len(),
                    "shared_study_instance_uid_expected": true,
                    "shared_frame_of_reference_uid_expected": true,
                    "distinct_series_instance_uids_expected": true
                }),
            );
    }
    manifest_entry
}

fn classic_ct_ds_number(encoded: &str) -> f64 {
    encoded
        .parse::<f64>()
        .expect("classic CT DS recipe value must be numeric")
}

fn classic_ct_ds_values<const N: usize>(encoded: &str) -> [f64; N] {
    encoded
        .split('\\')
        .map(|value| {
            value
                .parse::<f64>()
                .expect("classic CT geometry DS recipe value must be numeric")
        })
        .collect::<Vec<_>>()
        .try_into()
        .unwrap_or_else(|values: Vec<f64>| {
            panic!(
                "classic CT geometry DS recipe must contain {N} values, got {}",
                values.len()
            )
        })
}

fn classic_ct_adjacent_spacing(slices: &[ClassicCtSliceRecipe]) -> Vec<f64> {
    let mut positions = slices
        .iter()
        .map(|slice| slice.position_along_normal)
        .collect::<Vec<_>>();
    positions.sort_by(f64::total_cmp);
    positions
        .windows(2)
        .map(|pair| (pair[1] - pair[0]).abs())
        .collect()
}

fn classic_ct_spacing_is_uniform(slices: &[ClassicCtSliceRecipe]) -> bool {
    let spacing = classic_ct_adjacent_spacing(slices);
    spacing.first().is_none_or(|first| {
        spacing
            .iter()
            .all(|value| (value - first).abs() <= 0.000_01)
    })
}

fn classic_ct_instance_number_state(instance_number: ClassicCtInstanceNumber) -> &'static str {
    match instance_number {
        ClassicCtInstanceNumber::Numeric(_) => "numeric",
        ClassicCtInstanceNumber::Empty => "empty",
    }
}

fn classic_ct_instance_number_value(instance_number: ClassicCtInstanceNumber) -> Option<i64> {
    match instance_number {
        ClassicCtInstanceNumber::Numeric(value) => Some(
            value
                .parse::<i64>()
                .expect("numeric CT Instance Number recipe must contain an integer"),
        ),
        ClassicCtInstanceNumber::Empty => None,
    }
}

fn classic_ct_instance_number_order_index(
    slices: &[ClassicCtSliceRecipe],
    instance_number: ClassicCtInstanceNumber,
) -> Option<usize> {
    let instance_number = classic_ct_instance_number_value(instance_number)?;
    let mut instance_numbers = slices
        .iter()
        .map(|slice| classic_ct_instance_number_value(slice.instance_number))
        .collect::<Option<Vec<_>>>()?;
    instance_numbers.sort_unstable();
    if instance_numbers.windows(2).any(|pair| pair[0] == pair[1]) {
        return None;
    }
    instance_numbers
        .iter()
        .position(|candidate| *candidate == instance_number)
        .map(|index| index + 1)
}

fn classic_ct_profile_membership(recipe: ClassicCtRecipe) -> &'static [&'static str] {
    if recipe.case_id.starts_with("stress/") {
        &["stress"]
    } else if recipe.case_id == "geometry/ct/spatial_sort_conflicts_instance_number" {
        &["core", "extended"]
    } else if recipe.transfer_syntax == RLE_LOSSLESS {
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
    if classic_ct_series_recipes(recipe)
        .iter()
        .any(|series| series.slices.len() > 1)
    {
        capabilities.push("sort_series_by_geometry");
    }
    if recipe.series_organization.is_some() {
        capabilities.push("organize_series_by_study_and_frame_of_reference");
    }
    if recipe.gantry_detector_tilt_degrees.is_some() {
        capabilities.push("interpret_gantry_tilt");
    }
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
    if classic_ct_series_recipes(recipe)
        .iter()
        .any(|series| series.slices.len() > 1)
    {
        stressors.extend(["multi_instance_series", "geometry_slice_sorting"]);
    }
    if recipe.case_id == "geometry/ct/nonuniform_slice_spacing" {
        stressors.push("nonuniform_slice_spacing");
    }
    if recipe.gantry_detector_tilt_degrees.is_some() {
        stressors.extend(["gantry_detector_tilt", "sheared_slice_origins"]);
    }
    if recipe.case_id == "geometry/ct/duplicate_missing_instance_number" {
        stressors.extend(["duplicate_instance_number", "empty_type2_instance_number"]);
    }
    if recipe.series_organization.is_some() {
        stressors.extend([
            "multiple_series_one_study",
            "shared_frame_of_reference_across_series",
        ]);
    }
    if recipe.case_id == STRESS_HIGH_INSTANCE_CT_CASE_ID {
        stressors.extend(["reduced_stress_scale", "high_instance_count_study"]);
    }
    stressors
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
    // Study ID is a Study-level attribute and must remain identical to the
    // referenced Enhanced CT source within their shared Study Instance UID.
    put_str(&mut obj, tags::STUDY_ID, VR::SH, "DTS-ECT");
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
    put_str(&mut obj, tags::LOSSY_IMAGE_COMPRESSION, VR::CS, "00");
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

    let native_frames = if recipe.transfer_syntax == DEFLATED_IMAGE_FRAME {
        let frame_byte_len = segmentation_frame_byte_len(recipe);
        recipe
            .pixel_bytes
            .chunks(frame_byte_len)
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
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
            enhanced_pet_image: None,
            mg_image: None,
            dx_image: None,
            xa_image: None,
            xrf_image: None,
            us_image: None,
            us_multiframe: None,
            nm_image: None,
            pet_image: None,
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

fn write_rt_plan_case(
    run: &PreparedGenerationRun,
    case: &Value,
    recipe: RtPlanRecipe,
    structure_set_source: &GeneratedSourceObject,
    dose_source: &GeneratedSourceObject,
    standards_lock_sha256: &str,
) -> Result<GeneratedFile, GenerateError> {
    validate_rt_plan_sources(&run.out_dir, recipe, structure_set_source, dose_source)?;
    let structure_set_series_instance_uid = required_source_uid(
        structure_set_source.series_instance_uid.as_deref(),
        RT_PLAN_CASE_ID,
        "RT Plan Structure Set source Series Instance UID is missing",
    )?;
    let dose_series_instance_uid = required_source_uid(
        dose_source.series_instance_uid.as_deref(),
        RT_PLAN_CASE_ID,
        "RT Plan Dose source Series Instance UID is missing",
    )?;
    let frame_of_reference_uid = required_source_uid(
        structure_set_source.frame_of_reference_uid.as_deref(),
        RT_PLAN_CASE_ID,
        "RT Plan Structure Set source Frame of Reference UID is missing",
    )?;
    let series_instance_uid = deterministic_rt_plan_uid(
        standards_lock_sha256,
        recipe,
        run.seed,
        UidRole::SeriesInstance,
    );
    let sop_instance_uid = deterministic_rt_plan_uid(
        standards_lock_sha256,
        recipe,
        run.seed,
        UidRole::SopInstance,
    );
    let implementation_class_uid = deterministic_implementation_uid(standards_lock_sha256);
    let relative_path = format!("{}/{RT_PLAN_OUTPUT_FILE}", recipe.case_id);
    let path = run.out_dir.join(&relative_path);
    let case_dir = path.parent().ok_or_else(|| GenerateError::MetadataShape {
        path: PathBuf::from(&relative_path),
        message: "RT Plan output must have a parent directory",
    })?;
    fs::create_dir_all(case_dir).map_err(|source| GenerateError::CreateCaseOutputDir {
        path: case_dir.to_path_buf(),
        source,
    })?;

    let object = build_rt_plan(RtPlanInput {
        study_instance_uid: &structure_set_source.study_instance_uid,
        frame_of_reference_uid,
        series_instance_uid: &series_instance_uid,
        sop_instance_uid: &sop_instance_uid,
        structure_set_sop_class_uid: &structure_set_source.sop_class_uid,
        structure_set_sop_instance_uid: &structure_set_source.sop_instance_uid,
        dose_sop_class_uid: &dose_source.sop_class_uid,
        dose_sop_instance_uid: &dose_source.sop_instance_uid,
    })
    .map_err(|message| GenerateError::WriteDicomFile {
        path: path.clone(),
        message,
    })?;
    object
        .with_meta(
            FileMetaTableBuilder::new()
                .transfer_syntax(EXPLICIT_VR_LITTLE_ENDIAN.uid)
                .implementation_class_uid(&implementation_class_uid)
                .implementation_version_name(crate::IMPLEMENTATION_VERSION_NAME),
        )
        .map_err(|error| GenerateError::WriteDicomFile {
            path: path.clone(),
            message: error.to_string(),
        })?
        .write_to_file(&path)
        .map_err(|error| GenerateError::WriteDicomFile {
            path: path.clone(),
            message: error.to_string(),
        })?;

    let expected_rt_plan = linked_rt_plan_expected(LinkedRtPlanInput {
        sop_instance_uid: &sop_instance_uid,
        study_instance_uid: &structure_set_source.study_instance_uid,
        series_instance_uid: &series_instance_uid,
        frame_of_reference_uid,
        structure_set_series_instance_uid,
        structure_set_sop_instance_uid: &structure_set_source.sop_instance_uid,
        structure_set_sha256: &structure_set_source.sha256,
        dose_series_instance_uid,
        dose_sop_instance_uid: &dose_source.sop_instance_uid,
        dose_sha256: &dose_source.sha256,
    });
    let validated = validate_rt_plan_file(
        &path,
        &RtPlanExpectations {
            implementation_class_uid: &implementation_class_uid,
            synthetic_data: "YES",
            expected_rt_plan,
        },
    )?;
    let mut validation = validated.validation;
    validation["internal"]
        .as_array_mut()
        .expect("RT Plan validation internal results are an array")
        .push(serde_json::json!({
            "name": "rt_plan_source_precheck",
            "status": "passed",
            "message": "Rust reopened and hashed the linked RT Structure Set and RT Dose sources, then verified their manifest identities and shared Study and Frame of Reference before construction."
        }));
    let expected_rt_plan = serde_json::to_value(expected_rt_plan)
        .expect("RT Plan expectation serialization is infallible");
    let bytes = validated.bytes;

    Ok(GeneratedFile {
        case_id: recipe.case_id.to_string(),
        manifest_entry: serde_json::json!({
            "case_id": recipe.case_id,
            "profile_membership": ["extended"],
            "path": relative_path,
            "sha256": sha256_hex(&bytes),
            "size_bytes": bytes.len(),
            "determinism": "byte_stable",
            "recipe": {
                "recipe_id": recipe.recipe_id,
                "recipe_version": RT_PLAN_RECIPE_VERSION,
                "recipe_parameters": {
                    "structure_set_source_case_id": recipe.structure_set_source_case_id,
                    "dose_source_case_id": recipe.dose_source_case_id,
                    "fraction_group_count": 1,
                    "beam_count": 1,
                    "control_point_count": 2,
                    "beam_type": "STATIC",
                    "radiation_type": "PHOTON"
                }
            },
            "dicom": {
                "sop_class_uid": RT_PLAN_STORAGE_UID,
                "sop_class_name": "RT Plan Storage",
                "iod_name": "RT Plan",
                "modality": "RTPLAN",
                "transfer_syntax_uid": EXPLICIT_VR_LITTLE_ENDIAN.uid,
                "transfer_syntax_name": EXPLICIT_VR_LITTLE_ENDIAN.name
            },
            "uids": {
                "study_instance_uid": structure_set_source.study_instance_uid,
                "series_instance_uid": series_instance_uid,
                "sop_instance_uid": sop_instance_uid,
                "frame_of_reference_uid": frame_of_reference_uid,
                "implementation_class_uid": implementation_class_uid,
                "implementation_version_name": crate::IMPLEMENTATION_VERSION_NAME
            },
            "image": Value::Null,
            "pixel_data": Value::Null,
            "references": [
                structure_set_source.to_manifest_reference("referenced_structure_set", None),
                dose_source.to_manifest_reference("referenced_dose", None)
            ],
            "expected_capabilities": [
                "open_file", "read_metadata", "resolve_references", "read_rt_plan"
            ],
            "expected_semantics": {
                "synthetic_data": "YES",
                "linked_structure_set_sop_instance_uid": structure_set_source.sop_instance_uid,
                "linked_dose_sop_instance_uid": dose_source.sop_instance_uid,
                "pixel_data_absent": true
            },
            "expected_rt_plan": expected_rt_plan,
            "expected_visual_checks": {
                "pattern": "single_static_photon_beam_with_linked_structure_and_dose"
            },
            "validation": validation,
            "known_stressors": [
                "rt_plan_storage", "linked_rt_structure_set", "linked_rt_dose",
                "single_fraction_group", "static_photon_beam", "control_point_inheritance",
                "pixel_data_absent"
            ],
            "standards_evidence": deduplicated_standards_evidence(standards_evidence_from_case(case))
        }),
    })
}

fn validate_rt_plan_sources(
    generated_root: &std::path::Path,
    recipe: RtPlanRecipe,
    structure_set_source: &GeneratedSourceObject,
    dose_source: &GeneratedSourceObject,
) -> Result<(), GenerateError> {
    let structure_set_frame = required_source_uid(
        structure_set_source.frame_of_reference_uid.as_deref(),
        RT_PLAN_CASE_ID,
        "RT Plan Structure Set source Frame of Reference UID is missing",
    )?;
    let dose_frame = required_source_uid(
        dose_source.frame_of_reference_uid.as_deref(),
        RT_PLAN_CASE_ID,
        "RT Plan Dose source Frame of Reference UID is missing",
    )?;
    if structure_set_source.source_case_id != recipe.structure_set_source_case_id
        || dose_source.source_case_id != recipe.dose_source_case_id
        || structure_set_source.source_path
            != format!("{}/instance.dcm", recipe.structure_set_source_case_id)
        || dose_source.source_path != format!("{}/instance.dcm", recipe.dose_source_case_id)
        || structure_set_source.sop_class_uid != RT_PLAN_REFERENCED_STRUCTURE_SET_STORAGE_UID
        || dose_source.sop_class_uid != RT_PLAN_REFERENCED_DOSE_STORAGE_UID
        || structure_set_source.study_instance_uid != dose_source.study_instance_uid
        || structure_set_frame != dose_frame
        || structure_set_source.sop_instance_uid == dose_source.sop_instance_uid
    {
        return Err(rt_plan_source_error(
            "RT Plan sources differ from the locked linked Structure Set and Dose identity topology",
        ));
    }
    for source in [structure_set_source, dose_source] {
        required_source_uid(
            source.series_instance_uid.as_deref(),
            RT_PLAN_CASE_ID,
            "RT Plan source Series Instance UID is missing",
        )?;
        let path = generated_root.join(&source.source_path);
        let bytes = fs::read(&path).map_err(|error| GenerateError::ReadMetadata {
            path: path.clone(),
            source: error,
        })?;
        let object = open_file(&path).map_err(|error| GenerateError::ValidateDicomFile {
            path: path.clone(),
            message: error.to_string(),
        })?;
        let text = |tag| {
            object
                .element(tag)
                .map_err(|error| rt_plan_source_error(error.to_string()))?
                .to_str()
                .map(|value| value.trim_end_matches(['\0', ' ']).to_string())
                .map_err(|error| rt_plan_source_error(error.to_string()))
        };
        if sha256_hex(&bytes) != source.sha256
            || object.meta().media_storage_sop_class_uid() != source.sop_class_uid
            || object.meta().media_storage_sop_instance_uid() != source.sop_instance_uid
            || object.meta().transfer_syntax() != EXPLICIT_VR_LITTLE_ENDIAN.uid
            || text(tags::SOP_CLASS_UID)? != source.sop_class_uid
            || text(tags::SOP_INSTANCE_UID)? != source.sop_instance_uid
            || text(tags::STUDY_INSTANCE_UID)? != source.study_instance_uid
            || text(tags::SERIES_INSTANCE_UID)?
                != source.series_instance_uid.as_deref().unwrap_or_default()
            || text(tags::FRAME_OF_REFERENCE_UID)?
                != source.frame_of_reference_uid.as_deref().unwrap_or_default()
        {
            return Err(rt_plan_source_error(
                "RT Plan source bytes or DICOM identity differ from the generated-source registry",
            ));
        }
    }
    Ok(())
}

fn rt_plan_source_error(message: impl Into<String>) -> GenerateError {
    GenerateError::ValidateDicomFile {
        path: PathBuf::from(RT_PLAN_CASE_ID),
        message: message.into(),
    }
}

fn write_rt_image_case(
    run: &PreparedGenerationRun,
    case: &Value,
    recipe: RtImageRecipe,
    plan_source: &GeneratedSourceObject,
    standards_lock_sha256: &str,
) -> Result<GeneratedFile, GenerateError> {
    validate_rt_image_plan_source(&run.out_dir, recipe, plan_source)?;
    let plan_series_instance_uid = required_source_uid(
        plan_source.series_instance_uid.as_deref(),
        RT_IMAGE_CASE_ID,
        "RT Image Plan source Series Instance UID is missing",
    )?;
    let frame_of_reference_uid = required_source_uid(
        plan_source.frame_of_reference_uid.as_deref(),
        RT_IMAGE_CASE_ID,
        "RT Image Plan source Frame of Reference UID is missing",
    )?;
    let series_instance_uid = deterministic_rt_image_uid(
        standards_lock_sha256,
        recipe,
        run.seed,
        UidRole::SeriesInstance,
    );
    let sop_instance_uid = deterministic_rt_image_uid(
        standards_lock_sha256,
        recipe,
        run.seed,
        UidRole::SopInstance,
    );
    let implementation_class_uid = deterministic_implementation_uid(standards_lock_sha256);
    let relative_path = format!("{}/{RT_IMAGE_OUTPUT_FILE}", recipe.case_id);
    let path = run.out_dir.join(&relative_path);
    let case_dir = path.parent().ok_or_else(|| GenerateError::MetadataShape {
        path: PathBuf::from(&relative_path),
        message: "RT Image output must have a parent directory",
    })?;
    fs::create_dir_all(case_dir).map_err(|source| GenerateError::CreateCaseOutputDir {
        path: case_dir.to_path_buf(),
        source,
    })?;

    let object = build_rt_image(RtImageInput {
        study_instance_uid: &plan_source.study_instance_uid,
        frame_of_reference_uid,
        series_instance_uid: &series_instance_uid,
        sop_instance_uid: &sop_instance_uid,
        plan_sop_class_uid: &plan_source.sop_class_uid,
        plan_sop_instance_uid: &plan_source.sop_instance_uid,
    })
    .map_err(|message| GenerateError::WriteDicomFile {
        path: path.clone(),
        message,
    })?;
    object
        .with_meta(
            FileMetaTableBuilder::new()
                .transfer_syntax(EXPLICIT_VR_LITTLE_ENDIAN.uid)
                .implementation_class_uid(&implementation_class_uid)
                .implementation_version_name(crate::IMPLEMENTATION_VERSION_NAME),
        )
        .map_err(|error| GenerateError::WriteDicomFile {
            path: path.clone(),
            message: error.to_string(),
        })?
        .write_to_file(&path)
        .map_err(|error| GenerateError::WriteDicomFile {
            path: path.clone(),
            message: error.to_string(),
        })?;

    let expected_rt_image = linked_rt_image_expected(LinkedRtImageInput {
        sop_instance_uid: &sop_instance_uid,
        study_instance_uid: &plan_source.study_instance_uid,
        series_instance_uid: &series_instance_uid,
        frame_of_reference_uid,
        plan_series_instance_uid,
        plan_sop_instance_uid: &plan_source.sop_instance_uid,
        plan_sha256: &plan_source.sha256,
    });
    let validated = validate_rt_image_file(
        &path,
        &RtImageExpectations {
            implementation_class_uid: &implementation_class_uid,
            synthetic_data: "YES",
            expected_rt_image,
        },
    )?;
    let mut validation = validated.validation;
    validation["internal"]
        .as_array_mut()
        .expect("RT Image validation internal results are an array")
        .push(serde_json::json!({
            "name": "rt_image_plan_source_precheck",
            "status": "passed",
            "message": "Rust reopened and hashed the linked RT Plan, then verified its generated-source identity and shared Study and Frame of Reference before construction."
        }));
    let expected_rt_image = serde_json::to_value(expected_rt_image)
        .expect("RT Image expectation serialization is infallible");
    let bytes = validated.bytes;

    Ok(GeneratedFile {
        case_id: recipe.case_id.to_string(),
        manifest_entry: serde_json::json!({
            "case_id": recipe.case_id,
            "profile_membership": ["extended"],
            "path": relative_path,
            "sha256": sha256_hex(&bytes),
            "size_bytes": bytes.len(),
            "determinism": "byte_stable",
            "recipe": {
                "recipe_id": recipe.recipe_id,
                "recipe_version": RT_IMAGE_RECIPE_VERSION,
                "recipe_parameters": {
                    "plan_source_case_id": recipe.plan_source_case_id,
                    "referenced_fraction_group_number": 1,
                    "referenced_beam_number": 1,
                    "pixel_value_formula": "17 * (4 * r + c)",
                    "payload_sha256": RT_IMAGE_PIXEL_SHA256
                }
            },
            "dicom": {
                "sop_class_uid": RT_IMAGE_STORAGE_UID,
                "sop_class_name": "RT Image Storage",
                "iod_name": "RT Image",
                "modality": "RTIMAGE",
                "transfer_syntax_uid": EXPLICIT_VR_LITTLE_ENDIAN.uid,
                "transfer_syntax_name": EXPLICIT_VR_LITTLE_ENDIAN.name
            },
            "uids": {
                "study_instance_uid": plan_source.study_instance_uid,
                "series_instance_uid": series_instance_uid,
                "sop_instance_uid": sop_instance_uid,
                "frame_of_reference_uid": frame_of_reference_uid,
                "implementation_class_uid": implementation_class_uid,
                "implementation_version_name": crate::IMPLEMENTATION_VERSION_NAME
            },
            "image": {
                "sample_type": "integer",
                "rows": 4,
                "columns": 4,
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
                "value_length": RT_IMAGE_PIXEL_BYTES.len(),
                "frame_count": 1,
                "frame_hashes": [RT_IMAGE_PIXEL_SHA256]
            },
            "references": [plan_source.to_manifest_reference("referenced_rt_plan", None)],
            "expected_capabilities": [
                "open_file", "read_metadata", "resolve_references", "read_rt_image", "decode_native_pixels"
            ],
            "expected_semantics": {
                "synthetic_data": "YES",
                "linked_plan_sop_instance_uid": plan_source.sop_instance_uid,
                "referenced_fraction_group_number": 1,
                "referenced_beam_number": 1,
                "image_type": ["DERIVED", "SECONDARY", "DRR"],
                "conversion_type": "WSD",
                "rt_image_plane": "NORMAL",
                "pixel_value_formula": "17 * (4 * r + c)",
                "payload_sha256": RT_IMAGE_PIXEL_SHA256
            },
            "expected_rt_image": expected_rt_image,
            "expected_visual_checks": {
                "pattern": "4x4_monochrome_gradient",
                "minimum_displays_black": true,
                "maximum_displays_white": true
            },
            "validation": validation,
            "known_stressors": [
                "rt_image_storage", "linked_rt_plan", "beam_and_fraction_linkage",
                "native_ob_pixels", "drr_geometry", "pixel_data_present"
            ],
            "standards_evidence": deduplicated_standards_evidence(standards_evidence_from_case(case))
        }),
    })
}

fn validate_rt_image_plan_source(
    generated_root: &std::path::Path,
    recipe: RtImageRecipe,
    plan_source: &GeneratedSourceObject,
) -> Result<(), GenerateError> {
    let plan_series = required_source_uid(
        plan_source.series_instance_uid.as_deref(),
        RT_IMAGE_CASE_ID,
        "RT Image Plan source Series Instance UID is missing",
    )?;
    let plan_frame = required_source_uid(
        plan_source.frame_of_reference_uid.as_deref(),
        RT_IMAGE_CASE_ID,
        "RT Image Plan source Frame of Reference UID is missing",
    )?;
    if plan_source.source_case_id != recipe.plan_source_case_id
        || plan_source.source_path != format!("{}/instance.dcm", recipe.plan_source_case_id)
        || plan_source.sop_class_uid != RT_IMAGE_REFERENCED_PLAN_STORAGE_UID
        || plan_source.study_instance_uid.is_empty()
        || plan_series.is_empty()
        || plan_frame.is_empty()
    {
        return Err(rt_image_source_error(
            "RT Image source differs from the locked linked Plan identity topology",
        ));
    }
    let path = generated_root.join(&plan_source.source_path);
    let bytes = fs::read(&path).map_err(|error| GenerateError::ReadMetadata {
        path: path.clone(),
        source: error,
    })?;
    let object = open_file(&path).map_err(|error| GenerateError::ValidateDicomFile {
        path: path.clone(),
        message: error.to_string(),
    })?;
    let text = |tag| {
        object
            .element(tag)
            .map_err(|error| rt_image_source_error(error.to_string()))?
            .to_str()
            .map(|value| value.trim_end_matches(['\0', ' ']).to_string())
            .map_err(|error| rt_image_source_error(error.to_string()))
    };
    if sha256_hex(&bytes) != plan_source.sha256
        || object.meta().media_storage_sop_class_uid() != plan_source.sop_class_uid
        || object.meta().media_storage_sop_instance_uid() != plan_source.sop_instance_uid
        || object.meta().transfer_syntax() != EXPLICIT_VR_LITTLE_ENDIAN.uid
        || text(tags::SOP_CLASS_UID)? != plan_source.sop_class_uid
        || text(tags::SOP_INSTANCE_UID)? != plan_source.sop_instance_uid
        || text(tags::STUDY_INSTANCE_UID)? != plan_source.study_instance_uid
        || text(tags::SERIES_INSTANCE_UID)? != plan_series
        || text(tags::FRAME_OF_REFERENCE_UID)? != plan_frame
    {
        return Err(rt_image_source_error(
            "RT Image Plan source bytes or DICOM identity differ from the generated-source registry",
        ));
    }
    Ok(())
}

fn rt_image_source_error(message: impl Into<String>) -> GenerateError {
    GenerateError::ValidateDicomFile {
        path: PathBuf::from(RT_IMAGE_CASE_ID),
        message: message.into(),
    }
}

fn write_rt_radiation_case(
    run: &PreparedGenerationRun,
    case: &Value,
    recipe: RtRadiationRecipe,
    plan_source: &GeneratedSourceObject,
    standards_lock_sha256: &str,
) -> Result<GeneratedFile, GenerateError> {
    validate_generated_rt_source(
        &run.out_dir,
        recipe.case_id,
        plan_source,
        recipe.plan_source_case_id,
        RT_RADIATION_PLAN_STORAGE_UID,
    )?;
    let plan_series_instance_uid = required_source_uid(
        plan_source.series_instance_uid.as_deref(),
        RT_RADIATION_CASE_ID,
        "C-Arm RT Radiation Plan source Series Instance UID is missing",
    )?;
    let frame_of_reference_uid = required_source_uid(
        plan_source.frame_of_reference_uid.as_deref(),
        RT_RADIATION_CASE_ID,
        "C-Arm RT Radiation Plan source Frame of Reference UID is missing",
    )?;
    let series_instance_uid = deterministic_rt_radiation_uid(
        standards_lock_sha256,
        recipe,
        run.seed,
        UidRole::SeriesInstance,
        None,
    );
    let sop_instance_uid = deterministic_rt_radiation_uid(
        standards_lock_sha256,
        recipe,
        run.seed,
        UidRole::SopInstance,
        None,
    );
    let implementation_class_uid = deterministic_implementation_uid(standards_lock_sha256);
    let relative_path = format!("{}/{RT_RADIATION_OUTPUT_FILE}", recipe.case_id);
    let path = run.out_dir.join(&relative_path);
    let case_dir = path.parent().ok_or_else(|| GenerateError::MetadataShape {
        path: PathBuf::from(&relative_path),
        message: "C-Arm RT Radiation output must have a parent directory",
    })?;
    fs::create_dir_all(case_dir).map_err(|source| GenerateError::CreateCaseOutputDir {
        path: case_dir.to_path_buf(),
        source,
    })?;

    build_rt_radiation(RtRadiationInput {
        study_instance_uid: &plan_source.study_instance_uid,
        frame_of_reference_uid,
        series_instance_uid: &series_instance_uid,
        sop_instance_uid: &sop_instance_uid,
        plan_series_instance_uid,
        plan_sop_class_uid: &plan_source.sop_class_uid,
        plan_sop_instance_uid: &plan_source.sop_instance_uid,
    })
    .map_err(|message| GenerateError::WriteDicomFile {
        path: path.clone(),
        message,
    })?
    .with_meta(
        FileMetaTableBuilder::new()
            .transfer_syntax(EXPLICIT_VR_LITTLE_ENDIAN.uid)
            .implementation_class_uid(&implementation_class_uid)
            .implementation_version_name(crate::IMPLEMENTATION_VERSION_NAME),
    )
    .map_err(|error| GenerateError::WriteDicomFile {
        path: path.clone(),
        message: error.to_string(),
    })?
    .write_to_file(&path)
    .map_err(|error| GenerateError::WriteDicomFile {
        path: path.clone(),
        message: error.to_string(),
    })?;

    let expected_rt_radiation_contract = minimal_carm_rt_radiation_expected(CArmRtRadiationInput {
        sop_instance_uid: &sop_instance_uid,
        study_instance_uid: &plan_source.study_instance_uid,
        series_instance_uid: &series_instance_uid,
        frame_of_reference_uid,
        plan_series_instance_uid,
        plan_sop_instance_uid: &plan_source.sop_instance_uid,
        plan_sha256: &plan_source.sha256,
        software_versions: crate::PACKAGE_VERSION,
    });
    let validated = validate_rt_radiation_file(
        &path,
        &RtRadiationExpectations {
            implementation_class_uid: &implementation_class_uid,
            synthetic_data: "YES",
            expected_rt_radiation: expected_rt_radiation_contract,
        },
    )?;
    let mut validation = validated.validation;
    validation["internal"]
        .as_array_mut()
        .expect("RT Radiation validation internal results are an array")
        .push(serde_json::json!({
            "name": "rt_radiation_source_precheck",
            "status": "passed",
            "message": "Rust reopened and hashed the linked RT Plan, then verified its manifest identity, Study, and Frame of Reference before Radiation construction."
        }));
    let expected_rt_radiation = serde_json::to_value(expected_rt_radiation_contract)
        .expect("C-Arm RT Radiation expectation serialization is infallible");
    let bytes = validated.bytes;

    Ok(GeneratedFile {
        case_id: recipe.case_id.to_string(),
        manifest_entry: serde_json::json!({
            "case_id": recipe.case_id,
            "profile_membership": ["extended"],
            "path": relative_path,
            "sha256": sha256_hex(&bytes),
            "size_bytes": bytes.len(),
            "determinism": "byte_stable",
            "recipe": {
                "recipe_id": recipe.recipe_id,
                "recipe_version": RT_RADIATION_RECIPE_VERSION,
                "recipe_parameters": {
                    "plan_source_case_id": recipe.plan_source_case_id,
                    "physical_and_geometric_content_detail_flag": "IDENT_ONLY",
                    "rt_record_flag": "NO",
                    "treatment_position_count": 1,
                    "control_point_count": 2
                }
            },
            "dicom": {
                "sop_class_uid": C_ARM_PHOTON_ELECTRON_RADIATION_STORAGE_UID,
                "sop_class_name": "C-Arm Photon-Electron Radiation Storage",
                "iod_name": "C-Arm Photon-Electron Radiation",
                "modality": "RTRAD",
                "transfer_syntax_uid": EXPLICIT_VR_LITTLE_ENDIAN.uid,
                "transfer_syntax_name": EXPLICIT_VR_LITTLE_ENDIAN.name
            },
            "uids": {
                "study_instance_uid": plan_source.study_instance_uid,
                "series_instance_uid": series_instance_uid,
                "sop_instance_uid": sop_instance_uid,
                "frame_of_reference_uid": frame_of_reference_uid,
                "implementation_class_uid": implementation_class_uid,
                "implementation_version_name": crate::IMPLEMENTATION_VERSION_NAME
            },
            "image": Value::Null,
            "pixel_data": Value::Null,
            "references": [plan_source.to_manifest_reference("definition_source", None)],
            "expected_capabilities": [
                "open_file", "read_metadata", "resolve_references", "read_rt_radiation"
            ],
            "expected_semantics": {
                "synthetic_data": "YES",
                "linked_plan_sop_instance_uid": plan_source.sop_instance_uid,
                "rt_record_flag": "NO",
                "control_point_inheritance": true,
                "pixel_data_absent": true
            },
            "expected_rt_radiation": expected_rt_radiation,
            "expected_visual_checks": {
                "pattern": "single_static_carm_beam"
            },
            "validation": validation,
            "known_stressors": [
                "carm_photon_electron_radiation_storage", "linked_rt_plan",
                "ident_only_content", "control_point_inheritance", "pixel_data_absent"
            ],
            "standards_evidence": deduplicated_standards_evidence(standards_evidence_from_case(case))
        }),
    })
}

fn write_rt_radiation_set_case(
    run: &PreparedGenerationRun,
    case: &Value,
    recipe: RtRadiationSetRecipe,
    plan_source: &GeneratedSourceObject,
    radiation_source: &GeneratedSourceObject,
    standards_lock_sha256: &str,
) -> Result<GeneratedFile, GenerateError> {
    validate_generated_rt_source(
        &run.out_dir,
        recipe.case_id,
        plan_source,
        recipe.plan_source_case_id,
        RT_RADIATION_PLAN_STORAGE_UID,
    )?;
    validate_generated_rt_source(
        &run.out_dir,
        recipe.case_id,
        radiation_source,
        recipe.radiation_source_case_id,
        C_ARM_PHOTON_ELECTRON_RADIATION_STORAGE_UID,
    )?;
    let plan_series_instance_uid = required_source_uid(
        plan_source.series_instance_uid.as_deref(),
        RT_RADIATION_SET_CASE_ID,
        "RT Radiation Set Plan source Series Instance UID is missing",
    )?;
    let radiation_series_instance_uid = required_source_uid(
        radiation_source.series_instance_uid.as_deref(),
        RT_RADIATION_SET_CASE_ID,
        "RT Radiation Set Radiation source Series Instance UID is missing",
    )?;
    let plan_frame_of_reference_uid = required_source_uid(
        plan_source.frame_of_reference_uid.as_deref(),
        RT_RADIATION_SET_CASE_ID,
        "RT Radiation Set Plan source Frame of Reference UID is missing",
    )?;
    let radiation_frame_of_reference_uid = required_source_uid(
        radiation_source.frame_of_reference_uid.as_deref(),
        RT_RADIATION_SET_CASE_ID,
        "RT Radiation Set Radiation source Frame of Reference UID is missing",
    )?;
    if plan_source.study_instance_uid != radiation_source.study_instance_uid
        || plan_frame_of_reference_uid != radiation_frame_of_reference_uid
        || plan_source.sop_instance_uid == radiation_source.sop_instance_uid
        || plan_series_instance_uid == radiation_series_instance_uid
    {
        return Err(rt_generation_source_error(
            recipe.case_id,
            "RT Radiation Set sources differ from the locked Plan-to-Radiation identity topology",
        ));
    }

    let series_instance_uid = deterministic_rt_radiation_set_uid(
        standards_lock_sha256,
        recipe,
        run.seed,
        UidRole::SeriesInstance,
        None,
    );
    let sop_instance_uid = deterministic_rt_radiation_set_uid(
        standards_lock_sha256,
        recipe,
        run.seed,
        UidRole::SopInstance,
        None,
    );
    let treatment_position_group_uid = deterministic_rt_radiation_set_uid(
        standards_lock_sha256,
        recipe,
        run.seed,
        UidRole::DerivedReference,
        Some(0),
    );
    let implementation_class_uid = deterministic_implementation_uid(standards_lock_sha256);
    let relative_path = format!("{}/{RT_RADIATION_SET_OUTPUT_FILE}", recipe.case_id);
    let path = run.out_dir.join(&relative_path);
    let case_dir = path.parent().ok_or_else(|| GenerateError::MetadataShape {
        path: PathBuf::from(&relative_path),
        message: "RT Radiation Set output must have a parent directory",
    })?;
    fs::create_dir_all(case_dir).map_err(|source| GenerateError::CreateCaseOutputDir {
        path: case_dir.to_path_buf(),
        source,
    })?;

    build_rt_radiation_set(NativeRtRadiationSetInput {
        study_instance_uid: &plan_source.study_instance_uid,
        frame_of_reference_uid: plan_frame_of_reference_uid,
        series_instance_uid: &series_instance_uid,
        sop_instance_uid: &sop_instance_uid,
        plan_series_instance_uid,
        plan_sop_class_uid: &plan_source.sop_class_uid,
        plan_sop_instance_uid: &plan_source.sop_instance_uid,
        radiation_series_instance_uid,
        radiation_sop_class_uid: &radiation_source.sop_class_uid,
        radiation_sop_instance_uid: &radiation_source.sop_instance_uid,
        treatment_position_group_uid: &treatment_position_group_uid,
    })
    .map_err(|message| GenerateError::WriteDicomFile {
        path: path.clone(),
        message,
    })?
    .with_meta(
        FileMetaTableBuilder::new()
            .transfer_syntax(EXPLICIT_VR_LITTLE_ENDIAN.uid)
            .implementation_class_uid(&implementation_class_uid)
            .implementation_version_name(crate::IMPLEMENTATION_VERSION_NAME),
    )
    .map_err(|error| GenerateError::WriteDicomFile {
        path: path.clone(),
        message: error.to_string(),
    })?
    .write_to_file(&path)
    .map_err(|error| GenerateError::WriteDicomFile {
        path: path.clone(),
        message: error.to_string(),
    })?;

    let expected_rt_radiation_set_contract =
        minimal_rt_radiation_set_expected(RtRadiationSetInput {
            sop_instance_uid: &sop_instance_uid,
            study_instance_uid: &plan_source.study_instance_uid,
            series_instance_uid: &series_instance_uid,
            frame_of_reference_uid: plan_frame_of_reference_uid,
            treatment_position_group_uid: &treatment_position_group_uid,
            plan_series_instance_uid,
            plan_sop_instance_uid: &plan_source.sop_instance_uid,
            plan_sha256: &plan_source.sha256,
            radiation_series_instance_uid,
            radiation_sop_instance_uid: &radiation_source.sop_instance_uid,
            radiation_sha256: &radiation_source.sha256,
            software_versions: crate::PACKAGE_VERSION,
        });
    let validated = validate_rt_radiation_set_file(
        &path,
        &RtRadiationSetExpectations {
            implementation_class_uid: &implementation_class_uid,
            synthetic_data: "YES",
            expected_rt_radiation_set: expected_rt_radiation_set_contract,
        },
    )?;
    let mut validation = validated.validation;
    validation["internal"]
        .as_array_mut()
        .expect("RT Radiation Set validation internal results are an array")
        .push(serde_json::json!({
            "name": "rt_radiation_set_source_precheck",
            "status": "passed",
            "message": "Rust reopened and hashed the linked RT Plan and companion Radiation, then verified their manifest identities and shared Study and Frame of Reference before Set construction."
        }));
    let expected_rt_radiation_set = serde_json::to_value(expected_rt_radiation_set_contract)
        .expect("RT Radiation Set expectation serialization is infallible");
    let bytes = validated.bytes;

    Ok(GeneratedFile {
        case_id: recipe.case_id.to_string(),
        manifest_entry: serde_json::json!({
            "case_id": recipe.case_id,
            "profile_membership": ["extended"],
            "path": relative_path,
            "sha256": sha256_hex(&bytes),
            "size_bytes": bytes.len(),
            "determinism": "byte_stable",
            "recipe": {
                "recipe_id": recipe.recipe_id,
                "recipe_version": RT_RADIATION_SET_RECIPE_VERSION,
                "recipe_parameters": {
                    "plan_source_case_id": recipe.plan_source_case_id,
                    "radiation_source_case_id": recipe.radiation_source_case_id,
                    "intended_number_of_fractions": 1,
                    "treatment_position_group_count": 1,
                    "radiation_count": 1
                }
            },
            "dicom": {
                "sop_class_uid": RT_RADIATION_SET_STORAGE_UID,
                "sop_class_name": "RT Radiation Set Storage",
                "iod_name": "RT Radiation Set",
                "modality": "RTRAD",
                "transfer_syntax_uid": EXPLICIT_VR_LITTLE_ENDIAN.uid,
                "transfer_syntax_name": EXPLICIT_VR_LITTLE_ENDIAN.name
            },
            "uids": {
                "study_instance_uid": plan_source.study_instance_uid,
                "series_instance_uid": series_instance_uid,
                "sop_instance_uid": sop_instance_uid,
                "frame_of_reference_uid": plan_frame_of_reference_uid,
                "implementation_class_uid": implementation_class_uid,
                "implementation_version_name": crate::IMPLEMENTATION_VERSION_NAME
            },
            "image": Value::Null,
            "pixel_data": Value::Null,
            "references": [
                plan_source.to_manifest_reference("definition_source", None),
                radiation_source.to_manifest_reference("referenced_rt_radiation", None)
            ],
            "expected_capabilities": [
                "open_file", "read_metadata", "resolve_references", "read_rt_radiation_set"
            ],
            "expected_semantics": {
                "synthetic_data": "YES",
                "intent": "TREATMENT",
                "definition_source_plan_sop_instance_uid": plan_source.sop_instance_uid,
                "linked_radiation_sop_instance_uid": radiation_source.sop_instance_uid,
                "dose_contribution_absent": true,
                "pixel_data_absent": true
            },
            "expected_rt_radiation_set": expected_rt_radiation_set,
            "expected_visual_checks": {
                "pattern": "single_radiation_treatment_position_group"
            },
            "validation": validation,
            "known_stressors": [
                "rt_radiation_set_storage", "linked_rt_plan", "linked_rt_radiation",
                "treatment_position_group", "dose_contribution_absent", "pixel_data_absent"
            ],
            "standards_evidence": deduplicated_standards_evidence(standards_evidence_from_case(case))
        }),
    })
}

fn validate_generated_rt_source(
    generated_root: &std::path::Path,
    owner_case_id: &'static str,
    source: &GeneratedSourceObject,
    expected_case_id: &'static str,
    expected_sop_class_uid: &'static str,
) -> Result<(), GenerateError> {
    let series_instance_uid = source.series_instance_uid.as_deref().ok_or_else(|| {
        rt_generation_source_error(owner_case_id, "RT source Series Instance UID is missing")
    })?;
    let frame_of_reference_uid = source.frame_of_reference_uid.as_deref().ok_or_else(|| {
        rt_generation_source_error(owner_case_id, "RT source Frame of Reference UID is missing")
    })?;
    if source.source_case_id != expected_case_id
        || source.source_path != format!("{expected_case_id}/instance.dcm")
        || source.sop_class_uid != expected_sop_class_uid
        || source.study_instance_uid.is_empty()
        || series_instance_uid.is_empty()
        || frame_of_reference_uid.is_empty()
    {
        return Err(rt_generation_source_error(
            owner_case_id,
            "RT source differs from the locked dependency identity topology",
        ));
    }
    let path = generated_root.join(&source.source_path);
    let bytes = fs::read(&path).map_err(|error| GenerateError::ReadMetadata {
        path: path.clone(),
        source: error,
    })?;
    let object = open_file(&path).map_err(|error| GenerateError::ValidateDicomFile {
        path: path.clone(),
        message: error.to_string(),
    })?;
    let text = |tag| {
        object
            .element(tag)
            .map_err(|error| rt_generation_source_error(owner_case_id, error.to_string()))?
            .to_str()
            .map(|value| value.trim_end_matches(['\0', ' ']).to_string())
            .map_err(|error| rt_generation_source_error(owner_case_id, error.to_string()))
    };
    if sha256_hex(&bytes) != source.sha256
        || object.meta().media_storage_sop_class_uid() != source.sop_class_uid
        || object.meta().media_storage_sop_instance_uid() != source.sop_instance_uid
        || object.meta().transfer_syntax() != EXPLICIT_VR_LITTLE_ENDIAN.uid
        || text(tags::SOP_CLASS_UID)? != source.sop_class_uid
        || text(tags::SOP_INSTANCE_UID)? != source.sop_instance_uid
        || text(tags::STUDY_INSTANCE_UID)? != source.study_instance_uid
        || text(tags::SERIES_INSTANCE_UID)? != series_instance_uid
        || text(tags::FRAME_OF_REFERENCE_UID)? != frame_of_reference_uid
    {
        return Err(rt_generation_source_error(
            owner_case_id,
            "RT source bytes or DICOM identity differ from the generated-source registry",
        ));
    }
    Ok(())
}

fn rt_generation_source_error(case_id: &'static str, message: impl Into<String>) -> GenerateError {
    GenerateError::ValidateDicomFile {
        path: PathBuf::from(case_id),
        message: message.into(),
    }
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

fn write_encapsulated_stl_case(
    run: &PreparedGenerationRun,
    case: &Value,
    recipe: EncapsulatedStlRecipe,
    standards_lock_sha256: &str,
) -> Result<GeneratedFile, GenerateError> {
    let study_instance_uid = deterministic_encapsulated_stl_uid(
        standards_lock_sha256,
        recipe,
        run.seed,
        UidRole::StudyInstance,
    );
    let series_instance_uid = deterministic_encapsulated_stl_uid(
        standards_lock_sha256,
        recipe,
        run.seed,
        UidRole::SeriesInstance,
    );
    let sop_instance_uid = deterministic_encapsulated_stl_uid(
        standards_lock_sha256,
        recipe,
        run.seed,
        UidRole::SopInstance,
    );
    let frame_of_reference_uid = deterministic_encapsulated_stl_uid(
        standards_lock_sha256,
        recipe,
        run.seed,
        UidRole::FrameOfReference,
    );
    let implementation_class_uid = deterministic_implementation_uid(standards_lock_sha256);
    let document_bytes = closed_tetrahedron_binary_stl();

    let relative_path = format!("{}/instance.dcm", recipe.case_id);
    let path = run.out_dir.join(&relative_path);
    let case_dir = path.parent().ok_or_else(|| GenerateError::MetadataShape {
        path: PathBuf::from(&relative_path),
        message: "generated Encapsulated STL path must have a parent directory",
    })?;
    fs::create_dir_all(case_dir).map_err(|source| GenerateError::CreateCaseOutputDir {
        path: case_dir.to_path_buf(),
        source,
    })?;

    let mut obj = InMemDicomObject::new_empty();
    put_str(&mut obj, tags::SPECIFIC_CHARACTER_SET, VR::CS, "ISO_IR 192");
    put_str(
        &mut obj,
        tags::SOP_CLASS_UID,
        VR::UI,
        ENCAPSULATED_STL_STORAGE_UID,
    );
    put_str(&mut obj, tags::SOP_INSTANCE_UID, VR::UI, &sop_instance_uid);
    put_str(&mut obj, tags::SYNTHETIC_DATA, VR::CS, "YES");
    put_str(&mut obj, tags::INSTANCE_CREATION_DATE, VR::DA, "20260101");
    put_str(&mut obj, tags::INSTANCE_CREATION_TIME, VR::TM, "000000");
    put_str(
        &mut obj,
        tags::INSTANCE_CREATOR_UID,
        VR::UI,
        &implementation_class_uid,
    );

    put_str(&mut obj, tags::PATIENT_NAME, VR::PN, "DTS^Synthetic^Mesh");
    put_str(&mut obj, tags::PATIENT_ID, VR::LO, "DTS-MESH-001");
    put_str(&mut obj, tags::PATIENT_BIRTH_DATE, VR::DA, "");
    put_str(&mut obj, tags::PATIENT_SEX, VR::CS, "");

    put_str(
        &mut obj,
        tags::STUDY_INSTANCE_UID,
        VR::UI,
        &study_instance_uid,
    );
    put_str(&mut obj, tags::STUDY_DATE, VR::DA, "20260101");
    put_str(&mut obj, tags::STUDY_TIME, VR::TM, "000000");
    put_str(&mut obj, tags::REFERRING_PHYSICIAN_NAME, VR::PN, "");
    put_str(&mut obj, tags::STUDY_ID, VR::SH, "DTS-MESH");
    put_str(&mut obj, tags::ACCESSION_NUMBER, VR::SH, "");

    put_str(&mut obj, tags::MODALITY, VR::CS, "M3D");
    put_str(
        &mut obj,
        tags::SERIES_INSTANCE_UID,
        VR::UI,
        &series_instance_uid,
    );
    put_str(&mut obj, tags::SERIES_NUMBER, VR::IS, "82");
    put_str(
        &mut obj,
        tags::SERIES_DESCRIPTION,
        VR::LO,
        "DTS synthetic manufacturing model",
    );

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
    put_str(&mut obj, tags::DEVICE_SERIAL_NUMBER, VR::LO, "DTS-STL-0001");
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
        tags::ACQUISITION_DATE_TIME,
        VR::DT,
        "20260101000000+0000",
    );
    put_str(&mut obj, tags::BURNED_IN_ANNOTATION, VR::CS, "NO");
    put_str(&mut obj, tags::RECOGNIZABLE_VISUAL_FEATURES, VR::CS, "NO");
    put_code_sequence(
        &mut obj,
        tags::CONCEPT_NAME_CODE_SEQUENCE,
        "129006",
        "DCM",
        "Anatomical Model",
    );
    put_str(
        &mut obj,
        tags::DOCUMENT_TITLE,
        VR::ST,
        recipe.document_title,
    );
    put_str(
        &mut obj,
        tags::MIME_TYPE_OF_ENCAPSULATED_DOCUMENT,
        VR::LO,
        STL_MIME_TYPE,
    );
    put_u32(
        &mut obj,
        tags::ENCAPSULATED_DOCUMENT_LENGTH,
        VR::UL,
        document_bytes.len() as u32,
    );
    obj.put(DataElement::new(
        tags::ENCAPSULATED_DOCUMENT,
        VR::OB,
        PrimitiveValue::from(document_bytes.as_slice()),
    ));
    put_code_sequence(
        &mut obj,
        tags::MEASUREMENT_UNITS_CODE_SEQUENCE,
        STL_UNIT_CODE_VALUE,
        STL_UNIT_CODING_SCHEME,
        STL_UNIT_CODE_MEANING,
    );
    put_str(&mut obj, tags::MODEL_MODIFICATION, VR::CS, "NO");
    put_str(&mut obj, tags::MODEL_MIRRORING, VR::CS, "NO");
    put_str(
        &mut obj,
        tags::CONTENT_DESCRIPTION,
        VR::LO,
        recipe.content_description,
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

    let validated = reopen_and_validate_encapsulated_stl(
        &path,
        &sop_instance_uid,
        &frame_of_reference_uid,
        &implementation_class_uid,
        &document_bytes,
    )?;

    Ok(GeneratedFile {
        case_id: recipe.case_id.to_string(),
        manifest_entry: encapsulated_stl_manifest_entry(
            case,
            recipe,
            &relative_path,
            &study_instance_uid,
            &series_instance_uid,
            &sop_instance_uid,
            &frame_of_reference_uid,
            &implementation_class_uid,
            &document_bytes,
            &validated.0,
            validated.1,
        ),
    })
}

fn reopen_and_validate_encapsulated_stl(
    path: &Path,
    sop_instance_uid: &str,
    frame_of_reference_uid: &str,
    implementation_class_uid: &str,
    document_bytes: &[u8],
) -> Result<(Vec<u8>, Value), GenerateError> {
    let bytes = fs::read(path).map_err(|source| GenerateError::ReadGeneratedFile {
        path: path.to_path_buf(),
        source,
    })?;
    let obj = open_file(path).map_err(|error| GenerateError::ValidateDicomFile {
        path: path.to_path_buf(),
        message: format!("reopen Encapsulated STL: {error}"),
    })?;
    let text = |tag| {
        obj.element(tag)
            .map_err(|error| GenerateError::ValidateDicomFile {
                path: path.to_path_buf(),
                message: error.to_string(),
            })?
            .to_str()
            .map(|value| value.trim_end_matches(['\0', ' ']).to_string())
            .map_err(|error| GenerateError::ValidateDicomFile {
                path: path.to_path_buf(),
                message: error.to_string(),
            })
    };
    let payload_element = obj.element(tags::ENCAPSULATED_DOCUMENT).map_err(|error| {
        GenerateError::ValidateDicomFile {
            path: path.to_path_buf(),
            message: error.to_string(),
        }
    })?;
    let payload = payload_element
        .to_bytes()
        .map_err(|error| GenerateError::ValidateDicomFile {
            path: path.to_path_buf(),
            message: error.to_string(),
        })?;
    let document_length_element =
        obj.element(tags::ENCAPSULATED_DOCUMENT_LENGTH)
            .map_err(|error| GenerateError::ValidateDicomFile {
                path: path.to_path_buf(),
                message: error.to_string(),
            })?;
    let document_length = document_length_element.to_int::<u32>().map_err(|error| {
        GenerateError::ValidateDicomFile {
            path: path.to_path_buf(),
            message: error.to_string(),
        }
    })?;

    let valid = bytes.len() >= 132
        && &bytes[128..132] == b"DICM"
        && obj.meta().transfer_syntax() == EXPLICIT_VR_LITTLE_ENDIAN.uid
        && obj.meta().media_storage_sop_class_uid() == ENCAPSULATED_STL_STORAGE_UID
        && obj.meta().media_storage_sop_instance_uid() == sop_instance_uid
        && obj.meta().implementation_class_uid() == implementation_class_uid
        && text(tags::SOP_CLASS_UID)? == ENCAPSULATED_STL_STORAGE_UID
        && text(tags::SOP_INSTANCE_UID)? == sop_instance_uid
        && text(tags::SYNTHETIC_DATA)? == "YES"
        && text(tags::MODALITY)? == "M3D"
        && text(tags::FRAME_OF_REFERENCE_UID)? == frame_of_reference_uid
        && text(tags::MIME_TYPE_OF_ENCAPSULATED_DOCUMENT)? == STL_MIME_TYPE
        && text(tags::MODEL_MODIFICATION)? == "NO"
        && text(tags::MODEL_MIRRORING)? == "NO"
        && document_length as usize == document_bytes.len()
        && payload.as_ref() == document_bytes
        && obj.element(tags::PIXEL_DATA).is_err();
    if !valid {
        return Err(GenerateError::ValidateDicomFile {
            path: path.to_path_buf(),
            message: "Encapsulated STL identity, modules, or payload differ from the locked native recipe"
                .to_string(),
        });
    }

    Ok((
        bytes,
        serde_json::json!({
            "status": "passed",
            "internal": [
                {"name": "part10_identity", "status": "passed", "message": "Part 10, SOP, transfer syntax, and deterministic implementation identity match."},
                {"name": "encapsulated_stl_modules", "status": "passed", "message": "M3D modality, Frame of Reference, manufacturing-model flags, units, and MIME type match."},
                {"name": "encapsulated_stl_payload", "status": "passed", "message": "Encapsulated Document Length and exact binary STL bytes match the locked payload."},
                {"name": "pixel_data_absent", "status": "passed", "message": "Encapsulated STL contains no Pixel Data."}
            ],
            "standards": [
                {"name": "sop_class_encapsulated_stl", "status": "passed", "message": "SOP Class UID is Encapsulated STL Storage."},
                {"name": "transfer_syntax_explicit_vr_little_endian", "status": "passed", "message": "Transfer Syntax is Explicit VR Little Endian."}
            ],
            "external": []
        }),
    ))
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
                    DataElement::new(tags::CODE_VALUE, VR::SH, "85756007"),
                    DataElement::new(tags::CODING_SCHEME_DESIGNATOR, VR::SH, "SCT"),
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
        .map(|frame_number| {
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
        PixelDataLengthFormula::YbrFull422
        | PixelDataLengthFormula::BitPackedContinuousFrames
        | PixelDataLengthFormula::Encapsulated { .. } => {
            unreachable!("segmentation recipes do not use this native frame length formula")
        }
    }
}

fn segmentation_manifest_frame_hashes(recipe: SegmentationRecipe) -> Vec<String> {
    if matches!(
        recipe.pixel_data_length_formula,
        PixelDataLengthFormula::BitPackedContinuousFrames
    ) {
        let frame_samples = usize::from(recipe.rows) * usize::from(recipe.columns);
        return recipe
            .pixel_values
            .chunks_exact(frame_samples)
            .map(|frame| {
                let decoded = frame.iter().map(|value| *value as u8).collect::<Vec<_>>();
                sha256_hex(&decoded)
            })
            .collect();
    }

    let frame_byte_len = segmentation_frame_byte_len(recipe);
    recipe
        .pixel_bytes
        .chunks(frame_byte_len)
        .map(sha256_hex)
        .collect()
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
    let frame_hashes = segmentation_manifest_frame_hashes(recipe);
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
fn encapsulated_stl_manifest_entry(
    case: &Value,
    recipe: EncapsulatedStlRecipe,
    relative_path: &str,
    study_instance_uid: &str,
    series_instance_uid: &str,
    sop_instance_uid: &str,
    frame_of_reference_uid: &str,
    implementation_class_uid: &str,
    document_bytes: &[u8],
    bytes: &[u8],
    validation: Value,
) -> Value {
    let standards_evidence = standards_evidence_from_case(case);
    let payload_sha256 = sha256_hex(document_bytes);
    serde_json::json!({
        "case_id": recipe.case_id,
        "profile_membership": ["extended"],
        "path": relative_path,
        "sha256": sha256_hex(bytes),
        "size_bytes": bytes.len(),
        "determinism": "byte_stable",
        "recipe": {
            "recipe_id": recipe.recipe_id,
            "recipe_version": ENCAPSULATED_STL_RECIPE_VERSION,
            "recipe_parameters": {
                "document_title": recipe.document_title,
                "content_description": recipe.content_description,
                "payload_format": "binary_stl",
                "payload_length": document_bytes.len(),
                "payload_sha256": payload_sha256,
                "triangle_count": STL_TRIANGLE_COUNT,
                "bounds_min": [0, 0, 0],
                "bounds_max": [10, 10, 10]
            }
        },
        "dicom": {
            "sop_class_uid": ENCAPSULATED_STL_STORAGE_UID,
            "sop_class_name": "Encapsulated STL Storage",
            "iod_name": "Encapsulated STL",
            "modality": "M3D",
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
        "references": [],
        "expected_capabilities": [
            "open_file", "read_metadata", "show_unsupported_but_recognized",
            "extract_encapsulated_document", "parse_binary_stl"
        ],
        "expected_semantics": {
            "synthetic_data": "YES",
            "encapsulated_document": {
                "document_title": recipe.document_title,
                "mime_type": STL_MIME_TYPE,
                "document_length": document_bytes.len(),
                "document_sha256": payload_sha256,
                "burned_in_annotation": "NO",
                "recognizable_visual_features": "NO"
            }
        },
        "expected_encapsulated_stl": {
            "iod_kind": "encapsulated_stl",
            "profile": "extended",
            "payload": {
                "format": "binary_stl",
                "mime_type": STL_MIME_TYPE,
                "length": STL_PAYLOAD_LEN,
                "sha256": payload_sha256,
                "triangle_count": STL_TRIANGLE_COUNT
            },
            "units": {
                "code_value": STL_UNIT_CODE_VALUE,
                "coding_scheme_designator": STL_UNIT_CODING_SCHEME,
                "code_meaning": STL_UNIT_CODE_MEANING
            },
            "geometry": {
                "bounds_min": [0, 0, 0],
                "bounds_max": [10, 10, 10],
                "closed_manifold": true,
                "outward_winding": true,
                "nondegenerate_faces": true
            },
            "independent_validator_disposition": "required"
        },
        "expected_visual_checks": {
            "pattern": "recognized_unsupported_closed_tetrahedron_manufacturing_model"
        },
        "validation": validation,
        "known_stressors": [
            "encapsulated_stl_storage", "binary_stl_payload", "closed_manifold_mesh",
            "non_image_object", "recognized_unsupported"
        ],
        "standards_evidence": deduplicated_standards_evidence(standards_evidence)
    })
}

#[allow(clippy::too_many_arguments)]
fn deterministic_case_uid_with_file_index(
    standards_lock_sha256: &str,
    recipe: PixelRecipe,
    run_seed: u64,
    role: UidRole,
    file_index: u32,
) -> String {
    deterministic_uid(&DeterministicUidInput {
        standards_lock_sha256,
        case_id: recipe.case_id,
        recipe_version: PIXEL_RECIPE_VERSION,
        run_seed,
        file_index,
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

fn deterministic_rt_plan_uid(
    standards_lock_sha256: &str,
    recipe: RtPlanRecipe,
    run_seed: u64,
    role: UidRole,
) -> String {
    deterministic_uid(&DeterministicUidInput {
        standards_lock_sha256,
        case_id: recipe.case_id,
        recipe_version: RT_PLAN_RECIPE_VERSION,
        run_seed,
        file_index: 0,
        frame_index: None,
        referenced_object_index: Some(0),
        role,
    })
}

fn deterministic_rt_image_uid(
    standards_lock_sha256: &str,
    recipe: RtImageRecipe,
    run_seed: u64,
    role: UidRole,
) -> String {
    deterministic_uid(&DeterministicUidInput {
        standards_lock_sha256,
        case_id: recipe.case_id,
        recipe_version: RT_IMAGE_RECIPE_VERSION,
        run_seed,
        file_index: 0,
        frame_index: None,
        referenced_object_index: Some(0),
        role,
    })
}

fn deterministic_rt_radiation_uid(
    standards_lock_sha256: &str,
    recipe: RtRadiationRecipe,
    run_seed: u64,
    role: UidRole,
    referenced_object_index: Option<u32>,
) -> String {
    deterministic_uid(&DeterministicUidInput {
        standards_lock_sha256,
        case_id: recipe.case_id,
        recipe_version: RT_RADIATION_RECIPE_VERSION,
        run_seed,
        file_index: 0,
        frame_index: None,
        referenced_object_index,
        role,
    })
}

fn deterministic_rt_radiation_set_uid(
    standards_lock_sha256: &str,
    recipe: RtRadiationSetRecipe,
    run_seed: u64,
    role: UidRole,
    referenced_object_index: Option<u32>,
) -> String {
    deterministic_uid(&DeterministicUidInput {
        standards_lock_sha256,
        case_id: recipe.case_id,
        recipe_version: RT_RADIATION_SET_RECIPE_VERSION,
        run_seed,
        file_index: 0,
        frame_index: None,
        referenced_object_index,
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

fn deterministic_encapsulated_stl_uid(
    standards_lock_sha256: &str,
    recipe: EncapsulatedStlRecipe,
    run_seed: u64,
    role: UidRole,
) -> String {
    deterministic_uid(&DeterministicUidInput {
        standards_lock_sha256,
        case_id: recipe.case_id,
        recipe_version: ENCAPSULATED_STL_RECIPE_VERSION,
        run_seed,
        file_index: 0,
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
    if recipe.case_id == U1_SC_RECIPE.case_id {
        return PixelDataLengthFormula::BitPackedContinuousFrames;
    }
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

    fn write_supported_cases(
        run: &PreparedGenerationRun,
        registry: &Value,
        standards_lock_sha256: &str,
    ) -> Result<GenerationOutput, GenerateError> {
        write_supported_cases_with_plan_first_sc(run, registry, standards_lock_sha256, Vec::new())
    }

    fn execute_plan_first_case(run: &PreparedGenerationRun, case_id: &str) -> GeneratedFile {
        let bundle = crate::prepare_curated_case_plan(vec![case_id.to_string()], run.seed)
            .expect("plan-first fixture should plan")
            .expect("plan-first fixture should have a corpus plan");
        fs::create_dir_all(&run.out_dir).expect("plan-first fixture staging root should exist");
        let mut files = crate::execute_curated_sc_plan(Some(&bundle), &run.out_dir)
            .expect("plan-first fixture should execute");
        assert_eq!(
            files.len(),
            1,
            "single-artifact fixture {case_id} should emit once"
        );
        files.remove(0)
    }

    #[test]
    fn curated_recipe_registry_is_complete_and_unique() {
        let actual = [
            CuratedRecipeStage::SecondaryCapture,
            CuratedRecipeStage::ClassicCt,
            CuratedRecipeStage::ClassicImagesBeforeEnhancedPet,
            CuratedRecipeStage::ClassicImagesAfterEnhancedPet,
        ]
        .into_iter()
        .flat_map(curated_recipe_registry)
        .map(CuratedStageEntry::case_id)
        .collect::<Vec<_>>();
        let unique = actual.iter().copied().collect::<BTreeSet<_>>();
        assert_eq!(unique.len(), actual.len(), "recipe IDs must be unique");

        let mut expected = BTreeSet::new();
        expected.extend(PIXEL_RECIPES.iter().map(|recipe| recipe.case_id));
        expected.insert(COLOR_SOFTCOPY_PRESENTATION_STATE_SOURCE_CASE_ID);
        expected.extend(PLAN_FIRST_NATIVE_VL_CASE_IDS.iter().copied());
        expected.extend(PLAN_FIRST_RLE_VL_CASE_IDS.iter().copied());
        expected.extend(
            METADATA_SC_RECIPES
                .iter()
                .map(|recipe| recipe.pixel.case_id),
        );
        expected.extend([
            TIMEZONE_SC_RECIPE.pixel.case_id,
            EMPTY_TYPE2_SC_RECIPE.pixel.case_id,
            STRING_BOUNDARY_SC_RECIPE.pixel.case_id,
            PRIVATE_CREATOR_SC_RECIPE.pixel.case_id,
            SEQUENCE_LENGTH_SC_RECIPE.pixel.case_id,
            NONSQUARE_SPACING_SC_RECIPE.pixel.case_id,
        ]);
        expected.extend(CLASSIC_CT_RECIPES.iter().map(|recipe| recipe.case_id));
        expected.extend(PLAN_FIRST_CLASSIC_MG_DX_NM_CASE_IDS.iter().copied());
        expected.extend(PLAN_FIRST_CLASSIC_PET_CASE_IDS.iter().copied());
        expected.extend(
            PLAN_FIRST_CLASSIC_US_MULTIFRAME_XA_XRF_CASE_IDS
                .iter()
                .copied(),
        );
        expected.extend(PLAN_FIRST_CLASSIC_US_CR_MR_CASE_IDS.iter().copied());

        assert_eq!(unique, expected);
    }

    #[test]
    fn classic_curated_stages_are_plan_first_only() {
        let expected = [
            (
                CuratedRecipeStage::ClassicCt,
                CLASSIC_CT_RECIPES
                    .iter()
                    .map(|recipe| recipe.case_id)
                    .collect::<Vec<_>>(),
            ),
            (
                CuratedRecipeStage::ClassicImagesBeforeEnhancedPet,
                PLAN_FIRST_CLASSIC_MG_DX_NM_CASE_IDS
                    .iter()
                    .copied()
                    .chain(PLAN_FIRST_CLASSIC_PET_CASE_IDS.iter().copied())
                    .collect::<Vec<_>>(),
            ),
            (
                CuratedRecipeStage::ClassicImagesAfterEnhancedPet,
                PLAN_FIRST_CLASSIC_US_MULTIFRAME_XA_XRF_CASE_IDS
                    .iter()
                    .copied()
                    .chain(PLAN_FIRST_CLASSIC_US_CR_MR_CASE_IDS.iter().copied())
                    .collect::<Vec<_>>(),
            ),
        ];
        for (stage, expected_case_ids) in expected {
            let entries = curated_recipe_registry(stage);
            assert!(!entries.is_empty());
            assert_eq!(
                entries
                    .iter()
                    .copied()
                    .map(CuratedStageEntry::case_id)
                    .collect::<Vec<_>>(),
                expected_case_ids
            );
            assert!(entries.into_iter().all(|entry| matches!(
                entry,
                CuratedStageEntry::PlanFirst(PlanFirstStageEntry {
                    stage: entry_stage,
                    ..
                }) if entry_stage == stage
            )));
        }
    }

    #[test]
    fn enhanced_plan_first_compatibility_lists_preserve_historical_order() {
        assert_eq!(
            U9_PLAN_FIRST_ENHANCED_CT_CASE_IDS,
            &["enhanced/ct/multiframe_shared_perframe_explicit_le"]
        );
        assert_eq!(
            U9_PLAN_FIRST_ENHANCED_CT_CONCATENATION_CASE_IDS,
            &["enhanced/ct/concatenation_two_part_explicit_le"]
        );
        assert_eq!(
            U9_PLAN_FIRST_ENHANCED_MR_CASE_IDS,
            &[
                "enhanced/mr/multiframe_echo_perframe_explicit_le",
                "enhanced/mr/multiframe_temporal_position_explicit_le",
                "enhanced/mr/multiframe_phase_velocity_encoding_explicit_le",
            ]
        );
        assert_eq!(
            U9_PLAN_FIRST_ENHANCED_PET_CASE_IDS,
            &["enhanced/pet/multiframe_explicit_le"]
        );
    }

    #[test]
    fn wsi_plan_first_compatibility_list_preserves_historical_order() {
        assert_eq!(
            U9_PLAN_FIRST_WSI_CASE_IDS,
            &[
                "vl/wsi/tiled_full_small",
                "vl/wsi/tiled_sparse_small",
                "vl/wsi/multiple_optical_paths",
                "vl/wsi/pyramid_multiresolution",
            ]
        );
    }

    #[test]
    fn migrated_vl_and_color_softcopy_source_symbols_are_plan_first_only() {
        assert!(PIXEL_RECIPES.iter().all(|recipe| {
            recipe.case_id != COLOR_SOFTCOPY_PRESENTATION_STATE_SOURCE_CASE_ID
                && !PLAN_FIRST_NATIVE_VL_CASE_IDS.contains(&recipe.case_id)
                && !PLAN_FIRST_RLE_VL_CASE_IDS.contains(&recipe.case_id)
        }));
        let entries = curated_recipe_registry(CuratedRecipeStage::SecondaryCapture);
        let migrated_case_ids = std::iter::once(COLOR_SOFTCOPY_PRESENTATION_STATE_SOURCE_CASE_ID)
            .chain(PLAN_FIRST_NATIVE_VL_CASE_IDS.iter().copied())
            .chain(PLAN_FIRST_RLE_VL_CASE_IDS.iter().copied())
            .collect::<Vec<_>>();
        let migrated = entries
            .iter()
            .copied()
            .filter(|entry| migrated_case_ids.contains(&entry.case_id()))
            .collect::<Vec<_>>();
        assert_eq!(migrated.len(), migrated_case_ids.len());
        for case_id in migrated_case_ids {
            let matching = migrated
                .iter()
                .copied()
                .filter(|entry| entry.case_id() == case_id)
                .collect::<Vec<_>>();
            assert!(matches!(
                matching.as_slice(),
                [CuratedStageEntry::PlanFirst(PlanFirstStageEntry {
                    stage: CuratedRecipeStage::SecondaryCapture,
                    case_id: dispatched,
                })] if *dispatched == case_id
            ));
        }
        assert!(entries.iter().any(|entry| matches!(
            entry,
            CuratedStageEntry::Legacy(CuratedRecipeImplementation::Pixel(recipe))
                if recipe.case_id != COLOR_SOFTCOPY_PRESENTATION_STATE_SOURCE_CASE_ID
        )));
    }

    #[test]
    fn missing_selected_classic_output_is_a_typed_plan_first_error() {
        let stage = CuratedRecipeStage::ClassicCt;
        let entry = curated_recipe_registry(stage)
            .into_iter()
            .next()
            .expect("classic CT dispatch entry");
        let case_id = entry.case_id();
        let registry = serde_json::json!({
            "cases": [{
                "case_id": case_id,
                "status": "implemented",
                "profiles": ["core"],
                "requirements": {"features": [], "external_codecs": [], "external_validators": []}
            }]
        });
        let run = PreparedGenerationRun {
            profile: "core".into(),
            out_dir: PathBuf::from("unused-plan-first-stage-output"),
            manifest_path: PathBuf::from("unused-plan-first-stage-output/manifest.json"),
            seed: 7,
            include_stress: false,
        };
        let error = write_curated_recipe_stage(
            &mut GenerationContext::default(),
            &run,
            &registry,
            "unused-lock-hash",
            stage,
            &mut BTreeMap::new(),
        )
        .expect_err("selected classic case must not fall back to its legacy writer");
        assert!(matches!(
            error,
            GenerateError::PlanFirst {
                stage: "classic stage dispatch",
                ref message,
            } if message.contains(case_id) && message.contains(stage.name())
        ));
        assert!(!run.out_dir.exists());
    }

    #[cfg(feature = "jpegxl")]
    #[test]
    fn jpeg_xl_lossy_writer_records_independent_bounded_metrics() {
        assert_lossy_pixel_writer(
            "classic/sc/rgb_jpegxl_lossy",
            JPEG_XL_LOSSY_TRANSFER_SYNTAX_UID,
            1_185,
            0.7918037162,
            &[8, 2, 7],
        );
    }

    #[cfg(feature = "htj2k_openjph")]
    #[test]
    fn htj2k_lossy_writer_records_independent_bounded_metrics() {
        assert_lossy_pixel_writer(
            "classic/sc/mono2_u16_htj2k_lossy",
            HTJ2K_LOSSY_TRANSFER_SYNTAX_UID,
            1_476,
            4.3548643779,
            &[19],
        );
    }

    #[cfg(any(feature = "jpegxl", feature = "htj2k_openjph"))]
    fn assert_lossy_pixel_writer(
        case_id: &str,
        transfer_syntax_uid: &str,
        compressed_bytes: u64,
        overall_rmse: f64,
        channel_maxima: &[u64],
    ) {
        let output = ParametricMapStagingGuard::new();
        let run = PreparedGenerationRun {
            profile: "extended".to_string(),
            out_dir: output.path().to_path_buf(),
            manifest_path: output.path().join("manifest.json"),
            seed: 7,
            include_stress: false,
        };
        let recipe = PIXEL_RECIPES
            .iter()
            .copied()
            .find(|recipe| recipe.case_id == case_id)
            .expect("lossy recipe must be dispatched");
        let case = serde_json::json!({ "case_id": case_id, "standards_evidence": [] });
        let lock = sha256_hex(&fs::read("standards.lock.json").expect("standards lock"));

        let first = write_pixel_case(&run, &case, recipe, &lock)
            .expect("lossy fixture should write and independently validate");
        let output_path = output.path().join(case_id).join("instance.dcm");
        let first_bytes = fs::read(&output_path).expect("first lossy DICOM bytes");
        let second = write_pixel_case(&run, &case, recipe, &lock)
            .expect("repeated lossy fixture should write and independently validate");
        assert_eq!(
            first_bytes,
            fs::read(&output_path).expect("second lossy DICOM bytes")
        );
        assert_eq!(
            first.manifest_entry["sha256"],
            second.manifest_entry["sha256"]
        );

        let metrics = &first.manifest_entry["expected_lossy_metrics"];
        assert_eq!(metrics["compressed_bytes"], compressed_bytes);
        assert_eq!(metrics["overall_rmse"]["observed"], overall_rmse);
        assert_eq!(metrics["decoder"]["independence"], "independent");
        assert_eq!(
            metrics["channels"]
                .as_array()
                .expect("lossy channels")
                .iter()
                .map(|channel| channel["max_absolute_error"]["observed"]
                    .as_u64()
                    .expect("maximum error"))
                .collect::<Vec<_>>(),
            channel_maxima
        );
        let reopened = open_file(&output_path).expect("lossy DICOM should reopen");
        assert_eq!(reopened.meta().transfer_syntax(), transfer_syntax_uid);
        assert_eq!(
            reopened
                .element(tags::LOSSY_IMAGE_COMPRESSION)
                .expect("Lossy Image Compression")
                .to_str()
                .expect("Lossy Image Compression string"),
            "01"
        );
        let manifest_schema: Value = serde_json::from_slice(
            &fs::read("schemas/manifest.schema.json").expect("manifest schema"),
        )
        .expect("manifest schema JSON");
        let file_schema = serde_json::json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "$defs": manifest_schema["$defs"].clone(),
            "$ref": "#/$defs/file"
        });
        let validator = jsonschema::validator_for(&file_schema).expect("file schema compiles");
        let mut schema_entry = first.manifest_entry.clone();
        schema_entry["references"] = serde_json::json!([]);
        let errors = validator
            .iter_errors(&schema_entry)
            .map(|error| error.to_string())
            .collect::<Vec<_>>();
        assert!(
            errors.is_empty(),
            "lossy manifest schema errors: {errors:#?}"
        );
    }

    #[test]
    fn wsi_tile_segmentation_writer_binds_the_locked_source_frames() {
        let output = ParametricMapStagingGuard::new();
        let run = PreparedGenerationRun {
            profile: "extended".to_string(),
            out_dir: output.path().to_path_buf(),
            manifest_path: output.path().join("manifest.json"),
            seed: 7,
            include_stress: false,
        };
        let segmentation_case = serde_json::json!({
            "case_id": WSI_TILE_SEGMENTATION_CASE_ID,
            "profiles": ["extended"],
            "status": "implemented",
            "requirements": {"features": []},
            "standards_evidence": []
        });
        let standards_lock = "0000000000000000000000000000000000000000000000000000000000000000";
        let source_file = execute_plan_first_case(&run, WSI_TILE_SEGMENTATION_SOURCE_CASE_ID);
        let source = GeneratedSourceObject::from_generated_file(&source_file)
            .expect("WSI source manifest should satisfy the derived-object contract");

        let generated = match write_wsi_tile_segmentation_case(
            &run,
            &segmentation_case,
            &source,
            standards_lock,
        )
        .expect("WSI tile segmentation should invoke its backend")
        {
            WsiTileSegmentationCaseOutcome::Generated(file) => file,
            WsiTileSegmentationCaseOutcome::Unavailable(reason) => {
                assert_eq!(reason["status"], "unavailable");
                assert_eq!(reason["reason_code"], "external_backend_unavailable");
                return;
            }
        };

        assert_eq!(generated.manifest_entry["determinism"], "semantic_stable");
        assert_eq!(
            generated
                .manifest_entry
                .pointer("/expected_wsi_tile_segmentation/source/frame_numbers"),
            Some(&serde_json::json!([1, 4]))
        );
        assert_eq!(
            generated
                .manifest_entry
                .pointer("/expected_wsi_tile_segmentation/pixel_data/payload_sha256"),
            Some(&Value::from(
                crate::generation_backends::WSI_TILE_SEGMENTATION_PAYLOAD_SHA256,
            ))
        );
        assert_eq!(
            generated.manifest_entry.pointer(
                "/expected_wsi_tile_segmentation/tiling/reconstructed_total_pixel_matrix_sha256"
            ),
            Some(&Value::from(
                crate::generation_backends::WSI_TILE_SEGMENTATION_MATRIX_SHA256,
            ))
        );
        assert!(
            generated.manifest_entry["size_bytes"]
                .as_u64()
                .is_some_and(|size| size <= 16_384)
        );
        assert!(
            generated
                .manifest_entry
                .pointer("/generation_backend/invocation_elapsed_milliseconds")
                .and_then(Value::as_u64)
                .is_some_and(|milliseconds| milliseconds <= 5_000)
        );
        assert!(
            output
                .path()
                .join(WSI_TILE_SEGMENTATION_CASE_ID)
                .join(WSI_TILE_SEGMENTATION_OUTPUT_FILE)
                .is_file()
        );
    }

    #[test]
    fn advanced_blending_writer_reopens_all_sources_and_is_deterministic() {
        let output = ParametricMapStagingGuard::new();
        let run = PreparedGenerationRun {
            profile: "extended".to_string(),
            out_dir: output.path().to_path_buf(),
            manifest_path: output.path().join("manifest.json"),
            seed: 7,
            include_stress: false,
        };
        let source_recipe = CLASSIC_CT_RECIPES
            .iter()
            .find(|recipe| recipe.case_id == ADVANCED_BLENDING_PRESENTATION_STATE_SOURCE_CASE_ID)
            .copied()
            .expect("locked multiseries CT source recipe");
        let source_case = serde_json::json!({
            "case_id": ADVANCED_BLENDING_PRESENTATION_STATE_SOURCE_CASE_ID,
            "standards_evidence": []
        });
        let source_files = write_classic_ct_case(
            &run,
            &source_case,
            source_recipe,
            "0000000000000000000000000000000000000000000000000000000000000000",
        )
        .expect("four source CT files should write and validate");
        let sources: [GeneratedSourceObject; 4] = source_files
            .iter()
            .map(GeneratedSourceObject::from_generated_file)
            .collect::<Result<Vec<_>, _>>()
            .expect("source manifests should register")
            .try_into()
            .expect("source recipe should produce exactly four CT files");
        let advanced_case = serde_json::json!({
            "case_id": ADVANCED_BLENDING_PRESENTATION_STATE_CASE_ID,
            "standards_evidence": []
        });

        let first = write_advanced_blending_presentation_state_case(
            &run,
            &advanced_case,
            &sources,
            "0000000000000000000000000000000000000000000000000000000000000000",
        )
        .expect("Advanced Blending object should write and validate");
        let output_path = output
            .path()
            .join(ADVANCED_BLENDING_PRESENTATION_STATE_CASE_ID)
            .join(ADVANCED_BLENDING_PRESENTATION_STATE_OUTPUT_FILE);
        let first_bytes = fs::read(&output_path).expect("first Advanced Blending bytes");
        let second = write_advanced_blending_presentation_state_case(
            &run,
            &advanced_case,
            &sources,
            "0000000000000000000000000000000000000000000000000000000000000000",
        )
        .expect("repeated Advanced Blending object should validate");
        let second_bytes = fs::read(&output_path).expect("second Advanced Blending bytes");

        assert_eq!(first_bytes, second_bytes);
        assert_eq!(
            first.manifest_entry["sha256"],
            second.manifest_entry["sha256"]
        );
        assert_eq!(
            first.manifest_entry["references"].as_array().map(Vec::len),
            Some(4)
        );
        assert_eq!(
            first.manifest_entry.pointer(
                "/expected_advanced_blending_presentation_state/display_operation/input_numbers"
            ),
            Some(&serde_json::json!([1, 2]))
        );
        assert!(
            first
                .manifest_entry
                .pointer("/validation/internal")
                .and_then(Value::as_array)
                .expect("internal validation evidence")
                .iter()
                .any(|check| check.get("name").and_then(Value::as_str)
                    == Some("advanced_blending_source_precheck"))
        );
        let manifest_schema: Value =
            serde_json::from_str(include_str!("../schemas/manifest.schema.json"))
                .expect("manifest schema should parse");
        let file_schema = serde_json::json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "$ref": "#/$defs/file",
            "$defs": manifest_schema["$defs"].clone(),
        });
        let validator =
            jsonschema::validator_for(&file_schema).expect("file manifest schema should compile");
        assert!(
            validator.is_valid(&first.manifest_entry),
            "Advanced Blending manifest entry should satisfy schema: {:?}",
            validator
                .iter_errors(&first.manifest_entry)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn blending_writer_reopens_all_sources_and_is_deterministic() {
        let output = ParametricMapStagingGuard::new();
        let run = PreparedGenerationRun {
            profile: "extended".to_string(),
            out_dir: output.path().to_path_buf(),
            manifest_path: output.path().join("manifest.json"),
            seed: 7,
            include_stress: false,
        };
        let source_recipe = CLASSIC_CT_RECIPES
            .iter()
            .find(|recipe| recipe.case_id == BLENDING_PRESENTATION_STATE_SOURCE_CASE_ID)
            .copied()
            .expect("locked multiseries CT source recipe");
        let source_case = serde_json::json!({
            "case_id": BLENDING_PRESENTATION_STATE_SOURCE_CASE_ID,
            "standards_evidence": []
        });
        let source_files = write_classic_ct_case(
            &run,
            &source_case,
            source_recipe,
            "0000000000000000000000000000000000000000000000000000000000000000",
        )
        .expect("four source CT files should write and validate");
        let sources: [GeneratedSourceObject; 4] = source_files
            .iter()
            .map(GeneratedSourceObject::from_generated_file)
            .collect::<Result<Vec<_>, _>>()
            .expect("source manifests should register")
            .try_into()
            .expect("source recipe should produce exactly four CT files");
        let blending_case = serde_json::json!({
            "case_id": BLENDING_PRESENTATION_STATE_CASE_ID,
            "standards_evidence": []
        });

        let first = write_blending_presentation_state_case(
            &run,
            &blending_case,
            &sources,
            "0000000000000000000000000000000000000000000000000000000000000000",
        )
        .expect("Blending object should write and validate");
        let output_path = output
            .path()
            .join(BLENDING_PRESENTATION_STATE_CASE_ID)
            .join(BLENDING_PRESENTATION_STATE_OUTPUT_FILE);
        let first_bytes = fs::read(&output_path).expect("first Blending bytes");
        let second = write_blending_presentation_state_case(
            &run,
            &blending_case,
            &sources,
            "0000000000000000000000000000000000000000000000000000000000000000",
        )
        .expect("repeated Blending object should validate");
        let second_bytes = fs::read(&output_path).expect("second Blending bytes");

        assert_eq!(first_bytes, second_bytes);
        assert_eq!(
            first.manifest_entry["sha256"],
            second.manifest_entry["sha256"]
        );
        assert_eq!(
            first.manifest_entry["references"].as_array().map(Vec::len),
            Some(4)
        );
        assert_eq!(
            first
                .manifest_entry
                .pointer("/expected_blending_presentation_state/relative_opacity"),
            Some(&Value::from(0.5))
        );
        assert_eq!(
            first.manifest_entry.pointer(
                "/expected_blending_presentation_state/blending_items/1/blending_position"
            ),
            Some(&Value::from("SUPERIMPOSED"))
        );
        assert!(
            first
                .manifest_entry
                .pointer("/validation/internal")
                .and_then(Value::as_array)
                .expect("internal validation evidence")
                .iter()
                .any(|check| check.get("name").and_then(Value::as_str)
                    == Some("blending_source_precheck"))
        );
        let manifest_schema: Value =
            serde_json::from_str(include_str!("../schemas/manifest.schema.json"))
                .expect("manifest schema should parse");
        let file_schema = serde_json::json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "$ref": "#/$defs/file",
            "$defs": manifest_schema["$defs"].clone(),
        });
        let validator =
            jsonschema::validator_for(&file_schema).expect("file manifest schema should compile");
        assert!(
            validator.is_valid(&first.manifest_entry),
            "Blending manifest entry should satisfy schema: {:?}",
            validator
                .iter_errors(&first.manifest_entry)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn twelve_lead_ecg_writer_is_byte_deterministic_and_schema_valid() {
        let output = ParametricMapStagingGuard::new();
        let run = PreparedGenerationRun {
            profile: "extended".to_string(),
            out_dir: output.path().to_path_buf(),
            manifest_path: output.path().join("manifest.json"),
            seed: 7,
            include_stress: false,
        };
        let case = serde_json::json!({
            "case_id": TWELVE_LEAD_ECG_CASE_ID,
            "standards_evidence": []
        });
        let standards_lock = "0000000000000000000000000000000000000000000000000000000000000000";

        let first = write_twelve_lead_ecg_case(&run, &case, standards_lock)
            .expect("Twelve-lead ECG should write and validate");
        let output_path = output
            .path()
            .join(TWELVE_LEAD_ECG_CASE_ID)
            .join(TWELVE_LEAD_ECG_OUTPUT_FILE);
        let first_bytes = fs::read(&output_path).expect("first Twelve-lead ECG bytes");
        let second = write_twelve_lead_ecg_case(&run, &case, standards_lock)
            .expect("repeated Twelve-lead ECG should validate");
        let second_bytes = fs::read(&output_path).expect("second Twelve-lead ECG bytes");

        assert_eq!(first_bytes, second_bytes);
        assert_eq!(
            first.manifest_entry["sha256"],
            second.manifest_entry["sha256"]
        );
        assert_eq!(first.manifest_entry["image"], Value::Null);
        assert_eq!(first.manifest_entry["pixel_data"], Value::Null);
        assert_eq!(
            first
                .manifest_entry
                .pointer("/expected_waveform/multiplex_groups/0/channels")
                .and_then(Value::as_array)
                .map(Vec::len),
            Some(12)
        );
        assert_eq!(
            first
                .manifest_entry
                .pointer("/expected_waveform/multiplex_groups/0/storage/payload_sha256"),
            Some(&Value::from(
                "98b7a9b1be25d9d64ffa75bc6e16ea80f60deed1891aeed8dfb440c1c19e6713"
            ))
        );
        assert!(
            first
                .manifest_entry
                .pointer("/validation/internal")
                .and_then(Value::as_array)
                .expect("internal waveform validation evidence")
                .iter()
                .any(|check| check.get("name").and_then(Value::as_str)
                    == Some("twelve_lead_ecg_formula_and_interleave"))
        );

        let manifest_schema: Value =
            serde_json::from_str(include_str!("../schemas/manifest.schema.json"))
                .expect("manifest schema should parse");
        let file_schema = serde_json::json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "$ref": "#/$defs/file",
            "$defs": manifest_schema["$defs"].clone(),
        });
        let validator =
            jsonschema::validator_for(&file_schema).expect("file manifest schema should compile");
        assert!(
            validator.is_valid(&first.manifest_entry),
            "Twelve-lead ECG manifest entry should satisfy schema: {:?}",
            validator
                .iter_errors(&first.manifest_entry)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn general_ecg_writer_is_byte_deterministic_and_schema_valid() {
        let output = ParametricMapStagingGuard::new();
        let run = PreparedGenerationRun {
            profile: "extended".to_string(),
            out_dir: output.path().to_path_buf(),
            manifest_path: output.path().join("manifest.json"),
            seed: 7,
            include_stress: false,
        };
        let case = serde_json::json!({
            "case_id": GENERAL_ECG_CASE_ID,
            "profiles": ["extended"],
            "status": "implemented",
            "requirements": {"features": []},
            "standards_evidence": []
        });
        let standards_lock = "0000000000000000000000000000000000000000000000000000000000000000";

        assert!(
            should_generate_case(&case, &run).expect("implemented case should be well formed"),
            "General ECG must be included in extended generation after registry promotion"
        );

        let first = write_general_ecg_case(&run, &case, standards_lock)
            .expect("General ECG should write and validate directly");
        let output_path = output
            .path()
            .join(GENERAL_ECG_CASE_ID)
            .join(GENERAL_ECG_OUTPUT_FILE);
        let first_bytes = fs::read(&output_path).expect("first General ECG bytes");
        let second = write_general_ecg_case(&run, &case, standards_lock)
            .expect("repeated General ECG should validate");
        let second_bytes = fs::read(&output_path).expect("second General ECG bytes");

        assert_eq!(first_bytes, second_bytes);
        assert_eq!(
            first.manifest_entry["sha256"],
            Value::from(sha256_hex(&first_bytes))
        );
        assert_eq!(
            first.manifest_entry["sha256"],
            second.manifest_entry["sha256"]
        );
        assert_eq!(first.manifest_entry["image"], Value::Null);
        assert_eq!(first.manifest_entry["pixel_data"], Value::Null);

        let groups = first
            .manifest_entry
            .pointer("/expected_waveform/multiplex_groups")
            .and_then(Value::as_array)
            .expect("General ECG ordered multiplex groups");
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0]["label"], "STD12_250HZ");
        assert_eq!(groups[0]["channel_count"], 12);
        assert_eq!(groups[0]["samples_per_channel"], 1_000);
        assert_eq!(groups[0]["sampling_frequency_hz"], 250);
        assert_eq!(groups[0]["storage"]["payload_length_bytes"], 24_000);
        assert_eq!(
            groups[0]["storage"]["payload_sha256"],
            "e4bfb8a3290d9057fa5f5935fa6960ce2a44a07f18991d28c190522739008dbb"
        );
        assert_eq!(groups[1]["label"], "AUX4_1000HZ");
        assert_eq!(groups[1]["channel_count"], 4);
        assert_eq!(groups[1]["samples_per_channel"], 4_000);
        assert_eq!(groups[1]["sampling_frequency_hz"], 1_000);
        assert_eq!(groups[1]["storage"]["payload_length_bytes"], 32_000);
        assert_eq!(
            groups[1]["storage"]["payload_sha256"],
            "5b201d4fa7274ba36d6f7387c3d0217e1b5da161a915f983c2b63b995dde7bbe"
        );
        assert_eq!(
            first.manifest_entry.pointer("/expected_waveform/aggregate"),
            Some(&serde_json::json!({
                "group_count": 2,
                "total_channel_count": GENERAL_ECG_TOTAL_CHANNEL_COUNT,
                "common_duration_seconds": 4,
                "total_payload_length_bytes": GENERAL_ECG_TOTAL_PAYLOAD_LENGTH,
                "group_payload_sha256": [
                    "e4bfb8a3290d9057fa5f5935fa6960ce2a44a07f18991d28c190522739008dbb",
                    "5b201d4fa7274ba36d6f7387c3d0217e1b5da161a915f983c2b63b995dde7bbe"
                ],
                "aggregate_payload_sha256": GENERAL_ECG_AGGREGATE_SHA256
            }))
        );
        assert!(
            first
                .manifest_entry
                .pointer("/validation/internal")
                .and_then(Value::as_array)
                .expect("internal General ECG validation evidence")
                .iter()
                .any(|check| check.get("name").and_then(Value::as_str)
                    == Some("general_ecg_formula_and_interleave"))
        );
        assert!(
            first
                .manifest_entry
                .pointer("/validation/internal")
                .and_then(Value::as_array)
                .expect("internal General ECG validation evidence")
                .iter()
                .any(|check| check.get("name").and_then(Value::as_str)
                    == Some("general_ecg_aggregate_payload_sha256"))
        );

        let manifest_schema: Value =
            serde_json::from_str(include_str!("../schemas/manifest.schema.json"))
                .expect("manifest schema 0.2 should parse");
        let file_schema = serde_json::json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "$ref": "#/$defs/file",
            "$defs": manifest_schema["$defs"].clone(),
        });
        let validator =
            jsonschema::validator_for(&file_schema).expect("file manifest schema should compile");
        assert!(
            validator.is_valid(&first.manifest_entry),
            "General ECG manifest entry should satisfy schema 0.2: {:?}",
            validator
                .iter_errors(&first.manifest_entry)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn rt_plan_writer_links_generated_sources_and_is_byte_deterministic() {
        let output = ParametricMapStagingGuard::new();
        let run = PreparedGenerationRun {
            profile: "extended".to_string(),
            out_dir: output.path().to_path_buf(),
            manifest_path: output.path().join("manifest.json"),
            seed: 7,
            include_stress: false,
        };
        let standards_lock = "0000000000000000000000000000000000000000000000000000000000000000";
        let image_file = execute_plan_first_case(&run, RT_STRUCTURE_SET_SOURCE_CASE_ID);
        let image_source = GeneratedSourceObject::from_generated_file(&image_file)
            .expect("Enhanced CT should register as a source");
        let structure_case = serde_json::json!({
            "case_id": RT_PLAN_STRUCTURE_SET_SOURCE_CASE_ID,
            "standards_evidence": []
        });
        let structure_file = write_rt_structure_set_case(
            &run,
            &structure_case,
            RT_STRUCTURE_SET_RECIPES[0],
            &image_source,
            standards_lock,
        )
        .expect("linked RT Structure Set should write");
        let structure_source = GeneratedSourceObject::from_generated_file(&structure_file)
            .expect("RT Structure Set should register as a source");
        let dose_case = serde_json::json!({
            "case_id": RT_PLAN_DOSE_SOURCE_CASE_ID,
            "standards_evidence": []
        });
        let dose_file = write_rt_dose_case(
            &run,
            &dose_case,
            RT_DOSE_RECIPES[0],
            &image_source,
            &structure_source,
            standards_lock,
        )
        .expect("linked RT Dose should write");
        let dose_source = GeneratedSourceObject::from_generated_file(&dose_file)
            .expect("RT Dose should register as a source");
        let plan_case = serde_json::json!({
            "case_id": RT_PLAN_CASE_ID,
            "profiles": ["extended"],
            "status": "implemented",
            "requirements": {"features": []},
            "standards_evidence": []
        });
        assert!(
            should_generate_case(&plan_case, &run).expect("implemented Plan case should be valid")
        );
        let committed_registry: Value =
            serde_json::from_str(include_str!("../cases/registry.json"))
                .expect("committed registry should parse");
        let promoted_case = registry_case(&committed_registry, RT_PLAN_CASE_ID)
            .expect("registry shape should be valid")
            .expect("RT Plan registry row should exist");
        assert!(
            should_generate_case(promoted_case, &run).expect("promoted Plan case should be valid"),
            "the committed Plan registry row must keep the wired branch live"
        );

        let first = write_rt_plan_case(
            &run,
            &plan_case,
            RT_PLAN_RECIPES[0],
            &structure_source,
            &dose_source,
            standards_lock,
        )
        .expect("RT Plan should write and validate directly");
        let output_path = output
            .path()
            .join(RT_PLAN_CASE_ID)
            .join(RT_PLAN_OUTPUT_FILE);
        let first_bytes = fs::read(&output_path).expect("first RT Plan bytes");
        assert_eq!(
            sha256_hex(&first_bytes),
            "83c726a35cfeadbd1c4232e76824737d22a681645e984b729ea20bb1c596ee31"
        );
        let second = write_rt_plan_case(
            &run,
            &plan_case,
            RT_PLAN_RECIPES[0],
            &structure_source,
            &dose_source,
            standards_lock,
        )
        .expect("repeated RT Plan should validate");
        let second_bytes = fs::read(&output_path).expect("second RT Plan bytes");

        assert_eq!(first_bytes, second_bytes);
        assert_eq!(
            first.manifest_entry["sha256"],
            Value::from(sha256_hex(&first_bytes))
        );
        assert_eq!(
            first.manifest_entry["sha256"],
            second.manifest_entry["sha256"]
        );
        assert_eq!(first.manifest_entry["image"], Value::Null);
        assert_eq!(first.manifest_entry["pixel_data"], Value::Null);
        assert_eq!(
            first
                .manifest_entry
                .pointer("/expected_rt_plan/references/0/source_sha256"),
            Some(&Value::from(structure_source.sha256.as_str()))
        );
        assert_eq!(
            first
                .manifest_entry
                .pointer("/expected_rt_plan/references/1/source_sha256"),
            Some(&Value::from(dose_source.sha256.as_str()))
        );
        assert_eq!(
            first.manifest_entry.pointer(
                "/expected_rt_plan/beams/0/control_points/1/inherits_geometry_from_control_point"
            ),
            Some(&Value::from(0))
        );
        assert!(
            first
                .manifest_entry
                .pointer("/validation/internal")
                .and_then(Value::as_array)
                .expect("internal RT Plan validation evidence")
                .iter()
                .any(|check| check.get("name").and_then(Value::as_str)
                    == Some("rt_plan_source_precheck"))
        );

        let manifest_schema: Value =
            serde_json::from_str(include_str!("../schemas/manifest.schema.json"))
                .expect("manifest schema 0.2 should parse");
        let file_schema = serde_json::json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "$ref": "#/$defs/file",
            "$defs": manifest_schema["$defs"].clone(),
        });
        let validator =
            jsonschema::validator_for(&file_schema).expect("file manifest schema should compile");
        assert!(
            validator.is_valid(&first.manifest_entry),
            "RT Plan manifest entry should satisfy schema 0.2: {:?}",
            validator
                .iter_errors(&first.manifest_entry)
                .collect::<Vec<_>>()
        );

        let mut registry = GeneratedSourceRegistry::default();
        registry
            .register(&first)
            .expect("RT Plan should register for future RT Image generation");
        let registered = registry
            .first_for_case(RT_PLAN_CASE_ID)
            .expect("registered RT Plan should be discoverable by case ID");
        assert_eq!(
            registered.source_path,
            format!("{RT_PLAN_CASE_ID}/{RT_PLAN_OUTPUT_FILE}")
        );
        assert_eq!(
            registered.study_instance_uid,
            structure_source.study_instance_uid
        );
        assert_eq!(
            registered.frame_of_reference_uid,
            structure_source.frame_of_reference_uid
        );
        assert_eq!(registered.sha256, sha256_hex(&first_bytes));

        let mut tampered_structure_source = structure_source.clone();
        tampered_structure_source.sha256 = "0".repeat(64);
        let error = write_rt_plan_case(
            &run,
            &plan_case,
            RT_PLAN_RECIPES[0],
            &tampered_structure_source,
            &dose_source,
            standards_lock,
        )
        .expect_err("RT Plan writer must reject stale linked-source hashes");
        assert!(error.to_string().contains("source bytes or DICOM identity"));
    }

    #[test]
    fn rt_image_writer_links_plan_and_is_byte_deterministic() {
        let output = ParametricMapStagingGuard::new();
        let run = PreparedGenerationRun {
            profile: "extended".to_string(),
            out_dir: output.path().to_path_buf(),
            manifest_path: output.path().join("manifest.json"),
            seed: 7,
            include_stress: false,
        };
        let lock = "0000000000000000000000000000000000000000000000000000000000000000";
        let image_file = execute_plan_first_case(&run, RT_STRUCTURE_SET_SOURCE_CASE_ID);
        let image_source = GeneratedSourceObject::from_generated_file(&image_file).unwrap();
        let structure_case = serde_json::json!({"case_id": RT_PLAN_STRUCTURE_SET_SOURCE_CASE_ID, "standards_evidence": []});
        let structure_file = write_rt_structure_set_case(
            &run,
            &structure_case,
            RT_STRUCTURE_SET_RECIPES[0],
            &image_source,
            lock,
        )
        .expect("Structure Set source");
        let structure_source = GeneratedSourceObject::from_generated_file(&structure_file).unwrap();
        let dose_case =
            serde_json::json!({"case_id": RT_PLAN_DOSE_SOURCE_CASE_ID, "standards_evidence": []});
        let dose_file = write_rt_dose_case(
            &run,
            &dose_case,
            RT_DOSE_RECIPES[0],
            &image_source,
            &structure_source,
            lock,
        )
        .expect("Dose source");
        let dose_source = GeneratedSourceObject::from_generated_file(&dose_file).unwrap();
        let plan_case = serde_json::json!({"case_id": RT_PLAN_CASE_ID, "standards_evidence": []});
        let plan_file = write_rt_plan_case(
            &run,
            &plan_case,
            RT_PLAN_RECIPES[0],
            &structure_source,
            &dose_source,
            lock,
        )
        .expect("Plan source");
        let plan_source = GeneratedSourceObject::from_generated_file(&plan_file).unwrap();
        let case = serde_json::json!({
            "case_id": RT_IMAGE_CASE_ID,
            "profiles": ["extended"],
            "status": "implemented",
            "requirements": {"features": []},
            "standards_evidence": []
        });
        let committed: Value =
            serde_json::from_str(include_str!("../cases/registry.json")).unwrap();
        let promoted = registry_case(&committed, RT_IMAGE_CASE_ID)
            .unwrap()
            .unwrap();
        assert!(
            should_generate_case(promoted, &run).unwrap(),
            "the committed Image registry row must keep the wired branch live"
        );

        let first = write_rt_image_case(&run, &case, RT_IMAGE_RECIPES[0], &plan_source, lock)
            .expect("RT Image write");
        let path = output
            .path()
            .join(RT_IMAGE_CASE_ID)
            .join(RT_IMAGE_OUTPUT_FILE);
        let first_bytes = fs::read(&path).unwrap();
        let second = write_rt_image_case(&run, &case, RT_IMAGE_RECIPES[0], &plan_source, lock)
            .expect("repeat RT Image write");
        let second_bytes = fs::read(&path).unwrap();
        assert_eq!(first_bytes, second_bytes);
        assert_eq!(
            sha256_hex(&first_bytes),
            "ece52fe60b53f12c9db66d8b74c50e6450359f1da53d069adf1c4389b4c0d281"
        );
        assert_eq!(
            first.manifest_entry["sha256"],
            second.manifest_entry["sha256"]
        );
        assert_eq!(
            first
                .manifest_entry
                .pointer("/expected_rt_image/plan_reference/source_sha256"),
            Some(&Value::from(plan_source.sha256.as_str()))
        );
        assert_eq!(
            first.manifest_entry.pointer("/expected_rt_image/linkage"),
            Some(
                &serde_json::json!({"referenced_fraction_group_number": 1, "referenced_beam_number": 1})
            )
        );
        assert_eq!(
            first
                .manifest_entry
                .pointer("/expected_rt_image/storage/pixel_values"),
            Some(&serde_json::json!(RT_IMAGE_PIXEL_BYTES))
        );
        assert_eq!(
            first.manifest_entry.pointer("/pixel_data/frame_hashes/0"),
            Some(&Value::from(RT_IMAGE_PIXEL_SHA256))
        );
        assert_eq!(
            first.manifest_entry.pointer("/uids/study_instance_uid"),
            Some(&Value::from(plan_source.study_instance_uid.as_str()))
        );
        assert_eq!(
            first
                .manifest_entry
                .pointer("/uids/frame_of_reference_uid")
                .and_then(Value::as_str),
            plan_source.frame_of_reference_uid.as_deref()
        );
        assert!(
            first
                .manifest_entry
                .pointer("/validation/internal")
                .and_then(Value::as_array)
                .unwrap()
                .iter()
                .any(|check| check.get("name").and_then(Value::as_str)
                    == Some("rt_image_plan_source_precheck"))
        );

        let schema: Value =
            serde_json::from_str(include_str!("../schemas/manifest.schema.json")).unwrap();
        let file_schema = serde_json::json!({"$schema": "https://json-schema.org/draft/2020-12/schema", "$ref": "#/$defs/file", "$defs": schema["$defs"].clone()});
        let validator = jsonschema::validator_for(&file_schema).unwrap();
        assert!(
            validator.is_valid(&first.manifest_entry),
            "RT Image manifest schema errors: {:?}",
            validator
                .iter_errors(&first.manifest_entry)
                .collect::<Vec<_>>()
        );

        let mut sources = GeneratedSourceRegistry::default();
        sources.register(&first).expect("register RT Image");
        let registered = sources.first_for_case(RT_IMAGE_CASE_ID).unwrap();
        assert_eq!(registered.sha256, sha256_hex(&first_bytes));
        assert_eq!(
            registered.study_instance_uid,
            plan_source.study_instance_uid
        );
        assert_eq!(
            registered.frame_of_reference_uid,
            plan_source.frame_of_reference_uid
        );

        let mut stale_hash = plan_source.clone();
        stale_hash.sha256 = "0".repeat(64);
        let error = write_rt_image_case(&run, &case, RT_IMAGE_RECIPES[0], &stale_hash, lock)
            .expect_err("stale Plan hash must fail");
        assert!(error.to_string().contains("source bytes or DICOM identity"));
        let mut wrong_frame = plan_source.clone();
        wrong_frame.frame_of_reference_uid = Some("2.25.99999".to_string());
        let error = write_rt_image_case(&run, &case, RT_IMAGE_RECIPES[0], &wrong_frame, lock)
            .expect_err("Plan Frame of Reference drift must fail");
        assert!(error.to_string().contains("source bytes or DICOM identity"));
        let mut wrong_class = plan_source.clone();
        wrong_class.sop_class_uid = RT_IMAGE_STORAGE_UID.to_string();
        let error = write_rt_image_case(&run, &case, RT_IMAGE_RECIPES[0], &wrong_class, lock)
            .expect_err("non-Plan source must fail");
        assert!(error.to_string().contains("identity topology"));
    }

    #[test]
    fn color_softcopy_writer_reopens_source_validates_manifest_and_is_deterministic() {
        let output = ParametricMapStagingGuard::new();
        let run = PreparedGenerationRun {
            profile: "extended".to_string(),
            out_dir: output.path().to_path_buf(),
            manifest_path: output.path().join("manifest.json"),
            seed: 7,
            include_stress: false,
        };
        let source_recipe = COLOR_SOFTCOPY_PRIVATE_SOURCE_PIXEL_RECIPE;
        let source_case = serde_json::json!({
            "case_id": COLOR_SOFTCOPY_PRESENTATION_STATE_SOURCE_CASE_ID,
            "standards_evidence": []
        });
        let source_file = write_pixel_case(
            &run,
            &source_case,
            source_recipe,
            "0000000000000000000000000000000000000000000000000000000000000000",
        )
        .expect("RGB source should write and validate");
        let source = GeneratedSourceObject::from_generated_file(&source_file)
            .expect("RGB source manifest should register");
        let color_case = serde_json::json!({
            "case_id": COLOR_SOFTCOPY_PRESENTATION_STATE_CASE_ID,
            "standards_evidence": []
        });

        let first = write_color_softcopy_presentation_state_case(
            &run,
            &color_case,
            &source,
            "0000000000000000000000000000000000000000000000000000000000000000",
        )
        .expect("Color Softcopy Presentation State should write and validate");
        let output_path = output
            .path()
            .join(COLOR_SOFTCOPY_PRESENTATION_STATE_CASE_ID)
            .join(COLOR_SOFTCOPY_PRESENTATION_STATE_OUTPUT_FILE);
        let first_bytes = fs::read(&output_path).expect("first Color PR bytes");
        let second = write_color_softcopy_presentation_state_case(
            &run,
            &color_case,
            &source,
            "0000000000000000000000000000000000000000000000000000000000000000",
        )
        .expect("repeated Color Softcopy Presentation State should validate");
        let second_bytes = fs::read(&output_path).expect("second Color PR bytes");

        assert_eq!(first_bytes, second_bytes);
        assert_eq!(
            first.manifest_entry["sha256"],
            second.manifest_entry["sha256"]
        );
        assert_eq!(
            first.manifest_entry.pointer("/references/0/relationship"),
            Some(&Value::from("source_image"))
        );
        assert!(
            first
                .manifest_entry
                .pointer("/references/0/frame_numbers")
                .is_none(),
            "complete-instance relationship must not select frames"
        );
        assert_eq!(
            first
                .manifest_entry
                .pointer("/expected_color_softcopy_presentation_state/source/source_sha256"),
            Some(&Value::from(source.sha256.as_str()))
        );
        assert_eq!(
            first.manifest_entry.pointer(
                "/expected_color_softcopy_presentation_state/displayed_area/applies_to_all_references"
            ),
            Some(&Value::Bool(true))
        );
        assert!(
            first
                .manifest_entry
                .pointer("/validation/internal")
                .and_then(Value::as_array)
                .expect("internal validation evidence")
                .iter()
                .any(|check| check.get("name").and_then(Value::as_str)
                    == Some("color_softcopy_source_precheck"))
        );

        let manifest_schema: Value =
            serde_json::from_str(include_str!("../schemas/manifest.schema.json"))
                .expect("manifest schema should parse");
        let file_schema = serde_json::json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "$ref": "#/$defs/file",
            "$defs": manifest_schema["$defs"].clone(),
        });
        let validator =
            jsonschema::validator_for(&file_schema).expect("file manifest schema should compile");
        assert!(
            validator.is_valid(&first.manifest_entry),
            "Color PR manifest entry should satisfy the committed file schema: {:?}",
            validator
                .iter_errors(&first.manifest_entry)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn nonsquare_sc_writer_emits_and_reopens_independent_geometry_variants() {
        let output = ParametricMapStagingGuard::new();
        let run = PreparedGenerationRun {
            profile: "core".to_string(),
            out_dir: output.path().to_path_buf(),
            manifest_path: output.path().join("manifest.json"),
            seed: 1,
            include_stress: false,
        };
        let case = serde_json::json!({
            "case_id": NONSQUARE_SPACING_SC_RECIPE.pixel.case_id,
            "standards_evidence": []
        });

        let generated = write_nonsquare_spacing_sc_case(
            &run,
            &case,
            NONSQUARE_SPACING_SC_RECIPE,
            "0000000000000000000000000000000000000000000000000000000000000000",
        )
        .expect("non-square SC variants should write, reopen, and validate");

        assert_eq!(generated.len(), 2);
        assert_eq!(
            generated[0]
                .manifest_entry
                .pointer("/expected_nonsquare_spacing/variant_id"),
            Some(&Value::from("pixel_spacing"))
        );
        assert_eq!(
            generated[0]
                .manifest_entry
                .pointer("/expected_nonsquare_spacing/pixel_spacing/lexical_value"),
            Some(&Value::from("0.6\\0.3"))
        );
        assert_eq!(
            generated[1]
                .manifest_entry
                .pointer("/expected_nonsquare_spacing/variant_id"),
            Some(&Value::from("pixel_aspect_ratio"))
        );
        assert_eq!(
            generated[1]
                .manifest_entry
                .pointer("/expected_nonsquare_spacing/pixel_aspect_ratio/lexical_value"),
            Some(&Value::from("2\\1"))
        );
        let spacing = open_file(
            output
                .path()
                .join("classic/sc/nonsquare_pixel_spacing/pixel-spacing.dcm"),
        )
        .expect("Pixel Spacing variant should reopen");
        assert_eq!(
            spacing
                .element(tags::PIXEL_SPACING)
                .expect("Pixel Spacing should exist")
                .to_multi_str()
                .expect("Pixel Spacing should decode")
                .as_ref(),
            &["0.6", "0.3"]
        );
        assert!(spacing.element(tags::PIXEL_ASPECT_RATIO).is_err());

        let aspect = open_file(
            output
                .path()
                .join("classic/sc/nonsquare_pixel_spacing/pixel-aspect-ratio.dcm"),
        )
        .expect("Pixel Aspect Ratio variant should reopen");
        assert_eq!(
            aspect
                .element(tags::PIXEL_ASPECT_RATIO)
                .expect("Pixel Aspect Ratio should exist")
                .to_multi_str()
                .expect("Pixel Aspect Ratio should decode")
                .as_ref(),
            &["2", "1"]
        );
        assert!(aspect.element(tags::PIXEL_SPACING).is_err());
    }

    #[test]
    fn u32_sc_writer_reopens_and_preserves_full_unsigned_range() {
        let output = ParametricMapStagingGuard::new();
        let run = PreparedGenerationRun {
            profile: "extended".to_string(),
            out_dir: output.path().to_path_buf(),
            manifest_path: output.path().join("manifest.json"),
            seed: 1,
            include_stress: false,
        };
        let case = serde_json::json!({
            "case_id": U32_SC_RECIPE.case_id,
            "standards_evidence": []
        });
        let recipe = PIXEL_RECIPES
            .iter()
            .copied()
            .find(|recipe| recipe.case_id == U32_SC_RECIPE.case_id)
            .expect("unsigned 32-bit recipe must be dispatched");

        let generated = write_pixel_case(
            &run,
            &case,
            recipe,
            "0000000000000000000000000000000000000000000000000000000000000000",
        )
        .expect("unsigned 32-bit SC fixture should write, reopen, and validate");

        assert_eq!(
            generated
                .manifest_entry
                .pointer("/recipe/recipe_parameters/pixel_values"),
            Some(&serde_json::json!([
                0_u64,
                65_535_u64,
                2_147_483_648_u64,
                4_294_967_295_u64
            ]))
        );
        assert_eq!(
            generated.manifest_entry.pointer("/image/bits_allocated"),
            Some(&Value::from(32))
        );
        assert_eq!(
            generated.manifest_entry.pointer("/pixel_data/vr"),
            Some(&Value::from("OW"))
        );
        assert_eq!(
            generated.manifest_entry.pointer("/profile_membership"),
            Some(&serde_json::json!(["extended"]))
        );
        let known_stressors = generated
            .manifest_entry
            .pointer("/known_stressors")
            .and_then(Value::as_array)
            .expect("u32 manifest entry should declare known stressors");
        assert!(known_stressors.contains(&Value::from("native_ow_pixel_data")));
        assert!(!known_stressors.contains(&Value::from("native_ob_pixel_data")));
        assert_eq!(
            generated
                .manifest_entry
                .pointer("/expected_u32_pixels/stored_values"),
            Some(&serde_json::json!([
                0_u64,
                65_535_u64,
                2_147_483_648_u64,
                4_294_967_295_u64
            ]))
        );
        assert_eq!(
            generated.manifest_entry.pointer("/validation/status"),
            Some(&Value::from("passed"))
        );
        let path = output
            .path()
            .join("classic/sc/mono2_u32_explicit_le/instance.dcm");
        let reopened = dicom_object::open_file(path).expect("unsigned 32-bit SC should reopen");
        let pixels = reopened
            .element(tags::PIXEL_DATA)
            .expect("Pixel Data should be present");
        assert_eq!(pixels.vr(), VR::OW);
        assert_eq!(
            pixels
                .value()
                .to_bytes()
                .expect("Pixel Data should be bytes"),
            U32_SC_RECIPE.pixel_bytes_le.as_slice()
        );
        let manifest_schema: Value =
            serde_json::from_str(include_str!("../schemas/manifest.schema.json"))
                .expect("manifest schema should parse");
        let file_schema = serde_json::json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "$ref": "#/$defs/file",
            "$defs": manifest_schema["$defs"].clone(),
        });
        let validator =
            jsonschema::validator_for(&file_schema).expect("file manifest schema should compile");
        let mut schema_entry = generated.manifest_entry.clone();
        schema_entry["references"] = serde_json::json!([]);
        assert!(
            validator.is_valid(&schema_entry),
            "u32 manifest entry should satisfy schema: {:?}",
            validator.iter_errors(&schema_entry).collect::<Vec<_>>()
        );
    }

    #[test]
    fn enhanced_pet_plan_first_execution_reopens_and_validates_multiframe_quantitative_fixture() {
        let output = ParametricMapStagingGuard::new();
        let run = PreparedGenerationRun {
            profile: "extended".to_string(),
            out_dir: output.path().to_path_buf(),
            manifest_path: output.path().join("manifest.json"),
            seed: 1,
            include_stress: false,
        };
        let generated = execute_plan_first_case(&run, "enhanced/pet/multiframe_explicit_le");

        assert_eq!(generated.case_id, "enhanced/pet/multiframe_explicit_le");
        assert_eq!(
            generated.manifest_entry.pointer("/dicom/sop_class_uid"),
            Some(&Value::from(uids::ENHANCED_PET_IMAGE_STORAGE))
        );
        assert_eq!(
            generated.manifest_entry.pointer("/pixel_data/frame_hashes"),
            Some(&serde_json::json!([
                "03ec353fd2407afb09c8d65712ef9aa30f03c8243f6f3f1675dca7ea5f6a4784",
                "03ec353fd2407afb09c8d65712ef9aa30f03c8243f6f3f1675dca7ea5f6a4784"
            ]))
        );
        assert_eq!(
            generated
                .manifest_entry
                .pointer("/expected_enhanced_pet/activity_values_bqml_by_frame"),
            Some(&serde_json::json!([
                [0.0, 250.0, 500.0, 1000.0],
                [0.0, 250.0, 500.0, 1000.0]
            ]))
        );
        assert_eq!(
            generated
                .manifest_entry
                .pointer("/validation/status")
                .and_then(Value::as_str),
            Some("passed")
        );
        let names = generated
            .manifest_entry
            .pointer("/validation/internal")
            .and_then(Value::as_array)
            .expect("internal checks should exist")
            .iter()
            .filter_map(|check| check.get("name").and_then(Value::as_str))
            .collect::<BTreeSet<_>>();
        for name in [
            "native_frame_hashes",
            "enhanced_pet_shared_functional_groups",
            "enhanced_pet_per_frame_functional_groups",
            "enhanced_pet_derivation_image_sequence_empty",
            "enhanced_pet_rwvm_arithmetic",
            "enhanced_pet_temporal_position",
            "enhanced_pet_in_stack_position",
            "enhanced_pet_decay_corrected",
            "enhanced_pet_attenuation_method_absent",
            "enhanced_pet_view_code_sequence",
            "enhanced_pet_view_code_value",
            "enhanced_pet_view_modifier_absent",
            "enhanced_pet_slice_progression_direction_absent",
            "enhanced_pet_total_dose_present_empty",
        ] {
            assert!(names.contains(name), "missing internal check {name}");
        }
        assert!(
            generated
                .manifest_entry
                .pointer("/validation/standards")
                .and_then(Value::as_array)
                .expect("standards checks should exist")
                .iter()
                .any(|check| check.get("name").and_then(Value::as_str)
                    == Some("enhanced_pet_image_sop_class"))
        );
        let path = output
            .path()
            .join("enhanced/pet/multiframe_explicit_le/instance.dcm");
        assert!(path.is_file());
        let reopened = dicom_object::open_file(&path).expect("Enhanced PET fixture should reopen");
        assert_eq!(
            reopened
                .element(tags::NUMBER_OF_FRAMES)
                .unwrap()
                .to_str()
                .unwrap()
                .trim(),
            "2"
        );
        let manifest_schema: Value =
            serde_json::from_str(include_str!("../schemas/manifest.schema.json"))
                .expect("manifest schema should parse");
        let file_schema = serde_json::json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "$ref": "#/$defs/file",
            "$defs": manifest_schema["$defs"].clone(),
        });
        let validator =
            jsonschema::validator_for(&file_schema).expect("file manifest schema should compile");
        assert!(
            validator.is_valid(&generated.manifest_entry),
            "Enhanced PET manifest entry should satisfy the committed schema: {:?}",
            validator
                .iter_errors(&generated.manifest_entry)
                .collect::<Vec<_>>()
        );
    }

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
            TransferSyntaxSpec {
                capability_keyword: "ImplicitVRLittleEndian",
                capability_name: "Implicit VR Little Endian: Default Transfer Syntax for DICOM",
                uid: uids::IMPLICIT_VR_LITTLE_ENDIAN,
                name: "Implicit VR Little Endian",
            },
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
        assert_eq!(
            source.sha256,
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
        );
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
        assert_eq!(context.into_output().files.len(), 2);
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

    #[test]
    fn rt_radiation_pair_writers_are_linked_byte_stable_and_schema_valid() {
        let output = ParametricMapStagingGuard::new();
        let run = PreparedGenerationRun {
            profile: "extended".to_string(),
            out_dir: output.path().to_path_buf(),
            manifest_path: output.path().join("manifest.json"),
            seed: 7,
            include_stress: false,
        };
        let lock = "0000000000000000000000000000000000000000000000000000000000000000";
        let plan_source = rt_plan_source_for_radiation_tests(&run, lock);
        let radiation_case = serde_json::json!({
            "case_id": RT_RADIATION_CASE_ID,
            "standards_evidence": []
        });
        let first_radiation = write_rt_radiation_case(
            &run,
            &radiation_case,
            RT_RADIATION_RECIPES[0],
            &plan_source,
            lock,
        )
        .expect("C-Arm RT Radiation should write");
        let radiation_path = output
            .path()
            .join(RT_RADIATION_CASE_ID)
            .join(RT_RADIATION_OUTPUT_FILE);
        let first_radiation_bytes = fs::read(&radiation_path).expect("first Radiation bytes");
        assert_eq!(
            sha256_hex(&first_radiation_bytes),
            "cb58e637c9dafd4b124cbbfd3c757bdca04bd5bdb6ca61670c69d47bb32a4087"
        );
        let second_radiation = write_rt_radiation_case(
            &run,
            &radiation_case,
            RT_RADIATION_RECIPES[0],
            &plan_source,
            lock,
        )
        .expect("repeated C-Arm RT Radiation should write");
        let second_radiation_bytes = fs::read(&radiation_path).expect("second Radiation bytes");
        assert_eq!(first_radiation_bytes, second_radiation_bytes);
        assert_eq!(
            first_radiation.manifest_entry["sha256"],
            second_radiation.manifest_entry["sha256"]
        );
        assert_eq!(
            first_radiation
                .manifest_entry
                .pointer("/expected_rt_radiation/definition_source/source_sha256"),
            Some(&Value::from(plan_source.sha256.as_str()))
        );
        assert_eq!(
            first_radiation.manifest_entry.pointer(
                "/expected_rt_radiation/control_points/1/inherits_geometry_from_control_point"
            ),
            Some(&Value::from(1))
        );

        let radiation_source = GeneratedSourceObject::from_generated_file(&first_radiation)
            .expect("C-Arm RT Radiation should register as a source");
        let set_case = serde_json::json!({
            "case_id": RT_RADIATION_SET_CASE_ID,
            "standards_evidence": []
        });
        let first_set = write_rt_radiation_set_case(
            &run,
            &set_case,
            RT_RADIATION_SET_RECIPES[0],
            &plan_source,
            &radiation_source,
            lock,
        )
        .expect("RT Radiation Set should write");
        let set_path = output
            .path()
            .join(RT_RADIATION_SET_CASE_ID)
            .join(RT_RADIATION_SET_OUTPUT_FILE);
        let first_set_bytes = fs::read(&set_path).expect("first Radiation Set bytes");
        assert_eq!(
            sha256_hex(&first_set_bytes),
            "92485651db871d150d374ddccb913d46665d49118eebcc56da992e8379bb18a7"
        );
        let second_set = write_rt_radiation_set_case(
            &run,
            &set_case,
            RT_RADIATION_SET_RECIPES[0],
            &plan_source,
            &radiation_source,
            lock,
        )
        .expect("repeated RT Radiation Set should write");
        let second_set_bytes = fs::read(&set_path).expect("second Radiation Set bytes");
        assert_eq!(first_set_bytes, second_set_bytes);
        assert_eq!(
            first_set.manifest_entry["sha256"],
            second_set.manifest_entry["sha256"]
        );
        assert_eq!(
            first_set
                .manifest_entry
                .pointer("/expected_rt_radiation_set/definition_source/source_sha256"),
            Some(&Value::from(plan_source.sha256.as_str()))
        );
        assert_eq!(
            first_set
                .manifest_entry
                .pointer("/expected_rt_radiation_set/radiation_references/0/source_sha256"),
            Some(&Value::from(radiation_source.sha256.as_str()))
        );
        assert_eq!(
            first_set
                .manifest_entry
                .pointer("/references/0/relationship"),
            Some(&Value::from("definition_source"))
        );
        assert_eq!(
            first_set
                .manifest_entry
                .pointer("/references/1/relationship"),
            Some(&Value::from("referenced_rt_radiation"))
        );

        let manifest_schema: Value =
            serde_json::from_str(include_str!("../schemas/manifest.schema.json"))
                .expect("manifest schema should parse");
        let file_schema = serde_json::json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "$ref": "#/$defs/file",
            "$defs": manifest_schema["$defs"].clone(),
        });
        let validator =
            jsonschema::validator_for(&file_schema).expect("file manifest schema should compile");
        for entry in [&first_radiation.manifest_entry, &first_set.manifest_entry] {
            assert!(
                validator.is_valid(entry),
                "second-generation RT manifest entry should satisfy schema: {:?}",
                validator.iter_errors(entry).collect::<Vec<_>>()
            );
        }

        let mut stale_plan = plan_source.clone();
        stale_plan.sha256 = "0".repeat(64);
        let error = write_rt_radiation_case(
            &run,
            &radiation_case,
            RT_RADIATION_RECIPES[0],
            &stale_plan,
            lock,
        )
        .expect_err("stale Plan hash must fail before Radiation construction");
        assert!(error.to_string().contains("source bytes or DICOM identity"));

        let mut stale_radiation = radiation_source.clone();
        stale_radiation.sha256 = "0".repeat(64);
        let error = write_rt_radiation_set_case(
            &run,
            &set_case,
            RT_RADIATION_SET_RECIPES[0],
            &plan_source,
            &stale_radiation,
            lock,
        )
        .expect_err("stale Radiation hash must fail before Set construction");
        assert!(error.to_string().contains("source bytes or DICOM identity"));
    }

    fn rt_plan_source_for_radiation_tests(
        run: &PreparedGenerationRun,
        lock: &str,
    ) -> GeneratedSourceObject {
        let image_file = execute_plan_first_case(run, RT_STRUCTURE_SET_SOURCE_CASE_ID);
        let image_source = GeneratedSourceObject::from_generated_file(&image_file)
            .expect("Enhanced CT should register");
        let structure_case = serde_json::json!({
            "case_id": RT_PLAN_STRUCTURE_SET_SOURCE_CASE_ID,
            "standards_evidence": []
        });
        let structure_file = write_rt_structure_set_case(
            run,
            &structure_case,
            RT_STRUCTURE_SET_RECIPES[0],
            &image_source,
            lock,
        )
        .expect("RT Structure Set source should write");
        let structure_source = GeneratedSourceObject::from_generated_file(&structure_file)
            .expect("RT Structure Set should register");
        let dose_case = serde_json::json!({
            "case_id": RT_PLAN_DOSE_SOURCE_CASE_ID,
            "standards_evidence": []
        });
        let dose_file = write_rt_dose_case(
            run,
            &dose_case,
            RT_DOSE_RECIPES[0],
            &image_source,
            &structure_source,
            lock,
        )
        .expect("RT Dose source should write");
        let dose_source = GeneratedSourceObject::from_generated_file(&dose_file)
            .expect("RT Dose should register");
        let plan_case = serde_json::json!({
            "case_id": RT_PLAN_CASE_ID,
            "standards_evidence": []
        });
        let plan_file = write_rt_plan_case(
            run,
            &plan_case,
            RT_PLAN_RECIPES[0],
            &structure_source,
            &dose_source,
            lock,
        )
        .expect("RT Plan source should write");
        GeneratedSourceObject::from_generated_file(&plan_file).expect("RT Plan should register")
    }

    #[test]
    fn encapsulated_stl_writer_reopens_exact_mesh_and_is_byte_stable() {
        let first_root = ParametricMapStagingGuard::new();
        let second_root = ParametricMapStagingGuard::new();
        let run = |root: &ParametricMapStagingGuard| PreparedGenerationRun {
            profile: "extended".to_string(),
            out_dir: root.path().to_path_buf(),
            manifest_path: root.path().join("manifest.json"),
            seed: 7,
            include_stress: false,
        };
        let case = serde_json::json!({
            "case_id": "derived/mesh/encapsulated_stl",
            "standards_evidence": []
        });
        let lock = "0000000000000000000000000000000000000000000000000000000000000000";

        let first = write_encapsulated_stl_case(
            &run(&first_root),
            &case,
            ENCAPSULATED_STL_RECIPES[0],
            lock,
        )
        .expect("first Encapsulated STL should write and reopen");
        let second = write_encapsulated_stl_case(
            &run(&second_root),
            &case,
            ENCAPSULATED_STL_RECIPES[0],
            lock,
        )
        .expect("second Encapsulated STL should write and reopen");

        assert_eq!(
            first.manifest_entry["sha256"],
            second.manifest_entry["sha256"]
        );
        assert_eq!(
            first.manifest_entry["expected_encapsulated_stl"],
            second.manifest_entry["expected_encapsulated_stl"]
        );
        assert_eq!(
            first
                .manifest_entry
                .pointer("/expected_encapsulated_stl/payload/sha256"),
            Some(&Value::String(
                "3c3049d231f8e98c0d2fe7cb81cf6805141bcac39dd04b9cf7f8063ec44bbfb2".to_string()
            ))
        );
        assert_eq!(
            first
                .manifest_entry
                .pointer("/expected_encapsulated_stl/payload/triangle_count"),
            Some(&Value::from(4))
        );
        assert_eq!(
            fs::read(
                first_root
                    .path()
                    .join("derived/mesh/encapsulated_stl/instance.dcm")
            )
            .unwrap(),
            fs::read(
                second_root
                    .path()
                    .join("derived/mesh/encapsulated_stl/instance.dcm")
            )
            .unwrap()
        );

        let mut context = GenerationContext::default();
        let (request, started) = context
            .preflight_stress(
                StressRecipeKind::EnhancedCt,
                8 * 1024 * 1024,
                128 * 1024 * 1024,
            )
            .unwrap();
        context
            .record_stress_files(STRESS_ENHANCED_CT_CASE_ID, request, started, vec![first])
            .unwrap();
        let output = context.into_output();
        assert_eq!(output.files.len(), 1);
        assert_eq!(output.qualifications.len(), 1);
        assert_eq!(output.qualifications[0]["recipe"], "enhanced_ct");
        assert_eq!(output.qualifications[0]["requested"]["frames"], 256);
        assert_eq!(
            output.qualifications[0]["actual"]["output_bytes"],
            output.files[0].manifest_entry["size_bytes"]
        );
    }

    #[test]
    fn eot_sc_writer_reopens_exact_tables_and_empty_basic_offset_table() {
        let output = ParametricMapStagingGuard::new();
        let run = PreparedGenerationRun {
            profile: "extended".to_string(),
            out_dir: output.path().to_path_buf(),
            manifest_path: output.path().join("manifest.json"),
            seed: 7,
            include_stress: false,
        };
        let case = serde_json::json!({
            "case_id": EOT_CASE_ID,
            "profiles": ["extended"],
            "status": "implemented",
            "requirements": {"features": []},
            "standards_evidence": []
        });
        let recipe = PIXEL_RECIPES
            .iter()
            .copied()
            .find(|recipe| recipe.case_id == EOT_CASE_ID)
            .expect("EOT SC recipe must be dispatched");

        let generated = write_pixel_case(
            &run,
            &case,
            recipe,
            "0000000000000000000000000000000000000000000000000000000000000000",
        )
        .expect("EOT SC fixture should write, reopen, decode, and validate");

        assert_eq!(
            generated
                .manifest_entry
                .pointer("/pixel_data/encapsulated_pixel_data/extended_offset_table/offsets"),
            Some(&serde_json::json!([0, 78, 152]))
        );
        assert_eq!(
            generated
                .manifest_entry
                .pointer("/pixel_data/encapsulated_pixel_data/extended_offset_table/lengths"),
            Some(&serde_json::json!([69, 66, 69]))
        );
        assert_eq!(
            generated
                .manifest_entry
                .pointer("/pixel_data/encapsulated_pixel_data/basic_offset_table/offsets"),
            Some(&serde_json::json!([]))
        );
        assert_eq!(
            generated.manifest_entry.pointer("/expected_eot"),
            Some(&serde_json::json!({
                "origin": "first_fragment_item_tag",
                "item_header_bytes": 8,
                "frame_encoded_lengths": [69, 66, 69],
                "offsets": [0, 78, 152],
                "lengths": [69, 66, 69]
            }))
        );
        assert_eq!(
            generated.manifest_entry.pointer("/dicom/sop_class_uid"),
            Some(&Value::from(
                uids::MULTI_FRAME_GRAYSCALE_BYTE_SECONDARY_CAPTURE_IMAGE_STORAGE
            ))
        );
        assert_eq!(
            generated
                .manifest_entry
                .pointer("/dicom/transfer_syntax_uid"),
            Some(&Value::from(RLE_LOSSLESS.uid))
        );
        assert_eq!(
            generated.manifest_entry.pointer("/validation/status"),
            Some(&Value::from("passed"))
        );
    }

    #[test]
    fn negative_profile_is_deterministic_isolated_and_removes_private_sources() {
        fn negative_registry() -> Value {
            let mut registry: Value = serde_json::from_str(include_str!("../cases/registry.json"))
                .expect("registry should parse");
            for case in registry["cases"]
                .as_array_mut()
                .expect("registry cases should be an array")
            {
                if case["case_id"]
                    .as_str()
                    .is_some_and(|case_id| case_id.starts_with("negative/"))
                {
                    case["status"] = Value::from("implemented");
                    case["roadmap"] = Value::Null;
                    case["blockers"] = serde_json::json!([]);
                }
            }
            registry
        }

        let registry = negative_registry();
        let manifest_schema: Value =
            serde_json::from_str(include_str!("../schemas/manifest.schema.json"))
                .expect("manifest schema should parse");
        let negative_file_schema = serde_json::json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "$ref": "#/$defs/file",
            "$defs": manifest_schema["$defs"].clone()
        });
        let negative_file_validator = jsonschema::validator_for(&negative_file_schema)
            .expect("negative file schema should compile");
        let first_root = ParametricMapStagingGuard::new();
        let second_root = ParametricMapStagingGuard::new();
        let run = |root: &Path, profile: &str| PreparedGenerationRun {
            profile: profile.to_string(),
            out_dir: root.to_path_buf(),
            manifest_path: root.join("manifest.json"),
            seed: 7,
            include_stress: false,
        };
        let standards_lock = "0000000000000000000000000000000000000000000000000000000000000000";
        let first = write_supported_cases(
            &run(first_root.path(), "negative"),
            &registry,
            standards_lock,
        )
        .expect("first negative run should generate");
        let second = write_supported_cases(
            &run(second_root.path(), "negative"),
            &registry,
            standards_lock,
        )
        .expect("second negative run should generate");

        assert_eq!(first.files.len(), NEGATIVE_CASE_IDS.len());
        assert_eq!(first.files.len(), second.files.len());
        assert!(!first_root.path().join(".negative-private-sources").exists());
        assert!(
            !second_root
                .path()
                .join(".negative-private-sources")
                .exists()
        );
        for (first_file, second_file) in first.files.iter().zip(&second.files) {
            assert_eq!(first_file.case_id, second_file.case_id);
            assert_eq!(first_file.manifest_entry, second_file.manifest_entry);
            let relative_path = first_file.manifest_entry["path"]
                .as_str()
                .expect("negative path should be a string");
            assert_eq!(
                fs::read(first_root.path().join(relative_path)).unwrap(),
                fs::read(second_root.path().join(relative_path)).unwrap()
            );
            assert_eq!(
                first_file.manifest_entry["validity"],
                Value::from("expected_invalid")
            );
            assert_eq!(
                first_file.manifest_entry["negative_evidence"]["final_sha256"],
                first_file.manifest_entry["sha256"]
            );
            assert!(first_file.manifest_entry.get("dicom").is_none());
            assert!(first_file.manifest_entry.get("validation").is_none());
            let schema_errors = negative_file_validator
                .iter_errors(&first_file.manifest_entry)
                .map(|error| error.to_string())
                .collect::<Vec<_>>();
            assert!(
                schema_errors.is_empty(),
                "{} negative manifest schema errors: {schema_errors:#?}",
                first_file.case_id
            );
        }
        assert_eq!(
            first.files[0].manifest_entry["negative_evidence"]["mutation_steps"][0]["ordinal"],
            Value::from(1)
        );
        assert!(first.files.iter().all(|file| {
            file.manifest_entry["negative_evidence"]["source"]["case_id"]
                .as_str()
                .is_some_and(|source_case_id| !source_case_id.starts_with("negative/"))
        }));

        let negative_only_registry = serde_json::json!({
            "cases": registry["cases"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|case| case["case_id"].as_str().is_some_and(|id| id.starts_with("negative/")))
                .cloned()
                .collect::<Vec<_>>()
        });
        let all_root = ParametricMapStagingGuard::new();
        let all = write_supported_cases(
            &run(all_root.path(), "all"),
            &negative_only_registry,
            standards_lock,
        )
        .expect("all profile should ignore negative-only rows");
        assert!(all.files.is_empty());
        assert!(all.unavailable_cases.is_empty());
    }

    #[test]
    fn fuzz_profile_emits_reproducible_payload_free_qualification() {
        let mut registry: Value =
            serde_json::from_str(include_str!("../cases/registry.json")).unwrap();
        let fuzz_case = registry["cases"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|case| case["case_id"] == "fuzz/parser/bounded_seed_corpus")
            .expect("fuzz registry row");
        fuzz_case["status"] = Value::String("implemented".to_string());
        fuzz_case["roadmap"] = Value::Null;
        fuzz_case["blockers"] = serde_json::json!([]);
        fuzz_case["determinism"] = Value::String("semantic_stable".to_string());

        let first_root = ParametricMapStagingGuard::new();
        let second_root = ParametricMapStagingGuard::new();
        let run = |root: &ParametricMapStagingGuard| PreparedGenerationRun {
            profile: "fuzz".to_string(),
            out_dir: root.path().to_path_buf(),
            manifest_path: root.path().join("manifest.json"),
            seed: 7,
            include_stress: false,
        };
        let standards_lock = "0000000000000000000000000000000000000000000000000000000000000000";
        let first = write_supported_cases(&run(&first_root), &registry, standards_lock)
            .expect("first bounded fuzz run");
        let second = write_supported_cases(&run(&second_root), &registry, standards_lock)
            .expect("second bounded fuzz run");

        assert!(first.files.is_empty());
        assert!(second.files.is_empty());
        assert_eq!(
            first.completed_case_ids,
            vec!["fuzz/parser/bounded_seed_corpus"]
        );
        assert_eq!(first.qualifications, second.qualifications);
        assert_eq!(first.qualifications.len(), 1);
        let qualification = &first.qualifications[0];
        assert_eq!(qualification["status"], "passed");
        assert_eq!(qualification["counters"]["candidates"], 64);
        assert_eq!(qualification["seeds"].as_array().unwrap().len(), 2);
        assert_eq!(qualification["minimizations"].as_array().unwrap().len(), 2);
        for outcome in ["crash", "hang", "timeout", "resource_limit"] {
            assert_eq!(qualification["outcomes"][outcome], 0);
        }
        assert!(qualification.get("path").is_none());
        assert!(qualification.get("bytes").is_none());
        assert!(!first_root.path().join(".fuzz-private-sources").exists());
        assert!(!second_root.path().join(".fuzz-private-sources").exists());

        let manifest_schema: Value =
            serde_json::from_str(include_str!("../schemas/manifest.schema.json")).unwrap();
        let qualification_schema = serde_json::json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "$ref": "#/$defs/fuzz_qualification",
            "$defs": manifest_schema["$defs"].clone()
        });
        let validator = jsonschema::validator_for(&qualification_schema).unwrap();
        assert!(
            validator.is_valid(qualification),
            "qualification errors: {:?}",
            validator.iter_errors(qualification).collect::<Vec<_>>()
        );
    }

    #[test]
    fn reduced_many_frame_enhanced_ct_is_byte_stable_and_exact_scale() {
        let first_root = ParametricMapStagingGuard::new();
        let second_root = ParametricMapStagingGuard::new();
        let run = |root: &ParametricMapStagingGuard| PreparedGenerationRun {
            profile: "stress".to_string(),
            out_dir: root.path().to_path_buf(),
            manifest_path: root.path().join("manifest.json"),
            seed: 7,
            include_stress: false,
        };
        let first = execute_plan_first_case(&run(&first_root), STRESS_ENHANCED_CT_CASE_ID);
        let second = execute_plan_first_case(&run(&second_root), STRESS_ENHANCED_CT_CASE_ID);
        assert_eq!(first.manifest_entry, second.manifest_entry);
        assert_eq!(
            first.manifest_entry["profile_membership"],
            serde_json::json!(["stress"])
        );
        assert_eq!(first.manifest_entry["image"]["frames"], 256);
        assert_eq!(first.manifest_entry["image"]["rows"], 64);
        assert_eq!(first.manifest_entry["image"]["columns"], 64);
        assert_eq!(
            first.manifest_entry["pixel_data"]["value_length"],
            2_097_152
        );
        assert_eq!(
            fs::read(
                first_root
                    .path()
                    .join(STRESS_ENHANCED_CT_CASE_ID)
                    .join("instance.dcm")
            )
            .unwrap(),
            fs::read(
                second_root
                    .path()
                    .join(STRESS_ENHANCED_CT_CASE_ID)
                    .join("instance.dcm")
            )
            .unwrap()
        );
    }

    #[test]
    fn reduced_high_instance_ct_study_is_byte_stable_and_exact_scale() {
        let first_root = ParametricMapStagingGuard::new();
        let second_root = ParametricMapStagingGuard::new();
        let run = |root: &ParametricMapStagingGuard| PreparedGenerationRun {
            profile: "stress".to_string(),
            out_dir: root.path().to_path_buf(),
            manifest_path: root.path().join("manifest.json"),
            seed: 7,
            include_stress: false,
        };
        let case = serde_json::json!({
            "case_id": STRESS_HIGH_INSTANCE_CT_CASE_ID,
            "standards_evidence": []
        });
        let lock = "0000000000000000000000000000000000000000000000000000000000000000";
        let first = write_stress_high_instance_ct_case(&run(&first_root), &case, lock).unwrap();
        let second = write_stress_high_instance_ct_case(&run(&second_root), &case, lock).unwrap();
        assert_eq!(first.len(), 128);
        assert_eq!(first.len(), second.len());
        for (left, right) in first.iter().zip(&second) {
            assert_eq!(left.manifest_entry, right.manifest_entry);
            assert_eq!(
                left.manifest_entry["profile_membership"],
                serde_json::json!(["stress"])
            );
            assert_eq!(left.manifest_entry["image"]["rows"], 64);
            assert_eq!(left.manifest_entry["image"]["columns"], 64);
            assert_eq!(
                fs::read(
                    first_root
                        .path()
                        .join(left.manifest_entry["path"].as_str().unwrap())
                )
                .unwrap(),
                fs::read(
                    second_root
                        .path()
                        .join(right.manifest_entry["path"].as_str().unwrap())
                )
                .unwrap()
            );
        }
    }

    #[test]
    fn reduced_large_bulk_sc_is_byte_stable_and_exact_scale() {
        let first_root = ParametricMapStagingGuard::new();
        let second_root = ParametricMapStagingGuard::new();
        let run = |root: &ParametricMapStagingGuard| PreparedGenerationRun {
            profile: "stress".to_string(),
            out_dir: root.path().to_path_buf(),
            manifest_path: root.path().join("manifest.json"),
            seed: 7,
            include_stress: false,
        };
        let case = serde_json::json!({
            "case_id": STRESS_LARGE_BULK_CASE_ID,
            "standards_evidence": []
        });
        let lock = "0000000000000000000000000000000000000000000000000000000000000000";
        let first = write_stress_large_bulk_case(&run(&first_root), &case, lock).unwrap();
        let second = write_stress_large_bulk_case(&run(&second_root), &case, lock).unwrap();
        assert_eq!(first.manifest_entry, second.manifest_entry);
        assert_eq!(
            first.manifest_entry["pixel_data"]["value_length"],
            64 * 1024 * 1024
        );
        assert_eq!(first.manifest_entry["image"]["rows"], 8192);
        assert_eq!(first.manifest_entry["image"]["columns"], 4096);
        assert!(first.manifest_entry["size_bytes"].as_u64().unwrap() > 64 * 1024 * 1024);
    }

    #[test]
    fn reduced_metadata_stress_cases_are_byte_stable_and_exact_scale() {
        let first_root = ParametricMapStagingGuard::new();
        let second_root = ParametricMapStagingGuard::new();
        let run = |root: &ParametricMapStagingGuard| PreparedGenerationRun {
            profile: "stress".to_string(),
            out_dir: root.path().to_path_buf(),
            manifest_path: root.path().join("manifest.json"),
            seed: 7,
            include_stress: false,
        };
        let lock = "0000000000000000000000000000000000000000000000000000000000000000";
        for (case_id, writer) in [
            (
                STRESS_DEEP_NESTED_CASE_ID,
                write_stress_deep_nested_case
                    as fn(
                        &PreparedGenerationRun,
                        &Value,
                        &str,
                    ) -> Result<GeneratedFile, GenerateError>,
            ),
            (
                STRESS_LONG_METADATA_CASE_ID,
                write_stress_long_metadata_case,
            ),
        ] {
            let case = serde_json::json!({"case_id": case_id, "standards_evidence": []});
            let first = writer(&run(&first_root), &case, lock).unwrap();
            let second = writer(&run(&second_root), &case, lock).unwrap();
            assert_eq!(first.manifest_entry, second.manifest_entry);
            assert_eq!(
                first.manifest_entry["profile_membership"],
                serde_json::json!(["stress"])
            );
        }
        assert_eq!(
            fs::metadata(
                first_root
                    .path()
                    .join(STRESS_DEEP_NESTED_CASE_ID)
                    .join("instance.dcm")
            )
            .unwrap()
            .len()
                > 16 * 1024 * 1024,
            true
        );
        let long = open_file(
            first_root
                .path()
                .join(STRESS_LONG_METADATA_CASE_ID)
                .join("instance.dcm"),
        )
        .unwrap();
        assert_eq!(
            long.element(Tag(0x7777, 0x1000))
                .unwrap()
                .to_str()
                .unwrap()
                .len(),
            1024
        );
        assert_eq!(
            long.element(Tag(0x7777, 0x13ff))
                .unwrap()
                .to_str()
                .unwrap()
                .len(),
            1024
        );
    }

    #[test]
    fn reduced_encapsulated_stress_case_is_byte_stable_and_exact_scale() {
        let first_root = ParametricMapStagingGuard::new();
        let second_root = ParametricMapStagingGuard::new();
        let run = |root: &ParametricMapStagingGuard| PreparedGenerationRun {
            profile: "stress".to_string(),
            out_dir: root.path().to_path_buf(),
            manifest_path: root.path().join("manifest.json"),
            seed: 7,
            include_stress: false,
        };
        let case = serde_json::json!({
            "case_id": STRESS_ENCAPSULATED_CASE_ID,
            "standards_evidence": []
        });
        let lock = "0000000000000000000000000000000000000000000000000000000000000000";
        let first = write_stress_encapsulated_case(&run(&first_root), &case, lock).unwrap();
        let second = write_stress_encapsulated_case(&run(&second_root), &case, lock).unwrap();
        assert_eq!(first.manifest_entry, second.manifest_entry);
        assert_eq!(first.manifest_entry["image"]["frames"], 256);
        assert_eq!(
            first.manifest_entry["recipe"]["recipe_parameters"]["fragment_count"],
            16_384
        );
        assert_eq!(
            first.manifest_entry["pixel_data"]["encapsulated_pixel_data"]["extended_offset_table"]
                ["offset_count"],
            256
        );
        assert_eq!(
            first.manifest_entry["pixel_data"]["encapsulated_pixel_data"]["basic_offset_table"]["populated"],
            false
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
            "sha256": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
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
