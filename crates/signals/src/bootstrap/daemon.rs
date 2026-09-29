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
use yog_bootstrap::{HealthchecksHeartbeat, Heartbeat, HeartbeatSettings};
use yog_core::domain::{
    LiquidityFlowRepository, MaterializationRepository, PoolPriceSnapshotRepository, Protocol,
    SignalDetector, SignalRepository, SwapFlowRepository,
};
use yog_persistence::{
    Database, PgLiquidityFlowRepository, PgMaterializationRepository,
    PgPoolPriceSnapshotRepository, PgSignalRepository, PgSwapFlowRepository,
};

use crate::bootstrap::Config;
use crate::detectors::{
    FlowImbalanceDetector, FlowImbalanceSettings, PriceOracleDeviationDetector,
    PriceOracleDeviationSettings, TvlDrainDetector, TvlDrainSettings,
};
use crate::engine::SignalEngine;
use crate::materialization_watch::{MaterializationWatch, MaterializationWatchSettings};
use crate::metrics::{self, EngineMetrics, MaterializationMetrics};

/// Owns the assembled engine and the materialisation watch, ready to run.
pub(crate) struct Daemon {
    engine: SignalEngine,
    watch: MaterializationWatch,
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
            Arc::new(PgPoolPriceSnapshotRepository::new(pool.clone()));
        let liquidity_flow_repository: Arc<dyn LiquidityFlowRepository> =
            Arc::new(PgLiquidityFlowRepository::new(pool.clone()));
        let materialization_repository: Arc<dyn MaterializationRepository> =
            Arc::new(PgMaterializationRepository::new(pool));

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

        let tvl_drain: Arc<dyn SignalDetector> = Arc::new(TvlDrainDetector::new(
            liquidity_flow_repository,
            Protocol::MeteoraDammV2,
            TvlDrainSettings {
                window: config.tvl_drain_window,
                interval: config.tvl_drain_interval,
                cooldown: config.tvl_drain_cooldown,
                min_tvl_usd: config.tvl_drain_min_tvl_usd,
                threshold: config.tvl_drain_threshold,
                critical: config.tvl_drain_critical,
            },
        ));

        EngineMetrics::register_descriptions();
        MaterializationMetrics::register_descriptions();

        let engine = SignalEngine::new(
            signal_repository,
            vec![flow_imbalance, price_oracle_deviation, tvl_drain],
        );

        let heartbeat = config
            .materialization_heartbeat_url
            .clone()
            .map(|url| {
                let settings = HeartbeatSettings {
                    variable: "SIGNALS_MATERIALIZATION_HEARTBEAT_URL",
                    undelivered_counter: metrics::HEARTBEAT_FAILURES,
                };
                HealthchecksHeartbeat::new(url, settings)
                    .map(|heartbeat| Arc::new(heartbeat) as Arc<dyn Heartbeat>)
            })
            .transpose()
            .context("failed to build the heartbeat client")?;
        let watch = MaterializationWatch::new(
            materialization_repository,
            heartbeat,
            MaterializationWatchSettings {
                interval: config.materialization_interval,
                max_wait: config.materialization_max_wait,
            },
        );

        Ok(Self { engine, watch })
    }

    /// Run the engine until the process is asked to stop, then let every
    /// detector loop finish its tick.
    ///
    /// ⚠️ **SIGTERM, not only Ctrl-C.** This waited on `tokio::signal::ctrl_c()`
    /// alone — SIGINT — while `docker compose stop` sends SIGTERM. As PID 1 in
    /// its container the process did not even die on it: the kernel withholds a
    /// signal's default action from PID 1, so the SIGTERM was ignored and
    /// Docker's SIGKILL ended things ten seconds later, mid-tick.
    /// `shutdown_signal` covers both, and it is the one the other daemons use.
    ///
    /// ⚠️ **The other half is still missing here**, and it is written down
    /// rather than half-done: `engine.run` joins its detectors with no deadline,
    /// so a detector stuck in a slow query still holds the stop open without
    /// naming itself.
    pub(crate) async fn run(self) -> anyhow::Result<()> {
        let shutdown = CancellationToken::new();

        let signal = shutdown.clone();
        tokio::spawn(async move {
            yog_bootstrap::shutdown_signal().await;
            signal.cancel();
        });

        // The watch shares the stop, and ends as soon as it fires — its check
        // races the token — so it never outlasts the engine it runs beside.
        let (engine, ()) = tokio::join!(
            self.engine.run(shutdown.clone()),
            self.watch.run(shutdown.clone()),
        );
        engine.map_err(anyhow::Error::new)
    }
}
