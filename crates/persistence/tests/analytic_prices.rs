//! The price table of `PgAnalyticRepository` (028): each pool's two tokens priced once per
//! 5-minute tranche — the anchor at its market price, the other bounded by the pool's own —
//! the swaps valued at them, the hours summed from them, and no swap valued anywhere else.

use std::path::Path;

use chrono::{DateTime, Duration, DurationRound, Utc};
use sqlx::PgPool;
use sqlx::types::BigDecimal;
use yog_core::domain::{AnalyticProgress, AnalyticRepository, CycleOutcome, CycleWrite};
use yog_persistence::PgAnalyticRepository;

use super::helpers::{as_role, pk, price_at, sqrt_price_x64};

/// A tranche three hours back, ten minutes into its hour: settled, and clear of
/// the hour's first and last tranches.
fn settled_tranche() -> DateTime<Utc> {
    (Utc::now() - Duration::hours(3))
        .duration_trunc(Duration::hours(1))
        .unwrap()
        + Duration::minutes(10)
}

fn hour_of(at: DateTime<Utc>) -> DateTime<Utc> {
    at.duration_trunc(Duration::hours(1)).unwrap()
}

/// A pool of `(mint, decimals)` for each side.
async fn setup_pool(pool: &PgPool, seed: u8, a: (&str, i16), b: (&str, i16)) -> String {
    let address = pk(seed).to_string();
    sqlx::query(
        "INSERT INTO pools (pool_address, protocol, token_a_mint, token_b_mint)
         VALUES ($1, 'meteora_damm_v2', $2, $3)",
    )
    .bind(&address)
    .bind(a.0)
    .bind(b.0)
    .execute(pool)
    .await
    .unwrap();
    for (mint, decimals) in [a, b] {
        sqlx::query(
            "INSERT INTO token_metadata (mint, decimals, fetched_at, last_refresh_at)
             VALUES ($1, $2, now(), now())
             ON CONFLICT (mint) DO NOTHING",
        )
        .bind(mint)
        .bind(decimals)
        .execute(pool)
        .await
        .unwrap();
    }
    address
}

/// A swap A into B, the pool left at `sqrt_price`.
async fn swap(
    pool: &PgPool,
    address: &str,
    tag: &str,
    (amount_a, amount_b): (i64, i64),
    sqrt_price: BigDecimal,
    at: DateTime<Utc>,
) {
    sqlx::query(
        "INSERT INTO meteora_damm_v2_swap_events
           (pool_address, signature, trade_direction,
            amount_a, amount_b, reserve_a_after, reserve_b_after, next_sqrt_price,
            claiming_fee, protocol_fee, compounding_fee, referral_fee, fee_token_is_a,
            timestamp, slot, event_index)
         VALUES ($1, $2, 'a_to_b', $3, $4, 0, 0, $5, 0, 0, 0, 0, false, $6, 0, 0)",
    )
    .bind(address)
    .bind(tag)
    .bind(amount_a)
    .bind(amount_b)
    .bind(sqrt_price)
    .bind(at)
    .execute(pool)
    .await
    .unwrap();
}

fn repository_of(pool: PgPool) -> PgAnalyticRepository {
    PgAnalyticRepository::new(pool)
}

/// The cycle at `now` of a writer that valued every tranche before `tranche`.
async fn cycle_at(pool: &PgPool, tranche: DateTime<Utc>, now: DateTime<Utc>) -> CycleOutcome {
    let progress = AnalyticProgress {
        first_hour: Some(hour_of(tranche)),
        last_frozen: None,
        last_tranche: Some(tranche - Duration::minutes(5)),
    };
    repository_of(as_role(pool, "yog_analytic").await)
        .write_cycle(&CycleWrite::plan(now, progress))
        .await
        .unwrap()
}

/// Values `tranche` alone, at the first instant it may be.
async fn value(pool: &PgPool, tranche: DateTime<Utc>) -> CycleOutcome {
    cycle_at(pool, tranche, tranche + Duration::minutes(15)).await
}

type Range = (
    Option<BigDecimal>,
    Option<BigDecimal>,
    Option<BigDecimal>,
    Option<BigDecimal>,
);

/// `(market_a, price_a, market_b, price_b)` of the pool in the tranche.
async fn prices(pool: &PgPool, address: &str, tranche: DateTime<Utc>) -> Range {
    sqlx::query_as(
        "SELECT market_a_usd, price_a_usd, market_b_usd, price_b_usd
           FROM analytic_pool_prices_5min WHERE pool_address = $1 AND bucket = $2",
    )
    .bind(address)
    .bind(tranche)
    .fetch_one(pool)
    .await
    .unwrap()
}

fn dec(value: &str) -> BigDecimal {
    value.parse().unwrap()
}

/// Against the bound dropped: IA keeps its market price of before its fall, and its
/// 1.94 billion units sold are worth $14.5 million.
#[sqlx::test]
async fn a_token_emptied_in_its_pool_is_worth_what_the_pool_pays(pool: PgPool) {
    let tranche = settled_tranche();
    let (ia, sol, other) = (pk(10).to_string(), pk(11).to_string(), pk(12).to_string());
    let emptied = setup_pool(&pool, 1, (&ia, 6), (&sol, 9)).await;
    let beside = setup_pool(&pool, 2, (&other, 6), (&sol, 9)).await;
    price_at(
        &pool,
        &ia,
        "0.00744765469211184",
        tranche - Duration::minutes(1),
    )
    .await;
    price_at(&pool, &sol, "107.28", tranche - Duration::minutes(1)).await;
    swap(
        &pool,
        &emptied,
        "ia",
        (1_944_522_520_695_913, 4_850_552),
        dec("394034525786492"),
        tranche + Duration::minutes(1),
    )
    .await;
    swap(
        &pool,
        &beside,
        "beside",
        (1_000, 1_000),
        sqrt_price_x64(1, 1),
        tranche,
    )
    .await;

    value(&pool, tranche).await;

    let (market_ia, price_ia, market_sol, price_sol) = prices(&pool, &emptied, tranche).await;
    assert_eq!(market_ia, Some(dec("0.00744765469211184")));
    assert_eq!(
        (market_sol, price_sol),
        (Some(dec("107.28")), Some(dec("107.28")))
    );
    let price_ia = price_ia.unwrap();
    assert!(
        price_ia > dec("0.00000000001") && price_ia < dec("0.0000000001"),
        "IA is worth what its pool pays, ~4.9e-11 $: {price_ia}"
    );
    let volume: Option<BigDecimal> = sqlx::query_scalar(
        "SELECT volume_in_a_usd FROM analytic_pool_5min WHERE pool_address = $1",
    )
    .bind(&emptied)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(
        volume.unwrap() < dec("1"),
        "less than a dollar changed hands"
    );
}

/// Against the anchor bounded as well, on either side of its pool: SOL would take a
/// market price too low through the pool's rate; and against the other side left unbounded.
#[sqlx::test]
async fn the_anchor_keeps_its_market_price_on_either_side(pool: PgPool) {
    let tranche = settled_tranche();
    let sol = pk(30).to_string();
    let (m, n, o) = (pk(31).to_string(), pk(32).to_string(), pk(33).to_string());
    price_at(&pool, &sol, "100", tranche).await;
    // SOL as B, 0.25 SOL for an M: M is worth $25 in the pool, $0.001 on the market.
    let sol_as_b = setup_pool(&pool, 1, (&m, 6), (&sol, 6)).await;
    price_at(&pool, &m, "0.001", tranche).await;
    swap(&pool, &sol_as_b, "m", (4, 1), sqrt_price_x64(4, 1), tranche).await;
    // SOL as A, 4 N or 4 O for a SOL: each is worth $25 in its pool.
    let low_beside = setup_pool(&pool, 2, (&sol, 6), (&n, 6)).await;
    price_at(&pool, &n, "0.001", tranche).await;
    swap(
        &pool,
        &low_beside,
        "n",
        (1, 4),
        sqrt_price_x64(1, 4),
        tranche,
    )
    .await;
    let high_beside = setup_pool(&pool, 3, (&sol, 6), (&o, 6)).await;
    price_at(&pool, &o, "50", tranche).await;
    swap(
        &pool,
        &high_beside,
        "o",
        (1, 4),
        sqrt_price_x64(1, 4),
        tranche,
    )
    .await;

    value(&pool, tranche).await;

    let (_, price_m, _, sol_b) = prices(&pool, &sol_as_b, tranche).await;
    assert_eq!((price_m, sol_b), (Some(dec("0.001")), Some(dec("100"))));
    let (_, sol_a, _, price_n) = prices(&pool, &low_beside, tranche).await;
    assert_eq!((sol_a, price_n), (Some(dec("100")), Some(dec("0.001"))));
    let (_, sol_a, _, price_o) = prices(&pool, &high_beside, tranche).await;
    assert_eq!((sol_a, price_o), (Some(dec("100")), Some(dec("25"))));
}

/// Against a side taken for the anchor on a tie, A or B: it would keep a market price
/// its pool contradicts.
#[sqlx::test]
async fn with_no_anchor_each_token_takes_the_lower_of_its_two_prices(pool: PgPool) {
    let tranche = settled_tranche();
    let mut found = Vec::new();
    // 0.25 B for an A in both pools; A over-priced by the market in the first, B in the second.
    for (seed, market_a, market_b) in [(1u8, "30", "100"), (2, "2", "100")] {
        let (a, b) = (pk(seed + 10).to_string(), pk(seed + 20).to_string());
        let address = setup_pool(&pool, seed, (&a, 6), (&b, 6)).await;
        price_at(&pool, &a, market_a, tranche).await;
        price_at(&pool, &b, market_b, tranche).await;
        swap(
            &pool,
            &address,
            &format!("ab{seed}"),
            (4, 1),
            sqrt_price_x64(4, 1),
            tranche,
        )
        .await;
        found.push(address);
    }

    value(&pool, tranche).await;

    let (_, a_over, _, b_beside) = prices(&pool, &found[0], tranche).await;
    assert_eq!((a_over, b_beside), (Some(dec("25")), Some(dec("100"))));
    let (_, a_beside, _, b_over) = prices(&pool, &found[1], tranche).await;
    assert_eq!((a_beside, b_over), (Some(dec("2")), Some(dec("8"))));
}

/// Against `LEAST` turned into a strict minimum: a token Jupiter does not price would
/// lose its value through the pool, as most fresh tokens against SOL would.
#[sqlx::test]
async fn a_token_without_a_market_price_takes_its_price_in_the_pool(pool: PgPool) {
    let tranche = settled_tranche();
    let (a, b) = (pk(10).to_string(), pk(11).to_string());
    let address = setup_pool(&pool, 1, (&a, 6), (&b, 6)).await;
    price_at(&pool, &b, "100", tranche).await;
    swap(&pool, &address, "ab", (4, 1), sqrt_price_x64(4, 1), tranche).await;

    value(&pool, tranche).await;

    let (market_a, price_a, _, price_b) = prices(&pool, &address, tranche).await;
    assert_eq!(market_a, None);
    assert_eq!((price_a, price_b), (Some(dec("25")), Some(dec("100"))));
}

/// Against the valued flag read from one side only: a tranche with no price at all
/// would publish a zero.
#[sqlx::test]
async fn a_tranche_without_any_price_is_not_valued(pool: PgPool) {
    let tranche = settled_tranche();
    let (a, b) = (pk(10).to_string(), pk(11).to_string());
    let address = setup_pool(&pool, 1, (&a, 6), (&b, 6)).await;
    swap(&pool, &address, "ab", (4, 1), sqrt_price_x64(4, 1), tranche).await;

    value(&pool, tranche).await;

    let valued: (bool, Option<BigDecimal>) = sqlx::query_as(
        "SELECT valued, volume_in_a_usd FROM analytic_pool_5min WHERE pool_address = $1",
    )
    .bind(&address)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(valued, (false, None));
}

/// Against the margin dropped from the tranche's close: a swap landing within it would
/// miss a tranche already valued.
#[sqlx::test]
async fn a_tranche_waits_ten_minutes_after_its_close(pool: PgPool) {
    let tranche = settled_tranche();
    let (a, b) = (pk(10).to_string(), pk(11).to_string());
    let address = setup_pool(&pool, 1, (&a, 6), (&b, 6)).await;
    price_at(&pool, &b, "100", tranche).await;
    swap(&pool, &address, "ab", (4, 1), sqrt_price_x64(4, 1), tranche).await;

    let early = cycle_at(&pool, tranche, tranche + Duration::minutes(14)).await;

    assert_eq!(early.tranches_written, 0);
    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM analytic_pool_prices_5min")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(rows, 0);
}

/// ☠️ Pins the loss the READMEs warn about: a swap that lands in a tranche already
/// valued stays out of it, and its tranche is never written twice.
#[sqlx::test]
async fn a_tranche_is_valued_once(pool: PgPool) {
    let tranche = settled_tranche();
    let (a, b) = (pk(10).to_string(), pk(11).to_string());
    let address = setup_pool(&pool, 1, (&a, 6), (&b, 6)).await;
    price_at(&pool, &b, "100", tranche).await;
    swap(
        &pool,
        &address,
        "on_time",
        (4, 1),
        sqrt_price_x64(4, 1),
        tranche,
    )
    .await;
    value(&pool, tranche).await;

    swap(
        &pool,
        &address,
        "late",
        (4, 1),
        sqrt_price_x64(4, 1),
        tranche,
    )
    .await;
    let analytic = repository_of(as_role(&pool, "yog_analytic").await);
    let progress = analytic.progress().await.unwrap();
    analytic
        .write_cycle(&CycleWrite::plan(tranche + Duration::minutes(30), progress))
        .await
        .unwrap();

    let swaps: i64 = sqlx::query_scalar(
        "SELECT swap_count FROM analytic_pool_5min WHERE pool_address = $1 AND bucket = $2",
    )
    .bind(&address)
    .bind(tranche)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(swaps, 1);
}

/// Against the open hour summed short of its tranches, or a column summed from another.
/// ⚠️ The open hour ends the cycle's range: a bound that stops at its start leaves it empty.
#[sqlx::test]
async fn an_hourly_fact_is_the_sum_of_its_tranches(pool: PgPool) {
    let hour = hour_of(settled_tranche());
    let (a, b) = (pk(10).to_string(), pk(11).to_string());
    let address = setup_pool(&pool, 1, (&a, 6), (&b, 6)).await;
    price_at(&pool, &a, "30", hour).await;
    price_at(&pool, &b, "100", hour).await;
    for (minute, amounts) in [(5, (4, 1)), (20, (8, 2)), (50, (40, 10))] {
        let at = hour + Duration::minutes(minute) + Duration::seconds(30);
        swap(
            &pool,
            &address,
            &format!("m{minute}"),
            amounts,
            sqrt_price_x64(4, 1),
            at,
        )
        .await;
    }

    let progress = AnalyticProgress {
        first_hour: Some(hour),
        last_frozen: None,
        last_tranche: None,
    };
    repository_of(as_role(&pool, "yog_analytic").await)
        .write_cycle(&CycleWrite::plan(hour + Duration::minutes(40), progress))
        .await
        .unwrap();

    let sums: (
        i64,
        Option<BigDecimal>,
        Option<BigDecimal>,
        Option<BigDecimal>,
    ) = sqlx::query_as(
        "SELECT SUM(swap_count)::BIGINT, SUM(volume_in_a_usd), SUM(volume_in_b_usd),
                    SUM(fees_usd)
               FROM analytic_pool_5min WHERE pool_address = $1",
    )
    .bind(&address)
    .fetch_one(&pool)
    .await
    .unwrap();
    let fact: (
        i64,
        Option<BigDecimal>,
        Option<BigDecimal>,
        Option<BigDecimal>,
    ) = sqlx::query_as(
        "SELECT swap_count, volume_in_a_usd, volume_in_b_usd, fees_usd
               FROM analytic_pool_hourly WHERE pool_address = $1 AND bucket = $2",
    )
    .bind(&address)
    .bind(hour)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(sums.0, 2, "the tranche of :50 is not closed yet");
    assert_eq!(fact, sums);
}

/// The views the persistence code reads.
async fn views_read_by_a_binary(pool: &PgPool) -> Vec<String> {
    let views: Vec<String> =
        sqlx::query_scalar("SELECT viewname::TEXT FROM pg_views WHERE schemaname = 'public'")
            .fetch_all(pool)
            .await
            .unwrap();
    let sources = sources_under(&Path::new(env!("CARGO_MANIFEST_DIR")).join("src"));
    views
        .into_iter()
        .filter(|view| sources.iter().any(|(_, text)| text.contains(view.as_str())))
        .collect()
}

/// `(path, text)` of every Rust and SQL file under `dir`.
fn sources_under(dir: &Path) -> Vec<(String, String)> {
    let mut files = Vec::new();
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            files.extend(sources_under(&path));
        } else if path.extension().is_some_and(|e| e == "rs" || e == "sql") {
            let text = std::fs::read_to_string(&path).unwrap();
            files.push((path.display().to_string(), text));
        }
    }
    files
}

/// The views of `views` that reach, through other views, the swaps valued by the hour.
async fn reaching_the_hourly_swap_valuation(pool: &PgPool, views: &[String]) -> Vec<String> {
    sqlx::query_scalar(
        "WITH RECURSIVE dep (view, rel) AS (
             SELECT v.oid, d.refobjid
               FROM pg_class v
               JOIN pg_rewrite r ON r.ev_class = v.oid
               JOIN pg_depend d ON d.objid = r.oid AND d.classid = 'pg_rewrite'::regclass
                               AND d.refclassid = 'pg_class'::regclass
              WHERE v.relname = ANY($1) AND d.refobjid <> v.oid
             UNION
             SELECT dep.view, d.refobjid
               FROM dep
               JOIN pg_rewrite r ON r.ev_class = dep.rel
               JOIN pg_depend d ON d.objid = r.oid AND d.classid = 'pg_rewrite'::regclass
                               AND d.refclassid = 'pg_class'::regclass
              WHERE d.refobjid <> dep.rel
         )
         SELECT DISTINCT v.relname::TEXT
           FROM dep JOIN pg_class v ON v.oid = dep.view JOIN pg_class c ON c.oid = dep.rel
          WHERE c.relname = 'meteora_damm_v2_swap_events_hourly_priced'",
    )
    .bind(views)
    .fetch_all(pool)
    .await
    .unwrap()
}

#[sqlx::test]
async fn no_view_a_binary_reads_values_a_swap_by_the_hour(pool: PgPool) {
    let views = views_read_by_a_binary(&pool).await;
    assert!(
        views
            .iter()
            .any(|v| v == "meteora_damm_v2_pool_hourly_flow"),
        "the scan must reach the flow the signals read, got {views:?}"
    );

    assert_eq!(
        reaching_the_hourly_swap_valuation(&pool, &views).await,
        Vec::<String>::new()
    );
}

/// The guard above, against the flow pointed back at the hourly valuation.
#[sqlx::test]
async fn the_guard_catches_a_flow_valued_by_the_hour(pool: PgPool) {
    sqlx::query(
        "CREATE OR REPLACE VIEW meteora_damm_v2_pool_hourly_flow AS
         SELECT pool_address, bucket,
                yog_moved_amount_usd(volume_in_a, dec_a, eff_price_a) AS volume_a_to_b_usd,
                yog_moved_amount_usd(volume_in_b, dec_b, eff_price_b) AS volume_b_to_a_usd,
                valuation_complete
           FROM meteora_damm_v2_swap_events_hourly_priced",
    )
    .execute(&pool)
    .await
    .unwrap();

    let views = views_read_by_a_binary(&pool).await;
    assert_eq!(
        reaching_the_hourly_swap_valuation(&pool, &views).await,
        ["meteora_damm_v2_pool_hourly_flow"]
    );
}

/// Against a market price read in the writer outside the price table: a swap valued there
/// would escape the bound.
#[test]
fn the_writer_reads_market_prices_in_the_price_table_alone() {
    let writer = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/repositories");
    let readers: Vec<String> = sources_under(&writer)
        .into_iter()
        .filter(|(path, _)| path.contains("analytic"))
        .filter(|(_, text)| {
            [
                "token_prices",
                "yog_price_asof",
                "swap_hour_prices",
                "hourly_priced",
            ]
            .iter()
            .any(|reader| text.contains(reader))
        })
        .map(|(path, _)| path)
        .collect();

    assert_eq!(readers.len(), 1, "{readers:?}");
    assert!(readers[0].ends_with("meteora_damm_v2_pool_prices.sql"));
}
