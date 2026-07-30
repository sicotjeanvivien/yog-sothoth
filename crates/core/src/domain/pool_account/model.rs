use rust_decimal::Decimal;
use solana_pubkey::Pubkey;

use crate::domain::{
    MeteoraDammV2PoolAccountProperties, MeteoraDlmmPoolAccountProperties, Protocol,
};

/// The half of a pool account that belongs to the cross-protocol `pools`
/// registry.
///
/// **Named by its destination, not by its content.** A token pair and a base fee
/// are not cp-amm concepts — a DLMM `LbPair` has both, read at its own offsets —
/// so they land on the registry rather than on any protocol's satellite. Giving
/// them their own type is what lets each table have exactly one writer: this goes
/// to [`crate::domain::PoolRepository::set_registry_properties`], the rest to
/// that protocol's resolver.
///
/// Reads as the counterpart of [`PoolAccountProperties`]: registry properties vs
/// satellite properties, both from the same account.
///
/// Total, not partial: it is what a *successful* account read produces. The
/// nullability of the matching `pools` columns describes rows that have not been
/// read yet, which is a different fact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PoolRegistryProperties {
    pub token_a_mint: Pubkey,
    pub token_b_mint: Pubkey,
    /// Base trading fee in basis points (genesis cliff for a scheduler pool).
    pub fee_bps: Decimal,
}

/// The properties that only exist for one protocol, grouped by protocol.
///
/// Two-level, exactly like [`crate::domain::DomainEvent`]: the outer variant is
/// the protocol, the inner type is that protocol's own property set. Satellite
/// layouts have nothing in common across protocols, so a flat struct would mean
/// `Option` fields permanently `None` for every protocol but one — the shape
/// "voie 3" rejects.
///
/// Consumed by [`PoolAccountResolver::set_pool_account`], which matches on it to
/// pick the table that knows how to store it. The neutral half of the same
/// account read travels separately, as [`PoolRegistryProperties`].
///
/// [`PoolAccountResolver::set_pool_account`]: super::PoolAccountResolver::set_pool_account
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PoolAccountProperties {
    MeteoraDammV2(MeteoraDammV2PoolAccountProperties),
    MeteoraDlmm(MeteoraDlmmPoolAccountProperties),
}

impl PoolAccountProperties {
    /// The protocol this payload belongs to. Determined by the variant.
    pub fn protocol(&self) -> Protocol {
        match self {
            Self::MeteoraDammV2(_) => Protocol::MeteoraDammV2,
            Self::MeteoraDlmm(_) => Protocol::MeteoraDlmm,
        }
    }
}

/// One decoded pool account, split by *who stores it* rather than by where it
/// came from.
///
/// Both halves come from the same read of the same bytes, and the caller writes
/// them through two repositories — the neutral registry and the protocol's
/// satellite — so that neither table has more than one writer. Before this
/// split a single repository wrote both tables in one transaction, which made
/// the cp-amm satellite the owner of the cross-protocol `pools` registry.
///
/// Produced by [`crate::application::decode_pool_account`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedPoolAccount {
    /// Goes to the cross-protocol `pools` registry.
    pub registry: PoolRegistryProperties,
    /// Goes to this protocol's satellite.
    pub properties: PoolAccountProperties,
}

impl DecodedPoolAccount {
    /// The protocol this account belongs to. Determined by the per-protocol
    /// half — the neutral half, by construction, carries no protocol.
    pub fn protocol(&self) -> Protocol {
        self.properties.protocol()
    }
}
