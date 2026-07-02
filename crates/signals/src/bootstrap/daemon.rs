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
use yog_core::domain::{
    PoolPriceSnapshotRepository, Protocol, SignalDetector, SignalRepository, SwapFlowRepository,
};
use yog_persistence::{
    Database, PgPoolPriceSnapshotRepository, PgSignalRepository, PgSwapFlowRepository,
};

use crate::bootstrap::Config;
use crate::detectors::{
    FlowImbalanceDetector, FlowImbalanceSettings, PriceOracleDeviationDetector,
    PriceOracleDeviationSettings,
};
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
            Arc::new(PgSwapFlowRepository::new(pool.clone()));
        let snapshot_repository: Arc<dyn PoolPriceSnapshotRepository> =
            Arc::new(PgPoolPriceSnapshotRepository::new(pool));

        let flow_imbalance: Arc<dyn SignalDetector> = Arc::new(FlowImbalanceDetector::new(
            flow_repository,
            Protocol::MeteoraDammV2,
            FlowImbalanceSettings {
                window: config.flow_window,
                interval: config.flow_interval,
                cooldown: config.flow_cooldown,
                min_volume_usd: config.flow_min_volume_usd,
                threshold: config.flow_threshold,
                critical: config.flow_critical,
            },
        ));

        let price_oracle_deviation: Arc<dyn SignalDetector> =
            Arc::new(PriceOracleDeviationDetector::new(
                snapshot_repository,
                PriceOracleDeviationSettings {
                    interval: config.price_deviation_interval,
                    cooldown: config.price_deviation_cooldown,
                    max_price_age: config.price_deviation_max_price_age,
                    max_spot_age: config.price_deviation_max_spot_age,
                    threshold: config.price_deviation_threshold,
                    critical: config.price_deviation_critical,
                },
            ));

        EngineMetrics::register_descriptions();

        let engine = SignalEngine::new(
            signal_repository,
            vec![flow_imbalance, price_oracle_deviation],
        );
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
