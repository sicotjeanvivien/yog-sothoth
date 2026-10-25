//! A row of `pool-creations/totals`: the pools a protocol created over the window.

use serde::Serialize;
use utoipa::ToSchema;
use yog_core::domain::ProtocolPoolCreations;

use super::protocol::ProtocolResponse;
use crate::http::dto::schema::Count;

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProtocolPoolCreationsResponse {
    protocol: ProtocolResponse,
    #[schema(value_type = Count)]
    pools: i64,
}

impl From<ProtocolPoolCreations> for ProtocolPoolCreationsResponse {
    fn from(c: ProtocolPoolCreations) -> Self {
        Self {
            protocol: c.protocol.into(),
            pools: c.pools,
        }
    }
}
