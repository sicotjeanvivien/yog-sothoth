use crate::{
    application::{
        services::{TransactionProcessor, WatchedPoolService},
        source::{IngestedTransaction, TransactionSource},
        workers::{IngestionAlarm, NetworkStatusReporter},
    },
    bootstrap::{Config, IngestScope},
};
use anyhow::Context;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::info;
use yog_bootstrap::{Stop, handle_task_result};
use yog_core::{application::extraction::ExtractionDispatcher, domain::Protocol};
use yog_persistence::Database;

mod config_log;
mod init;
mod tasks;

use config_log::{
    log_ingestion_mode, log_probe_endpoints, warn_probe_not_independent, warn_saturating_couple,
};
use init::{
    init_alarm_db, init_db, init_ingestion_alarm, init_network_status_reporter, init_processor,
    init_source, init_watched_pool_service, register_metric_descriptions,
};
use tasks::{
    index_concurrency, spawn_indexer_task, spawn_ingestion_alarm_task,
    spawn_network_status_reporter_task, spawn_source_task,
};

/// The names the four tasks answer to, in the logs and in the list of what outlived the grace.
const SOURCE: &str = "transaction source";
const INDEXER: &str = "indexer worker";
const REPORTER: &str = "network status reporter";
const ALARM: &str = "ingestion alarm";

/// Top-level process and composition root: owns every runtime dependency, and is the only place
/// that knows which acquisition model runs, building one [`TransactionSource`].
pub(crate) struct Daemon {
    processor: Arc<TransactionProcessor>,
    source: Arc<dyn TransactionSource>,
    registration: Registration,
    network_status_reporter: NetworkStatusReporter,
    ingestion_alarm: IngestionAlarm,
    /// How many transactions may be persisted at once, from the pool actually opened; see
    /// [`tasks::index_concurrency`].
    index_concurrency: usize,
    _database: Database,
}

impl Daemon {
    /// Build and wire all runtime dependencies; fails fast on an unreachable database or a
    /// misconfiguration.
    pub(crate) async fn new(config: Config) -> anyhow::Result<Self> {
        // What the configuration entails, logged before anything acts on it.
        log_ingestion_mode(&config);
        warn_saturating_couple(&config);
        log_probe_endpoints(&config);
        warn_probe_not_independent(&config);

        let database = init_db(&config.database_url)
            .await
            .context("database initialization failed")?;
        info!("database initialized");

        // As early as the pool allows: a pool too small to reserve from cannot run this process.
        let index_concurrency = index_concurrency(database.max_connections())?;
        info!(index_concurrency, "index concurrency derived from the pool");

        // The second pool, opened beside the first so their share of connections reads in one place.
        let alarm_database = init_alarm_db(&config.database_url)
            .await
            .context("ingestion alarm database initialization failed")?;
        info!("ingestion alarm database initialized");

        let source = init_source(&config).context("transaction source initialization failed")?;
        info!("transaction source initialized: {}", config.ingest_stream);

        // Built here: the processor extracts with it, and the registration asks it what to watch.
        let extractor = Arc::new(ExtractionDispatcher::new());
        let implemented_protocols = extractor.implemented_protocols();
        info!(
            protocols = ?implemented_protocols,
            "protocols with a working extractor"
        );

        let processor = init_processor(&database, Arc::clone(&extractor));
        info!("indexer service initialized");

        let network_status_reporter = init_network_status_reporter(&database, &config)
            .await
            .context("network_status_reporter initialization failed")?;

        let ingestion_alarm = init_ingestion_alarm(&alarm_database, config.heartbeat_url.clone())
            .context("ingestion alarm initialization failed")?;

        let registration = match config.scope {
            IngestScope::Protocols => Registration::Protocols(implemented_protocols),
            IngestScope::Pools => {
                let service = init_watched_pool_service(
                    &database,
                    Arc::clone(&source),
                    implemented_protocols,
                )
                .await
                .context("watched pool service initialization failed")?;
                info!("watched pool service initialized");
                Registration::Pools(service)
            }
        };

        register_metric_descriptions();

        info!("daemon initialized");

        Ok(Self {
            processor,
            source,
            registration,
            network_status_reporter,
            ingestion_alarm,
            index_concurrency,
            _database: database,
        })
    }

    /// Start the daemon: four tasks, the ingestion half one edge, source → indexer worker. Returns
    /// when a task ends or shutdown fires, and not before the others have too, or the grace passed.
    /// ⚠️ The waiting is the point: `main` drops the runtime on return, destroying an `INSERT` in flight.
    pub(crate) async fn run(self, shutdown: CancellationToken) -> anyhow::Result<()> {
        match &self.registration {
            Registration::Protocols(protocols) => {
                for protocol in protocols {
                    self.source.watch_protocol(*protocol).await;
                }
            }
            Registration::Pools(service) => service.restore_subscriptions().await?,
        }
        let (tx, rx) = mpsc::channel::<IngestedTransaction>(INGESTED_CHANNEL_CAPACITY);

        let mut source_task = spawn_source_task(Arc::clone(&self.source), tx, shutdown.clone());
        let mut indexer_task = spawn_indexer_task(
            Arc::clone(&self.processor),
            rx,
            self.index_concurrency,
            shutdown.clone(),
        );
        let mut reporter_task =
            spawn_network_status_reporter_task(self.network_status_reporter, shutdown.clone());
        let mut alarm_task = spawn_ingestion_alarm_task(self.ingestion_alarm, shutdown.clone());

        // ⚠️ The cancellation arm carries no verdict: the RPC source cancels the token before returning,
        // so its `AllWorkersGaveUp` may come later, through `Stop`, which collects the verdict.
        let (ended, first) = tokio::select! {
            result = &mut source_task => (Some(SOURCE), handle_task_result(result, SOURCE)),
            result = &mut indexer_task => (Some(INDEXER), handle_task_result(result, INDEXER)),
            result = &mut reporter_task => (Some(REPORTER), handle_task_result(result, REPORTER)),
            result = &mut alarm_task => (Some(ALARM), handle_task_result(result, ALARM)),
            _ = shutdown.cancelled() => {
                tracing::info!("cancellation received — stopping");
                (None, Ok(()))
            }
        };

        // Idempotent: the cancellation arm needs no branch of its own.
        shutdown.cancel();

        // ⚠️ The indexer first: `Stop` spends one grace across all, the source's wait is unbounded,
        // and only the indexer holds work that is lost rather than abandoned, in-flight `INSERT`s.
        let mut stop = Stop::new(first, ended);
        stop.settle(INDEXER, &mut indexer_task).await;
        // Past the deadline a stage gets one poll, no wait, and is named in the `warn!`.
        stop.settle(SOURCE, &mut source_task).await;
        stop.settle(REPORTER, &mut reporter_task).await;
        stop.settle(ALARM, &mut alarm_task).await;

        stop.finish()
    }
}

/// What the source is told to watch before it runs: one of the two, never both, by `INGEST_SCOPE`.
enum Registration {
    /// The protocols whose extraction is written — **not every `Protocol`**,
    /// see `ExtractionDispatcher::implemented_protocols`.
    Protocols(Vec<Protocol>),
    /// The allowlist in `watched_pools`, restored through the service that
    /// filters it by the same list.
    Pools(Arc<WatchedPoolService>),
}

/// How many delivered transactions may queue before the source is made to wait.
/// ⚠️ Whole transactions of 5–20 kB each: 1 000 is 5–20 MB, still far more than the worker's
/// concurrency drains at once.
const INGESTED_CHANNEL_CAPACITY: usize = 1_000;
