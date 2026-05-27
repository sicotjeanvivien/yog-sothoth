use serde::Serialize;

/// Wire shape of a paginated response.
///
/// Generic over the item type so every paginated endpoint shares the
/// same envelope. Carries enough information for the client to render
/// Previous / Next / First / Last navigation without follow-up calls:
///
/// - `nextCursor` / `prevCursor` are opaque strings used to fetch the
///   adjacent pages. Either may be `null` when the current page sits
///   at the corresponding boundary.
/// - `isFirst` / `isLast` are explicit boundary flags. They are not
///   redundant with cursor nullity: a single-page result has both
///   cursors `null` AND both flags `true`, which the client uses to
///   disable all four navigation buttons.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PageResponse<T> {
    pub(crate) items: Vec<T>,
    pub(crate) next_cursor: Option<String>,
    pub(crate) prev_cursor: Option<String>,
    pub(crate) is_first: bool,
    pub(crate) is_last: bool,
}
