//! The daemon's tasks: how each one is spawned, and the names they answer to.

use std::sync::Arc;

use tokio::task::{JoinError, JoinHandle};
use tokio_util::sync::CancellationToken;
use yog_bootstrap::Heartbeat;
use yog_core::domain::{
    PoolAccountResolver, PoolRepository, TokenMetadataRepository, TokenPriceRepository,
};

use crate::application::heartbeat::HeartbeatReporter;
use crate::application::heartbeat::source_health::SourceHealth;
use crate::application::source::{MetadataSource, PoolAccountSource, PriceSource};
use crate::application::workers::{MetadataWorker, PoolAccountWorker, PriceWorker};
use crate::error::WorkerError;

/// The names the four tasks answer to, in the logs and in the list of what outlived the grace.
pub(super) const METADATA: &str = "metadata worker";
pub(super) const PRICE: &str = "price worker";
pub(super) const POOL_ACCOUNT: &str = "pool-account worker";
pub(super) const REPORTER: &str = "heartbeat reporter";

pub(super) async fn reporter_end(
    task: &mut Option<JoinHandle<Result<(), WorkerError>>>,
) -> Result<Result<(), WorkerError>, JoinError> {
    match task {
        Some(task) => task.await,
        None => std::future::pending().await,
    }
}

pub(super) fn spawn_heartbeat_reporter(
    health: Arc<SourceHealth>,
    heartbeat: Arc<dyn Heartbeat>,
    shutdown: CancellationToken,
) -> JoinHandle<Result<(), WorkerError>> {
    let reporter = HeartbeatReporter::new(health, heartbeat);
    tokio::spawn(async move { reporter.run(shutdown).await })
}

/// Spawn the metadata worker task.
pub(super) fn spawn_metadata_worker(
    repository: Arc<dyn TokenMetadataRepository>,
    metadata_source: Arc<dyn MetadataSource>,
    poll_interval: std::time::Duration,
    health: Arc<SourceHealth>,
    shutdown: CancellationToken,
) -> JoinHandle<Result<(), WorkerError>> {
    let worker = MetadataWorker::new(repository, metadata_source, poll_interval, health);
    tokio::spawn(async move { worker.run(shutdown).await })
}

/// Spawn the price worker task.
pub(super) fn spawn_price_worker(
    metadata_repository: Arc<dyn TokenMetadataRepository>,
    price_repository: Arc<dyn TokenPriceRepository>,
    price_source: Arc<dyn PriceSource>,
    interval: std::time::Duration,
    shutdown: CancellationToken,
) -> JoinHandle<Result<(), WorkerError>> {
    let worker = PriceWorker::new(
        metadata_repository,
        price_repository,
        price_source,
        interval,
    );
    tokio::spawn(async move { worker.run(shutdown).await })
}

/// Spawn the pool-account resolver worker task.
pub(super) fn spawn_pool_account_worker(
    resolvers: Vec<Arc<dyn PoolAccountResolver>>,
    pool_repository: Arc<dyn PoolRepository>,
    source: Arc<dyn PoolAccountSource>,
    poll_interval: std::time::Duration,
    health: Arc<SourceHealth>,
    shutdown: CancellationToken,
) -> JoinHandle<Result<(), WorkerError>> {
    let worker = PoolAccountWorker::new(resolvers, pool_repository, source, poll_interval, health);
    tokio::spawn(async move { worker.run(shutdown).await })
}
