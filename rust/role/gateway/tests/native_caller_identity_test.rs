//! The native call frame carries the authorized caller — GAP A of native-caller-identity scope, end
//! to end over REAL HTTP against a REAL spawned `echo-sidecar`. No mocks (CLAUDE §9 / testing §0): a
//! real `Node`, a real `axum` gateway on a real TCP port, a real OS-launched sidecar child that reads
//! `CallParams.caller` and reflects it back through its `whoami` tool.
//!
//!   - **frame carries the caller:** a routed `POST /native/call` from a known principal makes the
//!     child echo the exact `{sub, ws, role, delegated}` the host stamped — proof the projection
//!     survived host → frame → child.
//!   - **backward compatible:** the child's `whoami` handles a `caller` that is present; the OLD-frame
//!     path (no `caller`) is covered by the SDK's `old_frame_without_caller_deserializes_to_none`
//!     unit test and the shared `lb_supervisor::CallParams` `#[serde(default)]` — a child built before
//!     this change ignores the field and does not panic.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use lb_auth::{mint, Claims, Role, SigningKey};
use lb_host::{install_native, Node, OsLauncher, Role as NodeRole};
use lb_role_gateway::{router, Gateway};
use serde_json::{json, Value};

const NOW: u64 = 1000;
const MANIFEST: &str = include_str!("../../../extensions/echo-sidecar/extension.toml");

fn token(key: &SigningKey, sub: &str, ws: &str, caps: &[&str]) -> String {
    let claims = Claims {
        sub: sub.into(),
        ws: ws.into(),
        role: Role::Member,
        caps: caps.iter().map(|s| s.to_string()).collect(),
        iat: NOW - 1,
        exp: NOW + 10_000,
        constraint: None,
        run_id: None,
    };
    mint(key, &claims)
}

/// Where the built `echo-sidecar` binary lives (rebuilt for this test's `whoami` tool).
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

async fn install_echo(node: &Arc<Node>, key: &SigningKey, ws: &str) {
    let admin = lb_auth::verify(
        key,
        &token(
            key,
            "user:admin",
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
async fn frame_carries_the_authorized_caller_to_the_child() {
    let (node, key, base) = serve().await;
    let ws = "ci-caller";
    install_echo(&node, &key, ws).await;

    // A known principal calls `whoami`; the child reflects the caller the host stamped into the frame.
    let caller = token(&key, "user:ana", ws, &["mcp:native.call:call"]);
    let (status, body) = native_call(
        &base,
        &caller,
        json!({ "ext_id": "echo-sidecar", "tool": "whoami", "input": "{}" }),
    )
    .await;
    assert_eq!(status, 200, "granted whoami succeeds: {body}");

    let stamped = &body["caller"];
    assert!(
        stamped.is_object(),
        "the frame carried a caller (not null): {body}"
    );
    assert_eq!(stamped["sub"], "user:ana", "sub reached the child");
    assert_eq!(stamped["ws"], ws, "ws reached the child");
    assert_eq!(
        stamped["role"], "member",
        "role reached the child (kebab wire)"
    );
    assert_eq!(
        stamped["delegated"], false,
        "a plain session token is not a delegated caller"
    );
}
