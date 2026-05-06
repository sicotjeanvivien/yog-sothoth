//! Pagination primitives shared across domains.
//!
//! Repositories return `Page<T>` objects carrying the items of the
//! current page along with an opaque cursor used to fetch the next one.
//! Cursors themselves are domain-specific (each domain knows what its
//! ordering key looks like) and live next to their repository trait.

/// A page of results plus the cursor needed to fetch the next page.
///
/// `next_cursor` is `None` when the current page is the last one — i.e.
/// the repository returned strictly fewer items than the requested limit.
/// When the page is exactly full, `next_cursor` is `Some` even if no
/// further items exist; the next call will then return an empty page
/// with `next_cursor = None`. This is intentional: detecting "no more
/// data" reliably from a full page would require an extra row probe,
/// which is not worth the cost.
#[derive(Debug, Clone)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub next_cursor: Option<Cursor>,
}

/// Discriminated cursor type.
///
/// Each domain that supports pagination defines its own variant here.
/// This keeps the cursor strongly typed across the trait boundary
/// (no opaque `Vec<u8>` blob to misinterpret) while still allowing
/// repositories to share the same `Page<T>` shape.
///
/// Serialization to/from a wire-format string (e.g. base64-encoded JSON
/// for HTTP query parameters) is the responsibility of the calling
/// layer, not of this module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Cursor {
    Pool(crate::domain::PoolCursor),
    // Future: Swap(SwapCursor), Liquidity(LiquidityCursor), …
}

impl<T> Page<T> {
    /// Convenience constructor for the empty terminal page.
    pub fn empty() -> Self {
        Self {
            items: Vec::new(),
            next_cursor: None,
        }
    }

    /// Build a page from raw items, computing whether a next cursor is
    /// warranted from the requested limit.
    ///
    /// `cursor_extractor` is called on the last item only when the page
    /// is full, deferring the cursor construction to the caller (which
    /// knows the domain-specific cursor shape).
    pub fn build<F>(items: Vec<T>, requested_limit: usize, cursor_extractor: F) -> Self
    where
        F: FnOnce(&T) -> Cursor,
    {
        let next_cursor = if items.len() >= requested_limit {
            items.last().map(cursor_extractor)
        } else {
            None
        };
        Self { items, next_cursor }
    }
}
