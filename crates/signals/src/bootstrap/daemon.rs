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
    DetectorMetrics, FlowImbalanceDetector, PriceOracleDeviationDetector, TvlDrainDetector,
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
            config.flow_imbalance_settings(),
        ));

        let price_oracle_deviation: Arc<dyn SignalDetector> =
            Arc::new(PriceOracleDeviationDetector::new(
                snapshot_repository,
                config.price_oracle_deviation_settings(),
            ));

        let tvl_drain: Arc<dyn SignalDetector> = Arc::new(TvlDrainDetector::new(
            liquidity_flow_repository,
            Protocol::MeteoraDammV2,
            config.tvl_drain_settings(),
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
