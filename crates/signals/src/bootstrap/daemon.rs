//! Daemon assembly: connect the database, wire the Pg repositories into the detectors and the
//! engine, then run until shutdown; the one place that knows `yog-persistence`.

use std::sync::Arc;

use anyhow::Context;
use tokio_util::sync::CancellationToken;
use tracing::info;
use yog_core::domain::{
    LiquidityFlowRepository, PoolPriceSnapshotRepository, Protocol, SignalDetector,
    SignalRepository, SwapFlowRepository,
};
use yog_persistence::{
    Database, PgLiquidityFlowRepository, PgPoolPriceSnapshotRepository, PgSignalRepository,
    PgSwapFlowRepository,
};

mod init;

use crate::application::detectors::{
    DetectorMetrics, FlowImbalanceDetector, FlowImbalanceSettings, PriceOracleDeviationDetector,
    PriceOracleDeviationSettings, TvlDrainDetector, TvlDrainSettings,
};
use crate::application::workers::{
    AlarmMetrics, EngineMetrics, MaterializationAlarm, SignalEngine,
};
use crate::bootstrap::Config;
use init::init_materialization_alarm;

/// Owns the assembled engine and the materialisation alarm, ready to run.
pub(crate) struct Daemon {
    engine: SignalEngine,
    alarm: MaterializationAlarm,
}

impl Daemon {
    /// Connect to the database and build the engine and its detectors.
    pub(crate) async fn connect(config: &Config) -> anyhow::Result<Self> {
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
            Arc::new(PgLiquidityFlowRepository::new(pool));

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
                    min_tvl_usd: config.price_deviation_min_tvl_usd,
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
        DetectorMetrics::register_descriptions();
        AlarmMetrics::register_descriptions();

        let engine = SignalEngine::new(
            signal_repository,
            vec![flow_imbalance, price_oracle_deviation, tvl_drain],
        );
        let alarm =
            init_materialization_alarm(&config.database_url, &config.materialization_alarm).await?;

        Ok(Self { engine, alarm })
    }

    /// Run the engine and the materialisation alarm until the stop, then let every detector loop
    /// finish its tick within the grace; the alarm ends at once, its check racing the stop.
    pub(crate) async fn run(self) -> anyhow::Result<()> {
        let shutdown = CancellationToken::new();

        let signal = shutdown.clone();
        tokio::spawn(async move {
            yog_bootstrap::shutdown_signal().await;
            signal.cancel();
        });

        // The alarm shares the stop and ends as soon as it fires, never outlasting the engine.
        let (engine, ()) = tokio::join!(
            self.engine.run(shutdown.clone()),
            self.alarm.run(shutdown.clone()),
        );
        engine.map(drop).map_err(anyhow::Error::new)
    }
}
