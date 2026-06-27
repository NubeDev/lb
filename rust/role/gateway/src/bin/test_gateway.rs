//! A **test-only** real gateway server for the UI's Vitest harness (data-console scope: the frontend
//! tests must run against a REAL in-process node, not a `*.fake.ts` — CLAUDE §9, testing §0). This is
//! the smallest real-node harness the "retire the fakes" migration (STATUS Next-up #00) needs: boot a
//! real gateway-role node + the SSE/HTTP router and serve it on `$PORT`, so a Node test process can
//! `fetch` against it, `POST /login` for a real signed token, seed real rows through the real write
//! path, and drive the UI exactly as a browser would.
//!
//! It is NOT a production entry point (that is the `node` binary, role-by-config). It exists so the UI
//! suite has a real backend to talk to. Boots on `127.0.0.1:$PORT` (default 0 = an OS-assigned port,
//! printed as `LISTENING <addr>` so the harness can read it back).

use std::net::SocketAddr;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(0);

    // A real gateway-role node with a fresh signing key and the real clock — `POST /login` mints a
    // real token carrying the dev claim set (which includes the data-console caps), and every other
    // route verifies it. The same code path a deployed node runs; only the credential check is the
    // dev-login stand-in (collaboration scope).
    let gw = lb_role_gateway::Gateway::boot().await?;

    // Bind first so we can print the actual assigned port (when PORT=0) before serving.
    let listener = tokio::net::TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], port))).await?;
    let addr = listener.local_addr()?;
    // The harness greps this line to learn the URL.
    println!("LISTENING http://{addr}");

    axum::serve(listener, lb_role_gateway::router(gw)).await?;
    Ok(())
}
