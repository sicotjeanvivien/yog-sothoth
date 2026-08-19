//! Statement splitting: comments, quoting, and DDL hidden in a `$tag$` body.

use super::*;

#[test]
fn scan_should_skip_comments_and_dollar_quoted_bodies() {
    let sql = "-- CREATE INDEX ON commented_out (a);\n\
               CREATE FUNCTION f() RETURNS INTERVAL LANGUAGE SQL AS $$ SELECT INTERVAL '1 h'; $$;\n\
               CREATE INDEX ON real_table (a);";

    let replayed = run(sql);

    assert_eq!(replayed.named.len(), 1);
    assert_eq!(replayed.named[0].1.table, "real_table");
}

/// A dollar-quoted body is skipped, but not unread: index DDL hidden in a `DO`
/// block would be invisible to the parser *and* to the count cross-check.
#[test]
fn scan_should_refuse_index_ddl_hidden_in_a_dollar_quoted_body() {
    let error = scan_err("DO $$ BEGIN CREATE INDEX ON t (a); END $$;");

    assert!(error.contains("invisible to this guard"), "{error}");
}

/// A comment must become a *space*, or `INDEX/**/ON` welds into `INDEXON` and
/// the statement stops looking index-shaped — the quiet way to lose coverage.
#[test]
fn scan_should_not_weld_tokens_across_a_comment() {
    assert_eq!(run("CREATE INDEX/**/ON t (a);").named.len(), 1);
}

/// Postgres nests block comments; a scanner that stops at the first `*/` would
/// read the tail of the comment as live SQL.
#[test]
fn scan_should_nest_block_comments() {
    let sql = "/* outer /* inner */ CREATE INDEX ON hidden (a); */\n\
               CREATE INDEX ON real_table (b);";

    let replayed = run(sql);

    let tables: Vec<&str> = replayed
        .named
        .iter()
        .map(|(_, decl)| decl.table.as_str())
        .collect();
    assert_eq!(
        tables,
        vec!["real_table"],
        "the commented-out one is not live"
    );
}

#[test]
fn scan_should_refuse_an_escape_string_literal() {
    let error = scan_err("INSERT INTO x VALUES (E'\\'');\nCREATE INDEX ON t (a);");

    assert!(error.contains("escape string literals"), "{error}");
}

/// …but only when the `E` stands alone: `DATE'…'` and `ELSE'x'` are ordinary
/// literals, and aborting the whole file on them would be a guard nobody keeps.
#[test]
fn scan_should_not_mistake_a_typed_literal_for_an_escape_string() {
    let replayed = run("INSERT INTO x VALUES (DATE'2026-01-01');\nCREATE INDEX ON t (a);");

    assert_eq!(replayed.named.len(), 1);
}

/// A drop hidden in a `DO` block is as invisible as a creation, and a231e9b
/// started modelling drops — so the body has to be searched for those too.
#[test]
fn scan_should_refuse_a_drop_hidden_in_a_dollar_quoted_body() {
    let error = scan_err("DO $$ BEGIN EXECUTE 'DROP INDEX pools_protocol_idx'; END $$;");

    assert!(error.contains("invisible to this guard"), "{error}");
}
