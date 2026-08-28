use std::fs;

use dicom_test_suite::media::{
    CheckStatus, DcmtkProviderFingerprint, DicomDirQualification, MediaDeterminism,
    MediaValidationEvidence,
};
use dicom_test_suite::protocol::{
    DicomwebSection, PeerFingerprint, ProtocolQualification, ProtocolQualificationInput,
    ProtocolReport, ProtocolSection, ProtocolStep, ProviderRelationship, SourceCaseLink,
    StepOutcome, ToolFingerprint,
};
use serde_json::Value;

const SHA_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const SHA_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const SHA_C: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";

fn validator(path: &str) -> jsonschema::Validator {
    let schema: Value = serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap();
    jsonschema::validator_for(&schema).unwrap()
}

fn media_qualification() -> DicomDirQualification {
    DicomDirQualification {
        contract_version: "0.1.0",
        determinism: MediaDeterminism::SemanticStable,
        file_set_id: "DTSMIXED".to_string(),
        file_set_uid: "2.25.123456789".to_string(),
        sop_class_uid: "1.2.840.10008.1.3.10",
        transfer_syntax_uid: "1.2.840.10008.1.2.1",
        dicomdir_sha256: SHA_A.to_string(),
        member_count: 3,
        provider: DcmtkProviderFingerprint {
            provider_id: "dcmtk".to_string(),
            executable_name: "dcmmkdir".to_string(),
            version: "3.7.0".to_string(),
            executable_sha256: SHA_B.to_string(),
            arguments: vec![
                "-Pgp".to_string(),
                "+F".to_string(),
                "DTSMIXED".to_string(),
                "+id".to_string(),
                "/tmp/fileset".to_string(),
                "+r".to_string(),
                "+D".to_string(),
                "/tmp/fileset/DICOMDIR".to_string(),
            ],
        },
        provider_warnings: vec![],
        evidence: MediaValidationEvidence {
            rust_closure: CheckStatus::Passed,
            dicom3tools_dciodvfy: CheckStatus::Passed,
            dicom3tools_dcentvfy: CheckStatus::Unavailable,
            dcmtk_parser_same_family: CheckStatus::Passed,
            dcm4che_independent_peer: CheckStatus::Unavailable,
        },
        independent_interoperability_proven: false,
    }
}

#[test]
fn media_schema_accepts_serialized_qualification_and_binds_independent_proof() {
    let validator = validator("schemas/media-report.schema.json");
    let report = media_qualification().to_json().unwrap();
    assert!(validator.is_valid(&report));

    let mut false_claim = report.clone();
    false_claim["independent_interoperability_proven"] = Value::Bool(true);
    assert!(!validator.is_valid(&false_claim));

    let mut unlocked_provider = report;
    unlocked_provider["provider"]["version"] = Value::String("3.6.9".to_string());
    assert!(!validator.is_valid(&unlocked_provider));
}

fn protocol_report() -> ProtocolReport {
    let qualification = ProtocolQualification::new(ProtocolQualificationInput {
        case_id: "protocol/dicomweb/stow_qido_wado".to_string(),
        run_seed: 17,
        transaction_ordinal: 1,
        harness: ToolFingerprint {
            id: "dts_protocol_harness".to_string(),
            version: "0.1.0".to_string(),
            executable_sha256: SHA_A.to_string(),
        },
        peer: PeerFingerprint {
            id: "unconfigured_peer".to_string(),
            implementation: "replaceable DICOMweb server".to_string(),
            version: "unavailable".to_string(),
            artifact_sha256: SHA_B.to_string(),
        },
        provider_relationship: ProviderRelationship::SameProvider,
        sources: vec![SourceCaseLink {
            case_id: "classic/sc/mono2_u8_explicit_le".to_string(),
            path: "classic/sc/mono2_u8_explicit_le/instance.dcm".to_string(),
            sha256: SHA_C.to_string(),
            sop_instance_uid: "2.25.987654321".to_string(),
        }],
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
            server_identity: "unconfigured_peer".to_string(),
            services: vec![
                "stow_rs".to_string(),
                "qido_rs".to_string(),
                "wado_rs".to_string(),
            ],
            media_types: vec!["application/dicom".to_string()],
        }),
        pki_fixtures: vec![],
    })
    .unwrap();
    ProtocolReport::new(vec![qualification]).unwrap()
}

#[test]
fn transaction_schema_accepts_serialized_report_and_rejects_secret_fields() {
    let validator = validator("schemas/transaction-report.schema.json");
    let report = protocol_report().to_json().unwrap();
    assert!(validator.is_valid(&report));

    let mut secret_bearing = report.clone();
    secret_bearing["transactions"][0]["pki_fixtures"] = serde_json::json!([{
        "identity_id": "tls_server",
        "certificate_sha256": SHA_A,
        "certificate_fingerprint_sha256": SHA_B,
        "public_key_sha256": SHA_C,
        "private_key_sha256": SHA_A
    }]);
    assert!(!validator.is_valid(&secret_bearing));

    let mut malformed_transaction = report;
    malformed_transaction["transactions"][0]["transaction_id"] =
        Value::String("txn-not-a-digest".to_string());
    assert!(!validator.is_valid(&malformed_transaction));
}
