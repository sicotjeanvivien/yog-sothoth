//! A bar of a pool's `swaps/hourly` or `/daily`.

use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;
use yog_core::domain::PoolSwapBar;

use super::swap_sums::SwapSumsResponse;

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PoolSwapBarResponse {
    from: DateTime<Utc>,
    #[serde(flatten)]
    sums: SwapSumsResponse,
}

impl From<PoolSwapBar> for PoolSwapBarResponse {
    fn from(b: PoolSwapBar) -> Self {
        Self {
            from: b.from,
            sums: b.sums.into(),
        }
    }
}
