//! `dashboard.export {id} -> {json}` — serialize one of our dashboards back to a Grafana JSON (viz
//! import-export scope, Phase 4). A read: gated `mcp:dashboard.export:call` (deny opaque). Runs the
//! three-gate `dashboard.get` read first (workspace + read cap + membership/visibility), then the
//! inverse `cell→grafana` map, re-emitting each cell's `_grafana` passthrough so a supported dashboard
//! round-trips semantically stable.
//!
//! The emitted JSON pins `schemaVersion` to the version our mapper targets ([`grafana_map`]'s pinned
//! version) so a re-import migrates cleanly. We never store Grafana JSON — this is produced at the edge.

use lb_auth::Principal;
use lb_mcp::ToolDescriptor;
use serde_json::{json, Value};

use super::super::authorize::authorize_dashboard;
use super::super::error::DashboardError;
use super::super::get::dashboard_get;
use crate::boot::Node;

use super::to_grafana::cell_to_panel;

/// Export dashboard `id` in `ws` (as `principal`) to a Grafana dashboard JSON object.
pub async fn dashboard_export(
    node: &Node,
    principal: &Principal,
    ws: &str,
    id: &str,
) -> Result<Value, DashboardError> {
    // The export capability (a read) — checked before the get's own gates for an early opaque deny.
    authorize_dashboard(principal, ws, "dashboard.export")?;

    // The full three-gate read (hydrates library-panel refs to inline cells — a ref's spec exports as
    // a normal panel, which is what a Grafana consumer expects).
    let dashboard = dashboard_get(&node.store, principal, ws, id).await?;

    let panels: Vec<Value> = dashboard.cells.iter().map(cell_to_panel).collect();

    let mut out = json!({
        "schemaVersion": grafana_map::PINNED_SCHEMA_VERSION,
        "title": dashboard.title,
        "uid": dashboard.id,
        "panels": panels,
        "templating": { "list": dashboard.variables },
        // Grafana editor mode markers a consumer expects; harmless, keeps the JSON well-formed.
        "editable": true,
        "graphTooltip": 0,
    });
    if !dashboard.timezone.is_empty() {
        out["timezone"] = Value::String(dashboard.timezone.clone());
    }
    Ok(out)
}

/// The `dashboard.export` descriptor.
pub fn export_descriptor() -> ToolDescriptor {
    ToolDescriptor {
        emits_external: false,
        name: "dashboard.export".to_string(),
        title: "Export a dashboard as Grafana JSON".to_string(),
        group: "dashboard".to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": {
                "id": { "type": "string", "x-lb": { "label": "Dashboard id" } }
            },
            "required": ["id"]
        })),
        result: None,
    }
}
