//! The durable run-store over SurrealDB — the flow engine's backend (flow-run-scope "Data"; the
//! run-store shape ported from `rubix-cube` via the retired chain engine). The CAS claim
//! (`Pending|Enqueued → Running`)
//! is the **cross-node** exactly-once owner under redelivery (Decision 8); a lost claim no-ops, so a
//! duplicate node redelivery never double-runs. Per-node rows so concurrent branch jobs don't
//! contend, and a restart resumes from the recorded state (the headline offline/sync property).

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use lb_flows::{resolve_bindings, Flow, NodeOutput, FCTX_FIELD};
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

/// Seed a run: the coordinator record (pending, pinned `flow_version`) + a step row for each
/// **frontier** (in-degree-0 in-subgraph) node, keyed `(node, "")` Enqueued. Non-frontier nodes are
/// NOT pre-seeded — their `(node, fctx)` slots are minted dynamically by [`release_dependents`] as
/// upstreams settle (a barrier slot when its indegree hits 0; an `any`-funnel firing per settled
/// upstream). The pinned version is the spine of resume (Decision 1).
///
/// For a plain linear flow (empty `fctx` throughout) this is byte-for-byte the pre-ports end state: every node settles once at
/// `fctx=""` under the key `{run}:{node}`. The only difference is *when* a non-frontier record
/// appears (on first release, not at seed) — never observable in a terminal snapshot, and no test
/// asserts a mid-run pending non-frontier node.
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
    // from it. Frontier nodes (in-degree 0 within the subgraph) start `Enqueued` at `fctx=""`;
    // everything else is minted dynamically by `release_dependents` as the run drives.
    let set = run_node_set_for(flow, entry);
    let indeg = match entry {
        Some(_) => flow.indegrees_within(&set),
        None => flow.indegrees(),
    };
    for n in &flow.nodes {
        if !set.contains(&n.id) {
            continue;
        }
        if indeg[&n.id] != 0 {
            continue; // a non-frontier node — minted on release, not here
        }
        let rec = FlowStepRecord {
            run_id: run_id.to_string(),
            node_id: n.id.clone(),
            claim: ClaimState::Enqueued,
            indegree: 0,
            outcome: String::new(),
            output: Value::Null,
            findings: Value::Null,
            error: None,
            attempts: 0,
            ms: 0,
            patched_config: None,
            fctx: String::new(),
            triggered_by: None,
            parent_fctx: None,
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

/// CAS claim a `(node, fctx)` slot: `Pending|Enqueued → Running`. Returns true if THIS call won the
/// claim. The deterministic `fctx` makes a redelivered message re-mint the same slot id and re-claim
/// it (a no-op) — exactly-once per firing, one hop or ten past the funnel (Decision 8, now keyed on
/// the slot).
pub async fn claim_step(
    store: &Store,
    ws: &str,
    run_id: &str,
    node_id: &str,
    fctx: &str,
) -> Result<bool, String> {
    let Some(mut rec) = read_step(store, ws, run_id, node_id, fctx).await? else {
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

/// Record a `(node, fctx)` slot's terminal outcome (idempotent) + upsert `flow_node_state` last-value
/// on Ok (Decision 5 — the dashboard instant read; history is the node's series, not this record).
///
/// On `Ok`, the recorded value is the **message envelope** (flow-message-envelope-scope D1): the
/// `carry` map (the incoming inputs minus `payload`, so `topic` and friends propagate down a linear
/// chain — D4) merged under the node's `emitted` fields (which always include a fresh `payload`).
/// A join (`carry` empty) records just `emitted`. The envelope also carries the slot's `fctx`
/// (flow-input-ports-scope) so a downstream binding resolves the matching settle. `flow_node_state`
/// stores the whole envelope (D9); for an `any`-funnel firing the last-value is the last firing's.
pub async fn record_outcome(
    store: &Store,
    ws: &str,
    flow_id: &str,
    run_id: &str,
    node_id: &str,
    fctx: &str,
    outcome: NodeOutcome,
) -> Result<(), String> {
    let Some(mut state) = read_step(store, ws, run_id, node_id, fctx).await? else {
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
            // flow-input-ports-scope: stamp the firing context into the envelope so a downstream
            // binding `${steps.<this>}` resolves THIS firing's settle (empty in the common case ⇒ a
            // no-op field for back-compat of a non-fctx-aware reader).
            envelope.insert(FCTX_FIELD.into(), Value::String(fctx.to_string()));
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

/// Park a claimed `(node, fctx)` slot back to `Enqueued` without recording a terminal outcome — the
/// durable-delay suspend seam (Decision 16). A `delay` whose timer has not elapsed calls this: the
/// slot is re-driven from `Enqueued` on the next `flows.resume` (with an advanced clock), never
/// double-settling. A no-op if the slot is already terminal.
pub async fn park_step(
    store: &Store,
    ws: &str,
    run_id: &str,
    node_id: &str,
    fctx: &str,
) -> Result<(), String> {
    let Some(mut rec) = read_step(store, ws, run_id, node_id, fctx).await? else {
        return Ok(());
    };
    if rec.claim == ClaimState::Done {
        return Ok(());
    }
    rec.claim = ClaimState::Enqueued;
    write_step(store, ws, &rec).await
}

/// Release the dependents of a settled `(node, fctx)` slot, **per input-port join policy**
/// (flow-plain-wiring-scope — every port defaults to `any`). Each in-subgraph dependent `D` wired
/// from `finished` goes through [`release_one_dependent`] (the ONE policy-aware release seam — the
/// `switch` matched-release path uses it too, so a matched release can never bypass the policy).
pub async fn release_dependents(
    store: &Store,
    ws: &str,
    flow: &Flow,
    run_id: &str,
    finished: &str,
    fctx: &str,
    policies: &HashMap<String, lb_flows::NodeDescriptor>,
    subgraph: &std::collections::HashSet<String>,
) -> Result<(), String> {
    let dependents = flow.dependents();
    let deps = dependents.get(finished).cloned().unwrap_or_default();
    for dep in deps {
        release_one_dependent(
            store, ws, flow, run_id, &dep, finished, fctx, policies, subgraph,
        )
        .await?;
    }
    Ok(())
}

/// Release ONE dependent `dep` of a settled `(finished, fctx)` slot, per `dep`'s input-port join
/// policy — the single policy-aware release seam (flow-plain-wiring-scope; the `switch`
/// matched-release fix routes through here so a matched release honours the policy too):
/// - **`any` port** (the universal default): a per-message firing. A **single-wire** port
///   PROPAGATES the incoming `fctx` unchanged (a linear chain never grows its lineage — keys stay
///   byte-identical to the pre-ports engine); a **multi-wire** port MINTS a new firing id
///   `mint(dep, finished, fctx)` so sibling wires get distinct slots. Either way the slot records
///   `triggered_by = finished` + `parent_fctx = fctx` (the auto-wire), deterministically keyed so a
///   redelivered upstream re-mints the same slot (a no-op).
/// - **explicit-`all` port** (a descriptor opt-in — no built-in declares it): today's barrier —
///   touch the `(dep, fctx)` slot, create Pending at the port's in-subgraph indegree, decrement,
///   Enqueue at 0.
///
/// Out-of-subgraph dependents are not released (a per-trigger run never fires them).
#[allow(clippy::too_many_arguments)]
pub async fn release_one_dependent(
    store: &Store,
    ws: &str,
    flow: &Flow,
    run_id: &str,
    dep: &str,
    finished: &str,
    fctx: &str,
    policies: &HashMap<String, lb_flows::NodeDescriptor>,
    subgraph: &std::collections::HashSet<String>,
) -> Result<(), String> {
    if !subgraph.contains(dep) {
        return Ok(());
    }
    let Some(dep_node) = flow.node(dep) else {
        return Ok(());
    };
    let desc = policies.get(dep);
    let port = dep_node.to_port_from(finished);
    let policy = desc
        .map(|d| d.join_of(port.as_deref()))
        .unwrap_or(lb_flows::JoinPolicy::Any);
    match policy {
        lb_flows::JoinPolicy::All => {
            touch_barrier_slot(store, ws, flow, run_id, dep, fctx, subgraph).await?;
        }
        lb_flows::JoinPolicy::Any => {
            let wires = port_wire_count(dep_node, port.as_deref(), desc, subgraph);
            let new_fctx = if wires <= 1 {
                // Single-wire port: propagate — no lineage growth on a linear chain.
                fctx.to_string()
            } else {
                lb_flows::firing_context::mint(dep, finished, fctx)
            };
            mint_firing(store, ws, run_id, dep, &new_fctx, finished, fctx).await?;
        }
    }
    Ok(())
}

/// The number of in-subgraph wires landing on the SAME input port of `dep_node` as the edge whose
/// `to_port` is `port`. Ports are compared by their **effective** name (an omitted `to_port`
/// resolves to the descriptor's primary input), so `None` and an explicit primary name count as one
/// port. This is what decides propagate (1 wire) vs mint (≥2 wires) for an `any` release.
fn port_wire_count(
    dep_node: &lb_flows::Node,
    port: Option<&str>,
    desc: Option<&lb_flows::NodeDescriptor>,
    subgraph: &std::collections::HashSet<String>,
) -> usize {
    let primary = desc.and_then(|d| d.primary_input());
    let effective = |p: Option<&str>| p.or(primary).map(str::to_string);
    let this_port = effective(port);
    dep_node
        .needs
        .iter()
        .filter(|up| subgraph.contains(*up))
        .filter(|up| effective(dep_node.to_port_from(up).as_deref()) == this_port)
        .count()
}

/// Decrement an `all`-port dependent's `(node, fctx)` barrier slot; Enqueue at 0. Creates the slot
/// Pending (with the node's in-subgraph barrier indegree) on first touch — the dynamic counterpart of
/// today's pre-seed, so an explicit-`all` barrier's slots appear exactly as before, just on first release
/// rather than at seed (byte-identical keys + end state).
async fn touch_barrier_slot(
    store: &Store,
    ws: &str,
    flow: &Flow,
    run_id: &str,
    dep: &str,
    fctx: &str,
    subgraph: &std::collections::HashSet<String>,
) -> Result<(), String> {
    let barrier_indegree = flow.barrier_indegree(dep, subgraph);
    let Some(mut rec) = read_step(store, ws, run_id, dep, fctx).await? else {
        // First touch: seed the slot Pending with its full barrier indegree, then this release
        // decrements by one.
        let rec = FlowStepRecord {
            run_id: run_id.into(),
            node_id: dep.into(),
            claim: ClaimState::Pending,
            indegree: barrier_indegree,
            outcome: String::new(),
            output: Value::Null,
            findings: Value::Null,
            error: None,
            attempts: 0,
            ms: 0,
            patched_config: None,
            fctx: fctx.into(),
            triggered_by: None,
            parent_fctx: None,
        };
        write_step(store, ws, &rec).await?;
        let mut rec = rec;
        rec.indegree = rec.indegree.saturating_sub(1);
        if rec.indegree == 0 {
            rec.claim = ClaimState::Enqueued;
        }
        write_step(store, ws, &rec).await?;
        return Ok(());
    };
    // Idempotent guard: a re-release of the same (finished, fctx) must not double-decrement. The slot
    // records the set of upstreams already counted via its `triggered_by`-style marker — here a
    // barrier slot is touched once per distinct (upstream, fctx) by the drive, and a redelivered
    // release of the SAME (upstream, fctx) would re-mint the same key. We rely on the caller driving
    // each (node, fctx) settle exactly once (the CAS claim guarantees the settle is once). So a plain
    // decrement is correct: one release per upstream settle.
    if rec.claim == ClaimState::Done {
        return Ok(()); // already terminal (a gated skip or a live path) — leave it
    }
    rec.indegree = rec.indegree.saturating_sub(1);
    if matches!(rec.claim, ClaimState::Pending) && rec.indegree == 0 {
        rec.claim = ClaimState::Enqueued;
    }
    write_step(store, ws, &rec).await?;
    Ok(())
}

/// Mint an `any`-funnel firing slot `(node, new_fctx)` Enqueued, with the triggering upstream +
/// parent fctx recorded so the executor can auto-wire the single arriving message. Idempotent: a
/// redelivered upstream re-mints the same deterministic `new_fctx` and finds the slot already present
/// → no-op (exactly-once per firing).
async fn mint_firing(
    store: &Store,
    ws: &str,
    run_id: &str,
    node: &str,
    new_fctx: &str,
    triggered_by: &str,
    parent_fctx: &str,
) -> Result<(), String> {
    if read_step(store, ws, run_id, node, new_fctx)
        .await?
        .is_some()
    {
        return Ok(()); // already minted (a redelivery re-minted the same id) — exactly-once
    }
    let rec = FlowStepRecord {
        run_id: run_id.into(),
        node_id: node.into(),
        claim: ClaimState::Enqueued,
        indegree: 0,
        outcome: String::new(),
        output: Value::Null,
        findings: Value::Null,
        error: None,
        attempts: 0,
        ms: 0,
        patched_config: None,
        fctx: new_fctx.into(),
        triggered_by: Some(triggered_by.into()),
        parent_fctx: Some(parent_fctx.into()),
    };
    write_step(store, ws, &rec).await
}

/// Gate a `(node, fctx)` slot and its exclusive subtree as Skipped — a `switch` unmatched branch
/// (Decision 14) or a suppressed stateful node's downstream. Marks `gated` itself Skipped (if not yet
/// terminal), then cascades through its dependents under the SAME `fctx` (the `skip_subtree` walk
/// seeded at `gated`). A slot already Running/Done is left as-is (a live path reached it first — the
/// disjoint-branch assumption). For an `any` port, a gated upstream simply fires no slot (one fewer
/// firing) — this marks a barrier slot Skipped so it never hangs the run.
pub async fn skip_gated(
    store: &Store,
    ws: &str,
    flow: &Flow,
    run_id: &str,
    gated: &str,
    fctx: &str,
) -> Result<(), String> {
    match read_step(store, ws, run_id, gated, fctx).await? {
        Some(mut rec) => {
            if matches!(rec.claim, ClaimState::Pending | ClaimState::Enqueued) {
                rec.claim = ClaimState::Done;
                rec.outcome = "skipped".into();
                write_step(store, ws, &rec).await?;
            }
        }
        None => {
            // Frontier-only seeding: a gated dependent may have no slot yet. Create it Skipped so
            // finalize counts it and the run does not hang on a never-released node.
            let rec = FlowStepRecord {
                run_id: run_id.into(),
                node_id: gated.into(),
                claim: ClaimState::Done,
                indegree: 0,
                outcome: "skipped".into(),
                output: Value::Null,
                findings: Value::Null,
                error: None,
                attempts: 0,
                ms: 0,
                patched_config: None,
                fctx: fctx.into(),
                triggered_by: None,
                parent_fctx: None,
            };
            write_step(store, ws, &rec).await?;
        }
    }
    skip_subtree(store, ws, flow, run_id, gated, fctx).await
}

/// Mark the transitive subtree below a failed `(node, fctx)` slot as Skipped (Halt policy), under the
/// same `fctx`. Each direct dependent is touched under `fctx` (creating its slot Skipped if absent),
/// then the walk cascades.
pub async fn skip_subtree(
    store: &Store,
    ws: &str,
    flow: &Flow,
    run_id: &str,
    failed: &str,
    fctx: &str,
) -> Result<(), String> {
    let dependents = flow.dependents();
    let mut queue: std::collections::VecDeque<String> =
        dependents.get(failed).cloned().unwrap_or_default().into();
    let mut seen = std::collections::HashSet::new();
    while let Some(id) = queue.pop_front() {
        if !seen.insert(id.clone()) {
            continue;
        }
        let mut made_skip = false;
        if let Some(mut rec) = read_step(store, ws, run_id, &id, fctx).await? {
            if matches!(rec.claim, ClaimState::Pending | ClaimState::Enqueued) {
                rec.claim = ClaimState::Done;
                rec.outcome = "skipped".into();
                write_step(store, ws, &rec).await?;
                made_skip = true;
            }
        } else {
            // The dependent's slot does not exist yet (frontier-only seeding) — create it Skipped so
            // finalize counts it and the run does not hang on a never-released slot.
            let rec = FlowStepRecord {
                run_id: run_id.into(),
                node_id: id.clone(),
                claim: ClaimState::Done,
                indegree: 0,
                outcome: "skipped".into(),
                output: Value::Null,
                findings: Value::Null,
                error: None,
                attempts: 0,
                ms: 0,
                patched_config: None,
                fctx: fctx.into(),
                triggered_by: None,
                parent_fctx: None,
            };
            write_step(store, ws, &rec).await?;
            made_skip = true;
        }
        // Only cascade past a node we actually marked Skipped (a live/Done node keeps its subtree).
        if made_skip {
            if let Some(next) = dependents.get(&id) {
                for n in next {
                    queue.push_back(n.clone());
                }
            }
        }
    }
    Ok(())
}

/// If every slot of the run is terminal, write the run status. Returns the new status if finalised.
///
/// Run-terminal now **counts slots, not nodes** (flow-input-ports-scope): a run is done when every
/// `(node, fctx)` slot it minted is `Done` AND every node in its subgraph has ≥1 slot (so a not-yet-
/// released node keeps the run open). Scans the run's step records (keyed `{run}:…`).
pub async fn finalize_if_complete(
    store: &Store,
    ws: &str,
    flow: &Flow,
    run_id: &str,
) -> Result<Option<String>, String> {
    let set = run_node_set(store, ws, flow, run_id).await?;
    // Read every slot this run minted (scan the step table, filter by run_id).
    let slots = scan_run_slots(store, ws, run_id).await?;
    let mut any_failed = false;
    let mut any_ok = false;
    let mut touched_nodes: std::collections::HashSet<String> = std::collections::HashSet::new();
    for rec in &slots {
        touched_nodes.insert(rec.node_id.clone());
        if rec.claim != ClaimState::Done {
            return Ok(None); // an in-flight slot — not terminal
        }
        match rec.outcome.as_str() {
            "ok" => any_ok = true,
            "err" => any_failed = true,
            _ => {}
        }
    }
    // Every subgraph node must have ≥1 slot (a not-yet-released node means the run is still driving).
    for n in flow.nodes.iter().filter(|n| set.contains(&n.id)) {
        if !touched_nodes.contains(&n.id) {
            return Ok(None);
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

/// Scan every `flow_step_output` slot minted for a run (filter by `run_id` on the record body).
pub async fn scan_run_slots(
    store: &Store,
    ws: &str,
    run_id: &str,
) -> Result<Vec<FlowStepRecord>, String> {
    let page = lb_store::scan(store, ws, FLOW_STEP_TABLE, lb_store::MAX_SCAN_LIMIT, None)
        .await
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for row in page.rows {
        let inner = match row.data {
            Value::Object(mut o) => o.remove("data").unwrap_or(Value::Null),
            other => other,
        };
        if inner.get("run_id").and_then(|v| v.as_str()) != Some(run_id) {
            continue;
        }
        if let Ok(rec) = serde_json::from_value::<FlowStepRecord>(inner) {
            out.push(rec);
        }
    }
    Ok(out)
}

/// The `(node, fctx)` slots currently `Enqueued` (the ready frontier). Scans the run's step records.
pub async fn ready_slots(
    store: &Store,
    ws: &str,
    run_id: &str,
) -> Result<Vec<(String, String)>, String> {
    let mut out = Vec::new();
    for rec in scan_run_slots(store, ws, run_id).await? {
        if rec.claim == ClaimState::Enqueued {
            out.push((rec.node_id, rec.fctx));
        }
    }
    Ok(out)
}

/// A node's resolved inputs (its incoming message, D2) plus the `carry` map to merge forward (D4).
pub struct ResolvedInputs {
    /// The node's incoming `msg` — the map every builtin reads `payload`/`topic` from.
    pub inputs: serde_json::Map<String, Value>,
    /// The fields to carry forward onto the node's output envelope (inputs minus `payload`). Empty
    /// for a join (≥2 upstreams) — no ambiguous merge (D4).
    pub carry: serde_json::Map<String, Value>,
}

/// Resolve a `(node, fctx)` slot's incoming message (flow-message-envelope-scope D2/D3, widened to
/// the firing **lineage** by flow-plain-wiring-scope). Two shapes:
///
/// - **`any` firing** (`triggered_by` set — the universal per-message case): the slot fires for
///   exactly ONE upstream — the triggering one — whose envelope (settled under `parent_fctx`) is
///   auto-wired as the input, and whose non-`payload` fields carry forward (Node-RED "metadata
///   survives a join", unambiguous because each firing has exactly one incoming message).
/// - **barrier/frontier firing** (`triggered_by` None): auto-wire when there is exactly one `needs`
///   upstream and no explicit `payload` binding (copy that upstream's envelope); else build
///   `inputs` from the explicit `with` bindings.
///
/// In BOTH shapes, an explicit `${steps.X}` binding resolves against X's settle whose `fctx` is an
/// **ancestor** of this firing's ([`lb_flows::is_ancestor`] — equal, a whole-segment prefix, or
/// `""`), nearest ancestor winning. That keeps a grandparent binding resolvable down a linear chain
/// under universal `any` (the recorded map used to hold only the arriving upstream — a silent-null
/// regression this widening prevents); a genuine cross-branch settle never matches (and is a save
/// lint). Empty `fctx` everywhere ⇒ byte-for-byte the pre-ports resolution.
pub async fn resolve_node_bindings(
    store: &Store,
    ws: &str,
    flow: &Flow,
    run_id: &str,
    node_id: &str,
    fctx: &str,
    triggered_by: Option<&str>,
    parent_fctx: Option<&str>,
    with: &serde_json::Map<String, Value>,
    params: &serde_json::Map<String, Value>,
) -> Result<ResolvedInputs, String> {
    let needs = flow
        .node(node_id)
        .map(|n| n.needs.as_slice())
        .unwrap_or(&[]);
    let binds_payload = with.contains_key("payload");

    // An `any`-funnel firing auto-wires its single triggering upstream (settled under `parent_fctx`).
    if let Some(up) = triggered_by {
        let pf = parent_fctx.unwrap_or("");
        let envelope = match read_step(store, ws, run_id, up, pf).await? {
            Some(rec) if rec.claim == ClaimState::Done && rec.outcome == "ok" => rec.output,
            _ => Value::Null,
        };
        let mut inputs = match envelope {
            Value::Object(m) => m,
            other => {
                let mut m = serde_json::Map::new();
                m.insert("payload".into(), other);
                m
            }
        };
        // Apply any explicit bindings the author set on top (e.g. a hand-set `topic`, a grandparent
        // `${steps.X}` read), resolved against the firing's LINEAGE — every settle whose fctx is an
        // ancestor of this firing's, nearest winning. The triggering upstream's just-read envelope
        // overrides its recorded settle (same value; keeps the auto-wire authoritative).
        let mut recorded = lineage_recorded(store, ws, run_id, fctx).await?;
        recorded.insert(up.to_string(), NodeOutput::new(inputs_serialize(&inputs)));
        let bound = resolve_bindings(with, &recorded, params).map_err(|e| e.to_string())?;
        for (k, v) in bound {
            inputs.insert(k, v);
        }
        overlay_retained_inputs(store, ws, &flow.id, node_id, &mut inputs).await?;
        // The triggering upstream's non-`payload` fields carry forward (D4 / flow-input-ports-scope).
        let carry = without_payload(&inputs);
        return Ok(ResolvedInputs { inputs, carry });
    }

    // Barrier/frontier: build `recorded` from every done-ok settle in this firing's LINEAGE (its
    // own fctx, or any whole-segment-prefix ancestor down to ""). In the all-empty-fctx case this
    // is every settle — byte-for-byte the pre-ports resolution.
    let recorded = lineage_recorded(store, ws, run_id, fctx).await?;

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

/// Serialize a map into an object `Value` (a tiny helper for the any-firing recorded-envelope build).
fn inputs_serialize(m: &serde_json::Map<String, Value>) -> Value {
    Value::Object(m.clone())
}

/// The `${steps.X}` resolution map for a firing at `fctx`: every node's done-ok settle whose own
/// `fctx` is an **ancestor** of this firing's (equal / whole-segment prefix / `""`), keeping the
/// **nearest** (longest-prefix) settle when a node settled at several ancestor depths. This is the
/// lineage walk (flow-plain-wiring-scope): a linear chain's grandparent binding resolves under
/// universal `any`; a sibling firing's settle (a cross-branch fctx) never matches.
async fn lineage_recorded(
    store: &Store,
    ws: &str,
    run_id: &str,
    fctx: &str,
) -> Result<HashMap<String, NodeOutput>, String> {
    let mut best: HashMap<String, (usize, Value)> = HashMap::new();
    for rec in scan_run_slots(store, ws, run_id).await? {
        if rec.claim != ClaimState::Done || rec.outcome != "ok" {
            continue;
        }
        if !lb_flows::is_ancestor(&rec.fctx, fctx) {
            continue;
        }
        let depth = rec.fctx.len(); // a longer ancestor prefix = a nearer ancestor
        match best.get(&rec.node_id) {
            Some((d, _)) if *d >= depth => {}
            _ => {
                best.insert(rec.node_id.clone(), (depth, rec.output));
            }
        }
    }
    Ok(best
        .into_iter()
        .map(|(k, (_, v))| (k, NodeOutput::new(v)))
        .collect())
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
    fctx: &str,
) -> Result<Option<FlowStepRecord>, String> {
    let id = step_record_id(run_id, node_id, fctx);
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
    let id = step_record_id(&rec.run_id, &rec.node_id, &rec.fctx);
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
