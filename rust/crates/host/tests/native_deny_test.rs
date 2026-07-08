//! S7 native-tier slice — MANDATORY capability-deny category (testing §2): no `native.*` action
//! without its grant. A principal lacking `mcp:native.install:call` cannot spawn a child; lacking
//! `mcp:native.stop:call` cannot stop one. The refusal is at the gate, BEFORE any process is spawned
//! or any record is written — the spawn authority is the MCP gate, not a side effect.
//!
//! Uses an in-memory FAKE launcher (no OS process needed for the deny path — the point is that the
//! launcher is never reached). Mock only the external; here the external is never even invoked.

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{install_native, reset_native, status_native, stop_native, NativeServiceError, Node};
use lb_supervisor::{
    read_frame, write_frame, Channel, Kill, Launcher, Method, Reply, Request, SupervisorError,
};
use tokio::io::duplex;

const MANIFEST: &str = include_str!("../../../extensions/echo-sidecar/extension.toml");

/// A fake launcher that never spawns a real process — and counts launches so the deny path can
/// assert it was NEVER reached.
struct CountingLauncher(Arc<AtomicU32>);
struct NoKill;
impl Kill for NoKill {
    fn kill(self: Box<Self>) -> Pin<Box<dyn Future<Output = ()> + Send>> {
        Box::pin(async {})
    }
}
impl Launcher for CountingLauncher {
    async fn launch(
        &self,
        _exec: &str,
        _args: &[String],
        _env: &HashMap<String, String>,
    ) -> Result<Channel, SupervisorError> {
        self.0.fetch_add(1, Ordering::SeqCst);
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
        constraint: None,
        run_id: None,
    };
    verify(&key, &mint(&key, &claims), 1).expect("token verifies")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn denies_install_without_grant() {
    let ws = "native-deny";
    let node = Node::boot().await.unwrap();
    let launches = Arc::new(AtomicU32::new(0));
    let launcher = CountingLauncher(launches.clone());

    // A principal WITHOUT mcp:native.install:call (has an unrelated grant only).
    let nogrant = principal(ws, &["mcp:other.tool:call"]);

    let err = install_native(&node, &launcher, &nogrant, ws, MANIFEST, "", &[], 1)
        .await
        .expect_err("install must be refused without the grant");
    assert!(matches!(err, NativeServiceError::Denied));

    // The gate ran FIRST: no child was launched, and no durable status was written.
    assert_eq!(
        launches.load(Ordering::SeqCst),
        0,
        "the launcher must never be reached on a denied install"
    );
    let granted = principal(ws, &["mcp:native.status:call"]);
    assert!(
        status_native(&node, &granted, ws, "echo-sidecar")
            .await
            .unwrap()
            .is_none(),
        "no native_status record was written on the denied install"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn denies_stop_without_grant() {
    let ws = "native-deny-stop";
    let node = Node::boot().await.unwrap();
    let launcher = CountingLauncher(Arc::new(AtomicU32::new(0)));

    // Install with a fully-granted admin so a sidecar IS running.
    let admin = principal(ws, &["mcp:native.install:call"]);
    install_native(&node, &launcher, &admin, ws, MANIFEST, "", &[], 1)
        .await
        .expect("installs");
    assert!(node.sidecars.is_running(ws, "echo-sidecar"));

    // A different principal lacking mcp:native.stop:call cannot stop it.
    let nogrant = principal(ws, &["mcp:native.status:call"]);
    let err = stop_native(&node, &nogrant, ws, "echo-sidecar", 2)
        .await
        .expect_err("stop must be refused without the grant");
    assert!(matches!(err, NativeServiceError::Denied));
    assert!(
        node.sidecars.is_running(ws, "echo-sidecar"),
        "the sidecar must still be running after a denied stop"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn denies_reset_without_grant() {
    // The resilience rescue is capability-gated like every other native verb: re-arming a sidecar's
    // budget without `mcp:native.reset:call` is refused at the gate, before the launcher is touched.
    let ws = "native-deny-reset";
    let node = Node::boot().await.unwrap();
    let launches = Arc::new(AtomicU32::new(0));
    let launcher = CountingLauncher(launches.clone());

    let admin = principal(ws, &["mcp:native.install:call"]);
    install_native(&node, &launcher, &admin, ws, MANIFEST, "", &[], 1)
        .await
        .expect("installs");
    assert!(node.sidecars.is_running(ws, "echo-sidecar"));
    let launches_after_install = launches.load(Ordering::SeqCst);

    // A principal lacking mcp:native.reset:call cannot rescue the sidecar.
    let nogrant = principal(ws, &["mcp:native.status:call"]);
    let err = reset_native(&node, &launcher, &nogrant, ws, "echo-sidecar", 2)
        .await
        .expect_err("reset must be refused without the grant");
    assert!(matches!(err, NativeServiceError::Denied));
    // The gate ran first: no respawn happened (the launcher was not reached beyond the install spawn).
    assert_eq!(
        launches.load(Ordering::SeqCst),
        launches_after_install,
        "the launcher must never be reached on a denied reset"
    );
    assert!(node.sidecars.is_running(ws, "echo-sidecar"));
}
