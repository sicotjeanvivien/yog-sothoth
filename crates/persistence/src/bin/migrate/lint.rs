//! The rules `yog-migrate` imposes on the migrations it applies, checked as
//! ordinary DB-free unit tests of this binary.
//!
//! They live here rather than in the crate's library because `migrations/` is
//! this binary's subject: `yog-persistence` binds the processes to the database,
//! `yog-migrate` owns how the schema changes. They stay *tests* rather than a
//! runtime check because a migration that breaks a naming convention is badly
//! written, not dangerous to apply — the proportionate sanction is a red pull
//! request, not a blocked deployment.
//!
//! # Index names have to be written down
//!
//! `CREATE INDEX ON t (a, b);` leaves the name to the server, and the server has
//! only 63 bytes for it. Our table names run to 53 characters, so the generated
//! name is truncated — and when two truncate onto the same name, Postgres
//! appends `1`, `2`, … **in creation order**. Two tables already do:
//!
//! ```text
//! meteora_damm_v2_update_reward_duration_events  → …_signature_event_index_timesta_idx
//! meteora_damm_v2_update_reward_funder_events    → …_signature_event_index_timest_idx1
//! ```
//!
//! The `1` sits on `funder` only because `duration` is declared first. Add a
//! third table that truncates onto the same name and the suffix moves, so a
//! freshly migrated database no longer matches production — with no error
//! raised, because nothing here is illegal.
//!
//! The fix is not to predict what Postgres would name things. It is to stop
//! asking it: **name every index**. Then there is nothing to predict, and this
//! module only has to check that the rule was followed.
//!
//! `create_hypertable()` names one too — a default index on the time dimension,
//! on the root table, in `public` — so a new hypertable passes
//! `create_default_indexes => FALSE` and writes that index out like any other.
//!
//! # Scope
//!
//! Only migrations from [`FIRST_GUARDED_VERSION`] on. `001`–`009` are frozen
//! history (forward-only); the 70 auto-named indexes they created keep their
//! names, and renaming them is a separate job that needs its own migration.

use std::fs;
use std::path::{Path, PathBuf};

/// Longest identifier Postgres keeps; anything longer is silently truncated,
/// which is how names start colliding in the first place.
const MAX_IDENTIFIER_LEN: usize = 63;

/// Beyond this length a table's `<table>_pkey` truncates too, and that one
/// cannot be named explicitly.
const MAX_TABLE_NAME_LEN: usize = MAX_IDENTIFIER_LEN - "_pkey".len();

/// The rule binds migrations written from here on. Everything before is frozen.
const FIRST_GUARDED_VERSION: u64 = 10;

fn migrations_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations")
}

/// Every `*.sql` under `migrations/`, with the version its file name declares.
fn migration_files() -> Vec<(u64, PathBuf)> {
    let mut files: Vec<(u64, PathBuf)> = fs::read_dir(migrations_dir())
        .expect("migrations/ must be readable")
        .map(|entry| entry.expect("directory entry must be readable").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "sql"))
        .map(|path| {
            let name = file_name(&path);
            let version = name
                .split('_')
                .next()
                .and_then(|prefix| prefix.parse::<u64>().ok())
                .unwrap_or_else(|| panic!("`{name}` does not start with a numeric version"));
            (version, path)
        })
        .collect();
    files.sort();
    assert!(!files.is_empty(), "no migration found — wrong directory?");
    files
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .expect("migration file name must be UTF-8")
        .to_string()
}

/// The SQL statements of `sql`, each with the 1-based line it starts on,
/// whitespace collapsed.
///
/// Comments and quoted text become a space rather than disappearing: a comment
/// mentioning `CREATE INDEX` must not be read as one, and `INDEX/**/ON` must not
/// weld into a token that no longer looks like an index at all.
fn statements(sql: &str) -> Vec<(usize, String)> {
    let chars: Vec<char> = sql.chars().collect();
    let (mut out, mut current) = (Vec::new(), String::new());
    let (mut line, mut start_line, mut i) = (1usize, 1usize, 0usize);

    let flush = |current: &mut String, start_line: usize, out: &mut Vec<(usize, String)>| {
        let collapsed = current.split_whitespace().collect::<Vec<_>>().join(" ");
        if !collapsed.is_empty() {
            out.push((start_line, collapsed));
        }
        current.clear();
    };

    while i < chars.len() {
        let (c, next) = (chars[i], chars.get(i + 1).copied());
        let skip_to = |i: &mut usize, line: &mut usize, close: &dyn Fn(usize) -> bool| {
            while *i < chars.len() && !close(*i) {
                if chars[*i] == '\n' {
                    *line += 1;
                }
                *i += 1;
            }
        };
        match (c, next) {
            ('-', Some('-')) => {
                skip_to(&mut i, &mut line, &|i| chars[i] == '\n');
                current.push(' ');
            }
            ('/', Some('*')) => {
                i += 2;
                skip_to(&mut i, &mut line, &|i| {
                    chars[i] == '*' && chars.get(i + 1) == Some(&'/')
                });
                i += 2;
                current.push(' ');
            }
            ('\'', _) | ('"', _) | ('$', Some('$')) => {
                let delim = if c == '$' { "$$" } else { &c.to_string() };
                i += delim.len();
                skip_to(&mut i, &mut line, &|i| {
                    chars[i..].starts_with(&delim.chars().collect::<Vec<_>>()[..])
                });
                i += delim.len();
                current.push(' ');
            }
            (';', _) => {
                flush(&mut current, start_line, &mut out);
                i += 1;
            }
            _ => {
                if c == '\n' {
                    line += 1;
                }
                if !c.is_whitespace() && current.trim().is_empty() {
                    start_line = line;
                }
                current.push(c);
                i += 1;
            }
        }
    }
    flush(&mut current, start_line, &mut out);
    out
}

/// What a statement gets us told off for, if anything.
fn violation(statement: &str) -> Option<String> {
    let words: Vec<String> = statement
        .split_whitespace()
        .map(|w| w.to_uppercase())
        .collect();
    let word = |index: usize| words.get(index).map(String::as_str).unwrap_or_default();

    if word(0) == "CREATE" {
        let mut cursor = 1;
        if word(cursor) == "UNIQUE" {
            cursor += 1;
        }
        if word(cursor) == "INDEX" {
            cursor += 1;
            if word(cursor) == "CONCURRENTLY" {
                cursor += 1;
            }
            if word(cursor) == "IF" {
                cursor += 3; // IF NOT EXISTS
            }
            if word(cursor) == "ON" {
                return Some(
                    "this index is left for the server to name, and the generated name is \
                     truncated to 63 bytes — two of ours already truncate onto the same name. \
                     Name it."
                        .to_string(),
                );
            }
            let name = statement.split_whitespace().nth(cursor).unwrap_or_default();
            if name.len() > MAX_IDENTIFIER_LEN {
                return Some(format!(
                    "`{name}` is {} characters; Postgres truncates it to {MAX_IDENTIFIER_LEN}, \
                     which is how names start colliding",
                    name.len()
                ));
            }
        }
        if word(cursor) == "TABLE" {
            let table = statement
                .split_whitespace()
                .nth(cursor + 1)
                .unwrap_or_default()
                .trim_end_matches('(');
            if table.len() > MAX_TABLE_NAME_LEN {
                return Some(format!(
                    "`{table}` is {} characters; beyond {MAX_TABLE_NAME_LEN} its `_pkey` index \
                     name truncates, and that one cannot be named explicitly",
                    table.len()
                ));
            }
        }
    }

    let upper = statement.to_uppercase();
    if upper.contains("CREATE_HYPERTABLE") && !upper.contains("CREATE_DEFAULT_INDEXES => FALSE") {
        return Some(
            "create_hypertable builds a default index on the time dimension and names it itself. \
             Pass `create_default_indexes => FALSE` and write that index out."
                .to_string(),
        );
    }
    None
}

/// Every rule broken by `sql`, as `line: explanation`.
fn violations(sql: &str) -> Vec<String> {
    statements(sql)
        .into_iter()
        .filter_map(|(line, statement)| {
            violation(&statement).map(|why| format!("{line}: {why}\n    {statement}"))
        })
        .collect()
}

#[cfg(test)]
#[path = "tests/lint_tests.rs"]
mod tests;
