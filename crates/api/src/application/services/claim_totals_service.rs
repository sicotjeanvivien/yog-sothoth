//! The claims of positions' fees and rewards valued by yog-analytic: a protocol's totals and series
//! over a period. Their coverage starts at the first tranche their writer valued.

use std::sync::Arc;

use yog_core::{
    RepositoryResult,
    domain::{
        ClaimKind, ClaimTotals, ClaimTotalsLookup, Protocol, ProtocolActivityLookup,
        ProtocolBucket, Window,
    },
};

use super::period::{BucketPeriod, Measured, ReadPeriod, SeriesInterval, build_read_period};

pub(crate) struct ClaimTotalsService {
    hours: Arc<dyn ProtocolActivityLookup>,
    claims: Arc<dyn ClaimTotalsLookup>,
}

impl ClaimTotalsService {
    pub(crate) fn new(
        hours: Arc<dyn ProtocolActivityLookup>,
        claims: Arc<dyn ClaimTotalsLookup>,
    ) -> Self {
        Self { hours, claims }
    }

    /// The period of `protocol`'s claims over `window`, covered from the first tranche valued.
    async fn build_claims_period(
        &self,
        window: Window,
        protocol: Protocol,
    ) -> RepositoryResult<Option<ReadPeriod>> {
        let Some(period) = build_read_period(self.hours.as_ref(), window, None).await? else {
            return Ok(None);
        };
        let since = self.claims.find_measured_since(protocol).await?;
        Ok(Some(period.measured_from(since)))
    }

    /// `protocol`'s `kind` claims over `window`; no row while the claims do not cover it whole.
    pub(crate) async fn get_claim_totals(
        &self,
        kind: ClaimKind,
        window: Window,
        protocol: Protocol,
    ) -> RepositoryResult<Measured<Vec<ClaimTotals>, ReadPeriod>> {
        let period = self.build_claims_period(window, protocol).await?;
        let items = match &period {
            Some(ReadPeriod::Frozen(p)) if p.is_covered() => vec![match kind {
                ClaimKind::PositionFee => {
                    self.claims
                        .find_position_fee_claim_totals(p, protocol)
                        .await?
                }
                ClaimKind::Reward => self.claims.find_reward_claim_totals(p, protocol).await?,
            }],
            Some(ReadPeriod::Tranche(p)) if p.is_covered() => vec![
                self.claims
                    .find_totals_in_tranches(kind, p, protocol)
                    .await?,
            ],
            _ => Vec::new(),
        };
        Ok(Measured { period, items })
    }

    /// `protocol`'s `kind` claims of `window` by `interval`, from their coverage.
    pub(crate) async fn list_claim_series(
        &self,
        kind: ClaimKind,
        window: Window,
        interval: SeriesInterval,
        protocol: Protocol,
    ) -> RepositoryResult<Measured<Vec<ProtocolBucket<ClaimTotals>>, ReadPeriod>> {
        let period = self.build_claims_period(window, protocol).await?;
        let items = match (&period, interval.frozen_bucket()) {
            (Some(ReadPeriod::Tranche(p)), None) => {
                self.claims.list_tranches(kind, p, protocol).await?
            }
            (Some(ReadPeriod::Frozen(p)), Some(BucketPeriod::Hour)) => {
                self.claims.list_hourly(kind, p, protocol).await?
            }
            (Some(ReadPeriod::Frozen(p)), Some(bucket)) => {
                self.claims
                    .list_buckets(kind, p, bucket.width(), protocol)
                    .await?
            }
            _ => Vec::new(),
        };
        Ok(Measured { period, items })
    }
}

#[cfg(test)]
#[path = "tests/claim_totals_service_tests.rs"]
mod tests;
