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
