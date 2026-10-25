//! PostgreSQL implementation of [`PositionChangeEventLookup`]: each protocol's position event
//! tables, merged into one list and read by scope.

mod rows;

use std::collections::HashMap;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rows::{
    MeteoraDammV2ClosePositionEventRow, MeteoraDammV2CreatePositionEventRow,
    MeteoraDammV2LockPositionEventRow, MeteoraDammV2PermanentLockPositionEventRow,
    MeteoraDammV2SplitPositionEventRow, PositionChangeKey,
};
use sqlx::PgPool;
use yog_core::{
    Cursor, Page, RepositoryError, RepositoryResult,
    domain::{
        EventCursor, EventScope, MeteoraDammV2PositionChange, PositionChangeEvent,
        PositionChangeEventLookup, Protocol,
    },
};

use crate::repositories::helper::{EventKey, finish_event_page, map_sqlx_error};

pub struct PgPositionChangeEventRepository {
    pool: PgPool,
}

impl PgPositionChangeEventRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl PositionChangeEventLookup for PgPositionChangeEventRepository {
    async fn list_events(
        &self,
        scope: EventScope,
        from: DateTime<Utc>,
        cursor: Option<EventCursor>,
        limit: i64,
    ) -> RepositoryResult<Page<PositionChangeEvent>> {
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
            .map(PositionChangeEvent::MeteoraDammV2)
            .collect();
        Ok(finish_event_page(
            events,
            limit,
            cursor.is_some(),
            |event| {
                let PositionChangeEvent::MeteoraDammV2(change) = event;
                let (at, signature, event_index) = change.key();
                Cursor::PositionChangeEvent(Box::new(EventCursor {
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

impl PgPositionChangeEventRepository {
    /// The keys of DAMM v2's changes after the key, each table read by its index on the instant
    /// and cut to `limit` before the merge.
    async fn list_meteora_damm_v2(
        &self,
        from: DateTime<Utc>,
        key: &EventKey,
        limit: i64,
    ) -> RepositoryResult<Vec<PositionChangeKey>> {
        sqlx::query_as!(
            PositionChangeKey,
            r#"
            SELECT kind AS "kind!", id AS "id!", timestamp AS "timestamp!"
              FROM (
                (SELECT 'open' AS kind, id, timestamp, signature, event_index
                      FROM meteora_damm_v2_create_position_events
                     WHERE timestamp >= $1
                       AND timestamp <= COALESCE($2, 'infinity'::TIMESTAMPTZ)
                       AND (timestamp < COALESCE($2, 'infinity'::TIMESTAMPTZ)
                            OR (signature, event_index) < ($3, $4))
                     ORDER BY timestamp DESC, signature DESC, event_index DESC
                     LIMIT $5)
                UNION ALL
                (SELECT 'close' AS kind, id, timestamp, signature, event_index
                      FROM meteora_damm_v2_close_position_events
                     WHERE timestamp >= $1
                       AND timestamp <= COALESCE($2, 'infinity'::TIMESTAMPTZ)
                       AND (timestamp < COALESCE($2, 'infinity'::TIMESTAMPTZ)
                            OR (signature, event_index) < ($3, $4))
                     ORDER BY timestamp DESC, signature DESC, event_index DESC
                     LIMIT $5)
                UNION ALL
                (SELECT 'vesting_lock' AS kind, id, timestamp, signature, event_index
                      FROM meteora_damm_v2_lock_position_events
                     WHERE timestamp >= $1
                       AND timestamp <= COALESCE($2, 'infinity'::TIMESTAMPTZ)
                       AND (timestamp < COALESCE($2, 'infinity'::TIMESTAMPTZ)
                            OR (signature, event_index) < ($3, $4))
                     ORDER BY timestamp DESC, signature DESC, event_index DESC
                     LIMIT $5)
                UNION ALL
                (SELECT 'permanent_lock' AS kind, id, timestamp, signature, event_index
                      FROM meteora_damm_v2_permanent_lock_position_events
                     WHERE timestamp >= $1
                       AND timestamp <= COALESCE($2, 'infinity'::TIMESTAMPTZ)
                       AND (timestamp < COALESCE($2, 'infinity'::TIMESTAMPTZ)
                            OR (signature, event_index) < ($3, $4))
                     ORDER BY timestamp DESC, signature DESC, event_index DESC
                     LIMIT $5)
                UNION ALL
                (SELECT 'split' AS kind, id, timestamp, signature, event_index
                      FROM meteora_damm_v2_split_position_events
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
    ) -> RepositoryResult<Vec<PositionChangeKey>> {
        sqlx::query_as!(
            PositionChangeKey,
            r#"
            SELECT kind AS "kind!", id AS "id!", timestamp AS "timestamp!"
              FROM (
                (SELECT 'open' AS kind, id, timestamp, signature, event_index
                      FROM meteora_damm_v2_create_position_events
                     WHERE pool_address = $1 AND timestamp >= $2
                       AND timestamp <= COALESCE($3, 'infinity'::TIMESTAMPTZ)
                       AND (timestamp < COALESCE($3, 'infinity'::TIMESTAMPTZ)
                            OR (signature, event_index) < ($4, $5))
                     ORDER BY timestamp DESC, signature DESC, event_index DESC
                     LIMIT $6)
                UNION ALL
                (SELECT 'close' AS kind, id, timestamp, signature, event_index
                      FROM meteora_damm_v2_close_position_events
                     WHERE pool_address = $1 AND timestamp >= $2
                       AND timestamp <= COALESCE($3, 'infinity'::TIMESTAMPTZ)
                       AND (timestamp < COALESCE($3, 'infinity'::TIMESTAMPTZ)
                            OR (signature, event_index) < ($4, $5))
                     ORDER BY timestamp DESC, signature DESC, event_index DESC
                     LIMIT $6)
                UNION ALL
                (SELECT 'vesting_lock' AS kind, id, timestamp, signature, event_index
                      FROM meteora_damm_v2_lock_position_events
                     WHERE pool_address = $1 AND timestamp >= $2
                       AND timestamp <= COALESCE($3, 'infinity'::TIMESTAMPTZ)
                       AND (timestamp < COALESCE($3, 'infinity'::TIMESTAMPTZ)
                            OR (signature, event_index) < ($4, $5))
                     ORDER BY timestamp DESC, signature DESC, event_index DESC
                     LIMIT $6)
                UNION ALL
                (SELECT 'permanent_lock' AS kind, id, timestamp, signature, event_index
                      FROM meteora_damm_v2_permanent_lock_position_events
                     WHERE pool_address = $1 AND timestamp >= $2
                       AND timestamp <= COALESCE($3, 'infinity'::TIMESTAMPTZ)
                       AND (timestamp < COALESCE($3, 'infinity'::TIMESTAMPTZ)
                            OR (signature, event_index) < ($4, $5))
                     ORDER BY timestamp DESC, signature DESC, event_index DESC
                     LIMIT $6)
                UNION ALL
                (SELECT 'split' AS kind, id, timestamp, signature, event_index
                      FROM meteora_damm_v2_split_position_events
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
        keys: Vec<PositionChangeKey>,
    ) -> RepositoryResult<Vec<MeteoraDammV2PositionChange>> {
        let Some(span) = keys
            .iter()
            .map(|k| k.timestamp)
            .min()
            .zip(keys.iter().map(|k| k.timestamp).max())
        else {
            return Ok(Vec::new());
        };
        let ids = |kind: &str| -> Vec<i64> {
            keys.iter()
                .filter(|k| k.kind == kind)
                .map(|k| k.id)
                .collect()
        };
        let mut changes: HashMap<(&str, i64), MeteoraDammV2PositionChange> = HashMap::new();
        for row in self.fetch_open(&ids("open"), span).await? {
            changes.insert(
                ("open", row.id),
                MeteoraDammV2PositionChange::Open(row.try_into()?),
            );
        }
        for row in self.fetch_close(&ids("close"), span).await? {
            changes.insert(
                ("close", row.id),
                MeteoraDammV2PositionChange::Close(row.try_into()?),
            );
        }
        for row in self.fetch_vesting_lock(&ids("vesting_lock"), span).await? {
            let id = row.id;
            changes.insert(
                ("vesting_lock", id),
                MeteoraDammV2PositionChange::VestingLock(row.try_into()?),
            );
        }
        for row in self
            .fetch_permanent_lock(&ids("permanent_lock"), span)
            .await?
        {
            let id = row.id;
            changes.insert(
                ("permanent_lock", id),
                MeteoraDammV2PositionChange::PermanentLock(row.try_into()?),
            );
        }
        for row in self.fetch_split(&ids("split"), span).await? {
            let id = row.id;
            changes.insert(
                ("split", id),
                MeteoraDammV2PositionChange::Split(Box::new(row.try_into()?)),
            );
        }
        keys.iter()
            .map(|k| {
                changes.remove(&(k.kind.as_str(), k.id)).ok_or_else(|| {
                    RepositoryError::Integrity(format!(
                        "position change {} {} without its row",
                        k.kind, k.id
                    ))
                })
            })
            .collect()
    }

    async fn fetch_open(
        &self,
        ids: &[i64],
        span: (DateTime<Utc>, DateTime<Utc>),
    ) -> RepositoryResult<Vec<MeteoraDammV2CreatePositionEventRow>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        sqlx::query_as!(
            MeteoraDammV2CreatePositionEventRow,
            r#"
            SELECT id, pool_address, signature, timestamp, slot, transaction_index, event_index,
                   owner, position, position_nft_mint
              FROM meteora_damm_v2_create_position_events
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

    async fn fetch_close(
        &self,
        ids: &[i64],
        span: (DateTime<Utc>, DateTime<Utc>),
    ) -> RepositoryResult<Vec<MeteoraDammV2ClosePositionEventRow>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        sqlx::query_as!(
            MeteoraDammV2ClosePositionEventRow,
            r#"
            SELECT id, pool_address, signature, timestamp, slot, transaction_index, event_index,
                   owner, position, position_nft_mint
              FROM meteora_damm_v2_close_position_events
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

    async fn fetch_vesting_lock(
        &self,
        ids: &[i64],
        span: (DateTime<Utc>, DateTime<Utc>),
    ) -> RepositoryResult<Vec<MeteoraDammV2LockPositionEventRow>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        sqlx::query_as!(
            MeteoraDammV2LockPositionEventRow,
            r#"
            SELECT id, pool_address, signature, timestamp, slot, transaction_index, event_index,
                   position, owner, vesting, cliff_point, period_frequency,
                   cliff_unlock_liquidity, liquidity_per_period, number_of_period
              FROM meteora_damm_v2_lock_position_events
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

    async fn fetch_permanent_lock(
        &self,
        ids: &[i64],
        span: (DateTime<Utc>, DateTime<Utc>),
    ) -> RepositoryResult<Vec<MeteoraDammV2PermanentLockPositionEventRow>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        sqlx::query_as!(
            MeteoraDammV2PermanentLockPositionEventRow,
            r#"
            SELECT id, pool_address, signature, timestamp, slot, transaction_index, event_index,
                   position, lock_liquidity_amount, total_permanent_locked_liquidity
              FROM meteora_damm_v2_permanent_lock_position_events
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

    async fn fetch_split(
        &self,
        ids: &[i64],
        span: (DateTime<Utc>, DateTime<Utc>),
    ) -> RepositoryResult<Vec<MeteoraDammV2SplitPositionEventRow>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        sqlx::query_as!(
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
