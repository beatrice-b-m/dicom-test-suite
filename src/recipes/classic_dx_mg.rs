//! Plan-only Digital X-ray and mammography recipe providers.

use std::error::Error;
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
    CLASSIC_PIXEL_SLOT, CaseRecipe, ClassicFamilyProvider, ClassicInstanceRequest,
    ClassicPixelRequest, ClassicPlanError, ClassicProjectionFamily, CommonModuleRequest,
    DeclaredVrException, ElementPresence, EquipmentModuleInput, FamilyModuleFragment,
    ImageModuleInput, PatientModuleInput, RescalePlan, SeriesModuleInput, StudyModuleInput,
    WindowPlan,
};

pub const PLAN_PROVIDER_ID: &str = "native.classic_plan";
pub const CONTENT_PROVIDER_ID: &str = "content.native_pixels";
pub const ALGORITHM_PROVIDER_ID: &str = "algorithm.classic_dx_mg";
pub const FIELD_OF_VIEW_DIMENSIONS_VR_CONTRACT_ID: &str =
    "legacy.dx_mg.field_of_view_dimensions.ds";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DxMgFamily {
    Dx,
    Mammography,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DxMgProviderParameters {
    pub family: DxMgFamily,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CodeParameters {
    pub value: String,
    pub scheme: String,
    pub meaning: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShutterParameters {
    pub shape: String,
    pub left_vertical_edge: String,
    pub right_vertical_edge: String,
    pub upper_horizontal_edge: String,
    pub lower_horizontal_edge: String,
    pub presentation_value: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DxMgArtifactParameters {
    pub family: DxMgFamily,
    pub sop_class_uid: String,
    pub modality: String,
    pub study_id: String,
    pub patient_orientation: [String; 2],
    pub body_part_examined: String,
    pub image_laterality: String,
    pub presentation_intent_type: String,
    pub rows: u32,
    pub columns: u32,
    pub photometric_interpretation: PhotometricInterpretation,
    pub stored_values: Vec<i64>,
    pub pixel_min: i64,
    pub pixel_max: i64,
    pub expected_frame_sha256: String,
    pub imager_pixel_spacing: [String; 2],
    pub detector_id: String,
    pub field_of_view_dimensions: [String; 2],
    pub field_of_view_dimensions_vr: String,
    pub window_center: Option<String>,
    pub window_width: Option<String>,
    pub presentation_lut_shape: String,
    pub anatomic_region: CodeParameters,
    pub shutter: Option<ShutterParameters>,
    pub positioner_type: Option<String>,
    pub view_position: Option<String>,
    pub organ_exposed: Option<String>,
    pub breast_implant_present: Option<String>,
    pub view_code: Option<CodeParameters>,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct DxMgFamilyProvider;

impl ClassicFamilyProvider<DxMgArtifactParameters> for DxMgFamilyProvider {
    const PROVIDER_ID: &'static str = "classic.dx_mg";

    fn plan_family(
        &self,
        request: DxMgArtifactParameters,
    ) -> Result<Vec<FamilyModuleFragment>, ClassicPlanError> {
        let operations = family_operations(&request);
        let exception = DeclaredVrException::new(
            "0018,1149",
            DicomVr::DS,
            FIELD_OF_VIEW_DIMENSIONS_VR_CONTRACT_ID,
        )?;
        Ok(vec![FamilyModuleFragment::new_with_declared_vr_exceptions(
            Self::PROVIDER_ID,
            match request.family {
                DxMgFamily::Dx => "digital_x_ray",
                DxMgFamily::Mammography => "digital_mammography",
            },
            operations,
            &[exception],
        )?])
    }
}

/// Inspect the complete stable DX/MG capability without using caller names.
pub(crate) fn inspect_dx_mg_capability(
    recipe: &CaseRecipe,
) -> Result<Option<DxMgArtifactParameters>, ClassicDxMgPlanError> {
    let Some(dicom) = recipe.dicom.as_ref() else {
        return Ok(None);
    };
    let declared = dicom.artifacts.iter().any(|artifact| {
        artifact.algorithm_provider_id.as_deref() == Some(ALGORITHM_PROVIDER_ID)
            || artifact.template.as_ref().is_some_and(|template| {
                matches!(
                    template.template_id.as_str(),
                    "classic/dx/for-presentation"
                        | "classic/mammography/for-presentation"
                        | "classic/mammography/for-processing"
                )
            })
            || artifact
                .classic_projection
                .as_ref()
                .is_some_and(|projection| projection.family == ClassicProjectionFamily::DxMg)
    });
    if !declared {
        return Ok(None);
    }
    if recipe.plan_provider_id != PLAN_PROVIDER_ID {
        return Err(ClassicDxMgPlanError::Contract(format!(
            "{} must use {PLAN_PROVIDER_ID}",
            recipe.binding.case_id
        )));
    }
    let provider: DxMgProviderParameters =
        serde_json::from_value(Value::Object(recipe.provider_parameters.clone()))?;
    recipe.planning_order.ok_or_else(|| {
        ClassicDxMgPlanError::Contract(format!(
            "{} has no global planning_order",
            recipe.binding.case_id
        ))
    })?;
    let artifacts = recipe
        .dicom
        .as_ref()
        .ok_or_else(|| ClassicDxMgPlanError::Contract("owned recipe is not DICOM".into()))?
        .artifacts
        .as_slice();
    if artifacts.len() != 1 {
        return Err(ClassicDxMgPlanError::Contract(format!(
            "{} must declare exactly one artifact",
            recipe.binding.case_id
        )));
    }
    let artifact = &artifacts[0];
    if artifact.logical_id != "instance" || artifact.order != 0 {
        return Err(ClassicDxMgPlanError::Contract(format!(
            "{} must declare logical artifact instance at order zero",
            recipe.binding.case_id
        )));
    }
    if artifact.output.path.is_none() || artifact.output.provider_derived == Some(true) {
        return Err(ClassicDxMgPlanError::Contract(
            "DX/MG output must be explicit".into(),
        ));
    }
    OutputRelativePath::new(artifact.output.path.clone().expect("explicit output"))?;
    if !artifact
        .classic_projection
        .as_ref()
        .is_some_and(|projection| projection.family == ClassicProjectionFamily::DxMg)
        || artifact
            .template
            .as_ref()
            .is_none_or(|template| template.template_version != "1.0.0")
        || !artifact.attribute_operations.is_empty()
        || artifact.secondary_capture.is_some()
        || artifact.metadata_sc.is_some()
        || artifact.nonsquare_geometry.is_some()
    {
        return Err(ClassicDxMgPlanError::Contract(
            "DX/MG requires the complete typed template@1/projection contract".into(),
        ));
    }
    if artifact.content.provider_id != CONTENT_PROVIDER_ID
        || !artifact.content.parameters.is_empty()
    {
        return Err(ClassicDxMgPlanError::Contract(format!(
            "{} must use empty-parameter {CONTENT_PROVIDER_ID}",
            recipe.binding.case_id
        )));
    }
    if artifact.encoding.sequence_length_policy == "provider"
        || artifact.encoding.item_length_policy == "provider"
        || artifact.encoding.offset_table_policy == "provider"
        || artifact.encoding.fragmentation_policy == "provider"
    {
        return Err(ClassicDxMgPlanError::Contract(format!(
            "{} retains an unresolved encoding policy",
            recipe.binding.case_id
        )));
    }
    let parameters: DxMgArtifactParameters =
        serde_json::from_value(Value::Object(artifact.parameters.clone()))?;
    validate_parameters(recipe, artifact, provider.family, &parameters)?;

    Ok(Some(parameters))
}

/// Resolve an inspected DX/MG capability into a filesystem-free request.
pub fn plan_dx_mg_recipe(
    recipe: &CaseRecipe,
    standards_lock_sha256: &str,
    seed: u64,
) -> Result<Option<Vec<ClassicInstanceRequest>>, ClassicDxMgPlanError> {
    let Some(parameters) = inspect_dx_mg_capability(recipe)? else {
        return Ok(None);
    };
    let planning_order = recipe.planning_order.expect("inspected planning order");
    let artifact = &recipe.dicom.as_ref().expect("inspected DICOM").artifacts[0];
    let expected_path = artifact
        .output
        .path
        .clone()
        .expect("inspected explicit path");

    let uid = |role| {
        deterministic_uid(&DeterministicUidInput {
            standards_lock_sha256,
            case_id: &recipe.binding.case_id,
            recipe_version: &recipe.recipe_version,
            run_seed: seed,
            file_index: 0,
            frame_index: None,
            referenced_object_index: None,
            role,
        })
    };
    let study_instance_uid = uid(UidRole::StudyInstance);
    let series_instance_uid = uid(UidRole::SeriesInstance);
    let sop_instance_uid = uid(UidRole::SopInstance);
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
    let family = DxMgFamilyProvider.plan_family(parameters.clone())?;
    let window = match (&parameters.window_center, &parameters.window_width) {
        (Some(center), Some(width)) => Some(WindowPlan {
            center: vec![center.clone()],
            width: vec![width.clone()],
        }),
        (None, None) => None,
        _ => {
            return Err(ClassicDxMgPlanError::Contract(
                "window center and width must be paired".into(),
            ));
        }
    };
    let pixels = NativePixelRequest {
        shape: PixelShape {
            rows: parameters.rows,
            columns: parameters.columns,
            frames: 1,
            samples_per_pixel: 1,
            photometric_interpretation: parameters.photometric_interpretation,
            bits_allocated: 16,
            bits_stored: 12,
            high_bit: 11,
            pixel_representation: 0,
            stored_value_type: StoredValueType::U16,
            byte_order: ByteOrder::Little,
            pixel_data_vr: PixelDataVr::Ow,
            color: None,
        },
        stored_values: parameters.stored_values.clone(),
        declared_pixel_min: parameters.pixel_min,
        declared_pixel_max: parameters.pixel_max,
        expected_frame_sha256: vec![parameters.expected_frame_sha256.clone()],
        signed_stored_bits: Default::default(),
        padding: None,
        palette: None,
    };
    Ok(Some(vec![ClassicInstanceRequest {
        logical_id: artifact.logical_id.clone(),
        order: u64::from(planning_order),
        output_relative_path: OutputRelativePath::new(expected_path)?,
        dependencies: vec![],
        common: common_modules(recipe, &parameters, study_instance_uid, series_instance_uid),
        sop_class_uid: parameters.sop_class_uid,
        sop_instance_uid,
        implementation_class_uid,
        family,
        pixels: ClassicPixelRequest {
            slot: CLASSIC_PIXEL_SLOT.into(),
            pixels,
            rescale: Some(RescalePlan {
                intercept: "0".into(),
                slope: "1".into(),
                rescale_type: ElementPresence::Value("US".into()),
            }),
            window,
        },
    }]))
}

fn validate_parameters(
    recipe: &CaseRecipe,
    artifact: &super::PlannedArtifactRecipe,
    provider_family: DxMgFamily,
    parameters: &DxMgArtifactParameters,
) -> Result<(), ClassicDxMgPlanError> {
    if provider_family != parameters.family {
        return Err(ClassicDxMgPlanError::Contract(
            "provider and artifact family disagree".into(),
        ));
    }
    let (template, modality) = match parameters.family {
        DxMgFamily::Dx => ("classic/dx/for-presentation", "DX"),
        DxMgFamily::Mammography if parameters.presentation_intent_type == "FOR PRESENTATION" => {
            ("classic/mammography/for-presentation", "MG")
        }
        DxMgFamily::Mammography if parameters.presentation_intent_type == "FOR PROCESSING" => {
            ("classic/mammography/for-processing", "MG")
        }
        DxMgFamily::Mammography => {
            return Err(ClassicDxMgPlanError::Contract(
                "unknown mammography presentation intent".into(),
            ));
        }
    };
    if parameters.modality != modality
        || artifact.algorithm_provider_id.as_deref() != Some(ALGORITHM_PROVIDER_ID)
        || artifact
            .template
            .as_ref()
            .map(|value| value.template_id.as_str())
            != Some(template)
    {
        return Err(ClassicDxMgPlanError::Contract(format!(
            "{} family/template/algorithm contract disagrees",
            recipe.binding.case_id
        )));
    }
    if parameters.rows == 0
        || parameters.columns == 0
        || parameters.stored_values.len()
            != usize::try_from(parameters.rows * parameters.columns).unwrap_or(usize::MAX)
        || parameters.pixel_min != parameters.stored_values.iter().copied().min().unwrap_or(0)
        || parameters.pixel_max != parameters.stored_values.iter().copied().max().unwrap_or(0)
        || parameters.expected_frame_sha256.len() != 64
    {
        return Err(ClassicDxMgPlanError::Contract(
            "pixel declaration is incomplete or inconsistent".into(),
        ));
    }
    if parameters.field_of_view_dimensions_vr != "DS" {
        return Err(ClassicDxMgPlanError::Contract(format!(
            "field_of_view_dimensions_vr must declare historical DS contract {FIELD_OF_VIEW_DIMENSIONS_VR_CONTRACT_ID}"
        )));
    }
    match parameters.family {
        DxMgFamily::Dx => {
            if parameters.presentation_intent_type != "FOR PRESENTATION"
                || parameters.photometric_interpretation != PhotometricInterpretation::Monochrome2
                || parameters.shutter.is_none()
                || parameters.positioner_type.is_some()
                || parameters.view_code.is_some()
            {
                return Err(ClassicDxMgPlanError::Contract(
                    "DX presentation/shutter declaration is inconsistent".into(),
                ));
            }
        }
        DxMgFamily::Mammography => {
            let presentation = parameters.presentation_intent_type == "FOR PRESENTATION";
            if parameters.shutter.is_some()
                || parameters.positioner_type.as_deref() != Some("MAMMOGRAPHIC")
                || parameters.view_code.is_none()
                || (presentation
                    && (parameters.photometric_interpretation
                        != PhotometricInterpretation::Monochrome1
                        || parameters.presentation_lut_shape != "INVERSE"
                        || parameters.window_center.is_none()))
                || (!presentation
                    && (parameters.photometric_interpretation
                        != PhotometricInterpretation::Monochrome2
                        || parameters.presentation_lut_shape != "IDENTITY"
                        || parameters.window_center.is_some()))
            {
                return Err(ClassicDxMgPlanError::Contract(
                    "mammography presentation/processing semantics are inconsistent".into(),
                ));
            }
        }
    }
    Ok(())
}

fn common_modules(
    recipe: &CaseRecipe,
    parameters: &DxMgArtifactParameters,
    study_instance_uid: String,
    series_instance_uid: String,
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
            study_instance_uid,
            study_date: ElementPresence::Value("20260101".into()),
            study_time: ElementPresence::Value("000000".into()),
            accession_number: ElementPresence::Empty,
            referring_physician_name: ElementPresence::Empty,
            study_id: ElementPresence::Value(parameters.study_id.clone()),
        },
        series: SeriesModuleInput {
            modality: parameters.modality.clone(),
            series_instance_uid,
            series_number: ElementPresence::Value("1".into()),
            series_date: ElementPresence::Omitted,
            series_time: ElementPresence::Omitted,
        },
        frame_of_reference: None,
        equipment: EquipmentModuleInput {
            manufacturer: ElementPresence::Value("dicom-test-suite".into()),
            manufacturer_model_name: ElementPresence::Value(recipe.recipe_id.clone()),
            software_versions: ElementPresence::Value(crate::BYTE_STABLE_OUTPUT_VERSION.into()),
        },
        image: ImageModuleInput {
            instance_number: ElementPresence::Value("1".into()),
            patient_orientation: ElementPresence::Value(parameters.patient_orientation.to_vec()),
            content_date: ElementPresence::Value("20260101".into()),
            content_time: ElementPresence::Value("000000".into()),
        },
    }
}

fn family_operations(parameters: &DxMgArtifactParameters) -> Vec<AttributeOperation> {
    let mut operations = vec![
        string("0008,001C", DicomVr::CS, "YES"),
        string(
            "0008,0068",
            DicomVr::CS,
            &parameters.presentation_intent_type,
        ),
        string("0020,0012", DicomVr::IS, "1"),
        string("0008,0022", DicomVr::DA, "20260101"),
        string("0008,0032", DicomVr::TM, "000000"),
        multi("0008,0008", DicomVr::CS, ["ORIGINAL", "PRIMARY"]),
        string("0018,0015", DicomVr::CS, &parameters.body_part_examined),
        string("0020,0062", DicomVr::CS, &parameters.image_laterality),
        string("0028,1040", DicomVr::CS, "LIN"),
        signed("0028,1041", DicomVr::SS, -1),
        string("2050,0020", DicomVr::CS, &parameters.presentation_lut_shape),
        string("0028,2110", DicomVr::CS, "00"),
        string("0028,0301", DicomVr::CS, "NO"),
        multi(
            "0018,1164",
            DicomVr::DS,
            parameters.imager_pixel_spacing.iter().map(String::as_str),
        ),
        string("0018,7004", DicomVr::CS, "DIRECT"),
        string("0018,7005", DicomVr::CS, "AREA"),
        string("0018,7006", DicomVr::LT, "synthetic detector"),
        string("0018,700A", DicomVr::SH, &parameters.detector_id),
        multi(
            "0018,7022",
            DicomVr::DS,
            parameters.imager_pixel_spacing.iter().map(String::as_str),
        ),
        string("0018,1147", DicomVr::CS, "RECTANGLE"),
        multi(
            "0018,1149",
            DicomVr::DS,
            parameters
                .field_of_view_dimensions
                .iter()
                .map(String::as_str),
        ),
        code_sequence("0008,2218", &parameters.anatomic_region),
        sequence("0040,0555", Vec::new()),
    ];
    if let Some(shutter) = &parameters.shutter {
        operations.extend([
            string("0018,1600", DicomVr::CS, &shutter.shape),
            string("0018,1602", DicomVr::IS, &shutter.left_vertical_edge),
            string("0018,1604", DicomVr::IS, &shutter.right_vertical_edge),
            string("0018,1606", DicomVr::IS, &shutter.upper_horizontal_edge),
            string("0018,1608", DicomVr::IS, &shutter.lower_horizontal_edge),
            unsigned(
                "0018,1622",
                DicomVr::US,
                u64::from(shutter.presentation_value),
            ),
        ]);
    }
    if let Some(value) = &parameters.positioner_type {
        operations.push(string("0018,1508", DicomVr::CS, value));
    }
    if let Some(value) = &parameters.view_position {
        operations.push(string("0018,5101", DicomVr::CS, value));
    }
    if let Some(value) = &parameters.organ_exposed {
        operations.push(string("0040,0318", DicomVr::CS, value));
    }
    if let Some(value) = &parameters.breast_implant_present {
        operations.push(string("0028,1300", DicomVr::CS, value));
    }
    if let Some(code) = &parameters.view_code {
        operations.push(code_sequence("0054,0220", code));
    }
    operations
}

fn address(tag: &str) -> AttributeAddress {
    AttributeAddress::from_normalized_tag(tag).expect("DX/MG tag is valid")
}

fn string(tag: &str, vr: DicomVr, value: impl Into<String>) -> AttributeOperation {
    AttributeOperation::Set {
        address: address(tag),
        vr,
        value: AttributeValue::Primitive(PrimitiveValue::String(value.into())),
    }
}

fn multi<'a>(
    tag: &str,
    vr: DicomVr,
    values: impl IntoIterator<Item = &'a str>,
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

fn signed(tag: &str, vr: DicomVr, value: i64) -> AttributeOperation {
    AttributeOperation::Set {
        address: address(tag),
        vr,
        value: AttributeValue::Primitive(PrimitiveValue::Signed(value)),
    }
}

fn unsigned(tag: &str, vr: DicomVr, value: u64) -> AttributeOperation {
    AttributeOperation::Set {
        address: address(tag),
        vr,
        value: AttributeValue::Primitive(PrimitiveValue::Unsigned(value)),
    }
}

fn sequence(tag: &str, items: Vec<AttributeItem>) -> AttributeOperation {
    AttributeOperation::Set {
        address: address(tag),
        vr: DicomVr::SQ,
        value: AttributeValue::Sequence(items),
    }
}

fn code_sequence(tag: &str, code: &CodeParameters) -> AttributeOperation {
    sequence(
        tag,
        vec![AttributeItem {
            attributes: vec![
                string("0008,0100", DicomVr::SH, &code.value),
                string("0008,0102", DicomVr::SH, &code.scheme),
                string("0008,0104", DicomVr::LO, &code.meaning),
            ],
        }],
    )
}

#[derive(Debug)]
pub enum ClassicDxMgPlanError {
    Contract(String),
    Parameters(serde_json::Error),
    Classic(ClassicPlanError),
    Output(crate::corpus_plan::CorpusPlanError),
}

impl fmt::Display for ClassicDxMgPlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Contract(message) => formatter.write_str(message),
            Self::Parameters(error) => {
                write!(formatter, "invalid DX/MG recipe parameters: {error}")
            }
            Self::Classic(error) => write!(formatter, "invalid DX/MG classic plan: {error}"),
            Self::Output(error) => write!(formatter, "invalid DX/MG output path: {error}"),
        }
    }
}

impl Error for ClassicDxMgPlanError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Parameters(error) => Some(error),
            Self::Classic(error) => Some(error),
            Self::Output(error) => Some(error),
            Self::Contract(_) => None,
        }
    }
}

impl From<serde_json::Error> for ClassicDxMgPlanError {
    fn from(value: serde_json::Error) -> Self {
        Self::Parameters(value)
    }
}

impl From<ClassicPlanError> for ClassicDxMgPlanError {
    fn from(value: ClassicPlanError) -> Self {
        Self::Classic(value)
    }
}

impl From<crate::corpus_plan::CorpusPlanError> for ClassicDxMgPlanError {
    fn from(value: crate::corpus_plan::CorpusPlanError) -> Self {
        Self::Output(value)
    }
}
