// ---------------------------------------------------------------------------
// Liquidity event
// ---------------------------------------------------------------------------

use chrono::{DateTime, Utc};
use serde::Serialize;
use yog_core::domain::{LiquidityEvent, LiquidityEventKind};

/// `GET /api/pools/{address}/liquidity-events` item.
#[derive(Debug, Serialize)]
pub(crate) struct LiquidityEventResponse {
    pub(crate) pool_address: String,
    pub(crate) protocol: String,
    pub(crate) signature: String,
    pub(crate) timestamp: DateTime<Utc>,

    pub(crate) token_a_mint: String,
    pub(crate) token_b_mint: String,

    pub(crate) liquidity_event_kind: String,
    pub(crate) amount_a: u64,
    pub(crate) amount_b: u64,
    /// Liquidity delta (Q-format); encoded as a string.
    pub(crate) liquidity_delta: String,

    pub(crate) reserve_a_after: u64,
    pub(crate) reserve_b_after: u64,

    pub(crate) position: String,
    pub(crate) owner: String,
}

impl From<LiquidityEvent> for LiquidityEventResponse {
    fn from(event: LiquidityEvent) -> Self {
        Self {
            pool_address: event.pool_address.to_string(),
            protocol: event.protocol.as_str().to_string(),
            signature: event.signature,
            timestamp: event.timestamp,
            token_a_mint: event.token_a_mint.to_string(),
            token_b_mint: event.token_b_mint.to_string(),
            liquidity_event_kind: liquidity_event_kind_str(event.liquidity_event_kind),
            amount_a: event.amount_a,
            amount_b: event.amount_b,
            liquidity_delta: event.liquidity_delta.to_string(),
            reserve_a_after: event.reserve_a_after,
            reserve_b_after: event.reserve_b_after,
            position: event.position.to_string(),
            owner: event.owner.to_string(),
        }
    }
}

fn liquidity_event_kind_str(k: LiquidityEventKind) -> String {
    k.as_str().to_string()
}
