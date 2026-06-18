use async_trait::async_trait;

use crate::{RepositoryResult, domain::GlobalAnalytics};

/// Read-only access to protocol-wide aggregate analytics.
///
/// Implementations live in `yog-persistence`. The repository never writes; it
/// rolls up the per-pool valuation tables into a single [`GlobalAnalytics`]
/// snapshot at query time.
#[async_trait]
pub trait GlobalAnalyticsRepository: Send + Sync {
    /// Compute the protocol-wide aggregate analytics: summed TVL (with its
    /// coverage numerator), 24h volume and 24h realized fees, all in USD and
    /// valued at trade-time prices. Sums only the priceable pools — see
    /// [`GlobalAnalytics`] for the partial-coverage contract.
    async fn global_analytics(&self) -> RepositoryResult<GlobalAnalytics>;
}
