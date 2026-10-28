//! The claims of positions' fees and rewards over a frozen period, valued by yog-analytic: a
//! protocol's totals. Their coverage starts at the first whole hour their writer valued.

use std::sync::Arc;

use yog_core::{
    RepositoryResult,
    domain::{
        ClaimTotals, ClaimTotalsLookup, FrozenPeriod, Protocol, ProtocolActivityLookup, Window,
    },
};

use super::period::{Measured, build_frozen_period, first_whole_hour, measured_from};

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

    /// The frozen period of `protocol`'s claims, covered from the first whole hour valued.
    async fn build_claims_period(
        &self,
        window: Window,
        protocol: Protocol,
    ) -> RepositoryResult<Option<FrozenPeriod>> {
        let Some(period) = build_frozen_period(self.hours.as_ref(), window, None).await? else {
            return Ok(None);
        };
        let since = self
            .claims
            .find_measured_since(protocol)
            .await?
            .map(first_whole_hour);
        Ok(Some(measured_from(period, since)))
    }

    /// `protocol`'s fees claimed over `window`; no row while the claims do not cover it whole.
    pub(crate) async fn get_position_fee_claim_totals(
        &self,
        window: Window,
        protocol: Protocol,
    ) -> RepositoryResult<Measured<Vec<ClaimTotals>>> {
        let period = self.build_claims_period(window, protocol).await?;
        let items = match covered(period.as_ref()) {
            Some(period) => vec![
                self.claims
                    .find_position_fee_claim_totals(period, protocol)
                    .await?,
            ],
            None => Vec::new(),
        };
        Ok(Measured { period, items })
    }

    /// `protocol`'s rewards claimed over `window`; no row while the claims do not cover it whole.
    pub(crate) async fn get_reward_claim_totals(
        &self,
        window: Window,
        protocol: Protocol,
    ) -> RepositoryResult<Measured<Vec<ClaimTotals>>> {
        let period = self.build_claims_period(window, protocol).await?;
        let items = match covered(period.as_ref()) {
            Some(period) => vec![
                self.claims
                    .find_reward_claim_totals(period, protocol)
                    .await?,
            ],
            None => Vec::new(),
        };
        Ok(Measured { period, items })
    }
}

/// The period, if the claims cover it whole.
fn covered(period: Option<&FrozenPeriod>) -> Option<&FrozenPeriod> {
    period.filter(|period| period.is_covered())
}

#[cfg(test)]
#[path = "tests/claim_totals_service_tests.rs"]
mod tests;
