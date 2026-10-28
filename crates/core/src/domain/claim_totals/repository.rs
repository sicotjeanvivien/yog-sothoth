use async_trait::async_trait;
use chrono::{DateTime, Utc};

use super::ClaimTotals;
use crate::RepositoryResult;
use crate::domain::{FrozenPeriod, Protocol};

/// The valued claims. ⚠️ Measured from a later tranche than the swaps: every read takes its
/// period from [`Self::find_measured_since`], not from the facts.
#[async_trait]
pub trait ClaimTotalsLookup: Send + Sync {
    /// The first tranche whose claims `protocol`'s writer valued; `None` before it did.
    async fn find_measured_since(
        &self,
        protocol: Protocol,
    ) -> RepositoryResult<Option<DateTime<Utc>>>;

    async fn find_position_fee_claim_totals(
        &self,
        period: &FrozenPeriod,
        protocol: Protocol,
    ) -> RepositoryResult<ClaimTotals>;

    async fn find_reward_claim_totals(
        &self,
        period: &FrozenPeriod,
        protocol: Protocol,
    ) -> RepositoryResult<ClaimTotals>;
}
