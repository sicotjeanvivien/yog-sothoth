//! A bar of `swaps/hourly` or `/daily`, at the root or under a protocol.

use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;
use yog_core::domain::{ProtocolBar, SwapSums};

use super::protocol::ProtocolResponse;
use super::swap_sums::SwapSumsResponse;

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProtocolSwapBarResponse {
    protocol: ProtocolResponse,
    from: DateTime<Utc>,
    #[serde(flatten)]
    sums: SwapSumsResponse,
}

impl From<ProtocolBar<SwapSums>> for ProtocolSwapBarResponse {
    fn from(b: ProtocolBar<SwapSums>) -> Self {
        Self {
            protocol: b.protocol.into(),
            from: b.from,
            sums: b.value.into(),
        }
    }
}
