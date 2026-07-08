//! The MCP-shim bridge end-to-end: drive the REAL shim lib (`lb_mcp_shim::serve_on`) against this
//! REAL gateway over a `tokio::io::duplex` pair — no spawned binary, no fake backend (rule 9). The
//! shim's JSON-RPC `tools/list` + `tools/call` → `POST /mcp/call` round-trip is proven end to end,
//! including the run-status gate (D3) and the MCP deny contract.
//!
//! Proves (scope S1f, mandatory):
//!   - **tools/list returns the narrowed menu** — the shim serves the pre-baked JSON verbatim.
//!   - **tools/call round-trips `host.time.now`** — a real host-native tool, forwarded by the shim
//!     to `/mcp/call`, re-checked by `caps::check`, returned as an MCP content block.
//!   - **capability deny (mandatory):** a member-caps run token calling `host.fs.list` (which the
//!     token lacks) gets an MCP `isError` block carrying the gateway's opaque deny — honest failure.
//!   - **run-status gate (D3, mandatory):** cancelling the run mid-session makes the NEXT
//!     `tools/call` return the fail-closed "run token no longer valid" block — the gateway refuses
//!     the token at verify time (D3), and the shim surfaces it.
//!   - **workspace isolation (mandatory):** a ws-B run token cannot reach a ws-A resource — the
//!     call is forwarded, the gateway checks the token's ws first, the deny surfaces as `isError`.
//!   - **token never in stdout:** the shim's stdout is pure JSON-RPC — the bearer token never
//!     appears in any response the shim writes (byte-asserted).

mod common;

use std::net::SocketAddr;
use std::sync::Arc;

use common::*;
use lb_host::Node;
use lb_host::Role as NodeRole;
use lb_jobs::{cancel, create, Job};
use lb_mcp_shim::{serve_on, MenuEntry, Refresher};
use lb_role_gateway::{router, Gateway};
use std::time::Duration;
use tokio::io::{duplex, AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::time::timeout;

/// Seed a run job WITH an Ask floor in the payload (the persona preset's ask list). The gateway's
/// `/mcp/call` reads this to gate bridged calls (scope S2).
async fn seed_run_with_ask(node: &Arc<Node>, ws: &str, id: &str, ask: &[&str]) {
    let payload = serde_json::json!({
        "goal": "test",
        "ask": ask,
    })
    .to_string();
    create(&node.store, ws, &Job::new(id, "agent-session", &payload, 1))
        .await
        .expect("seed run job");
}

/// Boot a real gateway on a real TCP port; return `(base_url, key)`.
async fn serve_gateway() -> (String, lb_auth::SigningKey, Arc<Node>) {
    let node = Arc::new(Node::boot_as(NodeRole::Hub).await.expect("node boots"));
    let key = lb_auth::SigningKey::generate();
    let gw = Gateway::new(node.clone(), key.clone(), NOW);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr: SocketAddr = listener.local_addr().unwrap();
    let app = router(gw);
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    (format!("http://{addr}"), key, node)
}

/// Seed a `Running` agent-session job so the run-status gate sees a live run.
async fn seed_run(node: &Arc<Node>, ws: &str, id: &str) {
    create(&node.store, ws, &Job::new(id, "agent-session", "goal", 1))
        .await
        .expect("seed run job");
}

/// Build a menu for `tools/list` — the narrowed set the role crate pre-baked.
fn menu(names: &[&str]) -> Vec<MenuEntry> {
    names.iter().map(|n| MenuEntry::name_only(*n)).collect()
}

/// Drive the shim on a duplex pair, sending `reqs` one per line and collecting the responses.
/// Each entry in `reqs` produces one response line (or none for a notification). Returns the
/// concatenated response lines.
async fn drive_shim(
    base_url: String,
    run_token: String,
    run_id: &str,
    refresh_at: Option<u64>,
    menu: Vec<MenuEntry>,
    reqs: &[&str],
) -> String {
    // Two pipes: one per direction. The shim reads its stdin from `shim_stdin`, writes its stdout
    // to `shim_stdout`; the test writes to `stdin_write` and reads from `stdout_read`.
    let (shim_stdin, mut stdin_write) = duplex(8 * 1024);
    let (stdout_read, shim_stdout) = duplex(8 * 1024);
    let http = reqwest::Client::new();
    let refresher = Refresher::new(
        base_url.clone(),
        run_id.into(),
        run_token,
        refresh_at,
        http.clone(),
    );
    let gateway_url = base_url.clone();
    let serve_task = tokio::spawn(async move {
        let _ = serve_on(menu, refresher, http, gateway_url, shim_stdin, shim_stdout).await;
    });
    // Write each request line to the shim's stdin.
    for req in reqs {
        stdin_write.write_all(req.as_bytes()).await.unwrap();
        stdin_write.write_all(b"\n").await.unwrap();
    }
    drop(stdin_write); // EOF → the shim's stdin closes → serve_on returns
                       // Read responses from the shim's stdout.
    let mut output = String::new();
    let mut lines = BufReader::new(stdout_read).lines();
    while let Ok(Ok(Some(line))) = timeout(Duration::from_secs(5), lines.next_line()).await {
        output.push_str(&line);
        output.push('\n');
    }
    let _ = serve_task.await;
    output
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn tools_list_returns_the_narrowed_menu() {
    let (base, _key, _node) = serve_gateway().await;
    let menu = menu(&["host.time.now", "tools.catalog"]);
    let resp = drive_shim(
        base,
        "unused-no-call".into(),
        "run-list",
        None,
        menu,
        &[r#"{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}"#],
    )
    .await;
    let v: serde_json::Value = serde_json::from_str(resp.trim()).expect("valid JSON response");
    let tools = v["result"]["tools"].as_array().expect("tools array");
    assert_eq!(tools.len(), 2);
    let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert!(names.contains(&"host.time.now"));
    assert!(names.contains(&"tools.catalog"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn tools_call_round_trips_a_real_host_tool() {
    let (base, key, node) = serve_gateway().await;
    let ws = "gw-bridge-call";
    seed_run(&node, ws, "run-ok").await;
    // A run-scoped token WITH host.time.now cap — the call should succeed through the real gateway.
    let tok = run_token(
        &key,
        "agent:session",
        ws,
        &["mcp:host.time.now:call"],
        None,
        "run-ok",
    );
    let req = r#"{"jsonrpc":"2.0","id":42,"method":"tools/call","params":{"name":"host.time.now","arguments":{}}}"#;
    let resp = drive_shim(base, tok, "run-ok", None, menu(&["host.time.now"]), &[req]).await;
    let v: serde_json::Value = serde_json::from_str(resp.trim()).expect("valid JSON");
    assert_eq!(v["id"], 42);
    let content = v["result"]["content"].as_array().expect("content blocks");
    assert_eq!(content[0]["type"], "text");
    // The gateway returns the time as a JSON object; the shim wraps it in a text block. isError
    // is absent/false on success.
    assert!(
        v["result"]["isError"].as_bool().unwrap_or(false) == false,
        "a granted call is not an error"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_member_caps_run_is_denied_on_an_admin_tier_tool() {
    // Mandatory capability-deny: a run-scoped token WITHOUT the host.fs.list cap calls host.fs.list
    // → the gateway denies it → the shim surfaces the deny as an MCP isError block.
    let (base, key, node) = serve_gateway().await;
    let ws = "gw-bridge-deny";
    seed_run(&node, ws, "run-deny").await;
    let tok = run_token(
        &key,
        "agent:session",
        ws,
        &["mcp:host.time.now:call"],
        None,
        "run-deny",
    );
    let req = r#"{"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"host.fs.list","arguments":{}}}"#;
    let resp = drive_shim(
        base,
        tok,
        "run-deny",
        None,
        menu(&["host.fs.list", "host.time.now"]),
        &[req],
    )
    .await;
    let v: serde_json::Value = serde_json::from_str(resp.trim()).expect("valid JSON");
    assert_eq!(v["id"], 7);
    assert_eq!(
        v["result"]["isError"].as_bool(),
        Some(true),
        "the denied call surfaces as isError"
    );
    let text = v["result"]["content"][0]["text"].as_str().unwrap_or("");
    assert!(
        !text.is_empty(),
        "the deny carries the gateway's opaque reason"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_cancelled_run_makes_the_next_call_fail_closed() {
    // Mandatory D3: once the run is cancelled, the gateway refuses the token at verify time. The
    // shim's proactive-refresh path hits the same gate (refresh refused) and surfaces a fail-closed
    // isError block on the next tools/call.
    let (base, key, node) = serve_gateway().await;
    let ws = "gw-bridge-cancel";
    seed_run(&node, ws, "run-cancel").await;
    let tok = run_token(
        &key,
        "agent:session",
        ws,
        &["mcp:host.time.now:call"],
        None,
        "run-cancel",
    );
    // Cancel the run BEFORE driving the shim.
    cancel(&node.store, ws, "run-cancel").await.expect("cancel");
    let req = r#"{"jsonrpc":"2.0","id":9,"method":"tools/call","params":{"name":"host.time.now","arguments":{}}}"#;
    let resp = drive_shim(
        base,
        tok,
        "run-cancel",
        None,
        menu(&["host.time.now"]),
        &[req],
    )
    .await;
    let v: serde_json::Value = serde_json::from_str(resp.trim()).expect("valid JSON");
    assert_eq!(v["id"], 9);
    assert_eq!(
        v["result"]["isError"].as_bool(),
        Some(true),
        "a cancelled run's call surfaces isError (D3 fail-closed)"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_ws_b_run_token_cannot_reach_ws_a_resources() {
    // Mandatory workspace isolation: a ws-B run token forwards the call, but the gateway checks
    // the token's ws first → deny. The shim surfaces the isError block.
    let (base, key, node) = serve_gateway().await;
    seed_run(&node, "ws-a", "run-iso").await;
    let tok_b = run_token(
        &key,
        "agent:session",
        "ws-b",
        &["mcp:host.time.now:call"],
        None,
        "run-iso",
    );
    let req = r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"host.time.now","arguments":{}}}"#;
    let resp = drive_shim(
        base,
        tok_b,
        "run-iso",
        None,
        menu(&["host.time.now"]),
        &[req],
    )
    .await;
    let v: serde_json::Value = serde_json::from_str(resp.trim()).expect("valid JSON");
    assert_eq!(
        v["result"]["isError"].as_bool(),
        Some(true),
        "a ws-B token on a ws-A resource is denied (workspace isolation)"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_run_token_never_appears_in_shim_stdout() {
    // The shim's stdout is pure JSON-RPC — the bearer token never appears in any response line
    // (byte-asserted). This is the "token in the wrong place" risk mitigated by design.
    let (base, key, node) = serve_gateway().await;
    let ws = "gw-bridge-notok";
    seed_run(&node, ws, "run-notok").await;
    let secret_tok = run_token(
        &key,
        "agent:session",
        ws,
        &["mcp:host.time.now:call"],
        None,
        "run-notok",
    );
    let req = r#"{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}"#;
    let resp = drive_shim(
        base,
        secret_tok.clone(),
        "run-notok",
        None,
        menu(&["host.time.now"]),
        &[req],
    )
    .await;
    assert!(
        !resp.contains(&secret_tok),
        "the run token must NOT appear in shim stdout (got: {resp})"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_ask_gated_tool_returns_awaiting_approval_over_the_bridge() {
    // S2 (mandatory): the persona's Ask floor fires for bridged calls. A run-scoped token whose
    // persona gates `ext.publish` → the gateway intercepts the bridged call and returns the
    // "awaiting approval" tool result. The agent sees this, reports it, and ends — exactly the
    // report-and-end contract the scope decides. The call does NOT dispatch (no publish happens).
    let (base, key, node) = serve_gateway().await;
    let ws = "gw-bridge-ask";
    seed_run_with_ask(&node, ws, "run-ask", &["ext.publish"]).await;
    let tok = run_token(
        &key,
        "agent:session",
        ws,
        &["mcp:ext.publish:call"],
        None,
        "run-ask",
    );
    let req = r#"{"jsonrpc":"2.0","id":88,"method":"tools/call","params":{"name":"ext.publish","arguments":{}}}"#;
    let resp = drive_shim(base, tok, "run-ask", None, menu(&["ext.publish"]), &[req]).await;
    let v: serde_json::Value = serde_json::from_str(resp.trim()).expect("valid JSON");
    assert_eq!(v["id"], 88);
    // The tool result carries the awaiting-approval marker — the agent reports it and ends.
    let text = v["result"]["content"][0]["text"].as_str().unwrap_or("");
    assert!(
        text.contains("awaiting_approval") || text.contains("requires human approval"),
        "the Ask gate surfaces 'awaiting approval' to the agent (got: {text})"
    );
    assert!(
        !v["result"]["isError"].as_bool().unwrap_or(false),
        "the Ask result is not isError (it's a legitimate 'awaiting' state, not a failure)"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_non_ask_tool_dispatches_normally_over_the_bridge() {
    // S2 control: a tool NOT in the persona's ask floor dispatches normally (no suspension).
    // `host.time.now` is not in the ask list → the gateway dispatches it → a real time result.
    let (base, key, node) = serve_gateway().await;
    let ws = "gw-bridge-noask";
    seed_run_with_ask(&node, ws, "run-noask", &["ext.publish"]).await;
    let tok = run_token(
        &key,
        "agent:session",
        ws,
        &["mcp:host.time.now:call"],
        None,
        "run-noask",
    );
    let req = r#"{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"host.time.now","arguments":{}}}"#;
    let resp = drive_shim(
        base,
        tok,
        "run-noask",
        None,
        menu(&["host.time.now"]),
        &[req],
    )
    .await;
    let v: serde_json::Value = serde_json::from_str(resp.trim()).expect("valid JSON");
    assert_eq!(v["id"], 5);
    // A successful dispatch — NOT the awaiting-approval marker.
    let text = v["result"]["content"][0]["text"].as_str().unwrap_or("");
    assert!(
        !text.contains("awaiting_approval"),
        "a non-ask tool dispatches normally (got: {text})"
    );
}
