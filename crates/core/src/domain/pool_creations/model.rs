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
