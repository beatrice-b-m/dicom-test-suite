use dicom_test_suite::composition::{
    AttributeAddress, AttributeOperation, ContentMaterialization, DicomVr,
};
use dicom_test_suite::recipes::{
    BytePayloadContract, CodedConcept, CompletionFlag, ContentByteOrder, ContentProviderError,
    ContentProviderLimits, ContentProviderRequest, ContentTarget, FloatPixelsContract,
    FloatSamples, IntegerPixelsContract, IntegerSamples, MeshContract, MeshFormat,
    NeutralContentProvider, RtObjectKind, RtSemanticContract, SemanticReference,
    SemanticReferenceRole, StructuredReportContract, VerificationFlag, WaveformContract,
};
use dicom_test_suite::sha256_hex;

fn target(slot: &str, kind: &str, vr: DicomVr) -> ContentTarget {
    ContentTarget {
        slot: slot.into(),
        content_kind: kind.into(),
        address: AttributeAddress::from_normalized_tag("7FE0,0010").unwrap(),
        vr,
    }
}

fn bytes(request: ContentProviderRequest) -> Vec<u8> {
    let output = NeutralContentProvider
        .expand(&request, ContentProviderLimits::default())
        .unwrap();
    assert_eq!(output.contents.len(), 1);
    assert_eq!(output.digests.len(), 1);
    let ContentMaterialization::Inline(bytes) =
        output.contents[0].materialization.as_ref().unwrap()
    else {
        panic!("neutral provider must return inline bytes")
    };
    assert_eq!(output.contents[0].sha256, sha256_hex(bytes));
    assert_eq!(output.digests[0].sha256, sha256_hex(bytes));
    bytes.clone()
}

#[test]
fn integer_and_float_expansion_is_exact_bounded_and_deterministic() {
    let signed = ContentProviderRequest::IntegerPixels(IntegerPixelsContract {
        target: target("signed", "signed_pixels", DicomVr::OW),
        dimensions: vec![2, 2],
        bits_allocated: 16,
        byte_order: ContentByteOrder::LittleEndian,
        samples: IntegerSamples::Signed {
            values: vec![i16::MIN.into(), -1, 0, i16::MAX.into()],
        },
    });
    assert_eq!(
        bytes(signed.clone()),
        vec![0x00, 0x80, 0xff, 0xff, 0x00, 0x00, 0xff, 0x7f]
    );
    assert_eq!(bytes(signed.clone()), bytes(signed));

    let unsigned = ContentProviderRequest::IntegerPixels(IntegerPixelsContract {
        target: target("unsigned", "unsigned_pixels", DicomVr::OW),
        dimensions: vec![2],
        bits_allocated: 32,
        byte_order: ContentByteOrder::BigEndian,
        samples: IntegerSamples::Unsigned {
            values: vec![0x0102_0304, u32::MAX.into()],
        },
    });
    assert_eq!(
        bytes(unsigned),
        vec![0x01, 0x02, 0x03, 0x04, 0xff, 0xff, 0xff, 0xff]
    );

    let floats = ContentProviderRequest::FloatPixels(FloatPixelsContract {
        target: target("floats", "float_pixels", DicomVr::OF),
        dimensions: vec![2],
        byte_order: ContentByteOrder::LittleEndian,
        samples: FloatSamples::F32Bits {
            values: vec![1.0_f32.to_bits(), (-0.0_f32).to_bits()],
        },
    });
    assert_eq!(
        bytes(floats),
        [1.0_f32.to_le_bytes(), (-0.0_f32).to_le_bytes()].concat()
    );
    let doubles = ContentProviderRequest::FloatPixels(FloatPixelsContract {
        target: target("doubles", "double_pixels", DicomVr::OD),
        dimensions: vec![1],
        byte_order: ContentByteOrder::BigEndian,
        samples: FloatSamples::F64Bits {
            values: vec![f64::INFINITY.to_bits()],
        },
    });
    assert_eq!(bytes(doubles), f64::INFINITY.to_be_bytes());
}

#[test]
fn malformed_numeric_contracts_fail_before_allocation() {
    let provider = NeutralContentProvider;
    let limits = ContentProviderLimits::default();
    let request = |dimensions, bits, values| {
        ContentProviderRequest::IntegerPixels(IntegerPixelsContract {
            target: target("pixels", "native_pixels", DicomVr::OW),
            dimensions,
            bits_allocated: bits,
            byte_order: ContentByteOrder::LittleEndian,
            samples: IntegerSamples::Unsigned { values },
        })
    };
    assert!(matches!(
        provider.expand(&request(vec![], 16, vec![]), limits),
        Err(ContentProviderError::InvalidDimensions)
    ));
    assert!(matches!(
        provider.expand(&request(vec![2], 12, vec![0, 1]), limits),
        Err(ContentProviderError::InvalidBitsAllocated(12))
    ));
    assert!(matches!(
        provider.expand(&request(vec![2], 8, vec![0]), limits),
        Err(ContentProviderError::SampleCount { .. })
    ));
    assert!(matches!(
        provider.expand(&request(vec![1], 8, vec![256]), limits),
        Err(ContentProviderError::IntegerRange)
    ));
    assert!(matches!(
        provider.expand(
            &request(vec![u32::MAX; 8], 8, vec![]),
            ContentProviderLimits {
                max_elements: 64 * 1024 * 1024,
                ..limits
            }
        ),
        Err(ContentProviderError::ArithmeticOverflow | ContentProviderError::ElementCount { .. })
    ));
    assert!(matches!(
        ContentProviderLimits {
            max_output_bytes: 0,
            ..limits
        }
        .validate(),
        Err(ContentProviderError::InvalidLimit {
            field: "max_output_bytes",
            ..
        })
    ));
}

#[test]
fn waveform_contract_enforces_multiplex_cardinality_and_order() {
    let request = ContentProviderRequest::Waveform(WaveformContract {
        target: target("waveform", "waveform_multiplex", DicomVr::OW),
        channels: 2,
        samples_per_channel: 3,
        bits_allocated: 16,
        byte_order: ContentByteOrder::LittleEndian,
        samples: IntegerSamples::Signed {
            values: vec![1, -1, 2, -2, 3, -3],
        },
    });
    assert_eq!(
        bytes(request),
        vec![1, 0, 255, 255, 2, 0, 254, 255, 3, 0, 253, 255]
    );
    let invalid = ContentProviderRequest::Waveform(WaveformContract {
        target: target("waveform", "waveform_multiplex", DicomVr::OW),
        channels: 0,
        samples_per_channel: 3,
        bits_allocated: 16,
        byte_order: ContentByteOrder::LittleEndian,
        samples: IntegerSamples::Signed { values: vec![] },
    });
    assert!(matches!(
        NeutralContentProvider.expand(&invalid, ContentProviderLimits::default()),
        Err(ContentProviderError::ElementCount { .. })
    ));
}

#[test]
fn document_and_mesh_payloads_require_exact_declared_size_and_hash() {
    let document = b"%PDF-1.4\n%%EOF\n".to_vec();
    let request = ContentProviderRequest::EncapsulatedDocument(BytePayloadContract {
        target: target("document", "encapsulated_document", DicomVr::OB),
        media_type: "application/pdf".into(),
        declared_size_bytes: document.len() as u64,
        declared_sha256: sha256_hex(&document),
        bytes: document.clone(),
    });
    assert_eq!(bytes(request), document);
    for invalid in [
        BytePayloadContract {
            target: target("document", "encapsulated_document", DicomVr::OB),
            media_type: "application/pdf".into(),
            declared_size_bytes: document.len() as u64 + 1,
            declared_sha256: sha256_hex(&document),
            bytes: document.clone(),
        },
        BytePayloadContract {
            target: target("document", "encapsulated_document", DicomVr::OB),
            media_type: "application/pdf".into(),
            declared_size_bytes: document.len() as u64,
            declared_sha256: "0".repeat(64),
            bytes: document.clone(),
        },
    ] {
        assert!(
            NeutralContentProvider
                .expand(
                    &ContentProviderRequest::EncapsulatedDocument(invalid),
                    ContentProviderLimits::default(),
                )
                .is_err()
        );
    }

    let mut stl = vec![0_u8; 134];
    stl[80..84].copy_from_slice(&1_u32.to_le_bytes());
    let mesh = ContentProviderRequest::Mesh(MeshContract {
        target: target("mesh", "encapsulated_mesh", DicomVr::OB),
        format: MeshFormat::BinaryStl,
        declared_size_bytes: stl.len() as u64,
        declared_sha256: sha256_hex(&stl),
        triangle_count: Some(1),
        bytes: stl.clone(),
    });
    assert_eq!(bytes(mesh), stl);

    let bad = ContentProviderRequest::Mesh(MeshContract {
        target: target("mesh", "encapsulated_mesh", DicomVr::OB),
        format: MeshFormat::BinaryStl,
        declared_size_bytes: 84,
        declared_sha256: sha256_hex(&vec![0; 84]),
        triangle_count: Some(1),
        bytes: vec![0; 84],
    });
    assert!(matches!(
        NeutralContentProvider.expand(&bad, ContentProviderLimits::default()),
        Err(ContentProviderError::MeshContract)
    ));

    let mut oversized_triangle_declaration = vec![0_u8; 184];
    oversized_triangle_declaration[80..84].copy_from_slice(&2_u32.to_le_bytes());
    let oversized_triangle_declaration = ContentProviderRequest::Mesh(MeshContract {
        target: target("mesh", "encapsulated_mesh", DicomVr::OB),
        format: MeshFormat::BinaryStl,
        declared_size_bytes: 184,
        declared_sha256: sha256_hex(&oversized_triangle_declaration),
        triangle_count: Some(2),
        bytes: oversized_triangle_declaration,
    });
    assert!(matches!(
        NeutralContentProvider.expand(
            &oversized_triangle_declaration,
            ContentProviderLimits {
                max_elements: 1,
                max_output_bytes: 184,
                ..ContentProviderLimits::default()
            }
        ),
        Err(ContentProviderError::ElementCount { .. })
    ));

    let invalid_obj = b"v 0 0 \xff".to_vec();
    assert!(matches!(
        NeutralContentProvider.expand(
            &ContentProviderRequest::Mesh(MeshContract {
                target: target("mesh", "encapsulated_mesh", DicomVr::OB),
                format: MeshFormat::Utf8Obj,
                declared_size_bytes: invalid_obj.len() as u64,
                declared_sha256: sha256_hex(&invalid_obj),
                triangle_count: None,
                bytes: invalid_obj,
            }),
            ContentProviderLimits::default(),
        ),
        Err(ContentProviderError::MeshContract)
    ));
}

fn reference(role: SemanticReferenceRole) -> SemanticReference {
    SemanticReference {
        role,
        sop_class_uid: "1.2.840.10008.5.1.4.1.1.2".into(),
        sop_instance_uid: "2.25.123456789".into(),
        frames: vec![1, 3],
    }
}

#[test]
fn sr_and_rt_semantics_emit_typed_operations_and_reject_bad_references() {
    let sr = ContentProviderRequest::StructuredReport(StructuredReportContract {
        content_date: "20260829".into(),
        content_time: "120000.000000".into(),
        completion_flag: CompletionFlag::Complete,
        verification_flag: VerificationFlag::Unverified,
        concept_name: CodedConcept {
            code_value: "126000".into(),
            coding_scheme_designator: "DCM".into(),
            code_meaning: "Imaging Measurement Report".into(),
        },
        references: vec![reference(SemanticReferenceRole::Evidence)],
    });
    let first = NeutralContentProvider
        .expand(&sr, ContentProviderLimits::default())
        .unwrap();
    let second = NeutralContentProvider
        .expand(&sr, ContentProviderLimits::default())
        .unwrap();
    assert_eq!(first, second);
    assert!(first.contents.is_empty());
    assert!(first.attribute_operations.iter().any(|operation| {
        operation.address().normalized_tag() == "0040,A043"
            && matches!(
                operation,
                AttributeOperation::Set {
                    vr: DicomVr::SQ,
                    ..
                }
            )
    }));

    let rt = ContentProviderRequest::RtObject(RtSemanticContract {
        object_kind: RtObjectKind::Plan,
        label: "DTS_PLAN".into(),
        instance_number: 1,
        references: vec![reference(SemanticReferenceRole::ReferencedStructureSet)],
    });
    let output = NeutralContentProvider
        .expand(&rt, ContentProviderLimits::default())
        .unwrap();
    assert!(
        output
            .attribute_operations
            .iter()
            .any(|operation| { operation.address().normalized_tag() == "300C,0060" })
    );

    let mut invalid = reference(SemanticReferenceRole::Evidence);
    invalid.sop_instance_uid = "not-a-uid".into();
    let invalid = ContentProviderRequest::StructuredReport(StructuredReportContract {
        content_date: "20260829".into(),
        content_time: "120000".into(),
        completion_flag: CompletionFlag::Partial,
        verification_flag: VerificationFlag::Unverified,
        concept_name: CodedConcept {
            code_value: "1".into(),
            coding_scheme_designator: "99DTS".into(),
            code_meaning: "Test".into(),
        },
        references: vec![invalid],
    });
    assert!(matches!(
        NeutralContentProvider.expand(&invalid, ContentProviderLimits::default()),
        Err(ContentProviderError::InvalidUid)
    ));

    assert!(matches!(
        NeutralContentProvider.expand(
            &sr,
            ContentProviderLimits {
                max_text_bytes: 8,
                ..ContentProviderLimits::default()
            }
        ),
        Err(ContentProviderError::TextLimit { .. })
    ));
    assert!(matches!(
        NeutralContentProvider.expand(
            &sr,
            ContentProviderLimits {
                max_references: 1,
                max_attribute_operations: 5,
                ..ContentProviderLimits::default()
            }
        ),
        Err(ContentProviderError::OperationCount { .. })
    ));
}

#[test]
fn serde_is_strict_and_provider_source_has_no_filesystem_or_frontend_dependency() {
    let value = serde_json::json!({
        "kind": "rt_object",
        "object_kind": "plan",
        "label": "DTS",
        "instance_number": 1,
        "references": [],
        "unexpected": true
    });
    assert!(serde_json::from_value::<ContentProviderRequest>(value).is_err());

    let source = std::fs::read_to_string("src/recipes/content_provider.rs").unwrap();
    for forbidden in [
        "std::fs",
        "std::path",
        "crate::generator",
        "crate::cli",
        "case_id",
        "write_to_file",
    ] {
        assert!(
            !source.contains(forbidden),
            "neutral provider must not contain {forbidden}"
        );
    }
}
