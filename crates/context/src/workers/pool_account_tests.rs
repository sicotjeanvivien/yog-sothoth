//! Unit tests for `PoolAccountWorker::run_one_cycle` against fakes.

use super::*;
use async_trait::async_trait;
use rust_decimal::Decimal;
use solana_pubkey::Pubkey;
use std::sync::Mutex;
use yog_core::RepositoryResult;

use yog_core::domain::PoolAccountProperties;

use crate::error::SourceError;
use crate::source::ResolvedPoolAccount;

fn pk(seed: u8) -> Pubkey {
    Pubkey::new_from_array([seed; 32])
}

#[derive(Default)]
struct FakeRepo {
    unresolved: Vec<Pubkey>,
    written: Mutex<Vec<(Pubkey, PoolAccountProperties)>>,
}

#[async_trait]
impl PoolAccountResolver for FakeRepo {
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
    let properties = PoolAccountProperties {
        token_a_mint: pk(2),
        token_b_mint: pk(3),
        fee_bps: Decimal::new(25, 0),
        protocol_fee_percent: 20,
        partner_fee_percent: 0,
        referral_fee_percent: 20,
    };
    let source = Arc::new(FakeSource {
        resolved: vec![ResolvedPoolAccount {
            pool: pk(1),
            properties: properties.clone(),
        }],
    });

    worker(repo.clone(), source).run_one_cycle().await;

    let written = repo.written.lock().unwrap();
    assert_eq!(written.as_slice(), &[(pk(1), properties)]);
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
