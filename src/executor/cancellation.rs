use std::fmt;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub struct CancellationToken {
    state: Arc<Mutex<CancellationState>>,
}

#[derive(Debug, Default, PartialEq, Eq)]
struct CancellationState {
    reason: Option<String>,
}

impl Default for CancellationToken {
    fn default() -> Self {
        Self {
            state: Arc::new(Mutex::new(CancellationState::default())),
        }
    }
}

impl CancellationToken {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) -> bool {
        self.cancel_with_reason("requested")
    }

    /// Cancel the run, retaining the first reason supplied by any caller.
    pub fn cancel_with_reason(&self, reason: impl Into<String>) -> bool {
        let reason = reason.into();
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        if state.reason.is_some() {
            return false;
        }
        state.reason = Some(if reason.trim().is_empty() {
            "requested".into()
        } else {
            reason
        });
        true
    }

    pub fn is_cancelled(&self) -> bool {
        self.state
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .reason
            .is_some()
    }

    pub fn reason(&self) -> Option<String> {
        self.state
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .reason
            .clone()
    }

    pub fn checkpoint(&self, point: CancellationPoint) -> Result<(), Cancelled> {
        match self.reason() {
            Some(reason) => Err(Cancelled { point, reason }),
            None => Ok(()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CancellationStage {
    BeforeExecution,
    BeforeProvider,
    BeforeCodec,
    BeforeMaterialization,
    BeforeValidation,
    BeforeManifest,
    BeforePromotion,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CancellationPoint {
    pub stage: CancellationStage,
    pub artifact_id: Option<String>,
}

impl CancellationPoint {
    pub fn run(stage: CancellationStage) -> Self {
        Self {
            stage,
            artifact_id: None,
        }
    }

    pub fn artifact(stage: CancellationStage, artifact_id: impl Into<String>) -> Self {
        Self {
            stage,
            artifact_id: Some(artifact_id.into()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cancelled {
    pub point: CancellationPoint,
    pub reason: String,
}

impl fmt::Display for Cancelled {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.point.artifact_id {
            Some(artifact_id) => write!(
                formatter,
                "execution cancelled at {:?} for {artifact_id}: {}",
                self.point.stage, self.reason
            ),
            None => write!(
                formatter,
                "execution cancelled at {:?}: {}",
                self.point.stage, self.reason
            ),
        }
    }
}

impl std::error::Error for Cancelled {}
