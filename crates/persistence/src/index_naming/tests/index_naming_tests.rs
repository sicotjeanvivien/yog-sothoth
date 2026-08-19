use super::*;

// ─────────────────────────────────────────────────────────────────────────────
// The port, checked against names Postgres actually produced
// ─────────────────────────────────────────────────────────────────────────────

/// The one collision the schema already carries, and the truncated default index
/// `create_hypertable` builds on the longest table. All four strings were read
/// off a live `timescale/timescaledb:latest-pg16` before being asserted here;
/// they are the fixture the whole guard rests on.
const DURATION_TABLE: &str = "meteora_damm_v2_update_reward_duration_events";
const FUNDER_TABLE: &str = "meteora_damm_v2_update_reward_funder_events";
const DURATION_INDEX: &str = "meteora_damm_v2_update_reward_signature_event_index_timesta_idx";
const FUNDER_INDEX: &str = "meteora_damm_v2_update_reward_signature_event_index_timest_idx1";
const LONGEST_TABLE: &str = "meteora_damm_v2_withdraw_dead_liquidity_reward_events";
const LONGEST_HYPERTABLE_INDEX: &str =
    "meteora_damm_v2_withdraw_dead_liquidity_reward_ev_timestamp_idx";

const KEY_COLUMNS: [&str; 3] = ["signature", "event_index", "timestamp"];

fn key_columns() -> Vec<String> {
    KEY_COLUMNS.iter().map(|c| c.to_string()).collect()
}

fn decl(table: &str, columns: &[&str]) -> IndexDecl {
    IndexDecl {
        file: "synthetic.sql".to_string(),
        line: 1,
        explicit_name: None,
        table: table.to_string(),
        columns: columns.iter().map(|c| c.to_string()).collect(),
        origin: Origin::Written,
    }
}

fn events(decls: Vec<IndexDecl>) -> Vec<Event> {
    decls.into_iter().map(Event::Index).collect()
}

fn collided(decls: Vec<IndexDecl>) -> Vec<Collision> {
    collisions(&events(decls)).expect("no name is freed in these fixtures")
}

#[test]
fn make_object_name_should_leave_a_short_name_untouched() {
    assert_eq!(
        make_object_name("pools", Some("protocol"), "idx"),
        "pools_protocol_idx"
    );
}

#[test]
fn make_object_name_should_reproduce_the_known_truncated_name() {
    let addition = choose_index_name_addition(&key_columns());

    let name = make_object_name(DURATION_TABLE, Some(&addition), "idx");

    assert_eq!(name, DURATION_INDEX);
    assert_eq!(name.len(), MAX_IDENTIFIER_LEN);
}

/// The suffix lands on the *label*, so `idx1` costs one more character than
/// `idx` and the column part loses one too. This is the detail that makes
/// "the first 63 characters" the wrong mental model.
#[test]
fn make_object_name_should_shorten_further_when_the_label_grows() {
    let addition = choose_index_name_addition(&key_columns());

    let plain = make_object_name(FUNDER_TABLE, Some(&addition), "idx");
    let suffixed = make_object_name(FUNDER_TABLE, Some(&addition), "idx1");

    assert_eq!(plain, DURATION_INDEX, "truncates onto the same name");
    assert_eq!(suffixed, FUNDER_INDEX);
    assert_ne!(
        plain.trim_end_matches("_idx"),
        suffixed.trim_end_matches("_idx1"),
        "the column part must lose a character, not just the label"
    );
}

/// `makeObjectName` trims the longer part first, alternating — it does not cut
/// the tail of the concatenation.
#[test]
fn make_object_name_should_trim_the_longer_part_first() {
    let long_table = "a".repeat(50);
    let long_columns = "b".repeat(50);

    let name = make_object_name(&long_table, Some(&long_columns), "idx");

    let (table_part, column_part) = name
        .trim_end_matches("_idx")
        .split_once('_')
        .expect("the two parts stay separated");
    assert_eq!(
        table_part.len(),
        column_part.len(),
        "equal-length parts must be trimmed evenly, got {name}"
    );
    assert_eq!(name.len(), MAX_IDENTIFIER_LEN);
}

#[test]
fn choose_index_name_addition_should_join_column_names_with_underscores() {
    assert_eq!(
        choose_index_name_addition(&key_columns()),
        "signature_event_index_timestamp"
    );
}

#[test]
fn choose_index_name_addition_should_stop_once_the_buffer_is_full() {
    let columns = vec!["c".repeat(40), "d".repeat(40), "e".repeat(40)];

    let addition = choose_index_name_addition(&columns);

    assert_eq!(
        addition.len(),
        81,
        "two columns and a separator, then it stops"
    );
    assert!(!addition.contains('e'), "the third column is never reached");
}

#[test]
fn choose_relation_name_should_count_the_passes_it_needed() {
    let mut taken = HashSet::new();
    taken.insert("pools_protocol_idx".to_string());

    let (name, passes) = choose_relation_name("pools", Some("protocol"), "idx", &taken);

    assert_eq!(name, "pools_protocol_idx1");
    assert_eq!(passes, 1);
}

// ─────────────────────────────────────────────────────────────────────────────
// The detector bites — proved on synthetic DDL, not by running the real files
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn collisions_should_report_a_colliding_pair() {
    let found = collided(vec![
        decl(DURATION_TABLE, &KEY_COLUMNS),
        decl(FUNDER_TABLE, &KEY_COLUMNS),
    ]);

    assert_eq!(found.len(), 1, "got {found:#?}");
    assert_eq!(found[0].table, FUNDER_TABLE);
    assert_eq!(found[0].generated, FUNDER_INDEX);
}

/// The mutation that must make the guard red: the same pair minus one member
/// collides with nothing.
#[test]
fn collisions_should_be_empty_without_the_second_member() {
    assert!(collided(vec![decl(DURATION_TABLE, &KEY_COLUMNS)]).is_empty());
}

/// A brand-new table whose name survives truncation onto an existing one is
/// exactly the scenario this guard exists for: adding it must be caught.
#[test]
fn collisions_should_catch_a_third_table_added_to_the_colliding_family() {
    let found = collided(vec![
        decl(DURATION_TABLE, &KEY_COLUMNS),
        decl(FUNDER_TABLE, &KEY_COLUMNS),
        decl(
            "meteora_damm_v2_update_reward_authority_events",
            &KEY_COLUMNS,
        ),
    ]);

    assert_eq!(found.len(), 2, "got {found:#?}");
    assert_eq!(
        found[1].generated.trim_end_matches("_idx2").len(),
        MAX_IDENTIFIER_LEN - "_idx2".len(),
        "the third one takes the idx2 suffix: {}",
        found[1].generated
    );
}

/// The whole point of the ticket: which table wears the `1` is decided by
/// declaration order and by nothing else. Swap the two and the suffix moves —
/// a schema that is not the same one, with no error raised.
#[test]
fn the_suffix_follows_declaration_order() {
    let as_declared = collided(vec![
        decl(DURATION_TABLE, &KEY_COLUMNS),
        decl(FUNDER_TABLE, &KEY_COLUMNS),
    ]);
    let swapped = collided(vec![
        decl(FUNDER_TABLE, &KEY_COLUMNS),
        decl(DURATION_TABLE, &KEY_COLUMNS),
    ]);

    assert_eq!(as_declared[0].table, FUNDER_TABLE);
    assert_eq!(swapped[0].table, DURATION_TABLE);
    assert_eq!(
        as_declared[0].generated, swapped[0].generated,
        "the same name is handed to whichever table comes second"
    );
}

/// An explicitly named index is reserved as written — Postgres errors on a
/// duplicate rather than suffixing it — so it must still consume the name.
#[test]
fn collisions_should_account_for_explicitly_named_indexes() {
    let mut explicit = decl("pools", &["protocol"]);
    explicit.explicit_name = Some("pools_protocol_idx".to_string());

    let found = collided(vec![explicit, decl("pools", &["protocol"])]);

    assert_eq!(found.len(), 1);
    assert_eq!(found[0].generated, "pools_protocol_idx1");
}

/// A name freed mid-replay shifts every suffix after it, and the replay cannot
/// be right about that — so it must refuse rather than carry on.
#[test]
fn collisions_should_refuse_a_statement_that_frees_a_modelled_name() {
    let dropped = Event::Drop(DropStmt {
        file: "synthetic.sql".to_string(),
        line: 2,
        table: Some("pools".to_string()),
        index: None,
        column: Some("protocol".to_string()),
        statement: "ALTER TABLE pools DROP COLUMN protocol".to_string(),
    });

    let error = collisions(&[Event::Index(decl("pools", &["protocol"])), dropped])
        .expect_err("freeing a modelled name must be refused");

    assert!(
        error.contains("frees the index name `pools_protocol_idx`"),
        "{error}"
    );
}

/// …but a drop that touches nothing we model is not a reason to stop: the
/// migrations already drop views and unindexed columns.
#[test]
fn collisions_should_ignore_a_drop_that_frees_nothing_modelled() {
    let dropped = Event::Drop(DropStmt {
        file: "synthetic.sql".to_string(),
        line: 2,
        table: Some("pool_current_state".to_string()),
        index: None,
        column: Some("liquidity".to_string()),
        statement: "ALTER TABLE pool_current_state DROP COLUMN liquidity".to_string(),
    });

    let found = collisions(&[Event::Index(decl("pools", &["protocol"])), dropped])
        .expect("an unrelated drop is not a reason to refuse");

    assert!(found.is_empty());
}

// ─────────────────────────────────────────────────────────────────────────────
// create_hypertable — the 21 index names nobody writes down
// ─────────────────────────────────────────────────────────────────────────────

/// Read off a live TimescaleDB: `create_hypertable` builds a default index on
/// the time dimension, on the root table, in `public`, named by the very
/// algorithm this module ports — and on our longest table it is already
/// truncated to 63 bytes. Miss it and the replay believes a taken name is free.
#[test]
fn a_hypertable_contributes_its_default_time_dimension_index() {
    let scanned = scan_sql(
        "synthetic.sql",
        "SELECT create_hypertable('meteora_damm_v2_withdraw_dead_liquidity_reward_events', \
         'timestamp', chunk_time_interval => INTERVAL '7 days');",
    )
    .expect("the call parses");

    let decls: Vec<&IndexDecl> = scanned.indexes().collect();
    assert_eq!(decls.len(), 1);
    assert_eq!(decls[0].origin, Origin::HypertableDefault);
    assert_eq!(decls[0].table, LONGEST_TABLE);
    assert_eq!(decls[0].columns, vec!["timestamp".to_string()]);

    let (name, passes) = generated_index_name(decls[0], &HashSet::new());
    assert_eq!(name, LONGEST_HYPERTABLE_INDEX);
    assert_eq!(name.len(), MAX_IDENTIFIER_LEN);
    assert_eq!(passes, 0);
}

/// The failure the review reproduced against a real database: without the
/// hypertable default in `taken`, a later index that truncates onto it is
/// predicted un-suffixed and the guard reports green on a real collision.
#[test]
fn a_hypertable_default_index_takes_the_name_a_later_index_would_want() {
    let sql = "SELECT create_hypertable('token_prices', 'fetched_at');\n\
               CREATE INDEX ON token_prices (fetched_at);";

    let scanned = scan_sql("synthetic.sql", sql).expect("parses");
    let found = collisions(&scanned.events).expect("nothing is freed");

    assert_eq!(found.len(), 1, "got {found:#?}");
    assert_eq!(found[0].origin, Origin::Written);
    assert_eq!(found[0].generated, "token_prices_fetched_at_idx1");
}

#[test]
fn a_hypertable_with_default_indexes_disabled_contributes_nothing() {
    let scanned = scan_sql(
        "synthetic.sql",
        "SELECT create_hypertable('t', 'ts', create_default_indexes => FALSE);",
    )
    .expect("parses");

    assert_eq!(scanned.indexes().count(), 0);
}

#[test]
fn scan_should_refuse_a_hypertable_argument_it_cannot_vouch_for() {
    let error = scan_err("SELECT create_hypertable('t', 'ts', number_partitions => 4);");

    assert!(error.contains("number_partitions"), "{error}");
}

#[test]
fn scan_should_refuse_the_legacy_positional_partitioning_column() {
    let error = scan_err("SELECT create_hypertable('t', 'ts', 'device_id', 4);");

    assert!(error.contains("legacy partitioning_column"), "{error}");
}

// ─────────────────────────────────────────────────────────────────────────────
// The scanner refuses what it cannot model
// ─────────────────────────────────────────────────────────────────────────────

fn scan_err(sql: &str) -> String {
    scan_sql("synthetic.sql", sql).expect_err("this DDL must be refused")
}

#[test]
fn scan_should_read_a_plain_index_declaration() {
    let scanned = scan_sql(
        "synthetic.sql",
        "CREATE UNIQUE INDEX ON t (signature, event_index, timestamp);",
    )
    .expect("plain declarations parse");

    let decls: Vec<&IndexDecl> = scanned.indexes().collect();
    assert_eq!(decls.len(), 1);
    assert_eq!(decls[0].columns, key_columns());
    assert_eq!(decls[0].explicit_name, None);
}

#[test]
fn scan_should_ignore_sort_order_and_opclass_on_a_column() {
    let scanned = scan_sql(
        "synthetic.sql",
        "CREATE INDEX ON t (pool_address, timestamp DESC NULLS LAST);",
    )
    .expect("modifiers parse");

    assert_eq!(
        scanned.indexes().next().unwrap().columns,
        vec!["pool_address".to_string(), "timestamp".to_string()]
    );
}

#[test]
fn scan_should_read_a_declaration_spread_over_several_lines() {
    let scanned = scan_sql(
        "synthetic.sql",
        "CREATE INDEX idx_pools_needs_refresh\n    ON pools (needs_refresh)\n    WHERE needs_refresh;",
    )
    .expect("multi-line declarations parse");

    let decl = scanned.indexes().next().unwrap();
    assert_eq!(
        decl.explicit_name.as_deref(),
        Some("idx_pools_needs_refresh")
    );
    assert_eq!(decl.columns, vec!["needs_refresh".to_string()]);
}

#[test]
fn scan_should_skip_comments_and_dollar_quoted_bodies() {
    let sql = "-- CREATE INDEX ON commented_out (a);\n\
               CREATE FUNCTION f() RETURNS INTERVAL LANGUAGE SQL AS $$ SELECT INTERVAL '1 h'; $$;\n\
               CREATE INDEX ON real_table (a);";

    let scanned = scan_sql("synthetic.sql", sql).expect("parses");

    assert_eq!(scanned.indexes().count(), 1);
    assert_eq!(scanned.indexes().next().unwrap().table, "real_table");
}

/// A comment must become a *space*, or `INDEX/**/ON` welds into `INDEXON` and
/// the statement stops looking index-shaped — the quiet way to lose coverage.
#[test]
fn scan_should_not_weld_tokens_across_a_comment() {
    let scanned = scan_sql("synthetic.sql", "CREATE INDEX/**/ON t (a);").expect("parses");

    assert_eq!(scanned.indexes().count(), 1);
}

/// Postgres nests block comments; a scanner that stops at the first `*/` would
/// read the tail of the comment as live SQL.
#[test]
fn scan_should_nest_block_comments() {
    let sql = "/* outer /* inner */ CREATE INDEX ON hidden (a); */\n\
               CREATE INDEX ON real_table (b);";

    let scanned = scan_sql("synthetic.sql", sql).expect("parses");

    let tables: Vec<&str> = scanned.indexes().map(|d| d.table.as_str()).collect();
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

/// The backstop for every silent-skip path at once: if a statement mentions
/// CREATE INDEX and the parser did not turn it into one, that is an error.
#[test]
fn scan_should_refuse_when_an_index_statement_was_not_decomposed() {
    // Nothing here is decomposable into an index, yet the text says CREATE
    // INDEX — which is exactly what a mis-split or an unrecognised form looks
    // like from the outside.
    let error = scan_err("INSERT INTO audit (note) VALUES ('CREATE INDEX ON t (a)');");

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
fn scan_should_refuse_a_schema_qualified_table() {
    assert!(scan_err("CREATE INDEX ON public.t (a);").contains("unsupported table reference"));
}

#[test]
fn scan_should_refuse_an_unnamed_unique_constraint() {
    let error = scan_err("CREATE TABLE t (a TEXT, b TEXT, UNIQUE (a, b));");

    assert!(error.contains("unnamed UNIQUE constraint"), "{error}");
}

#[test]
fn scan_should_accept_a_named_unique_constraint() {
    let scanned = scan_sql(
        "synthetic.sql",
        "CREATE TABLE t (a TEXT, b TEXT, CONSTRAINT t_a_b_key UNIQUE (a, b));",
    )
    .expect("a named constraint is fine");

    assert_eq!(scanned.tables, vec!["t".to_string()]);
}

// ─────────────────────────────────────────────────────────────────────────────
// The guard over the real migration files
// ─────────────────────────────────────────────────────────────────────────────

/// The collisions the schema is *known* to carry. Adding a table whose index
/// name truncates onto an existing one moves the `1` from one table to another
/// and drifts the deployed schema from a freshly migrated one, silently.
///
/// If this list has to grow, that is a decision to take deliberately — name the
/// new index explicitly instead (see `migrations/README.md`).
const PINNED_COLLISIONS: &[(&str, &str)] = &[(FUNDER_TABLE, FUNDER_INDEX)];

#[test]
fn migrations_should_not_introduce_a_new_index_name_collision() {
    let scanned = scan_migrations().expect("every migration must be parseable");

    let found = collisions(&scanned.events).expect("no migration may free a modelled index name");

    let actual: Vec<(String, String)> = found
        .iter()
        .map(|c| (c.table.clone(), c.generated.clone()))
        .collect();
    let expected: Vec<(String, String)> = PINNED_COLLISIONS
        .iter()
        .map(|(table, name)| (table.to_string(), name.to_string()))
        .collect();

    assert_eq!(
        actual, expected,
        "index name collisions changed.\n  found: {found:#?}\nName the new index explicitly \
         (<= {MAX_IDENTIFIER_LEN} characters) rather than pinning another collision."
    );
}

/// `001_baseline.sql` is frozen — migrations are forward-only — so these counts
/// can never go stale, and they turn the ticket's prose into an assertion.
///
/// 49 written `CREATE INDEX` lines plus the 21 default indexes
/// `create_hypertable` builds without anyone writing them down: 70 names in
/// `public`, which is what the live schema holds.
#[test]
fn the_baseline_carries_seventy_auto_named_indexes_thirty_six_of_them_truncated() {
    let baseline = migrations_dir().join("001_baseline.sql");
    let scanned = scan_file(&baseline).expect("the baseline must be parseable");

    let auto_named: Vec<&IndexDecl> = scanned
        .indexes()
        .filter(|decl| decl.explicit_name.is_none())
        .collect();
    let written = auto_named
        .iter()
        .filter(|decl| decl.origin == Origin::Written)
        .count();
    let from_hypertables = auto_named
        .iter()
        .filter(|decl| decl.origin == Origin::HypertableDefault)
        .count();
    let truncated = auto_named
        .iter()
        .filter(|decl| {
            let addition = choose_index_name_addition(&decl.columns);
            decl.table.len() + addition.len() + "__idx".len() > MAX_IDENTIFIER_LEN
        })
        .count();

    assert_eq!(written, 49, "written CREATE INDEX lines");
    assert_eq!(from_hypertables, 21, "create_hypertable default indexes");
    assert_eq!(auto_named.len(), 70, "auto-named indexes in public");
    assert_eq!(truncated, 36, "of which truncated");
}

/// Closes the blind spot left by not parsing `CREATE TABLE` bodies for primary
/// keys: `<table>_pkey` carries no column part, so it only truncates once the
/// table name passes 58 characters. The longest today is 53. (Unnamed `UNIQUE`
/// constraints, which *do* carry a column part, are refused outright by the
/// scanner instead.)
#[test]
fn no_table_name_is_long_enough_for_its_pkey_to_truncate() {
    let scanned = scan_migrations().expect("every migration must be parseable");

    for table in &scanned.tables {
        assert_eq!(
            make_object_name(table, None, "pkey"),
            format!("{table}_pkey"),
            "`{table}` is {} characters — its primary key index name truncates, which this \
             guard does not model. Shorten the table name.",
            table.len()
        );
    }
}

/// Postgres truncates an over-long *explicit* name too, silently, which would
/// hand it straight back to the collision problem.
#[test]
fn every_explicitly_named_index_fits_without_truncation() {
    let scanned = scan_migrations().expect("every migration must be parseable");

    for decl in scanned.indexes() {
        if let Some(name) = &decl.explicit_name {
            assert!(
                name.len() <= MAX_IDENTIFIER_LEN,
                "{}:{}: `{name}` is {} characters and would be truncated to \
                 {MAX_IDENTIFIER_LEN}",
                decl.file,
                decl.line,
                name.len()
            );
        }
    }
}
