//! Postgres implementation of [`SignalRepository`].
//!
//! Backed by the `signals` hypertable (migration 022). Append-only: every
//! call is a plain multi-row INSERT — signals are immutable conclusions, so
//! there is no `ON CONFLICT` / UPSERT path.
//!
//! [`SignalRepository`]: yog_core::domain::SignalRepository

use crate::repositories::helper::map_sqlx_error;
use async_trait::async_trait;
use sqlx::{PgPool, QueryBuilder};
use yog_core::{
    RepositoryResult,
    domain::{Signal, SignalRepository},
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
}
