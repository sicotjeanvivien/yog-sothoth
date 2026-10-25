use async_trait::async_trait;
use solana_pubkey::Pubkey;

use super::MeteoraDammV2PoolProperties;
use crate::RepositoryResult;

/// The DAMM v2 satellite of a pool, the api's lens.
#[async_trait]
pub trait MeteoraDammV2PoolPropertiesLookup: Send + Sync {
    /// `None` before yog-context first resolved the pool's account.
    async fn find_by_pool(
        &self,
        pool_address: &Pubkey,
    ) -> RepositoryResult<Option<MeteoraDammV2PoolProperties>>;
}
