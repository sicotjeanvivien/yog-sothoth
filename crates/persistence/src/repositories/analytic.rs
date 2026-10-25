//! Postgres implementation of [`AnalyticRepository`]: one transaction per cycle, whose only
//! per-protocol parts are each protocol's tranches, liquidity, rotation runs and births.
//!
//! [`AnalyticRepository`]: yog_core::domain::AnalyticRepository

mod meteora_damm_v2;

use crate::repositories::helper::map_sqlx_error;
use async_trait::async_trait;
use chrono::{DateTime, TimeDelta, Utc};
use sqlx::{PgExecutor, PgPool, Postgres, Transaction, postgres::types::PgInterval};
use yog_core::{
    RepositoryResult,
    domain::{
        AnalyticProgress, AnalyticRepository, CycleOutcome, CycleWrite, Protocol, TRANCHE, Window,
    },
};

#[derive(Clone)]
pub struct PgAnalyticRepository {
    pool: PgPool,
}

impl PgAnalyticRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl AnalyticRepository for PgAnalyticRepository {
    async fn progress(&self) -> RepositoryResult<AnalyticProgress> {
        progress(&self.pool).await
    }

    async fn write_cycle(&self, cycle: &CycleWrite) -> RepositoryResult<CycleOutcome> {
        let mut tx = self.pool.begin().await.map_err(map_sqlx_error)?;
        lock_out_other_writers(&mut tx).await?;
        let cycle = cycle.without_written(progress(&mut *tx).await?);

        let mut tranches_written = 0;
        let mut tranche = cycle.tranches_from;
        // ⚠️ One statement per tranche: a whole catch-up in one would outlast its timeout.
        while tranche <= cycle.tranches_through {
            meteora_damm_v2::value_tranche(&mut tx, tranche, tranche_interval()).await?;
            meteora_damm_v2::value_liquidity_tranche(&mut tx, tranche, tranche_interval()).await?;
            tranches_written += 1;
            tranche += TRANCHE;
        }
        record_tranches(
            &mut tx,
            cycle.tranches_from,
            cycle.tranches_through,
            cycle.at,
        )
        .await?;
        // ⚠️ Without a tranche written, `tranches_through` may lie before the last one written.
        if tranches_written > 0 {
            meteora_damm_v2::write_rotation_runs(
                &mut tx,
                cycle.tranches_from,
                cycle.tranches_through,
                tranche_interval(),
            )
            .await?;
        }
        let pool_hours = upsert_pool_hourly(&mut tx, cycle.from, cycle.through).await?;
        upsert_protocol_hourly(&mut tx, cycle.from, cycle.through).await?;
        meteora_damm_v2::upsert_liquidity_hourly(
            &mut tx,
            cycle.from,
            cycle.through,
            cycle.tranches_from,
        )
        .await?;
        meteora_damm_v2::upsert_births(&mut tx, cycle.from, cycle.through).await?;
        record_hours(&mut tx, cycle.from, cycle.through).await?;
        let hours_frozen = match cycle.freeze_through {
            Some(through) => {
                let frozen = freeze(&mut tx, cycle.from, through, cycle.at).await?;
                write_protocol_periods(&mut tx, cycle.from, through).await?;
                write_token_periods(&mut tx, through).await?;
                frozen
            }
            None => 0,
        };
        if let Some(hour) = cycle.tvl_hour {
            measure_tvl(&mut tx, hour, cycle.at).await?;
            meteora_damm_v2::measure_discovered_tvl(&mut tx, hour).await?;
        }

        tx.commit().await.map_err(map_sqlx_error)?;
        Ok(CycleOutcome {
            written: cycle,
            tranches_written,
            pool_hours,
            hours_frozen,
        })
    }
}

/// ⚠️ Before the progress is read: a concurrent cycle is then either committed
/// and seen, or not begun. Readers are not blocked.
async fn lock_out_other_writers(tx: &mut Transaction<'static, Postgres>) -> RepositoryResult<()> {
    sqlx::query!("LOCK TABLE analytic_hours IN EXCLUSIVE MODE")
        .execute(&mut **tx)
        .await
        .map_err(map_sqlx_error)?;
    Ok(())
}

/// [`TRANCHE`] as Postgres takes it.
fn tranche_interval() -> PgInterval {
    interval_of(TRANCHE)
}

/// A span of minutes as Postgres takes it.
fn interval_of(span: TimeDelta) -> PgInterval {
    PgInterval {
        months: 0,
        days: 0,
        microseconds: span
            .num_microseconds()
            .expect("a span of minutes fits in microseconds"),
    }
}

/// Read again inside the cycle's transaction: what a cycle committed since
/// this one was planned must stay out of it.
async fn progress<'e>(executor: impl PgExecutor<'e>) -> RepositoryResult<AnalyticProgress> {
    let row = sqlx::query!(
        r#"
        SELECT (SELECT MIN(bucket) FROM analytic_hours) AS "first_hour?: DateTime<Utc>",
               (SELECT MAX(bucket) FROM analytic_hours WHERE frozen_at IS NOT NULL)
                   AS "last_frozen?: DateTime<Utc>",
               (SELECT MAX(bucket) FROM analytic_tranches) AS "last_tranche?: DateTime<Utc>"
        "#
    )
    .fetch_one(executor)
    .await
    .map_err(map_sqlx_error)?;

    Ok(AnalyticProgress {
        first_hour: row.first_hour,
        last_frozen: row.last_frozen,
        last_tranche: row.last_tranche,
    })
}

/// ⚠️ A plain insert: a tranche written twice is a bug, and fails the cycle.
async fn record_tranches(
    tx: &mut Transaction<'static, Postgres>,
    from: DateTime<Utc>,
    through: DateTime<Utc>,
    at: DateTime<Utc>,
) -> RepositoryResult<()> {
    sqlx::query!(
        r#"
        INSERT INTO analytic_tranches (bucket, written_at, rotations)
        SELECT generate_series($1::TIMESTAMPTZ, $2::TIMESTAMPTZ, $4::INTERVAL), $3, true
        "#,
        from,
        through,
        at,
        tranche_interval(),
    )
    .execute(&mut **tx)
    .await
    .map_err(map_sqlx_error)?;
    Ok(())
}

/// Every protocol's valued tranches of the hours, summed per pool and hour.
/// Returns the facts upserted.
async fn upsert_pool_hourly(
    tx: &mut Transaction<'static, Postgres>,
    from: DateTime<Utc>,
    through: DateTime<Utc>,
) -> RepositoryResult<u64> {
    let upserted = sqlx::query!(
        r#"
        INSERT INTO analytic_pool_hourly (
            pool_address, bucket, protocol, swap_count, valued,
            volume_in_a_usd, volume_in_b_usd, fees_usd, lp_fees_usd,
            protocol_fees_usd, referral_fees_usd
        )
        SELECT pool_address, time_bucket('1 hour', bucket), protocol,
               SUM(swap_count), bool_and(valued),
               CASE WHEN bool_and(valued) THEN SUM(volume_in_a_usd) END,
               CASE WHEN bool_and(valued) THEN SUM(volume_in_b_usd) END,
               CASE WHEN bool_and(valued) THEN SUM(fees_usd) END,
               CASE WHEN bool_and(valued) THEN SUM(lp_fees_usd) END,
               CASE WHEN bool_and(valued) THEN SUM(protocol_fees_usd) END,
               CASE WHEN bool_and(valued) THEN SUM(referral_fees_usd) END
          FROM analytic_pool_5min
         WHERE bucket >= $1 AND bucket < $2::TIMESTAMPTZ + INTERVAL '1 hour'
         GROUP BY pool_address, time_bucket('1 hour', bucket), protocol
        ON CONFLICT (pool_address, bucket) DO UPDATE SET
            protocol          = EXCLUDED.protocol,
            swap_count        = EXCLUDED.swap_count,
            valued            = EXCLUDED.valued,
            volume_in_a_usd   = EXCLUDED.volume_in_a_usd,
            volume_in_b_usd   = EXCLUDED.volume_in_b_usd,
            fees_usd          = EXCLUDED.fees_usd,
            lp_fees_usd       = EXCLUDED.lp_fees_usd,
            protocol_fees_usd = EXCLUDED.protocol_fees_usd,
            referral_fees_usd = EXCLUDED.referral_fees_usd
        "#,
        from,
        through,
    )
    .execute(&mut **tx)
    .await
    .map_err(map_sqlx_error)?;
    Ok(upserted.rows_affected())
}

/// Every protocol's facts of the hours, summed per protocol.
async fn upsert_protocol_hourly(
    tx: &mut Transaction<'static, Postgres>,
    from: DateTime<Utc>,
    through: DateTime<Utc>,
) -> RepositoryResult<()> {
    sqlx::query!(
        r#"
        INSERT INTO analytic_protocol_hourly (
            protocol, bucket, volume_usd, fees_usd, lp_fees_usd,
            protocol_fees_usd, referral_fees_usd,
            swap_count, active_pools, active_pools_valued
        )
        SELECT protocol, bucket,
               SUM(volume_in_a_usd + volume_in_b_usd),
               SUM(fees_usd), SUM(lp_fees_usd),
               SUM(protocol_fees_usd), SUM(referral_fees_usd),
               SUM(swap_count), COUNT(*), COUNT(*) FILTER (WHERE valued)
          FROM analytic_pool_hourly
         WHERE bucket BETWEEN $1 AND $2
         GROUP BY protocol, bucket
        ON CONFLICT (protocol, bucket) DO UPDATE SET
            volume_usd          = EXCLUDED.volume_usd,
            fees_usd            = EXCLUDED.fees_usd,
            lp_fees_usd         = EXCLUDED.lp_fees_usd,
            protocol_fees_usd   = EXCLUDED.protocol_fees_usd,
            referral_fees_usd   = EXCLUDED.referral_fees_usd,
            swap_count          = EXCLUDED.swap_count,
            active_pools        = EXCLUDED.active_pools,
            active_pools_valued = EXCLUDED.active_pools_valued
        "#,
        from,
        through,
    )
    .execute(&mut **tx)
    .await
    .map_err(map_sqlx_error)?;
    Ok(())
}

async fn record_hours(
    tx: &mut Transaction<'static, Postgres>,
    from: DateTime<Utc>,
    through: DateTime<Utc>,
) -> RepositoryResult<()> {
    sqlx::query!(
        r#"
        INSERT INTO analytic_hours (bucket)
        SELECT generate_series($1::TIMESTAMPTZ, $2::TIMESTAMPTZ, INTERVAL '1 hour')
        ON CONFLICT (bucket) DO NOTHING
        "#,
        from,
        through,
    )
    .execute(&mut **tx)
    .await
    .map_err(map_sqlx_error)?;
    Ok(())
}

async fn freeze(
    tx: &mut Transaction<'static, Postgres>,
    from: DateTime<Utc>,
    through: DateTime<Utc>,
    at: DateTime<Utc>,
) -> RepositoryResult<u64> {
    let frozen = sqlx::query!(
        r#"
        UPDATE analytic_hours
           SET frozen_at = $3
         WHERE bucket BETWEEN $1 AND $2
           AND frozen_at IS NULL
        "#,
        from,
        through,
        at,
    )
    .execute(&mut **tx)
    .await
    .map_err(map_sqlx_error)?;
    Ok(frozen.rows_affected())
}

/// [`Window::ALL`] as the two arrays the period queries unnest.
fn windows() -> (Vec<String>, Vec<i32>) {
    Window::ALL
        .iter()
        .map(|window| (window.as_str().to_owned(), window.hours()))
        .unzip()
}

/// The distinct pools of every protocol and window ending at each hour `from..=through` frozen;
/// a protocol without any gets 0, so a missing row means a window never counted.
async fn write_protocol_periods(
    tx: &mut Transaction<'static, Postgres>,
    from: DateTime<Utc>,
    through: DateTime<Utc>,
) -> RepositoryResult<()> {
    let (periods, hours) = windows();
    let protocols: Vec<&str> = Protocol::all().iter().map(|p| p.as_str()).collect();
    sqlx::query!(
        r#"
        -- ⚠️ Grouped per pool, then counted: a COUNT(DISTINCT) sorts, and spills.
        WITH frozen AS (
            SELECT generate_series($1::TIMESTAMPTZ, $2::TIMESTAMPTZ, INTERVAL '1 hour')
                       AS last_hour
        ),
        by_pool AS (
            SELECT h.last_hour, w.period, f.protocol, f.pool_address
              FROM frozen h
             CROSS JOIN UNNEST($3::TEXT[], $4::INT[]) AS w (period, hours)
              JOIN analytic_pool_hourly f
                ON f.bucket > h.last_hour - make_interval(hours => w.hours)
               AND f.bucket <= h.last_hour
             GROUP BY h.last_hour, w.period, f.protocol, f.pool_address
        )
        INSERT INTO analytic_protocol_periods (protocol, period, bucket, active_pools)
        SELECT pr.protocol, w.period, h.last_hour, COUNT(b.pool_address)
          FROM frozen h
         CROSS JOIN UNNEST($5::TEXT[]) AS pr (protocol)
         CROSS JOIN UNNEST($3::TEXT[]) AS w (period)
          LEFT JOIN by_pool b
                 ON b.last_hour = h.last_hour AND b.protocol = pr.protocol AND b.period = w.period
         GROUP BY h.last_hour, pr.protocol, w.period
        "#,
        from,
        through,
        &periods,
        &hours,
        &protocols as &[&str],
    )
    .execute(&mut **tx)
    .await
    .map_err(map_sqlx_error)?;
    Ok(())
}

/// Every token of every window ending at `last_hour`, in place of the last
/// ones: the volume of the pools that hold it, a swap counting for both sides.
async fn write_token_periods(
    tx: &mut Transaction<'static, Postgres>,
    last_hour: DateTime<Utc>,
) -> RepositoryResult<()> {
    sqlx::query!("DELETE FROM analytic_token_periods")
        .execute(&mut **tx)
        .await
        .map_err(map_sqlx_error)?;
    let (periods, hours) = windows();
    sqlx::query!(
        r#"
        WITH by_pool AS (
            SELECT w.period, f.protocol, f.pool_address,
                   SUM(f.volume_in_a_usd + f.volume_in_b_usd) FILTER (WHERE f.valued)
                       AS volume_usd,
                   COUNT(*) AS pool_hours, COUNT(*) FILTER (WHERE f.valued) AS pool_hours_valued
              FROM UNNEST($2::TEXT[], $3::INT[]) AS w (period, hours)
              JOIN analytic_pool_hourly f
                ON f.bucket > $1::TIMESTAMPTZ - make_interval(hours => w.hours)
               AND f.bucket <= $1
             GROUP BY w.period, f.protocol, f.pool_address
        )
        INSERT INTO analytic_token_periods (
            period, mint, bucket, volume_usd, pools, pool_hours, pool_hours_valued
        )
        -- ⚠️ Rounded: the page cursor, a Decimal, must hold the volume exactly.
        SELECT b.period, m.mint, $1, ROUND(COALESCE(SUM(b.volume_usd), 0), 6), COUNT(*),
               SUM(b.pool_hours), SUM(b.pool_hours_valued)
          FROM by_pool b
          -- ⚠️ The mints joined to the pools once summed, not to each pool-hour.
          JOIN pools p ON p.pool_address = b.pool_address AND p.protocol = b.protocol
         CROSS JOIN LATERAL (VALUES (p.token_a_mint), (p.token_b_mint)) AS m (mint)
         WHERE m.mint IS NOT NULL
         GROUP BY b.period, m.mint
        "#,
        last_hour,
        &periods,
        &hours,
    )
    .execute(&mut **tx)
    .await
    .map_err(map_sqlx_error)?;
    Ok(())
}

/// The current TVL of every protocol, by the rule of `pool_current_tvl`.
async fn measure_tvl(
    tx: &mut Transaction<'static, Postgres>,
    hour: DateTime<Utc>,
    at: DateTime<Utc>,
) -> RepositoryResult<()> {
    sqlx::query!(
        r#"
        INSERT INTO analytic_protocol_tvl
            (protocol, bucket, tvl_usd, pools_priced, pools_unvalued, measured_at)
        SELECT p.protocol, $1, SUM(t.tvl_usd),
               COUNT(*) FILTER (WHERE t.tvl_usd IS NOT NULL),
               COUNT(*) FILTER (WHERE t.tvl_usd IS NULL), $2
          FROM pool_current_tvl t
          JOIN pools p ON p.pool_address = t.pool_address
         GROUP BY p.protocol
        ON CONFLICT (protocol, bucket) DO UPDATE SET
            tvl_usd        = EXCLUDED.tvl_usd,
            pools_priced   = EXCLUDED.pools_priced,
            pools_unvalued = EXCLUDED.pools_unvalued,
            measured_at    = EXCLUDED.measured_at
        "#,
        hour,
        at,
    )
    .execute(&mut **tx)
    .await
    .map_err(map_sqlx_error)?;
    Ok(())
}
