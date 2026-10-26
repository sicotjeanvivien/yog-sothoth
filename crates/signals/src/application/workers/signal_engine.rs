//! The signal engine: one poll loop per detector, each on its own
//! [`interval`](SignalDetector::interval), recomputing from the database and persisting what it
//! emits. A failed tick is logged and stepped over; only a task panic reaches [`EngineError`].

use std::collections::HashMap;
use std::sync::Arc;

use chrono::{Duration as ChronoDuration, Utc};
use solana_pubkey::Pubkey;
use thiserror::Error;
use tokio::task::{Id, JoinError, JoinSet};
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};
use yog_bootstrap::{SHUTDOWN_GRACE, TaskEnd};

use yog_core::domain::{EvalContext, Severity, Signal, SignalDetector, SignalRepository};

mod metrics;

pub(crate) use metrics::EngineMetrics;

/// Loop-level failure of the engine; per-tick failures are skipped and logged inside the loop.
#[derive(Debug, Error)]
pub enum EngineError {
    /// A detector's poll loop panicked: a real bug, detectors skipping and logging by design.
    #[error("a detector task panicked: {0}")]
    DetectorPanicked(String),
}

/// Runs a set of detectors, each on its own cadence, and persists what they emit, through core
/// traits only.
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

    /// Spawn one poll loop per detector and run until `shutdown`, then wait for them within
    /// [`SHUTDOWN_GRACE`]: the detectors still running then, by name, or the first panic, after
    /// cancelling the rest.
    pub async fn run(self, shutdown: CancellationToken) -> Result<Vec<&'static str>, EngineError> {
        if self.detectors.is_empty() {
            warn!("signal engine started with no detectors — nothing to do");
        }

        let mut set = JoinSet::new();
        // ⚠️ Named at spawn: a task stuck in its tick never returns its name.
        let mut running: HashMap<Id, &'static str> = HashMap::new();
        for detector in self.detectors {
            let repository = Arc::clone(&self.signal_repository);
            let shutdown = shutdown.clone();
            let name = detector.name();
            let task = set.spawn(run_detector_loop(detector, repository, shutdown));
            running.insert(task.id(), name);
        }

        loop {
            tokio::select! {
                biased;
                _ = shutdown.cancelled() => break,
                joined = set.join_next_with_id() => match joined {
                    Some(joined) => settle(joined, &mut running, &shutdown, &mut set).await?,
                    None => return Ok(Vec::new()),
                },
            }
        }

        let deadline = Instant::now() + SHUTDOWN_GRACE;
        while let Ok(Some(joined)) =
            tokio::time::timeout_at(deadline, set.join_next_with_id()).await
        {
            settle(joined, &mut running, &shutdown, &mut set).await?;
        }
        let mut overran: Vec<&'static str> = running.into_values().collect();
        overran.sort_unstable();
        Ok(overran)
    }
}

/// One detector loop's end: logged and struck off `running`, or the engine stopped on a panic.
async fn settle(
    joined: Result<(Id, ()), JoinError>,
    running: &mut HashMap<Id, &'static str>,
    shutdown: &CancellationToken,
    set: &mut JoinSet<()>,
) -> Result<(), EngineError> {
    match joined {
        Ok((id, ())) => {
            let name = running.remove(&id).unwrap_or("unknown");
            info!(detector = name, "detector loop stopped");
        }
        // ⚠️ A `JoinError` is a panic or a cancellation ([`TaskEnd`]), and only the first is a
        // failure, and the reading must not default to panic.
        Err(e) => {
            let name = running.remove(&e.id()).unwrap_or("unknown");
            match TaskEnd::from(&e) {
                TaskEnd::Panicked => {
                    error!(detector = name, error = %e, "detector task panicked — stopping engine");
                    shutdown.cancel();
                    set.shutdown().await;
                    return Err(EngineError::DetectorPanicked(e.to_string()));
                }
                TaskEnd::Cancelled => {
                    debug!(detector = name, error = %e, "detector task was cancelled before it could stop");
                }
            }
        }
    }
    Ok(())
}

/// One detector's interval loop. Returns when `shutdown` is cancelled.
async fn run_detector_loop(
    detector: Arc<dyn SignalDetector>,
    repository: Arc<dyn SignalRepository>,
    shutdown: CancellationToken,
) {
    let name = detector.name();
    let mut ticker = tokio::time::interval(detector.interval());
    info!(detector = name, interval = ?detector.interval(), "detector started");

    loop {
        tokio::select! {
            // ⚠️ `biased`: with the default `Burst`, a slow tick leaves `tick()` ready, and an
            // unbiased pick would run a new tick after the stop, which `run` waits for without deadline.
            biased;

            _ = shutdown.cancelled() => {
                info!(detector = name, "shutdown requested — detector stopping");
                return;
            }
            _ = ticker.tick() => run_tick(detector.as_ref(), repository.as_ref(), name).await,
        }
    }
}

/// A single tick: evaluate, then persist, absorbing every recoverable error.
async fn run_tick(
    detector: &dyn SignalDetector,
    repository: &dyn SignalRepository,
    name: &'static str,
) {
    let ctx = EvalContext {
        evaluated_at: Utc::now(),
    };

    let candidates = match detector.evaluate(&ctx).await {
        Ok(candidates) => candidates,
        Err(e) => {
            warn!(detector = name, error = %e, "evaluation failed — skipping tick");
            EngineMetrics::record_tick(name, "eval_failed");
            return;
        }
    };

    if candidates.is_empty() {
        EngineMetrics::record_tick(name, "ok");
        return;
    }

    // Dedup: drop pools already signalled within the cooldown window, unless
    // the candidate escalates to a higher severity.
    let since = ctx.evaluated_at
        - ChronoDuration::from_std(detector.cooldown()).unwrap_or_else(|_| ChronoDuration::zero());
    let recent = match repository.latest_severity_by_pool(name, since).await {
        Ok(recent) => recent,
        Err(e) => {
            warn!(detector = name, error = %e, "dedup lookup failed — skipping tick");
            EngineMetrics::record_tick(name, "dedup_failed");
            return;
        }
    };

    let to_emit = emittable(candidates, &recent);
    if to_emit.is_empty() {
        EngineMetrics::record_tick(name, "suppressed");
        return;
    }

    let count = to_emit.len();
    if let Err(e) = repository.insert_batch(&to_emit).await {
        warn!(detector = name, error = %e, "signal persistence failed — skipping tick");
        EngineMetrics::record_tick(name, "persist_failed");
        return;
    }

    EngineMetrics::record_emitted(name, count);
    EngineMetrics::record_tick(name, "ok");
    info!(detector = name, count, "signals emitted");
}

/// Keep only candidates that are new or escalate: a candidate is dropped when a signal of equal
/// or higher severity for its pool is already within the cooldown (`recent`).
fn emittable(candidates: Vec<Signal>, recent: &HashMap<Pubkey, Severity>) -> Vec<Signal> {
    candidates
        .into_iter()
        .filter(|candidate| match recent.get(&candidate.pool_address) {
            None => true,
            Some(last) => candidate.severity > *last,
        })
        .collect()
}

#[cfg(test)]
#[path = "signal_engine/signal_engine_tests.rs"]
mod tests;
