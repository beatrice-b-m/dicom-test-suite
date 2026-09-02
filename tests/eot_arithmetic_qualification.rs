use std::collections::BTreeMap;
use std::fs;

use synth_dicom_gen::encapsulation::{
    CheckedEotArithmeticQualificationService, EOT_ARITHMETIC_QUALIFICATION_KIND,
    EotArithmeticExpectedError, EotArithmeticQualificationRequest, EotArithmeticStep,
    EotQualificationError, EotQualificationLimits,
};
use synth_dicom_gen::recipes::RecipeCatalog;
use serde_json::Value;

fn valid_request() -> EotArithmeticQualificationRequest {
    EotArithmeticQualificationRequest {
        fragment_lengths: vec![u64::MAX],
        arithmetic_steps: vec![
            EotArithmeticStep::PadFragmentToEven,
            EotArithmeticStep::AddItemHeader,
            EotArithmeticStep::AccumulateFrameOffset,
        ],
        expected_error: EotArithmeticExpectedError::FragmentPaddingOverflow,
        limits: EotQualificationLimits {
            max_input_bytes: 0,
            max_output_bytes: 0,
            max_operations: 3,
        },
    }
}

#[test]
fn committed_eot_inventory_executes_as_typed_payload_free_evidence() {
    let registry: Value =
        serde_json::from_slice(&fs::read("cases/registry.json").unwrap()).unwrap();
    let catalog = RecipeCatalog::load(
        "cases/recipes",
        "cases/registry.json",
        "templates/catalog.json",
    )
    .unwrap();
    let selected = registry["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|case| {
            case["status"] == "implemented" && case["provider"]["id"] == "checked_eot_arithmetic"
        })
        .collect::<Vec<_>>();
    assert!(!selected.is_empty());
    for case in selected {
        let identity = catalog
            .binding_for_case(case["case_id"].as_str().unwrap())
            .unwrap();
        let qualification = catalog.recipes()[identity].qualification.as_ref().unwrap();
        let request = EotArithmeticQualificationRequest::from_planned_parameters(
            &qualification.parameters.clone().into_iter().collect(),
            EotQualificationLimits {
                max_input_bytes: qualification.resource_policy.max_input_bytes,
                max_output_bytes: qualification.resource_policy.max_output_bytes,
                max_operations: qualification.resource_policy.max_operations,
            },
        )
        .unwrap();
        let evidence = CheckedEotArithmeticQualificationService
            .execute(request)
            .unwrap();
        assert_eq!(evidence.status, "passed");
        assert_eq!(evidence.operations_executed, 1);
        assert_eq!((evidence.input_bytes, evidence.output_bytes), (0, 0));
        assert_eq!(evidence.payload_policy, "evidence_only");
        assert_eq!(evidence.overflow_frame_index, 0);
        assert_eq!(evidence.overflow_step, EotArithmeticStep::PadFragmentToEven);
    }
}

#[test]
fn service_rejects_contract_and_resource_drift() {
    let service = CheckedEotArithmeticQualificationService;
    let mut changed_boundary = valid_request();
    changed_boundary.fragment_lengths[0] -= 1;
    assert!(matches!(
        service.execute(changed_boundary),
        Err(EotQualificationError::InvalidContract(_))
    ));
    let mut reordered = valid_request();
    reordered.arithmetic_steps.swap(0, 1);
    assert!(matches!(
        service.execute(reordered),
        Err(EotQualificationError::InvalidContract(_))
    ));
    let mut payload = valid_request();
    payload.limits.max_output_bytes = 1;
    assert!(matches!(
        service.execute(payload),
        Err(EotQualificationError::ResourceLimit(_))
    ));
    let mut operations = valid_request();
    operations.limits.max_operations = 2;
    assert!(matches!(
        service.execute(operations),
        Err(EotQualificationError::ResourceLimit(_))
    ));
}

#[test]
fn planned_parameter_adapter_rejects_unknown_and_changed_error_fields() {
    let parameters = |expected_error: &str| {
        BTreeMap::from([
            (
                "qualification_kind".into(),
                Value::String(EOT_ARITHMETIC_QUALIFICATION_KIND.into()),
            ),
            ("fragment_lengths".into(), serde_json::json!([u64::MAX])),
            (
                "arithmetic_steps".into(),
                serde_json::json!([
                    "pad_fragment_to_even",
                    "add_item_header",
                    "accumulate_frame_offset"
                ]),
            ),
            (
                "expected_error".into(),
                Value::String(expected_error.into()),
            ),
        ])
    };
    let limits = valid_request().limits;
    assert!(
        EotArithmeticQualificationRequest::from_planned_parameters(
            &parameters("different_overflow"),
            limits
        )
        .is_err()
    );
    let mut unknown = parameters("fragment_padding_overflow");
    unknown.insert("unknown".into(), Value::Bool(true));
    assert!(EotArithmeticQualificationRequest::from_planned_parameters(&unknown, limits).is_err());
}
