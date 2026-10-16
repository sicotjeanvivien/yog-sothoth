//! Network status reporter: records the chain's current slot and RPC latency into the
//! `network_status` singleton, read from its own `NETWORK_STATUS_*` endpoint.
//! ⚠️ Independent of ingestion on purpose: the panel's other half, `freshness`, reads what was
//! written, so when a link drops the two say which side failed. A failed tick stops nothing.

use std::convert::Infallible;
use std::sync::Arc;
use std::time::{Duration, Instant};

use solana_rpc_client::nonblocking::rpc_client::RpcClient;
use tokio::time::MissedTickBehavior;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};
use yog_bootstrap::SecretUrl;

use yog_core::domain::{NetworkStatus, NetworkStatusRepository};

mod error;
mod metrics;

use error::NetworkStatusReporterError;
pub(crate) use metrics::NetworkStatusReporterMetrics;

/// How often the reporter records a snapshot: at most ~15 s stale, at negligible RPC and DB load.
const TICK_INTERVAL: Duration = Duration::from_secs(15);

/// Periodic reporter of the chain's own progress, read from an endpoint independent of ingestion.
pub(crate) struct NetworkStatusReporter {
    rpc_client: Arc<RpcClient>,
    /// Kept to scrub the endpoint out of `solana-client` error strings.
    rpc_url: SecretUrl,
    repository: Arc<dyn NetworkStatusRepository>,
}

impl NetworkStatusReporter {
    /// Build the reporter over its RPC client and the network-status repository.
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

    /// Drive the tick loop until shutdown; the first tick fires at once.
    /// ⚠️ `Infallible` is the guard: a `?` on a tick, which would stop the daemon, does not compile.
    pub(crate) async fn run(self, shutdown: CancellationToken) -> Result<(), Infallible> {
        info!("NetworkStatusReporter started");

        let mut ticker = tokio::time::interval(TICK_INTERVAL);
        // ⚠️ `Delay`, not the default `Burst`: a `getSlot` timing out lasts twice the interval, and
        // `Burst` would fire every missed tick back to back when the link returns.
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

    /// One tick: record a snapshot and absorb its failure, which the next tick retries.
    async fn tick(&self) {
        if let Err(error) = self.record_snapshot().await {
            NetworkStatusReporterMetrics::record_tick_failure(error.reason());
            warn!(%error, "network status tick failed — skipped, the next tick retries");
        }
    }

    /// Time the `getSlot` call, then persist the snapshot; a failure is returned typed.
    async fn record_snapshot(&self) -> Result<(), NetworkStatusReporterError> {
        // This elapsed time is the reported latency.
        let started = Instant::now();
        let slot = self
            .rpc_client
            .get_slot()
            .await
            .map_err(|e| NetworkStatusReporterError::Rpc(self.rpc_url.scrub(&e.to_string())))?;
        let elapsed_ms = started.elapsed().as_millis();

        // Clamped rather than a panic: a round-trip never nears `u32::MAX` ms.
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
#[path = "network_status_reporter/network_status_reporter_tests.rs"]
mod tests;
