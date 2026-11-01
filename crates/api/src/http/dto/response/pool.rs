//! A pool of `/api/pools` and its scoped lists: its identity, its tokens' identity, and its
//! state now — read live, never what yog-analytic computes.

mod last_event_kind_response;
mod state_response;

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::Serialize;
use utoipa::ToSchema;
use yog_core::domain::PoolEntry;

use super::protocol::ProtocolResponse;
use super::token::EmbeddedTokenResponse;
use crate::http::dto::schema::UnsignedDecimal;

use state_response::PoolStateResponse;

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PoolResponse {
    #[schema(min_length = 1)]
    pool_address: String,
    protocol: ProtocolResponse,
    token_a: EmbeddedTokenResponse,
    token_b: EmbeddedTokenResponse,
    /// Base trading fee in basis points, the genesis tier; `null` while the pool is unresolved.
    #[schema(value_type = Option<UnsignedDecimal>)]
    fee_bps: Option<Decimal>,
    first_seen_at: DateTime<Utc>,
    last_seen_at: DateTime<Utc>,
    /// `null` before the first event that moved the pool.
    state: Option<PoolStateResponse>,
}

impl From<PoolEntry> for PoolResponse {
    fn from(entry: PoolEntry) -> Self {
        let PoolEntry {
            pool,
            token_a,
            token_b,
            state,
            value,
        } = entry;
        Self {
            pool_address: pool.pool_address.to_string(),
            protocol: pool.protocol.into(),
            token_a: EmbeddedTokenResponse::from_sources(pool.token_a_mint, token_a),
            token_b: EmbeddedTokenResponse::from_sources(pool.token_b_mint, token_b),
            fee_bps: pool.fee_bps,
            first_seen_at: pool.first_seen_at,
            last_seen_at: pool.last_seen_at,
            state: state.map(|state| PoolStateResponse::new(state, value)),
        }
    }
}

#[cfg(test)]
#[path = "tests/pool_tests.rs"]
mod tests;
