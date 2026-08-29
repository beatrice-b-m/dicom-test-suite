//! Data-first CT and CT geometry recipe planning.

use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{
    CaseRecipe, ClassicFamilyProvider, ClassicInstanceRequest, ClassicPixelRequest,
    ClassicPlanError, CommonModuleRequest, ElementPresence, EquipmentModuleInput,
    FamilyModuleFragment, FrameOfReferenceModuleInput, ImageModuleInput, PatientModuleInput,
    RescalePlan, SeriesModuleInput, StudyModuleInput, WindowPlan,
};
use crate::composition::{
    AttributeAddress, AttributeOperation, AttributeValue, DicomVr, PrimitiveValue,
};
use crate::corpus_plan::OutputRelativePath;
use crate::native_pixel::{
    ByteOrder, NativePixelRequest, PhotometricInterpretation, PixelDataVr, PixelShape,
    StoredValueType,
};
use crate::{DeterministicUidInput, UidRole, deterministic_uid};

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

/// Resolve an owned CT recipe into complete ordered, filesystem-free requests.
pub fn plan_ct_recipe(
    recipe: &CaseRecipe,
    standards_lock_sha256: &str,
    seed: u64,
) -> Result<Option<Vec<ClassicInstanceRequest>>, ClassicCtPlanError> {
    if !is_owned_case(&recipe.binding.case_id) {
        return Ok(None);
    }
    if recipe.plan_provider_id != PROVIDER_ID {
        return Err(contract(format!(
            "owned CT recipe requires {PROVIDER_ID}, got {}",
            recipe.plan_provider_id
        )));
    }
    let planning_order = recipe
        .planning_order
        .ok_or_else(|| contract("owned CT recipe requires planning_order"))?;
    if !(200..=206).contains(&planning_order) {
        return Err(contract(
            "owned CT planning_order is outside reserved range 200..=206",
        ));
    }
    let provider: ClassicCtProviderParameters = decode_object(
        Value::Object(recipe.provider_parameters.clone()),
        "provider_parameters",
    )?;
    validate_provider(&provider)?;
    let dicom = recipe
        .dicom
        .as_ref()
        .ok_or_else(|| contract("owned CT recipe requires DICOM artifacts"))?;
    if dicom.artifacts.is_empty() {
        return Err(contract("owned CT recipe requires at least one artifact"));
    }

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
        recipe_version: crate::PACKAGE_VERSION,
        run_seed: 0,
        file_index: 0,
        frame_index: None,
        referenced_object_index: None,
        role: UidRole::ImplementationClass,
    });
    let mut requests = Vec::with_capacity(dicom.artifacts.len());
    for (expected_order, artifact) in dicom.artifacts.iter().enumerate() {
        if artifact.order as usize != expected_order {
            return Err(contract(
                "CT artifacts must have contiguous zero-based order",
            ));
        }
        validate_artifact_contract(artifact)?;
        let parameters: ClassicCtArtifactParameters = decode_object(
            Value::Object(artifact.parameters.clone()),
            "artifact parameters",
        )?;
        validate_artifact_parameters(&parameters, artifact.order)?;

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
            artifact: &parameters,
            multi_instance_series: dicom.artifacts.len() > 1,
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
                slot: artifact.logical_id.clone(),
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
    if provider.image_type != ["ORIGINAL", "PRIMARY", "AXIAL"] {
        return Err(contract("CT Image Type must be ORIGINAL\\PRIMARY\\AXIAL"));
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
    order: u32,
) -> Result<(), ClassicCtPlanError> {
    if parameters.uid_file_index != order {
        return Err(contract("CT uid_file_index must equal artifact order"));
    }
    if parameters.pixels.rows == 0
        || parameters.pixels.columns == 0
        || parameters.pixels.stored_values.is_empty()
        || parameters.pixels.frame_sha256.len() != 64
    {
        return Err(contract("invalid CT native pixel declaration"));
    }
    Ok(())
}

fn validate_series_contract(
    provider: &ClassicCtProviderParameters,
    requests: &[ClassicInstanceRequest],
) -> Result<(), ClassicCtPlanError> {
    let distinct_series = requests
        .iter()
        .map(|request| request.common.series.series_instance_uid.as_str())
        .collect::<std::collections::BTreeSet<_>>()
        .len();
    match &provider.series_organization {
        Some(_) if distinct_series < 2 => Err(contract(
            "multi-series CT contract must produce distinct Series Instance UIDs",
        )),
        None if distinct_series > 1 => Err(contract(
            "multiple CT series require an explicit series organization contract",
        )),
        _ => Ok(()),
    }
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

fn is_owned_case(case_id: &str) -> bool {
    case_id.starts_with("classic/ct/") || case_id.starts_with("geometry/ct/")
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
