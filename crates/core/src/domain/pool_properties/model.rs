use crate::domain::{MeteoraDammV2PoolProperties, MeteoraDlmmPoolProperties, Protocol};

/// A pool's stored per-protocol properties, grouped by protocol.
///
/// Two-level, exactly like [`crate::domain::PoolAccountProperties`] and
/// [`crate::domain::DomainEvent`]: the outer variant is the protocol, the inner
/// type is that protocol's own satellite row. Satellites share no column, so a
/// flat struct would mean `Option` fields permanently `None` for every protocol
/// but one — the shape "voie 3" rejects.
///
/// # Read counterpart of [`PoolAccountProperties`]
///
/// [`PoolAccountProperties`] is what one *account read* yields, on its way into
/// the database. This is what the database *holds*, on its way out to a reader.
/// They are deliberately distinct types even where the fields overlap: the
/// account payload is total (a successful read fills every field), a satellite
/// row is partial (two independent writers fill it at different times), and the
/// account payload also carries the neutral `pools` columns, which a reader gets
/// from [`crate::domain::Pool`] instead.
///
/// Produced by [`PoolPropertiesLookup::find_by_pool`], consumed by the API's
/// pool-detail response, which matches on it to pick the wire shape.
///
/// [`PoolAccountProperties`]: crate::domain::PoolAccountProperties
/// [`PoolPropertiesLookup::find_by_pool`]: super::PoolPropertiesLookup::find_by_pool
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PoolProperties {
    MeteoraDammV2(MeteoraDammV2PoolProperties),
    MeteoraDlmm(MeteoraDlmmPoolProperties),
}

impl PoolProperties {
    /// The protocol these properties belong to. Determined by the variant.
    pub fn protocol(&self) -> Protocol {
        match self {
            Self::MeteoraDammV2(_) => Protocol::MeteoraDammV2,
            Self::MeteoraDlmm(_) => Protocol::MeteoraDlmm,
        }
    }
}
