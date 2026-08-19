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
//! # Every name in the namespace has to be modelled, not just ours
//!
//! A name we forget to enter is a name the replay thinks is free, and the guard
//! then reports green on a real collision. `create_hypertable()` is the trap
//! here: unless it is passed `create_default_indexes => FALSE`, TimescaleDB
//! creates a **default index on the time dimension, on the root table, in
//! `public`**, letting Postgres name it through this very algorithm — and it
//! does so *before* the `CREATE INDEX` lines that follow it in the file. The 21
//! calls in `001_baseline.sql` add 21 such names, one of which is already
//! truncated to 63 bytes
//! (`meteora_damm_v2_withdraw_dead_liquidity_reward_ev_timestamp_idx`).
//! [`scan_sql`] therefore models `create_hypertable` too.
//!
//! # What this guard still does not cover
//!
//! - **Constraint-borne indexes.** `<table>_pkey` is covered indirectly: the
//!   tests assert no table name is long enough for it to truncate. An *unnamed*
//!   `UNIQUE (…)` in a `CREATE TABLE` body would produce `<table>_<cols>_key`,
//!   which truncates and collides exactly like `_idx`; rather than parse table
//!   bodies, [`scan_sql`] refuses a `CREATE TABLE` carrying an unnamed `UNIQUE`.
//! - **Index-vs-table name collisions.** Postgres shares one relation namespace,
//!   so an index could in principle collide with a table or a view. Every
//!   generated index name here ends in `_idx`/`_idxN` and no relation of ours
//!   does, so only index-vs-index collisions are modelled.
//! - **TimescaleDB's per-chunk and materialization indexes**, which live in
//!   `_timescaledb_internal` and are named after `_hyper_N_M_chunk` /
//!   `_materialized_hypertable_N`, not after our tables.
//!
//! Anything else index-shaped that the scanner cannot decompose is a **hard
//! error**, never a skip — including a statement that would *free* a name
//! (`DROP INDEX`, `DROP TABLE`, `ALTER TABLE … DROP COLUMN`), because a freed
//! name shifts every suffix downstream of it.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

/// `NAMEDATALEN` from a stock Postgres build (`src/include/pg_config_manual.h`).
const NAMEDATALEN: usize = 64;

/// Longest identifier Postgres keeps; anything longer is truncated to it.
const MAX_IDENTIFIER_LEN: usize = NAMEDATALEN - 1;

/// Where an index declaration came from — both are named by the server.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Origin {
    /// A `CREATE INDEX` line in a migration.
    Written,
    /// The default time-dimension index `create_hypertable()` creates.
    HypertableDefault,
}

/// An index declaration read out of a migration file.
#[derive(Debug, Clone, PartialEq, Eq)]
struct IndexDecl {
    file: String,
    line: usize,
    /// `None` when the server picks the name, which is the case this guard is about.
    explicit_name: Option<String>,
    table: String,
    columns: Vec<String>,
    origin: Origin,
}

/// A statement that removes an index, or something an index hangs off.
#[derive(Debug, Clone, PartialEq, Eq)]
struct DropStmt {
    file: String,
    line: usize,
    /// Relation the drop targets; `None` for `DROP INDEX`, which names the index.
    table: Option<String>,
    /// Index name for `DROP INDEX`.
    index: Option<String>,
    /// Column for `ALTER TABLE … DROP COLUMN`; `None` drops the whole relation.
    column: Option<String>,
    statement: String,
}

/// DDL that bears on index naming, in file order.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Event {
    Index(IndexDecl),
    Drop(DropStmt),
}

/// An index whose generated name needed a disambiguating suffix.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Collision {
    file: String,
    line: usize,
    table: String,
    columns: Vec<String>,
    origin: Origin,
    /// The name the server settles on, suffix included.
    generated: String,
}

/// What the migrations contribute, in application order.
#[derive(Debug, Default)]
struct Scanned {
    events: Vec<Event>,
    tables: Vec<String>,
}

impl Scanned {
    fn indexes(&self) -> impl Iterator<Item = &IndexDecl> {
        self.events.iter().filter_map(|event| match event {
            Event::Index(decl) => Some(decl),
            Event::Drop(_) => None,
        })
    }
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
/// Byte slicing stands in for `pg_mbcliplen()`: [`scan_sql`] only ever accepts
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
///
/// Postgres feeds this the output of `ChooseIndexColumnNames()`, which suffixes
/// *repeated* column names (`a`, `a1`, `a2`) before joining. That step is not
/// ported; [`scan_sql`] refuses an index that repeats a column instead.
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

/// The name Postgres gives an index it has to name itself, given the names
/// already taken in the namespace.
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

/// Split SQL into statements, replacing comments with a space, keeping quoted
/// text intact, and collapsing whitespace. Each statement comes with the 1-based
/// line it starts on.
///
/// Comments become a **space**, not nothing: `CREATE INDEX/**/ON t (a)` must not
/// weld into `INDEXON` and slip past the parser as an unrecognised statement.
fn statements(sql: &str) -> Result<Vec<(usize, String)>, String> {
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
            current.push(' ');
            continue;
        }
        // /* block comment */ — Postgres nests these, so we do too.
        if c == '/' && next == Some('*') {
            let opened_at = line;
            let mut depth = 0usize;
            loop {
                if i >= chars.len() {
                    return Err(format!(
                        "unterminated block comment opened at line {opened_at}"
                    ));
                }
                if chars[i] == '/' && chars.get(i + 1) == Some(&'*') {
                    depth += 1;
                    i += 2;
                    continue;
                }
                if chars[i] == '*' && chars.get(i + 1) == Some(&'/') {
                    depth -= 1;
                    i += 2;
                    if depth == 0 {
                        break;
                    }
                    continue;
                }
                if chars[i] == '\n' {
                    line += 1;
                }
                i += 1;
            }
            current.push(' ');
            continue;
        }
        // E'…' / U&'…' carry backslash escapes we do not model; refuse rather
        // than mis-split on an escaped quote.
        if (c == 'E' || c == 'e') && next == Some('\'') {
            return Err(format!(
                "line {line}: escape string literals (E'…') are not modelled — extend \
                 index_naming.rs"
            ));
        }
        // 'string literal' and "quoted identifier" — kept, they may hold a
        // semicolon, and the doubled delimiter is an escape, not the end.
        if c == '\'' || c == '"' {
            let quote = c;
            if current.trim().is_empty() {
                start_line = line;
            }
            current.push(c);
            i += 1;
            loop {
                let Some(&ch) = chars.get(i) else {
                    return Err(format!("line {line}: unterminated {quote}-quoted text"));
                };
                if ch == '\n' {
                    line += 1;
                }
                current.push(ch);
                i += 1;
                if ch == quote {
                    if chars.get(i) == Some(&quote) {
                        current.push(quote);
                        i += 1;
                        continue;
                    }
                    break;
                }
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
            if i >= chars.len() {
                return Err(format!("line {line}: unterminated {tag} block"));
            }
            i += tag.len();
            current.push(' ');
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
    Ok(out)
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

/// Split a statement into tokens. `(`, `)`, `,` and `=>` stand alone; quoted
/// text stays whole.
fn tokens(statement: &str) -> Vec<String> {
    let chars: Vec<char> = statement.chars().collect();
    let mut out = Vec::new();
    let mut word = String::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '\'' || c == '"' {
            if !word.is_empty() {
                out.push(std::mem::take(&mut word));
            }
            let quote = c;
            let mut literal = String::from(c);
            i += 1;
            while i < chars.len() {
                literal.push(chars[i]);
                i += 1;
                if chars[i - 1] == quote {
                    if chars.get(i) == Some(&quote) {
                        literal.push(quote);
                        i += 1;
                        continue;
                    }
                    break;
                }
            }
            out.push(literal);
            continue;
        }
        if c == '=' && chars.get(i + 1) == Some(&'>') {
            if !word.is_empty() {
                out.push(std::mem::take(&mut word));
            }
            out.push("=>".to_string());
            i += 2;
            continue;
        }
        if c.is_whitespace() || c == '(' || c == ')' || c == ',' {
            if !word.is_empty() {
                out.push(std::mem::take(&mut word));
            }
            if !c.is_whitespace() {
                out.push(c.to_string());
            }
            i += 1;
            continue;
        }
        word.push(c);
        i += 1;
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

/// The content of a `'…'` literal, if the token is one.
fn string_literal(token: &str) -> Option<String> {
    let inner = token.strip_prefix('\'')?.strip_suffix('\'')?;
    Some(inner.replace("''", "'"))
}

/// `create_hypertable` arguments that provably do not change which default
/// indexes get created. Anything else is refused rather than assumed harmless.
const NAMING_NEUTRAL_HYPERTABLE_ARGS: &[&str] = &[
    "chunk_time_interval",
    "if_not_exists",
    "migrate_data",
    "associated_schema_name",
    "associated_table_prefix",
    "time_partitioning_func",
];

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
/// guard, because it reports green. The count cross-check at the end is what
/// catches a statement that failed to *look* index-shaped at all.
fn scan_sql(name: &str, sql: &str) -> Result<Scanned, String> {
    let mut scanned = Scanned::default();
    let statements = statements(sql).map_err(|e| format!("{name}: {e}"))?;
    let mut seen_index_statements = 0usize;
    let mut seen_hypertable_statements = 0usize;

    for (line, statement) in &statements {
        let (line, statement) = (*line, statement.as_str());
        let t = tokens(statement);
        let upper: Vec<String> = t.iter().map(|token| token.to_uppercase()).collect();
        let kw = |index: usize| upper.get(index).map(String::as_str).unwrap_or_default();
        let at = |index: usize| t.get(index).map(String::as_str).unwrap_or_default();
        let joined = upper.join(" ");
        let here = format!("{name}:{line}");

        if joined.contains("CREATE INDEX") || joined.contains("CREATE UNIQUE INDEX") {
            seen_index_statements += 1;
        }
        if joined.contains("CREATE_HYPERTABLE") {
            seen_hypertable_statements += 1;
        }

        // ── statements that free a name ──────────────────────────────────────
        if kw(0) == "DROP" {
            let mut cursor = 1;
            if kw(cursor) == "MATERIALIZED" {
                cursor += 1;
            }
            let kind = kw(cursor).to_string();
            cursor += 1;
            if kw(cursor) == "IF" && kw(cursor + 1) == "EXISTS" {
                cursor += 2;
            }
            let target = at(cursor).to_string();
            let (table, index) = match kind.as_str() {
                "INDEX" => (None, Some(target)),
                _ => (Some(target), None),
            };
            scanned.events.push(Event::Drop(DropStmt {
                file: name.to_string(),
                line,
                table,
                index,
                column: None,
                statement: statement.to_string(),
            }));
            continue;
        }
        if kw(0) == "ALTER" && kw(1) == "TABLE" {
            let mut cursor = 2;
            if kw(cursor) == "IF" && kw(cursor + 1) == "EXISTS" {
                cursor += 2;
            }
            if kw(cursor) == "ONLY" {
                cursor += 1;
            }
            let table = at(cursor).to_string();
            for (position, token) in upper.iter().enumerate() {
                if token != "DROP" {
                    continue;
                }
                match kw(position + 1) {
                    "COLUMN" => scanned.events.push(Event::Drop(DropStmt {
                        file: name.to_string(),
                        line,
                        table: Some(table.clone()),
                        index: None,
                        column: Some(at(position + 2).to_string()),
                        statement: statement.to_string(),
                    })),
                    "CONSTRAINT" | "DEFAULT" | "NOT" | "EXPRESSION" | "IDENTITY" => {}
                    other => {
                        return Err(format!(
                            "{here}: `DROP {other}` inside ALTER TABLE is not modelled — extend \
                             index_naming.rs\n  {statement}"
                        ));
                    }
                }
            }
            continue;
        }

        // ── create_hypertable: the default index nobody writes down ──────────
        if kw(0) == "SELECT" && kw(1) == "CREATE_HYPERTABLE" {
            let decl = parse_hypertable(&t, &upper, name, line, statement, &here)?;
            if let Some(decl) = decl {
                scanned.events.push(Event::Index(decl));
            }
            continue;
        }
        if kw(0) != "SELECT" && joined.contains("CREATE_HYPERTABLE") {
            return Err(format!(
                "{here}: create_hypertable in an unexpected position — extend \
                 index_naming.rs\n  {statement}"
            ));
        }

        // ── CREATE TABLE: names, plus the unnamed-UNIQUE refusal ─────────────
        if kw(0) == "CREATE" && kw(1) == "TABLE" {
            let mut cursor = 2;
            if kw(2) == "IF" && kw(3) == "NOT" && kw(4) == "EXISTS" {
                cursor = 5;
            }
            let table = at(cursor);
            if !is_plain_identifier(table) {
                return Err(format!("{here}: unsupported table name `{table}`"));
            }
            for (position, token) in upper.iter().enumerate() {
                if token == "UNIQUE" && kw(position.wrapping_sub(2)) != "CONSTRAINT" {
                    return Err(format!(
                        "{here}: an unnamed UNIQUE constraint produces a `<table>_<cols>_key` \
                         index that truncates and collides like any other, and this guard does \
                         not model it. Name the constraint.\n  {statement}"
                    ));
                }
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
                    "{here}: unsupported index name `{candidate}`\n  {statement}"
                ));
            }
            cursor += 1;
            Some(candidate.to_string())
        };

        if kw(cursor) != "ON" {
            return Err(format!(
                "{here}: expected ON after CREATE INDEX — extend index_naming.rs\n  {statement}"
            ));
        }
        cursor += 1;
        if kw(cursor) == "ONLY" {
            cursor += 1;
        }
        let table = at(cursor);
        if !is_plain_identifier(table) {
            return Err(format!(
                "{here}: unsupported table reference `{table}` — this guard reads unquoted, \
                 unqualified names only\n  {statement}"
            ));
        }
        cursor += 1;
        if kw(cursor) == "USING" {
            cursor += 2; // access method does not affect naming
        }
        if kw(cursor) != "(" {
            return Err(format!(
                "{here}: expected a column list — extend index_naming.rs\n  {statement}"
            ));
        }
        cursor += 1;

        // Column list: take the leading identifier of each item, refuse anything
        // else. Postgres names an expression element `expr`, and INCLUDE changes
        // what goes into the addition; neither is modelled here on purpose.
        let mut columns: Vec<String> = Vec::new();
        let mut expect_column = true;
        loop {
            match kw(cursor) {
                ")" => break,
                "(" => {
                    return Err(format!(
                        "{here}: unsupported index element — a parenthesised expression is named \
                         `expr` by Postgres, not after its columns. Extend \
                         index_naming.rs.\n  {statement}"
                    ));
                }
                "," => {
                    expect_column = true;
                    cursor += 1;
                }
                "" => {
                    return Err(format!("{here}: unterminated column list\n  {statement}"));
                }
                _ => {
                    if expect_column {
                        let column = at(cursor);
                        if !is_plain_identifier(column) {
                            return Err(format!(
                                "{here}: unsupported index element `{column}` (expression or \
                                 quoted identifier?) — Postgres names it differently. Extend \
                                 index_naming.rs.\n  {statement}"
                            ));
                        }
                        if columns.iter().any(|seen| seen == column) {
                            return Err(format!(
                                "{here}: column `{column}` appears twice — Postgres suffixes \
                                 repeated column names (`a`, `a1`) via ChooseIndexColumnNames(), \
                                 which this guard does not port. Extend \
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
        if upper[cursor..].iter().any(|token| token == "INCLUDE") {
            return Err(format!(
                "{here}: INCLUDE is not modelled by this guard — extend index_naming.rs.\n  \
                 {statement}"
            ));
        }
        if columns.is_empty() {
            return Err(format!("{here}: empty column list\n  {statement}"));
        }

        scanned.events.push(Event::Index(IndexDecl {
            file: name.to_string(),
            line,
            explicit_name,
            table: table.to_string(),
            columns,
            origin: Origin::Written,
        }));
    }

    // Completeness: every statement that *looks* index-shaped must have been
    // decomposed into one. A mismatch means the parser stopped seeing something
    // it used to see — the one failure this guard must never have.
    let parsed_written = scanned
        .indexes()
        .filter(|decl| decl.origin == Origin::Written)
        .count();
    if parsed_written != seen_index_statements {
        return Err(format!(
            "{name}: {seen_index_statements} statements mention CREATE INDEX but \
             {parsed_written} were decomposed — extend index_naming.rs rather than leaving one \
             uncovered"
        ));
    }
    let parsed_hypertables = scanned
        .indexes()
        .filter(|decl| decl.origin == Origin::HypertableDefault)
        .count();
    if parsed_hypertables > seen_hypertable_statements {
        return Err(format!(
            "{name}: modelled {parsed_hypertables} hypertable default indexes for \
             {seen_hypertable_statements} create_hypertable statements"
        ));
    }

    Ok(scanned)
}

/// The default time-dimension index `create_hypertable()` creates on the root
/// table, in `public`, letting Postgres name it. Returns `None` when the call
/// disables default indexes.
fn parse_hypertable(
    t: &[String],
    upper: &[String],
    file: &str,
    line: usize,
    statement: &str,
    here: &str,
) -> Result<Option<IndexDecl>, String> {
    let kw = |index: usize| upper.get(index).map(String::as_str).unwrap_or_default();
    let at = |index: usize| t.get(index).map(String::as_str).unwrap_or_default();

    if kw(2) != "(" {
        return Err(format!(
            "{here}: cannot read the create_hypertable arguments — extend index_naming.rs\n  \
             {statement}"
        ));
    }
    let Some(table) = string_literal(at(3)).filter(|table| is_plain_identifier(table)) else {
        return Err(format!(
            "{here}: create_hypertable's relation must be a plain quoted name\n  {statement}"
        ));
    };
    if kw(4) != "," {
        return Err(format!(
            "{here}: create_hypertable without a time column — the by_range/by_hash form is not \
             modelled. Extend index_naming.rs.\n  {statement}"
        ));
    }
    let Some(time_column) = string_literal(at(5)).filter(|column| is_plain_identifier(column))
    else {
        return Err(format!(
            "{here}: create_hypertable's time column must be a plain quoted name — the \
             by_range/by_hash form is not modelled. Extend index_naming.rs.\n  {statement}"
        ));
    };

    // Remaining arguments: only named ones we know cannot change which default
    // indexes are built. A positional third argument is the legacy
    // partitioning_column, which adds a second default index.
    let mut cursor = 6;
    let mut creates_default_indexes = true;
    while cursor < t.len() && kw(cursor) != ")" {
        if kw(cursor) == "," {
            cursor += 1;
            continue;
        }
        let argument = at(cursor).to_ascii_lowercase();
        if kw(cursor + 1) != "=>" {
            return Err(format!(
                "{here}: positional argument `{argument}` after the time column is the legacy \
                 partitioning_column, which adds a second default index. Extend \
                 index_naming.rs.\n  {statement}"
            ));
        }
        // The value runs to the next top-level comma; it is not one token —
        // `chunk_time_interval => INTERVAL '7 days'` is two.
        let value_start = cursor + 2;
        let mut value_end = value_start;
        let mut depth = 0usize;
        while value_end < t.len() {
            match kw(value_end) {
                "(" => depth += 1,
                ")" if depth == 0 => break,
                ")" => depth -= 1,
                "," if depth == 0 => break,
                _ => {}
            }
            value_end += 1;
        }
        let value: Vec<&str> = upper[value_start..value_end]
            .iter()
            .map(String::as_str)
            .collect();

        if argument == "create_default_indexes" {
            creates_default_indexes = match value.as_slice() {
                ["FALSE"] => false,
                ["TRUE"] => true,
                other => {
                    return Err(format!(
                        "{here}: create_default_indexes => `{}` is not a literal this guard can \
                         read\n  {statement}",
                        other.join(" ")
                    ));
                }
            };
        } else if !NAMING_NEUTRAL_HYPERTABLE_ARGS.contains(&argument.as_str()) {
            return Err(format!(
                "{here}: `{argument}` may change which default indexes create_hypertable builds. \
                 Extend index_naming.rs (or add it to NAMING_NEUTRAL_HYPERTABLE_ARGS once you \
                 have checked).\n  {statement}"
            ));
        }
        cursor = value_end;
    }

    Ok(creates_default_indexes.then(|| IndexDecl {
        file: file.to_string(),
        line,
        explicit_name: None,
        table,
        columns: vec![time_column],
        origin: Origin::HypertableDefault,
    }))
}

/// Scan every migration, in application order.
fn scan_migrations() -> Result<Scanned, String> {
    let mut all = Scanned::default();
    for path in migration_files() {
        let scanned = scan_file(&path)?;
        all.events.extend(scanned.events);
        all.tables.extend(scanned.tables);
    }
    Ok(all)
}

/// Replay Postgres' naming over `events`, in declaration order, and return the
/// indexes that needed a disambiguating suffix.
///
/// Errors on any statement that would free a name still held by a modelled
/// index: that shifts every suffix downstream, and the replay has no way to be
/// right about it.
fn collisions(events: &[Event]) -> Result<Vec<Collision>, String> {
    let mut taken: HashSet<String> = HashSet::new();
    let mut live: Vec<(String, String, Vec<String>)> = Vec::new(); // name, table, columns
    let mut found = Vec::new();

    for event in events {
        match event {
            Event::Index(decl) => {
                let name = match &decl.explicit_name {
                    // An explicit name is reserved as written; Postgres does not
                    // disambiguate it, it errors on a duplicate.
                    Some(name) => name.clone(),
                    None => {
                        let (name, passes) = generated_index_name(decl, &taken);
                        if passes > 0 {
                            found.push(Collision {
                                file: decl.file.clone(),
                                line: decl.line,
                                table: decl.table.clone(),
                                columns: decl.columns.clone(),
                                origin: decl.origin,
                                generated: name.clone(),
                            });
                        }
                        name
                    }
                };
                taken.insert(name.clone());
                live.push((name, decl.table.clone(), decl.columns.clone()));
            }
            Event::Drop(drop) => {
                let frees = live.iter().find(|(name, table, columns)| match drop {
                    DropStmt {
                        index: Some(dropped),
                        ..
                    } => name == dropped,
                    DropStmt {
                        table: Some(dropped),
                        column,
                        ..
                    } => {
                        table == dropped
                            && column
                                .as_ref()
                                .is_none_or(|column| columns.contains(column))
                    }
                    _ => false,
                });
                if let Some((name, _, _)) = frees {
                    return Err(format!(
                        "{}:{}: this frees the index name `{name}`, which shifts every \
                         disambiguating suffix created after it. That is not modelled — extend \
                         index_naming.rs before landing this migration.\n  {}",
                        drop.file, drop.line, drop.statement
                    ));
                }
            }
        }
    }
    Ok(found)
}

#[path = "index_naming/tests/index_naming_tests.rs"]
mod tests;
