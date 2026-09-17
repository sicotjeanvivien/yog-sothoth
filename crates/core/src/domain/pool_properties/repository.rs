use async_trait::async_trait;
use solana_pubkey::Pubkey;

use crate::RepositoryResult;
use crate::domain::{PoolProperties, Protocol};

/// Read of a pool's per-protocol properties — one implementation per protocol.
///
/// Read counterpart of [`crate::domain::PoolAccountResolver`], and generic for
/// the same reason: it carries the [`PoolProperties`] enum rather than one
/// protocol's concrete type. That is what lets a cross-protocol consumer hold
/// `Vec<Arc<dyn PoolPropertiesLookup>>` and name no protocol at all.
///
/// # Why a read trait apart from the writer's
///
/// Both traits are implemented by the same `Pg*PoolPropertiesRepository`, one
/// per protocol, and nothing in the table forces two contracts. The processes
/// do: yog-context writes the satellite and yog-api only reads it, each under
/// its own Postgres role. A single trait would hand the api
/// [`set_pool_account`], which its read-only role cannot execute, and hand
/// yog-context a [`find_by_pool`] it never calls. Split, each binary depends on
/// exactly the half it uses.
///
/// [`set_pool_account`]: crate::domain::PoolAccountResolver::set_pool_account
/// [`find_by_pool`]: Self::find_by_pool
///
/// # Why generic rather than one trait per protocol
///
/// A reader assembling a pool-detail sheet is handed a
/// [`crate::domain::Pool`] whose protocol it learns at runtime. A per-protocol
/// trait would put one protocol's name in the constructor of a cross-protocol
/// service, and add a field and a `match` arm there for every protocol added —
/// the accretion pattern migration 036 removed from the `pools` table,
/// re-formed one layer up.
///
/// The `match` does not vanish, it moves: it belongs at the serialization
/// boundary, where the wire shape is genuinely protocol-specific, not in the
/// service that only needs "this pool's properties, whatever they are".
#[async_trait]
pub trait PoolPropertiesLookup: Send + Sync {
    /// The protocol this lookup handles. Lets a consumer pair a pool with the
    /// lookup that can read it, without naming either.
    fn protocol(&self) -> Protocol;

    /// The properties of one pool, or `None` when this protocol stores none for
    /// it yet (pool discovered but neither enriched nor seen at genesis).
    ///
    /// Callers are expected to route by [`protocol`] first: an implementation
    /// asked for a pool of another protocol answers `None`, since it queries its
    /// own satellite table by address and finds nothing there.
    ///
    /// Deliberately no list/paginated variant: the cross-protocol pool listing
    /// does not surface these fields, so a bulk read would be dead code.
    ///
    /// [`protocol`]: Self::protocol
    async fn find_by_pool(&self, pool_address: &Pubkey)
    -> RepositoryResult<Option<PoolProperties>>;
}
