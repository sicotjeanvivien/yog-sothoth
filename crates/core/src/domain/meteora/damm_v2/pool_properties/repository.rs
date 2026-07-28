use async_trait::async_trait;
use solana_pubkey::Pubkey;

use crate::{RepositoryResult, domain::MeteoraDammV2PoolProperties};

/// Contract for the DAMM v2 pool-properties satellite (migration 036).
///
/// Split by writer, because the two column groups have different origins:
///
/// - [`set_fee_config`] is written by the **indexer**, decoding the genesis
///   `InitializePool` fee blob;
/// - the fee-split percents are written by **yog-context** as part of
///   [`crate::domain::PoolAccountResolver::set_pool_account`], which resolves the
///   on-chain account and fills the neutral `pools` columns and this satellite
///   from the same read. They are therefore absent from this trait.
///
/// [`set_fee_config`]: Self::set_fee_config
#[async_trait]
pub trait MeteoraDammV2PoolPropertiesRepository: Send + Sync {
    /// Record a pool's decoded fee *shape*, creating the satellite row if this
    /// is the first property written for that pool.
    ///
    /// An upsert rather than an `UPDATE`: unlike the columns this replaces, the
    /// satellite row is not created alongside the pool, so an `UPDATE` would
    /// silently do nothing on first write. The pool itself must already exist —
    /// the row is `REFERENCES pools (pool_address)`, so a write for an unknown
    /// pool is a [`crate::RepositoryError`] rather than a no-op.
    async fn set_fee_config(
        &self,
        pool_address: &Pubkey,
        base_fee_kind: &str,
        has_dynamic_fee: bool,
    ) -> RepositoryResult<()>;

    /// Record whether a volatility dynamic fee is enabled, **without touching
    /// `base_fee_kind`**.
    ///
    /// Separate from [`set_fee_config`] because an `UpdatePoolFees` event can
    /// toggle the dynamic fee but carries no base-fee *mode* — the program only
    /// lets an operator change the cliff numerator, and only while the base fee
    /// is static. Reusing `set_fee_config` here would mean re-reading
    /// `base_fee_kind` just to write it back, racily and for nothing.
    ///
    /// Column-level upsert, same shape as
    /// [`crate::domain::PoolRepository::set_fee_bps`]. Creates the satellite row
    /// if the genesis event was never seen.
    ///
    /// [`set_fee_config`]: Self::set_fee_config
    async fn set_has_dynamic_fee(
        &self,
        pool_address: &Pubkey,
        has_dynamic_fee: bool,
    ) -> RepositoryResult<()>;

    /// The properties of one pool, or `None` when no satellite row exists yet
    /// (pool discovered but neither enriched nor seen at genesis).
    ///
    /// Read side of the pool detail sheet. Deliberately no list/paginated
    /// variant: the cross-protocol pool listing does not surface these fields,
    /// so a bulk read would be dead code.
    async fn find_by_pool(
        &self,
        pool_address: &Pubkey,
    ) -> RepositoryResult<Option<MeteoraDammV2PoolProperties>>;
}
