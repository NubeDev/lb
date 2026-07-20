//! Native-tier **concurrency** — the mandatory categories (testing §2) re-proven under overlap
//! (native-call-concurrency scope, tests 3 and 4).
//!
//! The capability gate and the workspace wall were already tested serially. They are re-tested here
//! because multiplexing introduces a shared pending-reply map, and a shared map is exactly where an
//! authorization result could leak between callers: a deny that satisfied someone else's waiter
//! would be a capability bypass that *looks* like a successful call. Serial tests cannot see that.
//!
//! Real store (`Node::boot`, `mem://`), real capability gate, real supervisor. The child is an
//! in-memory fake **process** (mock only the true external, testing §0) — and it is deliberately
//! CONCURRENT and slow, because a child that answers instantly or serially makes every assertion
//! below pass whether or not the transport multiplexes.

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{call_sidecar, install_native, NativeServiceError, Node};
use lb_supervisor::{
    read_frame, write_frame, CallParams, Channel, Kill, Launcher, Method, Reply, Request,
    SupervisorError,
};
use tokio::io::duplex;
use tokio::sync::mpsc;

const MANIFEST: &str = include_str!("../../../extensions/echo-sidecar/extension.toml");

/// Each fake call "works" this long, so N serial calls are unmistakable from N concurrent ones.
const WORK: Duration = Duration::from_millis(100);

/// A concurrent fake child: handler per frame, one writer task, echoes the caller's own input back
/// (so a reply can be attributed to the caller that asked) plus the ws it was launched for.
struct ConcurrentLauncher {
    ws: String,
    /// Counts `call` FRAMES the child actually received — the quantity a wasted retry inflates.
    calls: Arc<AtomicU32>,
}
struct NoKill;
impl Kill for NoKill {
    fn kill(self: Box<Self>) -> Pin<Box<dyn Future<Output = ()> + Send>> {
        Box::pin(async {})
    }
}

impl Launcher for ConcurrentLauncher {
    async fn launch(
        &self,
        _exec: &str,
        _args: &[String],
        env: &HashMap<String, String>,
    ) -> Result<Channel, SupervisorError> {
        // The host injects LB_EXT_WS; echo it so a reply proves WHICH workspace's child answered.
        let ws = env
            .get("LB_EXT_WS")
            .cloned()
            .unwrap_or_else(|| self.ws.clone());
        let calls = Arc::clone(&self.calls);
        let (host_side, child_side) = duplex(256 * 1024);
        let (mut cr, mut cw) = tokio::io::split(child_side);

        let (tx, mut rx) = mpsc::channel::<Reply>(64);
        tokio::spawn(async move {
            while let Some(reply) = rx.recv().await {
                let bytes = serde_json::to_vec(&reply).unwrap();
                if write_frame(&mut cw, &bytes).await.is_err() {
                    break;
                }
            }
        });

        tokio::spawn(async move {
            while let Ok(body) = read_frame(&mut cr).await {
                let req: Request = serde_json::from_slice(&body).unwrap();
                match req.method {
                    Method::Init => {
                        let _ = tx.send(Reply::ok(req.id, "ready")).await;
                    }
                    Method::Health => {
                        let _ = tx.send(Reply::ok(req.id, "ok")).await;
                    }
                    Method::Shutdown => {
                        let _ = tx.send(Reply::ok(req.id, "bye")).await;
                        break;
                    }
                    Method::Call => {
                        calls.fetch_add(1, Ordering::SeqCst);
                        let tx = tx.clone();
                        let ws = ws.clone();
                        tokio::spawn(async move {
                            let p: CallParams = serde_json::from_str(&req.params).unwrap();
                            tokio::time::sleep(WORK).await;
                            let _ = tx
                                .send(Reply::ok(
                                    req.id,
                                    format!(r#"{{"echo":{},"ws":"{}"}}"#, p.input, ws),
                                ))
                                .await;
                        });
                    }
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
        sub: format!("user:{ws}"),
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

const INSTALL_CAPS: &[&str] = &["mcp:native.install:call", "mcp:native.call:call"];

/// **Test 3 (mandatory) — the capability gate holds under concurrency, and a deny never satisfies
/// another caller's waiter.**
///
/// Interleaves authorized and unauthorized calls against the SAME child. Every deny must be denied,
/// every allow must return ITS OWN result. The dangerous failure is not a missed deny — it is a
/// denied caller's waiter being completed by an authorized caller's reply (or vice versa), which
/// would read as success on both sides.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn capability_deny_holds_under_concurrent_calls() {
    let node = Node::boot().await.unwrap();
    let launcher = ConcurrentLauncher { ws: "acme".into(), calls: Arc::new(AtomicU32::new(0)) };
    let admin = principal("acme", INSTALL_CAPS);
    install_native(&node, &launcher, &admin, "acme", MANIFEST, "", &[], 1)
        .await
        .expect("installs");

    // Same workspace, but WITHOUT `mcp:native.call:call`.
    let ungranted = principal("acme", &["mcp:native.status:call"]);

    let node = Arc::new(node);
    let launcher = Arc::new(launcher);
    let mut tasks = Vec::new();

    for i in 0..12u32 {
        let node = Arc::clone(&node);
        let launcher = Arc::clone(&launcher);
        let granted = i % 2 == 0; // interleave allow / deny
        let caller = if granted {
            principal("acme", INSTALL_CAPS)
        } else {
            ungranted.clone()
        };
        tasks.push(tokio::spawn(async move {
            let out = call_sidecar(
                &node,
                &*launcher,
                &caller,
                "acme",
                "echo-sidecar",
                "echo",
                &i.to_string(),
                10 + i as u64,
            )
            .await;
            (i, granted, out)
        }));
    }

    for t in tasks {
        let (i, granted, out) = t.await.unwrap();
        if granted {
            let body = out.unwrap_or_else(|e| panic!("granted call {i} failed: {e:?}"));
            let v: serde_json::Value = serde_json::from_str(&body).unwrap();
            assert_eq!(
                v["echo"].as_u64().unwrap(),
                i as u64,
                "granted caller {i} received another caller's reply: {body}"
            );
        } else {
            assert!(
                matches!(out, Err(NativeServiceError::Denied)),
                "ungranted caller {i} was NOT denied under concurrency: {out:?}"
            );
        }
    }
}

/// **Test 4 (mandatory) — the workspace wall holds under concurrency, including colliding ids.**
///
/// Two workspaces each run their own child. Ids are per-sidecar, so both workspaces WILL use the
/// same id values concurrently — the case where a node-global pending map would cross-deliver. Each
/// reply must come from the caller's own workspace's child (the fake stamps `ws` for exactly this).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn workspace_isolation_holds_under_concurrent_calls_with_colliding_ids() {
    let node = Node::boot().await.unwrap();
    let launcher = ConcurrentLauncher { ws: "unset".into(), calls: Arc::new(AtomicU32::new(0)) };

    for ws in ["ws-a", "ws-b"] {
        let admin = principal(ws, INSTALL_CAPS);
        install_native(&node, &launcher, &admin, ws, MANIFEST, "", &[], 1)
            .await
            .unwrap_or_else(|e| panic!("{ws} installs: {e:?}"));
    }

    let node = Arc::new(node);
    let launcher = Arc::new(launcher);
    let mut tasks = Vec::new();

    // Both workspaces fire the same sequence at the same time → identical ids in flight.
    for i in 0..8u32 {
        for ws in ["ws-a", "ws-b"] {
            let node = Arc::clone(&node);
            let launcher = Arc::clone(&launcher);
            let caller = principal(ws, INSTALL_CAPS);
            tasks.push(tokio::spawn(async move {
                let out = call_sidecar(
                    &node,
                    &*launcher,
                    &caller,
                    ws,
                    "echo-sidecar",
                    "echo",
                    &i.to_string(),
                    20 + i as u64,
                )
                .await;
                (ws, i, out)
            }));
        }
    }

    for t in tasks {
        let (ws, i, out) = t.await.unwrap();
        let body = out.unwrap_or_else(|e| panic!("{ws} call {i} failed: {e:?}"));
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(
            v["ws"].as_str().unwrap(),
            ws,
            "a {ws} call was answered by ANOTHER workspace's child: {body}"
        );
        assert_eq!(
            v["echo"].as_u64().unwrap(),
            i as u64,
            "{ws} caller {i} received another caller's reply: {body}"
        );
    }
}

/// **Test 1 at the host layer — the routed path actually overlaps.**
///
/// The supervisor-level test proves `Conn` multiplexes; this proves the HOST stopped serializing on
/// top of it. `native/call.rs` used to hold the per-sidecar mutex across the round-trip, which would
/// re-impose concurrency 1 no matter how well `Conn` behaves.
///
/// REVERT-CHECK: restoring `handle.lock().await.call_with_caller(..).await` in `attempt()` (holding
/// the guard across the await) takes this from ~0.1 s to ~1.3 s → RED.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_host_call_path_does_not_serialize() {
    let node = Node::boot().await.unwrap();
    let launcher = ConcurrentLauncher { ws: "acme".into(), calls: Arc::new(AtomicU32::new(0)) };
    let admin = principal("acme", INSTALL_CAPS);
    install_native(&node, &launcher, &admin, "acme", MANIFEST, "", &[], 1)
        .await
        .expect("installs");

    let node = Arc::new(node);
    let launcher = Arc::new(launcher);

    let start = Instant::now();
    let mut tasks = Vec::new();
    for i in 0..13u32 {
        let node = Arc::clone(&node);
        let launcher = Arc::clone(&launcher);
        let caller = principal("acme", INSTALL_CAPS);
        tasks.push(tokio::spawn(async move {
            call_sidecar(
                &node,
                &*launcher,
                &caller,
                "acme",
                "echo-sidecar",
                "echo",
                &i.to_string(),
                30 + i as u64,
            )
            .await
        }));
    }
    for t in tasks {
        t.await.unwrap().expect("call succeeds");
    }
    let elapsed = start.elapsed();

    assert!(
        elapsed < WORK * 4,
        "13 concurrent host calls took {elapsed:?}; serial would be ~{:?}. \
         The host is still holding the sidecar lock across the round-trip.",
        WORK * 13
    );
}
