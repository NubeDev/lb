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

/// One grid cell: react-grid-layout geometry + the widget it hosts + its data binding (dashboard
/// scope, "Data"). `binding` is `{series}` OR `{find:{tags}}`; `options` is widget-type-specific.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Cell {
    /// react-grid-layout item key (stable per cell).
    pub i: String,
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
    /// Phase 1 built-ins: `chart` | `stat` | `gauge`. Phase 2 adds `ext:<id>` (federated widgets).
    pub widget_type: String,
    /// The data binding — `{ "series": "cooler.temp" }` or `{ "find": { "tags": [...] } }`.
    pub binding: Value,
    /// Widget-type-specific options (range, unit label, thresholds). Opaque to the host.
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
