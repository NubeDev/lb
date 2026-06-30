//! `flows.save` / `flows.get` / `flows.list` / `flows.delete` — the flow CRUD (flows-scope). `save`
//! **validates the DAG up front** (cycle/dangling/dup/self-edge/size — a bad graph is a deny-
//! equivalent before any run) AND **re-validates every node's config against its descriptor's
//! schema** (the `config_version` evolution discipline: a flow-version bump re-checks persisted
//! configs against the current descriptor, blocking the save on a drift — node-descriptor-scope).
//! Editing an existing flow writes a **new version** (Decision 1) — a live run keeps its pinned one.
//!
//! Gated at the bridge (`mcp:flows.<verb>:call`); here the store-write/read surface + validation.

use lb_auth::Principal;
use lb_caps::{check, Action, Decision, Request, Surface};
use lb_flows::{validate_flow, Flow, FlowSummary, MAX_FLOW_NODES};
use lb_store::{read, scan, write, Store};
use serde_json::Value;

use super::error::FlowsError;
use super::nodes::merged_registry_internal;
use super::record::FLOW_TABLE;

/// Persist a flow after validating its DAG + every node config against its descriptor's schema.
/// Editing an existing flow bumps its `version` (Decision 1). Returns the id.
pub async fn flows_save(
    store: &Store,
    principal: &Principal,
    ws: &str,
    flow: &mut Flow,
) -> Result<String, FlowsError> {
    authorize_store_write(principal, ws)?;
    flow.workspace = ws.to_string();
    validate_flow(flow, MAX_FLOW_NODES).map_err(|e| FlowsError::BadInput(e.to_string()))?; // rejected before any run
    validate_node_configs(store, ws, flow).await?;
    // Decision 1: editing writes a new version. An existing flow's version bumps; the live run keeps
    // the version it pinned.
    if let Some(existing) = read_flow_raw(store, ws, &flow.id).await? {
        flow.version = existing.version.saturating_add(1);
    } else if flow.version == 0 {
        flow.version = 1;
    }
    let value = serde_json::to_value(flow).map_err(|e| FlowsError::Internal(e.to_string()))?;
    write(store, ws, FLOW_TABLE, &flow.id, &value)
        .await
        .map_err(|e| FlowsError::Internal(e.to_string()))?;
    Ok(flow.id.clone())
}

/// Re-validate every node's config against its descriptor's schema at save (the config_version
/// evolution gate). A node whose type is unknown (its ext uninstalled) or whose config violates the
/// schema blocks the save with a precise error naming the node + the failing rule.
async fn validate_node_configs(store: &Store, ws: &str, flow: &Flow) -> Result<(), FlowsError> {
    let registry = merged_registry_internal(store, ws)
        .await
        .map_err(|e| FlowsError::Internal(e.to_string()))?;
    for n in &flow.nodes {
        let desc = registry
            .iter()
            .find(|d| d.r#type == n.node_type)
            .ok_or_else(|| FlowsError::BadInput(format!("node `{}`: unknown type `{}` (extension not installed in this workspace)", n.id, n.node_type)))?;
        lb_flows::validate_config(&desc.config, &n.config).map_err(|e| {
            FlowsError::BadInput(format!("node `{}` ({ }): {e}", n.id, n.node_type))
        })?;
    }
    Ok(())
}

/// Delete a flow (tombstone, idempotent). Teardown ordering (disarm sources → cancel runs → drop
/// cron) is the triggers slice's (Decision 13); here the record surface.
pub async fn flows_delete(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
) -> Result<(), FlowsError> {
    authorize_store_write(principal, ws)?;
    if read_flow_raw(store, ws, id).await?.is_none() {
        return Ok(());
    }
    write(
        store,
        ws,
        FLOW_TABLE,
        id,
        &serde_json::json!({ "id": id, "workspace": ws, "deleted": true }),
    )
    .await
    .map_err(|e| FlowsError::Internal(e.to_string()))?;
    Ok(())
}

/// Read one flow by id (skipping a tombstone). Authorized read.
pub async fn flows_get(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
) -> Result<Flow, FlowsError> {
    authorize_store_read(principal, ws)?;
    flows_get_internal(store, ws, id).await
}

/// Internal read (no auth gate — for callers that hold their own authority, e.g. the subflow loader
/// or the run engine). Workspace-scoped by `read`.
pub async fn flows_get_internal(store: &Store, ws: &str, id: &str) -> Result<Flow, FlowsError> {
    let val = read(store, ws, FLOW_TABLE, id)
        .await
        .map_err(|e| FlowsError::Internal(e.to_string()))?
        .ok_or(FlowsError::NotFound)?;
    if val.get("deleted").and_then(|v| v.as_bool()).unwrap_or(false) {
        return Err(FlowsError::NotFound);
    }
    serde_json::from_value(val).map_err(|e| FlowsError::Internal(e.to_string()))
}

/// List flows in the workspace (non-deleted), as compact summaries (the picker).
pub async fn flows_list(
    store: &Store,
    principal: &Principal,
    ws: &str,
) -> Result<Vec<FlowSummary>, FlowsError> {
    authorize_store_read(principal, ws)?;
    let page = scan(store, ws, FLOW_TABLE, lb_store::MAX_SCAN_LIMIT, None)
        .await
        .map_err(|e| FlowsError::Internal(e.to_string()))?;
    let mut out = Vec::new();
    for row in page.rows {
        let inner = match row.data {
            Value::Object(mut o) => o.remove("data").unwrap_or(Value::Null),
            other => other,
        };
        if inner.get("deleted").and_then(|v| v.as_bool()).unwrap_or(false) {
            continue;
        }
        if let Ok(f) = serde_json::from_value::<Flow>(inner) {
            out.push(FlowSummary::from(&f));
        }
    }
    Ok(out)
}

async fn read_flow_raw(store: &Store, ws: &str, id: &str) -> Result<Option<Flow>, FlowsError> {
    match read(store, ws, FLOW_TABLE, id).await {
        Ok(Some(v)) => {
            if v.get("deleted").and_then(|v| v.as_bool()).unwrap_or(false) {
                return Ok(None);
            }
            serde_json::from_value(v).map(Some).map_err(|e| FlowsError::Internal(e.to_string()))
        }
        Ok(None) => Ok(None),
        Err(e) => Err(FlowsError::Internal(e.to_string())),
    }
}

pub fn authorize_store_write(principal: &Principal, ws: &str) -> Result<(), FlowsError> {
    let req = Request::new(ws, Surface::Store, "flow", Action::Write);
    match check(principal, &req) {
        Decision::Allowed => Ok(()),
        Decision::Denied(_) => Err(FlowsError::Denied),
    }
}

pub fn authorize_store_read(principal: &Principal, ws: &str) -> Result<(), FlowsError> {
    let req = Request::new(ws, Surface::Store, "flow", Action::Read);
    match check(principal, &req) {
        Decision::Allowed => Ok(()),
        Decision::Denied(_) => Err(FlowsError::Denied),
    }
}
