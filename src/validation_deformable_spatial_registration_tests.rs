use std::{fs, path::PathBuf};

use dicom_core::{DataElement, PrimitiveValue, VR, value::DataSetSequence};
use dicom_dictionary_std::{tags, uids};
use dicom_object::{FileMetaTableBuilder, InMemDicomObject};

use super::{
    DeformableSpatialRegistrationExpectations, SpatialRegistrationReferenceExpectations,
    validate_deformable_spatial_registration_file,
};

const REG_UID: &str = "1.2.840.10008.5.1.4.1.1.66.3";
const SOP_UID: &str = "2.25.100000000000000000000000000000000000011";
const SERIES_UID: &str = "2.25.100000000000000000000000000000000000012";
const IMPLEMENTATION_UID: &str = "2.25.100000000000000000000000000000000000003";
const TARGET_STUDY: &str = "2.25.134995199367157411994930388153815854196";
const TARGET_SERIES: &str = "2.25.238876964988566215975277924285531276576";
const TARGET_SOP: &str = "2.25.219051415271916270043274692735027681679";
const TARGET_FOR: &str = "2.25.168702600023177440280518310562536327742";
const SOURCE_STUDY: &str = "2.25.33859400456710853663566358208192828275";
const SOURCE_SERIES: &str = "2.25.203557506487129061365904718969800468008";
const SOURCE_SOP: &str = "2.25.238084448525920044280507300720345263680";
const SOURCE_FOR: &str = "2.25.105625363449803176483593656673719191624";
const IDENTITY: [&str; 16] = [
    "1", "0", "0", "0", "0", "1", "0", "0", "0", "0", "1", "0", "0", "0", "0", "1",
];
const VECTORS: [[f32; 3]; 4] = [
    [-0.625, -0.625, -2.5],
    [-0.75, -0.625, -2.5],
    [-0.625, -0.75, -2.5],
    [-0.75, -0.75, -2.5],
];
const REGISTERED_POINTS: [[f64; 3]; 4] = [
    [0.0, 0.0, 2.5],
    [0.75, 0.0, 2.5],
    [0.0, 0.75, 2.5],
    [0.75, 0.75, 2.5],
];
const SOURCE_POINTS: [[f64; 3]; 4] = [
    [-0.625, -0.625, 0.0],
    [0.0, -0.625, 0.0],
    [-0.625, 0.0, 0.0],
    [0.0, 0.0, 0.0],
];

#[derive(Clone, Copy)]
enum Mutation {
    None,
    TruncatedPayload,
    PartialNan,
    SwappedVectorOrder,
    ZeroDimension,
    NegativeResolution,
    MissingGrid,
    DuplicateGrid,
    BadPre,
    BadPost,
    RedirectedReference,
    PixelData,
}

#[test]
fn accepts_the_exact_deformable_registration_contract() {
    let path = write_fixture("valid", Mutation::None);
    let validated = validate_deformable_spatial_registration_file(&path, &expectations())
        .expect("exact deformable registration should validate");
    assert_eq!(validated.validation["status"], "passed");
    assert!(
        validated.validation["internal"]
            .as_array()
            .is_some_and(|rows| {
                rows.iter().all(|row| row["status"] == "passed")
                    && rows.iter().any(|row| {
                        row["name"] == "deformable_registration_registered_to_source_mappings"
                    })
            })
    );
    cleanup(path);
}

#[test]
fn rejects_truncated_and_reordered_vector_payloads() {
    for (label, mutation) in [
        ("truncated", Mutation::TruncatedPayload),
        ("swapped", Mutation::SwappedVectorOrder),
    ] {
        let path = write_fixture(label, mutation);
        let error = validate_deformable_spatial_registration_file(&path, &expectations())
            .expect_err("payload mutation must fail")
            .to_string();
        assert!(
            error.contains("deformable_registration_vector_grid_byte_count_equation")
                || error.contains("deformable_registration_vector_grid_decoded_values")
        );
        cleanup(path);
    }
}

#[test]
fn rejects_a_partial_nan_vector_triple() {
    let path = write_fixture("partial-nan", Mutation::PartialNan);
    let error = validate_deformable_spatial_registration_file(&path, &expectations())
        .expect_err("partially undefined vector must fail")
        .to_string();
    assert!(error.contains("deformable_registration_vector_grid_finite_or_all_nan"));
    assert!(error.contains("deformable_registration_vector_grid_all_finite"));
    cleanup(path);
}

#[test]
fn rejects_non_positive_grid_geometry() {
    for (label, mutation, finding) in [
        (
            "zero-dimension",
            Mutation::ZeroDimension,
            "deformable_registration_grid_dimensions_positive",
        ),
        (
            "negative-resolution",
            Mutation::NegativeResolution,
            "deformable_registration_grid_resolution_positive",
        ),
    ] {
        let path = write_fixture(label, mutation);
        let error = validate_deformable_spatial_registration_file(&path, &expectations())
            .expect_err("non-positive grid geometry must fail")
            .to_string();
        assert!(error.contains(finding));
        cleanup(path);
    }
}

#[test]
fn rejects_missing_and_duplicate_grid_items() {
    for (label, mutation) in [
        ("missing-grid", Mutation::MissingGrid),
        ("duplicate-grid", Mutation::DuplicateGrid),
    ] {
        let path = write_fixture(label, mutation);
        assert!(
            validate_deformable_spatial_registration_file(&path, &expectations()).is_err(),
            "{label} must fail"
        );
        cleanup(path);
    }
}

#[test]
fn rejects_bad_pre_and_post_matrix_contracts() {
    for (label, mutation, finding) in [
        (
            "bad-pre",
            Mutation::BadPre,
            "deformable_registration_pre_matrix_identity",
        ),
        (
            "bad-post",
            Mutation::BadPost,
            "deformable_registration_post_matrix_type",
        ),
    ] {
        let path = write_fixture(label, mutation);
        let error = validate_deformable_spatial_registration_file(&path, &expectations())
            .expect_err("bad deformation matrix must fail")
            .to_string();
        assert!(error.contains(finding));
        cleanup(path);
    }
}

#[test]
fn rejects_redirected_reference_and_pixel_insertion() {
    for (label, mutation, finding) in [
        (
            "redirected-reference",
            Mutation::RedirectedReference,
            "deformable_registration_source_sop_instance_uid",
        ),
        (
            "pixel-data",
            Mutation::PixelData,
            "deformable_registration_pixel_data_absent",
        ),
    ] {
        let path = write_fixture(label, mutation);
        let error = validate_deformable_spatial_registration_file(&path, &expectations())
            .expect_err("reference or pixel mutation must fail")
            .to_string();
        assert!(error.contains(finding));
        cleanup(path);
    }
}

fn expectations() -> DeformableSpatialRegistrationExpectations<'static> {
    DeformableSpatialRegistrationExpectations {
        sop_class_uid: REG_UID,
        sop_instance_uid: SOP_UID,
        transfer_syntax_uid: uids::EXPLICIT_VR_LITTLE_ENDIAN,
        implementation_class_uid: IMPLEMENTATION_UID,
        synthetic_data: "YES",
        patient_id: "DTS-PATIENT-001",
        study_instance_uid: TARGET_STUDY,
        study_id: "DTS-ECT",
        series_instance_uid: SERIES_UID,
        series_number: "8004",
        laterality: "R",
        modality: "REG",
        instance_number: "1",
        content_date: "20260101",
        content_time: "000000",
        content_label: "DTS_DEFORM_REG",
        content_description: "Deformable CT pair registration",
        content_creator_name: "DTS^Generator",
        manufacturer: "dicom-test-suite",
        manufacturer_model_name: "Native Deformable Registration",
        device_serial_number: "DTS-DEFREG-001",
        software_versions: "0.1.0",
        registered_frame_of_reference_uid: TARGET_FOR,
        target: reference(
            TARGET_STUDY,
            TARGET_SERIES,
            uids::ENHANCED_CT_IMAGE_STORAGE,
            TARGET_SOP,
            TARGET_FOR,
        ),
        source: reference(
            SOURCE_STUDY,
            SOURCE_SERIES,
            uids::CT_IMAGE_STORAGE,
            SOURCE_SOP,
            SOURCE_FOR,
        ),
        pre_matrix: IDENTITY.map(|value| value.parse().unwrap()),
        post_matrix: IDENTITY.map(|value| value.parse().unwrap()),
        image_orientation_patient: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
        image_position_patient: [0.0, 0.0, 2.5],
        grid_dimensions: [2, 2, 1],
        grid_resolution: [0.75, 0.75, 2.5],
        vector_grid_data_sha256: "d0673d2da1b415db6465047e607b7f16f1a886dfae4ede91764c71bf7df72f47",
        decoded_vectors_mm: &VECTORS,
        registered_points_mm: &REGISTERED_POINTS,
        source_points_mm: &SOURCE_POINTS,
        tolerance: 1.0e-6,
    }
}

fn reference(
    study_instance_uid: &'static str,
    series_instance_uid: &'static str,
    sop_class_uid: &'static str,
    sop_instance_uid: &'static str,
    frame_of_reference_uid: &'static str,
) -> SpatialRegistrationReferenceExpectations<'static> {
    SpatialRegistrationReferenceExpectations {
        study_instance_uid,
        series_instance_uid,
        sop_class_uid,
        sop_instance_uid,
        frame_of_reference_uid,
    }
}

fn write_fixture(label: &str, mutation: Mutation) -> PathBuf {
    let mut vectors = VECTORS.iter().flatten().copied().collect::<Vec<_>>();
    match mutation {
        Mutation::TruncatedPayload => {
            vectors.pop();
        }
        Mutation::PartialNan => vectors[0] = f32::NAN,
        Mutation::SwappedVectorOrder => {
            for offset in 0..3 {
                vectors.swap(3 + offset, 6 + offset);
            }
        }
        _ => {}
    }
    let dimensions = if matches!(mutation, Mutation::ZeroDimension) {
        [2, 0, 1]
    } else {
        [2, 2, 1]
    };
    let resolution = if matches!(mutation, Mutation::NegativeResolution) {
        [0.75, -0.75, 2.5]
    } else {
        [0.75, 0.75, 2.5]
    };
    let grid_items = match mutation {
        Mutation::MissingGrid => vec![],
        Mutation::DuplicateGrid => vec![grid_item(dimensions, resolution, &vectors); 2],
        _ => vec![grid_item(dimensions, resolution, &vectors)],
    };
    let mut pre = IDENTITY;
    if matches!(mutation, Mutation::BadPre) {
        pre[3] = "1";
    }
    let post_type = if matches!(mutation, Mutation::BadPost) {
        "AFFINE"
    } else {
        "RIGID"
    };
    let direct_source_sop = if matches!(mutation, Mutation::RedirectedReference) {
        "2.25.999999999999999999999999999999999999999"
    } else {
        SOURCE_SOP
    };
    let registration = InMemDicomObject::from_element_iter([
        sequence(
            tags::REFERENCED_IMAGE_SEQUENCE,
            vec![sop_reference(uids::CT_IMAGE_STORAGE, direct_source_sop)],
        ),
        text(tags::SOURCE_FRAME_OF_REFERENCE_UID, VR::UI, SOURCE_FOR),
        sequence(tags::DEFORMABLE_REGISTRATION_GRID_SEQUENCE, grid_items),
        sequence(
            tags::PRE_DEFORMATION_MATRIX_REGISTRATION_SEQUENCE,
            vec![matrix_item(&pre, "RIGID")],
        ),
        sequence(
            tags::POST_DEFORMATION_MATRIX_REGISTRATION_SEQUENCE,
            vec![matrix_item(&IDENTITY, post_type)],
        ),
        sequence(tags::REGISTRATION_TYPE_CODE_SEQUENCE, vec![]),
    ]);
    let mut object = InMemDicomObject::from_element_iter([
        text(tags::SOP_CLASS_UID, VR::UI, REG_UID),
        text(tags::SOP_INSTANCE_UID, VR::UI, SOP_UID),
        text(tags::SYNTHETIC_DATA, VR::CS, "YES"),
        text(tags::PATIENT_NAME, VR::PN, "DTS^Synthetic^Patient001"),
        text(tags::PATIENT_ID, VR::LO, "DTS-PATIENT-001"),
        text(tags::PATIENT_BIRTH_DATE, VR::DA, "19700101"),
        text(tags::PATIENT_SEX, VR::CS, "O"),
        text(tags::STUDY_INSTANCE_UID, VR::UI, TARGET_STUDY),
        text(tags::STUDY_DATE, VR::DA, "20260101"),
        text(tags::STUDY_TIME, VR::TM, "000000"),
        text(tags::REFERRING_PHYSICIAN_NAME, VR::PN, ""),
        text(tags::STUDY_ID, VR::SH, "DTS-ECT"),
        text(tags::ACCESSION_NUMBER, VR::SH, ""),
        text(tags::MODALITY, VR::CS, "REG"),
        text(tags::SERIES_INSTANCE_UID, VR::UI, SERIES_UID),
        text(tags::SERIES_NUMBER, VR::IS, "8004"),
        text(tags::LATERALITY, VR::CS, "R"),
        text(tags::FRAME_OF_REFERENCE_UID, VR::UI, TARGET_FOR),
        text(tags::POSITION_REFERENCE_INDICATOR, VR::LO, ""),
        text(tags::MANUFACTURER, VR::LO, "dicom-test-suite"),
        text(tags::INSTITUTION_NAME, VR::LO, ""),
        text(tags::INSTITUTION_ADDRESS, VR::ST, ""),
        text(
            tags::MANUFACTURER_MODEL_NAME,
            VR::LO,
            "Native Deformable Registration",
        ),
        text(tags::DEVICE_SERIAL_NUMBER, VR::LO, "DTS-DEFREG-001"),
        text(tags::SOFTWARE_VERSIONS, VR::LO, "0.1.0"),
        text(tags::INSTANCE_NUMBER, VR::IS, "1"),
        text(tags::CONTENT_DATE, VR::DA, "20260101"),
        text(tags::CONTENT_TIME, VR::TM, "000000"),
        text(tags::CONTENT_LABEL, VR::CS, "DTS_DEFORM_REG"),
        text(
            tags::CONTENT_DESCRIPTION,
            VR::LO,
            "Deformable CT pair registration",
        ),
        text(tags::CONTENT_CREATOR_NAME, VR::PN, "DTS^Generator"),
        sequence(tags::DEFORMABLE_REGISTRATION_SEQUENCE, vec![registration]),
        sequence(
            tags::REFERENCED_SERIES_SEQUENCE,
            vec![reference_series(
                TARGET_SERIES,
                uids::ENHANCED_CT_IMAGE_STORAGE,
                TARGET_SOP,
            )],
        ),
        sequence(
            tags::STUDIES_CONTAINING_OTHER_REFERENCED_INSTANCES_SEQUENCE,
            vec![InMemDicomObject::from_element_iter([
                text(tags::STUDY_INSTANCE_UID, VR::UI, SOURCE_STUDY),
                sequence(
                    tags::REFERENCED_SERIES_SEQUENCE,
                    vec![reference_series(
                        SOURCE_SERIES,
                        uids::CT_IMAGE_STORAGE,
                        SOURCE_SOP,
                    )],
                ),
            ])],
        ),
    ]);
    if matches!(mutation, Mutation::PixelData) {
        object.put(DataElement::new(
            tags::PIXEL_DATA,
            VR::OB,
            PrimitiveValue::from(vec![0_u8, 0]),
        ));
    }
    let dir = std::env::temp_dir().join(format!(
        "dts-deformable-registration-validation-{label}-{}",
        std::process::id()
    ));
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("instance.dcm");
    object
        .with_meta(
            FileMetaTableBuilder::new()
                .transfer_syntax(uids::EXPLICIT_VR_LITTLE_ENDIAN)
                .implementation_class_uid(IMPLEMENTATION_UID)
                .implementation_version_name("DTS_TEST"),
        )
        .unwrap()
        .write_to_file(&path)
        .unwrap();
    path
}

fn grid_item(dimensions: [u32; 3], resolution: [f64; 3], vectors: &[f32]) -> InMemDicomObject {
    InMemDicomObject::from_element_iter([
        multi_str(tags::IMAGE_POSITION_PATIENT, &["0", "0", "2.5"]),
        multi_str(
            tags::IMAGE_ORIENTATION_PATIENT,
            &["1", "0", "0", "0", "1", "0"],
        ),
        DataElement::new(
            tags::GRID_DIMENSIONS,
            VR::UL,
            PrimitiveValue::U32(dimensions.to_vec().into()),
        ),
        DataElement::new(
            tags::GRID_RESOLUTION,
            VR::FD,
            PrimitiveValue::F64(resolution.to_vec().into()),
        ),
        DataElement::new(
            tags::VECTOR_GRID_DATA,
            VR::OF,
            PrimitiveValue::F32(vectors.to_vec().into()),
        ),
    ])
}

fn matrix_item(matrix: &[&str; 16], matrix_type: &str) -> InMemDicomObject {
    InMemDicomObject::from_element_iter([
        text(
            tags::FRAME_OF_REFERENCE_TRANSFORMATION_MATRIX_TYPE,
            VR::CS,
            matrix_type,
        ),
        multi_str(tags::FRAME_OF_REFERENCE_TRANSFORMATION_MATRIX, matrix),
    ])
}

fn reference_series(
    series_uid: &str,
    sop_class_uid: &str,
    sop_instance_uid: &str,
) -> InMemDicomObject {
    InMemDicomObject::from_element_iter([
        sequence(
            tags::REFERENCED_INSTANCE_SEQUENCE,
            vec![sop_reference(sop_class_uid, sop_instance_uid)],
        ),
        text(tags::SERIES_INSTANCE_UID, VR::UI, series_uid),
    ])
}

fn sop_reference(sop_class_uid: &str, sop_instance_uid: &str) -> InMemDicomObject {
    InMemDicomObject::from_element_iter([
        text(tags::REFERENCED_SOP_CLASS_UID, VR::UI, sop_class_uid),
        text(tags::REFERENCED_SOP_INSTANCE_UID, VR::UI, sop_instance_uid),
    ])
}

fn text(tag: dicom_core::Tag, vr: VR, value: &str) -> DataElement<InMemDicomObject> {
    DataElement::new(tag, vr, value)
}

fn multi_str(tag: dicom_core::Tag, values: &[&str]) -> DataElement<InMemDicomObject> {
    DataElement::new(
        tag,
        VR::DS,
        PrimitiveValue::Strs(
            values
                .iter()
                .map(|value| (*value).to_string())
                .collect::<Vec<_>>()
                .into(),
        ),
    )
}

fn sequence(tag: dicom_core::Tag, items: Vec<InMemDicomObject>) -> DataElement<InMemDicomObject> {
    DataElement::new(tag, VR::SQ, DataSetSequence::from(items))
}

fn cleanup(path: PathBuf) {
    fs::remove_file(&path).unwrap();
    fs::remove_dir(path.parent().unwrap()).unwrap();
}
