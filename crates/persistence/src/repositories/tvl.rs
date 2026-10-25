//! PostgreSQL implementation of [`TvlLookup`]: yog-analytic's hourly TVL readings.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use yog_core::{
    RepositoryResult,
    domain::{Protocol, ProtocolTvl, TvlLookup},
};

use super::protocol_activity::rows::ProtocolTvlRow;
use crate::repositories::helper::{bind_protocol_filter, map_sqlx_error};

pub struct PgTvlRepository {
    pool: PgPool,
}

impl PgTvlRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl TvlLookup for PgTvlRepository {
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
}
