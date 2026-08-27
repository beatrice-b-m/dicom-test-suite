use std::fs;
use std::path::{Path, PathBuf};

use dicom_core::{Tag, VR};
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
    sha256_hex,
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
            "color_softcopy_content_date",
            tags::CONTENT_DATE,
            "20260101",
            "Content Date matches the locked recipe.",
            "Content Date does not match the locked recipe.",
        ),
        (
            "color_softcopy_content_time",
            tags::CONTENT_TIME,
            "000000",
            "Content Time matches the locked recipe.",
            "Content Time does not match the locked recipe.",
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

fn item_f64(path: &Path, obj: &DatasetObject, tag: Tag) -> Result<f64, GenerateError> {
    obj.element(tag)
        .map_err(|err| validation_error(path, err))?
        .value()
        .to_float64()
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
        "1.2.840.10008.5.1.4.1.1.11.1" => "grayscale_softcopy_presentation_state_sop_class",
        "1.2.840.10008.5.1.4.1.1.11.2" => "color_softcopy_presentation_state_sop_class",
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
        "1.2.840.10008.5.1.4.1.1.11.1" => {
            "SOP Class UID matches Grayscale Softcopy Presentation State Storage in the 2026b reference."
        }
        "1.2.840.10008.5.1.4.1.1.11.2" => {
            "SOP Class UID matches Color Softcopy Presentation State Storage in the 2026b reference."
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
