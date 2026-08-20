//! Sentinels shared by every ring-2 round-trip test.

use chrono::{DateTime, Duration, TimeZone, Utc};
use solana_pubkey::Pubkey;
use solana_signature::Signature;
use sqlx::PgPool;

pub fn pk(seed: u8) -> Pubkey {
    Pubkey::new_from_array([seed; 32])
}

/// SQLSTATE of a failed statement, or the test dies with the error it did get.
///
/// The reason the schema tests do not settle for a bare `expect_err`: every one
/// of them asserts that a *specific* constraint fired, and an unqualified "some
/// error happened" is satisfied by the constraint not existing at all. Drop the
/// generated column and `INSERT … (protocol)` still fails — with `42703`,
/// undefined_column. Same green, nothing proven.
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
pub fn sg() -> Signature {
    Signature::from([7u8; 64])
}
pub fn ts() -> DateTime<Utc> {
    Utc.timestamp_opt(1_700_000_000, 0).unwrap()
}

/// Price a mint the way `yog-context` actually does: one observation per hour,
/// from `since_hours_ago` up to now.
///
/// ⚠️ **Use this rather than a single `INSERT INTO token_prices`.** Since
/// migration 005 the as-of lookup takes the most recent price at or before a
/// bucket's START and no older than `yog_price_max_age_asof()` (one hour), so
/// **one row prices exactly one bucket** — and a fixture that seeds one row far
/// in the past prices *nothing at all*.
///
/// That failure is silent, which is the whole reason this lives here: the test
/// still compiles, still runs, and its assertions pass over zeros. Sixteen
/// pre-existing tests were caught by it when 005 landed; every one had been
/// green for months while proving less than it claimed.
///
/// A test that needs a mint unpriced before some point expresses it by starting
/// the series there — that is *absence*, which the staleness policy deliberately
/// does not treat.
pub async fn price_mint_since(pool: &PgPool, mint: &str, price: &str, since_hours_ago: i64) {
    let now = Utc::now();
    for h in 0..=since_hours_ago {
        sqlx::query(
            "INSERT INTO token_prices (mint, price_usd, price_provider, fetched_at)
             VALUES ($1,$2::NUMERIC,'jupiter',$3)",
        )
        .bind(mint)
        .bind(price)
        .bind(now - Duration::hours(h))
        .execute(pool)
        .await
        .unwrap();
    }
}

/// A continuously-priced mint, covering any bucket a fixture is likely to place.
pub async fn price_mint(pool: &PgPool, mint: &str, price: &str) {
    price_mint_since(pool, mint, price, 48).await;
}

/// One `ClaimReward` event, the way the indexer writes it.
///
/// Shared because the reward aggregate is read from two angles — `claim_caggs`
/// checks that it keeps one row PER MINT, `reward_valuation` checks what the
/// activity view then does with those rows — and both need the same insert. Two
/// copies of it would drift apart the day the column list moves.
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

/// Drop the positivity constraint of migration 009 so a fixture can seed a price
/// that stores as zero.
///
/// ⚠️ **Only for tests of the layer *below* the constraint.** Since 009 a zero
/// cannot be written through any normal path, so a test that seeds one is, on
/// the face of it, testing a state that cannot occur — which this file's own
/// fixtures rule would forbid.
///
/// It is worth testing anyway, and the distinction is which layer is under
/// examination. The constraint stops a zero from being *stored*; the
/// `NULLIF(price_usd, 0)` pair inside `meteora_damm_v2_swap_events_hourly_priced`
/// (live definition in migration **007**, not the 002 that introduced it) stops
/// a zero that IS stored from being *valued*. The second is still
/// reachable — a restore from a pre-009 backup, a repair script run with the
/// constraint dropped, a future migration that relaxes it — and it is the layer
/// that decides whether a bad row becomes a fabricated number or an honest NULL.
///
/// So: call this only where the assertion is about the view's arithmetic.
/// `price_positivity.rs` is where the constraint itself is asserted, and it must
/// never call this.
pub async fn allow_zero_prices(pool: &PgPool) {
    sqlx::query("ALTER TABLE token_prices DROP CONSTRAINT token_prices_price_usd_positive")
        .execute(pool)
        .await
        .expect("migration 009 must have created the constraint this fixture removes");
}
