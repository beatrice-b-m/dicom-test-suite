//! Internal verified-corpus batch runner. Not a supported SDK or CLI surface.
#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::Arc;

use serde_json::{Value, json};

use crate::corpus_definition::CorpusDefinitionBundle;
use crate::curated_plan::{
    CapturedCuratedPlan, CapturedCuratedPlanningContext, CuratedScPlanRequest, CuratedScSelection,
};
use crate::engine_resources::EngineResources;
use crate::executor::adapters::ManifestProjectionInput;
use crate::executor::cancellation::CancellationToken;
use crate::executor::engine::{
    CorpusExecutionResult, CorpusExecutor, ManifestProjectionError, ManifestProjector,
};

#[derive(Debug, Clone)]
pub(crate) enum CorpusSelection {
    Profile {
        profile: String,
        include_stress: bool,
    },
    CaseIds {
        profile: String,
        include_stress: bool,
        case_ids: Vec<String>,
    },
}

pub(crate) struct CapturedCorpusRequest {
    pub(crate) selection: CorpusSelection,
    pub(crate) destination: PathBuf,
    pub(crate) seed: u64,
    pub(crate) parallelism: u32,
    pub(crate) dry_run: bool,
    pub(crate) cancellation: CancellationToken,
}

pub(crate) struct CapturedPlanningOutcome {
    pub(crate) plan: crate::corpus_plan::CorpusPlan,
    pub(crate) selector: Value,
    /// `ready` is planning disposition, never a generation/validation pass.
    pub(crate) selection_ledger: Vec<Value>,
    pub(crate) identities: crate::identity::ManifestIdentityProjection,
    pub(crate) validation: &'static str,
    pub(crate) publication: &'static str,
}

pub(crate) enum CapturedCorpusOutcome {
    Planned(CapturedPlanningOutcome),
    NoExecutableCases(CapturedPlanningOutcome),
    Published(CorpusExecutionResult),
}

#[derive(Debug)]
pub(crate) enum CapturedCorpusError {
    Input(String),
    Cancelled,
    Planning(String),
    Execution(String),
}

impl std::fmt::Display for CapturedCorpusError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for CapturedCorpusError {}

fn checkpoint(token: &CancellationToken) -> Result<(), CapturedCorpusError> {
    if token.is_cancelled() {
        Err(CapturedCorpusError::Cancelled)
    } else {
        Ok(())
    }
}

fn selection(
    bundle: &CorpusDefinitionBundle,
    selector: &CorpusSelection,
) -> Result<(String, bool, Value, BTreeSet<String>, BTreeSet<String>), CapturedCorpusError> {
    let (profile, include_stress, requested) = match selector {
        CorpusSelection::Profile {
            profile,
            include_stress,
        } => (profile, *include_stress, None),
        CorpusSelection::CaseIds {
            profile,
            include_stress,
            case_ids,
        } => (profile, *include_stress, Some(case_ids)),
    };
    let definition = bundle
        .manifest()
        .profiles
        .iter()
        .find(|p| &p.profile_id == profile)
        .ok_or_else(|| CapturedCorpusError::Input(format!("unknown profile {profile}")))?;
    if include_stress && profile != "all" {
        return Err(CapturedCorpusError::Input(
            "include_stress requires all profile".into(),
        ));
    }
    let mut scope = definition.members.iter().cloned().collect::<BTreeSet<_>>();
    for member_profile in &definition.union_of {
        scope.extend(
            bundle
                .manifest()
                .profiles
                .iter()
                .find(|p| &p.profile_id == member_profile)
                .expect("verified profile union")
                .members
                .iter()
                .cloned(),
        );
    }
    if include_stress {
        scope.extend(
            bundle
                .manifest()
                .profiles
                .iter()
                .find(|p| p.profile_id == "stress")
                .expect("verified stress profile")
                .members
                .iter()
                .cloned(),
        );
    }
    let direct = if let Some(ids) = requested {
        let unique = ids.iter().cloned().collect::<BTreeSet<_>>();
        if ids.is_empty() || unique.len() != ids.len() {
            return Err(CapturedCorpusError::Input(
                "case IDs must be nonempty and unique".into(),
            ));
        }
        if let Some(id) = unique.iter().find(|id| !scope.contains(*id)) {
            return Err(CapturedCorpusError::Input(format!(
                "unknown or out-of-scope case {id}"
            )));
        }
        unique
    } else {
        scope
    };
    let selector = if requested.is_some() {
        json!({"kind":"case_ids","case_ids":direct})
    } else {
        json!({"kind":"profile"})
    };
    let mut closure = direct.clone();
    loop {
        let before = closure.len();
        for case in &bundle.manifest().cases {
            if closure.contains(&case.case_id) {
                closure.extend(case.dependencies.iter().cloned());
            }
        }
        if closure.len() == before {
            break;
        }
    }
    Ok((profile.clone(), include_stress, selector, direct, closure))
}

pub(crate) fn run_captured_corpus(
    bundle: Arc<CorpusDefinitionBundle>,
    resources: EngineResources,
    request: CapturedCorpusRequest,
) -> Result<CapturedCorpusOutcome, CapturedCorpusError> {
    run_with_publication_check(bundle, resources, request, Arc::new(|| Ok(())))
}

// A private injection seam proves transaction cleanup at projection failure and
// cancellation immediately before publication without changing executor policy.
fn run_with_publication_check(
    bundle: Arc<CorpusDefinitionBundle>,
    resources: EngineResources,
    request: CapturedCorpusRequest,
    before_publication: Arc<dyn Fn() -> Result<(), ManifestProjectionError> + Send + Sync>,
) -> Result<CapturedCorpusOutcome, CapturedCorpusError> {
    checkpoint(&request.cancellation)?;
    if request.parallelism == 0 {
        return Err(CapturedCorpusError::Input(
            "parallelism must be positive".into(),
        ));
    }
    match std::fs::symlink_metadata(&request.destination) {
        Ok(_) => return Err(CapturedCorpusError::Input("destination exists".into())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(CapturedCorpusError::Input(e.to_string())),
    }
    let (profile, include_stress, selector, direct, closure) =
        selection(&bundle, &request.selection)?;
    let context = CapturedCuratedPlanningContext::from_verified_bundle(&bundle, &resources)
        .map_err(|e| CapturedCorpusError::Planning(e.to_string()))?;
    let planned = context
        .plan(CuratedScPlanRequest {
            // Bundle1 recipe descriptors exist only for implemented rows. All
            // directly selected statuses still remain in the outcome ledger.
            selection: CuratedScSelection::CaseIds(
                direct
                    .iter()
                    .filter(|id| {
                        bundle
                            .manifest()
                            .cases
                            .iter()
                            .any(|case| &case.case_id == *id)
                    })
                    .cloned()
                    .collect(),
            ),
            seed: request.seed,
            max_parallelism: request.parallelism,
        })
        .map_err(|e| CapturedCorpusError::Planning(e.to_string()))?;
    checkpoint(&request.cancellation)?;
    let identities =
        crate::identity::project_manifest_identities(&resources, Some(&bundle), vec![])
            .map_err(|e| CapturedCorpusError::Planning(e.to_string()))?;
    let registry: Value = serde_json::from_slice(
        bundle
            .bytes(&bundle.manifest().registry.path)
            .expect("verified registry"),
    )
    .map_err(|e| CapturedCorpusError::Planning(e.to_string()))?;
    let rows = registry["cases"]
        .as_array()
        .expect("verified registry cases")
        .iter()
        .map(|row| (row["case_id"].as_str().unwrap(), row))
        .collect::<BTreeMap<_, _>>();
    let ledger = closure.iter().map(|id| {
        let row = rows.get(id.as_str()).expect("verified dependency registry closure");
        let status = row["status"].as_str().unwrap();
        let pending = planned.planned.pending.iter().find(|pending| &pending.case_id == id);
        let outcome = if status == "implemented" { if pending.is_some() { "unavailable" } else { "ready" } } else { status };
        let reason = pending.map(|pending| pending.reason_code.clone()).unwrap_or_else(|| format!("case_{status}"));
        let mut dependencies = match bundle.manifest().cases.iter().find(|case| &case.case_id == id) {
            Some(case) => case.dependencies.clone(),
            None if status != "implemented" => vec![],
            None => unreachable!("verified implemented case closure"),
        };
        dependencies.sort();
        json!({"case_id":id,"selection":if direct.contains(id){"direct"}else{"dependency"},"dependency_case_ids":dependencies,"registry_status":status,"outcome":outcome,"reason_code":if outcome=="ready"{Value::Null}else{json!(reason)},"artifact_paths":[]})
    }).collect::<Vec<_>>();
    let preview = CapturedPlanningOutcome {
        plan: planned.planned.plan.clone(),
        selector: selector.clone(),
        selection_ledger: ledger.clone(),
        identities: identities.clone(),
        validation: "not_run",
        publication: "not_run",
    };
    if request.dry_run {
        return Ok(CapturedCorpusOutcome::Planned(preview));
    }
    if planned.planned.plan.artifacts.is_empty() {
        if ledger.iter().any(|entry| entry["outcome"] == "ready") {
            return Err(CapturedCorpusError::Planning(
                "ready selection has no executable artifacts".into(),
            ));
        }
        return Ok(CapturedCorpusOutcome::NoExecutableCases(preview));
    }
    let parent = request
        .destination
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| std::path::Path::new("."));
    std::fs::create_dir_all(parent).map_err(|e| CapturedCorpusError::Execution(e.to_string()))?;
    let destination = parent
        .canonicalize()
        .map_err(|e| CapturedCorpusError::Execution(e.to_string()))?
        .join(request.destination.file_name().ok_or_else(|| {
            CapturedCorpusError::Input("destination requires a final name".into())
        })?);
    let plan = planned.planned.plan.clone();
    let services = crate::curated_execution::CuratedExecutionServiceFactory::new(&planned.planned);
    let projector = CapturedManifestProjector {
        bundle,
        resources,
        planned,
        identities,
        ledger,
        selector,
        before_publication,
        run: crate::PreparedGenerationRun {
            profile,
            include_stress,
            seed: request.seed,
            out_dir: destination.clone(),
            manifest_path: destination.join("manifest.json"),
        },
    };
    let result = CorpusExecutor::new(services, projector)
        .execute(
            &plan,
            destination,
            request.parallelism,
            &request.cancellation,
        )
        .map_err(|e| CapturedCorpusError::Execution(e.to_string()))?;
    Ok(CapturedCorpusOutcome::Published(result))
}

struct CapturedManifestProjector {
    bundle: Arc<CorpusDefinitionBundle>,
    resources: EngineResources,
    planned: CapturedCuratedPlan,
    identities: crate::identity::ManifestIdentityProjection,
    ledger: Vec<Value>,
    selector: Value,
    run: crate::PreparedGenerationRun,
    before_publication: Arc<dyn Fn() -> Result<(), ManifestProjectionError> + Send + Sync>,
}

impl ManifestProjector for CapturedManifestProjector {
    fn project(&self, input: &ManifestProjectionInput) -> Result<Vec<u8>, ManifestProjectionError> {
        let err = |e: String| ManifestProjectionError(e);
        let entries = crate::curated_manifest::project_curated_file_entries(
            &self.planned.planned.projection,
            input,
        )
        .map_err(|e| err(e.to_string()))?;
        let mut qualifications = crate::curated_manifest::project_curated_stress_qualifications(
            &self.planned.planned.projection,
            input,
        )
        .map_err(|e| err(e.to_string()))?;
        qualifications.extend(
            crate::curated_manifest::project_curated_qualifications(input)
                .map_err(|e| err(e.to_string()))?,
        );
        let mut completed = qualifications
            .iter()
            .filter_map(|q| q["case_id"].as_str())
            .map(str::to_owned)
            .collect::<BTreeSet<_>>();
        let mut paths = BTreeMap::<String, Vec<String>>::new();
        for entry in &entries {
            let id = entry["case_id"]
                .as_str()
                .ok_or_else(|| err("file case ID missing".into()))?;
            paths.entry(id.into()).or_default().push(
                entry["path"]
                    .as_str()
                    .ok_or_else(|| err("file path missing".into()))?
                    .into(),
            );
            completed.insert(id.into());
        }
        let mut ledger = self.ledger.clone();
        for row in &mut ledger {
            let id = row["case_id"].as_str().unwrap().to_owned();
            if row["outcome"] == "ready" {
                if !completed.contains(&id) {
                    return Err(err(format!(
                        "selected ready case lacks terminal evidence: {id}"
                    )));
                }
                row["outcome"] = json!("generated");
                row["reason_code"] = Value::Null;
                let mut owned = paths.remove(&id).unwrap_or_default();
                owned.sort();
                row["artifact_paths"] = json!(owned);
            }
        }
        let mut identities = self.identities.clone();
        identities.external_runtime = crate::terminal_external_runtime_identities(input)?;
        if identities.corpus_definition.identity.as_ref() != Some(self.bundle.identity()) {
            return Err(err(
                "verified corpus identity changed before publication".into()
            ));
        }
        let standards_bytes = self
            .resources
            .bytes("standards.lock.json")
            .map_err(|e| err(e.to_string()))?;
        let standards: Value =
            serde_json::from_slice(&standards_bytes).map_err(|e| err(e.to_string()))?;
        let cargo = self
            .resources
            .bytes("Cargo.lock")
            .map_err(|e| err(e.to_string()))?;
        let legacy = self
            .resources
            .legacy_identity_v1()
            .map_err(|e| err(e.to_string()))?;
        let files = entries
            .into_iter()
            .map(|manifest_entry| crate::GeneratedFile {
                case_id: manifest_entry["case_id"].as_str().unwrap().into(),
                manifest_entry,
            })
            .collect();
        // Reuse established file/standards serialization, but never invoke the
        // embedded profile-wide skipping policy on caller-owned registry data.
        let mut manifest = crate::build_generation_manifest(
            &self.run,
            &standards,
            &standards_bytes,
            &cargo,
            &json!({"cases":[]}),
            &legacy,
            &identities,
            files,
            qualifications,
            &[],
            &[],
        )
        .map_err(|e| err(e.to_string()))?;
        manifest.as_object_mut().unwrap().remove("skipped_cases");
        manifest["manifest_schema_version"] = json!("2.0.0");
        manifest["run"]["kind"] = json!("external_corpus");
        manifest["run"]["selector"] = self.selector.clone();
        manifest["selection_ledger"] = json!(ledger);
        crate::manifest_contract::validate_external_corpus_manifest(&manifest)
            .map_err(|e| err(e.to_string()))?;
        (self.before_publication)()?;
        let mut bytes = serde_json::to_vec_pretty(&manifest).map_err(|e| err(e.to_string()))?;
        bytes.push(b'\n');
        Ok(bytes)
    }
}

#[cfg(test)]
mod captured_runner_tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct Workspace(PathBuf);
    impl Workspace {
        fn new() -> Self {
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let root = std::env::temp_dir().join(format!(
                "synth-dicom-gen-captured-run-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&root).unwrap();
            Self(root)
        }
    }
    impl Drop for Workspace {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }
    fn bundle() -> Arc<CorpusDefinitionBundle> {
        static BUNDLE: std::sync::OnceLock<Arc<CorpusDefinitionBundle>> =
            std::sync::OnceLock::new();
        BUNDLE
            .get_or_init(|| {
                let workspace = Workspace::new();
                let root = workspace.0.join("definition");
                assert!(
                    std::process::Command::new("python3")
                        .arg(concat!(
                            env!("CARGO_MANIFEST_DIR"),
                            "/scripts/build-current-corpus-definition-bundle.py"
                        ))
                        .arg(&root)
                        .status()
                        .unwrap()
                        .success()
                );
                Arc::new(CorpusDefinitionBundle::load(&root).unwrap())
                // Workspace is removed before any planning or execution.
            })
            .clone()
    }
    fn request(root: PathBuf) -> CapturedCorpusRequest {
        CapturedCorpusRequest {
            selection: CorpusSelection::Profile {
                profile: "smoke".into(),
                include_stress: false,
            },
            destination: root,
            seed: 1,
            parallelism: 3,
            dry_run: false,
            cancellation: CancellationToken::new(),
        }
    }
    fn published(outcome: CapturedCorpusOutcome) -> CorpusExecutionResult {
        match outcome {
            CapturedCorpusOutcome::Published(result) => result,
            _ => panic!("expected published corpus"),
        }
    }

    #[test]
    fn smoke_publication_is_verified_deterministic_and_reader_bounded() {
        let workspace = Workspace::new();
        let bundle = bundle();
        let resources = EngineResources::embedded();
        let first = published(
            run_captured_corpus(
                bundle.clone(),
                resources.clone(),
                request(workspace.0.join("parallel")),
            )
            .unwrap(),
        );
        let mut sequential = request(workspace.0.join("sequential"));
        sequential.parallelism = 1;
        let second =
            published(run_captured_corpus(bundle.clone(), resources.clone(), sequential).unwrap());
        let manifest: Value = serde_json::from_slice(&first.manifest_bytes).unwrap();
        let other: Value = serde_json::from_slice(&second.manifest_bytes).unwrap();
        assert_eq!(manifest["manifest_schema_version"], "2.0.0");
        assert_eq!(
            manifest["identity_projection"]["corpus_definition"]["identity"],
            serde_json::to_value(bundle.identity()).unwrap()
        );
        assert_eq!(
            manifest["identity_projection"]["external_runtime"],
            json!([])
        );
        assert_eq!(manifest["selection_ledger"].as_array().unwrap().len(), 3);
        for (case, hash) in [
            (
                "mono1_u8_explicit_le",
                "76dc5208b139899fcb87bbf7ec9edf1a323000a91c4015de9ef8bde7bd344ecc",
            ),
            (
                "mono2_u8_explicit_le",
                "fce766bcbb4b4aa79cfb3fa0c3b5e4ef888b11c0708fad713b9cde8d41ec6a15",
            ),
            (
                "rgb_planar0_explicit_le",
                "33de9448509431fda27005cbf83c79977f1c3ebadb669ae1dedf1a225742f3c5",
            ),
        ] {
            let file = manifest["files"]
                .as_array()
                .unwrap()
                .iter()
                .find(|f| f["case_id"].as_str().unwrap().ends_with(case))
                .unwrap();
            assert_eq!(file["sha256"], hash);
            let path = file["path"].as_str().unwrap();
            assert_eq!(
                fs::read(first.destination.join(path)).unwrap(),
                fs::read(second.destination.join(path)).unwrap()
            );
        }
        assert_eq!(manifest["selection_ledger"], other["selection_ledger"]);
        assert!(
            crate::validate_generated_root_with_resources(&first.destination, &resources)
                .unwrap()
                .failures
                .is_empty()
        );
        assert!(
            crate::build_coverage_report_with_resources(&first.destination, &resources)
                .unwrap_err()
                .to_string()
                .contains("not yet supported")
        );
        let sdk = crate::sdk::DicomTestSuite::embedded().unwrap();
        assert!(
            sdk.validate(crate::sdk::ValidateRequest::new(&first.destination))
                .unwrap_err()
                .to_string()
                .contains("not yet supported")
        );
        assert!(
            sdk.report(crate::sdk::ReportRequest::new(&first.destination))
                .unwrap_err()
                .to_string()
                .contains("not yet supported")
        );
    }

    #[test]
    fn selectors_dry_run_and_nonexecutable_outcomes_never_publish() {
        let workspace = Workspace::new();
        let bundle = bundle();
        let mut dry = request(workspace.0.join("dry"));
        dry.dry_run = true;
        let CapturedCorpusOutcome::Planned(preview) =
            run_captured_corpus(bundle.clone(), EngineResources::embedded(), dry).unwrap()
        else {
            panic!("expected plan")
        };
        assert_eq!(preview.validation, "not_run");
        assert_eq!(preview.publication, "not_run");
        assert!(preview.identities.corpus_definition.identity.is_some());
        let legacy = crate::curated_plan::CuratedScCorpusPlanProvider::load(
            crate::curated_plan::CuratedCatalogPaths::from_repository_root(env!(
                "CARGO_MANIFEST_DIR"
            )),
        )
        .unwrap()
        .plan(&CuratedScPlanRequest {
            selection: CuratedScSelection::Profile {
                profile: "smoke".into(),
                include_stress: false,
            },
            seed: 1,
            max_parallelism: 3,
        })
        .unwrap();
        assert_eq!(
            serde_json::to_value(&preview.plan).unwrap(),
            serde_json::to_value(legacy.plan).unwrap()
        );
        assert!(!workspace.0.join("dry").exists());
        for ids in [
            vec![],
            vec!["classic/sc/mono1_u8_explicit_le".into(); 2],
            vec!["unknown/case".into()],
        ] {
            let mut invalid = request(workspace.0.join("invalid"));
            invalid.selection = CorpusSelection::CaseIds {
                profile: "smoke".into(),
                include_stress: false,
                case_ids: ids,
            };
            assert!(
                run_captured_corpus(bundle.clone(), EngineResources::embedded(), invalid).is_err()
            );
        }
        let registry: Value =
            serde_json::from_slice(bundle.bytes(&bundle.manifest().registry.path).unwrap())
                .unwrap();
        let planned = registry["cases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["status"] == "planned")
            .unwrap();
        let id = planned["case_id"].as_str().unwrap();
        let profile = planned["profiles"][0].as_str().unwrap();
        let mut no = request(workspace.0.join("no-executable"));
        no.selection = CorpusSelection::CaseIds {
            profile: profile.into(),
            include_stress: false,
            case_ids: vec![id.into()],
        };
        let CapturedCorpusOutcome::NoExecutableCases(result) =
            run_captured_corpus(bundle, EngineResources::embedded(), no).unwrap()
        else {
            panic!("planned-only must not publish")
        };
        assert_eq!(result.validation, "not_run");
        assert_eq!(result.publication, "not_run");
        assert!(
            result
                .selection_ledger
                .iter()
                .any(|row| row["case_id"] == id && row["outcome"] == "planned")
        );
        assert!(fs::read_dir(&workspace.0).unwrap().next().is_none());
    }

    #[test]
    fn captured_runner_is_independent_of_working_directory() {
        let workspace = Workspace::new();
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "corpus_generation::captured_runner_tests::smoke_publication_is_verified_deterministic_and_reader_bounded"])
            .current_dir(&workspace.0).output().unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn explicit_selection_preserves_dependencies_and_mixed_statuses() {
        let workspace = Workspace::new();
        let bundle = bundle();
        let mut mixed = request(workspace.0.join("mixed"));
        mixed.selection = CorpusSelection::CaseIds {
            profile: "all".into(),
            include_stress: false,
            case_ids: vec![
                "classic/sc/mono1_u8_explicit_le".into(),
                "classic/dx/mono2_u12_jpeg_extended".into(),
            ],
        };
        let result = published(
            run_captured_corpus(bundle.clone(), EngineResources::embedded(), mixed).unwrap(),
        );
        let manifest: Value = serde_json::from_slice(&result.manifest_bytes).unwrap();
        assert_eq!(manifest["selection_ledger"].as_array().unwrap().len(), 2);
        assert_eq!(manifest["files"].as_array().unwrap().len(), 1);
        assert!(
            manifest["selection_ledger"]
                .as_array()
                .unwrap()
                .iter()
                .any(|r| r["outcome"] == "planned")
        );
        let mut dependency = request(workspace.0.join("dependency"));
        dependency.selection = CorpusSelection::CaseIds {
            profile: "all".into(),
            include_stress: false,
            case_ids: vec!["derived/registration/spatial_ct_pair".into()],
        };
        let result = published(
            run_captured_corpus(bundle.clone(), EngineResources::embedded(), dependency).unwrap(),
        );
        let manifest: Value = serde_json::from_slice(&result.manifest_bytes).unwrap();
        assert!(
            manifest["selection_ledger"]
                .as_array()
                .unwrap()
                .iter()
                .any(|r| r["selection"] == "dependency")
        );
        for row in manifest["selection_ledger"].as_array().unwrap() {
            let mut expected = bundle
                .manifest()
                .cases
                .iter()
                .find(|c| c.case_id == row["case_id"].as_str().unwrap())
                .unwrap()
                .dependencies
                .clone();
            expected.sort();
            assert_eq!(row["dependency_case_ids"], json!(expected));
        }
        let mut unavailable = request(workspace.0.join("unavailable"));
        unavailable.selection = CorpusSelection::CaseIds {
            profile: "all".into(),
            include_stress: false,
            case_ids: vec!["classic/sc/mono2_u16_jpeg2000_lossless".into()],
        };
        let CapturedCorpusOutcome::NoExecutableCases(result) =
            run_captured_corpus(bundle.clone(), EngineResources::embedded(), unavailable).unwrap()
        else {
            panic!("uncompiled codec must remain unavailable")
        };
        assert!(
            result
                .selection_ledger
                .iter()
                .all(|r| r["outcome"] == "unavailable")
        );
        assert!(!workspace.0.join("unavailable").exists());
        for (profile, include_stress, ids) in [
            (
                "smoke",
                false,
                vec!["derived/registration/spatial_ct_pair".into()],
            ),
            ("unknown", false, vec![]),
            ("smoke", true, vec![]),
        ] {
            assert!(
                selection(
                    &bundle,
                    &CorpusSelection::CaseIds {
                        profile: profile.into(),
                        include_stress,
                        case_ids: ids
                    }
                )
                .is_err()
            );
        }
        let mut invalid = request(workspace.0.join("zero"));
        invalid.parallelism = 0;
        assert!(run_captured_corpus(bundle, EngineResources::embedded(), invalid).is_err());
    }

    #[test]
    fn transaction_failure_cancellation_and_existing_destination_are_atomic() {
        let workspace = Workspace::new();
        let bundle = bundle();
        let cancelled = request(workspace.0.join("cancelled"));
        cancelled.cancellation.cancel();
        assert!(matches!(
            run_captured_corpus(bundle.clone(), EngineResources::embedded(), cancelled),
            Err(CapturedCorpusError::Cancelled)
        ));
        let failure = run_with_publication_check(
            bundle.clone(),
            EngineResources::embedded(),
            request(workspace.0.join("failed")),
            Arc::new(|| {
                Err(ManifestProjectionError(
                    "injected publication refusal".into(),
                ))
            }),
        );
        assert!(failure.is_err());
        assert!(fs::read_dir(&workspace.0).unwrap().next().is_none());
        let final_cancel = request(workspace.0.join("final-cancel"));
        let token = final_cancel.cancellation.clone();
        assert!(
            run_with_publication_check(
                bundle.clone(),
                EngineResources::embedded(),
                final_cancel,
                Arc::new(move || {
                    token.cancel();
                    Ok(())
                })
            )
            .is_err()
        );
        assert!(fs::read_dir(&workspace.0).unwrap().next().is_none());
        fs::create_dir(workspace.0.join("existing")).unwrap();
        fs::write(workspace.0.join("existing/sentinel"), b"retained").unwrap();
        assert!(
            run_captured_corpus(
                bundle,
                EngineResources::embedded(),
                request(workspace.0.join("existing"))
            )
            .is_err()
        );
        assert_eq!(
            fs::read(workspace.0.join("existing/sentinel")).unwrap(),
            b"retained"
        );
    }
}
