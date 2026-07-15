//! `jget` / `jset` / `jhas` — deep-path access over nested maps/arrays with the `"a.b[0].c"`
//! syntax (dots between keys, `[n]` for array indices, negative `n` counts from the end).
//! `jget` NEVER throws: an absent/malformed path yields `()` (or the explicit default — which also
//! covers a present JSON `null`, per the missing = `()` policy). `jset` returns a NEW value
//! (functional update): missing intermediate maps are created, an index equal to the array length
//! appends, anything else out of shape is a clear author error.

use rhai::{Array, Dynamic, Engine, EvalAltResult, Map};

use crate::grid::rhai_err;

pub(super) fn register(engine: &mut Engine) {
    engine.register_fn("jget", |v: Dynamic, path: &str| {
        jget(&v, path, Dynamic::UNIT)
    });
    engine.register_fn("jget", |v: Dynamic, path: &str, default: Dynamic| {
        jget(&v, path, default)
    });
    engine.register_fn("jhas", |v: Dynamic, path: &str| {
        parse_path(path).is_some_and(|segs| get_path(&v, &segs).is_some())
    });
    engine.register_fn(
        "jset",
        |v: Dynamic, path: &str, val: Dynamic| -> Result<Dynamic, Box<EvalAltResult>> {
            let segs = parse_path(path)
                .ok_or_else(|| rhai_err(format!("jset: malformed path {path:?}")))?;
            set_path(v, &segs, val)
        },
    );
}

/// One step of a deep path: a map key or an array index (negative = from the end).
#[derive(Debug)]
enum Seg {
    Key(String),
    Idx(i64),
}

/// Parse `"a.b[0].c"` into segments. `None` on a malformed path (empty segment, unclosed or
/// non-integer bracket) — `jget`/`jhas` treat that as absent, `jset` surfaces it as an error.
fn parse_path(path: &str) -> Option<Vec<Seg>> {
    let mut segs = Vec::new();
    for part in path.split('.') {
        let (name, mut rest) = match part.find('[') {
            Some(i) => (&part[..i], &part[i..]),
            None => (part, ""),
        };
        if name.is_empty() && rest.is_empty() {
            return None; // "" or "a..b"
        }
        if !name.is_empty() {
            segs.push(Seg::Key(name.to_string()));
        }
        while !rest.is_empty() {
            if !rest.starts_with('[') {
                return None; // trailing junk after a "]"
            }
            let close = rest.find(']')?;
            let idx: i64 = rest.get(1..close)?.trim().parse().ok()?;
            segs.push(Seg::Idx(idx));
            rest = &rest[close + 1..];
        }
    }
    Some(segs)
}

/// Resolve a (possibly negative) index against `len`; `None` when out of range.
fn resolve_idx(i: i64, len: usize) -> Option<usize> {
    if i >= 0 {
        let u = i as usize;
        (u < len).then_some(u)
    } else {
        len.checked_sub(i.unsigned_abs() as usize)
    }
}

/// Walk the segments; `None` the moment a key is absent, an index is out of range, or the current
/// value is not the right container. `Some(())` means the path IS present with a null value.
fn get_path(v: &Dynamic, segs: &[Seg]) -> Option<Dynamic> {
    let mut cur = v.clone();
    for seg in segs {
        cur = step(&cur, seg)?;
    }
    Some(cur)
}

fn step(cur: &Dynamic, seg: &Seg) -> Option<Dynamic> {
    match seg {
        Seg::Key(k) => cur.read_lock::<Map>()?.get(k.as_str()).cloned(),
        Seg::Idx(i) => {
            let arr = cur.read_lock::<Array>()?;
            arr.get(resolve_idx(*i, arr.len())?).cloned()
        }
    }
}

/// The never-throws get: absent path OR a resolved `()` (JSON null) yields `default`.
fn jget(v: &Dynamic, path: &str, default: Dynamic) -> Dynamic {
    match parse_path(path).and_then(|segs| get_path(v, &segs)) {
        Some(found) if !found.is_unit() => found,
        _ => default,
    }
}

/// Functional deep set: consume `cur`, return the updated value. Missing/`()` map segments are
/// created; arrays accept in-range indices (negative from the end) or `len` to append.
fn set_path(cur: Dynamic, segs: &[Seg], val: Dynamic) -> Result<Dynamic, Box<EvalAltResult>> {
    let Some((seg, rest)) = segs.split_first() else {
        return Ok(val);
    };
    match seg {
        Seg::Key(k) => {
            let mut map = if cur.is_map() {
                cur.try_cast::<Map>().unwrap_or_default()
            } else if cur.is_unit() {
                Map::new()
            } else {
                return Err(rhai_err(format!(
                    "jset: segment {k:?} reaches into a non-map value"
                )));
            };
            let inner = map.get(k.as_str()).cloned().unwrap_or(Dynamic::UNIT);
            map.insert(k.as_str().into(), set_path(inner, rest, val)?);
            Ok(Dynamic::from_map(map))
        }
        Seg::Idx(i) => {
            let mut arr = if cur.is_array() {
                cur.try_cast::<Array>().unwrap_or_default()
            } else if cur.is_unit() {
                Array::new()
            } else {
                return Err(rhai_err(format!(
                    "jset: index [{i}] reaches into a non-array value"
                )));
            };
            let len = arr.len();
            match resolve_idx(*i, len) {
                Some(idx) => {
                    let inner = std::mem::take(&mut arr[idx]);
                    arr[idx] = set_path(inner, rest, val)?;
                }
                None if *i == len as i64 => arr.push(set_path(Dynamic::UNIT, rest, val)?),
                None => {
                    return Err(rhai_err(format!(
                        "jset: index [{i}] out of range (len {len}; only 0..={len} may be set)"
                    )))
                }
            }
            Ok(Dynamic::from_array(arr))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> Dynamic {
        crate::grid::json_to_dynamic(&serde_json::json!({
            "a": { "b": [ { "c": 1 }, { "c": 2 } ], "n": null },
            "top": "t"
        }))
    }

    #[test]
    fn jget_walks_keys_and_indices() {
        let v = fixture();
        // Table: (path, expected-as-json)
        let cases = [
            ("a.b[0].c", serde_json::json!(1)),
            ("a.b[1].c", serde_json::json!(2)),
            ("a.b[-1].c", serde_json::json!(2)),
            ("top", serde_json::json!("t")),
        ];
        for (path, want) in cases {
            let got = jget(&v, path, Dynamic::UNIT);
            assert_eq!(crate::grid::dynamic_to_json(&got), want, "path {path}");
        }
    }

    #[test]
    fn jget_never_throws_and_defaults_cover_absent_and_null() {
        let v = fixture();
        for path in ["a.x.y", "a.b[9].c", "a.b[0].c.d", "..", "a.b[x]", "a.n"] {
            let got = jget(&v, path, Dynamic::from_int(42));
            assert_eq!(got.as_int(), Ok(42), "path {path} should fall to default");
        }
    }

    #[test]
    fn jhas_reports_presence_including_null() {
        let v = fixture();
        assert!(parse_path("a.n").is_some_and(|s| get_path(&v, &s).is_some()));
        assert!(parse_path("a.x").is_some_and(|s| get_path(&v, &s).is_none()));
    }

    #[test]
    fn jset_deep_sets_creates_and_appends() {
        let v = fixture();
        // Overwrite through an array index.
        let segs = parse_path("a.b[1].c").unwrap();
        let out = set_path(v.clone(), &segs, Dynamic::from_int(9)).unwrap();
        assert_eq!(jget(&out, "a.b[1].c", Dynamic::UNIT).as_int(), Ok(9));
        // Create intermediate maps.
        let segs = parse_path("a.new.deep").unwrap();
        let out = set_path(v.clone(), &segs, Dynamic::from_int(7)).unwrap();
        assert_eq!(jget(&out, "a.new.deep", Dynamic::UNIT).as_int(), Ok(7));
        // Append at len; past len is an error.
        let segs = parse_path("a.b[2]").unwrap();
        let out = set_path(v.clone(), &segs, Dynamic::from_int(3)).unwrap();
        assert_eq!(jget(&out, "a.b[2]", Dynamic::UNIT).as_int(), Ok(3));
        let segs = parse_path("a.b[9]").unwrap();
        assert!(set_path(v, &segs, Dynamic::UNIT).is_err());
    }
}
