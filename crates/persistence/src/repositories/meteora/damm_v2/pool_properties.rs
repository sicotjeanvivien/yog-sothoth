//! DAMM v2 pool-properties satellite repository (migration 036).
//!
//! Holds the pool properties that only exist for cp-amm, kept out of the
//! cross-protocol `pools` registry so no protocol carries NULL columns for
//! another protocol's concepts.
//!
//! One `Pg` type, two **generic** traits, one per consumer:
//!
//! - [`PoolAccountResolver`] — yog-context's enrichment queue and its write.
//!   Generic trait, per-protocol implementation: the queue below filters on this
//!   protocol, the write accepts only this protocol's variant.
//! - [`PoolPropertiesLookup`] — the api's read for the pool detail sheet. Also
//!   generic, so the reading service holds `Vec<Arc<dyn PoolPropertiesLookup>>`
//!   and names no protocol.
//!
//! # One table, one writer
//!
//! This repository writes **this satellite and nothing else**. It used to also
//! write `pools`' neutral columns, in a transaction, because one decoded account
//! carried both — which made the cp-amm satellite a co-owner of the
//! cross-protocol registry. The decode now yields the two halves separately and
//! the caller writes each through its own repository.
//!
//! The transaction went with it. It was the only one in this crate, and it
//! protected nothing observable: the pool-detail read issues two separate
//! queries (so it can already see a torn state, which its `Option`-everywhere
//! DTO expects), and `list_unresolved` reads one snapshot and simply re-proposes
//! a half-written pool, which converges on the next tick.
//!
//! # No write trait of its own
//!
//! The indexer used to write the fee shape here from the genesis blob, through a
//! `MeteoraDammV2PoolPropertiesRepository` trait. That is gone: the indexer no
//! longer writes property values at all, it raises `pools.needs_refresh` and
//! yog-context re-reads the account. Both traits left are protocol-agnostic —
//! nothing in this crate's public surface names cp-amm any more.

mod rows;

use async_trait::async_trait;
use solana_pubkey::Pubkey;
use sqlx::PgPool;
use yog_core::{
    RepositoryResult,
    domain::{
        MeteoraDammV2PoolProperties, PoolAccountProperties, PoolAccountResolver, PoolProperties,
        PoolPropertiesLookup, Protocol,
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
impl PoolPropertiesLookup for PgMeteoraDammV2PoolPropertiesRepository {
    fn protocol(&self) -> Protocol {
        Protocol::MeteoraDammV2
    }

    /// No protocol predicate needed here, unlike
    /// [`PoolAccountResolver::list_unresolved`]: this reads the satellite by
    /// primary key, and a pool of another protocol simply has no row in it.
    async fn find_by_pool(
        &self,
        pool_address: &Pubkey,
    ) -> RepositoryResult<Option<PoolProperties>> {
        let row = sqlx::query_as!(
            MeteoraDammV2PoolPropertiesRow,
            r#"
            SELECT pool_address, protocol_fee_percent,
                   referral_fee_percent, base_fee_kind, has_dynamic_fee
            FROM meteora_damm_v2_pool_properties
            WHERE pool_address = $1
            "#,
            pool_address.to_string(),
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        row.map(MeteoraDammV2PoolProperties::try_from)
            .transpose()
            .map(|p| p.map(PoolProperties::MeteoraDammV2))
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
    ///
    /// # Two reasons a pool is proposed
    ///
    /// **Never resolved** — a NULL column, the original case. And **stale**:
    /// `p.needs_refresh`, raised by the indexer when an event changed a property
    /// it no longer writes itself. The second is what lets a one-shot back-fill
    /// track values that move, without polling every pool on a timer.
    ///
    /// The flag is cleared by the *registry* write, which the caller issues
    /// last — so a pool re-enters the queue until the whole refresh has landed.
    ///
    /// # Why `base_fee_kind` is not in the predicate
    ///
    /// It is the one account-derived property that can legitimately stay NULL
    /// after a *successful* resolution: cp-amm may gain a `BaseFeeMode` this
    /// build cannot map. Testing it here would put every such pool back in the
    /// queue on every cycle, forever — and with `ORDER BY first_seen_at` and a
    /// capped batch, those pools would pile up at the head and starve the ones
    /// behind, which is precisely the failure this query's protocol filter
    /// exists to prevent.
    ///
    /// `has_dynamic_fee` stands in for the pair instead. It is written by the
    /// same call and is **always** decodable (a flag byte at a fixed offset), so
    /// it is NULL exactly when the fee shape has never been resolved, and never
    /// merely because a value was undecodable. A pool whose mode we cannot map
    /// therefore leaves the queue with the shape it could get — and is not
    /// re-proposed by a later build that learns the mode, which is the accepted
    /// cost of not starving the queue.
    async fn list_unresolved(&self, limit: i64) -> RepositoryResult<Vec<Pubkey>> {
        let rows = sqlx::query!(
            r#"
            SELECT p.pool_address
            FROM pools p
            LEFT JOIN meteora_damm_v2_pool_properties props
                   ON props.pool_address = p.pool_address
            WHERE p.protocol = $2
              AND (p.needs_refresh
                OR p.token_a_mint IS NULL OR p.token_b_mint IS NULL OR p.fee_bps IS NULL
                OR props.pool_address           IS NULL
                OR props.protocol_fee_percent   IS NULL
                OR props.referral_fee_percent   IS NULL
                OR props.has_dynamic_fee        IS NULL)
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

    /// One write, to this satellite only.
    ///
    /// The neutral columns of the same account read go through
    /// [`yog_core::domain::PoolRepository::set_registry_properties`] — one table, one
    /// writer. The caller sequences the two: **this first, the registry last**,
    /// because the registry write is what lowers `pools.needs_refresh`. A failure
    /// here therefore leaves the flag raised and the pool queued, rather than
    /// stranding a half-refreshed pool.
    ///
    /// An upsert, not an `UPDATE`: the satellite row is not created with the
    /// pool, so an `UPDATE` would silently do nothing on first resolution.
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
    /// So a cp-amm payload aimed at a DLMM pool is unreachable through the
    /// worker *and* refused by the database. The constraint is there for what
    /// the guards cannot cover: a refactor, a second writer, or a hand-run
    /// repair. It surfaces as [`yog_core::RepositoryError::Conflict`], so any
    /// such caller inherits skip-and-log rather than a silent success.
    async fn set_pool_account(
        &self,
        pool_address: &Pubkey,
        properties: &PoolAccountProperties,
    ) -> RepositoryResult<()> {
        let PoolAccountProperties::MeteoraDammV2(properties) = properties else {
            return Err(yog_core::RepositoryError::Integrity(format!(
                "expected cp-amm pool properties, got {:?}",
                properties.protocol()
            )));
        };

        // u8 → i16 (SMALLINT) is always lossless.
        let (protocol_pct, referral_pct) = (
            i16::from(properties.protocol_fee_percent),
            i16::from(properties.referral_fee_percent),
        );
        let base_fee_kind = properties.base_fee_kind.map(|kind| kind.as_str());

        sqlx::query!(
            r#"
            INSERT INTO meteora_damm_v2_pool_properties
                (pool_address, protocol_fee_percent, referral_fee_percent,
                 base_fee_kind, has_dynamic_fee)
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (pool_address) DO UPDATE
                SET protocol_fee_percent = EXCLUDED.protocol_fee_percent,
                    referral_fee_percent = EXCLUDED.referral_fee_percent,
                    has_dynamic_fee      = EXCLUDED.has_dynamic_fee,
                    -- COALESCE, not EXCLUDED: an account whose BaseFeeMode we
                    -- cannot map sends NULL, and that must not erase a kind an
                    -- earlier decode already established.
                    base_fee_kind        = COALESCE(EXCLUDED.base_fee_kind,
                                                    meteora_damm_v2_pool_properties.base_fee_kind)
            "#,
            pool_address.to_string(),
            protocol_pct,
            referral_pct,
            base_fee_kind,
            properties.has_dynamic_fee,
        )
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        Ok(())
    }
}
