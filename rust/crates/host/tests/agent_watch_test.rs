//! agent-run scope Part 3 — the live `agent.watch` feed: a watcher observes a run's `RunEvent`
//! stream (snapshot-then-deltas), gated by `mcp:agent.watch:call`, workspace-walled. Real node + bus
//! + store; the only stub is the model provider (MockProvider). Multi-thread flavor + unique ws ids
//! (a node boots a Zenoh peer — carry-forward from S3).

use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{invoke, load_extension, serve_ext, watch_run, AllowedTool, Invocation, Node};
use lb_role_ai_gateway::{AiGateway, AiResponse, MockProvider, ToolCall};
use lb_run_events::{RunEvent, RunOutcome};

const MANIFEST: &str = include_str!("../../../extensions/hello/extension.toml");

fn hello_wasm() -> Vec<u8> {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../extensions/hello/target/wasm32-wasip2/release/hello_ext.wasm");
    std::fs::read(&path).expect("hello wasm built")
}

fn principal(ws: &str, caps: &[&str]) -> Principal {
    let key = SigningKey::generate();
    let claims = Claims {
        sub: "user:ada".into(),
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

const INVOKE: &str = "mcp:agent.invoke:call";
const ECHO: &str = "mcp:hello.echo:call";
const WATCH: &str = "mcp:agent.watch:call";

fn echo_tool() -> Vec<AllowedTool> {
    vec![AllowedTool {
        name: "hello.echo".into(),
        description: "echo".into(),
        input_schema: None,
    }]
}

fn echo_then_stop() -> AiGateway<MockProvider> {
    AiGateway::new(MockProvider::new(vec![
        AiResponse::calls(
            "I'll echo it.",
            vec![ToolCall {
                id: "c1".into(),
                name: "hello.echo".into(),
                input: r#"{"msg":"hi"}"#.into(),
            }],
            10,
        ),
        AiResponse::stop("done", 5),
    ]))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_late_watcher_gets_a_transcript_snapshot_then_finish() {
    // The headline Part-3 property (review point 5): a watcher that attaches AFTER the run finished
    // still reconstructs the whole run from the durable transcript snapshot — RunStart … the tool
    // call … RunFinish — never from deltas it missed.
    let ws = "agent-watch-snap";
    let node = Arc::new(Node::boot().await.unwrap());
    load_extension(&node, MANIFEST, &hello_wasm(), &[])
        .await
        .unwrap();
    std::mem::forget(
        serve_ext(
            &node.bus,
            node.registry.clone(),
            "hello",
            &node.node_id(),
            &[ws],
        )
        .await
        .unwrap(),
    );

    let caller = principal(ws, &[INVOKE, ECHO, WATCH]);
    let gw = echo_then_stop();
    invoke(
        &node,
        &gw,
        &caller,
        &[ECHO.into()],
        ws,
        Invocation {
            job_id: "run1",
            goal: "echo hi",
            skill: None,
            doc: None,
            tools: &echo_tool(),
            ts: 1,
        },
    )
    .await
    .unwrap();

    // Attach AFTER completion → the snapshot carries the whole run.
    let watch = watch_run(&node.store, &node.bus, &caller, ws, "run1")
        .await
        .expect("watch authorized");
    let snap = watch.snapshot;
    assert!(
        matches!(snap.first(), Some(RunEvent::RunStart { .. })),
        "starts with RunStart"
    );
    assert!(
        snap.iter()
            .any(|e| matches!(e, RunEvent::ToolCallStart { name, .. } if name == "hello.echo")),
        "snapshot has the tool call"
    );
    assert!(
        snap.iter().any(|e| matches!(
            e,
            RunEvent::RunFinish {
                outcome: RunOutcome::Done,
                ..
            }
        )),
        "snapshot ends with a Done finish: {snap:?}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_live_watcher_receives_deltas_as_the_run_progresses() {
    // The live half: subscribe BEFORE the run, then drive it — the watcher receives deltas off the
    // bus. We collect until RunFinish (or a bounded number of events) so the test can't hang.
    let ws = "agent-watch-live";
    let node = Arc::new(Node::boot().await.unwrap());
    load_extension(&node, MANIFEST, &hello_wasm(), &[])
        .await
        .unwrap();
    std::mem::forget(
        serve_ext(
            &node.bus,
            node.registry.clone(),
            "hello",
            &node.node_id(),
            &[ws],
        )
        .await
        .unwrap(),
    );
    let caller = principal(ws, &[INVOKE, ECHO, WATCH]);

    // Attach first (empty snapshot — the run hasn't started), keep the live stream.
    let watch = watch_run(&node.store, &node.bus, &caller, ws, "run1")
        .await
        .expect("watch authorized");
    let live = watch.stream;

    // Drive the run concurrently.
    let node2 = node.clone();
    let caller2 = caller.clone();
    let driver = tokio::spawn(async move {
        let gw = echo_then_stop();
        invoke(
            &node2,
            &gw,
            &caller2,
            &[ECHO.into()],
            ws,
            Invocation {
                job_id: "run1",
                goal: "echo hi",
                skill: None,
                doc: None,
                tools: &echo_tool(),
                ts: 1,
            },
        )
        .await
        .unwrap();
    });

    // Collect live events until a RunFinish or a generous cap (bounded so a miss can't hang forever).
    let mut got = Vec::new();
    for _ in 0..32 {
        match tokio::time::timeout(std::time::Duration::from_secs(5), live.recv()).await {
            Ok(Some(ev)) => {
                let finish = matches!(ev, RunEvent::RunFinish { .. });
                got.push(ev);
                if finish {
                    break;
                }
            }
            _ => break,
        }
    }
    driver.await.unwrap();

    assert!(
        got.iter()
            .any(|e| matches!(e, RunEvent::ToolCallStart { .. })),
        "live watcher saw the tool call delta: {got:?}"
    );
    assert!(
        got.iter().any(|e| matches!(e, RunEvent::RunFinish { .. })),
        "live watcher saw the finish: {got:?}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn watch_is_denied_without_the_watch_cap() {
    // MANDATORY capability-deny (§2.1): no `mcp:agent.watch:call` → opaque Denied, no stream.
    let ws = "agent-watch-deny";
    let node = Arc::new(Node::boot().await.unwrap());
    let caller = principal(ws, &[INVOKE, ECHO]); // can invoke, cannot watch
    match watch_run(&node.store, &node.bus, &caller, ws, "run1").await {
        Err(lb_host::AgentError::Denied) => {}
        Ok(_) => panic!("watch without the cap must be denied"),
        Err(other) => panic!("expected Denied, got {other:?}"),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn ws_b_cannot_watch_a_ws_a_run() {
    // MANDATORY workspace-isolation (§2.2): a ws-B watcher's snapshot of a ws-A run id is empty (the
    // store read is namespaced) AND its subscription is on ws-B's subject — it can never observe
    // ws-A's run. We assert the snapshot is empty (no cross-ws transcript leak).
    let ws_a = "agent-watch-iso-a";
    let ws_b = "agent-watch-iso-b";
    let node = Arc::new(Node::boot().await.unwrap());
    load_extension(&node, MANIFEST, &hello_wasm(), &[])
        .await
        .unwrap();
    std::mem::forget(
        serve_ext(
            &node.bus,
            node.registry.clone(),
            "hello",
            &node.node_id(),
            &[ws_a],
        )
        .await
        .unwrap(),
    );

    // Run in ws-A.
    let a = principal(ws_a, &[INVOKE, ECHO, WATCH]);
    let gw = echo_then_stop();
    invoke(
        &node,
        &gw,
        &a,
        &[ECHO.into()],
        ws_a,
        Invocation {
            job_id: "run1",
            goal: "echo hi",
            skill: None,
            doc: None,
            tools: &echo_tool(),
            ts: 1,
        },
    )
    .await
    .unwrap();

    // ws-B watches the SAME run id — the namespaced store read returns no job, so an empty snapshot.
    let b = principal(ws_b, &[WATCH]);
    let watch = watch_run(&node.store, &node.bus, &b, ws_b, "run1")
        .await
        .expect("ws-B may watch its OWN namespace (just finds nothing)");
    assert!(
        watch.snapshot.is_empty(),
        "ws-B must not see ws-A's run transcript: {:?}",
        watch.snapshot
    );
}
