//! Turning migration text into statements and tokens.
//!
//! Nothing here knows what an index is; it only has to hand [`super::scan`] the
//! same statements Postgres would see — comments removed, quoted text intact,
//! and DDL hidden inside a `$tag$` body reported rather than skipped.

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
pub(super) fn statements(sql: &str) -> Result<Vec<(usize, String)>, String> {
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
pub(super) fn dollar_tag(chars: &[char], i: usize) -> Option<String> {
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
pub(super) fn tokens(statement: &str) -> Vec<String> {
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
pub(super) fn plain_identifier(token: &str) -> Option<String> {
    let ok = !token.is_empty()
        && token.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        && !token.starts_with(|c: char| c.is_ascii_digit());
    ok.then(|| token.to_ascii_lowercase())
}

/// The content of a `'…'` literal, if the token is one.
pub(super) fn string_literal(token: &str) -> Option<String> {
    let inner = token.strip_prefix('\'')?.strip_suffix('\'')?;
    Some(inner.replace("''", "'"))
}

/// One statement, tokenised, with where it came from.
///
/// Every parser in [`super::scan`] reads this shape, which is what keeps them
/// from each re-deriving `kw`/`at` closures over their own copies of the tokens.
pub(super) struct Statement<'a> {
    pub(super) file: &'a str,
    pub(super) line: usize,
    pub(super) text: &'a str,
    pub(super) tokens: Vec<String>,
    pub(super) upper: Vec<String>,
}

impl<'a> Statement<'a> {
    pub(super) fn new(file: &'a str, line: usize, text: &'a str) -> Self {
        let tokens = tokens(text);
        let upper = tokens.iter().map(|token| token.to_uppercase()).collect();
        Self {
            file,
            line,
            text,
            tokens,
            upper,
        }
    }

    /// The uppercased token at `index`, or `""` past the end — so a parser can
    /// look ahead without bounds-checking every step.
    pub(super) fn kw(&self, index: usize) -> &str {
        self.upper
            .get(index)
            .map(String::as_str)
            .unwrap_or_default()
    }

    /// The token at `index` as written, or `""` past the end.
    pub(super) fn at(&self, index: usize) -> &str {
        self.tokens
            .get(index)
            .map(String::as_str)
            .unwrap_or_default()
    }

    /// `file:line`, for error messages.
    pub(super) fn here(&self) -> String {
        format!("{}:{}", self.file, self.line)
    }

    /// The statement's *code*, uppercased, with quoted text left out:
    /// `COMMENT ON … IS 'use CREATE INDEX here'` must not be counted as an index
    /// statement and then refused for not decomposing into one.
    pub(super) fn code(&self) -> String {
        self.upper
            .iter()
            .filter(|token| !token.starts_with('\'') && !token.starts_with('"'))
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join(" ")
    }
}
