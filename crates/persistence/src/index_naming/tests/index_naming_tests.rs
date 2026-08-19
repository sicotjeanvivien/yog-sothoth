use super::*;

// ─────────────────────────────────────────────────────────────────────────────
// The port, checked against names Postgres actually produced
// ─────────────────────────────────────────────────────────────────────────────

/// Fixtures read off a live `timescale/timescaledb:latest-pg16` before being
/// asserted here: the one collision the schema already carries, and the
/// truncated default index `create_hypertable` builds on the longest table.
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
        unique: false,
        origin: Origin::Written,
    }
}

fn collided(decls: Vec<IndexDecl>) -> Vec<Collision> {
    replay(&decls.into_iter().map(Event::Index).collect::<Vec<_>>())
        .expect("these fixtures replay cleanly")
        .collisions
}

/// Scan then replay, the way the guard does over the real files.
fn run(sql: &str) -> Replay {
    let scanned = scan_sql("synthetic.sql", sql).expect("this DDL must parse");
    replay(&scanned.events).expect("this DDL must replay")
}

fn scan_err(sql: &str) -> String {
    scan_sql("synthetic.sql", sql).expect_err("this DDL must be refused")
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

// ─────────────────────────────────────────────────────────────────────────────
// Names that appear and disappear
// ─────────────────────────────────────────────────────────────────────────────

/// A dropped name goes back into circulation, exactly as it does on the server.
/// Refusing these outright would block ordinary migrations — and would be wrong,
/// because the replay can model them.
#[test]
fn a_dropped_index_frees_its_name_for_the_next_one() {
    let replayed = run("CREATE INDEX ON pools (protocol);\n\
                        DROP INDEX pools_protocol_idx;\n\
                        CREATE INDEX ON pools (protocol);");

    assert!(
        replayed.collisions.is_empty(),
        "the name was free again: {:#?}",
        replayed.collisions
    );
    let names: Vec<&str> = replayed
        .named
        .iter()
        .map(|(name, _)| name.as_str())
        .collect();
    assert_eq!(names, vec!["pools_protocol_idx", "pools_protocol_idx"]);
}

/// …and if it is *not* dropped, the second one is suffixed. Same fixture minus
/// the drop: the mutation that proves the branch above is load-bearing.
#[test]
fn without_the_drop_the_second_index_is_suffixed() {
    let replayed = run("CREATE INDEX ON pools (protocol);\n\
                        CREATE INDEX ON pools (protocol);");

    assert_eq!(replayed.collisions.len(), 1);
    assert_eq!(replayed.collisions[0].generated, "pools_protocol_idx1");
}

#[test]
fn dropping_a_table_frees_every_index_on_it() {
    let replayed = run("CREATE INDEX ON pools (protocol);\n\
                        DROP TABLE other_table, pools;\n\
                        CREATE INDEX ON pools (protocol);");

    assert!(replayed.collisions.is_empty(), "{:#?}", replayed.collisions);
}

#[test]
fn dropping_a_column_frees_the_indexes_that_use_it() {
    let replayed = run("CREATE INDEX ON pools (protocol);\n\
                        ALTER TABLE public.pools DROP COLUMN protocol;\n\
                        CREATE INDEX ON pools (protocol);");

    assert!(replayed.collisions.is_empty(), "{:#?}", replayed.collisions);
}

#[test]
fn a_drop_index_concurrently_still_names_its_target() {
    let replayed = run("CREATE INDEX ON pools (protocol);\n\
                        DROP INDEX CONCURRENTLY IF EXISTS pools_protocol_idx;\n\
                        CREATE INDEX ON pools (protocol);");

    assert!(replayed.collisions.is_empty(), "{:#?}", replayed.collisions);
}

/// `migrations/README.md` prescribes `ALTER INDEX … RENAME TO …` as the way out
/// when this guard goes red, so the guard has to understand it: the old name
/// comes free, the new one is taken.
#[test]
fn a_rename_frees_the_old_name_and_takes_the_new_one() {
    let replayed = run("CREATE INDEX ON pools (protocol);\n\
                        ALTER INDEX pools_protocol_idx RENAME TO idx_pools_protocol;\n\
                        CREATE INDEX ON pools (protocol);");

    assert!(replayed.collisions.is_empty(), "{:#?}", replayed.collisions);
}

#[test]
fn a_rename_onto_a_taken_name_is_refused() {
    let scanned = scan_sql(
        "synthetic.sql",
        "CREATE INDEX ON pools (protocol);\n\
         CREATE INDEX ON pools (last_seen_at);\n\
         ALTER INDEX pools_protocol_idx RENAME TO pools_last_seen_at_idx;",
    )
    .expect("parses");

    let error = replay(&scanned.events).expect_err("this migration would fail to apply");

    assert!(error.contains("already taken"), "{error}");
}

// ─────────────────────────────────────────────────────────────────────────────
// create_hypertable — the index names nobody writes down
// ─────────────────────────────────────────────────────────────────────────────

/// Read off a live TimescaleDB: `create_hypertable` builds a default index on
/// the time dimension, on the root table, in `public`, named by the very
/// algorithm this module ports — and on our longest table it is already
/// truncated to 63 bytes. Miss it and the replay believes a taken name is free.
#[test]
fn a_hypertable_contributes_its_default_time_dimension_index() {
    let replayed = run(
        "SELECT create_hypertable('meteora_damm_v2_withdraw_dead_liquidity_reward_events', \
                        'timestamp', chunk_time_interval => INTERVAL '7 days');",
    );

    assert_eq!(replayed.named.len(), 1);
    let (name, decl) = &replayed.named[0];
    assert_eq!(decl.origin, Origin::HypertableDefault);
    assert_eq!(decl.table, LONGEST_TABLE);
    assert_eq!(name, LONGEST_HYPERTABLE_INDEX);
    assert_eq!(name.len(), MAX_IDENTIFIER_LEN);
}

/// The failure the first review reproduced against a real database: without the
/// hypertable default in the namespace, a later index that truncates onto it is
/// predicted un-suffixed and the guard reports green on a real collision.
#[test]
fn a_hypertable_default_index_takes_the_name_a_later_index_would_want() {
    let replayed = run("SELECT create_hypertable('token_prices', 'fetched_at');\n\
                        CREATE INDEX ON token_prices (fetched_at);");

    assert_eq!(replayed.collisions.len(), 1, "{:#?}", replayed.collisions);
    assert_eq!(replayed.collisions[0].origin, Origin::Written);
    assert_eq!(
        replayed.collisions[0].generated,
        "token_prices_fetched_at_idx1"
    );
}

/// Measured, not assumed: a **non-unique** index already leading on the time
/// column makes TimescaleDB reuse it and skip its default.
#[test]
fn a_hypertable_reuses_a_non_unique_index_that_already_leads_on_the_time_column() {
    let replayed = run("CREATE INDEX ON a (ts DESC);\n\
                        SELECT create_hypertable('a', 'ts');");

    assert_eq!(replayed.named.len(), 1, "no default index is added");
    assert_eq!(replayed.named[0].1.origin, Origin::Written);
}

/// …but a unique index does not suppress it — also measured. Both tables in the
/// live check still got their default index.
#[test]
fn a_unique_index_on_the_time_column_does_not_suppress_the_default() {
    let replayed = run("CREATE UNIQUE INDEX ON d (ts, x);\n\
                        SELECT create_hypertable('d', 'ts');");

    assert_eq!(replayed.named.len(), 2);
    assert_eq!(replayed.named[1].1.origin, Origin::HypertableDefault);
    assert_eq!(replayed.named[1].0, "d_ts_idx");
}

#[test]
fn a_hypertable_with_default_indexes_disabled_contributes_nothing() {
    let replayed = run("SELECT create_hypertable('t', 'ts', create_default_indexes => FALSE);");

    assert!(replayed.named.is_empty());
}

/// The call is recognised however it is spelled, or the statement is refused —
/// a form that slips through is 21 names the replay never enters.
#[test]
fn a_hypertable_call_is_recognised_when_schema_qualified() {
    let replayed = run("SELECT public.create_hypertable('t', 'ts');");

    assert_eq!(replayed.named.len(), 1);
    assert_eq!(replayed.named[0].0, "t_ts_idx");
}

#[test]
fn a_hypertable_call_is_recognised_in_a_from_clause() {
    let replayed = run("SELECT * FROM create_hypertable('t', 'ts');");

    assert_eq!(replayed.named.len(), 1);
}

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

// ─────────────────────────────────────────────────────────────────────────────
// The scanner refuses what it cannot model
// ─────────────────────────────────────────────────────────────────────────────

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

/// The backstop for every silent-skip path at once: if a statement mentions
/// CREATE INDEX and the parser did not turn it into one, that is an error.
#[test]
fn scan_should_refuse_when_an_index_statement_was_not_decomposed() {
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
    let scanned = scan_sql(
        "synthetic.sql",
        "CREATE TABLE t (a TEXT, b TEXT, CONSTRAINT t_a_b_key UNIQUE (a, b));",
    )
    .expect("a named constraint is fine");

    assert_eq!(scanned.tables, vec!["t".to_string()]);
}

/// An unlogged or temporary table is still a table: it must reach the `_pkey`
/// length check and the unnamed-UNIQUE refusal like any other.
#[test]
fn scan_should_recognise_an_unlogged_table() {
    let error = scan_err("CREATE UNLOGGED TABLE t (a TEXT, UNIQUE (a));");

    assert!(error.contains("unnamed UNIQUE constraint"), "{error}");
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

fn replay_migrations() -> Replay {
    let scanned = scan_migrations().expect("every migration must be parseable");
    replay(&scanned.events).expect("every migration must replay")
}

#[test]
fn migrations_should_not_introduce_a_new_index_name_collision() {
    let found = replay_migrations().collisions;

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
/// `create_hypertable` builds that nobody writes down. Add the 7 explicitly
/// named ones and the whole set was diffed name-for-name against a live schema:
/// 77 server-named index relations, no discrepancy. (`public` also holds the
/// `_pkey` and constraint indexes, which this guard does not enumerate.)
#[test]
fn the_baseline_carries_seventy_auto_named_indexes_thirty_six_of_them_truncated() {
    let baseline = migrations_dir().join("001_baseline.sql");
    let scanned = scan_file(&baseline).expect("the baseline must be parseable");
    let replayed = replay(&scanned.events).expect("the baseline must replay");

    let auto_named: Vec<&IndexDecl> = replayed
        .named
        .iter()
        .map(|(_, decl)| decl)
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
    assert_eq!(auto_named.len(), 70, "auto-named index relations");
    assert_eq!(truncated, 36, "of which truncated");
    assert_eq!(
        replayed.named.len(),
        77,
        "including the explicitly named ones"
    );
}

/// Closes the blind spot left by not parsing `CREATE TABLE` bodies for primary
/// keys: `<table>_pkey` carries no column part, so it only truncates once the
/// table name passes 58 characters. The longest today is 53.
#[test]
fn no_table_name_is_long_enough_for_its_pkey_to_truncate() {
    for table in &scan_migrations().expect("parseable").tables {
        assert_eq!(
            make_object_name(table, None, "pkey"),
            format!("{table}_pkey"),
            "`{table}` is {} characters — its primary key index name truncates, which this \
             guard does not model. Shorten the table name.",
            table.len()
        );
    }
}

/// The module claims only index-vs-index collisions need modelling because no
/// relation of ours is named like a generated index. Asserted rather than left
/// as prose — a table named `…_idx` would break the replay's assumption.
#[test]
fn no_table_is_named_like_a_generated_index() {
    for table in &scan_migrations().expect("parseable").tables {
        let tail = table.rsplit('_').next().unwrap_or_default();
        assert!(
            !(tail == "idx"
                || tail
                    .strip_prefix("idx")
                    .is_some_and(|n| n.parse::<u32>().is_ok())),
            "`{table}` is named like a generated index, which the replay assumes never happens"
        );
    }
}

/// Postgres truncates an over-long *explicit* name too, silently, which would
/// hand it straight back to the collision problem.
#[test]
fn every_explicitly_named_index_fits_without_truncation() {
    for (name, decl) in &replay_migrations().named {
        if decl.explicit_name.is_some() {
            assert!(
                name.len() <= MAX_IDENTIFIER_LEN,
                "{}:{}: `{name}` is {} characters and would be truncated to {MAX_IDENTIFIER_LEN}",
                decl.file,
                decl.line,
                name.len()
            );
        }
    }
}
