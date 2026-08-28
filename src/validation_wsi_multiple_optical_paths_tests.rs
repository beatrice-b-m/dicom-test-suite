use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

use dicom_core::{DataElement, PrimitiveValue, VR, value::DataSetSequence};
use dicom_dictionary_std::{tags, uids};
use dicom_object::{FileMetaTableBuilder, InMemDicomObject};

use super::{
    Part10Expectations, PixelDataLengthFormula, validate_wsi_multiple_optical_paths_file,
    wsi_tiled_full_tests,
};
use crate::wsi_multiple_optical_paths_locked_contract;

const SOP_UID: &str = "2.25.8801";
const FOR_UID: &str = "2.25.8802";
const SPECIMEN_UID: &str = "2.25.8803";
const DIMENSION_UID: &str = "2.25.8804";
const IMPLEMENTATION_UID: &str = "2.25.8899";
const PIXELS: [u8; 96] = multiple_path_pixels();
static FIXTURE_ORDINAL: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy)]
enum Mutation {
    None,
    DuplicateIdentifier,
    ReorderedPaths,
    WrongWavelength,
    SwappedPathBlocks,
    SwappedFramesWithinPath,
    WrongPathCount,
    AddedPerFrameGroups,
    AddedTopLevelIcc,
}

#[test]
fn accepts_exact_multiple_optical_path_wsi_contract() {
    let path = write_fixture("valid", Mutation::None);
    let validated = validate_wsi_multiple_optical_paths_file(&path, &identity(), &contract())
        .expect("exact multiple-optical-path WSI fixture must validate");
    assert_eq!(validated.validation["status"], "passed");
    for finding in [
        "wsi_multiple_paths_optical_path_1_identifier",
        "wsi_multiple_paths_optical_path_2_identifier",
        "wsi_multiple_paths_nested_icc_identical",
        "wsi_multiple_paths_path_1_matrix_sha256",
        "wsi_multiple_paths_path_2_matrix_sha256",
        "wsi_multiple_paths_implicit_path_outermost_order",
        "wsi_multiple_paths_instance_budget",
    ] {
        assert!(
            validated.validation["internal"]
                .as_array()
                .unwrap()
                .iter()
                .any(|item| item["name"] == finding && item["status"] == "passed"),
            "missing passing finding {finding}"
        );
    }
    cleanup(path);
}

#[test]
fn rejects_ordered_path_identity_cardinality_and_absence_mutations() {
    for (label, mutation, finding) in [
        (
            "duplicate-identifier",
            Mutation::DuplicateIdentifier,
            "wsi_multiple_paths_optical_path_2_identifier",
        ),
        (
            "reordered-paths",
            Mutation::ReorderedPaths,
            "wsi_multiple_paths_optical_path_1_identifier",
        ),
        (
            "wrong-wavelength",
            Mutation::WrongWavelength,
            "wsi_multiple_paths_optical_path_2_wavelength",
        ),
        (
            "wrong-path-count",
            Mutation::WrongPathCount,
            "wsi_multiple_paths_number_of_optical_paths",
        ),
        (
            "per-frame",
            Mutation::AddedPerFrameGroups,
            "wsi_multiple_paths_per_frame_functional_groups_absent",
        ),
        (
            "top-level-icc",
            Mutation::AddedTopLevelIcc,
            "wsi_multiple_paths_top_level_icc_absent",
        ),
    ] {
        let path = write_fixture(label, mutation);
        let error = validate_wsi_multiple_optical_paths_file(&path, &identity(), &contract())
            .expect_err("mutated multiple-path WSI fixture must fail")
            .to_string();
        assert!(error.contains(finding), "{label}: {error}");
        cleanup(path);
    }
}

#[test]
fn rejects_swapped_path_blocks_and_frames_within_a_path() {
    for (label, mutation, finding) in [
        (
            "swapped-path-blocks",
            Mutation::SwappedPathBlocks,
            "wsi_multiple_paths_path_1_payload_sha256",
        ),
        (
            "swapped-within-path",
            Mutation::SwappedFramesWithinPath,
            "wsi_multiple_paths_frame_1_sha256",
        ),
    ] {
        let path = write_fixture(label, mutation);
        let error = validate_wsi_multiple_optical_paths_file(&path, &identity(), &contract())
            .expect_err("path/pixel ordering mutation must fail")
            .to_string();
        assert!(error.contains(finding), "{label}: {error}");
        cleanup(path);
    }
}

#[test]
fn rejects_manifest_contract_drift() {
    let path = write_fixture("contract-drift", Mutation::None);
    let mut expected = contract();
    expected["optical_paths"][1]["identifier"] = "BRIGHTFIELD".into();
    let error = validate_wsi_multiple_optical_paths_file(&path, &identity(), &expected)
        .expect_err("noncanonical multiple-path manifest expectation must fail")
        .to_string();
    assert!(
        error.contains("wsi_multiple_paths_expected_contract"),
        "{error}"
    );
    cleanup(path);
}

fn contract() -> serde_json::Value {
    wsi_multiple_optical_paths_locked_contract(FOR_UID, SPECIMEN_UID, DIMENSION_UID)
}

fn identity() -> Part10Expectations<'static> {
    Part10Expectations {
        sop_class_uid: "1.2.840.10008.5.1.4.1.1.77.1.6",
        sop_instance_uid: SOP_UID,
        transfer_syntax_uid: uids::EXPLICIT_VR_LITTLE_ENDIAN,
        implementation_class_uid: IMPLEMENTATION_UID,
        synthetic_data: "YES",
        rows: 2,
        columns: 2,
        frames: 8,
        samples_per_pixel: 3,
        photometric_interpretation: "RGB",
        bits_allocated: 8,
        bits_stored: 8,
        high_bit: 7,
        pixel_representation: 0,
        planar_configuration: Some(0),
        pixel_data_vr: VR::OB,
        pixel_data_length_formula: PixelDataLengthFormula::ContiguousSamples,
        decoded_frame_hashes: &[],
        palette: None,
        padding: None,
        ct_image: None,
        enhanced_ct_image: None,
        enhanced_mr_image: None,
        enhanced_pet_image: None,
        mg_image: None,
        dx_image: None,
        xa_image: None,
        xrf_image: None,
        us_image: None,
        us_multiframe: None,
        nm_image: None,
        pet_image: None,
        cr_image: None,
        mr_image: None,
        segmentation: None,
    }
}

fn write_fixture(label: &str, mutation: Mutation) -> PathBuf {
    let mut obj = wsi_tiled_full_tests::valid_object(wsi_tiled_full_tests::Mutation::None);
    let profile = obj
        .element(tags::OPTICAL_PATH_SEQUENCE)
        .unwrap()
        .items()
        .unwrap()[0]
        .element(tags::ICC_PROFILE)
        .unwrap()
        .to_bytes()
        .unwrap()
        .into_owned();
    put_str(&mut obj, tags::NUMBER_OF_FRAMES, VR::IS, "8");
    put_u32(&mut obj, tags::NUMBER_OF_OPTICAL_PATHS, 2);

    let mut paths = vec![
        optical_path(
            "BRIGHTFIELD",
            "Deterministic brightfield path",
            550.0,
            &profile,
        ),
        optical_path("ALTERNATE", "Deterministic alternate path", 650.0, &profile),
    ];
    let mut pixels = PIXELS;
    match mutation {
        Mutation::None => {}
        Mutation::DuplicateIdentifier => put_str(
            &mut paths[1],
            tags::OPTICAL_PATH_IDENTIFIER,
            VR::SH,
            "BRIGHTFIELD",
        ),
        Mutation::ReorderedPaths => paths.swap(0, 1),
        Mutation::WrongWavelength => {
            paths[1].put(DataElement::new(
                tags::ILLUMINATION_WAVE_LENGTH,
                VR::FL,
                PrimitiveValue::from(550.0_f32),
            ));
        }
        Mutation::SwappedPathBlocks => pixels.rotate_left(48),
        Mutation::SwappedFramesWithinPath => {
            let mut first = [0_u8; 12];
            first.copy_from_slice(&pixels[..12]);
            pixels.copy_within(12..24, 0);
            pixels[12..24].copy_from_slice(&first);
        }
        Mutation::WrongPathCount => put_u32(&mut obj, tags::NUMBER_OF_OPTICAL_PATHS, 1),
        Mutation::AddedPerFrameGroups => put_sequence(
            &mut obj,
            tags::PER_FRAME_FUNCTIONAL_GROUPS_SEQUENCE,
            vec![InMemDicomObject::new_empty()],
        ),
        Mutation::AddedTopLevelIcc => put_bytes(&mut obj, tags::ICC_PROFILE, profile.clone()),
    }
    put_sequence(&mut obj, tags::OPTICAL_PATH_SEQUENCE, paths);
    put_bytes(&mut obj, tags::PIXEL_DATA, pixels.to_vec());

    let path = std::env::temp_dir().join(format!(
        "dicom-test-suite-wsi-multiple-paths-{}-{label}-{}.dcm",
        std::process::id(),
        FIXTURE_ORDINAL.fetch_add(1, Ordering::Relaxed),
    ));
    obj.with_meta(
        FileMetaTableBuilder::new()
            .media_storage_sop_class_uid("1.2.840.10008.5.1.4.1.1.77.1.6")
            .media_storage_sop_instance_uid(SOP_UID)
            .transfer_syntax(uids::EXPLICIT_VR_LITTLE_ENDIAN)
            .implementation_class_uid(IMPLEMENTATION_UID),
    )
    .unwrap()
    .write_to_file(&path)
    .unwrap();
    path
}

fn optical_path(
    identifier: &str,
    description: &str,
    wavelength: f32,
    profile: &[u8],
) -> InMemDicomObject {
    let code = InMemDicomObject::from_element_iter([
        DataElement::new(tags::CODE_VALUE, VR::SH, "111744"),
        DataElement::new(tags::CODING_SCHEME_DESIGNATOR, VR::SH, "DCM"),
        DataElement::new(tags::CODE_MEANING, VR::LO, "Brightfield illumination"),
    ]);
    InMemDicomObject::from_element_iter([
        DataElement::new(
            tags::ILLUMINATION_TYPE_CODE_SEQUENCE,
            VR::SQ,
            DataSetSequence::from(vec![code]),
        ),
        DataElement::new(
            tags::ILLUMINATION_WAVE_LENGTH,
            VR::FL,
            PrimitiveValue::from(wavelength),
        ),
        DataElement::new(tags::OPTICAL_PATH_IDENTIFIER, VR::SH, identifier),
        DataElement::new(tags::OPTICAL_PATH_DESCRIPTION, VR::ST, description),
        DataElement::new(
            tags::ICC_PROFILE,
            VR::OB,
            PrimitiveValue::U8(profile.to_vec().into()),
        ),
        DataElement::new(tags::COLOR_SPACE, VR::CS, "SRGB"),
    ])
}

const fn multiple_path_pixels() -> [u8; 96] {
    let colors = [
        [255_u8, 0, 0],
        [0, 255, 0],
        [0, 0, 255],
        [255, 255, 255],
        [0, 255, 255],
        [255, 0, 255],
        [255, 255, 0],
        [0, 0, 0],
    ];
    let mut bytes = [0_u8; 96];
    let mut frame = 0;
    while frame < 8 {
        let mut pixel = 0;
        while pixel < 4 {
            let offset = frame * 12 + pixel * 3;
            bytes[offset] = colors[frame][0];
            bytes[offset + 1] = colors[frame][1];
            bytes[offset + 2] = colors[frame][2];
            pixel += 1;
        }
        frame += 1;
    }
    bytes
}

fn put_str(obj: &mut InMemDicomObject, tag: dicom_core::Tag, vr: VR, value: &str) {
    obj.put(DataElement::new(tag, vr, value));
}

fn put_u32(obj: &mut InMemDicomObject, tag: dicom_core::Tag, value: u32) {
    obj.put(DataElement::new(tag, VR::UL, PrimitiveValue::from(value)));
}

fn put_bytes(obj: &mut InMemDicomObject, tag: dicom_core::Tag, value: Vec<u8>) {
    obj.put(DataElement::new(
        tag,
        VR::OB,
        PrimitiveValue::U8(value.into()),
    ));
}

fn put_sequence(obj: &mut InMemDicomObject, tag: dicom_core::Tag, items: Vec<InMemDicomObject>) {
    obj.put(DataElement::new(tag, VR::SQ, DataSetSequence::from(items)));
}

fn cleanup(path: PathBuf) {
    let _ = fs::remove_file(path);
}
