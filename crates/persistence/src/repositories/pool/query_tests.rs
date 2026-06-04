use super::*;
use crate::repositories::helper::QueryMode;
use yog_core::PoolSort;

// The reference truth table, transcribed from the validated spec.
// Each row: (sort, mode) => (primary_order, tiebreak_order,
//                            primary_keyset_op, tiebreak_keyset_op).
//
// This table IS the specification. The tests assert the code
// matches it; they do not read behaviour back from the code. A
// wrong "fix" to effective_order / keyset_operators breaks these.
//
// | Sort           | Mode     | ord_pri | ord_tie | ks_pri | ks_tie |
// | FirstSeenDesc  | Forward  | DESC    | ASC     | <      | >      |
// | FirstSeenDesc  | Backward | ASC     | DESC    | >      | <      |
// | FirstSeenAsc   | Forward  | ASC     | ASC     | >      | >      |
// | FirstSeenAsc   | Backward | DESC    | DESC    | <      | <      |
// | LastSeenDesc   | Forward  | DESC    | ASC     | <      | >      |
// | LastSeenDesc   | Backward | ASC     | DESC    | >      | <      |
// | LastSeenAsc    | Forward  | ASC     | ASC     | >      | >      |
// | LastSeenAsc    | Backward | DESC    | DESC    | <      | <      |

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

// ── effective_order: full matrix ──────────────────────────────

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

// ── keyset_operators: full matrix ─────────────────────────────

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

// ── Cross-cutting invariants ──────────────────────────────────
//
// These don't re-check the table cell by cell; they assert
// structural properties that must hold no matter the table's
// exact contents. If both the table and these agree, confidence
// is high that the logic is internally consistent.

/// Forward and Backward of the same sort must produce opposite
/// primary orders — that's the definition of reversing the
/// traversal.
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

/// Forward and Backward of the same sort must produce opposite
/// keyset operators on both columns.
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

/// In Forward mode, the primary keyset operator must agree with
/// the primary order: DESC pairs with '<' (we walk toward smaller
/// values), ASC pairs with '>'. This is the property that, if
/// violated, makes "next page" skip or repeat rows.
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

/// Same agreement in Backward mode (the query runs in reversed
/// physical order, so the same pairing rule applies to the
/// reversed order).
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

/// The sort column is independent of first/last: FirstSeen* and
/// LastSeen* with the same direction must yield identical
/// operators and orders. Guards against a copy-paste that special-
/// cases one column.
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
