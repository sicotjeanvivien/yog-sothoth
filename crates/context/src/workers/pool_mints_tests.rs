//! Unit tests for `PoolMintsWorker::run_one_cycle` against fakes.

use super::*;
use async_trait::async_trait;
use solana_pubkey::Pubkey;
use std::sync::Mutex;
use yog_core::RepositoryResult;

use crate::error::SourceError;
use crate::source::ResolvedPoolMints;

fn pk(seed: u8) -> Pubkey {
    Pubkey::new_from_array([seed; 32])
}

#[derive(Default)]
struct FakeRepo {
    unresolved: Vec<Pubkey>,
    written: Mutex<Vec<(Pubkey, Pubkey, Pubkey)>>,
}

#[async_trait]
impl PoolMintResolver for FakeRepo {
    async fn list_unresolved(&self, _limit: i64) -> RepositoryResult<Vec<Pubkey>> {
        Ok(self.unresolved.clone())
    }
    async fn set_mints(
        &self,
        pool: &Pubkey,
        token_a_mint: &Pubkey,
        token_b_mint: &Pubkey,
    ) -> RepositoryResult<()> {
        self.written
            .lock()
            .unwrap()
            .push((*pool, *token_a_mint, *token_b_mint));
        Ok(())
    }
}

struct FakeSource {
    resolved: Vec<ResolvedPoolMints>,
}

#[async_trait]
impl PoolAccountSource for FakeSource {
    async fn fetch_mints(&self, _pools: &[Pubkey]) -> Result<Vec<ResolvedPoolMints>, SourceError> {
        Ok(self.resolved.clone())
    }
}

fn worker(repo: Arc<FakeRepo>, source: Arc<FakeSource>) -> PoolMintsWorker {
    PoolMintsWorker::new(repo, source, std::time::Duration::from_secs(10))
}

#[tokio::test]
async fn resolves_and_writes_mints() {
    let repo = Arc::new(FakeRepo {
        unresolved: vec![pk(1)],
        written: Mutex::new(Vec::new()),
    });
    let source = Arc::new(FakeSource {
        resolved: vec![ResolvedPoolMints {
            pool: pk(1),
            token_a_mint: pk(2),
            token_b_mint: pk(3),
        }],
    });

    worker(repo.clone(), source).run_one_cycle().await;

    let written = repo.written.lock().unwrap();
    assert_eq!(written.as_slice(), &[(pk(1), pk(2), pk(3))]);
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
