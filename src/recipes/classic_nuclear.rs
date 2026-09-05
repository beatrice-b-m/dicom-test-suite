//! Data-first planning for ordinary ultrasound, nuclear-medicine, and PET.

use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{
    CLASSIC_PIXEL_SLOT, CaseRecipe, ClassicFamilyProvider, ClassicInstanceRequest,
    ClassicPixelRequest, ClassicPlanError, CommonModuleRequest, ElementPresence,
    EquipmentModuleInput, FamilyModuleFragment, FrameOfReferenceModuleInput, ImageModuleInput,
    PatientModuleInput, RescalePlan, SeriesModuleInput, StudyModuleInput,
};
use crate::composition::{
    AttributeAddress, AttributeItem, AttributeOperation, AttributeValue, DicomVr, PrimitiveValue,
};
use crate::corpus_plan::OutputRelativePath;
use crate::native_pixel::{
    ByteOrder, NativePixelRequest, PhotometricInterpretation, PixelDataVr, PixelShape,
    StoredValueType,
};
use crate::{DeterministicUidInput, UidRole, deterministic_uid};

const PLAN_PROVIDER: &str = "native.classic_plan";
const CONTENT_PROVIDER: &str = "content.native_pixels";
const ALGORITHM_PROVIDER: &str = "algorithm.classic_nuclear";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClassicNuclearProviderParameters {
    pub patient_name: String,
    pub patient_id: String,
    pub patient_birth_date: String,
    pub patient_sex: String,
    pub study_date: String,
    pub study_time: String,
    pub study_id: String,
    pub accession_number: String,
    pub referring_physician_name: String,
    pub modality: String,
    pub series_number: String,
    #[serde(default)]
    pub series_date: Option<String>,
    #[serde(default)]
    pub series_time: Option<String>,
    #[serde(default)]
    pub body_part_examined: Option<String>,
    pub manufacturer: String,
    pub software_versions: String,
    pub acquisition_number: String,
    pub acquisition_date: String,
    pub acquisition_time: String,
    pub instance_number: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClassicNuclearPixels {
    pub rows: u16,
    pub columns: u16,
    pub frames: u32,
    pub stored_value_type: String,
    pub stored_values: Vec<i64>,
    pub pixel_min: i64,
    pub pixel_max: i64,
    pub frame_sha256: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "family", rename_all = "snake_case", deny_unknown_fields)]
pub enum ClassicNuclearArtifactParameters {
    UltrasoundSingleFrame {
        pixels: ClassicNuclearPixels,
        image_type: Vec<String>,
        lossy_image_compression: String,
        ultrasound_color_data_present: u16,
    },
    UltrasoundMultiframe {
        pixels: ClassicNuclearPixels,
        image_type: Vec<String>,
        frame_increment_pointer: String,
        frame_time_ms: u32,
        frame_relative_times_ms: Vec<u32>,
        payload_sha256: String,
        lossy_image_compression: String,
        color_data_present: bool,
        spatially_related_frames: bool,
        region_calibrated: bool,
    },
    NuclearMedicine {
        pixels: ClassicNuclearPixels,
        image_type: Vec<String>,
        pixel_spacing: [String; 2],
        energy_window_vector: Vec<u16>,
        detector_vector: Vec<u16>,
        energy_windows: Vec<NmEnergyWindow>,
        detectors: Vec<NmDetector>,
        actual_frame_duration_ms: u32,
        counts_accumulated: u32,
    },
    Pet {
        pixels: ClassicNuclearPixels,
        image_type: Vec<String>,
        units: String,
        counts_source: String,
        series_type: Vec<String>,
        number_of_slices: u16,
        corrected_image: Vec<String>,
        decay_correction: String,
        dose_calibration_factor: String,
        frame_reference_time_ms: String,
        actual_frame_duration_ms: String,
        image_index: u16,
        pixel_spacing: [String; 2],
        image_orientation_patient: [String; 6],
        image_position_patient: [String; 3],
        slice_thickness: String,
        rescale_intercept: String,
        rescale_slope: String,
        expected_activity_bqml: Vec<String>,
    },
}

impl ClassicNuclearArtifactParameters {
    fn pixels(&self) -> &ClassicNuclearPixels {
        match self {
            Self::UltrasoundSingleFrame { pixels, .. }
            | Self::UltrasoundMultiframe { pixels, .. }
            | Self::NuclearMedicine { pixels, .. }
            | Self::Pet { pixels, .. } => pixels,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NmEnergyWindow {
    pub index: u16,
    pub name: String,
    pub lower_limit_kev: String,
    pub upper_limit_kev: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NmDetector {
    pub index: u16,
    pub collimator_type: String,
    pub focal_distance_mm: String,
    pub start_angle_degrees: String,
    pub image_orientation_patient: [String; 6],
    pub image_position_patient: [String; 3],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Family {
    UltrasoundSingle,
    UltrasoundMultiframe,
    NuclearMedicine,
    Pet,
}

impl Family {
    fn template(self) -> (&'static str, &'static str) {
        match self {
            Self::UltrasoundSingle => (
                "classic/ultrasound/single-frame",
                "1.2.840.10008.5.1.4.1.1.6.1",
            ),
            Self::UltrasoundMultiframe => (
                "classic/ultrasound/multiframe",
                "1.2.840.10008.5.1.4.1.1.3.1",
            ),
            Self::NuclearMedicine => ("classic/nuclear-medicine", "1.2.840.10008.5.1.4.1.1.20"),
            Self::Pet => ("classic/pet", "1.2.840.10008.5.1.4.1.1.128"),
        }
    }
}

#[derive(Debug, Default, Clone, Copy)]
struct NuclearFamilyProvider;

impl ClassicFamilyProvider<&ClassicNuclearArtifactParameters> for NuclearFamilyProvider {
    const PROVIDER_ID: &'static str = ALGORITHM_PROVIDER;

    fn plan_family(
        &self,
        parameters: &ClassicNuclearArtifactParameters,
    ) -> Result<Vec<FamilyModuleFragment>, ClassicPlanError> {
        let mut operations = vec![string("0008,001C", DicomVr::CS, "YES")];
        match parameters {
            ClassicNuclearArtifactParameters::UltrasoundSingleFrame {
                image_type,
                lossy_image_compression,
                ultrasound_color_data_present,
                ..
            } => {
                operations.extend([
                    strings("0008,0008", DicomVr::CS, image_type.clone()),
                    string("0028,2110", DicomVr::CS, lossy_image_compression.clone()),
                    unsigned("0028,0014", u64::from(*ultrasound_color_data_present)),
                ]);
            }
            ClassicNuclearArtifactParameters::UltrasoundMultiframe {
                image_type,
                frame_increment_pointer,
                frame_time_ms,
                lossy_image_compression,
                color_data_present,
                ..
            } => {
                operations.extend([
                    strings("0008,0008", DicomVr::CS, image_type.clone()),
                    tag_value("0028,0009", frame_increment_pointer)?,
                    string("0018,1063", DicomVr::DS, frame_time_ms.to_string()),
                    string("0028,2110", DicomVr::CS, lossy_image_compression.clone()),
                    unsigned("0028,0014", u64::from(*color_data_present)),
                ]);
            }
            ClassicNuclearArtifactParameters::NuclearMedicine {
                image_type,
                pixel_spacing,
                energy_window_vector,
                detector_vector,
                energy_windows,
                detectors,
                actual_frame_duration_ms,
                counts_accumulated,
                ..
            } => {
                operations.extend([
                    strings("0008,0008", DicomVr::CS, image_type.clone()),
                    strings("0028,0030", DicomVr::DS, pixel_spacing.to_vec()),
                    tags_value("0028,0009", &["0054,0010", "0054,0020"]),
                    unsigneds("0054,0010", energy_window_vector),
                    unsigned("0054,0011", energy_windows.len() as u64),
                    unsigneds("0054,0020", detector_vector),
                    unsigned("0054,0021", detectors.len() as u64),
                    sequence("0054,0012", energy_window_items(energy_windows)),
                    sequence("0054,0016", vec![]),
                    sequence("0054,0022", detector_items(detectors)),
                    sequence("0054,0410", vec![]),
                    sequence("0054,0414", vec![]),
                    string(
                        "0018,1242",
                        DicomVr::IS,
                        actual_frame_duration_ms.to_string(),
                    ),
                    string("0018,0070", DicomVr::IS, counts_accumulated.to_string()),
                ]);
            }
            ClassicNuclearArtifactParameters::Pet {
                image_type,
                units,
                counts_source,
                series_type,
                number_of_slices,
                corrected_image,
                decay_correction,
                dose_calibration_factor,
                frame_reference_time_ms,
                actual_frame_duration_ms,
                image_index,
                pixel_spacing,
                image_orientation_patient,
                image_position_patient,
                slice_thickness,
                ..
            } => {
                operations.extend([
                    strings("0008,0008", DicomVr::CS, image_type.clone()),
                    string("0054,1001", DicomVr::CS, units.clone()),
                    string("0054,1002", DicomVr::CS, counts_source.clone()),
                    strings("0054,1000", DicomVr::CS, series_type.clone()),
                    unsigned("0054,0081", u64::from(*number_of_slices)),
                    strings("0028,0051", DicomVr::CS, corrected_image.clone()),
                    string("0054,1102", DicomVr::CS, decay_correction.clone()),
                    string("0018,1181", DicomVr::CS, "NONE"),
                    sequence("0054,0016", vec![]),
                    sequence("0054,0410", vec![]),
                    sequence("0054,0414", vec![]),
                    string("0028,2110", DicomVr::CS, "00"),
                    strings("0028,0030", DicomVr::DS, pixel_spacing.to_vec()),
                    strings("0020,0037", DicomVr::DS, image_orientation_patient.to_vec()),
                    strings("0020,0032", DicomVr::DS, image_position_patient.to_vec()),
                    string("0018,0050", DicomVr::DS, slice_thickness.clone()),
                    string("0054,1300", DicomVr::DS, frame_reference_time_ms.clone()),
                    unsigned("0054,1330", u64::from(*image_index)),
                    string("0018,1242", DicomVr::IS, actual_frame_duration_ms.clone()),
                    string("0054,1322", DicomVr::DS, dose_calibration_factor.clone()),
                ]);
            }
        }
        Ok(vec![FamilyModuleFragment::new(
            Self::PROVIDER_ID,
            "modality_image",
            operations,
        )?])
    }
}

/// Inspect only the qualified native single-frame US tuple, independent of names.
pub(crate) fn inspect_us_capability(
    recipe: &CaseRecipe,
) -> Result<Option<ClassicNuclearArtifactParameters>, ClassicNuclearPlanError> {
    let Some(dicom) = &recipe.dicom else {
        return Ok(None);
    };
    if !dicom.artifacts.iter().any(|a| {
        a.template
            .as_ref()
            .is_some_and(|t| t.template_id == "classic/ultrasound/single-frame")
            || a.parameters.get("family").and_then(Value::as_str) == Some("ultrasound_single_frame")
    }) {
        return Ok(None);
    }
    if dicom.artifacts.len() != 1 {
        return Err(contract("US requires one artifact"));
    }
    let a = &dicom.artifacts[0];
    if recipe.recipe_id == "us_mono2_u8_rle_lossless"
        && recipe.binding.case_id == "classic/us/mono2_u8_rle_lossless"
        && recipe.planning_order == Some(404)
        && a.encoding.transfer_syntax_uid == "1.2.840.10008.1.2.5"
    {
        return Ok(None);
    }
    let has = |v: &[String], s: &str| v.len() == 1 && v[0] == s;
    if recipe.kind != super::RecipeKind::Dicom
        || recipe.plan_provider_id != PLAN_PROVIDER
        || recipe.planning_order.is_none()
        || recipe.projection_order.is_none()
        || recipe.mutation.is_some()
        || recipe.qualification.is_some()
        || !recipe.dependencies.is_empty()
        || !has(&recipe.validation_rule_ids, "validation.shared")
        || !has(&recipe.projection_rule_ids, "projection.curated")
        || a.logical_id != "instance"
        || a.order != 0
        || a.output.role != "primary"
        || a.output.path.is_none()
        || a.output.provider_derived == Some(true)
        || a.public_profile_membership.is_some()
        || a.template.as_ref().is_none_or(|t| {
            t.template_id != "classic/ultrasound/single-frame" || t.template_version != "1.0.0"
        })
        || a.content.provider_id != CONTENT_PROVIDER
        || !a.content.parameters.is_empty()
        || a.algorithm_provider_id.as_deref() != Some(ALGORITHM_PROVIDER)
        || a.classic_projection.as_ref().is_none_or(|p| {
            p.family != super::ClassicProjectionFamily::Nuclear
                || p.mr.is_some()
                || p.icc.is_some()
                || p.semantic_labels.is_some()
                || !p.standards_evidence_append.is_empty()
                || p.include_implementation_version_name
        })
        || !a.attribute_operations.is_empty()
        || a.secondary_capture.is_some()
        || a.metadata_sc.is_some()
        || a.nonsquare_geometry.is_some()
        || !has(&a.validation_rule_ids, "validation.shared")
        || !has(&a.projection_rule_ids, "projection.curated")
        || a.encoding.transfer_syntax_uid != "1.2.840.10008.1.2.1"
        || a.encoding.non_template_encoding_provider_id.is_some()
        || a.encoding.fragments_per_frame.is_some()
        || a.encoding.sequence_length_policy != "default"
        || a.encoding.item_length_policy != "default"
        || a.encoding.offset_table_policy != "none"
        || a.encoding.fragmentation_policy != "native"
        || a.encoding.preamble_policy.as_deref() != Some("zero_filled")
        || a.encoding.file_meta_policy.as_deref() != Some("standard")
    {
        return Err(contract("complete native US tuple required"));
    }
    OutputRelativePath::new(a.output.path.clone().expect("explicit US output"))?;
    let provider: ClassicNuclearProviderParameters = decode(
        Value::Object(recipe.provider_parameters.clone()),
        "provider_parameters",
    )?;
    if provider.modality != "US"
        || provider.series_date.is_some()
        || provider.series_time.is_some()
        || provider.body_part_examined.is_some()
    {
        return Err(contract("bounded US provider contract"));
    }
    let parameters: ClassicNuclearArtifactParameters =
        decode(Value::Object(a.parameters.clone()), "artifact parameters")?;
    let ClassicNuclearArtifactParameters::UltrasoundSingleFrame {
        pixels,
        image_type,
        lossy_image_compression,
        ultrasound_color_data_present,
    } = &parameters
    else {
        return Err(contract("US single-frame parameters required"));
    };
    let count = usize::from(pixels.rows)
        .checked_mul(usize::from(pixels.columns))
        .ok_or_else(|| contract("US dimensions overflow"))?;
    if pixels.rows == 0
        || pixels.columns == 0
        || pixels.frames != 1
        || pixels.stored_value_type != "u8"
        || count != pixels.stored_values.len()
        || pixels.stored_values.iter().any(|v| !(0..=255).contains(v))
        || pixels.stored_values.iter().min().copied() != Some(pixels.pixel_min)
        || pixels.stored_values.iter().max().copied() != Some(pixels.pixel_max)
        || pixels.frame_sha256.len() != 1
        || image_type != &["ORIGINAL", "PRIMARY"]
        || lossy_image_compression != "00"
        || *ultrasound_color_data_present != 0
    {
        return Err(contract("bounded US pixels and semantics"));
    }
    let bytes: Vec<u8> = pixels.stored_values.iter().map(|v| *v as u8).collect();
    if crate::sha256_hex(&bytes) != pixels.frame_sha256[0] {
        return Err(contract("US frame hash"));
    }
    Ok(Some(parameters))
}

pub fn plan_nuclear_recipe(
    recipe: &CaseRecipe,
    standards_lock_sha256: &str,
    seed: u64,
) -> Result<Option<Vec<ClassicInstanceRequest>>, ClassicNuclearPlanError> {
    let native_us = inspect_us_capability(recipe)?.is_some();
    let Some(family) = (if native_us {
        Some(Family::UltrasoundSingle)
    } else {
        owned_family(&recipe.binding.case_id)
    }) else {
        return Ok(None);
    };
    if recipe.plan_provider_id != PLAN_PROVIDER {
        return Err(contract(
            "owned nuclear recipe requires native.classic_plan",
        ));
    }
    if !native_us
        && !(400..=404).contains(
            &recipe
                .planning_order
                .ok_or_else(|| contract("owned nuclear recipe requires planning_order"))?,
        )
    {
        return Err(contract(
            "owned nuclear planning_order is outside 400..=404",
        ));
    }
    let provider: ClassicNuclearProviderParameters = decode(
        Value::Object(recipe.provider_parameters.clone()),
        "provider_parameters",
    )?;
    let artifact = recipe
        .dicom
        .as_ref()
        .and_then(|dicom| (dicom.artifacts.len() == 1).then(|| &dicom.artifacts[0]))
        .ok_or_else(|| contract("owned nuclear recipe requires exactly one artifact"))?;
    validate_artifact(artifact, family)?;
    let parameters: ClassicNuclearArtifactParameters = decode(
        Value::Object(artifact.parameters.clone()),
        "artifact parameters",
    )?;
    if family_of_parameters(&parameters) != family {
        return Err(contract("case binding and typed nuclear family differ"));
    }
    validate_parameters(&parameters)?;

    let study_uid = uid(recipe, standards_lock_sha256, seed, UidRole::StudyInstance);
    let series_uid = uid(recipe, standards_lock_sha256, seed, UidRole::SeriesInstance);
    let sop_uid = uid(recipe, standards_lock_sha256, seed, UidRole::SopInstance);
    let implementation_uid = implementation_uid(standards_lock_sha256);
    let frame_of_reference = (family == Family::Pet).then(|| FrameOfReferenceModuleInput {
        frame_of_reference_uid: uid(
            recipe,
            standards_lock_sha256,
            seed,
            UidRole::FrameOfReference,
        ),
        position_reference_indicator: ElementPresence::Empty,
    });
    let common = CommonModuleRequest {
        patient: PatientModuleInput {
            specific_character_set: ElementPresence::Omitted,
            patient_name: ElementPresence::Value(provider.patient_name.clone()),
            patient_id: ElementPresence::Value(provider.patient_id.clone()),
            patient_birth_date: ElementPresence::Value(provider.patient_birth_date.clone()),
            patient_sex: ElementPresence::Value(provider.patient_sex.clone()),
        },
        study: StudyModuleInput {
            study_instance_uid: study_uid,
            study_date: ElementPresence::Value(provider.study_date.clone()),
            study_time: ElementPresence::Value(provider.study_time.clone()),
            accession_number: empty_or_value(&provider.accession_number),
            referring_physician_name: empty_or_value(&provider.referring_physician_name),
            study_id: ElementPresence::Value(provider.study_id.clone()),
        },
        series: SeriesModuleInput {
            modality: provider.modality.clone(),
            series_instance_uid: series_uid,
            series_number: ElementPresence::Value(provider.series_number.clone()),
            series_date: optional_value(&provider.series_date),
            series_time: optional_value(&provider.series_time),
        },
        frame_of_reference,
        equipment: EquipmentModuleInput {
            manufacturer: ElementPresence::Value(provider.manufacturer.clone()),
            manufacturer_model_name: ElementPresence::Value(recipe.recipe_id.clone()),
            software_versions: ElementPresence::Value(provider.software_versions.clone()),
        },
        image: ImageModuleInput {
            instance_number: ElementPresence::Value(provider.instance_number.clone()),
            patient_orientation: ElementPresence::Empty,
            content_date: ElementPresence::Value(provider.acquisition_date.clone()),
            content_time: ElementPresence::Value(provider.acquisition_time.clone()),
        },
    };
    let mut family_modules = NuclearFamilyProvider.plan_family(&parameters)?;
    family_modules.push(FamilyModuleFragment::new(
        ALGORITHM_PROVIDER,
        "acquisition_and_body_part",
        [
            provider
                .body_part_examined
                .as_ref()
                .map(|value| string("0018,0015", DicomVr::CS, value.clone())),
            Some(string(
                "0020,0012",
                DicomVr::IS,
                provider.acquisition_number.clone(),
            )),
            Some(string(
                "0008,0022",
                DicomVr::DA,
                provider.acquisition_date.clone(),
            )),
            Some(string(
                "0008,0032",
                DicomVr::TM,
                provider.acquisition_time.clone(),
            )),
        ]
        .into_iter()
        .flatten()
        .collect(),
    )?);

    let pixels = parameters.pixels();
    let stored = match pixels.stored_value_type.as_str() {
        "u8" => StoredValueType::U8,
        "u16" => StoredValueType::U16,
        _ => return Err(contract("nuclear pixels require u8 or u16 stored values")),
    };
    let bits = stored.bits_allocated();
    let rescale = match &parameters {
        ClassicNuclearArtifactParameters::Pet {
            rescale_intercept,
            rescale_slope,
            ..
        } => Some(RescalePlan {
            intercept: rescale_intercept.clone(),
            slope: rescale_slope.clone(),
            rescale_type: ElementPresence::Omitted,
        }),
        _ => None,
    };
    Ok(Some(vec![ClassicInstanceRequest {
        logical_id: artifact.logical_id.clone(),
        order: u64::from(artifact.order),
        output_relative_path: OutputRelativePath::new(
            artifact
                .output
                .path
                .clone()
                .ok_or_else(|| contract("nuclear output path must be explicit"))?,
        )?,
        dependencies: vec![],
        common,
        sop_class_uid: family.template().1.into(),
        sop_instance_uid: sop_uid,
        implementation_class_uid: implementation_uid,
        family: family_modules,
        pixels: ClassicPixelRequest {
            slot: CLASSIC_PIXEL_SLOT.into(),
            pixels: NativePixelRequest {
                shape: PixelShape {
                    rows: u32::from(pixels.rows),
                    columns: u32::from(pixels.columns),
                    frames: pixels.frames,
                    samples_per_pixel: 1,
                    photometric_interpretation: PhotometricInterpretation::Monochrome2,
                    bits_allocated: bits,
                    bits_stored: bits,
                    high_bit: bits - 1,
                    pixel_representation: 0,
                    stored_value_type: stored,
                    byte_order: ByteOrder::Little,
                    pixel_data_vr: if bits == 8 {
                        PixelDataVr::Ob
                    } else {
                        PixelDataVr::Ow
                    },
                    color: None,
                },
                stored_values: pixels.stored_values.clone(),
                declared_pixel_min: pixels.pixel_min,
                declared_pixel_max: pixels.pixel_max,
                expected_frame_sha256: pixels.frame_sha256.clone(),
                signed_stored_bits: Default::default(),
                padding: None,
                palette: None,
            },
            rescale,
            window: None,
        },
    }]))
}

fn validate_artifact(
    artifact: &super::PlannedArtifactRecipe,
    family: Family,
) -> Result<(), ClassicNuclearPlanError> {
    let template = artifact
        .template
        .as_ref()
        .ok_or_else(|| contract("nuclear artifact requires template"))?;
    if template.template_id != family.template().0 || template.template_version != "1.0.0" {
        return Err(contract("nuclear template differs from typed family"));
    }
    if artifact.order != 0
        || artifact.content.provider_id != CONTENT_PROVIDER
        || !artifact.content.parameters.is_empty()
        || artifact.algorithm_provider_id.as_deref() != Some(ALGORITHM_PROVIDER)
        || artifact.output.provider_derived == Some(true)
        || artifact.output.path.is_none()
        || !artifact.attribute_operations.is_empty()
        || artifact.secondary_capture.is_some()
        || artifact.metadata_sc.is_some()
        || artifact.nonsquare_geometry.is_some()
    {
        return Err(contract(
            "nuclear artifact contract is not fully data-first",
        ));
    }
    Ok(())
}

fn validate_parameters(
    parameters: &ClassicNuclearArtifactParameters,
) -> Result<(), ClassicNuclearPlanError> {
    let pixels = parameters.pixels();
    let expected_values = usize::from(pixels.rows)
        .checked_mul(usize::from(pixels.columns))
        .and_then(|value| value.checked_mul(pixels.frames as usize))
        .ok_or_else(|| contract("nuclear pixel dimensions overflow"))?;
    if pixels.stored_values.len() != expected_values
        || pixels.frame_sha256.len() != pixels.frames as usize
    {
        return Err(contract("nuclear pixel dimensions/hashes are inconsistent"));
    }
    match parameters {
        ClassicNuclearArtifactParameters::UltrasoundMultiframe {
            frame_increment_pointer,
            frame_time_ms,
            frame_relative_times_ms,
            payload_sha256,
            ..
        } => {
            if frame_increment_pointer != "0018,1063"
                || frame_relative_times_ms.len() != pixels.frames as usize
                || frame_relative_times_ms
                    .iter()
                    .enumerate()
                    .any(|(index, value)| *value != index as u32 * *frame_time_ms)
                || payload_sha256.len() != 64
            {
                return Err(contract("invalid ultrasound multiframe timing contract"));
            }
        }
        ClassicNuclearArtifactParameters::NuclearMedicine {
            energy_window_vector,
            detector_vector,
            energy_windows,
            detectors,
            counts_accumulated,
            ..
        } => {
            if energy_window_vector.len() != pixels.frames as usize
                || detector_vector.len() != pixels.frames as usize
                || energy_windows.is_empty()
                || detectors.is_empty()
                || pixels.stored_values.iter().sum::<i64>() != i64::from(*counts_accumulated)
            {
                return Err(contract("invalid NM dimensions or accumulated counts"));
            }
        }
        ClassicNuclearArtifactParameters::Pet {
            rescale_intercept,
            rescale_slope,
            expected_activity_bqml,
            ..
        } => {
            let intercept = rescale_intercept
                .parse::<f64>()
                .map_err(|_| contract("invalid PET rescale intercept"))?;
            let slope = rescale_slope
                .parse::<f64>()
                .map_err(|_| contract("invalid PET rescale slope"))?;
            if expected_activity_bqml.len() != pixels.stored_values.len()
                || expected_activity_bqml
                    .iter()
                    .zip(&pixels.stored_values)
                    .any(|(expected, stored)| {
                        expected.parse::<f64>().ok() != Some(*stored as f64 * slope + intercept)
                    })
            {
                return Err(contract("PET activity mapping differs from rescale"));
            }
        }
        ClassicNuclearArtifactParameters::UltrasoundSingleFrame { .. } => {}
    }
    Ok(())
}

fn energy_window_items(windows: &[NmEnergyWindow]) -> Vec<AttributeItem> {
    windows
        .iter()
        .map(|window| AttributeItem {
            attributes: vec![
                string("0054,0018", DicomVr::SH, window.name.clone()),
                sequence(
                    "0054,0013",
                    vec![AttributeItem {
                        attributes: vec![
                            string("0054,0014", DicomVr::DS, window.lower_limit_kev.clone()),
                            string("0054,0015", DicomVr::DS, window.upper_limit_kev.clone()),
                        ],
                    }],
                ),
            ],
        })
        .collect()
}

fn detector_items(detectors: &[NmDetector]) -> Vec<AttributeItem> {
    detectors
        .iter()
        .map(|detector| AttributeItem {
            attributes: vec![
                string("0018,1181", DicomVr::CS, detector.collimator_type.clone()),
                string("0018,1182", DicomVr::IS, detector.focal_distance_mm.clone()),
                string(
                    "0054,0200",
                    DicomVr::DS,
                    detector.start_angle_degrees.clone(),
                ),
                strings(
                    "0020,0037",
                    DicomVr::DS,
                    detector.image_orientation_patient.to_vec(),
                ),
                strings(
                    "0020,0032",
                    DicomVr::DS,
                    detector.image_position_patient.to_vec(),
                ),
            ],
        })
        .collect()
}

fn family_of_parameters(parameters: &ClassicNuclearArtifactParameters) -> Family {
    match parameters {
        ClassicNuclearArtifactParameters::UltrasoundSingleFrame { .. } => Family::UltrasoundSingle,
        ClassicNuclearArtifactParameters::UltrasoundMultiframe { .. } => {
            Family::UltrasoundMultiframe
        }
        ClassicNuclearArtifactParameters::NuclearMedicine { .. } => Family::NuclearMedicine,
        ClassicNuclearArtifactParameters::Pet { .. } => Family::Pet,
    }
}

fn owned_family(case_id: &str) -> Option<Family> {
    match case_id {
        "classic/us/mono2_u8_explicit_le" | "classic/us/mono2_u8_rle_lossless" => {
            Some(Family::UltrasoundSingle)
        }
        "classic/us/multiframe_explicit_le" => Some(Family::UltrasoundMultiframe),
        "classic/nm/multiframe_explicit_le" => Some(Family::NuclearMedicine),
        "classic/pet/rescaled_activity_explicit_le" => Some(Family::Pet),
        _ => None,
    }
}

fn uid(recipe: &CaseRecipe, lock: &str, seed: u64, role: UidRole) -> String {
    deterministic_uid(&DeterministicUidInput {
        standards_lock_sha256: lock,
        case_id: &recipe.binding.case_id,
        recipe_version: &recipe.recipe_version,
        run_seed: seed,
        file_index: 0,
        frame_index: None,
        referenced_object_index: None,
        role,
    })
}

fn implementation_uid(lock: &str) -> String {
    deterministic_uid(&DeterministicUidInput {
        standards_lock_sha256: lock,
        case_id: "dicom-test-suite/implementation",
        recipe_version: crate::BYTE_STABLE_OUTPUT_VERSION,
        run_seed: 0,
        file_index: 0,
        frame_index: None,
        referenced_object_index: None,
        role: UidRole::ImplementationClass,
    })
}

fn decode<T: for<'de> Deserialize<'de>>(
    value: Value,
    field: &'static str,
) -> Result<T, ClassicNuclearPlanError> {
    serde_json::from_value(value).map_err(|error| ClassicNuclearPlanError::Parameters {
        field,
        message: error.to_string(),
    })
}

fn optional_value(value: &Option<String>) -> ElementPresence<String> {
    value
        .as_ref()
        .map(|value| ElementPresence::Value(value.clone()))
        .unwrap_or(ElementPresence::Omitted)
}

fn empty_or_value(value: &str) -> ElementPresence<String> {
    if value.is_empty() {
        ElementPresence::Empty
    } else {
        ElementPresence::Value(value.into())
    }
}

fn address(tag: &str) -> AttributeAddress {
    AttributeAddress::from_normalized_tag(tag).expect("nuclear provider tag is valid")
}

fn string(tag: &str, vr: DicomVr, value: impl Into<String>) -> AttributeOperation {
    AttributeOperation::Set {
        address: address(tag),
        vr,
        value: AttributeValue::Primitive(PrimitiveValue::String(value.into())),
    }
}

fn strings(tag: &str, vr: DicomVr, values: Vec<String>) -> AttributeOperation {
    AttributeOperation::Set {
        address: address(tag),
        vr,
        value: AttributeValue::Multi(values.into_iter().map(PrimitiveValue::String).collect()),
    }
}

fn unsigned(tag: &str, value: u64) -> AttributeOperation {
    AttributeOperation::Set {
        address: address(tag),
        vr: DicomVr::US,
        value: AttributeValue::Primitive(PrimitiveValue::Unsigned(value)),
    }
}

fn unsigneds(tag: &str, values: &[u16]) -> AttributeOperation {
    AttributeOperation::Set {
        address: address(tag),
        vr: DicomVr::US,
        value: AttributeValue::Multi(
            values
                .iter()
                .map(|value| PrimitiveValue::Unsigned(u64::from(*value)))
                .collect(),
        ),
    }
}

fn tag_value(tag: &str, value: &str) -> Result<AttributeOperation, ClassicPlanError> {
    Ok(AttributeOperation::Set {
        address: address(tag),
        vr: DicomVr::AT,
        value: AttributeValue::Primitive(PrimitiveValue::Tag(
            AttributeAddress::from_normalized_tag(value)?,
        )),
    })
}

fn tags_value(tag: &str, values: &[&str]) -> AttributeOperation {
    AttributeOperation::Set {
        address: address(tag),
        vr: DicomVr::AT,
        value: AttributeValue::Multi(
            values
                .iter()
                .map(|value| {
                    PrimitiveValue::Tag(
                        AttributeAddress::from_normalized_tag(value)
                            .expect("NM frame pointer tag is valid"),
                    )
                })
                .collect(),
        ),
    }
}

fn sequence(tag: &str, items: Vec<AttributeItem>) -> AttributeOperation {
    AttributeOperation::Set {
        address: address(tag),
        vr: DicomVr::SQ,
        value: AttributeValue::Sequence(items),
    }
}

fn contract(message: impl Into<String>) -> ClassicNuclearPlanError {
    ClassicNuclearPlanError::Contract(message.into())
}

#[derive(Debug)]
pub enum ClassicNuclearPlanError {
    Contract(String),
    Parameters {
        field: &'static str,
        message: String,
    },
    Classic(ClassicPlanError),
    OutputPath(crate::corpus_plan::CorpusPlanError),
}

impl From<ClassicPlanError> for ClassicNuclearPlanError {
    fn from(error: ClassicPlanError) -> Self {
        Self::Classic(error)
    }
}

impl From<crate::corpus_plan::CorpusPlanError> for ClassicNuclearPlanError {
    fn from(error: crate::corpus_plan::CorpusPlanError) -> Self {
        Self::OutputPath(error)
    }
}

impl fmt::Display for ClassicNuclearPlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for ClassicNuclearPlanError {}
