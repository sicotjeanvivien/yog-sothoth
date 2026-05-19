//! Network status reporter — periodically records the indexer's view
//! of the Solana chain (current slot + RPC round-trip latency) into
//! the `network_status` singleton.
//!
//! Responsibility split (mirrors the pipeline stages):
//! - `run` owns the tick loop and the shutdown semantics.
//! - `record_snapshot` performs one tick: time the RPC call, persist.
//!
//! Error semantics:
//! - This reporter is supervised like a pipeline stage. A failed tick
//!   (RPC call failed, persistence failed) is propagated as
//!   `NetworkStatusReporterError` and bubbles up to `Daemon::run` via
//!   `handle_task_result`, which stops the daemon.
//! - This is a deliberate choice: the reporter does not self-heal.
//!   Resilience (retry / respawn) is expected to come from the same
//!   future respawn logic planned for the subscription workers.
//!
//! Placement rationale:
//! - This lives in the indexer, not in a separate daemon, because it
//!   measures the health of the indexer's own RPC link. The indexer
//!   already owns an `RpcClient`; no new dependency is introduced.

use std::sync::Arc;
use std::time::{Duration, Instant};

use solana_rpc_client::nonblocking::rpc_client::RpcClient;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info};

use yog_core::domain::{NetworkStatus, NetworkStatusRepository};

use crate::application::reporter::NetworkStatusReporterError;

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
    repository: Arc<dyn NetworkStatusRepository>,
}

impl NetworkStatusReporter {
    /// Build the reporter over the shared RPC client and the
    /// network-status repository.
    pub(crate) fn new(
        rpc_client: Arc<RpcClient>,
        repository: Arc<dyn NetworkStatusRepository>,
    ) -> Self {
        Self {
            rpc_client,
            repository,
        }
    }

    /// Drive the tick loop until a tick fails or the shutdown token is
    /// triggered.
    ///
    /// The first tick fires immediately (tokio's `interval` yields at
    /// once on the first `tick()`), so the singleton is refreshed as
    /// soon as the daemon starts rather than after the first delay.
    pub(crate) async fn run(
        self,
        shutdown: CancellationToken,
    ) -> Result<(), NetworkStatusReporterError> {
        info!("NetworkStatusReporter started");

        let mut ticker = tokio::time::interval(TICK_INTERVAL);

        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    self.record_snapshot().await?;
                }
                _ = shutdown.cancelled() => {
                    info!("shutdown requested — network status reporter stopping");
                    return Ok(());
                }
            }
        }
    }

    /// Perform one tick: time the `getSlot` call, then persist the
    /// resulting snapshot.
    ///
    /// Any failure is returned typed; `run` propagates it.
    async fn record_snapshot(&self) -> Result<(), NetworkStatusReporterError> {
        // Time the RPC round-trip — this elapsed value IS the
        // reported latency.
        let started = Instant::now();
        let slot = self
            .rpc_client
            .get_slot()
            .await
            .map_err(|e| NetworkStatusReporterError::Rpc(e.to_string()))?;
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

        // `?` converts RepositoryError into the reporter error via the
        // `#[from]` on the Persistence variant.
        self.repository.upsert(&status).await?;

        debug!(slot, rpc_latency_ms, "network status snapshot recorded");
        Ok(())
    }
}
