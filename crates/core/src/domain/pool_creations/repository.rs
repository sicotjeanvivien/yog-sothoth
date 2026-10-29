use async_trait::async_trait;

use super::{PoolCreations, ProtocolPoolCreations};
use crate::RepositoryResult;
use crate::domain::{FrozenPeriod, Protocol, ProtocolBucket, TranchePeriod};

/// The pools created, by protocol, under the same period rules as
/// [`SwapActivityLookup`](crate::domain::SwapActivityLookup).
#[async_trait]
pub trait PoolCreationsLookup: Send + Sync {
    async fn list_totals(
        &self,
        period: &FrozenPeriod,
        protocol: Option<Protocol>,
    ) -> RepositoryResult<Vec<ProtocolPoolCreations>>;

    async fn list_hourly(
        &self,
        period: &FrozenPeriod,
        protocol: Option<Protocol>,
    ) -> RepositoryResult<Vec<ProtocolBucket<PoolCreations>>>;

    async fn list_daily(
        &self,
        period: &FrozenPeriod,
        protocol: Option<Protocol>,
    ) -> RepositoryResult<Vec<ProtocolBucket<PoolCreations>>>;

    /// The pools created in `period`, counted from the creations themselves.
    async fn list_totals_in_tranches(
        &self,
        period: &TranchePeriod,
        protocol: Option<Protocol>,
    ) -> RepositoryResult<Vec<ProtocolPoolCreations>>;

    /// The tranches of `period`, oldest first, 0 for one without a creation.
    async fn list_tranches(
        &self,
        period: &TranchePeriod,
        protocol: Option<Protocol>,
    ) -> RepositoryResult<Vec<ProtocolBucket<PoolCreations>>>;
}
