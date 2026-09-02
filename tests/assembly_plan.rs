use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use synth_dicom_gen::assembly::plan_assembly;
use synth_dicom_gen::composition::CompositionUidRole;
use synth_dicom_gen::corpus_plan::PlannedArtifact;

const RESOURCE_HASH: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
static NEXT: AtomicU64 = AtomicU64::new(0);

fn temp_root(label: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "dicom-test-suite-assembly-plan-{label}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ))
}

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
    assert_eq!(
        serial.corpus.canonical_sha256().unwrap(),
        parallel.corpus.canonical_sha256().unwrap()
    );
}

#[test]
fn assembly_uid_namespace_is_independent_of_transitional_resource_membership() {
    let baseline = plan_assembly(request(), Path::new("."), 9, 1, RESOURCE_HASH).unwrap();
    let corpus_changed = plan_assembly(request(), Path::new("."), 9, 1, &"f".repeat(64)).unwrap();

    assert_eq!(baseline.instances, corpus_changed.instances);
    assert_eq!(
        baseline.corpus.canonical_sha256().unwrap(),
        corpus_changed.corpus.canonical_sha256().unwrap()
    );
}

#[test]
fn assembly_plans_each_typed_bulk_adapter_with_hash_provenance() {
    let request = br#"{
      "assembly_request_schema_version":"1.0.0",
      "instances":[
        {"instance_id":"integer","sop_class_uid":"1.2.3.1","elements":[],"bulk":[{"kind":"integer_pixel_data","source":{"kind":"inline_base64","base64":"AAECAw=="},"rows":2,"columns":2,"bits_allocated":8,"bits_stored":8}]},
        {"instance_id":"float","sop_class_uid":"1.2.3.2","elements":[],"bulk":[{"kind":"float_pixel_data","source":{"kind":"inline_base64","base64":"AACAPw=="},"rows":1,"columns":1}]},
        {"instance_id":"double","sop_class_uid":"1.2.3.3","elements":[],"bulk":[{"kind":"double_float_pixel_data","source":{"kind":"inline_base64","base64":"AAAAAAAA8D8="},"rows":1,"columns":1}]},
        {"instance_id":"waveform","sop_class_uid":"1.2.3.4","elements":[],"bulk":[{"kind":"waveform_data","source":{"kind":"inline_base64","base64":"AAAAAAAAAAA="},"channels":2,"samples":2,"bits_allocated":16}]},
        {"instance_id":"document","sop_class_uid":"1.2.3.5","elements":[],"bulk":[{"kind":"encapsulated_document","source":{"kind":"inline_base64","base64":"JVBERi0xLjQ="},"media_type":"application/pdf"}]},
        {"instance_id":"mesh","sop_class_uid":"1.2.3.6","elements":[],"bulk":[{"kind":"mesh","source":{"kind":"inline_base64","base64":"AAAAAAAAAAAAAAAA"}}]},
        {"instance_id":"general","sop_class_uid":"1.2.3.7","elements":[],"bulk":[{"kind":"general","tag":"7776,1000","vr":"OB","source":{"kind":"inline_base64","base64":"AQID"}}]}
      ]
    }"#;
    let plan = plan_assembly(request, Path::new("."), 1, 2, RESOURCE_HASH).unwrap();
    let kinds = plan
        .instances
        .values()
        .map(|instance| instance.content[0].kind.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(kinds.len(), 7);
    for instance in plan.instances.values() {
        let content = &instance.content[0];
        assert_eq!(content.properties["source_kind"], "inline_base64");
        assert_eq!(
            content.properties["source_sha256"],
            content.properties["resolved_sha256"]
        );
        assert_eq!(content.properties["iod_conformance"], "not_assessed");
    }
}

#[test]
fn assembly_rejects_typed_bulk_shape_mismatch_before_publication() {
    let request = br#"{
      "assembly_request_schema_version":"1.0.0",
      "instances":[{"instance_id":"bad","sop_class_uid":"1.2.3","elements":[],"bulk":[{
        "kind":"integer_pixel_data","source":{"kind":"inline_base64","base64":"AA=="},
        "rows":2,"columns":2,"bits_allocated":8,"bits_stored":8
      }]}]
    }"#;
    let error = plan_assembly(request, Path::new("."), 1, 1, RESOURCE_HASH).unwrap_err();
    assert!(error.to_string().contains("bulk length mismatch"));
}

#[test]
fn assembly_rejects_invalid_document_signatures_before_publication() {
    let request = br#"{
      "assembly_request_schema_version":"1.0.0",
      "instances":[{"instance_id":"bad","sop_class_uid":"1.2.3","elements":[],"bulk":[{
        "kind":"encapsulated_document","source":{"kind":"inline_base64","base64":"bm90IGEgcGRm"},
        "media_type":"application/pdf"
      }]}]
    }"#;
    let error = plan_assembly(request, Path::new("."), 1, 1, RESOURCE_HASH).unwrap_err();
    assert!(error.to_string().contains("invalid signature"));
}

#[cfg(unix)]
#[test]
fn assembly_rejects_symlinks_in_any_caller_asset_path_component() {
    use std::os::unix::fs::symlink;

    let root = temp_root("symlink");
    let outside = temp_root("outside");
    fs::create_dir_all(&root).unwrap();
    fs::create_dir_all(&outside).unwrap();
    fs::write(outside.join("payload.bin"), [0_u8, 1, 2, 3]).unwrap();
    symlink(&outside, root.join("linked")).unwrap();
    let request = br#"{
      "assembly_request_schema_version":"1.0.0",
      "instances":[{"instance_id":"bad","sop_class_uid":"1.2.3","elements":[],"bulk":[{
        "kind":"general","tag":"7776,1000","vr":"OB",
        "source":{"kind":"file","path":"linked/payload.bin","sha256":"054edec1d0211f624fed0cbca9d4f9400b0e491c43742af2c5b0abebf0c990d8"}
      }]}]
    }"#;
    let error = plan_assembly(&request[..], &root, 1, 1, RESOURCE_HASH).unwrap_err();
    assert!(matches!(
        error,
        synth_dicom_gen::assembly::AssemblyError::UnsafePath(_)
    ));
    fs::remove_dir_all(root).unwrap();
    fs::remove_dir_all(outside).unwrap();
}
