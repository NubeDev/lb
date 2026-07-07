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

use axum::Router;

#[path = "test_gateway_seed.rs"]
mod test_gateway_seed;

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

    // Boot-seed the built-in agent definitions into the reserved `_lb_agents` namespace, mirroring the
    // production `node` binary (agent-catalog scope). The UI catalog test reads these back over the
    // real read routes — seeding through the real boot path, not faking (testing §0).
    if let Err(e) = lb_host::seed_agent_definitions(&gw.node.store).await {
        eprintln!("test_gateway: agent-definition seed failed: {e}");
    }

    // Boot-seed the built-in personas into the reserved `_lb_personas` namespace, mirroring the
    // production `node` binary (agent-personas scope #1). The UI persona picker test reads these back
    // over the real read routes — seeding through the real boot path, not faking (testing §0).
    if let Err(e) = lb_host::seed_personas(&gw.node.store).await {
        eprintln!("test_gateway: persona seed failed: {e}");
    }

    // Legacy `active_persona` → ws-default prefs migration (persona-session #5), mirroring the
    // production boot: one-shot, idempotent, right after the persona seed.
    if let Err(e) = lb_host::migrate_active_persona(&gw.node.store).await {
        eprintln!("test_gateway: active_persona migration failed: {e}");
    }

    // APPROVAL-RELEASE REACTOR TICK (rules-approvals scope): the production `node` binary spawns this
    // beside the flow/agent reactors; the UI's `RulesApprovals.gateway.test.tsx` drives the full loop
    // (request_approval → held effect → approve → RELEASED) end to end, so the tick must run here too.
    // Scans the fixed `rules-approvals` workspace the approvals gateway test signs into (the reactor
    // needs a known ws list; the test uses distinct item ids per case within it). A 1s cadence keeps
    // the test's approve→released wait short; each tick is a cheap ws-scoped scan and the release is a
    // guarded (idempotent) transition.
    lb_host::spawn_approval_reactors(
        gw.node.clone(),
        vec!["rules-approvals".to_string()],
        std::time::Duration::from_secs(1),
    );

    // Bind first so we can print the actual assigned port (when PORT=0) before serving.
    let listener = tokio::net::TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], port))).await?;
    let addr = listener.local_addr()?;
    // The harness greps this line to learn the URL.
    println!("LISTENING http://{addr}");

    // Mount the production router PLUS the test-only `/_seed/*` routes (real host writes for surfaces
    // with no public create route — seeding, not faking). These exist only in this test binary. The
    // seed routes carry their own state; merge them with the production router (state already applied).
    let seed = test_gateway_seed::seed_routes(Router::new()).with_state(gw.clone());
    let app = lb_role_gateway::router(gw).merge(seed);
    axum::serve(listener, app).await?;
    Ok(())
}
