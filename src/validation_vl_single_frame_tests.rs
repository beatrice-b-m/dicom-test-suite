use std::{fs, path::PathBuf};

use dicom_core::{DataElement, PrimitiveValue, VR, value::DataSetSequence};
use dicom_dictionary_std::{tags, uids};
use dicom_object::{FileMetaTableBuilder, InMemDicomObject};

use super::{Part10Expectations, PixelDataLengthFormula, validate_part10_file};

const SOP_UID: &str = "2.25.7401";
const IMPLEMENTATION_UID: &str = "2.25.7499";
const PIXELS: [u8; 12] = [255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255];
const FRAME_HASHES: [&str; 1] =
    ["6733cdd08e5c7ef0453e2759ef0d28fbd43ea2aa7883b55422a13dac38e23ecc"];

#[derive(Clone, Copy, Debug)]
enum VlKind {
    Endoscopic,
    Microscopic,
}

impl VlKind {
    fn sop_class_uid(self) -> &'static str {
        match self {
            Self::Endoscopic => uids::VL_ENDOSCOPIC_IMAGE_STORAGE,
            Self::Microscopic => uids::VL_MICROSCOPIC_IMAGE_STORAGE,
        }
    }

    fn modality(self) -> &'static str {
        match self {
            Self::Endoscopic => "ES",
            Self::Microscopic => "GM",
        }
    }

    fn body_part(self) -> &'static str {
        match self {
            Self::Endoscopic => "LUNG",
            Self::Microscopic => "EYE",
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum Mutation {
    None,
    WrongSopClass,
    WrongModality,
    WrongBodyPart,
    WrongLaterality,
    WrongImageType,
    WrongLossy,
    NonEmptyAcquisitionContext,
    WrongRows,
    WrongColumns,
    WrongPhotometric,
    WrongPlanarConfiguration,
    WrongBitsAllocated,
    WrongBitsStored,
    WrongHighBit,
    WrongPixelRepresentation,
    WrongPixelVr,
    PixelByte,
    AddedNumberOfFrames,
    AddedFrameOfReference,
    AddedSpecimen,
    AddedOpticalPath,
    AddedIccProfile,
    AddedConversionType,
}

#[test]
fn accepts_exact_endoscopic_and_microscopic_contracts() {
    for kind in [VlKind::Endoscopic, VlKind::Microscopic] {
        let path = write_fixture(kind, "valid", Mutation::None);
        let validated = validate_part10_file(&path, &expectations(kind))
            .expect("exact single-frame VL fixture must validate");
        assert_eq!(validated.validation["status"], "passed");
        assert_eq!(
            validated.validation["standards"][0]["name"],
            match kind {
                VlKind::Endoscopic => "vl_endoscopic_image_sop_class",
                VlKind::Microscopic => "vl_microscopic_image_sop_class",
            }
        );
        cleanup(path);
    }
}

#[test]
fn rejects_identity_metadata_pixel_and_absence_mutations() {
    for kind in [VlKind::Endoscopic, VlKind::Microscopic] {
        for (label, mutation, finding) in [
            (
                "wrong-sop",
                Mutation::WrongSopClass,
                "sop_class_uid_consistency",
            ),
            (
                "wrong-modality",
                Mutation::WrongModality,
                "vl_single_frame_modality",
            ),
            (
                "wrong-body-part",
                Mutation::WrongBodyPart,
                "vl_single_frame_body_part_examined",
            ),
            (
                "wrong-laterality",
                Mutation::WrongLaterality,
                "vl_single_frame_laterality",
            ),
            (
                "wrong-image-type",
                Mutation::WrongImageType,
                "vl_single_frame_image_type",
            ),
            (
                "wrong-lossy",
                Mutation::WrongLossy,
                "vl_single_frame_lossy_image_compression",
            ),
            (
                "nonempty-acquisition-context",
                Mutation::NonEmptyAcquisitionContext,
                "vl_single_frame_acquisition_context_items",
            ),
            ("wrong-rows", Mutation::WrongRows, "rows"),
            ("wrong-columns", Mutation::WrongColumns, "columns"),
            (
                "wrong-photometric",
                Mutation::WrongPhotometric,
                "photometric_interpretation",
            ),
            (
                "wrong-planar",
                Mutation::WrongPlanarConfiguration,
                "planar_configuration",
            ),
            (
                "wrong-bits-allocated",
                Mutation::WrongBitsAllocated,
                "bits_allocated",
            ),
            (
                "wrong-bits-stored",
                Mutation::WrongBitsStored,
                "bits_stored",
            ),
            ("wrong-high-bit", Mutation::WrongHighBit, "high_bit"),
            (
                "wrong-pixel-representation",
                Mutation::WrongPixelRepresentation,
                "pixel_representation",
            ),
            ("wrong-pixel-vr", Mutation::WrongPixelVr, "pixel_data_vr"),
            (
                "pixel-byte",
                Mutation::PixelByte,
                "vl_single_frame_pixel_bytes",
            ),
            (
                "number-of-frames",
                Mutation::AddedNumberOfFrames,
                "vl_single_frame_number_of_frames_absent",
            ),
            (
                "frame-of-reference",
                Mutation::AddedFrameOfReference,
                "vl_single_frame_frame_of_reference_uid_absent",
            ),
            (
                "specimen",
                Mutation::AddedSpecimen,
                "vl_single_frame_specimen_description_sequence_absent",
            ),
            (
                "optical-path",
                Mutation::AddedOpticalPath,
                "vl_single_frame_optical_path_sequence_absent",
            ),
            (
                "icc-profile",
                Mutation::AddedIccProfile,
                "vl_single_frame_icc_profile_absent",
            ),
            (
                "conversion-type",
                Mutation::AddedConversionType,
                "vl_single_frame_conversion_type_absent",
            ),
        ] {
            let path = write_fixture(kind, label, mutation);
            let error = validate_part10_file(&path, &expectations(kind))
                .expect_err("mutated single-frame VL fixture must fail")
                .to_string();
            assert!(error.contains(finding), "{kind:?} {label}: {error}");
            cleanup(path);
        }
    }
}

#[test]
fn rejects_manifest_derived_expectation_drift_even_when_file_matches() {
    let path = write_fixture(VlKind::Endoscopic, "expected-drift", Mutation::WrongRows);
    let mut expected = expectations(VlKind::Endoscopic);
    expected.rows = 3;
    let error = validate_part10_file(&path, &expected)
        .expect_err("drifted manifest-derived VL contract must fail")
        .to_string();
    assert!(
        error.contains("vl_single_frame_expected_contract"),
        "{error}"
    );
    cleanup(path);
}

fn expectations(kind: VlKind) -> Part10Expectations<'static> {
    Part10Expectations {
        sop_class_uid: kind.sop_class_uid(),
        sop_instance_uid: SOP_UID,
        transfer_syntax_uid: uids::EXPLICIT_VR_LITTLE_ENDIAN,
        implementation_class_uid: IMPLEMENTATION_UID,
        synthetic_data: "YES",
        rows: 2,
        columns: 2,
        frames: 1,
        samples_per_pixel: 3,
        photometric_interpretation: "RGB",
        bits_allocated: 8,
        bits_stored: 8,
        high_bit: 7,
        pixel_representation: 0,
        planar_configuration: Some(0),
        pixel_data_vr: VR::OB,
        pixel_data_length_formula: PixelDataLengthFormula::ContiguousSamples,
        decoded_frame_hashes: &FRAME_HASHES,
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

fn write_fixture(kind: VlKind, label: &str, mutation: Mutation) -> PathBuf {
    let mut object = valid_object(kind);
    apply_mutation(&mut object, kind, mutation);
    let media_sop_class = if matches!(mutation, Mutation::WrongSopClass) {
        alternate_sop_class_uid(kind)
    } else {
        kind.sop_class_uid()
    };
    let path = std::env::temp_dir().join(format!(
        "dicom-test-suite-vl-single-frame-{}-{kind:?}-{label}.dcm",
        std::process::id()
    ));
    object
        .with_meta(
            FileMetaTableBuilder::new()
                .media_storage_sop_class_uid(media_sop_class)
                .media_storage_sop_instance_uid(SOP_UID)
                .transfer_syntax(uids::EXPLICIT_VR_LITTLE_ENDIAN)
                .implementation_class_uid(IMPLEMENTATION_UID),
        )
        .expect("file meta")
        .write_to_file(&path)
        .expect("write single-frame VL fixture");
    path
}

fn valid_object(kind: VlKind) -> InMemDicomObject {
    let mut object = InMemDicomObject::new_empty();
    put_str(
        &mut object,
        tags::SOP_CLASS_UID,
        VR::UI,
        kind.sop_class_uid(),
    );
    put_str(&mut object, tags::SOP_INSTANCE_UID, VR::UI, SOP_UID);
    put_str(&mut object, tags::SYNTHETIC_DATA, VR::CS, "YES");
    put_str(&mut object, tags::MODALITY, VR::CS, kind.modality());
    put_str(
        &mut object,
        tags::BODY_PART_EXAMINED,
        VR::CS,
        kind.body_part(),
    );
    put_str(&mut object, tags::LATERALITY, VR::CS, "R");
    put_str(&mut object, tags::IMAGE_TYPE, VR::CS, "ORIGINAL\\PRIMARY");
    put_str(&mut object, tags::LOSSY_IMAGE_COMPRESSION, VR::CS, "00");
    object.put(DataElement::new(
        tags::ACQUISITION_CONTEXT_SEQUENCE,
        VR::SQ,
        DataSetSequence::empty(),
    ));
    put_u16(&mut object, tags::ROWS, 2);
    put_u16(&mut object, tags::COLUMNS, 2);
    put_u16(&mut object, tags::SAMPLES_PER_PIXEL, 3);
    put_str(&mut object, tags::PHOTOMETRIC_INTERPRETATION, VR::CS, "RGB");
    put_u16(&mut object, tags::PLANAR_CONFIGURATION, 0);
    put_u16(&mut object, tags::BITS_ALLOCATED, 8);
    put_u16(&mut object, tags::BITS_STORED, 8);
    put_u16(&mut object, tags::HIGH_BIT, 7);
    put_u16(&mut object, tags::PIXEL_REPRESENTATION, 0);
    object.put(DataElement::new(
        tags::PIXEL_DATA,
        VR::OB,
        PrimitiveValue::U8(PIXELS.to_vec().into()),
    ));
    object
}

fn alternate_sop_class_uid(kind: VlKind) -> &'static str {
    match kind {
        VlKind::Endoscopic => uids::VL_MICROSCOPIC_IMAGE_STORAGE,
        VlKind::Microscopic => uids::VL_ENDOSCOPIC_IMAGE_STORAGE,
    }
}

fn apply_mutation(object: &mut InMemDicomObject, kind: VlKind, mutation: Mutation) {
    match mutation {
        Mutation::None => {}
        Mutation::WrongSopClass => put_str(
            object,
            tags::SOP_CLASS_UID,
            VR::UI,
            alternate_sop_class_uid(kind),
        ),
        Mutation::WrongModality => put_str(object, tags::MODALITY, VR::CS, "XC"),
        Mutation::WrongBodyPart => put_str(object, tags::BODY_PART_EXAMINED, VR::CS, "HEAD"),
        Mutation::WrongLaterality => put_str(object, tags::LATERALITY, VR::CS, "L"),
        Mutation::WrongImageType => put_str(object, tags::IMAGE_TYPE, VR::CS, "DERIVED\\PRIMARY"),
        Mutation::WrongLossy => put_str(object, tags::LOSSY_IMAGE_COMPRESSION, VR::CS, "01"),
        Mutation::NonEmptyAcquisitionContext => {
            object.put(DataElement::new(
                tags::ACQUISITION_CONTEXT_SEQUENCE,
                VR::SQ,
                DataSetSequence::from(vec![InMemDicomObject::new_empty()]),
            ));
        }
        Mutation::WrongRows => put_u16(object, tags::ROWS, 3),
        Mutation::WrongColumns => put_u16(object, tags::COLUMNS, 3),
        Mutation::WrongPhotometric => {
            put_str(object, tags::PHOTOMETRIC_INTERPRETATION, VR::CS, "YBR_FULL")
        }
        Mutation::WrongPlanarConfiguration => put_u16(object, tags::PLANAR_CONFIGURATION, 1),
        Mutation::WrongBitsAllocated => put_u16(object, tags::BITS_ALLOCATED, 16),
        Mutation::WrongBitsStored => put_u16(object, tags::BITS_STORED, 7),
        Mutation::WrongHighBit => put_u16(object, tags::HIGH_BIT, 6),
        Mutation::WrongPixelRepresentation => put_u16(object, tags::PIXEL_REPRESENTATION, 1),
        Mutation::WrongPixelVr => {
            object.put(DataElement::new(
                tags::PIXEL_DATA,
                VR::OW,
                PrimitiveValue::U8(PIXELS.to_vec().into()),
            ));
        }
        Mutation::PixelByte => {
            let mut pixels = PIXELS;
            pixels[0] = 254;
            object.put(DataElement::new(
                tags::PIXEL_DATA,
                VR::OB,
                PrimitiveValue::U8(pixels.to_vec().into()),
            ));
        }
        Mutation::AddedNumberOfFrames => put_str(object, tags::NUMBER_OF_FRAMES, VR::IS, "1"),
        Mutation::AddedFrameOfReference => {
            put_str(object, tags::FRAME_OF_REFERENCE_UID, VR::UI, "2.25.7402")
        }
        Mutation::AddedSpecimen => {
            object.put(DataElement::new(
                tags::SPECIMEN_DESCRIPTION_SEQUENCE,
                VR::SQ,
                DataSetSequence::empty(),
            ));
        }
        Mutation::AddedOpticalPath => {
            object.put(DataElement::new(
                tags::OPTICAL_PATH_SEQUENCE,
                VR::SQ,
                DataSetSequence::empty(),
            ));
        }
        Mutation::AddedIccProfile => {
            object.put(DataElement::new(
                tags::ICC_PROFILE,
                VR::OB,
                PrimitiveValue::U8(vec![0_u8; 128].into()),
            ));
        }
        Mutation::AddedConversionType => put_str(object, tags::CONVERSION_TYPE, VR::CS, "SYN"),
    }
}

fn put_str(object: &mut InMemDicomObject, tag: dicom_core::Tag, vr: VR, value: &str) {
    object.put(DataElement::new(tag, vr, value));
}

fn put_u16(object: &mut InMemDicomObject, tag: dicom_core::Tag, value: u16) {
    object.put(DataElement::new(tag, VR::US, PrimitiveValue::from(value)));
}

fn cleanup(path: PathBuf) {
    let _ = fs::remove_file(path);
}
