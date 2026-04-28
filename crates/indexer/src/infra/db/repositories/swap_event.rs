use async_trait::async_trait;
use solana_pubkey::Pubkey;
use sqlx::PgPool;
use std::str::FromStr;
use yog_core::{
    domain::{Protocol, SwapEvent, SwapEventRepository, TradeDirection},
    RepositoryError, RepositoryResult,
};

use crate::infra::db::{
    convert_bigdecimal_to_u128, convert_i64_to_u64, convert_string_to_pubkey,
    convert_u128_to_bigdecimal, convert_u64_to_i64, repository_utils::map_sqlx_error,
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
    async fn insert(&self, event: &SwapEvent) -> RepositoryResult<()> {
        sqlx::query!(
            r#"
            INSERT INTO swap_events (
                pool_address, protocol, signature,
                token_a_mint, token_b_mint,
                trade_direction, amount_a, amount_b,
                reserve_a_after, reserve_b_after, next_sqrt_price,
                claiming_fee, protocol_fee, compounding_fee, referral_fee,
                fee_token_is_a,
                timestamp
            )
            VALUES (
                $1, $2, $3,
                $4, $5,
                $6, $7, $8,
                $9, $10, $11,
                $12, $13, $14, $15,
                $16,
                $17
            )
            ON CONFLICT (signature, timestamp) DO NOTHING
            "#,
            event.pool_address.to_string(),
            event.protocol.as_str(),
            event.signature,
            event.token_a_mint.to_string(),
            event.token_b_mint.to_string(),
            event.trade_direction.as_str(),
            convert_u64_to_i64(event.amount_a, "amount_a")?,
            convert_u64_to_i64(event.amount_b, "amount_b")?,
            convert_u64_to_i64(event.reserve_a_after, "reserve_a_after")?,
            convert_u64_to_i64(event.reserve_b_after, "reserve_b_after")?,
            convert_u128_to_bigdecimal(event.next_sqrt_price, "next_sqrt_price"),
            convert_u64_to_i64(event.claiming_fee, "claiming_fee")?,
            convert_u64_to_i64(event.protocol_fee, "protocol_fee")?,
            convert_u64_to_i64(event.compounding_fee, "compounding_fee")?,
            convert_u64_to_i64(event.referral_fee, "referral_fee")?,
            event.fee_token_is_a,
            event.timestamp,
        )
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        Ok(())
    }

    async fn find_by_pool(
        &self,
        pool_address: &Pubkey,
        limit: i64,
    ) -> RepositoryResult<Vec<SwapEvent>> {
        let rows = sqlx::query!(
            r#"
            SELECT pool_address, protocol, signature,
                   token_a_mint, token_b_mint,
                   trade_direction, amount_a, amount_b,
                   reserve_a_after, reserve_b_after, next_sqrt_price,
                   claiming_fee, protocol_fee, compounding_fee, referral_fee,
                   fee_token_is_a,
                   timestamp
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
        .map_err(map_sqlx_error)?;

        rows.into_iter()
            .map(|row| {
                Ok(SwapEvent {
                    pool_address: convert_string_to_pubkey(row.pool_address, "pool_address")?,
                    protocol: Protocol::from_str(&row.protocol).map_err(|e| {
                        RepositoryError::Integrity(format!("invalid protocol: {e}"))
                    })?,
                    signature: row.signature,
                    timestamp: row.timestamp,
                    token_a_mint: convert_string_to_pubkey(row.token_a_mint, "token_a_mint")?,
                    token_b_mint: convert_string_to_pubkey(row.token_b_mint, "token_b_mint")?,
                    trade_direction: TradeDirection::from_str(&row.trade_direction).map_err(
                        |_| {
                            RepositoryError::Integrity(format!(
                                "invalid trade_direction: {}",
                                row.trade_direction
                            ))
                        },
                    )?,
                    amount_a: convert_i64_to_u64(row.amount_a, "amount_a")?,
                    amount_b: convert_i64_to_u64(row.amount_b, "amount_b")?,
                    reserve_a_after: convert_i64_to_u64(row.reserve_a_after, "reserve_a_after")?,
                    reserve_b_after: convert_i64_to_u64(row.reserve_b_after, "reserve_b_after")?,
                    next_sqrt_price: convert_bigdecimal_to_u128(
                        row.next_sqrt_price,
                        "next_sqrt_price",
                    )?,
                    claiming_fee: convert_i64_to_u64(row.claiming_fee, "claiming_fee")?,
                    protocol_fee: convert_i64_to_u64(row.protocol_fee, "protocol_fee")?,
                    compounding_fee: convert_i64_to_u64(row.compounding_fee, "compounding_fee")?,
                    referral_fee: convert_i64_to_u64(row.referral_fee, "referral_fee")?,
                    fee_token_is_a: row.fee_token_is_a,
                })
            })
            .collect::<RepositoryResult<Vec<_>>>()
    }
}
