//! The part of a TVL held by the pools first seen in a window, their birth unobserved:
//! liquidity that enters our view without entering the protocol.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;

use crate::domain::{Protocol, Window};

/// The part of a TVL held by the pools first seen in `window`, their birth unobserved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredTvl {
    pub window: Window,
    pub tvl_usd: Decimal,
    pub pools: i64,
}

/// One protocol's discovered part at the TVL reading of the hour that closes at `at`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtocolDiscoveredTvl {
    pub protocol: Protocol,
    pub at: DateTime<Utc>,
    pub pools: i64,
    pub tvl_usd: Decimal,
}
