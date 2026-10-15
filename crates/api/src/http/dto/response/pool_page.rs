use serde::Serialize;
use utoipa::ToSchema;

use super::page::PageResponse;
use crate::http::dto::schema::Count;

/// The page envelope, flattened, plus `asOf` — the instant the traversal is
/// anchored to, `null` on an immutable sort — and `touchedSince`, an upper bound
/// on the pools active since `asOf` (see `crates/api/README.md`).
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PoolPageResponse<T> {
    #[serde(flatten)]
    pub(crate) page: PageResponse<T>,
    #[schema(value_type = Option<String>, format = DateTime)]
    pub(crate) as_of: Option<String>,
    #[schema(value_type = Count)]
    pub(crate) touched_since: i64,
}
