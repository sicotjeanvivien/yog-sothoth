//! The signal engine — one poll loop per detector.
//!
//! Evaluation model (batch, per-detector cadence): each detector runs on its
//! own [`interval`](SignalDetector::interval); on every tick it recomputes
//! from a DB snapshot (read through its own repositories) and returns the
//! signals to raise. The engine persists them.
//!
//! Resilience follows the project's skip-and-log rule: a per-tick failure
//! (evaluation error, or a persistence error on the batch insert) is logged
//! and the tick is stepped over — one missed tick, never a dead loop. Only a
//! task panic (a genuine bug) is loop-level and bubbles up as [`EngineError`].

use std::sync::Arc;

use chrono::Utc;
use thiserror::Error;
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

use yog_core::domain::{EvalContext, SignalDetector, SignalRepository};

use crate::metrics::EngineMetrics;

/// Loop-level failure of the engine. Per-tick failures never reach here —
/// they are skipped-and-logged inside the loop.
#[derive(Debug, Error)]
pub enum EngineError {
    /// A detector's poll loop panicked. Detector logic is written to
    /// skip-and-log, so this signals a real bug, not a transient fault.
    #[error("a detector task panicked: {0}")]
    DetectorPanicked(String),
}

/// Runs a set of detectors, each on its own cadence, and persists what they
/// emit. Holds only core traits — the concrete repositories are injected by
/// the binary.
pub struct SignalEngine {
    signal_repository: Arc<dyn SignalRepository>,
    detectors: Vec<Arc<dyn SignalDetector>>,
}

impl SignalEngine {
    pub fn new(
        signal_repository: Arc<dyn SignalRepository>,
        detectors: Vec<Arc<dyn SignalDetector>>,
    ) -> Self {
        Self {
            signal_repository,
            detectors,
        }
    }

    /// Spawn one poll loop per detector and run until `shutdown` is
    /// cancelled. Returns `Ok(())` once every loop has stopped cleanly, or
    /// the first panic if a detector task blew up (after cancelling the rest).
    pub async fn run(self, shutdown: CancellationToken) -> Result<(), EngineError> {
        if self.detectors.is_empty() {
            warn!("signal engine started with no detectors — nothing to do");
        }

        let mut set = JoinSet::new();
        for detector in self.detectors {
            let repository = Arc::clone(&self.signal_repository);
            let shutdown = shutdown.clone();
            set.spawn(async move {
                let name = detector.name();
                detector_loop(detector, repository, shutdown).await;
                name
            });
        }

        while let Some(joined) = set.join_next().await {
            match joined {
                Ok(name) => info!(detector = name, "detector loop stopped"),
                Err(e) => {
                    error!(error = %e, "detector task panicked — stopping engine");
                    shutdown.cancel();
                    set.shutdown().await;
                    return Err(EngineError::DetectorPanicked(e.to_string()));
                }
            }
        }

        Ok(())
    }
}

/// One detector's interval loop. Returns when `shutdown` is cancelled.
async fn detector_loop(
    detector: Arc<dyn SignalDetector>,
    repository: Arc<dyn SignalRepository>,
    shutdown: CancellationToken,
) {
    let name = detector.name();
    let mut ticker = tokio::time::interval(detector.interval());
    info!(detector = name, interval = ?detector.interval(), "detector started");

    loop {
        tokio::select! {
            _ = ticker.tick() => run_tick(detector.as_ref(), repository.as_ref(), name).await,
            _ = shutdown.cancelled() => {
                info!(detector = name, "shutdown requested — detector stopping");
                return;
            }
        }
    }
}

/// A single tick: evaluate, then persist. Absorbs every recoverable error so
/// one hiccup never stops the loop.
async fn run_tick(
    detector: &dyn SignalDetector,
    repository: &dyn SignalRepository,
    name: &'static str,
) {
    let ctx = EvalContext {
        evaluated_at: Utc::now(),
    };

    let signals = match detector.evaluate(&ctx).await {
        Ok(signals) => signals,
        Err(e) => {
            warn!(detector = name, error = %e, "evaluation failed — skipping tick");
            EngineMetrics::record_tick(name, "eval_failed");
            return;
        }
    };

    if signals.is_empty() {
        EngineMetrics::record_tick(name, "ok");
        return;
    }

    let count = signals.len();
    if let Err(e) = repository.insert_batch(&signals).await {
        warn!(detector = name, error = %e, "signal persistence failed — skipping tick");
        EngineMetrics::record_tick(name, "persist_failed");
        return;
    }

    EngineMetrics::record_emitted(name, count);
    EngineMetrics::record_tick(name, "ok");
    info!(detector = name, count, "signals emitted");
}
