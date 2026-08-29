use std::path::PathBuf;

use dicom_test_suite::composition::{
    AttributeAddress, AttributeOperation, AttributeValue, DicomVr, PrimitiveValue, TemplateCatalog,
};
use dicom_test_suite::corpus_plan::OutputRelativePath;
use dicom_test_suite::native_pixel::{
    ByteOrder, ChromaSubsampling, ColorOrganization, NativePixelLimits, NativePixelRequest,
    PhotometricInterpretation, PixelDataVr, PixelPadding, PixelShape, StoredValueType,
};
use dicom_test_suite::recipes::{
    ClassicInstanceRequest, ClassicPixelProvider, ClassicPixelRequest, ClassicPlanError,
    ClassicResolvedPlanInput, CommonModuleProvider, CommonModuleRequest, DeclaredVrException,
    ElementPresence, EquipmentModuleInput, FamilyModuleFragment, FrameOfReferenceModuleInput,
    ImageModuleInput, OrderedSeriesProvider, PatientModuleInput, SeriesModuleInput,
    StudyModuleInput, resolved_classic_instance_plan,
};

fn common(instance: &str, series: &str) -> CommonModuleRequest {
    CommonModuleRequest {
        patient: PatientModuleInput {
            specific_character_set: ElementPresence::Value("ISO_IR 192".into()),
            patient_name: ElementPresence::Value("EXACT^PATIENT".into()),
            patient_id: ElementPresence::Value("EXACT-ID".into()),
            patient_birth_date: ElementPresence::Empty,
            patient_sex: ElementPresence::Omitted,
        },
        study: StudyModuleInput {
            study_instance_uid: format!("1.2.826.0.1.{instance}"),
            study_date: ElementPresence::Value("20260101".into()),
            study_time: ElementPresence::Value("010203".into()),
            accession_number: ElementPresence::Empty,
            referring_physician_name: ElementPresence::Empty,
            study_id: ElementPresence::Value("STUDY-X".into()),
        },
        series: SeriesModuleInput {
            modality: "CT".into(),
            series_instance_uid: format!("1.2.826.0.2.{series}"),
            series_number: ElementPresence::Value(series.into()),
            series_date: ElementPresence::Omitted,
            series_time: ElementPresence::Omitted,
        },
        frame_of_reference: Some(FrameOfReferenceModuleInput {
            frame_of_reference_uid: "1.2.826.0.3.1".into(),
            position_reference_indicator: ElementPresence::Empty,
        }),
        equipment: EquipmentModuleInput {
            manufacturer: ElementPresence::Value("Exact Manufacturer".into()),
            manufacturer_model_name: ElementPresence::Value("Exact Model".into()),
            software_versions: ElementPresence::Value("9.8.7".into()),
        },
        image: ImageModuleInput {
            instance_number: ElementPresence::Value(instance.into()),
            patient_orientation: ElementPresence::Empty,
            content_date: ElementPresence::Value("20260101".into()),
            content_time: ElementPresence::Value("010203".into()),
        },
    }
}

fn mono_request(stored: StoredValueType, frames: u32) -> NativePixelRequest {
    let signed = matches!(
        stored,
        StoredValueType::I8 | StoredValueType::I16 | StoredValueType::I32
    );
    let bits = stored.bits_allocated();
    let frame_values: Vec<i64> = if signed {
        vec![-2, -1, 0, 1]
    } else {
        vec![0, 1, 2, 3]
    };
    let mut values = Vec::new();
    for _ in 0..frames {
        values.extend(frame_values.iter().copied());
    }
    NativePixelRequest {
        shape: PixelShape {
            rows: 2,
            columns: 2,
            frames,
            samples_per_pixel: 1,
            photometric_interpretation: PhotometricInterpretation::Monochrome2,
            bits_allocated: bits,
            bits_stored: bits.min(12),
            high_bit: bits.min(12) - 1,
            pixel_representation: stored.pixel_representation(),
            stored_value_type: stored,
            byte_order: ByteOrder::Little,
            pixel_data_vr: if bits <= 8 {
                PixelDataVr::Ob
            } else {
                PixelDataVr::Ow
            },
            color: None,
        },
        declared_pixel_min: *values.iter().min().unwrap(),
        declared_pixel_max: *values.iter().max().unwrap(),
        stored_values: values,
        expected_frame_sha256: vec![],
        padding: None,
        palette: None,
    }
}

fn rgb_request(frames: u32) -> NativePixelRequest {
    let mut values = Vec::new();
    for frame in 0..frames {
        for value in [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11] {
            values.push(i64::from(value + frame as i32));
        }
    }
    NativePixelRequest {
        shape: PixelShape {
            rows: 2,
            columns: 2,
            frames,
            samples_per_pixel: 3,
            photometric_interpretation: PhotometricInterpretation::Rgb,
            bits_allocated: 8,
            bits_stored: 8,
            high_bit: 7,
            pixel_representation: 0,
            stored_value_type: StoredValueType::U8,
            byte_order: ByteOrder::Little,
            pixel_data_vr: PixelDataVr::Ob,
            color: Some(ColorOrganization {
                planar_configuration: 1,
                chroma_subsampling: ChromaSubsampling::None,
            }),
        },
        declared_pixel_min: 0,
        declared_pixel_max: 11 + i64::from(frames - 1),
        stored_values: values,
        expected_frame_sha256: vec![],
        padding: None,
        palette: None,
    }
}

fn string_value(operation: &AttributeOperation) -> Option<&str> {
    match operation {
        AttributeOperation::Set {
            value: AttributeValue::Primitive(PrimitiveValue::String(value)),
            ..
        } => Some(value),
        _ => None,
    }
}

fn op(tag: &str, vr: DicomVr, value: &str) -> AttributeOperation {
    AttributeOperation::Set {
        address: AttributeAddress::from_normalized_tag(tag).unwrap(),
        vr,
        value: AttributeValue::Primitive(PrimitiveValue::String(value.into())),
    }
}

#[test]
fn common_modules_preserve_exact_values_and_tri_state() {
    let plan = CommonModuleProvider.plan(common("7", "4")).unwrap();
    let operations = plan.operations();
    let find = |tag: &str| {
        operations
            .iter()
            .find(|operation| operation.address().normalized_tag() == tag)
            .unwrap()
    };
    assert_eq!(string_value(find("0010,0010")), Some("EXACT^PATIENT"));
    assert!(matches!(
        find("0010,0030"),
        AttributeOperation::Empty { .. }
    ));
    assert!(
        !operations
            .iter()
            .any(|operation| operation.address().normalized_tag() == "0010,0040")
    );
    assert!(matches!(
        find("0020,0020"),
        AttributeOperation::Empty { .. }
    ));
    assert_eq!(string_value(find("0020,0013")), Some("7"));
    assert_eq!(string_value(find("0020,0011")), Some("4"));
}

#[test]
fn family_fragments_reject_protected_and_duplicate_tags() {
    let protected = FamilyModuleFragment::new(
        "classic.ct",
        "geometry",
        vec![op("0020,000D", DicomVr::UI, "1.2.3")],
    );
    assert!(matches!(
        protected,
        Err(ClassicPlanError::ProtectedAttribute(_))
    ));

    let duplicate = FamilyModuleFragment::new(
        "classic.ct",
        "acquisition",
        vec![
            op("0018,0060", DicomVr::DS, "120"),
            op("0018,0060", DicomVr::DS, "100"),
        ],
    );
    assert!(matches!(
        duplicate,
        Err(ClassicPlanError::DuplicateAttribute(_))
    ));
}

#[test]
fn family_fragments_require_an_exact_named_declared_vr_exception() {
    let operation = AttributeOperation::Set {
        address: AttributeAddress::from_normalized_tag("0018,1149").unwrap(),
        vr: DicomVr::DS,
        value: AttributeValue::Multi(vec![
            PrimitiveValue::String("0.30".into()),
            PrimitiveValue::String("0.30".into()),
        ]),
    };
    assert!(matches!(
        FamilyModuleFragment::new("classic.dx", "detector", vec![operation.clone()]),
        Err(ClassicPlanError::Attribute(_))
    ));

    let exception = DeclaredVrException::new(
        "0018,1149",
        DicomVr::DS,
        "legacy.dx.field_of_view_dimensions.ds",
    )
    .unwrap();
    let fragment = FamilyModuleFragment::new_with_declared_vr_exceptions(
        "classic.dx",
        "detector",
        vec![operation],
        &[exception],
    )
    .unwrap();
    assert_eq!(fragment.module().operations.len(), 1);
    let planned = OrderedSeriesProvider
        .plan(vec![ClassicInstanceRequest {
            logical_id: "declared_vr".into(),
            order: 1,
            output_relative_path: OutputRelativePath::new("classic/dx/declared-vr.dcm").unwrap(),
            dependencies: vec![],
            common: common("1", "1"),
            sop_class_uid: "1.2.840.10008.5.1.4.1.1.1.1".into(),
            sop_instance_uid: "1.2.826.0.4.10".into(),
            implementation_class_uid: "1.2.826.0.5.1".into(),
            family: vec![fragment],
            pixels: ClassicPixelRequest {
                slot: "pixels".into(),
                pixels: mono_request(StoredValueType::U16, 1),
                rescale: None,
                window: None,
            },
        }])
        .unwrap();
    assert_eq!(planned.len(), 1);

    let unused = DeclaredVrException::new(
        "0018,1149",
        DicomVr::DS,
        "legacy.dx.field_of_view_dimensions.ds",
    )
    .unwrap();
    assert!(matches!(
        FamilyModuleFragment::new_with_declared_vr_exceptions(
            "classic.dx",
            "detector",
            vec![op("0018,0015", DicomVr::CS, "CHEST")],
            &[unused],
        ),
        Err(ClassicPlanError::UnusedDeclaredVrException(_))
    ));
}

#[test]
fn pixel_provider_plans_signed_unsigned_color_and_multiframe_content() {
    for stored in [
        StoredValueType::U8,
        StoredValueType::I16,
        StoredValueType::U32,
    ] {
        let output = ClassicPixelProvider
            .plan(ClassicPixelRequest {
                slot: "pixels".into(),
                pixels: mono_request(stored, 2),
                rescale: None,
                window: None,
            })
            .unwrap();
        assert_eq!(output.content.frames.len(), 2);
        assert_eq!(
            output.content.plan.shape.pixel_representation,
            stored.pixel_representation()
        );
        assert_eq!(
            output.content.unpadded_bytes.len() as u64,
            output.content.plan.unpadded_value_bytes
        );
    }

    let color = ClassicPixelProvider
        .plan(ClassicPixelRequest {
            slot: "pixels".into(),
            pixels: rgb_request(2),
            rescale: None,
            window: None,
        })
        .unwrap();
    assert_eq!(color.content.frames.len(), 2);
    assert!(color.module.operations.iter().any(|operation| {
        operation.address().normalized_tag() == "0028,0006"
            && matches!(
                operation,
                AttributeOperation::Set {
                    value: AttributeValue::Primitive(PrimitiveValue::Unsigned(1)),
                    ..
                }
            )
    }));
}

#[test]
fn pixel_provider_applies_bounds_before_large_allocation() {
    let error = ClassicPixelProvider
        .plan_with_limits(
            ClassicPixelRequest {
                slot: "pixels".into(),
                pixels: mono_request(StoredValueType::U8, 2),
                rescale: None,
                window: None,
            },
            NativePixelLimits {
                max_frames: 1,
                max_stored_values: 8,
                max_value_bytes: 8,
            },
        )
        .unwrap_err();
    assert!(matches!(error, ClassicPlanError::NativePixel(_)));
}

#[test]
fn padding_is_typed_from_pixel_representation() {
    let mut request = mono_request(StoredValueType::I16, 1);
    request.padding = Some(PixelPadding {
        value: -2048,
        range_limit: Some(-2040),
    });
    let output = ClassicPixelProvider
        .plan(ClassicPixelRequest {
            slot: "pixels".into(),
            pixels: request,
            rescale: None,
            window: None,
        })
        .unwrap();
    assert!(output.module.operations.iter().any(|operation| matches!(
        operation,
        AttributeOperation::Set {
            vr: DicomVr::SS,
            value: AttributeValue::Primitive(PrimitiveValue::Signed(-2048)),
            ..
        }
    )));
}

#[test]
fn ordered_series_is_deterministic_and_rejects_collisions() {
    let make = |logical_id: &str, order: u64, instance: &str| ClassicInstanceRequest {
        logical_id: logical_id.into(),
        order,
        output_relative_path: OutputRelativePath::new(format!("classic/ct/slice-{instance}.dcm"))
            .unwrap(),
        dependencies: vec![],
        common: common(instance, "1"),
        sop_class_uid: "1.2.840.10008.5.1.4.1.1.2".into(),
        sop_instance_uid: format!("1.2.826.0.4.{instance}"),
        implementation_class_uid: "1.2.826.0.5.1".into(),
        family: vec![],
        pixels: ClassicPixelRequest {
            slot: "pixels".into(),
            pixels: mono_request(StoredValueType::I16, 1),
            rescale: None,
            window: None,
        },
    };
    let planned = OrderedSeriesProvider
        .plan(vec![make("slice_b", 2, "2"), make("slice_a", 1, "1")])
        .unwrap();
    assert_eq!(
        planned
            .iter()
            .map(|item| item.logical_id.as_str())
            .collect::<Vec<_>>(),
        ["slice_a", "slice_b"]
    );
    assert_eq!(
        planned[0].output_relative_path.as_str(),
        "classic/ct/slice-1.dcm"
    );

    let duplicate =
        OrderedSeriesProvider.plan(vec![make("slice_a", 1, "1"), make("slice_b", 1, "2")]);
    assert!(matches!(
        duplicate,
        Err(ClassicPlanError::DuplicateOrder(1))
    ));

    let first = make("slice_a", 1, "1");
    let mut second = make("slice_b", 2, "2");
    second.output_relative_path = first.output_relative_path.clone();
    let duplicate_path = OrderedSeriesProvider.plan(vec![first, second]);
    assert!(matches!(
        duplicate_path,
        Err(ClassicPlanError::DuplicateOutputPath(_))
    ));
}

#[test]
fn ordered_classic_output_resolves_once_into_the_neutral_instance_plan() {
    let request = ClassicInstanceRequest {
        logical_id: "slice_a".into(),
        order: 1,
        output_relative_path: OutputRelativePath::new("classic/ct/slice-a.dcm").unwrap(),
        dependencies: vec![],
        common: common("1", "1"),
        sop_class_uid: "1.2.840.10008.5.1.4.1.1.2".into(),
        sop_instance_uid: "1.2.826.0.4.1".into(),
        implementation_class_uid: "1.2.826.0.5.1".into(),
        family: vec![],
        pixels: ClassicPixelRequest {
            slot: "pixels".into(),
            pixels: mono_request(StoredValueType::I16, 1),
            rescale: None,
            window: None,
        },
    };
    let planned = OrderedSeriesProvider.plan(vec![request]).unwrap().remove(0);
    let catalog = TemplateCatalog::load("templates/catalog.json").unwrap();
    let template = catalog
        .templates
        .iter()
        .find(|template| template.template_id.0 == "classic/ct")
        .unwrap();
    let resolved = resolved_classic_instance_plan(ClassicResolvedPlanInput {
        planned,
        template,
        transfer_syntax_uid: "1.2.840.10008.1.2.1",
        encoding_backend_id: "dicom-rs.part10",
    })
    .unwrap();

    assert_eq!(resolved.instance_id, "slice_a");
    assert_eq!(resolved.template_id.0, "classic/ct");
    assert_eq!(resolved.content.len(), 1);
    assert!(resolved.attributes.iter().any(|attribute| {
        attribute.address.normalized_tag() == "0010,0010"
            && attribute.value
                == Some(AttributeValue::Primitive(PrimitiveValue::String(
                    "EXACT^PATIENT".into(),
                )))
    }));
}

#[test]
fn planning_has_no_filesystem_or_frontend_channel() {
    let source = include_str!("../src/recipes/classic.rs");
    for forbidden in [
        "std::fs",
        "crate::generator",
        "PathBuf",
        "out_dir",
        "output_root",
        "Part10Materializer",
    ] {
        assert!(
            !source.contains(forbidden),
            "classic planning source contains {forbidden}"
        );
    }

    let absent = PathBuf::from(format!(
        "/tmp/dicom-test-suite-classic-plan-must-not-exist-{}",
        std::process::id()
    ));
    assert!(!absent.exists());
    let _ = CommonModuleProvider.plan(common("1", "1")).unwrap();
    assert!(!absent.exists());
}
