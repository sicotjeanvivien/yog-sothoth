//! Integration tests for the DAMM v2 pool-properties satellite (migration 036)
//! and the account-resolution queue that reads it.
//!
//! Gated behind `integration-tests`: they need a live Postgres (sqlx::test
//! provisions an isolated, migrated DB per test).
//!
//! These cover what the unit tests structurally cannot — that the two-table
//! write is atomic, that the two writers do not clobber each other, and above
//! all that `list_unresolved` no longer proposes pools it can never resolve.
//! That last one is the regression guard for the failure mode migration 036 set
//! out to remove: before it, the queue had no protocol filter, and since a pool
//! it cannot resolve is never removed from the result set while the ordering is
//! `first_seen_at` ascending, foreign-protocol pools accumulated at the head and
//! eventually starved DAMM v2 enrichment entirely.

use super::helpers::pk;
use chrono::{DateTime, TimeZone, Utc};
use rust_decimal::Decimal;
use solana_pubkey::Pubkey;
use sqlx::PgPool;

use yog_core::amm::damm_v2::BaseFeeKind;
use yog_core::domain::{
    MeteoraDammV2PoolAccountProperties, MeteoraDammV2PoolProperties, PoolAccountProperties,
    PoolAccountResolver, PoolProperties, PoolPropertiesLookup, PoolRegistryProperties,
    PoolRepository, Protocol,
};
use yog_persistence::{PgMeteoraDammV2PoolPropertiesRepository, PgPoolRepository};

fn ts(secs: i64) -> DateTime<Utc> {
    Utc.timestamp_opt(secs, 0).unwrap()
}

/// Read one pool's satellite row, unwrapping the cross-protocol
/// [`PoolProperties`] enum the lookup returns. The reads below assert on cp-amm
/// columns, so they want the concrete type; the enum is the api's concern.
async fn read_properties(
    repo: &PgMeteoraDammV2PoolPropertiesRepository,
    addr: &Pubkey,
) -> Option<MeteoraDammV2PoolProperties> {
    repo.find_by_pool(addr)
        .await
        .expect("find_by_pool failed")
        .map(|properties| match properties {
            PoolProperties::MeteoraDammV2(properties) => properties,
        })
}

/// Insert a bare pool row, as `discover_pool` would: address + protocol only,
/// everything else NULL. `seq` drives `first_seen_at` so queue ordering is
/// deterministic.
async fn seed_pool(pool: &PgPool, addr: Pubkey, protocol: Protocol, seq: i64) {
    sqlx::query(
        r#"
        INSERT INTO pools (pool_address, protocol, first_seen_at, last_seen_at)
        VALUES ($1, $2, $3, $3)
        "#,
    )
    .bind(addr.to_string())
    .bind(protocol.as_str())
    .bind(ts(seq * 100))
    .execute(pool)
    .await
    .expect("seed pool failed");
}

/// The cp-amm half alone — what the satellite repository now receives.
fn properties_only(base_fee_kind: Option<BaseFeeKind>) -> PoolAccountProperties {
    PoolAccountProperties::MeteoraDammV2(MeteoraDammV2PoolAccountProperties {
        protocol_fee_percent: 20,
        referral_fee_percent: 20,
        base_fee_kind,
        has_dynamic_fee: true,
    })
}

/// The neutral half — what the registry repository receives.
fn account_core() -> PoolRegistryProperties {
    PoolRegistryProperties {
        token_a_mint: pk(10),
        token_b_mint: pk(11),
        fee_bps: Decimal::new(25, 0),
    }
}

/// One full resolution, in the order the worker uses: satellite first, registry
/// last. The ordering is the invariant, so the tests below go through this
/// helper rather than restating it.
async fn resolve(
    satellite: &PgMeteoraDammV2PoolPropertiesRepository,
    registry: &PgPoolRepository,
    addr: &Pubkey,
    base_fee_kind: Option<BaseFeeKind>,
) {
    satellite
        .set_pool_account(addr, &properties_only(base_fee_kind))
        .await
        .expect("set_pool_account failed");
    registry
        .set_registry_properties(addr, &account_core())
        .await
        .expect("set_registry_properties failed");
}

// ── One account read, two owners ────────────────────────────────────

/// One decoded account reaches both tables — through two repositories, each
/// writing only what it owns.
#[sqlx::test]
async fn a_resolution_fills_the_registry_and_the_satellite(pool: PgPool) {
    seed_pool(&pool, pk(1), Protocol::MeteoraDammV2, 1).await;
    let satellite = PgMeteoraDammV2PoolPropertiesRepository::new(pool.clone());
    let registry = PgPoolRepository::new(pool.clone());

    resolve(&satellite, &registry, &pk(1), Some(BaseFeeKind::Constant)).await;

    // The neutral columns, written by the registry's own repository.
    let row = sqlx::query_as::<_, (Option<String>, Option<Decimal>)>(
        "SELECT token_a_mint, fee_bps FROM pools WHERE pool_address = $1",
    )
    .bind(pk(1).to_string())
    .fetch_one(&pool)
    .await
    .expect("pool read failed");
    assert_eq!(row.0.as_deref(), Some(pk(10).to_string().as_str()));
    assert_eq!(row.1, Some(Decimal::new(25, 0)));

    // The cp-amm ones, written by the satellite's.
    let props = read_properties(&satellite, &pk(1))
        .await
        .expect("satellite row should have been created");
    assert_eq!(props.protocol_fee_percent, Some(20));
    assert_eq!(props.referral_fee_percent, Some(20));
    // …including the fee shape, read from the same account rather than from a
    // genesis event that, for a discovered pool, never comes.
    assert_eq!(props.base_fee_kind.as_deref(), Some("constant"));
    assert_eq!(props.has_dynamic_fee, Some(true));
}

/// The satellite is `REFERENCES pools`, so an unknown pool is an error rather
/// than a silent no-op — and nothing must be left behind.
#[sqlx::test]
async fn set_pool_account_for_an_unknown_pool_writes_nothing(pool: PgPool) {
    let repo = PgMeteoraDammV2PoolPropertiesRepository::new(pool.clone());

    let result = repo
        .set_pool_account(&pk(99), &properties_only(Some(BaseFeeKind::Constant)))
        .await;

    assert!(result.is_err(), "expected a foreign-key error");
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM meteora_damm_v2_pool_properties")
        .fetch_one(&pool)
        .await
        .expect("count failed");
    assert_eq!(count.0, 0, "the transaction must have rolled back");
}

// ── list_unresolved: the queue ──────────────────────────────────────

#[sqlx::test]
async fn list_unresolved_returns_a_freshly_discovered_damm_v2_pool(pool: PgPool) {
    seed_pool(&pool, pk(1), Protocol::MeteoraDammV2, 1).await;
    let repo = PgMeteoraDammV2PoolPropertiesRepository::new(pool);

    let unresolved = repo.list_unresolved(100).await.expect("query failed");

    assert_eq!(unresolved, vec![pk(1)]);
}

#[sqlx::test]
async fn list_unresolved_drops_a_pool_once_its_account_is_resolved(pool: PgPool) {
    seed_pool(&pool, pk(1), Protocol::MeteoraDammV2, 1).await;
    let satellite = PgMeteoraDammV2PoolPropertiesRepository::new(pool.clone());
    let registry = PgPoolRepository::new(pool.clone());

    // **Both** halves are required to leave the queue — the satellite write
    // alone leaves the registry's mints and fee NULL. Covered on its own by
    // `only_the_registry_write_lowers_the_flag`.
    resolve(&satellite, &registry, &pk(1), Some(BaseFeeKind::Constant)).await;
    let unresolved = satellite.list_unresolved(100).await.expect("query failed");

    assert!(
        unresolved.is_empty(),
        "a resolved pool must leave the queue, else it is re-fetched forever"
    );
}

/// A pool resolved by an **older build** — percents and mints filled, fee shape
/// never — comes back into the queue, which is how the fee shape reaches the
/// pools that were enriched before the account decoder learned to read it.
///
/// The predicate keys off `has_dynamic_fee` for this: it is written by the same
/// call as `base_fee_kind` and is always decodable, so NULL means "never
/// resolved" and never "resolved but undecodable".
#[sqlx::test]
async fn a_pool_resolved_without_a_fee_shape_is_proposed_again(pool: PgPool) {
    seed_pool(&pool, pk(1), Protocol::MeteoraDammV2, 1).await;
    let repo = PgMeteoraDammV2PoolPropertiesRepository::new(pool.clone());

    // What the pre-PR resolver left behind: everything but the fee shape.
    sqlx::query(
        r#"
        UPDATE pools SET token_a_mint = $2, token_b_mint = $3, fee_bps = 25
        WHERE pool_address = $1
        "#,
    )
    .bind(pk(1).to_string())
    .bind(pk(10).to_string())
    .bind(pk(11).to_string())
    .execute(&pool)
    .await
    .expect("seed neutral columns failed");
    sqlx::query(
        r#"
        INSERT INTO meteora_damm_v2_pool_properties
            (pool_address, protocol_fee_percent, referral_fee_percent)
        VALUES ($1, 20, 20)
        "#,
    )
    .bind(pk(1).to_string())
    .execute(&pool)
    .await
    .expect("seed satellite failed");

    let unresolved = repo.list_unresolved(100).await.expect("query failed");

    assert_eq!(
        unresolved,
        vec![pk(1)],
        "a pool with no fee shape must be re-proposed so it can be back-filled"
    );
}

/// An account whose `BaseFeeMode` this build cannot map sends `base_fee_kind =
/// NULL`. That must **not** erase a kind already established — by an earlier
/// build that knew the mode, or by a genesis event.
///
/// This is why the upsert uses `COALESCE` on that one column rather than
/// `EXCLUDED`: every other column is authoritative from the account, this one
/// can legitimately arrive unknown.
#[sqlx::test]
async fn an_unmappable_fee_mode_does_not_erase_a_known_kind(pool: PgPool) {
    seed_pool(&pool, pk(1), Protocol::MeteoraDammV2, 1).await;
    let repo = PgMeteoraDammV2PoolPropertiesRepository::new(pool.clone());

    repo.set_pool_account(&pk(1), &properties_only(Some(BaseFeeKind::RateLimiter)))
        .await
        .expect("first resolution failed");
    repo.set_pool_account(&pk(1), &properties_only(None))
        .await
        .expect("second resolution failed");

    let props = read_properties(&repo, &pk(1))
        .await
        .expect("row should exist");
    assert_eq!(
        props.base_fee_kind.as_deref(),
        Some("rate_limiter"),
        "an unknown mode must leave the previously known kind standing"
    );
}

/// **The regression guard.** A DLMM pool can never be decoded by the cp-amm
/// account source, so proposing it means re-fetching it every cycle, forever —
/// and, since the queue is ordered oldest-first and capped, eventually crowding
/// out every DAMM v2 pool behind it.
#[sqlx::test]
async fn list_unresolved_never_returns_a_pool_of_another_protocol(pool: PgPool) {
    // The foreign-protocol pools are the *oldest*, so under the pre-036 query
    // they would sit at the head of the queue and starve the DAMM v2 one.
    seed_pool(&pool, pk(1), Protocol::MeteoraDlmm, 1).await;
    seed_pool(&pool, pk(2), Protocol::MeteoraDammV1, 2).await;
    seed_pool(&pool, pk(3), Protocol::MeteoraDammV2, 3).await;
    let repo = PgMeteoraDammV2PoolPropertiesRepository::new(pool);

    let unresolved = repo.list_unresolved(100).await.expect("query failed");

    assert_eq!(
        unresolved,
        vec![pk(3)],
        "only the DAMM v2 pool is resolvable by the cp-amm account source"
    );
}

/// Corollary of the above, stated as the failure it prevents: with the queue
/// capped at a batch smaller than the number of foreign pools, the DAMM v2 pool
/// must still be reached.
#[sqlx::test]
async fn list_unresolved_is_not_starved_by_older_foreign_pools(pool: PgPool) {
    for seq in 1..=5 {
        seed_pool(&pool, pk(seq as u8), Protocol::MeteoraDlmm, seq).await;
    }
    seed_pool(&pool, pk(50), Protocol::MeteoraDammV2, 99).await;
    let repo = PgMeteoraDammV2PoolPropertiesRepository::new(pool);

    let unresolved = repo.list_unresolved(2).await.expect("query failed");

    assert_eq!(
        unresolved,
        vec![pk(50)],
        "the DAMM v2 pool must be reached despite five older foreign pools \
         and a batch of 2"
    );
}

// ── needs_refresh: the invalidation loop ────────────────────────────

/// The whole indexer→context handshake, end to end.
///
/// A resolved pool is out of the queue. The indexer sees a fee-change event and
/// raises the flag — without writing any property value — and the pool comes
/// back. The refresh then lowers it, and the pool leaves again.
#[sqlx::test]
async fn the_refresh_flag_puts_a_resolved_pool_back_in_the_queue(pool: PgPool) {
    seed_pool(&pool, pk(1), Protocol::MeteoraDammV2, 1).await;
    let satellite = PgMeteoraDammV2PoolPropertiesRepository::new(pool.clone());
    let registry = PgPoolRepository::new(pool.clone());

    resolve(&satellite, &registry, &pk(1), Some(BaseFeeKind::Constant)).await;
    assert!(
        satellite.list_unresolved(100).await.unwrap().is_empty(),
        "a resolved pool must be out of the queue to begin with"
    );

    // What the indexer does on UpdatePoolFees — and all it does.
    registry
        .mark_needs_refresh(&pk(1))
        .await
        .expect("mark_needs_refresh failed");

    assert_eq!(
        satellite.list_unresolved(100).await.unwrap(),
        vec![pk(1)],
        "a flagged pool must be proposed again even though no column is NULL"
    );

    resolve(
        &satellite,
        &registry,
        &pk(1),
        Some(BaseFeeKind::RateLimiter),
    )
    .await;

    assert!(
        satellite.list_unresolved(100).await.unwrap().is_empty(),
        "the refresh must lower the flag"
    );
    let props = read_properties(&satellite, &pk(1))
        .await
        .expect("row should exist");
    assert_eq!(props.base_fee_kind.as_deref(), Some("rate_limiter"));
}

/// The satellite write **must not** clear the flag: only the registry write
/// does, and the worker issues it last. Were it otherwise, a satellite success
/// followed by a registry failure would drop the pool from the queue with half
/// its properties refreshed.
#[sqlx::test]
async fn only_the_registry_write_lowers_the_flag(pool: PgPool) {
    seed_pool(&pool, pk(1), Protocol::MeteoraDammV2, 1).await;
    let satellite = PgMeteoraDammV2PoolPropertiesRepository::new(pool.clone());
    let registry = PgPoolRepository::new(pool.clone());

    resolve(&satellite, &registry, &pk(1), Some(BaseFeeKind::Constant)).await;
    registry.mark_needs_refresh(&pk(1)).await.unwrap();

    // Only the satellite half lands — as if the registry write then failed.
    satellite
        .set_pool_account(&pk(1), &properties_only(Some(BaseFeeKind::Constant)))
        .await
        .expect("set_pool_account failed");

    assert_eq!(
        satellite.list_unresolved(100).await.unwrap(),
        vec![pk(1)],
        "the pool must stay queued until the registry write lands"
    );
}

/// `set_pool_account` writes the satellite and **nothing else** — the `pools`
/// registry is another repository's table now.
#[sqlx::test]
async fn set_pool_account_does_not_touch_the_registry(pool: PgPool) {
    seed_pool(&pool, pk(1), Protocol::MeteoraDammV2, 1).await;
    let satellite = PgMeteoraDammV2PoolPropertiesRepository::new(pool.clone());

    satellite
        .set_pool_account(&pk(1), &properties_only(Some(BaseFeeKind::Constant)))
        .await
        .expect("set_pool_account failed");

    let row = sqlx::query_as::<_, (Option<String>, Option<Decimal>)>(
        "SELECT token_a_mint, fee_bps FROM pools WHERE pool_address = $1",
    )
    .bind(pk(1).to_string())
    .fetch_one(&pool)
    .await
    .expect("pool read failed");
    assert_eq!(
        row.0, None,
        "the registry's mints are not this repo's to write"
    );
    assert_eq!(row.1, None, "nor its fee_bps");
}
