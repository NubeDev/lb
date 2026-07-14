//! The `filterByRefId` transformer — Grafana's `FilterFramesByRefIdTransformer` (viz
//! grafana-parity scope, tranche 2a). Options verbatim: `{ include?, exclude? }`, each a regex
//! over frame refIds (the editor writes alternations like `"A|B"`). Keep a frame when it matches
//! `include` (absent/empty = keep all) AND does not match `exclude`. A non-compiling regex is
//! ignored (that clause degrades to no-op — carried, never an error).

use regex::Regex;
use serde_json::Value;

use crate::frame::Frames;

/// Apply `filterByRefId`: drop whole frames by refId.
pub fn apply(frames: Frames, options: &Value) -> Frames {
    let include = compile(options.get("include"));
    let exclude = compile(options.get("exclude"));
    frames
        .into_iter()
        .filter(|f| {
            let kept = include.as_ref().is_none_or(|re| re.is_match(&f.ref_id));
            let dropped = exclude.as_ref().is_some_and(|re| re.is_match(&f.ref_id));
            kept && !dropped
        })
        .collect()
}

/// Compile an optional regex option (anchored whole-refId, matching Grafana's matcher semantics).
/// Absent/empty/non-compiling → `None` (the clause does not constrain).
fn compile(v: Option<&Value>) -> Option<Regex> {
    let s = v?.as_str()?;
    if s.is_empty() {
        return None;
    }
    Regex::new(&format!("^(?:{s})$")).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::{Field, Frame};
    use serde_json::json;

    fn seeded() -> Frames {
        ["A", "B", "C"]
            .iter()
            .map(|r| {
                let mut f = Frame::new(vec![Field::new("v", vec![json!(1)])]);
                f.ref_id = r.to_string();
                f
            })
            .collect()
    }

    fn ids(frames: &Frames) -> Vec<&str> {
        frames.iter().map(|f| f.ref_id.as_str()).collect()
    }

    #[test]
    fn include_keeps_only_matching() {
        let out = apply(seeded(), &json!({ "include": "A|C" }));
        assert_eq!(ids(&out), vec!["A", "C"]);
    }

    #[test]
    fn exclude_drops_matching_and_composes_with_include() {
        let out = apply(seeded(), &json!({ "exclude": "B" }));
        assert_eq!(ids(&out), vec!["A", "C"]);
        let out = apply(seeded(), &json!({ "include": "A|B", "exclude": "B" }));
        assert_eq!(ids(&out), vec!["A"]);
    }

    #[test]
    fn empty_or_bad_options_keep_everything() {
        assert_eq!(ids(&apply(seeded(), &json!({}))), vec!["A", "B", "C"]);
        assert_eq!(
            ids(&apply(seeded(), &json!({ "include": "(" }))),
            vec!["A", "B", "C"]
        );
    }
}
