use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use yog_core::{
    RepositoryError,
    domain::{LiquidityFlows, LiquidityMove, LiquidityMoveKind, ProtocolBucket},
};

use crate::repositories::helper::{
    convert_bigdecimal_to_decimal, convert_string_to_pubkey, parse_bucket_from, parse_protocol,
};

/// A move, and the amount its page is ordered by: rounded as the cursor holds it.
pub(super) struct LiquidityMoveRow {
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

impl LiquidityMoveRow {
    pub(super) fn into_keyed(self) -> Result<(LiquidityMove, Decimal), RepositoryError> {
        let kind = LiquidityMoveKind::from_label(&self.kind).ok_or_else(|| {
            RepositoryError::Integrity(format!("unknown liquidity move kind {:?}", self.kind))
        })?;
        let item = LiquidityMove {
            protocol: parse_protocol(&self.protocol)?,
            at: self.at,
            signature: self.signature,
            event_index: self.event_index,
            pool_address: convert_string_to_pubkey(self.pool_address, "pool_address")?,
            token_a_symbol: self.token_a_symbol,
            token_b_symbol: self.token_b_symbol,
            owner: convert_string_to_pubkey(self.owner, "owner")?,
            kind,
            amount_usd: convert_bigdecimal_to_decimal(self.amount_usd, "amount_usd")?,
        };
        Ok((
            item,
            convert_bigdecimal_to_decimal(self.amount_key, "amount_key")?,
        ))
    }
}

pub(super) struct LiquidityFlowsRow {
    pub(super) protocol: String,
    pub(super) bucket_from: Option<DateTime<Utc>>,
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
            from: parse_bucket_from(row.bucket_from)?,
            value: row.try_into()?,
        })
    }
}
