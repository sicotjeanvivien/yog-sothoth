//! Reading the migration files into the [`Event`]s that bear on index naming.
//!
//! Anything index-shaped this cannot decompose is a **hard error**, never a
//! skip: a guard that quietly covers 48 declarations out of 49 is worse than no
//! guard, because it reports green. The count cross-checks at the end of
//! [`scan_sql`] catch a statement that failed to *look* index-shaped at all.

use std::fs;
use std::path::{Path, PathBuf};

use super::hypertable::{UNMODELLED_INDEX_HELPERS, parse_hypertable};
use super::lexer::{Statement, statements};
use super::parse::{
    create_table_name_position, parse_alter_index, parse_alter_table, parse_create_index,
    parse_create_table, parse_drop,
};
use super::{Event, Scanned};

// ─────────────────────────────────────────────────────────────────────────────
// Reading the DDL
// ─────────────────────────────────────────────────────────────────────────────

pub(super) fn migrations_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations")
}

/// Every `*.sql` under `migrations/`, in the order `sqlx` applies them.
///
/// Read from disk rather than `include_str!`d: a new migration must be covered
/// the moment it lands, without anyone remembering to add it to a list.
///
/// Ordered by the **numeric** version prefix, which is what `sqlx::Migrator`
/// uses. A plain lexicographic sort agrees with it only while every prefix has
/// the same width, and suffix assignment is order-dependent — so a file whose
/// name does not start with `<digits>_` is refused rather than guessed at.
fn migration_files() -> Vec<PathBuf> {
    let mut files: Vec<(u64, PathBuf)> = fs::read_dir(migrations_dir())
        .expect("migrations/ must be readable")
        .map(|entry| entry.expect("directory entry must be readable").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "sql"))
        .map(|path| {
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .expect("migration file name must be UTF-8")
                .to_string();
            let version = name
                .split('_')
                .next()
                .and_then(|prefix| prefix.parse::<u64>().ok())
                .unwrap_or_else(|| {
                    panic!(
                        "`{name}` does not start with a numeric version — sqlx orders migrations \
                         by that number and this guard has to replay the same order"
                    )
                });
            (version, path)
        })
        .collect();
    files.sort();
    assert!(!files.is_empty(), "no migration found — wrong directory?");
    files.into_iter().map(|(_, path)| path).collect()
}

/// The body of [`scan_file`], over SQL already in hand — this is the entry point
/// the synthetic tests use, so they exercise the same parser as the real files.
///
/// Anything index-shaped that this cannot decompose is an **error**, never a
/// skip: a guard that quietly covers 48 declarations out of 49 is worse than no
/// guard, because it reports green. The count cross-checks at the end are what
/// catch a statement that failed to *look* index-shaped at all.
pub(super) fn scan_sql(name: &str, sql: &str) -> Result<Scanned, String> {
    let mut scanned = Scanned::default();
    let mut seen_index_statements = 0usize;
    let mut seen_hypertable_statements = 0usize;

    for (line, text) in statements(sql).map_err(|e| format!("{name}: {e}"))? {
        let s = Statement::new(name, line, &text);
        let code = s.code();

        if code.contains("CREATE INDEX") || code.contains("CREATE UNIQUE INDEX") {
            seen_index_statements += 1;
        }
        if code.contains("CREATE_HYPERTABLE") {
            seen_hypertable_statements += 1;
        }
        if let Some(helper) = UNMODELLED_INDEX_HELPERS
            .iter()
            .find(|helper| code.contains(*helper))
        {
            return Err(format!(
                "{}: `{}` can create an index of its own and is not modelled — extend \
                 index_naming.rs.\n  {}",
                s.here(),
                helper.to_ascii_lowercase(),
                s.text
            ));
        }

        if s.kw(0) == "DROP" {
            scanned.events.extend(parse_drop(&s)?);
        } else if s.kw(0) == "ALTER" && s.kw(1) == "INDEX" {
            scanned.events.push(parse_alter_index(&s)?);
        } else if s.kw(0) == "ALTER" && s.kw(1) == "TABLE" {
            scanned.events.extend(parse_alter_table(&s)?);
        } else if code.contains("CREATE_HYPERTABLE") {
            scanned
                .events
                .push(Event::Hypertable(parse_hypertable(&s)?));
        } else if s.kw(0) == "CREATE" {
            if let Some(cursor) = create_table_name_position(&s.upper) {
                scanned.events.push(parse_create_table(&s, cursor)?);
            } else if let Some(decl) = parse_create_index(&s)? {
                scanned.events.push(Event::Index(decl));
            }
        }
    }

    // Completeness: every statement that *looks* index-shaped must have been
    // decomposed into one. A mismatch means the parser stopped seeing something
    // it used to see — the one failure this guard must never have.
    let written = scanned
        .events
        .iter()
        .filter(|event| matches!(event, Event::Index(_)))
        .count();
    if written != seen_index_statements {
        return Err(format!(
            "{name}: {seen_index_statements} statements mention CREATE INDEX but {written} were \
             decomposed — extend index_naming.rs rather than leaving one uncovered"
        ));
    }
    let hypertables = scanned
        .events
        .iter()
        .filter(|event| matches!(event, Event::Hypertable(_)))
        .count();
    if hypertables != seen_hypertable_statements {
        return Err(format!(
            "{name}: {seen_hypertable_statements} statements mention create_hypertable but \
             {hypertables} were decomposed — extend index_naming.rs"
        ));
    }

    Ok(scanned)
}

/// Read one migration file.
pub(super) fn scan_file(path: &Path) -> Result<Scanned, String> {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default()
        .to_string();
    let sql = fs::read_to_string(path).map_err(|e| format!("{name}: unreadable ({e})"))?;
    scan_sql(&name, &sql)
}

/// Scan every migration, in application order.
pub(super) fn scan_migrations() -> Result<Scanned, String> {
    let mut all = Scanned::default();
    for path in migration_files() {
        let scanned = scan_file(&path)?;
        all.events.extend(scanned.events);
    }
    Ok(all)
}
