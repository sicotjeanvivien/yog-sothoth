//! A row of `swaps/totals`, at the root or under a protocol.

use serde::Serialize;
use utoipa::ToSchema;
use yog_core::domain::ProtocolSwapTotals;

use super::protocol::ProtocolResponse;
use super::swap_sums::SwapSumsResponse;
use crate::http::dto::schema::Count;

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProtocolSwapTotalsResponse {
    protocol: ProtocolResponse,
    #[serde(flatten)]
    sums: SwapSumsResponse,
    /// Distinct over the window; `null` when no cycle counted them at its last hour.
    #[schema(value_type = Option<Count>)]
    pools: Option<i64>,
}

impl From<ProtocolSwapTotals> for ProtocolSwapTotalsResponse {
    fn from(t: ProtocolSwapTotals) -> Self {
        Self {
            protocol: t.protocol.into(),
            sums: t.sums.into(),
            pools: t.pools,
        }
    }
}
