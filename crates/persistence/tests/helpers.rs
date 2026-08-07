//! Sentinels shared by every ring-2 round-trip test.

use chrono::{DateTime, Duration, TimeZone, Utc};
use solana_pubkey::Pubkey;
use solana_signature::Signature;
use sqlx::PgPool;

pub fn pk(seed: u8) -> Pubkey {
    Pubkey::new_from_array([seed; 32])
}
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
