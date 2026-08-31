use std::path::Path;

use dicom_test_suite::assembly::plan_assembly;
use dicom_test_suite::composition::CompositionUidRole;
use dicom_test_suite::corpus_plan::PlannedArtifact;

const RESOURCE_HASH: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn request() -> &'static [u8] {
    br#"{
      "assembly_request_schema_version":"1.0.0",
      "instances":[{
        "instance_id":"primary",
        "sop_class_uid":"1.2.840.10008.5.1.4.1.1.7",
        "modality":"OT",
        "output_path":"objects/custom.dcm",
        "elements":[
          {"address":{"keyword":"PatientName"},"value":{"kind":"string","value":"SYNTHETIC^ASSEMBLY"}},
          {"address":{"tag":"7776,0010"},"vr":"UL","value":{"kind":"integer","value":42}}
        ],
        "bulk":[{
          "kind":"integer_pixel_data",
          "source":{"kind":"inline_base64","base64":"AAECAw=="},
          "rows":2,"columns":2,"frames":1,"samples_per_pixel":1,
          "bits_allocated":8,"bits_stored":8,"signed":false,
          "photometric_interpretation":"MONOCHROME2"
        }]
      }]
    }"#
}

#[test]
fn assembly_plans_structural_artifacts_on_the_neutral_corpus_spine() {
    let plan = plan_assembly(request(), Path::new("."), 9, 4, RESOURCE_HASH).unwrap();
    plan.corpus.validate().unwrap();
    assert_eq!(plan.request_sha256.len(), 64);
    assert_eq!(plan.corpus.artifacts.len(), 1);
    let PlannedArtifact::Dicom(artifact) = &plan.corpus.artifacts[0] else {
        panic!("assembly must use the shared native DICOM artifact");
    };
    assert!(artifact.case_binding.is_none());
    assert_eq!(artifact.output.relative_path.as_str(), "objects/custom.dcm");
    assert_eq!(artifact.output.role, "structural_instance");
    assert_eq!(artifact.encoding.backend_id, "structural_part10");
    assert_eq!(
        artifact.validation.rules[0].rule_id,
        "structural_round_trip"
    );
    assert_eq!(artifact.instance.content[0].kind, "integer_pixel_data");
    assert_eq!(
        artifact
            .instance
            .identities
            .get(&CompositionUidRole::SopInstance, 0),
        plan.instances["primary"]
            .identities
            .get(&CompositionUidRole::SopInstance, 0)
    );
}

#[test]
fn assembly_plan_identity_and_hash_are_parallelism_independent() {
    let serial = plan_assembly(request(), Path::new("."), 9, 1, RESOURCE_HASH).unwrap();
    let parallel = plan_assembly(request(), Path::new("."), 9, 8, RESOURCE_HASH).unwrap();
    let serial_artifact = serial.instances.get("primary").unwrap();
    let parallel_artifact = parallel.instances.get("primary").unwrap();
    assert_eq!(
        serial_artifact
            .identities
            .get(&CompositionUidRole::SopInstance, 0),
        parallel_artifact
            .identities
            .get(&CompositionUidRole::SopInstance, 0)
    );
    assert_eq!(
        serial_artifact.canonical_sha256(),
        parallel_artifact.canonical_sha256()
    );
}
