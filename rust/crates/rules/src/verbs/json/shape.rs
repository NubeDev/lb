//! Map-shape verbs: `merge` (RFC-7386-style deep merge, `()` deletes), `flatten`/`unflatten`
//! (nested ↔ separator-joined keys), `pick`/`omit` (shape trimming), `entries`/`from_entries`
//! (map ↔ `[[k, v], …]`). All functional — inputs are consumed by value, a new value returns.

use rhai::{Array, Dynamic, Engine, EvalAltResult, Map};

use crate::grid::rhai_err;

pub(super) fn register(engine: &mut Engine) {
    engine.register_fn("merge", merge_maps);
    engine.register_fn("flatten", |m: Map, sep: &str| {
        let mut out = Map::new();
        flatten_into("", sep, &m, &mut out);
        out
    });
    engine.register_fn("unflatten", |m: Map, sep: &str| unflatten(m, sep));
    engine.register_fn(
        "pick",
        |m: Map, keys: Array| -> Result<Map, Box<EvalAltResult>> {
            let keys = key_list(keys, "pick")?;
            let mut out = Map::new();
            for k in keys {
                if let Some(v) = m.get(k.as_str()) {
                    out.insert(k.as_str().into(), v.clone());
                }
            }
            Ok(out)
        },
    );
    engine.register_fn(
        "omit",
        |mut m: Map, keys: Array| -> Result<Map, Box<EvalAltResult>> {
            for k in key_list(keys, "omit")? {
                m.remove(k.as_str());
            }
            Ok(m)
        },
    );
    engine.register_fn("entries", |m: Map| -> Array {
        m.into_iter()
            .map(|(k, v)| Dynamic::from_array(vec![Dynamic::from(k.to_string()), v]))
            .collect()
    });
    engine.register_fn(
        "from_entries",
        |pairs: Array| -> Result<Map, Box<EvalAltResult>> {
            let mut out = Map::new();
            for pair in pairs {
                let p = pair.try_cast::<Array>().ok_or_else(|| {
                    rhai_err("from_entries: every entry must be a [key, value] pair")
                })?;
                if p.len() != 2 {
                    return Err(rhai_err(
                        "from_entries: every entry must have exactly 2 items",
                    ));
                }
                let key = p[0]
                    .clone()
                    .into_string()
                    .map_err(|_| rhai_err("from_entries: entry keys must be strings"))?;
                out.insert(key.into(), p[1].clone());
            }
            Ok(out)
        },
    );
}

/// RFC-7386-style deep merge: `b` wins; a `()` value in `b` DELETES the key; two maps merge
/// recursively (a non-map in `a` is treated as an empty map so nested deletes still apply).
fn merge_maps(mut a: Map, b: Map) -> Map {
    for (k, v) in b {
        if v.is_unit() {
            a.remove(k.as_str());
        } else if v.is_map() {
            let base = match a.get(k.as_str()) {
                Some(x) if x.is_map() => x.clone().try_cast::<Map>().unwrap_or_default(),
                _ => Map::new(),
            };
            let patch = v.try_cast::<Map>().unwrap_or_default();
            a.insert(k, Dynamic::from_map(merge_maps(base, patch)));
        } else {
            a.insert(k, v);
        }
    }
    a
}

/// Depth-first: nested non-empty maps recurse under `prefix + sep + key`; everything else
/// (scalars, arrays, empty maps) lands as a leaf value.
fn flatten_into(prefix: &str, sep: &str, m: &Map, out: &mut Map) {
    for (k, v) in m {
        let key = if prefix.is_empty() {
            k.to_string()
        } else {
            format!("{prefix}{sep}{k}")
        };
        if let Some(inner) = v.read_lock::<Map>() {
            if !inner.is_empty() {
                flatten_into(&key, sep, &inner, out);
                continue;
            }
        }
        out.insert(key.into(), v.clone());
    }
}

/// Inverse of `flatten`: split each key on `sep` and rebuild the nesting. A scalar in the way of
/// a deeper key is replaced by a map (last write wins, as in a JSON build-up).
fn unflatten(m: Map, sep: &str) -> Map {
    let mut out = Map::new();
    for (k, v) in m {
        if sep.is_empty() {
            out.insert(k, v);
            continue;
        }
        let parts: Vec<&str> = k.as_str().split(sep).collect();
        insert_nested(&mut out, &parts, v);
    }
    out
}

fn insert_nested(out: &mut Map, parts: &[&str], v: Dynamic) {
    if parts.len() == 1 {
        out.insert(parts[0].into(), v);
        return;
    }
    let entry = out
        .entry(parts[0].into())
        .or_insert_with(|| Dynamic::from_map(Map::new()));
    if !entry.is_map() {
        *entry = Dynamic::from_map(Map::new());
    }
    if let Some(mut inner) = entry.write_lock::<Map>() {
        insert_nested(&mut inner, &parts[1..], v);
    }
}

/// Coerce a rhai array of string-ish values to `Vec<String>` (an author-facing error otherwise).
fn key_list(keys: Array, verb: &str) -> Result<Vec<String>, Box<EvalAltResult>> {
    keys.into_iter()
        .map(|d| {
            d.into_string()
                .map_err(|_| rhai_err(format!("{verb}: keys must be strings")))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid::{dynamic_to_json, json_to_dynamic};

    fn map_of(v: serde_json::Value) -> Map {
        json_to_dynamic(&v).try_cast::<Map>().unwrap()
    }

    #[test]
    fn merge_deep_merges_and_unit_deletes() {
        // Table: (a, b, expected) — the RFC-7386 semantics with () standing in for null.
        let cases = [
            (
                serde_json::json!({ "x": 1, "keep": true }),
                serde_json::json!({ "x": 2 }),
                serde_json::json!({ "x": 2, "keep": true }),
            ),
            (
                serde_json::json!({ "x": 1, "gone": "bye" }),
                serde_json::json!({ "gone": null }),
                serde_json::json!({ "x": 1 }),
            ),
            (
                serde_json::json!({ "nest": { "a": 1, "b": 2 } }),
                serde_json::json!({ "nest": { "b": null, "c": 3 } }),
                serde_json::json!({ "nest": { "a": 1, "c": 3 } }),
            ),
            // A non-map in `a` is treated as empty for a map patch.
            (
                serde_json::json!({ "nest": 5 }),
                serde_json::json!({ "nest": { "a": 1 } }),
                serde_json::json!({ "nest": { "a": 1 } }),
            ),
        ];
        for (a, b, want) in cases {
            let got = merge_maps(map_of(a.clone()), map_of(b.clone()));
            assert_eq!(
                dynamic_to_json(&Dynamic::from_map(got)),
                want,
                "merge({a}, {b})"
            );
        }
    }

    #[test]
    fn flatten_unflatten_round_trip() {
        let src = serde_json::json!({
            "a": { "b": { "c": 1 }, "d": [1, 2] },
            "top": "t"
        });
        let mut flat = Map::new();
        flatten_into("", ".", &map_of(src.clone()), &mut flat);
        assert_eq!(
            dynamic_to_json(flat.get("a.b.c").unwrap()),
            serde_json::json!(1)
        );
        let back = unflatten(flat, ".");
        assert_eq!(dynamic_to_json(&Dynamic::from_map(back)), src);
    }
}
