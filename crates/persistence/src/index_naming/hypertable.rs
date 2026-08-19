//! `create_hypertable()` — the index nobody writes down.
//!
//! Unless it is passed `create_default_indexes => FALSE`, TimescaleDB creates a
//! default index on the time dimension, on the root table, in `public`, named by
//! the same algorithm as any other. Whether the call actually contributes one
//! also depends on what already exists, so that half is settled in
//! [`super::replay`].

use super::HypertableCall;
use super::lexer::{Statement, plain_identifier, string_literal};
use super::parse::relation_name;

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
pub(super) const UNMODELLED_INDEX_HELPERS: &[&str] = &["ADD_DIMENSION", "ADD_REORDER_POLICY"];

/// A `create_hypertable()` call. Whether it yields a default index is settled in
/// [`replay`], which knows what already exists at that point.
pub(super) fn parse_hypertable(s: &Statement) -> Result<HypertableCall, String> {
    let (here, text, file, line) = (s.here(), s.text, s.file, s.line);
    let kw = |index: usize| s.kw(index);
    let at = |index: usize| s.at(index);

    let call = s
        .upper
        .iter()
        .position(|token| token == "CREATE_HYPERTABLE" || token.ends_with(".CREATE_HYPERTABLE"))
        .ok_or_else(|| format!("{here}: create_hypertable not found\n  {text}"))?;
    if kw(call + 1) != "(" {
        return Err(format!(
            "{here}: cannot read the create_hypertable arguments — extend index_naming.rs\n  \
             {text}"
        ));
    }
    let Some(table) = string_literal(at(call + 2)).and_then(|table| relation_name(&table)) else {
        return Err(format!(
            "{here}: create_hypertable's relation must be a plain quoted name\n  {text}"
        ));
    };
    if kw(call + 3) != "," {
        return Err(format!(
            "{here}: create_hypertable without a time column — the by_range/by_hash form is not \
             modelled. Extend index_naming.rs.\n  {text}"
        ));
    }
    let Some(time_column) = string_literal(at(call + 4)).and_then(|c| plain_identifier(&c)) else {
        return Err(format!(
            "{here}: create_hypertable's time column must be a plain quoted name — the \
             by_range/by_hash form is not modelled. Extend index_naming.rs.\n  {text}"
        ));
    };

    // Remaining arguments: only named ones we know cannot change which default
    // indexes are built. A positional third argument is the legacy
    // partitioning_column, which adds a second default index.
    let mut cursor = call + 5;
    let mut creates_default_indexes = true;
    while cursor < s.tokens.len() && kw(cursor) != ")" {
        if kw(cursor) == "," {
            cursor += 1;
            continue;
        }
        let argument = at(cursor).to_ascii_lowercase();
        if kw(cursor + 1) != "=>" {
            return Err(format!(
                "{here}: positional argument `{argument}` after the time column is the legacy \
                 partitioning_column, which adds a second default index. Extend \
                 index_naming.rs.\n  {text}"
            ));
        }
        // The value runs to the next top-level comma; it is not one token —
        // `chunk_time_interval => INTERVAL '7 days'` is two.
        let value_start = cursor + 2;
        let mut value_end = value_start;
        let mut depth = 0usize;
        while value_end < s.tokens.len() {
            match kw(value_end) {
                "(" => depth += 1,
                ")" if depth == 0 => break,
                ")" => depth -= 1,
                "," if depth == 0 => break,
                _ => {}
            }
            value_end += 1;
        }
        let value: Vec<&str> = s.upper[value_start..value_end]
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
                         read\n  {text}",
                        other.join(" ")
                    ));
                }
            };
        } else if !NAMING_NEUTRAL_HYPERTABLE_ARGS.contains(&argument.as_str()) {
            return Err(format!(
                "{here}: `{argument}` may change which default indexes create_hypertable builds. \
                 Extend index_naming.rs (or add it to NAMING_NEUTRAL_HYPERTABLE_ARGS once you \
                 have checked).\n  {text}"
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
