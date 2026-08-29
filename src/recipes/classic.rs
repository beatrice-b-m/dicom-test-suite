//! Parameterized, plan-only building blocks for classic image families.
//!
//! Values are supplied by callers: this module owns no curated defaults,
//! identity allocation, filesystem paths, writers, or execution services.

use std::collections::BTreeSet;
use std::fmt;

use crate::composition::{
    AttributeAddress, AttributeOperation, AttributeValue, DicomVr, PrimitiveValue,
};
use crate::corpus_plan::OutputRelativePath;
use crate::native_pixel::{
    NativePixelContent, NativePixelError, NativePixelFactory, NativePixelLimits,
    NativePixelRequest, PhotometricInterpretation,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ElementPresence<T> {
    Omitted,
    Empty,
    Value(T),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatientModuleInput {
    pub specific_character_set: ElementPresence<String>,
    pub patient_name: ElementPresence<String>,
    pub patient_id: ElementPresence<String>,
    pub patient_birth_date: ElementPresence<String>,
    pub patient_sex: ElementPresence<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StudyModuleInput {
    pub study_instance_uid: String,
    pub study_date: ElementPresence<String>,
    pub study_time: ElementPresence<String>,
    pub accession_number: ElementPresence<String>,
    pub referring_physician_name: ElementPresence<String>,
    pub study_id: ElementPresence<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeriesModuleInput {
    pub modality: String,
    pub series_instance_uid: String,
    pub series_number: ElementPresence<String>,
    pub series_date: ElementPresence<String>,
    pub series_time: ElementPresence<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameOfReferenceModuleInput {
    pub frame_of_reference_uid: String,
    pub position_reference_indicator: ElementPresence<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EquipmentModuleInput {
    pub manufacturer: ElementPresence<String>,
    pub manufacturer_model_name: ElementPresence<String>,
    pub software_versions: ElementPresence<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageModuleInput {
    pub instance_number: ElementPresence<String>,
    pub patient_orientation: ElementPresence<Vec<String>>,
    pub content_date: ElementPresence<String>,
    pub content_time: ElementPresence<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommonModuleRequest {
    pub patient: PatientModuleInput,
    pub study: StudyModuleInput,
    pub series: SeriesModuleInput,
    pub frame_of_reference: Option<FrameOfReferenceModuleInput>,
    pub equipment: EquipmentModuleInput,
    pub image: ImageModuleInput,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleFragment {
    pub module_id: String,
    pub operations: Vec<AttributeOperation>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommonModulePlan {
    pub patient: ModuleFragment,
    pub study: ModuleFragment,
    pub series: ModuleFragment,
    pub frame_of_reference: Option<ModuleFragment>,
    pub equipment: ModuleFragment,
    pub image: ModuleFragment,
}

impl CommonModulePlan {
    pub fn fragments(&self) -> impl Iterator<Item = &ModuleFragment> {
        [
            Some(&self.patient),
            Some(&self.study),
            Some(&self.series),
            self.frame_of_reference.as_ref(),
            Some(&self.equipment),
            Some(&self.image),
        ]
        .into_iter()
        .flatten()
    }

    pub fn operations(&self) -> Vec<AttributeOperation> {
        self.fragments()
            .flat_map(|fragment| fragment.operations.iter().cloned())
            .collect()
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct CommonModuleProvider;

impl CommonModuleProvider {
    pub fn plan(&self, input: CommonModuleRequest) -> Result<CommonModulePlan, ClassicPlanError> {
        require_value("study_instance_uid", &input.study.study_instance_uid)?;
        require_value("modality", &input.series.modality)?;
        require_value("series_instance_uid", &input.series.series_instance_uid)?;

        let patient = fragment(
            "patient",
            [
                text(
                    "0008,0005",
                    DicomVr::CS,
                    input.patient.specific_character_set,
                )?,
                text("0010,0010", DicomVr::PN, input.patient.patient_name)?,
                text("0010,0020", DicomVr::LO, input.patient.patient_id)?,
                text("0010,0030", DicomVr::DA, input.patient.patient_birth_date)?,
                text("0010,0040", DicomVr::CS, input.patient.patient_sex)?,
            ]
            .into_iter()
            .flatten()
            .collect(),
        )?;
        let study = fragment(
            "study",
            vec![Some(set_string(
                "0020,000D",
                DicomVr::UI,
                input.study.study_instance_uid,
            ))]
            .into_iter()
            .chain([
                text("0008,0020", DicomVr::DA, input.study.study_date)?,
                text("0008,0030", DicomVr::TM, input.study.study_time)?,
                text("0008,0050", DicomVr::SH, input.study.accession_number)?,
                text(
                    "0008,0090",
                    DicomVr::PN,
                    input.study.referring_physician_name,
                )?,
                text("0020,0010", DicomVr::SH, input.study.study_id)?,
            ])
            .flatten()
            .collect(),
        )?;
        let series = fragment(
            "series",
            vec![
                Some(set_string("0008,0060", DicomVr::CS, input.series.modality)),
                Some(set_string(
                    "0020,000E",
                    DicomVr::UI,
                    input.series.series_instance_uid,
                )),
                text("0020,0011", DicomVr::IS, input.series.series_number)?,
                text("0008,0021", DicomVr::DA, input.series.series_date)?,
                text("0008,0031", DicomVr::TM, input.series.series_time)?,
            ]
            .into_iter()
            .flatten()
            .collect(),
        )?;
        let frame_of_reference = input
            .frame_of_reference
            .map(|input| {
                require_value("frame_of_reference_uid", &input.frame_of_reference_uid)?;
                fragment(
                    "frame_of_reference",
                    vec![
                        Some(set_string(
                            "0020,0052",
                            DicomVr::UI,
                            input.frame_of_reference_uid,
                        )),
                        text("0020,1040", DicomVr::LO, input.position_reference_indicator)?,
                    ]
                    .into_iter()
                    .flatten()
                    .collect(),
                )
            })
            .transpose()?;
        let equipment = fragment(
            "equipment",
            [
                text("0008,0070", DicomVr::LO, input.equipment.manufacturer)?,
                text(
                    "0008,1090",
                    DicomVr::LO,
                    input.equipment.manufacturer_model_name,
                )?,
                text("0018,1020", DicomVr::LO, input.equipment.software_versions)?,
            ]
            .into_iter()
            .flatten()
            .collect(),
        )?;
        let image = fragment(
            "image",
            vec![
                text("0020,0013", DicomVr::IS, input.image.instance_number)?,
                multi_text("0020,0020", DicomVr::CS, input.image.patient_orientation)?,
                text("0008,0023", DicomVr::DA, input.image.content_date)?,
                text("0008,0033", DicomVr::TM, input.image.content_time)?,
            ]
            .into_iter()
            .flatten()
            .collect(),
        )?;
        let plan = CommonModulePlan {
            patient,
            study,
            series,
            frame_of_reference,
            equipment,
            image,
        };
        ensure_unique(plan.operations().iter())?;
        Ok(plan)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RescalePlan {
    pub intercept: String,
    pub slope: String,
    pub rescale_type: ElementPresence<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowPlan {
    pub center: Vec<String>,
    pub width: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassicPixelRequest {
    pub slot: String,
    pub pixels: NativePixelRequest,
    pub rescale: Option<RescalePlan>,
    pub window: Option<WindowPlan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassicPixelPlan {
    pub slot: String,
    pub module: ModuleFragment,
    pub content_request: NativePixelRequest,
    pub content: NativePixelContent,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct ClassicPixelProvider;

impl ClassicPixelProvider {
    pub fn plan(&self, request: ClassicPixelRequest) -> Result<ClassicPixelPlan, ClassicPlanError> {
        self.plan_with_limits(request, NativePixelLimits::default())
    }

    pub fn plan_with_limits(
        &self,
        request: ClassicPixelRequest,
        limits: NativePixelLimits,
    ) -> Result<ClassicPixelPlan, ClassicPlanError> {
        require_identifier("content slot", &request.slot)?;
        let content_request = request.pixels;
        let content = NativePixelFactory.create_with_limits(content_request.clone(), limits)?;
        let shape = &content.plan.shape;
        let mut operations = vec![
            set_unsigned("0028,0002", shape.samples_per_pixel.into()),
            set_string(
                "0028,0004",
                DicomVr::CS,
                photometric_name(shape.photometric_interpretation),
            ),
            set_unsigned("0028,0010", shape.rows.into()),
            set_unsigned("0028,0011", shape.columns.into()),
            set_unsigned("0028,0100", shape.bits_allocated.into()),
            set_unsigned("0028,0101", shape.bits_stored.into()),
            set_unsigned("0028,0102", shape.high_bit.into()),
            set_unsigned("0028,0103", shape.pixel_representation.into()),
        ];
        if shape.frames > 1 {
            operations.push(set_string(
                "0028,0008",
                DicomVr::IS,
                shape.frames.to_string(),
            ));
        }
        if let Some(color) = &shape.color {
            operations.push(set_unsigned(
                "0028,0006",
                u64::from(color.planar_configuration),
            ));
        }
        if let Some(rescale) = request.rescale {
            operations.push(set_string("0028,1052", DicomVr::DS, rescale.intercept));
            operations.push(set_string("0028,1053", DicomVr::DS, rescale.slope));
            if let Some(operation) = text("0028,1054", DicomVr::LO, rescale.rescale_type)? {
                operations.push(operation);
            }
        }
        if let Some(window) = request.window {
            if window.center.is_empty() || window.center.len() != window.width.len() {
                return Err(ClassicPlanError::InvalidWindow);
            }
            operations.push(set_multi_string("0028,1050", DicomVr::DS, window.center));
            operations.push(set_multi_string("0028,1051", DicomVr::DS, window.width));
        }
        if let Some(padding) = &content.padding {
            let vr = if shape.pixel_representation == 0 {
                DicomVr::US
            } else {
                DicomVr::SS
            };
            if vr == DicomVr::US
                && (padding.value < 0 || padding.range_limit.is_some_and(|value| value < 0))
            {
                return Err(ClassicPlanError::InvalidUnsignedPadding(
                    padding
                        .range_limit
                        .filter(|value| *value < 0)
                        .unwrap_or(padding.value),
                ));
            }
            operations.push(set_integer("0028,0120", vr, padding.value)?);
            if let Some(limit) = padding.range_limit {
                operations.push(set_integer("0028,0121", vr, limit)?);
            }
        }
        Ok(ClassicPixelPlan {
            slot: request.slot,
            module: fragment("image_pixel", operations)?,
            content_request,
            content,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FamilyModuleFragment(ModuleFragment);

impl FamilyModuleFragment {
    pub fn new(
        provider_id: impl Into<String>,
        module_id: impl Into<String>,
        operations: Vec<AttributeOperation>,
    ) -> Result<Self, ClassicPlanError> {
        let provider_id = provider_id.into();
        let module_id = module_id.into();
        require_identifier("family provider", &provider_id)?;
        require_identifier("module", &module_id)?;
        for operation in &operations {
            let tag = operation.address().normalized_tag();
            if PROTECTED_TAGS.contains(&tag.as_str()) {
                return Err(ClassicPlanError::ProtectedAttribute(tag));
            }
        }
        Ok(Self(fragment(
            &format!("{provider_id}:{module_id}"),
            operations,
        )?))
    }

    pub fn module(&self) -> &ModuleFragment {
        &self.0
    }
}

/// Family lanes implement this interface without gaining access to execution
/// or publication state. One request may produce several named IOD modules.
pub trait ClassicFamilyProvider<Request> {
    const PROVIDER_ID: &'static str;

    fn plan_family(&self, request: Request) -> Result<Vec<FamilyModuleFragment>, ClassicPlanError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassicInstanceRequest {
    pub logical_id: String,
    pub order: u64,
    pub output_relative_path: OutputRelativePath,
    pub dependencies: Vec<String>,
    pub common: CommonModuleRequest,
    pub sop_class_uid: String,
    pub sop_instance_uid: String,
    pub family: Vec<FamilyModuleFragment>,
    pub pixels: ClassicPixelRequest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassicPlannedInstance {
    pub logical_id: String,
    pub order: u64,
    pub output_relative_path: OutputRelativePath,
    pub dependencies: Vec<String>,
    pub modules: Vec<ModuleFragment>,
    pub pixels: ClassicPixelPlan,
}

impl ClassicPlannedInstance {
    pub fn operations(&self) -> Vec<AttributeOperation> {
        self.modules
            .iter()
            .flat_map(|module| module.operations.iter().cloned())
            .chain(self.pixels.module.operations.iter().cloned())
            .collect()
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct OrderedSeriesProvider;

impl OrderedSeriesProvider {
    pub fn plan(
        &self,
        requests: Vec<ClassicInstanceRequest>,
    ) -> Result<Vec<ClassicPlannedInstance>, ClassicPlanError> {
        let common_provider = CommonModuleProvider;
        let pixel_provider = ClassicPixelProvider;
        let mut instances = Vec::with_capacity(requests.len());
        let mut ids = BTreeSet::new();
        let mut orders = BTreeSet::new();
        let mut output_paths = BTreeSet::new();
        for request in requests {
            require_identifier("logical instance", &request.logical_id)?;
            require_value("sop_class_uid", &request.sop_class_uid)?;
            require_value("sop_instance_uid", &request.sop_instance_uid)?;
            let output_relative_path =
                OutputRelativePath::new(request.output_relative_path.as_str().to_owned())?;
            if !output_paths.insert(output_relative_path.clone()) {
                return Err(ClassicPlanError::DuplicateOutputPath(
                    output_relative_path.to_string(),
                ));
            }
            if !ids.insert(request.logical_id.clone()) {
                return Err(ClassicPlanError::DuplicateLogicalId(request.logical_id));
            }
            if !orders.insert(request.order) {
                return Err(ClassicPlanError::DuplicateOrder(request.order));
            }
            let common = common_provider.plan(request.common)?;
            let mut modules = common.fragments().cloned().collect::<Vec<_>>();
            modules.push(fragment(
                "sop_common",
                vec![
                    set_string("0008,0016", DicomVr::UI, request.sop_class_uid),
                    set_string("0008,0018", DicomVr::UI, request.sop_instance_uid),
                ],
            )?);
            modules.extend(request.family.into_iter().map(|fragment| fragment.0));
            let pixels = pixel_provider.plan(request.pixels)?;
            ensure_unique(
                modules
                    .iter()
                    .flat_map(|module| module.operations.iter())
                    .chain(pixels.module.operations.iter()),
            )?;
            instances.push(ClassicPlannedInstance {
                logical_id: request.logical_id,
                order: request.order,
                output_relative_path,
                dependencies: request.dependencies,
                modules,
                pixels,
            });
        }
        let known = instances
            .iter()
            .map(|instance| instance.logical_id.as_str())
            .collect::<BTreeSet<_>>();
        for instance in &instances {
            let mut dependencies = BTreeSet::new();
            for dependency in &instance.dependencies {
                if dependency == &instance.logical_id
                    || !known.contains(dependency.as_str())
                    || !dependencies.insert(dependency)
                {
                    return Err(ClassicPlanError::InvalidDependency {
                        instance: instance.logical_id.clone(),
                        dependency: dependency.clone(),
                    });
                }
            }
        }
        instances.sort_by(|left, right| {
            (left.order, &left.logical_id).cmp(&(right.order, &right.logical_id))
        });
        Ok(instances)
    }
}

const PROTECTED_TAGS: &[&str] = &[
    "0008,0016",
    "0008,0018",
    "0020,000D",
    "0020,000E",
    "0020,0052",
    "0028,0002",
    "0028,0004",
    "0028,0006",
    "0028,0008",
    "0028,0010",
    "0028,0011",
    "0028,0100",
    "0028,0101",
    "0028,0102",
    "0028,0103",
    "7FE0,0010",
];

fn fragment(
    module_id: &str,
    operations: Vec<AttributeOperation>,
) -> Result<ModuleFragment, ClassicPlanError> {
    require_identifier("module", module_id)?;
    ensure_unique(operations.iter())?;
    Ok(ModuleFragment {
        module_id: module_id.into(),
        operations,
    })
}

fn ensure_unique<'a>(
    operations: impl IntoIterator<Item = &'a AttributeOperation>,
) -> Result<(), ClassicPlanError> {
    let mut tags = BTreeSet::new();
    for operation in operations {
        operation.validate_trusted()?;
        let tag = operation.address().normalized_tag();
        if !tags.insert(tag.clone()) {
            return Err(ClassicPlanError::DuplicateAttribute(tag));
        }
    }
    Ok(())
}

fn text(
    tag: &str,
    vr: DicomVr,
    presence: ElementPresence<String>,
) -> Result<Option<AttributeOperation>, ClassicPlanError> {
    Ok(match presence {
        ElementPresence::Omitted => None,
        ElementPresence::Empty => Some(empty(tag)),
        ElementPresence::Value(value) => {
            if value.is_empty() {
                return Err(ClassicPlanError::EmptyValueMustBeTyped(tag.into()));
            }
            Some(set_string(tag, vr, value))
        }
    })
}

fn multi_text(
    tag: &str,
    vr: DicomVr,
    presence: ElementPresence<Vec<String>>,
) -> Result<Option<AttributeOperation>, ClassicPlanError> {
    Ok(match presence {
        ElementPresence::Omitted => None,
        ElementPresence::Empty => Some(empty(tag)),
        ElementPresence::Value(values) => {
            if values.is_empty() || values.iter().any(String::is_empty) {
                return Err(ClassicPlanError::EmptyValueMustBeTyped(tag.into()));
            }
            Some(set_multi_string(tag, vr, values))
        }
    })
}

fn set_string(tag: &str, vr: DicomVr, value: impl Into<String>) -> AttributeOperation {
    AttributeOperation::Set {
        address: address(tag),
        vr,
        value: AttributeValue::Primitive(PrimitiveValue::String(value.into())),
    }
}

fn set_multi_string(tag: &str, vr: DicomVr, values: Vec<String>) -> AttributeOperation {
    AttributeOperation::Set {
        address: address(tag),
        vr,
        value: AttributeValue::Multi(values.into_iter().map(PrimitiveValue::String).collect()),
    }
}

fn set_unsigned(tag: &str, value: u64) -> AttributeOperation {
    AttributeOperation::Set {
        address: address(tag),
        vr: DicomVr::US,
        value: AttributeValue::Primitive(PrimitiveValue::Unsigned(value)),
    }
}

fn set_integer(tag: &str, vr: DicomVr, value: i64) -> Result<AttributeOperation, ClassicPlanError> {
    let value = if vr == DicomVr::US {
        AttributeValue::Primitive(PrimitiveValue::Unsigned(
            u64::try_from(value).map_err(|_| ClassicPlanError::InvalidUnsignedPadding(value))?,
        ))
    } else {
        AttributeValue::Primitive(PrimitiveValue::Signed(value))
    };
    Ok(AttributeOperation::Set {
        address: address(tag),
        vr,
        value,
    })
}

fn empty(tag: &str) -> AttributeOperation {
    AttributeOperation::Empty {
        address: address(tag),
    }
}

fn address(tag: &str) -> AttributeAddress {
    AttributeAddress::from_normalized_tag(tag).expect("classic module tag is valid")
}

fn photometric_name(value: PhotometricInterpretation) -> &'static str {
    match value {
        PhotometricInterpretation::Monochrome1 => "MONOCHROME1",
        PhotometricInterpretation::Monochrome2 => "MONOCHROME2",
        PhotometricInterpretation::PaletteColor => "PALETTE COLOR",
        PhotometricInterpretation::Rgb => "RGB",
        PhotometricInterpretation::YbrFull => "YBR_FULL",
        PhotometricInterpretation::YbrFull422 => "YBR_FULL_422",
    }
}

fn require_value(field: &'static str, value: &str) -> Result<(), ClassicPlanError> {
    if value.is_empty() {
        Err(ClassicPlanError::MissingRequired(field))
    } else {
        Ok(())
    }
}

fn require_identifier(field: &'static str, value: &str) -> Result<(), ClassicPlanError> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b':'))
    {
        Err(ClassicPlanError::InvalidIdentifier {
            field,
            value: value.into(),
        })
    } else {
        Ok(())
    }
}

#[derive(Debug)]
pub enum ClassicPlanError {
    MissingRequired(&'static str),
    InvalidIdentifier {
        field: &'static str,
        value: String,
    },
    EmptyValueMustBeTyped(String),
    DuplicateAttribute(String),
    ProtectedAttribute(String),
    InvalidWindow,
    InvalidUnsignedPadding(i64),
    DuplicateLogicalId(String),
    DuplicateOrder(u64),
    DuplicateOutputPath(String),
    InvalidDependency {
        instance: String,
        dependency: String,
    },
    Attribute(crate::composition::AttributeError),
    NativePixel(NativePixelError),
    OutputPath(crate::corpus_plan::CorpusPlanError),
}

impl From<crate::composition::AttributeError> for ClassicPlanError {
    fn from(error: crate::composition::AttributeError) -> Self {
        Self::Attribute(error)
    }
}

impl From<NativePixelError> for ClassicPlanError {
    fn from(error: NativePixelError) -> Self {
        Self::NativePixel(error)
    }
}

impl From<crate::corpus_plan::CorpusPlanError> for ClassicPlanError {
    fn from(error: crate::corpus_plan::CorpusPlanError) -> Self {
        Self::OutputPath(error)
    }
}

impl fmt::Display for ClassicPlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for ClassicPlanError {}
