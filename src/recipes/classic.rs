//! Parameterized, plan-only building blocks for classic image families.
//!
//! Values are supplied by callers: this module owns no curated defaults,
//! identity allocation, filesystem paths, writers, or execution services.

pub const CLASSIC_PIXEL_SLOT: &str = "pixels";

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::str::FromStr;

use dicom_core::dictionary::{DataDictionary, DataDictionaryEntry, VirtualVr};
use dicom_dictionary_std::StandardDataDictionary;

use crate::composition::{
    AttributeAddress, AttributeOperation, AttributeValue, ByteOrder as CompositionByteOrder,
    CompositionUidRole, DicomVr, IdentityPlan, NativePixelPlan as CompositionPixelPlan,
    PhotometricInterpretation as CompositionPhotometric, PixelShape as CompositionPixelShape,
    PlanarConfiguration, PrimitiveValue, ResolvedAttribute, ResolvedInstancePlan,
    SampleType as CompositionSampleType, TemplateDescriptor, TemplateStatus, ValueOrigin,
    canonical_native_pixels,
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
        Self::new_with_declared_vr_exceptions(provider_id, module_id, operations, &[])
    }

    pub fn new_with_declared_vr_exceptions(
        provider_id: impl Into<String>,
        module_id: impl Into<String>,
        operations: Vec<AttributeOperation>,
        exceptions: &[DeclaredVrException],
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
        ensure_unique_with_declared_vr_exceptions(operations.iter(), exceptions)?;
        Ok(Self(ModuleFragment {
            module_id: format!("{provider_id}:{module_id}"),
            operations,
        }))
    }

    pub fn module(&self) -> &ModuleFragment {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclaredVrException {
    address: AttributeAddress,
    vr: DicomVr,
    contract_id: String,
}

impl DeclaredVrException {
    pub fn new(
        tag: &str,
        vr: DicomVr,
        contract_id: impl Into<String>,
    ) -> Result<Self, ClassicPlanError> {
        let contract_id = contract_id.into();
        require_identifier("declared VR contract", &contract_id)?;
        Ok(Self {
            address: AttributeAddress::from_normalized_tag(tag)?,
            vr,
            contract_id,
        })
    }

    pub fn contract_id(&self) -> &str {
        &self.contract_id
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
    pub implementation_class_uid: String,
    pub family: Vec<FamilyModuleFragment>,
    pub pixels: ClassicPixelRequest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassicPlannedInstance {
    pub logical_id: String,
    pub order: u64,
    pub output_relative_path: OutputRelativePath,
    pub dependencies: Vec<String>,
    pub sop_class_uid: String,
    pub identities: IdentityPlan,
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
            require_value(
                "implementation_class_uid",
                &request.implementation_class_uid,
            )?;
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
            let identity_values = [
                (
                    CompositionUidRole::StudyInstance,
                    0,
                    request.common.study.study_instance_uid.clone(),
                ),
                (
                    CompositionUidRole::SeriesInstance,
                    0,
                    request.common.series.series_instance_uid.clone(),
                ),
                (
                    CompositionUidRole::SopInstance,
                    0,
                    request.sop_instance_uid.clone(),
                ),
                (
                    CompositionUidRole::ImplementationClass,
                    0,
                    request.implementation_class_uid.clone(),
                ),
            ];
            let mut identity_values = identity_values.into_iter().collect::<Vec<_>>();
            if let Some(frame) = &request.common.frame_of_reference {
                identity_values.push((
                    CompositionUidRole::FrameOfReference,
                    0,
                    frame.frame_of_reference_uid.clone(),
                ));
            }
            let identities =
                IdentityPlan::from_exact_values(request.logical_id.clone(), identity_values)?;
            let common = common_provider.plan(request.common)?;
            let mut modules = common.fragments().cloned().collect::<Vec<_>>();
            modules.push(fragment(
                "sop_common",
                vec![
                    set_string("0008,0016", DicomVr::UI, request.sop_class_uid.clone()),
                    set_string("0008,0018", DicomVr::UI, request.sop_instance_uid),
                ],
            )?);
            modules.extend(request.family.into_iter().map(|fragment| fragment.0));
            let pixels = pixel_provider.plan(request.pixels)?;
            ensure_unique_tags(
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
                sop_class_uid: request.sop_class_uid,
                identities,
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

pub struct ClassicResolvedPlanInput<'a> {
    pub planned: ClassicPlannedInstance,
    pub template: &'a TemplateDescriptor,
    pub transfer_syntax_uid: &'a str,
    pub encoding_backend_id: &'a str,
}

pub fn resolved_classic_instance_plan(
    input: ClassicResolvedPlanInput<'_>,
) -> Result<ResolvedInstancePlan, ClassicPlanError> {
    if input.template.status != TemplateStatus::Qualified {
        return Err(ClassicPlanError::TemplateNotQualified(
            input.template.template_id.0.clone(),
        ));
    }
    if input.template.sop_class_uid != input.planned.sop_class_uid {
        return Err(ClassicPlanError::TemplateSopClassMismatch {
            expected: input.template.sop_class_uid.clone(),
            actual: input.planned.sop_class_uid.clone(),
        });
    }
    let template_supports_transfer_syntax = input
        .template
        .transfer_syntaxes
        .iter()
        .any(|syntax| syntax.uid == input.transfer_syntax_uid);
    if !template_supports_transfer_syntax
        && !super::encoding::qualifies_non_template_transfer_syntax(
            input.transfer_syntax_uid,
            input.encoding_backend_id,
        )
    {
        return Err(ClassicPlanError::UnsupportedTransferSyntax(
            input.transfer_syntax_uid.to_string(),
        ));
    }
    let mut attributes = BTreeMap::new();
    for operation in input.planned.operations() {
        let address = operation.address().clone();
        let (vr, value) = match operation {
            AttributeOperation::Set { vr, value, .. } => (vr, Some(value)),
            AttributeOperation::Empty { .. } => {
                let vr = dictionary_vr(&address).ok_or_else(|| {
                    ClassicPlanError::MissingEmptyAttributeVr(address.normalized_tag())
                })?;
                (vr, None)
            }
            AttributeOperation::Remove { .. } => continue,
        };
        attributes.insert(
            address.clone(),
            ResolvedAttribute {
                address,
                vr,
                value,
                origin: ValueOrigin::InstanceOverride,
            },
        );
    }
    let content = classic_canonical_pixel_content(&input.planned.pixels.content)?;
    Ok(ResolvedInstancePlan {
        plan_schema_version: "0.1.0".into(),
        instance_id: input.planned.logical_id,
        template_id: input.template.template_id.clone(),
        template_version: input.template.template_version,
        sop_class_uid: input.template.sop_class_uid.clone(),
        transfer_syntax_uid: input.transfer_syntax_uid.into(),
        identities: input.planned.identities,
        attributes: attributes.into_values().collect(),
        content: vec![content],
        references: Vec::new(),
    })
}

fn classic_canonical_pixel_content(
    native: &NativePixelContent,
) -> Result<crate::composition::CanonicalContent, ClassicPlanError> {
    let shape = &native.plan.shape;
    let composition_shape = CompositionPixelShape {
        rows: shape.rows,
        columns: shape.columns,
        frames: shape.frames,
        samples_per_pixel: u8::try_from(shape.samples_per_pixel)
            .map_err(|_| ClassicPlanError::NumericRange)?,
        photometric_interpretation: match shape.photometric_interpretation {
            PhotometricInterpretation::Monochrome1 => CompositionPhotometric::Monochrome1,
            PhotometricInterpretation::Monochrome2 => CompositionPhotometric::Monochrome2,
            PhotometricInterpretation::PaletteColor => CompositionPhotometric::PaletteColor,
            PhotometricInterpretation::Rgb => CompositionPhotometric::Rgb,
            PhotometricInterpretation::YbrFull => CompositionPhotometric::YbrFull,
            PhotometricInterpretation::YbrFull422 => CompositionPhotometric::YbrFull422,
        },
        sample_type: match shape.stored_value_type {
            crate::native_pixel::StoredValueType::U1 => CompositionSampleType::Bit1,
            crate::native_pixel::StoredValueType::U8
            | crate::native_pixel::StoredValueType::U16
            | crate::native_pixel::StoredValueType::U32 => CompositionSampleType::UnsignedInteger,
            crate::native_pixel::StoredValueType::I8
            | crate::native_pixel::StoredValueType::I16
            | crate::native_pixel::StoredValueType::I32 => CompositionSampleType::SignedInteger,
        },
        bits_allocated: u8::try_from(shape.bits_allocated)
            .map_err(|_| ClassicPlanError::NumericRange)?,
        bits_stored: u8::try_from(shape.bits_stored).map_err(|_| ClassicPlanError::NumericRange)?,
        high_bit: u8::try_from(shape.high_bit).map_err(|_| ClassicPlanError::NumericRange)?,
        byte_order: match shape.byte_order {
            crate::native_pixel::ByteOrder::Little => CompositionByteOrder::Little,
            crate::native_pixel::ByteOrder::Big => CompositionByteOrder::Big,
        },
        planar_configuration: shape
            .color
            .as_ref()
            .map(|color| match color.planar_configuration {
                0 => Ok(PlanarConfiguration::Interleaved),
                1 => Ok(PlanarConfiguration::Planar),
                value => Err(ClassicPlanError::InvalidPlanarConfiguration(value)),
            })
            .transpose()?,
    };
    let plan = CompositionPixelPlan::plan(composition_shape)?;
    if plan.unpadded_value_bytes != native.plan.unpadded_value_bytes
        || plan.padded_value_bytes != native.plan.padded_value_bytes
        || plan.padding_bytes != native.plan.padding_bytes
    {
        return Err(ClassicPlanError::PixelPlanMismatch);
    }
    let mut content = canonical_native_pixels(
        &plan,
        native.unpadded_bytes.clone(),
        BTreeMap::from([(
            "decoded_frame_sha256".into(),
            serde_json::json!(
                native
                    .frames
                    .iter()
                    .map(|frame| frame.decoded_sha256.as_str())
                    .collect::<Vec<_>>()
            )
            .to_string(),
        )]),
    );
    content.vr = match shape.pixel_data_vr {
        crate::native_pixel::PixelDataVr::Ob => DicomVr::OB,
        crate::native_pixel::PixelDataVr::Ow => DicomVr::OW,
    };
    Ok(content)
}

fn dictionary_vr(address: &AttributeAddress) -> Option<DicomVr> {
    let entry = StandardDataDictionary.by_tag(address.tag())?;
    let vr = match entry.vr() {
        VirtualVr::Exact(vr) => vr,
        _ => return None,
    };
    DicomVr::from_str(&vr.to_string()).ok()
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

fn ensure_unique_tags<'a>(
    operations: impl IntoIterator<Item = &'a AttributeOperation>,
) -> Result<(), ClassicPlanError> {
    let mut tags = BTreeSet::new();
    for operation in operations {
        let tag = operation.address().normalized_tag();
        if !tags.insert(tag.clone()) {
            return Err(ClassicPlanError::DuplicateAttribute(tag));
        }
    }
    Ok(())
}

fn ensure_unique_with_declared_vr_exceptions<'a>(
    operations: impl IntoIterator<Item = &'a AttributeOperation>,
    exceptions: &[DeclaredVrException],
) -> Result<(), ClassicPlanError> {
    let mut tags = BTreeSet::new();
    let mut used = BTreeSet::new();
    for operation in operations {
        let exception = match operation {
            AttributeOperation::Set { address, vr, .. } => exceptions
                .iter()
                .enumerate()
                .find(|(_, exception)| exception.address == *address && exception.vr == *vr),
            _ => None,
        };
        if let Some((index, _)) = exception {
            operation.validate_declared_vr()?;
            used.insert(index);
        } else {
            operation.validate_trusted()?;
        }
        let tag = operation.address().normalized_tag();
        if !tags.insert(tag.clone()) {
            return Err(ClassicPlanError::DuplicateAttribute(tag));
        }
    }
    if used.len() != exceptions.len() {
        return Err(ClassicPlanError::UnusedDeclaredVrException(
            exceptions
                .iter()
                .enumerate()
                .find(|(index, _)| !used.contains(index))
                .expect("exception count differs")
                .1
                .contract_id
                .clone(),
        ));
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
    TemplateNotQualified(String),
    TemplateSopClassMismatch {
        expected: String,
        actual: String,
    },
    UnsupportedTransferSyntax(String),
    MissingEmptyAttributeVr(String),
    NumericRange,
    InvalidPlanarConfiguration(u8),
    PixelPlanMismatch,
    UnusedDeclaredVrException(String),
    Attribute(crate::composition::AttributeError),
    Identity(crate::composition::IdentityError),
    PixelPlan(crate::composition::PixelError),
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

impl From<crate::composition::IdentityError> for ClassicPlanError {
    fn from(error: crate::composition::IdentityError) -> Self {
        Self::Identity(error)
    }
}

impl From<crate::composition::PixelError> for ClassicPlanError {
    fn from(error: crate::composition::PixelError) -> Self {
        Self::PixelPlan(error)
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
