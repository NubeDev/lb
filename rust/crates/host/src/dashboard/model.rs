//! The dashboard record + cell types (dashboard scope, "Data"). A dashboard is an **asset**: a
//! workspace-namespaced `dashboard:{id}` record holding the grid layout (`cells[]`), the owner, and
//! the S4 visibility tier. Sharing to a *team* is a `share` EDGE (reused from `lb_assets`), not a
//! field — so the existing three-gate read check applies unchanged (dashboard scope, "How it fits").
//!
//! `cells` is a typed nested object (queryable, no app-side JSON parsing) — the storage discipline
//! the ingest scope established. The binding is the forever-contract Phase 2 moves behind the bridge
//! unchanged: a cell names a `widget_type` and a `binding` (explicit series OR a tag-facet query).

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The table dashboards live in. Record id is `dashboard:{id}` (the id is a stable slug, unique per
/// workspace).
pub const TABLE: &str = "dashboard";

/// A dashboard's visibility tier — the S4 asset-sharing tiers (dashboard scope, "Access").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Visibility {
    /// Owner only.
    Private,
    /// Shared to a team via the `share` edge (read by team members).
    Team,
    /// Any workspace member with the read cap.
    Workspace,
}

impl Default for Visibility {
    fn default() -> Self {
        Visibility::Private
    }
}

/// A cell's data source, v2: ANY MCP tool call (read or write) in the install grant — not the
/// frozen four series verbs (widget-builder scope, "The widget contract, v2"). The forwardable set
/// is `cell.tools ∩ install-grant`, re-checked at the host per call. A v1 cell carries no `source`
/// and falls back to `binding`; a v2 cell names `{ tool, args }`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Source {
    /// The MCP tool the cell reads (or, for a control, the read it reflects). E.g. `series.read`,
    /// `series.watch`, `<ext>.<verb>`.
    pub tool: String,
    /// The arguments passed to `tool` (opaque to the host; re-checked per call).
    #[serde(default)]
    pub args: Value,
}

/// A control's write action, v2: the tool a `switch`/`slider`/`button` CALLS on interaction
/// (widget-builder scope, "Control views"). `args_template` is a typed template with one `{{value}}`
/// slot the interaction fills (the slider value, the switch state). The write tool is gated by its
/// own existing capability, re-checked at the host per call — the cell invents no new cap.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Action {
    /// The write tool invoked on interaction. E.g. `mqtt.publish`, `ingest.write`, `<ext>.<verb>`.
    pub tool: String,
    /// The argument template; a `{{value}}` token (any string leaf) is substituted with the control
    /// state on interaction. Opaque to the host.
    #[serde(default)]
    pub args_template: Value,
}

/// One grid cell: react-grid-layout geometry + the widget it hosts + its data binding (dashboard
/// scope, "Data").
///
/// **v1 (frozen):** `widget_type` + `binding` (`{series}` | `{find:{tags}}`) + `options`.
/// **v2 (widget-builder scope):** adds `view` (the render vocabulary), `source` (`{tool,args}` — any
/// granted tool, read or write), and `action` (a control's write tool). All v2 fields are
/// serde-defaulted so a v1 series cell deserializes unchanged (a v1 cell is a v2 cell whose tool set
/// is the four read verbs). The receiver rejects an unknown major `v`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Cell {
    /// react-grid-layout item key (stable per cell).
    pub i: String,
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
    /// Contract version. Absent/`0`/`1` = a v1 series cell; `2` = a v2 tool-bound cell.
    #[serde(default)]
    pub v: u32,
    /// Phase 1 built-ins: `chart` | `stat` | `gauge`. Phase 2 adds `ext:<id>` (federated widgets).
    pub widget_type: String,
    /// v2 render vocabulary: `chart`/`stat`/`gauge`/`table` (read), `plot`/`d3`/`template` (scripted,
    /// iframe), `switch`/`slider`/`button` (controls), `ext:<id>/<widget>` (extension tiles). Empty on
    /// a v1 cell — `widget_type` is authoritative there.
    #[serde(default)]
    pub view: String,
    /// The data binding — `{ "series": "cooler.temp" }` or `{ "find": { "tags": [...] } }`. v1; a v2
    /// cell uses `source` instead (this stays for v1 compatibility).
    #[serde(default)]
    pub binding: Value,
    /// v2 source: the `{ tool, args }` the cell reads/streams. Empty on a v1 cell.
    #[serde(default)]
    pub source: Source,
    /// v2 action: a control's write `{ tool, args_template }`. Empty on a non-control cell.
    #[serde(default)]
    pub action: Action,
    /// Widget-type-specific options (range, unit label, thresholds, inline template code). Opaque to
    /// the host.
    #[serde(default)]
    pub options: Value,
}

/// A dashboard record. The persisted layout + sharing metadata (dashboard scope, "Data").
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Dashboard {
    /// Stable slug, unique per workspace (the record id `dashboard:{id}`).
    pub id: String,
    pub title: String,
    /// The principal who created it (the private→shared model's anchor).
    pub owner: String,
    #[serde(default)]
    pub visibility: Visibility,
    #[serde(default)]
    pub cells: Vec<Cell>,
    pub updated_ts: u64,
    /// Tombstone (soft-delete, §6.8 idempotent). A deleted dashboard is hidden from `list`/`get`.
    #[serde(default)]
    pub deleted: bool,
}

/// The cheap roster row `list` returns — id/title/visibility/updated_ts, **no cell bodies** (the
/// roster stays cheap; dashboard scope, "Get / list").
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DashboardSummary {
    pub id: String,
    pub title: String,
    pub visibility: Visibility,
    pub updated_ts: u64,
}

impl From<&Dashboard> for DashboardSummary {
    fn from(d: &Dashboard) -> Self {
        Self {
            id: d.id.clone(),
            title: d.title.clone(),
            visibility: d.visibility,
            updated_ts: d.updated_ts,
        }
    }
}
