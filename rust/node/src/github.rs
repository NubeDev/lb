//! The github-workflow role wiring — env-gated, mounted from `main.rs`. This is the thin role-aware
//! layer §3.1 permits in the *binary*: no core crate is role-aware; the decision to run the webhook
//! ingress + the background driver lives here, keyed off config (env), never an `if cloud`.
//!
//! Two pieces, each independently optional:
//!   - `LB_WEBHOOK_ADDR` + `LB_WEBHOOK_SECRET` (+ `LB_WORKFLOW_WS`) → serve the single-tenant webhook
//!     front door, so a real GitHub delivery drives `ingest_via_bridge`.
//!   - `LB_WORKFLOW_WS` + `LB_GITHUB_API` (+ `LB_GITHUB_TOKEN`) → spawn the background driver loop:
//!     every `LB_WORKFLOW_TICK_SECS` it runs a reactor pass (auto-start approved jobs) + a relay pass
//!     (deliver PR/comment effects through the real GitHub `Target`), per configured workspace.
//!
//! `now` enters here, at the binary boundary, as wall-clock seconds — the no-wall-clock rule keeps
//! time out of the *core crates* (testing §3), and the binary is where real time legitimately enters.

use std::sync::Arc;
use std::time::Duration;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{register_workspace, Node};
use lb_role_github_target::GithubTarget;
use lb_role_github_webhook::{serve_tenants, TenantRegistry, WebhookTenant};
use lb_role_github_workflow::run_directory_loop;

/// Wall-clock seconds since the Unix epoch — the driver's `now` at the binary boundary.
fn unix_seconds() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// The service principal the unattended workflow acts as in `ws` — holds exactly the workflow caps
/// the ingress + reactor need. (A real login→token→principal session replaces this demo identity
/// later, the same follow-up the gateway's demo principal carries.)
fn service_principal(ws: &str) -> Principal {
    let key = SigningKey::generate();
    let claims = Claims {
        sub: "ext:coding-workflow".into(),
        ws: ws.into(),
        role: Role::Member,
        caps: vec![
            "mcp:github-bridge.normalize:call".into(),
            "mcp:workflow.ingest_issue:call".into(),
            "mcp:workflow.request_approval:call".into(),
            "mcp:workflow.resolve_approval:call".into(),
            "mcp:workflow.start_job:call".into(),
            "bus:chan/*:pub".into(),
        ],
        iat: 0,
        exp: u64::MAX,
    };
    verify(&key, &mint(&key, &claims), 1).expect("freshly minted token verifies")
}

/// Mount the github-workflow role on `node` per the environment. Spawns the webhook server and/or the
/// driver loop as background tasks; returns once they are spawned (the caller keeps the process
/// alive). A no-op if neither piece is configured. Async because it **seeds the workflow directory**
/// with the configured workspace before starting the directory-backed driver.
pub async fn mount(node: Arc<Node>) {
    let Ok(ws) = std::env::var("LB_WORKFLOW_WS") else {
        return; // The workflow role is not configured — solo node.
    };

    // The webhook front door (ingress), if an address + secret are configured.
    if let (Ok(addr), Ok(secret)) = (
        std::env::var("LB_WEBHOOK_ADDR"),
        std::env::var("LB_WEBHOOK_SECRET"),
    ) {
        if let Ok(addr) = addr.parse::<std::net::SocketAddr>() {
            let tenant =
                WebhookTenant::new(service_principal(&ws), ws.clone(), secret.into_bytes());
            // One tenant slug `default` for the single-workspace deployment; multi-tenant config is a
            // map the binary could read from a file (the registry already supports many tenants).
            let registry = TenantRegistry::new(node.clone(), [("default".to_string(), tenant)]);
            println!("github-webhook: serving workspace '{ws}' on http://{addr}/webhook/default");
            tokio::spawn(async move {
                if let Err(e) = serve_tenants(registry, addr).await {
                    eprintln!("github-webhook server stopped: {e}");
                }
            });
        }
    }

    // The background driver (reactor + relay), if a GitHub API base is configured.
    if let Ok(api) = std::env::var("LB_GITHUB_API") {
        let token = std::env::var("LB_GITHUB_TOKEN").unwrap_or_default();
        let tick = std::env::var("LB_WORKFLOW_TICK_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(5);

        // SEED the durable directory with the configured workspace, then drive from the directory —
        // so an operator can `register_workspace` more workspaces at runtime and the next tick picks
        // them up, no restart. `now` is wall-clock seconds (the binary is the clock boundary).
        if let Err(e) = register_workspace(&node.store, &ws, "progress", unix_seconds()).await {
            eprintln!("github-workflow: failed to seed directory for '{ws}': {e}");
            return;
        }
        let target = GithubTarget::new(&api, &token);
        println!("github-workflow: driving the directory (seeded '{ws}') every {tick}s → {api}");
        tokio::spawn(async move {
            run_directory_loop(
                &node,
                target,
                Duration::from_secs(tick),
                |ws| service_principal(ws),
                unix_seconds,
                |ws, e| eprintln!("workflow driver [{ws}]: {e}"),
            )
            .await;
        });
    }
}
