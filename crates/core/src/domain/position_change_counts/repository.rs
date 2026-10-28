use async_trait::async_trait;

use super::PositionChangeCounts;
use crate::RepositoryResult;
use crate::domain::{FrozenPeriod, Protocol};

/// The position changes yog-analytic counted, by protocol and hour.
#[async_trait]
pub trait PositionChangeCountsLookup: Send + Sync {
    /// `protocol`'s changes over the hours of `period`; `None` when it holds no hour of it.
    async fn find_totals(
        &self,
        period: &FrozenPeriod,
        protocol: Protocol,
    ) -> RepositoryResult<Option<PositionChangeCounts>>;
}
