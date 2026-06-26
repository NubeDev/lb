//! `echo-sidecar` — the reference native Tier-2 extension (native-tier scope). A real OS child the
//! host supervises: it reads its injected scoped identity from the env, then serves the control
//! protocol (`init`/`health`/`call`/`shutdown`) over `Content-Length`-framed stdio using the SAME
//! `lb-supervisor` wire types the host uses — so the child↔host ABI cannot drift (the native peer of
//! the wasm tier sharing the WIT world).
//!
//! It is stateless (§3.4): it holds nothing durable. A kill + respawn loses nothing — the host's
//! record is the truth. Its one tool, `echo`, returns its input plus the workspace it was spawned in
//! (read from `LB_EXT_WS`), proving the injected identity reached the child.
//!
//! Its `crash` tool replies then exits the process — a DETERMINISTIC crash the supervision/restart
//! test triggers (reply-then-exit, so "induce the crash" is observable and separate from "verify the
//! restart": the supervisor's NEXT call sees the dead child and restarts it). No env-var/kill racing.

use lb_supervisor::{read_frame, write_frame, CallParams, Method, Reply, Request};
use tokio::io::{stdin, stdout};

#[tokio::main(flavor = "current_thread")]
async fn main() {
    // Injected identity (native-tier scope): the host spawns us with our scoped token + ids in env.
    let ws = std::env::var("LB_EXT_WS").unwrap_or_default();
    let ext_id = std::env::var("LB_EXT_ID").unwrap_or_default();

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

        let mut crash_after_reply = false;
        let reply = match req.method {
            Method::Init => Reply::ok(req.id, format!(r#"{{"ready":true,"ext":"{ext_id}"}}"#)),
            Method::Health => Reply::ok(req.id, "ok"),
            Method::Shutdown => {
                // Acknowledge, then break so the process exits cooperatively.
                let bytes = serde_json::to_vec(&Reply::ok(req.id, "bye")).unwrap();
                let _ = write_frame(&mut output, &bytes).await;
                break;
            }
            Method::Call => {
                let (reply, crash) = handle_call(&req, &ws);
                crash_after_reply = crash;
                reply
            }
        };

        let bytes = serde_json::to_vec(&reply).unwrap();
        if write_frame(&mut output, &bytes).await.is_err() {
            break;
        }
        // Deterministic crash: reply landed, now exit — the supervisor's next call sees EOF + restarts.
        if crash_after_reply {
            std::process::exit(7);
        }
    }
}

/// Handle a `call`: parse the tool + input. `crash` replies then signals the caller to exit (the
/// restart trigger); `echo` returns the input with the workspace identity attached. Returns the
/// reply and whether to crash after sending it.
fn handle_call(req: &Request, ws: &str) -> (Reply, bool) {
    let params: CallParams = match serde_json::from_str(&req.params) {
        Ok(p) => p,
        Err(e) => return (Reply::err(req.id, format!("bad params: {e}")), false),
    };

    match params.tool.as_str() {
        "crash" => (Reply::ok(req.id, r#""crashing""#), true),
        "echo" => (
            Reply::ok(
                req.id,
                format!(r#"{{"echo":{},"ws":"{ws}"}}"#, params.input),
            ),
            false,
        ),
        other => (Reply::err(req.id, format!("unknown tool: {other}")), false),
    }
}
