use serde::Serialize;
use utoipa::ToSchema;

use super::page::PageResponse;
use crate::http::dto::schema::Count;

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
///   after `asOf`, and so moved to the **end of the ordering** — the head of
///   the list under `last_seen_desc`, its tail under `last_seen_asc`. It exists
///   so that a listing which cannot show those pools can at least say how many
///   there are; a client that reports it should send the reader to the end the
///   sort actually points at, not to the first page in both cases.
///
/// `touchedSince` is `0` whenever the anchor was minted by that same call — a
/// first page, or a `position` jump — but **not** on a backward page that lands
/// back on the first page: it carries a cursor, so it carries a real count. It
/// is also an upper bound on what the reader missed rather than a count of it;
/// `crates/api/README.md` states both caveats in full.
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
