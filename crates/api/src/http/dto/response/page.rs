use serde::Serialize;
use utoipa::ToSchema;

/// The envelope of every paginated response. `nextCursor`/`prevCursor` are
/// opaque, `null` at their boundary; `isFirst`/`isLast` are not redundant with
/// them — a single page has both cursors `null` and both flags `true`.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PageResponse<T> {
    pub(crate) items: Vec<T>,
    pub(crate) next_cursor: Option<String>,
    pub(crate) prev_cursor: Option<String>,
    pub(crate) is_first: bool,
    pub(crate) is_last: bool,
}
