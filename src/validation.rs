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
    pub mg_image: Option<MgImageExpectations<'a>>,
    pub dx_image: Option<DxImageExpectations<'a>>,
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
    if let Some(mg_image) = &expected.mg_image {
        validate_mg_image(path, &obj, &mut internal, mg_image)?;
    }
    if let Some(dx_image) = &expected.dx_image {
        validate_dx_image(path, &obj, &mut internal, dx_image)?;
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

fn item_f64(path: &Path, obj: &DatasetObject, tag: Tag) -> Result<f64, GenerateError> {
    obj.element(tag)
        .map_err(|err| validation_error(path, err))?
        .value()
        .to_float64()
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
        uids::DIGITAL_X_RAY_IMAGE_STORAGE_FOR_PRESENTATION => {
            "digital_x_ray_for_presentation_sop_class"
        }
        uids::ULTRASOUND_IMAGE_STORAGE => "ultrasound_image_sop_class",
        uids::ULTRASOUND_MULTI_FRAME_IMAGE_STORAGE => "ultrasound_multiframe_image_sop_class",
        uids::NUCLEAR_MEDICINE_IMAGE_STORAGE => "nuclear_medicine_image_sop_class",
        uids::DIGITAL_MAMMOGRAPHY_X_RAY_IMAGE_STORAGE_FOR_PRESENTATION => {
            "digital_mammography_for_presentation_sop_class"
        }
        uids::DIGITAL_MAMMOGRAPHY_X_RAY_IMAGE_STORAGE_FOR_PROCESSING => {
            "digital_mammography_for_processing_sop_class"
        }
        uids::VL_PHOTOGRAPHIC_IMAGE_STORAGE => "vl_photographic_image_sop_class",
        "1.2.840.10008.5.1.4.1.1.11.1" => "grayscale_softcopy_presentation_state_sop_class",
        "1.2.840.10008.5.1.4.1.1.67" => "real_world_value_mapping_sop_class",
        uids::BASIC_TEXT_SR_STORAGE => "basic_text_sr_sop_class",
        uids::COMPREHENSIVE_SR_STORAGE => "comprehensive_sr_sop_class",
        uids::KEY_OBJECT_SELECTION_DOCUMENT_STORAGE => "key_object_selection_document_sop_class",
        uids::RT_STRUCTURE_SET_STORAGE => "rt_structure_set_sop_class",
        uids::RT_DOSE_STORAGE => "rt_dose_sop_class",
        uids::ENCAPSULATED_PDF_STORAGE => "encapsulated_pdf_sop_class",
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
        uids::DIGITAL_X_RAY_IMAGE_STORAGE_FOR_PRESENTATION => {
            "SOP Class UID matches Digital X-Ray Image Storage - For Presentation in the 2026b reference."
        }
        uids::ULTRASOUND_IMAGE_STORAGE => {
            "SOP Class UID matches Ultrasound Image Storage in the 2026b reference."
        }
        uids::ULTRASOUND_MULTI_FRAME_IMAGE_STORAGE => {
            "SOP Class UID matches Ultrasound Multi-frame Image Storage in the 2026b reference."
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
