use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::mpsc;

use crate::corpus_plan::{CorpusPlan, CorpusPlanError, PlannedArtifact, ResourcePlan};
use crate::executor::cancellation::CancellationToken;

pub trait Cancellation: Sync {
    fn is_cancelled(&self) -> bool;
}

impl Cancellation for CancellationToken {
    fn is_cancelled(&self) -> bool {
        CancellationToken::is_cancelled(self)
    }
}

pub trait ArtifactWorker<T, E>: Sync {
    fn execute(
        &self,
        artifact: &PlannedArtifact,
        cancellation: &dyn Cancellation,
    ) -> Result<WorkerOutput<T>, E>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActualResourceUsage {
    pub output_bytes: u64,
    pub peak_working_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkerOutput<T> {
    pub value: T,
    pub resources: ActualResourceUsage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduledArtifact<T> {
    pub logical_id: String,
    pub order: u64,
    pub value: T,
    pub resources: ActualResourceUsage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResourceAccounting {
    pub artifact_count: u64,
    pub total_output_bytes: u64,
    pub peak_working_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduleOutcome<T> {
    pub artifacts: Vec<ScheduledArtifact<T>>,
    pub planned: ResourceAccounting,
    pub actual: ResourceAccounting,
    pub maximum_parallelism: u32,
}

#[derive(Debug)]
pub enum SchedulerError<E> {
    InvalidPlan(CorpusPlanError),
    ZeroParallelism,
    Cancelled,
    Worker {
        logical_id: String,
        source: E,
    },
    WorkerPanic {
        logical_id: String,
    },
    ResourceOverflow {
        phase: &'static str,
    },
    ResourceLimitExceeded {
        phase: &'static str,
        observed: ResourceAccounting,
        limits: ResourcePlan,
    },
    IncompleteGraph,
}

impl<E: fmt::Display> fmt::Display for SchedulerError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPlan(error) => write!(formatter, "invalid corpus plan: {error}"),
            Self::ZeroParallelism => formatter.write_str("scheduler parallelism must be non-zero"),
            Self::Cancelled => formatter.write_str("corpus execution was cancelled"),
            Self::Worker { logical_id, source } => {
                write!(
                    formatter,
                    "artifact worker failed for {logical_id}: {source}"
                )
            }
            Self::WorkerPanic { logical_id } => {
                write!(formatter, "artifact worker panicked for {logical_id}")
            }
            Self::ResourceOverflow { phase } => {
                write!(formatter, "{phase} resource accounting overflowed")
            }
            Self::ResourceLimitExceeded {
                phase,
                observed,
                limits,
            } => write!(
                formatter,
                "{phase} resources exceeded limits: observed={observed:?}, limits={limits:?}"
            ),
            Self::IncompleteGraph => {
                formatter.write_str("scheduler did not complete the artifact graph")
            }
        }
    }
}

impl<E: fmt::Debug + fmt::Display> std::error::Error for SchedulerError<E> {}

enum Completion<T, E> {
    Completed(Result<WorkerOutput<T>, E>),
    Panicked,
}

pub fn schedule<T, E, W>(
    plan: &CorpusPlan,
    requested_parallelism: u32,
    cancellation: &dyn Cancellation,
    worker: &W,
) -> Result<ScheduleOutcome<T>, SchedulerError<E>>
where
    T: Send,
    E: Send,
    W: ArtifactWorker<T, E>,
{
    if requested_parallelism == 0 {
        return Err(SchedulerError::ZeroParallelism);
    }
    let planned = planned_accounting(plan)?;
    check_limits("planned", planned, &plan.resources)?;
    plan.validate().map_err(SchedulerError::InvalidPlan)?;
    if cancellation.is_cancelled() {
        return Err(SchedulerError::Cancelled);
    }
    let worker_limit = usize::try_from(requested_parallelism.min(plan.resources.max_parallelism))
        .map_err(|_| SchedulerError::ResourceOverflow {
        phase: "parallelism",
    })?;

    let by_id = plan
        .artifacts
        .iter()
        .map(|artifact| (artifact.logical_id().to_owned(), artifact))
        .collect::<BTreeMap<_, _>>();
    let orders = plan
        .artifacts
        .iter()
        .map(|artifact| (artifact.logical_id().to_owned(), artifact.order()))
        .collect::<BTreeMap<_, _>>();
    let mut indegree = by_id
        .keys()
        .map(|id| (id.clone(), 0_usize))
        .collect::<BTreeMap<_, _>>();
    let mut dependents = by_id
        .keys()
        .map(|id| (id.clone(), BTreeSet::new()))
        .collect::<BTreeMap<_, _>>();
    let mut pairs = BTreeSet::new();
    for dependency in &plan.dependencies {
        if pairs.insert((
            dependency.artifact_id.clone(),
            dependency.depends_on.clone(),
        )) {
            *indegree
                .get_mut(&dependency.artifact_id)
                .expect("validated artifact") += 1;
            dependents
                .get_mut(&dependency.depends_on)
                .expect("validated dependency")
                .insert(dependency.artifact_id.clone());
        }
    }
    let mut ready = indegree
        .iter()
        .filter(|(_, degree)| **degree == 0)
        .map(|(id, _)| (orders[id], id.clone()))
        .collect::<BTreeSet<_>>();
    let mut results = Vec::with_capacity(plan.artifacts.len());
    let mut actual = ResourceAccounting {
        artifact_count: 0,
        total_output_bytes: 0,
        peak_working_bytes: 0,
    };
    let mut first_error = None;
    let mut halted = false;
    let mut maximum_active = 0_usize;

    std::thread::scope(|scope| {
        let (sender, receiver) = mpsc::channel();
        let mut active = 0_usize;
        let mut active_working_bytes = 0_u64;
        let mut peak_active_working_bytes = 0_u64;
        loop {
            if cancellation.is_cancelled() {
                halted = true;
                if first_error.is_none() {
                    first_error = Some(SchedulerError::Cancelled);
                }
            }
            while !halted && active < worker_limit {
                let Some((_, logical_id)) = ready.first().cloned() else {
                    break;
                };
                let artifact = by_id[&logical_id];
                let reserved_working_bytes = artifact.resource_estimate().peak_working_bytes;
                let Some(next_active_working_bytes) =
                    active_working_bytes.checked_add(reserved_working_bytes)
                else {
                    halted = true;
                    first_error.get_or_insert(SchedulerError::ResourceOverflow {
                        phase: "parallel admission",
                    });
                    break;
                };
                if next_active_working_bytes > plan.resources.max_peak_working_bytes {
                    break;
                }
                ready.pop_first();
                active_working_bytes = next_active_working_bytes;
                peak_active_working_bytes = peak_active_working_bytes.max(active_working_bytes);
                let sender = sender.clone();
                scope.spawn(move || {
                    let completion = match catch_unwind(AssertUnwindSafe(|| {
                        worker.execute(artifact, cancellation)
                    })) {
                        Ok(result) => Completion::Completed(result),
                        Err(_) => Completion::Panicked,
                    };
                    let _ = sender.send((logical_id, completion));
                });
                active += 1;
                maximum_active = maximum_active.max(active);
            }
            if active == 0 {
                break;
            }
            let (logical_id, completion) = receiver.recv().expect("active worker owns sender");
            active -= 1;
            active_working_bytes = active_working_bytes
                .checked_sub(by_id[&logical_id].resource_estimate().peak_working_bytes)
                .expect("active worker owns its working-byte reservation");
            match completion {
                Completion::Completed(Ok(output)) => {
                    let next_count = actual.artifact_count.checked_add(1);
                    let next_output = actual
                        .total_output_bytes
                        .checked_add(output.resources.output_bytes);
                    match (next_count, next_output) {
                        (Some(artifact_count), Some(total_output_bytes)) => {
                            actual = ResourceAccounting {
                                artifact_count,
                                total_output_bytes,
                                peak_working_bytes: peak_active_working_bytes,
                            };
                            if let Err(error) = check_limits("actual", actual, &plan.resources) {
                                halted = true;
                                first_error.get_or_insert(error);
                            } else {
                                results.push(ScheduledArtifact {
                                    logical_id: logical_id.clone(),
                                    order: orders[&logical_id],
                                    value: output.value,
                                    resources: output.resources,
                                });
                                for dependent in &dependents[&logical_id] {
                                    let degree =
                                        indegree.get_mut(dependent).expect("validated dependent");
                                    *degree -= 1;
                                    if *degree == 0 {
                                        ready.insert((orders[dependent], dependent.clone()));
                                    }
                                }
                            }
                        }
                        _ => {
                            halted = true;
                            first_error.get_or_insert(SchedulerError::ResourceOverflow {
                                phase: "actual",
                            });
                        }
                    }
                }
                Completion::Completed(Err(source)) => {
                    halted = true;
                    first_error.get_or_insert(SchedulerError::Worker { logical_id, source });
                }
                Completion::Panicked => {
                    halted = true;
                    first_error.get_or_insert(SchedulerError::WorkerPanic { logical_id });
                }
            }
        }
    });

    if let Some(error) = first_error {
        return Err(error);
    }
    if results.len() != plan.artifacts.len() {
        return Err(SchedulerError::IncompleteGraph);
    }
    results.sort_by(|left, right| {
        (left.order, &left.logical_id).cmp(&(right.order, &right.logical_id))
    });
    Ok(ScheduleOutcome {
        artifacts: results,
        planned,
        actual,
        maximum_parallelism: u32::try_from(maximum_active).map_err(|_| {
            SchedulerError::ResourceOverflow {
                phase: "parallelism",
            }
        })?,
    })
}

fn planned_accounting<E>(plan: &CorpusPlan) -> Result<ResourceAccounting, SchedulerError<E>> {
    let mut accounting = ResourceAccounting {
        artifact_count: 0,
        total_output_bytes: 0,
        peak_working_bytes: 0,
    };
    for artifact in &plan.artifacts {
        accounting.artifact_count = accounting
            .artifact_count
            .checked_add(1)
            .ok_or(SchedulerError::ResourceOverflow { phase: "planned" })?;
        accounting.total_output_bytes = accounting
            .total_output_bytes
            .checked_add(artifact.resource_estimate().output_bytes)
            .ok_or(SchedulerError::ResourceOverflow { phase: "planned" })?;
        accounting.peak_working_bytes = accounting
            .peak_working_bytes
            .max(artifact.resource_estimate().peak_working_bytes);
    }
    Ok(accounting)
}

fn check_limits<E>(
    phase: &'static str,
    observed: ResourceAccounting,
    limits: &ResourcePlan,
) -> Result<(), SchedulerError<E>> {
    if observed.artifact_count > limits.max_artifacts
        || observed.total_output_bytes > limits.max_total_output_bytes
        || observed.peak_working_bytes > limits.max_peak_working_bytes
    {
        return Err(SchedulerError::ResourceLimitExceeded {
            phase,
            observed,
            limits: limits.clone(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    use super::*;
    use crate::corpus_plan::{
        ArtifactDependency, ArtifactProvenance, ArtifactResourceEstimate,
        CORPUS_PLAN_SCHEMA_VERSION, EvidencePlan, OutputRelativePath, PlannedArtifact,
        PlannedQualification, PublicationPlan, PublicationTransaction, QualificationPayloadPolicy,
        ValidationPlan, ValidationRequirement, ValidationRule,
    };

    fn artifact(id: &str, order: u64, output_bytes: u64) -> PlannedArtifact {
        artifact_with_peak(id, order, output_bytes, 4)
    }

    fn artifact_with_peak(
        id: &str,
        order: u64,
        output_bytes: u64,
        peak_working_bytes: u64,
    ) -> PlannedArtifact {
        PlannedArtifact::Qualification(PlannedQualification {
            logical_id: id.into(),
            order,
            provenance: ArtifactProvenance::Requested,
            case_binding: None,
            profile: None,
            run_seed: None,
            qualification_kind: "test_qualification".into(),
            parameters: BTreeMap::new(),
            sources: vec![],
            payload_policy: QualificationPayloadPolicy::EvidenceOnly,
            validation: ValidationPlan {
                rules: vec![ValidationRule {
                    rule_id: "test_rule".into(),
                    requirement: ValidationRequirement::Required,
                    parameters: BTreeMap::new(),
                }],
            },
            evidence: EvidencePlan {
                obligations: Vec::new(),
            },
            resources: ArtifactResourceEstimate {
                output_bytes,
                peak_working_bytes,
            },
        })
    }

    fn plan(
        artifacts: Vec<PlannedArtifact>,
        dependencies: Vec<(&str, &str)>,
        resources: ResourcePlan,
    ) -> CorpusPlan {
        CorpusPlan {
            schema_version: CORPUS_PLAN_SCHEMA_VERSION.into(),
            seed: 1,
            artifacts,
            dependencies: dependencies
                .into_iter()
                .map(|(artifact_id, depends_on)| ArtifactDependency {
                    artifact_id: artifact_id.into(),
                    depends_on: depends_on.into(),
                    relationship: "test_dependency".into(),
                    frame_numbers: Vec::new(),
                })
                .collect(),
            unavailable: Vec::new(),
            publication: PublicationPlan {
                manifest_path: OutputRelativePath::new("manifest.json").unwrap(),
                transaction: PublicationTransaction::AtomicNoReplace,
                private_staging: true,
                no_overwrite: true,
            },
            resources,
        }
    }

    fn limits(max_parallelism: u32) -> ResourcePlan {
        ResourcePlan {
            max_artifacts: 16,
            max_total_output_bytes: 1024,
            max_peak_working_bytes: 64,
            max_parallelism,
        }
    }

    struct RecordingWorker {
        delays: BTreeMap<String, u64>,
        failures: BTreeSet<String>,
        panics: BTreeSet<String>,
        started: Mutex<Vec<String>>,
        active: AtomicUsize,
        maximum_active: AtomicUsize,
        actual_output_bytes: u64,
    }

    impl ArtifactWorker<String, &'static str> for RecordingWorker {
        fn execute(
            &self,
            artifact: &PlannedArtifact,
            _cancellation: &dyn Cancellation,
        ) -> Result<WorkerOutput<String>, &'static str> {
            let active = self.active.fetch_add(1, Ordering::SeqCst) + 1;
            self.maximum_active.fetch_max(active, Ordering::SeqCst);
            self.started
                .lock()
                .unwrap()
                .push(artifact.logical_id().into());
            std::thread::sleep(Duration::from_millis(
                self.delays.get(artifact.logical_id()).copied().unwrap_or(0),
            ));
            self.active.fetch_sub(1, Ordering::SeqCst);
            if self.panics.contains(artifact.logical_id()) {
                panic!("worker panic");
            }
            if self.failures.contains(artifact.logical_id()) {
                return Err("injected failure");
            }
            Ok(WorkerOutput {
                value: artifact.logical_id().into(),
                resources: ActualResourceUsage {
                    output_bytes: self.actual_output_bytes,
                    peak_working_bytes: 3,
                },
            })
        }
    }

    fn recording_worker() -> RecordingWorker {
        RecordingWorker {
            delays: BTreeMap::new(),
            failures: BTreeSet::new(),
            panics: BTreeSet::new(),
            started: Mutex::new(Vec::new()),
            active: AtomicUsize::new(0),
            maximum_active: AtomicUsize::new(0),
            actual_output_bytes: 1,
        }
    }

    #[test]
    fn dependency_ready_work_is_bounded_and_results_follow_explicit_order() {
        let corpus = plan(
            vec![
                artifact("d", 3, 1),
                artifact("b", 1, 1),
                artifact("c", 2, 1),
                artifact("a", 0, 1),
            ],
            vec![("c", "a"), ("d", "b")],
            limits(2),
        );
        let mut worker = recording_worker();
        worker.delays = BTreeMap::from([("a".into(), 30), ("b".into(), 2)]);
        let result = schedule(&corpus, 8, &CancellationToken::new(), &worker).unwrap();
        assert_eq!(
            result
                .artifacts
                .iter()
                .map(|artifact| artifact.logical_id.as_str())
                .collect::<Vec<_>>(),
            ["a", "b", "c", "d"]
        );
        assert!(worker.maximum_active.load(Ordering::SeqCst) <= 2);
        assert_eq!(result.planned.total_output_bytes, 4);
        assert_eq!(result.actual.total_output_bytes, 4);
    }

    #[test]
    fn parallel_dispatch_reserves_the_aggregate_working_set() {
        let corpus = plan(
            vec![
                artifact_with_peak("a", 0, 1, 40),
                artifact_with_peak("b", 1, 1, 40),
            ],
            Vec::new(),
            limits(2),
        );
        let mut worker = recording_worker();
        worker.delays = BTreeMap::from([("a".into(), 20), ("b".into(), 20)]);

        let result = schedule(&corpus, 2, &CancellationToken::new(), &worker).unwrap();

        assert_eq!(worker.maximum_active.load(Ordering::SeqCst), 1);
        assert_eq!(result.actual.peak_working_bytes, 40);
    }

    #[test]
    fn parallel_dispatch_uses_capacity_when_reservations_fit() {
        let corpus = plan(
            vec![
                artifact_with_peak("a", 0, 1, 32),
                artifact_with_peak("b", 1, 1, 32),
            ],
            Vec::new(),
            limits(2),
        );
        let mut worker = recording_worker();
        worker.delays = BTreeMap::from([("a".into(), 20), ("b".into(), 20)]);

        let result = schedule(&corpus, 2, &CancellationToken::new(), &worker).unwrap();

        assert_eq!(worker.maximum_active.load(Ordering::SeqCst), 2);
        assert_eq!(result.actual.peak_working_bytes, 64);
    }

    #[test]
    fn failure_stops_new_dependency_work() {
        let corpus = plan(
            vec![
                artifact("a", 0, 1),
                artifact("b", 1, 1),
                artifact("c", 2, 1),
                artifact("d", 3, 1),
            ],
            vec![("c", "a"), ("d", "b")],
            limits(2),
        );
        let mut worker = recording_worker();
        worker.failures.insert("a".into());
        worker.delays.insert("b".into(), 20);
        let error = schedule(&corpus, 2, &CancellationToken::new(), &worker).unwrap_err();
        assert!(matches!(error, SchedulerError::Worker { logical_id, .. } if logical_id == "a"));
        let started = worker
            .started
            .lock()
            .unwrap()
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        assert_eq!(started, BTreeSet::from(["a".into(), "b".into()]));
    }

    #[test]
    fn worker_panic_becomes_typed_error() {
        let corpus = plan(vec![artifact("a", 0, 1)], Vec::new(), limits(1));
        let mut worker = recording_worker();
        worker.panics.insert("a".into());
        assert!(matches!(
            schedule(&corpus, 1, &CancellationToken::new(), &worker),
            Err(SchedulerError::WorkerPanic { logical_id }) if logical_id == "a"
        ));
    }

    #[test]
    fn cancellation_before_dispatch_runs_no_worker() {
        let corpus = plan(vec![artifact("a", 0, 1)], Vec::new(), limits(1));
        let worker = recording_worker();
        let cancellation = CancellationToken::new();
        cancellation.cancel();
        assert!(matches!(
            schedule(&corpus, 1, &cancellation, &worker),
            Err(SchedulerError::Cancelled)
        ));
        assert!(worker.started.lock().unwrap().is_empty());
    }

    struct CancellingWorker(CancellationToken);
    impl ArtifactWorker<String, &'static str> for CancellingWorker {
        fn execute(
            &self,
            artifact: &PlannedArtifact,
            _cancellation: &dyn Cancellation,
        ) -> Result<WorkerOutput<String>, &'static str> {
            self.0.cancel_with_reason("test worker cancellation");
            Ok(WorkerOutput {
                value: artifact.logical_id().into(),
                resources: ActualResourceUsage {
                    output_bytes: 1,
                    peak_working_bytes: 1,
                },
            })
        }
    }

    #[test]
    fn cancellation_after_completion_stops_dependent_dispatch() {
        let corpus = plan(
            vec![artifact("a", 0, 1), artifact("b", 1, 1)],
            vec![("b", "a")],
            limits(1),
        );
        let cancellation = CancellationToken::new();
        assert!(matches!(
            schedule(
                &corpus,
                1,
                &cancellation,
                &CancellingWorker(cancellation.clone())
            ),
            Err(SchedulerError::Cancelled)
        ));
    }

    #[test]
    fn planned_resource_limit_and_overflow_are_typed() {
        let corpus = plan(
            vec![artifact("a", 0, 2)],
            Vec::new(),
            ResourcePlan {
                max_artifacts: 1,
                max_total_output_bytes: 1,
                max_peak_working_bytes: 64,
                max_parallelism: 1,
            },
        );
        assert!(matches!(
            schedule(&corpus, 1, &CancellationToken::new(), &recording_worker()),
            Err(SchedulerError::ResourceLimitExceeded {
                phase: "planned",
                ..
            })
        ));

        let corpus = plan(
            vec![artifact("a", 0, u64::MAX), artifact("b", 1, 1)],
            Vec::new(),
            ResourcePlan {
                max_artifacts: 2,
                max_total_output_bytes: u64::MAX,
                max_peak_working_bytes: 64,
                max_parallelism: 1,
            },
        );
        assert!(matches!(
            schedule(&corpus, 1, &CancellationToken::new(), &recording_worker()),
            Err(SchedulerError::ResourceOverflow { phase: "planned" })
        ));
    }

    #[test]
    fn actual_resource_limit_and_overflow_are_typed() {
        let corpus = plan(
            vec![artifact("a", 0, 1), artifact("b", 1, 1)],
            Vec::new(),
            ResourcePlan {
                max_artifacts: 2,
                max_total_output_bytes: u64::MAX,
                max_peak_working_bytes: 64,
                max_parallelism: 1,
            },
        );
        let mut worker = recording_worker();
        worker.actual_output_bytes = u64::MAX;
        assert!(matches!(
            schedule(&corpus, 1, &CancellationToken::new(), &worker),
            Err(SchedulerError::ResourceOverflow { phase: "actual" })
        ));

        let corpus = plan(vec![artifact("a", 0, 1)], Vec::new(), limits(1));
        let mut worker = recording_worker();
        worker.actual_output_bytes = 2048;
        assert!(matches!(
            schedule(&corpus, 1, &CancellationToken::new(), &worker),
            Err(SchedulerError::ResourceLimitExceeded {
                phase: "actual",
                ..
            })
        ));
    }
}
