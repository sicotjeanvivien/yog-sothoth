//! A bucket of a pool's `swaps/series`.

use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;
use yog_core::domain::PoolSwapBucket;

use super::swap_activity::SwapActivityResponse;

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PoolSwapBucketResponse {
    from: DateTime<Utc>,
    #[serde(flatten)]
    activity: SwapActivityResponse,
}

impl From<PoolSwapBucket> for PoolSwapBucketResponse {
    fn from(b: PoolSwapBucket) -> Self {
        Self {
            from: b.from,
            activity: b.activity.into(),
        }
    }
}
