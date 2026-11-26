//! A bucket of a pool's `swaps/series`.

use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;
use yog_core::domain::PoolSwapBucket;

use super::swap_activity::{
    SwapActivityMeteoraDammV2Response, SwapActivityResponse, meteora_damm_v2_of,
};

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PoolSwapBucketResponse {
    from: DateTime<Utc>,
    #[serde(flatten)]
    activity: SwapActivityResponse,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(nullable = false)]
    meteora_damm_v2: Option<SwapActivityMeteoraDammV2Response>,
}

impl From<PoolSwapBucket> for PoolSwapBucketResponse {
    fn from(mut b: PoolSwapBucket) -> Self {
        Self {
            from: b.from,
            meteora_damm_v2: meteora_damm_v2_of(b.activity.protocol_part.take()),
            activity: b.activity.into(),
        }
    }
}

#[cfg(test)]
#[path = "tests/pool_swap_bucket_tests.rs"]
mod tests;
