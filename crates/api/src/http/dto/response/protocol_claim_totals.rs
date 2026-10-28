//! The row of a protocol's `position-fee-claims/totals` and `reward-claims/totals`: the claims and
//! what they took, valued, with their coverage, `claimsValued` of `claims`.

use rust_decimal::Decimal;
use serde::Serialize;
use utoipa::ToSchema;
use yog_core::domain::{ClaimTotals, Protocol};

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
