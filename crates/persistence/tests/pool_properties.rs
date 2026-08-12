//! Integration tests for the DAMM v2 pool-properties satellite (baseline §8)
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
//!
//! The DLMM satellite (039) and the satellite↔protocol invariant migration 040
//! writes into the schema are covered in their own sections below.

use super::helpers::pk;
use chrono::{DateTime, TimeZone, Utc};
use rust_decimal::Decimal;
use solana_pubkey::Pubkey;
use sqlx::PgPool;

use yog_core::RepositoryError;
use yog_core::amm::damm_v2::{BaseFeeKind, FeeSchedulerParams};
use yog_core::domain::{
    MeteoraDammV2PoolAccountProperties, MeteoraDammV2PoolProperties,
    MeteoraDlmmPoolAccountProperties, MeteoraDlmmPoolProperties, PoolAccountProperties,
    PoolAccountResolver, PoolProperties, PoolPropertiesLookup, PoolRegistryProperties,
    PoolRepository, Protocol,
};
use yog_persistence::{
    PgMeteoraDammV2PoolPropertiesRepository, PgMeteoraDlmmPoolPropertiesRepository,
    PgPoolRepository,
};

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
            other => panic!("expected cp-amm properties, got {other:?}"),
        })
}

/// The DLMM equivalent of [`read_properties`].
async fn read_dlmm_properties(
    repo: &PgMeteoraDlmmPoolPropertiesRepository,
    addr: &Pubkey,
) -> Option<MeteoraDlmmPoolProperties> {
    repo.find_by_pool(addr)
        .await
        .expect("find_by_pool failed")
        .map(|properties| match properties {
            PoolProperties::MeteoraDlmm(properties) => properties,
            other => panic!("expected DLMM properties, got {other:?}"),
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
        // No decay curve — the helper builds a pool whose fee does not decay.
        fee_scheduler: None,
    })
}

/// The DLMM half alone, with the values read from
/// `HTvjzsfX3yU6BUodCjZ5vZkUrAxMDTrBs3CJaq43ashR` on mainnet.
fn dlmm_properties_only() -> PoolAccountProperties {
    PoolAccountProperties::MeteoraDlmm(MeteoraDlmmPoolAccountProperties {
        bin_step: 1,
        base_factor: 10_000,
        base_fee_power_factor: 0,
        variable_fee_control: 2_000_000,
        max_volatility_accumulator: 100_000,
        protocol_share: 1_000,
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

// ── DLMM satellite (baseline §9) ──────────────────────────────────

/// The DLMM resolver stores its six columns and reads them back unchanged,
/// including the values a signed column of the on-chain width could not hold.
#[sqlx::test]
async fn the_dlmm_satellite_round_trips_every_column(pool: PgPool) {
    seed_pool(&pool, pk(1), Protocol::MeteoraDlmm, 1).await;
    let repo = PgMeteoraDlmmPoolPropertiesRepository::new(pool);

    repo.set_pool_account(&pk(1), &dlmm_properties_only())
        .await
        .expect("set_pool_account failed");

    let props = read_dlmm_properties(&repo, &pk(1))
        .await
        .expect("row should exist");
    assert_eq!(props.bin_step, Some(1));
    assert_eq!(props.base_factor, Some(10_000));
    assert_eq!(props.base_fee_power_factor, Some(0));
    assert_eq!(props.variable_fee_control, Some(2_000_000));
    assert_eq!(props.max_volatility_accumulator, Some(100_000));
    assert_eq!(props.protocol_share, Some(1_000));
}

/// The reason the columns are INTEGER and BIGINT rather than SMALLINT and
/// INTEGER: an on-chain `u16` / `u32` at the top of its range must survive the
/// round trip through Postgres, which has no unsigned integers.
#[sqlx::test]
async fn the_dlmm_satellite_round_trips_the_top_of_each_unsigned_range(pool: PgPool) {
    seed_pool(&pool, pk(1), Protocol::MeteoraDlmm, 1).await;
    let repo = PgMeteoraDlmmPoolPropertiesRepository::new(pool);

    let extreme = PoolAccountProperties::MeteoraDlmm(MeteoraDlmmPoolAccountProperties {
        bin_step: u16::MAX,
        base_factor: u16::MAX,
        base_fee_power_factor: u8::MAX,
        variable_fee_control: u32::MAX,
        max_volatility_accumulator: u32::MAX,
        protocol_share: u16::MAX,
    });
    repo.set_pool_account(&pk(1), &extreme)
        .await
        .expect("set_pool_account failed");

    let props = read_dlmm_properties(&repo, &pk(1))
        .await
        .expect("row should exist");
    // All six, not a sample: each column is what justifies its own width, so
    // leaving one unread would leave that width unproven.
    assert_eq!(props.bin_step, Some(u16::MAX));
    assert_eq!(props.base_factor, Some(u16::MAX));
    assert_eq!(props.base_fee_power_factor, Some(u8::MAX));
    assert_eq!(props.variable_fee_control, Some(u32::MAX));
    assert_eq!(props.max_volatility_accumulator, Some(u32::MAX));
    assert_eq!(props.protocol_share, Some(u16::MAX));
}

/// A second resolution overwrites every column — no `COALESCE` anywhere, unlike
/// cp-amm's `base_fee_kind`. A successful `LbPair` decode always carries all six
/// fields, so there is never a NULL to protect an earlier value from, and an
/// `update_fee_parameters` must actually take effect.
#[sqlx::test]
async fn a_second_dlmm_resolution_overwrites_every_column(pool: PgPool) {
    seed_pool(&pool, pk(1), Protocol::MeteoraDlmm, 1).await;
    let repo = PgMeteoraDlmmPoolPropertiesRepository::new(pool);

    repo.set_pool_account(&pk(1), &dlmm_properties_only())
        .await
        .expect("first resolution failed");
    let updated = PoolAccountProperties::MeteoraDlmm(MeteoraDlmmPoolAccountProperties {
        bin_step: 1,
        base_factor: 20_000,
        base_fee_power_factor: 0,
        variable_fee_control: 0,
        max_volatility_accumulator: 0,
        protocol_share: 2_000,
    });
    repo.set_pool_account(&pk(1), &updated)
        .await
        .expect("second resolution failed");

    let props = read_dlmm_properties(&repo, &pk(1))
        .await
        .expect("row should exist");
    assert_eq!(props.base_factor, Some(20_000), "the new fee must land");
    assert_eq!(
        props.variable_fee_control,
        Some(0),
        "a dynamic fee turned off must actually turn off — 0 is a value, not a \
         missing one"
    );
}

/// A resolver must refuse another protocol's payload rather than write it or
/// silently do nothing. The worker routes by protocol, so reaching this is a
/// wiring bug, and it must surface as one.
#[sqlx::test]
async fn each_resolver_rejects_the_other_protocols_payload(pool: PgPool) {
    seed_pool(&pool, pk(1), Protocol::MeteoraDlmm, 1).await;
    seed_pool(&pool, pk(2), Protocol::MeteoraDammV2, 2).await;
    let dlmm = PgMeteoraDlmmPoolPropertiesRepository::new(pool.clone());
    let damm = PgMeteoraDammV2PoolPropertiesRepository::new(pool.clone());

    dlmm.set_pool_account(&pk(1), &properties_only(Some(BaseFeeKind::Constant)))
        .await
        .expect_err("the DLMM resolver must refuse a cp-amm payload");
    damm.set_pool_account(&pk(2), &dlmm_properties_only())
        .await
        .expect_err("the cp-amm resolver must refuse a DLMM payload");

    assert!(
        read_dlmm_properties(&dlmm, &pk(1)).await.is_none(),
        "a rejected payload must leave no row behind"
    );
    assert!(read_properties(&damm, &pk(2)).await.is_none());
}

// ── The two queues, side by side ────────────────────────────────────

/// **The test that needs two protocols to exist.** Each queue proposes only its
/// own pools — in both directions.
///
/// The cp-amm side of this was already covered, but it could only ever assert
/// half the invariant: before this migration there was no second resolver to
/// starve. A DLMM `list_unresolved` missing its `p.protocol` predicate would
/// propose the whole cp-amm catalogue, whose accounts it cannot decode, so those
/// pools would never resolve, never leave the queue, and — ordered oldest-first
/// with a capped batch — crowd out every DLMM pool behind them.
#[sqlx::test]
async fn each_queue_proposes_only_its_own_protocol(pool: PgPool) {
    seed_pool(&pool, pk(1), Protocol::MeteoraDammV2, 1).await;
    seed_pool(&pool, pk(2), Protocol::MeteoraDlmm, 2).await;
    seed_pool(&pool, pk(3), Protocol::MeteoraDammV1, 3).await;

    let damm = PgMeteoraDammV2PoolPropertiesRepository::new(pool.clone());
    let dlmm = PgMeteoraDlmmPoolPropertiesRepository::new(pool.clone());

    assert_eq!(
        damm.list_unresolved(100).await.expect("query failed"),
        vec![pk(1)],
        "the cp-amm queue must not see the DLMM pool"
    );
    assert_eq!(
        dlmm.list_unresolved(100).await.expect("query failed"),
        vec![pk(2)],
        "the DLMM queue must not see the cp-amm pool"
    );
}

/// The starvation guard, stated from the DLMM side: five older cp-amm pools and
/// a batch of two must not keep the DLMM pool out of its own queue.
#[sqlx::test]
async fn the_dlmm_queue_is_not_starved_by_older_cp_amm_pools(pool: PgPool) {
    for seq in 1..=5 {
        seed_pool(&pool, pk(seq as u8), Protocol::MeteoraDammV2, seq).await;
    }
    seed_pool(&pool, pk(50), Protocol::MeteoraDlmm, 99).await;
    let repo = PgMeteoraDlmmPoolPropertiesRepository::new(pool);

    let unresolved = repo.list_unresolved(2).await.expect("query failed");

    assert_eq!(
        unresolved,
        vec![pk(50)],
        "the DLMM pool must be reached despite five older cp-amm pools and a \
         batch of 2"
    );
}

/// A DLMM pool leaves its queue only once **both** halves are written — the
/// satellite and the neutral registry columns. Same contract as cp-amm's, and
/// the reason the worker writes the registry last.
#[sqlx::test]
async fn a_resolved_dlmm_pool_leaves_its_queue(pool: PgPool) {
    seed_pool(&pool, pk(1), Protocol::MeteoraDlmm, 1).await;
    let satellite = PgMeteoraDlmmPoolPropertiesRepository::new(pool.clone());
    let registry = PgPoolRepository::new(pool.clone());

    satellite
        .set_pool_account(&pk(1), &dlmm_properties_only())
        .await
        .expect("set_pool_account failed");
    assert_eq!(
        satellite.list_unresolved(100).await.unwrap(),
        vec![pk(1)],
        "the satellite write alone leaves the mints and fee NULL"
    );

    registry
        .set_registry_properties(&pk(1), &account_core())
        .await
        .expect("set_registry_properties failed");

    assert!(
        satellite.list_unresolved(100).await.unwrap().is_empty(),
        "a fully resolved pool must leave the queue"
    );
}

/// The satellite is `REFERENCES pools`, so a DLMM write for an unknown pool is
/// an error rather than a silent no-op.
#[sqlx::test]
async fn a_dlmm_write_for_an_unknown_pool_fails(pool: PgPool) {
    let repo = PgMeteoraDlmmPoolPropertiesRepository::new(pool);

    repo.set_pool_account(&pk(99), &dlmm_properties_only())
        .await
        .expect_err("an unknown pool must violate the foreign key");
}

// ── The pool↔protocol invariant, in the schema (baseline §8-§9) ──────

// `sqlstate` lives in `helpers` — `price_positivity.rs` asserts a SQLSTATE for
// the same reason, and one definition is what keeps the rationale attached to
// the rule rather than to whichever test file happened to need it first.
use super::helpers::sqlstate;

/// `23503` — foreign_key_violation.
const FOREIGN_KEY_VIOLATION: &str = "23503";
/// `428C9` — cannot insert into a `GENERATED ALWAYS` column.
const GENERATED_ALWAYS: &str = "428C9";

/// A dependent row cannot carry a protocol the registry disagrees with.
///
/// Deliberately **raw SQL**, not the repositories: the resolvers reject a
/// foreign *payload* in Rust and the worker screens the pool before that, so
/// going through them would exercise those guards and prove nothing about the
/// constraint. Raw SQL is also the realistic threat model — a repair script, a
/// psql session, a future writer that skipped the discipline.
#[sqlx::test]
async fn a_dependent_row_cannot_disagree_with_the_registry(pool: PgPool) {
    seed_pool(&pool, pk(1), Protocol::MeteoraDlmm, 1).await;
    seed_pool(&pool, pk(2), Protocol::MeteoraDammV2, 2).await;

    let err = sqlx::query(
        "INSERT INTO meteora_damm_v2_pool_properties (pool_address, protocol_fee_percent) \
         VALUES ($1, 20)",
    )
    .bind(pk(1).to_string())
    .execute(&pool)
    .await
    .expect_err("a cp-amm property row for a DLMM pool must violate the composite key");
    assert_eq!(sqlstate(&err), FOREIGN_KEY_VIOLATION, "{err:?}");

    let err = sqlx::query(
        "INSERT INTO meteora_dlmm_pool_properties (pool_address, bin_step) VALUES ($1, 1)",
    )
    .bind(pk(2).to_string())
    .execute(&pool)
    .await
    .expect_err("a DLMM property row for a cp-amm pool must violate the composite key");
    assert_eq!(sqlstate(&err), FOREIGN_KEY_VIOLATION, "{err:?}");

    // `pool_current_state` carries its own `protocol` column — real per-row data
    // written by the indexer, not a constant — so it is the one place the two
    // labels could genuinely be written apart.
    let err = sqlx::query(
        "INSERT INTO pool_current_state \
             (pool_address, protocol, last_event_at, last_event_kind, last_signature, \
              reserve_a, reserve_b, last_slot, last_event_index) \
         VALUES ($1, $2, NOW(), 'swap', 'sig', 0, 0,1,0)",
    )
    .bind(pk(1).to_string())
    .bind(Protocol::MeteoraDammV2.as_str())
    .execute(&pool)
    .await
    .expect_err("a state row labelled cp-amm for a DLMM pool must violate the composite key");
    assert_eq!(sqlstate(&err), FOREIGN_KEY_VIOLATION, "{err:?}");
}

/// The satellite's `protocol` column is `GENERATED ALWAYS`, so a writer cannot
/// name it — which is what stops the composite key from being talked around by
/// simply supplying the protocol the row wants to be.
///
/// The SQLSTATE is the whole test: `428C9` is "you wrote to a generated column",
/// and it is the only outcome that distinguishes the column being *there and
/// generated* from it being absent.
#[sqlx::test]
async fn the_satellite_protocol_column_cannot_be_written(pool: PgPool) {
    seed_pool(&pool, pk(1), Protocol::MeteoraDammV2, 1).await;

    let err = sqlx::query(
        "INSERT INTO meteora_damm_v2_pool_properties (pool_address, protocol) VALUES ($1, $2)",
    )
    .bind(pk(1).to_string())
    .bind(Protocol::MeteoraDammV2.as_str())
    .execute(&pool)
    .await
    .expect_err("a generated column must reject even the value it would compute");
    assert_eq!(sqlstate(&err), GENERATED_ALWAYS, "{err:?}");
}

/// The constraint seen from the repository, which is what yog-context actually
/// holds.
///
/// This scenario is **not reachable through the worker today**:
/// `context/src/workers/pool_account.rs` skips any pool whose decoded account
/// disagrees with the queue's protocol, before the resolver is called. What the
/// test pins is the contract the repository now offers regardless of who calls
/// it — a refused write, mapped to [`RepositoryError::Conflict`] like any other
/// foreign-key violation, so a future caller inherits skip-and-log rather than a
/// silent success.
///
/// Its mirror image, a *foreign payload* caught in Rust, is
/// `each_resolver_rejects_the_other_protocols_payload`.
#[sqlx::test]
async fn a_resolver_write_onto_a_foreign_pool_is_refused_by_the_schema(pool: PgPool) {
    seed_pool(&pool, pk(1), Protocol::MeteoraDlmm, 1).await;
    seed_pool(&pool, pk(2), Protocol::MeteoraDammV2, 2).await;
    let damm = PgMeteoraDammV2PoolPropertiesRepository::new(pool.clone());
    let dlmm = PgMeteoraDlmmPoolPropertiesRepository::new(pool.clone());

    let err = damm
        .set_pool_account(&pk(1), &properties_only(Some(BaseFeeKind::Constant)))
        .await
        .expect_err("a cp-amm payload on a DLMM pool must be refused by the database");
    assert!(
        matches!(err, RepositoryError::Conflict(_)),
        "a foreign-key violation must map to Conflict, got {err:?}"
    );

    let err = dlmm
        .set_pool_account(&pk(2), &dlmm_properties_only())
        .await
        .expect_err("a DLMM payload on a cp-amm pool must be refused by the database");
    assert!(matches!(err, RepositoryError::Conflict(_)), "got {err:?}");

    assert!(read_properties(&damm, &pk(1)).await.is_none());
    assert!(read_dlmm_properties(&dlmm, &pk(2)).await.is_none());
}

/// The invariant read from the other end: once a dependent row exists, the
/// registry can no longer relabel the pool underneath it.
///
/// Nothing updates `pools.protocol` today — `PgPoolRepository::upsert` writes it
/// on INSERT and its `ON CONFLICT` touches only `last_seen_at` — so this blocks
/// a bug rather than a workflow.
#[sqlx::test]
async fn a_pools_protocol_cannot_change_under_a_satellite_row(pool: PgPool) {
    seed_pool(&pool, pk(1), Protocol::MeteoraDammV2, 1).await;
    let satellite = PgMeteoraDammV2PoolPropertiesRepository::new(pool.clone());
    satellite
        .set_pool_account(&pk(1), &properties_only(Some(BaseFeeKind::Constant)))
        .await
        .expect("set_pool_account failed");

    let err = sqlx::query("UPDATE pools SET protocol = $2 WHERE pool_address = $1")
        .bind(pk(1).to_string())
        .bind(Protocol::MeteoraDlmm.as_str())
        .execute(&pool)
        .await
        .expect_err("relabelling a pool must be refused while a satellite row references it");
    assert_eq!(sqlstate(&err), FOREIGN_KEY_VIOLATION, "{err:?}");
}

/// The three satellite foreign keys are composite (baseline §8-§9), rebuilt
/// from single-column ones in migration 040. `ON DELETE CASCADE` came with the
/// originals, and losing it in that rebuild would leave orphan rows no code
/// path ever cleans up — so it is asserted rather than assumed.
#[sqlx::test]
async fn deleting_a_pool_still_cascades_to_its_dependents(pool: PgPool) {
    seed_pool(&pool, pk(1), Protocol::MeteoraDammV2, 1).await;
    seed_pool(&pool, pk(2), Protocol::MeteoraDlmm, 2).await;
    let damm = PgMeteoraDammV2PoolPropertiesRepository::new(pool.clone());
    let dlmm = PgMeteoraDlmmPoolPropertiesRepository::new(pool.clone());
    damm.set_pool_account(&pk(1), &properties_only(Some(BaseFeeKind::Constant)))
        .await
        .expect("set_pool_account failed");
    dlmm.set_pool_account(&pk(2), &dlmm_properties_only())
        .await
        .expect("set_pool_account failed");
    // A matching state row, written the way the indexer writes it: same protocol
    // as the registry, which is what the composite key now requires.
    sqlx::query(
        "INSERT INTO pool_current_state \
             (pool_address, protocol, last_event_at, last_event_kind, last_signature, \
              reserve_a, reserve_b, last_slot, last_event_index) \
         VALUES ($1, $2, NOW(), 'swap', 'sig', 0, 0,1,0)",
    )
    .bind(pk(1).to_string())
    .bind(Protocol::MeteoraDammV2.as_str())
    .execute(&pool)
    .await
    .expect("a state row agreeing with the registry must be accepted");

    sqlx::query("DELETE FROM pools")
        .execute(&pool)
        .await
        .expect("delete failed");

    assert!(read_properties(&damm, &pk(1)).await.is_none());
    assert!(read_dlmm_properties(&dlmm, &pk(2)).await.is_none());
    let orphans: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM pool_current_state")
        .fetch_one(&pool)
        .await
        .expect("count failed");
    assert_eq!(orphans.0, 0, "the state projection must cascade too");
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

/// The fee-scheduler curve makes the full round trip, and its absence is
/// written rather than skipped.
///
/// Six columns of one decoded curve. The test pins two things the upsert could
/// get wrong in opposite directions: that a curve lands intact (a swapped pair
/// among four same-typed integers would compile silently), and that a **later
/// read without a curve clears it** — plain `EXCLUDED`, deliberately unlike the
/// `COALESCE` that protects `base_fee_kind` one line above it. A pool whose fee
/// shape genuinely changed must not keep publishing a decay it no longer has:
/// a stale curve is confidently wrong, an absent one is visibly absent.
#[sqlx::test]
async fn the_fee_scheduler_curve_round_trips_and_can_be_cleared(pool: PgPool) {
    let repo = PgMeteoraDammV2PoolPropertiesRepository::new(pool.clone());
    seed_pool(&pool, pk(1), Protocol::MeteoraDammV2, 1).await;

    // `28BDU1…`'s real curve: 5000 bps decaying to 400 over 144 × 600 s.
    let scheduler = FeeSchedulerParams {
        cliff_fee_numerator: 500_000_000,
        number_of_period: 144,
        period_frequency: 600,
        reduction_factor: 3_194_444,
        activation_point: 1_785_180_416,
        activation_type: 1,
        kind: BaseFeeKind::SchedulerLinear,
    };
    let with_curve = PoolAccountProperties::MeteoraDammV2(MeteoraDammV2PoolAccountProperties {
        protocol_fee_percent: 20,
        referral_fee_percent: 20,
        base_fee_kind: Some(BaseFeeKind::SchedulerLinear),
        has_dynamic_fee: true,
        fee_scheduler: Some(scheduler),
    });

    repo.set_pool_account(&pk(1), &with_curve).await.unwrap();

    let row = sqlx::query!(
        r#"SELECT cliff_fee_numerator, number_of_period, period_frequency,
                  reduction_factor, activation_point, activation_type
           FROM meteora_damm_v2_pool_properties WHERE pool_address = $1"#,
        pk(1).to_string()
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(row.cliff_fee_numerator, Some(500_000_000));
    assert_eq!(row.number_of_period, Some(144));
    assert_eq!(row.period_frequency, Some(600));
    assert_eq!(row.reduction_factor, Some(3_194_444));
    assert_eq!(row.activation_point, Some(1_785_180_416));
    assert_eq!(row.activation_type, Some(1));

    // Now a read that establishes no curve — a constant pool, or an account too
    // short for the scheduler offsets. The six must go back to NULL.
    repo.set_pool_account(&pk(1), &properties_only(Some(BaseFeeKind::Constant)))
        .await
        .unwrap();

    let cleared = sqlx::query!(
        r#"SELECT cliff_fee_numerator, period_frequency, activation_type
           FROM meteora_damm_v2_pool_properties WHERE pool_address = $1"#,
        pk(1).to_string()
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(
        cleared.cliff_fee_numerator, None,
        "a curve must not outlive the shape it belonged to"
    );
    assert_eq!(cleared.period_frequency, None);
    assert_eq!(cleared.activation_type, None);
}

/// A curve that does not fit `BIGINT` costs the curve, not the whole satellite.
///
/// cp-amm bounds `cliff_fee_numerator`, `reduction_factor` and
/// `activation_point`, but bounds `period_frequency` by nothing beyond `!= 0` —
/// and pool creation is permissionless, so a `u64` past `i64::MAX` is writable
/// on chain today. Propagating that conversion failure with `?` would fail the
/// entire write: the percents, the fee shape and the dynamic-fee flag would not
/// land either, the worker would return before `set_registry_properties`, and
/// the pool would keep `needs_refresh` raised and a NULL satellite row — taking
/// a slot of every batch, forever, for a field nobody required.
#[sqlx::test]
async fn an_out_of_range_curve_costs_the_curve_and_nothing_else(pool: PgPool) {
    let repo = PgMeteoraDammV2PoolPropertiesRepository::new(pool.clone());
    seed_pool(&pool, pk(1), Protocol::MeteoraDammV2, 1).await;

    let props = PoolAccountProperties::MeteoraDammV2(MeteoraDammV2PoolAccountProperties {
        protocol_fee_percent: 20,
        referral_fee_percent: 20,
        base_fee_kind: Some(BaseFeeKind::SchedulerLinear),
        has_dynamic_fee: true,
        fee_scheduler: Some(FeeSchedulerParams {
            cliff_fee_numerator: 500_000_000,
            number_of_period: 144,
            // Past i64::MAX — no BIGINT holds it.
            period_frequency: u64::MAX,
            reduction_factor: 3_194_444,
            activation_point: 1_785_180_416,
            activation_type: 1,
            kind: BaseFeeKind::SchedulerLinear,
        }),
    });

    repo.set_pool_account(&pk(1), &props)
        .await
        .expect("an unstorable curve must not fail the write");

    let stored = read_properties(&repo, &pk(1))
        .await
        .expect("the satellite row must exist");
    assert_eq!(stored.protocol_fee_percent, Some(20));
    assert_eq!(stored.base_fee_kind.as_deref(), Some("scheduler_linear"));
    assert_eq!(stored.has_dynamic_fee, Some(true));
    assert!(
        stored.fee_scheduler.is_none(),
        "the curve is what is lost, and all that is lost"
    );
}
