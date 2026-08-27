use serde::Serialize;

const STRUCTURE_SET_CASE_ID: &str = "non-image/rt/structure_set_single_roi_explicit_le";
const STRUCTURE_SET_PATH: &str = "non-image/rt/structure_set_single_roi_explicit_le/instance.dcm";
const STRUCTURE_SET_SOP_CLASS_UID: &str = "1.2.840.10008.5.1.4.1.1.481.3";
const DOSE_CASE_ID: &str = "non-image/rt/dose_grid_u16_explicit_le";
const DOSE_PATH: &str = "non-image/rt/dose_grid_u16_explicit_le/instance.dcm";
const DOSE_SOP_CLASS_UID: &str = "1.2.840.10008.5.1.4.1.1.481.2";

#[derive(Debug, Clone, Copy)]
pub(crate) struct LinkedRtPlanInput<'a> {
    pub sop_instance_uid: &'a str,
    pub study_instance_uid: &'a str,
    pub series_instance_uid: &'a str,
    pub frame_of_reference_uid: &'a str,
    pub structure_set_series_instance_uid: &'a str,
    pub structure_set_sop_instance_uid: &'a str,
    pub structure_set_sha256: &'a str,
    pub dose_series_instance_uid: &'a str,
    pub dose_sop_instance_uid: &'a str,
    pub dose_sha256: &'a str,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub(crate) struct ExpectedRtPlan<'a> {
    pub iod_kind: &'a str,
    pub sop_class_uid: &'a str,
    pub iod_name: &'a str,
    pub modality: &'a str,
    pub transfer_syntax_uid: &'a str,
    pub sop_instance_uid: &'a str,
    pub study_instance_uid: &'a str,
    pub series_instance_uid: &'a str,
    pub frame_of_reference_uid: &'a str,
    pub references: [ExpectedRtPlanReference<'a>; 2],
    pub plan: ExpectedRtPlanIdentity<'a>,
    pub fraction_groups: &'a [ExpectedRtFractionGroup<'a>],
    pub beams: &'a [ExpectedRtBeam<'a>],
    pub absent_content: ExpectedRtPlanAbsentContent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub(crate) struct ExpectedRtPlanReference<'a> {
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
pub(crate) struct ExpectedRtPlanIdentity<'a> {
    pub label: &'a str,
    pub date: &'a str,
    pub time: &'a str,
    pub geometry: &'a str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub(crate) struct ExpectedRtFractionGroup<'a> {
    pub ordinal: u8,
    pub fraction_group_number: u8,
    pub number_of_fractions_planned: u8,
    pub number_of_beams: u8,
    pub number_of_brachy_application_setups: u8,
    pub referenced_beams: &'a [ExpectedRtReferencedBeam],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub(crate) struct ExpectedRtReferencedBeam {
    pub ordinal: u8,
    pub referenced_beam_number: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub(crate) struct ExpectedRtBeam<'a> {
    pub ordinal: u8,
    pub treatment_machine_name: &'a str,
    pub primary_dosimeter_unit: &'a str,
    pub source_axis_distance_mm: u16,
    pub beam_number: u8,
    pub beam_name: &'a str,
    pub beam_type: &'a str,
    pub radiation_type: &'a str,
    pub treatment_delivery_type: &'a str,
    pub accessories: ExpectedRtBeamAccessories,
    pub beam_limiting_devices: &'a [ExpectedRtBeamLimitingDevice<'a>],
    pub number_of_control_points: u8,
    pub final_cumulative_meterset_weight: u8,
    pub control_points: &'a [ExpectedRtControlPoint],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub(crate) struct ExpectedRtBeamAccessories {
    pub number_of_wedges: u8,
    pub wedge_sequence_absent: bool,
    pub number_of_compensators: u8,
    pub compensator_sequence_absent: bool,
    pub number_of_boli: u8,
    pub bolus_sequence_absent: bool,
    pub number_of_blocks: u8,
    pub block_sequence_absent: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub(crate) struct ExpectedRtBeamLimitingDevice<'a> {
    pub ordinal: u8,
    pub device_type: &'a str,
    pub number_of_leaf_jaw_pairs: u8,
    pub source_to_device_distance_mm: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub(crate) struct ExpectedRtControlPoint {
    pub ordinal: u8,
    pub control_point_index: u8,
    pub cumulative_meterset_weight: u8,
    pub geometry: Option<ExpectedRtControlPointGeometry>,
    pub inherits_geometry_from_control_point: Option<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub(crate) struct ExpectedRtControlPointGeometry {
    pub nominal_beam_energy_mev: u8,
    pub jaw_positions_mm: [[i16; 2]; 2],
    pub gantry_angle_degrees: u16,
    pub gantry_rotation_direction: &'static str,
    pub beam_limiting_device_angle_degrees: u16,
    pub beam_limiting_device_rotation_direction: &'static str,
    pub patient_support_angle_degrees: u16,
    pub patient_support_rotation_direction: &'static str,
    pub table_top_vertical_position_mm: i16,
    pub table_top_longitudinal_position_mm: i16,
    pub table_top_lateral_position_mm: i16,
    pub table_top_pitch_angle_degrees: i16,
    pub table_top_pitch_rotation_direction: &'static str,
    pub table_top_roll_angle_degrees: i16,
    pub table_top_roll_rotation_direction: &'static str,
    pub isocenter_position_mm: [i16; 3],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub(crate) struct ExpectedRtPlanAbsentContent {
    pub referenced_rt_plan_sequence: bool,
    pub rt_prescription_module: bool,
    pub rt_tolerance_tables_module: bool,
    pub rt_patient_setup_module: bool,
    pub rt_brachy_application_setups_module: bool,
    pub approval_module: bool,
    pub clinical_trial_module: bool,
    pub common_instance_reference_module: bool,
    pub image: bool,
    pub pixel_data: bool,
}

const REFERENCED_BEAMS: [ExpectedRtReferencedBeam; 1] = [ExpectedRtReferencedBeam {
    ordinal: 1,
    referenced_beam_number: 1,
}];

const FRACTION_GROUPS: [ExpectedRtFractionGroup<'static>; 1] = [ExpectedRtFractionGroup {
    ordinal: 1,
    fraction_group_number: 1,
    number_of_fractions_planned: 1,
    number_of_beams: 1,
    number_of_brachy_application_setups: 0,
    referenced_beams: &REFERENCED_BEAMS,
}];

const DEVICES: [ExpectedRtBeamLimitingDevice<'static>; 2] = [
    ExpectedRtBeamLimitingDevice {
        ordinal: 1,
        device_type: "X",
        number_of_leaf_jaw_pairs: 1,
        source_to_device_distance_mm: 500,
    },
    ExpectedRtBeamLimitingDevice {
        ordinal: 2,
        device_type: "Y",
        number_of_leaf_jaw_pairs: 1,
        source_to_device_distance_mm: 500,
    },
];

const CONTROL_POINTS: [ExpectedRtControlPoint; 2] = [
    ExpectedRtControlPoint {
        ordinal: 1,
        control_point_index: 0,
        cumulative_meterset_weight: 0,
        geometry: Some(ExpectedRtControlPointGeometry {
            nominal_beam_energy_mev: 6,
            jaw_positions_mm: [[-50, 50], [-50, 50]],
            gantry_angle_degrees: 0,
            gantry_rotation_direction: "NONE",
            beam_limiting_device_angle_degrees: 0,
            beam_limiting_device_rotation_direction: "NONE",
            patient_support_angle_degrees: 0,
            patient_support_rotation_direction: "NONE",
            table_top_vertical_position_mm: 0,
            table_top_longitudinal_position_mm: 0,
            table_top_lateral_position_mm: 0,
            table_top_pitch_angle_degrees: 0,
            table_top_pitch_rotation_direction: "NONE",
            table_top_roll_angle_degrees: 0,
            table_top_roll_rotation_direction: "NONE",
            isocenter_position_mm: [0, 0, 0],
        }),
        inherits_geometry_from_control_point: None,
    },
    ExpectedRtControlPoint {
        ordinal: 2,
        control_point_index: 1,
        cumulative_meterset_weight: 1,
        geometry: None,
        inherits_geometry_from_control_point: Some(0),
    },
];

const BEAMS: [ExpectedRtBeam<'static>; 1] = [ExpectedRtBeam {
    ordinal: 1,
    treatment_machine_name: "DTS_LINAC",
    primary_dosimeter_unit: "MU",
    source_axis_distance_mm: 1_000,
    beam_number: 1,
    beam_name: "DTS_STATIC_AP",
    beam_type: "STATIC",
    radiation_type: "PHOTON",
    treatment_delivery_type: "TREATMENT",
    accessories: ExpectedRtBeamAccessories {
        number_of_wedges: 0,
        wedge_sequence_absent: true,
        number_of_compensators: 0,
        compensator_sequence_absent: true,
        number_of_boli: 0,
        bolus_sequence_absent: true,
        number_of_blocks: 0,
        block_sequence_absent: true,
    },
    beam_limiting_devices: &DEVICES,
    number_of_control_points: 2,
    final_cumulative_meterset_weight: 1,
    control_points: &CONTROL_POINTS,
}];

pub(crate) fn linked_rt_plan_expected(input: LinkedRtPlanInput<'_>) -> ExpectedRtPlan<'_> {
    ExpectedRtPlan {
        iod_kind: "rt_plan",
        sop_class_uid: "1.2.840.10008.5.1.4.1.1.481.5",
        iod_name: "RT Plan",
        modality: "RTPLAN",
        transfer_syntax_uid: "1.2.840.10008.1.2.1",
        sop_instance_uid: input.sop_instance_uid,
        study_instance_uid: input.study_instance_uid,
        series_instance_uid: input.series_instance_uid,
        frame_of_reference_uid: input.frame_of_reference_uid,
        references: [
            ExpectedRtPlanReference {
                ordinal: 1,
                relationship: "referenced_structure_set",
                source_case_id: STRUCTURE_SET_CASE_ID,
                source_path: STRUCTURE_SET_PATH,
                source_sha256: input.structure_set_sha256,
                study_instance_uid: input.study_instance_uid,
                series_instance_uid: input.structure_set_series_instance_uid,
                sop_class_uid: STRUCTURE_SET_SOP_CLASS_UID,
                sop_instance_uid: input.structure_set_sop_instance_uid,
                frame_of_reference_uid: input.frame_of_reference_uid,
            },
            ExpectedRtPlanReference {
                ordinal: 2,
                relationship: "referenced_dose",
                source_case_id: DOSE_CASE_ID,
                source_path: DOSE_PATH,
                source_sha256: input.dose_sha256,
                study_instance_uid: input.study_instance_uid,
                series_instance_uid: input.dose_series_instance_uid,
                sop_class_uid: DOSE_SOP_CLASS_UID,
                sop_instance_uid: input.dose_sop_instance_uid,
                frame_of_reference_uid: input.frame_of_reference_uid,
            },
        ],
        plan: ExpectedRtPlanIdentity {
            label: "DTS_PLAN",
            date: "20260101",
            time: "000000",
            geometry: "PATIENT",
        },
        fraction_groups: &FRACTION_GROUPS,
        beams: &BEAMS,
        absent_content: ExpectedRtPlanAbsentContent {
            referenced_rt_plan_sequence: true,
            rt_prescription_module: true,
            rt_tolerance_tables_module: true,
            rt_patient_setup_module: true,
            rt_brachy_application_setups_module: true,
            approval_module: true,
            clinical_trial_module: true,
            common_instance_reference_module: true,
            image: true,
            pixel_data: true,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linked_plan_serializes_exact_order_and_inheritance() {
        let value = serde_json::to_value(linked_rt_plan_expected(LinkedRtPlanInput {
            sop_instance_uid: "2.25.1",
            study_instance_uid: "2.25.2",
            series_instance_uid: "2.25.3",
            frame_of_reference_uid: "2.25.4",
            structure_set_series_instance_uid: "2.25.5",
            structure_set_sop_instance_uid: "2.25.6",
            structure_set_sha256: &"a".repeat(64),
            dose_series_instance_uid: "2.25.7",
            dose_sop_instance_uid: "2.25.8",
            dose_sha256: &"b".repeat(64),
        }))
        .expect("RT Plan expectation should serialize");

        assert_eq!(
            value["references"][0]["relationship"],
            "referenced_structure_set"
        );
        assert_eq!(value["references"][1]["relationship"], "referenced_dose");
        assert_eq!(
            value["beams"][0]["beam_limiting_devices"][0]["device_type"],
            "X"
        );
        assert_eq!(
            value["beams"][0]["beam_limiting_devices"][1]["device_type"],
            "Y"
        );
        assert_eq!(
            value["beams"][0]["control_points"][0]["geometry"]["jaw_positions_mm"],
            serde_json::json!([[-50, 50], [-50, 50]])
        );
        assert!(value["beams"][0]["control_points"][1]["geometry"].is_null());
        assert_eq!(
            value["beams"][0]["control_points"][1]["inherits_geometry_from_control_point"],
            0
        );
    }
}
