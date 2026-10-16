//! The `SignalDetector` contract: batch evaluation, each detector on its own
//! cadence over a fresh snapshot, stateless between ticks; the engine persists.

use std::time::Duration;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use thiserror::Error;

use crate::RepositoryError;
use crate::domain::Signal;

/// What a tick hands to a detector; its data comes through its own repositories.
#[derive(Debug, Clone)]
pub struct EvalContext {
    /// The frozen "now" of this tick, shared by every signal it produces.
    pub evaluated_at: DateTime<Utc>,
}

/// A rule that turns observed data into zero or more [`Signal`]s; a failing
/// tick is skipped and logged, never fatal.
#[async_trait]
pub trait SignalDetector: Send + Sync {
    /// Stable snake_case tag, persisted as the `detector` column.
    /// ⚠️ Constant across releases: renaming it splits the detector's history.
    fn name(&self) -> &'static str;

    /// How often the engine ticks this detector.
    fn interval(&self) -> Duration;

    /// Minimum time before the same `(detector, pool)` re-emits, unless the
    /// new signal escalates to a higher severity.
    fn cooldown(&self) -> Duration;

    /// Evaluate a fresh snapshot and return the signals to raise.
    async fn evaluate(&self, ctx: &EvalContext) -> Result<Vec<Signal>, DetectorError>;
}

/// Failure of a single detector tick.
#[derive(Debug, Error)]
pub enum DetectorError {
    /// A read from one of the detector's repositories failed.
    #[error("detector read failed: {0}")]
    Repository(#[from] RepositoryError),

    /// The data was read but could not be evaluated: a data problem, not a backend one.
    #[error("detector evaluation failed: {0}")]
    Evaluation(String),
}
