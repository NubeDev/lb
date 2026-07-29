//! Host-layer tests for the **per-node interval timer reconciler** (interval-source-clock scope,
//! Phase 2 / Option A). The reconciler is the one owner of live interval cadence: it converges timer
//! tasks against the durable enabled graph, and each timer drives the SAME idempotent fire path the
//! 5s sweep used to drive.
//!
//! Two families here, and both are needed:
//!
//! 1. **Lifecycle** (enable / disable / period edit / double-enable / orphan-leak / ws isolation) —
//!    the reconciliation surface the scope names as "where this will actually break". These use the
//!    reconciler directly and assert the converged set.
//! 2. **The regression this whole scope exists for** — a `period_secs: 1` node really firing about
//!    once a second. That one has to touch real time (a timer's whole job is wall-clock cadence), so
//!    per the scope's Risks it asserts **ordering and count inside a generous window**, never exact
//!    wall-clock equality.
//!
//! Real store (`mem://`), real jobs, real caps — no mocks (rule 9). The **capability-deny** boundary
//! is deliberately NOT re-tested here: a timer-fired run takes the identical
//! `fire_flipflop_node` → `flows_run_async` path as a sweep-fired one under the same system
//! principal, so the deny wall is `flows_flipflop_test::flipflop_capability_deny_no_run_no_state`
//! passing **unchanged** — the assertion being that the new firing mechanism grants nothing.

use std::sync::Arc;
use std::time::Duration;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_flows::{Flow, Node, Placement};
use lb_host::{
    call_tool, flipflop_run_id, reconcile_interval_timers, IntervalTimers, Node as HostNode,
};
use serde_json::{json, Value};

fn principal(ws: &str, caps: &[&str]) -> Principal {
    let key = SigningKey::generate();
    let claims = Claims {
        sub: "user:test".into(),
        ws: ws.into(),
        role: Role::Member,
        caps: caps.iter().map(|s| s.to_string()).collect(),
        iat: 0,
        exp: u64::MAX,
        constraint: None,
        run_id: None,
    };
    verify(&key, &mint(&key, &claims), 1).unwrap()
}

const FULL: &[&str] = &[
    "mcp:flows.save:call",
    "mcp:flows.get:call",
    "mcp:flows.run:call",
    "mcp:flows.runs.get:call",
    "mcp:rules.run:call",
    "mcp:rules.eval:call",
    "store:flow:write",
    "store:flow:read",
];

/// A flip-flop source feeding a `rhai` echo, so a firing lands as a recorded run.
fn flipflop_flow(ws: &str, id: &str, node_id: &str, period_secs: u64, enabled: bool) -> Flow {
    let trig = Node {
        id: node_id.into(),
        node_type: "flipflop".into(),
        needs: vec![],
        with: Default::default(),
        config: json!({ "period_secs": period_secs, "start": true }),
        inputs: Vec::new(),
        position: None,
    };
    let echo = Node {
        id: "echo".into(),
        node_type: "rhai".into(),
        needs: vec![node_id.to_string()],
        with: Default::default(),
        config: json!({ "source": "payload" }),
        inputs: Vec::new(),
        position: None,
    };
    Flow {
        workspace: ws.into(),
        id: id.into(),
        name: id.into(),
        version: 0,
        params: Default::default(),
        nodes: vec![trig, echo],
        failure_policy: Default::default(),
        deleted: false,
        enabled,
        start_on_boot: false,
        placement: Placement::Either,
        concurrency: Default::default(),
        cron: None,
        next_attempt_ts: 0,
        managed_by: None,
    }
}

async fn save(node: &Arc<HostNode>, p: &Principal, ws: &str, f: &Flow) {
    let body = serde_json::to_value(f).unwrap().to_string();
    call_tool(node, p, ws, "flows.save", &body).await.unwrap();
}

/// Flip a flow's `enabled` flag through the real save verb (what the Deploy/Stop toggle does).
async fn set_enabled(node: &Arc<HostNode>, p: &Principal, ws: &str, f: &Flow, enabled: bool) {
    let mut f = f.clone();
    f.enabled = enabled;
    save(node, p, ws, &f).await;
}

fn wall_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

/// Which one-second slots in `[from, to]` actually produced a durable run for this trigger node.
/// Run ids are deterministic per scheduled instant, so existence of the job IS the firing record —
/// no scanning, no ordering assumptions.
async fn fired_slots(
    node: &Arc<HostNode>,
    ws: &str,
    flow: &str,
    node_id: &str,
    from: u64,
    to: u64,
) -> Vec<u64> {
    let mut hits = Vec::new();
    for ts in from..=to {
        let id = flipflop_run_id(flow, node_id, ts);
        if lb_jobs::load(&node.store, ws, &id).await.unwrap().is_some() {
            hits.push(ts);
        }
    }
    hits
}

// ---------------------------------------------------------------------------------------------
// Lifecycle — the reconciliation surface
// ---------------------------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn enable_spawns_one_timer_and_disable_tears_it_down() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);
    let timers = Arc::new(IntervalTimers::new());
    let flow = flipflop_flow("ws", "osc", "ff", 10, true);
    save(&node, &p, "ws", &flow).await;

    let pass = reconcile_interval_timers(&timers, &node, &p, "ws")
        .await
        .unwrap();
    assert_eq!((pass.started, pass.stopped), (1, 0));
    assert_eq!(timers.count().await, 1);

    set_enabled(&node, &p, "ws", &flow, false).await;
    let pass = reconcile_interval_timers(&timers, &node, &p, "ws")
        .await
        .unwrap();
    assert_eq!((pass.started, pass.stopped), (0, 1));
    assert_eq!(timers.count().await, 0, "a disabled flow keeps no timer");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reconciling_an_unchanged_graph_is_a_no_op_no_double_spawn() {
    // The pass runs on every reactor tick, so "converged" must mean "does nothing". A second enable
    // (or just a second tick) that spawned a second timer would double the node's firing rate —
    // silently, and only in production where the tick actually repeats.
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);
    let timers = Arc::new(IntervalTimers::new());
    save(&node, &p, "ws", &flipflop_flow("ws", "osc", "ff", 10, true)).await;

    let first = reconcile_interval_timers(&timers, &node, &p, "ws")
        .await
        .unwrap();
    assert_eq!(first.started, 1);
    for _ in 0..5 {
        let again = reconcile_interval_timers(&timers, &node, &p, "ws")
            .await
            .unwrap();
        assert_eq!(
            (again.started, again.stopped),
            (0, 0),
            "a converged graph must start and stop nothing"
        );
    }
    assert_eq!(timers.count().await, 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_period_edit_replaces_the_timer_rather_than_duplicating_it() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);
    let timers = Arc::new(IntervalTimers::new());
    save(&node, &p, "ws", &flipflop_flow("ws", "osc", "ff", 10, true)).await;
    reconcile_interval_timers(&timers, &node, &p, "ws")
        .await
        .unwrap();

    // Same flow + same node, different period: the old cadence must be retired, not left running
    // beside the new one (two timers on one node = double firing at two rates).
    save(&node, &p, "ws", &flipflop_flow("ws", "osc", "ff", 3, true)).await;
    let pass = reconcile_interval_timers(&timers, &node, &p, "ws")
        .await
        .unwrap();
    assert_eq!(
        (pass.started, pass.stopped),
        (1, 1),
        "a period edit is teardown + restart, so the new cadence applies immediately"
    );
    assert_eq!(timers.count().await, 1, "still exactly one timer");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn repeated_enable_disable_leaves_no_orphan_timer_still_firing() {
    // **The orphan/leak test.** The scope calls a leaked timer worse than the bug being fixed: a flow
    // the operator disabled but that keeps firing is silent and compounding.
    //
    // Asserting `count() == 0` alone is NOT enough — a task that outlived its map entry would pass
    // that and still fire. So this asserts the BEHAVIOUR: after the final disable, a window several
    // periods long produces no new runs at all.
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);
    let timers = Arc::new(IntervalTimers::new());
    let flow = flipflop_flow("ws", "osc", "ff", 1, true);

    // Task census before any timer exists. Taken after boot so the node's own tasks are counted in.
    let baseline_tasks = tokio::runtime::Handle::current()
        .metrics()
        .num_alive_tasks();

    for _ in 0..5 {
        set_enabled(&node, &p, "ws", &flow, true).await;
        reconcile_interval_timers(&timers, &node, &p, "ws")
            .await
            .unwrap();
        assert_eq!(timers.count().await, 1, "one enable ⇒ exactly one timer");
        set_enabled(&node, &p, "ws", &flow, false).await;
        reconcile_interval_timers(&timers, &node, &p, "ws")
            .await
            .unwrap();
        assert_eq!(timers.count().await, 0, "one disable ⇒ zero timers");
    }

    // A disabled flip-flop must produce no NEW runs. Compare the SET of fired slots across a quiet
    // window rather than asserting "nothing after time T": each enable above legitimately gets one
    // firing (the oscillator emits as soon as it is armed) and slots are whole seconds, so a
    // legitimate pre-disable firing can share the boundary second with any T sampled just after the
    // disable. Set-equality has no boundary to straddle.
    let window = 4u64;
    let scan_from = wall_now().saturating_sub(10);
    let before = fired_slots(&node, "ws", "osc", "ff", scan_from, wall_now() + window + 2).await;
    tokio::time::sleep(Duration::from_secs(window)).await;
    let after = fired_slots(&node, "ws", "osc", "ff", scan_from, wall_now() + 2).await;
    assert_eq!(
        before, after,
        "a disabled flip-flop must produce no new runs; an orphan timer fired in the quiet window \
         (before={before:?} after={after:?})"
    );

    // ...but that assertion ALONE does not prove teardown, and this is the trap worth naming: with
    // the `Drop` abort deliberately removed, every check above still passes. A leaked timer wakes,
    // calls the shared fire path, and that path re-reads the flow and finds it DISABLED — so it
    // fires nothing. The `enabled` re-check is an INNER gate that makes "did anything fire?" blind to
    // whether the task still exists. (Run-id idempotency hides it a second way: even two live timers
    // on one node cannot produce two runs for one slot.)
    //
    // So assert the property ONLY teardown has — the task is actually gone. A leak's real cost is a
    // task polling the store forever, once per period, per orphan, compounding with every
    // enable/disable cycle. `num_alive_tasks` is the direct measure of exactly that.
    //
    // The count is compared with a small fixed tolerance because a *firing* also spawns a detached
    // run-drive task (`flows_run_async`), and one may still be in flight. That transient is O(1);
    // an orphaned timer is O(cycles). That difference is what gives the assertion its teeth, and it
    // is measured, not assumed — revert-check over these 5 cycles:
    //
    //   `LiveTimer::drop` gutted → baseline + 6  → FAILS (and every other assertion here still passed)
    //   `LiveTimer::drop` real   → baseline + 1  → passes
    //
    // Let transients drain first so the tolerance stays tight rather than absorbing real leaks.
    let mut alive = usize::MAX;
    for _ in 0..30 {
        tokio::time::sleep(Duration::from_millis(100)).await;
        let now = tokio::runtime::Handle::current()
            .metrics()
            .num_alive_tasks();
        if now >= alive {
            break;
        }
        alive = now;
    }
    assert!(
        alive <= baseline_tasks + 2,
        "every timer task must be gone after its disable; {alive} tasks alive vs {baseline_tasks} \
         before any enable — {} orphan(s) leaked across 5 enable/disable cycles",
        alive.saturating_sub(baseline_tasks)
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_reconcile_pass_never_touches_another_workspaces_timers() {
    // Workspace is the hard wall (rule 6): a ws-B pass may neither start a ws-A timer nor tear one
    // down. Both directions matter — the teardown half is the one a naive "remove everything not
    // desired" reconciler gets wrong, and it would silently stop a healthy tenant's flows.
    let node = Arc::new(HostNode::boot().await.unwrap());
    let pa = principal("wsa", FULL);
    let pb = principal("wsb", FULL);
    let timers = Arc::new(IntervalTimers::new());

    save(
        &node,
        &pa,
        "wsa",
        &flipflop_flow("wsa", "osc", "ff", 10, true),
    )
    .await;
    reconcile_interval_timers(&timers, &node, &pa, "wsa")
        .await
        .unwrap();
    assert_eq!(timers.count().await, 1);

    // ws-B has NO interval flows at all. Its pass must be a complete no-op.
    let pass = reconcile_interval_timers(&timers, &node, &pb, "wsb")
        .await
        .unwrap();
    assert_eq!((pass.started, pass.stopped), (0, 0));
    assert_eq!(
        timers.count().await,
        1,
        "ws-B's pass must not have retired ws-A's timer"
    );

    // ws-B gains its own flow; both coexist, each owned by its own pass.
    save(
        &node,
        &pb,
        "wsb",
        &flipflop_flow("wsb", "osc", "ff", 10, true),
    )
    .await;
    let pass = reconcile_interval_timers(&timers, &node, &pb, "wsb")
        .await
        .unwrap();
    assert_eq!((pass.started, pass.stopped), (1, 0));
    assert_eq!(timers.count().await, 2);
    let keys = timers.keys().await;
    assert!(keys.iter().any(|k| k.ws == "wsa"));
    assert!(keys.iter().any(|k| k.ws == "wsb"));
}

// ---------------------------------------------------------------------------------------------
// The regression this scope exists for
// ---------------------------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_one_second_period_really_fires_about_once_a_second() {
    // **THE regression.** Before Phase 2 this was structurally impossible: every interval source was a
    // durable row noticed by one 5s workspace sweep, so `period_secs: 1` fired every 5 seconds and its
    // cursor drifted 4s further behind on every tick. Under the sweep this test can reach at most ONE
    // firing in the window — it cannot pass without a real per-node timer.
    //
    // Timing assertions are a flakiness magnet (scope, Risks), so this asserts a GENEROUS bound —
    // "several firings on distinct consecutive second-slots inside a ~4s window" — never an exact
    // count or an exact instant.
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);
    let timers = Arc::new(IntervalTimers::new());
    save(&node, &p, "ws", &flipflop_flow("ws", "fast", "ff", 1, true)).await;

    let started = wall_now();
    reconcile_interval_timers(&timers, &node, &p, "ws")
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(4_200)).await;
    let end = wall_now();

    let slots = fired_slots(&node, "ws", "fast", "ff", started, end).await;
    assert!(
        slots.len() >= 3,
        "a 1s flip-flop must fire several times in ~4s; the 5s sweep could manage at most one. \
         fired slots: {slots:?} (window {started}..={end})"
    );
    // Distinct slots, one second apart — the cadence is a real grid, not a burst.
    for pair in slots.windows(2) {
        assert_eq!(
            pair[1] - pair[0],
            1,
            "consecutive firings must be exactly one period apart; got {slots:?}"
        );
    }
    // And no drift: the last firing is close to the window's end, not lagging behind it. This is the
    // property the old cursor lost — it fired on ever-older scheduled instants.
    let last = *slots.last().unwrap();
    assert!(
        end - last <= 2,
        "the newest firing must track the wall clock (no drift); last={last} end={end}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_oscillator_value_keeps_flipping_across_timer_firings() {
    // The value is still STATE — durable in the cursor, flipped once per firing. Only the cadence
    // moved into the timer, so a timer-driven oscillator must alternate exactly as the sweep-driven
    // one did.
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);
    let timers = Arc::new(IntervalTimers::new());
    save(&node, &p, "ws", &flipflop_flow("ws", "osc", "ff", 1, true)).await;

    reconcile_interval_timers(&timers, &node, &p, "ws")
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(3_500)).await;

    // Read each fired run's echoed payload in slot order and assert it alternates.
    let slots = fired_slots(&node, "ws", "osc", "ff", wall_now() - 5, wall_now()).await;
    assert!(slots.len() >= 2, "need at least two firings; got {slots:?}");
    let mut values = Vec::new();
    for ts in &slots {
        let run_id = flipflop_run_id("osc", "ff", *ts);
        let out = call_tool(
            &node,
            &p,
            "ws",
            "flows.runs.get",
            &json!({ "run_id": run_id }).to_string(),
        )
        .await
        .unwrap();
        let snap: Value = serde_json::from_str(&out).unwrap();
        if snap["status"] == "success" {
            if let Some(v) = snap["steps"]
                .as_array()
                .and_then(|s| s.iter().find(|s| s["id"] == "echo"))
                .map(|s| s["output"]["payload"].clone())
            {
                values.push(v);
            }
        }
    }
    assert!(values.len() >= 2, "need two settled runs; got {values:?}");
    for pair in values.windows(2) {
        assert_ne!(
            pair[0], pair[1],
            "each firing must flip the value; got {values:?}"
        );
    }
}
