//! The live reads of the protocol fee claims, one function pair per table.

use async_trait::async_trait;
use chrono::Duration;
use yog_core::RepositoryResult;
use yog_core::domain::{
    LiveEventFeed, LiveRecord, ProtocolFeeClaimEvent, ProtocolFeeClaimTable as Table,
};

use super::PgProtocolFeeClaimEventRepository;
use super::rows::MeteoraDammV2ClaimProtocolFeeEventRow;
use crate::repositories::helper::{live_records, map_sqlx_error};

#[async_trait]
impl LiveEventFeed<Table, ProtocolFeeClaimEvent> for PgProtocolFeeClaimEventRepository {
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
    ) -> RepositoryResult<Vec<LiveRecord<RepositoryResult<ProtocolFeeClaimEvent>>>> {
        let lookback = lookback.num_seconds();
        match table {
            Table::MeteoraDammV2 => self.meteora_damm_v2_after(after_id, lookback, limit).await,
        }
    }
}

impl PgProtocolFeeClaimEventRepository {
    async fn meteora_damm_v2_tip(&self, lookback_secs: i64) -> RepositoryResult<Option<i64>> {
        sqlx::query_scalar!(
            r#"
            SELECT max(id) AS "id"
              FROM meteora_damm_v2_claim_protocol_fee_events
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
    ) -> RepositoryResult<Vec<LiveRecord<RepositoryResult<ProtocolFeeClaimEvent>>>> {
        let rows = sqlx::query_as!(
            MeteoraDammV2ClaimProtocolFeeEventRow,
            r#"
            SELECT id, pool_address, signature, timestamp, slot, transaction_index, event_index,
                   token_a_amount, token_b_amount
              FROM meteora_damm_v2_claim_protocol_fee_events
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
            ProtocolFeeClaimEvent::MeteoraDammV2,
        ))
    }
}
