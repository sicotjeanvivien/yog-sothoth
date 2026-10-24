//! A row of `pool-creations/totals`: the pools created over the window.

use serde::Serialize;
use utoipa::ToSchema;
use yog_core::domain::{ByProtocol, PoolCreationCount};

use super::protocol::ProtocolResponse;
use crate::http::dto::schema::Count;

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PoolCreationsResponse {
    protocol: ProtocolResponse,
    #[schema(value_type = Count)]
    pools: i64,
}

impl From<ByProtocol<PoolCreationCount>> for PoolCreationsResponse {
    fn from(c: ByProtocol<PoolCreationCount>) -> Self {
        Self {
            protocol: c.protocol.into(),
            pools: c.value.pools,
        }
    }
}
