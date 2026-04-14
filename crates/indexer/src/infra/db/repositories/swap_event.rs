use async_trait::async_trait;
use solana_pubkey::Pubkey;
use sqlx::PgPool;
use yog_core::{
    domain::{SwapEvent, SwapEventRepository},
    CoreError, CoreResult,
};

use crate::infra::db::{convert_i64_to_u64, convert_string_to_pubkey, convert_u64_to_i64};

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
            convert_u64_to_i64(event.amount_in, "amount_in")?,
            convert_u64_to_i64(event.amount_out, "amount_out")?,
            convert_u64_to_i64(event.reserve_a_before, "reserve_a_before")?,
            convert_u64_to_i64(event.reserve_b_before, "reserve_b_before")?,
            convert_u64_to_i64(event.reserve_a_after, "reserve_a_after")?,
            convert_u64_to_i64(event.reserve_b_after, "reserve_b_after")?,
            event.fee_bps.map(|f| f as i32),
            event
                .fee_amount
                .map(|f| convert_u64_to_i64(f, "fee_amount"))
                .transpose()?,
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
                Ok(SwapEvent {
                    pool_address: convert_string_to_pubkey(row.pool_address, "pool_address")?,
                    token_in_mint: convert_string_to_pubkey(row.token_in_mint, "token_in_mint")?,
                    token_out_mint: convert_string_to_pubkey(row.token_out_mint, "token_out_mint")?,
                    amount_in: convert_i64_to_u64(row.amount_in, "amount_in")?,
                    amount_out: convert_i64_to_u64(row.amount_out, "amount_out")?,
                    reserve_a_before: convert_i64_to_u64(row.reserve_a_before, "reserve_a_before")?,
                    reserve_b_before: convert_i64_to_u64(row.reserve_b_before, "reserve_b_before")?,
                    reserve_a_after: convert_i64_to_u64(row.reserve_a_after, "reserve_a_after")?,
                    reserve_b_after: convert_i64_to_u64(row.reserve_b_after, "reserve_b_after")?,
                    fee_bps: row.fee_bps.map(|f| f as u32),
                    fee_amount: row
                        .fee_amount
                        .map(|f| convert_i64_to_u64(f, "fee_amount"))
                        .transpose()?,
                    signature: row.signature,
                    timestamp: row.timestamp,
                })
            })
            .collect::<CoreResult<Vec<_>>>()
    }
}
