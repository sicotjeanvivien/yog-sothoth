//! A bucket of a protocol's `liquidity-moves/hourly` or `/daily`.

use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;
use yog_core::domain::{LiquidityFlows, ProtocolBucket};

use super::liquidity_move::LiquidityFlowsResponse;
use super::protocol::ProtocolResponse;

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProtocolLiquidityFlowsBucketResponse {
    protocol: ProtocolResponse,
    from: DateTime<Utc>,
    #[serde(flatten)]
    flows: LiquidityFlowsResponse,
}

impl From<ProtocolBucket<LiquidityFlows>> for ProtocolLiquidityFlowsBucketResponse {
    fn from(b: ProtocolBucket<LiquidityFlows>) -> Self {
        Self {
            protocol: b.protocol.into(),
            from: b.from,
            flows: b.value.into(),
        }
    }
}
