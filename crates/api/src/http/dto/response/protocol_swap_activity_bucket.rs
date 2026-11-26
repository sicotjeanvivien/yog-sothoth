//! A bucket of `swaps/series`, at the root or under a protocol.

use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;
use yog_core::domain::{ProtocolBucket, SwapActivity};

use super::protocol::ProtocolResponse;
use super::swap_activity::{
    SwapActivityMeteoraDammV2Response, SwapActivityResponse, meteora_damm_v2_of,
};

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProtocolSwapActivityBucketResponse {
    protocol: ProtocolResponse,
    from: DateTime<Utc>,
    #[serde(flatten)]
    activity: SwapActivityResponse,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(nullable = false)]
    meteora_damm_v2: Option<SwapActivityMeteoraDammV2Response>,
}

impl From<ProtocolBucket<SwapActivity>> for ProtocolSwapActivityBucketResponse {
    fn from(mut b: ProtocolBucket<SwapActivity>) -> Self {
        Self {
            protocol: b.protocol.into(),
            from: b.from,
            meteora_damm_v2: meteora_damm_v2_of(b.value.protocol_part.take()),
            activity: b.value.into(),
        }
    }
}
