//! The pools a protocol created, counted when the indexer saw their birth.

use crate::domain::Protocol;

/// The pools created over a period or a bucket.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PoolCreations {
    pub pools: i64,
}

/// One protocol's pools created over a period.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProtocolPoolCreations {
    pub protocol: Protocol,
    pub pools: i64,
}

impl ProtocolPoolCreations {
    /// The pools of every row; `None` without rows.
    pub fn total(rows: &[Self]) -> Option<i64> {
        (!rows.is_empty()).then(|| rows.iter().map(|row| row.pools).sum())
    }
}
