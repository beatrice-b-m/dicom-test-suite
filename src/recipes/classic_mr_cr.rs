//! Plan-only MR and computed-radiography recipe providers.

use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::composition::{
    AttributeAddress, AttributeItem, AttributeOperation, AttributeValue, DicomVr, PrimitiveValue,
};
use crate::corpus_plan::OutputRelativePath;
use crate::native_pixel::{
    ByteOrder, NativePixelRequest, PhotometricInterpretation, PixelDataVr, PixelShape,
    StoredValueType,
};
use crate::uid::{DeterministicUidInput, UidRole, deterministic_uid};

use super::{
    CLASSIC_PIXEL_SLOT, CaseRecipe, ClassicInstanceRequest, ClassicMrProjection,
    ClassicPixelRequest, ClassicProjectionFamily, CommonModuleRequest, ElementPresence,
    EquipmentModuleInput, FamilyModuleFragment, FrameOfReferenceModuleInput, ImageModuleInput,
    PatientModuleInput, SeriesModuleInput, StudyModuleInput,
};

const PROVIDER_ID: &str = "native.classic_plan";
const CONTENT_PROVIDER_ID: &str = "content.native_pixels";
const MR_ALGORITHM_ID: &str = "algorithm.classic_mr_cr";
const CR_ALGORITHM_ID: &str = "algorithm.classic_mr_cr";
const MR_SOP_CLASS_UID: &str = "1.2.840.10008.5.1.4.1.1.4";
const CR_SOP_CLASS_UID: &str = "1.2.840.10008.5.1.4.1.1.1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MrProviderParameters {
    pub patient: MrPatientParameters,
    pub study: MrStudyParameters,
    pub equipment: MrEquipmentParameters,
    pub acquisition_date: String,
    pub acquisition_time: String,
    pub image_type: Vec<String>,
    pub acquisition_number: String,
    pub series_number: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MrPatientParameters {
    pub patient_name: String,
    pub patient_id: String,
    pub patient_birth_date: String,
    pub patient_sex: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MrStudyParameters {
    pub study_date: String,
    pub study_time: String,
    pub accession_number: String,
    pub referring_physician_name: String,
    pub study_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MrEquipmentParameters {
    pub manufacturer: String,
    pub manufacturer_model_name: String,
    pub software_versions: String,
}

/// Fully decoded MR capability selected without consulting caller identities.
#[derive(Debug, Clone)]
pub(crate) struct MrCapability {
    pub provider: MrProviderParameters,
    pub mr: ClassicMrProjection,
    pub artifacts: Vec<MrArtifactParameters>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MrArtifactParameters {
    pub rows: u32,
    pub columns: u32,
    pub pixel_spacing: Vec<String>,
    pub image_orientation_patient: Vec<String>,
    pub image_position_patient: Vec<String>,
    pub slice_thickness: String,
    pub spacing_between_slices: String,
    pub slice_location: String,
    pub position_along_normal: f64,
    pub instance_number: String,
    pub stored_values: Vec<i64>,
    pub pixel_min: i64,
    pub pixel_max: i64,
    pub frame_sha256: String,
}

/// Inspect the complete native MR tuple without interpreting case names,
/// recipe names, paths, or planning order as provider selectors.
pub(crate) fn inspect_mr_capability(
    recipe: &CaseRecipe,
) -> Result<Option<MrCapability>, ClassicMrCrPlanError> {
    let Some(dicom) = recipe.dicom.as_ref() else {
        return Ok(None);
    };
    let declared = dicom.artifacts.iter().any(|artifact| {
        artifact
            .template
            .as_ref()
            .is_some_and(|template| template.template_id == "classic/mr")
            || artifact
                .classic_projection
                .as_ref()
                .is_some_and(|projection| {
                    projection.family == ClassicProjectionFamily::MrCr && projection.mr.is_some()
                })
    });
    if !declared {
        return Ok(None);
    }

    let transitional = matches!(
        (recipe.recipe_id.as_str(), recipe.binding.case_id.as_str()),
        (
            "mr_multislice_oblique",
            "classic/mr/multislice_oblique_explicit_le"
        ) | (
            "mr_mono2_u16_rle_lossless",
            "classic/mr/mono2_u16_rle_lossless"
        )
    ) && recipe.provider_parameters.is_empty();
    let provider = if transitional {
        transitional_mr_provider(recipe)
    } else {
        parameters_value(Value::Object(recipe.provider_parameters.clone()))?
    };
    if recipe.plan_provider_id != PROVIDER_ID
        || recipe.kind != super::RecipeKind::Dicom
        || recipe.mutation.is_some()
        || recipe.qualification.is_some()
        || recipe.planning_order.is_none()
        || recipe.projection_order.is_none()
        || !recipe.dependencies.is_empty()
        || recipe.validation_rule_ids != ["validation.shared"]
        || recipe.projection_rule_ids != ["projection.curated"]
        || dicom.artifacts.is_empty()
        || dicom.artifacts.len() > 4096
        || provider.image_type.is_empty()
        || provider.image_type.len() > 16
        || provider.image_type.iter().any(String::is_empty)
        || provider.acquisition_number.is_empty()
        || provider.series_number.is_empty()
        || provider.patient.patient_name.is_empty()
        || provider.patient.patient_id.is_empty()
        || provider.patient.patient_birth_date.is_empty()
        || provider.patient.patient_sex.is_empty()
        || provider.study.study_date.is_empty()
        || provider.study.study_time.is_empty()
        || provider.study.study_id.is_empty()
        || provider.equipment.manufacturer.is_empty()
        || provider.equipment.manufacturer_model_name.is_empty()
        || provider.equipment.software_versions.is_empty()
        || provider.acquisition_date.is_empty()
        || provider.acquisition_time.is_empty()
    {
        return Err(ClassicMrCrPlanError::Contract("complete native MR tuple"));
    }

    let mut decoded = Vec::with_capacity(dicom.artifacts.len());
    let mut shared_mr: Option<ClassicMrProjection> = None;
    let mut logical_ids = std::collections::BTreeSet::new();
    let mut roles = std::collections::BTreeSet::new();
    let mut paths = std::collections::BTreeSet::new();
    for (expected_order, artifact) in dicom.artifacts.iter().enumerate() {
        let native_encoding = artifact.encoding.transfer_syntax_uid == "1.2.840.10008.1.2.1"
            && artifact.encoding.offset_table_policy == "none"
            && artifact.encoding.fragmentation_policy == "native"
            && artifact
                .encoding
                .non_template_encoding_provider_id
                .is_none();
        let transitional_rle = transitional
            && recipe.recipe_id == "mr_mono2_u16_rle_lossless"
            && artifact.encoding.transfer_syntax_uid == "1.2.840.10008.1.2.5"
            && artifact.encoding.offset_table_policy == "populated_basic"
            && artifact.encoding.fragmentation_policy == "one_per_frame"
            && artifact
                .encoding
                .non_template_encoding_provider_id
                .as_deref()
                == Some("encoding.native.rle_lossless");
        let projection = artifact.classic_projection.as_ref();
        let mr = projection.and_then(|projection| projection.mr.clone());
        let path = artifact.output.path.as_ref();
        if artifact.order as usize != expected_order
            || artifact.logical_id.is_empty()
            || !logical_ids.insert(artifact.logical_id.as_str())
            || artifact.output.role.is_empty()
            || !roles.insert(artifact.output.role.as_str())
            || artifact.output.provider_derived == Some(true)
            || path.is_none()
            || !paths.insert(path.expect("checked path"))
            || artifact.public_profile_membership.is_some()
            || artifact.template.as_ref().is_none_or(|template| {
                template.template_id != "classic/mr" || template.template_version != "1.0.0"
            })
            || artifact.content.provider_id != CONTENT_PROVIDER_ID
            || !artifact.content.parameters.is_empty()
            || artifact.algorithm_provider_id.as_deref() != Some(MR_ALGORITHM_ID)
            || projection.is_none_or(|projection| {
                projection.family != ClassicProjectionFamily::MrCr
                    || projection.mr.is_none()
                    || projection.icc.is_some()
                    || projection.semantic_labels.is_some()
                    || !projection.standards_evidence_append.is_empty()
                    || projection.include_implementation_version_name
            })
            || !artifact.attribute_operations.is_empty()
            || artifact.secondary_capture.is_some()
            || artifact.metadata_sc.is_some()
            || artifact.nonsquare_geometry.is_some()
            || artifact.validation_rule_ids != ["validation.shared"]
            || artifact.projection_rule_ids != ["projection.curated"]
            || artifact.encoding.sequence_length_policy != "default"
            || artifact.encoding.item_length_policy != "default"
            || artifact.encoding.fragments_per_frame.is_some()
            || artifact.encoding.preamble_policy.as_deref() != Some("zero_filled")
            || artifact.encoding.file_meta_policy.as_deref() != Some("standard")
            || (!native_encoding && !transitional_rle)
        {
            return Err(ClassicMrCrPlanError::Contract(
                "complete native MR artifact tuple",
            ));
        }
        OutputRelativePath::new(path.expect("checked path").clone())?;
        let mr = mr.expect("checked MR projection");
        if shared_mr.as_ref().is_some_and(|shared| shared != &mr) {
            return Err(ClassicMrCrPlanError::Contract(
                "consistent MR acquisition values",
            ));
        }
        shared_mr.get_or_insert(mr);
        let item: MrArtifactParameters = parameters(artifact)?;
        validate_mr_pixels(&item)?;
        decoded.push(item);
    }
    validate_mr_geometry(&decoded)?;
    let mr = shared_mr.expect("nonempty inspected artifacts");
    if mr.scanning_sequence.is_empty()
        || mr.sequence_variant.is_empty()
        || mr.mr_acquisition_type.is_empty()
        || mr.repetition_time.is_empty()
        || mr.echo_time.is_empty()
        || mr.echo_train_length.is_empty()
        || mr.magnetic_field_strength.is_empty()
    {
        return Err(ClassicMrCrPlanError::Contract(
            "nonempty MR acquisition values",
        ));
    }
    let finite_nonnegative = |value: &str| {
        value
            .parse::<f64>()
            .is_ok_and(|value| value.is_finite() && value >= 0.0)
    };
    if !finite_nonnegative(&mr.repetition_time)
        || !finite_nonnegative(&mr.echo_time)
        || !mr
            .magnetic_field_strength
            .parse::<f64>()
            .is_ok_and(|value| value.is_finite() && value > 0.0)
        || !mr
            .echo_train_length
            .parse::<u64>()
            .is_ok_and(|value| value > 0)
    {
        return Err(ClassicMrCrPlanError::Contract(
            "valid numeric MR acquisition values",
        ));
    }
    Ok(Some(MrCapability {
        provider,
        mr,
        artifacts: decoded,
    }))
}

fn transitional_mr_provider(recipe: &CaseRecipe) -> MrProviderParameters {
    MrProviderParameters {
        patient: MrPatientParameters {
            patient_name: "DTS^Synthetic^Patient001".into(),
            patient_id: "DTS-PATIENT-001".into(),
            patient_birth_date: "19700101".into(),
            patient_sex: "O".into(),
        },
        study: MrStudyParameters {
            study_date: "20260101".into(),
            study_time: "000000".into(),
            accession_number: String::new(),
            referring_physician_name: String::new(),
            study_id: "DTS-MR".into(),
        },
        equipment: MrEquipmentParameters {
            manufacturer: "dicom-test-suite".into(),
            manufacturer_model_name: recipe.recipe_id.clone(),
            software_versions: crate::BYTE_STABLE_OUTPUT_VERSION.into(),
        },
        acquisition_date: "20260101".into(),
        acquisition_time: "000000".into(),
        image_type: vec!["ORIGINAL".into(), "PRIMARY".into()],
        acquisition_number: "1".into(),
        series_number: "1".into(),
    }
}

fn parameters_value<T: for<'de> Deserialize<'de>>(value: Value) -> Result<T, ClassicMrCrPlanError> {
    serde_json::from_value(value).map_err(ClassicMrCrPlanError::Parameters)
}

fn validate_mr_pixels(item: &MrArtifactParameters) -> Result<(), ClassicMrCrPlanError> {
    let count = item
        .rows
        .checked_mul(item.columns)
        .and_then(|count| usize::try_from(count).ok())
        .ok_or(ClassicMrCrPlanError::Contract("MR pixel count"))?;
    if item.rows == 0
        || item.columns == 0
        || item.rows > u16::MAX.into()
        || item.columns > u16::MAX.into()
        || item.stored_values.len() != count
        || item
            .stored_values
            .iter()
            .any(|value| !(0..=u16::MAX.into()).contains(value))
        || item.stored_values.iter().copied().min() != Some(item.pixel_min)
        || item.stored_values.iter().copied().max() != Some(item.pixel_max)
    {
        return Err(ClassicMrCrPlanError::Contract("MR U16 pixel declaration"));
    }
    let bytes = item
        .stored_values
        .iter()
        .flat_map(|value| (*value as u16).to_le_bytes())
        .collect::<Vec<_>>();
    if crate::sha256_hex(&bytes) != item.frame_sha256 {
        return Err(ClassicMrCrPlanError::Contract("MR frame hash"));
    }
    Ok(())
}

fn validate_mr_geometry(items: &[MrArtifactParameters]) -> Result<(), ClassicMrCrPlanError> {
    let first = &items[0];
    let parse = |values: &[String]| -> Result<Vec<f64>, ClassicMrCrPlanError> {
        values
            .iter()
            .map(|value| {
                value
                    .parse::<f64>()
                    .map_err(|_| ClassicMrCrPlanError::Geometry)
            })
            .collect::<Result<Vec<_>, _>>()
            .and_then(|values| {
                values
                    .iter()
                    .all(|value| value.is_finite())
                    .then_some(values)
                    .ok_or(ClassicMrCrPlanError::Geometry)
            })
    };
    if first.pixel_spacing.len() != 2 || first.image_orientation_patient.len() != 6 {
        return Err(ClassicMrCrPlanError::Geometry);
    }
    let spacing = parse(&first.pixel_spacing)?;
    let orientation = parse(&first.image_orientation_patient)?;
    if spacing.iter().any(|value| *value <= 0.0) {
        return Err(ClassicMrCrPlanError::Geometry);
    }
    let row = &orientation[..3];
    let column = &orientation[3..];
    let dot = row.iter().zip(column).map(|(a, b)| a * b).sum::<f64>();
    let norm = |v: &[f64]| v.iter().map(|n| n * n).sum::<f64>().sqrt();
    let normal = [
        row[1] * column[2] - row[2] * column[1],
        row[2] * column[0] - row[0] * column[2],
        row[0] * column[1] - row[1] * column[0],
    ];
    if (norm(row) - 1.0).abs() > 1e-5
        || (norm(column) - 1.0).abs() > 1e-5
        || dot.abs() > 1e-5
        || (norm(&normal) - 1.0).abs() > 1e-5
    {
        return Err(ClassicMrCrPlanError::Geometry);
    }
    let expected_step = first
        .spacing_between_slices
        .parse::<f64>()
        .map_err(|_| ClassicMrCrPlanError::Geometry)?;
    if !expected_step.is_finite() || expected_step <= 0.0 {
        return Err(ClassicMrCrPlanError::Geometry);
    }
    let slice_thickness = first
        .slice_thickness
        .parse::<f64>()
        .map_err(|_| ClassicMrCrPlanError::Geometry)?;
    if !slice_thickness.is_finite() || slice_thickness <= 0.0 {
        return Err(ClassicMrCrPlanError::Geometry);
    }
    let mut instance_numbers = std::collections::BTreeSet::new();
    for (index, item) in items.iter().enumerate() {
        let instance_number = item
            .instance_number
            .parse::<u64>()
            .map_err(|_| ClassicMrCrPlanError::Geometry)?;
        if item.pixel_spacing != first.pixel_spacing
            || item.image_orientation_patient != first.image_orientation_patient
            || item.slice_thickness != first.slice_thickness
            || item.spacing_between_slices != first.spacing_between_slices
            || item.image_position_patient.len() != 3
            || instance_number == 0
            || !instance_numbers.insert(instance_number)
        {
            return Err(ClassicMrCrPlanError::Geometry);
        }
        let position = parse(&item.image_position_patient)?;
        let projected = position.iter().zip(normal).map(|(a, b)| a * b).sum::<f64>();
        let slice_location = item
            .slice_location
            .parse::<f64>()
            .map_err(|_| ClassicMrCrPlanError::Geometry)?;
        if !item.position_along_normal.is_finite()
            || !slice_location.is_finite()
            || (projected - item.position_along_normal).abs() > 1e-4
            || (slice_location - item.position_along_normal).abs() > 1e-4
            || (index > 0
                && ((item.position_along_normal - items[index - 1].position_along_normal).abs()
                    - expected_step)
                    .abs()
                    > 1e-4)
        {
            return Err(ClassicMrCrPlanError::Geometry);
        }
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LutParameters {
    pub descriptor: [u16; 3],
    pub explanation: String,
    pub lut_type: Option<String>,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OverlayParameters {
    pub rows: u16,
    pub columns: u16,
    pub overlay_type: String,
    pub origin: [i16; 2],
    pub bits_allocated: u16,
    pub bit_position: u16,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CrArtifactParameters {
    pub rows: u32,
    pub columns: u32,
    pub stored_values: Vec<i64>,
    pub pixel_min: i64,
    pub pixel_max: i64,
    pub frame_sha256: String,
    pub body_part_examined: String,
    pub view_position: String,
    pub overlay: OverlayParameters,
    pub modality_lut: LutParameters,
    pub voi_lut: LutParameters,
}

/// Inspect native CR intent without interpreting caller case or recipe names.
pub(crate) fn inspect_cr_capability(
    recipe: &CaseRecipe,
) -> Result<Option<CrArtifactParameters>, ClassicMrCrPlanError> {
    let Some(dicom) = &recipe.dicom else {
        return Ok(None);
    };
    let intent = dicom.artifacts.iter().any(|a| {
        a.template
            .as_ref()
            .is_some_and(|t| t.template_id == "classic/cr")
            || ["overlay", "modality_lut", "voi_lut"]
                .iter()
                .any(|key| a.parameters.contains_key(*key))
    });
    if !intent {
        return Ok(None);
    }
    if dicom.artifacts.len() != 1 {
        return Err(ClassicMrCrPlanError::Contract("CR single artifact"));
    }
    let a = &dicom.artifacts[0];
    // Keep the existing named compressed qualification on its historical path.
    if recipe.recipe_id == "cr_overlay_modality_voi_rle_lossless"
        && recipe.binding.case_id == "classic/cr/overlay_modality_voi_rle_lossless"
        && recipe.planning_order == Some(501)
        && a.encoding.transfer_syntax_uid == "1.2.840.10008.1.2.5"
    {
        return Ok(None);
    }
    let has = |values: &[String], expected: &str| values.len() == 1 && values[0] == expected;
    if recipe.plan_provider_id != PROVIDER_ID
        || recipe.kind != super::RecipeKind::Dicom
        || recipe.mutation.is_some()
        || recipe.qualification.is_some()
        || recipe.planning_order.is_none()
        || recipe.projection_order.is_none()
        || a.public_profile_membership.is_some()
        || !recipe.provider_parameters.is_empty()
        || !recipe.dependencies.is_empty()
        || !has(&recipe.validation_rule_ids, "validation.shared")
        || !has(&recipe.projection_rule_ids, "projection.curated")
        || a.logical_id != "instance"
        || a.order != 0
        || a.output.role != "instance"
        || a.output.provider_derived == Some(true)
        || a.output.path.is_none()
        || a.template
            .as_ref()
            .is_none_or(|t| t.template_id != "classic/cr" || t.template_version != "1.0.0")
        || a.content.provider_id != CONTENT_PROVIDER_ID
        || !a.content.parameters.is_empty()
        || a.algorithm_provider_id.as_deref() != Some(CR_ALGORITHM_ID)
        || a.classic_projection.as_ref().is_none_or(|p| {
            p.family != super::ClassicProjectionFamily::MrCr
                || p.mr.is_some()
                || p.icc.is_some()
                || !p.standards_evidence_append.is_empty()
                || p.include_implementation_version_name
                || p.semantic_labels.as_ref().is_none_or(|labels| {
                    labels.overlay_pattern.is_none()
                        || labels.modality_lut.is_none()
                        || labels.voi_lut.is_none()
                        || labels.photometric_semantics.is_some()
                })
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
        return Err(ClassicMrCrPlanError::Contract("complete native CR tuple"));
    }
    OutputRelativePath::new(a.output.path.clone().expect("explicit CR output"))?;
    let p: CrArtifactParameters = parameters(a)?;
    let count = p
        .rows
        .checked_mul(p.columns)
        .ok_or(ClassicMrCrPlanError::CrStructure)?;
    let overlay_len = count
        .checked_add(7)
        .and_then(|n| n.checked_div(8))
        .and_then(|n| n.checked_add(n % 2))
        .ok_or(ClassicMrCrPlanError::CrStructure)?;
    if p.rows == 0
        || p.columns == 0
        || p.rows > u16::MAX.into()
        || p.columns > u16::MAX.into()
        || usize::try_from(count).ok() != Some(p.stored_values.len())
        || p.stored_values.iter().any(|v| !(0..=255).contains(v))
        || p.stored_values.iter().min().copied() != Some(p.pixel_min)
        || p.stored_values.iter().max().copied() != Some(p.pixel_max)
        || p.overlay.rows as u32 != p.rows
        || p.overlay.columns as u32 != p.columns
        || p.overlay.overlay_type != "G"
        || p.overlay.origin != [1, 1]
        || p.overlay.bits_allocated != 1
        || p.overlay.bit_position != 0
        || usize::try_from(overlay_len).ok() != Some(p.overlay.data.len())
        || p.modality_lut.descriptor != [4, 0, 16]
        || p.voi_lut.descriptor != [4, 0, 16]
        || p.modality_lut.data.len() != 8
        || p.voi_lut.data.len() != 8
        || p.modality_lut.lut_type.as_deref() != Some("US")
        || p.voi_lut.lut_type.is_some()
    {
        return Err(ClassicMrCrPlanError::CrStructure);
    }
    // Unused overlay bits and even-length padding must not invent extra pixels.
    for bit in u64::from(count)..(p.overlay.data.len() as u64 * 8) {
        if p.overlay.data[(bit / 8) as usize] & (1 << (bit % 8)) != 0 {
            return Err(ClassicMrCrPlanError::CrStructure);
        }
    }
    let bytes: Vec<u8> = p.stored_values.iter().map(|v| *v as u8).collect();
    if crate::sha256_hex(&bytes) != p.frame_sha256 {
        return Err(ClassicMrCrPlanError::Contract("CR frame hash"));
    }
    Ok(Some(p))
}

pub fn plan_mr_cr_recipe(
    recipe: &CaseRecipe,
    standards_lock_sha256: &str,
    seed: u64,
) -> Result<Option<Vec<ClassicInstanceRequest>>, ClassicMrCrPlanError> {
    let mr_capability = inspect_mr_capability(recipe)?;
    let native_cr = inspect_cr_capability(recipe)?.is_some();
    let family = if mr_capability.is_some() {
        Family::Mr
    } else if native_cr {
        Family::Cr
    } else {
        match recipe.recipe_id.as_str() {
            "cr_overlay_modality_voi" | "cr_overlay_modality_voi_rle_lossless" => Family::Cr,
            _ => return Ok(None),
        }
    };
    if recipe.plan_provider_id != PROVIDER_ID {
        return Err(ClassicMrCrPlanError::Contract("plan_provider_id"));
    }
    let topology = if mr_capability.is_some() {
        None
    } else if native_cr {
        Some(vec![ExpectedArtifact {
            logical_id: "instance",
            order: 0,
            path: "",
            transfer_syntax_uid: "1.2.840.10008.1.2.1",
            template_id: "classic/cr",
        }])
    } else {
        Some(expected_topology(recipe)?)
    };
    let dicom = recipe
        .dicom
        .as_ref()
        .ok_or(ClassicMrCrPlanError::Contract("dicom"))?;
    if topology
        .as_ref()
        .is_some_and(|topology| dicom.artifacts.len() != topology.len())
    {
        return Err(ClassicMrCrPlanError::Contract("dicom.artifacts topology"));
    }

    let shared_index = 0;
    let study_uid = uid(
        recipe,
        standards_lock_sha256,
        seed,
        shared_index,
        UidRole::StudyInstance,
    );
    let series_uid = uid(
        recipe,
        standards_lock_sha256,
        seed,
        shared_index,
        UidRole::SeriesInstance,
    );
    let frame_uid = (family == Family::Mr).then(|| {
        uid(
            recipe,
            standards_lock_sha256,
            seed,
            shared_index,
            UidRole::FrameOfReference,
        )
    });
    let implementation_class_uid = deterministic_uid(&DeterministicUidInput {
        standards_lock_sha256,
        case_id: "dicom-test-suite/implementation",
        recipe_version: crate::BYTE_STABLE_OUTPUT_VERSION,
        run_seed: 0,
        file_index: 0,
        frame_index: None,
        referenced_object_index: None,
        role: UidRole::ImplementationClass,
    });
    let mut requests = Vec::with_capacity(dicom.artifacts.len());
    for (index, artifact) in dicom.artifacts.iter().enumerate() {
        if let Some(expected) = topology.as_ref().map(|topology| &topology[index]) {
            if artifact.content.provider_id != CONTENT_PROVIDER_ID
                || artifact.algorithm_provider_id.as_deref() != Some(family.algorithm_id())
                || artifact.output.provider_derived.unwrap_or(false)
                || artifact.logical_id != expected.logical_id
                || artifact.order != expected.order
                || artifact.output.role != expected.logical_id
                || (!native_cr && artifact.output.path.as_deref() != Some(expected.path))
                || artifact.encoding.transfer_syntax_uid != expected.transfer_syntax_uid
                || artifact
                    .template
                    .as_ref()
                    .map(|template| template.template_id.as_str())
                    != Some(expected.template_id)
            {
                return Err(ClassicMrCrPlanError::Contract("artifact provider binding"));
            }
        }
        let output_path = artifact
            .output
            .path
            .as_ref()
            .ok_or(ClassicMrCrPlanError::Contract("artifact.output.path"))?;
        let file_index = artifact.order;
        let sop_uid = uid(
            recipe,
            standards_lock_sha256,
            seed,
            file_index,
            UidRole::SopInstance,
        );
        let (common, family_fragment, pixels) = match family {
            Family::Mr => {
                let capability = mr_capability.as_ref().expect("inspected MR capability");
                let parameters = &capability.artifacts[index];
                let common = mr_common(
                    &capability.provider,
                    &study_uid,
                    &series_uid,
                    frame_uid.as_deref().expect("MR frame of reference"),
                    &parameters.instance_number,
                );
                let fragment = mr_fragment(&capability.provider, &capability.mr, parameters)?;
                let pixels = pixel_request(
                    parameters.rows,
                    parameters.columns,
                    StoredValueType::U16,
                    PixelDataVr::Ow,
                    parameters.stored_values.clone(),
                    parameters.pixel_min,
                    parameters.pixel_max,
                    parameters.frame_sha256.clone(),
                );
                (common, fragment, pixels)
            }
            Family::Cr => {
                let parameters: CrArtifactParameters = parameters(artifact)?;
                let common = common(recipe, "CR", "DTS-CR", &study_uid, &series_uid, None, "1");
                let fragment = cr_fragment(&parameters)?;
                let pixels = pixel_request(
                    parameters.rows,
                    parameters.columns,
                    StoredValueType::U8,
                    PixelDataVr::Ob,
                    parameters.stored_values,
                    parameters.pixel_min,
                    parameters.pixel_max,
                    parameters.frame_sha256,
                );
                (common, fragment, pixels)
            }
        };
        requests.push(ClassicInstanceRequest {
            logical_id: artifact.logical_id.clone(),
            order: artifact.order.into(),
            output_relative_path: OutputRelativePath::new(output_path.clone())?,
            dependencies: Vec::new(),
            common,
            sop_class_uid: family.sop_class_uid().into(),
            sop_instance_uid: sop_uid,
            implementation_class_uid: implementation_class_uid.clone(),
            family: vec![family_fragment],
            pixels: ClassicPixelRequest {
                slot: CLASSIC_PIXEL_SLOT.into(),
                pixels,
                rescale: None,
                window: None,
            },
        });
    }
    Ok(Some(requests))
}

#[derive(Debug, Clone, Copy)]
struct ExpectedArtifact {
    logical_id: &'static str,
    order: u32,
    path: &'static str,
    transfer_syntax_uid: &'static str,
    template_id: &'static str,
}

fn expected_topology(recipe: &CaseRecipe) -> Result<Vec<ExpectedArtifact>, ClassicMrCrPlanError> {
    const LE: &str = "1.2.840.10008.1.2.1";
    const RLE: &str = "1.2.840.10008.1.2.5";
    let (planning_order, case_id, artifacts) = match recipe.recipe_id.as_str() {
        "cr_overlay_modality_voi" => (
            500,
            "classic/cr/overlay_modality_voi_explicit_le",
            vec![ExpectedArtifact {
                logical_id: "instance",
                order: 0,
                path: "classic/cr/overlay_modality_voi_explicit_le/instance.dcm",
                transfer_syntax_uid: LE,
                template_id: "classic/cr",
            }],
        ),
        "cr_overlay_modality_voi_rle_lossless" => (
            501,
            "classic/cr/overlay_modality_voi_rle_lossless",
            vec![ExpectedArtifact {
                logical_id: "instance",
                order: 0,
                path: "classic/cr/overlay_modality_voi_rle_lossless/instance.dcm",
                transfer_syntax_uid: RLE,
                template_id: "classic/cr",
            }],
        ),
        _ => return Err(ClassicMrCrPlanError::Contract("owned recipe identity")),
    };
    if recipe.planning_order != Some(planning_order) || recipe.binding.case_id != case_id {
        return Err(ClassicMrCrPlanError::Contract("planning_order"));
    }
    Ok(artifacts)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Family {
    Mr,
    Cr,
}

impl Family {
    fn algorithm_id(self) -> &'static str {
        match self {
            Self::Mr => MR_ALGORITHM_ID,
            Self::Cr => CR_ALGORITHM_ID,
        }
    }

    fn sop_class_uid(self) -> &'static str {
        match self {
            Self::Mr => MR_SOP_CLASS_UID,
            Self::Cr => CR_SOP_CLASS_UID,
        }
    }
}

fn parameters<T: for<'de> Deserialize<'de>>(
    artifact: &super::PlannedArtifactRecipe,
) -> Result<T, ClassicMrCrPlanError> {
    serde_json::from_value(Value::Object(artifact.parameters.clone()))
        .map_err(ClassicMrCrPlanError::Parameters)
}

fn common(
    recipe: &CaseRecipe,
    modality: &str,
    study_id: &str,
    study_uid: &str,
    series_uid: &str,
    frame_uid: Option<&str>,
    instance_number: &str,
) -> CommonModuleRequest {
    CommonModuleRequest {
        patient: PatientModuleInput {
            specific_character_set: ElementPresence::Omitted,
            patient_name: value("DTS^Synthetic^Patient001"),
            patient_id: value("DTS-PATIENT-001"),
            patient_birth_date: value("19700101"),
            patient_sex: value("O"),
        },
        study: StudyModuleInput {
            study_instance_uid: study_uid.into(),
            study_date: value("20260101"),
            study_time: value("000000"),
            accession_number: ElementPresence::Empty,
            referring_physician_name: ElementPresence::Empty,
            study_id: value(study_id),
        },
        series: SeriesModuleInput {
            modality: modality.into(),
            series_instance_uid: series_uid.into(),
            series_number: value("1"),
            series_date: ElementPresence::Omitted,
            series_time: ElementPresence::Omitted,
        },
        frame_of_reference: frame_uid.map(|uid| FrameOfReferenceModuleInput {
            frame_of_reference_uid: uid.into(),
            position_reference_indicator: ElementPresence::Empty,
        }),
        equipment: EquipmentModuleInput {
            manufacturer: value("dicom-test-suite"),
            manufacturer_model_name: value(&recipe.recipe_id),
            software_versions: value(crate::BYTE_STABLE_OUTPUT_VERSION),
        },
        image: ImageModuleInput {
            instance_number: value(instance_number),
            patient_orientation: ElementPresence::Empty,
            content_date: value("20260101"),
            content_time: value("000000"),
        },
    }
}

fn mr_common(
    provider: &MrProviderParameters,
    study_uid: &str,
    series_uid: &str,
    frame_uid: &str,
    instance_number: &str,
) -> CommonModuleRequest {
    CommonModuleRequest {
        patient: PatientModuleInput {
            specific_character_set: ElementPresence::Omitted,
            patient_name: value(&provider.patient.patient_name),
            patient_id: value(&provider.patient.patient_id),
            patient_birth_date: value(&provider.patient.patient_birth_date),
            patient_sex: value(&provider.patient.patient_sex),
        },
        study: StudyModuleInput {
            study_instance_uid: study_uid.into(),
            study_date: value(&provider.study.study_date),
            study_time: value(&provider.study.study_time),
            accession_number: empty_or_value(&provider.study.accession_number),
            referring_physician_name: empty_or_value(&provider.study.referring_physician_name),
            study_id: value(&provider.study.study_id),
        },
        series: SeriesModuleInput {
            modality: "MR".into(),
            series_instance_uid: series_uid.into(),
            series_number: value(&provider.series_number),
            series_date: ElementPresence::Omitted,
            series_time: ElementPresence::Omitted,
        },
        frame_of_reference: Some(FrameOfReferenceModuleInput {
            frame_of_reference_uid: frame_uid.into(),
            position_reference_indicator: ElementPresence::Empty,
        }),
        equipment: EquipmentModuleInput {
            manufacturer: value(&provider.equipment.manufacturer),
            manufacturer_model_name: value(&provider.equipment.manufacturer_model_name),
            software_versions: value(&provider.equipment.software_versions),
        },
        image: ImageModuleInput {
            instance_number: value(instance_number),
            patient_orientation: ElementPresence::Empty,
            content_date: value(&provider.acquisition_date),
            content_time: value(&provider.acquisition_time),
        },
    }
}

fn empty_or_value(value_: &str) -> ElementPresence<String> {
    if value_.is_empty() {
        ElementPresence::Empty
    } else {
        value(value_)
    }
}

fn mr_fragment(
    provider: &MrProviderParameters,
    mr: &ClassicMrProjection,
    parameters: &MrArtifactParameters,
) -> Result<FamilyModuleFragment, ClassicMrCrPlanError> {
    if parameters.pixel_spacing.len() != 2
        || parameters.image_orientation_patient.len() != 6
        || parameters.image_position_patient.len() != 3
        || !parameters.position_along_normal.is_finite()
    {
        return Err(ClassicMrCrPlanError::Geometry);
    }
    Ok(FamilyModuleFragment::new(
        MR_ALGORITHM_ID,
        "mr_image",
        vec![
            set_string("0008,001C", DicomVr::CS, "YES"),
            set_multi_string("0008,0008", DicomVr::CS, provider.image_type.clone()),
            set_string("0008,0022", DicomVr::DA, &provider.acquisition_date),
            set_string("0008,0032", DicomVr::TM, &provider.acquisition_time),
            set_multi_string("0028,0030", DicomVr::DS, parameters.pixel_spacing.clone()),
            set_multi_string(
                "0020,0037",
                DicomVr::DS,
                parameters.image_orientation_patient.clone(),
            ),
            set_multi_string(
                "0020,0032",
                DicomVr::DS,
                parameters.image_position_patient.clone(),
            ),
            set_string("0018,0050", DicomVr::DS, &parameters.slice_thickness),
            set_string("0018,0088", DicomVr::DS, &parameters.spacing_between_slices),
            set_string("0020,1041", DicomVr::DS, &parameters.slice_location),
            set_string("0020,0012", DicomVr::IS, &provider.acquisition_number),
            set_string("0018,0020", DicomVr::CS, &mr.scanning_sequence),
            set_string("0018,0021", DicomVr::CS, &mr.sequence_variant),
            if mr.scan_options.is_empty() {
                empty("0018,0022")
            } else {
                set_string("0018,0022", DicomVr::CS, &mr.scan_options)
            },
            set_string("0018,0023", DicomVr::CS, &mr.mr_acquisition_type),
            set_string("0018,0080", DicomVr::DS, &mr.repetition_time),
            set_string("0018,0081", DicomVr::DS, &mr.echo_time),
            set_string("0018,0091", DicomVr::IS, &mr.echo_train_length),
            set_string("0018,0087", DicomVr::DS, &mr.magnetic_field_strength),
        ],
    )?)
}

fn cr_fragment(
    parameters: &CrArtifactParameters,
) -> Result<FamilyModuleFragment, ClassicMrCrPlanError> {
    if parameters.overlay.rows as u32 != parameters.rows
        || parameters.overlay.columns as u32 != parameters.columns
        || parameters.modality_lut.lut_type.is_none()
        || parameters.voi_lut.lut_type.is_some()
    {
        return Err(ClassicMrCrPlanError::CrStructure);
    }
    Ok(FamilyModuleFragment::new(
        CR_ALGORITHM_ID,
        "cr_image",
        vec![
            set_string("0008,001C", DicomVr::CS, "YES"),
            set_multi_string("0008,0008", DicomVr::CS, ["ORIGINAL", "PRIMARY"]),
            set_string("0008,0022", DicomVr::DA, "20260101"),
            set_string("0008,0032", DicomVr::TM, "000000"),
            set_string("0020,0012", DicomVr::IS, "1"),
            set_string("0018,0015", DicomVr::CS, &parameters.body_part_examined),
            set_string("0018,5101", DicomVr::CS, &parameters.view_position),
            set_unsigned("6000,0010", parameters.overlay.rows.into()),
            set_unsigned("6000,0011", parameters.overlay.columns.into()),
            set_string("6000,0040", DicomVr::CS, &parameters.overlay.overlay_type),
            set_signed_multi("6000,0050", parameters.overlay.origin),
            set_unsigned("6000,0100", parameters.overlay.bits_allocated.into()),
            set_unsigned("6000,0102", parameters.overlay.bit_position.into()),
            set_binary("6000,3000", DicomVr::OW, parameters.overlay.data.clone()),
            lut_sequence("0028,3000", &parameters.modality_lut),
            lut_sequence("0028,3010", &parameters.voi_lut),
        ],
    )?)
}

fn pixel_request(
    rows: u32,
    columns: u32,
    stored_value_type: StoredValueType,
    pixel_data_vr: PixelDataVr,
    stored_values: Vec<i64>,
    pixel_min: i64,
    pixel_max: i64,
    frame_sha256: String,
) -> NativePixelRequest {
    let bits = stored_value_type.bits_allocated();
    NativePixelRequest {
        shape: PixelShape {
            rows,
            columns,
            frames: 1,
            samples_per_pixel: 1,
            photometric_interpretation: PhotometricInterpretation::Monochrome2,
            bits_allocated: bits,
            bits_stored: bits,
            high_bit: bits - 1,
            pixel_representation: stored_value_type.pixel_representation(),
            stored_value_type,
            byte_order: ByteOrder::Little,
            pixel_data_vr,
            color: None,
        },
        stored_values,
        declared_pixel_min: pixel_min,
        declared_pixel_max: pixel_max,
        expected_frame_sha256: vec![frame_sha256],
        signed_stored_bits: Default::default(),
        padding: None,
        palette: None,
    }
}

fn uid(
    recipe: &CaseRecipe,
    standards_lock_sha256: &str,
    seed: u64,
    file_index: u32,
    role: UidRole,
) -> String {
    deterministic_uid(&DeterministicUidInput {
        standards_lock_sha256,
        case_id: &recipe.binding.case_id,
        recipe_version: &recipe.recipe_version,
        run_seed: seed,
        file_index,
        frame_index: None,
        referenced_object_index: None,
        role,
    })
}

fn value(value: impl Into<String>) -> ElementPresence<String> {
    ElementPresence::Value(value.into())
}

fn address(tag: &str) -> AttributeAddress {
    AttributeAddress::from_normalized_tag(tag).expect("classic MR/CR tag is valid")
}

fn set_string(tag: &str, vr: DicomVr, value: impl Into<String>) -> AttributeOperation {
    AttributeOperation::Set {
        address: address(tag),
        vr,
        value: AttributeValue::Primitive(PrimitiveValue::String(value.into())),
    }
}

fn set_multi_string(
    tag: &str,
    vr: DicomVr,
    values: impl IntoIterator<Item = impl Into<String>>,
) -> AttributeOperation {
    AttributeOperation::Set {
        address: address(tag),
        vr,
        value: AttributeValue::Multi(
            values
                .into_iter()
                .map(|value| PrimitiveValue::String(value.into()))
                .collect(),
        ),
    }
}

fn set_unsigned(tag: &str, value: u64) -> AttributeOperation {
    AttributeOperation::Set {
        address: address(tag),
        vr: DicomVr::US,
        value: AttributeValue::Primitive(PrimitiveValue::Unsigned(value)),
    }
}

fn set_signed_multi(tag: &str, values: [i16; 2]) -> AttributeOperation {
    AttributeOperation::Set {
        address: address(tag),
        vr: DicomVr::SS,
        value: AttributeValue::Multi(
            values
                .into_iter()
                .map(|value| PrimitiveValue::Signed(value.into()))
                .collect(),
        ),
    }
}

fn set_binary(tag: &str, vr: DicomVr, data: Vec<u8>) -> AttributeOperation {
    AttributeOperation::Set {
        address: address(tag),
        vr,
        value: AttributeValue::Binary(data),
    }
}

fn empty(tag: &str) -> AttributeOperation {
    AttributeOperation::Empty {
        address: address(tag),
    }
}

fn lut_sequence(tag: &str, lut: &LutParameters) -> AttributeOperation {
    let mut attributes = vec![
        AttributeOperation::Set {
            address: address("0028,3002"),
            vr: DicomVr::US,
            value: AttributeValue::Multi(
                lut.descriptor
                    .into_iter()
                    .map(|value| PrimitiveValue::Unsigned(value.into()))
                    .collect(),
            ),
        },
        set_string("0028,3003", DicomVr::LO, &lut.explanation),
    ];
    if let Some(lut_type) = &lut.lut_type {
        attributes.push(set_string("0028,3004", DicomVr::LO, lut_type));
    }
    attributes.push(set_binary("0028,3006", DicomVr::OW, lut.data.clone()));
    AttributeOperation::Set {
        address: address(tag),
        vr: DicomVr::SQ,
        value: AttributeValue::Sequence(vec![AttributeItem { attributes }]),
    }
}

#[derive(Debug)]
pub enum ClassicMrCrPlanError {
    Contract(&'static str),
    Parameters(serde_json::Error),
    Geometry,
    CrStructure,
    OutputPath(crate::corpus_plan::CorpusPlanError),
    Classic(super::ClassicPlanError),
}

impl From<crate::corpus_plan::CorpusPlanError> for ClassicMrCrPlanError {
    fn from(error: crate::corpus_plan::CorpusPlanError) -> Self {
        Self::OutputPath(error)
    }
}

impl From<super::ClassicPlanError> for ClassicMrCrPlanError {
    fn from(error: super::ClassicPlanError) -> Self {
        Self::Classic(error)
    }
}

impl fmt::Display for ClassicMrCrPlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for ClassicMrCrPlanError {}
