//! The live reads of the pool changes, one function pair per table.

use async_trait::async_trait;
use chrono::Duration;
use yog_core::RepositoryResult;
use yog_core::domain::{
    LiveEventFeed, LiveRecord, MeteoraDammV2PoolChange, PoolChangeEvent, PoolChangeTable as Table,
};

use super::PgPoolChangeEventRepository;
use super::rows::{MeteoraDammV2SetPoolStatusEventRow, MeteoraDammV2UpdatePoolFeesEventRow};
use crate::repositories::helper::{live_records, map_sqlx_error};

#[async_trait]
impl LiveEventFeed<Table, PoolChangeEvent> for PgPoolChangeEventRepository {
    async fn live_tip(&self, table: Table, lookback: Duration) -> RepositoryResult<Option<i64>> {
        let lookback = lookback.num_seconds();
        match table {
            Table::MeteoraDammV2Status => self.meteora_damm_v2_status_tip(lookback).await,
            Table::MeteoraDammV2Fees => self.meteora_damm_v2_fees_tip(lookback).await,
        }
    }

    async fn live_after(
        &self,
        table: Table,
        after_id: i64,
        lookback: Duration,
        limit: i64,
    ) -> RepositoryResult<Vec<LiveRecord<RepositoryResult<PoolChangeEvent>>>> {
        let lookback = lookback.num_seconds();
        match table {
            Table::MeteoraDammV2Status => {
                self.meteora_damm_v2_status_after(after_id, lookback, limit)
                    .await
            }
            Table::MeteoraDammV2Fees => {
                self.meteora_damm_v2_fees_after(after_id, lookback, limit)
                    .await
            }
        }
    }
}

impl PgPoolChangeEventRepository {
    async fn meteora_damm_v2_status_tip(
        &self,
        lookback_secs: i64,
    ) -> RepositoryResult<Option<i64>> {
        sqlx::query_scalar!(
            r#"
            SELECT max(id) AS "id"
              FROM meteora_damm_v2_set_pool_status_events
             WHERE timestamp >= now() - $1::BIGINT * INTERVAL '1 second'
            "#,
            lookback_secs,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx_error)
    }

    async fn meteora_damm_v2_status_after(
        &self,
        after_id: i64,
        lookback_secs: i64,
        limit: i64,
    ) -> RepositoryResult<Vec<LiveRecord<RepositoryResult<PoolChangeEvent>>>> {
        let rows = sqlx::query_as!(
            MeteoraDammV2SetPoolStatusEventRow,
            r#"
            SELECT id, pool_address, signature, timestamp, slot, transaction_index, event_index,
                   status
              FROM meteora_damm_v2_set_pool_status_events
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
            |e| PoolChangeEvent::MeteoraDammV2(MeteoraDammV2PoolChange::Status(e)),
        ))
    }

    async fn meteora_damm_v2_fees_tip(&self, lookback_secs: i64) -> RepositoryResult<Option<i64>> {
        sqlx::query_scalar!(
            r#"
            SELECT max(id) AS "id"
              FROM meteora_damm_v2_update_pool_fees_events
             WHERE timestamp >= now() - $1::BIGINT * INTERVAL '1 second'
            "#,
            lookback_secs,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx_error)
    }

    async fn meteora_damm_v2_fees_after(
        &self,
        after_id: i64,
        lookback_secs: i64,
        limit: i64,
    ) -> RepositoryResult<Vec<LiveRecord<RepositoryResult<PoolChangeEvent>>>> {
        let rows = sqlx::query_as!(
            MeteoraDammV2UpdatePoolFeesEventRow,
            r#"
            SELECT id, pool_address, signature, timestamp, slot, transaction_index, event_index,
                   operator, params_raw
              FROM meteora_damm_v2_update_pool_fees_events
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
            |e| PoolChangeEvent::MeteoraDammV2(MeteoraDammV2PoolChange::Fees(e)),
        ))
    }
}
