use super::*;

#[test]
fn new_migrations_name_their_indexes() {
    let mut failures = Vec::new();
    let mut guarded = Vec::new();
    for (version, path) in migration_files() {
        if version < FIRST_GUARDED_VERSION {
            continue;
        }
        let name = file_name(&path);
        guarded.push(name.clone());
        let sql = fs::read_to_string(&path).expect("migration must be readable");
        failures.extend(
            violations(&sql)
                .into_iter()
                .map(|why| format!("{name}:{why}")),
        );
    }
    assert!(
        failures.is_empty(),
        "migrations/README.md → “Name every index”:\n\n{}\n",
        failures.join("\n\n")
    );
    // Not an assertion about coverage — a note, so a green run never reads
    // as "the rule was checked" when there is nothing yet to check.
    println!("checked {} migration(s): {guarded:?}", guarded.len());
}

#[test]
fn an_auto_named_index_is_refused() {
    let why = violations("CREATE UNIQUE INDEX ON t (signature, event_index, timestamp);");

    assert_eq!(why.len(), 1, "{why:?}");
    assert!(why[0].contains("Name it"), "{}", why[0]);
}

#[test]
fn a_named_index_is_accepted() {
    assert!(violations("CREATE UNIQUE INDEX t_sig_uniq ON t (signature, timestamp);").is_empty());
}

#[test]
fn an_auto_named_index_is_refused_however_it_is_spelled() {
    for sql in [
        "CREATE INDEX ON t (a);",
        "CREATE INDEX CONCURRENTLY ON t (a);",
        "CREATE UNIQUE INDEX IF NOT EXISTS ON t (a);",
        "CREATE INDEX\n    ON t (a)\n    WHERE b;",
        "CREATE INDEX/**/ON t (a);",
    ] {
        assert_eq!(violations(sql).len(), 1, "not caught: {sql}");
    }
}

#[test]
fn an_over_long_index_name_is_refused() {
    let sql = format!("CREATE INDEX {} ON t (a);", "z".repeat(70));

    assert!(violations(&sql)[0].contains("truncates it to 63"));
}

#[test]
fn a_table_name_too_long_for_its_pkey_is_refused() {
    let sql = format!("CREATE TABLE {} (a TEXT);", "z".repeat(60));

    assert!(violations(&sql)[0].contains("`_pkey` index"));
}

#[test]
fn a_hypertable_must_disable_its_default_indexes() {
    let sql = "SELECT create_hypertable('t', 'ts', chunk_time_interval => INTERVAL '7 days');";

    assert!(violations(sql)[0].contains("create_default_indexes"));
}

#[test]
fn a_hypertable_that_disables_them_is_accepted() {
    let sql = "SELECT create_hypertable('t', 'ts', create_default_indexes => FALSE);";

    assert!(violations(sql).is_empty());
}

/// Comments and literals are prose, not code: a migration that *mentions*
/// the rule must not trip it.
#[test]
fn prose_mentioning_an_index_is_not_a_violation() {
    for sql in [
        "-- CREATE INDEX ON t (a);\nCREATE INDEX t_a ON t (a);",
        "/* CREATE INDEX ON t (a); */",
        "COMMENT ON TABLE t IS 'never write CREATE INDEX ON t (a)';",
        "CREATE FUNCTION f() RETURNS INT LANGUAGE SQL AS $$ SELECT 1; $$;",
    ] {
        assert!(violations(sql).is_empty(), "false positive on: {sql}");
    }
}

/// The frozen migrations break the rule 70 times over — which is why the
/// guard starts at 010, and why that boundary is worth asserting.
#[test]
fn the_frozen_migrations_are_out_of_scope() {
    let baseline = migrations_dir().join("001_baseline.sql");
    let sql = fs::read_to_string(baseline).expect("readable");

    assert_eq!(
        violations(&sql).len(),
        70,
        "49 written CREATE INDEX lines plus 21 create_hypertable calls"
    );
}
