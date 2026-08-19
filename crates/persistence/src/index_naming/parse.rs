//! One function per statement the scanner understands.
//!
//! Each reads a [`Statement`] and either produces the [`Event`]s it implies or
//! refuses — never a silent skip, which is the failure this whole guard exists
//! to prevent.

use super::lexer::{Statement, plain_identifier};
use super::{DropStmt, Event, IndexDecl, Origin, RenameStmt};

/// `ALTER INDEX <name> RENAME TO <name>` — the statement `migrations/README.md`
/// prescribes when this guard goes red, so it has to be understood.
pub(super) fn parse_alter_index(s: &Statement) -> Result<Event, String> {
    let (here, text) = (s.here(), s.text);
    let mut cursor = 2;
    if s.kw(cursor) == "IF" && s.kw(cursor + 1) == "EXISTS" {
        cursor += 2;
    }
    let (Some(from), "RENAME", "TO", Some(to)) = (
        relation_name(s.at(cursor)),
        s.kw(cursor + 1),
        s.kw(cursor + 2),
        relation_name(s.at(cursor + 3)),
    ) else {
        return Err(format!(
            "{here}: only `ALTER INDEX <name> RENAME TO <name>` is modelled — extend \
             index_naming.rs\n  {text}"
        ));
    };
    Ok(Event::RenameIndex(RenameStmt {
        file: s.file.to_string(),
        line: s.line,
        table: None,
        from,
        to,
        statement: s.text.to_string(),
    }))
}

/// `ALTER TABLE …` — the renames that move indexes around, and the drops that
/// free their names.
pub(super) fn parse_alter_table(s: &Statement) -> Result<Vec<Event>, String> {
    let (here, text) = (s.here(), s.text);
    let mut events = Vec::new();
    let mut cursor = 2;
    if s.kw(cursor) == "IF" && s.kw(cursor + 1) == "EXISTS" {
        cursor += 2;
    }
    if s.kw(cursor) == "ONLY" {
        cursor += 1;
    }
    let Some(table) = relation_name(s.at(cursor)) else {
        return Err(format!(
            "{here}: unsupported table reference `{}`\n  {text}",
            s.at(cursor)
        ));
    };
    cursor += 1;
    refuse_unnamed_unique(s)?;

    // A table rename carries every index with it, and that changes what
    // a later create_hypertable can reuse. A column rename changes what
    // an index is *on*, for the same reason.
    if s.kw(cursor) == "RENAME" {
        let renamed = |from: Option<String>, to: Option<String>, column: bool| match (from, to) {
            (Some(from), Some(to)) => Ok(RenameStmt {
                file: s.file.to_string(),
                line: s.line,
                table: column.then(|| table.clone()),
                from,
                to,
                statement: s.text.to_string(),
            }),
            _ => Err(format!(
                "{here}: cannot read this RENAME — extend index_naming.rs\n  {text}"
            )),
        };
        match (s.kw(cursor + 1), s.kw(cursor + 2)) {
            ("TO", _) => events.push(Event::RenameTable(renamed(
                Some(table.clone()),
                plain_identifier(s.at(cursor + 2)),
                false,
            )?)),
            ("COLUMN", _) | (_, "TO") => {
                let offset = usize::from(s.kw(cursor + 1) == "COLUMN");
                if s.kw(cursor + 2 + offset) != "TO" {
                    return Err(format!(
                        "{here}: cannot read this RENAME — extend index_naming.rs\n  \
                         {text}"
                    ));
                }
                events.push(Event::RenameColumn(renamed(
                    plain_identifier(s.at(cursor + 1 + offset)),
                    plain_identifier(s.at(cursor + 3 + offset)),
                    true,
                )?));
            }
            _ => {
                return Err(format!(
                    "{here}: cannot read this RENAME — extend index_naming.rs\n  \
                     {text}"
                ));
            }
        }
        return Ok(events);
    }

    for (position, token) in s.upper.iter().enumerate() {
        if token != "DROP" {
            continue;
        }
        // `DROP [COLUMN] [IF EXISTS] <name>` — the COLUMN keyword is
        // optional in Postgres, and IF EXISTS shifts the name along.
        let mut at_name = position + 1;
        if s.kw(at_name) == "COLUMN" {
            at_name += 1;
        }
        if matches!(
            s.kw(at_name),
            "CONSTRAINT" | "DEFAULT" | "NOT" | "EXPRESSION" | "IDENTITY"
        ) {
            continue;
        }
        if s.kw(at_name) == "IF" && s.kw(at_name + 1) == "EXISTS" {
            at_name += 2;
        }
        // Never fall back to "then it drops the whole table": that is a
        // silent widening, and this file hard-errors everywhere else a
        // name is unreadable.
        let Some(column) = plain_identifier(s.at(at_name)) else {
            return Err(format!(
                "{here}: cannot read the column this DROP targets (`{}`) — extend \
                 index_naming.rs\n  {text}",
                s.at(at_name)
            ));
        };
        events.push(Event::Drop(DropStmt {
            table: Some(table.clone()),
            index: None,
            column: Some(column),
        }));
    }
    Ok(events)
}

/// `CREATE [UNLOGGED | TEMP | …] TABLE …` — recorded for the `_pkey` length
/// check, and refused if it carries an unnamed `UNIQUE`.
pub(super) fn parse_create_table(s: &Statement, cursor: usize) -> Result<Event, String> {
    let (here, text) = (s.here(), s.text);
    let Some(table) = relation_name(s.at(cursor)) else {
        return Err(format!(
            "{here}: unsupported table name `{}`\n  {text}",
            s.at(cursor)
        ));
    };
    refuse_unnamed_unique(s)?;
    Ok(Event::Table(table))
}

/// `CREATE [UNIQUE] INDEX [name] ON <table> (<columns>)`. Returns `None` when
/// the statement starts with CREATE but is not an index.
pub(super) fn parse_create_index(s: &Statement) -> Result<Option<IndexDecl>, String> {
    let (here, text) = (s.here(), s.text);
    let mut cursor = 1;
    let unique = s.kw(cursor) == "UNIQUE";
    if unique {
        cursor += 1;
    }
    if s.kw(cursor) != "INDEX" {
        return Ok(None);
    }
    cursor += 1;
    if s.kw(cursor) == "CONCURRENTLY" {
        cursor += 1; // does not affect naming
    }
    if s.kw(cursor) == "IF" && s.kw(cursor + 1) == "NOT" && s.kw(cursor + 2) == "EXISTS" {
        cursor += 3;
    }

    let explicit_name = if s.kw(cursor) == "ON" {
        None
    } else {
        let Some(candidate) = plain_identifier(s.at(cursor)) else {
            return Err(format!(
                "{here}: unsupported index name `{}`\n  {text}",
                s.at(cursor)
            ));
        };
        cursor += 1;
        Some(candidate)
    };

    if s.kw(cursor) != "ON" {
        return Err(format!(
            "{here}: expected ON after CREATE INDEX — extend index_naming.rs\n  {text}"
        ));
    }
    cursor += 1;
    if s.kw(cursor) == "ONLY" {
        cursor += 1;
    }
    let Some(table) = relation_name(s.at(cursor)) else {
        return Err(format!(
            "{here}: unsupported table reference `{}` — this guard reads unquoted names \
             only\n  {text}",
            s.at(cursor)
        ));
    };
    cursor += 1;
    if s.kw(cursor) == "USING" {
        cursor += 2; // access method does not affect naming
    }
    if s.kw(cursor) != "(" {
        return Err(format!(
            "{here}: expected a column list — extend index_naming.rs\n  {text}"
        ));
    }
    cursor += 1;

    // Column list: take the leading identifier of each item, refuse anything
    // else. Postgres names an expression element `expr`, and INCLUDE changes
    // what goes into the addition; neither is modelled here on purpose.
    let mut columns: Vec<String> = Vec::new();
    let mut expect_column = true;
    loop {
        match s.kw(cursor) {
            ")" => break,
            "(" => {
                return Err(format!(
                    "{here}: unsupported index element — a parenthesised expression is named \
                     `expr` by Postgres, not after its columns. {NAME_IT_EXPLICITLY}.\n  \
                     {text}"
                ));
            }
            "," => {
                expect_column = true;
                cursor += 1;
            }
            "" => {
                return Err(format!("{here}: unterminated column list\n  {text}"));
            }
            _ => {
                if expect_column {
                    let Some(column) = plain_identifier(s.at(cursor)) else {
                        return Err(format!(
                            "{here}: unsupported index element `{}` (expression or quoted \
                             identifier?) — Postgres names it differently. \
                             {NAME_IT_EXPLICITLY}.\n  {text}",
                            s.at(cursor)
                        ));
                    };
                    if columns.contains(&column) {
                        return Err(format!(
                            "{here}: column `{column}` appears twice — Postgres suffixes \
                             repeated column names (`a`, `a1`) via ChooseIndexColumnNames(), \
                             which this guard does not port. {NAME_IT_EXPLICITLY}.\n  \
                             {text}"
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
    if s.upper[cursor..].iter().any(|token| token == "INCLUDE") {
        return Err(format!(
            "{here}: INCLUDE changes what goes into the generated name and is not modelled. \
             {NAME_IT_EXPLICITLY}.\n  {text}"
        ));
    }
    if columns.is_empty() {
        return Err(format!("{here}: empty column list\n  {text}"));
    }
    Ok(Some(IndexDecl {
        file: s.file.to_string(),
        line: s.line,
        explicit_name,
        table,
        columns,
        unique,
        origin: Origin::Written,
    }))
}

/// `CREATE [UNLOGGED | TEMP | …] TABLE [IF NOT EXISTS] <name>` — the position of
/// `<name>`, or `None` if this is not a `CREATE TABLE`.
pub(super) fn create_table_name_position(upper: &[String]) -> Option<usize> {
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
fn refuse_unnamed_unique(s: &Statement) -> Result<(), String> {
    let (here, text) = (s.here(), s.text);
    for (position, token) in s.upper.iter().enumerate() {
        if token == "UNIQUE" && s.kw(position.wrapping_sub(2)) != "CONSTRAINT" {
            return Err(format!(
                "{here}: an unnamed UNIQUE constraint produces a `<table>_<cols>_key` index that \
                 truncates and collides like any other, and this guard does not model it. Name \
                 the constraint.\n  {text}"
            ));
        }
    }
    Ok(())
}

/// A relation reference: unquoted, optionally schema-qualified. Returns the bare
/// relation name, which is what Postgres names indexes after.
pub(super) fn relation_name(token: &str) -> Option<String> {
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
pub(super) fn parse_drop(s: &Statement) -> Result<Vec<Event>, String> {
    let (here, text) = (s.here(), s.text);
    let kw = |index: usize| s.kw(index);
    let at = |index: usize| s.at(index);

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
             then add it to DROPS_NO_INDEX or handle it here.\n  {text}"
        ));
    }
    if kw(cursor) == "CONCURRENTLY" {
        cursor += 1;
    }
    if kw(cursor) == "IF" && kw(cursor + 1) == "EXISTS" {
        cursor += 2;
    }

    let mut events = Vec::new();
    while cursor < s.tokens.len() {
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
                "{here}: unsupported drop target `{}` — extend index_naming.rs\n  {text}",
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
            "{here}: could not read what this DROP targets — extend index_naming.rs\n  {text}"
        ));
    }
    Ok(events)
}

/// The advice that actually resolves most refusals, so the message does not send
/// the reader off to extend a parser when one line of SQL will do.
pub(super) const NAME_IT_EXPLICITLY: &str =
    "Name the index explicitly (see migrations/README.md), or extend index_naming.rs";
