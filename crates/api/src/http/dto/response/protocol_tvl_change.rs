//! A row of `tvl/totals`: a protocol's TVL from the reading a window before its last to the last,
//! and the change between them without the pools discovered in the window.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::Serialize;
use utoipa::ToSchema;
use yog_core::domain::{ProtocolTvlChange, TvlReading};

use super::protocol::ProtocolResponse;
use crate::http::dto::schema::{SignedDecimal, UnsignedDecimal};

/// The TVL at `at`, the close of the hour it was measured in.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TvlReadingResponse {
    at: DateTime<Utc>,
    #[schema(value_type = Option<UnsignedDecimal>)]
    tvl_usd: Option<Decimal>,
}

impl From<TvlReading> for TvlReadingResponse {
    fn from(r: TvlReading) -> Self {
        Self {
            at: r.at,
            tvl_usd: r.tvl_usd,
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProtocolTvlChangeResponse {
    protocol: ProtocolResponse,
    /// The last reading at or before `asOf`.
    end: TvlReadingResponse,
    /// The reading exactly a window before `end`; `null` when it was not measured.
    start: Option<TvlReadingResponse>,
    /// What the pools first seen in the window, their birth unobserved, hold at `end`; `null`
    /// when not measured at that reading.
    #[schema(value_type = Option<UnsignedDecimal>)]
    discovered_tvl_usd: Option<Decimal>,
    /// `end − discoveredTvlUsd − start`; `null` as soon as one of them is.
    #[schema(value_type = Option<SignedDecimal>)]
    change_usd: Option<Decimal>,
}

impl From<ProtocolTvlChange> for ProtocolTvlChangeResponse {
    fn from(c: ProtocolTvlChange) -> Self {
        let change_usd = c.change_usd();
        Self {
            protocol: c.protocol.into(),
            end: c.end.into(),
            start: c.start.map(Into::into),
            discovered_tvl_usd: c.discovered_tvl_usd,
            change_usd,
        }
    }
}
