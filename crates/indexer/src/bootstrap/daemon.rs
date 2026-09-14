use crate::{
    application::{
        reporter::{NetworkStatusReporter, NetworkStatusReporterMetrics},
        services::{
            DammV2Repos, EventPersistor, EventPersistorMetrics, MeteoraDammV2EventPersistor,
            PoolMaintenance, TransactionProcessor, TransactionProcessorMetrics, WatchedPoolService,
        },
        source::{IngestedTransaction, TransactionSource},
        workers::{IndexerWorker, IndexerWorkerMetrics},
    },
    bootstrap::{Config, IngestScope, IngestSource},
    error::{IndexerWorkerError, SourceError, TaskEnd},
    infra::{
        DispatcherMetrics, FetchMetrics, GrpcBufferMetrics, GrpcListener, GrpcListenerMetrics,
        GrpcTransactionSource, RpcListener, RpcTransactionSource, SignatureDispatcher,
        TransactionFetcher,
    },
};
use anyhow::Context;
use solana_rpc_client::nonblocking::rpc_client::RpcClient;
use std::{convert::Infallible, sync::Arc, time::Duration};
use tokio::{sync::mpsc, task::JoinHandle, time::Instant};
use tokio_util::sync::CancellationToken;
use tracing::info;
use yog_bootstrap::SecretUrl;
use yog_core::{application::extraction::ExtractionDispatcher, domain::Protocol};
use yog_persistence::{
    Database, PgMeteoraDammV2ClaimPositionFeeEventRepository,
    PgMeteoraDammV2ClaimProtocolFeeEventRepository, PgMeteoraDammV2ClaimRewardEventRepository,
    PgMeteoraDammV2ClosePositionEventRepository, PgMeteoraDammV2CreatePositionEventRepository,
    PgMeteoraDammV2FundRewardEventRepository, PgMeteoraDammV2InitializePoolEventRepository,
    PgMeteoraDammV2InitializeRewardEventRepository, PgMeteoraDammV2LiquidityEventRepository,
    PgMeteoraDammV2LockPositionEventRepository,
    PgMeteoraDammV2PermanentLockPositionEventRepository,
    PgMeteoraDammV2SetPoolStatusEventRepository, PgMeteoraDammV2SplitPositionEventRepository,
    PgMeteoraDammV2SwapEventRepository, PgMeteoraDammV2UpdatePoolFeesEventRepository,
    PgMeteoraDammV2UpdateRewardDurationEventRepository,
    PgMeteoraDammV2UpdateRewardFunderEventRepository,
    PgMeteoraDammV2WithdrawDeadLiquidityRewardEventRepository,
    PgMeteoraDammV2WithdrawIneligibleRewardEventRepository, PgNetworkStatusRepository,
    PgPoolCurrentStateRepository, PgPoolRepository, PgWatchedPoolRepository,
};

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
    /// that was actually opened, see [`index_concurrency`].
    index_concurrency: usize,
    _database: Database,
}

impl Daemon {
    /// Build and wire all runtime dependencies.
    ///
    /// Fails fast if the database is unreachable, if migrations cannot
    /// be applied, or if the dispatcher is misconfigured.
    pub(crate) async fn new(config: Config) -> anyhow::Result<Self> {
        log_ingestion_mode(&config);

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

        DispatcherMetrics::register_descriptions();
        FetchMetrics::register_descriptions();
        IndexerWorkerMetrics::register_descriptions();
        TransactionProcessorMetrics::register_descriptions();
        EventPersistorMetrics::register_descriptions();
        // The gRPC path's two families are registered whichever source is
        // running. Descriptions are only HELP text — registering them costs a
        // string and exports nothing until a counter is touched — and the
        // alternative, registering them where the gRPC listener is built, is a
        // line that only ever runs on the path whose reader has the least
        // context. A counter exported without its HELP text is unreadable to
        // exactly the person who goes looking for it.
        GrpcBufferMetrics::register_descriptions();
        GrpcListenerMetrics::register_descriptions();
        NetworkStatusReporterMetrics::register_descriptions();
        info!("Metrics initialized");

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
    /// **not before every other task has returned too**, or [`SHUTDOWN_GRACE`]
    /// has passed.
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

        // Which task ended here, if it is one of them. Its handle must not be
        // polled again — `tokio` panics on a `JoinHandle` polled after
        // completion — so the drain below steps over it.
        let mut ended: Option<&'static str> = None;

        // ⚠️ **The cancellation arm carries no verdict.** It says the stop was
        // asked for, not what happened — and it now wins races it used not to:
        // `RpcTransactionSource::run` cancels the token *before* returning, so
        // on a dead ingestion this arm fires while the source is still joining
        // its stages, and its `AllWorkersGaveUp` arrives later, through the
        // drain. Treating that `Ok(())` as final made the process exit 0 on
        // every fleet that had exhausted its retry budget — measured on 14
        // September 2026, and the reason `Stop` collects a verdict instead of
        // the `select!` deciding one alone.
        let mut stop = Stop::new(tokio::select! {
            result = &mut source_task => {
                ended = Some(SOURCE);
                shutdown.cancel();
                handle_task_result(result, SOURCE)
            }
            result = &mut indexer_task => {
                ended = Some(INDEXER);
                shutdown.cancel();
                handle_task_result(result, INDEXER)
            }
            result = &mut reporter_task => {
                ended = Some(REPORTER);
                shutdown.cancel();
                handle_task_result(result, REPORTER)
            }
            _ = shutdown.cancelled() => {
                tracing::info!("cancellation received — stopping");
                Ok(())
            }
        });

        // One absolute deadline for all three, so waiting on them in turn is
        // still bounded by `SHUTDOWN_GRACE` in total.
        //
        // ⚠️ **The indexer is waited on first, and the order is the whole
        // point.** One deadline spent in order means the first stage waited on
        // can eat all of it: the source's own wait is unbounded by design, and
        // an `unsubscribe()` on a stalled link has nothing to cut it short. Ask
        // for the source first and the indexer gets `timeout_at` on a deadline
        // already past — one poll, no wait, and the in-flight `INSERT`s this
        // whole change exists to protect are destroyed anyway. The indexer is
        // the only stage holding work that is *lost* rather than merely
        // abandoned, so it is served first.
        let deadline = Instant::now() + SHUTDOWN_GRACE;
        if ended != Some(INDEXER) {
            stop.settle(INDEXER, &mut indexer_task, deadline).await;
        }
        if ended != Some(SOURCE) {
            stop.settle(SOURCE, &mut source_task, deadline).await;
        }
        if ended != Some(REPORTER) {
            stop.settle(REPORTER, &mut reporter_task, deadline).await;
        }

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

// ── Shutdown ─────────────────────────────────────────────────────────────────

/// The names the three tasks answer to — in the logs, and in the list of what
/// outlived the grace. Named once because each is written at two sites.
const SOURCE: &str = "transaction source";
const INDEXER: &str = "indexer worker";
const REPORTER: &str = "network status reporter";

/// How long `Daemon::run` waits for its tasks once the token has fired.
///
/// ⚠️ **Under Docker's ten seconds, not at them.** `docker-compose.yml` sets no
/// `stop_grace_period`, so the default applies: SIGKILL ten seconds after the
/// SIGTERM. A grace of ten would expire exactly when the process is killed, and
/// the log line saying which stage overran would never be written — the one
/// case where the timeout has something to say is the one where it stays
/// silent.
const SHUTDOWN_GRACE: Duration = Duration::from_secs(5);

/// What the stop has learned so far: the verdict to return, and who never
/// answered.
///
/// It exists because the verdict is **not** whatever the `select!` produced.
/// Its cancellation arm reports that a stop was asked for, which is not an
/// outcome, and a stage that failed can still be in the middle of stopping when
/// that arm fires — so the error arrives afterwards, through the drain. First
/// error wins; a stage stopping cleanly after one never erases it.
struct Stop {
    outcome: anyhow::Result<()>,
    still_running: Vec<&'static str>,
}

impl Stop {
    fn new(first: anyhow::Result<()>) -> Self {
        Self {
            outcome: first,
            still_running: Vec::new(),
        }
    }

    /// Wait for `handle`, but no longer than `deadline`.
    ///
    /// A task that ends in time has its result logged, and adopted as the
    /// verdict if nothing has failed yet. A task that does not is recorded by
    /// name: it is still running, and it will be destroyed mid-flight when
    /// `main` drops the runtime. Keeping the name — rather than only logging it
    /// — is what makes "the overrun names its task" something a test can
    /// falsify.
    async fn settle<E>(
        &mut self,
        name: &'static str,
        handle: &mut JoinHandle<Result<(), E>>,
        deadline: Instant,
    ) where
        E: std::error::Error + Send + Sync + 'static,
    {
        match tokio::time::timeout_at(deadline, handle).await {
            Ok(result) => {
                let reported = handle_task_result(result, name);
                if self.outcome.is_ok() {
                    self.outcome = reported;
                }
            }
            Err(_elapsed) => self.still_running.push(name),
        }
    }

    /// Say who outlived the grace, then hand back the verdict.
    fn finish(self) -> anyhow::Result<()> {
        if !self.still_running.is_empty() {
            tracing::warn!(
                tasks = ?self.still_running,
                grace_secs = SHUTDOWN_GRACE.as_secs(),
                "shutdown grace expired — these tasks are destroyed mid-flight with the runtime"
            );
        }
        self.outcome
    }
}

// ── Initialisation helpers ───────────────────────────────────────────────────

/// Say which acquisition model is running, and warn when it cannot keep up.
///
/// ⚠️ **The first lines the process writes, because it is the first question a
/// reader has.** Two acquisition models exist and one is running; from here on
/// nothing else in the crate names which. The two `as_str` were written for the
/// refusals of a validator that no longer exists — their remaining reader is
/// this line, and it is a better one: a refusal is read once, a running mode
/// every time something looks wrong.
///
/// ⚠️ **And the one couple that boots and cannot keep up.** `logsSubscribe` on
/// a program id delivers everything that program does, and the RPC path then
/// fetches each transaction back — measured at ~200 in 30 s against a ~10 req/s
/// tier. Nothing stops: fetch failures are skip-and-logged per transaction, so
/// the process stays up and the metrics stay plausible while most of what it
/// sees is dropped.
///
/// A `check_supported` used to refuse that couple, for a different reason — an
/// empty target set — and that reason is genuinely fixed. What went with the
/// refusal was the only loud signal an operator got, and the warning puts it
/// back at the cost of one branch.
fn log_ingestion_mode(config: &Config) {
    info!(
        source = config.source.as_str(),
        scope = config.scope.as_str(),
        "ingestion mode"
    );

    if matches!(
        (config.source, config.scope),
        (IngestSource::Rpc, IngestScope::Protocols)
    ) {
        tracing::warn!(
            "INGEST_SOURCE=rpc with INGEST_SCOPE=protocols subscribes to the whole program and fetches every transaction back, one request each. On a rate-limited endpoint most will be dropped and counted as fetch failures, with the process still up. INGEST_SOURCE=grpc is the mode this scope is for."
        );
    }
}

/// Connect to the database.
///
/// The database URL is held in `Config::database_url` (a redacted secret),
/// so we never log it directly — `anyhow::Context` is sufficient to surface
/// the failure at startup without leaking credentials.
async fn init_db(database_url: &SecretUrl) -> anyhow::Result<Database> {
    let db = Database::connect(database_url.expose())
        .await
        .context("failed to connect to database")?;
    tracing::info!("connected to database");
    Ok(db)
}

/// Build the transaction source the configuration selects.
///
/// **The only place in the crate that names a concrete source.** Everything
/// downstream holds `Arc<dyn TransactionSource>` and never learns which model
/// is running — which is why `INGEST_SOURCE` is read here and nowhere else.
///
/// The two builders below are not the same size, and that asymmetry is the
/// subject of the whole port: `logsSubscribe` notifies, so its source has to
/// assemble a fleet, a filter chain and a fetch stage; Yellowstone delivers, so
/// its source is the listener.
fn init_source(config: &Config) -> anyhow::Result<Arc<dyn TransactionSource>> {
    match config.source {
        IngestSource::Rpc => init_rpc_source(config),
        IngestSource::Grpc => init_grpc_source(config),
    }
}

/// The notify-then-ask model: a WebSocket fleet, a filter chain, a fetch stage.
fn init_rpc_source(config: &Config) -> anyhow::Result<Arc<dyn TransactionSource>> {
    let listener = Arc::new(RpcListener::new(
        config.ingest_stream.clone(),
        config.worker_max_retries,
    ));
    let dispatcher =
        Arc::new(SignatureDispatcher::new_default().context("dispatcher initialization failed")?);
    // ⚠️ **The RPC client is built here, inside the arm that needs one.** It
    // used to be built by `Daemon::new` and handed in, because the reporter
    // wanted one too — which made the composition root assemble an ingredient
    // belonging to exactly one of the two sources, and gave this function a
    // parameter the gRPC arm could only ignore. Raised in review of PR #139.
    // The price of not sharing is a second connection pool against the same
    // host, for a caller that makes one request every fifteen seconds.
    let rpc_client = Arc::new(RpcClient::new(
        config.ingest_transaction.url().expose().to_string(),
    ));
    info!(
        "transaction RPC client initialized: {}",
        config.ingest_transaction
    );
    let fetcher = Arc::new(TransactionFetcher::new(
        rpc_client,
        config.ingest_transaction.url(),
    ));

    Ok(Arc::new(RpcTransactionSource::new(
        listener, dispatcher, fetcher,
    )))
}

/// The delivering model: no fetch, no filter chain, no fleet — the listener is
/// the source.
///
/// Nothing here can fail today — the endpoint is checked in `run` — and the
/// `Result` is the shape of its sibling, so `init_source` reads as two arms of
/// one kind.
fn init_grpc_source(config: &Config) -> anyhow::Result<Arc<dyn TransactionSource>> {
    Ok(Arc::new(GrpcTransactionSource::new(Arc::new(
        GrpcListener::new(config.ingest_stream.clone(), config.worker_max_retries),
    ))))
}

/// Build the EventPersistor: shared pool maintenance plus the per-protocol
/// sub-persistor and its bundle of per-event-kind repositories.
fn init_event_persistor(database: &Database) -> Arc<EventPersistor> {
    // Cross-protocol repositories
    let pg_pool_repo = Arc::new(PgPoolRepository::new(database.pool().clone()));
    let pg_pool_current_state_repo =
        Arc::new(PgPoolCurrentStateRepository::new(database.pool().clone()));

    // Shared pool maintenance helper — reused by every per-protocol sub-persistor.
    let pool_maintenance = Arc::new(PoolMaintenance::new(
        pg_pool_repo,
        pg_pool_current_state_repo,
    ));

    // Meteora DAMM v2 sub-persistor and its per-event-kind repositories.
    let pool = || database.pool().clone();
    let damm_v2_repos = DammV2Repos {
        swap_event: Arc::new(PgMeteoraDammV2SwapEventRepository::new(pool())),
        liquidity_event: Arc::new(PgMeteoraDammV2LiquidityEventRepository::new(pool())),
        claim_position_fee: Arc::new(PgMeteoraDammV2ClaimPositionFeeEventRepository::new(pool())),
        claim_protocol_fee: Arc::new(PgMeteoraDammV2ClaimProtocolFeeEventRepository::new(pool())),
        claim_reward: Arc::new(PgMeteoraDammV2ClaimRewardEventRepository::new(pool())),
        initialize_reward: Arc::new(PgMeteoraDammV2InitializeRewardEventRepository::new(pool())),
        fund_reward: Arc::new(PgMeteoraDammV2FundRewardEventRepository::new(pool())),
        withdraw_ineligible_reward: Arc::new(
            PgMeteoraDammV2WithdrawIneligibleRewardEventRepository::new(pool()),
        ),
        update_reward_duration: Arc::new(PgMeteoraDammV2UpdateRewardDurationEventRepository::new(
            pool(),
        )),
        update_reward_funder: Arc::new(PgMeteoraDammV2UpdateRewardFunderEventRepository::new(
            pool(),
        )),
        withdraw_dead_liquidity_reward: Arc::new(
            PgMeteoraDammV2WithdrawDeadLiquidityRewardEventRepository::new(pool()),
        ),
        create_position: Arc::new(PgMeteoraDammV2CreatePositionEventRepository::new(pool())),
        close_position: Arc::new(PgMeteoraDammV2ClosePositionEventRepository::new(pool())),
        lock_position: Arc::new(PgMeteoraDammV2LockPositionEventRepository::new(pool())),
        permanent_lock_position: Arc::new(
            PgMeteoraDammV2PermanentLockPositionEventRepository::new(pool()),
        ),
        initialize_pool: Arc::new(PgMeteoraDammV2InitializePoolEventRepository::new(pool())),
        set_pool_status: Arc::new(PgMeteoraDammV2SetPoolStatusEventRepository::new(pool())),
        split_position: Arc::new(PgMeteoraDammV2SplitPositionEventRepository::new(pool())),
        update_pool_fees: Arc::new(PgMeteoraDammV2UpdatePoolFeesEventRepository::new(pool())),
    };

    let meteora_damm_v2 = Arc::new(MeteoraDammV2EventPersistor::new(
        damm_v2_repos,
        Arc::clone(&pool_maintenance),
    ));

    Arc::new(EventPersistor::new(meteora_damm_v2))
}

/// Initialise the indexer service and its repository dependencies.
fn init_processor(
    database: &Database,
    extractor: Arc<ExtractionDispatcher>,
) -> Arc<TransactionProcessor> {
    let event_persistor = init_event_persistor(database);
    info!("event persistor initialized");

    Arc::new(TransactionProcessor::new(extractor, event_persistor))
}

/// Initialise the NetworkStatusReporter and its repository dependency.
///
/// ⚠️ **It opens its own RPC client**, rather than borrowing the ingestion's.
/// Sharing one made the reporter's health probe and the fetcher's transport the
/// same object, which is a coincidence of endpoint and not a shared concern —
/// and it is why the client used to be built one storey up, where neither of
/// them lives.
///
/// ⚠️ **And what it should measure is an open question**, tracked by
/// `02 - backlog/pre-v02/le-reporter-mesure-un-lien-que-l-ingestion-n-utilise-pas.md`:
/// this probe reads `INGEST_TRANSACTION`, which the gRPC source will not use.
async fn init_network_status_reporter(
    database: &Database,
    config: &Config,
) -> anyhow::Result<NetworkStatusReporter> {
    let pg_network_status_reporter_repository =
        Arc::new(PgNetworkStatusRepository::new(database.pool().clone()));
    let rpc_client = Arc::new(RpcClient::new(
        config.ingest_transaction.url().expose().to_string(),
    ));
    Ok(NetworkStatusReporter::new(
        rpc_client,
        config.ingest_transaction.url(),
        pg_network_status_reporter_repository,
    ))
}

// Initialise the WatchedPoolService and its repository dependency.
async fn init_watched_pool_service(
    database: &Database,
    source: Arc<dyn TransactionSource>,
    implemented_protocols: Vec<Protocol>,
) -> anyhow::Result<Arc<WatchedPoolService>> {
    let pg_watched_pool_repository =
        Arc::new(PgWatchedPoolRepository::new(database.pool().clone()));
    Ok(Arc::new(WatchedPoolService::new(
        source,
        pg_watched_pool_repository,
        implemented_protocols,
    )))
}
// ── Task spawners ────────────────────────────────────────────────────────────

/// Spawn the ingestion task — whatever shape the source's own graph has.
fn spawn_source_task(
    source: Arc<dyn TransactionSource>,
    tx: mpsc::Sender<IngestedTransaction>,
    shutdown: CancellationToken,
) -> JoinHandle<Result<(), SourceError>> {
    tokio::spawn(async move { source.run(tx, shutdown).await })
}

/// How many connections the pool's *other* in-process users may need while the
/// indexer worker is running.
///
/// One today: [`NetworkStatusReporter`] upserts a snapshot on every tick.
/// `WatchedPoolService` is not counted — it restores subscriptions once, before
/// the worker is spawned, and never touches the pool again.
const CONNECTIONS_RESERVED: u32 = 1;

/// How many transactions may be persisted concurrently.
///
/// # ⚠️ The mechanism, stated correctly
///
/// A task does **not** hold a connection for its lifetime. Every repository
/// call executes against `&PgPool`, so sqlx takes a connection per *statement*
/// and returns it — nothing in `crates/persistence` opens a transaction or
/// calls `acquire`. What makes a task-count bound a connection bound is
/// something else: **an index task issues one statement at a time**. Its
/// events are persisted in a sequential loop, so *n* tasks put at most *n*
/// statements in flight, and capping tasks at `pool − 1` leaves the pool one
/// slot for the reporter.
///
/// ⚠️ **And that invariant is not enforced anywhere.** The first sub-persistor
/// that runs two repository calls under `tokio::join!` doubles a task's
/// concurrent statements and silently reinstates the starvation this bound
/// exists to prevent. What that starvation costs has changed: until 11
/// September 2026 the reporter propagated its tick errors, so one
/// `acquire_timeout` stopped the process by way of its health probe. A failed
/// tick is now counted and skipped, so the reporter only goes quiet — its
/// `network_status.observed_at` freezes and
/// `yog_indexer_network_status_tick_failures_total{reason="persistence"}`
/// climbs — while the index tasks queue on the same pool, and *their*
/// `acquire_timeout`s are what lose rows. Whoever parallelises a persist owes
/// this line a second look.
///
/// Read from the pool that was opened, not from
/// [`Database::DEFAULT_MAX_CONNECTIONS`], so that sizing the pool differently
/// resizes this too instead of silently parting ways with it. It takes the
/// number rather than the `Database` so the boundary is a plain unit test.
///
/// # Errors
///
/// ⚠️ **Refuses a pool too small to reserve from, rather than clamping to one.**
/// An earlier version returned `.max(1)`, which defeated the reservation in the
/// exact case this function exists to prevent: a pool of one hands its only
/// connection to an index task. Clamping cannot be right here —
/// `Semaphore::new(0)` would deadlock instead — so the only honest answers are
/// "refuse" or "open a bigger pool", and a configuration that cannot work
/// should say so at startup rather than five seconds into a busy minute.
fn index_concurrency(max_connections: u32) -> anyhow::Result<usize> {
    anyhow::ensure!(
        max_connections > CONNECTIONS_RESERVED,
        "database pool holds {max_connections} connection(s), and {CONNECTIONS_RESERVED} must \
         stay free for the network status reporter — the indexer would have none left to \
         persist with. Open the pool with more connections."
    );
    Ok((max_connections - CONNECTIONS_RESERVED) as usize)
}

/// Spawn the indexer worker task.
///
/// Per-transaction failures stay inside the worker (logged, counted, not
/// propagated). Only loop-level failures reach the returned `JoinHandle`
/// and bubble up to `Daemon::run`.
fn spawn_indexer_task(
    processor: Arc<TransactionProcessor>,
    rx: mpsc::Receiver<IngestedTransaction>,
    max_concurrent: usize,
    shutdown: CancellationToken,
) -> JoinHandle<Result<(), IndexerWorkerError>> {
    let worker = IndexerWorker::new(processor, max_concurrent);
    tokio::spawn(async move { worker.run(rx, shutdown).await })
}

/// Spawn the network status reporter task.
///
/// It cannot return an error — a failed tick is counted and skipped inside —
/// so the only way this handle ends the daemon from `run`'s `select!` is a
/// panic, which is a bug and should stop things.
fn spawn_network_status_reporter_task(
    reporter: NetworkStatusReporter,
    shutdown: CancellationToken,
) -> JoinHandle<Result<(), Infallible>> {
    tokio::spawn(async move { reporter.run(shutdown).await })
}

// ── Task result handling ─────────────────────────────────────────────────────

/// Normalise the result of a spawned task into a loggable anyhow::Result.
///
/// Distinguishes four cases: clean stop, task error, task panic, and a task
/// destroyed before it could answer — see [`TaskEnd`] for why the last two are
/// one type in `tokio` and must not be one here.
fn handle_task_result<E>(
    result: Result<Result<(), E>, tokio::task::JoinError>,
    task_name: &str,
) -> anyhow::Result<()>
where
    E: std::error::Error + Send + Sync + 'static,
{
    match result {
        Ok(Ok(())) => {
            tracing::info!("{task_name} stopped");
            Ok(())
        }
        Ok(Err(e)) => {
            tracing::error!(error = %e, "{task_name} failed");
            Err(anyhow::Error::new(e))
        }
        Err(e) => match TaskEnd::from(&e) {
            TaskEnd::Panicked => {
                tracing::error!(error = %e, "{task_name} panicked");
                Err(anyhow::anyhow!("{task_name} panicked: {e}"))
            }
            TaskEnd::Cancelled => {
                tracing::debug!(error = %e, "{task_name} was cancelled before it could stop");
                Ok(())
            }
        },
    }
}

#[cfg(test)]
#[path = "daemon_tests.rs"]
mod tests;
