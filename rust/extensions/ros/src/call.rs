//! Tool dispatch for the `ros-sidecar` (FILE-LAYOUT: the stdio loop is `main.rs`, the tool handling is
//! here; each verb's body lives under `handlers/`). This slice wires the CRUD tree + `ros.ping`; the
//! poller verbs (`ros.start|stop|status`, slice 3) and `point.write` (slice 4) are the remaining
//! unknown-tool arms.
//!
//! Every verb runs its own capability self-check inside its handler (`host.require`) — the inbound
//! `native.call` carries no caller identity, so the fine-grained `mcp:<verb>:call` gate is the
//! sidecar's job (see `host.rs`). A denial is an opaque error reply, indistinguishable from any other
//! refusal.

use lb_supervisor::{CallParams, Reply, Request};

use crate::handlers::{dispatch, parse_input};
use crate::host::{HostCtx, HostError};
use crate::resolve::RosApiFactory;

/// Handle a `call`: parse the tool + input, run the CRUD dispatcher, then (later slices) the poller /
/// point.write. `host` is the callback + grant handle; `factory` builds a `RosApi` per connection;
/// `ts` is the logical timestamp for shadow writes. Async because the verbs await REST round-trips and
/// host callbacks.
pub async fn handle(req: &Request, host: &HostCtx, factory: &dyn RosApiFactory, ts: u64) -> Reply {
    let params: CallParams = match serde_json::from_str(&req.params) {
        Ok(p) => p,
        Err(e) => return Reply::err(req.id, format!("bad params: {e}")),
    };

    let input = match parse_input(&params.input) {
        Ok(v) => v,
        Err(e) => return Reply::err(req.id, e.to_string()),
    };

    match dispatch(host, factory, &params.tool, &input, ts).await {
        Ok(Some(result)) => Reply::ok(req.id, result),
        // Not a CRUD verb — slices 3/4 add the poller + point.write arms; until then it is unknown.
        Ok(None) => Reply::err(req.id, format!("unknown tool: {}", params.tool)),
        // A capability denial is an opaque error (no oracle); other errors carry a diagnostic message
        // (never the token — HostError never holds secret material).
        Err(HostError::Denied) => Reply::err(req.id, "denied"),
        Err(e) => Reply::err(req.id, e.to_string()),
    }
}

// The CRUD verbs are exercised end-to-end against a REAL spawned gateway + store + secrets (only the
// ROS box faked behind `RosApi`) in `tests/crud_test.rs` — the cap-deny, workspace-isolation, and
// token-never-returned proofs the scope mandates. No mocked host here (CLAUDE rule 9 / testing §0).
