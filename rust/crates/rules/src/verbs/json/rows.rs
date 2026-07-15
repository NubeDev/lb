//! Row verbs over an array of row maps — the shape `g.records()` / `query(...).records()` returns:
//! `pluck`, `index_by`, `group_rows`, `where_eq`, `sort_by`, `uniq_by`, `count_by`, `rows_epoch`.
//! A non-map row is an author shape mistake (clear error, as the chart helpers do). A missing
//! field is `()` (the missing = `()` ↔ null policy); grouping keys stringify naturally
//! (`"cooler.temp"`, `"42"`, `"null"`).

use std::cmp::Ordering;
use std::collections::HashSet;

use rhai::{Array, Dynamic, Engine, EvalAltResult, Map};

use crate::grid::{dynamic_to_json, rhai_err};

pub(super) fn register(engine: &mut Engine) {
    engine.register_fn("pluck", |rows: Array, field: &str| pluck(rows, field));
    engine.register_fn("index_by", |rows: Array, key: &str| index_by(rows, key));
    engine.register_fn("group_rows", |rows: Array, key: &str| group_rows(rows, key));
    engine.register_fn("where_eq", |rows: Array, key: &str, val: Dynamic| {
        where_eq(rows, key, val)
    });
    engine.register_fn("sort_by", |rows: Array, key: &str| {
        sort_by(rows, key, false)
    });
    engine.register_fn("sort_by", |rows: Array, key: &str, desc: bool| {
        sort_by(rows, key, desc)
    });
    engine.register_fn("uniq_by", |rows: Array, key: &str| uniq_by(rows, key));
    engine.register_fn("count_by", |rows: Array, key: &str| count_by(rows, key));
    engine.register_fn("rows_epoch", |rows: Array, field: &str| {
        rows_epoch(rows, field)
    });
}

/// Array-of-maps → array of the named field (`()` where a row lacks it — alignment is preserved).
fn pluck(rows: Array, field: &str) -> Result<Array, Box<EvalAltResult>> {
    rows.into_iter()
        .map(|row| Ok(field_of(&as_map(row, "pluck")?, field)))
        .collect()
}

/// Rows → map keyed by the stringified field value; last row wins on duplicates.
fn index_by(rows: Array, key: &str) -> Result<Map, Box<EvalAltResult>> {
    let mut out = Map::new();
    for row in rows {
        let map = as_map(row, "index_by")?;
        let k = key_string(&field_of(&map, key));
        out.insert(k.into(), Dynamic::from_map(map));
    }
    Ok(out)
}

/// Rows → map of arrays, one bucket per distinct stringified field value (insertion order kept
/// within a bucket).
fn group_rows(rows: Array, key: &str) -> Result<Map, Box<EvalAltResult>> {
    let mut out = Map::new();
    for row in rows {
        let map = as_map(row, "group_rows")?;
        let k = key_string(&field_of(&map, key));
        let bucket = out
            .entry(k.into())
            .or_insert_with(|| Dynamic::from_array(Array::new()));
        if let Some(mut arr) = bucket.write_lock::<Array>() {
            arr.push(Dynamic::from_map(map));
        }
    }
    Ok(out)
}

/// Keep the rows whose field equals `val` (numeric 1 == 1.0; a missing field matches `val == ()`).
fn where_eq(rows: Array, key: &str, val: Dynamic) -> Result<Array, Box<EvalAltResult>> {
    let mut out = Array::new();
    for row in rows {
        let map = as_map(row, "where_eq")?;
        if dyn_eq(&field_of(&map, key), &val) {
            out.push(Dynamic::from_map(map));
        }
    }
    Ok(out)
}

/// Stable sort by the field: numbers before strings, missing (`()`) first; `desc` flips the order
/// (equal keys keep their input order either way).
fn sort_by(rows: Array, key: &str, desc: bool) -> Result<Array, Box<EvalAltResult>> {
    let mut pairs: Vec<(Dynamic, Map)> = Vec::with_capacity(rows.len());
    for row in rows {
        let map = as_map(row, "sort_by")?;
        pairs.push((field_of(&map, key), map));
    }
    pairs.sort_by(|a, b| {
        let ord = dyn_ord(&a.0, &b.0);
        if desc {
            ord.reverse()
        } else {
            ord
        }
    });
    Ok(pairs
        .into_iter()
        .map(|(_, m)| Dynamic::from_map(m))
        .collect())
}

/// Keep the FIRST row per distinct stringified field value.
fn uniq_by(rows: Array, key: &str) -> Result<Array, Box<EvalAltResult>> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut out = Array::new();
    for row in rows {
        let map = as_map(row, "uniq_by")?;
        if seen.insert(key_string(&field_of(&map, key))) {
            out.push(Dynamic::from_map(map));
        }
    }
    Ok(out)
}

/// Frequency map: stringified field value → row count.
fn count_by(rows: Array, key: &str) -> Result<Map, Box<EvalAltResult>> {
    let mut out = Map::new();
    for row in rows {
        let map = as_map(row, "count_by")?;
        let k = key_string(&field_of(&map, key));
        let n = out
            .get(k.as_str())
            .and_then(|d| d.as_int().ok())
            .unwrap_or(0);
        out.insert(k.into(), Dynamic::from_int(n + 1));
    }
    Ok(out)
}

/// Normalize the named ts field across every row to epoch SECONDS (ISO string | epoch-secs |
/// epoch-ms, number or string — whatever the source returned). Unparseable/missing is a clear
/// author error, not a silent skip.
fn rows_epoch(rows: Array, field: &str) -> Result<Array, Box<EvalAltResult>> {
    let mut out = Array::with_capacity(rows.len());
    for row in rows {
        let mut map = as_map(row, "rows_epoch")?;
        let raw = map
            .get(field)
            .ok_or_else(|| rhai_err(format!("rows_epoch: no field {field:?} in row")))?;
        let secs = super::surreal::epoch_of(raw).ok_or_else(|| {
            rhai_err(format!(
                "rows_epoch: field {field:?} is not a timestamp (ISO-8601 string, epoch-secs or epoch-ms)"
            ))
        })?;
        map.insert(field.into(), Dynamic::from_int(secs));
        out.push(Dynamic::from_map(map));
    }
    Ok(out)
}

/// Coerce one row to a `Map`, or an author-facing error (a non-record row is a shape mistake).
fn as_map(row: Dynamic, verb: &str) -> Result<Map, Box<EvalAltResult>> {
    row.try_cast::<Map>()
        .ok_or_else(|| rhai_err(format!("{verb}: every row must be a record (#{{…}})")))
}

/// The field's value, `()` when absent (missing = `()`).
fn field_of(map: &Map, field: &str) -> Dynamic {
    map.get(field).cloned().unwrap_or(Dynamic::UNIT)
}

fn num(d: &Dynamic) -> Option<f64> {
    if let Ok(i) = d.as_int() {
        Some(i as f64)
    } else if let Ok(f) = d.as_float() {
        Some(f)
    } else {
        None
    }
}

/// Equality across the value shapes rows carry: numerically for numbers (1 == 1.0), by JSON value
/// otherwise (so strings, bools, nested shapes, and `()` ↔ null all compare sanely).
fn dyn_eq(a: &Dynamic, b: &Dynamic) -> bool {
    if let (Some(x), Some(y)) = (num(a), num(b)) {
        return x == y;
    }
    dynamic_to_json(a) == dynamic_to_json(b)
}

/// Total order for sorting mixed columns: `()` < bool < number < string < everything-else, then
/// within a rank the natural order (numbers numerically, strings lexically, rest by JSON text).
fn dyn_ord(a: &Dynamic, b: &Dynamic) -> Ordering {
    if let (Some(x), Some(y)) = (num(a), num(b)) {
        return x.partial_cmp(&y).unwrap_or(Ordering::Equal);
    }
    fn rank(d: &Dynamic) -> u8 {
        if d.is_unit() {
            0
        } else if d.is_bool() {
            1
        } else if d.is_int() || d.is_float() {
            2
        } else if d.is_string() {
            3
        } else {
            4
        }
    }
    let (ra, rb) = (rank(a), rank(b));
    if ra != rb {
        return ra.cmp(&rb);
    }
    match ra {
        1 => a
            .as_bool()
            .unwrap_or(false)
            .cmp(&b.as_bool().unwrap_or(false)),
        3 => a
            .clone()
            .into_string()
            .unwrap_or_default()
            .cmp(&b.clone().into_string().unwrap_or_default()),
        _ => dynamic_to_json(a)
            .to_string()
            .cmp(&dynamic_to_json(b).to_string()),
    }
}

/// The natural grouping key for a field value: strings as-is, `()` as `"null"`, everything else
/// via its JSON text (`42`, `true`).
fn key_string(d: &Dynamic) -> String {
    if let Ok(s) = d.clone().into_string() {
        return s;
    }
    dynamic_to_json(d).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid::json_to_dynamic;

    fn rows() -> Array {
        json_to_dynamic(&serde_json::json!([
            { "id": "b", "v": 2.0, "s": "hot" },
            { "id": "a", "v": 1,   "s": "cold" },
            { "id": "c", "v": 2,   "s": "hot" },
        ]))
        .try_cast::<Array>()
        .unwrap()
    }

    #[test]
    fn where_eq_matches_across_int_and_float() {
        // v == 2 must match both the float 2.0 row and the int 2 row.
        let out = where_eq(rows(), "v", Dynamic::from_int(2)).unwrap();
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn sort_by_orders_and_desc_flips() {
        let asc = sort_by(rows(), "id", false).unwrap();
        let ids: Vec<String> = asc
            .iter()
            .map(|r| key_string(&field_of(&r.read_lock::<Map>().unwrap(), "id")))
            .collect();
        assert_eq!(ids, ["a", "b", "c"]);
        let desc = sort_by(rows(), "id", true).unwrap();
        assert_eq!(
            key_string(&field_of(&desc[0].read_lock::<Map>().unwrap(), "id")),
            "c"
        );
    }

    #[test]
    fn count_by_builds_a_frequency_map() {
        let out = count_by(rows(), "s").unwrap();
        assert_eq!(out.get("hot").unwrap().as_int(), Ok(2));
        assert_eq!(out.get("cold").unwrap().as_int(), Ok(1));
    }
}
