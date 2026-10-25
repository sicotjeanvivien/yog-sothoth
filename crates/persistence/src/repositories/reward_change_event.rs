//! PostgreSQL implementation of [`RewardChangeEventLookup`]: each protocol's reward event
//! tables, merged into one list and read by scope.

mod rows;

use std::collections::HashMap;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rows::{
    MeteoraDammV2FundRewardEventRow, MeteoraDammV2InitializeRewardEventRow,
    MeteoraDammV2UpdateRewardDurationEventRow, MeteoraDammV2UpdateRewardFunderEventRow,
    MeteoraDammV2WithdrawDeadLiquidityRewardEventRow,
    MeteoraDammV2WithdrawIneligibleRewardEventRow,
};
use sqlx::PgPool;
use yog_core::{
    Cursor, Page, RepositoryResult,
    domain::{
        EventCursor, EventScope, MeteoraDammV2RewardChange, Protocol, RewardChangeEvent,
        RewardChangeEventLookup,
    },
};

use crate::repositories::helper::{
    EventKey, MergedEventKey, finish_event_page, ids_of, in_key_order, instants_of, map_sqlx_error,
};

pub struct PgRewardChangeEventRepository {
    pool: PgPool,
}

impl PgRewardChangeEventRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl RewardChangeEventLookup for PgRewardChangeEventRepository {
    async fn list_events(
        &self,
        scope: EventScope,
        from: DateTime<Utc>,
        cursor: Option<EventCursor>,
        limit: i64,
    ) -> RepositoryResult<Page<RewardChangeEvent>> {
        let key = EventKey::of(cursor.as_ref());
        let keys = match scope {
            // Every protocol's changes: DAMM v2's tables are the only ones yet.
            EventScope::All => self.list_meteora_damm_v2(from, &key, limit + 1).await?,
            EventScope::Protocol(Protocol::MeteoraDammV2) => {
                self.list_meteora_damm_v2(from, &key, limit + 1).await?
            }
            EventScope::Protocol(Protocol::MeteoraDlmm) => return Ok(Page::empty()),
            EventScope::Pool(pool) => {
                self.list_meteora_damm_v2_pool(&pool.to_string(), from, &key, limit + 1)
                    .await?
            }
        };
        let events = self
            .fetch_meteora_damm_v2(keys)
            .await?
            .into_iter()
            .map(RewardChangeEvent::MeteoraDammV2)
            .collect();
        Ok(finish_event_page(
            events,
            limit,
            cursor.is_some(),
            |event| {
                let RewardChangeEvent::MeteoraDammV2(change) = event;
                let (at, signature, event_index) = change.key();
                Cursor::RewardChangeEvent(Box::new(EventCursor {
                    scope,
                    from,
                    at,
                    signature,
                    event_index,
                }))
            },
        ))
    }
}

impl PgRewardChangeEventRepository {
    /// The keys of DAMM v2's changes after the key, each table read by its index on the instant
    /// and cut to `limit` before the merge.
    async fn list_meteora_damm_v2(
        &self,
        from: DateTime<Utc>,
        key: &EventKey,
        limit: i64,
    ) -> RepositoryResult<Vec<MergedEventKey>> {
        sqlx::query_as!(
            MergedEventKey,
            r#"
            SELECT kind AS "kind!", id AS "id!", timestamp AS "timestamp!"
              FROM (
                (SELECT 'initialize' AS kind, id, timestamp, signature, event_index
                      FROM meteora_damm_v2_initialize_reward_events
                     WHERE timestamp >= $1
                       AND timestamp <= COALESCE($2, 'infinity'::TIMESTAMPTZ)
                       AND (timestamp < COALESCE($2, 'infinity'::TIMESTAMPTZ)
                            OR (signature, event_index) < ($3, $4))
                     ORDER BY timestamp DESC, signature DESC, event_index DESC
                     LIMIT $5)
                UNION ALL
                (SELECT 'fund' AS kind, id, timestamp, signature, event_index
                      FROM meteora_damm_v2_fund_reward_events
                     WHERE timestamp >= $1
                       AND timestamp <= COALESCE($2, 'infinity'::TIMESTAMPTZ)
                       AND (timestamp < COALESCE($2, 'infinity'::TIMESTAMPTZ)
                            OR (signature, event_index) < ($3, $4))
                     ORDER BY timestamp DESC, signature DESC, event_index DESC
                     LIMIT $5)
                UNION ALL
                (SELECT 'duration_change' AS kind, id, timestamp, signature, event_index
                      FROM meteora_damm_v2_update_reward_duration_events
                     WHERE timestamp >= $1
                       AND timestamp <= COALESCE($2, 'infinity'::TIMESTAMPTZ)
                       AND (timestamp < COALESCE($2, 'infinity'::TIMESTAMPTZ)
                            OR (signature, event_index) < ($3, $4))
                     ORDER BY timestamp DESC, signature DESC, event_index DESC
                     LIMIT $5)
                UNION ALL
                (SELECT 'funder_change' AS kind, id, timestamp, signature, event_index
                      FROM meteora_damm_v2_update_reward_funder_events
                     WHERE timestamp >= $1
                       AND timestamp <= COALESCE($2, 'infinity'::TIMESTAMPTZ)
                       AND (timestamp < COALESCE($2, 'infinity'::TIMESTAMPTZ)
                            OR (signature, event_index) < ($3, $4))
                     ORDER BY timestamp DESC, signature DESC, event_index DESC
                     LIMIT $5)
                UNION ALL
                (SELECT 'ineligible_withdrawal' AS kind, id, timestamp, signature, event_index
                      FROM meteora_damm_v2_withdraw_ineligible_reward_events
                     WHERE timestamp >= $1
                       AND timestamp <= COALESCE($2, 'infinity'::TIMESTAMPTZ)
                       AND (timestamp < COALESCE($2, 'infinity'::TIMESTAMPTZ)
                            OR (signature, event_index) < ($3, $4))
                     ORDER BY timestamp DESC, signature DESC, event_index DESC
                     LIMIT $5)
                UNION ALL
                (SELECT 'dead_liquidity_withdrawal' AS kind, id, timestamp, signature, event_index
                      FROM meteora_damm_v2_withdraw_dead_liquidity_reward_events
                     WHERE timestamp >= $1
                       AND timestamp <= COALESCE($2, 'infinity'::TIMESTAMPTZ)
                       AND (timestamp < COALESCE($2, 'infinity'::TIMESTAMPTZ)
                            OR (signature, event_index) < ($3, $4))
                     ORDER BY timestamp DESC, signature DESC, event_index DESC
                     LIMIT $5)
              ) AS keys
             ORDER BY timestamp DESC, signature DESC, event_index DESC
             LIMIT $5
            "#,
            from,
            key.at,
            key.signature,
            key.event_index,
            limit,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)
    }

    /// The keys of one DAMM v2 pool's changes after the key.
    async fn list_meteora_damm_v2_pool(
        &self,
        pool: &str,
        from: DateTime<Utc>,
        key: &EventKey,
        limit: i64,
    ) -> RepositoryResult<Vec<MergedEventKey>> {
        sqlx::query_as!(
            MergedEventKey,
            r#"
            SELECT kind AS "kind!", id AS "id!", timestamp AS "timestamp!"
              FROM (
                (SELECT 'initialize' AS kind, id, timestamp, signature, event_index
                      FROM meteora_damm_v2_initialize_reward_events
                     WHERE pool_address = $1 AND timestamp >= $2
                       AND timestamp <= COALESCE($3, 'infinity'::TIMESTAMPTZ)
                       AND (timestamp < COALESCE($3, 'infinity'::TIMESTAMPTZ)
                            OR (signature, event_index) < ($4, $5))
                     ORDER BY timestamp DESC, signature DESC, event_index DESC
                     LIMIT $6)
                UNION ALL
                (SELECT 'fund' AS kind, id, timestamp, signature, event_index
                      FROM meteora_damm_v2_fund_reward_events
                     WHERE pool_address = $1 AND timestamp >= $2
                       AND timestamp <= COALESCE($3, 'infinity'::TIMESTAMPTZ)
                       AND (timestamp < COALESCE($3, 'infinity'::TIMESTAMPTZ)
                            OR (signature, event_index) < ($4, $5))
                     ORDER BY timestamp DESC, signature DESC, event_index DESC
                     LIMIT $6)
                UNION ALL
                (SELECT 'duration_change' AS kind, id, timestamp, signature, event_index
                      FROM meteora_damm_v2_update_reward_duration_events
                     WHERE pool_address = $1 AND timestamp >= $2
                       AND timestamp <= COALESCE($3, 'infinity'::TIMESTAMPTZ)
                       AND (timestamp < COALESCE($3, 'infinity'::TIMESTAMPTZ)
                            OR (signature, event_index) < ($4, $5))
                     ORDER BY timestamp DESC, signature DESC, event_index DESC
                     LIMIT $6)
                UNION ALL
                (SELECT 'funder_change' AS kind, id, timestamp, signature, event_index
                      FROM meteora_damm_v2_update_reward_funder_events
                     WHERE pool_address = $1 AND timestamp >= $2
                       AND timestamp <= COALESCE($3, 'infinity'::TIMESTAMPTZ)
                       AND (timestamp < COALESCE($3, 'infinity'::TIMESTAMPTZ)
                            OR (signature, event_index) < ($4, $5))
                     ORDER BY timestamp DESC, signature DESC, event_index DESC
                     LIMIT $6)
                UNION ALL
                (SELECT 'ineligible_withdrawal' AS kind, id, timestamp, signature, event_index
                      FROM meteora_damm_v2_withdraw_ineligible_reward_events
                     WHERE pool_address = $1 AND timestamp >= $2
                       AND timestamp <= COALESCE($3, 'infinity'::TIMESTAMPTZ)
                       AND (timestamp < COALESCE($3, 'infinity'::TIMESTAMPTZ)
                            OR (signature, event_index) < ($4, $5))
                     ORDER BY timestamp DESC, signature DESC, event_index DESC
                     LIMIT $6)
                UNION ALL
                (SELECT 'dead_liquidity_withdrawal' AS kind, id, timestamp, signature, event_index
                      FROM meteora_damm_v2_withdraw_dead_liquidity_reward_events
                     WHERE pool_address = $1 AND timestamp >= $2
                       AND timestamp <= COALESCE($3, 'infinity'::TIMESTAMPTZ)
                       AND (timestamp < COALESCE($3, 'infinity'::TIMESTAMPTZ)
                            OR (signature, event_index) < ($4, $5))
                     ORDER BY timestamp DESC, signature DESC, event_index DESC
                     LIMIT $6)
              ) AS keys
             ORDER BY timestamp DESC, signature DESC, event_index DESC
             LIMIT $6
            "#,
            pool,
            from,
            key.at,
            key.signature,
            key.event_index,
            limit,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)
    }

    /// The changes of `keys`, read table by table and put back in the order of `keys`.
    async fn fetch_meteora_damm_v2(
        &self,
        keys: Vec<MergedEventKey>,
    ) -> RepositoryResult<Vec<MeteoraDammV2RewardChange>> {
        let Some(span) = instants_of(&keys) else {
            return Ok(Vec::new());
        };
        let ids = |kind| ids_of(&keys, kind);
        let mut changes = HashMap::new();
        for row in self.fetch_initialize(&ids("initialize"), span).await? {
            let id = row.id;
            changes.insert(
                ("initialize", id),
                MeteoraDammV2RewardChange::Initialize(row.try_into()?),
            );
        }
        for row in self.fetch_fund(&ids("fund"), span).await? {
            let id = row.id;
            changes.insert(
                ("fund", id),
                MeteoraDammV2RewardChange::Fund(row.try_into()?),
            );
        }
        for row in self
            .fetch_duration_change(&ids("duration_change"), span)
            .await?
        {
            let id = row.id;
            changes.insert(
                ("duration_change", id),
                MeteoraDammV2RewardChange::DurationChange(row.try_into()?),
            );
        }
        for row in self
            .fetch_funder_change(&ids("funder_change"), span)
            .await?
        {
            let id = row.id;
            changes.insert(
                ("funder_change", id),
                MeteoraDammV2RewardChange::FunderChange(row.try_into()?),
            );
        }
        for row in self
            .fetch_ineligible_withdrawal(&ids("ineligible_withdrawal"), span)
            .await?
        {
            let id = row.id;
            changes.insert(
                ("ineligible_withdrawal", id),
                MeteoraDammV2RewardChange::IneligibleWithdrawal(row.try_into()?),
            );
        }
        for row in self
            .fetch_dead_liquidity_withdrawal(&ids("dead_liquidity_withdrawal"), span)
            .await?
        {
            let id = row.id;
            changes.insert(
                ("dead_liquidity_withdrawal", id),
                MeteoraDammV2RewardChange::DeadLiquidityWithdrawal(row.try_into()?),
            );
        }
        in_key_order(&keys, changes)
    }

    async fn fetch_initialize(
        &self,
        ids: &[i64],
        span: (DateTime<Utc>, DateTime<Utc>),
    ) -> RepositoryResult<Vec<MeteoraDammV2InitializeRewardEventRow>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        sqlx::query_as!(
            MeteoraDammV2InitializeRewardEventRow,
            r#"
            SELECT id, pool_address, signature, timestamp, slot, transaction_index, event_index,
                   reward_mint, funder, creator, reward_index, reward_duration
              FROM meteora_damm_v2_initialize_reward_events
             WHERE id = ANY($1) AND timestamp >= $2 AND timestamp <= $3
            "#,
            ids,
            span.0,
            span.1,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)
    }

    async fn fetch_fund(
        &self,
        ids: &[i64],
        span: (DateTime<Utc>, DateTime<Utc>),
    ) -> RepositoryResult<Vec<MeteoraDammV2FundRewardEventRow>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        sqlx::query_as!(
            MeteoraDammV2FundRewardEventRow,
            r#"
            SELECT id, pool_address, signature, timestamp, slot, transaction_index, event_index,
                   funder, mint_reward, reward_index, amount, transfer_fee_excluded_amount_in, reward_duration_end, pre_reward_rate, post_reward_rate
              FROM meteora_damm_v2_fund_reward_events
             WHERE id = ANY($1) AND timestamp >= $2 AND timestamp <= $3
            "#,
            ids,
            span.0,
            span.1,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)
    }

    async fn fetch_duration_change(
        &self,
        ids: &[i64],
        span: (DateTime<Utc>, DateTime<Utc>),
    ) -> RepositoryResult<Vec<MeteoraDammV2UpdateRewardDurationEventRow>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        sqlx::query_as!(
            MeteoraDammV2UpdateRewardDurationEventRow,
            r#"
            SELECT id, pool_address, signature, timestamp, slot, transaction_index, event_index,
                   reward_index, old_reward_duration, new_reward_duration
              FROM meteora_damm_v2_update_reward_duration_events
             WHERE id = ANY($1) AND timestamp >= $2 AND timestamp <= $3
            "#,
            ids,
            span.0,
            span.1,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)
    }

    async fn fetch_funder_change(
        &self,
        ids: &[i64],
        span: (DateTime<Utc>, DateTime<Utc>),
    ) -> RepositoryResult<Vec<MeteoraDammV2UpdateRewardFunderEventRow>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        sqlx::query_as!(
            MeteoraDammV2UpdateRewardFunderEventRow,
            r#"
            SELECT id, pool_address, signature, timestamp, slot, transaction_index, event_index,
                   reward_index, old_funder, new_funder
              FROM meteora_damm_v2_update_reward_funder_events
             WHERE id = ANY($1) AND timestamp >= $2 AND timestamp <= $3
            "#,
            ids,
            span.0,
            span.1,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)
    }

    async fn fetch_ineligible_withdrawal(
        &self,
        ids: &[i64],
        span: (DateTime<Utc>, DateTime<Utc>),
    ) -> RepositoryResult<Vec<MeteoraDammV2WithdrawIneligibleRewardEventRow>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        sqlx::query_as!(
            MeteoraDammV2WithdrawIneligibleRewardEventRow,
            r#"
            SELECT id, pool_address, signature, timestamp, slot, transaction_index, event_index,
                   reward_mint, amount
              FROM meteora_damm_v2_withdraw_ineligible_reward_events
             WHERE id = ANY($1) AND timestamp >= $2 AND timestamp <= $3
            "#,
            ids,
            span.0,
            span.1,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)
    }

    async fn fetch_dead_liquidity_withdrawal(
        &self,
        ids: &[i64],
        span: (DateTime<Utc>, DateTime<Utc>),
    ) -> RepositoryResult<Vec<MeteoraDammV2WithdrawDeadLiquidityRewardEventRow>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        sqlx::query_as!(
            MeteoraDammV2WithdrawDeadLiquidityRewardEventRow,
            r#"
            SELECT id, pool_address, signature, timestamp, slot, transaction_index, event_index,
                   reward_mint, amount
              FROM meteora_damm_v2_withdraw_dead_liquidity_reward_events
             WHERE id = ANY($1) AND timestamp >= $2 AND timestamp <= $3
            "#,
            ids,
            span.0,
            span.1,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)
    }
}
