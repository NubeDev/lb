//! The `renameByRegex` transformer — Grafana's `RenameByRegexTransformer` (viz grafana-parity
//! scope, tranche 2a). Options verbatim: `{ regex, renamePattern }`. Every field name matching
//! `regex` is rewritten by `renamePattern` (with `$1`-style capture substitution — Grafana's JS
//! `String.replace` and Rust's `Regex::replace` share the `$n` grammar); a non-matching name and a
//! non-compiling regex pass through untouched (degrade, never an error). Grafana anchors the match
//! (`^…$` — `getRegex` wraps user input) so we do too.

use regex::Regex;
use serde_json::Value;

use crate::frame::Frames;

/// Apply `renameByRegex` to every field of every frame.
pub fn apply(mut frames: Frames, options: &Value) -> Frames {
    let pattern = options.get("regex").and_then(Value::as_str).unwrap_or("");
    let rename = options
        .get("renamePattern")
        .and_then(Value::as_str)
        .unwrap_or("");
    if pattern.is_empty() {
        return frames;
    }
    // Grafana anchors the user's pattern (whole-name match). A bad pattern → carried untouched.
    let Ok(re) = Regex::new(&format!("^(?:{pattern})$")) else {
        return frames;
    };
    for frame in &mut frames {
        for field in &mut frame.fields {
            if re.is_match(&field.name) {
                field.name = re.replace(&field.name, rename).into_owned();
            }
        }
    }
    frames
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::{Field, Frame};
    use serde_json::json;

    fn seeded() -> Frames {
        vec![Frame::new(vec![
            Field::new("temp_cooler", vec![json!(1)]),
            Field::new("temp_fryer", vec![json!(2)]),
            Field::new("state", vec![json!("on")]),
        ])]
    }

    #[test]
    fn capture_group_rename() {
        let out = apply(
            seeded(),
            &json!({ "regex": "temp_(.*)", "renamePattern": "$1" }),
        );
        let names: Vec<&str> = out[0].fields.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, vec!["cooler", "fryer", "state"]);
    }

    #[test]
    fn non_matching_names_untouched_and_match_is_anchored() {
        // "emp" matches inside every temp_* name but is NOT the whole name → nothing renamed.
        let out = apply(seeded(), &json!({ "regex": "emp", "renamePattern": "X" }));
        let names: Vec<&str> = out[0].fields.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, vec!["temp_cooler", "temp_fryer", "state"]);
    }

    #[test]
    fn bad_regex_or_empty_options_degrade_to_passthrough() {
        let out = apply(seeded(), &json!({ "regex": "(", "renamePattern": "x" }));
        assert_eq!(out[0].fields[0].name, "temp_cooler");
        let out = apply(seeded(), &json!({}));
        assert_eq!(out[0].fields[0].name, "temp_cooler");
    }
}
