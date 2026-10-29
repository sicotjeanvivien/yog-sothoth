//! PostgreSQL implementation of [`ClaimTotalsLookup`]: the claims yog-analytic valued, summed by
//! protocol over a period, whole or by bucket.

use async_trait::async_trait;
use chrono::{DateTime, TimeDelta, Utc};
use sqlx::PgPool;
use yog_core::{
    RepositoryResult,
    domain::{
        ClaimKind, ClaimTotals, ClaimTotalsLookup, FrozenPeriod, Protocol, ProtocolBucket,
        TranchePeriod,
    },
};

use crate::repositories::helper::{convert_bigdecimal_to_decimal, interval_of, map_sqlx_error};

struct ClaimTotalsRow {
    claims: i64,
    claims_valued: i64,
    amount_usd: sqlx::types::BigDecimal,
    owners: i64,
}

impl TryFrom<ClaimTotalsRow> for ClaimTotals {
    type Error = yog_core::RepositoryError;

    fn try_from(row: ClaimTotalsRow) -> Result<Self, Self::Error> {
        Ok(Self {
            claims: row.claims,
            claims_valued: row.claims_valued,
            amount_usd: convert_bigdecimal_to_decimal(row.amount_usd, "amount_usd")?,
            owners: row.owners,
        })
    }
}

struct ClaimBucketRow {
    bucket_from: DateTime<Utc>,
    claims: i64,
    claims_valued: i64,
    amount_usd: sqlx::types::BigDecimal,
    owners: i64,
}

/// The bounds of a series: buckets of `width` from `from` to `as_of`, none before `covered_since`.
struct SeriesBounds {
    from: DateTime<Utc>,
    as_of: DateTime<Utc>,
    covered_since: DateTime<Utc>,
    width: TimeDelta,
}

pub struct PgClaimTotalsRepository {
    pool: PgPool,
}

impl PgClaimTotalsRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ClaimTotalsLookup for PgClaimTotalsRepository {
    async fn find_measured_since(
        &self,
        protocol: Protocol,
    ) -> RepositoryResult<Option<DateTime<Utc>>> {
        match protocol {
            // ⚠️ The tranche ledger is not per protocol: only DAMM v2's writer values claims.
            Protocol::MeteoraDammV2 => {
                let row = sqlx::query!(
                    r#"
                    SELECT MIN(bucket) AS "since?: DateTime<Utc>"
                      FROM analytic_tranches
                     WHERE claims
                    "#,
                )
                .fetch_one(&self.pool)
                .await
                .map_err(map_sqlx_error)?;
                Ok(row.since)
            }
            Protocol::MeteoraDlmm => Ok(None),
        }
    }

    async fn find_position_fee_claim_totals(
        &self,
        period: &FrozenPeriod,
        protocol: Protocol,
    ) -> RepositoryResult<ClaimTotals> {
        self.totals(ClaimKind::PositionFee, period.from, period.as_of, protocol)
            .await
    }

    async fn find_reward_claim_totals(
        &self,
        period: &FrozenPeriod,
        protocol: Protocol,
    ) -> RepositoryResult<ClaimTotals> {
        self.totals(ClaimKind::Reward, period.from, period.as_of, protocol)
            .await
    }

    async fn find_totals_in_tranches(
        &self,
        kind: ClaimKind,
        period: &TranchePeriod,
        protocol: Protocol,
    ) -> RepositoryResult<ClaimTotals> {
        self.totals(kind, period.from, period.as_of, protocol).await
    }

    async fn list_hourly(
        &self,
        kind: ClaimKind,
        period: &FrozenPeriod,
        protocol: Protocol,
    ) -> RepositoryResult<Vec<ProtocolBucket<ClaimTotals>>> {
        self.list_series(kind, frozen_bounds(period, TimeDelta::hours(1)), protocol)
            .await
    }

    async fn list_daily(
        &self,
        kind: ClaimKind,
        period: &FrozenPeriod,
        protocol: Protocol,
    ) -> RepositoryResult<Vec<ProtocolBucket<ClaimTotals>>> {
        self.list_series(kind, frozen_bounds(period, TimeDelta::days(1)), protocol)
            .await
    }

    async fn list_tranches(
        &self,
        kind: ClaimKind,
        period: &TranchePeriod,
        protocol: Protocol,
    ) -> RepositoryResult<Vec<ProtocolBucket<ClaimTotals>>> {
        let bounds = SeriesBounds {
            from: period.from,
            as_of: period.as_of,
            covered_since: period.covered_since,
            width: TimeDelta::minutes(5),
        };
        self.list_series(kind, bounds, protocol).await
    }
}

fn frozen_bounds(period: &FrozenPeriod, width: TimeDelta) -> SeriesBounds {
    SeriesBounds {
        from: period.from,
        as_of: period.as_of,
        covered_since: period.covered_since,
        width,
    }
}

impl PgClaimTotalsRepository {
    /// `kind`'s claims in `from..as_of`, their owners distinct.
    async fn totals(
        &self,
        kind: ClaimKind,
        from: DateTime<Utc>,
        as_of: DateTime<Utc>,
        protocol: Protocol,
    ) -> RepositoryResult<ClaimTotals> {
        let row = match kind {
            ClaimKind::PositionFee => {
                sqlx::query_as!(
                    ClaimTotalsRow,
                    r#"
                    SELECT COUNT(*) AS "claims!", COUNT(amount_usd) AS "claims_valued!",
                           COALESCE(SUM(amount_usd), 0) AS "amount_usd!",
                           COUNT(DISTINCT owner) AS "owners!"
                      FROM analytic_position_fee_claims
                     WHERE protocol = $3 AND timestamp >= $1 AND timestamp < $2
                    "#,
                    from,
                    as_of,
                    protocol.as_str(),
                )
                .fetch_one(&self.pool)
                .await
            }
            ClaimKind::Reward => {
                sqlx::query_as!(
                    ClaimTotalsRow,
                    r#"
                    SELECT COUNT(*) AS "claims!", COUNT(amount_usd) AS "claims_valued!",
                           COALESCE(SUM(amount_usd), 0) AS "amount_usd!",
                           COUNT(DISTINCT owner) AS "owners!"
                      FROM analytic_reward_claims
                     WHERE protocol = $3 AND timestamp >= $1 AND timestamp < $2
                    "#,
                    from,
                    as_of,
                    protocol.as_str(),
                )
                .fetch_one(&self.pool)
                .await
            }
        }
        .map_err(map_sqlx_error)?;

        ClaimTotals::try_from(row)
    }

    /// `kind`'s claims by bucket of `bounds`, 0 in a bucket without; a bucket starting before the
    /// coverage is left out, never summed partial.
    async fn list_series(
        &self,
        kind: ClaimKind,
        bounds: SeriesBounds,
        protocol: Protocol,
    ) -> RepositoryResult<Vec<ProtocolBucket<ClaimTotals>>> {
        let rows = match kind {
            ClaimKind::PositionFee => {
                sqlx::query_as!(
                    ClaimBucketRow,
                    r#"
                    SELECT b.bucket AS "bucket_from!", COUNT(c.signature) AS "claims!",
                           COUNT(c.amount_usd) AS "claims_valued!",
                           COALESCE(SUM(c.amount_usd), 0) AS "amount_usd!",
                           COUNT(DISTINCT c.owner) AS "owners!"
                      FROM generate_series($1::TIMESTAMPTZ, $2::TIMESTAMPTZ - $4::INTERVAL,
                                           $4::INTERVAL) AS b (bucket)
                      LEFT JOIN analytic_position_fee_claims c
                             ON c.protocol = $5
                            AND c.timestamp >= b.bucket AND c.timestamp < b.bucket + $4::INTERVAL
                            -- ⚠️ Constant bounds too: the planner excludes chunks on them only.
                            AND c.timestamp >= $1 AND c.timestamp < $2
                     WHERE b.bucket >= $3
                     GROUP BY b.bucket
                     ORDER BY b.bucket
                    "#,
                    bounds.from,
                    bounds.as_of,
                    bounds.covered_since,
                    interval_of(bounds.width),
                    protocol.as_str(),
                )
                .fetch_all(&self.pool)
                .await
            }
            ClaimKind::Reward => {
                sqlx::query_as!(
                    ClaimBucketRow,
                    r#"
                    SELECT b.bucket AS "bucket_from!", COUNT(c.signature) AS "claims!",
                           COUNT(c.amount_usd) AS "claims_valued!",
                           COALESCE(SUM(c.amount_usd), 0) AS "amount_usd!",
                           COUNT(DISTINCT c.owner) AS "owners!"
                      FROM generate_series($1::TIMESTAMPTZ, $2::TIMESTAMPTZ - $4::INTERVAL,
                                           $4::INTERVAL) AS b (bucket)
                      LEFT JOIN analytic_reward_claims c
                             ON c.protocol = $5
                            AND c.timestamp >= b.bucket AND c.timestamp < b.bucket + $4::INTERVAL
                            AND c.timestamp >= $1 AND c.timestamp < $2
                     WHERE b.bucket >= $3
                     GROUP BY b.bucket
                     ORDER BY b.bucket
                    "#,
                    bounds.from,
                    bounds.as_of,
                    bounds.covered_since,
                    interval_of(bounds.width),
                    protocol.as_str(),
                )
                .fetch_all(&self.pool)
                .await
            }
        }
        .map_err(map_sqlx_error)?;

        rows.into_iter()
            .map(|row| {
                Ok(ProtocolBucket {
                    protocol,
                    from: row.bucket_from,
                    value: ClaimTotals {
                        claims: row.claims,
                        claims_valued: row.claims_valued,
                        amount_usd: convert_bigdecimal_to_decimal(row.amount_usd, "amount_usd")?,
                        owners: row.owners,
                    },
                })
            })
            .collect()
    }
}
