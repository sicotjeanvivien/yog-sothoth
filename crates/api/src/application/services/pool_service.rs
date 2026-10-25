//! The pools the catalog knows, looked up by address.

use std::sync::Arc;

use solana_pubkey::Pubkey;
use yog_core::{
    RepositoryResult,
    domain::{Pool, PoolCatalog},
};

pub(crate) struct PoolService {
    pools: Arc<dyn PoolCatalog>,
}

impl PoolService {
    pub(crate) fn new(pools: Arc<dyn PoolCatalog>) -> Self {
        Self { pools }
    }

    /// The pool at `address`, `None` when the catalog does not know it.
    pub(crate) async fn find_pool(&self, address: Pubkey) -> RepositoryResult<Option<Pool>> {
        Ok(self
            .pools
            .find_by_addresses(&[address])
            .await?
            .into_iter()
            .find(|pool| pool.pool_address == address))
    }
}
