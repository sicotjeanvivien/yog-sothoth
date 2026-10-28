//! The claims of positions' fees and rewards over a frozen period, valued by yog-analytic: a
//! protocol's totals. Their coverage starts at the first whole hour their writer valued.

use std::sync::Arc;

use yog_core::{
    RepositoryResult,
    domain::{
        ClaimTotals, ClaimTotalsLookup, FrozenPeriod, Protocol, ProtocolActivityLookup, Window,
    },
};

use super::period::{Measured, build_frozen_period, first_whole_hour};

/// Which claims a total sums.
#[derive(Debug, Clone, Copy)]
pub(crate) enum ClaimKind {
    PositionFees,
    Rewards,
}

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
        // ⚠️ Not measured yet: covered from `as_of` on, so no period reads as covered.
        let covered_since = since.map_or(period.as_of, |since| since.max(period.covered_since));
        Ok(Some(FrozenPeriod {
            covered_since,
            ..period
        }))
    }

    /// `protocol`'s claims of `kind` over `window`; no row while they do not cover it whole.
    pub(crate) async fn get_claim_totals(
        &self,
        window: Window,
        protocol: Protocol,
        kind: ClaimKind,
    ) -> RepositoryResult<Measured<Vec<ClaimTotals>>> {
        let period = self.build_claims_period(window, protocol).await?;
        let items = match &period {
            Some(period) if period.is_covered() => vec![match kind {
                ClaimKind::PositionFees => {
                    self.claims
                        .find_position_fee_claim_totals(period, protocol)
                        .await?
                }
                ClaimKind::Rewards => {
                    self.claims
                        .find_reward_claim_totals(period, protocol)
                        .await?
                }
            }],
            _ => Vec::new(),
        };
        Ok(Measured { period, items })
    }
}

#[cfg(test)]
#[path = "tests/claim_totals_service_tests.rs"]
mod tests;
