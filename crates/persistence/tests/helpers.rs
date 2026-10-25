//! Sentinels shared by every ring-2 round-trip test.

use std::str::FromStr;

use chrono::{DateTime, Duration, DurationRound, TimeZone, Utc};
use rust_decimal::Decimal;
use solana_pubkey::Pubkey;
use solana_signature::Signature;
use sqlx::PgPool;
use yog_core::domain::{
    AnalyticRepository, CycleWrite, FrozenPeriod, PriceProvider, TokenPrice, TokenPriceRepository,
};
use yog_persistence::{PgAnalyticRepository, PgTokenPriceRepository};

pub fn pk(seed: u8) -> Pubkey {
    Pubkey::new_from_array([seed; 32])
}

/// SQLSTATE of a failed statement, or the test dies with the error it got. ⚠️ A bare
/// `expect_err` is also satisfied by the constraint not existing at all.
pub fn sqlstate(err: &sqlx::Error) -> String {
    err.as_database_error()
        .and_then(|e| e.code())
        .unwrap_or_else(|| panic!("expected a database error, got {err:?}"))
        .into_owned()
}

/// `23514` — check_violation.
pub const CHECK_VIOLATION: &str = "23514";
/// `22003` — numeric_value_out_of_range, raised by the column TYPE during
/// coercion, before any constraint is consulted.
pub const NUMERIC_OVERFLOW: &str = "22003";
/// `23505` — unique_violation.
pub const UNIQUE_VIOLATION: &str = "23505";
/// `42501` — insufficient_privilege.
pub const INSUFFICIENT_PRIVILEGE: &str = "42501";
/// `2BP01` — dependent_objects_still_exist.
pub const DEPENDENT_OBJECTS_STILL_EXIST: &str = "2BP01";

/// The provisioning script, which no migration runs: a test that needs the
/// roles or what the file takes back from `PUBLIC` applies it itself.
pub const SETUP_ROLES_SQL: &str = include_str!("../src/bin/scripts/setup_roles.sql");

/// Held by every test that applies [`SETUP_ROLES_SQL`]. ⚠️ Roles are cluster-wide while each
/// test has its own database, and advisory locks are per-database: only a process lock serialises.
pub static CLUSTER_ROLES: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// The refusal `pg_read_all_data` would lift: every role's password hash.
pub async fn assert_reads_no_password_hash(conn: &mut sqlx::PgConnection) {
    let err = sqlx::query("SELECT rolpassword FROM pg_authid")
        .execute(conn)
        .await
        .expect_err("a runtime role reads no password hash");
    assert_eq!(sqlstate(&err), INSUFFICIENT_PRIVILEGE);
    let message = err.as_database_error().unwrap().message().to_owned();
    assert!(message.contains("table pg_authid"), "{message}");
}

/// Apply [`SETUP_ROLES_SQL`] to the test's database, under [`CLUSTER_ROLES`].
pub async fn apply_setup_roles(pool: &PgPool) {
    let _cluster = CLUSTER_ROLES.lock().await;
    sqlx::raw_sql(SETUP_ROLES_SQL)
        .execute(pool)
        .await
        .expect("apply setup_roles.sql");
}

/// A pool on the test's database whose connections act as `role`, after
/// [`apply_setup_roles`].
pub async fn setup_roles_and_connect_as(pool: &PgPool, role: &str) -> PgPool {
    apply_setup_roles(pool).await;
    let options = (*pool.connect_options()).clone().options([("role", role)]);
    let scoped = PgPool::connect_with(options).await.unwrap();
    let user: String = sqlx::query_scalar("SELECT current_user::TEXT")
        .fetch_one(&scoped)
        .await
        .unwrap();
    assert_eq!(user, role);
    scoped
}

/// Every view, materialized view and function of `public`, as `(name, definition)`.
///
/// ⚠️ `pg_matviews` and `pg_proc` too: a definition moved there escapes a `pg_views` guard.
pub async fn catalog_definitions(pool: &PgPool) -> Vec<(String, String)> {
    // ⚠️ `pg_get_functiondef`, not `prosrc`: a `BEGIN ATOMIC` body leaves `prosrc` empty.
    // It raises on an aggregate, hence `prokind`.
    sqlx::query_as(
        "SELECT viewname::TEXT, definition FROM pg_views WHERE schemaname = 'public'
         UNION ALL
         SELECT matviewname::TEXT, definition FROM pg_matviews WHERE schemaname = 'public'
         UNION ALL
         SELECT proname::TEXT, pg_get_functiondef(oid) FROM pg_proc
         WHERE pronamespace = 'public'::regnamespace AND prokind <> 'a'
         ORDER BY 1",
    )
    .fetch_all(pool)
    .await
    .unwrap()
}

pub fn sg() -> Signature {
    Signature::from([7u8; 64])
}
/// The `next_sqrt_price` of a pool that trades `amount_a` against `amount_b`
/// (raw units): the price, Q64.64, under its square root.
pub fn sqrt_price_x64(amount_a: i64, amount_b: i64) -> sqlx::types::BigDecimal {
    let sqrt = (amount_b as f64 / amount_a as f64).sqrt() * 2f64.powi(64);
    sqlx::types::BigDecimal::from_str(&format!("{}", sqrt as u128)).unwrap()
}

/// What yog-analytic writes of every tranche and hour from `first` to `last`
/// hours back on the database's clock, nothing frozen.
pub async fn write_facts_spanning(pool: &PgPool, first: i64, last: i64) {
    let now: DateTime<Utc> = sqlx::query_scalar("SELECT now()")
        .fetch_one(pool)
        .await
        .unwrap();
    let hour_of = |at: DateTime<Utc>| at.duration_trunc(Duration::hours(1)).unwrap();
    let cycle = CycleWrite {
        tranches_from: hour_of(now - Duration::hours(first)),
        tranches_through: hour_of(now - Duration::hours(last)) + Duration::minutes(55),
        from: hour_of(now - Duration::hours(first)),
        through: hour_of(now - Duration::hours(last)),
        freeze_through: None,
        tvl_hour: None,
        at: now,
    };
    PgAnalyticRepository::new(pool.clone())
        .write_cycle(&cycle)
        .await
        .unwrap();
}

pub fn ts() -> DateTime<Utc> {
    Utc.timestamp_opt(1_700_000_000, 0).unwrap()
}

/// Prices a mint as yog-context does: one observation per hour, from `since_hours_ago`.
/// ⚠️ Not a single `INSERT`: the as-of lookup reads one hour back, so one row prices one bucket,
/// and a row far in the past prices nothing while the assertions pass over zeros.
pub async fn price_mint_since(pool: &PgPool, mint: &str, price: &str, since_hours_ago: i64) {
    let now = Utc::now();
    let series: Vec<TokenPrice> = (0..=since_hours_ago)
        .map(|h| token_price(mint, price, now - Duration::hours(h)))
        .collect();
    record_prices(pool, &series).await;
}

/// One price at `at`, written like [`price_mint_since`].
pub async fn price_at(pool: &PgPool, mint: &str, price: &str, at: DateTime<Utc>) {
    record_prices(pool, &[token_price(mint, price, at)]).await;
}

pub fn token_price(mint: &str, price: &str, at: DateTime<Utc>) -> TokenPrice {
    TokenPrice {
        mint: Pubkey::from_str(mint).expect("a priced fixture mint must be a pubkey"),
        price_usd: Decimal::from_str(price).unwrap(),
        price_provider: PriceProvider::Jupiter,
        confidence: None,
        fetched_at: at,
    }
}

/// Write prices the way yog-context does.
pub async fn record_prices(pool: &PgPool, prices: &[TokenPrice]) {
    PgTokenPriceRepository::new(pool.clone())
        .insert_batch(prices)
        .await
        .unwrap();
}

/// A continuously-priced mint, covering any bucket a fixture is likely to place.
pub async fn price_mint(pool: &PgPool, mint: &str, price: &str) {
    price_mint_since(pool, mint, price, 48).await;
}

/// One `ClaimReward` event, the way the indexer writes it.
pub async fn claim_reward(
    pool: &PgPool,
    pool_addr: &str,
    signature: &str,
    mint_reward: &str,
    reward_index: i16,
    total_reward: i64,
    timestamp: DateTime<Utc>,
) {
    sqlx::query(
        "INSERT INTO meteora_damm_v2_claim_reward_events
           (pool_address, signature, position, owner, mint_reward, reward_index, total_reward, timestamp, slot, event_index)
         VALUES ($1,$2,'pos','own',$3,$4,$5,$6,0,0)",
    )
    .bind(pool_addr)
    .bind(signature)
    .bind(mint_reward)
    .bind(reward_index)
    .bind(total_reward)
    .bind(timestamp)
    .execute(pool)
    .await
    .unwrap();
}

/// Drops the positivity constraint so a fixture can store a zero price. ⚠️ Only for tests of
/// the view's `NULLIF` below the constraint; `price_positivity.rs` asserts the constraint and
/// must never call it.
pub async fn allow_zero_prices(pool: &PgPool) {
    sqlx::query("ALTER TABLE token_prices DROP CONSTRAINT token_prices_price_usd_positive")
        .execute(pool)
        .await
        .expect("migration 009 must have created the constraint this fixture removes");
}

/// One bucket of `meteora_damm_v2_pool_hourly_activity`, its swap columns only.
#[derive(Debug)]
pub struct SwapActivity {
    pub swap_count: Option<i64>,
    pub volume_usd: Option<Decimal>,
    pub fees_usd: Option<Decimal>,
    pub protocol_fees_usd: Option<Decimal>,
    pub referral_fees_usd: Option<Decimal>,
    pub lp_fees_usd: Option<Decimal>,
}

/// Every bucket of a pool in the activity view, oldest first.
pub async fn swap_activity(pool: &PgPool, pool_addr: &str) -> Vec<SwapActivity> {
    use sqlx::Row;
    sqlx::query(
        "SELECT swap_count, volume_usd, fees_usd, protocol_fees_usd, referral_fees_usd,
                lp_fees_usd
           FROM meteora_damm_v2_pool_hourly_activity
          WHERE pool_address = $1
          ORDER BY bucket",
    )
    .bind(pool_addr)
    .fetch_all(pool)
    .await
    .unwrap()
    .into_iter()
    .map(|r| SwapActivity {
        swap_count: r.get("swap_count"),
        volume_usd: r.get("volume_usd"),
        fees_usd: r.get("fees_usd"),
        protocol_fees_usd: r.get("protocol_fees_usd"),
        referral_fees_usd: r.get("referral_fees_usd"),
        lp_fees_usd: r.get("lp_fees_usd"),
    })
    .collect()
}

/// The pool's only bucket in the activity view; the test dies on any other count.
pub async fn the_only_bucket(pool: &PgPool, pool_addr: &str) -> SwapActivity {
    let mut buckets = swap_activity(pool, pool_addr).await;
    assert_eq!(buckets.len(), 1, "expected one bucket: {buckets:?}");
    buckets.remove(0)
}

pub const DAMM: &str = "meteora_damm_v2";
pub const DLMM: &str = "meteora_dlmm";

/// The close of the last frozen hour of the produced-resource reads: a day never starts at
/// midnight.
pub fn as_of() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 10, 20, 13, 0, 0).unwrap()
}

/// `hours` hours ending at [`as_of`], the facts holding them all.
pub fn period(hours: i64) -> FrozenPeriod {
    FrozenPeriod {
        from: as_of() - Duration::hours(hours),
        as_of: as_of(),
        covered_since: as_of() - Duration::hours(hours),
    }
}
