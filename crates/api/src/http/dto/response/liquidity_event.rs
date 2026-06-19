// ---------------------------------------------------------------------------
// Liquidity event
// ---------------------------------------------------------------------------

use chrono::{DateTime, Utc};
use serde::Serialize;
use yog_core::domain::{MeteoraDammV2LiquidityEvent, MeteoraDammV2LiquidityEventKind, Protocol};

/// `GET /api/pools/{address}/liquidity-events` item.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LiquidityEventResponse {
    pub(super) pool_address: String,
    pub(super) protocol: String,
    pub(super) signature: String,
    pub(super) timestamp: DateTime<Utc>,

    pub(super) liquidity_event_kind: String,
    // u64 quantities emitted as strings (can exceed 2^53); see SwapEventResponse.
    pub(super) amount_a: String,
    pub(super) amount_b: String,
    pub(super) liquidity_delta: String,

    pub(super) reserve_a_after: String,
    pub(super) reserve_b_after: String,

    pub(super) position: String,
    pub(super) owner: String,
}

impl From<MeteoraDammV2LiquidityEvent> for LiquidityEventResponse {
    fn from(event: MeteoraDammV2LiquidityEvent) -> Self {
        Self {
            pool_address: event.pool_address.to_string(),
            protocol: Protocol::MeteoraDammV2.to_string(),
            signature: event.signature.to_string(),
            timestamp: event.timestamp,
            liquidity_event_kind: liquidity_event_kind_str(event.liquidity_event_kind),
            amount_a: event.amount_a.to_string(),
            amount_b: event.amount_b.to_string(),
            liquidity_delta: event.liquidity_delta.to_string(),
            reserve_a_after: event.reserve_a_after.to_string(),
            reserve_b_after: event.reserve_b_after.to_string(),
            position: event.position.to_string(),
            owner: event.owner.to_string(),
        }
    }
}

fn liquidity_event_kind_str(k: MeteoraDammV2LiquidityEventKind) -> String {
    k.as_str().to_string()
}
