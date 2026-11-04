//! The live reads of the pool creations, one function pair per table.

use async_trait::async_trait;
use chrono::Duration;
use yog_core::RepositoryResult;
use yog_core::domain::{LiveEventFeed, LiveRecord, PoolCreationEvent, PoolCreationTable as Table};

use super::PgPoolCreationEventRepository;
use super::rows::MeteoraDammV2InitializePoolEventRow;
use crate::repositories::helper::{live_records, map_sqlx_error};

#[async_trait]
impl LiveEventFeed<Table, PoolCreationEvent> for PgPoolCreationEventRepository {
    async fn live_tip(&self, table: Table, lookback: Duration) -> RepositoryResult<Option<i64>> {
        let lookback = lookback.num_seconds();
        match table {
            Table::MeteoraDammV2 => self.meteora_damm_v2_tip(lookback).await,
        }
    }

    async fn live_after(
        &self,
        table: Table,
        after_id: i64,
        lookback: Duration,
        limit: i64,
    ) -> RepositoryResult<Vec<LiveRecord<RepositoryResult<PoolCreationEvent>>>> {
        let lookback = lookback.num_seconds();
        match table {
            Table::MeteoraDammV2 => self.meteora_damm_v2_after(after_id, lookback, limit).await,
        }
    }
}

impl PgPoolCreationEventRepository {
    async fn meteora_damm_v2_tip(&self, lookback_secs: i64) -> RepositoryResult<Option<i64>> {
        sqlx::query_scalar!(
            r#"
            SELECT max(id) AS "id"
              FROM meteora_damm_v2_initialize_pool_events
             WHERE timestamp >= now() - $1::BIGINT * INTERVAL '1 second'
            "#,
            lookback_secs,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx_error)
    }

    async fn meteora_damm_v2_after(
        &self,
        after_id: i64,
        lookback_secs: i64,
        limit: i64,
    ) -> RepositoryResult<Vec<LiveRecord<RepositoryResult<PoolCreationEvent>>>> {
        let rows = sqlx::query_as!(
            MeteoraDammV2InitializePoolEventRow,
            r#"
            SELECT id, pool_address, signature, timestamp, slot, transaction_index, event_index,
                   token_a_mint, token_b_mint, creator, payer, alpha_vault, sqrt_min_price,
                   sqrt_max_price, sqrt_price, liquidity, activation_type, activation_point,
                   collect_fee_mode, pool_type, token_a_flag, token_b_flag, token_a_amount,
                   token_b_amount, total_amount_a, total_amount_b, pool_fees_raw
              FROM meteora_damm_v2_initialize_pool_events
             WHERE id > $1
               AND timestamp >= now() - $2::BIGINT * INTERVAL '1 second'
             ORDER BY id
             LIMIT $3
            "#,
            after_id,
            lookback_secs,
            limit,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        Ok(live_records(
            rows,
            |row| row.id,
            PoolCreationEvent::MeteoraDammV2,
        ))
    }
}
