use dicom_core::value::Value as DicomValue;
use dicom_dictionary_std::tags;

use super::spatial_registration::{
    SOURCE_TO_TARGET_MATRIX, SPATIAL_REGISTRATION_STORAGE_UID, SpatialRegistrationInput,
    SpatialRegistrationReference, TARGET_IDENTITY_MATRIX, build_spatial_registration,
};

const ENHANCED_CT_STORAGE_UID: &str = "1.2.840.10008.5.1.4.1.1.2.1";
const CT_STORAGE_UID: &str = "1.2.840.10008.5.1.4.1.1.2";

fn locked_input() -> SpatialRegistrationInput<'static> {
    SpatialRegistrationInput {
        sop_instance_uid: "2.25.100000000000000000000000000000000000001",
        series_instance_uid: "2.25.100000000000000000000000000000000000002",
        target: SpatialRegistrationReference {
            study_instance_uid: "2.25.134995199367157411994930388153815854196",
            series_instance_uid: "2.25.238876964988566215975277924285531276576",
            sop_class_uid: ENHANCED_CT_STORAGE_UID,
            sop_instance_uid: "2.25.219051415271916270043274692735027681679",
            frame_of_reference_uid: "2.25.168702600023177440280518310562536327742",
        },
        source: SpatialRegistrationReference {
            study_instance_uid: "2.25.33859400456710853663566358208192828275",
            series_instance_uid: "2.25.203557506487129061365904718969800468008",
            sop_class_uid: CT_STORAGE_UID,
            sop_instance_uid: "2.25.238084448525920044280507300720345263680",
            frame_of_reference_uid: "2.25.105625363449803176483593656673719191624",
        },
    }
}

#[test]
fn spatial_registration_builds_the_locked_iod_identity() {
    let input = locked_input();
    let object = build_spatial_registration(input).expect("locked input should build");

    assert_eq!(
        object
            .element(tags::SOP_CLASS_UID)
            .expect("SOP Class UID")
            .to_str()
            .expect("text"),
        SPATIAL_REGISTRATION_STORAGE_UID
    );
    assert_eq!(
        object
            .element(tags::FRAME_OF_REFERENCE_UID)
            .expect("registered Frame of Reference UID")
            .to_str()
            .expect("text"),
        input.target.frame_of_reference_uid
    );
    assert_eq!(
        object
            .element(tags::MODALITY)
            .expect("Modality")
            .to_str()
            .expect("text"),
        "REG"
    );
    assert_eq!(
        object
            .element(tags::CONTENT_LABEL)
            .expect("Content Label")
            .to_str()
            .expect("text"),
        "DTS_RIGID_REG"
    );
    assert_eq!(
        object
            .element(tags::STUDY_ID)
            .expect("target Study ID")
            .to_str()
            .expect("text"),
        "DTS-ECT"
    );
    assert!(object.element(tags::PIXEL_DATA).is_err());
}

#[test]
fn spatial_registration_locks_target_identity_and_source_translation() {
    let object = build_spatial_registration(locked_input()).expect("locked input should build");
    let registrations = object
        .element(tags::REGISTRATION_SEQUENCE)
        .expect("Registration Sequence")
        .items()
        .expect("sequence items");

    assert_eq!(registrations.len(), 2);
    assert_registration_item(
        &registrations[0],
        locked_input().target,
        &TARGET_IDENTITY_MATRIX,
    );
    assert_registration_item(
        &registrations[1],
        locked_input().source,
        &SOURCE_TO_TARGET_MATRIX,
    );
}

#[test]
fn spatial_registration_partitions_same_and_other_study_references() {
    let input = locked_input();
    let object = build_spatial_registration(input).expect("locked input should build");

    let same_study = object
        .element(tags::REFERENCED_SERIES_SEQUENCE)
        .expect("same-study references")
        .items()
        .expect("sequence items");
    assert_eq!(same_study.len(), 1);
    assert_eq!(
        same_study[0]
            .element(tags::SERIES_INSTANCE_UID)
            .expect("target series UID")
            .to_str()
            .expect("text"),
        input.target.series_instance_uid
    );

    let other_studies = object
        .element(tags::STUDIES_CONTAINING_OTHER_REFERENCED_INSTANCES_SEQUENCE)
        .expect("other-study references")
        .items()
        .expect("sequence items");
    assert_eq!(other_studies.len(), 1);
    assert_eq!(
        other_studies[0]
            .element(tags::STUDY_INSTANCE_UID)
            .expect("source study UID")
            .to_str()
            .expect("text"),
        input.source.study_instance_uid
    );
}

#[test]
fn spatial_registration_rejects_a_shared_frame_of_reference() {
    let mut input = locked_input();
    input.source.frame_of_reference_uid = input.target.frame_of_reference_uid;

    let error = build_spatial_registration(input).expect_err("shared FoR must be rejected");
    assert_eq!(
        error,
        "the locked CT pair must have distinct Frames of Reference"
    );
}

fn assert_registration_item(
    item: &dicom_object::InMemDicomObject,
    reference: SpatialRegistrationReference<'_>,
    expected_matrix: &[&str; 16],
) {
    assert_eq!(
        item.element(tags::FRAME_OF_REFERENCE_UID)
            .expect("source Frame of Reference UID")
            .to_str()
            .expect("text"),
        reference.frame_of_reference_uid
    );
    let image_references = item
        .element(tags::REFERENCED_IMAGE_SEQUENCE)
        .expect("Referenced Image Sequence")
        .items()
        .expect("sequence items");
    assert_eq!(image_references.len(), 1);
    assert_eq!(
        image_references[0]
            .element(tags::REFERENCED_SOP_INSTANCE_UID)
            .expect("Referenced SOP Instance UID")
            .to_str()
            .expect("text"),
        reference.sop_instance_uid
    );

    let matrix_registration = item
        .element(tags::MATRIX_REGISTRATION_SEQUENCE)
        .expect("Matrix Registration Sequence")
        .items()
        .expect("sequence items");
    assert_eq!(matrix_registration.len(), 1);
    assert_eq!(
        matrix_registration[0]
            .element(tags::REGISTRATION_TYPE_CODE_SEQUENCE)
            .expect("Type 2 Registration Type Code Sequence")
            .items()
            .expect("sequence items")
            .len(),
        0
    );

    let matrices = matrix_registration[0]
        .element(tags::MATRIX_SEQUENCE)
        .expect("Matrix Sequence")
        .items()
        .expect("sequence items");
    assert_eq!(matrices.len(), 1);
    assert_eq!(
        matrices[0]
            .element(tags::FRAME_OF_REFERENCE_TRANSFORMATION_MATRIX_TYPE)
            .expect("matrix type")
            .to_str()
            .expect("text"),
        "RIGID"
    );
    let DicomValue::Primitive(matrix) = matrices[0]
        .element(tags::FRAME_OF_REFERENCE_TRANSFORMATION_MATRIX)
        .expect("matrix")
        .value()
    else {
        panic!("matrix must be primitive")
    };
    let values = matrix.to_multi_str().iter().cloned().collect::<Vec<_>>();
    assert_eq!(values, expected_matrix);
}
