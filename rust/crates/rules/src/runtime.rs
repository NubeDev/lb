//! Run-time value types — `Rule` / `RuleRun` / `RuleOutput` / `Finding` / `LogLine`. **Ported from
//! rubix-cube** (`rules/runtime.rs`, MIT/Apache-2.0), **re-keyed** `project_id` → `workspace` and
//! `allowed_datasets` → the workspace's granted sources (resolved host-side). No `Uuid` dependency:
//! a saved rule's id is the host's `rule:{ws}:{id}` key; an ad-hoc run has no id.

use std::collections::HashSet;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The declared TYPE of a param — steers the authoring input + the value coercion. Absent (an older
/// `{name,label}` record) reads as [`ParamKind::Text`] via serde default, so no migration is needed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ParamKind {
    #[default]
    Text,
    Number,
    Date,
    Enum,
}

/// A declared parameter of a saved rule — name + an optional human label + its type. The bound value
/// is supplied at run time in [`RuleRun::inputs`] (a JSON value whose type the host preserves into the
/// cage: a `number` param arrives as a rhai number, not a string). `kind`/`required`/`options` all
/// serde-default, so a legacy `{name,label}` record deserializes unchanged.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleParam {
    pub name: String,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub kind: ParamKind,
    /// The author must supply a value before the rule runs (the UI blocks an empty required param).
    #[serde(default)]
    pub required: bool,
    /// The allowed values for an `enum` param (ignored for other kinds).
    #[serde(default)]
    pub options: Vec<String>,
}

/// A rule definition: its Rhai body + declared params. `workspace` is host-set from the token (never
/// script-set); `name` is the saved id for a persisted rule, `"adhoc"` for a Playground run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
    pub workspace: String,
    pub name: String,
    /// The Rhai source. The only thing the author writes.
    pub body: String,
    #[serde(default)]
    pub params: Vec<RuleParam>,
}

/// One emitted finding (`emit`/`alert`). `level` (`info|warning|critical`) is lifted for filtering;
/// the whole emitted map rides through as `data`. `alert == true` marks it for inbox/outbox routing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    pub level: String,
    pub data: Value,
}

impl Finding {
    /// Whether this finding was raised by `alert` (so the host routes it to inbox + outbox).
    pub fn is_alert(&self) -> bool {
        self.data
            .get("alert")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    }
}

/// One `log(...)` line collected during a run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogLine {
    pub level: String,
    pub message: String,
}

/// The per-run AI spend, surfaced in the result for observability (mirrors the `AiMeter` counters).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AiBudget {
    pub calls: u32,
    pub tokens: u32,
}

/// A materialized grid result — columns + JSON rows (what `data.query`/`federation.query` return).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GridJson {
    pub columns: Vec<String>,
    pub rows: Vec<Value>,
}

/// The typed result of a run — what the rule body's last expression resolved to (ported verbatim).
#[derive(Debug, Clone)]
pub enum RuleOutput {
    Scalar(Value),
    Grid(GridJson),
    Findings,
    Nothing,
}

impl Serialize for RuleOutput {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        match self {
            RuleOutput::Scalar(v) => {
                let mut m = s.serialize_map(Some(2))?;
                m.serialize_entry("kind", "scalar")?;
                m.serialize_entry("value", v)?;
                m.end()
            }
            RuleOutput::Grid(g) => {
                let mut m = s.serialize_map(Some(3))?;
                m.serialize_entry("kind", "grid")?;
                m.serialize_entry("columns", &g.columns)?;
                m.serialize_entry("rows", &g.rows)?;
                m.end()
            }
            RuleOutput::Findings => {
                let mut m = s.serialize_map(Some(1))?;
                m.serialize_entry("kind", "findings")?;
                m.end()
            }
            RuleOutput::Nothing => {
                let mut m = s.serialize_map(Some(1))?;
                m.serialize_entry("kind", "nothing")?;
                m.end()
            }
        }
    }
}

/// The mutable state of one rule evaluation — the allowlist (granted sources), bound inputs, and the
/// drained collectors. `workspace` is the hard wall; a verb resolves sources only within it.
pub struct RuleRun {
    pub workspace: String,
    /// The set of source names this run may read — the workspace's granted sources, host-resolved.
    pub allowed_sources: Arc<HashSet<String>>,
    /// Param name → bound value (also pushed as scope vars).
    pub inputs: rhai::Map,
    /// The run's logical clock — the injected `now` (no wall-clock in core). Feeds deterministic
    /// messaging-write ids (`now` + per-run counter) so a re-run upserts (rules-messaging-scope).
    pub now: u64,
    /// `emit`/`alert` append here.
    pub findings: Vec<Finding>,
    pub log: Vec<LogLine>,
    pub ai_spend: AiBudget,
}

impl RuleRun {
    pub fn new(
        workspace: String,
        allowed_sources: Arc<HashSet<String>>,
        inputs: rhai::Map,
        now: u64,
    ) -> Self {
        Self {
            workspace,
            allowed_sources,
            inputs,
            now,
            findings: Vec::new(),
            log: Vec::new(),
            ai_spend: AiBudget::default(),
        }
    }
}

/// Errors a run can produce. `Eval` is a user-script fault (author feedback, 400-equivalent); `Seam`
/// is a host-side data/AI failure; `SourceNotAllowed` is the allowlist deny (opaque at the MCP layer);
/// `Join` is a task panic (500-equivalent).
#[derive(thiserror::Error, Debug)]
pub enum RuleError {
    #[error("{0}")]
    Eval(String),
    #[error("source not allowed: {0}")]
    SourceNotAllowed(String),
    #[error("seam error: {0}")]
    Seam(String),
    #[error("rule task failed: {0}")]
    Join(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_param_deserializes_with_defaults() {
        // A `{name,label}` record from before typed params must still load — kind defaults to Text,
        // required to false, options to empty (no migration).
        let p: RuleParam = serde_json::from_str(r#"{"name":"site","label":"Site"}"#).unwrap();
        assert_eq!(p.name, "site");
        assert_eq!(p.label.as_deref(), Some("Site"));
        assert_eq!(p.kind, ParamKind::Text);
        assert!(!p.required);
        assert!(p.options.is_empty());
    }

    #[test]
    fn typed_param_round_trips() {
        let p = RuleParam {
            name: "region".into(),
            label: None,
            kind: ParamKind::Enum,
            required: true,
            options: vec!["emea".into(), "amer".into()],
        };
        let json = serde_json::to_string(&p).unwrap();
        // `kind` serializes lowercase (matches the TS `ParamKind` union + the wire the UI sends).
        assert!(json.contains(r#""kind":"enum""#));
        let back: RuleParam = serde_json::from_str(&json).unwrap();
        assert_eq!(back.kind, ParamKind::Enum);
        assert!(back.required);
        assert_eq!(back.options, vec!["emea".to_string(), "amer".to_string()]);
    }
}
