use std::sync::Arc;
use tracing::info;
use yog_core::domain::WatchedPoolRepository;

use crate::{error::DatabaseError, infra::RpcListener};

/// Manages the lifecycle of pool subscriptions.
///
/// Single responsibility : keep the database and the WebSocket listener
/// in sync — a pool persisted in the database must always have an active
/// WebSocket subscription, and vice versa.
pub struct WatchedPoolService {
    listener: Arc<RpcListener>,
    repository: Arc<dyn WatchedPoolRepository>,
}

impl WatchedPoolService {
    pub(crate) fn new(
        listener: Arc<RpcListener>,
        repository: Arc<dyn WatchedPoolRepository>,
    ) -> Self {
        Self {
            listener,
            repository,
        }
    }

    /// On daemon startup, resubscribe to all pools persisted in the database.
    /// Ensures no subscription is lost across restarts.
    pub async fn restore_subscriptions(&self) -> Result<(), DatabaseError> {
        let pools = self.repository.find_all().await?;
        let count = pools.len();
        for pool in pools {
            if pool.active {
                self.listener
                    .watch_pool(pool.protocol, pool.pool_address)
                    .await;
            }
        }
        info!(count, "subscriptions restored from database");
        Ok(())
    }
}
