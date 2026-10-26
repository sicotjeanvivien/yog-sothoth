//! A protocol's or a pool's valued liquidity moves, and what they bring and take. Each USD sum
//! comes with its coverage, `movesValued` of `moves`.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::Serialize;
use utoipa::ToSchema;
use yog_core::domain::{LiquidityFlows, LiquidityMove, LiquidityMoveKind};

use super::protocol::ProtocolResponse;
use crate::http::dto::schema::{Count, UnsignedDecimal};

/// The liquidity deposited at the pools' creation, added and removed; the sums skip the
/// moves not valued.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LiquidityFlowsResponse {
    #[schema(value_type = UnsignedDecimal)]
    created_usd: Decimal,
    #[schema(value_type = UnsignedDecimal)]
    added_usd: Decimal,
    #[schema(value_type = UnsignedDecimal)]
    removed_usd: Decimal,
    #[schema(value_type = Count)]
    moves: i64,
    #[schema(value_type = Count)]
    moves_valued: i64,
}

impl From<LiquidityFlows> for LiquidityFlowsResponse {
    fn from(f: LiquidityFlows) -> Self {
        Self {
            created_usd: f.created_usd,
            added_usd: f.added_usd,
            removed_usd: f.removed_usd,
            moves: f.moves,
            moves_valued: f.moves_valued,
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub(crate) enum LiquidityMoveKindResponse {
    Create,
    Add,
    Remove,
}

/// One valued liquidity move, with its pool's token symbols.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LiquidityMoveResponse {
    protocol: ProtocolResponse,
    at: DateTime<Utc>,
    signature: String,
    pool_address: String,
    token_a_symbol: Option<String>,
    token_b_symbol: Option<String>,
    owner: String,
    kind: LiquidityMoveKindResponse,
    #[schema(value_type = UnsignedDecimal)]
    amount_usd: Decimal,
}

impl From<LiquidityMove> for LiquidityMoveResponse {
    fn from(m: LiquidityMove) -> Self {
        Self {
            protocol: m.protocol.into(),
            at: m.at,
            signature: m.signature,
            pool_address: m.pool_address.to_string(),
            token_a_symbol: m.token_a_symbol,
            token_b_symbol: m.token_b_symbol,
            owner: m.owner.to_string(),
            kind: match m.kind {
                LiquidityMoveKind::Create => LiquidityMoveKindResponse::Create,
                LiquidityMoveKind::Add => LiquidityMoveKindResponse::Add,
                LiquidityMoveKind::Remove => LiquidityMoveKindResponse::Remove,
            },
            amount_usd: m.amount_usd,
        }
    }
}
