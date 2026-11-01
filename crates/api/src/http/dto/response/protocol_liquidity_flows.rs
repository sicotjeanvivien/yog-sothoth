//! The row of a protocol's `liquidity-moves/totals`: its liquidity, and the distinct owners of
//! its moves over the window.

use serde::Serialize;
use utoipa::ToSchema;
use yog_core::domain::{LiquidityTotals, Protocol};

use super::liquidity_move::LiquidityFlowsResponse;
use super::protocol::ProtocolResponse;
use crate::http::dto::schema::Count;

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProtocolLiquidityFlowsResponse {
    protocol: ProtocolResponse,
    #[serde(flatten)]
    flows: LiquidityFlowsResponse,
    /// The addresses that created a pool, added or removed liquidity over the window, once each.
    #[schema(value_type = Count)]
    owners: i64,
}

impl ProtocolLiquidityFlowsResponse {
    pub(crate) fn new(protocol: Protocol, totals: LiquidityTotals) -> Self {
        Self {
            protocol: protocol.into(),
            flows: totals.flows.into(),
            owners: totals.owners,
        }
    }
}

#[cfg(test)]
#[path = "tests/protocol_liquidity_flows_tests.rs"]
mod tests;
