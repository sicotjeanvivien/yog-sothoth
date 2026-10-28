//! The TVL, a reading at an instant: each protocol's reading of an hour, never a sum.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;

use crate::domain::Protocol;

/// One protocol's TVL as last measured during the hour that closes at `at`: its pools
/// valued by their priced sides; `pools_unvalued` counts those with no side valued.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtocolTvl {
    pub protocol: Protocol,
    pub at: DateTime<Utc>,
    pub tvl_usd: Option<Decimal>,
    pub pools_priced: i64,
    pub pools_unvalued: i64,
}

/// A TVL reading: the protocol's TVL as measured during the hour that closes at `at`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TvlReading {
    pub at: DateTime<Utc>,
    pub tvl_usd: Option<Decimal>,
}

/// One protocol's TVL over a window: its last reading, the one a window before it, and the part
/// of the last held by the pools discovered in the window. `start` is `None` when not measured.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtocolTvlChange {
    pub protocol: Protocol,
    pub end: TvlReading,
    pub start: Option<TvlReading>,
    pub discovered_tvl_usd: Option<Decimal>,
}

impl ProtocolTvlChange {
    /// The change without the discovered part; `None` as soon as one term is unknown.
    pub fn change_usd(&self) -> Option<Decimal> {
        let start = self.start.as_ref()?.tvl_usd?;
        Some(self.end.tvl_usd? - self.discovered_tvl_usd? - start)
    }
}

#[cfg(test)]
#[path = "model_tests.rs"]
mod tests;
