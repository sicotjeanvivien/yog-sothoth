//! Unit tests for `PoolAccountWorker::run_one_cycle` against fakes.

use super::*;
use async_trait::async_trait;
use rust_decimal::Decimal;
use solana_pubkey::Pubkey;
use std::sync::Mutex;
use yog_core::RepositoryResult;

use crate::error::SourceError;
use crate::source::ResolvedPoolAccount;

fn pk(seed: u8) -> Pubkey {
    Pubkey::new_from_array([seed; 32])
}

type WrittenAccount = (Pubkey, Pubkey, Pubkey, Decimal, u8, u8, u8);

#[derive(Default)]
struct FakeRepo {
    unresolved: Vec<Pubkey>,
    written: Mutex<Vec<WrittenAccount>>,
}

#[async_trait]
impl PoolAccountResolver for FakeRepo {
    async fn list_unresolved(&self, _limit: i64) -> RepositoryResult<Vec<Pubkey>> {
        Ok(self.unresolved.clone())
    }
    #[allow(clippy::too_many_arguments)]
    async fn set_pool_account(
        &self,
        pool: &Pubkey,
        token_a_mint: &Pubkey,
        token_b_mint: &Pubkey,
        fee_bps: Decimal,
        protocol_fee_percent: u8,
        partner_fee_percent: u8,
        referral_fee_percent: u8,
    ) -> RepositoryResult<()> {
        self.written.lock().unwrap().push((
            *pool,
            *token_a_mint,
            *token_b_mint,
            fee_bps,
            protocol_fee_percent,
            partner_fee_percent,
            referral_fee_percent,
        ));
        Ok(())
    }
}

struct FakeSource {
    resolved: Vec<ResolvedPoolAccount>,
}

#[async_trait]
impl PoolAccountSource for FakeSource {
    async fn fetch_accounts(
        &self,
        _pools: &[Pubkey],
    ) -> Result<Vec<ResolvedPoolAccount>, SourceError> {
        Ok(self.resolved.clone())
    }
}

fn worker(repo: Arc<FakeRepo>, source: Arc<FakeSource>) -> PoolAccountWorker {
    PoolAccountWorker::new(repo, source, std::time::Duration::from_secs(10))
}

#[tokio::test]
async fn resolves_and_writes_mints_and_fee() {
    let repo = Arc::new(FakeRepo {
        unresolved: vec![pk(1)],
        written: Mutex::new(Vec::new()),
    });
    let source = Arc::new(FakeSource {
        resolved: vec![ResolvedPoolAccount {
            pool: pk(1),
            token_a_mint: pk(2),
            token_b_mint: pk(3),
            fee_bps: Decimal::new(25, 0),
            protocol_fee_percent: 20,
            partner_fee_percent: 0,
            referral_fee_percent: 20,
        }],
    });

    worker(repo.clone(), source).run_one_cycle().await;

    let written = repo.written.lock().unwrap();
    assert_eq!(
        written.as_slice(),
        &[(pk(1), pk(2), pk(3), Decimal::new(25, 0), 20, 0, 20)]
    );
}

#[tokio::test]
async fn no_unresolved_pools_writes_nothing() {
    let repo = Arc::new(FakeRepo::default());
    let source = Arc::new(FakeSource {
        resolved: Vec::new(),
    });

    worker(repo.clone(), source).run_one_cycle().await;

    assert!(repo.written.lock().unwrap().is_empty());
}
