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

#[path = "index_naming/tests/mod.rs"]
mod tests;

mod hypertable;
mod lexer;
mod parse;
mod pg_names;
mod replay;
mod scan;
