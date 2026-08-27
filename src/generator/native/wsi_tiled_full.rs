use dicom_core::{DataElement, PrimitiveValue, VR, value::DataSetSequence};
use dicom_dictionary_std::tags;
use dicom_object::InMemDicomObject;

use super::icc_profile::{ICC_COLOR_SPACE, ICC_PROFILE_BYTES};

pub(in crate::generator) const WSI_TILED_FULL_CASE_ID: &str = "vl/wsi/tiled_full_small";
pub(in crate::generator) const WSI_TILED_FULL_RECIPE_ID: &str = "vl_wsi_tiled_full_small";
pub(in crate::generator) const WSI_TILED_FULL_RECIPE_VERSION: &str = "0.1.0";
pub(in crate::generator) const WSI_TILED_FULL_STORAGE_UID: &str = "1.2.840.10008.5.1.4.1.1.77.1.6";
pub(in crate::generator) const WSI_TILED_FULL_OUTPUT_FILE: &str = "instance.dcm";
pub(in crate::generator) const WSI_TILE_ROWS: u16 = 2;
pub(in crate::generator) const WSI_TILE_COLUMNS: u16 = 2;
pub(in crate::generator) const WSI_TOTAL_PIXEL_MATRIX_ROWS: u32 = 4;
pub(in crate::generator) const WSI_TOTAL_PIXEL_MATRIX_COLUMNS: u32 = 4;
pub(in crate::generator) const WSI_NUMBER_OF_FRAMES: u16 = 4;
pub(in crate::generator) const WSI_PIXEL_SPACING: &str = r"0.5\0.5";
pub(in crate::generator) const WSI_IMAGE_ORIENTATION_SLIDE: &str = r"1\0\0\0\1\0";
pub(in crate::generator) const WSI_PIXEL_DATA_SHA256: &str =
    "b40b0afc9b180d5ebfb54a7db428e13fe09a33dcc9a8f76220f395ba2c68d2db";
pub(in crate::generator) const WSI_TOTAL_PIXEL_MATRIX_SHA256: &str =
    "62d9532d46c3f71b045a1393d95c49c4757ef5e62bb043a61baf4fffed189a2a";
pub(in crate::generator) const WSI_FRAME_SHA256: [&str; 4] = [
    "fcf067f6323bb42b8292a565a8f826ec5fdb1b142b7a69bf7f7721f0d5d46ef8",
    "6c8f6d772829d493618e079a099cf4f20d8524ed3656f49db234f5bbf60a4e65",
    "7263ad3fd60c6620abd423516d748baedf5e393b1fbdaaf780ff5803a443cc4f",
    "8688d249e9d047b4fc2fb89ce05afe9ec89252ffccdd969de6eef260dd7ffb21",
];
pub(in crate::generator) const WSI_PIXEL_BYTES: [u8; 48] = tiled_pixel_bytes();

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::generator) struct WsiTiledFullInput<'a> {
    pub(in crate::generator) study_instance_uid: &'a str,
    pub(in crate::generator) series_instance_uid: &'a str,
    pub(in crate::generator) sop_instance_uid: &'a str,
    pub(in crate::generator) frame_of_reference_uid: &'a str,
    pub(in crate::generator) dimension_organization_uid: &'a str,
    pub(in crate::generator) specimen_uid: &'a str,
}

pub(in crate::generator) fn build_wsi_tiled_full(
    input: WsiTiledFullInput<'_>,
) -> Result<InMemDicomObject, String> {
    validate_input(input)?;

    let mut object = InMemDicomObject::new_empty();
    put_str(
        &mut object,
        tags::SOP_CLASS_UID,
        VR::UI,
        WSI_TILED_FULL_STORAGE_UID,
    );
    put_str(
        &mut object,
        tags::SOP_INSTANCE_UID,
        VR::UI,
        input.sop_instance_uid,
    );
    put_str(&mut object, tags::SYNTHETIC_DATA, VR::CS, "YES");

    put_str(
        &mut object,
        tags::PATIENT_NAME,
        VR::PN,
        "DTS^Synthetic^Patient001",
    );
    put_str(&mut object, tags::PATIENT_ID, VR::LO, "DTS-PATIENT-001");
    put_str(&mut object, tags::PATIENT_BIRTH_DATE, VR::DA, "19700101");
    put_str(&mut object, tags::PATIENT_SEX, VR::CS, "O");

    put_str(
        &mut object,
        tags::STUDY_INSTANCE_UID,
        VR::UI,
        input.study_instance_uid,
    );
    put_str(&mut object, tags::STUDY_DATE, VR::DA, "20260101");
    put_str(&mut object, tags::STUDY_TIME, VR::TM, "000000");
    put_str(&mut object, tags::REFERRING_PHYSICIAN_NAME, VR::PN, "");
    put_str(&mut object, tags::STUDY_ID, VR::SH, "DTS-WSI");
    put_str(&mut object, tags::ACCESSION_NUMBER, VR::SH, "");

    put_str(&mut object, tags::MODALITY, VR::CS, "SM");
    put_str(
        &mut object,
        tags::SERIES_INSTANCE_UID,
        VR::UI,
        input.series_instance_uid,
    );
    put_str(&mut object, tags::SERIES_NUMBER, VR::IS, "41");

    put_str(
        &mut object,
        tags::FRAME_OF_REFERENCE_UID,
        VR::UI,
        input.frame_of_reference_uid,
    );
    put_str(
        &mut object,
        tags::POSITION_REFERENCE_INDICATOR,
        VR::LO,
        "SLIDE_CORNER",
    );

    put_str(&mut object, tags::MANUFACTURER, VR::LO, "dicom-test-suite");
    put_str(&mut object, tags::INSTITUTION_NAME, VR::LO, "");
    put_str(&mut object, tags::INSTITUTION_ADDRESS, VR::ST, "");
    put_str(
        &mut object,
        tags::MANUFACTURER_MODEL_NAME,
        VR::LO,
        "Native TILED_FULL WSI",
    );
    put_str(
        &mut object,
        tags::DEVICE_SERIAL_NUMBER,
        VR::LO,
        "DTS-WSI-001",
    );
    put_str(
        &mut object,
        tags::SOFTWARE_VERSIONS,
        VR::LO,
        crate::PACKAGE_VERSION,
    );

    put_str(&mut object, tags::INSTANCE_NUMBER, VR::IS, "1");
    put_str(&mut object, tags::CONTENT_DATE, VR::DA, "20260101");
    put_str(&mut object, tags::CONTENT_TIME, VR::TM, "000000");
    put_str(
        &mut object,
        tags::ACQUISITION_DATE_TIME,
        VR::DT,
        "20260101000000",
    );
    put_str(
        &mut object,
        tags::IMAGE_TYPE,
        VR::CS,
        r"ORIGINAL\PRIMARY\VOLUME\NONE",
    );
    put_str(&mut object, tags::VOLUMETRIC_PROPERTIES, VR::CS, "VOLUME");
    put_str(&mut object, tags::BURNED_IN_ANNOTATION, VR::CS, "NO");
    put_str(&mut object, tags::LOSSY_IMAGE_COMPRESSION, VR::CS, "00");
    put_empty_sequence(&mut object, tags::ACQUISITION_CONTEXT_SEQUENCE);

    put_u16(&mut object, tags::SAMPLES_PER_PIXEL, 3);
    put_str(&mut object, tags::PHOTOMETRIC_INTERPRETATION, VR::CS, "RGB");
    put_u16(&mut object, tags::PLANAR_CONFIGURATION, 0);
    put_u16(&mut object, tags::ROWS, WSI_TILE_ROWS);
    put_u16(&mut object, tags::COLUMNS, WSI_TILE_COLUMNS);
    put_str(
        &mut object,
        tags::NUMBER_OF_FRAMES,
        VR::IS,
        &WSI_NUMBER_OF_FRAMES.to_string(),
    );
    put_u16(&mut object, tags::BITS_ALLOCATED, 8);
    put_u16(&mut object, tags::BITS_STORED, 8);
    put_u16(&mut object, tags::HIGH_BIT, 7);
    put_u16(&mut object, tags::PIXEL_REPRESENTATION, 0);

    put_f32(&mut object, tags::IMAGED_VOLUME_WIDTH, 2.0);
    put_f32(&mut object, tags::IMAGED_VOLUME_HEIGHT, 2.0);
    put_f32(&mut object, tags::IMAGED_VOLUME_DEPTH, 0.001);
    put_str(&mut object, tags::SPECIMEN_LABEL_IN_IMAGE, VR::CS, "NO");
    put_str(&mut object, tags::FOCUS_METHOD, VR::CS, "AUTO");
    put_str(&mut object, tags::EXTENDED_DEPTH_OF_FIELD, VR::CS, "NO");

    put_specimen(&mut object, input.specimen_uid);
    put_optical_path(&mut object);
    put_tile_organization(&mut object);
    put_dimension_organization(&mut object, input.dimension_organization_uid);
    put_shared_functional_groups(&mut object);
    put_slide_label(&mut object);

    object.put(DataElement::new(
        tags::PIXEL_DATA,
        VR::OB,
        PrimitiveValue::from(WSI_PIXEL_BYTES.as_slice()),
    ));
    Ok(object)
}

pub(in crate::generator) fn reconstructed_total_pixel_matrix() -> [u8; 48] {
    let mut matrix = [0_u8; 48];
    let frame_size = usize::from(WSI_TILE_ROWS) * usize::from(WSI_TILE_COLUMNS) * 3;
    for frame_index in 0..usize::from(WSI_NUMBER_OF_FRAMES) {
        let tile_row = frame_index / 2;
        let tile_column = frame_index % 2;
        for row in 0..usize::from(WSI_TILE_ROWS) {
            let source = frame_index * frame_size + row * usize::from(WSI_TILE_COLUMNS) * 3;
            let destination = ((tile_row * usize::from(WSI_TILE_ROWS) + row)
                * WSI_TOTAL_PIXEL_MATRIX_COLUMNS as usize
                + tile_column * usize::from(WSI_TILE_COLUMNS))
                * 3;
            matrix[destination..destination + usize::from(WSI_TILE_COLUMNS) * 3].copy_from_slice(
                &WSI_PIXEL_BYTES[source..source + usize::from(WSI_TILE_COLUMNS) * 3],
            );
        }
    }
    matrix
}

fn put_specimen(object: &mut InMemDicomObject, specimen_uid: &str) {
    put_str(object, tags::CONTAINER_IDENTIFIER, VR::LO, "DTS-SLIDE-001");
    put_empty_sequence(object, tags::ISSUER_OF_THE_CONTAINER_IDENTIFIER_SEQUENCE);
    put_empty_sequence(object, tags::CONTAINER_TYPE_CODE_SEQUENCE);

    let mut specimen = InMemDicomObject::new_empty();
    put_str(
        &mut specimen,
        tags::SPECIMEN_IDENTIFIER,
        VR::LO,
        "DTS-SPECIMEN-001",
    );
    put_str(&mut specimen, tags::SPECIMEN_UID, VR::UI, specimen_uid);
    put_empty_sequence(
        &mut specimen,
        tags::ISSUER_OF_THE_SPECIMEN_IDENTIFIER_SEQUENCE,
    );
    put_empty_sequence(&mut specimen, tags::SPECIMEN_PREPARATION_SEQUENCE);
    object.put(DataElement::new(
        tags::SPECIMEN_DESCRIPTION_SEQUENCE,
        VR::SQ,
        DataSetSequence::from(vec![specimen]),
    ));
}

fn put_optical_path(object: &mut InMemDicomObject) {
    let mut optical_path = InMemDicomObject::new_empty();
    put_code_sequence(
        &mut optical_path,
        tags::ILLUMINATION_TYPE_CODE_SEQUENCE,
        "111744",
        "DCM",
        "Brightfield illumination",
    );
    put_f32(&mut optical_path, tags::ILLUMINATION_WAVE_LENGTH, 550.0);
    put_str(
        &mut optical_path,
        tags::OPTICAL_PATH_IDENTIFIER,
        VR::SH,
        "RGB",
    );
    optical_path.put(DataElement::new(
        tags::ICC_PROFILE,
        VR::OB,
        PrimitiveValue::U8(ICC_PROFILE_BYTES.to_vec().into()),
    ));
    put_str(
        &mut optical_path,
        tags::COLOR_SPACE,
        VR::CS,
        ICC_COLOR_SPACE,
    );
    object.put(DataElement::new(
        tags::OPTICAL_PATH_SEQUENCE,
        VR::SQ,
        DataSetSequence::from(vec![optical_path]),
    ));
    put_u32(object, tags::NUMBER_OF_OPTICAL_PATHS, 1);
}

fn put_tile_organization(object: &mut InMemDicomObject) {
    put_u32(
        object,
        tags::TOTAL_PIXEL_MATRIX_COLUMNS,
        WSI_TOTAL_PIXEL_MATRIX_COLUMNS,
    );
    put_u32(
        object,
        tags::TOTAL_PIXEL_MATRIX_ROWS,
        WSI_TOTAL_PIXEL_MATRIX_ROWS,
    );
    let mut origin = InMemDicomObject::new_empty();
    put_str(
        &mut origin,
        tags::X_OFFSET_IN_SLIDE_COORDINATE_SYSTEM,
        VR::DS,
        "0",
    );
    put_str(
        &mut origin,
        tags::Y_OFFSET_IN_SLIDE_COORDINATE_SYSTEM,
        VR::DS,
        "0",
    );
    put_str(
        &mut origin,
        tags::Z_OFFSET_IN_SLIDE_COORDINATE_SYSTEM,
        VR::DS,
        "0",
    );
    object.put(DataElement::new(
        tags::TOTAL_PIXEL_MATRIX_ORIGIN_SEQUENCE,
        VR::SQ,
        DataSetSequence::from(vec![origin]),
    ));
    put_str(
        object,
        tags::IMAGE_ORIENTATION_SLIDE,
        VR::DS,
        WSI_IMAGE_ORIENTATION_SLIDE,
    );
    put_u32(object, tags::TOTAL_PIXEL_MATRIX_FOCAL_PLANES, 1);
    put_str(object, tags::TILES_OVERLAP, VR::CS, "NONE");
}

fn put_dimension_organization(object: &mut InMemDicomObject, uid: &str) {
    let mut dimension = InMemDicomObject::new_empty();
    put_str(
        &mut dimension,
        tags::DIMENSION_ORGANIZATION_UID,
        VR::UI,
        uid,
    );
    object.put(DataElement::new(
        tags::DIMENSION_ORGANIZATION_SEQUENCE,
        VR::SQ,
        DataSetSequence::from(vec![dimension]),
    ));
    put_str(
        object,
        tags::DIMENSION_ORGANIZATION_TYPE,
        VR::CS,
        "TILED_FULL",
    );
}

fn put_shared_functional_groups(object: &mut InMemDicomObject) {
    let mut pixel_measures = InMemDicomObject::new_empty();
    put_str(
        &mut pixel_measures,
        tags::PIXEL_SPACING,
        VR::DS,
        WSI_PIXEL_SPACING,
    );
    put_str(&mut pixel_measures, tags::SLICE_THICKNESS, VR::DS, "0.001");

    let mut frame_type = InMemDicomObject::new_empty();
    put_str(
        &mut frame_type,
        tags::FRAME_TYPE,
        VR::CS,
        r"ORIGINAL\PRIMARY\VOLUME\NONE",
    );

    let mut shared = InMemDicomObject::new_empty();
    shared.put(DataElement::new(
        tags::PIXEL_MEASURES_SEQUENCE,
        VR::SQ,
        DataSetSequence::from(vec![pixel_measures]),
    ));
    shared.put(DataElement::new(
        tags::WHOLE_SLIDE_MICROSCOPY_IMAGE_FRAME_TYPE_SEQUENCE,
        VR::SQ,
        DataSetSequence::from(vec![frame_type]),
    ));
    object.put(DataElement::new(
        tags::SHARED_FUNCTIONAL_GROUPS_SEQUENCE,
        VR::SQ,
        DataSetSequence::from(vec![shared]),
    ));
}

fn put_slide_label(object: &mut InMemDicomObject) {
    put_str(object, tags::LABEL_TEXT, VR::UT, "DTS SYNTHETIC SLIDE 001");
    put_str(object, tags::BARCODE_VALUE, VR::LT, "DTS-SLIDE-001");
}

const fn tiled_pixel_bytes() -> [u8; 48] {
    let colors = [[255_u8, 0, 0], [0, 255, 0], [0, 0, 255], [255, 255, 255]];
    let mut bytes = [0_u8; 48];
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

fn validate_input(input: WsiTiledFullInput<'_>) -> Result<(), String> {
    let uids = [
        ("Study Instance UID", input.study_instance_uid),
        ("Series Instance UID", input.series_instance_uid),
        ("SOP Instance UID", input.sop_instance_uid),
        ("Frame of Reference UID", input.frame_of_reference_uid),
        (
            "Dimension Organization UID",
            input.dimension_organization_uid,
        ),
        ("Specimen UID", input.specimen_uid),
    ];
    for (name, uid) in uids {
        validate_uid(name, uid)?;
    }
    for left in 0..uids.len() {
        for right in left + 1..uids.len() {
            if uids[left].1 == uids[right].1 {
                return Err(format!(
                    "{} and {} must be distinct",
                    uids[left].0, uids[right].0
                ));
            }
        }
    }
    Ok(())
}

fn validate_uid(name: &str, value: &str) -> Result<(), String> {
    if value.is_empty()
        || value.len() > 64
        || value.starts_with('.')
        || value.ends_with('.')
        || value.split('.').any(|part| part.is_empty())
        || value
            .split('.')
            .any(|part| part.len() > 1 && part.starts_with('0'))
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || byte == b'.')
    {
        return Err(format!("{name} must be a valid DICOM UID"));
    }
    Ok(())
}

fn put_str(object: &mut InMemDicomObject, tag: dicom_core::Tag, vr: VR, value: &str) {
    object.put(DataElement::new(tag, vr, value));
}

fn put_u16(object: &mut InMemDicomObject, tag: dicom_core::Tag, value: u16) {
    object.put(DataElement::new(tag, VR::US, PrimitiveValue::from(value)));
}

fn put_u32(object: &mut InMemDicomObject, tag: dicom_core::Tag, value: u32) {
    object.put(DataElement::new(tag, VR::UL, PrimitiveValue::from(value)));
}

fn put_f32(object: &mut InMemDicomObject, tag: dicom_core::Tag, value: f32) {
    object.put(DataElement::new(tag, VR::FL, PrimitiveValue::from(value)));
}

fn put_empty_sequence(object: &mut InMemDicomObject, tag: dicom_core::Tag) {
    object.put(DataElement::new(tag, VR::SQ, DataSetSequence::empty()));
}

fn put_code_sequence(
    object: &mut InMemDicomObject,
    tag: dicom_core::Tag,
    code_value: &str,
    coding_scheme: &str,
    code_meaning: &str,
) {
    object.put(DataElement::new(
        tag,
        VR::SQ,
        DataSetSequence::from(vec![InMemDicomObject::from_element_iter([
            DataElement::new(tags::CODE_VALUE, VR::SH, code_value),
            DataElement::new(tags::CODING_SCHEME_DESIGNATOR, VR::SH, coding_scheme),
            DataElement::new(tags::CODE_MEANING, VR::LO, code_meaning),
        ])]),
    ));
}
