//! SQL construction for the paginated pool listing.
//!
//! Isolated from the repository so `pool.rs` stays an orchestrator
//! (build → execute → map → assemble Page) rather than a wall of
//! inline SQL. This module owns the dynamic `ORDER BY`, the keyset
//! cursor predicate, and the optional search filter. The row type
//! produced by this query lives in `rows.rs`; the repository binds
//! the two together.
//!
//! Trade-off: dynamic ORDER BY rules out `sqlx::query!`, so these
//! queries are NOT verified at compile time against the schema.
//! Safety against injection is preserved because the column and
//! direction come from closed enums (never user strings); only the
//! cursor/search VALUES are user-supplied, and they go through
//! `push_bind`. Correctness against the schema is covered by
//! integration tests rather than the macro.

use chrono::{DateTime, Utc};
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
    pub(super) fetch_limit: i64,
}

/// The physical column name for a sort column. Returned as a static
/// str (never user input) so it is safe to interpolate directly into
/// the SQL text.
fn column_sql(col: PoolSortColumn) -> &'static str {
    match col {
        PoolSortColumn::FirstSeen => "first_seen_at",
        PoolSortColumn::LastSeen => "last_seen_at",
    }
}

/// Resolve the effective SQL ordering for the (sort, mode) pair.
///
/// The displayed order is defined by `sort`. A `Backward` traversal
/// runs the query in the opposite physical order (so the keyset
/// predicate selects the rows just before the cursor); the repository
/// reverses the result afterwards to restore display order.
///
/// Returns `(primary_dir, tiebreak_dir)` as SQL keywords for the sort
/// column and the `pool_address` tiebreaker respectively.
fn effective_order(sort: PoolSort, mode: QueryMode) -> (&'static str, &'static str) {
    // Natural (forward) directions per sort.
    let (asc_primary, asc_tiebreak) = if sort.is_ascending() {
        // sort value ASC → tiebreak by address ASC for determinism
        (true, true)
    } else {
        // sort value DESC → tiebreak by address ASC (matches the
        // historical first_seen_at DESC, pool_address ASC ordering)
        (false, true)
    };

    // Backward traversal flips both.
    let (primary_asc, tiebreak_asc) = match mode {
        QueryMode::Forward => (asc_primary, asc_tiebreak),
        QueryMode::Backward => (!asc_primary, !asc_tiebreak),
    };

    (
        if primary_asc { "ASC" } else { "DESC" },
        if tiebreak_asc { "ASC" } else { "DESC" },
    )
}

/// The keyset comparison operator for the primary column, given the
/// natural sort direction and traversal mode.
///
/// Forward over an ASC sort wants rows with value strictly greater
/// than the cursor; forward over DESC wants strictly lesser. Backward
/// flips it. The tiebreak comparison on `pool_address` follows the
/// tiebreak direction.
fn keyset_operators(sort: PoolSort, mode: QueryMode) -> (&'static str, &'static str) {
    let primary_gt = match (sort.is_ascending(), mode) {
        (true, QueryMode::Forward) => true,
        (true, QueryMode::Backward) => false,
        (false, QueryMode::Forward) => false,
        (false, QueryMode::Backward) => true,
    };
    // Tiebreak direction mirrors the effective tiebreak order: ASC
    // tiebreak → '>' , DESC tiebreak → '<'.
    let (_, tiebreak_dir) = effective_order(sort, mode);
    let tiebreak_gt = tiebreak_dir == "ASC";

    (
        if primary_gt { ">" } else { "<" },
        if tiebreak_gt { ">" } else { "<" },
    )
}

/// Build the full paginated query.
pub(super) fn build(q: PaginatedPoolsQuery) -> QueryBuilder<'static, Postgres> {
    let sort_col = column_sql(q.sort.column());
    let (primary_order, tiebreak_order) = effective_order(q.sort, q.mode);

    let mut qb: QueryBuilder<Postgres> = QueryBuilder::new(
        "SELECT pool_address, protocol, token_a_mint, token_b_mint, \
         fee_bps, first_seen_at, last_seen_at FROM pools WHERE 1=1",
    );

    // ── Keyset cursor predicate ──────────────────────────────────
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

    // ── Search filter ────────────────────────────────────────────
    if let Some(term) = q.search {
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

    // ── Order + limit ────────────────────────────────────────────
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
