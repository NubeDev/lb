//! Run-time value types — `Rule` / `RuleRun` / `RuleOutput` / `Finding` / `LogLine`. **Ported from
//! rubix-cube** (`rules/runtime.rs`, MIT/Apache-2.0), **re-keyed** `project_id` → `workspace` and
//! `allowed_datasets` → the workspace's granted sources (resolved host-side). No `Uuid` dependency:
//! a saved rule's id is the host's `rule:{ws}:{id}` key; an ad-hoc run has no id.

use std::collections::HashSet;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A declared parameter of a saved rule — name + an optional human label. The bound value is supplied
/// at run time in [`RuleRun::inputs`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleParam {
    pub name: String,
    #[serde(default)]
    pub label: Option<String>,
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
    ) -> Self {
        Self {
            workspace,
            allowed_sources,
            inputs,
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
