use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use dicom_test_suite::corpus_plan::{
    ArtifactProvenance, ArtifactResourceEstimate, CaseBinding, EvidenceIndependence,
    EvidenceObligation, EvidencePlan, PlannedArtifact, PlannedQualification,
    PlannedQualificationSource, QualificationPayloadPolicy, ValidationPlan, ValidationRequirement,
    ValidationRule,
};
use dicom_test_suite::curated_execution::CuratedExecutionServiceFactory;
use dicom_test_suite::curated_plan::{
    CuratedCatalogPaths, CuratedScCorpusPlanProvider, CuratedScPlanRequest, CuratedScSelection,
};
use dicom_test_suite::executor::adapters::ManifestProjectionCompatibilityInput;
use dicom_test_suite::executor::cancellation::CancellationToken;
use dicom_test_suite::executor::engine::{
    CorpusExecutor, ManifestProjectionError, ManifestProjector,
};
use dicom_test_suite::executor::materialization::{
    AuxiliaryMaterializationHandler, AuxiliaryPayload, MaterializationDispatcher,
    MaterializationError,
};
use dicom_test_suite::executor::services::{
    ArtifactExecutionBindings, AssetDeclaration, AssetVisibility, MaterializationRequest,
    ProducedAsset, SlotExecutionBinding, StagedAssetHandle, StagedAssetRegistry,
    StagingRelativePath,
};
use dicom_test_suite::sha256_hex;
use serde_json::Value;

const FUZZ_CASE: &str = "fuzz/parser/bounded_seed_corpus";
const SOURCE_CASES: [&str; 2] = [
    "classic/sc/mono2_u8_explicit_le",
    "classic/sc/mono1_u8_rle_lossless",
];
const EXPECTED_QUALIFICATION_SHA256: &str =
    "7d4897cdbefa79a1ab96c64c8c6505d7b95945c4b269b7de933f4a2b93f76df7";

static NEXT: AtomicU64 = AtomicU64::new(0);

struct TempRoot(PathBuf);

impl TempRoot {
    fn new(label: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "dts-qualification-{label}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct UnusedProjector;

impl ManifestProjector for UnusedProjector {
    fn project(
        &self,
        _: &ManifestProjectionCompatibilityInput,
    ) -> Result<Vec<u8>, ManifestProjectionError> {
        panic!("source-only staging must not project a manifest")
    }
}

struct NoAuxiliary;

impl AuxiliaryMaterializationHandler for NoAuxiliary {
    fn render(
        &self,
        _: &dicom_test_suite::corpus_plan::PlannedAuxiliaryArtifact,
        _: &ArtifactExecutionBindings,
        _: &StagedAssetRegistry,
    ) -> Result<AuxiliaryPayload, MaterializationError> {
        panic!("qualification dispatch has no auxiliary payload")
    }
}

fn validation() -> ValidationPlan {
    ValidationPlan {
        rules: vec![ValidationRule {
            rule_id: "validation.qualification".into(),
            requirement: ValidationRequirement::Required,
            parameters: BTreeMap::new(),
        }],
    }
}

fn evidence() -> EvidencePlan {
    EvidencePlan {
        obligations: vec![EvidenceObligation {
            obligation_id: "projection.qualification".into(),
            route_id: "same_project.bounded_fuzz".into(),
            independence: EvidenceIndependence::SameProject,
            required: true,
            parameters: BTreeMap::new(),
        }],
    }
}

fn recipe_parameters() -> (BTreeMap<String, Value>, Vec<Value>) {
    let recipe: Value = serde_json::from_slice(
        &fs::read("cases/recipes/fuzz/parser/fuzz_parser_bounded_seed_corpus.json").unwrap(),
    )
    .unwrap();
    let parameters = recipe["qualification"]["parameters"]
        .as_object()
        .unwrap()
        .clone()
        .into_iter()
        .collect();
    let sources = recipe["qualification"]["parameters"]["sources"]
        .as_array()
        .unwrap()
        .clone();
    (parameters, sources)
}

fn files(root: &Path) -> Vec<PathBuf> {
    fn visit(root: &Path, current: &Path, output: &mut Vec<PathBuf>) {
        for entry in fs::read_dir(current).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                visit(root, &path, output);
            } else {
                output.push(path.strip_prefix(root).unwrap().to_path_buf());
            }
        }
    }
    let mut output = Vec::new();
    visit(root, root, &mut output);
    output.sort();
    output
}

#[test]
fn fuzz_dispatch_matches_frozen_qualification_and_publishes_no_payload() {
    let provider =
        CuratedScCorpusPlanProvider::load(CuratedCatalogPaths::from_repository_root(".")).unwrap();
    let bundle = provider
        .plan(&CuratedScPlanRequest {
            selection: CuratedScSelection::CaseIds(
                SOURCE_CASES.iter().map(|value| (*value).into()).collect(),
            ),
            seed: 7,
            max_parallelism: 2,
        })
        .unwrap();
    let staging = TempRoot::new("sources");
    CorpusExecutor::new(
        CuratedExecutionServiceFactory::new(&bundle),
        UnusedProjector,
    )
    .execute_into_staging(&bundle.plan, &staging.0, 2, &CancellationToken::new())
    .unwrap();

    let (parameters, recipe_sources) = recipe_parameters();
    let mut registry = StagedAssetRegistry::default();
    let mut planned_sources = Vec::new();
    let mut slots = BTreeMap::new();
    for (index, (case_id, recipe_source)) in SOURCE_CASES.iter().zip(&recipe_sources).enumerate() {
        let PlannedArtifact::Dicom(source) = bundle
            .plan
            .artifacts
            .iter()
            .find(|artifact| {
                matches!(
                    artifact,
                    PlannedArtifact::Dicom(value)
                        if value.case_binding.as_ref().is_some_and(|binding| binding.case_id == *case_id)
                )
            })
            .unwrap()
        else {
            unreachable!()
        };
        let bytes = fs::read(staging.0.join(source.output.relative_path.as_str())).unwrap();
        let digest = sha256_hex(&bytes);
        let handle = StagedAssetHandle::new(format!("output:{}", source.logical_id)).unwrap();
        registry
            .register(ProducedAsset {
                declaration: AssetDeclaration {
                    handle: handle.clone(),
                    relative_path: StagingRelativePath::new(
                        source.output.relative_path.to_string(),
                    )
                    .unwrap(),
                    size_bytes: bytes.len() as u64,
                    sha256: digest.clone(),
                    media_type: "application/dicom".into(),
                    visibility: AssetVisibility::Private,
                },
                observed_size_bytes: bytes.len() as u64,
                observed_sha256: digest.clone(),
            })
            .unwrap();
        let slot = format!("source_{index}");
        slots.insert(
            slot.clone(),
            SlotExecutionBinding::StagedAsset { asset: handle },
        );
        planned_sources.push(PlannedQualificationSource {
            artifact_id: source.logical_id.clone(),
            case_binding: source.case_binding.clone().unwrap(),
            artifact_logical_id: recipe_source["artifact_logical_id"]
                .as_str()
                .unwrap()
                .into(),
            dependency_role: recipe_source["dependency_role"].as_str().unwrap().into(),
            binding_slot: slot,
            expected_sha256: digest,
            expected_size_bytes: bytes.len() as u64,
            parameters: BTreeMap::from([
                (
                    "seed_description_id".into(),
                    recipe_source["seed_description_id"].clone(),
                ),
                (
                    "mutation_surfaces".into(),
                    recipe_source["mutation_surfaces"].clone(),
                ),
            ]),
        });
    }
    let request = MaterializationRequest {
        artifact: PlannedArtifact::Qualification(PlannedQualification {
            logical_id: "fuzz_qualification".into(),
            order: 2,
            provenance: ArtifactProvenance::Requested,
            case_binding: Some(CaseBinding {
                case_id: FUZZ_CASE.into(),
                recipe_id: "fuzz_parser_bounded_seed_corpus".into(),
                recipe_version: "0.1.0".into(),
            }),
            profile: Some("fuzz".into()),
            run_seed: Some(1),
            qualification_kind: "bounded_deterministic_fuzz".into(),
            parameters,
            sources: planned_sources,
            payload_policy: QualificationPayloadPolicy::NoPayload,
            validation: validation(),
            evidence: evidence(),
            resources: ArtifactResourceEstimate {
                output_bytes: 0,
                peak_working_bytes: 16 * 1024 * 1024,
            },
        }),
        bindings: ArtifactExecutionBindings {
            artifact_id: "fuzz_qualification".into(),
            slots,
        },
    };
    let before = files(&staging.0);
    let dispatcher = MaterializationDispatcher::new(&staging.0, Arc::new(NoAuxiliary)).unwrap();
    let result = dispatcher.dispatch(&request, &registry).unwrap();
    assert!(result.output.is_none());
    assert_eq!(files(&staging.0), before);
    assert_eq!(result.evidence.len(), 1);
    let qualification = Value::Object(result.evidence[0].claims.clone().into_iter().collect());
    assert_eq!(
        sha256_hex(&serde_json::to_vec(&qualification).unwrap()),
        EXPECTED_QUALIFICATION_SHA256
    );

    let mut missing = request.clone();
    missing.bindings.slots.remove("source_0");
    assert!(matches!(
        dispatcher.dispatch(&missing, &registry),
        Err(MaterializationError::QualificationContract(_))
    ));
    let mut extra = request.clone();
    extra
        .bindings
        .slots
        .insert("extra".into(), extra.bindings.slots["source_0"].clone());
    assert!(matches!(
        dispatcher.dispatch(&extra, &registry),
        Err(MaterializationError::QualificationContract(_))
    ));

    let mut public_registry = StagedAssetRegistry::default();
    for declaration in registry.iter() {
        let mut declaration = declaration.clone();
        declaration.visibility = AssetVisibility::PublicationCandidate;
        public_registry
            .register(ProducedAsset {
                observed_size_bytes: declaration.size_bytes,
                observed_sha256: declaration.sha256.clone(),
                declaration,
            })
            .unwrap();
    }
    assert!(matches!(
        dispatcher.dispatch(&request, &public_registry),
        Err(MaterializationError::QualificationSourceIdentity(_))
    ));

    let mut drift = request.clone();
    let PlannedArtifact::Qualification(artifact) = &mut drift.artifact else {
        unreachable!()
    };
    artifact.sources[0].expected_sha256 = "0".repeat(64);
    assert!(matches!(
        dispatcher.dispatch(&drift, &registry),
        Err(MaterializationError::QualificationSourceIdentity(_))
    ));

    let mut unacceptable = request.clone();
    let PlannedArtifact::Qualification(artifact) = &mut unacceptable.artifact else {
        unreachable!()
    };
    artifact.parameters.get_mut("budget").unwrap()["max_target_operations"] = Value::from(1);
    assert!(matches!(
        dispatcher.dispatch(&unacceptable, &registry),
        Err(MaterializationError::UnacceptableQualificationOutcome(_))
    ));

    let cancellation = CancellationToken::new();
    cancellation.cancel_with_reason("qualification test");
    assert!(matches!(
        dispatcher.dispatch_cancellable(&request, &registry, &cancellation),
        Err(MaterializationError::Cancelled)
    ));
    assert_eq!(files(&staging.0), before);

    let mut reordered = request;
    let PlannedArtifact::Qualification(artifact) = &mut reordered.artifact else {
        unreachable!()
    };
    artifact.sources.swap(0, 1);
    assert!(matches!(
        dispatcher.dispatch(&reordered, &registry),
        Err(MaterializationError::QualificationSourceIdentity(_))
    ));
}
