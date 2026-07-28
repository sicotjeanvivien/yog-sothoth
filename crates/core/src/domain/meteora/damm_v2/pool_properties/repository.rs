use async_trait::async_trait;
use solana_pubkey::Pubkey;

use crate::{
    RepositoryResult,
    domain::{MeteoraDammV2PoolAccountProperties, MeteoraDammV2PoolProperties},
};

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

/// Back-fill of a DAMM v2 pool's account-derived properties, performed by
/// yog-context.
///
/// These cannot be inferred from the event stream: the mints were mis-resolved
/// by a per-event heuristic, and the base fee is only emitted at pool genesis
/// (`InitializePool`) — which the indexer never sees for pools created before it
/// started watching. Reading the on-chain account back-fills both for every
/// pool, old or new.
///
/// # One resolver per protocol, on purpose
///
/// This trait is named after its protocol rather than being generic over one.
/// Its two methods are irreducibly cp-amm-specific: `list_unresolved` tests
/// columns that only exist on this protocol's satellite, and `set_pool_account`
/// takes a payload decoded at cp-amm's byte offsets. A DLMM equivalent will be a
/// sibling trait with its own queue, not a `protocol` parameter on this one.
#[async_trait]
pub trait MeteoraDammV2PoolAccountResolver: Send + Sync {
    /// DAMM v2 pools missing at least one account-derived property — a `NULL`
    /// mint, a `NULL` `fee_bps`, or a missing/incomplete satellite row — capped
    /// at `limit`, oldest first.
    ///
    /// **Implementations must filter on the protocol.** It is tempting to think
    /// this protocol's satellite table scopes the query by itself — it does
    /// not: "has no satellite row yet" is one of the conditions that makes a
    /// pool a candidate, and that condition is permanently true for every pool
    /// of every *other* protocol. Joining the satellite therefore *includes*
    /// them rather than excluding them.
    ///
    /// The consequence of getting this wrong is severe and silent. A pool this
    /// query proposes but the account source cannot decode is never resolved,
    /// so it never leaves the result set; with the ordering by `first_seen_at`
    /// ascending and a capped batch, such pools accumulate at the head of the
    /// queue and eventually starve enrichment for every pool behind them —
    /// which stops mints, then token metadata, then prices, then TVL, with no
    /// error anywhere. Covered by `tests/pool_properties.rs`.
    async fn list_unresolved(&self, limit: i64) -> RepositoryResult<Vec<Pubkey>>;

    /// Set a pool's account-derived properties, as decoded from its on-chain
    /// cp-amm account. Idempotent.
    ///
    /// Writes **two tables** from one account read — the mints and base fee onto
    /// the neutral `pools` registry, the fee-split percents onto this protocol's
    /// satellite. Implementations must do so atomically: a partial write leaves
    /// a half-enriched pool that [`list_unresolved`] will keep re-proposing
    /// every cycle.
    ///
    /// [`list_unresolved`]: Self::list_unresolved
    async fn set_pool_account(
        &self,
        pool_address: &Pubkey,
        properties: &MeteoraDammV2PoolAccountProperties,
    ) -> RepositoryResult<()>;
}
