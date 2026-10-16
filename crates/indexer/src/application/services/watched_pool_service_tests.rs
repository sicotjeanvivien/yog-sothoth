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

/// ⚠️ `watched_pools.protocol` has no `CHECK`: one INSERT can name a protocol whose extractor is a
/// stub. Mutation this is written against: deleting the `contains` check in `restore_subscriptions`.
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

/// An inactive pool is skipped for another reason, a deliberate pause; one case per reason.
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

/// Nothing is filtered when everything is implemented and active: a guard refusing everything
/// would pass both tests above.
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

/// ⚠️ Rows exist but none can be extracted: the source gets nothing and the listener refuses with
/// a message about subscriptions; this asserts the shape that lets the daemon name the cause.
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
