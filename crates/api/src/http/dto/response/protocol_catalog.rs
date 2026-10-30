//! A row of `/api/protocols`: the catalogue, read live.

use serde::Serialize;
use utoipa::ToSchema;
use yog_core::Page;

use super::page::PageResponse;
use super::protocol::ProtocolResponse;
use crate::application::ProtocolEntry;
use crate::http::dto::schema::Count;
use crate::http::error::ApiError;

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

/// A page of the catalogue; `poolsSeen` counts every protocol's, whatever the page.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProtocolCatalogPageResponse {
    items: Vec<ProtocolCatalogResponse>,
    next_cursor: Option<String>,
    prev_cursor: Option<String>,
    is_first: bool,
    is_last: bool,
    #[schema(value_type = Count)]
    pools_seen: i64,
}

impl ProtocolCatalogPageResponse {
    pub(crate) fn new(page: Page<ProtocolEntry>, pools_seen: i64) -> Result<Self, ApiError> {
        let page = PageResponse::<ProtocolCatalogResponse>::from_page(page)?;
        Ok(Self {
            items: page.items,
            next_cursor: page.next_cursor,
            prev_cursor: page.prev_cursor,
            is_first: page.is_first,
            is_last: page.is_last,
            pools_seen,
        })
    }
}
