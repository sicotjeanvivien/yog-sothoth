use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use yog_core::{
    RepositoryError,
    domain::{LiquidityFlows, LiquidityMove, ProtocolBucket},
};

use crate::repositories::helper::{
    convert_bigdecimal_to_decimal, parse_bucket_from, parse_protocol,
};
use crate::repositories::protocol_activity::rows::LiquidityMoveRow;

/// A pool's move, and the amount its page is ordered by: rounded as the cursor holds it.
pub(super) struct PoolLiquidityMoveRow {
    pub(super) protocol: String,
    pub(super) at: DateTime<Utc>,
    pub(super) signature: String,
    pub(super) event_index: i32,
    pub(super) pool_address: String,
    pub(super) token_a_symbol: Option<String>,
    pub(super) token_b_symbol: Option<String>,
    pub(super) owner: String,
    pub(super) kind: String,
    pub(super) amount_usd: BigDecimal,
    pub(super) amount_key: BigDecimal,
}

impl PoolLiquidityMoveRow {
    pub(super) fn into_keyed(self) -> Result<(LiquidityMove, Decimal), RepositoryError> {
        let key = convert_bigdecimal_to_decimal(self.amount_key, "amount_key")?;
        let row = LiquidityMoveRow {
            protocol: self.protocol,
            at: self.at,
            signature: self.signature,
            event_index: self.event_index,
            pool_address: self.pool_address,
            token_a_symbol: self.token_a_symbol,
            token_b_symbol: self.token_b_symbol,
            owner: self.owner,
            kind: self.kind,
            amount_usd: self.amount_usd,
        };
        Ok((LiquidityMove::try_from(row)?, key))
    }
}

pub(super) struct LiquidityFlowsRow {
    pub(super) protocol: String,
    pub(super) bar_from: Option<DateTime<Utc>>,
    pub(super) created_usd: BigDecimal,
    pub(super) added_usd: BigDecimal,
    pub(super) removed_usd: BigDecimal,
    pub(super) moves: i64,
    pub(super) moves_valued: i64,
}

impl TryFrom<LiquidityFlowsRow> for LiquidityFlows {
    type Error = RepositoryError;

    fn try_from(row: LiquidityFlowsRow) -> Result<Self, Self::Error> {
        Ok(LiquidityFlows {
            created_usd: convert_bigdecimal_to_decimal(row.created_usd, "created_usd")?,
            added_usd: convert_bigdecimal_to_decimal(row.added_usd, "added_usd")?,
            removed_usd: convert_bigdecimal_to_decimal(row.removed_usd, "removed_usd")?,
            moves: row.moves,
            moves_valued: row.moves_valued,
        })
    }
}

impl TryFrom<LiquidityFlowsRow> for ProtocolBucket<LiquidityFlows> {
    type Error = RepositoryError;

    fn try_from(row: LiquidityFlowsRow) -> Result<Self, Self::Error> {
        Ok(ProtocolBucket {
            protocol: parse_protocol(&row.protocol)?,
            from: parse_bucket_from(row.bar_from)?,
            value: row.try_into()?,
        })
    }
}
