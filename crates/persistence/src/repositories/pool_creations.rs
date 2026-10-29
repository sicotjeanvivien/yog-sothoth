//! PostgreSQL implementation of [`PoolCreationsLookup`]: the births yog-analytic counted, by
//! protocol and hour.

mod rows;

use async_trait::async_trait;
use rows::PoolCreationsRow;
use sqlx::PgPool;
use yog_core::{
    RepositoryResult,
    domain::{
        FrozenPeriod, PoolCreations, PoolCreationsLookup, Protocol, ProtocolBucket,
        ProtocolPoolCreations, TranchePeriod,
    },
};

use crate::repositories::helper::{bind_protocol_filter, map_sqlx_error};

pub struct PgPoolCreationsRepository {
    pool: PgPool,
}

impl PgPoolCreationsRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl PoolCreationsLookup for PgPoolCreationsRepository {
    async fn list_totals(
        &self,
        period: &FrozenPeriod,
        protocol: Option<Protocol>,
    ) -> RepositoryResult<Vec<ProtocolPoolCreations>> {
        let rows = sqlx::query_as!(
            PoolCreationsRow,
            r#"
            SELECT protocol AS "protocol!", NULL::TIMESTAMPTZ AS bucket_from,
                   SUM(births)::BIGINT AS "pools!"
              FROM analytic_protocol_births_hourly
             WHERE bucket >= $1 AND bucket < $2
               AND ($3::TEXT IS NULL OR protocol = $3)
             GROUP BY protocol
             ORDER BY protocol
            "#,
            period.from,
            period.as_of,
            bind_protocol_filter(protocol),
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter()
            .map(ProtocolPoolCreations::try_from)
            .collect()
    }

    async fn list_hourly(
        &self,
        period: &FrozenPeriod,
        protocol: Option<Protocol>,
    ) -> RepositoryResult<Vec<ProtocolBucket<PoolCreations>>> {
        let rows = sqlx::query_as!(
            PoolCreationsRow,
            r#"
            SELECT protocol, bucket AS "bucket_from?", births::BIGINT AS "pools!"
              FROM analytic_protocol_births_hourly
             WHERE bucket >= GREATEST($1::TIMESTAMPTZ, $3::TIMESTAMPTZ) AND bucket < $2
               AND ($4::TEXT IS NULL OR protocol = $4)
             ORDER BY bucket, protocol
            "#,
            period.from,
            period.as_of,
            period.covered_since,
            bind_protocol_filter(protocol),
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter().map(ProtocolBucket::try_from).collect()
    }

    async fn list_daily(
        &self,
        period: &FrozenPeriod,
        protocol: Option<Protocol>,
    ) -> RepositoryResult<Vec<ProtocolBucket<PoolCreations>>> {
        let rows = sqlx::query_as!(
            PoolCreationsRow,
            r#"
            SELECT protocol AS "protocol!", bucket_from AS "bucket_from?",
                   SUM(births)::BIGINT AS "pools!"
              FROM (
                  SELECT b.*,
                         time_bucket(INTERVAL '1 day', bucket, origin => $2::TIMESTAMPTZ)
                             AS bucket_from
                    FROM analytic_protocol_births_hourly b
                   WHERE bucket >= $1 AND bucket < $2
                     AND ($4::TEXT IS NULL OR protocol = $4)
              ) hours
             WHERE bucket_from >= $3
             GROUP BY protocol, bucket_from
             ORDER BY bucket_from, protocol
            "#,
            period.from,
            period.as_of,
            period.covered_since,
            bind_protocol_filter(protocol),
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter().map(ProtocolBucket::try_from).collect()
    }

    async fn list_totals_in_tranches(
        &self,
        period: &TranchePeriod,
        protocol: Option<Protocol>,
    ) -> RepositoryResult<Vec<ProtocolPoolCreations>> {
        // ⚠️ DAMM v2's creations are the only ones yet: another protocol would add its table here.
        let rows = sqlx::query_as!(
            PoolCreationsRow,
            r#"
            SELECT $3::TEXT AS "protocol!", NULL::TIMESTAMPTZ AS bucket_from,
                   COUNT(DISTINCT e.pool_address) AS "pools!"
              FROM meteora_damm_v2_initialize_pool_events e
             WHERE e.timestamp >= $1 AND e.timestamp < $2
            -- ⚠️ HAVING, not WHERE: an aggregate without GROUP BY keeps its row, 0, for another protocol.
            HAVING $4::TEXT IS NULL OR $4 = $3
            "#,
            period.from,
            period.as_of,
            Protocol::MeteoraDammV2.as_str(),
            bind_protocol_filter(protocol),
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter()
            .map(ProtocolPoolCreations::try_from)
            .collect()
    }

    async fn list_tranches(
        &self,
        period: &TranchePeriod,
        protocol: Option<Protocol>,
    ) -> RepositoryResult<Vec<ProtocolBucket<PoolCreations>>> {
        let rows = sqlx::query_as!(
            PoolCreationsRow,
            r#"
            SELECT $4::TEXT AS "protocol!", t.bucket AS bucket_from,
                   COUNT(DISTINCT e.pool_address) AS "pools!"
              FROM generate_series(GREATEST($1::TIMESTAMPTZ, $3::TIMESTAMPTZ),
                                   $2::TIMESTAMPTZ - INTERVAL '5 minutes',
                                   INTERVAL '5 minutes') AS t (bucket)
              LEFT JOIN meteora_damm_v2_initialize_pool_events e
                     ON e.timestamp >= t.bucket AND e.timestamp < t.bucket + INTERVAL '5 minutes'
                    -- ⚠️ Constant bounds too: the planner excludes chunks on them, never on `t.bucket`.
                    AND e.timestamp >= $1 AND e.timestamp < $2
             WHERE $5::TEXT IS NULL OR $5 = $4
             GROUP BY t.bucket
             ORDER BY t.bucket
            "#,
            period.from,
            period.as_of,
            period.covered_since,
            Protocol::MeteoraDammV2.as_str(),
            bind_protocol_filter(protocol),
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter().map(ProtocolBucket::try_from).collect()
    }
}
