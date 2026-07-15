//! Chart-return helpers — the "make this rule chartable" sugar (rules-for-widgets-scope slice 3). Pure
//! compute over plain rows (a `rhai::Array` of maps, exactly what `g.records()` or a rows literal
//! yields), zero authority (the data-stdlib doctrine — sibling to the `timeseries` plan-builders, but
//! these run over already-collected rows, not a lazy Grid). The convention already existed (last
//! expression = array of row maps; a `time` column makes it a time-series) — these helpers just spare
//! the author from knowing that SQLite returns ISO strings, series return epoch, and the frame builder
//! only tags a column named `time`.
//!
//! - `timeseries(rows, "ts")`            → normalize the named column to canonical epoch-ms, rename it
//!                                         `time`, sort ascending. The frame builder tags the x-axis.
//! - `timeseries(rows, "ts", ["v1"])`    → same, plus trim to `time` + the named value columns.
//! - `wide(rows, "ts", "series","value")`→ long→wide pivot: one row per timestamp, one numeric column
//!                                         per distinct `series` value (the multi-line shape).
//! - `category(rows, "name", "value")`   → the bar/pie shape: one label column + one numeric column.
//!
//! Each returns plain rows, so `timeseries(query(…).records(), "ts")` as a rule's LAST line is a
//! complete chart-ready rule. Raw rows keep working — this is normalization, not a required layer.

use std::collections::BTreeMap;

use rhai::{Array, Dynamic, Engine, EvalAltResult, Map};

use crate::grid::rhai_err;

pub fn register(engine: &mut Engine) {
    engine.register_fn("timeseries", |rows: Array, ts: &str| {
        timeseries(rows, ts, None)
    });
    engine.register_fn("timeseries", |rows: Array, ts: &str, keep: Array| {
        let cols = string_list(keep)?;
        timeseries(rows, ts, Some(cols))
    });
    engine.register_fn(
        "wide",
        |rows: Array, ts: &str, series: &str, value: &str| wide(rows, ts, series, value),
    );
    engine.register_fn("category", |rows: Array, name: &str, value: &str| {
        category(rows, name, value)
    });
}

/// Normalize the `ts` column to canonical epoch-ms + rename it `time`, sorted ascending. When `keep`
/// is `Some`, trim each row to `time` + the named value columns (shape trimming for the chart).
fn timeseries(
    rows: Array,
    ts: &str,
    keep: Option<Vec<String>>,
) -> Result<Array, Box<EvalAltResult>> {
    // (epoch_ms, row) pairs so we can sort by the canonical time without re-parsing.
    let mut stamped: Vec<(i64, Map)> = Vec::with_capacity(rows.len());
    for row in rows {
        let mut map = as_map(row, "timeseries")?;
        let raw = map
            .get(ts)
            .ok_or_else(|| rhai_err(format!("timeseries: no column {ts:?} in row")))?;
        let epoch_ms = to_epoch_ms(raw)
            .ok_or_else(|| rhai_err(format!("timeseries: column {ts:?} is not a timestamp (ISO-8601 string, epoch-secs or epoch-ms)")))?;
        // Canonicalize: drop the source column, write `time` as epoch-ms.
        map.remove(ts);
        map.insert("time".into(), Dynamic::from(epoch_ms));
        if let Some(cols) = &keep {
            trim_to(&mut map, cols);
        }
        stamped.push((epoch_ms, map));
    }
    stamped.sort_by_key(|(t, _)| *t);
    Ok(stamped
        .into_iter()
        .map(|(_, m)| Dynamic::from_map(m))
        .collect())
}

/// Long→wide pivot: one output row per distinct `ts` value, with one column per distinct `series`
/// value carrying that (ts, series) pair's `value`. The classic multi-line-chart shape.
fn wide(rows: Array, ts: &str, series: &str, value: &str) -> Result<Array, Box<EvalAltResult>> {
    // Ordered by first-seen timestamp; each bucket is series-name → value.
    let mut buckets: BTreeMap<i64, Map> = BTreeMap::new();
    for row in rows {
        let map = as_map(row, "wide")?;
        let epoch_ms = map
            .get(ts)
            .and_then(to_epoch_ms)
            .ok_or_else(|| rhai_err(format!("wide: column {ts:?} is not a timestamp")))?;
        let name = map
            .get(series)
            .and_then(|v| v.clone().into_string().ok())
            .ok_or_else(|| {
                rhai_err(format!(
                    "wide: column {series:?} must be a string series label"
                ))
            })?;
        let val = map
            .get(value)
            .cloned()
            .ok_or_else(|| rhai_err(format!("wide: no value column {value:?} in row")))?;
        let bucket = buckets.entry(epoch_ms).or_default();
        bucket.insert("time".into(), Dynamic::from(epoch_ms));
        bucket.insert(name.into(), val);
    }
    Ok(buckets.into_values().map(Dynamic::from_map).collect())
}

/// The bar/pie shape: trim each row to one label column + one numeric column (validated present).
fn category(rows: Array, name: &str, value: &str) -> Result<Array, Box<EvalAltResult>> {
    let mut out = Array::with_capacity(rows.len());
    for row in rows {
        let map = as_map(row, "category")?;
        let label = map
            .get(name)
            .cloned()
            .ok_or_else(|| rhai_err(format!("category: no label column {name:?} in row")))?;
        let val = map
            .get(value)
            .cloned()
            .ok_or_else(|| rhai_err(format!("category: no value column {value:?} in row")))?;
        if !is_number(&val) {
            return Err(rhai_err(format!(
                "category: value column {value:?} must be numeric"
            )));
        }
        let mut trimmed = Map::new();
        trimmed.insert(name.into(), label);
        trimmed.insert(value.into(), val);
        out.push(Dynamic::from_map(trimmed));
    }
    Ok(out)
}

/// Coerce one row `Dynamic` to a `Map`, or an author-facing error (a non-record row is a shape mistake).
fn as_map(row: Dynamic, verb: &str) -> Result<Map, Box<EvalAltResult>> {
    row.try_cast::<Map>()
        .ok_or_else(|| rhai_err(format!("{verb}: every row must be a record (#{{…}})")))
}

/// Keep only `time` + the named columns on a row (`time` is always retained — it's the x-axis).
fn trim_to(map: &mut Map, cols: &[String]) {
    map.retain(|k, _| k.as_str() == "time" || cols.iter().any(|c| c == k.as_str()));
}

/// Normalize a timestamp `Dynamic` across the shapes sources actually return to canonical epoch-ms:
/// an ISO-8601 string, epoch-seconds, or epoch-ms. Heuristic for the int case: a value below ~1e12 is
/// treated as seconds (year ~33658 in ms is the crossover; every realistic epoch-secs value is below
/// it and every realistic epoch-ms value is above). Floats are truncated. `None` if unparseable.
pub(crate) fn to_epoch_ms(d: &Dynamic) -> Option<i64> {
    if let Ok(i) = d.as_int() {
        return Some(normalize_epoch(i));
    }
    if let Ok(f) = d.as_float() {
        return Some(normalize_epoch(f as i64));
    }
    if let Some(s) = d.clone().into_string().ok() {
        // Epoch given as a numeric string.
        if let Ok(i) = s.trim().parse::<i64>() {
            return Some(normalize_epoch(i));
        }
        return parse_iso8601_ms(s.trim());
    }
    None
}

/// Sub-1e12 → seconds → ms; otherwise already ms. (0 stays 0.)
fn normalize_epoch(v: i64) -> i64 {
    const MS_THRESHOLD: i64 = 1_000_000_000_000; // 1e12
    if v != 0 && v.abs() < MS_THRESHOLD {
        v * 1000
    } else {
        v
    }
}

/// Parse a minimal ISO-8601 instant (`YYYY-MM-DD`, optionally `THH:MM:SS`, optional fractional secs
/// and a trailing `Z`) to epoch-ms. Deterministic + dependency-free (no wall-clock, no chrono) — the
/// data-stdlib runs pure. A malformed string returns `None` (the author gets a clear error upstream).
fn parse_iso8601_ms(s: &str) -> Option<i64> {
    let (date, time) = match s.split_once(['T', ' ']) {
        Some((d, t)) => (d, t),
        None => (s, ""),
    };
    let mut dp = date.split('-');
    let year: i64 = dp.next()?.parse().ok()?;
    let month: i64 = dp.next()?.parse().ok()?;
    let day: i64 = dp.next()?.parse().ok()?;
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    let (mut hh, mut mm, mut ss, mut ms) = (0i64, 0i64, 0i64, 0i64);
    if !time.is_empty() {
        let t = time.trim_end_matches('Z');
        let (clock, frac) = match t.split_once('.') {
            Some((c, f)) => (c, f),
            None => (t, ""),
        };
        let mut cp = clock.split(':');
        hh = cp.next()?.parse().ok()?;
        mm = cp.next().unwrap_or("0").parse().ok()?;
        ss = cp.next().unwrap_or("0").parse().ok()?;
        if !frac.is_empty() {
            // First three fractional digits = milliseconds (pad/truncate).
            let mut digits = frac
                .chars()
                .filter(|c| c.is_ascii_digit())
                .collect::<String>();
            digits.truncate(3);
            while digits.len() < 3 {
                digits.push('0');
            }
            ms = digits.parse().ok()?;
        }
    }
    let days = days_from_civil(year, month, day);
    Some(((days * 86_400 + hh * 3_600 + mm * 60 + ss) * 1000) + ms)
}

/// Days since 1970-01-01 for a civil date (Howard Hinnant's algorithm — exact, no leap-second table).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn is_number(d: &Dynamic) -> bool {
    d.is_int() || d.is_float()
}

/// Coerce a rhai array of string-ish values to `Vec<String>` (an author-facing error otherwise).
fn string_list(arr: Array) -> Result<Vec<String>, Box<EvalAltResult>> {
    arr.into_iter()
        .map(|d| {
            d.into_string()
                .map_err(|_| rhai_err("expected a string column name"))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iso8601_date_only() {
        // 2021-01-01 = 18628 days since epoch.
        assert_eq!(parse_iso8601_ms("2021-01-01"), Some(18628 * 86_400 * 1000));
    }

    #[test]
    fn iso8601_with_time_and_z() {
        // 1970-01-01T00:00:01Z = 1000 ms.
        assert_eq!(parse_iso8601_ms("1970-01-01T00:00:01Z"), Some(1000));
        // 1970-01-01T00:00:00.500Z = 500 ms (fractional).
        assert_eq!(parse_iso8601_ms("1970-01-01T00:00:00.500Z"), Some(500));
    }

    #[test]
    fn epoch_secs_promoted_to_ms() {
        assert_eq!(normalize_epoch(1_600_000_000), 1_600_000_000_000);
    }

    #[test]
    fn epoch_ms_kept() {
        assert_eq!(normalize_epoch(1_600_000_000_000), 1_600_000_000_000);
    }

    #[test]
    fn bad_iso_is_none() {
        assert_eq!(parse_iso8601_ms("not-a-date"), None);
        assert_eq!(parse_iso8601_ms("2021-13-01"), None); // bad month
    }
}
