//! A bucket of `pool-creations/series`.

use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;
use yog_core::domain::{PoolCreations, ProtocolBucket};

use super::protocol::ProtocolResponse;
use crate::http::dto::schema::Count;

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProtocolPoolCreationsBucketResponse {
    protocol: ProtocolResponse,
    from: DateTime<Utc>,
    #[schema(value_type = Count)]
    pools: i64,
}

impl From<ProtocolBucket<PoolCreations>> for ProtocolPoolCreationsBucketResponse {
    fn from(b: ProtocolBucket<PoolCreations>) -> Self {
        Self {
            protocol: b.protocol.into(),
            from: b.from,
            pools: b.value.pools,
        }
    }
}
