//! The assertions that run over the real migration files.

use super::*;

/// The collisions the schema is *known* to carry. Adding a table whose index
/// name truncates onto an existing one moves the `1` from one table to another
/// and drifts the deployed schema from a freshly migrated one, silently.
///
/// If this list has to grow, that is a decision to take deliberately — name the
/// new index explicitly instead (see `migrations/README.md`).
const PINNED_COLLISIONS: &[(&str, &str)] = &[(FUNDER_TABLE, FUNDER_INDEX)];

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
    for table in &replay_migrations().tables {
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
    for table in &replay_migrations().tables {
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
