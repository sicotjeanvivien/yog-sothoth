//! Pagination primitives shared across domains.
//!
//! Repositories return `Page<T>` objects carrying the items of the
//! current page along with cursors and boundary flags used to drive
//! Previous / Next / First / Last navigation. Cursors themselves are
//! domain-specific (each domain knows what its ordering key looks
//! like) and live next to their repository trait.

/// A page of results with bidirectional navigation hints.
///
/// `prev_cursor` is the key of the FIRST item of the current page
/// (used to navigate backward); `next_cursor` is the key of the LAST
/// item (used to navigate forward). Either may be `None` when the
/// page sits at the corresponding boundary of the list.
///
/// `is_first` / `is_last` are explicit flags rather than inferred
/// from cursor nullity: a single-page result set has both cursors
/// `None` AND both flags `true`, which the client needs to
/// distinguish from "we're on one boundary but the other end exists".
///
/// All four hints are expected to be computed by the repository in a
/// single query (typically via a "peek N+1" trick): no follow-up
/// round-trip is required to render the navigation.
#[derive(Debug, Clone)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub next_cursor: Option<Cursor>,
    pub prev_cursor: Option<Cursor>,
    pub is_first: bool,
    pub is_last: bool,
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
    Swap(crate::domain::SwapCursor),
    Liquidity(crate::domain::LiquidityCursor),
}

impl<T> Page<T> {
    /// Convenience constructor for the empty terminal page.
    ///
    /// An empty page sits at both boundaries simultaneously, hence
    /// `is_first = is_last = true` and both cursors `None`. This is
    /// the natural state when a cursor points past the end of the
    /// list, or when the underlying table is empty.
    pub fn empty() -> Self {
        Self {
            items: Vec::new(),
            next_cursor: None,
            prev_cursor: None,
            is_first: true,
            is_last: true,
        }
    }
}

/// Direction of traversal relative to a cursor.
///
/// `Next` moves further into the list (older items if the natural
/// order is `first_seen_at DESC`); `Prev` moves back toward more
/// recent items. A cursor is required for both — without one, see
/// `PagePosition` instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PageDirection {
    #[default]
    Next,
    Prev,
}

/// Absolute jump to a boundary of the list, ignoring any cursor.
///
/// Mutually exclusive with `cursor` + `PageDirection` at the request
/// level — the handler is responsible for enforcing that invariant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PagePosition {
    First,
    Last,
}

/// Column on which the pool listing can be sorted.
///
/// Restricted to materialized columns of the `pools` table — values
/// that exist at rest and can anchor a stable keyset cursor. Derived
/// metrics (TVL, 24h volume) are computed at read time and cannot be
/// sorted on until they are materialized into a dedicated analytics
/// table; they are deliberately absent here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PoolSortColumn {
    FirstSeen,
    LastSeen,
}

/// Sort order for the pool listing: a column plus a direction.
///
/// The default (`FirstSeenDesc`) preserves the historical ordering
/// (newest pools first) used before sorting was configurable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PoolSort {
    FirstSeenAsc,
    #[default]
    FirstSeenDesc,
    LastSeenAsc,
    LastSeenDesc,
}

impl PoolSort {
    /// The column this sort operates on. Used to stamp the cursor so
    /// it can be validated against the active sort on the next page.
    pub fn column(self) -> PoolSortColumn {
        match self {
            PoolSort::FirstSeenAsc | PoolSort::FirstSeenDesc => PoolSortColumn::FirstSeen,
            PoolSort::LastSeenAsc | PoolSort::LastSeenDesc => PoolSortColumn::LastSeen,
        }
    }

    /// Whether the natural (forward) direction is ascending.
    pub fn is_ascending(self) -> bool {
        matches!(self, PoolSort::FirstSeenAsc | PoolSort::LastSeenAsc)
    }
}
