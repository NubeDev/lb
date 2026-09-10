//! Integration tests for the platform node pack (ext-store-nodes scope): `ext-list` / `ext-call` /
//! `store-read` / `store-write` / `store-delete`, exercised end-to-end through the **real**
//! `flows.save`/`flows.run` path on the real store (`mem://`), real caps, real install records, and
//! the real hello wasm component for `ext-call` (CLAUDE §9 — no mocks, no fakes).
//!
//! Mandatory categories here: **capability-deny** (a principal lacking the dispatched verb's cap has
//! its node denied AT that node — run fails, no partial write) and **workspace-isolation** (`ext-list`
//! in ws-B omits ws-A's installs; `store-read` in ws-B reads none of ws-A's rows for the same table
//! name). The round-trips prove the `{data, rev}` envelope unwrap and the config-vs-payload
//! precedence rule on the shipped paths.

use std::sync::Arc;

use lb_assets::{record_install, Install};
use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_flows::{FailurePolicy, Flow, Node, Placement};
use lb_host::{call_tool, load_extension, Node as HostNode};
use serde_json::{json, Value};

const HELLO_MANIFEST: &str = include_str!("../../../extensions/hello/extension.toml");

/// Read the built hello component (built separately for wasm32-wasip2; fail loudly if missing).
fn hello_wasm() -> Vec<u8> {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../extensions/hello/target/wasm32-wasip2/release/hello_ext.wasm");
    std::fs::read(&path).unwrap_or_else(|e| {
        panic!(
            "missing hello component at {} ({e}).\nBuild it first:\n  \
             (cd rust/extensions/hello && cargo build --target wasm32-wasip2 --release)",
            path.display()
        )
    })
}

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

/// The flow-running base caps (save/run/observe) every test principal needs.
const FLOW_CAPS: &[&str] = &[
    "mcp:flows.save:call",
    "mcp:flows.run:call",
    "mcp:flows.runs.get:call",
    "store:flow:write",
    "store:flow:read",
];

/// FLOW_CAPS ∪ `extra` as owned strings (per-test verb caps ride on top of the base).
fn caps_with(extra: &[&str]) -> Vec<String> {
    FLOW_CAPS
        .iter()
        .chain(extra.iter())
        .map(|s| s.to_string())
        .collect()
}

fn principal_with(ws: &str, extra: &[&str]) -> Principal {
    let caps = caps_with(extra);
    let refs: Vec<&str> = caps.iter().map(|s| s.as_str()).collect();
    principal(ws, &refs)
}

fn one_node_flow(ws: &str, id: &str, node_type: &str, config: Value, payload: Value) -> Flow {
    let node = Node {
        id: "n".into(),
        node_type: node_type.into(),
        needs: vec![],
        with: serde_json::Map::from_iter([("payload".into(), payload)]),
        config,
        inputs: Vec::new(),
        position: None,
    };
    Flow {
        workspace: ws.into(),
        id: id.into(),
        name: id.into(),
        version: 0,
        params: Default::default(),
        nodes: vec![node],
        failure_policy: FailurePolicy::Halt,
        deleted: false,
        enabled: true,
        start_on_boot: false,
        placement: Placement::Either,
        concurrency: Default::default(),
        cron: None,
        next_attempt_ts: 0,
        managed_by: None,
    }
}

async fn save(node: &Arc<HostNode>, p: &Principal, ws: &str, flow: &Flow) {
    let body = serde_json::to_value(flow).unwrap().to_string();
    call_tool(node, p, ws, "flows.save", &body).await.unwrap();
}

/// Poll until the run reaches a terminal status; return the snapshot.
async fn await_terminal(node: &Arc<HostNode>, p: &Principal, ws: &str, run_id: &str) -> Value {
    for _ in 0..600 {
        let req = json!({ "run_id": run_id }).to_string();
        let out = call_tool(node, p, ws, "flows.runs.get", &req)
            .await
            .unwrap();
        let snap: Value = serde_json::from_str(&out).unwrap();
        let s = snap["status"].as_str().unwrap_or("");
        if matches!(
            s,
            "success" | "partialFailure" | "failed" | "cancelled" | "suspended"
        ) {
            return snap;
        }
        tokio::time::sleep(std::time::Duration::from_millis(3)).await;
    }
    panic!("run {run_id} did not settle");
}

/// Save + run a one-node flow on a SHARED host node; return `(run status, node outcome, output)`.
// Argument count is the explicit dependency list; bundling it into a struct would be a refactor.
#[allow(clippy::too_many_arguments)]
async fn run_one_on(
    node: &Arc<HostNode>,
    p: &Principal,
    ws: &str,
    flow_id: &str,
    run_id: &str,
    node_type: &str,
    config: Value,
    payload: Value,
) -> (String, String, Value) {
    let f = one_node_flow(ws, flow_id, node_type, config, payload);
    save(node, p, ws, &f).await;
    let req = json!({ "id": flow_id, "run_id": run_id, "ts": 1 }).to_string();
    call_tool(node, p, ws, "flows.run", &req).await.unwrap();
    let snap = await_terminal(node, p, ws, run_id).await;
    let step = &snap["steps"][0];
    if step["outcome"] == "err" {
        eprintln!("[{flow_id}] err step: {step}");
    }
    (
        snap["status"].as_str().unwrap().to_string(),
        step["outcome"].as_str().unwrap().to_string(),
        step["output"].clone(),
    )
}

/// The caps a store-CRUD test principal holds: the flow base + the three store verbs + the
/// per-table write gate for the user table under test.
const STORE_EXTRA: &[&str] = &[
    "mcp:store.query:call",
    "mcp:store.write:call",
    "mcp:store.delete:call",
    "store:ops_heartbeat:write",
];

// ---------------- store round-trips ----------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn store_write_then_read_round_trips_a_row() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal_with("ws", STORE_EXTRA);

    // Write a pinned row on a NON-reserved user table.
    let (status, outcome, output) = run_one_on(
        &node,
        &p,
        "ws",
        "f_w",
        "f_w-r",
        "store-write",
        json!({ "table": "ops_heartbeat", "id": "hb1", "value": {"status": "up", "n": 1} }),
        Value::Null,
    )
    .await;
    assert_eq!((status.as_str(), outcome.as_str()), ("success", "ok"));
    assert_eq!(
        output["payload"],
        json!({"table": "ops_heartbeat", "id": "hb1"}),
        "store-write emits {{table, id}}"
    );

    // Single-id read: the `{data, rev}` envelope is unwrapped — the row comes back as written.
    let (_, outcome, output) = run_one_on(
        &node,
        &p,
        "ws",
        "f_r",
        "f_r-r",
        "store-read",
        json!({ "table": "ops_heartbeat", "id": "hb1" }),
        Value::Null,
    )
    .await;
    assert_eq!(outcome, "ok");
    assert_eq!(output["payload"]["row"], json!({"status": "up", "n": 1}));

    // Filter read: `{rows: [...]}` with the value unwrapped; a non-matching filter reads empty.
    let (_, outcome, output) = run_one_on(
        &node,
        &p,
        "ws",
        "f_rf",
        "f_rf-r",
        "store-read",
        json!({ "table": "ops_heartbeat", "filter": {"status": "up"}, "limit": 10 }),
        Value::Null,
    )
    .await;
    assert_eq!(outcome, "ok");
    assert_eq!(output["payload"]["rows"], json!([{"status": "up", "n": 1}]));
    let (_, outcome, output) = run_one_on(
        &node,
        &p,
        "ws",
        "f_rn",
        "f_rn-r",
        "store-read",
        json!({ "table": "ops_heartbeat", "filter": {"status": "down"} }),
        Value::Null,
    )
    .await;
    assert_eq!(outcome, "ok");
    assert_eq!(output["payload"]["rows"], json!([]));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn store_delete_then_read_finds_nothing_and_delete_is_idempotent() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal_with("ws", STORE_EXTRA);

    let (_, outcome, _) = run_one_on(
        &node,
        &p,
        "ws",
        "f_w2",
        "f_w2-r",
        "store-write",
        json!({ "table": "ops_heartbeat", "id": "gone", "value": {"x": 1} }),
        Value::Null,
    )
    .await;
    assert_eq!(outcome, "ok");

    // Delete (a sink — the run succeeds, nothing wired downstream).
    let (status, outcome, _) = run_one_on(
        &node,
        &p,
        "ws",
        "f_d",
        "f_d-r1",
        "store-delete",
        json!({ "table": "ops_heartbeat", "id": "gone" }),
        Value::Null,
    )
    .await;
    assert_eq!((status.as_str(), outcome.as_str()), ("success", "ok"));

    // The row is gone.
    let (_, outcome, output) = run_one_on(
        &node,
        &p,
        "ws",
        "f_r2",
        "f_r2-r",
        "store-read",
        json!({ "table": "ops_heartbeat", "id": "gone" }),
        Value::Null,
    )
    .await;
    assert_eq!(outcome, "ok");
    assert_eq!(
        output["payload"]["row"],
        Value::Null,
        "deleted row reads null"
    );

    // Deleting an absent row is a success (the verb is idempotent).
    let req = json!({ "id": "f_d", "run_id": "f_d-r2", "ts": 2 }).to_string();
    call_tool(&node, &p, "ws", "flows.run", &req).await.unwrap();
    let snap = await_terminal(&node, &p, "ws", "f_d-r2").await;
    assert_eq!(snap["status"], "success", "store-delete is idempotent");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn payload_drives_id_and_value_while_the_table_stays_pinned() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal_with("ws", STORE_EXTRA);

    // Config pins only the table; the wire payload carries the id AND is the value.
    let (_, outcome, output) = run_one_on(
        &node,
        &p,
        "ws",
        "f_wp",
        "f_wp-r",
        "store-write",
        json!({ "table": "ops_heartbeat" }),
        json!({ "id": "wired", "status": "wired-up" }),
    )
    .await;
    assert_eq!(outcome, "ok");
    assert_eq!(output["payload"]["id"], "wired", "payload.id drove the key");

    let (_, outcome, output) = run_one_on(
        &node,
        &p,
        "ws",
        "f_rp",
        "f_rp-r",
        "store-read",
        json!({ "table": "ops_heartbeat", "id": "wired" }),
        Value::Null,
    )
    .await;
    assert_eq!(outcome, "ok");
    assert_eq!(
        output["payload"]["row"],
        json!({"id": "wired", "status": "wired-up"}),
        "the whole payload was the value (config.value omitted)"
    );

    // An explicit config.id WINS over the payload's (config-first precedence).
    let (_, outcome, output) = run_one_on(
        &node,
        &p,
        "ws",
        "f_wc",
        "f_wc-r",
        "store-write",
        json!({ "table": "ops_heartbeat", "id": "cfg" }),
        json!({ "id": "wired2", "status": "shadowed" }),
    )
    .await;
    assert_eq!(outcome, "ok");
    assert_eq!(
        output["payload"]["id"], "cfg",
        "config.id wins over payload.id"
    );

    // A payload-id store-delete (config.id omitted) — the wire drives the delete too.
    let (_, outcome, _) = run_one_on(
        &node,
        &p,
        "ws",
        "f_dp",
        "f_dp-r",
        "store-delete",
        json!({ "table": "ops_heartbeat" }),
        json!({ "id": "wired" }),
    )
    .await;
    assert_eq!(outcome, "ok");
    let (_, _, output) = run_one_on(
        &node,
        &p,
        "ws",
        "f_rp2",
        "f_rp2-r",
        "store-read",
        json!({ "table": "ops_heartbeat", "id": "wired" }),
        Value::Null,
    )
    .await;
    assert_eq!(output["payload"]["row"], Value::Null);
}

// ---------------- ext nodes ----------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn ext_list_returns_seeded_installs_and_filters_running_only() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    // Two real install records: one enabled (wasm ⇒ running), one disabled (⇒ not running).
    let alpha = Install::new("alpha", "0.1.0", vec![], 1);
    let mut beta = Install::new("beta", "0.2.0", vec![], 1);
    beta.enabled = false;
    record_install(&node.store, "ws", &alpha).await.unwrap();
    record_install(&node.store, "ws", &beta).await.unwrap();

    let p = principal_with("ws", &["mcp:ext.list:call"]);
    let (_, outcome, output) = run_one_on(
        &node,
        &p,
        "ws",
        "f_el",
        "f_el-r",
        "ext-list",
        json!({}),
        Value::Null,
    )
    .await;
    assert_eq!(outcome, "ok");
    let rows = output["payload"].as_array().unwrap();
    let ids: Vec<&str> = rows.iter().map(|r| r["ext"].as_str().unwrap()).collect();
    assert_eq!(ids, vec!["alpha", "beta"], "both installs, sorted by id");

    // running_only filters host-side on the joined `running` flag.
    let (_, outcome, output) = run_one_on(
        &node,
        &p,
        "ws",
        "f_elr",
        "f_elr-r",
        "ext-list",
        json!({ "running_only": true }),
        Value::Null,
    )
    .await;
    assert_eq!(outcome, "ok");
    let rows = output["payload"].as_array().unwrap();
    let ids: Vec<&str> = rows.iter().map(|r| r["ext"].as_str().unwrap()).collect();
    assert_eq!(ids, vec!["alpha"], "the disabled install is filtered out");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn ext_call_dispatches_the_picked_tool_end_to_end() {
    // The REAL wasm component (the spine_test seeding pattern): load hello, then reach its `echo`
    // through the ext-call node — the exact `<ext>.<tool>` path a picker-authored node takes.
    let node = Arc::new(HostNode::boot().await.unwrap());
    load_extension(&node, HELLO_MANIFEST, &hello_wasm(), &[])
        .await
        .expect("hello loads");

    let p = principal_with("ws", &["mcp:hello.echo:call"]);
    let (_, outcome, output) = run_one_on(
        &node,
        &p,
        "ws",
        "f_ec",
        "f_ec-r",
        "ext-call",
        json!({ "ext": "hello", "tool": "echo", "args": {"msg": "hi"} }),
        Value::Null,
    )
    .await;
    assert_eq!(outcome, "ok");
    assert_eq!(output["payload"]["echo"], "hi");

    // The tool-node args rule: an object payload merges over config.args (the wire wins).
    let (_, outcome, output) = run_one_on(
        &node,
        &p,
        "ws",
        "f_ec2",
        "f_ec2-r",
        "ext-call",
        json!({ "ext": "hello", "tool": "echo", "args": {"msg": "config"} }),
        json!({ "msg": "wire" }),
    )
    .await;
    assert_eq!(outcome, "ok");
    assert_eq!(
        output["payload"]["echo"], "wire",
        "payload merges over config.args"
    );

    // Missing config.tool fails the node before any dispatch.
    let (status, outcome, _) = run_one_on(
        &node,
        &p,
        "ws",
        "f_ec3",
        "f_ec3-r",
        "ext-call",
        json!({ "ext": "hello", "tool": "" }),
        Value::Null,
    )
    .await;
    assert_eq!((status.as_str(), outcome.as_str()), ("failed", "err"));
}

// ---------------- capability-deny (mandatory) ----------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn store_write_without_the_verb_cap_is_denied_at_the_node_no_partial_write() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    // Holds the per-table store gate but NOT `mcp:store.write:call` — the node's dispatch re-enters
    // the one chokepoint and is denied there (no widening through `flows.run`).
    let p = principal_with("ws", &["store:ops_heartbeat:write"]);
    let (status, outcome, _) = run_one_on(
        &node,
        &p,
        "ws",
        "f_dw",
        "f_dw-r",
        "store-write",
        json!({ "table": "ops_heartbeat", "id": "nope", "value": {"x": 1} }),
        Value::Null,
    )
    .await;
    assert_eq!((status.as_str(), outcome.as_str()), ("failed", "err"));
    // No partial write: the row never landed.
    let row = lb_store::read(&node.store, "ws", "ops_heartbeat", "nope")
        .await
        .unwrap();
    assert!(row.is_none(), "denied store-write must not land a row");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn store_read_without_store_query_cap_is_denied_at_the_node() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal_with("ws", &[]); // flow caps only — no mcp:store.query:call
    let (status, outcome, _) = run_one_on(
        &node,
        &p,
        "ws",
        "f_dr",
        "f_dr-r",
        "store-read",
        json!({ "table": "ops_heartbeat" }),
        Value::Null,
    )
    .await;
    assert_eq!((status.as_str(), outcome.as_str()), ("failed", "err"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn ext_list_without_the_cap_is_denied_at_the_node() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    record_install(
        &node.store,
        "ws",
        &Install::new("alpha", "0.1.0", vec![], 1),
    )
    .await
    .unwrap();
    let p = principal_with("ws", &[]); // no mcp:ext.list:call
    let (status, outcome, _) = run_one_on(
        &node,
        &p,
        "ws",
        "f_del",
        "f_del-r",
        "ext-list",
        json!({}),
        Value::Null,
    )
    .await;
    assert_eq!((status.as_str(), outcome.as_str()), ("failed", "err"));
}

// ---------------- workspace-isolation (mandatory) ----------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn ext_list_in_ws_b_omits_ws_a_installs() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    record_install(
        &node.store,
        "ws-a",
        &Install::new("alpha", "0.1.0", vec![], 1),
    )
    .await
    .unwrap();

    let pb = principal_with("ws-b", &["mcp:ext.list:call"]);
    let (_, outcome, output) = run_one_on(
        &node,
        &pb,
        "ws-b",
        "f_iso_el",
        "f_iso_el-r",
        "ext-list",
        json!({}),
        Value::Null,
    )
    .await;
    assert_eq!(outcome, "ok");
    assert_eq!(
        output["payload"],
        json!([]),
        "ws-B's ext-list must not see ws-A's install"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn store_read_in_ws_b_reads_none_of_ws_a_rows_for_the_same_table() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let pa = principal_with("ws-a", STORE_EXTRA);
    let (_, outcome, _) = run_one_on(
        &node,
        &pa,
        "ws-a",
        "f_iso_w",
        "f_iso_w-r",
        "store-write",
        json!({ "table": "ops_heartbeat", "id": "hb-a", "value": {"ws": "a"} }),
        Value::Null,
    )
    .await;
    assert_eq!(outcome, "ok");

    // Same table name from ws-B: the workspace namespace wall means zero rows — not a filtered view.
    let pb = principal_with("ws-b", STORE_EXTRA);
    let (_, outcome, output) = run_one_on(
        &node,
        &pb,
        "ws-b",
        "f_iso_r",
        "f_iso_r-r",
        "store-read",
        json!({ "table": "ops_heartbeat" }),
        Value::Null,
    )
    .await;
    assert_eq!(outcome, "ok");
    assert_eq!(output["payload"]["rows"], json!([]));

    // And ws-A still reads its own row (the wall, not an empty table).
    let (_, outcome, output) = run_one_on(
        &node,
        &pa,
        "ws-a",
        "f_iso_ra",
        "f_iso_ra-r",
        "store-read",
        json!({ "table": "ops_heartbeat", "id": "hb-a" }),
        Value::Null,
    )
    .await;
    assert_eq!(outcome, "ok");
    assert_eq!(output["payload"]["row"], json!({"ws": "a"}));
}

/// A `store-read` node must NOT be able to read the secret plane.
///
/// The node builds its own SELECT, so it does not need the untrusted-text gate — but the table name
/// is **author-supplied config**, and `store.query` was also what kept the secret-plane wall in
/// front of it. A flow author with `mcp:store.query:call` (an ordinary workspace capability) must
/// not be able to point a read node at `secret` and receive credentials.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_store_read_node_cannot_read_the_secret_plane() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal_with("ws", STORE_EXTRA);

    // Seed a credential the way the secret plane really holds one.
    node.store
        .query_ws(
            "ws",
            "CREATE secret:token SET data = { value: 'super-secret' };",
            vec![],
        )
        .await
        .expect("seed")
        .check()
        .expect("seeded");

    for table in ["secret", "credential", "identity_credential", "apikey"] {
        let (_, outcome, output) = run_one_on(
            &node,
            &p,
            "ws",
            &format!("f_sec_{table}"),
            &format!("f_sec_{table}-r"),
            "store-read",
            json!({ "table": table }),
            Value::Null,
        )
        .await;
        assert_eq!(
            outcome, "err",
            "reading `{table}` through a store-read node must fail, got {output}"
        );
        assert!(
            !output.to_string().contains("super-secret"),
            "the credential must never reach a flow payload: {output}"
        );
    }
}
