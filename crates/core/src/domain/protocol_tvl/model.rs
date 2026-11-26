//! The TVL, a reading at an instant: each protocol's reading of an hour, never a sum.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;

use crate::domain::{Protocol, ProtocolTvlPart};

/// One protocol's TVL as last measured during the hour that closes at `at`: its pools
/// valued by their priced sides; `pools_unvalued` counts those with no side valued.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtocolTvl {
    pub protocol: Protocol,
    pub at: DateTime<Utc>,
    pub tvl_usd: Option<Decimal>,
    pub pools_priced: i64,
    pub pools_unvalued: i64,
    /// `None` for another protocol, and for a reading made before the locks were measured.
    pub protocol_part: Option<ProtocolTvlPart>,
}

/// A TVL reading: the protocol's TVL as measured during the hour that closes at `at`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TvlReading {
    pub at: DateTime<Utc>,
    pub tvl_usd: Option<Decimal>,
}
