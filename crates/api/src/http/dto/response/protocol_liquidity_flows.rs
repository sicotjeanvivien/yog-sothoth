//! The row of a protocol's `liquidity-moves/totals`.

use serde::Serialize;
use utoipa::ToSchema;
use yog_core::domain::{LiquidityFlows, Protocol};

use super::protocol::ProtocolResponse;
use super::protocol_activity::LiquidityFlowsResponse;

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProtocolLiquidityFlowsResponse {
    protocol: ProtocolResponse,
    #[serde(flatten)]
    flows: LiquidityFlowsResponse,
}

impl ProtocolLiquidityFlowsResponse {
    pub(crate) fn new(protocol: Protocol, flows: LiquidityFlows) -> Self {
        Self {
            protocol: protocol.into(),
            flows: flows.into(),
        }
    }
}
