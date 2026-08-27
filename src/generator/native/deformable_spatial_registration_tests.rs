use dicom_core::{VR, value::Value as DicomValue};
use dicom_dictionary_std::tags;

use super::deformable_spatial_registration::{
    DEFORMABLE_SPATIAL_REGISTRATION_STORAGE_UID, DeformableRegistrationReference,
    DeformableSpatialRegistrationInput, GRID_DIMENSIONS, GRID_RESOLUTION, IDENTITY_MATRIX,
    VECTOR_GRID_BYTES, VECTOR_GRID_VALUES, build_deformable_spatial_registration,
};

const ENHANCED_CT_STORAGE_UID: &str = "1.2.840.10008.5.1.4.1.1.2.1";
const CT_STORAGE_UID: &str = "1.2.840.10008.5.1.4.1.1.2";

fn locked_input() -> DeformableSpatialRegistrationInput<'static> {
    DeformableSpatialRegistrationInput {
        sop_instance_uid: "2.25.100000000000000000000000000000000000003",
        series_instance_uid: "2.25.100000000000000000000000000000000000004",
        target: DeformableRegistrationReference {
            study_instance_uid: "2.25.134995199367157411994930388153815854196",
            series_instance_uid: "2.25.238876964988566215975277924285531276576",
            sop_class_uid: ENHANCED_CT_STORAGE_UID,
            sop_instance_uid: "2.25.219051415271916270043274692735027681679",
            frame_of_reference_uid: "2.25.168702600023177440280518310562536327742",
        },
        source: DeformableRegistrationReference {
            study_instance_uid: "2.25.33859400456710853663566358208192828275",
            series_instance_uid: "2.25.203557506487129061365904718969800468008",
            sop_class_uid: CT_STORAGE_UID,
            sop_instance_uid: "2.25.238084448525920044280507300720345263680",
            frame_of_reference_uid: "2.25.105625363449803176483593656673719191624",
        },
    }
}

#[test]
fn deformable_registration_builds_locked_identity_modules_without_pixels() {
    let input = locked_input();
    let object = build_deformable_spatial_registration(input).expect("locked input should build");

    assert_eq!(
        object
            .element(tags::SOP_CLASS_UID)
            .expect("SOP Class UID")
            .to_str()
            .expect("text"),
        DEFORMABLE_SPATIAL_REGISTRATION_STORAGE_UID
    );
    assert_eq!(
        object
            .element(tags::STUDY_INSTANCE_UID)
            .expect("target Study Instance UID")
            .to_str()
            .expect("text"),
        input.target.study_instance_uid
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
            .element(tags::CONTENT_LABEL)
            .expect("Content Label")
            .to_str()
            .expect("text"),
        "DTS_DEFORM_REG"
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
            .element(tags::SERIES_NUMBER)
            .expect("Series Number")
            .to_str()
            .expect("text"),
        "8004"
    );
    assert!(object.element(tags::PIXEL_DATA).is_err());
    assert!(object.element(tags::FLOAT_PIXEL_DATA).is_err());
    assert!(object.element(tags::DOUBLE_FLOAT_PIXEL_DATA).is_err());
}

#[test]
fn deformable_registration_locks_single_source_and_identity_pre_post_matrices() {
    let input = locked_input();
    let object = build_deformable_spatial_registration(input).expect("locked input should build");
    let registrations = object
        .element(tags::DEFORMABLE_REGISTRATION_SEQUENCE)
        .expect("Deformable Registration Sequence")
        .items()
        .expect("sequence items");
    assert_eq!(registrations.len(), 1);
    let registration = &registrations[0];
    assert_eq!(
        registration
            .element(tags::SOURCE_FRAME_OF_REFERENCE_UID)
            .expect("Source Frame of Reference UID")
            .to_str()
            .expect("text"),
        input.source.frame_of_reference_uid
    );
    let source_images = registration
        .element(tags::REFERENCED_IMAGE_SEQUENCE)
        .expect("Referenced Image Sequence")
        .items()
        .expect("sequence items");
    assert_eq!(source_images.len(), 1);
    assert_eq!(
        source_images[0]
            .element(tags::REFERENCED_SOP_INSTANCE_UID)
            .expect("source SOP Instance UID")
            .to_str()
            .expect("text"),
        input.source.sop_instance_uid
    );
    assert!(
        source_images[0]
            .element(tags::REFERENCED_FRAME_NUMBER)
            .is_err()
    );
    assert_eq!(
        registration
            .element(tags::REGISTRATION_TYPE_CODE_SEQUENCE)
            .expect("present Type 2 Registration Type Code Sequence")
            .items()
            .expect("sequence items")
            .len(),
        0
    );
    assert!(registration.element(tags::USED_FIDUCIALS_SEQUENCE).is_err());

    assert_identity_matrix_sequence(
        registration,
        tags::PRE_DEFORMATION_MATRIX_REGISTRATION_SEQUENCE,
    );
    assert_identity_matrix_sequence(
        registration,
        tags::POST_DEFORMATION_MATRIX_REGISTRATION_SEQUENCE,
    );
}

#[test]
fn deformable_registration_locks_grid_geometry_and_little_endian_of_payload() {
    let object =
        build_deformable_spatial_registration(locked_input()).expect("locked input should build");
    let registration = &object
        .element(tags::DEFORMABLE_REGISTRATION_SEQUENCE)
        .expect("Deformable Registration Sequence")
        .items()
        .expect("sequence items")[0];
    let grids = registration
        .element(tags::DEFORMABLE_REGISTRATION_GRID_SEQUENCE)
        .expect("Deformable Registration Grid Sequence")
        .items()
        .expect("sequence items");
    assert_eq!(grids.len(), 1);
    let grid = &grids[0];

    assert_eq!(
        grid.element(tags::IMAGE_POSITION_PATIENT)
            .expect("grid origin")
            .value()
            .to_multi_str()
            .expect("DS values")
            .as_ref(),
        ["0", "0", "2.5"]
    );
    assert_eq!(
        grid.element(tags::IMAGE_ORIENTATION_PATIENT)
            .expect("grid orientation")
            .value()
            .to_multi_str()
            .expect("DS values")
            .as_ref(),
        ["1", "0", "0", "0", "1", "0"]
    );
    let dimensions = grid
        .element(tags::GRID_DIMENSIONS)
        .expect("grid dimensions");
    assert_eq!(dimensions.vr(), VR::UL);
    assert_eq!(
        dimensions
            .to_multi_int::<u32>()
            .expect("UL values"),
        GRID_DIMENSIONS
    );
    let resolution = grid
        .element(tags::GRID_RESOLUTION)
        .expect("grid resolution");
    assert_eq!(resolution.vr(), VR::FD);
    assert_eq!(
        resolution
            .to_multi_float64()
            .expect("FD values"),
        GRID_RESOLUTION
    );

    let vector_grid = grid
        .element(tags::VECTOR_GRID_DATA)
        .expect("Vector Grid Data");
    assert_eq!(vector_grid.vr(), VR::OF);
    let DicomValue::Primitive(vector_value) = vector_grid.value() else {
        panic!("Vector Grid Data must be primitive")
    };
    assert_eq!(
        vector_value
            .to_multi_float32()
            .expect("OF values")
            .as_slice(),
        VECTOR_GRID_VALUES
    );
    assert_eq!(vector_value.to_bytes().as_ref(), VECTOR_GRID_BYTES);
}

#[test]
fn deformable_registration_partitions_registered_and_source_references() {
    let input = locked_input();
    let object = build_deformable_spatial_registration(input).expect("locked input should build");
    let registered = object
        .element(tags::REFERENCED_SERIES_SEQUENCE)
        .expect("same-study registered image references")
        .items()
        .expect("sequence items");
    assert_eq!(registered.len(), 1);
    assert_series_reference(&registered[0], input.target);

    let other_studies = object
        .element(tags::STUDIES_CONTAINING_OTHER_REFERENCED_INSTANCES_SEQUENCE)
        .expect("other-study source references")
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
    let source_series = other_studies[0]
        .element(tags::REFERENCED_SERIES_SEQUENCE)
        .expect("source series references")
        .items()
        .expect("sequence items");
    assert_eq!(source_series.len(), 1);
    assert_series_reference(&source_series[0], input.source);
}

#[test]
fn deformable_registration_rejects_a_shared_frame_of_reference() {
    let mut input = locked_input();
    input.source.frame_of_reference_uid = input.target.frame_of_reference_uid;

    let error = build_deformable_spatial_registration(input)
        .expect_err("shared Frames of Reference must be rejected");
    assert_eq!(
        error,
        "the locked CT pair must have distinct Frames of Reference"
    );
}

fn assert_identity_matrix_sequence(object: &dicom_object::InMemDicomObject, tag: dicom_core::Tag) {
    let items = object
        .element(tag)
        .expect("matrix sequence")
        .items()
        .expect("sequence items");
    assert_eq!(items.len(), 1);
    assert_eq!(
        items[0]
            .element(tags::FRAME_OF_REFERENCE_TRANSFORMATION_MATRIX_TYPE)
            .expect("matrix type")
            .to_str()
            .expect("text"),
        "RIGID"
    );
    assert_eq!(
        items[0]
            .element(tags::FRAME_OF_REFERENCE_TRANSFORMATION_MATRIX)
            .expect("matrix")
            .vr(),
        VR::DS
    );
    assert_eq!(
        items[0]
            .element(tags::FRAME_OF_REFERENCE_TRANSFORMATION_MATRIX)
            .expect("matrix")
            .value()
            .to_multi_str()
            .expect("DS values")
            .as_ref(),
        IDENTITY_MATRIX
    );
}

fn assert_series_reference(
    item: &dicom_object::InMemDicomObject,
    reference: DeformableRegistrationReference<'_>,
) {
    assert_eq!(
        item.element(tags::SERIES_INSTANCE_UID)
            .expect("Series Instance UID")
            .to_str()
            .expect("text"),
        reference.series_instance_uid
    );
    let instances = item
        .element(tags::REFERENCED_INSTANCE_SEQUENCE)
        .expect("Referenced Instance Sequence")
        .items()
        .expect("sequence items");
    assert_eq!(instances.len(), 1);
    assert_eq!(
        instances[0]
            .element(tags::REFERENCED_SOP_CLASS_UID)
            .expect("Referenced SOP Class UID")
            .to_str()
            .expect("text"),
        reference.sop_class_uid
    );
    assert_eq!(
        instances[0]
            .element(tags::REFERENCED_SOP_INSTANCE_UID)
            .expect("Referenced SOP Instance UID")
            .to_str()
            .expect("text"),
        reference.sop_instance_uid
    );
    assert!(instances[0].element(tags::REFERENCED_FRAME_NUMBER).is_err());
}
