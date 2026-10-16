//! SQL of the paginated pool listing: dynamic `ORDER BY`, keyset predicate,
//! snapshot fence and filters.
//!
//! ⚠️ Runtime SQL, not checked by the sqlx macros: only the integration tests
//! guard it against the schema. Columns and directions come from closed enums; values are bound.

use chrono::{DateTime, Utc};
use sqlx::types::BigDecimal;
use sqlx::{Postgres, QueryBuilder};
use yog_core::{PoolSort, PoolSortColumn};

use crate::repositories::helper::QueryMode;

/// Everything the query needs, resolved by the repository.
pub(super) struct PaginatedPoolsQuery {
    pub(super) mode: QueryMode,
    pub(super) sort: PoolSort,
    pub(super) cursor_sort_value: Option<DateTime<Utc>>,
    pub(super) cursor_pool_address: Option<String>,
    pub(super) search: Option<String>,
    /// Exact base-fee tier, in basis points. `None` leaves the fee unfiltered.
    pub(super) fee_bps: Option<BigDecimal>,
    /// When set, the listing reads only `last_seen_at <= as_of` — see
    /// [`yog_core::domain::PoolPage`].
    pub(super) as_of: Option<DateTime<Utc>>,
    pub(super) fetch_limit: i64,
}

/// Input of [`build_touched_since_count`]: the rows that left the traversal
/// by moving above its fence.
pub(super) struct TouchedSinceQuery {
    pub(super) as_of: DateTime<Utc>,
    pub(super) search: Option<String>,
    pub(super) fee_bps: Option<BigDecimal>,
}

/// The physical column of a sort column: a static str, never user input, so it
/// is interpolated into the SQL text.
fn column_sql(col: PoolSortColumn) -> &'static str {
    match col {
        PoolSortColumn::FirstSeen => "first_seen_at",
        PoolSortColumn::LastSeen => "last_seen_at",
    }
}

/// `(primary_dir, tiebreak_dir)` for the sort column and the `pool_address`
/// tiebreak. ⚠️ `Backward` runs the opposite physical order; the repository
/// reverses the result to restore display order.
fn effective_order(sort: PoolSort, mode: QueryMode) -> (&'static str, &'static str) {
    let (asc_primary, asc_tiebreak) = if sort.is_ascending() {
        (true, true)
    } else {
        (false, true)
    };

    let (primary_asc, tiebreak_asc) = match mode {
        QueryMode::Forward => (asc_primary, asc_tiebreak),
        QueryMode::Backward => (!asc_primary, !asc_tiebreak),
    };

    (
        if primary_asc { "ASC" } else { "DESC" },
        if tiebreak_asc { "ASC" } else { "DESC" },
    )
}

/// The keyset comparison operators for the sort column and the `pool_address`
/// tiebreak, both strict.
fn keyset_operators(sort: PoolSort, mode: QueryMode) -> (&'static str, &'static str) {
    let primary_gt = match (sort.is_ascending(), mode) {
        (true, QueryMode::Forward) => true,
        (true, QueryMode::Backward) => false,
        (false, QueryMode::Forward) => false,
        (false, QueryMode::Backward) => true,
    };
    let (_, tiebreak_dir) = effective_order(sort, mode);
    let tiebreak_gt = tiebreak_dir == "ASC";

    (
        if primary_gt { ">" } else { "<" },
        if tiebreak_gt { ">" } else { "<" },
    )
}

/// How the free-text search term was interpreted.
///
/// A `/` splits the term into a token *pair* ("SOL/USDC"); anything
/// else (no slash, or one side blank once trimmed) is a single term.
enum ParsedSearch {
    /// Both sides non-empty: require one token matching each side.
    Pair(String, String),
    /// A lone free-text term: address or either token symbol/name.
    Single(String),
}

/// Split the raw search term on its first `/`. Each side is trimmed;
/// a blank side collapses the request back to a single-term search on
/// the other side, so "SOL/" behaves like "SOL".
fn parse_search(raw: &str) -> ParsedSearch {
    match raw.split_once('/') {
        Some((left, right)) => {
            let (left, right) = (left.trim(), right.trim());
            match (left.is_empty(), right.is_empty()) {
                (false, false) => ParsedSearch::Pair(left.to_owned(), right.to_owned()),
                (false, true) => ParsedSearch::Single(left.to_owned()),
                (true, false) => ParsedSearch::Single(right.to_owned()),
                (true, true) => ParsedSearch::Single(raw.trim().to_owned()),
            }
        }
        None => ParsedSearch::Single(raw.trim().to_owned()),
    }
}

/// Pushes an `EXISTS`: the token at `mint_col` has a symbol or name containing
/// `term`, case-insensitive. `mint_col` is a static column name, interpolated;
/// `term` is bound.
fn push_side_match(qb: &mut QueryBuilder<Postgres>, mint_col: &str, term: &str) {
    qb.push("EXISTS (SELECT 1 FROM token_metadata tm WHERE tm.mint = pools.");
    qb.push(mint_col);
    qb.push(" AND (tm.symbol ILIKE ('%' || ");
    qb.push_bind(term.to_owned());
    qb.push(" || '%') OR tm.name ILIKE ('%' || ");
    qb.push_bind(term.to_owned());
    qb.push(" || '%')))");
}

/// Pushes the search and fee-tier filters.
/// ⚠️ Shared by the listing and the `touched_since` count, not copied: the count
/// must see exactly the population the listing sees.
fn push_filters(
    qb: &mut QueryBuilder<Postgres>,
    search: Option<String>,
    fee_bps: Option<BigDecimal>,
) {
    match search.as_deref().map(parse_search) {
        Some(ParsedSearch::Pair(left, right)) => {
            // Each side read off a distinct mint column: the two matched tokens
            // are different by construction.
            qb.push(" AND ((");
            push_side_match(qb, "token_a_mint", &left);
            qb.push(" AND ");
            push_side_match(qb, "token_b_mint", &right);
            qb.push(") OR (");
            push_side_match(qb, "token_a_mint", &right);
            qb.push(" AND ");
            push_side_match(qb, "token_b_mint", &left);
            qb.push("))");
        }
        Some(ParsedSearch::Single(term)) => {
            qb.push(" AND (pool_address = ");
            qb.push_bind(term.clone());
            qb.push(
                " OR EXISTS (SELECT 1 FROM token_metadata tm \
                 WHERE tm.mint IN (pools.token_a_mint, pools.token_b_mint) \
                 AND (tm.symbol ILIKE ",
            );
            // Wrap the term in % wildcards via SQL concat to keep it bound.
            qb.push("('%' || ");
            qb.push_bind(term.clone());
            qb.push(" || '%') OR tm.name ILIKE ('%' || ");
            qb.push_bind(term);
            qb.push(" || '%'))))");
        }
        None => {}
    }

    if let Some(fee) = fee_bps {
        qb.push(" AND fee_bps = ");
        qb.push_bind(fee);
    }
}

/// Counts the pools that left the traversal by being touched after its fence:
/// same filters, opposite side of the listing's `last_seen_at <= as_of`.
pub(super) fn build_touched_since_count(q: TouchedSinceQuery) -> QueryBuilder<Postgres> {
    let mut qb: QueryBuilder<Postgres> =
        QueryBuilder::new("SELECT COUNT(*) FROM pools WHERE last_seen_at > ");
    qb.push_bind(q.as_of);
    push_filters(&mut qb, q.search, q.fee_bps);
    qb
}

/// Build the full paginated query.
pub(super) fn build(q: PaginatedPoolsQuery) -> QueryBuilder<Postgres> {
    let sort_col = column_sql(q.sort.column());
    let (primary_order, tiebreak_order) = effective_order(q.sort, q.mode);

    let mut qb: QueryBuilder<Postgres> = QueryBuilder::new(
        // ⚠️ Kept in step with `PoolRow` by hand: no macro checks this list.
        "SELECT pool_address, protocol, token_a_mint, token_b_mint, \
         fee_bps, first_seen_at, last_seen_at FROM pools WHERE 1=1",
    );

    if let (Some(value), Some(addr)) = (q.cursor_sort_value, q.cursor_pool_address.clone()) {
        let (primary_op, tiebreak_op) = keyset_operators(q.sort, q.mode);
        qb.push(" AND (");
        qb.push(sort_col);
        qb.push(format!(" {primary_op} "));
        qb.push_bind(value);
        qb.push(" OR (");
        qb.push(sort_col);
        qb.push(" = ");
        qb.push_bind(value);
        qb.push(format!(" AND pool_address {tiebreak_op} "));
        qb.push_bind(addr);
        qb.push("))");
    }

    // `last_seen_at` only grows, so this bound makes the result set shrink-only:
    // a pool touched mid-traversal leaves it instead of crossing the cursor.
    if let Some(as_of) = q.as_of {
        qb.push(" AND last_seen_at <= ");
        qb.push_bind(as_of);
    }

    push_filters(&mut qb, q.search, q.fee_bps);

    qb.push(" ORDER BY ");
    qb.push(sort_col);
    qb.push(format!(
        " {primary_order}, pool_address {tiebreak_order} LIMIT "
    ));
    qb.push_bind(q.fetch_limit);

    qb
}

#[cfg(test)]
#[path = "query_tests.rs"]
mod tests;
