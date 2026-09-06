//! Data-first CT and CT geometry recipe planning.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{
    CLASSIC_PIXEL_SLOT, CaseRecipe, ClassicFamilyProvider, ClassicInstanceRequest,
    ClassicPixelRequest, ClassicPlanError, ClassicProjectionFamily, CommonModuleRequest,
    ElementPresence, EquipmentModuleInput, FamilyModuleFragment, FrameOfReferenceModuleInput,
    ImageModuleInput, PatientModuleInput, RescalePlan, SeriesModuleInput, StudyModuleInput,
    WindowPlan,
};
use crate::composition::{
    AttributeAddress, AttributeOperation, AttributeValue, DicomVr, PrimitiveValue,
};
use crate::corpus_plan::OutputRelativePath;
use crate::native_pixel::{
    ByteOrder, NativePixelRequest, PhotometricInterpretation, PixelDataVr, PixelShape,
    StoredValueType,
};
use crate::{DeterministicUidInput, UidRole, deterministic_uid, sha256_hex};

const PROVIDER_ID: &str = "native.classic_plan";
const CONTENT_PROVIDER_ID: &str = "content.native_pixels";
const ALGORITHM_PROVIDER_ID: &str = "algorithm.classic_ct";
const TEMPLATE_ID: &str = "classic/ct";
const TEMPLATE_VERSION: &str = "1.0.0";
const CT_IMAGE_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.2";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClassicCtProviderParameters {
    pub patient: ClassicCtPatientParameters,
    pub study: ClassicCtStudyParameters,
    pub equipment: ClassicCtEquipmentParameters,
    pub acquisition_date: String,
    pub acquisition_time: String,
    pub image_type: Vec<String>,
    pub pixel_spacing: [String; 2],
    pub image_orientation_patient: [String; 6],
    pub slice_thickness: String,
    #[serde(default)]
    pub spacing_between_slices: Option<String>,
    #[serde(default)]
    pub gantry_detector_tilt: Option<String>,
    pub kvp: String,
    pub rescale_intercept: String,
    pub rescale_slope: String,
    pub rescale_type: String,
    pub window_center: String,
    pub window_width: String,
    #[serde(default)]
    pub sorting_conflict_expected: Option<bool>,
    #[serde(default)]
    pub series_organization: Option<ClassicCtSeriesOrganization>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClassicCtPatientParameters {
    pub patient_name: String,
    pub patient_id: String,
    pub patient_birth_date: String,
    pub patient_sex: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClassicCtStudyParameters {
    pub study_date: String,
    pub study_time: String,
    pub accession_number: String,
    pub referring_physician_name: String,
    pub study_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClassicCtEquipmentParameters {
    pub manufacturer: String,
    pub software_versions: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClassicCtSeriesOrganization {
    pub group_id: String,
    pub shared_study_instance_uid: bool,
    pub shared_frame_of_reference_uid: bool,
    pub distinct_series_instance_uids: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClassicCtArtifactParameters {
    pub uid_file_index: u32,
    pub series_index: u32,
    pub series_number: String,
    pub acquisition_number: String,
    pub instance_number: ClassicCtInstanceNumber,
    pub image_position_patient: [String; 3],
    pub position_along_normal: f64,
    pub pixels: ClassicCtPixelParameters,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum ClassicCtInstanceNumber {
    Value { value: String },
    Empty,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClassicCtPixelParameters {
    pub rows: u16,
    pub columns: u16,
    pub stored_values: Vec<i64>,
    pub pixel_min: i64,
    pub pixel_max: i64,
    pub frame_sha256: String,
}

#[derive(Debug, Default, Clone, Copy)]
struct ClassicCtFamilyProvider;

#[derive(Debug, Clone)]
struct ClassicCtFamilyRequest<'a> {
    provider: &'a ClassicCtProviderParameters,
    artifact: &'a ClassicCtArtifactParameters,
    multi_instance_series: bool,
}

/// A fully decoded caller CT contract selected by stable capability IDs.
///
/// This deliberately contains no case or recipe name. The external corpus
/// loader can therefore reuse the same discriminator without granting a case
/// namespace authority over provider dispatch.
#[derive(Debug)]
pub(crate) struct ClassicCtCapability {
    pub(crate) provider: ClassicCtProviderParameters,
    pub(crate) artifacts: Vec<ClassicCtCapabilityArtifact>,
    pub(crate) study_series_count: usize,
}

#[derive(Debug)]
pub(crate) struct ClassicCtCapabilityArtifact {
    pub(crate) order: u32,
    pub(crate) parameters: ClassicCtArtifactParameters,
    pub(crate) geometric_order_index: usize,
    pub(crate) instance_number_order_index: Option<usize>,
    pub(crate) adjacent_spacing_mm: Vec<f64>,
    pub(crate) spacing_uniform: bool,
    pub(crate) sorting_conflict_expected: Option<bool>,
    pub(crate) series_instance_count: usize,
    pub(crate) series_ordinal: usize,
}

impl ClassicFamilyProvider<ClassicCtFamilyRequest<'_>> for ClassicCtFamilyProvider {
    const PROVIDER_ID: &'static str = ALGORITHM_PROVIDER_ID;

    fn plan_family(
        &self,
        request: ClassicCtFamilyRequest<'_>,
    ) -> Result<Vec<FamilyModuleFragment>, ClassicPlanError> {
        let provider = request.provider;
        let artifact = request.artifact;
        let mut general = vec![
            string("0008,001C", DicomVr::CS, "YES"),
            strings("0008,0008", DicomVr::CS, provider.image_type.clone()),
            string(
                "0020,0012",
                DicomVr::IS,
                artifact.acquisition_number.clone(),
            ),
            string("0008,0022", DicomVr::DA, provider.acquisition_date.clone()),
            string("0008,0032", DicomVr::TM, provider.acquisition_time.clone()),
        ];
        if request.multi_instance_series {
            general.push(empty("0018,5100"));
            general.push(string("0020,0062", DicomVr::CS, "U"));
        }

        let mut geometry = vec![
            strings("0028,0030", DicomVr::DS, provider.pixel_spacing.to_vec()),
            strings(
                "0020,0037",
                DicomVr::DS,
                provider.image_orientation_patient.to_vec(),
            ),
            strings(
                "0020,0032",
                DicomVr::DS,
                artifact.image_position_patient.to_vec(),
            ),
            string("0018,0050", DicomVr::DS, provider.slice_thickness.clone()),
        ];
        if let Some(value) = &provider.spacing_between_slices {
            geometry.push(string("0018,0088", DicomVr::DS, value.clone()));
        }
        if let Some(value) = &provider.gantry_detector_tilt {
            geometry.push(string("0018,1120", DicomVr::DS, value.clone()));
        }

        Ok(vec![
            FamilyModuleFragment::new(Self::PROVIDER_ID, "general_ct_image", general)?,
            FamilyModuleFragment::new(Self::PROVIDER_ID, "ct_geometry", geometry)?,
            FamilyModuleFragment::new(
                Self::PROVIDER_ID,
                "ct_acquisition",
                vec![string("0018,0060", DicomVr::DS, provider.kvp.clone())],
            )?,
        ])
    }
}

/// Resolve a declared CT capability into complete ordered, filesystem-free requests.
pub fn plan_ct_recipe(
    recipe: &CaseRecipe,
    standards_lock_sha256: &str,
    seed: u64,
) -> Result<Option<Vec<ClassicInstanceRequest>>, ClassicCtPlanError> {
    let Some(capability) = inspect_ct_capability(recipe)? else {
        return Ok(None);
    };
    recipe
        .planning_order
        .ok_or_else(|| contract("declared CT recipe requires planning_order"))?;
    let ClassicCtCapability {
        provider,
        artifacts: parameters,
        ..
    } = capability;
    let dicom = recipe
        .dicom
        .as_ref()
        .expect("inspected CT capability has DICOM artifacts");

    let study_instance_uid = uid(
        recipe,
        standards_lock_sha256,
        seed,
        UidRole::StudyInstance,
        0,
    );
    let frame_of_reference_uid = uid(
        recipe,
        standards_lock_sha256,
        seed,
        UidRole::FrameOfReference,
        0,
    );
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
    let artifacts_by_order = dicom
        .artifacts
        .iter()
        .map(|artifact| (artifact.order, artifact))
        .collect::<BTreeMap<_, _>>();
    for inspected in &parameters {
        let artifact = artifacts_by_order
            .get(&inspected.order)
            .expect("inspected CT artifact order exists");
        let parameters = &inspected.parameters;
        let series_instance_uid = uid(
            recipe,
            standards_lock_sha256,
            seed,
            UidRole::SeriesInstance,
            parameters.series_index,
        );
        let sop_instance_uid = uid(
            recipe,
            standards_lock_sha256,
            seed,
            UidRole::SopInstance,
            parameters.uid_file_index,
        );
        let instance_number = match &parameters.instance_number {
            ClassicCtInstanceNumber::Value { value } => ElementPresence::Value(value.clone()),
            ClassicCtInstanceNumber::Empty => ElementPresence::Empty,
        };
        let common = CommonModuleRequest {
            patient: PatientModuleInput {
                specific_character_set: ElementPresence::Omitted,
                patient_name: ElementPresence::Value(provider.patient.patient_name.clone()),
                patient_id: ElementPresence::Value(provider.patient.patient_id.clone()),
                patient_birth_date: ElementPresence::Value(
                    provider.patient.patient_birth_date.clone(),
                ),
                patient_sex: ElementPresence::Value(provider.patient.patient_sex.clone()),
            },
            study: StudyModuleInput {
                study_instance_uid: study_instance_uid.clone(),
                study_date: ElementPresence::Value(provider.study.study_date.clone()),
                study_time: ElementPresence::Value(provider.study.study_time.clone()),
                accession_number: empty_or_value(&provider.study.accession_number),
                referring_physician_name: empty_or_value(&provider.study.referring_physician_name),
                study_id: ElementPresence::Value(provider.study.study_id.clone()),
            },
            series: SeriesModuleInput {
                modality: "CT".into(),
                series_instance_uid,
                series_number: ElementPresence::Value(parameters.series_number.clone()),
                series_date: ElementPresence::Omitted,
                series_time: ElementPresence::Omitted,
            },
            frame_of_reference: Some(FrameOfReferenceModuleInput {
                frame_of_reference_uid: frame_of_reference_uid.clone(),
                position_reference_indicator: ElementPresence::Empty,
            }),
            equipment: EquipmentModuleInput {
                manufacturer: ElementPresence::Value(provider.equipment.manufacturer.clone()),
                manufacturer_model_name: ElementPresence::Value(recipe.recipe_id.clone()),
                software_versions: ElementPresence::Value(
                    provider.equipment.software_versions.clone(),
                ),
            },
            image: ImageModuleInput {
                instance_number,
                patient_orientation: ElementPresence::Empty,
                content_date: ElementPresence::Value(provider.acquisition_date.clone()),
                content_time: ElementPresence::Value(provider.acquisition_time.clone()),
            },
        };
        let family = ClassicCtFamilyProvider.plan_family(ClassicCtFamilyRequest {
            provider: &provider,
            artifact: parameters,
            multi_instance_series: inspected.series_instance_count > 1,
        })?;
        let pixels = &parameters.pixels;
        requests.push(ClassicInstanceRequest {
            logical_id: artifact.logical_id.clone(),
            order: u64::from(artifact.order),
            output_relative_path: OutputRelativePath::new(
                artifact
                    .output
                    .path
                    .clone()
                    .ok_or_else(|| contract("CT artifact output path must be explicit"))?,
            )?,
            dependencies: vec![],
            common,
            sop_class_uid: CT_IMAGE_STORAGE.into(),
            sop_instance_uid,
            implementation_class_uid: implementation_class_uid.clone(),
            family,
            pixels: ClassicPixelRequest {
                slot: CLASSIC_PIXEL_SLOT.into(),
                pixels: NativePixelRequest {
                    shape: PixelShape {
                        rows: u32::from(pixels.rows),
                        columns: u32::from(pixels.columns),
                        frames: 1,
                        samples_per_pixel: 1,
                        photometric_interpretation: PhotometricInterpretation::Monochrome2,
                        bits_allocated: 16,
                        bits_stored: 12,
                        high_bit: 11,
                        pixel_representation: 1,
                        stored_value_type: StoredValueType::I16,
                        byte_order: ByteOrder::Little,
                        pixel_data_vr: PixelDataVr::Ow,
                        color: None,
                    },
                    declared_pixel_min: pixels.pixel_min,
                    declared_pixel_max: pixels.pixel_max,
                    stored_values: pixels.stored_values.clone(),
                    expected_frame_sha256: vec![pixels.frame_sha256.clone()],
                    signed_stored_bits: Default::default(),
                    padding: None,
                    palette: None,
                },
                rescale: Some(RescalePlan {
                    intercept: provider.rescale_intercept.clone(),
                    slope: provider.rescale_slope.clone(),
                    rescale_type: ElementPresence::Value(provider.rescale_type.clone()),
                }),
                window: Some(WindowPlan {
                    center: vec![provider.window_center.clone()],
                    width: vec![provider.window_width.clone()],
                }),
            },
        });
    }
    validate_series_contract(&provider, &requests)?;
    Ok(Some(requests))
}

/// Identify and decode the CT member of the shared classic provider family.
///
/// Any CT marker declares intent. Once declared, every stable member of the
/// tuple must agree and every typed parameter object must decode and validate;
/// a partial or mixed tuple is an error rather than an opportunity for another
/// case-name-based family planner to claim the recipe.
pub(crate) fn inspect_ct_capability(
    recipe: &CaseRecipe,
) -> Result<Option<ClassicCtCapability>, ClassicCtPlanError> {
    let Some(dicom) = recipe.dicom.as_ref() else {
        return Ok(None);
    };
    let declared = dicom.artifacts.iter().any(|artifact| {
        artifact.algorithm_provider_id.as_deref() == Some(ALGORITHM_PROVIDER_ID)
            || artifact
                .template
                .as_ref()
                .is_some_and(|template| template.template_id == TEMPLATE_ID)
            || artifact
                .classic_projection
                .as_ref()
                .is_some_and(|projection| projection.family == ClassicProjectionFamily::Ct)
    });
    if !declared {
        return Ok(None);
    }
    if recipe.plan_provider_id != PROVIDER_ID {
        return Err(contract(format!(
            "declared CT recipe requires {PROVIDER_ID}, got {}",
            recipe.plan_provider_id
        )));
    }
    if dicom.artifacts.is_empty() {
        return Err(contract(
            "declared CT recipe requires at least one artifact",
        ));
    }
    let provider: ClassicCtProviderParameters = decode_object(
        Value::Object(recipe.provider_parameters.clone()),
        "provider_parameters",
    )?;
    validate_provider(&provider)?;
    let mut parameters = Vec::with_capacity(dicom.artifacts.len());
    let mut orders = BTreeSet::new();
    let mut uid_file_indices = BTreeSet::new();
    for artifact in &dicom.artifacts {
        if !orders.insert(artifact.order) {
            return Err(contract("CT artifact orders must be unique"));
        }
        validate_artifact_contract(artifact)?;
        if !artifact
            .classic_projection
            .as_ref()
            .is_some_and(|projection| projection.family == ClassicProjectionFamily::Ct)
        {
            return Err(contract(
                "CT artifact requires classic_projection family ct",
            ));
        }
        let decoded: ClassicCtArtifactParameters = decode_object(
            Value::Object(artifact.parameters.clone()),
            "artifact parameters",
        )?;
        validate_artifact_parameters(&decoded)?;
        validate_rescale_endpoints(&provider, &decoded)?;
        if !uid_file_indices.insert(decoded.uid_file_index) {
            return Err(contract("CT uid_file_index values must be unique"));
        }
        parameters.push((artifact.order, decoded));
    }
    if orders
        .iter()
        .copied()
        .ne(0..u32::try_from(orders.len()).unwrap_or(u32::MAX))
    {
        return Err(contract(
            "CT artifacts must have contiguous zero-based order",
        ));
    }
    parameters.sort_by_key(|(order, _)| *order);
    let artifacts = derive_series_contract(&provider, parameters)?;
    let study_series_count = artifacts
        .iter()
        .map(|artifact| artifact.parameters.series_index)
        .collect::<BTreeSet<_>>()
        .len();
    Ok(Some(ClassicCtCapability {
        provider,
        artifacts,
        study_series_count,
    }))
}

fn validate_artifact_contract(
    artifact: &super::PlannedArtifactRecipe,
) -> Result<(), ClassicCtPlanError> {
    let template = artifact
        .template
        .as_ref()
        .ok_or_else(|| contract("CT artifact requires a qualified template"))?;
    if template.template_id != TEMPLATE_ID || template.template_version != TEMPLATE_VERSION {
        return Err(contract(
            "CT artifact template does not match classic/ct@1.0.0",
        ));
    }
    if artifact.content.provider_id != CONTENT_PROVIDER_ID
        || !artifact.content.parameters.is_empty()
    {
        return Err(contract(
            "CT artifact requires parameter-free content.native_pixels",
        ));
    }
    if artifact.algorithm_provider_id.as_deref() != Some(ALGORITHM_PROVIDER_ID) {
        return Err(contract("CT artifact requires algorithm.classic_ct"));
    }
    if artifact.output.provider_derived == Some(true) || artifact.output.path.is_none() {
        return Err(contract("CT artifact output must be explicit"));
    }
    if !artifact.attribute_operations.is_empty()
        || artifact.secondary_capture.is_some()
        || artifact.metadata_sc.is_some()
        || artifact.nonsquare_geometry.is_some()
    {
        return Err(contract("CT static values must use typed CT parameters"));
    }
    Ok(())
}

fn validate_provider(provider: &ClassicCtProviderParameters) -> Result<(), ClassicCtPlanError> {
    if !(2..=8).contains(&provider.image_type.len())
        || !matches!(provider.image_type[0].as_str(), "ORIGINAL" | "DERIVED")
        || !matches!(provider.image_type[1].as_str(), "PRIMARY" | "SECONDARY")
        || provider.image_type.iter().any(|value| !valid_cs(value))
    {
        return Err(contract("invalid reusable CT Image Type declaration"));
    }
    validate_da(&provider.acquisition_date, "CT acquisition_date")?;
    validate_tm(&provider.acquisition_time, "CT acquisition_time")?;
    validate_da(
        &provider.patient.patient_birth_date,
        "CT patient_birth_date",
    )?;
    validate_da(&provider.study.study_date, "CT study_date")?;
    validate_tm(&provider.study.study_time, "CT study_time")?;
    for (field, value, positive) in [
        (
            "pixel_spacing row",
            provider.pixel_spacing[0].as_str(),
            true,
        ),
        (
            "pixel_spacing column",
            provider.pixel_spacing[1].as_str(),
            true,
        ),
        ("slice_thickness", provider.slice_thickness.as_str(), true),
        ("kvp", provider.kvp.as_str(), true),
        (
            "rescale_intercept",
            provider.rescale_intercept.as_str(),
            false,
        ),
        ("rescale_slope", provider.rescale_slope.as_str(), false),
        ("window_center", provider.window_center.as_str(), false),
        ("window_width", provider.window_width.as_str(), true),
    ] {
        let parsed = parse_ds(value, field)?;
        if positive && parsed <= 0.0 {
            return Err(contract(format!("CT {field} must be positive")));
        }
    }
    if parse_ds(&provider.rescale_slope, "rescale_slope")? == 0.0 {
        return Err(contract("CT rescale_slope must be nonzero"));
    }
    if provider.rescale_type.is_empty() || provider.rescale_type.len() > 16 {
        return Err(contract("invalid CT rescale_type"));
    }
    if let Some(value) = &provider.spacing_between_slices {
        if parse_ds(value, "spacing_between_slices")? <= 0.0 {
            return Err(contract("CT spacing_between_slices must be positive"));
        }
    }
    if let Some(value) = &provider.gantry_detector_tilt {
        let tilt = parse_ds(value, "gantry_detector_tilt")?;
        if !(0.0..=90.0).contains(&tilt) {
            return Err(contract(
                "CT gantry_detector_tilt magnitude must be within 0..90",
            ));
        }
    }
    let orientation = provider
        .image_orientation_patient
        .iter()
        .enumerate()
        .map(|(index, value)| parse_ds(value, &format!("image_orientation_patient[{index}]")))
        .collect::<Result<Vec<_>, _>>()?;
    let row = [orientation[0], orientation[1], orientation[2]];
    let column = [orientation[3], orientation[4], orientation[5]];
    if (dot(row, row).sqrt() - 1.0).abs() > 0.000_01
        || (dot(column, column).sqrt() - 1.0).abs() > 0.000_01
        || dot(row, column).abs() > 0.000_01
    {
        return Err(contract(
            "CT image_orientation_patient must contain orthonormal unit vectors",
        ));
    }
    if provider
        .series_organization
        .as_ref()
        .is_some_and(|organization| {
            organization.group_id.is_empty()
                || !organization.shared_study_instance_uid
                || !organization.shared_frame_of_reference_uid
                || !organization.distinct_series_instance_uids
        })
    {
        return Err(contract("invalid CT series organization contract"));
    }
    Ok(())
}

fn validate_artifact_parameters(
    parameters: &ClassicCtArtifactParameters,
) -> Result<(), ClassicCtPlanError> {
    parse_is(&parameters.series_number, "series_number")?;
    parse_is(&parameters.acquisition_number, "acquisition_number")?;
    if let ClassicCtInstanceNumber::Value { value } = &parameters.instance_number {
        parse_is(value, "instance_number")?;
    }
    let position = parameters
        .image_position_patient
        .iter()
        .enumerate()
        .map(|(index, value)| parse_ds(value, &format!("image_position_patient[{index}]")))
        .collect::<Result<Vec<_>, _>>()?;
    if !parameters.position_along_normal.is_finite() {
        return Err(contract("CT position_along_normal must be finite"));
    }
    let pixels = &parameters.pixels;
    let expected_values = usize::from(pixels.rows)
        .checked_mul(usize::from(pixels.columns))
        .ok_or_else(|| contract("CT native pixel cardinality overflow"))?;
    if pixels.rows == 0
        || pixels.columns == 0
        || pixels.stored_values.len() != expected_values
        || pixels
            .stored_values
            .iter()
            .any(|value| !(-2048..=2047).contains(value))
        || pixels.pixel_min != *pixels.stored_values.iter().min().unwrap_or(&0)
        || pixels.pixel_max != *pixels.stored_values.iter().max().unwrap_or(&0)
        || pixels.frame_sha256.len() != 64
        || !pixels
            .frame_sha256
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(contract("invalid CT native pixel declaration"));
    }
    let mut bytes = Vec::with_capacity(pixels.stored_values.len() * 2);
    for value in &pixels.stored_values {
        let stored_word = (*value as u16) & 0x0fff;
        bytes.extend_from_slice(&stored_word.to_le_bytes());
    }
    if sha256_hex(&bytes) != pixels.frame_sha256 {
        return Err(contract(
            "CT native pixel frame_sha256 does not match samples",
        ));
    }
    let _ = position;
    Ok(())
}

fn validate_rescale_endpoints(
    provider: &ClassicCtProviderParameters,
    parameters: &ClassicCtArtifactParameters,
) -> Result<(), ClassicCtPlanError> {
    let slope = parse_ds(&provider.rescale_slope, "rescale_slope")?;
    let intercept = parse_ds(&provider.rescale_intercept, "rescale_intercept")?;
    if [parameters.pixels.pixel_min, parameters.pixels.pixel_max]
        .into_iter()
        .map(|value| value as f64 * slope + intercept)
        .any(|value| !value.is_finite())
    {
        return Err(contract("CT rescale endpoint transformation is non-finite"));
    }
    Ok(())
}

fn derive_series_contract(
    provider: &ClassicCtProviderParameters,
    parameters: Vec<(u32, ClassicCtArtifactParameters)>,
) -> Result<Vec<ClassicCtCapabilityArtifact>, ClassicCtPlanError> {
    let orientation = provider
        .image_orientation_patient
        .iter()
        .map(|value| parse_ds(value, "image_orientation_patient"))
        .collect::<Result<Vec<_>, _>>()?;
    let row = [orientation[0], orientation[1], orientation[2]];
    let column = [orientation[3], orientation[4], orientation[5]];
    let normal = cross(row, column);
    let mut by_series = BTreeMap::<u32, Vec<(u32, &ClassicCtArtifactParameters, [f64; 3])>>::new();
    for (order, item) in &parameters {
        let values = item
            .image_position_patient
            .iter()
            .map(|value| parse_ds(value, "image_position_patient"))
            .collect::<Result<Vec<_>, _>>()?;
        let position = [values[0], values[1], values[2]];
        let projected = dot(normal, position);
        if (projected - item.position_along_normal).abs() > 0.000_01 {
            return Err(contract(
                "CT position_along_normal contradicts projected Image Position Patient",
            ));
        }
        by_series
            .entry(item.series_index)
            .or_default()
            .push((*order, item, position));
    }
    match &provider.series_organization {
        Some(_) if by_series.len() < 2 => {
            return Err(contract("CT series organization requires multiple series"));
        }
        None if by_series.len() > 1 => {
            return Err(contract(
                "multiple CT series require an explicit series organization contract",
            ));
        }
        _ => {}
    }
    let spacing_between = provider
        .spacing_between_slices
        .as_deref()
        .map(|value| parse_ds(value, "spacing_between_slices"))
        .transpose()?;
    let tilt = provider
        .gantry_detector_tilt
        .as_deref()
        .map(|value| parse_ds(value, "gantry_detector_tilt"))
        .transpose()?;
    let series_ordinals = by_series
        .keys()
        .enumerate()
        .map(|(index, value)| (*value, index + 1))
        .collect::<BTreeMap<_, _>>();
    let mut tilt_interval_observed = false;
    let mut derived = BTreeMap::<
        u32,
        (
            usize,
            Option<usize>,
            Vec<f64>,
            bool,
            Option<bool>,
            usize,
            usize,
        ),
    >::new();
    let mut series_conflicts = Vec::with_capacity(by_series.len());
    for (series_index, members) in &mut by_series {
        let first = members[0].1;
        if members.iter().any(|(_, item, _)| {
            item.series_number != first.series_number
                || item.acquisition_number != first.acquisition_number
        }) {
            return Err(contract(
                "CT series members must share Series and Acquisition Number declarations",
            ));
        }
        members.sort_by(|left, right| {
            left.1
                .position_along_normal
                .total_cmp(&right.1.position_along_normal)
                .then_with(|| left.0.cmp(&right.0))
        });
        if members.windows(2).any(|pair| {
            (pair[1].1.position_along_normal - pair[0].1.position_along_normal).abs() <= 0.000_01
        }) {
            return Err(contract(
                "CT projected slice positions must be unique within a series",
            ));
        }
        let adjacent = members
            .windows(2)
            .map(|pair| pair[1].1.position_along_normal - pair[0].1.position_along_normal)
            .collect::<Vec<_>>();
        let uniform = adjacent.first().is_none_or(|first| {
            adjacent
                .iter()
                .all(|value| (*value - *first).abs() <= 0.000_01)
        });
        if let Some(expected) = spacing_between {
            if adjacent
                .iter()
                .any(|value| (*value - expected).abs() > 0.000_01)
            {
                return Err(contract(
                    "CT spacing_between_slices contradicts projected slice intervals",
                ));
            }
        }
        if let Some(expected_tilt) = tilt {
            let mut shear_direction = None;
            for pair in members.windows(2) {
                tilt_interval_observed = true;
                let delta = subtract(pair[1].2, pair[0].2);
                let along = dot(delta, normal);
                let in_plane = subtract(delta, scale(normal, along));
                let magnitude = dot(in_plane, in_plane).sqrt();
                let actual = magnitude.atan2(along.abs()).to_degrees();
                if (actual - expected_tilt).abs() > 0.000_01 {
                    return Err(contract(
                        "CT gantry_detector_tilt contradicts the declared slice-origin shear",
                    ));
                }
                if magnitude > 0.000_01 {
                    let direction = scale(in_plane, 1.0 / magnitude);
                    if dot(direction, scale(column, -1.0)) < 0.999_99 {
                        return Err(contract(
                            "CT gantry tilt shear must follow the negative column direction",
                        ));
                    }
                    if shear_direction.is_some_and(|prior| dot(prior, direction) < 0.999_99) {
                        return Err(contract("CT gantry tilt shear direction is inconsistent"));
                    }
                    shear_direction = Some(direction);
                }
            }
        }
        let mut instance_order = members
            .iter()
            .map(|(order, item, _)| {
                let number = match &item.instance_number {
                    ClassicCtInstanceNumber::Value { value } => {
                        parse_is(value, "instance_number").ok()
                    }
                    ClassicCtInstanceNumber::Empty => None,
                };
                (*order, number)
            })
            .collect::<Vec<_>>();
        let fully_ordered = instance_order.iter().all(|(_, value)| value.is_some())
            && instance_order
                .iter()
                .filter_map(|(_, value)| *value)
                .collect::<BTreeSet<_>>()
                .len()
                == instance_order.len();
        let instance_ranks = if fully_ordered {
            instance_order.sort_by_key(|(order, value)| (value.unwrap(), *order));
            instance_order
                .iter()
                .enumerate()
                .map(|(index, (order, _))| (*order, index + 1))
                .collect::<BTreeMap<_, _>>()
        } else {
            BTreeMap::new()
        };
        let geometric_orders = members
            .iter()
            .map(|(order, _, _)| *order)
            .collect::<Vec<_>>();
        let numeric_orders = instance_order
            .iter()
            .map(|(order, _)| *order)
            .collect::<Vec<_>>();
        let conflict = fully_ordered.then_some(geometric_orders != numeric_orders);
        series_conflicts.push(conflict);
        for (index, (order, _, _)) in members.iter().enumerate() {
            derived.insert(
                *order,
                (
                    index + 1,
                    instance_ranks.get(order).copied(),
                    adjacent.clone(),
                    uniform,
                    conflict,
                    members.len(),
                    series_ordinals[series_index],
                ),
            );
        }
    }
    let aggregate_conflict = series_conflicts
        .iter()
        .all(Option::is_some)
        .then(|| series_conflicts.iter().any(|value| *value == Some(true)));
    if provider.sorting_conflict_expected != aggregate_conflict {
        return Err(contract(
            "CT sorting_conflict_expected contradicts the derived study-level conflict aggregate",
        ));
    }
    if tilt.is_some_and(|value| value > 0.000_01) && !tilt_interval_observed {
        return Err(contract(
            "positive CT gantry tilt requires at least one slice interval",
        ));
    }
    Ok(parameters
        .into_iter()
        .map(|(order, parameters)| {
            let facts = derived.remove(&order).expect("all CT members derived");
            ClassicCtCapabilityArtifact {
                order,
                parameters,
                geometric_order_index: facts.0,
                instance_number_order_index: facts.1,
                adjacent_spacing_mm: facts.2,
                spacing_uniform: facts.3,
                sorting_conflict_expected: facts.4,
                series_instance_count: facts.5,
                series_ordinal: facts.6,
            }
        })
        .collect())
}

fn validate_series_contract(
    provider: &ClassicCtProviderParameters,
    requests: &[ClassicInstanceRequest],
) -> Result<(), ClassicCtPlanError> {
    let distinct_series = requests
        .iter()
        .map(|request| request.common.series.series_instance_uid.as_str())
        .collect::<BTreeSet<_>>()
        .len();
    if provider.series_organization.is_some() != (distinct_series > 1) {
        return Err(contract(
            "CT planned series topology contradicts declaration",
        ));
    }
    Ok(())
}

fn parse_ds(value: &str, field: &str) -> Result<f64, ClassicCtPlanError> {
    if value.is_empty() || value.len() > 16 || !value.is_ascii() || value.trim() != value {
        return Err(contract(format!("invalid CT DS {field}")));
    }
    value
        .parse::<f64>()
        .ok()
        .filter(|parsed| parsed.is_finite())
        .ok_or_else(|| contract(format!("invalid CT DS {field}")))
}

fn parse_is(value: &str, field: &str) -> Result<i32, ClassicCtPlanError> {
    if value.is_empty() || value.len() > 12 || !value.is_ascii() || value.trim() != value {
        return Err(contract(format!("invalid CT IS {field}")));
    }
    value
        .parse::<i32>()
        .map_err(|_| contract(format!("invalid CT IS {field}")))
}

fn validate_da(value: &str, field: &str) -> Result<(), ClassicCtPlanError> {
    if value.len() != 8 || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(contract(format!("invalid {field}")));
    }
    let year = value[0..4].parse::<u32>().unwrap_or(0);
    let month = value[4..6].parse::<u32>().unwrap_or(0);
    let day = value[6..8].parse::<u32>().unwrap_or(0);
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let max_day = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => 0,
    };
    if year == 0 || day == 0 || day > max_day {
        return Err(contract(format!("invalid {field}")));
    }
    Ok(())
}

fn validate_tm(value: &str, field: &str) -> Result<(), ClassicCtPlanError> {
    let (whole, fraction) = value
        .split_once('.')
        .map_or((value, None), |(a, b)| (a, Some(b)));
    if !(2..=6).contains(&whole.len())
        || whole.len() % 2 != 0
        || !whole.bytes().all(|byte| byte.is_ascii_digit())
        || fraction.is_some_and(|part| {
            part.is_empty() || part.len() > 6 || !part.bytes().all(|byte| byte.is_ascii_digit())
        })
        || (fraction.is_some() && whole.len() != 6)
        || value.len() > 16
    {
        return Err(contract(format!("invalid {field}")));
    }
    let hour = whole[0..2].parse::<u32>().unwrap_or(24);
    let minute = whole
        .get(2..4)
        .map(|part| part.parse::<u32>().unwrap_or(60));
    let second = whole
        .get(4..6)
        .map(|part| part.parse::<u32>().unwrap_or(60));
    if hour > 23 || minute.is_some_and(|value| value > 59) || second.is_some_and(|value| value > 60)
    {
        return Err(contract(format!("invalid {field}")));
    }
    Ok(())
}

fn valid_cs(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 16
        && value.bytes().all(|byte| {
            byte.is_ascii_uppercase() || byte.is_ascii_digit() || matches!(byte, b' ' | b'_')
        })
}

fn dot(left: [f64; 3], right: [f64; 3]) -> f64 {
    left[0] * right[0] + left[1] * right[1] + left[2] * right[2]
}

fn cross(left: [f64; 3], right: [f64; 3]) -> [f64; 3] {
    [
        left[1] * right[2] - left[2] * right[1],
        left[2] * right[0] - left[0] * right[2],
        left[0] * right[1] - left[1] * right[0],
    ]
}

fn subtract(left: [f64; 3], right: [f64; 3]) -> [f64; 3] {
    [left[0] - right[0], left[1] - right[1], left[2] - right[2]]
}

fn scale(value: [f64; 3], factor: f64) -> [f64; 3] {
    [value[0] * factor, value[1] * factor, value[2] * factor]
}

fn uid(
    recipe: &CaseRecipe,
    standards_lock_sha256: &str,
    seed: u64,
    role: UidRole,
    file_index: u32,
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

fn empty_or_value(value: &str) -> ElementPresence<String> {
    if value.is_empty() {
        ElementPresence::Empty
    } else {
        ElementPresence::Value(value.into())
    }
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

fn empty(tag: &str) -> AttributeOperation {
    AttributeOperation::Empty {
        address: address(tag),
    }
}

fn address(tag: &str) -> AttributeAddress {
    AttributeAddress::from_normalized_tag(tag).expect("CT provider tag is valid")
}

fn decode_object<T: for<'de> Deserialize<'de>>(
    value: Value,
    field: &'static str,
) -> Result<T, ClassicCtPlanError> {
    serde_json::from_value(value).map_err(|error| ClassicCtPlanError::Parameters {
        field,
        message: error.to_string(),
    })
}

fn contract(message: impl Into<String>) -> ClassicCtPlanError {
    ClassicCtPlanError::Contract(message.into())
}

#[derive(Debug)]
pub enum ClassicCtPlanError {
    Contract(String),
    Parameters {
        field: &'static str,
        message: String,
    },
    Classic(ClassicPlanError),
    OutputPath(crate::corpus_plan::CorpusPlanError),
}

impl From<ClassicPlanError> for ClassicCtPlanError {
    fn from(error: ClassicPlanError) -> Self {
        Self::Classic(error)
    }
}

impl From<crate::corpus_plan::CorpusPlanError> for ClassicCtPlanError {
    fn from(error: crate::corpus_plan::CorpusPlanError) -> Self {
        Self::OutputPath(error)
    }
}

impl fmt::Display for ClassicCtPlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for ClassicCtPlanError {}
