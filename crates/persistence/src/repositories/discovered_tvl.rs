//! PostgreSQL implementation of [`DiscoveredTvlLookup`]: the part of each TVL reading held by
//! the pools discovered in a window.

mod rows;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rows::ProtocolDiscoveredTvlRow;
use sqlx::PgPool;
use yog_core::{
    RepositoryResult,
    domain::{DiscoveredTvlLookup, Protocol, ProtocolDiscoveredTvl, Window},
};

use crate::repositories::helper::{bind_protocol_filter, map_sqlx_error};

pub struct PgDiscoveredTvlRepository {
    pool: PgPool,
}

impl PgDiscoveredTvlRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl DiscoveredTvlLookup for PgDiscoveredTvlRepository {
    async fn list_totals(
        &self,
        last: DateTime<Utc>,
        window: Window,
        protocol: Option<Protocol>,
    ) -> RepositoryResult<Vec<ProtocolDiscoveredTvl>> {
        let rows = sqlx::query_as!(
            ProtocolDiscoveredTvlRow,
            r#"
            SELECT DISTINCT ON (protocol)
                   protocol, bucket + INTERVAL '1 hour' AS "at!", pools, tvl_usd
              FROM analytic_protocol_tvl_discovered
             WHERE period = $2 AND bucket <= $1 AND ($3::TEXT IS NULL OR protocol = $3)
             ORDER BY protocol, bucket DESC
            "#,
            last,
            window.as_str(),
            bind_protocol_filter(protocol),
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter()
            .map(ProtocolDiscoveredTvl::try_from)
            .collect()
    }
}
