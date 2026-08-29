use std::collections::{BTreeMap, BTreeSet};
use std::str::FromStr;

use dicom_core::dictionary::{DataDictionary, DataDictionaryEntry, VirtualVr};
use dicom_dictionary_std::StandardDataDictionary;

use super::{
    AttributeAddress, AttributeItem, AttributeOperation, AttributeValue, CanonicalContent,
    ClassicImageModulePlans, ClassicPlanError, ContentMaterialization, DetectorPlan, DicomVr,
    DisplayTransformPlan, IdentityPlan, NativePixelPlan, PhotometricInterpretation,
    PixelModulePlan, PixelShape, PlanarConfiguration, PrimitiveValue, ResolvedAttribute,
    SampleType, TemplateId, ValueOrigin,
};
use crate::sha256_hex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClassicFamilyKind {
    ScSingleBit,
    ScGrayscaleByte,
    Cr,
    Ct,
    Mr,
    DxPresentation,
    MammographyPresentation,
    MammographyProcessing,
    UltrasoundSingleFrame,
    UltrasoundMultiFrame,
    NuclearMedicine,
    Pet,
    VlEndoscopic,
    VlMicroscopic,
    VlPhotographic,
    Xa,
    Xrf,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ClassicFamilyProfile {
    pub kind: ClassicFamilyKind,
    pub template_id: TemplateId,
    pub iod_name: &'static str,
    pub sop_class_name: &'static str,
    pub sop_class_uid: &'static str,
    pub modality: &'static str,
    pub include_geometry: bool,
    pub default_shape: PixelShape,
    pub default_transfer_syntax_uid: &'static str,
}

impl ClassicFamilyProfile {
    pub fn for_template(template_id: &TemplateId) -> Option<Self> {
        let mono = |bits: u8, stored: u8, sample_type| PixelShape {
            rows: 16,
            columns: 16,
            frames: 1,
            samples_per_pixel: 1,
            photometric_interpretation: PhotometricInterpretation::Monochrome2,
            sample_type,
            bits_allocated: bits,
            bits_stored: stored,
            high_bit: stored - 1,
            byte_order: super::ByteOrder::Little,
            planar_configuration: None,
        };
        let rgb = || PixelShape {
            rows: 16,
            columns: 16,
            frames: 1,
            samples_per_pixel: 3,
            photometric_interpretation: PhotometricInterpretation::Rgb,
            sample_type: SampleType::UnsignedInteger,
            bits_allocated: 8,
            bits_stored: 8,
            high_bit: 7,
            byte_order: super::ByteOrder::Little,
            planar_configuration: Some(PlanarConfiguration::Interleaved),
        };
        let (kind, iod_name, sop_name, sop_uid, modality, geometry, shape, transfer_syntax) =
            match template_id.0.as_str() {
                "classic/secondary-capture/multiframe-single-bit" => (
                    ClassicFamilyKind::ScSingleBit,
                    "Multi-frame Single Bit Secondary Capture Image",
                    "Multi-frame Single Bit Secondary Capture Image Storage",
                    "1.2.840.10008.5.1.4.1.1.7.1",
                    "OT",
                    false,
                    PixelShape {
                        rows: 16,
                        columns: 16,
                        frames: 2,
                        samples_per_pixel: 1,
                        photometric_interpretation: PhotometricInterpretation::Monochrome2,
                        sample_type: SampleType::Bit1,
                        bits_allocated: 1,
                        bits_stored: 1,
                        high_bit: 0,
                        byte_order: super::ByteOrder::Little,
                        planar_configuration: None,
                    },
                    "1.2.840.10008.1.2.1",
                ),
                "classic/secondary-capture/multiframe-grayscale-byte" => (
                    ClassicFamilyKind::ScGrayscaleByte,
                    "Multi-frame Grayscale Byte Secondary Capture Image",
                    "Multi-frame Grayscale Byte Secondary Capture Image Storage",
                    "1.2.840.10008.5.1.4.1.1.7.2",
                    "OT",
                    false,
                    PixelShape {
                        frames: 2,
                        ..mono(8, 8, SampleType::UnsignedInteger)
                    },
                    "1.2.840.10008.1.2.1",
                ),
                "classic/cr" => (
                    ClassicFamilyKind::Cr,
                    "Computed Radiography Image",
                    "Computed Radiography Image Storage",
                    "1.2.840.10008.5.1.4.1.1.1",
                    "CR",
                    false,
                    mono(16, 12, SampleType::UnsignedInteger),
                    "1.2.840.10008.1.2.1",
                ),
                "classic/ct" => (
                    ClassicFamilyKind::Ct,
                    "CT Image",
                    "CT Image Storage",
                    "1.2.840.10008.5.1.4.1.1.2",
                    "CT",
                    true,
                    mono(16, 12, SampleType::SignedInteger),
                    "1.2.840.10008.1.2.1",
                ),
                "classic/mr" => (
                    ClassicFamilyKind::Mr,
                    "MR Image",
                    "MR Image Storage",
                    "1.2.840.10008.5.1.4.1.1.4",
                    "MR",
                    true,
                    mono(16, 12, SampleType::UnsignedInteger),
                    "1.2.840.10008.1.2.1",
                ),
                "classic/dx/for-presentation" => (
                    ClassicFamilyKind::DxPresentation,
                    "Digital X-Ray Image",
                    "Digital X-Ray Image Storage - For Presentation",
                    "1.2.840.10008.5.1.4.1.1.1.1",
                    "DX",
                    false,
                    mono(16, 12, SampleType::UnsignedInteger),
                    "1.2.840.10008.1.2.1",
                ),
                "classic/mammography/for-presentation" => (
                    ClassicFamilyKind::MammographyPresentation,
                    "Digital Mammography X-Ray Image",
                    "Digital Mammography X-Ray Image Storage - For Presentation",
                    "1.2.840.10008.5.1.4.1.1.1.2",
                    "MG",
                    false,
                    PixelShape {
                        photometric_interpretation: PhotometricInterpretation::Monochrome1,
                        ..mono(16, 12, SampleType::UnsignedInteger)
                    },
                    "1.2.840.10008.1.2.1",
                ),
                "classic/mammography/for-processing" => (
                    ClassicFamilyKind::MammographyProcessing,
                    "Digital Mammography X-Ray Image",
                    "Digital Mammography X-Ray Image Storage - For Processing",
                    "1.2.840.10008.5.1.4.1.1.1.2.1",
                    "MG",
                    false,
                    mono(16, 12, SampleType::UnsignedInteger),
                    "1.2.840.10008.1.2",
                ),
                "classic/ultrasound/single-frame" => (
                    ClassicFamilyKind::UltrasoundSingleFrame,
                    "Ultrasound Image",
                    "Ultrasound Image Storage",
                    "1.2.840.10008.5.1.4.1.1.6.1",
                    "US",
                    false,
                    mono(8, 8, SampleType::UnsignedInteger),
                    "1.2.840.10008.1.2.1",
                ),
                "classic/ultrasound/multiframe" => (
                    ClassicFamilyKind::UltrasoundMultiFrame,
                    "Ultrasound Multi-frame Image",
                    "Ultrasound Multi-frame Image Storage",
                    "1.2.840.10008.5.1.4.1.1.3.1",
                    "US",
                    false,
                    PixelShape {
                        frames: 2,
                        ..mono(8, 8, SampleType::UnsignedInteger)
                    },
                    "1.2.840.10008.1.2.1",
                ),
                "classic/nuclear-medicine" => (
                    ClassicFamilyKind::NuclearMedicine,
                    "Nuclear Medicine Image",
                    "Nuclear Medicine Image Storage",
                    "1.2.840.10008.5.1.4.1.1.20",
                    "NM",
                    false,
                    PixelShape {
                        frames: 2,
                        ..mono(16, 16, SampleType::UnsignedInteger)
                    },
                    "1.2.840.10008.1.2.1",
                ),
                "classic/pet" => (
                    ClassicFamilyKind::Pet,
                    "PET Image",
                    "Positron Emission Tomography Image Storage",
                    "1.2.840.10008.5.1.4.1.1.128",
                    "PT",
                    true,
                    mono(16, 16, SampleType::UnsignedInteger),
                    "1.2.840.10008.1.2.1",
                ),
                "vl/endoscopic" => (
                    ClassicFamilyKind::VlEndoscopic,
                    "VL Endoscopic Image",
                    "VL Endoscopic Image Storage",
                    "1.2.840.10008.5.1.4.1.1.77.1.1",
                    "ES",
                    false,
                    rgb(),
                    "1.2.840.10008.1.2.1",
                ),
                "vl/microscopic" => (
                    ClassicFamilyKind::VlMicroscopic,
                    "VL Microscopic Image",
                    "VL Microscopic Image Storage",
                    "1.2.840.10008.5.1.4.1.1.77.1.2",
                    "GM",
                    false,
                    rgb(),
                    "1.2.840.10008.1.2.1",
                ),
                "vl/photographic" => (
                    ClassicFamilyKind::VlPhotographic,
                    "VL Photographic Image",
                    "VL Photographic Image Storage",
                    "1.2.840.10008.5.1.4.1.1.77.1.4",
                    "XC",
                    false,
                    rgb(),
                    "1.2.840.10008.1.2.1",
                ),
                "classic/xa" => (
                    ClassicFamilyKind::Xa,
                    "X-Ray Angiographic Image",
                    "X-Ray Angiographic Image Storage",
                    "1.2.840.10008.5.1.4.1.1.12.1",
                    "XA",
                    false,
                    mono(16, 12, SampleType::UnsignedInteger),
                    "1.2.840.10008.1.2.1",
                ),
                "classic/xrf" => (
                    ClassicFamilyKind::Xrf,
                    "X-Ray Radiofluoroscopic Image",
                    "X-Ray Radiofluoroscopic Image Storage",
                    "1.2.840.10008.5.1.4.1.1.12.2",
                    "RF",
                    false,
                    mono(16, 12, SampleType::UnsignedInteger),
                    "1.2.840.10008.1.2.1",
                ),
                _ => return None,
            };
        Some(Self {
            kind,
            template_id: template_id.clone(),
            iod_name,
            sop_class_name: sop_name,
            sop_class_uid: sop_uid,
            modality,
            include_geometry: geometry,
            default_shape: shape,
            default_transfer_syntax_uid: transfer_syntax,
        })
    }

    pub fn module_plans(
        &self,
        identities: &IdentityPlan,
    ) -> Result<ClassicImageModulePlans, FamilyError> {
        let mut plans = ClassicImageModulePlans::synthetic(
            self.modality,
            self.sop_class_uid,
            identities,
            self.include_geometry,
        )?;
        match self.kind {
            ClassicFamilyKind::Ct => {
                plans.acquisition.image_type.push("AXIAL".into());
                plans.pixel = PixelModulePlan {
                    rescale_intercept: Some("-1024".into()),
                    rescale_slope: Some("1".into()),
                    rescale_type: Some("HU".into()),
                };
                plans.acquisition.body_part_examined = Some("HEAD".into());
            }
            ClassicFamilyKind::Mr => {
                plans.acquisition.image_type.push("OTHER".into());
                plans.acquisition.body_part_examined = Some("HEAD".into());
            }
            ClassicFamilyKind::Pet => {
                plans.acquisition.body_part_examined = Some("HEAD".into());
                plans.pixel = PixelModulePlan {
                    rescale_intercept: Some("0".into()),
                    rescale_slope: Some("1".into()),
                    rescale_type: None,
                };
            }
            ClassicFamilyKind::Cr => {
                plans.acquisition.body_part_examined = Some("CHEST".into());
            }
            ClassicFamilyKind::DxPresentation | ClassicFamilyKind::MammographyPresentation => {
                plans.acquisition.body_part_examined = Some("CHEST".into());
                plans.display = Some(DisplayTransformPlan::Identity);
                plans.detector = Some(DetectorPlan {
                    detector_type: "DIRECT".into(),
                    detector_configuration: Some("AREA".into()),
                });
            }
            ClassicFamilyKind::MammographyProcessing => {
                plans.acquisition.body_part_examined = Some("CHEST".into());
                plans.detector = Some(DetectorPlan {
                    detector_type: "DIRECT".into(),
                    detector_configuration: Some("AREA".into()),
                });
            }
            ClassicFamilyKind::Xa | ClassicFamilyKind::Xrf => {
                plans.acquisition.image_type.push("SINGLE PLANE".into());
                plans.acquisition.body_part_examined = Some(
                    if self.kind == ClassicFamilyKind::Xa {
                        "HEART"
                    } else {
                        "ABDOMEN"
                    }
                    .into(),
                );
            }
            _ => {}
        }
        Ok(plans)
    }

    pub fn family_operations(&self, pixels: &NativePixelPlan) -> Vec<AttributeOperation> {
        match self.kind {
            ClassicFamilyKind::ScSingleBit | ClassicFamilyKind::ScGrayscaleByte => {
                let mut operations = vec![
                    set_string("0008,0064", DicomVr::CS, "WSD"),
                    set_string("0018,1016", DicomVr::LO, "OpenAI"),
                    set_string("0018,1018", DicomVr::LO, "DICOM Test Suite"),
                    set_string("0018,1019", DicomVr::LO, "0.1"),
                    set_string("0020,0060", DicomVr::CS, "R"),
                    set_string("0028,0009", DicomVr::AT, "0018,1063"),
                    set_string("0018,1063", DicomVr::DS, "100"),
                    set_string("0028,0301", DicomVr::CS, "NO"),
                ];
                if self.kind == ClassicFamilyKind::ScGrayscaleByte {
                    operations.extend([
                        set_string("0028,1052", DicomVr::DS, "0"),
                        set_string("0028,1053", DicomVr::DS, "1"),
                        set_string("0028,1054", DicomVr::LO, "US"),
                        set_string("2050,0020", DicomVr::CS, "IDENTITY"),
                    ]);
                }
                operations
            }
            ClassicFamilyKind::Cr => vec![
                set_string("0018,0060", DicomVr::DS, "70"),
                set_string("0018,1405", DicomVr::IS, "200"),
                set_string("0018,5101", DicomVr::CS, "PA"),
            ],
            ClassicFamilyKind::Ct => vec![
                set_string("0018,0060", DicomVr::DS, "120"),
                set_string("0018,1210", DicomVr::SH, "STANDARD"),
                set_string("0018,5100", DicomVr::CS, "HFS"),
            ],
            ClassicFamilyKind::Mr => vec![
                set_string("0018,0020", DicomVr::CS, "SE"),
                set_string("0018,0021", DicomVr::CS, "SK"),
                empty("0018,0022"),
                set_string("0018,0023", DicomVr::CS, "2D"),
                set_string("0018,0080", DicomVr::DS, "500"),
                set_string("0018,0081", DicomVr::DS, "10"),
                set_string("0018,0087", DicomVr::DS, "1.5"),
                set_string("0018,0091", DicomVr::IS, "1"),
                set_string("0018,5100", DicomVr::CS, "HFS"),
            ],
            ClassicFamilyKind::DxPresentation
            | ClassicFamilyKind::MammographyPresentation
            | ClassicFamilyKind::MammographyProcessing => {
                let mammography = self.kind != ClassicFamilyKind::DxPresentation;
                let presentation = self.kind != ClassicFamilyKind::MammographyProcessing;
                let mut operations = vec![
                    set_string(
                        "0008,0068",
                        DicomVr::CS,
                        if presentation {
                            "FOR PRESENTATION"
                        } else {
                            "FOR PROCESSING"
                        },
                    ),
                    set_sequence(
                        "0008,2218",
                        vec![
                            set_string(
                                "0008,0100",
                                DicomVr::SH,
                                if mammography { "76752008" } else { "51185008" },
                            ),
                            set_string("0008,0102", DicomVr::SH, "SCT"),
                            set_string(
                                "0008,0104",
                                DicomVr::LO,
                                if mammography { "Breast" } else { "Chest" },
                            ),
                        ],
                    ),
                    set_string(
                        "0018,1508",
                        DicomVr::CS,
                        if mammography { "MAMMOGRAPHIC" } else { "CARM" },
                    ),
                    set_multi_strings("0018,1164", DicomVr::DS, ["0.15", "0.15"]),
                    set_multi_strings("0020,0020", DicomVr::CS, ["P", "F"]),
                    set_string("0020,0062", DicomVr::CS, "R"),
                    set_string("0028,0301", DicomVr::CS, "NO"),
                    set_string("0028,1040", DicomVr::CS, "LIN"),
                    set_signed("0028,1041", DicomVr::SS, -1),
                    set_string("0028,1052", DicomVr::DS, "0"),
                    set_string("0028,1053", DicomVr::DS, "1"),
                    set_string("0028,1054", DicomVr::LO, "US"),
                    set_string("0028,2110", DicomVr::CS, "00"),
                    set_string(
                        "2050,0020",
                        DicomVr::CS,
                        if mammography && presentation {
                            "INVERSE"
                        } else {
                            "IDENTITY"
                        },
                    ),
                    set_string("0040,0555", DicomVr::SQ, ""),
                    set_sequence(
                        "0054,0220",
                        vec![
                            set_string(
                                "0008,0100",
                                DicomVr::SH,
                                if mammography {
                                    "399368009"
                                } else {
                                    "399033003"
                                },
                            ),
                            set_string("0008,0102", DicomVr::SH, "SCT"),
                            set_string(
                                "0008,0104",
                                DicomVr::LO,
                                if mammography {
                                    "Medio-lateral oblique"
                                } else {
                                    "Postero-anterior"
                                },
                            ),
                            set_string("0054,0222", DicomVr::SQ, ""),
                        ],
                    ),
                ];
                if !mammography {
                    operations.push(set_string("0018,5101", DicomVr::CS, "PA"));
                }
                if presentation {
                    operations.extend([
                        set_string("0028,1050", DicomVr::DS, "2048"),
                        set_string("0028,1051", DicomVr::DS, "4096"),
                    ]);
                }
                if mammography {
                    operations.push(set_string("0040,0318", DicomVr::CS, "BREAST"));
                }
                operations
            }
            ClassicFamilyKind::UltrasoundSingleFrame => vec![
                set_string("0020,0060", DicomVr::CS, "R"),
                set_unsigned("0028,0014", 0),
                set_string("0028,2110", DicomVr::CS, "00"),
            ],
            ClassicFamilyKind::UltrasoundMultiFrame => vec![
                set_string(
                    "0008,0008",
                    DicomVr::CS,
                    "ORIGINAL\\PRIMARY\\ABDOMINAL\\0001",
                ),
                set_string("0018,0015", DicomVr::CS, "ABDOMEN"),
                set_string("0018,1063", DicomVr::DS, "100"),
                set_string("0028,0009", DicomVr::AT, "0018,1063"),
                set_unsigned("0028,0014", 0),
                set_string("0028,2110", DicomVr::CS, "00"),
            ],
            ClassicFamilyKind::NuclearMedicine => vec![
                set_string(
                    "0008,0008",
                    DicomVr::CS,
                    "ORIGINAL\\PRIMARY\\STATIC\\EMISSION",
                ),
                set_string("0020,0060", DicomVr::CS, "R"),
                set_string("0018,0070", DicomVr::IS, "2"),
                set_string("0018,1242", DicomVr::IS, "1000"),
                set_tags("0028,0009", ["0054,0010", "0054,0020"]),
                set_multi_strings("0028,0030", DicomVr::DS, ["4", "4"]),
                set_unsigned_multi(
                    "0054,0010",
                    std::iter::repeat_n(1, pixels.shape.frames as usize),
                ),
                set_unsigned("0054,0011", 1),
                set_string("0054,0012", DicomVr::SQ, ""),
                set_string("0054,0016", DicomVr::SQ, ""),
                set_unsigned_multi(
                    "0054,0020",
                    std::iter::repeat_n(1, pixels.shape.frames as usize),
                ),
                set_unsigned("0054,0021", 1),
                set_string("0054,0022", DicomVr::SQ, ""),
                set_string("0054,0410", DicomVr::SQ, ""),
                set_string("0054,0414", DicomVr::SQ, ""),
            ],
            ClassicFamilyKind::Pet => vec![
                set_string("0008,0021", DicomVr::DA, "20000101"),
                set_string("0008,0022", DicomVr::DA, "20000101"),
                set_string("0008,0031", DicomVr::TM, "000000"),
                set_string("0008,0032", DicomVr::TM, "000000"),
                set_string("0018,1181", DicomVr::CS, "NONE"),
                set_string("0018,1242", DicomVr::IS, "1000"),
                set_string("0028,0051", DicomVr::CS, "DCAL"),
                set_string("0028,2110", DicomVr::CS, "00"),
                set_string("0054,0016", DicomVr::SQ, ""),
                set_unsigned("0054,0081", 1),
                set_string("0054,0410", DicomVr::SQ, ""),
                set_string("0054,0414", DicomVr::SQ, ""),
                set_string("0054,1000", DicomVr::CS, "STATIC\\IMAGE"),
                set_string("0054,1001", DicomVr::CS, "BQML"),
                set_string("0054,1002", DicomVr::CS, "EMISSION"),
                set_string("0054,1102", DicomVr::CS, "NONE"),
                set_string("0054,1103", DicomVr::LO, "NONE"),
                set_string("0054,1300", DicomVr::DS, "0"),
                set_unsigned("0054,1330", 1),
            ],
            ClassicFamilyKind::VlEndoscopic
            | ClassicFamilyKind::VlMicroscopic
            | ClassicFamilyKind::VlPhotographic => vec![
                set_string("0008,002A", DicomVr::DT, "20000101000000+0000"),
                set_string("0020,0060", DicomVr::CS, "R"),
                set_string("0028,2110", DicomVr::CS, "00"),
                set_string("0040,0555", DicomVr::SQ, ""),
            ],
            ClassicFamilyKind::Xa | ClassicFamilyKind::Xrf => {
                let xa = self.kind == ClassicFamilyKind::Xa;
                let mut operations = vec![
                    set_string("0018,0060", DicomVr::DS, if xa { "80" } else { "70" }),
                    set_string("0018,1110", DicomVr::DS, "1200"),
                    set_string("0018,1111", DicomVr::DS, "800"),
                    set_string("0018,1114", DicomVr::DS, "1.5"),
                    set_string("0018,1152", DicomVr::IS, if xa { "4" } else { "1" }),
                    set_string("0018,1155", DicomVr::CS, if xa { "GR" } else { "SC" }),
                    set_multi_strings("0018,1164", DicomVr::DS, ["0.2", "0.2"]),
                    set_string("0028,1040", DicomVr::CS, "LIN"),
                    set_string("2050,0020", DicomVr::CS, "IDENTITY"),
                ];
                if xa {
                    operations.extend([
                        set_string("0018,1510", DicomVr::DS, "15"),
                        set_string("0018,1511", DicomVr::DS, "-10"),
                    ]);
                } else {
                    operations.push(set_string("0018,1450", DicomVr::DS, "10"));
                }
                operations
            }
        }
    }
}

pub fn default_family_pixels(
    profile: &ClassicFamilyProfile,
) -> Result<(NativePixelPlan, CanonicalContent), FamilyError> {
    let plan = NativePixelPlan::plan(profile.default_shape.clone())?;
    let mut bytes = vec![0_u8; plan.unpadded_value_bytes as usize];
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = ((index * 37 + 11) & 0xff) as u8;
    }
    if profile.default_shape.sample_type == SampleType::Bit1 {
        for (index, byte) in bytes.iter_mut().enumerate() {
            *byte = if index % 2 == 0 { 0x55 } else { 0xAA };
        }
    }
    let content = CanonicalContent {
        slot: "pixels".into(),
        kind: "native_pixels".into(),
        address: AttributeAddress::from_normalized_tag("7FE0,0010").unwrap(),
        vr: if profile.default_shape.bits_allocated <= 8 {
            DicomVr::OB
        } else {
            DicomVr::OW
        },
        size_bytes: bytes.len() as u64,
        sha256: sha256_hex(&bytes),
        properties: BTreeMap::new(),
        placement: super::ContentPlacement::TopLevel,
        materialization: Some(ContentMaterialization::Inline(bytes)),
    };
    Ok((plan, content))
}

pub fn resolve_family_attributes(
    profile: &ClassicFamilyProfile,
    identities: &IdentityPlan,
    pixels: &NativePixelPlan,
    run_defaults: &[AttributeOperation],
    caller: &[AttributeOperation],
) -> Result<Vec<ResolvedAttribute>, FamilyError> {
    let module_plans = profile.module_plans(identities)?;
    let mut template_operations = module_plans.operations(pixels)?;
    template_operations.extend(profile.family_operations(pixels));
    let protected = protected_tags();
    let mut state = BTreeMap::<AttributeAddress, ResolvedAttribute>::new();
    let mut known_vrs = BTreeMap::<AttributeAddress, DicomVr>::new();
    for operation in template_operations {
        apply_operation(
            &mut state,
            &mut known_vrs,
            &operation,
            if protected.contains(&operation.address().normalized_tag().as_str()) {
                ValueOrigin::DerivedStructural
            } else {
                ValueOrigin::TemplateDefault
            },
            false,
            &protected,
        )?;
    }
    for operation in run_defaults {
        apply_operation(
            &mut state,
            &mut known_vrs,
            operation,
            ValueOrigin::RunDefault,
            true,
            &protected,
        )?;
    }
    for operation in caller {
        apply_operation(
            &mut state,
            &mut known_vrs,
            operation,
            ValueOrigin::InstanceOverride,
            true,
            &protected,
        )?;
    }
    Ok(state.into_values().collect())
}

fn apply_operation(
    state: &mut BTreeMap<AttributeAddress, ResolvedAttribute>,
    known_vrs: &mut BTreeMap<AttributeAddress, DicomVr>,
    operation: &AttributeOperation,
    origin: ValueOrigin,
    caller: bool,
    protected: &BTreeSet<&'static str>,
) -> Result<(), FamilyError> {
    operation.validate_trusted()?;
    let address = operation.address().clone();
    let tag = address.normalized_tag();
    if caller && protected.contains(tag.as_str()) {
        return Err(FamilyError::ProtectedCollision(tag));
    }
    match operation {
        AttributeOperation::Set { vr, value, .. } => {
            known_vrs.insert(address.clone(), *vr);
            state.insert(
                address.clone(),
                ResolvedAttribute {
                    address,
                    vr: *vr,
                    value: Some(value.clone()),
                    origin,
                },
            );
        }
        AttributeOperation::Empty { .. } => {
            let vr = known_vrs
                .get(&address)
                .copied()
                .or_else(|| dictionary_vr(&address))
                .ok_or_else(|| FamilyError::UnknownEmptyVr(tag.clone()))?;
            known_vrs.insert(address.clone(), vr);
            state.insert(
                address.clone(),
                ResolvedAttribute {
                    address,
                    vr,
                    value: None,
                    origin,
                },
            );
        }
        AttributeOperation::Remove { .. } => {
            state.remove(&address);
        }
    }
    Ok(())
}

fn dictionary_vr(address: &AttributeAddress) -> Option<DicomVr> {
    let entry = StandardDataDictionary.by_tag(address.tag())?;
    let vr = match entry.vr() {
        VirtualVr::Exact(vr) => vr,
        _ => return None,
    };
    DicomVr::from_str(&vr.to_string()).ok()
}

fn protected_tags() -> BTreeSet<&'static str> {
    BTreeSet::from([
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
    ])
}

fn set_string(tag: &str, vr: DicomVr, value: impl Into<String>) -> AttributeOperation {
    let value = value.into();
    if vr == DicomVr::AT {
        return AttributeOperation::Set {
            address: AttributeAddress::from_normalized_tag(tag).unwrap(),
            vr,
            value: AttributeValue::Primitive(PrimitiveValue::Tag(
                AttributeAddress::from_normalized_tag(&value).unwrap(),
            )),
        };
    }
    if vr == DicomVr::SQ {
        return AttributeOperation::Set {
            address: AttributeAddress::from_normalized_tag(tag).unwrap(),
            vr,
            value: AttributeValue::Sequence(Vec::new()),
        };
    }
    if value.contains('\\') {
        return AttributeOperation::Set {
            address: AttributeAddress::from_normalized_tag(tag).unwrap(),
            vr,
            value: AttributeValue::Multi(
                value
                    .split('\\')
                    .map(|item| PrimitiveValue::String(item.into()))
                    .collect(),
            ),
        };
    }
    AttributeOperation::Set {
        address: AttributeAddress::from_normalized_tag(tag).unwrap(),
        vr,
        value: AttributeValue::Primitive(PrimitiveValue::String(value)),
    }
}

fn set_unsigned(tag: &str, value: u64) -> AttributeOperation {
    AttributeOperation::Set {
        address: AttributeAddress::from_normalized_tag(tag).unwrap(),
        vr: DicomVr::US,
        value: AttributeValue::Primitive(PrimitiveValue::Unsigned(value)),
    }
}

fn set_unsigned_multi(tag: &str, values: impl IntoIterator<Item = u64>) -> AttributeOperation {
    AttributeOperation::Set {
        address: AttributeAddress::from_normalized_tag(tag).unwrap(),
        vr: DicomVr::US,
        value: AttributeValue::Multi(values.into_iter().map(PrimitiveValue::Unsigned).collect()),
    }
}

fn set_tags(tag: &str, values: impl IntoIterator<Item = &'static str>) -> AttributeOperation {
    AttributeOperation::Set {
        address: AttributeAddress::from_normalized_tag(tag).unwrap(),
        vr: DicomVr::AT,
        value: AttributeValue::Multi(
            values
                .into_iter()
                .map(|value| {
                    PrimitiveValue::Tag(AttributeAddress::from_normalized_tag(value).unwrap())
                })
                .collect(),
        ),
    }
}

fn set_multi_strings(
    tag: &str,
    vr: DicomVr,
    values: impl IntoIterator<Item = impl Into<String>>,
) -> AttributeOperation {
    AttributeOperation::Set {
        address: AttributeAddress::from_normalized_tag(tag).unwrap(),
        vr,
        value: AttributeValue::Multi(
            values
                .into_iter()
                .map(|value| PrimitiveValue::String(value.into()))
                .collect(),
        ),
    }
}

fn set_sequence(tag: &str, attributes: Vec<AttributeOperation>) -> AttributeOperation {
    AttributeOperation::Set {
        address: AttributeAddress::from_normalized_tag(tag).unwrap(),
        vr: DicomVr::SQ,
        value: AttributeValue::Sequence(vec![AttributeItem { attributes }]),
    }
}

fn set_signed(tag: &str, vr: DicomVr, value: i64) -> AttributeOperation {
    AttributeOperation::Set {
        address: AttributeAddress::from_normalized_tag(tag).unwrap(),
        vr,
        value: AttributeValue::Primitive(PrimitiveValue::Signed(value)),
    }
}

fn empty(tag: &str) -> AttributeOperation {
    AttributeOperation::Empty {
        address: AttributeAddress::from_normalized_tag(tag).unwrap(),
    }
}

#[derive(Debug)]
pub enum FamilyError {
    Classic(ClassicPlanError),
    Pixel(super::PixelError),
    Attribute(super::AttributeError),
    ProtectedCollision(String),
    UnknownEmptyVr(String),
}

impl From<ClassicPlanError> for FamilyError {
    fn from(error: ClassicPlanError) -> Self {
        Self::Classic(error)
    }
}
impl From<super::PixelError> for FamilyError {
    fn from(error: super::PixelError) -> Self {
        Self::Pixel(error)
    }
}
impl From<super::AttributeError> for FamilyError {
    fn from(error: super::AttributeError) -> Self {
        Self::Attribute(error)
    }
}
impl std::fmt::Display for FamilyError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}
impl std::error::Error for FamilyError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::composition::{CompositionUidRole, IdentityAllocator};

    #[test]
    fn every_p3_family_has_a_valid_bounded_default_pixel_plan() {
        for id in [
            "classic/secondary-capture/multiframe-single-bit",
            "classic/secondary-capture/multiframe-grayscale-byte",
            "classic/cr",
            "classic/ct",
            "classic/mr",
            "classic/dx/for-presentation",
            "classic/mammography/for-presentation",
            "classic/mammography/for-processing",
            "classic/ultrasound/single-frame",
            "classic/ultrasound/multiframe",
            "classic/nuclear-medicine",
            "classic/pet",
            "vl/endoscopic",
            "vl/microscopic",
            "vl/photographic",
            "classic/xa",
            "classic/xrf",
        ] {
            let profile = ClassicFamilyProfile::for_template(&TemplateId(id.into())).unwrap();
            let (plan, content) = default_family_pixels(&profile).unwrap();
            assert!(plan.unpadded_value_bytes <= 4096, "{id}");
            assert_eq!(plan.unpadded_value_bytes, content.size_bytes);
        }
    }

    #[test]
    fn caller_cannot_override_family_identity_or_pixel_shape() {
        let profile = ClassicFamilyProfile::for_template(&TemplateId("classic/ct".into())).unwrap();
        let identities = IdentityAllocator::new(
            "823230c5932b81b504434330d118fba286d5ff41d4e2f7766372633f4a49e559",
            profile.template_id.clone(),
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
        .unwrap();
        let pixels = NativePixelPlan::plan(profile.default_shape.clone()).unwrap();
        let override_rows = set_unsigned("0028,0010", 99);
        assert!(matches!(
            resolve_family_attributes(&profile, &identities, &pixels, &[], &[override_rows]),
            Err(FamilyError::ProtectedCollision(_))
        ));
    }
}
