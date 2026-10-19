//! A raw amount is scaled by `10^decimals` in `yog_token_amount` alone, valued by `yog_amount_usd`
//! over it, and by `yog_moved_amount_usd` for an amount that moved. Nothing else in the schema or
//! in this crate's SQL scales an amount, nor rewrites the empty leg around the valuation.

use std::path::Path;

use sqlx::PgPool;

use super::helpers::catalog_definitions;

/// Lowercase, without whitespace, and with Postgres's deparsed `(10)::numeric` and
/// `(0)::numeric` brought back to `10` and `0`, so a source and a catalog read alike.
fn normalize(text: &str) -> String {
    text.chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>()
        .to_lowercase()
        .replace("(10)::numeric", "10")
        .replace("(0)::numeric", "0")
}

fn scales_an_amount(text: &str) -> bool {
    normalize(text).contains("power(10")
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
    assert!(
        scales_an_amount("(x::NUMERIC / POWER(10::NUMERIC, d))")
            && scales_an_amount("(x / power((10)::numeric, (d)::numeric))"),
        "the match must catch the source spelling and the one Postgres deparses"
    );

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
