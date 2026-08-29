use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use dicom_object::open_file;
use dicom_test_suite::curated_execution::CuratedExecutionServiceFactory;
use dicom_test_suite::curated_plan::{
    CuratedCatalogPaths, CuratedScCorpusPlanProvider, CuratedScPlanRequest, CuratedScSelection,
};
use dicom_test_suite::curated_validation::{
    ExtendedOffsetTableValidationSpec, ScPart10ValidationInput, ScPixelLengthFormula,
    validate_extended_offset_table_round_trip, validate_metadata_round_trip, validate_sc_part10,
};
use dicom_test_suite::executor::adapters::ManifestProjectionCompatibilityInput;
use dicom_test_suite::executor::cancellation::CancellationToken;
use dicom_test_suite::executor::engine::{
    CorpusExecutor, ManifestProjectionError, ManifestProjector,
};
use dicom_test_suite::executor::evidence::{ExecutionStatus, PublicationState, ResultStatus};
use dicom_test_suite::executor::transaction::OutputTransaction;
use dicom_test_suite::recipes::RecipeCatalog;

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

struct TempOutput(PathBuf);

impl TempOutput {
    fn absent() -> Self {
        let safe_temp = std::env::temp_dir().canonicalize().unwrap();
        Self(safe_temp.join(format!(
            "dicom-test-suite-curated-execution-{}-{}",
            std::process::id(),
            TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
        )))
    }
}

impl Drop for TempOutput {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct EvidenceProjector;

impl ManifestProjector for EvidenceProjector {
    fn project(
        &self,
        input: &ManifestProjectionCompatibilityInput,
    ) -> Result<Vec<u8>, ManifestProjectionError> {
        let mut bytes = serde_json::to_vec_pretty(&serde_json::json!({
            "corpus_plan_sha256": input.corpus_plan_sha256,
            "artifacts": input
                .artifacts
                .iter()
                .map(|artifact| serde_json::json!({
                    "logical_id": artifact.execution.logical_id,
                    "path": artifact.execution.output.as_ref().map(|output| &output.relative_path),
                    "status": artifact.execution.status,
                }))
                .collect::<Vec<_>>(),
        }))
        .map_err(|error| ManifestProjectionError(error.to_string()))?;
        bytes.push(b'\n');
        Ok(bytes)
    }
}

#[test]
fn corpus_executor_runs_a_production_curated_sc_plan_end_to_end() {
    let provider =
        CuratedScCorpusPlanProvider::load(CuratedCatalogPaths::from_repository_root(".")).unwrap();
    let bundle = provider
        .plan(&CuratedScPlanRequest {
            selection: CuratedScSelection::CaseIds(vec![
                "classic/sc/mono2_u8_explicit_le".into(),
                "classic/sc/mono2_u8_rle_lossless".into(),
                "metadata/sc/timezone_boundaries".into(),
            ]),
            seed: 7,
            max_parallelism: 2,
        })
        .unwrap();
    let destination = TempOutput::absent();
    let result = CorpusExecutor::new(
        CuratedExecutionServiceFactory::new(&bundle),
        EvidenceProjector,
    )
    .execute(&bundle.plan, &destination.0, 2, &CancellationToken::new())
    .unwrap();

    assert_eq!(result.destination, destination.0);
    assert_eq!(
        result.evidence.publication.state,
        PublicationState::Promoted
    );
    assert_eq!(result.evidence.artifacts.len(), bundle.plan.artifacts.len());
    assert_eq!(result.evidence.resources.used_parallelism, 2);
    assert!(destination.0.join("manifest.json").is_file());
    for artifact in &result.evidence.artifacts {
        assert_eq!(artifact.status, ExecutionStatus::Succeeded);
        assert!(artifact.resources.actual_output_bytes > 0);
        assert!(artifact.resources.actual_peak_working_bytes.is_some());
        assert!(
            artifact
                .validation
                .iter()
                .all(|validation| validation.status == ResultStatus::Passed)
        );
        assert!(
            artifact
                .obligations
                .iter()
                .all(|obligation| obligation.status == ResultStatus::Passed)
        );
        let output = artifact.output.as_ref().unwrap();
        let path = destination.0.join(&output.relative_path);
        assert_eq!(fs::metadata(&path).unwrap().len(), output.size_bytes);
        open_file(path).unwrap();
    }
    let rle = result
        .evidence
        .artifacts
        .iter()
        .find(|artifact| artifact.logical_id.contains("mono2_u8_rle_lossless"))
        .unwrap();
    assert_eq!(rle.codecs.len(), 1);
    assert_eq!(
        rle.codecs[0].backend_id,
        dicom_test_suite::codecs::NativeRleLosslessEncoder::BACKEND_ID
    );
}

#[test]
fn shared_executor_can_finish_inside_a_caller_owned_transaction() {
    let provider =
        CuratedScCorpusPlanProvider::load(CuratedCatalogPaths::from_repository_root(".")).unwrap();
    let bundle = provider
        .plan(&CuratedScPlanRequest {
            selection: CuratedScSelection::CaseIds(vec!["classic/sc/mono2_u8_explicit_le".into()]),
            seed: 7,
            max_parallelism: 1,
        })
        .unwrap();
    let destination = TempOutput::absent();
    let transaction = OutputTransaction::begin(&destination.0).unwrap();
    let staging_root = transaction.staging_root().to_owned();
    let result = CorpusExecutor::new(
        CuratedExecutionServiceFactory::new(&bundle),
        EvidenceProjector,
    )
    .execute_into_staging(&bundle.plan, &staging_root, 1, &CancellationToken::new())
    .unwrap();

    assert_eq!(result.evidence.publication.state, PublicationState::Staging);
    assert_eq!(result.projection.artifacts.len(), 1);
    assert!(
        staging_root
            .join("classic/sc/mono2_u8_explicit_le/instance.dcm")
            .is_file()
    );
    assert!(!staging_root.join("manifest.json").exists());
    assert!(!destination.0.exists());
    transaction.cleanup().unwrap();
}

#[test]
fn every_metadata_and_nonsquare_artifact_emits_reopened_typed_evidence() {
    let recipes = RecipeCatalog::load(
        "cases/recipes",
        "cases/registry.json",
        "templates/catalog.json",
    )
    .unwrap();
    let case_ids = recipes
        .recipes()
        .values()
        .filter(|recipe| {
            recipe.plan_provider_id == "native.metadata_sc_plan"
                || recipe.dicom.as_ref().is_some_and(|dicom| {
                    dicom.artifacts.iter().any(|artifact| {
                        artifact
                            .validation_rule_ids
                            .iter()
                            .any(|rule| rule == "validation.sc.geometry")
                    })
                })
        })
        .map(|recipe| recipe.binding.case_id.clone())
        .collect::<Vec<_>>();
    assert!(!case_ids.is_empty());
    let provider =
        CuratedScCorpusPlanProvider::load(CuratedCatalogPaths::from_repository_root(".")).unwrap();
    let bundle = provider
        .plan(&CuratedScPlanRequest {
            selection: CuratedScSelection::CaseIds(case_ids),
            seed: 7,
            max_parallelism: 2,
        })
        .unwrap();
    let destination = TempOutput::absent();
    let result = CorpusExecutor::new(
        CuratedExecutionServiceFactory::new(&bundle),
        EvidenceProjector,
    )
    .execute(&bundle.plan, &destination.0, 2, &CancellationToken::new())
    .unwrap();

    for artifact in &result.evidence.artifacts {
        let checks = artifact
            .validation
            .iter()
            .find_map(|result| result.details.get("checks"))
            .and_then(serde_json::Value::as_array)
            .expect("curated validation must publish ordered typed checks");
        assert!(checks.iter().any(|check| {
            check.get("name").and_then(serde_json::Value::as_str)
                == Some("curated_composition_plan")
        }));
        assert!(
            artifact
                .validation
                .iter()
                .any(|result| { result.details.contains_key("metadata_observation") })
        );
    }
}

#[test]
fn shared_part10_validator_rejects_corrupted_rows() {
    let provider =
        CuratedScCorpusPlanProvider::load(CuratedCatalogPaths::from_repository_root(".")).unwrap();
    let bundle = provider
        .plan(&CuratedScPlanRequest {
            selection: CuratedScSelection::CaseIds(vec!["classic/sc/mono2_u8_explicit_le".into()]),
            seed: 7,
            max_parallelism: 1,
        })
        .unwrap();
    let destination = TempOutput::absent();
    let result = CorpusExecutor::new(
        CuratedExecutionServiceFactory::new(&bundle),
        EvidenceProjector,
    )
    .execute(&bundle.plan, &destination.0, 1, &CancellationToken::new())
    .unwrap();
    let evidence = &result.evidence.artifacts[0];
    let dicom_test_suite::corpus_plan::PlannedArtifact::Dicom(planned) = &bundle.plan.artifacts[0]
    else {
        panic!("curated SC plan must contain DICOM");
    };
    let path = destination
        .0
        .join(&evidence.output.as_ref().unwrap().relative_path);
    let mut bytes = fs::read(&path).unwrap();
    let rows_header = [0x28, 0x00, 0x10, 0x00, b'U', b'S', 0x02, 0x00];
    let offset = bytes
        .windows(rows_header.len())
        .position(|window| window == rows_header)
        .expect("Rows header");
    bytes[offset + rows_header.len()] = 3;
    fs::write(&path, bytes).unwrap();
    let sop_instance_uid = planned
        .instance
        .identities
        .get(
            &dicom_test_suite::composition::CompositionUidRole::SopInstance,
            0,
        )
        .unwrap();
    let error = validate_sc_part10(
        &path,
        &ScPart10ValidationInput {
            sop_class_uid: &planned.instance.sop_class_uid,
            sop_instance_uid,
            transfer_syntax_uid: &planned.encoding.transfer_syntax_uid,
            implementation_class_uid: &planned.encoding.implementation.class_uid,
            rows: 2,
            columns: 2,
            frames: 1,
            samples_per_pixel: 1,
            photometric_interpretation: "MONOCHROME2",
            bits_allocated: 8,
            bits_stored: 8,
            high_bit: 7,
            pixel_representation: 0,
            planar_configuration: None,
            pixel_data_vr: dicom_core::VR::OB,
            pixel_data_length_formula: ScPixelLengthFormula::ContiguousSamples,
            decoded_frame_hashes: &[],
            palette: None,
            padding: None,
        },
    )
    .unwrap_err();
    assert!(error.to_string().contains("rows"));
}

#[test]
fn metadata_validator_rejects_corrupted_encoded_person_name() {
    let provider =
        CuratedScCorpusPlanProvider::load(CuratedCatalogPaths::from_repository_root(".")).unwrap();
    let bundle = provider
        .plan(&CuratedScPlanRequest {
            selection: CuratedScSelection::CaseIds(vec!["metadata/sc/utf8_person_name".into()]),
            seed: 7,
            max_parallelism: 1,
        })
        .unwrap();
    let destination = TempOutput::absent();
    let result = CorpusExecutor::new(
        CuratedExecutionServiceFactory::new(&bundle),
        EvidenceProjector,
    )
    .execute(&bundle.plan, &destination.0, 1, &CancellationToken::new())
    .unwrap();
    let path = destination.0.join(
        &result.evidence.artifacts[0]
            .output
            .as_ref()
            .unwrap()
            .relative_path,
    );
    let mut bytes = fs::read(&path).unwrap();
    let needle = b"Wang^XiaoDong";
    let offset = bytes
        .windows(needle.len())
        .position(|window| window == needle)
        .expect("encoded Patient Name bytes");
    bytes[offset] = b'V';
    fs::write(&path, bytes).unwrap();
    let metadata = bundle.projection.artifacts[0]
        .artifact_recipe
        .metadata_sc
        .as_ref()
        .unwrap();
    let error = validate_metadata_round_trip(&path, metadata).unwrap_err();
    assert!(error.to_string().contains("Patient Name"));
}

#[test]
fn curated_eot_validation_is_evidence_backed_and_rejects_corrupted_ov_words() {
    let provider =
        CuratedScCorpusPlanProvider::load(CuratedCatalogPaths::from_repository_root(".")).unwrap();
    let bundle = provider
        .plan(&CuratedScPlanRequest {
            selection: CuratedScSelection::CaseIds(vec![
                "encapsulation/sc/eot_single_fragment_multiframe".into(),
            ]),
            seed: 7,
            max_parallelism: 1,
        })
        .unwrap();
    let destination = TempOutput::absent();
    let result = CorpusExecutor::new(
        CuratedExecutionServiceFactory::new(&bundle),
        EvidenceProjector,
    )
    .execute(&bundle.plan, &destination.0, 1, &CancellationToken::new())
    .unwrap();
    let execution = &result.evidence.artifacts[0];
    let checks = execution
        .validation
        .iter()
        .find_map(|validation| validation.details.get("checks"))
        .and_then(serde_json::Value::as_array)
        .unwrap();
    let internal_names = checks
        .iter()
        .filter(|check| check.get("layer").and_then(serde_json::Value::as_str) == Some("internal"))
        .filter_map(|check| check.get("name").and_then(serde_json::Value::as_str))
        .collect::<Vec<_>>();
    assert_eq!(
        &internal_names[internal_names.len() - 2..],
        &[
            "curated_composition_plan",
            "extended_offset_table_round_trip"
        ]
    );
    assert!(checks.iter().any(|check| {
        check.get("name").and_then(serde_json::Value::as_str)
            == Some("extended_offset_table_round_trip")
            && check.get("message").and_then(serde_json::Value::as_str)
                == Some(
                    "Exact OV offsets and lengths reopened with an empty Basic Offset Table and one RLE fragment per frame.",
                )
    }));

    let content = execution
        .materialization
        .as_ref()
        .unwrap()
        .content
        .iter()
        .find(|content| content.slot == "pixels")
        .unwrap();
    let projection = bundle.projection.artifacts[0]
        .artifact_recipe
        .secondary_capture
        .as_ref()
        .unwrap()
        .encapsulation_projection
        .as_ref()
        .unwrap();
    let spec = ExtendedOffsetTableValidationSpec {
        offsets: content.extended_offset_table.clone(),
        lengths: content.extended_offset_table_lengths.clone(),
        compressed_fragment_lengths: content.compressed_lengths.clone(),
        padded_fragment_lengths: content.padded_fragment_lengths.clone(),
        fragments_per_frame: content.fragments_per_frame.clone(),
        fragment_item_start_offsets: content
            .fragments
            .iter()
            .map(|fragment| fragment.item_start_offset)
            .collect(),
        page_numbers: vec![1, 2, 3],
        offset_origin: projection.offset_origin.clone(),
        item_header_bytes: u64::from(projection.item_header_bytes),
    };
    let path = destination
        .0
        .join(&execution.output.as_ref().unwrap().relative_path);
    let mut bytes = fs::read(&path).unwrap();
    let eot_header = [0xE0, 0x7F, 0x01, 0x00, b'O', b'V', 0, 0, 24, 0, 0, 0];
    let offset = bytes
        .windows(eot_header.len())
        .position(|window| window == eot_header)
        .expect("Extended Offset Table header");
    bytes[offset + eot_header.len()] ^= 1;
    fs::write(&path, bytes).unwrap();
    let error = validate_extended_offset_table_round_trip(&path, &spec).unwrap_err();
    assert!(error.to_string().contains("Extended Offset Table"));
}
