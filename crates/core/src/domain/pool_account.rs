use async_trait::async_trait;
use solana_pubkey::Pubkey;

use crate::RepositoryResult;
use crate::domain::{MeteoraDammV2PoolAccountProperties, Protocol};

/// Everything one read of an on-chain pool account can yield, grouped by
/// protocol.
///
/// Two-level, exactly like [`crate::domain::DomainEvent`]: the outer variant is
/// the protocol, the inner type is that protocol's own property set. Account
/// layouts have nothing in common across protocols, so a flat struct would mean
/// `Option` fields permanently `None` for every protocol but one — the shape
/// "voie 3" rejects.
///
/// Produced by [`crate::application::decode_pool_account`], consumed by
/// [`PoolAccountResolver::set_pool_account`], which matches on it to pick the
/// table that knows how to store it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PoolAccountProperties {
    MeteoraDammV2(MeteoraDammV2PoolAccountProperties),
}

impl PoolAccountProperties {
    /// The protocol this payload belongs to. Determined by the variant.
    pub fn protocol(&self) -> Protocol {
        match self {
            Self::MeteoraDammV2(_) => Protocol::MeteoraDammV2,
        }
    }
}

/// Back-fill of a pool's account-derived properties, performed by yog-context —
/// one implementation per protocol.
///
/// These properties cannot be inferred from the event stream: the mints were
/// mis-resolved by a per-event heuristic, and the base fee is only emitted at
/// pool genesis, which the indexer never sees for a pool created before it
/// started watching. Reading the on-chain account back-fills both for every
/// pool, old or new.
///
/// # Generic trait, per-protocol implementations
///
/// The trait is protocol-agnostic because its payload is: it carries the
/// [`PoolAccountProperties`] enum rather than one protocol's concrete type. That
/// is what lets the enrichment worker hold `Vec<Arc<dyn PoolAccountResolver>>`
/// and know nothing about any protocol — the same shape as the indexer's
/// `EventPersistor` over `DomainEvent`.
///
/// **Each implementation still owns its own queue.** [`list_unresolved`] is not
/// parameterised by protocol: it is scoped by the implementation, which knows
/// its own satellite table and filters on its own protocol. Genericity lives at
/// the trait level; scoping lives in the impl.
///
/// [`list_unresolved`]: Self::list_unresolved
#[async_trait]
pub trait PoolAccountResolver: Send + Sync {
    /// The protocol this resolver handles. Lets the worker pair a batch of
    /// pools with the resolver that can store them, and label its metrics.
    fn protocol(&self) -> Protocol;

    /// Pools of *this* protocol missing at least one account-derived property,
    /// capped at `limit`, oldest first.
    ///
    /// **Implementations must filter on the protocol.** It is tempting to think
    /// a per-protocol satellite table scopes the query by itself — it does not:
    /// "has no satellite row yet" is one of the conditions that makes a pool a
    /// candidate, and that condition is permanently true for every pool of every
    /// *other* protocol. Joining the satellite therefore *includes* them rather
    /// than excluding them.
    ///
    /// The consequence of getting this wrong is severe and silent. A pool this
    /// query proposes but the decoder cannot decode is never resolved, so it
    /// never leaves the result set; with the ordering by `first_seen_at`
    /// ascending and a capped batch, such pools accumulate at the head of the
    /// queue and eventually starve enrichment for every pool behind them —
    /// which stops mints, then token metadata, then prices, then TVL, with no
    /// error anywhere. Covered by `persistence/tests/pool_properties.rs`.
    async fn list_unresolved(&self, limit: i64) -> RepositoryResult<Vec<Pubkey>>;

    /// Store a pool's decoded account properties. Idempotent.
    ///
    /// An implementation receives the enum and is entitled to reject a variant
    /// that is not its own — but the worker never mismatches them, since it
    /// routes each decoded payload by [`PoolAccountProperties::protocol`].
    ///
    /// May write **more than one table** from this single value (the neutral
    /// `pools` registry and the protocol's satellite). Implementations that do
    /// must be atomic: a partial write leaves a half-enriched pool that
    /// [`list_unresolved`] will keep re-proposing every cycle.
    ///
    /// [`list_unresolved`]: Self::list_unresolved
    async fn set_pool_account(
        &self,
        pool_address: &Pubkey,
        properties: &PoolAccountProperties,
    ) -> RepositoryResult<()>;
}
