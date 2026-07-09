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
use lb_flows::{validate_flow, validate_links, Flow, FlowSummary, MAX_FLOW_NODES};
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
                                                                                           // flow-input-ports-scope Slice 3: validate the wireless link-pair topology (a link-out naming a
                                                                                           // missing link-in, a wire from a link-out, a dead link-in) BEFORE any run. Resolution to physical
                                                                                           // edges happens at run load (`coordinator::resolve_links`); validation is save-time so a typo
                                                                                           // surfaces here, not as a silent no-op at run.
    validate_links(flow).map_err(|e| FlowsError::BadInput(e.to_string()))?;
    validate_node_configs(store, ws, flow).await?;
    // Decision 1: editing writes a new version. An existing flow's version bumps; the live run keeps
    // the version it pinned.
    if let Some(existing) = read_flow_raw(store, ws, &flow.id).await? {
        flow.version = existing.version.saturating_add(1);
    } else if flow.version == 0 {
        flow.version = 1;
    }
    // N independent triggers: a flow may carry ANY number of `mode:"cron"` trigger nodes, each with
    // its own schedule (its `config.cron`). The reactor scans them per-node (each owns its cursor in
    // `flow_trigger_state`), so there is no flow-level schedule to derive and no "one schedule"
    // rejection — we only validate that each spec is a well-formed cron (a bad spec is a clear save
    // error, not a silently-dead trigger). The reactor self-arms each cursor on its next pass.
    validate_cron_triggers(flow)?;
    let value = serde_json::to_value(&*flow).map_err(|e| FlowsError::Internal(e.to_string()))?;
    let id = flow.id.clone();
    write(store, ws, FLOW_TABLE, &id, &value)
        .await
        .map_err(|e| FlowsError::Internal(e.to_string()))?;
    Ok(id)
}

/// Validate every `mode:"cron"` trigger node's `config.cron` is a well-formed 5-field spec. A flow
/// may carry **any number** of cron triggers (each fires independently on its own cursor — there is no
/// "one schedule per flow" wall); we only reject a malformed spec so a typo surfaces at save instead
/// of silently arming nothing. An empty/absent spec on a cron trigger is rejected too (an armed cron
/// node with no schedule is a mistake). The reactor (`react_to_flows_cron`) owns the per-node cursor.
fn validate_cron_triggers(flow: &Flow) -> Result<(), FlowsError> {
    for n in &flow.nodes {
        let is_cron = n.node_type == "trigger"
            && n.config.get("mode").and_then(|v| v.as_str()) == Some("cron");
        if !is_cron {
            continue;
        }
        let spec = n
            .config
            .get("cron")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim();
        if spec.is_empty() {
            return Err(FlowsError::BadInput(format!(
                "node `{}`: a cron trigger needs a non-empty `config.cron` schedule",
                n.id
            )));
        }
        if !super::react_cron::cron_is_valid(spec) {
            return Err(FlowsError::BadInput(format!(
                "node `{}`: invalid cron schedule `{spec}` (expected a 5-field spec)",
                n.id
            )));
        }
    }
    Ok(())
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
            .ok_or_else(|| {
                FlowsError::BadInput(format!(
                    "node `{}`: unknown type `{}` (extension not installed in this workspace)",
                    n.id, n.node_type
                ))
            })?;
        lb_flows::validate_config(&desc.config, &n.config).map_err(|e| {
            FlowsError::BadInput(format!("node `{}` ({ }): {e}", n.id, n.node_type))
        })?;
        // Per-port edge lint (flow-input-ports-scope Axis 1): every wired `to_port` must name a
        // declared input port on this node's descriptor. A wire to an undeclared port is a mistake
        // (a misnamed handle, or a port the node type does not expose) — caught at save, not silently
        // dropped at run. An omitted `to_port` ⇒ the primary input (validated by existence of ≥1
        // input port below when the node has any wired edge).
        for e in &n.inputs {
            let Some(port) = &e.to_port else { continue };
            if !desc.inputs.iter().any(|p| p == port) {
                return Err(FlowsError::BadInput(format!(
                    "node `{}` ({}) wires upstream `{}` to undeclared input port `{}` (declared: [{}])",
                    n.id,
                    n.node_type,
                    e.from,
                    port,
                    desc.inputs.join(", ")
                )));
            }
        }
        // A node with at least one wired edge must declare ≥1 input port (somewhere for the wire to
        // land). A `trigger`/`source` (no inputs) receiving an inbound wire is a topology mistake.
        if !n.needs.is_empty() && desc.inputs.is_empty() {
            return Err(FlowsError::BadInput(format!(
                "node `{}` ({}) has {} incoming wire(s) but declares no input port",
                n.id,
                n.node_type,
                n.needs.len()
            )));
        }
        // Policy-aware join lint (flow-input-ports-scope, replacing envelope D3's node-id-only lint).
        // A multi-wire `all` port is a barrier that must bind `payload` explicitly (the engine cannot
        // know which upstream's message to carry — silently picking one would hide a join bug / drop
        // data). An `any` port with multiple wires is VALID — the funnel fires once per upstream
        // (the whole point). The primary port is the load-bearing one for v1's single-port nodes.
        let primary_policy = desc.join_of(None);
        if n.needs.len() >= 2
            && primary_policy == lb_flows::JoinPolicy::All
            && !n.with.contains_key("payload")
        {
            return Err(FlowsError::BadInput(format!(
                "node `{}` ({}) has {} wires into an `all` (join) input port — bind `payload` explicitly (an `any` funnel fires per upstream and needs no binding)",
                n.id,
                n.node_type,
                n.needs.len()
            )));
        }
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
    if val
        .get("deleted")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
    {
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
    Ok(flows_list_internal(store, ws)
        .await?
        .iter()
        .map(FlowSummary::from)
        .collect())
}

/// Internal list (no auth gate) returning full `Flow`s — for the reactors (cron scan, reconciler)
/// that hold their own authority. Workspace-scoped by `scan`; never another workspace's flows.
pub async fn flows_list_internal(store: &Store, ws: &str) -> Result<Vec<Flow>, FlowsError> {
    let page = scan(store, ws, FLOW_TABLE, lb_store::MAX_SCAN_LIMIT, None)
        .await
        .map_err(|e| FlowsError::Internal(e.to_string()))?;
    let mut out = Vec::new();
    for row in page.rows {
        let inner = match row.data {
            Value::Object(mut o) => o.remove("data").unwrap_or(Value::Null),
            other => other,
        };
        if inner
            .get("deleted")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            continue;
        }
        if let Ok(f) = serde_json::from_value::<Flow>(inner) {
            out.push(f);
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
            serde_json::from_value(v)
                .map(Some)
                .map_err(|e| FlowsError::Internal(e.to_string()))
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
