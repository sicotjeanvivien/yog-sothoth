use std::sync::Arc;

use tracing::info;
use yog_core::{
    domain::{WatchedPool, WatchedPoolRepository},
    CoreResult,
};

use crate::infra::RpcListener;

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

    /// Persist a pool and register its WebSocket subscription.
    pub async fn watch(&self, pool: WatchedPool) -> CoreResult<()> {
        self.repository.add(&pool).await?;
        self.listener.watch(pool.pool_address.to_string()).await;
        info!(address = %pool.pool_address, protocol = %pool.protocol, "pool watch registered");
        Ok(())
    }

    /// Remove a pool from persistence and cancel its WebSocket subscription.
    pub async fn unwatch(&self, address: &str) -> CoreResult<()> {
        self.repository.remove(address).await?;
        self.listener.unwatch(address.to_string()).await;
        info!(address = %address, "pool watch removed");
        Ok(())
    }

    /// On daemon startup, resubscribe to all pools persisted in the database.
    /// Ensures no subscription is lost across restarts.
    pub async fn restore_subscriptions(&self) -> CoreResult<()> {
        let pools = self.repository.find_all().await?;
        let count = pools.len();
        for pool in pools {
            self.listener.watch(pool.pool_address.to_string()).await;
        }
        info!(count, "subscriptions restored from database");
        Ok(())
    }
}
