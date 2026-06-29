//! The `filterFieldsByName` transformer — Grafana's `FilterFieldsByNameTransformer` (viz
//! transformations scope). Keeps/drops whole FIELDS by name or regexp. Option shape verbatim:
//! `include?: {names?, pattern?}`, `exclude?: {names?, pattern?}`. One responsibility: field-name
//! selection. Reuses the shared `Matcher` `byRegexp` so the pattern dialect never drifts from
//! `fieldConfig` overrides. Pure: takes frames by value, returns frames with fewer/same fields.

use serde_json::{json, Value};

use crate::config::Matcher;
use crate::frame::{Field, Frame, Frames};

/// Apply `filterFieldsByName` per frame. A field is kept if it matches `include` (when an include is
/// configured) AND does NOT match `exclude`. With neither set the frame passes through unchanged.
pub fn apply(frames: Frames, options: &Value) -> Frames {
    let include = options.get("include");
    let exclude = options.get("exclude");
    let has_include = spec_present(include);
    let has_exclude = spec_present(exclude);
    if !has_include && !has_exclude {
        return frames;
    }
    frames
        .into_iter()
        .map(|f| filter_frame(f, include, has_include, exclude, has_exclude))
        .collect()
}

fn filter_frame(
    frame: Frame,
    include: Option<&Value>,
    has_include: bool,
    exclude: Option<&Value>,
    has_exclude: bool,
) -> Frame {
    let fields: Vec<Field> = frame
        .fields
        .into_iter()
        .filter(|f| {
            let kept = !has_include || matches_spec(include, &f.name);
            let dropped = has_exclude && matches_spec(exclude, &f.name);
            kept && !dropped
        })
        .collect();
    let mut out = Frame::new(fields).relen();
    out.ref_id = frame.ref_id;
    out.name = frame.name;
    out
}

/// Whether an include/exclude spec actually constrains anything (a `names[]` or a non-empty `pattern`).
fn spec_present(spec: Option<&Value>) -> bool {
    let names = spec
        .and_then(|s| s.get("names"))
        .and_then(Value::as_array)
        .map(|a| !a.is_empty())
        .unwrap_or(false);
    let pattern = spec
        .and_then(|s| s.get("pattern"))
        .and_then(Value::as_str)
        .map(|p| !p.is_empty())
        .unwrap_or(false);
    names || pattern
}

/// Whether `name` matches a spec — `names[]` membership OR the `pattern` via the shared `byRegexp`
/// matcher (the same dialect `config.rs` ships, no regex dep).
fn matches_spec(spec: Option<&Value>, name: &str) -> bool {
    let by_name = spec
        .and_then(|s| s.get("names"))
        .and_then(Value::as_array)
        .map(|a| a.iter().any(|v| v.as_str() == Some(name)))
        .unwrap_or(false);
    if by_name {
        return true;
    }
    spec.and_then(|s| s.get("pattern"))
        .and_then(Value::as_str)
        .filter(|p| !p.is_empty())
        .map(|p| {
            Matcher {
                id: "byRegexp".into(),
                options: json!(p),
            }
            .matches_field(name, "")
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seeded() -> Frames {
        vec![Frame::new(vec![
            Field::new("temp_c", vec![json!(1)]),
            Field::new("temp_f", vec![json!(2)]),
            Field::new("humidity", vec![json!(3)]),
        ])]
    }

    fn names(frames: &Frames) -> Vec<String> {
        frames[0].fields.iter().map(|f| f.name.clone()).collect()
    }

    #[test]
    fn include_by_names() {
        let out = apply(
            seeded(),
            &json!({ "include": { "names": ["temp_c", "humidity"] } }),
        );
        assert_eq!(names(&out), vec!["temp_c", "humidity"]);
    }

    #[test]
    fn include_by_pattern_then_exclude() {
        let out = apply(
            seeded(),
            &json!({ "include": { "pattern": "^temp.*" }, "exclude": { "names": ["temp_f"] } }),
        );
        assert_eq!(names(&out), vec!["temp_c"]);
    }

    #[test]
    fn exclude_only() {
        let out = apply(seeded(), &json!({ "exclude": { "pattern": ".*_f$" } }));
        assert_eq!(names(&out), vec!["temp_c", "humidity"]);
    }

    #[test]
    fn no_spec_passes_through() {
        let out = apply(seeded(), &json!({}));
        assert_eq!(names(&out), vec!["temp_c", "temp_f", "humidity"]);
    }
}
