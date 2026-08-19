//! The port of Postgres' index-naming functions.
//!
//! Faithful to `src/backend/commands/indexcmds.c` and
//! `src/backend/catalog/indexing.c` — keep it that way; do not "simplify". Every
//! divergence here makes the guard predict a name the server does not use.

use std::collections::HashSet;

use super::{IndexDecl, MAX_IDENTIFIER_LEN, NAMEDATALEN};

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
pub(super) fn make_object_name(name1: &str, name2: Option<&str>, label: &str) -> String {
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
pub(super) fn choose_index_name_addition(columns: &[String]) -> String {
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
pub(super) fn choose_relation_name(
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
pub(super) fn generated_index_name(decl: &IndexDecl, taken: &HashSet<String>) -> (String, u32) {
    let addition = choose_index_name_addition(&decl.columns);
    choose_relation_name(&decl.table, Some(&addition), "idx", taken)
}
