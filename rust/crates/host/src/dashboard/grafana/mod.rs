//! Grafana JSON import / export (viz import-export scope, Phase 4) — the interop edge that turns a
//! Grafana dashboard JSON into our native [`Cell`/`Dashboard`](super::model) record and back. One
//! bidirectional mapper (one responsibility per file), consumed by two host verbs:
//!
//! - [`dashboard_import`] — a 4-stage pipeline: **migrate** (the P3 `grafana-map` pin: detect v1/v2,
//!   normalize `schemaVersion`, resolve `__inputs`) → **map** (`grafana→cell`, panel-by-panel) →
//!   **bind** (commit only: a remapped target → an EXECUTABLE one — the `tool` + arg names our verbs
//!   read; without it a panel imports clean and renders blank) → **report** (datasource-remap prompts
//!   + a degraded list). Two phases: a preview (no `mappings` → report only, no write) and a commit
//!   (`mappings` → UPSERT via `dashboard.save`).
//! - [`dashboard_export`] — the inverse `cell→grafana` map, re-emitting each cell's bounded `_grafana`
//!   passthrough so unknown Grafana fields survive a round-trip.
//!
//! We never store Grafana JSON raw (the panel-model spine decision): the mapper runs at the edge, the
//! record stays ours. Tenancy: the workspace is the caller's token, never the JSON; every referenced
//! datasource is remapped strictly within the caller's workspace (the hard wall).

mod bind;
mod datasources;
mod export;
mod import;
mod to_cell;
mod to_grafana;
mod view_alias;

use lb_auth::Principal;
use lb_mcp::ToolError;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::boot::Node;

use super::error::DashboardError;

pub use export::{dashboard_export, export_descriptor};
pub use import::{dashboard_import, import_descriptor};

/// MCP bridge for the two `&Node`-taking Grafana verbs (`dashboard.import`/`dashboard.export`) —
/// dispatched before the store-only `dashboard.` branch because import must resolve datasource remaps
/// against the workspace-walled `datasource.list`. Denials are opaque (`ToolError::Denied`).
pub async fn call_dashboard_grafana_tool(
    node: &Node,
    principal: &Principal,
    ws: &str,
    qualified_tool: &str,
    input: &Value,
) -> Result<Value, ToolError> {
    match qualified_tool {
        "dashboard.import" => {
            let json = input
                .get("json")
                .cloned()
                .ok_or_else(|| ToolError::BadInput("missing `json`".into()))?;
            // `mappings` absent/null = preview phase (empty vec).
            let mappings: Vec<DatasourceRemap> = match input.get("mappings") {
                Some(v) if !v.is_null() => serde_json::from_value(v.clone())
                    .map_err(|e| ToolError::BadInput(format!("bad `mappings`: {e}")))?,
                _ => Vec::new(),
            };
            let id = input.get("id").and_then(Value::as_str).unwrap_or("");
            let now = input.get("now").and_then(Value::as_u64).unwrap_or(0);
            let outcome = dashboard_import(node, principal, ws, json, mappings, id, now)
                .await
                .map_err(to_tool)?;
            Ok(serde_json::to_value(outcome).unwrap_or(Value::Null))
        }
        "dashboard.export" => {
            let id = input
                .get("id")
                .and_then(Value::as_str)
                .ok_or_else(|| ToolError::BadInput("missing `id`".into()))?;
            let json = dashboard_export(node, principal, ws, id)
                .await
                .map_err(to_tool)?;
            Ok(json)
        }
        _ => Err(ToolError::NotFound),
    }
}

/// Map the dashboard error onto the MCP tool error (denials opaque — mirrors `tool::to_tool`).
fn to_tool(e: DashboardError) -> ToolError {
    match e {
        DashboardError::Denied => ToolError::Denied,
        DashboardError::NotFound => ToolError::NotFound,
        DashboardError::BadInput(m) => ToolError::BadInput(m),
        DashboardError::Store(s) => ToolError::Extension(s.to_string()),
    }
}

/// One referenced datasource the import found, awaiting a remap onto one of *our* workspace
/// datasources. `type`/`uid` are Grafana's (untrusted); `mapped_to` is the caller's chosen target
/// datasource NAME in their workspace (empty in a preview report = not yet mapped).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DatasourceRemap {
    /// The Grafana datasource type (`prometheus`, `mysql`, …) — informational only.
    #[serde(rename = "type", default)]
    pub kind: String,
    /// The Grafana datasource uid the JSON references (or a `${DS_*}` name pre-resolution).
    pub uid: String,
    /// Our workspace datasource NAME the user bound it to (a commit-phase input; empty in preview).
    #[serde(default, rename = "mappedTo")]
    pub mapped_to: String,
}

/// One thing the import could not map cleanly — surfaced so nothing is silently dropped. The panel is
/// still imported (preserved + placeholder); the report lists why.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DegradedItem {
    /// `panel` | `transform` | `variable` | `datasource` | `migration` — what kind of thing degraded.
    pub kind: String,
    /// The cell/panel index or name it belongs to (empty for dashboard-level notices).
    #[serde(default)]
    pub cell: String,
    /// A human sentence naming exactly what degraded (e.g. `unsupported panel type 'heatmap'`).
    pub detail: String,
}

/// The import `report` — returned by BOTH phases. In preview it drives the remap UI; on commit it is
/// echoed back so the caller sees what shipped.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ImportReport {
    /// How many panels mapped to a native view.
    #[serde(rename = "mappedPanels")]
    pub mapped_panels: usize,
    /// Every referenced datasource + its (chosen or pending) remap target.
    #[serde(default)]
    pub datasources: Vec<DatasourceRemap>,
    /// Everything degraded (unsupported panel/transform/variable, unmapped datasource, migration
    /// notice) — preserved and flagged, never faked.
    #[serde(default)]
    pub degraded: Vec<DegradedItem>,
    /// The highest Grafana `schemaVersion` the migration pin normalizes to (surfaced so the user knows
    /// the bound) and any degradation notice from the pin.
    #[serde(default, rename = "migratedFrom")]
    pub migrated_from: u64,
}
