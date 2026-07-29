use rust_decimal::Decimal;
use solana_pubkey::Pubkey;

use crate::domain::{MeteoraDammV2PoolAccountProperties, Protocol};

/// The properties every protocol's pool account yields, in the vocabulary the
/// neutral [`crate::domain::Pool`] registry speaks.
///
/// A token pair and a base fee are not cp-amm concepts — a DLMM `LbPair` has
/// both, read at its own offsets — so they belong to the registry, not to a
/// protocol's satellite. Keeping them in their own type is what lets each table
/// have exactly one writer: this goes to
/// [`crate::domain::PoolRepository::set_account_core`], the rest to that
/// protocol's resolver.
///
/// Total, not partial: it is what a *successful* account read produces. The
/// nullability of the matching `pools` columns describes rows that have not been
/// read yet, which is a different fact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PoolAccountCore {
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
/// account read travels separately, as [`PoolAccountCore`].
///
/// [`PoolAccountResolver::set_pool_account`]: super::PoolAccountResolver::set_pool_account
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
    pub core: PoolAccountCore,
    pub properties: PoolAccountProperties,
}

impl DecodedPoolAccount {
    /// The protocol this account belongs to. Determined by the per-protocol
    /// half — the neutral half, by construction, carries no protocol.
    pub fn protocol(&self) -> Protocol {
        self.properties.protocol()
    }
}
