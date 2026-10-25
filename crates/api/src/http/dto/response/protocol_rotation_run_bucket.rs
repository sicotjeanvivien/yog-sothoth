//! A bucket of a protocol's `rotation-runs/hourly` or `/daily`.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::Serialize;
use utoipa::ToSchema;
use yog_core::domain::ProtocolBucket;

use super::protocol::ProtocolResponse;
use crate::http::dto::schema::UnsignedDecimal;

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProtocolRotationRunBucketResponse {
    protocol: ProtocolResponse,
    from: DateTime<Utc>,
    #[schema(value_type = UnsignedDecimal)]
    volume_usd: Decimal,
}

impl From<ProtocolBucket<Decimal>> for ProtocolRotationRunBucketResponse {
    fn from(b: ProtocolBucket<Decimal>) -> Self {
        Self {
            protocol: b.protocol.into(),
            from: b.from,
            volume_usd: b.value,
        }
    }
}
