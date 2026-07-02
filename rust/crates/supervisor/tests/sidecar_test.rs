//! `lb-supervisor` unit/integration — the supervision logic against an **in-memory fake child** (no
//! OS process here; the real-process restart proof lives in the host's `native_test.rs`, mock-only-
//! the-external per testing §3). A fake child echoes the protocol over an in-memory duplex so the
//! handshake, correlated `call`, `health`, `shutdown`, and `restart` paths are deterministic.

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use lb_supervisor::{
    read_frame, write_frame, Backoff, CallParams, Channel, Kill, Launcher, Method, Reply, Request,
    RestartPolicy, Sidecar, Spec,
};
use tokio::io::duplex;

/// A fake child: an in-memory duplex whose far end runs a tiny task speaking the wire protocol.
struct FakeLauncher {
    /// Bumped on every launch so the test can assert respawns happened.
    launches: Arc<AtomicU32>,
    /// If true, the spawned child exits after its `init` reply (simulates an immediate crash).
    crash_after_init: bool,
}

struct FakeKill;
impl Kill for FakeKill {
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
    ) -> Result<Channel, lb_supervisor::SupervisorError> {
        self.launches.fetch_add(1, Ordering::SeqCst);
        // host_side talks to the sidecar; child_side runs the fake child loop.
        let (host_side, child_side) = duplex(64 * 1024);
        let (mut child_read, mut child_write) = tokio::io::split(child_side);
        let crash = self.crash_after_init;
        tokio::spawn(async move {
            loop {
                let body = match read_frame(&mut child_read).await {
                    Ok(b) => b,
                    Err(_) => break, // host hung up
                };
                let req: Request = serde_json::from_slice(&body).unwrap();
                let reply = match req.method {
                    Method::Init => Reply::ok(req.id, "ready"),
                    Method::Health => Reply::ok(req.id, "ok"),
                    Method::Call => {
                        let p: CallParams = serde_json::from_str(&req.params).unwrap();
                        // echo tool: returns {"echo": <input>, "tool": <tool>}
                        Reply::ok(
                            req.id,
                            format!(r#"{{"tool":"{}","echo":{}}}"#, p.tool, p.input),
                        )
                    }
                    Method::Shutdown => Reply::ok(req.id, "bye"),
                };
                let bytes = serde_json::to_vec(&reply).unwrap();
                if write_frame(&mut child_write, &bytes).await.is_err() {
                    break;
                }
                if req.method == Method::Shutdown {
                    break;
                }
                if crash && req.method == Method::Init {
                    break; // die right after the handshake
                }
            }
        });
        let (read, write) = tokio::io::split(host_side);
        Ok(Channel {
            write: Box::pin(write),
            read: Box::pin(read),
            kill: Box::new(FakeKill),
        })
    }
}

fn launcher() -> (FakeLauncher, Arc<AtomicU32>) {
    let launches = Arc::new(AtomicU32::new(0));
    (
        FakeLauncher {
            launches: launches.clone(),
            crash_after_init: false,
        },
        launches,
    )
}

#[tokio::test]
async fn handshake_then_call_and_health() {
    let (l, launches) = launcher();
    let mut sc = Sidecar::spawn(Spec::new("fake"), &l).await.unwrap();
    assert_eq!(launches.load(Ordering::SeqCst), 1, "spawned once");

    let out = sc.call("format", r#"{"x":1}"#).await.unwrap();
    assert_eq!(out, r#"{"tool":"format","echo":{"x":1}}"#);

    sc.health().await.expect("healthy");
    assert_eq!(sc.restarts(), 0);
}

#[tokio::test]
async fn restart_relaunches_and_increments_count() {
    let (l, launches) = launcher();
    let mut sc = Sidecar::spawn(Spec::new("fake"), &l).await.unwrap();
    assert_eq!(launches.load(Ordering::SeqCst), 1);

    sc.restart(&l).await.expect("restarts");
    assert_eq!(launches.load(Ordering::SeqCst), 2, "relaunched");
    assert_eq!(sc.restarts(), 1);

    // still answers after restart
    let out = sc.call("echo", r#""hi""#).await.unwrap();
    assert_eq!(out, r#"{"tool":"echo","echo":"hi"}"#);
}

#[tokio::test]
async fn restart_budget_is_bounded() {
    let (mut l, _) = launcher();
    l.crash_after_init = false;
    let spec = Spec {
        backoff: Backoff {
            max_restarts: 2,
            ..Backoff::default()
        },
        ..Spec::new("fake")
    };
    let mut sc = Sidecar::spawn(spec, &l).await.unwrap();
    sc.restart(&l).await.expect("restart 1");
    sc.restart(&l).await.expect("restart 2");
    let err = sc.restart(&l).await.unwrap_err();
    assert!(
        matches!(err, lb_supervisor::SupervisorError::RestartExhausted(2)),
        "third restart exceeds the budget: {err:?}"
    );
}

#[tokio::test]
async fn rearm_recovers_an_exhausted_sidecar() {
    // The resilience proof: once the budget is exhausted (`restart` refuses), `rearm` re-arms it —
    // ignoring the budget — and the sidecar serves calls again. This is the operator `reset` path.
    let (l, launches) = launcher();
    let spec = Spec {
        backoff: Backoff {
            max_restarts: 1,
            ..Backoff::default()
        },
        ..Spec::new("fake")
    };
    let mut sc = Sidecar::spawn(spec, &l).await.unwrap();
    sc.restart(&l).await.expect("restart 1");
    // Budget spent: the next restart is refused (the permanent-dead-end the bug reproduced).
    assert!(matches!(
        sc.restart(&l).await.unwrap_err(),
        lb_supervisor::SupervisorError::RestartExhausted(1)
    ));

    // rearm ignores the budget: relaunch + zero the counter.
    sc.rearm(&l)
        .await
        .expect("rearm recovers the exhausted sidecar");
    assert_eq!(sc.restarts(), 0, "budget re-armed to zero");
    assert_eq!(
        launches.load(Ordering::SeqCst),
        3,
        "spawn + restart + rearm"
    );

    // It answers again — no longer a dead end.
    let out = sc.call("echo", r#""back""#).await.unwrap();
    assert_eq!(out, r#"{"tool":"echo","echo":"back"}"#);

    // And the full budget is available again (restart succeeds post-rearm).
    sc.restart(&l).await.expect("budget available after rearm");
    assert_eq!(sc.restarts(), 1);
}

#[tokio::test]
async fn reset_restarts_zeroes_the_counter_without_respawning() {
    // The decay primitive: a healthy sidecar's counter can be cleared WITHOUT touching the child
    // (no relaunch), so a subsequent fault gets the full budget again.
    let (l, launches) = launcher();
    let mut sc = Sidecar::spawn(Spec::new("fake"), &l).await.unwrap();
    sc.restart(&l).await.expect("restart");
    assert_eq!(sc.restarts(), 1);
    let before = launches.load(Ordering::SeqCst);

    sc.reset_restarts();
    assert_eq!(sc.restarts(), 0, "counter decayed");
    assert_eq!(
        launches.load(Ordering::SeqCst),
        before,
        "no respawn — the healthy child is untouched"
    );
    // Still the same live child.
    let out = sc.call("echo", r#""alive""#).await.unwrap();
    assert_eq!(out, r#"{"tool":"echo","echo":"alive"}"#);
}

#[tokio::test]
async fn rearm_refuses_a_never_policy() {
    let (l, _) = launcher();
    let spec = Spec {
        restart: RestartPolicy::Never,
        ..Spec::new("fake")
    };
    let mut sc = Sidecar::spawn(spec, &l).await.unwrap();
    assert!(matches!(
        sc.rearm(&l).await.unwrap_err(),
        lb_supervisor::SupervisorError::RestartExhausted(0)
    ));
}

#[tokio::test]
async fn never_policy_refuses_restart() {
    let (l, _) = launcher();
    let spec = Spec {
        restart: RestartPolicy::Never,
        ..Spec::new("fake")
    };
    let mut sc = Sidecar::spawn(spec, &l).await.unwrap();
    assert!(matches!(
        sc.restart(&l).await.unwrap_err(),
        lb_supervisor::SupervisorError::RestartExhausted(0)
    ));
}

#[tokio::test]
async fn shutdown_ends_the_sidecar() {
    let (l, _) = launcher();
    let mut sc = Sidecar::spawn(Spec::new("fake"), &l).await.unwrap();
    sc.shutdown().await;
    // After shutdown a call has no channel → transport error.
    assert!(sc.call("echo", "1").await.is_err());
}
