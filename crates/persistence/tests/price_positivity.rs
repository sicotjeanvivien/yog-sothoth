//! Integration tests for migration 009 — a price of zero is not a price.
//!
//! Four things are asserted here, and the last two are what keep the first two
//! from being decoration:
//!
//!   1. the constraint refuses a zero, with the **precise** SQLSTATE;
//!   2. it refuses `4e-19` too — a *positive* value, which only becomes zero
//!      when coerced to `NUMERIC(38, 18)`. This is the assertion that states
//!      the real rule: the bound is the column's scale, not the sign;
//!   3. **the guard is put to the sword**: with the constraint dropped, the
//!      very same insert succeeds. Without this, a green here is also produced
//!      by a constraint that does not exist, or by the value never reaching the
//!      column at all;
//!   4. the constraint is **validated**, not `NOT VALID` — the difference does
//!      not show up in any insert, only in `pg_constraint`, so nothing else
//!      would notice it silently shipping in the weaker form.
//!
//! And one that binds the two halves of the rule together: `agreement_…` checks
//! that `TokenPrice::is_storable` (the writer-side filter in `yog-context`)
//! accepts exactly what Postgres accepts. The rule is necessarily stated twice —
//! once in Rust so a batch is never aborted, once in SQL so the guarantee lives
//! in the schema — and this is what stops the two statements from drifting.

use super::helpers::{CHECK_VIOLATION, NUMERIC_OVERFLOW, pk, sqlstate};
use chrono::Utc;
use rust_decimal::Decimal;
use sqlx::PgPool;
use std::str::FromStr;
use yog_core::domain::{PriceProvider, TokenPrice};

const CONSTRAINT: &str = "token_prices_price_usd_positive";

fn dec(s: &str) -> Decimal {
    Decimal::from_str(s).expect("test literal must parse as Decimal")
}

/// Insert one price the way `insert_batch` does — a bound `Decimal`, not a SQL
/// literal, so the coercion under test is the one production performs.
async fn insert_price(pool: &PgPool, mint: &str, price: Decimal) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO token_prices (mint, price_usd, price_provider, fetched_at)
         VALUES ($1, $2, 'jupiter', $3)",
    )
    .bind(mint)
    .bind(price)
    .bind(Utc::now())
    .execute(pool)
    .await
    .map(|_| ())
}

#[sqlx::test]
async fn zero_is_refused(pool: PgPool) {
    let err = insert_price(&pool, "MintZero", dec("0"))
        .await
        .expect_err("a zero price must not be storable");

    assert_eq!(sqlstate(&err), CHECK_VIOLATION);
}

#[sqlx::test]
async fn negative_is_refused(pool: PgPool) {
    let err = insert_price(&pool, "MintNeg", dec("-1.25"))
        .await
        .expect_err("a negative price must not be storable");

    assert_eq!(sqlstate(&err), CHECK_VIOLATION);
}

#[sqlx::test]
async fn a_positive_price_below_the_column_scale_is_refused(pool: PgPool) {
    // 4e-19 is positive everywhere until it reaches the column, where it rounds
    // to 0.000000000000000000. A `price_usd > 0` filter in the writer would let
    // this through and the row would land as a zero — which is the entire bug.
    let err = insert_price(&pool, "MintDust", dec("0.0000000000000000004"))
        .await
        .expect_err("a price that rounds to zero must not be storable");

    assert_eq!(sqlstate(&err), CHECK_VIOLATION);
}

#[sqlx::test]
async fn the_midpoint_rounds_up_and_is_accepted(pool: PgPool) {
    // 5e-19 rounds AWAY from zero under Postgres's NUMERIC rules and lands on
    // 1e-18. The constraint must not be over-eager: this is a real price the
    // column can hold, and refusing it would lose data the schema can carry.
    insert_price(&pool, "MintMidpoint", dec("0.0000000000000000005"))
        .await
        .expect("5e-19 rounds to 1e-18 and must be accepted");

    let stored: Decimal = sqlx::query_scalar("SELECT price_usd FROM token_prices WHERE mint = $1")
        .bind("MintMidpoint")
        .fetch_one(&pool)
        .await
        .unwrap();

    assert_eq!(stored, dec("0.000000000000000001"));
}

#[sqlx::test]
async fn dropping_the_constraint_lets_the_zero_through(pool: PgPool) {
    // The mutation. Everything above is satisfied by a database that refuses
    // the insert for some *other* reason; this is what pins the refusal on the
    // constraint of migration 009, by removing it and watching the same value
    // land — as a zero, non-NULL, exactly the state the migration exists to
    // prevent.
    sqlx::query(&format!(
        "ALTER TABLE token_prices DROP CONSTRAINT {CONSTRAINT}"
    ))
    .execute(&pool)
    .await
    .expect("the constraint must exist to be dropped — if this fails, 009 did not apply");

    insert_price(&pool, "MintDust", dec("0.0000000000000000004"))
        .await
        .expect("without the constraint the dust price is accepted");

    let (stored, is_zero, is_null): (Decimal, bool, bool) = sqlx::query_as(
        "SELECT price_usd, price_usd = 0, price_usd IS NULL
         FROM token_prices WHERE mint = $1",
    )
    .bind("MintDust")
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(stored, Decimal::ZERO);
    // The pair that names the defect: it reads as a number, not as an absence,
    // so every `price_usd IS NULL` guard downstream waves it through.
    assert!(is_zero, "the stored value is zero");
    assert!(
        !is_null,
        "and it is NOT null — which is why the guards miss it"
    );
}

#[sqlx::test]
async fn an_overflowing_price_is_refused_by_the_type_not_the_constraint(pool: PgPool) {
    // The ceiling is not enforced by 009 and cannot be: `NUMERIC(38, 18)`
    // rejects the value while coercing it, before any CHECK is consulted. The
    // SQLSTATE is what proves which layer fired — asserting merely "it failed"
    // would let someone believe the constraint covers this end too, and write a
    // one-sided filter on that belief.
    let err = insert_price(&pool, "MintHuge", dec("100000000000000000000"))
        .await
        .expect_err("1e20 does not fit NUMERIC(38, 18)");

    assert_eq!(
        sqlstate(&err),
        NUMERIC_OVERFLOW,
        "expected 22003 from the column type, not 23514 from the CHECK"
    );
}

#[sqlx::test]
async fn the_constraint_is_validated_not_merely_declared(pool: PgPool) {
    // `NOT VALID` would pass every test above: it refuses new rows just the
    // same. What it would not do is prove anything about the rows already
    // stored, and the whole argument for the validating form was that there
    // were none to fear. This is the only place that difference is visible.
    let (validated, definition): (bool, String) = sqlx::query_as(
        "SELECT convalidated, pg_get_constraintdef(oid)
         FROM pg_constraint WHERE conname = $1 AND conrelid = 'token_prices'::regclass",
    )
    .bind(CONSTRAINT)
    .fetch_one(&pool)
    .await
    .expect("migration 009 must have created the constraint on token_prices");

    assert!(
        validated,
        "009 must ship a VALIDATED constraint, not NOT VALID"
    );
    assert!(
        !definition.contains("NOT VALID"),
        "constraint definition still carries NOT VALID: {definition}"
    );
}

/// The contract on `TokenPriceRepository::insert_batch` is enforced by a
/// `debug_assert`, so a caller that forgets to filter fails here rather than
/// silently dropping a whole tick in production. Gated on `debug_assertions`
/// because that is exactly when the assertion exists — a `--release` run must
/// not report a failure for a guard that was compiled out.
#[cfg(debug_assertions)]
#[sqlx::test]
#[should_panic(expected = "the caller must filter")]
async fn insert_batch_refuses_an_unfiltered_batch_in_debug(pool: PgPool) {
    use yog_core::domain::TokenPriceRepository;
    use yog_persistence::PgTokenPriceRepository;

    let repo = PgTokenPriceRepository::new(pool);
    let _ = repo
        .insert_batch(&[TokenPrice {
            mint: pk(1),
            price_usd: dec("0.0000000000000000004"),
            price_provider: PriceProvider::Jupiter,
            confidence: None,
            fetched_at: Utc::now(),
        }])
        .await;
}

#[sqlx::test]
async fn agreement_between_the_rust_filter_and_the_constraint(pool: PgPool) {
    // The rule is stated twice on purpose — `TokenPrice::is_storable` so that a
    // batch insert is never aborted, and the CHECK so the guarantee lives in
    // the schema. Two statements of one rule drift; this is the test that does
    // not let them.
    //
    // Every value straddling the boundary, plus ordinary ones for shape.
    // Both ends: the CHECK guards the low one, the column TYPE guards the high
    // one (`22003`, before any constraint runs). `is_storable` has to agree with
    // whichever of the two refuses, because the writer cannot tell them apart —
    // both abort the batch identically.
    let cases = [
        "1.25",
        "0.000001381771567258", // the smallest price actually observed in dev
        "0.000000000000000001", // 1e-18, one unit at the column's scale
        "0.0000000000000000006",
        "0.0000000000000000005", // the midpoint: rounds up, storable
        "0.0000000000000000004", // just below: rounds to zero
        "0.0000000000000000001",
        "0.0000000000000000000000001",
        "0",
        "-1.25",
        "99999999999999999999",  // 20 integer digits — the largest storable
        "100000000000000000000", // 1e20 — overflows the type
        "1000000000000000000000000",
    ];

    for (i, literal) in cases.iter().enumerate() {
        let value = dec(literal);
        let rust_says = TokenPrice {
            mint: pk(1),
            price_usd: value,
            price_provider: PriceProvider::Jupiter,
            confidence: None,
            fetched_at: Utc::now(),
        }
        .is_storable();

        let mint = format!("MintCase{i}");
        let postgres_says = insert_price(&pool, &mint, value).await.is_ok();

        assert_eq!(
            rust_says, postgres_says,
            "disagreement on {literal}: is_storable() = {rust_says}, \
             but Postgres accepted = {postgres_says}. The writer-side filter and \
             the constraint no longer describe the same rule."
        );
    }
}
