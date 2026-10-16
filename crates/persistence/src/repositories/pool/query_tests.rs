use super::*;
use crate::repositories::helper::QueryMode;
use chrono::TimeZone;
use yog_core::PoolSort;

// `reference` is the spec, written out by hand: the tests never read it back from the code.

struct Expected {
    ord_pri: &'static str,
    ord_tie: &'static str,
    ks_pri: &'static str,
    ks_tie: &'static str,
}

fn reference(sort: PoolSort, mode: QueryMode) -> Expected {
    use PoolSort::*;
    use QueryMode::*;
    match (sort, mode) {
        (FirstSeenDesc, Forward) => Expected {
            ord_pri: "DESC",
            ord_tie: "ASC",
            ks_pri: "<",
            ks_tie: ">",
        },
        (FirstSeenDesc, Backward) => Expected {
            ord_pri: "ASC",
            ord_tie: "DESC",
            ks_pri: ">",
            ks_tie: "<",
        },
        (FirstSeenAsc, Forward) => Expected {
            ord_pri: "ASC",
            ord_tie: "ASC",
            ks_pri: ">",
            ks_tie: ">",
        },
        (FirstSeenAsc, Backward) => Expected {
            ord_pri: "DESC",
            ord_tie: "DESC",
            ks_pri: "<",
            ks_tie: "<",
        },
        (LastSeenDesc, Forward) => Expected {
            ord_pri: "DESC",
            ord_tie: "ASC",
            ks_pri: "<",
            ks_tie: ">",
        },
        (LastSeenDesc, Backward) => Expected {
            ord_pri: "ASC",
            ord_tie: "DESC",
            ks_pri: ">",
            ks_tie: "<",
        },
        (LastSeenAsc, Forward) => Expected {
            ord_pri: "ASC",
            ord_tie: "ASC",
            ks_pri: ">",
            ks_tie: ">",
        },
        (LastSeenAsc, Backward) => Expected {
            ord_pri: "DESC",
            ord_tie: "DESC",
            ks_pri: "<",
            ks_tie: "<",
        },
    }
}

const ALL_CASES: &[(PoolSort, QueryMode)] = &[
    (PoolSort::FirstSeenDesc, QueryMode::Forward),
    (PoolSort::FirstSeenDesc, QueryMode::Backward),
    (PoolSort::FirstSeenAsc, QueryMode::Forward),
    (PoolSort::FirstSeenAsc, QueryMode::Backward),
    (PoolSort::LastSeenDesc, QueryMode::Forward),
    (PoolSort::LastSeenDesc, QueryMode::Backward),
    (PoolSort::LastSeenAsc, QueryMode::Forward),
    (PoolSort::LastSeenAsc, QueryMode::Backward),
];

#[test]
fn effective_order_matches_reference_for_all_cases() {
    for &(sort, mode) in ALL_CASES {
        let exp = reference(sort, mode);
        let (pri, tie) = effective_order(sort, mode);
        assert_eq!(
            pri, exp.ord_pri,
            "primary order mismatch for {sort:?} / {mode:?}"
        );
        assert_eq!(
            tie, exp.ord_tie,
            "tiebreak order mismatch for {sort:?} / {mode:?}"
        );
    }
}

#[test]
fn keyset_operators_match_reference_for_all_cases() {
    for &(sort, mode) in ALL_CASES {
        let exp = reference(sort, mode);
        let (pri, tie) = keyset_operators(sort, mode);
        assert_eq!(
            pri, exp.ks_pri,
            "primary keyset op mismatch for {sort:?} / {mode:?}"
        );
        assert_eq!(
            tie, exp.ks_tie,
            "tiebreak keyset op mismatch for {sort:?} / {mode:?}"
        );
    }
}

#[test]
fn forward_and_backward_have_opposite_primary_order() {
    for sort in [
        PoolSort::FirstSeenDesc,
        PoolSort::FirstSeenAsc,
        PoolSort::LastSeenDesc,
        PoolSort::LastSeenAsc,
    ] {
        let (fwd_pri, fwd_tie) = effective_order(sort, QueryMode::Forward);
        let (bwd_pri, bwd_tie) = effective_order(sort, QueryMode::Backward);
        assert_ne!(fwd_pri, bwd_pri, "primary order must flip for {sort:?}");
        assert_ne!(bwd_tie, fwd_tie, "tiebreak order must flip for {sort:?}");
    }
}

#[test]
fn forward_and_backward_have_opposite_keyset_operators() {
    for sort in [
        PoolSort::FirstSeenDesc,
        PoolSort::FirstSeenAsc,
        PoolSort::LastSeenDesc,
        PoolSort::LastSeenAsc,
    ] {
        let (fwd_pri, fwd_tie) = keyset_operators(sort, QueryMode::Forward);
        let (bwd_pri, bwd_tie) = keyset_operators(sort, QueryMode::Backward);
        assert_ne!(fwd_pri, bwd_pri, "primary op must flip for {sort:?}");
        assert_ne!(fwd_tie, bwd_tie, "tiebreak op must flip for {sort:?}");
    }
}

/// Mutation: a keyset operator that disagrees with the order — the next page skips or repeats rows.
#[test]
fn forward_keyset_op_agrees_with_order() {
    for sort in [
        PoolSort::FirstSeenDesc,
        PoolSort::FirstSeenAsc,
        PoolSort::LastSeenDesc,
        PoolSort::LastSeenAsc,
    ] {
        let (ord_pri, _) = effective_order(sort, QueryMode::Forward);
        let (ks_pri, _) = keyset_operators(sort, QueryMode::Forward);
        let agree = (ord_pri == "DESC" && ks_pri == "<") || (ord_pri == "ASC" && ks_pri == ">");
        assert!(
            agree,
            "forward order {ord_pri} and keyset op {ks_pri} disagree for {sort:?}"
        );
    }
}

#[test]
fn backward_keyset_op_agrees_with_order() {
    for sort in [
        PoolSort::FirstSeenDesc,
        PoolSort::FirstSeenAsc,
        PoolSort::LastSeenDesc,
        PoolSort::LastSeenAsc,
    ] {
        let (ord_pri, _) = effective_order(sort, QueryMode::Backward);
        let (ks_pri, _) = keyset_operators(sort, QueryMode::Backward);
        let agree = (ord_pri == "DESC" && ks_pri == "<") || (ord_pri == "ASC" && ks_pri == ">");
        assert!(
            agree,
            "backward order {ord_pri} and keyset op {ks_pri} disagree for {sort:?}"
        );
    }
}

/// Mutation: one sort column special-cased.
#[test]
fn first_and_last_seen_are_symmetric() {
    for (fs, ls) in [
        (PoolSort::FirstSeenDesc, PoolSort::LastSeenDesc),
        (PoolSort::FirstSeenAsc, PoolSort::LastSeenAsc),
    ] {
        for mode in [QueryMode::Forward, QueryMode::Backward] {
            assert_eq!(
                effective_order(fs, mode),
                effective_order(ls, mode),
                "order differs between {fs:?} and {ls:?} at {mode:?}"
            );
            assert_eq!(
                keyset_operators(fs, mode),
                keyset_operators(ls, mode),
                "keyset differs between {fs:?} and {ls:?} at {mode:?}"
            );
        }
    }
}

// Binds are `$n` placeholders, so only the SQL shape is asserted: the filtering
// itself is the DB-backed tests' job.
fn sql_for_search(search: Option<&str>) -> String {
    build(PaginatedPoolsQuery {
        mode: QueryMode::Forward,
        sort: PoolSort::LastSeenDesc,
        cursor_sort_value: None,
        cursor_pool_address: None,
        search: search.map(str::to_owned),
        fee_bps: None,
        as_of: None,
        fetch_limit: 50,
    })
    .into_sql()
    .as_str()
    .to_owned()
}

#[test]
fn single_term_uses_address_or_either_token() {
    let sql = sql_for_search(Some("SOL"));
    assert!(
        sql.contains("pool_address = "),
        "single term should keep the address match: {sql}"
    );
    assert!(
        sql.contains("tm.mint IN (pools.token_a_mint, pools.token_b_mint)"),
        "single term should match either token in one EXISTS: {sql}"
    );
}

#[test]
fn pair_term_requires_one_token_per_side() {
    let sql = sql_for_search(Some("SOL/USDC"));

    assert!(
        sql.contains("tm.mint = pools.token_a_mint"),
        "pair should read token_a directly: {sql}"
    );
    assert!(
        sql.contains("tm.mint = pools.token_b_mint"),
        "pair should read token_b directly: {sql}"
    );
    // Four EXISTS: (a=X AND b=Y) OR (a=Y AND b=X).
    assert_eq!(
        sql.matches("EXISTS").count(),
        4,
        "pair should expand to four EXISTS predicates: {sql}"
    );
    assert!(
        sql.contains(") OR ("),
        "pair should accept either side ordering: {sql}"
    );
    assert!(
        !sql.contains("tm.mint IN ("),
        "pair must not fall back to the single-term either-token form: {sql}"
    );
}

#[test]
fn pair_with_blank_side_falls_back_to_single() {
    let sql = sql_for_search(Some("SOL/"));
    assert!(
        sql.contains("tm.mint IN (pools.token_a_mint, pools.token_b_mint)"),
        "blank side should behave as a single term: {sql}"
    );
    assert!(
        !sql.contains("tm.mint = pools.token_a_mint"),
        "blank side must not build a pair filter: {sql}"
    );
}

#[test]
fn no_search_term_adds_no_token_filter() {
    let sql = sql_for_search(None);
    assert!(
        !sql.contains("token_metadata"),
        "absent search must not touch token_metadata: {sql}"
    );
}

// The fence's presence in the SQL only; that it holds a traversal together is
// the `pool_pagination` integration tests' job.
fn sql_for_fence(sort: PoolSort, as_of: Option<DateTime<Utc>>) -> String {
    build(PaginatedPoolsQuery {
        mode: QueryMode::Forward,
        sort,
        cursor_sort_value: None,
        cursor_pool_address: None,
        search: None,
        fee_bps: None,
        as_of,
        fetch_limit: 50,
    })
    .into_sql()
    .as_str()
    .to_owned()
}

fn instant() -> DateTime<Utc> {
    Utc.timestamp_opt(1_800_000_000, 0).unwrap()
}

#[test]
fn fence_bounds_last_seen_when_set() {
    for sort in [PoolSort::LastSeenDesc, PoolSort::LastSeenAsc] {
        let sql = sql_for_fence(sort, Some(instant()));
        assert!(
            sql.contains("AND last_seen_at <= "),
            "{sort:?} with a fence should bound last_seen_at: {sql}"
        );
    }
}

/// Mutation: the builder inventing a bound, which would hide freshly discovered pools.
#[test]
fn no_fence_leaves_the_listing_unbounded() {
    let sql = sql_for_fence(PoolSort::FirstSeenDesc, None);
    assert!(
        !sql.contains("last_seen_at <= "),
        "an unfenced sort must not bound last_seen_at: {sql}"
    );
}

#[test]
fn touched_since_count_reads_above_the_fence() {
    let sql = build_touched_since_count(TouchedSinceQuery {
        as_of: instant(),
        search: None,
        fee_bps: None,
    })
    .into_sql()
    .as_str()
    .to_owned();

    assert!(
        sql.contains("COUNT(*)") && sql.contains("last_seen_at > "),
        "the count should read strictly above the fence: {sql}"
    );
}

/// Mutation: a bare `COUNT(*)`, which would report pools the reader filtered out.
#[test]
fn touched_since_count_applies_the_same_filters() {
    let sql = build_touched_since_count(TouchedSinceQuery {
        as_of: instant(),
        search: Some("SOL".to_owned()),
        fee_bps: Some(BigDecimal::from(25)),
    })
    .into_sql()
    .as_str()
    .to_owned();

    assert!(
        sql.contains("tm.mint IN (pools.token_a_mint, pools.token_b_mint)"),
        "the count should apply the search filter: {sql}"
    );
    assert!(
        sql.contains("AND fee_bps = "),
        "the count should apply the fee filter: {sql}"
    );
}
