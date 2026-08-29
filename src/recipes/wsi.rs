//! Direct, filesystem-free Whole Slide Microscopy plan providers.

use std::collections::BTreeMap;

use dicom_dictionary_std::tags;

use super::{
    AdvancedArtifactProvenance, AdvancedArtifactRole, AdvancedPlanProvider,
    AdvancedPlanProviderOutput, AdvancedPlanProviderRequest, AdvancedPlannedArtifact,
    AdvancedProviderContractError, AdvancedProviderFamily, RecipeIdentity, WholeSlideArtifactKind,
};
use crate::composition::{
    AttributeAddress, AttributeItem, AttributeOperation, AttributeValue, CanonicalContent,
    CompositionUidRole, DicomVr, IdentityPlan, PrimitiveValue, ResolvedAttribute,
    ResolvedInstancePlan, TemplateId, TemplateVersion, ValueOrigin,
};
use crate::corpus_plan::{
    ArtifactProvenance, ArtifactResourceEstimate, CaseBinding, EncodingPlan, EvidencePlan,
    FileMetaPolicy, FragmentationPolicy, ImplementationIdentityPlan, ItemLengthPolicy,
    OffsetTablePolicy, OutputPlan, OutputRelativePath, PlannedDicomArtifact, PreamblePolicy,
    SequenceLengthPolicy, ValidationPlan, ValidationRequirement, ValidationRule,
};
use crate::executor::services::{
    ArtifactExecutionBindings, ByteBinding, NativeFrameBinding, SlotExecutionBinding,
};
use crate::{DeterministicUidInput, UidRole, deterministic_uid, sha256_hex};

pub const WSI_ADVANCED_PROVIDER_ID: &str = "native.wsi_plan";
const SOP_CLASS_UID: &str = "1.2.840.10008.5.1.4.1.1.77.1.6";
const TRANSFER_SYNTAX_UID: &str = "1.2.840.10008.1.2.1";
const PIXEL_SLOT: &str = "pixels";
const ICC_COLOR_SPACE: &str = "SRGB";
const ICC_PROFILE_SIZE: usize = 736;
const PROFILE_HEX: &[u8] = include_bytes!("../generator/native/dcmtk_srgb_input_profile.hex");

const FULL_CASE: &str = "vl/wsi/tiled_full_small";
const SPARSE_CASE: &str = "vl/wsi/tiled_sparse_small";
const MULTIPATH_CASE: &str = "vl/wsi/multiple_optical_paths";
const PYRAMID_CASE: &str = "vl/wsi/pyramid_multiresolution";
const STRESS_CASE: &str = "stress/wsi/large_pyramid";

#[derive(Debug, Clone)]
pub struct WsiAdvancedPlanProvider {
    standards_lock_sha256: String,
}

impl WsiAdvancedPlanProvider {
    pub fn new(standards_lock_sha256: impl Into<String>) -> Self {
        Self {
            standards_lock_sha256: standards_lock_sha256.into(),
        }
    }

    pub fn owns(case_id: &str) -> bool {
        matches!(
            case_id,
            FULL_CASE | SPARSE_CASE | MULTIPATH_CASE | PYRAMID_CASE | STRESS_CASE
        )
    }
}

impl AdvancedPlanProvider for WsiAdvancedPlanProvider {
    type ProviderInput = WsiPlanRecipe;

    fn provider_id(&self) -> &str {
        WSI_ADVANCED_PROVIDER_ID
    }

    fn plan(
        &self,
        request: &AdvancedPlanProviderRequest,
        recipe: &WsiPlanRecipe,
    ) -> Result<AdvancedPlanProviderOutput, AdvancedProviderContractError> {
        request.validate()?;
        if request.provider_id != self.provider_id() {
            return Err(invalid("provider_id", &request.provider_id));
        }
        if request.family != AdvancedProviderFamily::WholeSlide {
            return Err(AdvancedProviderContractError::FamilyRoleMismatch);
        }
        if recipe.recipe != request.recipe {
            return Err(invalid("recipe_id", &request.recipe.recipe_id));
        }
        if recipe.case_id != request.case_id || !Self::owns(&recipe.case_id) {
            return Err(invalid("case_id", &request.case_id));
        }

        let identities = WsiIdentities::new(
            &self.standards_lock_sha256,
            &request.case_id,
            &request.recipe.recipe_version,
            request.seed,
        );
        let specs = recipe.resolve_artifacts()?;

        let implementation_uid = implementation_uid(&self.standards_lock_sha256);
        let mut artifacts = Vec::with_capacity(specs.len());
        let mut bindings = Vec::with_capacity(specs.len());
        for spec in specs {
            let attrs = base_attributes(&identities, spec.file_index, &spec.overrides);
            let (planned, binding) =
                planned_artifact(request, spec, attrs, &identities, &implementation_uid);
            artifacts.push(planned);
            bindings.push(binding);
        }
        let output = AdvancedPlanProviderOutput {
            artifacts,
            dependencies: Vec::new(),
            references: Vec::new(),
            bindings,
        };
        output.validate(request)?;
        Ok(output)
    }
}

#[derive(Debug, Clone)]
struct WsiIdentities {
    study: String,
    series: String,
    frame_of_reference: String,
    specimen: String,
    pyramid: String,
    sop: [String; 3],
    dimension: [String; 3],
}

impl WsiIdentities {
    fn new(lock: &str, case_id: &str, version: &str, seed: u64) -> Self {
        let allocate = |role, file_index, referenced_object_index| {
            deterministic_uid(&DeterministicUidInput {
                standards_lock_sha256: lock,
                case_id,
                recipe_version: version,
                run_seed: seed,
                file_index,
                frame_index: None,
                referenced_object_index,
                role,
            })
        };
        Self {
            study: allocate(UidRole::StudyInstance, 0, None),
            series: allocate(UidRole::SeriesInstance, 0, None),
            frame_of_reference: allocate(UidRole::FrameOfReference, 0, None),
            specimen: allocate(UidRole::DerivedReference, 0, Some(0)),
            pyramid: allocate(UidRole::DerivedReference, 0, Some(1)),
            sop: [
                allocate(UidRole::SopInstance, 0, None),
                allocate(UidRole::SopInstance, 1, None),
                allocate(UidRole::SopInstance, 2, None),
            ],
            dimension: [
                allocate(UidRole::DimensionOrganization, 0, None),
                allocate(UidRole::DimensionOrganization, 1, None),
                allocate(UidRole::DimensionOrganization, 2, None),
            ],
        }
    }
}

#[derive(Debug, Clone)]
pub struct WsiPlanRecipe {
    pub case_id: String,
    pub recipe: RecipeIdentity,
    pub artifacts: Vec<WsiArtifactRecipe>,
}

impl WsiPlanRecipe {
    fn resolve_artifacts(&self) -> Result<Vec<WsiArtifactSpec>, AdvancedProviderContractError> {
        self.artifacts
            .iter()
            .map(WsiArtifactRecipe::resolve)
            .collect()
    }
}

#[derive(Debug, Clone)]
pub struct WsiArtifactRecipe {
    pub logical_id: String,
    pub order: u64,
    pub template_id: &'static str,
    pub relative_path: String,
    pub kind: WholeSlideArtifactKind,
    pub level: u32,
    pub file_index: usize,
    pub parameters: WsiArtifactParameters,
    pub pixel_algorithm: WsiPixelAlgorithm,
}

impl WsiArtifactRecipe {
    fn ordinary(
        logical_id: &str,
        order: u64,
        template_id: &'static str,
        relative_path: &str,
        kind: WholeSlideArtifactKind,
        file_index: usize,
        parameters: WsiArtifactParameters,
        pixel_algorithm: WsiPixelAlgorithm,
    ) -> Self {
        Self {
            logical_id: logical_id.into(),
            order,
            template_id,
            relative_path: relative_path.into(),
            kind,
            level: u32::try_from(file_index).expect("bounded WSI file index"),
            file_index,
            parameters,
            pixel_algorithm,
        }
    }

    fn resolve(&self) -> Result<WsiArtifactSpec, AdvancedProviderContractError> {
        Ok(WsiArtifactSpec {
            logical_id: self.logical_id.clone(),
            order: self.order,
            template_id: self.template_id,
            relative_path: self.relative_path.clone(),
            kind: self.kind,
            level: self.level,
            file_index: self.file_index,
            overrides: self.parameters.clone(),
            pixels: match self.pixel_algorithm {
                WsiPixelAlgorithm::TiledColorQuadrants => full_pixels(),
                WsiPixelAlgorithm::SparseDiagonalTiles => sparse_pixels(),
                WsiPixelAlgorithm::MultipleOpticalPaths => multipath_pixels(),
                WsiPixelAlgorithm::Thumbnail => {
                    vec![255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255]
                }
                WsiPixelAlgorithm::Label => {
                    vec![0, 32, 96, 255, 255, 255, 0, 32, 96, 255, 255, 255]
                }
                WsiPixelAlgorithm::ReducedStress { level_index, edge } => {
                    stress_pixels(level_index, edge)?
                }
            },
        })
    }
}

#[derive(Debug, Clone)]
pub struct WsiArtifactParameters {
    pub series_number: &'static str,
    pub model_name: &'static str,
    pub instance_number: String,
    pub image_type: &'static str,
    pub label_in_image: &'static str,
    pub rows: u16,
    pub columns: u16,
    pub frames: u16,
    pub matrix_rows: u32,
    pub matrix_columns: u32,
    pub width: f32,
    pub height: f32,
    pub spacing: &'static str,
    pub dimension_type: &'static str,
    pub pyramid_membership: bool,
    pub optical_paths: Vec<WsiOpticalPath>,
    pub sparse_dimension_indices: bool,
    pub sparse_positions: bool,
    pub specimen_identifier: &'static str,
    pub container_identifier: &'static str,
}

impl WsiArtifactParameters {
    fn full() -> Self {
        Self::base()
    }

    fn sparse() -> Self {
        Self {
            series_number: "42",
            model_name: "Native TILED_SPARSE WSI",
            frames: 2,
            dimension_type: "TILED_SPARSE",
            sparse_dimension_indices: true,
            sparse_positions: true,
            ..Self::base()
        }
    }

    fn multipath() -> Self {
        Self {
            series_number: "44",
            model_name: "Native Multi-Path WSI",
            frames: 8,
            optical_paths: vec![
                WsiOpticalPath::new("BRIGHTFIELD", Some("Deterministic brightfield path"), 550.0),
                WsiOpticalPath::new("ALTERNATE", Some("Deterministic alternate path"), 650.0),
            ],
            ..Self::base()
        }
    }

    fn base() -> Self {
        Self {
            series_number: "41",
            model_name: "Native TILED_FULL WSI",
            instance_number: "1".into(),
            image_type: r"ORIGINAL\PRIMARY\VOLUME\NONE",
            label_in_image: "NO",
            rows: 2,
            columns: 2,
            frames: 4,
            matrix_rows: 4,
            matrix_columns: 4,
            width: 2.0,
            height: 2.0,
            spacing: r"0.5\0.5",
            dimension_type: "TILED_FULL",
            pyramid_membership: false,
            optical_paths: vec![WsiOpticalPath::new("RGB", None, 550.0)],
            sparse_dimension_indices: false,
            sparse_positions: false,
            specimen_identifier: "DTS-SPECIMEN-001",
            container_identifier: "DTS-SLIDE-001",
        }
    }
}

#[derive(Debug, Clone)]
pub struct WsiOpticalPath {
    pub identifier: &'static str,
    pub description: Option<&'static str>,
    pub wavelength: f32,
}

impl WsiOpticalPath {
    fn new(identifier: &'static str, description: Option<&'static str>, wavelength: f32) -> Self {
        Self {
            identifier,
            description,
            wavelength,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WsiPixelAlgorithm {
    TiledColorQuadrants,
    SparseDiagonalTiles,
    MultipleOpticalPaths,
    Thumbnail,
    Label,
    ReducedStress { level_index: usize, edge: u32 },
}

#[derive(Debug, Clone)]
struct WsiArtifactSpec {
    logical_id: String,
    order: u64,
    template_id: &'static str,
    relative_path: String,
    kind: WholeSlideArtifactKind,
    level: u32,
    file_index: usize,
    overrides: WsiArtifactParameters,
    pixels: Vec<u8>,
}

pub fn curated_wsi_recipes() -> Vec<WsiPlanRecipe> {
    vec![
        single_recipe(
            FULL_CASE,
            "vl_wsi_tiled_full_small",
            WsiArtifactRecipe::ordinary(
                "wsi_tiled_full",
                0,
                "vl/wsi/tiled-full",
                "vl/wsi/tiled_full_small/instance.dcm",
                WholeSlideArtifactKind::Volume,
                0,
                WsiArtifactParameters::full(),
                WsiPixelAlgorithm::TiledColorQuadrants,
            ),
        ),
        single_recipe(
            SPARSE_CASE,
            "vl_wsi_tiled_sparse_small",
            WsiArtifactRecipe::ordinary(
                "wsi_tiled_sparse",
                0,
                "vl/wsi/tiled-sparse",
                "vl/wsi/tiled_sparse_small/instance.dcm",
                WholeSlideArtifactKind::Volume,
                0,
                WsiArtifactParameters::sparse(),
                WsiPixelAlgorithm::SparseDiagonalTiles,
            ),
        ),
        single_recipe(
            MULTIPATH_CASE,
            "vl_wsi_multiple_optical_paths",
            WsiArtifactRecipe::ordinary(
                "wsi_multiple_optical_paths",
                0,
                "vl/wsi/multiple-optical-paths",
                "vl/wsi/multiple_optical_paths/instance.dcm",
                WholeSlideArtifactKind::Volume,
                0,
                WsiArtifactParameters::multipath(),
                WsiPixelAlgorithm::MultipleOpticalPaths,
            ),
        ),
        WsiPlanRecipe {
            case_id: PYRAMID_CASE.into(),
            recipe: RecipeIdentity {
                recipe_id: "vl_wsi_pyramid_multiresolution".into(),
                recipe_version: "0.1.0".into(),
            },
            artifacts: ordinary_pyramid_artifacts(),
        },
        WsiPlanRecipe {
            case_id: STRESS_CASE.into(),
            recipe: RecipeIdentity {
                recipe_id: "stress_wsi_large_pyramid".into(),
                recipe_version: "0.1.0".into(),
            },
            artifacts: stress_pyramid_artifacts(),
        },
    ]
}

fn single_recipe(case_id: &str, recipe_id: &str, artifact: WsiArtifactRecipe) -> WsiPlanRecipe {
    WsiPlanRecipe {
        case_id: case_id.into(),
        recipe: RecipeIdentity {
            recipe_id: recipe_id.into(),
            recipe_version: "0.1.0".into(),
        },
        artifacts: vec![artifact],
    }
}

fn ordinary_pyramid_artifacts() -> Vec<WsiArtifactRecipe> {
    let mut common = WsiArtifactParameters::base();
    common.series_number = "43";
    common.model_name = "Native WSI Pyramid";
    common.pyramid_membership = true;
    vec![
        WsiArtifactRecipe::ordinary(
            "wsi_pyramid_volume",
            0,
            "vl/wsi/pyramid-volume",
            "vl/wsi/pyramid_multiresolution/volume.dcm",
            WholeSlideArtifactKind::Volume,
            0,
            common.clone(),
            WsiPixelAlgorithm::TiledColorQuadrants,
        ),
        WsiArtifactRecipe::ordinary(
            "wsi_pyramid_thumbnail",
            1,
            "vl/wsi/pyramid-thumbnail",
            "vl/wsi/pyramid_multiresolution/thumbnail.dcm",
            WholeSlideArtifactKind::Thumbnail,
            1,
            WsiArtifactParameters {
                instance_number: "2".into(),
                image_type: r"DERIVED\PRIMARY\THUMBNAIL\RESAMPLED",
                frames: 1,
                matrix_rows: 2,
                matrix_columns: 2,
                spacing: r"1.0\1.0",
                ..common.clone()
            },
            WsiPixelAlgorithm::Thumbnail,
        ),
        WsiArtifactRecipe::ordinary(
            "wsi_pyramid_label",
            2,
            "vl/wsi/pyramid-label",
            "vl/wsi/pyramid_multiresolution/label.dcm",
            WholeSlideArtifactKind::Label,
            2,
            WsiArtifactParameters {
                instance_number: "3".into(),
                image_type: r"ORIGINAL\PRIMARY\LABEL\NONE",
                label_in_image: "YES",
                frames: 1,
                matrix_rows: 2,
                matrix_columns: 2,
                width: 1.0,
                height: 1.0,
                pyramid_membership: false,
                ..common
            },
            WsiPixelAlgorithm::Label,
        ),
    ]
}

fn stress_pyramid_artifacts() -> Vec<WsiArtifactRecipe> {
    let edges = [1024_u32, 512, 256];
    let frames = [16_u16, 4, 1];
    let spacings = [r"0.0005\0.0005", r"0.001\0.001", r"0.002\0.002"];
    let mut specs = Vec::with_capacity(3);
    for index in 0..3 {
        specs.push(WsiArtifactRecipe::ordinary(
            &format!("stress_wsi_level_{:03}", index + 1),
            index as u64,
            "vl/wsi/pyramid-volume",
            &format!("stress/wsi/large_pyramid/level-{:03}.dcm", index + 1),
            WholeSlideArtifactKind::Volume,
            index,
            WsiArtifactParameters {
                series_number: "143",
                model_name: "Native Reduced Stress WSI Pyramid",
                instance_number: (index + 1).to_string(),
                rows: 256,
                columns: 256,
                frames: frames[index],
                matrix_rows: edges[index],
                matrix_columns: edges[index],
                width: 0.512,
                height: 0.512,
                spacing: spacings[index],
                pyramid_membership: true,
                specimen_identifier: "DTS-STRESS-SPECIMEN-001",
                container_identifier: "DTS-STRESS-SLIDE-001",
                ..WsiArtifactParameters::base()
            },
            WsiPixelAlgorithm::ReducedStress {
                level_index: index,
                edge: edges[index],
            },
        ));
    }
    specs
}

fn planned_artifact(
    request: &AdvancedPlanProviderRequest,
    spec: WsiArtifactSpec,
    attributes: Vec<ResolvedAttribute>,
    ids: &WsiIdentities,
    implementation_uid: &str,
) -> (AdvancedPlannedArtifact, ArtifactExecutionBindings) {
    let frame_size = usize::from(spec.overrides.rows) * usize::from(spec.overrides.columns) * 3;
    let frames = spec
        .pixels
        .chunks_exact(frame_size)
        .enumerate()
        .map(|(index, bytes)| NativeFrameBinding {
            frame_number: u32::try_from(index + 1).expect("bounded WSI frames"),
            bytes: ByteBinding::Inline {
                bytes: bytes.to_vec(),
                sha256: sha256_hex(bytes),
            },
            rows: u32::from(spec.overrides.rows),
            columns: u32::from(spec.overrides.columns),
            samples_per_pixel: 3,
            bits_allocated: 8,
            photometric_interpretation: "RGB".into(),
        })
        .collect::<Vec<_>>();
    debug_assert_eq!(frames.len(), usize::from(spec.overrides.frames));

    let identities = IdentityPlan::from_exact_values(
        spec.logical_id.clone(),
        [
            (CompositionUidRole::StudyInstance, 0, ids.study.clone()),
            (CompositionUidRole::SeriesInstance, 0, ids.series.clone()),
            (
                CompositionUidRole::SopInstance,
                0,
                ids.sop[spec.file_index].clone(),
            ),
            (
                CompositionUidRole::FrameOfReference,
                0,
                ids.frame_of_reference.clone(),
            ),
            (
                CompositionUidRole::DimensionOrganization,
                0,
                ids.dimension[spec.file_index].clone(),
            ),
            (
                CompositionUidRole::ImplementationClass,
                0,
                implementation_uid.to_owned(),
            ),
            (
                CompositionUidRole::TemplateDefined("specimen_uid".into()),
                0,
                ids.specimen.clone(),
            ),
        ],
    )
    .expect("deterministic WSI identities are valid");
    let content = CanonicalContent {
        slot: PIXEL_SLOT.into(),
        kind: "native_pixel_data".into(),
        address: AttributeAddress::standard(tags::PIXEL_DATA).expect("standard Pixel Data"),
        vr: DicomVr::OB,
        size_bytes: spec.pixels.len() as u64,
        sha256: sha256_hex(&spec.pixels),
        properties: BTreeMap::from([
            ("frames".into(), spec.overrides.frames.to_string()),
            ("photometric_interpretation".into(), "RGB".into()),
        ]),
        placement: Default::default(),
        materialization: None,
    };
    let instance = ResolvedInstancePlan {
        plan_schema_version: "0.1.0".into(),
        instance_id: spec.logical_id.clone(),
        template_id: TemplateId(spec.template_id.into()),
        template_version: "1.0.0".parse::<TemplateVersion>().expect("valid version"),
        sop_class_uid: SOP_CLASS_UID.into(),
        transfer_syntax_uid: TRANSFER_SYNTAX_UID.into(),
        identities,
        attributes,
        content: vec![content],
        references: Vec::new(),
    };
    let output_bytes = (spec.pixels.len() as u64).saturating_add(16 * 1024);
    let artifact = AdvancedPlannedArtifact {
        role: AdvancedArtifactRole::WholeSlidePyramid {
            level: spec.level,
            artifact_kind: spec.kind,
        },
        provenance: AdvancedArtifactProvenance::Requested,
        planned: PlannedDicomArtifact {
            logical_id: spec.logical_id.clone(),
            order: spec.order,
            provenance: ArtifactProvenance::Requested,
            case_binding: Some(CaseBinding {
                case_id: request.case_id.clone(),
                recipe_id: request.recipe.recipe_id.clone(),
                recipe_version: request.recipe.recipe_version.clone(),
            }),
            instance,
            output: OutputPlan {
                relative_path: OutputRelativePath::new(spec.relative_path)
                    .expect("constant WSI output path is safe"),
                role: match spec.kind {
                    WholeSlideArtifactKind::Volume => "volume",
                    WholeSlideArtifactKind::Thumbnail => "thumbnail",
                    WholeSlideArtifactKind::Label => "label",
                }
                .into(),
                publish: true,
            },
            encoding: EncodingPlan {
                transfer_syntax_uid: TRANSFER_SYNTAX_UID.into(),
                sequence_length: SequenceLengthPolicy::WriterDefault,
                item_length: ItemLengthPolicy::WriterDefault,
                fragmentation: FragmentationPolicy::Native,
                offset_table: OffsetTablePolicy::NotApplicable,
                preamble: PreamblePolicy::ZeroFilled,
                file_meta: FileMetaPolicy::Standard,
                implementation: ImplementationIdentityPlan {
                    class_uid: implementation_uid.into(),
                    version_name: Some(crate::IMPLEMENTATION_VERSION_NAME.into()),
                },
                backend_id: "dicom-rs.part10".into(),
            },
            validation: ValidationPlan {
                rules: vec![ValidationRule {
                    rule_id: "validation.shared".into(),
                    requirement: ValidationRequirement::Required,
                    parameters: BTreeMap::new(),
                }],
            },
            evidence: EvidencePlan {
                obligations: Vec::new(),
            },
            resources: ArtifactResourceEstimate {
                output_bytes,
                peak_working_bytes: output_bytes.saturating_mul(2),
            },
        },
    };
    let binding = ArtifactExecutionBindings {
        artifact_id: spec.logical_id,
        slots: BTreeMap::from([(
            PIXEL_SLOT.into(),
            SlotExecutionBinding::NativeFrames { frames },
        )]),
    };
    (artifact, binding)
}

fn base_attributes(
    ids: &WsiIdentities,
    file_index: usize,
    value: &WsiArtifactParameters,
) -> Vec<ResolvedAttribute> {
    let mut elements = Vec::new();
    macro_rules! s {
        ($tag:expr, $vr:ident, $value:expr) => {
            elements.push(set_string($tag, DicomVr::$vr, $value))
        };
    }
    macro_rules! u16v {
        ($tag:expr, $value:expr) => {
            elements.push(set_unsigned($tag, DicomVr::US, u64::from($value)))
        };
    }
    macro_rules! u32v {
        ($tag:expr, $value:expr) => {
            elements.push(set_unsigned($tag, DicomVr::UL, u64::from($value)))
        };
    }
    macro_rules! f32v {
        ($tag:expr, $value:expr) => {
            elements.push(set_f32($tag, $value))
        };
    }

    s!(tags::SOP_CLASS_UID, UI, SOP_CLASS_UID);
    s!(tags::SOP_INSTANCE_UID, UI, &ids.sop[file_index]);
    s!(tags::SYNTHETIC_DATA, CS, "YES");
    s!(tags::PATIENT_NAME, PN, "DTS^Synthetic^Patient001");
    s!(tags::PATIENT_ID, LO, "DTS-PATIENT-001");
    s!(tags::PATIENT_BIRTH_DATE, DA, "19700101");
    s!(tags::PATIENT_SEX, CS, "O");
    s!(tags::STUDY_INSTANCE_UID, UI, &ids.study);
    s!(tags::STUDY_DATE, DA, "20260101");
    s!(tags::STUDY_TIME, TM, "000000");
    elements.push(empty(tags::REFERRING_PHYSICIAN_NAME, DicomVr::PN));
    s!(tags::STUDY_ID, SH, "DTS-WSI");
    elements.push(empty(tags::ACCESSION_NUMBER, DicomVr::SH));
    s!(tags::MODALITY, CS, "SM");
    s!(tags::SERIES_INSTANCE_UID, UI, &ids.series);
    s!(tags::SERIES_NUMBER, IS, value.series_number);
    s!(tags::FRAME_OF_REFERENCE_UID, UI, &ids.frame_of_reference);
    s!(tags::POSITION_REFERENCE_INDICATOR, LO, "SLIDE_CORNER");
    s!(tags::MANUFACTURER, LO, "dicom-test-suite");
    elements.push(empty(tags::INSTITUTION_NAME, DicomVr::LO));
    elements.push(empty(tags::INSTITUTION_ADDRESS, DicomVr::ST));
    s!(tags::MANUFACTURER_MODEL_NAME, LO, value.model_name);
    s!(tags::DEVICE_SERIAL_NUMBER, LO, "DTS-WSI-001");
    s!(tags::SOFTWARE_VERSIONS, LO, crate::PACKAGE_VERSION);
    s!(tags::INSTANCE_NUMBER, IS, &value.instance_number);
    s!(tags::CONTENT_DATE, DA, "20260101");
    s!(tags::CONTENT_TIME, TM, "000000");
    s!(tags::ACQUISITION_DATE_TIME, DT, "20260101000000");
    s!(tags::IMAGE_TYPE, CS, value.image_type);
    s!(tags::VOLUMETRIC_PROPERTIES, CS, "VOLUME");
    s!(tags::BURNED_IN_ANNOTATION, CS, "NO");
    s!(tags::LOSSY_IMAGE_COMPRESSION, CS, "00");
    elements.push(sequence(tags::ACQUISITION_CONTEXT_SEQUENCE, vec![]));
    u16v!(tags::SAMPLES_PER_PIXEL, 3_u16);
    s!(tags::PHOTOMETRIC_INTERPRETATION, CS, "RGB");
    u16v!(tags::PLANAR_CONFIGURATION, 0_u16);
    u16v!(tags::ROWS, value.rows);
    u16v!(tags::COLUMNS, value.columns);
    s!(tags::NUMBER_OF_FRAMES, IS, &value.frames.to_string());
    u16v!(tags::BITS_ALLOCATED, 8_u16);
    u16v!(tags::BITS_STORED, 8_u16);
    u16v!(tags::HIGH_BIT, 7_u16);
    u16v!(tags::PIXEL_REPRESENTATION, 0_u16);
    f32v!(tags::IMAGED_VOLUME_WIDTH, value.width);
    f32v!(tags::IMAGED_VOLUME_HEIGHT, value.height);
    f32v!(tags::IMAGED_VOLUME_DEPTH, 0.001_f32);
    s!(tags::SPECIMEN_LABEL_IN_IMAGE, CS, value.label_in_image);
    s!(tags::FOCUS_METHOD, CS, "AUTO");
    s!(tags::EXTENDED_DEPTH_OF_FIELD, CS, "NO");
    s!(tags::CONTAINER_IDENTIFIER, LO, value.container_identifier);
    elements.push(sequence(
        tags::ISSUER_OF_THE_CONTAINER_IDENTIFIER_SEQUENCE,
        vec![],
    ));
    elements.push(sequence(tags::CONTAINER_TYPE_CODE_SEQUENCE, vec![]));
    elements.push(sequence(
        tags::SPECIMEN_DESCRIPTION_SEQUENCE,
        vec![item(vec![
            set_string(
                tags::SPECIMEN_IDENTIFIER,
                DicomVr::LO,
                value.specimen_identifier,
            ),
            set_string(tags::SPECIMEN_UID, DicomVr::UI, &ids.specimen),
            sequence(tags::ISSUER_OF_THE_SPECIMEN_IDENTIFIER_SEQUENCE, vec![]),
            sequence(tags::SPECIMEN_PREPARATION_SEQUENCE, vec![]),
        ])],
    ));
    elements.push(sequence(
        tags::OPTICAL_PATH_SEQUENCE,
        value
            .optical_paths
            .iter()
            .map(|path| {
                let mut attributes = vec![
                    code_sequence(
                        tags::ILLUMINATION_TYPE_CODE_SEQUENCE,
                        "111744",
                        "DCM",
                        "Brightfield illumination",
                    ),
                    set_f32(tags::ILLUMINATION_WAVE_LENGTH, path.wavelength),
                    set_string(tags::OPTICAL_PATH_IDENTIFIER, DicomVr::SH, path.identifier),
                ];
                if let Some(description) = path.description {
                    attributes.push(set_string(
                        tags::OPTICAL_PATH_DESCRIPTION,
                        DicomVr::ST,
                        description,
                    ));
                }
                attributes.push(set_binary(tags::ICC_PROFILE, DicomVr::OB, icc_profile()));
                attributes.push(set_string(tags::COLOR_SPACE, DicomVr::CS, ICC_COLOR_SPACE));
                item(attributes)
            })
            .collect(),
    ));
    u32v!(
        tags::NUMBER_OF_OPTICAL_PATHS,
        u32::try_from(value.optical_paths.len()).expect("bounded optical paths")
    );
    u32v!(tags::TOTAL_PIXEL_MATRIX_COLUMNS, value.matrix_columns);
    u32v!(tags::TOTAL_PIXEL_MATRIX_ROWS, value.matrix_rows);
    elements.push(sequence(
        tags::TOTAL_PIXEL_MATRIX_ORIGIN_SEQUENCE,
        vec![item(vec![
            set_string(tags::X_OFFSET_IN_SLIDE_COORDINATE_SYSTEM, DicomVr::DS, "0"),
            set_string(tags::Y_OFFSET_IN_SLIDE_COORDINATE_SYSTEM, DicomVr::DS, "0"),
            set_string(tags::Z_OFFSET_IN_SLIDE_COORDINATE_SYSTEM, DicomVr::DS, "0"),
        ])],
    ));
    s!(tags::IMAGE_ORIENTATION_SLIDE, DS, r"1\0\0\0\1\0");
    u32v!(tags::TOTAL_PIXEL_MATRIX_FOCAL_PLANES, 1_u32);
    s!(tags::TILES_OVERLAP, CS, "NONE");
    elements.push(sequence(
        tags::DIMENSION_ORGANIZATION_SEQUENCE,
        vec![item(vec![set_string(
            tags::DIMENSION_ORGANIZATION_UID,
            DicomVr::UI,
            &ids.dimension[file_index],
        )])],
    ));
    s!(tags::DIMENSION_ORGANIZATION_TYPE, CS, value.dimension_type);
    elements.push(shared_groups(value.image_type, value.spacing));
    s!(tags::LABEL_TEXT, UT, "DTS SYNTHETIC SLIDE 001");
    s!(tags::BARCODE_VALUE, LT, value.container_identifier);
    if value.pyramid_membership {
        s!(tags::PYRAMID_UID, UI, &ids.pyramid);
    }
    if value.sparse_dimension_indices {
        elements.push(dimension_indices(&ids.dimension[file_index]));
    }
    if value.sparse_positions {
        elements.push(sparse_per_frame_groups());
    }
    resolved(elements)
}

fn shared_groups(frame_type: &str, spacing: &str) -> AttributeOperation {
    sequence(
        tags::SHARED_FUNCTIONAL_GROUPS_SEQUENCE,
        vec![item(vec![
            sequence(
                tags::PIXEL_MEASURES_SEQUENCE,
                vec![item(vec![
                    set_string(tags::PIXEL_SPACING, DicomVr::DS, spacing),
                    set_string(tags::SLICE_THICKNESS, DicomVr::DS, "0.001"),
                ])],
            ),
            sequence(
                tags::WHOLE_SLIDE_MICROSCOPY_IMAGE_FRAME_TYPE_SEQUENCE,
                vec![item(vec![set_string(
                    tags::FRAME_TYPE,
                    DicomVr::CS,
                    frame_type,
                )])],
            ),
        ])],
    )
}

fn dimension_indices(uid: &str) -> AttributeOperation {
    let dimension = |pointer, label| {
        item(vec![
            set_tag(tags::DIMENSION_INDEX_POINTER, pointer),
            set_tag(
                tags::FUNCTIONAL_GROUP_POINTER,
                tags::PLANE_POSITION_SLIDE_SEQUENCE,
            ),
            set_string(tags::DIMENSION_ORGANIZATION_UID, DicomVr::UI, uid),
            set_string(tags::DIMENSION_DESCRIPTION_LABEL, DicomVr::LO, label),
        ])
    };
    sequence(
        tags::DIMENSION_INDEX_SEQUENCE,
        vec![
            dimension(
                tags::COLUMN_POSITION_IN_TOTAL_IMAGE_PIXEL_MATRIX,
                "Column Position",
            ),
            dimension(
                tags::ROW_POSITION_IN_TOTAL_IMAGE_PIXEL_MATRIX,
                "Row Position",
            ),
        ],
    )
}

fn sparse_per_frame_groups() -> AttributeOperation {
    let frame = |values: [u32; 2], column: i64, row: i64, x, y| {
        item(vec![
            sequence(
                tags::FRAME_CONTENT_SEQUENCE,
                vec![item(vec![set_multi_unsigned(
                    tags::DIMENSION_INDEX_VALUES,
                    DicomVr::UL,
                    values.map(u64::from).to_vec(),
                )])],
            ),
            sequence(
                tags::PLANE_POSITION_SLIDE_SEQUENCE,
                vec![item(vec![
                    set_signed(
                        tags::COLUMN_POSITION_IN_TOTAL_IMAGE_PIXEL_MATRIX,
                        DicomVr::SL,
                        column,
                    ),
                    set_signed(
                        tags::ROW_POSITION_IN_TOTAL_IMAGE_PIXEL_MATRIX,
                        DicomVr::SL,
                        row,
                    ),
                    set_string(tags::X_OFFSET_IN_SLIDE_COORDINATE_SYSTEM, DicomVr::DS, x),
                    set_string(tags::Y_OFFSET_IN_SLIDE_COORDINATE_SYSTEM, DicomVr::DS, y),
                    set_string(tags::Z_OFFSET_IN_SLIDE_COORDINATE_SYSTEM, DicomVr::DS, "0"),
                ])],
            ),
            sequence(
                tags::OPTICAL_PATH_IDENTIFICATION_SEQUENCE,
                vec![item(vec![set_string(
                    tags::OPTICAL_PATH_IDENTIFIER,
                    DicomVr::SH,
                    "RGB",
                )])],
            ),
        ])
    };
    sequence(
        tags::PER_FRAME_FUNCTIONAL_GROUPS_SEQUENCE,
        vec![frame([1, 1], 1, 1, "0", "0"), frame([2, 2], 3, 3, "1", "1")],
    )
}

fn code_sequence(
    tag: dicom_core::Tag,
    value: &str,
    scheme: &str,
    meaning: &str,
) -> AttributeOperation {
    sequence(
        tag,
        vec![item(vec![
            set_string(tags::CODE_VALUE, DicomVr::SH, value),
            set_string(tags::CODING_SCHEME_DESIGNATOR, DicomVr::SH, scheme),
            set_string(tags::CODE_MEANING, DicomVr::LO, meaning),
        ])],
    )
}

fn item(mut attributes: Vec<AttributeOperation>) -> AttributeItem {
    attributes.sort_by_key(|operation| operation.address().clone());
    AttributeItem { attributes }
}

fn sequence(tag: dicom_core::Tag, items: Vec<AttributeItem>) -> AttributeOperation {
    set_value(tag, DicomVr::SQ, AttributeValue::Sequence(items))
}

fn set_string(tag: dicom_core::Tag, vr: DicomVr, value: &str) -> AttributeOperation {
    let values = value
        .split('\\')
        .map(|part| PrimitiveValue::String(part.to_owned()))
        .collect::<Vec<_>>();
    let value = if values.len() == 1 {
        AttributeValue::Primitive(values.into_iter().next().expect("one value"))
    } else {
        AttributeValue::Multi(values)
    };
    set_value(tag, vr, value)
}

fn empty(tag: dicom_core::Tag, _vr: DicomVr) -> AttributeOperation {
    AttributeOperation::Empty {
        address: AttributeAddress::standard(tag).expect("standard DICOM tag"),
    }
}

fn set_unsigned(tag: dicom_core::Tag, vr: DicomVr, value: u64) -> AttributeOperation {
    set_value(
        tag,
        vr,
        AttributeValue::Primitive(PrimitiveValue::Unsigned(value)),
    )
}

fn set_multi_unsigned(tag: dicom_core::Tag, vr: DicomVr, values: Vec<u64>) -> AttributeOperation {
    set_value(
        tag,
        vr,
        AttributeValue::Multi(values.into_iter().map(PrimitiveValue::Unsigned).collect()),
    )
}

fn set_signed(tag: dicom_core::Tag, vr: DicomVr, value: i64) -> AttributeOperation {
    set_value(
        tag,
        vr,
        AttributeValue::Primitive(PrimitiveValue::Signed(value)),
    )
}

fn set_f32(tag: dicom_core::Tag, value: f32) -> AttributeOperation {
    set_value(
        tag,
        DicomVr::FL,
        AttributeValue::Primitive(PrimitiveValue::Float32Bits(value.to_bits())),
    )
}

fn set_tag(tag: dicom_core::Tag, value: dicom_core::Tag) -> AttributeOperation {
    set_value(
        tag,
        DicomVr::AT,
        AttributeValue::Primitive(PrimitiveValue::Tag(
            AttributeAddress::standard(value).expect("standard DICOM tag"),
        )),
    )
}

fn set_binary(tag: dicom_core::Tag, vr: DicomVr, bytes: Vec<u8>) -> AttributeOperation {
    set_value(tag, vr, AttributeValue::Binary(bytes))
}

fn set_value(tag: dicom_core::Tag, vr: DicomVr, value: AttributeValue) -> AttributeOperation {
    AttributeOperation::Set {
        address: AttributeAddress::standard(tag).expect("standard DICOM tag"),
        vr,
        value,
    }
}

fn resolved(mut operations: Vec<AttributeOperation>) -> Vec<ResolvedAttribute> {
    operations.sort_by_key(|operation| operation.address().clone());
    operations
        .into_iter()
        .map(|operation| match operation {
            AttributeOperation::Set { address, vr, value } => ResolvedAttribute {
                address,
                vr,
                value: Some(value),
                origin: ValueOrigin::InstanceOverride,
            },
            AttributeOperation::Empty { address } => ResolvedAttribute {
                vr: vr_for(&address),
                address,
                value: None,
                origin: ValueOrigin::InstanceOverride,
            },
            AttributeOperation::Remove { .. } => unreachable!("WSI plans contain no removals"),
        })
        .collect()
}

fn vr_for(address: &AttributeAddress) -> DicomVr {
    use dicom_core::dictionary::{DataDictionary, DataDictionaryEntry, VirtualVr};
    use std::str::FromStr;
    let tag = address.tag();
    let entry = dicom_dictionary_std::StandardDataDictionary
        .by_tag(tag)
        .expect("standard DICOM tag");
    let vr = match entry.vr() {
        VirtualVr::Exact(vr) => vr,
        other => other.exact().expect("empty WSI tag has exact VR"),
    };
    DicomVr::from_str(&vr.to_string()).expect("supported DICOM VR")
}

fn full_pixels() -> Vec<u8> {
    let colors = [[255_u8, 0, 0], [0, 255, 0], [0, 0, 255], [255, 255, 255]];
    colors
        .into_iter()
        .flat_map(|color| std::iter::repeat_n(color, 4).flatten())
        .collect()
}

fn sparse_pixels() -> Vec<u8> {
    [[255_u8, 0, 0]; 4]
        .into_iter()
        .chain([[255_u8, 255, 255]; 4])
        .flatten()
        .collect()
}

fn multipath_pixels() -> Vec<u8> {
    let mut bytes = full_pixels();
    for color in [[0_u8, 255, 255], [255, 0, 255], [255, 255, 0], [0, 0, 0]] {
        for _ in 0..4 {
            bytes.extend_from_slice(&color);
        }
    }
    bytes
}

fn stress_pixels(
    level_index: usize,
    total_matrix_edge: u32,
) -> Result<Vec<u8>, AdvancedProviderContractError> {
    let edge = usize::try_from(total_matrix_edge)
        .map_err(|_| AdvancedProviderContractError::ResourceOverflow)?;
    let tile_edge = 256_usize;
    if edge % tile_edge != 0 {
        return Err(AdvancedProviderContractError::ResourceOverflow);
    }
    let byte_count = edge
        .checked_mul(edge)
        .and_then(|pixels| pixels.checked_mul(3))
        .ok_or(AdvancedProviderContractError::ResourceOverflow)?;
    let mut bytes = Vec::with_capacity(byte_count);
    let scale = 1_usize << level_index;
    for tile_row in 0..edge / tile_edge {
        for tile_column in 0..edge / tile_edge {
            for row in 0..tile_edge {
                let y = (tile_row * tile_edge + row) * scale;
                for column in 0..tile_edge {
                    let x = (tile_column * tile_edge + column) * scale;
                    let red = ((x * 255) / 1023) as u8;
                    let green = ((y * 255) / 1023) as u8;
                    let blue = if ((x / 64) + (y / 64)) % 2 == 0 {
                        24
                    } else {
                        232
                    };
                    bytes.extend_from_slice(&[red, green, blue]);
                }
            }
        }
    }
    Ok(bytes)
}

fn icc_profile() -> Vec<u8> {
    let mut output = Vec::with_capacity(ICC_PROFILE_SIZE);
    let mut high = None;
    for byte in PROFILE_HEX.iter().copied() {
        if byte.is_ascii_whitespace() {
            continue;
        }
        let nibble = match byte {
            b'0'..=b'9' => byte - b'0',
            b'a'..=b'f' => byte - b'a' + 10,
            b'A'..=b'F' => byte - b'A' + 10,
            _ => unreachable!("locked ICC source is hexadecimal"),
        };
        if let Some(high) = high.take() {
            output.push((high << 4) | nibble);
        } else {
            high = Some(nibble);
        }
    }
    assert_eq!(output.len(), ICC_PROFILE_SIZE);
    output
}

fn implementation_uid(lock: &str) -> String {
    deterministic_uid(&DeterministicUidInput {
        standards_lock_sha256: lock,
        case_id: "dicom-test-suite/implementation",
        recipe_version: crate::PACKAGE_VERSION,
        run_seed: 0,
        file_index: 0,
        frame_index: None,
        referenced_object_index: None,
        role: UidRole::ImplementationClass,
    })
}

fn invalid(field: &'static str, value: &str) -> AdvancedProviderContractError {
    AdvancedProviderContractError::InvalidIdentifier {
        field,
        value: value.to_owned(),
    }
}
