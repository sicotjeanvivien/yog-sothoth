use chrono::{DateTime, Utc};
use serde::Serialize;
use yog_core::domain::{LastEventKind, PoolCurrentState};

/// `GET /api/pools/{address}/latest-state` response body.
///
/// `last_sqrt_price` and `liquidity` are emitted as JSON strings to
/// preserve the full u128 range across the JS bridge.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PoolCurrentStateResponse {
    pub(crate) pool_address: String,
    pub(crate) protocol: String,

    pub(crate) last_event_at: DateTime<Utc>,
    pub(crate) last_event_kind: String,
    pub(crate) last_signature: String,

    pub(crate) reserve_a: u64,
    pub(crate) reserve_b: u64,

    /// Q64.64 fixed-point; encoded as a string to keep precision in JS.
    pub(crate) last_sqrt_price: Option<String>,
    pub(crate) last_swap_at: Option<DateTime<Utc>>,

    /// Concentrated-liquidity L; encoded as a string to keep precision in JS.
    pub(crate) liquidity: Option<String>,
    pub(crate) last_liquidity_at: Option<DateTime<Utc>>,

    pub(crate) updated_at: DateTime<Utc>,
}

impl From<PoolCurrentState> for PoolCurrentStateResponse {
    fn from(state: PoolCurrentState) -> Self {
        Self {
            pool_address: state.pool_address,
            protocol: state.protocol,
            last_event_at: state.last_event_at,
            last_event_kind: last_event_kind_str(state.last_event_kind),
            last_signature: state.last_signature,
            reserve_a: state.reserve_a,
            reserve_b: state.reserve_b,
            last_sqrt_price: state.last_sqrt_price.map(|v| v.to_string()),
            last_swap_at: state.last_swap_at,
            liquidity: state.liquidity.map(|v| v.to_string()),
            last_liquidity_at: state.last_liquidity_at,
            updated_at: state.updated_at,
        }
    }
}

fn last_event_kind_str(kind: LastEventKind) -> String {
    kind.as_str().to_string()
}
