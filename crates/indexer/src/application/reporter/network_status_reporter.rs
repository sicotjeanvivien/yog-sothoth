//! Network status reporter — periodically records the indexer's view
//! of the Solana chain (current slot + RPC round-trip latency) into
//! the `network_status` singleton.
//!
//! Responsibility split (mirrors the pipeline stages):
//! - `run` owns the tick loop and the shutdown semantics.
//! - `record_snapshot` performs one tick: time the RPC call, persist.
//!
//! Error semantics:
//! - **A failed tick is logged, counted and skipped** — `tick` swallows it,
//!   `yog_indexer_network_status_tick_failures_total{reason}` counts it, and
//!   the next tick tries again. `run` cannot fail: it returns
//!   `Result<(), Infallible>`, so propagating a tick error does not compile.
//! - **The decision to stop the daemon belongs to the data path alone.** This
//!   probe reads the same endpoint over the same network as ingestion, so when
//!   the link drops it fails by construction — usually before the subscription
//!   worker has spent its first backoff. Until 11 September 2026 it propagated
//!   that first failure, `Daemon::run` stopped everything, and every restart
//!   reset the worker's retry budget to 1: nine restarts in two minutes, and a
//!   budget of ~5 minutes never consumed.
//! - What is left of that noise: a `warn!` per failed tick and the counter —
//!   and **nothing a reader of the dashboard can see**, which is not what this
//!   module claimed until the review of PR #141. The stale `observed_at` it
//!   leaves behind has no reader; see
//!   [`NetworkStatusReporterMetrics::record_tick_failure`] for why, and for
//!   what would close it. A panic still stops the daemon — that is a bug, not
//!   a network.
//!
//! Placement rationale:
//! - This lives in the indexer, not in a separate daemon, because it
//!   measures the health of the indexer's own RPC link. The indexer
//!   already owns an `RpcClient`; no new dependency is introduced.

use std::convert::Infallible;
use std::sync::Arc;
use std::time::{Duration, Instant};

use solana_rpc_client::nonblocking::rpc_client::RpcClient;
use tokio::time::MissedTickBehavior;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};
use yog_bootstrap::SecretUrl;

use yog_core::domain::{NetworkStatus, NetworkStatusRepository};

use crate::application::reporter::{NetworkStatusReporterError, NetworkStatusReporterMetrics};

/// How often the reporter records a snapshot.
///
/// The dashboard sidebar polls every ~10s; recording every 15s keeps
/// the stored slot at most ~15s stale — imperceptible on a 9-digit
/// slot counter — while keeping RPC and DB load negligible.
const TICK_INTERVAL: Duration = Duration::from_secs(15);

/// Periodic reporter of the indexer's chain-link health.
///
/// Generic over the repository so it can be unit-tested with a mock;
/// the daemon wires the concrete `PgNetworkStatusRepository`.
pub(crate) struct NetworkStatusReporter {
    rpc_client: Arc<RpcClient>,
    /// Kept to scrub the endpoint back out of `solana-client` error strings —
    /// see [`crate::infra::TransactionFetcher`], same reason, same boundary.
    rpc_url: SecretUrl,
    repository: Arc<dyn NetworkStatusRepository>,
}

impl NetworkStatusReporter {
    /// Build the reporter over the shared RPC client and the
    /// network-status repository.
    pub(crate) fn new(
        rpc_client: Arc<RpcClient>,
        rpc_url: SecretUrl,
        repository: Arc<dyn NetworkStatusRepository>,
    ) -> Self {
        Self {
            rpc_client,
            rpc_url,
            repository,
        }
    }

    /// Drive the tick loop until the shutdown token is triggered.
    ///
    /// The first tick fires immediately (tokio's `interval` yields at
    /// once on the first `tick()`), so the singleton is refreshed as
    /// soon as the daemon starts rather than after the first delay.
    ///
    /// ⚠️ **`Infallible` is the guard, not a formality.** A `?` on a tick is the
    /// defect this signature exists to forbid, and a test could only notice it
    /// once it was back; the type refuses to compile it. The `Result` is kept so
    /// the task joins through `handle_task_result` like every other one.
    pub(crate) async fn run(self, shutdown: CancellationToken) -> Result<(), Infallible> {
        info!("NetworkStatusReporter started");

        let mut ticker = tokio::time::interval(TICK_INTERVAL);
        // ⚠️ **Not the default `Burst`.** A failed tick is now survived, and on a
        // link that times out rather than refuses, one `getSlot` blocks for the
        // client's 30 s — twice the interval. Under `Burst` every tick missed
        // meanwhile fires back to back when the link returns: seen on 11
        // September 2026, three failures logged in the same millisecond after a
        // two-minute cut, and ~60 calls for a 30-minute outage, all on the
        // endpoint the subscription worker is resubscribing to. `Delay` fires
        // one and resumes the cadence from there.
        ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);

        loop {
            tokio::select! {
                _ = ticker.tick() => self.tick().await,
                _ = shutdown.cancelled() => {
                    info!("shutdown requested — network status reporter stopping");
                    return Ok(());
                }
            }
        }
    }

    /// One tick of the loop: record a snapshot, and absorb its failure.
    ///
    /// Returns nothing, deliberately — there is nothing a caller could do with
    /// the error that the next tick does not already do.
    async fn tick(&self) {
        if let Err(error) = self.record_snapshot().await {
            NetworkStatusReporterMetrics::record_tick_failure(error.reason());
            warn!(%error, "network status tick failed — skipped, the next tick retries");
        }
    }

    /// Perform one tick: time the `getSlot` call, then persist the
    /// resulting snapshot.
    ///
    /// Any failure is returned typed; `tick` counts it.
    async fn record_snapshot(&self) -> Result<(), NetworkStatusReporterError> {
        // Time the RPC round-trip — this elapsed value IS the
        // reported latency.
        let started = Instant::now();
        let slot = self
            .rpc_client
            .get_slot()
            .await
            .map_err(|e| NetworkStatusReporterError::Rpc(self.rpc_url.scrub(&e.to_string())))?;
        let elapsed_ms = started.elapsed().as_millis();

        // `as_millis` is u128; the domain model uses u32. A getSlot
        // round-trip is never anywhere near u32::MAX ms — clamp
        // defensively rather than risk a panic.
        let rpc_latency_ms = u32::try_from(elapsed_ms).unwrap_or(u32::MAX);

        let status = NetworkStatus {
            slot,
            rpc_latency_ms,
            observed_at: chrono::Utc::now(),
        };

        self.repository
            .upsert(&status)
            .await
            .map_err(NetworkStatusReporterError::Persistence)?;

        debug!(slot, rpc_latency_ms, "network status snapshot recorded");
        Ok(())
    }
}

#[cfg(test)]
#[path = "network_status_reporter_tests.rs"]
mod tests;
