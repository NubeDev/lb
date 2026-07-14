//! `dashboard.import {json, mappings?, id?, now}` — the Grafana-JSON import verb (viz import-export
//! scope, Phase 4). One verb, two phases (the scope's resolved decision — not a separate preview verb):
//!
//! - **preview** (no `mappings`): migrate → map → return `{report}` with NO write. The report drives
//!   the datasource-remap UI + the degraded list.
//! - **commit** (`mappings` present): re-map with the caller's chosen datasource bindings, verify each
//!   target is a datasource in the CALLER's workspace they can see, build the record, and UPSERT it
//!   through `dashboard_save` (which re-checks `mcp:dashboard.save:call` + every cell validator).
//!
//! Gates: the verb needs BOTH `mcp:dashboard.import:call` (checked here) AND `mcp:dashboard.save:call`
//! (checked by `dashboard_save` on commit) — it creates a dashboard. The **workspace is the caller's
//! token, never the JSON** (an imported `uid`/`org`/`title` carries no authority); every datasource
//! remap resolves strictly within the caller's workspace (the hard wall).

use std::collections::HashMap;

use lb_auth::Principal;
use lb_mcp::ToolDescriptor;

use super::super::authorize::authorize_dashboard;
use super::super::error::DashboardError;
use super::super::model::{Cell, Dashboard, Variable};
use super::super::save::dashboard_save_meta;
use crate::boot::Node;

use super::datasources;
use super::to_cell::panel_to_cell;
use super::{DatasourceRemap, ImportReport};

/// The outcome of an import call — a preview (report only, `id` empty) or a commit (`id` set + the
/// saved dashboard). Serialized as `{ id, report, dashboard? }`.
#[derive(serde::Serialize)]
pub struct ImportOutcome {
    pub id: String,
    pub report: ImportReport,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dashboard: Option<Dashboard>,
}

/// Import a Grafana dashboard JSON. `json` is the raw export; `mappings` (empty = preview) binds each
/// referenced datasource to one of our workspace datasources; `id` is the target dashboard id (a fresh
/// id creates); `now` is logical time. See the module doc for the two phases.
pub async fn dashboard_import(
    node: &Node,
    principal: &Principal,
    ws: &str,
    mut json: serde_json::Value,
    mappings: Vec<DatasourceRemap>,
    id: &str,
    now: u64,
) -> Result<ImportOutcome, DashboardError> {
    // Gate 1+2: workspace-first, then the import capability — before touching the JSON.
    authorize_dashboard(principal, ws, "dashboard.import")?;

    // Stage 1 — migrate. Run the P3 pin: detect v1/v2 (reject v2/snapshot), normalize schemaVersion,
    // resolve `__inputs`. We pass NO input values here — a `${DS_*}` that survives resolution is a
    // datasource the caller remaps below (its uid stays the token, which the remap targets).
    let pin = grafana_map::pin(&mut json, &HashMap::new())
        .map_err(|e| DashboardError::BadInput(format!("Grafana import: {e}")))?;

    let mut report = ImportReport {
        migrated_from: pin.migrate.from_version,
        ..Default::default()
    };
    if let Some(notice) = pin.migrate.degraded {
        report.degraded.push(super::DegradedItem {
            kind: "migration".to_string(),
            cell: String::new(),
            detail: notice,
        });
    }

    // Stage 2 — map panels. A dashboard with no `panels` is a hard error (not a Grafana dashboard).
    let panels = json
        .get("panels")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| DashboardError::BadInput("Grafana JSON has no `panels` array".into()))?
        .clone();
    let mut cells: Vec<Cell> = Vec::with_capacity(panels.len());
    for (i, panel) in panels.iter().enumerate() {
        let cell = panel_to_cell(panel, i, &mut report.degraded);
        cells.push(cell);
    }
    report.mapped_panels = cells.iter().filter(|c| c.view != "json").count();

    // Stage 3 — datasources. Collect every referenced source; the preview lists them for remap.
    report.datasources = datasources::collect(&json);

    // Variables: carried onto the record; unsupported types flagged but preserved (opaque `Value`).
    let variables = map_variables(&json, &mut report.degraded);

    // --- Preview phase: no mappings → return the report, write nothing. ---
    if mappings.is_empty() && !report.datasources.is_empty() {
        return Ok(ImportOutcome {
            id: String::new(),
            report,
            dashboard: None,
        });
    }

    // --- Commit phase. Verify each mapping target is a datasource THIS caller can see in THIS
    // workspace (the hard wall + the "you may only map to a source you hold" rule). A target that is
    // not in the caller's `datasource.list` is refused — a ws-B source is invisible here, so a ws-A
    // import can never bind it. ---
    verify_mappings(node, principal, ws, &mappings).await?;

    // Rewrite the datasource refs in the cells' target args by the verified mappings.
    apply_mappings_to_cells(&mut cells, &mappings, &mut report);
    // Echo the chosen mappings into the report.
    report.datasources = mappings;

    // Build + UPSERT through `dashboard.save` — it re-checks `mcp:dashboard.save:call` and runs EVERY
    // cell validator (bounds incl. the `_grafana` cap, view-name, genui, refs). Title comes from the
    // JSON (a label, not authority); owner/visibility are stamped by save (caller-owned, private).
    let title = json
        .get("title")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("Imported dashboard")
        .to_string();
    let timezone = json
        .get("timezone")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string);

    let dashboard = dashboard_save_meta(
        &node.store,
        principal,
        ws,
        id,
        &title,
        None,
        None,
        None,
        timezone,
        None,
        cells,
        variables,
        now,
    )
    .await?;

    Ok(ImportOutcome {
        id: dashboard.id.clone(),
        report,
        dashboard: Some(dashboard),
    })
}

/// Verify every non-empty `mapped_to` is a datasource visible to `principal` in `ws`. Uses
/// `datasource.list` — which authorizes `mcp:datasource.list:call` + the workspace wall — so a target
/// outside the caller's workspace/reach is simply absent → refused (the tenancy guarantee). A native/
/// series target (a reserved name) is allowed without a datasource row.
async fn verify_mappings(
    node: &Node,
    principal: &Principal,
    ws: &str,
    mappings: &[DatasourceRemap],
) -> Result<(), DashboardError> {
    let known: Vec<String> = crate::datasource_list(node, principal, ws)
        .await
        .map_err(|_| DashboardError::Denied)?
        .into_iter()
        .map(|d| d.name)
        .collect();
    for m in mappings {
        if m.mapped_to.is_empty() || is_reserved_target(&m.mapped_to) {
            continue;
        }
        if !known.contains(&m.mapped_to) {
            // A source not in the caller's workspace/reach — refuse the whole import (the hard wall).
            return Err(DashboardError::Denied);
        }
    }
    Ok(())
}

/// Reserved non-federation remap targets that need no datasource record (native store / live series).
fn is_reserved_target(name: &str) -> bool {
    matches!(name, "native" | "series" | "__expr__")
}

/// Apply the verified mappings to each cell's target-arg datasource refs, and record unmapped uids as
/// degraded (their panels render an honest empty). Reuses the shared tree-rewrite over each source's
/// `args` (which carries the original target object incl. its `datasource`).
fn apply_mappings_to_cells(
    cells: &mut [Cell],
    mappings: &[DatasourceRemap],
    report: &mut ImportReport,
) {
    let mut all_degraded = Vec::new();
    for cell in cells.iter_mut() {
        for src in cell.sources.iter_mut() {
            // Rewrite the top-level Target.datasource…
            let mut wrap = serde_json::json!({ "datasource": src.datasource.clone() });
            all_degraded.extend(datasources::apply(&mut wrap, mappings));
            src.datasource = wrap
                .get("datasource")
                .cloned()
                .unwrap_or(serde_json::Value::Null);
            // …and any datasource nested inside the opaque args (the original target object).
            all_degraded.extend(datasources::apply(&mut src.args, mappings));
        }
    }
    // Fold in unique unmapped-datasource notices.
    all_degraded.sort_by(|a, b| a.detail.cmp(&b.detail));
    all_degraded.dedup_by(|a, b| a.detail == b.detail);
    report.degraded.extend(all_degraded);
}

/// Map `templating.list[]` onto our variables — carried as opaque `Value` (the UI owns the typed vars
/// shape). An unsupported variable `type` is preserved and flagged, never dropped.
fn map_variables(
    json: &serde_json::Value,
    degraded: &mut Vec<super::DegradedItem>,
) -> Vec<Variable> {
    const SUPPORTED: &[&str] = &[
        "query",
        "custom",
        "constant",
        "textbox",
        "interval",
        "datasource",
        "adhoc",
    ];
    let list = json
        .get("templating")
        .and_then(|t| t.get("list"))
        .and_then(serde_json::Value::as_array);
    let Some(list) = list else {
        return Vec::new();
    };
    let mut out = Vec::with_capacity(list.len());
    for v in list {
        let ty = v
            .get("type")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("");
        let name = v
            .get("name")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("");
        if !SUPPORTED.contains(&ty) {
            degraded.push(super::DegradedItem {
                kind: "variable".to_string(),
                cell: name.to_string(),
                detail: format!("unsupported variable type '{ty}' — preserved, not resolved"),
            });
        }
        // Deserialize leniently onto our Variable (serde-default fields absorb Grafana's extra keys).
        if let Ok(var) = serde_json::from_value::<Variable>(v.clone()) {
            out.push(var);
        }
    }
    out
}

/// The `dashboard.import` descriptor — a real arg schema so an AI author can form the call.
pub fn import_descriptor() -> ToolDescriptor {
    ToolDescriptor {
        emits_external: false,
        name: "dashboard.import".to_string(),
        title: "Import a Grafana dashboard JSON (preview, then commit with datasource mappings)"
            .to_string(),
        group: "dashboard".to_string(),
        input_schema: Some(serde_json::json!({
            "type": "object",
            "properties": {
                "json": { "type": "object", "x-lb": { "label": "Grafana JSON", "description": "A Grafana dashboard export object (classic schemaVersion model; v2 app-platform is rejected)" } },
                "mappings": { "type": "array", "items": { "type": "object" }, "x-lb": { "label": "Datasource mappings", "description": "Omit for a PREVIEW (returns a report with the datasources to bind). On COMMIT, an array of { type, uid, mappedTo } binding each referenced datasource to one of THIS workspace's datasources" } },
                "id": { "type": "string", "x-lb": { "label": "Dashboard id", "description": "Target id — a fresh id creates a new dashboard (commit phase only)" } },
                "now": { "type": "integer", "x-lb": { "label": "Timestamp", "description": "Logical time — unix epoch seconds" } }
            },
            "required": ["json", "now"]
        })),
        result: None,
    }
}
