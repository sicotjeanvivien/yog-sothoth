//! A raw amount is scaled by `10^decimals` in `yog_token_amount` alone, valued by `yog_amount_usd`
//! over it, and by `yog_moved_amount_usd` for an amount that moved. Nothing else in the schema or
//! in this crate's SQL scales an amount, nor rewrites the empty leg around the valuation.

use std::path::Path;

use sqlx::{AssertSqlSafe, PgPool};

use super::helpers::catalog_definitions;

/// Lowercase, without whitespace, and with Postgres's deparsed casts of `10` and `0` brought
/// back to the bare number, so a source and a catalog read alike.
fn normalize(text: &str) -> String {
    text.chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>()
        .to_lowercase()
        .replace("(10)::numeric", "10")
        .replace("(10)::doubleprecision", "10")
        .replace("(0)::numeric", "0")
}

fn scales_an_amount(text: &str) -> bool {
    let text = normalize(text);
    ["power(10", "pow(10", "10^"]
        .iter()
        .any(|spelling| text.contains(spelling))
}

fn rewrites_the_empty_leg(text: &str) -> bool {
    normalize(text).contains("then0elseyog_amount_usd(")
}

/// The catalog's definitions, then every `.rs` and `.sql` under this crate's `src/`, as
/// `(name, text)`.
async fn sources(pool: &PgPool) -> Vec<(String, String)> {
    let mut sources = catalog_definitions(pool).await;

    let mut dirs = vec![Path::new(env!("CARGO_MANIFEST_DIR")).join("src")];
    while let Some(dir) = dirs.pop() {
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                dirs.push(path);
            } else if path.extension().is_some_and(|e| e == "rs" || e == "sql") {
                let text = std::fs::read_to_string(&path).unwrap();
                sources.push((path.display().to_string(), text));
            }
        }
    }
    sources
}

#[sqlx::test]
async fn every_amount_scaling_goes_through_yog_token_amount(pool: PgPool) {
    // Mutations: the formula written back in one of migration 025's views, in the implied price
    // of 026, or in the analytic swap facts — each turns this red.
    for spelling in [
        "(x::NUMERIC / POWER(10::NUMERIC, d))",
        "(x / power((10)::numeric, (d)::numeric))",
        "(x / power((10)::double precision, (d)::double precision))",
        "(x / pow(10, d))",
        "(x / ((10)::numeric ^ (d)::numeric))",
    ] {
        assert!(
            scales_an_amount(spelling),
            "the match must catch {spelling}, as written or as Postgres deparses it"
        );
    }

    let sources = sources(&pool).await;
    let definition = sources
        .iter()
        .find(|(name, _)| name == "yog_token_amount")
        .expect("yog_token_amount must be in the catalog");
    assert!(
        scales_an_amount(&definition.1),
        "the one definition must itself be found, or the match below proves nothing"
    );

    let copies: Vec<&str> = sources
        .iter()
        .filter(|(name, text)| name != "yog_token_amount" && scales_an_amount(text))
        .map(|(name, _)| name.as_str())
        .collect();
    assert!(
        copies.is_empty(),
        "these scale an amount by 10^decimals themselves; call yog_token_amount, \
         yog_amount_usd for its USD value, or yog_moved_amount_usd for an amount that \
         moved: {copies:?}"
    );
}

#[sqlx::test]
async fn no_empty_leg_is_written_around_yog_amount_usd(pool: PgPool) {
    // Mutation: `CASE WHEN COALESCE(x, 0) = 0 THEN 0 ELSE yog_amount_usd(…) END` in a view.
    let sources = sources(&pool).await;
    let definition = sources
        .iter()
        .find(|(name, _)| name == "yog_moved_amount_usd")
        .expect("yog_moved_amount_usd must be in the catalog");
    assert!(
        rewrites_the_empty_leg(&definition.1),
        "the one definition must itself be found, or the match below proves nothing"
    );

    let copies: Vec<&str> = sources
        .iter()
        .filter(|(name, text)| name != "yog_moved_amount_usd" && rewrites_the_empty_leg(text))
        .map(|(name, _)| name.as_str())
        .collect();
    assert!(
        copies.is_empty(),
        "these write the empty leg around yog_amount_usd; call yog_moved_amount_usd: {copies:?}"
    );
}

#[sqlx::test]
async fn the_amount_functions_are_inlined(pool: PgPool) {
    // Mutation: a `SET` clause on `yog_token_amount` — Postgres stops inlining it, and every
    // amount of the hourly views and the analytic facts becomes a per-row call.
    for view in [
        "meteora_damm_v2_pool_hourly_activity",
        "meteora_damm_v2_pool_hourly_liquidity_flow",
        "meteora_damm_v2_liquidity_events_valued",
        "meteora_damm_v2_swap_events_hourly_priced",
        "pool_current_tvl",
    ] {
        let plan: Vec<String> = sqlx::query_scalar(AssertSqlSafe(format!(
            "EXPLAIN (VERBOSE, COSTS OFF) SELECT * FROM {view}"
        )))
        .fetch_all(&pool)
        .await
        .unwrap();
        let calls: Vec<&str> = ["yog_token_amount", "yog_amount_usd", "yog_moved_amount_usd"]
            .into_iter()
            .filter(|function| plan.iter().any(|line| line.contains(function)))
            .collect();
        assert!(
            calls.is_empty(),
            "{view} calls {calls:?} instead of inlining them"
        );
    }
}
