//! Response DTOs for the public API.
//!
//! These types are the contract with the BFF and ultimately the browser.
//! They are deliberately decoupled from the domain types: a change to a
//! domain struct never silently breaks the wire format.
//!
//! # Conventions
//!
//! * `Pubkey` is rendered as its base58 string.
//! * `DateTime<Utc>` is rendered as an RFC 3339 / ISO 8601 string.
//! * `u64` is rendered as a JSON number — safe up to 2^53, which covers
//!   every SPL amount that can fit in 53 bits (about 9 PB of atomic units).
//! * `u128` is rendered as a JSON string ("BigInt safety") because JavaScript's
//!   `number` cannot represent values above 2^53 without precision loss.
//!   Frontends should use `BigInt(value)` to consume these fields.
//! * Optional fields are rendered as `null` when absent (`serde` default).

use chrono::{DateTime, Utc};
use serde::Serialize;

use yog_core::domain::{SwapEvent, TradeDirection};

// ---------------------------------------------------------------------------
// Swap event
// ---------------------------------------------------------------------------

/// `GET /api/pools/{address}/swaps` item.
#[derive(Debug, Serialize)]
pub(crate) struct SwapEventResponse {
    pub(crate) pool_address: String,
    pub(crate) protocol: String,
    pub(crate) signature: String,
    pub(crate) timestamp: DateTime<Utc>,

    pub(crate) token_a_mint: String,
    pub(crate) token_b_mint: String,

    pub(crate) trade_direction: String,
    pub(crate) amount_a: u64,
    pub(crate) amount_b: u64,

    pub(crate) reserve_a_after: u64,
    pub(crate) reserve_b_after: u64,
    /// Q64.64 fixed-point; encoded as a string.
    pub(crate) next_sqrt_price: String,

    pub(crate) claiming_fee: u64,
    pub(crate) protocol_fee: u64,
    pub(crate) compounding_fee: u64,
    pub(crate) referral_fee: u64,
    pub(crate) fee_token_is_a: bool,
}

impl From<SwapEvent> for SwapEventResponse {
    fn from(event: SwapEvent) -> Self {
        Self {
            pool_address: event.pool_address.to_string(),
            protocol: event.protocol.as_str().to_string(),
            signature: event.signature,
            timestamp: event.timestamp,
            token_a_mint: event.token_a_mint.to_string(),
            token_b_mint: event.token_b_mint.to_string(),
            trade_direction: trade_direction_str(event.trade_direction),
            amount_a: event.amount_a,
            amount_b: event.amount_b,
            reserve_a_after: event.reserve_a_after,
            reserve_b_after: event.reserve_b_after,
            next_sqrt_price: event.next_sqrt_price.to_string(),
            claiming_fee: event.claiming_fee,
            protocol_fee: event.protocol_fee,
            compounding_fee: event.compounding_fee,
            referral_fee: event.referral_fee,
            fee_token_is_a: event.fee_token_is_a,
        }
    }
}

fn trade_direction_str(d: TradeDirection) -> String {
    d.as_str().to_string()
}
