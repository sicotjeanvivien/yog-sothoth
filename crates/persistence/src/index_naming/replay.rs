//! Replaying the DDL: which name each index actually ends up with.
//!
//! The namespace is stateful — a name can be taken, freed by a drop, and taken
//! again — so the order of [`Event`]s is the whole point. This is also where a
//! `create_hypertable` call decides whether it contributes a default index,
//! because that depends on what exists at that moment.

use std::collections::HashSet;

use super::pg_names::generated_index_name;
use super::{
    Collision, DropStmt, Event, IndexDecl, LiveIndex, MAX_IDENTIFIER_LEN, Origin, RenameStmt,
    Replay,
};

/// Replay Postgres' naming over `events`, in declaration order.
pub(super) fn replay(events: &[Event]) -> Result<Replay, String> {
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
