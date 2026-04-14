use std::str::FromStr;

use async_trait::async_trait;
use solana_pubkey::Pubkey;
use sqlx::PgPool;
use yog_core::{
    domain::{SwapEvent, SwapEventRepository},
    CoreError, CoreResult,
};

pub(crate) struct PgSwapEventRepository {
    pool: PgPool,
}

impl PgSwapEventRepository {
    pub(crate) fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl SwapEventRepository for PgSwapEventRepository {
    async fn insert(&self, event: &SwapEvent) -> CoreResult<()> {
        sqlx::query!(
            r#"
            INSERT INTO swap_events
                (pool_address, signature, token_in_mint, token_out_mint,
                amount_in, amount_out,
                reserve_a_before, reserve_b_before, reserve_a_after, reserve_b_after,
                fee_bps, fee_amount, timestamp)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
            "#,
            event.pool_address.to_string(),
            event.signature,
            event.token_in_mint.to_string(),
            event.token_out_mint.to_string(),
            event.amount_in as i64,
            event.amount_out as i64,
            event.reserve_a_before as i64,
            event.reserve_b_before as i64,
            event.reserve_a_after as i64,
            event.reserve_b_after as i64,
            event.fee_bps.map(|f| f as i32),
            event.fee_amount.map(|f| f as i64),
            event.timestamp,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| CoreError::ParseError {
            signature: String::new(),
            reason: format!("db insert swap_event: {e}"),
        })?;

        Ok(())
    }

    async fn find_by_pool(&self, pool_address: &Pubkey, limit: i64) -> CoreResult<Vec<SwapEvent>> {
        let rows = sqlx::query!(
            r#"
            SELECT pool_address, signature, token_in_mint, token_out_mint,
                   amount_in, amount_out,
                   reserve_a_before, reserve_b_before, reserve_a_after, reserve_b_after,
                   fee_bps, fee_amount, timestamp
            FROM swap_events
            WHERE pool_address = $1
            ORDER BY timestamp DESC
            LIMIT $2
            "#,
            pool_address.to_string(),
            limit,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| CoreError::ParseError {
            signature: String::new(),
            reason: format!("db find_by_pool swap_events: {e}"),
        })?;

        rows.into_iter()
            .map(|row| {
                let parse_pubkey = |s: &str, field: &str| {
                    Pubkey::from_str(s).map_err(|e| CoreError::ParseError {
                        signature: String::new(),
                        reason: format!("invalid {field} pubkey: {e}"),
                    })
                };

                let parse_u64 = |v: i64, field: &str| {
                    u64::try_from(v).map_err(|e| CoreError::ParseError {
                        signature: String::new(),
                        reason: format!("invalid {field}: {e}"),
                    })
                };

                Ok(SwapEvent {
                    pool_address: parse_pubkey(&row.pool_address, "pool_address")?,
                    token_in_mint: parse_pubkey(&row.token_in_mint, "token_in_mint")?,
                    token_out_mint: parse_pubkey(&row.token_out_mint, "token_out_mint")?,
                    amount_in: parse_u64(row.amount_in, "amount_in")?,
                    amount_out: parse_u64(row.amount_out, "amount_out")?,
                    reserve_a_before: parse_u64(row.reserve_a_before, "reserve_a_before")?,
                    reserve_b_before: parse_u64(row.reserve_b_before, "reserve_b_before")?,
                    reserve_a_after: parse_u64(row.reserve_a_after, "reserve_a_after")?,
                    reserve_b_after: parse_u64(row.reserve_b_after, "reserve_b_after")?,
                    fee_bps: row.fee_bps.map(|f| f as u32),
                    fee_amount: row.fee_amount.map(|f| f as u64),
                    signature: row.signature,
                    timestamp: row.timestamp,
                })
            })
            .collect::<CoreResult<Vec<_>>>()
    }
}