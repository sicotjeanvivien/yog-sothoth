use serde::Serialize;

/// Wire shape of a paginated response.
///
/// Generic over the item type so every paginated endpoint shares the
/// same envelope. `next_cursor` is `None` (serialised as `null`) when
/// the current page is the last one.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PageResponse<T> {
    pub(crate) items: Vec<T>,
    pub(crate) next_cursor: Option<String>,
    pub prev_cursor: Option<String>,
    pub is_first: bool,
    pub is_last: bool,
}
