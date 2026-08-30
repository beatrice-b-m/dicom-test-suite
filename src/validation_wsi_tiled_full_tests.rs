use std::{fs, path::PathBuf};

use dicom_core::{DataElement, PrimitiveValue, VR, value::DataSetSequence};
use dicom_dictionary_std::{tags, uids};
use dicom_object::{FileMetaTableBuilder, InMemDicomObject};

use super::{Part10Expectations, PixelDataLengthFormula, validate_wsi_tiled_full_file};

const SOP_UID: &str = "2.25.8801";
const FOR_UID: &str = "2.25.8802";
const SPECIMEN_UID: &str = "2.25.8803";
const IMPLEMENTATION_UID: &str = "2.25.8899";
const PIXELS: [u8; 48] = tiled_pixels();

#[derive(Clone, Copy)]
pub(super) enum Mutation {
    None,
    WrongMatrixRows,
    SwappedFrames,
    WrongSpecimen,
    TamperedIcc,
    AddedPerFrameGroups,
}

#[test]
fn accepts_exact_tiled_full_wsi_contract() {
    let path = write_fixture("valid", Mutation::None);
    let validated = validate_wsi_tiled_full_file(&path, &identity(), &contract())
        .expect("exact TILED_FULL WSI fixture must validate");
    assert_eq!(validated.validation["status"], "passed");
    assert!(
        validated.validation["internal"]
            .as_array()
            .unwrap()
            .iter()
            .any(|finding| {
                finding["name"] == "wsi_total_pixel_matrix_sha256" && finding["status"] == "passed"
            })
    );
    cleanup(path);
}

#[test]
fn rejects_geometry_order_specimen_icc_and_absence_mutations() {
    for (label, mutation, finding) in [
        (
            "matrix-rows",
            Mutation::WrongMatrixRows,
            "wsi_total_pixel_matrix_rows",
        ),
        ("frame-order", Mutation::SwappedFrames, "wsi_frame_1_sha256"),
        ("specimen", Mutation::WrongSpecimen, "wsi_specimen_uid"),
        ("icc", Mutation::TamperedIcc, "wsi_icc_sha256"),
        (
            "per-frame",
            Mutation::AddedPerFrameGroups,
            "wsi_per_frame_functional_groups_absent",
        ),
    ] {
        let path = write_fixture(label, mutation);
        let error = validate_wsi_tiled_full_file(&path, &identity(), &contract())
            .expect_err("mutated TILED_FULL WSI fixture must fail")
            .to_string();
        assert!(error.contains(finding), "{label}: {error}");
        cleanup(path);
    }
}

#[test]
fn rejects_manifest_contract_drift_even_when_file_is_unchanged() {
    let path = write_fixture("contract-drift", Mutation::None);
    let mut expected = contract();
    expected["tiling"]["implicit_frame_positions"][1]["column_position"] = 1.into();
    let error = validate_wsi_tiled_full_file(&path, &identity(), &expected)
        .expect_err("noncanonical manifest expectation must fail")
        .to_string();
    assert!(error.contains("wsi_expected_contract"), "{error}");
    cleanup(path);
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
        frames: 4,
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

fn contract() -> serde_json::Value {
    crate::wsi_tiled_full_locked_contract(FOR_UID, SPECIMEN_UID)
}

fn write_fixture(label: &str, mutation: Mutation) -> PathBuf {
    let mut object = valid_object(mutation);
    match mutation {
        Mutation::None => {}
        Mutation::WrongMatrixRows => put_u32(&mut object, tags::TOTAL_PIXEL_MATRIX_ROWS, 5),
        Mutation::SwappedFrames => {
            let mut pixels = PIXELS;
            let (first, rest) = pixels.split_at_mut(12);
            first.swap_with_slice(&mut rest[..12]);
            put_bytes(&mut object, tags::PIXEL_DATA, pixels.to_vec());
        }
        Mutation::WrongSpecimen | Mutation::TamperedIcc => {}
        Mutation::AddedPerFrameGroups => put_sequence(
            &mut object,
            tags::PER_FRAME_FUNCTIONAL_GROUPS_SEQUENCE,
            vec![InMemDicomObject::new_empty(); 4],
        ),
    }
    let path = std::env::temp_dir().join(format!(
        "dicom-test-suite-wsi-validation-{}-{label}.dcm",
        std::process::id()
    ));
    object
        .with_meta(
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

pub(super) fn valid_object(mutation: Mutation) -> InMemDicomObject {
    let mut obj = InMemDicomObject::new_empty();
    for (tag, vr, value) in [
        (
            tags::SOP_CLASS_UID,
            VR::UI,
            "1.2.840.10008.5.1.4.1.1.77.1.6",
        ),
        (tags::SOP_INSTANCE_UID, VR::UI, SOP_UID),
        (tags::SYNTHETIC_DATA, VR::CS, "YES"),
        (tags::MODALITY, VR::CS, "SM"),
        (tags::FRAME_OF_REFERENCE_UID, VR::UI, FOR_UID),
        (tags::POSITION_REFERENCE_INDICATOR, VR::LO, "SLIDE_CORNER"),
        (tags::IMAGE_TYPE, VR::CS, r"ORIGINAL\PRIMARY\VOLUME\NONE"),
        (tags::ACQUISITION_DATE_TIME, VR::DT, "20260101000000"),
        (tags::VOLUMETRIC_PROPERTIES, VR::CS, "VOLUME"),
        (tags::SPECIMEN_LABEL_IN_IMAGE, VR::CS, "NO"),
        (tags::BURNED_IN_ANNOTATION, VR::CS, "NO"),
        (tags::FOCUS_METHOD, VR::CS, "AUTO"),
        (tags::EXTENDED_DEPTH_OF_FIELD, VR::CS, "NO"),
        (tags::LOSSY_IMAGE_COMPRESSION, VR::CS, "00"),
        (tags::DIMENSION_ORGANIZATION_TYPE, VR::CS, "TILED_FULL"),
        (tags::TILES_OVERLAP, VR::CS, "NONE"),
        (tags::LABEL_TEXT, VR::UT, "DTS SYNTHETIC SLIDE 001"),
        (tags::BARCODE_VALUE, VR::LT, "DTS-SLIDE-001"),
        (tags::PHOTOMETRIC_INTERPRETATION, VR::CS, "RGB"),
        (tags::NUMBER_OF_FRAMES, VR::IS, "4"),
        (tags::CONTAINER_IDENTIFIER, VR::LO, "DTS-SLIDE-001"),
        (tags::IMAGE_ORIENTATION_SLIDE, VR::DS, r"1\0\0\0\1\0"),
    ] {
        put_str(&mut obj, tag, vr, value);
    }
    for (tag, value) in [
        (tags::SAMPLES_PER_PIXEL, 3),
        (tags::PLANAR_CONFIGURATION, 0),
        (tags::ROWS, 2),
        (tags::COLUMNS, 2),
        (tags::BITS_ALLOCATED, 8),
        (tags::BITS_STORED, 8),
        (tags::HIGH_BIT, 7),
        (tags::PIXEL_REPRESENTATION, 0),
    ] {
        put_u16(&mut obj, tag, value);
    }
    for (tag, value) in [
        (tags::TOTAL_PIXEL_MATRIX_ROWS, 4),
        (tags::TOTAL_PIXEL_MATRIX_COLUMNS, 4),
        (tags::NUMBER_OF_OPTICAL_PATHS, 1),
        (tags::TOTAL_PIXEL_MATRIX_FOCAL_PLANES, 1),
    ] {
        put_u32(&mut obj, tag, value);
    }
    for (tag, value) in [
        (tags::IMAGED_VOLUME_WIDTH, 2.0),
        (tags::IMAGED_VOLUME_HEIGHT, 2.0),
        (tags::IMAGED_VOLUME_DEPTH, 0.001),
    ] {
        obj.put(DataElement::new(
            tag,
            VR::FL,
            PrimitiveValue::from(value as f32),
        ));
    }
    put_sequence(&mut obj, tags::ACQUISITION_CONTEXT_SEQUENCE, vec![]);
    put_sequence(
        &mut obj,
        tags::ISSUER_OF_THE_CONTAINER_IDENTIFIER_SEQUENCE,
        vec![],
    );
    put_sequence(&mut obj, tags::CONTAINER_TYPE_CODE_SEQUENCE, vec![]);

    let mut specimen = InMemDicomObject::new_empty();
    put_str(
        &mut specimen,
        tags::SPECIMEN_IDENTIFIER,
        VR::LO,
        "DTS-SPECIMEN-001",
    );
    put_str(
        &mut specimen,
        tags::SPECIMEN_UID,
        VR::UI,
        if matches!(mutation, Mutation::WrongSpecimen) {
            "2.25.9999"
        } else {
            SPECIMEN_UID
        },
    );
    put_sequence(
        &mut specimen,
        tags::ISSUER_OF_THE_SPECIMEN_IDENTIFIER_SEQUENCE,
        vec![],
    );
    put_sequence(&mut specimen, tags::SPECIMEN_PREPARATION_SEQUENCE, vec![]);
    put_sequence(
        &mut obj,
        tags::SPECIMEN_DESCRIPTION_SEQUENCE,
        vec![specimen],
    );

    let mut code = InMemDicomObject::new_empty();
    put_str(&mut code, tags::CODE_VALUE, VR::SH, "111744");
    put_str(&mut code, tags::CODING_SCHEME_DESIGNATOR, VR::SH, "DCM");
    put_str(
        &mut code,
        tags::CODE_MEANING,
        VR::LO,
        "Brightfield illumination",
    );
    let mut optical = InMemDicomObject::new_empty();
    put_sequence(
        &mut optical,
        tags::ILLUMINATION_TYPE_CODE_SEQUENCE,
        vec![code],
    );
    optical.put(DataElement::new(
        tags::ILLUMINATION_WAVE_LENGTH,
        VR::FL,
        PrimitiveValue::from(550_f32),
    ));
    put_str(&mut optical, tags::OPTICAL_PATH_IDENTIFIER, VR::SH, "RGB");
    put_str(&mut optical, tags::COLOR_SPACE, VR::CS, "SRGB");
    let mut profile = icc_profile();
    if matches!(mutation, Mutation::TamperedIcc) {
        profile[200] ^= 1;
    }
    put_bytes(&mut optical, tags::ICC_PROFILE, profile);
    put_sequence(&mut obj, tags::OPTICAL_PATH_SEQUENCE, vec![optical]);

    let mut origin = InMemDicomObject::new_empty();
    for tag in [
        tags::X_OFFSET_IN_SLIDE_COORDINATE_SYSTEM,
        tags::Y_OFFSET_IN_SLIDE_COORDINATE_SYSTEM,
        tags::Z_OFFSET_IN_SLIDE_COORDINATE_SYSTEM,
    ] {
        put_str(&mut origin, tag, VR::DS, "0");
    }
    put_sequence(
        &mut obj,
        tags::TOTAL_PIXEL_MATRIX_ORIGIN_SEQUENCE,
        vec![origin],
    );
    let mut dimension = InMemDicomObject::new_empty();
    put_str(
        &mut dimension,
        tags::DIMENSION_ORGANIZATION_UID,
        VR::UI,
        "2.25.8804",
    );
    put_sequence(
        &mut obj,
        tags::DIMENSION_ORGANIZATION_SEQUENCE,
        vec![dimension],
    );

    let mut measures = InMemDicomObject::new_empty();
    put_str(&mut measures, tags::PIXEL_SPACING, VR::DS, r"0.5\0.5");
    put_str(&mut measures, tags::SLICE_THICKNESS, VR::DS, "0.001");
    let mut frame_type = InMemDicomObject::new_empty();
    put_str(
        &mut frame_type,
        tags::FRAME_TYPE,
        VR::CS,
        r"ORIGINAL\PRIMARY\VOLUME\NONE",
    );
    let mut shared = InMemDicomObject::new_empty();
    put_sequence(&mut shared, tags::PIXEL_MEASURES_SEQUENCE, vec![measures]);
    put_sequence(
        &mut shared,
        tags::WHOLE_SLIDE_MICROSCOPY_IMAGE_FRAME_TYPE_SEQUENCE,
        vec![frame_type],
    );
    put_sequence(
        &mut obj,
        tags::SHARED_FUNCTIONAL_GROUPS_SEQUENCE,
        vec![shared],
    );
    put_bytes(&mut obj, tags::PIXEL_DATA, PIXELS.to_vec());
    obj
}

fn icc_profile() -> Vec<u8> {
    let source = include_bytes!("assets/dcmtk_srgb_input_profile.hex");
    let hex: Vec<u8> = source
        .iter()
        .copied()
        .filter(|byte| !byte.is_ascii_whitespace())
        .collect();
    hex.chunks_exact(2)
        .map(|pair| (hex_nibble(pair[0]) << 4) | hex_nibble(pair[1]))
        .collect()
}

fn hex_nibble(byte: u8) -> u8 {
    match byte {
        b'0'..=b'9' => byte - b'0',
        b'a'..=b'f' => byte - b'a' + 10,
        b'A'..=b'F' => byte - b'A' + 10,
        _ => panic!("invalid ICC fixture hex"),
    }
}

const fn tiled_pixels() -> [u8; 48] {
    let colors = [[255, 0, 0], [0, 255, 0], [0, 0, 255], [255, 255, 255]];
    let mut bytes = [0; 48];
    let mut frame = 0;
    while frame < 4 {
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

fn put_u16(obj: &mut InMemDicomObject, tag: dicom_core::Tag, value: u16) {
    obj.put(DataElement::new(tag, VR::US, PrimitiveValue::from(value)));
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
