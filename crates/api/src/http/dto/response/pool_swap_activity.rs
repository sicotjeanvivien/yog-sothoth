//! The row of a pool's `swaps/totals`: the swap fields, then its own protocol's under its name.

use serde::Serialize;
use utoipa::ToSchema;
use yog_core::domain::SwapActivity;

use super::swap_activity::{SwapActivityMeteoraDammV2Response, SwapActivityResponse};

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PoolSwapActivityResponse {
    #[serde(flatten)]
    activity: SwapActivityResponse,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(nullable = false)]
    meteora_damm_v2: Option<SwapActivityMeteoraDammV2Response>,
}

impl From<SwapActivity> for PoolSwapActivityResponse {
    fn from(mut activity: SwapActivity) -> Self {
        Self {
            meteora_damm_v2: activity.meteora_damm_v2.take().map(Into::into),
            activity: activity.into(),
        }
    }
}

#[cfg(test)]
#[path = "tests/pool_swap_activity_tests.rs"]
mod tests;
