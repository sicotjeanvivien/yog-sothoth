use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use tokio_util::sync::CancellationToken;
use yog_core::RepositoryResult;
use yog_core::domain::{LiveEventFeed, LiveRecord, PoolChangeEvent, PoolChangeTable};

use super::LivePollers;

/// A feed with no event, which records the tables anchored on.
#[derive(Default)]
struct AnchorsFeed(Mutex<Vec<PoolChangeTable>>);

#[async_trait]
impl LiveEventFeed<PoolChangeTable, PoolChangeEvent> for AnchorsFeed {
    async fn live_tip(
        &self,
        table: PoolChangeTable,
        _lookback: chrono::Duration,
    ) -> RepositoryResult<Option<i64>> {
        self.0.lock().unwrap().push(table);
        Ok(None)
    }

    async fn live_after(
        &self,
        _table: PoolChangeTable,
        _after_id: i64,
        _lookback: chrono::Duration,
        _limit: i64,
    ) -> RepositoryResult<Vec<LiveRecord<RepositoryResult<PoolChangeEvent>>>> {
        Ok(Vec::new())
    }
}

/// Mutation: one poller for the collection, or the tables after the first left out — a change of
/// pool fees never streamed.
#[tokio::test]
async fn every_table_of_a_collection_has_its_poller_on_the_topics_channel() {
    let feed = Arc::new(AnchorsFeed::default());
    let mut pollers = LivePollers::new(Duration::from_secs(3600));
    let sender = pollers.add_collection(feed.clone());
    let _listener = sender.subscribe();
    let shutdown = CancellationToken::new();
    let running = tokio::spawn(pollers.run(shutdown.clone()));

    tokio::time::sleep(Duration::from_millis(100)).await;
    shutdown.cancel();
    tokio::time::timeout(Duration::from_secs(1), running)
        .await
        .expect("every poller stops")
        .unwrap()
        .unwrap();

    let mut anchored = feed.0.lock().unwrap().clone();
    anchored.sort_by_key(|table| format!("{table:?}"));
    assert_eq!(
        anchored,
        vec![
            PoolChangeTable::MeteoraDammV2Fees,
            PoolChangeTable::MeteoraDammV2Status
        ]
    );
}

/// A feed whose anchoring takes a while, and which records how many run at once.
#[derive(Default)]
struct SlowFeed {
    running: AtomicUsize,
    most: AtomicUsize,
}

#[async_trait]
impl LiveEventFeed<PoolChangeTable, PoolChangeEvent> for SlowFeed {
    async fn live_tip(
        &self,
        _table: PoolChangeTable,
        _lookback: chrono::Duration,
    ) -> RepositoryResult<Option<i64>> {
        let now = self.running.fetch_add(1, Ordering::SeqCst) + 1;
        self.most.fetch_max(now, Ordering::SeqCst);
        tokio::time::sleep(Duration::from_millis(30)).await;
        self.running.fetch_sub(1, Ordering::SeqCst);
        Ok(None)
    }

    async fn live_after(
        &self,
        _table: PoolChangeTable,
        _after_id: i64,
        _lookback: chrono::Duration,
        _limit: i64,
    ) -> RepositoryResult<Vec<LiveRecord<RepositoryResult<PoolChangeEvent>>>> {
        Ok(Vec::new())
    }
}

/// Mutation: a read slot per poller — the two tables of `pool-changes` read at once, and 19
/// pollers would take 19 connections of the routes' pool.
#[tokio::test]
async fn the_pollers_read_one_at_a_time() {
    let feed = Arc::new(SlowFeed::default());
    let mut pollers = LivePollers::new(Duration::from_secs(3600));
    let sender = pollers.add_collection(feed.clone());
    let _listener = sender.subscribe();
    let shutdown = CancellationToken::new();
    let running = tokio::spawn(pollers.run(shutdown.clone()));

    tokio::time::sleep(Duration::from_millis(150)).await;
    shutdown.cancel();
    running.await.unwrap().unwrap();

    assert_eq!(feed.most.load(Ordering::SeqCst), 1);
}
