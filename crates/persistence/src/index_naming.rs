//! Guard over the index names Postgres *generates* for our migration DDL.
//!
//! `CREATE INDEX ON t (a, b);` leaves the name to the server. Postgres builds it
//! with `makeObjectName()`, truncates it to 63 bytes, and — when that truncation
//! makes two names equal — appends `1`, `2`, … **in creation order**
//! (`ChooseRelationName()`). Two tables of ours already collide:
//!
//! ```text
//! meteora_damm_v2_update_reward_duration_events  → …_signature_event_index_timesta_idx
//! meteora_damm_v2_update_reward_funder_events    → …_signature_event_index_timest_idx1
//! ```
//!
//! The `1` sits on `funder` only because `duration` is declared first in the
//! baseline. Adding a third table that collides would move it silently, and the
//! deployed schema would drift from a freshly migrated one with no error raised.
//! This module replays the server's algorithm over the migration files so that
//! any *new* collision fails a test instead.
//!
//! It is `#[cfg(test)]`-only: nothing here ships in the crate.
//!
//! # Why replaying is the only honest way to do this
//!
//! `makeObjectName()` does not cut the first 63 characters. It shortens the
//! *table part* and the *column part* separately, trimming whichever is longer,
//! one character at a time, until the whole fits. The disambiguating suffix is
//! then applied to the **label** (`idx` → `idx1`), which costs one more
//! character — which is why the colliding pair above differs by one character on
//! the column side too (`…_timesta_idx` vs `…_timest_idx1`). Reasoning about
//! "the first 63 characters" predicts the wrong names and misses collisions.
//!
//! # What this guard does not cover
//!
//! - **Constraint-borne indexes** (`<table>_pkey`, `<table>_<cols>_key`). Parsing
//!   `CREATE TABLE` bodies would be a lot of parser surface for no current risk.
//!   Instead [`tests`] asserts that no table name is long enough for its `_pkey`
//!   to truncate, which closes the hole at the source.
//! - **Index-vs-table name collisions.** Postgres shares one relation namespace,
//!   so an index could in principle collide with a table or a view. Every
//!   generated index name here ends in `_idx`/`_idxN` and no relation of ours
//!   does, so only index-vs-index collisions are modelled.
//! - **TimescaleDB's materialization indexes**, which live in
//!   `_timescaledb_internal` and are named after `_materialized_hypertable_N`,
//!   not after our tables.
//! - **`DROP INDEX`**, which would free a name for reuse. There is none today and
//!   [`scan_file`] refuses to run rather than silently mismodel one.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

/// `NAMEDATALEN` from a stock Postgres build (`src/include/pg_config_manual.h`).
const NAMEDATALEN: usize = 64;

/// Longest identifier Postgres keeps; anything longer is truncated to it.
const MAX_IDENTIFIER_LEN: usize = NAMEDATALEN - 1;

/// An auto-named index declaration read out of a migration file.
#[derive(Debug, Clone, PartialEq, Eq)]
struct IndexDecl {
    file: String,
    line: usize,
    /// `None` for `CREATE INDEX ON …`, i.e. the ones the server names.
    explicit_name: Option<String>,
    table: String,
    columns: Vec<String>,
}

/// An index whose generated name needed a disambiguating suffix.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Collision {
    file: String,
    line: usize,
    table: String,
    columns: Vec<String>,
    /// The name the server settles on, suffix included.
    generated: String,
}

/// What one migration file contributes.
#[derive(Debug, Default)]
struct Scanned {
    indexes: Vec<IndexDecl>,
    tables: Vec<String>,
}

// ─────────────────────────────────────────────────────────────────────────────
// The port. Faithful to src/backend/commands/indexcmds.c and
// src/backend/catalog/indexing.c — keep it that way; do not "simplify".
// ─────────────────────────────────────────────────────────────────────────────

/// Port of `makeObjectName()`.
///
/// `label` is never NULL for us (index names always carry `idx`/`pkey`/`key`),
/// so it is taken as `&str` rather than an `Option`.
///
/// Byte slicing stands in for `pg_mbcliplen()`: [`scan_file`] only ever accepts
/// identifiers matching `[a-z0-9_]+`, so every name here is ASCII.
fn make_object_name(name1: &str, name2: Option<&str>, label: &str) -> String {
    let mut overhead = label.len() + 1;
    let mut n1 = name1.len();
    let mut n2 = match name2 {
        Some(part) => {
            overhead += 1; // the separating underscore
            part.len()
        }
        None => 0,
    };

    let availchars = MAX_IDENTIFIER_LEN
        .checked_sub(overhead)
        .filter(|avail| *avail > 0)
        .expect("label leaves no room for a name — Postgres asserts this too");

    // Preferentially truncate the longer of the two parts, exactly as the loop
    // in makeObjectName does.
    while n1 + n2 > availchars {
        if n1 > n2 {
            n1 -= 1;
        } else {
            n2 -= 1;
        }
    }

    let mut out = String::with_capacity(MAX_IDENTIFIER_LEN);
    out.push_str(&name1[..n1]);
    if let Some(part) = name2 {
        out.push('_');
        out.push_str(&part[..n2]);
    }
    out.push('_');
    out.push_str(label);
    out
}

/// Port of `ChooseIndexNameAddition()`: the column names joined by `_`, stopped
/// once the buffer reaches `NAMEDATALEN`.
fn choose_index_name_addition(columns: &[String]) -> String {
    let mut buf = String::new();
    for column in columns {
        if !buf.is_empty() {
            buf.push('_');
        }
        // strlcpy(…, NAMEDATALEN) copies at most NAMEDATALEN - 1 bytes.
        let take = column.len().min(NAMEDATALEN - 1);
        buf.push_str(&column[..take]);
        if buf.len() >= NAMEDATALEN {
            break;
        }
    }
    buf
}

/// Port of `ChooseRelationName()`. Returns the chosen name and the number of
/// disambiguating passes it took — `0` means the natural name was free.
fn choose_relation_name(
    name1: &str,
    name2: Option<&str>,
    label: &str,
    taken: &HashSet<String>,
) -> (String, u32) {
    let mut pass = 0u32;
    let mut modlabel = label.to_string();
    loop {
        let candidate = make_object_name(name1, name2, &modlabel);
        if !taken.contains(&candidate) {
            return (candidate, pass);
        }
        pass += 1;
        modlabel = format!("{label}{pass}");
    }
}

/// The name Postgres gives an index declared as `CREATE INDEX ON table (cols)`,
/// given the index names already taken in the namespace.
fn generated_index_name(decl: &IndexDecl, taken: &HashSet<String>) -> (String, u32) {
    let addition = choose_index_name_addition(&decl.columns);
    choose_relation_name(&decl.table, Some(&addition), "idx", taken)
}

// ─────────────────────────────────────────────────────────────────────────────
// Reading the DDL
// ─────────────────────────────────────────────────────────────────────────────

fn migrations_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations")
}

/// Every `*.sql` under `migrations/`, in the order `sqlx` applies them.
///
/// Read from disk rather than `include_str!`d: a new migration must be covered
/// the moment it lands, without anyone remembering to add it to a list.
fn migration_files() -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = fs::read_dir(migrations_dir())
        .expect("migrations/ must be readable")
        .map(|entry| entry.expect("directory entry must be readable").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "sql"))
        .collect();
    files.sort();
    assert!(!files.is_empty(), "no migration found — wrong directory?");
    files
}

/// Split SQL into statements, dropping line comments, block comments and
/// dollar-quoted bodies, and collapsing whitespace. Each statement comes with
/// the 1-based line it starts on.
fn statements(sql: &str) -> Vec<(usize, String)> {
    let chars: Vec<char> = sql.chars().collect();
    let mut out = Vec::new();
    let mut current = String::new();
    let mut start_line = 0usize;
    let mut line = 1usize;
    let mut i = 0usize;

    let flush = |current: &mut String, start_line: usize, out: &mut Vec<(usize, String)>| {
        let collapsed = current.split_whitespace().collect::<Vec<_>>().join(" ");
        if !collapsed.is_empty() {
            out.push((start_line, collapsed));
        }
        current.clear();
    };

    while i < chars.len() {
        let c = chars[i];
        let next = chars.get(i + 1).copied();

        // -- line comment
        if c == '-' && next == Some('-') {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
            continue;
        }
        // /* block comment */ — Postgres nests these; we do not need to.
        if c == '/' && next == Some('*') {
            i += 2;
            while i < chars.len() && !(chars[i] == '*' && chars.get(i + 1) == Some(&'/')) {
                if chars[i] == '\n' {
                    line += 1;
                }
                i += 1;
            }
            i += 2;
            continue;
        }
        // 'string literal' — kept, it may hold a semicolon.
        if c == '\'' {
            if current.trim().is_empty() {
                start_line = line;
            }
            current.push(c);
            i += 1;
            while i < chars.len() {
                if chars[i] == '\n' {
                    line += 1;
                }
                current.push(chars[i]);
                if chars[i] == '\'' {
                    // '' is an escaped quote, not the end.
                    if chars.get(i + 1) == Some(&'\'') {
                        current.push('\'');
                        i += 2;
                        continue;
                    }
                    i += 1;
                    break;
                }
                i += 1;
            }
            continue;
        }
        // $tag$ … $tag$ body — skipped whole.
        if c == '$'
            && let Some(tag) = dollar_tag(&chars, i)
        {
            i += tag.len();
            while i < chars.len() && !starts_with_at(&chars, i, &tag) {
                if chars[i] == '\n' {
                    line += 1;
                }
                i += 1;
            }
            i += tag.len();
            continue;
        }
        if c == ';' {
            flush(&mut current, start_line, &mut out);
            i += 1;
            continue;
        }
        if c == '\n' {
            line += 1;
        }
        if !c.is_whitespace() && current.trim().is_empty() {
            start_line = line;
        }
        current.push(c);
        i += 1;
    }
    flush(&mut current, start_line, &mut out);
    out
}

/// `$$` or `$tag$` starting at `i`, if there is one.
fn dollar_tag(chars: &[char], i: usize) -> Option<String> {
    let mut j = i + 1;
    while j < chars.len() && (chars[j].is_ascii_alphanumeric() || chars[j] == '_') {
        j += 1;
    }
    (chars.get(j) == Some(&'$')).then(|| chars[i..=j].iter().collect())
}

fn starts_with_at(chars: &[char], i: usize, needle: &str) -> bool {
    needle
        .chars()
        .enumerate()
        .all(|(offset, c)| chars.get(i + offset) == Some(&c))
}

/// Split a statement into tokens, `(`, `)` and `,` standing alone.
fn tokens(statement: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut word = String::new();
    for c in statement.chars() {
        if c.is_whitespace() || c == '(' || c == ')' || c == ',' {
            if !word.is_empty() {
                out.push(std::mem::take(&mut word));
            }
            if !c.is_whitespace() {
                out.push(c.to_string());
            }
        } else {
            word.push(c);
        }
    }
    if !word.is_empty() {
        out.push(word);
    }
    out
}

fn is_plain_identifier(token: &str) -> bool {
    !token.is_empty()
        && token
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        && !token.starts_with(|c: char| c.is_ascii_digit())
}

/// Read one migration file.
fn scan_file(path: &Path) -> Result<Scanned, String> {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default()
        .to_string();
    let sql = fs::read_to_string(path).map_err(|e| format!("{name}: unreadable ({e})"))?;
    scan_sql(&name, &sql)
}

/// The body of [`scan_file`], over SQL already in hand — this is the entry point
/// the synthetic tests use, so they exercise the same parser as the real files.
///
/// Anything index-shaped that this cannot decompose is an **error**, never a
/// skip: a guard that quietly covers 48 declarations out of 49 is worse than no
/// guard, because it reports green.
fn scan_sql(name: &str, sql: &str) -> Result<Scanned, String> {
    let mut scanned = Scanned::default();

    for (line, statement) in statements(sql) {
        let t = tokens(&statement);
        let upper: Vec<String> = t.iter().map(|token| token.to_uppercase()).collect();
        let kw = |index: usize| upper.get(index).map(String::as_str).unwrap_or_default();
        let at = |index: usize| t.get(index).map(String::as_str).unwrap_or_default();

        if kw(0) == "DROP" && kw(1) == "INDEX" {
            return Err(format!(
                "{name}:{line}: DROP INDEX is not modelled by this guard — it frees a name for \
                 reuse, which changes every suffix downstream. Extend index_naming.rs before \
                 landing this migration.\n  {statement}"
            ));
        }
        if kw(0) == "CREATE" && kw(1) == "TABLE" {
            let mut cursor = 2;
            if kw(2) == "IF" && kw(3) == "NOT" && kw(4) == "EXISTS" {
                cursor = 5;
            }
            let table = at(cursor);
            if !is_plain_identifier(table) {
                return Err(format!("{name}:{line}: unsupported table name `{table}`"));
            }
            scanned.tables.push(table.to_string());
            continue;
        }
        if kw(0) != "CREATE" {
            continue;
        }
        let mut cursor = 1;
        if kw(cursor) == "UNIQUE" {
            cursor += 1;
        }
        if kw(cursor) != "INDEX" {
            continue;
        }
        cursor += 1;
        if kw(cursor) == "CONCURRENTLY" {
            cursor += 1; // does not affect naming
        }
        if kw(cursor) == "IF" && kw(cursor + 1) == "NOT" && kw(cursor + 2) == "EXISTS" {
            cursor += 3;
        }

        let explicit_name = if kw(cursor) == "ON" {
            None
        } else {
            let candidate = at(cursor);
            if !is_plain_identifier(candidate) {
                return Err(format!(
                    "{name}:{line}: unsupported index name `{candidate}`\n  {statement}"
                ));
            }
            cursor += 1;
            Some(candidate.to_string())
        };

        if kw(cursor) != "ON" {
            return Err(format!(
                "{name}:{line}: expected ON after CREATE INDEX — extend index_naming.rs\n  \
                 {statement}"
            ));
        }
        cursor += 1;
        let table = at(cursor);
        if !is_plain_identifier(table) {
            return Err(format!(
                "{name}:{line}: unsupported table reference `{table}` (schema-qualified?)\n  \
                 {statement}"
            ));
        }
        cursor += 1;
        if kw(cursor) == "USING" {
            cursor += 2; // access method does not affect naming
        }
        if kw(cursor) != "(" {
            return Err(format!(
                "{name}:{line}: expected a column list — extend index_naming.rs\n  {statement}"
            ));
        }
        cursor += 1;

        // Column list: take the leading identifier of each item, refuse anything
        // else. Postgres names an expression element `expr`, and INCLUDE changes
        // what goes into the addition; neither is modelled here on purpose.
        let mut columns = Vec::new();
        let mut expect_column = true;
        loop {
            match kw(cursor) {
                ")" => break,
                "(" => {
                    return Err(format!(
                        "{name}:{line}: unsupported index element — a parenthesised expression is \
                         named `expr` by Postgres, not after its columns. Extend \
                         index_naming.rs.\n  {statement}"
                    ));
                }
                "," => {
                    expect_column = true;
                    cursor += 1;
                }
                "" => {
                    return Err(format!(
                        "{name}:{line}: unterminated column list\n  {statement}"
                    ));
                }
                _ => {
                    if expect_column {
                        let column = at(cursor);
                        if !is_plain_identifier(column) {
                            return Err(format!(
                                "{name}:{line}: unsupported index element `{column}` (expression \
                                 or quoted identifier?) — Postgres names it differently. Extend \
                                 index_naming.rs.\n  {statement}"
                            ));
                        }
                        columns.push(column.to_string());
                        expect_column = false;
                    }
                    // Trailing DESC / NULLS LAST / opclass do not affect naming.
                    cursor += 1;
                }
            }
        }
        cursor += 1;
        if kw(cursor) == "INCLUDE" {
            return Err(format!(
                "{name}:{line}: INCLUDE is not modelled by this guard — extend \
                 index_naming.rs.\n  {statement}"
            ));
        }
        if columns.is_empty() {
            return Err(format!("{name}:{line}: empty column list\n  {statement}"));
        }

        scanned.indexes.push(IndexDecl {
            file: name.to_string(),
            line,
            explicit_name,
            table: table.to_string(),
            columns,
        });
    }
    Ok(scanned)
}

/// Scan every migration, in application order.
fn scan_migrations() -> Result<Scanned, String> {
    let mut all = Scanned::default();
    for path in migration_files() {
        let scanned = scan_file(&path)?;
        all.indexes.extend(scanned.indexes);
        all.tables.extend(scanned.tables);
    }
    Ok(all)
}

/// Replay Postgres' naming over `indexes`, in declaration order, and return the
/// ones that needed a disambiguating suffix.
fn collisions(indexes: &[IndexDecl]) -> Vec<Collision> {
    let mut taken: HashSet<String> = HashSet::new();
    let mut found = Vec::new();
    for decl in indexes {
        match &decl.explicit_name {
            // An explicit name is reserved as written; Postgres does not
            // disambiguate it, it errors on a duplicate.
            Some(name) => {
                taken.insert(name.clone());
            }
            None => {
                let (name, passes) = generated_index_name(decl, &taken);
                if passes > 0 {
                    found.push(Collision {
                        file: decl.file.clone(),
                        line: decl.line,
                        table: decl.table.clone(),
                        columns: decl.columns.clone(),
                        generated: name.clone(),
                    });
                }
                taken.insert(name);
            }
        }
    }
    found
}

#[cfg(test)]
#[path = "index_naming/tests/index_naming_tests.rs"]
mod tests;
