#[path = "../src/protocol.rs"]
mod protocol;

use protocol::{
    DicomwebSection, DimseSection, PeerFingerprint, PkiFixtureFingerprint, ProtocolEvidenceError,
    ProtocolQualification, ProtocolQualificationInput, ProtocolReport, ProtocolSection,
    ProtocolStep, ProviderRelationship, QualificationStatus, SourceCaseLink, StepOutcome,
    TlsSection, ToolFingerprint, deterministic_transaction_id,
};

const SOURCE_SHA256: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const HARNESS_SHA256: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const PEER_SHA256: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";

fn harness() -> ToolFingerprint {
    ToolFingerprint {
        id: "dts_protocol_harness".to_string(),
        version: "0.1.0".to_string(),
        executable_sha256: HARNESS_SHA256.to_string(),
    }
}

fn peer(id: &str) -> PeerFingerprint {
    PeerFingerprint {
        id: id.to_string(),
        implementation: "DCMTK storescp".to_string(),
        version: "3.6.9".to_string(),
        artifact_sha256: PEER_SHA256.to_string(),
    }
}

fn source() -> SourceCaseLink {
    SourceCaseLink {
        case_id: "classic/sc/mono2_u8_explicit_le".to_string(),
        path: "classic/sc/mono2_u8_explicit_le/instance.dcm".to_string(),
        sha256: SOURCE_SHA256.to_string(),
        sop_instance_uid: "2.25.123456789".to_string(),
    }
}

fn passed_step(ordinal: u32, operation: &str) -> ProtocolStep {
    ProtocolStep {
        ordinal,
        operation: operation.to_string(),
        outcome: StepOutcome::Passed,
        detail: format!("{operation} completed within its bounded deadline"),
    }
}

fn dimse_input() -> ProtocolQualificationInput {
    ProtocolQualificationInput {
        case_id: "protocol/dimse/storage_query_retrieve".to_string(),
        run_seed: 7,
        transaction_ordinal: 1,
        harness: harness(),
        peer: peer("dcmtk_storescp"),
        provider_relationship: ProviderRelationship::Independent,
        sources: vec![source()],
        steps: vec![
            passed_step(1, "associate"),
            passed_step(2, "c_echo"),
            passed_step(3, "c_store"),
        ],
        section: ProtocolSection::Dimse(DimseSection {
            calling_ae_title: "DTS_SCU".to_string(),
            called_ae_title: "DTS_SCP".to_string(),
            presentation_contexts: vec![
                "1.2.840.10008.5.1.4.1.1.7|1.2.840.10008.1.2.1".to_string(),
            ],
            operations: vec![
                "association".to_string(),
                "c_echo".to_string(),
                "c_store".to_string(),
            ],
        }),
        pki_fixtures: vec![],
    }
}

#[test]
fn transaction_ids_are_stable_and_input_scoped() {
    let first = deterministic_transaction_id("protocol/dimse/storage_query_retrieve", 7, 1);
    assert_eq!(
        first,
        deterministic_transaction_id("protocol/dimse/storage_query_retrieve", 7, 1)
    );
    assert_ne!(
        first,
        deterministic_transaction_id("protocol/dimse/storage_query_retrieve", 7, 2)
    );
    assert_ne!(
        first,
        deterministic_transaction_id("protocol/dicomweb/stow_qido_wado", 7, 1)
    );
    assert!(first.starts_with("txn-"));
    assert_eq!(first.len(), 36);
}

#[test]
fn dimse_evidence_preserves_independent_peer_sources_and_step_order() {
    let qualification = ProtocolQualification::new(dimse_input()).unwrap();
    assert_eq!(qualification.status, QualificationStatus::Passed);
    assert!(qualification.is_independent());
    assert_eq!(qualification.steps.len(), 3);
    assert_eq!(
        qualification.sources[0].case_id,
        "classic/sc/mono2_u8_explicit_le"
    );
    assert_eq!(qualification.unavailable_evidence().count(), 0);

    let json = qualification.to_json().unwrap();
    assert_eq!(json["contract_version"], "0.1.0");
    assert_eq!(json["provider_relationship"], "independent");
    assert_eq!(json["section"]["kind"], "dimse");
    assert_eq!(json["steps"][0]["ordinal"], 1);
    assert_eq!(json["steps"][2]["operation"], "c_store");

    let report = ProtocolReport::new(vec![qualification]).unwrap();
    assert_eq!(report.summary.dimse.passed, 1);
    assert_eq!(report.summary.total.passed, 1);
    assert_eq!(report.summary.dicomweb.passed, 0);
}

#[test]
fn unavailable_dicomweb_peer_is_explicit_and_same_provider_labeled() {
    let input = ProtocolQualificationInput {
        case_id: "protocol/dicomweb/stow_qido_wado".to_string(),
        run_seed: 7,
        transaction_ordinal: 1,
        harness: harness(),
        peer: peer("same_project_stub_server"),
        provider_relationship: ProviderRelationship::SameProvider,
        sources: vec![source()],
        steps: vec![ProtocolStep {
            ordinal: 1,
            operation: "discover_server".to_string(),
            outcome: StepOutcome::Unavailable {
                blocker_code: "independent_server_not_configured".to_string(),
                reason: "No pinned replaceable DICOMweb server was configured".to_string(),
            },
            detail: "Discovery stopped before opening a network socket".to_string(),
        }],
        section: ProtocolSection::Dicomweb(DicomwebSection {
            server_identity: "same_project_stub_server".to_string(),
            services: vec![
                "stow_rs".to_string(),
                "qido_rs".to_string(),
                "wado_rs".to_string(),
            ],
            media_types: vec!["application/dicom".to_string()],
        }),
        pki_fixtures: vec![],
    };
    let qualification = ProtocolQualification::new(input).unwrap();
    assert_eq!(qualification.status, QualificationStatus::Unavailable);
    assert!(!qualification.is_independent());
    assert_eq!(
        qualification.unavailable_evidence().collect::<Vec<_>>(),
        vec![(
            "independent_server_not_configured",
            "No pinned replaceable DICOMweb server was configured"
        )]
    );
    let report = ProtocolReport::new(vec![qualification]).unwrap();
    assert_eq!(report.summary.dicomweb.unavailable, 1);
    assert_eq!(report.summary.total.unavailable, 1);
}

fn pki_fixture(
    identity_id: &str,
    certificate_sha256: &str,
    certificate_fingerprint_sha256: &str,
    public_key_sha256: &str,
) -> PkiFixtureFingerprint {
    PkiFixtureFingerprint {
        identity_id: identity_id.to_string(),
        certificate_sha256: certificate_sha256.to_string(),
        certificate_fingerprint_sha256: certificate_fingerprint_sha256.to_string(),
        public_key_sha256: public_key_sha256.to_string(),
    }
}

#[test]
fn tls_evidence_accepts_only_public_locked_pki_fingerprints() {
    let pki_fixtures = vec![
        pki_fixture(
            "test_root_ca",
            "6b309374a4877b97327bcd82176dbfde4bf04059024ee701a444bd16d143a4a4",
            "d556f4c7febb2e48799fd6b92cc023a3c7c639bc9f3320e418eb7dfaf5dc4a05",
            "c12946c9f4478f3816b2d759e141e375dbf3bee6fdb23de2a3297754bf4413a5",
        ),
        pki_fixture(
            "tls_server",
            "eba35dab163bd3248728fffbc8f1df1dd53ce90038f21dfb01839dc97f4e680f",
            "b408fd3ab41793ecd2b3377374e278f8551670f960f7e3ab6123038271c3ff14",
            "82114ae2de45b69f8773f6be29c95c632d2d3176678eee0f1ca2b9c4626b17c0",
        ),
        pki_fixture(
            "tls_client",
            "3c9c7f586dacfebfaca3822926effb0bcb74d485b8ebd81c6fc3a91e41afd2f1",
            "c08b93f5401c7f67af89d9c7814bc83c2f3f704af72cfe9038dd47b00c8d77c3",
            "e93ee191870a8f91b69bf41a9e05d7017072d0f931489dba5b5df90773e6c02a",
        ),
    ];
    let qualification = ProtocolQualification::new(ProtocolQualificationInput {
        case_id: "protocol/security/tls_user_identity".to_string(),
        run_seed: 7,
        transaction_ordinal: 1,
        harness: harness(),
        peer: peer("dcmtk_tls_peer"),
        provider_relationship: ProviderRelationship::Independent,
        sources: vec![source()],
        steps: vec![
            passed_step(1, "mutual_tls_handshake"),
            passed_step(2, "user_identity"),
        ],
        section: ProtocolSection::Tls(TlsSection {
            protocol_version: "TLSv1.3".to_string(),
            cipher_suite: "TLS_AES_256_GCM_SHA384".to_string(),
            mutual_tls: true,
            user_identity_type: Some("username".to_string()),
            root_ca_identity_id: "test_root_ca".to_string(),
            server_identity_id: "tls_server".to_string(),
            client_identity_id: Some("tls_client".to_string()),
        }),
        pki_fixtures,
    })
    .unwrap();
    let serialized = serde_json::to_string(&qualification).unwrap();
    assert_eq!(qualification.status, QualificationStatus::Passed);
    assert!(!serialized.to_ascii_lowercase().contains("private_key"));
    assert!(!serialized.contains("BEGIN PRIVATE KEY"));
    assert!(
        serialized.contains("b408fd3ab41793ecd2b3377374e278f8551670f960f7e3ab6123038271c3ff14")
    );
    let report = ProtocolReport::new(vec![qualification]).unwrap();
    assert_eq!(report.summary.tls.passed, 1);
    assert_eq!(report.to_json().unwrap()["summary"]["total"]["passed"], 1);
}

#[test]
fn invalid_order_hashes_tls_links_and_failure_outcomes_are_checked() {
    let mut out_of_order = dimse_input();
    out_of_order.steps[1].ordinal = 3;
    assert!(matches!(
        ProtocolQualification::new(out_of_order),
        Err(ProtocolEvidenceError::StepOrder {
            expected: 2,
            actual: 3
        })
    ));

    let mut bad_hash = dimse_input();
    bad_hash.peer.artifact_sha256 = "ABC".to_string();
    assert!(ProtocolQualification::new(bad_hash).is_err());

    let mut timeout = dimse_input();
    timeout.steps[2].outcome = StepOutcome::Timeout {
        limit_milliseconds: 5_000,
    };
    assert_eq!(
        ProtocolQualification::new(timeout).unwrap().status,
        QualificationStatus::Failed
    );

    let mut rejected = dimse_input();
    rejected.steps[2].outcome = StepOutcome::Rejected {
        code: "0xA700".to_string(),
    };
    assert_eq!(
        ProtocolQualification::new(rejected).unwrap().status,
        QualificationStatus::Failed
    );

    let mut crashed = dimse_input();
    crashed.steps[2].outcome = StepOutcome::Crash {
        exit_code: None,
        signal: Some(11),
    };
    assert_eq!(
        ProtocolQualification::new(crashed).unwrap().status,
        QualificationStatus::Failed
    );

    let qualification = ProtocolQualification::new(dimse_input()).unwrap();
    assert!(matches!(
        ProtocolReport::new(vec![qualification.clone(), qualification]),
        Err(ProtocolEvidenceError::DuplicateTransaction(_))
    ));
}
