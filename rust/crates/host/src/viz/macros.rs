//! Grafana-compatible interval macros for `federation.query` targets (viz panel-resolution scope,
//! issue #101, v1 — the zero-parse federation half). Before a federated target is dispatched,
//! `viz.query` substitutes four macros into the target's `sql` string with the SAME derived resolution
//! the platform series path uses:
//!
//!   - `$__interval_ms` → the snapped bucket width in **milliseconds** (integer).
//!   - `$__interval`    → the same width as a Grafana-style duration string (`"30s"`, `"12h"`, `"1d"`).
//!   - `$__timeFrom`    → the window start (epoch **ms**, integer).
//!   - `$__timeTo`      → the window end   (epoch **ms**, integer).
//!
//! So an author writes `date_bin(INTERVAL '$__interval', ts, …) … WHERE ts BETWEEN $__timeFrom AND
//! $__timeTo` (the exact SQL shape the rules `rollup` verb already emits) and the query coarsens itself
//! as the visible range grows — no client math, one derivation, cache-key-stable.
//!
//! **Zero SQL parsing.** This is a pure textual substitution: we never tokenize the SELECT, never guess
//! a time column, never touch a string literal's contents on purpose (a `$__interval` the author put
//! *inside* a quoted string is substituted too — that is the author's problem, documented, not a parse
//! attempt). **An un-macro'd SQL is byte-identical afterwards** — a substitution with no matching token
//! is a no-op, so today's hand-SQL tiles run verbatim.
//!
//! Substitution order matters: `$__interval_ms` is a prefix-superset of `$__interval`, so the longer
//! token is replaced FIRST — otherwise `$__interval` would corrupt `$__interval_ms` into `"<dur>_ms"`.

use serde_json::{json, Value};

use super::resolution::Resolution;

/// The macro tokens, longest-first so a prefix (`$__interval`) never eats a superset (`$__interval_ms`).
const INTERVAL_MS: &str = "$__interval_ms";
const INTERVAL: &str = "$__interval";
const TIME_FROM: &str = "$__timeFrom";
const TIME_TO: &str = "$__timeTo";

/// Substitute the four interval macros into a `federation.query` target's `sql`, in place, using the
/// derived `res`. Returns `true` when a `sql` string was present and rewritten (even to an identical
/// value — the caller uses it only for diagnostics). A target with no `sql` string is left untouched.
///
/// The un-macro'd invariant: if `sql` contains none of the tokens, the output equals the input byte for
/// byte (the replacements are no-ops), so a hand-written SQL tile is never mutated.
pub fn substitute_macros(args: &mut Value, res: &Resolution) -> bool {
    let Value::Object(map) = args else {
        return false;
    };
    let Some(Value::String(sql)) = map.get("sql") else {
        return false;
    };
    let rewritten = apply(sql, res);
    map.insert("sql".into(), json!(rewritten));
    true
}

/// The pure substitution — exposed for the un-macro'd byte-identity test.
pub fn apply(sql: &str, res: &Resolution) -> String {
    sql.replace(INTERVAL_MS, &res.width_ms.to_string())
        .replace(INTERVAL, &interval_to_grafana(res.width_ms))
        .replace(TIME_FROM, &res.from.to_string())
        .replace(TIME_TO, &res.to.to_string())
}

/// Format a width in ms as a Grafana-style duration string (`secondsToHms`): the largest whole unit
/// among `y/d/h/m/s`; a sub-second width falls back to `Nms`. Matches Grafana core (no weeks/months
/// units), so an imported dashboard's `$__interval` renders identically.
fn interval_to_grafana(width_ms: u64) -> String {
    if width_ms == 0 {
        return "0s".into();
    }
    if width_ms % 1_000 != 0 {
        return format!("{width_ms}ms");
    }
    let secs = width_ms / 1_000;
    const UNITS: &[(u64, &str)] = &[
        (31_536_000, "y"),
        (86_400, "d"),
        (3_600, "h"),
        (60, "m"),
        (1, "s"),
    ];
    for &(size, suffix) in UNITS {
        if secs % size == 0 {
            return format!("{}{}", secs / size, suffix);
        }
    }
    format!("{secs}s")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn res(from: u64, to: u64, width_ms: u64) -> Resolution {
        Resolution { from, to, width_ms }
    }

    /// All four macros substitute with the derived values (the scope's headline example).
    #[test]
    fn substitutes_all_four_macros() {
        let sql = "SELECT date_bin(INTERVAL '$__interval', ts, TIMESTAMP '1970-01-01') AS t, \
                   avg(v) FROM m WHERE ts BETWEEN $__timeFrom AND $__timeTo \
                   GROUP BY 1 -- width $__interval_ms ms";
        let out = apply(sql, &res(1_000, 2_000, 43_200_000)); // 12h
        assert!(out.contains("INTERVAL '12h'"), "interval → 12h: {out}");
        assert!(out.contains("BETWEEN 1000 AND 2000"), "from/to → ms: {out}");
        assert!(
            out.contains("width 43200000 ms"),
            "interval_ms → integer: {out}"
        );
        assert!(!out.contains("$__"), "no macro token survives: {out}");
    }

    /// `$__interval_ms` is NOT corrupted by the `$__interval` pass (longest-token-first).
    #[test]
    fn interval_ms_not_eaten_by_interval() {
        let out = apply(
            "bucket=$__interval_ms step='$__interval'",
            &res(0, 1, 30_000),
        );
        assert_eq!(
            out, "bucket=30000 step='30s'",
            "ms integer + duration string, both intact"
        );
    }

    /// An un-macro'd SQL is byte-identical (the load-bearing invariant: hand-SQL tiles run verbatim).
    #[test]
    fn un_macrod_sql_is_byte_identical() {
        let sql = "SELECT ts, value FROM readings WHERE room = 'lab' ORDER BY ts LIMIT 500";
        let out = apply(sql, &res(1, 2, 30_000));
        assert_eq!(out, sql, "no token → no change, byte for byte");
    }

    /// Grafana-style duration formatting across the ladder + a sub-second and a non-round width.
    #[test]
    fn grafana_duration_formatting() {
        assert_eq!(interval_to_grafana(1_000), "1s");
        assert_eq!(interval_to_grafana(30_000), "30s");
        assert_eq!(interval_to_grafana(900_000), "15m");
        assert_eq!(interval_to_grafana(3_600_000), "1h");
        assert_eq!(interval_to_grafana(43_200_000), "12h");
        assert_eq!(interval_to_grafana(86_400_000), "1d");
        assert_eq!(interval_to_grafana(604_800_000), "7d");
        assert_eq!(interval_to_grafana(2_592_000_000), "30d");
        assert_eq!(interval_to_grafana(200), "200ms", "sub-second → ms");
        assert_eq!(
            interval_to_grafana(90_000),
            "90s",
            "non-round-minute stays seconds"
        );
    }

    /// `substitute_macros` writes back into `args.sql`; a target with no `sql` is untouched.
    #[test]
    fn substitute_in_place_and_skips_non_sql() {
        let mut args = json!({ "source": "demo", "sql": "SELECT $__interval_ms" });
        assert!(substitute_macros(&mut args, &res(0, 1, 5_000)));
        assert_eq!(args["sql"], json!("SELECT 5000"));

        let mut no_sql = json!({ "source": "demo", "series": "x" });
        let before = no_sql.clone();
        assert!(!substitute_macros(&mut no_sql, &res(0, 1, 5_000)));
        assert_eq!(no_sql, before, "no sql field → untouched");
    }
}
