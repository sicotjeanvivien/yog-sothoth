//! The wiring: every dependency [`super::Daemon::new`] builds before it owns
//! one, and the metric families it declares.

use crate::{
    application::{
        reporter::{NetworkStatusReporter, NetworkStatusReporterMetrics},
        services::{
            DammV2Repos, EventPersistor, EventPersistorMetrics, MeteoraDammV2EventPersistor,
            PoolMaintenance, TransactionProcessor, TransactionProcessorMetrics, WatchedPoolService,
        },
        source::TransactionSource,
        workers::IndexerWorkerMetrics,
    },
    bootstrap::{Config, IngestScope, TransactionArrival},
    infra::{
        DispatcherMetrics, FetchMetrics, GrpcBufferMetrics, GrpcListener, GrpcListenerMetrics,
        GrpcTransactionSource, RpcListener, RpcTransactionSource, SignatureDispatcher,
        TransactionFetcher,
    },
};
use anyhow::Context;
use solana_rpc_client::nonblocking::rpc_client::RpcClient;
use std::sync::Arc;
use tracing::info;
use yog_bootstrap::{Endpoint, SecretUrl};
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
pub(super) fn log_ingestion_mode(config: &Config) {
    info!(
        source = config.transaction_arrival.source().as_str(),
        scope = config.scope.as_str(),
        "ingestion mode"
    );

    if matches!(
        (&config.transaction_arrival, config.scope),
        (TransactionArrival::Fetched { .. }, IngestScope::Protocols)
    ) {
        tracing::warn!(
            "INGEST_SOURCE=rpc with INGEST_SCOPE=protocols subscribes to the whole program and fetches every transaction back, one request each. On a rate-limited endpoint most will be dropped and counted as fetch failures, with the process still up. INGEST_SOURCE=grpc is the mode this scope is for."
        );
    }
}

/// Connect to the database.
///
/// The database URL is held in `Config::database_url` (a redacted secret),
/// so we never log it directly — [`anyhow::Context`] is sufficient to surface
/// the failure at startup without leaking credentials.
pub(super) async fn init_db(database_url: &SecretUrl) -> anyhow::Result<Database> {
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
pub(super) fn init_source(config: &Config) -> anyhow::Result<Arc<dyn TransactionSource>> {
    match &config.transaction_arrival {
        TransactionArrival::Fetched { from } => init_rpc_source(config, from),
        TransactionArrival::Delivered => init_grpc_source(config),
    }
}

/// The notify-then-ask model: a WebSocket fleet, a filter chain, a fetch stage.
///
/// `transaction` comes in from the [`TransactionArrival::Fetched`] arm rather
/// than off `Config` directly: it is the endpoint `getTransaction` goes to, it
/// exists on this path alone, and this is the only function that needs it.
fn init_rpc_source(
    config: &Config,
    transaction: &Endpoint,
) -> anyhow::Result<Arc<dyn TransactionSource>> {
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
    // The price of not sharing is a second connection pool, for a caller making
    // one request every fifteen seconds — against the same host whenever the
    // two variables hold the same address, which the shipped `.env.example`
    // does. What changed on 21 September 2026 is not that the second pool went
    // away: it is that the probe reads its own variable, so separating the two
    // providers is now something configuration can express.
    let rpc_client = Arc::new(RpcClient::new(transaction.url().expose().to_string()));
    info!("transaction RPC client initialized: {transaction}");
    let fetcher = Arc::new(TransactionFetcher::new(rpc_client, transaction.url()));

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
pub(super) fn init_processor(
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
/// ⚠️ **And it reads its own variable**, `NETWORK_STATUS_*`, not
/// `INGEST_TRANSACTION`. What the probe measures is an *external reference on
/// the chain*, independent of ingestion by design; [`NetworkStatusReporter`]'s
/// own module docs carry the reasoning and the two readings it rules out. Until
/// 21 September 2026 it read the ingestion's fetch endpoint, which under
/// `INGEST_SOURCE=grpc` nothing ingests through: the panel showed the health of
/// a link no data travelled on, and no configuration could say so.
///
/// The start-up line below prints the probe beside **everything ingestion
/// touches**, so that whether they are independent in fact — and not merely in
/// name — is read off the logs rather than assumed. Everything, and not just
/// the stream: on the notify-then-ask path ingestion also holds
/// `INGEST_TRANSACTION`, the very endpoint the probe used to share, and a line
/// omitting it would let an operator read an independence that endpoint denies.
///
/// ⚠️ **And the line no longer *asserts* that independence — it checks it.** It
/// said "an external reference, not the ingestion link" whatever the addresses
/// were, which is a claim about configuration written where configuration is
/// not read: with the `.env.example` this repository ships, the probe and the
/// fetch endpoint are the *same* public host, so the sentence was false on a
/// fresh clone. A message is also what survives in an aggregator, where the
/// fields beside it do not. The equality below is what the check can honestly
/// see — one address written twice — and it is the likely mistake, since the
/// example file seeds both. Two spellings of one host it cannot see, which is
/// why the warning says what it compared.
pub(super) async fn init_network_status_reporter(
    database: &Database,
    config: &Config,
) -> anyhow::Result<NetworkStatusReporter> {
    let pg_network_status_reporter_repository =
        Arc::new(PgNetworkStatusRepository::new(database.pool().clone()));
    let rpc_client = Arc::new(RpcClient::new(
        config.network_status.url().expose().to_string(),
    ));
    // `Display`, not `expose`: the templates are what the operator wrote, they
    // compare exactly as well, and the `.expose()` guard list counts every site
    // in this file — a fourth one for a log line would be a widening bought for
    // nothing.
    let probe = config.network_status.to_string();
    let fetch = config
        .transaction_arrival
        .fetched_from()
        .map(ToString::to_string);
    let ingestion = match &fetch {
        Some(fetch) => format!(
            "{} (stream) + {fetch} (getTransaction)",
            config.ingest_stream
        ),
        None => format!("{} (stream)", config.ingest_stream),
    };
    info!(
        probe = %probe,
        %ingestion,
        "network status probe initialized — the chain reference the panel's slot and latency come from"
    );
    if fetch.as_deref() == Some(probe.as_str()) || config.ingest_stream.to_string() == probe {
        tracing::warn!(
            "NETWORK_STATUS_URL is the address ingestion already uses, so the dashboard's two \
             halves share one provider: the day it drops, the chain reading and the freshness \
             verdict go red together and neither says which failed. Point it elsewhere — the \
             probe costs one request every fifteen seconds. Compared as written; two spellings \
             of the same host would not be caught here."
        );
    }
    Ok(NetworkStatusReporter::new(
        rpc_client,
        config.network_status.url(),
        pg_network_status_reporter_repository,
    ))
}

// Initialise the WatchedPoolService and its repository dependency.
pub(super) async fn init_watched_pool_service(
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

/// Declare the HELP text of every metric family the process exports.
pub(super) fn register_metric_descriptions() {
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
}
