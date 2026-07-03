//! The panel record + spec types (library-panels scope, "Data"). A **panel** is an asset — the
//! `dashboard` shape cloned one level down: a workspace-namespaced `panel:{id}` record holding the
//! **non-layout half of a v3 `Cell`** (the spec), an owner, and the S4 visibility tier. Sharing to a
//! *team* is a `share` EDGE (reused from `lb_assets`), not a field — so the existing three-gate read
//! check applies unchanged (library-panels scope, "How it fits").
//!
//! The load-bearing observation the scope names: a v3 `Cell` is cleanly separable. **Layout**
//! (`i,x,y,w,h`) is per-dashboard placement and stays on the [`crate::dashboard::Cell`]; **everything
//! else** (`view`, `title`, `description`, `sources[]`, `transformations`, `fieldConfig`, `options`,
//! `action`, v1/v2 `binding`/`source`) is the panel spec and lives here as [`PanelSpec`]. A dashboard
//! `Cell` gains an additive `panel_ref` pointing at a `panel:{id}`; `dashboard.get` hydrates the spec
//! from this record at read time. A panel is a **lens over data access, never a grant** — sharing a
//! panel shares its *definition*; its `sources[]` still re-check under the **viewer's** caps per call.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The table panels live in. Record id is `panel:{id}` (the id is a stable slug, unique per workspace).
pub const TABLE: &str = "panel";

/// Our panel-model document version, pinned on [`Panel::schema_version`] at save — the SAME v3 panel
/// model the dashboard `Cell` uses ([`crate::dashboard`] `SCHEMA_VERSION`). A panel's spec is exactly
/// the non-layout portion of a v3 cell, so it versions identically.
pub const SCHEMA_VERSION: u32 = 3;

/// A panel's visibility tier — the S4 asset-sharing tiers (identical to the dashboard tiers, so the
/// same gate-3 read check applies unchanged; library-panels scope, "How it fits").
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

/// The **non-layout half of a v3 `Cell`** — the reusable panel definition (library-panels scope). Every
/// field mirrors the corresponding [`crate::dashboard::Cell`] field verbatim (same serde renames), so
/// the client extracts a spec by simply dropping the layout keys (`i,x,y,w,h`) + `panel_ref`, and
/// hydration re-inflates a full `Cell` by copying these back over a ref cell's layout. All fields are
/// serde-defaulted so a v1/v2 panel deserializes unchanged, exactly as a `Cell` does.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct PanelSpec {
    /// Contract version (as [`crate::dashboard::Cell::v`]).
    #[serde(default)]
    pub v: u32,
    /// The built-in widget kind (`chart`/`stat`/`gauge` …) or `ext:<id>` — as `Cell::widget_type`.
    #[serde(default)]
    pub widget_type: String,
    /// The panel title (the spec's own header; the asset name lives on [`Panel::title`]).
    #[serde(default)]
    pub title: String,
    /// v2 render vocabulary (as `Cell::view`).
    #[serde(default)]
    pub view: String,
    /// v1 data binding (as `Cell::binding`).
    #[serde(default)]
    pub binding: Value,
    /// v2 source (as `Cell::source`).
    #[serde(default)]
    pub source: super::super::dashboard::Source,
    /// v2 control action (as `Cell::action`).
    #[serde(default)]
    pub action: super::super::dashboard::Action,
    /// Widget-type options (as `Cell::options`).
    #[serde(default)]
    pub options: Value,
    /// v3 panel description (as `Cell::description`).
    #[serde(default)]
    pub description: String,
    /// v3 targets (as `Cell::sources`).
    #[serde(default)]
    pub sources: Vec<super::super::dashboard::Target>,
    /// v3 transformation pipeline (as `Cell::transformations`).
    #[serde(default)]
    pub transformations: Vec<Value>,
    /// v3 `fieldConfig` (as `Cell::field_config`).
    #[serde(default, rename = "fieldConfig")]
    pub field_config: Value,
    /// v3 plugin version (as `Cell::plugin_version`).
    #[serde(default, rename = "pluginVersion")]
    pub plugin_version: String,
}

/// A panel record. The persisted spec + sharing metadata (library-panels scope, "Data").
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Panel {
    /// Stable slug, unique per workspace (the record id `panel:{id}`).
    pub id: String,
    /// The asset name (free to rename; the slug is forever — library-panels scope Decision).
    pub title: String,
    /// The principal who created it (the private→shared model's anchor).
    pub owner: String,
    #[serde(default)]
    pub visibility: Visibility,
    /// The non-layout v3 spec this asset holds.
    #[serde(default)]
    pub spec: PanelSpec,
    /// OUR panel-model document version — pinned at save (`SCHEMA_VERSION`).
    #[serde(default, rename = "schemaVersion")]
    pub schema_version: u32,
    pub updated_ts: u64,
    /// Tombstone (soft-delete, §6.8 idempotent). A deleted panel is hidden from `list`/`get` and a
    /// ref to it hydrates to the placeholder.
    #[serde(default)]
    pub deleted: bool,
}

/// The cheap roster row `panel.list` returns — id/title/view/visibility/updated_ts, **no spec body and
/// no usage count** (usage is computed on demand by `panel.usage`; library-panels scope Decision).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PanelSummary {
    pub id: String,
    pub title: String,
    /// The panel's view/widget kind, so the picker can show an icon without fetching the spec.
    pub view: String,
    pub visibility: Visibility,
    pub updated_ts: u64,
}

impl From<&Panel> for PanelSummary {
    fn from(p: &Panel) -> Self {
        // The picker wants the render kind; a v3 panel uses `view`, a v1 panel uses `widget_type`.
        let view = if p.spec.view.is_empty() {
            p.spec.widget_type.clone()
        } else {
            p.spec.view.clone()
        };
        Self {
            id: p.id.clone(),
            title: p.title.clone(),
            view,
            visibility: p.visibility,
            updated_ts: p.updated_ts,
        }
    }
}

/// One dashboard that references a panel — the `panel.usage` row (library-panels scope, the
/// delete-safety + "where is this used" read). Id + title only (the cheap roster shape).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PanelUsageRow {
    /// The `dashboard:{id}` that has ≥1 cell referencing this panel.
    pub dashboard: String,
    pub title: String,
    /// How many cells on that dashboard reference the panel.
    pub cells: usize,
}
