//! S7 native-tier slice — MANDATORY workspace-isolation category (testing §2, store + MCP): ws-B can
//! never see or control ws-A's sidecar. The wall holds at two layers, both structural:
//!   - STORE: the `native_status` record lives in ws-A's namespace → a ws-A `status` read returns it,
//!     a ws-B principal's `status` read returns None (different namespace, §7).
//!   - RUNTIME MAP: the `SidecarMap` is keyed `(ws, ext_id)` → a ws-B `stop`/`restart` resolves
//!     nothing AND is gate-denied (workspace-first) — it can never reach the ws-A child.
//!
//! In-memory fake launcher (no OS process — the point is the isolation, not the supervision).

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{install_native, status_native, stop_native, NativeServiceError, Node};
use lb_supervisor::{
    read_frame, write_frame, Channel, Kill, Launcher, Method, Reply, Request, SupervisorError,
};
use tokio::io::duplex;

const MANIFEST: &str = include_str!("../../../extensions/echo-sidecar/extension.toml");

struct FakeLauncher;
struct NoKill;
impl Kill for NoKill {
    fn kill(self: Box<Self>) -> Pin<Box<dyn Future<Output = ()> + Send>> {
        Box::pin(async {})
    }
}
impl Launcher for FakeLauncher {
    async fn launch(
        &self,
        _exec: &str,
        _args: &[String],
        _env: &HashMap<String, String>,
    ) -> Result<Channel, SupervisorError> {
        let (host_side, child_side) = duplex(8192);
        let (mut cr, mut cw) = tokio::io::split(child_side);
        tokio::spawn(async move {
            while let Ok(body) = read_frame(&mut cr).await {
                let req: Request = serde_json::from_slice(&body).unwrap();
                let reply = match req.method {
                    Method::Init => Reply::ok(req.id, "ready"),
                    Method::Health => Reply::ok(req.id, "ok"),
                    Method::Call => Reply::ok(req.id, "{}"),
                    Method::Shutdown => Reply::ok(req.id, "bye"),
                };
                if write_frame(&mut cw, &serde_json::to_vec(&reply).unwrap())
                    .await
                    .is_err()
                {
                    break;
                }
                if req.method == Method::Shutdown {
                    break;
                }
            }
        });
        let (read, write) = tokio::io::split(host_side);
        Ok(Channel {
            write: Box::pin(write),
            read: Box::pin(read),
            kill: Box::new(NoKill),
        })
    }
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
    };
    verify(&key, &mint(&key, &claims), 1).expect("token verifies")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn ws_b_cannot_see_or_control_ws_a_sidecar() {
    let node = Node::boot().await.unwrap();
    let launcher = FakeLauncher;

    // ws-A installs a sidecar.
    let a_admin = principal(
        "ws-a",
        &["mcp:native.install:call", "mcp:native.status:call"],
    );
    install_native(&node, &launcher, &a_admin, "ws-a", MANIFEST, "", &[], 1)
        .await
        .expect("ws-a installs");
    assert!(node.sidecars.is_running("ws-a", "echo-sidecar"));

    // ws-A sees its own status.
    assert!(status_native(&node, &a_admin, "ws-a", "echo-sidecar")
        .await
        .unwrap()
        .is_some());

    // ws-B, even with every native grant IN ITS OWN WORKSPACE, sees nothing of ws-A's sidecar.
    let b_admin = principal(
        "ws-b",
        &[
            "mcp:native.install:call",
            "mcp:native.status:call",
            "mcp:native.stop:call",
        ],
    );

    // STORE isolation: ws-B's status read for the same ext_id returns None (different namespace).
    assert!(
        status_native(&node, &b_admin, "ws-b", "echo-sidecar")
            .await
            .unwrap()
            .is_none(),
        "ws-B must not see ws-A's native_status record"
    );

    // MCP/workspace-first deny: a ws-B principal targeting ws-A is refused at gate 1.
    let denied = stop_native(&node, &b_admin, "ws-a", "echo-sidecar", 2).await;
    assert!(
        matches!(denied, Err(NativeServiceError::Denied)),
        "a ws-B principal must be workspace-denied for a ws-A stop"
    );

    // RUNTIME-MAP isolation: ws-B's map namespace has no such sidecar (NotRunning), even if the gate
    // somehow passed — there is no ws-B child to stop.
    let b_self = principal("ws-b", &["mcp:native.stop:call"]);
    assert!(
        matches!(
            stop_native(&node, &b_self, "ws-b", "echo-sidecar", 3).await,
            Err(NativeServiceError::NotRunning)
        ),
        "ws-B has no sidecar of its own to stop"
    );

    // ws-A's sidecar is untouched by all of ws-B's attempts.
    assert!(
        node.sidecars.is_running("ws-a", "echo-sidecar"),
        "ws-A's sidecar must be unaffected by ws-B"
    );
}
