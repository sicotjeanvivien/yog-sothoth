//! A row of `swaps/totals`, at the root or under a protocol.

use serde::Serialize;
use utoipa::ToSchema;
use yog_core::domain::ProtocolSwapActivity;

use super::protocol::ProtocolResponse;
use super::swap_activity::SwapActivityResponse;
use crate::http::dto::schema::Count;

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProtocolSwapActivityResponse {
    protocol: ProtocolResponse,
    #[serde(flatten)]
    activity: SwapActivityResponse,
    /// Distinct over the window; `null` when no cycle counted them at its last hour.
    #[schema(value_type = Option<Count>)]
    pools: Option<i64>,
}

impl From<ProtocolSwapActivity> for ProtocolSwapActivityResponse {
    fn from(t: ProtocolSwapActivity) -> Self {
        Self {
            protocol: t.protocol.into(),
            activity: t.activity.into(),
            pools: t.pools,
        }
    }
}
