//! The row of a protocol's `position-fee-claims` and `reward-claims` measures, `totals` and
//! `series`: the claims and what they took, valued, with their coverage, `claimsValued` of `claims`.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::Serialize;
use utoipa::ToSchema;
use yog_core::domain::{ClaimTotals, Protocol, ProtocolBucket};

use super::protocol::ProtocolResponse;
use crate::http::dto::schema::{Count, UnsignedDecimal};

/// The sum skips the claims not valued; `owners` is distinct over the window.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProtocolClaimTotalsResponse {
    protocol: ProtocolResponse,
    #[schema(value_type = Count)]
    claims: i64,
    #[schema(value_type = Count)]
    claims_valued: i64,
    #[schema(value_type = UnsignedDecimal)]
    amount_usd: Decimal,
    #[schema(value_type = Count)]
    owners: i64,
}

impl ProtocolClaimTotalsResponse {
    pub(crate) fn new(protocol: Protocol, totals: ClaimTotals) -> Self {
        Self {
            protocol: protocol.into(),
            claims: totals.claims,
            claims_valued: totals.claims_valued,
            amount_usd: totals.amount_usd,
            owners: totals.owners,
        }
    }
}

/// A bucket of the series; `owners` is distinct over the bucket.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProtocolClaimBucketResponse {
    from: DateTime<Utc>,
    #[serde(flatten)]
    totals: ProtocolClaimTotalsResponse,
}

impl From<ProtocolBucket<ClaimTotals>> for ProtocolClaimBucketResponse {
    fn from(b: ProtocolBucket<ClaimTotals>) -> Self {
        Self {
            from: b.from,
            totals: ProtocolClaimTotalsResponse::new(b.protocol, b.value),
        }
    }
}

#[cfg(test)]
#[path = "tests/protocol_claim_totals_tests.rs"]
mod tests;
