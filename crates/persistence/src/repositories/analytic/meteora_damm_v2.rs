//! The DAMM v2 branch: one tranche of its swaps, its pools' prices written
//! and the swaps valued at them in a single statement; its pools born; and the
//! TVL of its pools discovered, seen first without their birth.

use chrono::{DateTime, Utc};
use sqlx::{Postgres, Transaction, postgres::types::PgInterval};
use yog_core::{RepositoryResult, domain::Protocol};

use super::windows;
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

/// The current TVL of the pools first seen in each window ending with `hour`, their birth
/// unobserved: what a variation of the TVL would take for a deposit. 0 for a window without.
pub(super) async fn measure_discovered_tvl(
    tx: &mut Transaction<'static, Postgres>,
    hour: DateTime<Utc>,
) -> RepositoryResult<()> {
    let (periods, hours) = windows();
    sqlx::query!(
        r#"
        INSERT INTO analytic_protocol_tvl_discovered (protocol, period, bucket, tvl_usd, pools)
        SELECT $2, w.period, $1, COALESCE(SUM(t.tvl_usd), 0), COUNT(p.pool_address)
          FROM UNNEST($3::TEXT[], $4::INT[]) AS w (period, hours)
          -- ⚠️ The window's start is the `from` of its variation: the hour's close, less the window.
          LEFT JOIN pools p
                 ON p.protocol = $2
                AND p.first_seen_at >= $1::TIMESTAMPTZ + INTERVAL '1 hour' - make_interval(hours => w.hours)
                AND NOT EXISTS (SELECT 1 FROM meteora_damm_v2_initialize_pool_events b
                                 WHERE b.pool_address = p.pool_address)
          LEFT JOIN pool_current_tvl t ON t.pool_address = p.pool_address
         GROUP BY w.period
        ON CONFLICT (protocol, period, bucket) DO UPDATE SET
            tvl_usd = EXCLUDED.tvl_usd,
            pools   = EXCLUDED.pools
        "#,
        hour,
        Protocol::MeteoraDammV2.as_str(),
        &periods,
        &hours,
    )
    .execute(&mut **tx)
    .await
    .map_err(map_sqlx_error)?;
    Ok(())
}
