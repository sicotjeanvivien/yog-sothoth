//! The DAMM v2 branch: one tranche of its swaps, its pools' prices written
//! and the swaps valued at them in a single statement, then its liquidity moves;
//! its pools born; and the TVL of its pools discovered, seen first without their birth.

use chrono::{DateTime, Utc};
use sqlx::{Postgres, Transaction, postgres::types::PgInterval};
use yog_core::{
    RepositoryResult,
    domain::{Protocol, QUOTED_MINTS, RUN_GAP, RUN_ROTATION},
};

use super::{interval_of, windows};
use crate::repositories::helper::{convert_decimal_to_bigdecimal, map_sqlx_error};

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
        &QUOTED_MINTS.map(|mint| mint.to_string())[..],
    )
    .execute(&mut **tx)
    .await
    .map_err(map_sqlx_error)?;
    Ok(())
}

/// The rotation runs as of the tranches written through `through`, `from` the first of this cycle:
/// those not yet settled rewritten, a settled one written once with its hours.
pub(super) async fn write_rotation_runs(
    tx: &mut Transaction<'static, Postgres>,
    from: DateTime<Utc>,
    through: DateTime<Utc>,
    width: PgInterval,
) -> RepositoryResult<()> {
    sqlx::query!(
        "DELETE FROM analytic_rotation_runs WHERE protocol = $1 AND NOT settled",
        Protocol::MeteoraDammV2.as_str(),
    )
    .execute(&mut **tx)
    .await
    .map_err(map_sqlx_error)?;
    sqlx::query_file!(
        "src/repositories/analytic/meteora_damm_v2_rotation_runs.sql",
        Protocol::MeteoraDammV2.as_str(),
        from,
        through,
        convert_decimal_to_bigdecimal(RUN_ROTATION, "RUN_ROTATION")?,
        interval_of(RUN_GAP),
        width,
    )
    .execute(&mut **tx)
    .await
    .map_err(map_sqlx_error)?;
    Ok(())
}

/// The tranche's liquidity moves, each valued once at its pool's price in it.
pub(super) async fn value_liquidity_tranche(
    tx: &mut Transaction<'static, Postgres>,
    tranche: DateTime<Utc>,
    width: PgInterval,
) -> RepositoryResult<()> {
    sqlx::query_file!(
        "src/repositories/analytic/meteora_damm_v2_liquidity_tranche.sql",
        tranche,
        Protocol::MeteoraDammV2.as_str(),
        width,
    )
    .execute(&mut **tx)
    .await
    .map_err(map_sqlx_error)?;
    Ok(())
}

/// The moves of the hours `from..=through` summed, each hour rewritten whole, zero included, but
/// none before the first hour whose tranches were all valued with their moves.
/// `next_tranche` is the first tranche this cycle values.
pub(super) async fn upsert_liquidity_hourly(
    tx: &mut Transaction<'static, Postgres>,
    from: DateTime<Utc>,
    through: DateTime<Utc>,
    next_tranche: DateTime<Utc>,
) -> RepositoryResult<()> {
    sqlx::query!(
        r#"
        INSERT INTO analytic_protocol_liquidity_hourly (
            protocol, bucket, created_usd, added_usd, removed_usd, moves, moves_valued
        )
        SELECT $3, h.bucket,
               COALESCE(SUM(m.amount_usd) FILTER (WHERE m.kind = 'create'), 0),
               COALESCE(SUM(m.amount_usd) FILTER (WHERE m.kind = 'add'), 0),
               COALESCE(SUM(m.amount_usd) FILTER (WHERE m.kind = 'remove'), 0),
               COUNT(m.signature), COUNT(m.amount_usd)
          FROM generate_series($1::TIMESTAMPTZ, $2::TIMESTAMPTZ, INTERVAL '1 hour') AS h (bucket)
          LEFT JOIN analytic_liquidity_moves m
                 ON m.protocol = $3
                AND m.timestamp >= h.bucket AND m.timestamp < h.bucket + INTERVAL '1 hour'
         -- ⚠️ Before the first hour written, an hour starting before `next_tranche` holds
         -- tranches valued without their moves: it would read as an hour without any.
         WHERE h.bucket >= COALESCE(
                   (SELECT MIN(bucket) FROM analytic_protocol_liquidity_hourly WHERE protocol = $3),
                   date_trunc('hour', $4::TIMESTAMPTZ - INTERVAL '1 microsecond') + INTERVAL '1 hour')
         GROUP BY h.bucket
        ON CONFLICT (protocol, bucket) DO UPDATE SET
            created_usd  = EXCLUDED.created_usd,
            added_usd    = EXCLUDED.added_usd,
            removed_usd  = EXCLUDED.removed_usd,
            moves        = EXCLUDED.moves,
            moves_valued = EXCLUDED.moves_valued
        "#,
        from,
        through,
        Protocol::MeteoraDammV2.as_str(),
        next_tranche,
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
/// unobserved, counted among those `pool_current_tvl` holds: what a variation of the TVL would
/// take for a deposit. 0 for a window without.
pub(super) async fn measure_discovered_tvl(
    tx: &mut Transaction<'static, Postgres>,
    hour: DateTime<Utc>,
) -> RepositoryResult<()> {
    let (periods, hours) = windows();
    sqlx::query!(
        r#"
        INSERT INTO analytic_protocol_tvl_discovered (protocol, period, bucket, tvl_usd, pools)
        SELECT $2, w.period, $1, COALESCE(SUM(t.tvl_usd), 0), COUNT(t.pool_address)
          FROM UNNEST($3::TEXT[], $4::INT[]) AS w (period, hours)
          LEFT JOIN analytic_protocol_tvl s
                 ON s.protocol = $2 AND s.bucket = $1::TIMESTAMPTZ - make_interval(hours => w.hours)
          -- ⚠️ From the start point's last measure, not `from`: a pool first seen in between
          -- would be in neither TVL.
          LEFT JOIN pools p
                 ON p.protocol = $2
                AND p.first_seen_at >= COALESCE(s.measured_at,
                        $1::TIMESTAMPTZ + INTERVAL '1 hour' - make_interval(hours => w.hours))
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
