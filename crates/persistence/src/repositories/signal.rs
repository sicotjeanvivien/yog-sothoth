//! Postgres implementation of [`SignalRepository`].
//!
//! Backed by the `signals` hypertable (migration 022). Append-only: every
//! call is a plain multi-row INSERT — signals are immutable conclusions, so
//! there is no `ON CONFLICT` / UPSERT path.
//!
//! [`SignalRepository`]: yog_core::domain::SignalRepository

use std::collections::HashMap;
use std::str::FromStr;

use crate::repositories::helper::{convert_string_to_pubkey, map_sqlx_error};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use solana_pubkey::Pubkey;
use sqlx::{PgPool, QueryBuilder};
use yog_core::{
    RepositoryError, RepositoryResult,
    domain::{Severity, Signal, SignalRepository},
};

/// Postgres-backed signal repository.
#[derive(Clone)]
pub struct PgSignalRepository {
    pool: PgPool,
}

impl PgSignalRepository {
    /// Build the repository over a shared connection pool.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl SignalRepository for PgSignalRepository {
    async fn insert_batch(&self, signals: &[Signal]) -> RepositoryResult<()> {
        if signals.is_empty() {
            return Ok(());
        }

        // Variable-arity bulk insert: QueryBuilder, since the `query!` macros
        // cannot generate a runtime-sized VALUES list (same reasoning as the
        // token_prices batch insert).
        let mut builder = QueryBuilder::new(
            "INSERT INTO signals \
             (detector, protocol, pool_address, severity, value, threshold, message, triggered_at) ",
        );

        builder.push_values(signals, |mut row, signal| {
            row.push_bind(signal.detector.as_str())
                .push_bind(signal.protocol.as_str())
                .push_bind(signal.pool_address.to_string())
                .push_bind(signal.severity.as_str())
                .push_bind(signal.value)
                .push_bind(signal.threshold)
                .push_bind(signal.message.as_deref())
                .push_bind(signal.triggered_at);
        });

        builder
            .build()
            .execute(&self.pool)
            .await
            .map_err(map_sqlx_error)?;

        Ok(())
    }

    async fn latest_severity_by_pool(
        &self,
        detector: &str,
        since: DateTime<Utc>,
    ) -> RepositoryResult<HashMap<Pubkey, Severity>> {
        // DISTINCT ON keeps the most recent row per pool. Because the engine's
        // dedup only ever emits an escalating severity within a window, the
        // most recent severity is also the max — the current suppression state.
        let rows = sqlx::query!(
            r#"
            SELECT DISTINCT ON (pool_address)
                pool_address AS "pool_address!",
                severity     AS "severity!"
            FROM signals
            WHERE detector = $1 AND triggered_at > $2
            ORDER BY pool_address, triggered_at DESC
            "#,
            detector,
            since,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        let mut out = HashMap::with_capacity(rows.len());
        for row in rows {
            let pool = convert_string_to_pubkey(row.pool_address, "pool_address")?;
            let severity = Severity::from_str(&row.severity).map_err(|_| {
                RepositoryError::Integrity(format!("invalid severity `{}`", row.severity))
            })?;
            out.insert(pool, severity);
        }
        Ok(out)
    }
}
