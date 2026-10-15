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

mod trade_direction_response;

use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;
use yog_core::domain::{self, MeteoraDammV2SwapEvent};

use super::pool::ProtocolResponse;
use crate::http::dto::schema::U128String;

use trade_direction_response::TradeDirectionResponse;

// ---------------------------------------------------------------------------
// Swap event
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SwapEventResponse {
    #[schema(min_length = 1)]
    pub(crate) pool_address: String,
    pub(crate) protocol: ProtocolResponse,
    #[schema(min_length = 1)]
    pub(crate) signature: String,
    /// Position of this event among its transaction's emissions. A routed
    /// transaction produces several rows sharing `signature` and `timestamp`;
    /// this is what tells them apart — and the only stable key for a list row.
    pub(crate) event_index: u16,
    pub(crate) timestamp: DateTime<Utc>,

    pub(crate) trade_direction: TradeDirectionResponse,
    // All `u64` token quantities (amounts, reserves, fees) are emitted as
    // strings: an atomic-unit value can exceed 2^53, which a JSON-number
    // consumer (JS) silently truncates. Same rationale as `next_sqrt_price`.
    #[schema(value_type = U128String)]
    pub(crate) amount_a: String,
    #[schema(value_type = U128String)]
    pub(crate) amount_b: String,

    #[schema(value_type = U128String)]
    pub(crate) reserve_a_after: String,
    #[schema(value_type = U128String)]
    pub(crate) reserve_b_after: String,
    #[schema(value_type = U128String)]
    pub(crate) next_sqrt_price: String,

    #[schema(value_type = U128String)]
    pub(crate) claiming_fee: String,
    #[schema(value_type = U128String)]
    pub(crate) protocol_fee: String,
    #[schema(value_type = U128String)]
    pub(crate) compounding_fee: String,
    #[schema(value_type = U128String)]
    pub(crate) referral_fee: String,
    pub(crate) fee_token_is_a: bool,
}

impl From<MeteoraDammV2SwapEvent> for SwapEventResponse {
    fn from(event: MeteoraDammV2SwapEvent) -> Self {
        Self {
            pool_address: event.pool_address.to_string(),
            protocol: domain::Protocol::MeteoraDammV2.into(),
            signature: event.signature.to_string(),
            event_index: event.event_index,
            timestamp: event.timestamp,
            trade_direction: event.trade_direction.into(),
            amount_a: event.amount_a.to_string(),
            amount_b: event.amount_b.to_string(),
            reserve_a_after: event.reserve_a_after.to_string(),
            reserve_b_after: event.reserve_b_after.to_string(),
            next_sqrt_price: event.next_sqrt_price.to_string(),
            claiming_fee: event.claiming_fee.to_string(),
            protocol_fee: event.protocol_fee.to_string(),
            compounding_fee: event.compounding_fee.to_string(),
            referral_fee: event.referral_fee.to_string(),
            fee_token_is_a: event.fee_token_is_a,
        }
    }
}

#[cfg(test)]
#[path = "tests/swap_event_tests.rs"]
mod tests;
