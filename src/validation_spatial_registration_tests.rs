use std::{fs, path::PathBuf};

use dicom_core::{DataElement, PrimitiveValue, VR, value::DataSetSequence};
use dicom_dictionary_std::{tags, uids};
use dicom_object::{FileMetaTableBuilder, InMemDicomObject};

use super::{
    SpatialRegistrationExpectations, SpatialRegistrationReferenceExpectations,
    validate_spatial_registration_file,
};

const REG_UID: &str = "1.2.840.10008.5.1.4.1.1.66.1";
const SOP_UID: &str = "2.25.100000000000000000000000000000000000001";
const SERIES_UID: &str = "2.25.100000000000000000000000000000000000002";
const IMPLEMENTATION_UID: &str = "2.25.100000000000000000000000000000000000003";
const TARGET_STUDY: &str = "2.25.134995199367157411994930388153815854196";
const TARGET_SERIES: &str = "2.25.238876964988566215975277924285531276576";
const TARGET_SOP: &str = "2.25.219051415271916270043274692735027681679";
const TARGET_FOR: &str = "2.25.168702600023177440280518310562536327742";
const SOURCE_STUDY: &str = "2.25.33859400456710853663566358208192828275";
const SOURCE_SERIES: &str = "2.25.203557506487129061365904718969800468008";
const SOURCE_SOP: &str = "2.25.238084448525920044280507300720345263680";
const SOURCE_FOR: &str = "2.25.105625363449803176483593656673719191624";
const TARGET_MATRIX: [&str; 16] = [
    "1", "0", "0", "0", "0", "1", "0", "0", "0", "0", "1", "0", "0", "0", "0", "1",
];
const SOURCE_MATRIX: [&str; 16] = [
    "1", "0", "0", "0.625", "0", "1", "0", "0.625", "0", "0", "1", "2.5", "0", "0", "0", "1",
];

#[test]
fn accepts_the_exact_rigid_registration_contract() {
    let path = write_fixture("valid", &SOURCE_MATRIX, SOURCE_SOP, true, false);
    let validated = validate_spatial_registration_file(&path, &expectations())
        .expect("exact registration should validate");

    assert_eq!(validated.validation["status"], "passed");
    assert!(
        validated.validation["internal"]
            .as_array()
            .is_some_and(|rows| {
                rows.iter().all(|row| row["status"] == "passed")
                    && rows
                        .iter()
                        .any(|row| row["name"] == "spatial_registration_source_landmark")
            })
    );
    cleanup(path);
}

#[test]
fn rejects_a_non_orthonormal_matrix_even_when_declared_rigid() {
    let mut matrix = SOURCE_MATRIX;
    matrix[0] = "2";
    let path = write_fixture("nonorthonormal", &matrix, SOURCE_SOP, true, false);

    let error = validate_spatial_registration_file(&path, &expectations())
        .expect_err("non-orthonormal RIGID matrix must fail");
    assert!(
        error
            .to_string()
            .contains("spatial_registration_source_matrix_orthonormal")
    );
    cleanup(path);
}

#[test]
fn rejects_a_vm_fifteen_matrix() {
    let path = write_fixture("vm15", &SOURCE_MATRIX[..15], SOURCE_SOP, true, false);
    let error = validate_spatial_registration_file(&path, &expectations())
        .expect_err("VM 15 matrix must fail");
    assert!(
        error
            .to_string()
            .contains("spatial_registration_source_matrix_vm")
    );
    cleanup(path);
}

#[test]
fn rejects_a_wrong_homogeneous_final_row() {
    let mut matrix = SOURCE_MATRIX;
    matrix[15] = "2";
    let path = write_fixture("final-row", &matrix, SOURCE_SOP, true, false);
    let error = validate_spatial_registration_file(&path, &expectations())
        .expect_err("wrong homogeneous final row must fail");
    assert!(
        error
            .to_string()
            .contains("spatial_registration_source_matrix_homogeneous_row")
    );
    cleanup(path);
}

#[test]
fn rejects_a_missing_type_two_registration_sequence() {
    let path = write_fixture("missing-type2", &SOURCE_MATRIX, SOURCE_SOP, false, false);

    assert!(validate_spatial_registration_file(&path, &expectations()).is_err());
    cleanup(path);
}

#[test]
fn rejects_common_reference_drift_and_pixel_data() {
    let path = write_fixture(
        "reference-pixel-drift",
        &SOURCE_MATRIX,
        "2.25.999999999999999999999999999999999999999",
        true,
        true,
    );

    let error = validate_spatial_registration_file(&path, &expectations())
        .expect_err("reference drift and Pixel Data must fail");
    let message = error.to_string();
    assert!(message.contains("spatial_registration_other_study_sop_instance_uid"));
    assert!(message.contains("spatial_registration_pixel_data_absent"));
    cleanup(path);
}

fn expectations() -> SpatialRegistrationExpectations<'static> {
    SpatialRegistrationExpectations {
        sop_class_uid: REG_UID,
        sop_instance_uid: SOP_UID,
        transfer_syntax_uid: uids::EXPLICIT_VR_LITTLE_ENDIAN,
        implementation_class_uid: IMPLEMENTATION_UID,
        synthetic_data: "YES",
        patient_id: "DTS-PATIENT-001",
        study_instance_uid: TARGET_STUDY,
        study_id: "DTS-ECT",
        series_instance_uid: SERIES_UID,
        series_number: "8003",
        laterality: "R",
        modality: "REG",
        instance_number: "1",
        content_date: "20260101",
        content_time: "000000",
        content_label: "DTS_RIGID_REG",
        content_description: "Rigid CT pair registration",
        content_creator_name: "DTS^Generator",
        manufacturer: "dicom-test-suite",
        manufacturer_model_name: "Native Spatial Registration",
        device_serial_number: "DTS-REG-001",
        software_versions: "0.1.0",
        registered_frame_of_reference_uid: TARGET_FOR,
        target: SpatialRegistrationReferenceExpectations {
            study_instance_uid: TARGET_STUDY,
            series_instance_uid: TARGET_SERIES,
            sop_class_uid: uids::ENHANCED_CT_IMAGE_STORAGE,
            sop_instance_uid: TARGET_SOP,
            frame_of_reference_uid: TARGET_FOR,
        },
        source: SpatialRegistrationReferenceExpectations {
            study_instance_uid: SOURCE_STUDY,
            series_instance_uid: SOURCE_SERIES,
            sop_class_uid: uids::CT_IMAGE_STORAGE,
            sop_instance_uid: SOURCE_SOP,
            frame_of_reference_uid: SOURCE_FOR,
        },
        target_matrix: TARGET_MATRIX.map(|value| value.parse().unwrap()),
        source_to_registered_matrix: SOURCE_MATRIX.map(|value| value.parse().unwrap()),
        source_landmark_mm: [-0.625, -0.625, 0.0],
        registered_landmark_mm: [0.0, 0.0, 2.5],
        rigid_tolerance: 1.0e-9,
    }
}

fn write_fixture(
    label: &str,
    source_matrix: &[&str],
    common_source_sop: &str,
    include_registration_type: bool,
    include_pixel_data: bool,
) -> PathBuf {
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
        text(tags::SERIES_NUMBER, VR::IS, "8003"),
        text(tags::LATERALITY, VR::CS, "R"),
        text(tags::FRAME_OF_REFERENCE_UID, VR::UI, TARGET_FOR),
        text(tags::POSITION_REFERENCE_INDICATOR, VR::LO, ""),
        text(tags::MANUFACTURER, VR::LO, "dicom-test-suite"),
        text(tags::INSTITUTION_NAME, VR::LO, ""),
        text(tags::INSTITUTION_ADDRESS, VR::ST, ""),
        text(
            tags::MANUFACTURER_MODEL_NAME,
            VR::LO,
            "Native Spatial Registration",
        ),
        text(tags::DEVICE_SERIAL_NUMBER, VR::LO, "DTS-REG-001"),
        text(tags::SOFTWARE_VERSIONS, VR::LO, "0.1.0"),
        text(tags::INSTANCE_NUMBER, VR::IS, "1"),
        text(tags::CONTENT_DATE, VR::DA, "20260101"),
        text(tags::CONTENT_TIME, VR::TM, "000000"),
        text(tags::CONTENT_LABEL, VR::CS, "DTS_RIGID_REG"),
        text(
            tags::CONTENT_DESCRIPTION,
            VR::LO,
            "Rigid CT pair registration",
        ),
        text(tags::CONTENT_CREATOR_NAME, VR::PN, "DTS^Generator"),
        sequence(
            tags::REGISTRATION_SEQUENCE,
            vec![
                registration_item(
                    TARGET_FOR,
                    uids::ENHANCED_CT_IMAGE_STORAGE,
                    TARGET_SOP,
                    &TARGET_MATRIX,
                    true,
                ),
                registration_item(
                    SOURCE_FOR,
                    uids::CT_IMAGE_STORAGE,
                    SOURCE_SOP,
                    source_matrix,
                    include_registration_type,
                ),
            ],
        ),
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
                        common_source_sop,
                    )],
                ),
            ])],
        ),
    ]);
    if include_pixel_data {
        object.put(DataElement::new(
            tags::PIXEL_DATA,
            VR::OB,
            PrimitiveValue::from(vec![0_u8, 0]),
        ));
    }
    let dir = std::env::temp_dir().join(format!(
        "dts-spatial-registration-validation-{label}-{}",
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

fn registration_item(
    frame_of_reference_uid: &str,
    sop_class_uid: &str,
    sop_instance_uid: &str,
    matrix: &[&str],
    include_registration_type: bool,
) -> InMemDicomObject {
    let mut matrix_registration = InMemDicomObject::from_element_iter([
        sequence(
            tags::MATRIX_SEQUENCE,
            vec![InMemDicomObject::from_element_iter([
                text(
                    tags::FRAME_OF_REFERENCE_TRANSFORMATION_MATRIX_TYPE,
                    VR::CS,
                    "RIGID",
                ),
                DataElement::new(
                    tags::FRAME_OF_REFERENCE_TRANSFORMATION_MATRIX,
                    VR::DS,
                    PrimitiveValue::Strs(
                        matrix
                            .iter()
                            .map(|value| (*value).to_string())
                            .collect::<Vec<_>>()
                            .into(),
                    ),
                ),
            ])],
        ),
        text(
            tags::FRAME_OF_REFERENCE_TRANSFORMATION_COMMENT,
            VR::LO,
            "Rigid registration",
        ),
    ]);
    if include_registration_type {
        matrix_registration.put(sequence(tags::REGISTRATION_TYPE_CODE_SEQUENCE, vec![]));
    }
    InMemDicomObject::from_element_iter([
        sequence(
            tags::REFERENCED_IMAGE_SEQUENCE,
            vec![sop_reference(sop_class_uid, sop_instance_uid)],
        ),
        text(tags::FRAME_OF_REFERENCE_UID, VR::UI, frame_of_reference_uid),
        sequence(
            tags::MATRIX_REGISTRATION_SEQUENCE,
            vec![matrix_registration],
        ),
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

fn sequence(tag: dicom_core::Tag, items: Vec<InMemDicomObject>) -> DataElement<InMemDicomObject> {
    DataElement::new(tag, VR::SQ, DataSetSequence::from(items))
}

fn cleanup(path: PathBuf) {
    fs::remove_file(&path).unwrap();
    fs::remove_dir(path.parent().unwrap()).unwrap();
}
