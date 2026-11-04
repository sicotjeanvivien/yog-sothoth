use std::sync::Arc;

use async_trait::async_trait;
use solana_signature::Signature;
use tokio::sync::broadcast;
use yog_core::domain::{
    LiveEventFeed, LiveRecord, MeteoraDammV2PoolChange, MeteoraDammV2SetPoolStatusEvent,
    PoolChangeEvent, PoolChangeTable,
};
use yog_core::{RepositoryError, RepositoryResult};

use super::EventSource;
use crate::application::live::poller::poll_once;
use crate::application::live::watermark::LiveWatermark;
use crate::testing::{MockMetadataRepo, make_metadata, make_pool, no_pairs, pk, pool_pairs};

/// One row past the anchor, which does not decode.
struct OneBadRow;

#[async_trait]
impl LiveEventFeed<PoolChangeTable, PoolChangeEvent> for OneBadRow {
    async fn live_tip(
        &self,
        _table: PoolChangeTable,
        _lookback: chrono::Duration,
    ) -> RepositoryResult<Option<i64>> {
        Ok(Some(9))
    }

    async fn live_after(
        &self,
        _table: PoolChangeTable,
        _after_id: i64,
        _lookback: chrono::Duration,
        _limit: i64,
    ) -> RepositoryResult<Vec<LiveRecord<RepositoryResult<PoolChangeEvent>>>> {
        Ok(vec![LiveRecord {
            id: 10,
            event: Err(RepositoryError::Integrity("bad row".into())),
        }])
    }
}

/// Mutation: an undecodable row broadcast, or left out of the watermark — the floor stays under
/// it, and the next ticks wait on a hole that is not one.
#[tokio::test]
async fn an_undecodable_row_is_passed_over_and_counted_as_read() {
    let source = EventSource::new(
        Arc::new(OneBadRow),
        PoolChangeTable::MeteoraDammV2Status,
        no_pairs(),
    );
    let (tx, mut rx) = broadcast::channel(8);

    let watermark = poll_once(&source, &tx, Some(LiveWatermark::anchored(9)))
        .await
        .expect("kept");

    assert_eq!(watermark.floor(), 10);
    assert!(rx.try_recv().is_err(), "nothing is broadcast");
}

/// One status change on pool `pk(1)` past the anchor.
struct OneChange;

#[async_trait]
impl LiveEventFeed<PoolChangeTable, PoolChangeEvent> for OneChange {
    async fn live_tip(
        &self,
        _table: PoolChangeTable,
        _lookback: chrono::Duration,
    ) -> RepositoryResult<Option<i64>> {
        Ok(Some(9))
    }

    async fn live_after(
        &self,
        _table: PoolChangeTable,
        _after_id: i64,
        _lookback: chrono::Duration,
        _limit: i64,
    ) -> RepositoryResult<Vec<LiveRecord<RepositoryResult<PoolChangeEvent>>>> {
        Ok(vec![LiveRecord {
            id: 10,
            event: Ok(PoolChangeEvent::MeteoraDammV2(
                MeteoraDammV2PoolChange::Status(MeteoraDammV2SetPoolStatusEvent {
                    pool_address: pk(1),
                    signature: Signature::from([1; 64]),
                    timestamp: chrono::Utc::now(),
                    slot: 9,
                    transaction_index: None,
                    event_index: 0,
                    status: 0,
                }),
            )),
        }])
    }
}

async fn broadcast_once(pairs: std::sync::Arc<crate::application::PoolPairs>) -> Vec<String> {
    let source = EventSource::new(
        Arc::new(OneChange),
        PoolChangeTable::MeteoraDammV2Status,
        pairs,
    );
    let (tx, mut rx) = broadcast::channel(8);

    poll_once(&source, &tx, Some(LiveWatermark::anchored(9)))
        .await
        .expect("kept");

    let sent = rx.try_recv().expect("the change is broadcast");
    assert_eq!(sent.record.id, 10);
    [&sent.record.event.token_a, &sent.record.event.token_b]
        .into_iter()
        .map(|t| {
            t.metadata
                .as_ref()
                .and_then(|m| m.symbol.clone())
                .unwrap_or_default()
        })
        .collect()
}

/// Mutation: the event broadcast without its pair — the activity table shows no token.
#[tokio::test]
async fn an_event_is_broadcast_with_its_pools_pair() {
    let pairs = pool_pairs(
        vec![make_pool(pk(1), pk(2), pk(3))],
        MockMetadataRepo::with(vec![
            (pk(2), make_metadata(pk(2), "MET")),
            (pk(3), make_metadata(pk(3), "SOL")),
        ]),
    );

    assert_eq!(broadcast_once(pairs).await, ["MET", "SOL"]);
}

/// Mutation: a failed pair holding the batch back — the events are lost to the live stream.
#[tokio::test]
async fn a_failed_pair_still_broadcasts_the_event_unpaired() {
    let pairs = pool_pairs(
        vec![make_pool(pk(1), pk(2), pk(3))],
        MockMetadataRepo::failing(),
    );

    assert_eq!(broadcast_once(pairs).await, ["", ""]);
}
