//! The `SignalDetector` contract and its evaluation context.
//!
//! Evaluation model (decided 1 July 2026): **batch, per-detector
//! cadence**. Each detector declares its own [`interval`] and, on every
//! tick, recomputes from a fresh DB snapshot read through *its own*
//! repositories — it holds no state between ticks (the database holds
//! the state). The engine owns the poll loops and persists whatever a
//! detector returns; the detector itself performs no writes.
//!
//! Streaming (event-callback, stateful) was deliberately not made the
//! substrate: a separate `signal-engine` process would need a transport
//! (LISTEN/NOTIFY or indexer coupling), and the first detectors are
//! windowed over already-bucketed caggs where sub-second reactivity buys
//! nothing. A `StreamDetector` can be added later as an extension.
//!
//! [`interval`]: SignalDetector::interval

use std::time::Duration;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use thiserror::Error;

use crate::RepositoryError;
use crate::domain::Signal;

/// Everything an evaluation tick hands to a detector.
///
/// Intentionally thin: under the "detector owns its repos" model the
/// data dependencies live in the detector itself (injected at
/// construction by the binary), so the context carries only the tick's
/// frozen clock. Kept as a struct — rather than a bare `DateTime` — so a
/// future field (e.g. a soft deadline) can be added without churning
/// every detector signature.
#[derive(Debug, Clone)]
pub struct EvalContext {
    /// The frozen "now" of this tick. Every signal produced in one
    /// evaluation shares it, so `triggered_at` is coherent and any time
    /// window is computed from a fixed point even if `evaluate` runs
    /// long.
    pub evaluated_at: DateTime<Utc>,
}

/// A rule that turns observed data into zero or more [`Signal`]s.
///
/// Implementors hold their own read-only repositories and are stateless
/// between ticks. The engine calls [`evaluate`] on each detector's own
/// [`interval`]; a failing tick is skipped-and-logged, never fatal.
///
/// [`evaluate`]: SignalDetector::evaluate
/// [`interval`]: SignalDetector::interval
#[async_trait]
pub trait SignalDetector: Send + Sync {
    /// Stable snake_case tag identifying this detector. Persisted verbatim
    /// as the `detector` column of every signal it produces, so it must
    /// stay constant across releases.
    fn name(&self) -> &'static str;

    /// This detector's evaluation cadence — how often the engine ticks it.
    fn interval(&self) -> Duration;

    /// Evaluate against a fresh snapshot (read through the detector's own
    /// repositories) and return any signals to raise. An empty vec means
    /// "nothing noteworthy this tick" — the common case.
    async fn evaluate(&self, ctx: &EvalContext) -> Result<Vec<Signal>, DetectorError>;
}

/// Failure of a single detector tick. Typed at the boundary so the engine
/// can skip-and-log per detector without aborting the loop.
#[derive(Debug, Error)]
pub enum DetectorError {
    /// A read from one of the detector's source repositories failed.
    /// A `?` on any repository call maps here via `From`.
    #[error("detector read failed: {0}")]
    Repository(#[from] RepositoryError),

    /// The detector read successfully but could not evaluate the data
    /// (unexpected shape, missing token decimals, arithmetic domain
    /// error, …). A logic/data problem, not a transient backend one.
    #[error("detector evaluation failed: {0}")]
    Evaluation(String),
}
