//! DLMM pool-properties satellite repository (migration 039).
//!
//! Holds the pool properties that only exist for the Liquidity Book product,
//! kept out of the cross-protocol `pools` registry so no protocol carries NULL
//! columns for another protocol's concepts.
//!
//! One `Pg` type, two **generic** traits, one per consumer — the shape cp-amm's
//! equivalent converged on after #82/#83:
//!
//! - [`PoolAccountResolver`] — yog-context's enrichment queue and its write.
//! - [`PoolPropertiesLookup`] — the api's read for the pool detail sheet.
//!
//! Neither names this protocol in its signature: the queue below filters on it,
//! and the write accepts only this protocol's variant.
//!
//! # One table, one writer
//!
//! This repository writes **this satellite and nothing else**. The neutral half
//! of the same account read — the mints and `fee_bps` — goes through
//! [`yog_core::domain::PoolRepository::set_registry_properties`], so adding this
//! second satellite adds no writer to `pools`.

mod rows;

use async_trait::async_trait;
use solana_pubkey::Pubkey;
use sqlx::PgPool;
use yog_core::{
    RepositoryResult,
    domain::{
        MeteoraDlmmPoolProperties, PoolAccountProperties, PoolAccountResolver, PoolProperties,
        PoolPropertiesLookup, Protocol,
    },
};

use crate::repositories::helper::{convert_string_to_pubkey, map_sqlx_error};
use rows::MeteoraDlmmPoolPropertiesRow;

pub struct PgMeteoraDlmmPoolPropertiesRepository {
    pool: PgPool,
}

impl PgMeteoraDlmmPoolPropertiesRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl PoolPropertiesLookup for PgMeteoraDlmmPoolPropertiesRepository {
    fn protocol(&self) -> Protocol {
        Protocol::MeteoraDlmm
    }

    /// No protocol predicate needed here, unlike
    /// [`PoolAccountResolver::list_unresolved`]: this reads the satellite by
    /// primary key, and a pool of another protocol simply has no row in it.
    async fn find_by_pool(
        &self,
        pool_address: &Pubkey,
    ) -> RepositoryResult<Option<PoolProperties>> {
        let row = sqlx::query_as!(
            MeteoraDlmmPoolPropertiesRow,
            r#"
            SELECT pool_address, bin_step, base_factor, base_fee_power_factor,
                   variable_fee_control, max_volatility_accumulator, protocol_share
            FROM meteora_dlmm_pool_properties
            WHERE pool_address = $1
            "#,
            pool_address.to_string(),
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        row.map(MeteoraDlmmPoolProperties::try_from)
            .transpose()
            .map(|p| p.map(PoolProperties::MeteoraDlmm))
    }
}

#[async_trait]
impl PoolAccountResolver for PgMeteoraDlmmPoolPropertiesRepository {
    fn protocol(&self) -> Protocol {
        Protocol::MeteoraDlmm
    }

    /// The protocol predicate is **required**, not decorative — see the trait
    /// doc. Joining this satellite does not scope the query on its own: "no
    /// satellite row yet" is one of the conditions that makes a pool a
    /// candidate, and that is true of every cp-amm pool, forever. Without
    /// `p.protocol` this queue would propose the entire cp-amm catalogue, whose
    /// accounts this resolver cannot store — so they would never resolve, never
    /// leave the queue, and starve every DLMM pool behind them.
    ///
    /// # Two reasons a pool is proposed
    ///
    /// **Never resolved** — a NULL column. And **stale**: `p.needs_refresh`,
    /// raised by the indexer when an event changed a property it does not write
    /// itself. The flag is cleared by the *registry* write, which the caller
    /// issues last, so a pool re-enters the queue until the whole refresh lands.
    ///
    /// # Why `bin_step` alone stands for the satellite
    ///
    /// cp-amm needs care here: its `base_fee_kind` can be legitimately NULL on a
    /// successfully resolved row, so testing it would re-propose those pools
    /// forever. DLMM has no such field — every column comes from one read of
    /// fixed-offset integers with no open enum to recognise, so they are NULL
    /// together or not at all. `bin_step IS NULL` is therefore an exact test for
    /// "never resolved", and listing the other five would be redundant rather
    /// than safer.
    async fn list_unresolved(&self, limit: i64) -> RepositoryResult<Vec<Pubkey>> {
        let rows = sqlx::query!(
            r#"
            SELECT p.pool_address
            FROM pools p
            LEFT JOIN meteora_dlmm_pool_properties props
                   ON props.pool_address = p.pool_address
            WHERE p.protocol = $2
              AND (p.needs_refresh
                OR p.token_a_mint IS NULL OR p.token_b_mint IS NULL OR p.fee_bps IS NULL
                OR props.pool_address IS NULL
                OR props.bin_step     IS NULL)
            ORDER BY p.first_seen_at
            LIMIT $1
            "#,
            limit,
            Protocol::MeteoraDlmm.as_str(),
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter()
            .map(|r| convert_string_to_pubkey(r.pool_address, "pool_address"))
            .collect()
    }

    /// One write, to this satellite only.
    ///
    /// The caller sequences this **first, the registry last**, because the
    /// registry write is what lowers `pools.needs_refresh`. A failure here
    /// therefore leaves the flag raised and the pool queued, rather than
    /// stranding a half-refreshed pool.
    ///
    /// An upsert, not an `UPDATE`: the satellite row is not created with the
    /// pool, so an `UPDATE` would silently do nothing on first resolution.
    ///
    /// Plain `EXCLUDED` on every column, with no `COALESCE` anywhere — cp-amm
    /// needs one to stop an unmappable fee mode erasing a known value, but a
    /// successful `LbPair` decode always carries all six fields, so there is
    /// never a NULL to guard against.
    ///
    /// Rejects a payload of another protocol rather than silently doing nothing:
    /// the worker routes by [`PoolAccountProperties::protocol`], so a mismatch
    /// here is a wiring bug, not a runtime condition.
    ///
    /// It is not the only guard, and it is not the last one. It answers "is this
    /// payload mine?"; the worker screens "is this *pool* mine" upstream
    /// (`context/src/workers/pool_account.rs` skips a pool whose decoded account
    /// disagrees with the queue's protocol); and since migration 040 the schema
    /// backs both — a generated `protocol` column and a composite foreign key
    /// onto `pools (pool_address, protocol)`.
    ///
    /// So a DLMM payload aimed at a cp-amm pool is unreachable through the
    /// worker *and* refused by the database. The constraint is there for what
    /// the guards cannot cover: a refactor, a second writer, or a hand-run
    /// repair. It surfaces as [`yog_core::RepositoryError::Conflict`], so any
    /// such caller inherits skip-and-log rather than a silent success.
    async fn set_pool_account(
        &self,
        pool_address: &Pubkey,
        properties: &PoolAccountProperties,
    ) -> RepositoryResult<()> {
        let PoolAccountProperties::MeteoraDlmm(properties) = properties else {
            return Err(yog_core::RepositoryError::Integrity(format!(
                "expected DLMM pool properties, got {:?}",
                properties.protocol()
            )));
        };

        // Widening to the signed SQL types is always lossless: u16 -> INTEGER,
        // u8 -> SMALLINT, u32 -> BIGINT. Migration 039 explains the widths.
        sqlx::query!(
            r#"
            INSERT INTO meteora_dlmm_pool_properties
                (pool_address, bin_step, base_factor, base_fee_power_factor,
                 variable_fee_control, max_volatility_accumulator, protocol_share)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            ON CONFLICT (pool_address) DO UPDATE
                SET bin_step                   = EXCLUDED.bin_step,
                    base_factor                = EXCLUDED.base_factor,
                    base_fee_power_factor      = EXCLUDED.base_fee_power_factor,
                    variable_fee_control       = EXCLUDED.variable_fee_control,
                    max_volatility_accumulator = EXCLUDED.max_volatility_accumulator,
                    protocol_share             = EXCLUDED.protocol_share
            "#,
            pool_address.to_string(),
            i32::from(properties.bin_step),
            i32::from(properties.base_factor),
            i16::from(properties.base_fee_power_factor),
            i64::from(properties.variable_fee_control),
            i64::from(properties.max_volatility_accumulator),
            i32::from(properties.protocol_share),
        )
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        Ok(())
    }
}
