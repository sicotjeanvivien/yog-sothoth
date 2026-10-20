//! The DAMM v2 branch: one tranche of its swaps, its pools' prices written
//! and the swaps valued at them in a single statement; and its pools born.

use chrono::{DateTime, Utc};
use sqlx::{Postgres, Transaction, postgres::types::PgInterval};
use yog_core::{RepositoryResult, domain::Protocol};

use crate::repositories::helper::map_sqlx_error;

pub(super) async fn value_tranche(
    tx: &mut Transaction<'static, Postgres>,
    tranche: DateTime<Utc>,
    width: PgInterval,
) -> RepositoryResult<()> {
    sqlx::query_file!(
        "src/repositories/analytic/meteora_damm_v2_tranche.sql",
        tranche,
        Protocol::MeteoraDammV2.as_str(),
        width,
    )
    .execute(&mut **tx)
    .await
    .map_err(map_sqlx_error)?;
    Ok(())
}

/// The pools initialized in the hours `from..=through`, each hour rewritten whole.
pub(super) async fn upsert_births(
    tx: &mut Transaction<'static, Postgres>,
    from: DateTime<Utc>,
    through: DateTime<Utc>,
) -> RepositoryResult<()> {
    sqlx::query!(
        r#"
        INSERT INTO analytic_protocol_births_hourly (protocol, bucket, births)
        SELECT $3, time_bucket('1 hour', timestamp), COUNT(DISTINCT pool_address)
          FROM meteora_damm_v2_initialize_pool_events
         WHERE timestamp >= $1 AND timestamp < $2::TIMESTAMPTZ + INTERVAL '1 hour'
         GROUP BY time_bucket('1 hour', timestamp)
        ON CONFLICT (protocol, bucket) DO UPDATE SET births = EXCLUDED.births
        "#,
        from,
        through,
        Protocol::MeteoraDammV2.as_str(),
    )
    .execute(&mut **tx)
    .await
    .map_err(map_sqlx_error)?;
    Ok(())
}
