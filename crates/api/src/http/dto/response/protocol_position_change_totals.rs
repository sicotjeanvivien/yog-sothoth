//! The row of a protocol's `position-changes` measure, `totals` and `series`: the positions
//! opened, locked, split and closed, one count per event.

use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;
use yog_core::domain::{PositionChangeCounts, Protocol, ProtocolBucket};

use super::protocol::ProtocolResponse;
use crate::http::dto::schema::Count;

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProtocolPositionChangeTotalsResponse {
    protocol: ProtocolResponse,
    #[schema(value_type = Count)]
    opened: i64,
    #[schema(value_type = Count)]
    vesting_locks: i64,
    #[schema(value_type = Count)]
    permanent_locks: i64,
    #[schema(value_type = Count)]
    splits: i64,
    #[schema(value_type = Count)]
    closed: i64,
}

impl ProtocolPositionChangeTotalsResponse {
    pub(crate) fn new(protocol: Protocol, counts: PositionChangeCounts) -> Self {
        Self {
            protocol: protocol.into(),
            opened: counts.opened,
            vesting_locks: counts.vesting_locks,
            permanent_locks: counts.permanent_locks,
            splits: counts.splits,
            closed: counts.closed,
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProtocolPositionChangeBucketResponse {
    from: DateTime<Utc>,
    #[serde(flatten)]
    counts: ProtocolPositionChangeTotalsResponse,
}

impl From<ProtocolBucket<PositionChangeCounts>> for ProtocolPositionChangeBucketResponse {
    fn from(b: ProtocolBucket<PositionChangeCounts>) -> Self {
        Self {
            from: b.from,
            counts: ProtocolPositionChangeTotalsResponse::new(b.protocol, b.value),
        }
    }
}

#[cfg(test)]
#[path = "tests/protocol_position_change_totals_tests.rs"]
mod tests;
