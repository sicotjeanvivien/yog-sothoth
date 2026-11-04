use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::broadcast;
use yog_core::domain::{LiveEventFeed, LiveRecord, PoolChangeEvent, PoolChangeTable};
use yog_core::{RepositoryError, RepositoryResult};

use super::EventSource;
use crate::application::live::poller::poll_once;
use crate::application::live::watermark::LiveWatermark;

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
    let source = EventSource::new(Arc::new(OneBadRow), PoolChangeTable::MeteoraDammV2Status);
    let (tx, mut rx) = broadcast::channel(8);

    let watermark = poll_once(&source, &tx, Some(LiveWatermark::anchored(9)))
        .await
        .expect("kept");

    assert_eq!(watermark.floor(), 10);
    assert!(rx.try_recv().is_err(), "nothing is broadcast");
}
