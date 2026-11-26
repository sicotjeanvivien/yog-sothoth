use rust_decimal::Decimal;

use super::{ProtocolTvl, TvlReading};
use crate::domain::{Protocol, ProtocolTvlPart};
use crate::tools::sum_known;

/// One protocol's TVL over a window: its last reading, the one a window before it, and the part
/// of the last held by the pools discovered in the window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtocolTvlChange {
    pub protocol: Protocol,
    pub end: TvlReading,
    pub start: Option<TvlReading>,
    pub discovered_tvl_usd: Option<Decimal>,
    /// The part of the last reading only its protocol has.
    pub protocol_part: Option<ProtocolTvlPart>,
}

impl ProtocolTvlChange {
    /// A last reading alone: no start, no discovered part, so no change.
    pub fn last_reading(reading: ProtocolTvl) -> Self {
        Self {
            protocol: reading.protocol,
            end: TvlReading {
                at: reading.at,
                tvl_usd: reading.tvl_usd,
            },
            start: None,
            discovered_tvl_usd: None,
            protocol_part: reading.protocol_part,
        }
    }

    /// The change without the discovered part; `None` as soon as one term is unknown.
    pub fn change_usd(&self) -> Option<Decimal> {
        let start = self.start.as_ref()?.tvl_usd?;
        Some(self.end.tvl_usd? - self.discovered_tvl_usd? - start)
    }

    /// The rows summed: a field unknown in one row is unknown; `None` without rows.
    pub fn total(rows: &[Self]) -> Option<TvlChangeTotal> {
        (!rows.is_empty()).then(|| TvlChangeTotal {
            end_tvl_usd: sum_known(rows.iter().map(|row| row.end.tvl_usd)),
            start_tvl_usd: sum_known(
                rows.iter()
                    .map(|row| row.start.as_ref().and_then(|start| start.tvl_usd)),
            ),
            discovered_tvl_usd: sum_known(rows.iter().map(|row| row.discovered_tvl_usd)),
            change_usd: sum_known(rows.iter().map(Self::change_usd)),
        })
    }
}

/// The protocols' TVL over a window, summed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TvlChangeTotal {
    pub end_tvl_usd: Option<Decimal>,
    pub start_tvl_usd: Option<Decimal>,
    pub discovered_tvl_usd: Option<Decimal>,
    pub change_usd: Option<Decimal>,
}

#[cfg(test)]
#[path = "change_tests.rs"]
mod tests;
