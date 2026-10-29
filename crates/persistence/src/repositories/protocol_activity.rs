//! PostgreSQL implementation of [`ProtocolActivityLookup`]: the hours yog-analytic wrote, and
//! the last it froze.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use yog_core::{
    RepositoryResult,
    domain::{FrozenHours, ProtocolActivityLookup, TRANCHE, WrittenTranches},
};

use crate::repositories::helper::map_sqlx_error;

pub struct PgProtocolActivityRepository {
    pool: PgPool,
}

impl PgProtocolActivityRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ProtocolActivityLookup for PgProtocolActivityRepository {
    async fn frozen_hours(&self) -> RepositoryResult<Option<FrozenHours>> {
        let row = sqlx::query!(
            r#"
            SELECT MIN(bucket) AS "first?: DateTime<Utc>",
                   MAX(bucket) FILTER (WHERE frozen_at IS NOT NULL)
                       AS "last_frozen?: DateTime<Utc>"
              FROM analytic_hours
            "#
        )
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        Ok(row
            .first
            .zip(row.last_frozen)
            .map(|(first, last_frozen)| FrozenHours { first, last_frozen }))
    }

    async fn written_tranches(&self) -> RepositoryResult<Option<WrittenTranches>> {
        let row = sqlx::query!(
            r#"
            -- ⚠️ The first tranche still held: `analytic_pool_5min` keeps 7 days, the record all.
            SELECT (SELECT MIN(bucket) FROM analytic_pool_5min) AS "first?: DateTime<Utc>",
                   MAX(bucket) AS "last?: DateTime<Utc>"
              FROM analytic_tranches
            "#
        )
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        Ok(row
            .first
            .zip(row.last)
            .map(|(first, last)| WrittenTranches {
                first,
                through: last + TRANCHE,
            }))
    }
}
