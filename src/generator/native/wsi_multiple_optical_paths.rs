use dicom_core::{DataElement, PrimitiveValue, VR, value::DataSetSequence};
use dicom_dictionary_std::tags;
use dicom_object::InMemDicomObject;

use super::{
    icc_profile::{ICC_COLOR_SPACE, ICC_PROFILE_BYTES},
    wsi_tiled_full::{
        WSI_FRAME_SHA256, WSI_PIXEL_BYTES, WSI_TILED_FULL_STORAGE_UID, WsiTiledFullInput,
        build_wsi_tiled_full,
    },
};

pub(in crate::generator) const WSI_MULTIPLE_OPTICAL_PATHS_CASE_ID: &str =
    "vl/wsi/multiple_optical_paths";
pub(in crate::generator) const WSI_MULTIPLE_OPTICAL_PATHS_RECIPE_ID: &str =
    "vl_wsi_multiple_optical_paths";
pub(in crate::generator) const WSI_MULTIPLE_OPTICAL_PATHS_RECIPE_VERSION: &str = "0.1.0";
pub(in crate::generator) const WSI_MULTIPLE_OPTICAL_PATHS_STORAGE_UID: &str =
    WSI_TILED_FULL_STORAGE_UID;
pub(in crate::generator) const WSI_MULTIPLE_OPTICAL_PATHS_OUTPUT_FILE: &str = "instance.dcm";
pub(in crate::generator) const WSI_MULTIPLE_OPTICAL_PATHS_NUMBER_OF_FRAMES: u16 = 8;
pub(in crate::generator) const WSI_MULTIPLE_OPTICAL_PATH_IDENTIFIERS: [&str; 2] =
    ["BRIGHTFIELD", "ALTERNATE"];
pub(in crate::generator) const WSI_MULTIPLE_OPTICAL_PATH_DESCRIPTIONS: [&str; 2] = [
    "Deterministic brightfield path",
    "Deterministic alternate path",
];
pub(in crate::generator) const WSI_MULTIPLE_OPTICAL_PATH_WAVELENGTHS_NM: [f32; 2] = [550.0, 650.0];
pub(in crate::generator) const WSI_MULTIPLE_OPTICAL_PATHS_PIXEL_DATA_SHA256: &str =
    "831fe6e50cbc3f3d82e3f57c984d3c273cdb18dd3bd3ab511b3633dc293f708f";
pub(in crate::generator) const WSI_MULTIPLE_OPTICAL_PATHS_PATH_PAYLOAD_SHA256: [&str; 2] = [
    "b40b0afc9b180d5ebfb54a7db428e13fe09a33dcc9a8f76220f395ba2c68d2db",
    "1f7ee233e83aebb2127b56d5d728f9ca2df9170ec4eb24e929dca261f9badbed",
];
pub(in crate::generator) const WSI_MULTIPLE_OPTICAL_PATHS_MATRIX_SHA256: [&str; 2] = [
    "62d9532d46c3f71b045a1393d95c49c4757ef5e62bb043a61baf4fffed189a2a",
    "caa1a1abb84ec283bbf92a0f00d5bd89650420d0b1fa911e191ddb368f50e09f",
];
pub(in crate::generator) const WSI_MULTIPLE_OPTICAL_PATHS_FRAME_SHA256: [&str; 8] = [
    WSI_FRAME_SHA256[0],
    WSI_FRAME_SHA256[1],
    WSI_FRAME_SHA256[2],
    WSI_FRAME_SHA256[3],
    "f7606fde280d9577c963618cc2a8fa52b15315ff63ec185029cf66bda64435ab",
    "81fd180e1f66d28018580f37d46188c02fd6709f875b3b620090718a8847c282",
    "745598fdcfa2650299b59b42f40c0750087e117d6bc236c66486087cd264ebd8",
    "15ec7bf0b50732b49f8228e07d24365338f9e3ab994b00af08e5a3bffe55fd8b",
];
pub(in crate::generator) const WSI_MULTIPLE_OPTICAL_PATHS_PIXEL_BYTES: [u8; 96] =
    multiple_optical_path_pixel_bytes();

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::generator) struct WsiMultipleOpticalPathsInput<'a> {
    pub(in crate::generator) study_instance_uid: &'a str,
    pub(in crate::generator) series_instance_uid: &'a str,
    pub(in crate::generator) sop_instance_uid: &'a str,
    pub(in crate::generator) frame_of_reference_uid: &'a str,
    pub(in crate::generator) dimension_organization_uid: &'a str,
    pub(in crate::generator) specimen_uid: &'a str,
}

pub(in crate::generator) fn build_wsi_multiple_optical_paths(
    input: WsiMultipleOpticalPathsInput<'_>,
) -> Result<InMemDicomObject, String> {
    let mut object = build_wsi_tiled_full(WsiTiledFullInput {
        study_instance_uid: input.study_instance_uid,
        series_instance_uid: input.series_instance_uid,
        sop_instance_uid: input.sop_instance_uid,
        frame_of_reference_uid: input.frame_of_reference_uid,
        dimension_organization_uid: input.dimension_organization_uid,
        specimen_uid: input.specimen_uid,
    })?;

    put_str(&mut object, tags::SERIES_NUMBER, VR::IS, "44");
    put_str(
        &mut object,
        tags::MANUFACTURER_MODEL_NAME,
        VR::LO,
        "Native Multi-Path WSI",
    );
    put_str(
        &mut object,
        tags::NUMBER_OF_FRAMES,
        VR::IS,
        &WSI_MULTIPLE_OPTICAL_PATHS_NUMBER_OF_FRAMES.to_string(),
    );
    object.put(DataElement::new(
        tags::OPTICAL_PATH_SEQUENCE,
        VR::SQ,
        DataSetSequence::from(
            WSI_MULTIPLE_OPTICAL_PATH_IDENTIFIERS
                .into_iter()
                .zip(WSI_MULTIPLE_OPTICAL_PATH_DESCRIPTIONS)
                .zip(WSI_MULTIPLE_OPTICAL_PATH_WAVELENGTHS_NM)
                .map(|((identifier, description), wavelength_nm)| {
                    optical_path(identifier, description, wavelength_nm)
                })
                .collect::<Vec<_>>(),
        ),
    ));
    object.put(DataElement::new(
        tags::NUMBER_OF_OPTICAL_PATHS,
        VR::UL,
        PrimitiveValue::from(2_u32),
    ));
    object.put(DataElement::new(
        tags::PIXEL_DATA,
        VR::OB,
        PrimitiveValue::from(WSI_MULTIPLE_OPTICAL_PATHS_PIXEL_BYTES.as_slice()),
    ));

    Ok(object)
}

pub(in crate::generator) fn reconstructed_optical_path_matrices() -> [[u8; 48]; 2] {
    let mut matrices = [[0_u8; 48]; 2];
    for (path_index, matrix) in matrices.iter_mut().enumerate() {
        for tile_index in 0..4 {
            let frame_index = path_index * 4 + tile_index;
            let tile_row = tile_index / 2;
            let tile_column = tile_index % 2;
            for row in 0..2 {
                let source = frame_index * 12 + row * 6;
                let destination = ((tile_row * 2 + row) * 4 + tile_column * 2) * 3;
                matrix[destination..destination + 6]
                    .copy_from_slice(&WSI_MULTIPLE_OPTICAL_PATHS_PIXEL_BYTES[source..source + 6]);
            }
        }
    }
    matrices
}

fn optical_path(identifier: &str, description: &str, wavelength_nm: f32) -> InMemDicomObject {
    let illumination_type = InMemDicomObject::from_element_iter([
        DataElement::new(tags::CODE_VALUE, VR::SH, "111744"),
        DataElement::new(tags::CODING_SCHEME_DESIGNATOR, VR::SH, "DCM"),
        DataElement::new(tags::CODE_MEANING, VR::LO, "Brightfield illumination"),
    ]);
    InMemDicomObject::from_element_iter([
        DataElement::new(
            tags::ILLUMINATION_TYPE_CODE_SEQUENCE,
            VR::SQ,
            DataSetSequence::from(vec![illumination_type]),
        ),
        DataElement::new(
            tags::ILLUMINATION_WAVE_LENGTH,
            VR::FL,
            PrimitiveValue::from(wavelength_nm),
        ),
        DataElement::new(tags::OPTICAL_PATH_IDENTIFIER, VR::SH, identifier),
        DataElement::new(tags::OPTICAL_PATH_DESCRIPTION, VR::ST, description),
        DataElement::new(
            tags::ICC_PROFILE,
            VR::OB,
            PrimitiveValue::U8(ICC_PROFILE_BYTES.to_vec().into()),
        ),
        DataElement::new(tags::COLOR_SPACE, VR::CS, ICC_COLOR_SPACE),
    ])
}

const fn multiple_optical_path_pixel_bytes() -> [u8; 96] {
    let second_path_colors = [[0_u8, 255, 255], [255, 0, 255], [255, 255, 0], [0, 0, 0]];
    let mut bytes = [0_u8; 96];
    let mut first_index = 0;
    while first_index < WSI_PIXEL_BYTES.len() {
        bytes[first_index] = WSI_PIXEL_BYTES[first_index];
        first_index += 1;
    }
    let mut frame = 0;
    while frame < 4 {
        let mut pixel = 0;
        while pixel < 4 {
            let offset = 48 + frame * 12 + pixel * 3;
            bytes[offset] = second_path_colors[frame][0];
            bytes[offset + 1] = second_path_colors[frame][1];
            bytes[offset + 2] = second_path_colors[frame][2];
            pixel += 1;
        }
        frame += 1;
    }
    bytes
}

fn put_str(object: &mut InMemDicomObject, tag: dicom_core::Tag, vr: VR, value: &str) {
    object.put(DataElement::new(tag, vr, value));
}

#[cfg(test)]
mod tests {
    use dicom_dictionary_std::tags;

    use super::*;
    use crate::sha256_hex;

    fn input() -> WsiMultipleOpticalPathsInput<'static> {
        WsiMultipleOpticalPathsInput {
            study_instance_uid: "1.2.826.0.1.3680043.10.543.1",
            series_instance_uid: "1.2.826.0.1.3680043.10.543.2",
            sop_instance_uid: "1.2.826.0.1.3680043.10.543.3",
            frame_of_reference_uid: "1.2.826.0.1.3680043.10.543.4",
            dimension_organization_uid: "1.2.826.0.1.3680043.10.543.5",
            specimen_uid: "1.2.826.0.1.3680043.10.543.6",
        }
    }

    #[test]
    fn builds_two_ordered_optical_paths_over_eight_implicit_frames() {
        let object = build_wsi_multiple_optical_paths(input()).unwrap();
        assert_eq!(
            object
                .element(tags::NUMBER_OF_FRAMES)
                .unwrap()
                .to_int::<u16>()
                .unwrap(),
            8
        );
        assert_eq!(
            object
                .element(tags::NUMBER_OF_OPTICAL_PATHS)
                .unwrap()
                .to_int::<u32>()
                .unwrap(),
            2
        );
        assert_eq!(
            object
                .element(tags::DIMENSION_ORGANIZATION_TYPE)
                .unwrap()
                .to_str()
                .unwrap(),
            "TILED_FULL"
        );
        assert_eq!(
            object
                .element(tags::TOTAL_PIXEL_MATRIX_FOCAL_PLANES)
                .unwrap()
                .to_int::<u32>()
                .unwrap(),
            1
        );
        assert!(object.element(tags::DIMENSION_INDEX_SEQUENCE).is_err());
        assert!(
            object
                .element(tags::PER_FRAME_FUNCTIONAL_GROUPS_SEQUENCE)
                .is_err()
        );
        for tag in [
            tags::REFERENCED_SERIES_SEQUENCE,
            tags::CONCATENATION_UID,
            tags::PYRAMID_UID,
            tags::ICC_PROFILE,
        ] {
            assert!(object.element(tag).is_err());
        }

        let paths = object
            .element(tags::OPTICAL_PATH_SEQUENCE)
            .unwrap()
            .items()
            .unwrap();
        assert_eq!(paths.len(), 2);
        for (index, path) in paths.iter().enumerate() {
            assert_eq!(
                path.element(tags::OPTICAL_PATH_IDENTIFIER)
                    .unwrap()
                    .to_str()
                    .unwrap(),
                WSI_MULTIPLE_OPTICAL_PATH_IDENTIFIERS[index]
            );
            assert_eq!(
                path.element(tags::OPTICAL_PATH_DESCRIPTION)
                    .unwrap()
                    .to_str()
                    .unwrap(),
                WSI_MULTIPLE_OPTICAL_PATH_DESCRIPTIONS[index]
            );
            assert_eq!(
                path.element(tags::ILLUMINATION_WAVE_LENGTH)
                    .unwrap()
                    .to_float32()
                    .unwrap(),
                WSI_MULTIPLE_OPTICAL_PATH_WAVELENGTHS_NM[index]
            );
            assert!(path.element(tags::OBJECTIVE_LENS_POWER).is_err());
            assert_eq!(
                path.element(tags::ICC_PROFILE)
                    .unwrap()
                    .to_bytes()
                    .unwrap()
                    .as_ref(),
                ICC_PROFILE_BYTES
            );
            let illumination = path
                .element(tags::ILLUMINATION_TYPE_CODE_SEQUENCE)
                .unwrap()
                .items()
                .unwrap();
            assert_eq!(illumination.len(), 1);
            assert_eq!(
                illumination[0]
                    .element(tags::CODE_VALUE)
                    .unwrap()
                    .to_str()
                    .unwrap(),
                "111744"
            );
        }
        assert_eq!(
            object
                .element(tags::PIXEL_DATA)
                .unwrap()
                .to_bytes()
                .unwrap()
                .as_ref(),
            WSI_MULTIPLE_OPTICAL_PATHS_PIXEL_BYTES
        );
    }

    #[test]
    fn locks_payload_frames_and_two_distinct_path_matrices() {
        assert_eq!(
            sha256_hex(&WSI_MULTIPLE_OPTICAL_PATHS_PIXEL_BYTES),
            WSI_MULTIPLE_OPTICAL_PATHS_PIXEL_DATA_SHA256
        );
        for (frame, expected) in WSI_MULTIPLE_OPTICAL_PATHS_PIXEL_BYTES
            .chunks_exact(12)
            .zip(WSI_MULTIPLE_OPTICAL_PATHS_FRAME_SHA256)
        {
            assert_eq!(sha256_hex(frame), expected);
        }
        for (payload, expected) in WSI_MULTIPLE_OPTICAL_PATHS_PIXEL_BYTES
            .chunks_exact(48)
            .zip(WSI_MULTIPLE_OPTICAL_PATHS_PATH_PAYLOAD_SHA256)
        {
            assert_eq!(sha256_hex(payload), expected);
        }
        let matrices = reconstructed_optical_path_matrices();
        assert_ne!(matrices[0], matrices[1]);
        for (matrix, expected) in matrices
            .iter()
            .zip(WSI_MULTIPLE_OPTICAL_PATHS_MATRIX_SHA256)
        {
            assert_eq!(sha256_hex(matrix), expected);
        }
    }

    #[test]
    fn preserves_base_uid_validation_and_distinctness() {
        let mut invalid = input();
        invalid.specimen_uid = "1.02.3";
        assert!(
            build_wsi_multiple_optical_paths(invalid)
                .unwrap_err()
                .contains("Specimen UID")
        );

        let mut collision = input();
        collision.dimension_organization_uid = collision.sop_instance_uid;
        assert!(
            build_wsi_multiple_optical_paths(collision)
                .unwrap_err()
                .contains("must be distinct")
        );
    }
}
