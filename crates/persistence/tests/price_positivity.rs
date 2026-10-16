//! A price of zero is not a price. The constraint refuses `4e-19`, which only becomes zero
//! in `NUMERIC(38, 18)`, with the precise SQLSTATE; dropping it lets the same insert land;
//! it is validated, not `NOT VALID`. And `TokenPrice::is_storable` accepts exactly what
//! Postgres accepts, so the rule's two statements cannot drift.

use super::helpers::{CHECK_VIOLATION, NUMERIC_OVERFLOW, pk, sqlstate};
use chrono::Utc;
use rust_decimal::Decimal;
use sqlx::{AssertSqlSafe, PgPool};
use std::str::FromStr;
use yog_core::domain::{PriceProvider, TokenPrice};

const CONSTRAINT: &str = "token_prices_price_usd_positive";

fn dec(s: &str) -> Decimal {
    Decimal::from_str(s).expect("test literal must parse as Decimal")
}

/// Inserts one price as `insert_batch` does — a bound `Decimal`, so the coercion is production's.
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
    // Positive until the column rounds it to zero: a writer's `> 0` filter lets it through.
    let err = insert_price(&pool, "MintDust", dec("0.0000000000000000004"))
        .await
        .expect_err("a price that rounds to zero must not be storable");

    assert_eq!(sqlstate(&err), CHECK_VIOLATION);
}

#[sqlx::test]
async fn the_midpoint_rounds_up_and_is_accepted(pool: PgPool) {
    // Rounds away from zero, to 1e-18: a price the column holds, which must not be refused.
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
    // The mutation: without the constraint, the same value lands as a zero.
    // `AssertSqlSafe`: a DDL identifier cannot be bound; only the constant is interpolated.
    sqlx::query(AssertSqlSafe(format!(
        "ALTER TABLE token_prices DROP CONSTRAINT {CONSTRAINT}"
    )))
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
    // A number, not an absence: every `IS NULL` guard downstream waves it through.
    assert!(is_zero, "the stored value is zero");
    assert!(
        !is_null,
        "and it is NOT null — which is why the guards miss it"
    );
}

#[sqlx::test]
async fn an_overflowing_price_is_refused_by_the_type_not_the_constraint(pool: PgPool) {
    // The type refuses this one before any CHECK: the SQLSTATE proves which layer fired.
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
    // `NOT VALID` passes every insert above: only the catalog shows the difference.
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

/// The `debug_assert` on `insert_batch` fails a caller that forgets to filter. Gated on
/// `debug_assertions`: a release build compiles the guard out.
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
    // Both ends of the boundary: the CHECK refuses the low one, the type (`22003`) the high one,
    // and `is_storable` must agree with either — both abort a batch alike.
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
