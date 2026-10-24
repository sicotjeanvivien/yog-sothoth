//! The row of a protocol's `liquidity-moves/totals`.

use serde::Serialize;
use utoipa::ToSchema;
use yog_core::domain::{LiquidityMoveSums, Protocol};

use super::liquidity_move_sums::LiquidityMoveSumsResponse;
use super::protocol::ProtocolResponse;

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProtocolLiquidityMoveTotalsResponse {
    protocol: ProtocolResponse,
    #[serde(flatten)]
    sums: LiquidityMoveSumsResponse,
}

impl ProtocolLiquidityMoveTotalsResponse {
    pub(crate) fn new(protocol: Protocol, sums: LiquidityMoveSums) -> Self {
        Self {
            protocol: protocol.into(),
            sums: sums.into(),
        }
    }
}
