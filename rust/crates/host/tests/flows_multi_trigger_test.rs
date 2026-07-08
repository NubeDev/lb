//! N independent triggers per flow + per-trigger subgraph runs + the stateful counter
//! (flow-multi-trigger-reactive-scope Testing plan). Real store (`mem://`) + real jobs + real caps,
//! injected logical clock — no mocks (CLAUDE §9). Mandatory categories: multi-cron independence,
//! per-wire subgraph isolation, the counter running total, capability-deny, workspace-isolation.

use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_flows::{Flow, Node, Placement};
use lb_host::{call_tool, react_to_flows_cron, Node as HostNode};
use lb_store::read as store_read;
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

fn trigger(id: &str, mode: &str, cron: &str) -> Node {
    let config = if cron.is_empty() {
        json!({ "mode": mode })
    } else {
        json!({ "mode": mode, "cron": cron })
    };
    Node {
        id: id.into(),
        node_type: "trigger".into(),
        needs: vec![],
        with: Default::default(),
        config,
    }
}

fn rhai(id: &str, needs: &str) -> Node {
    Node {
        id: id.into(),
        node_type: "rhai".into(),
        needs: vec![needs.into()],
        with: Default::default(),
        config: json!({ "source": "1" }),
    }
}

fn flow_with(id: &str, nodes: Vec<Node>) -> Flow {
    Flow {
        workspace: "ws".into(),
        id: id.into(),
        name: id.into(),
        version: 0,
        params: Default::default(),
        nodes,
        failure_policy: Default::default(),
        deleted: false,
        enabled: true,
        start_on_boot: false,
        placement: Placement::Either,
        concurrency: Default::default(),
        cron: None,
        next_attempt_ts: 0,
    }
}

async fn save(
    node: &Arc<HostNode>,
    p: &Principal,
    ws: &str,
    f: &Flow,
) -> Result<(), lb_mcp::ToolError> {
    let body = serde_json::to_value(f).unwrap().to_string();
    call_tool(node, p, ws, "flows.save", &body)
        .await
        .map(|_| ())
}

async fn cursor_next(node: &Arc<HostNode>, ws: &str, flow: &str, node_id: &str) -> u64 {
    store_read(
        &node.store,
        ws,
        "flow_trigger_state",
        &format!("{flow}:{node_id}"),
    )
    .await
    .unwrap()
    .and_then(|v| v["next_attempt_ts"].as_u64())
    .unwrap_or(0)
}

async fn run_snapshot(node: &Arc<HostNode>, p: &Principal, ws: &str, run_id: &str) -> Value {
    for _ in 0..600 {
        let out = call_tool(
            node,
            p,
            ws,
            "flows.runs.get",
            &json!({ "run_id": run_id }).to_string(),
        )
        .await
        .unwrap();
        let snap: Value = serde_json::from_str(&out).unwrap();
        if matches!(
            snap["status"].as_str().unwrap_or(""),
            "success" | "partialFailure" | "failed" | "cancelled"
        ) {
            return snap;
        }
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
    panic!("run {run_id} never settled");
}

fn step_ids(snap: &Value) -> Vec<String> {
    snap["steps"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|s| s["id"].as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

/// MANDATORY — two cron triggers with DIFFERENT schedules in ONE flow both save (no "one schedule"
/// rejection) and fire on their OWN independent cursors.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn multi_cron_triggers_fire_independently() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);
    // tA every minute → x ; tB daily at midnight → y. Two independent clocks in one flow (chosen so
    // tA is due long before tB, making the independence directly observable).
    let f = flow_with(
        "multi",
        vec![
            trigger("tA", "cron", "*/1 * * * *"),
            rhai("x", "tA"),
            trigger("tB", "cron", "0 0 * * *"),
            rhai("y", "tB"),
        ],
    );
    save(&node, &p, "ws", &f)
        .await
        .expect("multi-cron flow saves (no single-schedule rejection)");

    // Prime both cursors; they are distinct records with their own next instants.
    react_to_flows_cron(&node, &p, "ws", 100).await.unwrap();
    let a0 = cursor_next(&node, "ws", "multi", "tA").await;
    let b0 = cursor_next(&node, "ws", "multi", "tB").await;
    assert!(
        a0 > 0 && b0 > 0,
        "both trigger cursors primed independently"
    );
    assert!(
        b0 > a0,
        "the daily trigger's next instant is far past the per-minute one"
    );

    // Fire at tA's instant (still well before tB's midnight): ONLY tA fires; tB's cursor is untouched.
    let pass = react_to_flows_cron(&node, &p, "ws", a0).await.unwrap();
    assert_eq!(pass.fired, 1, "exactly the due trigger fired");
    let a1 = cursor_next(&node, "ws", "multi", "tA").await;
    let b1 = cursor_next(&node, "ws", "multi", "tB").await;
    assert!(a1 > a0, "tA advanced");
    assert_eq!(b1, b0, "tB untouched while tA fired — independent cursors");
}

/// MANDATORY — firing one trigger runs ONLY its downstream subgraph (per-wire), never the other
/// trigger's branch.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn per_trigger_run_executes_only_its_subgraph() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);
    let f = flow_with(
        "split",
        vec![
            trigger("A", "manual", ""),
            rhai("x", "A"),
            trigger("B", "manual", ""),
            rhai("y", "B"),
        ],
    );
    save(&node, &p, "ws", &f).await.unwrap();

    // Fire FROM A (entry=A): only {A, x} run; B and y are not part of this run (no step record).
    let out = call_tool(
        &node,
        &p,
        "ws",
        "flows.run",
        &json!({ "id": "split", "entry": "A", "ts": 1 }).to_string(),
    )
    .await
    .unwrap();
    let run_id = serde_json::from_str::<Value>(&out).unwrap()["run_id"]
        .as_str()
        .unwrap()
        .to_string();
    let snap = run_snapshot(&node, &p, "ws", &run_id).await;
    assert_eq!(snap["status"], "success");
    let mut ids = step_ids(&snap);
    ids.sort();
    assert_eq!(
        ids,
        vec!["A".to_string(), "x".to_string()],
        "only the triggered subgraph ran"
    );
    assert_eq!(
        snap["entryNode"].as_str(),
        Some("A"),
        "run records its entry trigger"
    );
}

/// MANDATORY — the stateful `counter` node goes UP one per firing and survives across runs (the
/// original "count should work like a counter" ask), driven by a real cron trigger.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn counter_node_increments_across_firings() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal("ws", FULL);
    let f = flow_with(
        "tick",
        vec![
            trigger("t", "cron", "*/1 * * * *"),
            Node {
                id: "c".into(),
                node_type: "counter".into(),
                needs: vec!["t".into()],
                with: Default::default(),
                config: json!({ "step": 1 }),
            },
        ],
    );
    save(&node, &p, "ws", &f).await.unwrap();

    react_to_flows_cron(&node, &p, "ws", 100).await.unwrap(); // prime
                                                              // Three due firings → the counter's running total reaches 3.
    for _ in 0..3 {
        let due = cursor_next(&node, "ws", "tick", "t").await;
        react_to_flows_cron(&node, &p, "ws", due).await.unwrap();
    }
    let mem = store_read(&node.store, "ws", "flow_node_memory", "tick:c")
        .await
        .unwrap()
        .expect("counter memory exists");
    assert_eq!(
        mem["count"], 3,
        "counter incremented once per firing and held its total"
    );
}

/// Capability-deny: `flows.run` without the cap is denied (the host chokepoint re-checks).
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn run_denied_without_capability() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let saver = principal("ws", FULL);
    let f = flow_with("d", vec![trigger("A", "manual", ""), rhai("x", "A")]);
    save(&node, &saver, "ws", &f).await.unwrap();
    let caps: Vec<&str> = FULL
        .iter()
        .filter(|c| **c != "mcp:flows.run:call")
        .cloned()
        .collect();
    let weak = principal("ws", &caps);
    let err = call_tool(
        &node,
        &weak,
        "ws",
        "flows.run",
        &json!({ "id": "d", "ts": 1 }).to_string(),
    )
    .await
    .unwrap_err();
    assert!(matches!(err, lb_mcp::ToolError::Denied));
}

/// Workspace-isolation: a ws-B reactor never fires a ws-A flow's triggers.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn triggers_workspace_isolated() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let pa = principal("ws-a", FULL);
    let f = flow_with(
        "iso",
        vec![trigger("t", "cron", "*/1 * * * *"), rhai("x", "t")],
    );
    save(&node, &pa, "ws-a", &f).await.unwrap();
    // ws-A primes + can fire; ws-B sees nothing.
    react_to_flows_cron(&node, &pa, "ws-a", 100).await.unwrap();
    assert!(cursor_next(&node, "ws-a", "iso", "t").await > 0);
    let pb = principal("ws-b", FULL);
    let pass = react_to_flows_cron(&node, &pb, "ws-b", 10_000)
        .await
        .unwrap();
    assert_eq!(pass.fired, 0, "a ws-B reactor never fires a ws-A trigger");
}
