//! `fleet-monitor` backend — the native Tier-2 half of the self-contained `fleet-monitor` extension
//! (native-tier scope). A real OS child the host supervises: it has its OWN PID, reads its injected
//! scoped identity from the env, and serves the control protocol (`init`/`health`/`call`/`shutdown`)
//! over `Content-Length`-framed stdio using the SAME `lb-supervisor` wire types the host uses — so the
//! child↔host ABI cannot drift (the native peer of the wasm tier sharing the WIT world).
//!
//! It is stateless (§3.4): it holds nothing durable. A kill + respawn loses nothing — the host's
//! record is the truth. Its tools live in `call.rs` (FILE-LAYOUT: the loop is one responsibility, the
//! tool dispatch another). The extension's FRONTEND half is co-located under `../ui/` (a federated
//! shadcn page + two widgets) — one folder, backend + frontend together.

mod call;

use lb_supervisor::{read_frame, write_frame, Method, Reply, Request};
use tokio::io::{stdin, stdout};

#[tokio::main(flavor = "current_thread")]
async fn main() {
    // Injected identity (native-tier scope): the host spawns us with our scoped ids in env.
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

        let reply = match req.method {
            Method::Init => Reply::ok(req.id, format!(r#"{{"ready":true,"ext":"{ext_id}"}}"#)),
            Method::Health => Reply::ok(req.id, "ok"),
            Method::Shutdown => {
                let bytes = serde_json::to_vec(&Reply::ok(req.id, "bye")).unwrap();
                let _ = write_frame(&mut output, &bytes).await;
                break;
            }
            Method::Call => call::handle(&req, &ws),
        };

        let bytes = serde_json::to_vec(&reply).unwrap();
        if write_frame(&mut output, &bytes).await.is_err() {
            break;
        }
    }
}
