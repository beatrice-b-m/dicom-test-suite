use std::path::Path;

use serde_json::Value;
use synth_dicom_gen::sdk::{DicomTestSuite, ReportKind, ReportRequest};

pub fn assert_current_contract(output_root: &Path, expected: &Value) {
    let product = DicomTestSuite::embedded().expect("embedded SDK resources must verify");
    let report = product
        .report(ReportRequest::new(output_root))
        .expect("current coverage report must pass the shared version-aware reader");
    assert_eq!(report.kind(), ReportKind::CuratedCoverage);
    assert_eq!(report.schema_version(), "1.0.0");
    let actual: Value = report
        .deserialize()
        .expect("SDK report JSON must deserialize");
    assert_eq!(actual, *expected);
}
