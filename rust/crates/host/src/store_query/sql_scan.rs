//! Reading SurrealQL as TEXT — the lexical half of the secret-plane wall.
//!
//! SurrealDB 3 sealed its AST (`parse.rs` records the exhaustive check), so the wall in
//! `secret_wall.rs` has to find table references in the statement text. That is a scanning problem,
//! kept here so the wall next door stays a statement of policy: what is refused, and why that is
//! sound. Nothing in this file decides anything — it reports what the text says.
//!
//! Every function is deliberately conservative. When the text cannot be read with certainty the
//! answer is "not provable", and the caller refuses; a wrong "provable" would be a credential leak,
//! a wrong "not provable" is a visible false refusal.

use super::secret_wall::Vars;

/// SurrealQL functions that build a table or record name from a value. A token scan cannot see the
/// resulting name, so each call's first argument is resolved separately ([`dynamic_table_args`]).
///
/// `type::record` is the one that matters most and was the one initially missed: SurrealDB 3
/// renamed 2.x's `type::thing` to it, lb uses it in 167 places, and a wall watching only the old
/// name let `FROM type::record('secret', $id)` through untouched. `type::thing` is kept because
/// costing nothing is cheaper than reasoning about whether an old literal survives somewhere.
const DYNAMIC_TABLE_FNS: &[&str] = &["type::table", "type::record", "type::thing"];

/// The first argument of every `type::table(…)` / `type::thing(…)` call in `sql`, as written.
///
/// Nested parentheses are tracked so an argument that is itself a call comes back whole and is
/// judged unresolvable, rather than being truncated into something that looks resolvable.
pub(super) fn dynamic_table_args(sql: &str) -> Vec<String> {
    let lowered = sql.to_ascii_lowercase();
    let mut out = Vec::new();
    for f in DYNAMIC_TABLE_FNS {
        let mut from = 0;
        while let Some(hit) = lowered[from..].find(f) {
            let open = from + hit + f.len();
            from = open;
            let rest = &sql[open..];
            let Some(body) = rest.strip_prefix('(') else {
                continue;
            };
            let mut depth = 1usize;
            let mut end = body.len();
            for (i, c) in body.char_indices() {
                match c {
                    '(' => depth += 1,
                    ')' => {
                        depth -= 1;
                        if depth == 0 {
                            end = i;
                            break;
                        }
                    }
                    ',' if depth == 1 => {
                        end = i;
                        break;
                    }
                    _ => {}
                }
            }
            out.push(body[..end].trim().to_string());
        }
    }
    out
}

/// The literal table name `arg` will take, or `None` when that cannot be proved.
///
/// Resolvable: a quoted string (`'site'`), and a `$param` bound to a string. Everything else — a
/// field reference, an expression, a parameter with no binding — is unprovable and refused by the
/// caller. Refusing the unprovable case is the point: an unchecked table position is exactly the
/// bypass this wall exists to close.
pub(super) fn resolve_arg(arg: &str, vars: Vars<'_>) -> Option<String> {
    let a = arg.trim();
    if let Some(name) = a.strip_prefix('$') {
        if !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            return None;
        }
        return vars
            .iter()
            .find(|(k, _)| k == name)
            .and_then(|(_, v)| v.as_str())
            .map(str::to_string);
    }
    for q in ['\'', '"'] {
        if let Some(inner) = a.strip_prefix(q).and_then(|r| r.strip_suffix(q)) {
            // A quote inside would mean the literal ended earlier than it appears to.
            if !inner.contains(q) {
                return Some(inner.to_string());
            }
        }
    }
    None
}

/// The term directly after each top-level `FROM`, as written.
///
/// Only that one term: a field reference anywhere else (a projection, a `WHERE`, an `ORDER BY`)
/// names no table, and treating one as a table position is what made a composed subquery refuse
/// itself. `FROM` inside a quoted string is not a keyword and is skipped with the string.
pub(super) fn from_terms(sql: &str) -> Vec<String> {
    let b = sql.as_bytes();
    let mut out = Vec::new();
    let mut word = String::new();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'"' || b[i] == b'\'' {
            let quote = b[i];
            i += 1;
            while i < b.len() {
                if b[i] == b'\\' && i + 1 < b.len() {
                    i += 2;
                    continue;
                }
                if b[i] == quote {
                    i += 1;
                    break;
                }
                i += 1;
            }
            word.clear();
            continue;
        }
        if b[i].is_ascii_alphanumeric() || b[i] == b'_' {
            word.push(b[i] as char);
            i += 1;
            continue;
        }
        if word.eq_ignore_ascii_case("from") {
            while i < b.len() && (b[i] as char).is_whitespace() {
                i += 1;
            }
            out.push(read_term(&b[i..]));
        }
        word.clear();
        i += 1;
    }
    out
}

/// One table term: up to the first whitespace or clause separator, with parentheses and the
/// bracket-quoted identifier form kept whole.
pub(super) fn read_term(b: &[u8]) -> String {
    let mut depth = 0usize;
    let mut end = b.len();
    for (i, c) in b.iter().enumerate() {
        match c {
            b'(' => depth += 1,
            b')' if depth > 0 => depth -= 1,
            b')' | b',' | b';' if depth == 0 => {
                end = i;
                break;
            }
            _ if depth == 0 && (*c as char).is_whitespace() => {
                end = i;
                break;
            }
            _ => {}
        }
    }
    String::from_utf8_lossy(&b[..end]).trim().to_string()
}

/// Can this table term be reduced to a name we can check?
///
/// Provable: a subquery (scanned on its own), a `type::…` call (resolved by [`dynamic_table_args`]),
/// a plain identifier or record literal (caught by the token scan), and a `$param` bound to a
/// string. A dotted path is a value read at run time, and is not.
pub(super) fn table_term_is_provable(term: &str, vars: Vars<'_>) -> bool {
    let t = term.trim();
    if t.is_empty() || t.starts_with('(') {
        return true;
    }
    let lower = t.to_ascii_lowercase();
    if lower.starts_with("type::") {
        // Only the constructors we actually resolve are provable. Any OTHER `type::` call in a
        // table position is a value computed at run time, and waving it through because of its
        // prefix would be the same bypass this function exists to close.
        return DYNAMIC_TABLE_FNS.iter().any(|f| lower.starts_with(f));
    }
    if t.starts_with('$') {
        return resolve_arg(t, vars).is_some();
    }
    // Strip the quoting forms that wrap an ordinary identifier, then a dot means a field path.
    let bare = t
        .trim_matches('`')
        .trim_start_matches('⟨')
        .trim_end_matches('⟩');
    !bare.contains('.')
}

/// Remove `--` / `#` line comments and `/* … */` blocks, so a name cannot be split across one.
pub(super) fn strip_comments(sql: &str) -> String {
    let b = sql.as_bytes();
    let mut out = String::with_capacity(sql.len());
    let mut i = 0;
    while i < b.len() {
        // A quoted string is copied verbatim; `identifiers` decides what to do with its contents.
        if b[i] == b'"' || b[i] == b'\'' {
            let quote = b[i];
            out.push(b[i] as char);
            i += 1;
            while i < b.len() {
                if b[i] == b'\\' && i + 1 < b.len() {
                    out.push(b[i] as char);
                    out.push(b[i + 1] as char);
                    i += 2;
                    continue;
                }
                out.push(b[i] as char);
                i += 1;
                if b[i - 1] == quote {
                    break;
                }
            }
            continue;
        }
        if b[i] == b'#' || (b[i] == b'-' && i + 1 < b.len() && b[i + 1] == b'-') {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        if b[i] == b'/' && i + 1 < b.len() && b[i + 1] == b'*' {
            i += 2;
            while i + 1 < b.len() && !(b[i] == b'*' && b[i + 1] == b'/') {
                i += 1;
            }
            i = (i + 2).min(b.len());
            out.push(' ');
            continue;
        }
        out.push(b[i] as char);
        i += 1;
    }
    out
}

/// Every identifier-shaped token in `sql`, in order.
///
/// The contents of an ordinary quoted string are NOT identifiers — refusing `WHERE note = 'my
/// apikey'` would be a false refusal with no security value, because a string cannot become a table
/// without [`DYNAMIC_TABLE_FNS`], whose arguments are resolved separately. A **record string**
/// (`r"secret:abc"`)
/// is the exception: it *is* a record reference, so its contents are scanned.
///
/// Backtick- and angle-quoted identifiers (`` `secret` ``, `⟨secret⟩`) are just delimiters around
/// ordinary identifier characters, so they need no special handling — the name inside is tokenized
/// like any other.
pub(super) fn identifiers(sql: &str) -> Vec<String> {
    let b = sql.as_bytes();
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut i = 0;
    while i < b.len() {
        // `r"…"` / `r'…'` — a RECORD literal, not an ordinary string: it names a table, so its
        // contents are tokenized rather than skipped.
        if (b[i] == b'r' || b[i] == b'R')
            && i + 1 < b.len()
            && (b[i + 1] == b'"' || b[i + 1] == b'\'')
            && cur.is_empty()
        {
            let quote = b[i + 1];
            i += 2;
            while i < b.len() && b[i] != quote {
                if b[i].is_ascii_alphanumeric() || b[i] == b'_' {
                    cur.push(b[i] as char);
                } else if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
                i += 1;
            }
            if !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
            i += 1; // past the closing quote
            continue;
        }
        if b[i] == b'"' || b[i] == b'\'' {
            if !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
            let quote = b[i];
            i += 1;
            while i < b.len() {
                if b[i] == b'\\' && i + 1 < b.len() {
                    i += 2;
                    continue;
                }
                if b[i] == quote {
                    i += 1;
                    break;
                }
                i += 1;
            }
            continue;
        }
        if b[i].is_ascii_alphanumeric() || b[i] == b'_' {
            cur.push(b[i] as char);
        } else if !cur.is_empty() {
            out.push(std::mem::take(&mut cur));
        }
        i += 1;
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}
