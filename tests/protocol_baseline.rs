#[path = "../src/protocol.rs"]
mod protocol;
#[path = "../src/protocol_baseline.rs"]
mod protocol_baseline;

use std::path::PathBuf;

use protocol::{ProviderRelationship, QualificationStatus, SourceCaseLink, ToolFingerprint};
use protocol_baseline::{
    ProtocolBaselineInput, build_unavailable_protocol_baseline, protocol_report_markdown,
};

fn source(case_id: &str, path: &str, uid: &str) -> SourceCaseLink {
    SourceCaseLink {
        case_id: case_id.into(),
        path: path.into(),
        sha256: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".into(),
        sop_instance_uid: uid.into(),
    }
}

fn input(sources: Vec<SourceCaseLink>) -> ProtocolBaselineInput {
    ProtocolBaselineInput {
        run_seed: 0x5eed,
        harness: ToolFingerprint {
            id: "dicom-test-suite-protocol-baseline".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            executable_sha256: "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789"
                .into(),
        },
        sources,
    }
}

fn fixture_lock() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("security/fixtures/fixtures.lock.json")
}

#[test]
fn builds_three_deterministic_explicit_unavailable_transactions() {
    let sources = vec![
        source("pixels/ct/native", "b.dcm", "1.2.840.10008.2"),
        source("derived/seg/binary", "a.dcm", "1.2.840.10008.1"),
    ];
    let first =
        build_unavailable_protocol_baseline(input(sources.clone()), &fixture_lock()).unwrap();
    let second = build_unavailable_protocol_baseline(
        input(sources.into_iter().rev().collect()),
        &fixture_lock(),
    )
    .unwrap();

    assert_eq!(first, second, "source ordering must not affect evidence");
    assert_eq!(first.transactions.len(), 3);
    assert_eq!(first.summary.total.unavailable, 3);
    assert_eq!(first.summary.dimse.unavailable, 1);
    assert_eq!(first.summary.dicomweb.unavailable, 1);
    assert_eq!(first.summary.tls.unavailable, 1);
    for transaction in &first.transactions {
        assert_eq!(transaction.status, QualificationStatus::Unavailable);
        assert_eq!(
            transaction.provider_relationship,
            ProviderRelationship::Independent
        );
        assert_eq!(transaction.sources.len(), 2);
        assert_eq!(transaction.steps.len(), 1);
    }
    let blockers = first
        .transactions
        .iter()
        .flat_map(|transaction| transaction.unavailable_evidence().map(|item| item.0))
        .collect::<Vec<_>>();
    assert_eq!(
        blockers,
        [
            "independent_dcm4che_peer_unavailable",
            "pinned_independent_dicomweb_server_unavailable",
            "replaceable_tls_peer_unavailable",
        ]
    );
}

#[test]
fn report_contains_only_public_pki_fingerprints() {
    let report = build_unavailable_protocol_baseline(
        input(vec![source(
            "pixels/ct/native",
            "ct.dcm",
            "1.2.840.10008.1",
        )]),
        &fixture_lock(),
    )
    .unwrap();
    let json = serde_json::to_string_pretty(&report.to_json().unwrap()).unwrap();

    for identity in ["test_root_ca", "tls_server", "tls_client"] {
        assert!(json.contains(identity));
    }
    assert!(!json.contains("private_key"));
    assert!(!json.contains("BEGIN PRIVATE KEY"));
    assert!(!json.contains("aa08edef46f96ee6d572030914f4614a99f9a46dc9cff4f96aaa0bf472c6a1a6"));
}

#[test]
fn markdown_distinguishes_all_unavailable_families() {
    let report = build_unavailable_protocol_baseline(
        input(vec![source(
            "pixels/ct/native",
            "ct.dcm",
            "1.2.840.10008.1",
        )]),
        &fixture_lock(),
    )
    .unwrap();
    let markdown = protocol_report_markdown(&report);

    assert!(markdown.contains("| DIMSE |"));
    assert!(markdown.contains("| DICOMweb |"));
    assert!(markdown.contains("| TLS / user identity |"));
    assert!(markdown.contains("3 unavailable, 0 passed, 0 failed"));
    assert!(markdown.contains("unavailable rows are not passes"));
}

#[test]
fn rejects_empty_source_metadata() {
    let error =
        build_unavailable_protocol_baseline(input(Vec::new()), &fixture_lock()).unwrap_err();
    assert_eq!(
        error.to_string(),
        "protocol baseline sources must not be empty"
    );
}
