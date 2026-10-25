//! PostgreSQL implementation of [`RewardClaimEventLookup`]: each protocol's reward claim
//! table, read by scope.

mod rows;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rows::MeteoraDammV2ClaimRewardEventRow;
use sqlx::PgPool;
use yog_core::{
    Cursor, Page, RepositoryResult,
    domain::{
        EventCursor, EventScope, MeteoraDammV2ClaimRewardEvent, Protocol, RewardClaimEvent,
        RewardClaimEventLookup,
    },
};

use crate::repositories::helper::{EventKey, finish_event_page, map_sqlx_error};

pub struct PgRewardClaimEventRepository {
    pool: PgPool,
}

impl PgRewardClaimEventRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl RewardClaimEventLookup for PgRewardClaimEventRepository {
    async fn list_events(
        &self,
        scope: EventScope,
        from: DateTime<Utc>,
        cursor: Option<EventCursor>,
        limit: i64,
    ) -> RepositoryResult<Page<RewardClaimEvent>> {
        let key = EventKey::of(cursor.as_ref());
        let rows = match scope {
            // Every protocol's claims: DAMM v2's table is the only one yet.
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
        let events = rows
            .into_iter()
            .map(|row| {
                MeteoraDammV2ClaimRewardEvent::try_from(row).map(RewardClaimEvent::MeteoraDammV2)
            })
            .collect::<RepositoryResult<Vec<_>>>()?;
        Ok(finish_event_page(
            events,
            limit,
            cursor.is_some(),
            |event| {
                let RewardClaimEvent::MeteoraDammV2(e) = event;
                Cursor::RewardClaimEvent(Box::new(EventCursor {
                    scope,
                    from,
                    at: e.timestamp,
                    signature: e.signature,
                    event_index: e.event_index,
                }))
            },
        ))
    }
}

impl PgRewardClaimEventRepository {
    /// DAMM v2's claims after the key.
    async fn list_meteora_damm_v2(
        &self,
        from: DateTime<Utc>,
        key: &EventKey,
        limit: i64,
    ) -> RepositoryResult<Vec<MeteoraDammV2ClaimRewardEventRow>> {
        sqlx::query_as!(
            MeteoraDammV2ClaimRewardEventRow,
            r#"
            SELECT pool_address, signature, timestamp, slot, transaction_index, event_index,
                   position, owner, mint_reward, reward_index, total_reward
              FROM meteora_damm_v2_claim_reward_events
             WHERE timestamp >= $1
               AND timestamp <= COALESCE($2, 'infinity'::TIMESTAMPTZ)
               AND (timestamp < COALESCE($2, 'infinity'::TIMESTAMPTZ)
                    OR (signature, event_index) < ($3, $4))
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

    /// One DAMM v2 pool's claims after the key.
    async fn list_meteora_damm_v2_pool(
        &self,
        pool: &str,
        from: DateTime<Utc>,
        key: &EventKey,
        limit: i64,
    ) -> RepositoryResult<Vec<MeteoraDammV2ClaimRewardEventRow>> {
        sqlx::query_as!(
            MeteoraDammV2ClaimRewardEventRow,
            r#"
            SELECT pool_address, signature, timestamp, slot, transaction_index, event_index,
                   position, owner, mint_reward, reward_index, total_reward
              FROM meteora_damm_v2_claim_reward_events
             WHERE pool_address = $1 AND timestamp >= $2
               AND timestamp <= COALESCE($3, 'infinity'::TIMESTAMPTZ)
               AND (timestamp < COALESCE($3, 'infinity'::TIMESTAMPTZ)
                    OR (signature, event_index) < ($4, $5))
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
}
