use serde::Serialize;
use utoipa::ToSchema;
use yog_core::Page;

use crate::http::{cursor::encode_cursor_opt, error::ApiError};

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

impl<T> PageResponse<T> {
    /// `page`, each item mapped to its response and both cursors encoded.
    pub(crate) fn from_page<D>(page: Page<D>) -> Result<Self, ApiError>
    where
        T: From<D>,
    {
        Ok(Self {
            next_cursor: encode_cursor_opt(page.next_cursor.as_ref())?,
            prev_cursor: encode_cursor_opt(page.prev_cursor.as_ref())?,
            is_first: page.is_first,
            is_last: page.is_last,
            items: page.items.into_iter().map(T::from).collect(),
        })
    }
}
