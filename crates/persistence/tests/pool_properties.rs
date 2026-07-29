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

use yog_core::domain::{
    MeteoraDammV2PoolAccountProperties, MeteoraDammV2PoolProperties,
    MeteoraDammV2PoolPropertiesRepository, PoolAccountProperties, PoolAccountResolver,
    PoolProperties, PoolPropertiesLookup, Protocol,
};
use yog_persistence::PgMeteoraDammV2PoolPropertiesRepository;

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

fn account_properties() -> PoolAccountProperties {
    PoolAccountProperties::MeteoraDammV2(MeteoraDammV2PoolAccountProperties {
        token_a_mint: pk(10),
        token_b_mint: pk(11),
        fee_bps: Decimal::new(25, 0),
        protocol_fee_percent: 20,
        referral_fee_percent: 20,
    })
}

// ── set_pool_account: one account read, two tables ──────────────────

#[sqlx::test]
async fn set_pool_account_writes_both_the_registry_and_the_satellite(pool: PgPool) {
    seed_pool(&pool, pk(1), Protocol::MeteoraDammV2, 1).await;
    // One Pg type, both traits: the enrichment queue + two-table write
    // (PoolAccountResolver, generic trait / per-protocol impl) and the
    // satellite's own read/write (MeteoraDammV2PoolPropertiesRepository).
    let repo = PgMeteoraDammV2PoolPropertiesRepository::new(pool.clone());

    repo.set_pool_account(&pk(1), &account_properties())
        .await
        .expect("set_pool_account failed");

    // Neutral columns land on `pools`…
    let row = sqlx::query_as::<_, (Option<String>, Option<Decimal>)>(
        "SELECT token_a_mint, fee_bps FROM pools WHERE pool_address = $1",
    )
    .bind(pk(1).to_string())
    .fetch_one(&pool)
    .await
    .expect("pool read failed");
    assert_eq!(row.0.as_deref(), Some(pk(10).to_string().as_str()));
    assert_eq!(row.1, Some(Decimal::new(25, 0)));

    // …and the cp-amm percents on the satellite, created by the same call.
    let props = read_properties(&repo, &pk(1))
        .await
        .expect("satellite row should have been created");
    assert_eq!(props.protocol_fee_percent, Some(20));
    assert_eq!(props.referral_fee_percent, Some(20));
    // Not written by this path — it is the indexer's column.
    assert_eq!(props.base_fee_kind, None);
}

/// The satellite is `REFERENCES pools`, so an unknown pool is an error rather
/// than a silent no-op — and the transaction must leave nothing behind.
#[sqlx::test]
async fn set_pool_account_for_an_unknown_pool_writes_nothing(pool: PgPool) {
    let repo = PgMeteoraDammV2PoolPropertiesRepository::new(pool.clone());

    let result = repo.set_pool_account(&pk(99), &account_properties()).await;

    assert!(result.is_err(), "expected a foreign-key error");
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM meteora_damm_v2_pool_properties")
        .fetch_one(&pool)
        .await
        .expect("count failed");
    assert_eq!(count.0, 0, "the transaction must have rolled back");
}

// ── The two writers must not clobber each other ─────────────────────

#[sqlx::test]
async fn fee_config_and_percents_coexist_whichever_lands_first(pool: PgPool) {
    seed_pool(&pool, pk(1), Protocol::MeteoraDammV2, 1).await;
    seed_pool(&pool, pk(2), Protocol::MeteoraDammV2, 2).await;
    // One Pg type, both traits: the enrichment queue + two-table write
    // (PoolAccountResolver, generic trait / per-protocol impl) and the
    // satellite's own read/write (MeteoraDammV2PoolPropertiesRepository).
    let repo = PgMeteoraDammV2PoolPropertiesRepository::new(pool.clone());

    // Pool 1: indexer first (fee shape), then context (percents).
    repo.set_fee_config(&pk(1), "scheduler_linear", true)
        .await
        .expect("set_fee_config failed");
    repo.set_pool_account(&pk(1), &account_properties())
        .await
        .expect("set_pool_account failed");

    // Pool 2: the reverse order.
    repo.set_pool_account(&pk(2), &account_properties())
        .await
        .expect("set_pool_account failed");
    repo.set_fee_config(&pk(2), "scheduler_linear", true)
        .await
        .expect("set_fee_config failed");

    for addr in [pk(1), pk(2)] {
        let props = read_properties(&repo, &addr)
            .await
            .expect("row should exist");
        assert_eq!(
            props.base_fee_kind.as_deref(),
            Some("scheduler_linear"),
            "fee shape lost for {addr}"
        );
        assert_eq!(
            props.protocol_fee_percent,
            Some(20),
            "percents lost for {addr}"
        );
    }
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
    let repo = PgMeteoraDammV2PoolPropertiesRepository::new(pool);

    repo.set_pool_account(&pk(1), &account_properties())
        .await
        .expect("set_pool_account failed");
    let unresolved = repo.list_unresolved(100).await.expect("query failed");

    assert!(
        unresolved.is_empty(),
        "a resolved pool must leave the queue, else it is re-fetched forever"
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

/// `set_has_dynamic_fee` must touch **only** its column. An `UpdatePoolFees`
/// event can toggle the dynamic fee but carries no base-fee mode, so writing
/// through `set_fee_config` would have meant inventing a `base_fee_kind` — or
/// re-reading it just to write it back.
#[sqlx::test]
async fn set_has_dynamic_fee_leaves_base_fee_kind_alone(pool: PgPool) {
    seed_pool(&pool, pk(1), Protocol::MeteoraDammV2, 1).await;
    let repo = PgMeteoraDammV2PoolPropertiesRepository::new(pool.clone());

    // Genesis wrote the shape…
    repo.set_fee_config(&pk(1), "scheduler_linear", false)
        .await
        .expect("set_fee_config failed");
    // …then an operator turns the dynamic fee on.
    repo.set_has_dynamic_fee(&pk(1), true)
        .await
        .expect("set_has_dynamic_fee failed");

    let props = read_properties(&repo, &pk(1))
        .await
        .expect("row should exist");
    assert_eq!(
        props.has_dynamic_fee,
        Some(true),
        "the flag must be updated"
    );
    assert_eq!(
        props.base_fee_kind.as_deref(),
        Some("scheduler_linear"),
        "base_fee_kind must survive — it cannot change through this event"
    );
}

/// …and it creates the row when the genesis event was never seen, which is the
/// common case: a pool's creation is only observable if we were already
/// watching.
#[sqlx::test]
async fn set_has_dynamic_fee_creates_the_row_when_genesis_was_missed(pool: PgPool) {
    seed_pool(&pool, pk(1), Protocol::MeteoraDammV2, 1).await;
    let repo = PgMeteoraDammV2PoolPropertiesRepository::new(pool.clone());

    repo.set_has_dynamic_fee(&pk(1), true)
        .await
        .expect("set_has_dynamic_fee failed");

    let props = read_properties(&repo, &pk(1))
        .await
        .expect("row should have been created");
    assert_eq!(props.has_dynamic_fee, Some(true));
    assert_eq!(props.base_fee_kind, None);
}
