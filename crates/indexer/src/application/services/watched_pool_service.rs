use std::sync::Arc;
use tracing::{error, info, warn};
use yog_core::domain::{Protocol, WatchedPoolRepository};

use crate::{application::source::TransactionSource, error::DatabaseError};

/// Keeps the database and the source in sync: a pool persisted in `watched_pools` is subscribed
/// to, through [`TransactionSource`], whatever subscribing means on the path running.
pub(crate) struct WatchedPoolService {
    source: Arc<dyn TransactionSource>,
    repository: Arc<dyn WatchedPoolRepository>,
    /// The protocols whose extraction is written; see `restore_subscriptions`.
    implemented_protocols: Vec<Protocol>,
}

impl WatchedPoolService {
    pub(crate) fn new(
        source: Arc<dyn TransactionSource>,
        repository: Arc<dyn WatchedPoolRepository>,
        implemented_protocols: Vec<Protocol>,
    ) -> Self {
        Self {
            source,
            repository,
            implemented_protocols,
        }
    }

    /// On start-up under `INGEST_SCOPE=pools`, resubscribe every active pool in the database.
    /// ⚠️ A pool whose protocol is not in `ExtractionDispatcher::implemented_protocols` is skipped
    /// with a `warn!`: `watched_pools.protocol` has no `CHECK`, and a stub would buy a firehose.
    pub(crate) async fn restore_subscriptions(&self) -> Result<(), DatabaseError> {
        let pools = self.repository.find_all().await?;
        let mut count = 0usize;
        let mut skipped = 0usize;

        for pool in pools {
            if !pool.active {
                continue;
            }
            if !self.implemented_protocols.contains(&pool.protocol) {
                warn!(
                    pool = %pool.pool_address,
                    protocol = %pool.protocol.as_str(),
                    "watched pool skipped — no working extractor for its protocol, so subscribing would fetch transactions nothing can decode"
                );
                skipped += 1;
                continue;
            }
            self.source
                .watch_pool(pool.protocol, pool.pool_address)
                .await;
            count += 1;
        }

        // ⚠️ Every row skipped is a dead end: the listener will refuse with `NoSubscriptionTargets`,
        // knowing nothing of the skip; the `error!` names the cause while it is in hand.
        if count == 0 && skipped > 0 {
            error!(
                skipped,
                "every active watched pool names a protocol with no working extractor — the listener has nothing to subscribe to and will refuse to start"
            );
        }

        info!(count, skipped, "watched pools registered");
        Ok(())
    }
}

#[cfg(test)]
#[path = "watched_pool_service_tests.rs"]
mod tests;
