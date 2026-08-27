use std::fs;
use std::path::{Path, PathBuf};

use dicom_core::{Tag, VR, header::Header, value::DicomValueType};
use dicom_dictionary_std::{StandardDataDictionary, tags, uids};
use dicom_object::{FileDicomObject, InMemDicomObject, open_file};
use serde_json::Value;

use crate::{
    GenerateError,
    codecs::{
        DEFLATED_IMAGE_FRAME_TRANSFER_SYNTAX_UID, FrameDecodeInput, FrameDecoder,
        HTJ2K_LOSSLESS_TRANSFER_SYNTAX_UID, JPEG_2000_LOSSLESS_TRANSFER_SYNTAX_UID,
        JPEG_LOSSLESS_PROCESS_14_TRANSFER_SYNTAX_UID, JPEG_LOSSLESS_SV1_TRANSFER_SYNTAX_UID,
        JPEG_LS_LOSSLESS_TRANSFER_SYNTAX_UID, JPEG_XL_LOSSLESS_TRANSFER_SYNTAX_UID,
        NativeRleLosslessEncoder, RLE_LOSSLESS_TRANSFER_SYNTAX_UID,
    },
    rt_manifest::{ExpectedRtImage, ExpectedRtPlan},
    rt_radiation_manifest::{
        ExpectedRtCode, ExpectedRtRadiation, ExpectedRtRadiationAbsentContent,
        ExpectedRtRadiationControlPoint, ExpectedRtRadiationInstance, ExpectedRtRadiationSet,
        ExpectedRtRadiationSetAbsentContent, ExpectedRtTreatmentDevice,
    },
    sha256_hex,
    waveform_manifest::ExpectedWaveform,
};

#[cfg(test)]
#[path = "validation_spatial_registration_tests.rs"]
mod spatial_registration_tests;

#[cfg(test)]
#[path = "validation_deformable_spatial_registration_tests.rs"]
mod deformable_spatial_registration_tests;

#[cfg(test)]
#[path = "validation_color_softcopy_presentation_state_tests.rs"]
mod color_softcopy_presentation_state_tests;

#[cfg(test)]
#[path = "validation_advanced_blending_presentation_state_tests.rs"]
mod advanced_blending_presentation_state_tests;

#[cfg(test)]
#[path = "validation_blending_presentation_state_tests.rs"]
mod blending_presentation_state_tests;

#[cfg(test)]
#[path = "validation_twelve_lead_ecg_tests.rs"]
mod twelve_lead_ecg_tests;

#[cfg(test)]
#[path = "validation_general_ecg_tests.rs"]
mod general_ecg_tests;

#[cfg(test)]
#[path = "validation_rt_plan_tests.rs"]
mod rt_plan_tests;

#[cfg(test)]
#[path = "validation_rt_image_tests.rs"]
mod rt_image_tests;

#[cfg(test)]
#[path = "validation_rt_radiation_tests.rs"]
mod rt_radiation_tests;

#[cfg(test)]
#[path = "validation_vl_single_frame_tests.rs"]
mod vl_single_frame_tests;

#[cfg(test)]
#[path = "validation_wsi_tiled_full_tests.rs"]
mod wsi_tiled_full_tests;

#[cfg(test)]
#[path = "validation_wsi_tiled_sparse_tests.rs"]
mod wsi_tiled_sparse_tests;

#[cfg(feature = "deflate")]
use crate::codecs::DicomRsDeflatedImageFrameEncoder;
#[cfg(feature = "charls")]
use crate::codecs::DicomRsJpegLsLosslessEncoder;
#[cfg(feature = "jpegxl")]
use crate::codecs::DicomRsJpegXlLosslessEncoder;
#[cfg(feature = "jpeg2000")]
use crate::codecs::OpenJp2Jpeg2000LosslessEncoder;
#[cfg(feature = "htj2k_openjph")]
use crate::codecs::OpenJphHtj2kLosslessEncoder;
#[cfg(feature = "legacy_jpeg_dcmtk")]
use dicom_encoding::{Codec, adapters::PixelDataReader};
#[cfg(feature = "legacy_jpeg_dcmtk")]
use dicom_transfer_syntax_registry::entries::{
    JPEG_LOSSLESS_NON_HIERARCHICAL, JPEG_LOSSLESS_NON_HIERARCHICAL_FIRST_ORDER_PREDICTION,
};

type OpenedObject = FileDicomObject<InMemDicomObject<StandardDataDictionary>>;
type DatasetObject = InMemDicomObject<StandardDataDictionary>;

#[derive(Debug, Clone)]
pub(crate) struct Part10Expectations<'a> {
    pub sop_class_uid: &'a str,
    pub sop_instance_uid: &'a str,
    pub transfer_syntax_uid: &'a str,
    pub implementation_class_uid: &'a str,
    pub synthetic_data: &'a str,
    pub rows: u16,
    pub columns: u16,
    pub frames: u16,
    pub samples_per_pixel: u16,
    pub photometric_interpretation: &'a str,
    pub bits_allocated: u16,
    pub bits_stored: u16,
    pub high_bit: u16,
    pub pixel_representation: u16,
    pub planar_configuration: Option<u16>,
    pub pixel_data_vr: VR,
    pub pixel_data_length_formula: PixelDataLengthFormula,
    pub decoded_frame_hashes: &'a [&'a str],
    pub palette: Option<PaletteExpectations>,
    pub padding: Option<PixelPaddingExpectations>,
    pub ct_image: Option<CtImageExpectations<'a>>,
    pub enhanced_ct_image: Option<EnhancedCtImageExpectations<'a>>,
    pub enhanced_mr_image: Option<EnhancedMrImageExpectations<'a>>,
    pub enhanced_pet_image: Option<EnhancedPetImageExpectations<'a>>,
    pub mg_image: Option<MgImageExpectations<'a>>,
    pub dx_image: Option<DxImageExpectations<'a>>,
    pub xa_image: Option<XaImageExpectations<'a>>,
    pub xrf_image: Option<XrfImageExpectations<'a>>,
    pub us_image: Option<UsImageExpectations<'a>>,
    pub us_multiframe: Option<UsMultiframeExpectations<'a>>,
    pub nm_image: Option<NmImageExpectations<'a>>,
    pub pet_image: Option<PetImageExpectations<'a>>,
    pub cr_image: Option<CrImageExpectations<'a>>,
    pub mr_image: Option<MrImageExpectations<'a>>,
    pub segmentation: Option<SegmentationExpectations<'a>>,
}

#[derive(Debug, Clone)]
pub(crate) struct PresentationStateExpectations<'a> {
    pub sop_class_uid: &'a str,
    pub sop_instance_uid: &'a str,
    pub transfer_syntax_uid: &'a str,
    pub implementation_class_uid: &'a str,
    pub synthetic_data: &'a str,
    pub modality: &'a str,
    pub presentation_label: &'a str,
    pub referenced_series_instance_uid: &'a str,
    pub referenced_sop_class_uid: &'a str,
    pub referenced_sop_instance_uid: &'a str,
    pub displayed_area_top_left: Vec<i32>,
    pub displayed_area_bottom_right: Vec<i32>,
    pub presentation_size_mode: &'a str,
    pub presentation_pixel_aspect_ratio: Vec<i32>,
    pub window_center: &'a str,
    pub window_width: &'a str,
    pub presentation_lut_shape: &'a str,
}

#[derive(Debug, Clone)]
pub(crate) struct ColorSoftcopyPresentationStateExpectations<'a> {
    pub sop_class_uid: &'a str,
    pub sop_instance_uid: &'a str,
    pub transfer_syntax_uid: &'a str,
    pub implementation_class_uid: &'a str,
    pub synthetic_data: &'a str,
    pub study_instance_uid: &'a str,
    pub series_instance_uid: &'a str,
    pub source_study_instance_uid: &'a str,
    pub source_series_instance_uid: &'a str,
    pub source_sop_class_uid: &'a str,
    pub source_sop_instance_uid: &'a str,
    pub icc_profile_sha256: &'a str,
}

#[derive(Debug, Clone)]
pub(crate) struct AdvancedBlendingSourceSeriesExpectations<'a> {
    pub series_instance_uid: &'a str,
    pub sop_class_uid: &'a str,
    pub sop_instance_uids: [&'a str; 2],
}

#[derive(Debug, Clone)]
pub(crate) struct AdvancedBlendingPresentationStateExpectations<'a> {
    pub sop_class_uid: &'a str,
    pub sop_instance_uid: &'a str,
    pub transfer_syntax_uid: &'a str,
    pub implementation_class_uid: &'a str,
    pub synthetic_data: &'a str,
    pub study_instance_uid: &'a str,
    pub series_instance_uid: &'a str,
    pub frame_of_reference_uid: &'a str,
    pub source_series: [AdvancedBlendingSourceSeriesExpectations<'a>; 2],
    pub icc_profile_sha256: &'a str,
}

#[derive(Debug, Clone)]
pub(crate) struct BlendingSourceSeriesExpectations<'a> {
    pub series_instance_uid: &'a str,
    pub sop_class_uid: &'a str,
    pub sop_instance_uids: [&'a str; 2],
}

#[derive(Debug, Clone)]
pub(crate) struct BlendingPresentationStateExpectations<'a> {
    pub sop_class_uid: &'a str,
    pub sop_instance_uid: &'a str,
    pub transfer_syntax_uid: &'a str,
    pub implementation_class_uid: &'a str,
    pub synthetic_data: &'a str,
    pub study_instance_uid: &'a str,
    pub series_instance_uid: &'a str,
    pub source_series: [BlendingSourceSeriesExpectations<'a>; 2],
    pub palette_channel_sha256: &'a str,
    pub icc_profile_sha256: &'a str,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct TwelveLeadEcgExpectations<'a> {
    pub sop_instance_uid: &'a str,
    pub implementation_class_uid: &'a str,
    pub study_instance_uid: &'a str,
    pub series_instance_uid: &'a str,
    pub waveform: ExpectedWaveform<'a>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct GeneralEcgExpectations<'a> {
    pub sop_instance_uid: &'a str,
    pub implementation_class_uid: &'a str,
    pub study_instance_uid: &'a str,
    pub series_instance_uid: &'a str,
    pub waveform: ExpectedWaveform<'a>,
}

#[derive(Debug, Clone)]
pub(crate) struct RealWorldValueMappingExpectations<'a> {
    pub sop_class_uid: &'a str,
    pub sop_instance_uid: &'a str,
    pub transfer_syntax_uid: &'a str,
    pub implementation_class_uid: &'a str,
    pub synthetic_data: &'a str,
    pub modality: &'a str,
    pub content_label: &'a str,
    pub referenced_series_instance_uid: &'a str,
    pub referenced_sop_class_uid: &'a str,
    pub referenced_sop_instance_uid: &'a str,
    pub referenced_frame_numbers: &'a [u16],
    pub lut_label: &'a str,
    pub first_value_mapped: u16,
    pub last_value_mapped: u16,
    pub intercept: f64,
    pub slope: f64,
    pub unit_code_value: &'a str,
    pub unit_coding_scheme_designator: &'a str,
    pub unit_code_meaning: &'a str,
}

#[derive(Debug, Clone)]
pub(crate) struct BasicTextSrExpectations<'a> {
    pub sop_class_uid: &'a str,
    pub sop_instance_uid: &'a str,
    pub transfer_syntax_uid: &'a str,
    pub implementation_class_uid: &'a str,
    pub synthetic_data: &'a str,
    pub modality: &'a str,
    pub completion_flag: &'a str,
    pub verification_flag: &'a str,
    pub referenced_study_instance_uid: &'a str,
    pub referenced_series_instance_uid: &'a str,
    pub referenced_sop_class_uid: &'a str,
    pub referenced_sop_instance_uid: &'a str,
    pub root_value_type: &'a str,
    pub root_continuity_of_content: &'a str,
    pub title_code_value: &'a str,
    pub title_coding_scheme_designator: &'a str,
    pub title_code_meaning: &'a str,
    pub observation_relationship_type: &'a str,
    pub observation_value_type: &'a str,
    pub observation_code_value: &'a str,
    pub observation_coding_scheme_designator: &'a str,
    pub observation_code_meaning: &'a str,
    pub observation_text: &'a str,
}

#[derive(Debug, Clone)]
pub(crate) struct ComprehensiveSrExpectations<'a> {
    pub sop_class_uid: &'a str,
    pub sop_instance_uid: &'a str,
    pub transfer_syntax_uid: &'a str,
    pub implementation_class_uid: &'a str,
    pub synthetic_data: &'a str,
    pub modality: &'a str,
    pub completion_flag: &'a str,
    pub verification_flag: &'a str,
    pub referenced_study_instance_uid: &'a str,
    pub referenced_series_instance_uid: &'a str,
    pub referenced_sop_class_uid: &'a str,
    pub referenced_sop_instance_uid: &'a str,
    pub root_value_type: &'a str,
    pub root_continuity_of_content: &'a str,
    pub title_code_value: &'a str,
    pub title_coding_scheme_designator: &'a str,
    pub title_code_meaning: &'a str,
    pub measurement_relationship_type: &'a str,
    pub measurement_value_type: &'a str,
    pub measurement_code_value: &'a str,
    pub measurement_coding_scheme_designator: &'a str,
    pub measurement_code_meaning: &'a str,
    pub numeric_value: &'a str,
    pub unit_code_value: &'a str,
    pub unit_coding_scheme_designator: &'a str,
    pub unit_code_meaning: &'a str,
    pub image_relationship_type: &'a str,
    pub image_value_type: &'a str,
    pub image_code_value: &'a str,
    pub image_coding_scheme_designator: &'a str,
    pub image_code_meaning: &'a str,
    pub referenced_frame_numbers: &'a [u16],
}

#[derive(Debug, Clone)]
pub(crate) struct Tid1500Expectations<'a> {
    pub sop_class_uid: &'a str,
    pub sop_instance_uid: &'a str,
    pub transfer_syntax_uid: &'a str,
    pub implementation_class_uid: &'a str,
    pub synthetic_data: &'a str,
    pub modality: &'a str,
    pub completion_flag: &'a str,
    pub verification_flag: &'a str,
    pub preliminary_flag: &'a str,
    pub referenced_study_instance_uid: &'a str,
    pub observer_uid: &'a str,
    pub tracking_identifier: &'a str,
    pub tracking_uid: &'a str,
    pub source_series_instance_uid: &'a str,
    pub source_sop_class_uid: &'a str,
    pub source_sop_instance_uid: &'a str,
    pub source_frame_numbers: &'a [u16],
    pub segmentation_series_instance_uid: &'a str,
    pub segmentation_sop_class_uid: &'a str,
    pub segmentation_sop_instance_uid: &'a str,
    pub referenced_segment_number: u16,
}

#[derive(Debug, Clone)]
pub(crate) struct Scoord3dExpectations<'a> {
    pub sop_class_uid: &'a str,
    pub sop_instance_uid: &'a str,
    pub transfer_syntax_uid: &'a str,
    pub implementation_class_uid: &'a str,
    pub synthetic_data: &'a str,
    pub modality: &'a str,
    pub completion_flag: &'a str,
    pub verification_flag: &'a str,
    pub preliminary_flag: &'a str,
    pub referenced_study_instance_uid: &'a str,
    pub observer_uid: &'a str,
    pub tracking_identifier: &'a str,
    pub tracking_uid: &'a str,
    pub frame_of_reference_uid: &'a str,
    pub fiducial_uid: &'a str,
    pub source_series_instance_uid: &'a str,
    pub source_sop_class_uid: &'a str,
    pub source_sop_instance_uid: &'a str,
    pub source_frame_numbers: &'a [u16],
}

#[derive(Debug, Clone)]
pub(crate) struct SpatialRegistrationReferenceExpectations<'a> {
    pub study_instance_uid: &'a str,
    pub series_instance_uid: &'a str,
    pub sop_class_uid: &'a str,
    pub sop_instance_uid: &'a str,
    pub frame_of_reference_uid: &'a str,
}

#[derive(Debug, Clone)]
pub(crate) struct SpatialRegistrationExpectations<'a> {
    pub sop_class_uid: &'a str,
    pub sop_instance_uid: &'a str,
    pub transfer_syntax_uid: &'a str,
    pub implementation_class_uid: &'a str,
    pub synthetic_data: &'a str,
    pub patient_id: &'a str,
    pub study_instance_uid: &'a str,
    pub study_id: &'a str,
    pub series_instance_uid: &'a str,
    pub series_number: &'a str,
    pub laterality: &'a str,
    pub modality: &'a str,
    pub instance_number: &'a str,
    pub content_date: &'a str,
    pub content_time: &'a str,
    pub content_label: &'a str,
    pub content_description: &'a str,
    pub content_creator_name: &'a str,
    pub manufacturer: &'a str,
    pub manufacturer_model_name: &'a str,
    pub device_serial_number: &'a str,
    pub software_versions: &'a str,
    pub registered_frame_of_reference_uid: &'a str,
    pub target: SpatialRegistrationReferenceExpectations<'a>,
    pub source: SpatialRegistrationReferenceExpectations<'a>,
    pub target_matrix: [f64; 16],
    pub source_to_registered_matrix: [f64; 16],
    pub source_landmark_mm: [f64; 3],
    pub registered_landmark_mm: [f64; 3],
    pub rigid_tolerance: f64,
}

#[derive(Debug, Clone)]
pub(crate) struct DeformableSpatialRegistrationExpectations<'a> {
    pub sop_class_uid: &'a str,
    pub sop_instance_uid: &'a str,
    pub transfer_syntax_uid: &'a str,
    pub implementation_class_uid: &'a str,
    pub synthetic_data: &'a str,
    pub patient_id: &'a str,
    pub study_instance_uid: &'a str,
    pub study_id: &'a str,
    pub series_instance_uid: &'a str,
    pub series_number: &'a str,
    pub laterality: &'a str,
    pub modality: &'a str,
    pub instance_number: &'a str,
    pub content_date: &'a str,
    pub content_time: &'a str,
    pub content_label: &'a str,
    pub content_description: &'a str,
    pub content_creator_name: &'a str,
    pub manufacturer: &'a str,
    pub manufacturer_model_name: &'a str,
    pub device_serial_number: &'a str,
    pub software_versions: &'a str,
    pub registered_frame_of_reference_uid: &'a str,
    pub target: SpatialRegistrationReferenceExpectations<'a>,
    pub source: SpatialRegistrationReferenceExpectations<'a>,
    pub pre_matrix: [f64; 16],
    pub post_matrix: [f64; 16],
    pub image_orientation_patient: [f64; 6],
    pub image_position_patient: [f64; 3],
    pub grid_dimensions: [u32; 3],
    pub grid_resolution: [f64; 3],
    pub vector_grid_data_sha256: &'a str,
    pub decoded_vectors_mm: &'a [[f32; 3]],
    pub registered_points_mm: &'a [[f64; 3]],
    pub source_points_mm: &'a [[f64; 3]],
    pub tolerance: f64,
}

#[derive(Debug, Clone)]
pub(crate) struct KeyObjectReferenceExpectations<'a> {
    pub referenced_series_instance_uid: &'a str,
    pub referenced_sop_class_uid: &'a str,
    pub referenced_sop_instance_uid: &'a str,
    pub referenced_frame_numbers: Option<&'a [u16]>,
}

#[derive(Debug, Clone)]
pub(crate) struct KeyObjectSelectionExpectations<'a> {
    pub sop_class_uid: &'a str,
    pub sop_instance_uid: &'a str,
    pub transfer_syntax_uid: &'a str,
    pub implementation_class_uid: &'a str,
    pub synthetic_data: &'a str,
    pub modality: &'a str,
    pub completion_flag: &'a str,
    pub verification_flag: &'a str,
    pub referenced_study_instance_uid: &'a str,
    pub root_value_type: &'a str,
    pub root_continuity_of_content: &'a str,
    pub title_code_value: &'a str,
    pub title_coding_scheme_designator: &'a str,
    pub title_code_meaning: &'a str,
    pub mapping_resource: &'a str,
    pub template_identifier: &'a str,
    pub relationship_type: &'a str,
    pub image_value_type: &'a str,
    pub key_objects: &'a [KeyObjectReferenceExpectations<'a>],
}

#[derive(Debug, Clone)]
pub(crate) struct RtStructureSetExpectations<'a> {
    pub sop_class_uid: &'a str,
    pub sop_instance_uid: &'a str,
    pub transfer_syntax_uid: &'a str,
    pub implementation_class_uid: &'a str,
    pub synthetic_data: &'a str,
    pub modality: &'a str,
    pub frame_of_reference_uid: &'a str,
    pub structure_set_label: &'a str,
    pub structure_set_roi_items: usize,
    pub roi_number: u16,
    pub roi_name: &'a str,
    pub roi_generation_algorithm: &'a str,
    pub roi_contour_items: usize,
    pub contour_items: usize,
    pub contour_geometric_type: &'a str,
    pub contour_points: u16,
    pub contour_data: &'a str,
    pub rt_roi_observation_items: usize,
    pub roi_interpreted_type: &'a str,
    pub roi_interpreter: &'a str,
    pub referenced_series_instance_uid: &'a str,
    pub referenced_sop_class_uid: &'a str,
    pub referenced_sop_instance_uid: &'a str,
}

#[derive(Debug, Clone)]
pub(crate) struct RtDoseExpectations<'a> {
    pub sop_class_uid: &'a str,
    pub sop_instance_uid: &'a str,
    pub transfer_syntax_uid: &'a str,
    pub implementation_class_uid: &'a str,
    pub synthetic_data: &'a str,
    pub modality: &'a str,
    pub frame_of_reference_uid: &'a str,
    pub rows: u16,
    pub columns: u16,
    pub frames: u16,
    pub pixel_bytes_len: usize,
    pub pixel_vr: VR,
    pub pixel_spacing: &'a str,
    pub image_orientation_patient: &'a str,
    pub image_position_patient: &'a str,
    pub slice_thickness: &'a str,
    pub frame_increment_pointer: Tag,
    pub grid_frame_offset_vector: &'a str,
    pub dose_units: &'a str,
    pub dose_type: &'a str,
    pub dose_summation_type: &'a str,
    pub dose_grid_scaling: &'a str,
    pub referenced_image_sop_class_uid: &'a str,
    pub referenced_image_sop_instance_uid: &'a str,
    pub referenced_structure_set_sop_class_uid: &'a str,
    pub referenced_structure_set_sop_instance_uid: &'a str,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct RtPlanExpectations<'a> {
    pub implementation_class_uid: &'a str,
    pub synthetic_data: &'a str,
    pub expected_rt_plan: ExpectedRtPlan<'a>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct RtImageExpectations<'a> {
    pub implementation_class_uid: &'a str,
    pub synthetic_data: &'a str,
    pub expected_rt_image: ExpectedRtImage<'a>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct RtRadiationExpectations<'a> {
    pub implementation_class_uid: &'a str,
    pub synthetic_data: &'a str,
    pub expected_rt_radiation: ExpectedRtRadiation<'a>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct RtRadiationSetExpectations<'a> {
    pub implementation_class_uid: &'a str,
    pub synthetic_data: &'a str,
    pub expected_rt_radiation_set: ExpectedRtRadiationSet<'a>,
}

#[derive(Debug, Clone)]
pub(crate) struct EncapsulatedPdfExpectations<'a> {
    pub sop_class_uid: &'a str,
    pub sop_instance_uid: &'a str,
    pub transfer_syntax_uid: &'a str,
    pub implementation_class_uid: &'a str,
    pub synthetic_data: &'a str,
    pub modality: &'a str,
    pub conversion_type: &'a str,
    pub instance_number: &'a str,
    pub content_date: &'a str,
    pub content_time: &'a str,
    pub acquisition_datetime: &'a str,
    pub burned_in_annotation: &'a str,
    pub recognizable_visual_features: &'a str,
    pub document_title: &'a str,
    pub mime_type: &'a str,
    pub document_bytes: &'a [u8],
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum PixelDataLengthFormula {
    ContiguousSamples,
    YbrFull422,
    BitPackedFrames,
    BitPackedContinuousFrames,
    Encapsulated {
        fragments: usize,
        basic_offset_table_offsets: usize,
    },
}

#[derive(Debug, Clone)]
pub(crate) struct PaletteExpectations {
    pub descriptor: [u16; 3],
    pub red_data_length: usize,
    pub green_data_length: usize,
    pub blue_data_length: usize,
}

#[derive(Debug, Clone)]
pub(crate) struct PixelPaddingExpectations {
    pub value: i16,
    pub range_limit: Option<i16>,
}

#[derive(Debug, Clone)]
pub(crate) struct CtImageExpectations<'a> {
    pub modality: &'a str,
    pub frame_of_reference_uid: &'a str,
    pub image_type: &'a str,
    pub pixel_spacing: &'a str,
    pub image_orientation_patient: &'a str,
    pub image_position_patient: &'a str,
    pub slice_thickness: &'a str,
    pub kvp: &'a str,
    pub acquisition_number: &'a str,
    pub rescale_intercept: &'a str,
    pub rescale_slope: &'a str,
    pub rescale_type: &'a str,
    pub window_center: &'a str,
    pub window_width: &'a str,
}

#[derive(Debug, Clone)]
pub(crate) struct EnhancedCtImageExpectations<'a> {
    pub modality: &'a str,
    pub frame_of_reference_uid: &'a str,
    pub image_type: &'a str,
    pub number_of_frames: u16,
    pub shared_functional_groups: usize,
    pub per_frame_functional_groups: usize,
    pub dimension_organization_uid: &'a str,
    pub dimension_index_count: usize,
    pub pixel_spacing: &'a str,
    pub image_orientation_patient: &'a str,
    pub image_position_patient: &'a [&'a str],
    pub dimension_index_values: &'a [u32],
    pub frame_type: &'a str,
    pub pixel_presentation: &'a str,
    pub volumetric_properties: &'a str,
    pub volume_based_calculation_technique: &'a str,
    pub rescale_intercept: &'a str,
    pub rescale_slope: &'a str,
    pub rescale_type: &'a str,
    pub irradiation_event_uid: &'a str,
    pub concatenation: Option<EnhancedCtConcatenationExpectations<'a>>,
}

#[derive(Debug, Clone)]
pub(crate) struct EnhancedCtConcatenationExpectations<'a> {
    pub concatenation_uid: &'a str,
    pub in_concatenation_number: u16,
    pub in_concatenation_total_number: u16,
    pub concatenation_frame_offset_number: u32,
    pub sop_instance_uid_of_concatenation_source: &'a str,
}

#[derive(Debug, Clone)]
pub(crate) struct EnhancedMrImageExpectations<'a> {
    pub modality: &'a str,
    pub patient_position: &'a str,
    pub frame_of_reference_uid: &'a str,
    pub image_type: &'a str,
    pub number_of_frames: u16,
    pub shared_functional_groups: usize,
    pub per_frame_functional_groups: usize,
    pub dimension_organization_uid: &'a str,
    pub dimension_index_count: usize,
    pub pixel_spacing: &'a str,
    pub image_orientation_patient: &'a str,
    pub image_position_patient: &'a [&'a str],
    pub frame_type: &'a str,
    pub pixel_presentation: &'a str,
    pub volumetric_properties: &'a str,
    pub volume_based_calculation_technique: &'a str,
    pub content_qualification: &'a str,
    pub applicable_safety_standard_agency: &'a str,
    pub complex_image_component: &'a str,
    pub acquisition_contrast: &'a str,
    pub burned_in_annotation: &'a str,
    pub lossy_image_compression: &'a str,
    pub presentation_lut_shape: &'a str,
    pub anatomic_region_code_value: &'a str,
    pub anatomic_region_coding_scheme: &'a str,
    pub anatomic_region_code_meaning: &'a str,
    pub rescale_intercept: &'a str,
    pub rescale_slope: &'a str,
    pub rescale_type: &'a str,
    pub repetition_time: &'a str,
    pub flip_angle: &'a str,
    pub echo_train_length: &'a str,
    pub rf_echo_train_length: u16,
    pub gradient_echo_train_length: u16,
    pub specific_absorption_rate_definition: &'a str,
    pub specific_absorption_rate_value: f64,
    pub operating_modes: &'a [(&'a str, &'a str)],
    pub effective_echo_times: Option<&'a [f64]>,
    pub temporal_position_time_offsets: Option<&'a [f64]>,
    pub velocity_encoding_directions: Option<&'a [[f64; 3]]>,
    pub velocity_encoding_minimum_value: Option<f64>,
    pub velocity_encoding_maximum_value: Option<f64>,
}

#[derive(Debug, Clone)]
pub(crate) struct EnhancedPetImageExpectations<'a> {
    pub modality: &'a str,
    pub frame_of_reference_uid: &'a str,
    pub image_type: &'a str,
    pub frame_type: &'a str,
    pub number_of_frames: u16,
    pub dimension_organization_uid: &'a str,
    pub pixel_spacing: &'a str,
    pub image_orientation_patient: &'a str,
    pub image_position_patient: &'a [&'a str],
    pub dimension_index_values: &'a [u32],
    pub temporal_position_indices: &'a [u32],
    pub in_stack_position_numbers: &'a [u32],
    pub stack_id: &'a str,
    pub rescale_intercept: &'a str,
    pub rescale_slope: &'a str,
    pub stored_values: &'a [u16],
    pub activity_values_bqml: &'a [f64],
}

#[derive(Debug, Clone)]
pub(crate) struct MgImageExpectations<'a> {
    pub modality: &'a str,
    pub presentation_intent_type: &'a str,
    pub image_type: &'a str,
    pub image_laterality: &'a str,
    pub view_position: &'a str,
    pub body_part_examined: &'a str,
    pub organ_exposed: &'a str,
    pub positioner_type: &'a str,
    pub imager_pixel_spacing: &'a str,
    pub detector_type: &'a str,
    pub detector_configuration: &'a str,
    pub detector_id: &'a str,
    pub pixel_intensity_relationship: &'a str,
    pub pixel_intensity_relationship_sign: i16,
    pub rescale_intercept: &'a str,
    pub rescale_slope: &'a str,
    pub rescale_type: &'a str,
    pub presentation_lut_shape: &'a str,
    pub lossy_image_compression: &'a str,
    pub burned_in_annotation: &'a str,
    pub breast_implant_present: &'a str,
    pub window_center: Option<&'a str>,
    pub window_width: Option<&'a str>,
    pub anatomic_region_code_value: &'a str,
    pub view_code_value: &'a str,
    pub acquisition_context_items: usize,
}

#[derive(Debug, Clone)]
pub(crate) struct DxImageExpectations<'a> {
    pub modality: &'a str,
    pub presentation_intent_type: &'a str,
    pub image_type: &'a str,
    pub image_laterality: &'a str,
    pub body_part_examined: &'a str,
    pub imager_pixel_spacing: &'a str,
    pub detector_type: &'a str,
    pub detector_configuration: &'a str,
    pub detector_id: &'a str,
    pub pixel_intensity_relationship: &'a str,
    pub pixel_intensity_relationship_sign: i16,
    pub rescale_intercept: &'a str,
    pub rescale_slope: &'a str,
    pub rescale_type: &'a str,
    pub presentation_lut_shape: &'a str,
    pub lossy_image_compression: &'a str,
    pub burned_in_annotation: &'a str,
    pub window_center: &'a str,
    pub window_width: &'a str,
    pub anatomic_region_code_value: &'a str,
    pub acquisition_context_items: usize,
    pub shutter_shape: &'a str,
    pub shutter_left_vertical_edge: &'a str,
    pub shutter_right_vertical_edge: &'a str,
    pub shutter_upper_horizontal_edge: &'a str,
    pub shutter_lower_horizontal_edge: &'a str,
    pub shutter_presentation_value: u16,
}

#[derive(Debug, Clone)]
pub(crate) struct XaImageExpectations<'a> {
    pub modality: &'a str,
    pub body_part_examined: &'a str,
    pub image_type: &'a str,
    pub patient_orientation: &'a str,
    pub pixel_intensity_relationship: &'a str,
    pub lossy_image_compression: &'a str,
    pub radiation_setting: &'a str,
    pub kvp: &'a str,
    pub exposure_mas: &'a str,
    pub imager_pixel_spacing_mm: &'a str,
    pub positioner_primary_angle_degrees: &'a str,
    pub positioner_secondary_angle_degrees: &'a str,
    pub distance_source_to_detector_mm: &'a str,
    pub distance_source_to_patient_mm: &'a str,
    pub estimated_radiographic_magnification_factor: &'a str,
}

#[derive(Debug, Clone)]
pub(crate) struct XrfImageExpectations<'a> {
    pub modality: &'a str,
    pub body_part_examined: &'a str,
    pub image_type: &'a str,
    pub patient_orientation: &'a str,
    pub pixel_intensity_relationship: &'a str,
    pub lossy_image_compression: &'a str,
    pub radiation_setting: &'a str,
    pub kvp: &'a str,
    pub exposure_mas: &'a str,
    pub imager_pixel_spacing_mm: &'a str,
    pub distance_source_to_detector_mm: &'a str,
    pub distance_source_to_patient_mm: &'a str,
    pub estimated_radiographic_magnification_factor: &'a str,
    pub column_angulation_degrees: &'a str,
}

#[derive(Debug, Clone)]
pub(crate) struct UsImageExpectations<'a> {
    pub modality: &'a str,
    pub image_type: &'a str,
    pub lossy_image_compression: &'a str,
    pub ultrasound_color_data_present: u16,
}

#[derive(Debug, Clone)]
pub(crate) struct UsMultiframeExpectations<'a> {
    pub modality: &'a str,
    pub body_part_examined: &'a str,
    pub image_type: &'a str,
    pub lossy_image_compression: &'a str,
    pub ultrasound_color_data_present: u16,
    pub number_of_frames: u16,
    pub frame_increment_pointer: Tag,
    pub frame_time_ms: &'a str,
}

#[derive(Debug, Clone)]
pub(crate) struct NmEnergyWindowExpectations<'a> {
    pub name: &'a str,
    pub lower_limit_kev: &'a str,
    pub upper_limit_kev: &'a str,
}

#[derive(Debug, Clone)]
pub(crate) struct NmDetectorExpectations<'a> {
    pub collimator_type: &'a str,
    pub focal_distance_mm: &'a str,
    pub start_angle_degrees: &'a str,
    pub image_orientation_patient: &'a str,
    pub image_position_patient: &'a str,
}

#[derive(Debug, Clone)]
pub(crate) struct NmImageExpectations<'a> {
    pub modality: &'a str,
    pub body_part_examined: &'a str,
    pub image_type: &'a str,
    pub pixel_spacing: &'a str,
    pub actual_frame_duration_ms: &'a str,
    pub counts_accumulated: &'a str,
    pub frame_increment_pointers: &'a [Tag],
    pub energy_window_vector: &'a [u16],
    pub detector_vector: &'a [u16],
    pub energy_windows: &'a [NmEnergyWindowExpectations<'a>],
    pub detectors: &'a [NmDetectorExpectations<'a>],
}

#[derive(Debug, Clone)]
pub(crate) struct PetImageExpectations<'a> {
    pub modality: &'a str,
    pub body_part_examined: &'a str,
    pub image_type: &'a str,
    pub series_date: &'a str,
    pub series_time: &'a str,
    pub units: &'a str,
    pub counts_source: &'a str,
    pub series_type: &'a str,
    pub frame_of_reference_uid: &'a str,
    pub position_reference_indicator: &'a str,
    pub number_of_slices: u16,
    pub corrected_image: &'a str,
    pub decay_correction: &'a str,
    pub collimator_type: &'a str,
    pub rescale_intercept: &'a str,
    pub rescale_slope: &'a str,
    pub stored_values: &'a [u16],
    pub activity_values_bqml: &'a [f64],
    pub dose_calibration_factor: &'a str,
    pub frame_reference_time_ms: &'a str,
    pub acquisition_date: &'a str,
    pub acquisition_time: &'a str,
    pub actual_frame_duration_ms: &'a str,
    pub image_index: u16,
    pub pixel_spacing: &'a str,
    pub image_orientation_patient: &'a str,
    pub image_position_patient: &'a str,
    pub slice_thickness: &'a str,
    pub radiopharmaceutical_information_items: usize,
    pub patient_orientation_code_items: usize,
    pub patient_gantry_relationship_code_items: usize,
}

#[derive(Debug, Clone)]
pub(crate) struct CrImageExpectations<'a> {
    pub modality: &'a str,
    pub image_type: &'a str,
    pub body_part_examined: &'a str,
    pub view_position: &'a str,
    pub acquisition_number: &'a str,
    pub overlay_rows: u16,
    pub overlay_columns: u16,
    pub overlay_type: &'a str,
    pub overlay_origin: Vec<i16>,
    pub overlay_bits_allocated: u16,
    pub overlay_bit_position: u16,
    pub overlay_data_length: usize,
    pub modality_lut_descriptor: [u16; 3],
    pub modality_lut_type: &'a str,
    pub modality_lut_data_length: usize,
    pub voi_lut_descriptor: [u16; 3],
    pub voi_lut_data_length: usize,
}

#[derive(Debug, Clone)]
pub(crate) struct MrImageExpectations<'a> {
    pub modality: &'a str,
    pub frame_of_reference_uid: &'a str,
    pub image_type: &'a str,
    pub instance_number: &'a str,
    pub acquisition_number: &'a str,
    pub pixel_spacing: &'a str,
    pub image_orientation_patient: &'a str,
    pub image_position_patient: &'a str,
    pub slice_thickness: &'a str,
    pub spacing_between_slices: &'a str,
    pub slice_location: &'a str,
    pub scanning_sequence: &'a str,
    pub sequence_variant: &'a str,
    pub scan_options: &'a str,
    pub mr_acquisition_type: &'a str,
    pub repetition_time: &'a str,
    pub echo_time: &'a str,
    pub echo_train_length: &'a str,
    pub magnetic_field_strength: &'a str,
    pub slice_order_index: usize,
    pub slice_count: usize,
    pub position_along_normal: f64,
}

#[derive(Debug, Clone)]
pub(crate) struct SegmentationExpectations<'a> {
    pub modality: &'a str,
    pub frame_of_reference_uid: &'a str,
    pub image_type: &'a str,
    pub segmentation_type: &'a str,
    pub segmentation_fractional_type: Option<&'a str>,
    pub maximum_fractional_value: Option<u16>,
    pub segment_sequence_items: usize,
    pub shared_functional_groups: usize,
    pub per_frame_functional_groups: usize,
    pub dimension_organization_uid: &'a str,
    pub dimension_index_count: usize,
    pub referenced_sop_class_uid: &'a str,
    pub referenced_sop_instance_uid: &'a str,
    pub referenced_frame_numbers: &'a [u16],
}

const TAG_SEGMENTATION_TYPE: Tag = Tag(0x0062, 0x0001);
const TAG_SEGMENT_SEQUENCE: Tag = Tag(0x0062, 0x0002);
const TAG_SEGMENT_NUMBER: Tag = Tag(0x0062, 0x0004);
const TAG_SEGMENT_ALGORITHM_TYPE: Tag = Tag(0x0062, 0x0008);
const TAG_SEGMENT_IDENTIFICATION_SEQUENCE: Tag = Tag(0x0062, 0x000A);
const TAG_REFERENCED_SEGMENT_NUMBER: Tag = Tag(0x0062, 0x000B);
const TAG_MAXIMUM_FRACTIONAL_VALUE: Tag = Tag(0x0062, 0x000E);
const TAG_SEGMENTATION_FRACTIONAL_TYPE: Tag = Tag(0x0062, 0x0010);
const TAG_REFERENCED_SERIES_SEQUENCE: Tag = Tag(0x0008, 0x1115);
const TAG_REFERENCED_INSTANCE_SEQUENCE: Tag = Tag(0x0008, 0x114A);
const TAG_REFERENCED_SOP_CLASS_UID: Tag = Tag(0x0008, 0x1150);
const TAG_REFERENCED_SOP_INSTANCE_UID: Tag = Tag(0x0008, 0x1155);
const TAG_REFERENCED_FRAME_NUMBER: Tag = Tag(0x0008, 0x1160);
const TAG_REFERENCED_IMAGE_SEQUENCE: Tag = Tag(0x0008, 0x1140);
const TAG_SOURCE_IMAGE_SEQUENCE: Tag = Tag(0x0008, 0x2112);
const TAG_DERIVATION_IMAGE_SEQUENCE: Tag = Tag(0x0008, 0x9124);
const TAG_REFERENCED_STRUCTURE_SET_SEQUENCE: Tag = Tag(0x300C, 0x0060);
const TAG_DISPLAYED_AREA_TOP_LEFT_HAND_CORNER: Tag = Tag(0x0070, 0x0052);
const TAG_DISPLAYED_AREA_BOTTOM_RIGHT_HAND_CORNER: Tag = Tag(0x0070, 0x0053);
const TAG_DISPLAYED_AREA_SELECTION_SEQUENCE: Tag = Tag(0x0070, 0x005A);
const TAG_CONTENT_LABEL: Tag = Tag(0x0070, 0x0080);
const TAG_PRESENTATION_SIZE_MODE: Tag = Tag(0x0070, 0x0100);
const TAG_PRESENTATION_PIXEL_ASPECT_RATIO: Tag = Tag(0x0070, 0x0102);
const TAG_SOFTCOPY_VOI_LUT_SEQUENCE: Tag = Tag(0x0028, 0x3110);
const TAG_PRESENTATION_LUT_SHAPE: Tag = Tag(0x2050, 0x0020);

#[derive(Debug, Clone)]
pub(crate) struct ValidatedPart10 {
    pub bytes: Vec<u8>,
    pub validation: Value,
}

pub(crate) fn validate_part10_file(
    path: &Path,
    expected: &Part10Expectations<'_>,
) -> Result<ValidatedPart10, GenerateError> {
    let bytes = fs::read(path).map_err(|source| GenerateError::ReadGeneratedFile {
        path: path.to_path_buf(),
        source,
    })?;
    let obj = open_file(path).map_err(|err| GenerateError::ValidateDicomFile {
        path: path.to_path_buf(),
        message: err.to_string(),
    })?;

    let mut internal = Vec::new();
    check(
        &mut internal,
        bytes.len() >= 132 && &bytes[128..132] == b"DICM",
        "part10_preamble",
        "File has a 128-byte preamble followed by the DICM marker.",
        "File is missing the Part 10 DICM marker at byte offset 128.",
    );

    let transfer_syntax = trim_uid(obj.meta().transfer_syntax());
    check_equal(
        &mut internal,
        "file_meta_transfer_syntax",
        "File Meta Information Transfer Syntax UID matches the recipe.",
        "File Meta Information Transfer Syntax UID does not match the recipe.",
        transfer_syntax.as_str(),
        expected.transfer_syntax_uid,
    );

    let dataset_sop_class = element_str(path, &obj, tags::SOP_CLASS_UID)?;
    let meta_sop_class = trim_uid(obj.meta().media_storage_sop_class_uid());
    check_equal(
        &mut internal,
        "sop_class_uid_consistency",
        "Dataset SOP Class UID, File Meta SOP Class UID, and recipe SOP Class UID match.",
        "SOP Class UID differs between dataset, File Meta Information, or recipe.",
        dataset_sop_class.as_str(),
        expected.sop_class_uid,
    );
    check_equal(
        &mut internal,
        "media_storage_sop_class_uid",
        "File Meta SOP Class UID matches the dataset SOP Class UID.",
        "File Meta SOP Class UID does not match the dataset SOP Class UID.",
        meta_sop_class.as_str(),
        dataset_sop_class.as_str(),
    );

    let dataset_sop_instance = element_str(path, &obj, tags::SOP_INSTANCE_UID)?;
    let meta_sop_instance = trim_uid(obj.meta().media_storage_sop_instance_uid());
    check_equal(
        &mut internal,
        "sop_instance_uid_consistency",
        "Dataset SOP Instance UID, File Meta SOP Instance UID, and manifest UID match.",
        "SOP Instance UID differs between dataset, File Meta Information, or manifest.",
        dataset_sop_instance.as_str(),
        expected.sop_instance_uid,
    );
    check_equal(
        &mut internal,
        "media_storage_sop_instance_uid",
        "File Meta SOP Instance UID matches the dataset SOP Instance UID.",
        "File Meta SOP Instance UID does not match the dataset SOP Instance UID.",
        meta_sop_instance.as_str(),
        dataset_sop_instance.as_str(),
    );

    let implementation_class_uid = trim_uid(obj.meta().implementation_class_uid());
    check_equal(
        &mut internal,
        "implementation_class_uid",
        "File Meta Implementation Class UID matches the deterministic generator UID.",
        "File Meta Implementation Class UID does not match the deterministic generator UID.",
        implementation_class_uid.as_str(),
        expected.implementation_class_uid,
    );

    let synthetic_data = element_str(path, &obj, tags::SYNTHETIC_DATA)?;
    check_equal(
        &mut internal,
        "synthetic_data",
        "Synthetic Data is present and set to YES.",
        "Synthetic Data is missing or not set to YES.",
        synthetic_data.as_str(),
        expected.synthetic_data,
    );

    check_equal(
        &mut internal,
        "rows",
        "Rows matches the recipe.",
        "Rows does not match the recipe.",
        element_u16(path, &obj, tags::ROWS)?,
        expected.rows,
    );
    check_equal(
        &mut internal,
        "columns",
        "Columns matches the recipe.",
        "Columns does not match the recipe.",
        element_u16(path, &obj, tags::COLUMNS)?,
        expected.columns,
    );
    if expected.frames > 1 {
        check_equal(
            &mut internal,
            "number_of_frames",
            "Number of Frames matches the recipe.",
            "Number of Frames does not match the recipe.",
            element_str(path, &obj, tags::NUMBER_OF_FRAMES)?.as_str(),
            expected.frames.to_string().as_str(),
        );
    }
    check_equal(
        &mut internal,
        "samples_per_pixel",
        "Samples per Pixel matches the recipe.",
        "Samples per Pixel does not match the recipe.",
        element_u16(path, &obj, tags::SAMPLES_PER_PIXEL)?,
        expected.samples_per_pixel,
    );
    check_equal(
        &mut internal,
        "photometric_interpretation",
        "Photometric Interpretation matches the recipe.",
        "Photometric Interpretation does not match the recipe.",
        element_str(path, &obj, tags::PHOTOMETRIC_INTERPRETATION)?.as_str(),
        expected.photometric_interpretation,
    );
    check_equal(
        &mut internal,
        "bits_allocated",
        "Bits Allocated matches the recipe.",
        "Bits Allocated does not match the recipe.",
        element_u16(path, &obj, tags::BITS_ALLOCATED)?,
        expected.bits_allocated,
    );
    check(
        &mut internal,
        expected.bits_allocated == 1 || expected.bits_allocated % 8 == 0,
        "bits_allocated_native_shape",
        "Bits Allocated is 1 or a multiple of 8 for native Pixel Data.",
        "Bits Allocated is not valid for native Pixel Data.",
    );
    check_equal(
        &mut internal,
        "bits_stored",
        "Bits Stored matches the recipe.",
        "Bits Stored does not match the recipe.",
        element_u16(path, &obj, tags::BITS_STORED)?,
        expected.bits_stored,
    );
    check(
        &mut internal,
        expected.bits_stored <= expected.bits_allocated,
        "bits_stored_within_bits_allocated",
        "Bits Stored is less than or equal to Bits Allocated.",
        "Bits Stored exceeds Bits Allocated.",
    );
    check_equal(
        &mut internal,
        "high_bit",
        "High Bit matches Bits Stored - 1.",
        "High Bit does not match the recipe.",
        element_u16(path, &obj, tags::HIGH_BIT)?,
        expected.high_bit,
    );
    check(
        &mut internal,
        expected.high_bit + 1 == expected.bits_stored,
        "high_bit_consistency",
        "High Bit equals Bits Stored - 1.",
        "High Bit does not equal Bits Stored - 1.",
    );
    check_equal(
        &mut internal,
        "pixel_representation",
        "Pixel Representation matches the recipe.",
        "Pixel Representation does not match the recipe.",
        element_u16(path, &obj, tags::PIXEL_REPRESENTATION)?,
        expected.pixel_representation,
    );
    match expected.planar_configuration {
        Some(expected_planar_configuration) => {
            check_equal(
                &mut internal,
                "planar_configuration",
                "Planar Configuration matches the recipe.",
                "Planar Configuration does not match the recipe.",
                element_u16(path, &obj, tags::PLANAR_CONFIGURATION)?,
                expected_planar_configuration,
            );
        }
        None => {
            let planar_configuration_present = obj
                .element_opt(tags::PLANAR_CONFIGURATION)
                .map_err(|err| validation_error(path, err))?
                .is_some();
            check(
                &mut internal,
                !planar_configuration_present,
                "planar_configuration_absent",
                "Planar Configuration is absent for single-sample pixel data.",
                "Planar Configuration is present for single-sample pixel data.",
            );
        }
    }
    validate_photometric_shape(expected, &mut internal);

    let pixel_element = obj
        .element(tags::PIXEL_DATA)
        .map_err(|err| validation_error(path, err))?;
    check_equal(
        &mut internal,
        "pixel_data_vr",
        "Pixel Data VR matches the recipe.",
        "Pixel Data VR does not match the recipe.",
        pixel_element.vr(),
        expected.pixel_data_vr,
    );
    match expected.pixel_data_length_formula {
        PixelDataLengthFormula::Encapsulated {
            fragments,
            basic_offset_table_offsets,
        } => match pixel_element.value() {
            dicom_core::value::Value::PixelSequence(sequence) => {
                check_equal(
                    &mut internal,
                    "encapsulated_fragment_count",
                    "Encapsulated Pixel Data fragment count matches the recipe.",
                    "Encapsulated Pixel Data fragment count does not match the recipe.",
                    sequence.fragments().len(),
                    fragments,
                );
                check_equal(
                    &mut internal,
                    "encapsulated_basic_offset_table_count",
                    "Basic Offset Table offset count matches the recipe.",
                    "Basic Offset Table offset count does not match the recipe.",
                    sequence.offset_table().len(),
                    basic_offset_table_offsets,
                );
                validate_rle_decoded_frame_hashes(expected, sequence.fragments(), &mut internal);
                validate_jpeg_ls_lossless_decoded_frame_hashes(
                    expected,
                    sequence.fragments(),
                    &mut internal,
                );
                validate_jpeg_xl_lossless_decoded_frame_hashes(
                    expected,
                    sequence.fragments(),
                    &mut internal,
                );
                validate_jpeg_2000_lossless_decoded_frame_hashes(
                    expected,
                    sequence.fragments(),
                    &mut internal,
                );
                validate_htj2k_lossless_decoded_frame_hashes(
                    expected,
                    &obj,
                    sequence.fragments(),
                    &mut internal,
                );
                validate_legacy_jpeg_lossless_decoded_frame_hashes(
                    expected,
                    &obj,
                    sequence.fragments(),
                    &mut internal,
                );
                validate_deflated_image_frame_decoded_frame_hashes(
                    expected,
                    sequence.fragments(),
                    &mut internal,
                );
            }
            _ => check(
                &mut internal,
                false,
                "encapsulated_pixel_sequence",
                "Pixel Data is encoded as an encapsulated fragment sequence.",
                "Pixel Data is not encoded as an encapsulated fragment sequence.",
            ),
        },
        _ => {
            let pixel_bytes = pixel_element
                .value()
                .to_bytes()
                .map_err(|err| validation_error(path, err))?;
            let (pixel_length_name, pixel_length_message, expected_pixel_data_length) =
                expected_pixel_data_length(expected);
            check_equal(
                &mut internal,
                pixel_length_name,
                pixel_length_message,
                "Native Pixel Data length does not match the uncompressed frame size.",
                pixel_bytes.len(),
                expected_pixel_data_length,
            );
            validate_native_frame_hashes(expected, pixel_bytes.as_ref(), &mut internal);
        }
    }
    if let Some(palette) = &expected.palette {
        validate_palette(path, &obj, &mut internal, palette)?;
    }
    if let Some(padding) = &expected.padding {
        validate_pixel_padding(path, &obj, &mut internal, padding)?;
    }
    if let Some(ct_image) = &expected.ct_image {
        validate_ct_image(path, &obj, &mut internal, ct_image)?;
    }
    if let Some(enhanced_ct_image) = &expected.enhanced_ct_image {
        validate_enhanced_ct_image(path, &obj, &mut internal, enhanced_ct_image)?;
    }
    if let Some(enhanced_mr_image) = &expected.enhanced_mr_image {
        validate_enhanced_mr_image(path, &obj, &mut internal, enhanced_mr_image)?;
    }
    if let Some(enhanced_pet_image) = &expected.enhanced_pet_image {
        validate_enhanced_pet_image(path, &obj, &mut internal, enhanced_pet_image)?;
    }
    if let Some(mg_image) = &expected.mg_image {
        validate_mg_image(path, &obj, &mut internal, mg_image)?;
    }
    if let Some(dx_image) = &expected.dx_image {
        validate_dx_image(path, &obj, &mut internal, dx_image)?;
    }
    if let Some(xa_image) = &expected.xa_image {
        validate_xa_image(path, &obj, &mut internal, xa_image)?;
    }
    if let Some(xrf_image) = &expected.xrf_image {
        validate_xrf_image(path, &obj, &mut internal, xrf_image)?;
    }
    if let Some(us_image) = &expected.us_image {
        validate_us_image(path, &obj, &mut internal, us_image)?;
    }
    if let Some(us_multiframe) = &expected.us_multiframe {
        validate_us_multiframe(path, &obj, &mut internal, us_multiframe)?;
    }
    if let Some(nm_image) = &expected.nm_image {
        validate_nm_image(path, &obj, &mut internal, nm_image)?;
    }
    if let Some(pet_image) = &expected.pet_image {
        validate_pet_image(path, &obj, &mut internal, pet_image)?;
    }
    if let Some(cr_image) = &expected.cr_image {
        validate_cr_image(path, &obj, &mut internal, cr_image)?;
    }
    if let Some(mr_image) = &expected.mr_image {
        validate_mr_image(path, &obj, &mut internal, mr_image)?;
    }
    if let Some(segmentation) = &expected.segmentation {
        validate_segmentation(path, &obj, &mut internal, segmentation)?;
    }
    validate_vl_single_frame(path, &obj, &mut internal, expected)?;

    fail_if_any_failed(path, &internal)?;

    Ok(ValidatedPart10 {
        bytes,
        validation: serde_json::json!({
            "status": "passed",
            "internal": internal,
            "standards": [
                {
                    "name": standard_sop_class_validation_name(expected.sop_class_uid),
                    "status": "passed",
                    "message": standard_sop_class_validation_message(expected.sop_class_uid)
                },
                {
                    "name": standard_transfer_syntax_validation_name(expected.transfer_syntax_uid),
                    "status": "passed",
                    "message": standard_transfer_syntax_validation_message(expected.transfer_syntax_uid)
                },
                {
                    "name": "synthetic_data_attribute",
                    "status": "passed",
                    "message": "Synthetic Data (0008,001C) is present with value YES."
                },
                {
                    "name": "image_pixel_description",
                    "status": "passed",
                    "message": "Image Pixel attributes match the native pixel recipe."
                }
            ],
            "external": []
        }),
    })
}

fn validate_vl_single_frame(
    path: &Path,
    obj: &OpenedObject,
    internal: &mut Vec<Value>,
    expected: &Part10Expectations<'_>,
) -> Result<(), GenerateError> {
    let (modality, body_part_examined, family_name) = match expected.sop_class_uid {
        uids::VL_ENDOSCOPIC_IMAGE_STORAGE => ("ES", "LUNG", "VL Endoscopic"),
        uids::VL_MICROSCOPIC_IMAGE_STORAGE => ("GM", "EYE", "VL Microscopic"),
        _ => return Ok(()),
    };
    const PIXELS: [u8; 12] = [255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255];

    check(
        internal,
        expected.transfer_syntax_uid == uids::EXPLICIT_VR_LITTLE_ENDIAN
            && expected.rows == 2
            && expected.columns == 2
            && expected.frames == 1
            && expected.samples_per_pixel == 3
            && expected.photometric_interpretation == "RGB"
            && expected.planar_configuration == Some(0)
            && expected.bits_allocated == 8
            && expected.bits_stored == 8
            && expected.high_bit == 7
            && expected.pixel_representation == 0
            && expected.pixel_data_vr == VR::OB
            && matches!(
                expected.pixel_data_length_formula,
                PixelDataLengthFormula::ContiguousSamples
            ),
        "vl_single_frame_expected_contract",
        "Manifest-derived single-frame VL expectations match the locked native RGB contract.",
        "Manifest-derived single-frame VL expectations do not match the locked native RGB contract.",
    );

    for (name, tag, locked) in [
        ("modality", tags::MODALITY, modality),
        (
            "body_part_examined",
            tags::BODY_PART_EXAMINED,
            body_part_examined,
        ),
        ("laterality", tags::LATERALITY, "R"),
        ("image_type", tags::IMAGE_TYPE, "ORIGINAL\\PRIMARY"),
        (
            "lossy_image_compression",
            tags::LOSSY_IMAGE_COMPRESSION,
            "00",
        ),
    ] {
        check_equal(
            internal,
            &format!("vl_single_frame_{name}"),
            &format!("{family_name} {name} matches the locked contract."),
            &format!("{family_name} {name} does not match the locked contract."),
            element_str(path, obj, tag)?.as_str(),
            locked,
        );
    }

    check_equal(
        internal,
        "vl_single_frame_acquisition_context_items",
        "Acquisition Context Sequence is present and empty.",
        "Acquisition Context Sequence is not present and empty.",
        sequence_item_count(path, obj, tags::ACQUISITION_CONTEXT_SEQUENCE)?,
        0,
    );

    let pixel_data = obj
        .element(tags::PIXEL_DATA)
        .map_err(|err| validation_error(path, err))?;
    check_equal(
        internal,
        "vl_single_frame_pixel_vr",
        "Single-frame VL Pixel Data uses native OB storage.",
        "Single-frame VL Pixel Data does not use native OB storage.",
        pixel_data.vr(),
        VR::OB,
    );
    check_equal(
        internal,
        "vl_single_frame_pixel_bytes",
        "Single-frame VL RGB bytes match the locked deterministic pattern.",
        "Single-frame VL RGB bytes do not match the locked deterministic pattern.",
        pixel_data
            .value()
            .to_bytes()
            .map_err(|err| validation_error(path, err))?
            .as_ref(),
        PIXELS.as_slice(),
    );

    for (name, tag) in [
        ("number_of_frames_absent", tags::NUMBER_OF_FRAMES),
        (
            "frame_of_reference_uid_absent",
            tags::FRAME_OF_REFERENCE_UID,
        ),
        (
            "specimen_description_sequence_absent",
            tags::SPECIMEN_DESCRIPTION_SEQUENCE,
        ),
        ("optical_path_sequence_absent", tags::OPTICAL_PATH_SEQUENCE),
        ("icc_profile_absent", tags::ICC_PROFILE),
        ("conversion_type_absent", tags::CONVERSION_TYPE),
    ] {
        check(
            internal,
            obj.element_opt(tag)
                .map_err(|err| validation_error(path, err))?
                .is_none(),
            &format!("vl_single_frame_{name}"),
            &format!("{family_name} locked optional content is absent."),
            &format!("{family_name} contains forbidden optional content."),
        );
    }

    Ok(())
}

/// Validate the complete, case-scoped Phase 4 TILED_FULL WSI contract.
///
/// The generic image validator owns Part 10 identity and Image Pixel invariants. This
/// validator additionally binds the generated object to the canonical manifest contract,
/// including implicit frame order and reconstruction of the total pixel matrix.
pub(crate) fn validate_wsi_tiled_full_file(
    path: &Path,
    identity: &Part10Expectations<'_>,
    expected_wsi_tiled_full: &Value,
) -> Result<ValidatedPart10, GenerateError> {
    let mut validated = validate_part10_file(path, identity)?;
    let obj = open_file(path).map_err(|err| GenerateError::ValidateDicomFile {
        path: path.to_path_buf(),
        message: err.to_string(),
    })?;
    let mut internal = Vec::new();
    validate_wsi_tiled_full(path, &obj, &mut internal, identity, expected_wsi_tiled_full)?;
    fail_if_any_failed(path, &internal)?;

    let validation_items = validated
        .validation
        .get_mut("internal")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| GenerateError::ValidateDicomFile {
            path: path.to_path_buf(),
            message: "generic validation result has no internal findings array".to_string(),
        })?;
    validation_items.extend(internal);
    if let Some(standards) = validated
        .validation
        .get_mut("standards")
        .and_then(Value::as_array_mut)
    {
        standards.push(serde_json::json!({
            "name": "vl_whole_slide_microscopy_image_sop_class",
            "status": "passed",
            "message": "TILED_FULL WSI identity, modules, implicit frame order, reconstructed matrix, specimen, optical path, ICC profile, and locked absences match the canonical manifest contract."
        }));
    }
    Ok(validated)
}

fn validate_wsi_tiled_full(
    path: &Path,
    obj: &OpenedObject,
    internal: &mut Vec<Value>,
    identity: &Part10Expectations<'_>,
    expected: &Value,
) -> Result<(), GenerateError> {
    const WSI_SOP_CLASS_UID: &str = "1.2.840.10008.5.1.4.1.1.77.1.6";
    const FRAME_HASHES: [&str; 4] = [
        "fcf067f6323bb42b8292a565a8f826ec5fdb1b142b7a69bf7f7721f0d5d46ef8",
        "6c8f6d772829d493618e079a099cf4f20d8524ed3656f49db234f5bbf60a4e65",
        "7263ad3fd60c6620abd423516d748baedf5e393b1fbdaaf780ff5803a443cc4f",
        "8688d249e9d047b4fc2fb89ce05afe9ec89252ffccdd969de6eef260dd7ffb21",
    ];
    const MATRIX_HASH: &str = "62d9532d46c3f71b045a1393d95c49c4757ef5e62bb043a61baf4fffed189a2a";
    const ICC_HASH: &str = "8e069a3476b71a0e0ae7272d9278ba70540d1c4a0b19af1c7d52e56f49091fef";

    let expected_for = expected
        .pointer("/frame_of_reference_uid")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let expected_specimen_uid = expected
        .pointer("/specimen/specimen_uid")
        .and_then(Value::as_str)
        .unwrap_or_default();
    check_equal(
        internal,
        "wsi_expected_contract",
        "Manifest-derived WSI expectation equals the canonical locked contract.",
        "Manifest-derived WSI expectation differs from the canonical locked contract.",
        expected,
        &crate::wsi_tiled_full_locked_contract(expected_for, expected_specimen_uid),
    );
    check(
        internal,
        identity.sop_class_uid == WSI_SOP_CLASS_UID
            && identity.transfer_syntax_uid == uids::EXPLICIT_VR_LITTLE_ENDIAN
            && identity.rows == 2
            && identity.columns == 2
            && identity.frames == 4
            && identity.samples_per_pixel == 3
            && identity.photometric_interpretation == "RGB"
            && identity.planar_configuration == Some(0)
            && identity.bits_allocated == 8
            && identity.bits_stored == 8
            && identity.high_bit == 7
            && identity.pixel_representation == 0
            && identity.pixel_data_vr == VR::OB
            && matches!(
                identity.pixel_data_length_formula,
                PixelDataLengthFormula::ContiguousSamples
            ),
        "wsi_identity_contract",
        "Manifest image identity matches the locked native TILED_FULL WSI contract.",
        "Manifest image identity differs from the locked native TILED_FULL WSI contract.",
    );

    for (name, tag, locked) in [
        ("modality", tags::MODALITY, "SM"),
        (
            "frame_of_reference_uid",
            tags::FRAME_OF_REFERENCE_UID,
            expected_for,
        ),
        (
            "position_reference_indicator",
            tags::POSITION_REFERENCE_INDICATOR,
            "SLIDE_CORNER",
        ),
        (
            "image_type",
            tags::IMAGE_TYPE,
            "ORIGINAL\\PRIMARY\\VOLUME\\NONE",
        ),
        (
            "acquisition_date_time",
            tags::ACQUISITION_DATE_TIME,
            "20260101000000",
        ),
        (
            "volumetric_properties",
            tags::VOLUMETRIC_PROPERTIES,
            "VOLUME",
        ),
        (
            "specimen_label_in_image",
            tags::SPECIMEN_LABEL_IN_IMAGE,
            "NO",
        ),
        ("burned_in_annotation", tags::BURNED_IN_ANNOTATION, "NO"),
        ("focus_method", tags::FOCUS_METHOD, "AUTO"),
        (
            "extended_depth_of_field",
            tags::EXTENDED_DEPTH_OF_FIELD,
            "NO",
        ),
        (
            "lossy_image_compression",
            tags::LOSSY_IMAGE_COMPRESSION,
            "00",
        ),
        (
            "dimension_organization_type",
            tags::DIMENSION_ORGANIZATION_TYPE,
            "TILED_FULL",
        ),
        ("tiles_overlap", tags::TILES_OVERLAP, "NONE"),
        ("label_text", tags::LABEL_TEXT, "DTS SYNTHETIC SLIDE 001"),
        ("barcode_value", tags::BARCODE_VALUE, "DTS-SLIDE-001"),
    ] {
        check_equal(
            internal,
            &format!("wsi_{name}"),
            "WSI string attribute matches the locked contract.",
            "WSI string attribute does not match the locked contract.",
            element_str(path, obj, tag)?.as_str(),
            locked,
        );
    }
    check_equal(
        internal,
        "wsi_acquisition_context_items",
        "Acquisition Context Sequence is present and empty.",
        "Acquisition Context Sequence is not present and empty.",
        sequence_item_count(path, obj, tags::ACQUISITION_CONTEXT_SEQUENCE)?,
        0,
    );

    for (name, tag, locked) in [
        ("total_pixel_matrix_rows", tags::TOTAL_PIXEL_MATRIX_ROWS, 4),
        (
            "total_pixel_matrix_columns",
            tags::TOTAL_PIXEL_MATRIX_COLUMNS,
            4,
        ),
        ("number_of_optical_paths", tags::NUMBER_OF_OPTICAL_PATHS, 1),
        (
            "total_pixel_matrix_focal_planes",
            tags::TOTAL_PIXEL_MATRIX_FOCAL_PLANES,
            1,
        ),
    ] {
        check_equal(
            internal,
            &format!("wsi_{name}"),
            "WSI tiling cardinality matches the locked contract.",
            "WSI tiling cardinality does not match the locked contract.",
            element_u32(path, obj, tag)?,
            locked,
        );
    }
    for (name, tag, locked) in [
        ("imaged_volume_width", tags::IMAGED_VOLUME_WIDTH, vec![2.0]),
        (
            "imaged_volume_height",
            tags::IMAGED_VOLUME_HEIGHT,
            vec![2.0],
        ),
        (
            "imaged_volume_depth",
            tags::IMAGED_VOLUME_DEPTH,
            vec![0.001],
        ),
        (
            "image_orientation_slide",
            tags::IMAGE_ORIENTATION_SLIDE,
            vec![1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
        ),
    ] {
        check_equal(
            internal,
            &format!("wsi_{name}"),
            "WSI geometry matches the locked contract.",
            "WSI geometry does not match the locked contract.",
            element_f64_values(path, obj, tag)?,
            locked,
        );
    }

    check_equal(
        internal,
        "wsi_origin_items",
        "Total Pixel Matrix Origin has one item.",
        "Total Pixel Matrix Origin cardinality differs from the contract.",
        sequence_item_count(path, obj, tags::TOTAL_PIXEL_MATRIX_ORIGIN_SEQUENCE)?,
        1,
    );
    let origin = top_level_sequence_item(path, obj, tags::TOTAL_PIXEL_MATRIX_ORIGIN_SEQUENCE, 0)?;
    for (name, tag) in [
        ("x", tags::X_OFFSET_IN_SLIDE_COORDINATE_SYSTEM),
        ("y", tags::Y_OFFSET_IN_SLIDE_COORDINATE_SYSTEM),
        ("z", tags::Z_OFFSET_IN_SLIDE_COORDINATE_SYSTEM),
    ] {
        check_equal(
            internal,
            &format!("wsi_origin_{name}"),
            "Total Pixel Matrix Origin component is zero.",
            "Total Pixel Matrix Origin component is not zero.",
            item_f64(path, origin, tag)?,
            0.0,
        );
    }

    check_equal(
        internal,
        "wsi_dimension_organization_items",
        "Dimension Organization Sequence has one item.",
        "Dimension Organization Sequence cardinality differs from the contract.",
        sequence_item_count(path, obj, tags::DIMENSION_ORGANIZATION_SEQUENCE)?,
        1,
    );
    let dimension = top_level_sequence_item(path, obj, tags::DIMENSION_ORGANIZATION_SEQUENCE, 0)?;
    let dimension_uid = item_str(path, dimension, tags::DIMENSION_ORGANIZATION_UID)?;
    check(
        internal,
        valid_dicom_uid(&dimension_uid),
        "wsi_dimension_organization_uid",
        "Dimension Organization UID is syntactically valid.",
        "Dimension Organization UID is not syntactically valid.",
    );

    check_equal(
        internal,
        "wsi_shared_functional_groups_items",
        "Shared Functional Groups Sequence has one item.",
        "Shared Functional Groups Sequence cardinality differs from the contract.",
        sequence_item_count(path, obj, tags::SHARED_FUNCTIONAL_GROUPS_SEQUENCE)?,
        1,
    );
    let shared = top_level_sequence_item(path, obj, tags::SHARED_FUNCTIONAL_GROUPS_SEQUENCE, 0)?;
    check_equal(
        internal,
        "wsi_pixel_measures_items",
        "Shared Pixel Measures Sequence has one item.",
        "Shared Pixel Measures Sequence cardinality differs from the contract.",
        item_sequence_item_count(path, shared, tags::PIXEL_MEASURES_SEQUENCE)?,
        1,
    );
    let measures = item_sequence_item(path, shared, tags::PIXEL_MEASURES_SEQUENCE, 0)?;
    check_equal(
        internal,
        "wsi_pixel_spacing",
        "Pixel Spacing matches the locked geometry.",
        "Pixel Spacing does not match the locked geometry.",
        item_f64_values(path, measures, tags::PIXEL_SPACING)?,
        vec![0.5, 0.5],
    );
    check_equal(
        internal,
        "wsi_slice_thickness",
        "Slice Thickness matches the locked geometry.",
        "Slice Thickness does not match the locked geometry.",
        item_f64(path, measures, tags::SLICE_THICKNESS)?,
        0.001,
    );
    check_equal(
        internal,
        "wsi_frame_type_items",
        "Shared WSI Frame Type Sequence has one item.",
        "Shared WSI Frame Type Sequence cardinality differs from the contract.",
        item_sequence_item_count(
            path,
            shared,
            tags::WHOLE_SLIDE_MICROSCOPY_IMAGE_FRAME_TYPE_SEQUENCE,
        )?,
        1,
    );
    check_equal(
        internal,
        "wsi_frame_type",
        "Shared Frame Type matches Image Type.",
        "Shared Frame Type differs from the locked Image Type.",
        nested_sequence_item_str(
            path,
            shared,
            tags::WHOLE_SLIDE_MICROSCOPY_IMAGE_FRAME_TYPE_SEQUENCE,
            0,
            tags::FRAME_TYPE,
        )?,
        "ORIGINAL\\PRIMARY\\VOLUME\\NONE".to_string(),
    );

    validate_wsi_specimen(path, obj, internal, expected_specimen_uid)?;
    validate_wsi_optical_path(path, obj, internal, ICC_HASH)?;

    let pixel = obj
        .element(tags::PIXEL_DATA)
        .map_err(|err| validation_error(path, err))?;
    let bytes = pixel
        .value()
        .to_bytes()
        .map_err(|err| validation_error(path, err))?;
    check_equal(
        internal,
        "wsi_pixel_vr",
        "Pixel Data uses native OB storage.",
        "Pixel Data does not use native OB storage.",
        pixel.vr(),
        VR::OB,
    );
    check_equal(
        internal,
        "wsi_pixel_length",
        "Pixel Data contains four 2x2 RGB frames.",
        "Pixel Data length differs from four 2x2 RGB frames.",
        bytes.len(),
        48,
    );
    for (index, locked) in FRAME_HASHES.iter().enumerate() {
        let start = index * 12;
        let actual = bytes.get(start..start + 12).map(sha256_hex);
        check_equal(
            internal,
            &format!("wsi_frame_{}_sha256", index + 1),
            "Implicitly ordered tile frame hash matches the contract.",
            "Implicitly ordered tile frame hash does not match the contract.",
            actual.as_deref(),
            Some(*locked),
        );
    }
    let matrix = reconstruct_tiled_full_matrix(bytes.as_ref());
    check_equal(
        internal,
        "wsi_total_pixel_matrix_sha256",
        "Implicit frame order reconstructs the locked 4x4 matrix.",
        "Implicit frame order reconstructs a different 4x4 matrix.",
        matrix.as_deref().map(sha256_hex).as_deref(),
        Some(MATRIX_HASH),
    );

    for (name, tags_to_check) in [
        (
            "per_frame_functional_groups",
            &[tags::PER_FRAME_FUNCTIONAL_GROUPS_SEQUENCE][..],
        ),
        ("dimension_index", &[tags::DIMENSION_INDEX_SEQUENCE][..]),
        (
            "references",
            &[
                tags::REFERENCED_SERIES_SEQUENCE,
                tags::STUDIES_CONTAINING_OTHER_REFERENCED_INSTANCES_SEQUENCE,
            ][..],
        ),
        (
            "concatenation",
            &[
                tags::CONCATENATION_UID,
                tags::IN_CONCATENATION_NUMBER,
                tags::CONCATENATION_FRAME_OFFSET_NUMBER,
                tags::SOP_INSTANCE_UID_OF_CONCATENATION_SOURCE,
            ][..],
        ),
        ("multi_resolution_pyramid", &[tags::PYRAMID_UID][..]),
        (
            "extended_depth_of_field_detail",
            &[
                tags::NUMBER_OF_FOCAL_PLANES,
                tags::DISTANCE_BETWEEN_FOCAL_PLANES,
            ][..],
        ),
        (
            "lossy_detail",
            &[
                tags::LOSSY_IMAGE_COMPRESSION_RATIO,
                tags::LOSSY_IMAGE_COMPRESSION_METHOD,
            ][..],
        ),
        (
            "specimen_reference",
            &[tags::SPECIMEN_REFERENCE_SEQUENCE][..],
        ),
    ] {
        check(
            internal,
            tags_to_check
                .iter()
                .all(|tag| obj.element_opt(*tag).is_ok_and(|value| value.is_none())),
            &format!("wsi_{name}_absent"),
            "Locked optional content is absent.",
            "Locked optional content is unexpectedly present.",
        );
    }
    Ok(())
}

fn validate_wsi_specimen(
    path: &Path,
    obj: &OpenedObject,
    internal: &mut Vec<Value>,
    specimen_uid: &str,
) -> Result<(), GenerateError> {
    check_equal(
        internal,
        "wsi_container_identifier",
        "Container Identifier matches the locked slide.",
        "Container Identifier does not match the locked slide.",
        element_str(path, obj, tags::CONTAINER_IDENTIFIER)?.as_str(),
        "DTS-SLIDE-001",
    );
    for (name, tag) in [
        (
            "container_issuer",
            tags::ISSUER_OF_THE_CONTAINER_IDENTIFIER_SEQUENCE,
        ),
        ("container_type", tags::CONTAINER_TYPE_CODE_SEQUENCE),
    ] {
        check_equal(
            internal,
            &format!("wsi_{name}_items"),
            "Required Type 2 sequence is present and empty.",
            "Required Type 2 sequence is not present and empty.",
            sequence_item_count(path, obj, tag)?,
            0,
        );
    }
    check_equal(
        internal,
        "wsi_specimen_description_items",
        "Specimen Description Sequence has one item.",
        "Specimen Description Sequence cardinality differs from the contract.",
        sequence_item_count(path, obj, tags::SPECIMEN_DESCRIPTION_SEQUENCE)?,
        1,
    );
    let specimen = top_level_sequence_item(path, obj, tags::SPECIMEN_DESCRIPTION_SEQUENCE, 0)?;
    check_equal(
        internal,
        "wsi_specimen_identifier",
        "Specimen Identifier matches the locked specimen.",
        "Specimen Identifier does not match the locked specimen.",
        item_str(path, specimen, tags::SPECIMEN_IDENTIFIER)?.as_str(),
        "DTS-SPECIMEN-001",
    );
    check_equal(
        internal,
        "wsi_specimen_uid",
        "Specimen UID matches the manifest contract.",
        "Specimen UID does not match the manifest contract.",
        item_str(path, specimen, tags::SPECIMEN_UID)?.as_str(),
        specimen_uid,
    );
    for (name, tag) in [
        (
            "specimen_issuer",
            tags::ISSUER_OF_THE_SPECIMEN_IDENTIFIER_SEQUENCE,
        ),
        ("specimen_preparation", tags::SPECIMEN_PREPARATION_SEQUENCE),
    ] {
        check_equal(
            internal,
            &format!("wsi_{name}_items"),
            "Required specimen Type 2 sequence is present and empty.",
            "Required specimen Type 2 sequence is not present and empty.",
            item_sequence_item_count(path, specimen, tag)?,
            0,
        );
    }
    Ok(())
}

fn validate_wsi_optical_path(
    path: &Path,
    obj: &OpenedObject,
    internal: &mut Vec<Value>,
    icc_hash: &str,
) -> Result<(), GenerateError> {
    check_equal(
        internal,
        "wsi_optical_path_items",
        "Optical Path Sequence has one item.",
        "Optical Path Sequence cardinality differs from the contract.",
        sequence_item_count(path, obj, tags::OPTICAL_PATH_SEQUENCE)?,
        1,
    );
    let optical = top_level_sequence_item(path, obj, tags::OPTICAL_PATH_SEQUENCE, 0)?;
    check_equal(
        internal,
        "wsi_optical_path_identifier",
        "Optical Path Identifier is RGB.",
        "Optical Path Identifier is not RGB.",
        item_str(path, optical, tags::OPTICAL_PATH_IDENTIFIER)?.as_str(),
        "RGB",
    );
    check_equal(
        internal,
        "wsi_illumination_wavelength",
        "Illumination Wave Length is 550 nm.",
        "Illumination Wave Length is not 550 nm.",
        item_f64(path, optical, tags::ILLUMINATION_WAVE_LENGTH)?,
        550.0,
    );
    check_equal(
        internal,
        "wsi_color_space",
        "Optical path DICOM Color Space is SRGB.",
        "Optical path DICOM Color Space is not SRGB.",
        item_str(path, optical, tags::COLOR_SPACE)?.as_str(),
        "SRGB",
    );
    check_equal(
        internal,
        "wsi_illumination_code_items",
        "Illumination Type Code Sequence has one item.",
        "Illumination Type Code Sequence cardinality differs from the contract.",
        item_sequence_item_count(path, optical, tags::ILLUMINATION_TYPE_CODE_SEQUENCE)?,
        1,
    );
    let code = item_sequence_item(path, optical, tags::ILLUMINATION_TYPE_CODE_SEQUENCE, 0)?;
    for (name, tag, locked) in [
        ("value", tags::CODE_VALUE, "111744"),
        ("scheme", tags::CODING_SCHEME_DESIGNATOR, "DCM"),
        ("meaning", tags::CODE_MEANING, "Brightfield illumination"),
    ] {
        check_equal(
            internal,
            &format!("wsi_illumination_code_{name}"),
            "Illumination code matches the locked contract.",
            "Illumination code does not match the locked contract.",
            item_str(path, code, tag)?.as_str(),
            locked,
        );
    }
    let icc = optical
        .element(tags::ICC_PROFILE)
        .map_err(|err| validation_error(path, err))?;
    let bytes = icc
        .value()
        .to_bytes()
        .map_err(|err| validation_error(path, err))?;
    check_equal(
        internal,
        "wsi_icc_vr",
        "ICC Profile uses OB storage.",
        "ICC Profile does not use OB storage.",
        icc.vr(),
        VR::OB,
    );
    check_equal(
        internal,
        "wsi_icc_size",
        "ICC Profile has the locked 736-byte size.",
        "ICC Profile size differs from the locked size.",
        bytes.len(),
        736,
    );
    check_equal(
        internal,
        "wsi_icc_sha256",
        "ICC Profile SHA-256 matches the locked profile.",
        "ICC Profile SHA-256 differs from the locked profile.",
        sha256_hex(bytes.as_ref()).as_str(),
        icc_hash,
    );
    check(
        internal,
        bytes.len() >= 40
            && &bytes[12..16] == b"scnr"
            && &bytes[16..20] == b"RGB "
            && &bytes[20..24] == b"XYZ "
            && &bytes[36..40] == b"acsp",
        "wsi_icc_header",
        "ICC header declares scanner RGB to XYZ with acsp signature.",
        "ICC header does not match the locked scanner RGB profile.",
    );
    Ok(())
}

fn reconstruct_tiled_full_matrix(pixel_bytes: &[u8]) -> Option<Vec<u8>> {
    if pixel_bytes.len() != 48 {
        return None;
    }
    let mut matrix = vec![0_u8; 48];
    for frame in 0..4 {
        let tile_row = frame / 2;
        let tile_column = frame % 2;
        for row in 0..2 {
            let source = frame * 12 + row * 6;
            let destination = ((tile_row * 2 + row) * 4 + tile_column * 2) * 3;
            matrix[destination..destination + 6].copy_from_slice(&pixel_bytes[source..source + 6]);
        }
    }
    Some(matrix)
}

/// Validate the complete, case-scoped Phase 4 TILED_SPARSE WSI contract.
pub(crate) fn validate_wsi_tiled_sparse_file(
    path: &Path,
    identity: &Part10Expectations<'_>,
    expected_wsi_tiled_sparse: &Value,
) -> Result<ValidatedPart10, GenerateError> {
    let mut validated = validate_part10_file(path, identity)?;
    let obj = open_file(path).map_err(|err| GenerateError::ValidateDicomFile {
        path: path.to_path_buf(),
        message: err.to_string(),
    })?;
    let mut internal = Vec::new();
    validate_wsi_tiled_sparse(
        path,
        &obj,
        &mut internal,
        identity,
        expected_wsi_tiled_sparse,
    )?;
    fail_if_any_failed(path, &internal)?;

    let validation_items = validated
        .validation
        .get_mut("internal")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| GenerateError::ValidateDicomFile {
            path: path.to_path_buf(),
            message: "generic validation result has no internal findings array".to_string(),
        })?;
    validation_items.extend(internal);
    if let Some(standards) = validated
        .validation
        .get_mut("standards")
        .and_then(Value::as_array_mut)
    {
        standards.push(serde_json::json!({
            "name": "vl_whole_slide_microscopy_tiled_sparse_contract",
            "status": "passed",
            "message": "TILED_SPARSE WSI dimension indices, explicit per-frame positions, optical-path references, pixels, occupancy, sentinel reconstruction, specimen, ICC profile, and locked absences match the canonical manifest contract."
        }));
    }
    Ok(validated)
}

fn validate_wsi_tiled_sparse(
    path: &Path,
    obj: &OpenedObject,
    internal: &mut Vec<Value>,
    identity: &Part10Expectations<'_>,
    expected: &Value,
) -> Result<(), GenerateError> {
    const WSI_SOP_CLASS_UID: &str = "1.2.840.10008.5.1.4.1.1.77.1.6";
    const FRAME_HASHES: [&str; 2] = [
        "fcf067f6323bb42b8292a565a8f826ec5fdb1b142b7a69bf7f7721f0d5d46ef8",
        "8688d249e9d047b4fc2fb89ce05afe9ec89252ffccdd969de6eef260dd7ffb21",
    ];
    const PAYLOAD_HASH: &str = "94a57aca44c4a97d424e8e546b2673fa91f711694de1ccb36f062aabbc9b55ee";
    const MATRIX_HASH: &str = "d10a587875f14a0b74a9e4935ce83cdb73377bd7357a172db8e9f7347c030eb3";
    const ICC_HASH: &str = "8e069a3476b71a0e0ae7272d9278ba70540d1c4a0b19af1c7d52e56f49091fef";

    let expected_for = expected
        .pointer("/frame_of_reference_uid")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let expected_dimension_uid = expected
        .pointer("/dimension_organization_uid")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let expected_specimen_uid = expected
        .pointer("/specimen/specimen_uid")
        .and_then(Value::as_str)
        .unwrap_or_default();
    check_equal(
        internal,
        "wsi_sparse_expected_contract",
        "Manifest-derived sparse WSI expectation equals the canonical locked contract.",
        "Manifest-derived sparse WSI expectation differs from the canonical locked contract.",
        expected,
        &crate::wsi_tiled_sparse_locked_contract(
            expected_for,
            expected_specimen_uid,
            expected_dimension_uid,
        ),
    );
    check(
        internal,
        identity.sop_class_uid == WSI_SOP_CLASS_UID
            && identity.transfer_syntax_uid == uids::EXPLICIT_VR_LITTLE_ENDIAN
            && identity.rows == 2
            && identity.columns == 2
            && identity.frames == 2
            && identity.samples_per_pixel == 3
            && identity.photometric_interpretation == "RGB"
            && identity.planar_configuration == Some(0)
            && identity.bits_allocated == 8
            && identity.bits_stored == 8
            && identity.high_bit == 7
            && identity.pixel_representation == 0
            && identity.pixel_data_vr == VR::OB
            && matches!(
                identity.pixel_data_length_formula,
                PixelDataLengthFormula::ContiguousSamples
            ),
        "wsi_sparse_identity_contract",
        "Manifest image identity matches the locked native TILED_SPARSE WSI contract.",
        "Manifest image identity differs from the locked native TILED_SPARSE WSI contract.",
    );

    for (name, tag, locked) in [
        ("modality", tags::MODALITY, "SM"),
        (
            "frame_of_reference_uid",
            tags::FRAME_OF_REFERENCE_UID,
            expected_for,
        ),
        (
            "position_reference_indicator",
            tags::POSITION_REFERENCE_INDICATOR,
            "SLIDE_CORNER",
        ),
        (
            "image_type",
            tags::IMAGE_TYPE,
            "ORIGINAL\\PRIMARY\\VOLUME\\NONE",
        ),
        (
            "acquisition_date_time",
            tags::ACQUISITION_DATE_TIME,
            "20260101000000",
        ),
        (
            "volumetric_properties",
            tags::VOLUMETRIC_PROPERTIES,
            "VOLUME",
        ),
        (
            "specimen_label_in_image",
            tags::SPECIMEN_LABEL_IN_IMAGE,
            "NO",
        ),
        ("burned_in_annotation", tags::BURNED_IN_ANNOTATION, "NO"),
        ("focus_method", tags::FOCUS_METHOD, "AUTO"),
        (
            "extended_depth_of_field",
            tags::EXTENDED_DEPTH_OF_FIELD,
            "NO",
        ),
        (
            "lossy_image_compression",
            tags::LOSSY_IMAGE_COMPRESSION,
            "00",
        ),
        (
            "dimension_organization_type",
            tags::DIMENSION_ORGANIZATION_TYPE,
            "TILED_SPARSE",
        ),
        ("tiles_overlap", tags::TILES_OVERLAP, "NONE"),
        ("label_text", tags::LABEL_TEXT, "DTS SYNTHETIC SLIDE 001"),
        ("barcode_value", tags::BARCODE_VALUE, "DTS-SLIDE-001"),
    ] {
        check_equal(
            internal,
            &format!("wsi_sparse_{name}"),
            "Sparse WSI string attribute matches the locked contract.",
            "Sparse WSI string attribute does not match the locked contract.",
            element_str(path, obj, tag)?.as_str(),
            locked,
        );
    }
    check_equal(
        internal,
        "wsi_sparse_acquisition_context_items",
        "Acquisition Context Sequence is present and empty.",
        "Acquisition Context Sequence is not present and empty.",
        sequence_item_count(path, obj, tags::ACQUISITION_CONTEXT_SEQUENCE)?,
        0,
    );

    for (name, tag, locked) in [
        ("total_pixel_matrix_rows", tags::TOTAL_PIXEL_MATRIX_ROWS, 4),
        (
            "total_pixel_matrix_columns",
            tags::TOTAL_PIXEL_MATRIX_COLUMNS,
            4,
        ),
        ("number_of_optical_paths", tags::NUMBER_OF_OPTICAL_PATHS, 1),
        (
            "total_pixel_matrix_focal_planes",
            tags::TOTAL_PIXEL_MATRIX_FOCAL_PLANES,
            1,
        ),
    ] {
        check_equal(
            internal,
            &format!("wsi_sparse_{name}"),
            "Sparse WSI tiling cardinality matches the locked contract.",
            "Sparse WSI tiling cardinality does not match the locked contract.",
            element_u32(path, obj, tag)?,
            locked,
        );
    }
    for (name, tag, locked) in [
        ("imaged_volume_width", tags::IMAGED_VOLUME_WIDTH, vec![2.0]),
        (
            "imaged_volume_height",
            tags::IMAGED_VOLUME_HEIGHT,
            vec![2.0],
        ),
        (
            "imaged_volume_depth",
            tags::IMAGED_VOLUME_DEPTH,
            vec![0.001],
        ),
        (
            "image_orientation_slide",
            tags::IMAGE_ORIENTATION_SLIDE,
            vec![1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
        ),
    ] {
        check_equal(
            internal,
            &format!("wsi_sparse_{name}"),
            "Sparse WSI geometry matches the locked contract.",
            "Sparse WSI geometry does not match the locked contract.",
            element_f64_values(path, obj, tag)?,
            locked,
        );
    }

    check_equal(
        internal,
        "wsi_sparse_origin_items",
        "Total Pixel Matrix Origin has one item.",
        "Total Pixel Matrix Origin cardinality differs from the contract.",
        sequence_item_count(path, obj, tags::TOTAL_PIXEL_MATRIX_ORIGIN_SEQUENCE)?,
        1,
    );
    let origin = top_level_sequence_item(path, obj, tags::TOTAL_PIXEL_MATRIX_ORIGIN_SEQUENCE, 0)?;
    for (name, tag) in [
        ("x", tags::X_OFFSET_IN_SLIDE_COORDINATE_SYSTEM),
        ("y", tags::Y_OFFSET_IN_SLIDE_COORDINATE_SYSTEM),
        ("z", tags::Z_OFFSET_IN_SLIDE_COORDINATE_SYSTEM),
    ] {
        check_equal(
            internal,
            &format!("wsi_sparse_origin_{name}"),
            "Sparse WSI origin component is zero.",
            "Sparse WSI origin component is not zero.",
            item_f64(path, origin, tag)?,
            0.0,
        );
    }

    check_equal(
        internal,
        "wsi_sparse_dimension_organization_items",
        "Dimension Organization Sequence has one item.",
        "Dimension Organization Sequence cardinality differs from the contract.",
        sequence_item_count(path, obj, tags::DIMENSION_ORGANIZATION_SEQUENCE)?,
        1,
    );
    let organization =
        top_level_sequence_item(path, obj, tags::DIMENSION_ORGANIZATION_SEQUENCE, 0)?;
    check_equal(
        internal,
        "wsi_sparse_dimension_organization_uid",
        "Dimension Organization UID matches the manifest contract.",
        "Dimension Organization UID does not match the manifest contract.",
        item_str(path, organization, tags::DIMENSION_ORGANIZATION_UID)?.as_str(),
        expected_dimension_uid,
    );
    check(
        internal,
        valid_dicom_uid(expected_dimension_uid),
        "wsi_sparse_expected_dimension_uid",
        "Manifest Dimension Organization UID is syntactically valid.",
        "Manifest Dimension Organization UID is not syntactically valid.",
    );

    check_equal(
        internal,
        "wsi_sparse_dimension_index_items",
        "Dimension Index Sequence has exactly two ordered items.",
        "Dimension Index Sequence does not have exactly two ordered items.",
        sequence_item_count(path, obj, tags::DIMENSION_INDEX_SEQUENCE)?,
        2,
    );
    for (index, pointer, label) in [
        (
            0,
            tags::COLUMN_POSITION_IN_TOTAL_IMAGE_PIXEL_MATRIX,
            "Column Position",
        ),
        (
            1,
            tags::ROW_POSITION_IN_TOTAL_IMAGE_PIXEL_MATRIX,
            "Row Position",
        ),
    ] {
        let item = top_level_sequence_item(path, obj, tags::DIMENSION_INDEX_SEQUENCE, index)?;
        check(
            internal,
            item.iter().count() == 4
                && item.iter().all(|element| {
                    matches!(
                        element.tag(),
                        tags::DIMENSION_INDEX_POINTER
                            | tags::FUNCTIONAL_GROUP_POINTER
                            | tags::DIMENSION_ORGANIZATION_UID
                            | tags::DIMENSION_DESCRIPTION_LABEL
                    )
                }),
            &format!("wsi_sparse_dimension_index_{}_attributes", index + 1),
            "Dimension index item contains exactly the four locked attributes.",
            "Dimension index item contains an unexpected or missing attribute.",
        );
        check_equal(
            internal,
            &format!("wsi_sparse_dimension_index_{}_pointer_vr", index + 1),
            "Dimension Index Pointer uses AT VR.",
            "Dimension Index Pointer does not use AT VR.",
            item.element(tags::DIMENSION_INDEX_POINTER)
                .map_err(|err| validation_error(path, err))?
                .vr(),
            VR::AT,
        );
        check_equal(
            internal,
            &format!("wsi_sparse_dimension_index_{}_pointer", index + 1),
            "Dimension Index Pointer matches the locked ordered dimension.",
            "Dimension Index Pointer does not match the locked ordered dimension.",
            item_tag(path, item, tags::DIMENSION_INDEX_POINTER)?,
            pointer,
        );
        check_equal(
            internal,
            &format!(
                "wsi_sparse_dimension_index_{}_functional_group_vr",
                index + 1
            ),
            "Functional Group Pointer uses AT VR.",
            "Functional Group Pointer does not use AT VR.",
            item.element(tags::FUNCTIONAL_GROUP_POINTER)
                .map_err(|err| validation_error(path, err))?
                .vr(),
            VR::AT,
        );
        check_equal(
            internal,
            &format!("wsi_sparse_dimension_index_{}_functional_group", index + 1),
            "Functional Group Pointer identifies Plane Position (Slide).",
            "Functional Group Pointer does not identify Plane Position (Slide).",
            item_tag(path, item, tags::FUNCTIONAL_GROUP_POINTER)?,
            tags::PLANE_POSITION_SLIDE_SEQUENCE,
        );
        check_equal(
            internal,
            &format!("wsi_sparse_dimension_index_{}_organization_uid", index + 1),
            "Dimension index references the locked organization UID.",
            "Dimension index references a different organization UID.",
            item_str(path, item, tags::DIMENSION_ORGANIZATION_UID)?.as_str(),
            expected_dimension_uid,
        );
        check_equal(
            internal,
            &format!("wsi_sparse_dimension_index_{}_label", index + 1),
            "Dimension Description Label matches the locked dimension.",
            "Dimension Description Label does not match the locked dimension.",
            item_str(path, item, tags::DIMENSION_DESCRIPTION_LABEL)?.as_str(),
            label,
        );
        check(
            internal,
            item.element_opt(tags::DIMENSION_INDEX_PRIVATE_CREATOR)
                .is_ok_and(|value| value.is_none())
                && item
                    .element_opt(tags::FUNCTIONAL_GROUP_PRIVATE_CREATOR)
                    .is_ok_and(|value| value.is_none()),
            &format!(
                "wsi_sparse_dimension_index_{}_private_creators_absent",
                index + 1
            ),
            "Dimension index private-creator attributes are absent.",
            "Dimension index unexpectedly contains private-creator attributes.",
        );
    }

    validate_wsi_sparse_shared_groups(path, obj, internal)?;
    validate_wsi_specimen(path, obj, internal, expected_specimen_uid)?;
    validate_wsi_optical_path(path, obj, internal, ICC_HASH)?;

    let positions = validate_wsi_sparse_per_frame(path, obj, internal)?;
    let pixel = obj
        .element(tags::PIXEL_DATA)
        .map_err(|err| validation_error(path, err))?;
    let bytes = pixel
        .value()
        .to_bytes()
        .map_err(|err| validation_error(path, err))?;
    check_equal(
        internal,
        "wsi_sparse_pixel_vr",
        "Pixel Data uses native OB storage.",
        "Pixel Data does not use native OB storage.",
        pixel.vr(),
        VR::OB,
    );
    check_equal(
        internal,
        "wsi_sparse_pixel_length",
        "Pixel Data contains two 2x2 RGB frames.",
        "Pixel Data length differs from two 2x2 RGB frames.",
        bytes.len(),
        24,
    );
    check_equal(
        internal,
        "wsi_sparse_payload_sha256",
        "Stored sparse payload hash matches.",
        "Stored sparse payload hash differs.",
        sha256_hex(bytes.as_ref()).as_str(),
        PAYLOAD_HASH,
    );
    for (index, locked) in FRAME_HASHES.iter().enumerate() {
        let start = index * 12;
        let actual = bytes.get(start..start + 12).map(sha256_hex);
        check_equal(
            internal,
            &format!("wsi_sparse_frame_{}_sha256", index + 1),
            "Sparse tile frame hash matches the contract.",
            "Sparse tile frame hash does not match the contract.",
            actual.as_deref(),
            Some(*locked),
        );
    }
    let reconstructed = reconstruct_tiled_sparse_matrix(bytes.as_ref(), &positions);
    check_equal(
        internal,
        "wsi_sparse_occupancy_mask",
        "Explicit positions reconstruct the locked present/absent occupancy mask.",
        "Explicit positions reconstruct a different occupancy mask.",
        reconstructed
            .as_ref()
            .map(|(_, occupancy)| occupancy.as_slice()),
        Some([true, false, false, true].as_slice()),
    );
    check_equal(
        internal,
        "wsi_sparse_sentinel_matrix_sha256",
        "Explicit positions reconstruct the locked black-sentinel matrix.",
        "Explicit positions reconstruct a different black-sentinel matrix.",
        reconstructed
            .as_ref()
            .map(|(matrix, _)| sha256_hex(matrix))
            .as_deref(),
        Some(MATRIX_HASH),
    );

    for (name, tags_to_check) in [
        (
            "references",
            &[
                tags::REFERENCED_SERIES_SEQUENCE,
                tags::STUDIES_CONTAINING_OTHER_REFERENCED_INSTANCES_SEQUENCE,
            ][..],
        ),
        (
            "concatenation",
            &[
                tags::CONCATENATION_UID,
                tags::IN_CONCATENATION_NUMBER,
                tags::CONCATENATION_FRAME_OFFSET_NUMBER,
                tags::SOP_INSTANCE_UID_OF_CONCATENATION_SOURCE,
            ][..],
        ),
        ("multi_resolution_pyramid", &[tags::PYRAMID_UID][..]),
        (
            "extended_depth_of_field_detail",
            &[
                tags::NUMBER_OF_FOCAL_PLANES,
                tags::DISTANCE_BETWEEN_FOCAL_PLANES,
            ][..],
        ),
        (
            "lossy_detail",
            &[
                tags::LOSSY_IMAGE_COMPRESSION_RATIO,
                tags::LOSSY_IMAGE_COMPRESSION_METHOD,
            ][..],
        ),
        (
            "specimen_reference",
            &[tags::SPECIMEN_REFERENCE_SEQUENCE][..],
        ),
        ("top_level_icc_profile", &[tags::ICC_PROFILE][..]),
    ] {
        check(
            internal,
            tags_to_check
                .iter()
                .all(|tag| obj.element_opt(*tag).is_ok_and(|value| value.is_none())),
            &format!("wsi_sparse_{name}_absent"),
            "Locked optional content is absent.",
            "Locked optional content is unexpectedly present.",
        );
    }
    Ok(())
}

fn validate_wsi_sparse_shared_groups(
    path: &Path,
    obj: &OpenedObject,
    internal: &mut Vec<Value>,
) -> Result<(), GenerateError> {
    check_equal(
        internal,
        "wsi_sparse_shared_functional_groups_items",
        "Shared Functional Groups Sequence has one item.",
        "Shared Functional Groups Sequence cardinality differs.",
        sequence_item_count(path, obj, tags::SHARED_FUNCTIONAL_GROUPS_SEQUENCE)?,
        1,
    );
    let shared = top_level_sequence_item(path, obj, tags::SHARED_FUNCTIONAL_GROUPS_SEQUENCE, 0)?;
    check(
        internal,
        shared.iter().count() == 2
            && shared.iter().all(|element| {
                matches!(
                    element.tag(),
                    tags::PIXEL_MEASURES_SEQUENCE
                        | tags::WHOLE_SLIDE_MICROSCOPY_IMAGE_FRAME_TYPE_SEQUENCE
                )
            }),
        "wsi_sparse_shared_macro_set",
        "Shared Functional Groups contain exactly Pixel Measures and WSI Frame Type.",
        "Shared Functional Groups contain an unexpected or missing Macro.",
    );
    let measures = item_sequence_item(path, shared, tags::PIXEL_MEASURES_SEQUENCE, 0)?;
    check_equal(
        internal,
        "wsi_sparse_pixel_measures_items",
        "Pixel Measures has one item.",
        "Pixel Measures item count differs.",
        item_sequence_item_count(path, shared, tags::PIXEL_MEASURES_SEQUENCE)?,
        1,
    );
    check_equal(
        internal,
        "wsi_sparse_pixel_spacing",
        "Pixel Spacing matches.",
        "Pixel Spacing differs.",
        item_f64_values(path, measures, tags::PIXEL_SPACING)?,
        vec![0.5, 0.5],
    );
    check_equal(
        internal,
        "wsi_sparse_slice_thickness",
        "Slice Thickness matches.",
        "Slice Thickness differs.",
        item_f64(path, measures, tags::SLICE_THICKNESS)?,
        0.001,
    );
    check_equal(
        internal,
        "wsi_sparse_frame_type_items",
        "Shared WSI Frame Type has one item.",
        "Shared WSI Frame Type count differs.",
        item_sequence_item_count(
            path,
            shared,
            tags::WHOLE_SLIDE_MICROSCOPY_IMAGE_FRAME_TYPE_SEQUENCE,
        )?,
        1,
    );
    check_equal(
        internal,
        "wsi_sparse_frame_type",
        "Shared Frame Type matches Image Type.",
        "Shared Frame Type differs.",
        nested_sequence_item_str(
            path,
            shared,
            tags::WHOLE_SLIDE_MICROSCOPY_IMAGE_FRAME_TYPE_SEQUENCE,
            0,
            tags::FRAME_TYPE,
        )?,
        "ORIGINAL\\PRIMARY\\VOLUME\\NONE".to_string(),
    );
    Ok(())
}

fn validate_wsi_sparse_per_frame(
    path: &Path,
    obj: &OpenedObject,
    internal: &mut Vec<Value>,
) -> Result<Vec<(usize, usize)>, GenerateError> {
    const LOCKED: [([u32; 2], usize, usize, f64, f64); 2] =
        [([1, 1], 1, 1, 0.0, 0.0), ([2, 2], 3, 3, 1.0, 1.0)];
    check_equal(
        internal,
        "wsi_sparse_per_frame_items",
        "Per-Frame Functional Groups has two items.",
        "Per-Frame Functional Groups count differs.",
        sequence_item_count(path, obj, tags::PER_FRAME_FUNCTIONAL_GROUPS_SEQUENCE)?,
        2,
    );
    let mut positions = Vec::with_capacity(2);
    for (index, (dimension_values, column, row, x, y)) in LOCKED.into_iter().enumerate() {
        let frame =
            top_level_sequence_item(path, obj, tags::PER_FRAME_FUNCTIONAL_GROUPS_SEQUENCE, index)?;
        check(
            internal,
            frame.iter().count() == 3
                && frame.iter().all(|element| {
                    matches!(
                        element.tag(),
                        tags::FRAME_CONTENT_SEQUENCE
                            | tags::PLANE_POSITION_SLIDE_SEQUENCE
                            | tags::OPTICAL_PATH_IDENTIFICATION_SEQUENCE
                    )
                }),
            &format!("wsi_sparse_frame_{}_macro_set", index + 1),
            "Per-frame item contains exactly the three locked Macros.",
            "Per-frame item contains an unexpected or missing Macro.",
        );
        for (name, tag) in [
            ("frame_content", tags::FRAME_CONTENT_SEQUENCE),
            ("plane_position", tags::PLANE_POSITION_SLIDE_SEQUENCE),
            ("optical_path", tags::OPTICAL_PATH_IDENTIFICATION_SEQUENCE),
        ] {
            check_equal(
                internal,
                &format!("wsi_sparse_frame_{}_{}_items", index + 1, name),
                "Per-frame Macro has one item.",
                "Per-frame Macro count differs.",
                item_sequence_item_count(path, frame, tag)?,
                1,
            );
        }
        let content = item_sequence_item(path, frame, tags::FRAME_CONTENT_SEQUENCE, 0)?;
        check(
            internal,
            content.iter().count() == 1
                && content
                    .iter()
                    .all(|element| element.tag() == tags::DIMENSION_INDEX_VALUES),
            &format!("wsi_sparse_frame_{}_content_attributes", index + 1),
            "Frame Content contains exactly Dimension Index Values.",
            "Frame Content contains an unexpected or missing attribute.",
        );
        check_equal(
            internal,
            &format!("wsi_sparse_frame_{}_dimension_index_vr", index + 1),
            "Dimension Index Values use UL VR.",
            "Dimension Index Values do not use UL VR.",
            content
                .element(tags::DIMENSION_INDEX_VALUES)
                .map_err(|err| validation_error(path, err))?
                .vr(),
            VR::UL,
        );
        check_equal(
            internal,
            &format!("wsi_sparse_frame_{}_dimension_index_values", index + 1),
            "Dimension Index Values match the locked ordinals.",
            "Dimension Index Values differ from the locked ordinals.",
            item_u32_values(path, content, tags::DIMENSION_INDEX_VALUES)?,
            dimension_values.to_vec(),
        );
        let plane = item_sequence_item(path, frame, tags::PLANE_POSITION_SLIDE_SEQUENCE, 0)?;
        check(
            internal,
            plane.iter().count() == 5
                && plane.iter().all(|element| {
                    matches!(
                        element.tag(),
                        tags::COLUMN_POSITION_IN_TOTAL_IMAGE_PIXEL_MATRIX
                            | tags::ROW_POSITION_IN_TOTAL_IMAGE_PIXEL_MATRIX
                            | tags::X_OFFSET_IN_SLIDE_COORDINATE_SYSTEM
                            | tags::Y_OFFSET_IN_SLIDE_COORDINATE_SYSTEM
                            | tags::Z_OFFSET_IN_SLIDE_COORDINATE_SYSTEM
                    )
                }),
            &format!("wsi_sparse_frame_{}_plane_attributes", index + 1),
            "Plane Position (Slide) contains exactly the locked position attributes.",
            "Plane Position (Slide) contains an unexpected or missing attribute.",
        );
        check(
            internal,
            [
                tags::COLUMN_POSITION_IN_TOTAL_IMAGE_PIXEL_MATRIX,
                tags::ROW_POSITION_IN_TOTAL_IMAGE_PIXEL_MATRIX,
            ]
            .iter()
            .all(|tag| {
                plane
                    .element(*tag)
                    .is_ok_and(|element| element.vr() == VR::SL)
            }),
            &format!("wsi_sparse_frame_{}_position_vrs", index + 1),
            "Matrix row and column positions use SL VR.",
            "Matrix row or column position does not use SL VR.",
        );
        let actual_column = item_u32(
            path,
            plane,
            tags::COLUMN_POSITION_IN_TOTAL_IMAGE_PIXEL_MATRIX,
        )? as usize;
        let actual_row =
            item_u32(path, plane, tags::ROW_POSITION_IN_TOTAL_IMAGE_PIXEL_MATRIX)? as usize;
        for (name, actual, locked) in [("column", actual_column, column), ("row", actual_row, row)]
        {
            check_equal(
                internal,
                &format!("wsi_sparse_frame_{}_{}_position", index + 1, name),
                "Tile matrix position matches.",
                "Tile matrix position differs.",
                actual,
                locked,
            );
        }
        for (name, tag, locked) in [
            ("x", tags::X_OFFSET_IN_SLIDE_COORDINATE_SYSTEM, x),
            ("y", tags::Y_OFFSET_IN_SLIDE_COORDINATE_SYSTEM, y),
            ("z", tags::Z_OFFSET_IN_SLIDE_COORDINATE_SYSTEM, 0.0),
        ] {
            check_equal(
                internal,
                &format!("wsi_sparse_frame_{}_{}_offset", index + 1, name),
                "Slide-coordinate offset matches geometry.",
                "Slide-coordinate offset differs from geometry.",
                item_f64(path, plane, tag)?,
                locked,
            );
        }
        check_equal(
            internal,
            &format!("wsi_sparse_frame_{}_optical_path_identifier", index + 1),
            "Frame references the RGB optical path.",
            "Frame references a different optical path.",
            nested_sequence_item_str(
                path,
                frame,
                tags::OPTICAL_PATH_IDENTIFICATION_SEQUENCE,
                0,
                tags::OPTICAL_PATH_IDENTIFIER,
            )?
            .as_str(),
            "RGB",
        );
        let optical =
            item_sequence_item(path, frame, tags::OPTICAL_PATH_IDENTIFICATION_SEQUENCE, 0)?;
        check(
            internal,
            optical.iter().count() == 1
                && optical
                    .iter()
                    .all(|element| element.tag() == tags::OPTICAL_PATH_IDENTIFIER),
            &format!("wsi_sparse_frame_{}_optical_path_attributes", index + 1),
            "Optical Path Identification contains exactly the locked identifier.",
            "Optical Path Identification contains an unexpected or missing attribute.",
        );
        positions.push((actual_column, actual_row));
    }
    Ok(positions)
}

fn reconstruct_tiled_sparse_matrix(
    pixel_bytes: &[u8],
    positions: &[(usize, usize)],
) -> Option<(Vec<u8>, [bool; 4])> {
    if pixel_bytes.len() != positions.len() * 12 || positions.len() != 2 {
        return None;
    }
    let mut matrix = vec![0_u8; 48];
    let mut occupancy = [false; 4];
    for (frame, &(column, row)) in positions.iter().enumerate() {
        if !matches!(column, 1 | 3) || !matches!(row, 1 | 3) {
            return None;
        }
        let tile_column = (column - 1) / 2;
        let tile_row = (row - 1) / 2;
        let occupancy_index = tile_row * 2 + tile_column;
        if occupancy[occupancy_index] {
            return None;
        }
        occupancy[occupancy_index] = true;
        for tile_pixel_row in 0..2 {
            let source = frame * 12 + tile_pixel_row * 6;
            let destination = ((tile_row * 2 + tile_pixel_row) * 4 + tile_column * 2) * 3;
            matrix[destination..destination + 6].copy_from_slice(&pixel_bytes[source..source + 6]);
        }
    }
    Some((matrix, occupancy))
}

fn valid_dicom_uid(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && !value.starts_with('.')
        && !value.ends_with('.')
        && value.split('.').all(|part| {
            !part.is_empty()
                && part.bytes().all(|byte| byte.is_ascii_digit())
                && (part.len() == 1 || !part.starts_with('0'))
        })
}

pub(crate) fn validate_presentation_state_file(
    path: &Path,
    expected: &PresentationStateExpectations<'_>,
) -> Result<ValidatedPart10, GenerateError> {
    let bytes = fs::read(path).map_err(|source| GenerateError::ReadGeneratedFile {
        path: path.to_path_buf(),
        source,
    })?;
    let obj = open_file(path).map_err(|err| GenerateError::ValidateDicomFile {
        path: path.to_path_buf(),
        message: err.to_string(),
    })?;

    let mut internal = Vec::new();
    check(
        &mut internal,
        bytes.len() >= 132 && &bytes[128..132] == b"DICM",
        "part10_preamble",
        "File has a 128-byte preamble followed by the DICM marker.",
        "File is missing the Part 10 DICM marker at byte offset 128.",
    );
    check_equal(
        &mut internal,
        "file_meta_transfer_syntax",
        "File Meta Information Transfer Syntax UID matches the recipe.",
        "File Meta Information Transfer Syntax UID does not match the recipe.",
        trim_uid(obj.meta().transfer_syntax()).as_str(),
        expected.transfer_syntax_uid,
    );

    let dataset_sop_class = element_str(path, &obj, tags::SOP_CLASS_UID)?;
    check_equal(
        &mut internal,
        "sop_class_uid_consistency",
        "Dataset SOP Class UID, File Meta SOP Class UID, and recipe SOP Class UID match.",
        "SOP Class UID differs between dataset, File Meta Information, or recipe.",
        dataset_sop_class.as_str(),
        expected.sop_class_uid,
    );
    check_equal(
        &mut internal,
        "media_storage_sop_class_uid",
        "File Meta SOP Class UID matches the dataset SOP Class UID.",
        "File Meta SOP Class UID does not match the dataset SOP Class UID.",
        trim_uid(obj.meta().media_storage_sop_class_uid()).as_str(),
        dataset_sop_class.as_str(),
    );
    let dataset_sop_instance = element_str(path, &obj, tags::SOP_INSTANCE_UID)?;
    check_equal(
        &mut internal,
        "sop_instance_uid_consistency",
        "Dataset SOP Instance UID, File Meta SOP Instance UID, and manifest UID match.",
        "SOP Instance UID differs between dataset, File Meta Information, or manifest.",
        dataset_sop_instance.as_str(),
        expected.sop_instance_uid,
    );
    check_equal(
        &mut internal,
        "media_storage_sop_instance_uid",
        "File Meta SOP Instance UID matches the dataset SOP Instance UID.",
        "File Meta SOP Instance UID does not match the dataset SOP Instance UID.",
        trim_uid(obj.meta().media_storage_sop_instance_uid()).as_str(),
        dataset_sop_instance.as_str(),
    );
    check_equal(
        &mut internal,
        "implementation_class_uid",
        "File Meta Implementation Class UID matches the deterministic generator UID.",
        "File Meta Implementation Class UID does not match the deterministic generator UID.",
        trim_uid(obj.meta().implementation_class_uid()).as_str(),
        expected.implementation_class_uid,
    );
    check_equal(
        &mut internal,
        "synthetic_data",
        "Synthetic Data is present and set to YES.",
        "Synthetic Data is missing or not set to YES.",
        element_str(path, &obj, tags::SYNTHETIC_DATA)?.as_str(),
        expected.synthetic_data,
    );
    check_equal(
        &mut internal,
        "presentation_state_modality",
        "Presentation Series Modality is PR.",
        "Presentation Series Modality does not match the recipe.",
        element_str(path, &obj, tags::MODALITY)?.as_str(),
        expected.modality,
    );
    check_equal(
        &mut internal,
        "presentation_state_content_label",
        "Presentation State Content Label matches the recipe.",
        "Presentation State Content Label does not match the recipe.",
        element_str(path, &obj, TAG_CONTENT_LABEL)?.as_str(),
        expected.presentation_label,
    );

    let referenced_series = top_level_sequence_item(path, &obj, TAG_REFERENCED_SERIES_SEQUENCE, 0)?;
    check_equal(
        &mut internal,
        "presentation_state_referenced_series_uid",
        "Presentation State references the source Series Instance UID.",
        "Presentation State referenced Series Instance UID does not match the source.",
        item_str(path, referenced_series, tags::SERIES_INSTANCE_UID)?.as_str(),
        expected.referenced_series_instance_uid,
    );
    let referenced_image =
        item_sequence_item(path, referenced_series, TAG_REFERENCED_IMAGE_SEQUENCE, 0)?;
    check_equal(
        &mut internal,
        "presentation_state_referenced_sop_class_uid",
        "Presentation State reference SOP Class UID matches the source image.",
        "Presentation State reference SOP Class UID does not match the source image.",
        item_str(path, referenced_image, TAG_REFERENCED_SOP_CLASS_UID)?.as_str(),
        expected.referenced_sop_class_uid,
    );
    check_equal(
        &mut internal,
        "presentation_state_referenced_sop_instance_uid",
        "Presentation State reference SOP Instance UID matches the source image.",
        "Presentation State reference SOP Instance UID does not match the source image.",
        item_str(path, referenced_image, TAG_REFERENCED_SOP_INSTANCE_UID)?.as_str(),
        expected.referenced_sop_instance_uid,
    );

    let displayed_area =
        top_level_sequence_item(path, &obj, TAG_DISPLAYED_AREA_SELECTION_SEQUENCE, 0)?;
    check_equal(
        &mut internal,
        "presentation_state_displayed_area_top_left",
        "Displayed Area top-left corner matches the recipe.",
        "Displayed Area top-left corner does not match the recipe.",
        item_i32_values(
            path,
            displayed_area,
            TAG_DISPLAYED_AREA_TOP_LEFT_HAND_CORNER,
        )?,
        expected.displayed_area_top_left.clone(),
    );
    check_equal(
        &mut internal,
        "presentation_state_displayed_area_bottom_right",
        "Displayed Area bottom-right corner matches the recipe.",
        "Displayed Area bottom-right corner does not match the recipe.",
        item_i32_values(
            path,
            displayed_area,
            TAG_DISPLAYED_AREA_BOTTOM_RIGHT_HAND_CORNER,
        )?,
        expected.displayed_area_bottom_right.clone(),
    );
    check_equal(
        &mut internal,
        "presentation_state_size_mode",
        "Presentation Size Mode matches the recipe.",
        "Presentation Size Mode does not match the recipe.",
        item_str(path, displayed_area, TAG_PRESENTATION_SIZE_MODE)?.as_str(),
        expected.presentation_size_mode,
    );
    check_equal(
        &mut internal,
        "presentation_state_pixel_aspect_ratio",
        "Presentation Pixel Aspect Ratio matches the recipe.",
        "Presentation Pixel Aspect Ratio does not match the recipe.",
        item_i32_values(path, displayed_area, TAG_PRESENTATION_PIXEL_ASPECT_RATIO)?,
        expected.presentation_pixel_aspect_ratio.clone(),
    );

    let voi_lut = top_level_sequence_item(path, &obj, TAG_SOFTCOPY_VOI_LUT_SEQUENCE, 0)?;
    check_equal(
        &mut internal,
        "presentation_state_window_center",
        "Softcopy VOI Window Center matches the recipe.",
        "Softcopy VOI Window Center does not match the recipe.",
        item_str(path, voi_lut, tags::WINDOW_CENTER)?.as_str(),
        expected.window_center,
    );
    check_equal(
        &mut internal,
        "presentation_state_window_width",
        "Softcopy VOI Window Width matches the recipe.",
        "Softcopy VOI Window Width does not match the recipe.",
        item_str(path, voi_lut, tags::WINDOW_WIDTH)?.as_str(),
        expected.window_width,
    );
    check_equal(
        &mut internal,
        "presentation_state_lut_shape",
        "Presentation LUT Shape matches the recipe.",
        "Presentation LUT Shape does not match the recipe.",
        element_str(path, &obj, TAG_PRESENTATION_LUT_SHAPE)?.as_str(),
        expected.presentation_lut_shape,
    );
    check(
        &mut internal,
        obj.element_opt(tags::PIXEL_DATA)
            .map_err(|err| validation_error(path, err))?
            .is_none(),
        "presentation_state_pixel_data_absent",
        "Presentation State contains no Pixel Data.",
        "Presentation State unexpectedly contains Pixel Data.",
    );

    fail_if_any_failed(path, &internal)?;

    Ok(ValidatedPart10 {
        bytes,
        validation: serde_json::json!({
            "status": "passed",
            "internal": internal,
            "standards": [
                {
                    "name": standard_sop_class_validation_name(expected.sop_class_uid),
                    "status": "passed",
                    "message": standard_sop_class_validation_message(expected.sop_class_uid)
                },
                {
                    "name": standard_transfer_syntax_validation_name(expected.transfer_syntax_uid),
                    "status": "passed",
                    "message": standard_transfer_syntax_validation_message(expected.transfer_syntax_uid)
                },
                {
                    "name": "synthetic_data_attribute",
                    "status": "passed",
                    "message": "Synthetic Data (0008,001C) is present with value YES."
                },
                {
                    "name": "grayscale_softcopy_presentation_state_modules",
                    "status": "passed",
                    "message": "GSPS relationship, displayed area, softcopy VOI, and presentation LUT attributes match the recipe."
                }
            ],
            "external": []
        }),
    })
}

pub(crate) fn validate_color_softcopy_presentation_state_file(
    path: &Path,
    expected: &ColorSoftcopyPresentationStateExpectations<'_>,
) -> Result<ValidatedPart10, GenerateError> {
    let bytes = fs::read(path).map_err(|source| GenerateError::ReadGeneratedFile {
        path: path.to_path_buf(),
        source,
    })?;
    let obj = open_file(path).map_err(|err| GenerateError::ValidateDicomFile {
        path: path.to_path_buf(),
        message: err.to_string(),
    })?;

    let mut internal = Vec::new();
    check(
        &mut internal,
        bytes.len() >= 132 && &bytes[128..132] == b"DICM",
        "color_softcopy_part10_preamble",
        "File has a 128-byte preamble followed by the DICM marker.",
        "File is missing the Part 10 DICM marker at byte offset 128.",
    );
    check_equal(
        &mut internal,
        "color_softcopy_file_meta_transfer_syntax",
        "File Meta Transfer Syntax UID is Explicit VR Little Endian.",
        "File Meta Transfer Syntax UID does not match the locked recipe.",
        trim_uid(obj.meta().transfer_syntax()),
        expected.transfer_syntax_uid.to_string(),
    );

    let dataset_sop_class = element_str(path, &obj, tags::SOP_CLASS_UID)?;
    check_equal(
        &mut internal,
        "color_softcopy_sop_class_uid",
        "Dataset SOP Class UID matches Color Softcopy Presentation State Storage.",
        "Dataset SOP Class UID does not match Color Softcopy Presentation State Storage.",
        dataset_sop_class.as_str(),
        expected.sop_class_uid,
    );
    check_equal(
        &mut internal,
        "color_softcopy_media_storage_sop_class_uid",
        "File Meta SOP Class UID matches the dataset SOP Class UID.",
        "File Meta SOP Class UID does not match the dataset SOP Class UID.",
        trim_uid(obj.meta().media_storage_sop_class_uid()),
        dataset_sop_class,
    );
    let dataset_sop_instance = element_str(path, &obj, tags::SOP_INSTANCE_UID)?;
    check_equal(
        &mut internal,
        "color_softcopy_sop_instance_uid",
        "Dataset SOP Instance UID matches the deterministic manifest identity.",
        "Dataset SOP Instance UID does not match the deterministic manifest identity.",
        dataset_sop_instance.as_str(),
        expected.sop_instance_uid,
    );
    check_equal(
        &mut internal,
        "color_softcopy_media_storage_sop_instance_uid",
        "File Meta SOP Instance UID matches the dataset SOP Instance UID.",
        "File Meta SOP Instance UID does not match the dataset SOP Instance UID.",
        trim_uid(obj.meta().media_storage_sop_instance_uid()),
        dataset_sop_instance,
    );
    check_equal(
        &mut internal,
        "color_softcopy_implementation_class_uid",
        "File Meta Implementation Class UID matches the deterministic generator UID.",
        "File Meta Implementation Class UID does not match the deterministic generator UID.",
        trim_uid(obj.meta().implementation_class_uid()).as_str(),
        expected.implementation_class_uid,
    );

    for (name, tag, actual_expected, passed, failed) in [
        (
            "color_softcopy_synthetic_data",
            tags::SYNTHETIC_DATA,
            expected.synthetic_data,
            "Synthetic Data is YES.",
            "Synthetic Data does not match the locked recipe.",
        ),
        (
            "color_softcopy_patient_name",
            tags::PATIENT_NAME,
            "DICOMTEST^SMOKE",
            "Patient Name matches the synthetic identity.",
            "Patient Name does not match the synthetic identity.",
        ),
        (
            "color_softcopy_patient_id",
            tags::PATIENT_ID,
            "DICOMTEST-SMOKE-001",
            "Patient ID matches the synthetic identity.",
            "Patient ID does not match the synthetic identity.",
        ),
        (
            "color_softcopy_patient_birth_date",
            tags::PATIENT_BIRTH_DATE,
            "19700101",
            "Patient Birth Date matches the synthetic identity.",
            "Patient Birth Date does not match the synthetic identity.",
        ),
        (
            "color_softcopy_patient_sex",
            tags::PATIENT_SEX,
            "O",
            "Patient Sex matches the synthetic identity.",
            "Patient Sex does not match the synthetic identity.",
        ),
        (
            "color_softcopy_study_date",
            tags::STUDY_DATE,
            "20260101",
            "Study Date matches the locked recipe.",
            "Study Date does not match the locked recipe.",
        ),
        (
            "color_softcopy_study_time",
            tags::STUDY_TIME,
            "000000",
            "Study Time matches the locked recipe.",
            "Study Time does not match the locked recipe.",
        ),
        (
            "color_softcopy_referring_physician_name",
            tags::REFERRING_PHYSICIAN_NAME,
            "",
            "Referring Physician Name is present with its locked empty value.",
            "Referring Physician Name does not match the locked recipe.",
        ),
        (
            "color_softcopy_study_id",
            tags::STUDY_ID,
            "SMOKE",
            "Study ID matches the locked recipe.",
            "Study ID does not match the locked recipe.",
        ),
        (
            "color_softcopy_accession_number",
            tags::ACCESSION_NUMBER,
            "",
            "Accession Number is present with its locked empty value.",
            "Accession Number does not match the locked recipe.",
        ),
        (
            "color_softcopy_modality",
            tags::MODALITY,
            "PR",
            "Presentation Series Modality is PR.",
            "Presentation Series Modality does not match the locked recipe.",
        ),
        (
            "color_softcopy_series_number",
            tags::SERIES_NUMBER,
            "62",
            "Series Number matches the locked recipe.",
            "Series Number does not match the locked recipe.",
        ),
        (
            "color_softcopy_body_part_examined",
            tags::BODY_PART_EXAMINED,
            "HAND",
            "Body Part Examined is HAND.",
            "Body Part Examined does not match the locked recipe.",
        ),
        (
            "color_softcopy_laterality",
            tags::LATERALITY,
            "R",
            "Laterality is R.",
            "Laterality does not match the locked recipe.",
        ),
        (
            "color_softcopy_manufacturer",
            tags::MANUFACTURER,
            "dicom-test-suite",
            "Manufacturer matches the locked equipment identity.",
            "Manufacturer does not match the locked equipment identity.",
        ),
        (
            "color_softcopy_manufacturer_model_name",
            tags::MANUFACTURER_MODEL_NAME,
            "Native Color Softcopy Presentation State",
            "Manufacturer Model Name matches the locked equipment identity.",
            "Manufacturer Model Name does not match the locked equipment identity.",
        ),
        (
            "color_softcopy_device_serial_number",
            tags::DEVICE_SERIAL_NUMBER,
            "DTS-COLOR-PR-0001",
            "Device Serial Number matches the locked equipment identity.",
            "Device Serial Number does not match the locked equipment identity.",
        ),
        (
            "color_softcopy_software_versions",
            tags::SOFTWARE_VERSIONS,
            crate::PACKAGE_VERSION,
            "Software Versions matches the running generator version.",
            "Software Versions does not match the running generator version.",
        ),
        (
            "color_softcopy_instance_number",
            tags::INSTANCE_NUMBER,
            "1",
            "Instance Number matches the locked recipe.",
            "Instance Number does not match the locked recipe.",
        ),
        (
            "color_softcopy_presentation_creation_date",
            tags::PRESENTATION_CREATION_DATE,
            "20260101",
            "Presentation Creation Date matches the locked recipe.",
            "Presentation Creation Date does not match the locked recipe.",
        ),
        (
            "color_softcopy_presentation_creation_time",
            tags::PRESENTATION_CREATION_TIME,
            "000000",
            "Presentation Creation Time matches the locked recipe.",
            "Presentation Creation Time does not match the locked recipe.",
        ),
        (
            "color_softcopy_content_label",
            tags::CONTENT_LABEL,
            "DTSCOLORPR",
            "Content Label matches the locked recipe.",
            "Content Label does not match the locked recipe.",
        ),
        (
            "color_softcopy_content_description",
            tags::CONTENT_DESCRIPTION,
            "Synthetic RGB color presentation state",
            "Content Description matches the locked recipe.",
            "Content Description does not match the locked recipe.",
        ),
        (
            "color_softcopy_content_creator_name",
            tags::CONTENT_CREATOR_NAME,
            "DTS^Generator",
            "Content Creator Name matches the locked recipe.",
            "Content Creator Name does not match the locked recipe.",
        ),
    ] {
        check_equal(
            &mut internal,
            name,
            passed,
            failed,
            element_str(path, &obj, tag)?.as_str(),
            actual_expected,
        );
    }

    let study_instance_uid = element_str(path, &obj, tags::STUDY_INSTANCE_UID)?;
    check_equal(
        &mut internal,
        "color_softcopy_study_instance_uid",
        "Presentation State Study Instance UID matches the source study.",
        "Presentation State Study Instance UID does not match the source study.",
        study_instance_uid.as_str(),
        expected.study_instance_uid,
    );
    check_equal(
        &mut internal,
        "color_softcopy_same_study",
        "Presentation State and source image share one Study Instance UID.",
        "Presentation State and source image do not share one Study Instance UID.",
        study_instance_uid.as_str(),
        expected.source_study_instance_uid,
    );
    let series_instance_uid = element_str(path, &obj, tags::SERIES_INSTANCE_UID)?;
    check_equal(
        &mut internal,
        "color_softcopy_series_instance_uid",
        "Presentation State Series Instance UID matches the deterministic identity.",
        "Presentation State Series Instance UID does not match the deterministic identity.",
        series_instance_uid.as_str(),
        expected.series_instance_uid,
    );
    check(
        &mut internal,
        series_instance_uid != expected.source_series_instance_uid,
        "color_softcopy_different_series",
        "Presentation State and source image use distinct Series Instance UIDs.",
        "Presentation State and source image unexpectedly share a Series Instance UID.",
    );

    check_equal(
        &mut internal,
        "color_softcopy_referenced_series_items",
        "Referenced Series Sequence contains exactly one item.",
        "Referenced Series Sequence does not contain exactly one item.",
        sequence_item_count(path, &obj, tags::REFERENCED_SERIES_SEQUENCE)?,
        1,
    );
    let referenced_series =
        top_level_sequence_item(path, &obj, tags::REFERENCED_SERIES_SEQUENCE, 0)?;
    check_equal(
        &mut internal,
        "color_softcopy_referenced_series_uid",
        "Referenced Series Sequence identifies the source series.",
        "Referenced Series Sequence does not identify the source series.",
        item_str(path, referenced_series, tags::SERIES_INSTANCE_UID)?.as_str(),
        expected.source_series_instance_uid,
    );
    check_equal(
        &mut internal,
        "color_softcopy_referenced_image_items",
        "Referenced Image Sequence contains exactly one item.",
        "Referenced Image Sequence does not contain exactly one item.",
        item_sequence_item_count(path, referenced_series, tags::REFERENCED_IMAGE_SEQUENCE)?,
        1,
    );
    let referenced_image =
        item_sequence_item(path, referenced_series, tags::REFERENCED_IMAGE_SEQUENCE, 0)?;
    check_equal(
        &mut internal,
        "color_softcopy_referenced_sop_class_uid",
        "Referenced SOP Class UID matches the source Secondary Capture image.",
        "Referenced SOP Class UID does not match the source image.",
        item_str(path, referenced_image, tags::REFERENCED_SOP_CLASS_UID)?.as_str(),
        expected.source_sop_class_uid,
    );
    check_equal(
        &mut internal,
        "color_softcopy_referenced_sop_instance_uid",
        "Referenced SOP Instance UID matches the source image.",
        "Referenced SOP Instance UID does not match the source image.",
        item_str(path, referenced_image, tags::REFERENCED_SOP_INSTANCE_UID)?.as_str(),
        expected.source_sop_instance_uid,
    );
    check(
        &mut internal,
        referenced_image
            .element_opt(tags::REFERENCED_FRAME_NUMBER)
            .map_err(|err| validation_error(path, err))?
            .is_none(),
        "color_softcopy_referenced_frame_numbers_absent",
        "The reference applies to the complete source instance.",
        "Referenced Frame Number unexpectedly narrows the source reference.",
    );

    check_equal(
        &mut internal,
        "color_softcopy_displayed_area_items",
        "Displayed Area Selection Sequence contains exactly one item.",
        "Displayed Area Selection Sequence does not contain exactly one item.",
        sequence_item_count(path, &obj, tags::DISPLAYED_AREA_SELECTION_SEQUENCE)?,
        1,
    );
    let displayed_area =
        top_level_sequence_item(path, &obj, tags::DISPLAYED_AREA_SELECTION_SEQUENCE, 0)?;
    check(
        &mut internal,
        displayed_area
            .element_opt(tags::REFERENCED_IMAGE_SEQUENCE)
            .map_err(|err| validation_error(path, err))?
            .is_none()
            && displayed_area
                .element_opt(tags::REFERENCED_FRAME_NUMBER)
                .map_err(|err| validation_error(path, err))?
                .is_none(),
        "color_softcopy_displayed_area_global",
        "Displayed Area applies globally to all referenced images.",
        "Displayed Area unexpectedly narrows its applicability.",
    );
    check_equal(
        &mut internal,
        "color_softcopy_displayed_area_top_left_vr",
        "Displayed Area top-left corner uses VR SL.",
        "Displayed Area top-left corner does not use VR SL.",
        displayed_area
            .element(tags::DISPLAYED_AREA_TOP_LEFT_HAND_CORNER)
            .map_err(|err| validation_error(path, err))?
            .vr(),
        VR::SL,
    );
    check_equal(
        &mut internal,
        "color_softcopy_displayed_area_top_left",
        "Displayed Area top-left corner is [1, 1].",
        "Displayed Area top-left corner does not match the 2x2 source geometry.",
        item_i32_values(
            path,
            displayed_area,
            tags::DISPLAYED_AREA_TOP_LEFT_HAND_CORNER,
        )?,
        vec![1, 1],
    );
    check_equal(
        &mut internal,
        "color_softcopy_displayed_area_bottom_right_vr",
        "Displayed Area bottom-right corner uses VR SL.",
        "Displayed Area bottom-right corner does not use VR SL.",
        displayed_area
            .element(tags::DISPLAYED_AREA_BOTTOM_RIGHT_HAND_CORNER)
            .map_err(|err| validation_error(path, err))?
            .vr(),
        VR::SL,
    );
    check_equal(
        &mut internal,
        "color_softcopy_displayed_area_bottom_right",
        "Displayed Area bottom-right corner is [2, 2].",
        "Displayed Area bottom-right corner does not match the 2x2 source geometry.",
        item_i32_values(
            path,
            displayed_area,
            tags::DISPLAYED_AREA_BOTTOM_RIGHT_HAND_CORNER,
        )?,
        vec![2, 2],
    );
    check_equal(
        &mut internal,
        "color_softcopy_presentation_size_mode",
        "Presentation Size Mode is SCALE TO FIT.",
        "Presentation Size Mode does not match the locked recipe.",
        item_str(path, displayed_area, tags::PRESENTATION_SIZE_MODE)?.as_str(),
        "SCALE TO FIT",
    );
    check_equal(
        &mut internal,
        "color_softcopy_presentation_pixel_aspect_ratio_vr",
        "Presentation Pixel Aspect Ratio uses VR IS.",
        "Presentation Pixel Aspect Ratio does not use VR IS.",
        displayed_area
            .element(tags::PRESENTATION_PIXEL_ASPECT_RATIO)
            .map_err(|err| validation_error(path, err))?
            .vr(),
        VR::IS,
    );
    check_equal(
        &mut internal,
        "color_softcopy_presentation_pixel_aspect_ratio",
        "Presentation Pixel Aspect Ratio is [1, 1].",
        "Presentation Pixel Aspect Ratio does not match the locked recipe.",
        item_i32_values(path, displayed_area, tags::PRESENTATION_PIXEL_ASPECT_RATIO)?,
        vec![1, 1],
    );
    for (name, tag) in [
        (
            "color_softcopy_presentation_pixel_spacing_absent",
            tags::PRESENTATION_PIXEL_SPACING,
        ),
        (
            "color_softcopy_presentation_pixel_magnification_ratio_absent",
            tags::PRESENTATION_PIXEL_MAGNIFICATION_RATIO,
        ),
    ] {
        check(
            &mut internal,
            displayed_area
                .element_opt(tag)
                .map_err(|err| validation_error(path, err))?
                .is_none(),
            name,
            "Mutually exclusive Displayed Area sizing attribute is absent.",
            "Displayed Area contains an unexpected mutually exclusive sizing attribute.",
        );
    }

    let icc_element = obj
        .element(tags::ICC_PROFILE)
        .map_err(|err| validation_error(path, err))?;
    check_equal(
        &mut internal,
        "color_softcopy_icc_profile_vr",
        "ICC Profile uses VR OB.",
        "ICC Profile does not use VR OB.",
        icc_element.vr(),
        VR::OB,
    );
    let icc_bytes = icc_element
        .value()
        .to_bytes()
        .map_err(|err| validation_error(path, err))?;
    let icc_profile = icc_bytes.as_ref();
    check_equal(
        &mut internal,
        "color_softcopy_icc_profile_size",
        "ICC Profile contains exactly 736 bytes.",
        "ICC Profile length does not match the locked sRGB profile.",
        icc_profile.len(),
        736,
    );
    check_equal(
        &mut internal,
        "color_softcopy_icc_profile_sha256",
        "ICC Profile SHA-256 matches the locked sRGB profile.",
        "ICC Profile bytes do not match the locked sRGB profile.",
        sha256_hex(icc_profile).as_str(),
        expected.icc_profile_sha256,
    );
    check_equal(
        &mut internal,
        "color_softcopy_icc_declared_size",
        "ICC header declares the exact profile size.",
        "ICC header does not declare the exact profile size.",
        icc_profile
            .get(0..4)
            .and_then(|value| <[u8; 4]>::try_from(value).ok())
            .map(u32::from_be_bytes),
        Some(736),
    );
    for (name, range, locked, passed, failed) in [
        (
            "color_softcopy_icc_device_class",
            12..16,
            &b"scnr"[..],
            "ICC device class is scnr.",
            "ICC device class does not match the locked profile.",
        ),
        (
            "color_softcopy_icc_data_color_space",
            16..20,
            &b"RGB "[..],
            "ICC data color space is RGB.",
            "ICC data color space does not match the locked profile.",
        ),
        (
            "color_softcopy_icc_profile_connection_space",
            20..24,
            &b"XYZ "[..],
            "ICC profile connection space is XYZ.",
            "ICC profile connection space does not match the locked profile.",
        ),
        (
            "color_softcopy_icc_signature",
            36..40,
            &b"acsp"[..],
            "ICC signature is acsp.",
            "ICC signature does not match the locked profile.",
        ),
    ] {
        check_equal(
            &mut internal,
            name,
            passed,
            failed,
            icc_profile.get(range),
            Some(locked),
        );
    }
    let color_space = obj
        .element(tags::COLOR_SPACE)
        .map_err(|err| validation_error(path, err))?;
    check_equal(
        &mut internal,
        "color_softcopy_color_space_vr",
        "DICOM Color Space uses VR CS.",
        "DICOM Color Space does not use VR CS.",
        color_space.vr(),
        VR::CS,
    );
    check_equal(
        &mut internal,
        "color_softcopy_color_space",
        "DICOM Color Space is SRGB.",
        "DICOM Color Space does not match the locked profile.",
        color_space
            .to_str()
            .map_err(|err| validation_error(path, err))?
            .trim_end_matches(['\0', ' ']),
        "SRGB",
    );

    for (name, tag, passed, failed) in [
        (
            "color_softcopy_content_date_absent",
            tags::CONTENT_DATE,
            "Content Date is absent from the Color Softcopy IOD.",
            "Content Date is unexpectedly present in the Color Softcopy IOD.",
        ),
        (
            "color_softcopy_content_time_absent",
            tags::CONTENT_TIME,
            "Content Time is absent from the Color Softcopy IOD.",
            "Content Time is unexpectedly present in the Color Softcopy IOD.",
        ),
        (
            "color_softcopy_shutter_shape_absent",
            tags::SHUTTER_SHAPE,
            "Shutter Shape is absent.",
            "Shutter Shape is unexpectedly present.",
        ),
        (
            "color_softcopy_shutter_left_edge_absent",
            tags::SHUTTER_LEFT_VERTICAL_EDGE,
            "Shutter left edge is absent.",
            "Shutter left edge is unexpectedly present.",
        ),
        (
            "color_softcopy_shutter_right_edge_absent",
            tags::SHUTTER_RIGHT_VERTICAL_EDGE,
            "Shutter right edge is absent.",
            "Shutter right edge is unexpectedly present.",
        ),
        (
            "color_softcopy_shutter_upper_edge_absent",
            tags::SHUTTER_UPPER_HORIZONTAL_EDGE,
            "Shutter upper edge is absent.",
            "Shutter upper edge is unexpectedly present.",
        ),
        (
            "color_softcopy_shutter_lower_edge_absent",
            tags::SHUTTER_LOWER_HORIZONTAL_EDGE,
            "Shutter lower edge is absent.",
            "Shutter lower edge is unexpectedly present.",
        ),
        (
            "color_softcopy_circular_shutter_center_absent",
            tags::CENTER_OF_CIRCULAR_SHUTTER,
            "Circular shutter center is absent.",
            "Circular shutter center is unexpectedly present.",
        ),
        (
            "color_softcopy_circular_shutter_radius_absent",
            tags::RADIUS_OF_CIRCULAR_SHUTTER,
            "Circular shutter radius is absent.",
            "Circular shutter radius is unexpectedly present.",
        ),
        (
            "color_softcopy_polygonal_shutter_vertices_absent",
            tags::VERTICES_OF_THE_POLYGONAL_SHUTTER,
            "Polygonal shutter vertices are absent.",
            "Polygonal shutter vertices are unexpectedly present.",
        ),
        (
            "color_softcopy_shutter_presentation_value_absent",
            tags::SHUTTER_PRESENTATION_VALUE,
            "Shutter Presentation Value is absent.",
            "Shutter Presentation Value is unexpectedly present.",
        ),
        (
            "color_softcopy_shutter_presentation_color_absent",
            tags::SHUTTER_PRESENTATION_COLOR_CIE_LAB_VALUE,
            "Shutter Presentation Color is absent.",
            "Shutter Presentation Color is unexpectedly present.",
        ),
        (
            "color_softcopy_graphic_annotation_sequence_absent",
            tags::GRAPHIC_ANNOTATION_SEQUENCE,
            "Graphic Annotation Sequence is absent.",
            "Graphic Annotation Sequence is unexpectedly present.",
        ),
        (
            "color_softcopy_graphic_layer_sequence_absent",
            tags::GRAPHIC_LAYER_SEQUENCE,
            "Graphic Layer Sequence is absent.",
            "Graphic Layer Sequence is unexpectedly present.",
        ),
        (
            "color_softcopy_image_horizontal_flip_absent",
            tags::IMAGE_HORIZONTAL_FLIP,
            "Image Horizontal Flip is absent.",
            "Image Horizontal Flip is unexpectedly present.",
        ),
        (
            "color_softcopy_image_rotation_absent",
            tags::IMAGE_ROTATION,
            "Image Rotation is absent.",
            "Image Rotation is unexpectedly present.",
        ),
        (
            "color_softcopy_pixel_data_absent",
            tags::PIXEL_DATA,
            "Color Softcopy Presentation State contains no Pixel Data.",
            "Color Softcopy Presentation State unexpectedly contains Pixel Data.",
        ),
    ] {
        check(
            &mut internal,
            obj.element_opt(tag)
                .map_err(|err| validation_error(path, err))?
                .is_none(),
            name,
            passed,
            failed,
        );
    }
    check(
        &mut internal,
        !obj.tags()
            .any(|tag| (0x6000..=0x60FE).contains(&tag.0) && tag.0 % 2 == 0),
        "color_softcopy_overlay_items_absent",
        "No overlay-group attributes are present.",
        "An overlay-group attribute is unexpectedly present.",
    );

    fail_if_any_failed(path, &internal)?;

    Ok(ValidatedPart10 {
        bytes,
        validation: serde_json::json!({
            "status": "passed",
            "internal": internal,
            "standards": [
                {
                    "name": standard_sop_class_validation_name(expected.sop_class_uid),
                    "status": "passed",
                    "message": standard_sop_class_validation_message(expected.sop_class_uid)
                },
                {
                    "name": standard_transfer_syntax_validation_name(expected.transfer_syntax_uid),
                    "status": "passed",
                    "message": standard_transfer_syntax_validation_message(expected.transfer_syntax_uid)
                },
                {
                    "name": "color_softcopy_presentation_state_modules",
                    "status": "passed",
                    "message": "Color Softcopy relationship, global displayed area, locked ICC profile, and prohibited-content invariants match the recipe."
                }
            ],
            "external": []
        }),
    })
}

pub(crate) fn validate_blending_presentation_state_file(
    path: &Path,
    expected: &BlendingPresentationStateExpectations<'_>,
) -> Result<ValidatedPart10, GenerateError> {
    let bytes = fs::read(path).map_err(|source| GenerateError::ReadGeneratedFile {
        path: path.to_path_buf(),
        source,
    })?;
    let obj = open_file(path).map_err(|err| GenerateError::ValidateDicomFile {
        path: path.to_path_buf(),
        message: err.to_string(),
    })?;
    let mut internal = Vec::new();

    check(
        &mut internal,
        bytes.len() >= 132 && &bytes[128..132] == b"DICM",
        "blending_part10_preamble",
        "File has a Part 10 preamble and DICM marker.",
        "File is missing the Part 10 DICM marker.",
    );
    check_equal(
        &mut internal,
        "blending_transfer_syntax",
        "Transfer Syntax matches the locked recipe.",
        "Transfer Syntax does not match the locked recipe.",
        trim_uid(obj.meta().transfer_syntax()),
        expected.transfer_syntax_uid.to_string(),
    );
    let sop_class_uid = element_str(path, &obj, tags::SOP_CLASS_UID)?;
    let sop_instance_uid = element_str(path, &obj, tags::SOP_INSTANCE_UID)?;
    for (name, actual, locked) in [
        (
            "blending_sop_class_uid",
            sop_class_uid.as_str(),
            expected.sop_class_uid,
        ),
        (
            "blending_sop_instance_uid",
            sop_instance_uid.as_str(),
            expected.sop_instance_uid,
        ),
        (
            "blending_synthetic_data",
            element_str(path, &obj, tags::SYNTHETIC_DATA)?.as_str(),
            expected.synthetic_data,
        ),
        (
            "blending_study_instance_uid",
            element_str(path, &obj, tags::STUDY_INSTANCE_UID)?.as_str(),
            expected.study_instance_uid,
        ),
        (
            "blending_series_instance_uid",
            element_str(path, &obj, tags::SERIES_INSTANCE_UID)?.as_str(),
            expected.series_instance_uid,
        ),
    ] {
        check_equal(
            &mut internal,
            name,
            "Identity matches the locked recipe.",
            "Identity does not match the locked recipe.",
            actual,
            locked,
        );
    }
    for (name, actual, locked) in [
        (
            "blending_media_storage_sop_class_uid",
            trim_uid(obj.meta().media_storage_sop_class_uid()),
            sop_class_uid.clone(),
        ),
        (
            "blending_media_storage_sop_instance_uid",
            trim_uid(obj.meta().media_storage_sop_instance_uid()),
            sop_instance_uid.clone(),
        ),
        (
            "blending_implementation_class_uid",
            trim_uid(obj.meta().implementation_class_uid()),
            expected.implementation_class_uid.to_string(),
        ),
    ] {
        check_equal(
            &mut internal,
            name,
            "File Meta identity matches the dataset and generator.",
            "File Meta identity does not match the dataset or generator.",
            actual,
            locked,
        );
    }
    for (name, tag, locked) in [
        (
            "patient_name",
            tags::PATIENT_NAME,
            "DTS^Synthetic^Patient001",
        ),
        ("patient_id", tags::PATIENT_ID, "DTS-PATIENT-001"),
        ("patient_birth_date", tags::PATIENT_BIRTH_DATE, "19700101"),
        ("patient_sex", tags::PATIENT_SEX, "O"),
        ("study_date", tags::STUDY_DATE, "20260101"),
        ("study_time", tags::STUDY_TIME, "000000"),
        ("referring_physician", tags::REFERRING_PHYSICIAN_NAME, ""),
        ("study_id", tags::STUDY_ID, "DTS-CT"),
        ("accession_number", tags::ACCESSION_NUMBER, ""),
        ("modality", tags::MODALITY, "PR"),
        ("series_number", tags::SERIES_NUMBER, "81"),
        ("laterality", tags::LATERALITY, "R"),
        ("manufacturer", tags::MANUFACTURER, "dicom-test-suite"),
        ("institution_name", tags::INSTITUTION_NAME, ""),
        ("institution_address", tags::INSTITUTION_ADDRESS, ""),
        (
            "model_name",
            tags::MANUFACTURER_MODEL_NAME,
            "Native Blending Softcopy Presentation State",
        ),
        ("device_serial", tags::DEVICE_SERIAL_NUMBER, "DTS-BLEND-001"),
        (
            "software_versions",
            tags::SOFTWARE_VERSIONS,
            crate::PACKAGE_VERSION,
        ),
        ("instance_number", tags::INSTANCE_NUMBER, "1"),
        (
            "creation_date",
            tags::PRESENTATION_CREATION_DATE,
            "20260101",
        ),
        ("creation_time", tags::PRESENTATION_CREATION_TIME, "000000"),
        ("content_label", tags::CONTENT_LABEL, "DTSBLEND"),
        (
            "content_description",
            tags::CONTENT_DESCRIPTION,
            "Synthetic DTSBLEND presentation state",
        ),
        (
            "content_creator",
            tags::CONTENT_CREATOR_NAME,
            "DTS^Generator",
        ),
    ] {
        check_equal(
            &mut internal,
            &format!("blending_{name}"),
            "Attribute matches the locked recipe.",
            "Attribute does not match the locked recipe.",
            element_str(path, &obj, tag)?.as_str(),
            locked,
        );
    }
    check(
        &mut internal,
        expected
            .source_series
            .iter()
            .all(|source| source.series_instance_uid != expected.series_instance_uid),
        "blending_distinct_presentation_series",
        "Presentation Series is distinct from source Series.",
        "Presentation Series reuses a source Series UID.",
    );

    check_equal(
        &mut internal,
        "blending_item_count",
        "Blending Sequence has exactly two items.",
        "Blending Sequence does not have exactly two items.",
        sequence_item_count(path, &obj, tags::BLENDING_SEQUENCE)?,
        2,
    );
    for (index, source) in expected.source_series.iter().enumerate() {
        let ordinal = index + 1;
        let item = top_level_sequence_item(path, &obj, tags::BLENDING_SEQUENCE, index)?;
        check_equal(
            &mut internal,
            &format!("blending_item_{ordinal}_position"),
            "Blending Position matches locked order and is unique.",
            "Blending Position is missing, duplicated, or reordered.",
            item_str(path, item, tags::BLENDING_POSITION)?.as_str(),
            if index == 0 {
                "UNDERLYING"
            } else {
                "SUPERIMPOSED"
            },
        );
        check_equal(
            &mut internal,
            &format!("blending_item_{ordinal}_study"),
            "Blending item references the source Study.",
            "Blending item redirects the source Study.",
            item_str(path, item, tags::STUDY_INSTANCE_UID)?.as_str(),
            expected.study_instance_uid,
        );
        check_equal(
            &mut internal,
            &format!("blending_item_{ordinal}_series_count"),
            "Blending item has one source Series.",
            "Blending item has an invalid source Series cardinality.",
            item_sequence_item_count(path, item, tags::REFERENCED_SERIES_SEQUENCE)?,
            1,
        );
        let series = item_sequence_item(path, item, tags::REFERENCED_SERIES_SEQUENCE, 0)?;
        check_equal(
            &mut internal,
            &format!("blending_item_{ordinal}_series"),
            "Blending item references the ordered source Series.",
            "Blending item redirects or reorders the source Series.",
            item_str(path, series, tags::SERIES_INSTANCE_UID)?.as_str(),
            source.series_instance_uid,
        );
        check_equal(
            &mut internal,
            &format!("blending_item_{ordinal}_image_count"),
            "Blending item references exactly two source Images.",
            "Blending item omits, duplicates, or adds a source Image.",
            item_sequence_item_count(path, series, tags::REFERENCED_IMAGE_SEQUENCE)?,
            2,
        );
        for (image_index, expected_uid) in source.sop_instance_uids.iter().enumerate() {
            let image =
                item_sequence_item(path, series, tags::REFERENCED_IMAGE_SEQUENCE, image_index)?;
            check_equal(
                &mut internal,
                &format!(
                    "blending_item_{ordinal}_image_{}_sop_class",
                    image_index + 1
                ),
                "Source SOP Class matches CT.",
                "Source SOP Class is redirected.",
                item_str(path, image, tags::REFERENCED_SOP_CLASS_UID)?.as_str(),
                source.sop_class_uid,
            );
            check_equal(
                &mut internal,
                &format!(
                    "blending_item_{ordinal}_image_{}_sop_instance",
                    image_index + 1
                ),
                "Source SOP Instance matches locked order.",
                "Source SOP Instance is redirected, duplicated, or reordered.",
                item_str(path, image, tags::REFERENCED_SOP_INSTANCE_UID)?.as_str(),
                *expected_uid,
            );
            check(
                &mut internal,
                image
                    .element_opt(tags::REFERENCED_FRAME_NUMBER)
                    .map_err(|err| validation_error(path, err))?
                    .is_none(),
                &format!(
                    "blending_item_{ordinal}_image_{}_complete_instance",
                    image_index + 1
                ),
                "Reference selects the complete Instance.",
                "Referenced Frame Number unexpectedly narrows the Instance.",
            );
        }
        for (name, tag, locked) in [
            ("rescale_intercept", tags::RESCALE_INTERCEPT, "-1024"),
            ("rescale_slope", tags::RESCALE_SLOPE, "1"),
            ("rescale_type", tags::RESCALE_TYPE, "HU"),
        ] {
            check_equal(
                &mut internal,
                &format!("blending_item_{ordinal}_{name}"),
                "Modality LUT transform matches the CT recipe.",
                "Modality LUT transform does not match the CT recipe.",
                item_str(path, item, tag)?.as_str(),
                locked,
            );
        }
        for (name, tag) in [
            ("softcopy_voi", tags::SOFTCOPY_VOILUT_SEQUENCE),
            (
                "spatial_registration",
                tags::REFERENCED_SPATIAL_REGISTRATION_SEQUENCE,
            ),
        ] {
            check(
                &mut internal,
                item.element_opt(tag)
                    .map_err(|err| validation_error(path, err))?
                    .is_none(),
                &format!("blending_item_{ordinal}_{name}_absent"),
                "Optional transform is absent.",
                "Forbidden optional transform is present.",
            );
        }
    }
    let opacity_element = obj
        .element(tags::RELATIVE_OPACITY)
        .map_err(|err| validation_error(path, err))?;
    check_equal(
        &mut internal,
        "blending_opacity_vr",
        "Relative Opacity uses VR FL.",
        "Relative Opacity does not use VR FL.",
        opacity_element.vr(),
        VR::FL,
    );
    check_equal(
        &mut internal,
        "blending_opacity_vm",
        "Relative Opacity has VM 1.",
        "Relative Opacity does not have VM 1.",
        opacity_element.value().multiplicity(),
        1,
    );
    let opacity = opacity_element
        .value()
        .to_float32()
        .map_err(|err| validation_error(path, err))?;
    check(
        &mut internal,
        opacity.is_finite() && (0.0..=1.0).contains(&opacity),
        "blending_opacity_range",
        "Relative Opacity is finite and in range.",
        "Relative Opacity is non-finite or out of range.",
    );
    check_equal(
        &mut internal,
        "blending_opacity",
        "Relative Opacity is exactly 0.5.",
        "Relative Opacity does not match the locked recipe.",
        opacity.to_bits(),
        0.5_f32.to_bits(),
    );

    check_equal(
        &mut internal,
        "blending_displayed_area_count",
        "Exactly one global displayed area is present.",
        "Displayed Area cardinality is invalid.",
        sequence_item_count(path, &obj, tags::DISPLAYED_AREA_SELECTION_SEQUENCE)?,
        1,
    );
    let area = top_level_sequence_item(path, &obj, tags::DISPLAYED_AREA_SELECTION_SEQUENCE, 0)?;
    check(
        &mut internal,
        area.element_opt(tags::REFERENCED_IMAGE_SEQUENCE)
            .map_err(|err| validation_error(path, err))?
            .is_none(),
        "blending_displayed_area_global",
        "Displayed area applies globally.",
        "Displayed area unexpectedly selects referenced Images.",
    );
    check_equal(
        &mut internal,
        "blending_displayed_area_top_left",
        "Top-left corner matches.",
        "Top-left corner does not match.",
        item_i32_values(path, area, tags::DISPLAYED_AREA_TOP_LEFT_HAND_CORNER)?,
        vec![1, 1],
    );
    check_equal(
        &mut internal,
        "blending_displayed_area_bottom_right",
        "Bottom-right corner matches.",
        "Bottom-right corner does not match.",
        item_i32_values(path, area, tags::DISPLAYED_AREA_BOTTOM_RIGHT_HAND_CORNER)?,
        vec![2, 2],
    );
    check_equal(
        &mut internal,
        "blending_displayed_area_mode",
        "Presentation Size Mode matches.",
        "Presentation Size Mode does not match.",
        item_str(path, area, tags::PRESENTATION_SIZE_MODE)?.as_str(),
        "SCALE TO FIT",
    );
    check_equal(
        &mut internal,
        "blending_displayed_area_aspect",
        "Pixel aspect ratio matches.",
        "Pixel aspect ratio does not match.",
        item_i32_values(path, area, tags::PRESENTATION_PIXEL_ASPECT_RATIO)?,
        vec![1, 1],
    );
    for (name, tag) in [
        ("spacing", tags::PRESENTATION_PIXEL_SPACING),
        (
            "magnification",
            tags::PRESENTATION_PIXEL_MAGNIFICATION_RATIO,
        ),
    ] {
        check(
            &mut internal,
            area.element_opt(tag)
                .map_err(|err| validation_error(path, err))?
                .is_none(),
            &format!("blending_displayed_area_{name}_absent"),
            "Conditional displayed-area attribute is absent.",
            "Unexpected displayed-area attribute is present.",
        );
    }

    for (channel, descriptor_tag, data_tag) in [
        (
            "red",
            tags::RED_PALETTE_COLOR_LOOKUP_TABLE_DESCRIPTOR,
            tags::RED_PALETTE_COLOR_LOOKUP_TABLE_DATA,
        ),
        (
            "green",
            tags::GREEN_PALETTE_COLOR_LOOKUP_TABLE_DESCRIPTOR,
            tags::GREEN_PALETTE_COLOR_LOOKUP_TABLE_DATA,
        ),
        (
            "blue",
            tags::BLUE_PALETTE_COLOR_LOOKUP_TABLE_DESCRIPTOR,
            tags::BLUE_PALETTE_COLOR_LOOKUP_TABLE_DATA,
        ),
    ] {
        let descriptor = obj
            .element(descriptor_tag)
            .map_err(|err| validation_error(path, err))?;
        check_equal(
            &mut internal,
            &format!("blending_palette_{channel}_descriptor_vr"),
            "Palette descriptor uses VR US.",
            "Palette descriptor does not use VR US.",
            descriptor.vr(),
            VR::US,
        );
        check_equal(
            &mut internal,
            &format!("blending_palette_{channel}_descriptor"),
            "Palette descriptor matches [256,0,16].",
            "Palette descriptor does not match.",
            descriptor
                .value()
                .to_multi_int::<u16>()
                .map_err(|err| validation_error(path, err))?,
            vec![256, 0, 16],
        );
        let data = obj
            .element(data_tag)
            .map_err(|err| validation_error(path, err))?;
        let data_bytes = data
            .value()
            .to_bytes()
            .map_err(|err| validation_error(path, err))?;
        check_equal(
            &mut internal,
            &format!("blending_palette_{channel}_data_vr"),
            "Palette data uses VR OW.",
            "Palette data does not use VR OW.",
            data.vr(),
            VR::OW,
        );
        check_equal(
            &mut internal,
            &format!("blending_palette_{channel}_data_size"),
            "Palette data has 512 bytes.",
            "Palette data length is invalid.",
            data_bytes.len(),
            512,
        );
        check_equal(
            &mut internal,
            &format!("blending_palette_{channel}_data_sha256"),
            "Palette bytes match the locked identity ramp.",
            "Palette bytes do not match the locked identity ramp.",
            sha256_hex(data_bytes.as_ref()).as_str(),
            expected.palette_channel_sha256,
        );
    }
    for (name, tag) in [
        (
            "segmented_red",
            tags::SEGMENTED_RED_PALETTE_COLOR_LOOKUP_TABLE_DATA,
        ),
        (
            "segmented_green",
            tags::SEGMENTED_GREEN_PALETTE_COLOR_LOOKUP_TABLE_DATA,
        ),
        (
            "segmented_blue",
            tags::SEGMENTED_BLUE_PALETTE_COLOR_LOOKUP_TABLE_DATA,
        ),
        ("uid", tags::PALETTE_COLOR_LOOKUP_TABLE_UID),
    ] {
        check(
            &mut internal,
            obj.element_opt(tag)
                .map_err(|err| validation_error(path, err))?
                .is_none(),
            &format!("blending_palette_{name}_absent"),
            "Forbidden palette representation is absent.",
            "Forbidden palette representation is present.",
        );
    }
    let icc_element = obj
        .element(tags::ICC_PROFILE)
        .map_err(|err| validation_error(path, err))?;
    let icc_bytes = icc_element
        .value()
        .to_bytes()
        .map_err(|err| validation_error(path, err))?;
    let icc = icc_bytes.as_ref();
    check_equal(
        &mut internal,
        "blending_icc_vr",
        "ICC Profile uses OB.",
        "ICC Profile does not use OB.",
        icc_element.vr(),
        VR::OB,
    );
    check_equal(
        &mut internal,
        "blending_icc_size",
        "ICC Profile has 736 bytes.",
        "ICC Profile size is invalid.",
        icc.len(),
        736,
    );
    check_equal(
        &mut internal,
        "blending_icc_sha256",
        "ICC bytes match the locked profile.",
        "ICC bytes do not match the locked profile.",
        sha256_hex(icc).as_str(),
        expected.icc_profile_sha256,
    );
    for (name, range, locked) in [
        ("device_class", 12..16, &b"scnr"[..]),
        ("data_color_space", 16..20, &b"RGB "[..]),
        ("connection_space", 20..24, &b"XYZ "[..]),
        ("signature", 36..40, &b"acsp"[..]),
    ] {
        check_equal(
            &mut internal,
            &format!("blending_icc_{name}"),
            "ICC header matches the locked profile.",
            "ICC header does not match the locked profile.",
            icc.get(range),
            Some(locked),
        );
    }
    check_equal(
        &mut internal,
        "blending_color_space",
        "DICOM Color Space is SRGB.",
        "DICOM Color Space is not SRGB.",
        element_str(path, &obj, tags::COLOR_SPACE)?.as_str(),
        "SRGB",
    );

    for (name, tag) in [
        ("frame_of_reference", tags::FRAME_OF_REFERENCE_UID),
        ("position_reference", tags::POSITION_REFERENCE_INDICATOR),
        ("common_reference", tags::REFERENCED_SERIES_SEQUENCE),
        (
            "other_studies",
            tags::STUDIES_CONTAINING_OTHER_REFERENCED_INSTANCES_SEQUENCE,
        ),
        ("graphic_group", tags::GRAPHIC_GROUP_SEQUENCE),
        ("graphic_annotation", tags::GRAPHIC_ANNOTATION_SEQUENCE),
        ("graphic_layer", tags::GRAPHIC_LAYER_SEQUENCE),
        ("spatial_transform_flip", tags::IMAGE_HORIZONTAL_FLIP),
        ("spatial_transform_rotation", tags::IMAGE_ROTATION),
        ("softcopy_voi", tags::SOFTCOPY_VOILUT_SEQUENCE),
        ("voi_lut", tags::VOILUT_SEQUENCE),
        ("window_center", tags::WINDOW_CENTER),
        ("window_width", tags::WINDOW_WIDTH),
        ("presentation_lut_sequence", tags::PRESENTATION_LUT_SEQUENCE),
        ("presentation_lut", tags::PRESENTATION_LUT_SHAPE),
        ("display_shutter_shape", tags::SHUTTER_SHAPE),
        ("display_shutter_value", tags::SHUTTER_PRESENTATION_VALUE),
        ("pixel_data", tags::PIXEL_DATA),
        ("specimen", tags::SPECIMEN_DESCRIPTION_SEQUENCE),
        ("patient_study", tags::PATIENT_AGE),
        ("patient_study_history", tags::ADDITIONAL_PATIENT_HISTORY),
        (
            "patient_study_diagnosis",
            tags::ADMITTING_DIAGNOSES_DESCRIPTION,
        ),
        ("patient_study_pregnancy", tags::PREGNANCY_STATUS),
        ("patient_study_menstrual_date", tags::LAST_MENSTRUAL_DATE),
        ("clinical_trial_subject", tags::CLINICAL_TRIAL_SPONSOR_NAME),
        ("clinical_trial_series", tags::CLINICAL_TRIAL_SERIES_ID),
    ] {
        check(
            &mut internal,
            obj.element_opt(tag)
                .map_err(|err| validation_error(path, err))?
                .is_none(),
            &format!("blending_{name}_absent"),
            "Forbidden module content is absent.",
            "Forbidden module content is present.",
        );
    }
    let overlay_activation_present = obj.iter().any(|element| {
        element.tag().group() & 0xFF00 == 0x6000 && element.tag().element() == 0x1001
    });
    check(
        &mut internal,
        !overlay_activation_present,
        "blending_overlay_activation_absent",
        "Overlay Activation is absent.",
        "Overlay Activation is present.",
    );
    let overlay_plane_present = obj
        .iter()
        .any(|element| element.tag().group() & 0xFF00 == 0x6000);
    check(
        &mut internal,
        !overlay_plane_present,
        "blending_overlay_plane_absent",
        "Overlay Plane and Bitmap Display Shutter content are absent.",
        "Overlay Plane or Bitmap Display Shutter content is present.",
    );

    fail_if_any_failed(path, &internal)?;
    Ok(ValidatedPart10 {
        bytes,
        validation: serde_json::json!({
            "status": "passed", "internal": internal,
            "standards": [
                {"name": standard_sop_class_validation_name(expected.sop_class_uid), "status": "passed", "message": standard_sop_class_validation_message(expected.sop_class_uid)},
                {"name": standard_transfer_syntax_validation_name(expected.transfer_syntax_uid), "status": "passed", "message": standard_transfer_syntax_validation_message(expected.transfer_syntax_uid)},
                {"name": "blending_presentation_state_modules", "status": "passed", "message": "Blending topology, source closure, rescale transforms, opacity, displayed area, palette, ICC, and absence invariants match the locked recipe."}
            ], "external": []
        }),
    })
}

pub(crate) fn validate_advanced_blending_presentation_state_file(
    path: &Path,
    expected: &AdvancedBlendingPresentationStateExpectations<'_>,
) -> Result<ValidatedPart10, GenerateError> {
    let bytes = fs::read(path).map_err(|source| GenerateError::ReadGeneratedFile {
        path: path.to_path_buf(),
        source,
    })?;
    let obj = open_file(path).map_err(|err| GenerateError::ValidateDicomFile {
        path: path.to_path_buf(),
        message: err.to_string(),
    })?;
    let mut internal = Vec::new();

    check(
        &mut internal,
        bytes.len() >= 132 && &bytes[128..132] == b"DICM",
        "advanced_blending_part10_preamble",
        "File has a Part 10 preamble and DICM marker.",
        "File is missing the Part 10 DICM marker.",
    );
    check_equal(
        &mut internal,
        "advanced_blending_transfer_syntax",
        "Transfer Syntax matches the locked recipe.",
        "Transfer Syntax does not match the locked recipe.",
        trim_uid(obj.meta().transfer_syntax()),
        expected.transfer_syntax_uid.to_string(),
    );
    let sop_class_uid = element_str(path, &obj, tags::SOP_CLASS_UID)?;
    let sop_instance_uid = element_str(path, &obj, tags::SOP_INSTANCE_UID)?;
    for (name, actual, locked) in [
        (
            "advanced_blending_sop_class_uid",
            sop_class_uid.as_str(),
            expected.sop_class_uid,
        ),
        (
            "advanced_blending_sop_instance_uid",
            sop_instance_uid.as_str(),
            expected.sop_instance_uid,
        ),
        (
            "advanced_blending_synthetic_data",
            element_str(path, &obj, tags::SYNTHETIC_DATA)?.as_str(),
            expected.synthetic_data,
        ),
        (
            "advanced_blending_study_instance_uid",
            element_str(path, &obj, tags::STUDY_INSTANCE_UID)?.as_str(),
            expected.study_instance_uid,
        ),
        (
            "advanced_blending_series_instance_uid",
            element_str(path, &obj, tags::SERIES_INSTANCE_UID)?.as_str(),
            expected.series_instance_uid,
        ),
        (
            "advanced_blending_frame_of_reference_uid",
            element_str(path, &obj, tags::FRAME_OF_REFERENCE_UID)?.as_str(),
            expected.frame_of_reference_uid,
        ),
    ] {
        check_equal(
            &mut internal,
            name,
            "Identity matches the locked recipe.",
            "Identity does not match the locked recipe.",
            actual,
            locked,
        );
    }
    check_equal(
        &mut internal,
        "advanced_blending_media_storage_sop_class_uid",
        "File Meta SOP Class matches the dataset.",
        "File Meta SOP Class does not match the dataset.",
        trim_uid(obj.meta().media_storage_sop_class_uid()),
        sop_class_uid,
    );
    check_equal(
        &mut internal,
        "advanced_blending_media_storage_sop_instance_uid",
        "File Meta SOP Instance matches the dataset.",
        "File Meta SOP Instance does not match the dataset.",
        trim_uid(obj.meta().media_storage_sop_instance_uid()),
        sop_instance_uid,
    );
    check_equal(
        &mut internal,
        "advanced_blending_implementation_class_uid",
        "Implementation Class UID matches the generator.",
        "Implementation Class UID does not match the generator.",
        trim_uid(obj.meta().implementation_class_uid()).as_str(),
        expected.implementation_class_uid,
    );
    for (name, tag, locked) in [
        (
            "advanced_blending_patient_name",
            tags::PATIENT_NAME,
            "DTS^Synthetic^Patient001",
        ),
        (
            "advanced_blending_patient_id",
            tags::PATIENT_ID,
            "DTS-PATIENT-001",
        ),
        (
            "advanced_blending_patient_birth_date",
            tags::PATIENT_BIRTH_DATE,
            "19700101",
        ),
        ("advanced_blending_patient_sex", tags::PATIENT_SEX, "O"),
        ("advanced_blending_study_date", tags::STUDY_DATE, "20260101"),
        ("advanced_blending_study_time", tags::STUDY_TIME, "000000"),
        (
            "advanced_blending_referring_physician",
            tags::REFERRING_PHYSICIAN_NAME,
            "",
        ),
        ("advanced_blending_study_id", tags::STUDY_ID, "DTS-CT"),
        (
            "advanced_blending_accession_number",
            tags::ACCESSION_NUMBER,
            "",
        ),
        ("advanced_blending_modality", tags::MODALITY, "PR"),
        ("advanced_blending_series_number", tags::SERIES_NUMBER, "80"),
        ("advanced_blending_laterality", tags::LATERALITY, "R"),
        (
            "advanced_blending_manufacturer",
            tags::MANUFACTURER,
            "dicom-test-suite",
        ),
        (
            "advanced_blending_institution_name",
            tags::INSTITUTION_NAME,
            "",
        ),
        (
            "advanced_blending_institution_address",
            tags::INSTITUTION_ADDRESS,
            "",
        ),
        (
            "advanced_blending_manufacturer_model_name",
            tags::MANUFACTURER_MODEL_NAME,
            "Native Advanced Blending Presentation State",
        ),
        (
            "advanced_blending_device_serial_number",
            tags::DEVICE_SERIAL_NUMBER,
            "DTS-ADVBLEND-001",
        ),
        (
            "advanced_blending_software_versions",
            tags::SOFTWARE_VERSIONS,
            crate::PACKAGE_VERSION,
        ),
        (
            "advanced_blending_instance_number",
            tags::INSTANCE_NUMBER,
            "1",
        ),
        (
            "advanced_blending_creation_date",
            tags::PRESENTATION_CREATION_DATE,
            "20260101",
        ),
        (
            "advanced_blending_creation_time",
            tags::PRESENTATION_CREATION_TIME,
            "000000",
        ),
        (
            "advanced_blending_content_label",
            tags::CONTENT_LABEL,
            "DTSADVBLEND",
        ),
        (
            "advanced_blending_content_description",
            tags::CONTENT_DESCRIPTION,
            "Synthetic DTSADVBLEND presentation state",
        ),
        (
            "advanced_blending_content_creator",
            tags::CONTENT_CREATOR_NAME,
            "DTS^Generator",
        ),
        (
            "advanced_blending_pixel_presentation",
            tags::PIXEL_PRESENTATION,
            "TRUE_COLOR",
        ),
    ] {
        check_equal(
            &mut internal,
            name,
            "Attribute matches the locked recipe.",
            "Attribute does not match the locked recipe.",
            element_str(path, &obj, tag)?.as_str(),
            locked,
        );
    }
    check_equal(
        &mut internal,
        "advanced_blending_position_reference_indicator",
        "Position Reference Indicator is present and empty.",
        "Position Reference Indicator is missing or non-empty.",
        element_str(path, &obj, tags::POSITION_REFERENCE_INDICATOR)?.as_str(),
        "",
    );
    check(
        &mut internal,
        expected
            .source_series
            .iter()
            .all(|source| source.series_instance_uid != expected.series_instance_uid),
        "advanced_blending_distinct_presentation_series",
        "Presentation Series is distinct from both source Series.",
        "Presentation Series unexpectedly reuses a source Series UID.",
    );

    check_equal(
        &mut internal,
        "advanced_blending_input_count",
        "Advanced Blending Sequence has exactly two inputs.",
        "Advanced Blending Sequence does not have exactly two inputs.",
        sequence_item_count(path, &obj, tags::ADVANCED_BLENDING_SEQUENCE)?,
        2,
    );
    let mut geometry_source_count = 0;
    for (input_index, source) in expected.source_series.iter().enumerate() {
        let input =
            top_level_sequence_item(path, &obj, tags::ADVANCED_BLENDING_SEQUENCE, input_index)?;
        let ordinal = (input_index + 1) as u16;
        let input_number_element = input
            .element(tags::BLENDING_INPUT_NUMBER)
            .map_err(|err| validation_error(path, err))?;
        check_equal(
            &mut internal,
            &format!("advanced_blending_input_{}_number_vr", ordinal),
            "Blending Input Number uses VR US.",
            "Blending Input Number does not use VR US.",
            input_number_element.vr(),
            VR::US,
        );
        check_equal(
            &mut internal,
            &format!("advanced_blending_input_{}_number_vm", ordinal),
            "Blending Input Number has VM 1.",
            "Blending Input Number does not have VM 1.",
            input_number_element.value().multiplicity(),
            1,
        );
        check_equal(
            &mut internal,
            &format!("advanced_blending_input_{}_number", ordinal),
            "Blending Input Number is the ordered ordinal.",
            "Blending Input Number is missing, duplicated, or non-ordinal.",
            item_u16(path, input, tags::BLENDING_INPUT_NUMBER)?,
            ordinal,
        );
        check_equal(
            &mut internal,
            &format!("advanced_blending_input_{}_study", ordinal),
            "Input references the source Study.",
            "Input redirects to another Study.",
            item_str(path, input, tags::STUDY_INSTANCE_UID)?.as_str(),
            expected.study_instance_uid,
        );
        check_equal(
            &mut internal,
            &format!("advanced_blending_input_{}_series", ordinal),
            "Input references the ordered source Series.",
            "Input redirects to or reorders a source Series.",
            item_str(path, input, tags::SERIES_INSTANCE_UID)?.as_str(),
            source.series_instance_uid,
        );
        check_equal(
            &mut internal,
            &format!("advanced_blending_input_{}_image_count", ordinal),
            "Input references exactly two source Images.",
            "Input omits, duplicates, or adds a source Image.",
            item_sequence_item_count(path, input, tags::REFERENCED_IMAGE_SEQUENCE)?,
            2,
        );
        for (image_index, expected_sop_uid) in source.sop_instance_uids.iter().enumerate() {
            let image =
                item_sequence_item(path, input, tags::REFERENCED_IMAGE_SEQUENCE, image_index)?;
            check_equal(
                &mut internal,
                &format!(
                    "advanced_blending_input_{}_image_{}_sop_class",
                    ordinal,
                    image_index + 1
                ),
                "Source SOP Class matches the locked CT identity.",
                "Source SOP Class is redirected.",
                item_str(path, image, tags::REFERENCED_SOP_CLASS_UID)?.as_str(),
                source.sop_class_uid,
            );
            check_equal(
                &mut internal,
                &format!(
                    "advanced_blending_input_{}_image_{}_sop_instance",
                    ordinal,
                    image_index + 1
                ),
                "Source SOP Instance matches the locked slice order.",
                "Source SOP Instance is dangling, duplicated, or reordered.",
                item_str(path, image, tags::REFERENCED_SOP_INSTANCE_UID)?.as_str(),
                *expected_sop_uid,
            );
            check(
                &mut internal,
                image
                    .element_opt(tags::REFERENCED_FRAME_NUMBER)
                    .map_err(|err| validation_error(path, err))?
                    .is_none(),
                &format!(
                    "advanced_blending_input_{}_image_{}_complete_instance",
                    ordinal,
                    image_index + 1
                ),
                "Reference selects the complete source Instance.",
                "Referenced Frame Number unexpectedly narrows the source Instance.",
            );
        }
        check_equal(
            &mut internal,
            &format!("advanced_blending_input_{}_time_series", ordinal),
            "Time Series Blending is FALSE.",
            "Time Series Blending does not match the locked recipe.",
            item_str(path, input, tags::TIME_SERIES_BLENDING)?.as_str(),
            "FALSE",
        );
        let geometry = item_str(path, input, tags::GEOMETRY_FOR_DISPLAY)?;
        if geometry == "TRUE" {
            geometry_source_count += 1;
        }
        check_equal(
            &mut internal,
            &format!("advanced_blending_input_{}_geometry", ordinal),
            "Geometry for Display matches the locked source selection.",
            "Geometry for Display does not match the locked source selection.",
            geometry.as_str(),
            if ordinal == 1 { "TRUE" } else { "FALSE" },
        );
        for (name, tag) in [
            (
                "spatial_registration",
                tags::REFERENCED_SPATIAL_REGISTRATION_SEQUENCE,
            ),
            ("optical_path", tags::OPTICAL_PATH_IDENTIFICATION_SEQUENCE),
            ("softcopy_voi", tags::SOFTCOPY_VOILUT_SEQUENCE),
            (
                "palette_color_lut",
                tags::PALETTE_COLOR_LOOKUP_TABLE_SEQUENCE,
            ),
            ("threshold", tags::THRESHOLD_SEQUENCE),
        ] {
            check(
                &mut internal,
                input
                    .element_opt(tag)
                    .map_err(|err| validation_error(path, err))?
                    .is_none(),
                &format!("advanced_blending_input_{}_{}_absent", ordinal, name),
                "Optional input transform is absent.",
                "A forbidden optional input transform is present.",
            );
        }
    }
    check_equal(
        &mut internal,
        "advanced_blending_single_geometry_source",
        "Exactly one input supplies display geometry.",
        "Display geometry has zero or multiple sources.",
        geometry_source_count,
        1,
    );

    check_equal(
        &mut internal,
        "advanced_blending_display_operation_count",
        "Exactly one display operation is present.",
        "Display operation cardinality does not match the locked recipe.",
        sequence_item_count(path, &obj, tags::BLENDING_DISPLAY_SEQUENCE)?,
        1,
    );
    let display = top_level_sequence_item(path, &obj, tags::BLENDING_DISPLAY_SEQUENCE, 0)?;
    check_equal(
        &mut internal,
        "advanced_blending_display_input_count",
        "Display operation consumes exactly two inputs.",
        "Display operation input cardinality is invalid.",
        item_sequence_item_count(path, display, tags::BLENDING_DISPLAY_INPUT_SEQUENCE)?,
        2,
    );
    for (index, ordinal) in [1_u16, 2].iter().enumerate() {
        let display_input =
            item_sequence_item(path, display, tags::BLENDING_DISPLAY_INPUT_SEQUENCE, index)?;
        let display_input_number = display_input
            .element(tags::BLENDING_INPUT_NUMBER)
            .map_err(|err| validation_error(path, err))?;
        check_equal(
            &mut internal,
            &format!("advanced_blending_display_input_{}_vr", index + 1),
            "Display Blending Input Number uses VR US.",
            "Display Blending Input Number does not use VR US.",
            display_input_number.vr(),
            VR::US,
        );
        check_equal(
            &mut internal,
            &format!("advanced_blending_display_input_{}_vm", index + 1),
            "Display Blending Input Number has VM 1.",
            "Display Blending Input Number does not have VM 1.",
            display_input_number.value().multiplicity(),
            1,
        );
        check_equal(
            &mut internal,
            &format!("advanced_blending_display_input_{}_order", index + 1),
            "Display input identifies the locked ordered input.",
            "Display input is dangling, duplicated, missing, or reordered.",
            item_u16(path, display_input, tags::BLENDING_INPUT_NUMBER)?,
            *ordinal,
        );
    }
    check_equal(
        &mut internal,
        "advanced_blending_mode",
        "Blending Mode is EQUAL.",
        "Blending Mode does not match the locked recipe.",
        item_str(path, display, tags::BLENDING_MODE)?.as_str(),
        "EQUAL",
    );
    for (name, tag) in [
        ("relative_opacity", tags::RELATIVE_OPACITY),
        ("output_input_number", tags::BLENDING_INPUT_NUMBER),
    ] {
        check(
            &mut internal,
            display
                .element_opt(tag)
                .map_err(|err| validation_error(path, err))?
                .is_none(),
            &format!("advanced_blending_display_{}_absent", name),
            "Optional display output attribute is absent.",
            "Sole display operation is not the locked final EQUAL operation.",
        );
    }

    let icc_element = obj
        .element(tags::ICC_PROFILE)
        .map_err(|err| validation_error(path, err))?;
    let icc_bytes = icc_element
        .value()
        .to_bytes()
        .map_err(|err| validation_error(path, err))?;
    let icc = icc_bytes.as_ref();
    check_equal(
        &mut internal,
        "advanced_blending_icc_vr",
        "ICC Profile uses OB.",
        "ICC Profile does not use OB.",
        icc_element.vr(),
        VR::OB,
    );
    check_equal(
        &mut internal,
        "advanced_blending_icc_size",
        "ICC Profile has 736 bytes.",
        "ICC Profile size is invalid.",
        icc.len(),
        736,
    );
    check_equal(
        &mut internal,
        "advanced_blending_icc_sha256",
        "ICC bytes match the locked profile.",
        "ICC bytes do not match the locked profile.",
        sha256_hex(icc).as_str(),
        expected.icc_profile_sha256,
    );
    check_equal(
        &mut internal,
        "advanced_blending_icc_declared_size",
        "ICC header declares the exact profile size.",
        "ICC header does not declare the exact profile size.",
        icc.get(0..4)
            .and_then(|bytes| <[u8; 4]>::try_from(bytes).ok())
            .map(u32::from_be_bytes),
        Some(736),
    );
    for (name, range, locked) in [
        ("device_class", 12..16, &b"scnr"[..]),
        ("data_color_space", 16..20, &b"RGB "[..]),
        ("connection_space", 20..24, &b"XYZ "[..]),
        ("signature", 36..40, &b"acsp"[..]),
    ] {
        check_equal(
            &mut internal,
            &format!("advanced_blending_icc_{name}"),
            "ICC header matches the locked profile.",
            "ICC header does not match the locked profile.",
            icc.get(range),
            Some(locked),
        );
    }
    check_equal(
        &mut internal,
        "advanced_blending_color_space",
        "DICOM Color Space is SRGB.",
        "DICOM Color Space is not SRGB.",
        element_str(path, &obj, tags::COLOR_SPACE)?.as_str(),
        "SRGB",
    );

    check_equal(
        &mut internal,
        "advanced_blending_common_series_count",
        "Common Instance Reference contains exactly two Series.",
        "Common Instance Reference Series cardinality is invalid.",
        sequence_item_count(path, &obj, tags::REFERENCED_SERIES_SEQUENCE)?,
        2,
    );
    for (series_index, source) in expected.source_series.iter().enumerate() {
        let series =
            top_level_sequence_item(path, &obj, tags::REFERENCED_SERIES_SEQUENCE, series_index)?;
        check_equal(
            &mut internal,
            &format!("advanced_blending_common_series_{}_uid", series_index + 1),
            "Common reference preserves source Series order.",
            "Common reference redirects or reorders a source Series.",
            item_str(path, series, tags::SERIES_INSTANCE_UID)?.as_str(),
            source.series_instance_uid,
        );
        let common_instance_count =
            item_sequence_item_count(path, series, tags::REFERENCED_INSTANCE_SEQUENCE)?;
        check_equal(
            &mut internal,
            &format!(
                "advanced_blending_common_series_{}_instance_count",
                series_index + 1
            ),
            "Common reference contains both source Instances.",
            "Common reference omits, duplicates, or adds an Instance.",
            common_instance_count,
            2,
        );
        for (image_index, expected_sop_uid) in source
            .sop_instance_uids
            .iter()
            .take(common_instance_count)
            .enumerate()
        {
            let image = item_sequence_item(
                path,
                series,
                tags::REFERENCED_INSTANCE_SEQUENCE,
                image_index,
            )?;
            check_equal(
                &mut internal,
                &format!(
                    "advanced_blending_common_series_{}_image_{}_sop_class",
                    series_index + 1,
                    image_index + 1
                ),
                "Common reference SOP Class mirrors the blending input.",
                "Common reference SOP Class does not mirror the blending input.",
                item_str(path, image, tags::REFERENCED_SOP_CLASS_UID)?.as_str(),
                source.sop_class_uid,
            );
            check_equal(
                &mut internal,
                &format!(
                    "advanced_blending_common_series_{}_image_{}_sop_instance",
                    series_index + 1,
                    image_index + 1
                ),
                "Common reference SOP Instance mirrors the blending input.",
                "Common reference is dangling, omitted, duplicated, or reordered.",
                item_str(path, image, tags::REFERENCED_SOP_INSTANCE_UID)?.as_str(),
                *expected_sop_uid,
            );
        }
    }
    for (name, tag) in [
        (
            "other_studies",
            tags::STUDIES_CONTAINING_OTHER_REFERENCED_INSTANCES_SEQUENCE,
        ),
        ("displayed_area", tags::DISPLAYED_AREA_SELECTION_SEQUENCE),
        ("graphic_annotation", tags::GRAPHIC_ANNOTATION_SEQUENCE),
        ("graphic_layer", tags::GRAPHIC_LAYER_SEQUENCE),
        ("spatial_transform_flip", tags::IMAGE_HORIZONTAL_FLIP),
        ("spatial_transform_rotation", tags::IMAGE_ROTATION),
        ("pixel_data", tags::PIXEL_DATA),
    ] {
        check(
            &mut internal,
            obj.element_opt(tag)
                .map_err(|err| validation_error(path, err))?
                .is_none(),
            &format!("advanced_blending_{name}_absent"),
            "Forbidden optional content is absent.",
            "Forbidden optional content is present.",
        );
    }

    fail_if_any_failed(path, &internal)?;
    Ok(ValidatedPart10 {
        bytes,
        validation: serde_json::json!({
            "status": "passed",
            "internal": internal,
            "standards": [
                {
                    "name": standard_sop_class_validation_name(expected.sop_class_uid),
                    "status": "passed",
                    "message": standard_sop_class_validation_message(expected.sop_class_uid)
                },
                {
                    "name": standard_transfer_syntax_validation_name(expected.transfer_syntax_uid),
                    "status": "passed",
                    "message": standard_transfer_syntax_validation_message(expected.transfer_syntax_uid)
                },
                {
                    "name": "advanced_blending_presentation_state_modules",
                    "status": "passed",
                    "message": "Advanced Blending input topology, display graph, ICC identity, common-reference closure, and absence invariants match the locked recipe."
                }
            ],
            "external": []
        }),
    })
}

#[derive(Debug, Clone, Copy)]
struct WaveformEcgValidationRecipe<'a> {
    finding_prefix: &'a str,
    series_number: &'a str,
    manufacturer_model_name: &'a str,
    device_serial_number: &'a str,
    module_validation_name: &'a str,
    module_validation_message: &'a str,
    qualify_channel_findings_by_group: bool,
    sample_formula: fn(usize, usize, usize) -> i16,
    sample_formula_contract: &'a str,
}

#[derive(Debug, Clone, Copy)]
struct WaveformEcgValidationExpectations<'a> {
    sop_instance_uid: &'a str,
    implementation_class_uid: &'a str,
    study_instance_uid: &'a str,
    series_instance_uid: &'a str,
    waveform: ExpectedWaveform<'a>,
}

pub(crate) fn validate_twelve_lead_ecg_file(
    path: &Path,
    expected: &TwelveLeadEcgExpectations<'_>,
) -> Result<ValidatedPart10, GenerateError> {
    validate_waveform_ecg_file(
        path,
        WaveformEcgValidationExpectations {
            sop_instance_uid: expected.sop_instance_uid,
            implementation_class_uid: expected.implementation_class_uid,
            study_instance_uid: expected.study_instance_uid,
            series_instance_uid: expected.series_instance_uid,
            waveform: expected.waveform,
        },
        WaveformEcgValidationRecipe {
            finding_prefix: "twelve_lead_ecg",
            series_number: "90",
            manufacturer_model_name: "Native Twelve-lead ECG",
            device_serial_number: "DTS-ECG-001",
            module_validation_name: "twelve_lead_ecg_waveform_modules",
            module_validation_message: "Twelve-lead ECG IOD, channel definitions, signed OW storage, deterministic interleave and absence invariants match the locked recipe.",
            qualify_channel_findings_by_group: false,
            sample_formula: |_group, sample, channel| {
                (((sample * (channel + 1) * 37 + channel * 101) % 2001) as i32 - 1000) as i16
            },
            sample_formula_contract: "((s * (c + 1) * 37 + c * 101) mod 2001) - 1000",
        },
    )
}

pub(crate) fn validate_general_ecg_file(
    path: &Path,
    expected: &GeneralEcgExpectations<'_>,
) -> Result<ValidatedPart10, GenerateError> {
    validate_waveform_ecg_file(
        path,
        WaveformEcgValidationExpectations {
            sop_instance_uid: expected.sop_instance_uid,
            implementation_class_uid: expected.implementation_class_uid,
            study_instance_uid: expected.study_instance_uid,
            series_instance_uid: expected.series_instance_uid,
            waveform: expected.waveform,
        },
        WaveformEcgValidationRecipe {
            finding_prefix: "general_ecg",
            series_number: "91",
            manufacturer_model_name: "Native General ECG",
            device_serial_number: "DTS-GECG-001",
            module_validation_name: "general_ecg_waveform_modules",
            module_validation_message: "General ECG IOD, two ordered heterogeneous groups, channel definitions, signed OW storage, deterministic interleave, aggregate closure, and absence invariants match the locked recipe.",
            qualify_channel_findings_by_group: true,
            sample_formula: |group, sample, channel| {
                (((sample * (channel + 1) * (group + 1) * 37 + channel * 101 + group * 307) % 2001)
                    as i32
                    - 1000) as i16
            },
            sample_formula_contract: "((s * (c + 1) * (g + 1) * 37 + c * 101 + g * 307) mod 2001) - 1000",
        },
    )
}

fn validate_waveform_ecg_file(
    path: &Path,
    expected: WaveformEcgValidationExpectations<'_>,
    recipe: WaveformEcgValidationRecipe<'_>,
) -> Result<ValidatedPart10, GenerateError> {
    let bytes = fs::read(path).map_err(|source| GenerateError::ReadGeneratedFile {
        path: path.to_path_buf(),
        source,
    })?;
    let obj = open_file(path).map_err(|err| GenerateError::ValidateDicomFile {
        path: path.to_path_buf(),
        message: err.to_string(),
    })?;
    let waveform = expected.waveform;
    let groups_expected = waveform.multiplex_groups;
    let aggregate_expected = waveform.aggregate;
    let mut internal = Vec::new();
    let finding = |suffix: &str| format!("{}_{suffix}", recipe.finding_prefix);

    check(
        &mut internal,
        bytes.len() >= 132 && &bytes[128..132] == b"DICM",
        &finding("part10_preamble"),
        "File has a Part 10 preamble and DICM marker.",
        "File is missing the Part 10 DICM marker.",
    );
    check_equal(
        &mut internal,
        &finding("transfer_syntax"),
        "Transfer Syntax matches the locked recipe.",
        "Transfer Syntax does not match the locked recipe.",
        trim_uid(obj.meta().transfer_syntax()),
        waveform.transfer_syntax_uid.to_string(),
    );
    let sop_class_uid = element_str(path, &obj, tags::SOP_CLASS_UID)?;
    let sop_instance_uid = element_str(path, &obj, tags::SOP_INSTANCE_UID)?;
    for (name, actual, locked) in [
        (
            "sop_class_uid",
            sop_class_uid.as_str(),
            waveform.sop_class_uid,
        ),
        (
            "sop_instance_uid",
            sop_instance_uid.as_str(),
            expected.sop_instance_uid,
        ),
        (
            "synthetic_data",
            element_str(path, &obj, tags::SYNTHETIC_DATA)?.as_str(),
            "YES",
        ),
        (
            "study_instance_uid",
            element_str(path, &obj, tags::STUDY_INSTANCE_UID)?.as_str(),
            expected.study_instance_uid,
        ),
        (
            "series_instance_uid",
            element_str(path, &obj, tags::SERIES_INSTANCE_UID)?.as_str(),
            expected.series_instance_uid,
        ),
    ] {
        check_equal(
            &mut internal,
            &finding(name),
            "Identity matches the locked recipe.",
            "Identity does not match the locked recipe.",
            actual,
            locked,
        );
    }
    check_equal(
        &mut internal,
        &finding("media_storage_sop_class_uid"),
        "File Meta SOP Class matches the dataset.",
        "File Meta SOP Class does not match the dataset.",
        trim_uid(obj.meta().media_storage_sop_class_uid()),
        sop_class_uid,
    );
    check_equal(
        &mut internal,
        &finding("media_storage_sop_instance_uid"),
        "File Meta SOP Instance matches the dataset.",
        "File Meta SOP Instance does not match the dataset.",
        trim_uid(obj.meta().media_storage_sop_instance_uid()),
        sop_instance_uid,
    );
    check_equal(
        &mut internal,
        &finding("implementation_class_uid"),
        "Implementation Class UID matches the deterministic generator.",
        "Implementation Class UID does not match the deterministic generator.",
        trim_uid(obj.meta().implementation_class_uid()).as_str(),
        expected.implementation_class_uid,
    );

    for (name, tag, locked) in [
        (
            "patient_name",
            tags::PATIENT_NAME,
            "DTS^Synthetic^Patient001",
        ),
        ("patient_id", tags::PATIENT_ID, "DTS-PATIENT-001"),
        ("patient_birth_date", tags::PATIENT_BIRTH_DATE, "19700101"),
        ("patient_sex", tags::PATIENT_SEX, "O"),
        ("study_date", tags::STUDY_DATE, "20260101"),
        ("study_time", tags::STUDY_TIME, "000000"),
        ("referring_physician", tags::REFERRING_PHYSICIAN_NAME, ""),
        ("study_id", tags::STUDY_ID, "DTS-ECG"),
        ("accession_number", tags::ACCESSION_NUMBER, ""),
        ("modality", tags::MODALITY, waveform.modality),
        ("series_number", tags::SERIES_NUMBER, recipe.series_number),
        ("manufacturer", tags::MANUFACTURER, "dicom-test-suite"),
        ("institution_name", tags::INSTITUTION_NAME, ""),
        ("institution_address", tags::INSTITUTION_ADDRESS, ""),
        (
            "manufacturer_model_name",
            tags::MANUFACTURER_MODEL_NAME,
            recipe.manufacturer_model_name,
        ),
        (
            "device_serial_number",
            tags::DEVICE_SERIAL_NUMBER,
            recipe.device_serial_number,
        ),
        (
            "software_versions",
            tags::SOFTWARE_VERSIONS,
            crate::PACKAGE_VERSION,
        ),
        ("instance_number", tags::INSTANCE_NUMBER, "1"),
        ("content_date", tags::CONTENT_DATE, "20260101"),
        ("content_time", tags::CONTENT_TIME, "000000"),
        (
            "acquisition_date_time",
            tags::ACQUISITION_DATE_TIME,
            "20260101000000",
        ),
    ] {
        check_equal(
            &mut internal,
            &finding(name),
            "Required IOD attribute matches the locked recipe.",
            "Required IOD attribute does not match the locked recipe.",
            element_str(path, &obj, tag)?.as_str(),
            locked,
        );
    }
    check_equal(
        &mut internal,
        &finding("acquisition_context_count"),
        "Acquisition Context Sequence is present and empty.",
        "Acquisition Context Sequence is missing or non-empty.",
        sequence_item_count(path, &obj, tags::ACQUISITION_CONTEXT_SEQUENCE)?,
        usize::from(waveform.acquisition_context_items),
    );

    let expected_group_count = groups_expected.len();
    let actual_group_count = sequence_item_count(path, &obj, tags::WAVEFORM_SEQUENCE)?;
    check_equal(
        &mut internal,
        &finding("manifest_aggregate_group_count"),
        "Manifest aggregate group count matches the ordered group array.",
        "Manifest aggregate group count does not match the ordered group array.",
        usize::from(aggregate_expected.group_count),
        expected_group_count,
    );
    check_equal(
        &mut internal,
        &finding("group_count"),
        "Waveform Sequence cardinality matches the ordered manifest groups.",
        "Waveform Sequence cardinality does not match the ordered manifest groups.",
        actual_group_count,
        expected_group_count,
    );
    let expected_total_channels = groups_expected
        .iter()
        .map(|group| usize::from(group.channel_count))
        .sum::<usize>();
    let expected_total_payload_bytes = groups_expected
        .iter()
        .map(|group| usize::from(group.storage.payload_length_bytes))
        .sum::<usize>();
    check_equal(
        &mut internal,
        &finding("manifest_aggregate_channel_count"),
        "Manifest aggregate channel count matches its ordered groups.",
        "Manifest aggregate channel count does not match its ordered groups.",
        usize::from(aggregate_expected.total_channel_count),
        expected_total_channels,
    );
    check_equal(
        &mut internal,
        &finding("manifest_aggregate_payload_length"),
        "Manifest aggregate payload length matches its ordered groups.",
        "Manifest aggregate payload length does not match its ordered groups.",
        usize::from(aggregate_expected.total_payload_length_bytes),
        expected_total_payload_bytes,
    );
    check_equal(
        &mut internal,
        &finding("manifest_aggregate_group_hash_count"),
        "Manifest aggregate contains one ordered payload hash per group.",
        "Manifest aggregate group hash cardinality does not match its ordered groups.",
        aggregate_expected.group_payload_sha256.len(),
        expected_group_count,
    );
    let common_duration_matches = groups_expected
        .iter()
        .all(|group| group.duration_seconds == aggregate_expected.common_duration_seconds);
    check(
        &mut internal,
        common_duration_matches,
        &finding("manifest_aggregate_common_duration"),
        "Every ordered group matches the manifest aggregate common duration.",
        "A group duration does not match the manifest aggregate common duration.",
    );
    let group_hashes_match = groups_expected
        .iter()
        .zip(aggregate_expected.group_payload_sha256.iter())
        .all(|(group, aggregate_hash)| group.storage.payload_sha256 == *aggregate_hash);
    check(
        &mut internal,
        aggregate_expected.group_payload_sha256.len() == expected_group_count && group_hashes_match,
        &finding("manifest_aggregate_group_hashes"),
        "Manifest aggregate preserves the ordered group payload hashes.",
        "Manifest aggregate group hashes are missing, reordered, or changed.",
    );
    fail_if_any_failed(path, &internal)?;

    let mut aggregate_payload = Vec::with_capacity(expected_total_payload_bytes);
    let mut actual_group_hashes = Vec::with_capacity(expected_group_count);
    let mut actual_total_channels = 0_usize;
    for (group_index, group_expected) in groups_expected.iter().enumerate() {
        let expected_ordinal = group_index + 1;
        let storage = group_expected.storage;
        check_equal(
            &mut internal,
            &format!("{}_group_{expected_ordinal}_ordinal", recipe.finding_prefix),
            "Manifest multiplex-group ordinal is one-based and ordered.",
            "Manifest multiplex-group ordinal is missing, duplicated, or reordered.",
            usize::from(group_expected.ordinal),
            expected_ordinal,
        );
        let group = top_level_sequence_item(path, &obj, tags::WAVEFORM_SEQUENCE, group_index)?;
        for (name, tag, locked) in [
            (
                "originality",
                tags::WAVEFORM_ORIGINALITY,
                group_expected.originality,
            ),
            ("label", tags::MULTIPLEX_GROUP_LABEL, group_expected.label),
            (
                "sample_interpretation",
                tags::WAVEFORM_SAMPLE_INTERPRETATION,
                storage.sample_interpretation,
            ),
        ] {
            check_equal(
                &mut internal,
                &finding(name),
                "Waveform attribute matches the locked recipe.",
                "Waveform attribute does not match the locked recipe.",
                item_str(path, group, tag)?.as_str(),
                locked,
            );
        }
        let channel_count = item_u16(path, group, tags::NUMBER_OF_WAVEFORM_CHANNELS)?;
        let sample_count = item_u32(path, group, tags::NUMBER_OF_WAVEFORM_SAMPLES)?;
        let bits_allocated = item_u16(path, group, tags::WAVEFORM_BITS_ALLOCATED)?;
        check_equal(
            &mut internal,
            &finding("sample_interpretation_vr"),
            "Waveform Sample Interpretation uses VR CS.",
            "Waveform Sample Interpretation does not use VR CS.",
            group
                .element(tags::WAVEFORM_SAMPLE_INTERPRETATION)
                .map_err(|err| validation_error(path, err))?
                .vr(),
            VR::CS,
        );
        check_equal(
            &mut internal,
            &finding("manifest_channel_count"),
            "Manifest group channel count matches its channel definitions.",
            "Manifest group channel count does not match its channel definitions.",
            usize::from(group_expected.channel_count),
            group_expected.channels.len(),
        );
        check_equal(
            &mut internal,
            &finding("channel_count"),
            "Number of Waveform Channels is exactly twelve.",
            "Number of Waveform Channels is not exactly twelve.",
            channel_count,
            u16::from(group_expected.channel_count),
        );
        check_equal(
            &mut internal,
            &finding("sample_count"),
            "Number of Waveform Samples matches the locked one-second trace.",
            "Number of Waveform Samples does not match the locked trace.",
            sample_count,
            u32::from(group_expected.samples_per_channel),
        );
        check_equal(
            &mut internal,
            &finding("sampling_frequency"),
            "Sampling Frequency is 500 Hz.",
            "Sampling Frequency is not 500 Hz.",
            item_f64(path, group, tags::SAMPLING_FREQUENCY)?,
            f64::from(group_expected.sampling_frequency_hz),
        );
        check_equal(
            &mut internal,
            &finding("duration"),
            "Sample count and frequency encode the locked duration.",
            "Sample count and frequency do not encode the locked duration.",
            f64::from(sample_count) / item_f64(path, group, tags::SAMPLING_FREQUENCY)?,
            f64::from(group_expected.duration_seconds),
        );
        check_equal(
            &mut internal,
            &finding("bits_allocated"),
            "Waveform Bits Allocated is 16.",
            "Waveform Bits Allocated is not 16.",
            bits_allocated,
            u16::from(storage.bits_allocated),
        );
        check_equal(
            &mut internal,
            &finding("channel_definition_count"),
            "Channel Definition Sequence contains the twelve ordered leads.",
            "Channel Definition Sequence does not contain exactly twelve leads.",
            item_sequence_item_count(path, group, tags::CHANNEL_DEFINITION_SEQUENCE)?,
            group_expected.channels.len(),
        );
        fail_if_any_failed(path, &internal)?;
        actual_total_channels += usize::from(channel_count);

        for (index, channel_expected) in group_expected.channels.iter().enumerate() {
            let channel =
                item_sequence_item(path, group, tags::CHANNEL_DEFINITION_SEQUENCE, index)?;
            let prefix = if recipe.qualify_channel_findings_by_group {
                format!(
                    "{}_group_{}_channel_{}",
                    recipe.finding_prefix,
                    expected_ordinal,
                    index + 1
                )
            } else {
                format!("{}_channel_{}", recipe.finding_prefix, index + 1)
            };
            check_equal(
                &mut internal,
                &format!("{prefix}_ordinal"),
                "Waveform Channel Number is the one-based channel ordinal.",
                "Waveform Channel Number is missing, duplicated, or reordered.",
                item_str(path, channel, tags::WAVEFORM_CHANNEL_NUMBER)?.as_str(),
                channel_expected.ordinal.to_string().as_str(),
            );
            check_equal(
                &mut internal,
                &format!("{prefix}_label"),
                "Channel Label matches the locked lead order.",
                "Channel Label does not match the locked lead order.",
                item_str(path, channel, tags::CHANNEL_LABEL)?.as_str(),
                channel_expected.label,
            );
            validate_waveform_code(
                &mut internal,
                path,
                channel,
                tags::CHANNEL_SOURCE_SEQUENCE,
                &format!("{prefix}_source"),
                channel_expected.source,
            )?;
            check_equal(
                &mut internal,
                &format!("{prefix}_sensitivity"),
                "Channel Sensitivity matches the locked recipe.",
                "Channel Sensitivity does not match the locked recipe.",
                item_f64(path, channel, tags::CHANNEL_SENSITIVITY)?,
                f64::from(channel_expected.sensitivity),
            );
            validate_waveform_code(
                &mut internal,
                path,
                channel,
                tags::CHANNEL_SENSITIVITY_UNITS_SEQUENCE,
                &format!("{prefix}_sensitivity_units"),
                channel_expected.sensitivity_units,
            )?;
            for (suffix, tag, locked) in [
                (
                    "sensitivity_correction_factor",
                    tags::CHANNEL_SENSITIVITY_CORRECTION_FACTOR,
                    f64::from(channel_expected.sensitivity_correction_factor),
                ),
                (
                    "baseline",
                    tags::CHANNEL_BASELINE,
                    f64::from(channel_expected.baseline),
                ),
            ] {
                check_equal(
                    &mut internal,
                    &format!("{prefix}_{suffix}"),
                    "Channel numeric metadata matches the locked recipe.",
                    "Channel numeric metadata does not match the locked recipe.",
                    item_f64(path, channel, tag)?,
                    locked,
                );
            }
            check(
                &mut internal,
                channel
                    .element_opt(tags::CHANNEL_TIME_SKEW)
                    .map_err(|err| validation_error(path, err))?
                    .is_some(),
                &format!("{prefix}_time_skew_present"),
                "Channel Time Skew is explicitly present.",
                "Channel Time Skew and Channel Sample Skew may not both be absent.",
            );
            fail_if_any_failed(path, &internal)?;
            check_equal(
                &mut internal,
                &format!("{prefix}_time_skew"),
                "Channel Time Skew matches the locked recipe.",
                "Channel Time Skew does not match the locked recipe.",
                item_f64(path, channel, tags::CHANNEL_TIME_SKEW)?,
                f64::from(channel_expected.time_skew_seconds),
            );
            check_equal(
                &mut internal,
                &format!("{prefix}_bits_stored"),
                "Waveform Bits Stored is 16 for every channel.",
                "Waveform Bits Stored is not 16 for every channel.",
                item_u16(path, channel, tags::WAVEFORM_BITS_STORED)?,
                u16::from(channel_expected.bits_stored),
            );
            check(
                &mut internal,
                channel
                    .element_opt(tags::CHANNEL_SAMPLE_SKEW)
                    .map_err(|err| validation_error(path, err))?
                    .is_none()
                    == channel_expected.sample_skew_absent,
                &format!("{prefix}_sample_skew_absent"),
                "Channel Sample Skew is absent while explicit Time Skew is zero.",
                "Channel Sample Skew is unexpectedly present.",
            );
        }

        let waveform_data = group
            .element(tags::WAVEFORM_DATA)
            .map_err(|err| validation_error(path, err))?;
        check_equal(
            &mut internal,
            &finding("waveform_data_vr"),
            "Waveform Data uses OW storage.",
            "Waveform Data does not use OW storage.",
            waveform_data.vr(),
            VR::OW,
        );
        let payload = waveform_data
            .to_bytes()
            .map_err(|err| validation_error(path, err))?;
        let payload_sha256 = sha256_hex(payload.as_ref());
        let arithmetic_length = usize::from(channel_count)
            * usize::try_from(sample_count).unwrap_or(usize::MAX)
            * usize::from(bits_allocated).div_ceil(8);
        check_equal(
            &mut internal,
            &finding("payload_byte_arithmetic"),
            "Waveform byte length equals channels times samples times bytes per signed sample.",
            "Waveform byte length does not match channel/sample/bit arithmetic.",
            payload.len(),
            arithmetic_length,
        );
        check_equal(
            &mut internal,
            &finding("payload_length"),
            "Waveform Data has the locked 12,000-byte length with no padding.",
            "Waveform Data length or value-field padding does not match the locked recipe.",
            payload.len(),
            usize::from(storage.payload_length_bytes)
                + usize::from(storage.value_field_padding_bytes),
        );
        check_equal(
            &mut internal,
            &finding("payload_sha256"),
            "Waveform payload hash matches the locked recipe.",
            "Waveform payload hash does not match the locked recipe.",
            payload_sha256.as_str(),
            storage.payload_sha256,
        );
        check(
            &mut internal,
            payload.len() % 2 == 0,
            &finding("signed_sample_width"),
            "Waveform payload is composed of complete signed 16-bit values.",
            "Waveform payload ends with a partial signed 16-bit value.",
        );
        let samples = payload
            .chunks_exact(2)
            .map(|pair| i16::from_le_bytes([pair[0], pair[1]]))
            .collect::<Vec<_>>();
        check_equal(
            &mut internal,
            &finding("sample_min"),
            "Decoded signed sample minimum matches the locked range.",
            "Decoded signed sample minimum does not match the locked range.",
            samples.iter().copied().min(),
            Some(storage.sample_min),
        );
        check_equal(
            &mut internal,
            &finding("sample_max"),
            "Decoded signed sample maximum matches the locked range.",
            "Decoded signed sample maximum does not match the locked range.",
            samples.iter().copied().max(),
            Some(storage.sample_max),
        );
        check_equal(
            &mut internal,
            &finding("formula_contract"),
            "Manifest sample formula is the locked deterministic formula.",
            "Manifest sample formula is not the locked deterministic formula.",
            storage.sample_value_formula,
            recipe.sample_formula_contract,
        );
        check_equal(
            &mut internal,
            &finding("interleave_contract"),
            "Manifest interleave is channel-then-sample.",
            "Manifest interleave is not channel-then-sample.",
            storage.interleave_order,
            "channel_then_sample",
        );
        check_equal(
            &mut internal,
            &finding("byte_order_contract"),
            "Manifest byte order is little endian.",
            "Manifest byte order is not little endian.",
            storage.byte_order,
            "little_endian",
        );
        let mut formula_matches =
            samples.len() == group_expected.channels.len() * sample_count as usize;
        let mut channel_bytes =
            vec![Vec::with_capacity(sample_count as usize * 2); group_expected.channels.len()];
        if formula_matches {
            for sample in 0..sample_count as usize {
                for (channel, bytes) in channel_bytes.iter_mut().enumerate() {
                    let value = samples[sample * group_expected.channels.len() + channel];
                    let expected_value = (recipe.sample_formula)(group_index, sample, channel);
                    formula_matches &= value == expected_value;
                    bytes.extend_from_slice(&value.to_le_bytes());
                }
            }
        }
        check(
            &mut internal,
            formula_matches,
            &finding("formula_and_interleave"),
            "Every signed sample matches the deterministic formula in channel-then-sample order.",
            "A sample differs from the formula or the waveform is not channel-then-sample interleaved.",
        );
        check_equal(
            &mut internal,
            &finding("channel_hash_count"),
            "Manifest contains one deinterleaved hash per channel.",
            "Manifest channel hash count does not match the channel count.",
            storage.channel_sha256.len(),
            group_expected.channels.len(),
        );
        fail_if_any_failed(path, &internal)?;
        for (index, (actual, locked)) in channel_bytes
            .iter()
            .map(|bytes| sha256_hex(bytes))
            .zip(storage.channel_sha256.iter())
            .enumerate()
        {
            check_equal(
                &mut internal,
                &if recipe.qualify_channel_findings_by_group {
                    format!(
                        "{}_group_{}_channel_{}_sha256",
                        recipe.finding_prefix,
                        expected_ordinal,
                        index + 1
                    )
                } else {
                    format!("{}_channel_{}_sha256", recipe.finding_prefix, index + 1)
                },
                "Deinterleaved channel hash matches the locked recipe.",
                "Deinterleaved channel hash does not match the locked recipe.",
                actual,
                (*locked).to_string(),
            );
        }

        check(
            &mut internal,
            group
                .element_opt(tags::WAVEFORM_PADDING_VALUE)
                .map_err(|err| validation_error(path, err))?
                .is_none()
                == storage.waveform_padding_value_absent,
            &finding("waveform_padding_absent"),
            "Waveform Padding Value is absent.",
            "Waveform Padding Value is unexpectedly present.",
        );
        actual_group_hashes.push(payload_sha256);
        aggregate_payload.extend_from_slice(payload.as_ref());
    }

    check_equal(
        &mut internal,
        &finding("aggregate_channel_count"),
        "Decoded group channel counts match the manifest aggregate.",
        "Decoded group channel counts do not match the manifest aggregate.",
        actual_total_channels,
        usize::from(aggregate_expected.total_channel_count),
    );
    check_equal(
        &mut internal,
        &finding("aggregate_payload_length"),
        "Concatenated ordered group payload length matches the manifest aggregate.",
        "Concatenated ordered group payload length does not match the manifest aggregate.",
        aggregate_payload.len(),
        usize::from(aggregate_expected.total_payload_length_bytes),
    );
    check(
        &mut internal,
        actual_group_hashes.len() == aggregate_expected.group_payload_sha256.len()
            && actual_group_hashes
                .iter()
                .map(String::as_str)
                .eq(aggregate_expected.group_payload_sha256.iter().copied()),
        &finding("aggregate_group_hashes"),
        "Actual payload hashes preserve manifest group order.",
        "Actual payload hashes are missing, reordered, or changed.",
    );
    check_equal(
        &mut internal,
        &finding("aggregate_payload_sha256"),
        "Concatenated ordered group payload hash matches the manifest aggregate.",
        "Concatenated ordered group payload hash does not match the manifest aggregate.",
        sha256_hex(&aggregate_payload),
        aggregate_expected.aggregate_payload_sha256.to_string(),
    );
    for (name, tag) in [
        ("waveform_annotation", tags::WAVEFORM_ANNOTATION_SEQUENCE),
        (
            "structured_waveform_annotation",
            tags::STRUCTURED_WAVEFORM_ANNOTATION_SEQUENCE,
        ),
        (
            "synchronization_frame_of_reference",
            tags::SYNCHRONIZATION_FRAME_OF_REFERENCE_UID,
        ),
        ("synchronization_trigger", tags::SYNCHRONIZATION_TRIGGER),
        ("synchronization_channel", tags::SYNCHRONIZATION_CHANNEL),
        (
            "acquisition_time_synchronized",
            tags::ACQUISITION_TIME_SYNCHRONIZED,
        ),
        ("time_source", tags::TIME_SOURCE),
        (
            "time_distribution_protocol",
            tags::TIME_DISTRIBUTION_PROTOCOL,
        ),
        ("ntp_source_address", tags::NTP_SOURCE_ADDRESS),
        ("referenced_study", tags::REFERENCED_STUDY_SEQUENCE),
        ("referenced_series", tags::REFERENCED_SERIES_SEQUENCE),
        ("referenced_waveform", tags::REFERENCED_WAVEFORM_SEQUENCE),
        ("referenced_image", tags::REFERENCED_IMAGE_SEQUENCE),
        ("referenced_instance", tags::REFERENCED_INSTANCE_SEQUENCE),
        ("source_image", tags::SOURCE_IMAGE_SEQUENCE),
        ("rows", tags::ROWS),
        ("columns", tags::COLUMNS),
        ("samples_per_pixel", tags::SAMPLES_PER_PIXEL),
        ("number_of_frames", tags::NUMBER_OF_FRAMES),
        (
            "photometric_interpretation",
            tags::PHOTOMETRIC_INTERPRETATION,
        ),
        ("bits_allocated", tags::BITS_ALLOCATED),
        ("bits_stored", tags::BITS_STORED),
        ("high_bit", tags::HIGH_BIT),
        ("pixel_representation", tags::PIXEL_REPRESENTATION),
        ("pixel_data", tags::PIXEL_DATA),
    ] {
        check(
            &mut internal,
            obj.element_opt(tag)
                .map_err(|err| validation_error(path, err))?
                .is_none(),
            &format!("{}_{name}_absent", recipe.finding_prefix),
            "Optional or forbidden content is absent.",
            "Optional or forbidden content is unexpectedly present.",
        );
    }

    fail_if_any_failed(path, &internal)?;
    Ok(ValidatedPart10 {
        bytes,
        validation: serde_json::json!({
            "status": "passed",
            "internal": internal,
            "standards": [
                {"name": standard_sop_class_validation_name(waveform.sop_class_uid), "status": "passed", "message": standard_sop_class_validation_message(waveform.sop_class_uid)},
                {"name": standard_transfer_syntax_validation_name(waveform.transfer_syntax_uid), "status": "passed", "message": standard_transfer_syntax_validation_message(waveform.transfer_syntax_uid)},
                {"name": recipe.module_validation_name, "status": "passed", "message": recipe.module_validation_message}
            ],
            "external": []
        }),
    })
}

fn validate_waveform_code(
    results: &mut Vec<Value>,
    path: &Path,
    item: &DatasetObject,
    sequence_tag: Tag,
    prefix: &str,
    expected: crate::waveform_manifest::ExpectedWaveformCode<'_>,
) -> Result<(), GenerateError> {
    check_equal(
        results,
        &format!("{prefix}_item_count"),
        "Code Sequence contains exactly one Item.",
        "Code Sequence does not contain exactly one Item.",
        item_sequence_item_count(path, item, sequence_tag)?,
        1,
    );
    let code = item_sequence_item(path, item, sequence_tag, 0)?;
    for (suffix, tag, locked) in [
        ("value", tags::CODE_VALUE, expected.code_value),
        (
            "scheme",
            tags::CODING_SCHEME_DESIGNATOR,
            expected.coding_scheme_designator,
        ),
        ("meaning", tags::CODE_MEANING, expected.code_meaning),
    ] {
        check_equal(
            results,
            &format!("{prefix}_{suffix}"),
            "Coded value matches the locked recipe.",
            "Coded value does not match the locked recipe.",
            item_str(path, code, tag)?.as_str(),
            locked,
        );
    }
    Ok(())
}

pub(crate) fn validate_real_world_value_mapping_file(
    path: &Path,
    expected: &RealWorldValueMappingExpectations<'_>,
) -> Result<ValidatedPart10, GenerateError> {
    let bytes = fs::read(path).map_err(|source| GenerateError::ReadGeneratedFile {
        path: path.to_path_buf(),
        source,
    })?;
    let obj = open_file(path).map_err(|err| GenerateError::ValidateDicomFile {
        path: path.to_path_buf(),
        message: err.to_string(),
    })?;

    let mut internal = Vec::new();
    check(
        &mut internal,
        bytes.len() >= 132 && &bytes[128..132] == b"DICM",
        "part10_preamble",
        "File has a 128-byte preamble followed by the DICM marker.",
        "File is missing the Part 10 DICM marker at byte offset 128.",
    );
    check_equal(
        &mut internal,
        "file_meta_transfer_syntax",
        "File Meta Information Transfer Syntax UID matches the recipe.",
        "File Meta Information Transfer Syntax UID does not match the recipe.",
        trim_uid(obj.meta().transfer_syntax()).as_str(),
        expected.transfer_syntax_uid,
    );

    let dataset_sop_class = element_str(path, &obj, tags::SOP_CLASS_UID)?;
    check_equal(
        &mut internal,
        "sop_class_uid_consistency",
        "Dataset SOP Class UID, File Meta SOP Class UID, and recipe SOP Class UID match.",
        "SOP Class UID differs between dataset, File Meta Information, or recipe.",
        dataset_sop_class.as_str(),
        expected.sop_class_uid,
    );
    check_equal(
        &mut internal,
        "media_storage_sop_class_uid",
        "File Meta SOP Class UID matches the dataset SOP Class UID.",
        "File Meta SOP Class UID does not match the dataset SOP Class UID.",
        trim_uid(obj.meta().media_storage_sop_class_uid()).as_str(),
        dataset_sop_class.as_str(),
    );
    let dataset_sop_instance = element_str(path, &obj, tags::SOP_INSTANCE_UID)?;
    check_equal(
        &mut internal,
        "sop_instance_uid_consistency",
        "Dataset SOP Instance UID, File Meta SOP Instance UID, and manifest UID match.",
        "SOP Instance UID differs between dataset, File Meta Information, or manifest.",
        dataset_sop_instance.as_str(),
        expected.sop_instance_uid,
    );
    check_equal(
        &mut internal,
        "media_storage_sop_instance_uid",
        "File Meta SOP Instance UID matches the dataset SOP Instance UID.",
        "File Meta SOP Instance UID does not match the dataset SOP Instance UID.",
        trim_uid(obj.meta().media_storage_sop_instance_uid()).as_str(),
        dataset_sop_instance.as_str(),
    );
    check_equal(
        &mut internal,
        "implementation_class_uid",
        "File Meta Implementation Class UID matches the deterministic generator UID.",
        "File Meta Implementation Class UID does not match the deterministic generator UID.",
        trim_uid(obj.meta().implementation_class_uid()).as_str(),
        expected.implementation_class_uid,
    );
    check_equal(
        &mut internal,
        "synthetic_data",
        "Synthetic Data is present and set to YES.",
        "Synthetic Data is missing or not set to YES.",
        element_str(path, &obj, tags::SYNTHETIC_DATA)?.as_str(),
        expected.synthetic_data,
    );
    check_equal(
        &mut internal,
        "rwvm_modality",
        "Real World Value Mapping Series Modality is RWV.",
        "Real World Value Mapping Series Modality does not match the recipe.",
        element_str(path, &obj, tags::MODALITY)?.as_str(),
        expected.modality,
    );
    check_equal(
        &mut internal,
        "rwvm_content_label",
        "Content Label matches the RWVM recipe.",
        "Content Label does not match the RWVM recipe.",
        element_str(path, &obj, TAG_CONTENT_LABEL)?.as_str(),
        expected.content_label,
    );

    let mapping = top_level_sequence_item(path, &obj, tags::REAL_WORLD_VALUE_MAPPING_SEQUENCE, 0)?;
    check_equal(
        &mut internal,
        "rwvm_lut_label",
        "RWVM LUT Label matches the recipe.",
        "RWVM LUT Label does not match the recipe.",
        item_str(path, mapping, tags::LUT_LABEL)?.as_str(),
        expected.lut_label,
    );
    check_equal(
        &mut internal,
        "rwvm_first_value_mapped",
        "RWVM first mapped stored value matches the recipe.",
        "RWVM first mapped stored value does not match the recipe.",
        item_u16(path, mapping, tags::REAL_WORLD_VALUE_FIRST_VALUE_MAPPED)?,
        expected.first_value_mapped,
    );
    check_equal(
        &mut internal,
        "rwvm_last_value_mapped",
        "RWVM last mapped stored value matches the recipe.",
        "RWVM last mapped stored value does not match the recipe.",
        item_u16(path, mapping, tags::REAL_WORLD_VALUE_LAST_VALUE_MAPPED)?,
        expected.last_value_mapped,
    );
    check_equal(
        &mut internal,
        "rwvm_intercept",
        "RWVM intercept matches the recipe.",
        "RWVM intercept does not match the recipe.",
        item_f64(path, mapping, tags::REAL_WORLD_VALUE_INTERCEPT)?,
        expected.intercept,
    );
    check_equal(
        &mut internal,
        "rwvm_slope",
        "RWVM slope matches the recipe.",
        "RWVM slope does not match the recipe.",
        item_f64(path, mapping, tags::REAL_WORLD_VALUE_SLOPE)?,
        expected.slope,
    );

    let units = item_sequence_item(path, mapping, tags::MEASUREMENT_UNITS_CODE_SEQUENCE, 0)?;
    check_equal(
        &mut internal,
        "rwvm_measurement_units_code_value",
        "RWVM units Code Value matches the recipe.",
        "RWVM units Code Value does not match the recipe.",
        item_str(path, units, tags::CODE_VALUE)?.as_str(),
        expected.unit_code_value,
    );
    check_equal(
        &mut internal,
        "rwvm_measurement_units_coding_scheme",
        "RWVM units Coding Scheme Designator matches the recipe.",
        "RWVM units Coding Scheme Designator does not match the recipe.",
        item_str(path, units, tags::CODING_SCHEME_DESIGNATOR)?.as_str(),
        expected.unit_coding_scheme_designator,
    );
    check_equal(
        &mut internal,
        "rwvm_measurement_units_code_meaning",
        "RWVM units Code Meaning matches the recipe.",
        "RWVM units Code Meaning does not match the recipe.",
        item_str(path, units, tags::CODE_MEANING)?.as_str(),
        expected.unit_code_meaning,
    );

    let referenced_image = item_sequence_item(path, mapping, TAG_REFERENCED_IMAGE_SEQUENCE, 0)?;
    check_equal(
        &mut internal,
        "rwvm_referenced_sop_class_uid",
        "RWVM reference SOP Class UID matches the source image.",
        "RWVM reference SOP Class UID does not match the source image.",
        item_str(path, referenced_image, TAG_REFERENCED_SOP_CLASS_UID)?.as_str(),
        expected.referenced_sop_class_uid,
    );
    check_equal(
        &mut internal,
        "rwvm_referenced_sop_instance_uid",
        "RWVM reference SOP Instance UID matches the source image.",
        "RWVM reference SOP Instance UID does not match the source image.",
        item_str(path, referenced_image, TAG_REFERENCED_SOP_INSTANCE_UID)?.as_str(),
        expected.referenced_sop_instance_uid,
    );
    check_equal(
        &mut internal,
        "rwvm_referenced_frame_numbers",
        "RWVM referenced frame numbers match the recipe.",
        "RWVM referenced frame numbers do not match the recipe.",
        item_i32_values(path, referenced_image, TAG_REFERENCED_FRAME_NUMBER)?,
        expected
            .referenced_frame_numbers
            .iter()
            .map(|frame| i32::from(*frame))
            .collect::<Vec<_>>(),
    );

    let referenced_series = top_level_sequence_item(path, &obj, TAG_REFERENCED_SERIES_SEQUENCE, 0)?;
    check_equal(
        &mut internal,
        "rwvm_referenced_series_uid",
        "Common Instance Reference points to the source Series Instance UID.",
        "Common Instance Reference Series Instance UID does not match the source.",
        item_str(path, referenced_series, tags::SERIES_INSTANCE_UID)?.as_str(),
        expected.referenced_series_instance_uid,
    );
    let referenced_instance =
        item_sequence_item(path, referenced_series, TAG_REFERENCED_INSTANCE_SEQUENCE, 0)?;
    check_equal(
        &mut internal,
        "rwvm_common_reference_sop_class_uid",
        "Common Instance Reference SOP Class UID matches the source image.",
        "Common Instance Reference SOP Class UID does not match the source image.",
        item_str(path, referenced_instance, TAG_REFERENCED_SOP_CLASS_UID)?.as_str(),
        expected.referenced_sop_class_uid,
    );
    check_equal(
        &mut internal,
        "rwvm_common_reference_sop_instance_uid",
        "Common Instance Reference SOP Instance UID matches the source image.",
        "Common Instance Reference SOP Instance UID does not match the source image.",
        item_str(path, referenced_instance, TAG_REFERENCED_SOP_INSTANCE_UID)?.as_str(),
        expected.referenced_sop_instance_uid,
    );
    check(
        &mut internal,
        obj.element_opt(tags::PIXEL_DATA)
            .map_err(|err| validation_error(path, err))?
            .is_none(),
        "rwvm_pixel_data_absent",
        "Real World Value Mapping contains no Pixel Data.",
        "Real World Value Mapping unexpectedly contains Pixel Data.",
    );

    fail_if_any_failed(path, &internal)?;

    Ok(ValidatedPart10 {
        bytes,
        validation: serde_json::json!({
            "status": "passed",
            "internal": internal,
            "standards": [
                {
                    "name": standard_sop_class_validation_name(expected.sop_class_uid),
                    "status": "passed",
                    "message": standard_sop_class_validation_message(expected.sop_class_uid)
                },
                {
                    "name": standard_transfer_syntax_validation_name(expected.transfer_syntax_uid),
                    "status": "passed",
                    "message": standard_transfer_syntax_validation_message(expected.transfer_syntax_uid)
                },
                {
                    "name": "synthetic_data_attribute",
                    "status": "passed",
                    "message": "Synthetic Data (0008,001C) is present with value YES."
                },
                {
                    "name": "real_world_value_mapping_modules",
                    "status": "passed",
                    "message": "RWVM mapping sequence, units, and references match the recipe."
                }
            ],
            "external": []
        }),
    })
}

pub(crate) fn validate_basic_text_sr_file(
    path: &Path,
    expected: &BasicTextSrExpectations<'_>,
) -> Result<ValidatedPart10, GenerateError> {
    let bytes = fs::read(path).map_err(|source| GenerateError::ReadGeneratedFile {
        path: path.to_path_buf(),
        source,
    })?;
    let obj = open_file(path).map_err(|err| GenerateError::ValidateDicomFile {
        path: path.to_path_buf(),
        message: err.to_string(),
    })?;

    let mut internal = Vec::new();
    check(
        &mut internal,
        bytes.len() >= 132 && &bytes[128..132] == b"DICM",
        "part10_preamble",
        "File has a 128-byte preamble followed by the DICM marker.",
        "File is missing the Part 10 DICM marker at byte offset 128.",
    );
    check_equal(
        &mut internal,
        "file_meta_transfer_syntax",
        "File Meta Information Transfer Syntax UID matches the recipe.",
        "File Meta Information Transfer Syntax UID does not match the recipe.",
        trim_uid(obj.meta().transfer_syntax()).as_str(),
        expected.transfer_syntax_uid,
    );

    let dataset_sop_class = element_str(path, &obj, tags::SOP_CLASS_UID)?;
    check_equal(
        &mut internal,
        "sop_class_uid_consistency",
        "Dataset SOP Class UID, File Meta SOP Class UID, and recipe SOP Class UID match.",
        "SOP Class UID differs between dataset, File Meta Information, or recipe.",
        dataset_sop_class.as_str(),
        expected.sop_class_uid,
    );
    check_equal(
        &mut internal,
        "media_storage_sop_class_uid",
        "File Meta SOP Class UID matches the dataset SOP Class UID.",
        "File Meta SOP Class UID does not match the dataset SOP Class UID.",
        trim_uid(obj.meta().media_storage_sop_class_uid()).as_str(),
        dataset_sop_class.as_str(),
    );
    let dataset_sop_instance = element_str(path, &obj, tags::SOP_INSTANCE_UID)?;
    check_equal(
        &mut internal,
        "sop_instance_uid_consistency",
        "Dataset SOP Instance UID, File Meta SOP Instance UID, and manifest UID match.",
        "SOP Instance UID differs between dataset, File Meta Information, or manifest.",
        dataset_sop_instance.as_str(),
        expected.sop_instance_uid,
    );
    check_equal(
        &mut internal,
        "media_storage_sop_instance_uid",
        "File Meta SOP Instance UID matches the dataset SOP Instance UID.",
        "File Meta SOP Instance UID does not match the dataset SOP Instance UID.",
        trim_uid(obj.meta().media_storage_sop_instance_uid()).as_str(),
        dataset_sop_instance.as_str(),
    );
    check_equal(
        &mut internal,
        "implementation_class_uid",
        "File Meta Implementation Class UID matches the deterministic generator UID.",
        "File Meta Implementation Class UID does not match the deterministic generator UID.",
        trim_uid(obj.meta().implementation_class_uid()).as_str(),
        expected.implementation_class_uid,
    );
    check_equal(
        &mut internal,
        "synthetic_data",
        "Synthetic Data is present and set to YES.",
        "Synthetic Data is missing or not set to YES.",
        element_str(path, &obj, tags::SYNTHETIC_DATA)?.as_str(),
        expected.synthetic_data,
    );
    check_equal(
        &mut internal,
        "sr_modality",
        "SR Document Series Modality is SR.",
        "SR Document Series Modality does not match the recipe.",
        element_str(path, &obj, tags::MODALITY)?.as_str(),
        expected.modality,
    );
    check_equal(
        &mut internal,
        "sr_completion_flag",
        "SR Completion Flag matches the recipe.",
        "SR Completion Flag does not match the recipe.",
        element_str(path, &obj, tags::COMPLETION_FLAG)?.as_str(),
        expected.completion_flag,
    );
    check_equal(
        &mut internal,
        "sr_verification_flag",
        "SR Verification Flag matches the recipe.",
        "SR Verification Flag does not match the recipe.",
        element_str(path, &obj, tags::VERIFICATION_FLAG)?.as_str(),
        expected.verification_flag,
    );

    let evidence = top_level_sequence_item(
        path,
        &obj,
        tags::CURRENT_REQUESTED_PROCEDURE_EVIDENCE_SEQUENCE,
        0,
    )?;
    check_equal(
        &mut internal,
        "sr_evidence_study_instance_uid",
        "SR evidence references the source Study Instance UID.",
        "SR evidence Study Instance UID does not match the source.",
        item_str(path, evidence, tags::STUDY_INSTANCE_UID)?.as_str(),
        expected.referenced_study_instance_uid,
    );
    let evidence_series = item_sequence_item(path, evidence, tags::REFERENCED_SERIES_SEQUENCE, 0)?;
    check_equal(
        &mut internal,
        "sr_evidence_series_instance_uid",
        "SR evidence references the source Series Instance UID.",
        "SR evidence Series Instance UID does not match the source.",
        item_str(path, evidence_series, tags::SERIES_INSTANCE_UID)?.as_str(),
        expected.referenced_series_instance_uid,
    );
    let evidence_sop = item_sequence_item(path, evidence_series, tags::REFERENCED_SOP_SEQUENCE, 0)?;
    check_equal(
        &mut internal,
        "sr_evidence_sop_class_uid",
        "SR evidence SOP Class UID matches the source image.",
        "SR evidence SOP Class UID does not match the source image.",
        item_str(path, evidence_sop, TAG_REFERENCED_SOP_CLASS_UID)?.as_str(),
        expected.referenced_sop_class_uid,
    );
    check_equal(
        &mut internal,
        "sr_evidence_sop_instance_uid",
        "SR evidence SOP Instance UID matches the source image.",
        "SR evidence SOP Instance UID does not match the source image.",
        item_str(path, evidence_sop, TAG_REFERENCED_SOP_INSTANCE_UID)?.as_str(),
        expected.referenced_sop_instance_uid,
    );

    check_equal(
        &mut internal,
        "sr_root_value_type",
        "SR root content item Value Type is CONTAINER.",
        "SR root content item Value Type does not match the recipe.",
        element_str(path, &obj, tags::VALUE_TYPE)?.as_str(),
        expected.root_value_type,
    );
    check_equal(
        &mut internal,
        "sr_root_continuity_of_content",
        "SR root Continuity of Content matches the recipe.",
        "SR root Continuity of Content does not match the recipe.",
        element_str(path, &obj, tags::CONTINUITY_OF_CONTENT)?.as_str(),
        expected.root_continuity_of_content,
    );
    let title = top_level_sequence_item(path, &obj, tags::CONCEPT_NAME_CODE_SEQUENCE, 0)?;
    check_equal(
        &mut internal,
        "sr_title_code_value",
        "SR title Code Value matches the recipe.",
        "SR title Code Value does not match the recipe.",
        item_str(path, title, tags::CODE_VALUE)?.as_str(),
        expected.title_code_value,
    );
    check_equal(
        &mut internal,
        "sr_title_coding_scheme",
        "SR title Coding Scheme Designator matches the recipe.",
        "SR title Coding Scheme Designator does not match the recipe.",
        item_str(path, title, tags::CODING_SCHEME_DESIGNATOR)?.as_str(),
        expected.title_coding_scheme_designator,
    );
    check_equal(
        &mut internal,
        "sr_title_code_meaning",
        "SR title Code Meaning matches the recipe.",
        "SR title Code Meaning does not match the recipe.",
        item_str(path, title, tags::CODE_MEANING)?.as_str(),
        expected.title_code_meaning,
    );
    check_equal(
        &mut internal,
        "sr_content_sequence_items",
        "SR root Content Sequence contains one observation item.",
        "SR root Content Sequence item count does not match the recipe.",
        sequence_item_count(path, &obj, tags::CONTENT_SEQUENCE)?,
        1,
    );

    let observation = top_level_sequence_item(path, &obj, tags::CONTENT_SEQUENCE, 0)?;
    check_equal(
        &mut internal,
        "sr_observation_relationship_type",
        "SR observation Relationship Type matches the recipe.",
        "SR observation Relationship Type does not match the recipe.",
        item_str(path, observation, tags::RELATIONSHIP_TYPE)?.as_str(),
        expected.observation_relationship_type,
    );
    check_equal(
        &mut internal,
        "sr_observation_value_type",
        "SR observation Value Type is TEXT.",
        "SR observation Value Type does not match the recipe.",
        item_str(path, observation, tags::VALUE_TYPE)?.as_str(),
        expected.observation_value_type,
    );
    let observation_code =
        item_sequence_item(path, observation, tags::CONCEPT_NAME_CODE_SEQUENCE, 0)?;
    check_equal(
        &mut internal,
        "sr_observation_code_value",
        "SR observation Code Value matches the recipe.",
        "SR observation Code Value does not match the recipe.",
        item_str(path, observation_code, tags::CODE_VALUE)?.as_str(),
        expected.observation_code_value,
    );
    check_equal(
        &mut internal,
        "sr_observation_coding_scheme",
        "SR observation Coding Scheme Designator matches the recipe.",
        "SR observation Coding Scheme Designator does not match the recipe.",
        item_str(path, observation_code, tags::CODING_SCHEME_DESIGNATOR)?.as_str(),
        expected.observation_coding_scheme_designator,
    );
    check_equal(
        &mut internal,
        "sr_observation_code_meaning",
        "SR observation Code Meaning matches the recipe.",
        "SR observation Code Meaning does not match the recipe.",
        item_str(path, observation_code, tags::CODE_MEANING)?.as_str(),
        expected.observation_code_meaning,
    );
    check_equal(
        &mut internal,
        "sr_observation_text",
        "SR observation Text Value matches the recipe.",
        "SR observation Text Value does not match the recipe.",
        item_str(path, observation, tags::TEXT_VALUE)?.as_str(),
        expected.observation_text,
    );
    check(
        &mut internal,
        obj.element_opt(tags::PIXEL_DATA)
            .map_err(|err| validation_error(path, err))?
            .is_none(),
        "sr_pixel_data_absent",
        "Basic Text SR contains no Pixel Data.",
        "Basic Text SR unexpectedly contains Pixel Data.",
    );

    fail_if_any_failed(path, &internal)?;

    Ok(ValidatedPart10 {
        bytes,
        validation: serde_json::json!({
            "status": "passed",
            "internal": internal,
            "standards": [
                {
                    "name": standard_sop_class_validation_name(expected.sop_class_uid),
                    "status": "passed",
                    "message": standard_sop_class_validation_message(expected.sop_class_uid)
                },
                {
                    "name": standard_transfer_syntax_validation_name(expected.transfer_syntax_uid),
                    "status": "passed",
                    "message": standard_transfer_syntax_validation_message(expected.transfer_syntax_uid)
                },
                {
                    "name": "synthetic_data_attribute",
                    "status": "passed",
                    "message": "Synthetic Data (0008,001C) is present with value YES."
                },
                {
                    "name": "basic_text_sr_modules",
                    "status": "passed",
                    "message": "Basic Text SR document flags, evidence, and content tree match the recipe."
                }
            ],
            "external": []
        }),
    })
}

pub(crate) fn validate_comprehensive_sr_file(
    path: &Path,
    expected: &ComprehensiveSrExpectations<'_>,
) -> Result<ValidatedPart10, GenerateError> {
    let bytes = fs::read(path).map_err(|source| GenerateError::ReadGeneratedFile {
        path: path.to_path_buf(),
        source,
    })?;
    let obj = open_file(path).map_err(|err| GenerateError::ValidateDicomFile {
        path: path.to_path_buf(),
        message: err.to_string(),
    })?;

    let mut internal = Vec::new();
    check(
        &mut internal,
        bytes.len() >= 132 && &bytes[128..132] == b"DICM",
        "part10_preamble",
        "File has a 128-byte preamble followed by the DICM marker.",
        "File is missing the Part 10 DICM marker at byte offset 128.",
    );
    check_equal(
        &mut internal,
        "file_meta_transfer_syntax",
        "File Meta Information Transfer Syntax UID matches the recipe.",
        "File Meta Information Transfer Syntax UID does not match the recipe.",
        trim_uid(obj.meta().transfer_syntax()).as_str(),
        expected.transfer_syntax_uid,
    );

    let dataset_sop_class = element_str(path, &obj, tags::SOP_CLASS_UID)?;
    check_equal(
        &mut internal,
        "sop_class_uid_consistency",
        "Dataset SOP Class UID, File Meta SOP Class UID, and recipe SOP Class UID match.",
        "SOP Class UID differs between dataset, File Meta Information, or recipe.",
        dataset_sop_class.as_str(),
        expected.sop_class_uid,
    );
    check_equal(
        &mut internal,
        "media_storage_sop_class_uid",
        "File Meta SOP Class UID matches the dataset SOP Class UID.",
        "File Meta SOP Class UID does not match the dataset SOP Class UID.",
        trim_uid(obj.meta().media_storage_sop_class_uid()).as_str(),
        dataset_sop_class.as_str(),
    );
    let dataset_sop_instance = element_str(path, &obj, tags::SOP_INSTANCE_UID)?;
    check_equal(
        &mut internal,
        "sop_instance_uid_consistency",
        "Dataset SOP Instance UID, File Meta SOP Instance UID, and manifest UID match.",
        "SOP Instance UID differs between dataset, File Meta Information, or manifest.",
        dataset_sop_instance.as_str(),
        expected.sop_instance_uid,
    );
    check_equal(
        &mut internal,
        "media_storage_sop_instance_uid",
        "File Meta SOP Instance UID matches the dataset SOP Instance UID.",
        "File Meta SOP Instance UID does not match the dataset SOP Instance UID.",
        trim_uid(obj.meta().media_storage_sop_instance_uid()).as_str(),
        dataset_sop_instance.as_str(),
    );
    check_equal(
        &mut internal,
        "implementation_class_uid",
        "File Meta Implementation Class UID matches the deterministic generator UID.",
        "File Meta Implementation Class UID does not match the deterministic generator UID.",
        trim_uid(obj.meta().implementation_class_uid()).as_str(),
        expected.implementation_class_uid,
    );
    check_equal(
        &mut internal,
        "synthetic_data",
        "Synthetic Data is present and set to YES.",
        "Synthetic Data is missing or not set to YES.",
        element_str(path, &obj, tags::SYNTHETIC_DATA)?.as_str(),
        expected.synthetic_data,
    );
    check_equal(
        &mut internal,
        "sr_modality",
        "SR Document Series Modality is SR.",
        "SR Document Series Modality does not match the recipe.",
        element_str(path, &obj, tags::MODALITY)?.as_str(),
        expected.modality,
    );
    check_equal(
        &mut internal,
        "sr_completion_flag",
        "SR Completion Flag matches the recipe.",
        "SR Completion Flag does not match the recipe.",
        element_str(path, &obj, tags::COMPLETION_FLAG)?.as_str(),
        expected.completion_flag,
    );
    check_equal(
        &mut internal,
        "sr_verification_flag",
        "SR Verification Flag matches the recipe.",
        "SR Verification Flag does not match the recipe.",
        element_str(path, &obj, tags::VERIFICATION_FLAG)?.as_str(),
        expected.verification_flag,
    );

    let evidence = top_level_sequence_item(
        path,
        &obj,
        tags::CURRENT_REQUESTED_PROCEDURE_EVIDENCE_SEQUENCE,
        0,
    )?;
    check_equal(
        &mut internal,
        "sr_evidence_study_instance_uid",
        "SR evidence references the source Study Instance UID.",
        "SR evidence Study Instance UID does not match the source.",
        item_str(path, evidence, tags::STUDY_INSTANCE_UID)?.as_str(),
        expected.referenced_study_instance_uid,
    );
    let evidence_series = item_sequence_item(path, evidence, tags::REFERENCED_SERIES_SEQUENCE, 0)?;
    check_equal(
        &mut internal,
        "sr_evidence_series_instance_uid",
        "SR evidence references the source Series Instance UID.",
        "SR evidence Series Instance UID does not match the source.",
        item_str(path, evidence_series, tags::SERIES_INSTANCE_UID)?.as_str(),
        expected.referenced_series_instance_uid,
    );
    let evidence_sop = item_sequence_item(path, evidence_series, tags::REFERENCED_SOP_SEQUENCE, 0)?;
    check_equal(
        &mut internal,
        "sr_evidence_sop_class_uid",
        "SR evidence SOP Class UID matches the source image.",
        "SR evidence SOP Class UID does not match the source image.",
        item_str(path, evidence_sop, TAG_REFERENCED_SOP_CLASS_UID)?.as_str(),
        expected.referenced_sop_class_uid,
    );
    check_equal(
        &mut internal,
        "sr_evidence_sop_instance_uid",
        "SR evidence SOP Instance UID matches the source image.",
        "SR evidence SOP Instance UID does not match the source image.",
        item_str(path, evidence_sop, TAG_REFERENCED_SOP_INSTANCE_UID)?.as_str(),
        expected.referenced_sop_instance_uid,
    );

    check_equal(
        &mut internal,
        "sr_root_value_type",
        "SR root content item Value Type is CONTAINER.",
        "SR root content item Value Type does not match the recipe.",
        element_str(path, &obj, tags::VALUE_TYPE)?.as_str(),
        expected.root_value_type,
    );
    check_equal(
        &mut internal,
        "sr_root_continuity_of_content",
        "SR root Continuity of Content matches the recipe.",
        "SR root Continuity of Content does not match the recipe.",
        element_str(path, &obj, tags::CONTINUITY_OF_CONTENT)?.as_str(),
        expected.root_continuity_of_content,
    );
    let title = top_level_sequence_item(path, &obj, tags::CONCEPT_NAME_CODE_SEQUENCE, 0)?;
    check_equal(
        &mut internal,
        "sr_title_code_value",
        "SR title Code Value matches the recipe.",
        "SR title Code Value does not match the recipe.",
        item_str(path, title, tags::CODE_VALUE)?.as_str(),
        expected.title_code_value,
    );
    check_equal(
        &mut internal,
        "sr_title_coding_scheme",
        "SR title Coding Scheme Designator matches the recipe.",
        "SR title Coding Scheme Designator does not match the recipe.",
        item_str(path, title, tags::CODING_SCHEME_DESIGNATOR)?.as_str(),
        expected.title_coding_scheme_designator,
    );
    check_equal(
        &mut internal,
        "sr_title_code_meaning",
        "SR title Code Meaning matches the recipe.",
        "SR title Code Meaning does not match the recipe.",
        item_str(path, title, tags::CODE_MEANING)?.as_str(),
        expected.title_code_meaning,
    );
    check_equal(
        &mut internal,
        "sr_content_sequence_items",
        "SR root Content Sequence contains the measurement and image reference items.",
        "SR root Content Sequence item count does not match the recipe.",
        sequence_item_count(path, &obj, tags::CONTENT_SEQUENCE)?,
        2,
    );

    let measurement = top_level_sequence_item(path, &obj, tags::CONTENT_SEQUENCE, 0)?;
    check_equal(
        &mut internal,
        "sr_measurement_relationship_type",
        "SR measurement Relationship Type matches the recipe.",
        "SR measurement Relationship Type does not match the recipe.",
        item_str(path, measurement, tags::RELATIONSHIP_TYPE)?.as_str(),
        expected.measurement_relationship_type,
    );
    check_equal(
        &mut internal,
        "sr_measurement_value_type",
        "SR measurement Value Type is NUM.",
        "SR measurement Value Type does not match the recipe.",
        item_str(path, measurement, tags::VALUE_TYPE)?.as_str(),
        expected.measurement_value_type,
    );
    let measurement_code =
        item_sequence_item(path, measurement, tags::CONCEPT_NAME_CODE_SEQUENCE, 0)?;
    check_equal(
        &mut internal,
        "sr_measurement_code_value",
        "SR measurement Code Value matches the recipe.",
        "SR measurement Code Value does not match the recipe.",
        item_str(path, measurement_code, tags::CODE_VALUE)?.as_str(),
        expected.measurement_code_value,
    );
    check_equal(
        &mut internal,
        "sr_measurement_coding_scheme",
        "SR measurement Coding Scheme Designator matches the recipe.",
        "SR measurement Coding Scheme Designator does not match the recipe.",
        item_str(path, measurement_code, tags::CODING_SCHEME_DESIGNATOR)?.as_str(),
        expected.measurement_coding_scheme_designator,
    );
    check_equal(
        &mut internal,
        "sr_measurement_code_meaning",
        "SR measurement Code Meaning matches the recipe.",
        "SR measurement Code Meaning does not match the recipe.",
        item_str(path, measurement_code, tags::CODE_MEANING)?.as_str(),
        expected.measurement_code_meaning,
    );
    let measured_value = item_sequence_item(path, measurement, tags::MEASURED_VALUE_SEQUENCE, 0)?;
    check_equal(
        &mut internal,
        "sr_measurement_numeric_value",
        "SR measurement Numeric Value matches the recipe.",
        "SR measurement Numeric Value does not match the recipe.",
        item_str(path, measured_value, tags::NUMERIC_VALUE)?.as_str(),
        expected.numeric_value,
    );
    let units = item_sequence_item(
        path,
        measured_value,
        tags::MEASUREMENT_UNITS_CODE_SEQUENCE,
        0,
    )?;
    check_equal(
        &mut internal,
        "sr_measurement_unit_code_value",
        "SR measurement unit Code Value matches the recipe.",
        "SR measurement unit Code Value does not match the recipe.",
        item_str(path, units, tags::CODE_VALUE)?.as_str(),
        expected.unit_code_value,
    );
    check_equal(
        &mut internal,
        "sr_measurement_unit_coding_scheme",
        "SR measurement unit Coding Scheme Designator matches the recipe.",
        "SR measurement unit Coding Scheme Designator does not match the recipe.",
        item_str(path, units, tags::CODING_SCHEME_DESIGNATOR)?.as_str(),
        expected.unit_coding_scheme_designator,
    );
    check_equal(
        &mut internal,
        "sr_measurement_unit_code_meaning",
        "SR measurement unit Code Meaning matches the recipe.",
        "SR measurement unit Code Meaning does not match the recipe.",
        item_str(path, units, tags::CODE_MEANING)?.as_str(),
        expected.unit_code_meaning,
    );

    let image = top_level_sequence_item(path, &obj, tags::CONTENT_SEQUENCE, 1)?;
    check_equal(
        &mut internal,
        "sr_image_relationship_type",
        "SR image reference Relationship Type matches the recipe.",
        "SR image reference Relationship Type does not match the recipe.",
        item_str(path, image, tags::RELATIONSHIP_TYPE)?.as_str(),
        expected.image_relationship_type,
    );
    check_equal(
        &mut internal,
        "sr_image_value_type",
        "SR image reference Value Type is IMAGE.",
        "SR image reference Value Type does not match the recipe.",
        item_str(path, image, tags::VALUE_TYPE)?.as_str(),
        expected.image_value_type,
    );
    let image_code = item_sequence_item(path, image, tags::CONCEPT_NAME_CODE_SEQUENCE, 0)?;
    check_equal(
        &mut internal,
        "sr_image_code_value",
        "SR image reference Code Value matches the recipe.",
        "SR image reference Code Value does not match the recipe.",
        item_str(path, image_code, tags::CODE_VALUE)?.as_str(),
        expected.image_code_value,
    );
    check_equal(
        &mut internal,
        "sr_image_coding_scheme",
        "SR image reference Coding Scheme Designator matches the recipe.",
        "SR image reference Coding Scheme Designator does not match the recipe.",
        item_str(path, image_code, tags::CODING_SCHEME_DESIGNATOR)?.as_str(),
        expected.image_coding_scheme_designator,
    );
    check_equal(
        &mut internal,
        "sr_image_code_meaning",
        "SR image reference Code Meaning matches the recipe.",
        "SR image reference Code Meaning does not match the recipe.",
        item_str(path, image_code, tags::CODE_MEANING)?.as_str(),
        expected.image_code_meaning,
    );
    let image_sop = item_sequence_item(path, image, tags::REFERENCED_SOP_SEQUENCE, 0)?;
    check_equal(
        &mut internal,
        "sr_image_sop_class_uid",
        "SR image reference SOP Class UID matches the source image.",
        "SR image reference SOP Class UID does not match the source image.",
        item_str(path, image_sop, TAG_REFERENCED_SOP_CLASS_UID)?.as_str(),
        expected.referenced_sop_class_uid,
    );
    check_equal(
        &mut internal,
        "sr_image_sop_instance_uid",
        "SR image reference SOP Instance UID matches the source image.",
        "SR image reference SOP Instance UID does not match the source image.",
        item_str(path, image_sop, TAG_REFERENCED_SOP_INSTANCE_UID)?.as_str(),
        expected.referenced_sop_instance_uid,
    );
    let expected_frame_numbers = expected
        .referenced_frame_numbers
        .iter()
        .map(u16::to_string)
        .collect::<Vec<_>>()
        .join("\\");
    check_equal(
        &mut internal,
        "sr_image_referenced_frame_numbers",
        "SR image reference frame numbers match the recipe.",
        "SR image reference frame numbers do not match the recipe.",
        item_str(path, image_sop, TAG_REFERENCED_FRAME_NUMBER)?.as_str(),
        expected_frame_numbers.as_str(),
    );
    check(
        &mut internal,
        obj.element_opt(tags::PIXEL_DATA)
            .map_err(|err| validation_error(path, err))?
            .is_none(),
        "sr_pixel_data_absent",
        "Comprehensive SR contains no Pixel Data.",
        "Comprehensive SR unexpectedly contains Pixel Data.",
    );

    fail_if_any_failed(path, &internal)?;

    Ok(ValidatedPart10 {
        bytes,
        validation: serde_json::json!({
            "status": "passed",
            "internal": internal,
            "standards": [
                {
                    "name": standard_sop_class_validation_name(expected.sop_class_uid),
                    "status": "passed",
                    "message": standard_sop_class_validation_message(expected.sop_class_uid)
                },
                {
                    "name": standard_transfer_syntax_validation_name(expected.transfer_syntax_uid),
                    "status": "passed",
                    "message": standard_transfer_syntax_validation_message(expected.transfer_syntax_uid)
                },
                {
                    "name": "synthetic_data_attribute",
                    "status": "passed",
                    "message": "Synthetic Data (0008,001C) is present with value YES."
                },
                {
                    "name": "comprehensive_sr_modules",
                    "status": "passed",
                    "message": "Comprehensive SR document flags, evidence, numeric measurement, and image reference match the recipe."
                }
            ],
            "external": []
        }),
    })
}

pub(crate) fn validate_tid1500_file(
    path: &Path,
    expected: &Tid1500Expectations<'_>,
) -> Result<ValidatedPart10, GenerateError> {
    let bytes = fs::read(path).map_err(|source| GenerateError::ReadGeneratedFile {
        path: path.to_path_buf(),
        source,
    })?;
    let obj = open_file(path).map_err(|err| GenerateError::ValidateDicomFile {
        path: path.to_path_buf(),
        message: err.to_string(),
    })?;
    let mut internal = Vec::new();

    check(
        &mut internal,
        bytes.len() >= 132 && &bytes[128..132] == b"DICM",
        "tid1500_part10_preamble",
        "TID 1500 file has a Part 10 preamble and DICM marker.",
        "TID 1500 file is missing its Part 10 preamble or DICM marker.",
    );
    check_equal(
        &mut internal,
        "tid1500_file_meta_transfer_syntax",
        "File Meta Information Transfer Syntax UID matches the recipe.",
        "File Meta Information Transfer Syntax UID does not match the recipe.",
        trim_uid(obj.meta().transfer_syntax()).as_str(),
        expected.transfer_syntax_uid,
    );
    let dataset_sop_class = element_str(path, &obj, tags::SOP_CLASS_UID)?;
    check_equal(
        &mut internal,
        "tid1500_sop_class_uid_consistency",
        "Dataset, File Meta, and recipe SOP Class UIDs identify Comprehensive 3D SR.",
        "Dataset, File Meta, or recipe SOP Class UID differs.",
        dataset_sop_class.as_str(),
        expected.sop_class_uid,
    );
    check_equal(
        &mut internal,
        "tid1500_media_storage_sop_class_uid",
        "File Meta SOP Class UID matches the dataset.",
        "File Meta SOP Class UID does not match the dataset.",
        trim_uid(obj.meta().media_storage_sop_class_uid()).as_str(),
        dataset_sop_class.as_str(),
    );
    let dataset_sop_instance = element_str(path, &obj, tags::SOP_INSTANCE_UID)?;
    check_equal(
        &mut internal,
        "tid1500_sop_instance_uid_consistency",
        "Dataset, File Meta, and recipe SOP Instance UIDs match.",
        "Dataset, File Meta, or recipe SOP Instance UID differs.",
        dataset_sop_instance.as_str(),
        expected.sop_instance_uid,
    );
    check_equal(
        &mut internal,
        "tid1500_media_storage_sop_instance_uid",
        "File Meta SOP Instance UID matches the dataset.",
        "File Meta SOP Instance UID does not match the dataset.",
        trim_uid(obj.meta().media_storage_sop_instance_uid()).as_str(),
        dataset_sop_instance.as_str(),
    );
    check_equal(
        &mut internal,
        "tid1500_implementation_class_uid",
        "Implementation Class UID matches the locked backend identity.",
        "Implementation Class UID does not match the locked backend identity.",
        trim_uid(obj.meta().implementation_class_uid()).as_str(),
        expected.implementation_class_uid,
    );
    for (name, tag, value) in [
        (
            "tid1500_synthetic_data",
            tags::SYNTHETIC_DATA,
            expected.synthetic_data,
        ),
        ("tid1500_modality", tags::MODALITY, expected.modality),
        (
            "tid1500_completion_flag",
            tags::COMPLETION_FLAG,
            expected.completion_flag,
        ),
        (
            "tid1500_verification_flag",
            tags::VERIFICATION_FLAG,
            expected.verification_flag,
        ),
        (
            "tid1500_preliminary_flag",
            tags::PRELIMINARY_FLAG,
            expected.preliminary_flag,
        ),
    ] {
        check_equal(
            &mut internal,
            name,
            "TID 1500 document attribute matches the recipe.",
            "TID 1500 document attribute does not match the recipe.",
            element_str(path, &obj, tag)?.as_str(),
            value,
        );
    }
    check_equal(
        &mut internal,
        "tid1500_root_value_type",
        "TID 1500 root Value Type is CONTAINER.",
        "TID 1500 root Value Type is not CONTAINER.",
        element_str(path, &obj, tags::VALUE_TYPE)?.as_str(),
        "CONTAINER",
    );
    check_equal(
        &mut internal,
        "tid1500_root_continuity",
        "Highdicom TID 1500 root Continuity of Content is CONTINUOUS.",
        "Highdicom TID 1500 root Continuity of Content is not CONTINUOUS.",
        element_str(path, &obj, tags::CONTINUITY_OF_CONTENT)?.as_str(),
        "CONTINUOUS",
    );
    validate_sr_code(
        &mut internal,
        path,
        &obj,
        tags::CONCEPT_NAME_CODE_SEQUENCE,
        "tid1500_document_title",
        "126000",
        "DCM",
        "Imaging Measurement Report",
    )?;
    check_equal(
        &mut internal,
        "scoord3d_root_template_count",
        "Root contains exactly one Content Template Sequence item.",
        "Root Content Template Sequence cardinality differs from the contract.",
        sequence_item_count(path, &obj, tags::CONTENT_TEMPLATE_SEQUENCE)?,
        1,
    );
    let root_template = top_level_sequence_item(path, &obj, tags::CONTENT_TEMPLATE_SEQUENCE, 0)?;
    check_equal(
        &mut internal,
        "tid1500_root_mapping_resource",
        "Root Content Template Sequence identifies DCMR.",
        "Root Content Template Sequence does not identify DCMR.",
        item_str(path, root_template, tags::MAPPING_RESOURCE)?.as_str(),
        "DCMR",
    );
    check_equal(
        &mut internal,
        "tid1500_root_template_identifier",
        "Root Content Template Sequence identifies TID 1500.",
        "Root Content Template Sequence does not identify TID 1500.",
        item_str(path, root_template, tags::TEMPLATE_IDENTIFIER)?.as_str(),
        "1500",
    );
    check_equal(
        &mut internal,
        "tid1500_root_content_count",
        "TID 1500 root contains the exact highdicom content tree.",
        "TID 1500 root content item count differs from the highdicom contract.",
        sequence_item_count(path, &obj, tags::CONTENT_SEQUENCE)?,
        8,
    );

    let language = top_level_sequence_item(path, &obj, tags::CONTENT_SEQUENCE, 0)?;
    validate_sr_content_item(
        &mut internal,
        path,
        language,
        "tid1500_language",
        "HAS CONCEPT MOD",
        "CODE",
        "121049",
        "DCM",
        "Language of Content Item and Descendants",
    )?;
    validate_sr_code(
        &mut internal,
        path,
        language,
        tags::CONCEPT_CODE_SEQUENCE,
        "tid1500_language_value",
        "en-US",
        "RFC5646",
        "English (United States)",
    )?;

    let observer_type = top_level_sequence_item(path, &obj, tags::CONTENT_SEQUENCE, 1)?;
    validate_sr_content_item(
        &mut internal,
        path,
        observer_type,
        "tid1500_observer_type",
        "HAS OBS CONTEXT",
        "CODE",
        "121005",
        "DCM",
        "Observer Type",
    )?;
    validate_sr_code(
        &mut internal,
        path,
        observer_type,
        tags::CONCEPT_CODE_SEQUENCE,
        "tid1500_observer_type_value",
        "121007",
        "DCM",
        "Device",
    )?;
    let observer_uid = top_level_sequence_item(path, &obj, tags::CONTENT_SEQUENCE, 2)?;
    validate_sr_content_item(
        &mut internal,
        path,
        observer_uid,
        "tid1500_observer_uid",
        "HAS OBS CONTEXT",
        "UIDREF",
        "121012",
        "DCM",
        "Device Observer UID",
    )?;
    check_equal(
        &mut internal,
        "tid1500_observer_uid_value",
        "Device Observer UID matches the deterministic recipe UID.",
        "Device Observer UID does not match the deterministic recipe UID.",
        item_str(path, observer_uid, tags::UID)?.as_str(),
        expected.observer_uid,
    );

    let procedure = top_level_sequence_item(path, &obj, tags::CONTENT_SEQUENCE, 6)?;
    validate_sr_content_item(
        &mut internal,
        path,
        procedure,
        "tid1500_procedure_reported",
        "HAS CONCEPT MOD",
        "CODE",
        "121058",
        "DCM",
        "Procedure reported",
    )?;
    validate_sr_code(
        &mut internal,
        path,
        procedure,
        tags::CONCEPT_CODE_SEQUENCE,
        "tid1500_procedure_value",
        "25045-6",
        "LN",
        "CT unspecified body region",
    )?;

    let imaging = top_level_sequence_item(path, &obj, tags::CONTENT_SEQUENCE, 7)?;
    validate_sr_content_item(
        &mut internal,
        path,
        imaging,
        "tid1500_imaging_measurements",
        "CONTAINS",
        "CONTAINER",
        "126010",
        "DCM",
        "Imaging Measurements",
    )?;
    check_equal(
        &mut internal,
        "tid1500_imaging_measurements_children",
        "Imaging Measurements contains one Measurement Group.",
        "Imaging Measurements child count does not match the recipe.",
        item_sequence_item_count(path, imaging, tags::CONTENT_SEQUENCE)?,
        1,
    );
    let group = item_sequence_item(path, imaging, tags::CONTENT_SEQUENCE, 0)?;
    validate_sr_content_item(
        &mut internal,
        path,
        group,
        "tid1500_measurement_group",
        "CONTAINS",
        "CONTAINER",
        "125007",
        "DCM",
        "Measurement Group",
    )?;
    check_equal(
        &mut internal,
        "scoord3d_measurement_group_template_count",
        "Measurement Group contains exactly one Content Template Sequence item.",
        "Measurement Group Content Template Sequence cardinality differs from the contract.",
        item_sequence_item_count(path, group, tags::CONTENT_TEMPLATE_SEQUENCE)?,
        1,
    );
    let group_template = item_sequence_item(path, group, tags::CONTENT_TEMPLATE_SEQUENCE, 0)?;
    check_equal(
        &mut internal,
        "tid1500_measurement_group_mapping_resource",
        "Measurement Group identifies DCMR.",
        "Measurement Group Mapping Resource does not identify DCMR.",
        item_str(path, group_template, tags::MAPPING_RESOURCE)?.as_str(),
        "DCMR",
    );
    check_equal(
        &mut internal,
        "tid1500_measurement_group_template_identifier",
        "Measurement Group identifies TID 1411.",
        "Measurement Group does not identify TID 1411.",
        item_str(path, group_template, tags::TEMPLATE_IDENTIFIER)?.as_str(),
        "1411",
    );
    check_equal(
        &mut internal,
        "tid1500_measurement_group_children",
        "TID 1411 Measurement Group has six ordered content items.",
        "TID 1411 Measurement Group content item count differs from the recipe.",
        item_sequence_item_count(path, group, tags::CONTENT_SEQUENCE)?,
        6,
    );

    let tracking = item_sequence_item(path, group, tags::CONTENT_SEQUENCE, 0)?;
    validate_sr_content_item(
        &mut internal,
        path,
        tracking,
        "tid1500_tracking_identifier",
        "HAS OBS CONTEXT",
        "TEXT",
        "112039",
        "DCM",
        "Tracking Identifier",
    )?;
    check_equal(
        &mut internal,
        "tid1500_tracking_identifier_value",
        "Tracking Identifier matches the recipe.",
        "Tracking Identifier does not match the recipe.",
        item_str(path, tracking, tags::TEXT_VALUE)?.as_str(),
        expected.tracking_identifier,
    );
    let tracking_uid = item_sequence_item(path, group, tags::CONTENT_SEQUENCE, 1)?;
    validate_sr_content_item(
        &mut internal,
        path,
        tracking_uid,
        "tid1500_tracking_uid",
        "HAS OBS CONTEXT",
        "UIDREF",
        "112040",
        "DCM",
        "Tracking Unique Identifier",
    )?;
    check_equal(
        &mut internal,
        "tid1500_tracking_uid_value",
        "Tracking Unique Identifier matches the recipe.",
        "Tracking Unique Identifier does not match the recipe.",
        item_str(path, tracking_uid, tags::UID)?.as_str(),
        expected.tracking_uid,
    );
    let finding = item_sequence_item(path, group, tags::CONTENT_SEQUENCE, 2)?;
    validate_sr_content_item(
        &mut internal,
        path,
        finding,
        "tid1500_finding",
        "CONTAINS",
        "CODE",
        "121071",
        "DCM",
        "Finding",
    )?;
    validate_sr_code(
        &mut internal,
        path,
        finding,
        tags::CONCEPT_CODE_SEQUENCE,
        "tid1500_finding_value",
        "123037004",
        "SCT",
        "Body structure",
    )?;

    let measurement = item_sequence_item(path, group, tags::CONTENT_SEQUENCE, 3)?;
    validate_sr_content_item(
        &mut internal,
        path,
        measurement,
        "tid1500_volume_measurement",
        "CONTAINS",
        "NUM",
        "118565006",
        "SCT",
        "Volume",
    )?;
    let measured_value = item_sequence_item(path, measurement, tags::MEASURED_VALUE_SEQUENCE, 0)?;
    check_equal(
        &mut internal,
        "tid1500_numeric_value",
        "Volume Numeric Value is exactly 5.625.",
        "Volume Numeric Value is not exactly 5.625.",
        item_str(path, measured_value, tags::NUMERIC_VALUE)?.as_str(),
        "5.625",
    );
    check_equal(
        &mut internal,
        "tid1500_floating_point_value",
        "Volume Floating Point Value is exactly 5.625.",
        "Volume Floating Point Value is not exactly 5.625.",
        item_f64(path, measured_value, tags::FLOATING_POINT_VALUE)?,
        5.625,
    );
    validate_sr_code(
        &mut internal,
        path,
        measured_value,
        tags::MEASUREMENT_UNITS_CODE_SEQUENCE,
        "tid1500_measurement_units",
        "mm3",
        "UCUM",
        "cubic millimeter",
    )?;

    let referenced_segment = item_sequence_item(path, group, tags::CONTENT_SEQUENCE, 4)?;
    validate_sr_content_item(
        &mut internal,
        path,
        referenced_segment,
        "tid1500_referenced_segment",
        "CONTAINS",
        "IMAGE",
        "121191",
        "DCM",
        "Referenced Segment",
    )?;
    let segment_sop =
        item_sequence_item(path, referenced_segment, tags::REFERENCED_SOP_SEQUENCE, 0)?;
    check_sr_reference(
        &mut internal,
        path,
        segment_sop,
        "tid1500_referenced_segment",
        expected.segmentation_sop_class_uid,
        expected.segmentation_sop_instance_uid,
    )?;
    check_equal(
        &mut internal,
        "tid1500_referenced_segment_number",
        "Referenced Segment identifies segment 1.",
        "Referenced Segment number does not match the recipe.",
        item_u16(path, segment_sop, TAG_REFERENCED_SEGMENT_NUMBER)?,
        expected.referenced_segment_number,
    );
    check(
        &mut internal,
        segment_sop
            .element_opt(TAG_REFERENCED_FRAME_NUMBER)
            .map_err(|err| validation_error(path, err))?
            .is_none(),
        "tid1500_referenced_segment_frame_absent",
        "Referenced Segment correctly omits Referenced Frame Number.",
        "Referenced Segment unexpectedly includes Referenced Frame Number.",
    );

    let source_image = item_sequence_item(path, group, tags::CONTENT_SEQUENCE, 5)?;
    validate_sr_content_item(
        &mut internal,
        path,
        source_image,
        "tid1500_source_image_for_segmentation",
        "CONTAINS",
        "IMAGE",
        "121233",
        "DCM",
        "Source image for segmentation",
    )?;
    let source_sop = item_sequence_item(path, source_image, tags::REFERENCED_SOP_SEQUENCE, 0)?;
    check_sr_reference(
        &mut internal,
        path,
        source_sop,
        "tid1500_source_image",
        expected.source_sop_class_uid,
        expected.source_sop_instance_uid,
    )?;
    let source_frames = item_i32_values(path, source_sop, TAG_REFERENCED_FRAME_NUMBER)?;
    let expected_source_frames = expected
        .source_frame_numbers
        .iter()
        .map(|number| i32::from(*number))
        .collect::<Vec<_>>();
    check_equal(
        &mut internal,
        "tid1500_source_image_frames",
        "Source image for segmentation references CT frames 1 and 2.",
        "Source image for segmentation frame references do not match the recipe.",
        source_frames,
        expected_source_frames,
    );

    let evidence = top_level_sequence_item(
        path,
        &obj,
        tags::CURRENT_REQUESTED_PROCEDURE_EVIDENCE_SEQUENCE,
        0,
    )?;
    check_equal(
        &mut internal,
        "tid1500_evidence_study_instance_uid",
        "Evidence Study Instance UID matches the source study.",
        "Evidence Study Instance UID does not match the source study.",
        item_str(path, evidence, tags::STUDY_INSTANCE_UID)?.as_str(),
        expected.referenced_study_instance_uid,
    );
    check_equal(
        &mut internal,
        "tid1500_evidence_series_count",
        "Evidence contains ordered CT then SEG series entries.",
        "Evidence does not contain exactly two ordered series entries.",
        item_sequence_item_count(path, evidence, tags::REFERENCED_SERIES_SEQUENCE)?,
        2,
    );
    for (index, prefix, series_uid, sop_class_uid, sop_instance_uid) in [
        (
            0,
            "tid1500_evidence_ct",
            expected.source_series_instance_uid,
            expected.source_sop_class_uid,
            expected.source_sop_instance_uid,
        ),
        (
            1,
            "tid1500_evidence_seg",
            expected.segmentation_series_instance_uid,
            expected.segmentation_sop_class_uid,
            expected.segmentation_sop_instance_uid,
        ),
    ] {
        let series = item_sequence_item(path, evidence, tags::REFERENCED_SERIES_SEQUENCE, index)?;
        check_equal(
            &mut internal,
            &format!("{prefix}_series_instance_uid"),
            "Evidence series identity and order match the recipe.",
            "Evidence series identity or order does not match the recipe.",
            item_str(path, series, tags::SERIES_INSTANCE_UID)?.as_str(),
            series_uid,
        );
        check_equal(
            &mut internal,
            &format!("{prefix}_sop_count"),
            "Evidence series contains exactly one SOP reference.",
            "Evidence series SOP reference count does not match the recipe.",
            item_sequence_item_count(path, series, tags::REFERENCED_SOP_SEQUENCE)?,
            1,
        );
        let sop = item_sequence_item(path, series, tags::REFERENCED_SOP_SEQUENCE, 0)?;
        check_sr_reference(
            &mut internal,
            path,
            sop,
            prefix,
            sop_class_uid,
            sop_instance_uid,
        )?;
    }

    for (name, tag) in [
        ("tid1500_integer_pixel_data_absent", tags::PIXEL_DATA),
        ("tid1500_float_pixel_data_absent", tags::FLOAT_PIXEL_DATA),
        (
            "tid1500_double_float_pixel_data_absent",
            tags::DOUBLE_FLOAT_PIXEL_DATA,
        ),
    ] {
        check(
            &mut internal,
            obj.element_opt(tag)
                .map_err(|err| validation_error(path, err))?
                .is_none(),
            name,
            "Structured Report contains no pixel payload.",
            "Structured Report unexpectedly contains pixel payload.",
        );
    }

    fail_if_any_failed(path, &internal)?;
    Ok(ValidatedPart10 {
        bytes,
        validation: serde_json::json!({
            "status": "passed",
            "internal": internal,
            "standards": [
                {
                    "name": standard_sop_class_validation_name(expected.sop_class_uid),
                    "status": "passed",
                    "message": standard_sop_class_validation_message(expected.sop_class_uid)
                },
                {
                    "name": standard_transfer_syntax_validation_name(expected.transfer_syntax_uid),
                    "status": "passed",
                    "message": standard_transfer_syntax_validation_message(expected.transfer_syntax_uid)
                },
                {
                    "name": "tid1500_measurement_report",
                    "status": "passed",
                    "message": "TID 1500, TID 1411, measurement, SEG/source references, and evidence closure match the recipe."
                }
            ],
            "external": []
        }),
    })
}

pub(crate) fn validate_scoord3d_file(
    path: &Path,
    expected: &Scoord3dExpectations<'_>,
) -> Result<ValidatedPart10, GenerateError> {
    let bytes = fs::read(path).map_err(|source| GenerateError::ReadGeneratedFile {
        path: path.to_path_buf(),
        source,
    })?;
    let obj = open_file(path).map_err(|err| GenerateError::ValidateDicomFile {
        path: path.to_path_buf(),
        message: err.to_string(),
    })?;
    let mut internal = Vec::new();

    check(
        &mut internal,
        bytes.len() >= 132 && &bytes[128..132] == b"DICM",
        "scoord3d_part10_preamble",
        "SCOORD3D report has a Part 10 preamble and DICM marker.",
        "SCOORD3D report is missing its Part 10 preamble or DICM marker.",
    );
    check_equal(
        &mut internal,
        "scoord3d_file_meta_transfer_syntax",
        "File Meta Information Transfer Syntax UID matches the recipe.",
        "File Meta Information Transfer Syntax UID does not match the recipe.",
        trim_uid(obj.meta().transfer_syntax()).as_str(),
        expected.transfer_syntax_uid,
    );
    let dataset_sop_class = element_str(path, &obj, tags::SOP_CLASS_UID)?;
    check_equal(
        &mut internal,
        "scoord3d_sop_class_uid_consistency",
        "Dataset, File Meta, and recipe identify Comprehensive 3D SR Storage.",
        "Dataset, File Meta, or recipe SOP Class UID differs.",
        dataset_sop_class.as_str(),
        expected.sop_class_uid,
    );
    check_equal(
        &mut internal,
        "scoord3d_comprehensive_3d_sr_storage",
        "SOP Class UID is Comprehensive 3D SR Storage.",
        "SOP Class UID is not Comprehensive 3D SR Storage.",
        dataset_sop_class.as_str(),
        "1.2.840.10008.5.1.4.1.1.88.34",
    );
    check_equal(
        &mut internal,
        "scoord3d_media_storage_sop_class_uid",
        "File Meta SOP Class UID matches the dataset.",
        "File Meta SOP Class UID does not match the dataset.",
        trim_uid(obj.meta().media_storage_sop_class_uid()).as_str(),
        dataset_sop_class.as_str(),
    );
    let dataset_sop_instance = element_str(path, &obj, tags::SOP_INSTANCE_UID)?;
    check_equal(
        &mut internal,
        "scoord3d_sop_instance_uid_consistency",
        "Dataset, File Meta, and recipe SOP Instance UIDs match.",
        "Dataset, File Meta, or recipe SOP Instance UID differs.",
        dataset_sop_instance.as_str(),
        expected.sop_instance_uid,
    );
    check_equal(
        &mut internal,
        "scoord3d_media_storage_sop_instance_uid",
        "File Meta SOP Instance UID matches the dataset.",
        "File Meta SOP Instance UID does not match the dataset.",
        trim_uid(obj.meta().media_storage_sop_instance_uid()).as_str(),
        dataset_sop_instance.as_str(),
    );
    check_equal(
        &mut internal,
        "scoord3d_implementation_class_uid",
        "Implementation Class UID matches the locked backend identity.",
        "Implementation Class UID does not match the locked backend identity.",
        trim_uid(obj.meta().implementation_class_uid()).as_str(),
        expected.implementation_class_uid,
    );
    for (name, tag, value) in [
        (
            "scoord3d_synthetic_data",
            tags::SYNTHETIC_DATA,
            expected.synthetic_data,
        ),
        ("scoord3d_modality", tags::MODALITY, expected.modality),
        (
            "scoord3d_completion_flag",
            tags::COMPLETION_FLAG,
            expected.completion_flag,
        ),
        (
            "scoord3d_verification_flag",
            tags::VERIFICATION_FLAG,
            expected.verification_flag,
        ),
        (
            "scoord3d_preliminary_flag",
            tags::PRELIMINARY_FLAG,
            expected.preliminary_flag,
        ),
    ] {
        check_equal(
            &mut internal,
            name,
            "SCOORD3D document attribute matches the recipe.",
            "SCOORD3D document attribute does not match the recipe.",
            element_str(path, &obj, tag)?.as_str(),
            value,
        );
    }
    check_equal(
        &mut internal,
        "scoord3d_root_value_type",
        "TID 1500 root Value Type is CONTAINER.",
        "TID 1500 root Value Type is not CONTAINER.",
        element_str(path, &obj, tags::VALUE_TYPE)?.as_str(),
        "CONTAINER",
    );
    check_equal(
        &mut internal,
        "scoord3d_root_continuity",
        "TID 1500 root Continuity of Content is CONTINUOUS.",
        "TID 1500 root Continuity of Content is not CONTINUOUS.",
        element_str(path, &obj, tags::CONTINUITY_OF_CONTENT)?.as_str(),
        "CONTINUOUS",
    );
    validate_sr_code(
        &mut internal,
        path,
        &obj,
        tags::CONCEPT_NAME_CODE_SEQUENCE,
        "scoord3d_document_title",
        "126000",
        "DCM",
        "Imaging Measurement Report",
    )?;
    let root_template = top_level_sequence_item(path, &obj, tags::CONTENT_TEMPLATE_SEQUENCE, 0)?;
    check_equal(
        &mut internal,
        "scoord3d_root_mapping_resource",
        "Root Content Template Sequence identifies DCMR.",
        "Root Content Template Sequence does not identify DCMR.",
        item_str(path, root_template, tags::MAPPING_RESOURCE)?.as_str(),
        "DCMR",
    );
    check_equal(
        &mut internal,
        "scoord3d_root_template_identifier",
        "Root Content Template Sequence identifies TID 1500.",
        "Root Content Template Sequence does not identify TID 1500.",
        item_str(path, root_template, tags::TEMPLATE_IDENTIFIER)?.as_str(),
        "1500",
    );
    check_equal(
        &mut internal,
        "scoord3d_root_content_count",
        "TID 1500 root contains the exact highdicom content tree.",
        "TID 1500 root content item count differs from the contract.",
        sequence_item_count(path, &obj, tags::CONTENT_SEQUENCE)?,
        8,
    );

    let language = top_level_sequence_item(path, &obj, tags::CONTENT_SEQUENCE, 0)?;
    validate_sr_content_item(
        &mut internal,
        path,
        language,
        "scoord3d_language",
        "HAS CONCEPT MOD",
        "CODE",
        "121049",
        "DCM",
        "Language of Content Item and Descendants",
    )?;
    validate_sr_code(
        &mut internal,
        path,
        language,
        tags::CONCEPT_CODE_SEQUENCE,
        "scoord3d_language_value",
        "en-US",
        "RFC5646",
        "English (United States)",
    )?;
    let observer_type = top_level_sequence_item(path, &obj, tags::CONTENT_SEQUENCE, 1)?;
    validate_sr_content_item(
        &mut internal,
        path,
        observer_type,
        "scoord3d_observer_type",
        "HAS OBS CONTEXT",
        "CODE",
        "121005",
        "DCM",
        "Observer Type",
    )?;
    validate_sr_code(
        &mut internal,
        path,
        observer_type,
        tags::CONCEPT_CODE_SEQUENCE,
        "scoord3d_observer_type_value",
        "121007",
        "DCM",
        "Device",
    )?;
    let observer_uid = top_level_sequence_item(path, &obj, tags::CONTENT_SEQUENCE, 2)?;
    validate_sr_content_item(
        &mut internal,
        path,
        observer_uid,
        "scoord3d_observer_uid",
        "HAS OBS CONTEXT",
        "UIDREF",
        "121012",
        "DCM",
        "Device Observer UID",
    )?;
    check_equal(
        &mut internal,
        "scoord3d_observer_uid_value",
        "Device Observer UID matches the deterministic recipe UID.",
        "Device Observer UID does not match the deterministic recipe UID.",
        item_str(path, observer_uid, tags::UID)?.as_str(),
        expected.observer_uid,
    );
    let procedure = top_level_sequence_item(path, &obj, tags::CONTENT_SEQUENCE, 6)?;
    validate_sr_content_item(
        &mut internal,
        path,
        procedure,
        "scoord3d_procedure_reported",
        "HAS CONCEPT MOD",
        "CODE",
        "121058",
        "DCM",
        "Procedure reported",
    )?;
    validate_sr_code(
        &mut internal,
        path,
        procedure,
        tags::CONCEPT_CODE_SEQUENCE,
        "scoord3d_procedure_value",
        "25045-6",
        "LN",
        "CT unspecified body region",
    )?;

    let imaging = top_level_sequence_item(path, &obj, tags::CONTENT_SEQUENCE, 7)?;
    validate_sr_content_item(
        &mut internal,
        path,
        imaging,
        "scoord3d_imaging_measurements",
        "CONTAINS",
        "CONTAINER",
        "126010",
        "DCM",
        "Imaging Measurements",
    )?;
    check_equal(
        &mut internal,
        "scoord3d_imaging_measurements_children",
        "Imaging Measurements contains one Measurement Group.",
        "Imaging Measurements child count does not match the recipe.",
        item_sequence_item_count(path, imaging, tags::CONTENT_SEQUENCE)?,
        1,
    );
    let group = item_sequence_item(path, imaging, tags::CONTENT_SEQUENCE, 0)?;
    validate_sr_content_item(
        &mut internal,
        path,
        group,
        "scoord3d_measurement_group",
        "CONTAINS",
        "CONTAINER",
        "125007",
        "DCM",
        "Measurement Group",
    )?;
    let group_template = item_sequence_item(path, group, tags::CONTENT_TEMPLATE_SEQUENCE, 0)?;
    check_equal(
        &mut internal,
        "scoord3d_measurement_group_mapping_resource",
        "Measurement Group identifies DCMR.",
        "Measurement Group Mapping Resource does not identify DCMR.",
        item_str(path, group_template, tags::MAPPING_RESOURCE)?.as_str(),
        "DCMR",
    );
    check_equal(
        &mut internal,
        "scoord3d_measurement_group_template_identifier",
        "Measurement Group identifies TID 1501.",
        "Measurement Group does not identify TID 1501.",
        item_str(path, group_template, tags::TEMPLATE_IDENTIFIER)?.as_str(),
        "1501",
    );
    check_equal(
        &mut internal,
        "scoord3d_measurement_group_children",
        "TID 1501 Measurement Group has five ordered content items.",
        "TID 1501 Measurement Group content item count differs from the recipe.",
        item_sequence_item_count(path, group, tags::CONTENT_SEQUENCE)?,
        5,
    );

    let tracking = item_sequence_item(path, group, tags::CONTENT_SEQUENCE, 0)?;
    validate_sr_content_item(
        &mut internal,
        path,
        tracking,
        "scoord3d_tracking_identifier",
        "HAS OBS CONTEXT",
        "TEXT",
        "112039",
        "DCM",
        "Tracking Identifier",
    )?;
    check_equal(
        &mut internal,
        "scoord3d_tracking_identifier_value",
        "Tracking Identifier matches the recipe.",
        "Tracking Identifier does not match the recipe.",
        item_str(path, tracking, tags::TEXT_VALUE)?.as_str(),
        expected.tracking_identifier,
    );
    let tracking_uid = item_sequence_item(path, group, tags::CONTENT_SEQUENCE, 1)?;
    validate_sr_content_item(
        &mut internal,
        path,
        tracking_uid,
        "scoord3d_tracking_uid",
        "HAS OBS CONTEXT",
        "UIDREF",
        "112040",
        "DCM",
        "Tracking Unique Identifier",
    )?;
    check_equal(
        &mut internal,
        "scoord3d_tracking_uid_value",
        "Tracking Unique Identifier matches the recipe.",
        "Tracking Unique Identifier does not match the recipe.",
        item_str(path, tracking_uid, tags::UID)?.as_str(),
        expected.tracking_uid,
    );
    let finding = item_sequence_item(path, group, tags::CONTENT_SEQUENCE, 2)?;
    validate_sr_content_item(
        &mut internal,
        path,
        finding,
        "scoord3d_finding",
        "CONTAINS",
        "CODE",
        "121071",
        "DCM",
        "Finding",
    )?;
    validate_sr_code(
        &mut internal,
        path,
        finding,
        tags::CONCEPT_CODE_SEQUENCE,
        "scoord3d_finding_value",
        "123037004",
        "SCT",
        "Body structure",
    )?;

    let measurement = item_sequence_item(path, group, tags::CONTENT_SEQUENCE, 3)?;
    validate_sr_content_item(
        &mut internal,
        path,
        measurement,
        "scoord3d_distance_measurement",
        "CONTAINS",
        "NUM",
        "121206",
        "DCM",
        "Distance",
    )?;
    check_equal(
        &mut internal,
        "scoord3d_measured_value_count",
        "Distance contains exactly one Measured Value Sequence item.",
        "Distance Measured Value Sequence cardinality differs from the contract.",
        item_sequence_item_count(path, measurement, tags::MEASURED_VALUE_SEQUENCE)?,
        1,
    );
    let measured_value = item_sequence_item(path, measurement, tags::MEASURED_VALUE_SEQUENCE, 0)?;
    check_equal(
        &mut internal,
        "scoord3d_numeric_value",
        "Distance Numeric Value is exactly 2.5.",
        "Distance Numeric Value is not exactly 2.5.",
        item_str(path, measured_value, tags::NUMERIC_VALUE)?.as_str(),
        "2.5",
    );
    check_equal(
        &mut internal,
        "scoord3d_floating_point_value",
        "Distance Floating Point Value is exactly 2.5.",
        "Distance Floating Point Value is not exactly 2.5.",
        item_f64(path, measured_value, tags::FLOATING_POINT_VALUE)?,
        2.5,
    );
    validate_sr_code(
        &mut internal,
        path,
        measured_value,
        tags::MEASUREMENT_UNITS_CODE_SEQUENCE,
        "scoord3d_measurement_units",
        "mm",
        "UCUM",
        "millimeter",
    )?;
    check_equal(
        &mut internal,
        "scoord3d_measurement_children",
        "Distance measurement contains one SCOORD3D source.",
        "Distance measurement does not contain exactly one SCOORD3D source.",
        item_sequence_item_count(path, measurement, tags::CONTENT_SEQUENCE)?,
        1,
    );
    let coordinates = item_sequence_item(path, measurement, tags::CONTENT_SEQUENCE, 0)?;
    validate_sr_content_item(
        &mut internal,
        path,
        coordinates,
        "scoord3d_coordinates",
        "INFERRED FROM",
        "SCOORD3D",
        "260753009",
        "SCT",
        "Source",
    )?;
    check_equal(
        &mut internal,
        "scoord3d_graphic_type",
        "SCOORD3D Graphic Type is POLYLINE.",
        "SCOORD3D Graphic Type is not POLYLINE.",
        item_str(path, coordinates, tags::GRAPHIC_TYPE)?.as_str(),
        "POLYLINE",
    );
    check_equal(
        &mut internal,
        "scoord3d_graphic_data",
        "SCOORD3D Graphic Data contains the exact two patient-space endpoints.",
        "SCOORD3D Graphic Data differs from the derived source geometry.",
        item_f32_values(path, coordinates, tags::GRAPHIC_DATA)?,
        vec![0.0_f32, 0.0, 0.0, 0.0, 0.0, 2.5],
    );
    check_equal(
        &mut internal,
        "scoord3d_frame_of_reference_uid",
        "SCOORD3D Frame of Reference UID matches the Enhanced CT.",
        "SCOORD3D Frame of Reference UID does not match the Enhanced CT.",
        item_str(path, coordinates, tags::REFERENCED_FRAME_OF_REFERENCE_UID)?.as_str(),
        expected.frame_of_reference_uid,
    );
    check_equal(
        &mut internal,
        "scoord3d_fiducial_uid",
        "SCOORD3D Fiducial UID matches the deterministic recipe UID.",
        "SCOORD3D Fiducial UID does not match the deterministic recipe UID.",
        item_str(path, coordinates, tags::FIDUCIAL_UID)?.as_str(),
        expected.fiducial_uid,
    );

    let source_image = item_sequence_item(path, group, tags::CONTENT_SEQUENCE, 4)?;
    validate_sr_content_item(
        &mut internal,
        path,
        source_image,
        "scoord3d_source_image",
        "CONTAINS",
        "IMAGE",
        "121112",
        "DCM",
        "Source of Measurement",
    )?;
    check_equal(
        &mut internal,
        "scoord3d_source_reference_count",
        "Source of Measurement contains exactly one SOP reference.",
        "Source of Measurement SOP reference cardinality differs from the contract.",
        item_sequence_item_count(path, source_image, tags::REFERENCED_SOP_SEQUENCE)?,
        1,
    );
    let source_sop = item_sequence_item(path, source_image, tags::REFERENCED_SOP_SEQUENCE, 0)?;
    check_sr_reference(
        &mut internal,
        path,
        source_sop,
        "scoord3d_source_image",
        expected.source_sop_class_uid,
        expected.source_sop_instance_uid,
    )?;
    let expected_source_frames = expected
        .source_frame_numbers
        .iter()
        .map(|number| i32::from(*number))
        .collect::<Vec<_>>();
    check_equal(
        &mut internal,
        "scoord3d_source_image_frames",
        "Source of Measurement references Enhanced CT frames 1 and 2.",
        "Source of Measurement frame references do not match the recipe.",
        item_i32_values(path, source_sop, TAG_REFERENCED_FRAME_NUMBER)?,
        expected_source_frames,
    );

    check_equal(
        &mut internal,
        "scoord3d_evidence_study_count",
        "Current Requested Procedure Evidence contains exactly one study.",
        "Current Requested Procedure Evidence does not contain exactly one study.",
        sequence_item_count(
            path,
            &obj,
            tags::CURRENT_REQUESTED_PROCEDURE_EVIDENCE_SEQUENCE,
        )?,
        1,
    );
    let evidence = top_level_sequence_item(
        path,
        &obj,
        tags::CURRENT_REQUESTED_PROCEDURE_EVIDENCE_SEQUENCE,
        0,
    )?;
    check_equal(
        &mut internal,
        "scoord3d_evidence_study_instance_uid",
        "Evidence Study Instance UID matches the source study.",
        "Evidence Study Instance UID does not match the source study.",
        item_str(path, evidence, tags::STUDY_INSTANCE_UID)?.as_str(),
        expected.referenced_study_instance_uid,
    );
    check_equal(
        &mut internal,
        "scoord3d_evidence_series_count",
        "Evidence contains exactly one Enhanced CT series.",
        "Evidence does not contain exactly one Enhanced CT series.",
        item_sequence_item_count(path, evidence, tags::REFERENCED_SERIES_SEQUENCE)?,
        1,
    );
    let evidence_series = item_sequence_item(path, evidence, tags::REFERENCED_SERIES_SEQUENCE, 0)?;
    check_equal(
        &mut internal,
        "scoord3d_evidence_series_instance_uid",
        "Evidence series identity matches the Enhanced CT.",
        "Evidence series identity does not match the Enhanced CT.",
        item_str(path, evidence_series, tags::SERIES_INSTANCE_UID)?.as_str(),
        expected.source_series_instance_uid,
    );
    check_equal(
        &mut internal,
        "scoord3d_evidence_sop_count",
        "Evidence series contains exactly one SOP reference.",
        "Evidence series SOP reference count does not match the recipe.",
        item_sequence_item_count(path, evidence_series, tags::REFERENCED_SOP_SEQUENCE)?,
        1,
    );
    let evidence_sop = item_sequence_item(path, evidence_series, tags::REFERENCED_SOP_SEQUENCE, 0)?;
    check_sr_reference(
        &mut internal,
        path,
        evidence_sop,
        "scoord3d_evidence",
        expected.source_sop_class_uid,
        expected.source_sop_instance_uid,
    )?;

    for (name, tag) in [
        ("scoord3d_integer_pixel_data_absent", tags::PIXEL_DATA),
        ("scoord3d_float_pixel_data_absent", tags::FLOAT_PIXEL_DATA),
        (
            "scoord3d_double_float_pixel_data_absent",
            tags::DOUBLE_FLOAT_PIXEL_DATA,
        ),
    ] {
        check(
            &mut internal,
            obj.element_opt(tag)
                .map_err(|err| validation_error(path, err))?
                .is_none(),
            name,
            "Structured Report contains no pixel payload.",
            "Structured Report unexpectedly contains pixel payload.",
        );
    }

    fail_if_any_failed(path, &internal)?;
    Ok(ValidatedPart10 {
        bytes,
        validation: serde_json::json!({
            "status": "passed",
            "internal": internal,
            "standards": [
                {
                    "name": standard_sop_class_validation_name(expected.sop_class_uid),
                    "status": "passed",
                    "message": standard_sop_class_validation_message(expected.sop_class_uid)
                },
                {
                    "name": standard_transfer_syntax_validation_name(expected.transfer_syntax_uid),
                    "status": "passed",
                    "message": standard_transfer_syntax_validation_message(expected.transfer_syntax_uid)
                },
                {
                    "name": "comprehensive_3d_sr_scoord3d",
                    "status": "passed",
                    "message": "TID 1500, TID 1501, distance measurement, SCOORD3D geometry, source reference, and evidence closure match the recipe."
                }
            ],
            "external": []
        }),
    })
}

pub(crate) fn validate_spatial_registration_file(
    path: &Path,
    expected: &SpatialRegistrationExpectations<'_>,
) -> Result<ValidatedPart10, GenerateError> {
    let bytes = fs::read(path).map_err(|source| GenerateError::ReadGeneratedFile {
        path: path.to_path_buf(),
        source,
    })?;
    let obj = open_file(path).map_err(|err| validation_error(path, err))?;
    let mut internal = Vec::new();

    check(
        &mut internal,
        bytes.len() >= 132 && &bytes[128..132] == b"DICM",
        "spatial_registration_part10_preamble",
        "Spatial Registration has a Part 10 preamble and DICM marker.",
        "Spatial Registration is missing its Part 10 preamble or DICM marker.",
    );
    check_equal(
        &mut internal,
        "spatial_registration_transfer_syntax",
        "File Meta Transfer Syntax matches the recipe.",
        "File Meta Transfer Syntax does not match the recipe.",
        trim_uid(obj.meta().transfer_syntax()).as_str(),
        expected.transfer_syntax_uid,
    );
    let dataset_sop_class = element_str(path, &obj, tags::SOP_CLASS_UID)?;
    check_equal(
        &mut internal,
        "spatial_registration_sop_class_uid",
        "Dataset SOP Class UID identifies Spatial Registration Storage.",
        "Dataset SOP Class UID does not identify Spatial Registration Storage.",
        dataset_sop_class.as_str(),
        expected.sop_class_uid,
    );
    check_equal(
        &mut internal,
        "spatial_registration_storage_uid",
        "SOP Class UID is the standard Spatial Registration Storage UID.",
        "SOP Class UID is not the standard Spatial Registration Storage UID.",
        dataset_sop_class.as_str(),
        "1.2.840.10008.5.1.4.1.1.66.1",
    );
    check_equal(
        &mut internal,
        "spatial_registration_media_sop_class_uid",
        "File Meta and dataset SOP Class UIDs match.",
        "File Meta and dataset SOP Class UIDs differ.",
        trim_uid(obj.meta().media_storage_sop_class_uid()).as_str(),
        dataset_sop_class.as_str(),
    );
    let dataset_sop_instance = element_str(path, &obj, tags::SOP_INSTANCE_UID)?;
    check_equal(
        &mut internal,
        "spatial_registration_sop_instance_uid",
        "Dataset SOP Instance UID matches the recipe.",
        "Dataset SOP Instance UID does not match the recipe.",
        dataset_sop_instance.as_str(),
        expected.sop_instance_uid,
    );
    check_equal(
        &mut internal,
        "spatial_registration_media_sop_instance_uid",
        "File Meta and dataset SOP Instance UIDs match.",
        "File Meta and dataset SOP Instance UIDs differ.",
        trim_uid(obj.meta().media_storage_sop_instance_uid()).as_str(),
        dataset_sop_instance.as_str(),
    );
    check_equal(
        &mut internal,
        "spatial_registration_implementation_class_uid",
        "Implementation Class UID matches the native generator.",
        "Implementation Class UID does not match the native generator.",
        trim_uid(obj.meta().implementation_class_uid()).as_str(),
        expected.implementation_class_uid,
    );

    for (name, tag, value) in [
        (
            "spatial_registration_synthetic_data",
            tags::SYNTHETIC_DATA,
            expected.synthetic_data,
        ),
        (
            "spatial_registration_patient_id",
            tags::PATIENT_ID,
            expected.patient_id,
        ),
        (
            "spatial_registration_study_instance_uid",
            tags::STUDY_INSTANCE_UID,
            expected.study_instance_uid,
        ),
        (
            "spatial_registration_study_id",
            tags::STUDY_ID,
            expected.study_id,
        ),
        (
            "spatial_registration_series_instance_uid",
            tags::SERIES_INSTANCE_UID,
            expected.series_instance_uid,
        ),
        (
            "spatial_registration_series_number",
            tags::SERIES_NUMBER,
            expected.series_number,
        ),
        (
            "spatial_registration_laterality",
            tags::LATERALITY,
            expected.laterality,
        ),
        (
            "spatial_registration_modality",
            tags::MODALITY,
            expected.modality,
        ),
        (
            "spatial_registration_instance_number",
            tags::INSTANCE_NUMBER,
            expected.instance_number,
        ),
        (
            "spatial_registration_content_date",
            tags::CONTENT_DATE,
            expected.content_date,
        ),
        (
            "spatial_registration_content_time",
            tags::CONTENT_TIME,
            expected.content_time,
        ),
        (
            "spatial_registration_content_label",
            tags::CONTENT_LABEL,
            expected.content_label,
        ),
        (
            "spatial_registration_content_description",
            tags::CONTENT_DESCRIPTION,
            expected.content_description,
        ),
        (
            "spatial_registration_content_creator_name",
            tags::CONTENT_CREATOR_NAME,
            expected.content_creator_name,
        ),
        (
            "spatial_registration_manufacturer",
            tags::MANUFACTURER,
            expected.manufacturer,
        ),
        (
            "spatial_registration_manufacturer_model_name",
            tags::MANUFACTURER_MODEL_NAME,
            expected.manufacturer_model_name,
        ),
        (
            "spatial_registration_device_serial_number",
            tags::DEVICE_SERIAL_NUMBER,
            expected.device_serial_number,
        ),
        (
            "spatial_registration_software_versions",
            tags::SOFTWARE_VERSIONS,
            expected.software_versions,
        ),
        (
            "spatial_registration_registered_frame_of_reference_uid",
            tags::FRAME_OF_REFERENCE_UID,
            expected.registered_frame_of_reference_uid,
        ),
    ] {
        check_equal(
            &mut internal,
            name,
            "Spatial Registration module attribute matches the recipe.",
            "Spatial Registration module attribute does not match the recipe.",
            element_str(path, &obj, tag)?.as_str(),
            value,
        );
    }
    check_equal(
        &mut internal,
        "spatial_registration_target_study_uid",
        "Registered target belongs to the Spatial Registration Study.",
        "Registered target Study does not match the Spatial Registration Study.",
        element_str(path, &obj, tags::STUDY_INSTANCE_UID)?.as_str(),
        expected.target.study_instance_uid,
    );
    check_equal(
        &mut internal,
        "spatial_registration_target_registered_frame",
        "Registered target Frame of Reference establishes the Registered RCS.",
        "Registered target Frame of Reference does not establish the Registered RCS.",
        element_str(path, &obj, tags::FRAME_OF_REFERENCE_UID)?.as_str(),
        expected.target.frame_of_reference_uid,
    );
    for (name, tag) in [
        ("spatial_registration_study_date", tags::STUDY_DATE),
        ("spatial_registration_study_time", tags::STUDY_TIME),
        (
            "spatial_registration_referring_physician",
            tags::REFERRING_PHYSICIAN_NAME,
        ),
        (
            "spatial_registration_accession_number",
            tags::ACCESSION_NUMBER,
        ),
        (
            "spatial_registration_position_reference_indicator",
            tags::POSITION_REFERENCE_INDICATOR,
        ),
    ] {
        check(
            &mut internal,
            obj.element_opt(tag)
                .map_err(|err| validation_error(path, err))?
                .is_some(),
            name,
            "Required module attribute is present.",
            "Required module attribute is absent.",
        );
    }

    check_equal(
        &mut internal,
        "spatial_registration_registration_count",
        "Registration Sequence has the exact target and source items.",
        "Registration Sequence does not have exactly two items.",
        sequence_item_count(path, &obj, tags::REGISTRATION_SEQUENCE)?,
        2,
    );
    let target_item = top_level_sequence_item(path, &obj, tags::REGISTRATION_SEQUENCE, 0)?;
    validate_spatial_registration_item(
        &mut internal,
        path,
        target_item,
        "spatial_registration_target",
        &expected.target,
        &expected.target_matrix,
        expected.source_landmark_mm,
        expected.source_landmark_mm,
        expected.rigid_tolerance,
    )?;
    let source_item = top_level_sequence_item(path, &obj, tags::REGISTRATION_SEQUENCE, 1)?;
    validate_spatial_registration_item(
        &mut internal,
        path,
        source_item,
        "spatial_registration_source",
        &expected.source,
        &expected.source_to_registered_matrix,
        expected.source_landmark_mm,
        expected.registered_landmark_mm,
        expected.rigid_tolerance,
    )?;

    check_equal(
        &mut internal,
        "spatial_registration_same_study_series_count",
        "Same-Study references contain only the registered target series.",
        "Same-Study reference series count differs from the contract.",
        sequence_item_count(path, &obj, tags::REFERENCED_SERIES_SEQUENCE)?,
        1,
    );
    let same_study = top_level_sequence_item(path, &obj, tags::REFERENCED_SERIES_SEQUENCE, 0)?;
    validate_common_reference_series(
        &mut internal,
        path,
        same_study,
        "spatial_registration_same_study",
        &expected.target,
    )?;

    check_equal(
        &mut internal,
        "spatial_registration_other_study_count",
        "Other-Study references contain only the moving source study.",
        "Other-Study reference count differs from the contract.",
        sequence_item_count(
            path,
            &obj,
            tags::STUDIES_CONTAINING_OTHER_REFERENCED_INSTANCES_SEQUENCE,
        )?,
        1,
    );
    let other_study = top_level_sequence_item(
        path,
        &obj,
        tags::STUDIES_CONTAINING_OTHER_REFERENCED_INSTANCES_SEQUENCE,
        0,
    )?;
    check_equal(
        &mut internal,
        "spatial_registration_other_study_uid",
        "Other-Study reference identifies the moving source Study.",
        "Other-Study reference does not identify the moving source Study.",
        item_str(path, other_study, tags::STUDY_INSTANCE_UID)?.as_str(),
        expected.source.study_instance_uid,
    );
    check_equal(
        &mut internal,
        "spatial_registration_other_study_series_count",
        "Moving source Study contains exactly one referenced series.",
        "Moving source Study reference series count differs from the contract.",
        item_sequence_item_count(path, other_study, tags::REFERENCED_SERIES_SEQUENCE)?,
        1,
    );
    let other_series = item_sequence_item(path, other_study, tags::REFERENCED_SERIES_SEQUENCE, 0)?;
    validate_common_reference_series(
        &mut internal,
        path,
        other_series,
        "spatial_registration_other_study",
        &expected.source,
    )?;

    for (name, tag) in [
        ("spatial_registration_pixel_data_absent", tags::PIXEL_DATA),
        (
            "spatial_registration_float_pixel_data_absent",
            tags::FLOAT_PIXEL_DATA,
        ),
        (
            "spatial_registration_double_float_pixel_data_absent",
            tags::DOUBLE_FLOAT_PIXEL_DATA,
        ),
    ] {
        check(
            &mut internal,
            obj.element_opt(tag)
                .map_err(|err| validation_error(path, err))?
                .is_none(),
            name,
            "Spatial Registration contains no pixel payload.",
            "Spatial Registration unexpectedly contains pixel payload.",
        );
    }

    fail_if_any_failed(path, &internal)?;
    Ok(ValidatedPart10 {
        bytes,
        validation: serde_json::json!({
            "status": "passed",
            "internal": internal,
            "standards": [
                {
                    "name": standard_sop_class_validation_name(expected.sop_class_uid),
                    "status": "passed",
                    "message": standard_sop_class_validation_message(expected.sop_class_uid)
                },
                {
                    "name": standard_transfer_syntax_validation_name(expected.transfer_syntax_uid),
                    "status": "passed",
                    "message": standard_transfer_syntax_validation_message(expected.transfer_syntax_uid)
                },
                {
                    "name": "spatial_registration_rigid_contract",
                    "status": "passed",
                    "message": "Ordered references, exact rigid matrices, landmark mapping, Common Instance References, and no-pixel invariants match the recipe."
                }
            ],
            "external": []
        }),
    })
}

#[allow(clippy::too_many_arguments)]
fn validate_spatial_registration_item(
    results: &mut Vec<Value>,
    path: &Path,
    item: &DatasetObject,
    prefix: &str,
    expected_reference: &SpatialRegistrationReferenceExpectations<'_>,
    expected_matrix: &[f64; 16],
    input_landmark: [f64; 3],
    output_landmark: [f64; 3],
    tolerance: f64,
) -> Result<(), GenerateError> {
    check_equal(
        results,
        &format!("{prefix}_frame_of_reference_uid"),
        "Registration item Frame of Reference matches the recipe.",
        "Registration item Frame of Reference does not match the recipe.",
        item_str(path, item, tags::FRAME_OF_REFERENCE_UID)?.as_str(),
        expected_reference.frame_of_reference_uid,
    );
    check_equal(
        results,
        &format!("{prefix}_referenced_image_count"),
        "Registration item references exactly one complete image instance.",
        "Registration item image-reference cardinality differs from the contract.",
        item_sequence_item_count(path, item, tags::REFERENCED_IMAGE_SEQUENCE)?,
        1,
    );
    let image = item_sequence_item(path, item, tags::REFERENCED_IMAGE_SEQUENCE, 0)?;
    check_equal(
        results,
        &format!("{prefix}_referenced_sop_class_uid"),
        "Referenced image SOP Class UID matches the recipe.",
        "Referenced image SOP Class UID does not match the recipe.",
        item_str(path, image, tags::REFERENCED_SOP_CLASS_UID)?.as_str(),
        expected_reference.sop_class_uid,
    );
    check_equal(
        results,
        &format!("{prefix}_referenced_sop_instance_uid"),
        "Referenced image SOP Instance UID matches the recipe.",
        "Referenced image SOP Instance UID does not match the recipe.",
        item_str(path, image, tags::REFERENCED_SOP_INSTANCE_UID)?.as_str(),
        expected_reference.sop_instance_uid,
    );
    check(
        results,
        image
            .element_opt(TAG_REFERENCED_FRAME_NUMBER)
            .map_err(|err| validation_error(path, err))?
            .is_none(),
        &format!("{prefix}_complete_instance_reference"),
        "Referenced Frame Number is absent, selecting the complete instance.",
        "Referenced Frame Number is present despite the complete-instance contract.",
    );
    check_equal(
        results,
        &format!("{prefix}_matrix_registration_count"),
        "Matrix Registration Sequence contains exactly one item.",
        "Matrix Registration Sequence cardinality differs from the contract.",
        item_sequence_item_count(path, item, tags::MATRIX_REGISTRATION_SEQUENCE)?,
        1,
    );
    let registration = item_sequence_item(path, item, tags::MATRIX_REGISTRATION_SEQUENCE, 0)?;
    check_equal(
        results,
        &format!("{prefix}_registration_type_code_count"),
        "Type 2 Registration Type Code Sequence is present and empty.",
        "Registration Type Code Sequence is absent or nonempty.",
        item_sequence_item_count(path, registration, tags::REGISTRATION_TYPE_CODE_SEQUENCE)?,
        0,
    );
    check_equal(
        results,
        &format!("{prefix}_matrix_sequence_count"),
        "Matrix Sequence contains exactly one item.",
        "Matrix Sequence cardinality differs from the contract.",
        item_sequence_item_count(path, registration, tags::MATRIX_SEQUENCE)?,
        1,
    );
    let matrix_item = item_sequence_item(path, registration, tags::MATRIX_SEQUENCE, 0)?;
    check_equal(
        results,
        &format!("{prefix}_matrix_type"),
        "Frame of Reference Transformation Matrix Type is RIGID.",
        "Frame of Reference Transformation Matrix Type is not RIGID.",
        item_str(
            path,
            matrix_item,
            tags::FRAME_OF_REFERENCE_TRANSFORMATION_MATRIX_TYPE,
        )?
        .as_str(),
        "RIGID",
    );
    let matrix_element = matrix_item
        .element(tags::FRAME_OF_REFERENCE_TRANSFORMATION_MATRIX)
        .map_err(|err| validation_error(path, err))?;
    check_equal(
        results,
        &format!("{prefix}_matrix_vr"),
        "Frame of Reference Transformation Matrix has VR DS.",
        "Frame of Reference Transformation Matrix does not have VR DS.",
        matrix_element.vr(),
        VR::DS,
    );
    let matrix = matrix_element
        .value()
        .to_multi_float64()
        .map_err(|err| validation_error(path, err))?;
    check_equal(
        results,
        &format!("{prefix}_matrix_vm"),
        "Frame of Reference Transformation Matrix has VM 16.",
        "Frame of Reference Transformation Matrix does not have VM 16.",
        matrix.len(),
        16,
    );
    if matrix.len() != 16 {
        return fail_if_any_failed(path, results);
    }
    check(
        results,
        matrix.iter().all(|value| value.is_finite()),
        &format!("{prefix}_matrix_finite"),
        "Every matrix value is finite.",
        "The matrix contains a non-finite value.",
    );
    check_equal(
        results,
        &format!("{prefix}_matrix_exact"),
        "Matrix values and row-major order exactly match the recipe.",
        "Matrix values or row-major order differ from the recipe.",
        matrix.as_slice(),
        expected_matrix.as_slice(),
    );
    let homogeneous = close(matrix[12], 0.0, tolerance)
        && close(matrix[13], 0.0, tolerance)
        && close(matrix[14], 0.0, tolerance)
        && close(matrix[15], 1.0, tolerance);
    check(
        results,
        homogeneous,
        &format!("{prefix}_matrix_homogeneous_row"),
        "Homogeneous matrix final row is [0,0,0,1].",
        "Homogeneous matrix final row is not [0,0,0,1].",
    );
    let rotation = [
        [matrix[0], matrix[1], matrix[2]],
        [matrix[4], matrix[5], matrix[6]],
        [matrix[8], matrix[9], matrix[10]],
    ];
    let orthonormal = (0..3).all(|row| {
        (0..3).all(|other| {
            let dot = (0..3)
                .map(|column| rotation[row][column] * rotation[other][column])
                .sum::<f64>();
            close(dot, if row == other { 1.0 } else { 0.0 }, tolerance)
        })
    });
    check(
        results,
        orthonormal,
        &format!("{prefix}_matrix_orthonormal"),
        "RIGID rotation submatrix is orthonormal.",
        "RIGID rotation submatrix is not orthonormal.",
    );
    let determinant = rotation[0][0]
        * (rotation[1][1] * rotation[2][2] - rotation[1][2] * rotation[2][1])
        - rotation[0][1] * (rotation[1][0] * rotation[2][2] - rotation[1][2] * rotation[2][0])
        + rotation[0][2] * (rotation[1][0] * rotation[2][1] - rotation[1][1] * rotation[2][0]);
    check(
        results,
        close(determinant, 1.0, tolerance),
        &format!("{prefix}_matrix_determinant"),
        "RIGID rotation determinant is +1.",
        "RIGID rotation determinant is not +1.",
    );
    let transformed = [
        matrix[0] * input_landmark[0]
            + matrix[1] * input_landmark[1]
            + matrix[2] * input_landmark[2]
            + matrix[3],
        matrix[4] * input_landmark[0]
            + matrix[5] * input_landmark[1]
            + matrix[6] * input_landmark[2]
            + matrix[7],
        matrix[8] * input_landmark[0]
            + matrix[9] * input_landmark[1]
            + matrix[10] * input_landmark[2]
            + matrix[11],
    ];
    check(
        results,
        transformed
            .iter()
            .zip(output_landmark)
            .all(|(actual, expected)| close(*actual, expected, tolerance)),
        &format!("{prefix}_landmark"),
        "Matrix maps the locked landmark to the expected registered point.",
        "Matrix does not map the locked landmark to the expected registered point.",
    );
    Ok(())
}

fn validate_common_reference_series(
    results: &mut Vec<Value>,
    path: &Path,
    series: &DatasetObject,
    prefix: &str,
    expected: &SpatialRegistrationReferenceExpectations<'_>,
) -> Result<(), GenerateError> {
    check_equal(
        results,
        &format!("{prefix}_series_instance_uid"),
        "Common Instance Reference Series UID matches the recipe.",
        "Common Instance Reference Series UID does not match the recipe.",
        item_str(path, series, tags::SERIES_INSTANCE_UID)?.as_str(),
        expected.series_instance_uid,
    );
    check_equal(
        results,
        &format!("{prefix}_instance_count"),
        "Referenced series contains exactly one instance.",
        "Referenced series instance count differs from the contract.",
        item_sequence_item_count(path, series, tags::REFERENCED_INSTANCE_SEQUENCE)?,
        1,
    );
    let instance = item_sequence_item(path, series, tags::REFERENCED_INSTANCE_SEQUENCE, 0)?;
    check_equal(
        results,
        &format!("{prefix}_sop_class_uid"),
        "Common Instance Reference SOP Class UID matches the recipe.",
        "Common Instance Reference SOP Class UID does not match the recipe.",
        item_str(path, instance, tags::REFERENCED_SOP_CLASS_UID)?.as_str(),
        expected.sop_class_uid,
    );
    check_equal(
        results,
        &format!("{prefix}_sop_instance_uid"),
        "Common Instance Reference SOP Instance UID matches the recipe.",
        "Common Instance Reference SOP Instance UID does not match the recipe.",
        item_str(path, instance, tags::REFERENCED_SOP_INSTANCE_UID)?.as_str(),
        expected.sop_instance_uid,
    );
    Ok(())
}

fn close(actual: f64, expected: f64, tolerance: f64) -> bool {
    actual.is_finite() && expected.is_finite() && (actual - expected).abs() <= tolerance
}

pub(crate) fn validate_deformable_spatial_registration_file(
    path: &Path,
    expected: &DeformableSpatialRegistrationExpectations<'_>,
) -> Result<ValidatedPart10, GenerateError> {
    let bytes = fs::read(path).map_err(|source| GenerateError::ReadGeneratedFile {
        path: path.to_path_buf(),
        source,
    })?;
    let obj = open_file(path).map_err(|err| validation_error(path, err))?;
    let mut internal = Vec::new();

    check(
        &mut internal,
        bytes.len() >= 132 && &bytes[128..132] == b"DICM",
        "deformable_registration_part10_preamble",
        "Deformable Spatial Registration has a Part 10 preamble and DICM marker.",
        "Deformable Spatial Registration is missing its Part 10 preamble or DICM marker.",
    );
    check_equal(
        &mut internal,
        "deformable_registration_transfer_syntax",
        "File Meta Transfer Syntax matches Explicit VR Little Endian.",
        "File Meta Transfer Syntax does not match the recipe.",
        trim_uid(obj.meta().transfer_syntax()).as_str(),
        expected.transfer_syntax_uid,
    );
    let dataset_sop_class = element_str(path, &obj, tags::SOP_CLASS_UID)?;
    check_equal(
        &mut internal,
        "deformable_registration_sop_class_uid",
        "Dataset SOP Class UID matches the recipe.",
        "Dataset SOP Class UID does not match the recipe.",
        dataset_sop_class.as_str(),
        expected.sop_class_uid,
    );
    check_equal(
        &mut internal,
        "deformable_registration_storage_uid",
        "SOP Class is Deformable Spatial Registration Storage.",
        "SOP Class is not Deformable Spatial Registration Storage.",
        dataset_sop_class.as_str(),
        "1.2.840.10008.5.1.4.1.1.66.3",
    );
    check_equal(
        &mut internal,
        "deformable_registration_media_sop_class_uid",
        "File Meta and dataset SOP Class UIDs match.",
        "File Meta and dataset SOP Class UIDs differ.",
        trim_uid(obj.meta().media_storage_sop_class_uid()).as_str(),
        dataset_sop_class.as_str(),
    );
    let dataset_sop_instance = element_str(path, &obj, tags::SOP_INSTANCE_UID)?;
    check_equal(
        &mut internal,
        "deformable_registration_sop_instance_uid",
        "Dataset SOP Instance UID matches the recipe.",
        "Dataset SOP Instance UID does not match the recipe.",
        dataset_sop_instance.as_str(),
        expected.sop_instance_uid,
    );
    check_equal(
        &mut internal,
        "deformable_registration_media_sop_instance_uid",
        "File Meta and dataset SOP Instance UIDs match.",
        "File Meta and dataset SOP Instance UIDs differ.",
        trim_uid(obj.meta().media_storage_sop_instance_uid()).as_str(),
        dataset_sop_instance.as_str(),
    );
    check_equal(
        &mut internal,
        "deformable_registration_implementation_class_uid",
        "Implementation Class UID matches the native generator.",
        "Implementation Class UID does not match the native generator.",
        trim_uid(obj.meta().implementation_class_uid()).as_str(),
        expected.implementation_class_uid,
    );

    for (name, tag, value) in [
        (
            "synthetic_data",
            tags::SYNTHETIC_DATA,
            expected.synthetic_data,
        ),
        ("patient_id", tags::PATIENT_ID, expected.patient_id),
        (
            "study_instance_uid",
            tags::STUDY_INSTANCE_UID,
            expected.study_instance_uid,
        ),
        ("study_id", tags::STUDY_ID, expected.study_id),
        (
            "series_instance_uid",
            tags::SERIES_INSTANCE_UID,
            expected.series_instance_uid,
        ),
        ("series_number", tags::SERIES_NUMBER, expected.series_number),
        ("laterality", tags::LATERALITY, expected.laterality),
        ("modality", tags::MODALITY, expected.modality),
        (
            "instance_number",
            tags::INSTANCE_NUMBER,
            expected.instance_number,
        ),
        ("content_date", tags::CONTENT_DATE, expected.content_date),
        ("content_time", tags::CONTENT_TIME, expected.content_time),
        ("content_label", tags::CONTENT_LABEL, expected.content_label),
        (
            "content_description",
            tags::CONTENT_DESCRIPTION,
            expected.content_description,
        ),
        (
            "content_creator_name",
            tags::CONTENT_CREATOR_NAME,
            expected.content_creator_name,
        ),
        ("manufacturer", tags::MANUFACTURER, expected.manufacturer),
        (
            "manufacturer_model_name",
            tags::MANUFACTURER_MODEL_NAME,
            expected.manufacturer_model_name,
        ),
        (
            "device_serial_number",
            tags::DEVICE_SERIAL_NUMBER,
            expected.device_serial_number,
        ),
        (
            "software_versions",
            tags::SOFTWARE_VERSIONS,
            expected.software_versions,
        ),
        (
            "registered_frame_of_reference_uid",
            tags::FRAME_OF_REFERENCE_UID,
            expected.registered_frame_of_reference_uid,
        ),
    ] {
        check_equal(
            &mut internal,
            &format!("deformable_registration_{name}"),
            "Deformable Spatial Registration module attribute matches the recipe.",
            "Deformable Spatial Registration module attribute does not match the recipe.",
            element_str(path, &obj, tag)?.as_str(),
            value,
        );
    }
    check_equal(
        &mut internal,
        "deformable_registration_target_study_uid",
        "Registered target belongs to the registration Study.",
        "Registered target Study does not match the registration Study.",
        expected.study_instance_uid,
        expected.target.study_instance_uid,
    );
    check_equal(
        &mut internal,
        "deformable_registration_target_frame_of_reference_uid",
        "Registered target Frame of Reference establishes the Registered RCS.",
        "Registered target Frame of Reference does not establish the Registered RCS.",
        expected.registered_frame_of_reference_uid,
        expected.target.frame_of_reference_uid,
    );
    for (name, tag) in [
        ("study_date", tags::STUDY_DATE),
        ("study_time", tags::STUDY_TIME),
        ("referring_physician", tags::REFERRING_PHYSICIAN_NAME),
        ("accession_number", tags::ACCESSION_NUMBER),
        (
            "position_reference_indicator",
            tags::POSITION_REFERENCE_INDICATOR,
        ),
    ] {
        check(
            &mut internal,
            obj.element_opt(tag)
                .map_err(|err| validation_error(path, err))?
                .is_some(),
            &format!("deformable_registration_{name}"),
            "Required Type 1 or Type 2 module attribute is present.",
            "Required Type 1 or Type 2 module attribute is absent.",
        );
    }

    check_equal(
        &mut internal,
        "deformable_registration_item_count",
        "Deformable Registration Sequence contains exactly one source item.",
        "Deformable Registration Sequence does not contain exactly one source item.",
        sequence_item_count(path, &obj, tags::DEFORMABLE_REGISTRATION_SEQUENCE)?,
        1,
    );
    let registration =
        top_level_sequence_item(path, &obj, tags::DEFORMABLE_REGISTRATION_SEQUENCE, 0)?;
    check_equal(
        &mut internal,
        "deformable_registration_source_frame_of_reference_uid",
        "Source Frame of Reference UID matches the referenced source image.",
        "Source Frame of Reference UID is inconsistent with the referenced source image.",
        item_str(path, registration, tags::SOURCE_FRAME_OF_REFERENCE_UID)?.as_str(),
        expected.source.frame_of_reference_uid,
    );
    check(
        &mut internal,
        expected.source.frame_of_reference_uid != expected.registered_frame_of_reference_uid,
        "deformable_registration_distinct_frames_of_reference",
        "Source and Registered Frames of Reference are distinct.",
        "Source and Registered Frames of Reference unexpectedly match.",
    );
    check_equal(
        &mut internal,
        "deformable_registration_referenced_image_count",
        "The source item references exactly one complete image instance.",
        "The source image reference cardinality differs from the contract.",
        item_sequence_item_count(path, registration, tags::REFERENCED_IMAGE_SEQUENCE)?,
        1,
    );
    let source_image = item_sequence_item(path, registration, tags::REFERENCED_IMAGE_SEQUENCE, 0)?;
    validate_deformable_sop_reference(
        &mut internal,
        path,
        source_image,
        "deformable_registration_source",
        &expected.source,
    )?;
    check(
        &mut internal,
        source_image
            .element_opt(TAG_REFERENCED_FRAME_NUMBER)
            .map_err(|err| validation_error(path, err))?
            .is_none(),
        "deformable_registration_complete_source_instance",
        "Referenced Frame Number is absent, selecting the complete source instance.",
        "Referenced Frame Number is present despite the complete-instance contract.",
    );
    check_equal(
        &mut internal,
        "deformable_registration_type_code_count",
        "Type 2 Registration Type Code Sequence is present and empty.",
        "Registration Type Code Sequence is absent or nonempty.",
        item_sequence_item_count(path, registration, tags::REGISTRATION_TYPE_CODE_SEQUENCE)?,
        0,
    );
    validate_deformation_matrix_sequence(
        &mut internal,
        path,
        registration,
        tags::PRE_DEFORMATION_MATRIX_REGISTRATION_SEQUENCE,
        "deformable_registration_pre",
        &expected.pre_matrix,
    )?;
    validate_deformation_matrix_sequence(
        &mut internal,
        path,
        registration,
        tags::POST_DEFORMATION_MATRIX_REGISTRATION_SEQUENCE,
        "deformable_registration_post",
        &expected.post_matrix,
    )?;

    check_equal(
        &mut internal,
        "deformable_registration_grid_count",
        "Deformable Registration Grid Sequence contains exactly one item.",
        "Deformable Registration Grid Sequence does not contain exactly one item.",
        item_sequence_item_count(
            path,
            registration,
            tags::DEFORMABLE_REGISTRATION_GRID_SEQUENCE,
        )?,
        1,
    );
    let grid = item_sequence_item(
        path,
        registration,
        tags::DEFORMABLE_REGISTRATION_GRID_SEQUENCE,
        0,
    )?;
    let orientation_element = grid
        .element(tags::IMAGE_ORIENTATION_PATIENT)
        .map_err(|err| validation_error(path, err))?;
    let orientation = orientation_element
        .value()
        .to_multi_float64()
        .map_err(|err| validation_error(path, err))?;
    check_equal(
        &mut internal,
        "deformable_registration_grid_orientation_vr",
        "Grid orientation has VR DS.",
        "Grid orientation does not have VR DS.",
        orientation_element.vr(),
        VR::DS,
    );
    check_equal(
        &mut internal,
        "deformable_registration_grid_orientation_vm",
        "Grid orientation has VM 6.",
        "Grid orientation does not have VM 6.",
        orientation.len(),
        6,
    );
    check_equal(
        &mut internal,
        "deformable_registration_grid_orientation_exact",
        "Grid orientation matches the locked axial direction.",
        "Grid orientation differs from the locked axial direction.",
        orientation.as_slice(),
        expected.image_orientation_patient.as_slice(),
    );
    let position_element = grid
        .element(tags::IMAGE_POSITION_PATIENT)
        .map_err(|err| validation_error(path, err))?;
    let position = position_element
        .value()
        .to_multi_float64()
        .map_err(|err| validation_error(path, err))?;
    check_equal(
        &mut internal,
        "deformable_registration_grid_origin_vr",
        "Grid origin has VR DS.",
        "Grid origin does not have VR DS.",
        position_element.vr(),
        VR::DS,
    );
    check_equal(
        &mut internal,
        "deformable_registration_grid_origin_vm",
        "Grid origin has VM 3.",
        "Grid origin does not have VM 3.",
        position.len(),
        3,
    );
    check_equal(
        &mut internal,
        "deformable_registration_grid_origin_exact",
        "Grid origin matches the registered frame-2 origin.",
        "Grid origin differs from the registered frame-2 origin.",
        position.as_slice(),
        expected.image_position_patient.as_slice(),
    );

    let dimensions_element = grid
        .element(tags::GRID_DIMENSIONS)
        .map_err(|err| validation_error(path, err))?;
    let dimensions = dimensions_element
        .value()
        .to_multi_int::<u32>()
        .map_err(|err| validation_error(path, err))?;
    check_equal(
        &mut internal,
        "deformable_registration_grid_dimensions_vr",
        "Grid Dimensions has VR UL.",
        "Grid Dimensions does not have VR UL.",
        dimensions_element.vr(),
        VR::UL,
    );
    check_equal(
        &mut internal,
        "deformable_registration_grid_dimensions_vm",
        "Grid Dimensions has VM 3.",
        "Grid Dimensions does not have VM 3.",
        dimensions.len(),
        3,
    );
    check_equal(
        &mut internal,
        "deformable_registration_grid_dimensions_exact",
        "Grid Dimensions match the recipe.",
        "Grid Dimensions differ from the recipe.",
        dimensions.as_slice(),
        expected.grid_dimensions.as_slice(),
    );
    check(
        &mut internal,
        dimensions.len() == 3 && dimensions.iter().all(|value| *value > 0),
        "deformable_registration_grid_dimensions_positive",
        "Every Grid Dimension is positive.",
        "At least one Grid Dimension is zero.",
    );

    let resolution_element = grid
        .element(tags::GRID_RESOLUTION)
        .map_err(|err| validation_error(path, err))?;
    let resolution = resolution_element
        .value()
        .to_multi_float64()
        .map_err(|err| validation_error(path, err))?;
    check_equal(
        &mut internal,
        "deformable_registration_grid_resolution_vr",
        "Grid Resolution has VR FD.",
        "Grid Resolution does not have VR FD.",
        resolution_element.vr(),
        VR::FD,
    );
    check_equal(
        &mut internal,
        "deformable_registration_grid_resolution_vm",
        "Grid Resolution has VM 3.",
        "Grid Resolution does not have VM 3.",
        resolution.len(),
        3,
    );
    check_equal(
        &mut internal,
        "deformable_registration_grid_resolution_exact",
        "Grid Resolution matches the recipe.",
        "Grid Resolution differs from the recipe.",
        resolution.as_slice(),
        expected.grid_resolution.as_slice(),
    );
    check(
        &mut internal,
        resolution.len() == 3
            && resolution
                .iter()
                .all(|value| value.is_finite() && *value > 0.0),
        "deformable_registration_grid_resolution_positive",
        "Every Grid Resolution value is finite and positive.",
        "At least one Grid Resolution value is non-finite or non-positive.",
    );

    let vector_element = grid
        .element(tags::VECTOR_GRID_DATA)
        .map_err(|err| validation_error(path, err))?;
    let vector_bytes = vector_element
        .value()
        .to_bytes()
        .map_err(|err| validation_error(path, err))?;
    check_equal(
        &mut internal,
        "deformable_registration_vector_grid_vr",
        "Vector Grid Data has VR OF.",
        "Vector Grid Data does not have VR OF.",
        vector_element.vr(),
        VR::OF,
    );
    check_equal(
        &mut internal,
        "deformable_registration_vector_grid_vm",
        "Vector Grid Data has VM 1.",
        "Vector Grid Data does not have VM 1.",
        vector_element.vr(),
        VR::OF,
    );
    let expected_byte_len = dimensions
        .iter()
        .try_fold(3_u64 * 4, |product, value| {
            product.checked_mul(u64::from(*value))
        })
        .and_then(|value| usize::try_from(value).ok());
    check_equal(
        &mut internal,
        "deformable_registration_vector_grid_byte_count_equation",
        "Vector byte count equals X_D * Y_D * Z_D * 3 * 4.",
        "Vector byte count violates X_D * Y_D * Z_D * 3 * 4.",
        Some(vector_bytes.len()),
        expected_byte_len,
    );
    check_equal(
        &mut internal,
        "deformable_registration_vector_grid_payload_sha256",
        "Raw little-endian OF payload hash matches the recipe.",
        "Raw OF payload bytes or byte order differ from the recipe.",
        sha256_hex(vector_bytes.as_ref()).as_str(),
        expected.vector_grid_data_sha256,
    );
    let decoded = vector_bytes
        .chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect::<Vec<_>>();
    check(
        &mut internal,
        vector_bytes.len() % 4 == 0,
        "deformable_registration_vector_grid_binary32_alignment",
        "OF payload contains complete binary32 values.",
        "OF payload ends with a partial binary32 value.",
    );
    let expected_flat = expected
        .decoded_vectors_mm
        .iter()
        .flatten()
        .copied()
        .collect::<Vec<_>>();
    check_equal(
        &mut internal,
        "deformable_registration_vector_grid_decoded_values",
        "Decoded vector triples and i/j/k order exactly match the recipe.",
        "Decoded vector values or vector order differ from the recipe.",
        decoded.as_slice(),
        expected_flat.as_slice(),
    );
    let triples_well_formed = decoded.chunks_exact(3).all(|triple| {
        triple.iter().all(|value| value.is_finite()) || triple.iter().all(|value| value.is_nan())
    });
    check(
        &mut internal,
        decoded.len() % 3 == 0 && triples_well_formed,
        "deformable_registration_vector_grid_finite_or_all_nan",
        "Every vector triple is wholly finite or wholly NaN.",
        "A vector triple mixes NaN and finite components.",
    );
    check(
        &mut internal,
        decoded.iter().all(|value| value.is_finite()),
        "deformable_registration_vector_grid_all_finite",
        "Every recipe vector component is finite.",
        "The recipe payload contains a NaN or infinite component.",
    );

    validate_deformable_point_mappings(&mut internal, expected, &decoded);

    check_equal(
        &mut internal,
        "deformable_registration_same_study_series_count",
        "Same-Study references contain only the registered target series.",
        "Same-Study reference series count differs from the contract.",
        sequence_item_count(path, &obj, tags::REFERENCED_SERIES_SEQUENCE)?,
        1,
    );
    let same_study = top_level_sequence_item(path, &obj, tags::REFERENCED_SERIES_SEQUENCE, 0)?;
    validate_common_reference_series(
        &mut internal,
        path,
        same_study,
        "deformable_registration_same_study",
        &expected.target,
    )?;
    check_equal(
        &mut internal,
        "deformable_registration_other_study_count",
        "Other-Study references contain only the source Study.",
        "Other-Study reference count differs from the contract.",
        sequence_item_count(
            path,
            &obj,
            tags::STUDIES_CONTAINING_OTHER_REFERENCED_INSTANCES_SEQUENCE,
        )?,
        1,
    );
    let other_study = top_level_sequence_item(
        path,
        &obj,
        tags::STUDIES_CONTAINING_OTHER_REFERENCED_INSTANCES_SEQUENCE,
        0,
    )?;
    check_equal(
        &mut internal,
        "deformable_registration_other_study_uid",
        "Other-Study reference identifies the source Study.",
        "Other-Study reference does not identify the source Study.",
        item_str(path, other_study, tags::STUDY_INSTANCE_UID)?.as_str(),
        expected.source.study_instance_uid,
    );
    check_equal(
        &mut internal,
        "deformable_registration_other_study_series_count",
        "Source Study contains exactly one referenced series.",
        "Source Study reference series count differs from the contract.",
        item_sequence_item_count(path, other_study, tags::REFERENCED_SERIES_SEQUENCE)?,
        1,
    );
    let source_series = item_sequence_item(path, other_study, tags::REFERENCED_SERIES_SEQUENCE, 0)?;
    validate_common_reference_series(
        &mut internal,
        path,
        source_series,
        "deformable_registration_other_study",
        &expected.source,
    )?;

    for (name, tag) in [
        ("pixel_data_absent", tags::PIXEL_DATA),
        ("float_pixel_data_absent", tags::FLOAT_PIXEL_DATA),
        (
            "double_float_pixel_data_absent",
            tags::DOUBLE_FLOAT_PIXEL_DATA,
        ),
    ] {
        check(
            &mut internal,
            obj.element_opt(tag)
                .map_err(|err| validation_error(path, err))?
                .is_none(),
            &format!("deformable_registration_{name}"),
            "Deformable Spatial Registration contains no pixel payload.",
            "Deformable Spatial Registration unexpectedly contains pixel payload.",
        );
    }

    fail_if_any_failed(path, &internal)?;
    Ok(ValidatedPart10 {
        bytes,
        validation: serde_json::json!({
            "status": "passed",
            "internal": internal,
            "standards": [
                {"name": standard_sop_class_validation_name(expected.sop_class_uid), "status": "passed", "message": standard_sop_class_validation_message(expected.sop_class_uid)},
                {"name": standard_transfer_syntax_validation_name(expected.transfer_syntax_uid), "status": "passed", "message": standard_transfer_syntax_validation_message(expected.transfer_syntax_uid)},
                {"name": "deformable_spatial_registration_contract", "status": "passed", "message": "Registered-to-source sampling, exact OF bytes and vector order, reference closure, and no-pixel invariants match the recipe."}
            ],
            "external": []
        }),
    })
}

fn validate_deformable_sop_reference(
    results: &mut Vec<Value>,
    path: &Path,
    reference: &DatasetObject,
    prefix: &str,
    expected: &SpatialRegistrationReferenceExpectations<'_>,
) -> Result<(), GenerateError> {
    check_equal(
        results,
        &format!("{prefix}_sop_class_uid"),
        "Referenced SOP Class UID matches the recipe.",
        "Referenced SOP Class UID differs from the recipe.",
        item_str(path, reference, tags::REFERENCED_SOP_CLASS_UID)?.as_str(),
        expected.sop_class_uid,
    );
    check_equal(
        results,
        &format!("{prefix}_sop_instance_uid"),
        "Referenced SOP Instance UID matches the recipe.",
        "Referenced SOP Instance UID differs from the recipe.",
        item_str(path, reference, tags::REFERENCED_SOP_INSTANCE_UID)?.as_str(),
        expected.sop_instance_uid,
    );
    Ok(())
}

fn validate_deformation_matrix_sequence(
    results: &mut Vec<Value>,
    path: &Path,
    registration: &DatasetObject,
    sequence_tag: Tag,
    prefix: &str,
    expected: &[f64; 16],
) -> Result<(), GenerateError> {
    check_equal(
        results,
        &format!("{prefix}_matrix_count"),
        "Deformation matrix sequence contains exactly one item.",
        "Deformation matrix sequence cardinality differs from the contract.",
        item_sequence_item_count(path, registration, sequence_tag)?,
        1,
    );
    let item = item_sequence_item(path, registration, sequence_tag, 0)?;
    check_equal(
        results,
        &format!("{prefix}_matrix_type"),
        "Deformation matrix type is RIGID.",
        "Deformation matrix type is not RIGID.",
        item_str(
            path,
            item,
            tags::FRAME_OF_REFERENCE_TRANSFORMATION_MATRIX_TYPE,
        )?
        .as_str(),
        "RIGID",
    );
    let element = item
        .element(tags::FRAME_OF_REFERENCE_TRANSFORMATION_MATRIX)
        .map_err(|err| validation_error(path, err))?;
    check_equal(
        results,
        &format!("{prefix}_matrix_vr"),
        "Deformation matrix has VR DS.",
        "Deformation matrix does not have VR DS.",
        element.vr(),
        VR::DS,
    );
    let values = element
        .value()
        .to_multi_float64()
        .map_err(|err| validation_error(path, err))?;
    check_equal(
        results,
        &format!("{prefix}_matrix_vm"),
        "Deformation matrix has VM 16.",
        "Deformation matrix does not have VM 16.",
        values.len(),
        16,
    );
    check(
        results,
        values.iter().all(|value| value.is_finite()),
        &format!("{prefix}_matrix_finite"),
        "Every deformation matrix value is finite.",
        "Deformation matrix contains a non-finite value.",
    );
    check_equal(
        results,
        &format!("{prefix}_matrix_identity"),
        "Deformation matrix is the exact row-major identity.",
        "Deformation matrix is not the exact row-major identity.",
        values.as_slice(),
        expected.as_slice(),
    );
    Ok(())
}

fn validate_deformable_point_mappings(
    results: &mut Vec<Value>,
    expected: &DeformableSpatialRegistrationExpectations<'_>,
    decoded: &[f32],
) {
    let mapping_count_matches = expected.registered_points_mm.len()
        == expected.source_points_mm.len()
        && expected.registered_points_mm.len() == expected.decoded_vectors_mm.len()
        && decoded.len() == expected.registered_points_mm.len() * 3;
    check(
        results,
        mapping_count_matches,
        "deformable_registration_point_mapping_count",
        "Point mappings cover every grid voxel.",
        "Point mapping cardinality differs from the grid vector count.",
    );
    let derived_registered_points = deformable_grid_centers(expected);
    let grid_centers_match = derived_registered_points.len() == expected.registered_points_mm.len()
        && derived_registered_points
            .iter()
            .zip(expected.registered_points_mm)
            .all(|(actual, locked)| {
                actual
                    .iter()
                    .zip(locked)
                    .all(|(actual, locked)| close(*actual, *locked, expected.tolerance))
            });
    check(
        results,
        grid_centers_match,
        "deformable_registration_grid_center_order",
        "Grid origin, orientation, dimensions, and resolution produce the locked i-fastest registered points.",
        "Grid geometry does not produce the locked i/j/k point order.",
    );
    let mappings_match = mapping_count_matches
        && expected
            .registered_points_mm
            .iter()
            .zip(expected.source_points_mm)
            .zip(decoded.chunks_exact(3))
            .all(|((registered, source), vector)| {
                let pre = apply_affine(&expected.pre_matrix, *registered);
                let displaced = [
                    pre[0] + f64::from(vector[0]),
                    pre[1] + f64::from(vector[1]),
                    pre[2] + f64::from(vector[2]),
                ];
                let actual = apply_affine(&expected.post_matrix, displaced);
                actual
                    .iter()
                    .zip(source)
                    .all(|(actual, locked)| close(*actual, *locked, expected.tolerance))
            });
    check(
        results,
        mappings_match,
        "deformable_registration_registered_to_source_mappings",
        "M_post(M_pre(P_registered)+D) maps every registered grid center to the locked source point.",
        "Registered-to-source sampling direction, vector order, or point mapping differs from the contract.",
    );
}

fn deformable_grid_centers(
    expected: &DeformableSpatialRegistrationExpectations<'_>,
) -> Vec<[f64; 3]> {
    let row = &expected.image_orientation_patient[..3];
    let column = &expected.image_orientation_patient[3..];
    let normal = [
        row[1] * column[2] - row[2] * column[1],
        row[2] * column[0] - row[0] * column[2],
        row[0] * column[1] - row[1] * column[0],
    ];
    let mut points = Vec::new();
    for k in 0..expected.grid_dimensions[2] {
        for j in 0..expected.grid_dimensions[1] {
            for i in 0..expected.grid_dimensions[0] {
                points.push([
                    expected.image_position_patient[0]
                        + f64::from(i) * expected.grid_resolution[0] * row[0]
                        + f64::from(j) * expected.grid_resolution[1] * column[0]
                        + f64::from(k) * expected.grid_resolution[2] * normal[0],
                    expected.image_position_patient[1]
                        + f64::from(i) * expected.grid_resolution[0] * row[1]
                        + f64::from(j) * expected.grid_resolution[1] * column[1]
                        + f64::from(k) * expected.grid_resolution[2] * normal[1],
                    expected.image_position_patient[2]
                        + f64::from(i) * expected.grid_resolution[0] * row[2]
                        + f64::from(j) * expected.grid_resolution[1] * column[2]
                        + f64::from(k) * expected.grid_resolution[2] * normal[2],
                ]);
            }
        }
    }
    points
}

fn apply_affine(matrix: &[f64; 16], point: [f64; 3]) -> [f64; 3] {
    [
        matrix[0] * point[0] + matrix[1] * point[1] + matrix[2] * point[2] + matrix[3],
        matrix[4] * point[0] + matrix[5] * point[1] + matrix[6] * point[2] + matrix[7],
        matrix[8] * point[0] + matrix[9] * point[1] + matrix[10] * point[2] + matrix[11],
    ]
}

pub(crate) fn validate_key_object_selection_file(
    path: &Path,
    expected: &KeyObjectSelectionExpectations<'_>,
) -> Result<ValidatedPart10, GenerateError> {
    let bytes = fs::read(path).map_err(|source| GenerateError::ReadGeneratedFile {
        path: path.to_path_buf(),
        source,
    })?;
    let obj = open_file(path).map_err(|err| GenerateError::ValidateDicomFile {
        path: path.to_path_buf(),
        message: err.to_string(),
    })?;

    let mut internal = Vec::new();
    check(
        &mut internal,
        bytes.len() >= 132 && &bytes[128..132] == b"DICM",
        "part10_preamble",
        "File has a 128-byte preamble followed by the DICM marker.",
        "File is missing the Part 10 DICM marker at byte offset 128.",
    );
    check_equal(
        &mut internal,
        "file_meta_transfer_syntax",
        "File Meta Information Transfer Syntax UID matches the recipe.",
        "File Meta Information Transfer Syntax UID does not match the recipe.",
        trim_uid(obj.meta().transfer_syntax()).as_str(),
        expected.transfer_syntax_uid,
    );

    let dataset_sop_class = element_str(path, &obj, tags::SOP_CLASS_UID)?;
    check_equal(
        &mut internal,
        "sop_class_uid_consistency",
        "Dataset SOP Class UID, File Meta SOP Class UID, and recipe SOP Class UID match.",
        "SOP Class UID differs between dataset, File Meta Information, or recipe.",
        dataset_sop_class.as_str(),
        expected.sop_class_uid,
    );
    check_equal(
        &mut internal,
        "media_storage_sop_class_uid",
        "File Meta SOP Class UID matches the dataset SOP Class UID.",
        "File Meta SOP Class UID does not match the dataset SOP Class UID.",
        trim_uid(obj.meta().media_storage_sop_class_uid()).as_str(),
        dataset_sop_class.as_str(),
    );
    let dataset_sop_instance = element_str(path, &obj, tags::SOP_INSTANCE_UID)?;
    check_equal(
        &mut internal,
        "sop_instance_uid_consistency",
        "Dataset SOP Instance UID, File Meta SOP Instance UID, and manifest UID match.",
        "SOP Instance UID differs between dataset, File Meta Information, or manifest.",
        dataset_sop_instance.as_str(),
        expected.sop_instance_uid,
    );
    check_equal(
        &mut internal,
        "media_storage_sop_instance_uid",
        "File Meta SOP Instance UID matches the dataset SOP Instance UID.",
        "File Meta SOP Instance UID does not match the dataset SOP Instance UID.",
        trim_uid(obj.meta().media_storage_sop_instance_uid()).as_str(),
        dataset_sop_instance.as_str(),
    );
    check_equal(
        &mut internal,
        "implementation_class_uid",
        "File Meta Implementation Class UID matches the deterministic generator UID.",
        "File Meta Implementation Class UID does not match the deterministic generator UID.",
        trim_uid(obj.meta().implementation_class_uid()).as_str(),
        expected.implementation_class_uid,
    );
    check_equal(
        &mut internal,
        "synthetic_data",
        "Synthetic Data is present and set to YES.",
        "Synthetic Data is missing or not set to YES.",
        element_str(path, &obj, tags::SYNTHETIC_DATA)?.as_str(),
        expected.synthetic_data,
    );
    check_equal(
        &mut internal,
        "kos_modality",
        "Key Object Document Series Modality is KO.",
        "Key Object Document Series Modality does not match the recipe.",
        element_str(path, &obj, tags::MODALITY)?.as_str(),
        expected.modality,
    );
    check_equal(
        &mut internal,
        "sr_completion_flag",
        "SR Completion Flag matches the recipe.",
        "SR Completion Flag does not match the recipe.",
        element_str(path, &obj, tags::COMPLETION_FLAG)?.as_str(),
        expected.completion_flag,
    );
    check_equal(
        &mut internal,
        "sr_verification_flag",
        "SR Verification Flag matches the recipe.",
        "SR Verification Flag does not match the recipe.",
        element_str(path, &obj, tags::VERIFICATION_FLAG)?.as_str(),
        expected.verification_flag,
    );

    let evidence = top_level_sequence_item(
        path,
        &obj,
        tags::CURRENT_REQUESTED_PROCEDURE_EVIDENCE_SEQUENCE,
        0,
    )?;
    check_equal(
        &mut internal,
        "sr_evidence_study_instance_uid",
        "KOS evidence references the source Study Instance UID.",
        "KOS evidence Study Instance UID does not match the source.",
        item_str(path, evidence, tags::STUDY_INSTANCE_UID)?.as_str(),
        expected.referenced_study_instance_uid,
    );
    check_equal(
        &mut internal,
        "kos_evidence_series_items",
        "KOS evidence records every source series.",
        "KOS evidence source series count does not match the recipe.",
        item_sequence_item_count(path, evidence, tags::REFERENCED_SERIES_SEQUENCE)?,
        expected.key_objects.len(),
    );
    for (index, key_object) in expected.key_objects.iter().enumerate() {
        let evidence_series =
            item_sequence_item(path, evidence, tags::REFERENCED_SERIES_SEQUENCE, index)?;
        check_equal(
            &mut internal,
            "kos_evidence_series_instance_uid",
            "KOS evidence Series Instance UID matches a source object.",
            "KOS evidence Series Instance UID does not match the source.",
            item_str(path, evidence_series, tags::SERIES_INSTANCE_UID)?.as_str(),
            key_object.referenced_series_instance_uid,
        );
        let evidence_sop =
            item_sequence_item(path, evidence_series, tags::REFERENCED_SOP_SEQUENCE, 0)?;
        check_equal(
            &mut internal,
            "kos_evidence_sop_class_uid",
            "KOS evidence SOP Class UID matches a source object.",
            "KOS evidence SOP Class UID does not match the source.",
            item_str(path, evidence_sop, TAG_REFERENCED_SOP_CLASS_UID)?.as_str(),
            key_object.referenced_sop_class_uid,
        );
        check_equal(
            &mut internal,
            "kos_evidence_sop_instance_uid",
            "KOS evidence SOP Instance UID matches a source object.",
            "KOS evidence SOP Instance UID does not match the source.",
            item_str(path, evidence_sop, TAG_REFERENCED_SOP_INSTANCE_UID)?.as_str(),
            key_object.referenced_sop_instance_uid,
        );
    }

    check_equal(
        &mut internal,
        "sr_root_value_type",
        "KOS root content item Value Type is CONTAINER.",
        "KOS root content item Value Type does not match the recipe.",
        element_str(path, &obj, tags::VALUE_TYPE)?.as_str(),
        expected.root_value_type,
    );
    check_equal(
        &mut internal,
        "sr_root_continuity_of_content",
        "KOS root Continuity of Content matches the recipe.",
        "KOS root Continuity of Content does not match the recipe.",
        element_str(path, &obj, tags::CONTINUITY_OF_CONTENT)?.as_str(),
        expected.root_continuity_of_content,
    );
    let title = top_level_sequence_item(path, &obj, tags::CONCEPT_NAME_CODE_SEQUENCE, 0)?;
    check_equal(
        &mut internal,
        "sr_title_code_value",
        "KOS title Code Value matches the recipe.",
        "KOS title Code Value does not match the recipe.",
        item_str(path, title, tags::CODE_VALUE)?.as_str(),
        expected.title_code_value,
    );
    check_equal(
        &mut internal,
        "sr_title_coding_scheme",
        "KOS title Coding Scheme Designator matches the recipe.",
        "KOS title Coding Scheme Designator does not match the recipe.",
        item_str(path, title, tags::CODING_SCHEME_DESIGNATOR)?.as_str(),
        expected.title_coding_scheme_designator,
    );
    check_equal(
        &mut internal,
        "sr_title_code_meaning",
        "KOS title Code Meaning matches the recipe.",
        "KOS title Code Meaning does not match the recipe.",
        item_str(path, title, tags::CODE_MEANING)?.as_str(),
        expected.title_code_meaning,
    );
    let content_template = top_level_sequence_item(path, &obj, tags::CONTENT_TEMPLATE_SEQUENCE, 0)?;
    check_equal(
        &mut internal,
        "kos_mapping_resource",
        "KOS root identifies the DICOM Content Mapping Resource.",
        "KOS root Mapping Resource does not match the recipe.",
        item_str(path, content_template, tags::MAPPING_RESOURCE)?.as_str(),
        expected.mapping_resource,
    );
    check_equal(
        &mut internal,
        "kos_template_identifier",
        "KOS root identifies TID 2010.",
        "KOS root Template Identifier does not match the recipe.",
        item_str(path, content_template, tags::TEMPLATE_IDENTIFIER)?.as_str(),
        expected.template_identifier,
    );
    check_equal(
        &mut internal,
        "kos_content_sequence_items",
        "KOS Content Sequence contains every key object item.",
        "KOS Content Sequence item count does not match the recipe.",
        sequence_item_count(path, &obj, tags::CONTENT_SEQUENCE)?,
        expected.key_objects.len(),
    );

    for (index, key_object) in expected.key_objects.iter().enumerate() {
        let content_item = top_level_sequence_item(path, &obj, tags::CONTENT_SEQUENCE, index)?;
        check_equal(
            &mut internal,
            "kos_image_relationship_type",
            "KOS IMAGE item Relationship Type matches the recipe.",
            "KOS IMAGE item Relationship Type does not match the recipe.",
            item_str(path, content_item, tags::RELATIONSHIP_TYPE)?.as_str(),
            expected.relationship_type,
        );
        check_equal(
            &mut internal,
            "kos_image_value_type",
            "KOS IMAGE item Value Type is IMAGE.",
            "KOS IMAGE item Value Type does not match the recipe.",
            item_str(path, content_item, tags::VALUE_TYPE)?.as_str(),
            expected.image_value_type,
        );
        check(
            &mut internal,
            content_item
                .element(tags::CONCEPT_NAME_CODE_SEQUENCE)
                .is_err(),
            "kos_image_concept_name_absent",
            "KOS IMAGE item omits Concept Name as required by TID 2010 Row 8.",
            "KOS IMAGE item contains a Concept Name forbidden by TID 2010 Row 8.",
        );
        let image_sop = item_sequence_item(path, content_item, tags::REFERENCED_SOP_SEQUENCE, 0)?;
        check_equal(
            &mut internal,
            "kos_image_sop_class_uid",
            "KOS IMAGE item SOP Class UID matches the source object.",
            "KOS IMAGE item SOP Class UID does not match the source object.",
            item_str(path, image_sop, TAG_REFERENCED_SOP_CLASS_UID)?.as_str(),
            key_object.referenced_sop_class_uid,
        );
        check_equal(
            &mut internal,
            "kos_image_sop_instance_uid",
            "KOS IMAGE item SOP Instance UID matches the source object.",
            "KOS IMAGE item SOP Instance UID does not match the source object.",
            item_str(path, image_sop, TAG_REFERENCED_SOP_INSTANCE_UID)?.as_str(),
            key_object.referenced_sop_instance_uid,
        );
        if let Some(frame_numbers) = key_object.referenced_frame_numbers {
            let expected_frame_numbers = frame_numbers
                .iter()
                .map(u16::to_string)
                .collect::<Vec<_>>()
                .join("\\");
            check_equal(
                &mut internal,
                "kos_image_referenced_frame_numbers",
                "KOS IMAGE item frame numbers match the recipe.",
                "KOS IMAGE item frame numbers do not match the recipe.",
                item_str(path, image_sop, TAG_REFERENCED_FRAME_NUMBER)?.as_str(),
                expected_frame_numbers.as_str(),
            );
        }
    }
    check(
        &mut internal,
        obj.element_opt(tags::PIXEL_DATA)
            .map_err(|err| validation_error(path, err))?
            .is_none(),
        "sr_pixel_data_absent",
        "KOS contains no Pixel Data.",
        "KOS unexpectedly contains Pixel Data.",
    );

    fail_if_any_failed(path, &internal)?;

    Ok(ValidatedPart10 {
        bytes,
        validation: serde_json::json!({
            "status": "passed",
            "internal": internal,
            "standards": [
                {
                    "name": standard_sop_class_validation_name(expected.sop_class_uid),
                    "status": "passed",
                    "message": standard_sop_class_validation_message(expected.sop_class_uid)
                },
                {
                    "name": standard_transfer_syntax_validation_name(expected.transfer_syntax_uid),
                    "status": "passed",
                    "message": standard_transfer_syntax_validation_message(expected.transfer_syntax_uid)
                },
                {
                    "name": "synthetic_data_attribute",
                    "status": "passed",
                    "message": "Synthetic Data (0008,001C) is present with value YES."
                },
                {
                    "name": "key_object_selection_document_modules",
                    "status": "passed",
                    "message": "KOS document flags, evidence, and IMAGE content items match the recipe."
                }
            ],
            "external": []
        }),
    })
}

pub(crate) fn validate_rt_radiation_file(
    path: &Path,
    expected: &RtRadiationExpectations<'_>,
) -> Result<ValidatedPart10, GenerateError> {
    let bytes = fs::read(path).map_err(|source| GenerateError::ReadGeneratedFile {
        path: path.to_path_buf(),
        source,
    })?;
    let obj = open_file(path).map_err(|err| GenerateError::ValidateDicomFile {
        path: path.to_path_buf(),
        message: err.to_string(),
    })?;
    let contract = expected.expected_rt_radiation;
    let mut internal = Vec::new();

    validate_rt_radiation_manifest(&mut internal, contract);
    validate_rt_part10_identity(
        path,
        &obj,
        &mut internal,
        contract.sop_class_uid,
        contract.sop_instance_uid,
        contract.transfer_syntax_uid,
        expected.implementation_class_uid,
        expected.synthetic_data,
        contract.study_instance_uid,
        contract.series_instance_uid,
        contract.frame_of_reference_uid,
        contract.modality,
        contract.instance.series_number,
    )?;
    validate_rt_instance_context(path, &obj, &mut internal, contract.instance)?;

    for (name, tag, vr, value) in [
        (
            "rt_radiation_user_content_label",
            tags::USER_CONTENT_LABEL,
            VR::SH,
            contract.content.user_content_label,
        ),
        (
            "rt_radiation_content_description",
            tags::CONTENT_DESCRIPTION,
            VR::LO,
            contract.content.content_description,
        ),
        (
            "rt_radiation_detail_flag",
            tags::RT_RADIATION_PHYSICAL_AND_GEOMETRIC_CONTENT_DETAIL_FLAG,
            VR::CS,
            contract.content.physical_and_geometric_content_detail_flag,
        ),
        (
            "rt_radiation_record_flag",
            tags::RT_RECORD_FLAG,
            VR::CS,
            contract.content.rt_record_flag,
        ),
        (
            "rt_radiation_equipment_for",
            tags::EQUIPMENT_FRAME_OF_REFERENCE_UID,
            VR::UI,
            contract.equipment_frame_of_reference_uid,
        ),
    ] {
        rt_check_top_str(path, &obj, &mut internal, name, tag, vr, value)?;
    }
    rt_check_top_u16(
        path,
        &obj,
        &mut internal,
        "rt_radiation_control_point_count",
        tags::NUMBER_OF_RT_CONTROL_POINTS,
        contract.content.number_of_rt_control_points.into(),
    )?;
    rt_check_top_u16(
        path,
        &obj,
        &mut internal,
        "rt_radiation_patient_support_device_count",
        tags::NUMBER_OF_PATIENT_SUPPORT_DEVICES,
        contract.number_of_patient_support_devices.into(),
    )?;
    rt_check_top_f64(
        path,
        &obj,
        &mut internal,
        "rt_radiation_modifier_distance",
        tags::RT_BEAM_MODIFIER_DEFINITION_DISTANCE,
        VR::FD,
        f64::from(contract.rt_beam_modifier_definition_distance_mm),
    )?;
    rt_check_top_f64(
        path,
        &obj,
        &mut internal,
        "rt_radiation_source_axis_distance",
        tags::RADIATION_SOURCE_AXIS_DISTANCE,
        VR::FD,
        f64::from(contract.radiation_source_axis_distance_mm),
    )?;
    rt_check_top_empty_sequence(
        path,
        &obj,
        &mut internal,
        "rt_radiation_equipment_reference_points_empty",
        tags::EQUIPMENT_REFERENCE_POINT_COORDINATES_SEQUENCE,
    )?;
    rt_check_top_empty_sequence(
        path,
        &obj,
        &mut internal,
        "rt_radiation_author_identification_empty",
        tags::AUTHOR_IDENTIFICATION_SEQUENCE,
    )?;

    validate_rt_top_code(
        path,
        &obj,
        &mut internal,
        "rt_radiation_treatment_technique",
        tags::RT_TREATMENT_TECHNIQUE_CODE_SEQUENCE,
        contract.content.treatment_technique,
    )?;
    validate_rt_top_code(
        path,
        &obj,
        &mut internal,
        "rt_radiation_dosimeter_unit",
        tags::RADIATION_DOSIMETER_UNIT_SEQUENCE,
        contract.dosimeter_unit,
    )?;
    validate_rt_top_code(
        path,
        &obj,
        &mut internal,
        "rt_radiation_distance_reference",
        tags::RT_DEVICE_DISTANCE_REFERENCE_LOCATION_CODE_SEQUENCE,
        contract.distance_reference_location,
    )?;
    validate_rt_top_code(
        path,
        &obj,
        &mut internal,
        "rt_radiation_patient_orientation",
        tags::PATIENT_ORIENTATION_CODE_SEQUENCE,
        contract.patient_orientation,
    )?;
    let orientation =
        top_level_sequence_item(path, &obj, tags::PATIENT_ORIENTATION_CODE_SEQUENCE, 0)?;
    validate_rt_item_code(
        path,
        orientation,
        &mut internal,
        "rt_radiation_orientation_modifier",
        tags::PATIENT_ORIENTATION_MODIFIER_CODE_SEQUENCE,
        contract.patient_orientation_modifier,
    )?;
    validate_rt_top_code(
        path,
        &obj,
        &mut internal,
        "rt_radiation_patient_equipment_relationship",
        tags::PATIENT_EQUIPMENT_RELATIONSHIP_CODE_SEQUENCE,
        contract.patient_equipment_relationship,
    )?;

    rt_check_top_sequence_count(
        path,
        &obj,
        &mut internal,
        "rt_radiation_definition_source_count",
        tags::DEFINITION_SOURCE_SEQUENCE,
        1,
    )?;
    rt_check_top_sequence_count(
        path,
        &obj,
        &mut internal,
        "rt_radiation_device_count",
        tags::TREATMENT_DEVICE_IDENTIFICATION_SEQUENCE,
        1,
    )?;
    rt_check_top_sequence_count(
        path,
        &obj,
        &mut internal,
        "rt_radiation_treatment_position_count",
        tags::TREATMENT_POSITION_SEQUENCE,
        contract.treatment_positions.len(),
    )?;
    rt_check_top_sequence_count(
        path,
        &obj,
        &mut internal,
        "rt_radiation_control_point_sequence_count",
        tags::C_ARM_PHOTON_ELECTRON_CONTROL_POINT_SEQUENCE,
        contract.control_points.len(),
    )?;
    rt_check_top_sequence_count(
        path,
        &obj,
        &mut internal,
        "rt_radiation_common_reference_count",
        tags::REFERENCED_SERIES_SEQUENCE,
        1,
    )?;
    fail_if_any_failed(path, &internal)?;

    let definition = top_level_sequence_item(path, &obj, tags::DEFINITION_SOURCE_SEQUENCE, 0)?;
    validate_rt_sop_reference(
        path,
        definition,
        &mut internal,
        "rt_radiation_definition_source",
        contract.definition_source.sop_class_uid,
        contract.definition_source.sop_instance_uid,
    )?;
    rt_check_item_str(
        path,
        definition,
        &mut internal,
        "rt_radiation_definition_beam",
        tags::REFERENCED_BEAM_NUMBER,
        VR::IS,
        &contract
            .definition_source
            .referenced_beam_number
            .to_string(),
    )?;

    let device = top_level_sequence_item(
        path,
        &obj,
        tags::TREATMENT_DEVICE_IDENTIFICATION_SEQUENCE,
        0,
    )?;
    validate_rt_treatment_device(path, device, &mut internal, contract.device)?;

    let position = top_level_sequence_item(path, &obj, tags::TREATMENT_POSITION_SEQUENCE, 0)?;
    let locked_position = contract.treatment_positions[0];
    rt_check_item_u16(
        path,
        position,
        &mut internal,
        "rt_radiation_position_index",
        tags::TREATMENT_POSITION_INDEX,
        locked_position.treatment_position_index.into(),
    )?;
    rt_check_item_f64_values(
        path,
        position,
        &mut internal,
        "rt_radiation_mapping_matrix",
        tags::IMAGE_TO_EQUIPMENT_MAPPING_MATRIX,
        VR::DS,
        locked_position
            .image_to_equipment_mapping_matrix
            .map(f64::from)
            .to_vec(),
    )?;
    rt_check_item_empty_sequence(
        path,
        position,
        &mut internal,
        "rt_radiation_patient_location_empty",
        tags::PATIENT_LOCATION_COORDINATES_SEQUENCE,
    )?;
    rt_check_item_empty_sequence(
        path,
        position,
        &mut internal,
        "rt_radiation_patient_support_position_empty",
        tags::PATIENT_SUPPORT_POSITION_SEQUENCE,
    )?;

    let first = top_level_sequence_item(
        path,
        &obj,
        tags::C_ARM_PHOTON_ELECTRON_CONTROL_POINT_SEQUENCE,
        0,
    )?;
    let second = top_level_sequence_item(
        path,
        &obj,
        tags::C_ARM_PHOTON_ELECTRON_CONTROL_POINT_SEQUENCE,
        1,
    )?;
    validate_rt_control_point(path, first, &mut internal, contract.control_points[0], true)?;
    validate_rt_control_point(
        path,
        second,
        &mut internal,
        contract.control_points[1],
        false,
    )?;
    validate_rt_common_reference(
        path,
        &obj,
        &mut internal,
        0,
        "rt_radiation_common_plan",
        contract.definition_source.series_instance_uid,
        contract.definition_source.sop_class_uid,
        contract.definition_source.sop_instance_uid,
    )?;

    validate_rt_radiation_absences(path, &obj, &mut internal, contract.absent_content)?;
    fail_if_any_failed(path, &internal)?;
    Ok(rt_validated(
        bytes,
        internal,
        "rt_radiation_sop_class",
        "C-Arm Photon-Electron Radiation modules and references match the locked contract.",
    ))
}

pub(crate) fn validate_rt_radiation_set_file(
    path: &Path,
    expected: &RtRadiationSetExpectations<'_>,
) -> Result<ValidatedPart10, GenerateError> {
    let bytes = fs::read(path).map_err(|source| GenerateError::ReadGeneratedFile {
        path: path.to_path_buf(),
        source,
    })?;
    let obj = open_file(path).map_err(|err| GenerateError::ValidateDicomFile {
        path: path.to_path_buf(),
        message: err.to_string(),
    })?;
    let contract = expected.expected_rt_radiation_set;
    let mut internal = Vec::new();
    validate_rt_radiation_set_manifest(&mut internal, contract);
    validate_rt_part10_identity(
        path,
        &obj,
        &mut internal,
        contract.sop_class_uid,
        contract.sop_instance_uid,
        contract.transfer_syntax_uid,
        expected.implementation_class_uid,
        expected.synthetic_data,
        contract.study_instance_uid,
        contract.series_instance_uid,
        contract.frame_of_reference_uid,
        contract.modality,
        contract.instance.series_number,
    )?;
    validate_rt_instance_context(path, &obj, &mut internal, contract.instance)?;
    for (name, tag, vr, value) in [
        (
            "rt_radiation_set_user_content_label",
            tags::USER_CONTENT_LABEL,
            VR::SH,
            contract.content.user_content_label,
        ),
        (
            "rt_radiation_set_content_description",
            tags::CONTENT_DESCRIPTION,
            VR::LO,
            contract.content.content_description,
        ),
        (
            "rt_radiation_set_intent",
            tags::RT_RADIATION_SET_INTENT,
            VR::CS,
            contract.content.intent,
        ),
    ] {
        rt_check_top_str(path, &obj, &mut internal, name, tag, vr, value)?;
    }
    rt_check_top_u16(
        path,
        &obj,
        &mut internal,
        "rt_radiation_set_fraction_count",
        tags::INTENDED_NUMBER_OF_FRACTIONS,
        contract.content.intended_number_of_fractions.into(),
    )?;
    rt_check_top_empty_sequence(
        path,
        &obj,
        &mut internal,
        "rt_radiation_set_physician_intent_empty",
        tags::REFERENCED_RT_PHYSICIAN_INTENT_SEQUENCE,
    )?;
    rt_check_top_empty_sequence(
        path,
        &obj,
        &mut internal,
        "rt_radiation_set_author_identification_empty",
        tags::AUTHOR_IDENTIFICATION_SEQUENCE,
    )?;
    for (name, tag, count) in [
        (
            "rt_radiation_set_definition_source_count",
            tags::DEFINITION_SOURCE_SEQUENCE,
            1,
        ),
        (
            "rt_radiation_set_direct_radiation_count",
            tags::RT_RADIATION_SEQUENCE,
            1,
        ),
        (
            "rt_radiation_set_position_group_count",
            tags::TREATMENT_POSITION_GROUP_SEQUENCE,
            1,
        ),
        (
            "rt_radiation_set_common_reference_count",
            tags::REFERENCED_SERIES_SEQUENCE,
            2,
        ),
    ] {
        rt_check_top_sequence_count(path, &obj, &mut internal, name, tag, count)?;
    }
    fail_if_any_failed(path, &internal)?;

    let plan = contract.definition_source;
    let radiation = contract.radiation_references[0];
    let definition = top_level_sequence_item(path, &obj, tags::DEFINITION_SOURCE_SEQUENCE, 0)?;
    validate_rt_sop_reference(
        path,
        definition,
        &mut internal,
        "rt_radiation_set_definition_source",
        plan.sop_class_uid,
        plan.sop_instance_uid,
    )?;
    rt_check_absent_item(
        path,
        definition,
        &mut internal,
        "rt_radiation_set_definition_beam_absent",
        tags::REFERENCED_BEAM_NUMBER,
    )?;
    let direct = top_level_sequence_item(path, &obj, tags::RT_RADIATION_SEQUENCE, 0)?;
    validate_rt_sop_reference(
        path,
        direct,
        &mut internal,
        "rt_radiation_set_direct_radiation",
        radiation.sop_class_uid,
        radiation.sop_instance_uid,
    )?;
    let group = top_level_sequence_item(path, &obj, tags::TREATMENT_POSITION_GROUP_SEQUENCE, 0)?;
    rt_check_item_str(
        path,
        group,
        &mut internal,
        "rt_radiation_set_group_uid",
        tags::TREATMENT_POSITION_GROUP_UID,
        VR::UI,
        contract.treatment_position_groups[0].treatment_position_group_uid,
    )?;
    rt_check_item_str(
        path,
        group,
        &mut internal,
        "rt_radiation_set_group_label",
        tags::TREATMENT_POSITION_GROUP_LABEL,
        VR::LO,
        contract.treatment_position_groups[0].label,
    )?;
    rt_check_item_sequence_count(
        path,
        group,
        &mut internal,
        "rt_radiation_set_group_membership_count",
        tags::REFERENCED_RT_RADIATION_SEQUENCE,
        1,
    )?;
    fail_if_any_failed(path, &internal)?;
    let member = item_sequence_item(path, group, tags::REFERENCED_RT_RADIATION_SEQUENCE, 0)?;
    validate_rt_sop_reference(
        path,
        member,
        &mut internal,
        "rt_radiation_set_group_radiation",
        radiation.sop_class_uid,
        radiation.sop_instance_uid,
    )?;
    validate_rt_common_reference(
        path,
        &obj,
        &mut internal,
        0,
        "rt_radiation_set_common_plan",
        plan.series_instance_uid,
        plan.sop_class_uid,
        plan.sop_instance_uid,
    )?;
    validate_rt_common_reference(
        path,
        &obj,
        &mut internal,
        1,
        "rt_radiation_set_common_radiation",
        radiation.series_instance_uid,
        radiation.sop_class_uid,
        radiation.sop_instance_uid,
    )?;
    check(
        &mut internal,
        direct == member,
        "rt_radiation_set_once_only_membership",
        "The direct and group projections identify the same sole Radiation.",
        "The group projection does not repeat the direct Radiation exactly once.",
    );
    validate_rt_radiation_set_absences(path, &obj, &mut internal, contract.absent_content)?;
    fail_if_any_failed(path, &internal)?;
    Ok(rt_validated(
        bytes,
        internal,
        "rt_radiation_set_sop_class",
        "RT Radiation Set modules and graph references match the locked contract.",
    ))
}

fn validate_rt_radiation_manifest(results: &mut Vec<Value>, expected: ExpectedRtRadiation<'_>) {
    let source = expected.definition_source;
    check(
        results,
        expected.iod_kind == "carm_photon_electron_radiation"
            && expected.sop_class_uid == "1.2.840.10008.5.1.4.1.1.481.13"
            && expected.iod_name == "C-Arm Photon-Electron Radiation"
            && expected.modality == "RTRAD"
            && expected.transfer_syntax_uid == uids::EXPLICIT_VR_LITTLE_ENDIAN
            && source.relationship == "definition_source"
            && source.source_case_id == "non-image/rt/plan_linked"
            && source.source_path == "non-image/rt/plan_linked/instance.dcm"
            && source.sop_class_uid == "1.2.840.10008.5.1.4.1.1.481.5"
            && source.study_instance_uid == expected.study_instance_uid
            && source.frame_of_reference_uid == expected.frame_of_reference_uid
            && source.referenced_beam_number == 1
            && source.common_instance_reference_ordinal == 1,
        "rt_radiation_manifest_identity_contract",
        "The manifest locks the C-Arm Radiation and Plan graph identity.",
        "The manifest weakens the C-Arm Radiation or Plan graph identity.",
    );
    check(
        results,
        expected.treatment_positions.len() == 1
            && expected.treatment_positions[0].ordinal == 1
            && expected.control_points.len() == 2
            && expected.control_points[0].ordinal == 1
            && expected.control_points[0].geometry.is_some()
            && expected.control_points[0]
                .inherits_geometry_from_control_point
                .is_none()
            && expected.control_points[1].ordinal == 2
            && expected.control_points[1].geometry.is_none()
            && expected.control_points[1].inherits_geometry_from_control_point == Some(1),
        "rt_radiation_manifest_cardinality_contract",
        "The manifest locks one position and two inherited control points.",
        "The manifest treatment-position or control-point topology is invalid.",
    );
    let absent = expected.absent_content;
    check(
        results,
        absent.patient_study_module
            && absent.clinical_trial_modules
            && absent.referenced_performed_procedure_step_sequences
            && absent.treatment_session_uid
            && absent.treatment_machine_special_mode
            && absent.rt_tolerance_set
            && absent.treatment_time_limit
            && absent.device_alternate_identifier_type
            && absent.device_alternate_identifier_format
            && absent.unique_device_identifier_sequence
            && absent.device_manufacture_date
            && absent.device_expiration_date
            && absent.device_institution_content
            && absent.long_device_description
            && absent.patient_support_devices_sequence
            && absent.radiation_generation_mode
            && absent.beam_limiting_device_definition_and_opening
            && absent.wedge
            && absent.compensator
            && absent.block
            && absent.accessory_holder
            && absent.general_accessory
            && absent.bolus
            && absent.beam_area_limit
            && absent.recorded_control_point_attributes
            && absent.image
            && absent.pixel_data
            && absent.synchronization,
        "rt_radiation_manifest_absence_contract",
        "The manifest records every locked C-Arm Radiation absence.",
        "The manifest weakens a locked C-Arm Radiation absence.",
    );
}

fn validate_rt_radiation_set_manifest(
    results: &mut Vec<Value>,
    expected: ExpectedRtRadiationSet<'_>,
) {
    let plan = expected.definition_source;
    let radiation = expected.radiation_references[0];
    check(
        results,
        expected.iod_kind == "rt_radiation_set"
            && expected.sop_class_uid == "1.2.840.10008.5.1.4.1.1.481.12"
            && expected.iod_name == "RT Radiation Set"
            && expected.modality == "RTRAD"
            && expected.transfer_syntax_uid == uids::EXPLICIT_VR_LITTLE_ENDIAN
            && plan.relationship == "definition_source"
            && plan.sop_class_uid == "1.2.840.10008.5.1.4.1.1.481.5"
            && radiation.relationship == "referenced_rt_radiation"
            && radiation.sop_class_uid == "1.2.840.10008.5.1.4.1.1.481.13",
        "rt_radiation_set_manifest_identity_contract",
        "The manifest locks the Set, Plan, and Radiation identities.",
        "The manifest weakens the Set, Plan, or Radiation identities.",
    );
    let group = expected.treatment_position_groups[0];
    check(
        results,
        expected.radiation_references.len() == 1
            && expected.treatment_position_groups.len() == 1
            && group.ordinal == 1
            && group.radiation_references == expected.radiation_references
            && expected.common_instance_references.len() == 2
            && expected.common_instance_references[0].sop_instance_uid == plan.sop_instance_uid
            && expected.common_instance_references[1].sop_instance_uid
                == radiation.sop_instance_uid
            && plan.study_instance_uid == expected.study_instance_uid
            && radiation.study_instance_uid == expected.study_instance_uid
            && plan.frame_of_reference_uid == expected.frame_of_reference_uid
            && radiation.frame_of_reference_uid == expected.frame_of_reference_uid,
        "rt_radiation_set_manifest_graph_contract",
        "The manifest repeats one Radiation in each ordered graph projection.",
        "The manifest graph cardinality, order, or shared identities are invalid.",
    );
    let absent = expected.absent_content;
    check(
        results,
        absent.patient_study_module
            && absent.clinical_trial_modules
            && absent.referenced_performed_procedure_step_sequences
            && absent.treatment_session_uid
            && absent.synchronization
            && absent.rt_dose_contribution_module
            && absent.fraction_pattern_sequence
            && absent.image
            && absent.pixel_data,
        "rt_radiation_set_manifest_absence_contract",
        "The manifest records every locked RT Radiation Set absence.",
        "The manifest weakens a locked RT Radiation Set absence.",
    );
}

#[allow(clippy::too_many_arguments)]
fn validate_rt_part10_identity(
    path: &Path,
    obj: &OpenedObject,
    results: &mut Vec<Value>,
    sop_class: &str,
    sop_instance: &str,
    transfer_syntax: &str,
    implementation: &str,
    synthetic: &str,
    study: &str,
    series: &str,
    frame: &str,
    modality: &str,
    series_number: u8,
) -> Result<(), GenerateError> {
    check(
        results,
        fs::read(path)
            .map(|bytes| bytes.len() >= 132 && &bytes[128..132] == b"DICM")
            .unwrap_or(false),
        "rt_part10_preamble",
        "The file has the Part 10 marker.",
        "The file lacks the Part 10 marker.",
    );
    for (name, tag, vr, value) in [
        ("rt_sop_class_uid", tags::SOP_CLASS_UID, VR::UI, sop_class),
        (
            "rt_sop_instance_uid",
            tags::SOP_INSTANCE_UID,
            VR::UI,
            sop_instance,
        ),
        ("rt_synthetic_data", tags::SYNTHETIC_DATA, VR::CS, synthetic),
        (
            "rt_study_instance_uid",
            tags::STUDY_INSTANCE_UID,
            VR::UI,
            study,
        ),
        (
            "rt_series_instance_uid",
            tags::SERIES_INSTANCE_UID,
            VR::UI,
            series,
        ),
        (
            "rt_frame_of_reference_uid",
            tags::FRAME_OF_REFERENCE_UID,
            VR::UI,
            frame,
        ),
        ("rt_modality", tags::MODALITY, VR::CS, modality),
    ] {
        rt_check_top_str(path, obj, results, name, tag, vr, value)?;
    }
    rt_check_top_str(
        path,
        obj,
        results,
        "rt_series_number",
        tags::SERIES_NUMBER,
        VR::IS,
        &series_number.to_string(),
    )?;
    check_equal(
        results,
        "rt_media_storage_sop_class_uid",
        "File Meta SOP Class matches the dataset.",
        "File Meta SOP Class differs from the dataset.",
        trim_uid(obj.meta().media_storage_sop_class_uid()),
        sop_class.to_string(),
    );
    check_equal(
        results,
        "rt_media_storage_sop_instance_uid",
        "File Meta SOP Instance matches the dataset.",
        "File Meta SOP Instance differs from the dataset.",
        trim_uid(obj.meta().media_storage_sop_instance_uid()),
        sop_instance.to_string(),
    );
    check_equal(
        results,
        "rt_transfer_syntax_uid",
        "Transfer Syntax matches the contract.",
        "Transfer Syntax differs from the contract.",
        trim_uid(obj.meta().transfer_syntax()),
        transfer_syntax.to_string(),
    );
    check_equal(
        results,
        "rt_implementation_class_uid",
        "Implementation Class UID matches the generator.",
        "Implementation Class UID differs from the generator.",
        trim_uid(obj.meta().implementation_class_uid()),
        implementation.to_string(),
    );
    Ok(())
}

fn validate_rt_instance_context(
    path: &Path,
    obj: &OpenedObject,
    results: &mut Vec<Value>,
    expected: ExpectedRtRadiationInstance<'_>,
) -> Result<(), GenerateError> {
    rt_check_top_str(
        path,
        obj,
        results,
        "rt_instance_number",
        tags::INSTANCE_NUMBER,
        VR::IS,
        &expected.instance_number.to_string(),
    )?;
    for (name, tag, vr, value) in [
        (
            "rt_patient_name",
            tags::PATIENT_NAME,
            VR::PN,
            expected.patient_name,
        ),
        (
            "rt_patient_id",
            tags::PATIENT_ID,
            VR::LO,
            expected.patient_id,
        ),
        (
            "rt_patient_birth_date",
            tags::PATIENT_BIRTH_DATE,
            VR::DA,
            expected.patient_birth_date,
        ),
        (
            "rt_patient_sex",
            tags::PATIENT_SEX,
            VR::CS,
            expected.patient_sex,
        ),
        ("rt_study_id", tags::STUDY_ID, VR::SH, expected.study_id),
        (
            "rt_referring_physician",
            tags::REFERRING_PHYSICIAN_NAME,
            VR::PN,
            expected.referring_physician_name,
        ),
        (
            "rt_accession_number",
            tags::ACCESSION_NUMBER,
            VR::SH,
            expected.accession_number,
        ),
        (
            "rt_position_reference_indicator",
            tags::POSITION_REFERENCE_INDICATOR,
            VR::LO,
            expected.position_reference_indicator,
        ),
        (
            "rt_manufacturer",
            tags::MANUFACTURER,
            VR::LO,
            expected.equipment_manufacturer,
        ),
        (
            "rt_model_name",
            tags::MANUFACTURER_MODEL_NAME,
            VR::LO,
            expected.equipment_model_name,
        ),
        (
            "rt_device_serial",
            tags::DEVICE_SERIAL_NUMBER,
            VR::LO,
            expected.equipment_serial_number,
        ),
        (
            "rt_software_versions",
            tags::SOFTWARE_VERSIONS,
            VR::LO,
            expected.software_versions,
        ),
        (
            "rt_series_date",
            tags::SERIES_DATE,
            VR::DA,
            expected.series_date,
        ),
        (
            "rt_series_time",
            tags::SERIES_TIME,
            VR::TM,
            expected.series_time,
        ),
        (
            "rt_instance_creation_date",
            tags::INSTANCE_CREATION_DATE,
            VR::DA,
            expected.instance_creation_date,
        ),
        (
            "rt_instance_creation_time",
            tags::INSTANCE_CREATION_TIME,
            VR::TM,
            expected.instance_creation_time,
        ),
        (
            "rt_content_date",
            tags::CONTENT_DATE,
            VR::DA,
            expected.content_date,
        ),
        (
            "rt_content_time",
            tags::CONTENT_TIME,
            VR::TM,
            expected.content_time,
        ),
    ] {
        rt_check_top_str(path, obj, results, name, tag, vr, value)?;
    }
    Ok(())
}

fn validate_rt_treatment_device(
    path: &Path,
    item: &DatasetObject,
    results: &mut Vec<Value>,
    expected: ExpectedRtTreatmentDevice<'_>,
) -> Result<(), GenerateError> {
    for (name, tag, vr, value) in [
        (
            "rt_radiation_device_manufacturer",
            tags::MANUFACTURER,
            VR::LO,
            expected.manufacturer,
        ),
        (
            "rt_radiation_device_model",
            tags::MANUFACTURER_MODEL_NAME,
            VR::LO,
            expected.model_name,
        ),
        (
            "rt_radiation_device_model_version",
            tags::MANUFACTURER_MODEL_VERSION,
            VR::LO,
            expected.model_version,
        ),
        (
            "rt_radiation_device_label",
            tags::DEVICE_LABEL,
            VR::LO,
            expected.device_label,
        ),
        (
            "rt_radiation_device_serial",
            tags::DEVICE_SERIAL_NUMBER,
            VR::LO,
            expected.serial_number,
        ),
        (
            "rt_radiation_device_software",
            tags::SOFTWARE_VERSIONS,
            VR::LO,
            expected.software_versions,
        ),
        (
            "rt_radiation_manufacturer_identifier",
            tags::MANUFACTURER_DEVICE_IDENTIFIER,
            VR::ST,
            expected.manufacturer_device_identifier,
        ),
        (
            "rt_radiation_manufacturer_class_uid",
            tags::MANUFACTURER_DEVICE_CLASS_UID,
            VR::UI,
            expected.manufacturer_device_class_uid,
        ),
        (
            "rt_radiation_device_alternate_identifier",
            tags::DEVICE_ALTERNATE_IDENTIFIER,
            VR::UC,
            expected.device_alternate_identifier,
        ),
    ] {
        rt_check_item_str(path, item, results, name, tag, vr, value)?;
    }
    validate_rt_item_code(
        path,
        item,
        results,
        "rt_radiation_device_type",
        tags::DEVICE_TYPE_CODE_SEQUENCE,
        expected.device_type,
    )
}

fn validate_rt_control_point(
    path: &Path,
    item: &DatasetObject,
    results: &mut Vec<Value>,
    expected: ExpectedRtRadiationControlPoint,
    first: bool,
) -> Result<(), GenerateError> {
    rt_check_item_u16(
        path,
        item,
        results,
        if first {
            "rt_radiation_control_point_1_index"
        } else {
            "rt_radiation_control_point_2_index"
        },
        tags::RT_CONTROL_POINT_INDEX,
        expected.rt_control_point_index.into(),
    )?;
    rt_check_item_f64(
        path,
        item,
        results,
        if first {
            "rt_radiation_control_point_1_meterset"
        } else {
            "rt_radiation_control_point_2_meterset"
        },
        tags::CUMULATIVE_METERSET,
        VR::FD,
        f64::from(expected.cumulative_meterset),
    )?;
    fail_if_any_failed(path, results)?;
    if let Some(geometry) = expected.geometry {
        rt_check_item_u16(
            path,
            item,
            results,
            "rt_radiation_control_point_1_position",
            tags::REFERENCED_TREATMENT_POSITION_INDEX,
            geometry.referenced_treatment_position_index.into(),
        )?;
        rt_check_item_empty(
            path,
            item,
            results,
            "rt_radiation_control_point_1_delivery_rate_empty",
            tags::DELIVERY_RATE,
            VR::FD,
        )?;
        rt_check_item_f64(
            path,
            item,
            results,
            "rt_radiation_control_point_1_source_roll",
            tags::SOURCE_ROLL_ANGLE,
            VR::FD,
            f64::from(geometry.source_roll_angle_degrees),
        )?;
        rt_check_item_f64(
            path,
            item,
            results,
            "rt_radiation_control_point_1_bl_angle",
            tags::RT_BEAM_LIMITING_DEVICE_ANGLE,
            VR::FD,
            f64::from(geometry.rt_beam_limiting_device_angle_degrees),
        )?;
        rt_check_item_empty(
            path,
            item,
            results,
            "rt_radiation_control_point_1_surface_distance_empty",
            tags::SOURCE_TO_PATIENT_SURFACE_DISTANCE,
            VR::FD,
        )?;
        rt_check_item_empty(
            path,
            item,
            results,
            "rt_radiation_control_point_1_contour_distance_empty",
            tags::SOURCE_TO_EXTERNAL_CONTOUR_DISTANCE,
            VR::FL,
        )?;
        rt_check_absent_item(
            path,
            item,
            results,
            "rt_radiation_control_point_1_delivery_unit_absent",
            tags::DELIVERY_RATE_UNIT_SEQUENCE,
        )?;
    } else {
        for (name, tag) in [
            (
                "rt_radiation_control_point_2_position_inherited",
                tags::REFERENCED_TREATMENT_POSITION_INDEX,
            ),
            (
                "rt_radiation_control_point_2_delivery_rate_inherited",
                tags::DELIVERY_RATE,
            ),
            (
                "rt_radiation_control_point_2_source_roll_inherited",
                tags::SOURCE_ROLL_ANGLE,
            ),
            (
                "rt_radiation_control_point_2_bl_angle_inherited",
                tags::RT_BEAM_LIMITING_DEVICE_ANGLE,
            ),
            (
                "rt_radiation_control_point_2_surface_distance_inherited",
                tags::SOURCE_TO_PATIENT_SURFACE_DISTANCE,
            ),
            (
                "rt_radiation_control_point_2_contour_distance_inherited",
                tags::SOURCE_TO_EXTERNAL_CONTOUR_DISTANCE,
            ),
        ] {
            rt_check_absent_item(path, item, results, name, tag)?;
        }
    }
    Ok(())
}

fn validate_rt_common_reference(
    path: &Path,
    obj: &OpenedObject,
    results: &mut Vec<Value>,
    index: usize,
    prefix: &str,
    series_uid: &str,
    sop_class: &str,
    sop_instance: &str,
) -> Result<(), GenerateError> {
    let series = top_level_sequence_item(path, obj, tags::REFERENCED_SERIES_SEQUENCE, index)?;
    rt_check_item_str(
        path,
        series,
        results,
        &format!("{prefix}_series_uid"),
        tags::SERIES_INSTANCE_UID,
        VR::UI,
        series_uid,
    )?;
    rt_check_item_sequence_count(
        path,
        series,
        results,
        &format!("{prefix}_instance_count"),
        tags::REFERENCED_INSTANCE_SEQUENCE,
        1,
    )?;
    fail_if_any_failed(path, results)?;
    let reference = item_sequence_item(path, series, tags::REFERENCED_INSTANCE_SEQUENCE, 0)?;
    validate_rt_sop_reference(path, reference, results, prefix, sop_class, sop_instance)
}

fn validate_rt_top_code(
    path: &Path,
    obj: &OpenedObject,
    results: &mut Vec<Value>,
    prefix: &str,
    tag: Tag,
    expected: ExpectedRtCode<'_>,
) -> Result<(), GenerateError> {
    rt_check_top_sequence_count(path, obj, results, &format!("{prefix}_count"), tag, 1)?;
    fail_if_any_failed(path, results)?;
    validate_rt_code_item(
        path,
        top_level_sequence_item(path, obj, tag, 0)?,
        results,
        prefix,
        expected,
    )
}

fn validate_rt_item_code(
    path: &Path,
    obj: &DatasetObject,
    results: &mut Vec<Value>,
    prefix: &str,
    tag: Tag,
    expected: ExpectedRtCode<'_>,
) -> Result<(), GenerateError> {
    rt_check_item_sequence_count(path, obj, results, &format!("{prefix}_count"), tag, 1)?;
    fail_if_any_failed(path, results)?;
    validate_rt_code_item(
        path,
        item_sequence_item(path, obj, tag, 0)?,
        results,
        prefix,
        expected,
    )
}

fn validate_rt_code_item(
    path: &Path,
    item: &DatasetObject,
    results: &mut Vec<Value>,
    prefix: &str,
    expected: ExpectedRtCode<'_>,
) -> Result<(), GenerateError> {
    rt_check_item_str(
        path,
        item,
        results,
        &format!("{prefix}_value"),
        tags::CODE_VALUE,
        VR::SH,
        expected.code_value,
    )?;
    rt_check_item_str(
        path,
        item,
        results,
        &format!("{prefix}_scheme"),
        tags::CODING_SCHEME_DESIGNATOR,
        VR::SH,
        expected.coding_scheme_designator,
    )?;
    rt_check_item_str(
        path,
        item,
        results,
        &format!("{prefix}_meaning"),
        tags::CODE_MEANING,
        VR::LO,
        expected.code_meaning,
    )
}

fn validate_rt_sop_reference(
    path: &Path,
    item: &DatasetObject,
    results: &mut Vec<Value>,
    prefix: &str,
    sop_class: &str,
    sop_instance: &str,
) -> Result<(), GenerateError> {
    rt_check_item_str(
        path,
        item,
        results,
        &format!("{prefix}_sop_class_uid"),
        tags::REFERENCED_SOP_CLASS_UID,
        VR::UI,
        sop_class,
    )?;
    rt_check_item_str(
        path,
        item,
        results,
        &format!("{prefix}_sop_instance_uid"),
        tags::REFERENCED_SOP_INSTANCE_UID,
        VR::UI,
        sop_instance,
    )
}

fn validate_rt_radiation_absences(
    path: &Path,
    obj: &OpenedObject,
    results: &mut Vec<Value>,
    _: ExpectedRtRadiationAbsentContent,
) -> Result<(), GenerateError> {
    for (name, tag) in [
        (
            "rt_radiation_referenced_pps_absent",
            tags::REFERENCED_PERFORMED_PROCEDURE_STEP_SEQUENCE,
        ),
        (
            "rt_radiation_patient_study_absent",
            tags::ADDITIONAL_PATIENT_HISTORY,
        ),
        (
            "rt_radiation_clinical_trial_absent",
            tags::CLINICAL_TRIAL_SPONSOR_NAME,
        ),
        (
            "rt_radiation_treatment_session_uid_absent",
            tags::TREATMENT_SESSION_UID,
        ),
        (
            "rt_radiation_special_mode_absent",
            tags::TREATMENT_MACHINE_SPECIAL_MODE_CODE_SEQUENCE,
        ),
        (
            "rt_radiation_tolerance_set_absent",
            tags::RT_TOLERANCE_SET_SEQUENCE,
        ),
        ("rt_radiation_time_limit_absent", tags::TREATMENT_TIME_LIMIT),
        (
            "rt_radiation_patient_support_sequence_absent",
            tags::PATIENT_SUPPORT_DEVICES_SEQUENCE,
        ),
        (
            "rt_radiation_generation_mode_absent",
            tags::RADIATION_GENERATION_MODE_SEQUENCE,
        ),
        (
            "rt_radiation_bl_definition_absent",
            tags::RT_BEAM_LIMITING_DEVICE_DEFINITION_SEQUENCE,
        ),
        (
            "rt_radiation_bl_opening_absent",
            tags::RT_BEAM_LIMITING_DEVICE_OPENING_SEQUENCE,
        ),
        ("rt_radiation_wedge_absent", tags::WEDGE_DEFINITION_SEQUENCE),
        (
            "rt_radiation_compensator_absent",
            tags::COMPENSATOR_DEFINITION_SEQUENCE,
        ),
        ("rt_radiation_block_absent", tags::BLOCK_DEFINITION_SEQUENCE),
        (
            "rt_radiation_accessory_holder_absent",
            tags::RT_ACCESSORY_HOLDER_DEFINITION_SEQUENCE,
        ),
        (
            "rt_radiation_general_accessory_absent",
            tags::GENERAL_ACCESSORY_DEFINITION_SEQUENCE,
        ),
        ("rt_radiation_bolus_absent", tags::BOLUS_DEFINITION_SEQUENCE),
        (
            "rt_radiation_beam_area_limit_absent",
            tags::BEAM_AREA_LIMIT_SEQUENCE,
        ),
        ("rt_radiation_patient_age_absent", tags::PATIENT_AGE),
        (
            "rt_radiation_patient_history_absent",
            tags::ADDITIONAL_PATIENT_HISTORY,
        ),
        (
            "rt_radiation_clinical_trial_subject_absent",
            tags::CLINICAL_TRIAL_SPONSOR_NAME,
        ),
        (
            "rt_radiation_clinical_trial_series_absent",
            tags::CLINICAL_TRIAL_SERIES_ID,
        ),
        (
            "rt_radiation_synchronization_for_absent",
            tags::SYNCHRONIZATION_FRAME_OF_REFERENCE_UID,
        ),
        (
            "rt_radiation_synchronization_trigger_absent",
            tags::SYNCHRONIZATION_TRIGGER,
        ),
        (
            "rt_radiation_acquisition_sync_absent",
            tags::ACQUISITION_TIME_SYNCHRONIZED,
        ),
        ("rt_radiation_rows_absent", tags::ROWS),
        ("rt_radiation_columns_absent", tags::COLUMNS),
        (
            "rt_radiation_samples_per_pixel_absent",
            tags::SAMPLES_PER_PIXEL,
        ),
        (
            "rt_radiation_photometric_interpretation_absent",
            tags::PHOTOMETRIC_INTERPRETATION,
        ),
        ("rt_radiation_pixel_data_absent", tags::PIXEL_DATA),
        ("rt_radiation_rows_absent", tags::ROWS),
        ("rt_radiation_columns_absent", tags::COLUMNS),
        (
            "rt_radiation_synchronization_absent",
            tags::SYNCHRONIZATION_FRAME_OF_REFERENCE_UID,
        ),
    ] {
        rt_check_absent_top(path, obj, results, name, tag)?;
    }
    let device =
        top_level_sequence_item(path, obj, tags::TREATMENT_DEVICE_IDENTIFICATION_SEQUENCE, 0)?;
    for (name, tag) in [
        (
            "rt_radiation_device_alt_type_absent",
            tags::DEVICE_ALTERNATE_IDENTIFIER_TYPE,
        ),
        (
            "rt_radiation_device_alt_format_absent",
            tags::DEVICE_ALTERNATE_IDENTIFIER_FORMAT,
        ),
        ("rt_radiation_device_udi_absent", tags::UDI_SEQUENCE),
        (
            "rt_radiation_device_manufacture_date_absent",
            tags::DATE_OF_MANUFACTURE,
        ),
        (
            "rt_radiation_device_expiration_date_absent",
            tags::EXPIRATION_DATE_TIME,
        ),
        (
            "rt_radiation_device_institution_absent",
            tags::INSTITUTION_NAME,
        ),
        (
            "rt_radiation_device_long_description_absent",
            tags::LONG_DEVICE_DESCRIPTION,
        ),
    ] {
        rt_check_absent_item(path, device, results, name, tag)?;
    }
    for index in 0..2 {
        let point = top_level_sequence_item(
            path,
            obj,
            tags::C_ARM_PHOTON_ELECTRON_CONTROL_POINT_SEQUENCE,
            index,
        )?;
        rt_check_absent_item(
            path,
            point,
            results,
            &format!("rt_radiation_control_point_{}_recorded_absent", index + 1),
            tags::RECORDED_RT_CONTROL_POINT_DATE_TIME,
        )?;
    }
    Ok(())
}

fn validate_rt_radiation_set_absences(
    path: &Path,
    obj: &OpenedObject,
    results: &mut Vec<Value>,
    _: ExpectedRtRadiationSetAbsentContent,
) -> Result<(), GenerateError> {
    for (name, tag) in [
        (
            "rt_radiation_set_referenced_pps_absent",
            tags::REFERENCED_PERFORMED_PROCEDURE_STEP_SEQUENCE,
        ),
        (
            "rt_radiation_set_patient_study_absent",
            tags::ADDITIONAL_PATIENT_HISTORY,
        ),
        (
            "rt_radiation_set_clinical_trial_absent",
            tags::CLINICAL_TRIAL_SPONSOR_NAME,
        ),
        (
            "rt_radiation_set_treatment_session_uid_absent",
            tags::TREATMENT_SESSION_UID,
        ),
        (
            "rt_radiation_set_fraction_pattern_absent",
            tags::FRACTION_PATTERN,
        ),
        (
            "rt_radiation_set_dose_contribution_absent",
            tags::RADIATION_DOSE_SEQUENCE,
        ),
        ("rt_radiation_set_patient_age_absent", tags::PATIENT_AGE),
        (
            "rt_radiation_set_patient_history_absent",
            tags::ADDITIONAL_PATIENT_HISTORY,
        ),
        (
            "rt_radiation_set_clinical_trial_subject_absent",
            tags::CLINICAL_TRIAL_SPONSOR_NAME,
        ),
        (
            "rt_radiation_set_clinical_trial_series_absent",
            tags::CLINICAL_TRIAL_SERIES_ID,
        ),
        (
            "rt_radiation_set_synchronization_for_absent",
            tags::SYNCHRONIZATION_FRAME_OF_REFERENCE_UID,
        ),
        (
            "rt_radiation_set_synchronization_trigger_absent",
            tags::SYNCHRONIZATION_TRIGGER,
        ),
        (
            "rt_radiation_set_acquisition_sync_absent",
            tags::ACQUISITION_TIME_SYNCHRONIZED,
        ),
        ("rt_radiation_set_rows_absent", tags::ROWS),
        ("rt_radiation_set_columns_absent", tags::COLUMNS),
        (
            "rt_radiation_set_samples_per_pixel_absent",
            tags::SAMPLES_PER_PIXEL,
        ),
        (
            "rt_radiation_set_photometric_interpretation_absent",
            tags::PHOTOMETRIC_INTERPRETATION,
        ),
        ("rt_radiation_set_pixel_data_absent", tags::PIXEL_DATA),
        ("rt_radiation_set_rows_absent", tags::ROWS),
        ("rt_radiation_set_columns_absent", tags::COLUMNS),
        (
            "rt_radiation_set_synchronization_absent",
            tags::SYNCHRONIZATION_FRAME_OF_REFERENCE_UID,
        ),
    ] {
        rt_check_absent_top(path, obj, results, name, tag)?;
    }
    Ok(())
}

fn rt_check_top_str(
    path: &Path,
    obj: &OpenedObject,
    results: &mut Vec<Value>,
    name: &str,
    tag: Tag,
    vr: VR,
    expected: &str,
) -> Result<(), GenerateError> {
    let element = obj
        .element(tag)
        .map_err(|err| rt_named_error(path, name, err))?;
    let actual = element
        .value()
        .to_str()
        .map_err(|err| rt_named_error(path, name, err))?;
    check(
        results,
        element.vr() == vr
            && element.value().cardinality() == usize::from(!expected.is_empty())
            && actual.trim_matches('\0').trim() == expected,
        name,
        "Attribute VR, VM, and value match the locked contract.",
        "Attribute VR, VM, or value differs from the locked contract.",
    );
    Ok(())
}
fn rt_check_item_str(
    path: &Path,
    obj: &DatasetObject,
    results: &mut Vec<Value>,
    name: &str,
    tag: Tag,
    vr: VR,
    expected: &str,
) -> Result<(), GenerateError> {
    let element = obj
        .element(tag)
        .map_err(|err| rt_named_error(path, name, err))?;
    let actual = element
        .value()
        .to_str()
        .map_err(|err| rt_named_error(path, name, err))?;
    check(
        results,
        element.vr() == vr
            && element.value().cardinality() == usize::from(!expected.is_empty())
            && actual.trim_matches('\0').trim() == expected,
        name,
        "Attribute VR, VM, and value match the locked contract.",
        "Attribute VR, VM, or value differs from the locked contract.",
    );
    Ok(())
}
fn rt_check_top_u16(
    path: &Path,
    obj: &OpenedObject,
    results: &mut Vec<Value>,
    name: &str,
    tag: Tag,
    expected: u16,
) -> Result<(), GenerateError> {
    let element = obj
        .element(tag)
        .map_err(|err| rt_named_error(path, name, err))?;
    let actual = element
        .value()
        .to_int::<u16>()
        .map_err(|err| rt_named_error(path, name, err))?;
    check(
        results,
        element.vr() == VR::US && element.value().cardinality() == 1 && actual == expected,
        name,
        "US attribute has VM 1 and the locked value.",
        "US attribute VR, VM, or value differs from the contract.",
    );
    Ok(())
}
fn rt_check_item_u16(
    path: &Path,
    obj: &DatasetObject,
    results: &mut Vec<Value>,
    name: &str,
    tag: Tag,
    expected: u16,
) -> Result<(), GenerateError> {
    let element = obj
        .element(tag)
        .map_err(|err| rt_named_error(path, name, err))?;
    let actual = element
        .value()
        .to_int::<u16>()
        .map_err(|err| rt_named_error(path, name, err))?;
    check(
        results,
        element.vr() == VR::US && element.value().cardinality() == 1 && actual == expected,
        name,
        "US attribute has VM 1 and the locked value.",
        "US attribute VR, VM, or value differs from the contract.",
    );
    Ok(())
}
fn rt_check_top_f64(
    path: &Path,
    obj: &OpenedObject,
    results: &mut Vec<Value>,
    name: &str,
    tag: Tag,
    vr: VR,
    expected: f64,
) -> Result<(), GenerateError> {
    let element = obj
        .element(tag)
        .map_err(|err| rt_named_error(path, name, err))?;
    let actual = element
        .value()
        .to_float64()
        .map_err(|err| rt_named_error(path, name, err))?;
    check(
        results,
        element.vr() == vr && element.value().cardinality() == 1 && actual == expected,
        name,
        "Numeric attribute VR, VM, and value match the contract.",
        "Numeric attribute VR, VM, or value differs from the contract.",
    );
    Ok(())
}
fn rt_check_item_f64(
    path: &Path,
    obj: &DatasetObject,
    results: &mut Vec<Value>,
    name: &str,
    tag: Tag,
    vr: VR,
    expected: f64,
) -> Result<(), GenerateError> {
    let element = obj
        .element(tag)
        .map_err(|err| rt_named_error(path, name, err))?;
    let actual = element
        .value()
        .to_float64()
        .map_err(|err| rt_named_error(path, name, err))?;
    check(
        results,
        element.vr() == vr && element.value().cardinality() == 1 && actual == expected,
        name,
        "Numeric attribute VR, VM, and value match the contract.",
        "Numeric attribute VR, VM, or value differs from the contract.",
    );
    Ok(())
}
fn rt_check_item_f64_values(
    path: &Path,
    obj: &DatasetObject,
    results: &mut Vec<Value>,
    name: &str,
    tag: Tag,
    vr: VR,
    expected: Vec<f64>,
) -> Result<(), GenerateError> {
    let element = obj
        .element(tag)
        .map_err(|err| rt_named_error(path, name, err))?;
    let actual = element
        .value()
        .to_multi_float64()
        .map_err(|err| rt_named_error(path, name, err))?;
    check(
        results,
        element.vr() == vr && element.value().cardinality() == expected.len() && actual == expected,
        name,
        "Numeric attribute VR, VM, order, and values match the contract.",
        "Numeric attribute VR, VM, order, or values differ from the contract.",
    );
    Ok(())
}
fn rt_check_top_sequence_count(
    path: &Path,
    obj: &OpenedObject,
    results: &mut Vec<Value>,
    name: &str,
    tag: Tag,
    expected: usize,
) -> Result<(), GenerateError> {
    let element = obj
        .element(tag)
        .map_err(|err| rt_named_error(path, name, err))?;
    let count = element
        .items()
        .map(|items| items.len())
        .unwrap_or(usize::MAX);
    check(
        results,
        element.vr() == VR::SQ && count == expected,
        name,
        "Sequence VR and cardinality match the contract.",
        "Sequence VR or cardinality differs from the contract.",
    );
    Ok(())
}
fn rt_check_item_sequence_count(
    path: &Path,
    obj: &DatasetObject,
    results: &mut Vec<Value>,
    name: &str,
    tag: Tag,
    expected: usize,
) -> Result<(), GenerateError> {
    let element = obj
        .element(tag)
        .map_err(|err| rt_named_error(path, name, err))?;
    let count = element
        .items()
        .map(|items| items.len())
        .unwrap_or(usize::MAX);
    check(
        results,
        element.vr() == VR::SQ && count == expected,
        name,
        "Sequence VR and cardinality match the contract.",
        "Sequence VR or cardinality differs from the contract.",
    );
    Ok(())
}
fn rt_check_top_empty_sequence(
    path: &Path,
    obj: &OpenedObject,
    results: &mut Vec<Value>,
    name: &str,
    tag: Tag,
) -> Result<(), GenerateError> {
    rt_check_top_sequence_count(path, obj, results, name, tag, 0)
}
fn rt_check_item_empty_sequence(
    path: &Path,
    obj: &DatasetObject,
    results: &mut Vec<Value>,
    name: &str,
    tag: Tag,
) -> Result<(), GenerateError> {
    rt_check_item_sequence_count(path, obj, results, name, tag, 0)
}
fn rt_check_item_empty(
    path: &Path,
    obj: &DatasetObject,
    results: &mut Vec<Value>,
    name: &str,
    tag: Tag,
    vr: VR,
) -> Result<(), GenerateError> {
    let element = obj
        .element(tag)
        .map_err(|err| rt_named_error(path, name, err))?;
    check(
        results,
        element.vr() == vr && element.value().cardinality() == 0,
        name,
        "Type 2 attribute is present empty with the locked VR.",
        "Type 2 attribute is missing, non-empty, or uses the wrong VR.",
    );
    Ok(())
}
fn rt_check_absent_top(
    path: &Path,
    obj: &OpenedObject,
    results: &mut Vec<Value>,
    name: &str,
    tag: Tag,
) -> Result<(), GenerateError> {
    check(
        results,
        obj.element_opt(tag)
            .map_err(|err| rt_named_error(path, name, err))?
            .is_none(),
        name,
        "Locked conditional attribute is absent.",
        "A locked-absent conditional attribute is present.",
    );
    Ok(())
}
fn rt_check_absent_item(
    path: &Path,
    obj: &DatasetObject,
    results: &mut Vec<Value>,
    name: &str,
    tag: Tag,
) -> Result<(), GenerateError> {
    check(
        results,
        obj.element_opt(tag)
            .map_err(|err| rt_named_error(path, name, err))?
            .is_none(),
        name,
        "Locked conditional attribute is absent.",
        "A locked-absent conditional attribute is present.",
    );
    Ok(())
}
fn rt_named_error(path: &Path, name: &str, err: impl std::fmt::Display) -> GenerateError {
    GenerateError::ValidateDicomFile {
        path: path.to_path_buf(),
        message: format!("{name}: {err}"),
    }
}
fn rt_validated(
    bytes: Vec<u8>,
    internal: Vec<Value>,
    standard_name: &str,
    message: &str,
) -> ValidatedPart10 {
    ValidatedPart10 {
        bytes,
        validation: serde_json::json!({"status":"passed","internal":internal,"standards":[{"name":standard_name,"status":"passed","message":message},{"name":"explicit_vr_little_endian_transfer_syntax","status":"passed","message":"Transfer Syntax UID matches Explicit VR Little Endian in the 2026b reference."},{"name":"synthetic_data_attribute","status":"passed","message":"Synthetic Data (0008,001C) is present with value YES."}],"external":[]}),
    }
}

pub(crate) fn validate_rt_image_file(
    path: &Path,
    expected: &RtImageExpectations<'_>,
) -> Result<ValidatedPart10, GenerateError> {
    let bytes = fs::read(path).map_err(|source| GenerateError::ReadGeneratedFile {
        path: path.to_path_buf(),
        source,
    })?;
    let obj = open_file(path).map_err(|err| GenerateError::ValidateDicomFile {
        path: path.to_path_buf(),
        message: err.to_string(),
    })?;
    let image_expected = expected.expected_rt_image;
    let storage = image_expected.storage;
    let image = image_expected.image;
    let reference = image_expected.plan_reference;
    let linkage = image_expected.linkage;
    let absent = image_expected.absent_content;
    let mut internal = Vec::new();

    let shape_length = usize::from(storage.rows)
        .checked_mul(usize::from(storage.columns))
        .and_then(|value| value.checked_mul(usize::from(storage.frames)))
        .and_then(|value| value.checked_mul(usize::from(storage.samples_per_pixel)));
    check(
        &mut internal,
        shape_length == Some(16)
            && storage.pixel_values.len() == 16
            && usize::from(storage.payload_length_bytes) == 16,
        "rt_image_manifest_pixel_cardinality",
        "Manifest pixel shape and payload contain exactly sixteen samples.",
        "Manifest pixel shape or payload cardinality is invalid.",
    );
    check(
        &mut internal,
        image_expected.iod_kind == "rt_image"
            && image_expected.sop_class_uid == "1.2.840.10008.5.1.4.1.1.481.1"
            && image_expected.iod_name == "RT Image"
            && image_expected.modality == "RTIMAGE"
            && image_expected.transfer_syntax_uid == uids::EXPLICIT_VR_LITTLE_ENDIAN
            && reference.relationship == "referenced_rt_plan"
            && reference.source_case_id == "non-image/rt/plan_linked"
            && reference.source_path == "non-image/rt/plan_linked/instance.dcm"
            && reference.sop_class_uid == "1.2.840.10008.5.1.4.1.1.481.5",
        "rt_image_manifest_identity_contract",
        "Manifest identity and Plan reference match the validator-owned recipe.",
        "Manifest identity or Plan reference does not match the validator-owned recipe.",
    );
    check(
        &mut internal,
        reference.study_instance_uid == image_expected.study_instance_uid
            && reference.frame_of_reference_uid == image_expected.frame_of_reference_uid,
        "rt_image_manifest_shared_identity",
        "Manifest RT Image and Plan share Study and Frame of Reference identities.",
        "Manifest RT Image and Plan do not share Study and Frame of Reference identities.",
    );
    check(
        &mut internal,
        reference.source_sha256.len() == 64
            && reference
                .source_sha256
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
        "rt_image_manifest_source_hash",
        "Manifest Plan source identity is a lowercase SHA-256 digest.",
        "Manifest Plan source identity is malformed.",
    );
    check(
        &mut internal,
        image.image_type == ["DERIVED", "SECONDARY", "DRR"]
            && image.conversion_type == "WSD"
            && image.label == "DTS_DRR"
            && image.plane == "NORMAL"
            && image.xray_image_receptor_angle_degrees == 0
            && image.image_plane_pixel_spacing_mm == [1, 1]
            && image.position_mm == [-1.5, 1.5]
            && image.radiation_machine_name == "DTS_LINAC"
            && image.radiation_machine_sad_mm == 1000
            && image.rt_image_sid_mm == 1500
            && image.primary_dosimeter_unit == "MU"
            && linkage.referenced_beam_number == 1
            && linkage.referenced_fraction_group_number == 1,
        "rt_image_manifest_geometry_contract",
        "Manifest RT Image geometry and linkage match the locked recipe.",
        "Manifest RT Image geometry or linkage weakens the locked recipe.",
    );
    let formula_pixels = (0_u8..16).map(|index| 17 * index).collect::<Vec<_>>();
    check(
        &mut internal,
        storage.rows == 4
            && storage.columns == 4
            && storage.frames == 1
            && storage.samples_per_pixel == 1
            && storage.photometric_interpretation == "MONOCHROME2"
            && storage.bits_allocated == 8
            && storage.bits_stored == 8
            && storage.high_bit == 7
            && storage.pixel_representation == 0
            && storage.data_vr == "OB"
            && storage.encoding == "native"
            && storage.value_field_padding_bytes == 0
            && storage.pixel_value_formula == "17 * (4 * r + c)"
            && storage.pixel_values == formula_pixels
            && storage.pixel_min == 0
            && storage.pixel_max == 255
            && storage.payload_sha256
                == "a8faed6abbf35c12a4b26e40f6feb19d736d90045c83b9f9a31f638d323e6811"
            && storage.decoded_pixels_sha256 == storage.payload_sha256,
        "rt_image_manifest_storage_contract",
        "Manifest storage and deterministic pixel formula match the locked recipe.",
        "Manifest storage or deterministic pixel formula weakens the locked recipe.",
    );
    check(
        &mut internal,
        absent.patient_study_module
            && absent.contrast_bolus_module
            && absent.cine_module
            && absent.multi_frame_module
            && absent.modality_lut_module
            && absent.voi_lut_module
            && absent.approval_module
            && absent.clinical_trial_module
            && absent.frame_extraction_module
            && absent.common_instance_reference_module
            && absent.reported_values_origin
            && absent.rt_image_orientation
            && absent.isocenter_position
            && absent.patient_position
            && absent.fluence_map_sequence
            && absent.exposure_sequence
            && absent.overlays
            && absent.encapsulated_pixel_data
            && absent.lossy_pixel_attributes,
        "rt_image_manifest_absence_contract",
        "Manifest records every locked RT Image absence.",
        "Manifest weakens a locked RT Image absence.",
    );
    fail_if_any_failed(path, &internal)?;

    check(
        &mut internal,
        bytes.len() >= 132 && &bytes[128..132] == b"DICM",
        "rt_image_part10_preamble",
        "File has a Part 10 preamble and DICM marker.",
        "File is missing the Part 10 DICM marker.",
    );
    let dataset_sop_class = element_str(path, &obj, tags::SOP_CLASS_UID)?;
    let dataset_sop_instance = element_str(path, &obj, tags::SOP_INSTANCE_UID)?;
    for (name, actual, locked) in [
        (
            "sop_class_uid",
            dataset_sop_class.as_str(),
            image_expected.sop_class_uid,
        ),
        (
            "sop_instance_uid",
            dataset_sop_instance.as_str(),
            image_expected.sop_instance_uid,
        ),
        (
            "synthetic_data",
            element_str(path, &obj, tags::SYNTHETIC_DATA)?.as_str(),
            expected.synthetic_data,
        ),
        (
            "study_instance_uid",
            element_str(path, &obj, tags::STUDY_INSTANCE_UID)?.as_str(),
            image_expected.study_instance_uid,
        ),
        (
            "series_instance_uid",
            element_str(path, &obj, tags::SERIES_INSTANCE_UID)?.as_str(),
            image_expected.series_instance_uid,
        ),
        (
            "frame_of_reference_uid",
            element_str(path, &obj, tags::FRAME_OF_REFERENCE_UID)?.as_str(),
            image_expected.frame_of_reference_uid,
        ),
    ] {
        check_equal(
            &mut internal,
            &format!("rt_image_{name}"),
            "RT Image identity matches the locked recipe.",
            "RT Image identity does not match the locked recipe.",
            actual,
            locked,
        );
    }
    for (name, actual, locked) in [
        (
            "media_storage_sop_class_uid",
            trim_uid(obj.meta().media_storage_sop_class_uid()),
            dataset_sop_class.clone(),
        ),
        (
            "media_storage_sop_instance_uid",
            trim_uid(obj.meta().media_storage_sop_instance_uid()),
            dataset_sop_instance.clone(),
        ),
        (
            "transfer_syntax",
            trim_uid(obj.meta().transfer_syntax()),
            image_expected.transfer_syntax_uid.to_string(),
        ),
        (
            "implementation_class_uid",
            trim_uid(obj.meta().implementation_class_uid()),
            expected.implementation_class_uid.to_string(),
        ),
    ] {
        check_equal(
            &mut internal,
            &format!("rt_image_{name}"),
            "File Meta identity matches the locked RT Image recipe.",
            "File Meta identity does not match the locked RT Image recipe.",
            actual,
            locked,
        );
    }
    for (name, tag, locked) in [
        (
            "patient_name",
            tags::PATIENT_NAME,
            "DTS^Synthetic^Patient001",
        ),
        ("patient_id", tags::PATIENT_ID, "DTS-PATIENT-001"),
        ("patient_birth_date", tags::PATIENT_BIRTH_DATE, "19700101"),
        ("patient_sex", tags::PATIENT_SEX, "O"),
        ("study_date", tags::STUDY_DATE, "20260101"),
        ("study_time", tags::STUDY_TIME, "000000"),
        ("referring_physician", tags::REFERRING_PHYSICIAN_NAME, ""),
        ("study_id", tags::STUDY_ID, "DTS-RTSTRUCT"),
        ("accession_number", tags::ACCESSION_NUMBER, ""),
        ("modality", tags::MODALITY, "RTIMAGE"),
        ("series_number", tags::SERIES_NUMBER, "73"),
        ("operators_name", tags::OPERATORS_NAME, ""),
        (
            "position_reference_indicator",
            tags::POSITION_REFERENCE_INDICATOR,
            "",
        ),
        ("manufacturer", tags::MANUFACTURER, "dicom-test-suite"),
        ("institution_name", tags::INSTITUTION_NAME, ""),
        ("institution_address", tags::INSTITUTION_ADDRESS, ""),
        (
            "manufacturer_model_name",
            tags::MANUFACTURER_MODEL_NAME,
            "Native Linked RT Image",
        ),
        (
            "device_serial_number",
            tags::DEVICE_SERIAL_NUMBER,
            "DTS-RTIMAGE-001",
        ),
        (
            "software_versions",
            tags::SOFTWARE_VERSIONS,
            crate::PACKAGE_VERSION,
        ),
        ("acquisition_date", tags::ACQUISITION_DATE, "20260101"),
        ("acquisition_time", tags::ACQUISITION_TIME, "000000"),
        ("image_type", tags::IMAGE_TYPE, "DERIVED\\SECONDARY\\DRR"),
        ("conversion_type", tags::CONVERSION_TYPE, "WSD"),
        ("instance_number", tags::INSTANCE_NUMBER, "1"),
        ("content_date", tags::CONTENT_DATE, "20260101"),
        ("content_time", tags::CONTENT_TIME, "000000"),
        ("patient_orientation", tags::PATIENT_ORIENTATION, ""),
        ("label", tags::RT_IMAGE_LABEL, image.label),
        ("plane", tags::RT_IMAGE_PLANE, image.plane),
        (
            "machine",
            tags::RADIATION_MACHINE_NAME,
            image.radiation_machine_name,
        ),
        (
            "dosimeter_unit",
            tags::PRIMARY_DOSIMETER_UNIT,
            image.primary_dosimeter_unit,
        ),
    ] {
        check_equal(
            &mut internal,
            &format!("rt_image_{name}"),
            "RT Image attribute matches the locked recipe.",
            "RT Image attribute does not match the locked recipe.",
            element_str(path, &obj, tag)?.as_str(),
            locked,
        );
    }
    for (name, tag, locked) in [
        (
            "samples_per_pixel",
            tags::SAMPLES_PER_PIXEL,
            storage.samples_per_pixel,
        ),
        ("rows", tags::ROWS, storage.rows),
        ("columns", tags::COLUMNS, storage.columns),
        (
            "bits_allocated",
            tags::BITS_ALLOCATED,
            storage.bits_allocated,
        ),
        ("bits_stored", tags::BITS_STORED, storage.bits_stored),
        ("high_bit", tags::HIGH_BIT, storage.high_bit),
        (
            "pixel_representation",
            tags::PIXEL_REPRESENTATION,
            storage.pixel_representation,
        ),
    ] {
        check_equal(
            &mut internal,
            &format!("rt_image_{name}"),
            "RT Image integer storage attribute matches the locked recipe.",
            "RT Image integer storage attribute does not match the locked recipe.",
            element_u16(path, &obj, tag)?,
            u16::from(locked),
        );
    }
    check_equal(
        &mut internal,
        "rt_image_photometric_interpretation",
        "Photometric Interpretation matches the locked recipe.",
        "Photometric Interpretation does not match the locked recipe.",
        element_str(path, &obj, tags::PHOTOMETRIC_INTERPRETATION)?.as_str(),
        storage.photometric_interpretation,
    );
    for (name, tag, locked) in [
        (
            "receptor_angle",
            tags::X_RAY_IMAGE_RECEPTOR_ANGLE,
            vec![f64::from(image.xray_image_receptor_angle_degrees)],
        ),
        (
            "pixel_spacing",
            tags::IMAGE_PLANE_PIXEL_SPACING,
            image.image_plane_pixel_spacing_mm.map(f64::from).to_vec(),
        ),
        (
            "position",
            tags::RT_IMAGE_POSITION,
            image.position_mm.map(f64::from).to_vec(),
        ),
        (
            "sad",
            tags::RADIATION_MACHINE_SAD,
            vec![f64::from(image.radiation_machine_sad_mm)],
        ),
        (
            "sid",
            tags::RT_IMAGE_SID,
            vec![f64::from(image.rt_image_sid_mm)],
        ),
    ] {
        check_equal(
            &mut internal,
            &format!("rt_image_{name}"),
            "RT Image geometry matches the locked recipe.",
            "RT Image geometry does not match the locked recipe.",
            element_f64_values(path, &obj, tag)?,
            locked,
        );
    }

    let actual_plane = element_str(path, &obj, tags::RT_IMAGE_PLANE)?;
    check(
        &mut internal,
        actual_plane == "NORMAL"
            || obj
                .element_opt(tags::RT_IMAGE_ORIENTATION)
                .map_err(|err| validation_error(path, err))?
                .is_some(),
        "rt_image_non_normal_orientation",
        "Non-NORMAL images provide orientation, or the image is NORMAL.",
        "A non-NORMAL RT Image omits RT Image Orientation.",
    );
    let actual_type = element_str(path, &obj, tags::IMAGE_TYPE)?;
    check(
        &mut internal,
        !actual_type.split('\\').any(|value| value == "PORTAL")
            || obj
                .element_opt(tags::REPORTED_VALUES_ORIGIN)
                .map_err(|err| validation_error(path, err))?
                .is_some(),
        "rt_image_portal_reported_values_origin",
        "PORTAL images provide Reported Values Origin, or the image is not PORTAL.",
        "A PORTAL RT Image omits Reported Values Origin.",
    );

    check_equal(
        &mut internal,
        "rt_image_plan_reference_count",
        "RT Image contains exactly one Plan reference.",
        "RT Image Plan reference cardinality is invalid.",
        sequence_item_count(path, &obj, tags::REFERENCED_RT_PLAN_SEQUENCE)?,
        1,
    );
    fail_if_any_failed(path, &internal)?;
    let plan_item = top_level_sequence_item(path, &obj, tags::REFERENCED_RT_PLAN_SEQUENCE, 0)?;
    for (name, tag, locked) in [
        (
            "plan_sop_class_uid",
            tags::REFERENCED_SOP_CLASS_UID,
            reference.sop_class_uid,
        ),
        (
            "plan_sop_instance_uid",
            tags::REFERENCED_SOP_INSTANCE_UID,
            reference.sop_instance_uid,
        ),
    ] {
        check_equal(
            &mut internal,
            &format!("rt_image_{name}"),
            "RT Image Plan reference matches the exact upstream identity.",
            "RT Image Plan reference does not match the exact upstream identity.",
            item_str(path, plan_item, tag)?.as_str(),
            locked,
        );
    }
    for (name, tag, locked) in [
        (
            "referenced_beam_number",
            tags::REFERENCED_BEAM_NUMBER,
            linkage.referenced_beam_number,
        ),
        (
            "referenced_fraction_group_number",
            tags::REFERENCED_FRACTION_GROUP_NUMBER,
            linkage.referenced_fraction_group_number,
        ),
    ] {
        check_equal(
            &mut internal,
            &format!("rt_image_{name}"),
            "RT Image reference selects the locked Plan member.",
            "RT Image reference selects the wrong Plan member.",
            element_u16(path, &obj, tag)?,
            u16::from(locked),
        );
    }
    check_equal(
        &mut internal,
        "rt_image_fraction_number",
        "RT Image Fraction Number matches the locked fraction.",
        "RT Image Fraction Number does not match the locked fraction.",
        element_u16(path, &obj, tags::FRACTION_NUMBER)?,
        u16::from(linkage.referenced_fraction_group_number),
    );

    let pixel_element = obj
        .element(tags::PIXEL_DATA)
        .map_err(|err| validation_error(path, err))?;
    let pixel_bytes = pixel_element
        .value()
        .to_bytes()
        .map_err(|err| validation_error(path, err))?;
    check_equal(
        &mut internal,
        "rt_image_pixel_vr",
        "Pixel Data uses native OB storage.",
        "Pixel Data does not use native OB storage.",
        pixel_element.vr(),
        VR::OB,
    );
    check_equal(
        &mut internal,
        "rt_image_pixel_length",
        "Pixel Data contains exactly sixteen bytes.",
        "Pixel Data byte length does not match the image shape.",
        pixel_bytes.len(),
        usize::from(storage.payload_length_bytes),
    );
    check_equal(
        &mut internal,
        "rt_image_pixel_values",
        "Pixel Data matches the deterministic gradient.",
        "Pixel Data does not match the deterministic gradient.",
        pixel_bytes.as_ref(),
        storage.pixel_values,
    );
    check_equal(
        &mut internal,
        "rt_image_pixel_formula",
        "Pixel Data satisfies 17 * (4 * r + c).",
        "Pixel Data violates the locked pixel formula.",
        pixel_bytes.as_ref(),
        formula_pixels.as_slice(),
    );
    check_equal(
        &mut internal,
        "rt_image_pixel_sha256",
        "Pixel payload SHA-256 matches the manifest.",
        "Pixel payload SHA-256 does not match the manifest.",
        sha256_hex(pixel_bytes.as_ref()),
        storage.payload_sha256.to_string(),
    );
    check_equal(
        &mut internal,
        "rt_image_pixel_minimum",
        "Pixel minimum matches the manifest.",
        "Pixel minimum does not match the manifest.",
        pixel_bytes.iter().copied().min(),
        Some(storage.pixel_min),
    );
    check_equal(
        &mut internal,
        "rt_image_pixel_maximum",
        "Pixel maximum matches the manifest.",
        "Pixel maximum does not match the manifest.",
        pixel_bytes.iter().copied().max(),
        Some(storage.pixel_max),
    );

    for (name, tags_to_check) in [
        ("patient_study_module", &[tags::PATIENT_AGE][..]),
        ("contrast_bolus_module", &[tags::CONTRAST_BOLUS_AGENT][..]),
        ("cine_module", &[tags::CINE_RATE][..]),
        (
            "multi_frame_module",
            &[tags::NUMBER_OF_FRAMES, tags::FRAME_INCREMENT_POINTER][..],
        ),
        ("modality_lut_module", &[tags::MODALITY_LUT_SEQUENCE][..]),
        (
            "voi_lut_module",
            &[
                tags::VOILUT_SEQUENCE,
                tags::WINDOW_CENTER,
                tags::WINDOW_WIDTH,
            ][..],
        ),
        (
            "approval_module",
            &[
                tags::APPROVAL_STATUS,
                tags::REVIEW_DATE,
                tags::REVIEW_TIME,
                tags::REVIEWER_NAME,
            ][..],
        ),
        (
            "clinical_trial_module",
            &[tags::CLINICAL_TRIAL_SPONSOR_NAME][..],
        ),
        (
            "frame_extraction_module",
            &[tags::FRAME_EXTRACTION_SEQUENCE][..],
        ),
        (
            "common_instance_reference_module",
            &[
                tags::STUDIES_CONTAINING_OTHER_REFERENCED_INSTANCES_SEQUENCE,
                tags::REFERENCED_SERIES_SEQUENCE,
            ][..],
        ),
        (
            "reported_values_origin",
            &[tags::REPORTED_VALUES_ORIGIN][..],
        ),
        ("rt_image_orientation", &[tags::RT_IMAGE_ORIENTATION][..]),
        ("isocenter_position", &[tags::ISOCENTER_POSITION][..]),
        ("patient_position", &[tags::PATIENT_POSITION][..]),
        ("fluence_map_sequence", &[tags::FLUENCE_MAP_SEQUENCE][..]),
        ("exposure_sequence", &[tags::EXPOSURE_SEQUENCE][..]),
        (
            "lossy_pixel_attributes",
            &[
                tags::LOSSY_IMAGE_COMPRESSION,
                tags::LOSSY_IMAGE_COMPRESSION_RATIO,
                tags::LOSSY_IMAGE_COMPRESSION_METHOD,
            ][..],
        ),
    ] {
        check(
            &mut internal,
            tags_to_check.iter().all(|tag| {
                obj.element_opt(*tag)
                    .map(|value| value.is_none())
                    .unwrap_or(false)
            }),
            &format!("rt_image_{name}_absent"),
            "Locked optional content is absent.",
            "Locked optional content is unexpectedly present.",
        );
    }
    check(
        &mut internal,
        !obj.iter()
            .any(|element| element.tag().group() & 0xff00 == 0x6000),
        "rt_image_overlays_absent",
        "No overlay groups are present.",
        "An overlay group is unexpectedly present.",
    );

    fail_if_any_failed(path, &internal)?;
    Ok(ValidatedPart10 {
        bytes,
        validation: serde_json::json!({
            "status": "passed",
            "internal": internal,
            "standards": [{
                "name": "rt_image_sop_class",
                "status": "passed",
                "message": "RT Image modules, linked Plan identity, geometry, native pixels, and locked absences match the recipe."
            }],
            "external": []
        }),
    })
}

pub(crate) fn validate_rt_plan_file(
    path: &Path,
    expected: &RtPlanExpectations<'_>,
) -> Result<ValidatedPart10, GenerateError> {
    let bytes = fs::read(path).map_err(|source| GenerateError::ReadGeneratedFile {
        path: path.to_path_buf(),
        source,
    })?;
    let obj = open_file(path).map_err(|err| GenerateError::ValidateDicomFile {
        path: path.to_path_buf(),
        message: err.to_string(),
    })?;
    let plan_expected = expected.expected_rt_plan;
    let mut internal = Vec::new();
    check_equal(
        &mut internal,
        "rt_plan_manifest_fraction_group_count",
        "Manifest contains exactly one fraction group.",
        "Manifest fraction-group cardinality is invalid.",
        plan_expected.fraction_groups.len(),
        1,
    );
    check_equal(
        &mut internal,
        "rt_plan_manifest_beam_count",
        "Manifest contains exactly one beam.",
        "Manifest beam cardinality is invalid.",
        plan_expected.beams.len(),
        1,
    );
    fail_if_any_failed(path, &internal)?;
    let structure_expected = plan_expected.references[0];
    let dose_expected = plan_expected.references[1];
    let fraction_expected = plan_expected.fraction_groups[0];
    let beam_expected = plan_expected.beams[0];
    for (name, actual, locked) in [
        (
            "manifest_referenced_beam_count",
            fraction_expected.referenced_beams.len(),
            1,
        ),
        (
            "manifest_device_count",
            beam_expected.beam_limiting_devices.len(),
            2,
        ),
        (
            "manifest_control_point_count",
            beam_expected.control_points.len(),
            2,
        ),
    ] {
        check_equal(
            &mut internal,
            &format!("rt_plan_{name}"),
            "Manifest nested cardinality matches the validator-owned recipe.",
            "Manifest nested cardinality does not match the validator-owned recipe.",
            actual,
            locked,
        );
    }
    fail_if_any_failed(path, &internal)?;

    check(
        &mut internal,
        bytes.len() >= 132 && &bytes[128..132] == b"DICM",
        "rt_plan_part10_preamble",
        "File has a Part 10 preamble and DICM marker.",
        "File is missing the Part 10 DICM marker.",
    );
    check_equal(
        &mut internal,
        "rt_plan_transfer_syntax",
        "Transfer Syntax matches the locked RT Plan recipe.",
        "Transfer Syntax does not match the locked RT Plan recipe.",
        trim_uid(obj.meta().transfer_syntax()).as_str(),
        plan_expected.transfer_syntax_uid,
    );
    let dataset_sop_class = element_str(path, &obj, tags::SOP_CLASS_UID)?;
    let dataset_sop_instance = element_str(path, &obj, tags::SOP_INSTANCE_UID)?;
    for (name, actual, locked) in [
        (
            "sop_class_uid",
            dataset_sop_class.as_str(),
            "1.2.840.10008.5.1.4.1.1.481.5",
        ),
        (
            "sop_instance_uid",
            dataset_sop_instance.as_str(),
            plan_expected.sop_instance_uid,
        ),
        (
            "synthetic_data",
            element_str(path, &obj, tags::SYNTHETIC_DATA)?.as_str(),
            expected.synthetic_data,
        ),
        (
            "study_instance_uid",
            element_str(path, &obj, tags::STUDY_INSTANCE_UID)?.as_str(),
            plan_expected.study_instance_uid,
        ),
        (
            "series_instance_uid",
            element_str(path, &obj, tags::SERIES_INSTANCE_UID)?.as_str(),
            plan_expected.series_instance_uid,
        ),
        (
            "frame_of_reference_uid",
            element_str(path, &obj, tags::FRAME_OF_REFERENCE_UID)?.as_str(),
            plan_expected.frame_of_reference_uid,
        ),
    ] {
        check_equal(
            &mut internal,
            &format!("rt_plan_{name}"),
            "RT Plan identity matches the locked recipe.",
            "RT Plan identity does not match the locked recipe.",
            actual,
            locked,
        );
    }
    for (name, actual, locked) in [
        (
            "media_storage_sop_class_uid",
            trim_uid(obj.meta().media_storage_sop_class_uid()),
            dataset_sop_class.clone(),
        ),
        (
            "media_storage_sop_instance_uid",
            trim_uid(obj.meta().media_storage_sop_instance_uid()),
            dataset_sop_instance.clone(),
        ),
        (
            "implementation_class_uid",
            trim_uid(obj.meta().implementation_class_uid()),
            expected.implementation_class_uid.to_string(),
        ),
    ] {
        check_equal(
            &mut internal,
            &format!("rt_plan_{name}"),
            "File Meta identity matches the locked dataset identity.",
            "File Meta identity does not match the locked dataset identity.",
            actual,
            locked,
        );
    }
    for (name, tag, locked) in [
        (
            "patient_name",
            tags::PATIENT_NAME,
            "DTS^Synthetic^Patient001",
        ),
        ("patient_id", tags::PATIENT_ID, "DTS-PATIENT-001"),
        ("patient_birth_date", tags::PATIENT_BIRTH_DATE, "19700101"),
        ("patient_sex", tags::PATIENT_SEX, "O"),
        ("study_date", tags::STUDY_DATE, "20260101"),
        ("study_time", tags::STUDY_TIME, "000000"),
        ("referring_physician", tags::REFERRING_PHYSICIAN_NAME, ""),
        ("study_id", tags::STUDY_ID, "DTS-RTSTRUCT"),
        ("accession_number", tags::ACCESSION_NUMBER, ""),
        ("modality", tags::MODALITY, "RTPLAN"),
        ("series_number", tags::SERIES_NUMBER, "72"),
        ("operators_name", tags::OPERATORS_NAME, ""),
        (
            "position_reference_indicator",
            tags::POSITION_REFERENCE_INDICATOR,
            "",
        ),
        ("manufacturer", tags::MANUFACTURER, "dicom-test-suite"),
        ("institution_name", tags::INSTITUTION_NAME, ""),
        ("institution_address", tags::INSTITUTION_ADDRESS, ""),
        (
            "manufacturer_model_name",
            tags::MANUFACTURER_MODEL_NAME,
            "Native Linked RT Plan",
        ),
        (
            "device_serial_number",
            tags::DEVICE_SERIAL_NUMBER,
            "DTS-RTPLAN-001",
        ),
        (
            "software_versions",
            tags::SOFTWARE_VERSIONS,
            crate::PACKAGE_VERSION,
        ),
        ("instance_number", tags::INSTANCE_NUMBER, "1"),
        ("label", tags::RT_PLAN_LABEL, "DTS_PLAN"),
        ("date", tags::RT_PLAN_DATE, "20260101"),
        ("time", tags::RT_PLAN_TIME, "000000"),
        ("geometry", tags::RT_PLAN_GEOMETRY, "PATIENT"),
    ] {
        check_equal(
            &mut internal,
            &format!("rt_plan_{name}"),
            "RT Plan IOD attribute matches the locked recipe.",
            "RT Plan IOD attribute does not match the locked recipe.",
            element_str(path, &obj, tag)?.as_str(),
            locked,
        );
    }

    for (name, actual, locked) in [
        ("manifest_iod_kind", plan_expected.iod_kind, "rt_plan"),
        (
            "manifest_sop_class",
            plan_expected.sop_class_uid,
            "1.2.840.10008.5.1.4.1.1.481.5",
        ),
        ("manifest_modality", plan_expected.modality, "RTPLAN"),
        ("manifest_plan_label", plan_expected.plan.label, "DTS_PLAN"),
        ("manifest_plan_date", plan_expected.plan.date, "20260101"),
        ("manifest_plan_time", plan_expected.plan.time, "000000"),
        (
            "manifest_plan_geometry",
            plan_expected.plan.geometry,
            "PATIENT",
        ),
        (
            "manifest_structure_role",
            structure_expected.relationship,
            "referenced_structure_set",
        ),
        (
            "manifest_structure_class",
            structure_expected.sop_class_uid,
            "1.2.840.10008.5.1.4.1.1.481.3",
        ),
        (
            "manifest_structure_case",
            structure_expected.source_case_id,
            "non-image/rt/structure_set_single_roi_explicit_le",
        ),
        (
            "manifest_structure_path",
            structure_expected.source_path,
            "non-image/rt/structure_set_single_roi_explicit_le/instance.dcm",
        ),
        (
            "manifest_dose_role",
            dose_expected.relationship,
            "referenced_dose",
        ),
        (
            "manifest_dose_class",
            dose_expected.sop_class_uid,
            "1.2.840.10008.5.1.4.1.1.481.2",
        ),
        (
            "manifest_dose_case",
            dose_expected.source_case_id,
            "non-image/rt/dose_grid_u16_explicit_le",
        ),
        (
            "manifest_dose_path",
            dose_expected.source_path,
            "non-image/rt/dose_grid_u16_explicit_le/instance.dcm",
        ),
    ] {
        check_equal(
            &mut internal,
            &format!("rt_plan_{name}"),
            "Manifest expectation matches the validator-owned RT Plan recipe.",
            "Manifest expectation does not match the validator-owned RT Plan recipe.",
            actual,
            locked,
        );
    }
    check(
        &mut internal,
        [structure_expected, dose_expected].iter().all(|reference| {
            reference.study_instance_uid == plan_expected.study_instance_uid
                && reference.frame_of_reference_uid == plan_expected.frame_of_reference_uid
        }),
        "rt_plan_manifest_shared_identity",
        "Manifest references share the Plan Study and Frame of Reference.",
        "Manifest references do not share the Plan Study and Frame of Reference.",
    );
    check(
        &mut internal,
        structure_expected.sop_instance_uid != dose_expected.sop_instance_uid,
        "rt_plan_manifest_distinct_references",
        "Structure Set and Dose references are distinct.",
        "Structure Set and Dose references are duplicated.",
    );
    check(
        &mut internal,
        [
            structure_expected.source_sha256,
            dose_expected.source_sha256,
        ]
        .iter()
        .all(|hash| {
            hash.len() == 64
                && hash
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        }),
        "rt_plan_manifest_source_hashes",
        "Manifest references retain lowercase SHA-256 source identities.",
        "Manifest source hash identity is malformed.",
    );
    check(
        &mut internal,
        structure_expected.ordinal == 1
            && dose_expected.ordinal == 2
            && fraction_expected.ordinal == 1
            && beam_expected.ordinal == 1
            && fraction_expected.referenced_beams[0].ordinal == 1
            && beam_expected.beam_limiting_devices[0].ordinal == 1
            && beam_expected.beam_limiting_devices[1].ordinal == 2
            && beam_expected.control_points[0].ordinal == 1
            && beam_expected.control_points[1].ordinal == 2,
        "rt_plan_manifest_order",
        "Manifest RT Plan arrays use explicit one-based order.",
        "Manifest RT Plan order or ordinal is invalid.",
    );
    check(
        &mut internal,
        fraction_expected.fraction_group_number == 1
            && fraction_expected.number_of_fractions_planned == 1
            && fraction_expected.number_of_beams == 1
            && fraction_expected.number_of_brachy_application_setups == 0
            && fraction_expected.referenced_beams[0].referenced_beam_number == 1
            && beam_expected.beam_number == 1
            && beam_expected.number_of_control_points == 2
            && beam_expected.final_cumulative_meterset_weight == 1,
        "rt_plan_manifest_topology",
        "Manifest fraction, beam, and control-point topology matches the recipe.",
        "Manifest fraction, beam, or control-point topology is invalid.",
    );
    check(
        &mut internal,
        beam_expected.treatment_machine_name == "DTS_LINAC"
            && beam_expected.primary_dosimeter_unit == "MU"
            && beam_expected.source_axis_distance_mm == 1000
            && beam_expected.beam_name == "DTS_STATIC_AP"
            && beam_expected.beam_type == "STATIC"
            && beam_expected.radiation_type == "PHOTON"
            && beam_expected.treatment_delivery_type == "TREATMENT"
            && beam_expected.accessories.number_of_wedges == 0
            && beam_expected.accessories.wedge_sequence_absent
            && beam_expected.accessories.number_of_compensators == 0
            && beam_expected.accessories.compensator_sequence_absent
            && beam_expected.accessories.number_of_boli == 0
            && beam_expected.accessories.bolus_sequence_absent
            && beam_expected.accessories.number_of_blocks == 0
            && beam_expected.accessories.block_sequence_absent
            && beam_expected.beam_limiting_devices[0].device_type == "X"
            && beam_expected.beam_limiting_devices[1].device_type == "Y"
            && beam_expected.beam_limiting_devices.iter().all(|device| {
                device.number_of_leaf_jaw_pairs == 1 && device.source_to_device_distance_mm == 500
            }),
        "rt_plan_manifest_beam_contract",
        "Manifest beam and accessory contract matches the validator-owned recipe.",
        "Manifest beam or accessory contract does not match the validator-owned recipe.",
    );
    let first_expected = beam_expected.control_points[0];
    let final_expected = beam_expected.control_points[1];
    let first_geometry = first_expected.geometry;
    check(
        &mut internal,
        first_expected.control_point_index == 0
            && first_expected.cumulative_meterset_weight == 0
            && first_expected
                .inherits_geometry_from_control_point
                .is_none()
            && first_geometry.is_some_and(|geometry| {
                geometry.nominal_beam_energy_mev == 6
                    && geometry.jaw_positions_mm == [[-50, 50], [-50, 50]]
                    && geometry.gantry_angle_degrees == 0
                    && geometry.gantry_rotation_direction == "NONE"
                    && geometry.beam_limiting_device_angle_degrees == 0
                    && geometry.beam_limiting_device_rotation_direction == "NONE"
                    && geometry.patient_support_angle_degrees == 0
                    && geometry.patient_support_rotation_direction == "NONE"
                    && geometry.table_top_vertical_position_mm == 0
                    && geometry.table_top_longitudinal_position_mm == 0
                    && geometry.table_top_lateral_position_mm == 0
                    && geometry.table_top_pitch_angle_degrees == 0
                    && geometry.table_top_pitch_rotation_direction == "NONE"
                    && geometry.table_top_roll_angle_degrees == 0
                    && geometry.table_top_roll_rotation_direction == "NONE"
                    && geometry.isocenter_position_mm == [0, 0, 0]
            })
            && final_expected.control_point_index == 1
            && final_expected.cumulative_meterset_weight == 1
            && final_expected.geometry.is_none()
            && final_expected.inherits_geometry_from_control_point == Some(0),
        "rt_plan_manifest_control_point_contract",
        "Manifest control points encode exact geometry and inheritance.",
        "Manifest control-point geometry or inheritance is invalid.",
    );
    let absent = plan_expected.absent_content;
    check(
        &mut internal,
        absent.referenced_rt_plan_sequence
            && absent.rt_prescription_module
            && absent.rt_tolerance_tables_module
            && absent.rt_patient_setup_module
            && absent.rt_brachy_application_setups_module
            && absent.approval_module
            && absent.clinical_trial_module
            && absent.common_instance_reference_module
            && absent.image
            && absent.pixel_data,
        "rt_plan_manifest_absence_contract",
        "Manifest records every locked RT Plan absence.",
        "Manifest weakens a locked RT Plan absence.",
    );

    for (name, tag) in [
        (
            "structure_reference_count",
            tags::REFERENCED_STRUCTURE_SET_SEQUENCE,
        ),
        ("dose_reference_count", tags::REFERENCED_DOSE_SEQUENCE),
        ("fraction_group_count", tags::FRACTION_GROUP_SEQUENCE),
        ("beam_count", tags::BEAM_SEQUENCE),
    ] {
        check_equal(
            &mut internal,
            &format!("rt_plan_{name}"),
            "Locked RT Plan sequence contains exactly one item.",
            "Locked RT Plan sequence cardinality is invalid.",
            sequence_item_count(path, &obj, tag)?,
            1,
        );
    }
    fail_if_any_failed(path, &internal)?;

    for (sequence_tag, prefix, reference) in [
        (
            tags::REFERENCED_STRUCTURE_SET_SEQUENCE,
            "structure",
            structure_expected,
        ),
        (tags::REFERENCED_DOSE_SEQUENCE, "dose", dose_expected),
    ] {
        let item = top_level_sequence_item(path, &obj, sequence_tag, 0)?;
        for (suffix, tag, locked) in [
            (
                "sop_class_uid",
                TAG_REFERENCED_SOP_CLASS_UID,
                reference.sop_class_uid,
            ),
            (
                "sop_instance_uid",
                TAG_REFERENCED_SOP_INSTANCE_UID,
                reference.sop_instance_uid,
            ),
        ] {
            check_equal(
                &mut internal,
                &format!("rt_plan_{prefix}_{suffix}"),
                "RT Plan reference matches the locked upstream identity.",
                "RT Plan reference does not match the locked upstream identity.",
                item_str(path, item, tag)?.as_str(),
                locked,
            );
        }
    }

    let fraction = top_level_sequence_item(path, &obj, tags::FRACTION_GROUP_SEQUENCE, 0)?;
    for (name, tag, locked) in [
        (
            "fraction_group_number",
            tags::FRACTION_GROUP_NUMBER,
            fraction_expected.fraction_group_number,
        ),
        (
            "fractions_planned",
            tags::NUMBER_OF_FRACTIONS_PLANNED,
            fraction_expected.number_of_fractions_planned,
        ),
        (
            "fraction_beam_count",
            tags::NUMBER_OF_BEAMS,
            fraction_expected.number_of_beams,
        ),
        (
            "brachy_setup_count",
            tags::NUMBER_OF_BRACHY_APPLICATION_SETUPS,
            fraction_expected.number_of_brachy_application_setups,
        ),
    ] {
        check_equal(
            &mut internal,
            &format!("rt_plan_{name}"),
            "Fraction Group value matches the locked recipe.",
            "Fraction Group value does not match the locked recipe.",
            item_u16(path, fraction, tag)?,
            u16::from(locked),
        );
    }
    check_equal(
        &mut internal,
        "rt_plan_referenced_beam_count",
        "Fraction Group contains exactly one referenced beam.",
        "Referenced Beam Sequence cardinality is invalid.",
        item_sequence_item_count(path, fraction, tags::REFERENCED_BEAM_SEQUENCE)?,
        1,
    );
    fail_if_any_failed(path, &internal)?;
    let fraction_beam = item_sequence_item(path, fraction, tags::REFERENCED_BEAM_SEQUENCE, 0)?;
    check_equal(
        &mut internal,
        "rt_plan_referenced_beam_number",
        "Fraction Group references the locked Beam Number.",
        "Fraction Group contains a dangling Beam Number.",
        item_u16(path, fraction_beam, tags::REFERENCED_BEAM_NUMBER)?,
        u16::from(fraction_expected.referenced_beams[0].referenced_beam_number),
    );

    let beam = top_level_sequence_item(path, &obj, tags::BEAM_SEQUENCE, 0)?;
    for (name, tag, locked) in [
        (
            "treatment_machine_name",
            tags::TREATMENT_MACHINE_NAME,
            "DTS_LINAC",
        ),
        ("primary_dosimeter_unit", tags::PRIMARY_DOSIMETER_UNIT, "MU"),
        ("beam_number", tags::BEAM_NUMBER, "1"),
        ("beam_name", tags::BEAM_NAME, "DTS_STATIC_AP"),
        ("beam_type", tags::BEAM_TYPE, "STATIC"),
        ("radiation_type", tags::RADIATION_TYPE, "PHOTON"),
        (
            "treatment_delivery_type",
            tags::TREATMENT_DELIVERY_TYPE,
            "TREATMENT",
        ),
    ] {
        check_equal(
            &mut internal,
            &format!("rt_plan_{name}"),
            "Beam value matches the locked recipe.",
            "Beam value does not match the locked recipe.",
            item_str(path, beam, tag)?.as_str(),
            locked,
        );
    }
    check_equal(
        &mut internal,
        "rt_plan_source_axis_distance",
        "Source-axis distance matches the locked recipe.",
        "Source-axis distance does not match the locked recipe.",
        item_f64(path, beam, tags::SOURCE_AXIS_DISTANCE)?,
        1000.0,
    );
    for (name, tag) in [
        ("wedges", tags::NUMBER_OF_WEDGES),
        ("compensators", tags::NUMBER_OF_COMPENSATORS),
        ("boli", tags::NUMBER_OF_BOLI),
        ("blocks", tags::NUMBER_OF_BLOCKS),
    ] {
        check_equal(
            &mut internal,
            &format!("rt_plan_number_of_{name}"),
            "Accessory count is present and zero.",
            "Accessory count is missing or nonzero.",
            item_u16(path, beam, tag)?,
            0,
        );
    }
    for (name, tag) in [
        ("wedge_sequence", tags::WEDGE_SEQUENCE),
        ("compensator_sequence", tags::COMPENSATOR_SEQUENCE),
        ("bolus_id", tags::BOLUS_ID),
        ("referenced_bolus_sequence", tags::REFERENCED_BOLUS_SEQUENCE),
        ("block_sequence", tags::BLOCK_SEQUENCE),
    ] {
        check(
            &mut internal,
            beam.element_opt(tag)
                .map_err(|err| validation_error(path, err))?
                .is_none(),
            &format!("rt_plan_{name}_absent"),
            "Conditional accessory content is absent for a zero count.",
            "Conditional accessory content is present despite a zero count.",
        );
    }
    check_equal(
        &mut internal,
        "rt_plan_device_count",
        "Beam Limiting Device Sequence contains X and Y.",
        "Beam Limiting Device Sequence cardinality is invalid.",
        item_sequence_item_count(path, beam, tags::BEAM_LIMITING_DEVICE_SEQUENCE)?,
        2,
    );
    check_equal(
        &mut internal,
        "rt_plan_control_point_count",
        "Control Point Sequence contains exactly two items.",
        "Control Point Sequence cardinality is invalid.",
        item_sequence_item_count(path, beam, tags::CONTROL_POINT_SEQUENCE)?,
        2,
    );
    check_equal(
        &mut internal,
        "rt_plan_number_of_control_points",
        "Number of Control Points matches the sequence.",
        "Number of Control Points does not match the sequence.",
        item_u16(path, beam, tags::NUMBER_OF_CONTROL_POINTS)?,
        2,
    );
    check_equal(
        &mut internal,
        "rt_plan_final_cumulative_meterset_weight",
        "Final meterset weight is one.",
        "Final meterset weight is not one.",
        item_f64(path, beam, tags::FINAL_CUMULATIVE_METERSET_WEIGHT)?,
        1.0,
    );
    fail_if_any_failed(path, &internal)?;

    for (index, device_expected) in beam_expected.beam_limiting_devices.iter().enumerate() {
        let device = item_sequence_item(path, beam, tags::BEAM_LIMITING_DEVICE_SEQUENCE, index)?;
        check_equal(
            &mut internal,
            &format!("rt_plan_device_{}_type", index + 1),
            "Beam limiting device order matches X then Y.",
            "Beam limiting device order is invalid.",
            item_str(path, device, tags::RT_BEAM_LIMITING_DEVICE_TYPE)?.as_str(),
            device_expected.device_type,
        );
        check_equal(
            &mut internal,
            &format!("rt_plan_device_{}_jaw_pairs", index + 1),
            "Beam limiting device has one jaw pair.",
            "Beam limiting device jaw-pair count is invalid.",
            item_u16(path, device, tags::NUMBER_OF_LEAF_JAW_PAIRS)?,
            1,
        );
        check_equal(
            &mut internal,
            &format!("rt_plan_device_{}_distance", index + 1),
            "Beam limiting device distance matches the recipe.",
            "Beam limiting device distance is invalid.",
            item_f64(path, device, tags::SOURCE_TO_BEAM_LIMITING_DEVICE_DISTANCE)?,
            500.0,
        );
    }

    let first = item_sequence_item(path, beam, tags::CONTROL_POINT_SEQUENCE, 0)?;
    let final_point = item_sequence_item(path, beam, tags::CONTROL_POINT_SEQUENCE, 1)?;
    check_equal(
        &mut internal,
        "rt_plan_control_point_1_index",
        "First control point index is zero.",
        "First control point index is invalid.",
        item_u16(path, first, tags::CONTROL_POINT_INDEX)?,
        0,
    );
    check_equal(
        &mut internal,
        "rt_plan_control_point_2_index",
        "Final control point index is one.",
        "Final control point index is invalid.",
        item_u16(path, final_point, tags::CONTROL_POINT_INDEX)?,
        1,
    );
    check_equal(
        &mut internal,
        "rt_plan_control_point_1_meterset",
        "First control point meterset is zero.",
        "First control point meterset is invalid.",
        item_f64(path, first, tags::CUMULATIVE_METERSET_WEIGHT)?,
        0.0,
    );
    check_equal(
        &mut internal,
        "rt_plan_control_point_2_meterset",
        "Final control point meterset is one.",
        "Final control point meterset is invalid.",
        item_f64(path, final_point, tags::CUMULATIVE_METERSET_WEIGHT)?,
        1.0,
    );
    check_equal(
        &mut internal,
        "rt_plan_jaw_position_count",
        "First control point has ordered X and Y jaw positions.",
        "First control point jaw-position cardinality is invalid.",
        item_sequence_item_count(path, first, tags::BEAM_LIMITING_DEVICE_POSITION_SEQUENCE)?,
        2,
    );
    fail_if_any_failed(path, &internal)?;
    for (index, device_type) in ["X", "Y"].iter().enumerate() {
        let jaws = item_sequence_item(
            path,
            first,
            tags::BEAM_LIMITING_DEVICE_POSITION_SEQUENCE,
            index,
        )?;
        check_equal(
            &mut internal,
            &format!("rt_plan_jaw_{}_type", index + 1),
            "Jaw position order matches X then Y.",
            "Jaw position device order is invalid.",
            item_str(path, jaws, tags::RT_BEAM_LIMITING_DEVICE_TYPE)?.as_str(),
            *device_type,
        );
        check_equal(
            &mut internal,
            &format!("rt_plan_jaw_{}_positions", index + 1),
            "Jaw positions match the locked aperture.",
            "Jaw positions do not match the locked aperture.",
            item_f64_values(path, jaws, tags::LEAF_JAW_POSITIONS)?,
            vec![-50.0, 50.0],
        );
    }
    for (name, tag, locked) in [
        ("nominal_energy", tags::NOMINAL_BEAM_ENERGY, 6.0),
        ("gantry_angle", tags::GANTRY_ANGLE, 0.0),
        (
            "beam_limiting_device_angle",
            tags::BEAM_LIMITING_DEVICE_ANGLE,
            0.0,
        ),
        ("patient_support_angle", tags::PATIENT_SUPPORT_ANGLE, 0.0),
        ("table_vertical", tags::TABLE_TOP_VERTICAL_POSITION, 0.0),
        (
            "table_longitudinal",
            tags::TABLE_TOP_LONGITUDINAL_POSITION,
            0.0,
        ),
        ("table_lateral", tags::TABLE_TOP_LATERAL_POSITION, 0.0),
        ("table_pitch", tags::TABLE_TOP_PITCH_ANGLE, 0.0),
        ("table_roll", tags::TABLE_TOP_ROLL_ANGLE, 0.0),
    ] {
        check_equal(
            &mut internal,
            &format!("rt_plan_control_point_1_{name}"),
            "Control-point geometry matches the locked recipe.",
            "Control-point geometry does not match the locked recipe.",
            item_f64(path, first, tag)?,
            locked,
        );
    }
    for (name, tag) in [
        ("gantry_rotation", tags::GANTRY_ROTATION_DIRECTION),
        (
            "beam_limiting_device_rotation",
            tags::BEAM_LIMITING_DEVICE_ROTATION_DIRECTION,
        ),
        (
            "patient_support_rotation",
            tags::PATIENT_SUPPORT_ROTATION_DIRECTION,
        ),
        (
            "table_pitch_rotation",
            tags::TABLE_TOP_PITCH_ROTATION_DIRECTION,
        ),
        (
            "table_roll_rotation",
            tags::TABLE_TOP_ROLL_ROTATION_DIRECTION,
        ),
    ] {
        check_equal(
            &mut internal,
            &format!("rt_plan_control_point_1_{name}"),
            "Control-point rotation direction is NONE.",
            "Control-point rotation direction is invalid.",
            item_str(path, first, tag)?.as_str(),
            "NONE",
        );
    }
    check_equal(
        &mut internal,
        "rt_plan_control_point_1_isocenter",
        "Isocenter is the locked origin.",
        "Isocenter does not match the locked origin.",
        item_f64_values(path, first, tags::ISOCENTER_POSITION)?,
        vec![0.0, 0.0, 0.0],
    );

    for (name, tag) in [
        ("nominal_energy", tags::NOMINAL_BEAM_ENERGY),
        (
            "jaw_positions",
            tags::BEAM_LIMITING_DEVICE_POSITION_SEQUENCE,
        ),
        ("gantry_angle", tags::GANTRY_ANGLE),
        ("gantry_rotation", tags::GANTRY_ROTATION_DIRECTION),
        (
            "beam_limiting_device_angle",
            tags::BEAM_LIMITING_DEVICE_ANGLE,
        ),
        (
            "beam_limiting_device_rotation",
            tags::BEAM_LIMITING_DEVICE_ROTATION_DIRECTION,
        ),
        ("patient_support_angle", tags::PATIENT_SUPPORT_ANGLE),
        (
            "patient_support_rotation",
            tags::PATIENT_SUPPORT_ROTATION_DIRECTION,
        ),
        ("table_vertical", tags::TABLE_TOP_VERTICAL_POSITION),
        ("table_longitudinal", tags::TABLE_TOP_LONGITUDINAL_POSITION),
        ("table_lateral", tags::TABLE_TOP_LATERAL_POSITION),
        ("table_pitch", tags::TABLE_TOP_PITCH_ANGLE),
        (
            "table_pitch_rotation",
            tags::TABLE_TOP_PITCH_ROTATION_DIRECTION,
        ),
        ("table_roll", tags::TABLE_TOP_ROLL_ANGLE),
        (
            "table_roll_rotation",
            tags::TABLE_TOP_ROLL_ROTATION_DIRECTION,
        ),
        ("isocenter", tags::ISOCENTER_POSITION),
    ] {
        check(
            &mut internal,
            final_point
                .element_opt(tag)
                .map_err(|err| validation_error(path, err))?
                .is_none(),
            &format!("rt_plan_control_point_2_{name}_inherited"),
            "Unchanged final control-point geometry is absent and inherited.",
            "Final control point repeats or changes inherited geometry.",
        );
    }
    for (name, tag) in [
        (
            "referenced_rt_plan_sequence",
            tags::REFERENCED_RT_PLAN_SEQUENCE,
        ),
        ("rt_prescription_module", tags::DOSE_REFERENCE_SEQUENCE),
        ("rt_tolerance_tables_module", tags::TOLERANCE_TABLE_SEQUENCE),
        ("rt_patient_setup_module", tags::PATIENT_SETUP_SEQUENCE),
        (
            "rt_brachy_application_setups_module",
            tags::APPLICATION_SETUP_SEQUENCE,
        ),
        ("approval_module", tags::APPROVAL_STATUS),
        ("approval_review_date", tags::REVIEW_DATE),
        ("approval_review_time", tags::REVIEW_TIME),
        ("approval_reviewer_name", tags::REVIEWER_NAME),
        ("clinical_trial_module", tags::CLINICAL_TRIAL_SPONSOR_NAME),
        (
            "clinical_trial_protocol_id",
            tags::CLINICAL_TRIAL_PROTOCOL_ID,
        ),
        (
            "clinical_trial_protocol_name",
            tags::CLINICAL_TRIAL_PROTOCOL_NAME,
        ),
        ("clinical_trial_site_id", tags::CLINICAL_TRIAL_SITE_ID),
        ("clinical_trial_site_name", tags::CLINICAL_TRIAL_SITE_NAME),
        ("clinical_trial_subject_id", tags::CLINICAL_TRIAL_SUBJECT_ID),
        (
            "clinical_trial_subject_reading_id",
            tags::CLINICAL_TRIAL_SUBJECT_READING_ID,
        ),
        (
            "common_instance_reference_module",
            tags::STUDIES_CONTAINING_OTHER_REFERENCED_INSTANCES_SEQUENCE,
        ),
        (
            "common_instance_referenced_series",
            tags::REFERENCED_SERIES_SEQUENCE,
        ),
        ("pixel_data", tags::PIXEL_DATA),
        ("rows", tags::ROWS),
        ("columns", tags::COLUMNS),
        ("samples_per_pixel", tags::SAMPLES_PER_PIXEL),
        (
            "photometric_interpretation",
            tags::PHOTOMETRIC_INTERPRETATION,
        ),
        ("planar_configuration", tags::PLANAR_CONFIGURATION),
        ("bits_allocated", tags::BITS_ALLOCATED),
        ("bits_stored", tags::BITS_STORED),
        ("high_bit", tags::HIGH_BIT),
        ("pixel_representation", tags::PIXEL_REPRESENTATION),
    ] {
        check(
            &mut internal,
            obj.element_opt(tag)
                .map_err(|err| validation_error(path, err))?
                .is_none(),
            &format!("rt_plan_{name}_absent"),
            "Locked optional module or pixel attribute is absent.",
            "Locked optional module or pixel attribute is unexpectedly present.",
        );
    }

    fail_if_any_failed(path, &internal)?;
    Ok(ValidatedPart10 {
        bytes,
        validation: serde_json::json!({
            "status": "passed",
            "internal": internal,
            "standards": [
                {
                    "name": "rt_plan_sop_class",
                    "status": "passed",
                    "message": "SOP Class UID matches RT Plan Storage in the 2026b reference."
                },
                {
                    "name": standard_transfer_syntax_validation_name(plan_expected.transfer_syntax_uid),
                    "status": "passed",
                    "message": standard_transfer_syntax_validation_message(plan_expected.transfer_syntax_uid)
                },
                {
                    "name": "synthetic_data_attribute",
                    "status": "passed",
                    "message": "Synthetic Data (0008,001C) is present with value YES."
                },
                {
                    "name": "rt_plan_modules",
                    "status": "passed",
                    "message": "RT General Plan, Fraction Scheme, Beam, control-point inheritance, reference closure, and absence invariants match the locked recipe."
                }
            ],
            "external": []
        }),
    })
}

pub(crate) fn validate_rt_structure_set_file(
    path: &Path,
    expected: &RtStructureSetExpectations<'_>,
) -> Result<ValidatedPart10, GenerateError> {
    let bytes = fs::read(path).map_err(|source| GenerateError::ReadGeneratedFile {
        path: path.to_path_buf(),
        source,
    })?;
    let obj = open_file(path).map_err(|err| GenerateError::ValidateDicomFile {
        path: path.to_path_buf(),
        message: err.to_string(),
    })?;

    let mut internal = Vec::new();
    check(
        &mut internal,
        bytes.len() >= 132 && &bytes[128..132] == b"DICM",
        "part10_preamble",
        "File has a 128-byte preamble followed by the DICM marker.",
        "File is missing the Part 10 DICM marker at byte offset 128.",
    );
    check_equal(
        &mut internal,
        "file_meta_transfer_syntax",
        "File Meta Information Transfer Syntax UID matches the recipe.",
        "File Meta Information Transfer Syntax UID does not match the recipe.",
        trim_uid(obj.meta().transfer_syntax()).as_str(),
        expected.transfer_syntax_uid,
    );

    let dataset_sop_class = element_str(path, &obj, tags::SOP_CLASS_UID)?;
    check_equal(
        &mut internal,
        "sop_class_uid_consistency",
        "Dataset SOP Class UID, File Meta SOP Class UID, and recipe SOP Class UID match.",
        "SOP Class UID differs between dataset, File Meta Information, or recipe.",
        dataset_sop_class.as_str(),
        expected.sop_class_uid,
    );
    check_equal(
        &mut internal,
        "media_storage_sop_class_uid",
        "File Meta SOP Class UID matches the dataset SOP Class UID.",
        "File Meta SOP Class UID does not match the dataset SOP Class UID.",
        trim_uid(obj.meta().media_storage_sop_class_uid()).as_str(),
        dataset_sop_class.as_str(),
    );
    let dataset_sop_instance = element_str(path, &obj, tags::SOP_INSTANCE_UID)?;
    check_equal(
        &mut internal,
        "sop_instance_uid_consistency",
        "Dataset SOP Instance UID, File Meta SOP Instance UID, and manifest UID match.",
        "SOP Instance UID differs between dataset, File Meta Information, or manifest.",
        dataset_sop_instance.as_str(),
        expected.sop_instance_uid,
    );
    check_equal(
        &mut internal,
        "media_storage_sop_instance_uid",
        "File Meta SOP Instance UID matches the dataset SOP Instance UID.",
        "File Meta SOP Instance UID does not match the dataset SOP Instance UID.",
        trim_uid(obj.meta().media_storage_sop_instance_uid()).as_str(),
        dataset_sop_instance.as_str(),
    );
    check_equal(
        &mut internal,
        "implementation_class_uid",
        "File Meta Implementation Class UID matches the deterministic generator UID.",
        "File Meta Implementation Class UID does not match the deterministic generator UID.",
        trim_uid(obj.meta().implementation_class_uid()).as_str(),
        expected.implementation_class_uid,
    );
    check_equal(
        &mut internal,
        "synthetic_data",
        "Synthetic Data is present and set to YES.",
        "Synthetic Data is missing or not set to YES.",
        element_str(path, &obj, tags::SYNTHETIC_DATA)?.as_str(),
        expected.synthetic_data,
    );
    check_equal(
        &mut internal,
        "rt_structure_set_modality",
        "RT Series Modality is RTSTRUCT.",
        "RT Series Modality does not match the recipe.",
        element_str(path, &obj, tags::MODALITY)?.as_str(),
        expected.modality,
    );
    check_equal(
        &mut internal,
        "rt_structure_set_frame_of_reference_uid",
        "RT Structure Set Frame of Reference UID matches the source image.",
        "RT Structure Set Frame of Reference UID does not match the source image.",
        element_str(path, &obj, tags::FRAME_OF_REFERENCE_UID)?.as_str(),
        expected.frame_of_reference_uid,
    );
    check_equal(
        &mut internal,
        "rt_structure_set_label",
        "Structure Set Label matches the recipe.",
        "Structure Set Label does not match the recipe.",
        element_str(path, &obj, tags::STRUCTURE_SET_LABEL)?.as_str(),
        expected.structure_set_label,
    );
    check_equal(
        &mut internal,
        "rt_structure_set_roi_sequence_items",
        "Structure Set ROI Sequence item count matches the recipe.",
        "Structure Set ROI Sequence item count does not match the recipe.",
        sequence_item_count(path, &obj, tags::STRUCTURE_SET_ROI_SEQUENCE)?,
        expected.structure_set_roi_items,
    );

    let referenced_for =
        top_level_sequence_item(path, &obj, tags::REFERENCED_FRAME_OF_REFERENCE_SEQUENCE, 0)?;
    check_equal(
        &mut internal,
        "rt_referenced_frame_of_reference_uid",
        "Referenced Frame of Reference UID matches the source image.",
        "Referenced Frame of Reference UID does not match the source image.",
        item_str(path, referenced_for, tags::FRAME_OF_REFERENCE_UID)?.as_str(),
        expected.frame_of_reference_uid,
    );
    let referenced_study =
        item_sequence_item(path, referenced_for, tags::RT_REFERENCED_STUDY_SEQUENCE, 0)?;
    let referenced_series = item_sequence_item(
        path,
        referenced_study,
        tags::RT_REFERENCED_SERIES_SEQUENCE,
        0,
    )?;
    check_equal(
        &mut internal,
        "rt_referenced_series_uid",
        "RT Referenced Series Sequence points to the source Series Instance UID.",
        "RT Referenced Series Sequence does not point to the source Series.",
        item_str(path, referenced_series, tags::SERIES_INSTANCE_UID)?.as_str(),
        expected.referenced_series_instance_uid,
    );
    let referenced_contour_image =
        item_sequence_item(path, referenced_series, tags::CONTOUR_IMAGE_SEQUENCE, 0)?;
    check_equal(
        &mut internal,
        "rt_referenced_contour_image_sop_class_uid",
        "RT referenced contour image SOP Class UID matches the source image.",
        "RT referenced contour image SOP Class UID does not match the source image.",
        item_str(path, referenced_contour_image, TAG_REFERENCED_SOP_CLASS_UID)?.as_str(),
        expected.referenced_sop_class_uid,
    );
    check_equal(
        &mut internal,
        "rt_referenced_contour_image_sop_instance_uid",
        "RT referenced contour image SOP Instance UID matches the source image.",
        "RT referenced contour image SOP Instance UID does not match the source image.",
        item_str(
            path,
            referenced_contour_image,
            TAG_REFERENCED_SOP_INSTANCE_UID,
        )?
        .as_str(),
        expected.referenced_sop_instance_uid,
    );

    let roi = top_level_sequence_item(path, &obj, tags::STRUCTURE_SET_ROI_SEQUENCE, 0)?;
    check_equal(
        &mut internal,
        "rt_structure_set_roi_number",
        "Structure Set ROI Number matches the recipe.",
        "Structure Set ROI Number does not match the recipe.",
        item_str(path, roi, tags::ROI_NUMBER)?.as_str(),
        expected.roi_number.to_string().as_str(),
    );
    check_equal(
        &mut internal,
        "rt_structure_set_roi_name",
        "Structure Set ROI Name matches the recipe.",
        "Structure Set ROI Name does not match the recipe.",
        item_str(path, roi, tags::ROI_NAME)?.as_str(),
        expected.roi_name,
    );
    check_equal(
        &mut internal,
        "rt_structure_set_roi_generation_algorithm",
        "ROI Generation Algorithm matches the recipe.",
        "ROI Generation Algorithm does not match the recipe.",
        item_str(path, roi, tags::ROI_GENERATION_ALGORITHM)?.as_str(),
        expected.roi_generation_algorithm,
    );

    check_equal(
        &mut internal,
        "rt_roi_contour_sequence_items",
        "ROI Contour Sequence item count matches the recipe.",
        "ROI Contour Sequence item count does not match the recipe.",
        sequence_item_count(path, &obj, tags::ROI_CONTOUR_SEQUENCE)?,
        expected.roi_contour_items,
    );
    let roi_contour = top_level_sequence_item(path, &obj, tags::ROI_CONTOUR_SEQUENCE, 0)?;
    check_equal(
        &mut internal,
        "rt_roi_contour_referenced_roi_number",
        "ROI Contour references the Structure Set ROI Number.",
        "ROI Contour does not reference the Structure Set ROI Number.",
        item_str(path, roi_contour, tags::REFERENCED_ROI_NUMBER)?.as_str(),
        expected.roi_number.to_string().as_str(),
    );
    let contour = item_sequence_item(path, roi_contour, tags::CONTOUR_SEQUENCE, 0)?;
    check_equal(
        &mut internal,
        "rt_contour_sequence_items",
        "Contour Sequence item count matches the recipe.",
        "Contour Sequence item count does not match the recipe.",
        roi_contour
            .element(tags::CONTOUR_SEQUENCE)
            .map_err(|err| validation_error(path, err))?
            .items()
            .ok_or_else(|| GenerateError::ValidateDicomFile {
                path: path.to_path_buf(),
                message: "Contour Sequence is not encoded as a sequence".to_string(),
            })?
            .len(),
        expected.contour_items,
    );
    check_equal(
        &mut internal,
        "rt_contour_geometric_type",
        "Contour Geometric Type matches the recipe.",
        "Contour Geometric Type does not match the recipe.",
        item_str(path, contour, tags::CONTOUR_GEOMETRIC_TYPE)?.as_str(),
        expected.contour_geometric_type,
    );
    check_equal(
        &mut internal,
        "rt_number_of_contour_points",
        "Number of Contour Points matches the recipe.",
        "Number of Contour Points does not match the recipe.",
        item_str(path, contour, tags::NUMBER_OF_CONTOUR_POINTS)?.as_str(),
        expected.contour_points.to_string().as_str(),
    );
    check_equal(
        &mut internal,
        "rt_contour_data",
        "Contour Data matches the recipe.",
        "Contour Data does not match the recipe.",
        item_str(path, contour, tags::CONTOUR_DATA)?.as_str(),
        expected.contour_data,
    );
    let contour_image = item_sequence_item(path, contour, tags::CONTOUR_IMAGE_SEQUENCE, 0)?;
    check_equal(
        &mut internal,
        "rt_contour_image_sop_class_uid",
        "Contour Image Sequence SOP Class UID matches the source image.",
        "Contour Image Sequence SOP Class UID does not match the source image.",
        item_str(path, contour_image, TAG_REFERENCED_SOP_CLASS_UID)?.as_str(),
        expected.referenced_sop_class_uid,
    );
    check_equal(
        &mut internal,
        "rt_contour_image_sop_instance_uid",
        "Contour Image Sequence SOP Instance UID matches the source image.",
        "Contour Image Sequence SOP Instance UID does not match the source image.",
        item_str(path, contour_image, TAG_REFERENCED_SOP_INSTANCE_UID)?.as_str(),
        expected.referenced_sop_instance_uid,
    );

    check_equal(
        &mut internal,
        "rt_roi_observations_sequence_items",
        "RT ROI Observations Sequence item count matches the recipe.",
        "RT ROI Observations Sequence item count does not match the recipe.",
        sequence_item_count(path, &obj, tags::RTROI_OBSERVATIONS_SEQUENCE)?,
        expected.rt_roi_observation_items,
    );
    let observation = top_level_sequence_item(path, &obj, tags::RTROI_OBSERVATIONS_SEQUENCE, 0)?;
    check_equal(
        &mut internal,
        "rt_roi_observation_referenced_roi_number",
        "RT ROI Observation references the Structure Set ROI Number.",
        "RT ROI Observation does not reference the Structure Set ROI Number.",
        item_str(path, observation, tags::REFERENCED_ROI_NUMBER)?.as_str(),
        expected.roi_number.to_string().as_str(),
    );
    check_equal(
        &mut internal,
        "rt_roi_interpreted_type",
        "RT ROI Interpreted Type matches the recipe.",
        "RT ROI Interpreted Type does not match the recipe.",
        item_str(path, observation, tags::RTROI_INTERPRETED_TYPE)?.as_str(),
        expected.roi_interpreted_type,
    );
    check_equal(
        &mut internal,
        "rt_roi_interpreter",
        "RT ROI Interpreter Type 2 attribute is present.",
        "RT ROI Interpreter does not match the recipe.",
        item_str(path, observation, tags::ROI_INTERPRETER)?.as_str(),
        expected.roi_interpreter,
    );
    check(
        &mut internal,
        obj.element_opt(tags::PIXEL_DATA)
            .map_err(|err| validation_error(path, err))?
            .is_none(),
        "rt_structure_set_pixel_data_absent",
        "RT Structure Set contains no Pixel Data.",
        "RT Structure Set unexpectedly contains Pixel Data.",
    );

    fail_if_any_failed(path, &internal)?;

    Ok(ValidatedPart10 {
        bytes,
        validation: serde_json::json!({
            "status": "passed",
            "internal": internal,
            "standards": [
                {
                    "name": standard_sop_class_validation_name(expected.sop_class_uid),
                    "status": "passed",
                    "message": standard_sop_class_validation_message(expected.sop_class_uid)
                },
                {
                    "name": standard_transfer_syntax_validation_name(expected.transfer_syntax_uid),
                    "status": "passed",
                    "message": standard_transfer_syntax_validation_message(expected.transfer_syntax_uid)
                },
                {
                    "name": "synthetic_data_attribute",
                    "status": "passed",
                    "message": "Synthetic Data (0008,001C) is present with value YES."
                },
                {
                    "name": "rt_structure_set_modules",
                    "status": "passed",
                    "message": "RT Series, Structure Set, ROI Contour, RT ROI Observations, and source references match the recipe."
                }
            ],
            "external": []
        }),
    })
}

pub(crate) fn validate_rt_dose_file(
    path: &Path,
    expected: &RtDoseExpectations<'_>,
) -> Result<ValidatedPart10, GenerateError> {
    let bytes = fs::read(path).map_err(|source| GenerateError::ReadGeneratedFile {
        path: path.to_path_buf(),
        source,
    })?;
    let obj = open_file(path).map_err(|err| GenerateError::ValidateDicomFile {
        path: path.to_path_buf(),
        message: err.to_string(),
    })?;

    let mut internal = Vec::new();
    check(
        &mut internal,
        bytes.len() >= 132 && &bytes[128..132] == b"DICM",
        "part10_preamble",
        "File has a 128-byte preamble followed by the DICM marker.",
        "File is missing the Part 10 DICM marker at byte offset 128.",
    );
    check_equal(
        &mut internal,
        "file_meta_transfer_syntax",
        "File Meta Information Transfer Syntax UID matches the recipe.",
        "File Meta Information Transfer Syntax UID does not match the recipe.",
        trim_uid(obj.meta().transfer_syntax()).as_str(),
        expected.transfer_syntax_uid,
    );

    let dataset_sop_class = element_str(path, &obj, tags::SOP_CLASS_UID)?;
    check_equal(
        &mut internal,
        "sop_class_uid_consistency",
        "Dataset SOP Class UID, File Meta SOP Class UID, and recipe SOP Class UID match.",
        "SOP Class UID differs between dataset, File Meta Information, or recipe.",
        dataset_sop_class.as_str(),
        expected.sop_class_uid,
    );
    check_equal(
        &mut internal,
        "media_storage_sop_class_uid",
        "File Meta SOP Class UID matches the dataset SOP Class UID.",
        "File Meta SOP Class UID does not match the dataset SOP Class UID.",
        trim_uid(obj.meta().media_storage_sop_class_uid()).as_str(),
        dataset_sop_class.as_str(),
    );
    let dataset_sop_instance = element_str(path, &obj, tags::SOP_INSTANCE_UID)?;
    check_equal(
        &mut internal,
        "sop_instance_uid_consistency",
        "Dataset SOP Instance UID, File Meta SOP Instance UID, and manifest UID match.",
        "SOP Instance UID differs between dataset, File Meta Information, or manifest.",
        dataset_sop_instance.as_str(),
        expected.sop_instance_uid,
    );
    check_equal(
        &mut internal,
        "media_storage_sop_instance_uid",
        "File Meta SOP Instance UID matches the dataset SOP Instance UID.",
        "File Meta SOP Instance UID does not match the dataset SOP Instance UID.",
        trim_uid(obj.meta().media_storage_sop_instance_uid()).as_str(),
        dataset_sop_instance.as_str(),
    );
    check_equal(
        &mut internal,
        "implementation_class_uid",
        "File Meta Implementation Class UID matches the deterministic generator UID.",
        "File Meta Implementation Class UID does not match the deterministic generator UID.",
        trim_uid(obj.meta().implementation_class_uid()).as_str(),
        expected.implementation_class_uid,
    );
    check_equal(
        &mut internal,
        "synthetic_data",
        "Synthetic Data is present and set to YES.",
        "Synthetic Data is missing or not set to YES.",
        element_str(path, &obj, tags::SYNTHETIC_DATA)?.as_str(),
        expected.synthetic_data,
    );
    check_equal(
        &mut internal,
        "rt_dose_modality",
        "RT Series Modality is RTDOSE.",
        "RT Series Modality does not match the recipe.",
        element_str(path, &obj, tags::MODALITY)?.as_str(),
        expected.modality,
    );
    check_equal(
        &mut internal,
        "rt_dose_frame_of_reference_uid",
        "RT Dose Frame of Reference UID matches the source image.",
        "RT Dose Frame of Reference UID does not match the source image.",
        element_str(path, &obj, tags::FRAME_OF_REFERENCE_UID)?.as_str(),
        expected.frame_of_reference_uid,
    );
    check_equal(
        &mut internal,
        "rt_dose_rows",
        "Rows matches the dose grid recipe.",
        "Rows does not match the dose grid recipe.",
        element_u16(path, &obj, tags::ROWS)?,
        expected.rows,
    );
    check_equal(
        &mut internal,
        "rt_dose_columns",
        "Columns matches the dose grid recipe.",
        "Columns does not match the dose grid recipe.",
        element_u16(path, &obj, tags::COLUMNS)?,
        expected.columns,
    );
    check_equal(
        &mut internal,
        "rt_dose_number_of_frames",
        "Number of Frames matches the dose grid recipe.",
        "Number of Frames does not match the dose grid recipe.",
        element_str(path, &obj, tags::NUMBER_OF_FRAMES)?.as_str(),
        expected.frames.to_string().as_str(),
    );
    check_equal(
        &mut internal,
        "rt_dose_samples_per_pixel",
        "Samples per Pixel is one for the dose grid.",
        "Samples per Pixel does not match the dose grid recipe.",
        element_u16(path, &obj, tags::SAMPLES_PER_PIXEL)?,
        1,
    );
    check_equal(
        &mut internal,
        "rt_dose_photometric_interpretation",
        "Photometric Interpretation is MONOCHROME2 for the dose grid.",
        "Photometric Interpretation does not match the dose grid recipe.",
        element_str(path, &obj, tags::PHOTOMETRIC_INTERPRETATION)?.as_str(),
        "MONOCHROME2",
    );
    check_equal(
        &mut internal,
        "rt_dose_bits_allocated",
        "Bits Allocated is 16 for the dose grid.",
        "Bits Allocated does not match the dose grid recipe.",
        element_u16(path, &obj, tags::BITS_ALLOCATED)?,
        16,
    );
    check_equal(
        &mut internal,
        "rt_dose_bits_stored",
        "Bits Stored is 16 for the dose grid.",
        "Bits Stored does not match the dose grid recipe.",
        element_u16(path, &obj, tags::BITS_STORED)?,
        16,
    );
    check_equal(
        &mut internal,
        "rt_dose_high_bit",
        "High Bit is 15 for the dose grid.",
        "High Bit does not match the dose grid recipe.",
        element_u16(path, &obj, tags::HIGH_BIT)?,
        15,
    );
    check_equal(
        &mut internal,
        "rt_dose_pixel_representation",
        "Pixel Representation is unsigned for the dose grid.",
        "Pixel Representation does not match the dose grid recipe.",
        element_u16(path, &obj, tags::PIXEL_REPRESENTATION)?,
        0,
    );
    check_equal(
        &mut internal,
        "rt_dose_pixel_spacing",
        "Pixel Spacing matches the dose grid recipe.",
        "Pixel Spacing does not match the dose grid recipe.",
        element_str(path, &obj, tags::PIXEL_SPACING)?.as_str(),
        expected.pixel_spacing,
    );
    check_equal(
        &mut internal,
        "rt_dose_image_orientation_patient",
        "Image Orientation Patient matches the dose grid recipe.",
        "Image Orientation Patient does not match the dose grid recipe.",
        element_str(path, &obj, tags::IMAGE_ORIENTATION_PATIENT)?.as_str(),
        expected.image_orientation_patient,
    );
    check_equal(
        &mut internal,
        "rt_dose_image_position_patient",
        "Image Position Patient matches the dose grid recipe.",
        "Image Position Patient does not match the dose grid recipe.",
        element_str(path, &obj, tags::IMAGE_POSITION_PATIENT)?.as_str(),
        expected.image_position_patient,
    );
    check_equal(
        &mut internal,
        "rt_dose_slice_thickness",
        "Slice Thickness matches the dose grid recipe.",
        "Slice Thickness does not match the dose grid recipe.",
        element_str(path, &obj, tags::SLICE_THICKNESS)?.as_str(),
        expected.slice_thickness,
    );
    check_equal(
        &mut internal,
        "rt_dose_frame_increment_pointer",
        "Frame Increment Pointer points to Grid Frame Offset Vector.",
        "Frame Increment Pointer does not point to Grid Frame Offset Vector.",
        element_tag(path, &obj, tags::FRAME_INCREMENT_POINTER)?,
        expected.frame_increment_pointer,
    );
    check_equal(
        &mut internal,
        "rt_dose_grid_frame_offset_vector",
        "Grid Frame Offset Vector matches the dose grid recipe.",
        "Grid Frame Offset Vector does not match the dose grid recipe.",
        element_str(path, &obj, tags::GRID_FRAME_OFFSET_VECTOR)?.as_str(),
        expected.grid_frame_offset_vector,
    );
    check_equal(
        &mut internal,
        "rt_dose_units",
        "Dose Units matches the recipe.",
        "Dose Units does not match the recipe.",
        element_str(path, &obj, tags::DOSE_UNITS)?.as_str(),
        expected.dose_units,
    );
    check_equal(
        &mut internal,
        "rt_dose_type",
        "Dose Type matches the recipe.",
        "Dose Type does not match the recipe.",
        element_str(path, &obj, tags::DOSE_TYPE)?.as_str(),
        expected.dose_type,
    );
    check_equal(
        &mut internal,
        "rt_dose_summation_type",
        "Dose Summation Type matches the recipe.",
        "Dose Summation Type does not match the recipe.",
        element_str(path, &obj, tags::DOSE_SUMMATION_TYPE)?.as_str(),
        expected.dose_summation_type,
    );
    check_equal(
        &mut internal,
        "rt_dose_grid_scaling",
        "Dose Grid Scaling matches the recipe.",
        "Dose Grid Scaling does not match the recipe.",
        element_str(path, &obj, tags::DOSE_GRID_SCALING)?.as_str(),
        expected.dose_grid_scaling,
    );

    let pixel_element = obj
        .element(tags::PIXEL_DATA)
        .map_err(|err| validation_error(path, err))?;
    check_equal(
        &mut internal,
        "rt_dose_pixel_data_vr",
        "Pixel Data VR is OW for the 16-bit dose grid.",
        "Pixel Data VR does not match the dose grid recipe.",
        pixel_element.vr(),
        expected.pixel_vr,
    );
    let pixel_bytes = pixel_element
        .value()
        .to_bytes()
        .map_err(|err| validation_error(path, err))?;
    check_equal(
        &mut internal,
        "rt_dose_pixel_data_length",
        "Pixel Data length matches Rows * Columns * Frames * two bytes.",
        "Pixel Data length does not match the dose grid shape.",
        pixel_bytes.len(),
        expected.pixel_bytes_len,
    );

    let referenced_image = top_level_sequence_item(path, &obj, TAG_REFERENCED_IMAGE_SEQUENCE, 0)?;
    check_equal(
        &mut internal,
        "rt_dose_referenced_image_sop_class_uid",
        "Referenced Image Sequence SOP Class UID matches the source image.",
        "Referenced Image Sequence SOP Class UID does not match the source image.",
        item_str(path, referenced_image, TAG_REFERENCED_SOP_CLASS_UID)?.as_str(),
        expected.referenced_image_sop_class_uid,
    );
    check_equal(
        &mut internal,
        "rt_dose_referenced_image_sop_instance_uid",
        "Referenced Image Sequence SOP Instance UID matches the source image.",
        "Referenced Image Sequence SOP Instance UID does not match the source image.",
        item_str(path, referenced_image, TAG_REFERENCED_SOP_INSTANCE_UID)?.as_str(),
        expected.referenced_image_sop_instance_uid,
    );
    let referenced_structure_set =
        top_level_sequence_item(path, &obj, TAG_REFERENCED_STRUCTURE_SET_SEQUENCE, 0)?;
    check_equal(
        &mut internal,
        "rt_dose_referenced_structure_set_sop_class_uid",
        "Referenced Structure Set Sequence SOP Class UID matches the RT Structure Set.",
        "Referenced Structure Set Sequence SOP Class UID does not match the RT Structure Set.",
        item_str(path, referenced_structure_set, TAG_REFERENCED_SOP_CLASS_UID)?.as_str(),
        expected.referenced_structure_set_sop_class_uid,
    );
    check_equal(
        &mut internal,
        "rt_dose_referenced_structure_set_sop_instance_uid",
        "Referenced Structure Set Sequence SOP Instance UID matches the RT Structure Set.",
        "Referenced Structure Set Sequence SOP Instance UID does not match the RT Structure Set.",
        item_str(
            path,
            referenced_structure_set,
            TAG_REFERENCED_SOP_INSTANCE_UID,
        )?
        .as_str(),
        expected.referenced_structure_set_sop_instance_uid,
    );

    fail_if_any_failed(path, &internal)?;

    Ok(ValidatedPart10 {
        bytes,
        validation: serde_json::json!({
            "status": "passed",
            "internal": internal,
            "standards": [
                {
                    "name": standard_sop_class_validation_name(expected.sop_class_uid),
                    "status": "passed",
                    "message": standard_sop_class_validation_message(expected.sop_class_uid)
                },
                {
                    "name": standard_transfer_syntax_validation_name(expected.transfer_syntax_uid),
                    "status": "passed",
                    "message": standard_transfer_syntax_validation_message(expected.transfer_syntax_uid)
                },
                {
                    "name": "synthetic_data_attribute",
                    "status": "passed",
                    "message": "Synthetic Data (0008,001C) is present with value YES."
                },
                {
                    "name": "rt_dose_modules",
                    "status": "passed",
                    "message": "RT Series, grid-based Image Pixel, Multi-frame, RT Dose, and source references match the recipe."
                }
            ],
            "external": []
        }),
    })
}

pub(crate) fn validate_encapsulated_pdf_file(
    path: &Path,
    expected: &EncapsulatedPdfExpectations<'_>,
) -> Result<ValidatedPart10, GenerateError> {
    let bytes = fs::read(path).map_err(|source| GenerateError::ReadGeneratedFile {
        path: path.to_path_buf(),
        source,
    })?;
    let obj = open_file(path).map_err(|err| GenerateError::ValidateDicomFile {
        path: path.to_path_buf(),
        message: err.to_string(),
    })?;

    let mut internal = Vec::new();
    check(
        &mut internal,
        bytes.len() >= 132 && &bytes[128..132] == b"DICM",
        "part10_preamble",
        "File has a 128-byte preamble followed by the DICM marker.",
        "File is missing the Part 10 DICM marker at byte offset 128.",
    );
    check_equal(
        &mut internal,
        "file_meta_transfer_syntax",
        "File Meta Information Transfer Syntax UID matches the recipe.",
        "File Meta Information Transfer Syntax UID does not match the recipe.",
        trim_uid(obj.meta().transfer_syntax()).as_str(),
        expected.transfer_syntax_uid,
    );

    let dataset_sop_class = element_str(path, &obj, tags::SOP_CLASS_UID)?;
    check_equal(
        &mut internal,
        "sop_class_uid_consistency",
        "Dataset SOP Class UID, File Meta SOP Class UID, and recipe SOP Class UID match.",
        "SOP Class UID differs between dataset, File Meta Information, or recipe.",
        dataset_sop_class.as_str(),
        expected.sop_class_uid,
    );
    check_equal(
        &mut internal,
        "media_storage_sop_class_uid",
        "File Meta SOP Class UID matches the dataset SOP Class UID.",
        "File Meta SOP Class UID does not match the dataset SOP Class UID.",
        trim_uid(obj.meta().media_storage_sop_class_uid()).as_str(),
        dataset_sop_class.as_str(),
    );
    let dataset_sop_instance = element_str(path, &obj, tags::SOP_INSTANCE_UID)?;
    check_equal(
        &mut internal,
        "sop_instance_uid_consistency",
        "Dataset SOP Instance UID, File Meta SOP Instance UID, and manifest UID match.",
        "SOP Instance UID differs between dataset, File Meta Information, or manifest.",
        dataset_sop_instance.as_str(),
        expected.sop_instance_uid,
    );
    check_equal(
        &mut internal,
        "media_storage_sop_instance_uid",
        "File Meta SOP Instance UID matches the dataset SOP Instance UID.",
        "File Meta SOP Instance UID does not match the dataset SOP Instance UID.",
        trim_uid(obj.meta().media_storage_sop_instance_uid()).as_str(),
        dataset_sop_instance.as_str(),
    );
    check_equal(
        &mut internal,
        "implementation_class_uid",
        "File Meta Implementation Class UID matches the deterministic generator UID.",
        "File Meta Implementation Class UID does not match the deterministic generator UID.",
        trim_uid(obj.meta().implementation_class_uid()).as_str(),
        expected.implementation_class_uid,
    );
    check_equal(
        &mut internal,
        "synthetic_data",
        "Synthetic Data is present and set to YES.",
        "Synthetic Data is missing or not set to YES.",
        element_str(path, &obj, tags::SYNTHETIC_DATA)?.as_str(),
        expected.synthetic_data,
    );
    check_equal(
        &mut internal,
        "encapsulated_pdf_modality",
        "Encapsulated Document Series Modality matches the recipe.",
        "Encapsulated Document Series Modality does not match the recipe.",
        element_str(path, &obj, tags::MODALITY)?.as_str(),
        expected.modality,
    );
    check_equal(
        &mut internal,
        "encapsulated_pdf_conversion_type",
        "SC Equipment Conversion Type matches the recipe.",
        "SC Equipment Conversion Type does not match the recipe.",
        element_str(path, &obj, tags::CONVERSION_TYPE)?.as_str(),
        expected.conversion_type,
    );
    check_equal(
        &mut internal,
        "encapsulated_pdf_instance_number",
        "Instance Number matches the document recipe.",
        "Instance Number does not match the document recipe.",
        element_str(path, &obj, tags::INSTANCE_NUMBER)?.as_str(),
        expected.instance_number,
    );
    check_equal(
        &mut internal,
        "encapsulated_pdf_content_date",
        "Content Date Type 2 attribute is present and deterministic.",
        "Content Date does not match the recipe.",
        element_str(path, &obj, tags::CONTENT_DATE)?.as_str(),
        expected.content_date,
    );
    check_equal(
        &mut internal,
        "encapsulated_pdf_content_time",
        "Content Time Type 2 attribute is present and deterministic.",
        "Content Time does not match the recipe.",
        element_str(path, &obj, tags::CONTENT_TIME)?.as_str(),
        expected.content_time,
    );
    check_equal(
        &mut internal,
        "encapsulated_pdf_acquisition_datetime",
        "Acquisition DateTime Type 2 attribute is present and deterministic.",
        "Acquisition DateTime does not match the recipe.",
        element_str(path, &obj, tags::ACQUISITION_DATE_TIME)?.as_str(),
        expected.acquisition_datetime,
    );
    check_equal(
        &mut internal,
        "encapsulated_pdf_burned_in_annotation",
        "Burned In Annotation is NO for the synthetic de-identified PDF.",
        "Burned In Annotation does not match the recipe.",
        element_str(path, &obj, tags::BURNED_IN_ANNOTATION)?.as_str(),
        expected.burned_in_annotation,
    );
    check_equal(
        &mut internal,
        "encapsulated_pdf_recognizable_visual_features",
        "Recognizable Visual Features is NO for the synthetic PDF.",
        "Recognizable Visual Features does not match the recipe.",
        element_str(path, &obj, tags::RECOGNIZABLE_VISUAL_FEATURES)?.as_str(),
        expected.recognizable_visual_features,
    );
    check_equal(
        &mut internal,
        "encapsulated_pdf_document_title",
        "Document Title Type 2 attribute is present.",
        "Document Title does not match the recipe.",
        element_str(path, &obj, tags::DOCUMENT_TITLE)?.as_str(),
        expected.document_title,
    );
    let concept_name_sequence = obj
        .element(tags::CONCEPT_NAME_CODE_SEQUENCE)
        .map_err(|err| validation_error(path, err))?;
    check(
        &mut internal,
        concept_name_sequence
            .items()
            .is_some_and(|items| items.is_empty()),
        "encapsulated_pdf_concept_name_code_sequence",
        "Concept Name Code Sequence Type 2 attribute is present with zero items.",
        "Concept Name Code Sequence is missing or not an empty sequence.",
    );
    check_equal(
        &mut internal,
        "encapsulated_pdf_mime_type",
        "MIME Type of Encapsulated Document is application/pdf.",
        "MIME Type of Encapsulated Document does not match the recipe.",
        element_str(path, &obj, tags::MIME_TYPE_OF_ENCAPSULATED_DOCUMENT)?.as_str(),
        expected.mime_type,
    );
    check_equal(
        &mut internal,
        "encapsulated_pdf_document_length",
        "Encapsulated Document Length records the original unpadded PDF length.",
        "Encapsulated Document Length does not match the PDF payload length.",
        element_u32(path, &obj, tags::ENCAPSULATED_DOCUMENT_LENGTH)?,
        expected.document_bytes.len() as u32,
    );

    let document_element = obj
        .element(tags::ENCAPSULATED_DOCUMENT)
        .map_err(|err| validation_error(path, err))?;
    check_equal(
        &mut internal,
        "encapsulated_pdf_document_vr",
        "Encapsulated Document VR is OB.",
        "Encapsulated Document VR does not match the standard data element.",
        document_element.vr(),
        VR::OB,
    );
    let document_bytes = document_element
        .value()
        .to_bytes()
        .map_err(|err| validation_error(path, err))?;
    let document_value = document_bytes.as_ref();
    let has_expected_payload = document_value.starts_with(expected.document_bytes)
        && matches!(
            document_value
                .len()
                .checked_sub(expected.document_bytes.len()),
            Some(0) | Some(1)
        )
        && document_value[expected.document_bytes.len()..]
            .iter()
            .all(|byte| *byte == 0);
    check(
        &mut internal,
        has_expected_payload,
        "encapsulated_pdf_document_payload",
        "Encapsulated Document contains the deterministic PDF payload.",
        "Encapsulated Document payload does not match the recipe.",
    );
    check(
        &mut internal,
        obj.element_opt(tags::PIXEL_DATA)
            .map_err(|err| validation_error(path, err))?
            .is_none(),
        "encapsulated_pdf_pixel_data_absent",
        "Encapsulated PDF contains no Pixel Data.",
        "Encapsulated PDF unexpectedly contains Pixel Data.",
    );

    fail_if_any_failed(path, &internal)?;

    Ok(ValidatedPart10 {
        bytes,
        validation: serde_json::json!({
            "status": "passed",
            "internal": internal,
            "standards": [
                {
                    "name": standard_sop_class_validation_name(expected.sop_class_uid),
                    "status": "passed",
                    "message": standard_sop_class_validation_message(expected.sop_class_uid)
                },
                {
                    "name": standard_transfer_syntax_validation_name(expected.transfer_syntax_uid),
                    "status": "passed",
                    "message": standard_transfer_syntax_validation_message(expected.transfer_syntax_uid)
                },
                {
                    "name": "synthetic_data_attribute",
                    "status": "passed",
                    "message": "Synthetic Data (0008,001C) is present with value YES."
                },
                {
                    "name": "encapsulated_pdf_modules",
                    "status": "passed",
                    "message": "Encapsulated Document Series, SC Equipment, Encapsulated Document, and SOP Common attributes match the recipe."
                }
            ],
            "external": []
        }),
    })
}

fn element_str(path: &Path, obj: &OpenedObject, tag: Tag) -> Result<String, GenerateError> {
    let value = obj
        .element(tag)
        .map_err(|err| validation_error(path, err))?
        .value()
        .to_str()
        .map_err(|err| validation_error(path, err))?;
    Ok(value.trim_matches('\0').trim().to_string())
}

fn element_u16(path: &Path, obj: &OpenedObject, tag: Tag) -> Result<u16, GenerateError> {
    obj.element(tag)
        .map_err(|err| validation_error(path, err))?
        .value()
        .to_int::<u16>()
        .map_err(|err| validation_error(path, err))
}

fn element_tag(path: &Path, obj: &OpenedObject, tag: Tag) -> Result<Tag, GenerateError> {
    let tags = element_tags(path, obj, tag)?;
    tags.first()
        .copied()
        .ok_or_else(|| GenerateError::ValidateDicomFile {
            path: path.to_path_buf(),
            message: format!("{tag:?} is empty"),
        })
}

fn element_tags(path: &Path, obj: &OpenedObject, tag: Tag) -> Result<Vec<Tag>, GenerateError> {
    let tags = obj
        .element(tag)
        .map_err(|err| validation_error(path, err))?
        .value()
        .tags()
        .map_err(|err| validation_error(path, err))?;
    Ok(tags.to_vec())
}

fn element_u32(path: &Path, obj: &OpenedObject, tag: Tag) -> Result<u32, GenerateError> {
    obj.element(tag)
        .map_err(|err| validation_error(path, err))?
        .value()
        .to_int::<u32>()
        .map_err(|err| validation_error(path, err))
}

fn element_i16(path: &Path, obj: &OpenedObject, tag: Tag) -> Result<i16, GenerateError> {
    obj.element(tag)
        .map_err(|err| validation_error(path, err))?
        .value()
        .to_int::<i16>()
        .map_err(|err| validation_error(path, err))
}

fn expected_pixel_data_length(
    expected: &Part10Expectations<'_>,
) -> (&'static str, &'static str, usize) {
    let bytes_per_sample = usize::from(expected.bits_allocated).div_ceil(8);
    match expected.pixel_data_length_formula {
        PixelDataLengthFormula::ContiguousSamples => (
            "native_pixel_data_length",
            "Native Pixel Data length matches rows * columns * frames * samples per pixel * bytes per sample.",
            usize::from(expected.rows)
                * usize::from(expected.columns)
                * usize::from(expected.frames)
                * usize::from(expected.samples_per_pixel)
                * bytes_per_sample,
        ),
        PixelDataLengthFormula::YbrFull422 => (
            "native_ybr_full_422_pixel_data_length",
            "Native YBR_FULL_422 Pixel Data length matches rows * columns * frames * 2 * bytes per sample.",
            usize::from(expected.rows)
                * usize::from(expected.columns)
                * usize::from(expected.frames)
                * bytes_per_sample
                * 2,
        ),
        PixelDataLengthFormula::BitPackedFrames => {
            let frame_bits = usize::from(expected.rows)
                * usize::from(expected.columns)
                * usize::from(expected.samples_per_pixel);
            let value_length = usize::from(expected.frames) * frame_bits.div_ceil(8);
            (
                "native_bit_packed_pixel_data_length",
                "Native one-bit Pixel Data length matches byte-packed frames.",
                value_length + (value_length % 2),
            )
        }
        PixelDataLengthFormula::BitPackedContinuousFrames => {
            let value_bits = usize::from(expected.rows)
                * usize::from(expected.columns)
                * usize::from(expected.frames)
                * usize::from(expected.samples_per_pixel);
            let value_length = value_bits.div_ceil(8);
            (
                "native_continuous_bit_packed_pixel_data_length",
                "Native one-bit Pixel Data length packs frames continuously and pads only the complete Value Field.",
                value_length + (value_length % 2),
            )
        }
        PixelDataLengthFormula::Encapsulated { .. } => (
            "encapsulated_pixel_data_length",
            "Encapsulated Pixel Data uses an undefined value length.",
            0,
        ),
    }
}

fn validate_native_frame_hashes(
    expected: &Part10Expectations<'_>,
    pixel_bytes: &[u8],
    results: &mut Vec<Value>,
) {
    if expected.decoded_frame_hashes.is_empty() {
        return;
    }
    let bytes_per_sample = usize::from(expected.bits_allocated).div_ceil(8);
    let frame_length = usize::from(expected.rows)
        * usize::from(expected.columns)
        * usize::from(expected.samples_per_pixel)
        * bytes_per_sample;
    check_equal(
        results,
        "native_frame_hash_count",
        "Native frame hash count matches Number of Frames.",
        "Native frame hash count does not match Number of Frames.",
        expected.decoded_frame_hashes.len(),
        usize::from(expected.frames),
    );
    if frame_length == 0 || pixel_bytes.len() != frame_length * usize::from(expected.frames) {
        check(
            results,
            false,
            "native_frame_hashes",
            "Every native frame hash matches the recipe.",
            "Native frame bytes cannot be split according to the declared image shape.",
        );
        return;
    }
    let actual_hashes = pixel_bytes
        .chunks_exact(frame_length)
        .map(sha256_hex)
        .collect::<Vec<_>>();
    let expected_hashes = expected
        .decoded_frame_hashes
        .iter()
        .map(|hash| (*hash).to_string())
        .collect::<Vec<_>>();
    check_equal(
        results,
        "native_frame_hashes",
        "Every native frame hash matches the recipe.",
        "One or more native frame hashes do not match the recipe.",
        actual_hashes,
        expected_hashes,
    );
}

fn validate_rle_decoded_frame_hashes(
    expected: &Part10Expectations<'_>,
    fragments: &[Vec<u8>],
    results: &mut Vec<Value>,
) {
    if expected.transfer_syntax_uid != RLE_LOSSLESS_TRANSFER_SYNTAX_UID {
        return;
    }
    if expected.decoded_frame_hashes.is_empty() {
        check(
            results,
            false,
            "rle_decoded_frame_hashes",
            "RLE Lossless frames decode to the expected native frame hashes.",
            "RLE Lossless validation requires expected native frame hashes.",
        );
        return;
    }
    if fragments.len() != expected.decoded_frame_hashes.len() {
        check(
            results,
            false,
            "rle_decoded_frame_hash_count",
            "RLE Lossless decoded frame count matches expected native frame hash count.",
            "RLE Lossless fragment count does not match expected native frame hash count.",
        );
        return;
    }

    let decoder = NativeRleLosslessEncoder::new();
    let mut decoded_hashes = Vec::with_capacity(fragments.len());
    for fragment in fragments {
        match decoder.decode_frame(FrameDecodeInput {
            encoded_frame: fragment,
            rows: expected.rows,
            columns: expected.columns,
            samples_per_pixel: expected.samples_per_pixel,
            bits_allocated: expected.bits_allocated,
            bits_stored: expected.bits_stored,
            photometric_interpretation: expected.photometric_interpretation,
        }) {
            Ok(decoded) => decoded_hashes.push(sha256_hex(&decoded.native_bytes)),
            Err(_) => {
                check(
                    results,
                    false,
                    "rle_decode_round_trip",
                    "RLE Lossless frames decode successfully.",
                    "RLE Lossless frame decode failed.",
                );
                return;
            }
        }
    }

    check_equal(
        results,
        "rle_decoded_frame_hashes",
        "RLE Lossless frames decode to the expected native frame hashes.",
        "RLE Lossless decoded frame hashes do not match expected native frame hashes.",
        decoded_hashes
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        expected.decoded_frame_hashes.to_vec(),
    );
}

fn validate_jpeg_ls_lossless_decoded_frame_hashes(
    expected: &Part10Expectations<'_>,
    fragments: &[Vec<u8>],
    results: &mut Vec<Value>,
) {
    if expected.transfer_syntax_uid != JPEG_LS_LOSSLESS_TRANSFER_SYNTAX_UID {
        return;
    }
    if expected.decoded_frame_hashes.is_empty() {
        check(
            results,
            false,
            "jpeg_ls_lossless_decoded_frame_hashes",
            "JPEG-LS Lossless frames decode to the expected native frame hashes.",
            "JPEG-LS Lossless validation requires expected native frame hashes.",
        );
        return;
    }
    if fragments.len() != expected.decoded_frame_hashes.len() {
        check(
            results,
            false,
            "jpeg_ls_lossless_decoded_frame_hash_count",
            "JPEG-LS Lossless decoded frame count matches expected native frame hash count.",
            "JPEG-LS Lossless fragment count does not match expected native frame hash count.",
        );
        return;
    }

    #[cfg(feature = "charls")]
    {
        let decoder = DicomRsJpegLsLosslessEncoder::new();
        let mut decoded_hashes = Vec::with_capacity(fragments.len());
        for fragment in fragments {
            match decoder.decode_frame(FrameDecodeInput {
                encoded_frame: fragment,
                rows: expected.rows,
                columns: expected.columns,
                samples_per_pixel: expected.samples_per_pixel,
                bits_allocated: expected.bits_allocated,
                bits_stored: expected.bits_stored,
                photometric_interpretation: expected.photometric_interpretation,
            }) {
                Ok(decoded) => decoded_hashes.push(sha256_hex(&decoded.native_bytes)),
                Err(_) => {
                    check(
                        results,
                        false,
                        "jpeg_ls_lossless_decode_round_trip",
                        "JPEG-LS Lossless frames decode successfully.",
                        "JPEG-LS Lossless frame decode failed.",
                    );
                    return;
                }
            }
        }

        check_equal(
            results,
            "jpeg_ls_lossless_decoded_frame_hashes",
            "JPEG-LS Lossless frames decode to the expected native frame hashes.",
            "JPEG-LS Lossless decoded frame hashes do not match expected native frame hashes.",
            decoded_hashes
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            expected.decoded_frame_hashes.to_vec(),
        );
    }

    #[cfg(not(feature = "charls"))]
    {
        let _ = fragments;
        check(
            results,
            false,
            "jpeg_ls_lossless_decoder_unavailable",
            "JPEG-LS Lossless frames decode to the expected native frame hashes.",
            "JPEG-LS Lossless validation requires the charls Cargo feature.",
        );
    }
}

fn validate_jpeg_xl_lossless_decoded_frame_hashes(
    expected: &Part10Expectations<'_>,
    fragments: &[Vec<u8>],
    results: &mut Vec<Value>,
) {
    if expected.transfer_syntax_uid != JPEG_XL_LOSSLESS_TRANSFER_SYNTAX_UID {
        return;
    }
    if expected.decoded_frame_hashes.is_empty() {
        check(
            results,
            false,
            "jpeg_xl_lossless_decoded_frame_hashes",
            "JPEG XL Lossless frames decode to the expected native frame hashes.",
            "JPEG XL Lossless validation requires expected native frame hashes.",
        );
        return;
    }
    if fragments.len() != expected.decoded_frame_hashes.len() {
        check(
            results,
            false,
            "jpeg_xl_lossless_decoded_frame_hash_count",
            "JPEG XL Lossless decoded frame count matches expected native frame hash count.",
            "JPEG XL Lossless fragment count does not match expected native frame hash count.",
        );
        return;
    }

    #[cfg(feature = "jpegxl")]
    {
        let decoder = DicomRsJpegXlLosslessEncoder::new();
        let mut decoded_hashes = Vec::with_capacity(fragments.len());
        for fragment in fragments {
            match decoder.decode_frame(FrameDecodeInput {
                encoded_frame: fragment,
                rows: expected.rows,
                columns: expected.columns,
                samples_per_pixel: expected.samples_per_pixel,
                bits_allocated: expected.bits_allocated,
                bits_stored: expected.bits_stored,
                photometric_interpretation: expected.photometric_interpretation,
            }) {
                Ok(decoded) => decoded_hashes.push(sha256_hex(&decoded.native_bytes)),
                Err(_) => {
                    check(
                        results,
                        false,
                        "jpeg_xl_lossless_decode_round_trip",
                        "JPEG XL Lossless frames decode successfully.",
                        "JPEG XL Lossless frame decode failed.",
                    );
                    return;
                }
            }
        }

        check_equal(
            results,
            "jpeg_xl_lossless_decoded_frame_hashes",
            "JPEG XL Lossless frames decode to the expected native frame hashes.",
            "JPEG XL Lossless decoded frame hashes do not match expected native frame hashes.",
            decoded_hashes
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            expected.decoded_frame_hashes.to_vec(),
        );
    }

    #[cfg(not(feature = "jpegxl"))]
    {
        let _ = fragments;
        check(
            results,
            false,
            "jpeg_xl_lossless_decoder_unavailable",
            "JPEG XL Lossless frames decode to the expected native frame hashes.",
            "JPEG XL Lossless validation requires the jpegxl Cargo feature.",
        );
    }
}

fn validate_jpeg_2000_lossless_decoded_frame_hashes(
    expected: &Part10Expectations<'_>,
    fragments: &[Vec<u8>],
    results: &mut Vec<Value>,
) {
    if expected.transfer_syntax_uid != JPEG_2000_LOSSLESS_TRANSFER_SYNTAX_UID {
        return;
    }
    if expected.decoded_frame_hashes.is_empty() {
        check(
            results,
            false,
            "jpeg_2000_lossless_decoded_frame_hashes",
            "JPEG 2000 Lossless frames decode to the expected native frame hashes.",
            "JPEG 2000 Lossless validation requires expected native frame hashes.",
        );
        return;
    }
    if fragments.len() != expected.decoded_frame_hashes.len() {
        check(
            results,
            false,
            "jpeg_2000_lossless_decoded_frame_hash_count",
            "JPEG 2000 Lossless decoded frame count matches expected native frame hash count.",
            "JPEG 2000 Lossless fragment count does not match expected native frame hash count.",
        );
        return;
    }

    #[cfg(feature = "jpeg2000")]
    {
        let decoder = OpenJp2Jpeg2000LosslessEncoder::new();
        let mut decoded_hashes = Vec::with_capacity(fragments.len());
        for fragment in fragments {
            match decoder.decode_frame(FrameDecodeInput {
                encoded_frame: fragment,
                rows: expected.rows,
                columns: expected.columns,
                samples_per_pixel: expected.samples_per_pixel,
                bits_allocated: expected.bits_allocated,
                bits_stored: expected.bits_stored,
                photometric_interpretation: expected.photometric_interpretation,
            }) {
                Ok(decoded) => decoded_hashes.push(sha256_hex(&decoded.native_bytes)),
                Err(_) => {
                    check(
                        results,
                        false,
                        "jpeg_2000_lossless_decode_round_trip",
                        "JPEG 2000 Lossless frames decode successfully.",
                        "JPEG 2000 Lossless frame decode failed.",
                    );
                    return;
                }
            }
        }

        check_equal(
            results,
            "jpeg_2000_lossless_decoded_frame_hashes",
            "JPEG 2000 Lossless frames decode to the expected native frame hashes.",
            "JPEG 2000 Lossless decoded frame hashes do not match expected native frame hashes.",
            decoded_hashes
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            expected.decoded_frame_hashes.to_vec(),
        );
    }

    #[cfg(not(feature = "jpeg2000"))]
    {
        let _ = fragments;
        check(
            results,
            false,
            "jpeg_2000_lossless_decoder_unavailable",
            "JPEG 2000 Lossless frames decode to the expected native frame hashes.",
            "JPEG 2000 Lossless validation requires the jpeg2000 Cargo feature.",
        );
    }
}

fn validate_htj2k_lossless_decoded_frame_hashes(
    expected: &Part10Expectations<'_>,
    _file: &OpenedObject,
    fragments: &[Vec<u8>],
    results: &mut Vec<Value>,
) {
    if expected.transfer_syntax_uid != HTJ2K_LOSSLESS_TRANSFER_SYNTAX_UID {
        return;
    }
    if expected.decoded_frame_hashes.is_empty() {
        check(
            results,
            false,
            "htj2k_lossless_decoded_frame_hashes",
            "HTJ2K Lossless frames decode to the expected native frame hashes.",
            "HTJ2K Lossless validation requires expected native frame hashes.",
        );
        return;
    }
    if fragments.len() != expected.decoded_frame_hashes.len() {
        check(
            results,
            false,
            "htj2k_lossless_decoded_frame_hash_count",
            "HTJ2K Lossless decoded frame count matches expected native frame hash count.",
            "HTJ2K Lossless fragment count does not match expected native frame hash count.",
        );
        return;
    }

    #[cfg(feature = "htj2k_openjph")]
    {
        let decoder = OpenJphHtj2kLosslessEncoder::new();
        let mut decoded_hashes = Vec::with_capacity(fragments.len());
        for fragment in fragments {
            match decoder.decode_frame(FrameDecodeInput {
                encoded_frame: fragment,
                rows: expected.rows,
                columns: expected.columns,
                samples_per_pixel: expected.samples_per_pixel,
                bits_allocated: expected.bits_allocated,
                bits_stored: expected.bits_stored,
                photometric_interpretation: expected.photometric_interpretation,
            }) {
                Ok(decoded) => decoded_hashes.push(sha256_hex(&decoded.native_bytes)),
                Err(_) => {
                    check(
                        results,
                        false,
                        "htj2k_lossless_decode_round_trip",
                        "HTJ2K Lossless frames decode successfully.",
                        "HTJ2K Lossless frame decode failed.",
                    );
                    return;
                }
            }
        }

        check_equal(
            results,
            "htj2k_lossless_decoded_frame_hashes",
            "HTJ2K Lossless frames decode to the expected native frame hashes.",
            "HTJ2K Lossless decoded frame hashes do not match expected native frame hashes.",
            decoded_hashes
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            expected.decoded_frame_hashes.to_vec(),
        );
    }

    #[cfg(not(feature = "htj2k_openjph"))]
    {
        let _ = fragments;
        check(
            results,
            false,
            "htj2k_lossless_decoder_unavailable",
            "HTJ2K Lossless frames decode to the expected native frame hashes.",
            "HTJ2K Lossless validation requires the htj2k_openjph Cargo feature.",
        );
    }
}

fn validate_legacy_jpeg_lossless_decoded_frame_hashes(
    expected: &Part10Expectations<'_>,
    file: &OpenedObject,
    fragments: &[Vec<u8>],
    results: &mut Vec<Value>,
) {
    let Some(validation) =
        LegacyJpegLosslessValidation::for_transfer_syntax(expected.transfer_syntax_uid)
    else {
        return;
    };
    if expected.decoded_frame_hashes.is_empty() {
        check(
            results,
            false,
            validation.hash_check_name,
            validation.success_message,
            validation.missing_hash_message,
        );
        return;
    }
    if fragments.len() != expected.decoded_frame_hashes.len() {
        check(
            results,
            false,
            validation.count_check_name,
            validation.count_success_message,
            validation.count_failure_message,
        );
        return;
    }

    #[cfg(feature = "legacy_jpeg_dcmtk")]
    {
        let codec = if expected.transfer_syntax_uid == JPEG_LOSSLESS_PROCESS_14_TRANSFER_SYNTAX_UID
        {
            JPEG_LOSSLESS_NON_HIERARCHICAL.codec()
        } else {
            JPEG_LOSSLESS_NON_HIERARCHICAL_FIRST_ORDER_PREDICTION.codec()
        };
        let Codec::EncapsulatedPixelData(Some(reader), _) = codec else {
            check(
                results,
                false,
                validation.decoder_unavailable_name,
                validation.success_message,
                validation.decoder_unavailable_message,
            );
            return;
        };
        let mut decoded_hashes = Vec::with_capacity(fragments.len());
        for frame_index in 0..fragments.len() {
            let mut decoded = Vec::new();
            match reader.decode_frame(file, frame_index as u32, &mut decoded) {
                Ok(()) => decoded_hashes.push(sha256_hex(&decoded)),
                Err(_) => {
                    check(
                        results,
                        false,
                        validation.decode_check_name,
                        validation.decode_success_message,
                        validation.decode_failure_message,
                    );
                    return;
                }
            }
        }

        check_equal(
            results,
            validation.hash_check_name,
            validation.success_message,
            validation.hash_failure_message,
            decoded_hashes
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            expected.decoded_frame_hashes.to_vec(),
        );
    }

    #[cfg(not(feature = "legacy_jpeg_dcmtk"))]
    {
        let _ = (file, fragments);
        check(
            results,
            false,
            validation.decoder_unavailable_name,
            validation.success_message,
            validation.decoder_unavailable_message,
        );
    }
}

fn validate_deflated_image_frame_decoded_frame_hashes(
    expected: &Part10Expectations<'_>,
    fragments: &[Vec<u8>],
    results: &mut Vec<Value>,
) {
    if expected.transfer_syntax_uid != DEFLATED_IMAGE_FRAME_TRANSFER_SYNTAX_UID {
        return;
    }
    if expected.decoded_frame_hashes.is_empty() {
        check(
            results,
            false,
            "deflated_image_frame_decoded_frame_hashes",
            "Deflated Image Frame frames decode to the expected native frame hashes.",
            "Deflated Image Frame validation requires expected native frame hashes.",
        );
        return;
    }
    if fragments.len() != expected.decoded_frame_hashes.len() {
        check(
            results,
            false,
            "deflated_image_frame_decoded_frame_hash_count",
            "Deflated Image Frame decoded frame count matches expected native frame hash count.",
            "Deflated Image Frame fragment count does not match expected native frame hash count.",
        );
        return;
    }

    #[cfg(feature = "deflate")]
    {
        let decoder = DicomRsDeflatedImageFrameEncoder::new();
        let mut decoded_hashes = Vec::with_capacity(fragments.len());
        for fragment in fragments {
            match decoder.decode_frame(FrameDecodeInput {
                encoded_frame: fragment,
                rows: expected.rows,
                columns: expected.columns,
                samples_per_pixel: expected.samples_per_pixel,
                bits_allocated: expected.bits_allocated,
                bits_stored: expected.bits_stored,
                photometric_interpretation: expected.photometric_interpretation,
            }) {
                Ok(decoded) => decoded_hashes.push(sha256_hex(&decoded.native_bytes)),
                Err(_) => {
                    check(
                        results,
                        false,
                        "deflated_image_frame_decode_round_trip",
                        "Deflated Image Frame frames decode successfully.",
                        "Deflated Image Frame frame decode failed.",
                    );
                    return;
                }
            }
        }

        check_equal(
            results,
            "deflated_image_frame_decoded_frame_hashes",
            "Deflated Image Frame frames decode to the expected native frame hashes.",
            "Deflated Image Frame decoded frame hashes do not match expected native frame hashes.",
            decoded_hashes
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            expected.decoded_frame_hashes.to_vec(),
        );
    }

    #[cfg(not(feature = "deflate"))]
    {
        let _ = fragments;
        check(
            results,
            false,
            "deflated_image_frame_decoder_unavailable",
            "Deflated Image Frame frames decode to the expected native frame hashes.",
            "Deflated Image Frame validation requires the deflate Cargo feature.",
        );
    }
}

#[allow(dead_code)]
struct LegacyJpegLosslessValidation {
    hash_check_name: &'static str,
    count_check_name: &'static str,
    decoder_unavailable_name: &'static str,
    decode_check_name: &'static str,
    success_message: &'static str,
    missing_hash_message: &'static str,
    count_success_message: &'static str,
    count_failure_message: &'static str,
    decoder_unavailable_message: &'static str,
    decode_success_message: &'static str,
    decode_failure_message: &'static str,
    hash_failure_message: &'static str,
}

impl LegacyJpegLosslessValidation {
    fn for_transfer_syntax(transfer_syntax_uid: &str) -> Option<Self> {
        match transfer_syntax_uid {
            JPEG_LOSSLESS_PROCESS_14_TRANSFER_SYNTAX_UID => Some(Self {
                hash_check_name: "jpeg_lossless_process_14_decoded_frame_hashes",
                count_check_name: "jpeg_lossless_process_14_decoded_frame_hash_count",
                decoder_unavailable_name: "jpeg_lossless_process_14_decoder_unavailable",
                decode_check_name: "jpeg_lossless_process_14_decode_round_trip",
                success_message: "JPEG Lossless Process 14 frames decode to the expected native frame hashes.",
                missing_hash_message: "JPEG Lossless Process 14 validation requires expected native frame hashes.",
                count_success_message: "JPEG Lossless Process 14 decoded frame count matches expected native frame hash count.",
                count_failure_message: "JPEG Lossless Process 14 fragment count does not match expected native frame hash count.",
                decoder_unavailable_message: "JPEG Lossless Process 14 validation requires the legacy_jpeg_dcmtk Cargo feature.",
                decode_success_message: "JPEG Lossless Process 14 frames decode successfully.",
                decode_failure_message: "JPEG Lossless Process 14 frame decode failed.",
                hash_failure_message: "JPEG Lossless Process 14 decoded frame hashes do not match expected native frame hashes.",
            }),
            JPEG_LOSSLESS_SV1_TRANSFER_SYNTAX_UID => Some(Self {
                hash_check_name: "jpeg_lossless_sv1_decoded_frame_hashes",
                count_check_name: "jpeg_lossless_sv1_decoded_frame_hash_count",
                decoder_unavailable_name: "jpeg_lossless_sv1_decoder_unavailable",
                decode_check_name: "jpeg_lossless_sv1_decode_round_trip",
                success_message: "JPEG Lossless SV1 frames decode to the expected native frame hashes.",
                missing_hash_message: "JPEG Lossless SV1 validation requires expected native frame hashes.",
                count_success_message: "JPEG Lossless SV1 decoded frame count matches expected native frame hash count.",
                count_failure_message: "JPEG Lossless SV1 fragment count does not match expected native frame hash count.",
                decoder_unavailable_message: "JPEG Lossless SV1 validation requires the legacy_jpeg_dcmtk Cargo feature.",
                decode_success_message: "JPEG Lossless SV1 frames decode successfully.",
                decode_failure_message: "JPEG Lossless SV1 frame decode failed.",
                hash_failure_message: "JPEG Lossless SV1 decoded frame hashes do not match expected native frame hashes.",
            }),
            _ => None,
        }
    }
}

fn validate_photometric_shape(expected: &Part10Expectations<'_>, results: &mut Vec<Value>) {
    let samples_per_pixel_valid = match expected.photometric_interpretation {
        "MONOCHROME1" | "MONOCHROME2" | "PALETTE COLOR" => expected.samples_per_pixel == 1,
        "RGB" | "YBR_FULL" | "YBR_FULL_422" => expected.samples_per_pixel == 3,
        _ => true,
    };
    check(
        results,
        samples_per_pixel_valid,
        "photometric_samples_per_pixel",
        "Samples per Pixel is consistent with Photometric Interpretation.",
        "Samples per Pixel is not consistent with Photometric Interpretation.",
    );

    let planar_configuration_valid = if expected.samples_per_pixel > 1 {
        expected.planar_configuration.is_some()
    } else {
        expected.planar_configuration.is_none()
    };
    check(
        results,
        planar_configuration_valid,
        "photometric_planar_configuration_presence",
        "Planar Configuration presence is consistent with Samples per Pixel.",
        "Planar Configuration presence is not consistent with Samples per Pixel.",
    );

    if expected.photometric_interpretation == "YBR_FULL_422" {
        check(
            results,
            expected.planar_configuration == Some(0),
            "ybr_full_422_planar_configuration",
            "YBR_FULL_422 uses Planar Configuration 0.",
            "YBR_FULL_422 does not use Planar Configuration 0.",
        );
    }
}

fn validate_palette(
    path: &Path,
    obj: &OpenedObject,
    results: &mut Vec<Value>,
    expected: &PaletteExpectations,
) -> Result<(), GenerateError> {
    for (name, tag) in [
        (
            "red_palette_lut_descriptor",
            tags::RED_PALETTE_COLOR_LOOKUP_TABLE_DESCRIPTOR,
        ),
        (
            "green_palette_lut_descriptor",
            tags::GREEN_PALETTE_COLOR_LOOKUP_TABLE_DESCRIPTOR,
        ),
        (
            "blue_palette_lut_descriptor",
            tags::BLUE_PALETTE_COLOR_LOOKUP_TABLE_DESCRIPTOR,
        ),
    ] {
        check_equal(
            results,
            name,
            "Palette Color Lookup Table Descriptor matches the recipe.",
            "Palette Color Lookup Table Descriptor does not match the recipe.",
            element_u16_values(path, obj, tag)?,
            expected.descriptor.to_vec(),
        );
    }
    for (name, tag, expected_length) in [
        (
            "red_palette_lut_data",
            tags::RED_PALETTE_COLOR_LOOKUP_TABLE_DATA,
            expected.red_data_length,
        ),
        (
            "green_palette_lut_data",
            tags::GREEN_PALETTE_COLOR_LOOKUP_TABLE_DATA,
            expected.green_data_length,
        ),
        (
            "blue_palette_lut_data",
            tags::BLUE_PALETTE_COLOR_LOOKUP_TABLE_DATA,
            expected.blue_data_length,
        ),
    ] {
        let element = obj
            .element(tag)
            .map_err(|err| validation_error(path, err))?;
        let value_length = element
            .value()
            .to_bytes()
            .map(|bytes| bytes.len())
            .map_err(|err| validation_error(path, err))?;
        check_equal(
            results,
            name,
            "Palette Color Lookup Table Data VR and length match the recipe.",
            "Palette Color Lookup Table Data VR or length does not match the recipe.",
            (element.vr(), value_length),
            (VR::OW, expected_length),
        );
    }
    Ok(())
}

fn element_u16_values(
    path: &Path,
    obj: &OpenedObject,
    tag: Tag,
) -> Result<Vec<u16>, GenerateError> {
    obj.element(tag)
        .map_err(|err| validation_error(path, err))?
        .value()
        .to_multi_int::<u16>()
        .map_err(|err| validation_error(path, err))
}

fn element_i16_values(
    path: &Path,
    obj: &OpenedObject,
    tag: Tag,
) -> Result<Vec<i16>, GenerateError> {
    obj.element(tag)
        .map_err(|err| validation_error(path, err))?
        .value()
        .to_multi_int::<i16>()
        .map_err(|err| validation_error(path, err))
}

fn element_f64_values(
    path: &Path,
    obj: &OpenedObject,
    tag: Tag,
) -> Result<Vec<f64>, GenerateError> {
    element_str(path, obj, tag)?
        .split('\\')
        .map(|value| {
            value
                .parse::<f64>()
                .map_err(|err| GenerateError::ValidateDicomFile {
                    path: path.to_path_buf(),
                    message: format!("attribute {} contains invalid DS value: {err}", tag),
                })
        })
        .collect()
}

fn validate_pixel_padding(
    path: &Path,
    obj: &OpenedObject,
    results: &mut Vec<Value>,
    expected: &PixelPaddingExpectations,
) -> Result<(), GenerateError> {
    let pixel_representation = element_u16(path, obj, tags::PIXEL_REPRESENTATION)?;
    if pixel_representation == 1 {
        check_equal(
            results,
            "pixel_padding_value",
            "Pixel Padding Value matches the recipe.",
            "Pixel Padding Value does not match the recipe.",
            element_i16(path, obj, tags::PIXEL_PADDING_VALUE)?,
            expected.value,
        );
    } else {
        check_equal(
            results,
            "pixel_padding_value",
            "Pixel Padding Value matches the recipe.",
            "Pixel Padding Value does not match the recipe.",
            Some(element_u16(path, obj, tags::PIXEL_PADDING_VALUE)?),
            u16::try_from(expected.value).ok(),
        );
    }
    if let Some(expected_range_limit) = expected.range_limit {
        if pixel_representation == 1 {
            check_equal(
                results,
                "pixel_padding_range_limit",
                "Pixel Padding Range Limit matches the recipe.",
                "Pixel Padding Range Limit does not match the recipe.",
                element_i16(path, obj, tags::PIXEL_PADDING_RANGE_LIMIT)?,
                expected_range_limit,
            );
        } else {
            check_equal(
                results,
                "pixel_padding_range_limit",
                "Pixel Padding Range Limit matches the recipe.",
                "Pixel Padding Range Limit does not match the recipe.",
                Some(element_u16(path, obj, tags::PIXEL_PADDING_RANGE_LIMIT)?),
                u16::try_from(expected_range_limit).ok(),
            );
        }
    }
    Ok(())
}

fn validate_ct_image(
    path: &Path,
    obj: &OpenedObject,
    results: &mut Vec<Value>,
    expected: &CtImageExpectations<'_>,
) -> Result<(), GenerateError> {
    for (name, tag, expected_value) in [
        ("ct_modality", tags::MODALITY, expected.modality),
        (
            "ct_frame_of_reference_uid",
            tags::FRAME_OF_REFERENCE_UID,
            expected.frame_of_reference_uid,
        ),
        ("ct_image_type", tags::IMAGE_TYPE, expected.image_type),
        (
            "ct_pixel_spacing",
            tags::PIXEL_SPACING,
            expected.pixel_spacing,
        ),
        (
            "ct_image_orientation_patient",
            tags::IMAGE_ORIENTATION_PATIENT,
            expected.image_orientation_patient,
        ),
        (
            "ct_image_position_patient",
            tags::IMAGE_POSITION_PATIENT,
            expected.image_position_patient,
        ),
        (
            "ct_slice_thickness",
            tags::SLICE_THICKNESS,
            expected.slice_thickness,
        ),
        ("ct_kvp", tags::KVP, expected.kvp),
        (
            "ct_acquisition_number",
            tags::ACQUISITION_NUMBER,
            expected.acquisition_number,
        ),
        (
            "ct_rescale_intercept",
            tags::RESCALE_INTERCEPT,
            expected.rescale_intercept,
        ),
        (
            "ct_rescale_slope",
            tags::RESCALE_SLOPE,
            expected.rescale_slope,
        ),
        ("ct_rescale_type", tags::RESCALE_TYPE, expected.rescale_type),
        (
            "ct_window_center",
            tags::WINDOW_CENTER,
            expected.window_center,
        ),
        ("ct_window_width", tags::WINDOW_WIDTH, expected.window_width),
    ] {
        check_equal(
            results,
            name,
            "CT Image attribute matches the recipe.",
            "CT Image attribute does not match the recipe.",
            element_str(path, obj, tag)?.as_str(),
            expected_value,
        );
    }

    Ok(())
}

fn validate_enhanced_ct_image(
    path: &Path,
    obj: &OpenedObject,
    results: &mut Vec<Value>,
    expected: &EnhancedCtImageExpectations<'_>,
) -> Result<(), GenerateError> {
    for (name, tag, expected_value) in [
        ("enhanced_ct_modality", tags::MODALITY, expected.modality),
        (
            "enhanced_ct_frame_of_reference_uid",
            tags::FRAME_OF_REFERENCE_UID,
            expected.frame_of_reference_uid,
        ),
        (
            "enhanced_ct_image_type",
            tags::IMAGE_TYPE,
            expected.image_type,
        ),
        (
            "enhanced_ct_pixel_presentation",
            tags::PIXEL_PRESENTATION,
            expected.pixel_presentation,
        ),
        (
            "enhanced_ct_volumetric_properties",
            tags::VOLUMETRIC_PROPERTIES,
            expected.volumetric_properties,
        ),
        (
            "enhanced_ct_volume_based_calculation_technique",
            tags::VOLUME_BASED_CALCULATION_TECHNIQUE,
            expected.volume_based_calculation_technique,
        ),
    ] {
        check_equal(
            results,
            name,
            "Enhanced CT top-level attribute matches the recipe.",
            "Enhanced CT top-level attribute does not match the recipe.",
            element_str(path, obj, tag)?.as_str(),
            expected_value,
        );
    }

    let expected_number_of_frames = expected.number_of_frames.to_string();
    check_equal(
        results,
        "enhanced_ct_number_of_frames",
        "Number of Frames matches the recipe.",
        "Number of Frames does not match the recipe.",
        element_str(path, obj, tags::NUMBER_OF_FRAMES)?.as_str(),
        expected_number_of_frames.as_str(),
    );
    check_equal(
        results,
        "enhanced_ct_shared_functional_groups_sequence_items",
        "Shared Functional Groups Sequence has one item.",
        "Shared Functional Groups Sequence item count does not match the recipe.",
        sequence_item_count(path, obj, tags::SHARED_FUNCTIONAL_GROUPS_SEQUENCE)?,
        expected.shared_functional_groups,
    );
    check_equal(
        results,
        "enhanced_ct_per_frame_functional_groups_sequence_items",
        "Per-Frame Functional Groups Sequence has one item per frame.",
        "Per-Frame Functional Groups Sequence item count does not match Number of Frames.",
        sequence_item_count(path, obj, tags::PER_FRAME_FUNCTIONAL_GROUPS_SEQUENCE)?,
        expected.per_frame_functional_groups,
    );
    check_equal(
        results,
        "enhanced_ct_dimension_organization_sequence_items",
        "Dimension Organization Sequence has one item.",
        "Dimension Organization Sequence item count does not match the recipe.",
        sequence_item_count(path, obj, tags::DIMENSION_ORGANIZATION_SEQUENCE)?,
        1,
    );
    check_equal(
        results,
        "enhanced_ct_dimension_index_sequence_items",
        "Dimension Index Sequence item count matches the recipe.",
        "Dimension Index Sequence item count does not match the recipe.",
        sequence_item_count(path, obj, tags::DIMENSION_INDEX_SEQUENCE)?,
        expected.dimension_index_count,
    );
    check_equal(
        results,
        "enhanced_ct_dimension_organization_uid",
        "Dimension Organization UID matches between the recipe and Dimension Organization Sequence.",
        "Dimension Organization UID does not match the recipe.",
        top_level_sequence_item_str(
            path,
            obj,
            tags::DIMENSION_ORGANIZATION_SEQUENCE,
            0,
            tags::DIMENSION_ORGANIZATION_UID,
        )?
        .as_str(),
        expected.dimension_organization_uid,
    );

    if let Some(concatenation) = &expected.concatenation {
        check_equal(
            results,
            "enhanced_ct_concatenation_uid",
            "Concatenation UID matches the recipe.",
            "Concatenation UID does not match the recipe.",
            element_str(path, obj, tags::CONCATENATION_UID)?.as_str(),
            concatenation.concatenation_uid,
        );
        check_equal(
            results,
            "enhanced_ct_in_concatenation_number",
            "In-concatenation Number matches the recipe.",
            "In-concatenation Number does not match the recipe.",
            element_u16(path, obj, tags::IN_CONCATENATION_NUMBER)?,
            concatenation.in_concatenation_number,
        );
        check_equal(
            results,
            "enhanced_ct_in_concatenation_total_number",
            "In-concatenation Total Number matches the recipe.",
            "In-concatenation Total Number does not match the recipe.",
            element_u16(path, obj, tags::IN_CONCATENATION_TOTAL_NUMBER)?,
            concatenation.in_concatenation_total_number,
        );
        check_equal(
            results,
            "enhanced_ct_concatenation_frame_offset_number",
            "Concatenation Frame Offset Number matches the recipe.",
            "Concatenation Frame Offset Number does not match the recipe.",
            element_u32(path, obj, tags::CONCATENATION_FRAME_OFFSET_NUMBER)?,
            concatenation.concatenation_frame_offset_number,
        );
        check_equal(
            results,
            "enhanced_ct_sop_instance_uid_of_concatenation_source",
            "SOP Instance UID of Concatenation Source matches the recipe.",
            "SOP Instance UID of Concatenation Source does not match the recipe.",
            element_str(path, obj, tags::SOP_INSTANCE_UID_OF_CONCATENATION_SOURCE)?.as_str(),
            concatenation.sop_instance_uid_of_concatenation_source,
        );
    }

    let shared = top_level_sequence_item(path, obj, tags::SHARED_FUNCTIONAL_GROUPS_SEQUENCE, 0)?;
    check_equal(
        results,
        "enhanced_ct_pixel_measures_sequence_items",
        "Pixel Measures Sequence has one shared item.",
        "Pixel Measures Sequence item count does not match the recipe.",
        item_sequence_item_count(path, shared, tags::PIXEL_MEASURES_SEQUENCE)?,
        1,
    );
    check_equal(
        results,
        "enhanced_ct_pixel_spacing",
        "Shared Pixel Measures Pixel Spacing matches the recipe.",
        "Shared Pixel Measures Pixel Spacing does not match the recipe.",
        nested_sequence_item_str(
            path,
            shared,
            tags::PIXEL_MEASURES_SEQUENCE,
            0,
            tags::PIXEL_SPACING,
        )?
        .as_str(),
        expected.pixel_spacing,
    );
    check_equal(
        results,
        "enhanced_ct_image_orientation_patient",
        "Shared Plane Orientation Image Orientation Patient matches the recipe.",
        "Shared Plane Orientation Image Orientation Patient does not match the recipe.",
        nested_sequence_item_str(
            path,
            shared,
            tags::PLANE_ORIENTATION_SEQUENCE,
            0,
            tags::IMAGE_ORIENTATION_PATIENT,
        )?
        .as_str(),
        expected.image_orientation_patient,
    );
    check_equal(
        results,
        "enhanced_ct_frame_type",
        "Shared CT Image Frame Type matches the recipe.",
        "Shared CT Image Frame Type does not match the recipe.",
        nested_sequence_item_str(
            path,
            shared,
            tags::CT_IMAGE_FRAME_TYPE_SEQUENCE,
            0,
            tags::FRAME_TYPE,
        )?
        .as_str(),
        expected.frame_type,
    );
    check_equal(
        results,
        "enhanced_ct_rescale_intercept",
        "Shared CT Pixel Value Transformation rescale intercept matches the recipe.",
        "Shared CT Pixel Value Transformation rescale intercept does not match the recipe.",
        nested_sequence_item_str(
            path,
            shared,
            tags::PIXEL_VALUE_TRANSFORMATION_SEQUENCE,
            0,
            tags::RESCALE_INTERCEPT,
        )?
        .as_str(),
        expected.rescale_intercept,
    );
    check_equal(
        results,
        "enhanced_ct_rescale_slope",
        "Shared CT Pixel Value Transformation rescale slope matches the recipe.",
        "Shared CT Pixel Value Transformation rescale slope does not match the recipe.",
        nested_sequence_item_str(
            path,
            shared,
            tags::PIXEL_VALUE_TRANSFORMATION_SEQUENCE,
            0,
            tags::RESCALE_SLOPE,
        )?
        .as_str(),
        expected.rescale_slope,
    );
    check_equal(
        results,
        "enhanced_ct_rescale_type",
        "Shared CT Pixel Value Transformation rescale type matches the recipe.",
        "Shared CT Pixel Value Transformation rescale type does not match the recipe.",
        nested_sequence_item_str(
            path,
            shared,
            tags::PIXEL_VALUE_TRANSFORMATION_SEQUENCE,
            0,
            tags::RESCALE_TYPE,
        )?
        .as_str(),
        expected.rescale_type,
    );
    check_equal(
        results,
        "enhanced_ct_irradiation_event_uid",
        "Shared Irradiation Event UID matches the recipe.",
        "Shared Irradiation Event UID does not match the recipe.",
        nested_sequence_item_str(
            path,
            shared,
            tags::IRRADIATION_EVENT_IDENTIFICATION_SEQUENCE,
            0,
            tags::IRRADIATION_EVENT_UID,
        )?
        .as_str(),
        expected.irradiation_event_uid,
    );

    for (index, expected_position) in expected.image_position_patient.iter().enumerate() {
        let frame =
            top_level_sequence_item(path, obj, tags::PER_FRAME_FUNCTIONAL_GROUPS_SEQUENCE, index)?;
        let expected_dimension_index_value = expected
            .dimension_index_values
            .get(index)
            .ok_or_else(|| GenerateError::MetadataShape {
                path: path.to_path_buf(),
                message: "missing expected Enhanced CT dimension index value",
            })?;
        check_equal(
            results,
            "enhanced_ct_per_frame_image_position_patient",
            "Per-frame Plane Position Image Position Patient matches the recipe.",
            "Per-frame Plane Position Image Position Patient does not match the recipe.",
            nested_sequence_item_str(
                path,
                frame,
                tags::PLANE_POSITION_SEQUENCE,
                0,
                tags::IMAGE_POSITION_PATIENT,
            )?
            .as_str(),
            *expected_position,
        );
        check_equal(
            results,
            "enhanced_ct_dimension_index_values",
            "Per-frame Dimension Index Values are one-based and monotonic.",
            "Per-frame Dimension Index Values do not match the recipe.",
            nested_sequence_item_u32(
                path,
                frame,
                tags::FRAME_CONTENT_SEQUENCE,
                0,
                tags::DIMENSION_INDEX_VALUES,
            )?,
            *expected_dimension_index_value,
        );
    }

    Ok(())
}

fn validate_segmentation(
    path: &Path,
    obj: &OpenedObject,
    results: &mut Vec<Value>,
    expected: &SegmentationExpectations<'_>,
) -> Result<(), GenerateError> {
    for (name, tag, expected_value) in [
        ("segmentation_modality", tags::MODALITY, expected.modality),
        (
            "segmentation_frame_of_reference_uid",
            tags::FRAME_OF_REFERENCE_UID,
            expected.frame_of_reference_uid,
        ),
        (
            "segmentation_image_type",
            tags::IMAGE_TYPE,
            expected.image_type,
        ),
        (
            "segmentation_type",
            TAG_SEGMENTATION_TYPE,
            expected.segmentation_type,
        ),
    ] {
        check_equal(
            results,
            name,
            "Segmentation top-level attribute matches the recipe.",
            "Segmentation top-level attribute does not match the recipe.",
            element_str(path, obj, tag)?.as_str(),
            expected_value,
        );
    }

    check_equal(
        results,
        "segmentation_segment_sequence_items",
        "Segment Sequence has the expected segment descriptions.",
        "Segment Sequence item count does not match the recipe.",
        sequence_item_count(path, obj, TAG_SEGMENT_SEQUENCE)?,
        expected.segment_sequence_items,
    );
    check_equal(
        results,
        "segmentation_segment_number",
        "Segment Number is one-based.",
        "Segment Number does not match the recipe.",
        top_level_sequence_item_u16(path, obj, TAG_SEGMENT_SEQUENCE, 0, TAG_SEGMENT_NUMBER)?,
        1,
    );
    check_equal(
        results,
        "segmentation_algorithm_type",
        "Segment Algorithm Type matches the deterministic recipe.",
        "Segment Algorithm Type does not match the recipe.",
        top_level_sequence_item_str(
            path,
            obj,
            TAG_SEGMENT_SEQUENCE,
            0,
            TAG_SEGMENT_ALGORITHM_TYPE,
        )?
        .as_str(),
        "AUTOMATIC",
    );
    if let Some(segmentation_fractional_type) = expected.segmentation_fractional_type {
        check_equal(
            results,
            "segmentation_fractional_type",
            "Segmentation Fractional Type matches the deterministic recipe.",
            "Segmentation Fractional Type does not match the recipe.",
            element_str(path, obj, TAG_SEGMENTATION_FRACTIONAL_TYPE)?.as_str(),
            segmentation_fractional_type,
        );
    }
    if let Some(maximum_fractional_value) = expected.maximum_fractional_value {
        check_equal(
            results,
            "segmentation_maximum_fractional_value",
            "Maximum Fractional Value matches the deterministic recipe.",
            "Maximum Fractional Value does not match the recipe.",
            element_u16(path, obj, TAG_MAXIMUM_FRACTIONAL_VALUE)?,
            maximum_fractional_value,
        );
    }

    check_equal(
        results,
        "segmentation_shared_functional_groups_sequence_items",
        "Shared Functional Groups Sequence has one item.",
        "Shared Functional Groups Sequence item count does not match the recipe.",
        sequence_item_count(path, obj, tags::SHARED_FUNCTIONAL_GROUPS_SEQUENCE)?,
        expected.shared_functional_groups,
    );
    check_equal(
        results,
        "segmentation_per_frame_functional_groups_sequence_items",
        "Per-Frame Functional Groups Sequence has one item per segmentation frame.",
        "Per-Frame Functional Groups Sequence item count does not match Number of Frames.",
        sequence_item_count(path, obj, tags::PER_FRAME_FUNCTIONAL_GROUPS_SEQUENCE)?,
        expected.per_frame_functional_groups,
    );
    check_equal(
        results,
        "segmentation_dimension_organization_sequence_items",
        "Dimension Organization Sequence has one item.",
        "Dimension Organization Sequence item count does not match the recipe.",
        sequence_item_count(path, obj, tags::DIMENSION_ORGANIZATION_SEQUENCE)?,
        1,
    );
    check_equal(
        results,
        "segmentation_dimension_index_sequence_items",
        "Dimension Index Sequence item count matches the recipe.",
        "Dimension Index Sequence item count does not match the recipe.",
        sequence_item_count(path, obj, tags::DIMENSION_INDEX_SEQUENCE)?,
        expected.dimension_index_count,
    );
    check_equal(
        results,
        "segmentation_dimension_organization_uid",
        "Dimension Organization UID matches between the recipe and Dimension Organization Sequence.",
        "Dimension Organization UID does not match the recipe.",
        top_level_sequence_item_str(
            path,
            obj,
            tags::DIMENSION_ORGANIZATION_SEQUENCE,
            0,
            tags::DIMENSION_ORGANIZATION_UID,
        )?
        .as_str(),
        expected.dimension_organization_uid,
    );

    let referenced_series = top_level_sequence_item(path, obj, TAG_REFERENCED_SERIES_SEQUENCE, 0)?;
    let referenced_instance =
        item_sequence_item(path, referenced_series, TAG_REFERENCED_INSTANCE_SEQUENCE, 0)?;
    check_equal(
        results,
        "segmentation_common_instance_reference_sop_class_uid",
        "Common Instance Reference SOP Class UID matches the source image.",
        "Common Instance Reference SOP Class UID does not match the source image.",
        item_str(path, referenced_instance, TAG_REFERENCED_SOP_CLASS_UID)?.as_str(),
        expected.referenced_sop_class_uid,
    );
    check_equal(
        results,
        "segmentation_common_instance_reference_sop_instance_uid",
        "Common Instance Reference SOP Instance UID matches the source image.",
        "Common Instance Reference SOP Instance UID does not match the source image.",
        item_str(path, referenced_instance, TAG_REFERENCED_SOP_INSTANCE_UID)?.as_str(),
        expected.referenced_sop_instance_uid,
    );

    for (index, expected_frame_number) in expected.referenced_frame_numbers.iter().enumerate() {
        let frame =
            top_level_sequence_item(path, obj, tags::PER_FRAME_FUNCTIONAL_GROUPS_SEQUENCE, index)?;
        check_equal(
            results,
            "segmentation_referenced_segment_number",
            "Per-frame Segment Identification references segment 1.",
            "Per-frame Segment Identification does not reference segment 1.",
            nested_sequence_item_u16(
                path,
                frame,
                TAG_SEGMENT_IDENTIFICATION_SEQUENCE,
                0,
                TAG_REFERENCED_SEGMENT_NUMBER,
            )?,
            1,
        );
        let derivation = item_sequence_item(path, frame, TAG_DERIVATION_IMAGE_SEQUENCE, 0)?;
        let source = item_sequence_item(path, derivation, TAG_SOURCE_IMAGE_SEQUENCE, 0)?;
        check_equal(
            results,
            "segmentation_source_image_sop_class_uid",
            "Derivation Image source SOP Class UID matches the source image.",
            "Derivation Image source SOP Class UID does not match the source image.",
            item_str(path, source, TAG_REFERENCED_SOP_CLASS_UID)?.as_str(),
            expected.referenced_sop_class_uid,
        );
        check_equal(
            results,
            "segmentation_source_image_sop_instance_uid",
            "Derivation Image source SOP Instance UID matches the source image.",
            "Derivation Image source SOP Instance UID does not match the source image.",
            item_str(path, source, TAG_REFERENCED_SOP_INSTANCE_UID)?.as_str(),
            expected.referenced_sop_instance_uid,
        );
        check_equal(
            results,
            "segmentation_source_image_frame_number",
            "Derivation Image source frame number matches the segmentation frame.",
            "Derivation Image source frame number does not match the recipe.",
            item_u16(path, source, TAG_REFERENCED_FRAME_NUMBER)?,
            *expected_frame_number,
        );
    }

    Ok(())
}

fn validate_enhanced_mr_image(
    path: &Path,
    obj: &OpenedObject,
    results: &mut Vec<Value>,
    expected: &EnhancedMrImageExpectations<'_>,
) -> Result<(), GenerateError> {
    for (name, tag, expected_value) in [
        ("enhanced_mr_modality", tags::MODALITY, expected.modality),
        (
            "enhanced_mr_patient_position",
            tags::PATIENT_POSITION,
            expected.patient_position,
        ),
        (
            "enhanced_mr_frame_of_reference_uid",
            tags::FRAME_OF_REFERENCE_UID,
            expected.frame_of_reference_uid,
        ),
        (
            "enhanced_mr_image_type",
            tags::IMAGE_TYPE,
            expected.image_type,
        ),
        (
            "enhanced_mr_pixel_presentation",
            tags::PIXEL_PRESENTATION,
            expected.pixel_presentation,
        ),
        (
            "enhanced_mr_volumetric_properties",
            tags::VOLUMETRIC_PROPERTIES,
            expected.volumetric_properties,
        ),
        (
            "enhanced_mr_volume_based_calculation_technique",
            tags::VOLUME_BASED_CALCULATION_TECHNIQUE,
            expected.volume_based_calculation_technique,
        ),
        (
            "enhanced_mr_content_qualification",
            tags::CONTENT_QUALIFICATION,
            expected.content_qualification,
        ),
        (
            "enhanced_mr_applicable_safety_standard_agency",
            tags::APPLICABLE_SAFETY_STANDARD_AGENCY,
            expected.applicable_safety_standard_agency,
        ),
        (
            "enhanced_mr_complex_image_component",
            tags::COMPLEX_IMAGE_COMPONENT,
            expected.complex_image_component,
        ),
        (
            "enhanced_mr_acquisition_contrast",
            tags::ACQUISITION_CONTRAST,
            expected.acquisition_contrast,
        ),
        (
            "enhanced_mr_burned_in_annotation",
            tags::BURNED_IN_ANNOTATION,
            expected.burned_in_annotation,
        ),
        (
            "enhanced_mr_lossy_image_compression",
            tags::LOSSY_IMAGE_COMPRESSION,
            expected.lossy_image_compression,
        ),
        (
            "enhanced_mr_presentation_lut_shape",
            tags::PRESENTATION_LUT_SHAPE,
            expected.presentation_lut_shape,
        ),
    ] {
        check_equal(
            results,
            name,
            "Enhanced MR top-level attribute matches the recipe.",
            "Enhanced MR top-level attribute does not match the recipe.",
            element_str(path, obj, tag)?.as_str(),
            expected_value,
        );
    }

    let expected_number_of_frames = expected.number_of_frames.to_string();
    check_equal(
        results,
        "enhanced_mr_number_of_frames",
        "Number of Frames matches the recipe.",
        "Number of Frames does not match the recipe.",
        element_str(path, obj, tags::NUMBER_OF_FRAMES)?.as_str(),
        expected_number_of_frames.as_str(),
    );
    check_equal(
        results,
        "enhanced_mr_shared_functional_groups_sequence_items",
        "Shared Functional Groups Sequence has one item.",
        "Shared Functional Groups Sequence item count does not match the recipe.",
        sequence_item_count(path, obj, tags::SHARED_FUNCTIONAL_GROUPS_SEQUENCE)?,
        expected.shared_functional_groups,
    );
    check_equal(
        results,
        "enhanced_mr_per_frame_functional_groups_sequence_items",
        "Per-Frame Functional Groups Sequence has one item per frame.",
        "Per-Frame Functional Groups Sequence item count does not match Number of Frames.",
        sequence_item_count(path, obj, tags::PER_FRAME_FUNCTIONAL_GROUPS_SEQUENCE)?,
        expected.per_frame_functional_groups,
    );
    check_equal(
        results,
        "enhanced_mr_dimension_organization_sequence_items",
        "Dimension Organization Sequence has one item.",
        "Dimension Organization Sequence item count does not match the recipe.",
        sequence_item_count(path, obj, tags::DIMENSION_ORGANIZATION_SEQUENCE)?,
        1,
    );
    check_equal(
        results,
        "enhanced_mr_dimension_index_sequence_items",
        "Dimension Index Sequence item count matches the recipe.",
        "Dimension Index Sequence item count does not match the recipe.",
        sequence_item_count(path, obj, tags::DIMENSION_INDEX_SEQUENCE)?,
        expected.dimension_index_count,
    );
    check_equal(
        results,
        "enhanced_mr_dimension_organization_uid",
        "Dimension Organization UID matches between the recipe and Dimension Organization Sequence.",
        "Dimension Organization UID does not match the recipe.",
        top_level_sequence_item_str(
            path,
            obj,
            tags::DIMENSION_ORGANIZATION_SEQUENCE,
            0,
            tags::DIMENSION_ORGANIZATION_UID,
        )?
        .as_str(),
        expected.dimension_organization_uid,
    );

    let shared = top_level_sequence_item(path, obj, tags::SHARED_FUNCTIONAL_GROUPS_SEQUENCE, 0)?;
    check_equal(
        results,
        "enhanced_mr_pixel_measures_sequence_items",
        "Pixel Measures Sequence has one shared item.",
        "Pixel Measures Sequence item count does not match the recipe.",
        item_sequence_item_count(path, shared, tags::PIXEL_MEASURES_SEQUENCE)?,
        1,
    );
    check_equal(
        results,
        "enhanced_mr_pixel_spacing",
        "Shared Pixel Measures Pixel Spacing matches the recipe.",
        "Shared Pixel Measures Pixel Spacing does not match the recipe.",
        nested_sequence_item_str(
            path,
            shared,
            tags::PIXEL_MEASURES_SEQUENCE,
            0,
            tags::PIXEL_SPACING,
        )?
        .as_str(),
        expected.pixel_spacing,
    );
    check_equal(
        results,
        "enhanced_mr_image_orientation_patient",
        "Shared Plane Orientation Image Orientation Patient matches the recipe.",
        "Shared Plane Orientation Image Orientation Patient does not match the recipe.",
        nested_sequence_item_str(
            path,
            shared,
            tags::PLANE_ORIENTATION_SEQUENCE,
            0,
            tags::IMAGE_ORIENTATION_PATIENT,
        )?
        .as_str(),
        expected.image_orientation_patient,
    );
    let frame_anatomy = item_sequence_item(path, shared, tags::FRAME_ANATOMY_SEQUENCE, 0)?;
    let anatomic_region =
        item_sequence_item(path, frame_anatomy, tags::ANATOMIC_REGION_SEQUENCE, 0)?;
    for (name, tag, expected_value) in [
        (
            "enhanced_mr_anatomic_region_code_value",
            tags::CODE_VALUE,
            expected.anatomic_region_code_value,
        ),
        (
            "enhanced_mr_anatomic_region_coding_scheme",
            tags::CODING_SCHEME_DESIGNATOR,
            expected.anatomic_region_coding_scheme,
        ),
        (
            "enhanced_mr_anatomic_region_code_meaning",
            tags::CODE_MEANING,
            expected.anatomic_region_code_meaning,
        ),
    ] {
        check_equal(
            results,
            name,
            "Enhanced MR Frame Anatomy code matches the recipe.",
            "Enhanced MR Frame Anatomy code does not match the recipe.",
            item_str(path, anatomic_region, tag)?.as_str(),
            expected_value,
        );
    }
    check_equal(
        results,
        "enhanced_mr_frame_type",
        "Shared MR Image Frame Type matches the recipe.",
        "Shared MR Image Frame Type does not match the recipe.",
        nested_sequence_item_str(
            path,
            shared,
            tags::MR_IMAGE_FRAME_TYPE_SEQUENCE,
            0,
            tags::FRAME_TYPE,
        )?
        .as_str(),
        expected.frame_type,
    );
    for (name, tag, expected_value) in [
        (
            "enhanced_mr_frame_complex_image_component",
            tags::COMPLEX_IMAGE_COMPONENT,
            expected.complex_image_component,
        ),
        (
            "enhanced_mr_frame_acquisition_contrast",
            tags::ACQUISITION_CONTRAST,
            expected.acquisition_contrast,
        ),
    ] {
        check_equal(
            results,
            name,
            "Enhanced MR frame-level image description matches the recipe.",
            "Enhanced MR frame-level image description does not match the recipe.",
            nested_sequence_item_str(path, shared, tags::MR_IMAGE_FRAME_TYPE_SEQUENCE, 0, tag)?
                .as_str(),
            expected_value,
        );
    }
    check_equal(
        results,
        "enhanced_mr_rescale_intercept",
        "Shared Pixel Value Transformation rescale intercept matches the recipe.",
        "Shared Pixel Value Transformation rescale intercept does not match the recipe.",
        nested_sequence_item_str(
            path,
            shared,
            tags::PIXEL_VALUE_TRANSFORMATION_SEQUENCE,
            0,
            tags::RESCALE_INTERCEPT,
        )?
        .as_str(),
        expected.rescale_intercept,
    );
    check_equal(
        results,
        "enhanced_mr_rescale_slope",
        "Shared Pixel Value Transformation rescale slope matches the recipe.",
        "Shared Pixel Value Transformation rescale slope does not match the recipe.",
        nested_sequence_item_str(
            path,
            shared,
            tags::PIXEL_VALUE_TRANSFORMATION_SEQUENCE,
            0,
            tags::RESCALE_SLOPE,
        )?
        .as_str(),
        expected.rescale_slope,
    );
    check_equal(
        results,
        "enhanced_mr_rescale_type",
        "Shared Pixel Value Transformation rescale type matches the recipe.",
        "Shared Pixel Value Transformation rescale type does not match the recipe.",
        nested_sequence_item_str(
            path,
            shared,
            tags::PIXEL_VALUE_TRANSFORMATION_SEQUENCE,
            0,
            tags::RESCALE_TYPE,
        )?
        .as_str(),
        expected.rescale_type,
    );
    check_equal(
        results,
        "enhanced_mr_repetition_time",
        "Shared MR Timing Repetition Time matches the recipe.",
        "Shared MR Timing Repetition Time does not match the recipe.",
        nested_sequence_item_str(
            path,
            shared,
            tags::MR_TIMING_AND_RELATED_PARAMETERS_SEQUENCE,
            0,
            tags::REPETITION_TIME,
        )?
        .as_str(),
        expected.repetition_time,
    );
    check_equal(
        results,
        "enhanced_mr_flip_angle",
        "Shared MR Timing Flip Angle matches the recipe.",
        "Shared MR Timing Flip Angle does not match the recipe.",
        nested_sequence_item_str(
            path,
            shared,
            tags::MR_TIMING_AND_RELATED_PARAMETERS_SEQUENCE,
            0,
            tags::FLIP_ANGLE,
        )?
        .as_str(),
        expected.flip_angle,
    );
    check_equal(
        results,
        "enhanced_mr_echo_train_length",
        "Shared MR Timing Echo Train Length matches the recipe.",
        "Shared MR Timing Echo Train Length does not match the recipe.",
        nested_sequence_item_str(
            path,
            shared,
            tags::MR_TIMING_AND_RELATED_PARAMETERS_SEQUENCE,
            0,
            tags::ECHO_TRAIN_LENGTH,
        )?
        .as_str(),
        expected.echo_train_length,
    );
    check_equal(
        results,
        "enhanced_mr_rf_echo_train_length",
        "Shared MR Timing RF Echo Train Length matches the recipe.",
        "Shared MR Timing RF Echo Train Length does not match the recipe.",
        nested_sequence_item_u16(
            path,
            shared,
            tags::MR_TIMING_AND_RELATED_PARAMETERS_SEQUENCE,
            0,
            tags::RF_ECHO_TRAIN_LENGTH,
        )?,
        expected.rf_echo_train_length,
    );
    check_equal(
        results,
        "enhanced_mr_gradient_echo_train_length",
        "Shared MR Timing Gradient Echo Train Length matches the recipe.",
        "Shared MR Timing Gradient Echo Train Length does not match the recipe.",
        nested_sequence_item_u16(
            path,
            shared,
            tags::MR_TIMING_AND_RELATED_PARAMETERS_SEQUENCE,
            0,
            tags::GRADIENT_ECHO_TRAIN_LENGTH,
        )?,
        expected.gradient_echo_train_length,
    );
    let timing = item_sequence_item(
        path,
        shared,
        tags::MR_TIMING_AND_RELATED_PARAMETERS_SEQUENCE,
        0,
    )?;
    check_equal(
        results,
        "enhanced_mr_specific_absorption_rate_sequence_items",
        "Specific Absorption Rate Sequence has one item.",
        "Specific Absorption Rate Sequence item count does not match the recipe.",
        item_sequence_item_count(path, timing, tags::SPECIFIC_ABSORPTION_RATE_SEQUENCE)?,
        1,
    );
    let sar = item_sequence_item(path, timing, tags::SPECIFIC_ABSORPTION_RATE_SEQUENCE, 0)?;
    check_equal(
        results,
        "enhanced_mr_specific_absorption_rate_definition",
        "Specific Absorption Rate Definition matches the recipe.",
        "Specific Absorption Rate Definition does not match the recipe.",
        item_str(path, sar, tags::SPECIFIC_ABSORPTION_RATE_DEFINITION)?.as_str(),
        expected.specific_absorption_rate_definition,
    );
    check_equal(
        results,
        "enhanced_mr_specific_absorption_rate_value",
        "Specific Absorption Rate Value matches the recipe.",
        "Specific Absorption Rate Value does not match the recipe.",
        item_f64(path, sar, tags::SPECIFIC_ABSORPTION_RATE_VALUE)?,
        expected.specific_absorption_rate_value,
    );
    check_equal(
        results,
        "enhanced_mr_operating_mode_sequence_items",
        "Operating Mode Sequence item count matches the recipe.",
        "Operating Mode Sequence item count does not match the recipe.",
        item_sequence_item_count(path, timing, tags::OPERATING_MODE_SEQUENCE)?,
        expected.operating_modes.len(),
    );
    for (index, (expected_type, expected_mode)) in expected.operating_modes.iter().enumerate() {
        let operating_mode =
            item_sequence_item(path, timing, tags::OPERATING_MODE_SEQUENCE, index)?;
        check_equal(
            results,
            "enhanced_mr_operating_mode_type",
            "Operating Mode Type matches the recipe.",
            "Operating Mode Type does not match the recipe.",
            item_str(path, operating_mode, tags::OPERATING_MODE_TYPE)?.as_str(),
            *expected_type,
        );
        check_equal(
            results,
            "enhanced_mr_operating_mode",
            "Operating Mode matches the recipe.",
            "Operating Mode does not match the recipe.",
            item_str(path, operating_mode, tags::OPERATING_MODE)?.as_str(),
            *expected_mode,
        );
    }

    for (index, expected_position) in expected.image_position_patient.iter().enumerate() {
        let frame =
            top_level_sequence_item(path, obj, tags::PER_FRAME_FUNCTIONAL_GROUPS_SEQUENCE, index)?;
        check_equal(
            results,
            "enhanced_mr_per_frame_image_position_patient",
            "Per-frame Plane Position Image Position Patient matches the recipe.",
            "Per-frame Plane Position Image Position Patient does not match the recipe.",
            nested_sequence_item_str(
                path,
                frame,
                tags::PLANE_POSITION_SEQUENCE,
                0,
                tags::IMAGE_POSITION_PATIENT,
            )?
            .as_str(),
            *expected_position,
        );
        if let Some(effective_echo_times) = expected.effective_echo_times {
            check_equal(
                results,
                "enhanced_mr_per_frame_effective_echo_time",
                "Per-frame MR Echo Effective Echo Time matches the recipe.",
                "Per-frame MR Echo Effective Echo Time does not match the recipe.",
                nested_sequence_item_f64(
                    path,
                    frame,
                    tags::MR_ECHO_SEQUENCE,
                    0,
                    tags::EFFECTIVE_ECHO_TIME,
                )?,
                effective_echo_times[index],
            );
        }
        if let Some(temporal_position_time_offsets) = expected.temporal_position_time_offsets {
            check_equal(
                results,
                "enhanced_mr_temporal_position_index",
                "Per-frame Temporal Position Index is one-based and monotonic.",
                "Per-frame Temporal Position Index does not match the recipe.",
                nested_sequence_item_u32(
                    path,
                    frame,
                    tags::FRAME_CONTENT_SEQUENCE,
                    0,
                    tags::TEMPORAL_POSITION_INDEX,
                )?,
                (index + 1) as u32,
            );
            check_equal(
                results,
                "enhanced_mr_temporal_position_time_offset",
                "Per-frame Temporal Position Time Offset matches the recipe.",
                "Per-frame Temporal Position Time Offset does not match the recipe.",
                nested_sequence_item_f64(
                    path,
                    frame,
                    tags::TEMPORAL_POSITION_SEQUENCE,
                    0,
                    tags::TEMPORAL_POSITION_TIME_OFFSET,
                )?,
                temporal_position_time_offsets[index],
            );
        }
        if let Some(velocity_encoding_directions) = expected.velocity_encoding_directions {
            check_equal(
                results,
                "enhanced_mr_velocity_encoding_direction",
                "Per-frame MR Velocity Encoding Direction matches the recipe.",
                "Per-frame MR Velocity Encoding Direction does not match the recipe.",
                nested_sequence_item_f64_values(
                    path,
                    frame,
                    tags::MR_VELOCITY_ENCODING_SEQUENCE,
                    0,
                    tags::VELOCITY_ENCODING_DIRECTION,
                )?,
                velocity_encoding_directions[index].to_vec(),
            );
            check_equal(
                results,
                "enhanced_mr_velocity_encoding_minimum_value",
                "Per-frame MR Velocity Encoding Minimum Value matches the recipe.",
                "Per-frame MR Velocity Encoding Minimum Value does not match the recipe.",
                nested_sequence_item_f64(
                    path,
                    frame,
                    tags::MR_VELOCITY_ENCODING_SEQUENCE,
                    0,
                    tags::VELOCITY_ENCODING_MINIMUM_VALUE,
                )?,
                expected
                    .velocity_encoding_minimum_value
                    .expect("velocity encoding expectations must define a minimum value"),
            );
            check_equal(
                results,
                "enhanced_mr_velocity_encoding_maximum_value",
                "Per-frame MR Velocity Encoding Maximum Value matches the recipe.",
                "Per-frame MR Velocity Encoding Maximum Value does not match the recipe.",
                nested_sequence_item_f64(
                    path,
                    frame,
                    tags::MR_VELOCITY_ENCODING_SEQUENCE,
                    0,
                    tags::VELOCITY_ENCODING_MAXIMUM_VALUE,
                )?,
                expected
                    .velocity_encoding_maximum_value
                    .expect("velocity encoding expectations must define a maximum value"),
            );
        }
        check_equal(
            results,
            "enhanced_mr_dimension_index_values",
            "Per-frame Dimension Index Values are one-based and monotonic.",
            "Per-frame Dimension Index Values do not match the recipe.",
            nested_sequence_item_u32(
                path,
                frame,
                tags::FRAME_CONTENT_SEQUENCE,
                0,
                tags::DIMENSION_INDEX_VALUES,
            )?,
            (index + 1) as u32,
        );
    }

    Ok(())
}

fn validate_enhanced_pet_image(
    path: &Path,
    obj: &OpenedObject,
    results: &mut Vec<Value>,
    expected: &EnhancedPetImageExpectations<'_>,
) -> Result<(), GenerateError> {
    for (name, tag, value) in [
        ("enhanced_pet_modality", tags::MODALITY, expected.modality),
        (
            "enhanced_pet_frame_of_reference_uid",
            tags::FRAME_OF_REFERENCE_UID,
            expected.frame_of_reference_uid,
        ),
        (
            "enhanced_pet_image_type",
            tags::IMAGE_TYPE,
            expected.image_type,
        ),
        ("enhanced_pet_table_motion", tags::TABLE_MOTION, "STATIC"),
        (
            "enhanced_pet_time_of_flight_information_used",
            tags::TIME_OF_FLIGHT_INFORMATION_USED,
            "FALSE",
        ),
        (
            "enhanced_pet_counts_source",
            tags::COUNTS_SOURCE,
            "EMISSION",
        ),
        (
            "enhanced_pet_pixel_presentation",
            tags::PIXEL_PRESENTATION,
            "MONOCHROME",
        ),
        (
            "enhanced_pet_volumetric_properties",
            tags::VOLUMETRIC_PROPERTIES,
            "VOLUME",
        ),
        (
            "enhanced_pet_volume_calculation",
            tags::VOLUME_BASED_CALCULATION_TECHNIQUE,
            "NONE",
        ),
        (
            "enhanced_pet_content_qualification",
            tags::CONTENT_QUALIFICATION,
            "RESEARCH",
        ),
        (
            "enhanced_pet_burned_in_annotation",
            tags::BURNED_IN_ANNOTATION,
            "NO",
        ),
        (
            "enhanced_pet_lossy_image_compression",
            tags::LOSSY_IMAGE_COMPRESSION,
            "00",
        ),
        (
            "enhanced_pet_presentation_lut_shape",
            tags::PRESENTATION_LUT_SHAPE,
            "IDENTITY",
        ),
    ] {
        check_equal(
            results,
            name,
            "Enhanced PET attribute matches the recipe.",
            "Enhanced PET attribute does not match the recipe.",
            element_str(path, obj, tag)?.as_str(),
            value,
        );
    }
    for (name, tag) in [
        ("enhanced_pet_decay_corrected", tags::DECAY_CORRECTED),
        (
            "enhanced_pet_attenuation_corrected",
            tags::ATTENUATION_CORRECTED,
        ),
        ("enhanced_pet_scatter_corrected", tags::SCATTER_CORRECTED),
        (
            "enhanced_pet_dead_time_corrected",
            tags::DEAD_TIME_CORRECTED,
        ),
        (
            "enhanced_pet_gantry_motion_corrected",
            tags::GANTRY_MOTION_CORRECTED,
        ),
        (
            "enhanced_pet_patient_motion_corrected",
            tags::PATIENT_MOTION_CORRECTED,
        ),
        (
            "enhanced_pet_count_loss_normalization_corrected",
            tags::COUNT_LOSS_NORMALIZATION_CORRECTED,
        ),
        ("enhanced_pet_randoms_corrected", tags::RANDOMS_CORRECTED),
        (
            "enhanced_pet_non_uniform_radial_sampling_corrected",
            tags::NON_UNIFORM_RADIAL_SAMPLING_CORRECTED,
        ),
        (
            "enhanced_pet_sensitivity_calibrated",
            tags::SENSITIVITY_CALIBRATED,
        ),
        (
            "enhanced_pet_detector_normalization_correction",
            tags::DETECTOR_NORMALIZATION_CORRECTION,
        ),
    ] {
        check_equal(
            results,
            name,
            "Enhanced PET correction flag is NO.",
            "Enhanced PET correction flag is not NO.",
            element_str(path, obj, tag)?.as_str(),
            "NO",
        );
    }
    let view = top_level_sequence_item(path, obj, tags::VIEW_CODE_SEQUENCE, 0)?;
    for (name, tag, value) in [
        ("enhanced_pet_view_code_value", tags::CODE_VALUE, "24422004"),
        (
            "enhanced_pet_view_coding_scheme",
            tags::CODING_SCHEME_DESIGNATOR,
            "SCT",
        ),
        (
            "enhanced_pet_view_code_meaning",
            tags::CODE_MEANING,
            "Axial",
        ),
    ] {
        check_equal(
            results,
            name,
            "Enhanced PET axial view code matches the locked contract.",
            "Enhanced PET axial view code does not match the locked contract.",
            item_str(path, view, tag)?.as_str(),
            value,
        );
    }
    check(
        results,
        view.element_opt(tags::VIEW_MODIFIER_CODE_SEQUENCE)
            .map_err(|err| validation_error(path, err))?
            .is_none(),
        "enhanced_pet_view_modifier_absent",
        "Plain axial view omits View Modifier Code Sequence.",
        "Plain axial view unexpectedly contains View Modifier Code Sequence.",
    );
    check(
        results,
        obj.element_opt(tags::SLICE_PROGRESSION_DIRECTION)
            .map_err(|err| validation_error(path, err))?
            .is_none(),
        "enhanced_pet_slice_progression_direction_absent",
        "Non-cardiac axial view omits Slice Progression Direction.",
        "Non-cardiac axial view unexpectedly contains Slice Progression Direction.",
    );
    for (name, tag) in [
        (
            "enhanced_pet_attenuation_method_absent",
            tags::ATTENUATION_CORRECTION_METHOD,
        ),
        (
            "enhanced_pet_scatter_method_absent",
            tags::SCATTER_CORRECTION_METHOD,
        ),
        (
            "enhanced_pet_randoms_method_absent",
            tags::RANDOMS_CORRECTION_METHOD,
        ),
        (
            "enhanced_pet_attenuation_source_absent",
            tags::ATTENUATION_CORRECTION_SOURCE,
        ),
        (
            "enhanced_pet_decay_datetime_absent",
            tags::DECAY_CORRECTION_DATE_TIME,
        ),
        (
            "enhanced_pet_attenuation_relationship_absent",
            tags::ATTENUATION_CORRECTION_TEMPORAL_RELATIONSHIP,
        ),
    ] {
        check(
            results,
            obj.element_opt(tag)
                .map_err(|err| validation_error(path, err))?
                .is_none(),
            name,
            "Unclaimed Enhanced PET correction detail is absent.",
            "Enhanced PET contains an unclaimed correction detail.",
        );
    }
    check_equal(
        results,
        "enhanced_pet_number_of_frames",
        "Number of Frames matches.",
        "Number of Frames does not match.",
        element_str(path, obj, tags::NUMBER_OF_FRAMES)?.as_str(),
        expected.number_of_frames.to_string().as_str(),
    );
    for (name, tag, count) in [
        (
            "enhanced_pet_shared_functional_groups",
            tags::SHARED_FUNCTIONAL_GROUPS_SEQUENCE,
            1,
        ),
        (
            "enhanced_pet_per_frame_functional_groups",
            tags::PER_FRAME_FUNCTIONAL_GROUPS_SEQUENCE,
            expected.number_of_frames as usize,
        ),
        (
            "enhanced_pet_dimension_organization",
            tags::DIMENSION_ORGANIZATION_SEQUENCE,
            1,
        ),
        (
            "enhanced_pet_dimension_index",
            tags::DIMENSION_INDEX_SEQUENCE,
            1,
        ),
        (
            "enhanced_pet_radiopharmaceutical_information",
            tags::RADIOPHARMACEUTICAL_INFORMATION_SEQUENCE,
            1,
        ),
        (
            "enhanced_pet_view_code_sequence",
            tags::VIEW_CODE_SEQUENCE,
            1,
        ),
    ] {
        check_equal(
            results,
            name,
            "Enhanced PET sequence item count matches.",
            "Enhanced PET sequence item count does not match.",
            sequence_item_count(path, obj, tag)?,
            count,
        );
    }
    check_equal(
        results,
        "enhanced_pet_dimension_organization_uid",
        "Dimension UID matches.",
        "Dimension UID does not match.",
        top_level_sequence_item_str(
            path,
            obj,
            tags::DIMENSION_ORGANIZATION_SEQUENCE,
            0,
            tags::DIMENSION_ORGANIZATION_UID,
        )?
        .as_str(),
        expected.dimension_organization_uid,
    );
    let isotope =
        top_level_sequence_item(path, obj, tags::RADIOPHARMACEUTICAL_INFORMATION_SEQUENCE, 0)?;
    check_equal(
        results,
        "enhanced_pet_radiopharmaceutical_agent",
        "Agent Number is 1.",
        "Agent Number is not 1.",
        item_u16(path, isotope, tags::RADIOPHARMACEUTICAL_AGENT_NUMBER)?,
        1,
    );
    for (name, tag, value) in [
        (
            "enhanced_pet_radiopharmaceutical_start",
            tags::RADIOPHARMACEUTICAL_START_DATE_TIME,
            "20260101000000",
        ),
        (
            "enhanced_pet_half_life",
            tags::RADIONUCLIDE_HALF_LIFE,
            "6586.2",
        ),
        (
            "enhanced_pet_positron_fraction",
            tags::RADIONUCLIDE_POSITRON_FRACTION,
            "0.967",
        ),
    ] {
        check_equal(
            results,
            name,
            "Isotope value matches.",
            "Isotope value does not match.",
            item_str(path, isotope, tag)?.as_str(),
            value,
        );
    }
    let total_dose = isotope
        .element(tags::RADIONUCLIDE_TOTAL_DOSE)
        .map_err(|err| validation_error(path, err))?;
    check_equal(
        results,
        "enhanced_pet_total_dose_vr",
        "Radionuclide Total Dose uses DS VR.",
        "Radionuclide Total Dose does not use DS VR.",
        total_dose.vr(),
        VR::DS,
    );
    check_equal(
        results,
        "enhanced_pet_total_dose_present_empty",
        "Unknown total dose is present with an empty value as required by Type 2.",
        "Unknown total dose is absent or contains a claimed numeric value.",
        item_str(path, isotope, tags::RADIONUCLIDE_TOTAL_DOSE)?.as_str(),
        "",
    );
    for (name, sequence, code) in [
        (
            "enhanced_pet_radionuclide_code",
            tags::RADIONUCLIDE_CODE_SEQUENCE,
            "77004003",
        ),
        (
            "enhanced_pet_route_code",
            tags::ADMINISTRATION_ROUTE_CODE_SEQUENCE,
            "47625008",
        ),
        (
            "enhanced_pet_radiopharmaceutical_code",
            tags::RADIOPHARMACEUTICAL_CODE_SEQUENCE,
            "35321007",
        ),
    ] {
        check_equal(
            results,
            name,
            "Isotope code matches.",
            "Isotope code does not match.",
            nested_sequence_item_str(path, isotope, sequence, 0, tags::CODE_VALUE)?.as_str(),
            code,
        );
    }

    let shared = top_level_sequence_item(path, obj, tags::SHARED_FUNCTIONAL_GROUPS_SEQUENCE, 0)?;
    for (name, tag, count) in [
        (
            "enhanced_pet_pixel_measures_sequence",
            tags::PIXEL_MEASURES_SEQUENCE,
            1,
        ),
        (
            "enhanced_pet_plane_orientation_sequence",
            tags::PLANE_ORIENTATION_SEQUENCE,
            1,
        ),
        (
            "enhanced_pet_frame_anatomy_sequence",
            tags::FRAME_ANATOMY_SEQUENCE,
            1,
        ),
        (
            "enhanced_pet_pixel_value_transformation_sequence",
            tags::PIXEL_VALUE_TRANSFORMATION_SEQUENCE,
            1,
        ),
        (
            "enhanced_pet_frame_voi_sequence",
            tags::FRAME_VOILUT_SEQUENCE,
            1,
        ),
        (
            "enhanced_pet_rwvm_sequence",
            tags::REAL_WORLD_VALUE_MAPPING_SEQUENCE,
            1,
        ),
        (
            "enhanced_pet_radiopharmaceutical_usage_sequence",
            tags::RADIOPHARMACEUTICAL_USAGE_SEQUENCE,
            1,
        ),
        (
            "enhanced_pet_frame_type_sequence",
            tags::PET_FRAME_TYPE_SEQUENCE,
            1,
        ),
        (
            "enhanced_pet_derivation_image_sequence_empty",
            tags::DERIVATION_IMAGE_SEQUENCE,
            0,
        ),
    ] {
        check_equal(
            results,
            name,
            "Shared macro item count matches.",
            "Shared macro item count does not match.",
            item_sequence_item_count(path, shared, tag)?,
            count,
        );
    }
    check_equal(
        results,
        "enhanced_pet_pixel_spacing",
        "Pixel Spacing matches.",
        "Pixel Spacing does not match.",
        nested_sequence_item_str(
            path,
            shared,
            tags::PIXEL_MEASURES_SEQUENCE,
            0,
            tags::PIXEL_SPACING,
        )?
        .as_str(),
        expected.pixel_spacing,
    );
    check_equal(
        results,
        "enhanced_pet_orientation",
        "Orientation matches.",
        "Orientation does not match.",
        nested_sequence_item_str(
            path,
            shared,
            tags::PLANE_ORIENTATION_SEQUENCE,
            0,
            tags::IMAGE_ORIENTATION_PATIENT,
        )?
        .as_str(),
        expected.image_orientation_patient,
    );
    check_equal(
        results,
        "enhanced_pet_frame_type",
        "Frame Type matches.",
        "Frame Type does not match.",
        nested_sequence_item_str(
            path,
            shared,
            tags::PET_FRAME_TYPE_SEQUENCE,
            0,
            tags::FRAME_TYPE,
        )?
        .as_str(),
        expected.frame_type,
    );
    check_equal(
        results,
        "enhanced_pet_rescale_intercept",
        "Rescale Intercept matches.",
        "Rescale Intercept does not match.",
        nested_sequence_item_str(
            path,
            shared,
            tags::PIXEL_VALUE_TRANSFORMATION_SEQUENCE,
            0,
            tags::RESCALE_INTERCEPT,
        )?
        .as_str(),
        expected.rescale_intercept,
    );
    check_equal(
        results,
        "enhanced_pet_rescale_slope",
        "Rescale Slope matches.",
        "Rescale Slope does not match.",
        nested_sequence_item_str(
            path,
            shared,
            tags::PIXEL_VALUE_TRANSFORMATION_SEQUENCE,
            0,
            tags::RESCALE_SLOPE,
        )?
        .as_str(),
        expected.rescale_slope,
    );
    let rwvm = item_sequence_item(path, shared, tags::REAL_WORLD_VALUE_MAPPING_SEQUENCE, 0)?;
    check_equal(
        results,
        "enhanced_pet_rwvm_first",
        "RWVM first value is 0.",
        "RWVM first value differs.",
        item_u16(path, rwvm, tags::REAL_WORLD_VALUE_FIRST_VALUE_MAPPED)?,
        0,
    );
    check_equal(
        results,
        "enhanced_pet_rwvm_last",
        "RWVM last value is 400.",
        "RWVM last value differs.",
        item_u16(path, rwvm, tags::REAL_WORLD_VALUE_LAST_VALUE_MAPPED)?,
        400,
    );
    let rwvm_intercept = item_f64(path, rwvm, tags::REAL_WORLD_VALUE_INTERCEPT)?;
    let rwvm_slope = item_f64(path, rwvm, tags::REAL_WORLD_VALUE_SLOPE)?;
    check_equal(
        results,
        "enhanced_pet_rwvm_intercept",
        "RWVM intercept is 0.",
        "RWVM intercept differs.",
        rwvm_intercept,
        0.0,
    );
    check_equal(
        results,
        "enhanced_pet_rwvm_slope",
        "RWVM slope is 2.5.",
        "RWVM slope differs.",
        rwvm_slope,
        2.5,
    );
    check_equal(
        results,
        "enhanced_pet_rwvm_lut_label",
        "RWVM LUT Label is BQML.",
        "RWVM LUT Label differs.",
        item_str(path, rwvm, tags::LUT_LABEL)?.as_str(),
        "BQML",
    );
    check_equal(
        results,
        "enhanced_pet_rwvm_unit",
        "RWVM unit is Bq/ml UCUM.",
        "RWVM unit differs.",
        nested_sequence_item_str(
            path,
            rwvm,
            tags::MEASUREMENT_UNITS_CODE_SEQUENCE,
            0,
            tags::CODE_VALUE,
        )?
        .as_str(),
        "Bq/ml",
    );
    let mapped = expected
        .stored_values
        .iter()
        .map(|value| f64::from(*value) * rwvm_slope + rwvm_intercept)
        .collect::<Vec<_>>();
    check_equal(
        results,
        "enhanced_pet_rwvm_arithmetic",
        "RWVM arithmetic independently matches expected BQML values.",
        "RWVM arithmetic does not match expected BQML values.",
        mapped.as_slice(),
        expected.activity_values_bqml,
    );

    for frame in 0..expected.number_of_frames as usize {
        let item =
            top_level_sequence_item(path, obj, tags::PER_FRAME_FUNCTIONAL_GROUPS_SEQUENCE, frame)?;
        check_equal(
            results,
            "enhanced_pet_per_frame_content_count",
            "Frame Content has one item.",
            "Frame Content count differs.",
            item_sequence_item_count(path, item, tags::FRAME_CONTENT_SEQUENCE)?,
            1,
        );
        let content = item_sequence_item(path, item, tags::FRAME_CONTENT_SEQUENCE, 0)?;
        check_equal(
            results,
            "enhanced_pet_stack_id",
            "Stack ID matches.",
            "Stack ID differs.",
            item_str(path, content, tags::STACK_ID)?.as_str(),
            expected.stack_id,
        );
        check_equal(
            results,
            "enhanced_pet_in_stack_position",
            "In-stack position matches.",
            "In-stack position differs.",
            item_u32(path, content, tags::IN_STACK_POSITION_NUMBER)?,
            expected.in_stack_position_numbers[frame],
        );
        check_equal(
            results,
            "enhanced_pet_temporal_position",
            "Temporal Position Index matches.",
            "Temporal Position Index differs.",
            item_u32(path, content, tags::TEMPORAL_POSITION_INDEX)?,
            expected.temporal_position_indices[frame],
        );
        check_equal(
            results,
            "enhanced_pet_dimension_index_value",
            "Dimension Index Value matches.",
            "Dimension Index Value differs.",
            item_u32(path, content, tags::DIMENSION_INDEX_VALUES)?,
            expected.dimension_index_values[frame],
        );
        check_equal(
            results,
            "enhanced_pet_plane_position",
            "Plane Position matches.",
            "Plane Position differs.",
            nested_sequence_item_str(
                path,
                item,
                tags::PLANE_POSITION_SEQUENCE,
                0,
                tags::IMAGE_POSITION_PATIENT,
            )?
            .as_str(),
            expected.image_position_patient[frame],
        );
    }
    Ok(())
}

fn validate_mg_image(
    path: &Path,
    obj: &OpenedObject,
    results: &mut Vec<Value>,
    expected: &MgImageExpectations<'_>,
) -> Result<(), GenerateError> {
    for (name, tag, expected_value) in [
        ("mg_modality", tags::MODALITY, expected.modality),
        (
            "mg_presentation_intent_type",
            tags::PRESENTATION_INTENT_TYPE,
            expected.presentation_intent_type,
        ),
        ("mg_image_type", tags::IMAGE_TYPE, expected.image_type),
        (
            "mg_image_laterality",
            tags::IMAGE_LATERALITY,
            expected.image_laterality,
        ),
        (
            "mg_view_position",
            tags::VIEW_POSITION,
            expected.view_position,
        ),
        (
            "mg_body_part_examined",
            tags::BODY_PART_EXAMINED,
            expected.body_part_examined,
        ),
        (
            "mg_organ_exposed",
            tags::ORGAN_EXPOSED,
            expected.organ_exposed,
        ),
        (
            "mg_positioner_type",
            tags::POSITIONER_TYPE,
            expected.positioner_type,
        ),
        (
            "mg_imager_pixel_spacing",
            tags::IMAGER_PIXEL_SPACING,
            expected.imager_pixel_spacing,
        ),
        (
            "mg_detector_type",
            tags::DETECTOR_TYPE,
            expected.detector_type,
        ),
        (
            "mg_detector_configuration",
            tags::DETECTOR_CONFIGURATION,
            expected.detector_configuration,
        ),
        ("mg_detector_id", tags::DETECTOR_ID, expected.detector_id),
        (
            "mg_pixel_intensity_relationship",
            tags::PIXEL_INTENSITY_RELATIONSHIP,
            expected.pixel_intensity_relationship,
        ),
        (
            "mg_rescale_intercept",
            tags::RESCALE_INTERCEPT,
            expected.rescale_intercept,
        ),
        (
            "mg_rescale_slope",
            tags::RESCALE_SLOPE,
            expected.rescale_slope,
        ),
        ("mg_rescale_type", tags::RESCALE_TYPE, expected.rescale_type),
        (
            "mg_presentation_lut_shape",
            tags::PRESENTATION_LUT_SHAPE,
            expected.presentation_lut_shape,
        ),
        (
            "mg_lossy_image_compression",
            tags::LOSSY_IMAGE_COMPRESSION,
            expected.lossy_image_compression,
        ),
        (
            "mg_burned_in_annotation",
            tags::BURNED_IN_ANNOTATION,
            expected.burned_in_annotation,
        ),
        (
            "mg_breast_implant_present",
            tags::BREAST_IMPLANT_PRESENT,
            expected.breast_implant_present,
        ),
    ] {
        check_equal(
            results,
            name,
            "Mammography attribute matches the recipe.",
            "Mammography attribute does not match the recipe.",
            element_str(path, obj, tag)?.as_str(),
            expected_value,
        );
    }
    validate_optional_mg_window(path, obj, results, expected)?;
    check_equal(
        results,
        "mg_pixel_intensity_relationship_sign",
        "Pixel Intensity Relationship Sign matches the recipe.",
        "Pixel Intensity Relationship Sign does not match the recipe.",
        element_i16(path, obj, tags::PIXEL_INTENSITY_RELATIONSHIP_SIGN)?,
        expected.pixel_intensity_relationship_sign,
    );

    check_equal(
        results,
        "mg_anatomic_region_sequence",
        "Anatomic Region Sequence contains the expected code.",
        "Anatomic Region Sequence does not contain the expected code.",
        first_sequence_code_value(path, obj, tags::ANATOMIC_REGION_SEQUENCE)?.as_str(),
        expected.anatomic_region_code_value,
    );
    check_equal(
        results,
        "mg_view_code_sequence",
        "View Code Sequence contains the expected code.",
        "View Code Sequence does not contain the expected code.",
        first_sequence_code_value(path, obj, tags::VIEW_CODE_SEQUENCE)?.as_str(),
        expected.view_code_value,
    );
    check_equal(
        results,
        "mg_acquisition_context_sequence",
        "Acquisition Context Sequence has the expected item count.",
        "Acquisition Context Sequence does not have the expected item count.",
        sequence_item_count(path, obj, tags::ACQUISITION_CONTEXT_SEQUENCE)?,
        expected.acquisition_context_items,
    );

    Ok(())
}

fn validate_optional_mg_window(
    path: &Path,
    obj: &OpenedObject,
    results: &mut Vec<Value>,
    expected: &MgImageExpectations<'_>,
) -> Result<(), GenerateError> {
    match (expected.window_center, expected.window_width) {
        (Some(window_center), Some(window_width)) => {
            for (name, tag, expected_value) in [
                ("mg_window_center", tags::WINDOW_CENTER, window_center),
                ("mg_window_width", tags::WINDOW_WIDTH, window_width),
            ] {
                check_equal(
                    results,
                    name,
                    "Mammography window attribute matches the recipe.",
                    "Mammography window attribute does not match the recipe.",
                    element_str(path, obj, tag)?.as_str(),
                    expected_value,
                );
            }
        }
        (None, None) => {
            for (name, tag) in [
                ("mg_window_center_absent", tags::WINDOW_CENTER),
                ("mg_window_width_absent", tags::WINDOW_WIDTH),
            ] {
                let present = obj
                    .element_opt(tag)
                    .map_err(|err| validation_error(path, err))?
                    .is_some();
                check(
                    results,
                    !present,
                    name,
                    "Mammography window attribute is absent for FOR PROCESSING.",
                    "Mammography window attribute is present for FOR PROCESSING.",
                );
            }
        }
        _ => {
            check(
                results,
                false,
                "mg_window_pair_consistency",
                "Window Center and Window Width are both present or both absent.",
                "Window Center and Window Width are not paired consistently.",
            );
        }
    }

    Ok(())
}

fn validate_dx_image(
    path: &Path,
    obj: &OpenedObject,
    results: &mut Vec<Value>,
    expected: &DxImageExpectations<'_>,
) -> Result<(), GenerateError> {
    for (name, tag, expected_value) in [
        ("dx_modality", tags::MODALITY, expected.modality),
        (
            "dx_presentation_intent_type",
            tags::PRESENTATION_INTENT_TYPE,
            expected.presentation_intent_type,
        ),
        ("dx_image_type", tags::IMAGE_TYPE, expected.image_type),
        (
            "dx_image_laterality",
            tags::IMAGE_LATERALITY,
            expected.image_laterality,
        ),
        (
            "dx_body_part_examined",
            tags::BODY_PART_EXAMINED,
            expected.body_part_examined,
        ),
        (
            "dx_imager_pixel_spacing",
            tags::IMAGER_PIXEL_SPACING,
            expected.imager_pixel_spacing,
        ),
        (
            "dx_detector_type",
            tags::DETECTOR_TYPE,
            expected.detector_type,
        ),
        (
            "dx_detector_configuration",
            tags::DETECTOR_CONFIGURATION,
            expected.detector_configuration,
        ),
        ("dx_detector_id", tags::DETECTOR_ID, expected.detector_id),
        (
            "dx_pixel_intensity_relationship",
            tags::PIXEL_INTENSITY_RELATIONSHIP,
            expected.pixel_intensity_relationship,
        ),
        (
            "dx_rescale_intercept",
            tags::RESCALE_INTERCEPT,
            expected.rescale_intercept,
        ),
        (
            "dx_rescale_slope",
            tags::RESCALE_SLOPE,
            expected.rescale_slope,
        ),
        ("dx_rescale_type", tags::RESCALE_TYPE, expected.rescale_type),
        (
            "dx_presentation_lut_shape",
            tags::PRESENTATION_LUT_SHAPE,
            expected.presentation_lut_shape,
        ),
        (
            "dx_lossy_image_compression",
            tags::LOSSY_IMAGE_COMPRESSION,
            expected.lossy_image_compression,
        ),
        (
            "dx_burned_in_annotation",
            tags::BURNED_IN_ANNOTATION,
            expected.burned_in_annotation,
        ),
        (
            "dx_window_center",
            tags::WINDOW_CENTER,
            expected.window_center,
        ),
        ("dx_window_width", tags::WINDOW_WIDTH, expected.window_width),
        (
            "dx_shutter_shape",
            tags::SHUTTER_SHAPE,
            expected.shutter_shape,
        ),
        (
            "dx_shutter_left_vertical_edge",
            tags::SHUTTER_LEFT_VERTICAL_EDGE,
            expected.shutter_left_vertical_edge,
        ),
        (
            "dx_shutter_right_vertical_edge",
            tags::SHUTTER_RIGHT_VERTICAL_EDGE,
            expected.shutter_right_vertical_edge,
        ),
        (
            "dx_shutter_upper_horizontal_edge",
            tags::SHUTTER_UPPER_HORIZONTAL_EDGE,
            expected.shutter_upper_horizontal_edge,
        ),
        (
            "dx_shutter_lower_horizontal_edge",
            tags::SHUTTER_LOWER_HORIZONTAL_EDGE,
            expected.shutter_lower_horizontal_edge,
        ),
    ] {
        check_equal(
            results,
            name,
            "Digital X-Ray attribute matches the recipe.",
            "Digital X-Ray attribute does not match the recipe.",
            element_str(path, obj, tag)?.as_str(),
            expected_value,
        );
    }

    check_equal(
        results,
        "dx_pixel_intensity_relationship_sign",
        "Pixel Intensity Relationship Sign matches the recipe.",
        "Pixel Intensity Relationship Sign does not match the recipe.",
        element_i16(path, obj, tags::PIXEL_INTENSITY_RELATIONSHIP_SIGN)?,
        expected.pixel_intensity_relationship_sign,
    );
    check_equal(
        results,
        "dx_anatomic_region_sequence",
        "Anatomic Region Sequence contains the expected code.",
        "Anatomic Region Sequence does not contain the expected code.",
        first_sequence_code_value(path, obj, tags::ANATOMIC_REGION_SEQUENCE)?.as_str(),
        expected.anatomic_region_code_value,
    );
    check_equal(
        results,
        "dx_acquisition_context_sequence",
        "Acquisition Context Sequence has the expected item count.",
        "Acquisition Context Sequence does not have the expected item count.",
        sequence_item_count(path, obj, tags::ACQUISITION_CONTEXT_SEQUENCE)?,
        expected.acquisition_context_items,
    );
    check_equal(
        results,
        "dx_shutter_presentation_value",
        "Shutter Presentation Value matches the recipe.",
        "Shutter Presentation Value does not match the recipe.",
        element_u16(path, obj, tags::SHUTTER_PRESENTATION_VALUE)?,
        expected.shutter_presentation_value,
    );

    Ok(())
}

fn validate_xa_image(
    path: &Path,
    obj: &OpenedObject,
    results: &mut Vec<Value>,
    expected: &XaImageExpectations<'_>,
) -> Result<(), GenerateError> {
    for (name, tag, expected_value) in [
        ("xa_modality", tags::MODALITY, expected.modality),
        (
            "xa_body_part_examined",
            tags::BODY_PART_EXAMINED,
            expected.body_part_examined,
        ),
        ("xa_image_type", tags::IMAGE_TYPE, expected.image_type),
        (
            "xa_patient_orientation_empty",
            tags::PATIENT_ORIENTATION,
            expected.patient_orientation,
        ),
        (
            "xa_pixel_intensity_relationship",
            tags::PIXEL_INTENSITY_RELATIONSHIP,
            expected.pixel_intensity_relationship,
        ),
        (
            "xa_lossy_image_compression",
            tags::LOSSY_IMAGE_COMPRESSION,
            expected.lossy_image_compression,
        ),
        (
            "xa_radiation_setting",
            tags::RADIATION_SETTING,
            expected.radiation_setting,
        ),
        ("xa_kvp", tags::KVP, expected.kvp),
        ("xa_exposure", tags::EXPOSURE, expected.exposure_mas),
        (
            "xa_imager_pixel_spacing",
            tags::IMAGER_PIXEL_SPACING,
            expected.imager_pixel_spacing_mm,
        ),
        (
            "xa_positioner_primary_angle",
            tags::POSITIONER_PRIMARY_ANGLE,
            expected.positioner_primary_angle_degrees,
        ),
        (
            "xa_positioner_secondary_angle",
            tags::POSITIONER_SECONDARY_ANGLE,
            expected.positioner_secondary_angle_degrees,
        ),
        (
            "xa_distance_source_to_detector",
            tags::DISTANCE_SOURCE_TO_DETECTOR,
            expected.distance_source_to_detector_mm,
        ),
        (
            "xa_distance_source_to_patient",
            tags::DISTANCE_SOURCE_TO_PATIENT,
            expected.distance_source_to_patient_mm,
        ),
        (
            "xa_estimated_magnification",
            tags::ESTIMATED_RADIOGRAPHIC_MAGNIFICATION_FACTOR,
            expected.estimated_radiographic_magnification_factor,
        ),
    ] {
        check_equal(
            results,
            name,
            "XA acquisition or projection attribute matches the recipe.",
            "XA acquisition or projection attribute does not match the recipe.",
            element_str(path, obj, tag)?.as_str(),
            expected_value,
        );
    }

    for (name, tag, expected_vr) in [
        ("xa_image_type_vr", tags::IMAGE_TYPE, VR::CS),
        (
            "xa_patient_orientation_vr",
            tags::PATIENT_ORIENTATION,
            VR::CS,
        ),
        ("xa_kvp_vr", tags::KVP, VR::DS),
        ("xa_exposure_vr", tags::EXPOSURE, VR::IS),
        (
            "xa_imager_pixel_spacing_vr",
            tags::IMAGER_PIXEL_SPACING,
            VR::DS,
        ),
        (
            "xa_positioner_primary_angle_vr",
            tags::POSITIONER_PRIMARY_ANGLE,
            VR::DS,
        ),
        (
            "xa_positioner_secondary_angle_vr",
            tags::POSITIONER_SECONDARY_ANGLE,
            VR::DS,
        ),
    ] {
        let actual_vr = obj
            .element(tag)
            .map_err(|err| validation_error(path, err))?
            .vr();
        check_equal(
            results,
            name,
            "XA attribute VR matches the 2026b data dictionary.",
            "XA attribute VR does not match the 2026b data dictionary.",
            actual_vr,
            expected_vr,
        );
    }

    let sid = element_f64_values(path, obj, tags::DISTANCE_SOURCE_TO_DETECTOR)?[0];
    let sod = element_f64_values(path, obj, tags::DISTANCE_SOURCE_TO_PATIENT)?[0];
    let magnification =
        element_f64_values(path, obj, tags::ESTIMATED_RADIOGRAPHIC_MAGNIFICATION_FACTOR)?[0];
    check(
        results,
        (sid / sod - magnification).abs() <= f64::EPSILON,
        "xa_sid_sod_magnification_relation",
        "Estimated magnification equals the serialized SID/SOD ratio.",
        "Estimated magnification does not equal the serialized SID/SOD ratio.",
    );

    for (name, tag) in [
        ("xa_laterality_absent", tags::LATERALITY),
        ("xa_number_of_frames_absent", tags::NUMBER_OF_FRAMES),
        (
            "xa_frame_increment_pointer_absent",
            tags::FRAME_INCREMENT_POINTER,
        ),
        ("xa_frame_time_absent", tags::FRAME_TIME),
        ("xa_frame_time_vector_absent", tags::FRAME_TIME_VECTOR),
        ("xa_positioner_motion_absent", tags::POSITIONER_MOTION),
        (
            "xa_primary_angle_increment_absent",
            tags::POSITIONER_PRIMARY_ANGLE_INCREMENT,
        ),
        (
            "xa_secondary_angle_increment_absent",
            tags::POSITIONER_SECONDARY_ANGLE_INCREMENT,
        ),
        (
            "xa_biplane_reference_absent",
            tags::REFERENCED_IMAGE_SEQUENCE,
        ),
        ("xa_contrast_agent_absent", tags::CONTRAST_BOLUS_AGENT),
        (
            "xa_mask_subtraction_absent",
            tags::MASK_SUBTRACTION_SEQUENCE,
        ),
        ("xa_frame_of_reference_absent", tags::FRAME_OF_REFERENCE_UID),
        (
            "xa_image_orientation_patient_absent",
            tags::IMAGE_ORIENTATION_PATIENT,
        ),
        (
            "xa_image_position_patient_absent",
            tags::IMAGE_POSITION_PATIENT,
        ),
        ("xa_pixel_spacing_absent", tags::PIXEL_SPACING),
        ("xa_modality_lut_absent", tags::MODALITY_LUT_SEQUENCE),
        ("xa_voi_lut_absent", tags::VOILUT_SEQUENCE),
        ("xa_calibration_image_absent", tags::CALIBRATION_IMAGE),
    ] {
        let present = obj
            .element_opt(tag)
            .map_err(|err| validation_error(path, err))?
            .is_some();
        check(
            results,
            !present,
            name,
            "Excluded XA conditional or optional claim is absent.",
            "An excluded XA conditional or optional claim is unexpectedly present.",
        );
    }

    Ok(())
}

fn validate_xrf_image(
    path: &Path,
    obj: &OpenedObject,
    results: &mut Vec<Value>,
    expected: &XrfImageExpectations<'_>,
) -> Result<(), GenerateError> {
    for (name, tag, expected_value) in [
        ("xrf_modality", tags::MODALITY, expected.modality),
        (
            "xrf_body_part_examined",
            tags::BODY_PART_EXAMINED,
            expected.body_part_examined,
        ),
        ("xrf_image_type", tags::IMAGE_TYPE, expected.image_type),
        (
            "xrf_patient_orientation_empty",
            tags::PATIENT_ORIENTATION,
            expected.patient_orientation,
        ),
        (
            "xrf_pixel_intensity_relationship",
            tags::PIXEL_INTENSITY_RELATIONSHIP,
            expected.pixel_intensity_relationship,
        ),
        (
            "xrf_lossy_image_compression",
            tags::LOSSY_IMAGE_COMPRESSION,
            expected.lossy_image_compression,
        ),
        (
            "xrf_radiation_setting",
            tags::RADIATION_SETTING,
            expected.radiation_setting,
        ),
        ("xrf_kvp", tags::KVP, expected.kvp),
        ("xrf_exposure", tags::EXPOSURE, expected.exposure_mas),
        (
            "xrf_imager_pixel_spacing",
            tags::IMAGER_PIXEL_SPACING,
            expected.imager_pixel_spacing_mm,
        ),
        (
            "xrf_distance_source_to_detector",
            tags::DISTANCE_SOURCE_TO_DETECTOR,
            expected.distance_source_to_detector_mm,
        ),
        (
            "xrf_distance_source_to_patient",
            tags::DISTANCE_SOURCE_TO_PATIENT,
            expected.distance_source_to_patient_mm,
        ),
        (
            "xrf_estimated_magnification",
            tags::ESTIMATED_RADIOGRAPHIC_MAGNIFICATION_FACTOR,
            expected.estimated_radiographic_magnification_factor,
        ),
        (
            "xrf_column_angulation",
            tags::COLUMN_ANGULATION,
            expected.column_angulation_degrees,
        ),
    ] {
        check_equal(
            results,
            name,
            "XRF acquisition or projection attribute matches the recipe.",
            "XRF acquisition or projection attribute does not match the recipe.",
            element_str(path, obj, tag)?.as_str(),
            expected_value,
        );
    }

    for (name, tag, expected_vr) in [
        ("xrf_modality_vr", tags::MODALITY, VR::CS),
        (
            "xrf_body_part_examined_vr",
            tags::BODY_PART_EXAMINED,
            VR::CS,
        ),
        ("xrf_image_type_vr", tags::IMAGE_TYPE, VR::CS),
        (
            "xrf_patient_orientation_vr",
            tags::PATIENT_ORIENTATION,
            VR::CS,
        ),
        (
            "xrf_pixel_intensity_relationship_vr",
            tags::PIXEL_INTENSITY_RELATIONSHIP,
            VR::CS,
        ),
        (
            "xrf_lossy_image_compression_vr",
            tags::LOSSY_IMAGE_COMPRESSION,
            VR::CS,
        ),
        ("xrf_radiation_setting_vr", tags::RADIATION_SETTING, VR::CS),
        ("xrf_kvp_vr", tags::KVP, VR::DS),
        ("xrf_exposure_vr", tags::EXPOSURE, VR::IS),
        (
            "xrf_imager_pixel_spacing_vr",
            tags::IMAGER_PIXEL_SPACING,
            VR::DS,
        ),
        (
            "xrf_distance_source_to_detector_vr",
            tags::DISTANCE_SOURCE_TO_DETECTOR,
            VR::DS,
        ),
        (
            "xrf_distance_source_to_patient_vr",
            tags::DISTANCE_SOURCE_TO_PATIENT,
            VR::DS,
        ),
        (
            "xrf_estimated_magnification_vr",
            tags::ESTIMATED_RADIOGRAPHIC_MAGNIFICATION_FACTOR,
            VR::DS,
        ),
        ("xrf_column_angulation_vr", tags::COLUMN_ANGULATION, VR::DS),
    ] {
        let actual_vr = obj
            .element(tag)
            .map_err(|err| validation_error(path, err))?
            .vr();
        check_equal(
            results,
            name,
            "XRF attribute VR matches the 2026b data dictionary.",
            "XRF attribute VR does not match the 2026b data dictionary.",
            actual_vr,
            expected_vr,
        );
    }

    let sid = element_f64_values(path, obj, tags::DISTANCE_SOURCE_TO_DETECTOR)?[0];
    let sod = element_f64_values(path, obj, tags::DISTANCE_SOURCE_TO_PATIENT)?[0];
    let magnification =
        element_f64_values(path, obj, tags::ESTIMATED_RADIOGRAPHIC_MAGNIFICATION_FACTOR)?[0];
    check(
        results,
        (sid / sod - magnification).abs() <= f64::EPSILON,
        "xrf_sid_sod_magnification_relation",
        "Estimated magnification equals the serialized XRF SID/SOD ratio.",
        "Estimated magnification does not equal the serialized XRF SID/SOD ratio.",
    );

    for (name, tag) in [
        ("xrf_laterality_absent", tags::LATERALITY),
        ("xrf_exposure_time_absent", tags::EXPOSURE_TIME),
        ("xrf_exposure_time_in_us_absent", tags::EXPOSURE_TIME_INU_S),
        ("xrf_x_ray_tube_current_absent", tags::X_RAY_TUBE_CURRENT),
        (
            "xrf_x_ray_tube_current_in_ua_absent",
            tags::X_RAY_TUBE_CURRENT_INU_A,
        ),
        ("xrf_exposure_in_uas_absent", tags::EXPOSURE_INU_AS),
        ("xrf_radiation_mode_absent", tags::RADIATION_MODE),
        ("xrf_average_pulse_width_absent", tags::AVERAGE_PULSE_WIDTH),
        (
            "xrf_positioner_primary_angle_absent",
            tags::POSITIONER_PRIMARY_ANGLE,
        ),
        (
            "xrf_positioner_secondary_angle_absent",
            tags::POSITIONER_SECONDARY_ANGLE,
        ),
        ("xrf_positioner_motion_absent", tags::POSITIONER_MOTION),
        (
            "xrf_primary_angle_increment_absent",
            tags::POSITIONER_PRIMARY_ANGLE_INCREMENT,
        ),
        (
            "xrf_secondary_angle_increment_absent",
            tags::POSITIONER_SECONDARY_ANGLE_INCREMENT,
        ),
        ("xrf_number_of_frames_absent", tags::NUMBER_OF_FRAMES),
        (
            "xrf_frame_increment_pointer_absent",
            tags::FRAME_INCREMENT_POINTER,
        ),
        ("xrf_frame_time_absent", tags::FRAME_TIME),
        ("xrf_frame_time_vector_absent", tags::FRAME_TIME_VECTOR),
        (
            "xrf_biplane_reference_absent",
            tags::REFERENCED_IMAGE_SEQUENCE,
        ),
        ("xrf_contrast_agent_absent", tags::CONTRAST_BOLUS_AGENT),
        (
            "xrf_mask_subtraction_absent",
            tags::MASK_SUBTRACTION_SEQUENCE,
        ),
        ("xrf_table_height_absent", tags::TABLE_HEIGHT),
        ("xrf_table_traverse_absent", tags::TABLE_TRAVERSE),
        ("xrf_table_position_absent", tags::TABLE_POSITION),
        ("xrf_table_motion_absent", tags::TABLE_MOTION),
        (
            "xrf_table_vertical_increment_absent",
            tags::TABLE_VERTICAL_INCREMENT,
        ),
        (
            "xrf_table_lateral_increment_absent",
            tags::TABLE_LATERAL_INCREMENT,
        ),
        (
            "xrf_table_longitudinal_increment_absent",
            tags::TABLE_LONGITUDINAL_INCREMENT,
        ),
        ("xrf_table_tilt_absent", tags::TABLE_ANGLE),
        ("xrf_scan_options_absent", tags::SCAN_OPTIONS),
        ("xrf_tomo_layer_height_absent", tags::TOMO_LAYER_HEIGHT),
        ("xrf_tomo_angle_absent", tags::TOMO_ANGLE),
        ("xrf_tomo_time_absent", tags::TOMO_TIME),
        ("xrf_tomo_type_absent", tags::TOMO_TYPE),
        ("xrf_tomo_class_absent", tags::TOMO_CLASS),
        (
            "xrf_number_of_tomosynthesis_source_images_absent",
            dicom_core::Tag(0x0018, 0x1495),
        ),
        (
            "xrf_frame_of_reference_absent",
            tags::FRAME_OF_REFERENCE_UID,
        ),
        (
            "xrf_image_orientation_patient_absent",
            tags::IMAGE_ORIENTATION_PATIENT,
        ),
        (
            "xrf_image_position_patient_absent",
            tags::IMAGE_POSITION_PATIENT,
        ),
        ("xrf_pixel_spacing_absent", tags::PIXEL_SPACING),
        ("xrf_modality_lut_absent", tags::MODALITY_LUT_SEQUENCE),
        ("xrf_voi_lut_absent", tags::VOILUT_SEQUENCE),
        (
            "xrf_presentation_lut_shape_absent",
            tags::PRESENTATION_LUT_SHAPE,
        ),
        ("xrf_window_center_absent", tags::WINDOW_CENTER),
        ("xrf_window_width_absent", tags::WINDOW_WIDTH),
        ("xrf_shutter_shape_absent", tags::SHUTTER_SHAPE),
        (
            "xrf_shutter_left_vertical_edge_absent",
            tags::SHUTTER_LEFT_VERTICAL_EDGE,
        ),
        (
            "xrf_shutter_right_vertical_edge_absent",
            tags::SHUTTER_RIGHT_VERTICAL_EDGE,
        ),
        (
            "xrf_shutter_upper_horizontal_edge_absent",
            tags::SHUTTER_UPPER_HORIZONTAL_EDGE,
        ),
        (
            "xrf_shutter_lower_horizontal_edge_absent",
            tags::SHUTTER_LOWER_HORIZONTAL_EDGE,
        ),
        ("xrf_overlay_rows_absent", dicom_core::Tag(0x6000, 0x0010)),
        ("xrf_overlay_data_absent", dicom_core::Tag(0x6000, 0x3000)),
        ("xrf_collimator_shape_absent", tags::COLLIMATOR_SHAPE),
        (
            "xrf_collimator_left_vertical_edge_absent",
            tags::COLLIMATOR_LEFT_VERTICAL_EDGE,
        ),
        (
            "xrf_collimator_right_vertical_edge_absent",
            tags::COLLIMATOR_RIGHT_VERTICAL_EDGE,
        ),
        (
            "xrf_collimator_upper_horizontal_edge_absent",
            tags::COLLIMATOR_UPPER_HORIZONTAL_EDGE,
        ),
        (
            "xrf_collimator_lower_horizontal_edge_absent",
            tags::COLLIMATOR_LOWER_HORIZONTAL_EDGE,
        ),
        (
            "xrf_area_dose_product_absent",
            tags::IMAGE_AND_FLUOROSCOPY_AREA_DOSE_PRODUCT,
        ),
        ("xrf_calibration_image_absent", tags::CALIBRATION_IMAGE),
        (
            "xrf_lossy_image_compression_ratio_absent",
            tags::LOSSY_IMAGE_COMPRESSION_RATIO,
        ),
        (
            "xrf_lossy_image_compression_method_absent",
            tags::LOSSY_IMAGE_COMPRESSION_METHOD,
        ),
        ("xrf_detector_type_absent", tags::DETECTOR_TYPE),
        (
            "xrf_detector_configuration_absent",
            tags::DETECTOR_CONFIGURATION,
        ),
        ("xrf_detector_id_absent", tags::DETECTOR_ID),
        (
            "xrf_detector_description_absent",
            tags::DETECTOR_DESCRIPTION,
        ),
        (
            "xrf_detector_element_physical_size_absent",
            tags::DETECTOR_ELEMENT_PHYSICAL_SIZE,
        ),
        (
            "xrf_detector_element_spacing_absent",
            tags::DETECTOR_ELEMENT_SPACING,
        ),
    ] {
        let present = obj
            .element_opt(tag)
            .map_err(|err| validation_error(path, err))?
            .is_some();
        check(
            results,
            !present,
            name,
            "Excluded XRF conditional or optional claim is absent.",
            "An excluded XRF conditional or optional claim is unexpectedly present.",
        );
    }

    Ok(())
}

fn validate_us_image(
    path: &Path,
    obj: &OpenedObject,
    results: &mut Vec<Value>,
    expected: &UsImageExpectations<'_>,
) -> Result<(), GenerateError> {
    for (name, tag, expected_value) in [
        ("us_modality", tags::MODALITY, expected.modality),
        ("us_image_type", tags::IMAGE_TYPE, expected.image_type),
        (
            "us_lossy_image_compression",
            tags::LOSSY_IMAGE_COMPRESSION,
            expected.lossy_image_compression,
        ),
    ] {
        check_equal(
            results,
            name,
            "Ultrasound attribute matches the recipe.",
            "Ultrasound attribute does not match the recipe.",
            element_str(path, obj, tag)?.as_str(),
            expected_value,
        );
    }

    check_equal(
        results,
        "us_ultrasound_color_data_present",
        "Ultrasound Color Data Present matches the recipe.",
        "Ultrasound Color Data Present does not match the recipe.",
        element_u16(path, obj, tags::ULTRASOUND_COLOR_DATA_PRESENT)?,
        expected.ultrasound_color_data_present,
    );

    Ok(())
}

fn validate_us_multiframe(
    path: &Path,
    obj: &OpenedObject,
    results: &mut Vec<Value>,
    expected: &UsMultiframeExpectations<'_>,
) -> Result<(), GenerateError> {
    for (name, tag, expected_value) in [
        ("us_multiframe_modality", tags::MODALITY, expected.modality),
        (
            "us_multiframe_body_part_examined",
            tags::BODY_PART_EXAMINED,
            expected.body_part_examined,
        ),
        (
            "us_multiframe_image_type",
            tags::IMAGE_TYPE,
            expected.image_type,
        ),
        (
            "us_multiframe_lossy_image_compression",
            tags::LOSSY_IMAGE_COMPRESSION,
            expected.lossy_image_compression,
        ),
        (
            "us_multiframe_frame_time",
            tags::FRAME_TIME,
            expected.frame_time_ms,
        ),
    ] {
        check_equal(
            results,
            name,
            "Ultrasound multi-frame attribute matches the recipe.",
            "Ultrasound multi-frame attribute does not match the recipe.",
            element_str(path, obj, tag)?.as_str(),
            expected_value,
        );
    }

    check_equal(
        results,
        "us_multiframe_color_data_present",
        "Ultrasound Color Data Present matches the monochrome recipe.",
        "Ultrasound Color Data Present does not match the monochrome recipe.",
        element_u16(path, obj, tags::ULTRASOUND_COLOR_DATA_PRESENT)?,
        expected.ultrasound_color_data_present,
    );
    check_equal(
        results,
        "us_multiframe_number_of_frames",
        "Number of Frames matches the recipe.",
        "Number of Frames does not match the recipe.",
        element_u16(path, obj, tags::NUMBER_OF_FRAMES)?,
        expected.number_of_frames,
    );
    check_equal(
        results,
        "us_multiframe_frame_increment_pointer",
        "Frame Increment Pointer names Frame Time exactly.",
        "Frame Increment Pointer does not name Frame Time exactly.",
        element_tags(path, obj, tags::FRAME_INCREMENT_POINTER)?,
        vec![expected.frame_increment_pointer],
    );

    for (name, tag) in [
        (
            "us_multiframe_frame_time_vector_absent",
            tags::FRAME_TIME_VECTOR,
        ),
        (
            "us_multiframe_frame_of_reference_absent",
            tags::FRAME_OF_REFERENCE_UID,
        ),
        ("us_multiframe_laterality_absent", tags::LATERALITY),
        (
            "us_multiframe_region_calibration_absent",
            tags::SEQUENCE_OF_ULTRASOUND_REGIONS,
        ),
    ] {
        let present = obj
            .element_opt(tag)
            .map_err(|err| validation_error(path, err))?
            .is_some();
        check(
            results,
            !present,
            name,
            "Optional ultrasound claim is explicitly absent.",
            "An optional ultrasound claim is unexpectedly present.",
        );
    }

    Ok(())
}

fn validate_nm_image(
    path: &Path,
    obj: &OpenedObject,
    results: &mut Vec<Value>,
    expected: &NmImageExpectations<'_>,
) -> Result<(), GenerateError> {
    for (name, tag, expected_value) in [
        ("nm_modality", tags::MODALITY, expected.modality),
        (
            "nm_body_part_examined",
            tags::BODY_PART_EXAMINED,
            expected.body_part_examined,
        ),
        ("nm_image_type", tags::IMAGE_TYPE, expected.image_type),
        (
            "nm_pixel_spacing",
            tags::PIXEL_SPACING,
            expected.pixel_spacing,
        ),
        (
            "nm_actual_frame_duration",
            tags::ACTUAL_FRAME_DURATION,
            expected.actual_frame_duration_ms,
        ),
        (
            "nm_counts_accumulated",
            tags::COUNTS_ACCUMULATED,
            expected.counts_accumulated,
        ),
    ] {
        check_equal(
            results,
            name,
            "Nuclear Medicine attribute matches the recipe.",
            "Nuclear Medicine attribute does not match the recipe.",
            element_str(path, obj, tag)?.as_str(),
            expected_value,
        );
    }

    check_equal(
        results,
        "nm_frame_increment_pointers",
        "Frame Increment Pointer preserves the ordered NM dimensions.",
        "Frame Increment Pointer does not preserve the ordered NM dimensions.",
        element_tags(path, obj, tags::FRAME_INCREMENT_POINTER)?,
        expected.frame_increment_pointers.to_vec(),
    );
    check_equal(
        results,
        "nm_energy_window_vector",
        "Energy Window Vector matches every frame tuple.",
        "Energy Window Vector does not match every frame tuple.",
        element_u16_values(path, obj, tags::ENERGY_WINDOW_VECTOR)?,
        expected.energy_window_vector.to_vec(),
    );
    check_equal(
        results,
        "nm_detector_vector",
        "Detector Vector matches every frame tuple.",
        "Detector Vector does not match every frame tuple.",
        element_u16_values(path, obj, tags::DETECTOR_VECTOR)?,
        expected.detector_vector.to_vec(),
    );
    check_equal(
        results,
        "nm_number_of_energy_windows",
        "Number of Energy Windows matches the recipe.",
        "Number of Energy Windows does not match the recipe.",
        element_u16(path, obj, tags::NUMBER_OF_ENERGY_WINDOWS)?,
        expected.energy_windows.len() as u16,
    );
    check_equal(
        results,
        "nm_energy_window_information_count",
        "Energy Window Information Sequence cardinality matches its declared count.",
        "Energy Window Information Sequence cardinality does not match its declared count.",
        sequence_item_count(path, obj, tags::ENERGY_WINDOW_INFORMATION_SEQUENCE)?,
        expected.energy_windows.len(),
    );
    for (index, window) in expected.energy_windows.iter().enumerate() {
        let item =
            top_level_sequence_item(path, obj, tags::ENERGY_WINDOW_INFORMATION_SEQUENCE, index)?;
        check_equal(
            results,
            "nm_energy_window_name",
            "Energy Window Name matches its one-based dimension index.",
            "Energy Window Name does not match its one-based dimension index.",
            item_str(path, item, tags::ENERGY_WINDOW_NAME)?.as_str(),
            window.name,
        );
        check_equal(
            results,
            "nm_energy_window_range_count",
            "Each Energy Window has one range Item.",
            "An Energy Window does not have exactly one range Item.",
            item_sequence_item_count(path, item, tags::ENERGY_WINDOW_RANGE_SEQUENCE)?,
            1,
        );
        let range = item_sequence_item(path, item, tags::ENERGY_WINDOW_RANGE_SEQUENCE, 0)?;
        check_equal(
            results,
            "nm_energy_window_lower_limit",
            "Energy Window lower limit matches the recipe.",
            "Energy Window lower limit does not match the recipe.",
            item_str(path, range, tags::ENERGY_WINDOW_LOWER_LIMIT)?.as_str(),
            window.lower_limit_kev,
        );
        check_equal(
            results,
            "nm_energy_window_upper_limit",
            "Energy Window upper limit matches the recipe.",
            "Energy Window upper limit does not match the recipe.",
            item_str(path, range, tags::ENERGY_WINDOW_UPPER_LIMIT)?.as_str(),
            window.upper_limit_kev,
        );
    }

    check_equal(
        results,
        "nm_number_of_detectors",
        "Number of Detectors matches the recipe.",
        "Number of Detectors does not match the recipe.",
        element_u16(path, obj, tags::NUMBER_OF_DETECTORS)?,
        expected.detectors.len() as u16,
    );
    check_equal(
        results,
        "nm_detector_information_count",
        "Detector Information Sequence cardinality matches its declared count.",
        "Detector Information Sequence cardinality does not match its declared count.",
        sequence_item_count(path, obj, tags::DETECTOR_INFORMATION_SEQUENCE)?,
        expected.detectors.len(),
    );
    for (index, detector) in expected.detectors.iter().enumerate() {
        let item = top_level_sequence_item(path, obj, tags::DETECTOR_INFORMATION_SEQUENCE, index)?;
        for (name, tag, expected_value) in [
            (
                "nm_detector_collimator_type",
                tags::COLLIMATOR_TYPE,
                detector.collimator_type,
            ),
            (
                "nm_detector_focal_distance",
                tags::FOCAL_DISTANCE,
                detector.focal_distance_mm,
            ),
            (
                "nm_detector_start_angle",
                tags::START_ANGLE,
                detector.start_angle_degrees,
            ),
            (
                "nm_detector_image_orientation_patient",
                tags::IMAGE_ORIENTATION_PATIENT,
                detector.image_orientation_patient,
            ),
            (
                "nm_detector_image_position_patient",
                tags::IMAGE_POSITION_PATIENT,
                detector.image_position_patient,
            ),
        ] {
            check_equal(
                results,
                name,
                "Detector Information Item matches its one-based dimension index.",
                "Detector Information Item does not match its one-based dimension index.",
                item_str(path, item, tag)?.as_str(),
                expected_value,
            );
        }
    }

    for (name, tag, expected_count) in [
        (
            "nm_radiopharmaceutical_information_empty",
            tags::RADIOPHARMACEUTICAL_INFORMATION_SEQUENCE,
            0,
        ),
        (
            "nm_patient_orientation_code_empty",
            tags::PATIENT_ORIENTATION_CODE_SEQUENCE,
            0,
        ),
        (
            "nm_patient_gantry_relationship_code_empty",
            tags::PATIENT_GANTRY_RELATIONSHIP_CODE_SEQUENCE,
            0,
        ),
    ] {
        check_equal(
            results,
            name,
            "Required Type 2 NM Sequence is present with the expected cardinality.",
            "Required Type 2 NM Sequence is missing or has the wrong cardinality.",
            sequence_item_count(path, obj, tag)?,
            expected_count,
        );
    }

    Ok(())
}

fn validate_pet_image(
    path: &Path,
    obj: &OpenedObject,
    results: &mut Vec<Value>,
    expected: &PetImageExpectations<'_>,
) -> Result<(), GenerateError> {
    for (name, tag, expected_value) in [
        ("pet_modality", tags::MODALITY, expected.modality),
        (
            "pet_body_part_examined",
            tags::BODY_PART_EXAMINED,
            expected.body_part_examined,
        ),
        ("pet_image_type", tags::IMAGE_TYPE, expected.image_type),
        ("pet_series_date", tags::SERIES_DATE, expected.series_date),
        ("pet_series_time", tags::SERIES_TIME, expected.series_time),
        ("pet_units", tags::UNITS, expected.units),
        (
            "pet_counts_source",
            tags::COUNTS_SOURCE,
            expected.counts_source,
        ),
        ("pet_series_type", tags::SERIES_TYPE, expected.series_type),
        (
            "pet_corrected_image",
            tags::CORRECTED_IMAGE,
            expected.corrected_image,
        ),
        (
            "pet_decay_correction",
            tags::DECAY_CORRECTION,
            expected.decay_correction,
        ),
        (
            "pet_collimator_type",
            tags::COLLIMATOR_TYPE,
            expected.collimator_type,
        ),
        (
            "pet_frame_of_reference_uid",
            tags::FRAME_OF_REFERENCE_UID,
            expected.frame_of_reference_uid,
        ),
        (
            "pet_position_reference_indicator",
            tags::POSITION_REFERENCE_INDICATOR,
            expected.position_reference_indicator,
        ),
        (
            "pet_rescale_intercept",
            tags::RESCALE_INTERCEPT,
            expected.rescale_intercept,
        ),
        (
            "pet_rescale_slope",
            tags::RESCALE_SLOPE,
            expected.rescale_slope,
        ),
        (
            "pet_dose_calibration_factor",
            tags::DOSE_CALIBRATION_FACTOR,
            expected.dose_calibration_factor,
        ),
        (
            "pet_frame_reference_time",
            tags::FRAME_REFERENCE_TIME,
            expected.frame_reference_time_ms,
        ),
        (
            "pet_acquisition_date",
            tags::ACQUISITION_DATE,
            expected.acquisition_date,
        ),
        (
            "pet_acquisition_time",
            tags::ACQUISITION_TIME,
            expected.acquisition_time,
        ),
        (
            "pet_actual_frame_duration",
            tags::ACTUAL_FRAME_DURATION,
            expected.actual_frame_duration_ms,
        ),
        (
            "pet_pixel_spacing",
            tags::PIXEL_SPACING,
            expected.pixel_spacing,
        ),
        (
            "pet_image_orientation_patient",
            tags::IMAGE_ORIENTATION_PATIENT,
            expected.image_orientation_patient,
        ),
        (
            "pet_image_position_patient",
            tags::IMAGE_POSITION_PATIENT,
            expected.image_position_patient,
        ),
        (
            "pet_slice_thickness",
            tags::SLICE_THICKNESS,
            expected.slice_thickness,
        ),
    ] {
        check_equal(
            results,
            name,
            "PET attribute matches the recipe.",
            "PET attribute does not match the recipe.",
            element_str(path, obj, tag)?.as_str(),
            expected_value,
        );
    }

    for (name, tag, expected_value) in [
        (
            "pet_number_of_slices",
            tags::NUMBER_OF_SLICES,
            expected.number_of_slices,
        ),
        ("pet_image_index", tags::IMAGE_INDEX, expected.image_index),
    ] {
        check_equal(
            results,
            name,
            "PET unsigned integer attribute matches the recipe.",
            "PET unsigned integer attribute does not match the recipe.",
            element_u16(path, obj, tag)?,
            expected_value,
        );
    }

    for (name, tag, expected_count) in [
        (
            "pet_radiopharmaceutical_information_empty",
            tags::RADIOPHARMACEUTICAL_INFORMATION_SEQUENCE,
            expected.radiopharmaceutical_information_items,
        ),
        (
            "pet_patient_orientation_code_empty",
            tags::PATIENT_ORIENTATION_CODE_SEQUENCE,
            expected.patient_orientation_code_items,
        ),
        (
            "pet_patient_gantry_relationship_code_empty",
            tags::PATIENT_GANTRY_RELATIONSHIP_CODE_SEQUENCE,
            expected.patient_gantry_relationship_code_items,
        ),
    ] {
        check_equal(
            results,
            name,
            "Required Type 2 PET Sequence is present with the expected cardinality.",
            "Required Type 2 PET Sequence is missing or has the wrong cardinality.",
            sequence_item_count(path, obj, tag)?,
            expected_count,
        );
    }

    let pixel_bytes = obj
        .element(tags::PIXEL_DATA)
        .map_err(|err| validation_error(path, err))?
        .value()
        .to_bytes()
        .map_err(|err| validation_error(path, err))?;
    let stored_values = pixel_bytes
        .chunks_exact(2)
        .map(|bytes| u16::from_le_bytes([bytes[0], bytes[1]]))
        .collect::<Vec<_>>();
    check_equal(
        results,
        "pet_stored_values",
        "Reopened PET stored values match the recipe.",
        "Reopened PET stored values do not match the recipe.",
        stored_values.as_slice(),
        expected.stored_values,
    );
    let intercept = expected.rescale_intercept.parse::<f64>().map_err(|err| {
        GenerateError::ValidateDicomFile {
            path: path.to_path_buf(),
            message: format!("invalid expected PET Rescale Intercept: {err}"),
        }
    })?;
    let slope =
        expected
            .rescale_slope
            .parse::<f64>()
            .map_err(|err| GenerateError::ValidateDicomFile {
                path: path.to_path_buf(),
                message: format!("invalid expected PET Rescale Slope: {err}"),
            })?;
    let activity_values = stored_values
        .iter()
        .map(|value| slope * f64::from(*value) + intercept)
        .collect::<Vec<_>>();
    check_equal(
        results,
        "pet_rescaled_activity_values_bqml",
        "Reopened PET stored values map to the expected BQML activity values.",
        "Reopened PET stored values do not map to the expected BQML activity values.",
        activity_values.as_slice(),
        expected.activity_values_bqml,
    );

    for (name, tag) in [
        ("pet_decay_factor_absent_for_none", tags::DECAY_FACTOR),
        ("pet_trigger_time_absent_for_static", tags::TRIGGER_TIME),
        ("pet_frame_time_absent_for_static", tags::FRAME_TIME),
        (
            "pet_reprojection_method_absent_for_image",
            tags::REPROJECTION_METHOD,
        ),
    ] {
        let present = obj
            .element_opt(tag)
            .map_err(|err| validation_error(path, err))?
            .is_some();
        check(
            results,
            !present,
            name,
            "Conditional PET attribute is absent because its condition is false.",
            "Conditional PET attribute is unexpectedly present.",
        );
    }

    Ok(())
}

fn validate_cr_image(
    path: &Path,
    obj: &OpenedObject,
    results: &mut Vec<Value>,
    expected: &CrImageExpectations<'_>,
) -> Result<(), GenerateError> {
    for (name, tag, expected_value) in [
        ("cr_modality", tags::MODALITY, expected.modality),
        ("cr_image_type", tags::IMAGE_TYPE, expected.image_type),
        (
            "cr_body_part_examined",
            tags::BODY_PART_EXAMINED,
            expected.body_part_examined,
        ),
        (
            "cr_view_position",
            tags::VIEW_POSITION,
            expected.view_position,
        ),
        (
            "cr_acquisition_number",
            tags::ACQUISITION_NUMBER,
            expected.acquisition_number,
        ),
        (
            "cr_overlay_type",
            tags::OVERLAY_TYPE.inner(),
            expected.overlay_type,
        ),
    ] {
        check_equal(
            results,
            name,
            "Computed Radiography attribute matches the recipe.",
            "Computed Radiography attribute does not match the recipe.",
            element_str(path, obj, tag)?.as_str(),
            expected_value,
        );
    }
    for (name, tag, expected_value) in [
        (
            "cr_overlay_rows",
            tags::OVERLAY_ROWS.inner(),
            expected.overlay_rows,
        ),
        (
            "cr_overlay_columns",
            tags::OVERLAY_COLUMNS.inner(),
            expected.overlay_columns,
        ),
        (
            "cr_overlay_bits_allocated",
            tags::OVERLAY_BITS_ALLOCATED.inner(),
            expected.overlay_bits_allocated,
        ),
        (
            "cr_overlay_bit_position",
            tags::OVERLAY_BIT_POSITION.inner(),
            expected.overlay_bit_position,
        ),
    ] {
        check_equal(
            results,
            name,
            "Computed Radiography overlay numeric attribute matches the recipe.",
            "Computed Radiography overlay numeric attribute does not match the recipe.",
            element_u16(path, obj, tag)?,
            expected_value,
        );
    }
    check_equal(
        results,
        "cr_overlay_origin",
        "Computed Radiography overlay origin matches the recipe.",
        "Computed Radiography overlay origin does not match the recipe.",
        element_i16_values(path, obj, tags::OVERLAY_ORIGIN.inner())?,
        expected.overlay_origin.clone(),
    );
    let overlay_data = obj
        .element(tags::OVERLAY_DATA.inner())
        .map_err(|err| validation_error(path, err))?;
    let overlay_data_length = overlay_data
        .value()
        .to_bytes()
        .map(|bytes| bytes.len())
        .map_err(|err| validation_error(path, err))?;
    check_equal(
        results,
        "cr_overlay_data",
        "Computed Radiography overlay data VR and length match the recipe.",
        "Computed Radiography overlay data VR or length does not match the recipe.",
        (overlay_data.vr(), overlay_data_length),
        (VR::OW, expected.overlay_data_length),
    );

    validate_lut_sequence(
        path,
        obj,
        results,
        tags::MODALITY_LUT_SEQUENCE,
        "cr_modality_lut",
        expected.modality_lut_descriptor,
        Some(expected.modality_lut_type),
        expected.modality_lut_data_length,
    )?;
    validate_lut_sequence(
        path,
        obj,
        results,
        tags::VOILUT_SEQUENCE,
        "cr_voi_lut",
        expected.voi_lut_descriptor,
        None,
        expected.voi_lut_data_length,
    )?;

    Ok(())
}

fn validate_lut_sequence(
    path: &Path,
    obj: &OpenedObject,
    results: &mut Vec<Value>,
    tag: Tag,
    name_prefix: &str,
    expected_descriptor: [u16; 3],
    expected_modality_lut_type: Option<&str>,
    expected_data_length: usize,
) -> Result<(), GenerateError> {
    let element = obj
        .element(tag)
        .map_err(|err| validation_error(path, err))?;
    let item = element
        .items()
        .and_then(|items| items.first())
        .ok_or_else(|| GenerateError::ValidateDicomFile {
            path: path.to_path_buf(),
            message: format!("sequence {} has no first item", tag),
        })?;
    check_equal(
        results,
        &format!("{name_prefix}_item_count"),
        "LUT Sequence has one item.",
        "LUT Sequence does not have one item.",
        sequence_item_count(path, obj, tag)?,
        1,
    );
    check_equal(
        results,
        &format!("{name_prefix}_descriptor"),
        "LUT Descriptor matches the recipe.",
        "LUT Descriptor does not match the recipe.",
        item.element(tags::LUT_DESCRIPTOR)
            .map_err(|err| validation_error(path, err))?
            .value()
            .to_multi_int::<u16>()
            .map_err(|err| validation_error(path, err))?,
        expected_descriptor.to_vec(),
    );
    let lut_data = item
        .element(tags::LUT_DATA)
        .map_err(|err| validation_error(path, err))?;
    let lut_data_length = lut_data
        .value()
        .to_bytes()
        .map(|bytes| bytes.len())
        .map_err(|err| validation_error(path, err))?;
    check_equal(
        results,
        &format!("{name_prefix}_data"),
        "LUT Data VR and length match the recipe.",
        "LUT Data VR or length does not match the recipe.",
        (lut_data.vr(), lut_data_length),
        (VR::OW, expected_data_length),
    );
    if let Some(expected_modality_lut_type) = expected_modality_lut_type {
        let value = item
            .element(tags::MODALITY_LUT_TYPE)
            .map_err(|err| validation_error(path, err))?
            .value()
            .to_str()
            .map_err(|err| validation_error(path, err))?;
        check_equal(
            results,
            &format!("{name_prefix}_type"),
            "Modality LUT Type matches the recipe.",
            "Modality LUT Type does not match the recipe.",
            value.trim_matches('\0').trim(),
            expected_modality_lut_type,
        );
    }

    Ok(())
}

fn validate_mr_image(
    path: &Path,
    obj: &OpenedObject,
    results: &mut Vec<Value>,
    expected: &MrImageExpectations<'_>,
) -> Result<(), GenerateError> {
    for (name, tag, expected_value) in [
        ("mr_modality", tags::MODALITY, expected.modality),
        (
            "mr_frame_of_reference_uid",
            tags::FRAME_OF_REFERENCE_UID,
            expected.frame_of_reference_uid,
        ),
        ("mr_image_type", tags::IMAGE_TYPE, expected.image_type),
        (
            "mr_instance_number",
            tags::INSTANCE_NUMBER,
            expected.instance_number,
        ),
        (
            "mr_acquisition_number",
            tags::ACQUISITION_NUMBER,
            expected.acquisition_number,
        ),
        (
            "mr_pixel_spacing",
            tags::PIXEL_SPACING,
            expected.pixel_spacing,
        ),
        (
            "mr_image_orientation_patient",
            tags::IMAGE_ORIENTATION_PATIENT,
            expected.image_orientation_patient,
        ),
        (
            "mr_image_position_patient",
            tags::IMAGE_POSITION_PATIENT,
            expected.image_position_patient,
        ),
        (
            "mr_slice_thickness",
            tags::SLICE_THICKNESS,
            expected.slice_thickness,
        ),
        (
            "mr_spacing_between_slices",
            tags::SPACING_BETWEEN_SLICES,
            expected.spacing_between_slices,
        ),
        (
            "mr_slice_location",
            tags::SLICE_LOCATION,
            expected.slice_location,
        ),
        (
            "mr_scanning_sequence",
            tags::SCANNING_SEQUENCE,
            expected.scanning_sequence,
        ),
        (
            "mr_sequence_variant",
            tags::SEQUENCE_VARIANT,
            expected.sequence_variant,
        ),
        ("mr_scan_options", tags::SCAN_OPTIONS, expected.scan_options),
        (
            "mr_acquisition_type",
            tags::MR_ACQUISITION_TYPE,
            expected.mr_acquisition_type,
        ),
        (
            "mr_repetition_time",
            tags::REPETITION_TIME,
            expected.repetition_time,
        ),
        ("mr_echo_time", tags::ECHO_TIME, expected.echo_time),
        (
            "mr_echo_train_length",
            tags::ECHO_TRAIN_LENGTH,
            expected.echo_train_length,
        ),
        (
            "mr_magnetic_field_strength",
            tags::MAGNETIC_FIELD_STRENGTH,
            expected.magnetic_field_strength,
        ),
    ] {
        check_equal(
            results,
            name,
            "MR Image attribute matches the recipe.",
            "MR Image attribute does not match the recipe.",
            element_str(path, obj, tag)?.as_str(),
            expected_value,
        );
    }

    check_equal(
        results,
        "mr_slice_order_index",
        "MR slice order index is recorded for deterministic geometry sorting.",
        "MR slice order index is not recorded as expected.",
        expected.slice_order_index,
        expected.slice_order_index,
    );
    check_equal(
        results,
        "mr_slice_count",
        "MR slice count is recorded for deterministic geometry sorting.",
        "MR slice count is not recorded as expected.",
        expected.slice_count,
        expected.slice_count,
    );
    let orientation = element_f64_values(path, obj, tags::IMAGE_ORIENTATION_PATIENT)?;
    let position = element_f64_values(path, obj, tags::IMAGE_POSITION_PATIENT)?;
    let position_along_normal = if orientation.len() == 6 && position.len() == 3 {
        let row = [orientation[0], orientation[1], orientation[2]];
        let column = [orientation[3], orientation[4], orientation[5]];
        let normal = [
            row[1] * column[2] - row[2] * column[1],
            row[2] * column[0] - row[0] * column[2],
            row[0] * column[1] - row[1] * column[0],
        ];
        Some(normal[0] * position[0] + normal[1] * position[1] + normal[2] * position[2])
    } else {
        None
    };
    let position_matches = position_along_normal
        .map(|actual| (actual - expected.position_along_normal).abs() < 0.000_01)
        .unwrap_or(false);
    check(
        results,
        position_matches,
        "mr_position_along_normal",
        "MR position along slice normal matches the deterministic geometry sort key.",
        "MR position along slice normal does not match the deterministic geometry sort key.",
    );

    Ok(())
}

fn first_sequence_code_value(
    path: &Path,
    obj: &OpenedObject,
    tag: Tag,
) -> Result<String, GenerateError> {
    let element = obj
        .element(tag)
        .map_err(|err| validation_error(path, err))?;
    let item = element
        .items()
        .and_then(|items| items.first())
        .ok_or_else(|| GenerateError::ValidateDicomFile {
            path: path.to_path_buf(),
            message: format!("sequence {} has no first item", tag),
        })?;
    let value = item
        .element(tags::CODE_VALUE)
        .map_err(|err| validation_error(path, err))?
        .value()
        .to_str()
        .map_err(|err| validation_error(path, err))?;
    Ok(value.trim_matches('\0').trim().to_string())
}

fn top_level_sequence_item<'a>(
    path: &Path,
    obj: &'a OpenedObject,
    tag: Tag,
    index: usize,
) -> Result<&'a DatasetObject, GenerateError> {
    let element = obj
        .element(tag)
        .map_err(|err| validation_error(path, err))?;
    let items = element
        .items()
        .ok_or_else(|| GenerateError::ValidateDicomFile {
            path: path.to_path_buf(),
            message: format!("attribute {} is not a sequence", tag),
        })?;
    items
        .get(index)
        .ok_or_else(|| GenerateError::ValidateDicomFile {
            path: path.to_path_buf(),
            message: format!("sequence {} has no item at index {}", tag, index),
        })
}

fn top_level_sequence_item_str(
    path: &Path,
    obj: &OpenedObject,
    sequence_tag: Tag,
    index: usize,
    tag: Tag,
) -> Result<String, GenerateError> {
    let item = top_level_sequence_item(path, obj, sequence_tag, index)?;
    item_str(path, item, tag)
}

fn top_level_sequence_item_u16(
    path: &Path,
    obj: &OpenedObject,
    sequence_tag: Tag,
    index: usize,
    tag: Tag,
) -> Result<u16, GenerateError> {
    let item = top_level_sequence_item(path, obj, sequence_tag, index)?;
    item_u16(path, item, tag)
}

fn item_sequence_item<'a>(
    path: &Path,
    obj: &'a DatasetObject,
    tag: Tag,
    index: usize,
) -> Result<&'a DatasetObject, GenerateError> {
    let element = obj
        .element(tag)
        .map_err(|err| validation_error(path, err))?;
    let items = element
        .items()
        .ok_or_else(|| GenerateError::ValidateDicomFile {
            path: path.to_path_buf(),
            message: format!("attribute {} is not a sequence", tag),
        })?;
    items
        .get(index)
        .ok_or_else(|| GenerateError::ValidateDicomFile {
            path: path.to_path_buf(),
            message: format!("sequence {} has no item at index {}", tag, index),
        })
}

fn item_sequence_item_count(
    path: &Path,
    obj: &DatasetObject,
    tag: Tag,
) -> Result<usize, GenerateError> {
    let element = obj
        .element(tag)
        .map_err(|err| validation_error(path, err))?;
    element
        .items()
        .map(|items| items.len())
        .ok_or_else(|| GenerateError::ValidateDicomFile {
            path: path.to_path_buf(),
            message: format!("attribute {} is not a sequence", tag),
        })
}

fn nested_sequence_item_str(
    path: &Path,
    obj: &DatasetObject,
    sequence_tag: Tag,
    index: usize,
    tag: Tag,
) -> Result<String, GenerateError> {
    let item = item_sequence_item(path, obj, sequence_tag, index)?;
    item_str(path, item, tag)
}

fn nested_sequence_item_u32(
    path: &Path,
    obj: &DatasetObject,
    sequence_tag: Tag,
    index: usize,
    tag: Tag,
) -> Result<u32, GenerateError> {
    let item = item_sequence_item(path, obj, sequence_tag, index)?;
    item.element(tag)
        .map_err(|err| validation_error(path, err))?
        .value()
        .to_int::<u32>()
        .map_err(|err| validation_error(path, err))
}

fn nested_sequence_item_u16(
    path: &Path,
    obj: &DatasetObject,
    sequence_tag: Tag,
    index: usize,
    tag: Tag,
) -> Result<u16, GenerateError> {
    let item = item_sequence_item(path, obj, sequence_tag, index)?;
    item.element(tag)
        .map_err(|err| validation_error(path, err))?
        .value()
        .to_int::<u16>()
        .map_err(|err| validation_error(path, err))
}

fn nested_sequence_item_f64(
    path: &Path,
    obj: &DatasetObject,
    sequence_tag: Tag,
    index: usize,
    tag: Tag,
) -> Result<f64, GenerateError> {
    let item = item_sequence_item(path, obj, sequence_tag, index)?;
    item.element(tag)
        .map_err(|err| validation_error(path, err))?
        .value()
        .to_float64()
        .map_err(|err| validation_error(path, err))
}

fn nested_sequence_item_f64_values(
    path: &Path,
    obj: &DatasetObject,
    sequence_tag: Tag,
    index: usize,
    tag: Tag,
) -> Result<Vec<f64>, GenerateError> {
    let item = item_sequence_item(path, obj, sequence_tag, index)?;
    item.element(tag)
        .map_err(|err| validation_error(path, err))?
        .value()
        .to_multi_float64()
        .map_err(|err| validation_error(path, err))
}

#[allow(clippy::too_many_arguments)]
fn validate_sr_code(
    results: &mut Vec<Value>,
    path: &Path,
    obj: &DatasetObject,
    sequence_tag: Tag,
    prefix: &str,
    expected_code_value: &str,
    expected_coding_scheme: &str,
    expected_code_meaning: &str,
) -> Result<(), GenerateError> {
    let code = item_sequence_item(path, obj, sequence_tag, 0)?;
    for (suffix, tag, expected) in [
        ("code_value", tags::CODE_VALUE, expected_code_value),
        (
            "coding_scheme_designator",
            tags::CODING_SCHEME_DESIGNATOR,
            expected_coding_scheme,
        ),
        ("code_meaning", tags::CODE_MEANING, expected_code_meaning),
    ] {
        check_equal(
            results,
            &format!("{prefix}_{suffix}"),
            "Structured Report coded value matches the recipe.",
            "Structured Report coded value does not match the recipe.",
            item_str(path, code, tag)?.as_str(),
            expected,
        );
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_sr_content_item(
    results: &mut Vec<Value>,
    path: &Path,
    item: &DatasetObject,
    prefix: &str,
    expected_relationship: &str,
    expected_value_type: &str,
    expected_code_value: &str,
    expected_coding_scheme: &str,
    expected_code_meaning: &str,
) -> Result<(), GenerateError> {
    check_equal(
        results,
        &format!("{prefix}_relationship_type"),
        "Structured Report relationship type matches the recipe.",
        "Structured Report relationship type does not match the recipe.",
        item_str(path, item, tags::RELATIONSHIP_TYPE)?.as_str(),
        expected_relationship,
    );
    check_equal(
        results,
        &format!("{prefix}_value_type"),
        "Structured Report value type matches the recipe.",
        "Structured Report value type does not match the recipe.",
        item_str(path, item, tags::VALUE_TYPE)?.as_str(),
        expected_value_type,
    );
    validate_sr_code(
        results,
        path,
        item,
        tags::CONCEPT_NAME_CODE_SEQUENCE,
        prefix,
        expected_code_value,
        expected_coding_scheme,
        expected_code_meaning,
    )
}

fn check_sr_reference(
    results: &mut Vec<Value>,
    path: &Path,
    reference: &DatasetObject,
    prefix: &str,
    expected_sop_class_uid: &str,
    expected_sop_instance_uid: &str,
) -> Result<(), GenerateError> {
    check_equal(
        results,
        &format!("{prefix}_sop_class_uid"),
        "Structured Report reference SOP Class UID matches the recipe.",
        "Structured Report reference SOP Class UID does not match the recipe.",
        item_str(path, reference, TAG_REFERENCED_SOP_CLASS_UID)?.as_str(),
        expected_sop_class_uid,
    );
    check_equal(
        results,
        &format!("{prefix}_sop_instance_uid"),
        "Structured Report reference SOP Instance UID matches the recipe.",
        "Structured Report reference SOP Instance UID does not match the recipe.",
        item_str(path, reference, TAG_REFERENCED_SOP_INSTANCE_UID)?.as_str(),
        expected_sop_instance_uid,
    );
    Ok(())
}

fn item_str(path: &Path, obj: &DatasetObject, tag: Tag) -> Result<String, GenerateError> {
    let value = obj
        .element(tag)
        .map_err(|err| validation_error(path, err))?
        .value()
        .to_str()
        .map_err(|err| validation_error(path, err))?;
    Ok(value.trim_matches('\0').trim().to_string())
}

fn item_u16(path: &Path, obj: &DatasetObject, tag: Tag) -> Result<u16, GenerateError> {
    obj.element(tag)
        .map_err(|err| validation_error(path, err))?
        .value()
        .to_int::<u16>()
        .map_err(|err| validation_error(path, err))
}

fn item_u32(path: &Path, obj: &DatasetObject, tag: Tag) -> Result<u32, GenerateError> {
    obj.element(tag)
        .map_err(|err| validation_error(path, err))?
        .value()
        .to_int::<u32>()
        .map_err(|err| validation_error(path, err))
}

fn item_u32_values(path: &Path, obj: &DatasetObject, tag: Tag) -> Result<Vec<u32>, GenerateError> {
    obj.element(tag)
        .map_err(|err| validation_error(path, err))?
        .value()
        .to_multi_int::<u32>()
        .map_err(|err| validation_error(path, err))
}

fn item_tag(path: &Path, obj: &DatasetObject, tag: Tag) -> Result<Tag, GenerateError> {
    obj.element(tag)
        .map_err(|err| validation_error(path, err))?
        .value()
        .to_tag()
        .map_err(|err| validation_error(path, err))
}

fn item_f64(path: &Path, obj: &DatasetObject, tag: Tag) -> Result<f64, GenerateError> {
    obj.element(tag)
        .map_err(|err| validation_error(path, err))?
        .value()
        .to_float64()
        .map_err(|err| validation_error(path, err))
}

fn item_f64_values(path: &Path, obj: &DatasetObject, tag: Tag) -> Result<Vec<f64>, GenerateError> {
    obj.element(tag)
        .map_err(|err| validation_error(path, err))?
        .value()
        .to_multi_float64()
        .map_err(|err| validation_error(path, err))
}

fn item_f32_values(path: &Path, obj: &DatasetObject, tag: Tag) -> Result<Vec<f32>, GenerateError> {
    obj.element(tag)
        .map_err(|err| validation_error(path, err))?
        .value()
        .to_multi_float32()
        .map_err(|err| validation_error(path, err))
}

fn item_i32_values(path: &Path, obj: &DatasetObject, tag: Tag) -> Result<Vec<i32>, GenerateError> {
    obj.element(tag)
        .map_err(|err| validation_error(path, err))?
        .value()
        .to_multi_int::<i32>()
        .map_err(|err| validation_error(path, err))
}

fn sequence_item_count(path: &Path, obj: &OpenedObject, tag: Tag) -> Result<usize, GenerateError> {
    let element = obj
        .element(tag)
        .map_err(|err| validation_error(path, err))?;
    element
        .items()
        .map(|items| items.len())
        .ok_or_else(|| GenerateError::ValidateDicomFile {
            path: path.to_path_buf(),
            message: format!("attribute {} is not a sequence", tag),
        })
}

fn standard_sop_class_validation_name(sop_class_uid: &str) -> &'static str {
    match sop_class_uid {
        uids::SECONDARY_CAPTURE_IMAGE_STORAGE => "secondary_capture_sop_class",
        uids::CT_IMAGE_STORAGE => "ct_image_sop_class",
        uids::ENHANCED_CT_IMAGE_STORAGE => "enhanced_ct_image_sop_class",
        uids::COMPUTED_RADIOGRAPHY_IMAGE_STORAGE => "computed_radiography_image_sop_class",
        uids::MR_IMAGE_STORAGE => "mr_image_sop_class",
        uids::ENHANCED_MR_IMAGE_STORAGE => "enhanced_mr_image_sop_class",
        uids::ENHANCED_PET_IMAGE_STORAGE => "enhanced_pet_image_sop_class",
        uids::DIGITAL_X_RAY_IMAGE_STORAGE_FOR_PRESENTATION => {
            "digital_x_ray_for_presentation_sop_class"
        }
        uids::ULTRASOUND_IMAGE_STORAGE => "ultrasound_image_sop_class",
        uids::ULTRASOUND_MULTI_FRAME_IMAGE_STORAGE => "ultrasound_multiframe_image_sop_class",
        uids::X_RAY_ANGIOGRAPHIC_IMAGE_STORAGE => "x_ray_angiographic_image_sop_class",
        uids::X_RAY_RADIOFLUOROSCOPIC_IMAGE_STORAGE => "x_ray_radiofluoroscopic_image_sop_class",
        uids::NUCLEAR_MEDICINE_IMAGE_STORAGE => "nuclear_medicine_image_sop_class",
        uids::DIGITAL_MAMMOGRAPHY_X_RAY_IMAGE_STORAGE_FOR_PRESENTATION => {
            "digital_mammography_for_presentation_sop_class"
        }
        uids::DIGITAL_MAMMOGRAPHY_X_RAY_IMAGE_STORAGE_FOR_PROCESSING => {
            "digital_mammography_for_processing_sop_class"
        }
        uids::VL_PHOTOGRAPHIC_IMAGE_STORAGE => "vl_photographic_image_sop_class",
        uids::VL_ENDOSCOPIC_IMAGE_STORAGE => "vl_endoscopic_image_sop_class",
        uids::VL_MICROSCOPIC_IMAGE_STORAGE => "vl_microscopic_image_sop_class",
        "1.2.840.10008.5.1.4.1.1.11.1" => "grayscale_softcopy_presentation_state_sop_class",
        "1.2.840.10008.5.1.4.1.1.11.2" => "color_softcopy_presentation_state_sop_class",
        "1.2.840.10008.5.1.4.1.1.11.4" => "blending_softcopy_presentation_state_sop_class",
        "1.2.840.10008.5.1.4.1.1.9.1.1" => "twelve_lead_ecg_waveform_sop_class",
        "1.2.840.10008.5.1.4.1.1.9.1.2" => "general_ecg_waveform_sop_class",
        "1.2.840.10008.5.1.4.1.1.67" => "real_world_value_mapping_sop_class",
        uids::BASIC_TEXT_SR_STORAGE => "basic_text_sr_sop_class",
        uids::COMPREHENSIVE_SR_STORAGE => "comprehensive_sr_sop_class",
        uids::KEY_OBJECT_SELECTION_DOCUMENT_STORAGE => "key_object_selection_document_sop_class",
        uids::RT_STRUCTURE_SET_STORAGE => "rt_structure_set_sop_class",
        uids::RT_DOSE_STORAGE => "rt_dose_sop_class",
        uids::ENCAPSULATED_PDF_STORAGE => "encapsulated_pdf_sop_class",
        "1.2.840.10008.5.1.4.1.1.66.1" => "spatial_registration_sop_class",
        "1.2.840.10008.5.1.4.1.1.66.3" => "deformable_spatial_registration_sop_class",
        _ => "sop_class_uid",
    }
}

fn standard_sop_class_validation_message(sop_class_uid: &str) -> &'static str {
    match sop_class_uid {
        uids::SECONDARY_CAPTURE_IMAGE_STORAGE => {
            "SOP Class UID matches Secondary Capture Image Storage in the 2026b reference."
        }
        uids::CT_IMAGE_STORAGE => "SOP Class UID matches CT Image Storage in the 2026b reference.",
        uids::ENHANCED_CT_IMAGE_STORAGE => {
            "SOP Class UID matches Enhanced CT Image Storage in the 2026b reference."
        }
        uids::COMPUTED_RADIOGRAPHY_IMAGE_STORAGE => {
            "SOP Class UID matches Computed Radiography Image Storage in the 2026b reference."
        }
        uids::MR_IMAGE_STORAGE => "SOP Class UID matches MR Image Storage in the 2026b reference.",
        uids::ENHANCED_MR_IMAGE_STORAGE => {
            "SOP Class UID matches Enhanced MR Image Storage in the 2026b reference."
        }
        uids::ENHANCED_PET_IMAGE_STORAGE => {
            "SOP Class UID matches Enhanced PET Image Storage in the 2026b reference."
        }
        uids::DIGITAL_X_RAY_IMAGE_STORAGE_FOR_PRESENTATION => {
            "SOP Class UID matches Digital X-Ray Image Storage - For Presentation in the 2026b reference."
        }
        uids::ULTRASOUND_IMAGE_STORAGE => {
            "SOP Class UID matches Ultrasound Image Storage in the 2026b reference."
        }
        uids::ULTRASOUND_MULTI_FRAME_IMAGE_STORAGE => {
            "SOP Class UID matches Ultrasound Multi-frame Image Storage in the 2026b reference."
        }
        uids::X_RAY_ANGIOGRAPHIC_IMAGE_STORAGE => {
            "SOP Class UID matches X-Ray Angiographic Image Storage in the 2026b reference."
        }
        uids::X_RAY_RADIOFLUOROSCOPIC_IMAGE_STORAGE => {
            "SOP Class UID matches X-Ray Radiofluoroscopic Image Storage in the 2026b reference."
        }
        uids::NUCLEAR_MEDICINE_IMAGE_STORAGE => {
            "SOP Class UID matches Nuclear Medicine Image Storage in the 2026b reference."
        }
        uids::DIGITAL_MAMMOGRAPHY_X_RAY_IMAGE_STORAGE_FOR_PRESENTATION => {
            "SOP Class UID matches Digital Mammography X-Ray Image Storage - For Presentation in the 2026b reference."
        }
        uids::DIGITAL_MAMMOGRAPHY_X_RAY_IMAGE_STORAGE_FOR_PROCESSING => {
            "SOP Class UID matches Digital Mammography X-Ray Image Storage - For Processing in the 2026b reference."
        }
        uids::VL_PHOTOGRAPHIC_IMAGE_STORAGE => {
            "SOP Class UID matches VL Photographic Image Storage in the 2026b reference."
        }
        uids::VL_ENDOSCOPIC_IMAGE_STORAGE => {
            "SOP Class UID matches VL Endoscopic Image Storage in the 2026b reference."
        }
        uids::VL_MICROSCOPIC_IMAGE_STORAGE => {
            "SOP Class UID matches VL Microscopic Image Storage in the 2026b reference."
        }
        "1.2.840.10008.5.1.4.1.1.11.1" => {
            "SOP Class UID matches Grayscale Softcopy Presentation State Storage in the 2026b reference."
        }
        "1.2.840.10008.5.1.4.1.1.11.2" => {
            "SOP Class UID matches Color Softcopy Presentation State Storage in the 2026b reference."
        }
        "1.2.840.10008.5.1.4.1.1.11.4" => {
            "SOP Class UID matches Blending Softcopy Presentation State Storage in the 2026b reference."
        }
        "1.2.840.10008.5.1.4.1.1.9.1.1" => {
            "SOP Class UID matches 12-lead ECG Waveform Storage in the 2026b reference."
        }
        "1.2.840.10008.5.1.4.1.1.9.1.2" => {
            "SOP Class UID matches General ECG Waveform Storage in the 2026b reference."
        }
        "1.2.840.10008.5.1.4.1.1.67" => {
            "SOP Class UID matches Real World Value Mapping Storage in the 2026b reference."
        }
        uids::BASIC_TEXT_SR_STORAGE => {
            "SOP Class UID matches Basic Text SR Storage in the 2026b reference."
        }
        uids::COMPREHENSIVE_SR_STORAGE => {
            "SOP Class UID matches Comprehensive SR Storage in the 2026b reference."
        }
        uids::KEY_OBJECT_SELECTION_DOCUMENT_STORAGE => {
            "SOP Class UID matches Key Object Selection Document Storage in the 2026b reference."
        }
        uids::RT_STRUCTURE_SET_STORAGE => {
            "SOP Class UID matches RT Structure Set Storage in the 2026b reference."
        }
        uids::RT_DOSE_STORAGE => "SOP Class UID matches RT Dose Storage in the 2026b reference.",
        uids::ENCAPSULATED_PDF_STORAGE => {
            "SOP Class UID matches Encapsulated PDF Storage in the 2026b reference."
        }
        "1.2.840.10008.5.1.4.1.1.66.1" => {
            "SOP Class UID matches Spatial Registration Storage in the 2026b reference."
        }
        "1.2.840.10008.5.1.4.1.1.66.3" => {
            "SOP Class UID matches Deformable Spatial Registration Storage in the 2026b reference."
        }
        _ => "SOP Class UID matches the recipe.",
    }
}

fn standard_transfer_syntax_validation_name(transfer_syntax_uid: &str) -> &'static str {
    match transfer_syntax_uid {
        uids::EXPLICIT_VR_LITTLE_ENDIAN => "explicit_vr_little_endian_transfer_syntax",
        uids::IMPLICIT_VR_LITTLE_ENDIAN => "implicit_vr_little_endian_transfer_syntax",
        uids::DEFLATED_EXPLICIT_VR_LITTLE_ENDIAN => {
            "deflated_explicit_vr_little_endian_transfer_syntax"
        }
        "1.2.840.10008.1.2.2" => "explicit_vr_big_endian_transfer_syntax",
        _ => "transfer_syntax_uid",
    }
}

fn standard_transfer_syntax_validation_message(transfer_syntax_uid: &str) -> &'static str {
    match transfer_syntax_uid {
        uids::EXPLICIT_VR_LITTLE_ENDIAN => {
            "Transfer Syntax UID matches Explicit VR Little Endian in the 2026b reference."
        }
        uids::IMPLICIT_VR_LITTLE_ENDIAN => {
            "Transfer Syntax UID matches Implicit VR Little Endian in the 2026b reference."
        }
        uids::DEFLATED_EXPLICIT_VR_LITTLE_ENDIAN => {
            "Transfer Syntax UID matches Deflated Explicit VR Little Endian in the 2026b reference."
        }
        "1.2.840.10008.1.2.2" => {
            "Transfer Syntax UID matches retired Explicit VR Big Endian in the 2026b reference."
        }
        _ => "Transfer Syntax UID matches the recipe.",
    }
}

fn check(
    results: &mut Vec<Value>,
    passed: bool,
    name: &str,
    passed_message: &str,
    failed_message: &str,
) {
    results.push(serde_json::json!({
        "name": name,
        "status": if passed { "passed" } else { "failed" },
        "message": if passed { passed_message } else { failed_message }
    }));
}

fn check_equal<T>(
    results: &mut Vec<Value>,
    name: &str,
    passed_message: &str,
    failed_message: &str,
    actual: T,
    expected: T,
) where
    T: PartialEq,
{
    check(
        results,
        actual == expected,
        name,
        passed_message,
        failed_message,
    );
}

fn fail_if_any_failed(path: &Path, results: &[Value]) -> Result<(), GenerateError> {
    let failures: Vec<&str> = results
        .iter()
        .filter(|result| result.get("status").and_then(Value::as_str) == Some("failed"))
        .filter_map(|result| result.get("name").and_then(Value::as_str))
        .collect();
    if failures.is_empty() {
        Ok(())
    } else {
        Err(GenerateError::ValidateDicomFile {
            path: path.to_path_buf(),
            message: failures.join(", "),
        })
    }
}

fn trim_uid(uid: &str) -> String {
    uid.trim_matches('\0').trim().to_string()
}

fn validation_error(path: &Path, err: impl std::error::Error) -> GenerateError {
    GenerateError::ValidateDicomFile {
        path: PathBuf::from(path),
        message: err.to_string(),
    }
}
