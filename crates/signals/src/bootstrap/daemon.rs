//! Daemon assembly: connect the DB, wire the concrete Pg repositories into
//! the detectors and the engine, then run until shutdown.
//!
//! This is the one place that knows about `yog-persistence` — the same
//! dependency-injection shape as `yog-context`'s daemon. The engine and
//! detectors only ever see core traits.

use std::sync::Arc;

use anyhow::Context;
use tokio_util::sync::CancellationToken;
use tracing::info;
use yog_core::domain::{Protocol, SignalDetector, SignalRepository, SwapFlowRepository};
use yog_persistence::{Database, PgSignalRepository, PgSwapFlowRepository};

use crate::bootstrap::Config;
use crate::detectors::FlowImbalanceDetector;
use crate::engine::SignalEngine;
use crate::metrics::EngineMetrics;

/// Owns the assembled engine, ready to run.
pub(crate) struct Daemon {
    engine: SignalEngine,
}

impl Daemon {
    /// Connect to the database and build the engine and its detectors.
    pub(crate) async fn new(config: &Config) -> anyhow::Result<Self> {
        let database = Database::connect(config.database_url.expose())
            .await
            .context("failed to connect to database")?;
        info!("connected to database");

        let pool = database.pool().clone();

        let signal_repository: Arc<dyn SignalRepository> =
            Arc::new(PgSignalRepository::new(pool.clone()));
        let flow_repository: Arc<dyn SwapFlowRepository> =
            Arc::new(PgSwapFlowRepository::new(pool));

        let flow_imbalance: Arc<dyn SignalDetector> = Arc::new(FlowImbalanceDetector::new(
            flow_repository,
            Protocol::MeteoraDammV2,
            config.flow_window,
            config.flow_interval,
            config.flow_cooldown,
            config.flow_min_volume_usd,
            config.flow_threshold,
        ));

        EngineMetrics::register_descriptions();

        let engine = SignalEngine::new(signal_repository, vec![flow_imbalance]);
        Ok(Self { engine })
    }

    /// Run the engine until Ctrl-C, then stop every detector loop gracefully.
    pub(crate) async fn run(self) -> anyhow::Result<()> {
        let shutdown = CancellationToken::new();

        let signal = shutdown.clone();
        tokio::spawn(async move {
            if tokio::signal::ctrl_c().await.is_ok() {
                info!("ctrl-c received — shutting down");
                signal.cancel();
            }
        });

        self.engine.run(shutdown).await.map_err(anyhow::Error::new)
    }
}
