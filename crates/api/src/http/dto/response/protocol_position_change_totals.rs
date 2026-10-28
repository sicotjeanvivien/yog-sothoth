//! The row of a protocol's `position-changes/totals`: the positions opened, locked, split and
//! closed over the window, one count per event.

use serde::Serialize;
use utoipa::ToSchema;
use yog_core::domain::{PositionChangeCounts, Protocol};

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
