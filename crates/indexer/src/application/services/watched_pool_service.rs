use std::sync::Arc;
use tracing::info;
use yog_core::domain::WatchedPoolRepository;

use crate::{application::source::TransactionSource, error::DatabaseError};

/// Manages the lifecycle of pool subscriptions.
///
/// Single responsibility : keep the database and the source in sync — a pool
/// persisted in the database must always be subscribed to, and vice versa.
///
/// It depends on [`TransactionSource`] and not on a concrete listener: what
/// "subscribe to a pool" means is one WebSocket per address on one path and one
/// entry in a filter on the other, and this service has no business knowing
/// which.
pub(crate) struct WatchedPoolService {
    source: Arc<dyn TransactionSource>,
    repository: Arc<dyn WatchedPoolRepository>,
}

impl WatchedPoolService {
    pub(crate) fn new(
        source: Arc<dyn TransactionSource>,
        repository: Arc<dyn WatchedPoolRepository>,
    ) -> Self {
        Self { source, repository }
    }

    /// On daemon startup, resubscribe to all pools persisted in the database.
    /// Ensures no subscription is lost across restarts.
    pub(crate) async fn restore_subscriptions(&self) -> Result<(), DatabaseError> {
        let pools = self.repository.find_all().await?;
        let count = pools.len();
        for pool in pools {
            if pool.active {
                self.source
                    .watch_pool(pool.protocol, pool.pool_address)
                    .await;
            }
        }
        info!(count, "subscriptions restored from database");
        Ok(())
    }
}
