//! `POST /native/call` — the browser bridge to a native sidecar's own tools, end to end over REAL
//! HTTP against a REAL spawned `echo-sidecar` (native-tier scope). No mocks (CLAUDE §9 / testing §0):
//! a real `Node`, a real `axum` gateway on a real TCP port, a real OS-launched sidecar child, real
//! `reqwest` calls. Proves the enabler the ROS UI needs:
//!   - **happy round-trip:** a granted `POST /native/call {ext_id, tool, input}` reaches the child and
//!     returns its output (tagged with the injected workspace — the scoped env reached the child).
//!   - **capability deny:** a session token WITHOUT `mcp:native.call:call` is refused (`403`).
//!   - **workspace isolation:** a ws-B token cannot reach the sidecar spawned for ws-A (the child is
//!     per-(ws,ext); the call resolves by the token's ws — structural, the hard wall §7).

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use lb_auth::{mint, Claims, Role, SigningKey};
use lb_host::{install_native, Node, OsLauncher, Role as NodeRole};
use lb_role_gateway::{router, Gateway};
use serde_json::{json, Value};

const NOW: u64 = 1000;
const MANIFEST: &str = include_str!("../../../extensions/echo-sidecar/extension.toml");

fn token(key: &SigningKey, ws: &str, caps: &[&str]) -> String {
    let claims = Claims {
        sub: "user:test".into(),
        ws: ws.into(),
        role: Role::Member,
        caps: caps.iter().map(|s| s.to_string()).collect(),
        iat: NOW - 1,
        exp: NOW + 10_000,
    };
    mint(key, &claims)
}

/// Where the built `echo-sidecar` binary lives (the native test fixture) — the dir the launcher execs.
fn sidecar_dir() -> String {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/debug");
    if !dir.join("echo-sidecar").exists() {
        panic!(
            "missing echo-sidecar at {} — run: (cd rust && cargo build -p echo-sidecar)",
            dir.join("echo-sidecar").display()
        );
    }
    dir.to_string_lossy().into_owned()
}

/// Boot a real node + gateway on a real TCP port; return `(node, key, base_url)`.
async fn serve() -> (Arc<Node>, SigningKey, String) {
    let node = Arc::new(Node::boot_as(NodeRole::Hub).await.expect("node boots"));
    let key = SigningKey::generate();
    let gw = Gateway::new(node.clone(), key.clone(), NOW);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr: SocketAddr = listener.local_addr().unwrap();
    let app = router(gw);
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (node, key, format!("http://{addr}"))
}

/// Install (spawn) the echo-sidecar into `ws` via the real launcher — the admin needs the native
/// lifecycle caps. Returns once the child is up and its tools are registered.
async fn install_echo(node: &Arc<Node>, key: &SigningKey, ws: &str) {
    let admin = lb_auth::verify(
        key,
        &token(
            key,
            ws,
            &["mcp:native.install:call", "mcp:native.call:call"],
        ),
        NOW,
    )
    .expect("admin token verifies");
    install_native(
        node,
        &OsLauncher,
        &admin,
        ws,
        MANIFEST,
        &sidecar_dir(),
        &[],
        NOW,
    )
    .await
    .expect("echo-sidecar installs + spawns");
}

/// POST /native/call as `bearer`; return `(status, json_body)`.
async fn native_call(base: &str, bearer: &str, body: Value) -> (u16, Value) {
    let resp = reqwest::Client::new()
        .post(format!("{base}/native/call"))
        .bearer_auth(bearer)
        .json(&body)
        .send()
        .await
        .expect("request sends");
    let status = resp.status().as_u16();
    let json = resp.json::<Value>().await.unwrap_or(Value::Null);
    (status, json)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn granted_call_reaches_the_sidecar() {
    let (node, key, base) = serve().await;
    let ws = "nc-happy";
    install_echo(&node, &key, ws).await;

    let caller = token(&key, ws, &["mcp:native.call:call"]);
    let (status, body) = native_call(
        &base,
        &caller,
        json!({ "ext_id": "echo-sidecar", "tool": "echo", "input": "hi" }),
    )
    .await;
    assert_eq!(status, 200, "granted native.call succeeds: {body}");
    assert_eq!(body["echo"], "hi", "the child echoed the input");
    assert_eq!(body["ws"], ws, "the injected workspace reached the child");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn call_without_cap_is_denied() {
    let (node, key, base) = serve().await;
    let ws = "nc-deny";
    install_echo(&node, &key, ws).await;

    // A session token WITHOUT mcp:native.call:call — the bridge refuses (403), never reaching the child.
    let caller = token(&key, ws, &["mcp:series.latest:call"]);
    let (status, _body) = native_call(
        &base,
        &caller,
        json!({ "ext_id": "echo-sidecar", "tool": "echo", "input": "nope" }),
    )
    .await;
    assert_eq!(status, 403, "native.call without the cap is denied");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn workspace_b_cannot_reach_ws_a_sidecar() {
    let (node, key, base) = serve().await;
    // The echo-sidecar is spawned only for ws-A.
    install_echo(&node, &key, "ws-a").await;

    // A ws-B token (full native.call grant) resolves the sidecar for ws-B — there is none, so the
    // call cannot reach ws-A's child (the sidecar is per-(ws,ext); the wall is the token's ws).
    let caller_b = token(&key, "ws-b", &["mcp:native.call:call"]);
    let (status, _body) = native_call(
        &base,
        &caller_b,
        json!({ "ext_id": "echo-sidecar", "tool": "echo", "input": "cross" }),
    )
    .await;
    assert_eq!(
        status, 403,
        "ws-B cannot reach ws-A's sidecar (not running for ws-B)"
    );
}
