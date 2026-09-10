use super::*;

use std::sync::Mutex;

use async_trait::async_trait;
use chrono::Utc;
use solana_pubkey::Pubkey;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use yog_core::{
    RepositoryResult,
    domain::WatchedPool,
    domain::{Protocol, WatchedPoolRepository},
};

use crate::{application::source::IngestedTransaction, error::SourceError};

/// A source that records what it was told to watch, and delivers nothing.
#[derive(Default)]
struct RecordingSource {
    watched_pools: Mutex<Vec<(Protocol, Pubkey)>>,
}

#[async_trait]
impl TransactionSource for RecordingSource {
    async fn watch_protocol(&self, _protocol: Protocol) {}

    async fn watch_pool(&self, protocol: Protocol, pool_address: Pubkey) {
        self.watched_pools
            .lock()
            .expect("no other thread poisons this")
            .push((protocol, pool_address));
    }

    async fn run(
        &self,
        _downstream: mpsc::Sender<IngestedTransaction>,
        _shutdown: CancellationToken,
    ) -> Result<(), SourceError> {
        Ok(())
    }
}

struct StubRepository {
    pools: Vec<WatchedPool>,
}

#[async_trait]
impl WatchedPoolRepository for StubRepository {
    async fn add(&self, _pool: &WatchedPool) -> RepositoryResult<()> {
        unreachable!("restore_subscriptions only reads")
    }

    async fn exists(&self, _address: Pubkey) -> RepositoryResult<bool> {
        unreachable!("restore_subscriptions only reads")
    }

    async fn remove(&self, _address: Pubkey) -> RepositoryResult<()> {
        unreachable!("restore_subscriptions only reads")
    }

    async fn find_all(&self) -> RepositoryResult<Vec<WatchedPool>> {
        Ok(self.pools.clone())
    }
}

fn pool(seed: u8, protocol: Protocol, active: bool) -> WatchedPool {
    WatchedPool {
        pool_address: Pubkey::new_from_array([seed; 32]),
        protocol,
        active,
        added_at: Utc::now(),
        note: None,
    }
}

fn service(
    pools: Vec<WatchedPool>,
    implemented: Vec<Protocol>,
) -> (WatchedPoolService, Arc<RecordingSource>) {
    let source = Arc::new(RecordingSource::default());
    let service = WatchedPoolService::new(
        Arc::clone(&source) as Arc<dyn TransactionSource>,
        Arc::new(StubRepository { pools }),
        implemented,
    );
    (service, source)
}

fn watched(source: &RecordingSource) -> Vec<(Protocol, Pubkey)> {
    source
        .watched_pools
        .lock()
        .expect("no other thread poisons this")
        .clone()
}

/// ⚠️ This test carries a spending decision, and it is the half that runs.
///
/// `INGEST_SCOPE=pools` is the shipped default, and `watched_pools.protocol` is
/// plain `TEXT` with no `CHECK` — one hand-written INSERT is enough to name a
/// protocol whose extractor is a stub. Subscribing to it fetches every
/// transaction of that pool and hands each to something that returns nothing.
///
/// Put in failure before being trusted: deleting the `contains` check in
/// `restore_subscriptions` turns this red. Its sibling in `yog-core` guards the
/// `protocols` scope and was mutation-proven the same way; this one was written
/// afterwards, because a guard on one scope of two is the defect this whole
/// slice kept repeating.
#[tokio::test]
async fn a_pool_whose_protocol_has_no_extractor_is_not_subscribed_to() {
    let (service, source) = service(
        vec![
            pool(1, Protocol::MeteoraDammV2, true),
            pool(2, Protocol::MeteoraDlmm, true),
        ],
        vec![Protocol::MeteoraDammV2],
    );

    service
        .restore_subscriptions()
        .await
        .expect("the stub repository cannot fail");

    assert_eq!(
        watched(&source),
        vec![(Protocol::MeteoraDammV2, Pubkey::new_from_array([1; 32]))],
        "only the pool whose protocol can be extracted may reach the source"
    );
}

/// The other reason a row is skipped, and it must not be confused with the
/// first: an inactive pool is a deliberate pause, not an unimplemented
/// protocol. One case per reason.
#[tokio::test]
async fn an_inactive_pool_is_not_subscribed_to_either() {
    let (service, source) = service(
        vec![pool(3, Protocol::MeteoraDammV2, false)],
        vec![Protocol::MeteoraDammV2],
    );

    service
        .restore_subscriptions()
        .await
        .expect("the stub repository cannot fail");

    assert!(watched(&source).is_empty());
}

/// And the case that must keep working: nothing is filtered when everything is
/// implemented and active. A guard that refuses everything would pass both
/// tests above.
#[tokio::test]
async fn every_active_pool_of_an_implemented_protocol_is_subscribed_to() {
    let (service, source) = service(
        vec![
            pool(4, Protocol::MeteoraDammV2, true),
            pool(5, Protocol::MeteoraDammV2, true),
        ],
        vec![Protocol::MeteoraDammV2],
    );

    service
        .restore_subscriptions()
        .await
        .expect("the stub repository cannot fail");

    assert_eq!(watched(&source).len(), 2);
}

/// ⚠️ The dead end, which is not the same as an empty allowlist.
///
/// Rows exist, all name a protocol nothing can extract, and the source is
/// handed nothing — so the listener refuses to start with
/// `NoSubscriptionTargets`, a message about subscriptions that says nothing
/// about protocols. This asserts the shape that lets the daemon say so while
/// the cause is still in hand.
#[tokio::test]
async fn an_allowlist_of_only_unimplemented_protocols_subscribes_to_nothing() {
    let (service, source) = service(
        vec![
            pool(6, Protocol::MeteoraDlmm, true),
            pool(7, Protocol::MeteoraDlmm, true),
        ],
        vec![Protocol::MeteoraDammV2],
    );

    service
        .restore_subscriptions()
        .await
        .expect("skipping every row is not a repository failure");

    assert!(
        watched(&source).is_empty(),
        "nothing may reach the source, which is what makes this a dead end"
    );
}
