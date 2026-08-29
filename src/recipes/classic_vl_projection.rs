//! Plan-only providers for ordinary single-frame visible-light and classic
//! XA/XRF projection recipes.

use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{
    CLASSIC_PIXEL_SLOT, CaseRecipe, ClassicInstanceRequest, ClassicPixelRequest, ClassicPlanError,
    CommonModuleRequest, ElementPresence, EquipmentModuleInput, FamilyModuleFragment,
    ImageModuleInput, PatientModuleInput, SeriesModuleInput, StudyModuleInput,
};
use crate::composition::{
    AttributeAddress, AttributeOperation, AttributeValue, DicomVr, PrimitiveValue,
};
use crate::corpus_plan::OutputRelativePath;
use crate::native_pixel::{
    ByteOrder, ChromaSubsampling, ColorOrganization, NativePixelRequest, Palette,
    PhotometricInterpretation, PixelDataVr, PixelShape, StoredValueType,
};
use crate::{DeterministicUidInput, UidRole, deterministic_uid};

const PLAN_PROVIDER_ID: &str = "native.classic_plan";
const CONTENT_PROVIDER_ID: &str = "content.native_pixels";
const ALGORITHM_ID: &str = "algorithm.classic_vl_projection";
const EXPLICIT_VR_LE: &str = "1.2.840.10008.1.2.1";
const RLE_LOSSLESS: &str = "1.2.840.10008.1.2.5";
const ENDOSCOPIC_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.77.1.1";
const MICROSCOPIC_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.77.1.2";
const PHOTOGRAPHIC_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.77.1.4";
const XA_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.12.1";
const XRF_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.12.2";

const OWNED_CASES: [(&str, u32); 10] = [
    ("vl/photo/rgb_planar0_explicit_le", 600),
    ("vl/endoscopic/rgb_explicit_le", 601),
    ("vl/microscopic/rgb_explicit_le", 602),
    ("vl/photo/rgb_icc_profile_explicit_le", 603),
    ("vl/photo/palette_color_explicit_le", 604),
    ("vl/photo/rgb_planar0_rle_lossless", 605),
    ("vl/photo/rgb_planar1_rle_lossless", 606),
    ("vl/photo/palette_color_rle_lossless", 607),
    ("classic/xa/monoplane_explicit_le", 608),
    ("classic/xrf/monoplane_explicit_le", 609),
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VlProviderParameters {
    pub patient_name: String,
    pub patient_id: String,
    pub patient_birth_date: String,
    pub patient_sex: String,
    pub study_date: String,
    pub study_time: String,
    pub study_id: String,
    pub manufacturer: String,
    pub software_versions: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VlArtifactParameters {
    pub uid_file_index: u32,
    pub modality: String,
    pub sop_class_uid: String,
    pub rows: u32,
    pub columns: u32,
    pub samples_per_pixel: u16,
    pub photometric_interpretation: VlPhotometricInterpretation,
    #[serde(default)]
    pub planar_configuration: Option<u8>,
    pub stored_values: Vec<i64>,
    pub pixel_min: i64,
    pub pixel_max: i64,
    pub frame_sha256: String,
    #[serde(default)]
    pub palette: Option<Palette>,
    #[serde(default)]
    pub body_part_examined: Option<String>,
    #[serde(default)]
    pub laterality: Option<String>,
    #[serde(default)]
    pub icc_profile_hex: Option<String>,
    #[serde(default)]
    pub icc_profile_sha256: Option<String>,
    #[serde(default)]
    pub color_space: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VlPhotometricInterpretation {
    #[serde(rename = "RGB")]
    Rgb,
    #[serde(rename = "PALETTE_COLOR", alias = "PALETTE COLOR")]
    PaletteColor,
}

impl VlPhotometricInterpretation {
    fn native(self) -> PhotometricInterpretation {
        match self {
            Self::Rgb => PhotometricInterpretation::Rgb,
            Self::PaletteColor => PhotometricInterpretation::PaletteColor,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectionProviderParameters {
    pub patient_name: String,
    pub patient_id: String,
    pub patient_birth_date: String,
    pub patient_sex: String,
    pub study_date: String,
    pub study_time: String,
    pub study_id: String,
    pub manufacturer: String,
    pub software_versions: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectionArtifactParameters {
    pub modality: String,
    pub sop_class_uid: String,
    pub rows: u32,
    pub columns: u32,
    pub stored_values: Vec<i64>,
    pub pixel_min: i64,
    pub pixel_max: i64,
    pub frame_sha256: String,
    pub image_type: Vec<String>,
    pub body_part_examined: String,
    pub pixel_intensity_relationship: String,
    pub lossy_image_compression: String,
    pub kvp: String,
    pub radiation_setting: String,
    pub exposure: String,
    pub imager_pixel_spacing: [String; 2],
    pub distance_source_to_detector: String,
    pub distance_source_to_patient: String,
    pub estimated_magnification_factor: String,
    #[serde(default)]
    pub positioner_primary_angle: Option<String>,
    #[serde(default)]
    pub positioner_secondary_angle: Option<String>,
    #[serde(default)]
    pub column_angulation: Option<String>,
    pub non_claims: ProjectionNonClaims,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectionNonClaims {
    pub laterality_present: bool,
    pub multiframe_cine: bool,
    pub biplane_data_present: bool,
    pub contrast_used: bool,
    pub subtraction_applied: bool,
    pub table_position_present: bool,
    pub table_motion_present: bool,
    pub table_tilt_present: bool,
    pub tomography_present: bool,
    pub patient_space_geometry_present: bool,
    pub pixel_spacing_calibrated: bool,
    pub xa_positioner_angles_present: bool,
}

pub fn plan_vl_projection_recipe(
    recipe: &CaseRecipe,
    standards_lock_sha256: &str,
    seed: u64,
) -> Result<Option<Vec<ClassicInstanceRequest>>, ClassicVlProjectionPlanError> {
    let Some((_, expected_planning_order)) = OWNED_CASES
        .iter()
        .find(|(case_id, _)| *case_id == recipe.binding.case_id)
    else {
        return Ok(None);
    };
    validate_recipe_contract(recipe, *expected_planning_order)?;
    let artifact = recipe
        .dicom
        .as_ref()
        .and_then(|dicom| dicom.artifacts.first())
        .ok_or_else(|| contract("owned recipe requires one DICOM artifact"))?;
    let request = if recipe.binding.case_id.starts_with("vl/") {
        plan_vl(recipe, artifact, standards_lock_sha256, seed)?
    } else {
        plan_projection(recipe, artifact, standards_lock_sha256, seed)?
    };
    Ok(Some(vec![request]))
}

fn plan_vl(
    recipe: &CaseRecipe,
    artifact: &super::PlannedArtifactRecipe,
    standards_lock_sha256: &str,
    seed: u64,
) -> Result<ClassicInstanceRequest, ClassicVlProjectionPlanError> {
    let provider: VlProviderParameters = decode(Value::Object(recipe.provider_parameters.clone()))?;
    let parameters: VlArtifactParameters = decode(Value::Object(artifact.parameters.clone()))?;
    validate_vl_parameters(&parameters)?;
    let icc_profile = parameters
        .icc_profile_hex
        .as_deref()
        .map(decode_hex)
        .transpose()?;
    if let (Some(bytes), Some(expected)) = (&icc_profile, &parameters.icc_profile_sha256) {
        if crate::sha256_hex(bytes) != *expected {
            return Err(contract("ICC profile hash does not match its declaration"));
        }
    } else if icc_profile.is_some() != parameters.icc_profile_sha256.is_some() {
        return Err(contract(
            "ICC profile bytes and hash must be declared together",
        ));
    }
    let mut general = vec![
        set_string("0008,001C", DicomVr::CS, "YES"),
        set_multi_string("0008,0008", DicomVr::CS, ["ORIGINAL", "PRIMARY"]),
        set_string("0028,2110", DicomVr::CS, "00"),
        AttributeOperation::Set {
            address: address("0040,0555"),
            vr: DicomVr::SQ,
            value: AttributeValue::Sequence(vec![]),
        },
    ];
    if let Some(value) = &parameters.body_part_examined {
        general.push(set_string("0018,0015", DicomVr::CS, value));
    }
    if let Some(value) = &parameters.laterality {
        general.push(set_string("0020,0060", DicomVr::CS, value));
    }
    if let Some(bytes) = icc_profile {
        general.push(set_binary("0028,2000", DicomVr::OB, bytes));
        general.push(set_string(
            "0028,2002",
            DicomVr::CS,
            parameters
                .color_space
                .as_deref()
                .ok_or_else(|| contract("ICC profile requires color_space"))?,
        ));
    } else if parameters.color_space.is_some() {
        return Err(contract("color_space requires an ICC profile"));
    }
    if let Some(palette) = &parameters.palette {
        general.extend(palette_operations(palette));
    }
    Ok(ClassicInstanceRequest {
        logical_id: artifact.logical_id.clone(),
        order: artifact.order.into(),
        output_relative_path: output_path(artifact)?,
        dependencies: vec![],
        common: common_modules(
            recipe,
            &provider.patient_name,
            &provider.patient_id,
            &provider.patient_birth_date,
            &provider.patient_sex,
            &provider.study_date,
            &provider.study_time,
            &provider.study_id,
            &provider.manufacturer,
            &provider.software_versions,
            &parameters.modality,
            standards_lock_sha256,
            seed,
            0,
        ),
        sop_class_uid: parameters.sop_class_uid.clone(),
        sop_instance_uid: uid(recipe, standards_lock_sha256, seed, 0, UidRole::SopInstance),
        implementation_class_uid: implementation_uid(standards_lock_sha256),
        family: vec![FamilyModuleFragment::new(
            ALGORITHM_ID,
            "vl_image",
            general,
        )?],
        pixels: ClassicPixelRequest {
            slot: CLASSIC_PIXEL_SLOT.into(),
            pixels: native_request(&parameters),
            rescale: None,
            window: None,
        },
    })
}

fn plan_projection(
    recipe: &CaseRecipe,
    artifact: &super::PlannedArtifactRecipe,
    standards_lock_sha256: &str,
    seed: u64,
) -> Result<ClassicInstanceRequest, ClassicVlProjectionPlanError> {
    let provider: ProjectionProviderParameters =
        decode(Value::Object(recipe.provider_parameters.clone()))?;
    let parameters: ProjectionArtifactParameters =
        decode(Value::Object(artifact.parameters.clone()))?;
    validate_projection_parameters(recipe, &parameters)?;
    let mut operations = vec![
        set_string("0008,001C", DicomVr::CS, "YES"),
        set_multi_string("0008,0008", DicomVr::CS, parameters.image_type.clone()),
        set_string("0018,0015", DicomVr::CS, &parameters.body_part_examined),
        set_string("0020,0012", DicomVr::IS, "1"),
        set_string("0008,0022", DicomVr::DA, "20260101"),
        set_string("0008,0032", DicomVr::TM, "000000"),
        set_string(
            "0028,1040",
            DicomVr::CS,
            &parameters.pixel_intensity_relationship,
        ),
        set_string(
            "0028,2110",
            DicomVr::CS,
            &parameters.lossy_image_compression,
        ),
        set_string("0018,0060", DicomVr::DS, &parameters.kvp),
        set_string("0018,1155", DicomVr::CS, &parameters.radiation_setting),
        set_string("0018,1152", DicomVr::IS, &parameters.exposure),
        set_multi_string(
            "0018,1164",
            DicomVr::DS,
            parameters.imager_pixel_spacing.clone(),
        ),
        set_string(
            "0018,1110",
            DicomVr::DS,
            &parameters.distance_source_to_detector,
        ),
        set_string(
            "0018,1111",
            DicomVr::DS,
            &parameters.distance_source_to_patient,
        ),
        set_string(
            "0018,1114",
            DicomVr::DS,
            &parameters.estimated_magnification_factor,
        ),
    ];
    if let Some(value) = &parameters.positioner_primary_angle {
        operations.push(set_string("0018,1510", DicomVr::DS, value));
    }
    if let Some(value) = &parameters.positioner_secondary_angle {
        operations.push(set_string("0018,1511", DicomVr::DS, value));
    }
    if let Some(value) = &parameters.column_angulation {
        operations.push(set_string("0018,1450", DicomVr::DS, value));
    }
    Ok(ClassicInstanceRequest {
        logical_id: artifact.logical_id.clone(),
        order: artifact.order.into(),
        output_relative_path: output_path(artifact)?,
        dependencies: vec![],
        common: common_modules(
            recipe,
            &provider.patient_name,
            &provider.patient_id,
            &provider.patient_birth_date,
            &provider.patient_sex,
            &provider.study_date,
            &provider.study_time,
            &provider.study_id,
            &provider.manufacturer,
            &provider.software_versions,
            &parameters.modality,
            standards_lock_sha256,
            seed,
            0,
        ),
        sop_class_uid: parameters.sop_class_uid.clone(),
        sop_instance_uid: uid(recipe, standards_lock_sha256, seed, 0, UidRole::SopInstance),
        implementation_class_uid: implementation_uid(standards_lock_sha256),
        family: vec![FamilyModuleFragment::new(
            ALGORITHM_ID,
            "projection_image",
            operations,
        )?],
        pixels: ClassicPixelRequest {
            slot: CLASSIC_PIXEL_SLOT.into(),
            pixels: NativePixelRequest {
                shape: PixelShape {
                    rows: parameters.rows,
                    columns: parameters.columns,
                    frames: 1,
                    samples_per_pixel: 1,
                    photometric_interpretation: PhotometricInterpretation::Monochrome2,
                    bits_allocated: 8,
                    bits_stored: 8,
                    high_bit: 7,
                    pixel_representation: 0,
                    stored_value_type: StoredValueType::U8,
                    byte_order: ByteOrder::Little,
                    pixel_data_vr: PixelDataVr::Ob,
                    color: None,
                },
                stored_values: parameters.stored_values,
                declared_pixel_min: parameters.pixel_min,
                declared_pixel_max: parameters.pixel_max,
                expected_frame_sha256: vec![parameters.frame_sha256],
                padding: None,
                palette: None,
            },
            rescale: None,
            window: None,
        },
    })
}

#[allow(clippy::too_many_arguments)]
fn common_modules(
    recipe: &CaseRecipe,
    patient_name: &str,
    patient_id: &str,
    patient_birth_date: &str,
    patient_sex: &str,
    study_date: &str,
    study_time: &str,
    study_id: &str,
    manufacturer: &str,
    software_versions: &str,
    modality: &str,
    standards_lock_sha256: &str,
    seed: u64,
    file_index: u32,
) -> CommonModuleRequest {
    CommonModuleRequest {
        patient: PatientModuleInput {
            specific_character_set: ElementPresence::Omitted,
            patient_name: value(patient_name),
            patient_id: value(patient_id),
            patient_birth_date: value(patient_birth_date),
            patient_sex: value(patient_sex),
        },
        study: StudyModuleInput {
            study_instance_uid: uid(
                recipe,
                standards_lock_sha256,
                seed,
                file_index,
                UidRole::StudyInstance,
            ),
            study_date: value(study_date),
            study_time: value(study_time),
            accession_number: ElementPresence::Empty,
            referring_physician_name: ElementPresence::Empty,
            study_id: value(study_id),
        },
        series: SeriesModuleInput {
            modality: modality.into(),
            series_instance_uid: uid(
                recipe,
                standards_lock_sha256,
                seed,
                file_index,
                UidRole::SeriesInstance,
            ),
            series_number: value("1"),
            series_date: ElementPresence::Omitted,
            series_time: ElementPresence::Omitted,
        },
        frame_of_reference: None,
        equipment: EquipmentModuleInput {
            manufacturer: value(manufacturer),
            manufacturer_model_name: value(&recipe.recipe_id),
            software_versions: value(software_versions),
        },
        image: ImageModuleInput {
            instance_number: value("1"),
            patient_orientation: ElementPresence::Empty,
            content_date: value("20260101"),
            content_time: value("000000"),
        },
    }
}

fn native_request(parameters: &VlArtifactParameters) -> NativePixelRequest {
    NativePixelRequest {
        shape: PixelShape {
            rows: parameters.rows,
            columns: parameters.columns,
            frames: 1,
            samples_per_pixel: parameters.samples_per_pixel,
            photometric_interpretation: parameters.photometric_interpretation.native(),
            bits_allocated: 8,
            bits_stored: 8,
            high_bit: 7,
            pixel_representation: 0,
            stored_value_type: StoredValueType::U8,
            byte_order: ByteOrder::Little,
            pixel_data_vr: PixelDataVr::Ob,
            color: parameters
                .planar_configuration
                .map(|planar_configuration| ColorOrganization {
                    planar_configuration,
                    chroma_subsampling: ChromaSubsampling::None,
                }),
        },
        stored_values: parameters.stored_values.clone(),
        declared_pixel_min: parameters.pixel_min,
        declared_pixel_max: parameters.pixel_max,
        expected_frame_sha256: vec![parameters.frame_sha256.clone()],
        padding: None,
        palette: parameters.palette.clone(),
    }
}

fn validate_recipe_contract(
    recipe: &CaseRecipe,
    expected_order: u32,
) -> Result<(), ClassicVlProjectionPlanError> {
    if recipe.plan_provider_id != PLAN_PROVIDER_ID {
        return Err(contract("owned recipe requires native.classic_plan"));
    }
    if recipe.planning_order != Some(expected_order) {
        return Err(contract("owned recipe has incorrect planning_order"));
    }
    if !recipe.dependencies.is_empty() {
        return Err(contract("owned recipe must not declare dependencies"));
    }
    let dicom = recipe
        .dicom
        .as_ref()
        .ok_or_else(|| contract("owned recipe requires DICOM artifacts"))?;
    if dicom.artifacts.len() != 1 {
        return Err(contract("owned recipe requires exactly one artifact"));
    }
    let artifact = &dicom.artifacts[0];
    if artifact.logical_id != "instance" || artifact.order != 0 {
        return Err(contract("owned artifact identity/order changed"));
    }
    let expected_path = format!("{}/instance.dcm", recipe.binding.case_id);
    if artifact.output.provider_derived == Some(true)
        || artifact.output.path.as_deref() != Some(expected_path.as_str())
    {
        return Err(contract(
            "owned artifact requires its explicit historical output path",
        ));
    }
    if artifact.content.provider_id != CONTENT_PROVIDER_ID
        || !artifact.content.parameters.is_empty()
    {
        return Err(contract(
            "owned artifact requires parameter-free content.native_pixels",
        ));
    }
    if artifact.algorithm_provider_id.as_deref() != Some(ALGORITHM_ID) {
        return Err(contract(
            "owned artifact requires algorithm.classic_vl_projection",
        ));
    }
    if !artifact.attribute_operations.is_empty()
        || artifact.secondary_capture.is_some()
        || artifact.metadata_sc.is_some()
        || artifact.nonsquare_geometry.is_some()
    {
        return Err(contract(
            "owned static values must use typed provider parameters",
        ));
    }
    if !matches!(
        artifact.encoding.transfer_syntax_uid.as_str(),
        EXPLICIT_VR_LE | RLE_LOSSLESS
    ) {
        return Err(contract(
            "owned artifact transfer syntax must be Explicit LE or RLE Lossless",
        ));
    }
    Ok(())
}

fn validate_vl_parameters(
    parameters: &VlArtifactParameters,
) -> Result<(), ClassicVlProjectionPlanError> {
    let expected_sop = match parameters.modality.as_str() {
        "XC" => PHOTOGRAPHIC_STORAGE,
        "ES" => ENDOSCOPIC_STORAGE,
        "GM" => MICROSCOPIC_STORAGE,
        _ => return Err(contract("unsupported visible-light modality")),
    };
    if parameters.sop_class_uid != expected_sop || parameters.frame_sha256.len() != 64 {
        return Err(contract("visible-light SOP class or frame hash is invalid"));
    }
    if parameters.photometric_interpretation == VlPhotometricInterpretation::PaletteColor {
        if parameters.samples_per_pixel != 1
            || parameters.planar_configuration.is_some()
            || parameters.palette.is_none()
        {
            return Err(contract("palette VL declaration is inconsistent"));
        }
    } else if parameters.photometric_interpretation == VlPhotometricInterpretation::Rgb {
        if parameters.samples_per_pixel != 3
            || !matches!(parameters.planar_configuration, Some(0 | 1))
            || parameters.palette.is_some()
        {
            return Err(contract("RGB VL declaration is inconsistent"));
        }
    } else {
        return Err(contract("ordinary VL recipes require RGB or PALETTE COLOR"));
    }
    Ok(())
}

fn validate_projection_parameters(
    recipe: &CaseRecipe,
    parameters: &ProjectionArtifactParameters,
) -> Result<(), ClassicVlProjectionPlanError> {
    let is_xa = recipe.binding.case_id.starts_with("classic/xa/");
    let expected_sop = if is_xa { XA_STORAGE } else { XRF_STORAGE };
    let expected_modality = if is_xa { "XA" } else { "RF" };
    if parameters.sop_class_uid != expected_sop
        || parameters.modality != expected_modality
        || parameters.image_type != ["ORIGINAL", "PRIMARY", "SINGLE PLANE"]
        || parameters.frame_sha256.len() != 64
        || parameters.non_claims.multiframe_cine
        || parameters.non_claims.biplane_data_present
        || parameters.non_claims.contrast_used
        || parameters.non_claims.subtraction_applied
        || parameters.non_claims.table_motion_present
        || parameters.non_claims.patient_space_geometry_present
        || parameters.non_claims.pixel_spacing_calibrated
    {
        return Err(contract(
            "projection parameters contradict the classic single-plane contract",
        ));
    }
    if is_xa {
        if parameters.positioner_primary_angle.is_none()
            || parameters.positioner_secondary_angle.is_none()
            || parameters.column_angulation.is_some()
            || parameters.non_claims.xa_positioner_angles_present
        {
            return Err(contract("XA positioner contract is inconsistent"));
        }
    } else if parameters.column_angulation.is_none()
        || parameters.positioner_primary_angle.is_some()
        || parameters.positioner_secondary_angle.is_some()
        || parameters.non_claims.laterality_present
        || parameters.non_claims.table_position_present
        || parameters.non_claims.table_tilt_present
        || parameters.non_claims.tomography_present
        || parameters.non_claims.xa_positioner_angles_present
    {
        return Err(contract("XRF projection non-claims are inconsistent"));
    }
    Ok(())
}

fn palette_operations(palette: &Palette) -> Vec<AttributeOperation> {
    let descriptor = palette.descriptor.map(u64::from);
    [
        ("0028,1101", descriptor),
        ("0028,1102", descriptor),
        ("0028,1103", descriptor),
    ]
    .into_iter()
    .map(|(tag, values)| set_unsigned_multi(tag, values))
    .chain([
        set_binary("0028,1201", DicomVr::OW, words_le(&palette.red)),
        set_binary("0028,1202", DicomVr::OW, words_le(&palette.green)),
        set_binary("0028,1203", DicomVr::OW, words_le(&palette.blue)),
    ])
    .collect()
}
fn words_le(words: &[u16]) -> Vec<u8> {
    words.iter().flat_map(|word| word.to_le_bytes()).collect()
}
fn decode_hex(text: &str) -> Result<Vec<u8>, ClassicVlProjectionPlanError> {
    let compact = text
        .chars()
        .filter(|character| !character.is_ascii_whitespace())
        .collect::<String>();
    if compact.len() % 2 != 0 {
        return Err(contract("ICC profile hex must contain complete bytes"));
    }
    (0..compact.len())
        .step_by(2)
        .map(|index| {
            u8::from_str_radix(&compact[index..index + 2], 16)
                .map_err(|_| contract("ICC profile hex is invalid"))
        })
        .collect()
}
fn output_path(
    artifact: &super::PlannedArtifactRecipe,
) -> Result<OutputRelativePath, ClassicVlProjectionPlanError> {
    Ok(OutputRelativePath::new(
        artifact
            .output
            .path
            .clone()
            .ok_or_else(|| contract("artifact output path must be explicit"))?,
    )?)
}
fn implementation_uid(standards_lock_sha256: &str) -> String {
    deterministic_uid(&DeterministicUidInput {
        standards_lock_sha256,
        case_id: "dicom-test-suite/implementation",
        recipe_version: crate::PACKAGE_VERSION,
        run_seed: 0,
        file_index: 0,
        frame_index: None,
        referenced_object_index: None,
        role: UidRole::ImplementationClass,
    })
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
    AttributeAddress::from_normalized_tag(tag).expect("VL/projection provider tag is valid")
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
fn set_unsigned_multi(tag: &str, values: impl IntoIterator<Item = u64>) -> AttributeOperation {
    AttributeOperation::Set {
        address: address(tag),
        vr: DicomVr::US,
        value: AttributeValue::Multi(values.into_iter().map(PrimitiveValue::Unsigned).collect()),
    }
}
fn set_binary(tag: &str, vr: DicomVr, bytes: Vec<u8>) -> AttributeOperation {
    AttributeOperation::Set {
        address: address(tag),
        vr,
        value: AttributeValue::Binary(bytes),
    }
}
fn decode<T: for<'de> Deserialize<'de>>(value: Value) -> Result<T, ClassicVlProjectionPlanError> {
    serde_json::from_value(value).map_err(ClassicVlProjectionPlanError::Parameters)
}
fn contract(message: &'static str) -> ClassicVlProjectionPlanError {
    ClassicVlProjectionPlanError::Contract(message)
}

#[derive(Debug)]
pub enum ClassicVlProjectionPlanError {
    Contract(&'static str),
    Parameters(serde_json::Error),
    OutputPath(crate::corpus_plan::CorpusPlanError),
    Classic(ClassicPlanError),
}
impl From<crate::corpus_plan::CorpusPlanError> for ClassicVlProjectionPlanError {
    fn from(error: crate::corpus_plan::CorpusPlanError) -> Self {
        Self::OutputPath(error)
    }
}
impl From<ClassicPlanError> for ClassicVlProjectionPlanError {
    fn from(error: ClassicPlanError) -> Self {
        Self::Classic(error)
    }
}
impl fmt::Display for ClassicVlProjectionPlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}
impl std::error::Error for ClassicVlProjectionPlanError {}
