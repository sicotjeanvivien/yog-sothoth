use async_trait::async_trait;
use solana_pubkey::Pubkey;

use crate::RepositoryResult;
use crate::domain::{PoolAccountProperties, Protocol};

/// Back-fill of a pool's account-derived properties by yog-context, one
/// implementation per protocol, each scoping its own queue.
#[async_trait]
pub trait PoolAccountResolver: Send + Sync {
    /// The protocol this resolver handles.
    fn protocol(&self) -> Protocol;

    /// Pools of *this* protocol missing an account-derived property, oldest first.
    /// ⚠️ Filter on the protocol: "no satellite row" holds for every other protocol,
    /// and an undecodable pool would sit at the head of the queue forever.
    async fn list_unresolved(&self, limit: i64) -> RepositoryResult<Vec<Pubkey>>;

    /// Store a pool's decoded account properties. Idempotent.
    async fn set_pool_account(
        &self,
        pool_address: &Pubkey,
        properties: &PoolAccountProperties,
    ) -> RepositoryResult<()>;
}
