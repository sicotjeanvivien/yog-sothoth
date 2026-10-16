//! The wiring: every dependency [`super::Daemon::new`] builds before it owns
//! one, and the metric families it declares.

use crate::{
    application::{
        services::{
            DammV2Repos, EventPersistor, EventPersistorMetrics, MeteoraDammV2EventPersistor,
            PoolMaintenance, TransactionProcessor, TransactionProcessorMetrics, WatchedPoolService,
        },
        source::TransactionSource,
        workers::{
            HEARTBEAT_FAILURES, IndexerWorkerMetrics, IngestionAlarm, IngestionAlarmMetrics,
            NetworkStatusReporter, NetworkStatusReporterMetrics, STATEMENT_TIMEOUT,
        },
    },
    bootstrap::{Config, TransactionArrival},
    infra::{
        DispatcherMetrics, FetchMetrics, GrpcBufferMetrics, GrpcListener, GrpcListenerMetrics,
        GrpcTransactionSource, RpcListener, RpcTransactionSource, STALL_TIMEOUT,
        SignatureDispatcher, TransactionFetcher,
    },
};
use anyhow::Context;
use solana_rpc_client::nonblocking::rpc_client::RpcClient;
use std::sync::Arc;
use tracing::info;
use yog_bootstrap::{Endpoint, HealthchecksHeartbeat, Heartbeat, HeartbeatSettings, SecretUrl};
use yog_core::{
    application::extraction::ExtractionDispatcher,
    domain::{EventFreshnessRepository, Protocol},
};
use yog_persistence::{
    Database, PgEventFreshnessRepository, PgMeteoraDammV2ClaimPositionFeeEventRepository,
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
    PgPoolCurrentStateRepository, PgPoolRepository, PgWatchedPoolRepository, PoolSettings,
};

/// Connect to the database; the URL is a redacted secret, never logged.
pub(super) async fn init_db(database_url: &SecretUrl) -> anyhow::Result<Database> {
    let db = Database::connect(database_url.expose())
        .await
        .context("failed to connect to database")?;
    tracing::info!("connected to database");
    Ok(db)
}

/// Build the transaction source the configuration selects: the only place in the crate that
/// names a concrete source.
pub(super) fn init_source(config: &Config) -> anyhow::Result<Arc<dyn TransactionSource>> {
    match &config.transaction_arrival {
        TransactionArrival::Fetched { from } => init_rpc_source(config, from),
        TransactionArrival::Delivered => init_grpc_source(config),
    }
}

/// The notify-then-ask model: a WebSocket fleet, a filter chain, a fetch stage; `transaction`,
/// the `getTransaction` endpoint, comes from the [`TransactionArrival::Fetched`] arm.
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
    // Built inside the arm that needs it: the probe opens its own client, on its own variable.
    let rpc_client = Arc::new(RpcClient::new(transaction.url().expose().to_string()));
    info!("transaction RPC client initialized: {transaction}");
    let fetcher = Arc::new(TransactionFetcher::new(rpc_client, transaction.url()));

    Ok(Arc::new(RpcTransactionSource::new(
        listener, dispatcher, fetcher,
    )))
}

/// The delivering model: the listener is the source. Nothing here can fail yet; the `Result`
/// matches its sibling's.
fn init_grpc_source(config: &Config) -> anyhow::Result<Arc<dyn TransactionSource>> {
    Ok(Arc::new(GrpcTransactionSource::new(Arc::new(
        GrpcListener::new(
            config.ingest_stream.clone(),
            config.worker_max_retries,
            STALL_TIMEOUT,
        ),
    ))))
}

/// Build the EventPersistor: shared pool maintenance plus the per-protocol
/// sub-persistor and its bundle of per-event-kind repositories.
fn init_event_persistor(database: &Database) -> Arc<EventPersistor> {
    // Cross-protocol repositories.
    let pg_pool_repo = Arc::new(PgPoolRepository::new(database.pool().clone()));
    let pg_pool_current_state_repo =
        Arc::new(PgPoolCurrentStateRepository::new(database.pool().clone()));

    // Shared pool maintenance, reused by every per-protocol sub-persistor.
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

/// Initialise the NetworkStatusReporter on its own RPC client and its own `NETWORK_STATUS_*`
/// variable, independent of ingestion; [`super::config_log`] says which endpoints it reads.
pub(super) async fn init_network_status_reporter(
    database: &Database,
    config: &Config,
) -> anyhow::Result<NetworkStatusReporter> {
    let pg_network_status_reporter_repository =
        Arc::new(PgNetworkStatusRepository::new(database.pool().clone()));
    let rpc_client = Arc::new(RpcClient::new(
        config.network_status.url().expose().to_string(),
    ));
    Ok(NetworkStatusReporter::new(
        rpc_client,
        config.network_status.url(),
        pg_network_status_reporter_repository,
    ))
}

/// Open the ingestion alarm's own one-connection pool, under a `statement_timeout`.
/// ⚠️ Not the indexing pool: a timeout there would bound the index writes, and a stuck read
/// would hold a connection [`super::tasks::index_concurrency`] counts on.
pub(super) async fn init_alarm_db(database_url: &SecretUrl) -> anyhow::Result<Database> {
    let db = Database::connect_with(
        database_url.expose(),
        PoolSettings {
            max_connections: 1,
            statement_timeout: Some(STATEMENT_TIMEOUT),
            ..PoolSettings::DEFAULT
        },
    )
    .await
    .context("failed to connect the ingestion alarm to the database")?;
    Ok(db)
}

/// The alarm on the ingestion, over the pool [`init_alarm_db`] opened, reporting to its check if any.
pub(super) fn init_ingestion_alarm(
    database: &Database,
    heartbeat_url: Option<SecretUrl>,
) -> anyhow::Result<IngestionAlarm> {
    let repository: Arc<dyn EventFreshnessRepository> =
        Arc::new(PgEventFreshnessRepository::new(database.pool().clone()));
    let heartbeat = heartbeat_url.map(init_heartbeat).transpose()?;
    Ok(IngestionAlarm::new(repository, heartbeat))
}

/// The Healthchecks.io check, when one is configured. A URL that cannot take
/// `/fail` stops the daemon here, with the variable named.
fn init_heartbeat(url: SecretUrl) -> anyhow::Result<Arc<dyn Heartbeat>> {
    let settings = HeartbeatSettings {
        variable: "INDEXER_HEARTBEAT_URL",
        undelivered_counter: HEARTBEAT_FAILURES,
    };
    let heartbeat = HealthchecksHeartbeat::new(url, settings)
        .context("failed to build the heartbeat client")?;
    Ok(Arc::new(heartbeat))
}

/// Initialise the WatchedPoolService and its repository dependency.
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
    // The gRPC families are registered whichever source runs: HELP text costs a string, and a
    // counter exported without it is unreadable to whoever goes looking.
    GrpcBufferMetrics::register_descriptions();
    GrpcListenerMetrics::register_descriptions();
    NetworkStatusReporterMetrics::register_descriptions();
    IngestionAlarmMetrics::register_descriptions();
    info!("Metrics initialized");
}
