//! PostgreSQL implementation of [`PositionChangeCountsLookup`]: the position changes yog-analytic
//! counted, summed by protocol over a period, whole or by bucket; the last hour counted on the
//! events themselves.

use async_trait::async_trait;
use chrono::{DateTime, TimeDelta, Utc};
use sqlx::PgPool;
use yog_core::{
    RepositoryResult,
    domain::{
        FrozenPeriod, PositionChangeCounts, PositionChangeCountsLookup, Protocol, ProtocolBucket,
        TranchePeriod,
    },
};

use crate::repositories::helper::{interval_of, map_sqlx_error};

struct PositionChangeBucketRow {
    bucket_from: DateTime<Utc>,
    opened: i64,
    vesting_locks: i64,
    permanent_locks: i64,
    splits: i64,
    closed: i64,
}

impl PositionChangeBucketRow {
    fn into_bucket(self, protocol: Protocol) -> ProtocolBucket<PositionChangeCounts> {
        ProtocolBucket {
            protocol,
            from: self.bucket_from,
            value: PositionChangeCounts {
                opened: self.opened,
                vesting_locks: self.vesting_locks,
                permanent_locks: self.permanent_locks,
                splits: self.splits,
                closed: self.closed,
            },
        }
    }
}

pub struct PgPositionChangeCountsRepository {
    pool: PgPool,
}

impl PgPositionChangeCountsRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl PositionChangeCountsLookup for PgPositionChangeCountsRepository {
    async fn find_totals(
        &self,
        period: &FrozenPeriod,
        protocol: Protocol,
    ) -> RepositoryResult<Option<PositionChangeCounts>> {
        sqlx::query_as!(
            PositionChangeCounts,
            r#"
            SELECT SUM(opened)::BIGINT AS "opened!", SUM(vesting_locks)::BIGINT AS "vesting_locks!",
                   SUM(permanent_locks)::BIGINT AS "permanent_locks!",
                   SUM(splits)::BIGINT AS "splits!", SUM(closed)::BIGINT AS "closed!"
              FROM analytic_protocol_position_changes_hourly
             WHERE protocol = $3 AND bucket >= $1 AND bucket < $2
            HAVING COUNT(*) > 0
            "#,
            period.from,
            period.as_of,
            protocol.as_str(),
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)
    }

    async fn find_totals_in_tranches(
        &self,
        period: &TranchePeriod,
        protocol: Protocol,
    ) -> RepositoryResult<Option<PositionChangeCounts>> {
        let buckets = self.list_tranches(period, protocol).await?;
        if buckets.is_empty() {
            return Ok(None);
        }
        Ok(Some(buckets.into_iter().fold(
            PositionChangeCounts {
                opened: 0,
                vesting_locks: 0,
                permanent_locks: 0,
                splits: 0,
                closed: 0,
            },
            |sum, b| PositionChangeCounts {
                opened: sum.opened + b.value.opened,
                vesting_locks: sum.vesting_locks + b.value.vesting_locks,
                permanent_locks: sum.permanent_locks + b.value.permanent_locks,
                splits: sum.splits + b.value.splits,
                closed: sum.closed + b.value.closed,
            },
        )))
    }

    async fn list_hourly(
        &self,
        period: &FrozenPeriod,
        protocol: Protocol,
    ) -> RepositoryResult<Vec<ProtocolBucket<PositionChangeCounts>>> {
        self.list_frozen(period, TimeDelta::hours(1), protocol)
            .await
    }

    async fn list_buckets(
        &self,
        period: &FrozenPeriod,
        width: TimeDelta,
        protocol: Protocol,
    ) -> RepositoryResult<Vec<ProtocolBucket<PositionChangeCounts>>> {
        self.list_frozen(period, width, protocol).await
    }

    async fn list_tranches(
        &self,
        period: &TranchePeriod,
        protocol: Protocol,
    ) -> RepositoryResult<Vec<ProtocolBucket<PositionChangeCounts>>> {
        let rows = match protocol {
            Protocol::MeteoraDammV2 => sqlx::query_as!(
                PositionChangeBucketRow,
                r#"
            SELECT bucket AS "bucket_from!", opened AS "opened!", vesting_locks AS "vesting_locks!",
                   permanent_locks AS "permanent_locks!", splits AS "splits!", closed AS "closed!"
              FROM meteora_damm_v2_position_changes_every(
                       GREATEST($1::TIMESTAMPTZ, $3::TIMESTAMPTZ),
                       $2::TIMESTAMPTZ - INTERVAL '5 minutes', INTERVAL '5 minutes')
             ORDER BY bucket
            "#,
                period.from,
                period.as_of,
                period.covered_since,
            )
            .fetch_all(&self.pool)
            .await
            .map_err(map_sqlx_error)?,
            Protocol::MeteoraDlmm => Vec::new(),
        };

        Ok(rows.into_iter().map(|r| r.into_bucket(protocol)).collect())
    }
}

impl PgPositionChangeCountsRepository {
    /// The hours written by bucket of `width` from the period's start, ending at its `as_of`; a
    /// bucket starting before the coverage is left out, never summed partial.
    async fn list_frozen(
        &self,
        period: &FrozenPeriod,
        width: TimeDelta,
        protocol: Protocol,
    ) -> RepositoryResult<Vec<ProtocolBucket<PositionChangeCounts>>> {
        let rows = sqlx::query_as!(
            PositionChangeBucketRow,
            r#"
            SELECT b.bucket AS "bucket_from!",
                   COALESCE(SUM(h.opened), 0)::BIGINT AS "opened!",
                   COALESCE(SUM(h.vesting_locks), 0)::BIGINT AS "vesting_locks!",
                   COALESCE(SUM(h.permanent_locks), 0)::BIGINT AS "permanent_locks!",
                   COALESCE(SUM(h.splits), 0)::BIGINT AS "splits!",
                   COALESCE(SUM(h.closed), 0)::BIGINT AS "closed!"
              FROM generate_series($1::TIMESTAMPTZ, $2::TIMESTAMPTZ - $4::INTERVAL, $4::INTERVAL)
                       AS b (bucket)
              LEFT JOIN analytic_protocol_position_changes_hourly h
                     ON h.protocol = $5
                    AND h.bucket >= b.bucket AND h.bucket < b.bucket + $4::INTERVAL
             WHERE b.bucket >= $3
             GROUP BY b.bucket
            -- ⚠️ A bucket without any hour written is not a 0: the protocol is not counted there.
            HAVING COUNT(h.bucket) > 0
             ORDER BY b.bucket
            "#,
            period.from,
            period.as_of,
            period.covered_since,
            interval_of(width),
            protocol.as_str(),
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        Ok(rows.into_iter().map(|r| r.into_bucket(protocol)).collect())
    }
}
