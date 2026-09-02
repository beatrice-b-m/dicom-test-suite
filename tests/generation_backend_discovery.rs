use std::path::Path;

use synth_dicom_gen::generation_backends::{
    BackendDiscovery, backend_policy, discover_prepared_backend, load_backend_lock,
};

#[test]
fn committed_highdicom_policy_discovers_or_explicitly_reports_absence() {
    let lock = load_backend_lock(Path::new(".")).expect("backend lock should validate");
    let policy =
        backend_policy(&lock, "highdicom_pydicom").expect("highdicom/pydicom policy should exist");
    match discover_prepared_backend(Path::new("."), policy)
        .expect("discovery policy should be internally valid")
    {
        BackendDiscovery::Available(prepared) => {
            assert_eq!(prepared.backend_id, "highdicom_pydicom");
            assert_eq!(prepared.version, "dts-highdicom-backend 0.5.0");
            assert_eq!(
                prepared
                    .runtime_identity
                    .pointer("/python/version")
                    .and_then(serde_json::Value::as_str),
                Some("3.12.12")
            );
            assert!(prepared.executable.is_absolute());
        }
        BackendDiscovery::Unavailable { code, message } => {
            assert_eq!(code, "dependency_unavailable");
            assert!(message.contains("provision it with the committed uv lock"));
        }
    }
}
