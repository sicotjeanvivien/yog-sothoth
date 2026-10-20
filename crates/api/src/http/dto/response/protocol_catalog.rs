//! A row of `/api/protocols`: the catalogue, read live.

use serde::Serialize;
use utoipa::ToSchema;

use super::protocol::ProtocolResponse;
use crate::application::ProtocolEntry;
use crate::http::dto::schema::Count;

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProtocolCatalogResponse {
    protocol: ProtocolResponse,
    /// Whether this build ingests the protocol.
    observed: bool,
    #[schema(value_type = Count)]
    pools_seen: i64,
}

impl From<ProtocolEntry> for ProtocolCatalogResponse {
    fn from(entry: ProtocolEntry) -> Self {
        Self {
            protocol: entry.protocol.into(),
            observed: entry.observed,
            pools_seen: entry.pools_seen,
        }
    }
}
