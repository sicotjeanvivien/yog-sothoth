//! A bucket of `swaps/series`, at the root or under a protocol.

use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;
use yog_core::domain::{ProtocolBucket, SwapActivity};

use super::protocol::ProtocolResponse;
use super::swap_activity::SwapActivityResponse;

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProtocolSwapActivityBucketResponse {
    protocol: ProtocolResponse,
    from: DateTime<Utc>,
    #[serde(flatten)]
    activity: SwapActivityResponse,
}

impl From<ProtocolBucket<SwapActivity>> for ProtocolSwapActivityBucketResponse {
    fn from(b: ProtocolBucket<SwapActivity>) -> Self {
        Self {
            protocol: b.protocol.into(),
            from: b.from,
            activity: b.value.into(),
        }
    }
}
