//! Grafana SQL **macro** translation (viz grafana-dashboard-fidelity scope, slice 1). A Grafana SQL
//! target carries the datasource plugin's macro dialect (`$__time`, `$__timeFilter`, …) which means
//! nothing to `federation.query`; the review measured that an untranslated `$__timeFilter` leaves the
//! scan **unbounded** and a real `histories` query then **hit the 30 s bound and was cancelled** — a
//! "mapped" chart that can never draw. This file is the one bounded DIALECT map that turns those macros
//! into the host's `$__from`/`$__to` window idiom (dashboard-time-range tokens: epoch **ms**, end-day
//! **exclusive**).
//!
//! **It is a dialect map, NOT a SQL bug-fixer** (scope non-goal). We translate the KNOWN macro set and
//! leave anything unrecognized **verbatim + REPORTED** (`unsupported macro $__foo`) — never silently
//! rewritten (a repaired-behind-your-back query is a worse surprise than a named degrade). `$__from`/
//! `$__to` are the HOST's own tokens (and Grafana's built-in epoch-ms variables — same semantics), so
//! they pass through untouched and are never reported.
//!
//! Rule 10: the map keys off the macro NAME, never a datasource or extension id — one dialect table
//! covers every SQL source.

/// The outcome of translating one SQL string: the rewritten SQL plus the names of any `$__` macros we
/// did not recognize (deduped, in first-seen order) so the verb can report each as a degrade.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Translated {
    pub sql: String,
    pub unsupported: Vec<String>,
}

/// Translate the Grafana SQL macro dialect to the host window idiom. Bounded map:
/// - `$__time(col)`        → `col AS "time"`
/// - `$__timeEpoch(col)`   → `extract(epoch from col) AS "time"`
/// - `$__timeFilter(col)`  → `col >= to_timestamp($__from / 1000.0) AND col < to_timestamp($__to / 1000.0 + 86400)`
/// - `$__timeFrom()`       → `to_timestamp($__from / 1000.0)`
/// - `$__timeTo()`         → `to_timestamp($__to / 1000.0)`
/// - `$__timeGroup(col,'5m')` → `to_timestamp(floor(extract(epoch from col) / 300) * 300)`
///
/// A `$__timeGroup` whose interval is not a literal duration (e.g. `$__interval`), or any other `$__foo`,
/// is left VERBATIM and its name recorded in `unsupported`.
pub fn translate_sql(sql: &str) -> Translated {
    let bytes = sql.as_bytes();
    let mut out = String::with_capacity(sql.len() + 64);
    let mut unsupported: Vec<String> = Vec::new();
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] == b'$' && sql[i..].starts_with("$__") {
            let name_start = i + 3;
            let name_end = name_start
                + sql[name_start..]
                    .find(|c: char| !c.is_ascii_alphanumeric() && c != '_')
                    .unwrap_or(sql.len() - name_start);
            let name = &sql[name_start..name_end];
            // `$__from`/`$__to` are host tokens (and Grafana's own epoch-ms vars) — pass through.
            if name == "from" || name == "to" {
                out.push_str(&sql[i..name_end]);
                i = name_end;
                continue;
            }
            // A macro CALL has a balanced `(...)` right after the name (spaces allowed).
            let after = skip_ws(sql, name_end);
            if after < bytes.len() && bytes[after] == b'(' {
                if let Some((args_str, close)) = read_parens(sql, after) {
                    let args = split_top_level_commas(&args_str);
                    match translate_call(name, &args) {
                        Some(rep) => out.push_str(&rep),
                        None => {
                            record_unsupported(&mut unsupported, name);
                            out.push_str(&sql[i..close]); // verbatim, incl. the parens
                        }
                    }
                    i = close;
                    continue;
                }
            }
            // A bare `$__foo` with no call form we know — leave it and report it.
            record_unsupported(&mut unsupported, name);
            out.push_str(&sql[i..name_end]);
            i = name_end;
            continue;
        }
        // Copy one UTF-8 char (indices are byte offsets; step by the char's byte length).
        let ch = sql[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    Translated {
        sql: out,
        unsupported,
    }
}

/// Translate one recognized macro call; `None` = we don't handle this name/arity (leave verbatim).
fn translate_call(name: &str, args: &[String]) -> Option<String> {
    match (name, args.len()) {
        ("time", 1) => Some(format!("{} AS \"time\"", args[0].trim())),
        ("timeEpoch", 1) => Some(format!(
            "extract(epoch from {}) AS \"time\"",
            args[0].trim()
        )),
        ("timeFilter", 1) => {
            let col = args[0].trim();
            Some(format!(
                "{col} >= to_timestamp($__from / 1000.0) AND {col} < to_timestamp($__to / 1000.0 + 86400)"
            ))
        }
        ("timeFrom", 0) => Some("to_timestamp($__from / 1000.0)".to_string()),
        ("timeTo", 0) => Some("to_timestamp($__to / 1000.0)".to_string()),
        ("timeGroup" | "timeGroupAlias", n) if n >= 2 => {
            let secs = duration_secs(args[1].trim())?; // non-literal interval → leave verbatim + report
            let col = args[0].trim();
            let expr = format!("to_timestamp(floor(extract(epoch from {col}) / {secs}) * {secs})");
            Some(if name == "timeGroupAlias" {
                format!("{expr} AS \"time\"")
            } else {
                expr
            })
        }
        _ => None,
    }
}

/// Parse a Grafana duration literal (`'5m'`, `"1h"`, `30s`, `2d`) to whole seconds. Anything else
/// (a variable like `$__interval`, an unknown unit) → `None` so the caller degrades honestly.
fn duration_secs(raw: &str) -> Option<u64> {
    let s = raw.trim().trim_matches(|c| c == '\'' || c == '"').trim();
    if s.is_empty() {
        return None;
    }
    let (num, unit) = s.split_at(s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len()));
    let n: u64 = num.parse().ok()?;
    let mult = match unit.trim() {
        "s" => 1,
        "m" => 60,
        "h" => 3_600,
        "d" => 86_400,
        _ => return None,
    };
    Some(n.checked_mul(mult)?)
}

/// Record an unsupported macro name once (first-seen order), as `$__name` for the report sentence.
fn record_unsupported(list: &mut Vec<String>, name: &str) {
    let token = format!("$__{name}");
    if !list.contains(&token) {
        list.push(token);
    }
}

fn skip_ws(s: &str, mut i: usize) -> usize {
    let b = s.as_bytes();
    while i < b.len() && (b[i] as char).is_whitespace() {
        i += 1;
    }
    i
}

/// Read a balanced `(...)` starting at `open` (which must index a `(`). Returns the INNER text and the
/// index just past the closing `)`. `None` if unbalanced (run off the end).
fn read_parens(s: &str, open: usize) -> Option<(String, usize)> {
    let b = s.as_bytes();
    debug_assert_eq!(b[open], b'(');
    let mut depth = 0i32;
    let mut i = open;
    let mut inner_start = open + 1;
    while i < b.len() {
        match b[i] {
            b'(' => {
                depth += 1;
                if depth == 1 {
                    inner_start = i + 1;
                }
            }
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return Some((s[inner_start..i].to_string(), i + 1));
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

/// Split an argument list on TOP-LEVEL commas (ignoring commas inside nested parens or quotes).
fn split_top_level_commas(args: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut quote: Option<char> = None;
    let mut cur = String::new();
    for ch in args.chars() {
        match quote {
            Some(q) => {
                cur.push(ch);
                if ch == q {
                    quote = None;
                }
            }
            None => match ch {
                '\'' | '"' => {
                    quote = Some(ch);
                    cur.push(ch);
                }
                '(' => {
                    depth += 1;
                    cur.push(ch);
                }
                ')' => {
                    depth -= 1;
                    cur.push(ch);
                }
                ',' if depth == 0 => {
                    out.push(cur.trim().to_string());
                    cur = String::new();
                }
                _ => cur.push(ch),
            },
        }
    }
    if !cur.trim().is_empty() || !out.is_empty() {
        out.push(cur.trim().to_string());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn time_and_time_filter_translate_to_the_host_window() {
        let t =
            translate_sql("SELECT $__time(ts), value FROM h WHERE $__timeFilter(ts) ORDER BY ts");
        assert_eq!(
            t.sql,
            "SELECT ts AS \"time\", value FROM h WHERE ts >= to_timestamp($__from / 1000.0) AND ts < to_timestamp($__to / 1000.0 + 86400) ORDER BY ts"
        );
        assert!(t.unsupported.is_empty());
        // the bounded window references the host picker tokens (drives dateSelect on)
        assert!(t.sql.contains("$__from"));
    }

    #[test]
    fn time_from_to_and_epoch() {
        let t = translate_sql("$__timeFrom() .. $__timeTo() .. $__timeEpoch(ts)");
        assert_eq!(
            t.sql,
            "to_timestamp($__from / 1000.0) .. to_timestamp($__to / 1000.0) .. extract(epoch from ts) AS \"time\""
        );
    }

    #[test]
    fn time_group_with_a_literal_interval() {
        let t = translate_sql("$__timeGroup(histories.timestamp, '5m')");
        assert_eq!(
            t.sql,
            "to_timestamp(floor(extract(epoch from histories.timestamp) / 300) * 300)"
        );
        assert!(t.unsupported.is_empty());
    }

    #[test]
    fn time_group_with_a_nonliteral_interval_is_left_verbatim_and_reported() {
        let t = translate_sql("$__timeGroup(ts, $__interval)");
        // the whole macro rides verbatim (we can't resolve the interval) and the outer macro is named
        assert_eq!(t.sql, "$__timeGroup(ts, $__interval)");
        assert_eq!(t.unsupported, vec!["$__timeGroup".to_string()]);
    }

    #[test]
    fn unknown_macro_left_verbatim_and_reported_once() {
        let t = translate_sql("SELECT $__unixEpochFilter(ts) AND $__unixEpochFilter(ts)");
        assert_eq!(
            t.sql,
            "SELECT $__unixEpochFilter(ts) AND $__unixEpochFilter(ts)"
        );
        assert_eq!(t.unsupported, vec!["$__unixEpochFilter".to_string()]);
    }

    #[test]
    fn host_from_to_tokens_pass_through_and_are_never_reported() {
        let t = translate_sql("WHERE ts BETWEEN $__from AND $__to");
        assert_eq!(t.sql, "WHERE ts BETWEEN $__from AND $__to");
        assert!(t.unsupported.is_empty());
    }

    #[test]
    fn no_macros_is_identity() {
        let s = "SELECT value, timestamp FROM histories ORDER BY timestamp DESC LIMIT 1";
        assert_eq!(translate_sql(s).sql, s);
    }

    #[test]
    fn nested_parens_in_a_column_expression_are_balanced() {
        let t = translate_sql("$__time(date_trunc('hour', ts))");
        assert_eq!(t.sql, "date_trunc('hour', ts) AS \"time\"");
    }
}
