//! The live reads of the reward changes, one function pair per table.

use async_trait::async_trait;
use chrono::Duration;
use yog_core::RepositoryResult;
use yog_core::domain::{
    LiveEventFeed, LiveRecord, MeteoraDammV2RewardChange, RewardChangeEvent,
    RewardChangeTable as Table,
};

use super::PgRewardChangeEventRepository;
use super::rows::{
    MeteoraDammV2FundRewardEventRow, MeteoraDammV2InitializeRewardEventRow,
    MeteoraDammV2UpdateRewardDurationEventRow, MeteoraDammV2UpdateRewardFunderEventRow,
    MeteoraDammV2WithdrawDeadLiquidityRewardEventRow,
    MeteoraDammV2WithdrawIneligibleRewardEventRow,
};
use crate::repositories::helper::{live_records, map_sqlx_error};

#[async_trait]
impl LiveEventFeed<Table, RewardChangeEvent> for PgRewardChangeEventRepository {
    async fn live_tip(&self, table: Table, lookback: Duration) -> RepositoryResult<Option<i64>> {
        let lookback = lookback.num_seconds();
        match table {
            Table::MeteoraDammV2Initialize => self.meteora_damm_v2_initialize_tip(lookback).await,
            Table::MeteoraDammV2Fund => self.meteora_damm_v2_fund_tip(lookback).await,
            Table::MeteoraDammV2DurationChange => {
                self.meteora_damm_v2_duration_change_tip(lookback).await
            }
            Table::MeteoraDammV2FunderChange => {
                self.meteora_damm_v2_funder_change_tip(lookback).await
            }
            Table::MeteoraDammV2IneligibleWithdrawal => {
                self.meteora_damm_v2_ineligible_withdrawal_tip(lookback)
                    .await
            }
            Table::MeteoraDammV2DeadLiquidityWithdrawal => {
                self.meteora_damm_v2_dead_liquidity_withdrawal_tip(lookback)
                    .await
            }
        }
    }

    async fn live_after(
        &self,
        table: Table,
        after_id: i64,
        lookback: Duration,
        limit: i64,
    ) -> RepositoryResult<Vec<LiveRecord<RepositoryResult<RewardChangeEvent>>>> {
        let lookback = lookback.num_seconds();
        match table {
            Table::MeteoraDammV2Initialize => {
                self.meteora_damm_v2_initialize_after(after_id, lookback, limit)
                    .await
            }
            Table::MeteoraDammV2Fund => {
                self.meteora_damm_v2_fund_after(after_id, lookback, limit)
                    .await
            }
            Table::MeteoraDammV2DurationChange => {
                self.meteora_damm_v2_duration_change_after(after_id, lookback, limit)
                    .await
            }
            Table::MeteoraDammV2FunderChange => {
                self.meteora_damm_v2_funder_change_after(after_id, lookback, limit)
                    .await
            }
            Table::MeteoraDammV2IneligibleWithdrawal => {
                self.meteora_damm_v2_ineligible_withdrawal_after(after_id, lookback, limit)
                    .await
            }
            Table::MeteoraDammV2DeadLiquidityWithdrawal => {
                self.meteora_damm_v2_dead_liquidity_withdrawal_after(after_id, lookback, limit)
                    .await
            }
        }
    }
}

impl PgRewardChangeEventRepository {
    async fn meteora_damm_v2_initialize_tip(
        &self,
        lookback_secs: i64,
    ) -> RepositoryResult<Option<i64>> {
        sqlx::query_scalar!(
            r#"
            SELECT max(id) AS "id"
              FROM meteora_damm_v2_initialize_reward_events
             WHERE timestamp >= now() - $1::BIGINT * INTERVAL '1 second'
            "#,
            lookback_secs,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx_error)
    }

    async fn meteora_damm_v2_initialize_after(
        &self,
        after_id: i64,
        lookback_secs: i64,
        limit: i64,
    ) -> RepositoryResult<Vec<LiveRecord<RepositoryResult<RewardChangeEvent>>>> {
        let rows = sqlx::query_as!(
            MeteoraDammV2InitializeRewardEventRow,
            r#"
            SELECT id, pool_address, signature, timestamp, slot, transaction_index, event_index,
                   reward_mint, funder, creator, reward_index, reward_duration
              FROM meteora_damm_v2_initialize_reward_events
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
            |e| RewardChangeEvent::MeteoraDammV2(MeteoraDammV2RewardChange::Initialize(e)),
        ))
    }

    async fn meteora_damm_v2_fund_tip(&self, lookback_secs: i64) -> RepositoryResult<Option<i64>> {
        sqlx::query_scalar!(
            r#"
            SELECT max(id) AS "id"
              FROM meteora_damm_v2_fund_reward_events
             WHERE timestamp >= now() - $1::BIGINT * INTERVAL '1 second'
            "#,
            lookback_secs,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx_error)
    }

    async fn meteora_damm_v2_fund_after(
        &self,
        after_id: i64,
        lookback_secs: i64,
        limit: i64,
    ) -> RepositoryResult<Vec<LiveRecord<RepositoryResult<RewardChangeEvent>>>> {
        let rows = sqlx::query_as!(
            MeteoraDammV2FundRewardEventRow,
            r#"
            SELECT id, pool_address, signature, timestamp, slot, transaction_index, event_index,
                   funder, mint_reward, reward_index, amount, transfer_fee_excluded_amount_in, reward_duration_end, pre_reward_rate, post_reward_rate
              FROM meteora_damm_v2_fund_reward_events
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
            |e| RewardChangeEvent::MeteoraDammV2(MeteoraDammV2RewardChange::Fund(e)),
        ))
    }

    async fn meteora_damm_v2_duration_change_tip(
        &self,
        lookback_secs: i64,
    ) -> RepositoryResult<Option<i64>> {
        sqlx::query_scalar!(
            r#"
            SELECT max(id) AS "id"
              FROM meteora_damm_v2_update_reward_duration_events
             WHERE timestamp >= now() - $1::BIGINT * INTERVAL '1 second'
            "#,
            lookback_secs,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx_error)
    }

    async fn meteora_damm_v2_duration_change_after(
        &self,
        after_id: i64,
        lookback_secs: i64,
        limit: i64,
    ) -> RepositoryResult<Vec<LiveRecord<RepositoryResult<RewardChangeEvent>>>> {
        let rows = sqlx::query_as!(
            MeteoraDammV2UpdateRewardDurationEventRow,
            r#"
            SELECT id, pool_address, signature, timestamp, slot, transaction_index, event_index,
                   reward_index, old_reward_duration, new_reward_duration
              FROM meteora_damm_v2_update_reward_duration_events
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
            |e| RewardChangeEvent::MeteoraDammV2(MeteoraDammV2RewardChange::DurationChange(e)),
        ))
    }

    async fn meteora_damm_v2_funder_change_tip(
        &self,
        lookback_secs: i64,
    ) -> RepositoryResult<Option<i64>> {
        sqlx::query_scalar!(
            r#"
            SELECT max(id) AS "id"
              FROM meteora_damm_v2_update_reward_funder_events
             WHERE timestamp >= now() - $1::BIGINT * INTERVAL '1 second'
            "#,
            lookback_secs,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx_error)
    }

    async fn meteora_damm_v2_funder_change_after(
        &self,
        after_id: i64,
        lookback_secs: i64,
        limit: i64,
    ) -> RepositoryResult<Vec<LiveRecord<RepositoryResult<RewardChangeEvent>>>> {
        let rows = sqlx::query_as!(
            MeteoraDammV2UpdateRewardFunderEventRow,
            r#"
            SELECT id, pool_address, signature, timestamp, slot, transaction_index, event_index,
                   reward_index, old_funder, new_funder
              FROM meteora_damm_v2_update_reward_funder_events
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
            |e| RewardChangeEvent::MeteoraDammV2(MeteoraDammV2RewardChange::FunderChange(e)),
        ))
    }

    async fn meteora_damm_v2_ineligible_withdrawal_tip(
        &self,
        lookback_secs: i64,
    ) -> RepositoryResult<Option<i64>> {
        sqlx::query_scalar!(
            r#"
            SELECT max(id) AS "id"
              FROM meteora_damm_v2_withdraw_ineligible_reward_events
             WHERE timestamp >= now() - $1::BIGINT * INTERVAL '1 second'
            "#,
            lookback_secs,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx_error)
    }

    async fn meteora_damm_v2_ineligible_withdrawal_after(
        &self,
        after_id: i64,
        lookback_secs: i64,
        limit: i64,
    ) -> RepositoryResult<Vec<LiveRecord<RepositoryResult<RewardChangeEvent>>>> {
        let rows = sqlx::query_as!(
            MeteoraDammV2WithdrawIneligibleRewardEventRow,
            r#"
            SELECT id, pool_address, signature, timestamp, slot, transaction_index, event_index,
                   reward_mint, amount
              FROM meteora_damm_v2_withdraw_ineligible_reward_events
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
            |e| {
                RewardChangeEvent::MeteoraDammV2(MeteoraDammV2RewardChange::IneligibleWithdrawal(e))
            },
        ))
    }

    async fn meteora_damm_v2_dead_liquidity_withdrawal_tip(
        &self,
        lookback_secs: i64,
    ) -> RepositoryResult<Option<i64>> {
        sqlx::query_scalar!(
            r#"
            SELECT max(id) AS "id"
              FROM meteora_damm_v2_withdraw_dead_liquidity_reward_events
             WHERE timestamp >= now() - $1::BIGINT * INTERVAL '1 second'
            "#,
            lookback_secs,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx_error)
    }

    async fn meteora_damm_v2_dead_liquidity_withdrawal_after(
        &self,
        after_id: i64,
        lookback_secs: i64,
        limit: i64,
    ) -> RepositoryResult<Vec<LiveRecord<RepositoryResult<RewardChangeEvent>>>> {
        let rows = sqlx::query_as!(
            MeteoraDammV2WithdrawDeadLiquidityRewardEventRow,
            r#"
            SELECT id, pool_address, signature, timestamp, slot, transaction_index, event_index,
                   reward_mint, amount
              FROM meteora_damm_v2_withdraw_dead_liquidity_reward_events
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
            |e| {
                RewardChangeEvent::MeteoraDammV2(
                    MeteoraDammV2RewardChange::DeadLiquidityWithdrawal(e),
                )
            },
        ))
    }
}
