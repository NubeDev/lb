//! `sql_macros` — the ONE Grafana-compatible SQL time-macro layer, expanded at QUERY TIME per
//! source `kind` (viz sql-time-macros scope). The host stays zero-parse: `viz.query` substitutes the
//! VALUE macros (`$__interval`, bare `$__timeFrom`/`$__timeTo`) and attaches the derived window as an
//! additive `resolution: {from_ms, to_ms, width_ms}`; this module expands the FUNCTION macros —
//! `$__timeFilter(col)`, `$__timeGroup(col,'<dur>')`, `$__timeGroupAlias`, `$__time(col)`,
//! `$__timeFrom()`, `$__timeTo()`, and `$__timeTable(...)` (a table-tier selector, engine-agnostic) —
//! into the dialect of the engine that will actually execute.
//!
//! The engine table is keyed on this child's OWN `kind` vocabulary (`source::connect`) — the mediated
//! seam, no engine name in core (rule 10). Expansion-by-kind IS the executing dialect: every
//! non-`information_schema` query runs the direct path against the source (`validate.rs::is_simple`),
//! and a macro'd query never probes the catalog.
//!
//! **Timestamp-column assumption per engine (v1, documented):** postgres/timescale — a native
//! timestamp(tz) column; mysql — a native DATETIME/TIMESTAMP column; sqlite — epoch **milliseconds**
//! as INTEGER (SQLite has no timestamp type; epoch-ms is the house convention — series export, the
//! demo sources). Epoch-integer columns on the other engines are the additive epoch-macro follow-up.
//!
//! **Honesty contract:** an unsupported `$__` token is a NAMED error, never silent breakage; a time
//! macro with no window names the missing `resolution` field; an un-macro'd SQL is byte-identical.
//! Pure textual scan (balanced parens, quote-aware comma split) — never a SQL parse.

use serde_json::Value;

/// The render window `viz.query` attaches as the additive `resolution` arg. Absent on direct calls.
#[derive(Debug, Clone, Copy)]
struct Window {
    from_ms: u64,
    to_ms: u64,
    width_ms: u64,
}

/// The engines this child can expand for — its own `kind` vocabulary (`source::connect` + scope).
#[derive(Debug, Clone, Copy, PartialEq)]
enum Dialect {
    Postgres,
    Timescale,
    Sqlite,
    Mysql,
}

/// Expand every `$__name(args…)` in `sql` for `kind`. No `$__` token → `sql` back byte-identical.
pub fn expand(sql: &str, kind: &str, resolution: Option<&Value>) -> Result<String, String> {
    if !sql.contains("$__") {
        return Ok(sql.to_string());
    }
    let dialect = dialect_for(kind)?;
    let window = parse_window(resolution);
    scan(sql, dialect, window)
}

fn dialect_for(kind: &str) -> Result<Dialect, String> {
    match kind {
        "postgres" => Ok(Dialect::Postgres),
        "timescale" => Ok(Dialect::Timescale),
        "sqlite" => Ok(Dialect::Sqlite),
        "mysql" => Ok(Dialect::Mysql),
        other => Err(format!(
            "no SQL time-macro expansion for source kind \"{other}\""
        )),
    }
}

fn parse_window(resolution: Option<&Value>) -> Option<Window> {
    let r = resolution?;
    Some(Window {
        from_ms: r.get("from_ms")?.as_u64()?,
        to_ms: r.get("to_ms")?.as_u64()?,
        width_ms: r.get("width_ms")?.as_u64()?,
    })
}

/// A time macro that needs the window but has none — name the missing field and the fix.
fn missing_window(name: &str) -> String {
    format!(
        "time macro $__{name} needs the render window, and no \"resolution\" \
         {{from_ms, to_ms, width_ms}} was on the call — dispatch through viz.query with a numeric \
         from/to on the target, or pass \"resolution\" on federation.query explicitly"
    )
}

/// The byte scanner — the discipline the retired import translator proved: `$__` detect, name scan,
/// balanced-paren args, top-level comma split. Unknown token → named error (never silent rewrite).
fn scan(sql: &str, dialect: Dialect, window: Option<Window>) -> Result<String, String> {
    let bytes = sql.as_bytes();
    let mut out = String::with_capacity(sql.len() + 64);
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] == b'$' && sql[i..].starts_with("$__") {
            let name_start = i + 3;
            let name_end = name_start
                + sql[name_start..]
                    .find(|c: char| !c.is_ascii_alphanumeric() && c != '_')
                    .unwrap_or(sql.len() - name_start);
            let name = &sql[name_start..name_end];
            let after = skip_ws(sql, name_end);
            if after < bytes.len() && bytes[after] == b'(' {
                if let Some((args_str, close)) = read_parens(sql, after) {
                    let args = split_top_level_commas(&args_str);
                    out.push_str(&expand_call(name, &args, dialect, window)?);
                    i = close;
                    continue;
                }
            }
            // A bare `$__foo` (no call form). The value macros are substituted by viz.query BEFORE
            // dispatch — one reaching this child means nobody resolved it; executing it would be a
            // cryptic engine error, so fail with the name and the fix instead.
            return Err(match name {
                "interval" | "interval_ms" | "timeFrom" | "timeTo" => format!(
                    "unexpanded value macro $__{name} — value macros are substituted by viz.query \
                     before dispatch; call through viz.query or inline the literal value"
                ),
                _ => format!("unsupported macro $__{name}"),
            });
        }
        let ch = sql[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    Ok(out)
}

/// One macro call → the engine expression. The v1 set is exactly Grafana's common SQL macros; a call
/// outside it (or a malformed one) is a named error.
fn expand_call(
    name: &str,
    args: &[String],
    dialect: Dialect,
    window: Option<Window>,
) -> Result<String, String> {
    let need_window = |n: &str| window.ok_or_else(|| missing_window(n));
    match (name, args.len()) {
        ("time", 1) => Ok(format!("{} AS \"time\"", args[0].trim())),
        ("timeFilter", 1) => {
            let w = need_window("timeFilter")?;
            Ok(time_filter(dialect, args[0].trim(), w.from_ms, w.to_ms))
        }
        // `$__timeFrom()` / `$__timeTo()` — the FUNCTION forms (Grafana-verbatim). The bare tokens
        // of the same name are the host's value macros; the value pass skips call forms.
        ("timeFrom", 0) => Ok(bound(dialect, need_window("timeFrom")?.from_ms)),
        ("timeTo", 0) => Ok(bound(dialect, need_window("timeTo")?.to_ms)),
        ("timeGroup" | "timeGroupAlias", 2) => {
            let ms = interval_ms(name, args[1].trim(), window)?;
            let expr = time_group(dialect, args[0].trim(), ms);
            Ok(if name == "timeGroupAlias" {
                format!("{expr} AS \"time\"")
            } else {
                expr
            })
        }
        // `$__timeTable('raw', 'hourly:1h', 'daily:1d', 'monthly:1M', 'yearly:1y')` — the table-tier
        // selector. Variadic, ordered FINEST → COARSEST; each arg is a table name, optionally tagged
        // `:width` with its native resolution (a bare name = width 0 = the finest tier, always a
        // candidate). Expands to the literal table name of the coarsest tier that still resolves at
        // least as fine as the derived width (walk coarsest → finest, first with native ≤ width), or
        // the coarsest given if none qualifies. Engine-agnostic — the same name for every dialect.
        ("timeTable", n) if n >= 1 => {
            let w = need_window("timeTable")?;
            Ok(select_time_table(args, w.width_ms)?)
        }
        ("timeTable", 0) => Err("wrong argument count for $__timeTable (0 given)".to_string()),
        // Grafana's 3-arg form carries a gap-FILL mode (NULL/previous/0) — that inserts rows the
        // query never returned, which this textual layer cannot honestly do. Named, fixable.
        ("timeGroup" | "timeGroupAlias", 3) => Err(format!(
            "the fill argument of $__{name} is not supported — drop it \
             ($__{name}({}, {})) and handle gaps client-side",
            args[0].trim(),
            args[1].trim()
        )),
        ("time" | "timeFilter" | "timeGroup" | "timeGroupAlias" | "timeFrom" | "timeTo", n) => {
            Err(format!("wrong argument count for $__{name} ({n} given)"))
        }
        _ => Err(format!("unsupported macro $__{name}")),
    }
}

/// A `$__timeTable` argument: a literal table name + its native bucket width in ms. A bare name (no
/// `:width` tag) is native width 0 — the finest tier, always a candidate for selection.
struct TableTier {
    name: String,
    native_ms: u64,
}

/// Select the table tier for the derived width. `args` arrive ordered FINEST → COARSEST (the
/// documented syntax); selection walks COARSEST → FINEST and returns the first tier whose native
/// width is ≤ the derived width — the coarsest table that still resolves at least as fine as the
/// chart needs (least data scanned without losing resolution). If none qualifies, falls back to the
/// coarsest given (max native width). `raw_data` (width 0) always qualifies, so a fine enough
/// derived width selects it last. Mirrors lb-internal rollup "governing tier" selection, exposed to
/// hand-authored federation SQL.
fn select_time_table(args: &[String], derived_ms: u64) -> Result<String, String> {
    let mut tiers: Vec<TableTier> = Vec::with_capacity(args.len());
    for raw in args {
        tiers.push(parse_time_table_arg(raw)?);
    }
    for tier in tiers.iter().rev() {
        if tier.native_ms <= derived_ms {
            return Ok(tier.name.clone());
        }
    }
    tiers
        .iter()
        .max_by_key(|t| t.native_ms)
        .map(|t| t.name.clone())
        .ok_or_else(|| "internal: empty $__timeTable".to_string())
}

/// Parse one `$__timeTable` arg — handles `'name:width'` or a bare `name` (width 0). Quote
/// chars (single or double) are stripped from the whole arg, matching the documented quoted form.
fn parse_time_table_arg(raw: &str) -> Result<TableTier, String> {
    let s = raw.trim().trim_matches(|c| c == '\'' || c == '"').trim();
    if s.is_empty() {
        return Err("bad argument in $__timeTable — expected 'table' or 'table:width'".to_string());
    }
    let (name, tag) = match s.rfind(':') {
        Some(i) => (&s[..i], &s[i + 1..]),
        None => (s, "0"),
    };
    let name = name.trim();
    if name.is_empty() {
        return Err(
            "bad table name in $__timeTable — expected 'table' or 'table:width'".to_string(),
        );
    }
    let native_ms = parse_tier_width(tag)?;
    Ok(TableTier {
        name: name.to_string(),
        native_ms,
    })
}

/// Parse a `$__timeTable` `:width` tag → ms. Accepts the Grafana duration forms (incl. `w`, `M`=30d,
/// `y`=365d) and a bare integer = ms; `0`/absent = native (finest) width, deliberately allowed.
fn parse_tier_width(tag: &str) -> Result<u64, String> {
    let t = tag.trim().trim_matches(|c| c == '\'' || c == '"').trim();
    let split = t.find(|c: char| !c.is_ascii_digit()).unwrap_or(t.len());
    let (num, unit) = (&t[..split], t[split..].trim());
    let n: u64 = num.parse().map_err(|_| bad_tier_width(tag))?;
    let mult_ms = match unit {
        "" | "ms" => 1,
        "s" => 1_000,
        "m" => 60_000,
        "h" => 3_600_000,
        "d" => 86_400_000,
        "w" => 604_800_000,
        "M" => 2_592_000_000,
        "y" => 31_536_000_000,
        _ => return Err(bad_tier_width(tag)),
    };
    n.checked_mul(mult_ms).ok_or_else(|| bad_tier_width(tag))
}

fn bad_tier_width(tag: &str) -> String {
    format!(
        "bad :width tag '{tag}' in $__timeTable — expected a Grafana duration like '1h', '1d', '1M', '1y'"
    )
}

/// A `$__timeGroup` interval argument → milliseconds. Accepts the literal Grafana duration forms
/// (`'5m'`, `"30s"`, `1500ms`, a bare integer = ms) and `'$__interval'` (resolved from the window —
/// what the value pass leaves for a DIRECT caller that skipped viz.query).
fn interval_ms(macro_name: &str, raw: &str, window: Option<Window>) -> Result<u64, String> {
    let s = raw.trim().trim_matches(|c| c == '\'' || c == '"').trim();
    if s == "$__interval" {
        return Ok(window.ok_or_else(|| missing_window(macro_name))?.width_ms);
    }
    let (num, unit) = s.split_at(s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len()));
    let n: u64 = num.parse().map_err(|_| bad_interval(macro_name, raw))?;
    let mult_ms = match unit.trim() {
        "ms" | "" => 1,
        "s" => 1_000,
        "m" => 60_000,
        "h" => 3_600_000,
        "d" => 86_400_000,
        "w" => 604_800_000,
        "y" => 31_536_000_000,
        _ => return Err(bad_interval(macro_name, raw)),
    };
    let ms = n
        .checked_mul(mult_ms)
        .ok_or_else(|| bad_interval(macro_name, raw))?;
    if ms == 0 {
        return Err(bad_interval(macro_name, raw));
    }
    Ok(ms)
}

fn bad_interval(macro_name: &str, raw: &str) -> String {
    format!("bad interval {raw} in $__{macro_name} — expected a literal like '30s', '5m', '1h'")
}

// ---- the per-engine expansion table (the reason this layer lives in the child) ----

/// The bucketing expression: floor `col` to `ms`-wide buckets in the engine's own idiom.
fn time_group(dialect: Dialect, col: &str, ms: u64) -> String {
    match dialect {
        Dialect::Postgres => {
            format!("date_bin(INTERVAL '{ms} milliseconds', {col}, TIMESTAMPTZ '1970-01-01')")
        }
        Dialect::Timescale => format!("time_bucket(INTERVAL '{ms} milliseconds', {col})"),
        // Epoch-ms INTEGER column: integer division floors.
        Dialect::Sqlite => format!("((({col}) / {ms}) * {ms})"),
        Dialect::Mysql => {
            let s = (ms / 1000).max(1);
            format!("FROM_UNIXTIME(FLOOR(UNIX_TIMESTAMP({col}) / {s}) * {s})")
        }
    }
}

/// The half-open range predicate `from <= col < to` in the engine's own comparison form.
fn time_filter(dialect: Dialect, col: &str, from_ms: u64, to_ms: u64) -> String {
    format!(
        "{col} >= {} AND {col} < {}",
        bound(dialect, from_ms),
        bound(dialect, to_ms)
    )
}

/// One window bound as an engine timestamp literal (`$__timeFrom()`/`$__timeTo()` + the filter).
fn bound(dialect: Dialect, ms: u64) -> String {
    match dialect {
        Dialect::Postgres | Dialect::Timescale => format!("to_timestamp({ms} / 1000.0)"),
        Dialect::Sqlite => format!("{ms}"),
        Dialect::Mysql => format!("FROM_UNIXTIME({ms} / 1000.0)"),
    }
}

// ---- the scanner primitives (ported verbatim from the retired import translator) ----

fn skip_ws(s: &str, mut i: usize) -> usize {
    let b = s.as_bytes();
    while i < b.len() && (b[i] as char).is_whitespace() {
        i += 1;
    }
    i
}

/// Read a balanced `(...)` starting at `open`; returns (inner, index-past-close).
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

/// Split on top-level commas, honoring nested parens and quotes.
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
    use serde_json::json;

    fn res() -> Value {
        json!({"from_ms": 1_000, "to_ms": 601_000, "width_ms": 300_000})
    }

    fn res_w(width_ms: u64) -> Value {
        json!({"from_ms": 1_000, "to_ms": 601_000, "width_ms": width_ms})
    }

    /// `$__timeTable` expands to the table name only when a `resolution.width_ms` is attached — a
    /// direct call with none names the missing field (same contract as the rest of the window set).
    #[test]
    fn time_table_needs_the_window() {
        let sql = "SELECT v FROM $__timeTable('raw','hourly:1h')";
        let e = expand(sql, "sqlite", None).unwrap_err();
        assert!(
            e.contains("resolution") && e.contains("viz.query") && e.contains("$__timeTable"),
            "{e}"
        );
    }

    /// Selection across every tier: the coarsest tier with native width ≤ derived wins; below the
    /// finest tier, `raw` (width 0) always qualifies. Asserted over the FULL ladder of derived
    /// widths using the issue's canonical 5-tier example.
    #[test]
    fn time_table_selects_tier_by_derived_width() {
        let sql = "SELECT v FROM $__timeTable('raw_data','hourly_data:1h','daily_data:1d',\
                   'monthly_data:1M','yearly_data:1y') WHERE $__timeFilter(ts)";
        for (width, expect) in [
            (1_000, "raw_data"),             // finer than 1h → finest tier (width 0)
            (3_600_000, "hourly_data"),      // exactly 1h
            (43_200_000, "hourly_data"),     // 12h → still fine enough = hourly
            (86_400_000, "daily_data"),      // 1d
            (2_592_000_000, "monthly_data"), // 30d
            (31_536_000_000, "yearly_data"), // 365d
        ] {
            let out = expand(sql, "sqlite", Some(&res_w(width))).unwrap();
            assert!(
                out.contains(&format!("FROM {expect}")),
                "width {width} → {expect}, got: {out}"
            );
            assert!(
                out.contains("ts >= "),
                "the neighbouring $__timeFilter(ts) still expands, got: {out}"
            );
        }
    }

    /// The one-arg case: only `raw_data` (width 0) → always the answer, any derived width.
    #[test]
    fn time_table_one_arg_is_always_raw() {
        for width in [1_000, 3_600_000, 86_400_000, 31_536_000_000] {
            let sql = "SELECT v FROM $__timeTable('raw_data') WHERE $__timeFilter(ts)";
            let out = expand(sql, "sqlite", Some(&res_w(width))).unwrap();
            assert!(out.contains("FROM raw_data"), "width {width}: {out}");
        }
    }

    /// No tier qualifies (coarser listed than the chart wants, and not `raw`) → fall back to the
    /// coarsest given (max native width). Engine-agnostic: identical on every dialect.
    #[test]
    fn time_table_falls_back_to_coarsest_when_none_qualify() {
        let sql = "SELECT v FROM $__timeTable('hourly_data:1h','daily_data:1d')";
        // derived 1s < both native widths → no tier qualifies → coarsest (daily) wins
        let out = expand(sql, "sqlite", Some(&res_w(1_000))).unwrap();
        assert!(out.contains("FROM daily_data"), "{out}");
        // same selection regardless of engine kind — the result is a literal table name
        for kind in ["postgres", "timescale", "mysql"] {
            let out = expand(sql, kind, Some(&res_w(1_000))).unwrap();
            assert!(out.contains("FROM daily_data"), "{kind}: {out}");
        }
    }

    /// Malformed and empty args are NAMED errors, not silent rewrites — the honesty contract.
    #[test]
    fn time_table_malformed_args_are_named_errors() {
        for (sql, want) in [
            (
                "SELECT v FROM $__timeTable('raw', 'hourly_data:xyz')",
                "bad :width tag 'xyz' in $__timeTable",
            ),
            (
                "SELECT v FROM $__timeTable()",
                "wrong argument count for $__timeTable (0 given)",
            ),
        ] {
            let e = expand(sql, "sqlite", Some(&res_w(86_400_000))).unwrap_err();
            assert!(e.contains(want), "{sql}: {e}");
        }
    }

    fn expand_ok(sql: &str, kind: &str) -> String {
        expand(sql, kind, Some(&res())).expect("expands")
    }

    /// Every engine × every macro in the v1 set — the expansion table, asserted whole.
    #[test]
    fn expansion_table_per_engine() {
        let sql = "SELECT $__time(ts), $__timeGroupAlias(ts, '5m'), max(v) FROM h \
                   WHERE $__timeFilter(ts) AND ts >= $__timeFrom() AND ts < $__timeTo() \
                   GROUP BY $__timeGroup(ts, '5m')";
        let pg = expand_ok(sql, "postgres");
        assert!(pg.contains("ts AS \"time\""));
        assert!(pg.contains(
            "date_bin(INTERVAL '300000 milliseconds', ts, TIMESTAMPTZ '1970-01-01') AS \"time\""
        ));
        assert!(
            pg.contains("ts >= to_timestamp(1000 / 1000.0) AND ts < to_timestamp(601000 / 1000.0)")
        );
        assert!(pg.contains(
            "GROUP BY date_bin(INTERVAL '300000 milliseconds', ts, TIMESTAMPTZ '1970-01-01')"
        ));

        let tsdb = expand_ok(sql, "timescale");
        assert!(tsdb.contains("time_bucket(INTERVAL '300000 milliseconds', ts) AS \"time\""));
        assert!(tsdb.contains("to_timestamp(1000 / 1000.0)"));

        let lite = expand_ok(sql, "sqlite");
        assert!(lite.contains("(((ts) / 300000) * 300000) AS \"time\""));
        assert!(lite.contains("ts >= 1000 AND ts < 601000"));
        assert!(lite.contains("GROUP BY (((ts) / 300000) * 300000)"));

        let my = expand_ok(sql, "mysql");
        assert!(my.contains("FROM_UNIXTIME(FLOOR(UNIX_TIMESTAMP(ts) / 300) * 300) AS \"time\""));
        assert!(my.contains("ts >= FROM_UNIXTIME(1000 / 1000.0)"));
    }

    /// Interval forms: quoted/unquoted, every unit, bare-int ms, and `'$__interval'` from the window.
    #[test]
    fn interval_forms() {
        for (raw, want_ms) in [
            ("'30s'", 30_000),
            ("\"5m\"", 300_000),
            ("1h", 3_600_000),
            ("'1d'", 86_400_000),
            ("'1500ms'", 1_500),
            ("250", 250),
            ("'$__interval'", 300_000), // resolved from the attached window
        ] {
            let sql = format!("SELECT $__timeGroup(t, {raw}) FROM h");
            let out = expand(&sql, "sqlite", Some(&res())).expect(raw);
            assert!(out.contains(&format!("* {want_ms})")), "{raw} → {out}");
        }
        let e = expand(
            "SELECT $__timeGroup(t, 'soon') FROM h",
            "sqlite",
            Some(&res()),
        )
        .unwrap_err();
        assert!(e.contains("bad interval 'soon' in $__timeGroup"), "{e}");
    }

    /// Nested parens in the column expression are captured whole (translator case, re-homed).
    #[test]
    fn nested_parens_in_a_column_expression() {
        let out = expand_ok("SELECT $__time(date_trunc('hour', ts)) FROM h", "postgres");
        assert_eq!(out, "SELECT date_trunc('hour', ts) AS \"time\" FROM h");
    }

    /// A time macro with no attached window → the error names the missing `resolution` field.
    #[test]
    fn missing_resolution_is_a_named_error() {
        for sql in [
            "SELECT v FROM h WHERE $__timeFilter(ts)",
            "SELECT $__timeFrom() FROM h",
            "SELECT $__timeGroup(ts, '$__interval') FROM h",
        ] {
            let e = expand(sql, "sqlite", None).unwrap_err();
            assert!(
                e.contains("resolution") && e.contains("viz.query"),
                "{sql}: {e}"
            );
        }
        // A literal interval is self-contained — no window needed.
        assert!(expand("SELECT $__timeGroup(ts, '5m') FROM h", "sqlite", None).is_ok());
    }

    /// Unsupported macros fail NAMED — call forms, epoch forms (translator cases, re-homed), and
    /// bare value tokens that nobody substituted.
    #[test]
    fn unsupported_and_unexpanded_macros_are_named_errors() {
        let e = expand(
            "SELECT $__unixEpochFilter(ts) FROM h",
            "sqlite",
            Some(&res()),
        )
        .unwrap_err();
        assert!(e.contains("unsupported macro $__unixEpochFilter"), "{e}");
        let e = expand("SELECT $__timeEpoch(ts) FROM h", "sqlite", Some(&res())).unwrap_err();
        assert!(e.contains("unsupported macro $__timeEpoch"), "{e}");
        let e = expand(
            "SELECT v FROM h WHERE t > $__timeFrom",
            "sqlite",
            Some(&res()),
        )
        .unwrap_err();
        assert!(e.contains("unexpanded value macro $__timeFrom"), "{e}");
        let e = expand(
            "SELECT $__timeGroup(ts, '5m', 0) FROM h",
            "sqlite",
            Some(&res()),
        )
        .unwrap_err();
        assert!(e.contains("fill argument"), "{e}");
    }

    /// Un-macro'd SQL is byte-identical — even for a kind with no expansion table.
    #[test]
    fn un_macrod_sql_is_byte_identical() {
        let sql = "SELECT t, v FROM readings WHERE v > 3 ORDER BY t -- $ not __";
        assert_eq!(expand(sql, "sqlite", None).unwrap(), sql);
        assert_eq!(expand(sql, "no-such-kind", None).unwrap(), sql);
    }

    /// Macros on an unknown kind → the kind is named (the table lives HERE, keyed on child vocab).
    #[test]
    fn unknown_kind_with_macros_is_a_named_error() {
        let e = expand("SELECT $__timeFrom() FROM h", "oracle", Some(&res())).unwrap_err();
        assert!(
            e.contains("no SQL time-macro expansion for source kind \"oracle\""),
            "{e}"
        );
    }
}
