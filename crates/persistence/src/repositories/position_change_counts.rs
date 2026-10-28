//! PostgreSQL implementation of [`PositionChangeCountsLookup`]: the position changes yog-analytic
//! counted, summed by protocol over a frozen period.

use async_trait::async_trait;
use sqlx::PgPool;
use yog_core::{
    RepositoryResult,
    domain::{FrozenPeriod, PositionChangeCounts, PositionChangeCountsLookup, Protocol},
};

use crate::repositories::helper::map_sqlx_error;

pub struct PgPositionChangeCountsRepository {
    pool: PgPool,
}

impl PgPositionChangeCountsRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl PositionChangeCountsLookup for PgPositionChangeCountsRepository {
    async fn find_totals(
        &self,
        period: &FrozenPeriod,
        protocol: Protocol,
    ) -> RepositoryResult<Option<PositionChangeCounts>> {
        sqlx::query_as!(
            PositionChangeCounts,
            r#"
            SELECT SUM(opened)::BIGINT AS "opened!", SUM(vesting_locks)::BIGINT AS "vesting_locks!",
                   SUM(permanent_locks)::BIGINT AS "permanent_locks!",
                   SUM(splits)::BIGINT AS "splits!", SUM(closed)::BIGINT AS "closed!"
              FROM analytic_protocol_position_changes_hourly
             WHERE protocol = $3 AND bucket >= $1 AND bucket < $2
            HAVING COUNT(*) > 0
            "#,
            period.from,
            period.as_of,
            protocol.as_str(),
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)
    }
}
