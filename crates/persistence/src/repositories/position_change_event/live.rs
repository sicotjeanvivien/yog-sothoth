//! The live reads of the position changes, one function pair per table.

use async_trait::async_trait;
use chrono::Duration;
use yog_core::RepositoryResult;
use yog_core::domain::{
    LiveEventFeed, LiveRecord, MeteoraDammV2PositionChange, PositionChangeEvent,
    PositionChangeTable as Table,
};

use super::PgPositionChangeEventRepository;
use super::rows::{
    MeteoraDammV2ClosePositionEventRow, MeteoraDammV2CreatePositionEventRow,
    MeteoraDammV2LockPositionEventRow, MeteoraDammV2PermanentLockPositionEventRow,
    MeteoraDammV2SplitPositionEventRow,
};
use crate::repositories::helper::{live_records, map_sqlx_error};

#[async_trait]
impl LiveEventFeed<Table, PositionChangeEvent> for PgPositionChangeEventRepository {
    async fn live_tip(&self, table: Table, lookback: Duration) -> RepositoryResult<Option<i64>> {
        let lookback = lookback.num_seconds();
        match table {
            Table::MeteoraDammV2Open => self.meteora_damm_v2_open_tip(lookback).await,
            Table::MeteoraDammV2Close => self.meteora_damm_v2_close_tip(lookback).await,
            Table::MeteoraDammV2VestingLock => {
                self.meteora_damm_v2_vesting_lock_tip(lookback).await
            }
            Table::MeteoraDammV2PermanentLock => {
                self.meteora_damm_v2_permanent_lock_tip(lookback).await
            }
            Table::MeteoraDammV2Split => self.meteora_damm_v2_split_tip(lookback).await,
        }
    }

    async fn live_after(
        &self,
        table: Table,
        after_id: i64,
        lookback: Duration,
        limit: i64,
    ) -> RepositoryResult<Vec<LiveRecord<RepositoryResult<PositionChangeEvent>>>> {
        let lookback = lookback.num_seconds();
        match table {
            Table::MeteoraDammV2Open => {
                self.meteora_damm_v2_open_after(after_id, lookback, limit)
                    .await
            }
            Table::MeteoraDammV2Close => {
                self.meteora_damm_v2_close_after(after_id, lookback, limit)
                    .await
            }
            Table::MeteoraDammV2VestingLock => {
                self.meteora_damm_v2_vesting_lock_after(after_id, lookback, limit)
                    .await
            }
            Table::MeteoraDammV2PermanentLock => {
                self.meteora_damm_v2_permanent_lock_after(after_id, lookback, limit)
                    .await
            }
            Table::MeteoraDammV2Split => {
                self.meteora_damm_v2_split_after(after_id, lookback, limit)
                    .await
            }
        }
    }
}

impl PgPositionChangeEventRepository {
    async fn meteora_damm_v2_open_tip(&self, lookback_secs: i64) -> RepositoryResult<Option<i64>> {
        sqlx::query_scalar!(
            r#"
            SELECT max(id) AS "id"
              FROM meteora_damm_v2_create_position_events
             WHERE timestamp >= now() - $1::BIGINT * INTERVAL '1 second'
            "#,
            lookback_secs,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx_error)
    }

    async fn meteora_damm_v2_open_after(
        &self,
        after_id: i64,
        lookback_secs: i64,
        limit: i64,
    ) -> RepositoryResult<Vec<LiveRecord<RepositoryResult<PositionChangeEvent>>>> {
        let rows = sqlx::query_as!(
            MeteoraDammV2CreatePositionEventRow,
            r#"
            SELECT id, pool_address, signature, timestamp, slot, transaction_index, event_index,
                   owner, position, position_nft_mint
              FROM meteora_damm_v2_create_position_events
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
            |e| PositionChangeEvent::MeteoraDammV2(MeteoraDammV2PositionChange::Open(e)),
        ))
    }

    async fn meteora_damm_v2_close_tip(&self, lookback_secs: i64) -> RepositoryResult<Option<i64>> {
        sqlx::query_scalar!(
            r#"
            SELECT max(id) AS "id"
              FROM meteora_damm_v2_close_position_events
             WHERE timestamp >= now() - $1::BIGINT * INTERVAL '1 second'
            "#,
            lookback_secs,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx_error)
    }

    async fn meteora_damm_v2_close_after(
        &self,
        after_id: i64,
        lookback_secs: i64,
        limit: i64,
    ) -> RepositoryResult<Vec<LiveRecord<RepositoryResult<PositionChangeEvent>>>> {
        let rows = sqlx::query_as!(
            MeteoraDammV2ClosePositionEventRow,
            r#"
            SELECT id, pool_address, signature, timestamp, slot, transaction_index, event_index,
                   owner, position, position_nft_mint
              FROM meteora_damm_v2_close_position_events
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
            |e| PositionChangeEvent::MeteoraDammV2(MeteoraDammV2PositionChange::Close(e)),
        ))
    }

    async fn meteora_damm_v2_vesting_lock_tip(
        &self,
        lookback_secs: i64,
    ) -> RepositoryResult<Option<i64>> {
        sqlx::query_scalar!(
            r#"
            SELECT max(id) AS "id"
              FROM meteora_damm_v2_lock_position_events
             WHERE timestamp >= now() - $1::BIGINT * INTERVAL '1 second'
            "#,
            lookback_secs,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx_error)
    }

    async fn meteora_damm_v2_vesting_lock_after(
        &self,
        after_id: i64,
        lookback_secs: i64,
        limit: i64,
    ) -> RepositoryResult<Vec<LiveRecord<RepositoryResult<PositionChangeEvent>>>> {
        let rows = sqlx::query_as!(
            MeteoraDammV2LockPositionEventRow,
            r#"
            SELECT id, pool_address, signature, timestamp, slot, transaction_index, event_index,
                   position, owner, vesting, cliff_point, period_frequency,
                   cliff_unlock_liquidity, liquidity_per_period, number_of_period
              FROM meteora_damm_v2_lock_position_events
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
            |e| PositionChangeEvent::MeteoraDammV2(MeteoraDammV2PositionChange::VestingLock(e)),
        ))
    }

    async fn meteora_damm_v2_permanent_lock_tip(
        &self,
        lookback_secs: i64,
    ) -> RepositoryResult<Option<i64>> {
        sqlx::query_scalar!(
            r#"
            SELECT max(id) AS "id"
              FROM meteora_damm_v2_permanent_lock_position_events
             WHERE timestamp >= now() - $1::BIGINT * INTERVAL '1 second'
            "#,
            lookback_secs,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx_error)
    }

    async fn meteora_damm_v2_permanent_lock_after(
        &self,
        after_id: i64,
        lookback_secs: i64,
        limit: i64,
    ) -> RepositoryResult<Vec<LiveRecord<RepositoryResult<PositionChangeEvent>>>> {
        let rows = sqlx::query_as!(
            MeteoraDammV2PermanentLockPositionEventRow,
            r#"
            SELECT id, pool_address, signature, timestamp, slot, transaction_index, event_index,
                   position, lock_liquidity_amount, total_permanent_locked_liquidity
              FROM meteora_damm_v2_permanent_lock_position_events
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
            |e| PositionChangeEvent::MeteoraDammV2(MeteoraDammV2PositionChange::PermanentLock(e)),
        ))
    }

    async fn meteora_damm_v2_split_tip(&self, lookback_secs: i64) -> RepositoryResult<Option<i64>> {
        sqlx::query_scalar!(
            r#"
            SELECT max(id) AS "id"
              FROM meteora_damm_v2_split_position_events
             WHERE timestamp >= now() - $1::BIGINT * INTERVAL '1 second'
            "#,
            lookback_secs,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx_error)
    }

    async fn meteora_damm_v2_split_after(
        &self,
        after_id: i64,
        lookback_secs: i64,
        limit: i64,
    ) -> RepositoryResult<Vec<LiveRecord<RepositoryResult<PositionChangeEvent>>>> {
        let rows = sqlx::query_as!(
            MeteoraDammV2SplitPositionEventRow,
            r#"
            SELECT id, pool_address, signature, timestamp, slot, transaction_index, event_index,
                   first_owner, second_owner, first_position, second_position, current_sqrt_price,
                   split_permanent_locked_liquidity, split_unlocked_liquidity, split_vested_liquidity,
                   split_fee_a, split_fee_b, split_reward_0, split_reward_1,
                   first_unlocked_liquidity, first_permanent_locked_liquidity, first_vested_liquidity,
                   first_fee_a, first_fee_b, first_reward_0, first_reward_1,
                   second_unlocked_liquidity, second_permanent_locked_liquidity,
                   second_vested_liquidity, second_fee_a, second_fee_b, second_reward_0,
                   second_reward_1, num_unlocked_liquidity, num_permanent_locked_liquidity,
                   num_fee_a, num_fee_b, num_reward_0, num_reward_1, num_inner_vesting_liquidity
              FROM meteora_damm_v2_split_position_events
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
            |e| PositionChangeEvent::MeteoraDammV2(MeteoraDammV2PositionChange::Split(Box::new(e))),
        ))
    }
}
