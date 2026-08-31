use dicom_test_suite::assembly::{AssemblyError, AssemblyRequest, AssemblyValue};

#[test]
fn assembly_request_types_standard_unknown_private_sequence_and_bulk() {
    let request = AssemblyRequest::from_slice(
        br#"{
          "assembly_request_schema_version":"1.0.0",
          "instances":[{
            "instance_id":"primary",
            "sop_class_uid":"1.2.840.10008.5.1.4.1.1.7",
            "elements":[
              {"address":{"keyword":"PatientName"},"value":{"kind":"string","value":"SYNTHETIC^ASSEMBLY"}},
              {"address":{"tag":"7776,0010"},"vr":"LO","value":{"kind":"string","value":"unknown standard"}},
              {"address":{"private_group":"0011","private_creator":"DTS_ASSEMBLY","private_offset":"10"},"vr":"OB","value":{"kind":"bytes","base64":"AAECAw=="}},
              {"address":{"keyword":"ReferencedImageSequence"},"value":{"kind":"sequence","items":[
                {"elements":[{"address":{"keyword":"ReferencedSOPClassUID"},"value":{"kind":"string","value":"1.2.840.10008.5.1.4.1.1.7"}}]}
              ]}}
            ],
            "bulk":[{"kind":"general","tag":"7776,1000","vr":"OB","source":{"kind":"inline_base64","base64":"AQIDBA=="}}]
          }]
        }"#,
    )
    .unwrap();

    assert_eq!(request.instances.len(), 1);
    assert!(matches!(
        request.instances[0].elements[3].value,
        AssemblyValue::Sequence { .. }
    ));
}

#[test]
fn assembly_request_rejects_protected_identity_elements() {
    let error = AssemblyRequest::from_slice(
        br#"{"assembly_request_schema_version":"1.0.0","instances":[{
          "instance_id":"primary","sop_class_uid":"1.2.3",
          "elements":[{"address":{"keyword":"SOPInstanceUID"},"value":{"kind":"string","value":"1.2.3.4"}}]
        }]}"#,
    )
    .unwrap_err();
    assert!(matches!(error, AssemblyError::ProtectedElement(_)));
}

#[test]
fn assembly_request_rejects_unsafe_assets_and_unknown_vr_inference() {
    let traversal = AssemblyRequest::from_slice(
        br#"{"assembly_request_schema_version":"1.0.0","instances":[{
          "instance_id":"primary","sop_class_uid":"1.2.3","elements":[],
          "bulk":[{"kind":"general","tag":"7776,1000","vr":"OB","source":{"kind":"file","path":"../secret","sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}}]
        }]}"#,
    )
    .unwrap_err();
    assert!(matches!(traversal, AssemblyError::Schema(_)));

    let unknown = AssemblyRequest::from_slice(
        br#"{"assembly_request_schema_version":"1.0.0","instances":[{
          "instance_id":"primary","sop_class_uid":"1.2.3",
          "elements":[{"address":{"tag":"7776,0010"},"value":{"kind":"empty"}}]
        }]}"#,
    )
    .unwrap_err();
    assert!(matches!(unknown, AssemblyError::VrRequired(_)));
}

#[test]
fn assembly_request_rejects_duplicate_instances_and_missing_references() {
    let duplicate = AssemblyRequest::from_slice(
        br#"{"assembly_request_schema_version":"1.0.0","instances":[
          {"instance_id":"same","sop_class_uid":"1.2.3","elements":[]},
          {"instance_id":"same","sop_class_uid":"1.2.4","elements":[]}
        ]}"#,
    )
    .unwrap_err();
    assert!(matches!(duplicate, AssemblyError::DuplicateInstance(_)));

    let missing = AssemblyRequest::from_slice(
        br#"{"assembly_request_schema_version":"1.0.0","instances":[{
          "instance_id":"primary","sop_class_uid":"1.2.3","elements":[],
          "references":[{"relationship":"source","target_instance_id":"absent","target_role":"sop"}]
        }]}"#,
    )
    .unwrap_err();
    assert!(matches!(missing, AssemblyError::MissingReference(_)));
}
