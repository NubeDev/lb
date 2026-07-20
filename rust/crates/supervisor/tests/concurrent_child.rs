//! Shared harness for the multiplexed-control-line suites (native-call-concurrency scope). Proves that N
//! calls to one child overlap, that each caller gets ITS OWN reply, and that a generation boundary
//! cannot hand a post-restart reply to a pre-restart waiter.
//!
//! The fake child here is deliberately **concurrent** (it spawns each handler and writes replies
//! through one writer task, mirroring `lb_supervisor::serve`) and **delayed** — a fake that answers
//! instantly, or serially, makes every test below pass whether or not the host multiplexes. That is
//! the vacuous-green trap the federation pool-cache session hit twice; each test states what it was
//! revert-checked against.

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicU32, Ordering};
pub use std::sync::Arc;
pub use std::time::{Duration, Instant};

pub use lb_supervisor::{
    read_frame, write_frame, CallParams, Channel, Kill, Launcher, Method, Reply, Request, Sidecar,
    Spec, SupervisorError,
};
use tokio::io::duplex;
use tokio::sync::mpsc;

/// How long the fake child "works" on each call. Long enough that N serial calls are unmistakably
/// distinguishable from N concurrent ones (13 × 100 ms = 1.3 s vs ~0.1 s).
pub const WORK: Duration = Duration::from_millis(100);

/// A concurrent fake child: spawns a handler per frame, replies through ONE writer task.
pub struct ConcurrentLauncher {
    launches: Arc<AtomicU32>,
    /// Generation counter stamped into every reply, so a test can tell WHICH child answered.
    generation: Arc<AtomicU32>,
}

pub struct FakeKill;
impl Kill for FakeKill {
    fn kill(self: Box<Self>) -> Pin<Box<dyn Future<Output = ()> + Send>> {
        Box::pin(async {})
    }
}

impl Launcher for ConcurrentLauncher {
    async fn launch(
        &self,
        _exec: &str,
        _args: &[String],
        _env: &HashMap<String, String>,
    ) -> Result<Channel, SupervisorError> {
        self.launches.fetch_add(1, Ordering::SeqCst);
        let gen = self.generation.fetch_add(1, Ordering::SeqCst);

        let (host_side, child_side) = duplex(256 * 1024);
        let (mut child_read, mut child_write) = tokio::io::split(child_side);

        // One writer task owns the write half (the stdout-interleaving rule).
        let (tx, mut rx) = mpsc::channel::<Reply>(64);
        tokio::spawn(async move {
            while let Some(reply) = rx.recv().await {
                let bytes = serde_json::to_vec(&reply).unwrap();
                if write_frame(&mut child_write, &bytes).await.is_err() {
                    break;
                }
            }
        });

        tokio::spawn(async move {
            loop {
                let body = match read_frame(&mut child_read).await {
                    Ok(b) => b,
                    Err(_) => break,
                };
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
                        // Spawned, NOT awaited — keep reading while this one works.
                        let tx = tx.clone();
                        tokio::spawn(async move {
                            let p: CallParams = serde_json::from_str(&req.params).unwrap();
                            tokio::time::sleep(WORK).await;
                            // Echo the caller's OWN input back, plus which generation answered.
                            let _ = tx
                                .send(Reply::ok(
                                    req.id,
                                    format!(
                                        r#"{{"echo":{},"gen":{},"caller":{}}}"#,
                                        p.input,
                                        gen,
                                        p.caller
                                            .map(|c| format!(r#""{}""#, c.sub))
                                            .unwrap_or_else(|| "null".into())
                                    ),
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
            kill: Box::new(FakeKill),
        })
    }
}

pub fn launcher() -> (ConcurrentLauncher, Arc<AtomicU32>) {
    let launches = Arc::new(AtomicU32::new(0));
    (
        ConcurrentLauncher {
            launches: launches.clone(),
            generation: Arc::new(AtomicU32::new(0)),
        },
        launches,
    )
}

