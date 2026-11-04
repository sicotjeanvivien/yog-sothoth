//! The live reads of the liquidity changes, one function pair per table.

use async_trait::async_trait;
use chrono::Duration;
use yog_core::RepositoryResult;
use yog_core::domain::{
    LiquidityChangeEvent, LiquidityChangeTable as Table, LiveEventFeed, LiveRecord,
};

use super::PgLiquidityChangeEventRepository;
use super::rows::MeteoraDammV2LiquidityEventRow;
use crate::repositories::helper::{live_records, map_sqlx_error};

#[async_trait]
impl LiveEventFeed<Table, LiquidityChangeEvent> for PgLiquidityChangeEventRepository {
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
    ) -> RepositoryResult<Vec<LiveRecord<RepositoryResult<LiquidityChangeEvent>>>> {
        let lookback = lookback.num_seconds();
        match table {
            Table::MeteoraDammV2 => self.meteora_damm_v2_after(after_id, lookback, limit).await,
        }
    }
}

impl PgLiquidityChangeEventRepository {
    async fn meteora_damm_v2_tip(&self, lookback_secs: i64) -> RepositoryResult<Option<i64>> {
        sqlx::query_scalar!(
            r#"
            SELECT max(id) AS "id"
              FROM meteora_damm_v2_liquidity_events
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
    ) -> RepositoryResult<Vec<LiveRecord<RepositoryResult<LiquidityChangeEvent>>>> {
        let rows = sqlx::query_as!(
            MeteoraDammV2LiquidityEventRow,
            r#"
            SELECT id, pool_address, signature, timestamp, slot, transaction_index, event_index,
                   liquidity_event_kind, amount_a, amount_b, liquidity_delta, reserve_a_after,
                   reserve_b_after, position, owner
              FROM meteora_damm_v2_liquidity_events
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
            LiquidityChangeEvent::MeteoraDammV2,
        ))
    }
}
