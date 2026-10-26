//! `Page<T>`, shared by every paginated repository; each cursor lives beside its
//! repository trait.

/// A page and its navigation: `prev_cursor` is the first item's key,
/// `next_cursor` the last's, each `None` at its boundary.
/// ⚠️ `is_first`/`is_last` are explicit: cursor nullity cannot tell a single page apart.
#[derive(Debug, Clone)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub next_cursor: Option<Cursor>,
    pub prev_cursor: Option<Cursor>,
    pub is_first: bool,
    pub is_last: bool,
}

/// A cursor, one variant per paginated domain; the wire encoding is the caller's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Cursor {
    Signal(crate::domain::SignalCursor),
    TokenActivity(crate::domain::TokenActivityCursor),
    LiquidityMove(Box<crate::domain::LiquidityMoveCursor>),
    Pool(Box<crate::domain::PoolCursor>),
    Token(crate::domain::TokenCursor),
    TokenPrice(Box<crate::domain::TokenPriceCursor>),
    SwapEvent(Box<crate::domain::EventCursor>),
    LiquidityChangeEvent(Box<crate::domain::EventCursor>),
    PoolCreationEvent(Box<crate::domain::EventCursor>),
    PositionChangeEvent(Box<crate::domain::EventCursor>),
    PositionFeeClaimEvent(Box<crate::domain::EventCursor>),
    ProtocolFeeClaimEvent(Box<crate::domain::EventCursor>),
    RewardClaimEvent(Box<crate::domain::EventCursor>),
    RewardChangeEvent(Box<crate::domain::EventCursor>),
    PoolChangeEvent(Box<crate::domain::EventCursor>),
    Announcement(Box<crate::domain::AnnouncementCursor>),
    RotationRun(Box<crate::domain::RotationRunCursor>),
    /// The last protocol of a page of the catalogue, in [`Protocol::all`](crate::domain::Protocol::all)'s order.
    Protocol(crate::domain::Protocol),
}

impl<T> Page<T> {
    /// The empty page, at both boundaries at once.
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

/// Direction of traversal from a cursor: `Next` further into the list, `Prev` back.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PageDirection {
    #[default]
    Next,
    Prev,
}

/// Jump to a boundary of the list; the handler refuses it alongside a cursor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PagePosition {
    First,
    Last,
}
