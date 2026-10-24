//! A bar of `pool-creations/hourly` or `/daily`.

use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;
use yog_core::domain::{PoolCreationCount, ProtocolBar};

use super::protocol::ProtocolResponse;
use crate::http::dto::schema::Count;

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PoolCreationBarResponse {
    protocol: ProtocolResponse,
    from: DateTime<Utc>,
    #[schema(value_type = Count)]
    pools: i64,
}

impl From<ProtocolBar<PoolCreationCount>> for PoolCreationBarResponse {
    fn from(b: ProtocolBar<PoolCreationCount>) -> Self {
        Self {
            protocol: b.protocol.into(),
            from: b.from,
            pools: b.value.pools,
        }
    }
}
