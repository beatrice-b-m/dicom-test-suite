use std::collections::BTreeSet;
use std::fs;

use dicom_test_suite::native_pixel::{
    ByteOrder, ChromaSubsampling, ColorOrganization, NativePixelError, NativePixelFactory,
    NativePixelLimits, NativePixelPatternRequest, NativePixelRequest, Palette,
    PhotometricInterpretation, PixelDataVr, PixelPadding, PixelShape, StoredValueType,
};
use dicom_test_suite::recipes::{RecipeCatalog, SecondaryCaptureParameters};
use serde_json::Value;

fn request(sc: &SecondaryCaptureParameters) -> NativePixelRequest {
    NativePixelRequest {
        shape: PixelShape {
            rows: sc.rows,
            columns: sc.columns,
            frames: sc.frames,
            samples_per_pixel: sc.samples_per_pixel,
            photometric_interpretation: match sc.photometric_interpretation.as_str() {
                "MONOCHROME1" => PhotometricInterpretation::Monochrome1,
                "MONOCHROME2" => PhotometricInterpretation::Monochrome2,
                "PALETTE COLOR" => PhotometricInterpretation::PaletteColor,
                "RGB" => PhotometricInterpretation::Rgb,
                "YBR_FULL" => PhotometricInterpretation::YbrFull,
                "YBR_FULL_422" => PhotometricInterpretation::YbrFull422,
                other => panic!("unexpected photometric interpretation {other}"),
            },
            bits_allocated: sc.bits_allocated,
            bits_stored: sc.bits_stored,
            high_bit: sc.high_bit,
            pixel_representation: sc.pixel_representation,
            stored_value_type: match sc.stored_value_type.as_str() {
                "u1" => StoredValueType::U1,
                "u8" => StoredValueType::U8,
                "i8" => StoredValueType::I8,
                "u16" => StoredValueType::U16,
                "i16" => StoredValueType::I16,
                "u32" => StoredValueType::U32,
                "i32" => StoredValueType::I32,
                other => panic!("unexpected stored value type {other}"),
            },
            // Native frames are canonical little-endian inputs. Target transfer
            // syntax byte order belongs to the later encoding plan.
            byte_order: ByteOrder::Little,
            pixel_data_vr: match sc.pixel_data_vr.as_str() {
                "OB" => PixelDataVr::Ob,
                "OW" => PixelDataVr::Ow,
                other => panic!("unexpected Pixel Data VR {other}"),
            },
            color: sc.color.as_ref().map(|color| ColorOrganization {
                planar_configuration: color.planar_configuration.unwrap(),
                chroma_subsampling: match color.chroma_subsampling.as_str() {
                    "none" => ChromaSubsampling::None,
                    "horizontal_2_to_1" => ChromaSubsampling::Horizontal2To1,
                    other => panic!("unexpected chroma subsampling {other}"),
                },
            }),
        },
        stored_values: sc.stored_values.clone(),
        declared_pixel_min: sc.pixel_min,
        declared_pixel_max: sc.pixel_max,
        expected_frame_sha256: sc.frame_sha256.clone(),
        padding: sc.padding.as_ref().map(|padding| PixelPadding {
            value: padding.value,
            range_limit: padding.range_limit,
        }),
        palette: sc.palette.as_ref().map(|palette| Palette {
            descriptor: palette.descriptor,
            red: palette.red.clone(),
            green: palette.green.clone(),
            blue: palette.blue.clone(),
        }),
    }
}

fn mono_request(stored_value_type: StoredValueType, values: Vec<i64>) -> NativePixelRequest {
    let bits = stored_value_type.bits_allocated();
    let minimum = *values.iter().min().unwrap();
    let maximum = *values.iter().max().unwrap();
    NativePixelRequest {
        shape: PixelShape {
            rows: 1,
            columns: values.len() as u32,
            frames: 1,
            samples_per_pixel: 1,
            photometric_interpretation: PhotometricInterpretation::Monochrome2,
            bits_allocated: bits,
            bits_stored: bits,
            high_bit: bits - 1,
            pixel_representation: stored_value_type.pixel_representation(),
            stored_value_type,
            byte_order: ByteOrder::Little,
            pixel_data_vr: if bits <= 8 {
                PixelDataVr::Ob
            } else {
                PixelDataVr::Ow
            },
            color: None,
        },
        stored_values: values,
        declared_pixel_min: minimum,
        declared_pixel_max: maximum,
        expected_frame_sha256: Vec::new(),
        padding: None,
        palette: None,
    }
}

#[test]
fn every_typed_secondary_capture_recipe_reproduces_its_golden_frames() {
    let catalog = RecipeCatalog::load(
        "cases/recipes",
        "cases/registry.json",
        "templates/catalog.json",
    )
    .unwrap();
    let factory = NativePixelFactory;
    let registry: Value =
        serde_json::from_slice(&fs::read("cases/registry.json").unwrap()).unwrap();
    let expected_cases = registry["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|case| {
            let case_id = case["case_id"].as_str().unwrap();
            case["status"] == "implemented"
                && case["provider"]["kind"] == "rust_native"
                && case["provider"]["id"] == "rust_native"
                && case["requirements"]["features"]
                    .as_array()
                    .unwrap()
                    .is_empty()
                && case["requirements"]["external_codecs"]
                    .as_array()
                    .unwrap()
                    .is_empty()
                && (case_id.starts_with("classic/sc/")
                    || case_id == "encapsulation/sc/eot_single_fragment_multiframe")
        })
        .map(|case| case["case_id"].as_str().unwrap().to_string())
        .collect::<BTreeSet<_>>();
    let mut actual_cases = BTreeSet::new();
    for recipe in catalog.recipes().values() {
        if recipe.plan_provider_id != "native.sc_plan" {
            continue;
        }
        actual_cases.insert(recipe.binding.case_id.clone());
        for artifact in &recipe.dicom.as_ref().unwrap().artifacts {
            let sc = artifact.secondary_capture.as_ref().unwrap();
            let output = factory.create(request(sc)).unwrap_or_else(|error| {
                panic!(
                    "{} failed neutral pixel construction: {error}",
                    recipe.recipe_id
                )
            });
            assert_eq!(
                output
                    .frames
                    .iter()
                    .map(|frame| frame.decoded_sha256.as_str())
                    .collect::<Vec<_>>(),
                sc.frame_sha256
                    .iter()
                    .map(String::as_str)
                    .collect::<Vec<_>>(),
                "{}",
                recipe.recipe_id
            );
            assert_eq!(output.pixel_min, sc.pixel_min);
            assert_eq!(output.pixel_max, sc.pixel_max);
        }
    }
    assert!(
        !actual_cases.is_empty(),
        "typed SC recipe inventory is empty"
    );
    assert_eq!(actual_cases, expected_cases);
}

#[test]
fn serializes_all_little_endian_signed_and_unsigned_widths() {
    let cases = [
        (StoredValueType::U8, vec![0, 255], vec![0x00, 0xff]),
        (StoredValueType::I8, vec![-128, 127], vec![0x80, 0x7f]),
        (
            StoredValueType::U16,
            vec![0, 0x55aa, 0xffff],
            vec![0x00, 0x00, 0xaa, 0x55, 0xff, 0xff],
        ),
        (
            StoredValueType::I16,
            vec![-32768, -1, 32767],
            vec![0x00, 0x80, 0xff, 0xff, 0xff, 0x7f],
        ),
        (
            StoredValueType::U32,
            vec![0, 65_535, 2_147_483_648, 4_294_967_295],
            vec![
                0, 0, 0, 0, 0xff, 0xff, 0, 0, 0, 0, 0, 0x80, 0xff, 0xff, 0xff, 0xff,
            ],
        ),
        (
            StoredValueType::I32,
            vec![i32::MIN as i64, -1, i32::MAX as i64],
            vec![
                0, 0, 0, 0x80, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x7f,
            ],
        ),
    ];
    for (stored_type, values, expected) in cases {
        let output = NativePixelFactory
            .create(mono_request(stored_type, values))
            .unwrap();
        assert_eq!(output.unpadded_bytes, expected, "{stored_type:?}");
    }
}

#[test]
fn clears_unused_high_bits_for_signed_stored_values() {
    let mut request = mono_request(StoredValueType::I16, vec![-1024, -1, 0, 2047]);
    request.shape.bits_stored = 12;
    request.shape.high_bit = 11;
    let output = NativePixelFactory.create(request).unwrap();

    assert_eq!(
        output.unpadded_bytes,
        [0x00, 0x0c, 0xff, 0x0f, 0x00, 0x00, 0xff, 0x07]
    );
}

#[test]
fn packs_u1_lsb_first_continuously_and_separates_value_padding() {
    let mut request = mono_request(
        StoredValueType::U1,
        vec![1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0],
    );
    request.shape.rows = 3;
    request.shape.columns = 3;
    request.shape.frames = 2;
    request.expected_frame_sha256 = vec![
        "a6188710c09cfbc77383ee0588dec2f7affa6e03e78aa900e9ae597a8d8faba3".into(),
        "c520efb8f894a1125bb1a513a9b64ef957f7c2cd63835fd7e130357c47f989ae".into(),
    ];
    let output = NativePixelFactory.create(request).unwrap();
    assert_eq!(output.unpadded_bytes, [0x55, 0x55, 0x01]);
    assert_eq!(output.padded_bytes, [0x55, 0x55, 0x01, 0x00]);
    assert_eq!(output.plan.padding_bytes, 1);
    assert_eq!(
        output.padded_sha256,
        "9d6baf87a79d40ef2b145f92945a05cf156a2741e2c2834a3a7721d52757594b"
    );
    assert_eq!(output.plan.frame_spans[1].bit_offset, 9);
}

#[test]
fn preserves_planar_color_ybr422_and_palette_bytes() {
    let factory = NativePixelFactory;
    for (photometric, planar, values, expected) in [
        (
            PhotometricInterpretation::Rgb,
            0,
            vec![255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255],
            vec![255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255],
        ),
        (
            PhotometricInterpretation::Rgb,
            1,
            vec![255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255],
            vec![255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255],
        ),
        (
            PhotometricInterpretation::YbrFull,
            0,
            vec![76, 85, 255, 150, 44, 21, 29, 255, 107, 255, 128, 128],
            vec![76, 85, 255, 150, 44, 21, 29, 255, 107, 255, 128, 128],
        ),
        (
            PhotometricInterpretation::YbrFull,
            1,
            vec![76, 150, 29, 255, 85, 44, 255, 128, 255, 21, 107, 128],
            vec![76, 150, 29, 255, 85, 44, 255, 128, 255, 21, 107, 128],
        ),
    ] {
        let request = NativePixelRequest {
            shape: PixelShape {
                rows: 2,
                columns: 2,
                frames: 1,
                samples_per_pixel: 3,
                photometric_interpretation: photometric,
                bits_allocated: 8,
                bits_stored: 8,
                high_bit: 7,
                pixel_representation: 0,
                stored_value_type: StoredValueType::U8,
                byte_order: ByteOrder::Little,
                pixel_data_vr: PixelDataVr::Ob,
                color: Some(ColorOrganization {
                    planar_configuration: planar,
                    chroma_subsampling: ChromaSubsampling::None,
                }),
            },
            declared_pixel_min: *values.iter().min().unwrap(),
            declared_pixel_max: *values.iter().max().unwrap(),
            stored_values: values,
            expected_frame_sha256: Vec::new(),
            padding: None,
            palette: None,
        };
        assert_eq!(factory.create(request).unwrap().unpadded_bytes, expected);
    }

    let ybr422 = NativePixelRequest {
        shape: PixelShape {
            rows: 2,
            columns: 2,
            frames: 1,
            samples_per_pixel: 3,
            photometric_interpretation: PhotometricInterpretation::YbrFull422,
            bits_allocated: 8,
            bits_stored: 8,
            high_bit: 7,
            pixel_representation: 0,
            stored_value_type: StoredValueType::U8,
            byte_order: ByteOrder::Little,
            pixel_data_vr: PixelDataVr::Ob,
            color: Some(ColorOrganization {
                planar_configuration: 0,
                chroma_subsampling: ChromaSubsampling::Horizontal2To1,
            }),
        },
        stored_values: vec![76, 150, 65, 138, 29, 255, 192, 118],
        declared_pixel_min: 29,
        declared_pixel_max: 255,
        expected_frame_sha256: Vec::new(),
        padding: None,
        palette: None,
    };
    assert_eq!(
        factory.create(ybr422).unwrap().unpadded_bytes,
        [76, 150, 65, 138, 29, 255, 192, 118]
    );

    let mut palette = mono_request(StoredValueType::U8, vec![0, 1, 2, 3]);
    palette.shape.rows = 2;
    palette.shape.columns = 2;
    palette.shape.photometric_interpretation = PhotometricInterpretation::PaletteColor;
    palette.palette = Some(Palette {
        descriptor: [4, 0, 16],
        red: vec![65535, 0, 0, 65535],
        green: vec![0, 65535, 0, 65535],
        blue: vec![0, 0, 65535, 65535],
    });
    let output = factory.create(palette).unwrap();
    assert_eq!(output.unpadded_bytes, [0, 1, 2, 3]);
    assert_eq!(output.palette.unwrap().descriptor, [4, 0, 16]);

    let mut invalid_index = mono_request(StoredValueType::U8, vec![0, 1, 2, 4]);
    invalid_index.shape.rows = 2;
    invalid_index.shape.columns = 2;
    invalid_index.shape.photometric_interpretation = PhotometricInterpretation::PaletteColor;
    invalid_index.palette = Some(Palette {
        descriptor: [4, 0, 16],
        red: vec![65535, 0, 0, 65535],
        green: vec![0, 65535, 0, 65535],
        blue: vec![0, 0, 65535, 65535],
    });
    assert!(matches!(
        factory.create(invalid_index),
        Err(NativePixelError::PaletteIndexOutOfRange {
            index: 3,
            value: 4,
            ..
        })
    ));
}

#[test]
fn reproduces_both_composition_default_patterns() {
    let mono = NativePixelFactory
        .create_pattern(NativePixelPatternRequest::MonochromeHorizontalRamp {
            rows: 64,
            columns: 64,
            frames: 1,
            column_step: 4,
        })
        .unwrap();
    assert_eq!(mono.unpadded_bytes.len(), 4096);
    assert_eq!(
        mono.unpadded_sha256,
        "fc79e707a60d7602732e7b610a0191cf3eb205264589af81571471727db68099"
    );

    let rgb = NativePixelFactory
        .create_pattern(NativePixelPatternRequest::RgbCoordinates {
            rows: 32,
            columns: 32,
            frames: 1,
        })
        .unwrap();
    assert_eq!(rgb.unpadded_bytes.len(), 3072);
    assert_eq!(
        rgb.unpadded_sha256,
        "56699dcfac1f1f988529c223f70bb5bad5c1879dc0ed4842ceecb82817cf0e02"
    );
}

#[test]
fn rejects_overflow_ranges_hash_drift_and_invalid_organizations() {
    let overflow = PixelShape {
        rows: u32::MAX,
        columns: u32::MAX,
        frames: u32::MAX,
        samples_per_pixel: 3,
        photometric_interpretation: PhotometricInterpretation::Rgb,
        bits_allocated: 32,
        bits_stored: 32,
        high_bit: 31,
        pixel_representation: 0,
        stored_value_type: StoredValueType::U32,
        byte_order: ByteOrder::Little,
        pixel_data_vr: PixelDataVr::Ow,
        color: Some(ColorOrganization {
            planar_configuration: 0,
            chroma_subsampling: ChromaSubsampling::None,
        }),
    };
    assert_eq!(
        dicom_test_suite::native_pixel::NativePixelPlan::plan(overflow),
        Err(NativePixelError::ArithmeticOverflow)
    );

    let range_error = NativePixelFactory
        .create(mono_request(StoredValueType::U8, vec![0, 256]))
        .unwrap_err();
    assert!(matches!(
        range_error,
        NativePixelError::StoredValueOutOfRange { index: 1, .. }
    ));

    let mut hash_drift = mono_request(StoredValueType::U8, vec![0, 255]);
    hash_drift.expected_frame_sha256 = vec!["0".repeat(64)];
    assert!(matches!(
        NativePixelFactory.create(hash_drift),
        Err(NativePixelError::FrameHashMismatch {
            frame_number: 1,
            ..
        })
    ));

    let mut invalid_422 = mono_request(StoredValueType::U8, vec![0, 0, 0, 0, 0, 0]);
    invalid_422.shape.rows = 1;
    invalid_422.shape.columns = 3;
    invalid_422.shape.samples_per_pixel = 3;
    invalid_422.shape.photometric_interpretation = PhotometricInterpretation::YbrFull422;
    invalid_422.shape.color = Some(ColorOrganization {
        planar_configuration: 0,
        chroma_subsampling: ChromaSubsampling::Horizontal2To1,
    });
    assert_eq!(
        NativePixelFactory.create(invalid_422),
        Err(NativePixelError::InvalidYbrFull422)
    );

    let mut bad_padding = mono_request(StoredValueType::I16, vec![-1, 1]);
    bad_padding.padding = Some(PixelPadding {
        value: 40_000,
        range_limit: None,
    });
    assert_eq!(
        NativePixelFactory.create(bad_padding),
        Err(NativePixelError::PaddingOutOfRange(40_000))
    );
}

#[test]
fn rejects_adversarial_sizes_before_allocation_under_typed_limits() {
    let default_bounded = PixelShape {
        rows: 1,
        columns: 1,
        frames: u32::MAX,
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
    };
    assert!(matches!(
        dicom_test_suite::native_pixel::NativePixelPlan::plan(default_bounded),
        Err(NativePixelError::ResourceLimitExceeded {
            resource: "frames",
            requested,
            ..
        }) if requested == u64::from(u32::MAX)
    ));

    let many_frames = PixelShape {
        rows: 1,
        columns: 1,
        frames: 11,
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
    };
    let limits = NativePixelLimits {
        max_frames: 10,
        max_stored_values: 100,
        max_value_bytes: 100,
    };
    assert_eq!(
        dicom_test_suite::native_pixel::NativePixelPlan::plan_with_limits(many_frames, limits),
        Err(NativePixelError::ResourceLimitExceeded {
            resource: "frames",
            limit: 10,
            requested: 11,
        })
    );

    let too_many_values = PixelShape {
        rows: 11,
        columns: 10,
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
    };
    assert!(matches!(
        dicom_test_suite::native_pixel::NativePixelPlan::plan_with_limits(too_many_values, limits),
        Err(NativePixelError::ResourceLimitExceeded {
            resource: "stored_values",
            requested: 110,
            ..
        })
    ));

    assert!(matches!(
        NativePixelFactory.create_pattern_with_limits(
            NativePixelPatternRequest::MonochromeHorizontalRamp {
                rows: 11,
                columns: 10,
                frames: 1,
                column_step: 1,
            },
            limits,
        ),
        Err(NativePixelError::ResourceLimitExceeded {
            resource: "stored_values",
            ..
        })
    ));

    let too_many_bytes = PixelShape {
        rows: 3,
        columns: 2,
        frames: 1,
        samples_per_pixel: 1,
        photometric_interpretation: PhotometricInterpretation::Monochrome2,
        bits_allocated: 16,
        bits_stored: 16,
        high_bit: 15,
        pixel_representation: 0,
        stored_value_type: StoredValueType::U16,
        byte_order: ByteOrder::Little,
        pixel_data_vr: PixelDataVr::Ow,
        color: None,
    };
    let byte_limits = NativePixelLimits {
        max_frames: 10,
        max_stored_values: 100,
        max_value_bytes: 10,
    };
    assert_eq!(
        dicom_test_suite::native_pixel::NativePixelPlan::plan_with_limits(
            too_many_bytes,
            byte_limits,
        ),
        Err(NativePixelError::ResourceLimitExceeded {
            resource: "value_bytes",
            limit: 10,
            requested: 12,
        })
    );
}

#[test]
fn neutral_module_has_no_frontend_writer_or_filesystem_dependencies() {
    let source = fs::read_to_string(format!(
        "{}/src/native_pixel.rs",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap();
    for forbidden in [
        "crate::composition",
        "crate::generator",
        "crate::recipes",
        "crate::executor",
        "std::fs",
        "std::path",
        "Part10Materializer",
        "OutputTransaction",
    ] {
        assert!(
            !source.contains(forbidden),
            "neutral native pixel module contains forbidden dependency {forbidden}"
        );
    }
}
