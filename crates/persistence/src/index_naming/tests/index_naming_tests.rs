use super::*;

// ─────────────────────────────────────────────────────────────────────────────
// The port, checked against names Postgres actually produced
// ─────────────────────────────────────────────────────────────────────────────

/// The one collision the schema already carries. These two strings were read
/// off a live database before being asserted here; they are the fixture the
/// whole guard rests on.
const DURATION_TABLE: &str = "meteora_damm_v2_update_reward_duration_events";
const FUNDER_TABLE: &str = "meteora_damm_v2_update_reward_funder_events";
const DURATION_INDEX: &str = "meteora_damm_v2_update_reward_signature_event_index_timesta_idx";
const FUNDER_INDEX: &str = "meteora_damm_v2_update_reward_signature_event_index_timest_idx1";

fn key_columns() -> Vec<String> {
    ["signature", "event_index", "timestamp"]
        .iter()
        .map(|c| c.to_string())
        .collect()
}

fn decl(table: &str, columns: &[&str]) -> IndexDecl {
    IndexDecl {
        file: "synthetic.sql".to_string(),
        line: 1,
        explicit_name: None,
        table: table.to_string(),
        columns: columns.iter().map(|c| c.to_string()).collect(),
    }
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
    let indexes = vec![
        decl(DURATION_TABLE, &["signature", "event_index", "timestamp"]),
        decl(FUNDER_TABLE, &["signature", "event_index", "timestamp"]),
    ];

    let found = collisions(&indexes);

    assert_eq!(found.len(), 1, "got {found:#?}");
    assert_eq!(found[0].table, FUNDER_TABLE);
    assert_eq!(found[0].generated, FUNDER_INDEX);
}

/// The mutation that must make the guard red: the same pair minus one member
/// collides with nothing.
#[test]
fn collisions_should_be_empty_without_the_second_member() {
    let indexes = vec![decl(
        DURATION_TABLE,
        &["signature", "event_index", "timestamp"],
    )];

    assert!(collisions(&indexes).is_empty());
}

/// A brand-new table whose name survives truncation onto an existing one is
/// exactly the scenario this guard exists for: adding it must be caught.
#[test]
fn collisions_should_catch_a_third_table_added_to_the_colliding_family() {
    let indexes = vec![
        decl(DURATION_TABLE, &["signature", "event_index", "timestamp"]),
        decl(FUNDER_TABLE, &["signature", "event_index", "timestamp"]),
        decl(
            "meteora_damm_v2_update_reward_authority_events",
            &["signature", "event_index", "timestamp"],
        ),
    ];

    let found = collisions(&indexes);

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
    let columns = ["signature", "event_index", "timestamp"];

    let as_declared = collisions(&[decl(DURATION_TABLE, &columns), decl(FUNDER_TABLE, &columns)]);
    let swapped = collisions(&[decl(FUNDER_TABLE, &columns), decl(DURATION_TABLE, &columns)]);

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
    let indexes = vec![explicit, decl("pools", &["protocol"])];

    let found = collisions(&indexes);

    assert_eq!(found.len(), 1);
    assert_eq!(found[0].generated, "pools_protocol_idx1");
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

    assert_eq!(scanned.indexes.len(), 1);
    assert_eq!(scanned.indexes[0].columns, key_columns());
    assert_eq!(scanned.indexes[0].explicit_name, None);
}

#[test]
fn scan_should_ignore_sort_order_and_opclass_on_a_column() {
    let scanned = scan_sql(
        "synthetic.sql",
        "CREATE INDEX ON t (pool_address, timestamp DESC NULLS LAST);",
    )
    .expect("modifiers parse");

    assert_eq!(
        scanned.indexes[0].columns,
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

    assert_eq!(
        scanned.indexes[0].explicit_name.as_deref(),
        Some("idx_pools_needs_refresh")
    );
    assert_eq!(
        scanned.indexes[0].columns,
        vec!["needs_refresh".to_string()]
    );
}

#[test]
fn scan_should_skip_comments_and_dollar_quoted_bodies() {
    let sql = "-- CREATE INDEX ON commented_out (a);\n\
               CREATE FUNCTION f() RETURNS INTERVAL LANGUAGE SQL AS $$ SELECT INTERVAL '1 h'; $$;\n\
               CREATE INDEX ON real_table (a);";

    let scanned = scan_sql("synthetic.sql", sql).expect("parses");

    assert_eq!(scanned.indexes.len(), 1);
    assert_eq!(scanned.indexes[0].table, "real_table");
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
fn scan_should_refuse_a_drop_index() {
    assert!(scan_err("DROP INDEX some_index;").contains("DROP INDEX is not modelled"));
}

#[test]
fn scan_should_refuse_a_schema_qualified_table() {
    assert!(scan_err("CREATE INDEX ON public.t (a);").contains("unsupported table reference"));
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

    let found = collisions(&scanned.indexes);

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
#[test]
fn the_baseline_carries_forty_nine_auto_named_indexes_thirty_five_of_them_truncated() {
    let baseline = migrations_dir().join("001_baseline.sql");
    let scanned = scan_file(&baseline).expect("the baseline must be parseable");

    let auto_named: Vec<&IndexDecl> = scanned
        .indexes
        .iter()
        .filter(|decl| decl.explicit_name.is_none())
        .collect();
    let truncated = auto_named
        .iter()
        .filter(|decl| {
            let addition = choose_index_name_addition(&decl.columns);
            decl.table.len() + addition.len() + "__idx".len() > MAX_IDENTIFIER_LEN
        })
        .count();

    assert_eq!(auto_named.len(), 49, "auto-named indexes in the baseline");
    assert_eq!(truncated, 35, "of which truncated");
}

/// Closes the blind spot left by not parsing `CREATE TABLE` bodies: a
/// constraint-borne index is named `<table>_pkey`, which only truncates once the
/// table name passes this length. The longest today is 53.
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

    for decl in &scanned.indexes {
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
