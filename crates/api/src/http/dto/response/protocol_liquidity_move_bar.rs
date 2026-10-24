//! A bar of a protocol's `liquidity-moves/hourly` or `/daily`.

use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;
use yog_core::domain::{LiquidityMoveSums, ProtocolBar};

use super::liquidity_move_sums::LiquidityMoveSumsResponse;
use super::protocol::ProtocolResponse;

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProtocolLiquidityMoveBarResponse {
    protocol: ProtocolResponse,
    from: DateTime<Utc>,
    #[serde(flatten)]
    sums: LiquidityMoveSumsResponse,
}

impl From<ProtocolBar<LiquidityMoveSums>> for ProtocolLiquidityMoveBarResponse {
    fn from(b: ProtocolBar<LiquidityMoveSums>) -> Self {
        Self {
            protocol: b.protocol.into(),
            from: b.from,
            sums: b.value.into(),
        }
    }
}
