use std::fs;

use dicom_test_suite::composition::{CompositionUidRole, Part10Materializer};
use dicom_test_suite::corpus_plan::OutputRelativePath;
use dicom_test_suite::recipes::{
    AdvancedPlanProviderRequest, AdvancedProviderFamily, AdvancedProviderLimits,
    EnhancedCommonInput, EnhancedCtInput, EnhancedCtPartInput, EnhancedFrameGeometry,
    EnhancedMrFrameAxis, EnhancedMrInput, EnhancedNativePixels, EnhancedPetInput,
    EnhancedPlanProvider, EnhancedProviderInput, RecipeIdentity,
};
use dicom_test_suite::sha256_hex;

const LOCK: &str = "823230c5932b81b504434330d118fba286d5ff41d4e2f7766372633f4a49e559";

fn common(
    case_id: &str,
    recipe_id: &str,
    template_id: &str,
    modality: &str,
    study_id: &str,
    serial: &str,
    image_type: &str,
    rows: u16,
    columns: u16,
) -> EnhancedCommonInput {
    EnhancedCommonInput {
        case_id: case_id.into(),
        recipe_id: recipe_id.into(),
        recipe_version: "0.1.0".into(),
        template_id: template_id.into(),
        modality: modality.into(),
        study_id: study_id.into(),
        device_serial_number: serial.into(),
        image_type: image_type.into(),
        rows,
        columns,
        frame_type: image_type.into(),
        pixel_presentation: "MONOCHROME".into(),
        volumetric_properties: "VOLUME".into(),
        volume_based_calculation_technique: "NONE".into(),
    }
}

fn request(input: &EnhancedProviderInput) -> AdvancedPlanProviderRequest {
    let common = match input {
        EnhancedProviderInput::Ct(value) => &value.common,
        EnhancedProviderInput::Mr(value) => &value.common,
        EnhancedProviderInput::Pet(value) => &value.common,
    };
    AdvancedPlanProviderRequest {
        provider_id: "native.enhanced_plan".into(),
        family: AdvancedProviderFamily::Enhanced,
        case_id: common.case_id.clone(),
        recipe: RecipeIdentity {
            recipe_id: common.recipe_id.clone(),
            recipe_version: common.recipe_version.clone(),
        },
        seed: 1,
        limits: AdvancedProviderLimits {
            max_artifacts: 4,
            max_references: 8,
            max_binding_slots: 8,
            max_total_output_bytes: 16 * 1024 * 1024,
            max_peak_working_bytes: 32 * 1024 * 1024,
            max_parallelism: 2,
        },
    }
}

fn pixels(values: &[i64]) -> EnhancedNativePixels {
    EnhancedNativePixels {
        stored_values: values.to_vec(),
        pixel_min: *values.iter().min().unwrap(),
        pixel_max: *values.iter().max().unwrap(),
    }
}

fn geometry(positions: &[&str], dimensions: &[u32]) -> Vec<EnhancedFrameGeometry> {
    positions
        .iter()
        .zip(dimensions)
        .map(|(position, dimension)| EnhancedFrameGeometry {
            image_position_patient: (*position).into(),
            dimension_index_value: *dimension,
        })
        .collect()
}

fn ct() -> EnhancedProviderInput {
    EnhancedProviderInput::Ct(EnhancedCtInput {
        common: common(
            "enhanced/ct/multiframe_shared_perframe_explicit_le",
            "enhanced_ct_multiframe_shared_perframe",
            "enhanced/ct",
            "CT",
            "DTS-ECT",
            "DTS-ECT-0001",
            "DERIVED\\PRIMARY\\AXIAL\\NONE",
            2,
            2,
        ),
        pixel_spacing: "0.75\\0.75".into(),
        image_orientation_patient: "1\\0\\0\\0\\1\\0".into(),
        slice_thickness: "2.5".into(),
        spacing_between_slices: "2.5".into(),
        rescale_intercept: "-1024".into(),
        rescale_slope: "1".into(),
        rescale_type: "HU".into(),
        parts: vec![EnhancedCtPartInput {
            template_id: "enhanced/ct".into(),
            output_path: OutputRelativePath::new(
                "enhanced/ct/multiframe_shared_perframe_explicit_le/instance.dcm",
            )
            .unwrap(),
            frames: geometry(&["0\\0\\0", "0\\0\\2.5"], &[1, 2]),
            pixels: pixels(&[0, 100, 200, 300, 400, 500, 600, 700]),
            in_concatenation_number: None,
            concatenation_frame_offset_number: None,
        }],
        concatenation: false,
        stress: false,
    })
}

fn concatenation() -> EnhancedProviderInput {
    let mut value = match ct() {
        EnhancedProviderInput::Ct(value) => value,
        _ => unreachable!(),
    };
    value.common.case_id = "enhanced/ct/concatenation_two_part_explicit_le".into();
    value.common.recipe_id = "enhanced_ct_concatenation_two_part".into();
    value.common.template_id = "enhanced/ct/concatenation-part-1".into();
    value.parts = vec![
        EnhancedCtPartInput {
            template_id: "enhanced/ct/concatenation-part-1".into(),
            output_path: OutputRelativePath::new(
                "enhanced/ct/concatenation_two_part_explicit_le/part-001.dcm",
            )
            .unwrap(),
            frames: geometry(&["0\\0\\0"], &[1]),
            pixels: pixels(&[0, 100, 200, 300]),
            in_concatenation_number: Some(1),
            concatenation_frame_offset_number: Some(0),
        },
        EnhancedCtPartInput {
            template_id: "enhanced/ct/concatenation-part-2".into(),
            output_path: OutputRelativePath::new(
                "enhanced/ct/concatenation_two_part_explicit_le/part-002.dcm",
            )
            .unwrap(),
            frames: geometry(&["0\\0\\2.5"], &[2]),
            pixels: pixels(&[400, 500, 600, 700]),
            in_concatenation_number: Some(2),
            concatenation_frame_offset_number: Some(1),
        },
    ];
    value.concatenation = true;
    EnhancedProviderInput::Ct(value)
}

fn mr(recipe_id: &str) -> EnhancedProviderInput {
    let (case_id, image_type, values, positions, axis, repetition_time) = match recipe_id {
        "enhanced_mr_multiframe_echo_perframe" => (
            "enhanced/mr/multiframe_echo_perframe_explicit_le",
            "DERIVED\\PRIMARY\\STATIC\\NONE",
            &[0, 50, 100, 150, 200, 250, 300, 350][..],
            &["0\\0\\0", "0\\0\\4"][..],
            EnhancedMrFrameAxis::EffectiveEchoTime {
                values: vec![12.5, 24.5],
            },
            "2000",
        ),
        "enhanced_mr_multiframe_temporal_position" => (
            "enhanced/mr/multiframe_temporal_position_explicit_le",
            "DERIVED\\PRIMARY\\DYNAMIC\\NONE",
            &[0, 25, 50, 75, 150, 175, 200, 225][..],
            &["0\\0\\0", "0\\0\\0"][..],
            EnhancedMrFrameAxis::TemporalPositionTimeOffset {
                values: vec![0.0, 1.5],
            },
            "1500",
        ),
        "enhanced_mr_multiframe_phase_velocity_encoding" => (
            "enhanced/mr/multiframe_phase_velocity_encoding_explicit_le",
            "DERIVED\\PRIMARY\\DYNAMIC\\NONE",
            &[0, 40, 80, 120, 160, 200, 240, 280][..],
            &["0\\0\\0", "0\\0\\0"][..],
            EnhancedMrFrameAxis::VelocityEncoding {
                directions: vec![[1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
                minimum: -150.0,
                maximum: 150.0,
            },
            "1500",
        ),
        _ => panic!("unknown fixture"),
    };
    EnhancedProviderInput::Mr(EnhancedMrInput {
        common: common(
            case_id,
            recipe_id,
            "enhanced/mr",
            "MR",
            "DTS-EMR",
            "DTS-EMR-0001",
            image_type,
            2,
            2,
        ),
        output_path: OutputRelativePath::new(format!("{case_id}/instance.dcm")).unwrap(),
        frames: geometry(positions, &[1, 2]),
        pixels: pixels(values),
        pixel_spacing: "1.000\\1.000".into(),
        image_orientation_patient: "1\\0\\0\\0\\1\\0".into(),
        slice_thickness: "4".into(),
        spacing_between_slices: "4".into(),
        rescale_intercept: "0".into(),
        rescale_slope: "1".into(),
        rescale_type: "US".into(),
        repetition_time: repetition_time.into(),
        flip_angle: "90".into(),
        echo_train_length: "1".into(),
        rf_echo_train_length: 1,
        gradient_echo_train_length: 0,
        axis,
    })
}

fn pet() -> EnhancedProviderInput {
    EnhancedProviderInput::Pet(EnhancedPetInput {
        common: common(
            "enhanced/pet/multiframe_explicit_le",
            "enhanced_pet_multiframe_explicit_le",
            "enhanced/pet",
            "PT",
            "DTS-EPET",
            "DTS-EPET-0001",
            "DERIVED\\PRIMARY\\STATIC\\MULTIPLICATION",
            2,
            2,
        ),
        output_path: OutputRelativePath::new("enhanced/pet/multiframe_explicit_le/instance.dcm")
            .unwrap(),
        frames: geometry(&["0\\0\\0", "0\\0\\5"], &[1, 2]),
        temporal_position_indices: vec![1, 1],
        in_stack_position_numbers: vec![1, 2],
        stack_id: "1".into(),
        pixels: pixels(&[0, 100, 200, 400, 0, 100, 200, 400]),
        pixel_spacing: "2\\2".into(),
        image_orientation_patient: "1\\0\\0\\0\\1\\0".into(),
        slice_thickness: "5".into(),
        spacing_between_slices: "5".into(),
        rescale_intercept: "0".into(),
        rescale_slope: "2.5".into(),
        units: "BQML".into(),
        counts_source: "EMISSION".into(),
    })
}

fn stress_ct() -> EnhancedProviderInput {
    const ROWS: u16 = 64;
    const COLUMNS: u16 = 64;
    const FRAMES: u32 = 256;
    let sample_count = usize::from(ROWS) * usize::from(COLUMNS) * FRAMES as usize;
    EnhancedProviderInput::Ct(EnhancedCtInput {
        common: common(
            "stress/enhanced-ct/many_frames",
            "stress_enhanced_ct_many_frames",
            "enhanced/ct",
            "CT",
            "DTS-ECT",
            "DTS-ECT-0001",
            "DERIVED\\PRIMARY\\VOLUME\\NONE",
            ROWS,
            COLUMNS,
        ),
        pixel_spacing: "0.75\\0.75".into(),
        image_orientation_patient: "1\\0\\0\\0\\1\\0".into(),
        slice_thickness: "2.5".into(),
        spacing_between_slices: "2.5".into(),
        rescale_intercept: "-1024".into(),
        rescale_slope: "1".into(),
        rescale_type: "HU".into(),
        parts: vec![EnhancedCtPartInput {
            template_id: "enhanced/ct".into(),
            output_path: OutputRelativePath::new("stress/enhanced-ct/many_frames/instance.dcm")
                .unwrap(),
            frames: (0..FRAMES)
                .map(|frame| EnhancedFrameGeometry {
                    image_position_patient: format!("0\\0\\{}", f64::from(frame) * 2.5),
                    dimension_index_value: frame + 1,
                })
                .collect(),
            pixels: EnhancedNativePixels {
                stored_values: (0..sample_count)
                    .map(|index| (index % 4096) as i64)
                    .collect(),
                pixel_min: 0,
                pixel_max: 4095,
            },
            in_concatenation_number: None,
            concatenation_frame_offset_number: None,
        }],
        concatenation: false,
        stress: true,
    })
}

#[test]
fn direct_enhanced_plans_match_frozen_seed_one_part10_bytes() {
    let fixtures = [
        (
            ct(),
            vec!["7ad8de623f589ac6f63f27631dadc9e7ab3d01e05bea1fd89a872ea08c9ef919"],
        ),
        (
            concatenation(),
            vec![
                "80080befc5ae4e8ea6c11e889c08ac391ec46fe7b55aac14f8ff11c854f73d50",
                "4d717c7b1b476d9544ba6886ed3a7537689aa503883e5294a0ea8d2146b167c9",
            ],
        ),
        (
            mr("enhanced_mr_multiframe_echo_perframe"),
            vec!["ae42d05cfba40706f6fe6856192b104238e82777c16c5520b780f32bf657264a"],
        ),
        (
            mr("enhanced_mr_multiframe_temporal_position"),
            vec!["7c87eb000fa46b1b772023f2f4c27d5351d24dfa7b967e71718cdd241b64a9a1"],
        ),
        (
            mr("enhanced_mr_multiframe_phase_velocity_encoding"),
            vec!["00bbee122bdfdaa844fad5a8919a1c984ffe3e9c4a41eecebd4e84f302386fd8"],
        ),
        (
            pet(),
            vec!["f40d03339b2344d0f415c3be9ed5194b3657dcf68a06680f131f1dfe0607125f"],
        ),
    ];
    let provider = EnhancedPlanProvider::new(LOCK).unwrap();
    let root =
        std::env::temp_dir().join(format!("dts-enhanced-plan-provider-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    for (fixture_index, (input, expected)) in fixtures.into_iter().enumerate() {
        let output = provider.plan_typed(&request(&input), &input).unwrap();
        assert_eq!(output.artifacts.len(), expected.len());
        for (artifact, expected_hash) in output.artifacts.iter().zip(expected) {
            let path = root.join(format!("{fixture_index}-{}.dcm", artifact.planned.order));
            Part10Materializer
                .materialize(&artifact.planned.instance, &path)
                .unwrap();
            assert_eq!(sha256_hex(&fs::read(path).unwrap()), expected_hash);
        }
    }
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn planning_is_output_free_ordered_and_identity_stable() {
    let provider = EnhancedPlanProvider::new(LOCK).unwrap();
    let input = concatenation();
    let first = provider.plan_typed(&request(&input), &input).unwrap();
    let second = provider.plan_typed(&request(&input), &input).unwrap();
    assert_eq!(first, second);
    assert_eq!(first.artifacts.len(), 2);
    assert!(first.artifacts[0].planned.order < first.artifacts[1].planned.order);
    assert_ne!(
        first.artifacts[0]
            .planned
            .instance
            .identities
            .get(&CompositionUidRole::SopInstance, 0),
        first.artifacts[1]
            .planned
            .instance
            .identities
            .get(&CompositionUidRole::SopInstance, 0)
    );
}

#[test]
fn reduced_many_frame_stress_plan_is_bounded_and_complete() {
    let provider = EnhancedPlanProvider::new(LOCK).unwrap();
    let input = stress_ct();
    let output = provider.plan_typed(&request(&input), &input).unwrap();
    let planned = &output.artifacts[0].planned;
    assert_eq!(planned.instance.content[0].size_bytes, 64 * 64 * 256 * 2);
    assert_eq!(output.bindings[0].slots.len(), 1);
    assert_eq!(
        planned
            .instance
            .attributes
            .iter()
            .find(|attribute| attribute.address.normalized_tag() == "5200,9230")
            .and_then(|attribute| attribute.value.as_ref())
            .and_then(|value| match value {
                dicom_test_suite::composition::AttributeValue::Sequence(items) => Some(items.len()),
                _ => None,
            }),
        Some(256)
    );
    assert!(planned.resources.peak_working_bytes <= request(&input).limits.max_peak_working_bytes);
}

#[test]
fn rejects_corrupt_dimensions_cardinality_and_catalog_ownership() {
    let provider = EnhancedPlanProvider::new(LOCK).unwrap();
    let mut input = ct();
    let EnhancedProviderInput::Ct(ct_input) = &mut input else {
        unreachable!()
    };
    ct_input.parts[0].frames[1].dimension_index_value = 1;
    assert!(provider.plan_typed(&request(&input), &input).is_err());

    let mut input = pet();
    let EnhancedProviderInput::Pet(pet) = &mut input else {
        unreachable!()
    };
    pet.temporal_position_indices.pop();
    assert!(provider.plan_typed(&request(&input), &input).is_err());

    let input = ct();
    let mut mismatched = request(&input);
    mismatched.recipe.recipe_id = "not_owned".into();
    assert!(provider.plan_typed(&mismatched, &input).is_err());
}

#[test]
fn provider_source_has_no_legacy_or_filesystem_bridge() {
    let source = include_str!("../src/recipes/enhanced.rs");
    for forbidden in [
        "std::fs",
        "std::path",
        "generator::",
        "InMemDicomObject",
        "resolved_plan_from_curated_dataset",
        "Part10Materializer",
        "output_root",
    ] {
        assert!(
            !source.contains(forbidden),
            "forbidden source boundary: {forbidden}"
        );
    }
}
