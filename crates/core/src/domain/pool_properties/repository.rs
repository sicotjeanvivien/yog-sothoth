use async_trait::async_trait;
use solana_pubkey::Pubkey;

use crate::RepositoryResult;
use crate::domain::{PoolProperties, Protocol};

/// Read of a pool's per-protocol properties, one implementation per protocol;
/// generic so a cross-protocol service names no protocol.
#[async_trait]
pub trait PoolPropertiesLookup: Send + Sync {
    /// The protocol this lookup handles.
    fn protocol(&self) -> Protocol;

    /// The properties of one pool, `None` when not stored yet.
    /// ⚠️ Route by [`protocol`](Self::protocol) first: another protocol's pool reads as `None`.
    async fn find_by_pool(&self, pool_address: &Pubkey)
    -> RepositoryResult<Option<PoolProperties>>;
}
