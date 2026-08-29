use std::collections::BTreeSet;

use super::{
    AttributeAddress, AttributeOperation, AttributeValue, CommonModulePlans, DicomVr, IdentityPlan,
    NativePixelPlan, PhotometricInterpretation, PlanarConfiguration, PrimitiveValue, SampleType,
    sop_common_operations,
};

#[derive(Debug, Clone, PartialEq)]
pub struct GeometryPlan {
    pub pixel_spacing: [f64; 2],
    pub image_orientation_patient: [f64; 6],
    pub image_position_patient: [f64; 3],
    pub slice_thickness: Option<f64>,
}

impl GeometryPlan {
    pub fn axial() -> Self {
        Self {
            pixel_spacing: [1.0, 1.0],
            image_orientation_patient: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
            image_position_patient: [0.0, 0.0, 0.0],
            slice_thickness: Some(1.0),
        }
    }

    pub fn operations(&self) -> Result<Vec<AttributeOperation>, ClassicPlanError> {
        if self
            .pixel_spacing
            .iter()
            .any(|value| !value.is_finite() || *value <= 0.0)
            || self
                .image_orientation_patient
                .iter()
                .chain(self.image_position_patient.iter())
                .any(|value| !value.is_finite())
            || self
                .slice_thickness
                .is_some_and(|value| !value.is_finite() || value <= 0.0)
        {
            return Err(ClassicPlanError::InvalidGeometry);
        }
        let mut operations = vec![
            set_multi_strings(
                "0028,0030",
                DicomVr::DS,
                self.pixel_spacing.map(ds).to_vec(),
            ),
            set_multi_strings(
                "0020,0037",
                DicomVr::DS,
                self.image_orientation_patient.map(ds).to_vec(),
            ),
            set_multi_strings(
                "0020,0032",
                DicomVr::DS,
                self.image_position_patient.map(ds).to_vec(),
            ),
        ];
        if let Some(value) = self.slice_thickness {
            operations.push(set_string("0018,0050", DicomVr::DS, ds(value)));
        }
        Ok(operations)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayTransformPlan {
    Identity,
    Inverse,
}

impl DisplayTransformPlan {
    pub fn operations(self) -> Vec<AttributeOperation> {
        vec![set_string(
            "2050,0020",
            DicomVr::CS,
            match self {
                Self::Identity => "IDENTITY",
                Self::Inverse => "INVERSE",
            },
        )]
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetectorPlan {
    pub detector_type: String,
    pub detector_configuration: Option<String>,
}

impl DetectorPlan {
    pub fn operations(&self) -> Vec<AttributeOperation> {
        let mut operations = vec![set_string(
            "0018,7004",
            DicomVr::CS,
            self.detector_type.clone(),
        )];
        if let Some(configuration) = &self.detector_configuration {
            operations.push(set_string("0018,7005", DicomVr::CS, configuration.clone()));
        }
        operations
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcquisitionPlan {
    pub image_type: Vec<String>,
    pub acquisition_number: Option<String>,
    pub body_part_examined: Option<String>,
}

impl AcquisitionPlan {
    pub fn operations(&self) -> Result<Vec<AttributeOperation>, ClassicPlanError> {
        if self.image_type.len() < 2 || self.image_type.iter().any(String::is_empty) {
            return Err(ClassicPlanError::InvalidImageType);
        }
        let mut operations = vec![set_multi_strings(
            "0008,0008",
            DicomVr::CS,
            self.image_type.clone(),
        )];
        if let Some(number) = &self.acquisition_number {
            operations.push(set_string("0020,0012", DicomVr::IS, number.clone()));
        }
        if let Some(body_part) = &self.body_part_examined {
            operations.push(set_string("0018,0015", DicomVr::CS, body_part.clone()));
        }
        Ok(operations)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PixelModulePlan {
    pub rescale_intercept: Option<String>,
    pub rescale_slope: Option<String>,
    pub rescale_type: Option<String>,
}

impl PixelModulePlan {
    pub fn operations(
        &self,
        pixels: &NativePixelPlan,
    ) -> Result<Vec<AttributeOperation>, ClassicPlanError> {
        let shape = &pixels.shape;
        let mut operations = vec![
            set_unsigned("0028,0002", shape.samples_per_pixel as u64),
            set_string(
                "0028,0004",
                DicomVr::CS,
                photometric_name(shape.photometric_interpretation),
            ),
            set_unsigned("0028,0010", shape.rows as u64),
            set_unsigned("0028,0011", shape.columns as u64),
            set_unsigned("0028,0100", shape.bits_allocated as u64),
            set_unsigned("0028,0101", shape.bits_stored as u64),
            set_unsigned("0028,0102", shape.high_bit as u64),
            set_unsigned(
                "0028,0103",
                u64::from(shape.sample_type == SampleType::SignedInteger),
            ),
        ];
        if shape.frames > 1 {
            operations.push(set_string(
                "0028,0008",
                DicomVr::IS,
                shape.frames.to_string(),
            ));
        }
        if let Some(planar) = shape.planar_configuration {
            operations.push(set_unsigned("0028,0006", planar as u64));
        }
        match (&self.rescale_intercept, &self.rescale_slope) {
            (Some(intercept), Some(slope)) => {
                operations.push(set_string("0028,1052", DicomVr::DS, intercept.clone()));
                operations.push(set_string("0028,1053", DicomVr::DS, slope.clone()));
                if let Some(rescale_type) = &self.rescale_type {
                    operations.push(set_string("0028,1054", DicomVr::LO, rescale_type.clone()));
                }
            }
            (None, None) => {}
            _ => return Err(ClassicPlanError::IncompleteRescale),
        }
        Ok(operations)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ClassicImageModulePlans {
    pub common: CommonModulePlans,
    pub sop_common: Vec<AttributeOperation>,
    pub geometry: Option<GeometryPlan>,
    pub display: Option<DisplayTransformPlan>,
    pub detector: Option<DetectorPlan>,
    pub acquisition: AcquisitionPlan,
    pub pixel: PixelModulePlan,
}

impl ClassicImageModulePlans {
    pub fn operations(
        &self,
        pixels: &NativePixelPlan,
    ) -> Result<Vec<AttributeOperation>, ClassicPlanError> {
        let mut operations = self.common.operations();
        operations.extend(self.sop_common.clone());
        if let Some(geometry) = &self.geometry {
            operations.extend(geometry.operations()?);
        }
        if let Some(display) = self.display {
            operations.extend(display.operations());
        }
        if let Some(detector) = &self.detector {
            operations.extend(detector.operations());
        }
        operations.extend(self.acquisition.operations()?);
        operations.extend(self.pixel.operations(pixels)?);
        ensure_unique(&operations)?;
        for operation in &operations {
            operation.validate()?;
        }
        Ok(operations)
    }

    pub fn synthetic(
        modality: &str,
        sop_class_uid: &str,
        identities: &IdentityPlan,
        include_geometry: bool,
    ) -> Result<Self, ClassicPlanError> {
        Ok(Self {
            common: CommonModulePlans::synthetic(modality, identities, include_geometry)?,
            sop_common: sop_common_operations(sop_class_uid, identities)?,
            geometry: include_geometry.then(GeometryPlan::axial),
            display: None,
            detector: None,
            acquisition: AcquisitionPlan {
                image_type: vec!["ORIGINAL".into(), "PRIMARY".into()],
                acquisition_number: Some("1".into()),
                body_part_examined: None,
            },
            pixel: PixelModulePlan {
                rescale_intercept: None,
                rescale_slope: None,
                rescale_type: None,
            },
        })
    }
}

fn ensure_unique(operations: &[AttributeOperation]) -> Result<(), ClassicPlanError> {
    let mut tags = BTreeSet::new();
    for operation in operations {
        let tag = operation.address().normalized_tag();
        if !tags.insert(tag.clone()) {
            return Err(ClassicPlanError::DuplicateAttribute(tag));
        }
    }
    Ok(())
}

fn set_string(tag: &str, vr: DicomVr, value: impl Into<String>) -> AttributeOperation {
    AttributeOperation::Set {
        address: AttributeAddress::from_normalized_tag(tag).expect("classic tag is valid"),
        vr,
        value: AttributeValue::Primitive(PrimitiveValue::String(value.into())),
    }
}

fn set_multi_strings(tag: &str, vr: DicomVr, values: Vec<String>) -> AttributeOperation {
    AttributeOperation::Set {
        address: AttributeAddress::from_normalized_tag(tag).expect("classic tag is valid"),
        vr,
        value: AttributeValue::Multi(values.into_iter().map(PrimitiveValue::String).collect()),
    }
}

fn set_unsigned(tag: &str, value: u64) -> AttributeOperation {
    AttributeOperation::Set {
        address: AttributeAddress::from_normalized_tag(tag).expect("classic tag is valid"),
        vr: DicomVr::US,
        value: AttributeValue::Primitive(PrimitiveValue::Unsigned(value)),
    }
}

fn ds(value: f64) -> String {
    if value == 0.0 {
        "0".into()
    } else if value.fract() == 0.0 {
        format!("{value:.1}")
    } else {
        value.to_string()
    }
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

#[derive(Debug)]
pub enum ClassicPlanError {
    Module(super::ModuleError),
    Attribute(super::AttributeError),
    InvalidGeometry,
    InvalidImageType,
    IncompleteRescale,
    DuplicateAttribute(String),
}

impl From<super::ModuleError> for ClassicPlanError {
    fn from(error: super::ModuleError) -> Self {
        Self::Module(error)
    }
}

impl From<super::AttributeError> for ClassicPlanError {
    fn from(error: super::AttributeError) -> Self {
        Self::Attribute(error)
    }
}

impl std::fmt::Display for ClassicPlanError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for ClassicPlanError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::composition::{
        ByteOrder, CompositionUidRole, IdentityAllocator, PixelShape, TemplateId,
    };

    fn identities() -> IdentityPlan {
        IdentityAllocator::new(
            "823230c5932b81b504434330d118fba286d5ff41d4e2f7766372633f4a49e559",
            TemplateId("classic/ct".into()),
            "1.0.0".parse().unwrap(),
            1,
        )
        .unwrap()
        .allocate_plan(
            "primary",
            [
                (CompositionUidRole::StudyInstance, 0),
                (CompositionUidRole::SeriesInstance, 0),
                (CompositionUidRole::SopInstance, 0),
                (CompositionUidRole::FrameOfReference, 0),
            ],
        )
        .unwrap()
    }

    fn pixels() -> NativePixelPlan {
        NativePixelPlan::plan(PixelShape {
            rows: 16,
            columns: 16,
            frames: 1,
            samples_per_pixel: 1,
            photometric_interpretation: PhotometricInterpretation::Monochrome2,
            sample_type: SampleType::SignedInteger,
            bits_allocated: 16,
            bits_stored: 12,
            high_bit: 11,
            byte_order: ByteOrder::Little,
            planar_configuration: None,
        })
        .unwrap()
    }

    #[test]
    fn shared_classic_plans_emit_unique_typed_geometry_acquisition_and_pixels() {
        let mut plan = ClassicImageModulePlans::synthetic(
            "CT",
            "1.2.840.10008.5.1.4.1.1.2",
            &identities(),
            true,
        )
        .unwrap();
        plan.pixel = PixelModulePlan {
            rescale_intercept: Some("-1024".into()),
            rescale_slope: Some("1".into()),
            rescale_type: Some("HU".into()),
        };
        let operations = plan.operations(&pixels()).unwrap();
        for tag in [
            "0020,0032",
            "0020,0037",
            "0028,0030",
            "0028,0103",
            "0028,1052",
        ] {
            assert!(
                operations
                    .iter()
                    .any(|operation| operation.address().normalized_tag() == tag)
            );
        }
    }

    #[test]
    fn invalid_geometry_and_partial_rescale_fail_before_materialization() {
        let mut geometry = GeometryPlan::axial();
        geometry.pixel_spacing[0] = 0.0;
        assert!(matches!(
            geometry.operations(),
            Err(ClassicPlanError::InvalidGeometry)
        ));
        let pixels = pixels();
        let partial = PixelModulePlan {
            rescale_intercept: Some("0".into()),
            rescale_slope: None,
            rescale_type: None,
        };
        assert!(matches!(
            partial.operations(&pixels),
            Err(ClassicPlanError::IncompleteRescale)
        ));
    }

    #[test]
    fn color_pixel_plan_projects_planar_configuration_conditionally() {
        let rgb = NativePixelPlan::plan(PixelShape {
            rows: 2,
            columns: 2,
            frames: 1,
            samples_per_pixel: 3,
            photometric_interpretation: PhotometricInterpretation::Rgb,
            sample_type: SampleType::UnsignedInteger,
            bits_allocated: 8,
            bits_stored: 8,
            high_bit: 7,
            byte_order: ByteOrder::Little,
            planar_configuration: Some(PlanarConfiguration::Interleaved),
        })
        .unwrap();
        let operations = PixelModulePlan {
            rescale_intercept: None,
            rescale_slope: None,
            rescale_type: None,
        }
        .operations(&rgb)
        .unwrap();
        assert!(
            operations
                .iter()
                .any(|operation| operation.address().normalized_tag() == "0028,0006")
        );
    }
}
