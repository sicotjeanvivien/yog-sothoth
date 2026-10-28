//! The life cycle of a protocol's positions over a period: how many were opened, locked, split
//! and closed, counted by yog-analytic from the position events.

/// The position changes of a period, one count per event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PositionChangeCounts {
    pub opened: i64,
    pub vesting_locks: i64,
    pub permanent_locks: i64,
    pub splits: i64,
    pub closed: i64,
}
