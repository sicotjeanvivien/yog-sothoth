use async_trait::async_trait;
use solana_pubkey::Pubkey;
use sqlx::PgPool;
use yog_core::{
    domain::{LiquidityEvent, LiquidityEventKind, LiquidityEventRepository},
    CoreError, CoreResult,
};

use crate::infra::db::{
    convert_i64_to_u64, convert_string_to_pubkey, convert_u64_to_i64,
    parse_string_to_liquidity_event_kind,
};

pub(crate) struct PgLiquidityEventRepository {
    pool: PgPool,
}

impl PgLiquidityEventRepository {
    pub(crate) fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl LiquidityEventRepository for PgLiquidityEventRepository {
    async fn insert(&self, event: &LiquidityEvent) -> CoreResult<()> {
        sqlx::query!(
            r#"
          INSERT INTO liquidity_events
              (pool_address, signature, liquidity_event_kind, amount_a, amount_b, timestamp)
          VALUES ($1, $2, $3, $4, $5, $6)
          "#,
            event.pool_address.to_string(),
            event.signature,
            event.liquidity_event_kind.to_string(),
            convert_u64_to_i64(event.amount_a, "amount_a")?,
            convert_u64_to_i64(event.amount_b, "amount_b")?,
            event.timestamp
        )
        .execute(&self.pool)
        .await
        .map_err(|e| CoreError::ParseError {
            signature: String::new(),
            reason: format!("db insert liquidity_events: {e}"),
        })?;
        Ok(())
    }

    async fn find_by_pool(
        &self,
        pool_address: &Pubkey,
        limit: i64,
    ) -> CoreResult<Vec<LiquidityEvent>> {
        let rows = sqlx::query!(
            r#"
            SELECT pool_address, signature, liquidity_event_kind, amount_a, amount_b, timestamp
            FROM liquidity_events
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
            reason: format!("db find_by_pool liquidity_events: {e}"),
        })?;

        rows.into_iter()
            .map(|row| {
                Ok(LiquidityEvent {
                    pool_address: convert_string_to_pubkey(row.pool_address, "pool_address")?,
                    liquidity_event_kind: parse_string_to_liquidity_event_kind(
                        row.liquidity_event_kind,
                        "liquidity_event_kind",
                    )?,
                    amount_a: convert_i64_to_u64(row.amount_a, "amount_a")?,
                    amount_b: convert_i64_to_u64(row.amount_b, "amount_b")?,
                    signature: row.signature,
                    timestamp: row.timestamp,
                })
            })
            .collect::<CoreResult<Vec<_>>>()
    }
}
