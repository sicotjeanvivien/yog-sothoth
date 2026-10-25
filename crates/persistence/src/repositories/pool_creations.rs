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
        ProtocolPoolCreations,
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
}
