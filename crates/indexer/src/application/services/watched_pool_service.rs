use std::sync::Arc;
use tracing::{info, warn};
use yog_core::domain::{Protocol, WatchedPoolRepository};

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
    /// The protocols whose extraction is written — see
    /// `ExtractionDispatcher::implemented_protocols`. A watched pool of any
    /// other protocol is skipped; the reason is on `restore_subscriptions`.
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

    /// On daemon startup, resubscribe to all pools persisted in the database.
    /// Ensures no subscription is lost across restarts.
    ///
    /// ⚠️ **A pool whose protocol has no working extractor is skipped**, and
    /// this guard exists because the same one on the other scope did not cover
    /// it. `INGEST_SCOPE=protocols` filters through
    /// `ExtractionDispatcher::implemented_protocols`; `pools` — the scope that
    /// actually runs today — subscribed to every active row regardless.
    /// `watched_pools.protocol` is plain `TEXT` with no `CHECK`, the allowlist
    /// is populated by hand, and `Protocol::from_str` accepts
    /// `"meteora_dlmm"` — so one INSERT was enough to have the indexer fetch
    /// every transaction of that pool and hand each to a stub that returns
    /// nothing. Exactly the spend the flag was added to prevent, on the half
    /// it did not reach.
    ///
    /// The skip is a `warn!` and not a silent filter: the row was put there on
    /// purpose, and a pool that is watched in the database but not on the wire
    /// is its own trap.
    ///
    /// ⚠️ **`count` changed meaning here, and the old one was wrong.** It used
    /// to be `pools.len()` — every row, *including the inactive ones* — under a
    /// message that says "subscriptions restored". It now counts what was
    /// handed to the source, with `skipped` beside it. Anyone comparing this
    /// line across the 10 September 2026 release will see the number drop
    /// without the allowlist changing; that is the log becoming true, not the
    /// indexer losing pools.
    ///
    /// The message lost the word "subscriptions" in the same pass. Under
    /// `INGEST_SCOPE=protocols` this set is populated and never read, so
    /// promising subscriptions would be a second false claim on the same
    /// line — and one nobody could check, since only the daemon knows the
    /// scope.
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

        // ⚠️ "registered", not "restored": under `INGEST_SCOPE=protocols` this
        // set is populated and never read — the subscription is built from the
        // protocol set instead — so a message promising subscriptions would be
        // believed for work that does not happen. The daemon populates both
        // sets whichever scope runs, and the `ingestion mode` line it logs
        // first is what says which one is in force.
        info!(count, skipped, "watched pools registered");
        Ok(())
    }
}

#[cfg(test)]
#[path = "watched_pool_service_tests.rs"]
mod tests;
