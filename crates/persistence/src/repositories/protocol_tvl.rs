//! PostgreSQL implementation of [`ProtocolTvlLookup`]: yog-analytic's hourly TVL readings.

mod rows;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use yog_core::{
    RepositoryResult,
    domain::{Protocol, ProtocolTvl, ProtocolTvlChange, ProtocolTvlLookup, Window},
};

use crate::repositories::helper::{bind_protocol_filter, map_sqlx_error};
use rows::{ProtocolTvlChangeRow, ProtocolTvlRow};

pub struct PgProtocolTvlRepository {
    pool: PgPool,
}

impl PgProtocolTvlRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ProtocolTvlLookup for PgProtocolTvlRepository {
    async fn list_latest(
        &self,
        last: DateTime<Utc>,
        protocol: Option<Protocol>,
    ) -> RepositoryResult<Vec<ProtocolTvl>> {
        let rows = sqlx::query_as!(
            ProtocolTvlRow,
            r#"
            SELECT DISTINCT ON (protocol)
                   protocol, bucket + INTERVAL '1 hour' AS "at!", tvl_usd, pools_priced,
                   pools_unvalued
              FROM analytic_protocol_tvl
             WHERE bucket <= $1 AND ($2::TEXT IS NULL OR protocol = $2)
             ORDER BY protocol, bucket DESC
            "#,
            last,
            bind_protocol_filter(protocol),
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter().map(ProtocolTvl::try_from).collect()
    }

    async fn list_hourly(
        &self,
        first: DateTime<Utc>,
        last: DateTime<Utc>,
        protocol: Option<Protocol>,
    ) -> RepositoryResult<Vec<ProtocolTvl>> {
        let rows = sqlx::query_as!(
            ProtocolTvlRow,
            r#"
            SELECT protocol, bucket + INTERVAL '1 hour' AS "at!", tvl_usd, pools_priced,
                   pools_unvalued
              FROM analytic_protocol_tvl
             WHERE bucket BETWEEN $1 AND $2 AND ($3::TEXT IS NULL OR protocol = $3)
             ORDER BY bucket, protocol
            "#,
            first,
            last,
            bind_protocol_filter(protocol),
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter().map(ProtocolTvl::try_from).collect()
    }

    async fn list_changes(
        &self,
        last: DateTime<Utc>,
        window: Window,
        protocol: Option<Protocol>,
    ) -> RepositoryResult<Vec<ProtocolTvlChange>> {
        let rows = sqlx::query_as!(
            ProtocolTvlChangeRow,
            r#"
            WITH latest AS (
                SELECT DISTINCT ON (protocol) protocol, bucket, tvl_usd
                  FROM analytic_protocol_tvl
                 WHERE bucket <= $1 AND ($4::TEXT IS NULL OR protocol = $4)
                 ORDER BY protocol, bucket DESC
            )
            SELECT l.protocol, l.bucket + INTERVAL '1 hour' AS "end_at!", l.tvl_usd AS end_tvl_usd,
                   s.bucket + INTERVAL '1 hour' AS start_at, s.tvl_usd AS start_tvl_usd,
                   d.tvl_usd AS "discovered_tvl_usd?"
              FROM latest l
              LEFT JOIN analytic_protocol_tvl s
                     ON s.protocol = l.protocol
                    AND s.bucket = l.bucket - make_interval(hours => $2)
              LEFT JOIN analytic_protocol_tvl_discovered d
                     ON d.protocol = l.protocol AND d.period = $3 AND d.bucket = l.bucket
             ORDER BY l.protocol
            "#,
            last,
            window.hours(),
            window.as_str(),
            bind_protocol_filter(protocol),
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter().map(ProtocolTvlChange::try_from).collect()
    }
}
