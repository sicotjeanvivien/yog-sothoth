use crate::{
    application::{
        reporter::NetworkStatusReporter,
        services::{TransactionProcessor, WatchedPoolService},
        source::{IngestedTransaction, TransactionSource},
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

mod consequences;
mod init;
mod tasks;

use consequences::{
    log_ingestion_mode, log_probe_endpoints, warn_probe_not_independent, warn_saturating_couple,
};
use init::{
    init_db, init_network_status_reporter, init_processor, init_source, init_watched_pool_service,
    register_metric_descriptions,
};
use tasks::{
    index_concurrency, spawn_indexer_task, spawn_network_status_reporter_task, spawn_source_task,
};

/// The names the three tasks answer to — in the logs, and in the list of what
/// outlived the grace. Named once because each is written at two sites.
const SOURCE: &str = "transaction source";
const INDEXER: &str = "indexer worker";
const REPORTER: &str = "network status reporter";

/// Top-level process — owns all runtime dependencies and drives the
/// indexer lifecycle.
///
/// Responsibilities:
/// - initialise all dependencies (database, RPC client, services)
/// - register the observed protocols at startup
/// - run the transaction source and the indexer worker
/// - handle graceful shutdown on SIGTERM / Ctrl-C
///
/// It is the **composition root**, and the only place that knows which
/// acquisition model is running: it builds one [`TransactionSource`] and
/// everything downstream sees the trait.
pub(crate) struct Daemon {
    processor: Arc<TransactionProcessor>,
    source: Arc<dyn TransactionSource>,
    registration: Registration,
    network_status_reporter: NetworkStatusReporter,
    /// How many transactions may be persisted at once — computed from the pool
    /// that was actually opened, see [`tasks::index_concurrency`].
    index_concurrency: usize,
    _database: Database,
}

impl Daemon {
    /// Build and wire all runtime dependencies.
    ///
    /// Fails fast if the database is unreachable, if migrations cannot
    /// be applied, or if the dispatcher is misconfigured.
    pub(crate) async fn new(config: Config) -> anyhow::Result<Self> {
        // What the configuration entails, before anything acts on it — see
        // `consequences`. Each pair states a fact, then objects if the
        // combination deserves it.
        log_ingestion_mode(&config);
        warn_saturating_couple(&config);
        log_probe_endpoints(&config);
        warn_probe_not_independent(&config);

        let database = init_db(&config.database_url)
            .await
            .context("database initialization failed")?;
        info!("database initialized");

        // As early as the pool allows: a pool too small to reserve from cannot
        // run this process, so nothing further — the RPC client, the source,
        // the processor — is worth building. It cannot come before
        // `database initialized` above, since that is the line that creates the
        // pool it reads.
        let index_concurrency = index_concurrency(database.max_connections())?;
        info!(index_concurrency, "index concurrency derived from the pool");

        let source = init_source(&config).context("transaction source initialization failed")?;
        info!("transaction source initialized: {}", config.ingest_stream);

        // Built here rather than inside `init_processor` because two callers
        // need it: the processor extracts with it, and the start-up
        // registration below asks it which protocols are worth subscribing to.
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
            index_concurrency,
            _database: database,
        })
    }

    /// Start the daemon. Consumes `self` — cannot be called twice.
    ///
    /// Spawns three tasks, and the ingestion half of the graph is **one edge**:
    ///
    /// ```text
    /// source → (IngestedTransaction) → indexer worker
    /// ```
    ///
    /// Whatever a source needs to produce that — a fleet of WebSockets, a
    /// filter chain and a fetch stage on one path, a single stream on the
    /// other — it owns and supervises itself. This graph does not change when
    /// the source does, which is the point of the port.
    ///
    /// Returns when any task fails or the shutdown token is triggered — and
    /// **not before every other task has returned too**, or
    /// [`yog_bootstrap::SHUTDOWN_GRACE`] has passed.
    ///
    /// ⚠️ **The waiting is the point.** `main` drops the runtime the moment
    /// this returns, and a dropped runtime destroys whatever is still in
    /// flight: a worker inside its `logsUnsubscribe`, an index task inside its
    /// `INSERT`. Until 14 September 2026 the cancellation arm below rendered
    /// its verdict alone and `run` returned ~7 ms after the signal, so the
    /// stop tore through all three stages — with a `JoinError` that
    /// `listener.rs` read as a panic, on the most ordinary path there is.
    ///
    /// The grace is what keeps that from becoming a hang: a stage that will
    /// not end is named in the logs and left to the runtime, which is the only
    /// way an orderly stop can still cost work.
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

        // ⚠️ **The cancellation arm carries no verdict.** It says the stop was
        // asked for, not what happened — and it now wins races it used not to:
        // `RpcTransactionSource::run` cancels the token *before* returning, so
        // on a dead ingestion this arm fires while the source is still joining
        // its stages, and its `AllWorkersGaveUp` arrives later, through the
        // drain. Treating that `Ok(())` as final made the process exit 0 on
        // every fleet that had exhausted its retry budget — measured on 14
        // September 2026, and the reason `Stop` collects a verdict instead of
        // the `select!` deciding one alone.
        //
        // Which stage answered comes back with its outcome, because `Stop`
        // needs it: that handle has been polled to completion, and the drain
        // below has to step over it. The rule lives in `settle`, not here.
        let (ended, first) = tokio::select! {
            result = &mut source_task => (Some(SOURCE), handle_task_result(result, SOURCE)),
            result = &mut indexer_task => (Some(INDEXER), handle_task_result(result, INDEXER)),
            result = &mut reporter_task => (Some(REPORTER), handle_task_result(result, REPORTER)),
            _ = shutdown.cancelled() => {
                tracing::info!("cancellation received — stopping");
                (None, Ok(()))
            }
        };

        // Whichever arm fired, the other two have to be told. Idempotent, so
        // the cancellation arm — whose token is already cancelled — needs no
        // branch of its own.
        shutdown.cancel();

        // ⚠️ **The indexer is waited on first, and the order is the whole
        // point.** `Stop` spends one grace across the three, so the first stage
        // waited on can eat all of it: the source's own wait is unbounded by
        // design, and an `unsubscribe()` on a stalled link has nothing to cut
        // it short. Ask for the source first and the indexer gets `timeout_at`
        // on a deadline already past — one poll, no wait, and the in-flight
        // `INSERT`s this whole change exists to protect are destroyed anyway.
        // The indexer is the only stage holding work that is *lost* rather than
        // merely abandoned, so it is served first.
        //
        // ⚠️ **What that order costs, said plainly.** A stage reached after the
        // deadline has passed still gets one poll — `timeout_at` polls the task
        // before the clock, so an answer already given is collected — but no
        // wait for one that has not come. An indexer that eats the whole grace
        // can therefore leave a source that is *still stopping* with its
        // verdict unsaid, and the process exits 0 with `transaction source`
        // named in the `warn!`. That is the grace doing its job rather than
        // hiding a failure: the same stage was going to be destroyed by the
        // runtime moments later whatever the order, and what an overrun owes is
        // to be named, not to become an exit code.
        let mut stop = Stop::new(first, ended);
        stop.settle(INDEXER, &mut indexer_task).await;
        stop.settle(SOURCE, &mut source_task).await;
        stop.settle(REPORTER, &mut reporter_task).await;

        stop.finish()
    }
}

/// What the source is told to watch before it runs — **one of the two, never
/// both**, and `INGEST_SCOPE` is read once to decide which.
///
/// ⚠️ An earlier shape of this PR populated both at start-up and let each
/// listener pick one by scope. `WatchedPoolService` then ran under
/// `INGEST_SCOPE=protocols` for a set nobody read, and its own log lines — the
/// count, the "listener will refuse to start" error — described work that did
/// not happen. Raised in review of PR #140. The type is what keeps that from
/// coming back: a value holds one registration or the other.
enum Registration {
    /// The protocols whose extraction is written — **not every `Protocol`**,
    /// see `ExtractionDispatcher::implemented_protocols`.
    Protocols(Vec<Protocol>),
    /// The allowlist in `watched_pools`, restored through the service that
    /// filters it by the same list.
    Pools(Arc<WatchedPoolService>),
}

/// How many delivered transactions may queue before the source is made to wait.
///
/// ⚠️ **Ten times smaller than the channels it replaces, and on purpose.** What
/// queued between the old stages was a signature and a little text, so 10 000
/// of them cost nothing. What queues here is a whole transaction — 5–20 kB
/// each — so the same figure would be 50–200 MB in this channel alone, on top
/// of the gRPC path's own pending buffer. 1 000 is 5–20 MB and still leaves two
/// orders of magnitude more room than the worker's concurrency bound, which is
/// what actually drains it.
const INGESTED_CHANNEL_CAPACITY: usize = 1_000;
