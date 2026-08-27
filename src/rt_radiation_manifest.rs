use serde::Serialize;

const PLAN_CASE_ID: &str = "non-image/rt/plan_linked";
const PLAN_PATH: &str = "non-image/rt/plan_linked/instance.dcm";
const PLAN_SOP_CLASS_UID: &str = "1.2.840.10008.5.1.4.1.1.481.5";
const RADIATION_CASE_ID: &str = "non-image/rt/carm_photon_electron_radiation_minimal";
const RADIATION_PATH: &str = "non-image/rt/carm_photon_electron_radiation_minimal/instance.dcm";
const RADIATION_SOP_CLASS_UID: &str = "1.2.840.10008.5.1.4.1.1.481.13";
const EQUIPMENT_FRAME_OF_REFERENCE_UID: &str = "1.2.840.10008.1.4.3.1";

#[derive(Debug, Clone, Copy)]
pub(crate) struct CArmRtRadiationInput<'a> {
    pub sop_instance_uid: &'a str,
    pub study_instance_uid: &'a str,
    pub series_instance_uid: &'a str,
    pub frame_of_reference_uid: &'a str,
    pub plan_series_instance_uid: &'a str,
    pub plan_sop_instance_uid: &'a str,
    pub plan_sha256: &'a str,
    pub software_versions: &'a str,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct RtRadiationSetInput<'a> {
    pub sop_instance_uid: &'a str,
    pub study_instance_uid: &'a str,
    pub series_instance_uid: &'a str,
    pub frame_of_reference_uid: &'a str,
    pub treatment_position_group_uid: &'a str,
    pub plan_series_instance_uid: &'a str,
    pub plan_sop_instance_uid: &'a str,
    pub plan_sha256: &'a str,
    pub radiation_series_instance_uid: &'a str,
    pub radiation_sop_instance_uid: &'a str,
    pub radiation_sha256: &'a str,
    pub software_versions: &'a str,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub(crate) struct ExpectedRtRadiation<'a> {
    pub iod_kind: &'a str,
    pub sop_class_uid: &'a str,
    pub iod_name: &'a str,
    pub modality: &'a str,
    pub transfer_syntax_uid: &'a str,
    pub sop_instance_uid: &'a str,
    pub study_instance_uid: &'a str,
    pub series_instance_uid: &'a str,
    pub frame_of_reference_uid: &'a str,
    pub instance: ExpectedRtRadiationInstance<'a>,
    pub definition_source: ExpectedRtRadiationSourceReference<'a>,
    pub content: ExpectedRtRadiationContent<'a>,
    pub device: ExpectedRtTreatmentDevice<'a>,
    pub dosimeter_unit: ExpectedRtCode<'a>,
    pub distance_reference_location: ExpectedRtCode<'a>,
    pub equipment_frame_of_reference_uid: &'a str,
    pub rt_beam_modifier_definition_distance_mm: u16,
    pub equipment_reference_point_coordinates_sequence_present_empty: bool,
    pub number_of_patient_support_devices: u8,
    pub radiation_source_axis_distance_mm: u16,
    pub treatment_positions: &'a [ExpectedRtTreatmentPosition<'a>],
    pub control_points: &'a [ExpectedRtRadiationControlPoint],
    pub absent_content: ExpectedRtRadiationAbsentContent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub(crate) struct ExpectedRtRadiationSourceReference<'a> {
    pub relationship: &'a str,
    pub source_case_id: &'a str,
    pub source_path: &'a str,
    pub source_sha256: &'a str,
    pub study_instance_uid: &'a str,
    pub series_instance_uid: &'a str,
    pub sop_class_uid: &'a str,
    pub sop_instance_uid: &'a str,
    pub frame_of_reference_uid: &'a str,
    pub referenced_beam_number: u8,
    pub common_instance_reference_ordinal: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub(crate) struct ExpectedRtRadiationInstance<'a> {
    pub series_number: u8,
    pub instance_number: u8,
    pub series_date: &'a str,
    pub series_time: &'a str,
    pub instance_creation_date: &'a str,
    pub instance_creation_time: &'a str,
    pub content_date: &'a str,
    pub content_time: &'a str,
    pub patient_name: &'a str,
    pub patient_id: &'a str,
    pub patient_birth_date: &'a str,
    pub patient_sex: &'a str,
    pub study_id: &'a str,
    pub referring_physician_name: &'a str,
    pub accession_number: &'a str,
    pub position_reference_indicator: &'a str,
    pub equipment_manufacturer: &'a str,
    pub equipment_model_name: &'a str,
    pub equipment_serial_number: &'a str,
    pub software_versions: &'a str,
    pub author_identification_sequence_present_empty: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub(crate) struct ExpectedRtRadiationContent<'a> {
    pub user_content_label: &'a str,
    pub content_description: &'a str,
    pub physical_and_geometric_content_detail_flag: &'a str,
    pub rt_record_flag: &'a str,
    pub treatment_technique: ExpectedRtCode<'a>,
    pub number_of_rt_control_points: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub(crate) struct ExpectedRtCode<'a> {
    pub code_value: &'a str,
    pub coding_scheme_designator: &'a str,
    pub code_meaning: &'a str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub(crate) struct ExpectedRtTreatmentDevice<'a> {
    pub manufacturer: &'a str,
    pub model_name: &'a str,
    pub model_version: &'a str,
    pub device_label: &'a str,
    pub serial_number: &'a str,
    pub software_versions: &'a str,
    pub manufacturer_device_identifier: &'a str,
    pub manufacturer_device_class_uid: &'a str,
    pub device_alternate_identifier: &'a str,
    pub device_type: ExpectedRtCode<'a>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub(crate) struct ExpectedRtTreatmentPosition<'a> {
    pub ordinal: u8,
    pub treatment_position_index: u8,
    pub image_to_equipment_mapping_matrix: [i8; 16],
    pub patient_location_coordinates_present_empty: bool,
    pub patient_support_position_sequence_present_empty: bool,
    pub patient_orientation: ExpectedRtCode<'a>,
    pub patient_orientation_modifier: ExpectedRtCode<'a>,
    pub patient_equipment_relationship: ExpectedRtCode<'a>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub(crate) struct ExpectedRtRadiationControlPoint {
    pub ordinal: u8,
    pub rt_control_point_index: u8,
    pub cumulative_meterset: u8,
    pub geometry: Option<ExpectedRtRadiationControlPointGeometry>,
    pub inherits_geometry_from_control_point: Option<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub(crate) struct ExpectedRtRadiationControlPointGeometry {
    pub referenced_treatment_position_index: u8,
    pub source_roll_angle_degrees: i16,
    pub rt_beam_limiting_device_angle_degrees: i16,
    pub delivery_rate_present_empty: bool,
    pub source_to_patient_surface_distance_present_empty: bool,
    pub source_to_external_contour_distance_present_empty: bool,
    pub delivery_rate_unit_sequence_absent: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub(crate) struct ExpectedRtRadiationAbsentContent {
    pub patient_study_module: bool,
    pub clinical_trial_modules: bool,
    pub referenced_performed_procedure_step_sequences: bool,
    pub treatment_session_uid: bool,
    pub treatment_machine_special_mode: bool,
    pub rt_tolerance_set: bool,
    pub treatment_time_limit: bool,
    pub device_alternate_identifier_type: bool,
    pub device_alternate_identifier_format: bool,
    pub unique_device_identifier_sequence: bool,
    pub device_manufacture_date: bool,
    pub device_expiration_date: bool,
    pub device_institution_content: bool,
    pub long_device_description: bool,
    pub patient_support_devices_sequence: bool,
    pub radiation_generation_mode: bool,
    pub beam_limiting_device_definition_and_opening: bool,
    pub wedge: bool,
    pub compensator: bool,
    pub block: bool,
    pub accessory_holder: bool,
    pub general_accessory: bool,
    pub bolus: bool,
    pub beam_area_limit: bool,
    pub recorded_control_point_attributes: bool,
    pub image: bool,
    pub pixel_data: bool,
    pub synchronization: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub(crate) struct ExpectedRtRadiationSet<'a> {
    pub iod_kind: &'a str,
    pub sop_class_uid: &'a str,
    pub iod_name: &'a str,
    pub modality: &'a str,
    pub transfer_syntax_uid: &'a str,
    pub sop_instance_uid: &'a str,
    pub study_instance_uid: &'a str,
    pub series_instance_uid: &'a str,
    pub frame_of_reference_uid: &'a str,
    pub instance: ExpectedRtRadiationInstance<'a>,
    pub content: ExpectedRtRadiationSetContent<'a>,
    pub linked_radiation_device: ExpectedRtTreatmentDevice<'a>,
    pub definition_source: ExpectedRtRadiationSetReference<'a>,
    pub radiation_references: [ExpectedRtRadiationSetReference<'a>; 1],
    pub treatment_position_groups: [ExpectedRtTreatmentPositionGroup<'a>; 1],
    pub common_instance_references: [ExpectedRtRadiationSetReference<'a>; 2],
    pub absent_content: ExpectedRtRadiationSetAbsentContent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub(crate) struct ExpectedRtRadiationSetContent<'a> {
    pub user_content_label: &'a str,
    pub content_description: &'a str,
    pub intent: &'a str,
    pub intended_number_of_fractions: u8,
    pub referenced_rt_physician_intent_sequence_present_empty: bool,
    pub author_identification_sequence_present_empty: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub(crate) struct ExpectedRtRadiationSetReference<'a> {
    pub ordinal: u8,
    pub relationship: &'a str,
    pub source_case_id: &'a str,
    pub source_path: &'a str,
    pub source_sha256: &'a str,
    pub study_instance_uid: &'a str,
    pub series_instance_uid: &'a str,
    pub sop_class_uid: &'a str,
    pub sop_instance_uid: &'a str,
    pub frame_of_reference_uid: &'a str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub(crate) struct ExpectedRtTreatmentPositionGroup<'a> {
    pub ordinal: u8,
    pub treatment_position_group_uid: &'a str,
    pub label: &'a str,
    pub radiation_references: [ExpectedRtRadiationSetReference<'a>; 1],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub(crate) struct ExpectedRtRadiationSetAbsentContent {
    pub patient_study_module: bool,
    pub clinical_trial_modules: bool,
    pub referenced_performed_procedure_step_sequences: bool,
    pub treatment_session_uid: bool,
    pub synchronization: bool,
    pub rt_dose_contribution_module: bool,
    pub fraction_pattern_sequence: bool,
    pub image: bool,
    pub pixel_data: bool,
}

const IDENTITY_MATRIX: [i8; 16] = [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1];

const TREATMENT_POSITIONS: [ExpectedRtTreatmentPosition<'static>; 1] =
    [ExpectedRtTreatmentPosition {
        ordinal: 1,
        treatment_position_index: 1,
        image_to_equipment_mapping_matrix: IDENTITY_MATRIX,
        patient_location_coordinates_present_empty: true,
        patient_support_position_sequence_present_empty: true,
        patient_orientation: ExpectedRtCode {
            code_value: "102538003",
            coding_scheme_designator: "SCT",
            code_meaning: "recumbent",
        },
        patient_orientation_modifier: ExpectedRtCode {
            code_value: "40199007",
            coding_scheme_designator: "SCT",
            code_meaning: "supine",
        },
        patient_equipment_relationship: ExpectedRtCode {
            code_value: "102540008",
            coding_scheme_designator: "SCT",
            code_meaning: "headfirst",
        },
    }];

const CONTROL_POINTS: [ExpectedRtRadiationControlPoint; 2] = [
    ExpectedRtRadiationControlPoint {
        ordinal: 1,
        rt_control_point_index: 1,
        cumulative_meterset: 0,
        geometry: Some(ExpectedRtRadiationControlPointGeometry {
            referenced_treatment_position_index: 1,
            source_roll_angle_degrees: 0,
            rt_beam_limiting_device_angle_degrees: 0,
            delivery_rate_present_empty: true,
            source_to_patient_surface_distance_present_empty: true,
            source_to_external_contour_distance_present_empty: true,
            delivery_rate_unit_sequence_absent: true,
        }),
        inherits_geometry_from_control_point: None,
    },
    ExpectedRtRadiationControlPoint {
        ordinal: 2,
        rt_control_point_index: 2,
        cumulative_meterset: 100,
        geometry: None,
        inherits_geometry_from_control_point: Some(1),
    },
];

fn treatment_device<'a>(software_versions: &'a str) -> ExpectedRtTreatmentDevice<'a> {
    ExpectedRtTreatmentDevice {
        manufacturer: "dicom-test-suite",
        model_name: "DTS C-Arm LINAC",
        model_version: "1",
        device_label: "DTS_LINAC",
        serial_number: "DTS-LINAC-001",
        software_versions,
        manufacturer_device_identifier: "DTS-LINAC-001",
        manufacturer_device_class_uid: "",
        device_alternate_identifier: "",
        device_type: ExpectedRtCode {
            code_value: "130361",
            coding_scheme_designator: "DCM",
            code_meaning: "Radiotherapy Treatment Device",
        },
    }
}

fn instance_context<'a>(
    series_number: u8,
    equipment_model_name: &'a str,
    software_versions: &'a str,
) -> ExpectedRtRadiationInstance<'a> {
    ExpectedRtRadiationInstance {
        series_number,
        instance_number: 1,
        series_date: "20260101",
        series_time: "000000",
        instance_creation_date: "20260101",
        instance_creation_time: "000000",
        content_date: "20260101",
        content_time: "000000",
        patient_name: "DTS^Synthetic^Patient001",
        patient_id: "DTS-PATIENT-001",
        patient_birth_date: "19700101",
        patient_sex: "O",
        study_id: "DTS-RTSTRUCT",
        referring_physician_name: "",
        accession_number: "",
        position_reference_indicator: "",
        equipment_manufacturer: "dicom-test-suite",
        equipment_model_name,
        equipment_serial_number: "DTS-LINAC-001",
        software_versions,
        author_identification_sequence_present_empty: true,
    }
}

pub(crate) fn minimal_carm_rt_radiation_expected(
    input: CArmRtRadiationInput<'_>,
) -> ExpectedRtRadiation<'_> {
    ExpectedRtRadiation {
        iod_kind: "carm_photon_electron_radiation",
        sop_class_uid: RADIATION_SOP_CLASS_UID,
        iod_name: "C-Arm Photon-Electron Radiation",
        modality: "RTRAD",
        transfer_syntax_uid: "1.2.840.10008.1.2.1",
        sop_instance_uid: input.sop_instance_uid,
        study_instance_uid: input.study_instance_uid,
        series_instance_uid: input.series_instance_uid,
        frame_of_reference_uid: input.frame_of_reference_uid,
        instance: instance_context(
            74,
            "Native C-Arm Photon-Electron Radiation",
            input.software_versions,
        ),
        definition_source: ExpectedRtRadiationSourceReference {
            relationship: "definition_source",
            source_case_id: PLAN_CASE_ID,
            source_path: PLAN_PATH,
            source_sha256: input.plan_sha256,
            study_instance_uid: input.study_instance_uid,
            series_instance_uid: input.plan_series_instance_uid,
            sop_class_uid: PLAN_SOP_CLASS_UID,
            sop_instance_uid: input.plan_sop_instance_uid,
            frame_of_reference_uid: input.frame_of_reference_uid,
            referenced_beam_number: 1,
            common_instance_reference_ordinal: 1,
        },
        content: ExpectedRtRadiationContent {
            user_content_label: "DTS_RADIATION",
            content_description: "",
            physical_and_geometric_content_detail_flag: "IDENT_ONLY",
            rt_record_flag: "NO",
            treatment_technique: ExpectedRtCode {
                code_value: "130102",
                coding_scheme_designator: "DCM",
                code_meaning: "Static Beam",
            },
            number_of_rt_control_points: 2,
        },
        device: treatment_device(input.software_versions),
        dosimeter_unit: ExpectedRtCode {
            code_value: "{MU}",
            coding_scheme_designator: "UCUM",
            code_meaning: "Monitor Units",
        },
        distance_reference_location: ExpectedRtCode {
            code_value: "130358",
            coding_scheme_designator: "DCM",
            code_meaning: "Nominal Radiation Source Location",
        },
        equipment_frame_of_reference_uid: EQUIPMENT_FRAME_OF_REFERENCE_UID,
        rt_beam_modifier_definition_distance_mm: 500,
        equipment_reference_point_coordinates_sequence_present_empty: true,
        number_of_patient_support_devices: 0,
        radiation_source_axis_distance_mm: 1_000,
        treatment_positions: &TREATMENT_POSITIONS,
        control_points: &CONTROL_POINTS,
        absent_content: ExpectedRtRadiationAbsentContent {
            patient_study_module: true,
            clinical_trial_modules: true,
            referenced_performed_procedure_step_sequences: true,
            treatment_session_uid: true,
            treatment_machine_special_mode: true,
            rt_tolerance_set: true,
            treatment_time_limit: true,
            device_alternate_identifier_type: true,
            device_alternate_identifier_format: true,
            unique_device_identifier_sequence: true,
            device_manufacture_date: true,
            device_expiration_date: true,
            device_institution_content: true,
            long_device_description: true,
            patient_support_devices_sequence: true,
            radiation_generation_mode: true,
            beam_limiting_device_definition_and_opening: true,
            wedge: true,
            compensator: true,
            block: true,
            accessory_holder: true,
            general_accessory: true,
            bolus: true,
            beam_area_limit: true,
            recorded_control_point_attributes: true,
            image: true,
            pixel_data: true,
            synchronization: true,
        },
    }
}

pub(crate) fn minimal_rt_radiation_set_expected(
    input: RtRadiationSetInput<'_>,
) -> ExpectedRtRadiationSet<'_> {
    let plan_reference = ExpectedRtRadiationSetReference {
        ordinal: 1,
        relationship: "definition_source",
        source_case_id: PLAN_CASE_ID,
        source_path: PLAN_PATH,
        source_sha256: input.plan_sha256,
        study_instance_uid: input.study_instance_uid,
        series_instance_uid: input.plan_series_instance_uid,
        sop_class_uid: PLAN_SOP_CLASS_UID,
        sop_instance_uid: input.plan_sop_instance_uid,
        frame_of_reference_uid: input.frame_of_reference_uid,
    };
    let radiation_reference = ExpectedRtRadiationSetReference {
        ordinal: 1,
        relationship: "referenced_rt_radiation",
        source_case_id: RADIATION_CASE_ID,
        source_path: RADIATION_PATH,
        source_sha256: input.radiation_sha256,
        study_instance_uid: input.study_instance_uid,
        series_instance_uid: input.radiation_series_instance_uid,
        sop_class_uid: RADIATION_SOP_CLASS_UID,
        sop_instance_uid: input.radiation_sop_instance_uid,
        frame_of_reference_uid: input.frame_of_reference_uid,
    };

    let radiation_references = [radiation_reference];
    let treatment_position_groups = [ExpectedRtTreatmentPositionGroup {
        ordinal: 1,
        treatment_position_group_uid: input.treatment_position_group_uid,
        label: "DTS_TPG_1",
        radiation_references: [radiation_reference],
    }];
    let common_instance_references = [
        plan_reference,
        ExpectedRtRadiationSetReference {
            ordinal: 2,
            ..radiation_reference
        },
    ];

    ExpectedRtRadiationSet {
        iod_kind: "rt_radiation_set",
        sop_class_uid: "1.2.840.10008.5.1.4.1.1.481.12",
        iod_name: "RT Radiation Set",
        modality: "RTRAD",
        transfer_syntax_uid: "1.2.840.10008.1.2.1",
        sop_instance_uid: input.sop_instance_uid,
        study_instance_uid: input.study_instance_uid,
        series_instance_uid: input.series_instance_uid,
        frame_of_reference_uid: input.frame_of_reference_uid,
        instance: instance_context(75, "Native RT Radiation Set", input.software_versions),
        content: ExpectedRtRadiationSetContent {
            user_content_label: "DTS_RADSET",
            content_description: "",
            intent: "TREATMENT",
            intended_number_of_fractions: 1,
            referenced_rt_physician_intent_sequence_present_empty: true,
            author_identification_sequence_present_empty: true,
        },
        linked_radiation_device: treatment_device(input.software_versions),
        definition_source: plan_reference,
        radiation_references,
        treatment_position_groups,
        common_instance_references,
        absent_content: ExpectedRtRadiationSetAbsentContent {
            patient_study_module: true,
            clinical_trial_modules: true,
            referenced_performed_procedure_step_sequences: true,
            treatment_session_uid: true,
            synchronization: true,
            rt_dose_contribution_module: true,
            fraction_pattern_sequence: true,
            image: true,
            pixel_data: true,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const STUDY_UID: &str = "1.2.826.0.1.3680043.10.543.1";
    const FRAME_UID: &str = "1.2.826.0.1.3680043.10.543.2";
    const PLAN_SERIES_UID: &str = "1.2.826.0.1.3680043.10.543.3";
    const PLAN_SOP_UID: &str = "1.2.826.0.1.3680043.10.543.4";
    const RADIATION_SERIES_UID: &str = "1.2.826.0.1.3680043.10.543.5";
    const RADIATION_SOP_UID: &str = "1.2.826.0.1.3680043.10.543.6";

    #[test]
    fn radiation_contract_locks_plan_device_codes_and_control_point_inheritance() {
        let expected = minimal_carm_rt_radiation_expected(CArmRtRadiationInput {
            sop_instance_uid: RADIATION_SOP_UID,
            study_instance_uid: STUDY_UID,
            series_instance_uid: RADIATION_SERIES_UID,
            frame_of_reference_uid: FRAME_UID,
            plan_series_instance_uid: PLAN_SERIES_UID,
            plan_sop_instance_uid: PLAN_SOP_UID,
            plan_sha256: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            software_versions: "0.1.0",
        });

        assert_eq!(expected.definition_source.sop_instance_uid, PLAN_SOP_UID);
        assert_eq!(expected.definition_source.referenced_beam_number, 1);
        assert_eq!(expected.device.serial_number, "DTS-LINAC-001");
        assert_eq!(expected.content.treatment_technique.code_value, "130102");
        assert_eq!(expected.treatment_positions.len(), 1);
        assert_eq!(expected.control_points.len(), 2);
        assert!(expected.control_points[0].geometry.is_some());
        assert_eq!(
            expected.control_points[1].inherits_geometry_from_control_point,
            Some(1)
        );
        assert!(
            expected
                .absent_content
                .beam_limiting_device_definition_and_opening
        );
    }

    #[test]
    fn radiation_set_contract_repeats_one_radiation_in_every_reference_projection() {
        let expected = minimal_rt_radiation_set_expected(RtRadiationSetInput {
            sop_instance_uid: "1.2.826.0.1.3680043.10.543.8",
            study_instance_uid: STUDY_UID,
            series_instance_uid: "1.2.826.0.1.3680043.10.543.7",
            frame_of_reference_uid: FRAME_UID,
            treatment_position_group_uid: "1.2.826.0.1.3680043.10.543.9",
            plan_series_instance_uid: PLAN_SERIES_UID,
            plan_sop_instance_uid: PLAN_SOP_UID,
            plan_sha256: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            radiation_series_instance_uid: RADIATION_SERIES_UID,
            radiation_sop_instance_uid: RADIATION_SOP_UID,
            radiation_sha256: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            software_versions: "0.1.0",
        });

        assert_eq!(expected.radiation_references.len(), 1);
        assert_eq!(expected.treatment_position_groups.len(), 1);
        assert_eq!(
            expected.treatment_position_groups[0].radiation_references,
            expected.radiation_references
        );
        assert_eq!(expected.common_instance_references.len(), 2);
        assert_eq!(
            expected.common_instance_references[1].sop_instance_uid,
            RADIATION_SOP_UID
        );
        assert_eq!(expected.definition_source.sop_instance_uid, PLAN_SOP_UID);
        assert!(expected.absent_content.rt_dose_contribution_module);
    }
}
