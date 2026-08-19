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
//! A name left out of the replay is a name it thinks is free, and the guard then
//! reports green on a real collision. `create_hypertable()` is the trap here: it
//! creates a **default index on the time dimension, on the root table, in
//! `public`**, letting Postgres name it through this very algorithm — and it does
//! so *before* the `CREATE INDEX` lines that follow it in the file. The 21 calls
//! in `001_baseline.sql` add 21 such names, one of which is already truncated to
//! 63 bytes
//! (`meteora_damm_v2_withdraw_dead_liquidity_reward_ev_timestamp_idx`).
//!
//! Two conditions suppress that default index, both measured against
//! `timescale/timescaledb:latest-pg16` rather than assumed:
//!
//! - `create_default_indexes => FALSE`;
//! - a **non-unique** index whose *leading* column is the time column already
//!   exists on the table. A `PRIMARY KEY (time_col, id)` does **not** suppress
//!   it, and neither does a `CREATE UNIQUE INDEX (time_col, …)` — both were
//!   tried, both still got the default index.
//!
//! # Names that appear and disappear
//!
//! `DROP INDEX`, `DROP TABLE` and `ALTER TABLE … DROP COLUMN` free a name for
//! reuse, and `ALTER INDEX … RENAME TO` frees one while taking another. The
//! replay models all four, because refusing them outright would block ordinary
//! migrations — including the explicit renaming that
//! `migrations/README.md` prescribes as the fix when this guard goes red.
//!
//! # What this guard does not cover
//!
//! - **Constraint-borne indexes.** `<table>_pkey` is covered indirectly: the
//!   tests assert no table name is long enough for it to truncate. An *unnamed*
//!   `UNIQUE (…)` produces `<table>_<cols>_key`, which truncates and collides
//!   exactly like `_idx`; rather than parse constraint bodies, [`scan_sql`]
//!   refuses one. A *named* UNIQUE constraint is accepted and its index name is
//!   neither length-checked nor entered into the namespace.
//! - **Index-vs-relation collisions.** Postgres shares one relation namespace, so
//!   an index could in principle collide with a table or a view. Only
//!   index-vs-index is modelled; the tests assert no table name ends in `_idx`,
//!   which is the shape that could collide. View names are not checked.
//! - **TimescaleDB's per-chunk and materialization indexes**, which live in
//!   `_timescaledb_internal` and are named after `_hyper_N_M_chunk` /
//!   `_materialized_hypertable_N`, not after our tables.
//!
//! - **TimescaleDB helpers other than `create_hypertable`.** `add_dimension`
//!   and friends can add their own default index; they are refused, not
//!   modelled, so the guard stops rather than guesses.
//!
//! `CREATE INDEX` and `create_hypertable` are the two index-creating forms the
//! scanner models, and a per-file count cross-check makes sure none of them
//! slipped past unrecognised. Anything it cannot decompose is a **hard error**,
//! never a skip.

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
    unique: bool,
    origin: Origin,
}

/// A `create_hypertable()` call. Whether it contributes a default index depends
/// on what already exists when it runs, so it is resolved during [`replay`].
#[derive(Debug, Clone, PartialEq, Eq)]
struct HypertableCall {
    file: String,
    line: usize,
    table: String,
    time_column: String,
    creates_default_indexes: bool,
}

/// A statement that removes an index, or something an index hangs off.
#[derive(Debug, Clone, PartialEq, Eq)]
struct DropStmt {
    /// Relation the drop targets; `None` for `DROP INDEX`, which names the index.
    table: Option<String>,
    /// Index name for `DROP INDEX`.
    index: Option<String>,
    /// Column for `ALTER TABLE … DROP COLUMN`; `None` drops the whole relation.
    column: Option<String>,
}

/// A rename: `ALTER INDEX … RENAME TO …` frees one index name and takes
/// another; `ALTER TABLE … RENAME TO …` moves every index of a table onto a new
/// table name, which changes what `create_hypertable` can reuse.
#[derive(Debug, Clone, PartialEq, Eq)]
struct RenameStmt {
    file: String,
    line: usize,
    /// For a column rename, the table the column belongs to.
    table: Option<String>,
    from: String,
    to: String,
    statement: String,
}

/// DDL that bears on index naming, in file order.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Event {
    Table(String),
    Index(IndexDecl),
    Hypertable(HypertableCall),
    Drop(DropStmt),
    RenameIndex(RenameStmt),
    RenameTable(RenameStmt),
    RenameColumn(RenameStmt),
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
}

/// An index the replay believes exists right now.
#[derive(Debug, Clone)]
struct LiveIndex {
    name: String,
    table: String,
    columns: Vec<String>,
    unique: bool,
}

/// The outcome of replaying the DDL.
#[derive(Debug, Default)]
struct Replay {
    /// The indexes that needed a disambiguating suffix.
    collisions: Vec<Collision>,
    /// Every index *creation*, with the name Postgres gave it at that moment —
    /// not the surviving set: a dropped index stays here, and a renamed one
    /// keeps the name it was born with. [`Replay::live_names`] is the surviving
    /// set.
    named: Vec<(String, IndexDecl)>,
    /// Index names that still exist at the end of the replay.
    live_names: Vec<String>,
    /// Tables that still exist at the end of the replay.
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
///
/// A `$tag$` body is replaced by a space too, so a `DO` block reads as one
/// opaque statement — but its text is searched first, because DDL hidden in one
/// would otherwise be invisible to the parser *and* to the count cross-check.
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
        // E'…' carries backslash escapes we do not model; refuse rather than
        // mis-split on an escaped quote. The prefix only counts when the letter
        // stands alone — `DATE'…'` and `ELSE'x'` are ordinary literals. (`U&'…'`
        // needs no special case: it escapes by doubling, like a plain literal.)
        if (c == 'E' || c == 'e')
            && next == Some('\'')
            && !chars
                .get(i.wrapping_sub(1))
                .is_some_and(|prev| prev.is_ascii_alphanumeric() || *prev == '_')
        {
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
        // $tag$ … $tag$ body — skipped whole, but not unread.
        if c == '$'
            && let Some(tag) = dollar_tag(&chars, i)
        {
            let opened_at = line;
            i += tag.len();
            let body_start = i;
            while i < chars.len() && !starts_with_at(&chars, i, &tag) {
                if chars[i] == '\n' {
                    line += 1;
                }
                i += 1;
            }
            if i >= chars.len() {
                return Err(format!("line {opened_at}: unterminated {tag} block"));
            }
            let body: String = chars[body_start..i]
                .iter()
                .collect::<String>()
                .to_uppercase();
            let flat = body.split_whitespace().collect::<Vec<_>>().join(" ");
            if HIDDEN_DDL_MARKERS
                .iter()
                .any(|marker| flat.contains(marker))
            {
                return Err(format!(
                    "line {opened_at}: index DDL inside a {tag} body is invisible to this guard — \
                     move it out of the block, or extend index_naming.rs"
                ));
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

/// Statements that create, remove or rename an index. Inside a `$tag$` body the
/// scanner cannot decompose them, so their mere presence is refused.
const HIDDEN_DDL_MARKERS: &[&str] = &[
    "CREATE INDEX",
    "CREATE UNIQUE INDEX",
    "CREATE_HYPERTABLE",
    "DROP INDEX",
    "DROP TABLE",
    "ALTER INDEX",
    "RENAME TO",
];

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

/// An unquoted SQL identifier, which Postgres folds to lower case. Quoted names
/// keep their `"` and are rejected — they are case-sensitive and this guard does
/// not model that.
fn plain_identifier(token: &str) -> Option<String> {
    let ok = !token.is_empty()
        && token.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        && !token.starts_with(|c: char| c.is_ascii_digit());
    ok.then(|| token.to_ascii_lowercase())
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

/// TimescaleDB helpers that can add a default index of their own, the way
/// `create_hypertable` does. None are used today; refusing beats guessing.
const UNMODELLED_INDEX_HELPERS: &[&str] = &["ADD_DIMENSION", "ADD_REORDER_POLICY"];

/// The advice that actually resolves most refusals, so the message does not send
/// the reader off to extend a parser when one line of SQL will do.
const NAME_IT_EXPLICITLY: &str =
    "Name the index explicitly (see migrations/README.md), or extend index_naming.rs";

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
/// guard, because it reports green. The count cross-checks at the end are what
/// catch a statement that failed to *look* index-shaped at all.
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
        // Built from code tokens only: `COMMENT ON … IS 'use CREATE INDEX here'`
        // must not be counted as an index statement and then refused for not
        // decomposing into one.
        let joined = upper
            .iter()
            .filter(|token| !token.starts_with('\'') && !token.starts_with('"'))
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join(" ");
        let here = format!("{name}:{line}");

        if joined.contains("CREATE INDEX") || joined.contains("CREATE UNIQUE INDEX") {
            seen_index_statements += 1;
        }
        if joined.contains("CREATE_HYPERTABLE") {
            seen_hypertable_statements += 1;
        }
        if let Some(helper) = UNMODELLED_INDEX_HELPERS
            .iter()
            .find(|helper| joined.contains(*helper))
        {
            return Err(format!(
                "{here}: `{}` can create an index of its own and is not modelled — extend \
                 index_naming.rs.\n  {statement}",
                helper.to_ascii_lowercase()
            ));
        }

        // ── statements that free a name ──────────────────────────────────────
        if kw(0) == "DROP" {
            scanned
                .events
                .extend(parse_drop(&t, &upper, &here, statement)?);
            continue;
        }
        if kw(0) == "ALTER" && kw(1) == "INDEX" {
            let mut cursor = 2;
            if kw(cursor) == "IF" && kw(cursor + 1) == "EXISTS" {
                cursor += 2;
            }
            let (Some(from), "RENAME", "TO", Some(to)) = (
                relation_name(at(cursor)),
                kw(cursor + 1),
                kw(cursor + 2),
                relation_name(at(cursor + 3)),
            ) else {
                return Err(format!(
                    "{here}: only `ALTER INDEX <name> RENAME TO <name>` is modelled — extend \
                     index_naming.rs\n  {statement}"
                ));
            };
            scanned.events.push(Event::RenameIndex(RenameStmt {
                file: name.to_string(),
                line,
                table: None,
                from,
                to,
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
            let Some(table) = relation_name(at(cursor)) else {
                return Err(format!(
                    "{here}: unsupported table reference `{}`\n  {statement}",
                    at(cursor)
                ));
            };
            cursor += 1;
            refuse_unnamed_unique(&upper, &here, statement)?;

            // A table rename carries every index with it, and that changes what
            // a later create_hypertable can reuse. A column rename changes what
            // an index is *on*, for the same reason.
            if kw(cursor) == "RENAME" {
                let renamed = |from: Option<String>, to: Option<String>, column: bool| match (
                    from, to,
                ) {
                    (Some(from), Some(to)) => Ok(RenameStmt {
                        file: name.to_string(),
                        line,
                        table: column.then(|| table.clone()),
                        from,
                        to,
                        statement: statement.to_string(),
                    }),
                    _ => Err(format!(
                        "{here}: cannot read this RENAME — extend index_naming.rs\n  {statement}"
                    )),
                };
                match (kw(cursor + 1), kw(cursor + 2)) {
                    ("TO", _) => scanned.events.push(Event::RenameTable(renamed(
                        Some(table.clone()),
                        plain_identifier(at(cursor + 2)),
                        false,
                    )?)),
                    ("COLUMN", _) | (_, "TO") => {
                        let offset = usize::from(kw(cursor + 1) == "COLUMN");
                        if kw(cursor + 2 + offset) != "TO" {
                            return Err(format!(
                                "{here}: cannot read this RENAME — extend index_naming.rs\n  \
                                 {statement}"
                            ));
                        }
                        scanned.events.push(Event::RenameColumn(renamed(
                            plain_identifier(at(cursor + 1 + offset)),
                            plain_identifier(at(cursor + 3 + offset)),
                            true,
                        )?));
                    }
                    _ => {
                        return Err(format!(
                            "{here}: cannot read this RENAME — extend index_naming.rs\n  \
                             {statement}"
                        ));
                    }
                }
                continue;
            }

            for (position, token) in upper.iter().enumerate() {
                if token != "DROP" {
                    continue;
                }
                // `DROP [COLUMN] [IF EXISTS] <name>` — the COLUMN keyword is
                // optional in Postgres, and IF EXISTS shifts the name along.
                let mut at_name = position + 1;
                if kw(at_name) == "COLUMN" {
                    at_name += 1;
                }
                if matches!(
                    kw(at_name),
                    "CONSTRAINT" | "DEFAULT" | "NOT" | "EXPRESSION" | "IDENTITY"
                ) {
                    continue;
                }
                if kw(at_name) == "IF" && kw(at_name + 1) == "EXISTS" {
                    at_name += 2;
                }
                // Never fall back to "then it drops the whole table": that is a
                // silent widening, and this file hard-errors everywhere else a
                // name is unreadable.
                let Some(column) = plain_identifier(at(at_name)) else {
                    return Err(format!(
                        "{here}: cannot read the column this DROP targets (`{}`) — extend \
                         index_naming.rs\n  {statement}",
                        at(at_name)
                    ));
                };
                scanned.events.push(Event::Drop(DropStmt {
                    table: Some(table.clone()),
                    index: None,
                    column: Some(column),
                }));
            }
            continue;
        }

        // ── create_hypertable: the default index nobody writes down ──────────
        if joined.contains("CREATE_HYPERTABLE") {
            scanned.events.push(Event::Hypertable(parse_hypertable(
                &t, &upper, name, line, statement, &here,
            )?));
            continue;
        }

        // ── CREATE TABLE: names, plus the unnamed-UNIQUE refusal ─────────────
        if kw(0) == "CREATE"
            && let Some(cursor) = create_table_name_position(&upper)
        {
            let Some(table) = relation_name(at(cursor)) else {
                return Err(format!(
                    "{here}: unsupported table name `{}`\n  {statement}",
                    at(cursor)
                ));
            };
            refuse_unnamed_unique(&upper, &here, statement)?;
            scanned.events.push(Event::Table(table));
            continue;
        }
        if kw(0) != "CREATE" {
            continue;
        }
        let mut cursor = 1;
        let unique = kw(cursor) == "UNIQUE";
        if unique {
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
            let Some(candidate) = plain_identifier(at(cursor)) else {
                return Err(format!(
                    "{here}: unsupported index name `{}`\n  {statement}",
                    at(cursor)
                ));
            };
            cursor += 1;
            Some(candidate)
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
        let Some(table) = relation_name(at(cursor)) else {
            return Err(format!(
                "{here}: unsupported table reference `{}` — this guard reads unquoted names \
                 only\n  {statement}",
                at(cursor)
            ));
        };
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
                         `expr` by Postgres, not after its columns. {NAME_IT_EXPLICITLY}.\n  \
                         {statement}"
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
                        let Some(column) = plain_identifier(at(cursor)) else {
                            return Err(format!(
                                "{here}: unsupported index element `{}` (expression or quoted \
                                 identifier?) — Postgres names it differently. \
                                 {NAME_IT_EXPLICITLY}.\n  {statement}",
                                at(cursor)
                            ));
                        };
                        if columns.contains(&column) {
                            return Err(format!(
                                "{here}: column `{column}` appears twice — Postgres suffixes \
                                 repeated column names (`a`, `a1`) via ChooseIndexColumnNames(), \
                                 which this guard does not port. {NAME_IT_EXPLICITLY}.\n  \
                                 {statement}"
                            ));
                        }
                        columns.push(column);
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
                "{here}: INCLUDE changes what goes into the generated name and is not modelled. \
                 {NAME_IT_EXPLICITLY}.\n  {statement}"
            ));
        }
        if columns.is_empty() {
            return Err(format!("{here}: empty column list\n  {statement}"));
        }

        scanned.events.push(Event::Index(IndexDecl {
            file: name.to_string(),
            line,
            explicit_name,
            table,
            columns,
            unique,
            origin: Origin::Written,
        }));
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

/// `CREATE [UNLOGGED | TEMP | …] TABLE [IF NOT EXISTS] <name>` — the position of
/// `<name>`, or `None` if this is not a `CREATE TABLE`.
fn create_table_name_position(upper: &[String]) -> Option<usize> {
    let mut cursor = 1;
    while matches!(
        upper.get(cursor).map(String::as_str),
        Some("UNLOGGED" | "TEMP" | "TEMPORARY" | "GLOBAL" | "LOCAL")
    ) {
        cursor += 1;
    }
    if upper.get(cursor).map(String::as_str) != Some("TABLE") {
        return None;
    }
    cursor += 1;
    if upper.get(cursor).map(String::as_str) == Some("IF") {
        cursor += 3; // IF NOT EXISTS
    }
    Some(cursor)
}

/// An unnamed `UNIQUE` constraint produces `<table>_<cols>_key`, which truncates
/// and collides exactly like `_idx`, and this guard does not model it.
fn refuse_unnamed_unique(upper: &[String], here: &str, statement: &str) -> Result<(), String> {
    for (position, token) in upper.iter().enumerate() {
        if token == "UNIQUE"
            && upper.get(position.wrapping_sub(2)).map(String::as_str) != Some("CONSTRAINT")
        {
            return Err(format!(
                "{here}: an unnamed UNIQUE constraint produces a `<table>_<cols>_key` index that \
                 truncates and collides like any other, and this guard does not model it. Name \
                 the constraint.\n  {statement}"
            ));
        }
    }
    Ok(())
}

/// A relation reference: unquoted, optionally schema-qualified. Returns the bare
/// relation name, which is what Postgres names indexes after.
fn relation_name(token: &str) -> Option<String> {
    let bare = token.rsplit('.').next()?;
    plain_identifier(bare)
}

/// Object kinds a `DROP` can name that carry no index of ours, so dropping one
/// frees nothing the replay is tracking.
const DROPS_NO_INDEX: &[&str] = &[
    "TRIGGER",
    "POLICY",
    "RULE",
    "FUNCTION",
    "PROCEDURE",
    "TYPE",
    "DOMAIN",
    "AGGREGATE",
    "OPERATOR",
    "COLLATION",
    "STATISTICS",
    "CAST",
];

/// `DROP INDEX|TABLE|VIEW|MATERIALIZED VIEW [CONCURRENTLY] [IF EXISTS] a, b …`
///
/// The kind is **whitelisted**. Treating everything that is not `INDEX` as a
/// relation drop reads `DROP TRIGGER x ON pools` as "drop the table `pools`" —
/// `ON` parses as a name too — which silently frees every index name held on
/// it, and the guard then reports green on a collision the server does produce.
fn parse_drop(
    t: &[String],
    upper: &[String],
    here: &str,
    statement: &str,
) -> Result<Vec<Event>, String> {
    let kw = |index: usize| upper.get(index).map(String::as_str).unwrap_or_default();
    let at = |index: usize| t.get(index).map(String::as_str).unwrap_or_default();

    let mut cursor = 1;
    if kw(cursor) == "MATERIALIZED" {
        cursor += 1;
    }
    let kind = kw(cursor).to_string();
    cursor += 1;
    if DROPS_NO_INDEX.contains(&kind.as_str()) {
        return Ok(Vec::new());
    }
    if !matches!(kind.as_str(), "INDEX" | "TABLE" | "VIEW") {
        return Err(format!(
            "{here}: `DROP {kind}` is not modelled — decide whether it can free an index name, \
             then add it to DROPS_NO_INDEX or handle it here.\n  {statement}"
        ));
    }
    if kw(cursor) == "CONCURRENTLY" {
        cursor += 1;
    }
    if kw(cursor) == "IF" && kw(cursor + 1) == "EXISTS" {
        cursor += 2;
    }

    let mut events = Vec::new();
    while cursor < t.len() {
        match kw(cursor) {
            "," => {
                cursor += 1;
                continue;
            }
            "CASCADE" | "RESTRICT" => break,
            _ => {}
        }
        let Some(target) = relation_name(at(cursor)) else {
            return Err(format!(
                "{here}: unsupported drop target `{}` — extend index_naming.rs\n  {statement}",
                at(cursor)
            ));
        };
        events.push(Event::Drop(match kind.as_str() {
            "INDEX" => DropStmt {
                table: None,
                index: Some(target),
                column: None,
            },
            _ => DropStmt {
                table: Some(target),
                index: None,
                column: None,
            },
        }));
        cursor += 1;
    }
    if events.is_empty() {
        return Err(format!(
            "{here}: could not read what this DROP targets — extend index_naming.rs\n  {statement}"
        ));
    }
    Ok(events)
}

/// A `create_hypertable()` call. Whether it yields a default index is settled in
/// [`replay`], which knows what already exists at that point.
fn parse_hypertable(
    t: &[String],
    upper: &[String],
    file: &str,
    line: usize,
    statement: &str,
    here: &str,
) -> Result<HypertableCall, String> {
    let kw = |index: usize| upper.get(index).map(String::as_str).unwrap_or_default();
    let at = |index: usize| t.get(index).map(String::as_str).unwrap_or_default();

    let call = upper
        .iter()
        .position(|token| token == "CREATE_HYPERTABLE" || token.ends_with(".CREATE_HYPERTABLE"))
        .ok_or_else(|| format!("{here}: create_hypertable not found\n  {statement}"))?;
    if kw(call + 1) != "(" {
        return Err(format!(
            "{here}: cannot read the create_hypertable arguments — extend index_naming.rs\n  \
             {statement}"
        ));
    }
    let Some(table) = string_literal(at(call + 2)).and_then(|table| relation_name(&table)) else {
        return Err(format!(
            "{here}: create_hypertable's relation must be a plain quoted name\n  {statement}"
        ));
    };
    if kw(call + 3) != "," {
        return Err(format!(
            "{here}: create_hypertable without a time column — the by_range/by_hash form is not \
             modelled. Extend index_naming.rs.\n  {statement}"
        ));
    }
    let Some(time_column) = string_literal(at(call + 4)).and_then(|c| plain_identifier(&c)) else {
        return Err(format!(
            "{here}: create_hypertable's time column must be a plain quoted name — the \
             by_range/by_hash form is not modelled. Extend index_naming.rs.\n  {statement}"
        ));
    };

    // Remaining arguments: only named ones we know cannot change which default
    // indexes are built. A positional third argument is the legacy
    // partitioning_column, which adds a second default index.
    let mut cursor = call + 5;
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

    Ok(HypertableCall {
        file: file.to_string(),
        line,
        table,
        time_column,
        creates_default_indexes,
    })
}

/// Scan every migration, in application order.
fn scan_migrations() -> Result<Scanned, String> {
    let mut all = Scanned::default();
    for path in migration_files() {
        let scanned = scan_file(&path)?;
        all.events.extend(scanned.events);
    }
    Ok(all)
}

/// Replay Postgres' naming over `events`, in declaration order.
fn replay(events: &[Event]) -> Result<Replay, String> {
    let mut taken: HashSet<String> = HashSet::new();
    let mut live: Vec<LiveIndex> = Vec::new();
    let mut tables: Vec<String> = Vec::new();
    let mut out = Replay::default();

    for event in events {
        match event {
            Event::Table(table) => tables.push(table.clone()),
            Event::Index(decl) => place(decl, &mut taken, &mut live, &mut out)?,
            Event::Hypertable(call) => {
                if !call.creates_default_indexes {
                    continue;
                }
                // Measured, not assumed: TimescaleDB reuses a **non-unique**
                // index whose leading column is the time column and skips its
                // default. A primary key on `(time_col, …)` does not count, nor
                // does a unique index.
                let reused = live.iter().any(|index| {
                    !index.unique
                        && index.table == call.table
                        && index.columns.first() == Some(&call.time_column)
                });
                if reused {
                    continue;
                }
                place(
                    &IndexDecl {
                        file: call.file.clone(),
                        line: call.line,
                        explicit_name: None,
                        table: call.table.clone(),
                        columns: vec![call.time_column.clone()],
                        unique: false,
                        origin: Origin::HypertableDefault,
                    },
                    &mut taken,
                    &mut live,
                    &mut out,
                )?;
            }
            Event::Drop(drop) => {
                if let DropStmt {
                    table: Some(dropped),
                    column: None,
                    ..
                } = drop
                {
                    tables.retain(|table| table != dropped);
                }
                live.retain(|index| {
                    let hit = match drop {
                        DropStmt {
                            index: Some(dropped),
                            ..
                        } => &index.name == dropped,
                        DropStmt {
                            table: Some(dropped),
                            column,
                            ..
                        } => {
                            &index.table == dropped
                                && column
                                    .as_ref()
                                    .is_none_or(|column| index.columns.contains(column))
                        }
                        _ => false,
                    };
                    if hit {
                        taken.remove(&index.name);
                    }
                    !hit
                });
            }
            Event::RenameIndex(rename) => {
                reserve(&rename.to, rename, &mut taken)?;
                if let Some(index) = live.iter_mut().find(|index| index.name == rename.from) {
                    taken.remove(&rename.from);
                    index.name = rename.to.clone();
                }
            }
            // A table rename carries its indexes onto the new name, which is
            // what a later create_hypertable looks at when deciding whether to
            // reuse one.
            Event::RenameTable(rename) => {
                for table in tables.iter_mut().filter(|table| **table == rename.from) {
                    table.clone_from(&rename.to);
                }
                for index in live.iter_mut().filter(|index| index.table == rename.from) {
                    index.table.clone_from(&rename.to);
                }
            }
            Event::RenameColumn(rename) => {
                let table = rename.table.as_deref().unwrap_or_default();
                for index in live.iter_mut().filter(|index| index.table == table) {
                    for column in index.columns.iter_mut().filter(|c| **c == rename.from) {
                        column.clone_from(&rename.to);
                    }
                }
            }
        }
    }
    out.live_names = live.into_iter().map(|index| index.name).collect();
    out.tables = tables;
    Ok(out)
}

/// Take a name that the author chose, checking what Postgres would check.
///
/// The 63-byte bound matters most here: `migrations/README.md` prescribes
/// `ALTER INDEX … RENAME TO …` as the way out when this guard goes red, and a
/// rename target longer than that is silently truncated by the server — handing
/// the collision problem straight back.
fn reserve(name: &str, rename: &RenameStmt, taken: &mut HashSet<String>) -> Result<(), String> {
    if name.len() > MAX_IDENTIFIER_LEN {
        return Err(format!(
            "{}:{}: `{name}` is {} characters — Postgres truncates it to {MAX_IDENTIFIER_LEN}, \
             which is how names start colliding in the first place\n  {}",
            rename.file,
            rename.line,
            name.len(),
            rename.statement
        ));
    }
    if !taken.insert(name.to_string()) {
        return Err(format!(
            "{}:{}: `{name}` is already taken — this migration would fail to apply\n  {}",
            rename.file, rename.line, rename.statement
        ));
    }
    Ok(())
}

fn place(
    decl: &IndexDecl,
    taken: &mut HashSet<String>,
    live: &mut Vec<LiveIndex>,
    out: &mut Replay,
) -> Result<(), String> {
    let name = match &decl.explicit_name {
        // An explicit name is reserved as written; Postgres does not
        // disambiguate it, it errors on a duplicate — and truncates an over-long
        // one, which would hand back the very problem this guard exists for.
        Some(name) => {
            if name.len() > MAX_IDENTIFIER_LEN {
                return Err(format!(
                    "{}:{}: `{name}` is {} characters — Postgres truncates it to \
                     {MAX_IDENTIFIER_LEN}\n",
                    decl.file,
                    decl.line,
                    name.len()
                ));
            }
            if taken.contains(name) {
                return Err(format!(
                    "{}:{}: `{name}` is already taken — this migration would fail to apply",
                    decl.file, decl.line
                ));
            }
            name.clone()
        }
        None => {
            let (name, passes) = generated_index_name(decl, taken);
            if passes > 0 {
                out.collisions.push(Collision {
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
    live.push(LiveIndex {
        name: name.clone(),
        table: decl.table.clone(),
        columns: decl.columns.clone(),
        unique: decl.unique,
    });
    out.named.push((name, decl.clone()));
    Ok(())
}

#[path = "index_naming/tests/index_naming_tests.rs"]
mod tests;
