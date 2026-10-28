//! PostgreSQL implementation of [`ClaimTotalsLookup`]: the claims yog-analytic valued, summed by
//! protocol over a frozen period.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use yog_core::{
    RepositoryResult,
    domain::{ClaimTotals, ClaimTotalsLookup, FrozenPeriod, Protocol},
};

use crate::repositories::helper::{convert_bigdecimal_to_decimal, map_sqlx_error};

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
        let row = sqlx::query_as!(
            ClaimTotalsRow,
            r#"
            SELECT COUNT(*) AS "claims!", COUNT(amount_usd) AS "claims_valued!",
                   COALESCE(SUM(amount_usd), 0) AS "amount_usd!",
                   COUNT(DISTINCT owner) AS "owners!"
              FROM analytic_position_fee_claims
             WHERE protocol = $3 AND timestamp >= $1 AND timestamp < $2
            "#,
            period.from,
            period.as_of,
            protocol.as_str(),
        )
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        ClaimTotals::try_from(row)
    }

    async fn find_reward_claim_totals(
        &self,
        period: &FrozenPeriod,
        protocol: Protocol,
    ) -> RepositoryResult<ClaimTotals> {
        let row = sqlx::query_as!(
            ClaimTotalsRow,
            r#"
            SELECT COUNT(*) AS "claims!", COUNT(amount_usd) AS "claims_valued!",
                   COALESCE(SUM(amount_usd), 0) AS "amount_usd!",
                   COUNT(DISTINCT owner) AS "owners!"
              FROM analytic_reward_claims
             WHERE protocol = $3 AND timestamp >= $1 AND timestamp < $2
            "#,
            period.from,
            period.as_of,
            protocol.as_str(),
        )
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        ClaimTotals::try_from(row)
    }
}
