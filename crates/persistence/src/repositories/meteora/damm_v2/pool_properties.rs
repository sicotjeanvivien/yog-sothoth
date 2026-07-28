//! DAMM v2 pool-properties satellite repository (migration 036).
//!
//! Holds the pool properties that only exist for cp-amm, kept out of the
//! cross-protocol `pools` registry so no protocol carries NULL columns for
//! another protocol's concepts.
//!
//! One `Pg` type, two traits, one per consumer — the same pattern as
//! `PgPoolRepository` (write side / read side):
//!
//! - [`MeteoraDammV2PoolPropertiesRepository`] — the indexer writes the fee shape
//!   (`base_fee_kind`, `has_dynamic_fee`) decoded from the genesis
//!   `InitializePool` blob; the api reads the whole row for the detail sheet.
//! - [`PoolAccountResolver`] — yog-context's enrichment queue and its two-table
//!   write. Generic trait, per-protocol implementation: the queue below filters
//!   on this protocol, the write accepts only this protocol's variant.
//!
//! Each upsert touches only its own columns on conflict: neither writer may
//! clobber the other's, and either may land first.
//!
//! # Why the resolver lives here and not on `PgPoolRepository`
//!
//! Both of its methods are irreducibly cp-amm-specific — the queue tests columns
//! that only exist on this satellite, and the write takes a payload decoded at
//! cp-amm's byte offsets. Putting them on the cross-protocol pool repository
//! would have made the generic module depend on one protocol, which is the very
//! coupling migration 036 set out to remove (just one layer up).
//!
//! It writes `pools`' neutral columns as well as this satellite, in one
//! transaction. That is legitimate: `pools` is the shared registry every
//! protocol writes to (the indexer's `discover_pool` does the same). What is not
//! legitimate is the reverse — the generic module knowing about cp-amm.

mod rows;

use async_trait::async_trait;
use solana_pubkey::Pubkey;
use sqlx::PgPool;
use yog_core::{
    RepositoryResult,
    domain::{
        MeteoraDammV2PoolProperties, MeteoraDammV2PoolPropertiesRepository, PoolAccountProperties,
        PoolAccountResolver, Protocol,
    },
};

use crate::repositories::helper::{convert_string_to_pubkey, map_sqlx_error};
use rows::MeteoraDammV2PoolPropertiesRow;

pub struct PgMeteoraDammV2PoolPropertiesRepository {
    pool: PgPool,
}

impl PgMeteoraDammV2PoolPropertiesRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl MeteoraDammV2PoolPropertiesRepository for PgMeteoraDammV2PoolPropertiesRepository {
    async fn set_fee_config(
        &self,
        pool_address: &Pubkey,
        base_fee_kind: &str,
        has_dynamic_fee: bool,
    ) -> RepositoryResult<()> {
        sqlx::query!(
            r#"
            INSERT INTO meteora_damm_v2_pool_properties
                (pool_address, base_fee_kind, has_dynamic_fee)
            VALUES ($1, $2, $3)
            ON CONFLICT (pool_address) DO UPDATE
                SET base_fee_kind   = EXCLUDED.base_fee_kind,
                    has_dynamic_fee = EXCLUDED.has_dynamic_fee
            "#,
            pool_address.to_string(),
            base_fee_kind,
            has_dynamic_fee,
        )
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        Ok(())
    }

    async fn find_by_pool(
        &self,
        pool_address: &Pubkey,
    ) -> RepositoryResult<Option<MeteoraDammV2PoolProperties>> {
        let row = sqlx::query_as!(
            MeteoraDammV2PoolPropertiesRow,
            r#"
            SELECT pool_address, protocol_fee_percent, partner_fee_percent,
                   referral_fee_percent, base_fee_kind, has_dynamic_fee
            FROM meteora_damm_v2_pool_properties
            WHERE pool_address = $1
            "#,
            pool_address.to_string(),
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        row.map(MeteoraDammV2PoolProperties::try_from).transpose()
    }
}

#[async_trait]
impl PoolAccountResolver for PgMeteoraDammV2PoolPropertiesRepository {
    fn protocol(&self) -> Protocol {
        Protocol::MeteoraDammV2
    }

    /// The protocol predicate is **required**, not decorative — see the trait
    /// doc. Joining this satellite does not scope the query on its own: "no
    /// satellite row yet" is one of the conditions that makes a pool a
    /// candidate, and that is true of every pool of every other protocol,
    /// forever. Only `p.protocol` excludes them.
    async fn list_unresolved(&self, limit: i64) -> RepositoryResult<Vec<Pubkey>> {
        let rows = sqlx::query!(
            r#"
            SELECT p.pool_address
            FROM pools p
            LEFT JOIN meteora_damm_v2_pool_properties props
                   ON props.pool_address = p.pool_address
            WHERE p.protocol = $2
              AND (p.token_a_mint IS NULL OR p.token_b_mint IS NULL OR p.fee_bps IS NULL
                OR props.pool_address           IS NULL
                OR props.protocol_fee_percent   IS NULL
                OR props.partner_fee_percent    IS NULL
                OR props.referral_fee_percent   IS NULL)
            ORDER BY p.first_seen_at
            LIMIT $1
            "#,
            limit,
            Protocol::MeteoraDammV2.as_str(),
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter()
            .map(|r| convert_string_to_pubkey(r.pool_address, "pool_address"))
            .collect()
    }

    /// Two writes from one account read, in a single transaction.
    ///
    /// The atomicity is load-bearing, not defensive: committing the mints
    /// without the percents (or the reverse) leaves a pool that
    /// [`PoolAccountResolver::list_unresolved`] keeps re-proposing every cycle,
    /// which is exactly the re-fetch loop migration 036 set out to remove.
    ///
    /// The satellite side is an upsert — unlike the columns it replaces, its row
    /// is not created with the pool, so an `UPDATE` would silently do nothing on
    /// first resolution. `ON CONFLICT` touches only the percents, leaving the
    /// indexer-owned fee-shape columns alone.
    /// Rejects a payload of another protocol rather than silently doing nothing:
    /// the worker routes by [`PoolAccountProperties::protocol`], so a mismatch
    /// here is a wiring bug, not a runtime condition.
    async fn set_pool_account(
        &self,
        pool_address: &Pubkey,
        properties: &PoolAccountProperties,
    ) -> RepositoryResult<()> {
        let PoolAccountProperties::MeteoraDammV2(properties) = properties;

        let fee_bps = crate::repositories::pool::fee_bps_to_numeric(properties.fee_bps)?;
        // u8 → i16 (SMALLINT) is always lossless.
        let (protocol_pct, partner_pct, referral_pct) = (
            i16::from(properties.protocol_fee_percent),
            i16::from(properties.partner_fee_percent),
            i16::from(properties.referral_fee_percent),
        );

        let mut tx = self.pool.begin().await.map_err(map_sqlx_error)?;

        sqlx::query!(
            r#"
            UPDATE pools
            SET token_a_mint = $2, token_b_mint = $3, fee_bps = $4
            WHERE pool_address = $1
            "#,
            pool_address.to_string(),
            properties.token_a_mint.to_string(),
            properties.token_b_mint.to_string(),
            fee_bps,
        )
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx_error)?;

        sqlx::query!(
            r#"
            INSERT INTO meteora_damm_v2_pool_properties
                (pool_address, protocol_fee_percent, partner_fee_percent,
                 referral_fee_percent)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (pool_address) DO UPDATE
                SET protocol_fee_percent = EXCLUDED.protocol_fee_percent,
                    partner_fee_percent  = EXCLUDED.partner_fee_percent,
                    referral_fee_percent = EXCLUDED.referral_fee_percent
            "#,
            pool_address.to_string(),
            protocol_pct,
            partner_pct,
            referral_pct,
        )
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx_error)?;

        tx.commit().await.map_err(map_sqlx_error)?;
        Ok(())
    }
}
