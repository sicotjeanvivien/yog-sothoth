//! Postgres implementation of `IngestCoverageRepository`, over `ingest_coverage`.

use async_trait::async_trait;
use sqlx::PgPool;
use yog_core::{
    RepositoryResult,
    domain::{IngestCoverageRepository, ObservedSlots},
};

use crate::repositories::helper::{convert_u64_to_i64, map_sqlx_error};

#[derive(Clone)]
pub struct PgIngestCoverageRepository {
    pool: PgPool,
}

impl PgIngestCoverageRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl IngestCoverageRepository for PgIngestCoverageRepository {
    async fn record(&self, observed: &ObservedSlots) -> RepositoryResult<()> {
        let range = observed.range;
        let from = convert_u64_to_i64(range.from(), "observed_from")?;
        let to = convert_u64_to_i64(range.to(), "observed_to")?;

        // ⚠️ Each end moves only outwards: a late write from a stale reading must not hand back
        // slots a later one declared observed.
        sqlx::query!(
            r#"
            INSERT INTO ingest_coverage AS c
                (protocol, session_started_at, observed_from, observed_to,
                 observed_from_at, observed_to_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, now())
            ON CONFLICT (protocol, session_started_at) DO UPDATE
            SET observed_from    = LEAST(c.observed_from, EXCLUDED.observed_from),
                observed_from_at = CASE WHEN EXCLUDED.observed_from < c.observed_from
                                        THEN EXCLUDED.observed_from_at ELSE c.observed_from_at END,
                observed_to      = GREATEST(c.observed_to, EXCLUDED.observed_to),
                observed_to_at   = CASE WHEN EXCLUDED.observed_to > c.observed_to
                                        THEN EXCLUDED.observed_to_at ELSE c.observed_to_at END,
                updated_at       = now()
            "#,
            observed.protocol.as_str(),
            observed.session_started_at,
            from,
            to,
            range.from_at(),
            range.to_at(),
        )
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        Ok(())
    }
}
