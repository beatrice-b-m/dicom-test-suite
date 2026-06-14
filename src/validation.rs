use std::fs;
use std::path::{Path, PathBuf};

use dicom_core::{Tag, VR};
use dicom_dictionary_std::{StandardDataDictionary, tags, uids};
use dicom_object::{FileDicomObject, InMemDicomObject, open_file};
use serde_json::Value;

use crate::GenerateError;

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
    pub palette: Option<PaletteExpectations>,
    pub padding: Option<PixelPaddingExpectations>,
    pub ct_image: Option<CtImageExpectations<'a>>,
    pub enhanced_ct_image: Option<EnhancedCtImageExpectations<'a>>,
    pub enhanced_mr_image: Option<EnhancedMrImageExpectations<'a>>,
    pub mg_image: Option<MgImageExpectations<'a>>,
    pub dx_image: Option<DxImageExpectations<'a>>,
    pub us_image: Option<UsImageExpectations<'a>>,
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
    pub relationship_type: &'a str,
    pub image_value_type: &'a str,
    pub image_code_value: &'a str,
    pub image_coding_scheme_designator: &'a str,
    pub image_code_meaning: &'a str,
    pub key_objects: &'a [KeyObjectReferenceExpectations<'a>],
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum PixelDataLengthFormula {
    ContiguousSamples,
    YbrFull422,
    BitPackedFrames,
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
    pub value: u16,
    pub range_limit: Option<u16>,
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
    pub rescale_intercept: &'a str,
    pub rescale_slope: &'a str,
    pub rescale_type: &'a str,
    pub repetition_time: &'a str,
    pub flip_angle: &'a str,
    pub echo_train_length: &'a str,
    pub rf_echo_train_length: u16,
    pub gradient_echo_train_length: u16,
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
        let image_code =
            item_sequence_item(path, content_item, tags::CONCEPT_NAME_CODE_SEQUENCE, 0)?;
        check_equal(
            &mut internal,
            "kos_image_code_value",
            "KOS IMAGE item Code Value matches the recipe.",
            "KOS IMAGE item Code Value does not match the recipe.",
            item_str(path, image_code, tags::CODE_VALUE)?.as_str(),
            expected.image_code_value,
        );
        check_equal(
            &mut internal,
            "kos_image_coding_scheme",
            "KOS IMAGE item Coding Scheme Designator matches the recipe.",
            "KOS IMAGE item Coding Scheme Designator does not match the recipe.",
            item_str(path, image_code, tags::CODING_SCHEME_DESIGNATOR)?.as_str(),
            expected.image_coding_scheme_designator,
        );
        check_equal(
            &mut internal,
            "kos_image_code_meaning",
            "KOS IMAGE item Code Meaning matches the recipe.",
            "KOS IMAGE item Code Meaning does not match the recipe.",
            item_str(path, image_code, tags::CODE_MEANING)?.as_str(),
            expected.image_code_meaning,
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
    check_equal(
        results,
        "pixel_padding_value",
        "Pixel Padding Value matches the recipe.",
        "Pixel Padding Value does not match the recipe.",
        element_u16(path, obj, tags::PIXEL_PADDING_VALUE)?,
        expected.value,
    );
    if let Some(expected_range_limit) = expected.range_limit {
        check_equal(
            results,
            "pixel_padding_range_limit",
            "Pixel Padding Range Limit matches the recipe.",
            "Pixel Padding Range Limit does not match the recipe.",
            element_u16(path, obj, tags::PIXEL_PADDING_RANGE_LIMIT)?,
            expected_range_limit,
        );
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
        uids::DIGITAL_MAMMOGRAPHY_X_RAY_IMAGE_STORAGE_FOR_PRESENTATION => {
            "digital_mammography_for_presentation_sop_class"
        }
        uids::DIGITAL_MAMMOGRAPHY_X_RAY_IMAGE_STORAGE_FOR_PROCESSING => {
            "digital_mammography_for_processing_sop_class"
        }
        "1.2.840.10008.5.1.4.1.1.11.1" => "grayscale_softcopy_presentation_state_sop_class",
        "1.2.840.10008.5.1.4.1.1.67" => "real_world_value_mapping_sop_class",
        uids::BASIC_TEXT_SR_STORAGE => "basic_text_sr_sop_class",
        uids::COMPREHENSIVE_SR_STORAGE => "comprehensive_sr_sop_class",
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
        uids::DIGITAL_MAMMOGRAPHY_X_RAY_IMAGE_STORAGE_FOR_PRESENTATION => {
            "SOP Class UID matches Digital Mammography X-Ray Image Storage - For Presentation in the 2026b reference."
        }
        uids::DIGITAL_MAMMOGRAPHY_X_RAY_IMAGE_STORAGE_FOR_PROCESSING => {
            "SOP Class UID matches Digital Mammography X-Ray Image Storage - For Processing in the 2026b reference."
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
        _ => "SOP Class UID matches the recipe.",
    }
}

fn standard_transfer_syntax_validation_name(transfer_syntax_uid: &str) -> &'static str {
    match transfer_syntax_uid {
        uids::EXPLICIT_VR_LITTLE_ENDIAN => "explicit_vr_little_endian_transfer_syntax",
        uids::IMPLICIT_VR_LITTLE_ENDIAN => "implicit_vr_little_endian_transfer_syntax",
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
