//! Typed many-instance CT stress planning on the shared classic CT contracts.

use std::error::Error;
use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::composition::{
    AttributeAddress, AttributeOperation, AttributeValue, DicomVr, PrimitiveValue,
};
use crate::corpus_plan::{ArtifactResourceEstimate, OutputRelativePath};
use crate::native_pixel::{
    ByteOrder, NativePixelRequest, PhotometricInterpretation, PixelDataVr, PixelShape,
    StoredValueType,
};
use crate::{DeterministicUidInput, UidRole, deterministic_uid, sha256_hex};

use super::{
    CLASSIC_PIXEL_SLOT, CaseRecipe, ClassicInstanceRequest, ClassicPixelRequest,
    CommonModuleRequest, ElementPresence, EquipmentModuleInput, FamilyModuleFragment,
    FrameOfReferenceModuleInput, ImageModuleInput, PatientModuleInput, ReducedStressPolicy,
    RescalePlan, SeriesModuleInput, StudyModuleInput, WindowPlan,
};

pub const STRESS_CT_PLAN_PROVIDER_ID: &str = "native.stress_ct_plan";
pub const STRESS_CT_ALGORITHM_PROVIDER_ID: &str = "algorithm.stress_ct";
const CT_IMAGE_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.2";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StressCtParameters {
    pub instances: u32,
    pub rows: u32,
    pub columns: u32,
    pub pixel_modulus: u32,
    pub pixel_offset: i32,
    pub pixel_min: i64,
    pub pixel_max: i64,
    pub slice_spacing_mm: String,
    pub pixel_spacing_mm: [String; 2],
    pub policy: ReducedStressPolicy,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StressCtArtifactParameters {
    pub uid_file_index: u32,
    pub instance_number: String,
    pub image_position_patient: [String; 3],
    pub position_along_normal: f64,
}

#[derive(Debug, Clone)]
pub struct StressCtPlanOutput {
    pub requests: Vec<ClassicInstanceRequest>,
    pub resources: Vec<ArtifactResourceEstimate>,
    pub policy: ReducedStressPolicy,
}

pub fn plan_stress_ct_recipe(
    recipe: &CaseRecipe,
    standards_lock_sha256: &str,
    seed: u64,
) -> Result<Option<StressCtPlanOutput>, StressCtPlanError> {
    if recipe.binding.case_id != "stress/study/high_instance_count_ct" {
        return Ok(None);
    }
    if recipe.plan_provider_id != STRESS_CT_PLAN_PROVIDER_ID || recipe.planning_order != Some(1400)
    {
        return Err(contract("stress CT provider or planning order mismatch"));
    }
    let parameters: StressCtParameters =
        serde_json::from_value(Value::Object(recipe.provider_parameters.clone()))
            .map_err(|error| StressCtPlanError::Parameters(error.to_string()))?;
    validate_parameters(&parameters)?;
    let artifacts = &recipe
        .dicom
        .as_ref()
        .ok_or_else(|| contract("stress CT recipe requires artifacts"))?
        .artifacts;
    if artifacts.len() != parameters.instances as usize {
        return Err(contract(
            "stress CT artifact cardinality differs from instances",
        ));
    }

    let stored_values = (0..parameters.rows * parameters.columns)
        .map(|index| i64::from((index % parameters.pixel_modulus) as i32 + parameters.pixel_offset))
        .collect::<Vec<_>>();
    let mut native_bytes = Vec::with_capacity(stored_values.len() * 2);
    for value in &stored_values {
        let stored_word = (*value as u64 & 0x0fff) as u16;
        native_bytes.extend_from_slice(&stored_word.to_le_bytes());
    }
    let frame_sha256 = sha256_hex(&native_bytes);
    let study_uid = uid(
        recipe,
        standards_lock_sha256,
        seed,
        UidRole::StudyInstance,
        0,
    );
    let series_uid = uid(
        recipe,
        standards_lock_sha256,
        seed,
        UidRole::SeriesInstance,
        0,
    );
    let frame_uid = uid(
        recipe,
        standards_lock_sha256,
        seed,
        UidRole::FrameOfReference,
        0,
    );
    let implementation_uid = deterministic_uid(&DeterministicUidInput {
        standards_lock_sha256,
        case_id: "dicom-test-suite/implementation",
        recipe_version: crate::PACKAGE_VERSION,
        run_seed: 0,
        file_index: 0,
        frame_index: None,
        referenced_object_index: None,
        role: UidRole::ImplementationClass,
    });
    let mut requests = Vec::with_capacity(artifacts.len());
    let mut resources = Vec::with_capacity(artifacts.len());
    for (index, artifact) in artifacts.iter().enumerate() {
        if artifact.order as usize != index
            || artifact.logical_id != format!("slice_{:03}", index + 1)
            || artifact.content.provider_id != "content.native_pixels"
            || artifact.algorithm_provider_id.as_deref() != Some(STRESS_CT_ALGORITHM_PROVIDER_ID)
            || artifact
                .template
                .as_ref()
                .map(|value| value.template_id.as_str())
                != Some("classic/ct")
            || artifact.encoding.transfer_syntax_uid != "1.2.840.10008.1.2.1"
            || !artifact.attribute_operations.is_empty()
        {
            return Err(contract("stress CT artifact contract mismatch"));
        }
        let artifact_parameters: StressCtArtifactParameters =
            serde_json::from_value(Value::Object(artifact.parameters.clone()))
                .map_err(|error| StressCtPlanError::Parameters(error.to_string()))?;
        if artifact_parameters.uid_file_index != artifact.order
            || artifact_parameters.instance_number != (index + 1).to_string()
            || artifact_parameters.position_along_normal != index as f64 * 2.5
        {
            return Err(contract("stress CT slice order or position mismatch"));
        }
        requests.push(ClassicInstanceRequest {
            logical_id: artifact.logical_id.clone(),
            order: artifact.order.into(),
            output_relative_path: OutputRelativePath::new(
                artifact
                    .output
                    .path
                    .clone()
                    .ok_or_else(|| contract("missing CT path"))?,
            )
            .map_err(|error| contract(error.to_string()))?,
            dependencies: Vec::new(),
            common: common(
                recipe,
                &study_uid,
                &series_uid,
                &frame_uid,
                &artifact_parameters.instance_number,
            ),
            sop_class_uid: CT_IMAGE_STORAGE.into(),
            sop_instance_uid: uid(
                recipe,
                standards_lock_sha256,
                seed,
                UidRole::SopInstance,
                artifact.order,
            ),
            implementation_class_uid: implementation_uid.clone(),
            family: family(&parameters, &artifact_parameters)?,
            pixels: ClassicPixelRequest {
                slot: CLASSIC_PIXEL_SLOT.into(),
                pixels: NativePixelRequest {
                    shape: PixelShape {
                        rows: parameters.rows,
                        columns: parameters.columns,
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
                    stored_values: stored_values.clone(),
                    declared_pixel_min: parameters.pixel_min,
                    declared_pixel_max: parameters.pixel_max,
                    expected_frame_sha256: vec![frame_sha256.clone()],
                    padding: None,
                    palette: None,
                },
                rescale: Some(RescalePlan {
                    intercept: "-1024".into(),
                    slope: "1".into(),
                    rescale_type: ElementPresence::Value("HU".into()),
                }),
                window: Some(WindowPlan {
                    center: vec!["40".into()],
                    width: vec!["400".into()],
                }),
            },
        });
        let native_byte_count =
            u64::try_from(native_bytes.len()).map_err(|_| StressCtPlanError::ResourceOverflow)?;
        resources.push(ArtifactResourceEstimate {
            output_bytes: native_byte_count
                .checked_add(4096)
                .ok_or(StressCtPlanError::ResourceOverflow)?,
            peak_working_bytes: native_byte_count
                .checked_mul(2)
                .ok_or(StressCtPlanError::ResourceOverflow)?,
        });
    }
    Ok(Some(StressCtPlanOutput {
        requests,
        resources,
        policy: parameters.policy,
    }))
}

fn validate_parameters(parameters: &StressCtParameters) -> Result<(), StressCtPlanError> {
    if parameters.instances != 128
        || parameters.rows != 64
        || parameters.columns != 64
        || parameters.pixel_modulus != 3072
        || parameters.pixel_offset != -1024
        || parameters.pixel_min != -1024
        || parameters.pixel_max != 2047
        || parameters.slice_spacing_mm != "2.5"
        || parameters.pixel_spacing_mm != ["0.75", "0.75"]
        || parameters.policy.qualification_scale != "reduced"
        || parameters.policy.full_scale_available
        || parameters.policy.full_scale_reason.is_empty()
    {
        return Err(contract(
            "stress CT reduced-scale parameters differ from qualification",
        ));
    }
    Ok(())
}

fn common(
    recipe: &CaseRecipe,
    study_uid: &str,
    series_uid: &str,
    frame_uid: &str,
    instance_number: &str,
) -> CommonModuleRequest {
    CommonModuleRequest {
        patient: PatientModuleInput {
            specific_character_set: ElementPresence::Omitted,
            patient_name: ElementPresence::Value("DTS^Synthetic^Patient001".into()),
            patient_id: ElementPresence::Value("DTS-PATIENT-001".into()),
            patient_birth_date: ElementPresence::Value("19700101".into()),
            patient_sex: ElementPresence::Value("O".into()),
        },
        study: StudyModuleInput {
            study_instance_uid: study_uid.into(),
            study_date: ElementPresence::Value("20260101".into()),
            study_time: ElementPresence::Value("000000".into()),
            accession_number: ElementPresence::Empty,
            referring_physician_name: ElementPresence::Empty,
            study_id: ElementPresence::Value("DTS-CT".into()),
        },
        series: SeriesModuleInput {
            modality: "CT".into(),
            series_instance_uid: series_uid.into(),
            series_number: ElementPresence::Value("1".into()),
            series_date: ElementPresence::Omitted,
            series_time: ElementPresence::Omitted,
        },
        frame_of_reference: Some(FrameOfReferenceModuleInput {
            frame_of_reference_uid: frame_uid.into(),
            position_reference_indicator: ElementPresence::Empty,
        }),
        equipment: EquipmentModuleInput {
            manufacturer: ElementPresence::Value("dicom-test-suite".into()),
            manufacturer_model_name: ElementPresence::Value(recipe.recipe_id.clone()),
            software_versions: ElementPresence::Value(crate::PACKAGE_VERSION.into()),
        },
        image: ImageModuleInput {
            instance_number: ElementPresence::Value(instance_number.into()),
            patient_orientation: ElementPresence::Empty,
            content_date: ElementPresence::Value("20260101".into()),
            content_time: ElementPresence::Value("000000".into()),
        },
    }
}

fn family(
    parameters: &StressCtParameters,
    artifact: &StressCtArtifactParameters,
) -> Result<Vec<FamilyModuleFragment>, StressCtPlanError> {
    Ok(vec![
        FamilyModuleFragment::new(
            STRESS_CT_ALGORITHM_PROVIDER_ID,
            "general_ct_image",
            vec![
                set("0008,001C", DicomVr::CS, "YES"),
                multi("0008,0008", DicomVr::CS, ["ORIGINAL", "PRIMARY", "AXIAL"]),
                set("0020,0012", DicomVr::IS, "1"),
                set("0008,0022", DicomVr::DA, "20260101"),
                set("0008,0032", DicomVr::TM, "000000"),
                empty("0018,5100"),
                set("0020,0062", DicomVr::CS, "U"),
            ],
        )?,
        FamilyModuleFragment::new(
            STRESS_CT_ALGORITHM_PROVIDER_ID,
            "ct_geometry",
            vec![
                multi(
                    "0028,0030",
                    DicomVr::DS,
                    parameters.pixel_spacing_mm.clone(),
                ),
                multi("0020,0037", DicomVr::DS, ["1", "0", "0", "0", "1", "0"]),
                multi(
                    "0020,0032",
                    DicomVr::DS,
                    artifact.image_position_patient.clone(),
                ),
                set("0018,0050", DicomVr::DS, "2.5"),
                set("0018,0088", DicomVr::DS, "2.5"),
            ],
        )?,
        FamilyModuleFragment::new(
            STRESS_CT_ALGORITHM_PROVIDER_ID,
            "ct_acquisition",
            vec![set("0018,0060", DicomVr::DS, "120")],
        )?,
    ])
}

fn set(tag: &str, vr: DicomVr, value: impl Into<String>) -> AttributeOperation {
    AttributeOperation::Set {
        address: address(tag),
        vr,
        value: AttributeValue::Primitive(PrimitiveValue::String(value.into())),
    }
}

fn multi<const N: usize>(
    tag: &str,
    vr: DicomVr,
    values: [impl Into<String>; N],
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

fn empty(tag: &str) -> AttributeOperation {
    AttributeOperation::Empty {
        address: address(tag),
    }
}

fn address(tag: &str) -> AttributeAddress {
    AttributeAddress::from_normalized_tag(tag).expect("stress CT tag is valid")
}

fn uid(recipe: &CaseRecipe, lock: &str, seed: u64, role: UidRole, index: u32) -> String {
    deterministic_uid(&DeterministicUidInput {
        standards_lock_sha256: lock,
        case_id: &recipe.binding.case_id,
        recipe_version: &recipe.recipe_version,
        run_seed: seed,
        file_index: index,
        frame_index: None,
        referenced_object_index: None,
        role,
    })
}

fn contract(message: impl Into<String>) -> StressCtPlanError {
    StressCtPlanError::Contract(message.into())
}

#[derive(Debug)]
pub enum StressCtPlanError {
    Contract(String),
    Parameters(String),
    ResourceOverflow,
    Classic(super::ClassicPlanError),
}

impl fmt::Display for StressCtPlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Contract(message) | Self::Parameters(message) => formatter.write_str(message),
            Self::ResourceOverflow => formatter.write_str("stress CT resource overflow"),
            Self::Classic(error) => write!(formatter, "classic planning failed: {error}"),
        }
    }
}

impl Error for StressCtPlanError {}

impl From<super::ClassicPlanError> for StressCtPlanError {
    fn from(value: super::ClassicPlanError) -> Self {
        Self::Classic(value)
    }
}
