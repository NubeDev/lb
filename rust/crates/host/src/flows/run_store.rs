//! The durable run-store over SurrealDB — the flow engine's backend (flow-run-scope "Data"; the
//! run-store shape ported from `rubix-cube` via the retired chain engine). The CAS claim
//! (`Pending|Enqueued → Running`)
//! is the **cross-node** exactly-once owner under redelivery (Decision 8); a lost claim no-ops, so a
//! duplicate node redelivery never double-runs. Per-node rows so concurrent branch jobs don't
//! contend, and a restart resumes from the recorded state (the headline offline/sync property).

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use lb_flows::{resolve_bindings, Flow, NodeOutput};
use lb_store::{read, scan, write_locked as write, Store};
use serde_json::{json, Value};
use tokio::sync::Mutex as AsyncMutex;

use super::record::{
    step_record_id, ClaimState, FlowRunRecord, FlowStepRecord, FLOW_INPUT_TABLE,
    FLOW_NODE_STATE_TABLE, FLOW_RUN_TABLE, FLOW_STEP_TABLE,
};

/// Per-`(ws,run_id)` lock guarding the create-if-absent seed in [`create_run`], so two concurrent
/// `start`s of the same run id can't both pass the existence check and double-seed. In-process: a
/// run is owned + resumed by one node. Cross-record `rev` safety is the store's `write_locked`; this
/// lock is specifically the check-then-seed window.
fn seed_lock(ws: &str, run_id: &str) -> Arc<AsyncMutex<()>> {
    static LOCKS: OnceLock<Mutex<HashMap<String, Arc<AsyncMutex<()>>>>> = OnceLock::new();
    let map = LOCKS.get_or_init(|| Mutex::new(HashMap::new()));
    let composite = format!("{ws}\u{1}{run_id}");
    let mut guard = map.lock().expect("flows seed-lock map poisoned");
    guard
        .entry(composite)
        .or_insert_with(|| Arc::new(AsyncMutex::new(())))
        .clone()
}

/// Seed a run: the coordinator record (pending, pinned `flow_version`) + a per-node state row (claim
/// from in-degree). The pinned version is the spine of resume (Decision 1).
pub async fn create_run(
    store: &Store,
    ws: &str,
    run_id: &str,
    flow: &Flow,
    params: &serde_json::Map<String, Value>,
    now: u64,
    entry: Option<&str>,
) -> Result<(), String> {
    // Idempotent seed (create-if-absent): a second concurrent `start` of the same run id must NOT
    // re-write the coordinator + step rows — that re-seed is what raced the monotonic `rev` and could
    // clobber an in-flight run's progress back to `pending`. The seed is keyed on the run record's
    // existence; the per-record `write_locked` lock makes this check-then-seed safe against a sibling
    // seeder (one wins the lock, seeds; the other reads the now-present run and no-ops).
    let lock = seed_lock(ws, run_id);
    let _guard = lock.lock().await;
    if read_run(store, ws, run_id).await?.is_some() {
        return Ok(());
    }
    let run = FlowRunRecord {
        run_id: run_id.to_string(),
        flow_id: flow.id.clone(),
        flow_version: flow.version,
        status: "pending".into(),
        params: json!(params),
        ts: now,
        entry_node: entry.map(str::to_string),
    };
    write(
        store,
        ws,
        FLOW_RUN_TABLE,
        run_id,
        &serde_json::to_value(&run).map_err(|e| e.to_string())?,
    )
    .await
    .map_err(|e| e.to_string())?;
    // Per-wire (Node-RED/FBP) seeding: a run fired FROM a trigger seeds only the subgraph reachable
    // from it, with indegrees counted within that subgraph (a join waits only on its in-subgraph
    // upstreams). `entry=None` keeps the whole-graph seed (manual "run all", resume, subflow). Nodes
    // outside the set get NO step record — they are not part of this run (the wire never carried a
    // message to them), so the drive/finalize loops simply never see them.
    let set = run_node_set_for(flow, entry);
    let indeg = match entry {
        Some(_) => flow.indegrees_within(&set),
        None => flow.indegrees(),
    };
    for n in &flow.nodes {
        if !set.contains(&n.id) {
            continue;
        }
        let d = indeg[&n.id];
        let rec = FlowStepRecord {
            run_id: run_id.to_string(),
            node_id: n.id.clone(),
            claim: if d == 0 {
                ClaimState::Enqueued
            } else {
                ClaimState::Pending
            },
            indegree: d,
            outcome: String::new(),
            output: Value::Null,
            findings: Value::Null,
            error: None,
            attempts: 0,
            ms: 0,
            patched_config: None,
        };
        write_step(store, ws, &rec).await?;
    }
    Ok(())
}

/// The set of node ids a run executes: the subgraph reachable from `entry` (per-wire firing), or
/// **every** node when `entry` is `None` (whole-graph run). The single source of truth for which
/// nodes `create_run` seeds and `finalize_if_complete` waits on — so the two never disagree.
fn run_node_set_for(flow: &Flow, entry: Option<&str>) -> std::collections::HashSet<String> {
    match entry {
        Some(e) => flow.reachable_from(e),
        None => flow.nodes.iter().map(|n| n.id.clone()).collect(),
    }
}

/// The node set of an EXISTING run, recovered from its persisted `entry_node` (so drive/finalize on a
/// resumed run scope to the same subgraph the seed used).
pub async fn run_node_set(
    store: &Store,
    ws: &str,
    flow: &Flow,
    run_id: &str,
) -> Result<std::collections::HashSet<String>, String> {
    let entry = read_run(store, ws, run_id)
        .await?
        .and_then(|r| r.entry_node);
    Ok(run_node_set_for(flow, entry.as_deref()))
}

/// CAS claim a node: `Pending|Enqueued → Running`. Returns true if THIS call won the claim.
pub async fn claim_step(
    store: &Store,
    ws: &str,
    run_id: &str,
    node_id: &str,
) -> Result<bool, String> {
    let Some(mut rec) = read_step(store, ws, run_id, node_id).await? else {
        return Ok(false);
    };
    match rec.claim {
        ClaimState::Pending | ClaimState::Enqueued => {
            rec.claim = ClaimState::Running;
            write_step(store, ws, &rec).await?;
            Ok(true)
        }
        _ => Ok(false),
    }
}

/// Record a node's terminal outcome (idempotent) + upsert `flow_node_state` last-value on Ok
/// (Decision 5 — the dashboard instant read; history is the node's series, not this record).
///
/// On `Ok`, the recorded value is the **message envelope** (flow-message-envelope-scope D1): the
/// `carry` map (the incoming inputs minus `payload`, so `topic` and friends propagate down a linear
/// chain — D4) merged under the node's `emitted` fields (which always include a fresh `payload`).
/// A join (`carry` empty) records just `emitted`. `flow_node_state` stores the whole envelope (D9).
pub async fn record_outcome(
    store: &Store,
    ws: &str,
    flow_id: &str,
    run_id: &str,
    node_id: &str,
    outcome: NodeOutcome,
) -> Result<(), String> {
    let Some(mut state) = read_step(store, ws, run_id, node_id).await? else {
        return Ok(());
    };
    state.claim = ClaimState::Done;
    match outcome {
        NodeOutcome::Ok { emitted, carry } => {
            // D4 carry-forward: `{ ...carry, ...emitted }` — emitted wins on a key collision (a node
            // overwrites a carried field, e.g. setting its own `topic`).
            let mut envelope = match carry {
                Value::Object(m) => m,
                _ => serde_json::Map::new(),
            };
            if let Value::Object(e) = emitted {
                for (k, v) in e {
                    envelope.insert(k, v);
                }
            }
            let envelope = Value::Object(envelope);
            state.outcome = "ok".into();
            state.output = envelope.clone();
            // Decision 5/9: last-value envelope for the instant dashboard read.
            write(
                store,
                ws,
                FLOW_NODE_STATE_TABLE,
                &format!("{flow_id}:{node_id}"),
                &envelope,
            )
            .await
            .map_err(|e| e.to_string())?;
        }
        NodeOutcome::Err(e) => {
            state.outcome = "err".into();
            state.error = Some(e);
        }
        NodeOutcome::Skipped => {
            state.outcome = "skipped".into();
        }
    }
    write_step(store, ws, &state).await
}

/// The terminal outcome a node's executor reports. `Ok` carries the node's freshly-`emitted` envelope
/// fields and the `carry` map (incoming inputs minus `payload`) to merge forward (D4).
#[allow(dead_code)]
pub enum NodeOutcome {
    Ok { emitted: Value, carry: Value },
    Err(String),
    Skipped,
}

impl NodeOutcome {
    /// Convenience: an `Ok` with no carry-forward (a join, or a node that synthesises its envelope).
    pub fn ok(emitted: Value) -> Self {
        NodeOutcome::Ok {
            emitted,
            carry: Value::Null,
        }
    }
}

/// Park a claimed node back to `Enqueued` without recording a terminal outcome — the durable-delay
/// suspend seam (Decision 16). A `delay` whose timer has not elapsed calls this: the node is re-driven
/// from `Enqueued` on the next `flows.resume` (with an advanced clock), never double-settling. A
/// no-op if the step is already terminal.
pub async fn park_step(store: &Store, ws: &str, run_id: &str, node_id: &str) -> Result<(), String> {
    let Some(mut rec) = read_step(store, ws, run_id, node_id).await? else {
        return Ok(());
    };
    if rec.claim == ClaimState::Done {
        return Ok(());
    }
    rec.claim = ClaimState::Enqueued;
    write_step(store, ws, &rec).await
}

/// Decrement dependents' in-degree; return those that reached 0 (marked Enqueued).
pub async fn ready_dependents(
    store: &Store,
    ws: &str,
    flow: &Flow,
    run_id: &str,
    finished: &str,
) -> Result<Vec<String>, String> {
    let dependents = flow.dependents();
    let deps = dependents.get(finished).cloned().unwrap_or_default();
    let mut ready = Vec::new();
    for dep in deps {
        let Some(mut rec) = read_step(store, ws, run_id, &dep).await? else {
            continue;
        };
        if rec.claim != ClaimState::Pending {
            rec.indegree = rec.indegree.saturating_sub(1);
            write_step(store, ws, &rec).await?;
            continue;
        }
        rec.indegree = rec.indegree.saturating_sub(1);
        if rec.indegree == 0 {
            rec.claim = ClaimState::Enqueued;
            ready.push(dep);
        }
        write_step(store, ws, &rec).await?;
    }
    Ok(ready)
}

/// Release exactly **one** dependent by decrementing its in-degree (enqueue at 0) — the selective
/// counterpart of [`ready_dependents`], used by `switch` edge-gating (Decision 14) to fire only the
/// dependents a matched rule named. Idempotent: a non-`Pending` dependent is only decremented.
pub async fn ready_one_dependent(
    store: &Store,
    ws: &str,
    run_id: &str,
    dep: &str,
) -> Result<(), String> {
    let Some(mut rec) = read_step(store, ws, run_id, dep).await? else {
        return Ok(());
    };
    rec.indegree = rec.indegree.saturating_sub(1);
    if rec.claim == ClaimState::Pending && rec.indegree == 0 {
        rec.claim = ClaimState::Enqueued;
    }
    write_step(store, ws, &rec).await
}

/// Gate a node and its exclusive subtree as Skipped — a `switch` unmatched branch (Decision 14) or a
/// suppressed stateful node's downstream. Marks `gated` itself Skipped (if not yet terminal), then
/// cascades through its dependents (the `skip_subtree` walk seeded at `gated`). A node already
/// Running/Done is left as-is (a live path reached it first — the disjoint-branch assumption).
pub async fn skip_gated(
    store: &Store,
    ws: &str,
    flow: &Flow,
    run_id: &str,
    gated: &str,
) -> Result<(), String> {
    if let Some(mut rec) = read_step(store, ws, run_id, gated).await? {
        if matches!(rec.claim, ClaimState::Pending | ClaimState::Enqueued) {
            rec.claim = ClaimState::Done;
            rec.outcome = "skipped".into();
            write_step(store, ws, &rec).await?;
        }
    }
    skip_subtree(store, ws, flow, run_id, gated).await
}

/// Mark the transitive subtree below a failed node as Skipped (Halt policy).
pub async fn skip_subtree(
    store: &Store,
    ws: &str,
    flow: &Flow,
    run_id: &str,
    failed: &str,
) -> Result<(), String> {
    let dependents = flow.dependents();
    let mut queue: std::collections::VecDeque<String> =
        dependents.get(failed).cloned().unwrap_or_default().into();
    let mut seen = std::collections::HashSet::new();
    while let Some(id) = queue.pop_front() {
        if !seen.insert(id.clone()) {
            continue;
        }
        if let Some(mut rec) = read_step(store, ws, run_id, &id).await? {
            if matches!(rec.claim, ClaimState::Pending | ClaimState::Enqueued) {
                rec.claim = ClaimState::Done;
                rec.outcome = "skipped".into();
                write_step(store, ws, &rec).await?;
            }
        }
        if let Some(next) = dependents.get(&id) {
            for n in next {
                queue.push_back(n.clone());
            }
        }
    }
    Ok(())
}

/// If every node is Done, write the terminal run status. Returns the new status string if finalised.
pub async fn finalize_if_complete(
    store: &Store,
    ws: &str,
    flow: &Flow,
    run_id: &str,
) -> Result<Option<String>, String> {
    let mut any_failed = false;
    let mut any_ok = false;
    // Only the run's OWN node set must be terminal — a per-trigger run finalises when ITS subgraph is
    // done, never waiting on out-of-subgraph nodes (which carry no step record this run).
    let set = run_node_set(store, ws, flow, run_id).await?;
    for n in flow.nodes.iter().filter(|n| set.contains(&n.id)) {
        let Some(rec) = read_step(store, ws, run_id, &n.id).await? else {
            return Ok(None);
        };
        if rec.claim != ClaimState::Done {
            return Ok(None);
        }
        match rec.outcome.as_str() {
            "ok" => any_ok = true,
            "err" => any_failed = true,
            _ => {}
        }
    }
    let status = if any_failed && any_ok {
        "partialFailure"
    } else if any_failed {
        "failed"
    } else {
        "success"
    };
    set_run_status(store, ws, run_id, status).await?;
    Ok(Some(status.to_string()))
}

/// A node's resolved inputs (its incoming message, D2) plus the `carry` map to merge forward (D4).
pub struct ResolvedInputs {
    /// The node's incoming `msg` — the map every builtin reads `payload`/`topic` from.
    pub inputs: serde_json::Map<String, Value>,
    /// The fields to carry forward onto the node's output envelope (inputs minus `payload`). Empty
    /// for a join (≥2 upstreams) — no ambiguous merge (D4).
    pub carry: serde_json::Map<String, Value>,
}

/// Resolve a node's incoming message (flow-message-envelope-scope D2/D3). Auto-wire: if the node has
/// **exactly one** `needs` upstream and its `with` does NOT bind `payload`, `inputs` = that upstream's
/// **full recorded envelope** (a copy — Node-RED "drag a wire and it flows"). With an explicit `with`,
/// `inputs` is built from the bindings only (no auto). With ≥2 upstreams and no `with` (a join), the
/// save-time lint already rejected it; defensively `inputs` is empty here.
///
/// `carry` (D4) = `inputs` minus `payload`, but only when `inputs` came from a single upstream OR a
/// single explicit `payload` binding (so `topic` propagates); a multi-input join carries nothing.
pub async fn resolve_node_bindings(
    store: &Store,
    ws: &str,
    flow: &Flow,
    run_id: &str,
    node_id: &str,
    with: &serde_json::Map<String, Value>,
    params: &serde_json::Map<String, Value>,
) -> Result<ResolvedInputs, String> {
    let mut recorded = HashMap::new();
    for n in &flow.nodes {
        if let Some(rec) = read_step(store, ws, run_id, &n.id).await? {
            if rec.claim == ClaimState::Done && rec.outcome == "ok" {
                recorded.insert(n.id.clone(), NodeOutput::new(rec.output));
            }
        }
    }
    let needs = flow
        .node(node_id)
        .map(|n| n.needs.as_slice())
        .unwrap_or(&[]);
    let binds_payload = with.contains_key("payload");

    // D3 auto-wire: single upstream, no explicit `payload` binding → copy the upstream's envelope.
    if needs.len() == 1 && !binds_payload {
        let up = &needs[0];
        let envelope = recorded
            .get(up)
            .map(|r| r.envelope.clone())
            .unwrap_or(Value::Null);
        let inputs = match envelope {
            Value::Object(m) => m,
            // The upstream produced a non-object (a `Continue`-null, say) — wrap it as a payload so
            // the node still reads a well-formed message.
            other => {
                let mut m = serde_json::Map::new();
                m.insert("payload".into(), other);
                m
            }
        };
        // Still apply any non-`payload` explicit bindings on top (e.g. a hand-set `topic`).
        let mut inputs = inputs;
        let bound = resolve_bindings(with, &recorded, params).map_err(|e| e.to_string())?;
        for (k, v) in bound {
            inputs.insert(k, v);
        }
        // Retained `flow_input` wins over auto-wire too (precedence: per-port > node-level > wire).
        overlay_retained_inputs(store, ws, &flow.id, node_id, &mut inputs).await?;
        let carry = without_payload(&inputs);
        return Ok(ResolvedInputs { inputs, carry });
    }

    // Explicit bindings (or a no-upstream node): build `inputs` from `with` only.
    let mut inputs = resolve_bindings(with, &recorded, params).map_err(|e| e.to_string())?;
    overlay_retained_inputs(store, ws, &flow.id, node_id, &mut inputs).await?;
    // Carry forward only when this is NOT a multi-input join (D4): a single (or zero) upstream, or a
    // single explicit `payload` binding.
    let carry = if needs.len() >= 2 {
        serde_json::Map::new()
    } else {
        without_payload(&inputs)
    };
    Ok(ResolvedInputs { inputs, carry })
}

/// Overlay this node's retained `flow_input` onto its resolved `inputs`, establishing the binding
/// precedence **per-port retained > node-level retained > static `with`/auto-wire** (flow-dashboard-
/// binding-ux-scope, ratified in flow-run-scope). The node-level retained value is the node's
/// `payload`; a per-port record sets that named port (and wins over the node-level value when both
/// target `payload`). A run reads the CURRENT retained value, so a control's inject always takes for
/// the next run — the "value didn't take" trap closed.
async fn overlay_retained_inputs(
    store: &Store,
    ws: &str,
    flow_id: &str,
    node_id: &str,
    inputs: &mut serde_json::Map<String, Value>,
) -> Result<(), String> {
    // Node-level retained (`flow_input:{flow}:{node}`) → the node's `payload`.
    if let Some(rec) = read(store, ws, FLOW_INPUT_TABLE, &format!("{flow_id}:{node_id}"))
        .await
        .map_err(|e| e.to_string())?
    {
        if let Some(val) = retained_value(&rec) {
            inputs.insert("payload".into(), val);
        }
    }
    // Per-port retained (`flow_input:{flow}:{node}:{port}`) → that named port; wins over node-level.
    // The records share the table, so we scan and filter by this node's `{flow}:{node}:` prefix on
    // the record id, reading the authoritative `port`/`value` off the stored body.
    let page = scan(store, ws, FLOW_INPUT_TABLE, lb_store::MAX_SCAN_LIMIT, None)
        .await
        .map_err(|e| e.to_string())?;
    let prefix = format!("{FLOW_INPUT_TABLE}:{flow_id}:{node_id}:");
    for row in page.rows {
        if !row.id.starts_with(&prefix) {
            continue;
        }
        let body = match &row.data {
            Value::Object(o) => o.get("data").cloned().unwrap_or(Value::Null),
            other => other.clone(),
        };
        let (Some(port), Some(val)) = (
            body.get("port").and_then(|v| v.as_str()),
            body.get("value").cloned(),
        ) else {
            continue;
        };
        inputs.insert(port.to_string(), val);
    }
    Ok(())
}

/// Pull the retained `value` off a `flow_input` record body (it lives under the store's `data`
/// envelope for a `read`).
fn retained_value(rec: &Value) -> Option<Value> {
    let body = match rec {
        Value::Object(o) => o.get("data").cloned().unwrap_or_else(|| rec.clone()),
        other => other.clone(),
    };
    body.get("value").cloned()
}

/// A copy of `inputs` with the `payload` key removed — the fields that carry forward (D4).
fn without_payload(inputs: &serde_json::Map<String, Value>) -> serde_json::Map<String, Value> {
    inputs
        .iter()
        .filter(|(k, _)| k.as_str() != "payload")
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

/// Read every retained `flow_input` for a flow and merge into `params` (Decision 9: a run reads the
/// current retained values; an inject into a retained node updates state and starts no run).
pub async fn merged_params_with_inputs(
    store: &Store,
    ws: &str,
    flow_id: &str,
    mut params: serde_json::Map<String, Value>,
) -> Result<serde_json::Map<String, Value>, String> {
    let page = scan(store, ws, FLOW_INPUT_TABLE, lb_store::MAX_SCAN_LIMIT, None)
        .await
        .map_err(|e| e.to_string())?;
    let prefix = format!("{flow_id}:");
    for row in page.rows {
        // flow_input ids are `{flow}:{node}`; pull this flow's retained values into params by node id.
        let inner = match row.data {
            Value::Object(mut o) => o.remove("data").unwrap_or(Value::Null),
            other => other,
        };
        // The row id isn't on the value; recover the node id from the value's `node` field (written
        // by flows.inject). Skip rows not belonging to this flow.
        if inner.get("flow").and_then(|v| v.as_str()) == Some(flow_id) {
            if let Some(node) = inner.get("node").and_then(|v| v.as_str()) {
                if let Some(val) = inner.get("value") {
                    params.insert(node.to_string(), val.clone());
                }
            }
        }
        let _ = prefix; // (prefix retained for clarity; the flow field is the authoritative filter)
    }
    Ok(params)
}

/// Read the run coordinator record.
pub async fn read_run(
    store: &Store,
    ws: &str,
    run_id: &str,
) -> Result<Option<FlowRunRecord>, String> {
    match read(store, ws, FLOW_RUN_TABLE, run_id).await {
        Ok(Some(v)) => serde_json::from_value(v)
            .map(Some)
            .map_err(|e| e.to_string()),
        Ok(None) => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

pub async fn read_step(
    store: &Store,
    ws: &str,
    run_id: &str,
    node_id: &str,
) -> Result<Option<FlowStepRecord>, String> {
    let id = step_record_id(run_id, node_id);
    match read(store, ws, FLOW_STEP_TABLE, &id).await {
        Ok(Some(v)) => serde_json::from_value(v)
            .map(Some)
            .map_err(|e| e.to_string()),
        Ok(None) => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

/// Set the run's lifecycle status.
pub async fn set_run_status(
    store: &Store,
    ws: &str,
    run_id: &str,
    status: &str,
) -> Result<(), String> {
    let Some(mut run) = read_run(store, ws, run_id).await? else {
        return Ok(());
    };
    run.status = status.to_string();
    write(
        store,
        ws,
        FLOW_RUN_TABLE,
        run_id,
        &serde_json::to_value(&run).map_err(|e| e.to_string())?,
    )
    .await
    .map_err(|e| e.to_string())
}

async fn write_step(store: &Store, ws: &str, rec: &FlowStepRecord) -> Result<(), String> {
    let id = step_record_id(&rec.run_id, &rec.node_id);
    write(
        store,
        ws,
        FLOW_STEP_TABLE,
        &id,
        &serde_json::to_value(rec).map_err(|e| e.to_string())?,
    )
    .await
    .map_err(|e| e.to_string())
}
