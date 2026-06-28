//! `lb-acp` — the ACP stdio adapter binary an editor (Zed/Cursor) launches (agent-run scope Part 4).
//! It boots a real node, authenticates a trusted session bound to one workspace, and runs the
//! JSON-RPC stdio loop over the [`AcpSession`](lb_role_acp::AcpSession) driver.
//!
//! Config is environment, never the wire (symmetric with the gateway's trusted-key rule — an editor
//! cannot self-elevate by what it sends):
//!   - `LB_ACP_WS`     — the workspace the session binds to (required).
//!   - `LB_ACP_USER`   — the principal sub (default `user:acp`).
//!   - `LB_ACP_TOOLS`  — comma-separated qualified MCP tool names the model may propose (optional).
//!   - `LB_ACP_MOCK_SCRIPT` — a JSON array of `AiResponse` (the deterministic provider script). The
//!     model provider is the ONE permitted fake (testing §3); feeding it via env lets a test spawn
//!     this REAL binary and drive it deterministically over a REAL stdio pipe (rule 9 — no fakes of
//!     anything else). A real provider adapter swaps in here behind the same `Provider` seam.
//!
//! The signing key is generated at boot and used to BOTH mint the session token and verify it — the
//! verify path is the real trusted-session check (`lb_auth::verify`); in dev the binary stands in for
//! the IdP that would otherwise hand it the token. Binding to one workspace is the wall (§7).

use std::sync::Arc;

use lb_auth::{mint, Claims, Role, SigningKey};
use lb_host::{AllowedTool, Node};
use lb_role_acp::{serve_stdio, AcpSession};
use lb_role_ai_gateway::{AiGateway, AiResponse, MockProvider};

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let ws = std::env::var("LB_ACP_WS").unwrap_or_else(|_| "default".into());
    let user = std::env::var("LB_ACP_USER").unwrap_or_else(|_| "user:acp".into());
    let tools = parse_tools(&std::env::var("LB_ACP_TOOLS").unwrap_or_default());
    let agent_caps = tools
        .iter()
        .map(|t| format!("mcp:{}:call", t.name))
        .collect::<Vec<_>>();

    // Boot a real node (the adapter IS a node — symmetric, §3.1). A persistent store path could be
    // wired via env later; dev uses in-memory.
    let node = Arc::new(Node::boot().await.expect("node boots"));

    // The deterministic provider script (the one permitted fake). Empty → a provider that always
    // stops (a no-op agent), so the binary still runs without a script.
    let script: Vec<AiResponse> = std::env::var("LB_ACP_MOCK_SCRIPT")
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    let model = Arc::new(AiGateway::new(MockProvider::new(script)));

    // Mint + (later, in the driver) verify the session token — the trusted-session path. The agent's
    // own caps include the invoke + watch + the proposable tools, so the run can drive + be observed.
    let now = 1; // logical clock seed (no wall-clock — testing §3); the driver bumps it per turn.
    let key = SigningKey::generate();
    let mut caps = vec![
        "mcp:agent.invoke:call".to_string(),
        "mcp:agent.watch:call".to_string(),
    ];
    caps.extend(agent_caps.iter().cloned());
    let claims = Claims {
        sub: user,
        ws: ws.clone(),
        role: Role::Member,
        caps: caps.clone(),
        iat: 0,
        exp: u64::MAX,
    };
    let token = mint(&key, &claims);

    let session = AcpSession::authenticate(node, model, &key, &token, now, caps, tools)
        .expect("trusted session authenticates");

    serve_stdio(session, tokio::io::stdin(), tokio::io::stdout()).await
}

/// Parse `a.b,c.d` into `AllowedTool`s (description = the name; the editor sees the qualified name).
fn parse_tools(raw: &str) -> Vec<AllowedTool> {
    raw.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|name| AllowedTool {
            name: name.to_string(),
            description: name.to_string(),
        })
        .collect()
}
