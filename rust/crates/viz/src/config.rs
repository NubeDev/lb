//! The transformation config — Grafana's `DataTransformerConfig` adopted **verbatim** (viz
//! transformations scope, "Adopt Grafana's transformation model verbatim") so an imported dashboard's
//! `transformations[]` is a pass-through, not a translation. One responsibility: the `Transformation`
//! + `Matcher` shapes the pipeline reads (the per-id `options` stay opaque `Value`, parsed by each
//! transformer file).

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// One pipeline step — Grafana's `DataTransformerConfig { id, options, disabled, filter, topic }`.
/// `id` is the Grafana transformer id (`reduce`/`organize`/`joinByField`/…); `options` is that
/// transformer's option bag (opaque here, parsed per-id); `disabled` skips the step but keeps it for
/// round-trip; `filter` scopes the step to matching frames (we keep `topic` in config for round-trip,
/// deferred on apply — no annotations plane yet).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Transformation {
    pub id: String,
    #[serde(default)]
    pub options: Value,
    #[serde(default)]
    pub disabled: bool,
    #[serde(default)]
    pub filter: Option<Matcher>,
    /// Kept for Grafana round-trip; deferred on apply.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub topic: Option<String>,
}

/// A field/frame matcher — the SAME shape `fieldConfig.overrides[]` uses (one matcher model across the
/// slice; the TS mirror is `fieldconfig.types.ts`). Phase-1 ids: `byName`, `byType`, `byRegexp`,
/// `byFrameRefID`. An unknown id matches nothing (honest — never a silent match-all).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Matcher {
    pub id: String,
    #[serde(default)]
    pub options: Value,
}

impl Matcher {
    /// Whether this matcher selects a field named `name` of type tag `ty` (`"number"`/`"string"`/…).
    /// `byName` → exact name; `byType` → type tag; `byRegexp` → the field name matches the (anchored,
    /// substring) pattern via a tiny matcher (no regex dep — Grafana's common cases: literal +
    /// `.*`/prefix/suffix). `byFrameRefID` is a frame matcher, not a field one → never matches a field.
    pub fn matches_field(&self, name: &str, ty: &str) -> bool {
        match self.id.as_str() {
            "byName" => self.options.as_str() == Some(name),
            "byType" => self.options.as_str() == Some(ty),
            "byRegexp" => self
                .options
                .as_str()
                .map(|p| simple_regex_match(p, name))
                .unwrap_or(false),
            _ => false,
        }
    }

    /// Whether this matcher selects a frame with `ref_id` (`byFrameRefID`). Other ids are field
    /// matchers → never select a whole frame.
    pub fn matches_frame(&self, ref_id: &str) -> bool {
        match self.id.as_str() {
            "byFrameRefID" => self.options.as_str() == Some(ref_id),
            _ => false,
        }
    }
}

/// A dependency-free subset of regex sufficient for Grafana's common field-name patterns: a literal,
/// or `.*` wildcards (`^a.*`, `.*b$`, `a.*b`, bare `.*`). Anchors `^`/`$` honored; an unsupported
/// metacharacter falls back to a literal substring test (honest — never a silent match-all).
fn simple_regex_match(pattern: &str, name: &str) -> bool {
    let p = pattern.trim();
    let anchored_start = p.starts_with('^');
    let anchored_end = p.ends_with('$');
    let core = p
        .strip_prefix('^')
        .unwrap_or(p)
        .strip_suffix('$')
        .unwrap_or(p.strip_prefix('^').unwrap_or(p));
    // Split on `.*` and require the literal segments to appear in order, honoring anchors.
    if core == ".*" || core.is_empty() {
        return true;
    }
    if !core.contains(".*") {
        // No wildcard: literal compare honoring anchors, else substring.
        return match (anchored_start, anchored_end) {
            (true, true) => name == core,
            (true, false) => name.starts_with(core),
            (false, true) => name.ends_with(core),
            (false, false) => name.contains(core),
        };
    }
    let segs: Vec<&str> = core.split(".*").collect();
    let mut idx = 0usize;
    for (i, seg) in segs.iter().enumerate() {
        if seg.is_empty() {
            continue;
        }
        match name[idx..].find(seg) {
            Some(pos) => {
                let abs = idx + pos;
                if i == 0 && anchored_start && abs != 0 {
                    return false;
                }
                idx = abs + seg.len();
            }
            None => return false,
        }
    }
    if anchored_end {
        if let Some(last) = segs.iter().rev().find(|s| !s.is_empty()) {
            return name.ends_with(last);
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn matcher_by_name_and_type() {
        let m = Matcher {
            id: "byName".into(),
            options: json!("temp"),
        };
        assert!(m.matches_field("temp", "number"));
        assert!(!m.matches_field("humidity", "number"));
        let t = Matcher {
            id: "byType".into(),
            options: json!("time"),
        };
        assert!(t.matches_field("ts", "time"));
        assert!(!t.matches_field("ts", "number"));
    }

    #[test]
    fn matcher_regex_wildcards_and_anchors() {
        let m = Matcher {
            id: "byRegexp".into(),
            options: json!("^temp.*"),
        };
        assert!(m.matches_field("temp_c", "number"));
        assert!(!m.matches_field("x_temp", "number"));
        let s = Matcher {
            id: "byRegexp".into(),
            options: json!(".*_c$"),
        };
        assert!(s.matches_field("temp_c", "number"));
        assert!(!s.matches_field("temp_f", "number"));
        let any = Matcher {
            id: "byRegexp".into(),
            options: json!(".*"),
        };
        assert!(any.matches_field("anything", "number"));
    }

    #[test]
    fn unknown_matcher_selects_nothing() {
        let m = Matcher {
            id: "byWhatever".into(),
            options: json!("x"),
        };
        assert!(!m.matches_field("x", "number"));
        assert!(!m.matches_frame("A"));
    }
}
