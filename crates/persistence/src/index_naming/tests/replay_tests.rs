//! The namespace over time: collisions, drops, renames, hypertable defaults.

use super::*;

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

/// `DROP TRIGGER x ON pools` names a table, but frees nothing. Reading every
/// non-INDEX drop as a relation drop turned this into "drop `pools`" — and `ON`
/// parses as a relation name too, so it dropped three things. That is the guard
/// reporting green on a collision the server does produce.
#[test]
fn dropping_a_trigger_frees_no_index_name() {
    let replayed = run("CREATE INDEX ON pools (protocol);\n\
                        DROP TRIGGER IF EXISTS ts_insert_blocker ON pools;\n\
                        CREATE INDEX ON pools (protocol);");

    assert_eq!(replayed.collisions.len(), 1, "{:#?}", replayed.collisions);
    assert_eq!(replayed.collisions[0].generated, "pools_protocol_idx1");
}

#[test]
fn dropping_a_policy_frees_no_index_name() {
    let replayed = run("CREATE INDEX ON pools (protocol);\n\
                        DROP POLICY p ON pools;\n\
                        CREATE INDEX ON pools (protocol);");

    assert_eq!(replayed.collisions.len(), 1);
}

/// A migration replacing one of the two SQL functions of `005` must not break
/// the guard — the old parser choked on the argument list.
#[test]
fn dropping_a_function_is_not_a_parse_failure() {
    let replayed = run("DROP FUNCTION yog_price_max_age_asof();\n\
                        CREATE INDEX ON pools (protocol);");

    assert_eq!(replayed.named.len(), 1);
}

#[test]
fn a_drop_column_if_exists_still_frees_its_indexes() {
    let replayed = run("CREATE INDEX ON pools (protocol);\n\
                        ALTER TABLE pools DROP COLUMN IF EXISTS protocol;\n\
                        CREATE INDEX ON pools (protocol);");

    assert!(replayed.collisions.is_empty(), "{:#?}", replayed.collisions);
}

/// The `COLUMN` keyword is optional in Postgres.
#[test]
fn a_bare_drop_column_is_understood() {
    let replayed = run("CREATE INDEX ON pools (protocol);\n\
                        ALTER TABLE pools DROP protocol;\n\
                        CREATE INDEX ON pools (protocol);");

    assert!(replayed.collisions.is_empty(), "{:#?}", replayed.collisions);
}

/// A table rename carries its indexes with it — otherwise the replay keeps
/// looking for them under the old table and invents a hypertable default index
/// that the server never creates.
#[test]
fn a_table_rename_carries_its_indexes_onto_the_new_name() {
    let replayed = run("CREATE INDEX ON old_t (ts);\n\
                        ALTER TABLE old_t RENAME TO new_t;\n\
                        SELECT create_hypertable('new_t', 'ts');");

    assert_eq!(
        replayed.named.len(),
        1,
        "the renamed index is reused, no default is added: {:#?}",
        replayed.named
    );
    assert_eq!(replayed.tables, Vec::<String>::new());
}

#[test]
fn a_column_rename_follows_into_the_indexes_that_use_it() {
    let replayed = run("CREATE INDEX ON t (old_ts);\n\
                        ALTER TABLE t RENAME COLUMN old_ts TO ts;\n\
                        SELECT create_hypertable('t', 'ts');");

    assert_eq!(replayed.named.len(), 1, "{:#?}", replayed.named);
}

/// The remedy both READMEs prescribe must obey the bound they state: Postgres
/// truncates an over-long rename target silently, handing the problem back.
#[test]
fn a_rename_target_longer_than_the_identifier_limit_is_refused() {
    let scanned = scan_sql(
        "synthetic.sql",
        &format!(
            "CREATE INDEX ON pools (protocol);\nALTER INDEX pools_protocol_idx RENAME TO {};",
            "z".repeat(70)
        ),
    )
    .expect("parses");

    let error = replay(&scanned.events).expect_err("an over-long rename must be refused");

    assert!(error.contains("Postgres truncates it"), "{error}");
}

#[test]
fn a_rename_onto_a_taken_name_is_refused_even_from_an_unmodelled_index() {
    let scanned = scan_sql(
        "synthetic.sql",
        "CREATE INDEX ON pools (protocol);\n\
         ALTER INDEX pools_pkey RENAME TO pools_protocol_idx;",
    )
    .expect("parses");

    let error = replay(&scanned.events).expect_err("the target name is already taken");

    assert!(error.contains("already taken"), "{error}");
}

/// Two indexes cannot share a name on the server, so a replay that accepts it
/// is modelling a database that cannot exist — and its `live` set goes wrong.
#[test]
fn an_explicit_name_that_duplicates_an_existing_one_is_refused() {
    let scanned = scan_sql(
        "synthetic.sql",
        "CREATE INDEX ON pools (protocol);\n\
         CREATE INDEX pools_protocol_idx ON pools (last_seen_at);",
    )
    .expect("parses");

    let error = replay(&scanned.events).expect_err("duplicate names must be refused");

    assert!(error.contains("already taken"), "{error}");
}

/// These migrations use `COMMENT ON … IS '…'` heavily. A comment that mentions
/// CREATE INDEX must not fail the whole file.
#[test]
fn a_string_literal_mentioning_create_index_is_not_counted() {
    let replayed = run("COMMENT ON TABLE pools IS 'use CREATE INDEX here';\n\
                        CREATE INDEX ON pools (protocol);");

    assert_eq!(replayed.named.len(), 1);
}

/// A table that no longer exists must stop constraining the naming rules.
#[test]
fn a_dropped_table_leaves_the_table_list() {
    let replayed = run("CREATE TABLE t (a TEXT);\nDROP TABLE t;");

    assert_eq!(replayed.tables, Vec::<String>::new());
}
