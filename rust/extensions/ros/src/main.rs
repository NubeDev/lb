//! `ros-sidecar` — the native Tier-2 backend of the `ros` driver extension (ros-scope "Intent"). A
//! real host-platform child the supervisor spawns over stdio, with its OWN PID: it owns the long-lived
//! HTTP connections to the ROS appliances and (in later slices) the poll-timer loop. It reads its
//! injected scoped identity from the env and serves the control protocol (`init`/`health`/`call`/
//! `shutdown`) over the SAME `lb-supervisor` wire types the host uses, so the child↔host ABI cannot
//! drift.
//!
//! Stateless (rule 4): a kill + respawn loses nothing — driver config lives in the store (via the
//! `assets.*` shadow), polled values in `series`, pending writes in the outbox. The tool bodies live
//! in the library (`lib.rs` → `handlers/`); this binary is the thin supervisor loop that builds the
//! host handle once and dispatches each `call` through it.

use std::sync::Arc;

use ros_sidecar::call;
use ros_sidecar::host::HostCtx;
use ros_sidecar::poller::run::PollRegistry;
use ros_sidecar::resolve::RealFactory;

use lb_supervisor::{read_frame, write_frame, Method, Reply, Request};
use tokio::io::{stdin, stdout};

/// How often the sidecar relay scans for due `ros` outbox effects (setpoint deliveries).
const RELAY_INTERVAL_SECS: u64 = 2;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let ext_id = std::env::var("LB_EXT_ID").unwrap_or_default();

    // The host handle: the callback client + the sidecar's own grant (for the per-verb cap
    // self-check). Built once at start. If the identity env is absent the sidecar still serves the
    // control loop (health/shutdown) but every tool call fails its callback — surfaced, not panicked.
    let host = HostCtx::from_env();
    let factory = RealFactory;
    // Process-lived poll-task registry (ros_uuid → running task). Holds only live counters, no durable
    // state (rule 4): a respawn drops it and `ros.start` re-arms from the config records.
    let registry = Arc::new(PollRegistry::new());

    // Arm the sidecar relay loop: it delivers `point.write` outbox effects (must-deliver setpoints) by
    // pulling `outbox.due {target:"ros"}` and writing the box via `RosTarget`. Stateless (rule 4): the
    // durable `due` set is the state, so a respawn resumes delivery. Only armed when the host handle is
    // present (a relay with no callback can deliver nothing).
    if let Ok(host) = &host {
        let relay_factory: Arc<dyn ros_sidecar::resolve::RosApiFactory> = Arc::new(RealFactory);
        ros_sidecar::poller::relay::spawn_relay(
            host.clone(),
            relay_factory,
            std::time::Duration::from_secs(RELAY_INTERVAL_SECS),
            now_ts,
        );
    }

    let mut input = stdin();
    let mut output = stdout();

    loop {
        let body = match read_frame(&mut input).await {
            Ok(b) => b,
            Err(_) => break, // host closed the line — exit cleanly
        };
        let req: Request = match serde_json::from_slice(&body) {
            Ok(r) => r,
            Err(_) => continue,
        };

        let reply = match req.method {
            Method::Init => Reply::ok(req.id, format!(r#"{{"ready":true,"ext":"{ext_id}"}}"#)),
            Method::Health => Reply::ok(req.id, "ok"),
            Method::Shutdown => {
                let bytes = serde_json::to_vec(&Reply::ok(req.id, "bye")).unwrap();
                let _ = write_frame(&mut output, &bytes).await;
                break;
            }
            Method::Call => match &host {
                Ok(host) => call::handle(&req, host, &factory, &registry, now_ts()).await,
                Err(e) => Reply::err(req.id, format!("no host handle: {e}")),
            },
        };

        let bytes = serde_json::to_vec(&reply).unwrap();
        if write_frame(&mut output, &bytes).await.is_err() {
            break;
        }
    }
}

/// The logical timestamp threaded into shadow writes (the `assets.put_doc` `ts`). The sidecar has no
/// core clock-free contract of its own (it is an edge producer), so wall-clock here is acceptable —
/// it is data on the record, never an ordering key (the store's own seq orders).
fn now_ts() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
