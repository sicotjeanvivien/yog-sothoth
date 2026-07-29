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
    MeteoraDammV2PoolAccountProperties, PoolAccountProperties, PoolAccountResolver, Protocol,
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

fn expected_properties(token_a: Pubkey, token_b: Pubkey) -> PoolAccountProperties {
    PoolAccountProperties::MeteoraDammV2(MeteoraDammV2PoolAccountProperties {
        token_a_mint: token_a,
        token_b_mint: token_b,
        fee_bps: Decimal::new(25, 0),
        protocol_fee_percent: 20,
        referral_fee_percent: 20,
        base_fee_kind: Some(BaseFeeKind::Constant),
        has_dynamic_fee: true,
    })
}

#[derive(Default)]
struct FakeRepo {
    unresolved: Vec<Pubkey>,
    written: Mutex<Vec<(Pubkey, PoolAccountProperties)>>,
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
        self.written
            .lock()
            .unwrap()
            .push((*pool, properties.clone()));
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

fn worker(repo: Arc<FakeRepo>, source: Arc<FakeSource>) -> PoolAccountWorker {
    PoolAccountWorker::new(vec![repo], source, std::time::Duration::from_secs(10))
}

#[tokio::test]
async fn decodes_and_writes_the_account_properties() {
    let repo = Arc::new(FakeRepo {
        unresolved: vec![pk(1)],
        written: Mutex::new(Vec::new()),
    });
    let source = Arc::new(FakeSource {
        accounts: vec![RawAccount {
            pool_address: pk(1),
            program_id: Protocol::MeteoraDammV2.program_id(),
            data: cp_amm_account(pk(2), pk(3)),
        }],
    });

    worker(repo.clone(), source).run_one_cycle().await;

    let written = repo.written.lock().unwrap();
    assert_eq!(
        written.as_slice(),
        &[(pk(1), expected_properties(pk(2), pk(3)))]
    );
}

#[tokio::test]
async fn no_unresolved_pools_writes_nothing() {
    let repo = Arc::new(FakeRepo::default());
    let source = Arc::new(FakeSource {
        accounts: Vec::new(),
    });

    worker(repo.clone(), source).run_one_cycle().await;

    assert!(repo.written.lock().unwrap().is_empty());
}

/// An account the decoder does not recognize is skipped — not written, not
/// fatal. The pool simply stays in the queue for the next cycle.
#[tokio::test]
async fn an_undecodable_account_is_skipped() {
    let repo = Arc::new(FakeRepo {
        unresolved: vec![pk(1)],
        written: Mutex::new(Vec::new()),
    });
    let source = Arc::new(FakeSource {
        accounts: vec![RawAccount {
            pool_address: pk(1),
            program_id: pk(99), // a program we do not index
            data: cp_amm_account(pk(2), pk(3)),
        }],
    });

    worker(repo.clone(), source).run_one_cycle().await;

    assert!(repo.written.lock().unwrap().is_empty());
}

/// The guard that makes the dispatch safe: an account belonging to another
/// protocol is never handed to this resolver, so a resolver is never asked to
/// store a payload it does not own.
#[tokio::test]
async fn an_account_of_another_protocol_is_not_written() {
    let repo = Arc::new(FakeRepo {
        unresolved: vec![pk(1)],
        written: Mutex::new(Vec::new()),
    });
    let source = Arc::new(FakeSource {
        accounts: vec![RawAccount {
            pool_address: pk(1),
            program_id: Protocol::MeteoraDlmm.program_id(),
            data: vec![0u8; 904],
        }],
    });

    worker(repo.clone(), source).run_one_cycle().await;

    assert!(repo.written.lock().unwrap().is_empty());
}
