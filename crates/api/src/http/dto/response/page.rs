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

/// Wire shape of `GET /api/pools` — the standard envelope plus what the pool
/// listing alone needs to say about its traversal.
///
/// Flattened rather than a field of its own so the four paginated endpoints
/// keep one envelope on the wire: a client reads `items` / `nextCursor` /
/// `isLast` the same way here as on the event feeds.
///
/// - `asOf` is the instant this traversal is anchored to. `null` when the sort
///   is over an immutable column, which needs no anchor.
/// - `touchedSince` is how many pools matching the same filters became active
///   after `asOf` — i.e. moved to the head of the list, past a reader who is
///   already deeper in it. `0` on a first page. It exists so that a listing
///   which cannot show those pools can at least say how many there are.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PoolPageResponse<T> {
    #[serde(flatten)]
    pub(crate) page: PageResponse<T>,
    pub(crate) as_of: Option<String>,
    pub(crate) touched_since: i64,
}
