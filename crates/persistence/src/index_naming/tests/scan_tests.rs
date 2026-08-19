//! What the scanner reads, and what it refuses to guess at.

use super::*;

#[test]
fn scan_should_refuse_a_hypertable_argument_it_cannot_vouch_for() {
    assert!(
        scan_err("SELECT create_hypertable('t', 'ts', number_partitions => 4);")
            .contains("number_partitions")
    );
}

#[test]
fn scan_should_refuse_the_legacy_positional_partitioning_column() {
    assert!(
        scan_err("SELECT create_hypertable('t', 'ts', 'device_id', 4);")
            .contains("legacy partitioning_column")
    );
}

#[test]
fn scan_should_read_a_plain_index_declaration() {
    let scanned = scan_sql(
        "synthetic.sql",
        "CREATE UNIQUE INDEX ON t (signature, event_index, timestamp);",
    )
    .expect("plain declarations parse");

    let Event::Index(decl) = &scanned.events[0] else {
        panic!("expected an index, got {:?}", scanned.events[0]);
    };
    assert_eq!(decl.columns, key_columns());
    assert_eq!(decl.explicit_name, None);
    assert!(decl.unique);
}

#[test]
fn scan_should_ignore_sort_order_and_opclass_on_a_column() {
    let replayed = run("CREATE INDEX ON t (pool_address, timestamp DESC NULLS LAST);");

    assert_eq!(replayed.named[0].0, "t_pool_address_timestamp_idx");
}

#[test]
fn scan_should_read_a_declaration_spread_over_several_lines() {
    let replayed = run(
        "CREATE INDEX idx_pools_needs_refresh\n    ON pools (needs_refresh)\n    WHERE needs_refresh;",
    );

    assert_eq!(replayed.named[0].0, "idx_pools_needs_refresh");
}

/// Unquoted identifiers fold to lower case on the server, so the guard must
/// fold them too rather than refuse a legal statement.
#[test]
fn scan_should_fold_unquoted_identifiers_to_lower_case() {
    let replayed = run("CREATE INDEX ON Pools (Protocol);");

    assert_eq!(replayed.named[0].0, "pools_protocol_idx");
}

/// The backstop for every silent-skip path at once: if a statement mentions
/// CREATE INDEX and the parser did not turn it into one, that is an error.
#[test]
fn scan_should_refuse_when_an_index_statement_was_not_decomposed() {
    // The *code* says CREATE INDEX and the parser produced nothing — which is
    // what a mis-split or an unrecognised form looks like from the outside. A
    // string literal saying it does not count; see the COMMENT ON test above.
    let error = scan_err("EXPLAIN CREATE INDEX ON t (a);");

    assert!(error.contains("were decomposed"), "{error}");
}

#[test]
fn scan_should_refuse_an_expression_index() {
    assert!(scan_err("CREATE INDEX ON t (lower(mint));").contains("unsupported index element"));
}

#[test]
fn scan_should_refuse_an_include_clause() {
    assert!(scan_err("CREATE INDEX ON t (a) INCLUDE (b);").contains("INCLUDE"));
}

#[test]
fn scan_should_refuse_a_repeated_column() {
    assert!(scan_err("CREATE INDEX ON t (a, a);").contains("appears twice"));
}

#[test]
fn scan_should_refuse_a_quoted_table_name() {
    assert!(scan_err("CREATE INDEX ON \"MixedCase\" (a);").contains("unsupported table reference"));
}

#[test]
fn scan_should_refuse_an_unnamed_unique_constraint_on_a_table() {
    let error = scan_err("CREATE TABLE t (a TEXT, b TEXT, UNIQUE (a, b));");

    assert!(error.contains("unnamed UNIQUE constraint"), "{error}");
}

/// Same index, other spelling — `ALTER TABLE … ADD UNIQUE` builds
/// `<table>_<cols>_key` just as an inline constraint does.
#[test]
fn scan_should_refuse_an_unnamed_unique_added_by_alter_table() {
    let error = scan_err("ALTER TABLE t ADD UNIQUE (a, b);");

    assert!(error.contains("unnamed UNIQUE constraint"), "{error}");
}

#[test]
fn scan_should_accept_a_named_unique_constraint() {
    let replayed = run("CREATE TABLE t (a TEXT, b TEXT, CONSTRAINT t_a_b_key UNIQUE (a, b));");

    assert_eq!(replayed.tables, vec!["t".to_string()]);
}

/// An unlogged or temporary table is still a table: it must reach the `_pkey`
/// length check and the unnamed-UNIQUE refusal like any other.
#[test]
fn scan_should_recognise_an_unlogged_table() {
    let error = scan_err("CREATE UNLOGGED TABLE t (a TEXT, UNIQUE (a));");

    assert!(error.contains("unnamed UNIQUE constraint"), "{error}");
}

#[test]
fn scan_should_refuse_a_drop_kind_it_has_not_ruled_on() {
    let error = scan_err("DROP SCHEMA public CASCADE;");

    assert!(error.contains("`DROP SCHEMA` is not modelled"), "{error}");
}

/// A column name the scanner cannot read used to fall back to `None`, which the
/// replay reads as "drop the whole relation" — a silent widening in a file that
/// hard-errors everywhere else a name is unreadable.
#[test]
fn scan_should_refuse_a_drop_column_whose_name_it_cannot_read() {
    let error = scan_err("ALTER TABLE pools DROP COLUMN \"Weird\";");

    assert!(error.contains("cannot read the column"), "{error}");
}

#[test]
fn scan_should_refuse_a_timescale_helper_that_may_add_its_own_index() {
    let error = scan_err("SELECT add_dimension('t', 'device_id', number_partitions => 4);");

    assert!(error.contains("add_dimension"), "{error}");
}

#[test]
fn scan_should_accept_a_schema_qualified_create_table() {
    let replayed = run("CREATE TABLE public.foo (a TEXT);");

    assert_eq!(replayed.tables, vec!["foo".to_string()]);
}
