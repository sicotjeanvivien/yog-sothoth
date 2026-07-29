//! Unit tests for `PoolAccountWorker::run_one_cycle` against fakes.
//!
//! The fake source now yields **raw accounts**, so these run through the real
//! `core` decoder rather than around it — which is the point of the refactor:
//! the worker no longer knows any layout, so its tests exercise the seam.

use super::*;
use async_trait::async_trait;
use rust_decimal::Decimal;
use solana_pubkey::Pubkey;
use std::sync::Mutex;
use yog_core::RepositoryResult;
use yog_core::amm::damm_v2::BaseFeeKind;

use yog_core::domain::{
    MeteoraDammV2PoolAccountProperties, PoolAccountCore, PoolAccountProperties,
    PoolAccountResolver, PoolRepository, Protocol,
};

use crate::error::SourceError;
use crate::source::RawAccount;

fn pk(seed: u8) -> Pubkey {
    Pubkey::new_from_array([seed; 32])
}

/// A cp-amm `Pool` account buffer. Offsets mirror
/// `core::application::decoder::meteora::damm_v2` — if they ever drift,
/// these fail alongside the decoder's own tests.
fn cp_amm_account(token_a: Pubkey, token_b: Pubkey) -> Vec<u8> {
    let mut bytes = vec![0u8; 1112];
    bytes[..8].copy_from_slice(&[0xf1, 0x9a, 0x6d, 0x04, 0x11, 0xb1, 0x6d, 0xbc]);
    bytes[8..16].copy_from_slice(&2_500_000u64.to_le_bytes()); // 25 bps
    bytes[16] = 0; // BaseFeeMode::FeeTimeSchedulerLinear
    bytes[22..24].copy_from_slice(&0u16.to_le_bytes()); // no periods → constant
    bytes[48] = 20; // protocol
    bytes[49] = 0; // padding_0 — decoded by nothing
    bytes[50] = 20; // referral
    bytes[56] = 1; // dynamic_fee.initialized
    bytes[168..200].copy_from_slice(token_a.as_ref());
    bytes[200..232].copy_from_slice(token_b.as_ref());
    bytes
}

fn expected_properties() -> PoolAccountProperties {
    PoolAccountProperties::MeteoraDammV2(MeteoraDammV2PoolAccountProperties {
        protocol_fee_percent: 20,
        referral_fee_percent: 20,
        base_fee_kind: Some(BaseFeeKind::Constant),
        has_dynamic_fee: true,
    })
}

fn expected_core(token_a: Pubkey, token_b: Pubkey) -> PoolAccountCore {
    PoolAccountCore {
        token_a_mint: token_a,
        token_b_mint: token_b,
        fee_bps: Decimal::new(25, 0),
    }
}

/// Every write both fakes receive, in order, so a test can assert on the
/// *sequence* and not merely on the contents.
type Journal = Arc<Mutex<Vec<Write>>>;

#[derive(Debug, PartialEq, Eq)]
enum Write {
    Satellite(Pubkey, PoolAccountProperties),
    Registry(Pubkey, PoolAccountCore),
}

#[derive(Default)]
struct FakeRepo {
    unresolved: Vec<Pubkey>,
    journal: Journal,
    /// When true, the satellite write fails — the case that must stop the
    /// registry write from lowering the refresh flag.
    fail: bool,
}

#[async_trait]
impl PoolAccountResolver for FakeRepo {
    fn protocol(&self) -> Protocol {
        Protocol::MeteoraDammV2
    }
    async fn list_unresolved(&self, _limit: i64) -> RepositoryResult<Vec<Pubkey>> {
        Ok(self.unresolved.clone())
    }
    async fn set_pool_account(
        &self,
        pool: &Pubkey,
        properties: &PoolAccountProperties,
    ) -> RepositoryResult<()> {
        if self.fail {
            return Err(yog_core::RepositoryError::Integrity("boom".into()));
        }
        self.journal
            .lock()
            .unwrap()
            .push(Write::Satellite(*pool, properties.clone()));
        Ok(())
    }
}

/// The neutral registry. Shares the journal with the resolver, so the ordering
/// between the two writes is observable.
#[derive(Default)]
struct FakePoolRepo {
    journal: Journal,
}

#[async_trait]
impl PoolRepository for FakePoolRepo {
    async fn upsert(&self, _: &yog_core::domain::Pool) -> RepositoryResult<()> {
        unreachable!("the worker never discovers pools")
    }
    async fn touch_last_seen(&self, _: &Pubkey) -> RepositoryResult<()> {
        unreachable!("the worker never touches last_seen")
    }
    async fn mark_needs_refresh(&self, _: &Pubkey) -> RepositoryResult<()> {
        unreachable!("raising the flag is the indexer's job, never this worker's")
    }
    async fn set_account_core(
        &self,
        pool: &Pubkey,
        core: &PoolAccountCore,
    ) -> RepositoryResult<()> {
        self.journal
            .lock()
            .unwrap()
            .push(Write::Registry(*pool, core.clone()));
        Ok(())
    }
}

struct FakeSource {
    accounts: Vec<RawAccount>,
}

#[async_trait]
impl PoolAccountSource for FakeSource {
    async fn fetch_accounts(
        &self,
        _pool_addresses: &[Pubkey],
    ) -> Result<Vec<RawAccount>, SourceError> {
        Ok(self.accounts.clone())
    }
}

fn worker(
    repo: Arc<FakeRepo>,
    pool_repo: Arc<FakePoolRepo>,
    source: Arc<FakeSource>,
) -> PoolAccountWorker {
    PoolAccountWorker::new(
        vec![repo],
        pool_repo,
        source,
        std::time::Duration::from_secs(10),
    )
}

/// A resolver and a registry repository sharing one journal.
fn fakes(unresolved: Vec<Pubkey>, fail: bool) -> (Arc<FakeRepo>, Arc<FakePoolRepo>, Journal) {
    let journal: Journal = Arc::new(Mutex::new(Vec::new()));
    (
        Arc::new(FakeRepo {
            unresolved,
            journal: journal.clone(),
            fail,
        }),
        Arc::new(FakePoolRepo {
            journal: journal.clone(),
        }),
        journal,
    )
}

/// The two halves of one account read reach **two different repositories**, and
/// in a fixed order: the protocol's satellite first, the neutral registry last.
///
/// The order is not cosmetic. The registry write is what lowers
/// `pools.needs_refresh`, so it must be the last thing to succeed — otherwise a
/// satellite failure would leave a pool marked fresh while half of its
/// properties are stale.
#[tokio::test]
async fn writes_both_halves_satellite_first() {
    let (repo, pool_repo, journal) = fakes(vec![pk(1)], false);
    let source = Arc::new(FakeSource {
        accounts: vec![RawAccount {
            pool_address: pk(1),
            program_id: Protocol::MeteoraDammV2.program_id(),
            data: cp_amm_account(pk(2), pk(3)),
        }],
    });

    worker(repo, pool_repo, source).run_one_cycle().await;

    assert_eq!(
        *journal.lock().unwrap(),
        vec![
            Write::Satellite(pk(1), expected_properties()),
            Write::Registry(pk(1), expected_core(pk(2), pk(3))),
        ]
    );
}

/// A failed satellite write must **not** be followed by the registry write.
///
/// This is the guard for the invariant above: letting the registry write run
/// anyway would clear the refresh flag, and the pool would leave the queue with
/// stale satellite properties and nothing to bring it back.
#[tokio::test]
async fn a_failed_satellite_write_does_not_clear_the_flag() {
    let (repo, pool_repo, journal) = fakes(vec![pk(1)], true);
    let source = Arc::new(FakeSource {
        accounts: vec![RawAccount {
            pool_address: pk(1),
            program_id: Protocol::MeteoraDammV2.program_id(),
            data: cp_amm_account(pk(2), pk(3)),
        }],
    });

    worker(repo, pool_repo, source).run_one_cycle().await;

    assert!(
        journal.lock().unwrap().is_empty(),
        "the registry write must be skipped when the satellite write failed"
    );
}

#[tokio::test]
async fn no_unresolved_pools_writes_nothing() {
    let (repo, pool_repo, journal) = fakes(Vec::new(), false);
    let source = Arc::new(FakeSource {
        accounts: Vec::new(),
    });

    worker(repo, pool_repo, source).run_one_cycle().await;

    assert!(journal.lock().unwrap().is_empty());
}

/// An account the decoder does not recognize is skipped — not written, not
/// fatal. The pool simply stays in the queue for the next cycle.
#[tokio::test]
async fn an_undecodable_account_is_skipped() {
    let (repo, pool_repo, journal) = fakes(vec![pk(1)], false);
    let source = Arc::new(FakeSource {
        accounts: vec![RawAccount {
            pool_address: pk(1),
            program_id: pk(99), // a program we do not index
            data: cp_amm_account(pk(2), pk(3)),
        }],
    });

    worker(repo, pool_repo, source).run_one_cycle().await;

    assert!(journal.lock().unwrap().is_empty());
}

/// The guard that makes the dispatch safe: an account belonging to another
/// protocol is never handed to this resolver, so a resolver is never asked to
/// store a payload it does not own.
#[tokio::test]
async fn an_account_of_another_protocol_is_not_written() {
    let (repo, pool_repo, journal) = fakes(vec![pk(1)], false);
    let source = Arc::new(FakeSource {
        accounts: vec![RawAccount {
            pool_address: pk(1),
            program_id: Protocol::MeteoraDlmm.program_id(),
            data: vec![0u8; 904],
        }],
    });

    worker(repo, pool_repo, source).run_one_cycle().await;

    assert!(journal.lock().unwrap().is_empty());
}
