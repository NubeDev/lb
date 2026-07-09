//! The Grafana dashboard JSON input schema — classic, `schemaVersion` 42 (grafana-
//! conversion scope, "Non-goals"). We model **only what we read**, loosely: every
//! field is `#[serde(default)]` so older/newer classic exports still parse, and
//! everything we do not interpret is captured as raw `Value` for the report's
//! degrade/drop accounting. This is *Grafana's* shape, not ours — never serialize
//! it back out (the output is the vendored `model::Dashboard`).
//!
//! One-direction only (Grafana → us). The export direction is Stage 3.

use serde::Deserialize;
use serde_json::Value;

/// The top-level Grafana dashboard export (`/api/dashboards/uid` shape, with or
/// without the wrapping `{"dashboard": …}` envelope — both supported).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GrafanaDashboard {
    /// The schema pin. We read 42 as-is; older schemas are reported as
    /// `dashboard.schemaVersion` degraded (no migration in this cut).
    #[serde(default)]
    pub schema_version: u32,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub editable: bool,
    #[serde(default)]
    pub graph_tooltip: Value,
    #[serde(default)]
    pub timezone: Value,
    #[serde(default)]
    pub week_start: Value,
    #[serde(default)]
    pub fiscal_year_start_month: Value,
    #[serde(default)]
    pub live_now: bool,
    #[serde(default)]
    pub preload: Value,
    /// Time range default `{from, to}` (e.g. `"now-6h"`, `"now"`).
    #[serde(default)]
    pub time: Value,
    /// Auto-refresh string (e.g. `"30s"`, `""`).
    #[serde(default)]
    pub refresh: Value,
    #[serde(default)]
    pub timepicker: Value,
    /// Classic flat panel list — the input shape we accept. v2 (`dashboard.grafana.app/
    /// v2beta1`) layout is reported as unsupported.
    #[serde(default)]
    pub panels: Vec<Panel>,
    /// Template-variable definitions.
    #[serde(default)]
    pub templating: Templating,
    #[serde(default)]
    pub annotations: Value,
    #[serde(default)]
    pub links: Value,
    /// Catch-all for everything we do not model by name (so the report can name the
    /// dropped/degraded fields without enumerating every classic key).
    #[serde(flatten)]
    pub other: serde_json::Map<String, Value>,
}

impl GrafanaDashboard {
    /// True if the input looks like the v2 kind-based layout (which we do not accept).
    /// Heuristic: the presence of `spec` + `kind: "Dashboard"` (Grafana app-plugin v2).
    pub fn looks_like_v2(&self) -> bool {
        self.other.get("kind").and_then(Value::as_str) == Some("Dashboard")
            && self.other.contains_key("spec")
    }
}

/// `templating: { list: [...] }`.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Templating {
    #[serde(default)]
    pub list: Vec<Variable>,
}

/// A classic `panels[]` entry. We model the bits the mapper reads; the rest is
/// carried in `other` for the report.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Panel {
    pub id: Option<i64>,
    #[serde(default)]
    pub r#type: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub grid_pos: GridPos,
    /// `type:"row"` only — children when collapsed (`panels` nested), empty when expanded.
    #[serde(default)]
    pub panels: Vec<Panel>,
    #[serde(default)]
    pub collapsed: bool,
    /// Repeat-by variable name (e.g. `"server"`). Empty when no repeat.
    #[serde(default)]
    pub repeat: Value,
    #[serde(default)]
    pub repeat_direction: Value,
    #[serde(default)]
    pub max_per_row: Value,
    /// Library-panel reference `{name, uid}`.
    #[serde(default)]
    pub library_panel: Value,
    /// Query targets (PromQL/SQL/etc.) — mapped to `Cell.sources[]`.
    #[serde(default)]
    pub targets: Vec<Value>,
    /// The datasource for the panel as a whole (`{type, uid}`), used when a target
    /// does not name one.
    #[serde(default)]
    pub datasource: Value,
    /// `fieldConfig { defaults, overrides[] }` — carried through opaque.
    #[serde(default)]
    pub field_config: Value,
    /// `transformations[]` — carried through opaque.
    #[serde(default)]
    pub transformations: Vec<Value>,
    /// Panel options (unit, thresholds, …) — carried through opaque.
    #[serde(default)]
    pub options: Value,
    #[serde(default)]
    pub plugin_version: Value,
    #[serde(flatten)]
    pub other: serde_json::Map<String, Value>,
}

impl Panel {
    /// True for `type:"row"` panels (the dual-encoded row container).
    pub fn is_row(&self) -> bool {
        self.r#type == "row"
    }
}

/// 24-column grid cell geometry.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct GridPos {
    #[serde(default)]
    pub x: u32,
    #[serde(default)]
    pub y: u32,
    #[serde(default)]
    pub w: u32,
    #[serde(default)]
    pub h: u32,
}

/// One `templating.list[]` entry. Loose — every field the mapper cares about by
/// name is here, the rest is in `other`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Variable {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub r#type: String,
    /// Grafana's `query` field is polymorphic per type: a comma-string for
    /// `custom`, an object `{query, ...}` for `query`/`datasource`, a number for
    /// `interval`. Carried as `Value` and decoded by the variable mapper.
    #[serde(default)]
    pub query: Value,
    /// Static options list for `custom`/`query` (`{text,value,selected}[]`).
    #[serde(default)]
    pub options: Vec<Value>,
    #[serde(default)]
    pub multi: bool,
    #[serde(default)]
    pub include_all: bool,
    #[serde(default)]
    pub all_value: Value,
    #[serde(default)]
    pub regex: Value,
    #[serde(default)]
    pub regex_apply_to: Value,
    #[serde(default)]
    pub sort: Value,
    #[serde(default)]
    pub refresh: Value,
    #[serde(default)]
    pub hide: Value,
    #[serde(default)]
    pub current: Value,
    #[serde(default)]
    pub datasource: Value,
    #[serde(flatten)]
    pub other: serde_json::Map<String, Value>,
}
