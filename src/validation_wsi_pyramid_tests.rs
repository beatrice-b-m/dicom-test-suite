use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

use dicom_core::{DataElement, PrimitiveValue, VR, value::DataSetSequence};
use dicom_dictionary_std::{tags, uids};
use dicom_object::{FileMetaTableBuilder, InMemDicomObject};

use super::{
    Part10Expectations, PixelDataLengthFormula, validate_wsi_pyramid_file, wsi_tiled_full_tests,
};
use crate::{
    WsiPyramidLockedInputs, WsiPyramidMemberIdentity, WsiPyramidRole, wsi_pyramid_locked_contract,
};

const STUDY_UID: &str = "2.25.9101";
const SERIES_UID: &str = "2.25.9102";
const FOR_UID: &str = "2.25.9103";
const SPECIMEN_UID: &str = "2.25.8803";
const PYRAMID_UID: &str = "2.25.9105";
const IMPLEMENTATION_UID: &str = "2.25.9199";
const SOP_UIDS: [&str; 3] = ["2.25.9111", "2.25.9112", "2.25.9113"];
const THUMBNAIL: [u8; 12] = [255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255];
const LABEL: [u8; 12] = [0, 32, 96, 255, 255, 255, 0, 32, 96, 255, 255, 255];
static FIXTURE_ORDINAL: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy)]
enum Mutation {
    None,
    Patient,
    Equipment,
    PyramidMembership,
    ImageType,
    LabelFlag,
    Geometry,
    Icc,
    Payload,
    PerFrame,
}

#[test]
fn accepts_all_three_exact_pyramid_members() {
    for role in [
        WsiPyramidRole::Volume,
        WsiPyramidRole::Thumbnail,
        WsiPyramidRole::Label,
    ] {
        let path = write_fixture(role, Mutation::None);
        let validated = validate_wsi_pyramid_file(&path, &identity(role), &contract(), role)
            .expect("exact pyramid member must validate");
        assert_eq!(validated.validation["status"], "passed");
        for finding in [
            "wsi_pyramid_membership",
            "wsi_pyramid_matrix_sha256",
            "wsi_icc_sha256",
        ] {
            assert!(
                validated.validation["internal"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|item| item["name"] == finding && item["status"] == "passed"),
                "missing passing {finding} for {}",
                role.as_str()
            );
        }
        cleanup(path);
    }
}

#[test]
fn rejects_shared_role_membership_geometry_icc_pixel_and_absence_mutations() {
    for (label, role, mutation, finding) in [
        (
            "patient",
            WsiPyramidRole::Volume,
            Mutation::Patient,
            "wsi_pyramid_patient_id",
        ),
        (
            "equipment",
            WsiPyramidRole::Thumbnail,
            Mutation::Equipment,
            "wsi_pyramid_manufacturer_model_name",
        ),
        (
            "membership",
            WsiPyramidRole::Thumbnail,
            Mutation::PyramidMembership,
            "wsi_pyramid_membership",
        ),
        (
            "role-type",
            WsiPyramidRole::Thumbnail,
            Mutation::ImageType,
            "wsi_pyramid_image_type",
        ),
        (
            "label-flag",
            WsiPyramidRole::Label,
            Mutation::LabelFlag,
            "wsi_pyramid_specimen_label_in_image",
        ),
        (
            "geometry",
            WsiPyramidRole::Volume,
            Mutation::Geometry,
            "wsi_pyramid_matrix_rows",
        ),
        (
            "icc",
            WsiPyramidRole::Volume,
            Mutation::Icc,
            "wsi_icc_sha256",
        ),
        (
            "payload",
            WsiPyramidRole::Label,
            Mutation::Payload,
            "wsi_pyramid_payload_sha256",
        ),
        (
            "per-frame",
            WsiPyramidRole::Thumbnail,
            Mutation::PerFrame,
            "wsi_pyramid_per_frame_absent",
        ),
    ] {
        let path = write_fixture(role, mutation);
        let error = validate_wsi_pyramid_file(&path, &identity(role), &contract(), role)
            .expect_err("mutated pyramid member must fail")
            .to_string();
        assert!(error.contains(finding), "{label}: {error}");
        cleanup(path);
    }
}

#[test]
fn rejects_role_sop_and_repeated_contract_drift() {
    let path = write_fixture(WsiPyramidRole::Volume, Mutation::None);
    let error = validate_wsi_pyramid_file(
        &path,
        &identity(WsiPyramidRole::Thumbnail),
        &contract(),
        WsiPyramidRole::Thumbnail,
    )
    .expect_err("file cannot be validated under another member identity")
    .to_string();
    assert!(
        error.contains("sop_instance") || error.contains("image_pixel"),
        "{error}"
    );
    let mut drift = contract();
    drift["ordered_roles"] = serde_json::json!(["thumbnail", "volume", "label"]);
    let error = validate_wsi_pyramid_file(
        &path,
        &identity(WsiPyramidRole::Volume),
        &drift,
        WsiPyramidRole::Volume,
    )
    .expect_err("drifted repeated contract must fail")
    .to_string();
    assert!(error.contains("wsi_pyramid_expected_contract"), "{error}");
    cleanup(path);
}

fn contract() -> serde_json::Value {
    wsi_pyramid_locked_contract(WsiPyramidLockedInputs {
        study_instance_uid: STUDY_UID,
        series_instance_uid: SERIES_UID,
        frame_of_reference_uid: FOR_UID,
        specimen_uid: SPECIMEN_UID,
        pyramid_uid: PYRAMID_UID,
        members: [
            member(WsiPyramidRole::Volume, 0),
            member(WsiPyramidRole::Thumbnail, 1),
            member(WsiPyramidRole::Label, 2),
        ],
    })
}

fn member(role: WsiPyramidRole, index: usize) -> WsiPyramidMemberIdentity<'static> {
    WsiPyramidMemberIdentity {
        role,
        path: match role {
            WsiPyramidRole::Volume => "volume.dcm",
            WsiPyramidRole::Thumbnail => "thumbnail.dcm",
            WsiPyramidRole::Label => "label.dcm",
        },
        sha256: "0000000000000000000000000000000000000000000000000000000000000000",
        size_bytes: 1,
        sop_instance_uid: SOP_UIDS[index],
    }
}

fn identity(role: WsiPyramidRole) -> Part10Expectations<'static> {
    let (index, frames) = match role {
        WsiPyramidRole::Volume => (0, 4),
        WsiPyramidRole::Thumbnail => (1, 1),
        WsiPyramidRole::Label => (2, 1),
    };
    Part10Expectations {
        sop_class_uid: "1.2.840.10008.5.1.4.1.1.77.1.6",
        sop_instance_uid: SOP_UIDS[index],
        transfer_syntax_uid: uids::EXPLICIT_VR_LITTLE_ENDIAN,
        implementation_class_uid: IMPLEMENTATION_UID,
        synthetic_data: "YES",
        rows: 2,
        columns: 2,
        frames,
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

fn write_fixture(role: WsiPyramidRole, mutation: Mutation) -> PathBuf {
    let base_mutation = if matches!(mutation, Mutation::Icc) {
        wsi_tiled_full_tests::Mutation::TamperedIcc
    } else {
        wsi_tiled_full_tests::Mutation::None
    };
    let mut obj = wsi_tiled_full_tests::valid_object(base_mutation);
    let (index, image_type, flag, frames, matrix, spacing, extent, pixels) = match role {
        WsiPyramidRole::Volume => (
            0,
            r"ORIGINAL\PRIMARY\VOLUME\NONE",
            "NO",
            4,
            4,
            r"0.5\0.5",
            2.0,
            None,
        ),
        WsiPyramidRole::Thumbnail => (
            1,
            r"DERIVED\PRIMARY\THUMBNAIL\RESAMPLED",
            "NO",
            1,
            2,
            r"1.0\1.0",
            2.0,
            Some(THUMBNAIL.as_slice()),
        ),
        WsiPyramidRole::Label => (
            2,
            r"ORIGINAL\PRIMARY\LABEL\NONE",
            "YES",
            1,
            2,
            r"0.5\0.5",
            1.0,
            Some(LABEL.as_slice()),
        ),
    };
    for (tag, vr, value) in [
        (tags::SOP_INSTANCE_UID, VR::UI, SOP_UIDS[index]),
        (tags::PATIENT_ID, VR::LO, "DTS-PATIENT-001"),
        (tags::STUDY_INSTANCE_UID, VR::UI, STUDY_UID),
        (tags::SERIES_INSTANCE_UID, VR::UI, SERIES_UID),
        (tags::FRAME_OF_REFERENCE_UID, VR::UI, FOR_UID),
        (
            tags::MANUFACTURER_MODEL_NAME,
            VR::LO,
            "Native WSI Pyramid",
        ),
        (tags::IMAGE_TYPE, VR::CS, image_type),
        (tags::SPECIMEN_LABEL_IN_IMAGE, VR::CS, flag),
        (
            tags::NUMBER_OF_FRAMES,
            VR::IS,
            if frames == 4 { "4" } else { "1" },
        ),
    ] {
        put_str(&mut obj, tag, vr, value);
    }
    put_u32(&mut obj, tags::TOTAL_PIXEL_MATRIX_ROWS, matrix);
    put_u32(&mut obj, tags::TOTAL_PIXEL_MATRIX_COLUMNS, matrix);
    put_f32(&mut obj, tags::IMAGED_VOLUME_WIDTH, extent);
    put_f32(&mut obj, tags::IMAGED_VOLUME_HEIGHT, extent);
    put_shared(&mut obj, image_type, spacing);
    if role != WsiPyramidRole::Label {
        put_str(&mut obj, tags::PYRAMID_UID, VR::UI, PYRAMID_UID);
    } else {
        obj.remove_element(tags::PYRAMID_UID);
    }
    obj.remove_element(tags::PYRAMID_LABEL);
    obj.remove_element(tags::PYRAMID_DESCRIPTION);
    if let Some(pixels) = pixels {
        put_bytes(&mut obj, tags::PIXEL_DATA, pixels.to_vec());
    }
    match mutation {
        Mutation::None | Mutation::Icc => {}
        Mutation::Patient => put_str(&mut obj, tags::PATIENT_ID, VR::LO, "OTHER"),
        Mutation::Equipment => put_str(
            &mut obj,
            tags::MANUFACTURER_MODEL_NAME,
            VR::LO,
            "CROSSED-EQUIPMENT",
        ),
        Mutation::PyramidMembership => put_str(&mut obj, tags::PYRAMID_UID, VR::UI, "2.25.9991"),
        Mutation::ImageType => put_str(
            &mut obj,
            tags::IMAGE_TYPE,
            VR::CS,
            r"ORIGINAL\PRIMARY\VOLUME\NONE",
        ),
        Mutation::LabelFlag => put_str(&mut obj, tags::SPECIMEN_LABEL_IN_IMAGE, VR::CS, "NO"),
        Mutation::Geometry => put_u32(&mut obj, tags::TOTAL_PIXEL_MATRIX_ROWS, 5),
        Mutation::Payload => {
            let mut changed = LABEL;
            changed[0] ^= 1;
            put_bytes(&mut obj, tags::PIXEL_DATA, changed.to_vec());
        }
        Mutation::PerFrame => put_sequence(
            &mut obj,
            tags::PER_FRAME_FUNCTIONAL_GROUPS_SEQUENCE,
            vec![InMemDicomObject::new_empty()],
        ),
    }
    let path = std::env::temp_dir().join(format!(
        "dicom-test-suite-wsi-pyramid-{}-{}-{}-{}.dcm",
        std::process::id(),
        role.as_str(),
        mutation as u8,
        FIXTURE_ORDINAL.fetch_add(1, Ordering::Relaxed),
    ));
    obj.with_meta(
        FileMetaTableBuilder::new()
            .media_storage_sop_class_uid("1.2.840.10008.5.1.4.1.1.77.1.6")
            .media_storage_sop_instance_uid(SOP_UIDS[index])
            .transfer_syntax(uids::EXPLICIT_VR_LITTLE_ENDIAN)
            .implementation_class_uid(IMPLEMENTATION_UID),
    )
    .unwrap()
    .write_to_file(&path)
    .unwrap();
    path
}

fn put_shared(obj: &mut InMemDicomObject, frame_type: &str, spacing: &str) {
    let measures = InMemDicomObject::from_element_iter([
        DataElement::new(tags::PIXEL_SPACING, VR::DS, spacing),
        DataElement::new(tags::SLICE_THICKNESS, VR::DS, "0.001"),
    ]);
    let frame = InMemDicomObject::from_element_iter([DataElement::new(
        tags::FRAME_TYPE,
        VR::CS,
        frame_type,
    )]);
    let shared = InMemDicomObject::from_element_iter([
        DataElement::new(
            tags::PIXEL_MEASURES_SEQUENCE,
            VR::SQ,
            DataSetSequence::from(vec![measures]),
        ),
        DataElement::new(
            tags::WHOLE_SLIDE_MICROSCOPY_IMAGE_FRAME_TYPE_SEQUENCE,
            VR::SQ,
            DataSetSequence::from(vec![frame]),
        ),
    ]);
    put_sequence(obj, tags::SHARED_FUNCTIONAL_GROUPS_SEQUENCE, vec![shared]);
}
fn put_str(obj: &mut InMemDicomObject, tag: dicom_core::Tag, vr: VR, value: &str) {
    obj.put(DataElement::new(tag, vr, value));
}
fn put_u32(obj: &mut InMemDicomObject, tag: dicom_core::Tag, value: u32) {
    obj.put(DataElement::new(tag, VR::UL, PrimitiveValue::from(value)));
}
fn put_f32(obj: &mut InMemDicomObject, tag: dicom_core::Tag, value: f32) {
    obj.put(DataElement::new(tag, VR::FL, PrimitiveValue::from(value)));
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
