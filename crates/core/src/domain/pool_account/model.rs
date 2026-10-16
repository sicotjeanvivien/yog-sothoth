use rust_decimal::Decimal;
use solana_pubkey::Pubkey;

use crate::domain::{
    MeteoraDammV2PoolAccountProperties, MeteoraDlmmPoolAccountProperties, Protocol,
};

/// The half of a pool account for the cross-protocol `pools` registry, written by
/// [`crate::domain::PoolRepository::set_registry_properties`]; named by destination.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PoolRegistryProperties {
    pub token_a_mint: Pubkey,
    pub token_b_mint: Pubkey,
    /// Base trading fee in basis points (genesis cliff for a scheduler pool).
    pub fee_bps: Decimal,
}

/// The satellite half of a pool account, one variant per protocol, stored by
/// [`PoolAccountResolver::set_pool_account`](super::PoolAccountResolver::set_pool_account).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PoolAccountProperties {
    MeteoraDammV2(MeteoraDammV2PoolAccountProperties),
    MeteoraDlmm(MeteoraDlmmPoolAccountProperties),
}

impl PoolAccountProperties {
    /// The protocol this payload belongs to.
    pub fn protocol(&self) -> Protocol {
        match self {
            Self::MeteoraDammV2(_) => Protocol::MeteoraDammV2,
            Self::MeteoraDlmm(_) => Protocol::MeteoraDlmm,
        }
    }
}

/// One decoded pool account, split by *who stores it*, so that each table has a
/// single writer. Produced by [`crate::application::decode_pool_account`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedPoolAccount {
    /// Goes to the cross-protocol `pools` registry.
    pub registry: PoolRegistryProperties,
    /// Goes to this protocol's satellite.
    pub properties: PoolAccountProperties,
}

impl DecodedPoolAccount {
    /// The protocol this account belongs to, from its satellite half.
    pub fn protocol(&self) -> Protocol {
        self.properties.protocol()
    }
}
