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
    DestinationExists,
    UnsafeDestination,
    OutputIo(std::io::Error),
    Execution(crate::executor::engine::CorpusExecutorError),
}

impl std::fmt::Display for CapturedCorpusError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for CapturedCorpusError {}

impl CapturedCorpusError {
    pub(crate) fn code(&self) -> &'static str {
        match self {
            Self::Input(_) => "request.schema.invalid",
            Self::Cancelled => "generation.execution.cancelled",
            Self::Planning(_) => "generation.planning.failed",
            Self::DestinationExists => "output.destination.exists",
            Self::UnsafeDestination => "output.path.unsafe",
            Self::OutputIo(_) => "io.write.failed",
            Self::Execution(error) => executor_error_code(error),
        }
    }
}

fn executor_error_code(error: &crate::executor::engine::CorpusExecutorError) -> &'static str {
    use crate::executor::engine::{ArtifactExecutionError as A, CorpusExecutorError as E};
    use crate::executor::scheduler::SchedulerError as S;
    use crate::executor::transaction::TransactionError as T;
    fn transaction(error: &T) -> &'static str {
        match error {
            T::DestinationExists(_) => "output.destination.exists",
            T::UnsafeDestination(_)
            | T::UnsafeStagingTarget(_)
            | T::UnsafeRelativePath(_)
            | T::UnsafeFilesystemEntry { .. } => "output.path.unsafe",
            T::PrimaryAndCleanup { .. } => "io.cleanup.failed",
            T::Io { .. } => "io.write.failed",
            _ => "internal.invariant.failed",
        }
    }
    match error {
        // Failed cleanup must not promise that cancellation cleanup completed.
        E::PrimaryAndCleanup { .. } => "io.cleanup.failed",
        E::Cancelled(_)
        | E::Scheduler(S::Cancelled)
        | E::Scheduler(S::Worker {
            source: A::Cancelled(_),
            ..
        }) => "generation.execution.cancelled",
        E::Scheduler(S::ResourceOverflow { .. } | S::ResourceLimitExceeded { .. }) => {
            "resource.limit.exceeded"
        }
        E::Transaction(error) => transaction(error),
        E::InvalidPlan(_) | E::EmptyPlan | E::Scheduler(S::InvalidPlan(_) | S::ZeroParallelism) => {
            "generation.planning.failed"
        }
        E::Manifest(_) => "validation.manifest.invalid",
        E::Service(_) | E::ServiceContract(_) => "generation.provider.failed",
        E::Scheduler(S::Worker {
            source: A::Service(_) | A::ServiceContract(_),
            ..
        }) => "generation.provider.failed",
        E::Scheduler(S::Worker {
            source: A::ValidationFailed { .. } | A::ObligationFailed(_),
            ..
        }) => "validation.artifact.failed",
        E::Scheduler(S::Worker {
            source: A::ResourceAccountingOverflow(_),
            ..
        }) => "resource.limit.exceeded",
        _ => "generation.materialization.failed",
    }
}

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
        Ok(_) => return Err(CapturedCorpusError::DestinationExists),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(CapturedCorpusError::OutputIo(e)),
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
        let reason = pending.map(|pending| pending.reason_code.clone()).unwrap_or_else(|| {
            row["skip"]["reason_code"].as_str().or_else(|| row["blockers"][0]["code"].as_str()).map(str::to_owned).unwrap_or_else(|| format!("case_{status}"))
        });
        let mut dependencies = match bundle.manifest().cases.iter().find(|case| &case.case_id == id) {
            Some(case) => case.dependencies.clone(),
            None if status != "implemented" => vec![],
            None => unreachable!("verified implemented case closure"),
        };
        dependencies.sort();
        json!({"case_id":id,"case_definition":row,"selection":if direct.contains(id){"direct"}else{"dependency"},"dependency_case_ids":dependencies,"registry_status":status,"outcome":outcome,"reason_code":if outcome=="ready"{Value::Null}else{json!(reason)},"artifact_paths":[]})
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
    std::fs::create_dir_all(parent).map_err(CapturedCorpusError::OutputIo)?;
    let destination = parent
        .canonicalize()
        .map_err(CapturedCorpusError::OutputIo)?
        .join(
            request
                .destination
                .file_name()
                .ok_or_else(|| CapturedCorpusError::UnsafeDestination)?,
        );
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
        .map_err(CapturedCorpusError::Execution)?;
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
#[path = "../tests/captured_corpus_generation.rs"]
mod captured_runner_tests;
