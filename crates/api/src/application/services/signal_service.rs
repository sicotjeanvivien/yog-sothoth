//! The signal feed: pagination of the detectors' signals, each composed with its pool's token
//! pair. Pure domain.

use std::sync::Arc;

use solana_pubkey::Pubkey;
use yog_core::{
    Page, PageDirection, PagePosition, RepositoryError, RepositoryResult,
    domain::{Protocol, Severity, SignalCursor, SignalFeed, SignalFilter, SignalRecord},
};

use crate::application::{EnrichedSignal, Paired, PoolPairs};

/// Input to [`SignalService::list_signals`].
pub(crate) struct SignalListParams {
    pub severity: Option<Severity>,
    pub protocol: Option<Protocol>,
    pub pool: Option<Pubkey>,
    pub cursor: Option<SignalCursor>,
    pub direction: PageDirection,
    pub position: Option<PagePosition>,
    pub limit: i64,
}

/// Signal feed queries, over the [`SignalFeed`] lens and the pairs of their pools.
pub(crate) struct SignalService {
    repo: Arc<dyn SignalFeed>,
    pairs: Arc<PoolPairs>,
}

impl SignalService {
    pub(crate) fn new(repo: Arc<dyn SignalFeed>, pairs: Arc<PoolPairs>) -> Self {
        Self { repo, pairs }
    }

    /// Paginates the feed, each signal enriched with its pool's pair. An unknown or unresolved
    /// pool yields unresolved sides, never an error: the signal is the payload.
    pub(crate) async fn list_signals(
        &self,
        params: SignalListParams,
    ) -> Result<Page<EnrichedSignal>, RepositoryError> {
        let page = self
            .repo
            .list(
                SignalFilter {
                    severity: params.severity,
                    protocol: params.protocol,
                    pool: params.pool,
                },
                params.cursor,
                params.direction,
                params.position,
                params.limit,
            )
            .await?;

        let Page {
            items,
            next_cursor,
            prev_cursor,
            is_first,
            is_last,
        } = page;

        let items = self.enrich_batch(items).await?;

        Ok(Page {
            items,
            next_cursor,
            prev_cursor,
            is_first,
            is_last,
        })
    }

    /// Enriches a batch of signals, each distinct pool resolved once. ⚠️ The poller calls it
    /// once per tick, before broadcasting, so a signal's cost does not grow with open streams.
    pub(crate) async fn enrich_batch(
        &self,
        records: Vec<SignalRecord>,
    ) -> RepositoryResult<Vec<EnrichedSignal>> {
        let pairs = self.pairs.of(&records).await?;
        Ok(records
            .into_iter()
            .map(|record| {
                let Paired {
                    item: record,
                    token_a,
                    token_b,
                } = Paired::of(record, &pairs);
                EnrichedSignal {
                    record,
                    token_a,
                    token_b,
                }
            })
            .collect())
    }
}

#[cfg(test)]
#[path = "tests/signal_service_tests.rs"]
mod tests;
