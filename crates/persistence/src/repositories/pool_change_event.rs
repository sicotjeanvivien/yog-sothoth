//! PostgreSQL implementation of [`PoolChangeEventLookup`]: each protocol's pool status and fee
//! event tables, merged into one list and read by scope.

mod rows;

use std::collections::HashMap;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rows::{MeteoraDammV2SetPoolStatusEventRow, MeteoraDammV2UpdatePoolFeesEventRow};
use sqlx::PgPool;
use yog_core::{
    Cursor, Page, RepositoryResult,
    domain::{
        EventCursor, EventScope, MeteoraDammV2PoolChange, PoolChangeEvent, PoolChangeEventLookup,
        Protocol,
    },
};

use crate::repositories::helper::{
    EventKey, MergedEventKey, finish_event_page, ids_of, in_key_order, instants_of, map_sqlx_error,
};

pub struct PgPoolChangeEventRepository {
    pool: PgPool,
}

impl PgPoolChangeEventRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl PoolChangeEventLookup for PgPoolChangeEventRepository {
    async fn list_events(
        &self,
        scope: EventScope,
        from: DateTime<Utc>,
        cursor: Option<EventCursor>,
        limit: i64,
    ) -> RepositoryResult<Page<PoolChangeEvent>> {
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
            .map(PoolChangeEvent::MeteoraDammV2)
            .collect();
        Ok(finish_event_page(
            events,
            limit,
            cursor.is_some(),
            |event| {
                let PoolChangeEvent::MeteoraDammV2(change) = event;
                let (at, signature, event_index) = change.key();
                Cursor::PoolChangeEvent(Box::new(EventCursor {
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

impl PgPoolChangeEventRepository {
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
                (SELECT 'status' AS kind, id, timestamp, signature, event_index
                      FROM meteora_damm_v2_set_pool_status_events
                     WHERE timestamp >= $1
                       AND timestamp <= COALESCE($2, 'infinity'::TIMESTAMPTZ)
                       AND (timestamp < COALESCE($2, 'infinity'::TIMESTAMPTZ)
                            OR (signature, event_index) < ($3, $4))
                     ORDER BY timestamp DESC, signature DESC, event_index DESC
                     LIMIT $5)
                UNION ALL
                (SELECT 'fees' AS kind, id, timestamp, signature, event_index
                      FROM meteora_damm_v2_update_pool_fees_events
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
                (SELECT 'status' AS kind, id, timestamp, signature, event_index
                      FROM meteora_damm_v2_set_pool_status_events
                     WHERE pool_address = $1 AND timestamp >= $2
                       AND timestamp <= COALESCE($3, 'infinity'::TIMESTAMPTZ)
                       AND (timestamp < COALESCE($3, 'infinity'::TIMESTAMPTZ)
                            OR (signature, event_index) < ($4, $5))
                     ORDER BY timestamp DESC, signature DESC, event_index DESC
                     LIMIT $6)
                UNION ALL
                (SELECT 'fees' AS kind, id, timestamp, signature, event_index
                      FROM meteora_damm_v2_update_pool_fees_events
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
    ) -> RepositoryResult<Vec<MeteoraDammV2PoolChange>> {
        let Some(span) = instants_of(&keys) else {
            return Ok(Vec::new());
        };
        let ids = |kind| ids_of(&keys, kind);
        let mut changes = HashMap::new();
        for row in self.fetch_status(&ids("status"), span).await? {
            let id = row.id;
            changes.insert(
                ("status", id),
                MeteoraDammV2PoolChange::Status(row.try_into()?),
            );
        }
        for row in self.fetch_fees(&ids("fees"), span).await? {
            let id = row.id;
            changes.insert(("fees", id), MeteoraDammV2PoolChange::Fees(row.try_into()?));
        }
        in_key_order(&keys, changes)
    }

    async fn fetch_status(
        &self,
        ids: &[i64],
        span: (DateTime<Utc>, DateTime<Utc>),
    ) -> RepositoryResult<Vec<MeteoraDammV2SetPoolStatusEventRow>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        sqlx::query_as!(
            MeteoraDammV2SetPoolStatusEventRow,
            r#"
            SELECT id, pool_address, signature, timestamp, slot, transaction_index, event_index,
                   status
              FROM meteora_damm_v2_set_pool_status_events
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

    async fn fetch_fees(
        &self,
        ids: &[i64],
        span: (DateTime<Utc>, DateTime<Utc>),
    ) -> RepositoryResult<Vec<MeteoraDammV2UpdatePoolFeesEventRow>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        sqlx::query_as!(
            MeteoraDammV2UpdatePoolFeesEventRow,
            r#"
            SELECT id, pool_address, signature, timestamp, slot, transaction_index, event_index,
                   operator, params_raw
              FROM meteora_damm_v2_update_pool_fees_events
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
