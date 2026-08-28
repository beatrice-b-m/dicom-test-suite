use dicom_core::{DataElement, PrimitiveValue, VR, value::DataSetSequence};
use dicom_dictionary_std::tags;
use dicom_object::InMemDicomObject;

use super::{
    icc_profile::{ICC_COLOR_SPACE, ICC_PROFILE_BYTES},
    wsi_tiled_full::{
        WSI_PIXEL_BYTES, WSI_TILED_FULL_STORAGE_UID, WsiTiledFullInput, build_wsi_tiled_full,
    },
};

pub(in crate::generator) const WSI_PYRAMID_CASE_ID: &str = "vl/wsi/pyramid_multiresolution";
pub(in crate::generator) const WSI_PYRAMID_RECIPE_ID: &str = "vl_wsi_pyramid_multiresolution";
pub(in crate::generator) const WSI_PYRAMID_RECIPE_VERSION: &str = "0.1.0";
const WSI_PYRAMID_MANUFACTURER_MODEL_NAME: &str = "Native WSI Pyramid";
pub(in crate::generator) const WSI_PYRAMID_STORAGE_UID: &str = WSI_TILED_FULL_STORAGE_UID;
pub(in crate::generator) const WSI_PYRAMID_VOLUME_OUTPUT_FILE: &str = "volume.dcm";
pub(in crate::generator) const WSI_PYRAMID_THUMBNAIL_OUTPUT_FILE: &str = "thumbnail.dcm";
pub(in crate::generator) const WSI_PYRAMID_LABEL_OUTPUT_FILE: &str = "label.dcm";

pub(in crate::generator) const WSI_PYRAMID_VOLUME_IMAGE_TYPE: &str =
    r"ORIGINAL\PRIMARY\VOLUME\NONE";
pub(in crate::generator) const WSI_PYRAMID_THUMBNAIL_IMAGE_TYPE: &str =
    r"DERIVED\PRIMARY\THUMBNAIL\RESAMPLED";
pub(in crate::generator) const WSI_PYRAMID_LABEL_IMAGE_TYPE: &str = r"ORIGINAL\PRIMARY\LABEL\NONE";
pub(in crate::generator) const WSI_PYRAMID_THUMBNAIL_PIXEL_DATA_SHA256: &str =
    "6733cdd08e5c7ef0453e2759ef0d28fbd43ea2aa7883b55422a13dac38e23ecc";
pub(in crate::generator) const WSI_PYRAMID_LABEL_PIXEL_DATA_SHA256: &str =
    "ad078f83d3ea66f075867d116c8c126e9c8a8a9dd873cd27280371c173d8ad02";
pub(in crate::generator) const WSI_PYRAMID_THUMBNAIL_PIXEL_BYTES: [u8; 12] =
    [255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255];
pub(in crate::generator) const WSI_PYRAMID_LABEL_PIXEL_BYTES: [u8; 12] =
    [0, 32, 96, 255, 255, 255, 0, 32, 96, 255, 255, 255];
pub(in crate::generator) const STRESS_WSI_TILE_ROWS: u16 = 256;
pub(in crate::generator) const STRESS_WSI_TILE_COLUMNS: u16 = 256;
pub(in crate::generator) const STRESS_WSI_TOTAL_MATRIX_EDGES: [u32; 3] = [1024, 512, 256];
pub(in crate::generator) const STRESS_WSI_FRAME_COUNTS: [u16; 3] = [16, 4, 1];
pub(in crate::generator) const STRESS_WSI_PIXEL_SPACINGS: [&str; 3] =
    [r"0.0005\0.0005", r"0.001\0.001", r"0.002\0.002"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::generator) struct WsiPyramidInput<'a> {
    pub(in crate::generator) study_instance_uid: &'a str,
    pub(in crate::generator) series_instance_uid: &'a str,
    pub(in crate::generator) frame_of_reference_uid: &'a str,
    pub(in crate::generator) specimen_uid: &'a str,
    pub(in crate::generator) specimen_identifier: &'a str,
    pub(in crate::generator) container_identifier: &'a str,
    pub(in crate::generator) optical_path_identifier: &'a str,
    pub(in crate::generator) pyramid_uid: &'a str,
    pub(in crate::generator) volume_sop_instance_uid: &'a str,
    pub(in crate::generator) thumbnail_sop_instance_uid: &'a str,
    pub(in crate::generator) label_sop_instance_uid: &'a str,
    pub(in crate::generator) volume_dimension_organization_uid: &'a str,
    pub(in crate::generator) thumbnail_dimension_organization_uid: &'a str,
    pub(in crate::generator) label_dimension_organization_uid: &'a str,
}

#[derive(Debug)]
pub(in crate::generator) struct WsiPyramidObjects {
    pub(in crate::generator) volume: InMemDicomObject,
    pub(in crate::generator) thumbnail: InMemDicomObject,
    pub(in crate::generator) label: InMemDicomObject,
}

#[derive(Debug)]
pub(in crate::generator) struct StressWsiPyramidLevel {
    pub(in crate::generator) level_index: usize,
    pub(in crate::generator) total_matrix_edge: u32,
    pub(in crate::generator) frame_count: u16,
    pub(in crate::generator) pixel_spacing: &'static str,
    pub(in crate::generator) pixel_bytes: Vec<u8>,
    pub(in crate::generator) object: InMemDicomObject,
}

pub(in crate::generator) fn build_wsi_pyramid(
    input: WsiPyramidInput<'_>,
) -> Result<WsiPyramidObjects, String> {
    validate_input(input)?;

    let mut volume = build_base(
        input,
        input.volume_sop_instance_uid,
        input.volume_dimension_organization_uid,
    )?;
    configure_role(&mut volume, WsiPyramidRole::Volume, Some(input.pyramid_uid));

    let mut thumbnail = build_base(
        input,
        input.thumbnail_sop_instance_uid,
        input.thumbnail_dimension_organization_uid,
    )?;
    configure_role(
        &mut thumbnail,
        WsiPyramidRole::Thumbnail,
        Some(input.pyramid_uid),
    );

    let mut label = build_base(
        input,
        input.label_sop_instance_uid,
        input.label_dimension_organization_uid,
    )?;
    configure_role(&mut label, WsiPyramidRole::Label, None);

    Ok(WsiPyramidObjects {
        volume,
        thumbnail,
        label,
    })
}

pub(in crate::generator) fn build_stress_wsi_pyramid(
    input: WsiPyramidInput<'_>,
) -> Result<Vec<StressWsiPyramidLevel>, String> {
    validate_input(input)?;
    let sop_instance_uids = [
        input.volume_sop_instance_uid,
        input.thumbnail_sop_instance_uid,
        input.label_sop_instance_uid,
    ];
    let dimension_organization_uids = [
        input.volume_dimension_organization_uid,
        input.thumbnail_dimension_organization_uid,
        input.label_dimension_organization_uid,
    ];
    let mut levels = Vec::with_capacity(3);
    for level_index in 0..3 {
        let mut object = build_base(
            input,
            sop_instance_uids[level_index],
            dimension_organization_uids[level_index],
        )?;
        let total_matrix_edge = STRESS_WSI_TOTAL_MATRIX_EDGES[level_index];
        let frame_count = STRESS_WSI_FRAME_COUNTS[level_index];
        let pixel_spacing = STRESS_WSI_PIXEL_SPACINGS[level_index];
        let pixel_bytes = stress_level_pixel_bytes(level_index, total_matrix_edge)?;
        configure_stress_level(
            &mut object,
            level_index,
            input.pyramid_uid,
            total_matrix_edge,
            frame_count,
            pixel_spacing,
            &pixel_bytes,
        );
        levels.push(StressWsiPyramidLevel {
            level_index,
            total_matrix_edge,
            frame_count,
            pixel_spacing,
            pixel_bytes,
            object,
        });
    }
    Ok(levels)
}

fn configure_stress_level(
    object: &mut InMemDicomObject,
    level_index: usize,
    pyramid_uid: &str,
    total_matrix_edge: u32,
    frame_count: u16,
    pixel_spacing: &str,
    pixel_bytes: &[u8],
) {
    put_str(object, tags::SERIES_NUMBER, VR::IS, "143");
    put_str(
        object,
        tags::MANUFACTURER_MODEL_NAME,
        VR::LO,
        "Native Reduced Stress WSI Pyramid",
    );
    put_str(
        object,
        tags::INSTANCE_NUMBER,
        VR::IS,
        &(level_index + 1).to_string(),
    );
    put_str(
        object,
        tags::IMAGE_TYPE,
        VR::CS,
        WSI_PYRAMID_VOLUME_IMAGE_TYPE,
    );
    put_str(object, tags::PYRAMID_UID, VR::UI, pyramid_uid);
    put_u16(object, tags::ROWS, STRESS_WSI_TILE_ROWS);
    put_u16(object, tags::COLUMNS, STRESS_WSI_TILE_COLUMNS);
    put_str(
        object,
        tags::NUMBER_OF_FRAMES,
        VR::IS,
        &frame_count.to_string(),
    );
    put_u32(object, tags::TOTAL_PIXEL_MATRIX_ROWS, total_matrix_edge);
    put_u32(object, tags::TOTAL_PIXEL_MATRIX_COLUMNS, total_matrix_edge);
    put_f32(object, tags::IMAGED_VOLUME_WIDTH, 0.512);
    put_f32(object, tags::IMAGED_VOLUME_HEIGHT, 0.512);
    put_shared_functional_groups(object, WSI_PYRAMID_VOLUME_IMAGE_TYPE, pixel_spacing);
    object.put(DataElement::new(
        tags::PIXEL_DATA,
        VR::OB,
        PrimitiveValue::from(pixel_bytes),
    ));
}

fn stress_level_pixel_bytes(level_index: usize, total_matrix_edge: u32) -> Result<Vec<u8>, String> {
    let edge = usize::try_from(total_matrix_edge)
        .map_err(|_| "stress WSI matrix edge does not fit usize".to_string())?;
    let tile_edge = usize::from(STRESS_WSI_TILE_ROWS);
    if edge % tile_edge != 0 {
        return Err("stress WSI matrix edge must be divisible by tile edge".to_string());
    }
    let tiles_per_edge = edge / tile_edge;
    let byte_count = edge
        .checked_mul(edge)
        .and_then(|pixels| pixels.checked_mul(3))
        .ok_or_else(|| "stress WSI pixel byte count overflowed".to_string())?;
    let mut bytes = Vec::with_capacity(byte_count);
    let scale = 1_usize << level_index;
    for tile_row in 0..tiles_per_edge {
        for tile_column in 0..tiles_per_edge {
            for row in 0..tile_edge {
                let y = (tile_row * tile_edge + row) * scale;
                for column in 0..tile_edge {
                    let x = (tile_column * tile_edge + column) * scale;
                    let red = ((x * 255) / 1023) as u8;
                    let green = ((y * 255) / 1023) as u8;
                    let blue = if ((x / 64) + (y / 64)) % 2 == 0 {
                        24
                    } else {
                        232
                    };
                    bytes.extend_from_slice(&[red, green, blue]);
                }
            }
        }
    }
    Ok(bytes)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WsiPyramidRole {
    Volume,
    Thumbnail,
    Label,
}

impl WsiPyramidRole {
    fn image_type(self) -> &'static str {
        match self {
            Self::Volume => WSI_PYRAMID_VOLUME_IMAGE_TYPE,
            Self::Thumbnail => WSI_PYRAMID_THUMBNAIL_IMAGE_TYPE,
            Self::Label => WSI_PYRAMID_LABEL_IMAGE_TYPE,
        }
    }

    fn instance_number(self) -> &'static str {
        match self {
            Self::Volume => "1",
            Self::Thumbnail => "2",
            Self::Label => "3",
        }
    }

    fn spacing(self) -> &'static str {
        match self {
            Self::Thumbnail => r"1.0\1.0",
            Self::Volume | Self::Label => r"0.5\0.5",
        }
    }

    fn extent(self) -> f32 {
        match self {
            Self::Volume | Self::Thumbnail => 2.0,
            Self::Label => 1.0,
        }
    }

    fn specimen_label_in_image(self) -> &'static str {
        match self {
            Self::Volume | Self::Thumbnail => "NO",
            Self::Label => "YES",
        }
    }
}

fn build_base(
    input: WsiPyramidInput<'_>,
    sop_instance_uid: &str,
    dimension_organization_uid: &str,
) -> Result<InMemDicomObject, String> {
    let mut object = build_wsi_tiled_full(WsiTiledFullInput {
        study_instance_uid: input.study_instance_uid,
        series_instance_uid: input.series_instance_uid,
        sop_instance_uid,
        frame_of_reference_uid: input.frame_of_reference_uid,
        dimension_organization_uid,
        specimen_uid: input.specimen_uid,
    })?;
    put_shared_identities(&mut object, input);
    Ok(object)
}

fn configure_role(object: &mut InMemDicomObject, role: WsiPyramidRole, pyramid_uid: Option<&str>) {
    put_str(object, tags::SERIES_NUMBER, VR::IS, "43");
    put_str(
        object,
        tags::MANUFACTURER_MODEL_NAME,
        VR::LO,
        WSI_PYRAMID_MANUFACTURER_MODEL_NAME,
    );
    put_str(
        object,
        tags::INSTANCE_NUMBER,
        VR::IS,
        role.instance_number(),
    );
    put_str(object, tags::IMAGE_TYPE, VR::CS, role.image_type());
    put_str(
        object,
        tags::SPECIMEN_LABEL_IN_IMAGE,
        VR::CS,
        role.specimen_label_in_image(),
    );

    if let Some(uid) = pyramid_uid {
        put_str(object, tags::PYRAMID_UID, VR::UI, uid);
    } else {
        object.remove_element(tags::PYRAMID_UID);
    }
    object.remove_element(tags::PYRAMID_LABEL);
    object.remove_element(tags::PYRAMID_DESCRIPTION);

    let matrix_edge = if role == WsiPyramidRole::Volume { 4 } else { 2 };
    put_u32(object, tags::TOTAL_PIXEL_MATRIX_ROWS, matrix_edge);
    put_u32(object, tags::TOTAL_PIXEL_MATRIX_COLUMNS, matrix_edge);
    put_f32(object, tags::IMAGED_VOLUME_WIDTH, role.extent());
    put_f32(object, tags::IMAGED_VOLUME_HEIGHT, role.extent());
    put_shared_functional_groups(object, role.image_type(), role.spacing());

    let pixels: &[u8] = match role {
        WsiPyramidRole::Volume => &WSI_PIXEL_BYTES,
        WsiPyramidRole::Thumbnail => {
            put_str(object, tags::NUMBER_OF_FRAMES, VR::IS, "1");
            &WSI_PYRAMID_THUMBNAIL_PIXEL_BYTES
        }
        WsiPyramidRole::Label => {
            put_str(object, tags::NUMBER_OF_FRAMES, VR::IS, "1");
            &WSI_PYRAMID_LABEL_PIXEL_BYTES
        }
    };
    object.put(DataElement::new(
        tags::PIXEL_DATA,
        VR::OB,
        PrimitiveValue::from(pixels),
    ));
}

fn put_shared_identities(object: &mut InMemDicomObject, input: WsiPyramidInput<'_>) {
    put_str(
        object,
        tags::CONTAINER_IDENTIFIER,
        VR::LO,
        input.container_identifier,
    );
    put_str(
        object,
        tags::BARCODE_VALUE,
        VR::LT,
        input.container_identifier,
    );

    let specimen = InMemDicomObject::from_element_iter([
        DataElement::new(tags::SPECIMEN_IDENTIFIER, VR::LO, input.specimen_identifier),
        DataElement::new(tags::SPECIMEN_UID, VR::UI, input.specimen_uid),
        DataElement::new(
            tags::ISSUER_OF_THE_SPECIMEN_IDENTIFIER_SEQUENCE,
            VR::SQ,
            DataSetSequence::empty(),
        ),
        DataElement::new(
            tags::SPECIMEN_PREPARATION_SEQUENCE,
            VR::SQ,
            DataSetSequence::empty(),
        ),
    ]);
    object.put(DataElement::new(
        tags::SPECIMEN_DESCRIPTION_SEQUENCE,
        VR::SQ,
        DataSetSequence::from(vec![specimen]),
    ));

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
        input.optical_path_identifier,
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
}

fn put_shared_functional_groups(object: &mut InMemDicomObject, frame_type: &str, spacing: &str) {
    let pixel_measures = InMemDicomObject::from_element_iter([
        DataElement::new(tags::PIXEL_SPACING, VR::DS, spacing),
        DataElement::new(tags::SLICE_THICKNESS, VR::DS, "0.001"),
    ]);
    let frame_type = InMemDicomObject::from_element_iter([DataElement::new(
        tags::FRAME_TYPE,
        VR::CS,
        frame_type,
    )]);
    let shared = InMemDicomObject::from_element_iter([
        DataElement::new(
            tags::PIXEL_MEASURES_SEQUENCE,
            VR::SQ,
            DataSetSequence::from(vec![pixel_measures]),
        ),
        DataElement::new(
            tags::WHOLE_SLIDE_MICROSCOPY_IMAGE_FRAME_TYPE_SEQUENCE,
            VR::SQ,
            DataSetSequence::from(vec![frame_type]),
        ),
    ]);
    object.put(DataElement::new(
        tags::SHARED_FUNCTIONAL_GROUPS_SEQUENCE,
        VR::SQ,
        DataSetSequence::from(vec![shared]),
    ));
}

fn validate_input(input: WsiPyramidInput<'_>) -> Result<(), String> {
    let uids = [
        ("Study Instance UID", input.study_instance_uid),
        ("Series Instance UID", input.series_instance_uid),
        ("Frame of Reference UID", input.frame_of_reference_uid),
        ("Specimen UID", input.specimen_uid),
        ("Pyramid UID", input.pyramid_uid),
        ("VOLUME SOP Instance UID", input.volume_sop_instance_uid),
        (
            "THUMBNAIL SOP Instance UID",
            input.thumbnail_sop_instance_uid,
        ),
        ("LABEL SOP Instance UID", input.label_sop_instance_uid),
        (
            "VOLUME Dimension Organization UID",
            input.volume_dimension_organization_uid,
        ),
        (
            "THUMBNAIL Dimension Organization UID",
            input.thumbnail_dimension_organization_uid,
        ),
        (
            "LABEL Dimension Organization UID",
            input.label_dimension_organization_uid,
        ),
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
    for (name, value) in [
        ("Specimen Identifier", input.specimen_identifier),
        ("Container Identifier", input.container_identifier),
        ("Optical Path Identifier", input.optical_path_identifier),
    ] {
        if value.is_empty() {
            return Err(format!("{name} must not be empty"));
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sha256_hex;

    fn input() -> WsiPyramidInput<'static> {
        WsiPyramidInput {
            study_instance_uid: "1.2.826.0.1.3680043.10.543.101",
            series_instance_uid: "1.2.826.0.1.3680043.10.543.102",
            frame_of_reference_uid: "1.2.826.0.1.3680043.10.543.103",
            specimen_uid: "1.2.826.0.1.3680043.10.543.104",
            specimen_identifier: "DTS-SPECIMEN-001",
            container_identifier: "DTS-SLIDE-001",
            optical_path_identifier: "RGB",
            pyramid_uid: "1.2.826.0.1.3680043.10.543.105",
            volume_sop_instance_uid: "1.2.826.0.1.3680043.10.543.106",
            thumbnail_sop_instance_uid: "1.2.826.0.1.3680043.10.543.107",
            label_sop_instance_uid: "1.2.826.0.1.3680043.10.543.108",
            volume_dimension_organization_uid: "1.2.826.0.1.3680043.10.543.109",
            thumbnail_dimension_organization_uid: "1.2.826.0.1.3680043.10.543.110",
            label_dimension_organization_uid: "1.2.826.0.1.3680043.10.543.111",
        }
    }

    #[test]
    fn builds_exact_three_member_pyramid_roles() {
        let objects = build_wsi_pyramid(input()).unwrap();
        for (object, image_type, frames, matrix_edge, spacing, label_in_image) in [
            (
                &objects.volume,
                WSI_PYRAMID_VOLUME_IMAGE_TYPE,
                4,
                4,
                r"0.5\0.5",
                "NO",
            ),
            (
                &objects.thumbnail,
                WSI_PYRAMID_THUMBNAIL_IMAGE_TYPE,
                1,
                2,
                r"1.0\1.0",
                "NO",
            ),
            (
                &objects.label,
                WSI_PYRAMID_LABEL_IMAGE_TYPE,
                1,
                2,
                r"0.5\0.5",
                "YES",
            ),
        ] {
            assert_eq!(
                object.element(tags::IMAGE_TYPE).unwrap().to_str().unwrap(),
                image_type
            );
            assert_eq!(
                object
                    .element(tags::MANUFACTURER_MODEL_NAME)
                    .unwrap()
                    .to_str()
                    .unwrap(),
                WSI_PYRAMID_MANUFACTURER_MODEL_NAME
            );
            assert_eq!(
                object
                    .element(tags::NUMBER_OF_FRAMES)
                    .unwrap()
                    .to_int::<u16>()
                    .unwrap(),
                frames
            );
            assert_eq!(
                object
                    .element(tags::TOTAL_PIXEL_MATRIX_ROWS)
                    .unwrap()
                    .to_int::<u32>()
                    .unwrap(),
                matrix_edge
            );
            assert_eq!(
                object
                    .element(tags::TOTAL_PIXEL_MATRIX_COLUMNS)
                    .unwrap()
                    .to_int::<u32>()
                    .unwrap(),
                matrix_edge
            );
            assert_eq!(
                object
                    .element(tags::SPECIMEN_LABEL_IN_IMAGE)
                    .unwrap()
                    .to_str()
                    .unwrap(),
                label_in_image
            );
            let shared = object
                .element(tags::SHARED_FUNCTIONAL_GROUPS_SEQUENCE)
                .unwrap()
                .items()
                .unwrap();
            let pixel_measures = shared[0]
                .element(tags::PIXEL_MEASURES_SEQUENCE)
                .unwrap()
                .items()
                .unwrap();
            assert_eq!(
                pixel_measures[0]
                    .element(tags::PIXEL_SPACING)
                    .unwrap()
                    .to_str()
                    .unwrap(),
                spacing
            );
            let frame_type = shared[0]
                .element(tags::WHOLE_SLIDE_MICROSCOPY_IMAGE_FRAME_TYPE_SEQUENCE)
                .unwrap()
                .items()
                .unwrap();
            assert_eq!(
                frame_type[0]
                    .element(tags::FRAME_TYPE)
                    .unwrap()
                    .to_str()
                    .unwrap(),
                image_type
            );
        }
        assert_eq!(
            objects
                .volume
                .element(tags::PYRAMID_UID)
                .unwrap()
                .to_str()
                .unwrap(),
            input().pyramid_uid
        );
        assert_eq!(
            objects
                .thumbnail
                .element(tags::PYRAMID_UID)
                .unwrap()
                .to_str()
                .unwrap(),
            input().pyramid_uid
        );
        assert!(objects.label.element(tags::PYRAMID_UID).is_err());
    }

    #[test]
    fn builds_reduced_stress_three_level_tiled_full_pyramid() {
        let first = build_stress_wsi_pyramid(input()).unwrap();
        let second = build_stress_wsi_pyramid(input()).unwrap();
        assert_eq!(first.len(), 3);
        assert_eq!(second.len(), 3);
        for (index, (first, second)) in first.iter().zip(&second).enumerate() {
            assert_eq!(first.level_index, index);
            assert_eq!(
                first.total_matrix_edge,
                STRESS_WSI_TOTAL_MATRIX_EDGES[index]
            );
            assert_eq!(first.frame_count, STRESS_WSI_FRAME_COUNTS[index]);
            assert_eq!(first.pixel_spacing, STRESS_WSI_PIXEL_SPACINGS[index]);
            assert_eq!(first.pixel_bytes, second.pixel_bytes);
            assert_eq!(
                first.pixel_bytes.len(),
                usize::from(STRESS_WSI_FRAME_COUNTS[index])
                    * usize::from(STRESS_WSI_TILE_ROWS)
                    * usize::from(STRESS_WSI_TILE_COLUMNS)
                    * 3
            );
            assert_eq!(
                first
                    .object
                    .element(tags::ROWS)
                    .unwrap()
                    .to_int::<u16>()
                    .unwrap(),
                STRESS_WSI_TILE_ROWS
            );
            assert_eq!(
                first
                    .object
                    .element(tags::TOTAL_PIXEL_MATRIX_ROWS)
                    .unwrap()
                    .to_int::<u32>()
                    .unwrap(),
                STRESS_WSI_TOTAL_MATRIX_EDGES[index]
            );
            assert_eq!(
                first
                    .object
                    .element(tags::NUMBER_OF_FRAMES)
                    .unwrap()
                    .to_int::<u16>()
                    .unwrap(),
                STRESS_WSI_FRAME_COUNTS[index]
            );
            assert_eq!(first.pixel_bytes[0..3], [0, 0, 24]);
        }
        assert_eq!(
            first
                .iter()
                .map(|level| level.pixel_bytes.len())
                .sum::<usize>(),
            4_128_768
        );
    }

    #[test]
    fn locks_companion_pixel_payloads() {
        let objects = build_wsi_pyramid(input()).unwrap();
        assert_eq!(
            sha256_hex(&WSI_PYRAMID_THUMBNAIL_PIXEL_BYTES),
            WSI_PYRAMID_THUMBNAIL_PIXEL_DATA_SHA256
        );
        assert_eq!(
            sha256_hex(&WSI_PYRAMID_LABEL_PIXEL_BYTES),
            WSI_PYRAMID_LABEL_PIXEL_DATA_SHA256
        );
        assert_eq!(
            objects
                .thumbnail
                .element(tags::PIXEL_DATA)
                .unwrap()
                .to_bytes()
                .unwrap()
                .as_ref(),
            WSI_PYRAMID_THUMBNAIL_PIXEL_BYTES
        );
        assert_eq!(
            objects
                .label
                .element(tags::PIXEL_DATA)
                .unwrap()
                .to_bytes()
                .unwrap()
                .as_ref(),
            WSI_PYRAMID_LABEL_PIXEL_BYTES
        );
    }

    #[test]
    fn rejects_cross_member_uid_collisions() {
        let mut input = input();
        input.label_sop_instance_uid = input.thumbnail_sop_instance_uid;
        assert!(
            build_wsi_pyramid(input)
                .unwrap_err()
                .contains("must be distinct")
        );
    }
}
