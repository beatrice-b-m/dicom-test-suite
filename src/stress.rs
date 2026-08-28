//! Policy primitives for opt-in stress generation.
//!
//! This module owns no DICOM payloads and performs no file I/O. It describes
//! the approved reduced and full scales, checks resource observations against
//! their envelopes, and produces payload-free qualification records. Writers
//! remain responsible for streaming data and discarding staging output unless
//! [`StressQualificationRecord::is_promotable`] returns `true`.

use std::error::Error;
use std::fmt;

use serde_json::{Value, json};

pub const STRESS_CONTRACT_VERSION: &str = "0.1.0";

const MIB: u64 = 1024 * 1024;
const GIB: u64 = 1024 * MIB;

/// The only approved stress job classes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StressScale {
    /// Boundary-sized variants suitable for an explicitly selected CI job.
    Reduced,
    /// Scheduled or release variants; never part of ordinary CI.
    Full,
}

/// Aggregate and per-case limits approved for a stress job class.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StressEnvelope {
    pub output_bytes: u64,
    pub peak_rss_bytes: u64,
    pub case_wall_milliseconds: u64,
    pub job_wall_milliseconds: u64,
}

impl StressScale {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Reduced => "reduced",
            Self::Full => "full",
        }
    }

    pub const fn envelope(self) -> StressEnvelope {
        match self {
            Self::Reduced => StressEnvelope {
                output_bytes: 256 * MIB,
                peak_rss_bytes: 512 * MIB,
                case_wall_milliseconds: 2 * 60 * 1000,
                job_wall_milliseconds: 10 * 60 * 1000,
            },
            Self::Full => StressEnvelope {
                output_bytes: 8 * GIB,
                peak_rss_bytes: 2 * GIB,
                case_wall_milliseconds: 30 * 60 * 1000,
                job_wall_milliseconds: 2 * 60 * 60 * 1000,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StressRecipeKind {
    EncapsulatedEot,
    EnhancedCt,
    CtStudy,
    NativeBulkData,
    NestedSequences,
    LongMetadata,
    WsiPyramid,
}

impl StressRecipeKind {
    pub const fn name(self) -> &'static str {
        match self {
            Self::EncapsulatedEot => "encapsulated_eot",
            Self::EnhancedCt => "enhanced_ct",
            Self::CtStudy => "ct_study",
            Self::NativeBulkData => "native_bulk_data",
            Self::NestedSequences => "nested_sequences",
            Self::LongMetadata => "long_metadata",
            Self::WsiPyramid => "wsi_pyramid",
        }
    }
}

/// Requested or measured scale values. Zero means the dimension does not
/// apply to the recipe. Output bytes are measured rather than requested.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StressScaleParameters {
    pub instances: u64,
    pub frames: u64,
    pub fragments: u64,
    pub payload_bytes: u64,
    pub output_bytes: u64,
    pub rows: u64,
    pub columns: u64,
    pub tile_rows: u64,
    pub tile_columns: u64,
    pub pyramid_levels: u64,
    pub sequence_depth: u64,
    pub metadata_values: u64,
}

/// One approved recipe at one approved scale.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StressRequest {
    pub contract_version: &'static str,
    pub recipe: StressRecipeKind,
    pub scale: StressScale,
    pub parameters: StressScaleParameters,
    /// A recipe-specific output cap when it is narrower than the job cap.
    pub recipe_output_ceiling: Option<u64>,
}

impl StressRequest {
    pub const fn approved(recipe: StressRecipeKind, scale: StressScale) -> Self {
        let parameters = match (recipe, scale) {
            (StressRecipeKind::EncapsulatedEot, StressScale::Reduced) => StressScaleParameters {
                frames: 256,
                fragments: 64,
                payload_bytes: 64 * MIB,
                ..StressScaleParameters::ZERO
            },
            (StressRecipeKind::EncapsulatedEot, StressScale::Full) => StressScaleParameters {
                frames: 2,
                fragments: 1024,
                payload_bytes: 4 * GIB + 64 * MIB,
                ..StressScaleParameters::ZERO
            },
            (StressRecipeKind::EnhancedCt, StressScale::Reduced) => StressScaleParameters {
                instances: 1,
                frames: 256,
                rows: 64,
                columns: 64,
                ..StressScaleParameters::ZERO
            },
            (StressRecipeKind::EnhancedCt, StressScale::Full) => StressScaleParameters {
                instances: 1,
                frames: 8192,
                rows: 64,
                columns: 64,
                ..StressScaleParameters::ZERO
            },
            (StressRecipeKind::CtStudy, StressScale::Reduced) => StressScaleParameters {
                instances: 128,
                frames: 128,
                rows: 64,
                columns: 64,
                ..StressScaleParameters::ZERO
            },
            (StressRecipeKind::CtStudy, StressScale::Full) => StressScaleParameters {
                instances: 2048,
                frames: 2048,
                rows: 64,
                columns: 64,
                ..StressScaleParameters::ZERO
            },
            (StressRecipeKind::NativeBulkData, StressScale::Reduced) => StressScaleParameters {
                payload_bytes: 64 * MIB,
                ..StressScaleParameters::ZERO
            },
            (StressRecipeKind::NativeBulkData, StressScale::Full) => StressScaleParameters {
                payload_bytes: GIB,
                ..StressScaleParameters::ZERO
            },
            (StressRecipeKind::NestedSequences, StressScale::Reduced) => StressScaleParameters {
                payload_bytes: 16 * MIB,
                sequence_depth: 32,
                ..StressScaleParameters::ZERO
            },
            (StressRecipeKind::NestedSequences, StressScale::Full) => StressScaleParameters {
                payload_bytes: 128 * MIB,
                sequence_depth: 256,
                ..StressScaleParameters::ZERO
            },
            (StressRecipeKind::LongMetadata, StressScale::Reduced) => StressScaleParameters {
                payload_bytes: MIB,
                metadata_values: 1024,
                ..StressScaleParameters::ZERO
            },
            (StressRecipeKind::LongMetadata, StressScale::Full) => StressScaleParameters {
                payload_bytes: 64 * MIB,
                metadata_values: 65_536,
                ..StressScaleParameters::ZERO
            },
            (StressRecipeKind::WsiPyramid, StressScale::Reduced) => StressScaleParameters {
                instances: 3,
                rows: 1024,
                columns: 1024,
                tile_rows: 256,
                tile_columns: 256,
                pyramid_levels: 3,
                ..StressScaleParameters::ZERO
            },
            (StressRecipeKind::WsiPyramid, StressScale::Full) => StressScaleParameters {
                instances: 5,
                rows: 16_384,
                columns: 16_384,
                tile_rows: 256,
                tile_columns: 256,
                pyramid_levels: 5,
                ..StressScaleParameters::ZERO
            },
        };
        let recipe_output_ceiling = match (recipe, scale) {
            (StressRecipeKind::WsiPyramid, StressScale::Full) => Some(512 * MIB),
            _ => None,
        };
        Self {
            contract_version: STRESS_CONTRACT_VERSION,
            recipe,
            scale,
            parameters,
            recipe_output_ceiling,
        }
    }
}

impl StressScaleParameters {
    const ZERO: Self = Self {
        instances: 0,
        frames: 0,
        fragments: 0,
        payload_bytes: 0,
        output_bytes: 0,
        rows: 0,
        columns: 0,
        tile_rows: 0,
        tile_columns: 0,
        pyramid_levels: 0,
        sequence_depth: 0,
        metadata_values: 0,
    };

    fn satisfies(self, requested: Self) -> bool {
        self.instances == requested.instances
            && self.frames == requested.frames
            && self.fragments == requested.fragments
            && self.payload_bytes == requested.payload_bytes
            && self.rows == requested.rows
            && self.columns == requested.columns
            && self.tile_rows == requested.tile_rows
            && self.tile_columns == requested.tile_columns
            && self.pyramid_levels == requested.pyramid_levels
            && self.sequence_depth == requested.sequence_depth
            && self.metadata_values == requested.metadata_values
            && self.output_bytes > 0
    }

    pub fn to_json(self) -> Value {
        json!({
            "instances": self.instances,
            "frames": self.frames,
            "fragments": self.fragments,
            "payload_bytes": self.payload_bytes,
            "output_bytes": self.output_bytes,
            "rows": self.rows,
            "columns": self.columns,
            "tile_rows": self.tile_rows,
            "tile_columns": self.tile_columns,
            "pyramid_levels": self.pyramid_levels,
            "sequence_depth": self.sequence_depth,
            "metadata_values": self.metadata_values
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResourceObservation {
    pub output_bytes: u64,
    pub elapsed_milliseconds: u64,
    /// `None` means the platform could not expose peak RSS.
    pub peak_rss_bytes: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceLimit {
    OutputBytes,
    PeakRssBytes,
    CaseWallMilliseconds,
    JobWallMilliseconds,
    RecipeOutputBytes,
    ArithmeticOverflow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LimitViolation {
    pub limit: ResourceLimit,
    pub observed: u64,
    pub ceiling: u64,
}

impl fmt::Display for LimitViolation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "stress resource {:?} observed {} exceeds ceiling {}",
            self.limit, self.observed, self.ceiling
        )
    }
}

impl Error for LimitViolation {}

/// Stateful guard for deterministic preflight and aggregate job accounting.
#[derive(Debug, Clone)]
pub struct StressResourceGuard {
    envelope: StressEnvelope,
    total_output_bytes: u64,
    total_elapsed_milliseconds: u64,
}

impl StressResourceGuard {
    pub const fn new(scale: StressScale) -> Self {
        Self {
            envelope: scale.envelope(),
            total_output_bytes: 0,
            total_elapsed_milliseconds: 0,
        }
    }

    /// Reject a case before generation if its declared upper bounds cannot fit.
    pub fn preflight(
        &self,
        request: StressRequest,
        planned_output_bytes: u64,
        planned_peak_rss_bytes: u64,
    ) -> Result<(), LimitViolation> {
        check_limit(
            ResourceLimit::OutputBytes,
            planned_output_bytes,
            self.envelope
                .output_bytes
                .saturating_sub(self.total_output_bytes),
        )?;
        check_limit(
            ResourceLimit::PeakRssBytes,
            planned_peak_rss_bytes,
            self.envelope.peak_rss_bytes,
        )?;
        if let Some(ceiling) = request.recipe_output_ceiling {
            check_limit(
                ResourceLimit::RecipeOutputBytes,
                planned_output_bytes,
                ceiling,
            )?;
        }
        Ok(())
    }

    /// Account for a finished case. Callers must discard staging output on an
    /// error; counters are changed only after every check succeeds.
    pub fn record_case(
        &mut self,
        request: StressRequest,
        observation: ResourceObservation,
    ) -> Result<(), LimitViolation> {
        check_limit(
            ResourceLimit::CaseWallMilliseconds,
            observation.elapsed_milliseconds,
            self.envelope.case_wall_milliseconds,
        )?;
        if let Some(peak_rss_bytes) = observation.peak_rss_bytes {
            check_limit(
                ResourceLimit::PeakRssBytes,
                peak_rss_bytes,
                self.envelope.peak_rss_bytes,
            )?;
        }
        if let Some(ceiling) = request.recipe_output_ceiling {
            check_limit(
                ResourceLimit::RecipeOutputBytes,
                observation.output_bytes,
                ceiling,
            )?;
        }
        let next_output = checked_sum(
            self.total_output_bytes,
            observation.output_bytes,
            ResourceLimit::OutputBytes,
        )?;
        check_limit(
            ResourceLimit::OutputBytes,
            next_output,
            self.envelope.output_bytes,
        )?;
        let next_elapsed = checked_sum(
            self.total_elapsed_milliseconds,
            observation.elapsed_milliseconds,
            ResourceLimit::JobWallMilliseconds,
        )?;
        check_limit(
            ResourceLimit::JobWallMilliseconds,
            next_elapsed,
            self.envelope.job_wall_milliseconds,
        )?;
        self.total_output_bytes = next_output;
        self.total_elapsed_milliseconds = next_elapsed;
        Ok(())
    }

    pub const fn totals(&self) -> (u64, u64) {
        (self.total_output_bytes, self.total_elapsed_milliseconds)
    }
}

fn check_limit(limit: ResourceLimit, observed: u64, ceiling: u64) -> Result<(), LimitViolation> {
    if observed > ceiling {
        Err(LimitViolation {
            limit,
            observed,
            ceiling,
        })
    } else {
        Ok(())
    }
}

fn checked_sum(left: u64, right: u64, limit: ResourceLimit) -> Result<u64, LimitViolation> {
    left.checked_add(right).ok_or(LimitViolation {
        limit: ResourceLimit::ArithmeticOverflow,
        observed: u64::MAX,
        ceiling: match limit {
            ResourceLimit::JobWallMilliseconds => u64::MAX,
            _ => u64::MAX,
        },
    })
}

/// Outcomes are separate so timeout, interruption, and process failure cannot
/// be mistaken for a clean resource-guard rejection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StressExecutionOutcome {
    Completed,
    RefusedByPreflight(LimitViolation),
    AbortedByGuard(LimitViolation),
    Interrupted,
    TimedOut,
    ProcessFailure {
        exit_code: Option<i32>,
        signal: Option<i32>,
    },
}

/// Payload-free evidence projected into the stress report and manifest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StressQualificationRecord {
    pub contract_version: &'static str,
    pub request: StressRequest,
    pub actual: StressScaleParameters,
    pub observation: ResourceObservation,
    pub outcome: StressExecutionOutcome,
}

impl StressQualificationRecord {
    /// Only a complete, exact-scale, non-empty result can leave staging.
    pub fn is_promotable(self) -> bool {
        let envelope = self.request.scale.envelope();
        self.contract_version == STRESS_CONTRACT_VERSION
            && self.request.contract_version == STRESS_CONTRACT_VERSION
            && self.request == StressRequest::approved(self.request.recipe, self.request.scale)
            && self.outcome == StressExecutionOutcome::Completed
            && self.actual.satisfies(self.request.parameters)
            && self.actual.output_bytes == self.observation.output_bytes
            && self.observation.output_bytes <= envelope.output_bytes
            && self.observation.elapsed_milliseconds <= envelope.case_wall_milliseconds
            && self
                .observation
                .peak_rss_bytes
                .is_none_or(|peak| peak <= envelope.peak_rss_bytes)
            && self
                .request
                .recipe_output_ceiling
                .is_none_or(|ceiling| self.actual.output_bytes <= ceiling)
    }

    /// Project the checked record into the payload-free manifest contract.
    /// Callers must reject non-promotable records before publishing output.
    pub fn to_manifest_value(self, case_id: &str) -> Value {
        let envelope = self.request.scale.envelope();
        json!({
            "case_id": case_id,
            "kind": "stress_case_run",
            "contract_version": self.contract_version,
            "profile": "stress",
            "recipe": self.request.recipe.name(),
            "scale": self.request.scale.name(),
            "requested": self.request.parameters.to_json(),
            "actual": self.actual.to_json(),
            "resource_envelope": {
                "output_bytes": envelope.output_bytes,
                "peak_rss_bytes": envelope.peak_rss_bytes,
                "case_wall_milliseconds": envelope.case_wall_milliseconds,
                "job_wall_milliseconds": envelope.job_wall_milliseconds,
                "recipe_output_bytes": self.request.recipe_output_ceiling
            },
            "observation": {
                "output_bytes": self.observation.output_bytes,
                "elapsed_milliseconds": self.observation.elapsed_milliseconds,
                "peak_rss_bytes": self.observation.peak_rss_bytes
            },
            "outcome": match self.outcome {
                StressExecutionOutcome::Completed => "completed",
                StressExecutionOutcome::RefusedByPreflight(_) => "refused_by_preflight",
                StressExecutionOutcome::AbortedByGuard(_) => "aborted_by_guard",
                StressExecutionOutcome::Interrupted => "interrupted",
                StressExecutionOutcome::TimedOut => "timed_out",
                StressExecutionOutcome::ProcessFailure { .. } => "process_failure",
            },
            "payload_policy": "generated_payloads_uncommitted",
            "status": if self.is_promotable() { "passed" } else { "failed" }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn approved_envelopes_lock_policy_values() {
        assert_eq!(StressScale::Reduced.envelope().output_bytes, 256 * MIB);
        assert_eq!(StressScale::Reduced.envelope().peak_rss_bytes, 512 * MIB);
        assert_eq!(
            StressScale::Reduced.envelope().case_wall_milliseconds,
            120_000
        );
        assert_eq!(
            StressScale::Full.envelope(),
            StressEnvelope {
                output_bytes: 8 * GIB,
                peak_rss_bytes: 2 * GIB,
                case_wall_milliseconds: 1_800_000,
                job_wall_milliseconds: 7_200_000,
            }
        );
    }

    #[test]
    fn all_recipe_scales_lock_the_approved_boundaries() {
        let eot = StressRequest::approved(StressRecipeKind::EncapsulatedEot, StressScale::Full);
        assert_eq!(eot.parameters.payload_bytes, 4 * GIB + 64 * MIB);
        assert_eq!(eot.parameters.frames, 2);
        assert_eq!(eot.parameters.fragments, 1024);

        let enhanced = StressRequest::approved(StressRecipeKind::EnhancedCt, StressScale::Reduced);
        assert_eq!(
            (enhanced.parameters.frames, enhanced.parameters.rows),
            (256, 64)
        );

        let study = StressRequest::approved(StressRecipeKind::CtStudy, StressScale::Full);
        assert_eq!(study.parameters.instances, 2048);

        let bulk = StressRequest::approved(StressRecipeKind::NativeBulkData, StressScale::Full);
        assert_eq!(bulk.parameters.payload_bytes, GIB);

        let nested = StressRequest::approved(StressRecipeKind::NestedSequences, StressScale::Full);
        assert_eq!(
            (
                nested.parameters.sequence_depth,
                nested.parameters.payload_bytes
            ),
            (256, 128 * MIB)
        );

        let metadata =
            StressRequest::approved(StressRecipeKind::LongMetadata, StressScale::Reduced);
        assert_eq!(
            (
                metadata.parameters.metadata_values,
                metadata.parameters.payload_bytes
            ),
            (1024, MIB)
        );

        let wsi = StressRequest::approved(StressRecipeKind::WsiPyramid, StressScale::Full);
        assert_eq!(
            (wsi.parameters.rows, wsi.parameters.columns),
            (16_384, 16_384)
        );
        assert_eq!(
            (wsi.parameters.tile_rows, wsi.parameters.pyramid_levels),
            (256, 5)
        );
        assert_eq!(wsi.recipe_output_ceiling, Some(512 * MIB));
    }

    #[test]
    fn preflight_rejects_job_and_recipe_specific_limits() {
        let guard = StressResourceGuard::new(StressScale::Full);
        let wsi = StressRequest::approved(StressRecipeKind::WsiPyramid, StressScale::Full);
        assert_eq!(
            guard.preflight(wsi, 512 * MIB + 1, GIB).unwrap_err().limit,
            ResourceLimit::RecipeOutputBytes
        );
        assert_eq!(
            guard
                .preflight(wsi, 256 * MIB, 2 * GIB + 1)
                .unwrap_err()
                .limit,
            ResourceLimit::PeakRssBytes
        );
    }

    #[test]
    fn failed_accounting_does_not_mutate_job_totals() {
        let mut guard = StressResourceGuard::new(StressScale::Reduced);
        let request =
            StressRequest::approved(StressRecipeKind::NativeBulkData, StressScale::Reduced);
        guard
            .record_case(
                request,
                ResourceObservation {
                    output_bytes: 64 * MIB,
                    elapsed_milliseconds: 60_000,
                    peak_rss_bytes: None,
                },
            )
            .unwrap();
        assert_eq!(guard.totals(), (64 * MIB, 60_000));
        let error = guard
            .record_case(
                request,
                ResourceObservation {
                    output_bytes: 1,
                    elapsed_milliseconds: 120_001,
                    peak_rss_bytes: Some(1),
                },
            )
            .unwrap_err();
        assert_eq!(error.limit, ResourceLimit::CaseWallMilliseconds);
        assert_eq!(guard.totals(), (64 * MIB, 60_000));
    }

    #[test]
    fn aggregate_output_and_time_are_bounded() {
        let mut output_guard = StressResourceGuard::new(StressScale::Reduced);
        let request =
            StressRequest::approved(StressRecipeKind::NativeBulkData, StressScale::Reduced);
        let observation = ResourceObservation {
            output_bytes: 64 * MIB,
            elapsed_milliseconds: 1,
            peak_rss_bytes: None,
        };
        for _ in 0..4 {
            output_guard.record_case(request, observation).unwrap();
        }
        assert_eq!(
            output_guard
                .record_case(request, observation)
                .unwrap_err()
                .limit,
            ResourceLimit::OutputBytes
        );

        let mut time_guard = StressResourceGuard::new(StressScale::Reduced);
        let timed = ResourceObservation {
            output_bytes: 1,
            elapsed_milliseconds: 120_000,
            peak_rss_bytes: None,
        };
        for _ in 0..5 {
            time_guard.record_case(request, timed).unwrap();
        }
        assert_eq!(
            time_guard.record_case(request, timed).unwrap_err().limit,
            ResourceLimit::JobWallMilliseconds
        );
    }

    fn completed_record() -> StressQualificationRecord {
        let request = StressRequest::approved(StressRecipeKind::EnhancedCt, StressScale::Reduced);
        let mut actual = request.parameters;
        actual.output_bytes = 2 * MIB;
        StressQualificationRecord {
            contract_version: STRESS_CONTRACT_VERSION,
            request,
            actual,
            observation: ResourceObservation {
                output_bytes: actual.output_bytes,
                elapsed_milliseconds: 1_000,
                peak_rss_bytes: Some(32 * MIB),
            },
            outcome: StressExecutionOutcome::Completed,
        }
    }

    #[test]
    fn only_complete_exact_scale_records_are_promotable() {
        let complete = completed_record();
        assert!(complete.is_promotable());

        let mut interrupted = complete;
        interrupted.outcome = StressExecutionOutcome::Interrupted;
        assert!(!interrupted.is_promotable());

        let mut partial = complete;
        partial.actual.frames -= 1;
        assert!(!partial.is_promotable());

        let mut inconsistent = complete;
        inconsistent.observation.output_bytes += 1;
        assert!(!inconsistent.is_promotable());

        let mut over_budget = complete;
        over_budget.observation.elapsed_milliseconds =
            StressScale::Reduced.envelope().case_wall_milliseconds + 1;
        assert!(!over_budget.is_promotable());

        let mut forged_request = complete;
        forged_request.request.parameters.frames = 1;
        forged_request.actual.frames = 1;
        assert!(!forged_request.is_promotable());
    }

    #[test]
    fn manifest_projection_records_requested_actual_and_resource_evidence() {
        let record = completed_record();
        let value = record.to_manifest_value("stress/enhanced-ct/many_frames");
        assert_eq!(value["kind"], "stress_case_run");
        assert_eq!(value["recipe"], "enhanced_ct");
        assert_eq!(value["scale"], "reduced");
        assert_eq!(value["requested"]["frames"], 256);
        assert_eq!(value["requested"]["output_bytes"], 0);
        assert_eq!(value["actual"]["output_bytes"], 2 * MIB);
        assert_eq!(value["observation"]["peak_rss_bytes"], 32 * MIB);
        assert_eq!(value["status"], "passed");
    }

    #[test]
    fn bounded_failures_remain_distinct() {
        let violation = LimitViolation {
            limit: ResourceLimit::PeakRssBytes,
            observed: 2,
            ceiling: 1,
        };
        assert_ne!(
            StressExecutionOutcome::RefusedByPreflight(violation),
            StressExecutionOutcome::AbortedByGuard(violation)
        );
        assert_ne!(
            StressExecutionOutcome::TimedOut,
            StressExecutionOutcome::ProcessFailure {
                exit_code: None,
                signal: Some(9),
            }
        );
    }
}
